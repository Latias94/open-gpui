use open_gpui_shared_string::SharedString;
use serde::{
    Deserialize,
    de::value::{BorrowedStrDeserializer, Error},
};

#[test]
fn char_conversion_preserves_utf8() {
    for (character, expected) in [
        ('\0', "\0"),
        ('a', "a"),
        ('é', "é"),
        ('中', "中"),
        ('🦀', "🦀"),
    ] {
        let string: SharedString = character.into();
        assert_eq!(string.as_str(), expected);
    }
}

#[test]
fn serde_string_boundaries() {
    for (json, expected) in [
        (r#""""#, ""),
        (r#""small""#, "small"),
        (r#""line\n\t\"\\end""#, "line\n\t\"\\end"),
        (r#""é中🦀""#, "é中🦀"),
        (r#""\u00e9\u4e2d\ud83e\udd80""#, "é中🦀"),
        (
            r#""a string long enough to exceed inline string storage""#,
            "a string long enough to exceed inline string storage",
        ),
    ] {
        let parsed: SharedString = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.as_str(), expected);

        let borrowed =
            SharedString::deserialize(BorrowedStrDeserializer::<Error>::new(expected)).unwrap();
        assert_eq!(borrowed, parsed);

        let owned: SharedString =
            serde_json::from_value(serde_json::Value::String(expected.to_owned())).unwrap();
        assert_eq!(owned, parsed);

        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            serde_json::to_string(expected).unwrap()
        );
    }

    assert!(serde_json::from_str::<SharedString>("42").is_err());
}
