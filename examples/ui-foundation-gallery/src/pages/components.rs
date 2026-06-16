//! Component consumer samples for the foundation gallery.

use open_gpui_ui_components::{
    Button, ButtonState, ButtonVariant, Checkbox, CheckboxState, Field, FieldState, Label,
    LabelState, Switch, SwitchState, TabsActivationMode, TabsItemDescriptor, TabsState, TextInput,
    TextInputState,
};
use open_gpui_ui_core::{Orientation, Sizable, Size, ThemeTokens};

/// Page title.
pub const TITLE: &str = "Components";
/// Page summary.
pub const SUMMARY: &str = "First concrete component consumers built on the foundation crate.";
/// Foundation signals exercised by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_ui_components::Button",
    "open_gpui_ui_components::Switch",
    "open_gpui_ui_components::Checkbox",
    "open_gpui_ui_components::Label",
    "open_gpui_ui_components::TextInput",
    "open_gpui_ui_components::TextInputController",
    "open_gpui_ui_components::Field",
    "open_gpui_ui_components::Tabs",
    "open_gpui_ui_components::TabsItem",
    "open_gpui_ui_components::TabsActivationMode",
    "open_gpui_ui_components::TabsState",
    "ThemeTokens",
    "Size",
    "Role::Button",
    "Role::Switch",
    "Role::CheckBox",
    "Role::Label",
    "Role::TextInput",
    "Role::TabList",
    "Role::Tab",
    "Role::TabPanel",
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

/// One checkbox sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: CheckboxState,
}

/// One label sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved label state.
    pub state: LabelState,
}

/// One text input sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample label.
    pub label: &'static str,
    /// Whether this sample is rendered with an editable controller.
    pub controller_driven: bool,
    /// Resolved state.
    pub state: TextInputState,
}

/// One field sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved field state.
    pub state: FieldState,
    /// Resolved control state.
    pub input_state: TextInputState,
}

/// One tab item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsItemSample {
    /// Stable tab value.
    pub value: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Panel copy shown for the sample.
    pub panel: &'static str,
    /// Whether the tab is disabled.
    pub disabled: bool,
}

/// One tabs sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Tab orientation.
    pub orientation: Orientation,
    /// Activation mode.
    pub activation_mode: TabsActivationMode,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Selected tab value.
    pub selected: &'static str,
    /// Tab items.
    pub items: Vec<TabsItemSample>,
    /// Resolved state.
    pub state: TabsState,
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

/// Returns checkbox samples backed by real component state.
pub fn checkbox_samples(tokens: ThemeTokens) -> [CheckboxSample; 6] {
    [
        (
            "unchecked",
            "Unchecked",
            false,
            false,
            false,
            false,
            false,
            Size::Medium,
        ),
        (
            "checked",
            "Checked",
            true,
            false,
            false,
            false,
            false,
            Size::Medium,
        ),
        (
            "mixed",
            "Indeterminate",
            false,
            true,
            false,
            false,
            false,
            Size::Medium,
        ),
        (
            "required",
            "Required",
            true,
            false,
            false,
            true,
            false,
            Size::Medium,
        ),
        (
            "invalid",
            "Invalid",
            false,
            false,
            false,
            true,
            true,
            Size::Medium,
        ),
        (
            "disabled",
            "Disabled",
            false,
            false,
            true,
            false,
            false,
            Size::Medium,
        ),
    ]
    .map(
        |(id, label, checked, indeterminate, disabled, required, invalid, size)| CheckboxSample {
            id,
            label,
            state: Checkbox::new(id)
                .label(label)
                .checked(checked)
                .indeterminate(indeterminate)
                .disabled(disabled)
                .required(required)
                .invalid(invalid)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns label samples backed by real component state.
pub fn label_samples(tokens: ThemeTokens) -> [LabelSample; 4] {
    [
        (
            "email",
            "Email",
            Some("email-input"),
            false,
            false,
            Size::Medium,
        ),
        (
            "terms",
            "Terms",
            Some("terms-checkbox"),
            true,
            false,
            Size::Medium,
        ),
        (
            "disabled",
            "Disabled",
            Some("disabled-control"),
            false,
            true,
            Size::Medium,
        ),
        ("standalone", "Standalone", None, false, false, Size::Small),
    ]
    .map(|(id, text, control_id, required, disabled, size)| {
        let label = Label::new(id, text)
            .required(required)
            .disabled(disabled)
            .with_size(size)
            .tokens(tokens);
        let label = if let Some(control_id) = control_id {
            label.for_control(control_id)
        } else {
            label
        };

        LabelSample {
            id,
            state: label.state(),
        }
    })
}

/// Returns text input samples backed by real component state.
pub fn text_input_samples(tokens: ThemeTokens) -> [TextInputSample; 5] {
    [
        (
            "default",
            "Default",
            "",
            "Email address",
            false,
            false,
            false,
            false,
            Size::Medium,
            true,
        ),
        (
            "filled",
            "Filled",
            "hello@example.com",
            "Email address",
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "invalid",
            "Invalid",
            "not-an-email",
            "Email address",
            false,
            false,
            true,
            true,
            Size::Medium,
            false,
        ),
        (
            "read-only",
            "Read only",
            "profile@example.com",
            "Email address",
            false,
            true,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "disabled",
            "Disabled",
            "",
            "Unavailable",
            true,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
    ]
    .map(
        |(
            id,
            label,
            value,
            placeholder,
            disabled,
            read_only,
            required,
            invalid,
            size,
            controller_driven,
        )| {
            TextInputSample {
                id,
                label,
                controller_driven,
                state: TextInput::new(id, label)
                    .value(value)
                    .placeholder(placeholder)
                    .disabled(disabled)
                    .read_only(read_only)
                    .required(required)
                    .invalid(invalid)
                    .with_size(size)
                    .tokens(tokens)
                    .state(),
            }
        },
    )
}

/// Returns field samples backed by real component state.
pub fn field_samples(tokens: ThemeTokens) -> [FieldSample; 3] {
    [
        (
            "email",
            "Email",
            "Use a work address.",
            None,
            "",
            "you@example.com",
            true,
            false,
            false,
        ),
        (
            "invalid",
            "Email",
            "Use a work address.",
            Some("Enter a valid email."),
            "not-an-email",
            "you@example.com",
            true,
            false,
            true,
        ),
        (
            "disabled",
            "Workspace",
            "Managed by your organization.",
            None,
            "",
            "Workspace name",
            false,
            true,
            false,
        ),
    ]
    .map(
        |(id, label, help, error, value, placeholder, required, disabled, invalid)| {
            let input = TextInput::new(format!("{id}-input"), label)
                .value(value)
                .placeholder(placeholder)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = Field::new(id, format!("{id}-input"), label)
                .help(help)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = if let Some(error) = error {
                field.error(error)
            } else {
                field
            };

            FieldSample {
                id,
                state: field.state(),
                input_state: input.state(),
            }
        },
    )
}

/// Returns tabs samples backed by real component state.
pub fn tabs_samples(tokens: ThemeTokens) -> [TabsSample; 2] {
    let overview_items = vec![
        TabsItemSample {
            value: "overview",
            label: "Overview",
            panel: "Project snapshot and recent activity.",
            disabled: false,
        },
        TabsItemSample {
            value: "details",
            label: "Details",
            panel: "Important metadata and settings.",
            disabled: false,
        },
        TabsItemSample {
            value: "history",
            label: "History",
            panel: "Previous revisions and timeline.",
            disabled: true,
        },
    ];
    let workspace_items = vec![
        TabsItemSample {
            value: "profile",
            label: "Profile",
            panel: "Identity and display settings.",
            disabled: false,
        },
        TabsItemSample {
            value: "security",
            label: "Security",
            panel: "Keys, authentication, and access rules.",
            disabled: false,
        },
        TabsItemSample {
            value: "billing",
            label: "Billing",
            panel: "Plans, invoices, and payment method.",
            disabled: false,
        },
        TabsItemSample {
            value: "integrations",
            label: "Integrations",
            panel: "Connected apps and webhooks.",
            disabled: true,
        },
        TabsItemSample {
            value: "notifications",
            label: "Notifications",
            panel: "Email and product alerts.",
            disabled: false,
        },
        TabsItemSample {
            value: "appearance",
            label: "Appearance",
            panel: "Theme and density preferences.",
            disabled: false,
        },
        TabsItemSample {
            value: "advanced",
            label: "Advanced",
            panel: "Migration and power-user settings.",
            disabled: false,
        },
    ];

    [
        TabsSample {
            id: "overview-tabs",
            title: "Overview",
            summary: "Automatic activation with roving focus and one disabled tab.",
            orientation: Orientation::Horizontal,
            activation_mode: TabsActivationMode::Automatic,
            size: Size::Medium,
            selected: "overview",
            state: tabs_state(
                Orientation::Horizontal,
                TabsActivationMode::Automatic,
                Size::Medium,
                "overview",
                &overview_items,
                tokens,
            ),
            items: overview_items,
        },
        TabsSample {
            id: "workspace-tabs",
            title: "Workspace",
            summary: "Manual activation with vertical navigation.",
            orientation: Orientation::Vertical,
            activation_mode: TabsActivationMode::Manual,
            size: Size::Small,
            selected: "profile",
            state: tabs_state(
                Orientation::Vertical,
                TabsActivationMode::Manual,
                Size::Small,
                "profile",
                &workspace_items,
                tokens,
            ),
            items: workspace_items,
        },
    ]
}

fn tabs_state(
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    size: Size,
    selected: &str,
    items: &[TabsItemSample],
    tokens: ThemeTokens,
) -> TabsState {
    TabsState::resolve(
        orientation,
        activation_mode,
        size,
        Some(selected),
        None,
        items
            .iter()
            .map(|item| TabsItemDescriptor::new(item.value, item.label).disabled(item.disabled)),
        tokens,
    )
}
