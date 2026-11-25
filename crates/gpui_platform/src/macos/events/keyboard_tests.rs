use super::*;
use objc2::{
    ClassType,
    rc::{Retained, autoreleasepool},
};
use objc2_foundation::{NSPoint, NSString};

fn key_event(
    event_type: NSEventType,
    characters: &str,
    modifiers: NSEventModifierFlags,
    is_repeat: bool,
) -> Retained<NSEvent> {
    let characters = NSString::from_str(characters);
    let event: Option<Retained<NSEvent>> = unsafe {
        msg_send![
            NSEvent::class(),
            keyEventWithType: event_type,
            location: NSPoint::new(0.0, 0.0),
            modifierFlags: modifiers,
            timestamp: 0.0_f64,
            windowNumber: 0_isize,
            context: std::ptr::null::<AnyObject>(),
            characters: &*characters,
            charactersIgnoringModifiers: &*characters,
            isARepeat: is_repeat,
            keyCode: 0_u16,
        ]
    };
    event.expect("AppKit must construct a keyboard event")
}

#[test]
fn menu_delete_equivalents_preserve_distinct_native_key_events() {
    autoreleasepool(|_| {
        for (key, native_characters, menu_equivalent) in [
            ("backspace", "\u{7f}", "\u{8}"),
            ("delete", "\u{f728}", "\u{7f}"),
        ] {
            assert_eq!(key_to_menu_equivalent(key), menu_equivalent);
            for event_type in [NSEventType::KeyDown, NSEventType::KeyUp] {
                let native_event = key_event(
                    event_type,
                    native_characters,
                    NSEventModifierFlags::empty(),
                    false,
                );
                let event = platform_input_from_native(&native_event, None);
                let keystroke = match event {
                    Some(PlatformInput::KeyDown(event)) => {
                        assert!(!event.is_held);
                        assert!(!event.prefer_character_input);
                        event.keystroke
                    }
                    Some(PlatformInput::KeyUp(event)) => event.keystroke,
                    event => panic!("expected a keyboard event, got {event:?}"),
                };
                assert_eq!(keystroke.key, key);
                assert_eq!(keystroke.key_char, None);
            }
        }
    });
}

#[test]
fn repeated_arrow_preserves_modifiers_without_implicit_function_or_character_intent() {
    autoreleasepool(|_| {
        let native_event = key_event(
            NSEventType::KeyDown,
            "\u{f700}",
            NSEventModifierFlags::Command
                | NSEventModifierFlags::Shift
                | NSEventModifierFlags::Function,
            true,
        );
        let Some(PlatformInput::KeyDown(event)) = platform_input_from_native(&native_event, None)
        else {
            panic!("expected a key-down event");
        };
        assert!(event.is_held);
        assert!(!event.prefer_character_input);
        assert_eq!(event.keystroke.key, "up");
        assert_eq!(event.keystroke.key_char, None);
        assert_eq!(
            event.keystroke.modifiers,
            Modifiers {
                platform: true,
                shift: true,
                ..Modifiers::default()
            }
        );
    });
}
