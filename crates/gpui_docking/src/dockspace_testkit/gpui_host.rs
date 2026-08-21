use crate::{
    DockActionOutcome, DockController, DockItemId, DockLayout, DockSpaceId,
    DockViewportOpenOutcome, DockViewportOpenStatus, DockViewportRuntimeHandle, DockWorkspace,
    DropZone,
    debug::DockDebugRegion,
    dockspace_testkit::{
        headless::{DockspaceHeadlessRunner, DockspaceScenarioRunError},
        ops::{DockspaceScenarioStep, DockspaceScenarioZone},
        topology::{DockspaceScenario, DockspaceScenarioBuildError},
    },
    host_test_support::{debug_bounds, selector_for, test_view},
    host_viewport_runtime_test_support::{
        DockCrossWindowVisualDragFixture, unfocused_viewport_window_options,
    },
};
use open_gpui::{AppContext as _, TestAppContext, VisualTestContext, WindowId};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub(crate) struct DockspaceGpuiHostRunner {
    controller: open_gpui::Entity<DockController>,
    runtime: DockViewportRuntimeHandle,
    expected_layouts: Vec<DockLayout>,
    viewport_windows: BTreeMap<DockSpaceId, WindowId>,
}

#[derive(Debug, Error)]
pub(crate) enum DockspaceGpuiHostRunError {
    #[error(transparent)]
    Build(#[from] DockspaceScenarioBuildError),
    #[error(transparent)]
    Headless(#[from] DockspaceScenarioRunError),
    #[error("dockspace scenario {0} has no primary or declared space")]
    MissingPrimarySpace(String),
    #[error("dockspace scenario {scenario} step {step} is not supported by the GPUI host adapter")]
    UnsupportedStep { scenario: String, step: String },
    #[error("dockspace scenario {scenario} step {step} cannot find item {item} in space {space}")]
    MissingItem {
        scenario: String,
        step: String,
        space: DockSpaceId,
        item: DockItemId,
    },
}

impl DockspaceGpuiHostRunner {
    pub(crate) fn from_scenario(
        cx: &mut TestAppContext,
        scenario: &DockspaceScenario,
    ) -> Result<Self, DockspaceGpuiHostRunError> {
        let primary_space = scenario
            .primary_space
            .clone()
            .or_else(|| scenario.spaces.first().map(|space| space.id.clone()))
            .ok_or_else(|| DockspaceGpuiHostRunError::MissingPrimarySpace(scenario.id.clone()))?;

        let mut headless = DockspaceHeadlessRunner::from_scenario(scenario, primary_space.clone())?;
        let mut expected_layouts = Vec::with_capacity(scenario.steps.len());
        for step in &scenario.steps {
            headless.run_step(step)?;
            expected_layouts.push(headless.graph().export_layout());
        }

        let graph = scenario.build_graph()?.graph;
        let mut workspace = DockWorkspace::new(primary_space, graph);
        workspace.policy_mut().set_allow_floating(true);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let items = scenario
            .spaces
            .iter()
            .flat_map(|space| workspace.graph().collect_items_in_space(&space.id))
            .collect::<BTreeSet<_>>();
        for item in items {
            workspace.register_panel_view(
                item.clone(),
                item.as_str().to_owned(),
                test_view(cx, "Scenario panel"),
            );
        }

        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let context = format!("scenario {} initialization", scenario.id);
        let mut viewport_windows = BTreeMap::new();
        let mut unique_window_ids = BTreeSet::new();
        for space in scenario.spaces.iter().map(|space| space.id.clone()) {
            let opened = open_scenario_viewport(cx, &runtime, &space, &context);
            assert_eq!(
                opened.status(),
                DockViewportOpenStatus::Opened,
                "{context}: viewport {space} should open exactly once during initialization"
            );
            assert_eq!(
                opened.space(),
                &space,
                "{context}: opened viewport should report its logical dock space"
            );
            let window_id = opened.window().window_id();
            assert!(
                unique_window_ids.insert(window_id),
                "{context}: viewport {space} should own a distinct GPUI WindowId"
            );
            let previous = viewport_windows.insert(space.clone(), window_id);
            assert!(
                previous.is_none(),
                "{context}: logical dock space {space} should be declared once"
            );
        }
        cx.run_until_parked();

        let runner = Self {
            controller,
            runtime,
            expected_layouts,
            viewport_windows,
        };
        runner.assert_registered_viewport_spaces(&context);
        Ok(runner)
    }

    pub(crate) fn run_all(
        &self,
        cx: &mut TestAppContext,
        scenario: &DockspaceScenario,
    ) -> Result<Vec<DockActionOutcome>, DockspaceGpuiHostRunError> {
        scenario
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| self.run_step(cx, scenario, index, step))
            .collect()
    }

    pub(crate) fn graph_layout(&self, cx: &TestAppContext) -> DockLayout {
        cx.read_entity(&self.controller, |controller, _| {
            controller.graph().export_layout()
        })
    }

    fn run_step(
        &self,
        cx: &mut TestAppContext,
        scenario: &DockspaceScenario,
        index: usize,
        step: &DockspaceScenarioStep,
    ) -> Result<DockActionOutcome, DockspaceGpuiHostRunError> {
        let step_id = step.id().unwrap_or("<missing>").to_owned();
        if !step.supports_gpui_host() {
            return Err(DockspaceGpuiHostRunError::UnsupportedStep {
                scenario: scenario.id.clone(),
                step: step_id,
            });
        }
        let crate::dockspace_testkit::ops::DockspaceSemanticOperation::MoveItemToItem {
            source_space,
            item,
            target_space,
            target_item,
            zone,
        } = step.operation()
        else {
            unreachable!("GPUI host support is limited to cross-space named-item moves");
        };

        let source_tabs = self.find_item_tabs(scenario, &step_id, source_space, item, cx)?;
        let target_tabs = self.find_item_tabs(scenario, &step_id, target_space, target_item, cx)?;
        let fixture = DockCrossWindowVisualDragFixture::open(
            cx,
            &self.runtime,
            source_space.clone(),
            unfocused_viewport_window_options(),
            target_space.clone(),
            unfocused_viewport_window_options(),
            &format!("scenario {} step {step_id}", scenario.id),
        );
        let context = format!("scenario {} step {step_id}", scenario.id);
        self.assert_reused_viewport(source_space, &fixture.source.opened, "source", &context);
        self.assert_reused_viewport(target_space, &fixture.target.opened, "target", &context);
        let target_window = fixture.target.window();
        let drop_zone = DropZone::from(*zone);
        let preview_context = context.clone();
        let assert_preview = move |target_host: &open_gpui::Entity<crate::DockHost>,
                                   cx: &mut TestAppContext| {
            let mut visual = VisualTestContext::from_window(target_window, cx);
            assert_rendered_region(
                &mut visual,
                target_host,
                DockDebugRegion::DropPreview,
                &preview_context,
            );
            assert_rendered_region(
                &mut visual,
                target_host,
                DockDebugRegion::ActiveDropGuide {
                    node: Some(target_tabs),
                    zone: drop_zone,
                },
                &preview_context,
            );
        };

        match zone {
            DockspaceScenarioZone::Center => fixture.drag_source_tab_to_target_center_with_hover(
                cx,
                source_tabs,
                item.clone(),
                target_tabs,
                &context,
                assert_preview,
            ),
            DockspaceScenarioZone::Left
            | DockspaceScenarioZone::Right
            | DockspaceScenarioZone::Top
            | DockspaceScenarioZone::Bottom => fixture
                .drag_source_tab_to_target_inner_edge_with_hover(
                    cx,
                    source_tabs,
                    item.clone(),
                    target_tabs,
                    drop_zone,
                    &context,
                    assert_preview,
                ),
        }
        fixture.assert_drop_previews_cleared(cx, &context);

        let expected_layout = &self.expected_layouts[index];
        assert_eq!(
            &self.graph_layout(cx),
            expected_layout,
            "{context}: GPUI host drag must match the headless checkpoint"
        );
        let moved_tabs = self.find_item_tabs(scenario, &step_id, target_space, item, cx)?;
        cx.read_entity(&self.controller, |controller, _| {
            assert_eq!(
                controller.graph().selected_item_in_tabs(moved_tabs),
                Some(item.clone()),
                "{context}: committed destination should select the moved item"
            );
        });
        let mut target_visual = fixture.target.visual(cx);
        assert_rendered_region(
            &mut target_visual,
            &fixture.target.host,
            DockDebugRegion::Panel { item: item.clone() },
            &context,
        );
        self.assert_persistent_viewports(cx, &context);
        Ok(DockActionOutcome::Changed)
    }

    fn assert_reused_viewport(
        &self,
        space: &DockSpaceId,
        opened: &DockViewportOpenOutcome,
        role: &str,
        context: &str,
    ) {
        let expected_window_id = self.expected_window_id(space, context);
        assert_eq!(
            opened.status(),
            DockViewportOpenStatus::Reused,
            "{context}: {role} viewport {space} should reuse its persistent GPUI window"
        );
        assert_eq!(
            opened.space(),
            space,
            "{context}: {role} viewport should preserve its logical dock space"
        );
        assert_eq!(
            opened.window().window_id(),
            expected_window_id,
            "{context}: {role} viewport {space} should preserve its initial WindowId"
        );
    }

    fn assert_persistent_viewports(&self, cx: &mut TestAppContext, context: &str) {
        self.assert_registered_viewport_spaces(context);
        for space in self.viewport_windows.keys() {
            assert!(
                self.runtime.is_viewport_open(space),
                "{context}: viewport {space} should remain registered after the drag step"
            );
            let reopened = open_scenario_viewport(cx, &self.runtime, space, context);
            self.assert_reused_viewport(space, &reopened, "persistent", context);
        }
        cx.run_until_parked();
        self.assert_registered_viewport_spaces(context);
    }

    fn assert_registered_viewport_spaces(&self, context: &str) {
        let expected = self.viewport_windows.keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            self.runtime.registered_viewport_spaces(),
            expected,
            "{context}: runtime viewport registrations should exactly match the scenario spaces"
        );
    }

    fn expected_window_id(&self, space: &DockSpaceId, context: &str) -> WindowId {
        self.viewport_windows
            .get(space)
            .copied()
            .unwrap_or_else(|| {
                panic!("{context}: scenario viewport {space} should have an initial WindowId")
            })
    }

    fn find_item_tabs(
        &self,
        scenario: &DockspaceScenario,
        step: &str,
        space: &DockSpaceId,
        item: &DockItemId,
        cx: &TestAppContext,
    ) -> Result<crate::DockNodeId, DockspaceGpuiHostRunError> {
        cx.read_entity(&self.controller, |controller, _| {
            controller
                .graph()
                .find_item_in_space(space, item)
                .map(|found| found.0)
        })
        .ok_or_else(|| DockspaceGpuiHostRunError::MissingItem {
            scenario: scenario.id.clone(),
            step: step.to_owned(),
            space: space.clone(),
            item: item.clone(),
        })
    }
}

fn open_scenario_viewport(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    space: &DockSpaceId,
    context: &str,
) -> DockViewportOpenOutcome {
    cx.update(|app| {
        runtime.open_viewport_unchecked_policy(
            space.clone(),
            unfocused_viewport_window_options(),
            app,
        )
    })
    .unwrap_or_else(|error| panic!("{context}: viewport {space} should open: {error}"))
}

fn assert_rendered_region(
    visual: &mut VisualTestContext,
    host: &open_gpui::Entity<crate::DockHost>,
    region: DockDebugRegion,
    context: &str,
) {
    let selector = selector_for(visual, host, region)
        .unwrap_or_else(|| panic!("{context}: debug selector should be available"));
    let bounds = debug_bounds(visual, &selector);
    assert!(
        bounds.size.width > open_gpui::px(0.0) && bounds.size.height > open_gpui::px(0.0),
        "{context}: rendered region {selector} should have non-empty bounds"
    );
}
