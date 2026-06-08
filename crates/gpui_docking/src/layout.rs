use crate::{
    DockFloatingContainer, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId, SplitAxis,
    dock_bounds,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;

/// Current docking layout serialization version.
pub const DOCK_LAYOUT_VERSION: u32 = 1;

/// Serializable dock layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayout {
    /// Layout schema version.
    pub layout_version: u32,
    /// Serialized logical dock spaces.
    pub spaces: Vec<DockLayoutSpace>,
    /// Serialized dock nodes.
    pub nodes: Vec<DockLayoutNode>,
}

impl DockLayout {
    /// Creates a layout with the current schema version.
    pub fn new(spaces: Vec<DockLayoutSpace>, nodes: Vec<DockLayoutNode>) -> Self {
        Self {
            layout_version: DOCK_LAYOUT_VERSION,
            spaces,
            nodes,
        }
    }

    /// Validates graph-level layout invariants before import.
    pub fn validate(&self) -> Result<(), DockLayoutValidationError> {
        if self.layout_version != DOCK_LAYOUT_VERSION {
            return Err(DockLayoutValidationError::UnsupportedVersion {
                expected: DOCK_LAYOUT_VERSION,
                found: self.layout_version,
            });
        }

        let mut space_ids = HashSet::new();
        for space in &self.spaces {
            if !space_ids.insert(space.id.clone()) {
                return Err(DockLayoutValidationError::DuplicateSpaceId {
                    space: space.id.clone(),
                });
            }
        }

        let mut by_id = HashMap::new();
        for node in &self.nodes {
            let id = node.id();
            if by_id.insert(id, node).is_some() {
                return Err(DockLayoutValidationError::DuplicateNodeId { id });
            }
        }

        let mut item_nodes = HashMap::new();
        for node in &self.nodes {
            if let DockLayoutNode::Tabs { id, items, .. } = node {
                for item in items {
                    if let Some(first_node) = item_nodes.insert(item.clone(), *id) {
                        return Err(DockLayoutValidationError::DuplicateItemId {
                            item: item.clone(),
                            first_node,
                            duplicate_node: *id,
                        });
                    }
                }
            }
        }

        for (id, node) in &by_id {
            match node {
                DockLayoutNode::Tabs { items, active, .. } => {
                    if items.is_empty() {
                        return Err(DockLayoutValidationError::EmptyTabs { id: *id });
                    }
                    if *active >= items.len() {
                        return Err(DockLayoutValidationError::TabsActiveOutOfBounds {
                            id: *id,
                            active: *active,
                            len: items.len(),
                        });
                    }
                }
                DockLayoutNode::Split {
                    children,
                    fractions,
                    ..
                } => {
                    if children.is_empty() {
                        return Err(DockLayoutValidationError::EmptySplitChildren { id: *id });
                    }
                    if children.len() != fractions.len() {
                        return Err(DockLayoutValidationError::SplitFractionsLenMismatch {
                            id: *id,
                            children_len: children.len(),
                            fractions_len: fractions.len(),
                        });
                    }
                    for (index, value) in fractions.iter().copied().enumerate() {
                        if !value.is_finite() {
                            return Err(DockLayoutValidationError::SplitNonFiniteFraction {
                                id: *id,
                                index,
                                value,
                            });
                        }
                        if value < 0.0 {
                            return Err(DockLayoutValidationError::SplitNegativeFraction {
                                id: *id,
                                index,
                                value,
                            });
                        }
                    }
                }
            }
        }

        for node in by_id.values() {
            if let DockLayoutNode::Split { children, .. } = node {
                for child in children {
                    if !by_id.contains_key(child) {
                        return Err(DockLayoutValidationError::MissingNodeId { id: *child });
                    }
                }
            }
        }

        detect_cycles(&by_id)?;

        for space in &self.spaces {
            if let Some(root) = space.root
                && !by_id.contains_key(&root)
            {
                return Err(DockLayoutValidationError::SpaceRootMissing {
                    space: space.id.clone(),
                    root,
                });
            }
            for floating in &space.floatings {
                if !by_id.contains_key(&floating.root) {
                    return Err(DockLayoutValidationError::FloatingRootMissing {
                        space: space.id.clone(),
                        root: floating.root,
                    });
                }
            }
        }

        validate_forest_ownership(&by_id, &self.spaces)?;

        Ok(())
    }
}

/// Serializable layout for one logical dock space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayoutSpace {
    /// Logical dock space id.
    pub id: DockSpaceId,
    /// Optional root node id in [`DockLayout::nodes`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<u32>,
    /// In-window floating containers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub floatings: Vec<DockLayoutFloatingContainer>,
}

/// Serializable in-window floating container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayoutFloatingContainer {
    /// Root node id for the floating container contents.
    pub root: u32,
    /// Floating container bounds.
    pub bounds: DockLayoutRect,
}

/// Serializable rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DockLayoutRect {
    /// Left coordinate.
    pub x: f32,
    /// Top coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl DockLayoutRect {
    /// Converts GPUI bounds into a serializable rectangle.
    pub fn from_bounds(bounds: open_gpui::Bounds<open_gpui::Pixels>) -> Self {
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }

    /// Converts this rectangle into GPUI bounds.
    pub fn to_bounds(self) -> open_gpui::Bounds<open_gpui::Pixels> {
        dock_bounds(self.x, self.y, self.width, self.height)
    }
}

/// Serializable dock node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum DockLayoutNode {
    /// Serializable split node.
    #[serde(rename = "split")]
    Split {
        /// Stable layout node id.
        id: u32,
        /// Split axis.
        axis: SplitAxis,
        /// Child layout node ids.
        children: Vec<u32>,
        /// Normalized split fractions.
        fractions: Vec<f32>,
    },
    /// Serializable tabs node.
    #[serde(rename = "tabs")]
    Tabs {
        /// Stable layout node id.
        id: u32,
        /// Dock items in tab order.
        items: Vec<DockItemId>,
        /// Active item index.
        active: usize,
    },
}

impl DockLayoutNode {
    /// Returns the layout node id.
    pub fn id(&self) -> u32 {
        match self {
            DockLayoutNode::Split { id, .. } | DockLayoutNode::Tabs { id, .. } => *id,
        }
    }
}

/// Validation error for serialized dock layouts.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DockLayoutValidationError {
    /// The layout version is unsupported.
    #[error("unsupported dock layout version: expected {expected}, found {found}")]
    UnsupportedVersion {
        /// Expected version.
        expected: u32,
        /// Found version.
        found: u32,
    },
    /// A dock space id appears more than once.
    #[error("duplicate dock layout space id: {space}")]
    DuplicateSpaceId {
        /// Duplicate dock space id.
        space: DockSpaceId,
    },
    /// A layout node id appears more than once.
    #[error("duplicate dock layout node id: {id}")]
    DuplicateNodeId {
        /// Duplicate id.
        id: u32,
    },
    /// A dock item id appears in more than one serialized tab position.
    #[error(
        "duplicate dock layout item id {item}: first seen in node {first_node}, duplicated in node {duplicate_node}"
    )]
    DuplicateItemId {
        /// Duplicate dock item id.
        item: DockItemId,
        /// First tabs node containing the item.
        first_node: u32,
        /// Tabs node containing the duplicate item.
        duplicate_node: u32,
    },
    /// A split references a missing node id.
    #[error("missing dock layout node id: {id}")]
    MissingNodeId {
        /// Missing id.
        id: u32,
    },
    /// The layout graph contains a cycle.
    #[error("dock layout cycle detected at node {id}")]
    CycleDetected {
        /// Cyclic id.
        id: u32,
    },
    /// A tabs node has no items.
    #[error("tabs node {id} is empty")]
    EmptyTabs {
        /// Invalid node id.
        id: u32,
    },
    /// A tabs node has an invalid active index.
    #[error("tabs node {id} active index {active} out of bounds for length {len}")]
    TabsActiveOutOfBounds {
        /// Invalid node id.
        id: u32,
        /// Active index.
        active: usize,
        /// Item count.
        len: usize,
    },
    /// A split node has no children.
    #[error("split node {id} has no children")]
    EmptySplitChildren {
        /// Invalid node id.
        id: u32,
    },
    /// A split node has mismatched child and fraction counts.
    #[error("split node {id} has {children_len} children and {fractions_len} fractions")]
    SplitFractionsLenMismatch {
        /// Invalid node id.
        id: u32,
        /// Child count.
        children_len: usize,
        /// Fraction count.
        fractions_len: usize,
    },
    /// A split fraction is not finite.
    #[error("split node {id} fraction {index} is non-finite: {value}")]
    SplitNonFiniteFraction {
        /// Invalid node id.
        id: u32,
        /// Fraction index.
        index: usize,
        /// Invalid value.
        value: f32,
    },
    /// A split fraction is negative.
    #[error("split node {id} fraction {index} is negative: {value}")]
    SplitNegativeFraction {
        /// Invalid node id.
        id: u32,
        /// Fraction index.
        index: usize,
        /// Invalid value.
        value: f32,
    },
    /// A dock space references a missing root node.
    #[error("dock space {space} references missing root node {root}")]
    SpaceRootMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing root id.
        root: u32,
    },
    /// A floating container references a missing root node.
    #[error("dock space {space} references missing floating root node {root}")]
    FloatingRootMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing root id.
        root: u32,
    },
    /// A layout node is reachable from more than one parent/root.
    #[error("dock layout node {id} is referenced more than once")]
    DuplicateNodeReference {
        /// Shared node id.
        id: u32,
    },
    /// A layout node is not reachable from any dock space root or floating root.
    #[error("dock layout node {id} is not reachable from any dock space")]
    UnreachableNodeId {
        /// Unreachable node id.
        id: u32,
    },
}

impl DockGraph {
    /// Exports every dock space in this graph into a serializable layout.
    pub fn export_layout(&self) -> DockLayout {
        let mut exporter = LayoutExporter::default();
        let mut spaces = Vec::new();

        for space in self.spaces() {
            let root = self
                .root(&space)
                .map(|root| exporter.export_subtree(self, root));
            let floatings = self
                .floating_containers(&space)
                .iter()
                .filter_map(|floating| {
                    floating_child(self, floating.node).map(|child| DockLayoutFloatingContainer {
                        root: exporter.export_subtree(self, child),
                        bounds: DockLayoutRect::from_bounds(floating.bounds),
                    })
                })
                .collect();

            spaces.push(DockLayoutSpace {
                id: space,
                root,
                floatings,
            });
        }

        DockLayout::new(spaces, exporter.nodes)
    }

    /// Imports a validated layout into a new graph.
    pub fn import_layout(layout: &DockLayout) -> Result<Self, DockLayoutValidationError> {
        layout.validate()?;

        let by_id: BTreeMap<u32, &DockLayoutNode> =
            layout.nodes.iter().map(|node| (node.id(), node)).collect();
        let mut importer = LayoutImporter {
            by_id,
            graph: DockGraph::new(),
            built: HashMap::new(),
        };

        for space in &layout.spaces {
            if let Some(root) = space.root {
                let root = importer.build_node(root);
                importer.graph.set_root(space.id.clone(), root);
            }

            for floating in &space.floatings {
                let child = importer.build_node(floating.root);
                let floating_node = importer.graph.insert_node(DockNode::Floating { child });
                importer
                    .graph
                    .floating_containers_mut(space.id.clone())
                    .push(DockFloatingContainer {
                        node: floating_node,
                        bounds: floating.bounds.to_bounds(),
                    });
            }
        }

        let mut graph = importer.graph;
        for space in graph.spaces() {
            graph.simplify_space(&space);
        }
        Ok(graph)
    }
}

#[derive(Default)]
struct LayoutExporter {
    next_id: u32,
    ids: HashMap<DockNodeId, u32>,
    nodes: Vec<DockLayoutNode>,
}

impl LayoutExporter {
    fn export_subtree(&mut self, graph: &DockGraph, node: DockNodeId) -> u32 {
        if let Some(id) = self.ids.get(&node).copied() {
            return id;
        }

        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        self.ids.insert(node, id);

        let Some(graph_node) = graph.node(node) else {
            return id;
        };

        let layout_node = match graph_node {
            DockNode::Tabs { items, active } => DockLayoutNode::Tabs {
                id,
                items: items.clone(),
                active: *active,
            },
            DockNode::Floating { child } => return self.export_subtree(graph, *child),
            DockNode::Split {
                axis,
                children,
                fractions,
            } => DockLayoutNode::Split {
                id,
                axis: *axis,
                children: children
                    .iter()
                    .copied()
                    .map(|child| self.export_subtree(graph, child))
                    .collect(),
                fractions: fractions.clone(),
            },
        };
        self.nodes.push(layout_node);
        id
    }
}

struct LayoutImporter<'a> {
    by_id: BTreeMap<u32, &'a DockLayoutNode>,
    graph: DockGraph,
    built: HashMap<u32, DockNodeId>,
}

impl LayoutImporter<'_> {
    fn build_node(&mut self, id: u32) -> DockNodeId {
        if let Some(node) = self.built.get(&id).copied() {
            return node;
        }

        let layout_node = self
            .by_id
            .get(&id)
            .expect("layout must be validated before import");
        let node = match layout_node {
            DockLayoutNode::Tabs { items, active, .. } => DockNode::Tabs {
                items: items.clone(),
                active: *active,
            },
            DockLayoutNode::Split {
                axis,
                children,
                fractions,
                ..
            } => {
                let children = children
                    .iter()
                    .copied()
                    .map(|child| self.build_node(child))
                    .collect();
                DockNode::Split {
                    axis: *axis,
                    children,
                    fractions: fractions.clone(),
                }
            }
        };
        let node = self.graph.insert_node(node);
        self.built.insert(id, node);
        node
    }
}

fn floating_child(graph: &DockGraph, node: DockNodeId) -> Option<DockNodeId> {
    match graph.node(node)? {
        DockNode::Floating { child } => Some(*child),
        _ => Some(node),
    }
}

fn detect_cycles(by_id: &HashMap<u32, &DockLayoutNode>) -> Result<(), DockLayoutValidationError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Done,
    }

    #[derive(Clone, Copy)]
    enum Step {
        Enter(u32),
        Exit(u32),
    }

    let mut marks = HashMap::new();
    for start in by_id.keys().copied() {
        if marks.contains_key(&start) {
            continue;
        }

        let mut stack = vec![Step::Enter(start)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter(id) => {
                    if marks.get(&id) == Some(&Mark::Done) {
                        continue;
                    }
                    if marks.get(&id) == Some(&Mark::Visiting) {
                        return Err(DockLayoutValidationError::CycleDetected { id });
                    }

                    marks.insert(id, Mark::Visiting);
                    stack.push(Step::Exit(id));

                    if let Some(DockLayoutNode::Split { children, .. }) = by_id.get(&id) {
                        for child in children.iter().rev().copied() {
                            stack.push(Step::Enter(child));
                        }
                    }
                }
                Step::Exit(id) => {
                    marks.insert(id, Mark::Done);
                }
            }
        }
    }

    Ok(())
}

fn validate_forest_ownership(
    by_id: &HashMap<u32, &DockLayoutNode>,
    spaces: &[DockLayoutSpace],
) -> Result<(), DockLayoutValidationError> {
    let mut seen = HashSet::new();
    for root in spaces.iter().filter_map(|space| space.root).chain(
        spaces
            .iter()
            .flat_map(|space| space.floatings.iter().map(|floating| floating.root)),
    ) {
        mark_reachable_once(root, by_id, &mut seen)?;
    }

    for id in by_id.keys().copied() {
        if !seen.contains(&id) {
            return Err(DockLayoutValidationError::UnreachableNodeId { id });
        }
    }

    Ok(())
}

fn mark_reachable_once(
    root: u32,
    by_id: &HashMap<u32, &DockLayoutNode>,
    seen: &mut HashSet<u32>,
) -> Result<(), DockLayoutValidationError> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            return Err(DockLayoutValidationError::DuplicateNodeReference { id });
        }

        if let Some(DockLayoutNode::Split { children, .. }) = by_id.get(&id) {
            stack.extend(children.iter().rev().copied());
        }
    }

    Ok(())
}
