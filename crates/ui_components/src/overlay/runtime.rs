//! GPUI runtime helpers for overlay-like component adapters.

use open_gpui::{App, Entity, FocusHandle, Window};
use open_gpui_ui_core::{
    ControllableState, DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent,
    OutsidePressPolicy, OverlayLayerKind, OverlayLayerPolicy, OverlayPresence,
};

use super::OverlayResolvedState;
use super::adapter::GpuiOverlayAdapterConfig;

/// Resolved open-change request emitted by overlay adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayOpenChange {
    open: bool,
    reason: DismissReason,
    consume_event: bool,
    allow_underlay: bool,
}

impl OverlayOpenChange {
    /// Creates an open-change request.
    pub const fn new(
        open: bool,
        reason: DismissReason,
        consume_event: bool,
        allow_underlay: bool,
    ) -> Self {
        Self {
            open,
            reason,
            consume_event,
            allow_underlay,
        }
    }

    /// Returns the requested open state.
    pub const fn open(self) -> bool {
        self.open
    }

    /// Returns the dismiss or open-change reason.
    pub const fn reason(self) -> DismissReason {
        self.reason
    }

    /// Returns whether the source event should be consumed.
    pub const fn consumes_event(self) -> bool {
        self.consume_event
    }

    /// Returns whether underlay dispatch may continue.
    pub const fn allows_underlay_dispatch(self) -> bool {
        self.allow_underlay
    }
}

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

/// Resolves an Escape-key open-change request.
pub const fn escape_open_change(policy: &OverlayLayerPolicy) -> Option<OverlayOpenChange> {
    match policy.escape_key_policy() {
        EscapeKeyPolicy::Ignore => None,
        EscapeKeyPolicy::Dismiss if policy.presence().interactive() => Some(OverlayOpenChange {
            open: false,
            reason: DismissReason::EscapeKey,
            consume_event: true,
            allow_underlay: false,
        }),
        EscapeKeyPolicy::Dismiss => None,
    }
}

/// Resolves an outside-press open-change request.
pub const fn outside_press_open_change(policy: &OverlayLayerPolicy) -> Option<OverlayOpenChange> {
    if !policy.presence().interactive() {
        return None;
    }

    let outcome = policy.outside_press_policy().resolve();
    if let Some(reason) = outcome.dismiss_reason() {
        Some(OverlayOpenChange {
            open: false,
            reason,
            consume_event: outcome.consumes_event(),
            allow_underlay: outcome.allows_underlay_dispatch(),
        })
    } else {
        None
    }
}

/// Consumes a GPUI event that was handled by overlay open, close, or barrier behavior.
pub(crate) fn consume_overlay_event(window: &mut Window, cx: &mut App) {
    cx.stop_propagation();
    window.prevent_default();
}

/// Returns whether the overlay should restore focus back to the trigger.
pub const fn focus_restore_requests_trigger(intent: &FocusRestoreIntent) -> bool {
    matches!(
        intent,
        FocusRestoreIntent::Trigger | FocusRestoreIntent::TriggerOrFallback(_)
    )
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

/// Updates runtime open state without invoking component callbacks.
pub(crate) fn set_overlay_open(runtime_open: &mut bool, open: bool) {
    *runtime_open = open;
}

/// Runtime request for applying an overlay open-state transition.
pub(crate) struct OverlayOpenRuntimeRequest<'a, T: 'static> {
    runtime: Entity<T>,
    open: bool,
    on_open_change: Option<&'a dyn Fn(bool, &mut Window, &mut App)>,
}

impl<'a, T: 'static> OverlayOpenRuntimeRequest<'a, T> {
    /// Creates an open-state transition request.
    pub(crate) fn new(
        runtime: Entity<T>,
        open: bool,
        on_open_change: Option<&'a dyn Fn(bool, &mut Window, &mut App)>,
    ) -> Self {
        Self {
            runtime,
            open,
            on_open_change,
        }
    }
}

/// Runtime request for applying an overlay close transition.
pub(crate) struct OverlayCloseRuntimeRequest<'a, T: 'static> {
    runtime: Entity<T>,
    focus_restore: &'a FocusRestoreIntent,
    trigger_focus: FocusHandle,
    defer_focus_restore: bool,
    on_open_change: Option<&'a dyn Fn(bool, &mut Window, &mut App)>,
}

impl<'a, T: 'static> OverlayCloseRuntimeRequest<'a, T> {
    /// Creates a close transition request.
    pub(crate) fn new(
        runtime: Entity<T>,
        focus_restore: &'a FocusRestoreIntent,
        trigger_focus: FocusHandle,
        on_open_change: Option<&'a dyn Fn(bool, &mut Window, &mut App)>,
    ) -> Self {
        Self {
            runtime,
            focus_restore,
            trigger_focus,
            defer_focus_restore: false,
            on_open_change,
        }
    }

    /// Applies deferred focus restore for close paths that cannot move focus immediately.
    pub(crate) const fn defer_focus_restore(mut self, defer_focus_restore: bool) -> Self {
        self.defer_focus_restore = defer_focus_restore;
        self
    }
}

/// Applies an overlay open-state change and emits the bool callback afterward.
pub(crate) fn apply_overlay_open_change<T: 'static>(
    request: OverlayOpenRuntimeRequest<'_, T>,
    window: &mut Window,
    cx: &mut App,
    update_runtime: impl FnOnce(&mut T),
) {
    apply_overlay_open_change_with_after_update(request, window, cx, update_runtime, |_, _| {});
}

/// Applies an overlay open-state change, runs a post-update hook, and emits the callback afterward.
pub(crate) fn apply_overlay_open_change_with_after_update<T: 'static>(
    request: OverlayOpenRuntimeRequest<'_, T>,
    window: &mut Window,
    cx: &mut App,
    update_runtime: impl FnOnce(&mut T),
    after_update: impl FnOnce(&mut Window, &mut App),
) {
    request.runtime.update(cx, |runtime, _| {
        update_runtime(runtime);
    });
    after_update(window, cx);
    emit_overlay_open_change(request.open, request.on_open_change, window, cx);
}

/// Closes an overlay runtime and applies the shared callback/focus tail.
pub(crate) fn close_overlay_runtime<T: 'static>(
    request: OverlayCloseRuntimeRequest<'_, T>,
    window: &mut Window,
    cx: &mut App,
    close_runtime: impl FnOnce(&mut T),
) {
    close_overlay_runtime_with_after_update(request, window, cx, close_runtime, |_, _| {});
}

/// Closes an overlay runtime, runs a post-update hook, and applies the callback/focus tail.
pub(crate) fn close_overlay_runtime_with_after_update<T: 'static>(
    request: OverlayCloseRuntimeRequest<'_, T>,
    window: &mut Window,
    cx: &mut App,
    close_runtime: impl FnOnce(&mut T),
    after_update: impl FnOnce(&mut Window, &mut App),
) {
    request.runtime.update(cx, |runtime, _| {
        close_runtime(runtime);
    });
    after_update(window, cx);
    emit_overlay_open_change(false, request.on_open_change, window, cx);
    restore_overlay_focus(
        request.focus_restore,
        Some(request.trigger_focus),
        request.defer_focus_restore,
        window,
        cx,
    );
}

/// Emits the bool open-change callback after runtime state has been updated.
pub(crate) fn emit_overlay_open_change(
    open: bool,
    on_open_change: Option<&dyn Fn(bool, &mut Window, &mut App)>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(on_open_change) = on_open_change {
        on_open_change(open, window, cx);
    }
}

/// Restores focus to a trigger handle when the focus-restore policy requests it.
pub(crate) fn restore_overlay_focus(
    focus_restore: &FocusRestoreIntent,
    trigger_focus: Option<FocusHandle>,
    defer_focus_restore: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if focus_restore_requests_trigger(focus_restore)
        && let Some(trigger_focus) = trigger_focus
    {
        if defer_focus_restore {
            window.defer(cx, move |window, cx| trigger_focus.focus(window, cx));
        } else {
            trigger_focus.focus(window, cx);
        }
    }
}
