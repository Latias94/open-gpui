use crate::{
    DockCentralRegion, DockFloatingContainer, DockGraph, DockGraphValidationError, DockItemId,
    DockLayoutRect, DockNode, DockNodeId, DockSpaceId, SplitAxis,
    dockspace_testkit::contract::{
        DockspaceAdapterContract, DockspaceDragTerminal, DockspaceProgressState,
        DockspaceReceiptExpectation,
    },
};
use open_gpui_ui_core::normalize_split_fractions;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use thiserror::Error;

pub(crate) use crate::dockspace_testkit::contract::{DockspaceAdapterTag, DockspaceOracleKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImGuiDockingCategory {
    BasicDocking,
    DockIntoParent,
    DockOverDockspace,
    TabOrder,
    TabFocusRestore,
    Undock,
    Sizing,
    SplitPayload,
    ViewportFocus,
    ViewportClose,
    ViewportOwnerChange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DockspaceTopologyNode {
    Tabs {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        node_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group_key: Option<String>,
        items: Vec<DockItemId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected: Option<DockItemId>,
    },
    Split {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        node_key: Option<String>,
        axis: SplitAxis,
        children: Vec<DockspaceTopologyNode>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fractions: Vec<f32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceFloatingSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) floating_key: Option<String>,
    pub(crate) root: DockspaceTopologyNode,
    pub(crate) bounds: DockLayoutRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceTopologySpace {
    pub(crate) id: DockSpaceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) root: Option<DockspaceTopologyNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) floatings: Vec<DockspaceFloatingSpec>,
    #[serde(default)]
    pub(crate) keep_central_when_empty: bool,
    #[serde(default)]
    pub(crate) passthrough_when_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceGroupStateExpectation {
    pub(crate) group: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selected: Option<DockItemId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) mru: Vec<DockItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) visible: Option<DockItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceFloatingOwnershipExpectation {
    pub(crate) floating: String,
    pub(crate) space: DockSpaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceIdentityExpectation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) preserved: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) created: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) retired: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) remapped: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceRevisionExpectation {
    #[serde(default)]
    pub(crate) topology_delta: u64,
    #[serde(default)]
    pub(crate) selection_delta: u64,
    #[serde(default)]
    pub(crate) interaction_delta: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceCheckpointFacts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) groups: Vec<DockspaceGroupStateExpectation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) floating_ownership: Vec<DockspaceFloatingOwnershipExpectation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) focused_item: Option<DockItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) terminal: Option<DockspaceDragTerminal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) progress: Option<DockspaceProgressState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) receipt: Option<DockspaceReceiptExpectation>,
    #[serde(
        default,
        skip_serializing_if = "DockspaceIdentityExpectation::is_empty"
    )]
    pub(crate) identities: DockspaceIdentityExpectation,
    #[serde(default, skip_serializing_if = "DockspaceRevisionExpectation::is_zero")]
    pub(crate) revisions: DockspaceRevisionExpectation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) unreachable_node_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceScenarioCheckpoint {
    pub(crate) after_step: String,
    pub(crate) spaces: Vec<DockspaceTopologySpace>,
    #[serde(default, skip_serializing_if = "DockspaceCheckpointFacts::is_empty")]
    pub(crate) facts: DockspaceCheckpointFacts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceScenario {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) primary_space: Option<DockSpaceId>,
    pub(crate) spaces: Vec<DockspaceTopologySpace>,
    #[serde(default)]
    pub(crate) steps: Vec<crate::dockspace_testkit::ops::DockspaceScenarioStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) checkpoints: Vec<DockspaceScenarioCheckpoint>,
    pub(crate) adapter_contracts: Vec<DockspaceAdapterContract>,
    #[serde(default)]
    pub(crate) imgui_categories: Vec<ImGuiDockingCategory>,
}

#[derive(Debug)]
pub(crate) struct DockspaceScenarioGraph {
    pub(crate) graph: DockGraph,
    pub(crate) roots_by_space: HashMap<DockSpaceId, DockNodeId>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum DockspaceScenarioBuildError {
    #[error("dockspace scenario {scenario} has duplicate space {space}")]
    DuplicateSpace {
        scenario: String,
        space: DockSpaceId,
    },
    #[error("dockspace scenario {scenario} names unknown primary space {space}")]
    UnknownPrimarySpace {
        scenario: String,
        space: DockSpaceId,
    },
    #[error("dockspace scenario {scenario} has duplicate panel item {item}")]
    DuplicatePanel { scenario: String, item: DockItemId },
    #[error("dockspace scenario {scenario} has an empty tabs node")]
    EmptyTabs { scenario: String },
    #[error("dockspace scenario {scenario} selected item {selected} is missing from tabs")]
    MissingSelected {
        scenario: String,
        selected: DockItemId,
    },
    #[error("dockspace scenario {scenario} split has fewer than two children")]
    SplitTooFewChildren { scenario: String },
    #[error(
        "dockspace scenario {scenario} split has {children_len} children but {fractions_len} fractions"
    )]
    SplitFractionCount {
        scenario: String,
        children_len: usize,
        fractions_len: usize,
    },
    #[error("dockspace scenario {scenario} has invalid split fraction at index {index}")]
    InvalidSplitFraction { scenario: String, index: usize },
    #[error("dockspace scenario {scenario} built an invalid graph: {error}")]
    InvalidGraph {
        scenario: String,
        error: DockGraphValidationError,
    },
}

impl DockspaceTopologyNode {
    pub(crate) fn tabs(items: impl IntoIterator<Item = impl Into<DockItemId>>) -> Self {
        let items = items.into_iter().map(Into::into).collect::<Vec<_>>();
        Self::Tabs {
            node_key: None,
            group_key: None,
            selected: items.first().cloned(),
            items,
        }
    }

    pub(crate) fn tabs_selected(
        items: impl IntoIterator<Item = impl Into<DockItemId>>,
        selected: impl Into<DockItemId>,
    ) -> Self {
        Self::Tabs {
            node_key: None,
            group_key: None,
            items: items.into_iter().map(Into::into).collect(),
            selected: Some(selected.into()),
        }
    }

    pub(crate) fn split(
        axis: SplitAxis,
        children: impl IntoIterator<Item = DockspaceTopologyNode>,
    ) -> Self {
        Self::Split {
            node_key: None,
            axis,
            children: children.into_iter().collect(),
            fractions: Vec::new(),
        }
    }
}

impl DockspaceFloatingSpec {
    pub(crate) fn new(root: DockspaceTopologyNode, bounds: DockLayoutRect) -> Self {
        Self {
            floating_key: None,
            root,
            bounds,
        }
    }
}

impl DockspaceIdentityExpectation {
    fn is_empty(&self) -> bool {
        self.preserved.is_empty()
            && self.created.is_empty()
            && self.retired.is_empty()
            && self.remapped.is_empty()
    }
}

impl DockspaceRevisionExpectation {
    fn is_zero(&self) -> bool {
        self.topology_delta == 0 && self.selection_delta == 0 && self.interaction_delta == 0
    }
}

impl DockspaceCheckpointFacts {
    fn is_empty(&self) -> bool {
        self.groups.is_empty()
            && self.floating_ownership.is_empty()
            && self.focused_item.is_none()
            && self.terminal.is_none()
            && self.progress.is_none()
            && self.receipt.is_none()
            && self.identities.is_empty()
            && self.revisions.is_zero()
            && self.unreachable_node_count.is_none()
    }
}

impl DockspaceTopologySpace {
    pub(crate) fn new(id: impl Into<DockSpaceId>) -> Self {
        Self {
            id: id.into(),
            root: None,
            floatings: Vec::new(),
            keep_central_when_empty: false,
            passthrough_when_empty: false,
        }
    }

    pub(crate) fn root(mut self, root: DockspaceTopologyNode) -> Self {
        self.root = Some(root);
        self
    }

    pub(crate) fn floating(mut self, floating: DockspaceFloatingSpec) -> Self {
        self.floatings.push(floating);
        self
    }
}

impl DockspaceScenario {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            primary_space: None,
            spaces: Vec::new(),
            steps: Vec::new(),
            checkpoints: Vec::new(),
            adapter_contracts: Vec::new(),
            imgui_categories: Vec::new(),
        }
    }

    pub(crate) fn adapter_contract(
        &self,
        adapter: DockspaceAdapterTag,
    ) -> Option<&DockspaceAdapterContract> {
        self.adapter_contracts
            .iter()
            .find(|contract| contract.adapter == adapter)
    }

    pub(crate) fn supports_adapter(&self, adapter: DockspaceAdapterTag) -> bool {
        self.adapter_contract(adapter).is_some()
    }

    pub(crate) fn oracles(&self) -> BTreeSet<DockspaceOracleKind> {
        self.adapter_contracts
            .iter()
            .flat_map(|contract| {
                contract
                    .required_oracles
                    .iter()
                    .chain(&contract.optional_oracles)
            })
            .copied()
            .collect()
    }

    pub(crate) fn primary_space(mut self, space: impl Into<DockSpaceId>) -> Self {
        self.primary_space = Some(space.into());
        self
    }

    pub(crate) fn space(mut self, space: DockspaceTopologySpace) -> Self {
        self.spaces.push(space);
        self
    }

    pub(crate) fn build_graph(
        &self,
    ) -> Result<DockspaceScenarioGraph, DockspaceScenarioBuildError> {
        self.build_graph_from_spaces(&self.spaces)
    }

    pub(crate) fn build_checkpoint_graph(
        &self,
        checkpoint: &DockspaceScenarioCheckpoint,
    ) -> Result<DockspaceScenarioGraph, DockspaceScenarioBuildError> {
        self.build_graph_from_spaces(&checkpoint.spaces)
    }

    fn build_graph_from_spaces(
        &self,
        spaces: &[DockspaceTopologySpace],
    ) -> Result<DockspaceScenarioGraph, DockspaceScenarioBuildError> {
        let mut seen_spaces = BTreeSet::new();
        let mut seen_items = BTreeSet::new();
        for space in spaces {
            if !seen_spaces.insert(space.id.clone()) {
                return Err(DockspaceScenarioBuildError::DuplicateSpace {
                    scenario: self.id.clone(),
                    space: space.id.clone(),
                });
            }
            if let Some(root) = &space.root {
                self.validate_node_items(root, &mut seen_items)?;
            }
            for floating in &space.floatings {
                self.validate_node_items(&floating.root, &mut seen_items)?;
            }
        }
        if let Some(primary_space) = &self.primary_space
            && !seen_spaces.contains(primary_space)
        {
            return Err(DockspaceScenarioBuildError::UnknownPrimarySpace {
                scenario: self.id.clone(),
                space: primary_space.clone(),
            });
        }

        let mut graph = DockGraph::new();
        let mut roots_by_space = HashMap::new();
        for space in spaces {
            if let Some(root) = &space.root {
                let root = self.build_node(&mut graph, root)?;
                graph.set_root(space.id.clone(), root);
                roots_by_space.insert(space.id.clone(), root);
            }
            for floating in &space.floatings {
                let child = self.build_node(&mut graph, &floating.root)?;
                let floating_node = graph.insert_node(DockNode::Floating { child });
                graph
                    .floating_containers_mut(space.id.clone())
                    .push(DockFloatingContainer {
                        node: floating_node,
                        bounds: floating.bounds.to_bounds(),
                    });
            }
            if space.keep_central_when_empty {
                graph.set_central_region(
                    space.id.clone(),
                    DockCentralRegion::empty()
                        .with_passthrough_when_empty(space.passthrough_when_empty),
                );
            }
        }
        graph.canonicalize();
        graph
            .validate_canonical()
            .map_err(|error| DockspaceScenarioBuildError::InvalidGraph {
                scenario: self.id.clone(),
                error,
            })?;
        Ok(DockspaceScenarioGraph {
            graph,
            roots_by_space,
        })
    }

    fn validate_node_items(
        &self,
        node: &DockspaceTopologyNode,
        seen_items: &mut BTreeSet<DockItemId>,
    ) -> Result<(), DockspaceScenarioBuildError> {
        match node {
            DockspaceTopologyNode::Tabs {
                items, selected, ..
            } => {
                if items.is_empty() {
                    return Err(DockspaceScenarioBuildError::EmptyTabs {
                        scenario: self.id.clone(),
                    });
                }
                if let Some(selected) = selected
                    && !items.contains(selected)
                {
                    return Err(DockspaceScenarioBuildError::MissingSelected {
                        scenario: self.id.clone(),
                        selected: selected.clone(),
                    });
                }
                for item in items {
                    if !seen_items.insert(item.clone()) {
                        return Err(DockspaceScenarioBuildError::DuplicatePanel {
                            scenario: self.id.clone(),
                            item: item.clone(),
                        });
                    }
                }
            }
            DockspaceTopologyNode::Split {
                children,
                fractions,
                ..
            } => {
                if children.len() < 2 {
                    return Err(DockspaceScenarioBuildError::SplitTooFewChildren {
                        scenario: self.id.clone(),
                    });
                }
                if !fractions.is_empty() && fractions.len() != children.len() {
                    return Err(DockspaceScenarioBuildError::SplitFractionCount {
                        scenario: self.id.clone(),
                        children_len: children.len(),
                        fractions_len: fractions.len(),
                    });
                }
                for (index, fraction) in fractions.iter().enumerate() {
                    if !fraction.is_finite() || *fraction < 0.0 {
                        return Err(DockspaceScenarioBuildError::InvalidSplitFraction {
                            scenario: self.id.clone(),
                            index,
                        });
                    }
                }
                for child in children {
                    self.validate_node_items(child, seen_items)?;
                }
            }
        }
        Ok(())
    }

    fn build_node(
        &self,
        graph: &mut DockGraph,
        node: &DockspaceTopologyNode,
    ) -> Result<DockNodeId, DockspaceScenarioBuildError> {
        match node {
            DockspaceTopologyNode::Tabs {
                items, selected, ..
            } => Ok(graph.insert_node(DockNode::Tabs {
                items: items.clone(),
                selected: selected.clone().or_else(|| items.first().cloned()),
            })),
            DockspaceTopologyNode::Split {
                axis,
                children,
                fractions,
                ..
            } => {
                let children = children
                    .iter()
                    .map(|child| self.build_node(graph, child))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut fractions = if fractions.is_empty() {
                    vec![1.0; children.len()]
                } else {
                    fractions.clone()
                };
                normalize_split_fractions(&mut fractions);
                Ok(graph.insert_node(DockNode::Split {
                    axis: *axis,
                    children,
                    fractions,
                }))
            }
        }
    }
}
