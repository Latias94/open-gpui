use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

use crate::{AnyView, AnyWindowHandle, Window, WindowId};

use super::{App, Effect, QuitMode};

pub(super) struct ReservedWindow<'a> {
    app: &'a mut App,
    id: WindowId,
    pending: bool,
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

    pub(super) fn commit(mut self, mut window: Window) -> anyhow::Result<()> {
        if !window.creation_can_commit() {
            anyhow::bail!("window closed before its creation transaction committed");
        }
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

pub(super) fn reserve(app: &mut App) -> ReservedWindow<'_> {
    let id = app.windows.insert(None);
    if let Some(cell) = app.this.upgrade() {
        cell.reserve_native_window(id);
    }
    ReservedWindow {
        app,
        id,
        pending: true,
    }
}

fn commit_reserved(app: &mut App, id: WindowId, window: Window) {
    assert!(
        app.windows.get(id).is_some_and(Option::is_none),
        "window reservation must be current when committed"
    );
    app.window_handles.insert(id, window.handle);
    app.window_mutation_profiles
        .insert(id, window.window_mutation_profile());
    app.windows
        .get_mut(id)
        .expect("reserved window id should still exist")
        .replace(Box::new(window));
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
    app.window_mutation_profiles.remove(&id);
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
}

impl<'a> WindowUpdateStackScope<'a> {
    fn new(app: &'a mut App, id: WindowId) -> Self {
        let previous_stack = app.window_update_stack.clone();
        app.window_update_stack.push(id);
        Self {
            app,
            previous_stack,
        }
    }

    fn run<R>(self, update: impl FnOnce(&mut App) -> R) -> R {
        update(&mut *self.app)
    }
}

impl Drop for WindowUpdateStackScope<'_> {
    fn drop(&mut self) {
        self.app.window_update_stack = std::mem::take(&mut self.previous_stack);
    }
}

pub(super) struct WindowUpdateTransaction<'a> {
    app: &'a mut App,
    id: WindowId,
    root_view: AnyView,
    window: Option<Box<Window>>,
    previous_stack: Option<Vec<WindowId>>,
}

impl<'a> WindowUpdateTransaction<'a> {
    pub(super) fn begin(app: &'a mut App, id: WindowId) -> Option<Self> {
        let root_view = app.windows.get(id)?.as_ref()?.root.clone()?;
        let window = app.windows.get_mut(id)?.take()?;
        let previous_stack = app.window_update_stack.clone();
        app.window_update_stack.push(window.handle.id);
        Some(Self {
            app,
            id,
            root_view,
            window: Some(window),
            previous_stack: Some(previous_stack),
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

pub(super) fn clear(app: &mut App) {
    app.windows.clear();
    app.window_handles.clear();
    app.window_mutation_profiles.clear();
    if let Some(cell) = app.this.upgrade() {
        cell.clear_native_windows();
    }
}

pub(super) fn handles(app: &App) -> Vec<AnyWindowHandle> {
    app.windows
        .keys()
        .flat_map(|window_id| app.window_handles.get(&window_id).copied())
        .collect()
}

pub(super) fn finish_window_update(app: &mut App, id: WindowId, window: Box<Window>) -> Option<()> {
    if window.removed {
        unregister_removed_window(app, id);
    } else {
        app.windows.get_mut(id)?.replace(window);
    }

    Some(())
}

fn unregister_removed_window(app: &mut App, id: WindowId) {
    app.window_handles.remove(&id);
    app.window_mutation_profiles.remove(&id);
    app.windows.remove(id);
    if let Some(cell) = app.this.upgrade() {
        cell.remove_native_window(id);
    }
    cleanup_entity_window_links(app, id);
    let mut first_panic = notify_window_closed(app, id);

    if should_quit_after_last_window(app.quit_mode) && app.windows.is_empty() {
        retain_first_panic(
            &mut first_panic,
            catch_unwind(AssertUnwindSafe(|| app.quit())).err(),
        );
    }

    if let Some(payload) = first_panic {
        resume_unwind(payload);
    }
}

fn cleanup_entity_window_links(app: &mut App, id: WindowId) {
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
    let mut first_panic = None;
    app.window_closed_observers.clone().retain(&(), |callback| {
        retain_first_panic(
            &mut first_panic,
            catch_unwind(AssertUnwindSafe(|| callback(app, id))).err(),
        );
        true
    });
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
