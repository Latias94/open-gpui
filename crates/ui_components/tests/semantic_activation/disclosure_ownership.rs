use super::*;

use open_gpui_ui_components::{Accordion, AccordionItem, Collapsible};

fn expanded_state(update: &accesskit::TreeUpdate, label: &str) -> Option<bool> {
    let id = node_with_label(update, label);
    update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then(|| node.is_expanded()))
        .flatten()
}

#[open_gpui::test]
fn uncontrolled_disclosures_commit_without_requiring_callbacks(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        accordion_handle: ActivationHandle,
        collapsible_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .child(
                    Accordion::new("uncontrolled-accordion")
                        .collapsible(true)
                        .item(AccordionItem::new(
                            "details",
                            "Uncontrolled accordion",
                            "Accordion content",
                        ))
                        .activation_handle("details", &self.accordion_handle),
                )
                .child(
                    Collapsible::new("uncontrolled-collapsible", "Uncontrolled disclosure")
                        .content("Disclosure content")
                        .activation_handle(&self.collapsible_handle),
                )
        }
    }

    let accordion_handle = ActivationHandle::new();
    let collapsible_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        accordion_handle: accordion_handle.clone(),
        collapsible_handle: collapsible_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("uncontrolled disclosures should publish a final tree");
    assert_eq!(
        expanded_state(&update, "Uncontrolled accordion"),
        Some(false)
    );
    assert_eq!(
        expanded_state(&update, "Uncontrolled disclosure"),
        Some(false)
    );

    cx.update(|window, cx| {
        for _ in 0..2 {
            assert_eq!(
                accordion_handle.request(window, cx),
                ActivationRequestResult::Dispatched
            );
            assert_eq!(
                collapsible_handle.request(window, cx),
                ActivationRequestResult::Dispatched
            );
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("same-frame double activation should commit both transitions");
    assert_eq!(
        expanded_state(&update, "Uncontrolled accordion"),
        Some(false)
    );
    assert_eq!(
        expanded_state(&update, "Uncontrolled disclosure"),
        Some(false)
    );

    cx.update(|window, cx| {
        assert_eq!(
            accordion_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            collapsible_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("uncontrolled commits should reach the final tree");
    assert_eq!(
        expanded_state(&update, "Uncontrolled accordion"),
        Some(true)
    );
    assert_eq!(
        expanded_state(&update, "Uncontrolled disclosure"),
        Some(true)
    );

    cx.update(|window, cx| {
        assert_eq!(
            accordion_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            collapsible_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("second uncontrolled commits should reach the final tree");
    assert_eq!(
        expanded_state(&update, "Uncontrolled accordion"),
        Some(false)
    );
    assert_eq!(
        expanded_state(&update, "Uncontrolled disclosure"),
        Some(false)
    );
}

#[open_gpui::test]
fn uncontrolled_callback_reentry_observes_the_committed_runtime(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        changes: Rc<RefCell<Vec<bool>>>,
        reentered: Rc<Cell<bool>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let reentered = self.reentered.clone();
            let handle = self.handle.clone();
            Collapsible::new("reentrant-collapsible", "Reentrant disclosure")
                .activation_handle(&self.handle)
                .on_open_change(move |open, window, cx| {
                    changes.borrow_mut().push(open);
                    if !reentered.replace(true) {
                        assert_eq!(
                            handle.request(window, cx),
                            ActivationRequestResult::Dispatched
                        );
                    }
                })
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let reentered = Rc::new(Cell::new(false));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        reentered: reentered.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(changes.borrow().as_slice(), &[true, false]);
    assert!(reentered.get());

    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("reentrant commit should reach the final tree");
    assert_eq!(expanded_state(&update, "Reentrant disclosure"), Some(false));
}
