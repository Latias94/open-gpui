use crate::{
    DockController, DockGraph, DockNode, DockSpaceId, DockViewportRuntimeHandle, DockWorkspace,
    EditorDockLayoutSpec, debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{
    AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size,
};
use std::{cell::Cell, rc::Rc};

#[open_gpui::test]
fn controller_backed_hosts_share_one_workspace(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
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
    let selected_item = cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { selected, .. } = controller
            .graph()
            .node(root)
            .expect("root tabs should still exist")
        else {
            panic!("root should be tabs");
        };
        selected.clone()
    });
    assert_eq!(selected_item.as_ref(), Some(&item("b")));
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
                cx.new(|cx| TestPanel::new("explorer", cx)).into()
            })
            .panel_factory("editor", "Editor", move |cx| {
                editor_factory_calls.set(editor_factory_calls.get() + 1);
                cx.new(|cx| TestPanel::new("editor", cx)).into()
            })
            .panel_factory("preview", "Preview", move |cx| {
                preview_factory_calls.set(preview_factory_calls.get() + 1);
                cx.new(|cx| TestPanel::new("preview", cx)).into()
            })
            .panel_factory("terminal", "Terminal", |cx| {
                cx.new(|cx| TestPanel::new("terminal", cx)).into()
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
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
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

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("target host should render a drop preview for cross-window drag");
    assert!(debug_bounds(&mut target_visual, &preview).size.width > px(0.0));

    target_visual.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);
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
        !cx.windows().contains(&source_window.into()),
        "the vacated source viewport should close after the cross-window drop"
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&source_space),
        None,
        "the vacated source space should no longer own a runtime window"
    );

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should remain present")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}
