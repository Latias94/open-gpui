use crate::{DockSpaceId, DockViewportAdapter};
use open_gpui::{AnyWindowHandle, WindowId};

/// Default behavior for a platform viewport close request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DockViewportClosePolicy {
    /// Unregister the runtime window and keep the logical dock layout available for reopen.
    #[default]
    RetainLayout,
    /// Reject the close request and leave the runtime mapping intact.
    ///
    /// This policy prevents platform closes only when viewports are opened through
    /// [`crate::DockViewportRuntime`] or [`crate::DockViewportRuntimeHandle`], which install GPUI
    /// should-close hooks. Adapter-level cleanup methods run after the platform close decision has
    /// already happened, so vetoes are reported only by should-close outcomes.
    Prevent,
}

/// Runtime result of closing a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportCloseOutcome {
    /// Logical dock space that was associated with the closed window, when known.
    pub space: Option<DockSpaceId>,
    /// GPUI window id received from the close callback.
    pub window_id: WindowId,
    /// How the close request resolved.
    pub status: DockViewportCloseStatus,
}

/// How a close request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportCloseStatus {
    /// The window closed and its runtime mapping was removed.
    Closed,
    /// The runtime did not know the closed window id.
    UnknownWindow,
}

/// Runtime result of a platform should-close query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportShouldCloseOutcome {
    /// Logical dock space associated with the queried window, when known.
    pub space: Option<DockSpaceId>,
    /// GPUI window id received from the should-close callback.
    pub window_id: WindowId,
    /// Whether the close should be allowed, vetoed, or ignored as unknown.
    pub status: DockViewportShouldCloseStatus,
}

impl DockViewportShouldCloseOutcome {
    /// Returns true when GPUI should continue closing the platform window.
    pub fn allows_close(&self) -> bool {
        !matches!(self.status, DockViewportShouldCloseStatus::Vetoed)
    }
}

/// How a should-close query resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportShouldCloseStatus {
    /// Runtime policy allows the platform close to continue.
    Allowed,
    /// Runtime policy rejects the platform close before the window closes.
    Vetoed,
    /// Runtime does not know this window id, so docking should not block GPUI.
    UnknownWindow,
}

/// Runtime result of unregistering a platform viewport mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportUnregisterOutcome {
    /// Logical dock space removed from the adapter mapping.
    pub space: DockSpaceId,
    /// GPUI window removed from the adapter mapping.
    pub window: AnyWindowHandle,
    /// Why the mapping was removed.
    pub reason: DockViewportUnregisterReason,
}

/// Reason a platform viewport mapping was removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportUnregisterReason {
    /// The platform window closed.
    Closed,
    /// A new window replaced the previous mapping.
    Replaced,
    /// The application discarded runtime placement for the space.
    Discarded,
}

impl DockViewportAdapter {
    /// Applies viewport close policy before a GPUI platform window closes.
    ///
    /// Unknown windows are allowed to close because docking has no mapping to protect.
    pub fn should_close_viewport(
        &self,
        window_id: WindowId,
        policy: DockViewportClosePolicy,
    ) -> DockViewportShouldCloseOutcome {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return DockViewportShouldCloseOutcome {
                space: None,
                window_id,
                status: DockViewportShouldCloseStatus::UnknownWindow,
            };
        };

        let status = match policy {
            DockViewportClosePolicy::RetainLayout => DockViewportShouldCloseStatus::Allowed,
            DockViewportClosePolicy::Prevent => DockViewportShouldCloseStatus::Vetoed,
        };
        DockViewportShouldCloseOutcome {
            space: Some(space),
            window_id,
            status,
        }
    }

    /// Removes a viewport by GPUI window id and returns a lifecycle outcome.
    ///
    /// This is the cleanup path for close callbacks that report only [`WindowId`].
    pub fn unregister_window_id(
        &mut self,
        window_id: WindowId,
        reason: DockViewportUnregisterReason,
    ) -> Option<DockViewportUnregisterOutcome> {
        let (space, snapshot) = self.unregister_window_id_snapshot(window_id)?;
        Some(DockViewportUnregisterOutcome {
            space,
            window: snapshot.window,
            reason,
        })
    }

    /// Handles an already-accepted GPUI window close by removing runtime mapping.
    pub fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        if let Some(outcome) =
            self.unregister_window_id(window_id, DockViewportUnregisterReason::Closed)
        {
            DockViewportCloseOutcome {
                space: Some(outcome.space),
                window_id,
                status: DockViewportCloseStatus::Closed,
            }
        } else {
            DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockHost, DockItemId, DockNode, DockViewportAdapter, DockViewportOpenOutcome,
        DockViewportOpenStatus,
    };
    use open_gpui::{AnyWindowHandle, WindowHandle};

    fn space(id: &str) -> DockSpaceId {
        DockSpaceId::from(id)
    }

    fn handle(id: u64) -> AnyWindowHandle {
        WindowHandle::<DockHost>::new(WindowId::from(id)).into()
    }

    #[test]
    fn unregistering_by_window_id_clears_close_callback_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        adapter.register_viewport(main.clone(), first);
        adapter.register_viewport(secondary.clone(), second);

        let removed = adapter
            .unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed)
            .expect("window id should be registered");
        assert_eq!(removed.space, main);
        assert_eq!(removed.window, first);
        assert_eq!(removed.reason, DockViewportUnregisterReason::Closed);
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);
        assert_eq!(adapter.window_for_space(&removed.space), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(second));

        assert_eq!(
            adapter.unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed),
            None
        );
    }

    #[test]
    fn window_closed_cleanup_removes_only_runtime_mapping() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let outcome = adapter.handle_window_closed(window.window_id());
        assert_eq!(
            outcome,
            DockViewportCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportCloseStatus::Closed,
            }
        );
        assert!(adapter.is_empty());
        assert!(
            graph.root(&main).is_some(),
            "runtime cleanup must not mutate the logical docking graph"
        );

        let reopened = handle(2);
        adapter.register_viewport(main.clone(), reopened);
        assert_eq!(adapter.window_for_space(&main), Some(reopened));
        assert_eq!(
            adapter.space_for_window_id(reopened.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn window_closed_unknown_window_reports_unknown() {
        let mut adapter = DockViewportAdapter::new();
        let unknown = WindowId::from(99);

        assert_eq!(
            adapter.handle_window_closed(unknown),
            DockViewportCloseOutcome {
                space: None,
                window_id: unknown,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
    }

    #[test]
    fn window_closed_discards_stale_window_index() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window_id = WindowId::from(1);
        adapter.insert_stale_window_index_for_test(window_id, main);

        assert_eq!(
            adapter.handle_window_closed(window_id),
            DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
        assert_eq!(adapter.space_for_window_id(window_id), None);
        assert!(adapter.is_empty());
    }

    #[test]
    fn viewport_lifecycle_types_preserve_runtime_boundaries() {
        let main = space("main");
        let window = handle(7);
        let open = DockViewportOpenOutcome {
            space: main.clone(),
            window,
            status: DockViewportOpenStatus::Opened,
        };
        assert_eq!(open.space, main.clone());
        assert_eq!(open.window, window);
        assert_eq!(open.status, DockViewportOpenStatus::Opened);
        assert_eq!(
            DockViewportClosePolicy::default(),
            DockViewportClosePolicy::RetainLayout
        );

        let close = DockViewportCloseOutcome {
            space: Some(main.clone()),
            window_id: window.window_id(),
            status: DockViewportCloseStatus::Closed,
        };
        assert_eq!(close.space, Some(main.clone()));
        assert_eq!(close.window_id, window.window_id());
        assert_eq!(close.status, DockViewportCloseStatus::Closed);

        let unregister = DockViewportUnregisterOutcome {
            space: main,
            window,
            reason: DockViewportUnregisterReason::Closed,
        };
        assert_eq!(unregister.reason, DockViewportUnregisterReason::Closed);
    }

    #[test]
    fn should_close_policy_reports_pre_close_veto_without_mutating_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let allowed = adapter
            .should_close_viewport(window.window_id(), DockViewportClosePolicy::RetainLayout);
        assert_eq!(
            allowed,
            DockViewportShouldCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportShouldCloseStatus::Allowed,
            }
        );
        assert!(allowed.allows_close());
        assert_eq!(adapter.window_for_space(&main), Some(window));

        let vetoed =
            adapter.should_close_viewport(window.window_id(), DockViewportClosePolicy::Prevent);
        assert_eq!(
            vetoed,
            DockViewportShouldCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportShouldCloseStatus::Vetoed,
            }
        );
        assert!(!vetoed.allows_close());
        assert_eq!(adapter.window_for_space(&main), Some(window));

        let unknown =
            adapter.should_close_viewport(WindowId::from(99), DockViewportClosePolicy::Prevent);
        assert_eq!(unknown.status, DockViewportShouldCloseStatus::UnknownWindow);
        assert!(unknown.allows_close());
    }
}
