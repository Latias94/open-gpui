use crate::{
    AnyDrag, AppContext as _, Bounds, Context, DevicePixels, DispatchEventResult, Empty,
    InteractiveElement as _, IntoElement, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NativeCapturedDragGeneration, NativeCapturedDragPhase, PlatformInput,
    PlatformInputCallback, PlatformNativePointerCaptureOwner, PlatformNativePointerPhysicalFrame,
    PlatformWindow as _, PlatformWindowHit, PlatformWindowHitStack, PlatformWindowPhysicalCoverage,
    PlatformWindowPhysicalGeometry, PointerCaptureHandle, Render, Styled as _, TestAppContext,
    Window, WindowHandle, div, point, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

struct DragSource {
    capture: PointerCaptureHandle,
}

impl Render for DragSource {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().track_pointer_capture(&self.capture)
    }
}

struct CapturedDrag {
    source: WindowHandle<DragSource>,
    generation: NativeCapturedDragGeneration,
    frame: PlatformNativePointerPhysicalFrame,
    stack: PlatformWindowHitStack,
}

fn start_drag(
    cx: &mut TestAppContext,
    source: WindowHandle<DragSource>,
) -> NativeCapturedDragGeneration {
    cx.test_window(source.into())
        .simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        }));
    source
        .update(cx, |view, window, app| {
            window
                .capture_pointer(&view.capture, MouseButton::Left)
                .unwrap();
            let reservation = app.reserve_native_captured_drag_start();
            let prepared = reservation.token().prepare_consumer();
            let preview = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: Some(view.capture),
                    view: preview,
                    value: Arc::new("stationary-drag"),
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                    external_payload_source: None,
                },
            ));
            assert!(prepared.is_active());
            prepared.generation()
        })
        .unwrap()
}

fn captured_drag(cx: &mut TestAppContext) -> CapturedDrag {
    let source = cx.open_window(size(px(320.0), px(200.0)), |window, _| DragSource {
        capture: window.new_pointer_capture_handle(),
    });
    cx.update_window(source.into(), |_, window, app| {
        let _ = window.activate_window();
        window.draw(app).clear(app);
    })
    .unwrap();
    cx.run_until_parked();
    // Keep redraw callbacks from consuming ingress sequences between the explicit inputs.
    cx.test_window(source.into())
        .defer_frame_requests_for_test();
    let generation = start_drag(cx, source);
    let bounds = Bounds::new(
        point(DevicePixels(100), DevicePixels(200)),
        size(DevicePixels(800), DevicePixels(600)),
    );
    let geometry = PlatformWindowPhysicalGeometry::try_new(bounds, 1.5).unwrap();
    let frame = PlatformNativePointerPhysicalFrame::new(
        point(DevicePixels(130), DevicePixels(260)),
        geometry,
    )
    .with_capture_owner(PlatformNativePointerCaptureOwner::Window(
        source.window_id(),
    ));
    let stack = PlatformWindowHitStack::try_available(
        frame.global_position(),
        vec![PlatformWindowHit::RegisteredApplication {
            window: source.into(),
            coverage: PlatformWindowPhysicalCoverage::try_new(bounds).unwrap(),
            geometry,
        }],
    )
    .unwrap();
    cx.set_platform_window_hit_stack(stack.clone());
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    cx.run_until_parked();
    CapturedDrag {
        source,
        generation,
        frame,
        stack,
    }
}

#[crate::test]
fn stationary_sample_uses_fresh_frames_without_emitting_input_or_reserving_ingress(
    cx: &mut TestAppContext,
) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update({
        let events = events.clone();
        move |app| {
            app.observe_native_captured_drag(move |event, _| {
                events
                    .borrow_mut()
                    .push((event.sequence(), event.generation(), event.phase()));
            })
        }
    });
    let cached = PlatformNativePointerPhysicalFrame::new(
        point(DevicePixels(115), DevicePixels(230)),
        frame.source_geometry(),
    )
    .with_capture_owner(frame.capture_owner());
    let mut native = cx.test_window(source.into());
    let movement = PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(10.0), px(20.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    native.simulate_input_result_with_native_pointer_physical_frame(movement.clone(), Some(cached));
    cx.run_until_parked();
    let before = events.borrow().clone();
    assert_eq!(before.len(), 1);
    assert_eq!(
        (before[0].1, before[0].2),
        (generation, NativeCapturedDragPhase::Moved)
    );

    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        None
    );
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        Some((frame, stack)),
    );
    assert_eq!(native.native_pointer_physical_frame(), None);
    cx.run_until_parked();
    assert_eq!(
        *events.borrow(),
        before,
        "sampling must not deliver a native drag event"
    );

    native.simulate_input_result_with_native_pointer_physical_frame(movement, Some(cached));
    cx.run_until_parked();
    let after = events.borrow();
    assert_eq!(after.len(), 2);
    assert_eq!(
        (after[1].1, after[1].2),
        (generation, NativeCapturedDragPhase::Moved)
    );
    assert_eq!(after[1].0.ordinal(), before[0].0.ordinal() + 1);
}

#[crate::test]
fn stationary_sample_rejects_missing_or_changed_physical_frames(cx: &mut TestAppContext) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let moved = PlatformNativePointerPhysicalFrame::new(
        point(DevicePixels(131), DevicePixels(260)),
        frame.source_geometry(),
    )
    .with_capture_owner(frame.capture_owner());
    let geometry = PlatformWindowPhysicalGeometry::try_new(
        Bounds::new(
            point(DevicePixels(101), DevicePixels(200)),
            size(DevicePixels(800), DevicePixels(600)),
        ),
        1.5,
    )
    .unwrap();
    let repositioned = PlatformNativePointerPhysicalFrame::new(frame.global_position(), geometry)
        .with_capture_owner(frame.capture_owner());
    let rescaled = PlatformNativePointerPhysicalFrame::new(
        frame.global_position(),
        PlatformWindowPhysicalGeometry::try_new(frame.source_geometry().client_bounds(), 2.0)
            .unwrap(),
    )
    .with_capture_owner(frame.capture_owner());

    for (reason, samples) in [
        ("missing first frame", [None, Some(frame)]),
        ("missing second frame", [Some(frame), None]),
        ("pointer moved", [Some(frame), Some(moved)]),
        ("source moved", [Some(frame), Some(repositioned)]),
        ("source scale changed", [Some(frame), Some(rescaled)]),
    ] {
        cx.set_platform_window_native_pointer_physical_samples(source.into(), samples);
        assert_eq!(
            cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
            None,
            "{reason}",
        );
    }
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        Some((frame, stack)),
    );
}

#[crate::test]
fn stationary_sample_requires_source_native_capture_before_and_after_hit_testing(
    cx: &mut TestAppContext,
) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let other = cx.add_window(|_, _| Empty);
    for capture in [
        PlatformNativePointerCaptureOwner::Unavailable,
        PlatformNativePointerCaptureOwner::None,
        PlatformNativePointerCaptureOwner::UnregisteredWindow,
        PlatformNativePointerCaptureOwner::Window(other.window_id()),
    ] {
        let invalid = frame.with_capture_owner(capture);
        for samples in [[Some(invalid); 2], [Some(frame), Some(invalid)]] {
            cx.set_platform_window_native_pointer_physical_samples(source.into(), samples);
            assert_eq!(
                cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
                None,
                "native capture {capture:?} must not be replaced by logical capture",
            );
        }
    }
    source
        .update(cx, |view, window, _| {
            assert_eq!(window.captured_pointer().unwrap().handle(), view.capture);
        })
        .unwrap();
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        Some((frame, stack)),
    );
}

#[crate::test]
fn stationary_sample_requires_pressed_button_before_and_after_hit_testing(cx: &mut TestAppContext) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let native = cx.test_window(source.into());
    for pressed in [Some(false), None] {
        cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
        cx.set_platform_mouse_button_is_pressed(MouseButton::Left, pressed);
        assert_eq!(
            cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
            None
        );
        cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
        assert_eq!(
            cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
            Some((frame, stack.clone())),
            "the initial button guard must leave fresh samples unconsumed",
        );

        cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
        native.set_mouse_button_after_next_pointer_sample(MouseButton::Left, pressed);
        assert_eq!(
            cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
            None,
            "the button must remain verifiably pressed after the physical samples",
        );
        cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    }
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        Some((frame, stack)),
    );
}

#[crate::test]
fn stationary_sample_requires_a_hit_stack_for_the_fresh_physical_point(cx: &mut TestAppContext) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let wrong_point = PlatformWindowHitStack::try_available_open_desktop(
        point(DevicePixels(131), DevicePixels(260)),
        Vec::new(),
    )
    .unwrap();
    for invalid in [PlatformWindowHitStack::Unavailable, wrong_point] {
        cx.set_platform_window_hit_stack(invalid);
        cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
        assert_eq!(
            cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
            None
        );
    }
    cx.set_platform_window_hit_stack(stack.clone());
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        Some((frame, stack)),
    );
}

#[crate::test]
fn stationary_sample_rejects_released_and_replaced_generations(cx: &mut TestAppContext) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let mut native = cx.test_window(source.into());
    native.simulate_input_result_with_native_pointer_physical_frame(
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
        }),
        Some(frame),
    );
    cx.run_until_parked();
    assert!(cx.read(|app| app.active_drag.is_none()));
    // Keep physical facts valid: only the released framework generation should reject this sample.
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        None
    );

    let replacement = start_drag(cx, source);
    assert_ne!(replacement, generation);
    cx.run_until_parked();
    cx.set_platform_window_native_pointer_physical_samples(source.into(), [Some(frame); 2]);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), generation)),
        None
    );
    let other = cx.add_window(|_, _| Empty);
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(other.window_id(), replacement)),
        None
    );
    assert_eq!(
        cx.read(|app| app.sample_native_captured_drag(source.window_id(), replacement)),
        Some((frame, stack)),
        "stale requests must not consume the replacement generation's samples",
    );
}

#[crate::test]
fn stationary_sampling_preserves_the_callback_scoped_frame(cx: &mut TestAppContext) {
    let CapturedDrag {
        source,
        generation,
        frame,
        stack,
    } = captured_drag(cx);
    let cached = PlatformNativePointerPhysicalFrame::new(
        point(DevicePixels(115), DevicePixels(230)),
        frame.source_geometry(),
    )
    .with_capture_owner(frame.capture_owner());
    cx.set_platform_window_native_pointer_physical_samples(
        source.into(),
        [None, Some(frame), Some(frame)],
    );
    let samples = Rc::new(Cell::new(0));
    let mut native = cx.test_window(source.into());
    native.on_input(PlatformInputCallback::new_unleased_for_test(Box::new({
        let app = cx.to_async();
        let samples = samples.clone();
        move |_| {
            app.update(|app| {
                let source_window = app
                    .windows
                    .get(source.window_id())
                    .unwrap()
                    .as_ref()
                    .unwrap();
                assert_eq!(
                    source_window
                        .platform_window
                        .native_pointer_physical_frame(),
                    Some(cached)
                );
                assert_eq!(
                    app.sample_native_captured_drag(source.window_id(), generation),
                    None
                );
                assert_eq!(
                    app.sample_native_captured_drag(source.window_id(), generation),
                    Some((frame, stack.clone())),
                );
                assert_eq!(
                    app.sample_native_captured_drag(source.window_id(), generation),
                    None
                );
                assert_eq!(
                    source_window
                        .platform_window
                        .native_pointer_physical_frame(),
                    Some(cached)
                );
                samples.set(samples.get() + 1);
            });
            DispatchEventResult::default()
        }
    })));
    native.simulate_input_result_with_native_pointer_physical_frame(
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(10.0), px(20.0)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
        Some(cached),
    );
    assert_eq!(samples.get(), 1);
    assert_eq!(native.native_pointer_physical_frame(), None);
    assert_eq!(native.sample_native_pointer_physical_frame(), None);
}
