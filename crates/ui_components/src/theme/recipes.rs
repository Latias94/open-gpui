use open_gpui_ui_core::ThemeTokens;

use crate::alert_dialog::{AlertDialogColors, AlertDialogIntent};
use crate::avatar::{AvatarColors, AvatarGroupCountColors};
use crate::badge::{BadgeColors, BadgeVariant};
use crate::button::{ButtonColors, ButtonVariant};
use crate::checkbox::CheckboxColors;
use crate::color::{ColorIntent, ColorState};
use crate::combobox::ComboboxColors;
use crate::command::CommandColors;
use crate::dialog::DialogColors;
use crate::feedback::{FeedbackColors, FeedbackIntent};
use crate::field::FieldColors;
use crate::hover_card::HoverCardColors;
use crate::kbd::KbdColors;
use crate::label::LabelColors;
use crate::listbox::ListboxColors;
use crate::menu::MenuColors;
use crate::popover::PopoverColors;
use crate::progress::ProgressColors;
use crate::radio::RadioGroupColors;
use crate::select::SelectColors;
use crate::separator::SeparatorColors;
use crate::sheet::SheetColors;
use crate::skeleton::SkeletonColors;
use crate::switch::SwitchColors;
use crate::table::TableToolbarColors;
use crate::text_input::TextInputColors;
use crate::tooltip::TooltipColors;

use super::palette::{
    DEFAULT_ACCENT, DEFAULT_ACCENT_FOREGROUND, DEFAULT_ACCENT_HOVER, DEFAULT_BORDER,
    DEFAULT_DESTRUCTIVE, DEFAULT_DESTRUCTIVE_FOREGROUND, DEFAULT_DESTRUCTIVE_HOVER,
    DEFAULT_FOCUS_RING, DEFAULT_GHOST_SURFACE, DEFAULT_MESSAGE, DEFAULT_PLACEHOLDER,
    DEFAULT_READ_ONLY_SURFACE, DEFAULT_SURFACE, DEFAULT_SWITCH_THUMB_SURFACE, DEFAULT_TEXT,
    DEFAULT_TEXT_MUTED,
};
use super::resolver::ThemeResolver;

#[allow(dead_code)]
const THEME_RECIPE_CATALOG: &[&str] = &[
    "alert_dialog_colors",
    "avatar_colors",
    "avatar_group_count_colors",
    "badge_colors",
    "button_colors",
    "checkbox_colors",
    "combobox_colors",
    "command_colors",
    "dialog_colors",
    "feedback_colors",
    "field_colors",
    "hover_card_colors",
    "kbd_colors",
    "label_colors",
    "listbox_colors",
    "menu_colors",
    "popover_colors",
    "progress_colors",
    "radio_group_colors",
    "select_colors",
    "separator_colors",
    "sheet_colors",
    "skeleton_colors",
    "switch_colors",
    "table_toolbar_colors",
    "text_input_colors",
    "textarea_colors",
    "tooltip_colors",
];

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

    pub(crate) const fn avatar_colors(tokens: ThemeTokens) -> AvatarColors {
        AvatarColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn avatar_group_count_colors(tokens: ThemeTokens) -> AvatarGroupCountColors {
        AvatarGroupCountColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn separator_colors(tokens: ThemeTokens) -> SeparatorColors {
        SeparatorColors {
            line: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn kbd_colors(tokens: ThemeTokens) -> KbdColors {
        KbdColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn progress_colors(tokens: ThemeTokens) -> ProgressColors {
        ProgressColors {
            track: ColorIntent::new(tokens.surface_muted, 0xdfe6dc),
            indicator: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
        }
    }

    pub(crate) const fn skeleton_colors(tokens: ThemeTokens) -> SkeletonColors {
        SkeletonColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
        }
    }

    pub(crate) const fn feedback_colors(
        tokens: ThemeTokens,
        intent: FeedbackIntent,
    ) -> FeedbackColors {
        let marker = match intent {
            FeedbackIntent::Neutral => ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            FeedbackIntent::Info | FeedbackIntent::Success => {
                ColorIntent::new(tokens.accent, DEFAULT_ACCENT)
            }
            FeedbackIntent::Warning => {
                ColorIntent::with_state(tokens.text_muted, ColorState::Message, 0xbf8700)
            }
            FeedbackIntent::Danger => ColorIntent::with_state(
                tokens.destructive,
                ColorState::Invalid,
                DEFAULT_DESTRUCTIVE,
            ),
        };

        FeedbackColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            marker,
        }
    }

    pub(crate) const fn tooltip_colors(tokens: ThemeTokens) -> TooltipColors {
        TooltipColors {
            background: ColorIntent::with_state(tokens.overlay, ColorState::Overlay, 0x263240),
            foreground: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn popover_colors(tokens: ThemeTokens, open: bool) -> PopoverColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        PopoverColors {
            background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn hover_card_colors(tokens: ThemeTokens, open: bool) -> HoverCardColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        HoverCardColors {
            background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn dialog_colors(tokens: ThemeTokens, open: bool) -> DialogColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        DialogColors {
            barrier: ColorIntent::with_state(
                tokens.modal_overlay,
                ColorState::ModalOverlay,
                0x000000,
            ),
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn alert_dialog_colors(
        tokens: ThemeTokens,
        intent: AlertDialogIntent,
        open: bool,
    ) -> AlertDialogColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };
        let action_background = match intent {
            AlertDialogIntent::Default => ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            AlertDialogIntent::Destructive => {
                ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE)
            }
        };
        let action_hover_background = match intent {
            AlertDialogIntent::Default => {
                ColorIntent::with_state(tokens.accent, ColorState::Hover, DEFAULT_ACCENT_HOVER)
            }
            AlertDialogIntent::Destructive => ColorIntent::with_state(
                tokens.destructive,
                ColorState::Hover,
                DEFAULT_DESTRUCTIVE_HOVER,
            ),
        };
        let action_foreground = match intent {
            AlertDialogIntent::Default => {
                ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND)
            }
            AlertDialogIntent::Destructive => ColorIntent::new(
                tokens.destructive_foreground,
                DEFAULT_DESTRUCTIVE_FOREGROUND,
            ),
        };

        AlertDialogColors {
            barrier: ColorIntent::with_state(
                tokens.modal_overlay,
                ColorState::ModalOverlay,
                0x000000,
            ),
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            action_background,
            action_hover_background,
            action_foreground,
            action_border: action_background,
            cancel_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            cancel_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            cancel_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            cancel_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn sheet_colors(tokens: ThemeTokens, open: bool) -> SheetColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        SheetColors {
            barrier: ColorIntent::with_state(
                tokens.modal_overlay,
                ColorState::ModalOverlay,
                0x000000,
            ),
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            close_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            close_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            close_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            close_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

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

    pub(crate) const fn menu_colors(tokens: ThemeTokens, open: bool) -> MenuColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        MenuColors {
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            item_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            item_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            item_focus_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::FocusVisible,
                0xe8ede6,
            ),
            item_disabled_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                0x7a8491,
            ),
            header_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            separator: ColorIntent::new(tokens.border, DEFAULT_BORDER),
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
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn table_toolbar_colors(tokens: ThemeTokens) -> TableToolbarColors {
        TableToolbarColors {
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
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
