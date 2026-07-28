use crate::{
    DockViewportRuntime, DockViewportRuntimeWorkContext, DockViewportWindowRetirement,
    DockViewportWindowRetirementKey,
    surface::{
        DockSurfaceChangeCategory, DockSurfaceTransactionId,
        window_session::DockSurfaceWindowSessionLease,
    },
};
use open_gpui::{AnyWindowHandle, App, WindowId};
use std::{cell::RefCell, rc::Rc};

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

#[derive(Debug, Default)]
pub(crate) struct DockViewportRuntimeUpdate {
    changed: bool,
    windows: Vec<AnyWindowHandle>,
    work_context: Option<DockViewportRuntimeWorkContext>,
    surface_transaction: Option<DockSurfaceTransactionId>,
    change_categories: Vec<DockSurfaceChangeCategory>,
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

    #[cfg(test)]
    pub(crate) const fn lease(&self) -> DockSurfaceWindowSessionLease {
        self.lease
    }

    #[cfg(test)]
    pub(crate) fn windows(&self) -> &[(crate::DockViewportWindowRole, AnyWindowHandle)] {
        &self.windows
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
    for effect in effects {
        let should_close = runtime
            .borrow_mut()
            .settle_window_retirement(effect.retirement());
        if should_close {
            close_window_quietly(effect.window(), cx);
        }
    }
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
    cx.defer(move |cx| settle_and_close_windows_quietly(&runtime, effects, cx));
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
    use super::{DockViewportRuntimeUpdate, unique_windows, unique_windows_excluding};
    use crate::{
        DockViewportRuntimeWorkContext, surface::DockSurfaceChangeCategory,
        viewport_test_support::handle,
    };

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
}
