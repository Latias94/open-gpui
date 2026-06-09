use crate::{
    DockController, DockNode, DockNodeId, SplitAxis, debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{
    AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size,
};
use slotmap::Key;

#[open_gpui::test]
fn dragging_floating_handle_updates_graph_bounds(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(320.0), px(220.0)));

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

    cx.read_entity(&controller, |controller, _| {
        let container = controller
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == floating)
            .expect("floating container should remain present");
        assert_close(f32::from(container.bounds.origin.x), 50.0);
        assert_close(f32::from(container.bounds.origin.y), 50.0);
    });
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_none());
    });
}

#[open_gpui::test]
fn stale_floating_drag_begin_does_not_leave_transient_drag(cx: &mut TestAppContext) {
    let (graph, _root, _floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let (_window, host, _visual) = open_workspace(cx, workspace, size(px(320.0), px(220.0)));

    let began = cx.update_entity(&host, |host, cx| {
        host.begin_floating_drag_from_render(
            space(),
            DockNodeId::null(),
            point(px(10.0), px(20.0)),
            floating_bounds(10.0, 20.0, 220.0, 140.0),
            cx,
        )
    });

    assert!(!began);
    assert!(cx.read_entity(&host, |host, _| host.floating_drag().is_none()));
}

#[open_gpui::test]
fn horizontal_splitter_drag_updates_width_fractions(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));

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
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Split { fractions, .. } =
            controller.graph().node(split).expect("split should exist")
        else {
            panic!("root should be split");
        };
        assert_close(fractions[0], 0.7);
        assert_close(fractions[1], 0.3);
    });
    host.read_with(&visual, |host, _| {
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
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

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
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, active } = controller
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
fn dragging_tab_within_same_stack_reorders_tabs(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b", "c"], 0);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(560.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("c"),
        },
    )
    .expect("target tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let target_bounds = debug_bounds(&mut visual, &target_tab);
    let end = point(
        target_bounds.origin.x + target_bounds.size.width - px(2.0),
        target_bounds.center().y,
    );

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active after reorder"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(tabs)
            .expect("tabs should still exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
        assert_eq!(*active, 2);
    });
}

#[open_gpui::test]
fn dragging_tab_to_right_edge_creates_horizontal_split(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

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
    cx.read_entity(&controller, |controller, _| {
        let root = controller
            .graph()
            .root(&space())
            .expect("space should keep root");
        let DockNode::Split { axis, children, .. } =
            controller.graph().node(root).expect("root should exist")
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
