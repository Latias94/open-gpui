//! Component consumer samples for the foundation gallery.

use open_gpui::{ParentElement, Styled, div, rgb};
use open_gpui_ui_components::{
    Avatar, AvatarState, Badge, BadgeState, BadgeVariant, Button, ButtonState, ButtonVariant,
    Checkbox, CheckboxState, ComboboxGroupDescriptor, ComboboxOptionDescriptor, ComboboxState,
    CommandGroupDescriptor, CommandItemDescriptor, CommandLoadingState, CommandState, EmptyState,
    EmptyStateState, FeedbackIntent, Field, FieldState, IconButton, IconButtonState, Kbd, KbdState,
    Label, LabelState, ListboxGroupDescriptor, ListboxOptionDescriptor, ListboxState, Progress,
    ProgressState, RadioGroupState, RadioItemDescriptor, ScrollAreaAxis, ScrollAreaState,
    ScrollResetPolicy, SelectState, Separator, SeparatorState, SidebarCollapseMode,
    SidebarItemDescriptor, SidebarSectionDescriptor, SidebarSide, SidebarState, SidebarVariant,
    Skeleton, SkeletonState, SplitterPanelDescriptor, SplitterState, StatusCue, StatusCueState,
    Switch, SwitchState, Table, TableColumn, TableFilter, TablePagination, TableRenderPlan,
    TableRow, TableSort, TableState, Tabs, TabsActivationMode, TabsItem, TabsItemDescriptor,
    TabsState, TextInput, TextInputState, Toggle, ToggleState, ToggleVariant, Toolbar, ToolbarItem,
    ToolbarItemDescriptor, ToolbarItemKind, ToolbarState, TreeItemDescriptor, TreeState,
    VirtualizedListScrollStrategy, VirtualizedListState,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, Orientation, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Sizable, Size, ThemeTokens, UiPx, ui_px,
};
use std::sync::LazyLock;

#[path = "components/render.rs"]
mod render;

pub(crate) use render::{
    ComponentPageAnchors, render_components_directory, render_components_page,
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
    "open_gpui_ui_foundation_gallery::pages::components::state_contract_readout_pairs",
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
    "open_gpui_ui_components::StatusCue",
    "open_gpui_ui_components::StatusCueState",
    "open_gpui_ui_components::EmptyState",
    "open_gpui_ui_components::EmptyStateState",
    "open_gpui_ui_components::FeedbackIntent",
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
    "open_gpui_ui_components::Table",
    "open_gpui_ui_components::TableState",
    "open_gpui_ui_components::TableHeaderAction",
    "open_gpui_ui_components::VirtualizerState",
    "open_gpui_ui_components::TreeState",
    "open_gpui_ui_components::TreeItemDescriptor",
    "open_gpui_ui_components::TreeItemState",
    "open_gpui_ui_components::TreeSelection",
    "open_gpui_ui_components::TreeToggle",
    "open_gpui_ui_components::TreeFocusTarget",
    "open_gpui_ui_components::TreeKeyboardAction",
    "open_gpui_ui_components::tree_navigation_target",
    "open_gpui_ui_components::VirtualizedListState",
    "open_gpui_ui_components::VirtualizedListActivation",
    "open_gpui_ui_components::VirtualizedListMetrics",
    "open_gpui_ui_components::VirtualizedListScrollStrategy",
    "open_gpui_ui_components::virtualized_list_navigation_target",
    "ThemeTokens",
    "Size",
    "Role::Button",
    "Role::Image",
    "Role::Label",
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
    "Role::Table",
    "Role::Row",
    "Role::ColumnHeader",
    "Role::Cell",
];

/// Stable jump targets for the Components page navigator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentPageJump {
    /// Stable jump id used by the page directory and section anchors.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
}

/// Page jump targets matching the Components page section order.
pub const COMPONENT_PAGE_JUMPS: &[ComponentPageJump] = &[
    ComponentPageJump {
        id: "catalog",
        label: "Component catalog",
    },
    ComponentPageJump {
        id: "primitives",
        label: "Primitives",
    },
    ComponentPageJump {
        id: "feedback",
        label: "Feedback",
    },
    ComponentPageJump {
        id: "state-contracts",
        label: "State contracts",
    },
    ComponentPageJump {
        id: "gates",
        label: "Conformance gates",
    },
    ComponentPageJump {
        id: "sidebar",
        label: "Sidebar",
    },
    ComponentPageJump {
        id: "toolbar",
        label: "Toolbar",
    },
    ComponentPageJump {
        id: "listbox",
        label: "Listbox",
    },
    ComponentPageJump {
        id: "select",
        label: "Select",
    },
    ComponentPageJump {
        id: "combobox",
        label: "Combobox",
    },
    ComponentPageJump {
        id: "command",
        label: "Command",
    },
    ComponentPageJump {
        id: "button",
        label: "Button",
    },
    ComponentPageJump {
        id: "splitter",
        label: "Splitter",
    },
    ComponentPageJump {
        id: "scroll-area",
        label: "ScrollArea",
    },
    ComponentPageJump {
        id: "badge",
        label: "Badge",
    },
    ComponentPageJump {
        id: "switch",
        label: "Switch",
    },
    ComponentPageJump {
        id: "checkbox",
        label: "Checkbox",
    },
    ComponentPageJump {
        id: "radio-group",
        label: "RadioGroup",
    },
    ComponentPageJump {
        id: "toggle",
        label: "Toggle",
    },
    ComponentPageJump {
        id: "icon-button",
        label: "IconButton",
    },
    ComponentPageJump {
        id: "label",
        label: "Label",
    },
    ComponentPageJump {
        id: "text-input",
        label: "TextInput",
    },
    ComponentPageJump {
        id: "field",
        label: "Field",
    },
    ComponentPageJump {
        id: "tabs",
        label: "Tabs",
    },
    ComponentPageJump {
        id: "table",
        label: "Table",
    },
    ComponentPageJump {
        id: "signals",
        label: "Signals",
    },
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
    /// Public renderer-neutral state contract that does not yet have an official renderer.
    StateContract,
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
            Self::StateContract => "state-contract",
            Self::Deferred => "deferred",
        }
    }

    /// Pill colors used by the gallery to render the catalog status badge.
    pub const fn badge_colors(self) -> (u32, u32, u32) {
        match self {
            Self::Official => (0xe8f3ef, 0x9ccdbd, 0x1f5f4d),
            Self::AdapterOnly => (0xf4f1ea, 0xd9c7a8, 0x6a512b),
            Self::InternalAnatomy => (0xf2f4f8, 0xc6cfdd, 0x475569),
            Self::StateContract => (0xeaf3fb, 0xa8c7df, 0x28516a),
            Self::Deferred => (0xf7f7f2, 0xd6d8ce, 0x5a6472),
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
    /// Stable rendered sample selector for official catalog entries.
    pub sample_selector: Option<&'static str>,
    /// Stable gallery readout selector for renderer-neutral state contracts.
    pub state_contract_selector: Option<&'static str>,
}

impl ComponentCatalogEntry {
    /// Creates an official catalog entry with a stable sample selector.
    pub const fn official(
        name: &'static str,
        family: &'static str,
        state: &'static str,
        coverage: &'static str,
        sample_selector: &'static str,
    ) -> Self {
        Self {
            name,
            status: ComponentCatalogStatus::Official,
            family,
            state: Some(state),
            coverage,
            sample_selector: Some(sample_selector),
            state_contract_selector: None,
        }
    }

    /// Creates a renderer-neutral state-contract catalog entry.
    pub const fn state_contract(
        name: &'static str,
        family: &'static str,
        state: &'static str,
        coverage: &'static str,
        state_contract_selector: &'static str,
    ) -> Self {
        Self {
            name,
            status: ComponentCatalogStatus::StateContract,
            family,
            state: Some(state),
            coverage,
            sample_selector: None,
            state_contract_selector: Some(state_contract_selector),
        }
    }

    /// Creates an adapter-only catalog entry.
    pub const fn adapter_only(
        name: &'static str,
        family: &'static str,
        coverage: &'static str,
    ) -> Self {
        Self {
            name,
            status: ComponentCatalogStatus::AdapterOnly,
            family,
            state: None,
            coverage,
            sample_selector: None,
            state_contract_selector: None,
        }
    }

    /// Creates an internal-anatomy catalog entry.
    pub const fn internal_anatomy(
        name: &'static str,
        family: &'static str,
        coverage: &'static str,
    ) -> Self {
        Self {
            name,
            status: ComponentCatalogStatus::InternalAnatomy,
            family,
            state: None,
            coverage,
            sample_selector: None,
            state_contract_selector: None,
        }
    }

    /// Returns the label the gallery should render for this entry's state row.
    pub const fn display_state_label(self) -> &'static str {
        match self.state {
            Some(state) => state,
            None => match self.status {
                ComponentCatalogStatus::AdapterOnly => "adapter-owned",
                ComponentCatalogStatus::InternalAnatomy => "internal-anatomy",
                ComponentCatalogStatus::StateContract => "state-contract",
                ComponentCatalogStatus::Deferred => "deferred",
                ComponentCatalogStatus::Official => "unclassified",
            },
        }
    }

    /// Returns the stable selector used for the visible catalog card.
    pub fn catalog_selector(self) -> String {
        format!("component-catalog:{}", self.name)
    }
}

/// Official component catalog and adjacent public surfaces.
pub const COMPONENT_CATALOG: &[ComponentCatalogEntry] = &[
    ComponentCatalogEntry::official(
        "Button",
        "action",
        "ButtonState",
        "exports / gallery / state tests",
        "gallery:component-button-sample:default",
    ),
    ComponentCatalogEntry::official(
        "Badge",
        "display",
        "BadgeState",
        "exports / gallery / state tests",
        "gallery:component-badge-sample:default",
    ),
    ComponentCatalogEntry::official(
        "IconButton",
        "action",
        "IconButtonState",
        "exports / gallery / a11y metadata",
        "gallery:component-icon-button-sample:search",
    ),
    ComponentCatalogEntry::official(
        "Switch",
        "form",
        "SwitchState",
        "exports / gallery / state tests",
        "gallery:component-switch-sample:off",
    ),
    ComponentCatalogEntry::official(
        "Checkbox",
        "form",
        "CheckboxState",
        "exports / gallery / state tests",
        "gallery:component-checkbox-sample:unchecked",
    ),
    ComponentCatalogEntry::official(
        "RadioGroup",
        "choice",
        "RadioGroupState",
        "exports / gallery / runtime smoke",
        "gallery:component-radio-sample:persona-radios",
    ),
    ComponentCatalogEntry::official(
        "Toggle",
        "action",
        "ToggleState",
        "exports / gallery / state tests",
        "gallery:component-toggle-sample:ghost-off",
    ),
    ComponentCatalogEntry::official(
        "Toolbar",
        "shell",
        "ToolbarState",
        "exports / gallery / runtime smoke",
        "gallery:component-toolbar-sample:editor-toolbar",
    ),
    ComponentCatalogEntry::official(
        "Sidebar",
        "shell",
        "SidebarState",
        "exports / gallery / scroll smoke",
        "gallery:component-sidebar-sample:workspace-sidebar",
    ),
    ComponentCatalogEntry::official(
        "Listbox",
        "choice",
        "ListboxState",
        "exports / gallery / runtime smoke",
        "gallery:component-listbox-sample:assignee-listbox",
    ),
    ComponentCatalogEntry::official(
        "Select",
        "choice",
        "SelectState",
        "exports / gallery / runtime smoke",
        "gallery:component-select-sample:priority-select",
    ),
    ComponentCatalogEntry::official(
        "Combobox",
        "choice-search",
        "ComboboxState",
        "exports / gallery / runtime smoke",
        "gallery:component-combobox-sample:framework-combobox",
    ),
    ComponentCatalogEntry::official(
        "Command",
        "choice-search",
        "CommandState",
        "exports / gallery / runtime smoke",
        "gallery:component-command-sample:workspace-command",
    ),
    ComponentCatalogEntry::official(
        "Label",
        "form",
        "LabelState",
        "exports / gallery / a11y metadata",
        "gallery:component-label-sample:email",
    ),
    ComponentCatalogEntry::official(
        "TextInput",
        "form",
        "TextInputState",
        "exports / gallery / controller tests",
        "gallery:component-text-input-sample:default",
    ),
    ComponentCatalogEntry::official(
        "Field",
        "form",
        "FieldState",
        "exports / gallery / composition tests",
        "gallery:component-field-sample:email",
    ),
    ComponentCatalogEntry::official(
        "Tabs",
        "navigation",
        "TabsState",
        "exports / gallery / runtime smoke",
        "gallery:component-tabs-sample:overview-tabs",
    ),
    ComponentCatalogEntry::official(
        "ScrollArea",
        "layout",
        "ScrollAreaState",
        "exports / gallery / redraw smoke",
        "gallery:component-scroll-area-sample:activity-log",
    ),
    ComponentCatalogEntry::official(
        "Splitter",
        "layout",
        "SplitterState",
        "exports / gallery / drag smoke",
        "gallery:component-splitter-sample:workspace-split",
    ),
    ComponentCatalogEntry::official(
        "Table",
        "data",
        "TableState",
        "exports / gallery / virtualized scroll smoke",
        "gallery:component-table-sample:release-queue",
    ),
    ComponentCatalogEntry::official(
        "StatusCue",
        "feedback",
        "StatusCueState",
        "exports / gallery / token intents",
        "gallery:component-status-cue-sample:sync-warning",
    ),
    ComponentCatalogEntry::official(
        "EmptyState",
        "feedback",
        "EmptyStateState",
        "exports / gallery / token intents",
        "gallery:component-empty-state-sample:no-results",
    ),
    ComponentCatalogEntry::adapter_only(
        "TextInputController",
        "form-adapter",
        "gpui_adapter export / controller tests",
    ),
    ComponentCatalogEntry::internal_anatomy("ToolbarItem", "shell", "Toolbar anatomy"),
    ComponentCatalogEntry::internal_anatomy("SidebarItem", "shell", "Sidebar anatomy"),
    ComponentCatalogEntry::internal_anatomy("ListboxOption", "choice", "Listbox anatomy"),
    ComponentCatalogEntry::official(
        "Separator",
        "layout",
        "SeparatorState",
        "exports / gallery / state tests",
        "gallery:component-separator-sample:section-rule",
    ),
    ComponentCatalogEntry::official(
        "Kbd",
        "display",
        "KbdState",
        "exports / gallery / state tests",
        "gallery:component-kbd-sample:command-palette",
    ),
    ComponentCatalogEntry::official(
        "Progress",
        "status",
        "ProgressState",
        "exports / gallery / state tests",
        "gallery:component-progress-sample:sync",
    ),
    ComponentCatalogEntry::official(
        "Skeleton",
        "status",
        "SkeletonState",
        "exports / gallery / state tests",
        "gallery:component-skeleton-sample:body-line",
    ),
    ComponentCatalogEntry::official(
        "Avatar",
        "identity",
        "AvatarState",
        "exports / gallery / state tests",
        "gallery:component-avatar-sample:ada",
    ),
    ComponentCatalogEntry::state_contract(
        "TreeState",
        "hierarchy",
        "TreeState",
        "state contract / renderer deferred",
        "gallery:component-tree-state-contract:document-outline",
    ),
    ComponentCatalogEntry::state_contract(
        "VirtualizedListState",
        "data",
        "VirtualizedListState",
        "state contract / virtualizer boundary",
        "gallery:component-virtualized-list-state-contract:release-navigation",
    ),
];

/// Returns the official catalog entries that own rendered sample selectors.
pub fn official_sample_selector_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    COMPONENT_CATALOG
        .iter()
        .filter_map(|entry| entry.sample_selector.map(|selector| (entry.name, selector)))
}

/// Returns renderer-neutral state contracts that own visible gallery readouts.
pub fn state_contract_readout_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    COMPONENT_CATALOG.iter().filter_map(|entry| {
        entry
            .state_contract_selector
            .map(|selector| (entry.name, selector))
    })
}

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
        id: "table-virtualization",
        title: "Table virtualization and row identity",
        summary: "Table keeps stable row ids, header action metadata, row-model metadata, and nested scroll ownership.",
        evidence: &[
            "TableState::resolve",
            "Table::render_plan",
            "TableHeaderAction",
            "components_gallery_smoke_table_scroll_stays_inside_sample",
        ],
    },
    ComponentConformanceGate {
        id: "state-contract-readouts",
        title: "State contract readouts",
        summary: "Renderer-neutral TreeState and VirtualizedListState stay visible without counting as official renderers.",
        evidence: &[
            "state_contract_readout_pairs",
            "TreeState::keyboard_action_for_key",
            "VirtualizedListState::navigation_target",
            "components_page_state_contract_samples_expose_tree_and_virtualized_list_contracts",
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
#[derive(Debug, Clone, PartialEq)]
pub struct IconButtonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible icon glyph.
    pub icon: &'static str,
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
    /// Resolved state.
    pub state: SeparatorState,
}

/// One keyboard shortcut sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct KbdSample {
    /// Stable sample id.
    pub id: &'static str,
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
    /// Resolved state.
    pub state: SkeletonState,
}

/// One avatar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: AvatarState,
}

/// One status cue sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusCueSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Resolved state.
    pub state: StatusCueState,
}

/// One empty state sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyStateSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Resolved state.
    pub state: EmptyStateState,
}

/// One tree state-contract sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeStateContractSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Short explanation of the contract slice.
    pub summary: &'static str,
    /// Resolved renderer-neutral tree state.
    pub state: TreeState,
}

impl TreeStateContractSample {
    /// Returns the stable debug selector used by the state-contract gallery section.
    pub fn debug_selector(&self) -> String {
        format!("gallery:component-tree-state-contract:{}", self.id)
    }
}

/// One virtualized-list state-contract sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListStateContractSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Short explanation of the contract slice.
    pub summary: &'static str,
    /// Resolved renderer-neutral virtualized-list state.
    pub state: VirtualizedListState,
    /// Semantic scroll alignment a future adapter would apply when revealing the active row.
    pub scroll_strategy: VirtualizedListScrollStrategy,
}

impl VirtualizedListStateContractSample {
    /// Returns the stable debug selector used by the state-contract gallery section.
    pub fn debug_selector(&self) -> String {
        format!(
            "gallery:component-virtualized-list-state-contract:{}",
            self.id
        )
    }
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
    /// Tab items.
    pub items: Vec<TabsItemSample>,
    /// Resolved state.
    pub state: TabsState,
}

impl TabsSample {
    /// Builds a tabs widget from the sample's resolved state and item descriptors.
    pub fn build_tabs(&self, tokens: ThemeTokens) -> Tabs {
        let tabs = self.items.iter().fold(
            Tabs::new(format!("component-tabs:{}", self.id))
                .orientation(self.state.orientation())
                .activation_mode(self.state.activation_mode())
                .with_size(self.state.size())
                .tokens(tokens),
            |tabs, item| {
                tabs.item(
                    TabsItem::new(
                        format!("component-tabs-item:{}:{}", self.id, item.value),
                        item.label,
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(item.label),
                            )
                            .child(div().text_xs().text_color(rgb(0x5a6472)).child(item.panel)),
                    )
                    .disabled(item.disabled),
                )
            },
        );

        if let Some(selected) = self.state.selected_value() {
            tabs.selected(selected)
        } else {
            tabs
        }
    }
}

/// One table sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Resolved renderer-neutral table state.
    pub state: TableState,
    /// Visual size applied to the concrete table.
    pub size: Size,
    /// Fixed table body viewport used by the sample.
    pub viewport_extent: UiPx,
    /// Fixed row height used by the virtualizer.
    pub row_height: UiPx,
    /// Overscan row budget.
    pub overscan: usize,
    /// Precomputed state summary used by the gallery page.
    state_summary: TableSampleStateSummary,
}

/// Precomputed state summary for a table sample.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableSampleStateSummary {
    /// Core row count after source resolution.
    pub core_rows: usize,
    /// Filtered row count after filters apply.
    pub filtered_rows: usize,
    /// Final row count after pagination.
    pub final_rows: usize,
    /// Rendered body row count after overscan.
    pub rendered_rows: usize,
    /// Visible body row count before overscan.
    pub visible_rows: usize,
    /// Visible row range start.
    pub visible_start: usize,
    /// Visible row range end.
    pub visible_end: usize,
    /// Overscan row range start.
    pub overscan_start: usize,
    /// Overscan row range end.
    pub overscan_end: usize,
    /// Visible column count.
    pub aria_columns: usize,
    /// Accessible row count including the header row.
    pub aria_rows: usize,
    /// Selected row count in the final model.
    pub selected_rows: usize,
}

impl TableSampleStateSummary {
    fn from_plan(plan: &TableRenderPlan) -> Self {
        let visible = plan.virtualizer().visible_range();
        let overscan = plan.virtualizer().overscan_range();

        Self {
            core_rows: plan.table().core_model().rows().len(),
            filtered_rows: plan.table().filtered_model().rows().len(),
            final_rows: plan.table().final_model().rows().len(),
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            visible_start: visible.start(),
            visible_end: visible.end(),
            overscan_start: overscan.start(),
            overscan_end: overscan.end(),
            aria_columns: plan.aria_column_count(),
            aria_rows: plan.aria_row_count(),
            selected_rows: plan.table().final_model().selected_count(),
        }
    }
}

impl TableSample {
    /// Builds the concrete GPUI table for this sample.
    pub fn build_table(&self) -> Table {
        Table::new(
            format!("component-table:{}", self.id),
            self.title,
            self.state.clone(),
        )
        .with_size(self.size)
        .viewport_extent(self.viewport_extent)
        .row_height(self.row_height)
        .overscan(self.overscan)
    }

    /// Resolves the table plan used by gallery tests and state rows.
    pub fn render_plan(&self) -> TableRenderPlan {
        self.build_table()
            .render_plan(UiPx::ZERO, self.viewport_extent)
    }

    /// Returns the precomputed state summary used by the gallery page.
    pub const fn state_summary(&self) -> TableSampleStateSummary {
        self.state_summary
    }
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
    /// Sample summary.
    pub summary: &'static str,
    /// Toolbar items.
    pub items: Vec<ToolbarItemSample>,
    /// Resolved state.
    pub state: ToolbarState,
}

impl ToolbarSample {
    /// Builds a toolbar widget from the sample's resolved state and item descriptors.
    pub fn build_toolbar(&self, tokens: ThemeTokens) -> Toolbar {
        let mut toolbar = Toolbar::new(
            format!("component-toolbar:{}", self.id),
            self.state.label().to_string(),
        )
        .orientation(self.state.orientation())
        .with_size(self.state.size())
        .tokens(tokens);

        if let Some(focused) = self.state.focused_value() {
            toolbar = toolbar.focused(focused);
        }

        for item in &self.items {
            let toolbar_item = match item.kind {
                ToolbarItemKind::Action => match item.icon {
                    Some(icon) => ToolbarItem::icon(item.value, icon, item.label),
                    None => ToolbarItem::action(item.value, item.label),
                },
                ToolbarItemKind::Toggle => match item.icon {
                    Some(icon) => ToolbarItem::toggle_icon(item.value, icon, item.label),
                    None => ToolbarItem::toggle(item.value, item.label),
                }
                .pressed(item.pressed),
                ToolbarItemKind::Separator => ToolbarItem::separator(item.value),
            }
            .disabled(item.disabled);
            toolbar = toolbar.item(toolbar_item);
        }

        toolbar
    }
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
    /// Sample summary.
    pub summary: &'static str,
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
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: ListboxState,
}

/// One select sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: SelectState,
}

/// One combobox sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: ComboboxState,
}

/// One command palette sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
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
impl_component_sample_selectors!(TableSample, "component-table-sample");
impl_component_sample_selectors!(ScrollAreaSample, "component-scroll-area-sample");
impl_component_sample_selectors!(SplitterSample, "component-splitter-sample");
impl_component_sample_selectors!(SeparatorSample, "component-separator-sample");
impl_component_sample_selectors!(KbdSample, "component-kbd-sample");
impl_component_sample_selectors!(ProgressSample, "component-progress-sample");
impl_component_sample_selectors!(SkeletonSample, "component-skeleton-sample");
impl_component_sample_selectors!(AvatarSample, "component-avatar-sample");
impl_component_sample_selectors!(StatusCueSample, "component-status-cue-sample");
impl_component_sample_selectors!(EmptyStateSample, "component-empty-state-sample");

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
            state: avatar.state(),
        }
    })
}

/// Returns status cue samples backed by real component state.
pub fn status_cue_samples(tokens: ThemeTokens) -> [StatusCueSample; 3] {
    [
        (
            "sync-warning",
            "Sync warning",
            "3 anchors need review",
            FeedbackIntent::Warning,
            Size::Small,
        ),
        (
            "healthy",
            "Healthy",
            "All queues clear",
            FeedbackIntent::Success,
            Size::Medium,
        ),
        (
            "indexing",
            "Indexing",
            "Indexing workspace",
            FeedbackIntent::Info,
            Size::Medium,
        ),
    ]
    .map(|(id, title, label, intent, size)| StatusCueSample {
        id,
        title,
        state: StatusCue::new(id, label)
            .intent(intent)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Returns empty-state samples backed by real component state.
pub fn empty_state_samples(tokens: ThemeTokens) -> [EmptyStateSample; 2] {
    [
        (
            "no-results",
            "No results",
            "No matching releases",
            Some("Adjust filters or clear the current query."),
            FeedbackIntent::Neutral,
            Size::Medium,
        ),
        (
            "blocked",
            "Blocked",
            "Queue blocked",
            Some("Resolve failing checks before merging the next item."),
            FeedbackIntent::Danger,
            Size::Small,
        ),
    ]
    .map(
        |(id, title, state_title, description, intent, size)| EmptyStateSample {
            id,
            title,
            state: {
                let empty_state = EmptyState::new(id, state_title)
                    .intent(intent)
                    .with_size(size)
                    .tokens(tokens);
                match description {
                    Some(description) => empty_state.description(description).state(),
                    None => empty_state.state(),
                }
            },
        },
    )
}

/// Returns tree state-contract samples for renderer follow-up review.
pub fn tree_state_contract_samples() -> [TreeStateContractSample; 1] {
    [TreeStateContractSample {
        id: "document-outline",
        title: "Document outline",
        summary: "Visible flattening, disabled-row skipping, and APG-style keyboard actions.",
        state: TreeState::resolve(
            Size::Medium,
            "Document outline",
            Some("intro"),
            Some("figures"),
            document_outline_tree_items(),
        ),
    }]
}

fn document_outline_tree_items() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("paper", "Paper")
            .expanded(true)
            .child(TreeItemDescriptor::new("intro", "Introduction"))
            .child(
                TreeItemDescriptor::new("figures", "Figures")
                    .expanded(false)
                    .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
            ),
        TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
        TreeItemDescriptor::new("notes", "Notes"),
    ]
}

/// Returns virtualized-list state-contract samples for renderer follow-up review.
pub fn virtualized_list_state_contract_samples() -> [VirtualizedListStateContractSample; 1] {
    [VirtualizedListStateContractSample {
        id: "release-navigation",
        title: "Release navigation",
        summary: "Long-list active descendant navigation without duplicating virtualizer range math.",
        state: VirtualizedListState::resolve(
            Size::Small,
            false,
            10_000,
            Some(42),
            Some(40),
            Some(12),
        ),
        scroll_strategy: VirtualizedListScrollStrategy::Center,
    }]
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
            let state = TextInputState::resolve(
                value,
                Some(placeholder),
                size,
                disabled,
                read_only,
                invalid,
                required,
                controller_driven,
                tokens,
            );

            TextInputSample { id, label, state }
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

static TABLE_SAMPLES: LazyLock<[TableSample; 2]> = LazyLock::new(build_table_samples);

/// Returns table samples backed by real table and virtualizer contracts.
pub fn table_samples(_tokens: ThemeTokens) -> &'static [TableSample] {
    TABLE_SAMPLES.as_slice()
}

fn build_table_samples() -> [TableSample; 2] {
    let release_queue_rows = (0..10_000).map(release_queue_row).collect::<Vec<_>>();
    let filter_board_rows = (0..180).map(filter_board_row).collect::<Vec<_>>();

    let release_queue = TableSample {
        id: "release-queue",
        title: "Release queue",
        summary: "Ten thousand stable rows sorted by score with a local virtualized viewport.",
        badge: "10k rows",
        state: TableState::new(release_queue_rows)
            .with_columns(table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["release-queue-row-0005"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 5,
        state_summary: TableSampleStateSummary::default(),
    };
    let filter_board = TableSample {
        id: "filter-board",
        title: "Filtered board",
        summary: "Filtered, sorted, and paginated rows keep selection tied to row ids.",
        badge: "filtered",
        state: TableState::new(filter_board_rows)
            .with_columns(table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_filters([TableFilter::contains("team", "UI")])
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["filter-board-row-177"])
            .with_pagination(TablePagination::new(0, 24)),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };

    [
        release_queue.with_state_summary(),
        filter_board.with_state_summary(),
    ]
}

impl TableSample {
    fn with_state_summary(self) -> Self {
        let plan = self
            .build_table()
            .render_plan(UiPx::ZERO, self.viewport_extent);
        Self {
            state_summary: TableSampleStateSummary::from_plan(&plan),
            ..self
        }
    }
}

fn table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ]
}

fn release_queue_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let score = 10_000_usize.saturating_sub(index);

    TableRow::new(format!("release-queue-row-{index:04}"))
        .with_cell("name", format!("Release #{index:04}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 7) % statuses.len()])
        .with_cell("score", score)
}

fn filter_board_row(index: usize) -> TableRow {
    let team = if index.is_multiple_of(3) {
        "UI"
    } else if index.is_multiple_of(2) {
        "Platform"
    } else {
        "Runtime"
    };
    let statuses = ["Todo", "Doing", "Review", "Done"];

    TableRow::new(format!("filter-board-row-{index:03}"))
        .with_cell("name", format!("Board item {index:03}"))
        .with_cell("team", team)
        .with_cell("status", statuses[index % statuses.len()])
        .with_cell("score", index)
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
            state: radio_group_state(
                Orientation::Vertical,
                Size::Medium,
                false,
                true,
                "team",
                &persona_items,
                tokens,
            ),
        },
        RadioGroupSample {
            id: "region-radios",
            title: "Region",
            summary: "Horizontal group with compact sizing.",
            state: radio_group_state(
                Orientation::Horizontal,
                Size::Small,
                false,
                false,
                "europe",
                &region_items,
                tokens,
            ),
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
            summary: "Horizontal actions with separators, one disabled item, and pressed toggles.",
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
            summary: "Vertical toolbar that keeps roving focus on command buttons.",
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
            summary: "Expanded docked navigation with sections, badges, and one disabled item.",
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
        },
        SidebarSample {
            id: "icon-sidebar",
            summary: "Icon collapse hides visible text while preserving explicit item labels.",
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
        },
        SidebarSample {
            id: "long-sidebar",
            summary: "Constrained long navigation remains scrollable and skips disabled items.",
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
            summary: "Grouped listbox with one disabled option and roving active metadata.",
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
        },
        ListboxSample {
            id: "empty-listbox",
            summary: "Empty state keeps a listbox role but has no tab stop.",
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
            summary: "Open select keeps selected and active option state distinct.",
            state: select_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Priority",
                "Choose priority",
                Some("critical"),
                Some("normal"),
                &priority_options,
                &priority_groups,
                tokens,
            ),
        },
        SelectSample {
            id: "status-select",
            summary: "Closed uncontrolled select with selected trigger label.",
            state: select_state(
                Size::Small,
                false,
                None,
                false,
                "Status",
                "Choose status",
                Some("doing"),
                Some("doing"),
                &status_options,
                &[],
                tokens,
            ),
        },
        SelectSample {
            id: "disabled-select",
            summary: "Disabled empty select suppresses popup presence and activation.",
            state: select_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled",
                "Unavailable",
                None,
                None,
                &disabled_options,
                &disabled_groups,
                tokens,
            ),
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
            summary: "Editable combobox keeps selected and active state distinct while filtering grouped options.",
            state: combobox_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Framework",
                "Search frameworks",
                "re",
                Some("solid"),
                Some("react"),
                &framework_options,
                &framework_groups,
                tokens,
            ),
        },
        ComboboxSample {
            id: "empty-combobox",
            summary: "Filtered empty state keeps the selected value independent from query text.",
            state: combobox_state(
                Size::Small,
                false,
                Some(true),
                false,
                "Empty search",
                "Search stack",
                "zz",
                None,
                None,
                &empty_options,
                &[],
                tokens,
            ),
        },
        ComboboxSample {
            id: "disabled-combobox",
            summary: "Disabled combobox preserves query metadata but suppresses popup presence.",
            state: combobox_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled search",
                "Unavailable",
                "",
                None,
                None,
                &disabled_options,
                &[],
                tokens,
            ),
        },
    ]
}

/// Returns command palette samples backed by real component state.
pub fn command_samples(tokens: ThemeTokens) -> [CommandSample; 3] {
    let quick_items = vec![
        CommandItemDescriptor::new("open-file", "Open File")
            .shortcut("Ctrl+O")
            .disabled(false),
    ];
    let command_groups = vec![
        CommandGroupDescriptor::new("file", "File").items(vec![
            CommandItemDescriptor::new("new-file", "New File").shortcut("Ctrl+N"),
            CommandItemDescriptor::new("close-window", "Close Window")
                .shortcut("Alt+F4")
                .disabled(true),
        ]),
        CommandGroupDescriptor::new("view", "View").item(
            CommandItemDescriptor::new("toggle-sidebar", "Toggle Sidebar").shortcut("Ctrl+B"),
        ),
    ];
    let empty_items = vec![
        CommandItemDescriptor::new("save", "Save")
            .shortcut("Ctrl+S")
            .disabled(false),
    ];
    let disabled_items = Vec::new();
    let loading = CommandLoadingState::new("Indexing commands", None);

    [
        CommandSample {
            id: "workspace-command",
            summary: "Dialog-backed command palette keeps selected and active state distinct.",
            state: command_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Workspace commands",
                "Type a command",
                "file",
                Some("new-file"),
                Some("open-file"),
                None,
                &quick_items,
                &command_groups,
                true,
                tokens,
            ),
        },
        CommandSample {
            id: "empty-command",
            summary: "Filtered command palette keeps empty and loading states explicit.",
            state: command_state(
                Size::Small,
                false,
                Some(true),
                false,
                "Empty commands",
                "Search commands",
                "deploy",
                None,
                None,
                Some(loading.clone()),
                &empty_items,
                &[],
                false,
                tokens,
            ),
        },
        CommandSample {
            id: "disabled-command",
            summary: "Disabled command surface blocks editing and hides deferred content.",
            state: command_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled commands",
                "Unavailable",
                "",
                None,
                None,
                None,
                &disabled_items,
                &[],
                false,
                tokens,
            ),
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
    active: Option<&str>,
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
        active,
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
    active: Option<&str>,
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
        active,
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

fn command_state(
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selected: Option<&str>,
    active: Option<&str>,
    loading: Option<CommandLoadingState>,
    items: &[CommandItemDescriptor],
    groups: &[CommandGroupDescriptor],
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
        active,
        loading,
        "No results",
        dialog.then_some("Command palette".to_string()),
        dialog.then_some("Run a workspace command".to_string()),
        groups.iter().cloned(),
        items.iter().cloned(),
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
