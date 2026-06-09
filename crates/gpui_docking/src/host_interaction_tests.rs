use crate::{
    DockController, DockGraph, DockNode, DockNodeId, DockPanel, DockWorkspace, SplitAxis,
    debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{
    AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size,
};
use slotmap::Key;

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
fn dragging_tab_bar_empty_area_moves_whole_stack(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        active: 1,
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
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

    let source_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: source_tabs })
        .expect("source tabs selector should be emitted");
    let target_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: target_tabs })
        .expect("target tabs selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_stack);
    let start = point(
        source_bounds.origin.x + source_bounds.size.width - px(8.0),
        source_bounds.origin.y + px(12.0),
    );
    let end = debug_bounds(&mut visual, &target_stack).center();

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("c") }).is_some(),
        "previously active stack item should remain active after stack drop"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(*active, 2);
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

    let preview = selector_for(&visual, &host, DockDebugRegion::DropPreview)
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
fn dragging_tab_to_empty_host_space_moves_item(cx: &mut TestAppContext) {
    let source_space = space();
    let empty_space = crate::DockSpaceId::from("empty");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));

    let (_source_window, source_host, mut source_visual) = open_controller_space(
        cx,
        controller.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space(
        cx,
        controller.clone(),
        empty_space.clone(),
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
    let target_empty = selector_for(&target_visual, &target_host, DockDebugRegion::EmptySpace)
        .expect("empty target selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = debug_bounds(&mut target_visual, &target_empty).center();

    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("empty target should render a host-level drop preview");
    assert!(debug_bounds(&mut target_visual, &preview).size.width > px(0.0));

    target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);

    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should render in the previously empty host after drop"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let target_root = controller
            .graph()
            .root(&empty_space)
            .expect("empty space should receive a root");
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(target_root)
            .expect("target root should exist")
        else {
            panic!("target root should be tabs");
        };
        assert_eq!(items, &vec![item("a")]);
        assert_eq!(*active, 0);
    });
}

#[open_gpui::test]
fn dragging_tab_to_floating_title_bar_merges_into_floating_stack(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(360.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("source tab selector should be emitted");
    let floating_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = debug_bounds(&mut visual, &floating_handle).center();

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "panel B should be active in the floating stack after title-bar drop"
    );
    cx.read_entity(&controller, |controller, _| {
        let floating_tabs = controller
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == floating)
            .and_then(|container| match controller.graph().node(container.node) {
                Some(DockNode::Floating { child }) => Some(*child),
                _ => None,
            })
            .expect("floating child should remain");
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(floating_tabs)
            .expect("floating tabs should exist")
        else {
            panic!("floating child should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b")]);
        assert_eq!(*active, 1);
        assert_eq!(controller.graph().root(&space()), None);
    });
}

#[open_gpui::test]
fn dragging_floating_title_bar_to_tabs_merges_floating_stack(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(360.0), px(240.0)));

    let floating_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: root })
        .expect("root tabs selector should be emitted");
    let start = debug_bounds(&mut visual, &floating_handle).center();
    let end = debug_bounds(&mut visual, &target_tabs).center();

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active in the root stack after floating title-bar drop"
    );
    cx.read_entity(&controller, |controller, _| {
        assert!(
            controller.graph().floating_containers(&space()).is_empty(),
            "floating container should be removed after its stack merges into root"
        );
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(root)
            .expect("root tabs should still exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(*active, 1);
    });
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
        selector_for(&visual, &host, DockDebugRegion::DropPreview).is_none(),
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

#[open_gpui::test]
fn clicking_tab_close_removes_closable_panel_from_graph(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let close_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::TabClose {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("closable tab should render a close control");
    let close_b_bounds = debug_bounds(&mut visual, &close_b);
    visual.simulate_click(close_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Tab {
                tabs: root,
                item: item("b"),
            },
        )
        .is_none(),
        "closed tab should be removed from rendered graph state"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "closing an inactive tab should keep the previous active panel mounted"
    );
    let (items, active, metadata_still_registered) = cx.update_entity(&host, |host, cx| {
        host.with_workspace(cx, |workspace| {
            let DockNode::Tabs { items, active } = workspace
                .graph()
                .node(root)
                .expect("root tabs should remain")
            else {
                panic!("root should stay as tabs");
            };
            (
                items.clone(),
                *active,
                workspace.panels().contains(&item("b")),
            )
        })
    });
    assert_eq!(items, vec![item("a")]);
    assert_eq!(active, 0);
    assert!(
        metadata_still_registered,
        "close should remove graph membership without discarding panel metadata"
    );
}

#[open_gpui::test]
fn non_closable_tab_omits_close_control_and_rejects_close_action(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["locked", "open"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel(
        item("locked"),
        DockPanel::new("Locked", test_view(cx, "A")).closable(false),
    );
    workspace.register_panel_view(item("open"), "Open", test_view(cx, "B"));
    let (_window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::TabClose {
                tabs: root,
                item: item("locked"),
            },
        )
        .is_none(),
        "non-closable tab should not expose a rendered close affordance"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::TabClose {
                tabs: root,
                item: item("open"),
            },
        )
        .is_some(),
        "closable sibling should still expose a close affordance"
    );

    let changed = cx.update_entity(&host, |host, cx| {
        host.close_item_from_render(item("locked"), cx)
    });
    assert!(!changed);

    let items = cx.update_entity(&host, |host, cx| {
        host.with_workspace(cx, |workspace| {
            let DockNode::Tabs { items, .. } = workspace
                .graph()
                .node(root)
                .expect("root tabs should remain")
            else {
                panic!("root should stay as tabs");
            };
            items.clone()
        })
    });
    assert_eq!(items, vec![item("locked"), item("open")]);
}
