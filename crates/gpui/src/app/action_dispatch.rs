use open_gpui_core_util::ResultExt;

use crate::{Action, AnyWindowHandle, DispatchPhase, PlatformFocusedWindow};

use super::App;

pub(super) fn is_action_available(app: &mut App, action: &dyn Action) -> bool {
    let mut action_available = false;
    if let Some(window) = focused_action_window(app)
        && let Ok(window_action_available) =
            window.update(app, |_, window, cx| window.is_action_available(action, cx))
    {
        action_available = window_action_available;
    }

    action_available
        || app
            .global_action_listeners
            .contains_key(&action.as_any().type_id())
}

pub(super) fn dispatch_action(app: &mut App, action: &dyn Action) {
    if let Some(focused_window) = focused_action_window(app) {
        focused_window
            .update(app, |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx)
            })
            .log_err();
    } else {
        dispatch_global_action(app, action);
    }
}

fn focused_action_window(app: &App) -> Option<AnyWindowHandle> {
    match app.focused_window() {
        PlatformFocusedWindow::Window(window) => Some(window),
        PlatformFocusedWindow::NoWindow | PlatformFocusedWindow::Unavailable => None,
    }
}

fn dispatch_global_action(app: &mut App, action: &dyn Action) {
    app.propagate_event = true;

    if let Some(mut global_listeners) = app
        .global_action_listeners
        .remove(&action.as_any().type_id())
    {
        for listener in &global_listeners {
            listener(action.as_any(), DispatchPhase::Capture, app);
            if !app.propagate_event {
                break;
            }
        }

        global_listeners.extend(
            app.global_action_listeners
                .remove(&action.as_any().type_id())
                .unwrap_or_default(),
        );

        app.global_action_listeners
            .insert(action.as_any().type_id(), global_listeners);
    }

    if app.propagate_event
        && let Some(mut global_listeners) = app
            .global_action_listeners
            .remove(&action.as_any().type_id())
    {
        for listener in global_listeners.iter().rev() {
            listener(action.as_any(), DispatchPhase::Bubble, app);
            if !app.propagate_event {
                break;
            }
        }

        global_listeners.extend(
            app.global_action_listeners
                .remove(&action.as_any().type_id())
                .unwrap_or_default(),
        );

        app.global_action_listeners
            .insert(action.as_any().type_id(), global_listeners);
    }
}
