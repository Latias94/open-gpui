use super::{
    FocusScopeRegistration, FocusScopeRuntime, FocusScopeRuntimeError, FocusTargetRegistration,
};
use open_gpui::{
    AppContext as _, Context, FocusHandle, IntoElement, KeyDownEvent, Keystroke, Modifiers, Render,
    VisualContext as _, Window, div, prelude::*,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, FocusScopeId, FocusScopeMode, FocusScopePolicy, FocusTargetAvailability,
    FocusTargetId, InitialFocusIntent,
};

const PARENT_SCOPE: &str = "focus-scope.parent";
const CHILD_SCOPE: &str = "focus-scope.child";
const EMPTY_SCOPE: &str = "focus-scope.empty";
const PENDING_SCOPE: &str = "focus-scope.pending";

struct FocusScopeProbe {
    runtime: FocusScopeRuntime,
    outside: FocusHandle,
    misplaced: FocusHandle,
    rebound: FocusHandle,
    fallback: FocusHandle,
    parent_root: FocusHandle,
    parent_first: FocusHandle,
    parent_last: FocusHandle,
    child_misplaced: FocusHandle,
    child_root: FocusHandle,
    child_first: FocusHandle,
    child_last: FocusHandle,
    child_unregistered: FocusHandle,
    empty_root: FocusHandle,
    deferred_target: FocusHandle,
    pending_root: FocusHandle,
    show_outside: bool,
    show_empty: bool,
    show_deferred_target: bool,
    show_pending: bool,
}

impl FocusScopeProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let runtime = FocusScopeRuntime::new(window, cx);
        let outside = cx.focus_handle();
        let misplaced = cx.focus_handle();
        let rebound = cx.focus_handle();
        let fallback = cx.focus_handle();
        let parent_root = cx.focus_handle();
        let parent_first = cx.focus_handle();
        let parent_last = cx.focus_handle();
        let child_misplaced = cx.focus_handle();
        let child_root = cx.focus_handle();
        let child_first = cx.focus_handle();
        let child_last = cx.focus_handle();
        let child_unregistered = cx.focus_handle();
        let empty_root = cx.focus_handle();
        let deferred_target = cx.focus_handle();
        let pending_root = cx.focus_handle();

        runtime
            .register_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(PARENT_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::None)
                        .with_focus_restore(FocusRestoreIntent::Trigger),
                    &parent_root,
                ),
                window,
                cx,
            )
            .expect("parent scope registration should be unique");
        runtime
            .register_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(CHILD_SCOPE, FocusScopeMode::ModalLoop)
                        .with_parent(PARENT_SCOPE)
                        .with_initial_focus(InitialFocusIntent::TargetOrFirstFocusable(
                            open_gpui_ui_core::FocusTargetId::new("child.missing"),
                        ))
                        .with_focus_restore(FocusRestoreIntent::Trigger),
                    &child_root,
                ),
                window,
                cx,
            )
            .expect("child scope registration should be unique");
        runtime
            .register_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(EMPTY_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::TargetOrFirstFocusable(
                            FocusTargetId::new("empty.misplaced"),
                        ))
                        .with_focus_restore(FocusRestoreIntent::None),
                    &empty_root,
                )
                .with_surface("empty.surface"),
                window,
                cx,
            )
            .expect("empty scope registration should be unique");
        runtime
            .register_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(PENDING_SCOPE, FocusScopeMode::ModalLoop)
                        .with_parent(PARENT_SCOPE)
                        .with_initial_focus(InitialFocusIntent::FirstFocusable),
                    &pending_root,
                )
                .with_surface("pending.surface"),
                window,
                cx,
            )
            .expect("pending scope registration should be unique");

        for registration in [
            FocusTargetRegistration::new("outside", &outside),
            FocusTargetRegistration::new("empty.misplaced", &misplaced).within_scope(EMPTY_SCOPE),
            FocusTargetRegistration::new("window-fallback", &fallback),
            FocusTargetRegistration::new("parent.first", &parent_first).within_scope(PARENT_SCOPE),
            FocusTargetRegistration::new("parent.last", &parent_last).within_scope(PARENT_SCOPE),
            FocusTargetRegistration::new("child.misplaced", &child_misplaced)
                .within_scope(CHILD_SCOPE),
            FocusTargetRegistration::new("child.first", &child_first).within_scope(CHILD_SCOPE),
            FocusTargetRegistration::new("child.last", &child_last).within_scope(CHILD_SCOPE),
            FocusTargetRegistration::new("empty.surface", &empty_root).within_scope(EMPTY_SCOPE),
            FocusTargetRegistration::new("empty.deferred", &deferred_target)
                .within_scope(EMPTY_SCOPE),
            FocusTargetRegistration::new("pending.surface", &pending_root)
                .within_scope(PENDING_SCOPE),
        ] {
            runtime
                .register_target(registration, window, cx)
                .expect("focus target registration should be unique");
        }
        runtime
            .set_window_fallback(
                Some(open_gpui_ui_core::FocusTargetId::new("window-fallback")),
                window,
                cx,
            )
            .expect("window fallback should belong to this runtime");

        Self {
            runtime,
            outside,
            misplaced,
            rebound,
            fallback,
            parent_root,
            parent_first,
            parent_last,
            child_misplaced,
            child_root,
            child_first,
            child_last,
            child_unregistered,
            empty_root,
            deferred_target,
            pending_root,
            show_outside: true,
            show_empty: true,
            show_deferred_target: false,
            show_pending: false,
        }
    }
}

impl Render for FocusScopeProbe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let runtime = self.runtime.clone();

        div()
            .id("focus-scope-probe")
            .capture_key_down(move |event, window, cx| {
                runtime.handle_key_down(event, window, cx);
            })
            .when(self.show_outside, |root| {
                root.child(
                    div()
                        .child(
                            div()
                                .id("focus-scope-outside")
                                .debug_selector(|| "focus-scope:outside".to_owned())
                                .track_focus(&self.outside)
                                .tab_index(0),
                        )
                        .child(
                            div()
                                .id("focus-scope-misplaced")
                                .debug_selector(|| "focus-scope:misplaced".to_owned())
                                .track_focus(&self.misplaced)
                                .tab_index(1),
                        ),
                )
            })
            .child(
                div()
                    .id("focus-scope-rebound")
                    .debug_selector(|| "focus-scope:rebound".to_owned())
                    .track_focus(&self.rebound)
                    .tab_index(2),
            )
            .child(
                div()
                    .id("focus-scope-window-fallback")
                    .debug_selector(|| "focus-scope:window-fallback".to_owned())
                    .track_focus(&self.fallback)
                    .tab_index(1),
            )
            .child(
                div()
                    .id("focus-scope-parent")
                    .debug_selector(|| "focus-scope:parent".to_owned())
                    .track_focus(&self.parent_root)
                    .tab_group()
                    .tab_stop(false)
                    .child(
                        div()
                            .id("focus-scope-parent-first")
                            .debug_selector(|| "focus-scope:parent-first".to_owned())
                            .track_focus(&self.parent_first)
                            .tab_index(0),
                    )
                    .child(
                        div()
                            .id("focus-scope-parent-last")
                            .debug_selector(|| "focus-scope:parent-last".to_owned())
                            .track_focus(&self.parent_last)
                            .tab_index(1),
                    )
                    .child(
                        div()
                            .id("focus-scope-child-misplaced")
                            .debug_selector(|| "focus-scope:child-misplaced".to_owned())
                            .track_focus(&self.child_misplaced)
                            .tab_index(3),
                    )
                    .child(
                        div()
                            .id("focus-scope-child")
                            .debug_selector(|| "focus-scope:child".to_owned())
                            .track_focus(&self.child_root)
                            .tab_group()
                            .tab_stop(false)
                            .child(
                                div()
                                    .id("focus-scope-child-first")
                                    .debug_selector(|| "focus-scope:child-first".to_owned())
                                    .track_focus(&self.child_first)
                                    .tab_index(0),
                            )
                            .child(
                                div()
                                    .id("focus-scope-child-last")
                                    .debug_selector(|| "focus-scope:child-last".to_owned())
                                    .track_focus(&self.child_last)
                                    .tab_index(1),
                            )
                            .child(
                                div()
                                    .id("focus-scope-child-unregistered")
                                    .debug_selector(|| "focus-scope:child-unregistered".to_owned())
                                    .track_focus(&self.child_unregistered)
                                    .tab_index(2),
                            ),
                    )
                    .when(self.show_pending, |parent| {
                        parent.child(
                            div()
                                .id("focus-scope-pending")
                                .debug_selector(|| "focus-scope:pending-surface".to_owned())
                                .track_focus(&self.pending_root)
                                .tab_group()
                                .tab_stop(false),
                        )
                    }),
            )
            .when(self.show_empty, |root| {
                root.child(
                    div()
                        .id("focus-scope-empty")
                        .debug_selector(|| "focus-scope:empty-surface".to_owned())
                        .track_focus(&self.empty_root)
                        .tab_group()
                        .tab_stop(false)
                        .when(self.show_deferred_target, |surface| {
                            surface.child(
                                div()
                                    .id("focus-scope-empty-deferred")
                                    .debug_selector(|| "focus-scope:empty-deferred".to_owned())
                                    .track_focus(&self.deferred_target)
                                    .tab_index(0),
                            )
                        }),
                )
            })
    }
}

fn draw(cx: &mut open_gpui::VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
}

fn settle_focus_claims_after_render(cx: &mut open_gpui::VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
    });
    cx.run_until_parked();
}

fn assert_focused(cx: &mut open_gpui::VisualTestContext, selector: &str) {
    assert_eq!(
        cx.focused_debug_selector().as_deref(),
        Some(selector),
        "unexpected focused selector"
    );
}

#[open_gpui::test]
fn real_tab_and_shift_tab_loop_inside_active_modal_scope(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
    });

    cx.simulate_keystrokes("tab");
    assert_focused(cx, "focus-scope:parent-first");
    cx.simulate_keystrokes("shift-tab");
    assert_focused(cx, "focus-scope:parent-last");
}

#[open_gpui::test]
fn nested_scope_close_restores_parent_then_outside(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be registered");
    });
    settle_focus_claims_after_render(cx);
    assert_focused(cx, "focus-scope:child-first");

    cx.simulate_keystrokes("tab");
    assert_focused(cx, "focus-scope:child-last");
    cx.simulate_keystrokes("shift-tab");
    assert_focused(cx, "focus-scope:child-first");

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .deactivate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be active");
    });
    settle_focus_claims_after_render(cx);
    assert_focused(cx, "focus-scope:parent-last");

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);
    assert_focused(cx, "focus-scope:outside");
}

#[open_gpui::test]
fn newer_programmatic_focus_suppresses_queued_restore(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be registered");
    });
    settle_focus_claims_after_render(cx);
    assert_focused(cx, "focus-scope:child-first");

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .deactivate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be active");
        view.outside.focus(window, cx);
    });
    cx.run_until_parked();

    assert_focused(cx, "focus-scope:outside");
}

#[open_gpui::test]
fn missing_saved_target_uses_live_window_fallback(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.show_outside = false;
        cx.notify();
    });
    draw(cx);
    assert!(
        cx.debug_bounds("focus-scope:outside").is_none(),
        "saved target should be unmounted before restoration"
    );

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:window-fallback");
}

#[open_gpui::test]
fn restore_waits_for_the_frame_that_unmounts_the_saved_target(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.show_outside = false;
        cx.notify();
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert!(cx.debug_bounds("focus-scope:outside").is_none());
    assert_focused(cx, "focus-scope:window-fallback");
}

#[open_gpui::test]
fn no_restore_candidate_clears_closing_scope_focus(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.runtime
            .set_window_fallback(None, window, cx)
            .expect("window fallback should belong to this runtime");
        view.show_outside = false;
        cx.notify();
    });
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_eq!(cx.focused_debug_selector(), None);
}

#[open_gpui::test]
fn identical_scope_ids_are_isolated_between_windows(cx: &mut open_gpui::TestAppContext) {
    let first_window = cx.add_window(FocusScopeProbe::new);
    let second_window = cx.add_window(FocusScopeProbe::new);
    let first_any = first_window.clone().into();
    let second_any = second_window.clone().into();

    cx.update_window(first_any, |_, window, cx| window.draw(cx).clear())
        .expect("first window should remain open");
    cx.update_window(second_any, |_, window, cx| window.draw(cx).clear())
        .expect("second window should remain open");
    first_window
        .update(cx, |view, window, cx| {
            view.runtime
                .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
                .expect("first parent scope should be registered");
            view.parent_last.focus(window, cx);
        })
        .expect("first window should remain open");
    second_window
        .update(cx, |view, window, cx| {
            view.runtime
                .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
                .expect("second parent scope should be registered");
            view.parent_last.focus(window, cx);
        })
        .expect("second window should remain open");
    cx.simulate_keystrokes(first_any, "tab");

    assert!(cx.debug_selector_is_focused_in_window(first_any, "focus-scope:parent-first"));
    assert!(cx.debug_selector_is_focused_in_window(second_any, "focus-scope:parent-last"));
}

#[open_gpui::test]
fn empty_modal_scope_keeps_focus_on_its_surface(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
    });
    settle_focus_claims_after_render(cx);
    assert_focused(cx, "focus-scope:empty-surface");

    cx.simulate_keystrokes("tab shift-tab");
    assert_focused(cx, "focus-scope:empty-surface");

    cx.update_window_entity(&view, |view, window, cx| view.outside.focus(window, cx));
    cx.simulate_keystrokes("tab");
    assert_focused(cx, "focus-scope:empty-surface");
}

#[open_gpui::test]
fn opening_initial_focus_wins_over_a_later_restore_in_the_same_turn(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
    });
    settle_focus_claims_after_render(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be registered");
        view.runtime
            .rebind_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(EMPTY_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::FirstFocusable)
                        .with_focus_restore(FocusRestoreIntent::Trigger),
                    &view.empty_root,
                )
                .with_surface("empty.surface"),
                window,
                cx,
            )
            .expect("empty scope should support policy rebinding");
        view.runtime
            .deactivate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:child-first");
}

#[open_gpui::test]
fn persistent_scope_waits_for_the_new_frame_before_resolving_initial_target(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .rebind_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(EMPTY_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::TargetOrFirstFocusable(
                            FocusTargetId::new("empty.deferred"),
                        )),
                    &view.empty_root,
                )
                .with_surface("empty.surface"),
                window,
                cx,
            )
            .expect("empty scope should support policy rebinding");
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
        view.show_deferred_target = true;
        cx.notify();
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:empty-deferred");
}

#[open_gpui::test]
fn logically_innermost_unrendered_modal_consumes_tab_until_it_mounts(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PENDING_SCOPE), window, cx)
            .expect("pending scope should be registered");
    });
    let dispatch = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: "tab".to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });

    assert_focused(cx, "focus-scope:parent-last");
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());

    cx.update_window_entity(&view, |view, _, cx| {
        view.show_pending = true;
        cx.notify();
    });
    settle_focus_claims_after_render(cx);
    assert_focused(cx, "focus-scope:pending-surface");
}

#[open_gpui::test]
fn modified_tab_is_left_for_application_shortcuts(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
    });
    let dispatch = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Modifiers::none()
            },
            key: "tab".to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });

    assert_focused(cx, "focus-scope:parent-first");
    assert!(!dispatch.default_prevented());
    assert!(dispatch.propagated());
}

#[open_gpui::test]
fn runtime_rejects_scoped_window_fallbacks_and_handle_aliases(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        assert_eq!(
            view.runtime
                .set_window_fallback(Some(FocusTargetId::new("parent.first")), window, cx,),
            Err(FocusScopeRuntimeError::ScopedWindowFallback(
                FocusTargetId::new("parent.first")
            ))
        );
        assert_eq!(
            view.runtime.register_target(
                FocusTargetRegistration::new("outside.alias", &view.outside),
                window,
                cx,
            ),
            Err(FocusScopeRuntimeError::DuplicateTargetHandle(
                FocusTargetId::new("outside")
            ))
        );
        assert_eq!(
            view.runtime.rebind_target(
                FocusTargetRegistration::new("window-fallback", &view.fallback)
                    .within_scope(PARENT_SCOPE),
                window,
                cx,
            ),
            Err(FocusScopeRuntimeError::ScopedWindowFallback(
                FocusTargetId::new("window-fallback")
            ))
        );
    });
}

#[open_gpui::test]
fn ancestor_initial_focus_rejects_a_descendant_target_outside_its_own_root(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be registered");
    });
    settle_focus_claims_after_render(cx);
    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .rebind_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(PARENT_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::TargetOrFirstFocusable(
                            FocusTargetId::new("child.misplaced"),
                        ))
                        .with_focus_restore(FocusRestoreIntent::Trigger),
                    &view.parent_root,
                ),
                window,
                cx,
            )
            .expect("parent scope should support policy rebinding");
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:parent-first");
}

#[open_gpui::test]
fn latest_none_restore_policy_cancels_a_queued_restore(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
        view.runtime
            .rebind_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(PARENT_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::None)
                        .with_focus_restore(FocusRestoreIntent::None),
                    &view.parent_root,
                ),
                window,
                cx,
            )
            .expect("parent scope should support policy rebinding");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:parent-first");
}

#[open_gpui::test]
fn canceling_a_newer_restore_does_not_strand_an_initial_retry(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
    });
    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    cx.run_until_parked();
    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should reopen and cancel its restore");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:empty-surface");
}

#[open_gpui::test]
fn restore_rejects_targets_owned_by_an_inactive_scope(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .rebind_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(PARENT_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::None)
                        .with_focus_restore(FocusRestoreIntent::Fallback(FocusTargetId::new(
                            "child.first",
                        ))),
                    &view.parent_root,
                ),
                window,
                cx,
            )
            .expect("parent scope should support policy rebinding");
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:window-fallback");
}

#[open_gpui::test]
fn registered_unavailable_targets_are_skipped_by_real_tab_traversal(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
    });

    for availability in [
        FocusTargetAvailability::Disabled,
        FocusTargetAvailability::Hidden,
        FocusTargetAvailability::Stale,
    ] {
        cx.update_window_entity(&view, |view, window, cx| {
            view.runtime
                .set_target_availability(
                    &FocusTargetId::new("parent.first"),
                    availability,
                    window,
                    cx,
                )
                .expect("parent target should be registered");
        });
        cx.simulate_keystrokes("tab");
        assert_focused(cx, "focus-scope:parent-last");
    }

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .set_target_availability(
                &FocusTargetId::new("parent.first"),
                FocusTargetAvailability::Available,
                window,
                cx,
            )
            .expect("parent target should be registered");
    });
    cx.simulate_keystrokes("tab");
    assert_focused(cx, "focus-scope:parent-first");
}

#[open_gpui::test]
fn inactive_nested_scope_targets_do_not_join_parent_traversal(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
    });

    cx.simulate_keystrokes("tab");
    assert_focused(cx, "focus-scope:parent-last");
    cx.simulate_keystrokes("tab");
    assert_focused(cx, "focus-scope:parent-first");
}

#[open_gpui::test]
fn no_intent_transition_does_not_cancel_an_unrelated_initial_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
    });
    settle_focus_claims_after_render(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_last.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(CHILD_SCOPE), window, cx)
            .expect("child scope should be registered");
        view.runtime
            .deactivate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:child-first");
}

#[open_gpui::test]
fn empty_modal_without_a_surface_clears_external_focus_on_tab(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
    });
    settle_focus_claims_after_render(cx);
    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .unregister_target(&FocusTargetId::new("empty.surface"), window, cx)
            .expect("surface target should be registered");
        view.outside.focus(window, cx);
    });

    cx.simulate_keystrokes("tab");
    assert_eq!(cx.focused_debug_selector(), None);
}

#[open_gpui::test]
fn repeated_activation_preserves_the_original_restore_target(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("repeated activation should be idempotent");
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:outside");
}

#[open_gpui::test]
fn reopening_a_none_initial_scope_cancels_its_queued_restore(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should reopen");
        view.runtime
            .set_target_availability(
                &FocusTargetId::new("parent.first"),
                FocusTargetAvailability::Stale,
                window,
                cx,
            )
            .expect("parent target should be registered");
    });
    cx.run_until_parked();

    assert_focused(cx, "focus-scope:parent-first");
}

#[open_gpui::test]
fn initial_focus_retries_after_a_conditionally_mounted_scope_renders(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    cx.update_window_entity(&view, |view, _, cx| {
        view.show_empty = false;
        cx.notify();
    });
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
        view.show_empty = true;
        cx.notify();
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:empty-surface");
}

#[open_gpui::test]
fn reasserting_the_current_focus_suppresses_an_older_restore(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
        view.parent_first.focus(window, cx);
    });
    cx.run_until_parked();

    assert_focused(cx, "focus-scope:parent-first");
}

#[open_gpui::test]
fn target_rebind_restores_the_same_logical_identity_to_its_new_handle(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.outside.focus(window, cx);
        view.runtime
            .activate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be registered");
        view.parent_first.focus(window, cx);
        view.runtime
            .rebind_target(
                FocusTargetRegistration::new("outside", &view.rebound),
                window,
                cx,
            )
            .expect("outside target should support live handle rebinding");
        view.runtime
            .deactivate_scope(FocusScopeId::new(PARENT_SCOPE), window, cx)
            .expect("parent scope should be active");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:rebound");
}

#[open_gpui::test]
fn unregister_releases_scope_and_target_identities_for_remount(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(FocusScopeProbe::new);
    draw(cx);

    cx.update_window_entity(&view, |view, window, cx| {
        view.runtime
            .unregister_scope(&FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("empty scope should be registered");
        view.runtime
            .register_scope(
                FocusScopeRegistration::new(
                    FocusScopePolicy::new(EMPTY_SCOPE, FocusScopeMode::ModalLoop)
                        .with_initial_focus(InitialFocusIntent::FirstFocusable),
                    &view.empty_root,
                )
                .with_surface("empty.surface"),
                window,
                cx,
            )
            .expect("unregistered scope identity should be reusable");
        view.runtime
            .register_target(
                FocusTargetRegistration::new("empty.surface", &view.empty_root)
                    .within_scope(EMPTY_SCOPE),
                window,
                cx,
            )
            .expect("targets removed with a scope should be reusable");
        view.runtime
            .activate_scope(FocusScopeId::new(EMPTY_SCOPE), window, cx)
            .expect("remounted scope should activate");
    });
    settle_focus_claims_after_render(cx);

    assert_focused(cx, "focus-scope:empty-surface");
}
