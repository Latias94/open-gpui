use crate::{
    DockCentralRegion, DockController, DockFloatingContainer, DockGraph, DockHost, DockNode,
    DockNodeId, DockPanelDescriptor, DockSpaceId, DockViewportActivationTransaction,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportFocusCommand, DockViewportFocusRequest, DockViewportHostGeometry,
    DockViewportPlatformSignals, DockViewportPlatformSyncDispatch,
    DockViewportPlatformSyncObservationOutcome, DockViewportRuntimeHandle,
    DockViewportTargetContext, DockWorkspace, SplitAxis,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_scene_fact,
    host_test_support::*,
    presentation_scene::{DockPresentationPane, DockPresentationPaneKind, DockPresentationScene},
    transition_executor::DockTransitionExecutionState,
    transition_geometry::{DockMotionPreference, DockTransitionPlan},
    visual_affordance_scene::{
        DockVisualAffordanceKind, DockVisualAffordanceLayer, DockVisualAffordanceScene,
        DockVisualAffordanceState, DockVisualLayerScope,
    },
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, Bounds, Context, Corners, Entity, FocusHandle,
    Focusable, HitboxBehavior, InteractiveElement, IntoElement, Modifiers, MouseButton,
    ParentElement, PlatformWindowDispatch, PlatformWindowMutationTerminal, Render,
    RequestFrameOptions, StatefulInteractiveElement, Styled, SubtreeClip, SubtreeClipExt,
    SubtreePresentation, SubtreePresentationExt, SubtreeTransform, SubtreeTransformExt,
    SubtreeTransformOrigin, TestAppContext, VisualTestContext, Window, WindowMutationDomain,
    canvas, div, fill, point, px, red, size,
};
use open_gpui_motion::{
    MotionDuration, MotionEasing, MotionIntent, MotionPreference, MotionTransition,
};
use slotmap::Key;
use std::{cell::RefCell, rc::Rc, time::Duration};

struct TransformedDockHostFixture {
    host: Entity<DockHost>,
    show_host: bool,
    presentation: SubtreePresentation,
    alternate_transform: bool,
    fail_late: bool,
    cache_probe_revision: u64,
}

struct PresentedDockHostWithExternalFocus {
    host: Entity<DockHost>,
    presentation: SubtreePresentation,
    external_focus: FocusHandle,
}

struct NestedFocusPanel {
    root_focus: FocusHandle,
    child_focus: FocusHandle,
}

impl NestedFocusPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
        }
    }
}

impl Focusable for NestedFocusPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.root_focus.clone()
    }
}

impl Render for NestedFocusPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("nested-focus-panel-root")
            .track_focus(&self.root_focus)
            .size_full()
            .child(
                div()
                    .id("nested-focus-panel-child")
                    .track_focus(&self.child_focus)
                    .size_full(),
            )
    }
}

impl TransformedDockHostFixture {
    fn transform(&self) -> SubtreeTransform {
        let (scale, translation) = if self.alternate_transform {
            (size(0.85, 1.35), point(px(72.0), px(12.0)))
        } else {
            (size(1.25, 0.8), point(px(24.0), px(30.0)))
        };
        SubtreeTransform::try_new(scale, translation, SubtreeTransformOrigin::TOP_LEFT)
            .expect("DockHost fixture transforms should remain representable")
    }
}

impl Render for TransformedDockHostFixture {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut transformed = div().w(px(420.0)).h(px(260.0));
        if self.show_host {
            transformed = transformed.child(
                AnyView::from(self.host.clone())
                    .cached(open_gpui::StyleRefinement::default().size_full()),
            );
        }
        if self.fail_late {
            transformed = transformed.child(
                canvas(
                    |_, _, _| {},
                    |_, _, window, _| {
                        window.paint_quad(fill(
                            Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                            red(),
                        ));
                    },
                )
                .absolute()
                .size_full(),
            );
        }
        div().size_full().child(
            transformed
                .with_subtree_transform(self.transform())
                .with_subtree_presentation(self.presentation),
        )
    }
}

impl Render for PresentedDockHostWithExternalFocus {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .size_full()
                    .child(AnyView::from(self.host.clone()))
                    .with_subtree_presentation(self.presentation),
            )
            .child(
                div()
                    .id("dock-external-focus")
                    .w(px(20.0))
                    .h(px(20.0))
                    .focusable()
                    .track_focus(&self.external_focus),
            )
    }
}

fn linear_continuity_transition(duration: Duration) -> MotionTransition {
    MotionTransition::duration(
        MotionIntent::Continuity,
        MotionPreference::Animated,
        MotionDuration::Custom(duration),
        MotionEasing::Linear,
    )
}

fn rounded_host_geometry(cx: &mut TestAppContext) -> DockViewportHostGeometry {
    let committed = Rc::new(RefCell::new(None));
    let visual = cx.add_empty_window();
    visual.draw(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)), {
        let committed = committed.clone();
        move |_, _| {
            let radius = size(px(50.0), px(50.0));
            canvas(
                move |bounds, window, _| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    *committed.borrow_mut() = Some(DockViewportHostGeometry::from_hitbox(&hitbox));
                },
                |_, _, _, _| {},
            )
            .size_full()
            .with_subtree_clip(
                SubtreeClip::try_own_rounded_border_box(Corners {
                    top_left: radius,
                    top_right: radius,
                    bottom_right: radius,
                    bottom_left: radius,
                })
                .expect("circular host clip should be valid"),
            )
        }
    });
    committed
        .borrow_mut()
        .take()
        .expect("host prepaint should commit an exact hit-test snapshot")
}

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
fn transformed_host_keeps_layout_facts_and_advances_display_geometry_generation(
    cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime.clone(), cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(700.0), px(420.0)), move |_, _| {
        TransformedDockHostFixture {
            host: window_host,
            show_host: true,
            presentation: SubtreePresentation::Visible,
            alternate_transform: false,
            fail_late: false,
            cache_probe_revision: 0,
        }
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());

    let window_id = window.window_id();
    let tabs_selector = selector_for(&visual, &host, DockDebugRegion::Tabs { node: root })
        .expect("transformed tabs should publish a debug selector");
    let host_selector = selector_for(&visual, &host, DockDebugRegion::Host)
        .expect("transformed host should publish a debug selector");
    let first_tabs_displayed = debug_bounds(&mut visual, &tabs_selector);
    let first_host_displayed = debug_bounds(&mut visual, &host_selector);
    let (first_tabs_layout, first_runtime_displayed, first_generation, host_local_center) = host
        .update(cx, |host, _| {
            let runtime = host.viewport_runtime();
            let tabs_layout = runtime
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .expect("transformed host should retain layout-space leaf facts");
            let tabs_displayed = runtime
                .rendered_leaf_displayed_bounds_for_tabs(host.space(), Some(window_id), root)
                .expect("transformed host should project leaf facts into window space");
            let runtime = runtime.borrow();
            let generation = runtime
                .adapter()
                .snapshot_facts_generation(host.space(), window_id)
                .expect("transformed host should publish route facts");
            let local_center = runtime
                .adapter()
                .window_to_host(host.space(), first_host_displayed.center())
                .expect("displayed host center should inverse-project into host-local space");
            (tabs_layout, tabs_displayed, generation, local_center)
        });

    assert_bounds_close(
        first_runtime_displayed,
        first_tabs_displayed,
        "first transformed leaf window bounds",
    );
    assert_ne!(
        first_tabs_layout, first_runtime_displayed,
        "non-uniform host transform must not overwrite layout-space facts"
    );
    assert_point_close(host_local_center, point(px(210.0), px(130.0)));

    window
        .update(cx, |fixture, _window, cx| {
            fixture.alternate_transform = true;
            cx.notify();
        })
        .expect("transformed DockHost fixture window should remain live");
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());

    let second_tabs_displayed = debug_bounds(&mut visual, &tabs_selector);
    let (second_tabs_layout, second_runtime_displayed, second_generation) =
        host.update(cx, |host, _| {
            let runtime = host.viewport_runtime();
            let tabs_layout = runtime
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .expect("transform-only frame should retain layout-space leaf facts");
            let tabs_displayed = runtime
                .rendered_leaf_displayed_bounds_for_tabs(host.space(), Some(window_id), root)
                .expect("transform-only frame should refresh displayed leaf facts");
            let generation = runtime
                .borrow()
                .adapter()
                .snapshot_facts_generation(host.space(), window_id)
                .expect("transform-only frame should publish route facts");
            (tabs_layout, tabs_displayed, generation)
        });

    assert_eq!(first_tabs_layout, second_tabs_layout);
    assert_ne!(first_tabs_displayed, second_tabs_displayed);
    assert_bounds_close(
        second_runtime_displayed,
        second_tabs_displayed,
        "second transformed leaf window bounds",
    );
    assert_ne!(
        first_generation, second_generation,
        "transform-only frames must invalidate stale route facts"
    );

    visual.simulate_mouse_move(point(px(f32::MAX), px(f32::MAX)), None, Modifiers::none());
    host.update(cx, |_, cx| cx.notify());
    visual.update(|window, cx| window.draw(cx).clear());
    host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_none(),
            "an early inverse-projection failure must retract the previous route scene"
        );
        assert!(host.interaction().viewport_host_scene_frame().is_none());
        assert!(host.last_presentation_scene().is_none());
    });

    visual.simulate_mouse_move(second_runtime_displayed.center(), None, Modifiers::none());
    host.update(cx, |_, cx| cx.notify());
    visual.update(|window, cx| window.draw(cx).clear());
    host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_some(),
            "the next valid frame must republish route geometry"
        );
        assert!(host.interaction().viewport_host_scene_frame().is_some());
        assert!(host.last_presentation_scene().is_some());
    });

    window
        .update(cx, |fixture, _window, cx| {
            fixture.show_host = false;
            cx.notify();
        })
        .expect("transformed DockHost fixture window should remain live");
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
    host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_none(),
            "removing the host subtree must expire its previous route publication"
        );
        assert!(host.interaction().viewport_host_scene_frame().is_none());
        assert!(host.last_presentation_scene().is_none());
    });

    window
        .update(cx, |fixture, _window, cx| {
            fixture.show_host = true;
            cx.notify();
        })
        .expect("transformed DockHost fixture window should remain live");
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
    let restored_generation = host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_some(),
            "restoring the host subtree must publish a fresh route scene"
        );
        host.viewport_runtime()
            .borrow()
            .adapter()
            .snapshot_facts_generation(host.space(), window_id)
            .expect("the restored host subtree must publish current route facts")
    });

    window
        .update(cx, |fixture, _window, cx| {
            fixture.cache_probe_revision += 1;
            cx.notify();
        })
        .expect("cached DockHost fixture window should remain live");
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
    host.update(cx, |host, _| {
        let runtime = host.viewport_runtime();
        assert_eq!(
            runtime
                .borrow()
                .adapter()
                .snapshot_facts_generation(host.space(), window_id),
            Some(restored_generation),
            "cached journal replay must preserve the restored route generation"
        );
        assert!(host.interaction().viewport_host_scene_frame().is_some());
        assert!(host.last_presentation_scene().is_some());
    });

    window
        .update(cx, |fixture, _window, cx| {
            fixture.fail_late = true;
            cx.notify();
        })
        .expect("transformed DockHost fixture window should remain live");
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());

    host.update(cx, |host, _| {
        let runtime = host.viewport_runtime();
        assert!(
            runtime
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_none(),
            "a paint-invalid transformed frame must retract the previous route scene"
        );
        assert!(
            host.interaction().viewport_host_scene_frame().is_none(),
            "a paint-invalid transformed frame must retract the event-receiver proof"
        );
        assert!(
            host.last_presentation_scene().is_none(),
            "a paint-invalid transformed frame must retract its presentation scene"
        );
    });
}

#[open_gpui::test]
fn dock_host_presentation_suppresses_routes_and_republishes_fresh_geometry(
    cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime.clone(), cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(700.0), px(420.0)), move |_, _| {
        TransformedDockHostFixture {
            host: window_host,
            show_host: true,
            presentation: SubtreePresentation::Visible,
            alternate_transform: false,
            fail_late: false,
            cache_probe_revision: 0,
        }
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let window_id = window.window_id();
    let host_selector = selector_for(&visual, &host, DockDebugRegion::Host)
        .expect("visible DockHost should publish its debug selector");
    let visible_displayed_bounds = host.update(cx, |host, _| {
        host.viewport_runtime()
            .rendered_leaf_displayed_bounds_for_tabs(host.space(), Some(window_id), root)
            .expect("visible DockHost should publish displayed route geometry")
    });

    start_tab_drag(&mut visual, &host, root, "a");
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("a"),
        },
    )
    .expect("source tab selector should remain available while starting the drag");
    let source_center = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_move(
        point(source_center.x + px(96.0), source_center.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    let payload = visual.update(|window, cx| {
        assert!(window.captured_pointer().is_some());
        assert!(
            window.accepts_pointer_input(),
            "payload drag must not make the source content window click-through"
        );
        assert!(cx.has_active_drag());
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("a real tab drag should publish its Dock payload")
    });
    host.update(cx, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_some());
    });

    window
        .update(cx, |fixture, _, cx| {
            fixture.presentation = SubtreePresentation::Inert;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(
        visual.debug_bounds(&host_selector).is_some(),
        "inert DockHost should remain painted and diagnosable"
    );
    visual.update(|window, cx| {
        assert!(window.captured_pointer().is_none());
        assert!(window.accepts_pointer_input());
        assert!(!cx.has_active_drag());
    });
    host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_none(),
            "inert DockHost must retract cross-window route geometry"
        );
        assert!(host.interaction().viewport_host_scene_frame().is_none());
        assert!(host.last_presentation_scene().is_none());
        assert!(host.active_payload_drag_session(&payload).is_none());
    });

    window
        .update(cx, |fixture, _, cx| {
            fixture.presentation = SubtreePresentation::Visible;
            fixture.alternate_transform = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());

    start_tab_drag(&mut visual, &host, root, "a");
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("a"),
        },
    )
    .expect("restored source tab selector should be available");
    let source_center = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_move(
        point(source_center.x + px(96.0), source_center.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    let hidden_payload = visual.update(|window, cx| {
        assert!(window.captured_pointer().is_some());
        assert!(
            window.accepts_pointer_input(),
            "the second payload drag must preserve source-window input"
        );
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("the second real tab drag should publish its Dock payload")
    });
    host.update(cx, |host, _| {
        assert!(host.active_payload_drag_session(&hidden_payload).is_some());
    });

    window
        .update(cx, |fixture, _, cx| {
            fixture.presentation = SubtreePresentation::Hidden;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.update(|window, cx| {
        assert!(window.captured_pointer().is_none());
        assert!(window.accepts_pointer_input());
        assert!(!cx.has_active_drag());
    });
    host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_none()
        );
        assert!(host.interaction().viewport_host_scene_frame().is_none());
        assert!(host.active_payload_drag_session(&hidden_payload).is_none());
    });

    window
        .update(cx, |fixture, _, cx| {
            fixture.presentation = SubtreePresentation::Visible;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
    host.update(cx, |host, _| {
        assert!(
            host.viewport_runtime()
                .rendered_leaf_bounds_for_tabs(host.space(), Some(window_id), root)
                .is_some(),
            "restored DockHost should publish fresh route geometry"
        );
        let restored_displayed_bounds = host
            .viewport_runtime()
            .rendered_leaf_displayed_bounds_for_tabs(host.space(), Some(window_id), root)
            .expect("restored DockHost should publish displayed route geometry");
        assert_ne!(restored_displayed_bounds, visible_displayed_bounds);
        assert!(host.active_payload_drag_session(&payload).is_none());
    });
}

#[open_gpui::test]
fn render_measured_tab_label_fact_overrides_scene_equal_slot_estimate(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["short", "long"], "short");
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("short", "S", "Short"),
            ("long", "A very long measured tab label", "Long"),
        ],
        size(px(480.0), px(200.0)),
    );
    let window_id = window.window_id();

    let short_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("short"),
        },
    )
    .expect("short tab selector should be emitted");
    let rendered_short_bounds = debug_bounds(&mut visual, &short_tab);
    let (scene_label_bounds, runtime_label_bounds) = host.update(cx, |host, cx| {
        let scene = host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 480.0, 200.0), cx);
        let scene_label_bounds = scene
            .tab_labels
            .iter()
            .find(|label| label.tabs == root && label.index == 0)
            .expect("scene label should exist")
            .bounds;
        let runtime_label_bounds = host
            .viewport_runtime()
            .rendered_tab_label_bounds_for_tabs(host.space(), Some(window_id), root, 0)
            .expect("runtime should keep render-measured tab label fact");
        (scene_label_bounds, runtime_label_bounds)
    });

    assert_bounds_close(
        runtime_label_bounds,
        rendered_short_bounds,
        "render-measured tab label fact",
    );
    assert!(
        (f32::from(runtime_label_bounds.size.width) - f32::from(scene_label_bounds.size.width))
            .abs()
            > 1.0,
        "tab label probe should preserve intrinsic render width when it differs from scene equal slots: runtime={runtime_label_bounds:?} scene={scene_label_bounds:?}"
    );
}

#[open_gpui::test]
fn render_tab_bar_bounds_match_presentation_scene_tab_bar(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["short", "long"], "short");
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("short", "S", "Short"),
            ("long", "A very long measured tab label", "Long"),
        ],
        size(px(480.0), px(200.0)),
    );
    let tab_bar = selector_for(&visual, &host, DockDebugRegion::TabBar { node: root })
        .expect("tab bar selector should be emitted");
    let rendered_tab_bar_bounds = debug_bounds(&mut visual, &tab_bar);
    let window_id = window.window_id();
    let (scene_tab_bar_bounds, runtime_tab_bar_bounds) = host.update(cx, |host, cx| {
        let scene = host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 480.0, 200.0), cx);
        let scene_tab_bar_bounds = scene
            .tab_bar_for_node(root)
            .expect("scene tab bar should exist")
            .bounds;
        let runtime_tab_bar_bounds = host
            .viewport_runtime()
            .rendered_tab_bar_bounds_for_tabs(host.space(), Some(window_id), root)
            .expect("runtime should keep scene-seeded tab bar fact");
        (scene_tab_bar_bounds, runtime_tab_bar_bounds)
    });

    assert_bounds_close(
        rendered_tab_bar_bounds,
        scene_tab_bar_bounds,
        "rendered tab bar",
    );
    assert_bounds_close(
        runtime_tab_bar_bounds,
        scene_tab_bar_bounds,
        "scene-seeded tab bar fact",
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
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let tabs_selector = selector_for(&visual, &host, DockDebugRegion::Tabs { node: root })
        .expect("tabs selector should be emitted");
    let tabs_bounds = debug_bounds(&mut visual, &tabs_selector);
    visual.simulate_mouse_move(tabs_bounds.center(), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let expected_boxes = crate::geometry::drop_boxes_with_style(
        tabs_bounds,
        crate::geometry::DockDropBoxSet::Inner,
        crate::DockDropGuideMetrics::default(),
    );

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
        let guide_bounds = debug_bounds(&mut visual, &guide);
        let expected = expected_boxes
            .iter()
            .find(|drop_box| drop_box.kind.zone() == zone)
            .unwrap_or_else(|| panic!("{zone:?} drop box should exist"));
        assert_bounds_close(
            guide_bounds,
            expected.draw_bounds,
            &format!("{zone:?} guide"),
        );
    }
}

#[open_gpui::test]
fn rendered_scene_frame_is_published_for_event_receiver_local_routing(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let host_position = inner_edge_drop_position(target_bounds, crate::DropZone::Left);
    let any_window: AnyWindowHandle = window.into();
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());
    let (runtime, scene_proof) = cx.update_entity(&host, |host, cx| {
        host.viewport_runtime()
            .begin_payload_drag_with_app(&payload, cx);
        (
            host.viewport_runtime().clone(),
            host.interaction().viewport_host_scene_frame().cloned(),
        )
    });
    let scene_proof =
        scene_proof.expect("rendered host scene frame should be published for routing");

    let request = DockViewportDropRouteRequest::from_platform_signals(
        space(),
        left_tabs,
        DockViewportDropPayload::Item(item("a")),
        host_position,
        None,
        DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
        )
        .with_event_receiver_window(any_window)
        .with_global_window_bounds(false),
    )
    .with_event_receiver_local_scene_proof(Some(scene_proof));
    let resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery_for_request(&request, app));

    assert!(
        matches!(resolution.route(), DockViewportDropRoute::Local { .. }),
        "event-receiver local routing should use the rendered scene frame, got {:?}",
        resolution.route()
    );
    assert!(
        resolution.delivery().is_some(),
        "local route should resolve a concrete dock target from the rendered scene"
    );
}

#[open_gpui::test]
fn rendered_scene_route_resolves_nested_leaf_edge_from_scene_seeded_frame(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let upper_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("upper")],
        selected: Some(item("upper")),
    });
    let lower_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("lower")],
        selected: Some(item("lower")),
    });
    let nested = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![upper_tabs, lower_tabs],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, nested],
        fractions: vec![0.35, 0.65],
    });
    graph.set_root(space(), root);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("upper", "Upper", "Upper"),
            ("lower", "Lower", "Lower"),
        ],
        size(px(600.0), px(300.0)),
    );

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: lower_tabs })
        .expect("lower nested tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let host_position = inner_edge_drop_position(target_bounds, crate::DropZone::Left);
    let any_window: AnyWindowHandle = window.into();
    let payload = DockDragPayload::new_item(space(), source_tabs, item("a"), "Panel A".to_string());
    let (runtime, scene_proof) = cx.update_entity(&host, |host, cx| {
        host.viewport_runtime()
            .begin_payload_drag_with_app(&payload, cx);
        (
            host.viewport_runtime().clone(),
            host.interaction().viewport_host_scene_frame().cloned(),
        )
    });
    let scene_proof =
        scene_proof.expect("rendered host scene frame should be published for routing");

    let request = DockViewportDropRouteRequest::from_platform_signals(
        space(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        host_position,
        None,
        DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
        )
        .with_event_receiver_window(any_window)
        .with_global_window_bounds(false),
    )
    .with_event_receiver_local_scene_proof(Some(scene_proof));
    let resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery_for_request(&request, app));
    let target = resolution
        .delivery()
        .and_then(|delivery| delivery.workspace_target())
        .expect("rendered scene route should resolve a workspace target");

    assert!(
        matches!(
            &target.target().kind,
            crate::drop_target::DockResolvedDropTargetKind::InnerEdge {
                target_tabs,
                zone: crate::DropZone::Left,
                ..
            } if *target_tabs == lower_tabs
        ),
        "scene-seeded frame should target the nested lower leaf left edge, got {:?}",
        target.target()
    );
}

#[open_gpui::test]
fn rendered_scene_route_commits_edge_drop_without_local_scene_update(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let host_position = inner_edge_drop_position(target_bounds, crate::DropZone::Left);
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());

    window
        .update(cx, |host, window, cx| {
            host.begin_payload_drag_from_render(&payload, window, cx);
            host.update_payload_drag_hover_from_rendered_host_scene(
                &payload,
                host_position,
                window,
                cx,
            );
            assert!(
                host.interaction().drop_preview().is_none(),
                "host-level fallback should not leave stale local previews above the routed preview"
            );
            assert!(
                host.viewport_runtime()
                    .routed_drop_preview_for(host.space(), window.window_handle().window_id())
                    .is_some()
                    || host
                        .viewport_runtime()
                        .routed_drop_route_preview_for(
                            host.space(),
                            window.window_handle().window_id()
                        )
                        .is_some(),
                "rendered scene routing should produce a routed preview"
            );
            assert!(
                host.drop_payload_release_from_rendered_host_scene(
                    payload.clone(),
                    host_position,
                    window,
                    cx,
                ),
                "release should commit through the rendered scene route"
            );
        })
        .expect("host window should be live");

    cx.read_entity(&controller, |controller, _| {
        let root = controller
            .graph()
            .root(&space())
            .expect("space should keep a root after edge drop");
        let DockNode::Split { axis, children, .. } =
            controller.graph().node(root).expect("root should exist")
        else {
            panic!("root should be a split after edge drop");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(children.len(), 2);
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[0]),
            vec![item("a")],
            "left-edge drop should place the moved item in the first child"
        );
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[1]),
            vec![item("b")],
            "left-edge drop should keep the target item in the second child"
        );
    });
}

#[open_gpui::test]
fn rendered_scene_hover_preserves_local_rejected_preview_for_same_pointer_pass(
    cx: &mut TestAppContext,
) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace
        .policy_mut()
        .allow_dock_class_in_space(space(), "inspector");
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller, size(px(500.0), px(240.0)));

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let position = inner_edge_drop_position(target_bounds, crate::DropZone::Right);
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());

    window
        .update(cx, |host, window, cx| {
            host.begin_payload_drag_from_render(&payload, window, cx);
            host.begin_host_drop_scene_from_render(
                &payload,
                window.bounds(),
                position,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                crate::drop_scene_fact::leaf(root, right_tabs, target_bounds, false),
                position,
                window,
                cx,
            );
            assert!(
                host.interaction()
                    .drop_preview()
                    .is_some_and(|preview| !preview.scene.decision.is_allowed()),
                "local scene should produce a rejected preview before host fallback runs"
            );

            host.update_payload_drag_hover_from_rendered_host_scene(
                &payload,
                position,
                window,
                cx,
            );
            assert!(
                host.interaction()
                    .drop_preview()
                    .is_some_and(|preview| !preview.scene.decision.is_allowed()),
                "host-level fallback must not clear a local preview already produced for this pointer position"
            );
        })
        .expect("host window should be live");
}

#[open_gpui::test]
fn rounded_host_corner_retracts_route_proof_and_drop_preview(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller, size(px(500.0), px(240.0)));

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let preview_position = target_bounds.center();
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());
    let rounded_geometry = rounded_host_geometry(cx);
    let rounded_corner = point(px(1.0), px(1.0));

    assert!(
        rounded_geometry.layout_bounds().contains(&rounded_corner),
        "the test corner must remain inside the host AABB"
    );
    assert!(
        rounded_geometry.window_to_host(rounded_corner).is_none(),
        "the rounded corner must be outside the exact committed hit region"
    );

    window
        .update(cx, |host, window, cx| {
            host.begin_payload_drag_from_render(&payload, window, cx);
            host.begin_host_drop_scene_from_render(
                &payload,
                target_bounds,
                preview_position,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                drop_scene_fact::leaf(root, right_tabs, target_bounds, false),
                preview_position,
                window,
                cx,
            );
            assert!(
                host.interaction().viewport_host_scene_frame().is_some(),
                "an in-bounds drag move should publish a route proof"
            );
            assert!(
                host.interaction().drop_preview().is_some(),
                "an in-bounds drag move should establish a local drop preview"
            );

            host.begin_host_drop_scene_from_render(
                &payload,
                rounded_geometry,
                rounded_corner,
                window,
                cx,
            );
            assert!(
                host.interaction().viewport_host_scene_frame().is_none(),
                "an exact-hit miss must retract the route proof"
            );
            assert!(
                host.interaction().drop_preview().is_none(),
                "an exact-hit miss must retract the local preview"
            );
        })
        .expect("host window should be live");
}

#[open_gpui::test]
fn host_scene_expiry_watch_survives_manual_frame_callbacks(cx: &mut TestAppContext) {
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
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
}

#[open_gpui::test]
fn transition_sample_visual_affordance_renders_from_executor(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a"]);
    let (window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(400.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 240.0), cx)
    });
    let affordance_bounds = floating_bounds(32.0, 16.0, 90.0, 26.0);
    let affordance_scene =
        DockVisualAffordanceScene::from_test_layers(vec![DockVisualAffordanceLayer::test_layer(
            DockVisualAffordanceKind::PayloadGhost,
            affordance_bounds,
            Some(root),
            Some(crate::DropZone::Center),
            DockVisualLayerScope::Local,
            DockVisualAffordanceState::Active,
            Some(0),
            Some("Panel A".to_string()),
            None,
            Some("Preview Panel A".to_string()),
        )]);
    let plan = DockTransitionPlan::from_visual_affordance_scene(
        &scene,
        &affordance_scene,
        DockMotionPreference::Animated,
    );

    window
        .update(cx, |host, window, cx| {
            assert_eq!(
                host.execute_transition_plan(
                    plan,
                    MotionTransition::committed_layout(DockMotionPreference::Animated),
                    Some(window),
                    cx,
                ),
                DockTransitionExecutionState::Scheduled
            );
        })
        .expect("host should execute transition plan");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let transition_layer = selector_for(&visual, &host, DockDebugRegion::TransitionLayer)
        .expect("transition layer selector should be emitted");
    assert!(debug_bounds(&mut visual, &transition_layer).size.width > px(0.0));
    let transition_overlay = selector_for(
        &visual,
        &host,
        DockDebugRegion::TransitionVisualAffordance { index: 0 },
    )
    .expect("sampled visual affordance selector should be emitted");
    assert_bounds_close(
        debug_bounds(&mut visual, &transition_overlay),
        affordance_bounds,
        "sampled transition visual affordance",
    );
}

#[open_gpui::test]
fn transition_pane_clip_mounts_real_pane_content(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let host_bounds = floating_bounds(0.0, 0.0, 400.0, 240.0);
    let previous = single_tabs_presentation_scene(left_tabs, host_bounds);
    let next = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds, cx)
    });
    let final_right_bounds = next
        .panes
        .iter()
        .find(|pane| pane.node == Some(right_tabs))
        .expect("final scene should contain right tabs pane")
        .bounds;
    let plan = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Animated);

    let sample = host.update(cx, |host, cx| {
        assert_eq!(
            host.execute_transition_plan(
                plan,
                linear_continuity_transition(Duration::from_millis(1)),
                None,
                cx,
            ),
            DockTransitionExecutionState::Scheduled
        );
        host.sample_transition_for_test(Duration::ZERO)
            .expect("scheduled transition should expose a pane clip sample")
    });
    let clip = sample
        .pane_clips
        .iter()
        .find(|clip| clip.node == right_tabs)
        .expect("entering pane should expose a clip sample");
    assert_eq!(clip.content_bounds, final_right_bounds);
    assert_eq!(clip.occlusion_bounds, final_right_bounds);
}

#[open_gpui::test]
fn transition_projection_clip_mounts_final_size_pane_content(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let host_bounds = floating_bounds(0.0, 0.0, 400.0, 240.0);
    let previous = single_tabs_presentation_scene(left_tabs, host_bounds);
    let previous_left_bounds = previous
        .pane_for_node(left_tabs)
        .expect("previous scene should contain left tabs pane")
        .bounds;
    let next = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds, cx)
    });
    let final_left_bounds = next
        .pane_for_node(left_tabs)
        .expect("final scene should contain left tabs pane")
        .bounds;
    let plan = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Animated);

    window
        .update(cx, |host, window, cx| {
            assert_eq!(
                host.execute_transition_plan(
                    plan,
                    linear_continuity_transition(Duration::from_secs(60)),
                    Some(window),
                    cx,
                ),
                DockTransitionExecutionState::Scheduled
            );
        })
        .expect("host should execute transition plan");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let clip = selector_for(
        &visual,
        &host,
        DockDebugRegion::TransitionPaneClip { node: left_tabs },
    )
    .expect("resizing pane projection clip selector should be emitted");
    let content = selector_for(
        &visual,
        &host,
        DockDebugRegion::TransitionPaneContent { node: left_tabs },
    )
    .expect("resizing pane projected content selector should be emitted");
    let occlusion = selector_for(
        &visual,
        &host,
        DockDebugRegion::TransitionPaneOcclusion { node: left_tabs },
    )
    .expect("resizing pane projection occlusion selector should be emitted");

    let clip_bounds = debug_bounds(&mut visual, &clip);
    assert!(
        clip_bounds.size.width > final_left_bounds.size.width
            && clip_bounds.size.width <= previous_left_bounds.size.width,
        "projection clip should start from previous width and move toward final width"
    );
    assert_bounds_close(
        debug_bounds(&mut visual, &content),
        final_left_bounds,
        "resizing pane projected content",
    );
    assert_bounds_close(
        debug_bounds(&mut visual, &occlusion),
        final_left_bounds,
        "resizing pane projection occlusion",
    );
}

#[open_gpui::test]
fn drag_active_frames_do_not_schedule_background_scene_expiry(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    start_tab_drag(&mut visual, &host, root, "a");
    cx.run_until_parked();

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    let tabs_selector = selector_for(&visual, &host, DockDebugRegion::Tabs { node: root })
        .expect("tabs selector should be emitted");
    let tabs_bounds = debug_bounds(&mut visual, &tabs_selector);
    visual.simulate_mouse_move(tabs_bounds.center(), MouseButton::Left, Modifiers::none());

    cx.executor().advance_clock(Duration::from_millis(64));
    visual.run_until_parked();

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::DropGuide {
                node: Some(root),
                zone: crate::DropZone::Center,
            },
        )
        .is_some(),
        "drag-active host frames should remain stable after background timers advance"
    );
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
    activate_window_for_pointer_input(&mut visual);
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
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let right_bounds = debug_bounds(&mut visual, &right_stack);
    visual.simulate_mouse_move(right_bounds.center(), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
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
fn root_drop_guides_use_outer_edge_drop_box_geometry(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    start_tab_drag(&mut visual, &host, left_tabs, "a");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_tab);
    let start = source_bounds.center();
    let root_selector = selector_for(&visual, &host, DockDebugRegion::Split { node: root })
        .expect("root split selector should be emitted");
    let root_bounds = debug_bounds(&mut visual, &root_selector);
    let expected_boxes = crate::geometry::drop_boxes_with_style(
        root_bounds,
        crate::geometry::DockDropBoxSet::Outer,
        crate::DockDropGuideMetrics::default(),
    );
    let outer_left_hit = expected_boxes
        .iter()
        .find(|drop_box| drop_box.kind.zone() == crate::DropZone::Left)
        .expect("left outer drop box should exist")
        .hit_bounds
        .center();
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());
    window
        .update(cx, |host, window, cx| {
            host.begin_tab_item_drag_from_render(left_tabs, item("a"), &payload, window, cx);
            host.update_payload_drag_tear_off_geometry_from_render(
                &payload,
                crate::drag::DockDragTearOffGeometry::from_source_bounds(source_bounds, start)
                    .with_preferred_size(source_bounds.size),
            );
            host.begin_host_drop_scene_from_render(
                &payload,
                root_bounds,
                outer_left_hit,
                window,
                cx,
            );
            host.update_local_root_drop_scene_from_render(
                &payload,
                root,
                root_bounds,
                outer_left_hit,
                window,
                cx,
            );
        })
        .expect("host should publish root drop scene");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_drop_guide_not_emitted(&visual, &host, None, crate::DropZone::Center);
    for zone in [
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        let guide = selector_for(
            &visual,
            &host,
            DockDebugRegion::DropGuide { node: None, zone },
        )
        .unwrap_or_else(|| panic!("{zone:?} root guide selector should be emitted"));
        let guide_bounds = debug_bounds(&mut visual, &guide);
        let expected = expected_boxes
            .iter()
            .find(|drop_box| drop_box.kind.zone() == zone)
            .unwrap_or_else(|| panic!("{zone:?} outer drop box should exist"));
        assert_bounds_close(
            guide_bounds,
            expected.draw_bounds,
            &format!("{zone:?} root guide"),
        );
    }
}

#[open_gpui::test]
fn root_edge_hover_keeps_target_leaf_side_guides_visible(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    start_tab_drag(&mut visual, &host, left_tabs, "a");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_tab);
    let start = source_bounds.center();
    let root_selector = selector_for(&visual, &host, DockDebugRegion::Split { node: root })
        .expect("root split selector should be emitted");
    let root_bounds = debug_bounds(&mut visual, &root_selector);
    let right_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("right stack selector should be emitted");
    let right_bounds = debug_bounds(&mut visual, &right_stack);
    let outer_boxes = crate::geometry::drop_boxes_with_style(
        root_bounds,
        crate::geometry::DockDropBoxSet::Outer,
        crate::DockDropGuideMetrics::default(),
    );
    let outer_right_hit = outer_boxes
        .iter()
        .find(|drop_box| drop_box.kind.zone() == crate::DropZone::Right)
        .expect("right outer drop box should exist")
        .hit_bounds
        .center();
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());
    window
        .update(cx, |host, window, cx| {
            host.begin_tab_item_drag_from_render(left_tabs, item("a"), &payload, window, cx);
            host.update_payload_drag_tear_off_geometry_from_render(
                &payload,
                crate::drag::DockDragTearOffGeometry::from_source_bounds(source_bounds, start)
                    .with_preferred_size(source_bounds.size),
            );
            host.begin_host_drop_scene_from_render(
                &payload,
                root_bounds,
                outer_right_hit,
                window,
                cx,
            );
            host.update_local_root_drop_scene_from_render(
                &payload,
                root,
                root_bounds,
                outer_right_hit,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                drop_scene_fact::leaf(root, right_tabs, right_bounds, false),
                outer_right_hit,
                window,
                cx,
            );
        })
        .expect("host should publish root drop scene");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let expected_inner_boxes = crate::geometry::drop_boxes_with_style(
        right_bounds,
        crate::geometry::DockDropBoxSet::Inner,
        crate::DockDropGuideMetrics::default(),
    );

    assert_drop_guide_not_emitted(&visual, &host, Some(right_tabs), crate::DropZone::Center);
    for zone in [
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_emitted(&visual, &host, None, zone);
        let guide = selector_for(
            &visual,
            &host,
            DockDebugRegion::DropGuide {
                node: Some(right_tabs),
                zone,
            },
        )
        .unwrap_or_else(|| panic!("{zone:?} right stack guide selector should be emitted"));
        let guide_bounds = debug_bounds(&mut visual, &guide);
        let expected = expected_inner_boxes
            .iter()
            .find(|drop_box| drop_box.kind.zone() == zone)
            .unwrap_or_else(|| panic!("{zone:?} inner drop box should exist"));
        assert_bounds_close(
            guide_bounds,
            expected.draw_bounds,
            &format!("{zone:?} right stack guide"),
        );
    }
}

#[open_gpui::test]
fn empty_host_center_guide_uses_center_drop_box_geometry(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("empty");
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let (_source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space.clone(),
        size(px(320.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller,
        runtime,
        target_space.clone(),
        size(px(420.0), px(260.0)),
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
    let empty_selector = selector_for(&target_visual, &target_host, DockDebugRegion::EmptySpace)
        .expect("empty host selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let end = debug_bounds(&mut target_visual, &empty_selector).center();

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);

    let empty_bounds = debug_bounds(&mut target_visual, &empty_selector);
    let expected_center = crate::geometry::drop_boxes_with_style(
        empty_bounds,
        crate::geometry::DockDropBoxSet::Inner,
        crate::DockDropGuideMetrics::default(),
    )
    .into_iter()
    .find(|drop_box| drop_box.kind == crate::geometry::DockDropBoxKind::Center)
    .expect("empty host center drop box should exist");
    let center_guide = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::DropGuide {
            node: None,
            zone: crate::DropZone::Center,
        },
    )
    .expect("empty host center guide selector should be emitted");
    assert_bounds_close(
        debug_bounds(&mut target_visual, &center_guide),
        expected_center.draw_bounds,
        "empty host center guide",
    );
    cx.set_platform_hovered_window(None);
}

#[open_gpui::test]
fn cross_window_leaf_interior_hover_keeps_guide_only_preview(cx: &mut TestAppContext) {
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

    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let source_window = cx.open_window(size(px(320.0), px(220.0)), {
        let controller = controller.clone();
        let runtime = runtime.clone();
        let source_space = source_space.clone();
        move |_, cx| {
            DockHost::from_controller(
                controller.clone(),
                source_space.clone(),
                runtime.clone(),
                cx,
            )
        }
    });
    let source_host = source_window
        .root(cx)
        .expect("source window should expose DockHost root");
    let target_window = cx.open_window(size(px(420.0), px(260.0)), {
        let controller = controller.clone();
        let runtime = runtime.clone();
        let target_space = target_space.clone();
        move |_, cx| {
            DockHost::from_controller(
                controller.clone(),
                target_space.clone(),
                runtime.clone(),
                cx,
            )
        }
    });
    let target_host = target_window
        .root(cx)
        .expect("target window should expose DockHost root");
    cx.run_until_parked();

    let mut source_visual = VisualTestContext::from_window(source_window.into(), cx);
    start_tab_drag(&mut source_visual, &source_host, source_tabs, "a");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut target_visual, &target_tabs_selector);
    let interior_miss = open_gpui::point(
        target_bounds.origin.x + target_bounds.size.width * 0.78,
        target_bounds.origin.y + target_bounds.size.height * 0.5,
    );
    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(interior_miss, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropPreviewBody,
        )
        .is_none(),
        "guide-only hover should not render a concrete body preview"
    );
    for zone in [
        crate::DropZone::Center,
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_emitted(&target_visual, &target_host, Some(target_tabs), zone);
    }
    cx.set_platform_hovered_window(None);
}

#[open_gpui::test]
fn cross_window_inner_edge_expanded_hit_area_docks(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source-expanded");
    let target_space = DockSpaceId::from("target-expanded");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let source_window = cx.open_window(size(px(320.0), px(220.0)), {
        let controller = controller.clone();
        let runtime = runtime.clone();
        let source_space = source_space.clone();
        move |_, cx| {
            DockHost::from_controller(
                controller.clone(),
                source_space.clone(),
                runtime.clone(),
                cx,
            )
        }
    });
    let source_host = source_window
        .root(cx)
        .expect("source window should expose DockHost root");
    let target_window = cx.open_window(size(px(420.0), px(260.0)), {
        let controller = controller.clone();
        let runtime = runtime.clone();
        let target_space = target_space.clone();
        move |_, cx| {
            DockHost::from_controller(
                controller.clone(),
                target_space.clone(),
                runtime.clone(),
                cx,
            )
        }
    });
    let target_host = target_window
        .root(cx)
        .expect("target window should expose DockHost root");
    cx.run_until_parked();

    let mut source_visual = VisualTestContext::from_window(source_window.into(), cx);
    start_tab_drag(&mut source_visual, &source_host, source_tabs, "a");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut target_visual, &target_tabs_selector);
    let right_box = crate::geometry::drop_boxes_with_style(
        target_bounds,
        crate::geometry::DockDropBoxSet::Inner,
        crate::DockDropGuideMetrics::default(),
    )
    .into_iter()
    .find(|drop_box| {
        drop_box.kind == crate::geometry::DockDropBoxKind::InnerEdge(crate::DropZone::Right)
    })
    .expect("right inner drop box should exist");
    let expanded_hit = open_gpui::point(
        right_box.draw_bounds.origin.x - px(1.0),
        target_bounds.center().y,
    );
    assert!(
        !right_box.draw_bounds.contains(&expanded_hit),
        "test point should exercise the expanded ImGui-style hit area outside the drawn guide box"
    );
    assert!(
        right_box.hit_bounds.contains(&expanded_hit),
        "test point should still be inside the expanded ImGui-style hit area"
    );

    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(expanded_hit, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert_drop_guide_emitted(
        &target_visual,
        &target_host,
        Some(target_tabs),
        crate::DropZone::Right,
    );
    target_visual.simulate_mouse_up(expanded_hit, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);

    controller.update(cx, |controller, _| {
        let DockNode::Tabs { items, .. } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should still exist")
        else {
            panic!("source should remain a tabs node");
        };
        assert_eq!(
            items,
            &[item("c")],
            "source should retain the non-dragged tab after expanded edge drop"
        );
        let target_root = controller
            .graph()
            .root(&target_space)
            .expect("target space should still have a root");
        let DockNode::Split { children, .. } = controller
            .graph()
            .node(target_root)
            .expect("target root should exist")
        else {
            panic!("expanded edge drop should split the target root");
        };
        assert_eq!(children.len(), 2);
        let DockNode::Tabs { items, .. } = controller
            .graph()
            .node(children[1])
            .expect("right child should exist")
        else {
            panic!("right child should be a tabs node");
        };
        assert_eq!(
            items,
            &[item("a")],
            "expanded right-edge drop should create a new right child"
        );
    });
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
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let tabs_selector = selector_for(&visual, &host, DockDebugRegion::Tabs { node: root })
        .expect("tabs selector should be emitted");
    let tabs_bounds = debug_bounds(&mut visual, &tabs_selector);
    visual.simulate_mouse_move(tabs_bounds.center(), MouseButton::Left, Modifiers::none());
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
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let central_selector =
        selector_for(&visual, &host, DockDebugRegion::Tabs { node: central_tabs })
            .expect("central tabs selector should be emitted");
    let central_bounds = debug_bounds(&mut visual, &central_selector);
    visual.simulate_mouse_move(
        central_bounds.center(),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert_drop_guide_not_emitted(&visual, &host, Some(central_tabs), crate::DropZone::Center);
    for zone in [
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_emitted(&visual, &host, Some(central_tabs), zone);
    }
}

#[open_gpui::test]
fn nested_central_region_drop_guides_keep_side_zones(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let central_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let sibling_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let nested = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![central_tabs, sibling_tabs],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, nested],
        fractions: vec![0.35, 0.65],
    });
    graph.set_root(space(), root);
    graph.set_central_region(space(), DockCentralRegion::with_node(central_tabs));
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace
        .policy_mut()
        .set_allow_central_region_dock_over(false);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(500.0), px(240.0)));

    start_tab_drag(&mut visual, &host, source_tabs, "a");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let central_selector =
        selector_for(&visual, &host, DockDebugRegion::Tabs { node: central_tabs })
            .expect("central tabs selector should be emitted");
    let central_bounds = debug_bounds(&mut visual, &central_selector);
    visual.simulate_mouse_move(
        central_bounds.center(),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert_drop_guide_not_emitted(&visual, &host, Some(central_tabs), crate::DropZone::Center);
    for zone in [
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        assert_drop_guide_emitted(&visual, &host, Some(central_tabs), zone);
    }
}

#[open_gpui::test]
fn root_central_leaf_hides_inner_side_guides(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let central_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(space(), central_tabs);
    graph.set_central_region(space(), DockCentralRegion::with_node(central_tabs));
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace
        .policy_mut()
        .set_allow_central_region_dock_over(false);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(500.0), px(240.0)));

    start_tab_drag(&mut visual, &host, central_tabs, "a");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: central_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_tab);
    let start = source_bounds.center();
    let root_selector = selector_for(&visual, &host, DockDebugRegion::Tabs { node: central_tabs })
        .expect("central root tabs selector should be emitted");
    let root_bounds = debug_bounds(&mut visual, &root_selector);
    let outer_boxes = crate::geometry::drop_boxes_with_style(
        root_bounds,
        crate::geometry::DockDropBoxSet::Outer,
        crate::DockDropGuideMetrics::default(),
    );
    let outer_left_hit = outer_boxes
        .iter()
        .find(|drop_box| drop_box.kind.zone() == crate::DropZone::Left)
        .expect("left outer drop box should exist")
        .hit_bounds
        .center();
    let payload =
        DockDragPayload::new_item(space(), central_tabs, item("a"), "Panel A".to_string());
    window
        .update(cx, |host, window, cx| {
            host.begin_tab_item_drag_from_render(central_tabs, item("a"), &payload, window, cx);
            host.update_payload_drag_tear_off_geometry_from_render(
                &payload,
                crate::drag::DockDragTearOffGeometry::from_source_bounds(source_bounds, start)
                    .with_preferred_size(source_bounds.size),
            );
            host.begin_host_drop_scene_from_render(
                &payload,
                root_bounds,
                outer_left_hit,
                window,
                cx,
            );
            host.update_local_root_drop_scene_from_render(
                &payload,
                central_tabs,
                root_bounds,
                outer_left_hit,
                window,
                cx,
            );
        })
        .expect("host should publish root drop scene");
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
        assert_drop_guide_emitted(&visual, &host, None, zone);
    }
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
        let session = host.presentation_session(cx);
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
    activate_window_for_pointer_input(visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
}

fn single_tabs_presentation_scene(
    tabs: DockNodeId,
    bounds: open_gpui::Bounds<open_gpui::Pixels>,
) -> DockPresentationScene {
    DockPresentationScene {
        space: space(),
        bounds,
        root: Some(tabs),
        panes: vec![DockPresentationPane {
            node: Some(tabs),
            kind: DockPresentationPaneKind::Tabs,
            bounds,
            floating: None,
            is_central: false,
        }],
        tab_bars: Vec::new(),
        tab_labels: Vec::new(),
        splitters: Vec::new(),
        floating_containers: Vec::new(),
        focus_regions: Vec::new(),
        overlay_anchors: Vec::new(),
    }
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
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), Some(true));
    });
}

#[open_gpui::test]
fn late_invalid_panel_focus_command_is_rejected_without_visible_replay(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = test_view(cx, "A");
    let panel_focus = cx.read_entity(&panel, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(700.0), px(420.0)), move |_, _| {
        TransformedDockHostFixture {
            host: window_host,
            show_host: true,
            presentation: SubtreePresentation::Visible,
            alternate_transform: false,
            fail_late: false,
            cache_probe_revision: 0,
        }
    });
    let any_window = window.into();
    cx.run_until_parked();

    window
        .update(cx, |fixture, _, cx| {
            fixture.fail_late = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("a"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), None);
    });
    cx.update_window(any_window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
        assert!(!window.is_focus_handle_rendered(&panel_focus));
    })
    .unwrap();

    window
        .update(cx, |fixture, _, cx| {
            fixture.fail_late = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        assert!(
            window.focused(cx).is_none(),
            "restoring a valid frame must require a fresh panel focus command"
        );
        assert!(window.is_focus_handle_rendered(&panel_focus));
    })
    .unwrap();
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), None);
    });
}

#[open_gpui::test]
fn already_focused_descendant_is_not_recorded_when_the_candidate_fails_late(
    cx: &mut TestAppContext,
) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = cx.new(NestedFocusPanel::new);
    let child_focus = cx.read_entity(&panel, |panel, _| panel.child_focus.clone());
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(700.0), px(420.0)), move |_, _| {
        TransformedDockHostFixture {
            host: window_host,
            show_host: true,
            presentation: SubtreePresentation::Visible,
            alternate_transform: false,
            fail_late: false,
            cache_probe_revision: 0,
        }
    });
    let any_window = window.into();
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        child_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    host.update(cx, |host, _| {
        host.viewport_runtime().record_no_panel_focus(host.space());
        assert_eq!(host.recorded_had_panel_focus(), Some(false));
    });

    window
        .update(cx, |fixture, _, cx| {
            fixture.fail_late = true;
            cx.notify();
        })
        .unwrap();
    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("a"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(false),
            "an old rendered descendant cannot satisfy a focus command for a discarded candidate"
        );
    });
}

#[open_gpui::test]
fn panel_focus_command_preserves_an_already_focused_descendant(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = cx.new(NestedFocusPanel::new);
    let child_focus = cx.read_entity(&panel, |panel, _| panel.child_focus.clone());
    let root_focus = cx.read_entity(&panel, |panel, _| panel.root_focus.clone());
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    visual.update(|window, cx| child_focus.focus(window, cx));
    cx.run_until_parked();
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(child_focus.clone()));
        assert!(root_focus.contains_focused(window, cx));
    });

    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("a"))
        ));
        cx.notify();
    });
    cx.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(child_focus),
            "a satisfied panel focus command must preserve the descendant caret/focus owner"
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), Some(true));
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
    let (_window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
    let (_window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
    let (window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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

    visual.deactivate_window();
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate for initial backend focus confirmation");
    cx.run_until_parked();
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(focus_b.clone()),
            "initial backend focus suppression should not disturb already-focused dock panel"
        );
    });

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
    let (window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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

    visual.deactivate_window();
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate for initial backend focus confirmation");
    cx.run_until_parked();
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(focus_b.clone()),
            "initial backend focus suppression should not disturb already-focused dock panel"
        );
    });

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
fn platform_activation_notifies_when_pending_activation_is_consumed_without_new_focus_command(
    cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_b = test_view(cx, "B");
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));
    let controller = host.update(cx, |host, _| host.controller().clone());
    let registration = cx
        .read_entity(&host, |host, _| {
            host.viewport_runtime()
                .registration_key_for_space_window(host.space(), window.window_id())
        })
        .expect("rendered host should have an exact viewport registration");
    let activation = DockViewportActivationTransaction::registered(
        registration,
        window,
        DockViewportFocusRequest::panel("b"),
    );

    host.update(cx, |host, _| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item(
                "b"
            )))
        ));
        assert!(
            host.viewport_runtime()
                .record_pending_activation(activation.clone())
        );
    });

    visual.deactivate_window();
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate");
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(
            host.viewport_runtime().pending_activation(),
            None,
            "platform activation should consume the matching pending activation"
        );
        assert_eq!(
            host.pending_focus_command(),
            None,
            "consuming pending activation is a runtime change and must notify even when the focus command was already queued"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(focus_b));
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
}

#[open_gpui::test]
fn platform_activation_policy_can_leave_dock_focus_unchanged(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let panel_b = test_view(cx, "B");
    let focus_b = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));

    let mut workspace = DockWorkspace::new(space(), graph);
    workspace
        .policy_mut()
        .set_platform_focus_sets_dock_focus(false);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let (window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
        assert_eq!(window.focused(cx), Some(focus_b));
    });

    visual.update(|window, cx| {
        window.focus(&stealer, cx);
        assert_eq!(window.focused(cx), Some(stealer.clone()));
    });
    host.update(cx, |host, _| {
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "the opt-out should not erase recorded dock focus history"
        );
    });

    visual.deactivate_window();
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("window should activate");
    cx.run_until_parked();

    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "platform focus opt-out should skip restoration without rewriting focus history"
        );
    });
    visual.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(stealer),
            "policy-disabled platform activation should not restore recorded dock panel focus"
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
    let (_window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
    let (_window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
    let (_window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
fn viewport_failed_panel_focus_clears_hidden_focus_and_preserves_history(cx: &mut TestAppContext) {
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
            None,
            "failed explicit panel focus must not retain focus in the now-hidden panel subtree"
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
            None,
            "repeated failed focus requests must not revive a hidden panel focus target"
        );
    });
}

#[open_gpui::test]
fn viewport_restore_request_without_focus_history_preserves_current_focus(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let (_window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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
fn no_panel_focus_preserves_focus_outside_the_dock_host(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(400.0), px(240.0)), move |_, cx| {
        PresentedDockHostWithExternalFocus {
            host: window_host,
            presentation: SubtreePresentation::Visible,
            external_focus: cx.focus_handle(),
        }
    });
    let fixture = window.root(cx).unwrap();
    let any_window = window.into();
    cx.run_until_parked();

    let external_focus = cx.read(|cx| fixture.read(cx).external_focus.clone());
    cx.update_window(any_window, |_, window, cx| {
        external_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    host.update(cx, |host, cx| {
        host.viewport_runtime()
            .record_panel_focus(host.space().clone(), item("a"));
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(
                DockViewportFocusRequest::no_panel_focus()
            )
        ));
        cx.notify();
    });
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&external_focus));
    })
    .unwrap();
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), Some(false));
    });
}

#[open_gpui::test]
fn no_panel_focus_supersedes_uncommitted_panel_intent_and_preserves_external_focus(
    cx: &mut TestAppContext,
) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = test_view(cx, "A");
    let panel_focus = cx.read_entity(&panel, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(400.0), px(240.0)), move |_, cx| {
        PresentedDockHostWithExternalFocus {
            host: window_host,
            presentation: SubtreePresentation::Visible,
            external_focus: cx.focus_handle(),
        }
    });
    let fixture = window.root(cx).unwrap();
    let any_window = window.into();
    cx.run_until_parked();

    let external_focus = cx.read(|cx| fixture.read(cx).external_focus.clone());
    cx.update_window(any_window, |_, window, cx| {
        external_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    host.update(cx, |host, _| {
        host.viewport_runtime()
            .record_panel_focus(host.space().clone(), item("a"));
    });

    cx.update(|app| {
        app.update_window(any_window, |_, window, cx| {
            panel_focus.focus(window, cx);
            assert_eq!(window.focused(cx).as_ref(), Some(&panel_focus));
            assert_eq!(window.committed_focus(cx).as_ref(), Some(&external_focus));
        })
        .unwrap();
        host.update(app, |host, cx| {
            assert!(host.request_viewport_focus_command(
                DockViewportFocusCommand::viewport_activation(
                    DockViewportFocusRequest::no_panel_focus()
                )
            ));
            cx.notify();
        });
    });
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        assert_eq!(
            window.focused(cx).as_ref(),
            Some(&external_focus),
            "NoPanelFocus must supersede a pending panel claim without discarding committed external focus"
        );
        assert_eq!(window.committed_focus(cx).as_ref(), Some(&external_focus));
    })
    .unwrap();
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), Some(false));
    });
}

#[open_gpui::test]
fn no_panel_focus_supersedes_unbound_panel_claim_before_the_panel_mounts(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let panel_b_focus = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller.clone(), dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(400.0), px(240.0)), move |_, cx| {
        PresentedDockHostWithExternalFocus {
            host: window_host,
            presentation: SubtreePresentation::Visible,
            external_focus: cx.focus_handle(),
        }
    });
    let fixture = window.root(cx).unwrap();
    let any_window = window.into();
    cx.run_until_parked();

    let external_focus = cx.read(|cx| fixture.read(cx).external_focus.clone());
    cx.update_window(any_window, |_, window, cx| external_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    cx.update(|app| {
        app.update_window(any_window, |_, window, cx| {
            panel_b_focus.focus(window, cx);
            assert_eq!(window.focused(cx).as_ref(), Some(&external_focus));
            assert_eq!(window.committed_focus(cx).as_ref(), Some(&external_focus));
        })
        .unwrap();
        controller.update(app, |controller, cx| {
            let outcome = controller
                .select_tab(root, item("b"))
                .expect("selecting tab B should succeed");
            assert!(outcome.changed());
            cx.notify();
        });
        host.update(app, |host, cx| {
            host.viewport_runtime()
                .record_panel_focus(host.space().clone(), item("a"));
            assert!(host.request_viewport_focus_command(
                DockViewportFocusCommand::viewport_activation(
                    DockViewportFocusRequest::no_panel_focus()
                )
            ));
            cx.notify();
        });
    });
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&external_focus));
        assert_eq!(window.committed_focus(cx).as_ref(), Some(&external_focus));
    })
    .unwrap();
    controller.update(cx, |controller, _| {
        assert_eq!(
            controller.graph().selected_item_in_tabs(root),
            Some(item("b"))
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), Some(false));
    });
}

#[open_gpui::test]
fn suppressed_no_panel_focus_preserves_external_focus_and_panel_history(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(400.0), px(240.0)), move |_, cx| {
        PresentedDockHostWithExternalFocus {
            host: window_host,
            presentation: SubtreePresentation::Visible,
            external_focus: cx.focus_handle(),
        }
    });
    let fixture = window.root(cx).unwrap();
    let any_window = window.into();
    cx.run_until_parked();

    let external_focus = cx.read(|cx| fixture.read(cx).external_focus.clone());
    cx.update_window(any_window, |_, window, cx| {
        external_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    host.update(cx, |host, _| {
        host.viewport_runtime()
            .record_panel_focus(host.space().clone(), item("a"));
    });
    fixture.update(cx, |fixture, cx| {
        fixture.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(
                DockViewportFocusRequest::no_panel_focus()
            )
        ));
        cx.notify();
    });
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&external_focus));
    })
    .unwrap();
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(
            host.recorded_had_panel_focus(),
            Some(true),
            "suppressed hosts cannot publish no-panel focus history"
        );
    });
}

#[open_gpui::test]
fn suppressed_panel_focus_does_not_change_selected_tab(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "a");
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller.clone(), dock_space, runtime, cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(400.0), px(240.0)), move |_, cx| {
        PresentedDockHostWithExternalFocus {
            host: window_host,
            presentation: SubtreePresentation::Visible,
            external_focus: cx.focus_handle(),
        }
    });
    let fixture = window.root(cx).unwrap();
    cx.run_until_parked();

    host.update(cx, |host, _| {
        host.viewport_runtime()
            .record_panel_focus(host.space().clone(), item("a"));
    });
    fixture.update(cx, |fixture, cx| {
        fixture.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.run_until_parked();
    host.update(cx, |host, cx| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("b"))
        ));
        cx.notify();
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
        assert_eq!(
            selected.as_ref(),
            Some(&item("a")),
            "a suppressed focus command must not mutate DockGraph selection"
        );
    });
    host.update(cx, |host, _| {
        assert_eq!(host.pending_focus_command(), None);
        assert_eq!(host.recorded_had_panel_focus(), Some(true));
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
    let (window, host, mut visual, stealer) =
        open_workspace_with_external_focus(cx, workspace, size(px(400.0), px(240.0)));

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

fn set_render_passthrough_graph(
    controller: &Entity<DockController>,
    has_content: bool,
    cx: &mut TestAppContext,
) {
    controller.update(cx, |controller, cx| {
        let mut graph = DockGraph::new();
        let central = if has_content {
            let tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("a")],
                selected: Some(item("a")),
            });
            graph.set_root(space(), tabs);
            DockCentralRegion::with_node(tabs).with_passthrough_when_empty(true)
        } else {
            DockCentralRegion::empty().with_passthrough_when_empty(true)
        };
        graph.set_central_region(space(), central);
        controller.workspace_mut().set_graph(graph);
        cx.notify();
    });
}

fn last_queued_pointer_input_generation(runtime: &DockViewportRuntimeHandle) -> Option<u64> {
    runtime
        .runtime_status()
        .last_platform_dispatch?
        .dispatches
        .into_iter()
        .find_map(|dispatch| match dispatch {
            DockViewportPlatformSyncDispatch::Queued {
                request: crate::DockViewportPlatformSyncRequest::PointerInput { requested: false },
                generation,
                ..
            } => Some(generation),
            _ => None,
        })
}

#[open_gpui::test]
fn empty_central_passthrough_queues_pointer_input_without_mutating_committed_facts(
    cx: &mut TestAppContext,
) {
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
        window
            .update(cx, |_, window, _| window
                .platform_facts()
                .accepts_pointer_input)
            .expect("host window should remain live"),
        "queued pointer intent must not rewrite the committed platform fact"
    );
    let runtime = host.update(cx, |host, _| host.viewport_runtime().clone());
    assert!(
        runtime
            .runtime_status()
            .last_platform_dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.dispatches.iter().any(|entry| {
                matches!(
                    entry,
                    DockViewportPlatformSyncDispatch::Queued {
                        request: crate::DockViewportPlatformSyncRequest::PointerInput {
                            requested: false
                        },
                        ..
                    }
                )
            })),
        "empty central passthrough should queue a typed pointer-input request"
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
            .update(cx, |_, window, _| window
                .platform_facts()
                .accepts_pointer_input)
            .expect("host window should remain live"),
        "the superseding unchanged request must preserve the committed platform fact"
    );
    let status = runtime.runtime_status();
    assert!(
        status
            .last_platform_dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.dispatches.iter().any(|entry| {
                matches!(
                    entry,
                    DockViewportPlatformSyncDispatch::Unchanged {
                        request: crate::DockViewportPlatformSyncRequest::PointerInput {
                            requested: true
                        },
                    }
                )
            })),
        "repopulating the central region should supersede the queued pass-through request"
    );
    assert!(
        status.recent_platform_observations.iter().any(|record| {
            record.observation.outcome == DockViewportPlatformSyncObservationOutcome::Superseded
        }),
        "the old queued ticket must settle as superseded rather than retrying indefinitely"
    );
}

#[open_gpui::test]
fn immediate_pointer_input_failure_is_not_retried_until_render_intent_changes(
    cx: &mut TestAppContext,
) {
    let (graph, _) = tabs_graph(&["a"]);
    let workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, _visual) =
        open_controller_workspace(cx, controller.clone(), size(px(320.0), px(200.0)));
    let runtime = host.update(cx, |host, _| host.viewport_runtime().clone());

    cx.set_next_window_pointer_input_dispatch(window.into(), PlatformWindowDispatch::Rejected);
    set_render_passthrough_graph(&controller, false, cx);
    cx.run_until_parked();

    let rejected = runtime
        .runtime_status()
        .last_platform_dispatch
        .expect("the rejected pointer-input request should remain diagnostic");
    assert!(rejected.dispatches.iter().any(|dispatch| {
        matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Rejected(rejected)
                if matches!(
                    rejected.request,
                    crate::DockViewportPlatformSyncRequest::PointerInput { requested: false }
                )
        )
    }));

    window
        .update(cx, |_, window, _| window.refresh())
        .expect("host window should remain live");
    cx.run_until_parked();
    assert!(
        last_queued_pointer_input_generation(&runtime).is_none(),
        "an unchanged terminal failure must not be retried on the next render"
    );

    set_render_passthrough_graph(&controller, true, cx);
    cx.run_until_parked();
    set_render_passthrough_graph(&controller, false, cx);
    cx.run_until_parked();
    assert!(
        last_queued_pointer_input_generation(&runtime).is_some(),
        "leaving and re-entering passthrough changes intent and must permit a new dispatch"
    );
}

fn assert_async_pointer_input_terminal_is_not_retried(
    cx: &mut TestAppContext,
    terminal: PlatformWindowMutationTerminal,
    expected: DockViewportPlatformSyncObservationOutcome,
) {
    let (graph, _) = tabs_graph(&["a"]);
    let workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, _visual) =
        open_controller_workspace(cx, controller.clone(), size(px(320.0), px(200.0)));
    let runtime = host.update(cx, |host, _| host.viewport_runtime().clone());

    set_render_passthrough_graph(&controller, false, cx);
    cx.run_until_parked();
    let generation = last_queued_pointer_input_generation(&runtime)
        .expect("passthrough should queue a pointer-input request");
    let facts = window
        .update(cx, |_, window, _| window.platform_facts().clone())
        .expect("host window should remain live");
    assert!(cx.simulate_window_mutation_terminal(
        window.into(),
        WindowMutationDomain::PointerInput,
        terminal,
        facts,
    ));

    for _ in 0..2 {
        window
            .update(cx, |_, window, _| window.refresh())
            .expect("host window should remain live");
        cx.run_until_parked();
    }

    assert_eq!(
        last_queued_pointer_input_generation(&runtime),
        Some(generation),
        "the same request and committed facts must retain the original dispatch generation"
    );
    assert!(
        runtime
            .runtime_status()
            .recent_platform_observations
            .iter()
            .any(|record| {
                record.observation.generation == generation
                    && record.observation.outcome == expected
            })
    );
}

#[open_gpui::test]
fn asynchronous_pointer_input_terminal_failures_do_not_retry_per_frame(cx: &mut TestAppContext) {
    for (terminal, expected) in [
        (
            PlatformWindowMutationTerminal::Observed,
            DockViewportPlatformSyncObservationOutcome::Adjusted,
        ),
        (
            PlatformWindowMutationTerminal::Rejected,
            DockViewportPlatformSyncObservationOutcome::Rejected,
        ),
        (
            PlatformWindowMutationTerminal::Unsupported,
            DockViewportPlatformSyncObservationOutcome::Unsupported,
        ),
        (
            PlatformWindowMutationTerminal::WindowClosed,
            DockViewportPlatformSyncObservationOutcome::WindowClosed,
        ),
    ] {
        assert_async_pointer_input_terminal_is_not_retried(cx, terminal, expected);
    }
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
        "floating visual affordance should still render above the empty central region"
    );
    let runtime = host.update(cx, |host, _| host.viewport_runtime().clone());
    assert_eq!(
        runtime.runtime_status().last_platform_dispatch,
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

    let external_dispatch = window
        .update(cx, |_, window, _| window.set_accepts_pointer_input(false))
        .expect("host window should remain live");
    assert!(
        external_dispatch.ticket().is_some(),
        "external pointer-input ownership should begin as queued intent"
    );
    assert!(
        cx.flush_window_mutation(window.into(), open_gpui::WindowMutationDomain::PointerInput),
        "test platform should publish the external pointer-input observation"
    );
    assert!(
        !window
            .update(cx, |_, window, _| window.accepts_pointer_input())
            .expect("host window should remain live"),
        "the observed external request should make the source viewport click-through"
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
fn floating_container_renders_panel_inside_affordance_bounds(cx: &mut TestAppContext) {
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
