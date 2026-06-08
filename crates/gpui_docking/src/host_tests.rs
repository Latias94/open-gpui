use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockFloatingContainer,
    DockGraph, DockHost, DockItemId, DockLayoutNode, DockLayoutRect, DockNode, DockNodeId,
    DockOpApplyError, DockPolicyError, DockSpaceId, DockViewportAdapter, DockViewportOpenStatus,
    DockViewportPlacement, DockViewportPlacementLayout, DockViewportWindowBounds,
    DockViewportWindowState, DockWorkspace, DropZone, EditorDockLayoutSpec, SplitAxis,
    debug::DockDebugRegion,
};
use open_gpui::{
    AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, Modifiers,
    MouseButton, ParentElement, Pixels, Render, Styled, TestAppContext, VisualTestContext, Window,
    WindowBounds, WindowHandle, WindowOptions, div, point, px, rgb, size,
};
use slotmap::Key;
use std::{cell::Cell, rc::Rc};

const SPACE: &str = "main";

struct TestPanel {
    label: &'static str,
}

impl Render for TestPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let selector = format!("test-panel:{}", self.label);
        div()
            .debug_selector(move || selector)
            .size_full()
            .bg(rgb(0xffffff))
            .child(self.label)
    }
}

fn space() -> DockSpaceId {
    DockSpaceId::from(SPACE)
}

fn item(id: &str) -> DockItemId {
    DockItemId::from(id)
}

fn tabs_graph(items: &[&str], active: usize) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: items.iter().copied().map(DockItemId::from).collect(),
        active,
    });
    graph.set_root(space(), root);
    (graph, root)
}

fn split_graph(
    axis: SplitAxis,
    first_fraction: f32,
    second_fraction: f32,
) -> (DockGraph, DockNodeId, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let first = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let second = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let root = graph.insert_node(DockNode::Split {
        axis,
        children: vec![first, second],
        fractions: vec![first_fraction, second_fraction],
    });
    graph.set_root(space(), root);
    (graph, root, first, second)
}

fn floating_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn viewport_window_options(width: f32, height: f32) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, width, height,
        ))),
        ..Default::default()
    }
}

fn floating_overlay_graph() -> (DockGraph, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(space(), root);
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
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

fn test_view(cx: &mut TestAppContext, label: &'static str) -> Entity<TestPanel> {
    cx.new(|_| TestPanel { label })
}

fn workspace_with_panels(
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

fn open_host(
    cx: &mut TestAppContext,
    graph: DockGraph,
    panels: &[(&str, &str, &'static str)],
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let workspace = workspace_with_panels(cx, graph, panels);
    open_workspace(cx, workspace, window_size)
}

fn open_workspace(
    cx: &mut TestAppContext,
    workspace: DockWorkspace,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let window = cx.open_window(window_size, move |_, _| DockHost::from_workspace(workspace));
    let host = window.root(cx).expect("window should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    (window, host, visual)
}

fn open_controller_workspace(
    cx: &mut TestAppContext,
    controller: Entity<DockController>,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    open_controller_space(cx, controller, space(), window_size)
}

fn open_controller_space(
    cx: &mut TestAppContext,
    controller: Entity<DockController>,
    dock_space: DockSpaceId,
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let window = cx.open_window(window_size, move |_, cx| {
        DockHost::from_controller(controller.clone(), dock_space, cx)
    });
    let host = window.root(cx).expect("window should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    (window, host, visual)
}

fn selector_for(
    cx: &VisualTestContext,
    host: &Entity<DockHost>,
    region: DockDebugRegion,
) -> Option<String> {
    cx.read_entity(host, |host, _| {
        host.debug_selector(&region).map(ToString::to_string)
    })
}

fn debug_bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("debug selector {selector} should have bounds"))
}

fn width(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.width)
}

fn height(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.height)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 2.0,
        "expected {actual} to be within 2px of {expected}"
    );
}

fn simulate_left_drag(
    visual: &mut VisualTestContext,
    start: open_gpui::Point<Pixels>,
    end: open_gpui::Point<Pixels>,
) {
    let threshold_point = if end.x >= start.x {
        point(start.x + px(24.0), start.y)
    } else {
        point(start.x - px(24.0), start.y)
    };
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold_point, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn registry_replaces_registered_panel(cx: &mut TestAppContext) {
    let mut registry = crate::DockPanelRegistry::new();
    let first = test_view(cx, "first");
    let second = test_view(cx, "second");

    assert!(registry.register_view(item("a"), "First", first).is_none());
    let previous = registry
        .register_view(item("a"), "Second", second)
        .expect("second registration should return previous panel");

    assert_eq!(previous.title(), "First");
    assert_eq!(
        registry.get(&item("a")).map(crate::DockPanel::title),
        Some("Second")
    );
    assert_eq!(registry.len(), 1);
}

#[open_gpui::test]
fn lazy_panel_factory_instantiates_on_first_render_and_reuses_view(cx: &mut TestAppContext) {
    let calls = Rc::new(Cell::new(0));
    let (graph, _root) = tabs_graph(&["lazy"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    let factory_calls = calls.clone();
    workspace.register_panel_factory("lazy", "Lazy", move |cx| {
        factory_calls.set(factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "lazy" }).into()
    });
    let panel = workspace
        .panels()
        .get(&item("lazy"))
        .expect("panel should be registered")
        .clone();

    assert!(!panel.has_view());
    assert_eq!(calls.get(), 0);

    let (window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));
    assert_eq!(calls.get(), 1);
    assert!(panel.has_view());
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel { item: item("lazy") }
        )
        .is_some(),
        "lazy panel should render after first instantiation"
    );

    let _visual = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    assert_eq!(calls.get(), 1, "lazy panel view should be reused");
}

#[open_gpui::test]
fn inactive_lazy_panel_title_does_not_instantiate_view(cx: &mut TestAppContext) {
    let active_calls = Rc::new(Cell::new(0));
    let inactive_calls = Rc::new(Cell::new(0));
    let (graph, _root) = tabs_graph(&["active", "inactive"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    let active_factory_calls = active_calls.clone();
    workspace.register_panel_factory("active", "Active", move |cx| {
        active_factory_calls.set(active_factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "active" }).into()
    });
    let inactive_factory_calls = inactive_calls.clone();
    workspace.register_panel_factory("inactive", "Inactive", move |cx| {
        inactive_factory_calls.set(inactive_factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "inactive" }).into()
    });

    let (_window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    assert_eq!(active_calls.get(), 1);
    assert_eq!(
        inactive_calls.get(),
        0,
        "inactive tab title lookup should not instantiate panel view"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("active")
            }
        )
        .is_some()
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("inactive")
            }
        )
        .is_none()
    );
}

#[open_gpui::test]
fn lazy_panel_state_stays_out_of_layout_export(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["lazy"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_factory("lazy", "Lazy Panel", |cx| {
        cx.new(|_| TestPanel { label: "lazy" }).into()
    });

    let (_window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));
    let json = host.read_with(&visual, |host, _| {
        serde_json::to_string(&host.graph().export_layout()).expect("layout should serialize")
    });

    assert!(!json.contains("Lazy Panel"));
    assert!(!json.contains("TestPanel"));
    assert!(!json.contains("AnyView"));
}

#[open_gpui::test]
fn host_applies_actions_through_workspace(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    let mut host = DockHost::from_workspace(workspace);

    let outcome = host
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab mutation should be valid");

    let DockNode::Tabs { active, .. } = host.graph().node(root).expect("tabs should exist") else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(*active, 1);
    assert!(host.panels().contains(&item("a")));
    assert!(host.panels().contains(&item("b")));
}

#[open_gpui::test]
fn floating_action_respects_workspace_policy(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);

    let result = workspace.apply_action(&DockAction::FloatItemInWindow {
        source_space: space(),
        item: item("a"),
        target_space: space(),
        bounds: floating_bounds(20.0, 30.0, 200.0, 120.0),
    });

    assert_eq!(
        result,
        Err(DockActionApplyError::Policy(
            DockPolicyError::FloatingDisabled
        ))
    );
    assert!(workspace.graph().floating_containers(&space()).is_empty());
    let DockNode::Tabs { items, .. } = workspace
        .graph()
        .node(root)
        .expect("root tabs should still exist")
    else {
        panic!("root should remain tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
}

#[open_gpui::test]
fn floating_actions_create_move_raise_and_merge_containers(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b", "c"], 0);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);

    let first_bounds = floating_bounds(20.0, 30.0, 200.0, 120.0);
    let second_bounds = floating_bounds(60.0, 80.0, 180.0, 100.0);
    assert_eq!(
        workspace
            .apply_action(&DockAction::FloatItemInWindow {
                source_space: space(),
                item: item("a"),
                target_space: space(),
                bounds: first_bounds,
            })
            .expect("floating should be enabled"),
        DockActionOutcome::Changed
    );
    workspace
        .apply_action(&DockAction::FloatItemInWindow {
            source_space: space(),
            item: item("b"),
            target_space: space(),
            bounds: second_bounds,
        })
        .expect("second floating should be valid");

    let first = workspace.graph().floating_containers(&space())[0].node;
    let second = workspace.graph().floating_containers(&space())[1].node;
    assert_eq!(
        workspace
            .graph()
            .floating_containers(&space())
            .iter()
            .map(|container| container.node)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    workspace
        .apply_action(&DockAction::RaiseFloating {
            space: space(),
            floating: first,
        })
        .expect("raising a floating container should be valid");
    assert_eq!(
        workspace
            .graph()
            .floating_containers(&space())
            .iter()
            .map(|container| container.node)
            .collect::<Vec<_>>(),
        vec![second, first]
    );

    let moved_bounds = floating_bounds(90.0, 100.0, 200.0, 120.0);
    workspace
        .apply_action(&DockAction::SetFloatingBounds {
            space: space(),
            floating: first,
            bounds: moved_bounds,
        })
        .expect("floating bounds update should be valid");
    assert_eq!(
        workspace
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == first)
            .expect("first floating should remain present")
            .bounds,
        moved_bounds
    );

    workspace
        .apply_action(&DockAction::MergeFloatingInto {
            space: space(),
            floating: first,
            target_tabs: root,
        })
        .expect("floating merge should be valid");
    assert_eq!(workspace.graph().floating_containers(&space()).len(), 1);
    let DockNode::Tabs { items, .. } = workspace
        .graph()
        .node(root)
        .expect("root tabs should remain present")
    else {
        panic!("root should remain tabs");
    };
    assert_eq!(items, &vec![item("c"), item("a")]);
}

#[open_gpui::test]
fn compatibility_constructor_delegates_to_workspace(_cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"], 0);
    let host = DockHost::new(space(), graph);

    assert_eq!(host.workspace().space(), &space());
    assert!(host.graph().root(&space()).is_some());
}

#[open_gpui::test]
fn controller_backed_hosts_share_one_workspace(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));

    let (window_a, host_a, mut visual_a) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));
    let (window_b, host_b, visual_b) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));

    assert!(
        selector_for(
            &visual_a,
            &host_a,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should be active in host A before mutation"
    );
    assert!(
        selector_for(
            &visual_b,
            &host_b,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should be active in host B before mutation"
    );

    let tab_b = selector_for(
        &visual_a,
        &host_a,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("tab B selector should be emitted");
    let tab_b_bounds = debug_bounds(&mut visual_a, &tab_b);
    visual_a.simulate_click(tab_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();

    let visual_a = VisualTestContext::from_window(window_a.into(), cx);
    let visual_b = VisualTestContext::from_window(window_b.into(), cx);

    assert!(
        selector_for(
            &visual_a,
            &host_a,
            DockDebugRegion::Panel { item: item("b") }
        )
        .is_some(),
        "panel B should be active in host A after mutation"
    );
    assert!(
        selector_for(
            &visual_b,
            &host_b,
            DockDebugRegion::Panel { item: item("b") }
        )
        .is_some(),
        "panel B should be active in host B after shared-owner mutation"
    );
    let active = cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { active, .. } = controller
            .graph()
            .node(root)
            .expect("root tabs should still exist")
        else {
            panic!("root should be tabs");
        };
        *active
    });
    assert_eq!(active, 1);
}

#[open_gpui::test]
fn viewport_adapter_opens_and_reuses_controller_backed_window(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), primary_tabs);
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut adapter = DockViewportAdapter::new();

    let opened = cx
        .update(|app| {
            adapter.open_viewport(
                controller.clone(),
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open");
    assert_eq!(opened.space, secondary_space);
    assert_eq!(opened.status, DockViewportOpenStatus::Opened);
    assert_eq!(
        adapter.window_for_space(&secondary_space),
        Some(opened.window)
    );

    let opened_window = opened
        .window
        .downcast::<DockHost>()
        .expect("viewport window should render DockHost");
    let host = opened_window
        .root(cx)
        .expect("opened viewport should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(opened.window, cx);
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "secondary viewport should render the shared controller's secondary panel"
    );

    let reused = cx
        .update(|app| {
            adapter.open_viewport(
                controller.clone(),
                secondary_space.clone(),
                viewport_window_options(480.0, 260.0),
                app,
            )
        })
        .expect("live secondary viewport should be reused");
    assert_eq!(reused.status, DockViewportOpenStatus::Reused);
    assert_eq!(reused.window, opened.window);
    assert_eq!(adapter.len(), 1);
}

#[open_gpui::test]
fn viewport_adapter_opens_with_saved_placement_options(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut adapter = DockViewportAdapter::new();
    let saved_window_bounds = WindowBounds::Windowed(floating_bounds(80.0, 90.0, 420.0, 260.0));
    let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
        space: secondary_space.clone(),
        display_id: None,
        window_bounds: Some(DockViewportWindowBounds {
            state: DockViewportWindowState::Windowed,
            bounds: DockLayoutRect::from_bounds(saved_window_bounds.get_bounds()),
        }),
        host_bounds: None,
    }]);
    let fallback_options = viewport_window_options(240.0, 160.0);

    let opened = cx
        .update(|app| {
            let options = placement
                .window_options_for_space(&secondary_space, fallback_options)
                .expect("saved placement should produce window options");
            adapter.open_viewport(controller.clone(), secondary_space.clone(), options, app)
        })
        .expect("secondary viewport should open with saved placement");

    assert_eq!(opened.status, DockViewportOpenStatus::Opened);
    assert_eq!(
        opened
            .window
            .update(cx, |_, window, _| window.window_bounds())
            .expect("opened window should still be live"),
        saved_window_bounds
    );
}

#[open_gpui::test]
fn controller_builder_mounts_host_with_lazy_panel_factories(cx: &mut TestAppContext) {
    let editor_calls = Rc::new(Cell::new(0));
    let preview_calls = Rc::new(Cell::new(0));
    let editor_factory_calls = editor_calls.clone();
    let preview_factory_calls = preview_calls.clone();

    let controller = cx.new(|_| {
        DockController::builder(space())
            .default_editor_layout(EditorDockLayoutSpec::new(
                ["explorer"],
                ["editor", "preview"],
                ["terminal"],
            ))
            .panel_factory("explorer", "Explorer", |cx| {
                cx.new(|_| TestPanel { label: "explorer" }).into()
            })
            .panel_factory("editor", "Editor", move |cx| {
                editor_factory_calls.set(editor_factory_calls.get() + 1);
                cx.new(|_| TestPanel { label: "editor" }).into()
            })
            .panel_factory("preview", "Preview", move |cx| {
                preview_factory_calls.set(preview_factory_calls.get() + 1);
                cx.new(|_| TestPanel { label: "preview" }).into()
            })
            .panel_factory("terminal", "Terminal", |cx| {
                cx.new(|_| TestPanel { label: "terminal" }).into()
            })
            .build()
    });

    let preview_tabs = cx.read_entity(&controller, |controller, _| {
        controller
            .graph()
            .find_item_in_space(&space(), &item("preview"))
            .expect("preview item should be in the builder layout")
            .0
    });
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(520.0), px(320.0)));

    assert!(
        cx.read_entity(&host, |host, _| host.controller().is_some()),
        "host should be controller-backed when mounted from the builder path"
    );
    assert_eq!(editor_calls.get(), 1);
    assert_eq!(
        preview_calls.get(),
        0,
        "inactive lazy panel should not instantiate during initial render"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("editor")
            }
        )
        .is_some(),
        "active builder-registered editor panel should render"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("preview")
            }
        )
        .is_none(),
        "inactive preview panel should not render before selection"
    );

    let preview_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: preview_tabs,
            item: item("preview"),
        },
    )
    .expect("preview tab selector should be emitted");
    let preview_tab_bounds = debug_bounds(&mut visual, &preview_tab);
    visual.simulate_click(preview_tab_bounds.center(), Modifiers::none());
    cx.run_until_parked();

    let visual = VisualTestContext::from_window(window.into(), cx);
    assert_eq!(preview_calls.get(), 1);
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("preview")
            }
        )
        .is_some(),
        "selecting the preview tab should instantiate and render its lazy panel"
    );
}

#[open_gpui::test]
fn cross_window_tab_drag_can_drop_into_target_controller_host(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));

    let (source_window, source_host, mut source_visual) = open_controller_space(
        cx,
        controller.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space(
        cx,
        controller.clone(),
        target_space.clone(),
        size(px(360.0), px(220.0)),
    );

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target = debug_bounds(&mut target_visual, &target_tabs_selector).center();

    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    target_visual.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let preview = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::DropPreview { tabs: target_tabs },
    )
    .expect("target host should render a drop preview for cross-window drag");
    assert!(debug_bounds(&mut target_visual, &preview).size.width > px(0.0));

    target_visual.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let source_visual = VisualTestContext::from_window(source_window.into(), cx);
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);

    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should render in the target window after cross-window drop"
    );
    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_none(),
        "panel A should leave the source window after cross-window drop"
    );

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should remain present")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(*active, 1);
    });
}

#[open_gpui::test]
fn workspace_applies_actions_and_preserves_registered_panels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "B", test_view(cx, "B"));

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab mutation should be valid");

    let DockNode::Tabs { active, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(*active, 1);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_selecting_active_tab_is_noop(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 1);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab selection should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_rejects_invalid_select_tab_actions(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let missing_item = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("missing"),
        })
        .expect_err("missing tab item should fail");
    assert_eq!(
        missing_item,
        DockActionApplyError::ItemNotInTabs {
            tabs: root,
            item: item("missing")
        }
    );

    let wrong_node = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: DockNodeId::null(),
            item: item("a"),
        })
        .expect_err("missing tabs node should fail");
    assert_eq!(
        wrong_node,
        DockActionApplyError::Graph(DockOpApplyError::TabsNodeNotFound {
            tabs: DockNodeId::null()
        })
    );

    let DockNode::Tabs { active, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(*active, 0);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_action_layout_export_remains_graph_only(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "Panel A", "Panel A"), ("b", "Panel B", "Panel B")],
    );

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab mutation should be valid");
    assert_eq!(outcome, DockActionOutcome::Changed);

    let layout = workspace.graph().export_layout();
    layout.validate().expect("exported layout should validate");
    let json = serde_json::to_string(&layout).expect("layout should serialize");

    assert!(!json.contains("Panel A"));
    assert!(!json.contains("Panel B"));
    assert!(!json.contains("AnyView"));
    assert!(!json.contains("Entity"));
    assert!(!json.contains("WindowHandle"));

    let DockLayoutNode::Tabs { active, items, .. } = layout
        .nodes
        .iter()
        .find(|node| matches!(node, DockLayoutNode::Tabs { .. }))
        .expect("layout should contain tabs node")
    else {
        panic!("expected tabs node");
    };
    assert_eq!(*active, 1);
    assert_eq!(items, &vec![item("a"), item("b")]);

    let imported = DockGraph::import_layout(&layout).expect("layout should import");
    let imported_root = imported.root(&space()).expect("space should keep root");
    let DockNode::Tabs { active, items } = imported
        .node(imported_root)
        .expect("imported root should exist")
    else {
        panic!("imported root should be tabs");
    };
    assert_eq!(*active, 1);
    assert_eq!(items, &vec![item("a"), item("b")]);
}

#[open_gpui::test]
fn workspace_move_tab_center_moves_item_between_stacks(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: left_tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: right_tabs,
            zone: DropZone::Center,
        })
        .expect("move tab action should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(right_tabs)
        .expect("target tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("b"), item("a")]);
    assert_eq!(*active, 1);
}

#[open_gpui::test]
fn workspace_same_stack_center_drop_is_noop(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: tabs,
            zone: DropZone::Center,
        })
        .expect("same-stack center drop should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(tabs)
        .expect("tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_resize_split_action_updates_fractions(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![0.7, 0.3],
        })
        .expect("resize split action should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Split { fractions, .. } = workspace
        .graph()
        .node(split)
        .expect("split should still exist")
    else {
        panic!("root should be split");
    };
    assert_close(fractions[0], 0.7);
    assert_close(fractions[1], 0.3);
}

#[open_gpui::test]
fn workspace_resize_split_action_reports_unchanged_for_same_fractions(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![0.5, 0.5],
        })
        .expect("resize split action should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
}

#[open_gpui::test]
fn workspace_resize_split_action_rejects_invalid_targets(cx: &mut TestAppContext) {
    let (graph, split, left_tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let missing = workspace
        .apply_action(&DockAction::ResizeSplit {
            split: DockNodeId::null(),
            fractions: vec![0.5, 0.5],
        })
        .expect_err("missing split should fail");
    assert_eq!(
        missing,
        DockActionApplyError::Graph(DockOpApplyError::SplitNodeNotFound {
            split: DockNodeId::null()
        })
    );

    let wrong_kind = workspace
        .apply_action(&DockAction::ResizeSplit {
            split: left_tabs,
            fractions: vec![0.5, 0.5],
        })
        .expect_err("tabs node is not a split");
    assert_eq!(
        wrong_kind,
        DockActionApplyError::Graph(DockOpApplyError::NodeIsNotSplit { node: left_tabs })
    );

    let mismatch = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![1.0],
        })
        .expect_err("fraction length mismatch should fail");
    assert_eq!(
        mismatch,
        DockActionApplyError::Graph(DockOpApplyError::SplitFractionsLenMismatch {
            split,
            children_len: 2,
            fractions_len: 1
        })
    );
}

#[open_gpui::test]
fn workspace_policy_blocks_edge_drop_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);

    let err = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: left_tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: right_tabs,
            zone: DropZone::Right,
        })
        .expect_err("edge drop should be rejected by policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::EdgeSplitDisabled)
    );
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(left_tabs)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_policy_blocks_splitter_resize_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_splitter_resize(false);

    let err = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![0.7, 0.3],
        })
        .expect_err("splitter resize should be rejected by policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::SplitterResizeDisabled)
    );
    let DockNode::Split { fractions, .. } =
        workspace.graph().node(split).expect("split should remain")
    else {
        panic!("root should be split");
    };
    assert_close(fractions[0], 0.5);
    assert_close(fractions[1], 0.5);
}

#[open_gpui::test]
fn single_tabs_render_active_panel_and_all_tab_labels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 1);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let tab_a = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("a"),
        },
    )
    .expect("tab A selector should be emitted");
    let tab_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("tab B selector should be emitted");
    let panel_b = selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") })
        .expect("active panel selector should be emitted");

    assert!(debug_bounds(&mut visual, &tab_a).size.width > px(0.0));
    assert!(debug_bounds(&mut visual, &tab_b).size.width > px(0.0));
    assert!(debug_bounds(&mut visual, &panel_b).size.height > px(0.0));
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_none(),
        "inactive panel should not be mounted"
    );
}

#[open_gpui::test]
fn missing_active_panel_renders_placeholder(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a", "missing"], 1);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(400.0), px(240.0)),
    );

    let missing = selector_for(
        &visual,
        &host,
        DockDebugRegion::MissingPanel {
            item: item("missing"),
        },
    )
    .expect("missing panel selector should be emitted");

    assert!(debug_bounds(&mut visual, &missing).size.width > px(0.0));
    assert_eq!(
        host.read_with(&visual, |host, _| host.graph().spaces().len()),
        1
    );
}

#[open_gpui::test]
fn empty_root_renders_placeholder(cx: &mut TestAppContext) {
    let graph = DockGraph::new();
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(320.0), px(200.0)),
    );

    let empty = selector_for(&visual, &host, DockDebugRegion::EmptySpace)
        .expect("empty selector should be emitted");

    assert!(debug_bounds(&mut visual, &empty).size.width > px(0.0));
    assert_eq!(host.read_with(&visual, |host, _| host.panels().len()), 1);
}

#[open_gpui::test]
fn floating_container_renders_panel_inside_overlay_bounds(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(220.0)),
    );

    let frame = selector_for(&visual, &host, DockDebugRegion::Floating { node: floating })
        .expect("floating frame selector should be emitted");
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");

    let frame_bounds = debug_bounds(&mut visual, &frame);
    assert_close(width(frame_bounds), 220.0);
    assert_close(height(frame_bounds), 140.0);
    assert!(debug_bounds(&mut visual, &handle).size.height > px(0.0));
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "floating panel should render"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "root panel should still render behind the overlay"
    );
}

#[open_gpui::test]
fn missing_floating_child_renders_missing_node_placeholder(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(space(), root);
    let missing_child = DockNodeId::null();
    let floating = graph.insert_node(DockNode::Floating {
        child: missing_child,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 220.0, 140.0),
        });

    let (_window, host, visual) = open_host(
        cx,
        graph,
        &[("b", "Panel B", "B")],
        size(px(320.0), px(220.0)),
    );

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::MissingNode {
                node: missing_child
            }
        )
        .is_some(),
        "missing floating child should render a test-visible placeholder"
    );
}

#[open_gpui::test]
fn dragging_floating_handle_updates_graph_bounds(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(320.0), px(220.0)));

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x + px(40.0), start.y + px(30.0));

    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    host.read_with(&visual, |host, _| {
        let container = host
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == floating)
            .expect("floating container should remain present");
        assert_close(f32::from(container.bounds.origin.x), 50.0);
        assert_close(f32::from(container.bounds.origin.y), 50.0);
        assert!(host.floating_drag().is_none());
    });
}

#[open_gpui::test]
fn horizontal_split_uses_normalized_flex_shares(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(200.0)),
    );

    let left = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let right = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left)), 100.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 300.0);
}

#[open_gpui::test]
fn vertical_split_uses_normalized_flex_shares(cx: &mut TestAppContext) {
    let (graph, split, _top, _bottom) = split_graph(SplitAxis::Vertical, 0.25, 0.75);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(200.0)),
    );

    let top = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("top split selector should be emitted");
    let bottom = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("bottom split selector should be emitted");

    assert_close(height(debug_bounds(&mut visual, &top)), 50.0);
    assert_close(height(debug_bounds(&mut visual, &bottom)), 150.0);
}

#[open_gpui::test]
fn unnormalized_split_fractions_are_repaired_for_rendering(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 2.0, 1.0);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(600.0), px(200.0)),
    );

    let left = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let right = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left)), 400.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 200.0);
}

#[open_gpui::test]
fn horizontal_splitter_drag_updates_width_fractions(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
    let left = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let right = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left)), 200.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 200.0);

    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x + px(80.0), start.y);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_close(width(debug_bounds(&mut visual, &left)), 280.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 120.0);
    host.read_with(&visual, |host, _| {
        let DockNode::Split { fractions, .. } =
            host.graph().node(split).expect("split should exist")
        else {
            panic!("root should be split");
        };
        assert_close(fractions[0], 0.7);
        assert_close(fractions[1], 0.3);
        assert!(host.splitter_drag().is_none());
    });
}

#[open_gpui::test]
fn vertical_splitter_drag_updates_height_fractions(cx: &mut TestAppContext) {
    let (graph, split, _top, _bottom) = split_graph(SplitAxis::Vertical, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(400.0)),
    );

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
    let top = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("top split selector should be emitted");
    let bottom = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("bottom split selector should be emitted");

    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x, start.y + px(80.0));
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_close(height(debug_bounds(&mut visual, &top)), 280.0);
    assert_close(height(debug_bounds(&mut visual, &bottom)), 120.0);
}

#[open_gpui::test]
fn splitter_drag_clamps_to_minimum_pane_size(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
    let left = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let right = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("right split selector should be emitted");

    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x - px(300.0), start.y);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_close(width(debug_bounds(&mut visual, &left)), 96.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 304.0);
}

#[open_gpui::test]
fn dragging_tab_to_other_stack_center_moves_panel(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = debug_bounds(&mut visual, &target_tabs).center();

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be visible after center drop"
    );
    host.read_with(&visual, |host, _| {
        let DockNode::Tabs { items, active } = host
            .graph()
            .node(right_tabs)
            .expect("target tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(*active, 1);
    });
}

#[open_gpui::test]
fn dragging_tab_to_right_edge_creates_horizontal_split(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = point(
        target_bounds.origin.x + target_bounds.size.width - px(2.0),
        start.y,
    );

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be visible after edge drop"
    );
    host.read_with(&visual, |host, _| {
        let root = host.graph().root(&space()).expect("space should keep root");
        let DockNode::Split { axis, children, .. } =
            host.graph().node(root).expect("root should exist")
        else {
            panic!("root should be split after edge drop");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(children.len(), 2);
    });
}

#[open_gpui::test]
fn dragging_tab_to_edge_renders_drop_preview(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = point(
        target_bounds.origin.x + target_bounds.size.width - px(2.0),
        start.y,
    );

    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let preview = selector_for(
        &visual,
        &host,
        DockDebugRegion::DropPreview { tabs: right_tabs },
    )
    .expect("drop preview selector should be emitted");
    let preview_bounds = debug_bounds(&mut visual, &preview);
    assert!(preview_bounds.size.width > px(0.0));
    assert!(preview_bounds.size.height > px(0.0));
    assert!(
        preview_bounds.size.width < target_bounds.size.width,
        "edge preview should occupy only an edge band"
    );
}

#[open_gpui::test]
fn policy_rejected_edge_hover_does_not_render_drop_preview(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = point(
        target_bounds.origin.x + target_bounds.size.width - px(2.0),
        start.y,
    );

    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::DropPreview { tabs: right_tabs }
        )
        .is_none(),
        "policy-rejected edge hover should not render preview"
    );
}

#[open_gpui::test]
fn clicking_inactive_tab_updates_active_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active before mutation"
    );

    let tab_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("tab B selector should be emitted");
    let tab_b_bounds = debug_bounds(&mut visual, &tab_b);
    visual.simulate_click(tab_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "panel B should be active after mutation"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_none(),
        "panel A should no longer be mounted after mutation"
    );
}
