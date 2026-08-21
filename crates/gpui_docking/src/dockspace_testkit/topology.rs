use crate::{
    DockCentralRegion, DockFloatingContainer, DockGraph, DockGraphValidationError, DockItemId,
    DockLayoutRect, DockNode, DockNodeId, DockSpaceId, SplitAxis,
};
use open_gpui_ui_core::normalize_split_fractions;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceAdapterTag {
    Headless,
    GpuiHost,
    Native,
    Benchmark,
}

impl DockspaceAdapterTag {
    pub(crate) const fn fixture_name(&self) -> &'static str {
        match self {
            Self::Headless => "headless",
            Self::GpuiHost => "GPUI host",
            Self::Native => "native",
            Self::Benchmark => "benchmark",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceOracleKind {
    Graph,
    Layout,
    Presentation,
    FocusSelection,
    ViewportRoute,
    NativeLifetime,
    Performance,
}

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
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum DockspaceTopologyNode {
    Tabs {
        items: Vec<DockItemId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected: Option<DockItemId>,
    },
    Split {
        axis: SplitAxis,
        children: Vec<DockspaceTopologyNode>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fractions: Vec<f32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DockspaceFloatingSpec {
    pub(crate) root: DockspaceTopologyNode,
    pub(crate) bounds: DockLayoutRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DockspaceScenarioCheckpoint {
    pub(crate) after_step: String,
    pub(crate) spaces: Vec<DockspaceTopologySpace>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DockspaceScenario {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) primary_space: Option<DockSpaceId>,
    pub(crate) spaces: Vec<DockspaceTopologySpace>,
    #[serde(default)]
    pub(crate) steps: Vec<crate::dockspace_testkit::ops::DockspaceScenarioStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) checkpoints: Vec<DockspaceScenarioCheckpoint>,
    #[serde(default)]
    pub(crate) adapters: Vec<DockspaceAdapterTag>,
    #[serde(default)]
    pub(crate) oracles: Vec<DockspaceOracleKind>,
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
            selected: items.first().cloned(),
            items,
        }
    }

    pub(crate) fn tabs_selected(
        items: impl IntoIterator<Item = impl Into<DockItemId>>,
        selected: impl Into<DockItemId>,
    ) -> Self {
        Self::Tabs {
            items: items.into_iter().map(Into::into).collect(),
            selected: Some(selected.into()),
        }
    }

    pub(crate) fn split(
        axis: SplitAxis,
        children: impl IntoIterator<Item = DockspaceTopologyNode>,
    ) -> Self {
        Self::Split {
            axis,
            children: children.into_iter().collect(),
            fractions: Vec::new(),
        }
    }
}

impl DockspaceFloatingSpec {
    pub(crate) fn new(root: DockspaceTopologyNode, bounds: DockLayoutRect) -> Self {
        Self { root, bounds }
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
            adapters: Vec::new(),
            oracles: Vec::new(),
            imgui_categories: Vec::new(),
        }
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
            DockspaceTopologyNode::Tabs { items, selected } => {
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
            DockspaceTopologyNode::Tabs { items, selected } => {
                Ok(graph.insert_node(DockNode::Tabs {
                    items: items.clone(),
                    selected: selected.clone().or_else(|| items.first().cloned()),
                }))
            }
            DockspaceTopologyNode::Split {
                axis,
                children,
                fractions,
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
