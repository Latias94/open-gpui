use super::*;

struct AnnouncementProbeView {
    focus: FocusHandle,
}

impl Render for AnnouncementProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            div()
                .id("announcement-focus-target")
                .role(Role::Button)
                .aria_label("Announcement focus target")
                .focusable()
                .tab_stop(true)
                .track_focus(&self.focus),
        )
    }
}

fn open_probe(cx: &mut TestAppContext) -> crate::WindowHandle<AnnouncementProbeView> {
    cx.open_window(size(px(320.0), px(200.0)), |_, cx| AnnouncementProbeView {
        focus: cx.focus_handle(),
    })
}

fn nodes_with_label(update: &TreeUpdate, label: &str) -> Vec<NodeId> {
    update
        .nodes
        .iter()
        .filter_map(|(node_id, node)| (node.label() == Some(label)).then_some(*node_id))
        .collect()
}

#[open_gpui::test]
fn window_announcement_commits_once_then_removes_without_moving_focus(cx: &mut TestAppContext) {
    const CANARY: &str = "u14-window-announcement-canary";

    let typed_window = open_probe(cx);
    let window = typed_window.into();
    assert!(cx.activate_accessibility(window));
    typed_window
        .update(cx, |view, window, cx| view.focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    let focused = cx.latest_accessibility_tree_update(window).unwrap();
    let (focus_id, _) = node_with_label(&focused, "Announcement focus target");
    assert_eq!(focused.focus, focus_id);
    let history_start = cx.accessibility_tree_update_history(window).len();

    let outcome = typed_window
        .update(cx, |_, window, cx| {
            window.announce(AccessibilityAnnouncement::polite(CANARY), cx)
        })
        .unwrap();
    assert!(outcome.is_accepted());
    cx.run_until_parked();

    let updates = cx.accessibility_tree_update_history(window);
    let new_updates = &updates[history_start..];
    let committed = new_updates
        .iter()
        .find(|update| !nodes_with_label(update, CANARY).is_empty())
        .expect("accepted announcement must enter one final tree");
    let announcement_ids = nodes_with_label(committed, CANARY);
    assert_eq!(announcement_ids.len(), 1);
    let announcement = committed
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == announcement_ids[0])
        .map(|(_, node)| node)
        .unwrap();
    assert_eq!(announcement.role(), Role::Status);
    assert_eq!(announcement.value(), Some(CANARY));
    assert_eq!(announcement.live(), Some(accesskit::Live::Polite));
    assert!(announcement.is_live_atomic());
    assert_eq!(committed.focus, focus_id);
    assert!(
        crate::window::a11y::ACCESSKIT_ACTIONS
            .iter()
            .all(|action| !announcement.supports_action(*action))
    );

    let committed_index = new_updates
        .iter()
        .position(|update| !nodes_with_label(update, CANARY).is_empty())
        .unwrap();
    let (diagnostics_at_removal_check, refresh_pending) = typed_window
        .update(cx, |_, window, _| {
            (
                window.accessibility_announcement_diagnostics().to_vec(),
                window.refresh_pending_for_test(),
            )
        })
        .unwrap();
    assert!(
        new_updates[committed_index + 1..]
            .iter()
            .any(|update| nodes_with_label(update, CANARY).is_empty()),
        "a later committed tree must remove the transient node; updates={}, diagnostics={diagnostics_at_removal_check:?}, refresh_pending={refresh_pending}",
        new_updates.len(),
    );
    assert_eq!(new_updates.last().unwrap().focus, focus_id);

    let diagnostics = typed_window
        .update(cx, |_, window, _| {
            window.accessibility_announcement_diagnostics().to_vec()
        })
        .unwrap();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle() == AccessibilityAnnouncementLifecycle::Accepted
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle() == AccessibilityAnnouncementLifecycle::Committed
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle() == AccessibilityAnnouncementLifecycle::Removed
    }));
    assert!(!format!("{diagnostics:?}").contains(CANARY));
}

#[open_gpui::test]
fn repeated_text_and_two_windows_keep_sequences_and_nodes_isolated(cx: &mut TestAppContext) {
    const FIRST_MESSAGE: &str = "Repeated window announcement";
    const SECOND_MESSAGE: &str = "Second window announcement";

    let first_window = open_probe(cx);
    let second_window = open_probe(cx);
    let first = first_window.into();
    let second = second_window.into();
    assert!(cx.activate_accessibility(first));
    assert!(cx.activate_accessibility(second));
    let first_start = cx.accessibility_tree_update_history(first).len();
    let second_start = cx.accessibility_tree_update_history(second).len();

    let (first_outcome, repeated_outcome) = first_window
        .update(cx, |_, window, cx| {
            (
                window.announce(AccessibilityAnnouncement::polite(FIRST_MESSAGE), cx),
                window.announce(AccessibilityAnnouncement::polite(FIRST_MESSAGE), cx),
            )
        })
        .unwrap();
    let second_outcome = second_window
        .update(cx, |_, window, cx| {
            window.announce(AccessibilityAnnouncement::assertive(SECOND_MESSAGE), cx)
        })
        .unwrap();
    assert_eq!(first_outcome.sequence().unwrap().as_u64(), 1);
    assert_eq!(repeated_outcome.sequence().unwrap().as_u64(), 2);
    assert_eq!(second_outcome.sequence().unwrap().as_u64(), 1);
    cx.run_until_parked();

    let first_updates = cx.accessibility_tree_update_history(first);
    let first_committed = first_updates[first_start..]
        .iter()
        .find(|update| nodes_with_label(update, FIRST_MESSAGE).len() == 2)
        .expect("equal text requests must remain distinct");
    let first_ids = nodes_with_label(first_committed, FIRST_MESSAGE);
    assert_ne!(first_ids[0], first_ids[1]);

    let second_updates = cx.accessibility_tree_update_history(second);
    let second_committed = second_updates[second_start..]
        .iter()
        .find(|update| nodes_with_label(update, SECOND_MESSAGE).len() == 1)
        .expect("second window announcement must commit independently");
    let second_id = nodes_with_label(second_committed, SECOND_MESSAGE)[0];
    assert!(!first_ids.contains(&second_id));
    assert!(
        first_updates
            .iter()
            .all(|update| nodes_with_label(update, SECOND_MESSAGE).is_empty())
    );
    assert!(
        second_updates
            .iter()
            .all(|update| nodes_with_label(update, FIRST_MESSAGE).is_empty())
    );
}

#[open_gpui::test]
fn interaction_quiescence_clears_accepted_announcements_and_drops_new_requests(
    cx: &mut TestAppContext,
) {
    const ACCEPTED: &str = "Accepted before interaction quiescence";
    const DROPPED: &str = "Dropped after interaction quiescence";

    let typed_window = open_probe(cx);
    let window = typed_window.into();
    assert!(cx.activate_accessibility(window));
    let history_start = cx.accessibility_tree_update_history(window).len();

    let (accepted, dropped, diagnostics) = typed_window
        .update(cx, |_, window, cx| {
            let accepted = window.announce(AccessibilityAnnouncement::polite(ACCEPTED), cx);
            assert!(window.quiesce_interaction(cx));
            let dropped = window.announce(AccessibilityAnnouncement::assertive(DROPPED), cx);
            (
                accepted,
                dropped,
                window.accessibility_announcement_diagnostics().to_vec(),
            )
        })
        .unwrap();

    assert!(accepted.is_accepted());
    assert!(accepted.sequence().is_some());
    assert_eq!(
        dropped.drop_reason(),
        Some(AccessibilityAnnouncementDropReason::InteractionQuiesced)
    );
    assert_eq!(dropped.sequence(), None);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle()
            == AccessibilityAnnouncementLifecycle::Cleared(
                AccessibilityAnnouncementClearReason::InteractionQuiesced,
            )
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle()
            == AccessibilityAnnouncementLifecycle::Dropped(
                AccessibilityAnnouncementDropReason::InteractionQuiesced,
            )
    }));
    assert!(!format!("{diagnostics:?}").contains(ACCEPTED));
    assert!(!format!("{diagnostics:?}").contains(DROPPED));

    cx.run_until_parked();
    assert!(
        cx.accessibility_tree_update_history(window)[history_start..]
            .iter()
            .all(|update| {
                nodes_with_label(update, ACCEPTED).is_empty()
                    && nodes_with_label(update, DROPPED).is_empty()
            }),
        "interaction-quiesced announcements must never publish or replay"
    );
}

#[open_gpui::test]
fn inactive_replacement_and_close_drop_without_replay(cx: &mut TestAppContext) {
    const INACTIVE: &str = "Inactive announcement must not replay";
    const DEACTIVATED: &str = "Deactivated announcement must not replay";
    const STALE: &str = "Stale activation announcement must not replay";
    const CLOSING: &str = "Closing announcement must not replay";

    let typed_window = open_probe(cx);
    let window = typed_window.into();
    let inactive = typed_window
        .update(cx, |_, window, cx| {
            window.announce(AccessibilityAnnouncement::polite(INACTIVE), cx)
        })
        .unwrap();
    assert_eq!(
        inactive.drop_reason(),
        Some(AccessibilityAnnouncementDropReason::AccessibilityInactive)
    );
    assert_eq!(inactive.sequence(), None);
    assert!(cx.activate_accessibility(window));
    assert!(
        cx.accessibility_tree_update_history(window)
            .iter()
            .all(|update| nodes_with_label(update, INACTIVE).is_empty())
    );

    assert!(cx.deactivate_accessibility(window));
    let deactivated = typed_window
        .update(cx, |_, window, cx| {
            window.announce(AccessibilityAnnouncement::polite(DEACTIVATED), cx)
        })
        .unwrap();
    assert_eq!(
        deactivated.drop_reason(),
        Some(AccessibilityAnnouncementDropReason::AccessibilityInactive)
    );
    assert!(cx.activate_accessibility(window));
    assert!(
        cx.accessibility_tree_update_history(window)
            .iter()
            .all(|update| nodes_with_label(update, DEACTIVATED).is_empty())
    );

    let platform_window = cx.test_window(window);
    let stale = typed_window
        .update(cx, |_, window, cx| {
            let outcome = window.announce(AccessibilityAnnouncement::assertive(STALE), cx);
            assert!(platform_window.activate_accessibility());
            outcome
        })
        .unwrap();
    assert!(stale.is_accepted());
    cx.run_until_parked();
    assert!(
        cx.accessibility_tree_update_history(window)
            .iter()
            .all(|update| nodes_with_label(update, STALE).is_empty())
    );
    let diagnostics = typed_window
        .update(cx, |_, window, _| {
            window.accessibility_announcement_diagnostics().to_vec()
        })
        .unwrap();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle()
            == AccessibilityAnnouncementLifecycle::Cleared(
                AccessibilityAnnouncementClearReason::ActivationReplaced,
            )
    }));

    let (accepted_before_close, dropped_while_closing, close_diagnostics) = typed_window
        .update(cx, |_, window, cx| {
            let accepted = window.announce(AccessibilityAnnouncement::polite(CLOSING), cx);
            window.remove_window(cx);
            let dropped =
                window.announce(AccessibilityAnnouncement::assertive("Already closing"), cx);
            (
                accepted,
                dropped,
                window.accessibility_announcement_diagnostics().to_vec(),
            )
        })
        .unwrap();
    assert!(accepted_before_close.is_accepted());
    assert_eq!(
        dropped_while_closing.drop_reason(),
        Some(AccessibilityAnnouncementDropReason::WindowClosed)
    );
    assert!(close_diagnostics.iter().any(|diagnostic| {
        diagnostic.lifecycle()
            == AccessibilityAnnouncementLifecycle::Cleared(
                AccessibilityAnnouncementClearReason::WindowClosed,
            )
    }));
    assert!(!format!("{close_diagnostics:?}").contains(CLOSING));
}

#[open_gpui::test]
fn native_close_rejects_announcement_before_logical_close_delivery(cx: &mut TestAppContext) {
    let typed_window = open_probe(cx);
    let window = typed_window.into();
    assert!(cx.activate_accessibility(window));

    let platform_window = cx.test_window(window);
    assert!(platform_window.simulate_close());

    let outcome = typed_window
        .update(cx, |_, window, cx| {
            window.announce(AccessibilityAnnouncement::polite("native closed"), cx)
        })
        .expect("the logical window remains registered until its close event is delivered");
    assert_eq!(
        outcome.drop_reason(),
        Some(AccessibilityAnnouncementDropReason::WindowClosed)
    );

    cx.run_until_parked();
}
