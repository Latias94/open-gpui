use crate::{
    DockController, DockHost, DockPanel, DockPanelPlacement, DockSurface,
    DockSurfaceActivationOutcome, DockViewportActivationTransaction, DockViewportFocusRequest,
    DockViewportRuntimeHandle,
    surface::{
        DockSurfaceActivationDispatch, DockSurfaceActivationHostLookup,
        DockSurfaceActivationHostRegistrationStatus, DockSurfaceActivationState,
    },
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, SubtreePresentation,
    SubtreePresentationExt, TestAppContext, Window, WindowHandle, WindowId, div, px, size,
};
use std::{cell::RefCell, rc::Rc};

struct FocusPanel {
    focus_handle: FocusHandle,
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
    cx.new(|cx| FocusPanel {
        focus_handle: cx.focus_handle(),
    })
}

fn fake_window(id: u64) -> AnyWindowHandle {
    WindowHandle::<DockHost>::new(WindowId::from(id)).into()
}

fn host_entity<C: AppContext>(cx: &mut C, controller: Entity<DockController>) -> Entity<DockHost> {
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    cx.new(|cx| DockHost::from_controller(controller, "main", runtime, cx))
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
fn facade_activation_targets_a_host_nested_below_an_arbitrary_window_root(cx: &mut TestAppContext) {
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
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("embedded host window should remain live");
    cx.run_until_parked();
    assert!(matches!(
        cx.read_entity(surface.owner(), |owner, _| owner
            .activation()
            .lookup_host(&"main".into())),
        DockSurfaceActivationHostLookup::Available { .. }
    ));

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
        &[DockSurfaceActivationOutcome::Committed]
    );
    assert!(cx.read_entity(&host, |host, _| host.pending_focus_command().is_none()));
    drop(subscription);
}

#[open_gpui::test]
fn activation_completion_can_reenter_surface_after_first_settlement(cx: &mut TestAppContext) {
    let (surface, host) = cx.update(|cx| {
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
        let host = cx.new(|cx| surface.host("main", cx));
        (surface, host)
    });
    let window_host = host.clone();
    let window = cx.open_window(size(px(360.0), px(240.0)), move |_, _| EmbeddedHostRoot {
        host: window_host,
        presentation: SubtreePresentation::Visible,
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, _| window.activate_window())
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

    let owner = surface.owner().clone();
    let owner_weak = owner.downgrade();
    let (binding, target_host, first_subscription) = cx.update_entity(&owner, |owner, _| {
        let begin = owner
            .activation_mut()
            .begin_request(owner_weak, "main".into(), |_, _cx| {});
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
            host.viewport_runtime().pending_activation().is_some(),
        )
    });
    let outcome = cx.update(|cx| {
        apply_viewport_activation_transaction(
            Some(DockViewportActivationTransaction::surface_activation(
                "main",
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
            host.viewport_runtime().pending_activation().is_some(),
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
fn facade_activation_rejects_inert_and_hidden_hosts_without_selecting(cx: &mut TestAppContext) {
    let (surface, host) = cx.update(|cx| {
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
        let host = cx.new(|cx| surface.host("main", cx));
        (surface, host)
    });
    let window_host = host.clone();
    let window = cx.open_window(size(px(360.0), px(240.0)), move |_, _| EmbeddedHostRoot {
        host: window_host,
        presentation: SubtreePresentation::Visible,
    });
    cx.run_until_parked();
    window
        .update(cx, |fixture, _, cx| {
            fixture.presentation = SubtreePresentation::Inert;
            cx.notify();
        })
        .expect("fixture window should remain live");
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
        Some("editor".into())
    );
    drop(inert_subscription);

    window
        .update(cx, |fixture, _, cx| {
            fixture.presentation = SubtreePresentation::Hidden;
            cx.notify();
        })
        .expect("fixture window should remain live");
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
        Some("editor".into())
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

        let first_result =
            state.register_host("main".into(), first_host.downgrade(), fake_window(1));
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

        let duplicate_result =
            state.register_host("main".into(), second_host.downgrade(), fake_window(2));
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
        let replacement_result =
            state.register_host("main".into(), second_host.downgrade(), fake_window(2));
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
        let registration = state
            .register_host("main".into(), host.downgrade(), fake_window(1))
            .into_parts()
            .0;
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let first_outcomes = outcomes.clone();
        let first = state.begin_request(
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
