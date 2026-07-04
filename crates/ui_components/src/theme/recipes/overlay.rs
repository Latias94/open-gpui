use open_gpui_ui_core::ThemeTokens;

use crate::alert_dialog::{AlertDialogColors, AlertDialogIntent};
use crate::color::{ColorIntent, ColorState};
use crate::dialog::DialogColors;
use crate::hover_card::HoverCardColors;
use crate::menu::MenuColors;
use crate::popover::PopoverColors;
use crate::sheet::SheetColors;
use crate::theme::palette::{
    DEFAULT_ACCENT, DEFAULT_ACCENT_FOREGROUND, DEFAULT_ACCENT_HOVER, DEFAULT_BORDER,
    DEFAULT_DESTRUCTIVE, DEFAULT_DESTRUCTIVE_FOREGROUND, DEFAULT_DESTRUCTIVE_HOVER,
    DEFAULT_FOCUS_RING, DEFAULT_GHOST_SURFACE, DEFAULT_SURFACE, DEFAULT_TEXT, DEFAULT_TEXT_MUTED,
};
use crate::theme::resolver::ThemeResolver;
use crate::tooltip::TooltipColors;

impl ThemeResolver {
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
}
