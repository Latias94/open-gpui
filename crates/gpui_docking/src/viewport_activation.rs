use crate::{DockHost, DockViewportActivationTarget};
use open_gpui::AppContext;
use std::{cell::Cell, rc::Rc};

/// Applies a viewport activation target to the matching runtime host window.
///
/// Returns `true` when the host state changed and the caller should notify GPUI.
pub(crate) fn apply_viewport_activation<C: AppContext>(
    activation: Option<DockViewportActivationTarget>,
    cx: &mut C,
) -> bool {
    let Some(activation) = activation else {
        return false;
    };

    let activation_space = activation.space().clone();
    let focus_item = activation.focus_item().cloned();
    let host_changed = Rc::new(Cell::new(false));
    let host_changed_flag = host_changed.clone();
    let _ = activation.window().update(cx, move |view, window, cx| {
        window.activate_window();
        if let Ok(host) = view.downcast::<DockHost>() {
            host.update(cx, |host, cx| {
                if host.space() == &activation_space {
                    host_changed_flag.set(true);
                    if let Some(focus_item) = focus_item.clone() {
                        host.clear_viewport_focus_restore_pending();
                        let _ = host.request_panel_focus(focus_item);
                    } else {
                        host.set_viewport_focus_restore_pending(true);
                    }
                    if host_changed_flag.get() {
                        cx.notify();
                    }
                }
            });
        }
    });

    host_changed.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_test_support::{open_host, space, tabs_graph};
    use open_gpui::{TestAppContext, px, size};

    #[open_gpui::test]
    fn activation_without_focus_item_sets_restore_pending(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTarget::new(space(), window, None);

        let (changed, restore_pending) = cx.update(|app| {
            let changed = apply_viewport_activation(Some(activation), app);
            let restore_pending =
                app.read_entity(&host, |host, _| host.viewport_focus_restore_pending());
            (changed, restore_pending)
        });

        assert!(changed);
        assert!(restore_pending);
    }
}
