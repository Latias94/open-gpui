#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

mod a11y;
pub mod accordion;
pub mod action;
mod activation;
pub mod alert_dialog;
pub mod avatar;
pub mod badge;
pub mod breadcrumb;
pub mod button;
pub mod checkbox;
mod choice;
mod choice_overlay_runtime;
pub mod collapsible;
mod collection_typeahead;
pub mod color;
pub mod combobox;
pub mod command;
pub mod common;
pub mod component_contract;
pub mod context_menu;
mod debug_selector;
pub mod dialog;
pub mod feedback;
pub mod field;
mod focus;
pub mod form_adapter;
mod form_control;
mod geometry;
pub mod hover_card;
pub mod icon_button;
pub mod kbd;
pub mod label;
pub mod link;
pub mod listbox;
pub mod menu;
mod motion_adapter;
pub mod number_input;
mod overlay;
pub mod popover;
pub mod prelude;
pub mod primitives;
pub mod progress;
mod public_api;
pub mod radio;
pub mod resource_adapter;
mod roving_focus;
pub mod scroll_area;
mod scroll_surface;
pub mod select;
pub mod separator;
pub mod sheet;
pub mod sidebar;
pub mod skeleton;
pub mod slider;
pub mod splitter;
mod stable_value_focus;
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
        UiA11yElementExt, gpui_accessible_action_from_ui, gpui_live_from_ui,
        gpui_orientation_from_ui, gpui_role_from_ui, gpui_toggled_from_ui,
    };
    pub use crate::field::adapter::{FieldControl, FieldControlSemantics};
    pub use crate::focus::focus_ring_shadow_with_theme;
    pub use crate::geometry::{gpui_point_from_ui, gpui_px_from_ui, gpui_size_from_ui};
    pub use crate::motion_adapter::subtree_transform_from_motion_projection;
    pub use crate::overlay::{
        DEFAULT_OVERLAY_SAFE_MARGIN, FocusScopeRuntimeError, FocusTargetRegistration,
        GpuiOverlayAdapterConfig, GpuiOverlayPlacement, GpuiOverlayState, OverlayFocusMode,
        OverlayFocusRestoreCondition, OverlayFocusTargetLease, OverlayInsideRegionId,
        OverlayLayerBinding, OverlayLayerGeneration, OverlayLayerLease, OverlayLayerPhase,
        OverlayLayerRegistration, OverlayLayerSnapshot, OverlayOpenIntent,
        OverlayOpenIntentRevision, OverlayOwnership, OverlaySurface, OverlayTabBehavior,
        WindowFocusFallbackLease, WindowOverlayRuntime, WindowOverlayRuntimeError,
        WindowOverlaySnapshot, default_deferred_priority, gpui_anchor, gpui_overlay_state,
        point_anchor_placement,
    };
    pub use crate::table::TableDebugSelector;
    pub use crate::text_input::adapter::{TextInputController, init as init_text_input};
    pub use crate::virtualized_list::runtime::VirtualizedListGpuiExt;
}

pub use public_api::default::*;
