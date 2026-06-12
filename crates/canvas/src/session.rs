use crate::gesture::{CanvasGestureSession, CanvasPreparedGestureCommit};
use crate::record_scope::normalize_selection;
use crate::tool::{CanvasSelection, CanvasTool};
use crate::{
    CanvasDocument, CanvasKindRegistry, CanvasResizeHandle, CanvasSnapGuide, CanvasTransaction,
    CanvasViewport, DocumentError, EdgeId, HitTarget, NodeId, ShapeId,
};
use open_gpui::{Axis, Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ToolState {
    Idle,
    Pointing {
        origin: Point<Pixels>,
        selection_mode: crate::tool::CanvasSelectionMode,
        base_selection: CanvasSelection,
    },
    Selecting {
        origin: Point<Pixels>,
        current: Point<Pixels>,
        selection_mode: crate::tool::CanvasSelectionMode,
        base_selection: CanvasSelection,
    },
    Translating {
        origin: Point<Pixels>,
        last: Point<Pixels>,
        constraint_axis: Option<Axis>,
        node_ids: Vec<NodeId>,
        shape_ids: Vec<ShapeId>,
        snap_guides: Vec<CanvasSnapGuide>,
    },
    Resizing {
        origin: Point<Pixels>,
        last: Point<Pixels>,
        handle: CanvasResizeHandle,
        node_ids: Vec<NodeId>,
        edge_ids: Vec<EdgeId>,
        shape_ids: Vec<ShapeId>,
        structural: bool,
        snap_guides: Vec<CanvasSnapGuide>,
    },
    Panning {
        origin: Point<Pixels>,
        last: Point<Pixels>,
    },
    Connecting {
        source: crate::CanvasEndpoint,
        current: Point<Pixels>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasToolSession {
    pub(crate) viewport: CanvasViewport,
    pub(crate) tool: CanvasTool,
    pub(crate) state: ToolState,
    pub(crate) selection: CanvasSelection,
    gesture: Option<CanvasGestureSession>,
}

impl CanvasToolSession {
    pub(crate) fn new() -> Self {
        Self {
            viewport: CanvasViewport::default(),
            tool: CanvasTool::Select,
            state: ToolState::Idle,
            selection: CanvasSelection::default(),
            gesture: None,
        }
    }

    pub(crate) fn viewport(&self) -> CanvasViewport {
        self.viewport
    }

    pub(crate) fn tool(&self) -> &CanvasTool {
        &self.tool
    }

    pub(crate) fn state(&self) -> &ToolState {
        &self.state
    }

    pub(crate) fn selection(&self) -> &CanvasSelection {
        &self.selection
    }

    pub(crate) fn snapshot(&self) -> CanvasToolSessionSnapshot {
        CanvasToolSessionSnapshot {
            viewport: self.viewport,
            selection: self.selection.clone(),
            state: self.state.clone(),
        }
    }

    pub(crate) fn retain_selection_for_document(&mut self, document: &CanvasDocument) {
        self.selection = normalize_selection(document, &self.selection);
    }

    pub(crate) fn set_selection(&mut self, selection: CanvasSelection, document: &CanvasDocument) {
        self.selection = normalize_selection(document, &selection);
    }

    pub(crate) fn apply_effect(
        &mut self,
        effect: CanvasToolSessionEffect,
        document: &CanvasDocument,
    ) {
        match effect {
            CanvasToolSessionEffect::SetSelection(selection) => {
                self.set_selection(selection, document);
            }
            CanvasToolSessionEffect::ReplaceSelection(target) => {
                self.replace_selection(target, document);
            }
            CanvasToolSessionEffect::AddSelection(target) => {
                self.add_selection(target, document);
            }
            CanvasToolSessionEffect::RemoveSelection(target) => {
                self.remove_selection(&target, document);
            }
            CanvasToolSessionEffect::ToggleSelection(target) => {
                self.toggle_selection(target, document);
            }
            CanvasToolSessionEffect::ClearSelection => {
                self.clear_selection();
            }
            CanvasToolSessionEffect::SetState(state) => {
                self.set_state(state);
            }
            CanvasToolSessionEffect::PanViewport(delta) => {
                self.pan_viewport(delta);
            }
            CanvasToolSessionEffect::SetViewport(viewport) => {
                self.set_viewport(viewport);
            }
        }
    }

    pub(crate) fn replace_selection(&mut self, target: HitTarget, document: &CanvasDocument) {
        let mut selection = CanvasSelection::default();
        selection.insert_target(target);
        self.set_selection(selection, document);
    }

    pub(crate) fn add_selection(&mut self, target: HitTarget, document: &CanvasDocument) {
        let mut selection = self.selection.clone();
        selection.insert_target(target);
        self.set_selection(selection, document);
    }

    pub(crate) fn remove_selection(&mut self, target: &HitTarget, document: &CanvasDocument) {
        let mut selection = self.selection.clone();
        selection.remove_target(target);
        self.set_selection(selection, document);
    }

    pub(crate) fn toggle_selection(&mut self, target: HitTarget, document: &CanvasDocument) {
        let mut selection = self.selection.clone();
        selection.toggle_target(target);
        self.set_selection(selection, document);
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub(crate) fn set_state(&mut self, state: ToolState) {
        self.state = state;
    }

    pub(crate) fn set_viewport(&mut self, viewport: CanvasViewport) {
        self.viewport = viewport;
    }

    pub(crate) fn pan_viewport(&mut self, delta: Point<Pixels>) {
        self.viewport.pan_by(delta);
    }

    pub(crate) fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.state = ToolState::Idle;
    }

    pub(crate) fn reset_for_kind_registry_change(&mut self, document: &CanvasDocument) {
        self.retain_selection_for_document(document);
        self.gesture = None;
    }

    pub(crate) fn begin_gesture(&mut self, document: &CanvasDocument) {
        if self.gesture.is_none() {
            self.gesture = Some(CanvasGestureSession::begin(document));
        }
    }

    pub(crate) fn begin_implicit_gesture(
        &mut self,
        document: &CanvasDocument,
    ) -> Option<CanvasGestureSession> {
        self.gesture
            .is_none()
            .then(|| CanvasGestureSession::begin(document))
    }

    pub(crate) fn install_implicit_gesture(&mut self, gesture: CanvasGestureSession) {
        self.gesture = Some(gesture);
    }

    pub(crate) fn prepare_gesture_commit(
        &self,
        current: &CanvasDocument,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<Option<CanvasPreparedGestureCommit>, DocumentError> {
        let Some(gesture) = &self.gesture else {
            return Ok(None);
        };
        gesture.prepare_commit_with_kind_registry(current, kind_registry)
    }

    pub(crate) fn clear_gesture(&mut self) {
        self.gesture = None;
    }

    pub(crate) fn cancel_gesture_transaction(
        &self,
        document: &CanvasDocument,
    ) -> Option<CanvasTransaction> {
        self.gesture
            .as_ref()
            .map(|gesture| gesture.cancel_transaction(document))
    }
}

impl Default for CanvasToolSession {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasToolSessionSnapshot {
    pub(crate) viewport: CanvasViewport,
    pub(crate) selection: CanvasSelection,
    pub(crate) state: ToolState,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasToolSessionEffect {
    SetSelection(CanvasSelection),
    ReplaceSelection(HitTarget),
    AddSelection(HitTarget),
    RemoveSelection(HitTarget),
    ToggleSelection(HitTarget),
    ClearSelection,
    SetState(ToolState),
    PanViewport(Point<Pixels>),
    SetViewport(CanvasViewport),
}
