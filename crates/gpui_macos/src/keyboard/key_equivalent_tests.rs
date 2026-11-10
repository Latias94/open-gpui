use super::*;
use open_gpui::Modifiers;

#[test]
fn spanish_iso_shortcuts_use_current_punctuation_equivalents() {
    let mapper = MacKeyboardMapper::new("com.apple.keylayout.Spanish-ISO");
    for (key, expected) in [
        ("/", "'"),
        (";", "¡"),
        ("=", "*"),
        ("\\", "ç"),
        ("}", "¨"),
        (".", "."),
    ] {
        let keystroke = Keystroke {
            key: key.to_string(),
            modifiers: Modifiers::command(),
            key_char: None,
        };
        let mapped = mapper.map_key_equivalent(keystroke.clone(), true);
        assert_eq!(mapped.key(), expected, "{key}");
        assert_eq!(mapped.modifiers(), &keystroke.modifiers);
        assert_eq!(mapped.inner().key_char, keystroke.key_char);
        assert_eq!(mapper.map_key_equivalent(keystroke, false).key(), key);
    }
}

#[test]
fn spanish_iso_mapping_preserves_named_keys() {
    let mapper = MacKeyboardMapper::new("com.apple.keylayout.Spanish-ISO");
    for key in ["backspace", "delete"] {
        let keystroke = Keystroke {
            key: key.to_string(),
            modifiers: Modifiers::command(),
            key_char: None,
        };
        let mapped = mapper.map_key_equivalent(keystroke.clone(), true);
        assert_eq!(mapped.inner(), &keystroke);
    }
}
