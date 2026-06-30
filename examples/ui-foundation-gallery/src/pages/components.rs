//! Component consumer samples for the foundation gallery.

use open_gpui::{App, AppContext, BorrowAppContext, Global, ParentElement, Styled, div, rgb};
use open_gpui_ui_components::{
    Accordion, AccordionItem, AccordionMode, AccordionState, Avatar, AvatarState, Badge,
    BadgeState, BadgeVariant, Breadcrumb, BreadcrumbItemDescriptor, BreadcrumbState, Button,
    ButtonState, ButtonVariant, Checkbox, CheckboxState, Collapsible, CollapsibleState,
    ComboboxGroupDescriptor, ComboboxOptionDescriptor, ComboboxState, CommandGroupDescriptor,
    CommandIndexSnapshot, CommandIndexSnapshotMode, CommandItemDescriptor, CommandLoadingState,
    CommandQueryMode, CommandSelectionMode, CommandState, EmptyState, EmptyStateState,
    FeedbackIntent, Field, FieldState, IconButton, IconButtonState, Kbd, KbdState, Label,
    LabelState, Link, LinkState, ListboxGroupDescriptor, ListboxOptionDescriptor, ListboxState,
    NumberInput, NumberInputState, Progress, ProgressState, RadioGroupState, RadioItemDescriptor,
    ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy, SelectState, Separator, SeparatorState,
    SidebarCollapseMode, SidebarItemDescriptor, SidebarSectionDescriptor, SidebarSide,
    SidebarState, SidebarVariant, Skeleton, SkeletonState, Slider, SliderState,
    SplitterPanelDescriptor, SplitterState, StatusCue, StatusCueState, Switch, SwitchState, Table,
    TableAggregation, TableCellEditApplyOutcome, TableCellEditChange, TableCellValue, TableColumn,
    TableColumnFacets, TableColumnGroup, TableColumnId, TableColumnOrderChange, TableColumnPinning,
    TableColumnRegion, TableColumnSizing, TableColumnSizingChange, TableColumnVisibilityChange,
    TableColumnVisibilityOverrides, TableExpansionMode, TableExpansionState, TableFacetValueCount,
    TableFacetedFilterChange, TableFilter, TableGlobalFilterChange, TablePagination,
    TablePredicateFilterChange, TableRangeFilterChange, TableRenderDiagnostics, TableRow,
    TableRowActivation, TableRowChildrenLoadState, TableRowExpansionToggle, TableRowPinning,
    TableRowPinningPolicy, TableSelectOption, TableSort, TableStageMode, TableState, Tabs,
    TabsActivationMode, TabsItem, TabsItemDescriptor, TabsState, Tag, TagState, TagVariant,
    TextInput, TextInputDisplayMode, TextInputState, Textarea, TextareaState, Toast, ToastStack,
    ToastStackState, Toggle, ToggleGroup, ToggleGroupItem, ToggleGroupSelectionMode,
    ToggleGroupState, ToggleState, ToggleVariant, Toolbar, ToolbarItem, ToolbarItemDescriptor,
    ToolbarItemKind, ToolbarState, Tree, TreeItemDescriptor, TreeMove, TreeRenderPlan, TreeState,
    VirtualizedList, VirtualizedListItemDescriptor, VirtualizedListMetrics,
    VirtualizedListRenderPlan, VirtualizedListScrollStrategy, VirtualizedListState,
    apply_tree_move,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, Orientation, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Sizable, Size, ThemeTokens, UiPx, ui_px,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

#[path = "components/render.rs"]
mod render;

pub(crate) use render::{
    component_page_section_count, component_page_section_index, render_components_directory,
    render_components_page,
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
    "open_gpui_ui_foundation_gallery::pages::components::ComponentFocusMode",
    "open_gpui_ui_foundation_gallery::pages::components::state_contract_readout_pairs",
    "open_gpui_ui_components::Button",
    "open_gpui_ui_components::ButtonState",
    "open_gpui_ui_components::ButtonVariant",
    "open_gpui_ui_components::Badge",
    "open_gpui_ui_components::BadgeState",
    "open_gpui_ui_components::BadgeVariant",
    "open_gpui_ui_components::Accordion",
    "open_gpui_ui_components::AccordionState",
    "open_gpui_ui_components::Collapsible",
    "open_gpui_ui_components::CollapsibleState",
    "open_gpui_ui_components::Slider",
    "open_gpui_ui_components::SliderState",
    "open_gpui_ui_components::NumberInput",
    "open_gpui_ui_components::NumberInputState",
    "open_gpui_ui_components::ToggleGroup",
    "open_gpui_ui_components::ToggleGroupState",
    "open_gpui_ui_components::Link",
    "open_gpui_ui_components::LinkState",
    "open_gpui_ui_components::Breadcrumb",
    "open_gpui_ui_components::BreadcrumbState",
    "open_gpui_ui_components::Tag",
    "open_gpui_ui_components::TagState",
    "open_gpui_ui_components::ToastStack",
    "open_gpui_ui_components::ToastStackState",
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
    "open_gpui_ui_components::AvatarGroup",
    "open_gpui_ui_components::AvatarGroupCount",
    "open_gpui_ui_components::AvatarGroupState",
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
    "open_gpui_ui_components::TextInputDisplayMode",
    "open_gpui_ui_components::TextInputState",
    "open_gpui_ui_components::gpui_adapter::TextInputController",
    "open_gpui_ui_components::Textarea",
    "open_gpui_ui_components::TextareaState",
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
    "open_gpui_ui_components::TableAggregation",
    "open_gpui_ui_components::TableColumnFacets",
    "open_gpui_ui_components::TableFacetValueCount",
    "open_gpui_ui_components::TableFacetRange",
    "open_gpui_ui_components::TableColumnPinning",
    "open_gpui_ui_components::TableColumnRegion",
    "open_gpui_ui_components::TableColumnSizing",
    "open_gpui_ui_components::TableColumnSizingChange",
    "open_gpui_ui_components::TableColumnVisibility",
    "open_gpui_ui_components::TableColumnVisibilityAction",
    "open_gpui_ui_components::TableColumnVisibilityChange",
    "open_gpui_ui_components::TableColumnVisibilityItemState",
    "open_gpui_ui_components::TableColumnVisibilityOverrides",
    "open_gpui_ui_components::TableColumnVisibilityState",
    "open_gpui_ui_components::TableFacetedFilter",
    "open_gpui_ui_components::TableFacetedFilterChange",
    "open_gpui_ui_components::TableFacetedFilterOptionState",
    "open_gpui_ui_components::TableFacetedFilterState",
    "open_gpui_ui_components::TableGlobalFilter",
    "open_gpui_ui_components::TableGlobalFilterChange",
    "open_gpui_ui_components::TableGlobalFilterState",
    "open_gpui_ui_components::TablePredicateFilter",
    "open_gpui_ui_components::TablePredicateFilterChange",
    "open_gpui_ui_components::TablePredicateFilterOperator",
    "open_gpui_ui_components::TablePredicateFilterOperatorOptionState",
    "open_gpui_ui_components::TablePredicateFilterState",
    "open_gpui_ui_components::TableToolbar",
    "open_gpui_ui_components::TableToolbarState",
    "open_gpui_ui_components::TableRangeFilter",
    "open_gpui_ui_components::TableRangeFilterChange",
    "open_gpui_ui_components::TableRangeFilterState",
    "open_gpui_ui_components::TableColumnResizeMode",
    "open_gpui_ui_components::TableExpansionState",
    "open_gpui_ui_components::TableRowPinning",
    "open_gpui_ui_components::TableRowPinningPolicy",
    "open_gpui_ui_components::TableRowRegion",
    "open_gpui_ui_components::TableRowRegions",
    "open_gpui_ui_components::VirtualizedList",
    "open_gpui_ui_components::VirtualizedListItemDescriptor",
    "open_gpui_ui_components::VirtualizedListRenderPlan",
    "open_gpui_ui_components::VirtualizerState",
    "open_gpui_ui_components::Tree",
    "open_gpui_ui_components::TreeMetrics",
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
    "Role::Tree",
    "Role::TreeItem",
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
        id: "foundation-components",
        label: "Foundation components",
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
        id: "tree",
        label: "Tree",
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
        id: "textarea",
        label: "Textarea",
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
        id: "virtualized-list",
        label: "VirtualizedList",
    },
    ComponentPageJump {
        id: "signals",
        label: "Signals",
    },
];

/// Components page rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentFocusMode {
    /// Render the full conformance page.
    All,
    /// Render one component section plus its local metadata.
    Section(&'static str),
}

impl Default for ComponentFocusMode {
    fn default() -> Self {
        Self::All
    }
}

impl ComponentFocusMode {
    /// Creates a focus mode for a known Components page section.
    pub fn section(id: &'static str) -> Option<Self> {
        focused_section_for_id(id).map(Self::Section)
    }

    /// Returns the stable reset key used by the outer gallery page viewport.
    pub const fn reset_key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Section(id) => id,
        }
    }

    /// Returns true when the section should render for this mode.
    pub const fn shows_section(self, id: &'static str) -> bool {
        match self {
            Self::All => true,
            Self::Section(focused) => str_eq(focused, id),
        }
    }

    /// Returns the focused section id when one is active.
    pub const fn focused_section(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Section(id) => Some(id),
        }
    }
}

const fn str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }

    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }

    true
}

/// Returns a section id that can be shown in focused mode.
pub fn focused_section_for_id(id: &'static str) -> Option<&'static str> {
    COMPONENT_PAGE_JUMPS
        .iter()
        .map(|jump| jump.id)
        .find(|candidate| {
            *candidate == id
                && *candidate != "catalog"
                && *candidate != "gates"
                && *candidate != "signals"
        })
}

/// Returns the focused section represented by a catalog entry.
pub fn focused_section_for_catalog_entry(entry: &ComponentCatalogEntry) -> Option<&'static str> {
    match entry.status {
        ComponentCatalogStatus::Official | ComponentCatalogStatus::StateContract => {
            focused_section_for_id(entry.sample_section_id())
        }
        ComponentCatalogStatus::AdapterOnly
        | ComponentCatalogStatus::InternalAnatomy
        | ComponentCatalogStatus::Deferred => None,
    }
}

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

    /// Returns the Components page section that contains this entry's rendered sample or readout.
    pub fn sample_section_id(self) -> &'static str {
        match self.name {
            "StatusCue" | "EmptyState" => "feedback",
            "Accordion" | "Collapsible" | "Slider" | "NumberInput" | "ToggleGroup" | "Link"
            | "Breadcrumb" | "Tag" | "ToastStack" => "foundation-components",
            "TreeState" | "VirtualizedListState" => "state-contracts",
            "RadioGroup" => "radio-group",
            "IconButton" => "icon-button",
            "TextInput" => "text-input",
            "ScrollArea" => "scroll-area",
            "VirtualizedList" => "virtualized-list",
            "Button" => "button",
            "Badge" => "badge",
            "Switch" => "switch",
            "Checkbox" => "checkbox",
            "Toggle" => "toggle",
            "Toolbar" => "toolbar",
            "Sidebar" => "sidebar",
            "Tree" => "tree",
            "Listbox" => "listbox",
            "Select" => "select",
            "Combobox" => "combobox",
            "Command" => "command",
            "Label" => "label",
            "Field" => "field",
            "Tabs" => "tabs",
            "Splitter" => "splitter",
            "Table" => "table",
            "Separator" | "Kbd" | "Progress" | "Skeleton" | "Avatar" | "AvatarGroup" => {
                "primitives"
            }
            _ => "catalog",
        }
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
        "Accordion",
        "disclosure",
        "AccordionState",
        "exports / gallery / state tests",
        "gallery:component-accordion-sample:shipping",
    ),
    ComponentCatalogEntry::official(
        "Collapsible",
        "disclosure",
        "CollapsibleState",
        "exports / gallery / state tests",
        "gallery:component-collapsible-sample:release-notes",
    ),
    ComponentCatalogEntry::official(
        "Slider",
        "form",
        "SliderState",
        "exports / gallery / keyboard tests",
        "gallery:component-slider-sample:volume",
    ),
    ComponentCatalogEntry::official(
        "NumberInput",
        "form",
        "NumberInputState",
        "exports / gallery / stepper tests",
        "gallery:component-number-input-sample:workers",
    ),
    ComponentCatalogEntry::official(
        "ToggleGroup",
        "action",
        "ToggleGroupState",
        "exports / gallery / stable value tests",
        "gallery:component-toggle-group-sample:alignment",
    ),
    ComponentCatalogEntry::official(
        "Link",
        "navigation",
        "LinkState",
        "exports / gallery / activation tests",
        "gallery:component-link-sample:docs",
    ),
    ComponentCatalogEntry::official(
        "Breadcrumb",
        "navigation",
        "BreadcrumbState",
        "exports / gallery / activation tests",
        "gallery:component-breadcrumb-sample:project",
    ),
    ComponentCatalogEntry::official(
        "Tag",
        "display",
        "TagState",
        "exports / gallery / remove tests",
        "gallery:component-tag-sample:ready",
    ),
    ComponentCatalogEntry::official(
        "ToastStack",
        "feedback",
        "ToastStackState",
        "exports / gallery / stack tests",
        "gallery:component-toast-stack-sample:notifications",
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
        "Tree",
        "hierarchy",
        "TreeState",
        "exports / gallery / tree runtime smoke",
        "gallery:component-tree-sample:document-outline",
    ),
    ComponentCatalogEntry::official(
        "Listbox",
        "choice",
        "ListboxState",
        "exports / gallery / shared navigation smoke",
        "gallery:component-listbox-sample:assignee-listbox",
    ),
    ComponentCatalogEntry::official(
        "Select",
        "choice",
        "SelectState",
        "exports / gallery / stable value smoke",
        "gallery:component-select-sample:priority-select",
    ),
    ComponentCatalogEntry::official(
        "Combobox",
        "choice-search",
        "ComboboxState",
        "exports / gallery / stable value smoke",
        "gallery:component-combobox-sample:framework-combobox",
    ),
    ComponentCatalogEntry::official(
        "Command",
        "choice-search",
        "CommandState",
        "exports / gallery / stable value and runtime smoke",
        "gallery:component-command-sample:ranked-search",
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
        "Textarea",
        "form",
        "TextareaState",
        "exports / gallery / controlled multiline tests",
        "gallery:component-textarea-sample:default",
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
        "exports / gallery / virtualized scroll and resize smoke",
        "gallery:component-table-sample:release-queue",
    ),
    ComponentCatalogEntry::official(
        "VirtualizedList",
        "data",
        "VirtualizedListState",
        "exports / gallery / virtualized scroll smoke",
        "gallery:component-virtualized-list-sample:release-navigation",
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
    ComponentCatalogEntry::official(
        "AvatarGroup",
        "identity",
        "AvatarGroupState",
        "exports / gallery / state tests",
        "gallery:component-avatar-group-sample:team",
    ),
    ComponentCatalogEntry::state_contract(
        "TreeState",
        "hierarchy",
        "TreeState",
        "state contract / renderer readout",
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
        title: "Table row models and scroll ownership",
        summary: "Table keeps stable row ids, grouped/expanded row metadata, aggregate metadata, pinned columns, pinned rows, resize handles, content-fit width growth, single-line and multiline editors, column visibility controls, and nested scroll ownership.",
        evidence: &[
            "TableState::resolve",
            "Table::render_plan",
            "TableHeaderAction",
            "release-rollup",
            "grouped-custom-aggregation",
            "release-resize",
            "content-fit-release",
            "toggle-release",
            "select-release",
            "multiline-release",
            "row-pinning",
            "filter-board",
            "TableColumnWidthPolicy",
            "TableGlobalFilter",
            "TablePredicateFilter",
            "TableFacetedFilter",
            "TableRangeFilter",
            "TableColumnVisibility",
            "TableColumnVisibilityChange",
            "TableToolbar",
            "components_gallery_smoke_global_filter_updates_table_rows",
            "components_gallery_smoke_predicate_filter_updates_table_rows",
            "components_gallery_smoke_faceted_filter_updates_table_rows",
            "components_gallery_smoke_range_filter_updates_table_rows",
            "components_gallery_smoke_content_fit_table_cell_edit_widens_name_column",
            "components_gallery_smoke_checkbox_table_cell_updates_sample_rows",
            "components_gallery_smoke_select_table_cell_updates_sample_rows",
            "components_gallery_smoke_multiline_table_cell_updates_sample_rows",
            "components_gallery_smoke_column_visibility_updates_release_matrix",
            "components_gallery_smoke_table_scroll_stays_inside_sample",
            "components_gallery_smoke_focused_table_scroll_stays_inside_sample",
            "components_gallery_smoke_grouped_table_scroll_stays_inside_sample",
            "components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample",
            "components_gallery_smoke_grouped_table_column_reorder_updates_sample",
            "components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample",
            "components_page_table_samples_expose_virtualized_row_model_contract",
            "components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample",
            "components_gallery_smoke_resizable_table_resize_updates_sample",
        ],
    },
    ComponentConformanceGate {
        id: "tree-renderer",
        title: "Tree renderer contract",
        summary: "Tree composes renderer-neutral hierarchy state with GPUI focus, expansion, selection, and local scroll ownership.",
        evidence: &[
            "Tree::state",
            "Tree::render_plan",
            "TreeRenderPlan",
            "TreeState::keyboard_action_for_key",
            "TreeState::typeahead_target",
            "tree_render_plan_virtualizes_visible_rows_with_stable_metadata",
            "tree_runtime_expands_reveals_and_selects_items",
            "tree_runtime_typeahead_focuses_visible_matching_row",
            "components_gallery_smoke_tree_expands_and_selects",
            "components_gallery_smoke_tree_lazy_branches_emit_load_metadata",
            "components_gallery_smoke_tree_card_wheel_does_not_leak_to_page",
            "components_gallery_smoke_virtualized_tree_scrolls_inside_sample",
        ],
    },
    ComponentConformanceGate {
        id: "virtualized-list-renderer",
        title: "VirtualizedList renderer contract",
        summary: "VirtualizedList keeps its state contract, row reveal logic, and inner scroll ownership aligned with the rendered adapter.",
        evidence: &[
            "VirtualizedList::render_plan",
            "VirtualizedListState::navigation_target",
            "virtualized_list_runtime_reveals_active_row_and_emits_activation",
            "components_gallery_smoke_virtualized_list_scroll_stays_inside_sample",
            "components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page",
            "components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates",
        ],
    },
    ComponentConformanceGate {
        id: "state-contract-readouts",
        title: "State contract readouts",
        summary: "Renderer-neutral TreeState and VirtualizedListState stay visible beside concrete renderers.",
        evidence: &[
            "state_contract_readout_pairs",
            "TreeState::keyboard_action_for_key",
            "VirtualizedListState::navigation_target",
            "components_page_state_contract_samples_expose_tree_and_virtualized_list_contracts",
        ],
    },
    ComponentConformanceGate {
        id: "choice-surfaces",
        title: "Choice identity and navigation",
        summary: "Choice surfaces keep stable value identity, shared listbox navigation, and focused gallery readouts aligned.",
        evidence: &[
            "choice.rs",
            "roving_focus.rs",
            "components_page_search_samples_expose_combobox_and_command_contracts",
            "component_gallery_shell_reads_choice_active_metadata_from_resolved_state",
            "components_gallery_smoke_focused_command_samples_cover_depth_behaviors",
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

/// One accordion sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AccordionSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: AccordionState,
    /// Concrete items rendered by the sample.
    pub items: Vec<AccordionItem>,
}

/// One collapsible sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CollapsibleSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: CollapsibleState,
    /// Visible content copy.
    pub content: &'static str,
}

/// One slider sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: SliderState,
}

/// One number input sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: NumberInputState,
}

/// One toggle group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToggleGroupSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: ToggleGroupState,
}

/// One link sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: LinkState,
}

/// One breadcrumb sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: BreadcrumbState,
}

/// One tag sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TagSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: TagState,
}

/// One toast stack sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastStackSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: ToastStackState,
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

/// One avatar group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarGroupSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Visible child avatars.
    pub avatars: Vec<AvatarSample>,
    /// Overflow counter label.
    pub count_label: &'static str,
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

/// One rendered tree sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Root item descriptors consumed by the concrete tree renderer.
    pub items: Vec<TreeItemDescriptor>,
    /// Resolved renderer-neutral tree state.
    pub state: TreeState,
    /// Visual size applied to the concrete tree.
    pub size: Size,
    /// Whether the concrete tree uses fixed-row virtualized rendering.
    pub virtualized: bool,
    /// Whether the concrete Tree enables pointer drag move affordances.
    pub draggable: bool,
    /// Fallback virtualized viewport item count before layout measurement.
    pub viewport_item_count: usize,
    /// Virtualized overscan item budget.
    pub overscan_count: usize,
}

/// One Tree drag move captured from the rendered gallery sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSampleMoveEvent {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Controlled Tree move payload.
    pub tree_move: TreeMove,
}

/// One selection captured from the rendered gallery `Tree` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSampleSelection {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Selected item value.
    pub value: String,
}

/// One expansion toggle captured from the rendered gallery `Tree` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSampleToggleEvent {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Toggled item value.
    pub value: String,
    /// Desired expanded state after the toggle.
    pub expanded: bool,
    /// Currently loaded child descriptor count at toggle time.
    pub loaded_child_count: usize,
    /// Stable child loading state label at toggle time.
    pub children_load_state: String,
    /// Loading or failure message at toggle time, when present.
    pub children_load_message: Option<String>,
}

/// Runtime interaction log used by gallery Tree smoke tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TreeSampleRuntimeLog {
    selections: Vec<TreeSampleSelection>,
    toggles: Vec<TreeSampleToggleEvent>,
    moves: Vec<TreeSampleMoveEvent>,
    tree_item_overrides: BTreeMap<String, Vec<TreeItemDescriptor>>,
}

impl Global for TreeSampleRuntimeLog {}

impl TreeSampleRuntimeLog {
    /// Returns captured selections in event order.
    pub fn selections(&self) -> &[TreeSampleSelection] {
        &self.selections
    }

    /// Returns captured toggles in event order.
    pub fn toggles(&self) -> &[TreeSampleToggleEvent] {
        &self.toggles
    }

    /// Returns captured move payloads in event order.
    pub fn moves(&self) -> &[TreeSampleMoveEvent] {
        &self.moves
    }

    /// Returns the current controlled item descriptors for a sample, if any.
    pub fn tree_item_override(&self, sample_id: &str) -> Option<&[TreeItemDescriptor]> {
        self.tree_item_overrides.get(sample_id).map(Vec::as_slice)
    }

    /// Clears captured interactions.
    pub fn clear(&mut self) {
        self.selections.clear();
        self.toggles.clear();
        self.moves.clear();
        self.tree_item_overrides.clear();
    }
}

/// Records a gallery `Tree` selection in app-global sample state.
pub fn record_tree_selection(sample_id: impl Into<String>, value: impl Into<String>, cx: &mut App) {
    cx.update_default_global::<TreeSampleRuntimeLog, _>(|log, _| {
        log.selections.push(TreeSampleSelection {
            sample_id: sample_id.into(),
            value: value.into(),
        });
    });
}

/// Records a gallery `Tree` expansion toggle in app-global sample state.
pub fn record_tree_toggle(
    sample_id: impl Into<String>,
    value: impl Into<String>,
    expanded: bool,
    loaded_child_count: usize,
    children_load_state: impl Into<String>,
    children_load_message: Option<String>,
    cx: &mut App,
) {
    cx.update_default_global::<TreeSampleRuntimeLog, _>(|log, _| {
        log.toggles.push(TreeSampleToggleEvent {
            sample_id: sample_id.into(),
            value: value.into(),
            expanded,
            loaded_child_count,
            children_load_state: children_load_state.into(),
            children_load_message,
        });
    });
}

/// Returns the current controlled item descriptors for a gallery `Tree` sample.
pub fn current_tree_sample_items(
    sample_id: impl Into<String>,
    fallback: &[TreeItemDescriptor],
    cx: &impl AppContext,
) -> Vec<TreeItemDescriptor> {
    let sample_id = sample_id.into();
    cx.read_global::<TreeSampleRuntimeLog, _>(|log, _| {
        log.tree_item_override(&sample_id)
            .map(|items| items.to_vec())
            .unwrap_or_else(|| fallback.to_vec())
    })
}

/// Records and applies a controlled gallery `Tree` move request.
pub fn record_tree_move(
    sample_id: impl Into<String>,
    fallback: &[TreeItemDescriptor],
    tree_move: &TreeMove,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.to_vec();
    let next = cx.read_global::<TreeSampleRuntimeLog, _>(|log, _| {
        let current = log
            .tree_item_override(&sample_id)
            .map(|items| items.to_vec())
            .unwrap_or_else(|| fallback.clone());
        apply_tree_move(current, tree_move)
    });

    if let Some(next) = next {
        cx.update_default_global::<TreeSampleRuntimeLog, _>(|log, _| {
            log.moves.push(TreeSampleMoveEvent {
                sample_id: sample_id.clone(),
                tree_move: tree_move.clone(),
            });
            log.tree_item_overrides.insert(sample_id, next);
        });
    }
}

/// One committed column sizing change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSampleSizingChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Resized column id.
    pub column_id: String,
    /// Committed resolved width.
    pub width: UiPx,
}

/// One row activation captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleRowActivation {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Activated row id.
    pub row_id: String,
    /// Concrete render key used by the adapter selectors.
    pub render_key: String,
    /// Stable activation kind label.
    pub kind: String,
    /// Final row-model index at activation time.
    pub model_index: usize,
    /// Resolved hierarchy depth at activation time.
    pub depth: usize,
    /// Whether the row is a source tree branch.
    pub tree_branch: bool,
    /// Resolved branch expansion state, when applicable.
    pub tree_expanded: Option<bool>,
    /// Whether the row was selected in caller-owned table state.
    pub selected: bool,
}

/// One source-tree expansion request captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleExpansionToggle {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Toggled row id.
    pub row_id: String,
    /// Desired expanded state after the toggle.
    pub expanded: bool,
    /// Resolved hierarchy depth at toggle time.
    pub depth: usize,
    /// Number of directly loaded child rows at toggle time.
    pub loaded_child_count: usize,
    /// Stable child loading state label at toggle time.
    pub children_load_state: String,
    /// Optional loading or failure message at toggle time.
    pub children_load_message: Option<String>,
}

/// One table-cell edit captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleCellEditChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Edited row id.
    pub row_id: String,
    /// Edited column id.
    pub column_id: String,
    /// Source-row index carried by the edit payload, when available.
    pub source_index: Option<usize>,
    /// Resolved text before the edit.
    pub previous_text: String,
    /// Next controlled text value.
    pub next_text: String,
    /// Result from applying the change to app-owned sample state.
    pub outcome: String,
}

/// Runtime interaction log used by gallery Table smoke tests.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TableSampleRuntimeLog {
    sizing_changes: Vec<TableSampleSizingChange>,
    committed_sizing: BTreeMap<String, TableColumnSizing>,
    row_activations: Vec<TableSampleRowActivation>,
    expansion_toggles: Vec<TableSampleExpansionToggle>,
    expansion_overrides: BTreeMap<String, TableExpansionState>,
    global_filter_changes: Vec<TableSampleGlobalFilterChange>,
    predicate_filter_changes: Vec<TableSamplePredicateFilterChange>,
    filter_overrides: BTreeMap<String, TableState>,
    visibility_changes: Vec<TableSampleColumnVisibilityChange>,
    visibility_overrides: BTreeMap<String, TableColumnVisibilityOverrides>,
    column_order_changes: Vec<TableSampleColumnOrderChange>,
    column_order_overrides: BTreeMap<String, Vec<TableColumnId>>,
    faceted_filter_changes: Vec<TableSampleFacetedFilterChange>,
    range_filter_changes: Vec<TableSampleRangeFilterChange>,
    cell_edit_changes: Vec<TableSampleCellEditChange>,
    cell_edit_overrides: BTreeMap<String, TableState>,
    server_tree_loaded: BTreeMap<String, bool>,
}

/// One global-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleGlobalFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filter query text.
    pub query: String,
    /// Whether this payload clears the global filter.
    pub cleared: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

/// One predicate-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSamplePredicateFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filtered column id.
    pub column_id: String,
    /// Stable operator value, when this is not a clear action.
    pub operator: Option<String>,
    /// Raw predicate value text.
    pub value: String,
    /// Whether this payload clears the predicate.
    pub cleared: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

/// One column-visibility change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleColumnVisibilityChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Stable visibility action label.
    pub action: String,
    /// Affected column ids.
    pub column_ids: Vec<String>,
    /// Next visibility for the affected columns, if the action sets one.
    pub next_visible: Option<bool>,
    /// Visible column count after the change.
    pub visible_columns: usize,
    /// Hidden column count after the change.
    pub hidden_columns: usize,
}

/// One column-order change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleColumnOrderChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Moved column id.
    pub column_id: String,
    /// Target column id.
    pub target_column_id: String,
    /// Stable insertion placement label.
    pub placement: String,
    /// Shared column region for the move.
    pub region: String,
    /// Full column order after the change.
    pub column_order: Vec<String>,
}

/// One faceted-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleFacetedFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filtered column id.
    pub column_id: String,
    /// Exact categorical tokens selected after the change.
    pub selected_values: Vec<String>,
    /// Token that was toggled, if any.
    pub toggled_value: Option<String>,
    /// Whether the toggled token is selected after the change.
    pub selected: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

/// One range-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSampleRangeFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filtered column id.
    pub column_id: String,
    /// Lower endpoint text.
    pub min_text: String,
    /// Upper endpoint text.
    pub max_text: String,
    /// Parsed lower endpoint after normalization.
    pub min_value: Option<f64>,
    /// Parsed upper endpoint after normalization.
    pub max_value: Option<f64>,
    /// Whether this payload clears the range.
    pub cleared: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

impl Global for TableSampleRuntimeLog {}

impl TableSampleRuntimeLog {
    /// Returns captured sizing changes in event order.
    pub fn sizing_changes(&self) -> &[TableSampleSizingChange] {
        &self.sizing_changes
    }

    /// Returns the latest committed sizing for a sample, if any.
    pub fn committed_sizing(&self, sample_id: &str) -> Option<&TableColumnSizing> {
        self.committed_sizing.get(sample_id)
    }

    /// Returns captured row activations in event order.
    pub fn row_activations(&self) -> &[TableSampleRowActivation] {
        &self.row_activations
    }

    /// Returns captured source-tree expansion requests in event order.
    pub fn expansion_toggles(&self) -> &[TableSampleExpansionToggle] {
        &self.expansion_toggles
    }

    /// Returns the current controlled expansion override for a sample, if any.
    pub fn expansion_override(&self, sample_id: &str) -> Option<&TableExpansionState> {
        self.expansion_overrides.get(sample_id)
    }

    /// Returns captured global-filter changes in event order.
    pub fn global_filter_changes(&self) -> &[TableSampleGlobalFilterChange] {
        &self.global_filter_changes
    }

    /// Returns the current controlled global-filter state for a sample, if any.
    pub fn global_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured predicate-filter changes in event order.
    pub fn predicate_filter_changes(&self) -> &[TableSamplePredicateFilterChange] {
        &self.predicate_filter_changes
    }

    /// Returns the current controlled predicate-filter state for a sample, if any.
    pub fn predicate_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured column-visibility changes in event order.
    pub fn visibility_changes(&self) -> &[TableSampleColumnVisibilityChange] {
        &self.visibility_changes
    }

    /// Returns the current controlled column-visibility state for a sample, if any.
    pub fn visibility_override(&self, sample_id: &str) -> Option<&TableColumnVisibilityOverrides> {
        self.visibility_overrides.get(sample_id)
    }

    /// Returns captured column-order changes in event order.
    pub fn column_order_changes(&self) -> &[TableSampleColumnOrderChange] {
        &self.column_order_changes
    }

    /// Returns the current controlled column-order state for a sample, if any.
    pub fn column_order_override(&self, sample_id: &str) -> Option<&[TableColumnId]> {
        self.column_order_overrides
            .get(sample_id)
            .map(Vec::as_slice)
    }

    /// Returns captured faceted filter changes in event order.
    pub fn faceted_filter_changes(&self) -> &[TableSampleFacetedFilterChange] {
        &self.faceted_filter_changes
    }

    /// Returns the current controlled faceted filter state for a sample, if any.
    pub fn faceted_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured range filter changes in event order.
    pub fn range_filter_changes(&self) -> &[TableSampleRangeFilterChange] {
        &self.range_filter_changes
    }

    /// Returns the current controlled range filter state for a sample, if any.
    pub fn range_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured text-cell edits in event order.
    pub fn cell_edit_changes(&self) -> &[TableSampleCellEditChange] {
        &self.cell_edit_changes
    }

    /// Returns the current controlled cell-edit state for a sample, if any.
    pub fn cell_edit_override(&self, sample_id: &str) -> Option<&TableState> {
        self.cell_edit_overrides.get(sample_id)
    }

    /// Clears captured interactions.
    pub fn clear(&mut self) {
        self.sizing_changes.clear();
        self.committed_sizing.clear();
        self.row_activations.clear();
        self.expansion_toggles.clear();
        self.expansion_overrides.clear();
        self.global_filter_changes.clear();
        self.predicate_filter_changes.clear();
        self.filter_overrides.clear();
        self.visibility_changes.clear();
        self.visibility_overrides.clear();
        self.column_order_changes.clear();
        self.column_order_overrides.clear();
        self.faceted_filter_changes.clear();
        self.range_filter_changes.clear();
        self.cell_edit_changes.clear();
        self.cell_edit_overrides.clear();
        self.server_tree_loaded.clear();
    }
}

/// Returns the current committed sizing for a gallery `Table` sample.
pub fn current_table_sample_sizing(
    sample_id: impl Into<String>,
    fallback: &TableColumnSizing,
    cx: &impl AppContext,
) -> TableColumnSizing {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.committed_sizing
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled expansion state for a gallery `Table` sample.
pub fn current_table_sample_expansion(
    sample_id: impl Into<String>,
    fallback: &TableExpansionState,
    cx: &impl AppContext,
) -> TableExpansionState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled faceted-filter state for a gallery `Table` sample.
pub fn current_table_sample_faceted_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled range-filter state for a gallery `Table` sample.
pub fn current_table_sample_range_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled global-filter state for a gallery `Table` sample.
pub fn current_table_sample_global_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled predicate-filter state for a gallery `Table` sample.
pub fn current_table_sample_predicate_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled column-visibility overrides for a gallery `Table` sample.
pub fn current_table_sample_column_visibility_overrides(
    sample_id: impl Into<String>,
    fallback: &TableColumnVisibilityOverrides,
    cx: &impl AppContext,
) -> TableColumnVisibilityOverrides {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

fn table_state_effective_column_order(state: &TableState) -> Vec<TableColumnId> {
    if state.column_order().is_empty() {
        state
            .columns()
            .iter()
            .map(|column| column.id().clone())
            .collect()
    } else {
        state.column_order().to_vec()
    }
}

/// Returns the current controlled column-order state for a gallery `Table` sample.
pub fn current_table_sample_column_order(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> Vec<TableColumnId> {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.column_order_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| table_state_effective_column_order(fallback))
    })
}

/// Returns the current controlled text-cell edit state for a gallery `Table` sample.
pub fn current_table_sample_cell_edit_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Applies a resolved expansion state to a sample table state.
pub fn table_state_with_expansion(state: TableState, expansion: TableExpansionState) -> TableState {
    match expansion {
        TableExpansionState::All => state.with_all_rows_expanded(),
        TableExpansionState::Rows(rows) => state.with_expanded_rows(rows),
    }
}

/// Applies current gallery runtime overrides to a table sample state.
pub fn table_sample_state_with_runtime(
    sample: &TableSample,
    sizing: TableColumnSizing,
    expansion: TableExpansionState,
    cx: &impl AppContext,
) -> TableState {
    let state = current_table_sample_global_filter_state(sample.id, &sample.state, cx);
    let state = current_table_sample_predicate_filter_state(sample.id, &state, cx);
    let state = current_table_sample_faceted_filter_state(sample.id, &state, cx);
    let state = current_table_sample_range_filter_state(sample.id, &state, cx);
    let state = current_table_sample_cell_edit_state(sample.id, &state, cx);
    let loaded_server_tree = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.server_tree_loaded
            .get(sample.id)
            .copied()
            .unwrap_or(false)
    });
    let state = if sample.id == "server-tree" && loaded_server_tree {
        server_tree_table_state(true)
    } else {
        state
    };
    let column_order = current_table_sample_column_order(sample.id, &state, cx);
    let state = state.with_column_order(column_order);
    let visibility =
        current_table_sample_column_visibility_overrides(sample.id, state.column_visibility(), cx);
    let state = state.with_column_visibility(visibility);

    table_state_with_expansion(state.with_column_sizing(sizing), expansion)
}

/// Records a gallery `Table` sizing commit in app-global sample state.
pub fn record_table_sizing_change(
    sample_id: impl Into<String>,
    change: &TableColumnSizingChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.sizing_changes.push(TableSampleSizingChange {
            sample_id: sample_id.clone(),
            column_id: change.column_id().as_str().to_owned(),
            width: change.width(),
        });
        log.committed_sizing
            .insert(sample_id, change.sizing().clone());
    });
}

/// Records and applies a controlled gallery `Table` column-order change.
pub fn record_table_column_order_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableColumnOrderChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = table_state_effective_column_order(fallback);
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .column_order_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to_order(current)
    });

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.column_order_changes.push(TableSampleColumnOrderChange {
            sample_id: sample_id.clone(),
            column_id: change.column_id().as_str().to_owned(),
            target_column_id: change.target_column_id().as_str().to_owned(),
            placement: change.placement().as_str().to_owned(),
            region: change.target_region().as_str().to_owned(),
            column_order: next
                .iter()
                .map(|column_id| column_id.as_str().to_owned())
                .collect(),
        });
        log.column_order_overrides.insert(sample_id, next);
    });
}

/// Records a gallery `Table` row activation in app-global sample state.
pub fn record_table_row_activation(
    sample_id: impl Into<String>,
    activation: &TableRowActivation,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let action = activation.action();
    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.row_activations.push(TableSampleRowActivation {
            sample_id,
            row_id: activation.row_id().as_str().to_owned(),
            render_key: action.render_key().to_owned(),
            kind: activation.kind().as_str().to_owned(),
            model_index: action.model_index(),
            depth: action.depth(),
            tree_branch: action.tree_branch(),
            tree_expanded: action.tree_expanded(),
            selected: action.selected(),
        });
    });
}

/// Records and applies a controlled gallery `Table` source-tree expansion request.
pub fn record_table_expansion_request(
    sample_id: impl Into<String>,
    fallback: &TableExpansionState,
    toggle: &TableRowExpansionToggle,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let row_id = toggle.row_id().clone();
    let expanded = toggle.expanded();
    let depth = toggle.action().depth();
    let loaded_child_count = toggle.loaded_child_count();
    let children_load_state = toggle
        .children_load_state()
        .map(TableRowChildrenLoadState::as_str)
        .unwrap_or("none")
        .to_owned();
    let children_load_message = toggle
        .children_load_state()
        .and_then(TableRowChildrenLoadState::message)
        .map(str::to_owned);

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_toggles.push(TableSampleExpansionToggle {
            sample_id: sample_id.clone(),
            row_id: row_id.as_str().to_owned(),
            expanded,
            depth,
            loaded_child_count,
            children_load_state,
            children_load_message,
        });
        if sample_id == "server-tree" && row_id.as_str() == "server-workspace" && expanded {
            log.server_tree_loaded.insert(sample_id.clone(), true);
        }

        let current = log
            .expansion_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        let next = match current {
            TableExpansionState::All if expanded => TableExpansionState::All,
            TableExpansionState::All => TableExpansionState::default(),
            TableExpansionState::Rows(mut rows) => {
                if expanded {
                    rows.insert(row_id);
                } else {
                    rows.remove(&row_id);
                }
                TableExpansionState::Rows(rows)
            }
        };
        log.expansion_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` faceted-filter change.
pub fn record_table_faceted_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableFacetedFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.faceted_filter_changes
            .push(TableSampleFacetedFilterChange {
                sample_id: sample_id.clone(),
                column_id: change.column_id().as_str().to_owned(),
                selected_values: change.selected_values().to_vec(),
                toggled_value: change.toggled_value().map(str::to_owned),
                selected: change.selected(),
                filtered_rows,
                final_rows,
            });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` range-filter change.
pub fn record_table_range_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableRangeFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.range_filter_changes.push(TableSampleRangeFilterChange {
            sample_id: sample_id.clone(),
            column_id: change.column_id().as_str().to_owned(),
            min_text: change.min_text().to_owned(),
            max_text: change.max_text().to_owned(),
            min_value: change.min_value(),
            max_value: change.max_value(),
            cleared: change.cleared(),
            filtered_rows,
            final_rows,
        });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` global-filter change.
pub fn record_table_global_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableGlobalFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.global_filter_changes
            .push(TableSampleGlobalFilterChange {
                sample_id: sample_id.clone(),
                query: change.query().to_owned(),
                cleared: change.cleared(),
                filtered_rows,
                final_rows,
            });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` predicate-filter change.
pub fn record_table_predicate_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TablePredicateFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.predicate_filter_changes
            .push(TableSamplePredicateFilterChange {
                sample_id: sample_id.clone(),
                column_id: change.column_id().as_str().to_owned(),
                operator: change
                    .operator()
                    .map(|operator| operator.as_str().to_owned()),
                value: change.value().to_owned(),
                cleared: change.cleared(),
                filtered_rows,
                final_rows,
            });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` column-visibility change.
pub fn record_table_column_visibility_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableColumnVisibilityChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current_visibility = log
            .visibility_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.column_visibility().clone());
        let current = fallback.clone().with_column_visibility(current_visibility);
        change.apply_to(current)
    });
    let visible_columns = next.visible_columns().len();
    let hidden_columns = next.columns().len().saturating_sub(visible_columns);
    let next_visibility = next.column_visibility().clone();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_changes
            .push(TableSampleColumnVisibilityChange {
                sample_id: sample_id.clone(),
                action: change.action().as_str().to_owned(),
                column_ids: change
                    .column_ids()
                    .iter()
                    .map(|column_id| column_id.as_str().to_owned())
                    .collect(),
                next_visible: change.next_visible(),
                visible_columns,
                hidden_columns,
            });
        log.visibility_overrides.insert(sample_id, next_visibility);
    });
}

/// Records and applies a controlled gallery `Table` cell edit.
pub fn record_table_cell_edit_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableCellEditChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let (next, outcome) = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .cell_edit_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes.push(TableSampleCellEditChange {
            sample_id: sample_id.clone(),
            row_id: change.row_id().as_str().to_owned(),
            column_id: change.column_id().as_str().to_owned(),
            source_index: change.source_index(),
            previous_text: change.previous_text().to_owned(),
            next_text: change.next_text().to_owned(),
            outcome: outcome.as_str().to_owned(),
        });
        if outcome == TableCellEditApplyOutcome::Updated {
            log.cell_edit_overrides.insert(sample_id, next);
        }
    });
}

impl TreeSample {
    /// Returns the current controlled item descriptors for this sample.
    pub fn current_items(&self, cx: &impl AppContext) -> Vec<TreeItemDescriptor> {
        current_tree_sample_items(self.id, &self.items, cx)
    }

    /// Returns the current controlled tree state for this sample.
    pub fn current_state(&self, cx: &impl AppContext) -> TreeState {
        let items = self.current_items(cx);
        TreeState::resolve(
            self.state.size(),
            self.state.label(),
            self.state.selected_value(),
            self.state.focused_value(),
            items,
        )
    }

    /// Builds the concrete GPUI tree for this sample.
    pub fn build_tree(&self) -> Tree {
        let mut tree = Tree::new(
            format!("component-tree:{}", self.id),
            self.title,
            self.items.clone(),
        )
        .with_size(self.size)
        .virtualized(self.virtualized)
        .draggable(self.draggable)
        .viewport_item_count(self.viewport_item_count)
        .overscan_count(self.overscan_count);

        if let Some(selected) = self.state.selected_value() {
            tree = tree.default_selected(selected);
        }
        if let Some(focused) = self.state.focused_value() {
            tree = tree.default_focused(focused);
        }

        tree
    }

    /// Builds the concrete GPUI tree for this sample using current gallery overrides.
    pub fn build_tree_with_runtime(&self, cx: &impl AppContext) -> Tree {
        let mut tree = Tree::new(
            format!("component-tree:{}", self.id),
            self.title,
            self.current_items(cx),
        )
        .with_size(self.size)
        .virtualized(self.virtualized)
        .draggable(self.draggable)
        .viewport_item_count(self.viewport_item_count)
        .overscan_count(self.overscan_count);

        if let Some(selected) = self.state.selected_value() {
            tree = tree.default_selected(selected);
        }
        if let Some(focused) = self.state.focused_value() {
            tree = tree.default_focused(focused);
        }

        tree
    }

    /// Resolves the sample's virtualized render plan at the viewport origin.
    pub fn render_plan(&self) -> TreeRenderPlan {
        self.build_tree().render_plan(
            UiPx::ZERO,
            self.state.metrics().row_height() * self.viewport_item_count as f32,
        )
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
    /// Semantic scroll alignment the rendered adapter can apply when revealing the active row.
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

/// One virtualized list sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Shared item descriptors consumed by the concrete list renderer.
    pub items: Arc<[VirtualizedListItemDescriptor]>,
    /// Resolved renderer-neutral list state.
    pub state: VirtualizedListState,
    /// Visual size applied to the concrete list.
    pub size: Size,
    /// Fixed list viewport used by the sample summary.
    pub viewport_extent: UiPx,
    /// Fixed row height used by the virtualizer.
    pub row_height: UiPx,
    /// Overscan row budget.
    pub overscan: usize,
    /// Precomputed state summary used by the gallery page.
    state_summary: VirtualizedListSampleStateSummary,
}

/// Precomputed state summary for a virtualized list sample.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VirtualizedListSampleStateSummary {
    /// Total source item count.
    pub item_count: usize,
    /// Rendered row count after overscan.
    pub rendered_rows: usize,
    /// Visible row count before overscan.
    pub visible_rows: usize,
    /// Visible row range start.
    pub visible_start: usize,
    /// Visible row range end.
    pub visible_end: usize,
    /// Overscan row range start.
    pub overscan_start: usize,
    /// Overscan row range end.
    pub overscan_end: usize,
    /// Active row index.
    pub active_index: Option<usize>,
    /// Selected row index.
    pub selected_index: Option<usize>,
}

/// One activation captured from the rendered gallery `VirtualizedList` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleActivation {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Activated item index.
    pub index: usize,
}

/// Runtime activation log used by gallery smoke tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleRuntimeLog {
    activations: Vec<VirtualizedListSampleActivation>,
}

impl Global for VirtualizedListSampleRuntimeLog {}

impl VirtualizedListSampleRuntimeLog {
    /// Returns captured activations in event order.
    pub fn activations(&self) -> &[VirtualizedListSampleActivation] {
        &self.activations
    }

    /// Clears captured activations.
    pub fn clear(&mut self) {
        self.activations.clear();
    }
}

/// Records a gallery `VirtualizedList` activation in app-global sample state.
pub fn record_virtualized_list_activation(
    sample_id: impl Into<String>,
    index: usize,
    cx: &mut App,
) {
    cx.update_default_global::<VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.activations.push(VirtualizedListSampleActivation {
            sample_id: sample_id.into(),
            index,
        });
    });
}

impl VirtualizedListSampleStateSummary {
    fn from_plan(plan: &VirtualizedListRenderPlan) -> Self {
        let visible = plan.virtualizer().visible_range();
        let overscan = plan.virtualizer().overscan_range();

        Self {
            item_count: plan.state().item_count(),
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            visible_start: visible.start(),
            visible_end: visible.end(),
            overscan_start: overscan.start(),
            overscan_end: overscan.end(),
            active_index: plan.state().active_index(),
            selected_index: plan.state().selected_index(),
        }
    }
}

impl VirtualizedListSample {
    /// Builds the concrete GPUI virtualized list for this sample.
    pub fn build_list(&self) -> VirtualizedList {
        let mut list = VirtualizedList::from_shared_items(
            format!("component-virtualized-list:{}", self.id),
            self.title,
            self.items.clone(),
        )
        .with_size(self.size)
        .row_height(self.row_height)
        .overscan(self.overscan)
        .viewport_item_count(self.state.viewport_item_count())
        .disabled(self.state.disabled());

        if let Some(active_index) = self.state.active_index() {
            list = list.default_active_index(active_index);
        }
        if let Some(selected_index) = self.state.selected_index() {
            list = list.default_selected_index(selected_index);
        }

        list
    }

    /// Resolves the sample's render plan at the viewport origin.
    pub fn render_plan(&self) -> VirtualizedListRenderPlan {
        self.build_list()
            .render_plan(UiPx::ZERO, self.viewport_extent)
    }

    /// Returns the precomputed state summary.
    pub const fn state_summary(&self) -> VirtualizedListSampleStateSummary {
        self.state_summary
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

/// One textarea sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TextareaSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample label.
    pub label: &'static str,
    /// Resolved state.
    pub state: TextareaState,
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

/// One field sample that composes a textarea control.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldTextareaSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved field state.
    pub state: FieldState,
    /// Resolved textarea control state.
    pub textarea_state: TextareaState,
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
            tabs.default_selected(selected)
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
    /// Top-pinned row count in the final visual model.
    pub pinned_top_rows: usize,
    /// Center row count used by the row virtualizer.
    pub pinned_center_rows: usize,
    /// Bottom-pinned row count in the final visual model.
    pub pinned_bottom_rows: usize,
    /// Whether row pinning is limited to the current page.
    pub row_pinning_page_only: bool,
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
    /// Visible header row count across all rendered regions.
    pub header_rows: usize,
    /// Unique visible group header count across all rendered regions.
    pub header_groups: usize,
    /// Visible leaf column count across all rendered regions.
    pub visible_leaf_columns: usize,
    /// Row count before expansion flattens the grouped tree.
    pub grouped_rows: usize,
    /// Row count after expansion applies.
    pub expanded_rows: usize,
    /// Visible group row count in the final model.
    pub group_rows: usize,
    /// Visible leaf row count in the final model.
    pub leaf_rows: usize,
    /// Visible tree row count in the final model.
    pub tree_rows: usize,
    /// Visible tree branch row count in the final model.
    pub tree_branch_rows: usize,
    /// Visible expandable tree rows without loaded children.
    pub unloaded_tree_branches: usize,
    /// Visible tree rows currently marked as loading children.
    pub loading_tree_rows: usize,
    /// Visible tree rows currently marked as failed child loads.
    pub failed_tree_rows: usize,
    /// Deepest visible tree depth in the final model.
    pub tree_depth: usize,
    /// Whether the sample keeps expansion pruning app-owned.
    pub manual_expansion: bool,
    /// Whether filtering is app-owned.
    pub manual_filtering: bool,
    /// Whether sorting is app-owned.
    pub manual_sorting: bool,
    /// Whether pagination is app-owned.
    pub manual_pagination: bool,
    /// Zero-based page index in the current snapshot.
    pub pagination_page_index: usize,
    /// Page size in the current snapshot.
    pub pagination_page_size: usize,
    /// Server-known total row count, if any.
    pub pagination_row_count: Option<usize>,
    /// Total page count, if any.
    pub pagination_page_count: Option<usize>,
    /// Resolved facet summary count.
    pub facet_columns: usize,
    /// Resolved caller-owned facet summary count.
    pub manual_facet_columns: usize,
    /// Unique status facet value count.
    pub status_facet_values: usize,
    /// Sum of status facet value counts.
    pub status_facet_total_count: usize,
    /// Rounded score facet minimum, if present.
    pub score_facet_min: Option<usize>,
    /// Rounded score facet maximum, if present.
    pub score_facet_max: Option<usize>,
    /// Configured grouping column count.
    pub grouping_columns: usize,
    /// Configured aggregate column count.
    pub aggregation_count: usize,
    /// Named custom aggregate callback count.
    pub custom_aggregation_count: usize,
    /// Explicit expanded group row ids, or all group rows when expansion is global.
    pub expanded_group_inputs: usize,
    /// Explicit expanded tree row ids, or all tree branch rows when expansion is global.
    pub expanded_tree_inputs: usize,
    /// Whether every group row is expanded.
    pub all_rows_expanded: bool,
    /// Visible left-pinned columns.
    pub pinned_left_columns: usize,
    /// Visible unpinned center columns.
    pub pinned_center_columns: usize,
    /// Visible right-pinned columns.
    pub pinned_right_columns: usize,
    /// Rounded visible left-pinned lane width.
    pub pinned_left_width_px: usize,
    /// Rounded visible center lane width.
    pub pinned_center_width_px: usize,
    /// Rounded visible right-pinned lane width.
    pub pinned_right_width_px: usize,
    /// Rounded total visible column width.
    pub total_column_width_px: usize,
    /// Visible resizable columns.
    pub resizable_columns: usize,
}

impl TableSampleStateSummary {
    fn from_plan(plan: &TableRenderDiagnostics, state: &TableState) -> Self {
        let visible = plan.virtualizer().visible_range();
        let overscan = plan.virtualizer().overscan_range();
        let final_rows = plan.table().final_model().rows();
        let row_regions = plan.table().row_regions();
        let group_rows = final_rows.iter().filter(|row| row.is_group()).count();
        let tree_rows = final_rows.iter().filter(|row| row.tree().is_some()).count();
        let tree_branch_rows = final_rows.iter().filter(|row| row.is_tree_branch()).count();
        let unloaded_tree_branches = final_rows
            .iter()
            .filter(|row| {
                row.is_tree_branch()
                    && row.loaded_child_count() == 0
                    && row
                        .children_load_state()
                        .is_some_and(|state| *state == TableRowChildrenLoadState::Idle)
            })
            .count();
        let loading_tree_rows = final_rows
            .iter()
            .filter(|row| {
                row.children_load_state()
                    .is_some_and(TableRowChildrenLoadState::is_loading)
            })
            .count();
        let failed_tree_rows = final_rows
            .iter()
            .filter(|row| {
                row.children_load_state()
                    .is_some_and(TableRowChildrenLoadState::is_failed)
            })
            .count();
        let tree_depth = final_rows.iter().map(|row| row.depth()).max().unwrap_or(0);
        let regions = plan.table().visible_column_regions();
        let header_groups = plan.table().header_groups();
        let visible_group_ids = header_groups
            .all()
            .flat_map(|group| group.headers().iter())
            .filter(|cell| cell.is_group())
            .map(|cell| cell.source_id().to_owned())
            .collect::<BTreeSet<_>>();
        let status_column = TableColumnId::new("status");
        let score_column = TableColumnId::new("score");
        let status_facet = plan.column_facet(&status_column);
        let score_range = plan
            .column_facet(&score_column)
            .and_then(|facet| facet.numeric_range());
        let score_facet_min = score_range.map(|range| range.min().round() as usize);
        let score_facet_max = score_range.map(|range| range.max().round() as usize);
        let (all_rows_expanded, expanded_group_inputs, expanded_tree_inputs) =
            match state.expansion() {
                TableExpansionState::All => (
                    true,
                    plan.table()
                        .grouped_model()
                        .rows()
                        .iter()
                        .filter(|row| row.is_group())
                        .count(),
                    plan.table()
                        .core_model()
                        .rows()
                        .iter()
                        .filter(|row| row.is_tree_branch())
                        .count(),
                ),
                TableExpansionState::Rows(rows) => (
                    false,
                    rows.iter()
                        .filter(|row_id| {
                            plan.table()
                                .grouped_model()
                                .row(row_id)
                                .is_some_and(|row| row.is_group())
                        })
                        .count(),
                    rows.iter()
                        .filter(|row_id| {
                            plan.table()
                                .core_model()
                                .row(row_id)
                                .is_some_and(|row| row.is_tree_branch())
                        })
                        .count(),
                ),
            };

        Self {
            core_rows: plan.table().core_model().rows().len(),
            filtered_rows: plan.table().filtered_model().rows().len(),
            final_rows: plan.table().final_model().rows().len(),
            pinned_top_rows: row_regions.top().len(),
            pinned_center_rows: row_regions.center().len(),
            pinned_bottom_rows: row_regions.bottom().len(),
            row_pinning_page_only: plan.table().row_pinning_policy()
                == TableRowPinningPolicy::PageOnly,
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            visible_start: visible.start(),
            visible_end: visible.end(),
            overscan_start: overscan.start(),
            overscan_end: overscan.end(),
            aria_columns: plan.aria_column_count(),
            aria_rows: plan.aria_row_count(),
            selected_rows: plan.table().final_model().selected_count(),
            header_rows: plan.header_row_count(),
            header_groups: visible_group_ids.len(),
            visible_leaf_columns: plan.columns().len(),
            grouped_rows: plan.table().grouped_model().rows().len(),
            expanded_rows: plan.table().expanded_model().rows().len(),
            group_rows,
            leaf_rows: final_rows.len().saturating_sub(group_rows),
            tree_rows,
            tree_branch_rows,
            unloaded_tree_branches,
            loading_tree_rows,
            failed_tree_rows,
            tree_depth,
            manual_expansion: state.expansion_mode() == TableExpansionMode::Manual,
            manual_filtering: plan.filtering_mode() == TableStageMode::Manual,
            manual_sorting: plan.sorting_mode() == TableStageMode::Manual,
            manual_pagination: plan.pagination_mode() == TableStageMode::Manual,
            pagination_page_index: state.pagination().page_index(),
            pagination_page_size: state.pagination().page_size(),
            pagination_row_count: plan.pagination_row_count(),
            pagination_page_count: plan.pagination_page_count(),
            facet_columns: plan.column_facets().len(),
            manual_facet_columns: plan
                .column_facets()
                .iter()
                .filter(|facet| facet.mode() == TableStageMode::Manual)
                .count(),
            status_facet_values: status_facet
                .map(|facet| facet.unique_values().len())
                .unwrap_or(0),
            status_facet_total_count: status_facet
                .map(|facet| {
                    facet
                        .unique_values()
                        .iter()
                        .map(|entry| entry.count())
                        .sum()
                })
                .unwrap_or(0),
            score_facet_min,
            score_facet_max,
            grouping_columns: state.grouping().len(),
            aggregation_count: state.aggregations().len(),
            custom_aggregation_count: plan.aggregation_fn_count(),
            expanded_group_inputs,
            expanded_tree_inputs,
            all_rows_expanded,
            pinned_left_columns: regions.left().len(),
            pinned_center_columns: regions.center().len(),
            pinned_right_columns: regions.right().len(),
            pinned_left_width_px: plan
                .column_region_width(TableColumnRegion::Left)
                .as_f32()
                .round() as usize,
            pinned_center_width_px: plan
                .column_region_width(TableColumnRegion::Center)
                .as_f32()
                .round() as usize,
            pinned_right_width_px: plan
                .column_region_width(TableColumnRegion::Right)
                .as_f32()
                .round() as usize,
            total_column_width_px: plan.total_column_width().as_f32().round() as usize,
            resizable_columns: plan
                .columns()
                .iter()
                .filter(|column| column.resizable())
                .count(),
        }
    }
}

impl TableSample {
    /// Builds the concrete GPUI table for this sample.
    pub fn build_table(&self) -> Table {
        self.build_table_with_sizing(self.state.column_sizing().clone())
    }

    /// Builds the concrete GPUI table with caller-owned column sizing.
    pub fn build_table_with_sizing(&self, column_sizing: TableColumnSizing) -> Table {
        self.build_table_with_state(self.state.clone().with_column_sizing(column_sizing))
    }

    /// Builds the concrete GPUI table from a fully resolved sample state.
    pub fn build_table_with_state(&self, state: TableState) -> Table {
        Table::new(format!("component-table:{}", self.id), self.title, state)
            .with_size(self.size)
            .viewport_extent(self.viewport_extent)
            .row_height(self.row_height)
            .overscan(self.overscan)
    }

    /// Resolves the table plan used by gallery tests and state rows.
    pub fn render_plan(&self) -> TableRenderDiagnostics {
        self.build_table()
            .diagnostics(UiPx::ZERO, self.viewport_extent)
    }

    /// Returns the precomputed state summary used by the gallery page.
    pub const fn state_summary(&self) -> TableSampleStateSummary {
        self.state_summary
    }

    /// Resolves the summary for a caller-supplied table state using this sample's layout settings.
    pub fn state_summary_for_state(&self, state: &TableState) -> TableSampleStateSummary {
        let plan = self
            .build_table_with_state(state.clone())
            .diagnostics(UiPx::ZERO, self.viewport_extent);
        TableSampleStateSummary::from_plan(&plan, state)
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
            toolbar = toolbar.default_focused(focused);
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
    /// Standalone descriptors consumed by the concrete command renderer.
    pub items: Arc<[CommandItemDescriptor]>,
    /// Group descriptors consumed by the concrete command renderer.
    pub groups: Arc<[CommandGroupDescriptor]>,
    /// Optional caller-owned command index snapshot.
    pub index_snapshot: Option<CommandIndexSnapshot>,
    /// Persistent selected values for multi-select samples.
    pub selected_values: Arc<[String]>,
    /// Estimated visible row count for the result viewport.
    pub viewport_item_count: usize,
    /// Optional fixed row height for virtualized command results.
    pub row_height: Option<UiPx>,
    /// Overscan row budget for virtualized command results.
    pub overscan: usize,
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
impl_component_sample_selectors!(AccordionSample, "component-accordion-sample");
impl_component_sample_selectors!(CollapsibleSample, "component-collapsible-sample");
impl_component_sample_selectors!(SliderSample, "component-slider-sample");
impl_component_sample_selectors!(NumberInputSample, "component-number-input-sample");
impl_component_sample_selectors!(ToggleGroupSample, "component-toggle-group-sample");
impl_component_sample_selectors!(LinkSample, "component-link-sample");
impl_component_sample_selectors!(BreadcrumbSample, "component-breadcrumb-sample");
impl_component_sample_selectors!(TagSample, "component-tag-sample");
impl_component_sample_selectors!(ToastStackSample, "component-toast-stack-sample");
impl_component_sample_selectors!(IconButtonSample, "component-icon-button-sample");
impl_component_sample_selectors!(SwitchSample, "component-switch-sample");
impl_component_sample_selectors!(CheckboxSample, "component-checkbox-sample");
impl_component_sample_selectors!(RadioGroupSample, "component-radio-sample");
impl_component_sample_selectors!(ToggleSample, "component-toggle-sample");
impl_component_sample_selectors!(ToolbarSample, "component-toolbar-sample");
impl_component_sample_selectors!(SidebarSample, "component-sidebar-sample");
impl_component_sample_selectors!(TreeSample, "component-tree-sample");
impl_component_sample_selectors!(ListboxSample, "component-listbox-sample");
impl_component_sample_selectors!(SelectSample, "component-select-sample");
impl_component_sample_selectors!(ComboboxSample, "component-combobox-sample");
impl_component_sample_selectors!(CommandSample, "component-command-sample");
impl_component_sample_selectors!(LabelSample, "component-label-sample");
impl_component_sample_selectors!(TextInputSample, "component-text-input-sample");
impl_component_sample_selectors!(TextareaSample, "component-textarea-sample");
impl_component_sample_selectors!(FieldSample, "component-field-sample");
impl_component_sample_selectors!(FieldTextareaSample, "component-field-textarea-sample");
impl_component_sample_selectors!(TabsSample, "component-tabs-sample");
impl_component_sample_selectors!(TableSample, "component-table-sample");
impl_component_sample_selectors!(VirtualizedListSample, "component-virtualized-list-sample");
impl_component_sample_selectors!(ScrollAreaSample, "component-scroll-area-sample");
impl_component_sample_selectors!(SplitterSample, "component-splitter-sample");
impl_component_sample_selectors!(SeparatorSample, "component-separator-sample");
impl_component_sample_selectors!(KbdSample, "component-kbd-sample");
impl_component_sample_selectors!(ProgressSample, "component-progress-sample");
impl_component_sample_selectors!(SkeletonSample, "component-skeleton-sample");
impl_component_sample_selectors!(AvatarSample, "component-avatar-sample");
impl_component_sample_selectors!(AvatarGroupSample, "component-avatar-group-sample");
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

/// Grouped samples for newly completed foundation components.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationComponentSamples {
    /// Accordion samples.
    pub accordions: [AccordionSample; 1],
    /// Collapsible samples.
    pub collapsibles: [CollapsibleSample; 1],
    /// Slider samples.
    pub sliders: [SliderSample; 2],
    /// Number input samples.
    pub number_inputs: [NumberInputSample; 2],
    /// Toggle group samples.
    pub toggle_groups: [ToggleGroupSample; 2],
    /// Link samples.
    pub links: [LinkSample; 2],
    /// Breadcrumb samples.
    pub breadcrumbs: [BreadcrumbSample; 1],
    /// Tag samples.
    pub tags: [TagSample; 3],
    /// Toast stack samples.
    pub toast_stacks: [ToastStackSample; 1],
}

/// Returns samples for the foundation component completion slice.
pub fn foundation_component_samples(tokens: ThemeTokens) -> FoundationComponentSamples {
    FoundationComponentSamples {
        accordions: accordion_samples(tokens),
        collapsibles: collapsible_samples(tokens),
        sliders: slider_samples(tokens),
        number_inputs: number_input_samples(tokens),
        toggle_groups: toggle_group_samples(tokens),
        links: link_samples(tokens),
        breadcrumbs: breadcrumb_samples(tokens),
        tags: tag_samples(tokens),
        toast_stacks: toast_stack_samples(tokens),
    }
}

/// Returns accordion samples backed by real component state.
pub fn accordion_samples(tokens: ThemeTokens) -> [AccordionSample; 1] {
    let items = vec![
        AccordionItem::new("scope", "Scope", "Component contracts, samples, and tests."),
        AccordionItem::new(
            "risk",
            "Risk",
            "Breaking changes are acceptable before launch.",
        ),
        AccordionItem::new(
            "done",
            "Done",
            "Exported state and gallery coverage are required.",
        )
        .disabled(true),
    ];
    let accordion = Accordion::new("shipping")
        .mode(AccordionMode::Multiple)
        .collapsible(true)
        .default_open_values(["scope", "risk"])
        .tokens(tokens);
    let state = items
        .iter()
        .cloned()
        .fold(accordion, |accordion, item| accordion.item(item))
        .state();

    [AccordionSample {
        id: "shipping",
        title: "Shipping checklist",
        summary: "Multiple open panels with one disabled item.",
        state,
        items,
    }]
}

/// Returns collapsible samples backed by real component state.
pub fn collapsible_samples(tokens: ThemeTokens) -> [CollapsibleSample; 1] {
    [CollapsibleSample {
        id: "release-notes",
        summary: "Controlled disclosure content that keeps trigger and panel roles separate.",
        content: "Release notes stay mounted only when the disclosure is open.",
        state: Collapsible::new("release-notes", "Release notes")
            .default_open(true)
            .tokens(tokens)
            .state(),
    }]
}

/// Returns slider samples backed by real component state.
pub fn slider_samples(tokens: ThemeTokens) -> [SliderSample; 2] {
    [
        (
            "volume",
            "Volume",
            72.0,
            0.0,
            100.0,
            1.0,
            false,
            Size::Medium,
        ),
        (
            "threshold",
            "Threshold",
            42.0,
            0.0,
            50.0,
            5.0,
            true,
            Size::Small,
        ),
    ]
    .map(
        |(id, label, value, min, max, step, disabled, size)| SliderSample {
            id,
            state: Slider::new(id, label)
                .value(value)
                .min(min)
                .max(max)
                .step(step)
                .disabled(disabled)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns number input samples backed by real component state.
pub fn number_input_samples(tokens: ThemeTokens) -> [NumberInputSample; 2] {
    [
        ("workers", "Workers", 6.0, 1.0, 12.0, 1.0, false, false),
        ("budget", "Budget", 85.0, 0.0, 100.0, 5.0, false, true),
    ]
    .map(
        |(id, label, value, min, max, step, read_only, invalid)| NumberInputSample {
            id,
            state: NumberInput::new(id, label)
                .value(value)
                .min(min)
                .max(max)
                .step(step)
                .read_only(read_only)
                .invalid(invalid)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns toggle group samples backed by real component state.
pub fn toggle_group_samples(tokens: ThemeTokens) -> [ToggleGroupSample; 2] {
    let alignment = ToggleGroup::new("alignment", "Alignment")
        .item(ToggleGroupItem::new("left", "Left"))
        .item(ToggleGroupItem::new("center", "Center"))
        .item(ToggleGroupItem::new("right", "Right").disabled(true))
        .selected_values(["left"])
        .default_focused("center")
        .selection_required(true)
        .tokens(tokens)
        .state();
    let formatting = ToggleGroup::new("formatting", "Formatting")
        .mode(ToggleGroupSelectionMode::Multiple)
        .item(ToggleGroupItem::new("bold", "Bold"))
        .item(ToggleGroupItem::new("italic", "Italic"))
        .item(ToggleGroupItem::new("code", "Code"))
        .selected_values(["bold", "code"])
        .tokens(tokens)
        .state();

    [
        ToggleGroupSample {
            id: "alignment",
            summary: "Required single selection with disabled item skip.",
            state: alignment,
        },
        ToggleGroupSample {
            id: "formatting",
            summary: "Multiple stable values selected at once.",
            state: formatting,
        },
    ]
}

/// Returns link samples backed by real component state.
pub fn link_samples(tokens: ThemeTokens) -> [LinkSample; 2] {
    [
        LinkSample {
            id: "docs",
            state: Link::new("docs", "Component docs", "/docs/components")
                .external(true)
                .tokens(tokens)
                .state(),
        },
        LinkSample {
            id: "disabled",
            state: Link::new("disabled", "Disabled target", "/disabled")
                .disabled(true)
                .tokens(tokens)
                .state(),
        },
    ]
}

/// Returns breadcrumb samples backed by real component state.
pub fn breadcrumb_samples(tokens: ThemeTokens) -> [BreadcrumbSample; 1] {
    [BreadcrumbSample {
        id: "project",
        state: Breadcrumb::new("project", "Project path")
            .item(BreadcrumbItemDescriptor::new("home", "Home").href("/"))
            .item(BreadcrumbItemDescriptor::new("ui", "UI").href("/ui"))
            .item(BreadcrumbItemDescriptor::new("components", "Components").current(true))
            .tokens(tokens)
            .state(),
    }]
}

/// Returns tag samples backed by real component state.
pub fn tag_samples(tokens: ThemeTokens) -> [TagSample; 3] {
    [
        ("ready", "ready", "Ready", TagVariant::Default, true, false),
        (
            "blocked",
            "blocked",
            "Blocked",
            TagVariant::Destructive,
            false,
            false,
        ),
        (
            "archived",
            "archived",
            "Archived",
            TagVariant::Outline,
            true,
            true,
        ),
    ]
    .map(
        |(id, value, label, variant, removable, disabled)| TagSample {
            id,
            state: Tag::new(id, value, label)
                .variant(variant)
                .removable(removable)
                .disabled(disabled)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns toast stack samples backed by real component state.
pub fn toast_stack_samples(tokens: ThemeTokens) -> [ToastStackSample; 1] {
    [ToastStackSample {
        id: "notifications",
        state: ToastStack::new("notifications", "Notifications")
            .max_visible(2)
            .toast(
                Toast::new("saved", "Saved")
                    .description("Settings are synced.")
                    .intent(FeedbackIntent::Success)
                    .action("Undo"),
            )
            .toast(
                Toast::new("queued", "Queued")
                    .description("Release job will start shortly.")
                    .intent(FeedbackIntent::Info)
                    .timeout(Duration::from_secs(8)),
            )
            .toast(
                Toast::new("expired", "Expired")
                    .elapsed(Duration::from_secs(8))
                    .timeout(Duration::from_secs(2)),
            )
            .tokens(tokens)
            .state(),
    }]
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

/// Returns avatar group samples backed by real component state.
pub fn avatar_group_samples(tokens: ThemeTokens) -> [AvatarGroupSample; 1] {
    [AvatarGroupSample {
        id: "team",
        summary: "Compact overlapping roster with overflow count",
        avatars: vec![
            AvatarSample {
                id: "team-ada",
                state: Avatar::new("team-ada", "Ada Lovelace")
                    .accessible_label("Ada Lovelace")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
            AvatarSample {
                id: "team-grace",
                state: Avatar::new("team-grace", "Grace Hopper")
                    .accessible_label("Grace Hopper")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
            AvatarSample {
                id: "team-katherine",
                state: Avatar::new("team-katherine", "Katherine Johnson")
                    .accessible_label("Katherine Johnson")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
            AvatarSample {
                id: "team-margaret",
                state: Avatar::new("team-margaret", "Margaret Hamilton")
                    .accessible_label("Margaret Hamilton")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
        ],
        count_label: "+1",
    }]
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

static TREE_SAMPLES: LazyLock<[TreeSample; 4]> = LazyLock::new(build_tree_samples);

/// Returns tree samples backed by the concrete renderer and hierarchy contract.
pub fn tree_samples(_tokens: ThemeTokens) -> &'static [TreeSample] {
    TREE_SAMPLES.as_slice()
}

fn build_tree_samples() -> [TreeSample; 4] {
    let size = Size::Small;
    let items = document_outline_tree_sample_items();
    let state = TreeState::resolve(
        size,
        "Document outline",
        Some("paper"),
        Some("paper"),
        items.clone(),
    );
    let editable_items = editable_outline_tree_sample_items();
    let editable_state = TreeState::resolve(
        size,
        "Editable outline",
        Some("root"),
        Some("root"),
        editable_items.clone(),
    );

    let remote_items = remote_workspace_tree_sample_items();
    let remote_state = TreeState::resolve(
        size,
        "Remote workspace",
        Some("remote-src"),
        Some("remote-src"),
        remote_items.clone(),
    );

    let release_items = virtualized_release_tree_sample_items();
    let release_state = TreeState::resolve(
        size,
        "Release outline",
        Some("release-node-0000"),
        Some("release-node-0000"),
        release_items.clone(),
    );

    [
        TreeSample {
            id: "document-outline",
            title: "Document outline",
            summary: "Expandable hierarchy with roving focus, selection, and an owned scroll viewport.",
            badge: "tree",
            items,
            state,
            size,
            virtualized: false,
            draggable: false,
            viewport_item_count: 12,
            overscan_count: 4,
        },
        TreeSample {
            id: "remote-workspace",
            title: "Remote workspace",
            summary: "Loadable branches expose unloaded, loading, loaded, and failed child state.",
            badge: "lazy tree",
            items: remote_items,
            state: remote_state,
            size,
            virtualized: false,
            draggable: false,
            viewport_item_count: 12,
            overscan_count: 4,
        },
        TreeSample {
            id: "release-outline",
            title: "Release outline",
            summary: "Large visible hierarchy rendered through the Tree fixed-row virtual window.",
            badge: "virtual tree",
            items: release_items,
            state: release_state,
            size,
            virtualized: true,
            draggable: false,
            viewport_item_count: 8,
            overscan_count: 4,
        },
        TreeSample {
            id: "editable-outline",
            title: "Editable outline",
            summary: "Controlled drag moves update the visible outline in place.",
            badge: "drag tree",
            items: editable_items,
            state: editable_state,
            size,
            virtualized: false,
            draggable: true,
            viewport_item_count: 12,
            overscan_count: 4,
        },
    ]
}

/// Returns tree state-contract samples for renderer-neutral review.
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

fn document_outline_tree_sample_items() -> Vec<TreeItemDescriptor> {
    let appendix_items = (1..=12).map(|index| {
        TreeItemDescriptor::new(
            format!("appendix-{index:02}"),
            format!("Appendix section {index:02}"),
        )
    });

    vec![
        TreeItemDescriptor::new("paper", "Paper")
            .child(TreeItemDescriptor::new("intro", "Introduction"))
            .child(
                TreeItemDescriptor::new("figures", "Figures")
                    .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
            ),
        TreeItemDescriptor::new("appendix", "Appendix")
            .expanded(true)
            .children(appendix_items),
        TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
        TreeItemDescriptor::new("notes", "Notes"),
    ]
}

fn remote_workspace_tree_sample_items() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("remote-root", "Remote project")
            .expanded(true)
            .child(TreeItemDescriptor::new("remote-src", "src").with_children_unloaded())
            .child(
                TreeItemDescriptor::new("remote-crates", "crates")
                    .with_children_loading("Loading child packages"),
            )
            .child(
                TreeItemDescriptor::new("remote-build", "build artifacts")
                    .with_children_load_failed("Network unavailable"),
            )
            .child(
                TreeItemDescriptor::new("remote-docs", "docs")
                    .expanded(true)
                    .child(TreeItemDescriptor::new("remote-readme", "README.md")),
            ),
    ]
}

fn virtualized_release_tree_sample_items() -> Vec<TreeItemDescriptor> {
    (0..240)
        .map(|index| {
            TreeItemDescriptor::new(
                format!("release-node-{index:04}"),
                format!("Release node {index:04}"),
            )
        })
        .collect()
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

fn editable_outline_tree_sample_items() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("root", "Root")
            .expanded(true)
            .child(TreeItemDescriptor::new("child", "Child"))
            .child(TreeItemDescriptor::new("peer", "Peer")),
        TreeItemDescriptor::new("sibling", "Sibling"),
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
pub fn text_input_samples(tokens: ThemeTokens) -> [TextInputSample; 6] {
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
            TextInputDisplayMode::Plain,
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
            TextInputDisplayMode::Plain,
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
            TextInputDisplayMode::Plain,
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
            TextInputDisplayMode::Plain,
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
            TextInputDisplayMode::Plain,
        ),
        (
            "password",
            "Password",
            "a🙂中",
            "Password",
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
            TextInputDisplayMode::Password,
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
            display_mode,
        )| {
            let state = TextInputState::resolve_with_display_mode(
                value,
                Some(placeholder),
                size,
                disabled,
                read_only,
                invalid,
                required,
                controller_driven,
                display_mode,
                tokens,
            );

            TextInputSample { id, label, state }
        },
    )
}

/// Returns textarea samples backed by real component state.
pub fn textarea_samples(tokens: ThemeTokens) -> [TextareaSample; 4] {
    [
        (
            "default",
            "Default",
            "",
            "Write release notes...",
            3,
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "filled",
            "Filled",
            "Line 1\nLine 2",
            "Write release notes...",
            4,
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "overflow",
            "Overflow",
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8",
            "Write release notes...",
            3,
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
            "Needs a rollback note.",
            "Write release notes...",
            3,
            false,
            false,
            true,
            true,
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
            rows,
            disabled,
            read_only,
            required,
            invalid,
            size,
            controller_driven,
        )| {
            let state = TextareaState::resolve(
                value,
                Some(placeholder),
                size,
                rows,
                disabled,
                read_only,
                invalid,
                required,
                controller_driven,
                tokens,
            );

            TextareaSample { id, label, state }
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

/// Returns field samples that compose a textarea control.
pub fn field_textarea_samples(tokens: ThemeTokens) -> [FieldTextareaSample; 1] {
    [(
        "release-notes",
        "Release notes",
        "Summarize user-visible changes.",
        Some("Add a concise release note."),
        "",
        "Write release notes...",
        4,
        true,
        false,
        true,
    )]
    .map(
        |(id, label, help, error, value, placeholder, rows, required, disabled, invalid)| {
            let textarea = Textarea::new(format!("{id}-textarea"), label)
                .value(value)
                .placeholder(placeholder)
                .rows(rows)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = Field::new(id, format!("{id}-textarea"), label)
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

            FieldTextareaSample {
                id,
                state: field.state(),
                textarea_state: textarea.state(),
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

static VIRTUALIZED_LIST_SAMPLES: LazyLock<[VirtualizedListSample; 1]> =
    LazyLock::new(build_virtualized_list_samples);

/// Returns virtualized-list samples backed by the concrete renderer and virtualizer contract.
pub fn virtualized_list_samples(_tokens: ThemeTokens) -> &'static [VirtualizedListSample] {
    VIRTUALIZED_LIST_SAMPLES.as_slice()
}

fn build_virtualized_list_samples() -> [VirtualizedListSample; 1] {
    let size = Size::Small;
    let row_height = ui_px(28.0);
    let overscan = 4;
    let item_count = 10_000;
    let items = Arc::from(
        (0..item_count)
            .map(release_navigation_item)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let state = VirtualizedListState::resolve(size, false, item_count, Some(0), Some(0), Some(8))
        .with_metrics(
            VirtualizedListMetrics::from_size(size)
                .with_row_height(row_height)
                .with_overscan_count(overscan),
        );

    [VirtualizedListSample {
        id: "release-navigation",
        title: "Release navigation",
        summary: "Ten thousand stable options with a local virtualized viewport and keyboard reveal.",
        badge: "10k items",
        items,
        state,
        size,
        viewport_extent: ui_px(224.0),
        row_height,
        overscan,
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
    .with_state_summary()]
}

impl VirtualizedListSample {
    fn with_state_summary(self) -> Self {
        let plan = self.render_plan();
        Self {
            state_summary: VirtualizedListSampleStateSummary::from_plan(&plan),
            ..self
        }
    }
}

fn release_navigation_item(index: usize) -> VirtualizedListItemDescriptor {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];

    VirtualizedListItemDescriptor::new(
        format!("release-nav-{index:04}"),
        format!(
            "Release #{index:04} / {} / {}",
            teams[index % teams.len()],
            statuses[(index / 11) % statuses.len()]
        ),
    )
}

const RELEASE_MATRIX_METRIC_COUNT: usize = 14;

static TABLE_SAMPLES: LazyLock<Vec<TableSample>> = LazyLock::new(build_table_samples);

/// Returns table samples backed by real table and virtualizer contracts.
pub fn table_samples(_tokens: ThemeTokens) -> &'static [TableSample] {
    TABLE_SAMPLES.as_slice()
}

fn build_table_samples() -> Vec<TableSample> {
    let release_queue_rows = (0..10_000).map(release_queue_row).collect::<Vec<_>>();
    let filter_board_rows = (0..180).map(filter_board_row).collect::<Vec<_>>();
    let server_paged_rows = server_paged_rows();
    let release_resize_rows = (0..160).map(release_resize_row).collect::<Vec<_>>();
    let editable_release_rows = (0..32).map(editable_release_row).collect::<Vec<_>>();
    let toggle_release_rows = (0..28).map(toggle_release_row).collect::<Vec<_>>();
    let select_release_rows = (0..28).map(select_release_row).collect::<Vec<_>>();
    let multiline_release_rows = (0..24).map(multiline_release_row).collect::<Vec<_>>();
    let grouped_release_rows = (0..320).map(grouped_release_row).collect::<Vec<_>>();
    let grouped_custom_aggregation_rows = (0..8)
        .map(grouped_custom_aggregation_row)
        .collect::<Vec<_>>();
    let release_matrix_rows = (0..480).map(release_matrix_row).collect::<Vec<_>>();
    let row_pinning_rows = (0..96).map(row_pinning_row).collect::<Vec<_>>();
    let dependency_tree_rows = dependency_tree_rows();

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
    let server_paged = TableSample {
        id: "server-paged",
        title: "Server paged board",
        summary: "Manual filtering, sorting, and pagination render a server-owned page snapshot with total counts.",
        badge: "manual rows",
        state: TableState::new(server_paged_rows)
            .with_columns(table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_filters([TableFilter::contains("team", "missing")])
            .with_manual_filtering()
            .with_sorting([TableSort::ascending("score")])
            .with_manual_sorting()
            .with_selected_rows(["server-paged-row-0018"])
            .with_pagination(TablePagination::manual(2, 8, 64))
            .with_manual_facets([
                TableColumnFacets::manual("score", 64).with_numeric_range(1.0, 64.0),
                TableColumnFacets::manual("status", 64).with_unique_values([
                    TableFacetValueCount::new("Blocked", 16),
                    TableFacetValueCount::new("Queued", 16),
                    TableFacetValueCount::new("Ready", 16),
                    TableFacetValueCount::new("Review", 16),
                ]),
            ]),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let release_resize = TableSample {
        id: "release-resize",
        title: "Resizable release table",
        summary: "Controlled column widths with live resize handles and a fixed score column.",
        badge: "resizable",
        state: TableState::new(release_resize_rows)
            .with_columns(resizable_table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(188.0))
                    .with_width("team", ui_px(116.0))
                    .with_width("status", ui_px(132.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let content_fit_release = TableSample {
        id: "content-fit-release",
        title: "Content-fit release table",
        summary: "A fit-content identity column widens from visible edits while a fixed score column stays anchored.",
        badge: "content fit",
        state: TableState::new(editable_release_rows.clone())
            .with_columns(content_fit_release_table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_selected_rows(["editable-release-row-002"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let editable_release = TableSample {
        id: "editable-release",
        title: "Editable release cells",
        summary: "Text-cell editors emit controlled row/column payloads while app-owned rows feed updated values back into Table.",
        badge: "cell edit",
        state: TableState::new(editable_release_rows)
            .with_columns(editable_table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(204.0))
                    .with_width("team", ui_px(132.0))
                    .with_width("status", ui_px(128.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows(["editable-release-row-002"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let toggle_release = TableSample {
        id: "toggle-release",
        title: "Toggle release cells",
        summary: "Checkbox cell editors emit controlled bool payloads while app-owned rows feed updated values back into Table.",
        badge: "checkbox cells",
        state: TableState::new(toggle_release_rows)
            .with_columns(toggle_release_table_columns())
            .with_column_order(["name", "enabled", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(196.0))
                    .with_width("enabled", ui_px(104.0))
                    .with_width("status", ui_px(128.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows(["toggle-release-row-002"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let select_release = TableSample {
        id: "select-release",
        title: "Select release cells",
        summary: "Fixed-option select editors emit controlled choice payloads while app-owned rows feed updated values back into Table.",
        badge: "select cells",
        state: TableState::new(select_release_rows)
            .with_columns(select_release_table_columns())
            .with_column_order(["name", "status", "team", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(196.0))
                    .with_width("status", ui_px(132.0))
                    .with_width("team", ui_px(128.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows(["select-release-row-002"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let multiline_release = TableSample {
        id: "multiline-release",
        title: "Multiline release notes",
        summary: "Fixed-height textarea cell editors preserve newline edits while app-owned rows feed updated values back into Table.",
        badge: "textarea cells",
        state: TableState::new(multiline_release_rows)
            .with_columns(multiline_edit_table_columns())
            .with_column_order(["name", "notes", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(164.0))
                    .with_width("notes", ui_px(264.0))
                    .with_width("status", ui_px(112.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows(["multiline-release-row-002"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(220.0),
        row_height: ui_px(82.0),
        overscan: 3,
        state_summary: TableSampleStateSummary::default(),
    };
    let grouped_release = TableSample {
        id: "release-rollup",
        title: "Release rollup",
        summary: "Grouped release rows keep left and right lanes fixed while the wide center lane scrolls horizontally.",
        badge: "sticky pinned",
        state: TableState::new(grouped_release_rows)
            .with_columns(sticky_pinned_table_columns())
            .with_column_order(["name", "team", "score", "status"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_grouping(["team"])
            .with_expanded_rows(["group:team=UI", "group:team=Platform"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::sum("score"),
            ])
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["grouped-release-row-000"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let grouped_custom_aggregation = TableSample {
        id: "grouped-custom-aggregation",
        title: "Custom aggregation",
        summary: "Grouped rows combine a built-in count with a named custom score aggregate.",
        badge: "custom aggregate",
        state: TableState::new(grouped_custom_aggregation_rows)
            .with_columns(sticky_pinned_table_columns())
            .with_column_order(["name", "team", "score", "status"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_grouping(["team"])
            .with_expanded_rows(["group:team=UI", "group:team=Platform"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::named("score", "score_plus_one"),
            ])
            .with_aggregation_fn("score_plus_one", |column, rows| {
                let score = rows.iter().fold(0.0, |sum, row| match row.cell(column) {
                    Some(TableCellValue::Number(value)) => sum + *value,
                    _ => sum,
                });
                TableCellValue::Number(score + 1.0)
            })
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["grouped-custom-aggregation-row-000"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let release_matrix = TableSample {
        id: "release-matrix",
        title: "Release matrix",
        summary: "Nested release groups keep pinned identity and status lanes fixed around a wide virtualized center window.",
        badge: "column window",
        state: TableState::new(release_matrix_rows)
            .with_column_tree(release_matrix_column_tree())
            .with_column_order(release_matrix_column_order())
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_sorting([TableSort::descending("metric_13")])
            .with_selected_rows(["release-matrix-row-005"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let row_pinning = TableSample {
        id: "row-pinning",
        title: "Pinned row review",
        summary: "Top and bottom review rows stay visible while the paged center body scrolls.",
        badge: "row pins",
        state: TableState::new(row_pinning_rows)
            .with_columns(release_matrix_table_columns())
            .with_column_order(release_matrix_column_order())
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-pinning-row-003"])
                    .pinned_bottom(["row-pinning-row-030", "row-pinning-row-070"]),
            )
            .with_selected_rows(["row-pinning-row-030"])
            .with_pagination(TablePagination::new(2, 12)),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let dependency_tree = TableSample {
        id: "dependency-tree",
        title: "Dependency tree",
        summary: "Nested source rows expose controlled expansion, row focus, and activation payloads.",
        badge: "tree rows",
        state: TableState::new(dependency_tree_rows)
            .with_columns(dependency_tree_table_columns())
            .with_column_order(dependency_tree_column_order())
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(220.0))
                    .with_width("kind", ui_px(120.0))
                    .with_width("owner", ui_px(132.0))
                    .with_width("risk", ui_px(112.0))
                    .with_width("change", ui_px(148.0))
                    .with_width("score", ui_px(92.0))
                    .with_width("status", ui_px(132.0)),
            )
            .with_expanded_rows(["dependency-workspace"])
            .with_selected_rows(["dependency-ui"])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let server_tree = TableSample {
        id: "server-tree",
        title: "Server tree",
        summary: "Manual expansion keeps async child loading app-owned while Table renders branch metadata.",
        badge: "manual expansion",
        state: server_tree_table_state(false),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };

    vec![
        release_queue.with_state_summary(),
        filter_board.with_state_summary(),
        server_paged.with_state_summary(),
        release_resize.with_state_summary(),
        content_fit_release.with_state_summary(),
        editable_release.with_state_summary(),
        toggle_release.with_state_summary(),
        select_release.with_state_summary(),
        multiline_release.with_state_summary(),
        grouped_release.with_state_summary(),
        grouped_custom_aggregation.with_state_summary(),
        release_matrix.with_state_summary(),
        row_pinning.with_state_summary(),
        dependency_tree.with_state_summary(),
        server_tree.with_state_summary(),
    ]
}

impl TableSample {
    fn with_state_summary(self) -> Self {
        let plan = self
            .build_table()
            .diagnostics(UiPx::ZERO, self.viewport_extent);
        Self {
            state_summary: TableSampleStateSummary::from_plan(&plan, &self.state),
            ..self
        }
    }
}

fn editable_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_text_editable(true)
            .with_width(ui_px(204.0))
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("team", "Team")
            .with_text_editable(true)
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn toggle_release_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(196.0))
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(260.0)),
        TableColumn::new("enabled", "Enabled")
            .with_checkbox_editor()
            .with_width(ui_px(104.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(128.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn select_release_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(196.0))
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(260.0)),
        TableColumn::new("status", "Status")
            .with_select_editor([
                TableSelectOption::new("ready", "Ready"),
                TableSelectOption::new("blocked", "Blocked"),
            ])
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(108.0))
            .with_max_width(ui_px(184.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn multiline_edit_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(164.0))
            .with_min_width(ui_px(132.0))
            .with_max_width(ui_px(240.0)),
        TableColumn::new("notes", "Notes")
            .with_multiline_text_editor(3)
            .with_width(ui_px(264.0))
            .with_min_width(ui_px(220.0))
            .with_max_width(ui_px(360.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(112.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ]
}

fn sticky_pinned_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(188.0))
            .with_min_width(ui_px(144.0))
            .with_max_width(ui_px(280.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(220.0))
            .with_min_width(ui_px(128.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(164.0))
            .with_min_width(ui_px(120.0))
            .with_max_width(ui_px(240.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(180.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(220.0)),
    ]
}

fn resizable_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(188.0))
            .with_min_width(ui_px(140.0))
            .with_max_width(ui_px(280.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(116.0))
            .with_min_width(ui_px(92.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0))
            .with_resizable(false),
    ]
}

fn content_fit_release_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_text_editable(true)
            .with_content_fit()
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0))
            .with_resizable(false),
    ]
}

fn release_matrix_column_tree() -> Vec<TableColumnGroup> {
    vec![TableColumnGroup::new(
        "release",
        "Release",
        [
            TableColumnGroup::new(
                "identity",
                "Identity",
                [TableColumn::new("name", "Release")
                    .with_hideable(false)
                    .with_width(ui_px(172.0))
                    .with_min_width(ui_px(140.0))
                    .with_max_width(ui_px(260.0))],
            ),
            TableColumnGroup::new(
                "metrics",
                "Metrics",
                (0..RELEASE_MATRIX_METRIC_COUNT).map(|index| {
                    TableColumn::new(format!("metric_{index:02}"), format!("Metric {index:02}"))
                        .with_width(ui_px(92.0 + (index % 4) as f32 * 12.0))
                        .with_min_width(ui_px(72.0))
                        .with_max_width(ui_px(180.0))
                }),
            ),
            TableColumnGroup::new(
                "delivery",
                "Delivery",
                [TableColumn::new("status", "Status")
                    .with_hideable(false)
                    .with_width(ui_px(148.0))
                    .with_min_width(ui_px(112.0))
                    .with_max_width(ui_px(220.0))],
            ),
        ],
    )]
}

fn release_matrix_table_columns() -> Vec<TableColumn> {
    let mut columns = Vec::with_capacity(RELEASE_MATRIX_METRIC_COUNT + 2);
    columns.push(
        TableColumn::new("name", "Release")
            .with_hideable(false)
            .with_width(ui_px(172.0))
            .with_min_width(ui_px(140.0))
            .with_max_width(ui_px(260.0)),
    );
    columns.extend((0..RELEASE_MATRIX_METRIC_COUNT).map(|index| {
        let width = ui_px(92.0 + (index % 4) as f32 * 12.0);
        TableColumn::new(format!("metric_{index:02}"), format!("Metric {index:02}"))
            .with_width(width)
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(180.0))
    }));
    columns.push(
        TableColumn::new("status", "Status")
            .with_hideable(false)
            .with_width(ui_px(148.0))
            .with_min_width(ui_px(112.0))
            .with_max_width(ui_px(220.0)),
    );
    columns
}

fn release_matrix_column_order() -> Vec<String> {
    let mut order = Vec::with_capacity(RELEASE_MATRIX_METRIC_COUNT + 2);
    order.push("name".to_owned());
    order.extend((0..RELEASE_MATRIX_METRIC_COUNT).map(|index| format!("metric_{index:02}")));
    order.push("status".to_owned());
    order
}

fn dependency_tree_table_columns() -> [TableColumn; 7] {
    [
        TableColumn::new("name", "Package")
            .with_width(ui_px(220.0))
            .with_min_width(ui_px(172.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("kind", "Kind")
            .with_width(ui_px(120.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("owner", "Owner")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(200.0)),
        TableColumn::new("risk", "Risk")
            .with_width(ui_px(112.0))
            .with_min_width(ui_px(88.0))
            .with_max_width(ui_px(160.0)),
        TableColumn::new("change", "Change")
            .with_width(ui_px(148.0))
            .with_min_width(ui_px(112.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(92.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(132.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(188.0)),
    ]
}

fn dependency_tree_column_order() -> [&'static str; 7] {
    ["name", "kind", "owner", "risk", "change", "score", "status"]
}

fn dependency_tree_rows() -> Vec<TableRow> {
    vec![
        dependency_tree_row(
            "dependency-workspace",
            "open-gpui",
            "workspace",
            "Foundation",
            "medium",
            "tree table slice",
            91,
            "active",
        )
        .with_children([
            dependency_tree_row(
                "dependency-ui",
                "crates/ui_components",
                "crate",
                "Components",
                "high",
                "row interactions",
                88,
                "review",
            )
            .with_children([
                dependency_tree_row(
                    "dependency-ui-table",
                    "table/mod.rs",
                    "module",
                    "Components",
                    "high",
                    "tree affordance",
                    94,
                    "active",
                ),
                dependency_tree_row(
                    "dependency-ui-tree",
                    "tree.rs",
                    "module",
                    "Components",
                    "medium",
                    "navigation parity",
                    77,
                    "stable",
                ),
            ]),
            dependency_tree_row(
                "dependency-core",
                "crates/ui_core",
                "crate",
                "Foundation",
                "medium",
                "row model",
                84,
                "active",
            )
            .with_child(dependency_tree_row(
                "dependency-core-table",
                "table/mod.rs",
                "module",
                "Foundation",
                "medium",
                "source hierarchy",
                90,
                "ready",
            )),
            dependency_tree_row(
                "dependency-docs",
                "docs/ui",
                "docs",
                "Product",
                "low",
                "contract update",
                71,
                "queued",
            ),
        ]),
    ]
}

fn server_tree_table_state(loaded: bool) -> TableState {
    TableState::new(server_tree_rows(loaded))
        .with_columns(dependency_tree_table_columns())
        .with_column_order(dependency_tree_column_order())
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["status"]),
        )
        .with_column_sizing(
            TableColumnSizing::new()
                .with_width("name", ui_px(220.0))
                .with_width("kind", ui_px(120.0))
                .with_width("owner", ui_px(132.0))
                .with_width("risk", ui_px(112.0))
                .with_width("change", ui_px(148.0))
                .with_width("score", ui_px(92.0))
                .with_width("status", ui_px(132.0)),
        )
        .with_manual_expansion()
        .with_selected_rows(["server-workspace"])
        .with_pagination(TablePagination::disabled())
}

fn server_tree_rows(loaded: bool) -> Vec<TableRow> {
    let workspace_status = if loaded { "loaded" } else { "unloaded" };
    let mut workspace = dependency_tree_row(
        "server-workspace",
        "remote workspace",
        "workspace",
        "Platform",
        "medium",
        "server children",
        86,
        workspace_status,
    )
    .with_expandable(true);

    if loaded {
        workspace = workspace.with_children([
            dependency_tree_row(
                "server-api",
                "api gateway",
                "service",
                "Platform",
                "medium",
                "loaded child",
                82,
                "ready",
            ),
            dependency_tree_row(
                "server-workers",
                "worker queue",
                "service",
                "Runtime",
                "high",
                "manual expansion",
                79,
                "active",
            ),
        ]);
    }

    vec![
        workspace,
        dependency_tree_row(
            "server-cache",
            "cache prefetch",
            "remote",
            "Runtime",
            "medium",
            "async children",
            74,
            "loading",
        )
        .with_children_loading("Loading cached modules"),
        dependency_tree_row(
            "server-failed",
            "failed shard",
            "remote",
            "Platform",
            "high",
            "retry children",
            61,
            "retry",
        )
        .with_children_load_failed("Gateway timeout"),
    ]
}

#[allow(clippy::too_many_arguments)]
fn dependency_tree_row(
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    owner: &'static str,
    risk: &'static str,
    change: &'static str,
    score: usize,
    status: &'static str,
) -> TableRow {
    TableRow::new(id)
        .with_cell("name", name)
        .with_cell("kind", kind)
        .with_cell("owner", owner)
        .with_cell("risk", risk)
        .with_cell("change", change)
        .with_cell("score", score)
        .with_cell("status", status)
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

fn release_resize_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "QA"];
    let statuses = ["Queued", "Running", "Ready", "Held"];
    let score = 500_usize.saturating_sub(index % 500);

    TableRow::new(format!("release-resize-row-{index:03}"))
        .with_cell("name", format!("Resize candidate #{index:03}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 5) % statuses.len()])
        .with_cell("score", score)
}

fn editable_release_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "QA"];
    let statuses = ["Draft", "Review", "Ready", "Held"];
    let score = 320_usize.saturating_sub(index % 320);

    TableRow::new(format!("editable-release-row-{index:03}"))
        .with_cell("name", format!("Editable release {index:03}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 4) % statuses.len()])
        .with_cell("score", score)
}

fn toggle_release_row(index: usize) -> TableRow {
    let statuses = ["Draft", "Review", "Ready", "Held"];
    let score = 280_usize.saturating_sub(index % 280);

    TableRow::new(format!("toggle-release-row-{index:03}"))
        .with_cell("name", format!("Toggle release {index:03}"))
        .with_cell("enabled", index.is_multiple_of(2))
        .with_cell("status", statuses[(index / 4) % statuses.len()])
        .with_cell("score", score)
}

fn select_release_row(index: usize) -> TableRow {
    let statuses = ["ready", "blocked"];
    let teams = ["UI", "Runtime", "Platform", "QA"];
    let score = 260_usize.saturating_sub(index % 260);

    TableRow::new(format!("select-release-row-{index:03}"))
        .with_cell("name", format!("Select release {index:03}"))
        .with_cell("status", statuses[index % statuses.len()])
        .with_cell("team", teams[(index / 3) % teams.len()])
        .with_cell("score", score)
}

fn multiline_release_row(index: usize) -> TableRow {
    let statuses = ["Draft", "Review", "Ready", "Held"];
    let score = 240_usize.saturating_sub(index % 240);

    TableRow::new(format!("multiline-release-row-{index:03}"))
        .with_cell("name", format!("Release note {index:03}"))
        .with_cell(
            "notes",
            format!("User-visible summary {index:03}\nRollback: pending"),
        )
        .with_cell("status", statuses[(index / 3) % statuses.len()])
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

fn grouped_release_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let score = 500_usize.saturating_sub(index);

    TableRow::new(format!("grouped-release-row-{index:03}"))
        .with_cell("name", format!("Release rollup {index:03}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 9) % statuses.len()])
        .with_cell("score", score)
}

fn grouped_custom_aggregation_row(index: usize) -> TableRow {
    let status = ["Ready", "Review", "Blocked", "Verify"][index % 4];
    let (team, score) = match index {
        0..=3 => ("UI", index + 1),
        _ => ("Platform", (index - 3) * 10),
    };

    TableRow::new(format!("grouped-custom-aggregation-row-{index:03}"))
        .with_cell("name", format!("Custom aggregate {index:03}"))
        .with_cell("team", team)
        .with_cell("status", status)
        .with_cell("score", score)
}

fn release_matrix_row(index: usize) -> TableRow {
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let mut row = TableRow::new(format!("release-matrix-row-{index:03}"))
        .with_cell("name", format!("Train {index:03}"))
        .with_cell("status", statuses[(index / 13) % statuses.len()]);

    for metric in 0..RELEASE_MATRIX_METRIC_COUNT {
        row = row.with_cell(
            format!("metric_{metric:02}"),
            (index + 1) * (metric + 3) % 997,
        );
    }

    row
}

fn row_pinning_row(index: usize) -> TableRow {
    let statuses = ["Queued", "Ready", "Review", "Blocked"];
    let mut row = TableRow::new(format!("row-pinning-row-{index:03}"))
        .with_cell("name", format!("Review lane {index:03}"))
        .with_cell("status", statuses[(index / 4) % statuses.len()]);

    for metric in 0..RELEASE_MATRIX_METRIC_COUNT {
        row = row.with_cell(
            format!("metric_{metric:02}"),
            (index + 11) * (metric + 5) % 991,
        );
    }

    row
}

fn server_paged_rows() -> Vec<TableRow> {
    let teams = ["UI", "Runtime", "Platform", "Docs"];
    let statuses = ["Queued", "Ready", "Review", "Blocked"];
    let mut rows = Vec::with_capacity(8);

    for index in 16..24 {
        rows.push(
            TableRow::new(format!("server-paged-row-{index:04}"))
                .with_cell("name", format!("Page row {index:04}"))
                .with_cell("team", teams[index % teams.len()])
                .with_cell("status", statuses[(index / 2) % statuses.len()])
                .with_cell("score", 64 - index),
        );
    }

    rows
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
            summary: "Grouped listbox with shared roving navigation, typeahead, and one disabled option.",
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
            summary: "Open select keeps stable trigger selection distinct from popup active state.",
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
            summary: "Editable combobox keeps stable selected value while query filtering changes the visible list.",
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

static VIRTUALIZED_COMMAND_ITEMS: LazyLock<Arc<[CommandItemDescriptor]>> = LazyLock::new(|| {
    (0..10_000)
        .map(|index| {
            CommandItemDescriptor::new(
                format!("command-{index:04}"),
                format!("Command item {index:04}"),
            )
            .keyword(format!("release-{index:04}"))
        })
        .collect::<Vec<_>>()
        .into()
});

/// Returns command palette samples backed by real component state.
pub fn command_samples(tokens: ThemeTokens) -> [CommandSample; 4] {
    let ranked_items: Arc<[CommandItemDescriptor]> = vec![
        CommandItemDescriptor::new("archive", "Archive").keyword("file"),
        CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"),
        CommandItemDescriptor::new("file-action", "Launcher").shortcut("Ctrl+L"),
    ]
    .into();
    let ranked_groups: Arc<[CommandGroupDescriptor]> = vec![
        CommandGroupDescriptor::new("view", "View").item(
            CommandItemDescriptor::new("toggle-sidebar", "Toggle Sidebar")
                .keyword("layout")
                .shortcut("Ctrl+B"),
        ),
    ]
    .into();
    let multi_items: Arc<[CommandItemDescriptor]> = vec![
        CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"),
        CommandItemDescriptor::new("new-file", "New File").shortcut("Ctrl+N"),
        CommandItemDescriptor::new("delete-file", "Delete File").disabled(true),
    ]
    .into();
    let virtualized_items = VIRTUALIZED_COMMAND_ITEMS.clone();
    let indexed_snapshot = CommandIndexSnapshot::new("workspace-index-v3")
        .mode(CommandIndexSnapshotMode::PreRankedFilter)
        .loading(CommandLoadingState::new(
            "Refreshing command index",
            Some(45),
        ))
        .item(CommandItemDescriptor::new("recent-open", "Open Recent").keyword("file"))
        .item(CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"))
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .group(
            CommandGroupDescriptor::new("workspace", "Workspace")
                .item(CommandItemDescriptor::new("switch-window", "Switch Window"))
                .item(CommandItemDescriptor::new("close-window", "Close Window").disabled(true)),
        );

    [
        command_sample_from_local(
            "ranked-search",
            "Ranked query keeps stable selected value while label and value matches outrank keyword-only commands.",
            Size::Medium,
            false,
            Some(true),
            false,
            "Ranked commands",
            "Search commands",
            "file",
            CommandSelectionMode::Single,
            Some("open-file"),
            Vec::<String>::new(),
            Some("open-file"),
            None,
            ranked_items,
            ranked_groups,
            true,
            8,
            None,
            6,
            tokens,
        ),
        command_sample_from_local(
            "multi-select",
            "Multi-select keeps selected chips even when query filtering hides a command.",
            Size::Small,
            false,
            Some(true),
            false,
            "Bulk commands",
            "Filter commands",
            "new",
            CommandSelectionMode::Multiple,
            None,
            vec!["open-file".to_string(), "new-file".to_string()],
            Some("new-file"),
            None,
            multi_items,
            Arc::from([]),
            false,
            6,
            None,
            4,
            tokens,
        ),
        command_sample_from_local(
            "virtualized-index",
            "Ten-thousand command results render through the fixed-row virtualizer.",
            Size::Small,
            false,
            Some(true),
            false,
            "Virtualized commands",
            "Search large index",
            "",
            CommandSelectionMode::Single,
            Some("command-0000"),
            Vec::<String>::new(),
            Some("command-0000"),
            None,
            virtualized_items,
            Arc::from([]),
            false,
            7,
            Some(ui_px(28.0)),
            4,
            tokens,
        ),
        command_sample_from_snapshot(
            "indexed-loading",
            "App-owned pre-ranked snapshot carries revision and loading metadata without a registry.",
            Size::Small,
            false,
            Some(true),
            false,
            "Indexed commands",
            "Search indexed commands",
            "file",
            CommandSelectionMode::Single,
            Some("open-file"),
            Vec::<String>::new(),
            Some("recent-open"),
            indexed_snapshot,
            false,
            6,
            None,
            4,
            tokens,
        ),
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

#[allow(clippy::too_many_arguments)]
fn command_sample_from_local(
    id: &'static str,
    summary: &'static str,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selection_mode: CommandSelectionMode,
    selected: Option<&str>,
    selected_values: Vec<String>,
    active: Option<&str>,
    loading: Option<CommandLoadingState>,
    items: Arc<[CommandItemDescriptor]>,
    groups: Arc<[CommandGroupDescriptor]>,
    dialog: bool,
    viewport_item_count: usize,
    row_height: Option<UiPx>,
    overscan: usize,
    tokens: ThemeTokens,
) -> CommandSample {
    let state = CommandState::resolve(
        size,
        disabled,
        open,
        default_open,
        dialog,
        label,
        placeholder,
        query,
        CommandQueryMode::Uncontrolled,
        selection_mode,
        selected,
        selected_values.iter().cloned(),
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
    );
    CommandSample {
        id,
        summary,
        items,
        groups,
        index_snapshot: None,
        selected_values: selected_values.into(),
        viewport_item_count,
        row_height,
        overscan,
        state,
    }
}

#[allow(clippy::too_many_arguments)]
fn command_sample_from_snapshot(
    id: &'static str,
    summary: &'static str,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selection_mode: CommandSelectionMode,
    selected: Option<&str>,
    selected_values: Vec<String>,
    active: Option<&str>,
    snapshot: CommandIndexSnapshot,
    dialog: bool,
    viewport_item_count: usize,
    row_height: Option<UiPx>,
    overscan: usize,
    tokens: ThemeTokens,
) -> CommandSample {
    let state = CommandState::resolve_from_index_snapshot(
        size,
        disabled,
        open,
        default_open,
        dialog,
        label,
        placeholder,
        query,
        CommandQueryMode::Uncontrolled,
        selection_mode,
        selected,
        selected_values.iter().cloned(),
        active,
        None,
        "No results",
        dialog.then_some("Command palette".to_string()),
        dialog.then_some("Run a workspace command".to_string()),
        snapshot.clone(),
        OutsidePressPolicy::DismissAndConsume,
        EscapeKeyPolicy::Dismiss,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        tokens,
    );
    CommandSample {
        id,
        summary,
        items: Arc::from([]),
        groups: Arc::from([]),
        index_snapshot: Some(snapshot),
        selected_values: selected_values.into(),
        viewport_item_count,
        row_height,
        overscan,
        state,
    }
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
