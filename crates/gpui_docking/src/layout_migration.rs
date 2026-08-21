#![allow(dead_code)]

use crate::graph_assembly::{
    AssemblyNodeId, CanonicalDockGraph, CanonicalGraphError, CanonicalNode, CanonicalSizeRef,
    DockGraphAssembly,
};
use crate::model::DockLayoutNode;
use crate::{
    CanonicalGraphAuthority, CanonicalNodeId, DockFloatingId, DockGroupId, DockItemId, DockLayout,
    DockLayoutRect, DockSpaceId, SplitAxis,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use thiserror::Error;

pub(crate) const CANONICAL_LAYOUT_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanonicalLayoutV3 {
    pub(crate) layout_version: u32,
    pub(crate) spaces: Vec<CanonicalLayoutSpaceV3>,
    pub(crate) nodes: Vec<CanonicalLayoutNodeV3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanonicalLayoutSpaceV3 {
    pub(crate) id: DockSpaceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) root: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) floating_roots: Vec<CanonicalLayoutFloatingV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) central: Option<CanonicalLayoutCentralV3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanonicalLayoutCentralV3 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) node: Option<u32>,
    #[serde(default = "default_true")]
    pub(crate) keep_alive_when_empty: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) passthrough_when_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanonicalLayoutFloatingV3 {
    pub(crate) id: String,
    pub(crate) root: u32,
    pub(crate) bounds: DockLayoutRect,
    pub(crate) z_order: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanonicalLayoutSizeRefV3 {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl CanonicalLayoutSizeRefV3 {
    fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }
}

impl From<CanonicalLayoutSizeRefV3> for CanonicalSizeRef {
    fn from(value: CanonicalLayoutSizeRefV3) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

impl From<CanonicalSizeRef> for CanonicalLayoutSizeRefV3 {
    fn from(value: CanonicalSizeRef) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CanonicalLayoutNodeV3 {
    Branch {
        id: u32,
        axis: SplitAxis,
        first: u32,
        second: u32,
        /// Inclusive ratio of the first child. Zero-share panes are representable.
        ratio: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_ref: Option<CanonicalLayoutSizeRefV3>,
    },
    Group {
        id: u32,
        group_id: String,
        items: Vec<DockItemId>,
        selected: DockItemId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_ref: Option<CanonicalLayoutSizeRefV3>,
    },
}

impl CanonicalLayoutNodeV3 {
    fn id(&self) -> u32 {
        match self {
            Self::Branch { id, .. } | Self::Group { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum LayoutMigrationError {
    #[error(transparent)]
    Legacy(#[from] crate::DockLayoutValidationError),
    #[error(transparent)]
    Graph(#[from] CanonicalGraphError),
    #[error("unsupported docking layout version {actual}")]
    UnsupportedVersion { actual: u32 },
    #[error("layout v3 declares dock space {space} more than once")]
    DuplicateSpace { space: DockSpaceId },
    #[error("layout v3 has an empty dock space id")]
    EmptySpaceId,
    #[error("layout v3 has duplicate node id {id}")]
    DuplicateNode { id: u32 },
    #[error("layout v3 has duplicate group id {id}")]
    DuplicateGroupId { id: String },
    #[error("layout v3 group {id} has an empty durable id")]
    EmptyGroupId { id: u32 },
    #[error("layout v3 has duplicate floating id {id}")]
    DuplicateFloating { id: String },
    #[error("layout v3 space {space} has duplicate floating z-order {z_order}")]
    DuplicateFloatingZOrder { space: DockSpaceId, z_order: i32 },
    #[error("layout v3 has an empty floating id in space {space}")]
    EmptyFloatingId { space: DockSpaceId },
    #[error("layout v3 has duplicate item {item}")]
    DuplicateItem { item: DockItemId },
    #[error("layout v3 group {id} is empty")]
    EmptyGroup { id: u32 },
    #[error("layout v3 group {id} selects an item that is not present")]
    SelectedItemMissing { id: u32 },
    #[error("layout v3 branch {id} has an invalid ratio {ratio}")]
    InvalidRatio { id: u32, ratio: f32 },
    #[error("layout v3 node {id} has an invalid size reference")]
    InvalidSizeRef { id: u32 },
    #[error("layout v3 node {id} references missing child {child}")]
    MissingChild { id: u32, child: u32 },
    #[error("layout v3 space {space} references missing root {root}")]
    MissingSpaceRoot { space: DockSpaceId, root: u32 },
    #[error("layout v3 floating root {id} references missing node {root}")]
    MissingFloatingRoot { id: String, root: u32 },
    #[error("layout v3 central policy in space {space} references missing node {node}")]
    MissingCentralNode { space: DockSpaceId, node: u32 },
    #[error("layout v3 node {id} participates in a cycle")]
    Cycle { id: u32 },
    #[error("layout v3 node {id} is referenced more than once")]
    SharedNode { id: u32 },
    #[error("layout v3 node {id} is unreachable")]
    UnreachableNode { id: u32 },
    #[error("layout v3 central node {node} is outside the main root of space {space}")]
    CentralOutsideSpace { space: DockSpaceId, node: u32 },
    #[error("layout v3 central node {node} in space {space} must be a tab group leaf")]
    CentralNodeNotGroup { space: DockSpaceId, node: u32 },
    #[error("layout v3 floating root {id} has invalid bounds")]
    InvalidFloatingBounds { id: String },
    #[error("layout v2 split {id} has zero total weight")]
    AllZeroSplit { id: u32 },
    #[error("layout migration exhausted structural node ids")]
    NodeIdExhausted,
    #[error("layout export exhausted structural node ids")]
    ExportNodeIdExhausted,
    #[error("layout v2 has too many floating roots in space {space}")]
    FloatingOrdinalOverflow { space: DockSpaceId },
}

#[derive(Debug, Error)]
pub(crate) enum CanonicalLayoutDecodeError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("unsupported docking layout version {actual}")]
    UnsupportedVersion { actual: u32 },
    #[error(transparent)]
    Migration(#[from] LayoutMigrationError),
}

#[derive(Deserialize)]
struct LayoutVersionProbe {
    layout_version: u32,
}

/// Decodes the version tag before selecting a version-specific wire DTO.
pub(crate) fn decode_canonical_layout_json(
    json: &str,
) -> Result<CanonicalLayoutV3, CanonicalLayoutDecodeError> {
    let version = serde_json::from_str::<LayoutVersionProbe>(json)?.layout_version;
    match version {
        crate::DOCK_LAYOUT_VERSION => {
            let legacy = serde_json::from_str::<DockLayout>(json)?;
            Ok(migrate_layout_v2(&legacy)?)
        }
        CANONICAL_LAYOUT_VERSION => {
            let layout = serde_json::from_str::<CanonicalLayoutV3>(json)?;
            validate_layout_v3(&layout)?;
            Ok(layout)
        }
        actual => Err(CanonicalLayoutDecodeError::UnsupportedVersion { actual }),
    }
}

pub(crate) fn migrate_layout_v2(
    layout: &DockLayout,
) -> Result<CanonicalLayoutV3, LayoutMigrationError> {
    layout.validate()?;
    let by_id = layout
        .nodes
        .iter()
        .map(|node| (node.id(), node))
        .collect::<BTreeMap<_, _>>();
    let mut state = MigrationState {
        by_id,
        output: Vec::new(),
        memo: HashMap::new(),
        next_id: Some(1),
        visiting: BTreeSet::new(),
        group_ids: BTreeSet::new(),
    };

    let mut legacy_spaces = layout.spaces.iter().collect::<Vec<_>>();
    legacy_spaces.sort_by(|left, right| left.id.cmp(&right.id));
    let mut spaces = Vec::with_capacity(legacy_spaces.len());
    let mut seen_spaces = BTreeSet::new();
    let mut floating_ids = BTreeSet::new();
    for space in legacy_spaces {
        if space.id.as_str().is_empty() {
            return Err(LayoutMigrationError::EmptySpaceId);
        }
        if !seen_spaces.insert(space.id.clone()) {
            return Err(LayoutMigrationError::DuplicateSpace {
                space: space.id.clone(),
            });
        }
        let root = space
            .root
            .map(|root| state.convert_node(root, &space.id, LegacyRootScope::Main, &mut Vec::new()))
            .transpose()?;
        let floatings = space
            .floatings
            .iter()
            .enumerate()
            .map(|(floating_index, floating)| {
                let z_order = i32::try_from(floating_index).map_err(|_| {
                    LayoutMigrationError::FloatingOrdinalOverflow {
                        space: space.id.clone(),
                    }
                })?;
                let id = legacy_floating_id(&space.id, floating_index);
                if !floating_ids.insert(id.clone()) {
                    return Err(LayoutMigrationError::DuplicateFloating { id });
                }
                Ok(CanonicalLayoutFloatingV3 {
                    id,
                    root: state.convert_node(
                        floating.root,
                        &space.id,
                        LegacyRootScope::Floating(floating_index),
                        &mut Vec::new(),
                    )?,
                    bounds: floating.bounds,
                    z_order,
                })
            })
            .collect::<Result<Vec<_>, LayoutMigrationError>>()?;
        let central = if let Some(central) = space.central.as_ref() {
            Some(CanonicalLayoutCentralV3 {
                node: central
                    .node
                    .map(|node| {
                        state.memo.get(&node).copied().ok_or_else(|| {
                            LayoutMigrationError::MissingCentralNode {
                                space: space.id.clone(),
                                node,
                            }
                        })
                    })
                    .transpose()?,
                keep_alive_when_empty: central.keep_alive_when_empty,
                passthrough_when_empty: central.passthrough_when_empty,
            })
        } else {
            None
        };
        spaces.push(CanonicalLayoutSpaceV3 {
            id: space.id.clone(),
            root,
            floating_roots: floatings,
            central,
        });
    }

    state.output.sort_by_key(CanonicalLayoutNodeV3::id);
    let migrated = CanonicalLayoutV3 {
        layout_version: CANONICAL_LAYOUT_VERSION,
        spaces,
        nodes: state.output,
    };
    validate_layout_v3(&migrated)?;
    Ok(migrated)
}

pub(crate) fn validate_layout_v3(layout: &CanonicalLayoutV3) -> Result<(), LayoutMigrationError> {
    if layout.layout_version != CANONICAL_LAYOUT_VERSION {
        return Err(LayoutMigrationError::UnsupportedVersion {
            actual: layout.layout_version,
        });
    }

    let mut by_id = BTreeMap::new();
    let mut group_ids = BTreeSet::new();
    let mut items = BTreeSet::new();
    for node in &layout.nodes {
        if by_id.insert(node.id(), node).is_some() {
            return Err(LayoutMigrationError::DuplicateNode { id: node.id() });
        }
        match node {
            CanonicalLayoutNodeV3::Group {
                id,
                group_id,
                items: node_items,
                selected,
                size_ref,
            } => {
                if group_id.is_empty() {
                    return Err(LayoutMigrationError::EmptyGroupId { id: *id });
                }
                if !group_ids.insert(group_id.clone()) {
                    return Err(LayoutMigrationError::DuplicateGroupId {
                        id: group_id.clone(),
                    });
                }
                if node_items.is_empty() {
                    return Err(LayoutMigrationError::EmptyGroup { id: *id });
                }
                if !node_items.contains(selected) {
                    return Err(LayoutMigrationError::SelectedItemMissing { id: *id });
                }
                if size_ref.is_some_and(|size_ref| !size_ref.is_valid()) {
                    return Err(LayoutMigrationError::InvalidSizeRef { id: *id });
                }
                for item in node_items {
                    if !items.insert(item.clone()) {
                        return Err(LayoutMigrationError::DuplicateItem { item: item.clone() });
                    }
                }
            }
            CanonicalLayoutNodeV3::Branch {
                id,
                ratio,
                size_ref,
                ..
            } => {
                if !ratio.is_finite() || !(0.0..=1.0).contains(ratio) {
                    return Err(LayoutMigrationError::InvalidRatio {
                        id: *id,
                        ratio: *ratio,
                    });
                }
                if size_ref.is_some_and(|size_ref| !size_ref.is_valid()) {
                    return Err(LayoutMigrationError::InvalidSizeRef { id: *id });
                }
            }
        }
    }

    let mut seen_spaces = BTreeSet::new();
    let mut floating_ids = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut owners = BTreeMap::new();
    for space in &layout.spaces {
        if space.id.as_str().is_empty() {
            return Err(LayoutMigrationError::EmptySpaceId);
        }
        if !seen_spaces.insert(space.id.clone()) {
            return Err(LayoutMigrationError::DuplicateSpace {
                space: space.id.clone(),
            });
        }
        if let Some(root) = space.root {
            if !by_id.contains_key(&root) {
                return Err(LayoutMigrationError::MissingSpaceRoot {
                    space: space.id.clone(),
                    root,
                });
            }
            visit_v3(
                root,
                &LayoutNodeOwner::Main(space.id.clone()),
                &by_id,
                &mut visited,
                &mut BTreeSet::new(),
                &mut owners,
            )?;
        }
        let mut z_orders = BTreeSet::new();
        for floating in &space.floating_roots {
            if floating.id.is_empty() {
                return Err(LayoutMigrationError::EmptyFloatingId {
                    space: space.id.clone(),
                });
            }
            if !floating_ids.insert(floating.id.clone()) {
                return Err(LayoutMigrationError::DuplicateFloating {
                    id: floating.id.clone(),
                });
            }
            if !z_orders.insert(floating.z_order) {
                return Err(LayoutMigrationError::DuplicateFloatingZOrder {
                    space: space.id.clone(),
                    z_order: floating.z_order,
                });
            }
            if !floating.bounds.is_finite_with_non_negative_size()
                || floating.bounds.width <= 0.0
                || floating.bounds.height <= 0.0
            {
                return Err(LayoutMigrationError::InvalidFloatingBounds {
                    id: floating.id.clone(),
                });
            }
            if !by_id.contains_key(&floating.root) {
                return Err(LayoutMigrationError::MissingFloatingRoot {
                    id: floating.id.clone(),
                    root: floating.root,
                });
            }
            visit_v3(
                floating.root,
                &LayoutNodeOwner::Floating {
                    space: space.id.clone(),
                    floating: floating.id.clone(),
                },
                &by_id,
                &mut visited,
                &mut BTreeSet::new(),
                &mut owners,
            )?;
        }
    }

    if visited.len() != by_id.len()
        && let Some(id) = by_id.keys().find(|id| !visited.contains(id))
    {
        return Err(LayoutMigrationError::UnreachableNode { id: *id });
    }

    for space in &layout.spaces {
        if let Some(central) = &space.central
            && let Some(node) = central.node
        {
            let Some(node_data) = by_id.get(&node) else {
                return Err(LayoutMigrationError::MissingCentralNode {
                    space: space.id.clone(),
                    node,
                });
            };
            if !matches!(owners.get(&node), Some(LayoutNodeOwner::Main(owner)) if owner == &space.id)
            {
                return Err(LayoutMigrationError::CentralOutsideSpace {
                    space: space.id.clone(),
                    node,
                });
            }
            if !matches!(node_data, CanonicalLayoutNodeV3::Group { .. }) {
                return Err(LayoutMigrationError::CentralNodeNotGroup {
                    space: space.id.clone(),
                    node,
                });
            }
        }
    }

    Ok(())
}

impl CanonicalLayoutV3 {
    pub(crate) fn import_graph(
        &self,
        authority: &mut CanonicalGraphAuthority,
    ) -> Result<CanonicalDockGraph, LayoutMigrationError> {
        validate_layout_v3(self)?;
        let by_id = self
            .nodes
            .iter()
            .map(|node| (node.id(), node))
            .collect::<BTreeMap<_, _>>();
        let mut assembly = DockGraphAssembly::new();
        for space in &self.spaces {
            assembly.add_space(space.id.clone())?;
        }
        let mut imported = BTreeMap::new();
        for space in &self.spaces {
            if let Some(root) = space.root {
                let root = import_v3_node(root, &by_id, &mut assembly, &mut imported)?;
                assembly.set_root(space.id.clone(), root)?;
            }
            for floating in &space.floating_roots {
                let root = import_v3_node(floating.root, &by_id, &mut assembly, &mut imported)?;
                assembly.add_floating_root(
                    DockFloatingId::from(floating.id.clone()),
                    space.id.clone(),
                    root,
                    floating.bounds,
                    floating.z_order,
                )?;
            }
            if let Some(central) = &space.central {
                let node = central
                    .node
                    .map(|node| {
                        imported.get(&node).copied().ok_or_else(|| {
                            LayoutMigrationError::MissingCentralNode {
                                space: space.id.clone(),
                                node,
                            }
                        })
                    })
                    .transpose()?;
                assembly.set_central_node(
                    space.id.clone(),
                    node,
                    central.keep_alive_when_empty,
                    central.passthrough_when_empty,
                )?;
            }
        }
        Ok(assembly.publish(authority)?)
    }

    pub(crate) fn from_graph(graph: &CanonicalDockGraph) -> Result<Self, LayoutMigrationError> {
        graph.validate()?;
        let mut state = ExportState::default();
        let mut spaces = Vec::with_capacity(graph.spaces().len());
        for (space_id, space) in graph.spaces() {
            let root = space
                .root
                .map(|root| state.export_node(graph, root))
                .transpose()?;
            let mut floating_ids = space.floating_roots.iter().collect::<Vec<_>>();
            floating_ids.sort_by(|left, right| {
                let left_root = graph.floating_roots().get(*left);
                let right_root = graph.floating_roots().get(*right);
                left_root
                    .map(|floating| floating.z_order)
                    .cmp(&right_root.map(|floating| floating.z_order))
                    .then_with(|| left.cmp(right))
            });
            let floating_roots = floating_ids
                .into_iter()
                .map(|floating_id| {
                    let floating = graph.floating_roots().get(floating_id).ok_or_else(|| {
                        LayoutMigrationError::Graph(CanonicalGraphError::FloatingRootMissing {
                            floating: floating_id.clone(),
                        })
                    })?;
                    Ok(CanonicalLayoutFloatingV3 {
                        id: floating_id.as_str().to_owned(),
                        root: state.export_node(graph, floating.root)?,
                        bounds: floating.bounds,
                        z_order: floating.z_order,
                    })
                })
                .collect::<Result<Vec<_>, LayoutMigrationError>>()?;
            let central_node = space
                .central
                .node
                .map(|node| state.export_node(graph, node))
                .transpose()?;
            let central = if central_node.is_some()
                || !space.central.keep_alive_when_empty
                || space.central.passthrough_when_empty
            {
                Some(CanonicalLayoutCentralV3 {
                    node: central_node,
                    keep_alive_when_empty: space.central.keep_alive_when_empty,
                    passthrough_when_empty: space.central.passthrough_when_empty,
                })
            } else {
                None
            };
            spaces.push(CanonicalLayoutSpaceV3 {
                id: space_id.clone(),
                root,
                floating_roots,
                central,
            });
        }
        let layout = Self {
            layout_version: CANONICAL_LAYOUT_VERSION,
            spaces,
            nodes: state.nodes.into_values().collect(),
        };
        validate_layout_v3(&layout)?;
        Ok(layout)
    }
}

#[derive(Debug, Clone, Copy)]
enum LegacyRootScope {
    Main,
    Floating(usize),
}

struct MigrationState<'a> {
    by_id: BTreeMap<u32, &'a DockLayoutNode>,
    output: Vec<CanonicalLayoutNodeV3>,
    memo: HashMap<u32, u32>,
    next_id: Option<u32>,
    visiting: BTreeSet<u32>,
    group_ids: BTreeSet<String>,
}

impl MigrationState<'_> {
    fn convert_node(
        &mut self,
        id: u32,
        space: &DockSpaceId,
        scope: LegacyRootScope,
        path: &mut Vec<usize>,
    ) -> Result<u32, LayoutMigrationError> {
        if let Some(mapped) = self.memo.get(&id).copied() {
            return Ok(mapped);
        }
        if !self.visiting.insert(id) {
            return Err(LayoutMigrationError::Cycle { id });
        }
        let node = self
            .by_id
            .get(&id)
            .copied()
            .ok_or(LayoutMigrationError::MissingChild { id, child: id })?;
        let result = match node {
            DockLayoutNode::Tabs {
                items, selected, ..
            } => {
                let structural_id = self.allocate_node_id()?;
                let selected = selected
                    .clone()
                    .ok_or(LayoutMigrationError::SelectedItemMissing { id })?;
                let group_id = legacy_group_id(space, scope, path);
                if !self.group_ids.insert(group_id.clone()) {
                    return Err(LayoutMigrationError::DuplicateGroupId { id: group_id });
                }
                self.output.push(CanonicalLayoutNodeV3::Group {
                    id: structural_id,
                    group_id,
                    items: items.clone(),
                    selected,
                    size_ref: None,
                });
                structural_id
            }
            DockLayoutNode::Split {
                axis,
                children,
                fractions,
                ..
            } => {
                let total_weight = fractions
                    .iter()
                    .map(|weight| f64::from(*weight))
                    .sum::<f64>();
                if total_weight == 0.0 {
                    return Err(LayoutMigrationError::AllZeroSplit { id });
                }
                let mut converted_children = Vec::with_capacity(children.len());
                for (index, child) in children.iter().copied().enumerate() {
                    path.push(index);
                    let converted = self.convert_node(child, space, scope, path)?;
                    path.pop();
                    converted_children.push(converted);
                }
                if converted_children.len() == 1 {
                    converted_children[0]
                } else {
                    let structural_id = self.allocate_node_id()?;
                    let mut right = *converted_children
                        .last()
                        .expect("validated non-empty split");
                    let mut right_weight =
                        f64::from(*fractions.last().expect("fractions validated"));
                    for index in (0..converted_children.len() - 1).rev() {
                        let left_weight = f64::from(fractions[index]);
                        let denominator = left_weight + right_weight;
                        let ratio = if denominator == 0.0 {
                            0.5
                        } else {
                            (left_weight / denominator) as f32
                        };
                        if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
                            return Err(LayoutMigrationError::InvalidRatio { id, ratio });
                        }
                        let branch_id = if index == 0 {
                            structural_id
                        } else {
                            self.allocate_node_id()?
                        };
                        self.output.push(CanonicalLayoutNodeV3::Branch {
                            id: branch_id,
                            axis: *axis,
                            first: converted_children[index],
                            second: right,
                            ratio,
                            size_ref: None,
                        });
                        right = branch_id;
                        right_weight = denominator;
                    }
                    right
                }
            }
        };
        self.visiting.remove(&id);
        self.memo.insert(id, result);
        Ok(result)
    }

    fn allocate_node_id(&mut self) -> Result<u32, LayoutMigrationError> {
        let id = self.next_id.ok_or(LayoutMigrationError::NodeIdExhausted)?;
        self.next_id = id.checked_add(1);
        Ok(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LayoutNodeOwner {
    Main(DockSpaceId),
    Floating {
        space: DockSpaceId,
        floating: String,
    },
}

fn visit_v3(
    id: u32,
    owner: &LayoutNodeOwner,
    by_id: &BTreeMap<u32, &CanonicalLayoutNodeV3>,
    visited: &mut BTreeSet<u32>,
    visiting: &mut BTreeSet<u32>,
    owners: &mut BTreeMap<u32, LayoutNodeOwner>,
) -> Result<(), LayoutMigrationError> {
    if visiting.contains(&id) {
        return Err(LayoutMigrationError::Cycle { id });
    }
    if visited.contains(&id) {
        return Err(LayoutMigrationError::SharedNode { id });
    }
    let node = by_id
        .get(&id)
        .copied()
        .ok_or(LayoutMigrationError::MissingChild { id, child: id })?;
    visiting.insert(id);
    visited.insert(id);
    owners.insert(id, owner.clone());
    if let CanonicalLayoutNodeV3::Branch { first, second, .. } = node {
        if !by_id.contains_key(first) {
            return Err(LayoutMigrationError::MissingChild { id, child: *first });
        }
        visit_v3(*first, owner, by_id, visited, visiting, owners)?;
        if !by_id.contains_key(second) {
            return Err(LayoutMigrationError::MissingChild { id, child: *second });
        }
        visit_v3(*second, owner, by_id, visited, visiting, owners)?;
    }
    visiting.remove(&id);
    Ok(())
}

fn import_v3_node(
    id: u32,
    by_id: &BTreeMap<u32, &CanonicalLayoutNodeV3>,
    assembly: &mut DockGraphAssembly,
    imported: &mut BTreeMap<u32, AssemblyNodeId>,
) -> Result<AssemblyNodeId, LayoutMigrationError> {
    if let Some(node) = imported.get(&id).copied() {
        return Ok(node);
    }
    let node = by_id
        .get(&id)
        .copied()
        .ok_or(LayoutMigrationError::MissingChild { id, child: id })?;
    let imported_node = match node {
        CanonicalLayoutNodeV3::Group {
            group_id,
            items,
            selected,
            size_ref,
            ..
        } => assembly.add_group_with_size_ref(
            DockGroupId::from(group_id.clone()),
            items.clone(),
            selected.clone(),
            size_ref.map(Into::into),
        )?,
        CanonicalLayoutNodeV3::Branch {
            axis,
            first,
            second,
            ratio,
            size_ref,
            ..
        } => {
            let first = import_v3_node(*first, by_id, assembly, imported)?;
            let second = import_v3_node(*second, by_id, assembly, imported)?;
            assembly.add_branch_with_size_ref(
                *axis,
                first,
                second,
                *ratio,
                size_ref.map(Into::into),
            )?
        }
    };
    imported.insert(id, imported_node);
    Ok(imported_node)
}

#[derive(Default)]
struct ExportState {
    next_id: u64,
    node_ids: BTreeMap<CanonicalNodeId, u32>,
    nodes: BTreeMap<u32, CanonicalLayoutNodeV3>,
}

impl ExportState {
    fn export_node(
        &mut self,
        graph: &CanonicalDockGraph,
        node: CanonicalNodeId,
    ) -> Result<u32, LayoutMigrationError> {
        if let Some(id) = self.node_ids.get(&node).copied() {
            return Ok(id);
        }
        let id = self.allocate_node_id()?;
        self.node_ids.insert(node, id);
        let node_data = graph
            .nodes()
            .get(node)
            .cloned()
            .ok_or(CanonicalGraphError::MissingNode { node })?;
        let exported = match node_data {
            CanonicalNode::Group { group, size_ref } => {
                let group_data = graph.groups().get(&group).ok_or_else(|| {
                    CanonicalGraphError::MissingGroup {
                        group: group.clone(),
                    }
                })?;
                CanonicalLayoutNodeV3::Group {
                    id,
                    group_id: group.as_str().to_owned(),
                    items: group_data.items.clone(),
                    selected: group_data.selected.clone(),
                    size_ref: size_ref.map(Into::into),
                }
            }
            CanonicalNode::Branch {
                axis,
                first,
                second,
                ratio,
                size_ref,
            } => CanonicalLayoutNodeV3::Branch {
                id,
                axis,
                first: self.export_node(graph, first)?,
                second: self.export_node(graph, second)?,
                ratio,
                size_ref: size_ref.map(Into::into),
            },
        };
        self.nodes.insert(id, exported);
        Ok(id)
    }

    fn allocate_node_id(&mut self) -> Result<u32, LayoutMigrationError> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(LayoutMigrationError::ExportNodeIdExhausted)?;
        u32::try_from(self.next_id).map_err(|_| LayoutMigrationError::ExportNodeIdExhausted)
    }
}

fn legacy_group_id(space: &DockSpaceId, scope: LegacyRootScope, path: &[usize]) -> String {
    let mut id = format!("group:{}:{}:", space.as_str().len(), space.as_str());
    match scope {
        LegacyRootScope::Main => id.push_str("main"),
        LegacyRootScope::Floating(index) => {
            id.push_str("floating/");
            id.push_str(&index.to_string());
        }
    }
    for index in path {
        id.push('/');
        id.push_str(&index.to_string());
    }
    id
}

fn legacy_floating_id(space: &DockSpaceId, ordinal: usize) -> String {
    format!(
        "floating:{}:{}:{ordinal}",
        space.as_str().len(),
        space.as_str()
    )
}

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}
