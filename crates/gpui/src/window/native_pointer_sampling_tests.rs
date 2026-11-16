use crate::{
    AppContext as _, Bounds, DevicePixels, DispatchEventResult, Empty, Modifiers, MouseMoveEvent,
    PlatformInput, PlatformInputCallback, PlatformNativePointerPhysicalFrame, PlatformWindow as _,
    PlatformWindowPhysicalGeometry, TestAppContext, point, px, size,
};
use std::{cell::Cell, rc::Rc};

#[crate::test]
fn unsupported_fresh_sampling_preserves_the_active_callback_frame(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let mut native = cx.test_window(handle.into());
    let frame = PlatformNativePointerPhysicalFrame::new(
        point(DevicePixels(130), DevicePixels(260)),
        PlatformWindowPhysicalGeometry::try_new(
            Bounds::new(
                point(DevicePixels(100), DevicePixels(200)),
                size(DevicePixels(800), DevicePixels(600)),
            ),
            1.5,
        )
        .unwrap(),
    );
    let calls = Rc::new(Cell::new(0));
    native.on_input(PlatformInputCallback::new_unleased_for_test(Box::new({
        let mut app = cx.to_async();
        let calls = calls.clone();
        move |_| {
            app.update_window(handle.into(), |_, window, _| {
                assert!(window.creation_can_commit());
                assert!(window.user_interaction_is_admitted());
                assert_eq!(
                    window.platform_window.native_pointer_physical_frame(),
                    Some(frame)
                );
                assert_eq!(
                    window
                        .platform_window
                        .sample_native_pointer_physical_frame(),
                    None
                );
                assert_eq!(window.sample_native_pointer_physical_frame(), None);
                assert_eq!(
                    window.platform_window.native_pointer_physical_frame(),
                    Some(frame)
                );
                calls.set(calls.get() + 1);
            })
            .unwrap();
            DispatchEventResult::default()
        }
    })));

    native.simulate_input_result_with_native_pointer_physical_frame(
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(20.), px(40.)),
            pressed_button: None,
            modifiers: Modifiers::none(),
        }),
        Some(frame),
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(native.native_pointer_physical_frame(), None);
    assert_eq!(native.sample_native_pointer_physical_frame(), None);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.sample_native_pointer_physical_frame(), None);
    })
    .unwrap();
}
