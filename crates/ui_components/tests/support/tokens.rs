use open_gpui_ui_core::{ThemeTokens, TokenKey};

pub const TEST_SURFACE: TokenKey = TokenKey::new("test.surface");
pub const TEST_SURFACE_MUTED: TokenKey = TokenKey::new("test.surface_muted");
pub const TEST_BORDER: TokenKey = TokenKey::new("test.border");
pub const TEST_TEXT: TokenKey = TokenKey::new("test.text");
pub const TEST_TEXT_MUTED: TokenKey = TokenKey::new("test.text_muted");
pub const TEST_ACCENT: TokenKey = TokenKey::new("test.accent");
pub const TEST_FOCUS_RING: TokenKey = TokenKey::new("test.focus_ring");
pub const TEST_DESTRUCTIVE: TokenKey = TokenKey::new("test.destructive");

pub fn custom_tokens() -> ThemeTokens {
    ThemeTokens {
        surface: TEST_SURFACE,
        surface_muted: TEST_SURFACE_MUTED,
        border: TEST_BORDER,
        text: TEST_TEXT,
        text_muted: TEST_TEXT_MUTED,
        accent: TEST_ACCENT,
        focus_ring: TEST_FOCUS_RING,
        destructive: TEST_DESTRUCTIVE,
        ..ThemeTokens::default()
    }
}
