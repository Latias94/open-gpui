//! Application-visible action results and deferred commit observations.

use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use dockspace::{
    close::CloseDecision,
    geometry::PhysicalRect,
    model::{
        DockAnchor, DockPlacement, DockspaceActionOutcome, DockspaceActionRejection,
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        NativeWindowPlacement, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceCloseOutcome, DockspaceCloseResolution, DockspaceEventKind,
        DockspacePresentationTransitionResult, DockspaceSession, HostInputOutcome,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Empty, PlatformWindowPresentOutcome,
    RequestFrameOptions, TestAppContext, VisualTestContext, WindowMutationDomain, WindowOptions,
};

use super::DockCommitNotification;
use crate::adapter::{DockPane, DockSurface, GpuiDockspaceConfig};

const SOURCE: SurfaceId = SurfaceId::new(1);
const TARGET: SurfaceId = SurfaceId::new(2);
const SOURCE_ROOT: RootId = RootId::new(1);
const FIRST: ItemId = ItemId::new(1);
const SECOND: ItemId = ItemId::new(2);
const TARGET_ITEM: ItemId = ItemId::new(3);

fn session(policy: DockPolicy) -> DockspaceSession {
    let layout = DockspaceLayout::new([
        DockspaceSurfaceLayout::new(
            SOURCE,
            DockspaceRootLayout::new(SOURCE_ROOT, DockspaceNode::tabs([FIRST, SECOND])),
        ),
        DockspaceSurfaceLayout::new(
            TARGET,
            DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([TARGET_ITEM])),
        ),
    ])
    .unwrap();
    DockspaceSession::from_layout(layout, policy).unwrap()
}

fn workspace(session: DockspaceSession, managed: bool, cx: &mut App) -> DockSurface {
    let panes = [FIRST, SECOND, TARGET_ITEM]
        .map(|item| DockPane::new(item, "Application pane", cx.new(|_| Empty)));
    if managed {
        DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    } else {
        DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    }
}

fn observe(dock: &DockSurface, cx: &mut App) -> Rc<RefCell<Vec<DockCommitNotification>>> {
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let observed = notifications.clone();
    dock.set_commit_handler(
        move |dock, notification, _| {
            assert!(dock.inspection().is_ok());
            observed.borrow_mut().push(notification);
        },
        cx,
    )
    .unwrap();
    notifications
}

fn selected(notification: &DockCommitNotification) -> Vec<ItemId> {
    notification
        .notification()
        .events()
        .iter()
        .filter_map(|event| match event.kind() {
            DockspaceEventKind::SelectionCommitted { item } => Some(*item),
            _ => None,
        })
        .collect()
}

fn pane_locations(dock: &DockSurface) -> [(SurfaceId, RootId, usize, bool); 3] {
    dock.with_view(|view| {
        [FIRST, SECOND, TARGET_ITEM].map(|item| {
            let item = view.item(item).expect("the original pane remains open");
            assert!(item.contained().is_none());
            assert!(item.is_visible());
            (
                item.surface(),
                item.root(),
                item.tab_index(),
                item.is_selected(),
            )
        })
    })
}

#[open_gpui::test]
fn update_action_returns_the_exact_submitted_outcome(cx: &mut TestAppContext) {
    let dock = cx.update(|cx| workspace(session(DockPolicy::default()), false, cx));
    let outcome = cx
        .update(|cx| {
            dock.update_action(
                |frame| {
                    let submitted = frame.select_item_current(SECOND)?;
                    frame.select_item_current(FIRST)?;
                    Ok(submitted)
                },
                |outcome| match outcome {
                    Some(HostInputOutcome::ProductActionApplied(outcome)) => Some(outcome.clone()),
                    _ => None,
                },
                cx,
            )
        })
        .unwrap();
    assert_eq!(
        outcome,
        Some(DockspaceActionOutcome::Selected {
            item: SECOND,
            changed: true,
        })
    );
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_selected()));
    let original_value = cx
        .update(|cx| {
            dock.update(
                |frame| {
                    frame.select_item_current(SECOND)?;
                    Ok("application result")
                },
                cx,
            )
        })
        .unwrap();
    assert_eq!(original_value, "application result");
    assert!(dock.failure().is_none());
}

#[open_gpui::test]
fn update_action_reports_policy_rejection_without_a_change_notification(cx: &mut TestAppContext) {
    let (dock, notifications) = cx.update(|cx| {
        let mut policy = DockPolicy::default();
        policy.set_allow_tab_merge(false);
        let dock = workspace(session(policy), false, cx);
        let notifications = observe(&dock, cx);
        (dock, notifications)
    });
    let before = pane_locations(&dock);
    let before_version = cx
        .update(|cx| dock.update(|frame| Ok(frame.version()), cx))
        .unwrap();
    let rejection = cx
        .update(|cx| {
            dock.update_action(
                |frame| {
                    Ok(frame.dock_item_current(
                        FIRST,
                        DockPlacement::Center(DockAnchor::Item(TARGET_ITEM)),
                    )?)
                },
                |outcome| match outcome {
                    Some(HostInputOutcome::ProductActionRejected(reason)) => Some(*reason),
                    _ => None,
                },
                cx,
            )
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(rejection, Some(DockspaceActionRejection::PolicyDenied));
    assert_eq!(pane_locations(&dock), before);
    let after_version = cx
        .update(|cx| dock.update(|frame| Ok(frame.version()), cx))
        .unwrap();
    assert_eq!(after_version, before_version);
    assert!(notifications.borrow().is_empty());
    assert!(dock.failure().is_none());
}

#[open_gpui::test]
fn update_action_maps_close_requests_and_their_decision_outcomes(cx: &mut TestAppContext) {
    let (dock, notifications) = cx.update(|cx| {
        let dock = workspace(session(DockPolicy::default()), false, cx);
        let notifications = observe(&dock, cx);
        (dock, notifications)
    });
    let plan = cx
        .update(|cx| {
            dock.update_action(
                |frame| Ok(frame.request_close_item_current(SECOND)?),
                |outcome| match outcome {
                    Some(HostInputOutcome::CloseRequested { plan, .. }) => Some(plan.clone()),
                    _ => None,
                },
                cx,
            )
        })
        .unwrap()
        .expect("the close token must expose its complete request result");
    assert_eq!(plan.items().len(), 1);
    assert_eq!(plan.items()[0].item(), SECOND);
    let request = plan.request();
    let token = plan.items()[0].token();
    let closed = cx
        .update(|cx| {
            dock.update_action(
                |frame| Ok(frame.resolve_close(request, token, CloseDecision::Allow)?),
                |outcome| {
                    matches!(
                        outcome,
                        Some(HostInputOutcome::CloseDecisionProcessed {
                            resolution: DockspaceCloseResolution::Approved { request: approved },
                            application: Some(Ok(DockspaceCloseOutcome::ItemClosed {
                                item: SECOND,
                                root: SOURCE_ROOT,
                            })),
                            changed: true,
                            ..
                        }) if *approved == request
                    )
                },
                cx,
            )
        })
        .unwrap();
    assert!(closed);
    assert!(dock.with_view(|view| view.item(SECOND).is_none()));
    assert!(dock.pending_content_close_plans().unwrap().is_empty());
    cx.run_until_parked();
    assert!(notifications.borrow().iter().any(|notification| {
        notification.notification().events().iter().any(|event| {
            matches!(
                event.kind(),
                DockspaceEventKind::Closed(DockspaceCloseOutcome::ItemClosed {
                    item: SECOND,
                    root: SOURCE_ROOT,
                })
            )
        })
    }));
    assert!(dock.failure().is_none());
}

#[open_gpui::test]
fn update_action_distinguishes_stale_actions_from_old_foreign_and_rolled_back_tokens(
    cx: &mut TestAppContext,
) {
    let (dock, prepared) = cx.update(|cx| {
        let session = session(DockPolicy::default());
        let prepared = session.prepare_select_item(FIRST);
        (workspace(session, false, cx), prepared)
    });
    let old_token = cx
        .update(|cx| dock.update(|frame| Ok(frame.select_item_current(SECOND)?), cx))
        .unwrap();
    let notifications = cx.update(|cx| observe(&dock, cx));
    let stale = cx
        .update(|cx| {
            dock.update_action(
                |frame| Ok(frame.submit_prepared_action(prepared)?),
                |outcome| match outcome {
                    Some(HostInputOutcome::StaleRejected { expected, accepted }) => {
                        Some((*expected, *accepted))
                    }
                    _ => None,
                },
                cx,
            )
        })
        .unwrap();
    assert!(matches!(
        stale,
        Some((expected, accepted)) if expected != accepted
    ));
    cx.run_until_parked();
    assert!(notifications.borrow().is_empty());
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_selected()));

    let foreign = cx.update(|cx| workspace(session(DockPolicy::default()), false, cx));
    let foreign_token = cx
        .update(|cx| foreign.update(|frame| Ok(frame.select_item_current(SECOND)?), cx))
        .unwrap();
    let rolled_back = Cell::new(None);
    cx.update(|cx| {
        dock.update(
            |frame| {
                rolled_back.set(Some(frame.select_item_current(FIRST)?));
                Err::<(), _>(anyhow::anyhow!("application cancels this frame"))
            },
            cx,
        )
    })
    .unwrap_err();
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_selected()));

    for token in [old_token, foreign_token, rolled_back.get().unwrap()] {
        let unmatched = cx
            .update(|cx| {
                dock.update_action(
                    |frame| {
                        frame.select_item_current(FIRST)?;
                        Ok(token)
                    },
                    |outcome| outcome.is_none(),
                    cx,
                )
            })
            .unwrap();
        assert!(unmatched);
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_selected()));
    }
    assert!(dock.failure().is_none());
}

#[open_gpui::test]
fn commit_notifications_preserve_order_and_reentrant_handler_replacement(cx: &mut TestAppContext) {
    let dock = cx.update(|cx| workspace(session(DockPolicy::default()), false, cx));
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let handlers = Rc::new(RefCell::new(Vec::new()));
    let in_callback = Rc::new(Cell::new(false));
    cx.update(|cx| {
        let observed = notifications.clone();
        let observed_handlers = handlers.clone();
        let active = in_callback.clone();
        dock.set_commit_handler(
            move |dock, notification, cx| {
                assert!(!active.replace(true));
                assert!(dock.inspection().is_ok());
                observed.borrow_mut().push(notification);
                observed_handlers.borrow_mut().push("original");
                let observed = observed.clone();
                let observed_handlers = observed_handlers.clone();
                let active_next = active.clone();
                dock.set_commit_handler(
                    move |dock, notification, cx| {
                        assert!(!active_next.replace(true));
                        observed.borrow_mut().push(notification);
                        observed_handlers.borrow_mut().push("replacement");
                        dock.update(|_| Ok(()), cx).unwrap();
                        active_next.set(false);
                    },
                    cx,
                )
                .unwrap();
                dock.update_action(|frame| Ok(frame.select_item_current(SECOND)?), |_| (), cx)
                    .unwrap();
                dock.pump_native(cx);
                active.set(false);
            },
            cx,
        )
        .unwrap();
        for item in [SECOND, FIRST] {
            dock.update_action(|frame| Ok(frame.select_item_current(item)?), |_| (), cx)
                .unwrap();
            assert!(notifications.borrow().is_empty());
        }
    });
    cx.run_until_parked();
    let notifications = notifications.borrow();
    assert_eq!(
        *handlers.borrow(),
        ["original", "replacement", "replacement"]
    );
    assert_eq!(
        notifications.iter().map(selected).collect::<Vec<_>>(),
        [vec![SECOND], vec![FIRST], vec![SECOND]]
    );
    for pair in notifications.windows(2) {
        assert_eq!(
            pair[0].notification().layout_change().unwrap().after(),
            pair[1].notification().layout_change().unwrap().before()
        );
    }
    assert!(dock.failure().is_none());
}

#[open_gpui::test]
fn commit_handler_panic_resumes_pending_notifications_without_another_dock_call(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(|cx| workspace(session(DockPolicy::default()), false, cx));
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let panic = catch_unwind(AssertUnwindSafe(|| {
        cx.update(|cx| {
            let observed = notifications.clone();
            dock.set_commit_handler(
                move |_, notification, _| {
                    observed.borrow_mut().push(notification);
                    if observed.borrow().len() == 1 {
                        panic!("application commit handler failed");
                    }
                },
                cx,
            )
            .unwrap();
            for item in [SECOND, FIRST] {
                dock.update_action(|frame| Ok(frame.select_item_current(item)?), |_| (), cx)
                    .unwrap();
                assert!(notifications.borrow().is_empty());
            }
        });
    }))
    .expect_err("the handler panic must reach its original caller");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"application commit handler failed")
    );
    assert_eq!(
        notifications
            .borrow()
            .iter()
            .map(selected)
            .collect::<Vec<_>>(),
        [vec![SECOND]]
    );

    cx.update(|_| {});
    cx.run_until_parked();
    assert_eq!(
        notifications
            .borrow()
            .iter()
            .map(selected)
            .collect::<Vec<_>>(),
        [vec![SECOND], vec![FIRST]]
    );
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_selected()));
    assert!(dock.failure().is_none());
}

#[open_gpui::test]
fn commit_notifications_keep_every_event_in_one_commit(cx: &mut TestAppContext) {
    let (dock, notifications) = cx.update(|cx| {
        let dock = workspace(session(DockPolicy::default()), false, cx);
        let notifications = observe(&dock, cx);
        (dock, notifications)
    });
    let expected = (0..300)
        .map(|index| if index % 2 == 0 { SECOND } else { FIRST })
        .collect::<Vec<_>>();
    cx.update(|cx| {
        dock.update(
            |frame| {
                for item in &expected {
                    frame.select_item_current(*item)?;
                }
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    cx.run_until_parked();
    let notifications = notifications.borrow();
    assert_eq!(notifications.len(), 1);
    assert_eq!(selected(&notifications[0]), expected);
    assert_eq!(notifications[0].notification().events().len(), 300);
}

#[open_gpui::test]
fn commit_handlers_do_not_replay_unobserved_or_cleared_commits(cx: &mut TestAppContext) {
    let dock = cx.update(|cx| workspace(session(DockPolicy::default()), false, cx));
    cx.update(|cx| dock.update_action(|frame| Ok(frame.select_item_current(SECOND)?), |_| (), cx))
        .unwrap();
    let first = cx.update(|cx| {
        let notifications = observe(&dock, cx);
        dock.update(|_| Ok(()), cx).unwrap();
        notifications
    });
    cx.run_until_parked();
    assert!(first.borrow().is_empty());
    cx.update(|cx| {
        dock.update_action(|frame| Ok(frame.select_item_current(FIRST)?), |_| (), cx)
            .unwrap();
        dock.clear_commit_handler().unwrap();
    });
    cx.run_until_parked();
    assert!(first.borrow().is_empty());
    cx.update(|cx| dock.update_action(|frame| Ok(frame.select_item_current(SECOND)?), |_| (), cx))
        .unwrap();
    let second = cx.update(|cx| observe(&dock, cx));
    cx.run_until_parked();
    assert!(second.borrow().is_empty());
    cx.update(|cx| dock.update_action(|frame| Ok(frame.select_item_current(FIRST)?), |_| (), cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(second.borrow().len(), 1);
    assert_eq!(selected(&second.borrow()[0]), [FIRST]);
}

fn frames(visual: &mut VisualTestContext, dock: &DockSurface) {
    for _ in 0..4 {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.failure().is_none(), "{:?}", dock.failure());
    }
}

fn managed_workspace(cx: &mut TestAppContext) -> (DockSurface, AnyWindowHandle) {
    let dock = cx.update(|cx| {
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        workspace(session(policy), true, cx)
    });
    cx.defer_next_window_frame_requests();
    let target = cx
        .update(|cx| dock.open_native_root(TARGET, true, WindowOptions::default(), cx))
        .unwrap();
    cx.run_until_parked();
    frames(&mut VisualTestContext::from_window(target, cx), &dock);
    (dock, target)
}

fn placement() -> NativeWindowPlacement {
    NativeWindowPlacement::new(PhysicalRect::new(20.0, 30.0, 600.0, 450.0).unwrap())
}

fn admit_native_child(
    dock: &DockSurface,
    target: AnyWindowHandle,
    cx: &mut TestAppContext,
) -> AnyWindowHandle {
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    let source = cx.read(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| *window != target)
            .expect("the committed create must open its original native child")
    });
    assert!(cx.flush_window_mutation(source, WindowMutationDomain::Placement));
    frames(&mut VisualTestContext::from_window(source, cx), dock);
    frames(&mut VisualTestContext::from_window(target, cx), dock);
    source
}

#[open_gpui::test]
fn a_mapper_panic_preserves_native_create_commit_and_drain(cx: &mut TestAppContext) {
    let (dock, target) = managed_workspace(cx);
    cx.update(|cx| {
        let panic = catch_unwind(AssertUnwindSafe(|| {
            dock.update_action(
                |frame| Ok(frame.create_native_surface_current(SOURCE, TARGET, placement())?),
                |outcome| -> () {
                    assert!(matches!(
                        outcome,
                        Some(HostInputOutcome::NativeSurfaceCreateRequested { .. })
                    ));
                    panic!("application mapper failed after commit");
                },
                cx,
            )
        }));
        let panic = panic.expect_err("the mapper panic must reach its original caller");
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"application mapper failed after commit")
        );
        assert!(dock.inspection().is_ok());
    });
    let source = admit_native_child(&dock, target, cx);
    assert!(dock.native_binding_for_window(source.window_id()).is_some());
    assert_eq!(
        dock.with_view(|view| view.item(FIRST).unwrap().surface()),
        SOURCE
    );
    assert!(dock.failure().is_none());
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_later_native_target_failure_notifies_the_original_transition_once(cx: &mut TestAppContext) {
    let (dock, target) = managed_workspace(cx);
    cx.update(|cx| {
        dock.update(
            |frame| Ok(frame.create_native_surface_current(SOURCE, TARGET, placement())?),
            cx,
        )
    })
    .unwrap();
    admit_native_child(&dock, target, cx);
    let before = pane_locations(&dock);
    let before_version = cx
        .update(|cx| dock.update(|frame| Ok(frame.version()), cx))
        .unwrap();
    let notifications = cx.update(|cx| observe(&dock, cx));
    cx.defer_next_window_frame_requests();
    let transition = cx
        .update(|cx| {
            dock.update_action(
                |frame| {
                    Ok(frame.dock_root_current(
                        SOURCE_ROOT,
                        DockPlacement::Center(DockAnchor::Item(TARGET_ITEM)),
                    )?)
                },
                |outcome| match outcome {
                    Some(HostInputOutcome::ProductPresentationActionRequested {
                        transition,
                        ..
                    }) => Some(*transition),
                    _ => None,
                },
                cx,
            )
        })
        .unwrap()
        .expect("the native root move must retain its presentation transition");
    assert!(dock.inspection().unwrap().pending_presentation);
    assert_eq!(pane_locations(&dock), before);
    {
        let mut visual = VisualTestContext::from_window(target, cx);
        visual.set_window_present_outcome(target, PlatformWindowPresentOutcome::Rejected);
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        visual.set_window_present_outcome(target, PlatformWindowPresentOutcome::Submitted);
    }
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    {
        let notifications = notifications.borrow();
        let terminals = notifications
            .iter()
            .flat_map(|notification| notification.presentation_transitions())
            .filter(|terminal| terminal.id() == transition)
            .collect::<Vec<_>>();
        assert_eq!(terminals.len(), 1, "{notifications:?}");
        assert_eq!(
            terminals[0].result(),
            DockspacePresentationTransitionResult::TargetNotPresented
        );
        assert!(terminals[0].outcome().is_none());
        assert_eq!(terminals[0].source_surface(), SOURCE);
        assert_eq!(terminals[0].target_surface(), TARGET);
        assert!(notifications.iter().all(|notification| {
            notification.notification().events().is_empty()
                && notification.notification().layout_change().is_none()
        }));
    }
    assert!(!dock.inspection().unwrap().pending_presentation);
    assert_eq!(pane_locations(&dock), before);
    let after_version = cx
        .update(|cx| dock.update(|frame| Ok(frame.version()), cx))
        .unwrap();
    assert_eq!(after_version, before_version);
    assert!(dock.failure().is_none(), "{:?}", dock.failure());
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}
