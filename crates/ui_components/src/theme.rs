//! Theme resolution helpers for component color intents.

use open_gpui::{Rgba, rgb};
use open_gpui_ui_core::ThemeTokens;

use crate::button::{ButtonColors, ButtonVariant};
use crate::color::ColorIntent;
use crate::field::FieldColors;
use crate::switch::SwitchColors;
use crate::text_input::TextInputColors;

const DEFAULT_SURFACE: u32 = 0xffffff;
const DEFAULT_GHOST_SURFACE: u32 = 0xf6f7f2;
const DEFAULT_BORDER: u32 = 0xcfd5cc;
const DEFAULT_TEXT: u32 = 0x18202a;
const DEFAULT_ACCENT: u32 = 0x1f7a66;
const DEFAULT_ACCENT_HOVER: u32 = 0x176656;
const DEFAULT_ACCENT_FOREGROUND: u32 = 0xffffff;
const DEFAULT_SWITCH_THUMB_SURFACE: u32 = 0xffffff;
const DEFAULT_FOCUS_RING: u32 = 0x2f80ed;
const DEFAULT_DESTRUCTIVE: u32 = 0xb42318;
const DEFAULT_DESTRUCTIVE_HOVER: u32 = 0x971b12;
const DEFAULT_DESTRUCTIVE_FOREGROUND: u32 = 0xffffff;
const DEFAULT_READ_ONLY_SURFACE: u32 = 0xf8f9f5;
const DEFAULT_PLACEHOLDER: u32 = 0x6d7785;
const DEFAULT_MESSAGE: u32 = 0x5a6472;

/// Theme resolution namespace for component color intents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeResolver;

impl ThemeResolver {
    /// Resolves a color intent to a concrete GPUI color.
    pub fn resolve(intent: ColorIntent) -> Rgba {
        rgb(intent.fallback_rgb())
    }

    pub(crate) const fn button_colors(
        tokens: ThemeTokens,
        variant: ButtonVariant,
        selected: bool,
    ) -> ButtonColors {
        if selected {
            return Self::accent_button_colors(tokens);
        }

        match variant {
            ButtonVariant::Default => Self::accent_button_colors(tokens),
            ButtonVariant::Secondary => ButtonColors {
                background: ColorIntent::new(tokens.surface_muted, 0xe8ede6),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, 0xd6d8ce),
                hover_background: ColorIntent::new(tokens.surface_muted, 0xdfe6dc),
                focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
            },
            ButtonVariant::Outline => ButtonColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
                hover_background: ColorIntent::new(tokens.surface_muted, 0xf1f5ee),
                focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
            },
            ButtonVariant::Ghost => ButtonColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_GHOST_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.surface, DEFAULT_GHOST_SURFACE),
                hover_background: ColorIntent::new(tokens.surface_muted, 0xe8ede6),
                focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
            },
            ButtonVariant::Destructive => ButtonColors {
                background: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                foreground: ColorIntent::new(
                    tokens.destructive_foreground,
                    DEFAULT_DESTRUCTIVE_FOREGROUND,
                ),
                border: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                hover_background: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE_HOVER),
                focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
            },
        }
    }

    pub(crate) const fn switch_colors(tokens: ThemeTokens, checked: bool) -> SwitchColors {
        let track_token = if checked {
            tokens.accent
        } else {
            tokens.surface_muted
        };
        let track_fallback = if checked { DEFAULT_ACCENT } else { 0xdfe6dc };
        let border_token = if checked {
            tokens.accent
        } else {
            tokens.border
        };
        let border_fallback = if checked {
            DEFAULT_ACCENT
        } else {
            DEFAULT_BORDER
        };

        SwitchColors {
            track: ColorIntent::new(track_token, track_fallback),
            thumb: ColorIntent::new(tokens.surface, DEFAULT_SWITCH_THUMB_SURFACE),
            border: ColorIntent::new(border_token, border_fallback),
            label: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
        }
    }

    pub(crate) const fn text_input_colors(
        tokens: ThemeTokens,
        disabled: bool,
        read_only: bool,
        invalid: bool,
    ) -> TextInputColors {
        let border = if invalid {
            ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE)
        } else {
            ColorIntent::new(tokens.border, DEFAULT_BORDER)
        };
        let foreground = if disabled {
            ColorIntent::new(tokens.text_muted, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };
        let background = if disabled {
            ColorIntent::new(tokens.surface_muted, 0xf1f5ee)
        } else if read_only {
            ColorIntent::new(tokens.surface_muted, DEFAULT_READ_ONLY_SURFACE)
        } else {
            ColorIntent::new(tokens.surface, DEFAULT_SURFACE)
        };

        TextInputColors {
            background,
            foreground,
            placeholder: ColorIntent::new(tokens.text_muted, DEFAULT_PLACEHOLDER),
            border,
            focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
        }
    }

    pub(crate) const fn field_colors(
        tokens: ThemeTokens,
        disabled: bool,
        invalid: bool,
    ) -> FieldColors {
        let message = if invalid {
            ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE)
        } else {
            ColorIntent::new(tokens.text_muted, DEFAULT_MESSAGE)
        };
        let label = if disabled {
            ColorIntent::new(tokens.text_muted, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };

        FieldColors {
            label,
            message,
            required_marker: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
        }
    }

    const fn accent_button_colors(tokens: ThemeTokens) -> ButtonColors {
        ButtonColors {
            background: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            foreground: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
            border: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            hover_background: ColorIntent::new(tokens.accent, DEFAULT_ACCENT_HOVER),
            focus_ring: ColorIntent::new(tokens.focus_ring, DEFAULT_FOCUS_RING),
        }
    }
}
