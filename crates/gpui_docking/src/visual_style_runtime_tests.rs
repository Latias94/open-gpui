use crate::{
    DockController, DockGraph, DockHost, DockNode, DockNodeId, DockSpaceId, DockSurface,
    DockViewportRuntimeHandle, DockVisualPalette, DockVisualStyle, DockVisualStyleResolver,
    DockWorkspace,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    host_test_support::{
        activate_window_for_pointer_input, debug_bounds, floating_overlay_graph, item,
        open_controller_space_with_runtime, selector_for, space, tabs_graph, test_view,
        workspace_with_panels,
    },
    host_viewport_runtime_test_support::configure_native_registered_window_hit,
};
use open_gpui::{
    AnyView, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, StyleRefinement,
    Styled, SubtreePresentation, SubtreePresentationExt as _, TestAppContext, VisualTestContext,
    Window, WindowId, div, point, px, rgb, size,
};
use open_gpui_ui_components::{
    ThemeResolver,
    theme::{DARK_THEME_ID, LIGHT_THEME_ID, ThemeContext, ThemeMode, ThemeScope, set_app_theme},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

struct ScopedDockHosts {
    first: Entity<DockHost>,
    second: Entity<DockHost>,
    first_theme: ThemeContext,
    second_theme: ThemeContext,
    first_presentation: SubtreePresentation,
}

impl Render for ScopedDockHosts {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let first =
            AnyView::from(self.first.clone()).cached(StyleRefinement::default().size_full());
        let second =
            AnyView::from(self.second.clone()).cached(StyleRefinement::default().size_full());

        div()
            .flex()
            .size_full()
            .child(ThemeScope::new(
                "dock-style-first",
                self.first_theme.clone(),
                div()
                    .w(px(300.0))
                    .h_full()
                    .child(first)
                    .with_subtree_presentation(self.first_presentation),
            ))
            .child(ThemeScope::new(
                "dock-style-second",
                self.second_theme.clone(),
                div().w(px(300.0)).h_full().child(second),
            ))
    }
}

fn style_for_mode(mode: ThemeMode) -> DockVisualStyle {
    let mut palette = DockVisualPalette::built_in();
    let (surface, muted, accent) = match mode {
        ThemeMode::Light => (0xfafafa, 0xf1f5f9, 0x2563eb),
        ThemeMode::Dark => (0x171717, 0x262626, 0x60a5fa),
        ThemeMode::HighContrast => (0x000000, 0x101010, 0xffff00),
    };
    palette.surface = rgb(surface);
    palette.surface_muted = rgb(muted);
    palette.accent = rgb(accent);
    DockVisualStyle::from_palette(palette)
}

fn theme_resolver() -> DockVisualStyleResolver {
    DockVisualStyleResolver::new(|window, cx| {
        style_for_mode(ThemeResolver::current_snapshot(window, cx).mode())
    })
}

fn resolved_style(host: &Entity<DockHost>, visual: &VisualTestContext) -> DockVisualStyle {
    visual.read_entity(host, |host, _| {
        host.last_resolved_visual_style()
            .cloned()
            .expect("rendered DockHost should retain its test-only resolved style")
    })
}

fn draw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx).clear());
}

fn start_tab_drag(
    visual: &mut VisualTestContext,
    host: &Entity<DockHost>,
    tabs: DockNodeId,
    item_id: &str,
) -> DockDragPayload {
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
    activate_window_for_pointer_input(visual);
    visual.simulate_mouse_down(
        start,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    visual.update(|_, cx| {
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("tab drag should create a docking payload")
    })
}

fn start_floating_drag(
    visual: &mut VisualTestContext,
    host: &Entity<DockHost>,
    floating: DockNodeId,
) -> DockDragPayload {
    let handle = selector_for(
        visual,
        host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating drag handle should be emitted");
    let start = debug_bounds(visual, &handle).center();
    activate_window_for_pointer_input(visual);
    visual.simulate_mouse_down(
        start,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(start.x + px(26.0), start.y),
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    visual.update(|_, cx| {
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("floating drag should create a docking payload")
    })
}

#[open_gpui::test]
fn immutable_surface_resolvers_stay_isolated_across_windows(cx: &mut TestAppContext) {
    let light = style_for_mode(ThemeMode::Light);
    let dark = style_for_mode(ThemeMode::Dark);
    let first = cx.update(|app| {
        DockSurface::builder("first")
            .visual_style_resolver(DockVisualStyleResolver::fixed(light.clone()))
            .build(app)
            .expect("first surface should build")
    });
    let second = cx.update(|app| {
        DockSurface::builder("second")
            .visual_style_resolver(DockVisualStyleResolver::fixed(dark.clone()))
            .build(app)
            .expect("second surface should build")
    });
    let before_first = cx.update(|app| first.export_snapshot(app).layout().clone());
    let before_second = cx.update(|app| second.export_snapshot(app).layout().clone());

    let first_for_window = first.clone();
    let first_window = cx.open_window(size(px(320.0), px(220.0)), move |_, cx| {
        first_for_window.primary_host(cx)
    });
    let second_for_window = second.clone();
    let second_window = cx.open_window(size(px(320.0), px(220.0)), move |_, cx| {
        second_for_window.primary_host(cx)
    });
    let first_host = first_window.root(cx).expect("first DockHost should mount");
    let second_host = second_window
        .root(cx)
        .expect("second DockHost should mount");
    cx.run_until_parked();
    let mut first_visual = VisualTestContext::from_window(first_window.into(), cx);
    let mut second_visual = VisualTestContext::from_window(second_window.into(), cx);
    draw(&mut first_visual);
    draw(&mut second_visual);

    assert_eq!(resolved_style(&first_host, &first_visual), light);
    assert_eq!(resolved_style(&second_host, &second_visual), dark);
    assert_eq!(
        first_visual.update(|_, app| first.export_snapshot(app).layout().clone()),
        before_first
    );
    assert_eq!(
        second_visual.update(|_, app| second.export_snapshot(app).layout().clone()),
        before_second
    );
}

#[open_gpui::test]
fn app_theme_change_refreshes_a_read_only_dock_snapshot_consumer(cx: &mut TestAppContext) {
    cx.update(|app| {
        set_app_theme(app, LIGHT_THEME_ID).expect("built-in app theme should resolve");
    });
    let (graph, _) = tabs_graph(&["a"]);
    let workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let before_layout = cx.read_entity(&controller, |controller, _| {
        controller.graph().export_layout()
    });
    let runtime =
        DockViewportRuntimeHandle::with_visual_style_resolver(controller.clone(), theme_resolver());
    let window_controller = controller.clone();
    let window = cx.open_window(size(px(320.0), px(220.0)), move |_, cx| {
        DockHost::from_controller(window_controller, space(), runtime, cx)
    });
    let host = window.root(cx).expect("DockHost should mount");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    draw(&mut visual);
    assert_eq!(
        resolved_style(&host, &visual),
        style_for_mode(ThemeMode::Light)
    );

    cx.update(|app| {
        set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve");
    });
    cx.run_until_parked();

    assert_eq!(
        resolved_style(&host, &visual),
        style_for_mode(ThemeMode::Dark),
        "app-theme mutation must invalidate a window whose only consumer uses the read-only adapter"
    );
    assert_eq!(
        visual.read_entity(&controller, |controller, _| controller
            .graph()
            .export_layout()),
        before_layout
    );
}

#[open_gpui::test]
fn cached_hosts_resolve_nearest_subtree_theme_without_cross_contamination(cx: &mut TestAppContext) {
    let (graph, _) = tabs_graph(&["a"]);
    let workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let before_layout = cx.read_entity(&controller, |controller, _| {
        controller.graph().export_layout()
    });
    let runtime =
        DockViewportRuntimeHandle::with_visual_style_resolver(controller.clone(), theme_resolver());
    let fixture_controller = controller.clone();
    let window = cx.open_window(size(px(600.0), px(260.0)), move |_, cx| {
        let first = cx.new(|cx| {
            DockHost::from_controller(fixture_controller.clone(), space(), runtime.clone(), cx)
        });
        let second = cx.new(|cx| {
            DockHost::from_controller(fixture_controller.clone(), space(), runtime.clone(), cx)
        });
        ScopedDockHosts {
            first,
            second,
            first_theme: ThemeContext::dark(),
            second_theme: ThemeContext::light(),
            first_presentation: SubtreePresentation::Visible,
        }
    });
    let fixture = window.root(cx).expect("scoped Dock fixture should mount");
    let (first, second) = cx.read_entity(&fixture, |fixture, _| {
        (fixture.first.clone(), fixture.second.clone())
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    draw(&mut visual);

    assert_eq!(
        resolved_style(&first, &visual),
        style_for_mode(ThemeMode::Dark)
    );
    let second_style = resolved_style(&second, &visual);
    assert_eq!(second_style, style_for_mode(ThemeMode::Light));

    fixture.update(&mut visual, |fixture, cx| {
        fixture.first_theme = ThemeContext::high_contrast();
        cx.notify();
    });
    draw(&mut visual);

    assert_eq!(
        resolved_style(&first, &visual),
        style_for_mode(ThemeMode::HighContrast)
    );
    assert_eq!(resolved_style(&second, &visual), second_style);
    assert_eq!(
        visual.read_entity(&controller, |controller, _| controller
            .graph()
            .export_layout()),
        before_layout
    );
}

#[open_gpui::test]
fn inert_and_hidden_hosts_resume_with_current_style_without_stale_replay(cx: &mut TestAppContext) {
    let (graph, _) = tabs_graph(&["a"]);
    let workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime =
        DockViewportRuntimeHandle::with_visual_style_resolver(controller.clone(), theme_resolver());
    let fixture_controller = controller.clone();
    let window = cx.open_window(size(px(600.0), px(260.0)), move |_, cx| {
        let first = cx.new(|cx| {
            DockHost::from_controller(fixture_controller.clone(), space(), runtime.clone(), cx)
        });
        let second = cx.new(|cx| {
            DockHost::from_controller(fixture_controller.clone(), space(), runtime.clone(), cx)
        });
        ScopedDockHosts {
            first,
            second,
            first_theme: ThemeContext::light(),
            second_theme: ThemeContext::light(),
            first_presentation: SubtreePresentation::Visible,
        }
    });
    let fixture = window
        .root(cx)
        .expect("presentation Dock fixture should mount");
    let first = cx.read_entity(&fixture, |fixture, _| fixture.first.clone());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    draw(&mut visual);
    assert_eq!(
        resolved_style(&first, &visual),
        style_for_mode(ThemeMode::Light)
    );

    fixture.update(&mut visual, |fixture, cx| {
        fixture.first_theme = ThemeContext::dark();
        fixture.first_presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    draw(&mut visual);
    assert_eq!(
        resolved_style(&first, &visual),
        style_for_mode(ThemeMode::Dark)
    );

    fixture.update(&mut visual, |fixture, cx| {
        fixture.first_theme = ThemeContext::high_contrast();
        fixture.first_presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    draw(&mut visual);
    assert_eq!(
        resolved_style(&first, &visual),
        style_for_mode(ThemeMode::HighContrast),
        "Hidden may rebuild retained elements, but its style resolution must already be current"
    );

    fixture.update(&mut visual, |fixture, cx| {
        fixture.first_presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    draw(&mut visual);
    assert_eq!(
        resolved_style(&first, &visual),
        style_for_mode(ThemeMode::HighContrast),
        "Visible must resolve the current scope instead of replaying hidden paint"
    );
}

#[open_gpui::test]
fn cross_window_drag_freezes_source_style_and_uses_live_target_style(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));

    let styles = Rc::new(RefCell::new(HashMap::<WindowId, DockVisualStyle>::new()));
    let resolver_styles = styles.clone();
    let resolver = DockVisualStyleResolver::new(move |window, _| {
        resolver_styles
            .borrow()
            .get(&window.window_handle().window_id())
            .cloned()
            .unwrap_or_else(DockVisualStyle::built_in)
    });
    let runtime =
        DockViewportRuntimeHandle::with_visual_style_resolver(controller.clone(), resolver);
    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space,
        size(px(320.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller,
        runtime.clone(),
        target_space,
        size(px(420.0), px(260.0)),
    );

    let source_opening_style = style_for_mode(ThemeMode::Dark);
    let source_live_style = style_for_mode(ThemeMode::Light);
    let target_opening_style = style_for_mode(ThemeMode::Light);
    let target_live_style = style_for_mode(ThemeMode::HighContrast);
    styles.borrow_mut().extend([
        (source_window.window_id(), source_opening_style.clone()),
        (target_window.window_id(), target_opening_style),
    ]);
    draw(&mut source_visual);
    draw(&mut target_visual);

    let target_empty = selector_for(&target_visual, &target_host, DockDebugRegion::EmptySpace)
        .expect("target empty-space selector should be emitted");
    let target_position = debug_bounds(&mut target_visual, &target_empty).center();
    let target_from_source = point(px(400.0) + target_position.x, target_position.y);
    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_from_source,
    );
    let payload = start_tab_drag(&mut source_visual, &source_host, source_tabs, "a");
    let payload_identity = payload.identity();
    let first_session = runtime
        .active_payload_drag_session(&payload)
        .expect("source drag should create a runtime session");
    assert_eq!(
        runtime.active_payload_drag_visual_style(Some(&first_session)),
        Some(source_opening_style.drag.clone())
    );

    styles
        .borrow_mut()
        .insert(source_window.window_id(), source_live_style.clone());
    styles
        .borrow_mut()
        .insert(target_window.window_id(), target_live_style.clone());
    draw(&mut source_visual);
    draw(&mut target_visual);
    assert_eq!(
        resolved_style(&source_host, &source_visual),
        source_live_style,
        "source host should continue resolving its live window style"
    );
    assert_eq!(
        runtime.active_payload_drag_visual_style(Some(&first_session)),
        Some(source_opening_style.drag),
        "the deferred source visual must retain its opening-generation style"
    );

    source_visual.simulate_mouse_move(
        target_from_source,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert_eq!(
        resolved_style(&target_host, &target_visual),
        target_live_style,
        "target guides must consume the target host's current style"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropGuide {
                node: None,
                zone: crate::DropZone::Center,
            },
        )
        .is_some(),
        "the target host should render a center guide for the routed drag"
    );

    assert!(source_visual.update(|window, app| {
        source_host.update(app, |host, cx| {
            host.cancel_payload_drag_from_render(&payload, window, cx)
        })
    }));
    cx.run_until_parked();
    assert!(
        runtime
            .active_payload_drag_visual_style(Some(&first_session))
            .is_none(),
        "cancel must retire the first session's visual metadata"
    );
    assert!(source_visual.update(|_, app| app.active_drag_value::<DockDragPayload>().is_none()));

    styles.borrow_mut().insert(
        source_window.window_id(),
        style_for_mode(ThemeMode::HighContrast),
    );
    draw(&mut source_visual);
    let reopened_payload = start_tab_drag(&mut source_visual, &source_host, source_tabs, "a");
    let reopened_session = runtime
        .active_payload_drag_session(&reopened_payload)
        .expect("reopened source drag should create a new runtime session");
    assert_eq!(reopened_payload.identity(), payload_identity);
    assert_ne!(reopened_session.id(), first_session.id());
    assert_eq!(
        runtime.active_payload_drag_visual_style(Some(&reopened_session)),
        Some(style_for_mode(ThemeMode::HighContrast).drag),
        "reopen must capture the new source style without changing payload identity"
    );
    assert!(
        runtime
            .active_payload_drag_visual_style(Some(&first_session))
            .is_none(),
        "the retired generation must remain inaccessible after reopen"
    );

    source_visual.simulate_mouse_move(
        target_from_source,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropGuide {
                node: None,
                zone: crate::DropZone::Center,
            },
        )
        .is_some(),
        "the same route must remain valid after reopening with a different source visual style"
    );
    assert!(source_visual.update(|window, app| {
        source_host.update(app, |host, cx| {
            host.cancel_payload_drag_from_render(&reopened_payload, window, cx)
        })
    }));
}

#[open_gpui::test]
fn floating_drag_freezes_its_opening_drag_style_until_reopen(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));

    let opening_style = style_for_mode(ThemeMode::Dark);
    let current_style = Rc::new(RefCell::new(opening_style.clone()));
    let resolver_style = current_style.clone();
    let runtime = DockViewportRuntimeHandle::with_visual_style_resolver(
        controller.clone(),
        DockVisualStyleResolver::new(move |_, _| resolver_style.borrow().clone()),
    );
    let (_window, host, mut visual) = open_controller_space_with_runtime(
        cx,
        controller,
        runtime.clone(),
        space(),
        size(px(360.0), px(240.0)),
    );
    draw(&mut visual);

    let payload = start_floating_drag(&mut visual, &host, floating);
    let first_session = runtime
        .active_payload_drag_session(&payload)
        .expect("floating drag should create a runtime session");
    assert_eq!(
        runtime.active_payload_drag_visual_style(Some(&first_session)),
        Some(opening_style.drag.clone())
    );

    let live_style = style_for_mode(ThemeMode::HighContrast);
    *current_style.borrow_mut() = live_style.clone();
    draw(&mut visual);
    assert_eq!(resolved_style(&host, &visual), live_style);
    assert_eq!(
        runtime.active_payload_drag_visual_style(Some(&first_session)),
        Some(opening_style.drag),
        "the floating deferred visual must retain its opening-generation drag style"
    );

    assert!(visual.update(|window, app| {
        host.update(app, |host, cx| {
            host.cancel_payload_drag_from_render(&payload, window, cx)
        })
    }));
    cx.run_until_parked();
    assert!(
        runtime
            .active_payload_drag_visual_style(Some(&first_session))
            .is_none()
    );

    let reopened_style = style_for_mode(ThemeMode::Light);
    *current_style.borrow_mut() = reopened_style.clone();
    draw(&mut visual);
    let reopened_payload = start_floating_drag(&mut visual, &host, floating);
    let reopened_session = runtime
        .active_payload_drag_session(&reopened_payload)
        .expect("reopened floating drag should create a runtime session");
    assert_eq!(reopened_payload.identity(), payload.identity());
    assert_ne!(reopened_session.id(), first_session.id());
    assert_eq!(
        runtime.active_payload_drag_visual_style(Some(&reopened_session)),
        Some(reopened_style.drag)
    );

    assert!(visual.update(|window, app| {
        host.update(app, |host, cx| {
            host.cancel_payload_drag_from_render(&reopened_payload, window, cx)
        })
    }));
}
