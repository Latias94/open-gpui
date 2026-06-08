use crate::{
    DockFloatingContainer, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId, SplitAxis,
    dock_bounds,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[path = "layout_validation.rs"]
mod layout_validation;
pub use layout_validation::DockLayoutValidationError;

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

    /// Returns true when all coordinates are finite and the size is non-negative.
    pub fn is_finite_with_non_negative_size(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 0.0
            && self.height >= 0.0
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
