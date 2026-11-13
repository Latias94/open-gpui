//! Application decisions for the core's original content and native close plans.

use std::rc::Rc;

use anyhow::{Result, anyhow};
use dockspace::runtime::{DockspaceClosePlan, HostFrameReport, HostInputOutcome};
use open_gpui::App;

use super::{DockSurface, HostState, SharedHost, host_native};

pub(super) type CloseHandler = dyn Fn(&DockSurface, DockspaceClosePlan, &mut App);

impl DockSurface {
    /// Installs the application's content decisions for committed close plans.
    ///
    /// This receives both tab/root requests and native surface plans. Native
    /// allow/veto/defer policy is a separate first step. A native allow does not
    /// decide whether the application's pane content may close.
    ///
    /// The handler runs outside the host borrow after unaccepted candidates
    /// release their update lease. Resolve the plan's exact pending tokens in
    /// [`Self::update`], or retain the plan in application confirmation state.
    /// The supplied surface avoids capturing a strong clone in its own handler.
    /// Register before opening windows; installing later also offers existing
    /// content plans, but cannot recover a previously unobserved native plan.
    /// Repeated requests may offer the same plan; the core validates every token.
    pub fn set_close_handler(
        &self,
        handler: impl Fn(&DockSurface, DockspaceClosePlan, &mut App) + 'static,
        cx: &mut App,
    ) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow!("docking host is busy"))?;
        host.close_handler = Some(Rc::new(handler));
        let pending: Vec<_> = host.session.pending_content_close_plans().collect();
        for plan in pending {
            enqueue(&mut host, plan);
        }
        drop(host);
        host_native::schedule(&self.host, cx);
        Ok(())
    }

    /// Copies unresolved content plans without granting new decision authority.
    ///
    /// Native surface plans arrive through [`Self::set_close_handler`] instead.
    pub fn pending_content_close_plans(&self) -> Result<Vec<DockspaceClosePlan>> {
        let host = self
            .host
            .try_borrow()
            .map_err(|_| anyhow!("docking host is busy"))?;
        Ok(host.session.pending_content_close_plans().collect())
    }
}

pub(super) fn observe(host: &mut HostState, report: &HostFrameReport) {
    if host.close_handler.is_none() {
        return;
    }
    for input in report.inputs() {
        match input {
            HostInputOutcome::CloseRequested { plan, .. }
            | HostInputOutcome::NativeSurfaceCloseRequested { plan, .. } => {
                enqueue(host, plan.clone());
            }
            _ => {}
        }
    }
}

fn enqueue(host: &mut HostState, plan: DockspaceClosePlan) {
    if !host
        .close_requests
        .iter()
        .any(|queued| queued.request() == plan.request())
    {
        host.close_requests.push_back(plan);
    }
}

pub(super) fn ready(host: &HostState) -> bool {
    host.close_handler.is_some()
        && !host.close_requests.is_empty()
        && !host.has_unaccepted_update()
        && !host.session.has_pending_native_input()
        && host
            .native
            .as_ref()
            .is_none_or(|native| native.can_dispatch_close() && native.transport.is_empty())
}

pub(super) fn dispatch(shared: &SharedHost, cx: &mut App) {
    let count = {
        let Ok(mut host) = shared.try_borrow_mut() else {
            return;
        };
        if host
            .native
            .as_ref()
            .is_some_and(|native| native.transport.is_frozen())
        {
            host.close_requests.clear();
            return;
        }
        if host.close_dispatching || !ready(&host) {
            return;
        }
        host.close_dispatching = true;
        host.close_requests.len()
    };
    let _guard = DispatchGuard(shared.clone());
    for _ in 0..count {
        let (handler, plan) = {
            let Ok(mut host) = shared.try_borrow_mut() else {
                return;
            };
            if !ready(&host) {
                return;
            }
            let handler = host.close_handler.as_ref().unwrap().clone();
            let plan = host.close_requests.pop_front().unwrap();
            (handler, plan)
        };
        handler(
            &DockSurface {
                host: shared.clone(),
            },
            plan,
            cx,
        );
    }
    drop(_guard);
    if ready(&shared.borrow()) {
        host_native::schedule(shared, cx);
    }
}

struct DispatchGuard(SharedHost);

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        self.0.borrow_mut().close_dispatching = false;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use dockspace::{
        close::CloseDecision,
        model::{
            DockAnchor, DockPlacement, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
            DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    };
    use open_gpui::{AppContext as _, Context, IntoElement, Render, TestAppContext, Window, div};

    use super::*;
    use crate::adapter::{DockPane, GpuiDockspaceConfig};

    struct View;

    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn workspace(cx: &mut App) -> DockSurface {
        let layout = DockspaceLayout::new([1, 2].map(|id| {
            DockspaceSurfaceLayout::new(
                SurfaceId::new(id),
                DockspaceRootLayout::new(
                    RootId::new(id),
                    DockspaceNode::central_tabs([ItemId::new(id), ItemId::new(id + 2)]),
                ),
            )
        }))
        .unwrap();
        let panes = (1..=4)
            .map(|id| DockPane::new(ItemId::new(id), "Retained pane", cx.new(|_| View)))
            .collect::<Vec<_>>();
        DockSurface::new(
            DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap(),
            panes,
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap()
    }

    fn decide(
        dock: &DockSurface,
        plan: &DockspaceClosePlan,
        decision: CloseDecision,
        cx: &mut App,
    ) {
        dock.update(
            |frame| {
                let items = frame
                    .pending_close_decisions(plan.request())
                    .collect::<Vec<_>>();
                for item in items {
                    frame.resolve_close(plan.request(), item.token(), decision)?;
                }
                Ok(())
            },
            cx,
        )
        .unwrap();
    }

    #[open_gpui::test]
    fn installing_a_handler_offers_pending_content_and_retains_the_original_entity(
        cx: &mut TestAppContext,
    ) {
        let dock = cx.update(workspace);
        let first = ItemId::new(1);
        let entity = dock.host.borrow().panels[&first].view.entity_id();
        cx.update(|cx| {
            dock.update(
                |frame| {
                    frame.request_close_item_current(first)?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        assert_eq!(dock.pending_content_close_plans().unwrap().len(), 1);
        assert!(dock.with_view(|view| view.item(first).is_some()));

        cx.update(|cx| {
            dock.set_close_handler(
                |dock, plan, cx| decide(dock, &plan, CloseDecision::Veto, cx),
                cx,
            )
            .unwrap();
        });
        cx.run_until_parked();
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
        assert!(dock.with_view(|view| view.item(first).is_some()));

        cx.update(|cx| {
            dock.set_close_handler(
                |dock, plan, cx| decide(dock, &plan, CloseDecision::Allow, cx),
                cx,
            )
            .unwrap();
            dock.update(
                |frame| {
                    frame.request_close_item_current(first)?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        cx.run_until_parked();
        assert!(dock.with_view(|view| view.item(first).is_none()));
        assert_eq!(dock.host.borrow().panels[&first].view.entity_id(), entity);
        cx.update(|cx| {
            dock.update(
                |frame| {
                    frame.open_item_current(
                        first,
                        DockPlacement::Center(DockAnchor::Item(ItemId::new(3))),
                    )?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        assert!(dock.with_view(|view| view.item(first).is_some()));
        assert_eq!(dock.host.borrow().panels[&first].view.entity_id(), entity);
    }

    #[open_gpui::test]
    fn close_callbacks_can_update_without_reentering_the_next_callback(cx: &mut TestAppContext) {
        let dock = cx.update(workspace);
        let depth = Rc::new(Cell::new(false));
        let calls = Rc::new(Cell::new(0));
        cx.update(|cx| {
            let depth = depth.clone();
            let calls = calls.clone();
            dock.set_close_handler(
                move |dock, plan, cx| {
                    assert!(!depth.replace(true), "close callbacks must not reenter");
                    calls.set(calls.get() + 1);
                    decide(dock, &plan, CloseDecision::Allow, cx);
                    depth.set(false);
                },
                cx,
            )
            .unwrap();
            dock.update(
                |frame| {
                    frame.request_close_item_current(ItemId::new(1))?;
                    frame.request_close_item_current(ItemId::new(2))?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        cx.run_until_parked();
        assert_eq!(calls.get(), 2);
        assert!(!depth.get());
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
        assert!(dock.with_view(|view| view.item(ItemId::new(1)).is_none()));
        assert!(dock.with_view(|view| view.item(ItemId::new(2)).is_none()));
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }

    #[open_gpui::test]
    fn a_followup_close_requested_by_a_handler_receives_its_own_wake(cx: &mut TestAppContext) {
        let dock = cx.update(workspace);
        let calls = Rc::new(Cell::new(0));
        cx.update(|cx| {
            let calls = calls.clone();
            dock.set_close_handler(
                move |dock, plan, cx| {
                    let first = calls.get() == 0;
                    calls.set(calls.get() + 1);
                    dock.update(
                        |frame| {
                            let items = frame
                                .pending_close_decisions(plan.request())
                                .collect::<Vec<_>>();
                            for item in items {
                                frame.resolve_close(
                                    plan.request(),
                                    item.token(),
                                    CloseDecision::Allow,
                                )?;
                            }
                            if first {
                                frame.request_close_item_current(ItemId::new(2))?;
                            }
                            Ok(())
                        },
                        cx,
                    )
                    .unwrap();
                },
                cx,
            )
            .unwrap();
            dock.update(
                |frame| {
                    frame.request_close_item_current(ItemId::new(1))?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        cx.run_until_parked();
        assert_eq!(calls.get(), 2);
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
        assert!(dock.with_view(|view| view.item(ItemId::new(2)).is_none()));
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }
}
