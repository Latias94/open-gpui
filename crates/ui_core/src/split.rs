//! Renderer-neutral split layout primitives.

use crate::{Orientation, Size, UiPx, UiRect, ui_point, ui_px, ui_rect, ui_size};
use open_gpui_motion::{
    MotionEdge, MotionPreference, MotionProjection, MotionProjectionClip, MotionProjectionError,
    MotionRect, advanced::MotionSpec, motion_point, motion_px, motion_rect, motion_size,
    reveal_rect_from_edge,
};
use std::collections::HashMap;

const EPSILON: f32 = 0.000_1;

/// Convenient imports for renderer-neutral split layout work.
pub mod prelude {
    pub use super::{
        SplitTreeChild, SplitTreeNode, SplitterHandleLayout, SplitterHandlePlacement,
        SplitterHandleState, SplitterHitMap, SplitterHitTarget, SplitterJunctionHitRegion,
        SplitterLayoutScene, SplitterMetrics, SplitterPanelDescriptor, SplitterPanelLayout,
        SplitterPanelState, SplitterResizeOutcome, SplitterResizeResult, SplitterState,
        normalize_split_fractions, resize_split_fractions_by_pixels, resolve_split_fractions,
        resolve_split_fractions_with_fill_child,
    };
}

/// Panel constraints for a split layout.
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

    /// Returns the stable panel id.
    pub fn id(&self) -> &str {
        &self.id
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

/// Resolved split metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitterMetrics {
    handle_thickness: UiPx,
    handle_hit_size: UiPx,
    radius: UiPx,
    handle_placement: SplitterHandlePlacement,
}

impl SplitterMetrics {
    /// Creates explicit metrics for adapters that do not use size tokens directly.
    pub const fn new(handle_thickness: UiPx, handle_hit_size: UiPx, radius: UiPx) -> Self {
        Self {
            handle_thickness,
            handle_hit_size,
            radius,
            handle_placement: SplitterHandlePlacement::BetweenPanels,
        }
    }

    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            handle_thickness: match size {
                Size::XSmall | Size::Small => ui_px(1.0),
                Size::Medium | Size::Large => ui_px(2.0),
            },
            handle_hit_size: match size {
                Size::XSmall => ui_px(8.0),
                Size::Small => ui_px(10.0),
                Size::Medium => ui_px(12.0),
                Size::Large => ui_px(14.0),
            },
            radius: size.control_radius(),
            handle_placement: SplitterHandlePlacement::BetweenPanels,
        }
    }

    /// Returns metrics with a different handle placement strategy.
    pub const fn with_handle_placement(mut self, placement: SplitterHandlePlacement) -> Self {
        self.handle_placement = placement;
        self
    }

    /// Returns the painted handle thickness.
    pub const fn handle_thickness(self) -> UiPx {
        self.handle_thickness
    }

    /// Returns the pointer hit size reserved for the handle.
    pub const fn handle_hit_size(self) -> UiPx {
        self.handle_hit_size
    }

    /// Returns the split corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns whether handle hit bounds reserve layout space or overlay panel boundaries.
    pub const fn handle_placement(self) -> SplitterHandlePlacement {
        self.handle_placement
    }

    /// Returns the axis span shared by panels after reserving between-panel handle hit regions.
    pub fn panel_axis_extent(self, outer_axis_extent: UiPx, handle_count: usize) -> UiPx {
        let outer_axis_extent = outer_axis_extent.as_f32();
        if !outer_axis_extent.is_finite() {
            return ui_px(0.0);
        }

        let reserved_handle_extent = match self.handle_placement {
            SplitterHandlePlacement::BetweenPanels => {
                self.handle_hit_size.as_f32() * handle_count as f32
            }
            SplitterHandlePlacement::OverlayBoundary => 0.0,
        };
        ui_px((outer_axis_extent - reserved_handle_extent).max(0.0))
    }
}

/// Strategy used when resolving handle hit bounds in a split layout scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterHandlePlacement {
    /// Handle hit bounds occupy space between panels.
    BetweenPanels,
    /// Handle hit bounds are centered over the adjacent panel boundary.
    OverlayBoundary,
}

/// Resolved state for one split panel.
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
        let max_fraction = sanitize_max_fraction(descriptor.max_fraction)
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

/// Resolved state for one split handle.
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

/// Outcome for a requested split resize operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterResizeOutcome {
    /// The requested delta was applied without clamping.
    Applied,
    /// The resize applied after clamping to panel constraints.
    Clamped,
    /// The resize did not change state.
    Rejected,
}

/// Result for a requested split resize operation.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterResizeResult {
    state: SplitterState,
    outcome: SplitterResizeOutcome,
}

impl SplitterResizeResult {
    /// Returns the resulting split state.
    pub fn state(&self) -> &SplitterState {
        &self.state
    }

    /// Consumes the result and returns the resulting split state.
    pub fn into_state(self) -> SplitterState {
        self.state
    }

    /// Returns the resize outcome.
    pub const fn outcome(&self) -> SplitterResizeOutcome {
        self.outcome
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
        self.resize_by(handle_index, delta_fraction).into_state()
    }

    /// Applies a fraction delta to a handle and returns the operation outcome.
    pub fn resize_by(&self, handle_index: usize, delta_fraction: f32) -> SplitterResizeResult {
        if self
            .handles
            .get(handle_index)
            .is_none_or(|handle| handle.disabled)
            || !delta_fraction.is_finite()
            || delta_fraction.abs() <= EPSILON
            || handle_index + 1 >= self.panels.len()
        {
            return SplitterResizeResult {
                state: self.clone(),
                outcome: SplitterResizeOutcome::Rejected,
            };
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
            return SplitterResizeResult {
                state: self.clone(),
                outcome: SplitterResizeOutcome::Rejected,
            };
        }

        let before_target = next.panels[before].fraction + delta;
        let after_target = next.panels[after].fraction - delta;
        let (before_panels, after_panels) = next.panels.split_at_mut(after);
        if !apply_resize_fraction(&mut before_panels[before], before_target)
            || !apply_resize_fraction(&mut after_panels[0], after_target)
        {
            return SplitterResizeResult {
                state: self.clone(),
                outcome: SplitterResizeOutcome::Rejected,
            };
        }

        normalize_panel_fractions(&mut next.panels);
        next.handles = resolve_handles(&next.panels, next.disabled);
        let outcome = if (delta - delta_fraction).abs() > EPSILON {
            SplitterResizeOutcome::Clamped
        } else {
            SplitterResizeOutcome::Applied
        };
        SplitterResizeResult {
            state: next,
            outcome,
        }
    }

    /// Applies a pixel delta to a handle for a split with the given axis extent.
    pub fn resize_by_pixels(
        &self,
        handle_index: usize,
        axis_extent: UiPx,
        delta: UiPx,
    ) -> SplitterResizeResult {
        let axis_extent = axis_extent.as_f32();
        if !axis_extent.is_finite() || axis_extent <= EPSILON {
            return SplitterResizeResult {
                state: self.clone(),
                outcome: SplitterResizeOutcome::Rejected,
            };
        }

        self.resize_by(handle_index, delta.as_f32() / axis_extent)
    }

    /// Returns a new state with panel fractions overridden by runtime layout state.
    pub fn with_panel_fractions(&self, fractions: &[f32]) -> Self {
        if fractions.len() != self.panels.len() {
            return self.clone();
        }

        let mut next = self.clone();
        for (panel, fraction) in next.panels.iter_mut().zip(fractions.iter().copied()) {
            if panel.collapsed {
                let fraction = sanitize_fraction(fraction);
                if fraction + EPSILON < collapsed_restore_threshold(panel) {
                    panel.fraction = panel.collapsed_fraction;
                    continue;
                }

                panel.collapsed = false;
            }

            panel.fraction =
                sanitize_fraction(fraction).clamp(panel.min_fraction, panel.max_fraction);
        }
        normalize_panel_fractions(&mut next.panels);
        next.handles = resolve_handles(&next.panels, next.disabled);
        next
    }
}

/// Resolves a normalized list of split fractions for an ordered child list.
pub fn resolve_split_fractions(child_count: usize, fractions: &[f32]) -> Vec<f32> {
    let mut shares: Vec<f32> = (0..child_count)
        .map(|index| fractions.get(index).copied().unwrap_or(1.0))
        .collect();
    normalize_split_fractions(&mut shares);
    shares
}

/// Resolves split fractions while one child receives the remaining unassigned share.
pub fn resolve_split_fractions_with_fill_child(
    child_count: usize,
    fractions: &[f32],
    fill_child_index: Option<usize>,
) -> Vec<f32> {
    let Some(fill_child_index) = fill_child_index else {
        return resolve_split_fractions(child_count, fractions);
    };
    if child_count == 0 || fill_child_index >= child_count {
        return resolve_split_fractions(child_count, fractions);
    }
    if child_count == 1 {
        return vec![1.0];
    }

    let mut shares = (0..child_count)
        .map(|index| {
            if index == fill_child_index {
                0.0
            } else {
                sanitize_fraction(fractions.get(index).copied().unwrap_or(0.0))
            }
        })
        .collect::<Vec<_>>();

    let non_fill_sum: f32 = shares.iter().sum();
    if non_fill_sum > 1.0 {
        for (index, share) in shares.iter_mut().enumerate() {
            if index != fill_child_index {
                *share /= non_fill_sum;
            }
        }
        shares[fill_child_index] = 0.0;
    } else {
        shares[fill_child_index] = 1.0 - non_fill_sum;
    }

    shares
}

/// Normalizes split fractions in place, repairing invalid values.
pub fn normalize_split_fractions(fractions: &mut Vec<f32>) {
    for fraction in fractions.iter_mut() {
        if !fraction.is_finite() || *fraction < 0.0 {
            *fraction = 0.0;
        }
    }

    let sum: f32 = fractions.iter().sum();
    if !sum.is_finite() || sum <= EPSILON {
        let len = fractions.len().max(1);
        *fractions = vec![1.0 / len as f32; len];
        return;
    }

    for fraction in fractions.iter_mut() {
        *fraction /= sum;
    }

    if !fractions.is_empty() {
        let rest: f32 = fractions
            .iter()
            .take(fractions.len().saturating_sub(1))
            .sum();
        let last = fractions.len().saturating_sub(1);
        fractions[last] = (1.0 - rest).clamp(0.0, 1.0);
    }
}

/// Resizes adjacent split fractions by a pixel delta along the split axis.
pub fn resize_split_fractions_by_pixels(
    fractions: &[f32],
    handle_index: usize,
    axis_extent: UiPx,
    delta: UiPx,
    min_panel_extent: UiPx,
) -> Option<Vec<f32>> {
    let child_count = fractions.len();
    if child_count < 2 || handle_index + 1 >= child_count {
        return None;
    }

    let extent = axis_extent.as_f32();
    if !extent.is_finite() || extent <= EPSILON {
        return None;
    }

    let shares = resolve_split_fractions(child_count, fractions);
    let pair_total = shares[handle_index] + shares[handle_index + 1];
    if !pair_total.is_finite() || pair_total <= EPSILON {
        return None;
    }

    let min_fraction = (min_panel_extent.as_f32().max(0.0) / extent).clamp(0.0, pair_total / 2.0);
    let max_pair_fraction = pair_total - min_fraction;
    let panels = shares.iter().copied().enumerate().map(|(index, share)| {
        let descriptor = SplitterPanelDescriptor::new(format!("panel-{index}"), share);
        if index == handle_index || index == handle_index + 1 {
            descriptor
                .min_fraction(min_fraction)
                .max_fraction(max_pair_fraction)
        } else {
            descriptor.min_fraction(0.0)
        }
    });
    let state = SplitterState::resolve(
        "split-fraction-resize",
        Orientation::Horizontal,
        Size::Medium,
        false,
        panels,
    );
    let resized = state.resize_by_pixels(handle_index, axis_extent, delta);
    Some(
        resized
            .state()
            .panels()
            .iter()
            .map(SplitterPanelState::fraction)
            .collect(),
    )
}

/// Resolved flat layout for an ordered splitter or split tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterLayoutScene {
    group_id: String,
    orientation: Orientation,
    bounds: UiRect,
    panels: Vec<SplitterPanelLayout>,
    handles: Vec<SplitterHandleLayout>,
    junctions: Vec<SplitterJunctionHitRegion>,
}

impl SplitterLayoutScene {
    /// Resolves a flat layout scene from an ordered splitter state.
    pub fn from_state(state: &SplitterState, bounds: UiRect) -> Self {
        Self::from_state_with_metrics(state, bounds, state.metrics())
    }

    /// Resolves a flat layout scene from an ordered splitter state using adapter metrics.
    pub fn from_state_with_metrics(
        state: &SplitterState,
        bounds: UiRect,
        metrics: SplitterMetrics,
    ) -> Self {
        let mut scene = Self {
            group_id: state.group_id().to_owned(),
            orientation: state.orientation(),
            bounds,
            panels: Vec::new(),
            handles: Vec::new(),
            junctions: Vec::new(),
        };
        scene.push_state_layout_with_metrics(state, bounds, true, metrics);
        scene.resolve_junctions();
        scene
    }

    /// Resolves a flat layout scene from a nested split tree.
    pub fn from_tree(root: &SplitTreeNode, bounds: UiRect, size: Size) -> Self {
        let mut scene = Self {
            group_id: root.id().to_owned(),
            orientation: root.orientation().unwrap_or(Orientation::Horizontal),
            bounds,
            panels: Vec::new(),
            handles: Vec::new(),
            junctions: Vec::new(),
        };
        scene.push_tree_node(root, bounds, size, 1.0);
        scene.resolve_junctions();
        scene
    }

    /// Returns the scene group id.
    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    /// Returns the root orientation used by this scene.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the layout bounds.
    pub const fn bounds(&self) -> UiRect {
        self.bounds
    }

    /// Returns resolved leaf panel layouts.
    pub fn panels(&self) -> &[SplitterPanelLayout] {
        &self.panels
    }

    /// Returns resolved handle layouts.
    pub fn handles(&self) -> &[SplitterHandleLayout] {
        &self.handles
    }

    /// Returns resolved junction hit regions.
    pub fn junctions(&self) -> &[SplitterJunctionHitRegion] {
        &self.junctions
    }

    fn push_tree_node(&mut self, node: &SplitTreeNode, bounds: UiRect, size: Size, fraction: f32) {
        match node {
            SplitTreeNode::Leaf { id } => {
                self.panels.push(SplitterPanelLayout {
                    id: id.clone(),
                    index: self.panels.len(),
                    bounds,
                    fraction,
                    collapsed: false,
                });
            }
            SplitTreeNode::Split {
                id,
                orientation,
                children,
            } => {
                let state = SplitterState::resolve(
                    id.clone(),
                    *orientation,
                    size,
                    false,
                    children.iter().map(|child| {
                        SplitterPanelDescriptor::new(child.node.id().to_owned(), child.fraction)
                            .min_fraction(child.min_fraction)
                            .max_fraction(child.max_fraction)
                    }),
                );
                let fractions = state
                    .panels()
                    .iter()
                    .map(SplitterPanelState::fraction)
                    .collect::<Vec<_>>();
                let ranges = self.push_state_layout(&state, bounds, false);
                for ((child, child_bounds), fraction) in children.iter().zip(ranges).zip(fractions)
                {
                    self.push_tree_node(&child.node, child_bounds, size, fraction);
                }
            }
        }
    }

    fn push_state_layout(
        &mut self,
        state: &SplitterState,
        bounds: UiRect,
        record_panels: bool,
    ) -> Vec<UiRect> {
        self.push_state_layout_with_metrics(state, bounds, record_panels, state.metrics())
    }

    fn push_state_layout_with_metrics(
        &mut self,
        state: &SplitterState,
        bounds: UiRect,
        record_panels: bool,
        metrics: SplitterMetrics,
    ) -> Vec<UiRect> {
        let is_vertical = matches!(state.orientation(), Orientation::Vertical);
        let handle_count = state.handles().len();
        let axis_length = if is_vertical {
            bounds.size.height.as_f32()
        } else {
            bounds.size.width.as_f32()
        };
        let cross_length = if is_vertical {
            bounds.size.width.as_f32()
        } else {
            bounds.size.height.as_f32()
        };
        if axis_length <= EPSILON || cross_length <= EPSILON || state.panels().is_empty() {
            return Vec::new();
        }

        let handle_hit = metrics.handle_hit_size().as_f32();
        let handle_thickness = metrics.handle_thickness().as_f32();
        let reserves_handle_space = matches!(
            metrics.handle_placement(),
            SplitterHandlePlacement::BetweenPanels
        );
        let panel_axis = metrics
            .panel_axis_extent(ui_px(axis_length), handle_count)
            .as_f32();
        let mut cursor = if is_vertical {
            bounds.origin.y.as_f32()
        } else {
            bounds.origin.x.as_f32()
        };
        let mut child_bounds = Vec::with_capacity(state.panels().len());

        for (index, panel) in state.panels().iter().enumerate() {
            let length = panel_axis * panel.fraction();
            let panel_bounds = if is_vertical {
                ui_rect(
                    ui_point(bounds.origin.x, ui_px(cursor)),
                    ui_size(bounds.size.width, ui_px(length)),
                )
            } else {
                ui_rect(
                    ui_point(ui_px(cursor), bounds.origin.y),
                    ui_size(ui_px(length), bounds.size.height),
                )
            };
            child_bounds.push(panel_bounds);
            if record_panels {
                self.panels.push(SplitterPanelLayout {
                    id: panel.id().to_owned(),
                    index: self.panels.len(),
                    bounds: panel_bounds,
                    fraction: panel.fraction(),
                    collapsed: panel.collapsed(),
                });
            }
            cursor += length;

            if let Some(handle) = state.handles().get(index) {
                let handle_origin = if reserves_handle_space {
                    cursor
                } else {
                    cursor - handle_hit / 2.0
                };
                let handle_bounds = if is_vertical {
                    ui_rect(
                        ui_point(bounds.origin.x, ui_px(handle_origin)),
                        ui_size(bounds.size.width, ui_px(handle_hit)),
                    )
                } else {
                    ui_rect(
                        ui_point(ui_px(handle_origin), bounds.origin.y),
                        ui_size(ui_px(handle_hit), bounds.size.height),
                    )
                };
                let visual_bounds = centered_handle_visual_bounds(
                    handle_bounds,
                    state.orientation(),
                    ui_px(handle_thickness),
                );
                self.handles.push(SplitterHandleLayout {
                    group_id: state.group_id().to_owned(),
                    orientation: state.orientation(),
                    index: handle.index(),
                    before_id: handle.before_id().to_owned(),
                    after_id: handle.after_id().to_owned(),
                    disabled: handle.disabled(),
                    bounds: handle_bounds,
                    visual_bounds,
                });
                if reserves_handle_space {
                    cursor += handle_hit;
                }
            }
        }

        child_bounds
    }

    fn resolve_junctions(&mut self) {
        self.junctions = junctions_for_handles(&self.handles);
    }
}

/// Programmatic split layout transition intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterTransitionIntent {
    /// A panel or nested split is inserted.
    Insert,
    /// A panel or nested split is removed.
    Remove,
    /// One or more panels collapse.
    Collapse,
    /// One or more panels expand.
    Expand,
    /// Existing panel fractions or bounds change.
    Resize,
    /// The adapter cannot classify the semantic reason more narrowly.
    Replace,
}

/// Transition classification for one split panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterPanelTransitionKind {
    /// Panel remains visually unchanged.
    Unchanged,
    /// Panel enters the scene.
    Entering,
    /// Panel leaves the scene.
    Leaving,
    /// Panel moves without resizing.
    Moving,
    /// Panel changes size.
    Resizing,
    /// Panel transitions into its collapsed state.
    Collapsing,
    /// Panel transitions out of its collapsed state.
    Expanding,
}

/// Transition descriptor for one split panel.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelTransition {
    id: String,
    kind: SplitterPanelTransitionKind,
    from: Option<UiRect>,
    to: Option<UiRect>,
    collapsed_from: bool,
    collapsed_to: bool,
}

impl SplitterPanelTransition {
    /// Returns the stable panel id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns this panel's transition kind.
    pub const fn kind(&self) -> SplitterPanelTransitionKind {
        self.kind
    }

    /// Returns the previous visual bounds, when the panel existed before the transition.
    pub const fn from(&self) -> Option<UiRect> {
        self.from
    }

    /// Returns the final visual bounds, when the panel exists after the transition.
    pub const fn to(&self) -> Option<UiRect> {
        self.to
    }

    /// Returns whether the panel was collapsed before the transition.
    pub const fn collapsed_from(&self) -> bool {
        self.collapsed_from
    }

    /// Returns whether the panel is collapsed after the transition.
    pub const fn collapsed_to(&self) -> bool {
        self.collapsed_to
    }
}

/// Transition classification for one split handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterHandleTransitionKind {
    /// Handle remains visually unchanged.
    Unchanged,
    /// Handle enters the scene.
    Entering,
    /// Handle leaves the scene.
    Leaving,
    /// Handle moves or changes disabled state.
    Moving,
}

/// Transition descriptor for one split handle.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterHandleTransition {
    group_id: String,
    index: usize,
    kind: SplitterHandleTransitionKind,
    from: Option<UiRect>,
    to: Option<UiRect>,
    disabled_from: bool,
    disabled_to: bool,
}

impl SplitterHandleTransition {
    /// Returns the split group id.
    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    /// Returns the handle index within the split group.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns this handle's transition kind.
    pub const fn kind(&self) -> SplitterHandleTransitionKind {
        self.kind
    }

    /// Returns previous handle bounds, when the handle existed before the transition.
    pub const fn from(&self) -> Option<UiRect> {
        self.from
    }

    /// Returns final handle bounds, when the handle exists after the transition.
    pub const fn to(&self) -> Option<UiRect> {
        self.to
    }

    /// Returns whether the handle was disabled before the transition.
    pub const fn disabled_from(&self) -> bool {
        self.disabled_from
    }

    /// Returns whether the handle is disabled after the transition.
    pub const fn disabled_to(&self) -> bool {
        self.disabled_to
    }
}

/// Renderer-neutral transition descriptor between two resolved split layout scenes.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterLayoutTransition {
    intent: SplitterTransitionIntent,
    from: SplitterLayoutScene,
    to: SplitterLayoutScene,
    spec: MotionSpec,
    panels: Vec<SplitterPanelTransition>,
    handles: Vec<SplitterHandleTransition>,
}

/// Sampled split layout transition frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterLayoutTransitionSample {
    progress: f32,
    panels: Vec<SplitterPanelTransitionSample>,
    handles: Vec<SplitterHandleTransitionSample>,
}

/// Sampled split panel transition frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelTransitionSample {
    id: String,
    kind: SplitterPanelTransitionKind,
    clip: Option<MotionProjectionClip>,
}

/// Sampled split handle transition frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterHandleTransitionSample {
    group_id: String,
    index: usize,
    kind: SplitterHandleTransitionKind,
    bounds: Option<UiRect>,
}

impl SplitterLayoutTransition {
    /// Resolves a transition between two flat split layout scenes.
    pub fn between(
        intent: SplitterTransitionIntent,
        from: SplitterLayoutScene,
        to: SplitterLayoutScene,
        spec: MotionSpec,
    ) -> Self {
        let panels = split_panel_transitions(&from, &to);
        let handles = split_handle_transitions(&from, &to);

        Self {
            intent,
            from,
            to,
            spec,
            panels,
            handles,
        }
    }

    /// Returns the semantic transition intent supplied by the adapter.
    pub const fn intent(&self) -> SplitterTransitionIntent {
        self.intent
    }

    /// Returns the previous resolved scene.
    pub const fn from_scene(&self) -> &SplitterLayoutScene {
        &self.from
    }

    /// Returns the final resolved scene.
    pub const fn to_scene(&self) -> &SplitterLayoutScene {
        &self.to
    }

    /// Returns the motion spec adapters should use to sample this transition.
    pub const fn spec(&self) -> MotionSpec {
        self.spec
    }

    /// Returns whether the transition completes immediately.
    pub const fn is_immediate(&self) -> bool {
        self.spec.is_immediate()
    }

    /// Samples renderer-neutral panel and handle transition geometry at unit progress.
    pub fn sample(&self, progress: f32) -> SplitterLayoutTransitionSample {
        let progress = if self.spec.preference().is_immediate() {
            1.0
        } else {
            progress.clamp(0.0, 1.0)
        };
        SplitterLayoutTransitionSample {
            progress,
            panels: self
                .panels
                .iter()
                .map(|panel| {
                    SplitterPanelTransitionSample::from_transition(
                        panel,
                        self.to.bounds(),
                        self.to.orientation(),
                        progress,
                        self.spec.preference(),
                    )
                })
                .collect(),
            handles: self
                .handles
                .iter()
                .map(|handle| SplitterHandleTransitionSample::from_transition(handle, progress))
                .collect(),
        }
    }

    /// Returns panel transition descriptors.
    pub fn panels(&self) -> &[SplitterPanelTransition] {
        &self.panels
    }

    /// Returns handle transition descriptors.
    pub fn handles(&self) -> &[SplitterHandleTransition] {
        &self.handles
    }

    /// Returns the transition descriptor for a panel id.
    pub fn panel(&self, id: &str) -> Option<&SplitterPanelTransition> {
        self.panels.iter().find(|panel| panel.id == id)
    }
}

impl SplitterLayoutTransitionSample {
    /// Returns clamped transition progress.
    pub const fn progress(&self) -> f32 {
        self.progress
    }

    /// Returns sampled panel transitions.
    pub fn panels(&self) -> &[SplitterPanelTransitionSample] {
        &self.panels
    }

    /// Returns sampled handle transitions.
    pub fn handles(&self) -> &[SplitterHandleTransitionSample] {
        &self.handles
    }

    /// Returns the sampled transition for a panel id.
    pub fn panel(&self, id: &str) -> Option<&SplitterPanelTransitionSample> {
        self.panels.iter().find(|panel| panel.id == id)
    }
}

impl SplitterPanelTransitionSample {
    fn from_transition(
        transition: &SplitterPanelTransition,
        scene_bounds: UiRect,
        orientation: Orientation,
        progress: f32,
        preference: MotionPreference,
    ) -> Self {
        let clip = match (transition.kind, transition.from, transition.to) {
            (SplitterPanelTransitionKind::Entering, _, Some(to)) => Some(splitter_enter_clip(
                to,
                scene_bounds,
                orientation,
                progress,
                preference,
            )),
            (SplitterPanelTransitionKind::Leaving, Some(from), _) => Some(splitter_leave_clip(
                from,
                scene_bounds,
                orientation,
                progress,
                preference,
            )),
            (
                SplitterPanelTransitionKind::Moving
                | SplitterPanelTransitionKind::Resizing
                | SplitterPanelTransitionKind::Collapsing
                | SplitterPanelTransitionKind::Expanding,
                Some(from),
                Some(to),
            ) => splitter_resize_clip(
                transition.kind,
                from,
                to,
                scene_bounds,
                orientation,
                progress,
                preference,
            ),
            _ => None,
        };
        Self {
            id: transition.id.clone(),
            kind: transition.kind,
            clip,
        }
    }

    /// Returns the stable panel id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the transition kind.
    pub const fn kind(&self) -> SplitterPanelTransitionKind {
        self.kind
    }

    /// Returns the sampled clip for panels that need transition rendering.
    pub const fn clip(&self) -> Option<MotionProjectionClip> {
        self.clip
    }
}

fn splitter_resize_clip(
    kind: SplitterPanelTransitionKind,
    from: UiRect,
    to: UiRect,
    scene_bounds: UiRect,
    orientation: Orientation,
    progress: f32,
    preference: MotionPreference,
) -> Option<MotionProjectionClip> {
    match MotionProjectionClip::from_projection_with_preference(
        MotionProjection::between(motion_rect_from_ui_rect(from), motion_rect_from_ui_rect(to)),
        progress,
        preference,
    ) {
        Ok(clip) => Some(clip),
        Err(MotionProjectionError::NonPositiveExtent) => match kind {
            // A zero-size collapsed panel has no invertible target geometry. Preserve the
            // collapse/expand contract with an edge reveal instead of manufacturing a scale.
            SplitterPanelTransitionKind::Collapsing => Some(splitter_leave_clip(
                from,
                scene_bounds,
                orientation,
                progress,
                preference,
            )),
            SplitterPanelTransitionKind::Expanding => Some(splitter_enter_clip(
                to,
                scene_bounds,
                orientation,
                progress,
                preference,
            )),
            _ => None,
        },
        // Invalid renderer-neutral geometry degrades to the committed layout. It must never
        // panic or inject a non-finite clip into the element tree.
        Err(_) => None,
    }
}

impl SplitterHandleTransitionSample {
    fn from_transition(transition: &SplitterHandleTransition, progress: f32) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        let bounds = match (transition.from, transition.to) {
            (Some(from), Some(to)) => Some(lerp_ui_rect(from, to, progress)),
            (None, Some(to)) => Some(to),
            (Some(from), None) => Some(from),
            (None, None) => None,
        };
        Self {
            group_id: transition.group_id.clone(),
            index: transition.index,
            kind: transition.kind,
            bounds,
        }
    }

    /// Returns the split group id.
    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    /// Returns the handle index within the split group.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the transition kind.
    pub const fn kind(&self) -> SplitterHandleTransitionKind {
        self.kind
    }

    /// Returns sampled handle bounds.
    pub const fn bounds(&self) -> Option<UiRect> {
        self.bounds
    }
}

fn splitter_enter_clip(
    to: UiRect,
    scene_bounds: UiRect,
    orientation: Orientation,
    progress: f32,
    preference: MotionPreference,
) -> MotionProjectionClip {
    let progress = if preference.is_immediate() {
        1.0
    } else {
        progress.clamp(0.0, 1.0)
    };
    MotionProjectionClip::reveal(
        motion_rect_from_ui_rect(to),
        splitter_reveal_edge(to, scene_bounds, orientation),
        progress,
    )
}

fn splitter_leave_clip(
    from: UiRect,
    scene_bounds: UiRect,
    orientation: Orientation,
    progress: f32,
    preference: MotionPreference,
) -> MotionProjectionClip {
    let progress = if preference.is_immediate() {
        1.0
    } else {
        progress.clamp(0.0, 1.0)
    };
    let edge = splitter_reveal_edge(from, scene_bounds, orientation);
    let from = motion_rect_from_ui_rect(from);
    MotionProjectionClip::new(
        from,
        reveal_rect_from_edge(from, edge, 1.0 - progress),
        from,
        progress,
    )
}

fn splitter_reveal_edge(
    bounds: UiRect,
    scene_bounds: UiRect,
    orientation: Orientation,
) -> MotionEdge {
    match orientation {
        Orientation::Horizontal => {
            if rect_center_x(bounds) <= rect_center_x(scene_bounds) {
                MotionEdge::Left
            } else {
                MotionEdge::Right
            }
        }
        Orientation::Vertical => {
            if rect_center_y(bounds) <= rect_center_y(scene_bounds) {
                MotionEdge::Top
            } else {
                MotionEdge::Bottom
            }
        }
    }
}

fn motion_rect_from_ui_rect(rect: UiRect) -> MotionRect {
    motion_rect(
        motion_point(
            motion_px(rect.origin.x.as_f32()),
            motion_px(rect.origin.y.as_f32()),
        ),
        motion_size(
            motion_px(rect.size.width.as_f32()),
            motion_px(rect.size.height.as_f32()),
        ),
    )
}

fn lerp_ui_rect(from: UiRect, to: UiRect, progress: f32) -> UiRect {
    let progress = progress.clamp(0.0, 1.0);
    ui_rect(
        ui_point(
            ui_px(lerp_f32(
                from.origin.x.as_f32(),
                to.origin.x.as_f32(),
                progress,
            )),
            ui_px(lerp_f32(
                from.origin.y.as_f32(),
                to.origin.y.as_f32(),
                progress,
            )),
        ),
        ui_size(
            ui_px(lerp_f32(
                from.size.width.as_f32(),
                to.size.width.as_f32(),
                progress,
            )),
            ui_px(lerp_f32(
                from.size.height.as_f32(),
                to.size.height.as_f32(),
                progress,
            )),
        ),
    )
}

fn lerp_f32(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

fn rect_center_x(bounds: UiRect) -> f32 {
    bounds.origin.x.as_f32() + bounds.size.width.as_f32() / 2.0
}

fn rect_center_y(bounds: UiRect) -> f32 {
    bounds.origin.y.as_f32() + bounds.size.height.as_f32() / 2.0
}

/// Resolved layout for one split leaf panel.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelLayout {
    id: String,
    index: usize,
    bounds: UiRect,
    fraction: f32,
    collapsed: bool,
}

impl SplitterPanelLayout {
    /// Returns the panel id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the scene-wide panel index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the resolved panel bounds.
    pub const fn bounds(&self) -> UiRect {
        self.bounds
    }

    /// Returns the resolved panel fraction within its parent split.
    pub const fn fraction(&self) -> f32 {
        self.fraction
    }

    /// Returns whether the panel is collapsed.
    pub const fn collapsed(&self) -> bool {
        self.collapsed
    }
}

/// Resolved layout for one split handle.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterHandleLayout {
    group_id: String,
    orientation: Orientation,
    index: usize,
    before_id: String,
    after_id: String,
    disabled: bool,
    bounds: UiRect,
    visual_bounds: UiRect,
}

impl SplitterHandleLayout {
    /// Creates a handle layout from adapter-resolved bounds.
    pub fn new(
        group_id: impl Into<String>,
        orientation: Orientation,
        index: usize,
        before_id: impl Into<String>,
        after_id: impl Into<String>,
        disabled: bool,
        bounds: UiRect,
        visual_bounds: UiRect,
    ) -> Self {
        Self {
            group_id: group_id.into(),
            orientation,
            index,
            before_id: before_id.into(),
            after_id: after_id.into(),
            disabled,
            bounds,
            visual_bounds,
        }
    }

    /// Returns the split group id that owns this handle.
    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    /// Returns the split orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the handle index within its split group.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the id before the handle.
    pub fn before_id(&self) -> &str {
        &self.before_id
    }

    /// Returns the id after the handle.
    pub fn after_id(&self) -> &str {
        &self.after_id
    }

    /// Returns whether resize is disabled for this handle.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the handle hit bounds.
    pub const fn bounds(&self) -> UiRect {
        self.bounds
    }

    /// Returns the painted handle bounds.
    pub const fn visual_bounds(&self) -> UiRect {
        self.visual_bounds
    }
}

/// Resolved hit region for intersecting split handles.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterJunctionHitRegion {
    bounds: UiRect,
    horizontal: SplitterHandleLayout,
    vertical: SplitterHandleLayout,
}

impl SplitterJunctionHitRegion {
    /// Returns the junction hit bounds.
    pub const fn bounds(&self) -> UiRect {
        self.bounds
    }

    /// Returns the horizontal handle participating in the junction.
    pub const fn horizontal(&self) -> &SplitterHandleLayout {
        &self.horizontal
    }

    /// Returns the vertical handle participating in the junction.
    pub const fn vertical(&self) -> &SplitterHandleLayout {
        &self.vertical
    }
}

/// Split hit target.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitterHitTarget {
    /// A single split handle target.
    Handle(SplitterHandleLayout),
    /// A junction between horizontal and vertical split handles.
    Junction(SplitterJunctionHitRegion),
}

/// Hit map for split handles and junctions.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterHitMap {
    targets: Vec<SplitterHitTarget>,
}

impl SplitterHitMap {
    /// Builds a hit map from a split layout scene.
    pub fn from_scene(scene: &SplitterLayoutScene) -> Self {
        let mut targets = scene
            .junctions()
            .iter()
            .cloned()
            .map(SplitterHitTarget::Junction)
            .collect::<Vec<_>>();
        targets.extend(
            scene
                .handles()
                .iter()
                .cloned()
                .map(SplitterHitTarget::Handle),
        );
        Self { targets }
    }

    /// Builds a hit map from resolved handle layouts.
    pub fn from_handles(handles: impl IntoIterator<Item = SplitterHandleLayout>) -> Self {
        let handles = handles.into_iter().collect::<Vec<_>>();
        let mut targets = junctions_for_handles(&handles)
            .into_iter()
            .map(SplitterHitTarget::Junction)
            .collect::<Vec<_>>();
        targets.extend(handles.into_iter().map(SplitterHitTarget::Handle));
        Self { targets }
    }

    /// Returns all hit targets in priority order.
    pub fn targets(&self) -> &[SplitterHitTarget] {
        &self.targets
    }

    /// Returns the top-priority target containing the point.
    pub fn hit(&self, point: crate::UiPoint) -> Option<&SplitterHitTarget> {
        self.targets.iter().find(|target| match target {
            SplitterHitTarget::Handle(handle) => contains_point(handle.bounds(), point),
            SplitterHitTarget::Junction(junction) => contains_point(junction.bounds(), point),
        })
    }
}

fn split_panel_transitions(
    from: &SplitterLayoutScene,
    to: &SplitterLayoutScene,
) -> Vec<SplitterPanelTransition> {
    let previous = from
        .panels()
        .iter()
        .map(|panel| (panel.id().to_owned(), panel))
        .collect::<HashMap<_, _>>();
    let next = to
        .panels()
        .iter()
        .map(|panel| (panel.id().to_owned(), panel))
        .collect::<HashMap<_, _>>();
    let mut transitions = Vec::new();

    for panel in to.panels() {
        let previous_panel = previous.get(panel.id()).copied();
        transitions.push(match previous_panel {
            Some(previous_panel) => SplitterPanelTransition {
                id: panel.id().to_owned(),
                kind: split_panel_transition_kind(previous_panel, panel),
                from: Some(previous_panel.bounds()),
                to: Some(panel.bounds()),
                collapsed_from: previous_panel.collapsed(),
                collapsed_to: panel.collapsed(),
            },
            None => SplitterPanelTransition {
                id: panel.id().to_owned(),
                kind: SplitterPanelTransitionKind::Entering,
                from: None,
                to: Some(panel.bounds()),
                collapsed_from: false,
                collapsed_to: panel.collapsed(),
            },
        });
    }

    for panel in from.panels() {
        if next.contains_key(panel.id()) {
            continue;
        }
        transitions.push(SplitterPanelTransition {
            id: panel.id().to_owned(),
            kind: SplitterPanelTransitionKind::Leaving,
            from: Some(panel.bounds()),
            to: None,
            collapsed_from: panel.collapsed(),
            collapsed_to: false,
        });
    }

    transitions
}

fn split_panel_transition_kind(
    from: &SplitterPanelLayout,
    to: &SplitterPanelLayout,
) -> SplitterPanelTransitionKind {
    if !from.collapsed() && to.collapsed() {
        return SplitterPanelTransitionKind::Collapsing;
    }
    if from.collapsed() && !to.collapsed() {
        return SplitterPanelTransitionKind::Expanding;
    }
    if from.bounds() == to.bounds() {
        SplitterPanelTransitionKind::Unchanged
    } else if from.bounds().size != to.bounds().size {
        SplitterPanelTransitionKind::Resizing
    } else {
        SplitterPanelTransitionKind::Moving
    }
}

fn split_handle_transitions(
    from: &SplitterLayoutScene,
    to: &SplitterLayoutScene,
) -> Vec<SplitterHandleTransition> {
    let previous = from
        .handles()
        .iter()
        .map(|handle| (split_handle_key(handle), handle))
        .collect::<HashMap<_, _>>();
    let next = to
        .handles()
        .iter()
        .map(|handle| (split_handle_key(handle), handle))
        .collect::<HashMap<_, _>>();
    let mut transitions = Vec::new();

    for handle in to.handles() {
        let previous_handle = previous.get(&split_handle_key(handle)).copied();
        transitions.push(match previous_handle {
            Some(previous_handle) => SplitterHandleTransition {
                group_id: handle.group_id().to_owned(),
                index: handle.index(),
                kind: split_handle_transition_kind(previous_handle, handle),
                from: Some(previous_handle.bounds()),
                to: Some(handle.bounds()),
                disabled_from: previous_handle.disabled(),
                disabled_to: handle.disabled(),
            },
            None => SplitterHandleTransition {
                group_id: handle.group_id().to_owned(),
                index: handle.index(),
                kind: SplitterHandleTransitionKind::Entering,
                from: None,
                to: Some(handle.bounds()),
                disabled_from: false,
                disabled_to: handle.disabled(),
            },
        });
    }

    for handle in from.handles() {
        if next.contains_key(&split_handle_key(handle)) {
            continue;
        }
        transitions.push(SplitterHandleTransition {
            group_id: handle.group_id().to_owned(),
            index: handle.index(),
            kind: SplitterHandleTransitionKind::Leaving,
            from: Some(handle.bounds()),
            to: None,
            disabled_from: handle.disabled(),
            disabled_to: false,
        });
    }

    transitions
}

fn split_handle_key(handle: &SplitterHandleLayout) -> (String, usize) {
    (handle.group_id().to_owned(), handle.index())
}

fn split_handle_transition_kind(
    from: &SplitterHandleLayout,
    to: &SplitterHandleLayout,
) -> SplitterHandleTransitionKind {
    if from.bounds() == to.bounds() && from.disabled() == to.disabled() {
        SplitterHandleTransitionKind::Unchanged
    } else {
        SplitterHandleTransitionKind::Moving
    }
}

fn junctions_for_handles(handles: &[SplitterHandleLayout]) -> Vec<SplitterJunctionHitRegion> {
    let mut junctions = Vec::new();
    for horizontal in handles
        .iter()
        .filter(|handle| matches!(handle.orientation, Orientation::Horizontal))
    {
        for vertical in handles
            .iter()
            .filter(|handle| matches!(handle.orientation, Orientation::Vertical))
        {
            if let Some(bounds) = orthogonal_handle_junction(horizontal.bounds, vertical.bounds) {
                junctions.push(SplitterJunctionHitRegion {
                    bounds,
                    horizontal: horizontal.clone(),
                    vertical: vertical.clone(),
                });
            }
        }
    }
    junctions
}

/// A nested split tree node.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitTreeNode {
    /// A leaf pane.
    Leaf {
        /// Stable leaf id.
        id: String,
    },
    /// A split containing child nodes.
    Split {
        /// Stable split id.
        id: String,
        /// Split orientation.
        orientation: Orientation,
        /// Child nodes and their constraints.
        children: Vec<SplitTreeChild>,
    },
}

impl SplitTreeNode {
    /// Creates a leaf node.
    pub fn leaf(id: impl Into<String>) -> Self {
        Self::Leaf { id: id.into() }
    }

    /// Creates a split node.
    pub fn split(
        id: impl Into<String>,
        orientation: Orientation,
        children: impl IntoIterator<Item = SplitTreeChild>,
    ) -> Self {
        Self::Split {
            id: id.into(),
            orientation,
            children: children.into_iter().collect(),
        }
    }

    /// Returns the node id.
    pub fn id(&self) -> &str {
        match self {
            Self::Leaf { id } | Self::Split { id, .. } => id,
        }
    }

    /// Returns the split orientation, if this node is a split.
    pub const fn orientation(&self) -> Option<Orientation> {
        match self {
            Self::Leaf { .. } => None,
            Self::Split { orientation, .. } => Some(*orientation),
        }
    }
}

/// A constrained child entry for a split tree node.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitTreeChild {
    node: SplitTreeNode,
    fraction: f32,
    min_fraction: f32,
    max_fraction: f32,
}

impl SplitTreeChild {
    /// Creates a child entry.
    pub fn new(node: SplitTreeNode, fraction: f32) -> Self {
        Self {
            node,
            fraction,
            min_fraction: 0.1,
            max_fraction: 1.0,
        }
    }

    /// Applies the minimum child fraction.
    pub fn min_fraction(mut self, min_fraction: f32) -> Self {
        self.min_fraction = min_fraction;
        self
    }

    /// Applies the maximum child fraction.
    pub fn max_fraction(mut self, max_fraction: f32) -> Self {
        self.max_fraction = max_fraction;
        self
    }

    /// Returns the child node.
    pub const fn node(&self) -> &SplitTreeNode {
        &self.node
    }

    /// Returns the child fraction.
    pub const fn fraction(&self) -> f32 {
        self.fraction
    }
}

fn centered_handle_visual_bounds(
    bounds: UiRect,
    orientation: Orientation,
    thickness: UiPx,
) -> UiRect {
    match orientation {
        Orientation::Horizontal => {
            let x = bounds.origin.x + (bounds.size.width - thickness) / 2.0;
            ui_rect(
                ui_point(x, bounds.origin.y),
                ui_size(thickness, bounds.size.height),
            )
        }
        Orientation::Vertical => {
            let y = bounds.origin.y + (bounds.size.height - thickness) / 2.0;
            ui_rect(
                ui_point(bounds.origin.x, y),
                ui_size(bounds.size.width, thickness),
            )
        }
    }
}

fn sanitize_fraction(fraction: f32) -> f32 {
    if fraction.is_finite() {
        fraction.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_max_fraction(fraction: f32) -> f32 {
    if fraction.is_finite() {
        fraction.max(0.0)
    } else {
        1.0
    }
}

fn collapsed_restore_threshold(panel: &SplitterPanelState) -> f32 {
    panel.min_fraction.max(panel.collapsed_fraction)
}

fn apply_resize_fraction(panel: &mut SplitterPanelState, fraction: f32) -> bool {
    let fraction = sanitize_fraction(fraction);
    if panel.collapsed {
        if fraction + EPSILON < collapsed_restore_threshold(panel) {
            panel.fraction = panel.collapsed_fraction;
            return false;
        }

        panel.collapsed = false;
    }

    panel.fraction = fraction.clamp(panel.min_fraction, panel.max_fraction);
    true
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

fn contains_point(bounds: UiRect, point: crate::UiPoint) -> bool {
    point.x >= bounds.origin.x
        && point.y >= bounds.origin.y
        && point.x <= bounds.origin.x + bounds.size.width
        && point.y <= bounds.origin.y + bounds.size.height
}

fn orthogonal_handle_junction(horizontal: UiRect, vertical: UiRect) -> Option<UiRect> {
    let x = overlap_or_touching_span(
        horizontal.origin.x.as_f32(),
        (horizontal.origin.x + horizontal.size.width).as_f32(),
        vertical.origin.x.as_f32(),
        (vertical.origin.x + vertical.size.width).as_f32(),
        horizontal.origin.x.as_f32(),
        (horizontal.origin.x + horizontal.size.width).as_f32(),
    )?;
    let y = overlap_or_touching_span(
        horizontal.origin.y.as_f32(),
        (horizontal.origin.y + horizontal.size.height).as_f32(),
        vertical.origin.y.as_f32(),
        (vertical.origin.y + vertical.size.height).as_f32(),
        vertical.origin.y.as_f32(),
        (vertical.origin.y + vertical.size.height).as_f32(),
    )?;

    Some(ui_rect(
        ui_point(ui_px(x.0), ui_px(y.0)),
        ui_size(ui_px(x.1 - x.0), ui_px(y.1 - y.0)),
    ))
}

fn overlap_or_touching_span(
    a_start: f32,
    a_end: f32,
    b_start: f32,
    b_end: f32,
    touching_start: f32,
    touching_end: f32,
) -> Option<(f32, f32)> {
    let start = a_start.max(b_start);
    let end = a_end.min(b_end);
    if end - start > EPSILON {
        return Some((start, end));
    }

    ((a_end - b_start).abs() <= EPSILON || (b_end - a_start).abs() <= EPSILON)
        .then_some((touching_start, touching_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_motion::MotionPreference;

    fn rect(width: f32, height: f32) -> UiRect {
        at_rect(0.0, 0.0, width, height)
    }

    fn at_rect(x: f32, y: f32, width: f32, height: f32) -> UiRect {
        ui_rect(
            ui_point(ui_px(x), ui_px(y)),
            ui_size(ui_px(width), ui_px(height)),
        )
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.001,
            "expected {actual} to be close to {expected}"
        );
    }

    #[test]
    fn split_fraction_resolution_fills_missing_fractions() {
        let shares = resolve_split_fractions(3, &[0.25]);

        assert_close(shares.iter().sum(), 1.0);
        assert_close(shares[0], 0.1111);
        assert_close(shares[1], 0.4444);
        assert_close(shares[2], 0.4444);
    }

    #[test]
    fn split_fraction_normalization_repairs_invalid_values() {
        let mut shares = vec![f32::NAN, -1.0, 3.0];

        normalize_split_fractions(&mut shares);

        assert_close(shares.iter().sum(), 1.0);
        assert_close(shares[0], 0.0);
        assert_close(shares[1], 0.0);
        assert_close(shares[2], 1.0);
    }

    #[test]
    fn split_fraction_normalization_falls_back_to_equal_shares_when_sum_is_empty() {
        let mut shares = vec![0.0, f32::NAN, -1.0];

        normalize_split_fractions(&mut shares);

        assert_eq!(shares, vec![1.0 / 3.0; 3]);
    }

    #[test]
    fn split_fraction_fill_child_receives_remaining_share() {
        let shares = resolve_split_fractions_with_fill_child(3, &[0.2, 0.0, 0.3], Some(1));

        assert_close(shares[0], 0.2);
        assert_close(shares[1], 0.5);
        assert_close(shares[2], 0.3);
        assert_close(shares.iter().sum(), 1.0);
    }

    #[test]
    fn split_fraction_fill_child_yields_when_siblings_over_allocate() {
        let shares = resolve_split_fractions_with_fill_child(3, &[0.8, 0.0, 0.7], Some(1));

        assert_close(shares[0], 0.5333);
        assert_close(shares[1], 0.0);
        assert_close(shares[2], 0.4667);
        assert_close(shares.iter().sum(), 1.0);
    }

    #[test]
    fn split_state_resize_reports_outcomes() {
        let state = SplitterState::resolve(
            "editor",
            Orientation::Horizontal,
            Size::Small,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.35)
                    .min_fraction(0.2)
                    .max_fraction(0.4),
                SplitterPanelDescriptor::new("right", 0.65)
                    .min_fraction(0.5)
                    .max_fraction(0.8),
            ],
        );

        let applied = state.resize_by(0, 0.02);
        assert_eq!(applied.outcome(), SplitterResizeOutcome::Applied);
        assert!((applied.state().panels()[0].fraction() - 0.37).abs() < 0.001);

        let clamped = state.resize_by(0, 0.3);
        assert_eq!(clamped.outcome(), SplitterResizeOutcome::Clamped);
        assert!((clamped.state().panels()[0].fraction() - 0.4).abs() < 0.001);

        let rejected = state.resize_by(0, f32::NAN);
        assert_eq!(rejected.outcome(), SplitterResizeOutcome::Rejected);
        assert_eq!(rejected.state(), &state);
    }

    #[test]
    fn split_state_resize_by_pixels_converts_extent_to_fraction_delta() {
        let state = SplitterState::resolve(
            "editor",
            Orientation::Horizontal,
            Size::Small,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.25).min_fraction(0.12),
                SplitterPanelDescriptor::new("right", 0.75).min_fraction(0.12),
            ],
        );

        let applied = state.resize_by_pixels(0, ui_px(400.0), ui_px(40.0));
        assert_eq!(applied.outcome(), SplitterResizeOutcome::Applied);
        assert_close(applied.state().panels()[0].fraction(), 0.35);
        assert_close(applied.state().panels()[1].fraction(), 0.65);

        let clamped = state.resize_by_pixels(0, ui_px(400.0), ui_px(-300.0));
        assert_eq!(clamped.outcome(), SplitterResizeOutcome::Clamped);
        assert_close(clamped.state().panels()[0].fraction(), 0.12);
        assert_close(clamped.state().panels()[1].fraction(), 0.88);
    }

    #[test]
    fn splitter_metrics_panel_axis_extent_respects_handle_placement() {
        let metrics = SplitterMetrics::from_size(Size::Medium);

        assert_eq!(metrics.panel_axis_extent(ui_px(400.0), 2), ui_px(376.0));
        assert_eq!(
            metrics
                .with_handle_placement(SplitterHandlePlacement::OverlayBoundary)
                .panel_axis_extent(ui_px(400.0), 2),
            ui_px(400.0)
        );
        assert_eq!(metrics.panel_axis_extent(ui_px(12.0), 2), ui_px(0.0));
    }

    #[test]
    fn split_state_resize_by_pixels_rejects_invalid_extent() {
        let state = SplitterState::resolve(
            "editor",
            Orientation::Horizontal,
            Size::Small,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.5),
                SplitterPanelDescriptor::new("right", 0.5),
            ],
        );

        let rejected = state.resize_by_pixels(0, ui_px(0.0), ui_px(40.0));

        assert_eq!(rejected.outcome(), SplitterResizeOutcome::Rejected);
        assert_eq!(rejected.state(), &state);
    }

    #[test]
    fn split_fraction_pixel_resize_grows_first_adjacent_pane() {
        let next = resize_split_fractions_by_pixels(
            &[0.25, 0.75],
            0,
            ui_px(400.0),
            ui_px(40.0),
            ui_px(48.0),
        )
        .expect("resize should be valid");

        assert_close(next[0], 0.35);
        assert_close(next[1], 0.65);
    }

    #[test]
    fn split_fraction_pixel_resize_shrinks_first_adjacent_pane() {
        let next = resize_split_fractions_by_pixels(
            &[0.5, 0.5],
            0,
            ui_px(400.0),
            ui_px(-80.0),
            ui_px(48.0),
        )
        .expect("resize should be valid");

        assert_close(next[0], 0.3);
        assert_close(next[1], 0.7);
    }

    #[test]
    fn split_fraction_pixel_resize_clamps_at_minimum_pane_extent() {
        let next = resize_split_fractions_by_pixels(
            &[0.5, 0.5],
            0,
            ui_px(400.0),
            ui_px(-300.0),
            ui_px(100.0),
        )
        .expect("resize should be valid");

        assert_close(next[0], 0.25);
        assert_close(next[1], 0.75);
    }

    #[test]
    fn split_fraction_pixel_resize_splits_impossible_minimum_evenly() {
        let next = resize_split_fractions_by_pixels(
            &[0.5, 0.5],
            0,
            ui_px(120.0),
            ui_px(100.0),
            ui_px(80.0),
        )
        .expect("resize should be valid");

        assert_close(next[0], 0.5);
        assert_close(next[1], 0.5);
    }

    #[test]
    fn split_fraction_pixel_resize_rejects_invalid_handle_index() {
        assert!(
            resize_split_fractions_by_pixels(
                &[0.5, 0.5],
                1,
                ui_px(400.0),
                ui_px(10.0),
                ui_px(48.0)
            )
            .is_none()
        );
    }

    #[test]
    fn split_layout_scene_resolves_ordered_panels_and_handles() {
        let state = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("nav", 0.25),
                SplitterPanelDescriptor::new("main", 0.75),
            ],
        );
        let scene = SplitterLayoutScene::from_state(&state, rect(412.0, 200.0));

        assert_eq!(scene.panels().len(), 2);
        assert_eq!(scene.handles().len(), 1);
        assert_eq!(scene.panels()[0].id(), "nav");
        assert!((scene.panels()[0].bounds().size.width.as_f32() - 100.0).abs() < 0.001);
        assert!((scene.handles()[0].bounds().origin.x.as_f32() - 100.0).abs() < 0.001);
        assert_eq!(scene.handles()[0].before_id(), "nav");
        assert_eq!(scene.handles()[0].after_id(), "main");
    }

    #[test]
    fn split_layout_scene_can_overlay_handles_on_panel_boundaries() {
        let state = SplitterState::resolve(
            "dock",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.25).min_fraction(0.0),
                SplitterPanelDescriptor::new("right", 0.75).min_fraction(0.0),
            ],
        );
        let metrics = SplitterMetrics::new(ui_px(2.0), ui_px(6.0), ui_px(0.0))
            .with_handle_placement(SplitterHandlePlacement::OverlayBoundary);
        let scene =
            SplitterLayoutScene::from_state_with_metrics(&state, rect(400.0, 200.0), metrics);

        assert!((scene.panels()[0].bounds().size.width.as_f32() - 100.0).abs() < 0.001);
        assert!((scene.panels()[1].bounds().origin.x.as_f32() - 100.0).abs() < 0.001);
        assert!((scene.panels()[1].bounds().size.width.as_f32() - 300.0).abs() < 0.001);
        assert!((scene.handles()[0].bounds().origin.x.as_f32() - 97.0).abs() < 0.001);
        assert!((scene.handles()[0].bounds().size.width.as_f32() - 6.0).abs() < 0.001);
    }

    #[test]
    fn split_tree_scene_resolves_leaf_panels_and_junctions() {
        let tree = SplitTreeNode::split(
            "root",
            Orientation::Horizontal,
            [
                SplitTreeChild::new(SplitTreeNode::leaf("left"), 0.5),
                SplitTreeChild::new(
                    SplitTreeNode::split(
                        "right",
                        Orientation::Vertical,
                        [
                            SplitTreeChild::new(SplitTreeNode::leaf("top"), 0.5),
                            SplitTreeChild::new(SplitTreeNode::leaf("bottom"), 0.5),
                        ],
                    ),
                    0.5,
                ),
            ],
        );

        let scene = SplitterLayoutScene::from_tree(&tree, rect(424.0, 224.0), Size::Medium);
        let hit_map = SplitterHitMap::from_scene(&scene);

        assert_eq!(
            scene
                .panels()
                .iter()
                .map(SplitterPanelLayout::id)
                .collect::<Vec<_>>(),
            vec!["left", "top", "bottom"]
        );
        assert_eq!(scene.handles().len(), 2);
        assert_eq!(scene.junctions().len(), 1);
        assert!(matches!(
            hit_map.hit(scene.junctions()[0].bounds().top_left()),
            Some(SplitterHitTarget::Junction(_))
        ));
    }

    #[test]
    fn split_hit_map_from_handles_prefers_junctions() {
        let horizontal = SplitterHandleLayout::new(
            "root",
            Orientation::Horizontal,
            0,
            "left",
            "right",
            false,
            at_rect(97.0, 0.0, 6.0, 200.0),
            at_rect(97.0, 0.0, 6.0, 200.0),
        );
        let vertical = SplitterHandleLayout::new(
            "right",
            Orientation::Vertical,
            0,
            "top",
            "bottom",
            false,
            at_rect(100.0, 97.0, 180.0, 6.0),
            at_rect(100.0, 97.0, 180.0, 6.0),
        );

        let hit_map = SplitterHitMap::from_handles([horizontal, vertical]);

        assert!(matches!(
            hit_map.hit(ui_point(ui_px(100.0), ui_px(100.0))),
            Some(SplitterHitTarget::Junction(_))
        ));
    }

    #[test]
    fn split_layout_transition_describes_insert_remove_and_resize() {
        let from_state = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.5).min_fraction(0.0),
                SplitterPanelDescriptor::new("right", 0.5).min_fraction(0.0),
            ],
        );
        let to_state = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.25).min_fraction(0.0),
                SplitterPanelDescriptor::new("center", 0.25).min_fraction(0.0),
                SplitterPanelDescriptor::new("right", 0.5).min_fraction(0.0),
            ],
        );
        let from_scene = SplitterLayoutScene::from_state(&from_state, rect(424.0, 200.0));
        let to_scene = SplitterLayoutScene::from_state(&to_state, rect(436.0, 200.0));

        let insert = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Insert,
            from_scene.clone(),
            to_scene.clone(),
            MotionSpec::committed_layout(MotionPreference::Animated),
        );

        assert_eq!(insert.intent(), SplitterTransitionIntent::Insert);
        assert!(!insert.is_immediate());
        assert_eq!(
            insert.panel("center").map(SplitterPanelTransition::kind),
            Some(SplitterPanelTransitionKind::Entering)
        );
        assert_eq!(
            insert.panel("left").map(SplitterPanelTransition::kind),
            Some(SplitterPanelTransitionKind::Resizing)
        );
        assert!(
            insert
                .handles()
                .iter()
                .any(|handle| handle.kind() == SplitterHandleTransitionKind::Entering)
        );

        let insert_start = insert.sample(0.0);
        assert_eq!(insert_start.progress(), 0.0);
        let center_to = insert
            .panel("center")
            .and_then(SplitterPanelTransition::to)
            .expect("inserted panel should have final bounds");
        let center_start = insert_start
            .panel("center")
            .expect("inserted panel should be sampled");
        assert_eq!(center_start.kind(), SplitterPanelTransitionKind::Entering);
        let center_clip = center_start
            .clip()
            .expect("entering panel should provide a reveal clip");
        assert_eq!(
            center_clip.content_bounds(),
            motion_rect_from_ui_rect(center_to)
        );
        assert_eq!(
            center_clip.occlusion_bounds(),
            motion_rect_from_ui_rect(center_to)
        );
        assert_eq!(
            center_clip.visible_bounds().size.width,
            open_gpui_motion::motion_px(0.0)
        );
        assert_eq!(
            center_clip.visible_bounds().size.height,
            open_gpui_motion::motion_px(center_to.size.height.as_f32())
        );

        let left_transition = insert
            .panel("left")
            .expect("left panel should have a resize transition");
        let left_from = left_transition
            .from()
            .expect("resizing panel should have previous bounds");
        let left_to = left_transition
            .to()
            .expect("resizing panel should have final bounds");
        let left_start_clip = insert_start
            .panel("left")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("resizing panel should provide a projection clip");
        assert_eq!(
            left_start_clip.content_bounds(),
            motion_rect_from_ui_rect(left_to)
        );
        assert_eq!(
            left_start_clip.visible_bounds(),
            motion_rect_from_ui_rect(left_from)
        );
        let left_end_clip = insert
            .sample(1.0)
            .panel("left")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("resizing panel should finish at final bounds");
        assert_eq!(
            left_end_clip.visible_bounds(),
            motion_rect_from_ui_rect(left_to)
        );

        let remove = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Remove,
            to_scene,
            from_scene,
            MotionSpec::committed_layout(MotionPreference::Animated),
        );
        assert_eq!(
            remove.panel("center").map(SplitterPanelTransition::kind),
            Some(SplitterPanelTransitionKind::Leaving)
        );

        let center_from = remove
            .panel("center")
            .and_then(SplitterPanelTransition::from)
            .expect("removed panel should have previous bounds");
        let remove_start_clip = remove
            .sample(0.0)
            .panel("center")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("leaving panel should provide a reveal clip");
        assert_eq!(
            remove_start_clip.content_bounds(),
            motion_rect_from_ui_rect(center_from)
        );
        assert_eq!(
            remove_start_clip.visible_bounds(),
            motion_rect_from_ui_rect(center_from)
        );
        let remove_end_clip = remove
            .sample(1.0)
            .panel("center")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("leaving panel should shrink its visible bounds");
        assert_eq!(
            remove_end_clip.content_bounds(),
            motion_rect_from_ui_rect(center_from)
        );
        assert_eq!(
            remove_end_clip.visible_bounds().size.width,
            open_gpui_motion::motion_px(0.0)
        );
        assert_eq!(
            remove_end_clip.visible_bounds().size.height,
            open_gpui_motion::motion_px(center_from.size.height.as_f32())
        );
    }

    #[test]
    fn split_layout_transition_describes_collapse_and_expand() {
        let expanded = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("nav", 0.3)
                    .collapsible(true)
                    .collapsed_fraction(0.05)
                    .min_fraction(0.0),
                SplitterPanelDescriptor::new("main", 0.7).min_fraction(0.0),
            ],
        );
        let collapsed = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("nav", 0.3)
                    .collapsible(true)
                    .collapsed(true)
                    .collapsed_fraction(0.05)
                    .min_fraction(0.0),
                SplitterPanelDescriptor::new("main", 0.7).min_fraction(0.0),
            ],
        );
        let expanded_scene = SplitterLayoutScene::from_state(&expanded, rect(412.0, 200.0));
        let collapsed_scene = SplitterLayoutScene::from_state(&collapsed, rect(412.0, 200.0));

        let collapse = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Collapse,
            expanded_scene.clone(),
            collapsed_scene.clone(),
            MotionSpec::committed_layout(MotionPreference::Animated),
        );
        let nav_collapse = collapse.panel("nav").expect("nav transition should exist");
        assert_eq!(nav_collapse.kind(), SplitterPanelTransitionKind::Collapsing);
        assert!(!nav_collapse.collapsed_from());
        assert!(nav_collapse.collapsed_to());

        let expand = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Expand,
            collapsed_scene,
            expanded_scene,
            MotionSpec::committed_layout(MotionPreference::Animated),
        );
        let nav_expand = expand.panel("nav").expect("nav transition should exist");
        assert_eq!(nav_expand.kind(), SplitterPanelTransitionKind::Expanding);
        assert!(nav_expand.collapsed_from());
        assert!(!nav_expand.collapsed_to());
    }

    #[test]
    fn zero_fraction_collapse_uses_reveal_semantics_without_projection_panic() {
        let expanded = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("nav", 0.3)
                    .collapsible(true)
                    .min_fraction(0.0),
                SplitterPanelDescriptor::new("main", 0.7).min_fraction(0.0),
            ],
        );
        let collapsed = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("nav", 0.3)
                    .collapsible(true)
                    .collapsed(true)
                    .min_fraction(0.0),
                SplitterPanelDescriptor::new("main", 0.7).min_fraction(0.0),
            ],
        );
        let expanded_scene = SplitterLayoutScene::from_state(&expanded, rect(412.0, 200.0));
        let collapsed_scene = SplitterLayoutScene::from_state(&collapsed, rect(412.0, 200.0));
        let collapse = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Collapse,
            expanded_scene.clone(),
            collapsed_scene.clone(),
            MotionSpec::committed_layout(MotionPreference::Animated),
        );

        let mid = collapse
            .sample(0.5)
            .panel("nav")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("zero-size collapse should retain a reveal clip");
        assert!(mid.visible_bounds().size.width.as_f32() > 0.0);
        let end = collapse
            .sample(1.0)
            .panel("nav")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("zero-size collapse should finish with a reveal clip");
        assert_eq!(end.visible_bounds().size.width.as_f32(), 0.0);

        let expand = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Expand,
            collapsed_scene,
            expanded_scene,
            MotionSpec::committed_layout(MotionPreference::Animated),
        );
        let start = expand
            .sample(0.0)
            .panel("nav")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("zero-size expansion should retain a reveal clip");
        assert_eq!(start.visible_bounds().size.width.as_f32(), 0.0);
        let end = expand
            .sample(1.0)
            .panel("nav")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("zero-size expansion should finish with a reveal clip");
        assert!(end.visible_bounds().size.width.as_f32() > 0.0);
    }

    #[test]
    fn split_layout_transition_reduced_motion_preserves_final_scene() {
        let from_state = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.5).min_fraction(0.0),
                SplitterPanelDescriptor::new("right", 0.5).min_fraction(0.0),
            ],
        );
        let to_state = SplitterState::resolve(
            "workspace",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.25).min_fraction(0.0),
                SplitterPanelDescriptor::new("right", 0.75).min_fraction(0.0),
            ],
        );
        let from_scene = SplitterLayoutScene::from_state(&from_state, rect(400.0, 200.0));
        let to_scene = SplitterLayoutScene::from_state(&to_state, rect(400.0, 200.0));

        let transition = SplitterLayoutTransition::between(
            SplitterTransitionIntent::Resize,
            from_scene,
            to_scene.clone(),
            MotionSpec::committed_layout(MotionPreference::Reduced),
        );

        assert!(transition.is_immediate());
        assert_eq!(transition.to_scene(), &to_scene);
        assert_eq!(
            transition.panel("left").map(SplitterPanelTransition::kind),
            Some(SplitterPanelTransitionKind::Resizing)
        );
        let reduced_start = transition.sample(0.0);
        assert_eq!(reduced_start.progress(), 1.0);
        let left_to = transition
            .panel("left")
            .and_then(SplitterPanelTransition::to)
            .expect("resizing panel should have final bounds");
        let left_clip = reduced_start
            .panel("left")
            .and_then(SplitterPanelTransitionSample::clip)
            .expect("reduced-motion resize should still expose final clip geometry");
        assert_eq!(
            left_clip.content_bounds(),
            motion_rect_from_ui_rect(left_to)
        );
        assert_eq!(
            left_clip.visible_bounds(),
            motion_rect_from_ui_rect(left_to)
        );
    }
}
