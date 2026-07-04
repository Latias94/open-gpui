use open_gpui_ui_core::ThemeTokens;

use crate::checkbox::CheckboxColors;
use crate::color::{ColorIntent, ColorState};
use crate::field::FieldColors;
use crate::label::LabelColors;
use crate::radio::RadioGroupColors;
use crate::text_input::TextInputColors;
use crate::theme::palette::{
    DEFAULT_ACCENT, DEFAULT_ACCENT_FOREGROUND, DEFAULT_ACCENT_HOVER, DEFAULT_BORDER,
    DEFAULT_DESTRUCTIVE, DEFAULT_FOCUS_RING, DEFAULT_MESSAGE, DEFAULT_PLACEHOLDER,
    DEFAULT_READ_ONLY_SURFACE, DEFAULT_SURFACE, DEFAULT_TEXT, DEFAULT_TEXT_MUTED,
};
use crate::theme::resolver::ThemeResolver;

impl ThemeResolver {
    pub(crate) const fn text_input_colors(
        tokens: ThemeTokens,
        disabled: bool,
        read_only: bool,
        invalid: bool,
    ) -> TextInputColors {
        let border = if invalid {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, DEFAULT_DESTRUCTIVE)
        } else {
            ColorIntent::new(tokens.border, DEFAULT_BORDER)
        };
        let foreground = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };
        let background = if disabled {
            ColorIntent::with_state(tokens.surface_muted, ColorState::Disabled, 0xf1f5ee)
        } else if read_only {
            ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::ReadOnly,
                DEFAULT_READ_ONLY_SURFACE,
            )
        } else {
            ColorIntent::new(tokens.surface, DEFAULT_SURFACE)
        };

        TextInputColors {
            background,
            foreground,
            placeholder: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Placeholder,
                DEFAULT_PLACEHOLDER,
            ),
            border,
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn textarea_colors(
        tokens: ThemeTokens,
        disabled: bool,
        read_only: bool,
        invalid: bool,
    ) -> crate::textarea::TextareaColors {
        let colors = Self::text_input_colors(tokens, disabled, read_only, invalid);
        crate::textarea::TextareaColors {
            background: colors.background,
            foreground: colors.foreground,
            placeholder: colors.placeholder,
            border: colors.border,
            focus_ring: colors.focus_ring,
        }
    }

    pub(crate) const fn field_colors(
        tokens: ThemeTokens,
        disabled: bool,
        invalid: bool,
    ) -> FieldColors {
        let message = if invalid {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, DEFAULT_DESTRUCTIVE)
        } else {
            ColorIntent::with_state(tokens.text_muted, ColorState::Message, DEFAULT_MESSAGE)
        };
        let label = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };

        FieldColors {
            label,
            message,
            required_marker: ColorIntent::with_state(
                tokens.destructive,
                ColorState::Required,
                DEFAULT_DESTRUCTIVE,
            ),
        }
    }

    pub(crate) const fn checkbox_colors(
        tokens: ThemeTokens,
        checked: bool,
        indeterminate: bool,
        disabled: bool,
        invalid: bool,
    ) -> CheckboxColors {
        let selected = checked || indeterminate;
        let background = if disabled {
            ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Disabled,
                DEFAULT_READ_ONLY_SURFACE,
            )
        } else if selected {
            ColorIntent::with_state(tokens.accent, ColorState::Selected, DEFAULT_ACCENT)
        } else {
            ColorIntent::new(tokens.surface, DEFAULT_SURFACE)
        };
        let hover_background = if disabled {
            background
        } else if selected {
            ColorIntent::with_state(tokens.accent, ColorState::Hover, DEFAULT_ACCENT_HOVER)
        } else {
            ColorIntent::with_state(tokens.surface_muted, ColorState::Hover, 0xdfe6dc)
        };
        let border = if invalid {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, DEFAULT_DESTRUCTIVE)
        } else if selected {
            ColorIntent::with_state(tokens.accent, ColorState::Selected, DEFAULT_ACCENT)
        } else {
            ColorIntent::new(tokens.border, DEFAULT_BORDER)
        };
        let indicator = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND)
        };
        let label = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };

        CheckboxColors {
            background,
            hover_background,
            border,
            indicator,
            label,
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn radio_group_colors(tokens: ThemeTokens) -> RadioGroupColors {
        RadioGroupColors {
            control_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            control_background_selected: ColorIntent::with_state(
                tokens.accent,
                ColorState::Selected,
                DEFAULT_ACCENT,
            ),
            control_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            control_border_selected: ColorIntent::with_state(
                tokens.accent,
                ColorState::Selected,
                DEFAULT_ACCENT,
            ),
            indicator: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
            label: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            label_muted: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                DEFAULT_TEXT_MUTED,
            ),
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
        }
    }

    pub(crate) const fn label_colors(tokens: ThemeTokens, disabled: bool) -> LabelColors {
        LabelColors {
            text: if disabled {
                ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
            } else {
                ColorIntent::new(tokens.text, DEFAULT_TEXT)
            },
            required_marker: ColorIntent::with_state(
                tokens.destructive,
                ColorState::Required,
                DEFAULT_DESTRUCTIVE,
            ),
        }
    }
}
