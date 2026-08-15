use crate::{
    DockController, DockHost, DockPanel, DockPanelPlacement, DockSurface,
    DockSurfaceActivationOutcome, DockSurfacePrimaryWindowOpenOutcome,
    DockViewportActivationTransaction, DockViewportFocusRequest, DockViewportRuntimeHandle,
    surface::{
        DockSurfaceActivationDispatch, DockSurfaceActivationHostLookup,
        DockSurfaceActivationHostRegistrationStatus, DockSurfaceActivationState,
        window_session::{
            DockSurfaceWindowSession, DockSurfaceWindowSessionBeginShutdownOutcome,
            DockSurfaceWindowSessionLease, DockSurfaceWindowSessionRuntimeEmptyOutcome,
            DockSurfaceWindowSessionShutdownConvergenceOutcome,
            DockSurfaceWindowSessionShutdownReason, DockSurfaceWindowSessionTerminalDisposition,
            DockSurfaceWindowSessionTerminalOutcome,
        },
    },
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, SubtreePresentation,
    SubtreePresentationExt, TestAppContext, Window, WindowActivationPolicy, WindowHandle, WindowId,
    WindowMutationDispatch, WindowMutationDomain, WindowOptions, div, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct FocusPanel {
    focus_handle: FocusHandle,
    presentation: Rc<Cell<SubtreePresentation>>,
}

impl Focusable for FocusPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FocusPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("surface-activation-focus-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .with_subtree_presentation(self.presentation.get())
    }
}

struct EmbeddedHostRoot {
    host: Entity<DockHost>,
    presentation: SubtreePresentation,
}

impl Render for EmbeddedHostRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(AnyView::from(self.host.clone()).with_subtree_presentation(self.presentation))
    }
}

fn focus_panel(cx: &mut App) -> Entity<FocusPanel> {
    focus_panel_with_presentation(Rc::new(Cell::new(SubtreePresentation::Visible)), cx)
}

fn focus_panel_with_presentation(
    presentation: Rc<Cell<SubtreePresentation>>,
    cx: &mut App,
) -> Entity<FocusPanel> {
    cx.new(|cx| FocusPanel {
        focus_handle: cx.focus_handle(),
        presentation,
    })
}

fn fake_window(id: u64) -> AnyWindowHandle {
    WindowHandle::<DockHost>::new(WindowId::from(id)).into()
}

fn active_activation_lease(authority: u64, anchor: u64) -> DockSurfaceWindowSessionLease {
    let mut session = DockSurfaceWindowSession::new(open_gpui::EntityId::from(authority));
    let opening = session
        .reserve_opening()
        .expect("activation test session should reserve an opening");
    session
        .commit_opening(opening, WindowId::from(anchor))
        .expect("activation test session should activate")
}

fn host_entity<C: AppContext>(cx: &mut C, controller: Entity<DockController>) -> Entity<DockHost> {
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    cx.new(|cx| DockHost::from_controller(controller, "main", runtime, cx))
}

fn open_primary_host(surface: &DockSurface, cx: &mut App) -> (AnyWindowHandle, Entity<DockHost>) {
    let opened = match surface.open_primary_window(WindowOptions::default(), cx) {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
        outcome => panic!("managed primary host should open, got {outcome:?}"),
    };
    let window = opened.window();
    let host = window
        .downcast::<DockHost>()
        .expect("managed primary window should retain a DockHost root")
        .entity(cx)
        .expect("managed primary DockHost should remain live");
    (window, host)
}

#[open_gpui::test]
fn surface_activation_without_a_registered_item_settles_unavailable(cx: &mut TestAppContext) {
    let (outcomes, subscription, request_id) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .build(cx)
            .expect("surface should build");
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let observed = outcomes.clone();

        let (request_id, subscription) =
            surface.activate_panel_with_completion("missing", cx, move |outcome, _cx| {
                observed.borrow_mut().push(outcome);
            });

        (outcomes, subscription, request_id)
    });
    cx.run_until_parked();
    assert_eq!(request_id.sequence(), 1);
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Unavailable]
    );
    drop(subscription);
}

#[open_gpui::test]
fn rejected_native_activation_cannot_be_completed_by_later_user_focus(cx: &mut TestAppContext) {
    let (surface, host, target_window, external_window) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .build(cx)
            .expect("surface should build");
        let (target_window, host) = open_primary_host(&surface, cx);
        let external_window = cx
            .open_window(WindowOptions::default(), |_, cx| focus_panel(cx))
            .expect("external focus owner should open");
        (surface, host, target_window, external_window)
    });
    cx.run_until_parked();

    let _ = external_window
        .update(cx, |_, window, _| {
            let _ = window.activate_window();
        })
        .expect("external focus owner should activate");
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.active_window()),
        Some(external_window.into())
    );

    let target_handle = target_window;
    let dispatch = target_window
        .update(cx, |_, window, _| {
            window.request_activation_policy(WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: true,
            })
        })
        .expect("managed target should remain live");
    assert!(matches!(dispatch, WindowMutationDispatch::Queued(_)));
    assert!(cx.flush_window_mutation(target_handle, WindowMutationDomain::ActivationPolicy));

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let observed = outcomes.clone();
    let subscription = cx.update(|cx| {
        surface
            .activate_panel_with_completion("editor", cx, move |outcome, _cx| {
                observed.borrow_mut().push(outcome);
            })
            .1
    });
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Rejected]
    );
    assert_eq!(
        cx.read_entity(&host, |host, _| {
            (
                host.pending_focus_command().is_some(),
                host.viewport_runtime().activation_execution_count(),
            )
        }),
        (false, 0),
        "a rejected native ticket must retire every Dock activation authority"
    );

    let dispatch = target_window
        .update(cx, |_, window, _| {
            window.request_activation_policy(WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: true,
            })
        })
        .expect("managed target should remain live");
    assert!(matches!(dispatch, WindowMutationDispatch::Queued(_)));
    assert!(cx.flush_window_mutation(target_handle, WindowMutationDomain::ActivationPolicy));
    let _ = target_window
        .update(cx, |_, window, _| {
            let _ = window.activate_window();
        })
        .expect("a later independent activation should succeed");
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Rejected],
        "later user focus must not resurrect the rejected surface request"
    );
    drop(subscription);
}

#[open_gpui::test]
fn immediate_activation_outcome_is_suppressed_when_subscription_is_dropped(
    cx: &mut TestAppContext,
) {
    let outcomes = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .build(cx)
            .expect("surface should build");
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let observed = outcomes.clone();
        let (_, subscription) =
            surface.activate_panel_with_completion("missing", cx, move |outcome, _cx| {
                observed.borrow_mut().push(outcome);
            });
        drop(subscription);
        outcomes
    });

    cx.run_until_parked();
    assert!(outcomes.borrow().is_empty());
}

#[open_gpui::test]
fn embedded_surface_host_does_not_register_managed_activation_authority(cx: &mut TestAppContext) {
    let (surface, host) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .build(cx)
            .expect("surface should build");
        let host = cx.new(|cx| surface.host("main", cx));
        (surface, host)
    });
    let window_host = host.clone();
    let window = cx.open_window(size(px(360.0), px(240.0)), move |_, _| EmbeddedHostRoot {
        host: window_host,
        presentation: SubtreePresentation::Visible,
    });
    cx.run_until_parked();
    assert!(matches!(
        cx.read_entity(surface.owner(), |owner, _| owner
            .activation()
            .lookup_host(&"main".into())),
        DockSurfaceActivationHostLookup::Unavailable
    ));
    assert!(cx.read_entity(&host, |host, _| {
        host.viewport_runtime()
            .registration_key_for_space_window(host.space(), window.window_id())
            .is_none()
    }));

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let observed = outcomes.clone();
    let subscription = cx.update(|cx| {
        surface
            .activate_panel_with_completion("editor", cx, move |outcome, _cx| {
                observed.borrow_mut().push(outcome);
            })
            .1
    });
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Unavailable]
    );
    assert!(cx.read_entity(&host, |host, _| host.pending_focus_command().is_none()));
    drop(subscription);
}

#[open_gpui::test]
fn facade_activation_from_current_window_listener_commits_without_reborrowing_window(
    cx: &mut TestAppContext,
) {
    let (surface, host, window) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .build(cx)
            .expect("surface should build");
        let (window, host) = open_primary_host(&surface, cx);
        (surface, host, window)
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, _| {
            let _ = window.activate_window();
        })
        .expect("managed primary window should remain live");
    cx.run_until_parked();

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let observed = outcomes.clone();
    let (request_id, subscription) = window
        .update(cx, |_, current_window, app| {
            surface.activate_panel_with_completion_from_window(
                "editor",
                current_window,
                app,
                move |outcome, _cx| observed.borrow_mut().push(outcome),
            )
        })
        .expect("current event-receiver window should remain live");
    cx.run_until_parked();

    assert_eq!(request_id.sequence(), 1);
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Committed],
        "current-window activation must wait for the event receiver to return instead of reporting WindowUnavailable"
    );
    assert!(cx.read_entity(&host, |host, _| host.pending_focus_command().is_none()));
    drop(subscription);
}

#[open_gpui::test]
fn deferred_current_window_activation_rejects_replaced_viewport_registration(
    cx: &mut TestAppContext,
) {
    let (surface, host, window) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .build(cx)
            .expect("surface should build");
        let (window, host) = open_primary_host(&surface, cx);
        (surface, host, window)
    });
    cx.run_until_parked();

    let runtime = cx.read_entity(&host, |host, _| host.viewport_runtime().clone());
    let space = cx.read_entity(&host, |host, _| host.space().clone());
    let first_registration = runtime
        .registration_key_for_space_window(&space, window.window_id())
        .expect("mounted host should retain its viewport registration");
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let observed = outcomes.clone();
    let subscription = window
        .update(cx, |_, current_window, app| {
            let (_, subscription) = surface.activate_panel_with_completion_from_window(
                "editor",
                current_window,
                app,
                move |outcome, _cx| observed.borrow_mut().push(outcome),
            );
            let replacement_registration = {
                let mut runtime = runtime.borrow_mut();
                runtime.unregister_adapter_window_for_test(window.window_id());
                runtime
                    .register_opened_viewport_with_cleanup(
                        space.clone(),
                        current_window.window_handle(),
                    )
                    .expect("replacement registration should succeed")
                    .outcome
                    .registration_key()
                    .clone()
            };
            assert_ne!(first_registration, replacement_registration);
            subscription
        })
        .expect("current event-receiver window should remain live");
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Unavailable],
        "the deferred activation must reject the registration generation captured before replacement"
    );
    assert_eq!(
        cx.read_entity(&host, |host, _| {
            (
                host.pending_focus_command().is_some(),
                host.viewport_runtime().activation_execution_count() != 0,
            )
        }),
        (false, false),
        "a stale deferred activation must not mutate host or backend-focus state"
    );
    drop(subscription);
}

#[open_gpui::test]
fn activation_completion_can_reenter_surface_after_first_settlement(cx: &mut TestAppContext) {
    let (surface, _host, window) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor").selected(),
                DockPanelPlacement::stacked_with("terminal", "editor"),
            ])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .panel(
                "terminal",
                DockPanel::lazy_focusable("Terminal", focus_panel),
            )
            .build(cx)
            .expect("surface should build");
        let (window, host) = open_primary_host(&surface, cx);
        (surface, host, window)
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, _| {
            let _ = window.activate_window();
        })
        .expect("activation test window should remain live");
    cx.run_until_parked();

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let outcomes_for_callback = outcomes.clone();
    let surface_for_callback = surface.clone();
    let subscription = cx.update(|cx| {
        surface
            .activate_panel_with_completion("terminal", cx, move |outcome, cx| {
                outcomes_for_callback.borrow_mut().push(outcome);
                if outcome == DockSurfaceActivationOutcome::Committed {
                    surface_for_callback.activate_panel("editor", cx);
                }
            })
            .1
    });
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Committed]
    );
    assert_eq!(
        cx.read(|cx| surface.selected_panel_in_space("main", cx)),
        Some("editor".into())
    );
    drop(subscription);
}

#[open_gpui::test]
fn stale_surface_activation_is_rejected_before_runtime_focus_mutation(cx: &mut TestAppContext) {
    let (surface, host, window) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .build(cx)
            .expect("surface should build");
        let (window, host) = open_primary_host(&surface, cx);
        (surface, host, window)
    });
    cx.run_until_parked();

    let owner = surface.owner().clone();
    let owner_weak = owner.downgrade();
    let (binding, target_host, first_subscription) = cx.update_entity(&owner, |owner, _| {
        let lease = owner
            .window_session()
            .active_lease()
            .expect("the managed primary host should own an active lease");
        let begin =
            owner
                .activation_mut()
                .begin_request(lease, owner_weak, "main".into(), |_, _cx| {});
        let (_request_id, subscription, dispatch, settlements) = begin.into_parts();
        assert!(settlements.is_empty());
        let DockSurfaceActivationDispatch::Available(target) = dispatch else {
            panic!("expected the mounted host to be available");
        };
        (
            target.binding().clone(),
            target.host().clone(),
            subscription,
        )
    });

    let (second_subscription, second_settlements) = cx.update_entity(&owner, |owner, _| {
        let begin = owner
            .activation_mut()
            .begin_immediate_request(DockSurfaceActivationOutcome::Superseded, |_, _cx| {});
        let (_request_id, subscription, _dispatch, settlements) = begin.into_parts();
        assert!(!settlements.is_empty());
        (subscription, settlements)
    });
    cx.update(|cx| second_settlements.deliver(cx));

    let before = cx.read_entity(&host, |host, _| {
        (
            host.pending_focus_command().is_some(),
            host.viewport_runtime().activation_execution_count() != 0,
        )
    });
    let registration = cx
        .read_entity(&host, |host, _| {
            host.viewport_runtime()
                .registration_key_for_space_window(host.space(), window.window_id())
        })
        .expect("mounted host should retain its viewport registration");
    let outcome = cx.update(|cx| {
        apply_viewport_activation_transaction(
            Some(DockViewportActivationTransaction::surface_activation(
                registration,
                window,
                DockViewportFocusRequest::panel("editor"),
                binding,
                target_host,
            )),
            cx,
        )
    });
    let after = cx.read_entity(&host, |host, _| {
        (
            host.pending_focus_command().is_some(),
            host.viewport_runtime().activation_execution_count() != 0,
        )
    });

    assert_eq!(outcome, DockViewportActivationApplyOutcome::NoTarget);
    assert_eq!(
        before, after,
        "a stale surface activation must not mutate backend-focus or pending activation state"
    );
    drop(first_subscription);
    drop(second_subscription);
}

#[open_gpui::test]
fn facade_activation_keeps_selection_commit_when_inert_or_hidden_focus_is_rejected(
    cx: &mut TestAppContext,
) {
    let terminal_presentation = Rc::new(Cell::new(SubtreePresentation::Visible));
    let terminal_panel_presentation = terminal_presentation.clone();
    let (surface, _host, window) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor").selected(),
                DockPanelPlacement::stacked_with("terminal", "editor"),
            ])
            .panel("editor", DockPanel::lazy_focusable("Editor", focus_panel))
            .panel(
                "terminal",
                DockPanel::lazy_focusable("Terminal", move |cx| {
                    focus_panel_with_presentation(terminal_panel_presentation.clone(), cx)
                }),
            )
            .build(cx)
            .expect("surface should build");
        let (window, host) = open_primary_host(&surface, cx);
        (surface, host, window)
    });
    cx.run_until_parked();
    terminal_presentation.set(SubtreePresentation::Inert);
    window
        .update(cx, |_, window, _| window.refresh())
        .expect("managed primary window should remain live");
    cx.run_until_parked();

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let inert_outcomes = outcomes.clone();
    let inert_subscription = cx.update(|cx| {
        surface
            .activate_panel_with_completion("terminal", cx, move |outcome, _cx| {
                inert_outcomes.borrow_mut().push(outcome);
            })
            .1
    });
    cx.run_until_parked();
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[DockSurfaceActivationOutcome::Rejected]
    );
    assert_eq!(
        cx.read(|cx| surface.selected_panel_in_space("main", cx)),
        Some("terminal".into())
    );
    drop(inert_subscription);

    terminal_presentation.set(SubtreePresentation::Hidden);
    window
        .update(cx, |_, window, _| window.refresh())
        .expect("managed primary window should remain live");
    cx.run_until_parked();
    let hidden_outcomes = outcomes.clone();
    let hidden_subscription = cx.update(|cx| {
        surface
            .activate_panel_with_completion("terminal", cx, move |outcome, _cx| {
                hidden_outcomes.borrow_mut().push(outcome);
            })
            .1
    });
    cx.run_until_parked();
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[
            DockSurfaceActivationOutcome::Rejected,
            DockSurfaceActivationOutcome::Rejected,
        ]
    );
    assert_eq!(
        cx.read(|cx| surface.selected_panel_in_space("main", cx)),
        Some("terminal".into())
    );
    drop(hidden_subscription);
}

#[open_gpui::test]
fn activation_host_registration_rejects_duplicates_and_reuses_a_new_generation(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let controller = cx.new(|_| {
            DockController::builder("main")
                .panel_placements([DockPanelPlacement::center("editor")])
                .build()
        });
        let first_host = host_entity(cx, controller.clone());
        let second_host = host_entity(cx, controller);
        let mut state = DockSurfaceActivationState::new();
        let lease = active_activation_lease(1, 101);
        assert!(state.activate_lease(lease));

        let first_result =
            state.register_host(lease, "main".into(), first_host.downgrade(), fake_window(1));
        let first_result = first_result.expect("the active lease should register its first host");
        let (first_registration, first_settlements) = first_result.into_parts();
        assert_eq!(
            first_registration.status(),
            DockSurfaceActivationHostRegistrationStatus::Committed
        );
        assert!(first_settlements.is_empty());
        assert!(matches!(
            state.lookup_host(&"main".into()),
            DockSurfaceActivationHostLookup::Available { generation: 1, .. }
        ));

        let duplicate_result = state.register_host(
            lease,
            "main".into(),
            second_host.downgrade(),
            fake_window(2),
        );
        let duplicate_result =
            duplicate_result.expect("the active lease should record its duplicate host");
        let (duplicate_registration, duplicate_settlements) = duplicate_result.into_parts();
        assert_eq!(
            duplicate_registration.status(),
            DockSurfaceActivationHostRegistrationStatus::DuplicateHostConflict
        );
        assert!(duplicate_settlements.is_empty());
        assert!(matches!(
            state.lookup_host(&"main".into()),
            DockSurfaceActivationHostLookup::DuplicateHostConflict
        ));

        assert!(state.release_host(&duplicate_registration).is_empty());
        assert!(matches!(
            state.lookup_host(&"main".into()),
            DockSurfaceActivationHostLookup::Available { generation: 1, .. }
        ));

        assert!(state.release_host(&first_registration).is_empty());
        let replacement_result = state.register_host(
            lease,
            "main".into(),
            second_host.downgrade(),
            fake_window(2),
        );
        let replacement_result =
            replacement_result.expect("the active lease should register its replacement host");
        let (replacement_registration, replacement_settlements) = replacement_result.into_parts();
        assert_eq!(
            replacement_registration.status(),
            DockSurfaceActivationHostRegistrationStatus::Committed
        );
        assert!(replacement_settlements.is_empty());
        assert!(matches!(
            state.lookup_host(&"main".into()),
            DockSurfaceActivationHostLookup::Available { generation: 2, .. }
        ));
    });
}

#[open_gpui::test]
fn activation_state_supersedes_pending_requests_and_rejects_stale_bindings(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let controller = cx.new(|_| {
            DockController::builder("main")
                .panel_placements([DockPanelPlacement::center("editor")])
                .build()
        });
        let host = host_entity(cx, controller);
        let mut state = DockSurfaceActivationState::new();
        let lease = active_activation_lease(2, 201);
        assert!(state.activate_lease(lease));
        let registration = state
            .register_host(lease, "main".into(), host.downgrade(), fake_window(1))
            .expect("the active lease should register its host")
            .into_parts()
            .0;
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let first_outcomes = outcomes.clone();
        let first = state.begin_request(
            lease,
            open_gpui::WeakEntity::new_invalid(),
            "main".into(),
            move |outcome, _cx| first_outcomes.borrow_mut().push(outcome),
        );
        let (first_id, first_subscription, first_dispatch, first_settlements) = first.into_parts();
        assert_eq!(first_id.sequence(), 1);
        assert!(first_settlements.is_empty());
        let first_binding = match first_dispatch {
            DockSurfaceActivationDispatch::Available(target) => target.binding().clone(),
            DockSurfaceActivationDispatch::Immediate(outcome) => {
                panic!("expected an available host, got {outcome:?}")
            }
        };

        let second_outcomes = outcomes.clone();
        let second = state.begin_immediate_request(
            DockSurfaceActivationOutcome::Rejected,
            move |outcome, _cx| second_outcomes.borrow_mut().push(outcome),
        );
        let (_second_id, second_subscription, _dispatch, settlements) = second.into_parts();
        settlements.deliver(cx);
        assert_eq!(
            outcomes.borrow().as_slice(),
            &[
                DockSurfaceActivationOutcome::Superseded,
                DockSurfaceActivationOutcome::Rejected
            ]
        );

        assert!(
            state
                .settle(&first_binding, DockSurfaceActivationOutcome::Committed)
                .is_empty()
        );
        drop(first_subscription);
        drop(second_subscription);
        assert!(state.release_host(&registration).is_empty());
    });
}

#[open_gpui::test]
fn activation_shutdown_settles_g1_once_and_isolates_reopened_g2(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let controller = cx.new(|_| {
            DockController::builder("main")
                .panel_placements([DockPanelPlacement::center("editor")])
                .build()
        });
        let g1_host = host_entity(cx, controller.clone());
        let g2_host = host_entity(cx, controller);
        let authority = open_gpui::EntityId::from(3);
        let mut session = DockSurfaceWindowSession::new(authority);
        let mut state = DockSurfaceActivationState::new();

        let g1_opening = session.reserve_opening().expect("G1 should reserve");
        let g1 = session
            .commit_opening(g1_opening, WindowId::from(301))
            .expect("G1 should activate");
        assert!(state.activate_lease(g1));
        let (g1_registration, g1_registration_settlements) = state
            .register_host(g1, "main".into(), g1_host.downgrade(), fake_window(301))
            .expect("G1 should register its activation host")
            .into_parts();
        assert!(g1_registration_settlements.is_empty());

        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let observed = outcomes.clone();
        let begin = state.begin_request(
            g1,
            open_gpui::WeakEntity::new_invalid(),
            "main".into(),
            move |outcome, _cx| observed.borrow_mut().push(outcome),
        );
        let (_request, subscription, dispatch, begin_settlements) = begin.into_parts();
        assert!(begin_settlements.is_empty());
        let g1_binding = match dispatch {
            DockSurfaceActivationDispatch::Available(target) => target.binding().clone(),
            DockSurfaceActivationDispatch::Immediate(outcome) => {
                panic!("G1 activation host should be available, got {outcome:?}")
            }
        };

        assert_eq!(
            session.begin_shutdown(
                g1,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                std::iter::empty(),
            ),
            DockSurfaceWindowSessionBeginShutdownOutcome::Started {
                terminal_ticket_count: 1,
            }
        );
        state.freeze_lease(g1).deliver(cx);
        assert_eq!(
            outcomes.borrow().as_slice(),
            &[DockSurfaceActivationOutcome::WindowClosed]
        );
        assert!(
            state.freeze_lease(g1).is_empty(),
            "repeated G1 freeze must not redeliver the terminal callback"
        );
        assert!(
            state
                .settle(&g1_binding, DockSurfaceActivationOutcome::Committed)
                .is_empty(),
            "a frozen G1 binding must remain terminal"
        );
        assert!(
            state
                .register_host(g1, "main".into(), g1_host.downgrade(), fake_window(301),)
                .is_none(),
            "a frozen G1 lease must not restore activation authority"
        );

        assert_eq!(
            session.mark_runtime_empty(g1),
            DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked
        );
        assert_eq!(
            session.settle_terminal(
                g1,
                g1.anchor(),
                DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
            ),
            DockSurfaceWindowSessionTerminalOutcome::Settled
        );
        assert_eq!(
            session.complete_shutdown(g1),
            DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
        );

        let g2_opening = session.reserve_opening().expect("G2 should reserve");
        let g2 = session
            .commit_opening(g2_opening, WindowId::from(302))
            .expect("G2 should activate");
        assert!(state.activate_lease(g2));
        let (g2_registration, g2_registration_settlements) = state
            .register_host(g2, "main".into(), g2_host.downgrade(), fake_window(302))
            .expect("G2 should register the same logical space independently")
            .into_parts();
        assert!(g2_registration_settlements.is_empty());
        assert_eq!(g2_registration.lease(), g2);

        assert!(state.release_host(&g1_registration).is_empty());
        assert!(matches!(
            state.lookup_host(&"main".into()),
            DockSurfaceActivationHostLookup::Available {
                window,
                generation: 2,
                ..
            } if window == fake_window(302)
        ));
        assert_eq!(
            outcomes.borrow().as_slice(),
            &[DockSurfaceActivationOutcome::WindowClosed],
            "late G1 operations must not settle or supersede G2"
        );

        drop(subscription);
        assert!(state.release_host(&g2_registration).is_empty());
    });
}

#[open_gpui::test]
fn activation_state_dropped_subscription_suppresses_terminal_delivery(cx: &mut TestAppContext) {
    let (delivered, settlements, subscription) = cx.update(|_cx| {
        let mut state = DockSurfaceActivationState::new();
        let delivered = Rc::new(RefCell::new(false));
        let observed = delivered.clone();
        let begin = state
            .begin_immediate_request(DockSurfaceActivationOutcome::WindowClosed, move |_, _cx| {
                *observed.borrow_mut() = true
            });
        let (_request_id, subscription, _dispatch, settlements) = begin.into_parts();
        (delivered, settlements, subscription)
    });
    drop(subscription);
    cx.update(|cx| settlements.deliver(cx));
    assert!(!*delivered.borrow());
}
