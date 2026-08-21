//! Feature-gated data projection for native dockspace scenario adapters.

use crate::{
    DockDropGuideMetrics, DockItemId, DockLayout, DockSpaceId, DropZone,
    dockspace_testkit::{
        fixtures::load_builtin_scenario_fixtures,
        headless::DockspaceHeadlessRunner,
        ops::{DockspaceScenarioStep, DockspaceScenarioZone},
        topology::{DockspaceAdapterTag, DockspaceOracleKind},
    },
    geometry::{DockDropBoxSet, drop_boxes_with_style},
};
use open_gpui::{Bounds, Pixels, Point};
use thiserror::Error;

/// A data-only native scenario projected from the internal dockspace fixture corpus.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeDockspaceScenario {
    id: String,
    primary_space: DockSpaceId,
    spaces: Vec<DockSpaceId>,
    items: Vec<DockItemId>,
    layout: DockLayout,
    steps: Vec<NativeDockspaceStep>,
    expected_layouts: Vec<DockLayout>,
    oracles: Vec<NativeDockspaceOracle>,
}

impl NativeDockspaceScenario {
    /// Returns the stable fixture id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the space opened through `DockSurface::open_primary_window`.
    pub fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    /// Returns all viewport spaces in fixture order.
    pub fn spaces(&self) -> &[DockSpaceId] {
        &self.spaces
    }

    /// Returns all panel ids in deterministic topology order.
    pub fn items(&self) -> &[DockItemId] {
        &self.items
    }

    /// Returns the production layout seed for the native adapter.
    pub fn layout(&self) -> &DockLayout {
        &self.layout
    }

    /// Returns the ordered user-facing operations in the scenario.
    pub fn steps(&self) -> &[NativeDockspaceStep] {
        &self.steps
    }

    /// Returns the expected production layout after the step at `index` commits.
    pub fn expected_layout_after_step(&self, index: usize) -> Option<&DockLayout> {
        self.expected_layouts.get(index)
    }

    /// Returns the oracle categories required by the fixture.
    pub fn oracles(&self) -> &[NativeDockspaceOracle] {
        &self.oracles
    }
}

/// A single native operation with a stable fixture step id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDockspaceStep {
    id: String,
    source_space: DockSpaceId,
    item: DockItemId,
    target: NativeDockspaceTarget,
}

impl NativeDockspaceStep {
    /// Returns the stable fixture step id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the source viewport space.
    pub fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    /// Returns the panel moved by this step.
    pub fn item(&self) -> &DockItemId {
        &self.item
    }

    /// Returns the semantic target resolved by the native adapter.
    pub fn target(&self) -> &NativeDockspaceTarget {
        &self.target
    }
}

/// A user-facing native docking destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeDockspaceTarget {
    /// Dock relative to a panel in an existing tab stack.
    Item {
        /// Target viewport space.
        space: DockSpaceId,
        /// Panel identifying the target tab stack.
        item: DockItemId,
        /// Production drop-guide zone to activate.
        zone: NativeDockspaceZone,
    },
    /// Dock into a currently empty viewport space.
    EmptySpace {
        /// Target viewport space.
        space: DockSpaceId,
    },
}

impl NativeDockspaceTarget {
    /// Returns the target viewport space.
    pub fn space(&self) -> &DockSpaceId {
        match self {
            Self::Item { space, .. } | Self::EmptySpace { space } => space,
        }
    }
}

/// A drop-guide zone understood by the native adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeDockspaceZone {
    /// Merge into the target tab stack.
    Center,
    /// Split to the target's left.
    Left,
    /// Split to the target's right.
    Right,
    /// Split above the target.
    Top,
    /// Split below the target.
    Bottom,
}

/// Production geometry used to route and verify one native drop-guide zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDockspaceDropGeometry {
    target_point: Point<Pixels>,
    guide_bounds: Bounds<Pixels>,
    preview_bounds: Bounds<Pixels>,
}

impl NativeDockspaceDropGeometry {
    /// Returns a deterministic point inside the production hit region.
    pub fn target_point(self) -> Point<Pixels> {
        self.target_point
    }

    /// Returns the exact visible guide rectangle for the active zone.
    pub fn guide_bounds(self) -> Bounds<Pixels> {
        self.guide_bounds
    }

    /// Returns the split or merge preview rectangle derived by the guide geometry.
    pub fn preview_bounds(self) -> Bounds<Pixels> {
        self.preview_bounds
    }
}

impl NativeDockspaceZone {
    /// Returns the production hit, guide, and preview geometry for this zone.
    pub fn target_geometry(self, bounds: Bounds<Pixels>) -> Option<NativeDockspaceDropGeometry> {
        let zone = DropZone::from(self);
        drop_boxes_with_style(
            bounds,
            DockDropBoxSet::Inner,
            DockDropGuideMetrics::default(),
        )
        .into_iter()
        .find(|drop_box| drop_box.kind.zone() == zone)
        .map(|drop_box| NativeDockspaceDropGeometry {
            target_point: drop_box.hit_bounds.center(),
            guide_bounds: drop_box.draw_bounds,
            preview_bounds: drop_box.preview_bounds,
        })
    }

    /// Returns a deterministic point inside the production drop-guide hit region.
    pub fn target_point(self, bounds: Bounds<Pixels>) -> Option<Point<Pixels>> {
        self.target_geometry(bounds)
            .map(NativeDockspaceDropGeometry::target_point)
    }
}

impl From<NativeDockspaceZone> for DropZone {
    fn from(zone: NativeDockspaceZone) -> Self {
        match zone {
            NativeDockspaceZone::Center => Self::Center,
            NativeDockspaceZone::Left => Self::Left,
            NativeDockspaceZone::Right => Self::Right,
            NativeDockspaceZone::Top => Self::Top,
            NativeDockspaceZone::Bottom => Self::Bottom,
        }
    }
}

/// An oracle category requested by a native dockspace fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeDockspaceOracle {
    /// Graph topology and item uniqueness.
    Graph,
    /// Serialized layout correctness.
    Layout,
    /// Renderer-submitted preview and final frame evidence.
    Presentation,
    /// Selected tab and target-window focus.
    FocusSelection,
    /// Exact cross-window route and capture lineage.
    ViewportRoute,
    /// Native HWND creation and retirement.
    NativeLifetime,
    /// Measured operation latency and allocation evidence.
    Performance,
}

/// Errors returned while projecting the internal fixture corpus for a native adapter.
#[derive(Debug, Error)]
pub enum NativeDockspaceScenarioError {
    /// The checked-in fixture corpus could not be parsed or validated.
    #[error("failed to load dockspace fixtures: {0}")]
    Fixture(String),
    /// The requested scenario id is absent.
    #[error("dockspace scenario {0} was not found")]
    MissingScenario(String),
    /// The fixture is not approved for native execution.
    #[error("dockspace scenario {0} does not declare the native adapter")]
    NativeAdapterMissing(String),
    /// A native scenario did not select its primary viewport space.
    #[error("dockspace scenario {0} does not declare a primary space")]
    PrimarySpaceMissing(String),
    /// A step has no stable id for native evidence.
    #[error("dockspace scenario {scenario} contains a native step without an id")]
    StepIdMissing {
        /// Scenario containing the invalid step.
        scenario: String,
    },
    /// A fixture operation has no native input adapter yet.
    #[error("dockspace scenario {scenario} step {step} is not supported by the native adapter")]
    UnsupportedStep {
        /// Scenario containing the unsupported step.
        scenario: String,
        /// Stable unsupported step id.
        step: String,
    },
    /// The fixture could not build a canonical production graph.
    #[error("dockspace scenario {scenario} failed to build: {message}")]
    Build {
        /// Scenario that failed to build.
        scenario: String,
        /// Production graph validation diagnostic.
        message: String,
    },
    /// The production headless transaction path rejected one fixture step.
    #[error("dockspace scenario {scenario} failed to replay step {step}: {message}")]
    Replay {
        /// Scenario that failed to replay.
        scenario: String,
        /// Stable step id that failed.
        step: String,
        /// Production transaction diagnostic.
        message: String,
    },
}

/// Loads one native-ready scenario from the checked-in dockspace fixture corpus.
pub fn load_native_dockspace_scenario(
    scenario_id: &str,
) -> Result<NativeDockspaceScenario, NativeDockspaceScenarioError> {
    let scenarios = load_builtin_scenario_fixtures()
        .map_err(|error| NativeDockspaceScenarioError::Fixture(error.to_string()))?;
    let scenario = scenarios
        .into_iter()
        .find(|scenario| scenario.id == scenario_id)
        .ok_or_else(|| NativeDockspaceScenarioError::MissingScenario(scenario_id.to_owned()))?;
    if !scenario.supports_adapter(DockspaceAdapterTag::Native) {
        return Err(NativeDockspaceScenarioError::NativeAdapterMissing(
            scenario.id,
        ));
    }
    let primary_space = scenario
        .primary_space
        .clone()
        .ok_or_else(|| NativeDockspaceScenarioError::PrimarySpaceMissing(scenario.id.clone()))?;
    let seed = scenario
        .build_graph()
        .map_err(|error| NativeDockspaceScenarioError::Build {
            scenario: scenario.id.clone(),
            message: error.to_string(),
        })?;
    let spaces = scenario
        .spaces
        .iter()
        .map(|space| space.id.clone())
        .collect::<Vec<_>>();
    let items = spaces
        .iter()
        .flat_map(|space| seed.graph.collect_items_in_space(space))
        .collect::<Vec<_>>();
    let layout = seed.graph.export_layout();
    let steps = scenario
        .steps
        .iter()
        .map(|step| project_step(&scenario.id, step))
        .collect::<Result<Vec<_>, _>>()?;
    let mut headless = DockspaceHeadlessRunner::from_graph(primary_space.clone(), seed.graph);
    let mut expected_layouts = Vec::with_capacity(scenario.steps.len());
    for step in &scenario.steps {
        headless
            .run_step(step)
            .map_err(|error| NativeDockspaceScenarioError::Replay {
                scenario: scenario.id.clone(),
                step: step.id().unwrap_or("<missing>").to_owned(),
                message: error.to_string(),
            })?;
        expected_layouts.push(headless.graph().export_layout());
    }
    let oracles = scenario
        .oracles()
        .into_iter()
        .map(NativeDockspaceOracle::from)
        .collect();

    Ok(NativeDockspaceScenario {
        id: scenario.id,
        primary_space,
        spaces,
        items,
        layout,
        steps,
        expected_layouts,
        oracles,
    })
}

fn project_step(
    scenario: &str,
    step: &DockspaceScenarioStep,
) -> Result<NativeDockspaceStep, NativeDockspaceScenarioError> {
    let id = step
        .id()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| NativeDockspaceScenarioError::StepIdMissing {
            scenario: scenario.to_owned(),
        })?
        .to_owned();
    let (source_space, item, target) = match step.operation() {
        crate::dockspace_testkit::ops::DockspaceSemanticOperation::MoveItemToItem {
            source_space,
            item,
            target_space,
            target_item,
            zone,
        } => (
            source_space.clone(),
            item.clone(),
            NativeDockspaceTarget::Item {
                space: target_space.clone(),
                item: target_item.clone(),
                zone: NativeDockspaceZone::from(*zone),
            },
        ),
        crate::dockspace_testkit::ops::DockspaceSemanticOperation::MoveItemToEmptySpace {
            source_space,
            item,
            target_space,
        } => (
            source_space.clone(),
            item.clone(),
            NativeDockspaceTarget::EmptySpace {
                space: target_space.clone(),
            },
        ),
        _ => {
            return Err(NativeDockspaceScenarioError::UnsupportedStep {
                scenario: scenario.to_owned(),
                step: id,
            });
        }
    };
    Ok(NativeDockspaceStep {
        id,
        source_space,
        item,
        target,
    })
}

impl From<DockspaceScenarioZone> for NativeDockspaceZone {
    fn from(zone: DockspaceScenarioZone) -> Self {
        match zone {
            DockspaceScenarioZone::Center => Self::Center,
            DockspaceScenarioZone::Left => Self::Left,
            DockspaceScenarioZone::Right => Self::Right,
            DockspaceScenarioZone::Top => Self::Top,
            DockspaceScenarioZone::Bottom => Self::Bottom,
        }
    }
}

impl From<DockspaceOracleKind> for NativeDockspaceOracle {
    fn from(oracle: DockspaceOracleKind) -> Self {
        match oracle {
            DockspaceOracleKind::Graph => Self::Graph,
            DockspaceOracleKind::Layout => Self::Layout,
            DockspaceOracleKind::Presentation => Self::Presentation,
            DockspaceOracleKind::FocusSelection => Self::FocusSelection,
            DockspaceOracleKind::ViewportRoute => Self::ViewportRoute,
            DockspaceOracleKind::NativeLifetime => Self::NativeLifetime,
            DockspaceOracleKind::Performance => Self::Performance,
        }
    }
}
