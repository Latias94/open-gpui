//! GPUI adapter helpers for shared overlay behavior.

mod adapter;
mod placement;
mod runtime;

pub use adapter::{
    DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayState,
    default_deferred_priority, gpui_overlay_state,
};
pub(crate) use adapter::{
    gpui_full_window_overlay_layer, gpui_positioned_overlay_layer, gpui_relative_overlay_layer,
};
pub use open_gpui_ui_core::OverlayResolvedState;
pub use placement::{GpuiOverlayPlacement, gpui_anchor, point_anchor_placement};
pub(crate) use runtime::{
    OverlayDisclosureConfig, OverlayDisclosureOpenMode, consume_overlay_event,
    emit_overlay_open_change, resolve_overlay_open_state, restore_overlay_focus, set_overlay_open,
};
pub use runtime::{OverlayOpenChange, escape_open_change, outside_press_open_change};

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{
        EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
        OverlayLayerKind,
    };

    #[test]
    fn overlay_runtime_state_resolves_controlled_without_emitting() {
        let uncontrolled = resolve_overlay_open_state(None, true);
        assert!(uncontrolled.open());
        assert!(!uncontrolled.controlled());
        assert!(!uncontrolled.runtime_changed());

        let controlled_same = resolve_overlay_open_state(Some(true), true);
        assert!(controlled_same.open());
        assert!(controlled_same.controlled());
        assert!(!controlled_same.runtime_changed());

        let controlled_changed = resolve_overlay_open_state(Some(false), true);
        assert!(!controlled_changed.open());
        assert!(controlled_changed.controlled());
        assert!(controlled_changed.runtime_changed());
    }

    #[test]
    fn overlay_disclosure_state_resolves_open_mode_and_policy() {
        let state = OverlayDisclosureConfig::new(OverlayLayerKind::Modal)
            .controlled_open(Some(true))
            .default_open(false)
            .outside_press_policy(OutsidePressPolicy::Ignore)
            .escape_key_policy(EscapeKeyPolicy::Dismiss)
            .initial_focus_intent(InitialFocusIntent::FirstFocusable)
            .focus_restore_intent(FocusRestoreIntent::Trigger)
            .resolve();

        assert!(state.open());
        assert_eq!(state.open_mode(), OverlayDisclosureOpenMode::Controlled);
        assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
        assert_eq!(
            state.overlay().policy().outside_press_policy(),
            OutsidePressPolicy::Ignore
        );
        assert_eq!(
            state.overlay().policy().escape_key_policy(),
            EscapeKeyPolicy::Dismiss
        );
        assert_eq!(
            state.overlay().policy().initial_focus_intent(),
            &InitialFocusIntent::FirstFocusable
        );
        assert_eq!(
            state.overlay().policy().focus_restore_intent(),
            &FocusRestoreIntent::Trigger
        );
    }

    #[test]
    fn overlay_disclosure_state_gates_disabled_and_unopenable_surfaces() {
        let disabled = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .default_open(true)
            .disabled(true)
            .resolve();
        let unopenable = OverlayDisclosureConfig::new(OverlayLayerKind::Menu)
            .default_open(true)
            .openable(false)
            .resolve();

        assert!(!disabled.open());
        assert_eq!(
            disabled.open_mode(),
            OverlayDisclosureOpenMode::Uncontrolled
        );
        assert!(!disabled.overlay().policy().presence().interactive());

        assert!(!unopenable.open());
        assert_eq!(unopenable.overlay().policy().kind(), OverlayLayerKind::Menu);
        assert!(!unopenable.overlay().policy().presence().interactive());
    }
}
