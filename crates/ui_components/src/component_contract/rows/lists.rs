//! Compatibility and family marker lists for contract rows.

/// Official overlay component rows in the API inventory.
pub const OFFICIAL_OVERLAY_COMPONENTS: &[&str] = &[
    "Tooltip",
    "HoverCard",
    "Popover",
    "Dialog",
    "AlertDialog",
    "Sheet",
    "Menu",
    "ContextMenu",
];

/// Component recipe rows that belong to a larger official family.
pub const COMPONENT_RECIPE_COMPONENTS: &[&str] = &[
    "TableColumnVisibility",
    "TableFacetedFilter",
    "TableGlobalFilter",
    "TablePredicateFilter",
    "TableRangeFilter",
    "TableToolbar",
];
