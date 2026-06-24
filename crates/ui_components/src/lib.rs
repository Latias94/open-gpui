#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

mod a11y;
pub mod alert_dialog;
pub mod avatar;
pub mod badge;
pub mod button;
pub mod checkbox;
pub mod color;
pub mod combobox;
pub mod command;
pub mod context_menu;
pub mod dialog;
pub mod feedback;
pub mod field;
mod focus;
mod geometry;
pub mod hover_card;
pub mod icon_button;
pub mod kbd;
pub mod label;
pub mod listbox;
pub mod menu;
mod overlay;
pub mod popover;
pub mod prelude;
pub mod progress;
pub mod radio;
pub mod roving_focus;
pub mod scroll_area;
pub mod select;
pub mod separator;
pub mod sheet;
pub mod sidebar;
pub mod skeleton;
pub mod splitter;
pub mod switch;
pub mod table;
pub mod tabs;
pub mod text_input;
pub mod theme;
pub mod toggle;
pub mod toolbar;
pub mod tooltip;
pub mod tree;
pub mod virtualized_list;

/// GPUI-specific adapter APIs that are intentionally outside renderer-neutral component state.
///
/// These exports remain public for applications that use the concrete GPUI components, but a
/// future headless crate should not depend on them as component contracts.
pub mod gpui_adapter {
    pub use crate::a11y::{
        UiA11yElementExt, gpui_accessible_action_from_ui, gpui_orientation_from_ui,
        gpui_role_from_ui, gpui_toggled_from_ui,
    };
    pub use crate::focus::focus_ring_shadow;
    pub use crate::geometry::{gpui_point_from_ui, gpui_px_from_ui, gpui_size_from_ui};
    pub use crate::overlay::{
        DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement,
        GpuiOverlayState, OverlayOpenChange, default_deferred_priority, escape_open_change,
        gpui_anchor, gpui_overlay_state, outside_press_open_change, point_anchor_placement,
    };
    pub use crate::text_input::adapter::{TextInputController, init as init_text_input};
}

pub use alert_dialog::{
    AlertDialog, AlertDialogActionKind, AlertDialogActionState, AlertDialogColors,
    AlertDialogIntent, AlertDialogMetrics, AlertDialogOpenMode, AlertDialogState,
};
pub use avatar::{Avatar, AvatarColors, AvatarMetrics, AvatarSource, AvatarState};
pub use badge::{Badge, BadgeColors, BadgeMetrics, BadgeState, BadgeVariant};
pub use button::{Button, ButtonColors, ButtonMetrics, ButtonState, ButtonVariant};
pub use checkbox::{Checkbox, CheckboxColors, CheckboxMetrics, CheckboxState};
pub use color::{ColorIntent, ColorState};
pub use combobox::{
    Combobox, ComboboxColors, ComboboxGroup, ComboboxGroupDescriptor, ComboboxMetrics,
    ComboboxOpenMode, ComboboxOption, ComboboxOptionDescriptor, ComboboxSelection, ComboboxState,
};
pub use command::{
    Command, CommandColors, CommandDialogState, CommandGroup, CommandGroupDescriptor,
    CommandIndexSnapshot, CommandIndexSnapshotMode, CommandItem, CommandItemDescriptor,
    CommandItemState, CommandLoadingState, CommandMatchSource, CommandMetrics, CommandOpenMode,
    CommandQueryMode, CommandRenderPlan, CommandRowRenderPlan, CommandSelectedChipState,
    CommandSelection, CommandSelectionChange, CommandSelectionMode, CommandState,
};
pub use context_menu::{ContextMenu, ContextMenuState};
pub use dialog::{Dialog, DialogColors, DialogMetrics, DialogOpenMode, DialogState};
pub use feedback::{
    EmptyState, EmptyStateMetrics, EmptyStateState, FeedbackColors, FeedbackIntent, StatusCue,
    StatusCueMetrics, StatusCueState,
};
pub use field::{Field, FieldColors, FieldMessage, FieldMetrics, FieldState};
pub use focus::{DEFAULT_FOCUS_RING_WIDTH, FocusRing};
pub use hover_card::{
    HoverCard, HoverCardColors, HoverCardContentKind, HoverCardDelayPolicy, HoverCardMetrics,
    HoverCardOpenIntent, HoverCardOpenMode, HoverCardState,
};
pub use icon_button::{IconButton, IconButtonColors, IconButtonMetrics, IconButtonState};
pub use kbd::{Kbd, KbdColors, KbdMetrics, KbdState};
pub use label::{Label, LabelColors, LabelMetrics, LabelState};
pub use listbox::{
    Listbox, ListboxColors, ListboxGroup, ListboxGroupDescriptor, ListboxGroupState,
    ListboxMetrics, ListboxOption, ListboxOptionDescriptor, ListboxOptionKind, ListboxOptionState,
    ListboxSelection, ListboxState, listbox_navigation_target,
};
pub use menu::{
    Menu, MenuColors, MenuItem, MenuItemDescriptor, MenuItemKind, MenuItemState, MenuMetrics,
    MenuOpenMode, MenuSelection, MenuState, MenuSubmenuNavigation, menu_navigation_target,
};
pub use open_gpui_ui_core::{
    GridViewport2D, TABLE_DEFAULT_COLUMN_WIDTH, TABLE_MAX_COLUMN_WIDTH, TABLE_MIN_COLUMN_WIDTH,
    TABLE_ROW_MODEL_PIPELINE, TABLE_ROW_MODEL_V0_PIPELINE, TableAggregateKind, TableAggregation,
    TableCellValue, TableColumn, TableColumnFacets, TableColumnId, TableColumnPinning,
    TableColumnRegion, TableColumnRegions, TableColumnResizeDirection, TableColumnResizeMode,
    TableColumnResizeState, TableColumnResizeUpdate, TableColumnSizing, TableExpansionMode,
    TableExpansionState, TableFacetRange, TableFacetValueCount, TableFilter, TableFilterKind,
    TableGroupRow, TablePagination, TableResolvedColumnSizing, TableResolvedColumnSizingRegions,
    TableResolvedRow, TableResolvedRowKind, TableResolvedState, TableRow,
    TableRowChildrenLoadState, TableRowId, TableRowModel, TableRowModelStage, TableRowPinning,
    TableRowPinningPolicy, TableRowRegion, TableRowRegions, TableSort, TableSortDirection,
    TableStageMode, TableState, TableStateCacheKey, TableTreeRow, VirtualizerItemKey,
    VirtualizerItemMeasurement, VirtualizerRange, VirtualizerResolvedState, VirtualizerSnapshot,
    VirtualizerSnapshotItem, VirtualizerState, drag_table_column_resize, end_table_column_resize,
    resolve_grid_viewport_2d,
};
pub use open_gpui_ui_core::{
    TableSelectionActivationMode, TableSelectionMode, TableSelectionPolicy, TableSelectionSummary,
    TableSelectionSummaryState, TableSubRowSelectionPolicy,
};
pub use overlay::OverlayResolvedState;
pub use popover::{Popover, PopoverColors, PopoverMetrics, PopoverOpenMode, PopoverState};
pub use progress::{Progress, ProgressColors, ProgressMetrics, ProgressState, ProgressVisualMode};
pub use radio::{
    RadioGroup, RadioGroupColors, RadioGroupMetrics, RadioGroupState, RadioItem,
    RadioItemDescriptor, RadioItemState, RadioSelection,
};
pub use roving_focus::{active_index_from_str_keys, first_enabled, last_enabled, next_enabled};
pub use scroll_area::{
    ScrollArea, ScrollAreaAxis, ScrollAreaMetrics, ScrollAreaState, ScrollResetPolicy,
};
pub use select::{
    Select, SelectColors, SelectMetrics, SelectOpenMode, SelectSelection, SelectState,
};
pub use separator::{Separator, SeparatorColors, SeparatorMetrics, SeparatorState};
pub use sheet::{
    Sheet, SheetCloseAffordance, SheetColors, SheetMetrics, SheetModalMode, SheetOpenMode,
    SheetSide, SheetState,
};
pub use sidebar::{
    Sidebar, SidebarCollapseMode, SidebarColors, SidebarItem, SidebarItemDescriptor,
    SidebarItemState, SidebarMetrics, SidebarSection, SidebarSectionDescriptor,
    SidebarSectionState, SidebarSelection, SidebarSide, SidebarState, SidebarVariant,
    sidebar_navigation_target,
};
pub use skeleton::{Skeleton, SkeletonColors, SkeletonMetrics, SkeletonState};
pub use splitter::{
    Splitter, SplitterHandleState, SplitterMetrics, SplitterPanel, SplitterPanelDescriptor,
    SplitterPanelState, SplitterState,
};
pub use switch::{Switch, SwitchColors, SwitchMetrics, SwitchState};
pub use table::{
    Table, TableCellRenderPlan, TableCenterColumnWindowPlan, TableColumnRegionRenderPlan,
    TableColumnRenderPlan, TableColumnSizingChange, TableFacetedFilter, TableFacetedFilterChange,
    TableFacetedFilterOptionState, TableFacetedFilterState, TableHeaderAction, TableInputModifiers,
    TableMetrics, TablePinnedLayoutPlan, TableRenderPlan, TableRowAction, TableRowActivation,
    TableRowActivationKind, TableRowExpansionToggle, TableRowRenderPlan, TableRowSelectionChange,
    TableSelectionScope,
};
pub use tabs::{
    Tabs, TabsActivationMode, TabsColors, TabsItem, TabsItemDescriptor, TabsItemState, TabsMetrics,
    TabsSelection, TabsState,
};
pub use text_input::{TextInput, TextInputColors, TextInputMetrics, TextInputState};
pub use theme::{ThemeColor, ThemeMode, ThemeResolver, ThemeSnapshot};
pub use toggle::{Toggle, ToggleColors, ToggleMetrics, ToggleState, ToggleVariant};
pub use toolbar::{
    Toolbar, ToolbarColors, ToolbarItem, ToolbarItemDescriptor, ToolbarItemKind, ToolbarItemState,
    ToolbarMetrics, ToolbarSelection, ToolbarState, toolbar_navigation_target,
};
pub use tooltip::{
    Tooltip, TooltipColors, TooltipContentKind, TooltipDelayPolicy, TooltipMetrics,
    TooltipOpenIntent, TooltipState,
};
pub use tree::{
    Tree, TreeFocusTarget, TreeItemDescriptor, TreeItemState, TreeKeyboardAction, TreeMetrics,
    TreeSelection, TreeState, TreeToggle, tree_navigation_target,
};
pub use virtualized_list::{
    VirtualizedList, VirtualizedListActivation, VirtualizedListItemDescriptor,
    VirtualizedListMetrics, VirtualizedListRenderPlan, VirtualizedListRowRenderPlan,
    VirtualizedListScrollStrategy, VirtualizedListState, virtualized_list_navigation_target,
    virtualized_list_scroll_target,
};
