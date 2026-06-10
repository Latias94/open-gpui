use crate::tool::ToolState;
use crate::{
    CanvasDefaultEdgeRouter, CanvasDocument, CanvasEdgeRouter, CanvasEditor, CanvasKindRegistry,
    CanvasRuntime, CanvasSelection, CanvasViewport,
};
use open_gpui::{Hsla, Pixels, TextAlign, px, rgb};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CanvasPaintModel {
    pub(super) document: Arc<CanvasDocument>,
    pub(super) runtime: Arc<CanvasRuntime>,
    pub(super) kind_registry: Arc<CanvasKindRegistry>,
    pub(super) viewport: CanvasViewport,
    pub(super) interaction: CanvasPaintInteraction,
}

impl CanvasPaintModel {
    pub fn new(document: CanvasDocument, viewport: CanvasViewport) -> Self {
        Self::new_with_router(document, viewport, &CanvasDefaultEdgeRouter)
    }

    pub fn new_with_kind_registry(
        document: CanvasDocument,
        viewport: CanvasViewport,
        kind_registry: CanvasKindRegistry,
    ) -> Self {
        Self::new_with_router_and_kind_registry(
            document,
            viewport,
            &CanvasDefaultEdgeRouter,
            kind_registry,
        )
    }

    pub fn new_with_router<R>(
        document: CanvasDocument,
        viewport: CanvasViewport,
        router: &R,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::new_with_router_and_kind_registry(
            document,
            viewport,
            router,
            CanvasKindRegistry::open(),
        )
    }

    pub fn new_with_router_and_kind_registry<R>(
        document: CanvasDocument,
        viewport: CanvasViewport,
        router: &R,
        kind_registry: CanvasKindRegistry,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        let runtime =
            CanvasRuntime::rebuild_with_router_and_kind_registry(&document, router, &kind_registry);
        Self {
            document: Arc::new(document),
            runtime: Arc::new(runtime),
            kind_registry: Arc::new(kind_registry),
            viewport,
            interaction: CanvasPaintInteraction::default(),
        }
    }

    pub fn document(&self) -> &CanvasDocument {
        self.document.as_ref()
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        self.runtime.as_ref()
    }

    pub fn kind_registry(&self) -> &CanvasKindRegistry {
        self.kind_registry.as_ref()
    }

    pub fn viewport(&self) -> CanvasViewport {
        self.viewport
    }

    pub fn interaction(&self) -> &CanvasPaintInteraction {
        &self.interaction
    }

    pub fn with_interaction(mut self, interaction: CanvasPaintInteraction) -> Self {
        self.interaction = interaction;
        self
    }

    pub fn with_selection(mut self, selection: CanvasSelection) -> Self {
        self.interaction = self.interaction.with_selection(selection);
        self
    }
}

impl From<&CanvasEditor> for CanvasPaintModel {
    fn from(editor: &CanvasEditor) -> Self {
        Self {
            document: editor.document_snapshot(),
            runtime: editor.runtime_snapshot(),
            kind_registry: editor.kind_registry_snapshot(),
            viewport: editor.viewport(),
            interaction: CanvasPaintInteraction::new(editor.selection().clone())
                .with_internal_tool_state(editor.state().clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintInteraction {
    selection: CanvasSelection,
    state: ToolState,
}

impl CanvasPaintInteraction {
    pub fn new(selection: CanvasSelection) -> Self {
        Self {
            selection,
            state: ToolState::Idle,
        }
    }

    pub fn selection(&self) -> &CanvasSelection {
        &self.selection
    }

    pub(crate) fn tool_state(&self) -> &ToolState {
        &self.state
    }

    pub fn with_selection(mut self, selection: CanvasSelection) -> Self {
        self.selection = selection;
        self
    }

    pub(crate) fn with_internal_tool_state(mut self, state: ToolState) -> Self {
        self.state = state;
        self
    }
}

impl Default for CanvasPaintInteraction {
    fn default() -> Self {
        Self {
            selection: CanvasSelection::default(),
            state: ToolState::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPaintOptions {
    pub include_hidden: bool,
    pub include_handles: bool,
    pub include_interaction_feedback: bool,
    pub cull_margin: Pixels,
}

impl Default for CanvasPaintOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_handles: false,
            include_interaction_feedback: true,
            cull_margin: Pixels::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPaintTheme {
    pub background: Option<Hsla>,
    pub node_fill: Hsla,
    pub node_stroke: Hsla,
    pub node_stroke_width: Pixels,
    pub node_corner_radius: Pixels,
    pub shape_fill: Hsla,
    pub shape_stroke: Hsla,
    pub shape_stroke_width: Pixels,
    pub edge_stroke: Hsla,
    pub edge_stroke_width: Pixels,
    pub handle_fill: Hsla,
    pub handle_stroke: Hsla,
    pub handle_stroke_width: Pixels,
    pub handle_corner_radius: Pixels,
    pub selection_fill: Hsla,
    pub selection_stroke: Hsla,
    pub selection_stroke_width: Pixels,
    pub selection_corner_radius: Pixels,
    pub selection_bounds_fill: Hsla,
    pub selection_bounds_stroke: Hsla,
    pub selection_bounds_stroke_width: Pixels,
    pub connection_preview_stroke: Hsla,
    pub connection_preview_stroke_width: Pixels,
    pub snap_guide_stroke: Hsla,
    pub snap_guide_stroke_width: Pixels,
    pub label_color: Hsla,
    pub label_font_size: Pixels,
    pub label_line_height: Pixels,
    pub label_line_clamp: Option<usize>,
    pub label_text_align: TextAlign,
}

impl Default for CanvasPaintTheme {
    fn default() -> Self {
        Self {
            background: None,
            node_fill: Hsla::from(rgb(0xffffff)),
            node_stroke: Hsla::from(rgb(0xd0d7de)),
            node_stroke_width: px(1.0),
            node_corner_radius: px(6.0),
            shape_fill: Hsla::from(rgb(0xf6f8fa)),
            shape_stroke: Hsla::from(rgb(0xd0d7de)),
            shape_stroke_width: px(1.0),
            edge_stroke: Hsla::from(rgb(0x57606a)),
            edge_stroke_width: px(2.0),
            handle_fill: Hsla::from(rgb(0x0969da)),
            handle_stroke: Hsla::from(rgb(0xffffff)),
            handle_stroke_width: px(1.0),
            handle_corner_radius: px(6.0),
            selection_fill: Hsla::from(rgb(0x0969da)).alpha(0.08),
            selection_stroke: Hsla::from(rgb(0x0969da)),
            selection_stroke_width: px(2.0),
            selection_corner_radius: px(7.0),
            selection_bounds_fill: Hsla::from(rgb(0x0969da)).alpha(0.08),
            selection_bounds_stroke: Hsla::from(rgb(0x0969da)).alpha(0.7),
            selection_bounds_stroke_width: px(1.0),
            connection_preview_stroke: Hsla::from(rgb(0x0969da)).alpha(0.7),
            connection_preview_stroke_width: px(2.0),
            snap_guide_stroke: Hsla::from(rgb(0xbf8700)).alpha(0.9),
            snap_guide_stroke_width: px(1.0),
            label_color: Hsla::from(rgb(0x24292f)),
            label_font_size: px(14.0),
            label_line_height: px(18.0),
            label_line_clamp: Some(3),
            label_text_align: TextAlign::Center,
        }
    }
}
