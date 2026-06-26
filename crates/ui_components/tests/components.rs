use open_gpui::{
    Anchor, AppContext, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, ScrollDelta, ScrollWheelEvent, Styled, Window, div, point, px,
};
use open_gpui_ui_components::{
    AlertDialog, AlertDialogActionKind, AlertDialogIntent, AlertDialogOpenMode, Avatar,
    AvatarGroup, AvatarGroupCount, Badge, BadgeVariant, Button, ButtonVariant, Checkbox,
    ColorIntent, ColorState, Combobox, ComboboxGroup, ComboboxOpenMode, ComboboxOption,
    ComboboxSelection, Command, CommandGroup, CommandGroupDescriptor, CommandIndexSnapshot,
    CommandIndexSnapshotMode, CommandItem, CommandItemDescriptor, CommandLoadingState,
    CommandMatchSource, CommandOpenMode, CommandQueryMode, CommandSelection,
    CommandSelectionChange, CommandSelectionMode, ContextMenu, DEFAULT_FOCUS_RING_WIDTH, Dialog,
    DialogOpenMode, EmptyState, FeedbackIntent, Field, FocusRing, HoverCard, HoverCardContentKind,
    HoverCardDelayPolicy, HoverCardOpenIntent, HoverCardOpenMode, IconButton, Kbd, Label, Listbox,
    ListboxGroup, ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor,
    ListboxOptionKind, ListboxSelection, ListboxState, Menu, MenuItem, MenuItemKind, MenuOpenMode,
    MenuSelection, Popover, PopoverOpenMode, Progress, ProgressVisualMode, RadioGroup,
    RadioGroupState, RadioItem, RadioItemDescriptor, RadioSelection, ScrollArea, ScrollAreaAxis,
    ScrollAreaState, ScrollResetPolicy, Select, SelectOpenMode, SelectSelection, Separator, Sheet,
    SheetCloseAffordance, SheetModalMode, SheetOpenMode, SheetSide, Sidebar, SidebarCollapseMode,
    SidebarItem, SidebarItemDescriptor, SidebarSection, SidebarSectionDescriptor, SidebarSide,
    SidebarState, SidebarVariant, Skeleton, Splitter, SplitterPanel, SplitterPanelDescriptor,
    SplitterState, StatusCue, Switch, Table, TableCellEditApplyOutcome, TableCellEditChange,
    TableCellEditor, TableCellValue, TableCenterColumnWindowPlan, TableColumn, TableColumnFacets,
    TableColumnGroup, TableColumnId, TableColumnPinning, TableColumnRegion, TableColumnResizeMode,
    TableColumnSizing, TableColumnSizingChange, TableColumnVisibility, TableColumnVisibilityAction,
    TableColumnVisibilityChange, TableColumnVisibilityOverrides, TableColumnVisibilityState,
    TableExpansionMode, TableFacetValueCount, TableFacetedFilter, TableFacetedFilterChange,
    TableFacetedFilterState, TableFilter, TableGlobalFacetSummary, TableGlobalFilter,
    TableGlobalFilterChange, TableGlobalFilterState, TableHeaderAction, TableNumericFilterOperator,
    TablePagination, TablePredicateFilter, TablePredicateFilterChange,
    TablePredicateFilterOperator, TablePredicateFilterOperatorOptionState,
    TablePredicateFilterState, TableRangeFilter, TableRangeFilterChange, TableRangeFilterState,
    TableResolvedHeaderKind, TableRow, TableRowChildrenLoadState, TableRowId, TableRowPinning,
    TableRowPinningPolicy, TableRowRegion, TableSelectionActivationMode, TableSelectionMode,
    TableSelectionScope, TableSort, TableSortDirection, TableStageMode, TableState,
    TableTextFilterOperator, TableToolbar, TableToolbarState, Tabs, TabsActivationMode, TabsItem,
    TabsItemDescriptor, TabsSelection, TabsState, TextInput, TextInputDisplayMode, Textarea,
    ThemeColor, ThemeMode, ThemeResolver, ThemeSnapshot, Toggle, ToggleVariant, Toolbar,
    ToolbarItem, ToolbarItemDescriptor, ToolbarItemKind, ToolbarSelection, ToolbarState, Tooltip,
    TooltipContentKind, TooltipDelayPolicy, TooltipOpenIntent, Tree, TreeChildrenLoadState,
    TreeDropPosition, TreeItemDescriptor, TreeMove, TreeMoveTarget, TreeRenderPlan,
    TreeRowRenderPlan, VirtualizedList, VirtualizedListActivation, VirtualizedListItemDescriptor,
    VirtualizedListRenderPlan, VirtualizedListRowRenderPlan, VirtualizedListScrollStrategy,
    VirtualizedListState, VirtualizerItemKey, VirtualizerRange, VirtualizerSnapshot,
    VirtualizerSnapshotItem, VirtualizerState, active_index_from_str_keys, apply_tree_move,
    first_enabled,
    gpui_adapter::{
        DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement,
        TextInputController, default_deferred_priority, escape_open_change, focus_ring_shadow,
        gpui_anchor, gpui_role_from_ui, init_text_input, outside_press_open_change,
        point_anchor_placement,
    },
    last_enabled, listbox_navigation_target, menu_navigation_target, next_enabled,
    sidebar_navigation_target, toolbar_navigation_target, virtualized_list_scroll_target,
};
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, Orientation,
    OutsidePressPolicy, OverlayAnchorInput, OverlayLayerKind, OverlayLayerPolicy,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Role,
    Sizable, Size, ThemeTokens, Toggled, TokenKey, UiPoint, UiPx, UiSize, rect, semantic, ui_point,
    ui_px, ui_size,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const TEST_SURFACE: TokenKey = TokenKey::new("test.surface");
const TEST_SURFACE_MUTED: TokenKey = TokenKey::new("test.surface_muted");
const TEST_BORDER: TokenKey = TokenKey::new("test.border");
const TEST_TEXT: TokenKey = TokenKey::new("test.text");
const TEST_TEXT_MUTED: TokenKey = TokenKey::new("test.text_muted");
const TEST_ACCENT: TokenKey = TokenKey::new("test.accent");
const TEST_FOCUS_RING: TokenKey = TokenKey::new("test.focus_ring");
const TEST_DESTRUCTIVE: TokenKey = TokenKey::new("test.destructive");

#[derive(Debug)]
struct DefaultSeedApi {
    builder: &'static str,
    runtime_value: &'static str,
}

#[derive(Debug)]
struct CallbackApi {
    name: &'static str,
    payload: &'static str,
}

#[derive(Debug)]
struct ComponentApiInventoryEntry {
    component: &'static str,
    controlled_inputs: &'static [&'static str],
    default_seeds: &'static [DefaultSeedApi],
    legacy_seed_inputs: &'static [&'static str],
    policy_hints: &'static [&'static str],
    callbacks: &'static [CallbackApi],
    renderer_neutral_state: bool,
    no_interaction_note: Option<&'static str>,
}

impl ComponentApiInventoryEntry {
    fn has_classification(&self) -> bool {
        !component_render_inputs(self.component).is_empty()
            || !self.controlled_inputs.is_empty()
            || !self.default_seeds.is_empty()
            || !self.legacy_seed_inputs.is_empty()
            || !self.policy_hints.is_empty()
            || !self.callbacks.is_empty()
            || self.no_interaction_note.is_some()
    }
}

const COMPONENT_API_INVENTORY: &[ComponentApiInventoryEntry] = &[
    ComponentApiInventoryEntry {
        component: "Button",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_click",
            payload: "ClickEvent",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Badge",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only primitive"),
    },
    ComponentApiInventoryEntry {
        component: "IconButton",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_click",
            payload: "ClickEvent",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Switch",
        controlled_inputs: &["checked"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Checkbox",
        controlled_inputs: &["checked", "checked_state"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_toggle",
            payload: "Toggled",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "RadioGroup",
        controlled_inputs: &[],
        default_seeds: &[DefaultSeedApi {
            builder: "default_selected",
            runtime_value: "selected",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation"],
        callbacks: &[CallbackApi {
            name: "on_selection_change",
            payload: "RadioSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Toggle",
        controlled_inputs: &["pressed"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Toolbar",
        controlled_inputs: &[],
        default_seeds: &[DefaultSeedApi {
            builder: "default_focused",
            runtime_value: "focused",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation"],
        callbacks: &[CallbackApi {
            name: "on_select",
            payload: "ToolbarSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Sidebar",
        controlled_inputs: &["collapsed", "selected"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_focused",
            runtime_value: "focused",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["side", "variant", "collapse_mode"],
        callbacks: &[CallbackApi {
            name: "on_selection_change",
            payload: "SidebarSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Tree",
        controlled_inputs: &[],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_selected",
                runtime_value: "selected",
            },
            DefaultSeedApi {
                builder: "default_focused",
                runtime_value: "focused",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "virtualized",
            "viewport_item_count",
            "overscan_count",
            "draggable",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_select",
                payload: "TreeSelection",
            },
            CallbackApi {
                name: "on_toggle",
                payload: "TreeToggle",
            },
            CallbackApi {
                name: "on_move",
                payload: "TreeMove",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Listbox",
        controlled_inputs: &["selected", "active"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["embedded", "typeahead_query"],
        callbacks: &[CallbackApi {
            name: "on_select",
            payload: "ListboxSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Select",
        controlled_inputs: &["open", "selected", "active"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "placement",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "SelectSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Combobox",
        controlled_inputs: &["open", "selected", "active"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_query",
                runtime_value: "query",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &["placement", "outside_press_policy"],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "ComboboxSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Command",
        controlled_inputs: &[
            "open",
            "query",
            "selected",
            "selected_values",
            "active",
            "index_snapshot",
        ],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_query",
                runtime_value: "query",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "dialog",
            "dialog_enabled",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "viewport_item_count",
            "row_height",
            "overscan",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_query_change",
                payload: "String",
            },
            CallbackApi {
                name: "on_select",
                payload: "CommandSelection",
            },
            CallbackApi {
                name: "on_selected_values_change",
                payload: "CommandSelectionChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Label",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["for_control"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("control-association primitive"),
    },
    ComponentApiInventoryEntry {
        component: "TextInput",
        controlled_inputs: &["value"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["controller"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "String",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Textarea",
        controlled_inputs: &["value"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["rows"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "String",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Field",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["control"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("composition wrapper"),
    },
    ComponentApiInventoryEntry {
        component: "Tabs",
        controlled_inputs: &[],
        default_seeds: &[DefaultSeedApi {
            builder: "default_selected",
            runtime_value: "selected",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "activation_mode"],
        callbacks: &[CallbackApi {
            name: "on_selection_change",
            payload: "TabsSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "ScrollArea",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["axis", "reset_on_key", "preserve_scroll", "scroll_handle"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Splitter",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "panel"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Table",
        controlled_inputs: &["state", "column_sizing"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_focused_row",
            runtime_value: "focused_row",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "virtualizer_snapshot",
            "expansion_mode",
            "filtering_mode",
            "sorting_mode",
            "faceting_mode",
            "facet_metadata",
            "pagination_mode",
            "pagination_totals",
            "row_pinning_policy",
            "enable_column_resizing",
            "column_resize_mode",
            "column_resize_direction",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_sort_requested",
                payload: "TableHeaderAction",
            },
            CallbackApi {
                name: "on_column_sizing_change",
                payload: "TableColumnSizingChange",
            },
            CallbackApi {
                name: "on_row_activate",
                payload: "TableRowActivation",
            },
            CallbackApi {
                name: "on_row_selection_change",
                payload: "TableRowSelectionChange",
            },
            CallbackApi {
                name: "on_row_expansion_request",
                payload: "TableRowExpansionToggle",
            },
            CallbackApi {
                name: "on_cell_edit_change",
                payload: "TableCellEditChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableFacetedFilter",
        controlled_inputs: &["open", "query", "selected_values"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_query",
                runtime_value: "query",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "facets",
            "placeholder",
            "empty_label",
            "clear_label",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_query_change",
                payload: "String",
            },
            CallbackApi {
                name: "on_change",
                payload: "TableFacetedFilterChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableColumnVisibility",
        controlled_inputs: &["open", "visibility"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_visibility",
                runtime_value: "visibility",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "columns",
            "empty_label",
            "show_all_label",
            "reset_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_change",
                payload: "TableColumnVisibilityChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableGlobalFilter",
        controlled_inputs: &["query"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_query",
            runtime_value: "query",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["placeholder", "clear_label", "disabled"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "TableGlobalFilterChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TablePredicateFilter",
        controlled_inputs: &["operator", "value"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_operator",
                runtime_value: "operator",
            },
            DefaultSeedApi {
                builder: "default_value",
                runtime_value: "value",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "operator_option",
            "operators",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
        ],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "TablePredicateFilterChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableToolbar",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["control", "secondary_control", "summary", "tokens"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("slot container"),
    },
    ComponentApiInventoryEntry {
        component: "TableRangeFilter",
        controlled_inputs: &["open"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_min_text",
                runtime_value: "min_text",
            },
            DefaultSeedApi {
                builder: "default_max_text",
                runtime_value: "max_text",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "facets",
            "range",
            "clear_label",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_change",
                payload: "TableRangeFilterChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "VirtualizedList",
        controlled_inputs: &[],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_active_index",
                runtime_value: "active_index",
            },
            DefaultSeedApi {
                builder: "default_selected_index",
                runtime_value: "selected_index",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &["viewport_item_count", "row_height", "overscan"],
        callbacks: &[CallbackApi {
            name: "on_activate",
            payload: "VirtualizedListActivation",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "StatusCue",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("feedback readout"),
    },
    ComponentApiInventoryEntry {
        component: "EmptyState",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("feedback readout"),
    },
    ComponentApiInventoryEntry {
        component: "Separator",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "decorative"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only primitive"),
    },
    ComponentApiInventoryEntry {
        component: "Kbd",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only primitive"),
    },
    ComponentApiInventoryEntry {
        component: "Progress",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["indeterminate"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("status readout"),
    },
    ComponentApiInventoryEntry {
        component: "Skeleton",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only loading placeholder"),
    },
    ComponentApiInventoryEntry {
        component: "Avatar",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["accessible_label"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("identity readout"),
    },
    ComponentApiInventoryEntry {
        component: "Tooltip",
        controlled_inputs: &["open"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
        ],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "HoverCard",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Popover",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Dialog",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "AlertDialog",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_cancel",
                payload: "()",
            },
            CallbackApi {
                name: "on_action",
                payload: "()",
            },
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Sheet",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "side",
            "modal_mode",
            "close_affordance",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_close",
                payload: "()",
            },
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Menu",
        controlled_inputs: &["open"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_focused_value",
                runtime_value: "focused_value",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "placement",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "MenuSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "ContextMenu",
        controlled_inputs: &["open"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_focused_value",
                runtime_value: "focused_value",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "anchor_point",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "MenuSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
];

fn component_render_inputs(component: &str) -> &'static [&'static str] {
    match component {
        "Button" => &["variant", "disabled", "selected"],
        "Badge" => &["variant"],
        "IconButton" => &["variant", "disabled"],
        "Switch" => &["label", "disabled"],
        "Checkbox" => &["label", "indeterminate", "disabled", "required", "invalid"],
        "RadioGroup" => &["label", "disabled", "required", "item"],
        "Toggle" => &["variant", "disabled"],
        "Toolbar" => &["disabled", "item", "items"],
        "Sidebar" => &["disabled", "section", "sections"],
        "Tree" => &[
            "item",
            "items",
            "virtualized",
            "viewport_item_count",
            "overscan_count",
            "draggable",
        ],
        "Listbox" => &[
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "empty_label",
        ],
        "Select" => &[
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
        ],
        "Combobox" => &[
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "required",
            "empty_label",
        ],
        "Command" => &[
            "placeholder",
            "trigger_label",
            "item",
            "items",
            "group",
            "groups",
            "index_snapshot",
            "disabled",
            "dialog_description",
            "loading",
            "idle",
            "empty_label",
        ],
        "Label" => &["required", "disabled"],
        "TextInput" => &[
            "placeholder",
            "display_mode",
            "disabled",
            "read_only",
            "invalid",
            "required",
        ],
        "Textarea" => &[
            "placeholder",
            "rows",
            "disabled",
            "read_only",
            "invalid",
            "required",
        ],
        "Field" => &[
            "help_text",
            "help",
            "error_text",
            "error",
            "required",
            "disabled",
            "invalid",
        ],
        "Tabs" => &["item"],
        "Splitter" => &["disabled", "panel"],
        "Table" => &[
            "label",
            "overscan",
            "row_height",
            "header_height",
            "viewport_extent",
            "min_column_width",
            "expansion_mode",
            "enable_column_resizing",
            "column_resize_mode",
            "column_resize_direction",
            "content_fit_columns",
        ],
        "TableFacetedFilter" => &[
            "facets",
            "selected_values",
            "open",
            "default_open",
            "query",
            "default_query",
            "placeholder",
            "empty_label",
            "clear_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_query_change",
            "on_change",
        ],
        "TableColumnVisibility" => &[
            "columns",
            "visibility",
            "default_visibility",
            "open",
            "default_open",
            "empty_label",
            "show_all_label",
            "reset_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_change",
        ],
        "TablePredicateFilter" => &[
            "operator",
            "default_operator",
            "value",
            "default_value",
            "operator_option",
            "operators",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
            "on_change",
        ],
        "TableToolbar" => &[
            "control",
            "controls",
            "secondary_control",
            "secondary_controls",
            "summary",
            "tokens",
        ],
        "VirtualizedList" => &["disabled", "viewport_item_count", "row_height", "overscan"],
        "StatusCue" => &["intent"],
        "EmptyState" => &["description", "intent"],
        "Separator" => &["orientation", "vertical", "decorative"],
        "Progress" => &["value", "indeterminate"],
        "Skeleton" => &["subtle"],
        "Avatar" => &["source", "fallback", "accessible_label"],
        "AvatarGroup" => &["avatars", "avatar", "max_visible"],
        "Tooltip" => &["disabled"],
        "HoverCard" => &["disabled"],
        "Popover" => &["disabled"],
        "Dialog" => &["description", "disabled"],
        "AlertDialog" => &[
            "intent",
            "cancel_label",
            "disabled",
            "cancel_disabled",
            "action_disabled",
        ],
        "Sheet" => &["description", "disabled"],
        "Menu" => &["item", "items", "disabled"],
        "ContextMenu" => &["item", "items"],
        _ => &[],
    }
}

fn component_source_file(component: &str) -> &'static str {
    match component {
        "Button" => "button.rs",
        "Badge" => "badge.rs",
        "IconButton" => "icon_button.rs",
        "Switch" => "switch.rs",
        "Checkbox" => "checkbox.rs",
        "RadioGroup" => "radio.rs",
        "Toggle" => "toggle.rs",
        "Toolbar" => "toolbar.rs",
        "Sidebar" => "sidebar.rs",
        "Tree" => "tree.rs",
        "Listbox" => "listbox.rs",
        "Select" => "select.rs",
        "Combobox" => "combobox.rs",
        "Command" => "command.rs",
        "Label" => "label.rs",
        "TextInput" => "text_input.rs",
        "Textarea" => "textarea.rs",
        "Field" => "field.rs",
        "Tabs" => "tabs.rs",
        "ScrollArea" => "scroll_area.rs",
        "Splitter" => "splitter.rs",
        "Table" => "table.rs",
        "TableColumnVisibility" => "table.rs",
        "TableFacetedFilter" => "table.rs",
        "TableGlobalFilter" => "table.rs",
        "TablePredicateFilter" => "table.rs",
        "TableRangeFilter" => "table.rs",
        "TableToolbar" => "table.rs",
        "VirtualizedList" => "virtualized_list.rs",
        "StatusCue" => "feedback.rs",
        "EmptyState" => "feedback.rs",
        "Separator" => "separator.rs",
        "Kbd" => "kbd.rs",
        "Progress" => "progress.rs",
        "Skeleton" => "skeleton.rs",
        "Avatar" => "avatar.rs",
        "AvatarGroup" => "avatar.rs",
        "Tooltip" => "tooltip.rs",
        "HoverCard" => "hover_card.rs",
        "Popover" => "popover.rs",
        "Dialog" => "dialog.rs",
        "AlertDialog" => "alert_dialog.rs",
        "Sheet" => "sheet.rs",
        "Menu" => "menu.rs",
        "ContextMenu" => "context_menu.rs",
        _ => panic!("missing source file mapping for `{component}`"),
    }
}

fn component_public_methods(component: &str) -> &'static [&'static str] {
    match component {
        "Button" => &[
            "new", "variant", "disabled", "selected", "tokens", "on_click", "state",
        ],
        "Badge" => &["new", "variant", "tokens", "state"],
        "IconButton" => &[
            "new",
            "variant",
            "disabled",
            "tokens",
            "on_click",
            "accessible_label",
            "state",
        ],
        "Switch" => &[
            "new",
            "label",
            "checked",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "Checkbox" => &[
            "new",
            "label",
            "checked",
            "indeterminate",
            "checked_state",
            "disabled",
            "required",
            "invalid",
            "tokens",
            "on_toggle",
            "state",
        ],
        "RadioGroup" => &[
            "new",
            "label",
            "orientation",
            "default_selected",
            "disabled",
            "required",
            "tokens",
            "item",
            "on_selection_change",
            "state",
        ],
        "Toggle" => &[
            "new",
            "variant",
            "pressed",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "Toolbar" => &[
            "new",
            "orientation",
            "default_focused",
            "disabled",
            "tokens",
            "item",
            "items",
            "on_select",
            "state",
        ],
        "Sidebar" => &[
            "new",
            "side",
            "left",
            "right",
            "variant",
            "collapse_mode",
            "collapsed",
            "disabled",
            "selected",
            "default_focused",
            "tokens",
            "section",
            "sections",
            "on_selection_change",
            "state",
        ],
        "Tree" => &[
            "new",
            "item",
            "default_selected",
            "default_focused",
            "virtualized",
            "viewport_item_count",
            "overscan_count",
            "draggable",
            "on_select",
            "on_toggle",
            "on_move",
            "items",
            "state",
            "render_plan",
        ],
        "Listbox" => &[
            "new",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "embedded",
            "selected",
            "active",
            "typeahead_query",
            "empty_label",
            "tokens",
            "on_select",
            "state",
        ],
        "Select" => &[
            "new",
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "open",
            "default_open",
            "selected",
            "active",
            "placement",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        "Combobox" => &[
            "new",
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "required",
            "open",
            "default_open",
            "default_query",
            "selected",
            "active",
            "empty_label",
            "placement",
            "outside_press_policy",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        "Command" => &[
            "new",
            "placeholder",
            "trigger_label",
            "item",
            "items",
            "group",
            "groups",
            "index_snapshot",
            "disabled",
            "open",
            "default_open",
            "dialog",
            "dialog_enabled",
            "dialog_description",
            "query",
            "default_query",
            "selection_mode",
            "multi_select",
            "selected",
            "selected_values",
            "active",
            "viewport_item_count",
            "row_height",
            "overscan",
            "loading",
            "idle",
            "empty_label",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_query_change",
            "on_select",
            "on_selected_values_change",
            "state",
            "render_plan",
            "render_plan_with_viewport",
        ],
        "Label" => &[
            "new",
            "for_control",
            "required",
            "disabled",
            "tokens",
            "state",
        ],
        "TextInput" => &[
            "new",
            "controller",
            "value",
            "on_change",
            "placeholder",
            "display_mode",
            "disabled",
            "read_only",
            "invalid",
            "required",
            "tokens",
            "state",
        ],
        "Textarea" => &[
            "new",
            "value",
            "on_change",
            "placeholder",
            "rows",
            "disabled",
            "read_only",
            "invalid",
            "required",
            "tokens",
            "state",
        ],
        "Field" => &[
            "new",
            "help_text",
            "help",
            "error_text",
            "error",
            "required",
            "disabled",
            "invalid",
            "tokens",
            "control",
            "state",
        ],
        "Tabs" => &[
            "new",
            "orientation",
            "activation_mode",
            "default_selected",
            "tokens",
            "item",
            "on_selection_change",
            "state",
        ],
        "ScrollArea" => &[
            "new",
            "axis",
            "vertical",
            "horizontal",
            "both",
            "scroll_handle",
            "reset_on_key",
            "preserve_scroll",
            "state",
        ],
        "Splitter" => &[
            "new",
            "orientation",
            "horizontal",
            "vertical",
            "disabled",
            "panel",
            "state",
        ],
        "Table" => &[
            "new",
            "label",
            "overscan",
            "row_height",
            "header_height",
            "viewport_extent",
            "expansion_mode",
            "min_column_width",
            "virtualizer_snapshot",
            "default_focused_row",
            "on_sort_requested",
            "enable_column_resizing",
            "column_resize_mode",
            "column_resize_direction",
            "on_column_sizing_change",
            "on_row_selection_change",
            "on_row_activate",
            "on_row_expansion_request",
            "on_cell_edit_change",
            "table_state",
            "state",
            "render_plan",
        ],
        "TableFacetedFilter" => &[
            "new",
            "facets",
            "selected_values",
            "open",
            "default_open",
            "query",
            "default_query",
            "placeholder",
            "empty_label",
            "clear_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_query_change",
            "on_change",
            "state",
        ],
        "TableColumnVisibility" => &[
            "new",
            "columns",
            "visibility",
            "default_visibility",
            "open",
            "default_open",
            "empty_label",
            "show_all_label",
            "reset_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_change",
            "state",
        ],
        "TableGlobalFilter" => &[
            "new",
            "query",
            "default_query",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "TablePredicateFilter" => &[
            "new",
            "operator",
            "default_operator",
            "value",
            "default_value",
            "operator_option",
            "operators",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "TableToolbar" => &[
            "new",
            "control",
            "controls",
            "secondary_control",
            "secondary_controls",
            "summary",
            "tokens",
            "state",
        ],
        "TableRangeFilter" => &[
            "new",
            "facets",
            "range",
            "default_min_text",
            "default_max_text",
            "open",
            "default_open",
            "clear_label",
            "disabled",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_change",
            "state",
        ],
        "VirtualizedList" => &[
            "new",
            "from_shared_items",
            "disabled",
            "default_active_index",
            "default_selected_index",
            "viewport_item_count",
            "row_height",
            "overscan",
            "on_activate",
            "state",
            "render_plan",
        ],
        "StatusCue" => &["new", "intent", "tokens", "state"],
        "EmptyState" => &["new", "description", "intent", "tokens", "state"],
        "Separator" => &[
            "new",
            "orientation",
            "vertical",
            "decorative",
            "tokens",
            "state",
        ],
        "Kbd" => &["new", "tokens", "state"],
        "Progress" => &["new", "value", "indeterminate", "tokens", "state"],
        "Skeleton" => &["new", "subtle", "tokens", "state"],
        "Avatar" => &[
            "new",
            "source",
            "fallback",
            "accessible_label",
            "tokens",
            "state",
        ],
        "AvatarGroup" => &["new", "avatar", "avatars", "max_visible", "tokens", "state"],
        "Tooltip" => &[
            "new",
            "element",
            "disabled",
            "open",
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
            "tokens",
            "state",
        ],
        "HoverCard" => &[
            "new",
            "element",
            "disabled",
            "open",
            "default_open",
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "state",
        ],
        "Popover" => &[
            "new",
            "element",
            "disabled",
            "open",
            "default_open",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "state",
        ],
        "Dialog" => &[
            "new",
            "element",
            "description",
            "disabled",
            "open",
            "default_open",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "state",
        ],
        "AlertDialog" => &[
            "new",
            "intent",
            "cancel_label",
            "disabled",
            "cancel_disabled",
            "action_disabled",
            "open",
            "default_open",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_cancel",
            "on_action",
            "on_open_change",
            "state",
        ],
        "Sheet" => &[
            "new",
            "element",
            "description",
            "disabled",
            "open",
            "default_open",
            "side",
            "modal_mode",
            "close_affordance",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_close",
            "on_open_change",
            "state",
        ],
        "Menu" => &[
            "new",
            "item",
            "items",
            "disabled",
            "open",
            "default_open",
            "default_focused_value",
            "placement",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        "ContextMenu" => &[
            "new",
            "item",
            "items",
            "open",
            "default_open",
            "anchor_point",
            "default_focused_value",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        _ => panic!("missing public method baseline for `{component}`"),
    }
}

fn component_public_methods_from_source(component: &str) -> Vec<String> {
    const MARKER_PREFIX: &str = "impl ";

    let source_file = component_source_file(component);
    let source_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/");
    let source_path = format!("{source_path}{source_file}");
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {source_path}: {error}"));
    let marker = format!("{MARKER_PREFIX}{component} {{");
    let impl_start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("missing `{marker}` in {source_file}"));
    let body_start = source[impl_start..]
        .find('{')
        .map(|offset| impl_start + offset)
        .expect("impl body should open with `{`");

    let mut depth = 0usize;
    let mut body_end = None;
    for (index, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = Some(body_start + index);
                    break;
                }
            }
            _ => {}
        }
    }
    let body_end = body_end.expect("impl body should close");
    let body = &source[body_start + 1..body_end];
    let mut methods = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(signature) = trimmed.strip_prefix("pub const fn ") {
            let before_paren = signature
                .split_once('(')
                .map(|(name, _)| name)
                .unwrap_or(signature);
            let name = before_paren
                .split_once('<')
                .map(|(name, _)| name)
                .unwrap_or(before_paren)
                .trim();
            methods.push(name.to_string());
        } else if let Some(signature) = trimmed.strip_prefix("pub fn ") {
            let before_paren = signature
                .split_once('(')
                .map(|(name, _)| name)
                .unwrap_or(signature);
            let name = before_paren
                .split_once('<')
                .map(|(name, _)| name)
                .unwrap_or(before_paren)
                .trim();
            methods.push(name.to_string());
        }
    }

    methods
}

fn custom_tokens() -> ThemeTokens {
    ThemeTokens {
        surface: TEST_SURFACE,
        surface_muted: TEST_SURFACE_MUTED,
        border: TEST_BORDER,
        text: TEST_TEXT,
        text_muted: TEST_TEXT_MUTED,
        accent: TEST_ACCENT,
        focus_ring: TEST_FOCUS_RING,
        destructive: TEST_DESTRUCTIVE,
        ..ThemeTokens::default()
    }
}

fn sample_table_state(row_count: usize) -> TableState {
    let rows = (0..row_count).map(|index| {
        TableRow::new(format!("row-{index:04}"))
            .with_cell("name", format!("Package {index:04}"))
            .with_cell(
                "team",
                if index.is_multiple_of(2) {
                    "Core"
                } else {
                    "UI"
                },
            )
            .with_cell("score", index)
    });

    TableState::new(rows).with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
    ])
}

fn text_facet_counts(facet: &TableColumnFacets) -> Vec<(String, usize)> {
    facet
        .unique_values()
        .iter()
        .map(|entry| match entry.value() {
            TableCellValue::Text(value) => (value.clone(), entry.count()),
            value => panic!("expected text facet value, got {value:?}"),
        })
        .collect()
}

fn sample_pinned_table_state() -> TableState {
    TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "Platform")
        .with_cell("score", 42_usize)
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
        TableColumn::new("status", "Status"),
    ])
    .with_column_order(["status", "score", "team", "name"])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name", "score"])
            .pinned_right(["status"]),
    )
    .with_pagination(TablePagination::disabled())
}

fn sample_pinned_table_state_with_rows(row_count: usize) -> TableState {
    let rows = (0..row_count).map(|index| {
        TableRow::new(format!("row-{index:04}"))
            .with_cell("name", format!("Alpha {index:04}"))
            .with_cell(
                "team",
                if index.is_multiple_of(2) {
                    "Platform"
                } else {
                    "UI"
                },
            )
            .with_cell("score", index + 1)
            .with_cell(
                "status",
                if index.is_multiple_of(3) {
                    "Ready"
                } else {
                    "Queued"
                },
            )
    });

    TableState::new(rows)
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
            TableColumn::new("status", "Status"),
        ])
        .with_column_order(["status", "score", "team", "name"])
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name", "score"])
                .pinned_right(["status"]),
        )
        .with_pagination(TablePagination::disabled())
}

fn sample_center_window_table_state() -> TableState {
    let row = TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("metric_00", 10_usize)
        .with_cell("metric_01", 20_usize)
        .with_cell("metric_02", 30_usize)
        .with_cell("metric_03", 40_usize)
        .with_cell("metric_04", 50_usize)
        .with_cell("metric_05", 60_usize)
        .with_cell("status", "Ready");

    sample_center_window_table_state_from_rows([row])
}

fn sample_center_window_table_state_with_rows(row_count: usize) -> TableState {
    let rows = (0..row_count).map(|index| {
        TableRow::new(format!("row-{index:04}"))
            .with_cell("name", format!("Package {index:04}"))
            .with_cell("metric_00", index + 10)
            .with_cell("metric_01", index + 20)
            .with_cell("metric_02", index + 30)
            .with_cell("metric_03", index + 40)
            .with_cell("metric_04", index + 50)
            .with_cell("metric_05", index + 60)
            .with_cell(
                "status",
                if index.is_multiple_of(2) {
                    "Ready"
                } else {
                    "Queued"
                },
            )
    });

    sample_center_window_table_state_from_rows(rows)
}

fn sample_center_window_table_state_from_rows(
    rows: impl IntoIterator<Item = TableRow>,
) -> TableState {
    TableState::new(rows)
        .with_columns([
            TableColumn::new("name", "Name").with_width(ui_px(140.0)),
            TableColumn::new("metric_00", "Metric 00").with_width(ui_px(60.0)),
            TableColumn::new("metric_01", "Metric 01").with_width(ui_px(72.0)),
            TableColumn::new("metric_02", "Metric 02").with_width(ui_px(84.0)),
            TableColumn::new("metric_03", "Metric 03").with_width(ui_px(96.0)),
            TableColumn::new("metric_04", "Metric 04").with_width(ui_px(108.0)),
            TableColumn::new("metric_05", "Metric 05").with_width(ui_px(120.0)),
            TableColumn::new("status", "Status").with_width(ui_px(132.0)),
        ])
        .with_column_order([
            "name",
            "metric_00",
            "metric_01",
            "metric_02",
            "metric_03",
            "metric_04",
            "metric_05",
            "status",
        ])
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["status"]),
        )
        .with_pagination(TablePagination::disabled())
}

#[test]
fn overlay_adapter_config_defaults_follow_overlay_kind_policy() {
    let tooltip =
        GpuiOverlayAdapterConfig::new(OverlayLayerKind::Tooltip, OverlayPresence::open()).state();
    let popover = GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::NonModalDismissible,
        OverlayPresence::open(),
    )
    .state();
    let dialog =
        GpuiOverlayAdapterConfig::new(OverlayLayerKind::Modal, OverlayPresence::open()).state();
    let menu =
        GpuiOverlayAdapterConfig::new(OverlayLayerKind::Menu, OverlayPresence::open()).state();

    assert_eq!(tooltip.policy().kind(), OverlayLayerKind::Tooltip);
    assert_eq!(
        tooltip.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(tooltip.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
    assert!(tooltip.should_render_deferred_layer());
    assert!(!tooltip.layer_state().hit_testable());

    assert_eq!(
        popover.policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        popover.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::NonModalDismissible)
    );
    assert!(popover.layer_state().visible());
    assert!(popover.wants_outside_press_handler());

    assert_eq!(dialog.policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(
        dialog.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Modal)
    );
    assert!(dialog.layer_state().blocks_underlay_input());

    assert_eq!(menu.policy().kind(), OverlayLayerKind::Menu);
    assert_eq!(
        menu.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Menu)
    );
    assert!(menu.layer_state().visible());
}

#[test]
fn overlay_adapter_config_can_override_focus_and_dismiss_policy() {
    let state = GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::NonModalDismissible,
        OverlayPresence::open(),
    )
    .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
    .escape_key_policy(EscapeKeyPolicy::Dismiss)
    .focus_restore_intent(open_gpui_ui_core::FocusRestoreIntent::TriggerOrFallback(
        open_gpui_ui_core::OverlayFocusTarget::new("fallback"),
    ))
    .initial_focus_intent(InitialFocusIntent::FirstFocusable)
    .deferred_priority(9)
    .snap_margin(px(12.0))
    .state();

    assert_eq!(
        state.policy().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.policy().escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(state.deferred_priority(), 9);
    assert_eq!(state.snap_margin(), px(12.0));
}

#[test]
fn overlay_placement_maps_to_gpui_anchor_and_margin() {
    let input = OverlayPlacementInput::new(
        OverlayAnchorInput::from_visual_and_layout_bounds(
            Some(rect(
                ui_point(ui_px(10.0), ui_px(20.0)),
                ui_size(ui_px(100.0), ui_px(40.0)),
            )),
            Some(rect(
                ui_point(ui_px(30.0), ui_px(40.0)),
                ui_size(ui_px(120.0), ui_px(60.0)),
            )),
        ),
        ui_size(ui_px(180.0), ui_px(120.0)),
    )
    .with_side(OverlayPlacementSide::Bottom)
    .with_alignment(OverlayPlacementAlignment::End)
    .with_offset(ui_px(6.0))
    .with_safe_bounds(rect(
        ui_point(ui_px(0.0), ui_px(0.0)),
        ui_size(ui_px(300.0), ui_px(220.0)),
    ));

    let placement = GpuiOverlayPlacement::resolve(input, DEFAULT_OVERLAY_SAFE_MARGIN);

    assert_eq!(placement.anchor(), Anchor::TopRight);
    assert_eq!(placement.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
    assert!(placement.position().is_some());
    assert_eq!(placement.safe_bounds(), input.safe_bounds());
}

#[test]
fn overlay_open_change_helpers_match_core_policies() {
    let dismissible = OverlayLayerPolicy::new(
        OverlayLayerKind::NonModalDismissible,
        OverlayPresence::open(),
    );
    let tooltip = OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open());

    let escape =
        escape_open_change(&dismissible).expect("dismissible overlay should close on escape");
    assert_eq!(escape.reason(), DismissReason::EscapeKey);
    assert!(escape.consumes_event());
    assert!(!escape.allows_underlay_dispatch());

    let outside = outside_press_open_change(&dismissible)
        .expect("dismissible overlay should close on outside press");
    assert_eq!(outside.reason(), DismissReason::OutsidePress);
    assert!(outside.allows_underlay_dispatch());

    assert_eq!(escape_open_change(&tooltip), None);
    assert_eq!(outside_press_open_change(&tooltip), None);
    assert_eq!(
        gpui_anchor(OverlayPlacementSide::Top, OverlayPlacementAlignment::Start),
        Anchor::BottomLeft
    );
    let point_placement =
        point_anchor_placement(point(px(5.0), px(6.0)), ui_size(ui_px(80.0), ui_px(40.0)));
    assert_eq!(
        GpuiOverlayPlacement::resolve(point_placement, DEFAULT_OVERLAY_SAFE_MARGIN).anchor(),
        Anchor::TopLeft
    );
}

#[test]
fn overlay_label_helpers_are_stable() {
    assert_eq!(MenuOpenMode::Uncontrolled.as_str(), "uncontrolled");
    assert_eq!(TooltipOpenIntent::Manual.as_str(), "manual");
    assert_eq!(TooltipContentKind::Element.as_str(), "element");
    assert_eq!(HoverCardOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(HoverCardOpenIntent::HoverOrFocus.as_str(), "hover or focus");
    assert_eq!(HoverCardContentKind::Text.as_str(), "text");
    assert_eq!(PopoverOpenMode::Uncontrolled.as_str(), "uncontrolled");
    assert_eq!(DialogOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(AlertDialogOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(AlertDialogIntent::Destructive.as_str(), "destructive");
    assert_eq!(SheetOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(SheetSide::Left.as_str(), "left");
    assert_eq!(SheetModalMode::NonModal.as_str(), "non-modal");
    assert_eq!(OverlayLayerKind::Menu.as_str(), "menu");
    assert_eq!(
        OutsidePressPolicy::DismissAndPassThrough.as_str(),
        "dismiss + pass-through"
    );
    assert_eq!(EscapeKeyPolicy::Ignore.as_str(), "ignore");
    assert_eq!(FocusRestoreIntent::None.as_str(), "none");
    assert_eq!(
        InitialFocusIntent::TargetOrFirstFocusable(open_gpui_ui_core::OverlayFocusTarget::new("x"))
            .as_str(),
        "target or first focusable"
    );
}

#[test]
fn tooltip_state_records_descriptive_overlay_policy() {
    let state = Tooltip::new("tip", "Save changes").open(true).state();

    assert_eq!(state.content_kind(), TooltipContentKind::Text);
    assert_eq!(state.role(), Role::Label);
    assert!(state.open());
    assert!(state.descriptive());
    assert!(!state.interactive_content());
    assert!(state.open_intent().opens_on_hover());
    assert!(state.open_intent().opens_on_focus());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.placement_alignment(),
        OverlayPlacementAlignment::Center
    );
    assert_eq!(state.delay().open_delay(), Duration::from_millis(500));
    assert_eq!(state.colors().background().token(), semantic::OVERLAY);
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Tooltip);
    assert!(state.overlay().should_render_deferred_layer());
    assert!(!state.overlay().layer_state().hit_testable());
}

#[test]
fn tooltip_state_models_disabled_element_content_and_delay_overrides() {
    let delay = TooltipDelayPolicy::new(
        Duration::from_millis(120),
        Duration::from_millis(40),
        Duration::from_millis(250),
    );
    let state = Tooltip::element("rich-tip", div().child("Rich"))
        .open(true)
        .disabled(true)
        .open_intent(TooltipOpenIntent::Focus)
        .placement_side(OverlayPlacementSide::Bottom)
        .placement_alignment(OverlayPlacementAlignment::End)
        .delay(delay)
        .small()
        .state();

    assert_eq!(state.content_kind(), TooltipContentKind::Element);
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.open_intent().opens_on_hover());
    assert!(state.open_intent().opens_on_focus());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Bottom);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(state.delay(), delay);
    assert_eq!(state.size(), Size::Small);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn hover_card_state_records_interactive_hover_focus_overlay_policy() {
    let state = HoverCard::new("profile-card", "Open profile", "Profile details")
        .open(true)
        .placement_side(OverlayPlacementSide::Right)
        .placement_alignment(OverlayPlacementAlignment::End)
        .state();

    assert_eq!(state.content_kind(), HoverCardContentKind::Text);
    assert!(state.open());
    assert_eq!(state.open_mode(), HoverCardOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Window);
    assert!(!state.descriptive());
    assert!(state.interactive_content());
    assert!(state.open_intent().opens_on_hover());
    assert!(state.open_intent().opens_on_focus());
    assert!(!state.open_intent().opens_manually());
    assert!(state.trigger_selected());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(state.delay().open_delay(), Duration::from_millis(700));
    assert_eq!(state.delay().close_delay(), Duration::from_millis(300));
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(state.colors().background().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[test]
fn hover_card_state_models_manual_disabled_and_policy_overrides() {
    let delay = HoverCardDelayPolicy::new(Duration::from_millis(80), Duration::from_millis(20));
    let state = HoverCard::element("rich-hover-card", "Details", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .open_intent(HoverCardOpenIntent::Manual)
        .delay(delay)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .initial_focus_intent(InitialFocusIntent::FirstFocusable)
        .focus_restore_intent(FocusRestoreIntent::Trigger)
        .small()
        .state();

    assert_eq!(state.content_kind(), HoverCardContentKind::Element);
    assert_eq!(state.open_mode(), HoverCardOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.activation_enabled());
    assert!(!state.open_intent().opens_on_hover());
    assert!(!state.open_intent().opens_on_focus());
    assert!(state.open_intent().opens_manually());
    assert_eq!(state.delay(), delay);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn popover_state_records_interactive_overlay_policy() {
    let state = Popover::new("settings-popover", "Settings", "Panel")
        .open(true)
        .placement_side(OverlayPlacementSide::Right)
        .placement_alignment(OverlayPlacementAlignment::End)
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), PopoverOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Window);
    assert!(state.trigger_selected());
    assert!(state.activation_enabled());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert!(state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(state.colors().background().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[test]
fn popover_state_models_default_open_disabled_and_policy_overrides() {
    let state = Popover::element("help-popover", "Help", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), PopoverOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.activation_enabled());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn dialog_state_records_modal_title_and_focus_policy() {
    let state = Dialog::new("confirm-dialog", "Open", "Confirm changes", "Body")
        .description("This cannot be undone.")
        .open(true)
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), DialogOpenMode::Controlled);
    assert_eq!(state.title(), "Confirm changes");
    assert_eq!(state.description(), Some("This cannot be undone."));
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Window);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert!(state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Consume);
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.colors().barrier().token(), semantic::MODAL_OVERLAY);
}

#[test]
fn dialog_state_models_disabled_default_open_and_policy_overrides() {
    let state = Dialog::element("modal", "Open", "Blocked dialog", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::Ignore)
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), DialogOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.activation_enabled());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn dialog_runtime_respects_escape_policy_and_restores_trigger_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        escape_policy: Rc<RefCell<EscapeKeyPolicy>>,
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let escape_policy = *self.escape_policy.borrow();
            let open_events = self.open_events.clone();

            div().size_full().child(
                Dialog::new("runtime-dialog", "Open dialog", "Runtime dialog", "Body")
                    .escape_key_policy(escape_policy)
                    .on_open_change(move |open, _, _| {
                        open_events.borrow_mut().push(open);
                    }),
            )
        }
    }

    let escape_policy = Rc::new(RefCell::new(EscapeKeyPolicy::Ignore));
    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        escape_policy: escape_policy.clone(),
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("dialog:runtime-dialog:trigger")
        .expect("dialog trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("dialog:runtime-dialog:surface"),
        "opened dialog should move focus to the surface"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("dialog:runtime-dialog:surface").is_some(),
        "EscapeKeyPolicy::Ignore should keep dialog content mounted"
    );

    *escape_policy.borrow_mut() = EscapeKeyPolicy::Dismiss;
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("dialog:runtime-dialog:surface").is_none(),
        "EscapeKeyPolicy::Dismiss should close dialog content"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:runtime-dialog:trigger"),
        "Escape dismissal should restore focus to the dialog trigger"
    );
    assert_eq!(open_events.borrow().as_slice(), &[true, false]);
}

#[test]
fn alert_dialog_state_records_required_actions_and_destructive_intent() {
    let state = AlertDialog::new(
        "delete-project",
        "Delete project",
        "Delete this project?",
        "This action permanently removes project data.",
        "Delete",
    )
    .cancel_label("Keep project")
    .intent(AlertDialogIntent::Destructive)
    .open(true)
    .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), AlertDialogOpenMode::Controlled);
    assert_eq!(state.title(), "Delete this project?");
    assert_eq!(
        state.description(),
        "This action permanently removes project data."
    );
    assert_eq!(state.content_role(), Role::AlertDialog);
    assert_eq!(state.intent(), AlertDialogIntent::Destructive);
    assert_eq!(state.cancel().kind(), AlertDialogActionKind::Cancel);
    assert_eq!(state.cancel().label(), "Keep project");
    assert!(state.cancel().default_focus());
    assert_eq!(state.action().kind(), AlertDialogActionKind::Action);
    assert_eq!(state.action().label(), "Delete");
    assert_eq!(state.action().variant(), ButtonVariant::Destructive);
    assert!(!state.action().default_focus());
    assert_eq!(
        state.colors().action_background().token(),
        semantic::DESTRUCTIVE
    );
}

#[test]
fn alert_dialog_state_blocks_underlay_and_restores_focus_to_trigger() {
    let state = AlertDialog::new(
        "confirm",
        "Open",
        "Archive item?",
        "It can be restored.",
        "Archive",
    )
    .default_open(true)
    .state();

    assert_eq!(state.open_mode(), AlertDialogOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.open());
    assert_eq!(state.trigger_role(), Role::Button);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert!(state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Consume);
    assert!(!state.outside_press_policy().resolve().dismisses());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.colors().barrier().token(), semantic::MODAL_OVERLAY);
}

#[test]
fn sheet_state_records_side_modal_mode_size_and_close_affordance() {
    let state = Sheet::new(
        "settings-sheet",
        "Open settings",
        "Settings",
        "Configure workspace",
    )
    .description("Workspace preferences")
    .default_open(true)
    .side(SheetSide::Left)
    .state();

    assert_eq!(state.open_mode(), SheetOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.open());
    assert_eq!(state.side(), SheetSide::Left);
    assert!(state.side().is_horizontal());
    assert_eq!(state.modal_mode(), SheetModalMode::Modal);
    assert_eq!(state.close_affordance(), SheetCloseAffordance::Visible);
    assert!(state.close_affordance().visible());
    assert_eq!(state.title(), "Settings");
    assert_eq!(state.description(), Some("Workspace preferences"));
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Dialog);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert!(state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(state.outside_press_policy().resolve().dismisses());
    assert_eq!(state.colors().surface().token(), semantic::SURFACE);
    assert!(state.metrics().surface_size() > ui_px(0.0));
}

#[test]
fn sheet_state_models_non_modal_and_explicit_dismiss_policy() {
    let state = Sheet::new(
        "bottom-sheet",
        "Open details",
        "Details",
        "Non-modal information",
    )
    .open(true)
    .side(SheetSide::Bottom)
    .modal_mode(SheetModalMode::NonModal)
    .close_affordance(SheetCloseAffordance::Hidden)
    .outside_press_policy(OutsidePressPolicy::Ignore)
    .escape_key_policy(EscapeKeyPolicy::Ignore)
    .initial_focus_intent(InitialFocusIntent::None)
    .focus_restore_intent(FocusRestoreIntent::None)
    .small()
    .state();

    assert_eq!(state.open_mode(), SheetOpenMode::Controlled);
    assert!(state.open());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.side(), SheetSide::Bottom);
    assert!(!state.side().is_horizontal());
    assert_eq!(state.modal_mode(), SheetModalMode::NonModal);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert!(!state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(state.close_affordance(), SheetCloseAffordance::Hidden);
    assert!(!state.close_affordance().visible());
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert!(!state.overlay().wants_outside_press_handler());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
}

#[test]
fn menu_state_records_items_roving_focus_and_overlay_policy() {
    let state = Menu::new("file-menu", "File")
        .open(true)
        .default_focused_value("save")
        .item(MenuItem::action("new", "New"))
        .item(MenuItem::action("save", "Save"))
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), MenuOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Menu);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Menu);
    assert!(state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(state.focused_value(), Some("save"));
    assert_eq!(state.items().len(), 4);
    assert_eq!(state.items()[0].role(), Some(Role::MenuItem));
    assert_eq!(state.items()[2].kind(), MenuItemKind::Separator);
    assert!(!state.items()[2].focusable());
    assert!(state.items()[3].disabled());
    assert!(!state.items()[3].activation_enabled());
    assert_eq!(state.colors().surface().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[test]
fn menu_state_defaults_focus_to_first_focusable_item_when_open() {
    let state = Menu::new("file-menu", "File")
        .open(true)
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("save", "Save"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.focused_value(), Some("save"));
    assert_eq!(state.items()[0].kind(), MenuItemKind::Separator);
    assert!(state.items()[2].disabled());
}

#[test]
fn menu_navigation_and_activation_skip_disabled_and_separator_items() {
    let state = Menu::new("edit-menu", "Edit")
        .open(true)
        .default_focused_value("copy")
        .items([
            MenuItem::action("cut", "Cut"),
            MenuItem::action("copy", "Copy"),
            MenuItem::separator("separator"),
            MenuItem::action("paste", "Paste").disabled(true),
            MenuItem::action("select-all", "Select all"),
        ])
        .state();
    let disabled = [false, false, true, true, false];

    assert_eq!(menu_navigation_target("down", 1, &disabled), Some(4));
    assert_eq!(menu_navigation_target("up", 1, &disabled), Some(0));
    assert_eq!(menu_navigation_target("home", 4, &disabled), Some(0));
    assert_eq!(menu_navigation_target("end", 0, &disabled), Some(4));
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("select-all")
    );
    assert_eq!(
        state.activation_for_key("enter").map(|selection| {
            (
                selection.index(),
                selection.value().to_owned(),
                selection.label().to_owned(),
            )
        }),
        Some((1, "copy".to_owned(), "Copy".to_owned()))
    );
    assert!(state.activation_for_key("space").is_some());
    assert!(state.activation_for_key("escape").is_none());
}

#[test]
fn menu_state_resolves_checked_radio_and_submenu_item_contracts() {
    let state = Menu::new("view-menu", "View")
        .open(true)
        .default_focused_value("show-hidden")
        .items([
            MenuItem::checkbox("show-hidden", "Show hidden files", true),
            MenuItem::radio("density-compact", "Compact", false),
            MenuItem::radio("density-comfortable", "Comfortable", true),
            MenuItem::submenu(
                "sort",
                "Sort by",
                [
                    MenuItem::action("name", "Name"),
                    MenuItem::action("name", "Nested duplicate name"),
                ],
            ),
            MenuItem::submenu("empty", "Empty submenu", []),
            MenuItem::separator("separator"),
        ])
        .state();

    assert_eq!(state.items()[0].kind(), MenuItemKind::Checkbox);
    assert!(state.items()[0].checked());
    assert_eq!(state.items()[0].toggled(), Some(Toggled::True));
    assert!(state.items()[0].activation_enabled());
    let checkbox_selection = state
        .activation_for_key("enter")
        .expect("focused checkbox should activate");
    assert_eq!(checkbox_selection.kind(), MenuItemKind::Checkbox);
    assert!(checkbox_selection.checked());
    assert_eq!(checkbox_selection.path_key(), "0:show-hidden");

    assert_eq!(state.items()[1].kind(), MenuItemKind::Radio);
    assert_eq!(state.items()[1].toggled(), Some(Toggled::False));
    assert_eq!(state.items()[2].toggled(), Some(Toggled::True));

    let submenu = &state.items()[3];
    assert_eq!(submenu.kind(), MenuItemKind::Submenu);
    assert!(submenu.focusable());
    assert!(!submenu.activation_enabled());
    assert_eq!(submenu.child_count(), 2);
    assert_eq!(submenu.children()[0].parent_value(), Some("sort"));
    assert_eq!(submenu.path_key(), "3:sort");
    assert_eq!(submenu.children()[0].path_key(), "3:sort/0:name");
    assert_eq!(submenu.children()[1].path_key(), "3:sort/1:name");

    let empty_submenu = &state.items()[4];
    assert_eq!(empty_submenu.kind(), MenuItemKind::Submenu);
    assert!(!empty_submenu.focusable());
    assert!(!empty_submenu.activation_enabled());
    assert!(!state.items()[5].focusable());
}

#[test]
fn menu_state_resolves_typeahead_without_runtime_timer_state() {
    let state = Menu::new("search-menu", "Search")
        .open(true)
        .default_focused_value("beta")
        .items([
            MenuItem::action("alpha", "Alpha"),
            MenuItem::action("beta", "Beta"),
            MenuItem::separator("separator"),
            MenuItem::action("blocked", "Bravo blocked").disabled(true),
            MenuItem::checkbox("bravo", "Bravo visible", false),
            MenuItem::submenu("empty", "Bravo empty submenu", []),
        ])
        .state();

    assert_eq!(
        state.typeahead_target(" br").map(|item| item.value()),
        Some("bravo")
    );
    assert_eq!(
        state.typeahead_target("AL").map(|item| item.value()),
        Some("alpha")
    );
    assert!(state.typeahead_target("").is_none());
    assert!(state.typeahead_target("missing").is_none());
}

#[test]
fn menu_state_resolves_visible_submenu_navigation_and_local_scroll_contract() {
    let state = Menu::new("nested-menu", "Nested")
        .open(true)
        .default_focused_value("sort")
        .items([
            MenuItem::action("open", "Open"),
            MenuItem::submenu(
                "sort",
                "Sort by",
                [
                    MenuItem::action("name", "Name"),
                    MenuItem::action("modified", "Modified"),
                ],
            ),
            MenuItem::action("close", "Close"),
        ])
        .state();

    assert_eq!(state.focused_value(), Some("sort"));
    assert_eq!(state.focused_path_key().as_deref(), Some("1:sort"));
    assert_eq!(state.visible_items().len(), 3);
    let opened = state
        .submenu_navigation_target("right")
        .expect("Right should open a focused submenu");
    let _: open_gpui_ui_components::MenuSubmenuNavigation = opened.clone();
    assert_eq!(opened.open_path_key().as_deref(), Some("1:sort"));
    assert_eq!(opened.focused_path_key(), "1:sort/0:name");
    assert_eq!(opened.focused_value(), "name");

    let long_state = Menu::new("long-menu", "Long")
        .open(true)
        .items(
            (0..10).map(|index| MenuItem::action(format!("item-{index}"), format!("Item {index}"))),
        )
        .state();
    assert!(long_state.scrollable_content());
    assert_eq!(long_state.visible_items().len(), 10);
}

#[test]
fn menu_state_discards_invalid_runtime_submenu_paths_after_items_change() {
    let state = Menu::new("changed-menu", "Changed")
        .open(true)
        .default_focused_value("sort")
        .item(MenuItem::submenu("sort", "Sort by", []))
        .state();

    assert_eq!(state.visible_items().len(), 1);
    assert_eq!(state.focused_value(), None);
    assert_eq!(state.open_path_key(), None);
    assert!(!state.items()[0].focusable());
    assert!(state.submenu_navigation_target("right").is_none());
}

#[test]
fn menu_runtime_keyboard_navigation_keeps_runtime_focused_value_after_rerender() {
    let state = Menu::new("runtime-menu", "Runtime menu")
        .open(true)
        .default_focused_value("copy")
        .items([
            MenuItem::action("cut", "Cut"),
            MenuItem::action("copy", "Copy"),
            MenuItem::action("select-all", "Select all"),
        ])
        .state();

    assert_eq!(state.focused_value(), Some("copy"));
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("select-all")
    );
}

#[open_gpui::test]
fn menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<MenuSelection>>>,
        item_selections: Rc<RefCell<Vec<MenuSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let item_selections = self.item_selections.clone();

            div().size_full().child(
                Menu::new("runtime-menu", "Runtime menu")
                    .default_focused_value("copy")
                    .item(MenuItem::action("cut", "Cut"))
                    .item(MenuItem::action("copy", "Copy"))
                    .item(MenuItem::action("select-all", "Select all").on_select(
                        move |selection, _, _| {
                            item_selections.borrow_mut().push(selection);
                        },
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let item_selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        item_selections: item_selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:runtime-menu:trigger")
        .expect("runtime menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("menu:runtime-menu:content").is_some(),
        "runtime menu content should render when opened"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move the runtime focus without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after_enter = selections.borrow().clone();
    let after_item_enter = item_selections.borrow().clone();
    assert_eq!(after_enter.len(), 1);
    assert_eq!(after_enter[0].index(), 2);
    assert_eq!(after_enter[0].value(), "select-all");
    assert_eq!(after_enter[0].label(), "Select all");
    assert_eq!(after_item_enter.len(), 1);
    assert_eq!(after_item_enter[0].path_key(), "2:select-all");
}

#[open_gpui::test]
fn menu_runtime_keyboard_submenu_opens_and_selects_child(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<MenuSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Menu::new("runtime-submenu", "Runtime submenu")
                    .default_focused_value("sort")
                    .item(MenuItem::action("open", "Open"))
                    .item(MenuItem::submenu(
                        "sort",
                        "Sort by",
                        [
                            MenuItem::action("name", "Name"),
                            MenuItem::action("modified", "Modified"),
                        ],
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:runtime-submenu:trigger")
        .expect("runtime submenu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_none(),
        "submenu child should not render before opening the branch"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_some(),
        "Right should open the focused submenu branch"
    );

    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_none(),
        "Left should close the active submenu branch and return focus to the trigger row"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_some(),
        "Right should reopen the submenu branch after closing it"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after_enter = selections.borrow().clone();
    assert_eq!(after_enter.len(), 1);
    assert_eq!(after_enter[0].path_key(), "1:sort/0:name");
    assert_eq!(after_enter[0].value(), "name");
}

#[open_gpui::test]
fn menu_runtime_hover_opens_submenu_and_preserves_child_focus(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Menu::new("hover-submenu", "Hover submenu")
                    .item(MenuItem::action("open", "Open"))
                    .item(MenuItem::submenu(
                        "sort",
                        "Sort by",
                        [
                            MenuItem::action("name", "Name"),
                            MenuItem::action("modified", "Modified"),
                        ],
                    ))
                    .item(MenuItem::action("rename", "Rename")),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:hover-submenu:trigger")
        .expect("hover submenu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_none(),
        "submenu child should not render before hovering the submenu trigger"
    );

    let sort = cx
        .debug_bounds("menu:hover-submenu:item:1:sort")
        .expect("submenu trigger item should render");
    cx.simulate_mouse_move(sort.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering a submenu trigger should open its branch"
    );

    let child = cx
        .debug_bounds("menu:hover-submenu:item:1:sort/0:name")
        .expect("submenu child should render after hover");
    cx.simulate_mouse_move(child.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering inside the open submenu branch should preserve that branch"
    );

    let rename = cx
        .debug_bounds("menu:hover-submenu:item:2:rename")
        .expect("next root item should render");
    cx.simulate_mouse_move(rename.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_none(),
        "hovering another root item should close the previous submenu branch"
    );
}

#[test]
fn menu_state_models_default_open_disabled_and_policy_overrides() {
    let state = Menu::new("disabled-menu", "Disabled")
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::Ignore)
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .item(MenuItem::action("open", "Open"))
        .state();

    assert_eq!(state.open_mode(), MenuOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn context_menu_state_reuses_menu_model_and_point_anchor_placement() {
    let anchor = point(px(280.0), px(160.0));
    let state = ContextMenu::new("canvas-context-menu", "Canvas menu")
        .open(true)
        .anchor_point(anchor)
        .default_focused_value("duplicate")
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), MenuOpenMode::Controlled);
    let neutral_anchor = ui_point(ui_px(280.0), ui_px(160.0));
    assert_eq!(state.anchor_point(), neutral_anchor);
    assert_eq!(state.content_role(), Role::Menu);
    assert_eq!(state.menu().focused_value(), Some("duplicate"));
    assert_eq!(state.menu().items()[1].kind(), MenuItemKind::Separator);
    assert!(!state.menu().items()[2].activation_enabled());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Menu);
    assert!(state.overlay().wants_outside_press_handler());
    let placement_input = state.placement_input();
    assert_eq!(placement_input.side(), OverlayPlacementSide::Bottom);
    assert_eq!(
        placement_input.alignment(),
        OverlayPlacementAlignment::Start
    );
    assert_eq!(placement_input.offset(), ui_px(0.0));
    let placement_row_gap =
        ui_px(4.0).as_f32() * (state.menu().visible_items().len().saturating_sub(1) as f32);
    assert_eq!(
        placement_input.content_size(),
        ui_size(
            ui_px(state.metrics().min_width().as_f32()),
            ui_px(
                state.metrics().surface_padding().as_f32() * 2.0
                    + state.metrics().item_height().as_f32()
                        * state.menu().visible_items().len() as f32
                    + placement_row_gap
            )
        )
    );
    let placement = GpuiOverlayPlacement::resolve(placement_input, DEFAULT_OVERLAY_SAFE_MARGIN);
    assert_eq!(placement.anchor(), Anchor::TopLeft);
    assert_eq!(
        placement_input.preferred_anchor_bounds(),
        Some(open_gpui_ui_core::anchor_rect_from_point(neutral_anchor))
    );
    assert_eq!(placement.position(), Some(point(px(280.0), px(161.0))));
    assert_eq!(placement.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
}

#[test]
fn context_menu_state_defaults_focus_to_first_focusable_item_when_open() {
    let anchor = point(px(280.0), px(160.0));
    let state = ContextMenu::new("canvas-context-menu", "Canvas menu")
        .open(true)
        .anchor_point(anchor)
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert_eq!(state.menu().focused_value(), Some("duplicate"));
    assert!(state.menu().items()[0].kind() == MenuItemKind::Separator);
}

#[test]
fn context_menu_state_navigation_target_prefers_runtime_focused_value() {
    let anchor = point(px(280.0), px(160.0));
    let state = ContextMenu::new("runtime-context-menu", "Runtime context menu")
        .open(true)
        .anchor_point(anchor)
        .default_focused_value("copy")
        .item(MenuItem::action("cut", "Cut"))
        .item(MenuItem::action("copy", "Copy"))
        .item(MenuItem::action("select-all", "Select all"))
        .state();

    assert_eq!(state.menu().focused_value(), Some("copy"));
    assert_eq!(
        state
            .menu()
            .navigation_target("down")
            .map(|item| item.value()),
        Some("select-all")
    );
}

#[test]
fn context_menu_state_reuses_visible_submenu_navigation_contract() {
    let anchor = point(px(320.0), px(220.0));
    let state = ContextMenu::new("nested-context-menu", "Nested context menu")
        .open(true)
        .anchor_point(anchor)
        .default_focused_value("organize")
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::submenu(
            "organize",
            "Organize",
            [
                MenuItem::action("move", "Move"),
                MenuItem::action("tag", "Tag"),
            ],
        ))
        .state();

    assert_eq!(state.menu().focused_value(), Some("organize"));
    let opened = state
        .menu()
        .submenu_navigation_target("right")
        .expect("ContextMenu should reuse Menu submenu navigation");
    assert_eq!(opened.open_path_key().as_deref(), Some("1:organize"));
    assert_eq!(opened.focused_path_key(), "1:organize/0:move");
}

#[test]
fn context_menu_state_uses_clamped_visible_menu_size_for_point_placement() {
    let state = ContextMenu::new("edge-long-context-menu", "Edge long context menu")
        .open(true)
        .anchor_point(point(px(960.0), px(560.0)))
        .items(
            (0..12).map(|index| MenuItem::action(format!("item-{index}"), format!("Item {index}"))),
        )
        .state();

    assert!(state.menu().scrollable_content());
    assert_eq!(
        state.placement_input().content_size(),
        ui_size(state.metrics().min_width(), state.metrics().max_height())
    );
}

#[open_gpui::test]
fn context_menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<MenuSelection>>>,
        item_selections: Rc<RefCell<Vec<MenuSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let item_selections = self.item_selections.clone();

            div().size_full().child(
                ContextMenu::new("runtime-context-menu", "Runtime context menu")
                    .anchor_point(point(px(280.0), px(160.0)))
                    .default_focused_value("copy")
                    .item(MenuItem::action("cut", "Cut"))
                    .item(MenuItem::action("copy", "Copy"))
                    .item(MenuItem::action("select-all", "Select all").on_select(
                        move |selection, _, _| {
                            item_selections.borrow_mut().push(selection);
                        },
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let item_selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        item_selections: item_selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let hotspot = cx
        .debug_bounds("context-menu:runtime-context-menu:hotspot")
        .expect("runtime context menu hotspot should render");
    cx.simulate_mouse_down(hotspot.center(), MouseButton::Right, Default::default());
    cx.simulate_mouse_up(hotspot.center(), MouseButton::Right, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("context-menu:runtime-context-menu:surface")
            .is_some(),
        "runtime context menu surface should render when opened"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move the runtime focus without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after_enter = selections.borrow().clone();
    let after_item_enter = item_selections.borrow().clone();
    assert_eq!(after_enter.len(), 1);
    assert_eq!(after_enter[0].index(), 2);
    assert_eq!(after_enter[0].value(), "select-all");
    assert_eq!(after_enter[0].label(), "Select all");
    assert_eq!(after_item_enter.len(), 1);
    assert_eq!(after_item_enter[0].path_key(), "2:select-all");
}

#[test]
fn default_button_state_uses_button_role_and_medium_metrics() {
    let state = Button::new("save", "Save").state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Default);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.button_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.button_px());
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert_eq!(state.focus_ring().width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn destructive_button_uses_destructive_token_intent() {
    let state = Button::new("delete", "Delete")
        .variant(ButtonVariant::Destructive)
        .state();

    assert_eq!(state.colors().background().token(), semantic::DESTRUCTIVE);
    assert_eq!(
        state.colors().foreground().token(),
        semantic::DESTRUCTIVE_FOREGROUND
    );
}

#[test]
fn disabled_button_blocks_activation_metadata() {
    let state = Button::new("disabled", "Disabled").disabled(true).state();

    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn button_size_helpers_apply_foundation_size_metrics() {
    let state = Button::new("large", "Large").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[test]
fn tabs_navigation_helpers_skip_disabled_tabs() {
    let keys = vec![
        "overview".to_string(),
        "details".to_string(),
        "history".to_string(),
    ];
    let disabled = [false, true, false];

    assert_eq!(first_enabled(&disabled), Some(0));
    assert_eq!(last_enabled(&disabled), Some(2));
    assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
    assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    assert_eq!(
        active_index_from_str_keys(&keys, Some("details"), &disabled),
        Some(0)
    );
    assert_eq!(
        active_index_from_str_keys(&keys, Some("missing"), &disabled),
        Some(0)
    );
}

#[test]
fn tabs_state_resolution_tracks_selected_focus_and_tab_stop() {
    let state = TabsState::resolve(
        Orientation::Vertical,
        TabsActivationMode::Manual,
        Size::Small,
        Some("security"),
        Some("billing"),
        [
            TabsItemDescriptor::new("profile", "Profile"),
            TabsItemDescriptor::new("security", "Security"),
            TabsItemDescriptor::new("billing", "Billing").disabled(true),
            TabsItemDescriptor::new("integrations", "Integrations"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.activation_mode(), TabsActivationMode::Manual);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("security"));
    assert_eq!(state.focused_value(), Some("security"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].focused());
}

#[test]
fn tabs_builder_state_falls_back_to_first_enabled_tab() {
    let state = Tabs::new("settings")
        .orientation(Orientation::Horizontal)
        .activation_mode(TabsActivationMode::Automatic)
        .with_size(Size::Large)
        .default_selected("history")
        .item(TabsItem::new("overview", "Overview", div()))
        .item(TabsItem::new("details", "Details", div()))
        .item(TabsItem::new("history", "History", div()).disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.activation_mode(), TabsActivationMode::Automatic);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.selected_value(), Some("overview"));
    assert_eq!(state.focused_value(), Some("overview"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn scroll_area_state_exposes_axis_metrics_and_reset_policy() {
    let state = ScrollAreaState::resolve(
        "activity-log",
        ScrollAreaAxis::Both,
        Size::Small,
        ScrollResetPolicy::ResetOnKeyChange,
        Some("components".to_string()),
    );

    assert_eq!(state.viewport_id(), "activity-log");
    assert_eq!(state.axis(), ScrollAreaAxis::Both);
    assert_eq!(state.axis().as_str(), "both");
    assert_eq!(state.size(), Size::Small);
    assert!(state.scrolls_x());
    assert!(state.scrolls_y());
    assert_eq!(state.reset_policy(), ScrollResetPolicy::ResetOnKeyChange);
    assert_eq!(state.reset_policy().as_str(), "reset-on-key-change");
    assert_eq!(state.reset_key(), Some("components"));
    assert_eq!(state.metrics().scrollbar_width(), ui_px(8.0));
    assert!(state.should_reset_for_key_change(Some("tokens")));
    assert!(!state.should_reset_for_key_change(Some("components")));
    assert!(!state.should_reset_for_key_change(None));
}

#[test]
fn scroll_area_builder_state_keeps_gpui_handle_out_of_resolved_state() {
    let external_handle = open_gpui::ScrollHandle::new();
    let state = ScrollArea::new("component-scroll", div())
        .horizontal()
        .large()
        .reset_on_key("settings")
        .state();
    let preserved = ScrollArea::new("preserved-scroll", div())
        .both()
        .scroll_handle(&external_handle)
        .preserve_scroll()
        .state();

    assert_eq!(state.viewport_id(), "component-scroll");
    assert_eq!(state.axis(), ScrollAreaAxis::Horizontal);
    assert!(state.scrolls_x());
    assert!(!state.scrolls_y());
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().scrollbar_width(), ui_px(12.0));
    assert_eq!(state.reset_key(), Some("settings"));
    assert!(state.should_reset_for_key_change(Some("overview")));
    assert_eq!(preserved.reset_policy(), ScrollResetPolicy::Preserve);
    assert_eq!(preserved.reset_key(), None);
    assert!(!preserved.should_reset_for_key_change(Some("overview")));
}

#[test]
fn table_render_plan_uses_core_state_and_virtualizer_contracts() {
    let state = sample_table_state(100)
        .with_sorting([TableSort::new("score", TableSortDirection::Descending)])
        .with_selected_rows(["row-0091"])
        .with_filters([TableFilter::contains("team", "UI")])
        .with_pagination(TablePagination::disabled());
    let table = Table::new("contracts-table", "Contracts", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .overscan(4);
    let plan = table.render_plan(ui_px(120.0), ui_px(96.0));

    assert_eq!(plan.role(), Role::Table);
    assert_eq!(plan.row_role(), Role::Row);
    assert_eq!(plan.column_header_role(), Role::ColumnHeader);
    assert_eq!(plan.cell_role(), Role::Cell);
    assert_eq!(plan.columns().len(), 3);
    assert_eq!(plan.aria_column_count(), 3);
    assert_eq!(plan.aria_row_count(), 51);
    assert_eq!(
        *plan.virtualizer().visible_range(),
        VirtualizerRange::new(5, 9)
    );
    assert_eq!(
        *plan.virtualizer().overscan_range(),
        VirtualizerRange::new(3, 11)
    );
    assert!(plan.rendered_row_count() <= plan.visible_row_count() + plan.metrics().overscan());
    assert_eq!(plan.rows()[0].model_index(), 3);
    assert_eq!(plan.rows()[0].id().as_str(), "row-0093");
    assert!(
        plan.rows()
            .iter()
            .any(|row| row.id().as_str() == "row-0091" && row.selected()),
        "expected selection to follow row id after filtering and sorting"
    );

    let score_column = plan
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should be present");
    assert_eq!(
        score_column.sort_direction(),
        Some(TableSortDirection::Descending)
    );
    assert_eq!(score_column.accessible_label(), "Score, sorted descending");
}

#[test]
fn table_render_plan_exposes_tree_row_metadata_for_adapter_rendering() {
    let state = TableState::new([TableRow::new("root")
        .with_cell("name", "Workspace")
        .with_cell("status", "Ready")
        .with_child(
            TableRow::new("child")
                .with_cell("name", "UI")
                .with_cell("status", "Building"),
        )])
    .with_columns([
        TableColumn::new("name", "Name").with_width(ui_px(160.0)),
        TableColumn::new("status", "Status").with_width(ui_px(120.0)),
    ])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_expanded_rows(["root"])
    .with_pagination(TablePagination::disabled());
    let plan = Table::new("tree-table", "Tree table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));

    assert_eq!(plan.rows().len(), 2);
    assert_eq!(plan.rows()[0].id().as_str(), "root");
    assert!(plan.rows()[0].is_tree_branch());
    assert_eq!(plan.rows()[0].tree_expanded(), Some(true));
    assert_eq!(plan.rows()[0].depth(), 0);
    assert_eq!(plan.rows()[1].id().as_str(), "child");
    assert!(!plan.rows()[1].is_tree_branch());
    assert_eq!(plan.rows()[1].tree_expanded(), None);
    assert_eq!(plan.rows()[1].depth(), 1);
    assert_eq!(
        plan.rows()[0]
            .cells_for_region(TableColumnRegion::Left)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["name"]
    );
}

#[test]
fn table_render_plan_exposes_manual_expansion_and_child_load_metadata() {
    let manual_state = TableState::new([TableRow::new("root")
        .with_cell("name", "Workspace")
        .with_child(TableRow::new("child").with_cell("name", "Loaded child"))])
    .with_columns([TableColumn::new("name", "Name").with_width(ui_px(160.0))])
    .with_pagination(TablePagination::disabled());
    let manual_plan = Table::new("manual-tree", "Manual tree", manual_state)
        .expansion_mode(TableExpansionMode::Manual)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));

    assert_eq!(
        manual_plan
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["root", "child"],
        "manual expansion should render the caller-supplied visible tree snapshot"
    );
    assert_eq!(manual_plan.rows()[0].tree_expanded(), Some(false));
    assert_eq!(manual_plan.rows()[0].loaded_child_count(), 1);
    assert_eq!(
        manual_plan.rows()[0].children_load_state(),
        Some(&TableRowChildrenLoadState::Idle)
    );

    let loading_state = TableState::new([TableRow::new("remote")
        .with_cell("name", "Remote branch")
        .with_children_loading("Loading children")])
    .with_columns([TableColumn::new("name", "Name").with_width(ui_px(160.0))])
    .with_pagination(TablePagination::disabled());
    let loading_plan = Table::new("loading-tree", "Loading tree", loading_state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));
    let loading_row = &loading_plan.rows()[0];

    assert!(loading_row.is_tree_branch());
    assert_eq!(loading_row.loaded_child_count(), 0);
    assert_eq!(
        loading_row
            .children_load_state()
            .and_then(TableRowChildrenLoadState::message),
        Some("Loading children")
    );
    assert!(
        loading_row
            .children_load_state()
            .is_some_and(TableRowChildrenLoadState::is_loading)
    );
}

#[test]
fn table_render_plan_exposes_manual_row_model_metadata() {
    let state = TableState::new([
        TableRow::new("row-020")
            .with_cell("name", "Delta")
            .with_cell("team", "UI")
            .with_cell("score", 20_usize),
        TableRow::new("row-021")
            .with_cell("name", "Echo")
            .with_cell("team", "Platform")
            .with_cell("score", 21_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_width(ui_px(160.0)),
        TableColumn::new("team", "Team").with_width(ui_px(120.0)),
        TableColumn::new("score", "Score").with_width(ui_px(96.0)),
    ])
    .with_filters([TableFilter::contains("team", "missing")])
    .with_manual_filtering()
    .with_sorting([TableSort::ascending("score")])
    .with_manual_sorting()
    .with_pagination(TablePagination::manual(10, 2, 42));

    let plan = Table::new("manual-row-model", "Manual row model", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));

    assert_eq!(plan.filtering_mode(), TableStageMode::Manual);
    assert_eq!(plan.sorting_mode(), TableStageMode::Manual);
    assert_eq!(plan.pagination_mode(), TableStageMode::Manual);
    assert_eq!(plan.pagination_row_count(), Some(42));
    assert_eq!(plan.pagination_page_count(), Some(21));
    assert_eq!(
        plan.rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-020", "row-021"],
        "manual row-model stages should render the caller-supplied page snapshot"
    );
}

#[test]
fn table_render_plan_exposes_faceting_metadata() {
    let state = TableState::new([
        TableRow::new("row-1")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-2")
            .with_cell("team", "UI")
            .with_cell("status", "Blocked")
            .with_cell("score", 20_usize),
        TableRow::new("row-3")
            .with_cell("team", "API")
            .with_cell("status", "Ready")
            .with_cell("score", 30_usize),
        TableRow::new("row-4")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("status", "Ready"),
        TableFilter::contains("team", "UI"),
    ])
    .with_pagination(TablePagination::new(0, 1))
    .with_manual_facets(
        [TableColumnFacets::manual("status", 64).with_unique_values([
            TableFacetValueCount::new("Blocked", 24),
            TableFacetValueCount::new("Ready", 40),
        ])],
    );

    let plan = Table::new("faceted-table", "Faceted table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));

    assert_eq!(plan.faceting_mode(), TableStageMode::Client);
    assert_eq!(plan.column_facets().len(), 3);
    assert_eq!(
        plan.rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-1"],
        "pagination still limits the rendered row window"
    );

    let status = plan
        .column_facet(&TableColumnId::new("status"))
        .expect("status facet should resolve");
    assert_eq!(status.mode(), TableStageMode::Manual);
    assert_eq!(status.row_count(), 64);
    assert_eq!(
        text_facet_counts(status),
        [("Blocked".to_string(), 24), ("Ready".to_string(), 40)],
        "manual facet payloads should survive render-plan resolution"
    );

    let team = plan
        .column_facet(&TableColumnId::new("team"))
        .expect("team facet should resolve");
    assert_eq!(team.mode(), TableStageMode::Client);
    assert_eq!(team.row_count(), 3);
    assert_eq!(
        text_facet_counts(team),
        [("API".to_string(), 1), ("UI".to_string(), 2)],
        "client facets ignore their own column filter and honor the other filters"
    );

    let score = plan
        .column_facet(&TableColumnId::new("score"))
        .expect("score facet should resolve");
    let range = score
        .numeric_range()
        .expect("score facet should expose a numeric range");
    assert_eq!(range.min(), 10.0);
    assert_eq!(range.max(), 40.0);
}

#[test]
fn table_render_plan_exposes_global_facet_summary() {
    let state = TableState::new([
        TableRow::new("row-1")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize)
            .with_cell("enabled", true)
            .with_cell("tag", "alpha")
            .with_cell("notes", "ready"),
        TableRow::new("row-2")
            .with_cell("team", "UI")
            .with_cell("status", "Blocked")
            .with_cell("score", 20_usize)
            .with_cell("enabled", false)
            .with_cell("notes", "done"),
        TableRow::new("row-3")
            .with_cell("team", "API")
            .with_cell("status", "Ready")
            .with_cell("score", 30_usize)
            .with_cell("enabled", true)
            .with_cell("tag", "beta")
            .with_cell("notes", "done"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
        TableColumn::new("enabled", "Enabled"),
        TableColumn::new("tag", "Tag"),
        TableColumn::new("notes", "Notes").with_global_filterable(false),
    ])
    .with_filters([TableFilter::contains("team", "UI")])
    .with_global_filter("done")
    .with_pagination(TablePagination::disabled());

    let plan = Table::new("global-facet-table", "Global facet table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));

    let summary: &TableGlobalFacetSummary = plan.global_facet_summary();
    assert_eq!(summary.mode(), TableStageMode::Client);
    assert_eq!(summary.row_count(), 2);
    assert!(summary.column_facet(&TableColumnId::new("notes")).is_none());
    assert_eq!(
        summary
            .column_facets()
            .iter()
            .map(|facet| facet.column().as_str())
            .collect::<Vec<_>>(),
        ["team", "status", "score", "enabled", "tag"]
    );
    assert_eq!(
        text_facet_counts(
            summary
                .column_facet(&TableColumnId::new("status"))
                .expect("status global facet should resolve")
        ),
        [("Blocked".to_string(), 1), ("Ready".to_string(), 1)]
    );
}

#[test]
fn table_faceted_filter_state_resolves_query_selection_and_popover_contract() {
    let facets = TableColumnFacets::manual("status", 4).with_unique_values([
        TableFacetValueCount::new("Ready", 2),
        TableFacetValueCount::new("Blocked", 1),
        TableFacetValueCount::new("Review", 1),
    ]);

    let state: TableFacetedFilterState =
        TableFacetedFilter::new("status-filter", "Status", "status")
            .facets(facets)
            .selected_values(["Ready", "Blocked"])
            .query("rea")
            .open(true)
            .placeholder("Find status")
            .empty_label("No statuses")
            .clear_label("Reset")
            .small()
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::End)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .state();

    assert_eq!(state.id(), "status-filter");
    assert_eq!(state.label(), "Status");
    assert_eq!(state.column_id().as_str(), "status");
    assert_eq!(state.query(), "rea");
    assert_eq!(state.trigger_label(), "Status: Ready, Blocked");
    assert_eq!(
        state.selected_values(),
        &["Blocked".to_string(), "Ready".to_string()]
    );
    assert_eq!(
        state.selected_labels(),
        &["Ready".to_string(), "Blocked".to_string()]
    );
    assert_eq!(state.total_option_count(), 3);
    assert!(state.clear_enabled());
    assert_eq!(state.empty_label(), "No statuses");
    assert_eq!(state.clear_label(), "Reset");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.search_input().value(), "rea");
    assert_eq!(state.search_input().placeholder(), Some("Find status"));
    assert_eq!(state.search_input().size(), Size::Small);
    assert!(state.search_input().controller_driven());

    let options = state
        .options()
        .iter()
        .map(|option| {
            (
                option.value().to_owned(),
                option.label().to_owned(),
                option.count(),
                option.selected(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        options,
        vec![("Ready".to_string(), "Ready".to_string(), 2, true)]
    );
}

#[test]
fn table_faceted_filter_state_reports_empty_query_result() {
    let state = TableFacetedFilter::new("status-filter", "Status", "status")
        .facets(TableColumnFacets::manual("status", 2).with_unique_values([
            TableFacetValueCount::new("Ready", 1),
            TableFacetValueCount::new("Blocked", 1),
        ]))
        .query("missing")
        .empty_label("No matching statuses")
        .state();

    assert!(state.empty());
    assert!(!state.clear_enabled());
    assert_eq!(state.total_option_count(), 2);
    assert_eq!(state.empty_label(), "No matching statuses");
    assert!(state.options().is_empty());
}

#[test]
fn table_faceted_filter_change_updates_filters_and_resets_pagination() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("status", "Ready"),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("status", "Blocked"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::one_of("status", ["Ready"]),
    ])
    .with_pagination(TablePagination::new(3, 25));

    let change =
        TableFacetedFilterChange::new("status", ["Blocked", "Ready"], Some("Blocked"), true);
    assert_eq!(change.column_id().as_str(), "status");
    assert_eq!(change.toggled_value(), Some("Blocked"));
    assert!(change.selected());
    assert!(!change.cleared());

    let next = change.apply_to(state);
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters().len(), 2);
    let team_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "team")
        .expect("unrelated team filter should be preserved");
    assert_eq!(team_filter.query(), "UI");
    let status_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "status")
        .expect("status filter should be replaced by the faceted selection");
    assert_eq!(
        status_filter
            .selected_values()
            .expect("status filter should be categorical")
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["Blocked".to_string(), "Ready".to_string()]
    );

    let cleared = TableFacetedFilterChange::clear("status");
    assert!(cleared.cleared());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 1);
    assert_eq!(cleared_state.filters()[0].column().as_str(), "team");
}

#[test]
fn table_range_filter_state_resolves_bounds_and_popover_contract() {
    let facets = TableColumnFacets::manual("score", 64).with_numeric_range(1.0, 64.0);

    let state: TableRangeFilterState = TableRangeFilter::new("score-range", "Score", "score")
        .facets(facets)
        .range(Some(40.0), Some(10.0))
        .open(true)
        .clear_label("Reset score")
        .small()
        .placement_side(OverlayPlacementSide::Top)
        .placement_alignment(OverlayPlacementAlignment::End)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .state();

    assert_eq!(state.id(), "score-range");
    assert_eq!(state.label(), "Score");
    assert_eq!(state.column_id().as_str(), "score");
    assert_eq!(state.min_text(), "10");
    assert_eq!(state.max_text(), "40");
    assert_eq!(state.min_value(), Some(10.0));
    assert_eq!(state.max_value(), Some(40.0));
    assert_eq!(state.trigger_label(), "Score: 10-40");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.clear_label(), "Reset score");
    let facet_range = state
        .facet_range()
        .expect("manual score facets should expose a numeric range");
    assert_eq!(facet_range.min(), 1.0);
    assert_eq!(facet_range.max(), 64.0);
    assert_eq!(state.min_placeholder(), "Min (1)");
    assert_eq!(state.max_placeholder(), "Max (64)");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.min_input().value(), "10");
    assert_eq!(state.max_input().value(), "40");
    assert_eq!(state.min_input().placeholder(), Some("Min (1)"));
    assert_eq!(state.max_input().placeholder(), Some("Max (64)"));
    assert!(state.min_input().controller_driven());
    assert!(state.max_input().controller_driven());
}

#[test]
fn table_range_filter_change_updates_filters_and_resets_pagination() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::contains("score", "1"),
        TableFilter::number_range("score", Some(5.0), Some(20.0))
            .expect("initial score range should be valid"),
    ])
    .with_pagination(TablePagination::new(3, 25));

    let change = TableRangeFilterChange::new("score", "30", "10");
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.min_text(), "30");
    assert_eq!(change.max_text(), "10");
    assert_eq!(change.min_value(), Some(10.0));
    assert_eq!(change.max_value(), Some(30.0));
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state);
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters().len(), 3);
    let team_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "team")
        .expect("unrelated team filter should be preserved");
    assert_eq!(team_filter.query(), "UI");
    let score_text_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "score" && filter.query() == "1")
        .expect("same-column non-range filter should be preserved");
    assert_eq!(score_text_filter.number_range_bounds(), None);
    let score_filter = next
        .filters()
        .iter()
        .find(|filter| {
            filter.column().as_str() == "score" && filter.number_range_bounds().is_some()
        })
        .expect("score filter should be replaced by the range selection");
    assert_eq!(
        score_filter.number_range_bounds(),
        Some((Some(10.0), Some(30.0)))
    );

    let cleared = TableRangeFilterChange::clear("score");
    assert!(cleared.cleared());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 2);
    assert!(
        cleared_state
            .filters()
            .iter()
            .all(|filter| filter.number_range_bounds().is_none())
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.column().as_str() == "team")
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.column().as_str() == "score" && filter.query() == "1")
    );
}

#[test]
fn table_column_visibility_state_resolves_items_counts_and_popover_contract() {
    let visibility = TableColumnVisibilityOverrides::new()
        .hide("name")
        .show("team")
        .hide("score");

    let state: TableColumnVisibilityState =
        TableColumnVisibility::new("column-visibility", "Columns")
            .columns([
                TableColumn::new("name", "Name").with_hideable(false),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .visibility(visibility)
            .open(true)
            .empty_label("No columns configured")
            .show_all_label("Show every column")
            .reset_label("Reset columns")
            .small()
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::End)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .state();

    assert_eq!(state.id(), "column-visibility");
    assert_eq!(state.label(), "Columns");
    assert_eq!(state.trigger_label(), "Columns: 1 hidden");
    assert_eq!(state.item_count(), 3);
    assert_eq!(state.visible_count(), 2);
    assert_eq!(state.hidden_count(), 1);
    assert_eq!(state.hideable_count(), 2);
    assert!(!state.all_visible());
    assert!(state.some_visible());
    assert!(state.show_all_enabled());
    assert!(state.reset_enabled());
    assert_eq!(state.empty_label(), "No columns configured");
    assert_eq!(state.show_all_label(), "Show every column");
    assert_eq!(state.reset_label(), "Reset columns");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );

    let items = state
        .items()
        .iter()
        .map(|item| {
            (
                item.column_id().as_str().to_owned(),
                item.label().to_owned(),
                item.checked(),
                item.hideable(),
                item.disabled(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        items,
        vec![
            ("name".to_string(), "Name".to_string(), true, false, true),
            ("team".to_string(), "Team".to_string(), true, true, false),
            ("score".to_string(), "Score".to_string(), false, true, false),
        ]
    );
}

#[test]
fn table_column_visibility_change_updates_visibility_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("name", "Beta")
            .with_cell("team", "Platform")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_hideable(false),
        TableColumn::new("team", "Team").with_visible(false),
        TableColumn::new("score", "Score"),
    ])
    .with_column_visibility(
        TableColumnVisibilityOverrides::new()
            .hide("name")
            .show("team")
            .hide("score"),
    )
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::number_range("score", Some(5.0), Some(50.0))
            .expect("finite score range should be valid"),
    ])
    .with_sorting([TableSort::descending("score")])
    .with_selected_rows(["row-a"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_row_pinning(TableRowPinning::new().pinned_top(["row-a"]))
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(180.0)))
    .with_pagination(TablePagination::new(2, 25));

    let change = TableColumnVisibilityChange::new("team", false);
    assert_eq!(change.action(), TableColumnVisibilityAction::ToggleColumn);
    assert_eq!(change.action().as_str(), "toggle_column");
    assert_eq!(change.column_id().map(TableColumnId::as_str), Some("team"));
    assert_eq!(change.column_ids(), &[TableColumnId::new("team")]);
    assert_eq!(change.next_visible(), Some(false));

    let next = change.apply_to(state.clone());
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("team")),
        Some(false)
    );
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("score")),
        Some(false)
    );
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("name")),
        Some(false)
    );
    assert_eq!(next.filters(), state.filters());
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.pagination(), state.pagination());
    assert_eq!(next.selected_rows(), state.selected_rows());
    assert_eq!(next.column_pinning(), state.column_pinning());
    assert_eq!(next.row_pinning(), state.row_pinning());
    assert_eq!(next.column_sizing(), state.column_sizing());

    let show_all = TableColumnVisibilityChange::show_all(["team", "score"]);
    assert_eq!(show_all.action(), TableColumnVisibilityAction::ShowAll);
    assert_eq!(show_all.action().as_str(), "show_all");
    assert_eq!(show_all.next_visible(), Some(true));
    assert_eq!(show_all.column_ids().len(), 2);
    let shown = show_all.apply_to(state.clone());
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("team")),
        Some(true)
    );
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("score")),
        Some(true)
    );
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("name")),
        Some(false)
    );

    let reset = TableColumnVisibilityChange::reset();
    assert_eq!(reset.action(), TableColumnVisibilityAction::Reset);
    assert_eq!(reset.action().as_str(), "reset");
    assert!(reset.column_ids().is_empty());
    assert_eq!(reset.column_id(), None);
    assert_eq!(reset.next_visible(), None);
    let reset_state = reset.apply_to(state);
    assert!(reset_state.column_visibility().is_empty());
}

#[test]
fn table_global_filter_state_resolves_input_contract() {
    let state: TableGlobalFilterState = TableGlobalFilter::new("global-filter", "Search rows")
        .default_query("stale")
        .query("  done  ")
        .placeholder("Search every row")
        .clear_label("Reset search")
        .small()
        .state();

    assert_eq!(state.id(), "global-filter");
    assert_eq!(state.label(), "Search rows");
    assert_eq!(state.query(), "  done  ");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.placeholder(), "Search every row");
    assert_eq!(state.clear_label(), "Reset search");
    assert_eq!(state.size(), Size::Small);
    assert!(!state.disabled());
    assert_eq!(state.input().value(), "  done  ");
    assert_eq!(state.input().placeholder(), Some("Search every row"));
    assert_eq!(state.input().size(), Size::Small);
    assert!(state.input().controller_driven());

    let empty = TableGlobalFilter::new("empty-global-filter", "Search")
        .default_query("   ")
        .disabled(true)
        .state();
    assert!(!empty.active());
    assert!(empty.clear_enabled());
    assert!(empty.disabled());
    assert!(empty.input().disabled());
}

#[test]
fn table_toolbar_state_resolves_slot_counts_and_summary() {
    let tokens = custom_tokens();
    let state: TableToolbarState = TableToolbar::new("table-toolbar", "Filters")
        .small()
        .tokens(tokens)
        .control(div())
        .controls(vec![div(), div()])
        .secondary_control(div())
        .secondary_controls(vec![div(), div()])
        .summary("3 filtered / 8 total")
        .state();

    assert_eq!(state.id(), "table-toolbar");
    assert_eq!(state.label(), "Filters");
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.primary_control_count(), 3);
    assert_eq!(state.secondary_control_count(), 3);
    assert_eq!(state.control_count(), 6);
    assert!(state.has_controls());
    assert_eq!(state.summary(), Some("3 filtered / 8 total"));
    assert!(state.has_summary());
    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.tokens(), tokens);
    assert_eq!(state.foreground().token(), TEST_TEXT);
    assert_eq!(state.muted_foreground().token(), TEST_TEXT_MUTED);

    let empty = TableToolbar::new("empty-table-toolbar", "Filters").state();
    assert_eq!(empty.primary_control_count(), 0);
    assert_eq!(empty.secondary_control_count(), 0);
    assert_eq!(empty.control_count(), 0);
    assert!(!empty.has_controls());
    assert_eq!(empty.summary(), None);
    assert!(!empty.has_summary());
}

#[test]
fn table_global_filter_change_updates_state_and_resets_pagination() {
    let state = sample_table_state(4)
        .with_filters([TableFilter::contains("team", "UI")])
        .with_sorting([TableSort::ascending("name")])
        .with_selection_mode(TableSelectionMode::Multiple)
        .with_selected_rows(["row-0001"])
        .with_global_filter("old")
        .with_pagination(TablePagination::new(3, 25));

    let change = TableGlobalFilterChange::new("  done  ");
    assert_eq!(change.query(), "  done  ");
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state.clone());
    assert_eq!(next.global_filter(), Some("done"));
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters(), state.filters());
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.selected_rows(), state.selected_rows());

    let cleared = TableGlobalFilterChange::clear();
    assert_eq!(cleared.query(), "");
    assert!(cleared.cleared());
    assert!(!cleared.active());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.global_filter(), None);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters(), state.filters());
    assert_eq!(cleared_state.sorting(), state.sorting());
    assert_eq!(cleared_state.selected_rows(), state.selected_rows());
}

#[test]
fn table_predicate_filter_state_resolves_operator_and_input_contract() {
    let starts_with = TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith);
    let state: TablePredicateFilterState =
        TablePredicateFilter::new("name-predicate", "Name", "name")
            .default_operator(TablePredicateFilterOperator::text(
                TableTextFilterOperator::Contains,
            ))
            .operator(starts_with)
            .default_value("stale")
            .value("  Al  ")
            .operators([
                TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith),
                TablePredicateFilterOperator::text(TableTextFilterOperator::EndsWith),
                TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThan),
            ])
            .placeholder("Filter name")
            .clear_label("Reset name")
            .small()
            .state();

    assert_eq!(state.id(), "name-predicate");
    assert_eq!(state.label(), "Name");
    assert_eq!(state.column_id().as_str(), "name");
    assert_eq!(state.operator(), starts_with);
    assert_eq!(
        state.operator().text_operator(),
        Some(TableTextFilterOperator::StartsWith)
    );
    assert_eq!(state.value(), "  Al  ");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.placeholder(), "Filter name");
    assert_eq!(state.clear_label(), "Reset name");
    assert_eq!(state.size(), Size::Small);
    assert!(!state.disabled());
    assert_eq!(state.input().value(), "  Al  ");
    assert_eq!(state.input().placeholder(), Some("Filter name"));
    assert!(state.input().controller_driven());
    assert_eq!(state.select().selected_value(), Some("text:starts_with"));
    assert_eq!(state.select().trigger_label(), "Starts with");
    let first_option: &TablePredicateFilterOperatorOptionState = state
        .operator_options()
        .first()
        .expect("predicate filter should expose operator options");
    assert_eq!(first_option.operator(), starts_with);

    let options = state
        .operator_options()
        .iter()
        .map(|option| {
            (
                option.value().to_owned(),
                option.label().to_owned(),
                option.selected(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        options,
        vec![
            (
                "text:starts_with".to_string(),
                "Starts with".to_string(),
                true,
            ),
            ("text:ends_with".to_string(), "Ends with".to_string(), false),
            (
                "number:greater_than".to_string(),
                "Greater than".to_string(),
                false,
            ),
        ]
    );

    let invalid_number = TablePredicateFilter::new("score-predicate", "Score", "score")
        .operator(TablePredicateFilterOperator::number(
            TableNumericFilterOperator::GreaterThan,
        ))
        .value("not a number")
        .state();
    assert!(!invalid_number.active());
    assert!(invalid_number.clear_enabled());
    assert_eq!(
        TablePredicateFilterOperator::from_str("number:less_than")
            .expect("stable numeric operator should parse")
            .numeric_operator(),
        Some(TableNumericFilterOperator::LessThan)
    );
}

#[test]
fn table_predicate_filter_change_updates_only_target_predicate_filters() {
    let score_range = TableFilter::number_range("score", Some(0.0), Some(100.0))
        .expect("finite score range should be valid");
    let score_comparison = TableFilter::number_greater_than("score", 5.0)
        .expect("finite score comparison should be valid");
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("status", "Blocked")
            .with_cell("score", 50_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::contains("score", "1"),
        score_comparison,
        score_range.clone(),
        TableFilter::one_of("score", ["10", "50"]),
        TableFilter::one_of("status", ["Ready"]),
    ])
    .with_sorting([TableSort::descending("score")])
    .with_selection_mode(TableSelectionMode::Multiple)
    .with_selected_rows(["row-a"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["team"]))
    .with_row_pinning(TableRowPinning::new().pinned_top(["row-a"]))
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(180.0)))
    .with_column_visibility(TableColumnVisibilityOverrides::new().hide("status"))
    .with_global_filter("ready")
    .with_pagination(TablePagination::new(3, 25));

    let change = TablePredicateFilterChange::new(
        "score",
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThanOrEqual),
        " 42 ",
    );
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(
        change.operator(),
        Some(TablePredicateFilterOperator::number(
            TableNumericFilterOperator::LessThanOrEqual
        ))
    );
    assert_eq!(change.value(), " 42 ");
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state.clone());
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.selected_rows(), state.selected_rows());
    assert_eq!(next.column_pinning(), state.column_pinning());
    assert_eq!(next.row_pinning(), state.row_pinning());
    assert_eq!(next.column_sizing(), state.column_sizing());
    assert_eq!(next.column_visibility(), state.column_visibility());
    assert_eq!(next.global_filter(), state.global_filter());
    assert_eq!(next.filters().len(), 5);
    assert!(
        next.filters()
            .iter()
            .any(|filter| filter.column().as_str() == "team" && filter.query() == "UI")
    );
    assert!(
        next.filters()
            .iter()
            .any(|filter| filter.number_range_bounds() == score_range.number_range_bounds())
    );
    assert!(next.filters().iter().any(|filter| {
        filter.column().as_str() == "score"
            && filter
                .selected_values()
                .is_some_and(|values| values.contains("10") && values.contains("50"))
    }));
    let score_predicate = next
        .filters()
        .iter()
        .find(|filter| filter.number_comparison_value().is_some())
        .expect("score numeric comparison should be replaced");
    assert_eq!(score_predicate.column().as_str(), "score");
    assert_eq!(
        score_predicate.number_comparison_value(),
        Some((TableNumericFilterOperator::LessThanOrEqual, 42.0))
    );
    assert!(
        next.filters().iter().all(|filter| {
            filter.column().as_str() != "score" || filter.text_predicate().is_none()
        }),
        "same-column legacy/text predicate should be removed"
    );

    let cleared = TablePredicateFilterChange::clear("score");
    assert!(cleared.cleared());
    assert!(!cleared.active());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 4);
    assert!(
        cleared_state
            .filters()
            .iter()
            .all(|filter| filter.number_comparison_value().is_none())
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.number_range_bounds() == score_range.number_range_bounds())
    );
    assert!(cleared_state.filters().iter().any(|filter| {
        filter.column().as_str() == "score"
            && filter
                .selected_values()
                .is_some_and(|values| values.contains("10") && values.contains("50"))
    }));
}

#[test]
fn table_render_plan_exposes_text_cell_editability_for_leaf_cells_only() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("notes", "Line 1\nLine 2")
            .with_cell("score", 10_usize),
        TableRow::new("row-b").with_cell("score", 20_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("notes", "Notes").with_multiline_text_editor(3),
        TableColumn::new("score", "Score"),
    ])
    .with_grouping(["score"])
    .with_all_rows_expanded()
    .with_pagination(TablePagination::disabled());
    let plan = Table::new("editable-plan-table", "Editable plan table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(120.0))
        .render_plan(UiPx::ZERO, ui_px(120.0));

    let name_column = plan
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .expect("name column should resolve");
    let score_column = plan
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should resolve");
    let notes_column = plan
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "notes")
        .expect("notes column should resolve");
    assert!(name_column.text_editable());
    assert_eq!(name_column.editor(), Some(TableCellEditor::Text));
    assert!(notes_column.text_editable());
    assert_eq!(
        notes_column.editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert!(!score_column.text_editable());
    assert_eq!(score_column.editor(), None);

    let group_row = plan
        .rows()
        .iter()
        .find(|row| row.row().is_group())
        .expect("group row should resolve");
    let group_name_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("group name cell should resolve");
    assert!(
        !group_name_cell.text_editable(),
        "synthetic grouped rows must stay display-only"
    );
    let group_notes_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "notes")
        .expect("group notes cell should resolve");
    assert_eq!(group_notes_cell.editor(), None);

    let editable_leaf = plan
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "row-a")
        .expect("row-a should resolve");
    let editable_name = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("row-a name cell should resolve");
    assert!(editable_name.text_editable());
    let editable_notes = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "notes")
        .expect("row-a notes cell should resolve");
    assert_eq!(
        editable_notes.editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );

    let missing_leaf = plan
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "row-b")
        .expect("row-b should resolve");
    let missing_name = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("row-b missing name cell should resolve");
    assert!(!missing_name.text_editable());
}

#[test]
fn table_cell_edit_change_updates_source_row_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("root")
            .with_cell("name", "Root")
            .with_cell("team", "Platform")
            .with_child(
                TableRow::new("child")
                    .with_cell("name", "Child")
                    .with_cell("team", "UI"),
            ),
        TableRow::new("other")
            .with_cell("name", "Other")
            .with_cell("team", "Ops"),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("team", "Team"),
    ])
    .with_column_order(["team", "name"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_filters([TableFilter::contains("team", "UI")])
    .with_sorting([TableSort::ascending("name")])
    .with_expanded_rows(["root"])
    .with_selected_rows(["child"])
    .with_pagination(TablePagination::new(2, 25));

    let change = TableCellEditChange::for_row("child", "name", "Child", "Child Prime");

    let (next, outcome) = change.apply_to(state.clone());
    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
    assert_eq!(next.column_order()[0].as_str(), "team");
    assert_eq!(next.column_pinning().left()[0].as_str(), "name");
    assert_eq!(next.filters()[0].query(), "UI");
    assert_eq!(next.sorting()[0].column().as_str(), "name");
    assert_eq!(next.expansion(), state.expansion());
    assert!(next.selected_rows().contains(&TableRowId::new("child")));
    assert_eq!(next.pagination().page_index(), 2);

    let updated = next
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "root")
        .and_then(|row| row.children().first())
        .expect("nested child should remain nested");
    assert_eq!(
        updated
            .cell(&TableColumnId::new("name"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Child Prime")
    );

    let missing_column = TableCellEditChange::for_row("child", "missing", "old", "new");
    let (missing_column_state, missing_outcome) = missing_column.apply_to(next.clone());
    assert_eq!(missing_outcome, TableCellEditApplyOutcome::CellNotFound);
    assert_eq!(missing_column_state, next);
    assert_eq!(
        missing_column_state.cache_key().rows_identity(),
        next.cache_key().rows_identity(),
        "missing cell edits should be inspectable no-ops"
    );

    let missing_row = TableCellEditChange::for_row("missing-row", "name", "old", "new");
    let (missing_row_state, missing_row_outcome) = missing_row.apply_to(next.clone());
    assert_eq!(missing_row_outcome, TableCellEditApplyOutcome::RowNotFound);
    assert_eq!(missing_row_state, next);
    assert_eq!(
        missing_row_state.cache_key().rows_identity(),
        next.cache_key().rows_identity(),
        "missing row edits should be inspectable no-ops"
    );
}

#[test]
fn table_render_plan_exposes_pinned_column_regions() {
    let flat_plan = Table::new("flat-table", "Flat table", sample_table_state(1))
        .render_plan(UiPx::ZERO, ui_px(96.0));
    assert!(!flat_plan.uses_split_pinned_layout());
    assert!(
        flat_plan.pinned_layout().is_none(),
        "unpinned tables should keep the flat render topology"
    );

    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("score", 42_usize)
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
        TableColumn::new("status", "Status"),
    ])
    .with_column_order(["status", "score", "team", "name"])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name", "score"])
            .pinned_right(["status"]),
    )
    .with_pagination(TablePagination::disabled());
    let plan = Table::new("pinned-table", "Pinned table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));
    let layout = plan
        .pinned_layout()
        .expect("pinned columns should request split pinned layout metadata");
    assert!(plan.uses_split_pinned_layout());
    assert_eq!(layout.table_id(), "pinned-table");
    assert_eq!(layout.left_width(), ui_px(256.0));
    assert_eq!(layout.center_width(), ui_px(128.0));
    assert_eq!(layout.right_width(), ui_px(128.0));
    assert_eq!(layout.total_width(), ui_px(512.0));
    assert_eq!(
        layout.header_center_scroll_id(),
        "table:pinned-table:header-center-scroll"
    );
    assert_eq!(
        layout.header_center_scroll_selector(),
        "scroll-area:table:pinned-table:header-center-scroll"
    );
    assert_eq!(
        layout.header_region_selector(TableColumnRegion::Left),
        "table:pinned-table:header-region:left"
    );

    let region_columns = plan
        .column_regions()
        .iter()
        .map(|region| {
            (
                region.region(),
                region
                    .columns()
                    .iter()
                    .map(|column| column.id().as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        region_columns,
        [
            (TableColumnRegion::Left, vec!["score", "name"]),
            (TableColumnRegion::Center, vec!["team"]),
            (TableColumnRegion::Right, vec!["status"]),
        ]
    );
    assert_eq!(
        plan.columns()
            .iter()
            .map(|column| (column.id().as_str(), column.region()))
            .collect::<Vec<_>>(),
        [
            ("score", TableColumnRegion::Left),
            ("name", TableColumnRegion::Left),
            ("team", TableColumnRegion::Center),
            ("status", TableColumnRegion::Right),
        ]
    );

    let row = &plan.rows()[0];
    assert_eq!(
        layout.row_center_scroll_id(row.render_key()),
        "table:pinned-table:row-center-scroll:row-a"
    );
    assert_eq!(
        layout.row_center_scroll_selector(row.render_key()),
        "scroll-area:table:pinned-table:row-center-scroll:row-a"
    );
    assert_eq!(
        layout.row_region_selector(row.render_key(), TableColumnRegion::Right),
        "table:pinned-table:row-region:row-a:right"
    );
    assert_eq!(
        row.cells_for_region(TableColumnRegion::Left)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["score", "name"]
    );
    assert_eq!(
        row.cells_for_region(TableColumnRegion::Center)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["team"]
    );
    assert_eq!(
        row.cells_for_region(TableColumnRegion::Right)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["status"]
    );
}

#[test]
fn table_render_plan_exposes_row_pinning_regions() {
    let state = sample_table_state(12)
        .with_pagination(TablePagination::new(1, 4))
        .with_row_pinning(
            TableRowPinning::new()
                .pinned_top(["row-0001"])
                .pinned_bottom(["row-0005", "row-0010"]),
        );
    let plan = Table::new("row-pinning-table", "Row pinning table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .overscan(0)
        .render_plan(UiPx::ZERO, ui_px(96.0));

    assert_eq!(
        plan.top_rows()
            .iter()
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [("row-0001", TableRowRegion::Top, 0)]
    );
    assert_eq!(
        plan.center_rows()
            .iter()
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-0004", TableRowRegion::Center, 0),
            ("row-0006", TableRowRegion::Center, 1),
            ("row-0007", TableRowRegion::Center, 2),
        ],
        "the center region should be the current page with pinned duplicates removed"
    );
    assert_eq!(
        plan.bottom_rows()
            .iter()
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-0005", TableRowRegion::Bottom, 0),
            ("row-0010", TableRowRegion::Bottom, 1),
        ]
    );
    assert_eq!(
        plan.rendered_rows()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "row-0001", "row-0004", "row-0006", "row-0007", "row-0005", "row-0010",
        ]
    );
    assert_eq!(
        plan.table()
            .final_model()
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "row-0001", "row-0004", "row-0006", "row-0007", "row-0005", "row-0010",
        ],
        "final visual rows should match the top + center + bottom render order"
    );
    assert_eq!(plan.virtualizer().count(), 3);
    assert_eq!(plan.rendered_row_count(), 6);
    assert_eq!(plan.visible_row_count(), 6);
    assert_eq!(plan.aria_row_count(), 7);
}

#[test]
fn table_render_plan_respects_page_only_row_pinning_policy() {
    let state = sample_table_state(12)
        .with_pagination(TablePagination::new(1, 4))
        .with_row_pinning(
            TableRowPinning::new()
                .pinned_top(["row-0001"])
                .pinned_bottom(["row-0005", "row-0010"]),
        )
        .with_row_pinning_policy(TableRowPinningPolicy::PageOnly);
    let plan = Table::new(
        "row-pinning-page-only-table",
        "Row pinning page-only table",
        state,
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .render_plan(UiPx::ZERO, ui_px(96.0));

    assert!(plan.top_rows().is_empty());
    assert_eq!(
        plan.center_rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-0004", "row-0006", "row-0007"],
        "outside-page pinned rows should be omitted under page-only policy"
    );
    assert_eq!(
        plan.bottom_rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-0005"]
    );
    assert_eq!(plan.virtualizer().count(), 3);
    assert_eq!(plan.aria_row_count(), 5);
}

#[test]
fn table_render_plan_exposes_center_column_window_metadata() {
    let plan = Table::new(
        "center-window-table",
        "Center window table",
        sample_center_window_table_state(),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(4)
    .render_plan(UiPx::ZERO, ui_px(96.0));
    let window = plan
        .center_column_window()
        .expect("center columns should resolve window metadata");

    assert_eq!(window.center_width(), ui_px(540.0));
    assert!(!window.virtualized());
    assert_eq!(*window.visible_range(), VirtualizerRange::new(0, 6));
    assert_eq!(*window.overscan_range(), VirtualizerRange::new(0, 6));
    assert_eq!(window.leading_spacer_width(), UiPx::ZERO);
    assert_eq!(window.trailing_spacer_width(), UiPx::ZERO);
    assert_eq!(window.rendered_column_count(), 6);
    assert_eq!(
        window
            .rendered_columns()
            .iter()
            .map(|column| column.id().as_str())
            .collect::<Vec<_>>(),
        [
            "metric_00",
            "metric_01",
            "metric_02",
            "metric_03",
            "metric_04",
            "metric_05",
        ]
    );
    assert!(
        window
            .rendered_columns()
            .iter()
            .all(|column| column.region() == TableColumnRegion::Center),
        "pinned left/right columns must stay outside the center window"
    );
}

#[test]
fn table_center_column_window_matches_exact_size_virtualizer() {
    let plan = Table::new(
        "wide-center-window-table",
        "Wide center window table",
        sample_center_window_table_state(),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(4)
    .render_plan(UiPx::ZERO, ui_px(96.0));
    let center_columns = plan
        .column_regions()
        .iter()
        .find(|region| region.region() == TableColumnRegion::Center)
        .expect("center region should resolve")
        .columns();
    let window =
        TableCenterColumnWindowPlan::resolve(center_columns, ui_px(170.0), ui_px(120.0), 2)
            .expect("center column window should resolve");
    let expected = VirtualizerState::new(center_columns.len(), center_columns[0].width())
        .with_viewport_extent(ui_px(120.0))
        .with_scroll_offset(ui_px(170.0))
        .with_overscan(2)
        .resolve_known_size_window(|index| {
            let column = &center_columns[index];
            (
                VirtualizerItemKey::new(column.id().as_str().to_owned()),
                column.width(),
            )
        });

    assert!(window.virtualized());
    assert!(window.rendered_column_count() < center_columns.len());
    assert_eq!(window.center_width(), expected.total_size());
    assert_eq!(window.visible_range(), expected.visible_range());
    assert_eq!(window.overscan_range(), expected.overscan_range());
    assert_eq!(window.rendered_column_count(), expected.items().len());
    assert_eq!(
        window.leading_spacer_width(),
        expected
            .items()
            .first()
            .map(|item| item.start())
            .unwrap_or(UiPx::ZERO)
    );
    assert_eq!(
        window.trailing_spacer_width(),
        expected
            .items()
            .last()
            .map(|item| expected.total_size() - item.end())
            .unwrap_or(UiPx::ZERO)
    );
    assert_eq!(
        window
            .rendered_columns()
            .iter()
            .map(|column| column.id().as_str())
            .collect::<Vec<_>>(),
        expected
            .items()
            .iter()
            .map(|item| item.key().as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.column_region_width(TableColumnRegion::Left),
        ui_px(140.0)
    );
    assert_eq!(
        plan.column_region_width(TableColumnRegion::Right),
        ui_px(132.0)
    );
}

#[test]
fn table_center_column_window_preserves_full_accessibility_indexes() {
    let plan = Table::new(
        "accessibility-center-window-table",
        "Accessibility center window table",
        sample_center_window_table_state(),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .render_plan(UiPx::ZERO, ui_px(96.0));
    let center_columns = plan
        .column_regions()
        .iter()
        .find(|region| region.region() == TableColumnRegion::Center)
        .expect("center region should resolve")
        .columns();
    let window = TableCenterColumnWindowPlan::resolve(center_columns, ui_px(420.0), ui_px(68.0), 0)
        .expect("center column window should resolve");

    assert_eq!(
        window
            .rendered_columns()
            .iter()
            .map(|column| (column.id().as_str(), column.aria_column_index()))
            .collect::<Vec<_>>(),
        [("metric_05", 7)]
    );
}

#[test]
fn table_virtualizer_snapshot_restores_measurements_without_overriding_live_scroll() {
    let snapshot = VirtualizerSnapshot::new(
        ui_px(0.0),
        [VirtualizerSnapshotItem::new(
            VirtualizerItemKey::new("row-0005"),
            ui_px(48.0),
        )],
    );
    let table = Table::new("snapshot-table", "Snapshot table", sample_table_state(30))
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .virtualizer_snapshot(snapshot);
    let plan = table.render_plan(ui_px(120.0), ui_px(96.0));

    assert_eq!(plan.virtualizer().scroll_offset(), ui_px(120.0));
    let measured_row = plan
        .virtualizer()
        .measurements()
        .iter()
        .find(|measurement| measurement.key().as_str() == "row-0005")
        .expect("snapshot measurement should be restored by stable row key");
    assert_eq!(measured_row.size(), ui_px(48.0));
    assert!(measured_row.measured());
}

#[test]
fn table_render_plan_disambiguates_duplicate_row_ids_for_rendering() {
    let state = TableState::new([
        TableRow::new("duplicate").with_cell("name", "First"),
        TableRow::new("duplicate").with_cell("name", "Second"),
        TableRow::new("unique").with_cell("name", "Third"),
    ])
    .with_columns([TableColumn::new("name", "Name")])
    .with_pagination(TablePagination::disabled());
    let table = Table::new("duplicate-table", "Duplicate rows", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(120.0));
    let plan = table.render_plan(UiPx::ZERO, ui_px(120.0));

    assert_eq!(plan.table().duplicate_row_ids()[0].as_str(), "duplicate");
    assert_eq!(plan.rows()[0].id().as_str(), "duplicate");
    assert_eq!(plan.rows()[0].render_key(), "0:duplicate");
    assert_eq!(plan.rows()[1].id().as_str(), "duplicate");
    assert_eq!(plan.rows()[1].render_key(), "1:duplicate");
    assert_eq!(plan.rows()[2].id().as_str(), "unique");
    assert_eq!(plan.rows()[2].render_key(), "unique");

    let keys = plan
        .virtualizer()
        .measurements()
        .iter()
        .map(|measurement| measurement.key().as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, ["0:duplicate", "1:duplicate", "unique"]);
}

#[test]
fn table_render_plan_exposes_column_sizing_metadata_and_matching_cell_widths() {
    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("score", 42_usize)
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name").with_width(ui_px(100.0)),
        TableColumn::new("team", "Team").with_width(ui_px(120.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(80.0))
            .with_min_width(ui_px(70.0))
            .with_max_width(ui_px(90.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(60.0))
            .with_resizable(false),
    ])
    .with_column_order(["status", "score", "team", "name"])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name", "score"])
            .pinned_right(["status"]),
    )
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(95.0)))
    .with_pagination(TablePagination::disabled());
    let plan = Table::new("sized-table", "Sized table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));

    assert_eq!(plan.total_column_width(), ui_px(370.0));
    assert_eq!(
        plan.column_region_width(TableColumnRegion::Left),
        ui_px(190.0)
    );
    assert_eq!(
        plan.column_region_width(TableColumnRegion::Center),
        ui_px(120.0)
    );
    assert_eq!(
        plan.column_region_width(TableColumnRegion::Right),
        ui_px(60.0)
    );
    assert_eq!(plan.column_regions()[0].total_width(), ui_px(190.0));
    assert_eq!(plan.column_regions()[1].total_width(), ui_px(120.0));
    assert_eq!(plan.column_regions()[2].total_width(), ui_px(60.0));

    let score_column = plan
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should be present");
    assert_eq!(score_column.width(), ui_px(90.0));
    assert_eq!(score_column.min_width(), ui_px(70.0));
    assert_eq!(score_column.max_width(), ui_px(90.0));
    assert_eq!(score_column.start(), ui_px(0.0));
    assert_eq!(score_column.after(), ui_px(100.0));
    assert!(score_column.resizable());

    let score_cell = plan.rows()[0]
        .cells_for_region(TableColumnRegion::Left)
        .find(|cell| cell.column_id().as_str() == "score")
        .expect("score cell should be present");
    assert_eq!(score_cell.width(), score_column.width());
}

#[test]
fn table_render_plan_preserves_column_width_policies() {
    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("status", "Status").with_content_fit(),
    ])
    .with_pagination(TablePagination::disabled());
    let plan = Table::new("policy-table", "Policy table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .render_plan(UiPx::ZERO, ui_px(96.0));
    let status_column = plan
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "status")
        .expect("status column should be present");

    assert_eq!(
        status_column.width_policy(),
        open_gpui_ui_components::TableColumnWidthPolicy::ContentFit
    );
}

#[test]
fn table_render_plan_exposes_nested_header_groups_and_region_widths() {
    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("score", 42_usize)
        .with_cell("status", "Ready")])
    .with_column_tree([
        TableColumnGroup::new(
            "identity",
            "Identity",
            [
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
            ],
        ),
        TableColumnGroup::new(
            "metrics",
            "Metrics",
            [TableColumnGroup::new(
                "scores",
                "Scores",
                [
                    TableColumn::new("score", "Score").with_width(ui_px(80.0)),
                    TableColumn::new("status", "Status").with_width(ui_px(90.0)),
                ],
            )],
        ),
    ])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name"])
            .pinned_right(["status"]),
    )
    .with_pagination(TablePagination::disabled());
    let plan = Table::new("nested-headers", "Nested headers", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(240.0))
        .render_plan(UiPx::ZERO, ui_px(240.0));

    assert_eq!(plan.header_row_count(), 3);
    assert_eq!(plan.left_header_groups().header_row_count(), 2);
    assert_eq!(plan.center_header_groups().header_row_count(), 3);
    assert_eq!(plan.right_header_groups().header_row_count(), 3);
    assert_eq!(plan.left_header_groups().total_width(), ui_px(100.0));
    assert_eq!(
        plan.center_header_groups().total_width(),
        ui_px(120.0 + 80.0)
    );
    assert_eq!(plan.right_header_groups().total_width(), ui_px(90.0));
    assert_eq!(
        plan.center_header_groups().groups()[0].headers()[0].label(),
        "Identity"
    );
    assert_eq!(
        plan.center_header_groups().groups()[1].headers()[0].label(),
        "Team"
    );
    assert_eq!(
        plan.center_header_groups().groups()[2].headers()[0].kind(),
        TableResolvedHeaderKind::Placeholder
    );
}

#[test]
fn virtualized_list_render_plan_uses_item_descriptors_and_virtualizer_contracts() {
    let items = (0..10_000)
        .map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("item-{index:04}"),
                format!("Item {index:04}"),
            )
        })
        .collect::<Vec<_>>();
    let state = VirtualizedListState::resolve(
        Size::Small,
        false,
        items.len(),
        Some(104),
        Some(101),
        Some(7),
    );
    let plan = VirtualizedListRenderPlan::resolve(
        "contracts-list",
        "Contracts list",
        state,
        &items,
        ui_px(2_800.0),
        ui_px(196.0),
    );

    assert_eq!(plan.role(), Role::ListBox);
    assert_eq!(plan.row_role(), Role::ListBoxOption);
    assert_eq!(plan.virtualizer().count(), 10_000);
    assert_eq!(plan.virtualizer().total_size(), ui_px(280_000.0));
    assert_eq!(
        *plan.virtualizer().visible_range(),
        VirtualizerRange::new(100, 107)
    );
    assert_eq!(
        *plan.virtualizer().overscan_range(),
        VirtualizerRange::new(98, 109)
    );
    assert_eq!(plan.visible_row_count(), 7);
    assert_eq!(plan.rendered_row_count(), 11);
    assert_eq!(plan.rows()[0].index(), 98);
    assert_eq!(plan.rows()[0].render_key(), "item-0098");

    let active_row: &VirtualizedListRowRenderPlan =
        plan.active_row().expect("active row should be rendered");
    assert_eq!(active_row.index(), 104);
    assert_eq!(active_row.key(), "item-0104");
    assert_eq!(active_row.label(), "Item 0104");
    assert!(active_row.active());
    assert!(!active_row.selected());
    assert_eq!(active_row.role(), Role::ListBoxOption);
    assert_eq!(active_row.position_in_set(), 105);
    assert_eq!(active_row.size_of_set(), 10_000);
    assert_eq!(active_row.virtual_start(), ui_px(2_912.0));
    assert_eq!(active_row.virtual_size(), ui_px(28.0));

    let selected_row = plan
        .selected_row()
        .expect("selected row should be rendered");
    assert_eq!(selected_row.index(), 101);
    assert!(selected_row.selected());

    let activation = VirtualizedListActivation::new(active_row.index());
    assert_eq!(activation.index(), 104);
    assert_eq!(
        virtualized_list_scroll_target(
            VirtualizedListScrollStrategy::Top,
            activation.index(),
            plan.state().item_count(),
            plan.metrics().row_height(),
            plan.virtualizer().viewport_extent(),
            plan.virtualizer().scroll_offset(),
        ),
        ui_px(2_912.0)
    );
}

#[test]
fn virtualized_list_component_render_plan_applies_builder_metrics() {
    let items = (0..32)
        .map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("item-{index:04}"),
                format!("Item {index:04}"),
            )
        })
        .collect::<Vec<_>>();
    let plan = VirtualizedList::new("builder-list", "Builder list", items)
        .with_size(Size::Small)
        .row_height(ui_px(24.0))
        .overscan(2)
        .default_active_index(5)
        .default_selected_index(3)
        .viewport_item_count(4)
        .render_plan(ui_px(48.0), ui_px(96.0));

    assert_eq!(plan.metrics().row_height(), ui_px(24.0));
    assert_eq!(plan.overscan_count(), 2);
    assert_eq!(plan.visible_row_count(), 4);
    assert_eq!(
        *plan.virtualizer().visible_range(),
        VirtualizerRange::new(2, 6)
    );
    assert_eq!(
        *plan.virtualizer().overscan_range(),
        VirtualizerRange::new(1, 7)
    );
    assert_eq!(plan.active_row().map(|row| row.index()), Some(5));
    assert_eq!(plan.selected_row().map(|row| row.index()), Some(3));
}

#[open_gpui::test]
fn tree_runtime_expands_reveals_and_selects_items(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
        toggles: Rc<RefCell<Vec<(String, bool)>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let toggles = self.toggles.clone();
            let tree = Tree::new(
                "runtime-tree",
                "Runtime tree",
                vec![
                    TreeItemDescriptor::new("paper", "Paper")
                        .child(TreeItemDescriptor::new("intro", "Introduction"))
                        .child(
                            TreeItemDescriptor::new("figures", "Figures")
                                .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
                        ),
                    TreeItemDescriptor::new("notes", "Notes"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("paper")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_toggle(move |toggle, _, _| {
                toggles
                    .borrow_mut()
                    .push((toggle.value().to_owned(), toggle.expanded()));
            });

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(180.0)).child(tree))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("tree:runtime-tree:item:paper").is_some(),
        "expected the root tree item to render before expansion"
    );
    assert!(
        cx.debug_bounds("tree:runtime-tree:item:intro").is_none(),
        "expected collapsed descendants to stay hidden before expansion"
    );

    let root = cx
        .debug_bounds("tree:runtime-tree:root")
        .expect("tree root should render as a focusable interaction region");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Default::default(),
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("tree:runtime-tree:item:paper"),
        "clicking Tree chrome should focus the current roving item for keyboard navigation"
    );

    let paper = cx
        .debug_bounds("tree:runtime-tree:item:paper")
        .expect("paper row should be visible");
    cx.simulate_click(paper.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    selections.borrow_mut().clear();

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        toggles.borrow().as_slice(),
        [("paper".to_owned(), true)],
        "expected right arrow to expand the focused root branch"
    );
    assert!(
        cx.debug_bounds("tree:runtime-tree:item:intro").is_some(),
        "expected expanded descendants to render after toggling open"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(selections.borrow().as_slice(), ["intro".to_owned()]);
}

#[open_gpui::test]
fn tree_runtime_typeahead_focuses_visible_matching_row(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let tree = Tree::new(
                "runtime-typeahead-tree",
                "Runtime typeahead tree",
                vec![
                    TreeItemDescriptor::new("paper", "Paper")
                        .child(TreeItemDescriptor::new("figures", "Figures")),
                    TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
                    TreeItemDescriptor::new("notes", "Notes"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("paper")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            });

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(180.0)).child(tree))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("tree:runtime-typeahead-tree:root")
        .expect("tree root should render");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Default::default(),
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("n o");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_selector_is_focused("tree:runtime-typeahead-tree:item:notes"),
        "expected typeahead to focus the visible Notes row; focused={:?}",
        cx.focused_debug_selector()
    );
    assert!(
        selections.borrow().is_empty(),
        "typeahead should move focus without selecting a row"
    );
}

#[open_gpui::test]
fn tree_runtime_drag_move_emits_controlled_payload(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        moves: Rc<RefCell<Vec<TreeMove>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let moves = self.moves.clone();
            let selections = self.selections.clone();
            let tree = Tree::new(
                "runtime-drag-tree",
                "Runtime drag tree",
                vec![
                    TreeItemDescriptor::new("root", "Root")
                        .expanded(true)
                        .child(TreeItemDescriptor::new("child", "Child"))
                        .child(TreeItemDescriptor::new("peer", "Peer")),
                    TreeItemDescriptor::new("sibling", "Sibling"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("root")
            .draggable(true)
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_move(move |tree_move, _, _| {
                moves.borrow_mut().push(tree_move);
            });

            div()
                .size_full()
                .child(div().w(px(320.0)).h(px(220.0)).child(tree))
        }
    }

    let moves = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        moves: moves.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let child = cx
        .debug_bounds("tree:runtime-drag-tree:item:child")
        .expect("expanded child row should render");
    cx.simulate_click(child.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().as_slice(),
        ["child".to_owned()],
        "enabling tree drag affordances should not break regular row clicks"
    );
    assert!(
        moves.borrow().is_empty(),
        "regular clicks should not emit controlled tree moves"
    );
    selections.borrow_mut().clear();

    let source = cx
        .debug_bounds("tree:runtime-drag-tree:item:child")
        .expect("child row should remain rendered")
        .center();
    let target = cx
        .debug_bounds("tree:runtime-drag-tree:drop:before:sibling")
        .expect("before-sibling drop zone should render")
        .center();

    cx.simulate_mouse_down(source, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(source.x + px(18.0), source.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let moves = moves.borrow();
    assert_eq!(
        moves.len(),
        1,
        "expected one controlled move after dropping child before sibling"
    );
    let tree_move = &moves[0];
    assert_eq!(tree_move.value(), "child");
    assert_eq!(tree_move.label(), "Child");
    assert_eq!(tree_move.source_parent_value(), Some("root"));
    assert_eq!(tree_move.position(), TreeDropPosition::Before);
    assert_eq!(tree_move.target().target_value(), "sibling");
    assert_eq!(tree_move.target_parent_value(), None);
    assert_eq!(tree_move.sibling_anchor_value(), Some("sibling"));
    assert!(
        selections.borrow().is_empty(),
        "drag drops should not also emit row selections"
    );
}

#[test]
fn table_header_action_cycles_sorting_without_render_coupling() {
    let unsorted = Table::new("sort-cycle", "Sort cycle", sample_table_state(8))
        .render_plan(UiPx::ZERO, ui_px(120.0));
    let name_action = unsorted
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .and_then(|column| column.sort_action())
        .expect("sortable column should expose a header action");
    assert_eq!(name_action.current_direction(), None);
    assert_eq!(
        name_action.next_direction(),
        Some(TableSortDirection::Ascending)
    );

    let ascending_state = name_action.apply_to(sample_table_state(8));
    assert_eq!(ascending_state.sorting()[0].column().as_str(), "name");
    assert_eq!(
        ascending_state.sorting()[0].direction(),
        TableSortDirection::Ascending
    );

    let ascending = Table::new("sort-cycle", "Sort cycle", ascending_state)
        .render_plan(UiPx::ZERO, ui_px(120.0));
    let descending_action = ascending
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .and_then(|column| column.sort_action())
        .expect("ascending column should expose a descending action");
    assert_eq!(
        descending_action.current_direction(),
        Some(TableSortDirection::Ascending)
    );
    assert_eq!(
        descending_action.next_direction(),
        Some(TableSortDirection::Descending)
    );

    let descending_state = descending_action.apply_to(sample_table_state(8));
    let descending = Table::new("sort-cycle", "Sort cycle", descending_state)
        .render_plan(UiPx::ZERO, ui_px(120.0));
    let clear_action = descending
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .and_then(|column| column.sort_action())
        .expect("descending column should expose a clear action");
    assert_eq!(
        clear_action.current_direction(),
        Some(TableSortDirection::Descending)
    );
    assert_eq!(clear_action.next_direction(), None);
    assert!(
        clear_action
            .apply_to(sample_table_state(8))
            .sorting()
            .is_empty()
    );
}

#[test]
fn table_public_exports_include_core_table_and_virtualizer_contracts() {
    use open_gpui_ui_components::{self as root, prelude};

    let state: root::TableState =
        root::TableState::new([root::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_columns([root::TableColumn::new("name", "Name")]);
    let table: root::Table = root::Table::new("root-table", "Root table", state.clone());
    let _prelude_state: prelude::TableState = state;
    let _prelude_table: prelude::Table = prelude::Table::new(
        "prelude-table",
        "Prelude table",
        root::TableState::new([root::TableRow::new("row-b").with_cell("name", "Beta")])
            .with_columns([root::TableColumn::new("name", "Name")]),
    );
    let virtualizer: root::VirtualizerState =
        root::VirtualizerState::new(4, ui_px(24.0)).with_overscan(2);
    let root_plan: root::TableRenderPlan = table.state();
    let _root_region_plan: &root::TableColumnRegionRenderPlan = &root_plan.column_regions()[0];
    let _root_header_groups: &root::TableResolvedHeaderGroupRegions =
        root_plan.table().header_groups();
    let _root_header_kind: root::TableResolvedHeaderKind =
        root_plan.table().center_header_groups()[0].headers()[0].kind();
    let _root_header_cell: &root::TableResolvedHeaderCell =
        &root_plan.table().center_header_groups()[0].headers()[0];
    let _root_header_group: &root::TableResolvedHeaderGroup =
        &root_plan.table().center_header_groups()[0];
    let _root_header_plan: &root::TableHeaderGroupRegionsRenderPlan = root_plan.header_groups();
    let root_group_id = root::TableColumnGroupId::new("identity");
    assert_eq!(root_group_id.as_str(), "identity");
    let root_column_group = root::TableColumnGroup::new(
        root_group_id.clone(),
        "Identity",
        [root::TableColumn::new("name", "Name")],
    )
    .with_child(root::TableColumn::new("team", "Team"));
    let root_column_tree_state =
        root::TableState::new([root::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_column_tree([root_column_group.clone()]);
    let _root_column_node: &root::TableColumnNode = &root_column_tree_state.column_tree()[0];
    let _root_column_group: root::TableColumnGroup = root_column_group;
    let prelude_group = prelude::TableColumnGroup::new(
        prelude::TableColumnGroupId::new("status-group"),
        "Status",
        [prelude::TableColumn::new("status", "Status")],
    );
    let prelude_state =
        prelude::TableState::new([prelude::TableRow::new("row-c").with_cell("status", "Ready")])
            .with_column_tree([prelude::TableColumnNode::from(prelude_group)]);
    assert_eq!(prelude_state.columns()[0].id().as_str(), "status");
    let root_pinned_state = root::TableState::new([root::TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("status", "Ready")])
    .with_columns([
        root::TableColumn::new("name", "Name"),
        root::TableColumn::new("team", "Team"),
        root::TableColumn::new("status", "Status"),
    ])
    .with_column_pinning(
        root::TableColumnPinning::new()
            .pinned_left(["name"])
            .pinned_right(["status"]),
    );
    let root_pinned_render_plan =
        root::Table::new("root-pinned-table", "Root pinned table", root_pinned_state).state();
    let root_pinned_layout: root::TablePinnedLayoutPlan = root_pinned_render_plan
        .pinned_layout()
        .expect("exported pinned layout plan should resolve")
        .clone();
    let _prelude_pinned_layout: prelude::TablePinnedLayoutPlan = root_pinned_layout.clone();
    let _prelude_header_plan: &prelude::TableHeaderGroupRegionsRenderPlan = _root_header_plan;
    assert_eq!(root_pinned_layout.table_id(), "root-pinned-table");
    let root_row_pinning: root::TableRowPinning = root::TableRowPinning::new()
        .pinned_top(["row-a"])
        .pinned_bottom(["row-b"]);
    let _prelude_row_pinning: prelude::TableRowPinning = root_row_pinning.clone();
    let _root_row_pinning_policy: root::TableRowPinningPolicy =
        root::TableRowPinningPolicy::PageOnly;
    let _prelude_row_pinning_policy: prelude::TableRowPinningPolicy =
        prelude::TableRowPinningPolicy::KeepPinnedRows;
    let _root_row_region: root::TableRowRegion = root::TableRowRegion::Top;
    let _prelude_row_region: prelude::TableRowRegion = prelude::TableRowRegion::Bottom;
    let root_row_regions: root::TableRowRegions = root::Table::new(
        "root-row-pinning-table",
        "Root row pinning table",
        root::TableState::new([
            root::TableRow::new("row-a").with_cell("name", "Alpha"),
            root::TableRow::new("row-b").with_cell("name", "Beta"),
        ])
        .with_columns([root::TableColumn::new("name", "Name")])
        .with_row_pinning(root_row_pinning.clone()),
    )
    .state()
    .table()
    .row_regions()
    .clone();
    let _prelude_row_regions: prelude::TableRowRegions = root_row_regions;
    let root_center_window: root::TableCenterColumnWindowPlan =
        root::TableCenterColumnWindowPlan::resolve(
            root_pinned_render_plan
                .column_regions()
                .iter()
                .find(|region| region.region() == root::TableColumnRegion::Center)
                .expect("center region should resolve")
                .columns(),
            ui_px(0.0),
            ui_px(128.0),
            2,
        )
        .expect("exported center column window plan should resolve");
    let _prelude_center_window: prelude::TableCenterColumnWindowPlan = root_center_window.clone();
    assert_eq!(root_center_window.rendered_column_count(), 1);
    let root_grid_viewport: root::GridViewport2D = root::resolve_grid_viewport_2d(
        &root::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
        &root::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
    );
    let _prelude_grid_viewport: prelude::GridViewport2D = root_grid_viewport.clone();
    let _prelude_grid_viewport_via_prelude: prelude::GridViewport2D =
        prelude::resolve_grid_viewport_2d(
            &prelude::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
            &prelude::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
        );
    assert_eq!(root_grid_viewport.row_overscan_range().start(), 0);
    let header_action: root::TableHeaderAction = root_plan.columns()[0]
        .sort_action()
        .expect("sortable exported table column should expose a header action")
        .clone();
    let _root_cache_key: root::TableStateCacheKey = table.table_state().cache_key();
    let _prelude_header_action: prelude::TableHeaderAction = header_action;
    let _prelude_cache_key: prelude::TableStateCacheKey = table.table_state().cache_key();
    let _root_aggregation: root::TableAggregation =
        root::TableAggregation::new("score", root::TableAggregateKind::Sum);
    let _prelude_aggregation: prelude::TableAggregation =
        prelude::TableAggregation::average("score");
    let _root_expansion: root::TableExpansionState = root::TableExpansionState::all();
    let _prelude_expansion: prelude::TableExpansionState =
        prelude::TableExpansionState::rows([prelude::TableRowId::new("group:team=ui")]);
    let _root_expansion_mode: root::TableExpansionMode = root::TableExpansionMode::Manual;
    let _prelude_expansion_mode: prelude::TableExpansionMode = prelude::TableExpansionMode::Client;
    let _root_stage_mode: root::TableStageMode = root::TableStageMode::Manual;
    let _prelude_stage_mode: prelude::TableStageMode = prelude::TableStageMode::Client;
    let root_filter = root::TableFilter::one_of("status", ["Ready", "Blocked"]);
    let _prelude_filter: prelude::TableFilter = prelude::TableFilter::contains("team", "UI");
    let _root_filter_kind: root::TableFilterKind = root_filter.kind().clone();
    let _prelude_filter_kind: prelude::TableFilterKind =
        prelude::TableFilterKind::Contains { query: "UI".into() };
    let _root_text_filter_operator: root::TableTextFilterOperator =
        root::TableTextFilterOperator::StartsWith;
    let _prelude_text_filter_operator: prelude::TableTextFilterOperator =
        prelude::TableTextFilterOperator::NotContains;
    let _root_numeric_bound: root::TableNumericFilterBound =
        root::TableNumericFilterBound::new(10.0)
            .expect("finite numeric bounds should be constructible");
    let _prelude_numeric_bound: prelude::TableNumericFilterBound =
        prelude::TableNumericFilterBound::new(20.0)
            .expect("finite numeric bounds should be constructible");
    let _root_numeric_filter_operator: root::TableNumericFilterOperator =
        root::TableNumericFilterOperator::GreaterThanOrEqual;
    let _prelude_numeric_filter_operator: prelude::TableNumericFilterOperator =
        prelude::TableNumericFilterOperator::LessThan;
    let root_range_filter = root::TableFilter::number_range("score", Some(10.0), Some(20.0))
        .expect("exported numeric range filter should construct");
    assert_eq!(
        root_range_filter.number_range_bounds(),
        Some((Some(10.0), Some(20.0)))
    );
    let root_facet_value = root::TableFacetValueCount::new("Ready", 2);
    let root_facets: root::TableColumnFacets =
        root::TableColumnFacets::manual("status", 2).with_unique_values([root_facet_value]);
    let _prelude_facets: prelude::TableColumnFacets = root_facets.clone();
    let root_global_facets: root::TableGlobalFacetSummary =
        root::TableGlobalFacetSummary::default();
    let _prelude_global_facets: prelude::TableGlobalFacetSummary = root_global_facets.clone();
    let root_global_filter: root::TableGlobalFilter =
        root::TableGlobalFilter::new("root-global-filter", "Search").query("ready");
    let _root_global_filter_state: root::TableGlobalFilterState = root_global_filter.state();
    let _root_global_filter_change: root::TableGlobalFilterChange =
        root::TableGlobalFilterChange::new("ready");
    let root_predicate_operator: root::TablePredicateFilterOperator =
        root::TablePredicateFilterOperator::text(root::TableTextFilterOperator::StartsWith);
    let root_predicate_filter: root::TablePredicateFilter =
        root::TablePredicateFilter::new("root-name-predicate", "Name", "name")
            .operator(root_predicate_operator)
            .value("Al");
    let root_predicate_filter_state: root::TablePredicateFilterState =
        root_predicate_filter.state();
    let _root_predicate_option: Option<&root::TablePredicateFilterOperatorOptionState> =
        root_predicate_filter_state.operator_options().first();
    let _root_predicate_change: root::TablePredicateFilterChange =
        root::TablePredicateFilterChange::new("name", root_predicate_operator, "Al");
    let root_table_toolbar: root::TableToolbar =
        root::TableToolbar::new("root-table-toolbar", "Filters").summary("2 visible controls");
    let _root_table_toolbar_state: root::TableToolbarState = root_table_toolbar.state();
    let prelude_global_filter: prelude::TableGlobalFilter =
        prelude::TableGlobalFilter::new("prelude-global-filter", "Search").default_query("ready");
    let _prelude_global_filter_state: prelude::TableGlobalFilterState =
        prelude_global_filter.state();
    let _prelude_global_filter_change: prelude::TableGlobalFilterChange =
        prelude::TableGlobalFilterChange::clear();
    let prelude_predicate_operator: prelude::TablePredicateFilterOperator =
        prelude::TablePredicateFilterOperator::number(
            prelude::TableNumericFilterOperator::GreaterThan,
        );
    let prelude_predicate_filter: prelude::TablePredicateFilter =
        prelude::TablePredicateFilter::new("prelude-score-predicate", "Score", "score")
            .operator(prelude_predicate_operator)
            .default_value("10");
    let prelude_predicate_filter_state: prelude::TablePredicateFilterState =
        prelude_predicate_filter.state();
    let _prelude_predicate_option: Option<&prelude::TablePredicateFilterOperatorOptionState> =
        prelude_predicate_filter_state.operator_options().first();
    let _prelude_predicate_change: prelude::TablePredicateFilterChange =
        prelude::TablePredicateFilterChange::clear("score");
    let prelude_table_toolbar: prelude::TableToolbar =
        prelude::TableToolbar::new("prelude-table-toolbar", "Filters")
            .summary("2 visible controls");
    let _prelude_table_toolbar_state: prelude::TableToolbarState = prelude_table_toolbar.state();
    let root_faceted_filter: root::TableFacetedFilter =
        root::TableFacetedFilter::new("root-status-filter", "Status", "status")
            .facets(root_facets.clone())
            .selected_values(["Ready"]);
    let root_faceted_filter_state: root::TableFacetedFilterState = root_faceted_filter.state();
    let _root_faceted_option: Option<&root::TableFacetedFilterOptionState> =
        root_faceted_filter_state.options().first();
    let _root_faceted_change: root::TableFacetedFilterChange =
        root::TableFacetedFilterChange::new("status", ["Ready"], Some("Ready"), true);
    let prelude_faceted_filter: prelude::TableFacetedFilter =
        prelude::TableFacetedFilter::new("prelude-status-filter", "Status", "status")
            .facets(root_facets.clone())
            .selected_values(["Ready"]);
    let prelude_faceted_filter_state: prelude::TableFacetedFilterState =
        prelude_faceted_filter.state();
    let _prelude_faceted_option: Option<&prelude::TableFacetedFilterOptionState> =
        prelude_faceted_filter_state.options().first();
    let _prelude_faceted_change: prelude::TableFacetedFilterChange =
        prelude::TableFacetedFilterChange::clear("status");
    let root_column_visibility: root::TableColumnVisibility =
        root::TableColumnVisibility::new("root-columns", "Columns")
            .columns([
                root::TableColumn::new("name", "Name").with_hideable(false),
                root::TableColumn::new("status", "Status"),
            ])
            .visibility(root::TableColumnVisibilityOverrides::new().hide("status"));
    let root_column_visibility_state: root::TableColumnVisibilityState =
        root_column_visibility.state();
    let _root_column_visibility_item: Option<&root::TableColumnVisibilityItemState> =
        root_column_visibility_state.items().first();
    let root_column_visibility_change: root::TableColumnVisibilityChange =
        root::TableColumnVisibilityChange::new("status", false);
    let _root_column_visibility_action: root::TableColumnVisibilityAction =
        root_column_visibility_change.action();
    let prelude_column_visibility: prelude::TableColumnVisibility =
        prelude::TableColumnVisibility::new("prelude-columns", "Columns")
            .columns([prelude::TableColumn::new("status", "Status")])
            .default_visibility(prelude::TableColumnVisibilityOverrides::new().hide("status"));
    let prelude_column_visibility_state: prelude::TableColumnVisibilityState =
        prelude_column_visibility.state();
    let _prelude_column_visibility_item: Option<&prelude::TableColumnVisibilityItemState> =
        prelude_column_visibility_state.items().first();
    let prelude_column_visibility_change: prelude::TableColumnVisibilityChange =
        prelude::TableColumnVisibilityChange::reset();
    let _prelude_column_visibility_action: prelude::TableColumnVisibilityAction =
        prelude_column_visibility_change.action();
    let _root_facet_range: Option<root::TableFacetRange> = root::TableFacetRange::new(1.0, 2.0);
    let root_range_facets =
        root::TableColumnFacets::manual("score", 2).with_numeric_range(1.0, 20.0);
    let root_range_filter: root::TableRangeFilter =
        root::TableRangeFilter::new("root-score-range", "Score", "score")
            .facets(root_range_facets.clone())
            .range(Some(1.0), Some(20.0));
    let _root_range_filter_state: root::TableRangeFilterState = root_range_filter.state();
    let _root_range_change: root::TableRangeFilterChange =
        root::TableRangeFilterChange::new("score", "1", "20");
    let prelude_range_filter: prelude::TableRangeFilter =
        prelude::TableRangeFilter::new("prelude-score-range", "Score", "score")
            .facets(root_range_facets)
            .range(Some(1.0), Some(20.0));
    let _prelude_range_filter_state: prelude::TableRangeFilterState = prelude_range_filter.state();
    let _prelude_range_change: prelude::TableRangeFilterChange =
        prelude::TableRangeFilterChange::clear("score");
    let _prelude_facet_value: prelude::TableFacetValueCount =
        prelude::TableFacetValueCount::new("Blocked", 1);
    let _root_child_load_state: root::TableRowChildrenLoadState =
        root::TableRowChildrenLoadState::loading("Loading children");
    let _prelude_child_load_state: prelude::TableRowChildrenLoadState =
        prelude::TableRowChildrenLoadState::failed("Load failed");
    let _prelude_row_kind: prelude::TableResolvedRowKind = prelude::TableResolvedRowKind::Leaf;
    let root_tree_state = root::TableState::new([root::TableRow::new("root")
        .with_cell("name", "Root")
        .with_child(root::TableRow::new("child").with_cell("name", "Child"))])
    .with_columns([root::TableColumn::new("name", "Name")])
    .with_all_rows_expanded();
    let root_tree_row: root::TableTreeRow = root_tree_state.resolve().final_model().rows()[0]
        .tree()
        .expect("tree source row should expose hierarchy metadata")
        .clone();
    let _prelude_tree_row: prelude::TableTreeRow = root_tree_row;
    let _resolved_kind: Option<&root::TableGroupRow> =
        table.table_state().resolve().final_model().rows()[0].group();
    let _root_table_modifiers: root::TableInputModifiers = root::TableInputModifiers::default();
    let _prelude_table_modifiers: prelude::TableInputModifiers =
        prelude::TableInputModifiers::default();
    let _root_row_action: Option<root::TableRowAction> = None;
    let _prelude_row_action: Option<prelude::TableRowAction> = None;
    let _root_row_activation: Option<root::TableRowActivation> = None;
    let _prelude_row_activation: Option<prelude::TableRowActivation> = None;
    let _root_row_expansion: Option<root::TableRowExpansionToggle> = None;
    let _prelude_row_expansion: Option<prelude::TableRowExpansionToggle> = None;
    let _root_activation_kind: root::TableRowActivationKind =
        root::TableRowActivationKind::DoubleClick;
    let _prelude_activation_kind: prelude::TableRowActivationKind =
        prelude::TableRowActivationKind::Keyboard;
    let _root_pinning: root::TableColumnPinning =
        root::TableColumnPinning::new().pinned_left(["name"]);
    let _root_width_policy: root::TableColumnWidthPolicy = root::TableColumnWidthPolicy::ContentFit;
    let _prelude_width_policy: prelude::TableColumnWidthPolicy =
        prelude::TableColumnWidthPolicy::Fixed;
    let content_fit_column = root::TableColumn::new("status", "Status").with_content_fit();
    assert!(content_fit_column.is_content_fit());
    assert_eq!(
        content_fit_column.width_policy(),
        root::TableColumnWidthPolicy::ContentFit
    );
    let root_visibility = root::TableColumnVisibilityOverrides::new()
        .hide("score")
        .show("status")
        .without("missing");
    let _root_visibility: root::TableColumnVisibilityOverrides = root_visibility.clone();
    let _prelude_visibility: prelude::TableColumnVisibilityOverrides =
        prelude::TableColumnVisibilityOverrides::new().show("status");
    assert_eq!(
        root_visibility.override_for(&root::TableColumnId::new("score")),
        Some(false)
    );
    let root_sizing = root::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let _root_sizing: root::TableColumnSizing = root_sizing.clone();
    let _prelude_sizing: prelude::TableColumnSizing =
        prelude::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let root_resize_state = root::TableColumnResizeState::begin(
        "name",
        ui_px(12.0),
        ui_px(180.0),
        [("name", ui_px(180.0))],
    );
    let root_resize_update: root::TableColumnResizeUpdate = root::drag_table_column_resize(
        root::TableColumnResizeMode::OnChange,
        root::TableColumnResizeDirection::Ltr,
        &root_sizing,
        &root_resize_state,
        ui_px(24.0),
    );
    let _prelude_resize_state: prelude::TableColumnResizeState = root_resize_update.state().clone();
    let _prelude_resize_update: prelude::TableColumnResizeUpdate = root::end_table_column_resize(
        prelude::TableColumnResizeMode::OnEnd,
        prelude::TableColumnResizeDirection::Ltr,
        &prelude::TableColumnSizing::new().with_width("name", ui_px(180.0)),
        &root_resize_state,
        Some(ui_px(24.0)),
    );
    let root_resize_change = root::TableColumnSizingChange::new(
        "name",
        ui_px(204.0),
        root_resize_update
            .committed_sizing()
            .cloned()
            .expect("resize update should commit in on-change mode"),
    );
    let _prelude_resize_change: prelude::TableColumnSizingChange = root_resize_change;
    let _root_resolved_sizing: root::TableResolvedColumnSizing = table
        .table_state()
        .resolve()
        .visible_column_sizing()
        .column(&root::TableColumnId::new("name"))
        .expect("resolved column sizing should be available")
        .clone();
    let _prelude_resolved_sizing: prelude::TableResolvedColumnSizing =
        _root_resolved_sizing.clone();
    let _root_resolved_sizing_regions: root::TableResolvedColumnSizingRegions = table
        .table_state()
        .resolve()
        .visible_column_sizing()
        .clone();
    let _prelude_resolved_sizing_regions: prelude::TableResolvedColumnSizingRegions =
        _root_resolved_sizing_regions.clone();
    let _root_default_width = root::TABLE_DEFAULT_COLUMN_WIDTH;
    let _root_min_width = root::TABLE_MIN_COLUMN_WIDTH;
    let _root_max_width = root::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_default_width = prelude::TABLE_DEFAULT_COLUMN_WIDTH;
    let _prelude_min_width = prelude::TABLE_MIN_COLUMN_WIDTH;
    let _prelude_max_width = prelude::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_region: prelude::TableColumnRegion = prelude::TableColumnRegion::Center;
    let _prelude_regions: prelude::TableColumnRegions = table
        .table_state()
        .resolve()
        .visible_column_regions()
        .clone();

    assert_eq!(root_plan.role(), Role::Table);
    assert!(!root_plan.column_facets().is_empty());
    assert_eq!(
        root::TableRowActivationKind::DoubleClick.as_str(),
        "double-click"
    );
    assert_eq!(virtualizer.resolve().overscan(), 2);
}

#[test]
fn feedback_tree_and_virtualized_list_public_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_status_cue: root::StatusCue = root::StatusCue::new("status", "Ready");
    let prelude_status_cue: prelude::StatusCue = prelude::StatusCue::new("status", "Ready");
    let root_empty_state: root::EmptyState = root::EmptyState::new("empty", "No results");
    let prelude_empty_state: prelude::EmptyState = prelude::EmptyState::new("empty", "No results");
    let root_tree_descriptor: root::TreeItemDescriptor =
        root::TreeItemDescriptor::new("root", "Root")
            .with_children_unloaded()
            .child(root::TreeItemDescriptor::new("child", "Child"));
    let prelude_tree_descriptor: prelude::TreeItemDescriptor =
        prelude::TreeItemDescriptor::new("root", "Root").with_children_load_failed("Offline");
    let root_tree_load_state: root::TreeChildrenLoadState =
        root::TreeChildrenLoadState::loading("Loading children");
    let prelude_tree_load_state: prelude::TreeChildrenLoadState =
        prelude::TreeChildrenLoadState::unloaded();
    let direct_tree_load_state: TreeChildrenLoadState = TreeChildrenLoadState::loaded();
    let root_tree: root::Tree =
        root::Tree::new("root-tree", "Root tree", [root_tree_descriptor.clone()])
            .default_selected("root")
            .default_focused("root")
            .virtualized(true)
            .viewport_item_count(2)
            .overscan_count(1);
    let prelude_tree: prelude::Tree = prelude::Tree::new(
        "prelude-tree",
        "Prelude tree",
        [prelude::TreeItemDescriptor::new("root", "Root")],
    )
    .default_focused("root")
    .virtualized(true)
    .viewport_item_count(2)
    .overscan_count(1);
    let root_tree_state: root::TreeState = root::TreeState::resolve(
        Size::Medium,
        "Tree",
        None,
        None,
        [root_tree_descriptor.clone()],
    );
    let prelude_tree_state: prelude::TreeState =
        prelude::TreeState::resolve(Size::Medium, "Tree", None, None, [prelude_tree_descriptor]);
    let root_tree_plan: root::TreeRenderPlan = root::TreeRenderPlan::resolve(
        "root-tree-plan",
        "Root tree plan",
        root_tree_state.clone(),
        ui_px(0.0),
        ui_px(32.0),
        2,
        1,
    );
    let prelude_tree_plan: prelude::TreeRenderPlan = prelude::TreeRenderPlan::resolve(
        "prelude-tree-plan",
        "Prelude tree plan",
        prelude_tree_state.clone(),
        ui_px(0.0),
        ui_px(32.0),
        2,
        1,
    );
    let direct_tree_plan: TreeRenderPlan = TreeRenderPlan::resolve(
        "direct-tree-plan",
        "Direct tree plan",
        root_tree_state.clone(),
        ui_px(0.0),
        ui_px(32.0),
        2,
        1,
    );
    let move_items = [
        root::TreeItemDescriptor::new("root", "Root")
            .expanded(true)
            .child(root::TreeItemDescriptor::new("child", "Child")),
        root::TreeItemDescriptor::new("sibling", "Sibling"),
    ];
    let move_state: root::TreeState =
        root::TreeState::resolve(Size::Medium, "Move tree", None, None, move_items.clone());
    let root_tree_move: root::TreeMove = move_state
        .move_for_drop("child", "sibling", root::TreeDropPosition::Before)
        .expect("public Tree move payload should resolve");
    let _root_tree_move_target: &root::TreeMoveTarget = root_tree_move.target();
    let prelude_tree_position: prelude::TreeDropPosition = prelude::TreeDropPosition::Inside;
    let _direct_tree_move: TreeMove = root_tree_move.clone();
    let _direct_tree_move_target: &TreeMoveTarget = root_tree_move.target();
    let moved_tree = root::apply_tree_move(move_items, &root_tree_move)
        .expect("public apply_tree_move helper should apply valid payload");
    let _direct_moved_tree = apply_tree_move(moved_tree.clone(), &root_tree_move);
    let prelude_move_state: prelude::TreeState =
        prelude::TreeState::resolve(Size::Medium, "Move tree", None, None, moved_tree);
    let root_virtualized_state: root::VirtualizedListState =
        root::VirtualizedListState::resolve(Size::Small, false, 12, Some(4), Some(4), Some(3));
    let prelude_virtualized_state: prelude::VirtualizedListState =
        prelude::VirtualizedListState::resolve(Size::Small, false, 12, Some(4), Some(4), Some(3));
    let root_virtualized_items = (0..12)
        .map(|index| {
            root::VirtualizedListItemDescriptor::new(
                format!("root-item-{index}"),
                format!("Root item {index}"),
            )
        })
        .collect::<Vec<_>>();
    let root_virtualized_list: root::VirtualizedList = root::VirtualizedList::new(
        "root-virtualized-component",
        "Root virtualized component",
        root_virtualized_items.clone(),
    )
    .default_active_index(4)
    .default_selected_index(4)
    .viewport_item_count(3);
    let prelude_virtualized_items = (0..12)
        .map(|index| {
            prelude::VirtualizedListItemDescriptor::new(
                format!("prelude-item-{index}"),
                format!("Prelude item {index}"),
            )
        })
        .collect::<Vec<_>>();
    let prelude_virtualized_list: prelude::VirtualizedList = prelude::VirtualizedList::new(
        "prelude-virtualized-component",
        "Prelude virtualized component",
        prelude_virtualized_items.clone(),
    )
    .default_active_index(4)
    .default_selected_index(4)
    .viewport_item_count(3);
    let root_virtualized_plan: root::VirtualizedListRenderPlan =
        root::VirtualizedListRenderPlan::resolve(
            "root-virtualized-list",
            "Root virtualized list",
            root_virtualized_state.clone(),
            &root_virtualized_items,
            ui_px(28.0),
            ui_px(56.0),
        );
    let prelude_virtualized_plan: prelude::VirtualizedListRenderPlan =
        prelude::VirtualizedListRenderPlan::resolve(
            "prelude-virtualized-list",
            "Prelude virtualized list",
            prelude_virtualized_state.clone(),
            &prelude_virtualized_items,
            ui_px(28.0),
            ui_px(56.0),
        );
    let _root_virtualized_row: &root::VirtualizedListRowRenderPlan =
        root_virtualized_plan.active_row().unwrap();
    let _prelude_virtualized_row: &prelude::VirtualizedListRowRenderPlan =
        prelude_virtualized_plan.active_row().unwrap();
    let root_virtualized_component_plan = root_virtualized_list.state();
    let prelude_virtualized_component_plan = prelude_virtualized_list.state();
    let root_tree_component_state = root_tree.state();
    let root_tree_component_plan = root_tree.render_plan(ui_px(0.0), ui_px(32.0));
    let prelude_tree_component_state = prelude_tree.state();
    let prelude_tree_component_plan = prelude_tree.render_plan(ui_px(0.0), ui_px(32.0));
    let _root_tree_row: &root::TreeRowRenderPlan = root_tree_plan.rows().first().unwrap();
    let _prelude_tree_row: &prelude::TreeRowRenderPlan = prelude_tree_plan.rows().first().unwrap();
    let _direct_tree_row: &TreeRowRenderPlan = direct_tree_plan.rows().first().unwrap();
    let _root_tree_toggle: Option<root::TreeToggle> =
        root::TreeToggle::from_item(&root_tree_state.items()[0]);
    let _prelude_tree_toggle: Option<prelude::TreeToggle> =
        prelude::TreeToggle::from_item(&prelude_tree_state.items()[0]);
    let _root_tree_selection: Option<root::TreeSelection> =
        root::TreeSelection::from_item(&root_tree_state.items()[0]);
    let _prelude_tree_selection: Option<prelude::TreeSelection> =
        prelude::TreeSelection::from_item(&prelude_tree_state.items()[0]);
    let _root_tree_focus: root::TreeFocusTarget = root::TreeFocusTarget::new(0, "root");
    let _prelude_tree_focus: prelude::TreeFocusTarget = prelude::TreeFocusTarget::new(0, "root");
    let _root_tree_action: Option<root::TreeKeyboardAction> =
        root_tree_state.keyboard_action_for_key("right");
    let _prelude_tree_action: Option<prelude::TreeKeyboardAction> =
        prelude_tree_state.keyboard_action_for_key("right");
    let _root_virtualized_activation: root::VirtualizedListActivation =
        root::VirtualizedListActivation::new(4);
    let _prelude_virtualized_activation: prelude::VirtualizedListActivation =
        prelude::VirtualizedListActivation::new(4);
    let _root_scroll_strategy: root::VirtualizedListScrollStrategy =
        root::VirtualizedListScrollStrategy::Center;
    let _prelude_scroll_strategy: prelude::VirtualizedListScrollStrategy =
        prelude::VirtualizedListScrollStrategy::Center;

    assert_eq!(root_status_cue.state().role(), Role::Label);
    assert_eq!(prelude_status_cue.state().role(), Role::Label);
    assert_eq!(root_empty_state.state().role(), Role::Section);
    assert_eq!(prelude_empty_state.state().role(), Role::Section);
    assert_eq!(root_tree_component_state.role(), Role::Tree);
    assert_eq!(prelude_tree_component_state.item_role(), Role::TreeItem);
    assert_eq!(root_tree_component_state.focused_value(), Some("root"));
    assert_eq!(root_tree_component_plan.role(), Role::Tree);
    assert_eq!(prelude_tree_component_plan.row_role(), Role::TreeItem);
    assert_eq!(root_tree_state.items().len(), 1);
    assert_eq!(prelude_tree_state.items().len(), 1);
    assert_eq!(root_tree_state.role(), Role::Tree);
    assert_eq!(root_tree_state.items()[0].role(), Role::TreeItem);
    assert!(root_tree_state.items()[0].has_children());
    assert_eq!(
        root_tree_state.items()[0].children_load_state().as_str(),
        "unloaded"
    );
    assert!(prelude_tree_state.items()[0].children_load_failed());
    assert!(root_tree_load_state.is_loading());
    assert!(prelude_tree_load_state.is_unloaded());
    assert!(direct_tree_load_state.is_loaded());
    assert_eq!(root::tree_navigation_target("home", 0, &[false]), Some(0));
    assert_eq!(
        prelude::tree_navigation_target("home", 0, &[false]),
        Some(0)
    );
    assert_eq!(root_tree_plan.rows()[0].render_key(), "0:root");
    assert_eq!(prelude_tree_plan.virtualizer().count(), 1);
    assert_eq!(direct_tree_plan.rendered_row_count(), 1);
    assert_eq!(root_tree_move.position(), TreeDropPosition::Before);
    assert_eq!(root_tree_move.target_parent_value(), None);
    assert_eq!(root_tree_move.sibling_anchor_value(), Some("sibling"));
    assert_eq!(prelude_tree_position.as_str(), "inside");
    assert_eq!(prelude_move_state.items()[0].value(), "root");
    assert_eq!(prelude_move_state.items()[1].value(), "child");
    assert_eq!(
        root_virtualized_state.navigation_target("pagedown"),
        Some(7)
    );
    assert_eq!(
        prelude_virtualized_state.navigation_target("pagedown"),
        Some(7)
    );
    assert_eq!(root_virtualized_component_plan.role(), Role::ListBox);
    assert_eq!(
        prelude_virtualized_component_plan.row_role(),
        Role::ListBoxOption
    );
    assert_eq!(root_virtualized_plan.role(), Role::ListBox);
    assert_eq!(prelude_virtualized_plan.row_role(), Role::ListBoxOption);
    assert_eq!(
        root::virtualized_list_scroll_target(
            root::VirtualizedListScrollStrategy::Top,
            4,
            root_virtualized_plan.state().item_count(),
            root_virtualized_plan.metrics().row_height(),
            root_virtualized_plan.virtualizer().viewport_extent(),
            root_virtualized_plan.virtualizer().scroll_offset(),
        ),
        ui_px(112.0)
    );
    assert_eq!(
        prelude::virtualized_list_scroll_target(
            prelude::VirtualizedListScrollStrategy::Top,
            4,
            prelude_virtualized_plan.state().item_count(),
            prelude_virtualized_plan.metrics().row_height(),
            prelude_virtualized_plan.virtualizer().viewport_extent(),
            prelude_virtualized_plan.virtualizer().scroll_offset(),
        ),
        ui_px(112.0)
    );
    assert_eq!(
        root::virtualized_list_navigation_target("end", 4, 12, 3),
        Some(11)
    );
    assert_eq!(
        prelude::virtualized_list_navigation_target("end", 4, 12, 3),
        Some(11)
    );
}

#[open_gpui::test]
fn table_runtime_header_click_emits_sort_action(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new("sort-runtime-table", "Sort runtime", sample_table_state(12))
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_sort_requested(move |action, _, _| {
                    actions.borrow_mut().push(action);
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let score_header = cx
        .debug_bounds("table:sort-runtime-table:header:score")
        .expect("score header should render as an interactive sort target");
    cx.simulate_click(score_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].column_id().as_str(), "score");
    assert_eq!(actions[0].label(), "Score");
    assert_eq!(actions[0].current_direction(), None);
    assert_eq!(
        actions[0].next_direction(),
        Some(TableSortDirection::Ascending)
    );
    assert_eq!(actions[0].next_sorting()[0].column().as_str(), "score");
}

#[open_gpui::test]
fn table_runtime_row_click_and_tree_toggle_emit_controlled_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActivationLog = Vec<(String, String, usize, Option<bool>, bool)>;
    type ToggleLog = Vec<(String, bool, usize, Option<bool>)>;

    struct TestView {
        activations: Rc<RefCell<ActivationLog>>,
        toggles: Rc<RefCell<ToggleLog>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let toggles = self.toggles.clone();
            let state = TableState::new([TableRow::new("root")
                .with_cell("name", "Workspace")
                .with_cell("status", "Ready")
                .with_child(
                    TableRow::new("child")
                        .with_cell("name", "UI")
                        .with_cell("status", "Building"),
                )])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(180.0)),
                TableColumn::new("status", "Status").with_width(ui_px(120.0)),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("tree-runtime-table", "Tree runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push((
                        activation.row_id().as_str().to_owned(),
                        activation.kind().as_str().to_owned(),
                        activation.action().depth(),
                        activation.action().tree_expanded(),
                        activation.action().modifiers().modified(),
                    ));
                })
                .on_row_expansion_request(move |toggle, _, _| {
                    toggles.borrow_mut().push((
                        toggle.row_id().as_str().to_owned(),
                        toggle.expanded(),
                        toggle.action().depth(),
                        toggle.action().tree_expanded(),
                    ));
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row = cx
        .debug_bounds("table:tree-runtime-table:row:root")
        .expect("root row should render");
    cx.simulate_click(row.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        activations.borrow().as_slice(),
        &[("root".to_owned(), "click".to_owned(), 0, Some(false), false)]
    );
    assert!(toggles.borrow().is_empty());

    let toggle = cx
        .debug_bounds("table:tree-runtime-table:tree-toggle:root")
        .expect("root tree toggle should render");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().len(), 1);
    assert_eq!(
        toggles.borrow().as_slice(),
        &[("root".to_owned(), true, 0, Some(false))]
    );
}

#[open_gpui::test]
fn table_runtime_row_click_selection_is_controlled_and_preserves_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActivationLog = Vec<String>;
    type SelectionLog = Vec<(
        String,
        bool,
        TableSelectionMode,
        TableSelectionScope,
        Vec<String>,
    )>;

    struct TestView {
        activations: Rc<RefCell<ActivationLog>>,
        selections: Rc<RefCell<SelectionLog>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let state = TableState::new([
                TableRow::new("row-a")
                    .with_cell("name", "Alpha")
                    .with_cell("status", "Ready"),
                TableRow::new("row-b")
                    .with_cell("name", "Beta")
                    .with_cell("status", "Blocked"),
            ])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(180.0)),
                TableColumn::new("status", "Status").with_width(ui_px(120.0)),
            ])
            .with_pagination(TablePagination::disabled())
            .with_selection_mode(TableSelectionMode::Multiple)
            .with_selection_activation_mode(TableSelectionActivationMode::RowClick)
            .with_selected_rows(["row-a"]);
            let table = Table::new("selection-runtime-table", "Selection runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_selection_change(move |change, _, _| {
                    selections.borrow_mut().push((
                        change.row_id().as_str().to_owned(),
                        change.selected(),
                        change.selection_mode(),
                        change.scope(),
                        change
                            .current_selection()
                            .iter()
                            .map(|row_id| row_id.as_str().to_owned())
                            .collect(),
                    ));
                })
                .on_row_activate(move |activation, _, _| {
                    activations
                        .borrow_mut()
                        .push(activation.row_id().as_str().to_owned());
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row = cx
        .debug_bounds("table:selection-runtime-table:row:row-a")
        .expect("selected row should render");
    cx.simulate_click(row.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().as_slice(), ["row-a"]);
    assert_eq!(
        selections.borrow().as_slice(),
        &[(
            "row-a".to_owned(),
            false,
            TableSelectionMode::Multiple,
            TableSelectionScope::Row,
            Vec::<String>::new(),
        )],
        "row-click selection should emit the next selected-row ids without swallowing activation"
    );
}

#[open_gpui::test]
fn table_runtime_text_cell_edit_emits_change_without_row_interaction(
    cx: &mut open_gpui::TestAppContext,
) {
    type EditLog = Vec<(String, String, Option<usize>, String, String)>;

    struct TestView {
        state: Rc<RefCell<TableState>>,
        edits: Rc<RefCell<EditLog>>,
        activations: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table_state = self.state.borrow().clone();
            let state_for_edit = self.state.clone();
            let edits = self.edits.clone();
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let table = Table::new("edit-runtime-table", "Edit runtime", table_state)
                .row_height(ui_px(32.0))
                .viewport_extent(ui_px(96.0))
                .on_cell_edit_change(move |change, _, _| {
                    edits.borrow_mut().push((
                        change.row_id().as_str().to_owned(),
                        change.column_id().as_str().to_owned(),
                        change.source_index(),
                        change.previous_text().to_owned(),
                        change.next_text().to_owned(),
                    ));
                    let (next, outcome) = change.apply_to(state_for_edit.borrow().clone());
                    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
                    *state_for_edit.borrow_mut() = next;
                })
                .on_row_activate(move |activation, _, _| {
                    activations
                        .borrow_mut()
                        .push(activation.row_id().as_str().to_owned());
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections
                        .borrow_mut()
                        .push(selection.row_id().as_str().to_owned());
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    cx.update(init_text_input);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("status", "Ready")])
        .with_columns([
            TableColumn::new("name", "Name")
                .with_text_editable(true)
                .with_width(ui_px(180.0)),
            TableColumn::new("status", "Status").with_width(ui_px(120.0)),
        ])
        .with_pagination(TablePagination::disabled())
        .with_selection_activation_mode(TableSelectionActivationMode::RowClick),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        state: state.clone(),
        edits: edits.clone(),
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:edit-runtime-table:cell:row-a:status")
            .is_some(),
        "read-only cell should still render as a plain table cell"
    );
    assert!(
        cx.debug_bounds("text-input:table:edit-runtime-table:cell:row-a:name:editor:root")
            .is_some(),
        "editable name cell should render a nested text input with a stable selector"
    );
    assert!(
        cx.debug_bounds("text-input:table:edit-runtime-table:cell:row-a:status:editor:root")
            .is_none(),
        "read-only status cell must not mount a text input"
    );

    let input = cx
        .debug_bounds("text-input:table:edit-runtime-table:cell:row-a:name:editor:root")
        .expect("editable name input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input(" Prime");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let edits = edits.borrow();
    assert!(
        edits.len() >= 2,
        "simulated text entry should emit controlled changes as the input value evolves"
    );
    assert!(
        edits.iter().all(|(row_id, column_id, source_index, _, _)| {
            row_id == "row-a" && column_id == "name" && *source_index == Some(0)
        }),
        "every edit payload should stay targeted by stable row and column ids"
    );
    assert_eq!(
        edits.first().cloned(),
        Some((
            "row-a".to_owned(),
            "name".to_owned(),
            Some(0),
            "Alpha".to_owned(),
            "Alpha ".to_owned(),
        ))
    );
    assert_eq!(
        edits.last().cloned(),
        Some((
            "row-a".to_owned(),
            "name".to_owned(),
            Some(0),
            "Alpha Prim".to_owned(),
            "Alpha Prime".to_owned(),
        ))
    );
    assert_eq!(
        state
            .borrow()
            .rows()
            .first()
            .and_then(|row| row.cell(&TableColumnId::new("name")))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Alpha Prime")
    );
    assert!(
        activations.borrow().is_empty(),
        "typing inside editable cell must not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "typing inside editable cell must not toggle row selection"
    );
}

#[open_gpui::test]
fn table_runtime_multiline_cell_edit_emits_newline_change_without_row_interaction(
    cx: &mut open_gpui::TestAppContext,
) {
    type EditLog = Vec<(String, String, Option<usize>, String, String)>;

    struct TestView {
        state: Rc<RefCell<TableState>>,
        edits: Rc<RefCell<EditLog>>,
        activations: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table_state = self.state.borrow().clone();
            let state_for_edit = self.state.clone();
            let edits = self.edits.clone();
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let table = Table::new(
                "multiline-edit-table",
                "Multiline edit runtime",
                table_state,
            )
            .row_height(ui_px(82.0))
            .viewport_extent(ui_px(120.0))
            .on_cell_edit_change(move |change, _, _| {
                edits.borrow_mut().push((
                    change.row_id().as_str().to_owned(),
                    change.column_id().as_str().to_owned(),
                    change.source_index(),
                    change.previous_text().to_owned(),
                    change.next_text().to_owned(),
                ));
                let (next, outcome) = change.apply_to(state_for_edit.borrow().clone());
                assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
                *state_for_edit.borrow_mut() = next;
            })
            .on_row_activate(move |activation, _, _| {
                activations
                    .borrow_mut()
                    .push(activation.row_id().as_str().to_owned());
            })
            .on_row_selection_change(move |selection, _, _| {
                selections
                    .borrow_mut()
                    .push(selection.row_id().as_str().to_owned());
            });

            div().w(px(520.0)).h(px(180.0)).child(table)
        }
    }

    cx.update(init_text_input);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("notes", "Line 1")])
        .with_columns([
            TableColumn::new("name", "Name").with_width(ui_px(120.0)),
            TableColumn::new("notes", "Notes")
                .with_multiline_text_editor(3)
                .with_width(ui_px(280.0)),
        ])
        .with_pagination(TablePagination::disabled())
        .with_selection_activation_mode(TableSelectionActivationMode::RowClick),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        state: state.clone(),
        edits: edits.clone(),
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("textarea:table:multiline-edit-table:cell:row-a:notes:editor:root")
            .is_some(),
        "multiline editable notes cell should render a nested textarea"
    );
    assert!(
        cx.debug_bounds("text-input:table:multiline-edit-table:cell:row-a:notes:editor:root")
            .is_none(),
        "multiline editable notes cell must not render the single-line text input"
    );

    let textarea = cx
        .debug_bounds("textarea:table:multiline-edit-table:cell:row-a:notes:editor:root")
        .expect("multiline notes textarea should expose a stable debug selector");
    cx.simulate_click(textarea.center(), Default::default());
    cx.simulate_input("\nLine 2");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let edits = edits.borrow();
    assert!(
        edits.len() >= 2,
        "simulated multiline entry should emit controlled changes as the textarea value evolves"
    );
    assert!(
        edits.iter().all(|(row_id, column_id, source_index, _, _)| {
            row_id == "row-a" && column_id == "notes" && *source_index == Some(0)
        }),
        "every multiline edit payload should stay targeted by stable row and column ids"
    );
    assert_eq!(
        edits.last().cloned(),
        Some((
            "row-a".to_owned(),
            "notes".to_owned(),
            Some(0),
            "Line 1\nLine ".to_owned(),
            "Line 1\nLine 2".to_owned(),
        ))
    );
    assert_eq!(
        state
            .borrow()
            .rows()
            .first()
            .and_then(|row| row.cell(&TableColumnId::new("notes")))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Line 1\nLine 2")
    );
    assert!(
        activations.borrow().is_empty(),
        "typing inside multiline editable cell must not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "typing inside multiline editable cell must not toggle row selection"
    );
}

#[open_gpui::test]
fn table_runtime_explicit_control_selection_ignores_row_click(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let state = sample_table_state(4)
                .with_selection_activation_mode(TableSelectionActivationMode::ExplicitControl)
                .with_selected_rows(["row-0001"]);
            let table = Table::new("explicit-selection-table", "Explicit selection", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_selection_change(move |change, _, _| {
                    selections
                        .borrow_mut()
                        .push(change.row_id().as_str().to_owned());
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row = cx
        .debug_bounds("table:explicit-selection-table:row:row-0001")
        .expect("selected row should render");
    cx.simulate_click(row.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        selections.borrow().is_empty(),
        "explicit-control selection should wait for checkbox/radio chrome instead of row clicks"
    );
}

#[open_gpui::test]
fn table_runtime_unloaded_branch_toggle_emits_child_load_metadata(
    cx: &mut open_gpui::TestAppContext,
) {
    type ToggleLog = Vec<(String, bool, usize, Option<String>, bool, usize)>;

    struct TestView {
        toggles: Rc<RefCell<ToggleLog>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let toggles = self.toggles.clone();
            let state = TableState::new([TableRow::new("remote")
                .with_cell("name", "Remote workspace")
                .with_cell("status", "Retry")
                .with_children_load_failed("Network unavailable")])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(180.0)),
                TableColumn::new("status", "Status").with_width(ui_px(120.0)),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("remote-runtime-table", "Remote runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_expansion_request(move |toggle, _, _| {
                    let load_state = toggle
                        .children_load_state()
                        .and_then(TableRowChildrenLoadState::message)
                        .map(str::to_owned);
                    let failed = toggle
                        .children_load_state()
                        .is_some_and(TableRowChildrenLoadState::is_failed);
                    toggles.borrow_mut().push((
                        toggle.row_id().as_str().to_owned(),
                        toggle.expanded(),
                        toggle.action().depth(),
                        load_state,
                        failed,
                        toggle.loaded_child_count(),
                    ));
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let toggle = cx
        .debug_bounds("table:remote-runtime-table:tree-toggle:remote")
        .expect("remote branch tree toggle should render without loaded children");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        toggles.borrow().as_slice(),
        &[(
            "remote".to_owned(),
            true,
            0,
            Some("Network unavailable".to_owned()),
            true,
            0,
        )]
    );
}

#[open_gpui::test]
fn table_runtime_resize_emits_controlled_sizing_change(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnSizingChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = sample_table_state(12)
                .with_column_sizing(TableColumnSizing::new().with_width("name", ui_px(160.0)));
            let table = Table::new("resize-runtime-table", "Resize runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .column_resize_mode(TableColumnResizeMode::OnEnd)
                .on_column_sizing_change(move |change, _, _| {
                    changes.borrow_mut().push(change);
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let handle = cx
        .debug_bounds("table:resize-runtime-table:resize:name")
        .expect("name resize handle should be rendered")
        .center();

    cx.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(handle.x + px(18.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());

    cx.simulate_mouse_move(
        point(handle.x + px(58.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());

    cx.simulate_mouse_up(
        point(handle.x + px(58.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].column_id().as_str(), "name");
    assert!(changes[0].width() > ui_px(160.0));
    assert_eq!(
        changes[0]
            .sizing()
            .width(changes[0].column_id())
            .expect("controlled sizing should include resized column"),
        changes[0].width()
    );
}

#[open_gpui::test]
fn table_runtime_exposes_pinned_region_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "UI")
                .with_cell("score", 42_usize)
                .with_cell("status", "Ready")])
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team"),
                TableColumn::new("score", "Score"),
                TableColumn::new("status", "Status"),
            ])
            .with_column_order(["status", "score", "team", "name"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score"])
                    .pinned_right(["status"]),
            )
            .with_pagination(TablePagination::disabled());
            let table = Table::new("pinned-runtime-table", "Pinned runtime table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0));

            div()
                .size_full()
                .child(div().w(px(520.0)).h(px(140.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for region in ["left", "center", "right"] {
        assert!(
            cx.debug_bounds(&format!(
                "table:pinned-runtime-table:header-region:{region}"
            ))
            .is_some(),
            "expected header {region} region selector to render"
        );
        assert!(
            cx.debug_bounds(&format!(
                "table:pinned-runtime-table:row-region:row-a:{region}"
            ))
            .is_some(),
            "expected body {region} region selector to render"
        );
    }

    assert!(
        cx.debug_bounds("scroll-area:table:pinned-runtime-table:header-center-scroll")
            .is_some(),
        "expected pinned header center region to render a horizontal scroll viewport"
    );
    assert!(
        cx.debug_bounds("scroll-area:table:pinned-runtime-table:row-center-scroll:row-a")
            .is_some(),
        "expected pinned body center region to render a horizontal scroll viewport"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_center_scrolls_without_moving_fixed_lanes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "pinned-scroll-runtime-table",
                "Pinned scroll table",
                sample_pinned_table_state(),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0));

            div()
                .size_full()
                .child(div().w(px(420.0)).h(px(140.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let header_center_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:header:team")
        .expect("center header should render before horizontal scrolling");
    let body_center_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:team")
        .expect("center body cell should render before horizontal scrolling");
    let left_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:score")
        .expect("left pinned body cell should render before horizontal scrolling");
    let right_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:status")
        .expect("right pinned body cell should render before horizontal scrolling");
    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-scroll-runtime-table:row-center-scroll:row-a")
        .expect("body center lane should expose a horizontal scroll viewport");

    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-64.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let header_center_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:header:team")
        .expect("center header should remain rendered after horizontal scrolling");
    let body_center_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:team")
        .expect("center body cell should remain rendered after horizontal scrolling");
    let left_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:score")
        .expect("left pinned body cell should remain rendered after horizontal scrolling");
    let right_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:status")
        .expect("right pinned body cell should remain rendered after horizontal scrolling");

    assert!(
        header_center_after.left() < header_center_before.left(),
        "expected shared horizontal handle to move center header left; before={header_center_before:?} after={header_center_after:?}"
    );
    assert!(
        body_center_after.left() < body_center_before.left(),
        "expected horizontal body center lane to move left; before={body_center_before:?} after={body_center_after:?}"
    );
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "expected left pinned lane to keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "expected right pinned lane to keep its screen-space x position"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_mounts_only_rendered_center_cells(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "center-window-runtime-table",
                "Center window runtime table",
                sample_center_window_table_state_with_rows(20),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0);

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_00")
            .is_some(),
        "expected the first center header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_00")
            .is_some(),
        "expected the first center body cell to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_05")
            .is_none(),
        "far-right center headers should stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_05")
            .is_none(),
        "far-right center body cells should stay unmounted before horizontal scrolling"
    );

    let left_before = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should render before horizontal scrolling");
    let right_before = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:status")
        .expect("right pinned cell should render before horizontal scrolling");
    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:center-window-runtime-table:row-center-scroll:row-0000")
        .expect("body center lane should expose a horizontal scroll viewport");

    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_00")
            .is_none(),
        "leftmost center headers should unmount after the center window advances"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_00")
            .is_none(),
        "leftmost center cells should unmount after the center window advances"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_05")
            .is_some(),
        "far-right center headers should render after horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "far-right center cells should render after horizontal scrolling"
    );

    let left_after = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should remain rendered after horizontal scrolling");
    let right_after = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:status")
        .expect("right pinned cell should remain rendered after horizontal scrolling");
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "left pinned lane should keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "right pinned lane should keep its screen-space x position"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_still_emits_sort_for_rendered_center_header(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new(
                "center-window-sort-runtime-table",
                "Center window sort table",
                sample_center_window_table_state_with_rows(20),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0)
            .on_sort_requested(move |action, _, _| {
                actions.borrow_mut().push(action);
            });

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds(
            "scroll-area:table:center-window-sort-runtime-table:row-center-scroll:row-0000",
        )
        .expect("body center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let metric_05_header = cx
        .debug_bounds("table:center-window-sort-runtime-table:header:metric_05")
        .expect("virtualized center header should render after horizontal scrolling");
    cx.simulate_click(metric_05_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].column_id().as_str(), "metric_05");
    assert_eq!(actions[0].label(), "Metric 05");
    assert_eq!(actions[0].current_direction(), None);
    assert_eq!(
        actions[0].next_direction(),
        Some(TableSortDirection::Ascending)
    );
}

#[test]
fn table_center_column_window_recomputes_geometry_for_center_column_resize() {
    let base_plan = Table::new(
        "center-window-resize-plan-table",
        "Center window resize plan table",
        sample_center_window_table_state()
            .with_column_sizing(TableColumnSizing::new().with_width("metric_05", ui_px(120.0))),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .render_plan(UiPx::ZERO, ui_px(96.0));
    let base_center_columns = base_plan
        .column_regions()
        .iter()
        .find(|region| region.region() == TableColumnRegion::Center)
        .expect("center region should resolve")
        .columns();
    let base_window =
        TableCenterColumnWindowPlan::resolve(base_center_columns, ui_px(420.0), ui_px(68.0), 0)
            .expect("center column window should resolve");

    let resized_plan = Table::new(
        "center-window-resize-plan-table",
        "Center window resize plan table",
        sample_center_window_table_state()
            .with_column_sizing(TableColumnSizing::new().with_width("metric_05", ui_px(180.0))),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .render_plan(UiPx::ZERO, ui_px(96.0));
    let resized_center_columns = resized_plan
        .column_regions()
        .iter()
        .find(|region| region.region() == TableColumnRegion::Center)
        .expect("center region should resolve after resize")
        .columns();
    let resized_window =
        TableCenterColumnWindowPlan::resolve(resized_center_columns, ui_px(420.0), ui_px(68.0), 0)
            .expect("center column window should resolve after resize");

    assert!(resized_window.center_width() > base_window.center_width());
    assert_eq!(resized_window.visible_range(), base_window.visible_range());
    assert_eq!(
        resized_window.overscan_range(),
        base_window.overscan_range()
    );
    assert_eq!(
        resized_window
            .rendered_columns()
            .iter()
            .map(|column| column.id().as_str())
            .collect::<Vec<_>>(),
        base_window
            .rendered_columns()
            .iter()
            .map(|column| column.id().as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        resized_window.rendered_columns().last().unwrap().width()
            > base_window.rendered_columns().last().unwrap().width(),
        "expected the resized virtualized center header to widen"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_keeps_row_virtualizer_independent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "center-window-rows-runtime-table",
                "Center window rows runtime table",
                sample_center_window_table_state_with_rows(80),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(0);

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds(
            "scroll-area:table:center-window-rows-runtime-table:row-center-scroll:row-0000",
        )
        .expect("body center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "horizontal center window should reveal far-right cells before vertical scrolling"
    );

    let first_row_pinned_cell = cx
        .debug_bounds("table:center-window-rows-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should remain reachable before vertical scrolling");
    cx.simulate_event(ScrollWheelEvent {
        position: first_row_pinned_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:row:row-0000")
            .is_none(),
        "vertical scrolling should still advance the row virtualizer"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:row:row-0010")
            .is_some(),
        "row 10 should render after vertical scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0010:metric_05")
            .is_some(),
        "newly rendered rows should consume the current center column window"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0010:metric_00")
            .is_none(),
        "off-window center cells should remain unmounted on newly rendered rows"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_body_scrolls_without_moving_parent(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "pinned-body-scroll-runtime-table",
                "Pinned body scroll table",
                sample_pinned_table_state_with_rows(80),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div().w(px(440.0)).h(px(220.0)).child(
                    ScrollArea::new(
                        "pinned-table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "pinned-table-wrapper".into())
                                    .h(px(140.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let first_row_before = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0000")
        .expect("first pinned body row should render before vertical scrolling");
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0010")
            .is_none(),
        "row 10 should start outside the initial pinned body window"
    );
    let parent_bottom_before = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should be rendered before table scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:table:pinned-body-scroll-runtime-table:body-scroll")
        .expect("pinned table body viewport should expose a stable scroll selector");
    let first_row_cell = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:cell:row-0000:name")
        .expect("first pinned body row cell should render before vertical scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: first_row_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should still be rendered after table scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside pinned Table to stay inside the table body; before={parent_bottom_before:?} after={parent_bottom_after:?}"
    );
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0000")
            .is_none(),
        "expected first pinned row to unmount after the virtual window advances"
    );
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0010")
            .is_some(),
        "expected row 10 to render after scrolling the pinned table body"
    );
    assert!(
        viewport.size.width > px(0.0) && first_row_before.top() <= parent_bottom_after.bottom(),
        "pinned body viewport should remain measurable during the test"
    );
}

#[open_gpui::test]
fn table_runtime_row_pinning_keeps_bands_fixed_while_center_scrolls(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = sample_center_window_table_state_with_rows(80).with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-0000"])
                    .pinned_bottom(["row-0079"]),
            );
            let table = Table::new(
                "row-pinning-runtime-table",
                "Row pinning runtime table",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div().w(px(480.0)).h(px(240.0)).child(
                    ScrollArea::new(
                        "row-pinning-table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-table-wrapper".into())
                                    .h(px(164.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:top")
            .is_some(),
        "top row-pinning band should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:center")
            .is_some(),
        "center row-pinning band should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:bottom")
            .is_some(),
        "bottom row-pinning band should expose a stable debug selector"
    );
    let top_row_before = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0000")
        .expect("top pinned row should render before scrolling");
    let bottom_row_before = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0079")
        .expect("bottom pinned row should render before scrolling");
    let parent_bottom_before = cx
        .debug_bounds("row-pinning-parent-bottom")
        .expect("parent bottom should render before table scrolling");
    let top_name_before = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0000:name")
        .expect("top pinned row left-pinned cell should render before horizontal scrolling");
    let top_center_viewport = cx
        .debug_bounds("scroll-area:table:row-pinning-runtime-table:row-center-scroll:row-0000")
        .expect("top pinned row should expose a horizontal center lane");

    cx.simulate_event(ScrollWheelEvent {
        position: top_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_name_after_horizontal = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0000:name")
        .expect("top pinned row left-pinned cell should stay mounted after horizontal scrolling");
    assert_eq!(
        top_name_after_horizontal.left(),
        top_name_before.left(),
        "left-pinned cells inside pinned rows should not move with the center lane"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "horizontally scrolled pinned rows should reveal far-right center cells"
    );
    let _center_viewport = cx
        .debug_bounds("scroll-area:table:row-pinning-runtime-table:body-scroll")
        .expect("center body should expose the vertical scroll viewport");
    let center_row_cell = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0001:name")
        .expect("first center row left-pinned cell should render before center scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: center_row_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_after = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0000")
        .expect("top pinned row should remain mounted after center scrolling");
    let bottom_row_after = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0079")
        .expect("bottom pinned row should remain mounted after center scrolling");
    let parent_bottom_after = cx
        .debug_bounds("row-pinning-parent-bottom")
        .expect("parent bottom should remain mounted after center scrolling");
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "top pinned rows should stay fixed while center rows scroll"
    );
    assert_eq!(
        bottom_row_after.top(),
        bottom_row_before.top(),
        "bottom pinned rows should stay fixed while center rows scroll"
    );
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "vertical wheel input inside row-pinned Table should not move the outer page"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:row:row-0011")
            .is_some(),
        "center rows should advance independently between pinned bands"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:cell:row-0011:metric_05")
            .is_some(),
        "new center rows should consume the current horizontal center window"
    );
}

#[open_gpui::test]
fn table_runtime_row_pinning_keyboard_navigation_scrolls_to_unrendered_center_row(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = sample_center_window_table_state_with_rows(80)
                .with_row_pinning(TableRowPinning::new().pinned_top(["row-0000"]));
            let table = Table::new(
                "row-pinning-keyboard-table",
                "Row pinning keyboard table",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div()
                    .w(px(480.0))
                    .h(px(164.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(table),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_before = cx
        .debug_bounds("table:row-pinning-keyboard-table:row:row-0000")
        .expect("top pinned row should render before keyboard navigation");
    assert!(
        cx.debug_bounds("table:row-pinning-keyboard-table:row:row-0079")
            .is_none(),
        "far center row should start outside the rendered virtual window"
    );

    cx.simulate_click(top_row_before.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("end");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_after = cx
        .debug_bounds("table:row-pinning-keyboard-table:row:row-0000")
        .expect("top pinned row should remain mounted after keyboard navigation");
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "keyboard navigation into the center region should not move the top pinned band"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-keyboard-table:row:row-0079")
            .is_some(),
        "End should scroll an unrendered center row into the center virtual window"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_headers_still_sort_after_center_scroll(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new(
                "pinned-sort-runtime-table",
                "Pinned sort table",
                sample_pinned_table_state(),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .on_sort_requested(move |action, _, _| {
                actions.borrow_mut().push(action);
            });

            div()
                .size_full()
                .child(div().w(px(420.0)).h(px(140.0)).child(table))
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-sort-runtime-table:row-center-scroll:row-a")
        .expect("body center lane should expose a horizontal scroll viewport");
    let header_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-sort-runtime-table:header-center-scroll")
        .expect("header center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-160.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:pinned-sort-runtime-table:header:team")
            .is_some(),
        "center header should remain visible after scrolling"
    );
    let score_header = cx
        .debug_bounds("table:pinned-sort-runtime-table:header:score")
        .expect("left pinned header should remain visible after scrolling");
    let status_header = cx
        .debug_bounds("table:pinned-sort-runtime-table:header:status")
        .expect("right pinned header should remain visible after scrolling");

    cx.simulate_click(header_center_viewport.center(), Default::default());
    cx.simulate_click(score_header.center(), Default::default());
    cx.simulate_click(status_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].column_id().as_str(), "team");
    assert_eq!(actions[1].column_id().as_str(), "score");
    assert_eq!(actions[2].column_id().as_str(), "status");
}

#[open_gpui::test]
fn table_runtime_pinned_resize_handles_emit_changes_for_center_and_pinned_columns(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnSizingChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let table = Table::new(
                "pinned-resize-runtime-table",
                "Pinned resize table",
                sample_pinned_table_state()
                    .with_column_sizing(TableColumnSizing::new().with_width("team", ui_px(160.0))),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .column_resize_mode(TableColumnResizeMode::OnEnd)
            .on_column_sizing_change(move |change, _, _| {
                changes.borrow_mut().push(change);
            });

            div()
                .size_full()
                .child(div().w(px(620.0)).h(px(140.0)).child(table))
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let team_handle_bounds = cx
        .debug_bounds("table:pinned-resize-runtime-table:resize:team")
        .expect("center resize handle should remain reachable in split layout");
    let team_handle = point(
        team_handle_bounds.right() - px(1.0),
        team_handle_bounds.center().y,
    );
    let score_handle_bounds = cx
        .debug_bounds("table:pinned-resize-runtime-table:resize:score")
        .expect("pinned resize handle should remain reachable");
    let score_handle = point(
        score_handle_bounds.right() - px(1.0),
        score_handle_bounds.center().y,
    );

    cx.simulate_mouse_down(team_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(team_handle.x + px(4.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());
    cx.simulate_mouse_move(
        point(team_handle.x + px(24.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());
    cx.simulate_mouse_move(
        point(team_handle.x + px(60.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(team_handle.x + px(60.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(changes.borrow().len(), 1);
    assert_eq!(changes.borrow()[0].column_id().as_str(), "team");

    cx.simulate_mouse_down(score_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(score_handle.x + px(4.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(changes.borrow().len(), 1);
    cx.simulate_mouse_move(
        point(score_handle.x + px(24.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(changes.borrow().len(), 1);
    cx.simulate_mouse_move(
        point(score_handle.x + px(60.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(score_handle.x + px(60.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].column_id().as_str(), "team");
    assert!(changes[0].width() > ui_px(160.0));
    assert_eq!(changes[1].column_id().as_str(), "score");
    assert!(changes[1].width() > ui_px(128.0));
}

#[open_gpui::test]
fn virtualized_list_runtime_reveals_active_row_and_emits_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        activations: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let items = (0..100).map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("item-{index:04}"),
                    format!("Item {index:04}"),
                )
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(112.0)).child(
                    VirtualizedList::new("runtime-list", "Runtime list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(4)
                        .overscan(2)
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.index());
                        }),
                ),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("virtualized-list:runtime-list:root")
        .expect("virtualized list root should render as a focusable target");
    cx.simulate_click(root.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("virtualized-list:runtime-list:root"),
        "clicking the VirtualizedList root should focus it for keyboard navigation"
    );

    let row_0 = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0000")
        .expect("row 0 should render before keyboard navigation");
    cx.simulate_click(row_0.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    activations.borrow_mut().clear();

    let row_4_before = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0004")
        .expect("row 4 should be present in the overscan window before PageDown");
    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_4_after = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0004")
        .expect("row 4 should stay rendered after PageDown reveal");
    assert!(
        row_4_after.top() < row_4_before.top(),
        "expected PageDown to scroll the new active row upward; before={row_4_before:?} after={row_4_after:?}"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().as_slice(), &[4]);
}

#[open_gpui::test]
fn scroll_area_default_handle_survives_reconstructed_component_values(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("scroll-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Row {index}"))
            });

            div().size_full().child(
                div().w(px(180.0)).h(px(60.0)).child(
                    ScrollArea::new(
                        "default-runtime-scroll",
                        div().flex().flex_col().children(rows),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let before = cx
        .debug_bounds("scroll-row-2")
        .expect("row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after = cx
        .debug_bounds("scroll-row-2")
        .expect("row should still be rendered after scrolling");

    assert!(
        after.top() < before.top(),
        "expected row bounds to move upward after wheel scrolling; before={before:?} after={after:?}"
    );
}

#[open_gpui::test]
fn scroll_area_reset_key_resets_default_runtime_handle(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        reset_key: String,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("reset-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Row {index}"))
            });

            div().size_full().child(
                div().w(px(180.0)).h(px(60.0)).child(
                    ScrollArea::new(
                        "reset-runtime-scroll",
                        div().flex().flex_col().children(rows),
                    )
                    .vertical()
                    .reset_on_key(self.reset_key.clone()),
                ),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView {
        reset_key: "overview".to_string(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let initial = cx
        .debug_bounds("reset-row-2")
        .expect("row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let scrolled = cx
        .debug_bounds("reset-row-2")
        .expect("row should still be rendered after scrolling");
    assert!(
        scrolled.top() < initial.top(),
        "expected row bounds to move upward after wheel scrolling; initial={initial:?} scrolled={scrolled:?}"
    );

    view.update(cx, |view, cx| {
        view.reset_key = "details".to_string();
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let reset = cx
        .debug_bounds("reset-row-2")
        .expect("row should still be rendered after reset");
    assert_eq!(
        reset.top(),
        initial.top(),
        "expected reset key change to restore the scroll origin; initial={initial:?} reset={reset:?}"
    );
}

#[open_gpui::test]
fn scroll_area_runtime_scrolls_horizontal_and_two_axis_content(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let horizontal_cells = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("horizontal-cell-{index}"))
                    .w(px(96.0))
                    .h(px(40.0))
                    .flex_none()
                    .child(format!("Column {index}"))
            });
            let grid_rows = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("grid-row-{index}"))
                    .w(px(520.0))
                    .h(px(36.0))
                    .flex_none()
                    .child(format!("Grid row {index}"))
            });

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div().w(px(180.0)).h(px(64.0)).child(
                        ScrollArea::new(
                            "horizontal-runtime-scroll",
                            div()
                                .flex()
                                .gap_2()
                                .min_w(px(820.0))
                                .children(horizontal_cells),
                        )
                        .horizontal(),
                    ),
                )
                .child(
                    div().w(px(180.0)).h(px(70.0)).child(
                        ScrollArea::new(
                            "two-axis-runtime-scroll",
                            div().flex().flex_col().min_w(px(520.0)).children(grid_rows),
                        )
                        .both(),
                    ),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let horizontal_before = cx
        .debug_bounds("horizontal-cell-2")
        .expect("horizontal cell should be rendered before scrolling");
    let grid_before_x = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(24.0)),
        delta: ScrollDelta::Pixels(point(px(-70.0), px(0.0))),
        ..Default::default()
    });
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(108.0)),
        delta: ScrollDelta::Pixels(point(px(-60.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let horizontal_after = cx
        .debug_bounds("horizontal-cell-2")
        .expect("horizontal cell should remain rendered after scrolling");
    let grid_after_x = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should remain rendered after scrolling");

    assert!(
        horizontal_after.left() < horizontal_before.left(),
        "expected horizontal content to move left after wheel scrolling; before={horizontal_before:?} after={horizontal_after:?}"
    );
    assert!(
        grid_after_x.left() < grid_before_x.left(),
        "expected two-axis content to move left after horizontal wheel scrolling; before={grid_before_x:?} after={grid_after_x:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(108.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-42.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let grid_after_y = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should remain rendered after vertical scrolling");
    assert!(
        grid_after_y.top() < grid_after_x.top(),
        "expected two-axis content to move up after vertical wheel scrolling; before={grid_after_x:?} after={grid_after_y:?}"
    );
}

#[open_gpui::test]
fn table_runtime_virtualized_body_scrolls_without_moving_parent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new("runtime-table", "Runtime table", sample_table_state(80))
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(120.0))
                .overscan(2);

            div().size_full().child(
                div().w(px(360.0)).h(px(220.0)).child(
                    ScrollArea::new(
                        "table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-wrapper".into())
                                    .h(px(132.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0000")
            .is_some(),
        "expected first table row to render before scrolling"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0010")
            .is_none(),
        "expected row 10 to stay outside the initial overscan window"
    );
    let parent_bottom_before = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should be rendered before table scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:table:runtime-table:body-scroll")
        .expect("table body viewport should expose a stable scroll selector");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should still be rendered after table scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside Table to stay inside the table body; before={parent_bottom_before:?} after={parent_bottom_after:?}"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0000")
            .is_none(),
        "expected row 0 to unmount after the virtual window advances"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0010")
            .is_some(),
        "expected row 10 to render after scrolling the table body"
    );
}

#[open_gpui::test]
fn table_runtime_cache_invalidates_when_table_state_changes(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        descending: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut state = sample_table_state(20);
            if self.descending {
                state = state.with_sorting([TableSort::descending("score")]);
            }

            let table = Table::new("cache-runtime-table", "Cache runtime table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .overscan(0);

            div().w(px(360.0)).h(px(140.0)).child(table)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { descending: false });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0000")
            .is_some(),
        "expected unsorted table to render row 0 first"
    );
    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0019")
            .is_none(),
        "expected last row to stay outside the initial unsorted window"
    );

    view.update(cx, |view, cx| {
        view.descending = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0019")
            .is_some(),
        "expected cache invalidation to expose the descending first row"
    );
    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0000")
            .is_none(),
        "expected stale unsorted row window to be replaced"
    );
}

#[open_gpui::test]
fn table_runtime_content_fit_widths_follow_visible_content(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        long_value: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let status_value = if self.long_value {
                "Ready for release rollout"
            } else {
                "Ready"
            };
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("status", status_value)])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(140.0)),
                TableColumn::new("status", "Status").with_content_fit(),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("content-fit-runtime-table", "Content fit runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0));

            div().w(px(360.0)).h(px(140.0)).child(table)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { long_value: false });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let status_header_before = cx
        .debug_bounds("table:content-fit-runtime-table:header:status")
        .expect("status header should render before content growth");
    let status_cell_before = cx
        .debug_bounds("table:content-fit-runtime-table:cell:row-a:status")
        .expect("status cell should render before content growth");
    assert_eq!(status_header_before.left(), status_cell_before.left());
    assert_eq!(status_header_before.right(), status_cell_before.right());

    view.update(cx, |view, cx| {
        view.long_value = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let status_header_after = cx
        .debug_bounds("table:content-fit-runtime-table:header:status")
        .expect("status header should still render after content growth");
    let status_cell_after = cx
        .debug_bounds("table:content-fit-runtime-table:cell:row-a:status")
        .expect("status cell should still render after content growth");
    assert_eq!(status_header_after.left(), status_cell_after.left());
    assert_eq!(status_header_after.right(), status_cell_after.right());
    assert!(
        (status_header_after.right() - status_header_after.left())
            > (status_header_before.right() - status_header_before.left()),
        "expected the content-fit column to widen when a longer visible value appears"
    );
    assert_eq!(
        cx.debug_bounds("table:content-fit-runtime-table:cell:row-a:name")
            .expect("fixed-width name cell should stay rendered")
            .right()
            - cx.debug_bounds("table:content-fit-runtime-table:cell:row-a:name")
                .expect("fixed-width name cell should stay rendered")
                .left(),
        px(140.0)
    );
}

#[open_gpui::test]
fn scroll_area_nested_scroll_keeps_parent_static(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let queue_lanes = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("nested-lane-{index}"))
                    .w(px(128.0))
                    .h(px(32.0))
                    .flex_none()
                    .child(format!("Lane {index}"))
            });
            let outer_rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("nested-outer-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Outer row {index}"))
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(120.0)).child(
                    ScrollArea::new(
                        "nested-outer-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .debug_selector(|| "nested-outer-header".into())
                                    .h(px(24.0))
                                    .w_full()
                                    .child("Outer header"),
                            )
                            .child(
                                div().h(px(52.0)).min_h(px(0.0)).overflow_hidden().child(
                                    ScrollArea::new(
                                        "nested-inner-scroll",
                                        div()
                                            .flex()
                                            .gap_2()
                                            .min_w(px(1024.0))
                                            .children(queue_lanes),
                                    )
                                    .horizontal()
                                    .with_size(Size::Small),
                                ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "nested-outer-bottom".into())
                                    .h(px(24.0))
                                    .w_full()
                                    .child("Outer bottom marker"),
                            )
                            .child(div().flex().flex_col().gap_1().children(outer_rows)),
                    )
                    .vertical()
                    .with_size(Size::Small),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let lane_before = cx
        .debug_bounds("nested-lane-2")
        .expect("inner lane should be rendered before scrolling");
    let outer_before = cx
        .debug_bounds("nested-outer-bottom")
        .expect("outer marker should be rendered before scrolling");
    let inner_viewport = cx
        .debug_bounds("scroll-area:nested-inner-scroll")
        .expect("inner scroll viewport should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: inner_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-48.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let lane_after = cx
        .debug_bounds("nested-lane-2")
        .expect("inner lane should remain rendered after scrolling");
    let outer_after = cx
        .debug_bounds("nested-outer-bottom")
        .expect("outer marker should remain rendered after scrolling");

    assert!(
        lane_after.left() < lane_before.left(),
        "expected nested horizontal ScrollArea to move after wheel scrolling; before={lane_before:?} after={lane_after:?}"
    );
    assert_eq!(
        outer_after.top(),
        outer_before.top(),
        "expected wheel scrolling inside the nested ScrollArea to leave the parent viewport in place; before={outer_before:?} after={outer_after:?}"
    );
}

#[open_gpui::test]
fn tabs_vertical_tablist_scrolls_when_constrained(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let tabs = (0..12).fold(
                Tabs::new("overflow-tabs")
                    .orientation(Orientation::Vertical)
                    .small()
                    .default_selected("tab-0"),
                |tabs, index| {
                    tabs.item(TabsItem::new(
                        format!("tab-{index}"),
                        format!("Tab {index}"),
                        div().child(format!("Panel {index}")),
                    ))
                },
            );

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(120.0)).child(tabs))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_before = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should be rendered before scrolling");
    let tablist = cx
        .debug_bounds("tabs:overflow-tabs:tablist")
        .expect("tablist should be rendered");
    let tablist_viewport = cx
        .debug_bounds("scroll-area:tabs:overflow-tabs:tablist-scroll")
        .expect("vertical tablist should use the shared ScrollArea viewport");

    assert!(
        tablist.contains(&tablist_viewport.center()),
        "expected ScrollArea viewport to stay inside the tablist shell; tablist={tablist:?} viewport={tablist_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: tablist_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-64.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_after = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should remain rendered after scrolling");

    assert!(
        tab_after.top() < tab_before.top(),
        "expected constrained vertical tablist to scroll; before={tab_before:?} after={tab_after:?}"
    );
}

#[open_gpui::test]
fn tabs_runtime_manual_keyboard_activation_preserves_selected_seed_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<TabsSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Tabs::new("runtime-tabs")
                    .activation_mode(TabsActivationMode::Manual)
                    .default_selected("details")
                    .item(TabsItem::new(
                        "overview",
                        "Overview",
                        div()
                            .debug_selector(|| "tabs-panel:overview".to_string())
                            .child("Overview panel"),
                    ))
                    .item(
                        TabsItem::new(
                            "billing",
                            "Billing",
                            div()
                                .debug_selector(|| "tabs-panel:billing".to_string())
                                .child("Billing panel"),
                        )
                        .disabled(true),
                    )
                    .item(TabsItem::new(
                        "details",
                        "Details",
                        div()
                            .debug_selector(|| "tabs-panel:details".to_string())
                            .child("Details panel"),
                    ))
                    .item(TabsItem::new(
                        "history",
                        "History",
                        div()
                            .debug_selector(|| "tabs-panel:history".to_string())
                            .child("History panel"),
                    ))
                    .on_selection_change(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "expected seeded selected tab to render the Details panel"
    );

    let disabled_billing = cx
        .debug_bounds("tabs:runtime-tabs:trigger:billing")
        .expect("disabled Billing tab trigger should be rendered");
    cx.simulate_click(disabled_billing.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled tab click should not emit a selection change"
    );
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "disabled tab click should keep the current selected panel"
    );

    let overview = cx
        .debug_bounds("tabs:runtime-tabs:trigger:overview")
        .expect("Overview tab trigger should be rendered");
    cx.simulate_click(overview.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 0);
    assert_eq!(after_click[0].value(), "overview");
    assert_eq!(after_click[0].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "enabled tab click should render the selected panel"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().len(),
        1,
        "manual activation should move roving focus without selecting on arrow key"
    );
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "manual activation should keep the selected panel until Enter or Space"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = selections.borrow().clone();
    assert_eq!(after_enter.len(), 2);
    assert_eq!(after_enter[1].index(), 2);
    assert_eq!(after_enter[1].value(), "details");
    assert_eq!(after_enter[1].label(), "Details");
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "Enter should activate the focused tab after keyboard navigation skips disabled tabs"
    );

    cx.simulate_keystrokes("home enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 3);
    assert_eq!(after_home[2].index(), 0);
    assert_eq!(after_home[2].value(), "overview");
    assert_eq!(after_home[2].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "Home plus Enter should activate the first enabled tab in manual mode"
    );

    cx.simulate_keystrokes("end space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(after_space.len(), 4);
    assert_eq!(after_space[3].index(), 3);
    assert_eq!(after_space[3].value(), "history");
    assert_eq!(after_space[3].label(), "History");
    assert!(
        cx.debug_bounds("tabs-panel:history").is_some(),
        "End plus Space should activate the last enabled tab in manual mode"
    );
}

#[open_gpui::test]
fn toolbar_runtime_keyboard_navigation_skips_disabled_and_separator_items(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<ToolbarSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Toolbar::new("keyboard-toolbar", "Keyboard toolbar")
                    .small()
                    .default_focused("bold")
                    .item(ToolbarItem::icon("undo", "U", "Undo"))
                    .item(ToolbarItem::icon("redo", "R", "Redo").disabled(true))
                    .item(ToolbarItem::separator("history-separator"))
                    .item(ToolbarItem::toggle_icon("bold", "B", "Bold").pressed(true))
                    .item(ToolbarItem::toggle_icon("italic", "I", "Italic"))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let undo = cx
        .debug_bounds("toolbar:keyboard-toolbar:item:undo")
        .expect("undo toolbar item should be rendered");
    cx.simulate_click(undo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("right enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 2);
    assert_eq!(after_right[0].value(), "undo");
    assert_eq!(after_right[0].kind(), ToolbarItemKind::Action);
    assert_eq!(after_right[1].value(), "bold");
    assert_eq!(after_right[1].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("right enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_second_right = selections.borrow().clone();
    assert_eq!(after_second_right.len(), 3);
    assert_eq!(after_second_right[2].value(), "italic");
    assert_eq!(after_second_right[2].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("home enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 4);
    assert_eq!(after_home[3].value(), "undo");
    assert_eq!(after_home[3].kind(), ToolbarItemKind::Action);
}

#[open_gpui::test]
fn splitter_runtime_drag_resizes_horizontal_and_vertical_panels(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let horizontal = Splitter::new("horizontal-drag-split")
                .horizontal()
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("left", 0.5).min_fraction(0.2),
                    div(),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("right", 0.5).min_fraction(0.2),
                    div(),
                ));
            let vertical = Splitter::new("vertical-drag-split")
                .vertical()
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("top", 0.5).min_fraction(0.2),
                    div(),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("bottom", 0.5).min_fraction(0.2),
                    div(),
                ));

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(div().w(px(400.0)).h(px(120.0)).child(horizontal))
                .child(div().w(px(240.0)).h(px(360.0)).child(vertical))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left_before = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should be rendered");
    let right_before = cx
        .debug_bounds("splitter-panel:right")
        .expect("right panel should be rendered");
    let horizontal_handle = cx
        .debug_bounds("splitter:horizontal-drag-split:handle:0")
        .expect("horizontal handle should be rendered")
        .center();
    let top_before = cx
        .debug_bounds("splitter-panel:top")
        .expect("top panel should be rendered");
    let bottom_before = cx
        .debug_bounds("splitter-panel:bottom")
        .expect("bottom panel should be rendered");
    let vertical_handle = cx
        .debug_bounds("splitter:vertical-drag-split:handle:0")
        .expect("vertical handle should be rendered")
        .center();

    cx.simulate_mouse_down(horizontal_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(4.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(24.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(80.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(horizontal_handle.x + px(80.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_down(vertical_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(4.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(24.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(72.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(vertical_handle.x, vertical_handle.y + px(72.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left_after = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should remain rendered");
    let right_after = cx
        .debug_bounds("splitter-panel:right")
        .expect("right panel should remain rendered");
    let top_after = cx
        .debug_bounds("splitter-panel:top")
        .expect("top panel should remain rendered");
    let bottom_after = cx
        .debug_bounds("splitter-panel:bottom")
        .expect("bottom panel should remain rendered");

    assert!(
        left_after.size.width > left_before.size.width
            && right_after.size.width < right_before.size.width,
        "expected horizontal drag to grow the first panel and shrink the second; before=({left_before:?}, {right_before:?}) after=({left_after:?}, {right_after:?})"
    );
    assert!(
        top_after.size.height > top_before.size.height
            && bottom_after.size.height < bottom_before.size.height,
        "expected vertical drag to grow the first panel and shrink the second; before=({top_before:?}, {bottom_before:?}) after=({top_after:?}, {bottom_after:?})"
    );
}

#[test]
fn splitter_state_normalizes_panel_fractions_and_constraints() {
    let state = SplitterState::resolve(
        "workspace",
        Orientation::Horizontal,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("nav", 0.2)
                .min_fraction(0.18)
                .max_fraction(0.32),
            SplitterPanelDescriptor::new("main", 0.65)
                .min_fraction(0.42)
                .max_fraction(0.7),
            SplitterPanelDescriptor::new("inspector", 0.35)
                .min_fraction(0.12)
                .max_fraction(0.28),
        ],
    );

    let sum: f32 = state.panels().iter().map(|panel| panel.fraction()).sum();
    assert_eq!(state.group_id(), "workspace");
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Medium);
    assert!((sum - 1.0).abs() < 0.001);
    assert_eq!(state.panels().len(), 3);
    assert!(state.panels()[0].fraction() >= 0.18);
    assert!(state.panels()[1].fraction() <= 0.7);
    assert!(state.panels()[2].fraction() <= 0.28);
    assert_eq!(state.handles().len(), 2);
    assert_eq!(state.handles()[0].before_id(), "nav");
    assert_eq!(state.handles()[0].after_id(), "main");
    assert_eq!(state.metrics().handle_hit_size(), ui_px(12.0));
}

#[test]
fn splitter_resize_delta_clamps_to_adjacent_min_max() {
    let state = SplitterState::resolve(
        "editor",
        Orientation::Horizontal,
        Size::Small,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.35)
                .min_fraction(0.2)
                .max_fraction(0.4),
            SplitterPanelDescriptor::new("right", 0.65)
                .min_fraction(0.5)
                .max_fraction(0.8),
        ],
    );
    let grown = state.resized_by(0, 0.3);
    let shrunk = grown.resized_by(0, -0.5);

    assert!((grown.panels()[0].fraction() - 0.4).abs() < 0.001);
    assert!((grown.panels()[1].fraction() - 0.6).abs() < 0.001);
    assert!((shrunk.panels()[0].fraction() - 0.2).abs() < 0.001);
    assert!((shrunk.panels()[1].fraction() - 0.8).abs() < 0.001);
}

#[test]
fn splitter_runtime_fraction_overrides_still_use_resize_constraints() {
    let state = SplitterState::resolve(
        "runtime-editor",
        Orientation::Horizontal,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.3)
                .min_fraction(0.15)
                .max_fraction(0.75),
            SplitterPanelDescriptor::new("right", 0.7)
                .min_fraction(0.25)
                .max_fraction(0.85),
        ],
    );

    let overridden = state.with_panel_fractions(&[0.45, 0.55]);
    let grown = overridden.resized_by(0, 0.5);
    let invalid = overridden.with_panel_fractions(&[0.2]);

    assert!((overridden.panels()[0].fraction() - 0.45).abs() < 0.001);
    assert!((overridden.panels()[1].fraction() - 0.55).abs() < 0.001);
    assert!((grown.panels()[0].fraction() - 0.75).abs() < 0.001);
    assert!((grown.panels()[1].fraction() - 0.25).abs() < 0.001);
    assert_eq!(invalid, overridden);
}

#[test]
fn splitter_collapsed_panel_uses_collapsed_fraction() {
    let state = Splitter::new("collapsed-split")
        .vertical()
        .small()
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("summary", 0.3)
                .min_fraction(0.2)
                .collapsible(true)
                .collapsed(true)
                .collapsed_fraction(0.05),
            div(),
        ))
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("details", 0.7).min_fraction(0.4),
            div(),
        ))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert!(state.panels()[0].collapsible());
    assert!(state.panels()[0].collapsed());
    assert!((state.panels()[0].fraction() - 0.05).abs() < 0.001);
    assert_eq!(state.panels()[0].collapsed_fraction(), 0.05);
    assert_eq!(state.handles().len(), 1);
    assert!(!state.handles()[0].disabled());

    let unchanged = state.resized_by(0, 0.1);
    let restored = state.resized_by(0, 0.16);
    let runtime_restored = state.with_panel_fractions(&[0.22, 0.78]);

    assert_eq!(unchanged, state);
    assert!(!restored.panels()[0].collapsed());
    assert!(restored.panels()[0].fraction() >= 0.2);
    assert!(!runtime_restored.panels()[0].collapsed());
    assert!((runtime_restored.panels()[0].fraction() - 0.22).abs() < 0.001);
}

#[open_gpui::test]
fn radio_group_runtime_keyboard_navigation_skips_disabled_items_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<RadioSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                RadioGroup::new("runtime-radio")
                    .label("Runtime radio")
                    .orientation(Orientation::Horizontal)
                    .default_selected("personal")
                    .item(RadioItem::new("personal", "Personal"))
                    .item(RadioItem::new("team", "Team").disabled(true))
                    .item(RadioItem::new("enterprise", "Enterprise"))
                    .on_selection_change(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("radio-group:runtime-radio").is_some(),
        "radio group root should expose a stable debug selector"
    );

    let disabled_team = cx
        .debug_bounds("radio-group:runtime-radio:item:team")
        .expect("disabled Team radio item should be rendered");
    cx.simulate_click(disabled_team.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled radio click should not emit a selection change"
    );

    let enterprise = cx
        .debug_bounds("radio-group:runtime-radio:item:enterprise")
        .expect("Enterprise radio item should be rendered");
    cx.simulate_click(enterprise.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 2);
    assert_eq!(after_click[0].value(), "enterprise");
    assert_eq!(after_click[0].label(), "Enterprise");

    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_left = selections.borrow().clone();
    assert_eq!(after_left.len(), 2);
    assert_eq!(after_left[1].index(), 0);
    assert_eq!(after_left[1].value(), "personal");
    assert_eq!(after_left[1].label(), "Personal");

    cx.simulate_keystrokes("space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(
        after_space.len(),
        2,
        "Space on the already selected radio should not emit a duplicate selection change"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 3);
    assert_eq!(after_right[2].index(), 2);
    assert_eq!(after_right[2].value(), "enterprise");
    assert_eq!(after_right[2].label(), "Enterprise");
}

#[test]
fn radio_group_state_exposes_selection_required_and_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Vertical,
        Size::Medium,
        false,
        true,
        Some("team"),
        None,
        [
            RadioItemDescriptor::new("personal", "Personal"),
            RadioItemDescriptor::new("team", "Team"),
            RadioItemDescriptor::new("enterprise", "Enterprise").disabled(true),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::RadioGroup);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("team"));
    assert_eq!(state.focused_value(), Some("team"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[0].role(), Role::RadioButton);
}

#[test]
fn radio_group_reuses_roving_focus_helpers_and_skips_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        false,
        Some("missing"),
        Some("enterprise"),
        [
            RadioItemDescriptor::new("starter", "Starter"),
            RadioItemDescriptor::new("pro", "Pro").disabled(true),
            RadioItemDescriptor::new("enterprise", "Enterprise"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("enterprise"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].disabled());
    assert!(!state.items()[1].focused());
}

#[test]
fn radio_group_builder_state_falls_back_to_first_enabled_item() {
    let state = RadioGroup::new("plan")
        .label("Plan")
        .orientation(Orientation::Horizontal)
        .with_size(Size::Large)
        .required(true)
        .default_selected("enterprise")
        .item(RadioItem::new("starter", "Starter"))
        .item(RadioItem::new("pro", "Pro"))
        .item(RadioItem::new("enterprise", "Enterprise").disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Large);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("starter"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn toggle_pressed_state_maps_to_button_role_and_toggled_state() {
    let state = Toggle::new("notifications", "Notifications")
        .variant(ToggleVariant::Outline)
        .pressed(true)
        .small()
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::True);
    assert!(state.pressed());
    assert_eq!(state.variant(), ToggleVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn disabled_toggle_blocks_activation_without_checkbox_semantics() {
    let state = Toggle::new("locked", "Locked")
        .pressed(false)
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::False);
    assert!(!state.pressed());
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn badge_variants_resolve_display_only_token_intents() {
    let default = Badge::new("status", "Live").state();
    let secondary = Badge::new("beta", "Beta")
        .variant(BadgeVariant::Secondary)
        .small()
        .state();
    let destructive = Badge::new("risk", "Risk")
        .variant(BadgeVariant::Destructive)
        .state();
    let outline = Badge::new("neutral", "Neutral")
        .variant(BadgeVariant::Outline)
        .state();

    assert_eq!(default.variant(), BadgeVariant::Default);
    assert!(default.display_only());
    assert_eq!(default.role(), None);
    assert_eq!(default.colors().background().token(), semantic::ACCENT);
    assert_eq!(secondary.size(), Size::Small);
    assert_eq!(
        secondary.colors().background().token(),
        semantic::SURFACE_MUTED
    );
    assert_eq!(
        destructive.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(outline.colors().border().token(), semantic::BORDER);
}

#[test]
fn icon_button_requires_accessible_label_and_reuses_button_primitives() {
    let button = IconButton::new("search", "?", "Search")
        .variant(ButtonVariant::Outline)
        .small();
    let state = button.state();

    assert_eq!(button.accessible_label(), "Search");
    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.metrics().size(), Size::Small.icon_button_size());
    assert_eq!(state.metrics().icon_size(), Size::Small.icon_size());
    assert_eq!(state.colors().border().token(), semantic::BORDER);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn crate_root_and_prelude_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_overlay: root::OverlayResolvedState = root::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let prelude_overlay: prelude::OverlayResolvedState = prelude::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let root_button = root::Button::new("save", "Save");
    let root_alert_dialog = root::AlertDialog::new(
        "delete",
        "Delete",
        "Delete item?",
        "This removes it.",
        "Delete",
    );
    let root_sheet = root::Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content");
    let root_hover_card = root::HoverCard::new("hover-card", "Profile", "Profile details");
    let root_sidebar = root::Sidebar::new("sidebar", "Primary navigation");
    let root_toolbar = root::Toolbar::new("toolbar", "Editor");
    let root_listbox = root::Listbox::new("listbox", "Choices");
    let root_select = root::Select::new("select", "Choice");
    let root_combobox = root::Combobox::new("combobox", "Search");
    let root_command = root::Command::new("command", "Commands");
    let root_command_items = vec![root::CommandItem::new("open", "Open")];
    let root_command_snapshot = root::CommandIndexSnapshot::new("root-v1")
        .mode(root::CommandIndexSnapshotMode::PreRankedFilter)
        .item(root::CommandItemDescriptor::new("open", "Open"));
    let root_command_plan: root::CommandRenderPlan =
        root::Command::new("root-command-plan", "Commands")
            .items(root_command_items)
            .index_snapshot(root_command_snapshot)
            .render_plan();
    let _root_command_row: Option<&root::CommandRowRenderPlan> = root_command_plan.rows().first();
    let root_menu_state = root::Menu::new("root-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(root::MenuItem::submenu(
            "more",
            "More",
            [root::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let root_menu_submenu_navigation: root::MenuSubmenuNavigation = root_menu_state
        .submenu_navigation_target("right")
        .expect("root MenuSubmenuNavigation should be exported");
    let root_scroll = root::ScrollArea::new("scroll", div());
    let root_splitter = root::Splitter::new("split");
    let root_tabs = root::Tabs::new("tabs");
    let root_global_filter = root::TableGlobalFilter::new("global-filter", "Search");
    let root_predicate_filter = root::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let root_table_toolbar =
        root::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let root_faceted_filter = root::TableFacetedFilter::new("status-filter", "Status", "status");
    let root_column_visibility = root::TableColumnVisibility::new("column-visibility", "Columns")
        .columns([root::TableColumn::new("status", "Status")]);
    let root_avatar = root::Avatar::new("avatar", "Ada Lovelace");
    let root_separator = root::Separator::new("separator");
    let root_kbd = root::Kbd::new("kbd", "Ctrl+K");
    let root_progress = root::Progress::new("progress", "Progress");
    let root_skeleton = root::Skeleton::new("skeleton");
    let root_status_cue = root::StatusCue::new("status", "Ready");
    let root_empty_state = root::EmptyState::new("empty", "No results");
    let prelude_button = prelude::Button::new("save", "Save");
    let prelude_alert_dialog = prelude::AlertDialog::new(
        "delete",
        "Delete",
        "Delete item?",
        "This removes it.",
        "Delete",
    );
    let prelude_sheet = prelude::Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content");
    let prelude_hover_card = prelude::HoverCard::new("hover-card", "Profile", "Profile details");
    let prelude_sidebar = prelude::Sidebar::new("sidebar", "Primary navigation");
    let prelude_toolbar = prelude::Toolbar::new("toolbar", "Editor");
    let prelude_listbox = prelude::Listbox::new("listbox", "Choices");
    let prelude_select = prelude::Select::new("select", "Choice");
    let prelude_combobox = prelude::Combobox::new("combobox", "Search");
    let prelude_command = prelude::Command::new("command", "Commands");
    let prelude_command_items = vec![prelude::CommandItem::new("open", "Open")];
    let prelude_command_snapshot = prelude::CommandIndexSnapshot::new("prelude-v1")
        .mode(prelude::CommandIndexSnapshotMode::PreFiltered)
        .item(prelude::CommandItemDescriptor::new("open", "Open"));
    let prelude_command_plan: prelude::CommandRenderPlan =
        prelude::Command::new("prelude-command-plan", "Commands")
            .items(prelude_command_items)
            .index_snapshot(prelude_command_snapshot)
            .render_plan();
    let _prelude_command_row: Option<&prelude::CommandRowRenderPlan> =
        prelude_command_plan.rows().first();
    let prelude_menu_state = prelude::Menu::new("prelude-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(prelude::MenuItem::submenu(
            "more",
            "More",
            [prelude::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let prelude_menu_submenu_navigation: prelude::MenuSubmenuNavigation = prelude_menu_state
        .submenu_navigation_target("right")
        .expect("prelude MenuSubmenuNavigation should be exported");
    let prelude_scroll = prelude::ScrollArea::new("scroll", div());
    let prelude_splitter = prelude::Splitter::new("split");
    let prelude_tabs = prelude::Tabs::new("tabs");
    let prelude_global_filter = prelude::TableGlobalFilter::new("global-filter", "Search");
    let prelude_predicate_filter =
        prelude::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let prelude_table_toolbar =
        prelude::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let prelude_faceted_filter =
        prelude::TableFacetedFilter::new("status-filter", "Status", "status");
    let prelude_column_visibility =
        prelude::TableColumnVisibility::new("column-visibility", "Columns")
            .columns([prelude::TableColumn::new("status", "Status")]);
    let prelude_avatar = prelude::Avatar::new("avatar", "Ada Lovelace");
    let prelude_separator = prelude::Separator::new("separator");
    let prelude_kbd = prelude::Kbd::new("kbd", "Ctrl+K");
    let prelude_progress = prelude::Progress::new("progress", "Progress");
    let prelude_skeleton = prelude::Skeleton::new("skeleton");
    let prelude_status_cue = prelude::StatusCue::new("status", "Ready");
    let prelude_empty_state = prelude::EmptyState::new("empty", "No results");

    let _ = (
        root_button.state(),
        root_alert_dialog.state(),
        root_sheet.state(),
        root_hover_card.state(),
        root_sidebar.state(),
        root_toolbar.state(),
        root_listbox.state(),
        root_select.state(),
        root_combobox.state(),
        root_command.state(),
        root_command_plan.role(),
        root_menu_submenu_navigation.focused_value(),
        root_scroll.state(),
        root_splitter.state(),
        root_tabs.state(),
        root_global_filter.state(),
        root_predicate_filter.state(),
        root_table_toolbar.state(),
        root_faceted_filter.state(),
        root_column_visibility.state(),
        root_avatar.state(),
        root_separator.state(),
        root_kbd.state(),
        root_progress.state(),
        root_skeleton.state(),
        root_status_cue.state(),
        root_empty_state.state(),
        prelude_button.state(),
        prelude_alert_dialog.state(),
        prelude_sheet.state(),
        prelude_hover_card.state(),
        prelude_sidebar.state(),
        prelude_toolbar.state(),
        prelude_listbox.state(),
        prelude_select.state(),
        prelude_combobox.state(),
        prelude_command.state(),
        prelude_command_plan.row_role(),
        prelude_menu_submenu_navigation.focused_value(),
        prelude_scroll.state(),
        prelude_splitter.state(),
        prelude_tabs.state(),
        prelude_global_filter.state(),
        prelude_predicate_filter.state(),
        prelude_table_toolbar.state(),
        prelude_faceted_filter.state(),
        prelude_column_visibility.state(),
        prelude_avatar.state(),
        prelude_separator.state(),
        prelude_kbd.state(),
        prelude_progress.state(),
        prelude_skeleton.state(),
        prelude_status_cue.state(),
        prelude_empty_state.state(),
        root_overlay.policy().kind(),
        prelude_overlay.policy().kind(),
    );
}

#[test]
fn gpui_role_mapping_covers_neutral_image_and_separator_fallback() {
    assert_eq!(gpui_role_from_ui(Role::Image), open_gpui::Role::Image);
    assert_eq!(gpui_role_from_ui(Role::Separator), open_gpui::Role::Group);
    assert_eq!(gpui_role_from_ui(Role::Tree), open_gpui::Role::Tree);
    assert_eq!(gpui_role_from_ui(Role::TreeItem), open_gpui::Role::TreeItem);
    assert_eq!(gpui_role_from_ui(Role::Table), open_gpui::Role::Table);
    assert_eq!(gpui_role_from_ui(Role::Row), open_gpui::Role::Row);
    assert_eq!(
        gpui_role_from_ui(Role::ColumnHeader),
        open_gpui::Role::ColumnHeader
    );
    assert_eq!(gpui_role_from_ui(Role::Cell), open_gpui::Role::Cell);
}

fn official_component_catalog_names_from_gallery_source() -> Vec<String> {
    const GALLERY_COMPONENTS_SOURCE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/ui-foundation-gallery/src/pages/components.rs"
    );
    const MARKER: &str = "ComponentCatalogEntry::official(";

    let source = std::fs::read_to_string(GALLERY_COMPONENTS_SOURCE)
        .unwrap_or_else(|error| panic!("failed to read {GALLERY_COMPONENTS_SOURCE}: {error}"));
    let mut remaining = source.as_str();
    let mut names = Vec::new();

    while let Some(marker_index) = remaining.find(MARKER) {
        remaining = &remaining[marker_index + MARKER.len()..];
        let name_start = remaining
            .find('"')
            .unwrap_or_else(|| panic!("missing catalog name opener after {MARKER}"));
        remaining = &remaining[name_start + 1..];
        let name_end = remaining
            .find('"')
            .unwrap_or_else(|| panic!("missing catalog name closer after {MARKER}"));
        names.push(remaining[..name_end].to_string());
        remaining = &remaining[name_end + 1..];
    }

    assert!(
        !names.is_empty(),
        "Components gallery source should contain official catalog entries"
    );
    names
}

fn component_api_entry(component: &str) -> &'static ComponentApiInventoryEntry {
    COMPONENT_API_INVENTORY
        .iter()
        .find(|entry| entry.component == component)
        .unwrap_or_else(|| panic!("missing component API inventory row for `{component}`"))
}

fn assert_inventory_contains_controlled_input(component: &str, input: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry.controlled_inputs.contains(&input),
        "{component} inventory should classify `{input}` as a controlled input"
    );
}

fn assert_inventory_contains_default_seed(component: &str, builder: &str, runtime_value: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry
            .default_seeds
            .iter()
            .any(|seed| seed.builder == builder && seed.runtime_value == runtime_value),
        "{component} inventory should classify `{builder}` as a default seed for `{runtime_value}`"
    );
}

fn assert_inventory_contains_callback(component: &str, name: &str, payload: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry
            .callbacks
            .iter()
            .any(|callback| callback.name == name && callback.payload == payload),
        "{component} inventory should document callback `{name}` payload `{payload}`"
    );
}

#[test]
fn component_api_inventory_covers_official_gallery_catalog() {
    use std::collections::BTreeSet;

    let inventory_names = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component.to_string())
        .collect::<BTreeSet<_>>();
    let catalog_names = official_component_catalog_names_from_gallery_source()
        .into_iter()
        .collect::<BTreeSet<_>>();

    let missing = catalog_names
        .difference(&inventory_names)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "official Components catalog entries need public API inventory rows: {missing:?}"
    );

    for overlay in [
        "Tooltip",
        "HoverCard",
        "Popover",
        "Dialog",
        "AlertDialog",
        "Sheet",
        "Menu",
        "ContextMenu",
    ] {
        assert!(
            inventory_names.contains(overlay),
            "overlay component `{overlay}` needs a public API inventory row"
        );
    }
}

#[test]
fn component_api_inventory_uses_stable_ownership_vocabulary() {
    const CURRENT_CALLBACK_NAMES: &[&str] = &[
        "on_activate",
        "on_action",
        "on_cancel",
        "on_change",
        "on_click",
        "on_close",
        "on_cell_edit_change",
        "on_column_sizing_change",
        "on_move",
        "on_open_change",
        "on_query_change",
        "on_row_activate",
        "on_row_selection_change",
        "on_row_expansion_request",
        "on_select",
        "on_selected_values_change",
        "on_selection_change",
        "on_sort_requested",
        "on_toggle",
    ];
    const CURRENT_LEGACY_SEED_INPUTS: &[(&str, &str)] = &[];

    let mut seen = std::collections::BTreeSet::new();
    for entry in COMPONENT_API_INVENTORY {
        assert!(
            seen.insert(entry.component),
            "component API inventory contains duplicate row for `{}`",
            entry.component
        );
        assert!(
            entry.has_classification(),
            "{} must document at least one API ownership bucket or no-interaction note",
            entry.component
        );
        assert!(
            entry.renderer_neutral_state,
            "{} resolved state must remain renderer-neutral",
            entry.component
        );
        let source_methods = component_public_methods_from_source(entry.component);
        let expected_methods = component_public_methods(entry.component)
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            source_methods, expected_methods,
            "{} public method surface drifted; update COMPONENT_API_INVENTORY and the method baseline together",
            entry.component
        );

        for seed in entry.default_seeds {
            assert!(
                seed.builder.starts_with("default_"),
                "{} default seed `{}` must use default_* naming",
                entry.component,
                seed.builder
            );
            assert!(
                !seed.runtime_value.is_empty(),
                "{} default seed `{}` must name the adapter-owned runtime value it seeds",
                entry.component,
                seed.builder
            );
        }

        for callback in entry.callbacks {
            assert!(
                CURRENT_CALLBACK_NAMES.contains(&callback.name),
                "{} callback `{}` is not part of the current inventory vocabulary",
                entry.component,
                callback.name
            );
            assert!(
                !callback.payload.is_empty(),
                "{} callback `{}` must document its payload type",
                entry.component,
                callback.name
            );
        }

        for legacy_seed in entry.legacy_seed_inputs {
            assert!(
                CURRENT_LEGACY_SEED_INPUTS.contains(&(entry.component, *legacy_seed)),
                "{} legacy seed `{}` needs an explicit migration decision before U2",
                entry.component,
                legacy_seed
            );
        }
    }

    assert_inventory_contains_controlled_input("TextInput", "value");
    assert_inventory_contains_callback("TextInput", "on_change", "String");
    assert_inventory_contains_controlled_input("Textarea", "value");
    assert_inventory_contains_callback("Textarea", "on_change", "String");
    assert_inventory_contains_controlled_input("Switch", "checked");
    assert_inventory_contains_callback("Switch", "on_change", "bool");
    assert_inventory_contains_default_seed("Popover", "default_open", "open");
    assert_inventory_contains_callback("Popover", "on_open_change", "bool");
    assert_inventory_contains_controlled_input("Select", "selected");
    assert_inventory_contains_controlled_input("Select", "active");
    assert_inventory_contains_callback("Select", "on_select", "SelectSelection");
    assert_inventory_contains_default_seed("Combobox", "default_query", "query");
    assert_inventory_contains_controlled_input("Command", "query");
    assert_inventory_contains_controlled_input("Command", "selected_values");
    assert_inventory_contains_controlled_input("Command", "index_snapshot");
    assert_inventory_contains_default_seed("Command", "default_query", "query");
    assert_inventory_contains_callback("Command", "on_query_change", "String");
    assert_inventory_contains_callback(
        "Command",
        "on_selected_values_change",
        "CommandSelectionChange",
    );
    assert_inventory_contains_default_seed("Tabs", "default_selected", "selected");
    assert_inventory_contains_default_seed("RadioGroup", "default_selected", "selected");
    assert_inventory_contains_default_seed("Toolbar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Sidebar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Tree", "default_selected", "selected");
    assert_inventory_contains_default_seed("Tree", "default_focused", "focused");
    assert_inventory_contains_callback("Tree", "on_toggle", "TreeToggle");
    assert_inventory_contains_callback("Tree", "on_move", "TreeMove");
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_active_index",
        "active_index",
    );
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_selected_index",
        "selected_index",
    );
    assert_inventory_contains_callback(
        "VirtualizedList",
        "on_activate",
        "VirtualizedListActivation",
    );
    assert_inventory_contains_default_seed("Menu", "default_focused_value", "focused_value");
    assert_inventory_contains_default_seed("ContextMenu", "default_focused_value", "focused_value");
    assert_inventory_contains_default_seed("Table", "default_focused_row", "focused_row");
    assert_inventory_contains_controlled_input("TableGlobalFilter", "query");
    assert_inventory_contains_default_seed("TableGlobalFilter", "default_query", "query");
    assert_inventory_contains_callback("TableGlobalFilter", "on_change", "TableGlobalFilterChange");
    assert_inventory_contains_controlled_input("TablePredicateFilter", "operator");
    assert_inventory_contains_controlled_input("TablePredicateFilter", "value");
    assert_inventory_contains_default_seed("TablePredicateFilter", "default_operator", "operator");
    assert_inventory_contains_default_seed("TablePredicateFilter", "default_value", "value");
    assert_inventory_contains_callback(
        "TablePredicateFilter",
        "on_change",
        "TablePredicateFilterChange",
    );
    assert_inventory_contains_controlled_input("TableColumnVisibility", "visibility");
    assert_inventory_contains_controlled_input("TableColumnVisibility", "open");
    assert_inventory_contains_default_seed(
        "TableColumnVisibility",
        "default_visibility",
        "visibility",
    );
    assert_inventory_contains_default_seed("TableColumnVisibility", "default_open", "open");
    assert_inventory_contains_callback(
        "TableColumnVisibility",
        "on_change",
        "TableColumnVisibilityChange",
    );
    assert_inventory_contains_callback("Table", "on_row_activate", "TableRowActivation");
    assert_inventory_contains_callback(
        "Table",
        "on_row_selection_change",
        "TableRowSelectionChange",
    );
    assert_inventory_contains_callback(
        "Table",
        "on_row_expansion_request",
        "TableRowExpansionToggle",
    );
}

#[test]
fn public_resolved_state_contracts_avoid_gpui_runtime_types() {
    const FORBIDDEN: &[&str] = &[
        "Window",
        "App",
        "Context<",
        "RenderOnce",
        "IntoElement",
        "ElementId",
        "Entity<",
        "FocusHandle",
        "ScrollHandle",
        "Rc<dyn",
    ];
    let mut source_files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("ui_components src directory should be readable")
        .map(|entry| {
            entry
                .expect("source directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    source_files.sort();

    let mut checked = 0;
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for state in public_contract_structs(&source, &["State"]) {
            checked += 1;
            let fields = uncommented_lines(state.fields);
            for forbidden in FORBIDDEN {
                assert!(
                    !fields.contains(forbidden),
                    "{file_name}::{} leaks forbidden runtime/render type `{forbidden}`",
                    state.name
                );
            }
        }
    }

    assert!(
        checked >= 40,
        "expected to scan all public resolved-state structs, scanned {checked}"
    );
}

#[test]
fn public_contract_extraction_blockers_match_allowlist() {
    const BLOCKER_TOKENS: &[&str] = &["GpuiOverlayState", "open_gpui::Pixels", "Point<Pixels>"];
    let expected: [(&str, &str, &str); 0] = [];
    let mut expected = expected
        .into_iter()
        .map(|(file, contract, token)| {
            PublicContractBlocker::new(file.to_owned(), contract.to_owned(), token.to_owned())
        })
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = public_contract_extraction_blockers(BLOCKER_TOKENS);
    actual.sort();

    assert_eq!(
        actual, expected,
        "public component contracts gained or removed extraction blockers; update this inventory as U2-U6 migrate them"
    );
}

#[test]
fn adapter_only_public_surfaces_match_allowlist() {
    let expected = [
        ("focus.rs", "BoxShadow"),
        ("focus.rs", "focus_ring_shadow"),
        ("overlay.rs", "GpuiOverlayState"),
        ("scroll_area.rs", "ScrollHandle"),
        ("text_input.rs", "Entity<TextInputController>"),
        ("text_input.rs", "EntityInputHandler"),
        ("text_input.rs", "TextInputController"),
        ("textarea.rs", "EntityInputHandler"),
    ];
    let mut expected = expected
        .into_iter()
        .map(|(file, token)| PublicSurfaceBlocker::new(file.to_owned(), token.to_owned()))
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = public_surface_blockers(&[
        "BoxShadow",
        "Entity<TextInputController>",
        "EntityInputHandler",
        "GpuiOverlayState",
        "ScrollHandle",
        "TextInputController",
        "focus_ring_shadow",
    ]);
    actual.sort();

    assert_eq!(
        actual, expected,
        "adapter-only public surfaces changed; update this inventory as U6 classifies or narrows GPUI-specific APIs"
    );
}

#[test]
fn gpui_adapter_exports_group_runtime_specific_surfaces() {
    use open_gpui_ui_components::{self as root, prelude};

    let module_text_input = root::text_input::TextInput::new("module-text-input", "Module input");
    let _module_state: root::text_input::TextInputState = module_text_input.state();
    let _module_colors: Option<root::text_input::TextInputColors> = None;
    let _module_metrics: Option<root::text_input::TextInputMetrics> = None;
    let _module_display_mode: root::text_input::TextInputDisplayMode =
        root::text_input::TextInputDisplayMode::Plain;
    let module_textarea = root::textarea::Textarea::new("module-textarea", "Module textarea");
    let _module_textarea_state: root::textarea::TextareaState = module_textarea.state();
    let _module_textarea_colors: Option<root::textarea::TextareaColors> = None;
    let _module_textarea_metrics: Option<root::textarea::TextareaMetrics> = None;

    let root_overlay = root::gpui_adapter::GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::Tooltip,
        OverlayPresence::open(),
    )
    .state();

    let _root_init: fn(&mut open_gpui::App) = root::gpui_adapter::init_text_input;
    let _root_controller: Option<root::gpui_adapter::TextInputController> = None;
    let _root_px: fn(UiPx) -> open_gpui::Pixels = root::gpui_adapter::gpui_px_from_ui;
    let _root_point: fn(UiPoint) -> open_gpui::Point<open_gpui::Pixels> =
        root::gpui_adapter::gpui_point_from_ui;
    let _root_size: fn(UiSize) -> open_gpui::Size<open_gpui::Pixels> =
        root::gpui_adapter::gpui_size_from_ui;
    let _prelude_button: prelude::Button = prelude::Button::new("save", "Save");
    let _prelude_textarea: prelude::Textarea = prelude::Textarea::new("notes", "Notes");
    let _prelude_display_mode: prelude::TextInputDisplayMode = prelude::TextInputDisplayMode::Plain;

    assert_eq!(
        root_overlay.deferred_priority(),
        root::gpui_adapter::default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(
        root_overlay.snap_margin(),
        root::gpui_adapter::DEFAULT_OVERLAY_SAFE_MARGIN
    );
    assert_eq!(
        root::gpui_adapter::focus_ring_shadow(FocusRing::from_color(ColorIntent::new(
            semantic::FOCUS_RING,
            0x2f80ed,
        )))[0]
            .spread_radius,
        px(2.0)
    );
}

#[test]
fn adapter_only_helpers_do_not_leak_from_default_exports() {
    let adapter_only_tokens = [
        "TextInputController",
        "init_text_input",
        "focus_ring_shadow",
        "GpuiOverlayState",
        "GpuiOverlayAdapterConfig",
        "gpui_px_from_ui",
    ];

    for file_name in ["lib.rs", "prelude.rs"] {
        let source =
            std::fs::read_to_string(format!("{}/src/{file_name}", env!("CARGO_MANIFEST_DIR")))
                .unwrap_or_else(|error| panic!("failed to read {file_name}: {error}"));
        let default_interface = if file_name == "lib.rs" {
            source_without_gpui_adapter_module(&source)
        } else {
            source
        };

        for token in adapter_only_tokens {
            assert!(
                !default_interface.contains(token),
                "{file_name} default interface must not expose adapter-only token `{token}`"
            );
        }
    }

    let text_input_source =
        std::fs::read_to_string(format!("{}/src/text_input.rs", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|error| panic!("failed to read text_input.rs: {error}"));
    assert!(text_input_source.contains("pub(crate) mod adapter"));
    assert!(
        !text_input_source.contains("pub use adapter"),
        "text_input must not re-export its internal adapter module"
    );
}

#[test]
fn public_reexports_stay_explicit_without_wildcards() {
    let mut wildcard_exports = Vec::new();
    for file_name in ["lib.rs", "prelude.rs"] {
        let source =
            std::fs::read_to_string(format!("{}/src/{file_name}", env!("CARGO_MANIFEST_DIR")))
                .unwrap_or_else(|error| panic!("failed to read {file_name}: {error}"));

        for (line_number, line) in source.lines().enumerate() {
            if line.contains("pub use ") && line.contains("::*") {
                wildcard_exports.push(format!("{file_name}:{}", line_number + 1));
            }
        }
    }

    assert_eq!(
        wildcard_exports,
        Vec::<String>::new(),
        "public re-exports must stay explicit, including adapter-only groupings"
    );
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PublicContractBlocker {
    file: String,
    contract: String,
    token: String,
}

impl PublicContractBlocker {
    fn new(file: String, contract: String, token: String) -> Self {
        Self {
            file,
            contract,
            token,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PublicSurfaceBlocker {
    file: String,
    token: String,
}

impl PublicSurfaceBlocker {
    fn new(file: String, token: String) -> Self {
        Self { file, token }
    }
}

struct PublicContractStruct<'a> {
    name: &'a str,
    fields: &'a str,
}

fn public_contract_structs<'a>(
    source: &'a str,
    suffixes: &[&str],
) -> Vec<PublicContractStruct<'a>> {
    let mut states = Vec::new();
    let mut search_from = 0;

    while let Some(relative_start) = source[search_from..].find("pub struct ") {
        let start = search_from + relative_start;
        let name_start = start + "pub struct ".len();
        let name_end = source[name_start..]
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = &source[name_start..name_end];

        search_from = name_end;
        if !suffixes.iter().any(|suffix| name.ends_with(suffix)) {
            continue;
        }
        if ["EmptyState"].contains(&name) {
            continue;
        }

        let Some(open_brace) = source[name_end..].find('{').map(|offset| name_end + offset) else {
            continue;
        };
        let Some(close_brace) = matching_brace(source, open_brace) else {
            continue;
        };

        states.push(PublicContractStruct {
            name,
            fields: &source[open_brace + 1..close_brace],
        });
        search_from = close_brace + 1;
    }

    states
}

fn public_contract_extraction_blockers(tokens: &[&str]) -> Vec<PublicContractBlocker> {
    let mut source_files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("ui_components src directory should be readable")
        .map(|entry| {
            entry
                .expect("source directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    source_files.sort();

    let mut blockers = Vec::new();
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for contract in public_contract_structs(&source, &["State", "Metrics"]) {
            let fields = uncommented_lines(contract.fields);
            for token in tokens {
                if fields.contains(token) {
                    blockers.push(PublicContractBlocker::new(
                        file_name.to_owned(),
                        contract.name.to_owned(),
                        (*token).to_owned(),
                    ));
                }
            }
        }
    }

    blockers
}

fn public_surface_blockers(tokens: &[&str]) -> Vec<PublicSurfaceBlocker> {
    let mut source_files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("ui_components src directory should be readable")
        .map(|entry| {
            entry
                .expect("source directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    source_files.sort();

    let mut blockers = Vec::new();
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        let source = if matches!(file_name, "lib.rs" | "prelude.rs") {
            source_without_gpui_adapter_module(&source)
        } else {
            source
        };
        let surface = public_api_surface(&uncommented_lines(&source));

        for token in tokens {
            if surface.contains(token) {
                blockers.push(PublicSurfaceBlocker::new(
                    file_name.to_owned(),
                    (*token).to_owned(),
                ));
            }
        }
    }

    blockers
}

fn source_without_gpui_adapter_module(source: &str) -> String {
    let Some(module_start) = source.find("pub mod gpui_adapter") else {
        return source.to_owned();
    };
    let Some(open_brace) = source[module_start..]
        .find('{')
        .map(|offset| module_start + offset)
    else {
        return source.to_owned();
    };
    let Some(close_brace) = matching_brace(source, open_brace) else {
        return source.to_owned();
    };

    let mut stripped = String::with_capacity(source.len());
    stripped.push_str(&source[..module_start]);
    stripped.push_str(&source[close_brace + 1..]);
    stripped
}

fn public_api_surface(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let mut surface = Vec::new();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let line = lines[line_index];
        let trimmed = line.trim_start();

        if trimmed.starts_with("pub use ") {
            while line_index < lines.len() {
                let signature_line = lines[line_index];
                surface.push(signature_line);
                line_index += 1;
                if signature_line.contains(';') {
                    break;
                }
            }
            continue;
        }

        if trimmed.starts_with("pub fn ") {
            while line_index < lines.len() {
                let signature_line = lines[line_index];
                surface.push(signature_line);
                line_index += 1;
                if signature_line.contains('{') || signature_line.contains(';') {
                    break;
                }
            }
            continue;
        }

        if trimmed.starts_with("pub const ")
            || trimmed.starts_with("pub type ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("impl EntityInputHandler for ")
        {
            surface.push(line);
            line_index += 1;
            continue;
        }

        line_index += 1;
    }

    surface.join("\n")
}

fn matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_brace + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn uncommented_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn sidebar_state_exposes_shell_navigation_contract() {
    let state = SidebarState::resolve(
        SidebarSide::Left,
        SidebarVariant::Docked,
        SidebarCollapseMode::Icon,
        false,
        false,
        "Primary navigation",
        Some("projects"),
        None,
        [
            SidebarSectionDescriptor::new("workspace", "Workspace").items([
                SidebarItemDescriptor::new("home", "Home").icon("H"),
                SidebarItemDescriptor::new("projects", "Projects")
                    .icon("P")
                    .badge("12"),
                SidebarItemDescriptor::new("archive", "Archive")
                    .icon("A")
                    .disabled(true),
            ]),
            SidebarSectionDescriptor::new("account", "Account").items([
                SidebarItemDescriptor::new("settings", "Settings").icon("S"),
                SidebarItemDescriptor::new("billing", "Billing")
                    .icon("B")
                    .action_label("new"),
            ]),
        ],
        Size::Medium,
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Navigation);
    assert_eq!(state.side(), SidebarSide::Left);
    assert_eq!(state.variant(), SidebarVariant::Docked);
    assert_eq!(state.collapse_mode(), SidebarCollapseMode::Icon);
    assert!(!state.collapsed());
    assert_eq!(state.sections().len(), 2);
    assert_eq!(state.sections()[0].role(), Role::Section);
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.selected_value(), Some("projects"));
    assert_eq!(state.focused_value(), Some("projects"));
    assert_eq!(state.focused_index(), Some(1));
    assert!(state.scrollable());
    assert!(state.items()[1].selected());
    assert_eq!(state.items()[1].badge_label(), Some("12"));
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[1].role(), Role::Button);
    assert_eq!(state.items()[1].position_in_set(), Some(2));
    assert_eq!(state.items()[1].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("settings")
    );
    assert_eq!(
        state
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("projects".to_string())
    );
}

#[test]
fn sidebar_icon_collapse_keeps_accessible_items_but_hides_text() {
    let state = Sidebar::new("app-sidebar", "Application")
        .collapse_mode(SidebarCollapseMode::Icon)
        .collapsed(true)
        .selected("dashboard")
        .section(
            SidebarSection::new("main", "Main")
                .item(SidebarItem::new("dashboard", "Dashboard").icon("D"))
                .item(SidebarItem::new("inbox", "Inbox").icon("I").badge("4")),
        )
        .state();

    assert!(state.collapsed());
    assert!(state.icon_collapsed());
    assert!(!state.offcanvas_collapsed());
    assert_eq!(
        state.metrics().resolved_width(),
        state.metrics().collapsed_width()
    );
    assert_eq!(state.selected_value(), Some("dashboard"));
    assert_eq!(state.focused_value(), Some("dashboard"));
    assert!(state.scrollable());
    assert!(state.items()[0].focusable());
    assert_eq!(state.items()[0].label(), "Dashboard");
    assert_eq!(state.items()[1].badge_label(), Some("4"));
}

#[test]
fn sidebar_offcanvas_collapse_removes_items_from_roving_focus() {
    let state = SidebarState::resolve(
        SidebarSide::Right,
        SidebarVariant::Floating,
        SidebarCollapseMode::Offcanvas,
        true,
        false,
        "Secondary navigation",
        Some("reports"),
        None,
        [SidebarSectionDescriptor::new("main", "Main").items([
            SidebarItemDescriptor::new("overview", "Overview"),
            SidebarItemDescriptor::new("reports", "Reports"),
        ])],
        Size::Small,
        ThemeTokens::default(),
    );

    assert!(state.collapsed());
    assert!(state.offcanvas_collapsed());
    assert_eq!(state.metrics().resolved_width(), ui_px(0.0));
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.focused_value(), None);
    assert_eq!(state.focused_index(), None);
    assert!(!state.scrollable());
    assert!(!state.items()[0].focusable());
    assert!(state.activation_for_key("space").is_none());
}

#[open_gpui::test]
fn sidebar_long_navigation_scrolls_inside_shared_scroll_area(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let section = (0..14).fold(SidebarSection::new("main", "Main"), |section, index| {
                section.item(
                    SidebarItem::new(format!("item-{index}"), format!("Item {index}"))
                        .icon(index.to_string()),
                )
            });

            div().size_full().child(
                div()
                    .h(px(120.0))
                    .child(Sidebar::new("long-sidebar", "Long navigation").section(section)),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_before = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should be rendered before scrolling");
    let sidebar = cx
        .debug_bounds("sidebar:long-sidebar")
        .expect("sidebar shell should be rendered");
    let sidebar_viewport = cx
        .debug_bounds("scroll-area:long-sidebar-scroll")
        .expect("long Sidebar should use the shared ScrollArea viewport");

    assert!(
        sidebar.contains(&sidebar_viewport.center()),
        "expected Sidebar ScrollArea viewport to stay inside the sidebar shell; sidebar={sidebar:?} viewport={sidebar_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_after = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should remain rendered after scrolling");

    assert!(
        item_after.top() < item_before.top(),
        "expected long Sidebar navigation to scroll inside its ScrollArea; before={item_before:?} after={item_after:?}"
    );
}

#[test]
fn sidebar_navigation_helper_skips_disabled_items() {
    assert_eq!(
        sidebar_navigation_target("down", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("up", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("home", 2, &[false, true, false]),
        Some(0)
    );
    assert_eq!(sidebar_navigation_target("right", 0, &[false, false]), None);
}

#[test]
fn toolbar_state_exposes_roving_focus_and_toggle_metadata() {
    let state = ToolbarState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        "Editor toolbar",
        Some("bold"),
        [
            ToolbarItemDescriptor::action("undo", "Undo"),
            ToolbarItemDescriptor::separator("history-separator"),
            ToolbarItemDescriptor::toggle("bold", "Bold").pressed(true),
            ToolbarItemDescriptor::toggle("italic", "Italic").disabled(true),
            ToolbarItemDescriptor::action("save", "Save"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.label(), "Editor toolbar");
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.focused_value(), Some("bold"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items()[0].role(), Some(Role::Button));
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert_eq!(state.items()[1].role(), None);
    assert!(!state.items()[1].focusable());
    assert!(state.items()[2].pressed());
    assert_eq!(state.items()[2].toggled(), Some(Toggled::True));
    assert!(!state.items()[3].activation_enabled());
    assert_eq!(
        state.navigation_target("right").map(|item| item.value()),
        Some("save")
    );
    assert_eq!(
        state
            .activation_for_key("space")
            .map(|selection| (selection.value().to_owned(), selection.kind())),
        Some(("bold".to_string(), ToolbarItemKind::Toggle))
    );
}

#[test]
fn toolbar_builder_state_skips_disabled_and_separator_items() {
    let state = Toolbar::new("editor-tools", "Editor")
        .orientation(Orientation::Vertical)
        .large()
        .default_focused("missing")
        .item(ToolbarItem::action("cut", "Cut").disabled(true))
        .item(ToolbarItem::separator("clipboard-separator"))
        .item(ToolbarItem::icon("copy", "C", "Copy"))
        .item(ToolbarItem::toggle("wrap", "Wrap").pressed(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.focused_value(), Some("copy"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[0].disabled());
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert!(state.items()[3].pressed());
    assert_eq!(
        toolbar_navigation_target(
            Orientation::Vertical,
            "down",
            state.focused_index().unwrap(),
            &[true, true, false, false],
        ),
        Some(3)
    );
}

#[test]
fn listbox_state_resolves_grouped_options_navigation_and_typeahead() {
    let state = ListboxState::resolve(
        Size::Small,
        false,
        "Assignee",
        Some("bravo"),
        Some("missing"),
        Some("ch"),
        "No assignees",
        [ListboxGroupDescriptor::new("team", "Team")
            .option(ListboxOptionDescriptor::option("charlie", "Charlie"))
            .option(ListboxOptionDescriptor::option("delta", "Delta").disabled(true))
            .option(ListboxOptionDescriptor::option("bravo", "Bravo"))],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::separator("standalone-separator"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::ListBox);
    assert_eq!(state.label(), "Assignee");
    assert_eq!(state.typeahead_query(), Some("ch"));
    assert_eq!(state.groups().len(), 1);
    assert_eq!(state.groups()[0].role(), Role::Group);
    assert_eq!(state.groups()[0].option_count(), 3);
    assert_eq!(state.options().len(), 5);
    assert_eq!(state.selected_value(), Some("bravo"));
    assert_eq!(state.active_value(), Some("bravo"));
    assert_eq!(state.options()[1].kind(), ListboxOptionKind::Separator);
    assert_eq!(state.options()[1].role(), None);
    assert!(!state.options()[1].focusable());
    assert!(state.options()[3].disabled());
    assert!(!state.options()[3].focusable());
    assert_eq!(state.options()[4].role(), Some(Role::ListBoxOption));
    assert_eq!(state.options()[4].position_in_set(), Some(4));
    assert_eq!(state.options()[4].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|option| option.value()),
        Some("alpha")
    );
    assert_eq!(
        state.typeahead_target("ch").map(|option| option.value()),
        Some("charlie")
    );
    assert_eq!(
        state
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("bravo".to_string())
    );
    assert_eq!(
        listbox_navigation_target(
            "down",
            state.active_index().unwrap(),
            &[false, true, false, true, false]
        ),
        Some(0)
    );
}

#[test]
fn listbox_state_scrollable_content_tracks_flattened_option_count_threshold() {
    let scrollable = ListboxState::resolve(
        Size::Small,
        false,
        "Scrollable",
        None,
        None,
        None,
        "No options",
        [],
        (0..7).map(|index| {
            ListboxOptionDescriptor::option(format!("item-{index}"), format!("Item {index}"))
        }),
        ThemeTokens::default(),
    );
    let not_scrollable = ListboxState::resolve(
        Size::Small,
        false,
        "Compact",
        None,
        None,
        None,
        "No options",
        [],
        (0..6).map(|index| {
            ListboxOptionDescriptor::option(format!("item-{index}"), format!("Item {index}"))
        }),
        ThemeTokens::default(),
    );

    assert!(scrollable.scrollable_content());
    assert!(!not_scrollable.scrollable_content());
}

#[test]
fn listbox_builder_state_models_empty_disabled_and_tokens() {
    let tokens = custom_tokens();
    let empty = Listbox::new("empty-listbox", "Empty")
        .empty_label("Nothing available")
        .tokens(tokens)
        .state();
    let disabled = Listbox::new("disabled-listbox", "Disabled")
        .disabled(true)
        .selected("one")
        .option(ListboxOption::new("one", "One"))
        .state();

    assert!(empty.empty());
    assert_eq!(empty.empty_label(), "Nothing available");
    assert_eq!(empty.colors().surface().token(), tokens.surface);
    assert!(disabled.disabled());
    assert_eq!(disabled.selected_value(), None);
    assert_eq!(disabled.active_value(), None);
    assert_eq!(disabled.activation_for_key("space"), None);
}

#[open_gpui::test]
fn listbox_runtime_click_and_keyboard_selection_skip_disabled_items(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct SelectionEvent {
        source: &'static str,
        selection: ListboxSelection,
    }

    struct TestView {
        events: Rc<RefCell<Vec<SelectionEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let listbox_events = self.events.clone();
            let alpha_events = self.events.clone();
            let charlie_events = self.events.clone();

            div().size_full().child(
                Listbox::new("runtime-listbox", "Runtime listbox")
                    .selected("alpha")
                    .option(ListboxOption::new("alpha", "Alpha").on_select(
                        move |selection, _, _| {
                            alpha_events.borrow_mut().push(SelectionEvent {
                                source: "option:alpha",
                                selection,
                            });
                        },
                    ))
                    .option(ListboxOption::separator("standalone-separator"))
                    .option(ListboxOption::new("bravo", "Bravo").disabled(true))
                    .group(
                        ListboxGroup::new("team", "Team")
                            .option(ListboxOption::new("charlie", "Charlie").on_select(
                                move |selection, _, _| {
                                    charlie_events.borrow_mut().push(SelectionEvent {
                                        source: "option:charlie",
                                        selection,
                                    });
                                },
                            ))
                            .option(ListboxOption::new("delta", "Delta")),
                    )
                    .on_select(move |selection, _, _| {
                        listbox_events.borrow_mut().push(SelectionEvent {
                            source: "listbox",
                            selection,
                        });
                    }),
            )
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:runtime-listbox").is_some(),
        "listbox root should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-listbox:separator:standalone-separator")
            .is_some(),
        "listbox separator should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-listbox:group:team")
            .is_some(),
        "listbox group label should expose a stable debug selector"
    );

    let disabled_bravo = cx
        .debug_bounds("listbox:runtime-listbox:option:bravo")
        .expect("disabled Bravo option should be rendered");
    cx.simulate_click(disabled_bravo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        events.borrow().is_empty(),
        "disabled option click should not emit selection callbacks"
    );

    let delta = cx
        .debug_bounds("listbox:runtime-listbox:option:delta")
        .expect("Delta option should be rendered");
    cx.simulate_click(delta.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_delta_click = events.borrow().clone();
    assert_eq!(after_delta_click.len(), 1);
    assert_eq!(after_delta_click[0].source, "listbox");
    assert_eq!(after_delta_click[0].selection.index(), 4);
    assert_eq!(after_delta_click[0].selection.value(), "delta");
    assert_eq!(after_delta_click[0].selection.label(), "Delta");

    let alpha = cx
        .debug_bounds("listbox:runtime-listbox:option:alpha")
        .expect("Alpha option should be rendered");
    cx.simulate_click(alpha.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_alpha_click = events.borrow().clone();
    assert_eq!(after_alpha_click.len(), 3);
    assert_eq!(after_alpha_click[1].source, "option:alpha");
    assert_eq!(after_alpha_click[1].selection.index(), 0);
    assert_eq!(after_alpha_click[1].selection.value(), "alpha");
    assert_eq!(after_alpha_click[2].source, "listbox");
    assert_eq!(after_alpha_click[2].selection.value(), "alpha");

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().len(),
        3,
        "arrow navigation should move active option without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = events.borrow().clone();
    assert_eq!(after_enter.len(), 5);
    assert_eq!(after_enter[3].source, "option:charlie");
    assert_eq!(after_enter[3].selection.index(), 3);
    assert_eq!(after_enter[3].selection.value(), "charlie");
    assert_eq!(after_enter[3].selection.label(), "Charlie");
    assert_eq!(after_enter[4].source, "listbox");
    assert_eq!(after_enter[4].selection.value(), "charlie");

    cx.simulate_keystrokes("up");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().len(),
        5,
        "arrow navigation after selection should still move active option without selecting"
    );

    cx.simulate_keystrokes("space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = events.borrow().clone();
    assert_eq!(after_space.len(), 7);
    assert_eq!(after_space[5].source, "option:alpha");
    assert_eq!(after_space[5].selection.index(), 0);
    assert_eq!(after_space[5].selection.value(), "alpha");
    assert_eq!(after_space[5].selection.label(), "Alpha");
    assert_eq!(after_space[6].source, "listbox");
    assert_eq!(after_space[6].selection.value(), "alpha");
}

#[test]
fn select_state_records_popup_listbox_overlay_and_scroll_contract() {
    let state = Select::new("priority-select", "Priority")
        .placeholder("Choose priority")
        .open(true)
        .selected("high")
        .placement(OverlayPlacementSide::Right, OverlayPlacementAlignment::End)
        .option(ListboxOption::new("low", "Low"))
        .option(ListboxOption::new("medium", "Medium").disabled(true))
        .group(
            ListboxGroup::new("recommended", "Recommended")
                .option(ListboxOption::new("high", "High"))
                .option(ListboxOption::new("urgent", "Urgent"))
                .option(ListboxOption::new("normal", "Normal"))
                .option(ListboxOption::new("later", "Later"))
                .option(ListboxOption::new("someday", "Someday")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), SelectOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::ListBox);
    assert!(state.trigger_selected());
    assert_eq!(state.trigger_label(), "High");
    assert_eq!(state.selected_value(), Some("high"));
    assert_eq!(state.active_value(), Some("high"));
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.listbox().role(), Role::ListBox);
    assert_eq!(state.listbox().selected_value(), Some("high"));
    assert!(state.scrollable_content());
    assert!(state.scroll_area().scrolls_y());
}

#[test]
fn select_state_models_disabled_empty_and_policy_overrides() {
    let state = Select::new("empty-select", "Empty")
        .placeholder("Nothing to choose")
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::DismissAndPassThrough)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), SelectOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.trigger_label(), "Nothing to choose");
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.active_value(), None);
    assert!(!state.scrollable_content());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn select_runtime_click_and_keyboard_selection_close_popup_and_emit_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SelectRuntimeEvent {
        Open(bool),
        Select(SelectSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<SelectRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Select::new("runtime-select", "Runtime select")
                    .placeholder("Choose item")
                    .option(ListboxOption::new("alpha", "Alpha"))
                    .option(ListboxOption::new("bravo", "Bravo").disabled(true))
                    .group(
                        ListboxGroup::new("team", "Team")
                            .option(ListboxOption::new("charlie", "Charlie"))
                            .option(ListboxOption::new("delta", "Delta")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(SelectRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(SelectRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("select:runtime-select:root").is_some(),
        "select root should expose a stable debug selector"
    );

    let trigger = cx
        .debug_bounds("select:runtime-select:trigger")
        .expect("select trigger should be rendered");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![SelectRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "select content should open from the real trigger"
    );

    let disabled_bravo = cx
        .debug_bounds("listbox:runtime-select-listbox:option:bravo")
        .expect("disabled Bravo option should be rendered in the popup listbox");
    cx.simulate_click(disabled_bravo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![SelectRuntimeEvent::Open(true)],
        "disabled popup option click should not emit selection callbacks or close the popup"
    );

    let alpha = cx
        .debug_bounds("listbox:runtime-select-listbox:option:alpha")
        .expect("Alpha option should be rendered in the popup listbox");
    cx.simulate_click(alpha.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_none(),
        "enabled popup option click should close the content"
    );

    let trigger = cx
        .debug_bounds("select:runtime-select:trigger")
        .expect("select trigger should still be rendered after selection");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "select content should reopen from the trigger after a prior selection"
    );

    let alpha = cx
        .debug_bounds("listbox:runtime-select-listbox:option:alpha")
        .expect("Alpha option should be rendered after reopening");
    cx.simulate_mouse_down(alpha.center(), MouseButton::Left, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ],
        "mouse down should focus the option without selecting until mouse up or keyboard activation"
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "mouse down focus should keep the popup open for keyboard activation"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ],
        "arrow navigation in the popup listbox should not select"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(2, "charlie", "Charlie")),
            SelectRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_none(),
        "keyboard popup selection should close the content"
    );
}

#[test]
fn combobox_state_filters_query_without_clearing_selection() {
    let state = Combobox::new("framework-combobox", "Framework")
        .placeholder("Search frameworks")
        .open(true)
        .default_query("re")
        .selected("solid")
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember").disabled(true))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), ComboboxOpenMode::Controlled);
    assert_eq!(state.input_role(), Role::EditableComboBox);
    assert_eq!(state.content_role(), Role::ListBox);
    assert_eq!(state.query(), "re");
    assert_eq!(state.total_option_count(), 5);
    assert_eq!(state.filtered_option_count(), 3);
    assert!(state.filtered());
    assert_eq!(state.selected_value(), Some("solid"));
    assert_eq!(state.active_value(), Some("react"));
    assert_eq!(state.listbox().role(), Role::ListBox);
    assert_eq!(state.listbox().selected_value(), None);
    assert_eq!(state.listbox().typeahead_query(), Some("re"));
    assert_eq!(
        state.listbox().options()[0].role(),
        Some(Role::ListBoxOption)
    );
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(state.input().placeholder(), Some("Search frameworks"));
}

#[test]
fn combobox_state_scrollable_content_tracks_filtered_option_count() {
    let scrollable = Combobox::new("scrolling-combobox", "Scrolling combobox")
        .placeholder("Search frameworks")
        .open(true)
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember"))
        .option(ComboboxOption::new("svelte", "Svelte"))
        .option(ComboboxOption::new("angular", "Angular"))
        .option(ComboboxOption::new("vue", "Vue"))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react")),
        )
        .state();
    let not_scrollable = Combobox::new("filtered-combobox", "Filtered combobox")
        .placeholder("Search frameworks")
        .open(true)
        .default_query("re")
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember"))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
        )
        .state();

    assert_eq!(scrollable.total_option_count(), 7);
    assert_eq!(scrollable.filtered_option_count(), 7);
    assert!(scrollable.scrollable_content());

    assert_eq!(not_scrollable.total_option_count(), 5);
    assert_eq!(not_scrollable.filtered_option_count(), 3);
    assert!(!not_scrollable.scrollable_content());
}

#[test]
fn combobox_disabled_empty_state_blocks_popup_and_input() {
    let state = Combobox::new("empty-combobox", "Empty")
        .placeholder("Search")
        .default_open(true)
        .disabled(true)
        .default_query("zzz")
        .option(ComboboxOption::new("react", "React"))
        .empty_label("No frameworks")
        .outside_press_policy(OutsidePressPolicy::DismissAndPassThrough)
        .state();

    assert_eq!(state.open_mode(), ComboboxOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.filtered_option_count(), 0);
    assert!(state.listbox().empty());
    assert_eq!(state.listbox().empty_label(), "No frameworks");
    assert!(!state.input().editable());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn combobox_runtime_filters_input_and_selects_filtered_option(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ComboboxRuntimeEvent {
        Open(bool),
        Select(ComboboxSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<ComboboxRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Combobox::new("runtime-combobox", "Runtime combobox")
                    .placeholder("Search frameworks")
                    .option(ComboboxOption::new("react", "React").keyword("library"))
                    .option(ComboboxOption::new("solid", "Solid"))
                    .option(ComboboxOption::new("ember", "Ember").disabled(true))
                    .group(
                        ComboboxGroup::new("meta", "Meta")
                            .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                            .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-combobox-input:root")
        .expect("combobox text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("re");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_none(),
        "typing text should filter input without implicitly opening the popup"
    );

    let toggle = cx
        .debug_bounds("combobox:runtime-combobox:toggle")
        .expect("combobox toggle should be rendered");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![ComboboxRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_some(),
        "toggle click should open filtered popup content"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:react")
            .is_some(),
        "React should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:remix")
            .is_some(),
        "Remix should match query keyword"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:solid")
            .is_none(),
        "Solid should be filtered out by query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:ember")
            .is_none(),
        "disabled Ember should still be filtered out when it does not match"
    );

    let remix = cx
        .debug_bounds("listbox:runtime-combobox-listbox:option:remix")
        .expect("filtered Remix option should be rendered");
    cx.simulate_click(remix.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            ComboboxRuntimeEvent::Open(true),
            ComboboxRuntimeEvent::Select(ComboboxSelection::new("remix", "Remix")),
            ComboboxRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_none(),
        "combobox selection should close popup content"
    );
}

#[open_gpui::test]
fn combobox_runtime_keyboard_selects_filtered_option(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ComboboxRuntimeEvent {
        Open(bool),
        Select(ComboboxSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<ComboboxRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Combobox::new("keyboard-combobox", "Keyboard combobox")
                    .placeholder("Search frameworks")
                    .option(ComboboxOption::new("react", "React").keyword("library"))
                    .option(ComboboxOption::new("solid", "Solid"))
                    .group(
                        ComboboxGroup::new("meta", "Meta")
                            .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                            .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:keyboard-combobox-input:root")
        .expect("combobox text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("re");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![ComboboxRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("combobox:keyboard-combobox:content")
            .is_some(),
        "down arrow should open filtered combobox content from the input row"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            ComboboxRuntimeEvent::Open(true),
            ComboboxRuntimeEvent::Select(ComboboxSelection::new("remix", "Remix")),
            ComboboxRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("combobox:keyboard-combobox:content")
            .is_none(),
        "keyboard selection should close filtered combobox content"
    );
}

#[test]
fn command_state_filters_groups_shortcuts_loading_and_dialog_policy() {
    let state = Command::new("command-palette", "Command palette")
        .placeholder("Type a command")
        .open(true)
        .default_query("file")
        .selected("new-file")
        .loading("Indexing commands", Some(45))
        .dialog("Command palette")
        .dialog_description("Run a workspace command")
        .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroup::new("file", "File")
                .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
        )
        .group(
            CommandGroup::new("view", "View")
                .item(CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), CommandOpenMode::Controlled);
    assert_eq!(state.input_role(), Role::TextInput);
    assert_eq!(state.list_role(), Role::ListBox);
    assert_eq!(state.query(), "file");
    assert_eq!(state.total_item_count(), 4);
    assert_eq!(state.filtered_item_count(), 2);
    assert!(state.filtered());
    assert_eq!(state.selected_value(), Some("new-file"));
    assert_eq!(state.active_value(), Some("new-file"));
    assert_eq!(state.groups().len(), 2);
    assert_eq!(state.groups()[0].label(), "Commands");
    assert_eq!(state.groups()[1].label(), "File");
    assert!(state.groups()[0].match_score() > 0);
    assert!(state.groups()[1].match_score() > 0);
    assert_eq!(state.items().len(), 2);
    assert_eq!(state.items()[1].shortcut(), Some("Ctrl+N"));
    assert!(state.items()[1].selected());
    let activation = state.activation_for_key("enter").unwrap();
    assert_eq!(activation.value(), "new-file");
    assert_eq!(activation.shortcut(), Some("Ctrl+N"));
    assert!(state.loading().is_some());
    assert_eq!(state.loading().unwrap().role(), Role::ProgressIndicator);
    assert_eq!(state.loading().unwrap().progress_percent(), Some(45));
    assert!(state.scroll_area().scrolls_y());
    assert_eq!(
        state.scroll_area().reset_policy(),
        ScrollResetPolicy::Preserve
    );
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    let dialog = state.dialog().unwrap();
    assert!(dialog.open());
    assert_eq!(dialog.content_role(), Role::Window);
    assert_eq!(dialog.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(dialog.description(), Some("Run a workspace command"));
}

#[test]
fn command_state_reports_match_sources_for_label_value_keyword_and_shortcut() {
    let label_state = Command::new("label-command", "Commands")
        .default_query("open")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    assert_eq!(label_state.items()[0].value(), "open-file");
    assert_eq!(
        label_state.items()[0].match_source(),
        Some(CommandMatchSource::Label)
    );

    let value_state = Command::new("value-command", "Commands")
        .default_query("open-file")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    assert_eq!(
        value_state.items()[0].match_source(),
        Some(CommandMatchSource::Value)
    );

    let keyword_state = Command::new("keyword-command", "Commands")
        .default_query("prefs")
        .item(CommandItem::new("settings", "Settings").keyword("prefs"))
        .state();
    assert_eq!(keyword_state.items()[0].value(), "settings");
    assert_eq!(
        keyword_state.items()[0].match_source(),
        Some(CommandMatchSource::Keyword)
    );

    let shortcut_state = Command::new("shortcut-command", "Commands")
        .default_query("ctrl+p")
        .item(CommandItem::new("palette", "Command Palette").shortcut("Ctrl+P"))
        .state();
    assert_eq!(shortcut_state.items()[0].value(), "palette");
    assert_eq!(
        shortcut_state.items()[0].match_source(),
        Some(CommandMatchSource::Shortcut)
    );
}

#[test]
fn command_state_empty_query_preserves_caller_order() {
    let state = Command::new("ordered-command", "Commands")
        .item(CommandItem::new("root-two", "Root Two"))
        .item(CommandItem::new("root-one", "Root One"))
        .group(
            CommandGroup::new("group", "Group")
                .item(CommandItem::new("group-two", "Group Two"))
                .item(CommandItem::new("group-one", "Group One")),
        )
        .state();
    let values = state
        .items()
        .iter()
        .map(|item| item.value().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "root-two".to_string(),
            "root-one".to_string(),
            "group-two".to_string(),
            "group-one".to_string(),
        ]
    );
    assert!(
        state
            .items()
            .iter()
            .all(|item| item.match_source().is_none() && item.match_score() == 0)
    );
    assert!(state.groups().iter().all(|group| group.match_score() == 0));
}

#[test]
fn command_state_ranks_label_and_value_matches_before_keyword_only_matches() {
    let state = Command::new("ranked-command", "Commands")
        .default_query("file")
        .item(CommandItem::new("archive", "Archive").keyword("file"))
        .item(CommandItem::new("open-file", "Open File"))
        .item(CommandItem::new("file-action", "Launcher"))
        .item(CommandItem::new("bulk-action", "Bulk Action").keyword("file"))
        .state();
    let values = state
        .items()
        .iter()
        .map(|item| item.value().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "open-file".to_string(),
            "file-action".to_string(),
            "archive".to_string(),
            "bulk-action".to_string(),
        ]
    );
    assert_eq!(
        state.items()[0].match_source(),
        Some(CommandMatchSource::Label)
    );
    assert_eq!(
        state.items()[1].match_source(),
        Some(CommandMatchSource::Value)
    );
    assert_eq!(
        state.items()[2].match_source(),
        Some(CommandMatchSource::Keyword)
    );
    assert!(state.items()[1].match_score() > state.items()[2].match_score());
}

#[test]
fn command_state_tracks_active_and_selected_by_value_after_reorder() {
    let first = Command::new("first-command", "Commands")
        .selected("target")
        .active("target")
        .item(CommandItem::new("other", "Other"))
        .item(CommandItem::new("target", "Target"))
        .state();
    let reordered = Command::new("reordered-command", "Commands")
        .selected("target")
        .active("target")
        .item(CommandItem::new("target", "Target"))
        .item(CommandItem::new("other", "Other"))
        .state();

    assert_eq!(first.selected_value(), Some("target"));
    assert_eq!(first.active_value(), Some("target"));
    assert!(first.items()[1].selected());
    assert!(first.items()[1].active());
    assert_eq!(reordered.selected_value(), Some("target"));
    assert_eq!(reordered.active_value(), Some("target"));
    assert!(reordered.items()[0].selected());
    assert!(reordered.items()[0].active());
}

#[test]
fn command_state_keeps_disabled_matches_visible_but_non_activatable() {
    let state = Command::new("disabled-command", "Commands")
        .default_query("delete")
        .selected("delete-project")
        .active("delete-project")
        .item(CommandItem::new("delete-project", "Delete Project").disabled(true))
        .state();

    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(state.items()[0].value(), "delete-project");
    assert!(state.items()[0].disabled());
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.active_value(), None);
    assert_eq!(state.activation_for_key("enter"), None);
}

#[test]
fn command_state_models_controlled_and_default_query_ownership() {
    let controlled = Command::new("controlled-query-command", "Commands")
        .query("open")
        .default_query("ignored")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    let seeded = Command::new("seeded-query-command", "Commands")
        .default_query("file")
        .item(CommandItem::new("open-file", "Open File"))
        .state();

    assert_eq!(controlled.query(), "open");
    assert_eq!(controlled.query_mode(), CommandQueryMode::Controlled);
    assert_eq!(controlled.filtered_item_count(), 1);
    assert_eq!(seeded.query(), "file");
    assert_eq!(seeded.query_mode(), CommandQueryMode::Uncontrolled);
}

#[test]
fn command_state_models_multi_selected_values_and_hidden_chips() {
    let state = Command::new("multi-command", "Commands")
        .default_query("new")
        .multi_select(true)
        .selected_values(["open-file", "new-file", "missing", "delete-file"])
        .item(CommandItem::new("open-file", "Open File"))
        .item(CommandItem::new("delete-file", "Delete File").disabled(true))
        .group(CommandGroup::new("file", "File").item(CommandItem::new("new-file", "New File")))
        .state();

    assert_eq!(state.selection_mode(), CommandSelectionMode::Multiple);
    assert_eq!(
        state.selected_values(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(
        state
            .selected_chips()
            .iter()
            .map(|chip| chip.value().to_owned())
            .collect::<Vec<_>>(),
        vec!["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(state.selected_chips()[0].label(), "Open File");
    assert!(state.items()[0].selected());
}

#[test]
fn command_index_snapshot_matches_equivalent_local_descriptors() {
    let snapshot = CommandIndexSnapshot::new("commands-v1")
        .item(CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroupDescriptor::new("file", "File")
                .item(CommandItemDescriptor::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(
                    CommandItemDescriptor::new("close-window", "Close Window").shortcut("Alt+F4"),
                ),
        );
    let local = Command::new("local-command", "Commands")
        .default_query("file")
        .selected("new-file")
        .active("new-file")
        .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroup::new("file", "File")
                .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
        )
        .state();
    let indexed = Command::new("indexed-command", "Commands")
        .default_query("file")
        .selected("new-file")
        .active("new-file")
        .index_snapshot(snapshot)
        .state();

    assert_eq!(indexed.index_revision(), Some("commands-v1"));
    assert_eq!(indexed.index_mode(), CommandIndexSnapshotMode::LocalRanked);
    assert_eq!(indexed.total_item_count(), local.total_item_count());
    assert_eq!(indexed.filtered_item_count(), local.filtered_item_count());
    assert_eq!(
        indexed
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.label().to_owned(),
                item.match_source(),
                item.match_score(),
                item.selected(),
                item.active(),
            ))
            .collect::<Vec<_>>(),
        local
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.label().to_owned(),
                item.match_source(),
                item.match_score(),
                item.selected(),
                item.active(),
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn command_index_snapshot_revision_preserves_selection_by_value_after_reorder() {
    let first = CommandIndexSnapshot::new("commands-v1")
        .item(CommandItemDescriptor::new("other", "Other"))
        .item(CommandItemDescriptor::new("target", "Target"));
    let second = CommandIndexSnapshot::new("commands-v2")
        .item(CommandItemDescriptor::new("target", "Target"))
        .item(CommandItemDescriptor::new("other", "Other"));
    let first_state = Command::new("snapshot-revision-command", "Commands")
        .selected("target")
        .active("target")
        .index_snapshot(first)
        .state();
    let second_state = Command::new("snapshot-revision-command", "Commands")
        .selected("target")
        .active("target")
        .index_snapshot(second)
        .state();

    assert_eq!(first_state.index_revision(), Some("commands-v1"));
    assert_eq!(second_state.index_revision(), Some("commands-v2"));
    assert_eq!(first_state.items()[1].value(), "target");
    assert!(first_state.items()[1].selected());
    assert!(first_state.items()[1].active());
    assert_eq!(second_state.items()[0].value(), "target");
    assert!(second_state.items()[0].selected());
    assert!(second_state.items()[0].active());
}

#[test]
fn command_index_snapshot_modes_preserve_pre_ranked_and_pre_filtered_order() {
    let pre_ranked = CommandIndexSnapshot::new("pre-ranked")
        .mode(CommandIndexSnapshotMode::PreRankedFilter)
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .item(CommandItemDescriptor::new("open-file", "Open File"))
        .item(CommandItemDescriptor::new("file-action", "Launcher"))
        .item(CommandItemDescriptor::new("bulk-action", "Bulk Action").keyword("file"));
    let pre_filtered = CommandIndexSnapshot::new("pre-filtered")
        .mode(CommandIndexSnapshotMode::PreFiltered)
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .item(CommandItemDescriptor::new("unmatched", "Unmatched"));

    let pre_ranked_state = Command::new("pre-ranked-command", "Commands")
        .query("file")
        .index_snapshot(pre_ranked)
        .state();
    let pre_filtered_state = Command::new("pre-filtered-command", "Commands")
        .query("file")
        .index_snapshot(pre_filtered)
        .state();

    assert_eq!(
        pre_ranked_state
            .items()
            .iter()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "archive".to_string(),
            "open-file".to_string(),
            "file-action".to_string(),
            "bulk-action".to_string(),
        ]
    );
    assert_eq!(
        pre_ranked_state
            .items()
            .iter()
            .map(|item| item.match_source())
            .collect::<Vec<_>>(),
        vec![
            Some(CommandMatchSource::Keyword),
            Some(CommandMatchSource::Label),
            Some(CommandMatchSource::Value),
            Some(CommandMatchSource::Keyword),
        ]
    );
    assert_eq!(pre_filtered_state.filtered_item_count(), 2);
    assert_eq!(
        pre_filtered_state
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.match_source(),
                item.match_score()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("archive".to_string(), None, 0),
            ("unmatched".to_string(), None, 0),
        ]
    );
}

#[test]
fn command_index_snapshot_loading_coexists_with_visible_and_empty_results() {
    let visible = CommandIndexSnapshot::new("loading-visible")
        .mode(CommandIndexSnapshotMode::PreFiltered)
        .loading(CommandLoadingState::new(
            "Refreshing command index",
            Some(30),
        ))
        .item(CommandItemDescriptor::new(
            "stale-open",
            "Open from stale index",
        ));
    let empty = CommandIndexSnapshot::new("loading-empty")
        .loading(CommandLoadingState::new("Indexing commands", None));

    let visible_state = Command::new("snapshot-loading-visible", "Commands")
        .query("anything")
        .loading("Builder loading is overridden", Some(99))
        .index_snapshot(visible)
        .state();
    let empty_state = Command::new("snapshot-loading-empty", "Commands")
        .query("anything")
        .index_snapshot(empty)
        .state();

    assert_eq!(visible_state.filtered_item_count(), 1);
    assert_eq!(
        visible_state.loading().map(CommandLoadingState::message),
        Some("Refreshing command index")
    );
    assert_eq!(
        visible_state
            .loading()
            .and_then(CommandLoadingState::progress_percent),
        Some(30)
    );
    assert!(empty_state.empty());
    assert_eq!(
        empty_state.loading().map(CommandLoadingState::message),
        Some("Indexing commands")
    );
    assert_eq!(empty_state.loading().unwrap().progress_percent(), None);
}

#[test]
fn command_render_plan_virtualizes_large_result_sets_with_stable_rows() {
    let command =
        Command::new("large-command", "Commands")
            .with_size(Size::Small)
            .row_height(ui_px(28.0))
            .overscan(4)
            .active("item-0104")
            .selected("item-0101")
            .items((0..10_000).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }));
    let plan = command.render_plan_with_viewport(ui_px(2_800.0), ui_px(196.0));

    assert_eq!(plan.role(), Role::ListBox);
    assert_eq!(plan.row_role(), Role::ListBoxOption);
    assert_eq!(plan.virtualizer().count(), 10_000);
    assert_eq!(plan.state().total_item_count(), 10_000);
    assert_eq!(plan.state().filtered_item_count(), 10_000);
    assert_eq!(
        *plan.virtualizer().visible_range(),
        VirtualizerRange::new(100, 107)
    );
    assert_eq!(
        *plan.virtualizer().overscan_range(),
        VirtualizerRange::new(98, 109)
    );
    assert_eq!(plan.visible_row_count(), 7);
    assert_eq!(plan.rendered_row_count(), 11);
    assert_eq!(plan.rows()[0].index(), 98);
    assert_eq!(plan.rows()[0].render_key(), "item-0098");

    let active = plan.active_row().expect("active command row should render");
    assert_eq!(active.index(), 104);
    assert_eq!(active.value(), "item-0104");
    assert!(active.active());
    assert_eq!(active.virtual_start(), ui_px(2_912.0));
    assert_eq!(active.virtual_size(), ui_px(28.0));
    assert_eq!(
        plan.selected_rows()
            .map(|row| row.value().to_owned())
            .collect::<Vec<_>>(),
        vec!["item-0101".to_string()]
    );

    let scrolled = command.render_plan_with_viewport(ui_px(5_600.0), ui_px(196.0));
    assert_eq!(
        *scrolled.virtualizer().visible_range(),
        VirtualizerRange::new(200, 207)
    );
    assert_eq!(scrolled.rows()[0].value(), "item-0198");
}

#[test]
fn command_render_plan_clamps_filtered_scroll_and_disambiguates_duplicate_values() {
    let duplicate_plan = Command::new("duplicate-command", "Commands")
        .row_height(ui_px(28.0))
        .item(CommandItem::new("duplicate", "Open File"))
        .item(CommandItem::new("duplicate", "Open Recent"))
        .item(CommandItem::new("unique", "Close File"))
        .render_plan_with_viewport(ui_px(0.0), ui_px(112.0));

    assert_eq!(
        duplicate_plan
            .rows()
            .iter()
            .map(|row| (row.value().to_owned(), row.render_key().to_owned()))
            .collect::<Vec<_>>(),
        vec![
            ("duplicate".to_string(), "0:duplicate".to_string()),
            ("duplicate".to_string(), "1:duplicate".to_string()),
            ("unique".to_string(), "unique".to_string()),
        ]
    );
    assert_eq!(
        duplicate_plan
            .virtualizer()
            .items()
            .iter()
            .map(|measurement| measurement.key().as_str())
            .collect::<Vec<_>>(),
        ["0:duplicate", "1:duplicate", "unique"]
    );

    let filtered =
        Command::new("filtered-command", "Commands")
            .default_query("item 0001")
            .row_height(ui_px(28.0))
            .items((0..10_000).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }))
            .render_plan_with_viewport(ui_px(80_000.0), ui_px(112.0));

    assert_eq!(filtered.state().filtered_item_count(), 1);
    assert_eq!(filtered.virtualizer().scroll_offset(), ui_px(0.0));
    assert_eq!(filtered.rows()[0].value(), "item-0001");
}

#[test]
fn command_multi_selection_change_toggles_values_without_duplicates() {
    let add = CommandSelectionChange::new(
        vec!["open-file".to_string(), "new-file".to_string()],
        CommandSelection::new(1, "new-file", "New File", None),
        true,
    );
    let remove = CommandSelectionChange::new(
        vec!["open-file".to_string()],
        CommandSelection::new(1, "new-file", "New File", None),
        false,
    );

    assert_eq!(
        add.values(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert!(add.selected());
    assert_eq!(add.toggled().value(), "new-file");
    assert_eq!(remove.values(), &["open-file".to_string()]);
    assert!(!remove.selected());
}

#[test]
fn command_state_models_empty_disabled_and_escape_policy() {
    let state = Command::new("empty-command", "Commands")
        .default_open(true)
        .disabled(true)
        .default_query("missing")
        .item(CommandItem::new("open", "Open"))
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .focus_restore_intent(FocusRestoreIntent::None)
        .state();

    assert_eq!(state.open_mode(), CommandOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.filtered_item_count(), 0);
    assert!(state.listbox().empty());
    assert!(!state.input().editable());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(
        state.overlay().policy().escape_key_policy(),
        EscapeKeyPolicy::Ignore
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn command_runtime_filters_input_and_selects_with_keyboard(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<CommandSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Command::new("runtime-command", "Runtime command")
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
                    .group(
                        CommandGroup::new("file", "File")
                            .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                            .item(
                                CommandItem::new("close-window", "Close Window").shortcut("Alt+F4"),
                            ),
                    )
                    .group(CommandGroup::new("view", "View").item(
                        CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout"),
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:runtime-command:content").is_some(),
        "inline command content should render immediately"
    );
    let input = cx
        .debug_bounds("text-input:runtime-command-input:root")
        .expect("command text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("file");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:open-file")
            .is_some(),
        "Open File should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:new-file")
            .is_some(),
        "New File should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:toggle-sidebar")
            .is_none(),
        "Toggle Sidebar should be filtered out before keyboard activation"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move active command without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selections.borrow().clone(),
        vec![CommandSelection::new(
            1,
            "new-file",
            "New File",
            Some("Ctrl+N".to_string())
        )]
    );
    assert!(
        cx.debug_bounds("command:runtime-command:content").is_some(),
        "inline command selection should not close non-dialog content"
    );
}

#[open_gpui::test]
fn command_runtime_controlled_query_emits_sanitized_query_changes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        query: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let query = self.query.borrow().clone();
            let next_query = self.query.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Command::new("controlled-query-runtime-command", "Runtime command")
                    .query(query)
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File"))
                    .item(CommandItem::new("close-window", "Close Window"))
                    .on_query_change(move |query, _, _| {
                        *next_query.borrow_mut() = query.clone();
                        changes.borrow_mut().push(query);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let query = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        query: query.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-query-runtime-command-input:root")
        .expect("controlled command input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("open\nfile");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(query.borrow().as_str(), "open file");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("open file")
    );
    assert!(
        cx.debug_bounds("listbox:controlled-query-runtime-command-listbox:option:open-file")
            .is_some(),
        "controlled query should feed filtered command rows after caller feedback"
    );
}

#[open_gpui::test]
fn command_runtime_dialog_selects_and_dismisses_without_stale_modal_layer(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum CommandDialogRuntimeEvent {
        Open(bool),
        Select(CommandSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<CommandDialogRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Command::new("dialog-runtime-command", "Dialog runtime command")
                    .dialog("Command palette")
                    .trigger_label("Open command")
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
                    .group(
                        CommandGroup::new("file", "File")
                            .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                            .item(
                                CommandItem::new("close-window", "Close Window").shortcut("Alt+F4"),
                            ),
                    )
                    .group(CommandGroup::new("view", "View").item(
                        CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout"),
                    ))
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(CommandDialogRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(CommandDialogRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "dialog command content should start closed"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![CommandDialogRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_some(),
        "trigger click should open dialog command content"
    );

    let input = cx
        .debug_bounds("text-input:dialog-runtime-command-input:root")
        .expect("dialog command text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("file");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:open-file")
            .is_some(),
        "Open File should match query text in dialog mode"
    );
    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:new-file")
            .is_some(),
        "New File should match query text in dialog mode"
    );
    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:toggle-sidebar")
            .is_none(),
        "unmatched command rows should be filtered out in dialog mode"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![CommandDialogRuntimeEvent::Open(true)],
        "arrow navigation should move the active command without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "dialog command selection should close the modal content"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should remain rendered after selection");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let input = cx
        .debug_bounds("text-input:dialog-runtime-command-input:root")
        .expect("dialog command input should render after reopening");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
        ],
        "escape should close a reopened dialog exactly once"
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "escape should remove the dialog content"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should remain rendered after escape");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_click(point(px(4.0), px(4.0)), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
        ],
        "outside press should close a reopened dialog exactly once"
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "outside press should remove the dialog content"
    );
}

#[open_gpui::test]
fn command_runtime_multi_select_toggles_chips_without_closing_dialog(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selected_values: Rc<RefCell<Vec<String>>>,
        changes: Rc<RefCell<Vec<CommandSelectionChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selected_values = self.selected_values.borrow().clone();
            let next_values = self.selected_values.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Command::new("multi-runtime-command", "Runtime command")
                    .dialog("Command palette")
                    .trigger_label("Open command")
                    .multi_select(true)
                    .selected_values(selected_values)
                    .item(CommandItem::new("open-file", "Open File"))
                    .item(CommandItem::new("new-file", "New File"))
                    .item(CommandItem::new("delete-file", "Delete File").disabled(true))
                    .on_selected_values_change(move |change, _, _| {
                        *next_values.borrow_mut() = change.values().to_vec();
                        changes.borrow_mut().push(change);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let selected_values = Rc::new(RefCell::new(vec!["open-file".to_string()]));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selected_values: selected_values.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("command:multi-runtime-command:trigger")
        .expect("multi command trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:multi-runtime-command:selected-chip:open-file")
            .is_some(),
        "initial selected value should render as a chip"
    );

    let new_file = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:new-file")
        .expect("New File option should render");
    cx.simulate_click(new_file.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:multi-runtime-command:content")
            .is_some(),
        "multi-select activation should not close dialog content"
    );
    assert_eq!(
        selected_values.borrow().as_slice(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(changes.borrow().len(), 1);
    assert!(changes.borrow()[0].selected());
    assert_eq!(changes.borrow()[0].toggled().value(), "new-file");
    assert!(
        cx.debug_bounds("command:multi-runtime-command:selected-chip:new-file")
            .is_some(),
        "newly selected value should render as a chip after controlled feedback"
    );

    let disabled = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:delete-file")
        .expect("disabled matching option should still render");
    cx.simulate_click(disabled.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selected_values.borrow().as_slice(),
        &["open-file".to_string(), "new-file".to_string()],
        "disabled command should not alter the multi-selection set"
    );
    assert_eq!(changes.borrow().len(), 1);

    let open_file = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:open-file")
        .expect("Open File option should render");
    cx.simulate_click(open_file.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selected_values.borrow().as_slice(),
        &["new-file".to_string()]
    );
    assert_eq!(changes.borrow().len(), 2);
    assert!(!changes.borrow()[1].selected());
    assert_eq!(changes.borrow()[1].toggled().value(), "open-file");
}

#[open_gpui::test]
fn command_runtime_virtualized_results_scroll_inside_viewport_and_reveal_keyboard_targets(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<CommandSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let items = (0..120).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            });

            div().size_full().child(
                div().w(px(340.0)).h(px(420.0)).child(
                    ScrollArea::new(
                        "command-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "command-parent-top".into())
                                    .h(px(48.0))
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "command-wrapper".into())
                                    .h(px(300.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(
                                        Command::new("virtualized-runtime-command", "Commands")
                                            .with_size(Size::Small)
                                            .row_height(ui_px(28.0))
                                            .overscan(2)
                                            .viewport_item_count(4)
                                            .items(items)
                                            .on_select(move |selection, _, _| {
                                                selections.borrow_mut().push(selection);
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "command-parent-bottom".into())
                                    .h(px(240.0))
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0000")
            .is_some(),
        "initial command row should render"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0010")
            .is_none(),
        "row 10 should stay outside the initial virtual window"
    );
    let parent_bottom_before = cx
        .debug_bounds("command-parent-bottom")
        .expect("parent bottom should render before command scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:Commands:command-list-scroll")
        .expect("command result viewport should expose a stable scroll selector");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("command-parent-bottom")
        .expect("parent bottom should remain rendered after command scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside Command to stay inside the command viewport"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0000")
            .is_none(),
        "row 0 should unmount after internal command scroll"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0010")
            .is_some(),
        "row 10 should render after internal command scroll"
    );

    let input = cx
        .debug_bounds("text-input:virtualized-runtime-command-input:root")
        .expect("virtualized command input should render");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0007")
            .is_some(),
        "PageDown should reveal the newly active command row"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selections.borrow().as_slice(),
        &[CommandSelection::new(7, "item-0007", "Item 0007", None)]
    );
}

#[test]
fn disabled_icon_button_blocks_activation_metadata() {
    let state = IconButton::new("locked", "x", "Locked")
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn avatar_fallback_initials_derive_from_display_names_and_empty_names() {
    let ada = Avatar::new("ada", "Ada Lovelace").state();
    let single = Avatar::new("single", "Grace").state();
    let trio = Avatar::new("trio", "Foo Bar Dar").state();
    let empty = Avatar::new("empty", "  ").state();

    assert_eq!(ada.name(), "Ada Lovelace");
    assert_eq!(ada.fallback(), "AL");
    assert_eq!(ada.accessible_label(), "Ada Lovelace");
    assert_eq!(ada.role(), Role::Image);

    assert_eq!(single.fallback(), "GR");
    assert_eq!(trio.fallback(), "FB");
    assert_eq!(empty.fallback(), "?");
    assert_eq!(empty.accessible_label(), "Avatar");
}

#[test]
fn avatar_explicit_fallback_overrides_derived_initials() {
    let state = Avatar::new("current-user", "Ada Lovelace")
        .fallback("ME")
        .state();

    assert_eq!(state.name(), "Ada Lovelace");
    assert_eq!(state.fallback(), "ME");
}

#[test]
fn avatar_source_metadata_does_not_own_loading_state() {
    let state = Avatar::new("profile", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .state();

    assert!(state.has_source());
    assert_eq!(
        state.source().map(|source| source.uri()),
        Some("asset://avatars/ada.png")
    );
    assert_eq!(state.fallback(), "AL");
    assert_eq!(state.accessible_label(), "Ada Lovelace");
}

#[test]
fn avatar_accessible_label_can_be_explicit_for_source_and_fallback_avatars() {
    let fallback = Avatar::new("fallback-avatar", "Ada Lovelace")
        .accessible_label("Current user")
        .state();
    let source = Avatar::new("source-avatar", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .accessible_label("Ada profile photo")
        .state();

    assert_eq!(fallback.accessible_label(), "Current user");
    assert_eq!(source.accessible_label(), "Ada profile photo");
}

#[test]
fn avatar_size_metrics_and_token_intents_are_stable() {
    let tokens = custom_tokens();
    let small = Avatar::new("small-avatar", "Ada")
        .small()
        .tokens(tokens)
        .state();
    let medium = Avatar::new("medium-avatar", "Ada").tokens(tokens).state();
    let large = Avatar::new("large-avatar", "Ada")
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(small.size(), Size::Small);
    assert_eq!(small.metrics().diameter(), ui_px(28.0));
    assert_eq!(small.metrics().text_size(), ui_px(11.0));

    assert_eq!(medium.metrics().diameter(), ui_px(32.0));
    assert_eq!(medium.metrics().radius(), ui_px(16.0));

    assert_eq!(large.metrics().diameter(), ui_px(40.0));
    assert_eq!(large.metrics().text_size(), ui_px(14.0));
    assert_eq!(large.colors().background().token(), tokens.surface_muted);
    assert_eq!(large.colors().foreground().token(), tokens.text);
    assert_eq!(large.colors().border().token(), tokens.border);
}

#[test]
fn avatar_group_state_tracks_visible_and_hidden_counts() {
    let group = AvatarGroup::new("team")
        .avatar(Avatar::new("ada", "Ada Lovelace"))
        .avatar(Avatar::new("grace", "Grace Hopper"))
        .avatar(Avatar::new("katherine", "Katherine Johnson"))
        .avatar(Avatar::new("margaret", "Margaret Hamilton"))
        .max_visible(3)
        .tokens(custom_tokens());
    let state = group.state();

    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.total_count(), 4);
    assert_eq!(state.visible_count(), 3);
    assert_eq!(state.hidden_count(), 1);

    let count = AvatarGroupCount::new("team-count", state.hidden_count())
        .with_size(state.size())
        .tokens(custom_tokens());
    let count_state = count.state();

    assert_eq!(count_state.count(), 1);
    assert_eq!(count_state.size(), Size::Medium);
    assert_eq!(count_state.role(), Role::Label);
}

#[open_gpui::test]
fn avatar_renders_stable_debug_selector(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Avatar::new("runtime-avatar", "Ada Lovelace"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(cx.debug_bounds("avatar:runtime-avatar:root").is_some());
}

#[test]
fn separator_state_exposes_orientation_role_and_decorative_mode() {
    let horizontal = Separator::new("section-separator").state();
    let vertical = Separator::new("panel-separator").vertical().large().state();
    let decorative = Separator::new("decorative-separator")
        .decorative(true)
        .state();

    assert_eq!(horizontal.orientation(), Orientation::Horizontal);
    assert_eq!(horizontal.role(), Some(Role::Separator));
    assert_eq!(horizontal.metrics().thickness(), ui_px(1.0));
    assert_eq!(horizontal.colors().line().token(), semantic::BORDER);

    assert_eq!(vertical.orientation(), Orientation::Vertical);
    assert_eq!(vertical.role(), Some(Role::Separator));
    assert_eq!(vertical.metrics().thickness(), ui_px(2.0));

    assert!(decorative.decorative());
    assert_eq!(decorative.role(), None);
}

#[test]
fn kbd_state_is_display_only_with_muted_token_intents() {
    let tokens = custom_tokens();
    let state = Kbd::new("command-shortcut", "Ctrl+K")
        .small()
        .tokens(tokens)
        .state();

    assert_eq!(state.label(), "Ctrl+K");
    assert_eq!(state.size(), Size::Small);
    assert!(state.display_only());
    assert_eq!(state.metrics().min_width(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
    assert_eq!(state.colors().foreground().token(), tokens.text_muted);
    assert_eq!(state.colors().border().token(), tokens.border);
}

#[test]
fn progress_state_clamps_values_and_preserves_indeterminate_mode() {
    let full = Progress::new("upload-progress", "Upload")
        .value(142.0)
        .large()
        .state();
    let empty = Progress::new("empty-progress", "Empty")
        .value(f32::NAN)
        .state();
    let indeterminate = Progress::new("pending-progress", "Pending")
        .indeterminate()
        .state();

    assert_eq!(full.role(), Role::ProgressIndicator);
    assert_eq!(full.value_percent(), Some(100.0));
    assert_eq!(full.normalized_value(), Some(1.0));
    assert_eq!(
        full.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 1.0
        }
    );
    assert_eq!(full.indicator_start_fraction(), 0.0);
    assert_eq!(full.indicator_fraction(), 1.0);
    assert_eq!(full.metrics().height(), ui_px(10.0));
    assert_eq!(full.colors().track().token(), semantic::SURFACE_MUTED);
    assert_eq!(full.colors().indicator().token(), semantic::ACCENT);

    assert_eq!(empty.value_percent(), Some(0.0));
    assert_eq!(empty.normalized_value(), Some(0.0));
    assert_eq!(
        empty.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 0.0
        }
    );
    assert!(indeterminate.indeterminate());
    assert_eq!(indeterminate.value_percent(), None);
    assert_eq!(indeterminate.normalized_value(), None);
    assert_eq!(
        indeterminate.visual_mode(),
        ProgressVisualMode::Indeterminate
    );
    assert!(
        indeterminate.indicator_start_fraction() > 0.0,
        "indeterminate progress should not look like a left-anchored determinate fill"
    );
    assert!(
        indeterminate.indicator_fraction() > 0.0 && indeterminate.indicator_fraction() < 0.5,
        "indeterminate progress should render as a short segment, not as a fixed percentage value"
    );
}

#[test]
fn skeleton_state_is_noninteractive_placeholder_with_stable_metrics() {
    let tokens = custom_tokens();
    let state = Skeleton::new("loading-line")
        .subtle(true)
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(state.size(), Size::Large);
    assert!(state.subtle());
    assert!(state.display_only());
    assert_eq!(state.metrics().width(), ui_px(224.0));
    assert_eq!(state.metrics().height(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
}

#[open_gpui::test]
fn low_state_primitives_render_stable_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(Separator::new("runtime-separator"))
                .child(Kbd::new("runtime-kbd", "Ctrl+K"))
                .child(Progress::new("runtime-progress", "Loading").value(40.0))
                .child(Progress::new("runtime-progress-indeterminate", "Indexing").indeterminate())
                .child(Skeleton::new("runtime-skeleton"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for selector in [
        "separator:runtime-separator:root",
        "kbd:runtime-kbd:root",
        "progress:runtime-progress:root",
        "progress:runtime-progress:indicator",
        "progress:runtime-progress-indeterminate:root",
        "progress:runtime-progress-indeterminate:indicator",
        "skeleton:runtime-skeleton:root",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} should be rendered"
        );
    }

    let determinate_root = cx
        .debug_bounds("progress:runtime-progress:root")
        .expect("determinate progress root should render");
    let determinate_indicator = cx
        .debug_bounds("progress:runtime-progress:indicator")
        .expect("determinate progress indicator should render");
    let indeterminate_root = cx
        .debug_bounds("progress:runtime-progress-indeterminate:root")
        .expect("indeterminate progress root should render");
    let indeterminate_indicator = cx
        .debug_bounds("progress:runtime-progress-indeterminate:indicator")
        .expect("indeterminate progress indicator should render");

    let determinate_width =
        determinate_indicator.size.width.as_f32() / determinate_root.size.width.as_f32();
    let indeterminate_start = (indeterminate_indicator.left().as_f32()
        - indeterminate_root.left().as_f32())
        / indeterminate_root.size.width.as_f32();
    let indeterminate_width =
        indeterminate_indicator.size.width.as_f32() / indeterminate_root.size.width.as_f32();

    assert!(
        (determinate_width - 0.4).abs() < 0.02,
        "determinate progress indicator should match the provided value"
    );
    assert!(
        indeterminate_start > 0.25,
        "indeterminate progress indicator should not be left-anchored"
    );
    assert!(
        indeterminate_width > 0.25 && indeterminate_width < 0.45,
        "indeterminate progress indicator should be a short segment"
    );
}

#[test]
fn button_accepts_custom_token_bundle() {
    let tokens = custom_tokens();
    let state = Button::new("outline", "Outline")
        .variant(ButtonVariant::Outline)
        .tokens(tokens)
        .state();

    assert_eq!(state.colors().border().token(), tokens.border);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
}

#[test]
fn theme_resolver_keeps_token_intent_and_resolves_fallback_color() {
    let tokens = custom_tokens();
    let state = Button::new("default", "Default").tokens(tokens).state();
    let background = state.colors().background();

    assert_eq!(background.token(), tokens.accent);
    assert_eq!(background.state(), ColorState::Default);
    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(u32::from(ThemeResolver::resolve(background)), 0x1f7a66ff);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            background,
            ThemeSnapshot::dark()
        )),
        0x1f7a66ff
    );
}

#[test]
fn theme_resolver_prefers_runtime_theme_table_for_known_tokens() {
    let state = Button::new("default", "Default").state();
    let background = state.colors().background();
    let custom_colors = [ThemeColor::new(
        semantic::ACCENT,
        ColorState::Default,
        0x123456,
    )];
    let snapshot = ThemeSnapshot::new(ThemeMode::Light, 42, &custom_colors);

    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(background, snapshot)),
        0x123456ff
    );
    assert_eq!(snapshot.mode(), ThemeMode::Light);
    assert_eq!(snapshot.revision(), 42);
}

#[test]
fn default_theme_snapshots_expose_distinct_modes_and_revisions() {
    let light = ThemeSnapshot::light();
    let dark = ThemeSnapshot::dark();
    let high_contrast = ThemeSnapshot::high_contrast();

    assert_eq!(light.mode().as_str(), "light");
    assert_eq!(dark.mode().as_str(), "dark");
    assert_eq!(high_contrast.mode().as_str(), "high-contrast");
    assert!(light.revision() < dark.revision());
    assert!(dark.revision() < high_contrast.revision());
    assert_ne!(
        light.color_rgb(semantic::SURFACE, ColorState::Default),
        dark.color_rgb(semantic::SURFACE, ColorState::Default)
    );
    assert_ne!(
        dark.color_rgb(semantic::FOCUS_RING, ColorState::FocusVisible),
        high_contrast.color_rgb(semantic::FOCUS_RING, ColorState::FocusVisible)
    );
}

#[test]
fn default_theme_resolves_all_current_component_color_intents() {
    let theme = [
        ThemeSnapshot::light(),
        ThemeSnapshot::dark(),
        ThemeSnapshot::high_contrast(),
    ];
    let buttons = [
        Button::new("default", "Default").state(),
        Button::new("secondary", "Secondary")
            .variant(ButtonVariant::Secondary)
            .state(),
        Button::new("outline", "Outline")
            .variant(ButtonVariant::Outline)
            .state(),
        Button::new("ghost", "Ghost")
            .variant(ButtonVariant::Ghost)
            .state(),
        Button::new("destructive", "Destructive")
            .variant(ButtonVariant::Destructive)
            .state(),
        Button::new("selected", "Selected").selected(true).state(),
    ];
    let badges = [
        Badge::new("default-badge", "Default").state(),
        Badge::new("secondary-badge", "Secondary")
            .variant(BadgeVariant::Secondary)
            .state(),
        Badge::new("destructive-badge", "Destructive")
            .variant(BadgeVariant::Destructive)
            .state(),
        Badge::new("outline-badge", "Outline")
            .variant(BadgeVariant::Outline)
            .state(),
    ];
    let avatars = [
        Avatar::new("avatar", "Ada Lovelace").state(),
        Avatar::new("source-avatar", "Ada Lovelace")
            .source("asset://avatars/ada.png")
            .state(),
    ];
    let status_cues = [
        StatusCue::new("status-neutral", "Neutral").state(),
        StatusCue::new("status-info", "Info")
            .intent(FeedbackIntent::Info)
            .state(),
        StatusCue::new("status-success", "Success")
            .intent(FeedbackIntent::Success)
            .state(),
        StatusCue::new("status-warning", "Warning")
            .intent(FeedbackIntent::Warning)
            .state(),
        StatusCue::new("status-danger", "Danger")
            .intent(FeedbackIntent::Danger)
            .state(),
    ];
    let empty_states = [
        EmptyState::new("empty-neutral", "Neutral").state(),
        EmptyState::new("empty-danger", "Danger")
            .description("Needs action")
            .intent(FeedbackIntent::Danger)
            .state(),
    ];
    let icon_buttons = [
        IconButton::new("search", "?", "Search").state(),
        IconButton::new("outline-icon", "+", "Add")
            .variant(ButtonVariant::Outline)
            .state(),
        IconButton::new("danger-icon", "!", "Delete")
            .variant(ButtonVariant::Destructive)
            .state(),
    ];
    let switches = [
        Switch::new("off").state(),
        Switch::new("on").checked(true).state(),
    ];
    let checkboxes = [
        Checkbox::new("unchecked").state(),
        Checkbox::new("checked").checked(true).state(),
        Checkbox::new("mixed").indeterminate(true).state(),
        Checkbox::new("invalid").invalid(true).state(),
    ];
    let radio_groups = [
        RadioGroup::new("plan")
            .default_selected("team")
            .item(RadioItem::new("personal", "Personal"))
            .item(RadioItem::new("team", "Team"))
            .state(),
        RadioGroup::new("disabled-plan")
            .disabled(true)
            .item(RadioItem::new("personal", "Personal"))
            .state(),
    ];
    let toggles = [
        Toggle::new("ghost-off", "Ghost off").state(),
        Toggle::new("ghost-on", "Ghost on").pressed(true).state(),
        Toggle::new("outline-on", "Outline on")
            .variant(ToggleVariant::Outline)
            .pressed(true)
            .state(),
    ];
    let text_inputs = [
        TextInput::new("default", "Default").state(),
        TextInput::new("disabled", "Disabled")
            .disabled(true)
            .state(),
        TextInput::new("readonly", "Read only")
            .read_only(true)
            .state(),
        TextInput::new("invalid", "Invalid").invalid(true).state(),
    ];
    let fields = [
        Field::new("field", "control", "Field").state(),
        Field::new("required", "control", "Required")
            .required(true)
            .state(),
        Field::new("disabled", "control", "Disabled")
            .disabled(true)
            .state(),
        Field::new("invalid", "control", "Invalid")
            .invalid(true)
            .state(),
    ];
    let labels = [
        Label::new("label", "Label").state(),
        Label::new("required-label", "Required")
            .required(true)
            .state(),
        Label::new("disabled-label", "Disabled")
            .disabled(true)
            .state(),
    ];
    let separators = [
        Separator::new("separator").state(),
        Separator::new("vertical-separator").vertical().state(),
    ];
    let kbds = [
        Kbd::new("kbd", "Ctrl+K").state(),
        Kbd::new("large-kbd", "Enter").large().state(),
    ];
    let progress = [
        Progress::new("progress", "Progress").value(50.0).state(),
        Progress::new("indeterminate-progress", "Progress")
            .indeterminate()
            .state(),
    ];
    let skeletons = [
        Skeleton::new("skeleton").state(),
        Skeleton::new("subtle-skeleton").subtle(true).state(),
    ];
    let menus = [
        Menu::new("menu", "Menu")
            .open(true)
            .item(MenuItem::action("open", "Open"))
            .state(),
        Menu::new("closed-menu", "Closed")
            .item(MenuItem::action("open", "Open"))
            .state(),
    ];
    let alert_dialogs = [
        AlertDialog::new(
            "alert",
            "Open",
            "Confirm",
            "Continue with changes.",
            "Continue",
        )
        .open(true)
        .state(),
        AlertDialog::new(
            "danger-alert",
            "Delete",
            "Delete item?",
            "This removes it.",
            "Delete",
        )
        .intent(AlertDialogIntent::Destructive)
        .open(true)
        .state(),
    ];
    let sheets = [
        Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content")
            .open(true)
            .state(),
        Sheet::new("closed-sheet", "Closed sheet", "Closed", "Closed content").state(),
    ];
    let hover_cards = [
        HoverCard::new("hover-card", "Profile", "Profile details")
            .open(true)
            .state(),
        HoverCard::element("closed-hover-card", "Details", div().child("Rich")).state(),
    ];
    let listboxes = [
        Listbox::new("listbox", "Choices")
            .selected("one")
            .option(ListboxOption::new("one", "One"))
            .option(ListboxOption::new("two", "Two").disabled(true))
            .state(),
        Listbox::new("empty-listbox", "Empty").state(),
    ];
    let selects = [
        Select::new("select", "Choice")
            .open(true)
            .selected("one")
            .option(ListboxOption::new("one", "One"))
            .state(),
        Select::new("closed-select", "Choice").state(),
    ];
    let comboboxes = [
        Combobox::new("combobox", "Search")
            .open(true)
            .default_query("one")
            .option(ComboboxOption::new("one", "One"))
            .state(),
        Combobox::new("closed-combobox", "Search").state(),
    ];
    let commands = [
        Command::new("command", "Commands")
            .open(true)
            .default_query("open")
            .item(CommandItem::new("open", "Open"))
            .state(),
        Command::new("closed-command", "Commands").state(),
    ];

    for state in buttons {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in badges {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in avatars {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in status_cues {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.marker(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in empty_states {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.marker(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in icon_buttons {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in switches {
        let colors = state.colors();
        for intent in [
            colors.track(),
            colors.thumb(),
            colors.border(),
            colors.label(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in checkboxes {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.hover_background(),
            colors.border(),
            colors.indicator(),
            colors.label(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in radio_groups {
        let colors = state.colors();
        for intent in [
            colors.control_background(),
            colors.control_background_selected(),
            colors.control_border(),
            colors.control_border_selected(),
            colors.indicator(),
            colors.label(),
            colors.label_muted(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in toggles {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in text_inputs {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.placeholder(),
            colors.border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in fields {
        let colors = state.colors();
        for intent in [colors.label(), colors.message(), colors.required_marker()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in labels {
        let colors = state.colors();
        for intent in [colors.text(), colors.required_marker()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in separators {
        let colors = state.colors();
        for intent in [colors.line()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in kbds {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in progress {
        let colors = state.colors();
        for intent in [colors.track(), colors.indicator()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in skeletons {
        let colors = state.colors();
        for intent in [colors.background()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in menus {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.border(),
            colors.item_background(),
            colors.item_hover_background(),
            colors.item_focus_background(),
            colors.item_disabled_foreground(),
            colors.separator(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in alert_dialogs {
        let colors = state.colors();
        for intent in [
            colors.barrier(),
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.action_background(),
            colors.action_hover_background(),
            colors.action_foreground(),
            colors.action_border(),
            colors.cancel_background(),
            colors.cancel_hover_background(),
            colors.cancel_foreground(),
            colors.cancel_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in sheets {
        let colors = state.colors();
        for intent in [
            colors.barrier(),
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.close_background(),
            colors.close_hover_background(),
            colors.close_foreground(),
            colors.close_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in hover_cards {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in listboxes {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.option_background(),
            colors.option_hover_background(),
            colors.option_active_background(),
            colors.option_selected_background(),
            colors.option_disabled_foreground(),
            colors.separator(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in selects {
        let colors = state.colors();
        for intent in [
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_placeholder_foreground(),
            colors.trigger_border(),
            colors.content_background(),
            colors.content_foreground(),
            colors.content_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in comboboxes {
        let colors = state.colors();
        for intent in [
            colors.popup_background(),
            colors.popup_foreground(),
            colors.popup_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in commands {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }
}

fn assert_theme_has_exact_color(
    themes: [ThemeSnapshot<'_>; 3],
    intent: open_gpui_ui_components::ColorIntent,
) {
    for theme in themes {
        assert!(
            theme
                .colors()
                .iter()
                .any(|entry| entry.token() == intent.token() && entry.state() == intent.state()),
            "missing {} theme color for {} / {}",
            theme.mode().as_str(),
            intent.token(),
            intent.state().as_str()
        );
    }
}

#[test]
fn theme_snapshots_resolve_state_specific_component_tokens() {
    let button = Button::new("secondary", "Secondary")
        .variant(ButtonVariant::Secondary)
        .state();
    let selected_switch = Switch::new("feature").checked(true).state();
    let mixed_checkbox = Checkbox::new("permissions").indeterminate(true).state();
    let disabled_input = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .state();
    let invalid_input = TextInput::new("email", "Email").invalid(true).state();
    let required_field = Field::new("email-field", "email", "Email")
        .required(true)
        .state();
    let theme = ThemeSnapshot::light();

    assert_eq!(
        button.colors().hover_background().state(),
        ColorState::Hover
    );
    assert_eq!(
        selected_switch.colors().track().state(),
        ColorState::Selected
    );
    assert_eq!(
        mixed_checkbox.colors().background().state(),
        ColorState::Selected
    );
    assert_eq!(
        disabled_input.colors().background().state(),
        ColorState::Disabled
    );
    assert_eq!(invalid_input.colors().border().state(), ColorState::Invalid);
    assert_eq!(
        invalid_input.colors().focus_ring().state(),
        ColorState::FocusVisible
    );
    assert_eq!(
        required_field.colors().required_marker().state(),
        ColorState::Required
    );
    assert_eq!(
        Label::new("required-label", "Required")
            .required(true)
            .state()
            .colors()
            .required_marker()
            .state(),
        ColorState::Required
    );

    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            button.colors().hover_background(),
            theme
        )),
        0xdfe6dcff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            disabled_input.colors().background(),
            theme
        )),
        0xf1f5eeff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            invalid_input.colors().focus_ring(),
            theme
        )),
        0x2f80edff
    );
}

#[test]
fn switch_label_uses_theme_text_token() {
    let tokens = custom_tokens();
    let state = Switch::new("feature").tokens(tokens).state();

    assert_eq!(state.colors().label().token(), tokens.text);
}

#[test]
fn checked_switch_maps_to_true_toggled_state() {
    let state = Switch::new("feature").checked(true).state();

    assert!(state.checked());
    assert_eq!(state.role(), Role::Switch);
    assert_eq!(state.toggled(), Toggled::True);
    assert_eq!(state.colors().track().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn unchecked_switch_maps_to_false_toggled_state() {
    let state = Switch::new("feature").state();

    assert!(!state.checked());
    assert_eq!(state.toggled(), Toggled::False);
    assert_eq!(state.colors().track().token(), semantic::SURFACE_MUTED);
}

#[test]
fn disabled_switch_keeps_role_but_blocks_activation_metadata() {
    let state = Switch::new("feature").disabled(true).state();

    assert_eq!(state.role(), Role::Switch);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn switch_size_metrics_are_deterministic() {
    let state = Switch::new("feature").small().state();
    let metrics = state.metrics();

    assert_eq!(state.size(), Size::Small);
    assert_eq!(metrics.track_width(), ui_px(32.0));
    assert_eq!(metrics.track_height(), ui_px(18.0));
    assert_eq!(metrics.thumb_size(), ui_px(14.0));
    assert_eq!(metrics.checked_thumb_x(), ui_px(16.0));
}

#[open_gpui::test]
fn switch_runtime_click_emits_on_change_with_next_checked(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        checked: Rc<RefCell<bool>>,
        changes: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let checked = *self.checked.borrow();
            let next_checked = self.checked.clone();
            let changes = self.changes.clone();
            let disabled_changes = self.changes.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Switch::new("runtime-switch")
                        .label("Runtime switch")
                        .checked(checked)
                        .on_change(move |checked, _, _, _| {
                            *next_checked.borrow_mut() = checked;
                            changes.borrow_mut().push(checked);
                        }),
                )
                .child(
                    Switch::new("disabled-runtime-switch")
                        .label("Disabled runtime switch")
                        .disabled(true)
                        .on_change(move |checked, _, _, _| {
                            disabled_changes.borrow_mut().push(checked);
                        }),
                )
        }
    }

    let checked = Rc::new(RefCell::new(false));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        checked: checked.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let disabled_switch = cx
        .debug_bounds("switch:disabled-runtime-switch:root")
        .expect("disabled switch should expose a stable debug selector");
    cx.simulate_click(disabled_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        changes.borrow().is_empty(),
        "disabled switch click should not emit on_change"
    );

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should expose a stable debug selector");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), true);
    assert_eq!(changes.borrow().as_slice(), &[true]);

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should remain rendered after controlled update");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), false);
    assert_eq!(changes.borrow().as_slice(), &[true, false]);
}

#[test]
fn checkbox_states_map_to_checkbox_role_and_toggled_values() {
    let unchecked = Checkbox::new("unchecked").state();
    let checked = Checkbox::new("checked").checked(true).state();
    let mixed = Checkbox::new("mixed").indeterminate(true).state();

    assert_eq!(unchecked.role(), Role::CheckBox);
    assert_eq!(unchecked.toggled(), Toggled::False);
    assert!(!unchecked.checked());
    assert!(!unchecked.indeterminate());

    assert_eq!(checked.role(), Role::CheckBox);
    assert_eq!(checked.toggled(), Toggled::True);
    assert!(checked.checked());
    assert!(!checked.indeterminate());

    assert_eq!(mixed.role(), Role::CheckBox);
    assert_eq!(mixed.toggled(), Toggled::Mixed);
    assert!(!mixed.checked());
    assert!(mixed.indeterminate());
}

#[test]
fn disabled_checkbox_blocks_activation_metadata() {
    let state = Checkbox::new("disabled").disabled(true).state();

    assert_eq!(state.role(), Role::CheckBox);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
    assert!(!state.tab_stop_enabled());
    assert_eq!(state.colors().background().state(), ColorState::Disabled);
}

#[test]
fn invalid_and_required_checkbox_expose_state_and_token_intents() {
    let tokens = custom_tokens();
    let state = Checkbox::new("terms")
        .checked(true)
        .required(true)
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.required());
    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().border().state(), ColorState::Invalid);
    assert_eq!(state.colors().background().token(), tokens.accent);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
}

#[test]
fn checkbox_checked_state_builder_accepts_mixed() {
    let state = Checkbox::new("bulk").checked_state(Toggled::Mixed).state();

    assert_eq!(state.toggled(), Toggled::Mixed);
    assert!(state.indeterminate());
    assert!(!state.checked());
}

#[test]
fn label_state_records_control_association_and_required_marker() {
    let tokens = custom_tokens();
    let state = Label::new("email-label", "Email")
        .for_control("email-input")
        .required(true)
        .tokens(tokens)
        .state();

    assert_eq!(state.role(), Role::Label);
    assert_eq!(state.text(), "Email");
    assert_eq!(state.control_id(), Some("email-input"));
    assert!(state.associated());
    assert!(state.required());
    assert_eq!(state.colors().text().token(), tokens.text);
    assert_eq!(state.colors().required_marker().token(), tokens.destructive);
}

#[test]
fn disabled_label_uses_muted_text_intent() {
    let tokens = custom_tokens();
    let state = Label::new("disabled-label", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().text().token(), tokens.text_muted);
    assert_eq!(state.colors().text().state(), ColorState::Disabled);
}

#[test]
fn default_text_input_state_uses_text_input_role_and_placeholder_display() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.input_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.input_px());
    assert!(!state.has_value());
    assert_eq!(state.display_text().as_ref(), "Email address");
    assert!(state.displaying_placeholder());
    assert!(state.editable());
}

#[test]
fn filled_text_input_reports_value_state() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .value("hello@example.com")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "hello@example.com");
    assert_eq!(state.display_text().as_ref(), "hello@example.com");
    assert!(!state.displaying_placeholder());
}

#[test]
fn password_text_input_masks_display_without_hiding_value() {
    let state = TextInput::new("password", "Password")
        .placeholder("Password")
        .value("a🙂中")
        .display_mode(TextInputDisplayMode::Password)
        .state();

    assert_eq!(state.value(), "a🙂中");
    assert_eq!(state.display_mode(), TextInputDisplayMode::Password);
    assert_eq!(state.display_text().as_ref(), "•••");
    assert!(state.display_mode().masks_value());
    assert!(!state.displaying_placeholder());
}

#[test]
fn controlled_text_input_on_change_marks_input_controller_driven() {
    let state = TextInput::new("email", "Email")
        .value("hello@example.com")
        .on_change(|_, _, _| {})
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
    assert_eq!(state.value(), "hello@example.com");
}

#[test]
fn disabled_and_read_only_text_inputs_block_editability() {
    let tokens = custom_tokens();
    let disabled = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = TextInput::new("readonly", "Read only")
        .read_only(true)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(!disabled.activation_enabled());
    assert_eq!(disabled.colors().background().token(), tokens.surface_muted);
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(!read_only.activation_enabled());
    assert_eq!(
        read_only.colors().background().token(),
        ThemeTokens::default().surface_muted
    );
    assert_eq!(read_only.role(), Role::TextInput);
}

#[test]
fn invalid_text_input_uses_destructive_border_token() {
    let tokens = custom_tokens();
    let state = TextInput::new("email", "Email")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
    assert_eq!(state.colors().placeholder().token(), tokens.text_muted);
}

#[test]
fn focus_ring_preserves_token_intent_without_layout_shift() {
    let ring = FocusRing::from_color(Button::new("save", "Save").state().colors().focus_ring());
    let shadow = focus_ring_shadow(ring);

    assert_eq!(ring.color().token(), semantic::FOCUS_RING);
    assert_eq!(ring.width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!ring.changes_layout());
    assert_eq!(shadow[0].spread_radius, px(2.0));
    assert_eq!(shadow[0].blur_radius, px(0.0));
    assert!(!shadow[0].inset);
}

#[test]
fn text_input_size_helpers_apply_input_metrics() {
    let state = TextInput::new("query", "Search").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[open_gpui::test]
fn text_input_controller_converts_utf16_ranges_and_replaces_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a🙂中", cx));

    cx.update_entity(&controller, |controller, cx| {
        let mut adjusted = None;

        assert_eq!(
            controller
                .text_for_range_utf16(1..3, &mut adjusted)
                .as_deref(),
            Some("🙂")
        );
        assert_eq!(adjusted, Some(1..3));

        controller.select_range(1.."a🙂".len(), cx);
        controller.replace_text_in_range_utf16(None, "b\nc", cx);

        assert_eq!(controller.value(), "ab c中");
        assert_eq!(controller.selected_range(), 4..4);
        assert_eq!(controller.selected_range_utf16(), 4..4);
    });
}

#[open_gpui::test]
fn text_input_controller_updates_marked_text_and_commits_composition(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(TextInputController::new);

    cx.update_entity(&controller, |controller, cx| {
        controller.replace_and_mark_text_in_range_utf16(None, "ni", Some(1..2), cx);

        assert_eq!(controller.value(), "ni");
        assert_eq!(controller.marked_range_utf16(), Some(0..2));
        assert_eq!(controller.selected_range_utf16(), 1..2);

        controller.replace_text_in_range_utf16(None, "你", cx);

        assert_eq!(controller.value(), "你");
        assert_eq!(controller.marked_range_utf16(), None);
        assert_eq!(controller.selected_range_utf16(), 1..1);
    });
}

#[open_gpui::test]
fn text_input_controller_delete_commands_respect_grapheme_boundaries(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a👨‍👩‍👧‍👦b", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.move_to_offset("a👨‍👩‍👧‍👦".len(), cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "ab");

        controller.move_to_offset(1, cx);
        controller.delete_forward(cx);

        assert_eq!(controller.value(), "a");
    });
}

#[open_gpui::test]
fn text_input_controller_rejects_editing_when_disabled_or_read_only(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("locked", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.set_read_only(true, cx);
        controller.select_range(0..controller.value().len(), cx);
        controller.replace_text_in_range_utf16(None, "changed", cx);

        assert_eq!(controller.value(), "locked");

        controller.set_read_only(false, cx);
        controller.set_disabled(true, cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "locked");
        assert!(!controller.accepts_editing());
    });
}

#[open_gpui::test]
fn text_input_runtime_accepts_controller_backed_simulated_input(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controller: open_gpui::Entity<TextInputController>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                TextInput::new("runtime-text-input", "Runtime text input")
                    .controller(self.controller.clone())
                    .placeholder("Type here"),
            )
        }
    }

    cx.update(init_text_input);
    let controller = cx.new(TextInputController::new);
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        controller: controller.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-text-input:root")
        .expect("standalone text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.update_entity(&controller, |controller, _| {
        assert_eq!(controller.value(), "hello world");
        assert_eq!(
            controller.selected_range(),
            controller.value().len()..controller.value().len()
        );
    });
}

#[open_gpui::test]
fn controlled_text_input_on_change_accepts_input_without_supplied_controller(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                TextInput::new("controlled-text-input", "Controlled text input")
                    .value(value)
                    .placeholder("Type here")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-text-input:root")
        .expect("controlled text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "hello world");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("hello world")
    );
}

#[open_gpui::test]
fn text_input_state_marks_controller_driven_inputs(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("editable", "Editable")
        .controller(controller)
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
}

#[open_gpui::test]
fn controller_driven_text_input_state_marks_disabled_editing(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("disabled", "Disabled")
        .controller(controller)
        .disabled(true)
        .state();

    assert!(state.controller_driven());
    assert!(state.disabled());
    assert!(!state.editable());
}

#[test]
fn default_textarea_state_uses_text_input_role_and_rows() {
    let state = Textarea::new("notes", "Notes")
        .placeholder("Release notes")
        .rows(4)
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.rows(), 4);
    assert_eq!(state.metrics().rows(), 4);
    assert!(state.placeholder_visible());
    assert_eq!(state.display_text(), "Release notes");
    assert!(state.editable());
    assert!(!state.controller_driven());
}

#[test]
fn filled_textarea_preserves_newlines_in_state() {
    let state = Textarea::new("notes", "Notes")
        .value("Line 1\r\nLine 2")
        .placeholder("Release notes")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "Line 1\nLine 2");
    assert_eq!(state.display_text(), "Line 1\nLine 2");
    assert!(!state.displaying_placeholder());
}

#[test]
fn disabled_read_only_and_invalid_textareas_expose_control_state() {
    let tokens = custom_tokens();
    let disabled = Textarea::new("disabled-notes", "Disabled notes")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = Textarea::new("readonly-notes", "Read-only notes")
        .read_only(true)
        .state();
    let invalid = Textarea::new("invalid-notes", "Invalid notes")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(invalid.invalid());
    assert_eq!(invalid.colors().border().token(), tokens.destructive);
}

#[open_gpui::test]
fn controlled_textarea_on_change_preserves_newline_input(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Textarea::new("controlled-textarea", "Controlled textarea")
                    .value(value)
                    .placeholder("Type notes")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("textarea:controlled-textarea:root")
        .expect("controlled textarea should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("Line 1\nLine 2");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "Line 1\nLine 2");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("Line 1\nLine 2")
    );
}

#[test]
fn default_field_state_exposes_label_help_and_metrics() {
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .state();

    assert_eq!(state.label(), "Email");
    assert_eq!(state.help().unwrap(), "Use a work address.");
    assert_eq!(state.support_text().unwrap(), "Use a work address.");
    assert!(!state.support_is_error());
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(
        state.metrics().label_text_size(),
        Size::Medium.control_text_px()
    );
}

#[test]
fn required_field_exposes_required_metadata() {
    let state = Field::new("email-field", "email", "Email")
        .required(true)
        .state();

    assert!(state.required());
    assert_eq!(
        state.colors().required_marker().token(),
        semantic::DESTRUCTIVE
    );
}

#[test]
fn invalid_field_prefers_error_support_text() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .error("Enter a valid email.")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.support_text().unwrap(), "Enter a valid email.");
    assert!(state.support_is_error());
    assert_eq!(state.colors().message().token(), tokens.destructive);
}

#[test]
fn disabled_field_uses_muted_label_intent() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().message().token(), tokens.text_muted);
}
