//! Component catalog metadata for the foundation gallery.

use crate::story::{StoryContract, StoryContractKind, StoryProbeContract, StoryProbeOperation::*};
use open_gpui_ui_components::component_contract::{
    SurfaceGalleryStatus, component_contract_family, component_contract_gallery_status,
};

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
    entry.story_contract().and_then(|story| story.section_id())
}

fn story_section_for_catalog_entry(entry: &ComponentCatalogEntry) -> Option<&'static str> {
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
    /// Maps contract-owned gallery status into the Components page status vocabulary.
    pub const fn from_contract(status: SurfaceGalleryStatus) -> Self {
        match status {
            SurfaceGalleryStatus::OfficialComponent => Self::Official,
            SurfaceGalleryStatus::AdapterOnly => Self::AdapterOnly,
            SurfaceGalleryStatus::InternalAnatomy => Self::InternalAnatomy,
            SurfaceGalleryStatus::StateContract => Self::StateContract,
            SurfaceGalleryStatus::OfficialOverlay | SurfaceGalleryStatus::NotInGallery => {
                Self::Deferred
            }
        }
    }

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
    /// Creates a rendered catalog entry with a stable sample selector.
    pub const fn contract_sample(
        name: &'static str,
        family: &'static str,
        state: &'static str,
        coverage: &'static str,
        sample_selector: &'static str,
    ) -> Self {
        Self {
            name,
            status: ComponentCatalogStatus::from_contract(component_contract_gallery_status(name)),
            family: contract_family_or(name, family),
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
            status: ComponentCatalogStatus::from_contract(component_contract_gallery_status(name)),
            family: contract_family_or(name, family),
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
            status: ComponentCatalogStatus::from_contract(component_contract_gallery_status(name)),
            family: contract_family_or(name, family),
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
            status: ComponentCatalogStatus::from_contract(component_contract_gallery_status(name)),
            family: contract_family_or(name, family),
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

    /// Returns the gallery story contract represented by this catalog entry, if any.
    pub fn story_contract(self) -> Option<StoryContract> {
        match self.status {
            ComponentCatalogStatus::Official => Some(StoryContract::component(
                StoryContractKind::Component,
                self.name,
                self.family,
                self.state,
                story_section_for_catalog_entry(&self),
                self.catalog_selector(),
                self.sample_selector,
                official_story_state_readout_selector(self.name),
                component_story_probes(&self),
            )),
            ComponentCatalogStatus::StateContract => Some(StoryContract::component(
                StoryContractKind::StateContract,
                self.name,
                self.family,
                self.state,
                story_section_for_catalog_entry(&self),
                self.catalog_selector(),
                None,
                self.state_contract_selector,
                component_story_probes(&self),
            )),
            ComponentCatalogStatus::AdapterOnly
            | ComponentCatalogStatus::InternalAnatomy
            | ComponentCatalogStatus::Deferred => None,
        }
    }
}

const fn contract_family_or(name: &'static str, fallback: &'static str) -> &'static str {
    match component_contract_family(name) {
        Some(family) => family,
        None => fallback,
    }
}

fn official_story_state_readout_selector(name: &'static str) -> Option<&'static str> {
    match name {
        "Listbox" => Some("gallery:component-listbox-sample:assignee-listbox:state"),
        "Select" => Some("gallery:component-select-sample:priority-select:state"),
        "Combobox" => Some("gallery:component-combobox-sample:framework-combobox:state"),
        "Command" => Some("gallery:component-command-sample:ranked-search:state"),
        _ => None,
    }
}

/// Official component catalog and adjacent public surfaces.
pub const COMPONENT_CATALOG: &[ComponentCatalogEntry] = &[
    ComponentCatalogEntry::contract_sample(
        "Button",
        "action",
        "ButtonState",
        "exports / gallery / state tests",
        "gallery:component-button-sample:default",
    ),
    ComponentCatalogEntry::contract_sample(
        "Badge",
        "display",
        "BadgeState",
        "exports / gallery / state tests",
        "gallery:component-badge-sample:default",
    ),
    ComponentCatalogEntry::contract_sample(
        "Accordion",
        "disclosure",
        "AccordionState",
        "exports / gallery / state tests",
        "gallery:component-accordion-sample:shipping",
    ),
    ComponentCatalogEntry::contract_sample(
        "Collapsible",
        "disclosure",
        "CollapsibleState",
        "exports / gallery / state tests",
        "gallery:component-collapsible-sample:release-notes",
    ),
    ComponentCatalogEntry::contract_sample(
        "Slider",
        "form",
        "SliderState",
        "exports / gallery / keyboard tests",
        "gallery:component-slider-sample:volume",
    ),
    ComponentCatalogEntry::contract_sample(
        "NumberInput",
        "form",
        "NumberInputState",
        "exports / gallery / stepper tests",
        "gallery:component-number-input-sample:workers",
    ),
    ComponentCatalogEntry::contract_sample(
        "ToggleGroup",
        "action",
        "ToggleGroupState",
        "exports / gallery / stable value tests",
        "gallery:component-toggle-group-sample:alignment",
    ),
    ComponentCatalogEntry::contract_sample(
        "Link",
        "navigation",
        "LinkState",
        "exports / gallery / activation tests",
        "gallery:component-link-sample:docs",
    ),
    ComponentCatalogEntry::contract_sample(
        "Breadcrumb",
        "navigation",
        "BreadcrumbState",
        "exports / gallery / activation tests",
        "gallery:component-breadcrumb-sample:project",
    ),
    ComponentCatalogEntry::contract_sample(
        "Tag",
        "display",
        "TagState",
        "exports / gallery / remove tests",
        "gallery:component-tag-sample:ready",
    ),
    ComponentCatalogEntry::contract_sample(
        "ToastStack",
        "feedback",
        "ToastStackState",
        "exports / gallery / stack tests",
        "gallery:component-toast-stack-sample:notifications",
    ),
    ComponentCatalogEntry::contract_sample(
        "IconButton",
        "action",
        "IconButtonState",
        "exports / gallery / a11y metadata",
        "gallery:component-icon-button-sample:search",
    ),
    ComponentCatalogEntry::contract_sample(
        "Switch",
        "form",
        "SwitchState",
        "exports / gallery / state tests",
        "gallery:component-switch-sample:off",
    ),
    ComponentCatalogEntry::contract_sample(
        "Checkbox",
        "form",
        "CheckboxState",
        "exports / gallery / state tests",
        "gallery:component-checkbox-sample:unchecked",
    ),
    ComponentCatalogEntry::contract_sample(
        "RadioGroup",
        "choice",
        "RadioGroupState",
        "exports / gallery / runtime smoke",
        "gallery:component-radio-sample:persona-radios",
    ),
    ComponentCatalogEntry::contract_sample(
        "Toggle",
        "action",
        "ToggleState",
        "exports / gallery / state tests",
        "gallery:component-toggle-sample:ghost-off",
    ),
    ComponentCatalogEntry::contract_sample(
        "Toolbar",
        "shell",
        "ToolbarState",
        "exports / gallery / runtime smoke",
        "gallery:component-toolbar-sample:editor-toolbar",
    ),
    ComponentCatalogEntry::contract_sample(
        "Sidebar",
        "shell",
        "SidebarState",
        "exports / gallery / scroll smoke",
        "gallery:component-sidebar-sample:workspace-sidebar",
    ),
    ComponentCatalogEntry::contract_sample(
        "Tree",
        "hierarchy",
        "TreeState",
        "exports / gallery / tree runtime smoke",
        "gallery:component-tree-sample:document-outline",
    ),
    ComponentCatalogEntry::contract_sample(
        "Listbox",
        "choice",
        "ListboxState",
        "exports / gallery / shared navigation smoke",
        "gallery:component-listbox-sample:assignee-listbox",
    ),
    ComponentCatalogEntry::contract_sample(
        "Select",
        "choice",
        "SelectState",
        "exports / gallery / stable value smoke",
        "gallery:component-select-sample:priority-select",
    ),
    ComponentCatalogEntry::contract_sample(
        "Combobox",
        "choice-search",
        "ComboboxState",
        "exports / gallery / stable value smoke",
        "gallery:component-combobox-sample:framework-combobox",
    ),
    ComponentCatalogEntry::contract_sample(
        "Command",
        "choice-search",
        "CommandState",
        "exports / gallery / stable value and runtime smoke",
        "gallery:component-command-sample:ranked-search",
    ),
    ComponentCatalogEntry::contract_sample(
        "Label",
        "form",
        "LabelState",
        "exports / gallery / a11y metadata",
        "gallery:component-label-sample:email",
    ),
    ComponentCatalogEntry::contract_sample(
        "TextInput",
        "form",
        "TextInputState",
        "exports / gallery / controller tests",
        "gallery:component-text-input-sample:default",
    ),
    ComponentCatalogEntry::contract_sample(
        "Textarea",
        "form",
        "TextareaState",
        "exports / gallery / controlled multiline tests",
        "gallery:component-textarea-sample:default",
    ),
    ComponentCatalogEntry::contract_sample(
        "Field",
        "form",
        "FieldState",
        "exports / gallery / composition tests",
        "gallery:component-field-sample:email",
    ),
    ComponentCatalogEntry::contract_sample(
        "Tabs",
        "navigation",
        "TabsState",
        "exports / gallery / runtime smoke",
        "gallery:component-tabs-sample:overview-tabs",
    ),
    ComponentCatalogEntry::contract_sample(
        "ScrollArea",
        "layout",
        "ScrollAreaState",
        "exports / gallery / redraw smoke",
        "gallery:component-scroll-area-sample:activity-log",
    ),
    ComponentCatalogEntry::contract_sample(
        "Splitter",
        "layout",
        "SplitterState",
        "exports / gallery / drag smoke",
        "gallery:component-splitter-sample:workspace-split",
    ),
    ComponentCatalogEntry::contract_sample(
        "Table",
        "data",
        "TableState",
        "exports / gallery / virtualized scroll and resize smoke",
        "gallery:component-table-sample:release-queue",
    ),
    ComponentCatalogEntry::contract_sample(
        "VirtualizedList",
        "data",
        "VirtualizedListState",
        "exports / gallery / virtualized scroll smoke",
        "gallery:component-virtualized-list-sample:release-navigation",
    ),
    ComponentCatalogEntry::contract_sample(
        "StatusCue",
        "feedback",
        "StatusCueState",
        "exports / gallery / token intents",
        "gallery:component-status-cue-sample:sync-warning",
    ),
    ComponentCatalogEntry::contract_sample(
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
    ComponentCatalogEntry::contract_sample(
        "Separator",
        "layout",
        "SeparatorState",
        "exports / gallery / state tests",
        "gallery:component-separator-sample:section-rule",
    ),
    ComponentCatalogEntry::contract_sample(
        "Kbd",
        "display",
        "KbdState",
        "exports / gallery / state tests",
        "gallery:component-kbd-sample:command-palette",
    ),
    ComponentCatalogEntry::contract_sample(
        "Progress",
        "status",
        "ProgressState",
        "exports / gallery / state tests",
        "gallery:component-progress-sample:sync",
    ),
    ComponentCatalogEntry::contract_sample(
        "Skeleton",
        "status",
        "SkeletonState",
        "exports / gallery / state tests",
        "gallery:component-skeleton-sample:body-line",
    ),
    ComponentCatalogEntry::contract_sample(
        "Avatar",
        "identity",
        "AvatarState",
        "exports / gallery / state tests",
        "gallery:component-avatar-sample:ada",
    ),
    ComponentCatalogEntry::contract_sample(
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
    component_story_contracts().into_iter().filter_map(|story| {
        if story.kind() == StoryContractKind::Component {
            story
                .selectors()
                .sample_selector()
                .map(|selector| (story.owner_name(), selector))
        } else {
            None
        }
    })
}

/// Returns renderer-neutral state contracts that own visible gallery readouts.
pub fn state_contract_readout_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    component_story_contracts().into_iter().filter_map(|story| {
        if story.kind() == StoryContractKind::StateContract {
            story
                .selectors()
                .state_readout_selector()
                .map(|selector| (story.owner_name(), selector))
        } else {
            None
        }
    })
}

const ACTION_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Activate, "primary control", "activation state"),
    StoryProbeContract::new(Focus, "sample", "focusable control"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved component state"),
];

const CHOICE_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "trigger", "choice popup or active option"),
    StoryProbeContract::new(Select, "option", "selected value"),
    StoryProbeContract::new(Focus, "active option", "roving focus"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved choice state"),
];

const TEXT_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Edit, "input", "edited text"),
    StoryProbeContract::new(Focus, "input", "input focus"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved text state"),
];

const TABLE_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Scroll, "body viewport", "stable sample position"),
    StoryProbeContract::new(Select, "row or cell", "row activation or cell payload"),
    StoryProbeContract::new(Edit, "cell editor", "cell edit payload"),
    StoryProbeContract::new(Open, "table filter or select editor", "popup surface"),
    StoryProbeContract::new(ReadPublicPayload, "runtime log", "table callback payload"),
];

const TREE_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "disclosure", "expanded branch"),
    StoryProbeContract::new(Select, "tree item", "selection payload"),
    StoryProbeContract::new(Scroll, "tree viewport", "stable sample position"),
    StoryProbeContract::new(Focus, "tree item", "roving focus"),
    StoryProbeContract::new(ReadPublicPayload, "runtime log", "tree callback payload"),
];

const VIRTUALIZED_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Scroll, "virtualized viewport", "windowed rows"),
    StoryProbeContract::new(Activate, "active row", "activation payload"),
    StoryProbeContract::new(Focus, "list root", "keyboard focus"),
    StoryProbeContract::new(ReadPublicPayload, "state", "virtualized state summary"),
];

const STATE_CONTRACT_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Select, "state row", "selection metadata"),
    StoryProbeContract::new(Focus, "state row", "focus metadata"),
    StoryProbeContract::new(
        ReadPublicPayload,
        "readout",
        "renderer-neutral state contract",
    ),
];

const DISPLAY_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Focus, "sample", "accessible role metadata"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved display state"),
];

fn component_story_probes(entry: &ComponentCatalogEntry) -> &'static [StoryProbeContract] {
    match entry.status {
        ComponentCatalogStatus::StateContract => STATE_CONTRACT_STORY_PROBES,
        ComponentCatalogStatus::Official => match entry.name {
            "Listbox" | "Select" | "Combobox" | "Command" | "RadioGroup" | "ToggleGroup"
            | "Tabs" | "Toolbar" | "Sidebar" => CHOICE_STORY_PROBES,
            "TextInput" | "Textarea" | "Field" | "NumberInput" | "Slider" => TEXT_STORY_PROBES,
            "Table" => TABLE_STORY_PROBES,
            "Tree" => TREE_STORY_PROBES,
            "VirtualizedList" => VIRTUALIZED_STORY_PROBES,
            "Button" | "IconButton" | "Switch" | "Checkbox" | "Toggle" | "Accordion"
            | "Collapsible" | "Link" | "Breadcrumb" | "Tag" | "ToastStack" => ACTION_STORY_PROBES,
            _ => DISPLAY_STORY_PROBES,
        },
        ComponentCatalogStatus::AdapterOnly
        | ComponentCatalogStatus::InternalAnatomy
        | ComponentCatalogStatus::Deferred => &[],
    }
}

/// Returns user-observable story contracts for official component samples and state readouts.
pub fn component_story_contracts() -> Vec<StoryContract> {
    COMPONENT_CATALOG
        .iter()
        .filter_map(|entry| entry.story_contract())
        .collect()
}

/// Returns the story contract for one official component or state readout.
pub fn component_story_contract_for(name: &str) -> Option<StoryContract> {
    COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == name)
        .and_then(|entry| entry.story_contract())
}

/// Returns story contracts visible for a Components page focus mode.
pub fn component_story_contracts_for_focus(mode: ComponentFocusMode) -> Vec<StoryContract> {
    match mode {
        ComponentFocusMode::All => component_story_contracts(),
        ComponentFocusMode::Section(section_id) => component_story_contracts()
            .into_iter()
            .filter(|story| story.section_id() == Some(section_id))
            .collect(),
    }
}
