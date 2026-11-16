//! Real GPUI capture packets retain their place ahead of later device facts.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    runtime::{
        DockspaceSession, NativePointerId, NativePointerOwner, NativeReceiverAnswer,
        NativeSurfaceBinding, PointerDeviceKind, PointerEventMetadata, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AnyWindowHandle, Context, Empty, IntoElement, Modifiers, MouseButton, NativeCapturedDragEvent,
    NativeCapturedDragGeneration, NativeCapturedDragPhase, NativeCapturedDragReleaseTerminal,
    PlatformPointerCaptureReleaseOutcome, Render, Subscription, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px,
};

use super::Inbound;
use crate::adapter::native::{NativeBridge, NativeBridgeError, NativeCallbacks, NativeReport};

struct DragView;

#[open_gpui::test]
fn inspection_retains_original_rejected_head_without_consuming_or_replaying_it(
    cx: &mut TestAppContext,
) {
    use dockspace::{
        policy::DockPolicy,
        runtime::{NativeHostCapabilities, NativeHostErrorKind},
    };
    let mut session =
        DockspaceSession::from_layout(DockspaceLayout::empty(), DockPolicy::default()).unwrap();
    let transport = super::NativeTransport::new(|_| {});
    let mut bridge = NativeBridge::enable(
        &mut session,
        transport.clone(),
        NativeHostCapabilities::none_supported(),
        None,
    )
    .unwrap();
    cx.update(|cx| transport.device_removed(cx));
    let mut unavailable =
        DockspaceSession::from_layout(DockspaceLayout::empty(), DockPolicy::default()).unwrap();
    assert!(bridge.collect(&mut unavailable).is_err());
    let before = transport.inspection();
    assert_eq!(before.front_record, Some("device-removed"));
    assert_eq!(before.queued_record_count, 1);
    assert_eq!(
        before.rejection.as_ref().unwrap().native_kind,
        Some(NativeHostErrorKind::NotEnabled)
    );
    assert_eq!(transport.inspection(), before);
    assert!(bridge.collect(&mut unavailable).is_err());
    assert_eq!(transport.inspection(), before);
    assert_eq!(bridge.collect(&mut session).unwrap(), 1);
    let after = transport.inspection();
    assert_eq!(after.queued_record_count, 0);
    assert!(after.front_record.is_none());
    assert!(after.rejection.is_none());
}

impl Render for DragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("ordered-native-drag").size_full().on_drag(
            "original FIFO payload",
            |_, geometry, _, cx| {
                let _consumer = geometry.prepare_native_captured_drag_consumer();
                cx.new(|_| Empty)
            },
        )
    }
}

struct ObservedStartView(Rc<RefCell<Option<open_gpui::PreparedNativeCapturedDragConsumer>>>);

impl Render for ObservedStartView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let starts = self.0.clone();
        div().id("observed-native-start").size_full().on_drag(
            "original prepared capture",
            move |_, geometry, _, cx| {
                *starts.borrow_mut() = Some(geometry.prepare_native_captured_drag_consumer());
                cx.new(|_| Empty)
            },
        )
    }
}

#[open_gpui::test]
fn desktop_publication_commits_before_its_suffix_and_rejects_stale_pointer_topology(
    cx: &mut TestAppContext,
) {
    use dockspace::runtime::HostWorkAreaToken;
    use open_gpui::{
        Bounds, DevicePixels, PlatformDisplaySnapshot, PlatformNativePointerPhysicalFrame,
        PlatformPhysicalDisplayObservation, PlatformWindowPhysicalGeometry, size,
    };

    let (mut session, mut bridge, _, _) = cx.update(crate::adapter::native::tests::fixture);
    for _ in 0..4 {
        cx.run_until_parked();
        commit_capture_transport(&mut session, &mut bridge);
        if bridge.transport().is_empty() {
            break;
        }
    }
    assert!(bridge.transport().is_empty());
    let (displays, primary) = cx.update(|cx| {
        (
            cx.displays(),
            cx.primary_display().map(|display| display.id()),
        )
    });
    assert_eq!(displays.len(), 1);
    let display_id = displays[0].id();
    let bounds = Bounds::new(
        point(DevicePixels(-2560), DevicePixels(533)),
        size(DevicePixels(2560), DevicePixels(1440)),
    );
    let display = |generation| {
        PlatformPhysicalDisplayObservation::try_new(
            generation,
            display_id,
            bounds,
            Bounds::new(bounds.origin, size(DevicePixels(2560), DevicePixels(1380))),
            1.25,
        )
        .unwrap()
    };
    let snapshot = |generation| {
        PlatformDisplaySnapshot::try_new(Some(generation), displays.clone(), primary).unwrap()
    };
    assert!(snapshot(1).with_physical_displays(vec![]).is_none());
    assert!(
        snapshot(1)
            .with_physical_displays(vec![display(2)])
            .is_none()
    );
    let pointer = |generation| {
        PlatformNativePointerPhysicalFrame::new(
            point(DevicePixels(-2000), DevicePixels(700)),
            PlatformWindowPhysicalGeometry::try_new(bounds, 1.25).unwrap(),
        )
        .with_target_display(display(generation))
        .unwrap()
    };
    let token = HostWorkAreaToken::new(u64::from(display_id));
    let first = snapshot(1)
        .with_physical_displays(vec![display(1)])
        .unwrap();
    let transport = bridge.transport();
    cx.update(|cx| {
        transport.desktop(&first, cx);
        transport.device_removed(cx);
    });
    assert_eq!(bridge.collect(&mut session).unwrap(), 1);
    assert_eq!(session.native_work_area(token), None);
    assert_eq!(bridge.desktop.work_area(&session, pointer(1)), None);
    assert!(!transport.is_empty());
    assert_eq!(bridge.collect(&mut session).unwrap(), 0);
    commit_capture_transport(&mut session, &mut bridge);
    let binding = session.native_work_area(token).unwrap();
    assert_eq!(
        bridge.desktop.work_area(&session, pointer(1)),
        Some(binding)
    );
    assert!(!transport.is_empty());
    commit_capture_transport(&mut session, &mut bridge);
    assert!(transport.is_empty());

    let second = snapshot(2)
        .with_physical_displays(vec![display(2)])
        .unwrap();
    cx.update(|cx| transport.desktop(&second, cx));
    assert_eq!(bridge.collect(&mut session).unwrap(), 1);
    assert_eq!(bridge.desktop.work_area(&session, pointer(1)), None);
    commit_capture_transport(&mut session, &mut bridge);
    assert!(bridge.desktop.work_area(&session, pointer(2)).is_some());

    cx.update(|cx| transport.desktop(&snapshot(3), cx));
    commit_capture_transport(&mut session, &mut bridge);
    assert_eq!(session.native_work_area(token), None);
    assert_eq!(bridge.desktop.work_area(&session, pointer(2)), None);
}

#[open_gpui::test]
fn captured_foreign_intent_uses_each_original_frame_and_preserves_unknown_hover(
    cx: &mut TestAppContext,
) {
    use dockspace::{
        geometry::PhysicalPoint,
        runtime::{
            HostWorkAreaToken, NativeDesktopPointerLocation, NativeDesktopPosition,
            NativeLocalToDesktopHandoff, NativePointerEvent, NativePointerHover,
            NativePointerInput, NativePointerLocation,
        },
    };
    use open_gpui::{
        Bounds, DevicePixels, MouseMoveEvent, MouseUpEvent, PlatformInput,
        PlatformNativePointerCaptureOwner, PlatformNativePointerPhysicalFrame,
        PlatformPhysicalDisplayObservation, PlatformWindowHit, PlatformWindowHitStack,
        PlatformWindowPhysicalCoverage, PlatformWindowPhysicalGeometry, size,
    };

    let display = cx.update(|cx| {
        let snapshot = cx.display_snapshot();
        let display = &snapshot.displays()[0];
        PlatformPhysicalDisplayObservation::try_new(
            snapshot.generation().unwrap(),
            display.id(),
            display.bounds().to_device_pixels(1.0),
            display.visible_bounds().to_device_pixels(1.0),
            1.0,
        )
        .unwrap()
    });
    cx.simulate_physical_display_roster(Some(vec![display]));
    let mut fixture = natural_capture(cx);
    let source = fixture.windows[1];
    let bounds = Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(640), DevicePixels(480)),
    );
    cx.set_platform_window_physical_client_geometry(source, Some(bounds), 1.0);
    cx.update_window(source, |_, window, cx| window.bounds_changed(cx))
        .unwrap();
    cx.update(|cx| {
        fixture
            .bridge
            .transport()
            .desktop(&cx.display_snapshot(), cx)
    });
    for _ in 0..4 {
        commit_capture_transport(&mut fixture.session, &mut fixture.bridge);
        if fixture.bridge.transport().is_empty() {
            break;
        }
    }
    assert!(
        fixture.bridge.transport().is_empty(),
        "the complete desktop publication must commit before sampling its work area"
    );
    let area = fixture
        .session
        .native_work_area(HostWorkAreaToken::new(u64::from(display.display_id())))
        .unwrap();
    let metadata = PointerEventMetadata::new(PointerDeviceKind::Mouse, Duration::from_millis(50));
    let mut visual = VisualTestContext::from_window(source, cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let binding = fixture.bindings[1].window();
    for (index, release, unknown) in [(0, false, false), (1, false, true), (2, true, false)] {
        let position = point(DevicePixels(80 + index * 20), DevicePixels(20));
        let frame = PlatformNativePointerPhysicalFrame::new(
            position,
            PlatformWindowPhysicalGeometry::try_new(bounds, 1.0).unwrap(),
        )
        .with_target_display(display)
        .unwrap()
        .with_capture_owner(if release {
            PlatformNativePointerCaptureOwner::None
        } else {
            PlatformNativePointerCaptureOwner::Window(source.window_id())
        });
        cx.set_platform_window_hit_stack(if unknown {
            PlatformWindowHitStack::Unavailable
        } else {
            PlatformWindowHitStack::try_available(
                position,
                vec![PlatformWindowHit::OpaqueBarrier {
                    coverage: PlatformWindowPhysicalCoverage::try_new(bounds).unwrap(),
                }],
            )
            .unwrap()
        });
        let local = point(px(position.x.0 as f32), px(position.y.0 as f32));
        cx.simulate_platform_input_with_native_pointer_physical_frame(
            source,
            if release {
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: local,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                })
            } else {
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: local,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                })
            },
            Some(frame),
        );
        cx.run_until_parked();
        let event = fixture
            .events
            .borrow()
            .last()
            .cloned()
            .expect("the original captured edge");
        assert_eq!(event.physical_frame(), Some(frame));
        let capture = if release {
            NativePointerOwner::None
        } else {
            NativePointerOwner::Native(binding)
        };
        let input = fixture.bridge.captured_pointer_input(
            &fixture.session,
            binding,
            capture,
            &event,
            NativePointerId::new(1),
            metadata,
        );
        let event_kind = if release {
            NativePointerEvent::ButtonReleased(dockspace::runtime::NativePointerButton::Primary)
        } else {
            NativePointerEvent::Moved
        };
        let location = NativeDesktopPointerLocation::new(
            NativeDesktopPosition::Exact(
                PhysicalPoint::new(f64::from(position.x.0), f64::from(position.y.0)).unwrap(),
            ),
            if unknown {
                NativePointerHover::Unknown
            } else {
                NativePointerHover::Foreign
            },
            None,
        );
        let expected = match input.location() {
            NativePointerLocation::LocalToDesktop(packet) => NativePointerInput::new_handoff(
                NativePointerId::new(1),
                event_kind,
                NativeLocalToDesktopHandoff::new(packet.local(), location),
                NativePointerOwner::Native(binding),
                capture,
            ),
            NativePointerLocation::Desktop(_) => NativePointerInput::new(
                NativePointerId::new(1),
                event_kind,
                location,
                NativePointerOwner::Native(binding),
                capture,
            ),
            NativePointerLocation::NativeLocal(_) => {
                panic!("an original desktop frame must retain its desktop facts")
            }
        }
        .with_metadata(metadata);
        assert_eq!(
            input,
            if unknown {
                expected
            } else {
                expected.with_native_tear_off(area, None)
            }
        );
    }
}

#[open_gpui::test]
fn a_later_activated_start_does_not_overtake_a_queued_native_terminal(cx: &mut TestAppContext) {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let source = windows[1];
    let callbacks = bridge.callbacks(bindings[1].window()).unwrap();
    let transport = bridge.transport();
    let starts = Rc::new(RefCell::new(None));
    let events = Rc::new(RefCell::new(Vec::new()));
    let _route = cx.update({
        let events = events.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new(()),
                move |event, cx| {
                    events.borrow_mut().push(event.clone());
                    transport.captured_drag(
                        bindings[1].window(),
                        NativePointerOwner::Unknown,
                        event,
                        NativePointerId::new(1),
                        PointerEventMetadata::new(
                            PointerDeviceKind::Mouse,
                            Duration::from_millis(31),
                        ),
                        cx,
                    );
                },
            )
        }
    });
    cx.update_window(source, |_, window, cx| {
        window.replace_root(cx, |_, _| ObservedStartView(starts.clone()));
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        commit_capture_transport(&mut session, &mut bridge);
    }
    let mut visual = VisualTestContext::from_window(source, cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let first = events
        .borrow()
        .last()
        .expect("original native move")
        .generation();
    commit_capture_transport(&mut session, &mut bridge);
    bridge.take_reports();
    let update = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap()
        .suspend();
    visual.simulate_mouse_up(
        point(px(80.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_down(
        point(px(100.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(140.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let second = starts
        .borrow()
        .clone()
        .expect("second original prepared consumer");
    assert!(second.is_active());
    assert_ne!(first, second.generation());
    assert!(
        events
            .borrow()
            .iter()
            .any(|event| event.generation() == first
                && event.phase() == NativeCapturedDragPhase::Released)
    );
    assert!(
        events
            .borrow()
            .iter()
            .all(|event| event.generation() == first),
        "the second native generation has not moved"
    );
    callbacks.retain_capture_start(source.window_id(), &second);
    drop(update);
    bridge.collect(&mut session).unwrap();
    let reports = bridge.take_reports();
    assert!(reports.iter().all(|report| !matches!(report, NativeReport::StaleCallback(binding) if *binding == bindings[1].window())),
        "an activated cleanup start cannot invalidate a preceding legitimate terminal: {reports:?}");
    visual.update(|_, cx| {
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(source.window_id())
        )
    });
}

#[open_gpui::test]
fn captured_motion_keeps_its_fifo_position_while_an_update_lease_is_held(cx: &mut TestAppContext) {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let transport = bridge.transport();
    let received = Rc::new(RefCell::new(Vec::new()));
    let metadata = PointerEventMetadata::new(PointerDeviceKind::Mouse, Duration::from_millis(17));
    let subscription = cx.update({
        let received = received.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new("original route lock"),
                move |event, cx| {
                    if event.phase() == NativeCapturedDragPhase::Moved {
                        received.borrow_mut().push(event.clone());
                        transport.captured_drag(
                            bindings[0].window(),
                            NativePointerOwner::Native(bindings[0].window()),
                            event,
                            NativePointerId::new(1),
                            metadata,
                            cx,
                        );
                        transport.device_removed(cx);
                    }
                },
            )
        }
    });
    cx.update_window(windows[0], |_, window, cx| {
        window.replace_root(cx, |_, _| DragView);
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    // Root replacement/activation can publish original native observations.
    // Settle those facts before suspending the candidate used by this probe.
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        bridge.collect(&mut session).unwrap();
        let mut frame = session
            .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
            .unwrap();
        frame
            .complete_unpainted_surfaces(dockspace::runtime::SurfaceUnavailableReason::Deferred)
            .unwrap();
        let report = frame.commit().unwrap();
        bridge.accept_report(report);
    }
    assert!(transport.is_empty());
    let update = session
        .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
        .unwrap()
        .suspend();
    let recorded = session.native_input_recorded_count();
    let mut visual = VisualTestContext::from_window(windows[0], cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    assert_eq!(
        session.native_input_recorded_count(),
        recorded,
        "the original candidate holds semantic publication while transport records arrivals"
    );
    let received = received.borrow();
    assert!(
        !received.is_empty(),
        "the test must receive an actual GPUI native captured move"
    );
    let expected = received.len();
    {
        let queue = transport.0.queue.borrow();
        assert_eq!(queue.len(), expected * 2);
        let mut queued = queue.iter();
        for original in received.iter() {
            let Some(Inbound::CapturedDrag {
                binding,
                capture,
                event,
                metadata: actual_metadata,
                ..
            }) = queued.next()
            else {
                panic!("later device removal overtook the original native capture packet");
            };
            assert_eq!(*binding, bindings[0].window());
            assert_eq!(*capture, NativePointerOwner::Native(bindings[0].window()));
            assert_eq!(event.sequence(), original.sequence());
            assert_eq!(event.generation(), original.generation());
            assert_eq!(event.source_window(), original.source_window());
            assert_eq!(event.physical_frame(), original.physical_frame());
            assert_eq!(event.payload::<&str>(), Some(&"original FIFO payload"));
            assert_eq!(*actual_metadata, metadata);
            assert!(matches!(queued.next(), Some(Inbound::DeviceRemoved)));
        }
        assert!(queued.next().is_none());
    }
    drop(received);
    drop(update);
    assert_eq!(bridge.collect(&mut session).unwrap(), expected * 2);
    assert!(transport.is_empty());
    assert!(session.native_input_recorded_count() > recorded);
    let _ = subscription;
}

#[open_gpui::test]
fn a_queued_release_cannot_cancel_the_next_real_drag_generation(cx: &mut TestAppContext) {
    queued_terminal_preserves_the_next_generation(cx, false);
}

#[open_gpui::test]
fn a_queued_cancellation_cannot_cancel_the_next_real_drag_generation(cx: &mut TestAppContext) {
    queued_terminal_preserves_the_next_generation(cx, true);
}

fn queued_terminal_preserves_the_next_generation(cx: &mut TestAppContext, cancel_first: bool) {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let transport = bridge.transport();
    let received = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let received = received.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new("original generation route"),
                move |event, cx| {
                    received.borrow_mut().push(event.clone());
                    transport.captured_drag(
                        bindings[0].window(),
                        NativePointerOwner::Unknown,
                        event,
                        NativePointerId::new(1),
                        PointerEventMetadata::new(
                            PointerDeviceKind::Mouse,
                            Duration::from_millis(19),
                        ),
                        cx,
                    );
                },
            )
        }
    });
    cx.update_window(windows[0], |_, window, cx| {
        window.replace_root(cx, |_, _| DragView);
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        bridge.collect(&mut session).unwrap();
        let mut frame = session
            .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
            .unwrap();
        frame
            .complete_unpainted_surfaces(dockspace::runtime::SurfaceUnavailableReason::Deferred)
            .unwrap();
        let report = frame.commit().unwrap();
        bridge.accept_report(report);
    }
    assert!(transport.is_empty());
    let update = session
        .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
        .unwrap()
        .suspend();
    let recorded = session.native_input_recorded_count();
    let mut visual = VisualTestContext::from_window(windows[0], cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    if cancel_first {
        visual.update(|window, cx| {
            window.cancel_pointer_session(open_gpui::PointerCancelReason::CaptureRevoked, cx);
        });
    }
    visual.simulate_mouse_up(
        point(px(80.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_down(
        point(px(100.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(140.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(160.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let events = received.borrow();
    let first_terminal = events
        .iter()
        .find(|event| !matches!(event.phase(), NativeCapturedDragPhase::Moved))
        .expect("the first real GPUI generation must terminate");
    if cancel_first {
        assert!(matches!(
            first_terminal.phase(),
            NativeCapturedDragPhase::Cancelled(_)
        ));
    } else {
        assert_eq!(first_terminal.phase(), NativeCapturedDragPhase::Released);
    }
    let second = events.last().expect("a second real GPUI drag must start");
    assert_eq!(second.phase(), NativeCapturedDragPhase::Moved);
    assert_ne!(first_terminal.generation(), second.generation());
    assert!(first_terminal.sequence().ordinal() < second.sequence().ordinal());
    let second_generation = second.generation();
    let second_sequence = second.sequence();
    let expected = events.len();
    drop(events);
    assert_eq!(session.native_input_recorded_count(), recorded);
    drop(update);
    assert_eq!(bridge.collect(&mut session).unwrap(), expected);
    visual.update(|_, cx| {
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(windows[0].window_id())
        );
        bridge
            .dispatch_effects(
                &mut |_, _, _| panic!("capture cleanup cannot create a window"),
                cx,
            )
            .unwrap();
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(windows[0].window_id()),
            "the first generation's deferred terminal must not cancel the current generation"
        );
    });
    visual.simulate_mouse_move(
        point(px(180.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let events = received.borrow();
    let continued = events.last().unwrap();
    assert_eq!(continued.phase(), NativeCapturedDragPhase::Moved);
    assert_eq!(continued.generation(), second_generation);
    assert!(continued.sequence().ordinal() > second_sequence.ordinal());
    assert!(events.iter().all(|event| {
        event.generation() != second_generation
            || matches!(event.phase(), NativeCapturedDragPhase::Moved)
    }));
    drop(events);
    bridge.collect(&mut session).unwrap();
    assert!(bridge.take_reports().iter().all(|report| {
        !matches!(report, crate::adapter::native::NativeReport::CaptureTerminal { barrier, .. }
            if barrier.drag_generation() == second_generation)
    }));
    let _ = subscription;
}

struct ContinuedCapture {
    session: DockspaceSession,
    bridge: NativeBridge,
    before: NativeSurfaceBinding,
    after: NativeSurfaceBinding,
    source: AnyWindowHandle,
    callbacks: NativeCallbacks,
    events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    generation: NativeCapturedDragGeneration,
    release_allowed: Rc<Cell<bool>>,
    _subscription: Subscription,
}

fn commit_capture_transport(session: &mut DockspaceSession, bridge: &mut NativeBridge) {
    if let Err(error) = bridge.collect(session) {
        assert!(
            session.has_pending_native_input(),
            "a rejected FIFO front needs an accepted prefix to commit: {error}"
        );
    }
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    bridge.accept_report(report);
}

fn continued_capture(cx: &mut TestAppContext) -> ContinuedCapture {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let before = bindings[1];
    let source = windows[1];
    let callbacks = bridge.callbacks(before.window()).unwrap();
    let transport = bridge.transport();
    let events = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let events = events.clone();
        let callbacks = callbacks.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new("continued physical capture"),
                move |event, cx| {
                    events.borrow_mut().push(event.clone());
                    transport.captured_drag(
                        callbacks.binding(),
                        NativePointerOwner::Unknown,
                        event,
                        NativePointerId::new(1),
                        PointerEventMetadata::new(
                            PointerDeviceKind::Mouse,
                            Duration::from_millis(23),
                        ),
                        cx,
                    );
                },
            )
        }
    });
    cx.update_window(source, |_, window, cx| {
        window.replace_root(cx, |_, _| DragView);
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        commit_capture_transport(&mut session, &mut bridge);
    }
    assert!(transport.is_empty());
    let mut visual = VisualTestContext::from_window(source, cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let generation = events
        .borrow()
        .last()
        .expect("a real GPUI capture must start before continuation")
        .generation();
    commit_capture_transport(&mut session, &mut bridge);
    let release_allowed = Rc::new(Cell::new(false));
    let attempts = Rc::new(Cell::new(0));
    cx.set_pointer_capture_release_callback(source, {
        let release_allowed = release_allowed.clone();
        let attempts = attempts.clone();
        move |_| {
            attempts.set(attempts.get() + 1);
            if release_allowed.get() {
                PlatformPointerCaptureReleaseOutcome::Released
            } else {
                PlatformPointerCaptureReleaseOutcome::Rejected
            }
        }
    });
    cx.update(|cx| callbacks.cancel_capture(cx));
    cx.run_until_parked();
    assert_eq!(attempts.get(), 1);
    commit_capture_transport(&mut session, &mut bridge);
    assert!(bridge.take_reports().iter().all(|report| {
        !matches!(report, NativeReport::CaptureTerminal { barrier, .. }
            if barrier.drag_generation() == generation)
    }));
    let layout = DockspaceLayout::new([1, 2].map(|value| {
        DockspaceSurfaceLayout::new(
            SurfaceId::new(value),
            DockspaceRootLayout::new(
                RootId::new(value),
                DockspaceNode::tabs([ItemId::new(value)]),
            ),
        )
    }))
    .unwrap();
    let reset = session.prepare_layout_reset(layout).unwrap();
    let mut frame = session
        .begin_native_layout_reset_frame(&reset, |_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    let after = report
        .native_root_continuations()
        .iter()
        .find(|pair| pair.predecessor() == before)
        .expect("the original Root must continue on the same physical window")
        .successor();
    bridge.accept_report(report);
    bridge.take_reports();
    assert_eq!(callbacks.binding(), after.window());
    assert_eq!(
        bridge.binding_for_window(source.window_id()),
        Some(after.window())
    );
    assert!(transport.is_empty());
    assert!(!session.has_pending_native_input());
    ContinuedCapture {
        session,
        bridge,
        before,
        after,
        source,
        callbacks,
        events,
        generation,
        release_allowed,
        _subscription: subscription,
    }
}

#[open_gpui::test]
fn failed_shutdown_observes_original_capture_terminal_without_consuming_the_fifo(
    cx: &mut TestAppContext,
) {
    let mut capture = continued_capture(cx);
    let transport = capture.bridge.transport();
    let recorded = capture.session.native_input_recorded_count();
    transport.freeze_admission();
    cx.update(|cx| transport.device_removed(cx));
    let prefix = transport.inspection();
    assert_eq!(prefix.front_record, Some("device-removed"));
    cx.update(|cx| capture.bridge.drive_failed_shutdown(true, cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(cx.windows().contains(&capture.source)));
    assert!(!capture.bridge.failed_shutdown_complete());

    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    assert!(transport.0.queue.borrow().iter().any(|record| matches!(record,
        Inbound::CaptureTerminal(binding, barrier, NativeCapturedDragReleaseTerminal::Released, _)
            if *binding == capture.before.window()
                && barrier.drag_generation() == capture.generation
    )), "the original physical release proof must arrive before close");
    for _ in 0..4 {
        cx.update(|cx| capture.bridge.drive_failed_shutdown(true, cx))
            .unwrap();
        cx.run_until_parked();
    }
    assert!(capture.bridge.failed_shutdown_complete());
    assert!(
        !capture.bridge.is_quiescent(),
        "failed cleanup does not invent semantic drain"
    );
    assert_eq!(capture.session.native_input_recorded_count(), recorded);
    assert_eq!(transport.inspection().front_record, prefix.front_record);
    assert!(transport.inspection().queued_record_count > prefix.queued_record_count);
    assert!(
        transport
            .0
            .queue
            .borrow()
            .iter()
            .any(|record| matches!(record,
                Inbound::CaptureTerminal(_, barrier, NativeCapturedDragReleaseTerminal::Released, _)
                    if barrier.drag_generation() == capture.generation
            )),
        "the exact terminal stays owned by the old FIFO"
    );
}

#[open_gpui::test]
fn native_destruction_does_not_end_an_undelivered_capture_completion(cx: &mut TestAppContext) {
    let mut capture = continued_capture(cx);
    let transport = capture.bridge.transport();
    assert!(
        !transport.callbacks_idle(),
        "the original capture continuation is pending"
    );
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update({
        let transport = transport.clone();
        let observed = observed.clone();
        let source = capture.source.window_id();
        move |cx| {
            cx.on_window_native_terminal(move |_, owner| {
                if owner == source {
                    observed.borrow_mut().push(transport.callbacks_idle());
                }
            })
        }
    });
    cx.update_window(capture.source, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        *observed.borrow(),
        [false],
        "native destruction precedes the original capture handoff"
    );
    assert!(transport.callbacks_idle());
    assert!(transport.0.queue.borrow().iter().any(|record| matches!(record,
        Inbound::CaptureTerminal(_, barrier, NativeCapturedDragReleaseTerminal::NativeWindowTerminal, _)
            if barrier.drag_generation() == capture.generation
    )));
    for _ in 0..3 {
        cx.update(|cx| capture.bridge.drive_failed_shutdown(true, cx))
            .unwrap();
        cx.run_until_parked();
    }
    assert!(capture.bridge.failed_shutdown_complete());
}

#[open_gpui::test]
fn callback_wakes_remain_owned_until_the_original_handoff_finishes(cx: &mut TestAppContext) {
    let transport = super::NativeTransport::new(|_| {});
    let pending = cx.update(|cx| transport.wake_after_callback(cx));
    assert!(!transport.callbacks_idle());
    drop(pending);
    assert!(
        transport.callbacks_idle(),
        "an undispatched subscription releases its owner"
    );
    let pending = cx.update(|cx| transport.wake_after_callback(cx));
    pending();
    assert!(
        !transport.callbacks_idle(),
        "a detached foreground handoff still owns the generation"
    );
    cx.run_until_parked();
    assert!(transport.callbacks_idle());
    assert_eq!(transport.inspection().front_record, Some("wake"));
}

#[open_gpui::test]
fn a_failed_capture_release_keeps_shutdown_blocked_without_repeated_cancellation(
    cx: &mut TestAppContext,
) {
    let mut capture = continued_capture(cx);
    cx.update(|cx| capture.bridge.begin_shutdown(cx)).unwrap();
    for delay in [8, 32, 128] {
        cx.background_executor
            .advance_clock(Duration::from_millis(delay));
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
    }
    let reports = capture.bridge.take_reports();
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { barrier, terminal, .. }
                    if barrier.drag_generation() == capture.generation
                        && *terminal == NativeCapturedDragReleaseTerminal::Failed
            ))
            .count(),
        1,
        "the bounded original release must actually fail: {reports:?}"
    );
    for _ in 0..3 {
        cx.update(|cx| {
            capture
                .bridge
                .dispatch_effects(&mut |_, _, _| panic!("shutdown cannot create"), cx)
                .unwrap();
            capture.bridge.drive_shutdown(cx).unwrap();
        });
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        let repeated = capture.bridge.take_reports();
        assert!(
            repeated.is_empty(),
            "a failed original barrier must not restart cancellation: {repeated:?}"
        );
        assert!(capture.bridge.transport().is_empty());
    }
    cx.update(|cx| assert!(cx.windows().contains(&capture.source)));
    assert!(!capture.bridge.is_quiescent());
}

#[open_gpui::test]
fn shutdown_waits_for_the_original_capture_release_before_dispatching_close(
    cx: &mut TestAppContext,
) {
    let mut capture = continued_capture(cx);
    let source = capture.source;
    let anchor = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| *window != source)
            .unwrap()
    });
    let release_collected = Rc::new(Cell::new(false));
    let closed = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update({
        let release_collected = release_collected.clone();
        let closed = closed.clone();
        move |cx| {
            cx.on_window_closed(move |_, window| {
                closed.borrow_mut().push((window, release_collected.get()));
            })
        }
    });
    cx.update(|cx| capture.bridge.begin_shutdown(cx)).unwrap();
    cx.run_until_parked();
    assert!(
        closed.borrow().is_empty(),
        "a rejected original capture release must prevent logical close dispatch"
    );
    cx.update(|cx| {
        assert!(cx.windows().contains(&source));
        assert!(cx.windows().contains(&anchor));
    });
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    assert!(
        capture.bridge.transport().0.queue.borrow().iter().any(|record| {
            matches!(record,
                Inbound::CaptureTerminal(_, barrier, NativeCapturedDragReleaseTerminal::Released, _)
                    if barrier.source_window() == source.window_id()
                        && barrier.drag_generation() == capture.generation
            )
        }),
        "the real release callback must reach its original transport before collection"
    );
    cx.update(|cx| capture.bridge.drive_shutdown(cx)).unwrap();
    assert!(
        closed.borrow().is_empty(),
        "an uncollected release terminal must not authorize close dispatch"
    );
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let releases = capture.bridge.take_reports();
    assert_eq!(
        releases
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.before.window()
                        && barrier.source_window() == source.window_id()
                        && barrier.drag_generation() == capture.generation
                        && *terminal == NativeCapturedDragReleaseTerminal::Released
            ))
            .count(),
        1,
        "{releases:?}"
    );
    release_collected.set(true);
    for _ in 0..3 {
        cx.update(|cx| capture.bridge.drive_shutdown(cx)).unwrap();
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
    }
    assert_eq!(
        *closed.borrow(),
        vec![(source.window_id(), true), (anchor.window_id(), true)],
        "only the exact collected capture terminal can authorize dependent-first close"
    );
}

#[open_gpui::test]
fn continued_root_waits_for_the_original_capture_release_before_quiescence(
    cx: &mut TestAppContext,
) {
    let mut capture = continued_capture(cx);
    assert!(matches!(
        capture
            .bridge
            .quiesce_binding(&mut capture.session, capture.before),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(capture.session.recognizes_native_binding(capture.before));
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let reports = capture.bridge.take_reports();
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.before.window()
                        && barrier.source_window() == capture.source.window_id()
                        && barrier.drag_generation() == capture.generation
                        && *terminal == NativeCapturedDragReleaseTerminal::Released
            ))
            .count(),
        1,
        "{reports:?}"
    );
    capture
        .bridge
        .quiesce_binding(&mut capture.session, capture.before)
        .unwrap();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    assert!(!capture.session.recognizes_native_binding(capture.before));
    assert!(capture.session.is_current_native_binding(capture.after));
}

#[open_gpui::test]
fn continued_root_capture_release_preserves_the_next_real_drag(cx: &mut TestAppContext) {
    let mut capture = continued_capture(cx);
    let mut visual = VisualTestContext::from_window(capture.source, cx);
    visual.simulate_mouse_up(
        point(px(80.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_down(
        point(px(100.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(140.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(160.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let second_generation = capture.events.borrow().last().unwrap().generation();
    assert_ne!(capture.generation, second_generation);
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let mut reports = capture.bridge.take_reports();
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    reports.extend(capture.bridge.take_reports());
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.before.window()
                        && barrier.drag_generation() == capture.generation
                        && *terminal != NativeCapturedDragReleaseTerminal::Failed
            ))
            .count(),
        1,
        "{reports:?}"
    );
    cx.update(|cx| {
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(capture.source.window_id())
        );
        capture.callbacks.cancel_capture(cx);
        assert_eq!(cx.active_native_captured_drag_source_window(), None);
    });
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    assert!(
        capture
            .bridge
            .take_reports()
            .iter()
            .any(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.after.window()
                        && barrier.drag_generation() == second_generation
                        && *terminal != NativeCapturedDragReleaseTerminal::Failed
            ))
    );
}

#[open_gpui::test]
fn a_late_capture_release_cannot_settle_a_replaced_callback_owner(cx: &mut TestAppContext) {
    let mut capture = continued_capture(cx);
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    assert!(
        capture
            .bridge
            .transport()
            .0
            .queue
            .borrow()
            .iter()
            .any(|record| matches!(record, Inbound::CaptureTerminal(..)))
    );
    let original = capture
        .bridge
        .windows
        .remove(capture.after.window())
        .unwrap();
    let replacement = cx
        .update(|cx| {
            capture
                .bridge
                .bind_window(capture.after.window(), capture.source, false, cx)
        })
        .unwrap();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let reports = capture.bridge.take_reports();
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::StaleCallback(binding) if *binding == capture.before.window()
    )));
    assert!(reports.iter().all(|report| !matches!(report,
        NativeReport::CaptureTerminal { barrier, .. }
            if barrier.drag_generation() == capture.generation
    )));
    assert_eq!(replacement.binding(), capture.after.window());
    assert_eq!(original.window_id(), Some(capture.source.window_id()));
}

struct NaturalCapture {
    session: DockspaceSession,
    bridge: NativeBridge,
    bindings: [NativeSurfaceBinding; 2],
    windows: [AnyWindowHandle; 2],
    events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    starts: Rc<RefCell<Option<open_gpui::PreparedNativeCapturedDragConsumer>>>,
    _subscription: Subscription,
}

fn natural_capture(cx: &mut TestAppContext) -> NaturalCapture {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let source = windows[1];
    let callbacks = bridge.callbacks(bindings[1].window()).unwrap();
    let transport = bridge.transport();
    let events = Rc::new(RefCell::new(Vec::new()));
    let starts = Rc::new(RefCell::new(None));
    let subscription = cx.update({
        let release_callbacks = callbacks.clone();
        let events = events.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                move |event, _, cx| {
                    release_callbacks.observe_capture_release(event, cx);
                    Arc::new(())
                },
                move |event, cx| {
                    events.borrow_mut().push(event.clone());
                    transport.captured_drag(
                        callbacks.binding(),
                        NativePointerOwner::Unknown,
                        event,
                        NativePointerId::new(1),
                        PointerEventMetadata::new(
                            PointerDeviceKind::Mouse,
                            Duration::from_millis(37),
                        ),
                        cx,
                    );
                },
            )
        }
    });
    cx.update_window(source, |_, window, cx| {
        window.replace_root(cx, |_, _| ObservedStartView(starts.clone()));
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        commit_capture_transport(&mut session, &mut bridge);
    }
    assert!(transport.is_empty());
    bridge.take_reports();
    NaturalCapture {
        session,
        bridge,
        bindings,
        windows,
        events,
        starts,
        _subscription: subscription,
    }
}

#[open_gpui::test]
fn two_natural_capture_releases_survive_collection_after_both_drag_generations(
    cx: &mut TestAppContext,
) {
    let mut capture = natural_capture(cx);
    let source = capture.windows[1];
    let binding = capture.bindings[1].window();
    let transport = capture.bridge.transport();
    let attempts = Rc::new(RefCell::new(Vec::new()));
    cx.set_pointer_capture_release_callback(source, {
        let attempts = attempts.clone();
        move |generation| {
            attempts.borrow_mut().push(generation);
            PlatformPointerCaptureReleaseOutcome::Released
        }
    });
    let recorded = capture.session.native_input_recorded_count();
    let update = capture
        .session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap()
        .suspend();
    let mut visual = VisualTestContext::from_window(source, cx);
    for (index, x) in [20.0, 100.0].into_iter().enumerate() {
        visual.simulate_mouse_down(point(px(x), px(20.0)), MouseButton::Left, Modifiers::none());
        visual.simulate_mouse_move(
            point(px(x + 40.0), px(20.0)),
            Some(MouseButton::Left),
            Modifiers::none(),
        );
        visual.simulate_mouse_move(
            point(px(x + 60.0), px(20.0)),
            Some(MouseButton::Left),
            Modifiers::none(),
        );
        if index == 1 {
            let callbacks = capture.bridge.callbacks(binding).unwrap();
            callbacks.retain_capture_start(
                source.window_id(),
                capture.starts.borrow().as_ref().unwrap(),
            );
            cx.update(|cx| capture.bridge.begin_shutdown(cx)).unwrap();
        }
        visual.simulate_mouse_up(
            point(px(x + 60.0), px(20.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
    }
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.active_native_captured_drag_source_window(), None));
    let released: Vec<_> = capture
        .events
        .borrow()
        .iter()
        .filter(|event| event.phase() == NativeCapturedDragPhase::Released)
        .cloned()
        .collect();
    assert_eq!(released.len(), 2);
    assert_ne!(released[0].generation(), released[1].generation());
    assert!(released[0].sequence() < released[1].sequence());
    assert!(
        capture
            .events
            .borrow()
            .iter()
            .all(|event| !matches!(event.phase(), NativeCapturedDragPhase::Cancelled(_)))
    );
    assert_eq!(attempts.borrow().len(), 2);
    let original_terminals: Vec<_> = transport
        .0
        .queue
        .borrow()
        .iter()
        .filter_map(|record| match record {
            Inbound::CaptureTerminal(binding, barrier, terminal, _) => {
                Some((*binding, *barrier, *terminal))
            }
            _ => None,
        })
        .collect();
    assert_eq!(original_terminals.len(), 2);
    for (index, (owner, barrier, terminal)) in original_terminals.iter().enumerate() {
        assert_eq!(*owner, binding);
        assert_eq!(barrier.source_window(), source.window_id());
        assert_eq!(barrier.drag_generation(), released[index].generation());
        assert_eq!(barrier.release_generation(), attempts.borrow()[index]);
        assert_eq!(*terminal, NativeCapturedDragReleaseTerminal::Released);
    }
    assert_ne!(original_terminals[0].1, original_terminals[1].1);
    assert!(
        transport
            .0
            .queue
            .borrow()
            .iter()
            .all(|record| !matches!(record, Inbound::CaptureUnobserved(_)))
    );
    assert_eq!(capture.session.native_input_recorded_count(), recorded);

    drop(update);
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    assert!(transport.is_empty());
    let mut reports = capture.bridge.take_reports();
    let release_packets: Vec<_> = reports
        .iter()
        .filter_map(|report| {
            if let NativeReport::CapturedRelease { capture, frame } = report {
                Some((*capture, *frame))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(release_packets.len(), 2);
    for (packet, event) in release_packets.iter().zip(&released) {
        assert_eq!(packet.0, super::super::NativeCapture::from_event(event));
        assert_eq!(packet.1, event.physical_frame());
    }
    let collected_terminals: Vec<_> = reports
        .iter()
        .filter_map(|report| match report {
            NativeReport::CaptureTerminal {
                binding,
                barrier,
                terminal,
            } => Some((*binding, *barrier, *terminal)),
            _ => None,
        })
        .collect();
    assert_eq!(collected_terminals, original_terminals);
    cx.update(|cx| capture.bridge.begin_shutdown(cx)).unwrap();
    for _ in 0..4 {
        cx.update(|cx| capture.bridge.drive_shutdown(cx)).unwrap();
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        reports.extend(capture.bridge.take_reports());
    }
    cx.update(|cx| {
        assert!(
            capture
                .windows
                .iter()
                .all(|window| !cx.windows().contains(window))
        );
    });
    for binding in capture.bindings {
        capture
            .bridge
            .callbacks(binding.window())
            .unwrap()
            .seal()
            .unwrap();
        capture
            .bridge
            .quiesce_binding(&mut capture.session, binding)
            .unwrap();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        reports.extend(capture.bridge.take_reports());
    }
    assert!(capture.bridge.is_quiescent());
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report, NativeReport::CaptureTerminal { .. }))
            .count(),
        2,
        "service must not duplicate either original natural terminal: {reports:?}"
    );
    assert!(!reports.iter().any(|report| matches!(report,
        NativeReport::Unsupported { operation }
            if *operation == "observing the original native capture release after drag retirement"
    )), "{reports:?}");
}

#[open_gpui::test]
fn natural_capture_release_failure_stays_latched_until_the_actual_native_terminal(
    cx: &mut TestAppContext,
) {
    let mut capture = natural_capture(cx);
    let source = capture.windows[1];
    let binding = capture.bindings[1].window();
    let callbacks = capture.bridge.callbacks(binding).unwrap();
    let attempts = Rc::new(RefCell::new(Vec::new()));
    let release_allowed = Rc::new(Cell::new(false));
    cx.set_pointer_capture_release_callback(source, {
        let attempts = attempts.clone();
        let release_allowed = release_allowed.clone();
        move |generation| {
            attempts.borrow_mut().push(generation);
            if release_allowed.get() {
                PlatformPointerCaptureReleaseOutcome::Released
            } else {
                PlatformPointerCaptureReleaseOutcome::Rejected
            }
        }
    });
    let mut visual = VisualTestContext::from_window(source, cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let generation = capture.events.borrow().last().unwrap().generation();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    capture.bridge.take_reports();
    visual.simulate_mouse_up(
        point(px(80.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.active_native_captured_drag_source_window(), None));
    assert_eq!(attempts.borrow().len(), 1);
    let release_generation = attempts.borrow()[0];
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let mut reports = capture.bridge.take_reports();
    assert!(
        !reports
            .iter()
            .any(|report| matches!(report, NativeReport::CaptureTerminal { .. }))
    );
    for (index, delay) in [8, 32, 128].into_iter().enumerate() {
        cx.background_executor
            .advance_clock(Duration::from_millis(delay));
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        reports.extend(capture.bridge.take_reports());
        assert_eq!(attempts.borrow().len(), index + 2);
    }
    assert_eq!(attempts.borrow().as_slice(), &[release_generation; 4]);
    let terminals: Vec<_> = reports
        .iter()
        .filter_map(|report| match report {
            NativeReport::CaptureTerminal {
                binding,
                barrier,
                terminal,
            } => Some((*binding, *barrier, *terminal)),
            _ => None,
        })
        .collect();
    assert_eq!(terminals.len(), 1, "{reports:?}");
    let (owner, barrier, terminal) = terminals[0];
    assert_eq!(owner, binding);
    assert_eq!(barrier.source_window(), source.window_id());
    assert_eq!(barrier.drag_generation(), generation);
    assert_eq!(barrier.release_generation(), release_generation);
    assert_eq!(terminal, NativeCapturedDragReleaseTerminal::Failed);
    let events = capture.events.borrow();
    assert_eq!(
        events.last().unwrap().phase(),
        NativeCapturedDragPhase::Released
    );
    assert!(events.iter().all(|event| event.generation() == generation
        && !matches!(event.phase(), NativeCapturedDragPhase::Cancelled(_))));
    drop(events);

    release_allowed.set(true);
    cx.update(|cx| capture.bridge.begin_shutdown(cx)).unwrap();
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    reports.extend(capture.bridge.take_reports());
    for _ in 0..3 {
        cx.update(|cx| {
            callbacks.cancel_capture(cx);
            capture
                .bridge
                .dispatch_effects(&mut |_, _, _| panic!("shutdown cannot create"), cx)
                .unwrap();
            capture.bridge.drive_shutdown(cx).unwrap();
        });
        cx.background_executor.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        let repeated = capture.bridge.take_reports();
        assert!(
            repeated.is_empty(),
            "a failed natural release must remain retired: {repeated:?}"
        );
        assert_eq!(attempts.borrow().as_slice(), &[release_generation; 4]);
        assert!(capture.bridge.transport().is_empty());
    }
    cx.update(|cx| {
        assert!(
            capture
                .windows
                .iter()
                .all(|window| cx.windows().contains(window))
        );
    });
    assert!(!capture.bridge.is_quiescent());

    let held = cx.hold_window_native_terminal(source);
    cx.update_window(source, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let logical = capture.bridge.take_reports();
    assert!(
        logical.iter().any(|report| matches!(report,
            NativeReport::LogicalClosed(owner) if *owner == binding
        )),
        "{logical:?}"
    );
    assert!(
        !logical.iter().any(|report| matches!(report,
            NativeReport::NativeTerminal(owner) if *owner == binding
        )),
        "{logical:?}"
    );
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));
    reports.extend(logical);
    cx.update(|cx| capture.bridge.drive_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(cx.windows().contains(&capture.windows[0])));

    assert!(held.release());
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let native = capture.bridge.take_reports();
    assert_eq!(
        native
            .iter()
            .filter(|report| matches!(report,
                NativeReport::NativeTerminal(owner) if *owner == binding
            ))
            .count(),
        1,
        "{native:?}"
    );
    assert!(
        !native
            .iter()
            .any(|report| matches!(report, NativeReport::CaptureTerminal { .. })),
        "native retirement must not rewrite the failed terminal: {native:?}"
    );
    callbacks
        .seal()
        .expect("only the actual native terminal clears capture failure custody");
    reports.extend(native);
    for _ in 0..3 {
        cx.update(|cx| capture.bridge.drive_shutdown(cx)).unwrap();
        cx.run_until_parked();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        reports.extend(capture.bridge.take_reports());
    }
    for binding in capture.bindings {
        capture
            .bridge
            .callbacks(binding.window())
            .unwrap()
            .seal()
            .unwrap();
        capture
            .bridge
            .quiesce_binding(&mut capture.session, binding)
            .unwrap();
        commit_capture_transport(&mut capture.session, &mut capture.bridge);
        reports.extend(capture.bridge.take_reports());
    }
    assert!(capture.bridge.is_quiescent());
    assert_eq!(attempts.borrow().as_slice(), &[release_generation; 4]);
    let all_terminals: Vec<_> = reports
        .iter()
        .filter_map(|report| match report {
            NativeReport::CaptureTerminal {
                binding,
                barrier,
                terminal,
            } => Some((*binding, *barrier, *terminal)),
            _ => None,
        })
        .collect();
    assert_eq!(all_terminals, terminals);
    assert!(!reports.iter().any(|report| matches!(report,
        NativeReport::Unsupported { operation }
            if *operation == "observing the original native capture release after drag retirement"
    )), "{reports:?}");
}

fn uncollected_focus_transport(
    cx: &mut open_gpui::App,
) -> (
    DockspaceSession,
    NativeBridge,
    NativeSurfaceBinding,
    AnyWindowHandle,
) {
    use dockspace::{
        policy::DockPolicy,
        runtime::{HostWindowToken, NativeHostCapabilities, NativeHostCapability},
    };
    use open_gpui::{AppContext as _, WindowOptions};

    let surface = SurfaceId::new(1);
    let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
        surface,
        DockspaceRootLayout::new(RootId::new(1), DockspaceNode::tabs([ItemId::new(1)])),
    )])
    .unwrap();
    let mut session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
    let capabilities = NativeHostCapabilities::none_supported()
        .with(NativeHostCapability::AuthoritativeInventory)
        .with(NativeHostCapability::GlobalFocusObservation);
    let mut bridge = NativeBridge::enable(
        &mut session,
        super::NativeTransport::new(|_| {}),
        capabilities,
        None,
    )
    .unwrap();
    let window: AnyWindowHandle = cx
        .open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
        .unwrap()
        .into();
    session
        .register_native_root(surface, HostWindowToken::new(window.window_id().as_u64()))
        .unwrap();
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    let binding = report.native_bindings()[0];
    bridge.accept_report(report);
    bridge
        .bind_window(binding.window(), window, false, cx)
        .unwrap();
    window
        .update(cx, |_, window, _| {
            let _ = window.activate_window();
        })
        .unwrap();
    (session, bridge, binding, window)
}

#[open_gpui::test]
fn initial_focus_follows_the_first_window_inventory(cx: &mut TestAppContext) {
    use dockspace::runtime::HostInputOutcome;

    let (mut session, mut bridge, binding, _) = cx.update(uncollected_focus_transport);
    cx.run_until_parked();
    let transport = bridge.transport();
    let (queued, focus_count) = {
        let queue = transport.0.queue.borrow();
        let inventory = queue
            .iter()
            .position(|record| matches!(record, Inbound::Observed(owner, _) if *owner == binding.window()))
            .expect("installation must publish the first window inventory");
        let focus = queue
            .iter()
            .position(|record| matches!(record, Inbound::FocusedWindow(_)))
            .expect("installation must sample initial focus with the capability enabled");
        assert!(
            inventory < focus,
            "initial focus cannot precede its inventory"
        );
        let focus_count = queue
            .iter()
            .filter(|record| matches!(record, Inbound::FocusedWindow(_)))
            .count();
        (queue.len(), focus_count)
    };
    let mut collected = 0;
    let mut inventory_committed = false;
    let mut focus_committed = 0;
    while !transport.is_empty() {
        let count = bridge
            .collect(&mut session)
            .expect("initial focus must record");
        assert!(
            count > 0,
            "committing each prefix must release the next observation"
        );
        collected += count;
        let mut frame = session
            .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
            .unwrap();
        frame
            .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
            .unwrap();
        let report = frame.commit().unwrap();
        for input in report.inputs() {
            match input {
                HostInputOutcome::NativePlatformSnapshotApplied { .. } => {
                    inventory_committed = true
                }
                HostInputOutcome::NativeFocusObservationApplied => {
                    assert!(
                        inventory_committed,
                        "initial focus cannot precede its committed inventory"
                    );
                    focus_committed += 1;
                }
                _ => {}
            }
        }
        bridge.accept_report(report);
    }
    assert!(inventory_committed);
    assert_eq!(collected, queued);
    assert_eq!(
        focus_committed, focus_count,
        "initial focus must reach the core instead of being silently skipped"
    );
    assert!(session.is_current_native_binding(binding));
    assert!(!session.has_pending_native_input());
    assert!(transport.is_empty());
    assert!(transport.inspection().rejection.is_none());
}

#[open_gpui::test]
fn delayed_platform_focus_revalidates_the_committed_visibility_prefix(cx: &mut TestAppContext) {
    use dockspace::runtime::{HostInputOutcome, NativeWindowFacts, NativeWindowPresentationState};
    use open_gpui::PlatformFocusedWindow;

    for presentation in [
        Some(NativeWindowPresentationState::Hidden),
        Some(NativeWindowPresentationState::Minimized),
        None,
    ] {
        let (mut session, mut bridge, binding, window) = cx.update(uncollected_focus_transport);
        cx.run_until_parked();
        let transport = bridge.transport();
        while !transport.is_empty() {
            commit_capture_transport(&mut session, &mut bridge);
        }
        assert!(session.native_binding_can_report_focus(binding));
        let unavailable = presentation.map_or_else(NativeWindowFacts::live, |state| {
            NativeWindowFacts::live().with_presentation(state)
        });
        // A native focus hint names a real retained Window, but the preceding
        // complete snapshot revokes its authority before that hint is reduced.
        // The original FIFO must commit that prefix before mapping the hint.
        let visible =
            NativeWindowFacts::live().with_presentation(NativeWindowPresentationState::Visible);
        for (facts, reportable) in [(unavailable, false), (visible, true)] {
            cx.update(|cx| {
                transport.push(Inbound::Observed(binding.window(), facts), cx);
                transport.focused_window(PlatformFocusedWindow::Window(window), cx);
            });
            let mut focus_committed = 0;
            while !transport.is_empty() {
                assert!(bridge.collect(&mut session).unwrap() > 0);
                let mut frame = session
                    .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
                    .unwrap();
                frame
                    .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
                    .unwrap();
                let report = frame.commit().unwrap();
                focus_committed += report
                    .inputs()
                    .iter()
                    .filter(|input| {
                        matches!(input, HostInputOutcome::NativeFocusObservationApplied)
                    })
                    .count();
                bridge.accept_report(report);
            }
            assert_eq!(
                focus_committed, 1,
                "the delayed focus must commit exactly once"
            );
            assert_eq!(session.native_binding_can_report_focus(binding), reportable);
            assert!(transport.inspection().rejection.is_none());
            assert!(!session.has_pending_native_input());
        }
    }
}

#[open_gpui::test]
fn retired_focus_does_not_block_the_original_native_terminal_suffix(cx: &mut TestAppContext) {
    use dockspace::runtime::{HostInputOutcome, NativeGlobalFocus, NativeHostErrorKind};
    use open_gpui::PlatformFocusedWindow;

    let (mut session, mut bridge, binding, window) = cx.update(uncollected_focus_transport);
    cx.run_until_parked();
    let transport = bridge.transport();
    while !transport.is_empty() {
        commit_capture_transport(&mut session, &mut bridge);
    }
    assert!(transport.is_empty());
    let observed_focus = cx.update(|cx| cx.focused_window());
    assert_eq!(observed_focus, PlatformFocusedWindow::Window(window));
    assert!(session.is_current_native_binding(binding));
    let callbacks = bridge.callbacks(binding.window()).unwrap();

    // Removing the surface retires its whole native window, unlike a same-surface
    // root continuation, which installs a current successor on the same handle.
    let reset = session
        .prepare_layout_reset(DockspaceLayout::empty())
        .unwrap();
    let mut frame = session
        .begin_native_layout_reset_frame(&reset, |_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    assert!(report.native_root_continuations().is_empty());
    bridge.accept_report(report);
    bridge.take_reports();
    assert!(!session.is_current_native_binding(binding));
    assert!(session.recognizes_native_window_binding(binding.window()));
    let current: Vec<_> = session.native_window_bindings().collect();
    assert!(
        current.is_empty(),
        "the removed window must be retired: {current:?}"
    );
    assert_eq!(
        bridge.binding_for_window(window.window_id()),
        Some(binding.window())
    );
    let recorded = session.native_input_recorded_count();
    let error = session
        .report_native_global_focus(NativeGlobalFocus::Dock(binding.window()))
        .expect_err("the real retired binding must reject its old Dock focus");
    assert_eq!(error.native_kind(), Some(NativeHostErrorKind::StaleBinding));
    assert_eq!(session.native_input_recorded_count(), recorded);

    cx.update(|cx| transport.focused_window(observed_focus, cx));
    let held = cx.hold_window_native_terminal(window);
    cx.update_window(window, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(held.release());
    cx.run_until_parked();
    let queued = {
        let queue = transport.0.queue.borrow();
        assert!(
            matches!(queue.front(), Some(Inbound::FocusedWindow(focus)) if *focus == observed_focus)
        );
        assert!(
            queue.iter().any(|record| matches!(record,
                Inbound::NativeTerminal(owner, _) if *owner == binding.window()
            )),
            "the real terminal must be queued behind the delayed focus"
        );
        queue.len()
    };
    assert_eq!(
        bridge
            .collect(&mut session)
            .expect("retired focus must not retain the FIFO head ahead of native terminal"),
        queued,
    );
    assert!(transport.is_empty());
    assert!(transport.inspection().rejection.is_none());
    let reports = bridge.take_reports();
    let logical = reports
        .iter()
        .position(|report| matches!(report, NativeReport::LogicalClosed(owner) if *owner == binding.window()))
        .expect("the original logical close must be consumed");
    let terminal = reports
        .iter()
        .position(|report| matches!(report, NativeReport::NativeTerminal(owner) if *owner == binding.window()))
        .expect("the original native terminal must be consumed");
    assert!(logical < terminal);
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    let focus = report
        .inputs()
        .iter()
        .position(|input| matches!(input, HostInputOutcome::NativeFocusObservationApplied))
        .expect("the retired focus observation must commit");
    let terminal = report
        .inputs()
        .iter()
        .position(|input| {
            matches!(
                input,
                HostInputOutcome::NativePlatformSnapshotApplied { .. }
            )
        })
        .expect("the original terminal snapshot must commit");
    assert!(focus < terminal);
    bridge.accept_report(report);
    assert!(!session.has_pending_native_input());
    callbacks
        .seal()
        .expect("the real terminal must settle callback custody");
}
