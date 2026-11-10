//! Ordinary public Canvas-fallback policy consumption without browser or test support.

use open_gpui_web::{
    CanvasFontFallback,
    canvas_fallback::{CanvasFallback, classify_canvas_fallback},
};

#[test]
fn ordinary_application_can_select_policy_and_classify_complete_graphemes() {
    assert_eq!(CanvasFontFallback::default(), CanvasFontFallback::Emoji);
    let _: [CanvasFontFallback; 3] = [
        CanvasFontFallback::Disabled,
        CanvasFontFallback::Emoji,
        CanvasFontFallback::EmojiAndCjk,
    ];
    assert_eq!(
        classify_canvas_fallback("👩‍💻"),
        Some(CanvasFallback {
            is_emoji: true,
            emoji_presentation: true,
        })
    );
    assert_eq!(
        classify_canvas_fallback("漢"),
        Some(CanvasFallback {
            is_emoji: false,
            emoji_presentation: false,
        })
    );
    assert!(classify_canvas_fallback("👩‍💻漢").is_none());
}
