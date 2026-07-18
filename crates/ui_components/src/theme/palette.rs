use std::sync::LazyLock;

use open_gpui_ui_core::{ThemeDesignScales, semantic};

use crate::color::ColorState;

use super::snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};

pub(super) const COMPLETE_THEME_COLOR_COUNT: usize = LIGHT_THEME_COLORS.len();

pub(super) const DEFAULT_SURFACE: u32 = 0xffffff;
pub(super) const DEFAULT_GHOST_SURFACE: u32 = 0xf6f7f2;
pub(super) const DEFAULT_BORDER: u32 = 0xcfd5cc;
pub(super) const DEFAULT_TEXT: u32 = 0x18202a;
pub(super) const DEFAULT_ACCENT: u32 = 0x1f7a66;
pub(super) const DEFAULT_ACCENT_HOVER: u32 = 0x176656;
pub(super) const DEFAULT_ACCENT_FOREGROUND: u32 = 0xffffff;
pub(super) const DEFAULT_SWITCH_THUMB_SURFACE: u32 = 0xffffff;
pub(super) const DEFAULT_FOCUS_RING: u32 = 0x2f80ed;
pub(super) const DEFAULT_DESTRUCTIVE: u32 = 0xb42318;
pub(super) const DEFAULT_DESTRUCTIVE_HOVER: u32 = 0x971b12;
pub(super) const DEFAULT_DESTRUCTIVE_FOREGROUND: u32 = 0xffffff;
pub(super) const DEFAULT_READ_ONLY_SURFACE: u32 = 0xf8f9f5;
pub(super) const DEFAULT_PLACEHOLDER: u32 = 0x6d7785;
pub(super) const DEFAULT_MESSAGE: u32 = 0x5a6472;
pub(super) const DEFAULT_TEXT_MUTED: u32 = 0x5a6472;

const LIGHT_THEME_REVISION: u64 = 1;
const DARK_THEME_REVISION: u64 = 2;
const HIGH_CONTRAST_THEME_REVISION: u64 = 3;

static LIGHT_THEME_SNAPSHOT: LazyLock<ThemeSnapshot> = LazyLock::new(|| {
    ThemeSnapshot::new(
        ThemeMode::Light,
        LIGHT_THEME_REVISION,
        LIGHT_THEME_COLORS,
        ThemeDesignScales::default(),
    )
});
static DARK_THEME_SNAPSHOT: LazyLock<ThemeSnapshot> = LazyLock::new(|| {
    ThemeSnapshot::new(
        ThemeMode::Dark,
        DARK_THEME_REVISION,
        DARK_THEME_COLORS,
        ThemeDesignScales::default(),
    )
});
static HIGH_CONTRAST_THEME_SNAPSHOT: LazyLock<ThemeSnapshot> = LazyLock::new(|| {
    ThemeSnapshot::new(
        ThemeMode::HighContrast,
        HIGH_CONTRAST_THEME_REVISION,
        HIGH_CONTRAST_THEME_COLORS,
        ThemeDesignScales::default(),
    )
});

impl ThemeSnapshot {
    /// Returns the default light theme snapshot.
    pub fn light() -> Self {
        LazyLock::force(&LIGHT_THEME_SNAPSHOT).clone()
    }

    /// Returns the default dark theme snapshot.
    pub fn dark() -> Self {
        LazyLock::force(&DARK_THEME_SNAPSHOT).clone()
    }

    /// Returns the default high contrast theme snapshot.
    pub fn high_contrast() -> Self {
        LazyLock::force(&HIGH_CONTRAST_THEME_SNAPSHOT).clone()
    }
}

impl Default for ThemeSnapshot {
    fn default() -> Self {
        Self::light()
    }
}

const LIGHT_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, DEFAULT_SURFACE),
    ThemeColor::new(
        semantic::SURFACE_MUTED,
        ColorState::Default,
        DEFAULT_GHOST_SURFACE,
    ),
    ThemeColor::new(
        semantic::SURFACE_MUTED,
        ColorState::Selected,
        DEFAULT_GHOST_SURFACE,
    ),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xdfe6dc),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::FocusVisible, 0xe8ede6),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0xf1f5ee),
    ThemeColor::new(
        semantic::SURFACE_MUTED,
        ColorState::ReadOnly,
        DEFAULT_READ_ONLY_SURFACE,
    ),
    ThemeColor::new(semantic::BORDER, ColorState::Default, DEFAULT_BORDER),
    ThemeColor::new(semantic::TEXT, ColorState::Default, DEFAULT_TEXT),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Default, DEFAULT_MESSAGE),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Disabled, 0x7a8491),
    ThemeColor::new(
        semantic::TEXT_MUTED,
        ColorState::Placeholder,
        DEFAULT_PLACEHOLDER,
    ),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Message, DEFAULT_MESSAGE),
    ThemeColor::new(semantic::ACCENT, ColorState::Default, DEFAULT_ACCENT),
    ThemeColor::new(semantic::ACCENT, ColorState::Selected, DEFAULT_ACCENT),
    ThemeColor::new(semantic::ACCENT, ColorState::Hover, DEFAULT_ACCENT_HOVER),
    ThemeColor::new(
        semantic::ACCENT_FOREGROUND,
        ColorState::Default,
        DEFAULT_ACCENT_FOREGROUND,
    ),
    ThemeColor::new(
        semantic::FOCUS_RING,
        ColorState::Default,
        DEFAULT_FOCUS_RING,
    ),
    ThemeColor::new(
        semantic::FOCUS_RING,
        ColorState::FocusVisible,
        DEFAULT_FOCUS_RING,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Default,
        DEFAULT_DESTRUCTIVE,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Invalid,
        DEFAULT_DESTRUCTIVE,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Required,
        DEFAULT_DESTRUCTIVE,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Hover,
        DEFAULT_DESTRUCTIVE_HOVER,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        DEFAULT_DESTRUCTIVE_FOREGROUND,
    ),
    ThemeColor::new(semantic::OVERLAY, ColorState::Default, 0x263240),
    ThemeColor::new(semantic::OVERLAY, ColorState::Overlay, 0x263240),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::Default, 0x111827),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::ModalOverlay, 0x111827),
];

const DARK_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0x121417),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Default, 0x20262d),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Selected, 0x20262d),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0x2a333d),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::FocusVisible, 0x27313b),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0x1a1f25),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::ReadOnly, 0x171b20),
    ThemeColor::new(semantic::BORDER, ColorState::Default, 0x3b4450),
    ThemeColor::new(semantic::TEXT, ColorState::Default, 0xf4f7fb),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Default, 0xb7c0cc),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Disabled, 0x7e8996),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Placeholder, 0x9aa5b2),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Message, 0xb7c0cc),
    ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x5ee0bc),
    ThemeColor::new(semantic::ACCENT, ColorState::Selected, 0x5ee0bc),
    ThemeColor::new(semantic::ACCENT, ColorState::Hover, 0x39caa7),
    ThemeColor::new(semantic::ACCENT_FOREGROUND, ColorState::Default, 0x05231c),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::Default, 0x7ab8ff),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::FocusVisible, 0x7ab8ff),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Default, 0xff6b5f),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Invalid, 0xff6b5f),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Required, 0xff6b5f),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Hover, 0xff4d3f),
    ThemeColor::new(
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        0x1c0705,
    ),
    ThemeColor::new(semantic::OVERLAY, ColorState::Default, 0x0b0f14),
    ThemeColor::new(semantic::OVERLAY, ColorState::Overlay, 0x0b0f14),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::ModalOverlay, 0x000000),
];

const HIGH_CONTRAST_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Default, 0xf0f0f0),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Selected, 0xf0f0f0),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xd9e8ff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::FocusVisible, 0xe0f0ff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0xe6e6e6),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::ReadOnly, 0xf7f7f7),
    ThemeColor::new(semantic::BORDER, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::TEXT, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Default, 0x222222),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Disabled, 0x555555),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Placeholder, 0x333333),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Message, 0x222222),
    ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x005fcc),
    ThemeColor::new(semantic::ACCENT, ColorState::Selected, 0x005fcc),
    ThemeColor::new(semantic::ACCENT, ColorState::Hover, 0x004799),
    ThemeColor::new(semantic::ACCENT_FOREGROUND, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::Default, 0xffbf00),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::FocusVisible, 0xffbf00),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Default, 0xd00000),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Invalid, 0xd00000),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Required, 0xd00000),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Hover, 0x9f0000),
    ThemeColor::new(
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        0xffffff,
    ),
    ThemeColor::new(semantic::OVERLAY, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::OVERLAY, ColorState::Overlay, 0x000000),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::ModalOverlay, 0x000000),
];
