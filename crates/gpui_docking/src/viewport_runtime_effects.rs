use crate::{
    DockViewportRuntime, DockViewportRuntimeWorkContext, DockViewportWindowRetirement,
    DockViewportWindowRetirementKey,
    surface::{
        DockSurfaceChangeCategory, DockSurfaceTransactionId,
        window_session::DockSurfaceWindowSessionLease,
    },
    viewport_runtime_handle::DockViewportRuntimeIdentity,
    workspace_drop_transaction::DockWorkspaceLockedPayloadDropCommitId,
};
use open_gpui::{AnyWindowHandle, App, WindowId};
use std::{
    any::Any,
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportWindowCloseEffect {
    window: AnyWindowHandle,
    retirement: DockViewportWindowRetirementKey,
}

impl DockViewportWindowCloseEffect {
    pub(crate) fn from_retirement(
        window: AnyWindowHandle,
        retirement: DockViewportWindowRetirement,
    ) -> Option<Self> {
        let retirement = retirement.key()?;
        debug_assert_eq!(
            window.window_id(),
            retirement.window_id(),
            "dock viewport close effect must carry its window retirement ticket"
        );
        (window.window_id() == retirement.window_id()).then_some(Self { window, retirement })
    }

    pub(crate) fn window(self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn retirement(self) -> DockViewportWindowRetirementKey {
        self.retirement
    }

    #[cfg(test)]
    pub(crate) fn for_test(window: AnyWindowHandle) -> Self {
        Self {
            window,
            retirement: DockViewportWindowRetirementKey::for_test(window.window_id()),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportWindowEffects {
    close_now: Vec<DockViewportWindowCloseEffect>,
    refresh: Vec<AnyWindowHandle>,
    close_after_current_effect: Vec<DockViewportWindowCloseEffect>,
}

impl DockViewportWindowEffects {
    pub(crate) fn new(
        close_now: impl IntoIterator<Item = DockViewportWindowCloseEffect>,
        refresh: impl IntoIterator<Item = AnyWindowHandle>,
        close_after_current_effect: impl IntoIterator<Item = DockViewportWindowCloseEffect>,
    ) -> Self {
        let mut effects = Self::default();
        extend_unique_close_effects(&mut effects.close_now, close_now);
        extend_unique_windows(&mut effects.refresh, refresh);
        extend_unique_close_effects(
            &mut effects.close_after_current_effect,
            close_after_current_effect,
        );
        effects
    }

    pub(crate) fn refresh_only(refresh: impl IntoIterator<Item = AnyWindowHandle>) -> Self {
        Self::new(Vec::new(), refresh, Vec::new())
    }

    pub(crate) fn close_now_only(close: DockViewportWindowCloseEffect) -> Self {
        Self::new([close], Vec::new(), Vec::new())
    }

    pub(crate) fn merge(mut self, other: Self) -> Self {
        extend_unique_close_effects(&mut self.close_now, other.close_now);
        extend_unique_windows(&mut self.refresh, other.refresh);
        extend_unique_close_effects(
            &mut self.close_after_current_effect,
            other.close_after_current_effect,
        );
        self
    }

    pub(crate) fn close_now(&self) -> &[DockViewportWindowCloseEffect] {
        &self.close_now
    }

    pub(crate) fn refresh(&self) -> &[AnyWindowHandle] {
        &self.refresh
    }

    pub(crate) fn close_after_current_effect(&self) -> &[DockViewportWindowCloseEffect] {
        &self.close_after_current_effect
    }

    pub(crate) fn has_effects(&self) -> bool {
        !self.close_now.is_empty()
            || !self.refresh.is_empty()
            || !self.close_after_current_effect.is_empty()
    }
}

fn extend_unique_close_effects(
    effects: &mut Vec<DockViewportWindowCloseEffect>,
    next_effects: impl IntoIterator<Item = DockViewportWindowCloseEffect>,
) {
    for effect in next_effects {
        if effects
            .iter()
            .any(|existing| existing.retirement() == effect.retirement())
        {
            continue;
        }
        effects.push(effect);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockViewportRuntimeCommitAuthority {
    Active(DockViewportRuntimeWorkContext),
    FrozenSurfaceShutdown(DockViewportRuntimeWorkContext),
}

impl DockViewportRuntimeCommitAuthority {
    pub(crate) const fn work_context(self) -> DockViewportRuntimeWorkContext {
        match self {
            Self::Active(context) | Self::FrozenSurfaceShutdown(context) => context,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DockViewportRuntimeUpdate {
    changed: bool,
    windows: Vec<AnyWindowHandle>,
    work_context: Option<DockViewportRuntimeWorkContext>,
    surface_transaction: Option<DockSurfaceTransactionId>,
    change_categories: Vec<DockSurfaceChangeCategory>,
}

#[must_use = "a frozen surface shutdown reservation must commit before windows close"]
#[derive(Debug)]
pub(crate) struct DockViewportSurfaceShutdownReservation {
    lease: DockSurfaceWindowSessionLease,
    windows: Vec<(crate::DockViewportWindowRole, AnyWindowHandle)>,
}

impl DockViewportSurfaceShutdownReservation {
    pub(crate) fn new(
        lease: DockSurfaceWindowSessionLease,
        windows: Vec<(crate::DockViewportWindowRole, AnyWindowHandle)>,
    ) -> Self {
        Self { lease, windows }
    }

    #[cfg(test)]
    pub(crate) const fn lease(&self) -> DockSurfaceWindowSessionLease {
        self.lease
    }

    pub(crate) fn windows(&self) -> &[(crate::DockViewportWindowRole, AnyWindowHandle)] {
        &self.windows
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        DockSurfaceWindowSessionLease,
        Vec<(crate::DockViewportWindowRole, AnyWindowHandle)>,
    ) {
        (self.lease, self.windows)
    }
}

#[must_use = "surface shutdown effects must publish their cleanup commit before windows close"]
#[derive(Debug)]
pub(crate) struct DockViewportSurfaceShutdownEffects {
    lease: DockSurfaceWindowSessionLease,
    windows: Vec<(crate::DockViewportWindowRole, AnyWindowHandle)>,
    cleanup_update: DockViewportRuntimeUpdate,
}

impl DockViewportSurfaceShutdownEffects {
    pub(crate) fn new(
        lease: DockSurfaceWindowSessionLease,
        windows: Vec<(crate::DockViewportWindowRole, AnyWindowHandle)>,
        cleanup_update: DockViewportRuntimeUpdate,
    ) -> Self {
        Self {
            lease,
            windows,
            cleanup_update,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        DockSurfaceWindowSessionLease,
        Vec<(crate::DockViewportWindowRole, AnyWindowHandle)>,
        DockViewportRuntimeUpdate,
    ) {
        (self.lease, self.windows, self.cleanup_update)
    }
}

impl DockViewportRuntimeUpdate {
    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) const fn work_context(&self) -> Option<DockViewportRuntimeWorkContext> {
        self.work_context
    }

    pub(crate) fn bind_work_context(&mut self, context: DockViewportRuntimeWorkContext) {
        if let Some(current) = self.work_context {
            assert_eq!(
                current, context,
                "cannot merge viewport runtime updates from different work contexts"
            );
        } else {
            self.work_context = Some(context);
        }
    }

    pub(crate) fn mark_changed(&mut self, changed: bool) {
        self.changed |= changed;
    }

    pub(crate) fn mark_viewport_topology(
        &mut self,
        changed: bool,
        context: DockViewportRuntimeWorkContext,
    ) {
        self.mark_category(
            changed,
            context,
            DockSurfaceChangeCategory::ViewportTopology,
        );
    }

    pub(crate) fn mark_graph_commit(
        &mut self,
        changed: bool,
        context: DockViewportRuntimeWorkContext,
    ) {
        for category in [
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
            DockSurfaceChangeCategory::PanelLifecycle,
        ] {
            self.mark_category(changed, context, category);
        }
    }

    pub(crate) fn mark_observed_viewport_placement(
        &mut self,
        changed: bool,
        context: DockViewportRuntimeWorkContext,
    ) {
        self.mark_category(
            changed,
            context,
            DockSurfaceChangeCategory::ObservedViewportPlacement,
        );
    }

    pub(crate) fn surface_transaction(&self) -> Option<DockSurfaceTransactionId> {
        self.surface_transaction
    }

    pub(crate) fn change_categories(&self) -> &[DockSurfaceChangeCategory] {
        &self.change_categories
    }

    pub(crate) fn extend_windows(&mut self, windows: impl IntoIterator<Item = AnyWindowHandle>) {
        extend_unique_windows(&mut self.windows, windows);
    }

    pub(crate) fn merge(&mut self, update: DockViewportRuntimeUpdate) {
        let DockViewportRuntimeUpdate {
            changed,
            windows,
            work_context,
            surface_transaction,
            change_categories,
        } = update;
        self.mark_changed(changed);
        self.extend_windows(windows);
        if let Some(work_context) = work_context {
            self.bind_work_context(work_context);
        }
        if !change_categories.is_empty() {
            self.merge_surface_transaction(surface_transaction);
        }
        self.extend_change_categories(change_categories);
    }

    pub(crate) fn into_windows(self) -> Vec<AnyWindowHandle> {
        self.windows
    }

    fn mark_category(
        &mut self,
        changed: bool,
        context: DockViewportRuntimeWorkContext,
        category: DockSurfaceChangeCategory,
    ) {
        self.mark_changed(changed);
        if changed {
            self.bind_work_context(context);
            self.merge_surface_transaction(context.surface_transaction());
            self.extend_change_categories([category]);
        }
    }

    fn merge_surface_transaction(&mut self, surface_transaction: Option<DockSurfaceTransactionId>) {
        if self.change_categories.is_empty() {
            self.surface_transaction = surface_transaction;
        } else {
            assert_eq!(
                self.surface_transaction, surface_transaction,
                "cannot merge viewport runtime commits from different surface transactions"
            );
        }
    }

    fn extend_change_categories(
        &mut self,
        categories: impl IntoIterator<Item = DockSurfaceChangeCategory>,
    ) {
        for category in categories {
            if !self.change_categories.contains(&category) {
                self.change_categories.push(category);
            }
        }
    }
}

pub(crate) fn extend_unique_windows(
    windows: &mut Vec<AnyWindowHandle>,
    next_windows: impl IntoIterator<Item = AnyWindowHandle>,
) {
    for window in next_windows {
        if windows
            .iter()
            .any(|existing| existing.window_id() == window.window_id())
        {
            continue;
        }
        windows.push(window);
    }
}

pub(crate) fn unique_windows(windows: Vec<AnyWindowHandle>) -> Vec<AnyWindowHandle> {
    let mut unique = Vec::new();
    extend_unique_windows(&mut unique, windows);
    unique
}

fn unique_windows_excluding(
    windows: Vec<AnyWindowHandle>,
    excluded_window: Option<WindowId>,
) -> Vec<AnyWindowHandle> {
    unique_windows(windows)
        .into_iter()
        .filter(|window| Some(window.window_id()) != excluded_window)
        .collect()
}

pub(crate) fn refresh_windows<C: open_gpui::AppContext>(windows: Vec<AnyWindowHandle>, cx: &mut C) {
    refresh_windows_excluding(windows, None, cx);
}

pub(crate) fn refresh_windows_excluding<C: open_gpui::AppContext>(
    windows: Vec<AnyWindowHandle>,
    excluded_window: Option<WindowId>,
    cx: &mut C,
) {
    for window in unique_windows_excluding(windows, excluded_window) {
        let _ = window.update(cx, |_, window, _| window.refresh());
    }
}

pub(crate) fn refresh_runtime_update<C: open_gpui::AppContext>(
    update: DockViewportRuntimeUpdate,
    cx: &mut C,
) -> bool {
    refresh_runtime_update_excluding(update, None, cx)
}

pub(crate) fn refresh_runtime_update_excluding<C: open_gpui::AppContext>(
    update: DockViewportRuntimeUpdate,
    excluded_window: Option<WindowId>,
    cx: &mut C,
) -> bool {
    let changed = update.changed();
    refresh_windows_excluding(update.into_windows(), excluded_window, cx);
    changed
}

pub(crate) fn close_window_quietly(window: AnyWindowHandle, cx: &mut App) {
    let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
}

fn settle_and_close_windows_quietly(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    effects: Vec<DockViewportWindowCloseEffect>,
    cx: &mut App,
) {
    let mut pending = Vec::new();
    for effect in effects {
        let should_close = match runtime.try_borrow_mut() {
            Ok(mut runtime) => runtime.settle_window_retirement(effect.retirement()),
            Err(error) => {
                log::debug!(
                    "dock viewport retirement was reentered while the runtime was borrowed; \
                     deferring the exact retirement effect: {error}"
                );
                pending.push(effect);
                continue;
            }
        };
        if should_close {
            close_window_quietly(effect.window(), cx);
        }
    }
    if pending.is_empty() {
        return;
    }

    let runtime = runtime.clone();
    cx.defer_after_or_shutdown_critical_before_window_registry_clear(Duration::ZERO, move |cx| {
        settle_and_close_windows_quietly(&runtime, pending, cx)
    });
}

fn close_windows_after_current_effect(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    effects: Vec<DockViewportWindowCloseEffect>,
    cx: &mut App,
) {
    if effects.is_empty() {
        return;
    }
    let runtime = runtime.clone();
    cx.defer_shutdown_critical_before_window_registry_clear_or_run_now(move |cx| {
        settle_and_close_windows_quietly(&runtime, effects, cx)
    });
}

/// Exact proof that one committed host drop transferred its window effects to App-owned work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockViewportCommittedWindowEffectsReceipt {
    runtime: DockViewportRuntimeIdentity,
    commit_id: DockWorkspaceLockedPayloadDropCommitId,
}

impl DockViewportCommittedWindowEffectsReceipt {
    pub(crate) const fn runtime(self) -> DockViewportRuntimeIdentity {
        self.runtime
    }

    pub(crate) const fn commit_id(self) -> DockWorkspaceLockedPayloadDropCommitId {
        self.commit_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockViewportCommittedWindowEffectsAcceptanceOutcome {
    Accepted(DockViewportCommittedWindowEffectsReceipt),
    InProgress,
    Stale,
}

/// Replay-safe owner for the window effects produced by one committed host drop.
///
/// The shared state is intentional: the viewport runtime tombstone and every upper-layer journal
/// clone observe the same exact acceptance. Reentry while dispatch is in progress sees `None` and
/// retries after the current App turn instead of applying the same close effects recursively.
#[derive(Clone, Debug)]
pub(crate) struct DockViewportCommittedWindowEffects {
    inner: Rc<RefCell<DockViewportCommittedWindowEffectsState>>,
}

#[derive(Debug)]
struct DockViewportCommittedWindowEffectsState {
    runtime: DockViewportRuntimeIdentity,
    commit_id: DockWorkspaceLockedPayloadDropCommitId,
    phase: DockViewportCommittedWindowEffectsPhase,
}

#[derive(Debug)]
enum DockViewportCommittedWindowEffectsPhase {
    Pending(DockViewportWindowEffects),
    Dispatching,
    Accepted(DockViewportCommittedWindowEffectsReceipt),
}

pub(crate) enum DockViewportCommittedWindowEffectsPreparation {
    Accepted(DockViewportCommittedWindowEffectsReceipt),
    InProgress,
    Transfer(DockViewportCommittedWindowEffectsTransfer),
    Stale,
}

pub(crate) struct DockViewportCommittedWindowEffectsTransfer {
    effects: DockViewportCommittedWindowEffects,
    pending: Option<DockViewportWindowEffects>,
}

impl DockViewportCommittedWindowEffectsTransfer {
    pub(crate) fn accept(
        mut self,
        runtime: &Rc<RefCell<DockViewportRuntime>>,
        cx: &mut App,
    ) -> DockViewportCommittedWindowEffectsReceipt {
        let pending = self
            .pending
            .as_ref()
            .expect("a window-effects transfer must retain its pending batch");
        if pending.has_effects() {
            let runtime = runtime.clone();
            let effects = pending.clone();
            cx.defer_after_or_shutdown_critical_before_window_registry_clear(
                Duration::ZERO,
                move |cx| apply_committed_viewport_window_effects_job(&runtime, effects, cx),
            );
        }
        let receipt = self.effects.finish_transfer();
        self.pending = None;
        receipt
    }
}

impl Drop for DockViewportCommittedWindowEffectsTransfer {
    fn drop(&mut self) {
        if let Some(effects) = self.pending.take() {
            self.effects.restore_interrupted_transfer(effects);
        }
    }
}

impl DockViewportCommittedWindowEffects {
    pub(crate) fn new(
        runtime: DockViewportRuntimeIdentity,
        commit_id: DockWorkspaceLockedPayloadDropCommitId,
        effects: DockViewportWindowEffects,
    ) -> Self {
        Self {
            inner: Rc::new(RefCell::new(DockViewportCommittedWindowEffectsState {
                runtime,
                commit_id,
                phase: DockViewportCommittedWindowEffectsPhase::Pending(effects),
            })),
        }
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn receipt(&self) -> Option<DockViewportCommittedWindowEffectsReceipt> {
        match &self.inner.borrow().phase {
            DockViewportCommittedWindowEffectsPhase::Accepted(receipt) => Some(*receipt),
            DockViewportCommittedWindowEffectsPhase::Pending(_)
            | DockViewportCommittedWindowEffectsPhase::Dispatching => None,
        }
    }

    pub(crate) fn prepare_acceptance(
        &self,
        runtime: DockViewportRuntimeIdentity,
        commit_id: DockWorkspaceLockedPayloadDropCommitId,
    ) -> DockViewportCommittedWindowEffectsPreparation {
        let pending = {
            let mut state = self.inner.borrow_mut();
            if state.runtime != runtime || state.commit_id != commit_id {
                return DockViewportCommittedWindowEffectsPreparation::Stale;
            }
            let phase = std::mem::replace(
                &mut state.phase,
                DockViewportCommittedWindowEffectsPhase::Dispatching,
            );
            match phase {
                DockViewportCommittedWindowEffectsPhase::Accepted(receipt) => {
                    state.phase = DockViewportCommittedWindowEffectsPhase::Accepted(receipt);
                    return DockViewportCommittedWindowEffectsPreparation::Accepted(receipt);
                }
                DockViewportCommittedWindowEffectsPhase::Dispatching => {
                    state.phase = DockViewportCommittedWindowEffectsPhase::Dispatching;
                    return DockViewportCommittedWindowEffectsPreparation::InProgress;
                }
                DockViewportCommittedWindowEffectsPhase::Pending(effects) => effects,
            }
        };
        DockViewportCommittedWindowEffectsPreparation::Transfer(
            DockViewportCommittedWindowEffectsTransfer {
                effects: self.clone(),
                pending: Some(pending),
            },
        )
    }

    fn finish_transfer(&self) -> DockViewportCommittedWindowEffectsReceipt {
        let mut state = self.inner.borrow_mut();
        assert!(
            matches!(
                state.phase,
                DockViewportCommittedWindowEffectsPhase::Dispatching
            ),
            "committed window effects must finish the exact in-flight transfer"
        );
        let receipt = DockViewportCommittedWindowEffectsReceipt {
            runtime: state.runtime,
            commit_id: state.commit_id,
        };
        state.phase = DockViewportCommittedWindowEffectsPhase::Accepted(receipt);
        receipt
    }

    fn restore_interrupted_transfer(&self, effects: DockViewportWindowEffects) {
        let mut state = self.inner.borrow_mut();
        if matches!(
            state.phase,
            DockViewportCommittedWindowEffectsPhase::Dispatching
        ) {
            state.phase = DockViewportCommittedWindowEffectsPhase::Pending(effects);
        }
    }
}

fn record_window_effect_panic(
    first_panic: &mut Option<Box<dyn Any + Send>>,
    callback: impl FnOnce(),
) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(callback))
        && first_panic.is_none()
    {
        *first_panic = Some(payload);
    }
}

fn apply_committed_viewport_window_effects_job(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    effects: DockViewportWindowEffects,
    cx: &mut App,
) {
    let mut first_panic = None;
    for effect in effects.close_now().iter().copied() {
        record_window_effect_panic(&mut first_panic, || {
            close_windows_after_current_effect(runtime, vec![effect], cx)
        });
    }
    for window in unique_windows(effects.refresh().to_vec()) {
        record_window_effect_panic(&mut first_panic, || {
            let _ = window.update(cx, |_, window, _| window.refresh());
        });
    }
    for effect in effects.close_after_current_effect().iter().copied() {
        record_window_effect_panic(&mut first_panic, || {
            close_windows_after_current_effect(runtime, vec![effect], cx)
        });
    }
    if let Some(payload) = first_panic {
        resume_unwind(payload);
    }
}

pub(crate) fn apply_viewport_window_effects(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    effects: DockViewportWindowEffects,
    cx: &mut App,
) {
    apply_viewport_window_effects_excluding(runtime, effects, None, cx);
}

pub(crate) fn apply_viewport_window_effects_excluding(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    effects: DockViewportWindowEffects,
    excluded_window: Option<WindowId>,
    cx: &mut App,
) {
    close_windows_after_current_effect(runtime, effects.close_now().to_vec(), cx);
    refresh_windows_excluding(effects.refresh().to_vec(), excluded_window, cx);
    close_windows_after_current_effect(runtime, effects.close_after_current_effect().to_vec(), cx);
}

pub(crate) fn refresh_viewport_window_effects_excluding<C: open_gpui::AppContext>(
    effects: DockViewportWindowEffects,
    excluded_window: Option<WindowId>,
    cx: &mut C,
) {
    debug_assert!(effects.close_now().is_empty());
    debug_assert!(effects.close_after_current_effect().is_empty());
    refresh_windows_excluding(effects.refresh().to_vec(), excluded_window, cx);
}

#[cfg(test)]
mod tests {
    use super::{
        DockViewportCommittedWindowEffects, DockViewportCommittedWindowEffectsPreparation,
        DockViewportRuntimeUpdate, DockViewportWindowEffects, settle_and_close_windows_quietly,
        unique_windows, unique_windows_excluding,
    };
    use crate::{
        DockController, DockGraph, DockSpaceId, DockViewportRuntime, DockViewportRuntimeIdentity,
        DockViewportRuntimeLineage, DockViewportRuntimeWorkContext, DockWorkspace,
        surface::DockSurfaceChangeCategory, viewport_test_support::handle,
        workspace_drop_transaction::DockWorkspaceLockedPayloadDropCommitId,
    };
    use open_gpui::{AppContext as _, Empty, TestAppContext, px, size};
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn committed_window_effects_replay_exact_acceptance_after_interrupted_transfer() {
        let runtime = DockViewportRuntimeIdentity::for_test(3);
        let commit_id = DockWorkspaceLockedPayloadDropCommitId::new(7);
        let effects = DockViewportCommittedWindowEffects::new(
            runtime,
            commit_id,
            DockViewportWindowEffects::default(),
        );

        let interrupted = match effects.prepare_acceptance(runtime, commit_id) {
            DockViewportCommittedWindowEffectsPreparation::Transfer(transfer) => transfer,
            _ => panic!("the first exact acceptance must prepare one transfer"),
        };
        drop(interrupted);
        assert_eq!(effects.receipt(), None);

        let mut replay = match effects.prepare_acceptance(runtime, commit_id) {
            DockViewportCommittedWindowEffectsPreparation::Transfer(transfer) => transfer,
            _ => panic!("an interrupted transfer must restore the pending batch"),
        };
        let receipt = replay.effects.finish_transfer();
        replay.pending = None;
        drop(replay);
        assert_eq!(receipt.runtime(), runtime);
        assert_eq!(receipt.commit_id(), commit_id);

        assert!(matches!(
            effects.prepare_acceptance(runtime, commit_id),
            DockViewportCommittedWindowEffectsPreparation::Accepted(replayed)
                if replayed == receipt
        ));
    }

    #[test]
    fn committed_window_effects_reentry_observes_dispatch_in_flight() {
        let runtime = DockViewportRuntimeIdentity::for_test(5);
        let commit_id = DockWorkspaceLockedPayloadDropCommitId::new(11);
        let effects = DockViewportCommittedWindowEffects::new(
            runtime,
            commit_id,
            DockViewportWindowEffects::default(),
        );

        let transfer = match effects.prepare_acceptance(runtime, commit_id) {
            DockViewportCommittedWindowEffectsPreparation::Transfer(transfer) => transfer,
            _ => panic!("the first exact acceptance must prepare one transfer"),
        };
        assert!(matches!(
            effects.prepare_acceptance(runtime, commit_id),
            DockViewportCommittedWindowEffectsPreparation::InProgress
        ));
        assert!(matches!(
            effects.prepare_acceptance(DockViewportRuntimeIdentity::for_test(6), commit_id),
            DockViewportCommittedWindowEffectsPreparation::Stale
        ));

        drop(transfer);
        assert!(matches!(
            effects.prepare_acceptance(runtime, commit_id),
            DockViewportCommittedWindowEffectsPreparation::Transfer(_)
        ));
    }

    #[test]
    fn unique_windows_preserves_first_occurrence_order() {
        let first = handle(1);
        let second = handle(2);

        assert_eq!(
            unique_windows(vec![first, second, first, second, first]),
            vec![first, second]
        );
    }

    #[test]
    fn unique_windows_excluding_never_reenters_current_window() {
        let current = handle(1);
        let other = handle(2);

        assert_eq!(
            unique_windows_excluding(
                vec![current, other, current, other],
                Some(current.window_id()),
            ),
            vec![other]
        );
    }

    #[test]
    fn generic_runtime_changes_do_not_claim_surface_commit_categories() {
        let mut update = DockViewportRuntimeUpdate::default();

        update.mark_changed(true);

        assert!(update.changed());
        assert!(update.change_categories().is_empty());
        assert_eq!(update.surface_transaction(), None);
    }

    #[test]
    fn explicit_runtime_commit_categories_merge_without_duplicates() {
        let context =
            DockViewportRuntimeWorkContext::new(crate::DockViewportRuntimeLineage::Unmanaged, None);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_viewport_topology(true, context);
        update.mark_viewport_topology(true, context);

        let mut observed = DockViewportRuntimeUpdate::default();
        observed.mark_observed_viewport_placement(true, context);
        update.merge(observed);

        assert_eq!(
            update.change_categories(),
            [
                DockSurfaceChangeCategory::ViewportTopology,
                DockSurfaceChangeCategory::ObservedViewportPlacement,
            ]
        );
    }

    #[open_gpui::test]
    fn runtime_busy_retirement_waits_for_authority_before_closing(cx: &mut TestAppContext) {
        let controller = cx.new(|_| {
            DockController::new(DockWorkspace::new(
                DockSpaceId::from("main"),
                DockGraph::new(),
            ))
        });
        let runtime = Rc::new(RefCell::new(DockViewportRuntime::new(controller)));
        let window = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        let any_window = window.into();
        let effect = {
            let mut runtime = runtime.borrow_mut();
            let attempt = runtime
                .begin_window_open_attempt(any_window, DockViewportRuntimeLineage::Unmanaged)
                .expect("the test window should reserve one runtime ownership generation");
            runtime
                .retire_window_open_attempt_for_close(attempt, any_window)
                .expect("the exact opening generation should produce one retirement effect")
        };

        let runtime_borrow = runtime.borrow_mut();
        cx.update(|app| {
            settle_and_close_windows_quietly(&runtime, vec![effect], app);
        });
        assert!(
            window.update(cx, |_, _, _| ()).is_ok(),
            "a busy runtime must not trigger native close before its retirement authority settles"
        );

        drop(runtime_borrow);
        cx.run_until_parked();
        assert!(
            window.update(cx, |_, _, _| ()).is_err(),
            "the exact retirement effect must close after the runtime borrow is released"
        );
    }
}
