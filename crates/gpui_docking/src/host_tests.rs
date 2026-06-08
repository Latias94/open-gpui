use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockGraph, DockHost,
    DockHostAccessError, DockNode, DockPolicyError, DockSpaceId, DockWorkspace,
    EditorDockLayoutSpec, SplitAxis, debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{
    AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size,
};
use std::{cell::Cell, rc::Rc};

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

    let graph = host.graph().expect("owned host should expose graph");
    let DockNode::Tabs { active, .. } = graph.node(root).expect("tabs should exist") else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(*active, 1);
    let panels = host.panels().expect("owned host should expose panels");
    assert!(panels.contains(&item("a")));
    assert!(panels.contains(&item("b")));
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
    assert_eq!(
        workspace
            .apply_action(&DockAction::RaiseFloating {
                space: space(),
                floating: first,
            })
            .expect("raising the top floating container should be a valid no-op"),
        DockActionOutcome::Unchanged
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
    assert_eq!(
        workspace
            .apply_action(&DockAction::SetFloatingBounds {
                space: space(),
                floating: first,
                bounds: moved_bounds,
            })
            .expect("setting identical floating bounds should be a valid no-op"),
        DockActionOutcome::Unchanged
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

    assert_eq!(
        host.workspace()
            .expect("owned host should expose workspace")
            .space(),
        &space()
    );
    assert!(
        host.graph()
            .expect("owned host should expose graph")
            .root(&space())
            .is_some()
    );
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

    assert!(host_a.read_with(&visual_a, |host, _| matches!(
        host.workspace(),
        Err(DockHostAccessError::ControllerBackedHost)
    )));

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
            .expect("owned host should expose graph")
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
        let graph = host.graph().expect("owned host should expose graph");
        let DockNode::Split { fractions, .. } = graph.node(split).expect("split should exist")
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
            .expect("owned host should expose graph")
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
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
        size(px(560.0), px(240.0)),
    );

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
    host.read_with(&visual, |host, _| {
        let graph = host.graph().expect("owned host should expose graph");
        let DockNode::Tabs { items, active } = graph.node(tabs).expect("tabs should still exist")
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
        let graph = host.graph().expect("owned host should expose graph");
        let root = graph.root(&space()).expect("space should keep root");
        let DockNode::Split { axis, children, .. } = graph.node(root).expect("root should exist")
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
