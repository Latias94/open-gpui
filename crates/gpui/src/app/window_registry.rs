use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Weak,
};

use crate::{AnyView, AnyWindowHandle, PreparedPlatformPresentationShutdown, Window, WindowId};

use super::{App, AppCell, Effect, QuitMode, native_captured_drag::WindowUpdateProvenance};

#[derive(Debug, thiserror::Error)]
pub(super) enum WindowReservationError {
    #[error("application shutdown invalidated the window reservation")]
    AppShutdown,
    #[error("window closed before its creation transaction committed")]
    WindowClosed,
    #[error("window reservation is no longer current")]
    NotCurrent,
    #[error("provisional window session could not bind at registry commit: {0}")]
    ProvisionalSession(#[from] crate::WindowProvisionalSessionError),
}

pub(super) struct ReservedWindow<'a> {
    app: &'a mut App,
    id: WindowId,
    epoch: u64,
    pending: bool,
}

/// Retains a native window created during an in-flight open transaction until the normal
/// app-owned retirement queue can drain its exact presentation-shutdown authority.
///
/// A reservation is intentionally not a committed window, so merely dropping the local `Window`
/// would bypass `NativeWindowRetirement`. This guard makes every post-native-create failure use
/// the same renderer-before-native-terminal protocol as an ordinary logical close.
pub(super) struct WindowCreationRollback {
    app_cell: Weak<AppCell>,
    window_id: WindowId,
    window: Option<Window>,
}

impl WindowCreationRollback {
    pub(super) fn new(window_id: WindowId, window: Window, app_cell: Weak<AppCell>) -> Self {
        Self {
            app_cell,
            window_id,
            window: Some(window),
        }
    }

    pub(super) fn window_mut(&mut self) -> &mut Window {
        self.window
            .as_mut()
            .expect("window-creation rollback must retain its window before commit")
    }

    fn into_window(mut self) -> Window {
        self.window
            .take()
            .expect("window-creation rollback must retain its window before commit")
    }
}

impl Drop for WindowCreationRollback {
    fn drop(&mut self) {
        let Some(window) = self.window.take() else {
            return;
        };

        if let Some(cell) = self.app_cell.upgrade() {
            cell.enqueue_native_window_retirement(self.window_id, Box::new(window));
        } else {
            // The app cell should outlive every synchronous open transaction. If that invariant
            // is broken, leak the native owner rather than dropping a live backend window without
            // an exact retirement authority.
            log::error!(
                "native window creation rollback lost its AppCell; retaining the backend owner until process teardown"
            );
            std::mem::forget(window);
        }
    }
}

impl ReservedWindow<'_> {
    pub(super) fn id(&self) -> WindowId {
        self.id
    }

    pub(super) fn app_mut(&mut self) -> &mut App {
        &mut *self.app
    }

    pub(super) fn with_update_scope<R>(&mut self, update: impl FnOnce(&mut App) -> R) -> R {
        WindowUpdateStackScope::new(&mut *self.app, self.id).run(update)
    }

    pub(super) fn validate(&self) -> Result<(), WindowReservationError> {
        validate_reservation(self.app, self.id, self.epoch)
    }

    pub(super) fn commit(
        mut self,
        mut rollback: WindowCreationRollback,
    ) -> Result<(), WindowReservationError> {
        self.validate()?;
        if !rollback.window_mut().creation_can_commit() {
            return Err(WindowReservationError::WindowClosed);
        }
        rollback.window_mut().bind_provisional_session()?;
        let mut window = rollback.into_window();
        let initial_presentation = window.take_initial_presentation_command();
        commit_reserved(&mut *self.app, self.id, window);
        self.pending = false;
        if let Some((sink, command)) = initial_presentation {
            sink.enqueue(command);
        }
        Ok(())
    }
}

impl Drop for ReservedWindow<'_> {
    fn drop(&mut self) {
        if self.pending {
            rollback_reserved(&mut *self.app, self.id);
            self.pending = false;
        }
    }
}

pub(super) fn reserve(app: &mut App) -> Result<ReservedWindow<'_>, WindowReservationError> {
    if app.window_open_barrier_depth > 0 {
        return Err(WindowReservationError::AppShutdown);
    }

    let epoch = app.window_open_epoch;
    let id = app.windows.insert(None);
    if let Some(cell) = app.this.upgrade() {
        cell.reserve_native_window(id);
    }
    Ok(ReservedWindow {
        app,
        id,
        epoch,
        pending: true,
    })
}

fn validate_reservation(app: &App, id: WindowId, epoch: u64) -> Result<(), WindowReservationError> {
    if app.window_open_barrier_depth > 0 || app.window_open_epoch != epoch {
        return Err(WindowReservationError::AppShutdown);
    }
    if !app.windows.get(id).is_some_and(Option::is_none) {
        return Err(WindowReservationError::NotCurrent);
    }
    Ok(())
}

fn commit_reserved(app: &mut App, id: WindowId, window: Window) {
    let slot = app
        .windows
        .get_mut(id)
        .expect("validated window reservation must retain its slot");
    assert!(
        slot.is_none(),
        "validated window reservation must remain pending until commit"
    );
    slot.replace(Box::new(window));
    let window = slot
        .as_ref()
        .expect("committed window reservation must retain its window");
    app.window_handles.insert(id, window.handle);
    app.window_profiles.insert(id, window.window_profile());
    if let Some(cell) = app.this.upgrade() {
        cell.commit_native_window(id);
    }
}

fn rollback_reserved(app: &mut App, id: WindowId) {
    app.pending_effects.retain(|effect| {
        !matches!(
            effect,
            Effect::EntityCreated {
                window: Some(window),
                ..
            } if *window == id
        )
    });
    app.window_handles.remove(&id);
    app.window_profiles.remove(&id);
    cleanup_entity_window_links(app, id);
    if app.windows.remove(id).is_some()
        && let Some(cell) = app.this.upgrade()
    {
        cell.remove_native_window(id);
    }
}

struct WindowUpdateStackScope<'a> {
    app: &'a mut App,
    previous_stack: Vec<WindowId>,
    previous_provenance: WindowUpdateProvenance,
}

impl<'a> WindowUpdateStackScope<'a> {
    fn new(app: &'a mut App, id: WindowId) -> Self {
        let previous_stack = app.window_update_stack.clone();
        app.window_update_stack.push(id);
        let previous_provenance = std::mem::replace(
            &mut app.window_update_provenance,
            WindowUpdateProvenance::Ordinary,
        );
        Self {
            app,
            previous_stack,
            previous_provenance,
        }
    }

    fn run<R>(self, update: impl FnOnce(&mut App) -> R) -> R {
        update(&mut *self.app)
    }
}

impl Drop for WindowUpdateStackScope<'_> {
    fn drop(&mut self) {
        self.app.window_update_stack = std::mem::take(&mut self.previous_stack);
        self.app.window_update_provenance = self.previous_provenance;
    }
}

pub(super) struct WindowUpdateTransaction<'a> {
    app: &'a mut App,
    id: WindowId,
    root_view: AnyView,
    window: Option<Box<Window>>,
    previous_stack: Option<Vec<WindowId>>,
    previous_provenance: Option<WindowUpdateProvenance>,
}

impl<'a> WindowUpdateTransaction<'a> {
    pub(super) fn begin(
        app: &'a mut App,
        id: WindowId,
        provenance: WindowUpdateProvenance,
    ) -> Option<Self> {
        let root_view = app.windows.get(id)?.as_ref()?.root.clone()?;
        let window = app.windows.get_mut(id)?.take()?;
        let previous_stack = app.window_update_stack.clone();
        app.window_update_stack.push(window.handle.id);
        let previous_provenance = std::mem::replace(&mut app.window_update_provenance, provenance);
        Some(Self {
            app,
            id,
            root_view,
            window: Some(window),
            previous_stack: Some(previous_stack),
            previous_provenance: Some(previous_provenance),
        })
    }

    pub(super) fn update<T>(
        &mut self,
        update: impl FnOnce(AnyView, &mut Window, &mut App) -> T,
    ) -> T {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Self {
                app,
                root_view,
                window,
                ..
            } = self;
            update(
                root_view.clone(),
                window
                    .as_deref_mut()
                    .expect("window update transaction must own its window"),
                &mut **app,
            )
        }));

        match result {
            Ok(result) => result,
            Err(payload) => {
                let _ = catch_unwind(AssertUnwindSafe(|| self.settle_after_update_panic()));
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(super) fn finish(mut self) -> Option<()> {
        self.restore_stack();
        let window = self
            .window
            .take()
            .expect("window update transaction must own its window");
        finish_window_update(&mut *self.app, self.id, window)
    }

    fn restore_stack(&mut self) {
        if let Some(previous_stack) = self.previous_stack.take() {
            self.app.window_update_stack = previous_stack;
        }
        if let Some(previous_provenance) = self.previous_provenance.take() {
            self.app.window_update_provenance = previous_provenance;
        }
    }

    fn settle_after_update_panic(&mut self) {
        self.restore_stack();
        let Some(window) = self.window.take() else {
            return;
        };
        if window.removed {
            let _ = finish_window_update(&mut *self.app, self.id, window);
        } else {
            restore_taken_window(&mut *self.app, self.id, window);
        }
    }
}

impl Drop for WindowUpdateTransaction<'_> {
    fn drop(&mut self) {
        let mut settle = || {
            self.restore_stack();
            if let Some(window) = self.window.take() {
                if window.removed {
                    let _ = finish_window_update(&mut *self.app, self.id, window);
                } else {
                    restore_taken_window(&mut *self.app, self.id, window);
                }
            }
        };

        if std::thread::panicking() {
            let _ = catch_unwind(AssertUnwindSafe(settle));
        } else {
            settle();
        }
    }
}

fn restore_taken_window(app: &mut App, id: WindowId, window: Box<Window>) {
    let Some(slot) = app.windows.get_mut(id) else {
        return;
    };
    if slot.is_none() {
        slot.replace(window);
    }
}

pub(super) fn take_all_for_shutdown(app: &mut App) -> Vec<(WindowId, Box<Window>)> {
    let windows = std::mem::take(&mut app.windows);
    app.window_handles.clear();
    app.window_profiles.clear();
    if let Some(cell) = app.this.upgrade() {
        cell.clear_native_windows();
    }
    windows
        .into_iter()
        .filter_map(|(window_id, window)| window.map(|window| (window_id, window)))
        .collect()
}

pub(super) fn prepare_presentation_shutdowns(
    app: &mut App,
) -> Vec<PreparedPlatformPresentationShutdown> {
    app.windows
        .values_mut()
        .filter_map(Option::as_mut)
        .map(|window| window.claim_presentation_shutdown())
        .collect()
}

pub(super) fn has_checked_out_window(app: &App) -> bool {
    app.windows.values().any(Option::is_none)
}

pub(super) fn handles(app: &App) -> Vec<AnyWindowHandle> {
    app.windows
        .keys()
        .flat_map(|window_id| app.window_handles.get(&window_id).copied())
        .collect()
}

pub(super) fn finish_window_update(app: &mut App, id: WindowId, window: Box<Window>) -> Option<()> {
    if window.removed {
        let first_panic = unregister_removed_window(app, id);
        if let Some(cell) = app.this.upgrade() {
            cell.enqueue_native_window_retirement(id, window);
        } else {
            // There is no retirement queue left to prove renderer quiescence. Retain the
            // platform owner rather than dropping a surface that may still have submitted work.
            log::error!(
                "removed window lost its AppCell before native retirement; retaining the backend owner until process teardown"
            );
            std::mem::forget(window);
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    } else {
        app.windows.get_mut(id)?.replace(window);
    }

    Some(())
}

fn unregister_removed_window(app: &mut App, id: WindowId) -> Option<Box<dyn Any + Send>> {
    app.window_handles.remove(&id);
    app.window_profiles.remove(&id);
    app.windows.remove(id);
    if let Some(cell) = app.this.upgrade() {
        cell.remove_native_window(id);
    }
    cleanup_entity_window_links(app, id);
    let mut first_panic = notify_window_closed(app, id);

    if !app.notifying_window_closed
        && should_quit_after_last_window(app.quit_mode)
        && app.windows.is_empty()
    {
        retain_first_panic(
            &mut first_panic,
            catch_unwind(AssertUnwindSafe(|| app.quit())).err(),
        );
    }

    first_panic
}

fn cleanup_entity_window_links(app: &mut App, id: WindowId) {
    app.view_presentation_windows
        .window_closed(id, &mut app.current_window_by_entity);
    if let Some(tracked) = app.tracked_entities.remove(&id) {
        for entity_id in tracked {
            if let Some(windows) = app.window_invalidators_by_entity.get_mut(&entity_id) {
                windows.remove(&id);
            }
            if app.current_window_by_entity.get(&entity_id) == Some(&id) {
                app.current_window_by_entity.remove(&entity_id);
            }
        }
    }
}

fn notify_window_closed(app: &mut App, id: WindowId) -> Option<Box<dyn Any + Send>> {
    app.pending_window_closed_notifications.push_back(id);
    if app.notifying_window_closed {
        return None;
    }

    app.notifying_window_closed = true;
    let mut first_panic = None;
    while let Some(closed_window) = app.pending_window_closed_notifications.pop_front() {
        app.window_closed_observers.clone().retain(&(), |callback| {
            retain_first_panic(
                &mut first_panic,
                catch_unwind(AssertUnwindSafe(|| callback(app, closed_window))).err(),
            );
            true
        });
    }
    app.notifying_window_closed = false;
    first_panic
}

fn retain_first_panic(
    first_panic: &mut Option<Box<dyn Any + Send>>,
    candidate: Option<Box<dyn Any + Send>>,
) {
    if first_panic.is_none() {
        *first_panic = candidate;
    }
}

fn should_quit_after_last_window(quit_mode: QuitMode) -> bool {
    match quit_mode {
        QuitMode::Explicit => false,
        QuitMode::LastWindowClosed => true,
        QuitMode::Default => cfg!(not(target_os = "macos")),
    }
}
