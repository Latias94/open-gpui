//! Splitter component.

use crate::geometry::gpui_px_from_ui;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, Context, CursorStyle, DefiniteLength, DragMoveEvent, ElementId, Empty, Entity,
    IntoElement, ParentElement, Pixels, Point, Render, RenderOnce, Styled, Window, div, px,
    relative, rgb,
};
use open_gpui_ui_core::{Orientation, Sizable, Size};

const EPSILON: f32 = 0.000_1;

pub use open_gpui_ui_core::{
    SplitterHandleLayout, SplitterHandleState, SplitterHandleTransition,
    SplitterHandleTransitionKind, SplitterHitMap, SplitterHitTarget, SplitterJunctionHitRegion,
    SplitterLayoutScene, SplitterLayoutTransition, SplitterMetrics, SplitterPanelDescriptor,
    SplitterPanelLayout, SplitterPanelState, SplitterPanelTransition, SplitterPanelTransitionKind,
    SplitterResizeOutcome, SplitterResizeResult, SplitterState, SplitterTransitionIntent,
};

#[derive(Debug, Clone, Default)]
struct SplitterRuntime {
    panel_ids: Vec<String>,
    panel_fractions: Vec<f32>,
    drag_start: Option<SplitterDragStart>,
}

impl SplitterRuntime {
    fn sync(&mut self, state: &SplitterState) {
        let panel_ids = state
            .panels()
            .iter()
            .map(|panel| panel.id().to_owned())
            .collect::<Vec<_>>();

        if self.panel_ids == panel_ids && self.panel_fractions.len() == state.panels().len() {
            return;
        }

        self.panel_ids = panel_ids;
        self.panel_fractions = state
            .panels()
            .iter()
            .map(SplitterPanelState::fraction)
            .collect();
        self.drag_start = None;
    }
}

#[derive(Debug, Clone)]
struct SplitterDragStart {
    origin: Point<Pixels>,
    origin_fractions: Vec<f32>,
    axis_length: Pixels,
}

#[derive(Clone)]
struct SplitterDrag {
    group_id: String,
    handle_index: usize,
}

#[derive(Clone)]
struct SplitterDragPreview;

impl Render for SplitterDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        Empty
    }
}

/// A concrete GPUI splitter panel.
#[derive(IntoElement)]
pub struct SplitterPanel {
    descriptor: SplitterPanelDescriptor,
    content: AnyElement,
}

impl SplitterPanel {
    /// Creates a splitter panel.
    pub fn new(descriptor: SplitterPanelDescriptor, content: impl IntoElement) -> Self {
        Self {
            descriptor,
            content: content.into_any_element(),
        }
    }

    /// Returns the panel descriptor.
    pub fn descriptor(&self) -> SplitterPanelDescriptor {
        self.descriptor.clone()
    }
}

impl RenderOnce for SplitterPanel {
    fn render(self, _: &mut open_gpui::Window, _: &mut open_gpui::App) -> impl IntoElement {
        self.content
    }
}

/// A concrete GPUI splitter component.
#[derive(IntoElement)]
pub struct Splitter {
    id: ElementId,
    group_id: String,
    orientation: Orientation,
    size: Size,
    disabled: bool,
    panels: Vec<SplitterPanel>,
}

impl Splitter {
    /// Creates a splitter group.
    pub fn new(id: impl Into<String>) -> Self {
        let group_id = id.into();

        Self {
            id: group_id.clone().into(),
            group_id,
            orientation: Orientation::Horizontal,
            size: Size::Medium,
            disabled: false,
            panels: Vec::new(),
        }
    }

    /// Applies the splitter orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Uses a horizontal row of panels.
    pub fn horizontal(self) -> Self {
        self.orientation(Orientation::Horizontal)
    }

    /// Uses a vertical stack of panels.
    pub fn vertical(self) -> Self {
        self.orientation(Orientation::Vertical)
    }

    /// Disables resize handles.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Adds a panel to the splitter.
    pub fn panel(mut self, panel: SplitterPanel) -> Self {
        self.panels.push(panel);
        self
    }

    /// Returns the resolved splitter state.
    pub fn state(&self) -> SplitterState {
        SplitterState::resolve(
            self.group_id.clone(),
            self.orientation,
            self.size,
            self.disabled,
            self.panels.iter().map(SplitterPanel::descriptor),
        )
    }
}

impl Sizable for Splitter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Splitter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base_state = self.state();
        let runtime =
            window.use_keyed_state(self.id.clone(), cx, |_, _| SplitterRuntime::default());
        runtime.update(cx, |runtime, _| runtime.sync(&base_state));
        let runtime_snapshot = runtime.read(cx).clone();
        let state = base_state.with_panel_fractions(&runtime_snapshot.panel_fractions);
        let is_vertical = matches!(state.orientation(), Orientation::Vertical);
        let metrics = state.metrics();
        let handles = state.handles().to_vec();
        let panels = self.panels;
        let runtime_for_drag = runtime.clone();
        let drag_state = state.clone();

        div()
            .id(self.id)
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .overflow_hidden()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .when(is_vertical, |this| this.flex_col())
            .when(!is_vertical, |this| this.flex_row())
            .on_drag_move(move |event: &DragMoveEvent<SplitterDrag>, window, cx| {
                let drag = event.drag(cx).clone();
                if drag.group_id != drag_state.group_id() {
                    return;
                }

                runtime_for_drag.update(cx, |runtime, _| {
                    runtime.sync(&drag_state);

                    let axis_length = if is_vertical {
                        event.bounds.size.height
                    } else {
                        event.bounds.size.width
                    };
                    if axis_length.as_f32() <= EPSILON {
                        return;
                    }

                    if runtime.drag_start.is_none() {
                        runtime.drag_start = Some(SplitterDragStart {
                            origin: event.event.position,
                            origin_fractions: runtime.panel_fractions.clone(),
                            axis_length,
                        });
                    }

                    let Some(start) = runtime.drag_start.clone() else {
                        return;
                    };

                    if start.axis_length.as_f32() <= EPSILON {
                        return;
                    }

                    let delta_px = if is_vertical {
                        event.event.position.y - start.origin.y
                    } else {
                        event.event.position.x - start.origin.x
                    };
                    let delta_fraction = delta_px.as_f32() / start.axis_length.as_f32();
                    let origin_state = drag_state.with_panel_fractions(&start.origin_fractions);
                    let resized = origin_state.resized_by(drag.handle_index, delta_fraction);
                    runtime.panel_fractions = resized
                        .panels()
                        .iter()
                        .map(SplitterPanelState::fraction)
                        .collect();
                });
                window.refresh();
            })
            .children(
                panels
                    .into_iter()
                    .enumerate()
                    .flat_map(move |(index, panel)| {
                        let panel_state = state.panels()[index].clone();
                        let mut elements = Vec::with_capacity(2);
                        elements.push(render_panel(panel_state, panel, is_vertical));
                        if let Some(handle) = handles.get(index) {
                            elements.push(render_handle(
                                state.clone(),
                                handle.clone(),
                                runtime.clone(),
                                metrics,
                                is_vertical,
                            ));
                        }
                        elements
                    }),
            )
    }
}

fn render_panel(state: SplitterPanelState, panel: SplitterPanel, is_vertical: bool) -> AnyElement {
    let panel_selector = format!("splitter-panel:{}", state.id());
    div()
        .id(format!("splitter-panel:{}", state.id()))
        .debug_selector(move || panel_selector)
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .flex_grow(0.0)
        .flex_shrink(0.0)
        .flex_basis(DefiniteLength::from(relative(state.fraction())))
        .when(state.collapsed(), |this| this.opacity(0.0))
        .when(is_vertical, |this| this.w_full())
        .when(!is_vertical, |this| this.h_full())
        .child(panel)
        .into_any_element()
}

fn render_handle(
    splitter_state: SplitterState,
    state: SplitterHandleState,
    runtime: Entity<SplitterRuntime>,
    metrics: SplitterMetrics,
    is_vertical: bool,
) -> AnyElement {
    let cursor = if state.disabled() {
        CursorStyle::OperationNotAllowed
    } else if is_vertical {
        CursorStyle::ResizeRow
    } else {
        CursorStyle::ResizeColumn
    };

    div()
        .id(format!("splitter-handle:{}", state.index()))
        .debug_selector({
            let group_id = splitter_state.group_id().to_owned();
            let handle_index = state.index();
            move || format!("splitter:{group_id}:handle:{handle_index}")
        })
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor(cursor)
        .when(state.disabled(), |this| this.opacity(0.48))
        .when(!state.disabled(), |this| {
            let drag_runtime = runtime.clone();
            let drag_state = splitter_state.clone();
            let group_id = splitter_state.group_id().to_owned();
            let handle_index = state.index();

            this.on_drag(
                SplitterDrag {
                    group_id,
                    handle_index,
                },
                move |_, _, _, _, cx| {
                    cx.stop_propagation();
                    drag_runtime.update(cx, |runtime, _| {
                        runtime.sync(&drag_state);
                        runtime.drag_start = None;
                    });
                    cx.new(|_| SplitterDragPreview)
                },
            )
        })
        .when(is_vertical, |this| {
            this.w_full().h(gpui_px_from_ui(metrics.handle_hit_size()))
        })
        .when(!is_vertical, |this| {
            this.h_full().w(gpui_px_from_ui(metrics.handle_hit_size()))
        })
        .child(
            div()
                .rounded_sm()
                .bg(rgb(0xc8cdc2))
                .when(is_vertical, |this| {
                    this.w_full().h(gpui_px_from_ui(metrics.handle_thickness()))
                })
                .when(!is_vertical, |this| {
                    this.h_full().w(gpui_px_from_ui(metrics.handle_thickness()))
                }),
        )
        .into_any_element()
}
