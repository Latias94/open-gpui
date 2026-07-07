use crate::{AnyWindowHandle, Window, WindowId};

use super::{App, QuitMode};

pub(super) fn reserve(app: &mut App) -> WindowId {
    app.windows.insert(None)
}

pub(super) fn commit(app: &mut App, id: WindowId, window: Window) {
    app.window_handles.insert(id, window.handle);
    app.windows
        .get_mut(id)
        .expect("reserved window id should still exist")
        .replace(Box::new(window));
}

pub(super) fn rollback_reserved(app: &mut App, id: WindowId) {
    app.windows.remove(id);
}

pub(super) fn clear(app: &mut App) {
    app.windows.clear();
    app.window_handles.clear();
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
    app.windows.remove(id);
    cleanup_entity_window_links(app, id);
    notify_window_closed(app, id);

    if should_quit_after_last_window(app.quit_mode) && app.windows.is_empty() {
        app.quit();
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

fn notify_window_closed(app: &mut App, id: WindowId) {
    app.window_closed_observers.clone().retain(&(), |callback| {
        callback(app, id);
        true
    });
}

fn should_quit_after_last_window(quit_mode: QuitMode) -> bool {
    match quit_mode {
        QuitMode::Explicit => false,
        QuitMode::LastWindowClosed => true,
        QuitMode::Default => cfg!(not(target_os = "macos")),
    }
}
