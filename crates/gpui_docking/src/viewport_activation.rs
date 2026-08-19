use crate::{
    DockHost, DockSpaceId, DockViewportExactFocusTarget, DockViewportFocusCommand,
    DockViewportFocusCommandSource, DockViewportFocusRequest,
    surface::{DockSurfaceActivationBinding, DockSurfaceActivationOutcome},
    viewport_registry::DockViewportRegistrationKey,
};
#[cfg(test)]
use open_gpui::WindowActivationStatus;
use open_gpui::{
    AnyWindowHandle, App, Context, PlatformFocusedWindow, Subscription, WeakEntity, Window,
    WindowActivationSnapshot, WindowActivationTerminal, WindowActivationTicket, WindowId,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    fmt,
    rc::{Rc, Weak as RcWeak},
};

/// Platform activation policy for a runtime viewport activation transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportWindowActivation {
    /// Bring the target GPUI window to the platform foreground.
    Request,
    /// Do not request platform activation; apply internal focus only if already active.
    DoNotRequest,
}

/// Backend-focus observation for a viewport activation transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportActivationBackendFocusObservation {
    /// The platform backend already reports the target viewport window as focused.
    TargetFocused,
    /// The target viewport window is not currently backend-focused, or backend focus is unknown.
    TargetNotFocused,
}

impl DockViewportActivationBackendFocusObservation {
    pub(crate) fn from_platform_focused_window(
        focused_window: PlatformFocusedWindow,
        target_window: AnyWindowHandle,
    ) -> Self {
        match focused_window {
            PlatformFocusedWindow::Window(window) if window == target_window => Self::TargetFocused,
            PlatformFocusedWindow::Window(_)
            | PlatformFocusedWindow::NoWindow
            | PlatformFocusedWindow::Unavailable => Self::TargetNotFocused,
        }
    }

    pub(crate) fn target_focused(self) -> bool {
        matches!(self, Self::TargetFocused)
    }
}

/// Runtime viewport activation transaction selected by drop, tear-off, or close recovery.
///
/// Creating this value records intent only. Platform activation and host focus are applied only by
/// [`apply_viewport_activation_transaction`] or
/// [`apply_viewport_activation_transaction_from_window`], after the viewport registration and
/// graph mutation are complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportActivationTransaction {
    /// Exact logical-space-to-window registration that owns this activation.
    registration: DockViewportRegistrationKey,
    /// GPUI window rendering the logical dock space.
    window: AnyWindowHandle,
    /// Whether applying this target should request platform window activation.
    window_activation: DockViewportWindowActivation,
    /// Source of the focus command requested after window activation.
    focus_source: DockViewportFocusCommandSource,
    /// Explicit focus request to apply after the window is active.
    focus_request: DockViewportFocusRequest,
    /// Exact descendant focus authority captured before an asynchronous viewport transition.
    exact_focus_target: Option<DockViewportExactFocusTarget>,
    /// Optional embedded host target for surface activation.
    ///
    /// A surface host may be nested below an arbitrary window root, so a window-root downcast
    /// cannot be the only activation route.
    target_host: Option<WeakEntity<DockHost>>,
    surface_activation: Option<DockSurfaceActivationBinding>,
}

impl DockViewportActivationTransaction {
    pub(crate) fn registered(
        registration: DockViewportRegistrationKey,
        window: impl Into<AnyWindowHandle>,
        focus_request: DockViewportFocusRequest,
    ) -> Self {
        Self::with_policy(
            registration,
            window.into(),
            DockViewportWindowActivation::Request,
            DockViewportFocusCommandSource::ViewportActivation,
            focus_request,
            None,
            None,
        )
    }

    pub(crate) fn close_recovery(
        registration: DockViewportRegistrationKey,
        window: impl Into<AnyWindowHandle>,
        focus_request: DockViewportFocusRequest,
    ) -> Self {
        Self::with_policy(
            registration,
            window.into(),
            DockViewportWindowActivation::DoNotRequest,
            DockViewportFocusCommandSource::CloseRecovery,
            focus_request,
            None,
            None,
        )
    }

    pub(crate) fn surface_activation(
        registration: DockViewportRegistrationKey,
        window: impl Into<AnyWindowHandle>,
        focus_request: DockViewportFocusRequest,
        binding: DockSurfaceActivationBinding,
        target_host: WeakEntity<DockHost>,
    ) -> Self {
        Self::with_policy(
            registration,
            window.into(),
            DockViewportWindowActivation::Request,
            DockViewportFocusCommandSource::ViewportActivation,
            focus_request,
            Some(target_host),
            Some(binding),
        )
    }

    fn with_policy(
        registration: DockViewportRegistrationKey,
        window: AnyWindowHandle,
        window_activation: DockViewportWindowActivation,
        focus_source: DockViewportFocusCommandSource,
        focus_request: DockViewportFocusRequest,
        target_host: Option<WeakEntity<DockHost>>,
        surface_activation: Option<DockSurfaceActivationBinding>,
    ) -> Self {
        debug_assert_eq!(
            registration.window_id(),
            window.window_id(),
            "viewport activation registration must belong to its target window"
        );
        Self {
            registration,
            window,
            window_activation,
            focus_source,
            focus_request,
            exact_focus_target: None,
            target_host,
            surface_activation,
        }
    }

    #[cfg(test)]
    pub(crate) fn new(
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
        focus_request: DockViewportFocusRequest,
    ) -> Self {
        let space = space.into();
        let window = window.into();
        Self::registered(
            DockViewportRegistrationKey::for_test(space, window.window_id()),
            window,
            focus_request,
        )
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        self.registration.space()
    }

    pub(crate) fn registration_key(&self) -> &DockViewportRegistrationKey {
        &self.registration
    }

    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn window_activation(&self) -> DockViewportWindowActivation {
        self.window_activation
    }

    pub(crate) fn requests_window_activation(&self) -> bool {
        matches!(
            self.window_activation,
            DockViewportWindowActivation::Request
        )
    }

    pub(crate) fn focus_request(&self) -> &DockViewportFocusRequest {
        &self.focus_request
    }

    pub(crate) fn focus_source(&self) -> DockViewportFocusCommandSource {
        self.focus_source
    }

    pub(crate) fn with_exact_focus_target(mut self, target: DockViewportExactFocusTarget) -> Self {
        debug_assert!(
            matches!(self.focus_request, DockViewportFocusRequest::Panel(_)),
            "an exact focus descendant must belong to a concrete panel request"
        );
        self.exact_focus_target = Some(target);
        self
    }

    pub(crate) fn exact_focus_target(&self) -> Option<&DockViewportExactFocusTarget> {
        self.exact_focus_target.as_ref()
    }

    pub(crate) fn surface_activation_binding(&self) -> Option<&DockSurfaceActivationBinding> {
        self.surface_activation.as_ref()
    }

    pub(crate) fn target_host(&self) -> Option<&WeakEntity<DockHost>> {
        self.target_host.as_ref()
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.registration.window_id()
    }

    pub(crate) fn matches_window(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.registration.space() == space && self.registration.window_id() == window_id
    }
}

/// Sole Dock-side owner of in-flight native-window activation requests.
///
/// The GPUI ticket owns native activation authority. Dock retains only the semantic transaction
/// that must be transferred to the focus coordinator after an exact `Activated` terminal, or
/// settled if native activation fails. Backend focus observations never complete these records on
/// their own.
#[derive(Clone, Default)]
pub(crate) struct DockViewportActivationExecutor {
    state: Rc<RefCell<DockViewportActivationExecutorState>>,
}

#[derive(Default)]
struct DockViewportActivationExecutorState {
    executions: BTreeMap<u64, DockViewportActivationExecution>,
    latest_generation_by_registration: HashMap<DockViewportRegistrationKey, u64>,
}

struct DockViewportActivationExecution {
    transaction: DockViewportActivationTransaction,
    ticket: WindowActivationTicket,
    retirement_outcome: Option<DockSurfaceActivationOutcome>,
    _subscription: Subscription,
}

impl fmt::Debug for DockViewportActivationExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockViewportActivationExecutor")
            .field("execution_count", &self.state.borrow().executions.len())
            .finish()
    }
}

impl DockViewportActivationExecutor {
    pub(crate) fn observe(
        &self,
        transaction: DockViewportActivationTransaction,
        ticket: WindowActivationTicket,
        cx: &mut Context<DockHost>,
    ) {
        let snapshot = ticket.snapshot();
        debug_assert_eq!(snapshot.target(), transaction.window_id());
        let generation = snapshot.request_generation();
        let registration = transaction.registration_key().clone();
        let state: RcWeak<RefCell<DockViewportActivationExecutorState>> =
            Rc::downgrade(&self.state);
        let async_cx = cx.to_async();
        let subscription = ticket.subscribe(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            async_cx.update(|cx| settle_viewport_activation_execution(&state, generation, cx));
        });

        let mut state = self.state.borrow_mut();
        state
            .latest_generation_by_registration
            .entry(registration)
            .and_modify(|latest_generation| {
                *latest_generation = (*latest_generation).max(generation);
            })
            .or_insert(generation);
        let replaced = state.executions.insert(
            generation,
            DockViewportActivationExecution {
                transaction,
                ticket,
                retirement_outcome: None,
                _subscription: subscription,
            },
        );
        debug_assert!(
            replaced.is_none(),
            "native activation generations must identify one Dock execution"
        );
    }

    /// Returns whether an exact registration still owns native activation settlement.
    pub(crate) fn has_execution_for_registration(
        &self,
        registration: &DockViewportRegistrationKey,
    ) -> bool {
        self.state
            .borrow()
            .executions
            .values()
            .any(|execution| execution.transaction.registration_key() == registration)
    }

    /// Retires every activation owned by one exact viewport registration before its host binding
    /// is released.
    pub(crate) fn retire_registration(
        &self,
        registration: &DockViewportRegistrationKey,
        lifecycle_outcome: DockSurfaceActivationOutcome,
        cx: &mut App,
    ) {
        let generations = self
            .state
            .borrow()
            .executions
            .iter()
            .filter_map(|(generation, execution)| {
                (execution.transaction.registration_key() == registration).then_some(*generation)
            })
            .collect::<Vec<_>>();
        for generation in generations {
            let ticket = {
                let mut state = self.state.borrow_mut();
                let Some(execution) = state.executions.get_mut(&generation) else {
                    continue;
                };
                let snapshot = execution.ticket.snapshot();
                if snapshot.status().terminal().is_none()
                    || snapshot.status().terminal() == Some(WindowActivationTerminal::Activated)
                {
                    execution.retirement_outcome = Some(lifecycle_outcome);
                }
                execution.ticket.clone()
            };
            if !ticket.snapshot().status().is_terminal() {
                let _ = ticket.cancel_for_target_replacement();
            }
            settle_viewport_activation_execution(&self.state, generation, cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn execution_count(&self) -> usize {
        self.state.borrow().executions.len()
    }

    #[cfg(test)]
    pub(crate) fn settle_execution(&self, generation: u64, cx: &mut App) {
        settle_viewport_activation_execution(&self.state, generation, cx);
    }
}

fn settle_viewport_activation_execution(
    state: &Rc<RefCell<DockViewportActivationExecutorState>>,
    generation: u64,
    cx: &mut App,
) {
    let (transaction, snapshot, retirement_outcome, is_latest_for_registration) = {
        let state = state.borrow();
        let Some(execution) = state.executions.get(&generation) else {
            return;
        };
        let latest_generation = state
            .latest_generation_by_registration
            .get(execution.transaction.registration_key())
            .copied();
        (
            execution.transaction.clone(),
            execution.ticket.snapshot(),
            execution.retirement_outcome,
            latest_generation == Some(generation),
        )
    };

    if let Some(outcome) = retirement_outcome {
        settle_surface_activation_binding(&transaction, outcome, cx);
    } else if !is_latest_for_registration {
        settle_surface_activation_binding(
            &transaction,
            DockSurfaceActivationOutcome::Superseded,
            cx,
        );
    } else if snapshot.status().is_terminal() {
        settle_viewport_activation_ticket(&transaction, snapshot, cx);
    } else {
        return;
    }

    let mut state = state.borrow_mut();
    let Some(execution) = state.executions.remove(&generation) else {
        return;
    };
    let registration = execution.transaction.registration_key().clone();
    if !state
        .executions
        .values()
        .any(|execution| execution.transaction.registration_key() == &registration)
    {
        state
            .latest_generation_by_registration
            .remove(&registration);
    }
}

fn focus_command_for_transaction(
    transaction: &DockViewportActivationTransaction,
) -> DockViewportFocusCommand {
    let command = match transaction.surface_activation_binding() {
        Some(binding) => DockViewportFocusCommand::surface_activation(
            transaction.focus_request().clone(),
            binding.clone(),
        ),
        None => DockViewportFocusCommand::new(
            transaction.focus_source(),
            transaction.focus_request().clone(),
        ),
    };
    match transaction.exact_focus_target() {
        Some(target) => command.with_exact_focus_target(target.clone()),
        None => command,
    }
}

fn settle_viewport_activation_ticket(
    transaction: &DockViewportActivationTransaction,
    snapshot: WindowActivationSnapshot,
    cx: &mut App,
) {
    let terminal = snapshot
        .status()
        .terminal()
        .expect("a viewport activation observer must receive one terminal snapshot");
    if terminal == WindowActivationTerminal::Activated {
        let outcome =
            apply_viewport_activation_transaction_with_policy(Some(transaction.clone()), false, cx);
        if matches!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                focus_command_queued: true,
                ..
            }
        ) {
            return;
        }
        let outcome = match outcome {
            DockViewportActivationApplyOutcome::WindowUnavailable => {
                DockSurfaceActivationOutcome::WindowClosed
            }
            DockViewportActivationApplyOutcome::NoTarget
            | DockViewportActivationApplyOutcome::SpaceMismatch
            | DockViewportActivationApplyOutcome::Applied { .. } => {
                DockSurfaceActivationOutcome::Superseded
            }
            DockViewportActivationApplyOutcome::WrongRootView => {
                DockSurfaceActivationOutcome::Unavailable
            }
        };
        settle_surface_activation_binding(transaction, outcome, cx);
        return;
    }

    let outcome = match terminal {
        WindowActivationTerminal::Activated => unreachable!(),
        WindowActivationTerminal::Rejected | WindowActivationTerminal::PolicyChanged => {
            DockSurfaceActivationOutcome::Rejected
        }
        WindowActivationTerminal::Unsupported => DockSurfaceActivationOutcome::Unavailable,
        WindowActivationTerminal::Superseded
        | WindowActivationTerminal::TargetReplaced
        | WindowActivationTerminal::Cancelled => DockSurfaceActivationOutcome::Superseded,
        WindowActivationTerminal::WindowClosed => DockSurfaceActivationOutcome::WindowClosed,
    };
    settle_surface_activation_binding(transaction, outcome, cx);
}

fn settle_surface_activation_binding(
    transaction: &DockViewportActivationTransaction,
    outcome: DockSurfaceActivationOutcome,
    cx: &mut App,
) {
    if let Some(binding) = transaction.surface_activation_binding() {
        binding.settle(outcome, cx);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportActivationApplyOutcome {
    NoTarget,
    WindowUnavailable,
    WrongRootView,
    SpaceMismatch,
    Applied {
        changed: bool,
        focus_command_queued: bool,
        window_activation_requested: bool,
        backend_focus: DockViewportActivationBackendFocusObservation,
        backend_focus_apply: DockViewportActivationBackendFocusApply,
    },
}

impl DockViewportActivationApplyOutcome {
    pub(crate) fn changed(self) -> bool {
        match self {
            Self::Applied { changed, .. } => changed,
            Self::NoTarget
            | Self::WindowUnavailable
            | Self::WrongRootView
            | Self::SpaceMismatch => false,
        }
    }
}

/// Runtime backend-focus state updates performed while applying a viewport activation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum DockViewportActivationBackendFocusRecordEffect {
    #[default]
    Unchanged,
    RecordedTargetFocus,
}

impl DockViewportActivationBackendFocusRecordEffect {
    pub(crate) fn from_changed(changed: bool) -> Self {
        if changed {
            Self::RecordedTargetFocus
        } else {
            Self::Unchanged
        }
    }

    fn changed(self) -> bool {
        matches!(self, Self::RecordedTargetFocus)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DockViewportActivationBackendFocusApply {
    backend_focus_record: DockViewportActivationBackendFocusRecordEffect,
}

impl DockViewportActivationBackendFocusApply {
    pub(crate) fn new(
        backend_focus_record: DockViewportActivationBackendFocusRecordEffect,
    ) -> Self {
        Self {
            backend_focus_record,
        }
    }

    pub(crate) fn changed(self) -> bool {
        self.backend_focus_record.changed()
    }
}

fn apply_activation_to_host(
    transaction: &DockViewportActivationTransaction,
    focus_command: &DockViewportFocusCommand,
    should_activate_window: bool,
    host: &mut DockHost,
    window: &mut open_gpui::Window,
    cx: &mut open_gpui::Context<DockHost>,
    outcome: &Cell<DockViewportActivationApplyOutcome>,
) {
    if host.space() != transaction.space()
        || window.window_handle() != transaction.window()
        || transaction.window_id() != transaction.window().window_id()
        || transaction
            .target_host()
            .is_some_and(|target| target.entity_id() != cx.entity().entity_id())
    {
        outcome.set(DockViewportActivationApplyOutcome::SpaceMismatch);
        return;
    }
    let registration_is_current = |host: &DockHost| {
        host.viewport_runtime()
            .registration_key_for_space_window(transaction.space(), transaction.window_id())
            .as_ref()
            == Some(transaction.registration_key())
    };
    if !registration_is_current(host) {
        outcome.set(DockViewportActivationApplyOutcome::NoTarget);
        return;
    }
    // Validate a surface activation before touching backend-focus state. A queued callback from
    // an old host/window generation must be a complete no-op rather than a runtime mutation that
    // is rejected only when the focus command reaches the host.
    if transaction
        .surface_activation_binding()
        .is_some_and(|binding| !binding.is_current(cx))
    {
        outcome.set(DockViewportActivationApplyOutcome::NoTarget);
        return;
    }
    let backend_focus = DockViewportActivationBackendFocusObservation::from_platform_focused_window(
        cx.focused_window(),
        transaction.window(),
    );
    // Do not pre-filter through cached capability or policy booleans. `activate_window` is the
    // sole native activation authority and returns an exact typed terminal for unsupported or
    // rejected requests. Skipping that ticket here would strand the Dock transaction forever.
    let request_backend_activation = should_activate_window;
    let backend_focus_apply = host
        .viewport_runtime()
        .record_activation_backend_focus(transaction.window_id(), backend_focus);
    if !registration_is_current(host) {
        outcome.set(DockViewportActivationApplyOutcome::NoTarget);
        return;
    }
    let focus_changed = if backend_focus.target_focused() {
        host.request_viewport_focus_command_in_context(focus_command.clone(), cx)
    } else {
        false
    };
    if !registration_is_current(host) {
        outcome.set(DockViewportActivationApplyOutcome::NoTarget);
        return;
    }
    let window_activation_requested = request_backend_activation && !backend_focus.target_focused();
    if window_activation_requested {
        let ticket = window.activate_window();
        host.viewport_runtime()
            .observe_activation_ticket(transaction.clone(), ticket, cx);
    }
    let changed = backend_focus_apply.changed() || focus_changed || window_activation_requested;
    outcome.set(DockViewportActivationApplyOutcome::Applied {
        changed,
        focus_command_queued: focus_changed,
        window_activation_requested,
        backend_focus,
        backend_focus_apply,
    });
    if changed {
        cx.notify();
    }
}

/// Applies an activation without re-entering a host window already owned by its event callback.
///
/// Only the exact current host/window target uses the borrowed values. Every other target keeps
/// the generic window lookup path, including its normal unavailable-window outcome.
pub(crate) fn apply_viewport_activation_transaction_from_window(
    transaction: Option<DockViewportActivationTransaction>,
    current_host: &mut DockHost,
    current_window: &mut Window,
    cx: &mut Context<DockHost>,
) -> DockViewportActivationApplyOutcome {
    let Some(transaction) = transaction else {
        return DockViewportActivationApplyOutcome::NoTarget;
    };
    let current_window_handle = current_window.window_handle();
    let targets_current_host = transaction.window() == current_window_handle
        && transaction.matches_window(current_host.space(), current_window_handle.window_id())
        && transaction
            .target_host()
            .is_none_or(|target| target.entity_id() == cx.entity().entity_id());
    if !targets_current_host {
        return apply_viewport_activation_transaction(Some(transaction), cx);
    }

    let focus_command = focus_command_for_transaction(&transaction);
    let outcome = Cell::new(DockViewportActivationApplyOutcome::WindowUnavailable);
    apply_activation_to_host(
        &transaction,
        &focus_command,
        transaction.requests_window_activation(),
        current_host,
        current_window,
        cx,
        &outcome,
    );
    outcome.get()
}

/// Applies a viewport activation transaction to the matching runtime host window.
///
/// Returns a structured outcome so lifecycle code can distinguish no-op from stale transactions.
pub(crate) fn apply_viewport_activation_transaction(
    transaction: Option<DockViewportActivationTransaction>,
    cx: &mut App,
) -> DockViewportActivationApplyOutcome {
    let should_activate_window = transaction
        .as_ref()
        .is_some_and(DockViewportActivationTransaction::requests_window_activation);
    apply_viewport_activation_transaction_with_policy(transaction, should_activate_window, cx)
}

fn apply_viewport_activation_transaction_with_policy(
    transaction: Option<DockViewportActivationTransaction>,
    should_activate_window: bool,
    cx: &mut App,
) -> DockViewportActivationApplyOutcome {
    let Some(transaction) = transaction else {
        return DockViewportActivationApplyOutcome::NoTarget;
    };

    let focus_command = focus_command_for_transaction(&transaction);
    let outcome = Rc::new(Cell::new(
        DockViewportActivationApplyOutcome::WindowUnavailable,
    ));
    let target_host = transaction.target_host().cloned();
    let applied = if let Some(target_host) = target_host {
        let transaction_for_host = transaction.clone();
        let focus_command_for_host = focus_command.clone();
        let outcome_for_host = outcome.clone();
        target_host.update_in(cx, move |host, window, cx| {
            if window.window_handle().window_id() != transaction_for_host.window_id() {
                outcome_for_host.set(DockViewportActivationApplyOutcome::WindowUnavailable);
                return;
            }
            apply_activation_to_host(
                &transaction_for_host,
                &focus_command_for_host,
                should_activate_window,
                host,
                window,
                cx,
                &outcome_for_host,
            );
        })
    } else {
        let transaction_for_window = transaction.clone();
        let focus_command_for_window = focus_command.clone();
        let outcome_for_window = outcome.clone();
        transaction.window().update(cx, move |view, window, cx| {
            let Ok(host) = view.downcast::<DockHost>() else {
                outcome_for_window.set(DockViewportActivationApplyOutcome::WrongRootView);
                return;
            };
            host.update(cx, |host, cx| {
                apply_activation_to_host(
                    &transaction_for_window,
                    &focus_command_for_window,
                    should_activate_window,
                    host,
                    window,
                    cx,
                    &outcome_for_window,
                )
            });
        })
    };
    if applied.is_err() {
        return DockViewportActivationApplyOutcome::WindowUnavailable;
    }

    outcome.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockViewportFocusCommand, DockViewportFocusCommandSource, DockViewportFocusRequest,
        host_test_support::{open_host, space, tabs_graph},
    };
    use open_gpui::{
        AppContext as _, Entity, TestAppContext, WindowActivationPolicy, WindowMutationDispatch,
        WindowMutationDomain, px, size,
    };

    fn current_registration(
        cx: &TestAppContext,
        host: &Entity<DockHost>,
        window_id: WindowId,
    ) -> DockViewportRegistrationKey {
        cx.read_entity(host, |host, _| {
            host.viewport_runtime()
                .registration_key_for_space_window(host.space(), window_id)
        })
        .expect("open host should have an exact viewport registration")
    }

    fn unchanged_backend_focus_apply() -> DockViewportActivationBackendFocusApply {
        DockViewportActivationBackendFocusApply::default()
    }

    fn recorded_confirmed_backend_focus() -> DockViewportActivationBackendFocusApply {
        DockViewportActivationBackendFocusApply::new(
            DockViewportActivationBackendFocusRecordEffect::RecordedTargetFocus,
        )
    }

    #[open_gpui::test]
    fn activation_without_target_reports_no_target(cx: &mut TestAppContext) {
        let outcome = cx.update(|app| apply_viewport_activation_transaction(None, app));

        assert_eq!(outcome, DockViewportActivationApplyOutcome::NoTarget);
        assert!(!outcome.changed());
    }

    #[open_gpui::test]
    fn current_window_activation_applies_without_reborrowing_the_window(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let outcome = window
            .update(cx, |host, window, cx| {
                apply_viewport_activation_transaction_from_window(
                    Some(activation),
                    host,
                    window,
                    cx,
                )
            })
            .expect("current viewport window should remain available");

        assert_ne!(
            outcome,
            DockViewportActivationApplyOutcome::WindowUnavailable,
            "the event receiver already owns the current window, so activation must not reborrow it"
        );
        assert!(
            matches!(outcome, DockViewportActivationApplyOutcome::Applied { .. }),
            "the exact current host/window activation should be applied"
        );
    }

    #[open_gpui::test]
    fn activation_rejects_replaced_registration_with_same_space_and_window(
        cx: &mut TestAppContext,
    ) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let window: AnyWindowHandle = window.into();
        let runtime = cx.read_entity(&host, |host, _| host.viewport_runtime().clone());
        let first_registration = current_registration(cx, &host, window.window_id());
        let first = DockViewportActivationTransaction::registered(
            first_registration.clone(),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let second_registration = {
            let mut runtime = runtime.borrow_mut();
            runtime.unregister_adapter_window_for_test(window.window_id());
            runtime
                .register_opened_viewport_with_cleanup(space(), window)
                .expect("replacement registration should succeed")
                .outcome
                .registration_key()
                .clone()
        };
        assert_ne!(first_registration, second_registration);
        let second = DockViewportActivationTransaction::registered(
            second_registration,
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let first_outcome =
            cx.update(|app| apply_viewport_activation_transaction(Some(first), app));
        let after_first = cx.read_entity(&host, |host, _| {
            (
                host.pending_focus_command().is_some(),
                host.viewport_runtime().activation_execution_count() != 0,
            )
        });
        let second_outcome =
            cx.update(|app| apply_viewport_activation_transaction(Some(second), app));

        assert_eq!(first_outcome, DockViewportActivationApplyOutcome::NoTarget);
        assert_eq!(
            after_first,
            (false, false),
            "a stale registration lease must not mutate host or backend-focus state"
        );
        assert!(
            matches!(
                second_outcome,
                DockViewportActivationApplyOutcome::Applied { .. }
            ),
            "the replacement registration lease should remain activatable"
        );
    }

    #[open_gpui::test]
    fn activation_request_transfers_without_a_parallel_backend_focus_slot(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let (outcome, pending_request) = cx.update(|app| {
            let outcome = apply_viewport_activation_transaction(Some(activation), app);
            let pending_request = app.read_entity(&host, |host, _| {
                host.pending_focus_command()
                    .map(|command| command.request().clone())
            });
            (outcome, pending_request)
        });

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: false,
                window_activation_requested: true,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert_eq!(pending_request, None);

        assert_eq!(
            cx.read_entity(&host, |host, _| {
                host.viewport_runtime().activation_execution_count()
            }),
            0,
            "the synchronous exact native terminal may transfer and retire in the same update"
        );
        assert_eq!(cx.update(|app| app.active_window()), Some(window.into()));
    }

    #[open_gpui::test]
    fn rejected_activation_policy_settles_through_the_native_ticket(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let window_handle: AnyWindowHandle = window.into();
        let dispatch = window
            .update(cx, |_, window, _| {
                window.request_activation_policy(WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: true,
                })
            })
            .expect("the viewport should remain live");
        assert!(matches!(dispatch, WindowMutationDispatch::Queued(_)));
        assert!(cx.flush_window_mutation(window_handle, WindowMutationDomain::ActivationPolicy));
        assert!(
            !window
                .update(cx, |_, window, _| {
                    window.platform_facts().accepts_activation
                })
                .expect("the committed viewport facts should remain readable")
        );

        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );
        let outcome = cx.update(|app| apply_viewport_activation_transaction(Some(activation), app));

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: false,
                window_activation_requested: true,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                backend_focus_apply: DockViewportActivationBackendFocusApply::default(),
            }
        );
        assert_eq!(
            cx.read_entity(&host, |host, _| {
                host.viewport_runtime().activation_execution_count()
            }),
            0,
            "the already-rejected native ticket must deliver and retire without a focus edge"
        );
        assert_ne!(cx.update(|app| app.active_window()), Some(window_handle));
    }

    #[open_gpui::test]
    fn latest_registration_generation_prevents_older_activated_transfer(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let registration = current_registration(cx, &host, window.window_id());
        let activated = window
            .update(cx, |_, window, _| window.activate_window())
            .expect("viewport activation should remain available");
        cx.run_until_parked();
        assert_eq!(
            activated.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Activated)
        );

        let dispatch = window
            .update(cx, |_, window, _| {
                window.request_activation_policy(WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: true,
                })
            })
            .expect("the viewport should remain live");
        assert!(matches!(dispatch, WindowMutationDispatch::Queued(_)));
        assert!(cx.flush_window_mutation(window.into(), WindowMutationDomain::ActivationPolicy));
        let rejected = window
            .update(cx, |_, window, _| window.activate_window())
            .expect("the rejected activation should still return a ticket");
        assert_eq!(
            rejected.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Rejected)
        );
        let activated_generation = activated.snapshot().request_generation();
        let rejected_generation = rejected.snapshot().request_generation();

        let first = DockViewportActivationTransaction::registered(
            registration.clone(),
            window,
            DockViewportFocusRequest::panel("a"),
        );
        let second = DockViewportActivationTransaction::registered(
            registration,
            window,
            DockViewportFocusRequest::panel("a"),
        );
        host.update(cx, |host, cx| {
            host.viewport_runtime()
                .observe_activation_ticket(first, activated, cx);
            host.viewport_runtime()
                .observe_activation_ticket(second, rejected, cx);
            host.viewport_runtime()
                .settle_activation_execution_for_test(rejected_generation, cx);
            host.viewport_runtime()
                .settle_activation_execution_for_test(activated_generation, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            cx.read_entity(&host, |host, _| {
                (
                    host.pending_focus_command().is_some(),
                    host.viewport_runtime().activation_execution_count(),
                )
            }),
            (false, 0),
            "a newer settled generation must permanently prevent an older Activated ticket from transferring focus"
        );
    }

    #[open_gpui::test]
    fn activation_request_records_pending_when_backend_focus_unavailable(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        cx.set_platform_focused_window_available(false);
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let (outcome, pending_request, activation_execution_count) = cx.update(|app| {
            let outcome = apply_viewport_activation_transaction(Some(activation), app);
            let (pending_request, activation_execution_count) =
                app.read_entity(&host, |host, _| {
                    (
                        host.pending_focus_command()
                            .map(|command| command.request().clone()),
                        host.viewport_runtime().activation_execution_count(),
                    )
                });
            (outcome, pending_request, activation_execution_count)
        });

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: false,
                window_activation_requested: true,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert_eq!(pending_request, None);
        assert_eq!(
            activation_execution_count, 1,
            "backend focus Unavailable must not erase the exact native activation ticket"
        );
    }

    #[open_gpui::test]
    fn activation_request_skips_redundant_platform_activation_when_backend_already_confirms_focus(
        cx: &mut TestAppContext,
    ) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        window
            .update(cx, |_, window, _| {
                let _ = window.activate_window();
            })
            .expect("viewport should confirm backend focus before activation apply");
        cx.run_until_parked();

        let (outcome, pending_request) = cx.update(|app| {
            let outcome = apply_viewport_activation_transaction(Some(activation), app);
            let pending_request = app.read_entity(&host, |host, _| {
                host.pending_focus_command()
                    .map(|command| command.request().clone())
            });
            (outcome, pending_request)
        });

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: true,
                window_activation_requested: false,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert_eq!(pending_request, Some(DockViewportFocusRequest::panel("a")));
    }

    #[open_gpui::test]
    fn activation_confirmed_backend_focus_records_runtime_focus_even_when_focus_command_is_queued(
        cx: &mut TestAppContext,
    ) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        window
            .update(cx, |_, window, _| {
                let _ = window.activate_window();
            })
            .expect("viewport should become the backend-focused window");

        let outcome = window
            .update(cx, |host, window, cx| {
                assert!(host.request_viewport_focus_command(
                    DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(
                        "a"
                    ),),
                ));
                apply_viewport_activation_transaction_from_window(
                    Some(activation),
                    host,
                    window,
                    cx,
                )
            })
            .expect("viewport should remain live");

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: false,
                window_activation_requested: false,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetFocused,
                backend_focus_apply: recorded_confirmed_backend_focus(),
            },
            "confirmed backend focus should still update runtime focus stamps even when the focus command was already queued"
        );
    }

    #[open_gpui::test]
    fn activation_ignores_window_when_space_does_not_match(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTransaction::new(
            "secondary",
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let (outcome, pending_request) = cx.update(|app| {
            let outcome = apply_viewport_activation_transaction(Some(activation), app);
            let pending_request = app.read_entity(&host, |host, _| {
                host.pending_focus_command()
                    .map(|command| command.request().clone())
            });
            (outcome, pending_request)
        });

        assert_eq!(outcome, DockViewportActivationApplyOutcome::SpaceMismatch);
        assert_eq!(pending_request, None);
    }

    #[open_gpui::test]
    fn viewport_activation_overrides_pending_platform_activation_after_backend_focus(
        cx: &mut TestAppContext,
    ) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        window
            .update(cx, |_, window, _| {
                let _ = window.activate_window();
            })
            .expect("viewport should become the backend-focused window");
        let (outcome, pending_request, pending_source) = window
            .update(cx, |host, window, cx| {
                let _ = host
                    .viewport_runtime()
                    .record_confirmed_backend_focus_for_window(window.window_handle().window_id());
                assert!(host.request_viewport_focus_command(
                    DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel(
                        "a"
                    ),),
                ));
                let outcome = apply_viewport_activation_transaction_from_window(
                    Some(activation),
                    host,
                    window,
                    cx,
                );
                let pending_request = host
                    .pending_focus_command()
                    .map(|command| command.request().clone());
                let pending_source = host.pending_focus_command().map(|command| command.source());
                (outcome, pending_request, pending_source)
            })
            .expect("viewport should remain live");

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: true,
                window_activation_requested: false,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert_eq!(pending_request, Some(DockViewportFocusRequest::panel("a")));
        assert_eq!(
            pending_source,
            Some(DockViewportFocusCommandSource::ViewportActivation),
            "confirmed viewport activation should replace the lower-priority platform restore"
        );
    }

    #[open_gpui::test]
    fn repeated_focus_request_requests_platform_activation_without_requeuing_focus(
        cx: &mut TestAppContext,
    ) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, mut visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        visual.deactivate_window();
        let activation = DockViewportActivationTransaction::registered(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let (outcome, pending_request) = window
            .update(cx, |host, window, cx| {
                assert!(host.request_viewport_focus_command(
                    DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(
                        "a"
                    ),),
                ));
                let outcome = apply_viewport_activation_transaction_from_window(
                    Some(activation),
                    host,
                    window,
                    cx,
                );
                let pending_request = host
                    .pending_focus_command()
                    .map(|command| command.request().clone());
                (outcome, pending_request)
            })
            .expect("viewport should remain live");
        cx.run_until_parked();
        let active = window
            .update(cx, |_, window, _| window.is_window_active())
            .expect("window should remain live");

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: true,
                focus_command_queued: false,
                window_activation_requested: true,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert_eq!(pending_request, Some(DockViewportFocusRequest::panel("a")));
        assert!(
            active,
            "focus command dedupe must not suppress viewport activation"
        );
    }

    #[open_gpui::test]
    fn close_recovery_does_not_focus_when_platform_focus_is_unavailable(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, mut visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        visual.deactivate_window();
        cx.set_platform_focused_window_available(false);
        let activation = DockViewportActivationTransaction::close_recovery(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let (outcome, pending_request) = cx.update(|app| {
            let outcome = apply_viewport_activation_transaction(Some(activation), app);
            let pending_request = app.read_entity(&host, |host, _| {
                host.pending_focus_command()
                    .map(|command| command.request().clone())
            });
            (outcome, pending_request)
        });
        cx.run_until_parked();
        let active = window
            .update(cx, |_, window, cx| {
                let _ = cx;
                window.is_window_active()
            })
            .expect("window should remain live");

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: false,
                focus_command_queued: false,
                window_activation_requested: false,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert!(!active);
        assert_eq!(pending_request, None);
    }

    #[open_gpui::test]
    fn close_recovery_does_not_activate_when_platform_reports_no_active_window(
        cx: &mut TestAppContext,
    ) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, mut visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        visual.deactivate_window();
        let activation = DockViewportActivationTransaction::close_recovery(
            current_registration(cx, &host, window.window_id()),
            window,
            DockViewportFocusRequest::panel("a"),
        );

        let (outcome, pending_request) = cx.update(|app| {
            let outcome = apply_viewport_activation_transaction(Some(activation), app);
            let pending_request = app.read_entity(&host, |host, _| {
                host.pending_focus_command()
                    .map(|command| command.request().clone())
            });
            (outcome, pending_request)
        });
        cx.run_until_parked();
        let active = window
            .update(cx, |_, window, _| window.is_window_active())
            .expect("window should remain live");

        assert_eq!(
            outcome,
            DockViewportActivationApplyOutcome::Applied {
                changed: false,
                focus_command_queued: false,
                window_activation_requested: false,
                backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                backend_focus_apply: unchanged_backend_focus_apply(),
            }
        );
        assert!(!active);
        assert_eq!(pending_request, None);
    }

    #[open_gpui::test]
    fn platform_activation_does_not_override_pending_viewport_activation(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a", "b"]);
        let (_window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
            size(px(320.0), px(240.0)),
        );

        let (changed, pending) = host.update(cx, |host, _| {
            assert!(host.request_viewport_focus_command(
                DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel("a"))
            ));
            let changed = host.request_viewport_focus_command(
                DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel("a")),
            );
            let pending = host
                .pending_focus_command()
                .map(|command| (command.request().clone(), command.source()));
            (changed, pending)
        });

        assert!(!changed);
        assert_eq!(
            pending,
            Some((
                DockViewportFocusRequest::panel("a"),
                DockViewportFocusCommandSource::ViewportActivation,
            ))
        );
    }

    #[open_gpui::test]
    fn platform_activation_does_not_override_pending_close_recovery(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a", "b"]);
        let (_window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
            size(px(320.0), px(240.0)),
        );

        let (changed, pending) = host.update(cx, |host, _| {
            assert!(
                host.request_viewport_focus_command(DockViewportFocusCommand::new(
                    DockViewportFocusCommandSource::CloseRecovery,
                    DockViewportFocusRequest::panel("a"),
                ))
            );
            let changed = host.request_viewport_focus_command(
                DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel("a")),
            );
            let pending = host
                .pending_focus_command()
                .map(|command| (command.request().clone(), command.source()));
            (changed, pending)
        });

        assert!(!changed);
        assert_eq!(
            pending,
            Some((
                DockViewportFocusRequest::panel("a"),
                DockViewportFocusCommandSource::CloseRecovery,
            ))
        );
    }
}
