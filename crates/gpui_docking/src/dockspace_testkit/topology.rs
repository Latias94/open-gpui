use crate::{
    DockCentralRegion, DockFloatingContainer, DockFloatingId, DockGraph, DockGraphValidationError,
    DockGroupId, DockItemId, DockLayoutRect, DockNode, DockNodeId, DockSpaceId, SplitAxis,
    canonical_runtime::CanonicalGraphRuntime,
    dockspace_testkit::contract::{
        DockspaceAdapterContract, DockspaceAdapterStepSupport, DockspaceDragTerminal,
        DockspaceProgressState, DockspaceReceiptExpectation,
    },
    graph_assembly::{AssemblyNodeId, CanonicalGraphError, DockGraphAssembly},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceFixtureArrangement {
    DirectDeterministic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceHarnessReadyPoint {
    CanonicalGraphPublished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceFixturePhase {
    pub(crate) arrangement: DockspaceFixtureArrangement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceHarnessPhase {
    pub(crate) ready: DockspaceHarnessReadyPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceWarmupPhase {
    pub(crate) iterations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceTeardownPhase {
    pub(crate) require_canonical_graph: bool,
    pub(crate) require_no_pending_progress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceScenarioPhases {
    pub(crate) fixture: DockspaceFixturePhase,
    pub(crate) harness: DockspaceHarnessPhase,
    pub(crate) warmup: DockspaceWarmupPhase,
    pub(crate) teardown: DockspaceTeardownPhase,
}

impl Default for DockspaceScenarioPhases {
    fn default() -> Self {
        Self {
            fixture: DockspaceFixturePhase {
                arrangement: DockspaceFixtureArrangement::DirectDeterministic,
            },
            harness: DockspaceHarnessPhase {
                ready: DockspaceHarnessReadyPoint::CanonicalGraphPublished,
            },
            warmup: DockspaceWarmupPhase { iterations: 0 },
            teardown: DockspaceTeardownPhase {
                require_canonical_graph: true,
                require_no_pending_progress: true,
            },
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) central_item: Option<DockItemId>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceCheckpointPhase {
    AfterPrepare,
    AfterProviderReceipt,
    AfterRecoveryTransfer,
    Terminal,
    AfterTeardown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceScenarioCheckpoint {
    pub(crate) after_step: String,
    pub(crate) phase: DockspaceCheckpointPhase,
    pub(crate) spaces: Vec<DockspaceTopologySpace>,
    #[serde(default, skip_serializing_if = "DockspaceCheckpointFacts::is_empty")]
    pub(crate) facts: DockspaceCheckpointFacts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceScenario {
    pub(crate) id: String,
    pub(crate) phases: DockspaceScenarioPhases,
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
    #[error("dockspace scenario {scenario} built an invalid canonical graph: {error}")]
    InvalidCanonicalGraph {
        scenario: String,
        error: CanonicalGraphError,
    },
    #[error("dockspace scenario {scenario} has too many floating roots in space {space}")]
    FloatingOrderOverflow {
        scenario: String,
        space: DockSpaceId,
    },
    #[error("dockspace scenario {scenario} cannot observe missing graph node {node:?}")]
    MissingObservedNode { scenario: String, node: DockNodeId },
    #[error("dockspace scenario {scenario} central item {item} is not in the dock-space root")]
    MissingCentralItem { scenario: String, item: DockItemId },
    #[error("dockspace scenario {scenario} step {step} has no terminal checkpoint")]
    MissingTerminalCheckpoint { scenario: String, step: String },
    #[error("dockspace scenario {scenario} step {step} repeats its terminal checkpoint")]
    DuplicateTerminalCheckpoint { scenario: String, step: String },
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
            central_item: None,
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
            phases: DockspaceScenarioPhases::default(),
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

    pub(crate) fn adapter_step_support(
        &self,
        adapter: DockspaceAdapterTag,
        step: &crate::dockspace_testkit::ops::DockspaceScenarioStep,
    ) -> Option<DockspaceAdapterStepSupport> {
        self.adapter_contract(adapter)
            .map(|contract| contract.step_support(&step.required_capabilities()))
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

    pub(crate) fn declared_items(&self) -> BTreeSet<DockItemId> {
        let mut items = BTreeSet::new();
        for space in &self.spaces {
            if let Some(root) = &space.root {
                Self::collect_node_items(root, &mut items);
            }
            for floating in &space.floatings {
                Self::collect_node_items(&floating.root, &mut items);
            }
        }
        items
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

    pub(crate) fn build_canonical_runtime(
        &self,
    ) -> Result<CanonicalGraphRuntime, DockspaceScenarioBuildError> {
        self.build_canonical_runtime_from_spaces(&self.spaces)
    }

    pub(crate) fn build_canonical_checkpoint_runtime(
        &self,
        checkpoint: &DockspaceScenarioCheckpoint,
    ) -> Result<CanonicalGraphRuntime, DockspaceScenarioBuildError> {
        self.build_canonical_runtime_from_spaces(&checkpoint.spaces)
    }

    pub(crate) fn terminal_checkpoints(
        &self,
    ) -> Result<Vec<&DockspaceScenarioCheckpoint>, DockspaceScenarioBuildError> {
        self.steps
            .iter()
            .map(|step| {
                let mut matches = self.checkpoints.iter().filter(|checkpoint| {
                    checkpoint.after_step == step.id
                        && checkpoint.phase == DockspaceCheckpointPhase::Terminal
                });
                let checkpoint = matches.next().ok_or_else(|| {
                    DockspaceScenarioBuildError::MissingTerminalCheckpoint {
                        scenario: self.id.clone(),
                        step: step.id.clone(),
                    }
                })?;
                if matches.next().is_some() {
                    return Err(DockspaceScenarioBuildError::DuplicateTerminalCheckpoint {
                        scenario: self.id.clone(),
                        step: step.id.clone(),
                    });
                }
                Ok(checkpoint)
            })
            .collect()
    }

    pub(crate) fn observe_graph_spaces(
        &self,
        graph: &DockGraph,
    ) -> Result<Vec<DockspaceTopologySpace>, DockspaceScenarioBuildError> {
        self.spaces
            .iter()
            .map(|declared| {
                let root = graph
                    .root(&declared.id)
                    .map(|root| self.observe_node(graph, root))
                    .transpose()?;
                let floatings = graph
                    .floating_containers(&declared.id)
                    .iter()
                    .map(|floating| {
                        Ok(DockspaceFloatingSpec {
                            floating_key: None,
                            root: self.observe_node(graph, floating.node)?,
                            bounds: DockLayoutRect::from_bounds(floating.bounds),
                        })
                    })
                    .collect::<Result<Vec<_>, DockspaceScenarioBuildError>>()?;
                let central = graph.central_region(&declared.id);
                Ok(DockspaceTopologySpace {
                    id: declared.id.clone(),
                    root,
                    floatings,
                    keep_central_when_empty: central
                        .is_some_and(|region| region.keep_alive_when_empty),
                    passthrough_when_empty: central
                        .is_some_and(|region| region.passthrough_when_empty),
                    central_item: central
                        .and_then(|region| region.node)
                        .and_then(|node| self.first_item_in_node(graph, node)),
                })
            })
            .collect()
    }

    fn observe_node(
        &self,
        graph: &DockGraph,
        node: DockNodeId,
    ) -> Result<DockspaceTopologyNode, DockspaceScenarioBuildError> {
        match graph
            .node(node)
            .ok_or_else(|| DockspaceScenarioBuildError::MissingObservedNode {
                scenario: self.id.clone(),
                node,
            })? {
            DockNode::Tabs { items, selected } => Ok(DockspaceTopologyNode::Tabs {
                node_key: None,
                group_key: None,
                items: items.clone(),
                selected: selected.clone(),
            }),
            DockNode::Split {
                axis,
                children,
                fractions,
            } => Ok(DockspaceTopologyNode::Split {
                node_key: None,
                axis: *axis,
                children: children
                    .iter()
                    .map(|child| self.observe_node(graph, *child))
                    .collect::<Result<Vec<_>, _>>()?,
                fractions: fractions.clone(),
            }),
            DockNode::Floating { child } => self.observe_node(graph, *child),
        }
    }

    fn first_item_in_node(&self, graph: &DockGraph, node: DockNodeId) -> Option<DockItemId> {
        match graph.node(node)? {
            DockNode::Tabs { items, selected } => {
                selected.clone().or_else(|| items.first().cloned())
            }
            DockNode::Split { children, .. } => children
                .iter()
                .find_map(|child| self.first_item_in_node(graph, *child)),
            DockNode::Floating { child } => self.first_item_in_node(graph, *child),
        }
    }

    fn build_graph_from_spaces(
        &self,
        spaces: &[DockspaceTopologySpace],
    ) -> Result<DockspaceScenarioGraph, DockspaceScenarioBuildError> {
        self.validate_spaces(spaces)?;

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
                let central = space
                    .central_item
                    .as_ref()
                    .and_then(|item| graph.find_item_in_space(&space.id, item))
                    .filter(|(node, _)| {
                        roots_by_space.get(&space.id).copied()
                            == graph.root_for_node_in_space(&space.id, *node)
                    })
                    .map(|(node, _)| DockCentralRegion::with_node(node))
                    .unwrap_or_else(DockCentralRegion::empty)
                    .with_passthrough_when_empty(space.passthrough_when_empty);
                if space.central_item.is_some() && central.node.is_none() {
                    return Err(DockspaceScenarioBuildError::MissingCentralItem {
                        scenario: self.id.clone(),
                        item: space.central_item.clone().expect("checked above"),
                    });
                }
                graph.set_central_region(space.id.clone(), central);
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

    fn validate_spaces(
        &self,
        spaces: &[DockspaceTopologySpace],
    ) -> Result<(), DockspaceScenarioBuildError> {
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
        Ok(())
    }

    fn build_canonical_runtime_from_spaces(
        &self,
        spaces: &[DockspaceTopologySpace],
    ) -> Result<CanonicalGraphRuntime, DockspaceScenarioBuildError> {
        self.validate_spaces(spaces)?;

        let mut assembly = DockGraphAssembly::new();
        for space in spaces {
            assembly
                .add_space(space.id.clone())
                .map_err(|error| self.canonical_build_error(error))?;
        }

        for space in spaces {
            let mut main_item_nodes = BTreeMap::new();
            if let Some(root) = &space.root {
                let root = self.build_canonical_node(
                    &mut assembly,
                    &space.id,
                    "root",
                    root,
                    &mut main_item_nodes,
                )?;
                assembly
                    .set_root(space.id.clone(), root)
                    .map_err(|error| self.canonical_build_error(error))?;
            }

            for (index, floating) in space.floatings.iter().enumerate() {
                let mut floating_item_nodes = BTreeMap::new();
                let root = self.build_canonical_node(
                    &mut assembly,
                    &space.id,
                    &format!("floating-{index}"),
                    &floating.root,
                    &mut floating_item_nodes,
                )?;
                let floating_id = floating
                    .floating_key
                    .clone()
                    .map(DockFloatingId::from)
                    .unwrap_or_else(|| {
                        DockFloatingId::new(format!(
                            "fixture:{}:{}:floating:{index}",
                            self.id,
                            space.id.as_str()
                        ))
                    });
                let z_order = i32::try_from(index).map_err(|_| {
                    DockspaceScenarioBuildError::FloatingOrderOverflow {
                        scenario: self.id.clone(),
                        space: space.id.clone(),
                    }
                })?;
                assembly
                    .add_floating_root(
                        floating_id,
                        space.id.clone(),
                        root,
                        floating.bounds,
                        z_order,
                    )
                    .map_err(|error| self.canonical_build_error(error))?;
            }

            let central = match &space.central_item {
                Some(item) => Some(*main_item_nodes.get(item).ok_or_else(|| {
                    DockspaceScenarioBuildError::MissingCentralItem {
                        scenario: self.id.clone(),
                        item: item.clone(),
                    }
                })?),
                None => None,
            };
            assembly
                .set_central_node(
                    space.id.clone(),
                    central,
                    space.keep_central_when_empty,
                    space.passthrough_when_empty,
                )
                .map_err(|error| self.canonical_build_error(error))?;
        }

        CanonicalGraphRuntime::from_assembly(assembly)
            .map_err(|error| self.canonical_build_error(error))
    }

    fn build_canonical_node(
        &self,
        assembly: &mut DockGraphAssembly,
        space: &DockSpaceId,
        path: &str,
        node: &DockspaceTopologyNode,
        item_nodes: &mut BTreeMap<DockItemId, AssemblyNodeId>,
    ) -> Result<AssemblyNodeId, DockspaceScenarioBuildError> {
        match node {
            DockspaceTopologyNode::Tabs {
                node_key,
                group_key,
                items,
                selected,
            } => {
                let group = group_key
                    .as_ref()
                    .or(node_key.as_ref())
                    .cloned()
                    .map(DockGroupId::from)
                    .unwrap_or_else(|| {
                        DockGroupId::new(format!(
                            "fixture:{}:{}:group:{path}",
                            self.id,
                            space.as_str()
                        ))
                    });
                let selected = selected
                    .clone()
                    .or_else(|| items.first().cloned())
                    .expect("validated fixture tabs must be non-empty");
                let leaf = assembly
                    .add_group(group, items.iter().cloned(), selected)
                    .map_err(|error| self.canonical_build_error(error))?;
                for item in items {
                    item_nodes.insert(item.clone(), leaf);
                }
                Ok(leaf)
            }
            DockspaceTopologyNode::Split {
                axis,
                children,
                fractions,
                ..
            } => {
                let mut built = children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        self.build_canonical_node(
                            assembly,
                            space,
                            &format!("{path}/child-{index}"),
                            child,
                            item_nodes,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut weights = if fractions.is_empty() {
                    vec![1.0; built.len()]
                } else {
                    fractions.clone()
                };
                normalize_split_fractions(&mut weights);

                let mut current = built.pop().expect("validated split has children");
                let mut remaining_weight = weights.pop().expect("weights match children");
                for (child, weight) in built.into_iter().zip(weights).rev() {
                    let total = weight + remaining_weight;
                    let ratio = if total > 0.0 { weight / total } else { 0.5 };
                    current = assembly
                        .add_branch(*axis, child, current, ratio)
                        .map_err(|error| self.canonical_build_error(error))?;
                    remaining_weight = total;
                }
                Ok(current)
            }
        }
    }

    fn canonical_build_error(&self, error: CanonicalGraphError) -> DockspaceScenarioBuildError {
        DockspaceScenarioBuildError::InvalidCanonicalGraph {
            scenario: self.id.clone(),
            error,
        }
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

    fn collect_node_items(node: &DockspaceTopologyNode, items: &mut BTreeSet<DockItemId>) {
        match node {
            DockspaceTopologyNode::Tabs {
                items: node_items, ..
            } => items.extend(node_items.iter().cloned()),
            DockspaceTopologyNode::Split { children, .. } => {
                for child in children {
                    Self::collect_node_items(child, items);
                }
            }
        }
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
