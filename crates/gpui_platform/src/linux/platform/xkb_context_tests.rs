use super::*;

#[test]
fn server_context_accepts_resolved_keymaps_without_local_definitions() {
    let context = new_xkb_context().expect("server XKB context should initialize");
    assert!(!context.get_raw_ptr().is_null());
    assert_eq!(context.include_paths().count(), 0);

    let keymap = xkb::Keymap::new_from_string(
        &context,
        r#"xkb_keymap {
            xkb_keycodes "resolved" {
                minimum = 8;
                maximum = 255;
                <AC01> = 38;
            };
            xkb_types "resolved" {
                type "ONE_LEVEL" {
                    modifiers = None;
                    level_name[Level1] = "Any";
                };
            };
            xkb_compatibility "resolved" {};
            xkb_symbols "resolved" {
                key <AC01> { type = "ONE_LEVEL", [ a ] };
            };
        };"#
        .to_owned(),
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .expect("a resolved server keymap should compile without local definitions");
    let state = xkb::State::new(&keymap);
    assert_eq!(state.key_get_utf8(xkb::Keycode::new(38)), "a");

    assert!(
        xkb::Keymap::new_from_names(
            &context,
            "evdev",
            "pc105",
            "us",
            "",
            Some(String::new()),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .is_none(),
        "named layouts require include paths that server contexts deliberately omit"
    );
}
