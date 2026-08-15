use crate::{
    DockController, DockFloatingContainer, DockGraph, DockHost, DockItemId, DockNode, DockNodeId,
    DockSpaceId, DockViewportRuntimeHandle, DockWorkspace, DropZone, SplitAxis,
    debug::DockDebugRegion,
    geometry::{self, DockDropBoxKind, DockDropBoxSet},
    presentation_scene::DockPresentationScene,
};
use open_gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, Modifiers, MouseButton, ParentElement, Pixels, Point, Render, Styled,
    TestAppContext, VisualTestContext, Window, WindowBounds, WindowHandle, WindowOptions, div,
    point, px, rgb, size,
};

const SPACE: &str = "main";

pub(crate) struct TestPanel {
    pub(crate) label: &'static str,
    focus_handle: FocusHandle,
}

pub(crate) struct TestDockHostFocusFixture {
    host: Entity<DockHost>,
    external_focus: FocusHandle,
}

impl TestPanel {
    pub(crate) fn new(label: &'static str, cx: &mut Context<Self>) -> Self {
        Self {
            label,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for TestPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let selector = format!("test-panel:{}", self.label);
        div()
            .debug_selector(move || selector)
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(0xffffff))
            .child(self.label)
    }
}

impl Focusable for TestPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestDockHostFocusFixture {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(self.host.clone()).child(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .w(px(0.0))
                .h(px(0.0))
                .track_focus(&self.external_focus),
        )
    }
}

pub(crate) fn space() -> DockSpaceId {
    DockSpaceId::from(SPACE)
}

pub(crate) fn item(id: &str) -> DockItemId {
    DockItemId::from(id)
}

pub(crate) fn tabs_graph(items: &[&str]) -> (DockGraph, DockNodeId) {
    tabs_graph_with_optional_selected(items, items.first().copied())
}

pub(crate) fn tabs_graph_with_selected(items: &[&str], selected: &str) -> (DockGraph, DockNodeId) {
    tabs_graph_with_optional_selected(items, Some(selected))
}

fn tabs_graph_with_optional_selected(
    items: &[&str],
    selected: Option<&str>,
) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    let items: Vec<DockItemId> = items.iter().copied().map(DockItemId::from).collect();
    let selected = selected.map(DockItemId::from);
    let root = graph.insert_node(DockNode::Tabs { items, selected });
    graph.set_root(space(), root);
    (graph, root)
}

pub(crate) fn split_graph(
    axis: SplitAxis,
    first_fraction: f32,
    second_fraction: f32,
) -> (DockGraph, DockNodeId, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let first = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let second = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis,
        children: vec![first, second],
        fractions: vec![first_fraction, second_fraction],
    });
    graph.set_root(space(), root);
    (graph, root, first, second)
}

pub(crate) fn floating_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

pub(crate) fn viewport_window_options(width: f32, height: f32) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, width, height,
        ))),
        focus_on_appearing: false,
        ..Default::default()
    }
}

pub(crate) fn floating_overlay_graph() -> (DockGraph, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), root);
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 220.0, 140.0),
        });
    (graph, root, floating)
}

pub(crate) fn test_view(cx: &mut TestAppContext, label: &'static str) -> Entity<TestPanel> {
    cx.new(|cx| TestPanel::new(label, cx))
}

pub(crate) fn workspace_with_panels(
    cx: &mut TestAppContext,
    graph: DockGraph,
    panels: &[(&str, &str, &'static str)],
) -> DockWorkspace {
    let mut workspace = DockWorkspace::new(space(), graph);
    for (id, title, label) in panels {
        workspace.register_panel_view(item(id), *title, test_view(cx, label));
    }
    workspace
}

pub(crate) fn open_host(
    cx: &mut TestAppContext,
    graph: DockGraph,
    panels: &[(&str, &str, &'static str)],
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let workspace = workspace_with_panels(cx, graph, panels);
    open_workspace(cx, workspace, window_size)
}

pub(crate) fn open_workspace(
    cx: &mut TestAppContext,
    workspace: DockWorkspace,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    open_controller_space(cx, controller, dock_space, window_size)
}

pub(crate) fn open_workspace_with_external_focus(
    cx: &mut TestAppContext,
    workspace: DockWorkspace,
    window_size: open_gpui::Size<Pixels>,
) -> (
    WindowHandle<TestDockHostFocusFixture>,
    Entity<DockHost>,
    VisualTestContext,
    FocusHandle,
) {
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let window = cx.open_window(window_size, move |_, cx| {
        let host = cx.new(|cx| {
            DockHost::from_controller(controller.clone(), dock_space.clone(), runtime.clone(), cx)
        });
        TestDockHostFocusFixture {
            host,
            external_focus: cx.focus_handle(),
        }
    });
    let fixture = window
        .root(cx)
        .expect("window should expose the Dock focus fixture root");
    let (host, external_focus) = cx.read_entity(&fixture, |fixture, _| {
        (fixture.host.clone(), fixture.external_focus.clone())
    });
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    (window, host, visual, external_focus)
}

pub(crate) fn open_controller_workspace(
    cx: &mut TestAppContext,
    controller: Entity<DockController>,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    open_controller_space(cx, controller, space(), window_size)
}

pub(crate) fn open_controller_space(
    cx: &mut TestAppContext,
    controller: Entity<DockController>,
    dock_space: DockSpaceId,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    open_controller_space_with_runtime(cx, controller, runtime, dock_space, window_size)
}

pub(crate) fn open_controller_space_with_runtime(
    cx: &mut TestAppContext,
    controller: Entity<DockController>,
    runtime: DockViewportRuntimeHandle,
    dock_space: DockSpaceId,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let window = cx.open_window(window_size, move |_, cx| {
        DockHost::from_controller(controller.clone(), dock_space, runtime.clone(), cx)
    });
    let host = window.root(cx).expect("window should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    (window, host, visual)
}

pub(crate) fn activate_window_for_pointer_input(visual: &mut VisualTestContext) {
    visual.update(|window, _| {
        let _ = window.activate_window();
    });
    visual.run_until_parked();
}

pub(crate) fn selector_for(
    cx: &VisualTestContext,
    host: &Entity<DockHost>,
    region: DockDebugRegion,
) -> Option<String> {
    cx.read_entity(host, |host, _| {
        host.debug_selector(&region).map(ToString::to_string)
    })
}

pub(crate) fn debug_bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("debug selector {selector} should have bounds"))
}

pub(crate) fn width(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.width)
}

pub(crate) fn height(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.height)
}

pub(crate) fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 2.0,
        "expected {actual} to be within 2px of {expected}"
    );
}

pub(crate) fn assert_bounds_close(actual: Bounds<Pixels>, expected: Bounds<Pixels>, context: &str) {
    assert_close(f32::from(actual.origin.x), f32::from(expected.origin.x));
    assert_close(f32::from(actual.origin.y), f32::from(expected.origin.y));
    assert_close(f32::from(actual.size.width), f32::from(expected.size.width));
    assert_close(
        f32::from(actual.size.height),
        f32::from(expected.size.height),
    );
    assert!(
        expected.contains(&actual.center()),
        "{context} center should remain inside expected drop box: actual={actual:?}, expected={expected:?}"
    );
}

pub(crate) fn assert_scene_pane_matches_render_region(
    visual: &mut VisualTestContext,
    host: &Entity<DockHost>,
    scene: &DockPresentationScene,
    node: DockNodeId,
    region: DockDebugRegion,
    context: &str,
) {
    let expected = scene
        .pane_for_node(node)
        .unwrap_or_else(|| panic!("{context} pane should be in presentation scene"))
        .bounds;
    assert_render_region_matches_bounds(visual, host, region, expected, context);
}

pub(crate) fn assert_render_region_matches_bounds(
    visual: &mut VisualTestContext,
    host: &Entity<DockHost>,
    region: DockDebugRegion,
    expected: Bounds<Pixels>,
    context: &str,
) {
    let selector = selector_for(visual, host, region)
        .unwrap_or_else(|| panic!("{context} debug selector should be emitted"));
    assert_bounds_close(debug_bounds(visual, &selector), expected, context);
}

pub(crate) fn assert_point_close(actual: Point<Pixels>, expected: Point<Pixels>) {
    assert_close(f32::from(actual.x), f32::from(expected.x));
    assert_close(f32::from(actual.y), f32::from(expected.y));
}

pub(crate) fn center_drop_position(bounds: Bounds<Pixels>) -> open_gpui::Point<Pixels> {
    drop_box_position(bounds, DockDropBoxSet::Inner, DockDropBoxKind::Center)
}

pub(crate) fn screen_position_for_host_position(
    window_bounds: WindowBounds,
    host_position: open_gpui::Point<Pixels>,
) -> open_gpui::Point<Pixels> {
    point(
        window_bounds.get_bounds().origin.x + host_position.x,
        window_bounds.get_bounds().origin.y + host_position.y,
    )
}

pub(crate) fn inner_edge_drop_position(
    bounds: Bounds<Pixels>,
    zone: DropZone,
) -> open_gpui::Point<Pixels> {
    drop_box_position(
        bounds,
        DockDropBoxSet::Inner,
        DockDropBoxKind::InnerEdge(zone),
    )
}

pub(crate) fn outer_edge_drop_position(
    bounds: Bounds<Pixels>,
    zone: DropZone,
) -> open_gpui::Point<Pixels> {
    drop_box_position(
        bounds,
        DockDropBoxSet::Outer,
        DockDropBoxKind::OuterEdge(zone),
    )
}

fn drop_box_position(
    bounds: Bounds<Pixels>,
    set: DockDropBoxSet,
    kind: DockDropBoxKind,
) -> open_gpui::Point<Pixels> {
    geometry::drop_boxes(bounds, set)
        .into_iter()
        .find(|drop_box| drop_box.kind == kind)
        .map(|drop_box| drop_box.hit_bounds.center())
        .unwrap_or_else(|| panic!("{kind:?} drop box should exist"))
}

pub(crate) fn simulate_left_drag(
    visual: &mut VisualTestContext,
    start: open_gpui::Point<Pixels>,
    end: open_gpui::Point<Pixels>,
) {
    let threshold_point = if end.x >= start.x {
        point(start.x + px(24.0), start.y)
    } else {
        point(start.x - px(24.0), start.y)
    };
    activate_window_for_pointer_input(visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold_point, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
}
