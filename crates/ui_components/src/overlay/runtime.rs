//! GPUI runtime helpers for overlay-like component adapters.

use open_gpui_ui_core::{
    ControllableState, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayLayerKind, OverlayPresence,
};

use super::OverlayResolvedState;
use super::adapter::GpuiOverlayAdapterConfig;

/// Shared open-state ownership for overlay-like component adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum OverlayDisclosureOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Input used to resolve renderer-neutral disclosure state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverlayDisclosureConfig {
    controlled_open: Option<bool>,
    default_open: bool,
    disabled: bool,
    openable: bool,
    layer_kind: OverlayLayerKind,
    outside_press_policy: Option<OutsidePressPolicy>,
    escape_key_policy: Option<EscapeKeyPolicy>,
    initial_focus_intent: Option<InitialFocusIntent>,
    focus_restore_intent: Option<FocusRestoreIntent>,
}

impl OverlayDisclosureConfig {
    /// Creates a disclosure config for an overlay layer kind.
    pub(crate) fn new(layer_kind: OverlayLayerKind) -> Self {
        Self {
            controlled_open: None,
            default_open: false,
            disabled: false,
            openable: true,
            layer_kind,
            outside_press_policy: None,
            escape_key_policy: None,
            initial_focus_intent: None,
            focus_restore_intent: None,
        }
    }

    /// Applies caller-owned open state.
    pub(crate) const fn controlled_open(mut self, open: Option<bool>) -> Self {
        self.controlled_open = open;
        self
    }

    /// Applies uncontrolled initial open state.
    pub(crate) const fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Applies disabled state.
    pub(crate) const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies whether this disclosure has content that can be opened.
    pub(crate) const fn openable(mut self, openable: bool) -> Self {
        self.openable = openable;
        self
    }

    /// Applies outside-press dismissal policy.
    pub(crate) fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = Some(policy);
        self
    }

    /// Applies Escape-key dismissal policy.
    pub(crate) fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = Some(policy);
        self
    }

    /// Applies initial-focus intent.
    pub(crate) fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = Some(intent);
        self
    }

    /// Applies focus-restore intent.
    pub(crate) fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = Some(intent);
        self
    }

    /// Resolves disclosure state.
    pub(crate) fn resolve(self) -> OverlayDisclosureState {
        let requested_open = ControllableState::resolve(self.controlled_open, || self.default_open);
        let open_mode = if requested_open.is_controlled() {
            OverlayDisclosureOpenMode::Controlled
        } else {
            OverlayDisclosureOpenMode::Uncontrolled
        };
        let open = *requested_open.value() && !self.disabled && self.openable;
        let presence = OverlayPresence::from_open(open);
        let mut config = GpuiOverlayAdapterConfig::new(self.layer_kind, presence);
        if let Some(outside_press_policy) = self.outside_press_policy {
            config = config.outside_press_policy(outside_press_policy);
        }
        if let Some(escape_key_policy) = self.escape_key_policy {
            config = config.escape_key_policy(escape_key_policy);
        }
        if let Some(initial_focus_intent) = self.initial_focus_intent {
            config = config.initial_focus_intent(initial_focus_intent);
        }
        if let Some(focus_restore_intent) = self.focus_restore_intent {
            config = config.focus_restore_intent(focus_restore_intent);
        }
        let overlay = config.resolved_state();

        OverlayDisclosureState {
            open,
            open_mode,
            overlay,
        }
    }
}

/// Resolved shared disclosure state for overlay-like adapters.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverlayDisclosureState {
    open: bool,
    open_mode: OverlayDisclosureOpenMode,
    overlay: OverlayResolvedState,
}

impl OverlayDisclosureState {
    /// Returns the gated open value adapters should render.
    pub(crate) const fn open(&self) -> bool {
        self.open
    }

    /// Returns open-state ownership.
    pub(crate) const fn open_mode(&self) -> OverlayDisclosureOpenMode {
        self.open_mode
    }

    /// Returns renderer-neutral overlay state.
    pub(crate) const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

/// Resolved adapter-owned open state for controlled and uncontrolled overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverlayRuntimeState {
    open: bool,
    controlled: bool,
    runtime_changed: bool,
}

impl OverlayRuntimeState {
    /// Returns the resolved open value the adapter should render.
    pub(crate) const fn open(self) -> bool {
        self.open
    }

    /// Returns whether the open value was provided by the caller.
    pub(crate) const fn controlled(self) -> bool {
        self.controlled
    }

    /// Returns whether the stored runtime value should be synchronized to the resolved value.
    pub(crate) const fn runtime_changed(self) -> bool {
        self.runtime_changed
    }
}

/// Resolves controlled/uncontrolled open state without emitting callbacks.
pub(crate) fn resolve_overlay_open_state(
    controlled_open: Option<bool>,
    runtime_open: bool,
) -> OverlayRuntimeState {
    let open_state = ControllableState::resolve(controlled_open, || runtime_open);
    let open = *open_state.value();
    let controlled = open_state.is_controlled();
    OverlayRuntimeState {
        open,
        controlled,
        runtime_changed: controlled && runtime_open != open,
    }
}
