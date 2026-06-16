//! Component consumer samples for the foundation gallery.

use open_gpui_ui_components::{
    Badge, BadgeState, BadgeVariant, Button, ButtonState, ButtonVariant, Checkbox, CheckboxState,
    Field, FieldState, IconButton, IconButtonState, Label, LabelState, RadioGroupState,
    RadioItemDescriptor, ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy,
    SplitterPanelDescriptor, SplitterState, Switch, SwitchState, TabsActivationMode,
    TabsItemDescriptor, TabsState, TextInput, TextInputState, Toggle, ToggleState, ToggleVariant,
    ToolbarItemDescriptor, ToolbarItemKind, ToolbarState,
};
use open_gpui_ui_core::{Orientation, Sizable, Size, ThemeTokens};

/// Page title.
pub const TITLE: &str = "Components";
/// Page summary.
pub const SUMMARY: &str = "First concrete component consumers built on the foundation crate.";
/// Foundation signals exercised by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_ui_foundation_gallery::pages::components::CONFORMANCE_GATES",
    "open_gpui_ui_foundation_gallery::pages::components::ComponentConformanceGate",
    "open_gpui_ui_components::Button",
    "open_gpui_ui_components::ButtonState",
    "open_gpui_ui_components::ButtonVariant",
    "open_gpui_ui_components::Badge",
    "open_gpui_ui_components::BadgeState",
    "open_gpui_ui_components::BadgeVariant",
    "open_gpui_ui_components::IconButton",
    "open_gpui_ui_components::IconButtonState",
    "open_gpui_ui_components::Switch",
    "open_gpui_ui_components::SwitchState",
    "open_gpui_ui_components::Checkbox",
    "open_gpui_ui_components::CheckboxState",
    "open_gpui_ui_components::RadioGroup",
    "open_gpui_ui_components::RadioGroupState",
    "open_gpui_ui_components::RadioItem",
    "open_gpui_ui_components::Toggle",
    "open_gpui_ui_components::ToggleState",
    "open_gpui_ui_components::ToggleVariant",
    "open_gpui_ui_components::Toolbar",
    "open_gpui_ui_components::ToolbarItem",
    "open_gpui_ui_components::ToolbarState",
    "open_gpui_ui_components::ToolbarItemKind",
    "open_gpui_ui_components::Label",
    "open_gpui_ui_components::LabelState",
    "open_gpui_ui_components::TextInput",
    "open_gpui_ui_components::TextInputState",
    "open_gpui_ui_components::TextInputController",
    "open_gpui_ui_components::Field",
    "open_gpui_ui_components::FieldState",
    "open_gpui_ui_components::Tabs",
    "open_gpui_ui_components::TabsItem",
    "open_gpui_ui_components::TabsActivationMode",
    "open_gpui_ui_components::TabsState",
    "open_gpui_ui_components::ScrollArea",
    "open_gpui_ui_components::ScrollAreaState",
    "open_gpui_ui_components::ScrollAreaAxis",
    "open_gpui_ui_components::ScrollResetPolicy",
    "open_gpui_ui_components::Splitter",
    "open_gpui_ui_components::SplitterState",
    "open_gpui_ui_components::SplitterPanel",
    "open_gpui_ui_components::SplitterPanelDescriptor",
    "ThemeTokens",
    "Size",
    "Role::Button",
    "Role::Toolbar",
    "Role::Switch",
    "Role::CheckBox",
    "Role::RadioGroup",
    "Role::RadioButton",
    "Role::Label",
    "Role::TextInput",
    "Role::TabList",
    "Role::Tab",
    "Role::TabPanel",
];

/// One component conformance gate shown by the Components page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentConformanceGate {
    /// Stable gate id.
    pub id: &'static str,
    /// Visible gate title.
    pub title: &'static str,
    /// Behavior or contract that this gate protects.
    pub summary: &'static str,
    /// Durable test or document evidence for this gate.
    pub evidence: &'static [&'static str],
}

/// Regression-prone component behaviors that every new slice should keep covered.
pub const CONFORMANCE_GATES: &[ComponentConformanceGate] = &[
    ComponentConformanceGate {
        id: "public-api-exports",
        title: "Public API exports",
        summary: "Crate root and prelude exports stay explicit for every shipped component type.",
        evidence: &[
            "crates/ui_components/src/lib.rs",
            "crates/ui_components/src/prelude.rs",
            "crates/ui_components/tests/components.rs",
        ],
    },
    ComponentConformanceGate {
        id: "gallery-metadata",
        title: "Gallery metadata",
        summary: "Components samples expose stable ids, real resolved state, and page signals.",
        evidence: &[
            "examples/ui-foundation-gallery/src/pages/components.rs",
            "examples/ui-foundation-gallery/tests/foundation_gallery.rs",
        ],
    },
    ComponentConformanceGate {
        id: "scroll-redraw",
        title: "Scroll redraw persistence",
        summary: "ScrollArea default handles survive reconstructed component values and reset only by policy.",
        evidence: &[
            "ScrollAreaRuntime",
            "scroll_area_default_handle_survives_reconstructed_component_values",
            "scroll_area_reset_key_resets_default_runtime_handle",
        ],
    },
    ComponentConformanceGate {
        id: "splitter-runtime",
        title: "Splitter runtime constraints",
        summary: "Splitter runtime fractions keep min/max and collapsed-panel restore behavior centralized.",
        evidence: &[
            "SplitterState::with_panel_fractions",
            "SplitterState::resized_by",
            "splitter_runtime_fraction_overrides_still_use_resize_constraints",
        ],
    },
    ComponentConformanceGate {
        id: "tabs-overflow",
        title: "Tabs overflow and roving focus",
        summary: "Tabs keep disabled-item skipping, tab-stop metadata, and vertical rail overflow dogfood.",
        evidence: &[
            "workspace-tabs",
            "components_page_tabs_samples_expose_roving_focus_contract",
            "docs/verification.md",
        ],
    },
    ComponentConformanceGate {
        id: "a11y-labels",
        title: "A11y labels and associations",
        summary: "Icon-only controls and label associations remain explicit instead of relying on visual text.",
        evidence: &[
            "IconButton::new",
            "Label::for_control",
            "components_page_samples_keep_explicit_a11y_metadata",
        ],
    },
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

/// One badge sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadgeSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: BadgeState,
}

/// One icon button sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconButtonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible icon glyph.
    pub icon: &'static str,
    /// Required accessible label.
    pub accessible_label: &'static str,
    /// Resolved state.
    pub state: IconButtonState,
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

/// One scroll area sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollAreaSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Rows or cells rendered inside the sample viewport.
    pub items: Vec<&'static str>,
    /// Resolved state.
    pub state: ScrollAreaState,
}

/// One splitter panel sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelSample {
    /// Stable panel id.
    pub id: &'static str,
    /// Visible title.
    pub title: &'static str,
    /// Panel body copy.
    pub body: &'static str,
    /// Panel descriptor.
    pub descriptor: SplitterPanelDescriptor,
}

/// One splitter sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Splitter orientation.
    pub orientation: Orientation,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Splitter panels.
    pub panels: Vec<SplitterPanelSample>,
    /// Resolved state.
    pub state: SplitterState,
}

/// One radio item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct RadioItemSample {
    /// Stable item value.
    pub value: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Whether the item is disabled.
    pub disabled: bool,
}

/// One radio group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct RadioGroupSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Tab orientation.
    pub orientation: Orientation,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Selected radio value.
    pub selected: &'static str,
    /// Whether the sample is disabled.
    pub disabled: bool,
    /// Whether the sample is required.
    pub required: bool,
    /// Radio items.
    pub items: Vec<RadioItemSample>,
    /// Resolved state.
    pub state: RadioGroupState,
}

/// One toggle sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToggleSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: ToggleState,
}

/// One toolbar item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarItemSample {
    /// Stable item value.
    pub value: &'static str,
    /// Visible or accessible label.
    pub label: &'static str,
    /// Icon glyph used by compact toolbar items.
    pub icon: Option<&'static str>,
    /// Item kind.
    pub kind: ToolbarItemKind,
    /// Whether the item is disabled.
    pub disabled: bool,
    /// Whether the toggle item is pressed.
    pub pressed: bool,
}

/// One toolbar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Toolbar orientation.
    pub orientation: Orientation,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Seeded focused item.
    pub focused: &'static str,
    /// Toolbar items.
    pub items: Vec<ToolbarItemSample>,
    /// Resolved state.
    pub state: ToolbarState,
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

/// Returns badge samples backed by real component state.
pub fn badge_samples(tokens: ThemeTokens) -> [BadgeSample; 4] {
    [
        ("default", "Live", BadgeVariant::Default, Size::Medium),
        ("secondary", "Beta", BadgeVariant::Secondary, Size::Medium),
        (
            "destructive",
            "Risk",
            BadgeVariant::Destructive,
            Size::Medium,
        ),
        ("outline", "Neutral", BadgeVariant::Outline, Size::Small),
    ]
    .map(|(id, label, variant, size)| BadgeSample {
        id,
        label,
        state: Badge::new(id, label)
            .variant(variant)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Returns icon button samples backed by real component state.
pub fn icon_button_samples(tokens: ThemeTokens) -> [IconButtonSample; 4] {
    [
        (
            "search",
            "?",
            "Search",
            ButtonVariant::Ghost,
            false,
            Size::Medium,
        ),
        (
            "add",
            "+",
            "Add item",
            ButtonVariant::Outline,
            false,
            Size::Small,
        ),
        (
            "delete",
            "!",
            "Delete item",
            ButtonVariant::Destructive,
            false,
            Size::Medium,
        ),
        (
            "locked",
            "x",
            "Locked action",
            ButtonVariant::Ghost,
            true,
            Size::Medium,
        ),
    ]
    .map(
        |(id, icon, accessible_label, variant, disabled, size)| IconButtonSample {
            id,
            icon,
            accessible_label,
            state: IconButton::new(id, icon, accessible_label)
                .variant(variant)
                .disabled(disabled)
                .with_size(size)
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

/// Returns scroll area samples backed by real component state.
pub fn scroll_area_samples(_tokens: ThemeTokens) -> [ScrollAreaSample; 3] {
    [
        ScrollAreaSample {
            id: "activity-log",
            title: "Activity log",
            summary: "Vertical viewport with stable metadata and preserved offset.",
            items: vec![
                "09:12  Indexed 128 records",
                "09:13  Synced component tokens",
                "09:15  Rebuilt preview cache",
                "09:17  Published validation report",
                "09:21  Accepted keyboard navigation update",
                "09:24  Queued layout smoke test",
                "09:28  Completed gallery startup path",
                "09:34  Updated engineering memory",
                "09:39  Prepared review notes",
            ],
            state: ScrollAreaState::resolve(
                "activity-log",
                ScrollAreaAxis::Vertical,
                Size::Medium,
                ScrollResetPolicy::Preserve,
                None,
            ),
        },
        ScrollAreaSample {
            id: "release-queue",
            title: "Release queue",
            summary: "Horizontal overflow for fixed-width operational lanes.",
            items: vec![
                "Intake",
                "Design",
                "Implementation",
                "Verification",
                "Docs",
                "Release",
                "Follow-up",
            ],
            state: ScrollAreaState::resolve(
                "release-queue",
                ScrollAreaAxis::Horizontal,
                Size::Small,
                ScrollResetPolicy::Preserve,
                None,
            ),
        },
        ScrollAreaSample {
            id: "data-grid",
            title: "Data grid",
            summary: "Two-axis viewport with explicit view-key reset semantics.",
            items: vec![
                "Component / Axis / Reset / Metrics",
                "Tabs / horizontal / preserve / medium",
                "ScrollArea / both / reset-on-key-change / small",
                "Menu / vertical / preserve / medium",
                "Dialog / none / preserve / medium",
                "Popover / none / preserve / medium",
                "ContextMenu / point / preserve / medium",
            ],
            state: ScrollAreaState::resolve(
                "data-grid",
                ScrollAreaAxis::Both,
                Size::Small,
                ScrollResetPolicy::ResetOnKeyChange,
                Some("components".to_string()),
            ),
        },
    ]
}

/// Returns splitter samples backed by real component state.
pub fn splitter_samples(_tokens: ThemeTokens) -> [SplitterSample; 2] {
    let workspace_panels = vec![
        SplitterPanelSample {
            id: "navigator",
            title: "Navigator",
            body: "Folders, symbols, and filters.",
            descriptor: SplitterPanelDescriptor::new("navigator", 0.24)
                .min_fraction(0.18)
                .max_fraction(0.34),
        },
        SplitterPanelSample {
            id: "editor",
            title: "Editor",
            body: "Primary working surface.",
            descriptor: SplitterPanelDescriptor::new("editor", 0.56)
                .min_fraction(0.42)
                .max_fraction(0.72),
        },
        SplitterPanelSample {
            id: "inspector",
            title: "Inspector",
            body: "Metadata and actions.",
            descriptor: SplitterPanelDescriptor::new("inspector", 0.2)
                .min_fraction(0.12)
                .max_fraction(0.28)
                .collapsible(true),
        },
    ];
    let details_panels = vec![
        SplitterPanelSample {
            id: "summary",
            title: "Summary",
            body: "Collapsed header keeps context visible.",
            descriptor: SplitterPanelDescriptor::new("summary", 0.32)
                .min_fraction(0.2)
                .max_fraction(0.45)
                .collapsible(true)
                .collapsed(true)
                .collapsed_fraction(0.08),
        },
        SplitterPanelSample {
            id: "details",
            title: "Details",
            body: "Scrollable content can own the remaining space.",
            descriptor: SplitterPanelDescriptor::new("details", 0.68)
                .min_fraction(0.42)
                .max_fraction(0.92),
        },
    ];

    [
        SplitterSample {
            id: "workspace-split",
            title: "Workspace split",
            summary: "Horizontal panels constrained by min and max fractions.",
            orientation: Orientation::Horizontal,
            size: Size::Medium,
            state: SplitterState::resolve(
                "workspace-split",
                Orientation::Horizontal,
                Size::Medium,
                false,
                workspace_panels
                    .iter()
                    .map(|panel| panel.descriptor.clone()),
            ),
            panels: workspace_panels,
        },
        SplitterSample {
            id: "details-split",
            title: "Details split",
            summary: "Vertical stack with a collapsed but restorable panel.",
            orientation: Orientation::Vertical,
            size: Size::Small,
            state: SplitterState::resolve(
                "details-split",
                Orientation::Vertical,
                Size::Small,
                false,
                details_panels.iter().map(|panel| panel.descriptor.clone()),
            ),
            panels: details_panels,
        },
    ]
}

/// Returns radio group samples backed by real component state.
pub fn radio_group_samples(tokens: ThemeTokens) -> [RadioGroupSample; 2] {
    let persona_items = vec![
        RadioItemSample {
            value: "personal",
            label: "Personal",
            disabled: false,
        },
        RadioItemSample {
            value: "team",
            label: "Team",
            disabled: false,
        },
        RadioItemSample {
            value: "enterprise",
            label: "Enterprise",
            disabled: true,
        },
    ];
    let region_items = vec![
        RadioItemSample {
            value: "asia",
            label: "Asia",
            disabled: false,
        },
        RadioItemSample {
            value: "europe",
            label: "Europe",
            disabled: false,
        },
        RadioItemSample {
            value: "americas",
            label: "Americas",
            disabled: false,
        },
    ];

    [
        RadioGroupSample {
            id: "persona-radios",
            title: "Persona",
            summary: "Vertical group with required metadata and one disabled item.",
            orientation: Orientation::Vertical,
            size: Size::Medium,
            selected: "team",
            disabled: false,
            required: true,
            state: radio_group_state(
                Orientation::Vertical,
                Size::Medium,
                false,
                true,
                "team",
                &persona_items,
                tokens,
            ),
            items: persona_items,
        },
        RadioGroupSample {
            id: "region-radios",
            title: "Region",
            summary: "Horizontal group with compact sizing.",
            orientation: Orientation::Horizontal,
            size: Size::Small,
            selected: "europe",
            disabled: false,
            required: false,
            state: radio_group_state(
                Orientation::Horizontal,
                Size::Small,
                false,
                false,
                "europe",
                &region_items,
                tokens,
            ),
            items: region_items,
        },
    ]
}

/// Returns toggle samples backed by real component state.
pub fn toggle_samples(tokens: ThemeTokens) -> [ToggleSample; 4] {
    [
        (
            "ghost-off",
            "Ghost off",
            ToggleVariant::Ghost,
            false,
            false,
            Size::Medium,
        ),
        (
            "ghost-on",
            "Ghost on",
            ToggleVariant::Ghost,
            true,
            false,
            Size::Medium,
        ),
        (
            "outline-on",
            "Outline on",
            ToggleVariant::Outline,
            true,
            false,
            Size::Small,
        ),
        (
            "outline-disabled",
            "Disabled",
            ToggleVariant::Outline,
            false,
            true,
            Size::Medium,
        ),
    ]
    .map(
        |(id, label, variant, pressed, disabled, size)| ToggleSample {
            id,
            label,
            state: Toggle::new(id, label)
                .variant(variant)
                .pressed(pressed)
                .disabled(disabled)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns toolbar samples backed by real component state.
pub fn toolbar_samples(tokens: ThemeTokens) -> [ToolbarSample; 2] {
    let editor_items = vec![
        ToolbarItemSample {
            value: "undo",
            label: "Undo",
            icon: Some("U"),
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
        },
        ToolbarItemSample {
            value: "redo",
            label: "Redo",
            icon: Some("R"),
            kind: ToolbarItemKind::Action,
            disabled: true,
            pressed: false,
        },
        ToolbarItemSample {
            value: "history-separator",
            label: "",
            icon: None,
            kind: ToolbarItemKind::Separator,
            disabled: true,
            pressed: false,
        },
        ToolbarItemSample {
            value: "bold",
            label: "Bold",
            icon: Some("B"),
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            pressed: true,
        },
        ToolbarItemSample {
            value: "italic",
            label: "Italic",
            icon: Some("I"),
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            pressed: false,
        },
        ToolbarItemSample {
            value: "save",
            label: "Save",
            icon: None,
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
        },
    ];
    let inspector_items = vec![
        ToolbarItemSample {
            value: "pin",
            label: "Pin",
            icon: Some("P"),
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            pressed: true,
        },
        ToolbarItemSample {
            value: "refresh",
            label: "Refresh",
            icon: Some("R"),
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
        },
        ToolbarItemSample {
            value: "inspector-separator",
            label: "",
            icon: None,
            kind: ToolbarItemKind::Separator,
            disabled: true,
            pressed: false,
        },
        ToolbarItemSample {
            value: "details",
            label: "Details",
            icon: Some("D"),
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
        },
    ];

    [
        ToolbarSample {
            id: "editor-toolbar",
            title: "Editor toolbar",
            summary: "Horizontal actions with separators, one disabled item, and pressed toggles.",
            orientation: Orientation::Horizontal,
            size: Size::Small,
            focused: "bold",
            state: toolbar_state(
                Orientation::Horizontal,
                Size::Small,
                "Editor toolbar",
                "bold",
                &editor_items,
                tokens,
            ),
            items: editor_items,
        },
        ToolbarSample {
            id: "inspector-toolbar",
            title: "Inspector rail",
            summary: "Vertical toolbar that keeps roving focus on command buttons.",
            orientation: Orientation::Vertical,
            size: Size::Medium,
            focused: "pin",
            state: toolbar_state(
                Orientation::Vertical,
                Size::Medium,
                "Inspector toolbar",
                "pin",
                &inspector_items,
                tokens,
            ),
            items: inspector_items,
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

fn toolbar_state(
    orientation: Orientation,
    size: Size,
    label: &str,
    focused: &str,
    items: &[ToolbarItemSample],
    tokens: ThemeTokens,
) -> ToolbarState {
    ToolbarState::resolve(
        orientation,
        size,
        false,
        label,
        Some(focused),
        items.iter().map(|item| {
            let descriptor = match item.kind {
                ToolbarItemKind::Action => ToolbarItemDescriptor::action(item.value, item.label),
                ToolbarItemKind::Toggle => {
                    ToolbarItemDescriptor::toggle(item.value, item.label).pressed(item.pressed)
                }
                ToolbarItemKind::Separator => ToolbarItemDescriptor::separator(item.value),
            };
            descriptor.disabled(item.disabled)
        }),
        tokens,
    )
}

fn radio_group_state(
    orientation: Orientation,
    size: Size,
    disabled: bool,
    required: bool,
    selected: &str,
    items: &[RadioItemSample],
    tokens: ThemeTokens,
) -> RadioGroupState {
    RadioGroupState::resolve(
        orientation,
        size,
        disabled,
        required,
        Some(selected),
        None,
        items
            .iter()
            .map(|item| RadioItemDescriptor::new(item.value, item.label).disabled(item.disabled)),
        tokens,
    )
}
