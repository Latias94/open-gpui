//! Renderer-neutral split layout primitives.

use crate::{Orientation, Size, UiPx, UiRect, ui_point, ui_px, ui_rect, ui_size};

const EPSILON: f32 = 0.000_1;

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
        let reserved_axis = if reserves_handle_space {
            handle_hit * handle_count as f32
        } else {
            0.0
        };
        let panel_axis = (axis_length - reserved_axis).max(0.0);
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
        for horizontal in self
            .handles
            .iter()
            .filter(|handle| matches!(handle.orientation, Orientation::Horizontal))
        {
            for vertical in self
                .handles
                .iter()
                .filter(|handle| matches!(handle.orientation, Orientation::Vertical))
            {
                if let Some(bounds) = orthogonal_handle_junction(horizontal.bounds, vertical.bounds)
                {
                    self.junctions.push(SplitterJunctionHitRegion {
                        bounds,
                        horizontal: horizontal.clone(),
                        vertical: vertical.clone(),
                    });
                }
            }
        }
    }
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

    fn rect(width: f32, height: f32) -> UiRect {
        ui_rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(width), ui_px(height)),
        )
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
}
