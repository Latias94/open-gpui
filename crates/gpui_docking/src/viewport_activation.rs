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
    let focus_request = activation.focus_request().clone();
    let host_changed = Rc::new(Cell::new(false));
    let host_changed_flag = host_changed.clone();
    let _ = activation.window().update(cx, move |view, window, cx| {
        window.activate_window();
        if let Ok(host) = view.downcast::<DockHost>() {
            host.update(cx, |host, cx| {
                if host.space() == &activation_space {
                    let changed = host.request_viewport_focus(focus_request.clone());
                    host_changed_flag.set(changed);
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
    use crate::{
        DockViewportFocusRequest,
        host_test_support::{open_host, space, tabs_graph},
    };
    use open_gpui::{TestAppContext, px, size};

    #[open_gpui::test]
    fn activation_without_focus_item_requests_last_focused_restore(cx: &mut TestAppContext) {
        let (graph, _) = tabs_graph(&["a"]);
        let (window, host, _visual) = open_host(
            cx,
            graph,
            &[("a", "Panel A", "A")],
            size(px(320.0), px(240.0)),
        );
        let activation = DockViewportActivationTarget::new(
            space(),
            window,
            DockViewportFocusRequest::restore_last_focused(),
        );

        let (changed, pending_request) = cx.update(|app| {
            let changed = apply_viewport_activation(Some(activation), app);
            let pending_request =
                app.read_entity(&host, |host, _| host.pending_focus_request().cloned());
            (changed, pending_request)
        });

        assert!(changed);
        assert_eq!(
            pending_request,
            Some(DockViewportFocusRequest::restore_last_focused())
        );
    }
}
