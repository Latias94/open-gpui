use crate::{
    DockController, DockDropDelivery, DockFloatingContainer, DockGraph, DockHost, DockItemId,
    DockNode, DockNodeId, DockSpaceId, DockViewportClosePolicy, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteRequest, DockViewportInputStatus,
    DockViewportOpenOutcome, DockViewportRuntime, DockViewportRuntimeHandle,
    DockViewportShouldCloseStatus, DockViewportTargetContext, DockViewportTearOffRequest,
    DockViewportWindowFacts, DockWorkspace, DropZone, SplitAxis,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_runtime::DockHostDropSceneFact,
    drop_target::DockLeafDropTarget,
    host_test_support::*,
    interaction::{DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton, Pixels, Point,
    TestAppContext, VisualTestContext, WindowBounds, WindowId, WindowOptions, point, px,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct DockViewportRuntimeFixture {
    pub(crate) controller: Entity<DockController>,
    pub(crate) runtime: DockViewportRuntimeHandle,
    tabs_by_space: BTreeMap<DockSpaceId, DockNodeId>,
}

pub(crate) struct DockViewportControllerFixture {
    pub(crate) controller: Entity<DockController>,
    tabs_by_space: BTreeMap<DockSpaceId, DockNodeId>,
}

pub(crate) struct DockViewportRuntimeFixtureBuilder {
    primary_space: DockSpaceId,
    spaces: Vec<DockViewportTabsSpec>,
    focusable_items: BTreeSet<&'static str>,
    close_policy: Option<DockViewportClosePolicy>,
    allow_platform_viewports: bool,
}

struct DockViewportTabsSpec {
    space: DockSpaceId,
    items: Vec<&'static str>,
    selected: Option<&'static str>,
}

pub(crate) struct DockCrossWindowVisualDragFixture {
    pub(crate) source: DockViewportVisualHostFixture,
    pub(crate) target: DockViewportVisualHostFixture,
}

pub(crate) struct DockViewportVisualHostFixture {
    pub(crate) opened: DockViewportOpenOutcome,
    pub(crate) host: Entity<DockHost>,
}

pub(crate) struct DockViewportHostSceneSeed {
    space: DockSpaceId,
    window: AnyWindowHandle,
    root: DockNodeId,
    target_tabs: DockNodeId,
    window_bounds: WindowBounds,
    host_bounds: Bounds<Pixels>,
    host_position: Point<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockCrossWindowDragRelease {
    Hold,
    Release,
}

impl DockViewportRuntimeFixture {
    pub(crate) fn builder(
        primary_space: impl Into<DockSpaceId>,
    ) -> DockViewportRuntimeFixtureBuilder {
        DockViewportRuntimeFixtureBuilder {
            primary_space: primary_space.into(),
            spaces: Vec::new(),
            focusable_items: BTreeSet::new(),
            close_policy: None,
            allow_platform_viewports: false,
        }
    }

    pub(crate) fn open_unfocused_viewport(
        &self,
        cx: &mut TestAppContext,
        space: &DockSpaceId,
    ) -> DockViewportOpenOutcome {
        self.open_viewport(cx, space, unfocused_viewport_window_options())
    }

    pub(crate) fn tabs(&self, space: &DockSpaceId) -> DockNodeId {
        tabs_for_space(&self.tabs_by_space, space)
    }

    pub(crate) fn open_viewport(
        &self,
        cx: &mut TestAppContext,
        space: &DockSpaceId,
        options: WindowOptions,
    ) -> DockViewportOpenOutcome {
        cx.update(|app| {
            self.runtime
                .open_viewport_unchecked_policy(space.clone(), options, app)
        })
        .unwrap_or_else(|error| panic!("test viewport {space} should open: {error}"))
    }
}

impl DockCrossWindowVisualDragFixture {
    pub(crate) fn open(
        cx: &mut TestAppContext,
        runtime: &DockViewportRuntimeHandle,
        source_space: DockSpaceId,
        source_options: WindowOptions,
        target_space: DockSpaceId,
        target_options: WindowOptions,
        context: &str,
    ) -> Self {
        let source = DockViewportVisualHostFixture::open(
            cx,
            runtime,
            source_space,
            source_options,
            "source",
            context,
        );
        let target = DockViewportVisualHostFixture::open(
            cx,
            runtime,
            target_space,
            target_options,
            "target",
            context,
        );
        cx.run_until_parked();

        Self { source, target }
    }

    pub(crate) fn drag_source_tab_to_target_inner_edge(
        &self,
        cx: &mut TestAppContext,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_tabs: DockNodeId,
        zone: DropZone,
        release: DockCrossWindowDragRelease,
        context: &str,
    ) {
        self.drag_source_tab_to_target_region(
            cx,
            source_tabs,
            item,
            DockDebugRegion::Tabs { node: target_tabs },
            |bounds| inner_edge_drop_position(bounds, zone),
            release,
            context,
            |_, _| {},
        );
    }

    pub(crate) fn drag_source_tab_to_target_inner_edge_with_hover(
        &self,
        cx: &mut TestAppContext,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_tabs: DockNodeId,
        zone: DropZone,
        context: &str,
        before_release: impl FnOnce(&Entity<DockHost>, &mut TestAppContext),
    ) {
        self.drag_source_tab_to_target_region(
            cx,
            source_tabs,
            item,
            DockDebugRegion::Tabs { node: target_tabs },
            |bounds| inner_edge_drop_position(bounds, zone),
            DockCrossWindowDragRelease::Release,
            context,
            before_release,
        );
    }

    pub(crate) fn drag_source_tab_to_target_center(
        &self,
        cx: &mut TestAppContext,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_tabs: DockNodeId,
        release: DockCrossWindowDragRelease,
        context: &str,
    ) {
        self.drag_source_tab_to_target_region(
            cx,
            source_tabs,
            item,
            DockDebugRegion::Tabs { node: target_tabs },
            center_drop_position,
            release,
            context,
            |_, _| {},
        );
    }

    pub(crate) fn assert_drop_previews_cleared(&self, cx: &mut TestAppContext, context: &str) {
        assert!(
            !self.target.has_drop_preview(cx),
            "{context}: target drop preview should clear"
        );
        assert!(
            !self.source.has_drop_preview(cx),
            "{context}: source drop preview should clear"
        );
    }

    fn drag_source_tab_to_target_region(
        &self,
        cx: &mut TestAppContext,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_region: DockDebugRegion,
        target_position: impl FnOnce(Bounds<Pixels>) -> Point<Pixels>,
        release: DockCrossWindowDragRelease,
        context: &str,
        before_release: impl FnOnce(&Entity<DockHost>, &mut TestAppContext),
    ) {
        let mut source_visual = self.source.visual(cx);
        let mut target_visual = self.target.visual(cx);

        let source_tab = selector_for(
            &source_visual,
            &self.source.host,
            DockDebugRegion::Tab {
                tabs: source_tabs,
                item,
            },
        )
        .unwrap_or_else(|| panic!("{context}: source tab selector should be emitted"));
        let target_selector = selector_for(&target_visual, &self.target.host, target_region)
            .unwrap_or_else(|| panic!("{context}: target selector should be emitted"));

        let start = debug_bounds(&mut source_visual, &source_tab).center();
        let threshold = point(start.x + px(24.0), start.y);
        let end = target_position(debug_bounds(&mut target_visual, &target_selector));

        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        before_release(&self.target.host, cx);
        if release == DockCrossWindowDragRelease::Release {
            target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        }
        cx.run_until_parked();
    }
}

impl DockViewportVisualHostFixture {
    fn open(
        cx: &mut TestAppContext,
        runtime: &DockViewportRuntimeHandle,
        space: DockSpaceId,
        options: WindowOptions,
        role: &str,
        context: &str,
    ) -> Self {
        let opened = cx
            .update(|app| runtime.open_viewport_unchecked_policy(space.clone(), options, app))
            .unwrap_or_else(|error| panic!("{context}: {role} viewport should open: {error}"));
        let window = opened
            .window()
            .downcast::<DockHost>()
            .unwrap_or_else(|| panic!("{context}: {role} viewport should render DockHost"));
        let host = window
            .root(cx)
            .unwrap_or_else(|_| panic!("{context}: {role} viewport should expose DockHost root"));

        Self { opened, host }
    }

    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.opened.window()
    }

    pub(crate) fn visual(&self, cx: &mut TestAppContext) -> VisualTestContext {
        VisualTestContext::from_window(self.window(), cx)
    }

    pub(crate) fn has_drop_preview(&self, cx: &mut TestAppContext) -> bool {
        let visual = self.visual(cx);
        selector_for(&visual, &self.host, DockDebugRegion::DropPreview).is_some()
    }
}

impl DockViewportHostSceneSeed {
    pub(crate) fn new(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        tabs: DockNodeId,
    ) -> Self {
        let host_bounds = Self::default_host_bounds();
        Self {
            space: space.into(),
            window,
            root: tabs,
            target_tabs: tabs,
            window_bounds: Self::default_window_bounds(),
            host_bounds,
            host_position: center_drop_position(host_bounds),
        }
    }

    pub(crate) fn default_window_bounds() -> WindowBounds {
        WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0))
    }

    fn default_host_bounds() -> Bounds<Pixels> {
        floating_bounds(0.0, 0.0, 360.0, 220.0)
    }

    pub(crate) fn with_root(mut self, root: DockNodeId) -> Self {
        self.root = root;
        self
    }

    pub(crate) fn with_window_bounds(mut self, window_bounds: WindowBounds) -> Self {
        self.window_bounds = window_bounds;
        self
    }

    pub(crate) fn with_host_position(mut self, host_position: Point<Pixels>) -> Self {
        self.host_position = host_position;
        self
    }

    pub(crate) fn host_position(&self) -> Point<Pixels> {
        self.host_position
    }

    pub(crate) fn screen_position(&self) -> Point<Pixels> {
        screen_position_for_host_position(self.window_bounds, self.host_position)
    }

    pub(crate) fn publish(&self, runtime: &DockViewportRuntimeHandle) {
        let window_id = self.window.window_id();
        assert!(runtime.begin_viewport_host_scene(
            self.space.clone(),
            window_id,
            DockViewportWindowFacts::from_window_bounds(self.window_bounds),
            self.host_bounds,
            self.host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &self.space,
            window_id,
            leaf_host_scene_fact(self.root, self.target_tabs),
        ));
    }

    pub(crate) fn publish_runtime(&self, runtime: &mut DockViewportRuntime) {
        let window_id = self.window.window_id();
        assert!(runtime.begin_viewport_host_scene(
            self.space.clone(),
            window_id,
            DockViewportWindowFacts::from_window_bounds(self.window_bounds),
            self.host_bounds,
            self.host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &self.space,
            window_id,
            leaf_host_scene_fact(self.root, self.target_tabs),
        ));
    }

    pub(crate) fn begin_empty_runtime_frame(&self, runtime: &mut DockViewportRuntime) {
        assert!(runtime.begin_viewport_host_scene(
            self.space.clone(),
            self.window.window_id(),
            DockViewportWindowFacts::from_window_bounds(self.window_bounds),
            self.host_bounds,
            self.host_position,
        ));
    }
}

impl DockViewportControllerFixture {
    pub(crate) fn tabs(&self, space: &DockSpaceId) -> DockNodeId {
        tabs_for_space(&self.tabs_by_space, space)
    }
}

impl DockViewportRuntimeFixtureBuilder {
    pub(crate) fn space(
        mut self,
        space: impl Into<DockSpaceId>,
        items: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        let items = items.into_iter().collect::<Vec<_>>();
        let selected = items.first().copied();
        self.spaces.push(DockViewportTabsSpec {
            space: space.into(),
            items,
            selected,
        });
        self
    }

    pub(crate) fn space_selected(
        mut self,
        space: impl Into<DockSpaceId>,
        items: impl IntoIterator<Item = &'static str>,
        selected: &'static str,
    ) -> Self {
        let items = items.into_iter().collect::<Vec<_>>();
        assert!(
            items.contains(&selected),
            "selected tab {selected} must be present in test fixture items"
        );
        self.spaces.push(DockViewportTabsSpec {
            space: space.into(),
            items,
            selected: Some(selected),
        });
        self
    }

    pub(crate) fn focusable_panel(mut self, item: &'static str) -> Self {
        self.focusable_items.insert(item);
        self
    }

    pub(crate) fn close_policy(mut self, close_policy: DockViewportClosePolicy) -> Self {
        self.close_policy = Some(close_policy);
        self
    }

    pub(crate) fn allow_platform_viewports(mut self, allowed: bool) -> Self {
        self.allow_platform_viewports = allowed;
        self
    }

    pub(crate) fn build_controller(self, cx: &mut TestAppContext) -> DockViewportControllerFixture {
        let mut graph = DockGraph::new();
        let mut panels = BTreeSet::new();
        let mut tabs_by_space = BTreeMap::new();

        for spec in &self.spaces {
            let items: Vec<DockItemId> = spec.items.iter().copied().map(item).collect();
            let selected = spec.selected.map(item);
            let tabs = graph.insert_node(DockNode::Tabs { items, selected });
            graph.set_root(spec.space.clone(), tabs);
            tabs_by_space.insert(spec.space.clone(), tabs);
            panels.extend(spec.items.iter().copied());
        }

        let mut workspace = DockWorkspace::new(self.primary_space, graph);
        workspace
            .policy_mut()
            .set_allow_platform_viewports(self.allow_platform_viewports);
        for panel in panels {
            let title = format!("Panel {panel}");
            if self.focusable_items.contains(panel) {
                workspace.register_focusable_panel_view(item(panel), title, test_view(cx, panel));
            } else {
                workspace.register_panel_view(item(panel), title, test_view(cx, panel));
            }
        }

        let controller = cx.new(|_| DockController::new(workspace));

        DockViewportControllerFixture {
            controller,
            tabs_by_space,
        }
    }

    pub(crate) fn build(self, cx: &mut TestAppContext) -> DockViewportRuntimeFixture {
        let close_policy = self.close_policy.clone();
        let controller_fixture = self.build_controller(cx);
        let runtime = match close_policy {
            Some(close_policy) => DockViewportRuntimeHandle::with_close_policy(
                controller_fixture.controller.clone(),
                close_policy,
            ),
            None => DockViewportRuntimeHandle::new(controller_fixture.controller.clone()),
        };

        DockViewportRuntimeFixture {
            controller: controller_fixture.controller,
            runtime,
            tabs_by_space: controller_fixture.tabs_by_space,
        }
    }
}

fn tabs_for_space(
    tabs_by_space: &BTreeMap<DockSpaceId, DockNodeId>,
    space: &DockSpaceId,
) -> DockNodeId {
    *tabs_by_space
        .get(space)
        .unwrap_or_else(|| panic!("test fixture has no tabs for dock space {space}"))
}

pub(crate) fn unfocused_viewport_window_options() -> WindowOptions {
    WindowOptions {
        focus: false,
        ..viewport_window_options(360.0, 220.0)
    }
}

pub(crate) fn tear_off_request(
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    item: DockItemId,
) -> DockViewportTearOffRequest {
    DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item),
        point(px(900.0), px(900.0)),
        None,
    )
}

pub(crate) fn viewport_input_status(
    runtime: &DockViewportRuntimeHandle,
    space: &DockSpaceId,
) -> Option<DockViewportInputStatus> {
    runtime
        .runtime_status()
        .viewport_lifecycle
        .iter()
        .find(|record| &record.space == space)
        .map(|record| record.input_status)
}

pub(crate) fn leaf_host_scene_fact(
    root: DockNodeId,
    target_tabs: DockNodeId,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
        root,
        target_tabs,
        bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
        is_central: false,
    })
}

pub(crate) fn horizontal_split_floating_graph(
    primary_space: DockSpaceId,
    root_items: Option<&[&'static str]>,
) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    if let Some(root_items) = root_items {
        let items: Vec<DockItemId> = root_items.iter().copied().map(item).collect();
        let selected = items.first().cloned();
        let root = graph.insert_node(DockNode::Tabs { items, selected });
        graph.set_root(primary_space.clone(), root);
    }

    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_tabs],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(primary_space)
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 260.0, 150.0),
        });

    (graph, floating)
}

pub(crate) fn target_center_host_position() -> open_gpui::Point<open_gpui::Pixels> {
    center_drop_position(floating_bounds(0.0, 0.0, 360.0, 220.0))
}

pub(crate) fn assert_tabs_node_items(
    graph: &DockGraph,
    tabs: DockNodeId,
    expected_items: &[DockItemId],
    context: &str,
) {
    let DockNode::Tabs { items, selected } = graph
        .node(tabs)
        .unwrap_or_else(|| panic!("{context}: tabs node should exist"))
    else {
        panic!("{context}: node should be tabs");
    };
    assert_eq!(items.as_slice(), expected_items, "{context}");
    assert_eq!(selected.as_ref(), expected_items.first(), "{context}");
}

pub(crate) fn hovered_window_route_request_for_test(
    source_space: impl Into<DockSpaceId>,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    suggested_window_bounds: Option<WindowBounds>,
    hovered_window: AnyWindowHandle,
    release_origin: DockPayloadDropReleaseOrigin,
) -> DockViewportDropRouteRequest {
    DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_node,
        payload,
        release_position,
        suggested_window_bounds,
        crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(hovered_window),
        )
        .with_event_receiver_window(hovered_window)
        .with_global_window_bounds(true),
        release_origin,
    )
}

pub(crate) fn freeze_should_close_plan(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    window_id: WindowId,
) {
    let should_close = cx.update(|app| runtime.handle_window_should_close_with_app(window_id, app));
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
}

pub(crate) fn cache_known_viewport_preview_for_test(
    runtime: &mut DockViewportRuntime,
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    target_space: &DockSpaceId,
    target_window: AnyWindowHandle,
    target_tabs: DockNodeId,
    cx: &mut TestAppContext,
) -> crate::interaction::DockRuntimeDragSession {
    let host_scene =
        DockViewportHostSceneSeed::new(target_space.clone(), target_window, target_tabs);
    let release_position = host_scene.screen_position();
    host_scene.publish_runtime(runtime);

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let request = hovered_window_route_request_for_test(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        target_window,
        DockPayloadDropReleaseOrigin::HoveredHost,
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    let update = runtime.update_routed_drop_preview(&resolution, &payload);
    assert!(update.changed());

    session
}

pub(crate) fn close_window_quietly_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
}

pub(crate) fn focus_backend_window_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("test viewport should activate");
}

pub(crate) fn seed_runtime_host_scene_for_test(
    runtime: &DockViewportRuntimeHandle,
    space: &DockSpaceId,
    window: AnyWindowHandle,
    tabs: DockNodeId,
) {
    DockViewportHostSceneSeed::new(space.clone(), window, tabs).publish(runtime);
}

pub(crate) fn cache_known_viewport_preview(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    hovered_window: impl Into<open_gpui::AnyWindowHandle>,
    drag_session: Option<DockRuntimeDragSession>,
    payload_title: &str,
) -> crate::DockViewportResolvedDropRoute {
    let drag_payload = DockDragPayload::new_item(
        source_space.clone(),
        source_node,
        item("__test__"),
        payload_title.to_string(),
    );
    cache_known_viewport_preview_with_payload(
        cx,
        runtime,
        source_space,
        source_node,
        payload,
        release_position,
        hovered_window,
        drag_session,
        &drag_payload,
    )
}

pub(crate) fn cache_known_viewport_preview_with_payload(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    hovered_window: impl Into<open_gpui::AnyWindowHandle>,
    drag_session: Option<DockRuntimeDragSession>,
    drag_payload: &DockDragPayload,
) -> crate::DockViewportResolvedDropRoute {
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_node,
        payload,
        release_position,
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(hovered_window),
    )
    .with_drag_session(drag_session);
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::KnownViewport { .. }
        ),
        "preview setup should resolve a known viewport route, got {:?}",
        resolution.route()
    );
    let preview_changed =
        cx.update(|app| runtime.update_routed_drop_preview(&resolution, drag_payload, app));
    let preview_target = resolution
        .routed_preview_target_snapshot()
        .expect("known viewport preview should carry a routed preview target");
    let target_space = preview_target.target_space();
    let target_window_id = preview_target
        .target_window_id()
        .expect("known viewport preview should target a window");
    let preview = runtime.routed_drop_preview_for(target_space, target_window_id);
    let _ = (preview_changed, preview);
    resolution
}

pub(crate) fn cache_host_route_preview(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    resolution: &crate::DockViewportResolvedDropRoute,
    payload_title: &str,
    host_space: DockSpaceId,
    host_window_id: open_gpui::WindowId,
    host_position: open_gpui::Point<open_gpui::Pixels>,
) {
    let payload = DockDragPayload::new_item(
        DockSpaceId::from("__test__"),
        Default::default(),
        item("__test__"),
        payload_title.to_string(),
    );
    cache_host_route_preview_with_payload(
        cx,
        runtime,
        resolution,
        &payload,
        host_space,
        host_window_id,
        host_position,
    );
}

pub(crate) fn cache_host_route_preview_with_payload(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    resolution: &crate::DockViewportResolvedDropRoute,
    payload: &DockDragPayload,
    host_space: DockSpaceId,
    host_window_id: open_gpui::WindowId,
    host_position: open_gpui::Point<open_gpui::Pixels>,
) {
    cx.update(|app| {
        runtime.update_host_routed_drop_preview(
            resolution,
            payload,
            host_space,
            host_window_id,
            host_position,
            app,
        );
    });
}

pub(crate) fn fresh_delivery_for_request(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    request: &DockViewportDropRouteRequest,
) -> DockDropDelivery {
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(request, app));
    DockDropDelivery::from_resolution(resolution)
        .expect("fresh current-facts route should mint a delivery")
}

pub(crate) fn backend_route_resolution_fixture(
    cx: &mut TestAppContext,
) -> (
    DockViewportRuntimeHandle,
    open_gpui::AnyWindowHandle,
    DockViewportDropRouteRequest,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
        .space(source_space.clone(), ["a"])
        .space(target_space.clone(), ["b"])
        .build(cx);
    let source_tabs = fixture.tabs(&source_space);
    let runtime = fixture.runtime.clone();
    cx.set_platform_focused_window_available(false);
    let _source_opened = fixture.open_unfocused_viewport(cx, &source_space);
    let target_opened = fixture.open_unfocused_viewport(cx, &target_space);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new(),
    );
    (runtime, target_opened.window(), request)
}
