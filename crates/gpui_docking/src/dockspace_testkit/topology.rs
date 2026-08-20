use crate::{
    DockCentralRegion, DockFloatingContainer, DockGraph, DockGraphValidationError, DockItemId,
    DockLayoutRect, DockNode, DockNodeId, DockSpaceId, SplitAxis, dock_bounds,
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
pub(crate) struct DockspaceScenario {
    pub(crate) id: String,
    pub(crate) spaces: Vec<DockspaceTopologySpace>,
    #[serde(default)]
    pub(crate) steps: Vec<crate::dockspace_testkit::ops::DockspaceScenarioStep>,
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

    pub(crate) fn empty_central(mut self, passthrough_when_empty: bool) -> Self {
        self.keep_central_when_empty = true;
        self.passthrough_when_empty = passthrough_when_empty;
        self
    }
}

impl DockspaceScenario {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            spaces: Vec::new(),
            steps: Vec::new(),
            adapters: Vec::new(),
            oracles: Vec::new(),
            imgui_categories: Vec::new(),
        }
    }

    pub(crate) fn space(mut self, space: DockspaceTopologySpace) -> Self {
        self.spaces.push(space);
        self
    }

    pub(crate) fn step(
        mut self,
        step: crate::dockspace_testkit::ops::DockspaceScenarioStep,
    ) -> Self {
        self.steps.push(step);
        self
    }

    pub(crate) fn adapters(
        mut self,
        adapters: impl IntoIterator<Item = DockspaceAdapterTag>,
    ) -> Self {
        self.adapters = adapters.into_iter().collect();
        self
    }

    pub(crate) fn oracles(
        mut self,
        oracles: impl IntoIterator<Item = DockspaceOracleKind>,
    ) -> Self {
        self.oracles = oracles.into_iter().collect();
        self
    }

    pub(crate) fn imgui_categories(
        mut self,
        categories: impl IntoIterator<Item = ImGuiDockingCategory>,
    ) -> Self {
        self.imgui_categories = categories.into_iter().collect();
        self
    }

    pub(crate) fn build_graph(
        &self,
    ) -> Result<DockspaceScenarioGraph, DockspaceScenarioBuildError> {
        let mut seen_spaces = BTreeSet::new();
        let mut seen_items = BTreeSet::new();
        for space in &self.spaces {
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

        let mut graph = DockGraph::new();
        let mut roots_by_space = HashMap::new();
        for space in &self.spaces {
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

pub(crate) fn complex_tabs_split_float_scenario() -> DockspaceScenario {
    DockspaceScenario::new("tabs_split_float_merge")
        .space(
            DockspaceTopologySpace::new("main")
                .root(DockspaceTopologyNode::split(
                    SplitAxis::Horizontal,
                    [
                        DockspaceTopologyNode::tabs_selected(["AAA", "BBB"], "AAA"),
                        DockspaceTopologyNode::tabs(["CCC"]),
                    ],
                ))
                .floating(DockspaceFloatingSpec::new(
                    DockspaceTopologyNode::tabs(["DDD"]),
                    fixture_bounds(24.0, 32.0, 320.0, 220.0),
                )),
        )
        .adapters([
            DockspaceAdapterTag::Headless,
            DockspaceAdapterTag::GpuiHost,
            DockspaceAdapterTag::Benchmark,
        ])
        .oracles([
            DockspaceOracleKind::Graph,
            DockspaceOracleKind::Layout,
            DockspaceOracleKind::Presentation,
            DockspaceOracleKind::Performance,
        ])
        .imgui_categories([
            ImGuiDockingCategory::DockIntoParent,
            ImGuiDockingCategory::SplitPayload,
            ImGuiDockingCategory::Undock,
        ])
}

pub(crate) fn empty_central_with_floating_scenario() -> DockspaceScenario {
    DockspaceScenario::new("empty_central_with_floating")
        .space(
            DockspaceTopologySpace::new("main")
                .empty_central(false)
                .floating(DockspaceFloatingSpec::new(
                    DockspaceTopologyNode::tabs(["floating"]),
                    fixture_bounds(12.0, 18.0, 260.0, 160.0),
                )),
        )
        .adapters([DockspaceAdapterTag::Headless])
        .oracles([DockspaceOracleKind::Graph, DockspaceOracleKind::Layout])
}

pub(crate) fn three_viewport_rehome_scenario() -> DockspaceScenario {
    use crate::dockspace_testkit::ops::{DockspaceScenarioStep, DockspaceScenarioZone};

    DockspaceScenario::new("three_viewport_rehome")
        .space(DockspaceTopologySpace::new("source").root(DockspaceTopologyNode::tabs(["AAA"])))
        .space(DockspaceTopologySpace::new("middle").root(DockspaceTopologyNode::tabs(["BBB"])))
        .space(
            DockspaceTopologySpace::new("target").root(DockspaceTopologyNode::split(
                SplitAxis::Vertical,
                [
                    DockspaceTopologyNode::tabs(["CCC"]),
                    DockspaceTopologyNode::tabs(["DDD"]),
                ],
            )),
        )
        .step(DockspaceScenarioStep::move_item_to_item(
            "source",
            "AAA",
            "target",
            "CCC",
            DockspaceScenarioZone::Right,
        ))
        .step(DockspaceScenarioStep::move_item_to_empty_space(
            "target", "AAA", "source",
        ))
        .adapters([
            DockspaceAdapterTag::Headless,
            DockspaceAdapterTag::GpuiHost,
            DockspaceAdapterTag::Native,
            DockspaceAdapterTag::Benchmark,
        ])
        .oracles([
            DockspaceOracleKind::Graph,
            DockspaceOracleKind::Layout,
            DockspaceOracleKind::Presentation,
            DockspaceOracleKind::FocusSelection,
            DockspaceOracleKind::ViewportRoute,
            DockspaceOracleKind::NativeLifetime,
            DockspaceOracleKind::Performance,
        ])
        .imgui_categories([
            ImGuiDockingCategory::ViewportFocus,
            ImGuiDockingCategory::ViewportOwnerChange,
            ImGuiDockingCategory::DockOverDockspace,
        ])
}

pub(crate) fn fixture_bounds(x: f32, y: f32, width: f32, height: f32) -> DockLayoutRect {
    DockLayoutRect::from_bounds(dock_bounds(x, y, width, height))
}
