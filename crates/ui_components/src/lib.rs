#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

pub mod a11y;
pub mod alert_dialog;
pub mod badge;
pub mod button;
pub mod checkbox;
pub mod color;
pub mod combobox;
pub mod command;
pub mod context_menu;
pub mod dialog;
pub mod field;
pub mod focus;
mod geometry;
pub mod hover_card;
pub mod icon_button;
pub mod label;
pub mod listbox;
pub mod menu;
pub mod overlay;
pub mod popover;
pub mod prelude;
pub mod radio;
pub mod roving_focus;
pub mod scroll_area;
pub mod select;
pub mod sheet;
pub mod sidebar;
pub mod splitter;
pub mod switch;
pub mod tabs;
pub mod text_input;
pub mod theme;
pub mod toggle;
pub mod toolbar;
pub mod tooltip;

pub use a11y::{
    UiA11yElementExt, gpui_accessible_action_from_ui, gpui_orientation_from_ui, gpui_role_from_ui,
    gpui_toggled_from_ui,
};
pub use alert_dialog::{
    AlertDialog, AlertDialogActionKind, AlertDialogActionState, AlertDialogColors,
    AlertDialogIntent, AlertDialogMetrics, AlertDialogOpenMode, AlertDialogState,
};
pub use badge::{Badge, BadgeColors, BadgeMetrics, BadgeState, BadgeVariant};
pub use button::{Button, ButtonColors, ButtonMetrics, ButtonState, ButtonVariant};
pub use checkbox::{Checkbox, CheckboxColors, CheckboxMetrics, CheckboxState};
pub use color::{ColorIntent, ColorState};
pub use combobox::{
    Combobox, ComboboxColors, ComboboxGroup, ComboboxGroupDescriptor, ComboboxMetrics,
    ComboboxOpenMode, ComboboxOption, ComboboxOptionDescriptor, ComboboxSelection, ComboboxState,
};
pub use command::{
    Command, CommandColors, CommandDialogState, CommandGroup, CommandGroupDescriptor, CommandItem,
    CommandItemDescriptor, CommandItemState, CommandLoadingState, CommandMetrics, CommandOpenMode,
    CommandSelection, CommandState,
};
pub use context_menu::{ContextMenu, ContextMenuState};
pub use dialog::{Dialog, DialogColors, DialogMetrics, DialogOpenMode, DialogState};
pub use field::{Field, FieldColors, FieldMessage, FieldMetrics, FieldState};
pub use focus::{DEFAULT_FOCUS_RING_WIDTH, FocusRing, focus_ring_shadow};
pub use hover_card::{
    HoverCard, HoverCardColors, HoverCardContentKind, HoverCardDelayPolicy, HoverCardMetrics,
    HoverCardOpenIntent, HoverCardOpenMode, HoverCardState,
};
pub use icon_button::{IconButton, IconButtonColors, IconButtonMetrics, IconButtonState};
pub use label::{Label, LabelColors, LabelMetrics, LabelState};
pub use listbox::{
    Listbox, ListboxColors, ListboxGroup, ListboxGroupDescriptor, ListboxGroupState,
    ListboxMetrics, ListboxOption, ListboxOptionDescriptor, ListboxOptionKind, ListboxOptionState,
    ListboxSelection, ListboxState, listbox_navigation_target,
};
pub use menu::{
    Menu, MenuColors, MenuItem, MenuItemDescriptor, MenuItemKind, MenuItemState, MenuMetrics,
    MenuOpenMode, MenuSelection, MenuState, menu_navigation_target,
};
pub use overlay::{
    DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement, GpuiOverlayState,
    OverlayOpenChange, OverlayResolvedState, default_deferred_priority, escape_open_change,
    gpui_anchor, outside_press_open_change, point_anchor_placement,
};
pub use popover::{Popover, PopoverColors, PopoverMetrics, PopoverOpenMode, PopoverState};
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
pub use splitter::{
    Splitter, SplitterHandleState, SplitterMetrics, SplitterPanel, SplitterPanelDescriptor,
    SplitterPanelState, SplitterState,
};
pub use switch::{Switch, SwitchColors, SwitchMetrics, SwitchState};
pub use tabs::{
    Tabs, TabsActivationMode, TabsColors, TabsItem, TabsItemDescriptor, TabsItemState, TabsMetrics,
    TabsSelection, TabsState,
};
pub use text_input::{
    TextInput, TextInputColors, TextInputController, TextInputMetrics, TextInputState,
    init as init_text_input,
};
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
