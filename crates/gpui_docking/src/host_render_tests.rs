use crate::{
    DockCentralRegion, DockController, DockFloatingContainer, DockGraph, DockHost, DockNode,
    DockNodeId, DockPanelDescriptor, DockViewportFocusCommand, DockViewportFocusRequest,
    DockViewportPlatformSyncAction, DockWorkspace, SplitAxis, debug::DockDebugRegion,
    host_test_support::*,
};
use open_gpui::{
    AppContext as _, Entity, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext,
    px, size,
};
use slotmap::Key;

#[open_gpui::test]
fn single_tabs_render_selected_panel_and_all_tab_labels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "b");
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
        .expect("selected panel selector should be emitted");

    assert!(debug_bounds(&mut visual, &tab_a).size.width > px(0.0));
    assert!(debug_bounds(&mut visual, &tab_b).size.width > px(0.0));
    assert!(debug_bounds(&mut visual, &panel_b).size.height > px(0.0));
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_none(),
        "inactive panel should not be mounted"
    );
}

#[open_gpui::test]
fn drop_guides_render_while_tab_drag_is_active(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    for zone in [
        crate::DropZone::Center,
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        let guide = selector_for(
            &visual,
            &host,
            DockDebugRegion::DropGuide {
                node: Some(root),
                zone,
            },
        )
        .unwrap_or_else(|| panic!("{zone:?} drop guide selector should be emitted"));
        assert!(
            debug_bounds(&mut visual, &guide).size.width > px(0.0),
            "{zone:?} guide should have visible bounds"
        );
    }
}

#[open_gpui::test]
fn tab_drag_start_selects_dragged_tab_and_requests_panel_focus(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    host.update(cx, |host, cx| {
        let selected = host.with_workspace(cx, |workspace| {
            workspace.graph().selected_item_in_tabs(root)
        });
        assert_eq!(selected, Some(item("b")));
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(focus_b),
            "tab drag start should use the same selection/focus path as tab activation"
        );
    });
}

#[open_gpui::test]
fn drop_guides_are_scoped_to_each_target_tabs_node(cx: &mut TestAppContext) {
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
    let right_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("right tabs selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let right_bounds = debug_bounds(&mut visual, &right_stack);
    let right_center_guide = selector_for(
        &visual,
        &host,
        DockDebugRegion::DropGuide {
            node: Some(right_tabs),
            zone: crate::DropZone::Center,
        },
    )
    .expect("right stack center guide selector should be emitted");
    assert!(
        right_bounds.contains(&debug_bounds(&mut visual, &right_center_guide).center()),
        "right stack guide should be positioned inside the right tab stack"
    );
}

#[open_gpui::test]
fn drop_guides_hide_edge_zones_when_edge_split_policy_rejects(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    start_tab_drag(&mut visual, &host, root, "a");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert_drop_guide_emitted(&visual, &host, Some(root), crate::DropZone::Center);
    for zone in [
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_not_emitted(&visual, &host, Some(root), zone);
    }
}

#[open_gpui::test]
fn central_region_drop_guides_hide_center_when_policy_rejects_dock_over(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let central_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, central_tabs],
        fractions: vec![0.35, 0.65],
    });
    graph.set_root(space(), root);
    graph.set_central_region(space(), DockCentralRegion::with_node(central_tabs));
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace
        .policy_mut()
        .set_allow_central_region_dock_over(false);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(500.0), px(240.0)));

    start_tab_drag(&mut visual, &host, source_tabs, "a");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert_drop_guide_not_emitted(&visual, &host, Some(central_tabs), crate::DropZone::Center);
    for zone in [
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_not_emitted(&visual, &host, Some(central_tabs), zone);
    }
    assert_drop_guide_emitted(&visual, &host, Some(source_tabs), crate::DropZone::Center);
}

#[open_gpui::test]
fn drop_guides_hide_zones_rejected_by_dock_class_policy(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(space(), "inspector");
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(500.0), px(240.0)));

    start_tab_drag(&mut visual, &host, left_tabs, "a");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    for zone in [
        crate::DropZone::Center,
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_not_emitted(&visual, &host, Some(right_tabs), zone);
    }
}

#[open_gpui::test]
fn render_session_uses_default_title_for_split_floating_children(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, right],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating { child: split });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: open_gpui::Bounds::new(
                open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(200.0)),
            ),
        });
    let workspace = DockWorkspace::new(space(), graph);
    let (_window, host, _visual) = open_workspace(cx, workspace, size(px(320.0), px(200.0)));

    let (title, chrome_target) = host.update(cx, |host, cx| {
        let session = host.render_session(cx);
        (
            session.floating_title(floating),
            session.floating_chrome_target(floating),
        )
    });

    assert_eq!(title, "Floating");
    assert_eq!(
        chrome_target,
        Some(crate::host_render_session::DockFloatingChromeTarget::AmbiguousSplit)
    );
}

fn start_tab_drag(
    visual: &mut VisualTestContext,
    host: &Entity<DockHost>,
    tabs: DockNodeId,
    item_id: &str,
) {
    let source_tab = selector_for(
        visual,
        host,
        DockDebugRegion::Tab {
            tabs,
            item: item(item_id),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(visual, &source_tab).center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
}

fn assert_drop_guide_emitted(
    visual: &VisualTestContext,
    host: &Entity<DockHost>,
    node: Option<DockNodeId>,
    zone: crate::DropZone,
) {
    assert!(
        selector_for(visual, host, DockDebugRegion::DropGuide { node, zone }).is_some(),
        "{zone:?} drop guide selector should be emitted"
    );
}

fn assert_drop_guide_not_emitted(
    visual: &VisualTestContext,
    host: &Entity<DockHost>,
    node: Option<DockNodeId>,
    zone: crate::DropZone,
) {
    assert!(
        selector_for(visual, host, DockDebugRegion::DropGuide { node, zone }).is_none(),
        "{zone:?} drop guide selector should not be emitted"
    );
}

#[open_gpui::test]
fn pending_panel_focus_targets_active_focusable_panel(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = test_view(cx, "A");
    let expected_focus = cx.read_entity(&panel, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "a"
            )))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(expected_focus));
    });
}

#[open_gpui::test]
fn viewport_activation_restores_recorded_last_focused_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let focus_a = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    visual.deactivate_window();
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(
            host.pending_focus_command().is_none(),
            "test setup should not have a pending focus request"
        );
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("b"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    assert_ne!(focus_a, focus_b);
}

#[open_gpui::test]
fn viewport_panel_request_selects_hidden_tab_before_restoring_focus(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let focus_a = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    let controller = host.update(cx, |host, _| host.controller().clone());
    controller.update(cx, |controller, cx| {
        let outcome = controller
            .select_tab(root, item("a"))
            .expect("selecting tab A should succeed");
        if outcome.changed() {
            cx.notify();
        }
    });
    cx.run_until_parked();
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { selected, .. } = controller
            .graph()
            .node(root)
            .expect("root tabs should exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(selected.as_ref(), Some(&item("a")));
    });

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "b"
            )))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { selected, .. } = controller
            .graph()
            .node(root)
            .expect("root tabs should exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(selected.as_ref(), Some(&item("b")));
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
    });

    assert_ne!(focus_a, focus_b);
}

#[open_gpui::test]
fn platform_activation_does_not_restore_panel_focus_while_mouse_is_pressed(
    cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let panel_b = test_view(cx, "B");
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    visual.deactivate_window();
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate");
    cx.run_until_parked();
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(stealer),
            "platform focus caused by mouse interaction must not restore panel focus"
        );
    });
}

#[open_gpui::test]
fn platform_activation_restores_recorded_panel_after_non_docking_focus_owner(
    cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let panel_b = test_view(cx, "B");
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });
    host.update(cx, |host, _| {
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "platform activation only tracks whether this viewport had dock-panel focus"
        );
    });

    visual.deactivate_window();
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate");
    cx.run_until_parked();
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
    });

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(focus_b),
            "backend-confirmed platform activation should restore recorded dock focus"
        );
    });
}

#[open_gpui::test]
fn platform_activation_does_not_reveal_hidden_recorded_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let focus_a = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    let controller = host.update(cx, |host, _| host.controller().clone());
    controller.update(cx, |controller, cx| {
        let outcome = controller
            .select_tab(root, item("a"))
            .expect("selecting tab A should succeed");
        if outcome.changed() {
            cx.notify();
        }
    });
    cx.run_until_parked();

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel("b"),),
        ));
        cx.notify();
    });
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(stealer.clone()),
            "platform activation restore must not guess focus from the current visible panel"
        );
    });
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { selected, .. } = controller
            .graph()
            .node(root)
            .expect("root tabs should exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(
            selected.as_ref(),
            Some(&item("a")),
            "platform activation restore must preserve the currently visible tab"
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
    });

    assert_ne!(focus_a, focus_b);
}

#[open_gpui::test]
fn close_recovery_does_not_reveal_hidden_recorded_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let focus_a = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    let controller = host.update(cx, |host, _| host.controller().clone());
    controller.update(cx, |controller, cx| {
        let outcome = controller
            .select_tab(root, item("a"))
            .expect("selecting tab A should succeed");
        if outcome.changed() {
            cx.notify();
        }
    });
    cx.run_until_parked();

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(
            host.request_viewport_focus_command(DockViewportFocusCommand::new(
                crate::DockViewportFocusCommandSource::CloseRecovery,
                DockViewportFocusRequest::panel("b"),
            ),)
        );
        cx.notify();
    });
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(stealer.clone()),
            "close recovery restore must not guess focus from the current visible panel"
        );
    });
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { selected, .. } = controller
            .graph()
            .node(root)
            .expect("root tabs should exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(
            selected.as_ref(),
            Some(&item("a")),
            "close recovery restore must preserve the currently visible tab"
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
    });

    assert_ne!(focus_a, focus_b);
}

#[open_gpui::test]
fn platform_activation_after_no_panel_focus_does_not_restore_old_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let panel_b = test_view(cx, "B");
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(
                DockViewportFocusRequest::no_panel_focus()
            )
        ));
        cx.notify();
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), None);
    });
    host.update(cx, |host, _| {
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(false),
            "explicit no-panel focus records that the viewport last had no dock-panel focus"
        );
    });

    visual.deactivate_window();
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate");
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            None,
            "platform activation without dock-panel focus history must not restore the old panel"
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(false),
            "explicit no-panel request keeps a no-panel activation fact for platform restore"
        );
    });
}

#[open_gpui::test]
fn viewport_activation_failure_clears_request_without_blurring_current_focus(
    cx: &mut TestAppContext,
) {
    let (graph, _root) = tabs_graph(&["a"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "a"
            )))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            None,
            "a failed explicit focus request must not synthesize panel-focus history"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(stealer),
            "failed focus restoration must leave the current focus fact untouched"
        );
    });
}

#[open_gpui::test]
fn viewport_failed_panel_focus_preserves_current_focus_and_history(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let focus_a = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "a"
            )))
        ));
        cx.notify();
    });
    cx.run_until_parked();
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_a.clone()));
    });

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

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "b"
            )))
        ));
        cx.notify();
    });
    visual.run_until_parked();
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "failed focus requests must not overwrite the last successful panel-focus fact"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(focus_a.clone()),
            "failed explicit panel focus must preserve the already focused dock panel"
        );
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("b"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "failed viewport activation restores must not record no-panel focus"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(focus_a),
            "failed viewport activation restores must leave the current dock-panel focus untouched"
        );
    });
}

#[open_gpui::test]
fn viewport_restore_request_without_focus_history_preserves_current_focus(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(
            host.pending_focus_command().is_none(),
            "test setup should not have a pending focus request"
        );
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("b"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            None,
            "restore attempts without focus history must not synthesize focus facts"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(stealer),
            "failed restore requests without history must leave the current focus untouched"
        );
    });
}

#[open_gpui::test]
fn platform_restore_failure_does_not_overwrite_had_panel_focus_fact(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph_with_selected(&["a"], "a");
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let (_window, host, _visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    host.update(cx, |host, cx| {
        host.viewport_runtime()
            .record_panel_focus(host.space().clone(), item("a"));
        assert!(host.request_viewport_focus_command(
            crate::DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel(
                "b"
            ))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "platform activation failures must not overwrite recorded panel focus"
        );
    });
}

#[open_gpui::test]
fn close_recovery_restore_failure_does_not_overwrite_had_panel_focus_fact(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph_with_selected(&["a"], "a");
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let (_window, host, _visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    host.update(cx, |host, cx| {
        host.viewport_runtime()
            .record_panel_focus(host.space().clone(), item("a"));
        assert!(host.request_viewport_focus_command(
            crate::DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(
                "b"
            ))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "close recovery failures must not overwrite the target viewport's focus history"
        );
    });
}

#[open_gpui::test]
fn viewport_no_panel_focus_request_blurs_without_restore(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = test_view(cx, "A");
    let panel_focus = cx.read_entity(&panel, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "a"
            )))
        ));
        cx.notify();
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(panel_focus.clone()));
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(
                DockViewportFocusRequest::no_panel_focus()
            )
        ));
        cx.notify();
    });
    visual.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            None,
            "explicit no-panel request must clear focus instead of restoring the last panel"
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
    });
}

#[open_gpui::test]
fn viewport_activation_without_history_does_not_pick_first_panel(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a", "b"]);
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let focus_a = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    let stealer = visual.update(|_, cx| cx.focus_handle());
    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });

    visual.deactivate_window();
    cx.run_until_parked();

    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate");
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(stealer));
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
    });
    assert_ne!(focus_a, focus_b);
}

#[open_gpui::test]
fn viewport_activation_for_gone_recorded_panel_preserves_current_focus(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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

    let controller = host.update(cx, |host, _| host.controller().clone());
    controller.update(cx, |controller, cx| {
        let outcome = controller
            .workspace_mut()
            .close_item(space(), item("b"))
            .expect("closing recorded panel should succeed");
        if outcome.changed() {
            cx.notify();
        }
    });
    let focused_before_restore = visual.update(|window, cx| window.focused(cx));

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("b"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "a failed restore for a removed panel must preserve the existing had-panel-focus fact"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            focused_before_restore,
            "restore failure for a removed panel must preserve whatever focus the close path already established"
        );
    });
}

#[open_gpui::test]
fn platform_activation_for_gone_recorded_panel_records_no_panel_focus(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

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

    let controller = host.update(cx, |host, _| host.controller().clone());
    controller.update(cx, |controller, cx| {
        let outcome = controller
            .workspace_mut()
            .close_item(space(), item("b"))
            .expect("closing recorded panel should succeed");
        if outcome.changed() {
            cx.notify();
        }
    });
    let focused_before_restore = visual.update(|window, cx| window.focused(cx));

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel("b"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(false),
            "platform activation restore for a removed panel must clear stale panel-focus history"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            focused_before_restore,
            "platform activation restore failure must preserve the current GPUI focus fact"
        );
    });
}

#[open_gpui::test]
fn missing_selected_panel_renders_placeholder(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph_with_selected(&["a", "missing"], "missing");
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
}

#[open_gpui::test]
fn empty_central_passthrough_renders_full_host_drop_target(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space(),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );
    let (_window, host, mut visual) = open_host(cx, graph, &[], size(px(320.0), px(200.0)));

    let empty = selector_for(&visual, &host, DockDebugRegion::EmptySpace)
        .expect("empty central passthrough selector should be emitted");
    let bounds = debug_bounds(&mut visual, &empty);

    assert_close(width(bounds), 320.0);
    assert_close(height(bounds), 200.0);
}

#[open_gpui::test]
fn empty_central_passthrough_syncs_window_pointer_input(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space(),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, _visual) =
        open_controller_workspace(cx, controller.clone(), size(px(320.0), px(200.0)));

    assert!(
        !window
            .update(cx, |_, window, _| window.accepts_pointer_input())
            .expect("host window should remain live"),
        "empty central passthrough should make the host window click-through"
    );
    let runtime = host.update(cx, |host, _| host.viewport_runtime().clone());
    assert_eq!(
        runtime
            .runtime_status()
            .last_platform_sync
            .as_ref()
            .map(|sync| sync.applied.as_slice()),
        Some([DockViewportPlatformSyncAction::PointerInput { enabled: false }].as_slice())
    );

    controller.update(cx, |controller, cx| {
        let mut graph = controller.graph().clone();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), tabs);
        graph.set_central_region(
            space(),
            DockCentralRegion::with_node(tabs).with_passthrough_when_empty(true),
        );
        controller.workspace_mut().set_graph(graph);
        cx.notify();
    });
    cx.run_until_parked();

    assert!(
        window
            .update(cx, |_, window, _| window.accepts_pointer_input())
            .expect("host window should remain live"),
        "repopulating the central region should restore the render-owned pointer input sync"
    );
    assert_eq!(
        runtime
            .runtime_status()
            .last_platform_sync
            .as_ref()
            .map(|sync| sync.applied.as_slice()),
        Some([DockViewportPlatformSyncAction::PointerInput { enabled: true }].as_slice())
    );
}

#[open_gpui::test]
fn empty_central_passthrough_with_floating_content_keeps_window_pointer_input(
    cx: &mut TestAppContext,
) {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space(),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );
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
            bounds: floating_bounds(20.0, 20.0, 220.0, 140.0),
        });
    let (window, host, visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(320.0), px(220.0)),
    );

    assert!(
        window
            .update(cx, |_, window, _| window.accepts_pointer_input())
            .expect("host window should remain live"),
        "window-level click-through would also pierce floating content"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Floating { node: floating }).is_some(),
        "floating overlay should still render above the empty central region"
    );
    let runtime = host.update(cx, |host, _| host.viewport_runtime().clone());
    assert_eq!(
        runtime.runtime_status().last_platform_sync,
        None,
        "empty central with floating content must not request whole-window pointer passthrough"
    );
}

#[open_gpui::test]
fn ordinary_render_does_not_restore_externally_owned_pointer_passthrough(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a"]);
    let (window, _host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(320.0), px(200.0)),
    );
    assert_ne!(root, DockNodeId::null(), "test graph should have a root");

    assert!(
        !window
            .update(cx, |_, window, _| {
                window.set_accepts_pointer_input(false);
                window.accepts_pointer_input()
            })
            .expect("host window should remain live"),
        "test setup should make the source viewport click-through outside render passthrough"
    );

    window
        .update(cx, |_, window, _| window.refresh())
        .expect("host window should remain live");
    cx.run_until_parked();

    assert!(
        !window
            .update(cx, |_, window, _| window.accepts_pointer_input())
            .expect("host window should remain live"),
        "ordinary render must not restore no-input owned by another runtime transaction"
    );
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
        selected: Some(item("b")),
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
fn central_split_child_uses_remaining_render_space(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let main = graph.insert_node(DockNode::Tabs {
        items: vec![item("main")],
        selected: Some(item("main")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, main, right],
        fractions: vec![0.2, 0.0, 0.3],
    });
    graph.set_root(space(), split);
    graph.set_central_region(space(), DockCentralRegion::with_node(main));

    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("left", "Left", "Left"),
            ("main", "Main", "Main"),
            ("right", "Right", "Right"),
        ],
        size(px(1000.0), px(200.0)),
    );

    let left_selector = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let main_selector = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("main split selector should be emitted");
    let right_selector = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 2 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left_selector)), 200.0);
    assert_close(width(debug_bounds(&mut visual, &main_selector)), 500.0);
    assert_close(width(debug_bounds(&mut visual, &right_selector)), 300.0);
}
