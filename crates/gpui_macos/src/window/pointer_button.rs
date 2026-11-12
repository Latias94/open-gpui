use crate::events::platform_input_from_native;
use objc2_app_kit::NSEvent;
use open_gpui::{MouseButton, Pixels, PlatformInput};

#[derive(Default)]
pub(super) struct MacPointerButton {
    // Ctrl-left remains a right click until this physical left-button press ends.
    left_button: Option<MouseButton>,
    // A prepared native release must not cancel a subsequent press.
    generation: u64,
}

pub(super) struct MacPointerInput {
    pub(super) event: PlatformInput,
    pub(super) physical_pressed_button: Option<MouseButton>,
}

impl MacPointerButton {
    pub(super) fn input(
        &mut self,
        native_event: &NSEvent,
        window_height: Pixels,
        first_mouse: &mut bool,
    ) -> Option<MacPointerInput> {
        let mut event = platform_input_from_native(native_event, Some(window_height))?;
        let physical_pressed_button = match &event {
            PlatformInput::MouseMove(event) => event.pressed_button,
            _ => None,
        };
        match &mut event {
            PlatformInput::MouseDown(event) => {
                self.generation += 1;
                if event.button == MouseButton::Left {
                    let button = *self.left_button.get_or_insert(if event.modifiers.control {
                        MouseButton::Right
                    } else {
                        MouseButton::Left
                    });
                    event.button = button;
                    if button == MouseButton::Right {
                        event.modifiers.control = false;
                        event.click_count = 1;
                    } else if *first_mouse {
                        event.first_mouse = true;
                        *first_mouse = false;
                    }
                }
            }
            PlatformInput::MouseMove(event) if event.pressed_button == Some(MouseButton::Left) => {
                if let Some(button) = self.left_button {
                    event.pressed_button = Some(button);
                    if button == MouseButton::Right {
                        event.modifiers.control = false;
                    }
                }
            }
            PlatformInput::MouseUp(event) if event.button == MouseButton::Left => {
                if let Some(button) = self.left_button.take() {
                    event.button = button;
                    if button == MouseButton::Right {
                        event.modifiers.control = false;
                        event.click_count = 1;
                    }
                }
            }
            _ => {}
        }
        Some(MacPointerInput {
            event,
            physical_pressed_button,
        })
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn cancel(&mut self) {
        self.left_button = None;
    }

    pub(super) fn cancel_generation(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.cancel();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::{
        event::{CGEvent, CGEventFlags, CGEventType, CGMouseButton, EventField},
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::CGPoint,
    };
    use foreign_types::ForeignType;
    use objc2::{ClassType, msg_send, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::{NSEventModifierFlags, NSEventType};
    use objc2_foundation::NSPoint;
    use open_gpui::{
        Context, InteractiveElement, IntoElement, Modifiers, PointerCaptureHandle, Render, Styled,
        Window, div, px,
    };

    struct RightButtonCapture {
        handle: PointerCaptureHandle,
    }

    impl Render for RightButtonCapture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let handle = self.handle;
            div()
                .w(px(100.0))
                .h(px(100.0))
                .track_pointer_capture(&handle)
                .on_mouse_down(MouseButton::Right, move |_, window, _| {
                    window.capture_pointer(&handle, MouseButton::Right).unwrap();
                })
        }
    }

    fn mouse_event(event_type: NSEventType, modifiers: NSEventModifierFlags) -> Retained<NSEvent> {
        let event: Option<Retained<NSEvent>> = if event_type == NSEventType::RightMouseUp {
            // NSEvent's mouse factory can report button 0 for RightMouseUp. Quartz accepts
            // the physical button explicitly, preserving the unrelated-button release fixture.
            let source = CGEventSource::new(CGEventSourceStateID::Private)
                .expect("Quartz must construct a private event source");
            let event = CGEvent::new_mouse_event(
                source,
                CGEventType::RightMouseUp,
                CGPoint::new(20.0, 80.0),
                CGMouseButton::Right,
            )
            .expect("Quartz must construct a physical right-button release");
            event.set_flags(CGEventFlags::from_bits_truncate(modifiers.bits() as u64));
            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 2);
            // SAFETY: Both bindings refer to the same CGEvent type, which remains retained
            // until AppKit has constructed its NSEvent.
            unsafe {
                let event = &*event.as_ptr().cast::<objc2_core_graphics::CGEvent>();
                msg_send![NSEvent::class(), eventWithCGEvent: event]
            }
        } else {
            unsafe {
                msg_send![
                    NSEvent::class(),
                    mouseEventWithType: event_type,
                    location: NSPoint::new(20.0, 80.0),
                    modifierFlags: modifiers,
                    timestamp: 0.0_f64,
                    windowNumber: 0_isize,
                    context: std::ptr::null::<AnyObject>(),
                    eventNumber: 0_isize,
                    clickCount: 2_isize,
                    pressure: 1.0_f32,
                ]
            }
        };
        let event = event.expect("AppKit must construct a mouse event");
        let button_number = match event_type {
            NSEventType::LeftMouseDown
            | NSEventType::LeftMouseDragged
            | NSEventType::LeftMouseUp => 0,
            NSEventType::RightMouseUp => 1,
            _ => panic!("unsupported native pointer fixture: {event_type:?}"),
        };
        assert_eq!(event.r#type(), event_type);
        assert_eq!(event.buttonNumber(), button_number, "{event_type:?}");
        event
    }

    fn native_input(
        pointer: &mut MacPointerButton,
        first_mouse: &mut bool,
        event_type: NSEventType,
        modifiers: NSEventModifierFlags,
    ) -> MacPointerInput {
        pointer
            .input(&mouse_event(event_type, modifiers), px(100.0), first_mouse)
            .expect("the native mouse event must reach the view input producer")
    }

    #[open_gpui::test]
    fn control_left_release_after_control_up_ends_the_real_core_pointer_session(
        cx: &mut open_gpui::TestAppContext,
    ) {
        let (_, cx) = cx.add_window_view(|window, _| RightButtonCapture {
            handle: window.new_pointer_capture_handle(),
        });
        let _ = cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let mut pointer = MacPointerButton::default();
        let mut first_mouse = false;
        for (event_type, modifiers, is_down) in [
            (
                NSEventType::LeftMouseDown,
                NSEventModifierFlags::Control,
                true,
            ),
            (
                NSEventType::LeftMouseDragged,
                NSEventModifierFlags::empty(),
                false,
            ),
            (
                NSEventType::LeftMouseUp,
                NSEventModifierFlags::empty(),
                false,
            ),
        ] {
            let input = native_input(&mut pointer, &mut first_mouse, event_type, modifiers);
            match &input.event {
                PlatformInput::MouseDown(event) => {
                    assert_eq!(event.button, MouseButton::Right);
                    assert_eq!(event.click_count, 1);
                    assert!(!event.modifiers.control);
                }
                PlatformInput::MouseMove(event) => {
                    assert_eq!(event.pressed_button, Some(MouseButton::Right));
                    assert_eq!(input.physical_pressed_button, Some(MouseButton::Left));
                    assert!(super::super::file_drag::synthetic_drag_button_is_pressed(
                        input.physical_pressed_button,
                        1
                    ));
                }
                PlatformInput::MouseUp(event) => assert_eq!(event.button, MouseButton::Right),
                event => panic!("unexpected native view input: {event:?}"),
            }
            cx.update(|window, cx| {
                window.dispatch_event(input.event, cx);
                if is_down {
                    assert!(window.has_active_pointer_session(cx));
                    assert_eq!(
                        window.captured_pointer().unwrap().button(),
                        MouseButton::Right
                    );
                }
                if event_type == NSEventType::LeftMouseUp {
                    assert!(!window.has_active_pointer_session(cx));
                    assert!(window.captured_pointer().is_none());
                }
            });
        }
    }

    #[test]
    fn native_left_press_then_control_preserves_left_and_the_focusing_click() {
        let mut pointer = MacPointerButton::default();
        let mut first_mouse = true;
        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseDown,
            NSEventModifierFlags::empty(),
        );
        let PlatformInput::MouseDown(event) = input.event else {
            panic!()
        };
        assert_eq!(event.button, MouseButton::Left);
        assert!(event.first_mouse);
        assert_eq!(event.click_count, 2);
        assert!(!first_mouse);

        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseDragged,
            NSEventModifierFlags::Control | NSEventModifierFlags::Shift,
        );
        let PlatformInput::MouseMove(event) = input.event else {
            panic!()
        };
        assert_eq!(event.pressed_button, Some(MouseButton::Left));
        assert!(event.modifiers.control && event.modifiers.shift);

        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseUp,
            NSEventModifierFlags::Control,
        );
        let PlatformInput::MouseUp(event) = input.event else {
            panic!()
        };
        assert_eq!(event.button, MouseButton::Left);
        assert!(event.modifiers.control);
        assert_eq!(event.click_count, 2);
    }

    #[test]
    fn native_control_left_keeps_mapping_and_other_modifiers_until_matching_up() {
        let mut pointer = MacPointerButton::default();
        let mut first_mouse = true;
        for modifiers in [
            NSEventModifierFlags::Control | NSEventModifierFlags::Option,
            NSEventModifierFlags::Option,
        ] {
            let input = native_input(
                &mut pointer,
                &mut first_mouse,
                NSEventType::LeftMouseDown,
                modifiers,
            );
            let PlatformInput::MouseDown(event) = input.event else {
                panic!()
            };
            assert_eq!(event.button, MouseButton::Right);
            assert!(!event.first_mouse);
            assert_eq!(
                event.modifiers,
                Modifiers {
                    alt: true,
                    ..Modifiers::default()
                }
            );
            assert!(first_mouse);
        }
        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::RightMouseUp,
            NSEventModifierFlags::empty(),
        );
        assert!(
            matches!(input.event, PlatformInput::MouseUp(event) if event.button == MouseButton::Right)
        );
        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseUp,
            NSEventModifierFlags::Control | NSEventModifierFlags::Option,
        );
        let PlatformInput::MouseUp(event) = input.event else {
            panic!()
        };
        assert_eq!(event.button, MouseButton::Right);
        assert!(!event.modifiers.control);
        assert!(event.modifiers.alt);
        assert_eq!(event.click_count, 1);
        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseDown,
            NSEventModifierFlags::empty(),
        );
        assert!(
            matches!(input.event, PlatformInput::MouseDown(event) if event.button == MouseButton::Left && event.first_mouse)
        );
    }

    #[test]
    fn cancellation_resets_mapping_without_retiring_a_later_press() {
        let mut pointer = MacPointerButton::default();
        let mut first_mouse = false;
        native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseDown,
            NSEventModifierFlags::Control,
        );
        let old_generation = pointer.generation();
        pointer.cancel();
        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseDown,
            NSEventModifierFlags::empty(),
        );
        assert!(
            matches!(input.event, PlatformInput::MouseDown(event) if event.button == MouseButton::Left)
        );
        assert!(!pointer.cancel_generation(old_generation));
        let input = native_input(
            &mut pointer,
            &mut first_mouse,
            NSEventType::LeftMouseUp,
            NSEventModifierFlags::Control,
        );
        assert!(
            matches!(input.event, PlatformInput::MouseUp(event) if event.button == MouseButton::Left)
        );
        assert!(pointer.cancel_generation(pointer.generation()));
    }
}
