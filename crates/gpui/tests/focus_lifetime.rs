#![cfg(feature = "test-support")]

use open_gpui::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, TestApp, WeakFocusHandle, Window, div, px,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

struct FocusTree {
    outer: FocusHandle,
    parent: FocusHandle,
    leaf: FocusHandle,
    depth: usize,
    lost_targets: Vec<Option<WeakFocusHandle>>,
    panic_once: bool,
    _subscription: Subscription,
}

impl FocusTree {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let outer = cx.focus_handle();
        let parent = cx.focus_handle();
        let leaf = cx.focus_handle();
        leaf.focus(window, cx);
        let subscription = cx.on_focus_lost(window, |this: &mut Self, window, cx| {
            let target = window.focus_lost_restore_target(cx);
            this.lost_targets
                .push(target.as_ref().map(FocusHandle::downgrade));
            assert!(
                !std::mem::take(&mut this.panic_once),
                "focus-lost observer failed"
            );
            if let Some(target) = target {
                target.focus(window, cx);
            }
        });
        Self {
            outer,
            parent,
            leaf,
            depth: 2,
            lost_targets: Vec::new(),
            panic_once: false,
            _subscription: subscription,
        }
    }
}

impl Render for FocusTree {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let child = (self.depth > 0).then(|| {
            div()
                .size(px(64.))
                .track_focus(&self.parent)
                .children((self.depth > 1).then(|| div().size(px(24.)).track_focus(&self.leaf)))
        });
        div().size_full().track_focus(&self.outer).children(child)
    }
}

#[test]
fn public_focus_lost_restores_the_nearest_surviving_ancestor() {
    let mut app = TestApp::new();
    let mut window = app.open_window(FocusTree::new);
    window.draw();
    window.update(|tree, window, cx| {
        assert_eq!(window.committed_focus(cx), Some(tree.leaf.clone()));
        assert!(window.focus_lost_restore_target(cx).is_none());
        tree.depth = 1;
        cx.notify();
    });
    window.draw();
    app.run_until_parked();
    window.draw();
    window.update(|tree, window, cx| {
        assert_eq!(window.committed_focus(cx), Some(tree.parent.clone()));
        assert_eq!(tree.lost_targets, [Some(tree.parent.downgrade())]);
        assert!(window.focus_lost_restore_target(cx).is_none());
        tree.depth = 0;
        cx.notify();
    });
    window.draw();
    app.run_until_parked();
    window.draw();
    window.update(|tree, window, cx| {
        assert_eq!(window.committed_focus(cx), Some(tree.outer.clone()));
        assert_eq!(
            tree.lost_targets,
            [Some(tree.parent.downgrade()), Some(tree.outer.downgrade())]
        );
        assert!(window.focus_lost_restore_target(cx).is_none());
    });
}

#[test]
fn public_focus_lost_scope_is_cleared_when_an_observer_unwinds() {
    let mut app = TestApp::new();
    let mut window = app.open_window(FocusTree::new);
    window.draw();
    let result = catch_unwind(AssertUnwindSafe(|| {
        window.update(|tree, _, cx| {
            tree.depth = 1;
            tree.panic_once = true;
            cx.notify();
        });
        window.draw();
    }));
    assert!(result.is_err());
    window.update(|tree, window, cx| {
        assert_eq!(tree.lost_targets, [Some(tree.parent.downgrade())]);
        assert!(window.focus_lost_restore_target(cx).is_none());
        tree.parent.focus(window, cx);
    });
    window.draw();
    window.update(|tree, window, cx| {
        assert_eq!(window.committed_focus(cx), Some(tree.parent.clone()));
        assert!(window.focus_lost_restore_target(cx).is_none());
    });
}
