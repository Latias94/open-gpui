#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

pub mod badge;
pub mod button;
pub mod checkbox;
pub mod color;
pub mod field;
pub mod focus;
pub mod icon_button;
pub mod label;
pub mod overlay;
pub mod popover;
pub mod prelude;
pub mod radio;
pub mod switch;
pub mod tabs;
pub mod text_input;
pub mod theme;
pub mod toggle;
pub mod tooltip;

pub use badge::{Badge, BadgeColors, BadgeMetrics, BadgeState, BadgeVariant};
pub use button::{Button, ButtonColors, ButtonMetrics, ButtonState, ButtonVariant};
pub use checkbox::{Checkbox, CheckboxColors, CheckboxMetrics, CheckboxState};
pub use color::{ColorIntent, ColorState};
pub use field::{Field, FieldColors, FieldMessage, FieldMetrics, FieldState};
pub use focus::{DEFAULT_FOCUS_RING_WIDTH, FocusRing, focus_ring_shadow};
pub use icon_button::{IconButton, IconButtonColors, IconButtonMetrics, IconButtonState};
pub use label::{Label, LabelColors, LabelMetrics, LabelState};
pub use overlay::{
    DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement, GpuiOverlayState,
    OverlayOpenChange, default_deferred_priority, escape_open_change, gpui_anchor,
    outside_press_open_change, point_anchor_placement,
};
pub use popover::{Popover, PopoverColors, PopoverMetrics, PopoverOpenMode, PopoverState};
pub use radio::{
    RadioGroup, RadioGroupColors, RadioGroupMetrics, RadioGroupState, RadioItem,
    RadioItemDescriptor, RadioItemState, RadioSelection,
};
pub use switch::{Switch, SwitchColors, SwitchMetrics, SwitchState};
pub use tabs::{
    Tabs, TabsActivationMode, TabsColors, TabsItem, TabsItemDescriptor, TabsItemState, TabsMetrics,
    TabsSelection, TabsState, active_index_from_str_keys, first_enabled, last_enabled,
    next_enabled,
};
pub use text_input::{
    TextInput, TextInputColors, TextInputController, TextInputMetrics, TextInputState,
    init as init_text_input,
};
pub use theme::{ThemeColor, ThemeMode, ThemeResolver, ThemeSnapshot};
pub use toggle::{Toggle, ToggleColors, ToggleMetrics, ToggleState, ToggleVariant};
pub use tooltip::{
    Tooltip, TooltipColors, TooltipContentKind, TooltipDelayPolicy, TooltipMetrics,
    TooltipOpenIntent, TooltipState,
};
