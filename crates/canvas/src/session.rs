use crate::gesture::{CanvasGestureSession, CanvasPreparedGestureCommit};
use crate::tool::{CanvasSelection, CanvasTool, ToolState};
use crate::{
    CanvasDocument, CanvasKindRegistry, CanvasTransaction, CanvasViewport, DocumentError, HitTarget,
};
use open_gpui::{Pixels, Point};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasEditorSession {
    pub(crate) viewport: CanvasViewport,
    pub(crate) tool: CanvasTool,
    pub(crate) state: ToolState,
    pub(crate) selection: CanvasSelection,
    gesture: Option<CanvasGestureSession>,
}

impl CanvasEditorSession {
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

    pub(crate) fn snapshot(&self) -> CanvasEditorSessionSnapshot {
        CanvasEditorSessionSnapshot {
            viewport: self.viewport,
            selection: self.selection.clone(),
            state: self.state.clone(),
        }
    }

    pub(crate) fn retain_selection_for_document(&mut self, document: &CanvasDocument) {
        self.selection.retain_document(document);
    }

    pub(crate) fn set_selection(
        &mut self,
        mut selection: CanvasSelection,
        document: &CanvasDocument,
    ) {
        selection.retain_document(document);
        self.selection = selection;
    }

    pub(crate) fn apply_effect(&mut self, effect: CanvasSessionEffect, document: &CanvasDocument) {
        match effect {
            CanvasSessionEffect::SetSelection(selection) => {
                self.set_selection(selection, document);
            }
            CanvasSessionEffect::ReplaceSelection(target) => {
                self.replace_selection(target, document);
            }
            CanvasSessionEffect::AddSelection(target) => {
                self.add_selection(target, document);
            }
            CanvasSessionEffect::RemoveSelection(target) => {
                self.remove_selection(&target, document);
            }
            CanvasSessionEffect::ToggleSelection(target) => {
                self.toggle_selection(target, document);
            }
            CanvasSessionEffect::ClearSelection => {
                self.clear_selection();
            }
            CanvasSessionEffect::SetState(state) => {
                self.set_state(state);
            }
            CanvasSessionEffect::PanViewport(delta) => {
                self.pan_viewport(delta);
            }
            CanvasSessionEffect::SetViewport(viewport) => {
                self.set_viewport(viewport);
            }
        }
    }

    pub(crate) fn replace_selection(&mut self, target: HitTarget, document: &CanvasDocument) {
        self.selection.replace_with(target);
        self.retain_selection_for_document(document);
    }

    pub(crate) fn add_selection(&mut self, target: HitTarget, document: &CanvasDocument) {
        self.selection.insert_target(target);
        self.retain_selection_for_document(document);
    }

    pub(crate) fn remove_selection(&mut self, target: &HitTarget, document: &CanvasDocument) {
        self.selection.remove_target(target);
        self.retain_selection_for_document(document);
    }

    pub(crate) fn toggle_selection(&mut self, target: HitTarget, document: &CanvasDocument) {
        self.selection.toggle_target(target);
        self.retain_selection_for_document(document);
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

impl Default for CanvasEditorSession {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasEditorSessionSnapshot {
    pub(crate) viewport: CanvasViewport,
    pub(crate) selection: CanvasSelection,
    pub(crate) state: ToolState,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasSessionEffect {
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
