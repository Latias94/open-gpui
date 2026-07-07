use crate::{
    DockCentralRegion, DockFloatingContainer, DockGraph, DockGraphValidationError, DockItemId,
    DockNode, DockNodeId, DockSpaceId, SplitAxis,
};
use std::collections::HashMap;

const DEFAULT_LEFT_RAIL_FRACTION: f32 = 0.26;
const DEFAULT_RIGHT_RAIL_FRACTION: f32 = 0.24;
const DEFAULT_BOTTOM_RAIL_FRACTION: f32 = 0.28;

/// Product-level target for placing a dock panel.
#[derive(Debug, Clone, PartialEq)]
pub enum DockPanelPlacementTarget {
    /// Place the panel in the central work area.
    Center,
    /// Place the panel in the left rail.
    LeftRail {
        /// Desired share of the horizontal work area.
        fraction: f32,
    },
    /// Place the panel in the right rail.
    RightRail {
        /// Desired share of the horizontal work area.
        fraction: f32,
    },
    /// Place the panel in the bottom rail.
    BottomRail {
        /// Desired share of the vertical dock area.
        fraction: f32,
    },
    /// Place the panel in the same tab stack as another item.
    Stack {
        /// Anchor item whose tab stack should receive the panel.
        anchor: DockItemId,
        /// Optional insertion index in the target tab stack.
        insert_index: Option<usize>,
    },
}

impl DockPanelPlacementTarget {
    /// Targets the central work area.
    pub const fn center() -> Self {
        Self::Center
    }

    /// Targets the left rail with the default fraction.
    pub const fn left_rail() -> Self {
        Self::LeftRail {
            fraction: DEFAULT_LEFT_RAIL_FRACTION,
        }
    }

    /// Targets the right rail with the default fraction.
    pub const fn right_rail() -> Self {
        Self::RightRail {
            fraction: DEFAULT_RIGHT_RAIL_FRACTION,
        }
    }

    /// Targets the bottom rail with the default fraction.
    pub const fn bottom_rail() -> Self {
        Self::BottomRail {
            fraction: DEFAULT_BOTTOM_RAIL_FRACTION,
        }
    }

    /// Targets the tab stack containing `anchor`.
    pub fn stacked_with(anchor: impl Into<DockItemId>) -> Self {
        Self::Stack {
            anchor: anchor.into(),
            insert_index: None,
        }
    }

    /// Sets the rail fraction when this target is a rail.
    pub fn fraction(self, fraction: f32) -> Self {
        match self {
            Self::LeftRail { .. } => Self::LeftRail {
                fraction: sanitize_fraction(fraction, DEFAULT_LEFT_RAIL_FRACTION),
            },
            Self::RightRail { .. } => Self::RightRail {
                fraction: sanitize_fraction(fraction, DEFAULT_RIGHT_RAIL_FRACTION),
            },
            Self::BottomRail { .. } => Self::BottomRail {
                fraction: sanitize_fraction(fraction, DEFAULT_BOTTOM_RAIL_FRACTION),
            },
            Self::Center | Self::Stack { .. } => self,
        }
    }

    /// Sets an insertion index when this target is a tab stack.
    pub fn insert_index(self, insert_index: usize) -> Self {
        match self {
            Self::Stack { anchor, .. } => Self::Stack {
                anchor,
                insert_index: Some(insert_index),
            },
            Self::Center
            | Self::LeftRail { .. }
            | Self::RightRail { .. }
            | Self::BottomRail { .. } => self,
        }
    }
}

/// Product-level placement for one dock panel item.
#[derive(Debug, Clone, PartialEq)]
pub struct DockPanelPlacement {
    item: DockItemId,
    target: DockPanelPlacementTarget,
    selected: bool,
    fallback: Option<DockPanelPlacementTarget>,
}

impl DockPanelPlacement {
    /// Creates a placement from an item and explicit target.
    pub fn new(item: impl Into<DockItemId>, target: DockPanelPlacementTarget) -> Self {
        Self {
            item: item.into(),
            target,
            selected: false,
            fallback: None,
        }
    }

    /// Places an item in the central work area.
    pub fn center(item: impl Into<DockItemId>) -> Self {
        Self::new(item, DockPanelPlacementTarget::center())
    }

    /// Places an item in the left rail.
    pub fn left_rail(item: impl Into<DockItemId>) -> Self {
        Self::new(item, DockPanelPlacementTarget::left_rail())
    }

    /// Places an item in the right rail.
    pub fn right_rail(item: impl Into<DockItemId>) -> Self {
        Self::new(item, DockPanelPlacementTarget::right_rail())
    }

    /// Places an item in the bottom rail.
    pub fn bottom_rail(item: impl Into<DockItemId>) -> Self {
        Self::new(item, DockPanelPlacementTarget::bottom_rail())
    }

    /// Places an item in the tab stack that contains `anchor`.
    pub fn stacked_with(item: impl Into<DockItemId>, anchor: impl Into<DockItemId>) -> Self {
        Self::new(item, DockPanelPlacementTarget::stacked_with(anchor))
    }

    /// Returns the panel item id.
    pub fn item(&self) -> &DockItemId {
        &self.item
    }

    /// Returns the requested target.
    pub fn target(&self) -> &DockPanelPlacementTarget {
        &self.target
    }

    /// Returns the fallback target, if one was provided.
    pub fn fallback_target(&self) -> Option<&DockPanelPlacementTarget> {
        self.fallback.as_ref()
    }

    /// Selects this item in its target tab stack after placement.
    pub fn selected(mut self) -> Self {
        self.selected = true;
        self
    }

    /// Sets the rail fraction when the placement target is a rail.
    pub fn fraction(mut self, fraction: f32) -> Self {
        self.target = self.target.fraction(fraction);
        self
    }

    /// Uses `fallback` if the primary placement target cannot be resolved.
    pub fn fallback(mut self, fallback: DockPanelPlacementTarget) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Sets an insertion index when the placement target is a tab stack.
    pub fn insert_index(mut self, insert_index: usize) -> Self {
        self.target = self.target.insert_index(insert_index);
        self
    }

    pub(crate) fn open_insert_index(
        &self,
        graph: &DockGraph,
        space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
    ) -> Option<usize> {
        let DockPanelPlacementTarget::Stack {
            anchor,
            insert_index,
        } = &self.target
        else {
            return None;
        };
        let (anchor_tabs, anchor_index) = graph.find_item_in_space(space, anchor)?;
        (Some(anchor_tabs) == target_tabs).then_some(insert_index.unwrap_or(anchor_index + 1))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PlacementBucket {
    Center,
    Left,
    Right,
    Bottom,
}

#[derive(Debug)]
struct PlacementStack {
    items: Vec<DockItemId>,
    selected: Option<DockItemId>,
    fraction: f32,
}

impl PlacementStack {
    fn new(fraction: f32) -> Self {
        Self {
            items: Vec::new(),
            selected: None,
            fraction,
        }
    }

    fn push(&mut self, placement: &DockPanelPlacement) {
        self.items.push(placement.item.clone());
        if placement.selected {
            self.selected = Some(placement.item.clone());
        }
    }
}

/// Convenience builder for programmatic dock layouts.
#[derive(Debug, Default)]
pub struct DockLayoutBuilder {
    graph: DockGraph,
}

impl DockLayoutBuilder {
    /// Creates an empty layout builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a tabs node.
    pub fn tabs(&mut self, items: impl IntoIterator<Item = impl Into<DockItemId>>) -> DockNodeId {
        let items: Vec<DockItemId> = items.into_iter().map(Into::into).collect();
        let selected = items.first().cloned();
        self.graph.insert_node(DockNode::Tabs { items, selected })
    }

    /// Inserts a tabs node with an explicit selected item.
    pub fn tabs_with_selected(
        &mut self,
        items: impl IntoIterator<Item = impl Into<DockItemId>>,
        selected: impl Into<DockItemId>,
    ) -> DockNodeId {
        let items: Vec<DockItemId> = items.into_iter().map(Into::into).collect();
        let selected = Some(selected.into());
        self.graph.insert_node(DockNode::Tabs { items, selected })
    }

    /// Inserts a split node with explicit children and fractions.
    pub fn split(
        &mut self,
        axis: SplitAxis,
        children: Vec<DockNodeId>,
        fractions: Vec<f32>,
    ) -> DockNodeId {
        self.graph.insert_node(DockNode::Split {
            axis,
            children,
            fractions,
        })
    }

    /// Inserts a horizontal two-child split.
    pub fn split_horizontal(
        &mut self,
        left: DockNodeId,
        right: DockNodeId,
        left_fraction: f32,
    ) -> DockNodeId {
        let left_fraction = left_fraction.clamp(0.0, 1.0);
        self.split(
            SplitAxis::Horizontal,
            vec![left, right],
            vec![left_fraction, 1.0 - left_fraction],
        )
    }

    /// Inserts a vertical two-child split.
    pub fn split_vertical(
        &mut self,
        top: DockNodeId,
        bottom: DockNodeId,
        top_fraction: f32,
    ) -> DockNodeId {
        let top_fraction = top_fraction.clamp(0.0, 1.0);
        self.split(
            SplitAxis::Vertical,
            vec![top, bottom],
            vec![top_fraction, 1.0 - top_fraction],
        )
    }

    /// Sets a root node for a dock space.
    pub fn set_root(&mut self, space: impl Into<DockSpaceId>, root: DockNodeId) {
        self.graph.set_root(space.into(), root);
    }

    /// Adds an in-window floating container.
    pub fn add_floating(
        &mut self,
        space: impl Into<DockSpaceId>,
        child: DockNodeId,
        bounds: open_gpui::Bounds<open_gpui::Pixels>,
    ) -> DockNodeId {
        let floating = self.graph.insert_node(DockNode::Floating { child });
        self.graph
            .floating_containers_mut(space.into())
            .push(DockFloatingContainer {
                node: floating,
                bounds,
            });
        floating
    }

    /// Sets central region semantics for a dock space.
    pub fn set_central_region(
        &mut self,
        space: impl Into<DockSpaceId>,
        central: DockCentralRegion,
    ) {
        self.graph.set_central_region(space, central);
    }

    /// Finishes the builder and returns a canonical graph without validation.
    pub fn build(mut self) -> DockGraph {
        self.simplify_graph();
        self.graph
    }

    /// Finishes the builder, validates reachable graph state, and returns a canonical graph.
    pub fn try_build(mut self) -> Result<DockGraph, DockGraphValidationError> {
        self.simplify_graph();
        self.graph.validate()?;
        Ok(self.graph)
    }

    /// Replaces the builder graph with product-level panel placements for `space`.
    pub fn set_panel_placements(
        &mut self,
        space: impl Into<DockSpaceId>,
        placements: impl IntoIterator<Item = DockPanelPlacement>,
    ) {
        self.graph = DockGraph::from_panel_placements(space, placements);
    }

    fn simplify_graph(&mut self) {
        for space in self.graph.spaces() {
            self.graph.simplify_space(&space);
        }
    }
}

/// Specification for a common editor-style default layout.
#[derive(Debug, Clone)]
pub struct EditorDockLayoutSpec {
    /// Items in the left tab stack.
    pub left_items: Vec<DockItemId>,
    /// Items in the main tab stack.
    pub main_items: Vec<DockItemId>,
    /// Items in the bottom tab stack.
    pub bottom_items: Vec<DockItemId>,
    /// Fraction allocated to the left stack.
    pub left_fraction: f32,
    /// Fraction allocated to the top stack within the right split.
    pub main_fraction: f32,
    /// Selected item in the left stack.
    pub selected_left: Option<DockItemId>,
    /// Selected item in the main stack.
    pub selected_main: Option<DockItemId>,
    /// Selected item in the bottom stack.
    pub selected_bottom: Option<DockItemId>,
}

impl EditorDockLayoutSpec {
    /// Creates an editor-style layout specification.
    pub fn new(
        left_items: impl IntoIterator<Item = impl Into<DockItemId>>,
        main_items: impl IntoIterator<Item = impl Into<DockItemId>>,
        bottom_items: impl IntoIterator<Item = impl Into<DockItemId>>,
    ) -> Self {
        Self {
            left_items: left_items.into_iter().map(Into::into).collect(),
            main_items: main_items.into_iter().map(Into::into).collect(),
            bottom_items: bottom_items.into_iter().map(Into::into).collect(),
            left_fraction: 0.26,
            main_fraction: 0.72,
            selected_left: None,
            selected_main: None,
            selected_bottom: None,
        }
    }

    /// Sets the primary split fractions.
    pub fn with_fractions(mut self, left_fraction: f32, main_fraction: f32) -> Self {
        self.left_fraction = left_fraction;
        self.main_fraction = main_fraction;
        self
    }

    /// Sets selected tab identities.
    pub fn with_selected_items(
        mut self,
        selected_left: impl Into<DockItemId>,
        selected_main: impl Into<DockItemId>,
        selected_bottom: impl Into<DockItemId>,
    ) -> Self {
        self.selected_left = Some(selected_left.into());
        self.selected_main = Some(selected_main.into());
        self.selected_bottom = Some(selected_bottom.into());
        self
    }
}

impl DockGraph {
    /// Builds a graph from product-level panel placements.
    pub fn from_panel_placements(
        space: impl Into<DockSpaceId>,
        placements: impl IntoIterator<Item = DockPanelPlacement>,
    ) -> Self {
        let space = space.into();
        let mut stacks = PlacementStacks::default();
        for placement in placements {
            stacks.push(placement);
        }

        let mut builder = DockLayoutBuilder::new();
        let center = build_stack_node(&mut builder, &stacks.center);
        let left = build_stack_node(&mut builder, &stacks.left);
        let right = build_stack_node(&mut builder, &stacks.right);
        let bottom = build_stack_node(&mut builder, &stacks.bottom);
        let work_area = build_work_area_node(&mut builder, left, center, right, &stacks);
        let root = match (work_area, bottom) {
            (Some(work_area), Some(bottom)) => {
                Some(builder.split_vertical(work_area, bottom, 1.0 - stacks.bottom.fraction))
            }
            (Some(work_area), None) => Some(work_area),
            (None, Some(bottom)) => Some(bottom),
            (None, None) => None,
        };

        if let Some(root) = root {
            builder.set_root(space.clone(), root);
        }
        if let Some(center) = center {
            builder.set_central_region(space.clone(), DockCentralRegion::with_node(center));
        } else {
            builder.set_central_region(space.clone(), DockCentralRegion::empty());
        }
        builder.build()
    }

    /// Resolves a product-level placement to an existing target tabs node.
    pub fn target_tabs_for_panel_placement(
        &self,
        space: &DockSpaceId,
        placement: &DockPanelPlacement,
    ) -> Option<DockNodeId> {
        self.target_tabs_for_panel_placement_target(space, placement.target())
            .or_else(|| {
                placement
                    .fallback_target()
                    .and_then(|target| self.target_tabs_for_panel_placement_target(space, target))
            })
            .or_else(|| {
                self.target_tabs_for_panel_placement_target(
                    space,
                    &DockPanelPlacementTarget::center(),
                )
            })
    }

    /// Resolves a product-level placement target to an existing target tabs node.
    pub fn target_tabs_for_panel_placement_target(
        &self,
        space: &DockSpaceId,
        target: &DockPanelPlacementTarget,
    ) -> Option<DockNodeId> {
        match target {
            DockPanelPlacementTarget::Center => center_tabs(self, space),
            DockPanelPlacementTarget::LeftRail { .. } => {
                rail_tabs(self, space, PlacementBucket::Left)
            }
            DockPanelPlacementTarget::RightRail { .. } => {
                rail_tabs(self, space, PlacementBucket::Right)
            }
            DockPanelPlacementTarget::BottomRail { .. } => {
                rail_tabs(self, space, PlacementBucket::Bottom)
            }
            DockPanelPlacementTarget::Stack { anchor, .. } => {
                self.find_item_in_space(space, anchor).map(|(tabs, _)| tabs)
            }
        }
    }

    /// Infers the current product-level placement for a dock item.
    pub fn panel_placement_for_item(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Option<DockPanelPlacement> {
        let (tabs, index) = self.find_item_in_space(space, item)?;
        let target = panel_placement_target_for_tabs(self, space, tabs, index)?;
        Some(DockPanelPlacement::new(item.clone(), target))
    }

    /// Builds a common editor-style default layout.
    pub fn default_editor_layout(
        space: impl Into<DockSpaceId>,
        spec: EditorDockLayoutSpec,
    ) -> Self {
        let space = space.into();
        let mut builder = DockLayoutBuilder::new();
        let left = builder.editor_tabs(spec.left_items, spec.selected_left);
        let main = builder.editor_tabs(spec.main_items, spec.selected_main);
        let bottom = builder.editor_tabs(spec.bottom_items, spec.selected_bottom);
        let right = builder.split_vertical(main, bottom, spec.main_fraction);
        let root = builder.split_horizontal(left, right, spec.left_fraction);
        builder.set_root(space.clone(), root);
        builder.set_central_region(space, DockCentralRegion::with_node(main));
        builder.build()
    }
}

impl DockLayoutBuilder {
    fn editor_tabs(&mut self, items: Vec<DockItemId>, selected: Option<DockItemId>) -> DockNodeId {
        if let Some(selected) = selected {
            return self.tabs_with_selected(items, selected);
        }
        self.tabs(items)
    }
}

struct PlacementStacks {
    center: PlacementStack,
    left: PlacementStack,
    right: PlacementStack,
    bottom: PlacementStack,
    item_buckets: HashMap<DockItemId, PlacementBucket>,
}

impl Default for PlacementStacks {
    fn default() -> Self {
        Self {
            center: PlacementStack::new(0.0),
            left: PlacementStack::new(DEFAULT_LEFT_RAIL_FRACTION),
            right: PlacementStack::new(DEFAULT_RIGHT_RAIL_FRACTION),
            bottom: PlacementStack::new(DEFAULT_BOTTOM_RAIL_FRACTION),
            item_buckets: HashMap::new(),
        }
    }
}

impl PlacementStacks {
    fn push(&mut self, placement: DockPanelPlacement) {
        let resolved_target = self
            .bucket_for_target(&placement.target)
            .map(|bucket| (&placement.target, bucket))
            .or_else(|| {
                placement.fallback.as_ref().and_then(|target| {
                    self.bucket_for_target(target)
                        .map(|bucket| (target, bucket))
                })
            });
        let (target, bucket) =
            resolved_target.unwrap_or((&DockPanelPlacementTarget::Center, PlacementBucket::Center));

        match (target, bucket) {
            (DockPanelPlacementTarget::LeftRail { fraction }, PlacementBucket::Left) => {
                self.left.fraction = sanitize_fraction(*fraction, DEFAULT_LEFT_RAIL_FRACTION);
            }
            (DockPanelPlacementTarget::RightRail { fraction }, PlacementBucket::Right) => {
                self.right.fraction = sanitize_fraction(*fraction, DEFAULT_RIGHT_RAIL_FRACTION);
            }
            (DockPanelPlacementTarget::BottomRail { fraction }, PlacementBucket::Bottom) => {
                self.bottom.fraction = sanitize_fraction(*fraction, DEFAULT_BOTTOM_RAIL_FRACTION);
            }
            _ => {}
        }

        self.stack_mut(bucket).push(&placement);
        self.item_buckets.insert(placement.item.clone(), bucket);
    }

    fn bucket_for_target(&self, target: &DockPanelPlacementTarget) -> Option<PlacementBucket> {
        match target {
            DockPanelPlacementTarget::Center => Some(PlacementBucket::Center),
            DockPanelPlacementTarget::LeftRail { .. } => Some(PlacementBucket::Left),
            DockPanelPlacementTarget::RightRail { .. } => Some(PlacementBucket::Right),
            DockPanelPlacementTarget::BottomRail { .. } => Some(PlacementBucket::Bottom),
            DockPanelPlacementTarget::Stack { anchor, .. } => {
                self.item_buckets.get(anchor).copied()
            }
        }
    }

    fn stack_mut(&mut self, bucket: PlacementBucket) -> &mut PlacementStack {
        match bucket {
            PlacementBucket::Center => &mut self.center,
            PlacementBucket::Left => &mut self.left,
            PlacementBucket::Right => &mut self.right,
            PlacementBucket::Bottom => &mut self.bottom,
        }
    }
}

fn build_stack_node(builder: &mut DockLayoutBuilder, stack: &PlacementStack) -> Option<DockNodeId> {
    if stack.items.is_empty() {
        return None;
    }

    Some(if let Some(selected) = stack.selected.clone() {
        builder.tabs_with_selected(stack.items.clone(), selected)
    } else {
        builder.tabs(stack.items.clone())
    })
}

fn build_work_area_node(
    builder: &mut DockLayoutBuilder,
    left: Option<DockNodeId>,
    center: Option<DockNodeId>,
    right: Option<DockNodeId>,
    stacks: &PlacementStacks,
) -> Option<DockNodeId> {
    let mut children = Vec::new();
    let mut fractions = Vec::new();
    if let Some(left) = left {
        children.push(left);
        fractions.push(stacks.left.fraction);
    }
    if let Some(center) = center {
        children.push(center);
        let side_total = left.map(|_| stacks.left.fraction).unwrap_or(0.0)
            + right.map(|_| stacks.right.fraction).unwrap_or(0.0);
        fractions.push((1.0 - side_total).max(0.0));
    }
    if let Some(right) = right {
        children.push(right);
        fractions.push(stacks.right.fraction);
    }

    match children.len() {
        0 => None,
        1 => children.into_iter().next(),
        _ => {
            if center.is_none() {
                let share = 1.0 / children.len() as f32;
                fractions.fill(share);
            }
            Some(builder.split(SplitAxis::Horizontal, children, fractions))
        }
    }
}

fn panel_placement_target_for_tabs(
    graph: &DockGraph,
    space: &DockSpaceId,
    tabs: DockNodeId,
    item_index: usize,
) -> Option<DockPanelPlacementTarget> {
    if let Some(target) = stack_target_for_item(graph, tabs, item_index) {
        return Some(target);
    }

    rail_target_for_tabs(graph, space, tabs).or_else(|| {
        (center_tabs(graph, space) == Some(tabs)).then_some(DockPanelPlacementTarget::Center)
    })
}

fn stack_target_for_item(
    graph: &DockGraph,
    tabs: DockNodeId,
    item_index: usize,
) -> Option<DockPanelPlacementTarget> {
    let Some(DockNode::Tabs { items, .. }) = graph.node(tabs) else {
        return None;
    };
    if items.len() <= 1 {
        return None;
    }
    let anchor_index = if item_index > 0 { item_index - 1 } else { 1 };
    items
        .get(anchor_index)
        .cloned()
        .map(|anchor| DockPanelPlacementTarget::stacked_with(anchor).insert_index(item_index))
}

fn rail_target_for_tabs(
    graph: &DockGraph,
    space: &DockSpaceId,
    tabs: DockNodeId,
) -> Option<DockPanelPlacementTarget> {
    bottom_target_for_tabs(graph, space, tabs)
        .or_else(|| horizontal_rail_target_for_tabs(graph, space, tabs))
}

fn bottom_target_for_tabs(
    graph: &DockGraph,
    space: &DockSpaceId,
    tabs: DockNodeId,
) -> Option<DockPanelPlacementTarget> {
    let root = graph.root(space)?;
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph.node(root)?
    else {
        return None;
    };
    if *axis != SplitAxis::Vertical || children.len() < 2 {
        return None;
    }
    let bottom_index = children.len() - 1;
    subtree_contains_node(graph, children[bottom_index], tabs).then(|| {
        DockPanelPlacementTarget::bottom_rail().fraction(
            *fractions
                .get(bottom_index)
                .unwrap_or(&DEFAULT_BOTTOM_RAIL_FRACTION),
        )
    })
}

fn horizontal_rail_target_for_tabs(
    graph: &DockGraph,
    space: &DockSpaceId,
    tabs: DockNodeId,
) -> Option<DockPanelPlacementTarget> {
    let root = graph.root(space)?;
    let work_area = work_area_root(graph, root);
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph.node(work_area)?
    else {
        return None;
    };
    if *axis != SplitAxis::Horizontal || children.len() < 2 {
        return None;
    }
    if subtree_contains_node(graph, children[0], tabs) {
        return Some(
            DockPanelPlacementTarget::left_rail()
                .fraction(*fractions.first().unwrap_or(&DEFAULT_LEFT_RAIL_FRACTION)),
        );
    }
    let right_index = children.len() - 1;
    subtree_contains_node(graph, children[right_index], tabs).then(|| {
        DockPanelPlacementTarget::right_rail().fraction(
            *fractions
                .get(right_index)
                .unwrap_or(&DEFAULT_RIGHT_RAIL_FRACTION),
        )
    })
}

fn sanitize_fraction(fraction: f32, fallback: f32) -> f32 {
    if fraction.is_finite() && fraction > 0.0 && fraction < 1.0 {
        fraction
    } else {
        fallback
    }
}

fn center_tabs(graph: &DockGraph, space: &DockSpaceId) -> Option<DockNodeId> {
    graph
        .central_region(space)
        .and_then(|central| central.node)
        .and_then(|node| first_tabs_in_subtree(graph, node))
        .or_else(|| {
            graph
                .root(space)
                .and_then(|root| first_tabs_in_subtree(graph, root))
        })
}

fn rail_tabs(
    graph: &DockGraph,
    space: &DockSpaceId,
    bucket: PlacementBucket,
) -> Option<DockNodeId> {
    let root = graph.root(space)?;
    match bucket {
        PlacementBucket::Bottom => bottom_tabs(graph, root),
        PlacementBucket::Left | PlacementBucket::Right => {
            horizontal_rail_tabs(graph, work_area_root(graph, root), space, bucket)
        }
        PlacementBucket::Center => center_tabs(graph, space),
    }
}

fn work_area_root(graph: &DockGraph, root: DockNodeId) -> DockNodeId {
    let Some(DockNode::Split {
        axis: SplitAxis::Vertical,
        children,
        ..
    }) = graph.node(root)
    else {
        return root;
    };
    children.first().copied().unwrap_or(root)
}

fn bottom_tabs(graph: &DockGraph, root: DockNodeId) -> Option<DockNodeId> {
    let DockNode::Split {
        axis: SplitAxis::Vertical,
        children,
        ..
    } = graph.node(root)?
    else {
        return None;
    };
    children
        .last()
        .copied()
        .and_then(|child| first_tabs_in_subtree(graph, child))
}

fn horizontal_rail_tabs(
    graph: &DockGraph,
    work_area: DockNodeId,
    space: &DockSpaceId,
    bucket: PlacementBucket,
) -> Option<DockNodeId> {
    let DockNode::Split {
        axis: SplitAxis::Horizontal,
        children,
        ..
    } = graph.node(work_area)?
    else {
        return None;
    };

    let central = graph.central_region(space).and_then(|central| central.node);
    let central_index = central.and_then(|central| {
        children
            .iter()
            .position(|child| subtree_contains_node(graph, *child, central))
    });

    match (bucket, central_index) {
        (PlacementBucket::Left, Some(index)) => children[..index]
            .iter()
            .rev()
            .find_map(|child| first_tabs_in_subtree(graph, *child)),
        (PlacementBucket::Right, Some(index)) => children[index + 1..]
            .iter()
            .find_map(|child| first_tabs_in_subtree(graph, *child)),
        (PlacementBucket::Left, None) => children
            .first()
            .and_then(|child| first_tabs_in_subtree(graph, *child)),
        (PlacementBucket::Right, None) => children
            .last()
            .and_then(|child| first_tabs_in_subtree(graph, *child)),
        (PlacementBucket::Center | PlacementBucket::Bottom, _) => None,
    }
}

fn first_tabs_in_subtree(graph: &DockGraph, root: DockNodeId) -> Option<DockNodeId> {
    match graph.node(root)? {
        DockNode::Tabs { .. } => Some(root),
        DockNode::Floating { child } => first_tabs_in_subtree(graph, *child),
        DockNode::Split { children, .. } => children
            .iter()
            .find_map(|child| first_tabs_in_subtree(graph, *child)),
    }
}

fn subtree_contains_node(graph: &DockGraph, root: DockNodeId, target: DockNodeId) -> bool {
    if root == target {
        return true;
    }
    match graph.node(root) {
        Some(DockNode::Tabs { .. }) | None => false,
        Some(DockNode::Floating { child }) => subtree_contains_node(graph, *child, target),
        Some(DockNode::Split { children, .. }) => children
            .iter()
            .any(|child| subtree_contains_node(graph, *child, target)),
    }
}
