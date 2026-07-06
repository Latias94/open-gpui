use open_gpui_ui_core::ThemeTokens;

use crate::color::{ColorIntent, ColorState};
use crate::combobox::ComboboxColors;
use crate::command::CommandColors;
use crate::listbox::ListboxColors;
use crate::select::SelectColors;
use crate::theme::palette::{
    DEFAULT_BORDER, DEFAULT_FOCUS_RING, DEFAULT_GHOST_SURFACE, DEFAULT_MESSAGE, DEFAULT_SURFACE,
    DEFAULT_TEXT, DEFAULT_TEXT_MUTED,
};
use crate::theme::resolver::ThemeResolver;

impl ThemeResolver {
    pub(crate) const fn listbox_colors(tokens: ThemeTokens) -> ListboxColors {
        ListboxColors {
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            option_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            option_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            option_active_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::FocusVisible,
                0xe8ede6,
            ),
            option_selected_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                0xe8ede6,
            ),
            option_disabled_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                0x7a8491,
            ),
            separator: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn select_colors(tokens: ThemeTokens, open: bool) -> SelectColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        SelectColors {
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                DEFAULT_GHOST_SURFACE,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            trigger_placeholder_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            content_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            content_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            content_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn combobox_colors(tokens: ThemeTokens) -> ComboboxColors {
        ComboboxColors {
            popup_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            popup_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            popup_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn command_colors(tokens: ThemeTokens) -> CommandColors {
        CommandColors {
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            shortcut_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Message,
                DEFAULT_MESSAGE,
            ),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }
}
