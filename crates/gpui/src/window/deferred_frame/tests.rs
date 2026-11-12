use super::*;
use crate::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, Render, RequestFrameOptions,
    Role, StatefulInteractiveElement as _, Styled as _, TestAppContext, WindowOptions, div, rgb,
};
use accesskit::{Action, ActionRequest, TreeId};
use std::cell::RefCell;

struct ActionRoot {
    revision: bool,
    actions: Rc<RefCell<Vec<bool>>>,
}

impl Render for ActionRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let revision = self.revision;
        let actions = self.actions.clone();
        div()
            .id("queued-accessible-action")
            .role(Role::Button)
            .aria_label("queued action")
            .size_full()
            .bg(rgb(if revision { 0x112233 } else { 0x445566 }))
            .on_a11y_action(Action::Click, move |_, _, _| {
                actions.borrow_mut().push(revision);
            })
    }
}

#[crate::test]
fn deferred_represent_without_model_changes_uses_an_exact_submission(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let handle = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..Default::default()
                },
                |_, app| {
                    app.new(|_| ActionRoot {
                        revision: false,
                        actions: Rc::new(RefCell::new(Vec::new())),
                    })
                },
            )
        })
        .unwrap();
    let mut native = cx.test_window(handle.into());
    assert!(native.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    cx.run_until_parked();
    native.set_frame_delivery_mode(crate::PlatformFrameDeliveryMode::Deferred);
    let original = cx
        .update_window(handle.into(), |_, window, _| {
            assert!(!window.invalidator.is_dirty());
            window.needs_present.set(true);
            window.rendered_frame_revision()
        })
        .unwrap();
    for _ in 0..2 {
        assert!(native.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            ..Default::default()
        }));
        cx.run_until_parked();
    }
    let submission = native
        .pending_frame_submission()
        .expect("a deferred re-presentation must reach the exact submission path");
    assert!(submission.frame_generation() > original);
    assert!(submission.complete(PlatformWindowPresentOutcome::Submitted));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.rendered_frame_revision(),
            submission.frame_generation()
        );
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            Some(submission.frame_generation())
        );
        assert!(!window.needs_present.get());
    })
    .unwrap();
    assert!(!submission.complete(PlatformWindowPresentOutcome::Submitted));
}

#[crate::test]
fn older_accessibility_ingress_cannot_publish_a_later_submitted_candidate(cx: &mut TestAppContext) {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let handle = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..Default::default()
                },
                |_, app| {
                    app.new(|_| ActionRoot {
                        revision: false,
                        actions: actions.clone(),
                    })
                },
            )
        })
        .expect("ordinary initial window");
    let mut native = cx.test_window(handle.into());
    assert!(cx.activate_accessibility(handle.into()));
    assert!(native.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    cx.run_until_parked();
    let tree = native
        .latest_accessibility_tree_update()
        .expect("submitted action tree");
    let target = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("queued action"))
        .map(|(id, _)| *id)
        .expect("submitted action target");
    let submitted = cx
        .update_window(handle.into(), |_, window, _| {
            window.presentation_facts().present_submitted_generation
        })
        .unwrap();
    native.set_frame_delivery_mode(crate::PlatformFrameDeliveryMode::Deferred);
    let root = handle.root(cx).unwrap();
    root.update(cx, |root, cx| {
        root.revision = true;
        cx.notify();
    });
    assert!(native.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    cx.run_until_parked();
    let candidate = native
        .pending_frame_submission()
        .expect("original pending candidate");
    cx.update(|_| {
        assert!(native.dispatch_accessibility_action(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: target,
            data: None,
        }));
        assert!(candidate.complete(PlatformWindowPresentOutcome::Submitted));
    });
    cx.run_until_parked();
    assert_eq!(actions.borrow().as_slice(), &[false]);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            submitted
        );
        assert_eq!(Some(window.rendered_frame_revision()), submitted);
    })
    .unwrap();
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
}

#[cfg(feature = "profiler")]
#[crate::test]
fn submitted_candidate_consumes_its_dirty_samples_and_preserves_newer_demand(
    cx: &mut TestAppContext,
) {
    let handle = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..Default::default()
                },
                |_, app| {
                    app.new(|_| ActionRoot {
                        revision: false,
                        actions: Rc::new(RefCell::new(Vec::new())),
                    })
                },
            )
        })
        .unwrap();
    let mut native = cx.test_window(handle.into());
    assert!(native.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        window.invalidator.take_frame_dirty();
    })
    .unwrap();
    native.set_frame_delivery_mode(crate::PlatformFrameDeliveryMode::Deferred);
    let root = handle.root(cx).unwrap();
    root.update(cx, |root, cx| {
        root.revision = true;
        cx.notify();
    });
    assert!(native.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    cx.run_until_parked();
    let candidate = native.pending_frame_submission().unwrap();
    cx.update_window(handle.into(), |_, window, _| {
        let prepared = window.deferred_candidate.as_ref().unwrap();
        assert!(prepared.draw_timing.is_some());
        assert!(
            window.invalidator.pending_dirty_at().is_none(),
            "the prepared draw must own its consumed dirty accumulator"
        );
    })
    .unwrap();

    root.update(cx, |root, cx| {
        root.revision = false;
        cx.notify();
    });
    let newer_dirty_at = cx
        .update_window(handle.into(), |_, window, _| {
            window
                .invalidator
                .pending_dirty_at()
                .expect("new source demand")
        })
        .unwrap();
    cx.update(|_| assert!(candidate.complete(PlatformWindowPresentOutcome::Submitted)));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            Some(candidate.frame_generation())
        );
        let retained_dirty_at = window
            .deferred_candidate
            .as_ref()
            .and_then(|candidate| candidate.draw_timing.as_ref())
            .and_then(|(_, _, dirty)| dirty.dirty_at);
        assert_eq!(
            retained_dirty_at.or_else(|| window.invalidator.pending_dirty_at()),
            Some(newer_dirty_at),
            "the next frame must not inherit the submitted draw's earlier timestamp"
        );
        assert!(window.invalidator.is_dirty() || window.deferred_candidate.is_some());
    })
    .unwrap();
}
