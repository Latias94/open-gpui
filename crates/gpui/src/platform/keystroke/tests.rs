use super::*;
use crate::{PlatformInfo, PlatformTarget, TestAppContext};

const HOST_SYSTEMS: [OperatingSystem; 7] = [
    OperatingSystem::Mac,
    OperatingSystem::Windows,
    OperatingSystem::Linux,
    OperatingSystem::FreeBsd,
    OperatingSystem::Ios,
    OperatingSystem::Android,
    OperatingSystem::Unknown,
];

#[test]
fn static_parser_rejects_secondary_with_original_diagnostic() {
    for source in [
        "secondary-a",
        "SECONDARY-A",
        "ctrl-secondary-a",
        "secondary",
    ] {
        let error = Keystroke::parse(source).unwrap_err();
        assert_eq!(error.keystroke, source);
        assert_eq!(
            error.to_string(),
            format!("Invalid keystroke \"{source}\". {KEYSTROKE_PARSE_EXPECTED_MESSAGE}")
        );
    }
}

#[test]
fn dynamic_parser_resolves_secondary_for_every_host_os() {
    let source = String::from("SeCoNdArY-Shift-Q");
    let modifier_source = String::from("secondary");
    let explicit = Keystroke::parse("ctrl-alt-cmd-fn-shift-enter").unwrap();

    for os in HOST_SYSTEMS {
        let secondary = if os == OperatingSystem::Mac {
            Modifiers::command()
        } else {
            Modifiers::control()
        };
        assert_eq!(
            Keystroke::parse_with_os(&source, os).unwrap(),
            Keystroke {
                modifiers: secondary | Modifiers::shift(),
                key: "q".into(),
                key_char: None,
            },
            "{os:?}"
        );
        let modifier = Keystroke::parse_with_os(&modifier_source, os).unwrap();
        assert_eq!(modifier.modifiers, Modifiers::none());
        assert_eq!(
            modifier.key,
            if os == OperatingSystem::Mac {
                "platform"
            } else {
                "control"
            },
            "{os:?}"
        );
        assert!(modifier.is_modifier_key());
        assert_eq!(
            Keystroke::parse_with_os(&String::from("ctrl-alt-cmd-fn-shift-enter"), os).unwrap(),
            explicit,
            "explicit modifiers must not depend on {os:?}"
        );
    }
}

#[test]
fn parser_preserves_modifier_only_priority_and_test_event_suffixes() {
    for (source, key, modifiers) in [
        ("ctrl", "control", Modifiers::none()),
        ("alt", "alt", Modifiers::none()),
        ("shift", "shift", Modifiers::none()),
        ("cmd", "platform", Modifiers::none()),
        ("super", "platform", Modifiers::none()),
        ("win", "platform", Modifiers::none()),
        ("fn", "function", Modifiers::none()),
        ("ctrl-shift", "shift", Modifiers::control()),
        ("alt-ctrl", "control", Modifiers::alt()),
    ] {
        let keystroke = Keystroke::parse(source).unwrap();
        assert_eq!(keystroke.key, key, "{source}");
        assert_eq!(keystroke.modifiers, modifiers, "{source}");
        assert!(keystroke.is_modifier_key(), "{source}");
    }

    assert_eq!(
        Keystroke::parse("ctrl--").unwrap(),
        Keystroke {
            modifiers: Modifiers::control(),
            key: "-".into(),
            key_char: None,
        }
    );
    assert_eq!(
        Keystroke::parse("Q").unwrap(),
        Keystroke::parse("shift-q").unwrap()
    );
    assert_eq!(Keystroke::parse("TAB").unwrap().key, "tab");
    assert_eq!(
        Keystroke::parse("alt-s->ß").unwrap(),
        Keystroke {
            modifiers: Modifiers::alt(),
            key: "s".into(),
            key_char: Some("ß".into()),
        }
    );
    assert!(!Keystroke::parse("a").unwrap().is_modifier_key());
}

#[test]
fn dynamic_parse_errors_own_the_original_input() {
    for os in HOST_SYSTEMS {
        let source = String::from("ctrl-a-b");
        let error = Keystroke::parse_with_os(&source, os).unwrap_err();
        drop(source);
        assert_eq!(error.keystroke, "ctrl-a-b");
        assert_eq!(
            error.to_string(),
            format!("Invalid keystroke \"ctrl-a-b\". {KEYSTROKE_PARSE_EXPECTED_MESSAGE}")
        );
    }
}

#[test]
fn display_and_unparse_follow_explicit_os_for_events_and_bindings() {
    let keystroke = Keystroke::parse("fn-ctrl-alt-cmd-shift-tab").unwrap();
    let binding = KeybindingKeystroke::from_keystroke(keystroke.clone());

    for (os, display, unparsed) in [
        (OperatingSystem::Mac, "^⌥⌘⇧⇥", "fn-ctrl-alt-cmd-shift-tab"),
        (
            OperatingSystem::Windows,
            "ctrl-alt-⊞shift-tab",
            "fn-ctrl-alt-win-shift-tab",
        ),
        (
            OperatingSystem::Linux,
            "ctrl-alt-❖shift-tab",
            "fn-ctrl-alt-super-shift-tab",
        ),
        (
            OperatingSystem::FreeBsd,
            "ctrl-alt-❖shift-tab",
            "fn-ctrl-alt-super-shift-tab",
        ),
        (
            OperatingSystem::Ios,
            "ctrl-alt-cmd-shift-tab",
            "fn-ctrl-alt-cmd-shift-tab",
        ),
        (
            OperatingSystem::Android,
            "ctrl-alt-cmd-shift-tab",
            "fn-ctrl-alt-super-shift-tab",
        ),
        (
            OperatingSystem::Unknown,
            "ctrl-alt-cmd-shift-tab",
            "fn-ctrl-alt-super-shift-tab",
        ),
    ] {
        assert_eq!(keystroke.display(os).to_string(), display, "{os:?}");
        assert_eq!(binding.display(os).to_string(), display, "{os:?}");
        assert_eq!(keystroke.unparse(os), unparsed, "{os:?}");
        assert_eq!(binding.unparse(os), unparsed, "{os:?}");
        assert_eq!(
            Keystroke::parse_with_os(&keystroke.unparse(os), os).unwrap(),
            keystroke,
            "{os:?}"
        );
    }
}

#[test]
fn named_key_glyphs_are_selected_by_runtime_os() {
    for (key, mac_display) in [
        ("backspace", "⌫"),
        ("up", "↑"),
        ("down", "↓"),
        ("left", "←"),
        ("right", "→"),
        ("tab", "⇥"),
        ("escape", "⎋"),
        ("shift", "⇧"),
        ("control", "⌃"),
        ("alt", "⌥"),
        ("platform", "⌘"),
    ] {
        let keystroke = Keystroke {
            key: key.into(),
            ..Default::default()
        };
        for os in HOST_SYSTEMS {
            assert_eq!(
                keystroke.display(os).to_string(),
                if os == OperatingSystem::Mac {
                    mac_display
                } else {
                    key
                },
                "{key} on {os:?}"
            );
        }
    }
    let keystroke = Keystroke::parse("ctrl-a").unwrap();
    assert_eq!(keystroke.display(OperatingSystem::Mac).to_string(), "^A");
    assert_eq!(
        keystroke.display(OperatingSystem::Windows).to_string(),
        "ctrl-A"
    );
}

#[test]
fn matching_keeps_physical_keys_and_ime_character_paths() {
    let target = KeybindingKeystroke::from_keystroke(Keystroke::parse("\"").unwrap());
    let typed = Keystroke::parse("shift-2->\"").unwrap();
    assert!(typed.should_match(&target));
    assert!(typed.should_match(&KeybindingKeystroke::from_keystroke(
        Keystroke::parse("shift-2").unwrap()
    )));

    let target = KeybindingKeystroke::from_keystroke(Keystroke::parse("ctrl-\"").unwrap());
    let typed = Keystroke::parse("ctrl-alt-ç->\"").unwrap();
    assert_eq!(typed.should_match(&target), !cfg!(target_os = "windows"));
    assert!(!typed.should_match(&KeybindingKeystroke::from_keystroke(
        Keystroke::parse("cmd-\"").unwrap()
    )));
}

#[cfg(target_os = "windows")]
#[test]
fn mapped_windows_display_keeps_its_raw_keystroke() {
    let raw = Keystroke::parse("ctrl-@").unwrap();
    let mut binding = KeybindingKeystroke::new(raw.clone(), Modifiers::control_shift(), "2".into());
    assert_eq!(binding.inner(), &raw);
    assert!(raw.should_match(&binding));
    assert_eq!(
        binding.display(OperatingSystem::Windows).to_string(),
        "ctrl-shift-2"
    );
    assert_eq!(binding.display(OperatingSystem::Mac).to_string(), "^⇧2");
    assert_eq!(binding.unparse(OperatingSystem::Windows), "ctrl-shift-2");
    assert_eq!(raw.unparse(OperatingSystem::Windows), "ctrl-@");

    binding.set_key("a".into());
    binding.set_modifiers(Modifiers::command());
    assert_eq!(binding.inner().key, "a");
    assert_eq!(binding.inner().modifiers, Modifiers::command());
    assert_eq!(binding.unparse(OperatingSystem::Windows), "win-a");
    assert_eq!(binding.display(OperatingSystem::Mac).to_string(), "⌘A");
}

#[crate::test]
fn secondary_helpers_follow_runtime_host_on_native_and_web(cx: &mut TestAppContext) {
    for os in HOST_SYSTEMS {
        for target in [PlatformTarget::Native, PlatformTarget::Web] {
            let info = PlatformInfo { os, target };
            cx.set_platform_info(info);
            cx.read(|app| {
                assert_eq!(app.platform_info(), info);
                let expected = if os == OperatingSystem::Mac {
                    Modifiers::command()
                } else {
                    Modifiers::control()
                };
                assert_eq!(Modifiers::secondary_key(app), expected, "{info:?}");
                assert!(expected.secondary(app), "{info:?}");
                assert_eq!(
                    Modifiers::command().secondary(app),
                    info.is_mac(),
                    "{info:?}"
                );
                assert_eq!(
                    Modifiers::control().secondary(app),
                    !info.is_mac(),
                    "{info:?}"
                );
                assert!(!Modifiers::none().secondary(app));
            });
        }
    }
}
