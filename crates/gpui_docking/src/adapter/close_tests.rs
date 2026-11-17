//! Atomic tabs closure through the GPUI facade and application callbacks.

use std::{cell::RefCell, rc::Rc};

use dockspace::{
    close::{CloseDecision, ClosePlanTarget, DeferredCloseDecision, DeferredCloseToken},
    model::{
        DockAnchor, DockPlacement, DockspaceAxis, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::{CloseCapability, DockPolicy},
    runtime::{
        DockspaceCloseInertReason, DockspaceCloseOutcome, DockspaceClosePlan,
        DockspaceCloseResolution, DockspaceEventKind, DockspaceRuntimeError,
        DockspaceRuntimeErrorKind, DockspaceSession, HostCloseRequestOrigin, HostInputOutcome,
    },
};
use open_gpui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, Window, div,
};

use crate::adapter::{DockCommitNotification, DockPane, DockSurface, GpuiDockspaceConfig};

const SURFACE: SurfaceId = SurfaceId::new(1);
const ROOT: RootId = RootId::new(1);
const ITEMS: [ItemId; 4] = [
    ItemId::new(1),
    ItemId::new(2),
    ItemId::new(3),
    ItemId::new(4),
];

struct Content {
    edits: usize,
}

impl Render for Content {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct Fixture {
    dock: DockSurface,
    views: [Entity<Content>; 4],
    plans: Rc<RefCell<Vec<DockspaceClosePlan>>>,
    notifications: Rc<RefCell<Vec<DockCommitNotification>>>,
}

impl Fixture {
    fn new(cx: &mut App) -> Self {
        let content = DockspaceNode::equal_split(
            DockspaceAxis::Horizontal,
            [
                DockspaceNode::tabs([ITEMS[0], ITEMS[1]]),
                DockspaceNode::tabs([ITEMS[2], ITEMS[3]]),
            ],
        )
        .unwrap();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(ROOT, content),
        )])
        .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_close_capability(CloseCapability::DeferredAllowed);
        let views = std::array::from_fn(|index| cx.new(|_| Content { edits: index + 1 }));
        let panes = ITEMS
            .into_iter()
            .zip(&views)
            .map(|(item, view)| DockPane::new(item, "Edited pane", view.clone()));
        let dock = DockSurface::new(
            DockspaceSession::from_layout(layout, policy).unwrap(),
            panes,
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap();
        let plans = Rc::new(RefCell::new(Vec::new()));
        let received = plans.clone();
        dock.set_close_handler(move |_, plan, _| received.borrow_mut().push(plan), cx)
            .unwrap();
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
        Self {
            dock,
            views,
            plans,
            notifications,
        }
    }

    fn assert_views(&self, cx: &mut TestAppContext) {
        cx.update(|cx| {
            let host = self.dock.host.borrow();
            assert_eq!(host.panels.len(), ITEMS.len());
            for (index, (item, view)) in ITEMS.into_iter().zip(&self.views).enumerate() {
                assert_eq!(host.panels[&item].view.entity_id(), view.entity_id());
                assert_eq!(view.read(cx).edits, index + 1);
            }
        });
        assert!(self.dock.failure().is_none(), "{:?}", self.dock.failure());
    }

    fn events(&self) -> Vec<DockspaceEventKind> {
        self.notifications
            .borrow()
            .iter()
            .flat_map(|notification| {
                notification
                    .notification()
                    .events()
                    .iter()
                    .map(|event| event.kind().clone())
            })
            .collect()
    }
}

fn assert_open(dock: &DockSurface, expected: [bool; 4]) {
    assert_eq!(
        dock.with_view(|view| ITEMS.map(|item| view.item(item).is_some())),
        expected
    );
}

#[derive(Debug, PartialEq, Eq)]
struct PaneState {
    tabs: Vec<ItemId>,
    selected: Option<ItemId>,
}

fn pane_state(dock: &DockSurface) -> [Option<PaneState>; 4] {
    dock.with_view(|view| {
        ITEMS.map(|item| {
            view.item(item).map(|location| {
                assert_eq!(location.surface(), SURFACE);
                assert_eq!(location.root(), ROOT);
                assert!(location.contained().is_none());
                assert!(location.is_visible());
                PaneState {
                    tabs: location.tabs().items().to_vec(),
                    selected: location.tabs().selected(),
                }
            })
        })
    })
}

fn sibling_locations(dock: &DockSurface) -> [(SurfaceId, RootId, usize, bool); 2] {
    dock.with_view(|view| {
        [ITEMS[2], ITEMS[3]].map(|item| {
            let location = view.item(item).unwrap();
            assert_eq!(location.tabs().items().len(), 2);
            assert!(location.contained().is_none());
            assert!(location.is_visible());
            (
                location.surface(),
                location.root(),
                location.tab_index(),
                location.is_selected(),
            )
        })
    })
}

fn request_group(fixture: &Fixture, cx: &mut TestAppContext) -> DockspaceClosePlan {
    let calls = fixture.plans.borrow().len();
    let group = fixture
        .dock
        .capture_tabs_group(DockAnchor::Item(ITEMS[0]))
        .unwrap();
    let plan = cx
        .update(|cx| {
            fixture.dock.update_action(
                |frame| Ok(frame.request_close_tabs_current(group)?),
                |outcome| match outcome {
                    Some(HostInputOutcome::CloseRequested {
                        plan,
                        reused: false,
                        origin: HostCloseRequestOrigin::Application,
                    }) => plan.clone(),
                    outcome => panic!("expected the exact tabs close request, got {outcome:?}"),
                },
                cx,
            )
        })
        .unwrap();
    assert_eq!(
        plan.target(),
        ClosePlanTarget::Tabs {
            root: ROOT,
            anchor: ITEMS[0],
        }
    );
    assert_eq!(
        plan.items()
            .iter()
            .map(|item| item.item())
            .collect::<Vec<_>>(),
        ITEMS[..2]
    );
    assert_eq!(
        fixture.dock.pending_content_close_plans().unwrap(),
        [plan.clone()]
    );
    cx.run_until_parked();
    let received = fixture.plans.borrow();
    assert_eq!(received.len(), calls + 1);
    assert_eq!(received.last(), Some(&plan));
    plan
}

fn close_outcome(
    outcome: Option<&HostInputOutcome>,
) -> (
    DockspaceCloseResolution,
    Option<DockspaceCloseOutcome>,
    bool,
) {
    match outcome {
        Some(HostInputOutcome::CloseDecisionProcessed {
            resolution,
            application,
            changed,
            ..
        }) => (
            *resolution,
            application.clone().transpose().unwrap(),
            *changed,
        ),
        outcome => panic!("expected the exact close decision result, got {outcome:?}"),
    }
}

fn resolve(
    dock: &DockSurface,
    plan: &DockspaceClosePlan,
    index: usize,
    decision: CloseDecision,
    cx: &mut TestAppContext,
) -> (
    DockspaceCloseResolution,
    Option<DockspaceCloseOutcome>,
    bool,
) {
    cx.update(|cx| {
        dock.update_action(
            |frame| {
                Ok(frame.resolve_close(plan.request(), plan.items()[index].token(), decision)?)
            },
            close_outcome,
            cx,
        )
    })
    .unwrap()
}

fn continue_close(
    dock: &DockSurface,
    plan: &DockspaceClosePlan,
    continuation: DeferredCloseToken,
    cx: &mut TestAppContext,
) -> (
    DockspaceCloseResolution,
    Option<DockspaceCloseOutcome>,
    bool,
) {
    cx.update(|cx| {
        dock.update_action(
            |frame| {
                Ok(frame.continue_deferred_close(
                    plan.request(),
                    continuation,
                    DeferredCloseDecision::Allow,
                )?)
            },
            close_outcome,
            cx,
        )
    })
    .unwrap()
}

fn defer_group(
    fixture: &Fixture,
    cx: &mut TestAppContext,
) -> (DockspaceClosePlan, DeferredCloseToken) {
    let plan = request_group(fixture, cx);
    let (resolution, application, changed) = cx
        .update(|cx| {
            fixture.dock.update_action(
                |frame| {
                    frame.resolve_close(
                        plan.request(),
                        plan.items()[0].token(),
                        CloseDecision::Allow,
                    )?;
                    Ok(frame.resolve_close(
                        plan.request(),
                        plan.items()[1].token(),
                        CloseDecision::Deferred,
                    )?)
                },
                close_outcome,
                cx,
            )
        })
        .unwrap();
    assert_eq!(application, None);
    assert!(!changed);
    let continuation = match resolution {
        DockspaceCloseResolution::Deferred {
            request,
            item,
            continuation,
        } if request == plan.request() && item == ITEMS[1] => continuation,
        resolution => panic!("expected the second pane's deferred token, got {resolution:?}"),
    };
    let pending = fixture.dock.pending_content_close_plans().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request(), plan.request());
    assert_eq!(pending[0].target(), plan.target());
    assert_eq!(pending[0].items()[1].deferred_token(), Some(continuation));
    assert_open(&fixture.dock, [true; 4]);
    cx.run_until_parked();
    assert!(fixture.notifications.borrow().is_empty());
    (plan, continuation)
}

fn tabs_closed() -> DockspaceCloseOutcome {
    DockspaceCloseOutcome::TabsClosed {
        root: ROOT,
        items: ITEMS[..2].to_vec(),
    }
}

fn assert_inert(
    outcome: (
        DockspaceCloseResolution,
        Option<DockspaceCloseOutcome>,
        bool,
    ),
) {
    assert!(matches!(
        outcome,
        (DockspaceCloseResolution::Inert(_), None, false)
    ));
}

#[open_gpui::test]
fn tabs_close_waits_for_every_allow_and_preserves_sibling_views(cx: &mut TestAppContext) {
    let fixture = cx.update(Fixture::new);
    let siblings = sibling_locations(&fixture.dock);
    let before = pane_state(&fixture.dock);
    let plan = request_group(&fixture, cx);
    assert_eq!(pane_state(&fixture.dock), before);
    assert_eq!(
        resolve(&fixture.dock, &plan, 0, CloseDecision::Allow, cx),
        (
            DockspaceCloseResolution::Recorded {
                request: plan.request(),
                item: ITEMS[0],
            },
            None,
            false,
        )
    );
    assert_eq!(pane_state(&fixture.dock), before);
    assert_open(&fixture.dock, [true; 4]);
    assert_eq!(
        resolve(&fixture.dock, &plan, 0, CloseDecision::Allow, cx),
        (
            DockspaceCloseResolution::Inert(DockspaceCloseInertReason::AlreadyResolved),
            None,
            false,
        )
    );
    cx.run_until_parked();
    assert!(fixture.notifications.borrow().is_empty());
    fixture.assert_views(cx);

    assert_eq!(
        resolve(&fixture.dock, &plan, 1, CloseDecision::Allow, cx),
        (
            DockspaceCloseResolution::Approved {
                request: plan.request(),
            },
            Some(tabs_closed()),
            true,
        )
    );
    assert_open(&fixture.dock, [false, false, true, true]);
    assert_eq!(sibling_locations(&fixture.dock), siblings);
    assert!(
        fixture
            .dock
            .pending_content_close_plans()
            .unwrap()
            .is_empty()
    );
    cx.run_until_parked();
    assert_eq!(fixture.notifications.borrow().len(), 1);
    assert!(
        fixture.notifications.borrow()[0]
            .notification()
            .document_changed()
    );
    assert_eq!(
        fixture.events(),
        [DockspaceEventKind::Closed(tabs_closed())]
    );
    fixture.assert_views(cx);

    cx.update(|cx| {
        fixture.dock.update(
            |frame| {
                frame.open_item_current(ITEMS[0], DockPlacement::After(ITEMS[3]))?;
                frame.open_item_current(ITEMS[1], DockPlacement::After(ITEMS[0]))?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    cx.run_until_parked();
    fixture.notifications.borrow_mut().clear();
    let reopened = pane_state(&fixture.dock);
    for index in 0..2 {
        assert_inert(resolve(
            &fixture.dock,
            &plan,
            index,
            CloseDecision::Allow,
            cx,
        ));
    }
    cx.run_until_parked();
    assert_open(&fixture.dock, [true; 4]);
    assert_eq!(pane_state(&fixture.dock), reopened);
    assert!(fixture.notifications.borrow().is_empty());
    assert_eq!(fixture.plans.borrow().len(), 1);
    fixture.assert_views(cx);
}

#[open_gpui::test]
fn any_tabs_close_veto_preserves_all_views(cx: &mut TestAppContext) {
    for vetoed in 0..2 {
        let fixture = cx.update(Fixture::new);
        let before = pane_state(&fixture.dock);
        let plan = request_group(&fixture, cx);
        resolve(&fixture.dock, &plan, 1 - vetoed, CloseDecision::Allow, cx);
        assert_eq!(
            resolve(&fixture.dock, &plan, vetoed, CloseDecision::Veto, cx),
            (
                DockspaceCloseResolution::Vetoed {
                    request: plan.request(),
                    item: ITEMS[vetoed],
                },
                None,
                false,
            )
        );
        for index in 0..2 {
            assert_inert(resolve(
                &fixture.dock,
                &plan,
                index,
                CloseDecision::Allow,
                cx,
            ));
        }
        cx.run_until_parked();
        assert_open(&fixture.dock, [true; 4]);
        assert_eq!(pane_state(&fixture.dock), before);
        assert!(
            fixture
                .dock
                .pending_content_close_plans()
                .unwrap()
                .is_empty()
        );
        assert!(fixture.notifications.borrow().is_empty());
        assert_eq!(fixture.plans.borrow().len(), 1);
        fixture.assert_views(cx);
    }
}

#[open_gpui::test]
fn deferred_tabs_close_preserves_original_roster_after_local_reorder_and_sibling_change(
    cx: &mut TestAppContext,
) {
    let fixture = cx.update(Fixture::new);
    let (plan, continuation) = defer_group(&fixture, cx);
    cx.update(|cx| {
        fixture.dock.update(
            |frame| {
                frame.dock_item_current(ITEMS[1], DockPlacement::Before(ITEMS[0]))?;
                frame.dock_item_current(ITEMS[3], DockPlacement::Before(ITEMS[2]))?;
                frame.select_item_current(ITEMS[3])?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    fixture.dock.with_view(|view| {
        assert_eq!(
            view.item(ITEMS[0]).unwrap().tabs().items(),
            [ITEMS[1], ITEMS[0]]
        );
        assert_eq!(
            view.item(ITEMS[2]).unwrap().tabs().items(),
            [ITEMS[3], ITEMS[2]]
        );
        assert!(view.item(ITEMS[3]).unwrap().is_selected());
    });
    let pending = fixture.dock.pending_content_close_plans().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].target(), plan.target());
    assert_eq!(
        pending[0]
            .items()
            .iter()
            .map(|item| item.item())
            .collect::<Vec<_>>(),
        ITEMS[..2]
    );
    assert_eq!(pending[0].items()[1].deferred_token(), Some(continuation));
    assert_open(&fixture.dock, [true; 4]);
    let siblings = sibling_locations(&fixture.dock);
    cx.run_until_parked();
    fixture.notifications.borrow_mut().clear();

    assert_eq!(
        continue_close(&fixture.dock, &plan, continuation, cx),
        (
            DockspaceCloseResolution::Approved {
                request: plan.request(),
            },
            Some(tabs_closed()),
            true,
        )
    );
    assert_open(&fixture.dock, [false, false, true, true]);
    assert_eq!(sibling_locations(&fixture.dock), siblings);
    assert_inert(continue_close(&fixture.dock, &plan, continuation, cx));
    cx.run_until_parked();
    assert!(
        fixture
            .dock
            .pending_content_close_plans()
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.notifications.borrow().len(), 1);
    assert_eq!(
        fixture.events(),
        [DockspaceEventKind::Closed(tabs_closed())]
    );
    assert_eq!(fixture.plans.borrow().len(), 1);
    fixture.assert_views(cx);
}

#[open_gpui::test]
fn deferred_tabs_close_membership_changes_permanently_revoke_tokens(cx: &mut TestAppContext) {
    for moved in 0..2 {
        let fixture = cx.update(Fixture::new);
        let (plan, continuation) = defer_group(&fixture, cx);
        let restore = if moved == 0 {
            DockPlacement::Before(ITEMS[1])
        } else {
            DockPlacement::After(ITEMS[0])
        };
        for placement in [DockPlacement::After(ITEMS[2]), restore] {
            cx.update(|cx| {
                fixture.dock.update(
                    |frame| {
                        frame.dock_item_current(ITEMS[moved], placement)?;
                        Ok(())
                    },
                    cx,
                )
            })
            .unwrap();
            cx.run_until_parked();
            fixture.notifications.borrow_mut().clear();
            assert!(
                fixture
                    .dock
                    .pending_content_close_plans()
                    .unwrap()
                    .is_empty()
            );
            let before = pane_state(&fixture.dock);
            assert_inert(continue_close(&fixture.dock, &plan, continuation, cx));
            assert_inert(resolve(&fixture.dock, &plan, 1, CloseDecision::Allow, cx));
            cx.run_until_parked();
            assert_open(&fixture.dock, [true; 4]);
            assert_eq!(pane_state(&fixture.dock), before);
            assert!(fixture.notifications.borrow().is_empty());
            fixture.assert_views(cx);
        }
        fixture.dock.with_view(|view| {
            assert_eq!(view.item(ITEMS[0]).unwrap().tabs().items(), &ITEMS[..2]);
            assert_eq!(view.item(ITEMS[2]).unwrap().tabs().items(), &ITEMS[2..]);
        });
    }
}

#[open_gpui::test]
fn layout_reset_invalidates_deferred_tabs_close_in_the_new_epoch(cx: &mut TestAppContext) {
    let fixture = cx.update(Fixture::new);
    let (plan, continuation) = defer_group(&fixture, cx);
    let before = pane_state(&fixture.dock);
    let layout = fixture.dock.with_view(|view| view.to_layout());
    let previous = fixture.dock.host.borrow().session.version();
    let reset = fixture
        .dock
        .host
        .borrow()
        .session
        .prepare_layout_reset(layout)
        .unwrap();
    cx.update(|cx| {
        fixture.dock.update(
            |frame| {
                frame.submit_layout_reset(&reset)?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    assert!(!previous.is_same_epoch(fixture.dock.host.borrow().session.version()));
    assert!(
        fixture
            .dock
            .pending_content_close_plans()
            .unwrap()
            .is_empty()
    );
    cx.run_until_parked();
    assert_eq!(fixture.events(), [DockspaceEventKind::LayoutReset]);
    fixture.notifications.borrow_mut().clear();

    assert_inert(continue_close(&fixture.dock, &plan, continuation, cx));
    for index in 0..2 {
        assert_inert(resolve(
            &fixture.dock,
            &plan,
            index,
            CloseDecision::Allow,
            cx,
        ));
    }
    cx.run_until_parked();
    assert_open(&fixture.dock, [true; 4]);
    assert_eq!(pane_state(&fixture.dock), before);
    assert!(fixture.notifications.borrow().is_empty());
    fixture.assert_views(cx);
}

#[open_gpui::test]
fn foreign_tabs_close_captures_and_tokens_cannot_close_another_session(cx: &mut TestAppContext) {
    let source = cx.update(Fixture::new);
    let target = cx.update(Fixture::new);
    let (source_plan, source_continuation) = defer_group(&source, cx);
    let (target_plan, target_continuation) = defer_group(&target, cx);
    assert_ne!(source_plan.request(), target_plan.request());
    let group = source
        .dock
        .capture_tabs_group(DockAnchor::Item(ITEMS[0]))
        .unwrap();
    let error = cx
        .update(|cx| {
            target.dock.update_action(
                |frame| Ok(frame.request_close_tabs_current(group)?),
                |_| (),
                cx,
            )
        })
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<DockspaceRuntimeError>()
            .unwrap()
            .kind(),
        DockspaceRuntimeErrorKind::OperationConflict
    );
    assert_eq!(
        resolve(&target.dock, &source_plan, 0, CloseDecision::Allow, cx),
        (
            DockspaceCloseResolution::Inert(DockspaceCloseInertReason::AuthorityMismatch),
            None,
            false,
        )
    );
    assert_eq!(
        continue_close(&target.dock, &source_plan, source_continuation, cx),
        (
            DockspaceCloseResolution::Inert(DockspaceCloseInertReason::AuthorityMismatch),
            None,
            false,
        )
    );
    assert_eq!(
        continue_close(&target.dock, &target_plan, source_continuation, cx),
        (
            DockspaceCloseResolution::Inert(DockspaceCloseInertReason::TokenMismatch),
            None,
            false,
        )
    );
    cx.run_until_parked();
    assert_open(&source.dock, [true; 4]);
    assert_open(&target.dock, [true; 4]);
    assert_eq!(target.dock.pending_content_close_plans().unwrap().len(), 1);
    assert_eq!(source.dock.pending_content_close_plans().unwrap().len(), 1);
    assert!(source.notifications.borrow().is_empty());
    assert!(target.notifications.borrow().is_empty());
    assert_eq!(
        continue_close(&target.dock, &target_plan, target_continuation, cx),
        (
            DockspaceCloseResolution::Approved {
                request: target_plan.request(),
            },
            Some(tabs_closed()),
            true,
        )
    );
    cx.run_until_parked();
    assert_open(&source.dock, [true; 4]);
    assert_open(&target.dock, [false, false, true, true]);
    assert_eq!(target.events(), [DockspaceEventKind::Closed(tabs_closed())]);
    source.assert_views(cx);
    target.assert_views(cx);
}

#[open_gpui::test]
fn stale_tabs_group_capture_cannot_follow_its_moved_anchor(cx: &mut TestAppContext) {
    let fixture = cx.update(Fixture::new);
    let captured = fixture.dock.host.borrow().session.version();
    let group = fixture
        .dock
        .capture_tabs_group(DockAnchor::Item(ITEMS[0]))
        .unwrap();
    assert_eq!(group.expected_version(), captured);
    assert_eq!(fixture.dock.host.borrow().session.version(), captured);
    cx.update(|cx| {
        fixture.dock.update(
            |frame| {
                frame.dock_item_current(ITEMS[0], DockPlacement::After(ITEMS[2]))?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    let accepted = fixture.dock.host.borrow().session.version();
    let before = pane_state(&fixture.dock);
    assert_ne!(captured, accepted);
    cx.run_until_parked();
    fixture.notifications.borrow_mut().clear();
    let rejected = cx
        .update(|cx| {
            fixture.dock.update_action(
                |frame| Ok(frame.request_close_tabs_current(group)?),
                |outcome| match outcome {
                    Some(HostInputOutcome::StaleRejected { expected, accepted }) => {
                        (*expected, *accepted)
                    }
                    outcome => {
                        panic!("expected the captured version to be rejected, got {outcome:?}")
                    }
                },
                cx,
            )
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(rejected, (captured, accepted));
    assert_eq!(pane_state(&fixture.dock), before);
    assert_open(&fixture.dock, [true; 4]);
    assert!(
        fixture
            .dock
            .pending_content_close_plans()
            .unwrap()
            .is_empty()
    );
    assert!(fixture.plans.borrow().is_empty());
    assert!(fixture.notifications.borrow().is_empty());
    fixture.assert_views(cx);
}
