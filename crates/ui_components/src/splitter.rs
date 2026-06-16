//! Splitter component.

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, CursorStyle, DefiniteLength, ElementId, IntoElement, ParentElement, RenderOnce,
    Styled, div, px, relative, rgb,
};
use open_gpui_ui_core::{Orientation, Sizable, Size};

const EPSILON: f32 = 0.000_1;

/// Panel constraints for a [`Splitter`].
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelDescriptor {
    id: String,
    fraction: f32,
    min_fraction: f32,
    max_fraction: f32,
    collapsible: bool,
    collapsed: bool,
    collapsed_fraction: f32,
}

impl SplitterPanelDescriptor {
    /// Creates a panel descriptor with an initial fraction.
    pub fn new(id: impl Into<String>, fraction: f32) -> Self {
        Self {
            id: id.into(),
            fraction,
            min_fraction: 0.1,
            max_fraction: 1.0,
            collapsible: false,
            collapsed: false,
            collapsed_fraction: 0.0,
        }
    }

    /// Applies the minimum panel fraction.
    pub fn min_fraction(mut self, min_fraction: f32) -> Self {
        self.min_fraction = min_fraction;
        self
    }

    /// Applies the maximum panel fraction.
    pub fn max_fraction(mut self, max_fraction: f32) -> Self {
        self.max_fraction = max_fraction;
        self
    }

    /// Marks the panel as collapsible.
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Seeds whether the panel starts collapsed.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Applies the fraction used while collapsed.
    pub fn collapsed_fraction(mut self, collapsed_fraction: f32) -> Self {
        self.collapsed_fraction = collapsed_fraction;
        self
    }
}

/// Resolved splitter metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitterMetrics {
    handle_thickness: open_gpui::Pixels,
    handle_hit_size: open_gpui::Pixels,
    radius: open_gpui::Pixels,
}

impl SplitterMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            handle_thickness: match size {
                Size::XSmall | Size::Small => px(1.0),
                Size::Medium | Size::Large => px(2.0),
            },
            handle_hit_size: match size {
                Size::XSmall => px(8.0),
                Size::Small => px(10.0),
                Size::Medium => px(12.0),
                Size::Large => px(14.0),
            },
            radius: size.control_radius(),
        }
    }

    /// Returns the painted handle thickness.
    pub const fn handle_thickness(self) -> open_gpui::Pixels {
        self.handle_thickness
    }

    /// Returns the pointer hit size reserved for the handle.
    pub const fn handle_hit_size(self) -> open_gpui::Pixels {
        self.handle_hit_size
    }

    /// Returns the splitter corner radius.
    pub const fn radius(self) -> open_gpui::Pixels {
        self.radius
    }
}

/// Resolved state for one splitter panel.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelState {
    id: String,
    fraction: f32,
    min_fraction: f32,
    max_fraction: f32,
    collapsible: bool,
    collapsed: bool,
    collapsed_fraction: f32,
}

impl SplitterPanelState {
    fn from_descriptor(descriptor: SplitterPanelDescriptor) -> Self {
        let min_fraction = sanitize_fraction(descriptor.min_fraction).min(1.0);
        let max_fraction = sanitize_fraction(descriptor.max_fraction)
            .max(min_fraction)
            .min(1.0);
        let collapsible = descriptor.collapsible;
        let collapsed = collapsible && descriptor.collapsed;
        let collapsed_fraction = sanitize_fraction(descriptor.collapsed_fraction)
            .min(max_fraction)
            .max(0.0);
        let fraction = if collapsed {
            collapsed_fraction
        } else {
            sanitize_fraction(descriptor.fraction).clamp(min_fraction, max_fraction)
        };

        Self {
            id: descriptor.id,
            fraction,
            min_fraction,
            max_fraction,
            collapsible,
            collapsed,
            collapsed_fraction,
        }
    }

    /// Returns the stable panel id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the resolved panel fraction.
    pub const fn fraction(&self) -> f32 {
        self.fraction
    }

    /// Returns the minimum panel fraction.
    pub const fn min_fraction(&self) -> f32 {
        self.min_fraction
    }

    /// Returns the maximum panel fraction.
    pub const fn max_fraction(&self) -> f32 {
        self.max_fraction
    }

    /// Returns whether the panel may collapse.
    pub const fn collapsible(&self) -> bool {
        self.collapsible
    }

    /// Returns whether the panel is currently collapsed.
    pub const fn collapsed(&self) -> bool {
        self.collapsed
    }

    /// Returns the collapsed panel fraction.
    pub const fn collapsed_fraction(&self) -> f32 {
        self.collapsed_fraction
    }
}

/// Resolved state for one splitter handle.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterHandleState {
    index: usize,
    before_id: String,
    after_id: String,
    disabled: bool,
}

impl SplitterHandleState {
    /// Returns the zero-based handle index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the panel id before the handle.
    pub fn before_id(&self) -> &str {
        &self.before_id
    }

    /// Returns the panel id after the handle.
    pub fn after_id(&self) -> &str {
        &self.after_id
    }

    /// Returns whether resize interaction is disabled for this handle.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }
}

/// Resolved splitter state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterState {
    group_id: String,
    orientation: Orientation,
    size: Size,
    disabled: bool,
    panels: Vec<SplitterPanelState>,
    handles: Vec<SplitterHandleState>,
    metrics: SplitterMetrics,
}

impl SplitterState {
    /// Resolves a splitter state from panel descriptors.
    pub fn resolve(
        group_id: impl Into<String>,
        orientation: Orientation,
        size: Size,
        disabled: bool,
        panels: impl IntoIterator<Item = SplitterPanelDescriptor>,
    ) -> Self {
        let mut panels = panels
            .into_iter()
            .map(SplitterPanelState::from_descriptor)
            .collect::<Vec<_>>();
        normalize_panel_fractions(&mut panels);
        let handles = resolve_handles(&panels, disabled);

        Self {
            group_id: group_id.into(),
            orientation,
            size,
            disabled,
            panels,
            handles,
            metrics: SplitterMetrics::from_size(size),
        }
    }

    /// Returns the stable splitter group id.
    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    /// Returns the splitter orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether all resize handles are disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns resolved panel states.
    pub fn panels(&self) -> &[SplitterPanelState] {
        &self.panels
    }

    /// Returns resolved handle states.
    pub fn handles(&self) -> &[SplitterHandleState] {
        &self.handles
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> SplitterMetrics {
        self.metrics
    }

    /// Returns a new state after applying a fraction delta to a handle.
    pub fn resized_by(&self, handle_index: usize, delta_fraction: f32) -> Self {
        if !delta_fraction.is_finite()
            || delta_fraction.abs() <= EPSILON
            || handle_index + 1 >= self.panels.len()
        {
            return self.clone();
        }

        let mut next = self.clone();
        let before = handle_index;
        let after = handle_index + 1;
        let delta = if delta_fraction > 0.0 {
            let grow_room = next.panels[before].max_fraction - next.panels[before].fraction;
            let shrink_room = next.panels[after].fraction - next.panels[after].min_fraction;
            delta_fraction
                .min(grow_room.max(0.0))
                .min(shrink_room.max(0.0))
        } else {
            let shrink_room = next.panels[before].fraction - next.panels[before].min_fraction;
            let grow_room = next.panels[after].max_fraction - next.panels[after].fraction;
            -((-delta_fraction)
                .min(shrink_room.max(0.0))
                .min(grow_room.max(0.0)))
        };

        if delta.abs() <= EPSILON {
            return self.clone();
        }

        next.panels[before].fraction += delta;
        next.panels[after].fraction -= delta;
        normalize_panel_fractions(&mut next.panels);
        next.handles = resolve_handles(&next.panels, next.disabled);
        next
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
    fn render(self, _: &mut open_gpui::Window, _: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let is_vertical = matches!(state.orientation(), Orientation::Vertical);
        let metrics = state.metrics();
        let handles = state.handles().to_vec();
        let panels = self.panels;

        div()
            .id(self.id)
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .overflow_hidden()
            .rounded(metrics.radius())
            .when(is_vertical, |this| this.flex_col())
            .when(!is_vertical, |this| this.flex_row())
            .children(
                panels
                    .into_iter()
                    .enumerate()
                    .flat_map(move |(index, panel)| {
                        let panel_state = state.panels()[index].clone();
                        let mut elements = Vec::with_capacity(2);
                        elements.push(render_panel(panel_state, panel, is_vertical));
                        if let Some(handle) = handles.get(index) {
                            elements.push(render_handle(handle.clone(), metrics, is_vertical));
                        }
                        elements
                    }),
            )
    }
}

fn render_panel(state: SplitterPanelState, panel: SplitterPanel, is_vertical: bool) -> AnyElement {
    div()
        .id(format!("splitter-panel:{}", state.id()))
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
    state: SplitterHandleState,
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
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor(cursor)
        .when(state.disabled(), |this| this.opacity(0.48))
        .when(is_vertical, |this| {
            this.w_full().h(metrics.handle_hit_size())
        })
        .when(!is_vertical, |this| {
            this.h_full().w(metrics.handle_hit_size())
        })
        .child(
            div()
                .rounded_sm()
                .bg(rgb(0xc8cdc2))
                .when(is_vertical, |this| {
                    this.w_full().h(metrics.handle_thickness())
                })
                .when(!is_vertical, |this| {
                    this.h_full().w(metrics.handle_thickness())
                }),
        )
        .into_any_element()
}

fn sanitize_fraction(fraction: f32) -> f32 {
    if fraction.is_finite() {
        fraction.max(0.0)
    } else {
        0.0
    }
}

fn normalize_panel_fractions(panels: &mut [SplitterPanelState]) {
    if panels.is_empty() {
        return;
    }

    fit_panel_sum(panels, 1.0);
    let sum: f32 = panels.iter().map(|panel| panel.fraction).sum();
    if sum.is_finite() && sum > EPSILON {
        let diff = 1.0 - sum;
        if diff.abs() > EPSILON
            && let Some(panel) = panels.iter_mut().rev().find(|panel| !panel.collapsed)
        {
            panel.fraction = (panel.fraction + diff).clamp(panel.min_fraction, panel.max_fraction);
        }
    }
}

fn fit_panel_sum(panels: &mut [SplitterPanelState], target: f32) {
    for _ in 0..8 {
        let sum: f32 = panels.iter().map(|panel| panel.fraction).sum();
        let diff = target - sum;
        if !diff.is_finite() || diff.abs() <= EPSILON {
            return;
        }

        if diff > 0.0 {
            let room: f32 = panels
                .iter()
                .filter(|panel| !panel.collapsed)
                .map(|panel| (panel.max_fraction - panel.fraction).max(0.0))
                .sum();
            if room <= EPSILON {
                return;
            }

            for panel in panels.iter_mut().filter(|panel| !panel.collapsed) {
                let panel_room = (panel.max_fraction - panel.fraction).max(0.0);
                let take = diff * (panel_room / room);
                panel.fraction = (panel.fraction + take).min(panel.max_fraction);
            }
        } else {
            let room: f32 = panels
                .iter()
                .filter(|panel| !panel.collapsed)
                .map(|panel| (panel.fraction - panel.min_fraction).max(0.0))
                .sum();
            if room <= EPSILON {
                return;
            }

            for panel in panels.iter_mut().filter(|panel| !panel.collapsed) {
                let panel_room = (panel.fraction - panel.min_fraction).max(0.0);
                let take = (-diff) * (panel_room / room);
                panel.fraction = (panel.fraction - take).max(panel.min_fraction);
            }
        }
    }
}

fn resolve_handles(panels: &[SplitterPanelState], disabled: bool) -> Vec<SplitterHandleState> {
    panels
        .windows(2)
        .enumerate()
        .map(|(index, pair)| SplitterHandleState {
            index,
            before_id: pair[0].id.clone(),
            after_id: pair[1].id.clone(),
            disabled: disabled || (pair[0].collapsed && pair[1].collapsed),
        })
        .collect()
}
