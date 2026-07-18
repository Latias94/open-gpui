//! Canonical product metadata for official components.

use super::super::ComponentContractEntry;

/// Scenarios required for every official component contract.
///
/// The test-side artifacts own package, target, test, and contract binding coordinates. These ids
/// express only the product requirement that every component participates in both projections.
pub const COMPONENT_CONTRACT_GLOBAL_SCENARIOS: &[&str] = &[
    "gallery.component-contract.metadata",
    "public-api.component-contract.exports",
];

/// Product-level metadata for the 48 official component contracts.
pub const COMPONENT_CONTRACT_ROWS: &[ComponentContractEntry] = &[
    ComponentContractEntry::new("Accordion", 1, "disclosure"),
    ComponentContractEntry::new("Button", 1, "action")
        .with_required_scenarios(&["a11y.button.final-tree-actions"]),
    ComponentContractEntry::new("Badge", 1, "display"),
    ComponentContractEntry::new("Collapsible", 1, "disclosure"),
    ComponentContractEntry::new("Link", 1, "navigation"),
    ComponentContractEntry::new("Breadcrumb", 1, "navigation"),
    ComponentContractEntry::new("Tag", 1, "display"),
    ComponentContractEntry::new("ToastStack", 1, "feedback"),
    ComponentContractEntry::new("IconButton", 1, "action"),
    ComponentContractEntry::new("Slider", 1, "form")
        .with_required_scenarios(&["a11y.numeric-controls.final-tree-actions"]),
    ComponentContractEntry::new("NumberInput", 1, "form")
        .with_required_scenarios(&["a11y.numeric-controls.final-tree-actions"]),
    ComponentContractEntry::new("Switch", 1, "form"),
    ComponentContractEntry::new("Checkbox", 1, "form")
        .with_required_scenarios(&["a11y.checkbox.final-tree-actions"]),
    ComponentContractEntry::new("RadioGroup", 1, "choice"),
    ComponentContractEntry::new("Toggle", 1, "action"),
    ComponentContractEntry::new("ToggleGroup", 1, "action"),
    ComponentContractEntry::new("Toolbar", 1, "shell"),
    ComponentContractEntry::new("Sidebar", 1, "shell"),
    ComponentContractEntry::new("Tree", 1, "hierarchy")
        .with_required_scenarios(&["a11y.tree.final-tree-actions"]),
    ComponentContractEntry::new("Listbox", 1, "choice")
        .with_required_scenarios(&["a11y.listbox.final-tree-actions"]),
    ComponentContractEntry::new("Select", 1, "choice")
        .with_required_scenarios(&["a11y.select.final-tree-actions"]),
    ComponentContractEntry::new("Combobox", 1, "choice-search"),
    ComponentContractEntry::new("Command", 1, "choice-search"),
    ComponentContractEntry::new("Label", 1, "form"),
    ComponentContractEntry::new("TextInput", 1, "form")
        .with_required_scenarios(&["gallery.focus-a11y.devtools-projection"]),
    ComponentContractEntry::new("Textarea", 1, "form")
        .with_required_scenarios(&["gallery.focus-a11y.devtools-projection"]),
    ComponentContractEntry::new("Field", 1, "form").with_required_scenarios(&[
        "a11y.field.final-tree-relations",
        "gallery.focus-a11y.devtools-projection",
    ]),
    ComponentContractEntry::new("Tabs", 1, "navigation")
        .with_required_scenarios(&["a11y.tabs.final-tree-actions"]),
    ComponentContractEntry::new("ScrollArea", 1, "layout"),
    ComponentContractEntry::new("Splitter", 1, "layout")
        .with_required_scenarios(&["a11y.splitter.final-tree-actions"]),
    ComponentContractEntry::new("Table", 1, "data")
        .with_required_scenarios(&["a11y.table.final-tree-identity"]),
    ComponentContractEntry::new("VirtualizedList", 1, "data")
        .with_required_scenarios(&["a11y.virtualized-list.final-tree-recycle"]),
    ComponentContractEntry::new("StatusCue", 1, "feedback"),
    ComponentContractEntry::new("EmptyState", 1, "feedback"),
    ComponentContractEntry::new("Separator", 1, "layout")
        .with_required_scenarios(&["a11y.separator.final-tree-projection"]),
    ComponentContractEntry::new("Kbd", 1, "display"),
    ComponentContractEntry::new("Progress", 1, "status")
        .with_required_scenarios(&["a11y.numeric-controls.final-tree-actions"]),
    ComponentContractEntry::new("Skeleton", 1, "status"),
    ComponentContractEntry::new("Avatar", 1, "identity"),
    ComponentContractEntry::new("AvatarGroup", 1, "identity"),
    ComponentContractEntry::new("Tooltip", 1, "overlay"),
    ComponentContractEntry::new("HoverCard", 1, "overlay"),
    ComponentContractEntry::new("Popover", 1, "overlay"),
    ComponentContractEntry::new("Dialog", 1, "overlay")
        .with_required_scenarios(&["a11y.dialog.final-tree-actions"]),
    ComponentContractEntry::new("AlertDialog", 1, "overlay"),
    ComponentContractEntry::new("Sheet", 1, "overlay"),
    ComponentContractEntry::new("Menu", 1, "overlay"),
    ComponentContractEntry::new("ContextMenu", 1, "overlay"),
];
