#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

mod a11y;
pub mod accordion;
pub mod alert_dialog;
pub mod avatar;
pub mod badge;
pub mod breadcrumb;
pub mod button;
pub mod checkbox;
mod choice;
pub mod collapsible;
pub mod color;
pub mod combobox;
pub mod command;
pub mod component_contract;
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
pub mod link;
pub mod listbox;
pub mod menu;
pub mod number_input;
mod overlay;
pub mod popover;
pub mod prelude;
pub mod primitives;
pub mod progress;
mod public_api;
pub mod radio;
pub mod roving_focus;
pub mod scroll_area;
pub mod select;
pub mod separator;
pub mod sheet;
pub mod sidebar;
pub mod skeleton;
pub mod slider;
pub mod splitter;
pub mod switch;
pub mod table;
pub mod tabs;
pub mod tag;
mod text_editing;
pub mod text_input;
pub mod textarea;
pub mod theme;
pub mod toast;
pub mod toggle;
pub mod toggle_group;
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

pub use public_api::default::*;
