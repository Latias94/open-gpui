//! Component consumer samples for the foundation gallery.

use open_gpui_ui_components::{
    Avatar, AvatarState, Badge, BadgeState, BadgeVariant, Button, ButtonState, ButtonVariant,
    Checkbox, CheckboxState, ComboboxGroupDescriptor, ComboboxOpenMode, ComboboxOptionDescriptor,
    ComboboxState, CommandGroupDescriptor, CommandItemDescriptor, CommandOpenMode, CommandState,
    Field, FieldState, IconButton, IconButtonState, Kbd, KbdState, Label, LabelState,
    ListboxGroupDescriptor, ListboxOptionDescriptor, ListboxState, Progress, ProgressState,
    RadioGroupState, RadioItemDescriptor, ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy,
    SelectOpenMode, SelectState, Separator, SeparatorState, SidebarCollapseMode,
    SidebarItemDescriptor, SidebarSectionDescriptor, SidebarSide, SidebarState, SidebarVariant,
    Skeleton, SkeletonState, SplitterPanelDescriptor, SplitterState, Switch, SwitchState,
    TabsActivationMode, TabsItemDescriptor, TabsState, TextInput, TextInputState, Toggle,
    ToggleState, ToggleVariant, ToolbarItemDescriptor, ToolbarItemKind, ToolbarState,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, Orientation, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Sizable, Size, ThemeTokens,
};

/// Page title.
pub const TITLE: &str = "Components";
/// Page summary.
pub const SUMMARY: &str = "First concrete component consumers built on the foundation crate.";
/// Foundation signals exercised by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_ui_foundation_gallery::pages::components::CONFORMANCE_GATES",
    "open_gpui_ui_foundation_gallery::pages::components::ComponentConformanceGate",
    "open_gpui_ui_foundation_gallery::pages::components::COMPONENT_CATALOG",
    "open_gpui_ui_foundation_gallery::pages::components::ComponentCatalogEntry",
    "open_gpui_ui_foundation_gallery::pages::components::ComponentCatalogStatus",
    "open_gpui_ui_components::Button",
    "open_gpui_ui_components::ButtonState",
    "open_gpui_ui_components::ButtonVariant",
    "open_gpui_ui_components::Badge",
    "open_gpui_ui_components::BadgeState",
    "open_gpui_ui_components::BadgeVariant",
    "open_gpui_ui_components::Separator",
    "open_gpui_ui_components::SeparatorState",
    "open_gpui_ui_components::Kbd",
    "open_gpui_ui_components::KbdState",
    "open_gpui_ui_components::Progress",
    "open_gpui_ui_components::ProgressState",
    "open_gpui_ui_components::ProgressVisualMode",
    "open_gpui_ui_components::Skeleton",
    "open_gpui_ui_components::SkeletonState",
    "open_gpui_ui_components::Avatar",
    "open_gpui_ui_components::AvatarState",
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
    "open_gpui_ui_components::Sidebar",
    "open_gpui_ui_components::SidebarState",
    "open_gpui_ui_components::SidebarSection",
    "open_gpui_ui_components::SidebarItem",
    "open_gpui_ui_components::SidebarCollapseMode",
    "open_gpui_ui_components::Listbox",
    "open_gpui_ui_components::ListboxState",
    "open_gpui_ui_components::ListboxOption",
    "open_gpui_ui_components::ListboxGroup",
    "open_gpui_ui_components::Select",
    "open_gpui_ui_components::SelectState",
    "open_gpui_ui_components::SelectOpenMode",
    "open_gpui_ui_components::Combobox",
    "open_gpui_ui_components::ComboboxState",
    "open_gpui_ui_components::ComboboxOpenMode",
    "open_gpui_ui_components::Command",
    "open_gpui_ui_components::CommandState",
    "open_gpui_ui_components::CommandOpenMode",
    "open_gpui_ui_components::Label",
    "open_gpui_ui_components::LabelState",
    "open_gpui_ui_components::TextInput",
    "open_gpui_ui_components::TextInputState",
    "open_gpui_ui_components::gpui_adapter::TextInputController",
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
    "Role::Image",
    "Role::Toolbar",
    "Role::Navigation",
    "Role::Section",
    "Role::ListBox",
    "Role::ListBoxOption",
    "Role::EditableComboBox",
    "Role::ProgressIndicator",
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

/// Component catalog status shown by the Components page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentCatalogStatus {
    /// Official component with resolved state, exports, gallery sample, docs, and verification.
    Official,
    /// Public adapter helper or implementation detail that is not an official standalone component.
    AdapterOnly,
    /// Public anatomy used by an official component family, not a standalone gallery component.
    InternalAnatomy,
    /// Planned component not present in the current official catalog.
    Deferred,
}

impl ComponentCatalogStatus {
    /// Stable status label used by tests and the gallery.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::AdapterOnly => "adapter-only",
            Self::InternalAnatomy => "internal-anatomy",
            Self::Deferred => "deferred",
        }
    }
}

/// One component catalog entry shown by the Components page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentCatalogEntry {
    /// Public component or helper name.
    pub name: &'static str,
    /// Current catalog status.
    pub status: ComponentCatalogStatus,
    /// Component family or ownership area.
    pub family: &'static str,
    /// Resolved state or public contract type, when applicable.
    pub state: Option<&'static str>,
    /// Gallery, adapter, or follow-up coverage note.
    pub coverage: &'static str,
}

/// Official component catalog and adjacent public surfaces.
pub const COMPONENT_CATALOG: &[ComponentCatalogEntry] = &[
    ComponentCatalogEntry {
        name: "Button",
        status: ComponentCatalogStatus::Official,
        family: "action",
        state: Some("ButtonState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Badge",
        status: ComponentCatalogStatus::Official,
        family: "display",
        state: Some("BadgeState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "IconButton",
        status: ComponentCatalogStatus::Official,
        family: "action",
        state: Some("IconButtonState"),
        coverage: "exports / gallery / a11y metadata",
    },
    ComponentCatalogEntry {
        name: "Switch",
        status: ComponentCatalogStatus::Official,
        family: "form",
        state: Some("SwitchState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Checkbox",
        status: ComponentCatalogStatus::Official,
        family: "form",
        state: Some("CheckboxState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "RadioGroup",
        status: ComponentCatalogStatus::Official,
        family: "choice",
        state: Some("RadioGroupState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "Toggle",
        status: ComponentCatalogStatus::Official,
        family: "action",
        state: Some("ToggleState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Toolbar",
        status: ComponentCatalogStatus::Official,
        family: "shell",
        state: Some("ToolbarState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "Sidebar",
        status: ComponentCatalogStatus::Official,
        family: "shell",
        state: Some("SidebarState"),
        coverage: "exports / gallery / scroll smoke",
    },
    ComponentCatalogEntry {
        name: "Listbox",
        status: ComponentCatalogStatus::Official,
        family: "choice",
        state: Some("ListboxState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "Select",
        status: ComponentCatalogStatus::Official,
        family: "choice",
        state: Some("SelectState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "Combobox",
        status: ComponentCatalogStatus::Official,
        family: "choice-search",
        state: Some("ComboboxState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "Command",
        status: ComponentCatalogStatus::Official,
        family: "choice-search",
        state: Some("CommandState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "Label",
        status: ComponentCatalogStatus::Official,
        family: "form",
        state: Some("LabelState"),
        coverage: "exports / gallery / a11y metadata",
    },
    ComponentCatalogEntry {
        name: "TextInput",
        status: ComponentCatalogStatus::Official,
        family: "form",
        state: Some("TextInputState"),
        coverage: "exports / gallery / controller tests",
    },
    ComponentCatalogEntry {
        name: "Field",
        status: ComponentCatalogStatus::Official,
        family: "form",
        state: Some("FieldState"),
        coverage: "exports / gallery / composition tests",
    },
    ComponentCatalogEntry {
        name: "Tabs",
        status: ComponentCatalogStatus::Official,
        family: "navigation",
        state: Some("TabsState"),
        coverage: "exports / gallery / runtime smoke",
    },
    ComponentCatalogEntry {
        name: "ScrollArea",
        status: ComponentCatalogStatus::Official,
        family: "layout",
        state: Some("ScrollAreaState"),
        coverage: "exports / gallery / redraw smoke",
    },
    ComponentCatalogEntry {
        name: "Splitter",
        status: ComponentCatalogStatus::Official,
        family: "layout",
        state: Some("SplitterState"),
        coverage: "exports / gallery / drag smoke",
    },
    ComponentCatalogEntry {
        name: "TextInputController",
        status: ComponentCatalogStatus::AdapterOnly,
        family: "form-adapter",
        state: None,
        coverage: "gpui_adapter export / controller tests",
    },
    ComponentCatalogEntry {
        name: "ToolbarItem",
        status: ComponentCatalogStatus::InternalAnatomy,
        family: "shell",
        state: None,
        coverage: "Toolbar anatomy",
    },
    ComponentCatalogEntry {
        name: "SidebarItem",
        status: ComponentCatalogStatus::InternalAnatomy,
        family: "shell",
        state: None,
        coverage: "Sidebar anatomy",
    },
    ComponentCatalogEntry {
        name: "ListboxOption",
        status: ComponentCatalogStatus::InternalAnatomy,
        family: "choice",
        state: None,
        coverage: "Listbox anatomy",
    },
    ComponentCatalogEntry {
        name: "Separator",
        status: ComponentCatalogStatus::Official,
        family: "layout",
        state: Some("SeparatorState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Kbd",
        status: ComponentCatalogStatus::Official,
        family: "display",
        state: Some("KbdState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Progress",
        status: ComponentCatalogStatus::Official,
        family: "status",
        state: Some("ProgressState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Skeleton",
        status: ComponentCatalogStatus::Official,
        family: "status",
        state: Some("SkeletonState"),
        coverage: "exports / gallery / state tests",
    },
    ComponentCatalogEntry {
        name: "Avatar",
        status: ComponentCatalogStatus::Official,
        family: "identity",
        state: Some("AvatarState"),
        coverage: "exports / gallery / state tests",
    },
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

/// One separator sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Semantic orientation.
    pub orientation: Orientation,
    /// Whether the separator is decorative.
    pub decorative: bool,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Resolved state.
    pub state: SeparatorState,
}

/// One keyboard shortcut sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct KbdSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible shortcut label.
    pub label: &'static str,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Resolved state.
    pub state: KbdState,
}

/// One progress sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Accessible progress label.
    pub label: &'static str,
    /// Determinate value, or `None` for indeterminate progress.
    pub value_percent: Option<f32>,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Resolved state.
    pub state: ProgressState,
}

/// One skeleton sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkeletonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Whether the skeleton uses lower emphasis.
    pub subtle: bool,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Resolved state.
    pub state: SkeletonState,
}

/// One avatar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Optional source URI metadata.
    pub source: Option<&'static str>,
    /// Explicit fallback text.
    pub fallback: Option<&'static str>,
    /// Explicit accessible label.
    pub accessible_label: &'static str,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Resolved state.
    pub state: AvatarState,
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

/// One sidebar item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarItemSample {
    /// Stable item value.
    pub value: &'static str,
    /// Visible or accessible label.
    pub label: &'static str,
    /// Icon glyph shown by the sample.
    pub icon: &'static str,
    /// Optional display-only badge text.
    pub badge: Option<&'static str>,
    /// Optional trailing action label.
    pub action_label: Option<&'static str>,
    /// Whether the item is disabled.
    pub disabled: bool,
}

/// One sidebar section sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarSectionSample {
    /// Stable section value.
    pub value: &'static str,
    /// Visible section label.
    pub label: &'static str,
    /// Navigation items in this section.
    pub items: Vec<SidebarItemSample>,
}

/// One sidebar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Sidebar side.
    pub side: SidebarSide,
    /// Visual variant.
    pub variant: SidebarVariant,
    /// Collapse mode.
    pub collapse_mode: SidebarCollapseMode,
    /// Collapsed state.
    pub collapsed: bool,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Selected item value.
    pub selected: &'static str,
    /// Seeded focused item value.
    pub focused: Option<&'static str>,
    /// Sidebar sections.
    pub sections: Vec<SidebarSectionSample>,
    /// Resolved state.
    pub state: SidebarState,
}

/// One listbox option sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxOptionSample {
    /// Stable option value.
    pub value: &'static str,
    /// Visible option label.
    pub label: &'static str,
    /// Whether the option is disabled.
    pub disabled: bool,
}

/// One listbox group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxGroupSample {
    /// Stable group value.
    pub value: &'static str,
    /// Visible group label.
    pub label: &'static str,
    /// Options in this group.
    pub options: Vec<ListboxOptionSample>,
}

/// One listbox sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Selected option value.
    pub selected: Option<&'static str>,
    /// Seeded active option value.
    pub active: Option<&'static str>,
    /// Whether the sample is disabled.
    pub disabled: bool,
    /// Standalone options.
    pub options: Vec<ListboxOptionSample>,
    /// Grouped options.
    pub groups: Vec<ListboxGroupSample>,
    /// Resolved state.
    pub state: ListboxState,
}

/// One select sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Placeholder text.
    pub placeholder: &'static str,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Selected option value.
    pub selected: Option<&'static str>,
    /// Whether the select is disabled.
    pub disabled: bool,
    /// Open-state ownership.
    pub open_mode: SelectOpenMode,
    /// Whether the interactive gallery control mounts its popup open.
    pub interactive_open: bool,
    /// Standalone options.
    pub options: Vec<ListboxOptionSample>,
    /// Grouped options.
    pub groups: Vec<ListboxGroupSample>,
    /// Resolved state.
    pub state: SelectState,
}

/// One combobox sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Placeholder text.
    pub placeholder: &'static str,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Query text.
    pub query: &'static str,
    /// Selected option value.
    pub selected: Option<&'static str>,
    /// Whether the combobox is disabled.
    pub disabled: bool,
    /// Open-state ownership.
    pub open_mode: ComboboxOpenMode,
    /// Whether the interactive gallery control mounts its popup open.
    pub interactive_open: bool,
    /// Standalone options.
    pub options: Vec<ListboxOptionSample>,
    /// Grouped options.
    pub groups: Vec<ListboxGroupSample>,
    /// Resolved state.
    pub state: ComboboxState,
}

/// One command item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandItemSample {
    /// Stable command value.
    pub value: &'static str,
    /// Visible command label.
    pub label: &'static str,
    /// Optional shortcut display.
    pub shortcut: Option<&'static str>,
    /// Whether the command is disabled.
    pub disabled: bool,
}

/// One command group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandGroupSample {
    /// Stable group value.
    pub value: &'static str,
    /// Visible group label.
    pub label: &'static str,
    /// Commands in this group.
    pub items: Vec<CommandItemSample>,
}

/// One command palette sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Placeholder text.
    pub placeholder: &'static str,
    /// Foundation size used by the sample.
    pub size: Size,
    /// Query text.
    pub query: &'static str,
    /// Selected command value.
    pub selected: Option<&'static str>,
    /// Whether the command surface is disabled.
    pub disabled: bool,
    /// Open-state ownership.
    pub open_mode: CommandOpenMode,
    /// Whether the interactive gallery control mounts its popup open.
    pub interactive_open: bool,
    /// Whether the surface models command dialog policy.
    pub dialog: bool,
    /// Standalone command items.
    pub items: Vec<CommandItemSample>,
    /// Grouped command items.
    pub groups: Vec<CommandGroupSample>,
    /// Resolved state.
    pub state: CommandState,
}

macro_rules! impl_component_sample_selectors {
    ($ty:ident, $selector_family:literal) => {
        impl $ty {
            /// Returns the stable debug selector used by the gallery shell and tests.
            pub fn debug_selector(&self) -> String {
                format!("gallery:{}:{}", $selector_family, self.id)
            }
        }
    };
}

impl_component_sample_selectors!(ButtonSample, "component-button-sample");
impl_component_sample_selectors!(BadgeSample, "component-badge-sample");
impl_component_sample_selectors!(IconButtonSample, "component-icon-button-sample");
impl_component_sample_selectors!(SwitchSample, "component-switch-sample");
impl_component_sample_selectors!(CheckboxSample, "component-checkbox-sample");
impl_component_sample_selectors!(RadioGroupSample, "component-radio-sample");
impl_component_sample_selectors!(ToggleSample, "component-toggle-sample");
impl_component_sample_selectors!(ToolbarSample, "component-toolbar-sample");
impl_component_sample_selectors!(SidebarSample, "component-sidebar-sample");
impl_component_sample_selectors!(ListboxSample, "component-listbox-sample");
impl_component_sample_selectors!(SelectSample, "component-select-sample");
impl_component_sample_selectors!(ComboboxSample, "component-combobox-sample");
impl_component_sample_selectors!(CommandSample, "component-command-sample");
impl_component_sample_selectors!(LabelSample, "component-label-sample");
impl_component_sample_selectors!(TextInputSample, "component-text-input-sample");
impl_component_sample_selectors!(FieldSample, "component-field-sample");
impl_component_sample_selectors!(TabsSample, "component-tabs-sample");
impl_component_sample_selectors!(ScrollAreaSample, "component-scroll-area-sample");
impl_component_sample_selectors!(SplitterSample, "component-splitter-sample");
impl_component_sample_selectors!(SeparatorSample, "component-separator-sample");
impl_component_sample_selectors!(KbdSample, "component-kbd-sample");
impl_component_sample_selectors!(ProgressSample, "component-progress-sample");
impl_component_sample_selectors!(SkeletonSample, "component-skeleton-sample");
impl_component_sample_selectors!(AvatarSample, "component-avatar-sample");

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

/// Returns separator samples backed by real component state.
pub fn separator_samples(tokens: ThemeTokens) -> [SeparatorSample; 3] {
    [
        (
            "section-rule",
            "Section rule",
            Orientation::Horizontal,
            false,
            Size::Medium,
        ),
        (
            "panel-divider",
            "Panel divider",
            Orientation::Vertical,
            false,
            Size::Large,
        ),
        (
            "decorative-rule",
            "Decorative rule",
            Orientation::Horizontal,
            true,
            Size::Small,
        ),
    ]
    .map(
        |(id, title, orientation, decorative, size)| SeparatorSample {
            id,
            title,
            orientation,
            decorative,
            size,
            state: Separator::new(id)
                .orientation(orientation)
                .decorative(decorative)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns keyboard shortcut samples backed by real component state.
pub fn kbd_samples(tokens: ThemeTokens) -> [KbdSample; 3] {
    [
        ("command-palette", "Ctrl+K", Size::Medium),
        ("save", "Ctrl+S", Size::Small),
        ("confirm", "Enter", Size::Large),
    ]
    .map(|(id, label, size)| KbdSample {
        id,
        label,
        size,
        state: Kbd::new(id, label).with_size(size).tokens(tokens).state(),
    })
}

/// Returns progress samples backed by real component state.
pub fn progress_samples(tokens: ThemeTokens) -> [ProgressSample; 3] {
    [
        ("sync", "Sync progress", Some(64.0), Size::Medium),
        ("complete", "Complete progress", Some(100.0), Size::Large),
        ("indexing", "Indexing", None, Size::Small),
    ]
    .map(|(id, label, value_percent, size)| {
        let progress = Progress::new(id, label).with_size(size).tokens(tokens);
        let progress = match value_percent {
            Some(value) => progress.value(value),
            None => progress.indeterminate(),
        };

        ProgressSample {
            id,
            label,
            value_percent,
            size,
            state: progress.state(),
        }
    })
}

/// Returns skeleton samples backed by real component state.
pub fn skeleton_samples(tokens: ThemeTokens) -> [SkeletonSample; 3] {
    [
        ("body-line", "Body line", false, Size::Medium),
        ("compact-line", "Compact line", true, Size::Small),
        ("headline", "Headline", false, Size::Large),
    ]
    .map(|(id, title, subtle, size)| SkeletonSample {
        id,
        title,
        subtle,
        size,
        state: Skeleton::new(id)
            .subtle(subtle)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Returns avatar samples backed by real component state.
pub fn avatar_samples(tokens: ThemeTokens) -> [AvatarSample; 4] {
    [
        (
            "ada",
            "Ada Lovelace",
            None,
            None,
            "Ada Lovelace",
            Size::Medium,
        ),
        (
            "current-user",
            "Grace Hopper",
            None,
            Some("ME"),
            "Current user",
            Size::Large,
        ),
        (
            "source-user",
            "Katherine Johnson",
            Some("asset://avatars/katherine.png"),
            None,
            "Katherine profile photo",
            Size::Small,
        ),
        ("empty", "  ", None, None, "Anonymous avatar", Size::Small),
    ]
    .map(|(id, name, source, fallback, accessible_label, size)| {
        let avatar = Avatar::new(id, name)
            .accessible_label(accessible_label)
            .with_size(size)
            .tokens(tokens);
        let avatar = match source {
            Some(source) => avatar.source(source),
            None => avatar,
        };
        let avatar = match fallback {
            Some(fallback) => avatar.fallback(fallback),
            None => avatar,
        };

        AvatarSample {
            id,
            name,
            source,
            fallback,
            accessible_label,
            size,
            state: avatar.state(),
        }
    })
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
        TabsItemSample {
            value: "audit",
            label: "Audit",
            panel: "Security log retention and export controls.",
            disabled: false,
        },
        TabsItemSample {
            value: "members",
            label: "Members",
            panel: "Seat management and team invitations.",
            disabled: false,
        },
        TabsItemSample {
            value: "projects",
            label: "Projects",
            panel: "Default project templates and access.",
            disabled: false,
        },
        TabsItemSample {
            value: "automation",
            label: "Automation",
            panel: "Rules, scheduled jobs, and notification routing.",
            disabled: false,
        },
        TabsItemSample {
            value: "experiments",
            label: "Experiments",
            panel: "Feature flags and rollout controls.",
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
            body: "Resizable header keeps context visible.",
            descriptor: SplitterPanelDescriptor::new("summary", 0.32)
                .min_fraction(0.2)
                .max_fraction(0.45)
                .collapsible(true)
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

/// Returns sidebar samples backed by real component state.
pub fn sidebar_samples(tokens: ThemeTokens) -> [SidebarSample; 3] {
    let workspace_sections = vec![
        SidebarSectionSample {
            value: "workspace",
            label: "Workspace",
            items: vec![
                SidebarItemSample {
                    value: "dashboard",
                    label: "Dashboard",
                    icon: "D",
                    badge: None,
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "projects",
                    label: "Projects",
                    icon: "P",
                    badge: Some("12"),
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "inbox",
                    label: "Inbox",
                    icon: "I",
                    badge: Some("4"),
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "archive",
                    label: "Archive",
                    icon: "A",
                    badge: None,
                    action_label: None,
                    disabled: true,
                },
            ],
        },
        SidebarSectionSample {
            value: "account",
            label: "Account",
            items: vec![
                SidebarItemSample {
                    value: "settings",
                    label: "Settings",
                    icon: "S",
                    badge: None,
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "billing",
                    label: "Billing",
                    icon: "B",
                    badge: None,
                    action_label: Some("new"),
                    disabled: false,
                },
            ],
        },
    ];
    let icon_sections = vec![SidebarSectionSample {
        value: "primary",
        label: "Primary",
        items: vec![
            SidebarItemSample {
                value: "home",
                label: "Home",
                icon: "H",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "search",
                label: "Search",
                icon: "S",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "reports",
                label: "Reports",
                icon: "R",
                badge: Some("8"),
                action_label: None,
                disabled: false,
            },
        ],
    }];
    let long_sections = vec![SidebarSectionSample {
        value: "reports",
        label: "Reports",
        items: vec![
            SidebarItemSample {
                value: "overview",
                label: "Overview",
                icon: "O",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "traffic",
                label: "Traffic",
                icon: "T",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "funnels",
                label: "Funnels",
                icon: "F",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "retention",
                label: "Retention",
                icon: "R",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "quality",
                label: "Quality",
                icon: "Q",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "alerts",
                label: "Alerts",
                icon: "A",
                badge: Some("3"),
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "exports",
                label: "Exports",
                icon: "E",
                badge: None,
                action_label: None,
                disabled: true,
            },
            SidebarItemSample {
                value: "history",
                label: "History",
                icon: "H",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "forecast",
                label: "Forecast",
                icon: "F",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "segments",
                label: "Segments",
                icon: "S",
                badge: None,
                action_label: None,
                disabled: false,
            },
        ],
    }];

    [
        SidebarSample {
            id: "workspace-sidebar",
            title: "Workspace sidebar",
            summary: "Expanded docked navigation with sections, badges, and one disabled item.",
            side: SidebarSide::Left,
            variant: SidebarVariant::Docked,
            collapse_mode: SidebarCollapseMode::Icon,
            collapsed: false,
            size: Size::Medium,
            selected: "projects",
            focused: None,
            state: sidebar_state(
                SidebarSide::Left,
                SidebarVariant::Docked,
                SidebarCollapseMode::Icon,
                false,
                Size::Medium,
                "Workspace navigation",
                "projects",
                None,
                &workspace_sections,
                tokens,
            ),
            sections: workspace_sections,
        },
        SidebarSample {
            id: "icon-sidebar",
            title: "Icon rail",
            summary: "Icon collapse hides visible text while preserving explicit item labels.",
            side: SidebarSide::Left,
            variant: SidebarVariant::Inset,
            collapse_mode: SidebarCollapseMode::Icon,
            collapsed: true,
            size: Size::Small,
            selected: "reports",
            focused: Some("reports"),
            state: sidebar_state(
                SidebarSide::Left,
                SidebarVariant::Inset,
                SidebarCollapseMode::Icon,
                true,
                Size::Small,
                "Icon navigation",
                "reports",
                Some("reports"),
                &icon_sections,
                tokens,
            ),
            sections: icon_sections,
        },
        SidebarSample {
            id: "long-sidebar",
            title: "Scrollable reports",
            summary: "Constrained long navigation remains scrollable and skips disabled items.",
            side: SidebarSide::Right,
            variant: SidebarVariant::Floating,
            collapse_mode: SidebarCollapseMode::None,
            collapsed: false,
            size: Size::Small,
            selected: "alerts",
            focused: Some("quality"),
            state: sidebar_state(
                SidebarSide::Right,
                SidebarVariant::Floating,
                SidebarCollapseMode::None,
                false,
                Size::Small,
                "Reports navigation",
                "alerts",
                Some("quality"),
                &long_sections,
                tokens,
            ),
            sections: long_sections,
        },
    ]
}

/// Returns listbox samples backed by real component state.
pub fn listbox_samples(tokens: ThemeTokens) -> [ListboxSample; 2] {
    let assigned_options = vec![
        ListboxOptionSample {
            value: "unassigned",
            label: "Unassigned",
            disabled: false,
        },
        ListboxOptionSample {
            value: "separator",
            label: "",
            disabled: true,
        },
    ];
    let assigned_groups = vec![
        ListboxGroupSample {
            value: "core",
            label: "Core team",
            options: vec![
                ListboxOptionSample {
                    value: "maya",
                    label: "Maya Chen",
                    disabled: false,
                },
                ListboxOptionSample {
                    value: "owen",
                    label: "Owen Patel",
                    disabled: false,
                },
                ListboxOptionSample {
                    value: "li",
                    label: "Li Wei",
                    disabled: true,
                },
            ],
        },
        ListboxGroupSample {
            value: "support",
            label: "Support",
            options: vec![
                ListboxOptionSample {
                    value: "nora",
                    label: "Nora Lee",
                    disabled: false,
                },
                ListboxOptionSample {
                    value: "sam",
                    label: "Sam Rivera",
                    disabled: false,
                },
            ],
        },
    ];
    let empty_options = Vec::new();
    let empty_groups = Vec::new();

    [
        ListboxSample {
            id: "assignee-listbox",
            title: "Assignee",
            summary: "Grouped listbox with one disabled option and roving active metadata.",
            size: Size::Medium,
            selected: Some("owen"),
            active: Some("maya"),
            disabled: false,
            state: listbox_state(
                Size::Medium,
                false,
                "Assignee",
                Some("owen"),
                Some("maya"),
                &assigned_options,
                &assigned_groups,
                tokens,
            ),
            options: assigned_options,
            groups: assigned_groups,
        },
        ListboxSample {
            id: "empty-listbox",
            title: "Empty list",
            summary: "Empty state keeps a listbox role but has no tab stop.",
            size: Size::Small,
            selected: None,
            active: None,
            disabled: false,
            state: listbox_state(
                Size::Small,
                false,
                "Empty list",
                None,
                None,
                &empty_options,
                &empty_groups,
                tokens,
            ),
            options: empty_options,
            groups: empty_groups,
        },
    ]
}

/// Returns select samples backed by real component state.
pub fn select_samples(tokens: ThemeTokens) -> [SelectSample; 3] {
    let priority_options = vec![
        ListboxOptionSample {
            value: "low",
            label: "Low",
            disabled: false,
        },
        ListboxOptionSample {
            value: "normal",
            label: "Normal",
            disabled: false,
        },
        ListboxOptionSample {
            value: "blocked",
            label: "Blocked",
            disabled: true,
        },
    ];
    let priority_groups = vec![ListboxGroupSample {
        value: "urgent",
        label: "Urgent",
        options: vec![
            ListboxOptionSample {
                value: "high",
                label: "High",
                disabled: false,
            },
            ListboxOptionSample {
                value: "critical",
                label: "Critical",
                disabled: false,
            },
            ListboxOptionSample {
                value: "today",
                label: "Today",
                disabled: false,
            },
            ListboxOptionSample {
                value: "tomorrow",
                label: "Tomorrow",
                disabled: false,
            },
            ListboxOptionSample {
                value: "later",
                label: "Later",
                disabled: false,
            },
        ],
    }];
    let status_options = vec![
        ListboxOptionSample {
            value: "todo",
            label: "Todo",
            disabled: false,
        },
        ListboxOptionSample {
            value: "doing",
            label: "Doing",
            disabled: false,
        },
        ListboxOptionSample {
            value: "done",
            label: "Done",
            disabled: false,
        },
    ];
    let disabled_options = Vec::new();
    let disabled_groups = Vec::new();

    [
        SelectSample {
            id: "priority-select",
            title: "Priority",
            summary: "Open select composes non-modal overlay, listbox, and scroll metadata.",
            placeholder: "Choose priority",
            size: Size::Medium,
            selected: Some("critical"),
            disabled: false,
            open_mode: SelectOpenMode::Controlled,
            interactive_open: false,
            state: select_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Priority",
                "Choose priority",
                Some("critical"),
                &priority_options,
                &priority_groups,
                tokens,
            ),
            options: priority_options,
            groups: priority_groups,
        },
        SelectSample {
            id: "status-select",
            title: "Status",
            summary: "Closed uncontrolled select with selected trigger label.",
            placeholder: "Choose status",
            size: Size::Small,
            selected: Some("doing"),
            disabled: false,
            open_mode: SelectOpenMode::Uncontrolled,
            interactive_open: false,
            state: select_state(
                Size::Small,
                false,
                None,
                false,
                "Status",
                "Choose status",
                Some("doing"),
                &status_options,
                &[],
                tokens,
            ),
            options: status_options,
            groups: Vec::new(),
        },
        SelectSample {
            id: "disabled-select",
            title: "Disabled",
            summary: "Disabled empty select suppresses popup presence and activation.",
            placeholder: "Unavailable",
            size: Size::Small,
            selected: None,
            disabled: true,
            open_mode: SelectOpenMode::Uncontrolled,
            interactive_open: false,
            state: select_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled",
                "Unavailable",
                None,
                &disabled_options,
                &disabled_groups,
                tokens,
            ),
            options: disabled_options,
            groups: disabled_groups,
        },
    ]
}

/// Returns combobox samples backed by real component state.
pub fn combobox_samples(tokens: ThemeTokens) -> [ComboboxSample; 3] {
    let framework_options = vec![
        ListboxOptionSample {
            value: "react",
            label: "React",
            disabled: false,
        },
        ListboxOptionSample {
            value: "solid",
            label: "Solid",
            disabled: false,
        },
        ListboxOptionSample {
            value: "ember",
            label: "Ember",
            disabled: true,
        },
    ];
    let framework_groups = vec![ListboxGroupSample {
        value: "meta",
        label: "Meta",
        options: vec![
            ListboxOptionSample {
                value: "remix",
                label: "Remix",
                disabled: false,
            },
            ListboxOptionSample {
                value: "relay",
                label: "Relay",
                disabled: false,
            },
        ],
    }];
    let empty_options = vec![ListboxOptionSample {
        value: "rust",
        label: "Rust",
        disabled: false,
    }];
    let disabled_options = Vec::new();

    [
        ComboboxSample {
            id: "framework-combobox",
            title: "Framework",
            summary: "Editable combobox filters grouped options while keeping listbox navigation.",
            placeholder: "Search frameworks",
            size: Size::Medium,
            query: "re",
            selected: Some("solid"),
            disabled: false,
            open_mode: ComboboxOpenMode::Controlled,
            interactive_open: false,
            state: combobox_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Framework",
                "Search frameworks",
                "re",
                Some("solid"),
                &framework_options,
                &framework_groups,
                tokens,
            ),
            options: framework_options,
            groups: framework_groups,
        },
        ComboboxSample {
            id: "empty-combobox",
            title: "Empty search",
            summary: "Filtered empty state keeps the selected value independent from query text.",
            placeholder: "Search stack",
            size: Size::Small,
            query: "zz",
            selected: None,
            disabled: false,
            open_mode: ComboboxOpenMode::Controlled,
            interactive_open: false,
            state: combobox_state(
                Size::Small,
                false,
                Some(true),
                false,
                "Empty search",
                "Search stack",
                "zz",
                None,
                &empty_options,
                &[],
                tokens,
            ),
            options: empty_options,
            groups: Vec::new(),
        },
        ComboboxSample {
            id: "disabled-combobox",
            title: "Disabled search",
            summary: "Disabled combobox preserves query metadata but suppresses popup presence.",
            placeholder: "Unavailable",
            size: Size::Small,
            query: "",
            selected: None,
            disabled: true,
            open_mode: ComboboxOpenMode::Uncontrolled,
            interactive_open: false,
            state: combobox_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled search",
                "Unavailable",
                "",
                None,
                &disabled_options,
                &[],
                tokens,
            ),
            options: disabled_options,
            groups: Vec::new(),
        },
    ]
}

/// Returns command palette samples backed by real component state.
pub fn command_samples(tokens: ThemeTokens) -> [CommandSample; 3] {
    let quick_items = vec![CommandItemSample {
        value: "open-file",
        label: "Open File",
        shortcut: Some("Ctrl+O"),
        disabled: false,
    }];
    let command_groups = vec![
        CommandGroupSample {
            value: "file",
            label: "File",
            items: vec![
                CommandItemSample {
                    value: "new-file",
                    label: "New File",
                    shortcut: Some("Ctrl+N"),
                    disabled: false,
                },
                CommandItemSample {
                    value: "close-window",
                    label: "Close Window",
                    shortcut: Some("Alt+F4"),
                    disabled: true,
                },
            ],
        },
        CommandGroupSample {
            value: "view",
            label: "View",
            items: vec![CommandItemSample {
                value: "toggle-sidebar",
                label: "Toggle Sidebar",
                shortcut: Some("Ctrl+B"),
                disabled: false,
            }],
        },
    ];
    let empty_items = vec![CommandItemSample {
        value: "save",
        label: "Save",
        shortcut: Some("Ctrl+S"),
        disabled: false,
    }];
    let disabled_items = Vec::new();

    [
        CommandSample {
            id: "workspace-command",
            title: "Workspace commands",
            summary: "Dialog-backed command palette filters groups and exposes shortcut metadata.",
            placeholder: "Type a command",
            size: Size::Medium,
            query: "file",
            selected: Some("new-file"),
            disabled: false,
            open_mode: CommandOpenMode::Controlled,
            interactive_open: false,
            dialog: true,
            state: command_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Workspace commands",
                "Type a command",
                "file",
                Some("new-file"),
                &quick_items,
                &command_groups,
                true,
                tokens,
            ),
            items: quick_items,
            groups: command_groups,
        },
        CommandSample {
            id: "empty-command",
            title: "Empty commands",
            summary: "Filtered command palette keeps empty and loading states explicit.",
            placeholder: "Search commands",
            size: Size::Small,
            query: "deploy",
            selected: None,
            disabled: false,
            open_mode: CommandOpenMode::Controlled,
            interactive_open: false,
            dialog: false,
            state: command_state(
                Size::Small,
                false,
                Some(true),
                false,
                "Empty commands",
                "Search commands",
                "deploy",
                None,
                &empty_items,
                &[],
                false,
                tokens,
            ),
            items: empty_items,
            groups: Vec::new(),
        },
        CommandSample {
            id: "disabled-command",
            title: "Disabled commands",
            summary: "Disabled command surface blocks editing and hides deferred content.",
            placeholder: "Unavailable",
            size: Size::Small,
            query: "",
            selected: None,
            disabled: true,
            open_mode: CommandOpenMode::Uncontrolled,
            interactive_open: false,
            dialog: false,
            state: command_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled commands",
                "Unavailable",
                "",
                None,
                &disabled_items,
                &[],
                false,
                tokens,
            ),
            items: disabled_items,
            groups: Vec::new(),
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

fn sidebar_state(
    side: SidebarSide,
    variant: SidebarVariant,
    collapse_mode: SidebarCollapseMode,
    collapsed: bool,
    size: Size,
    label: &str,
    selected: &str,
    focused: Option<&str>,
    sections: &[SidebarSectionSample],
    tokens: ThemeTokens,
) -> SidebarState {
    SidebarState::resolve(
        side,
        variant,
        collapse_mode,
        collapsed,
        false,
        label,
        Some(selected),
        focused,
        sections.iter().map(|section| {
            SidebarSectionDescriptor::new(section.value, section.label).items(
                section.items.iter().map(|item| {
                    let mut descriptor =
                        SidebarItemDescriptor::new(item.value, item.label).icon(item.icon);
                    if let Some(badge) = item.badge {
                        descriptor = descriptor.badge(badge);
                    }
                    if let Some(action_label) = item.action_label {
                        descriptor = descriptor.action_label(action_label);
                    }
                    descriptor.disabled(item.disabled)
                }),
            )
        }),
        size,
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

fn listbox_state(
    size: Size,
    disabled: bool,
    label: &str,
    selected: Option<&str>,
    active: Option<&str>,
    options: &[ListboxOptionSample],
    groups: &[ListboxGroupSample],
    tokens: ThemeTokens,
) -> ListboxState {
    ListboxState::resolve(
        size,
        disabled,
        label,
        selected,
        active,
        None,
        "No options",
        groups.iter().map(listbox_group_descriptor),
        options.iter().map(listbox_option_descriptor),
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_state(
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    selected: Option<&str>,
    options: &[ListboxOptionSample],
    groups: &[ListboxGroupSample],
    tokens: ThemeTokens,
) -> SelectState {
    SelectState::resolve(
        size,
        disabled,
        open,
        default_open,
        label,
        placeholder,
        selected,
        selected,
        groups.iter().map(listbox_group_descriptor),
        options.iter().map(listbox_option_descriptor),
        OverlayPlacementSide::Bottom,
        OverlayPlacementAlignment::Start,
        OutsidePressPolicy::DismissAndConsume,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn combobox_state(
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selected: Option<&str>,
    options: &[ListboxOptionSample],
    groups: &[ListboxGroupSample],
    tokens: ThemeTokens,
) -> ComboboxState {
    ComboboxState::resolve(
        size,
        disabled,
        false,
        open,
        default_open,
        label,
        placeholder,
        query,
        selected,
        selected,
        "No results",
        groups.iter().map(combobox_group_descriptor),
        options.iter().map(combobox_option_descriptor),
        OverlayPlacementSide::Bottom,
        OverlayPlacementAlignment::Start,
        OutsidePressPolicy::DismissAndConsume,
        InitialFocusIntent::None,
        FocusRestoreIntent::Trigger,
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn command_state(
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selected: Option<&str>,
    items: &[CommandItemSample],
    groups: &[CommandGroupSample],
    dialog: bool,
    tokens: ThemeTokens,
) -> CommandState {
    CommandState::resolve(
        size,
        disabled,
        open,
        default_open,
        dialog,
        label,
        placeholder,
        query,
        selected,
        selected,
        (!disabled && query == "deploy")
            .then(|| open_gpui_ui_components::CommandLoadingState::new("Indexing commands", None)),
        "No results",
        dialog.then_some("Command palette".to_string()),
        dialog.then_some("Run a workspace command".to_string()),
        groups.iter().map(command_group_descriptor),
        items.iter().map(command_item_descriptor),
        OutsidePressPolicy::DismissAndConsume,
        EscapeKeyPolicy::Dismiss,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        tokens,
    )
}

fn listbox_group_descriptor(group: &ListboxGroupSample) -> ListboxGroupDescriptor {
    ListboxGroupDescriptor::new(group.value, group.label)
        .options(group.options.iter().map(listbox_option_descriptor))
}

fn listbox_option_descriptor(option: &ListboxOptionSample) -> ListboxOptionDescriptor {
    ListboxOptionDescriptor::option(option.value, option.label).disabled(option.disabled)
}

fn combobox_group_descriptor(group: &ListboxGroupSample) -> ComboboxGroupDescriptor {
    ComboboxGroupDescriptor::new(group.value, group.label)
        .options(group.options.iter().map(combobox_option_descriptor))
}

fn combobox_option_descriptor(option: &ListboxOptionSample) -> ComboboxOptionDescriptor {
    ComboboxOptionDescriptor::new(option.value, option.label).disabled(option.disabled)
}

fn command_group_descriptor(group: &CommandGroupSample) -> CommandGroupDescriptor {
    CommandGroupDescriptor::new(group.value, group.label)
        .items(group.items.iter().map(command_item_descriptor))
}

fn command_item_descriptor(item: &CommandItemSample) -> CommandItemDescriptor {
    let mut descriptor = CommandItemDescriptor::new(item.value, item.label).disabled(item.disabled);
    if let Some(shortcut) = item.shortcut {
        descriptor = descriptor.shortcut(shortcut);
    }
    descriptor
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
