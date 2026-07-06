use open_gpui_ui_core::ThemeTokens;

use crate::badge::{BadgeColors, BadgeVariant};
use crate::button::{ButtonColors, ButtonVariant};
use crate::color::{ColorIntent, ColorState};
use crate::switch::SwitchColors;
use crate::theme::palette::{
    DEFAULT_ACCENT, DEFAULT_ACCENT_FOREGROUND, DEFAULT_ACCENT_HOVER, DEFAULT_BORDER,
    DEFAULT_DESTRUCTIVE, DEFAULT_DESTRUCTIVE_FOREGROUND, DEFAULT_DESTRUCTIVE_HOVER,
    DEFAULT_FOCUS_RING, DEFAULT_GHOST_SURFACE, DEFAULT_SURFACE, DEFAULT_SWITCH_THUMB_SURFACE,
    DEFAULT_TEXT,
};
use crate::theme::resolver::ThemeResolver;

impl ThemeResolver {
    pub(crate) const fn button_colors(
        tokens: ThemeTokens,
        variant: ButtonVariant,
        selected: bool,
    ) -> ButtonColors {
        if selected {
            return Self::accent_button_colors(tokens, ColorState::Selected);
        }

        match variant {
            ButtonVariant::Default => Self::accent_button_colors(tokens, ColorState::Default),
            ButtonVariant::Secondary => ButtonColors {
                background: ColorIntent::new(tokens.surface_muted, 0xe8ede6),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, 0xd6d8ce),
                hover_background: ColorIntent::with_state(
                    tokens.surface_muted,
                    ColorState::Hover,
                    0xdfe6dc,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
            ButtonVariant::Outline => ButtonColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
                hover_background: ColorIntent::with_state(
                    tokens.surface_muted,
                    ColorState::Hover,
                    0xf1f5ee,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
            ButtonVariant::Ghost => ButtonColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_GHOST_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.surface, DEFAULT_GHOST_SURFACE),
                hover_background: ColorIntent::with_state(
                    tokens.surface_muted,
                    ColorState::Hover,
                    0xe8ede6,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
            ButtonVariant::Destructive => ButtonColors {
                background: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                foreground: ColorIntent::new(
                    tokens.destructive_foreground,
                    DEFAULT_DESTRUCTIVE_FOREGROUND,
                ),
                border: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                hover_background: ColorIntent::with_state(
                    tokens.destructive,
                    ColorState::Hover,
                    DEFAULT_DESTRUCTIVE_HOVER,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
        }
    }

    pub(crate) const fn badge_colors(tokens: ThemeTokens, variant: BadgeVariant) -> BadgeColors {
        match variant {
            BadgeVariant::Default => BadgeColors {
                background: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
                foreground: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
                border: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            },
            BadgeVariant::Secondary => BadgeColors {
                background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            },
            BadgeVariant::Destructive => BadgeColors {
                background: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                foreground: ColorIntent::new(
                    tokens.destructive_foreground,
                    DEFAULT_DESTRUCTIVE_FOREGROUND,
                ),
                border: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
            },
            BadgeVariant::Outline => BadgeColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            track: ColorIntent::with_state(
                track_token,
                if checked {
                    ColorState::Selected
                } else {
                    ColorState::Default
                },
                track_fallback,
            ),
            thumb: ColorIntent::new(tokens.surface, DEFAULT_SWITCH_THUMB_SURFACE),
            border: ColorIntent::with_state(
                border_token,
                if checked {
                    ColorState::Selected
                } else {
                    ColorState::Default
                },
                border_fallback,
            ),
            label: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    const fn accent_button_colors(tokens: ThemeTokens, state: ColorState) -> ButtonColors {
        ButtonColors {
            background: ColorIntent::with_state(tokens.accent, state, DEFAULT_ACCENT),
            foreground: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
            border: ColorIntent::with_state(tokens.accent, state, DEFAULT_ACCENT),
            hover_background: ColorIntent::with_state(
                tokens.accent,
                ColorState::Hover,
                DEFAULT_ACCENT_HOVER,
            ),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }
}
