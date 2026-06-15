//! Component consumer samples for the foundation gallery.

use open_gpui_ui_components::{Button, ButtonState, ButtonVariant, Switch, SwitchState};
use open_gpui_ui_core::{Sizable, Size, ThemeTokens};

/// Page title.
pub const TITLE: &str = "Components";
/// Page summary.
pub const SUMMARY: &str = "First concrete component consumers built on the foundation crate.";
/// Foundation signals exercised by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_ui_components::Button",
    "open_gpui_ui_components::Switch",
    "ThemeTokens",
    "Size",
    "Role::Button",
    "Role::Switch",
];

/// One button sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: ButtonState,
}

/// One switch sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: SwitchState,
}

/// Returns button samples backed by real component state.
pub fn button_samples(tokens: ThemeTokens) -> [ButtonSample; 6] {
    [
        (
            "default",
            "Default",
            ButtonVariant::Default,
            Size::Medium,
            false,
            false,
        ),
        (
            "secondary",
            "Secondary",
            ButtonVariant::Secondary,
            Size::Medium,
            false,
            false,
        ),
        (
            "outline",
            "Outline",
            ButtonVariant::Outline,
            Size::Small,
            false,
            false,
        ),
        (
            "destructive",
            "Destructive",
            ButtonVariant::Destructive,
            Size::Medium,
            false,
            false,
        ),
        (
            "selected",
            "Selected",
            ButtonVariant::Ghost,
            Size::Medium,
            false,
            true,
        ),
        (
            "disabled",
            "Disabled",
            ButtonVariant::Default,
            Size::Medium,
            true,
            false,
        ),
    ]
    .map(
        |(id, label, variant, size, disabled, selected)| ButtonSample {
            id,
            label,
            state: Button::new(id, label)
                .variant(variant)
                .with_size(size)
                .disabled(disabled)
                .selected(selected)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns switch samples backed by real component state.
pub fn switch_samples(tokens: ThemeTokens) -> [SwitchSample; 4] {
    [
        ("off", "Unchecked", false, false, Size::Medium),
        ("on", "Checked", true, false, Size::Medium),
        ("small", "Small checked", true, false, Size::Small),
        ("disabled", "Disabled", false, true, Size::Medium),
    ]
    .map(|(id, label, checked, disabled, size)| SwitchSample {
        id,
        label,
        state: Switch::new(id)
            .label(label)
            .checked(checked)
            .disabled(disabled)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}
