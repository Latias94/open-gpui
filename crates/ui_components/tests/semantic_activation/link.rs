use super::*;

#[open_gpui::test]
fn link_accepts_enter_rejects_space_and_preserves_typed_navigation_payload(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<(String, ActivationSource)>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            div().size_full().child(
                Link::new("semantic-link", "Documentation", "/docs").on_activate(
                    move |payload, activation, _, _| {
                        activations
                            .borrow_mut()
                            .push((payload.href().to_owned(), activation.source()));
                    },
                ),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let bounds = cx
        .debug_bounds("link:semantic-link:root")
        .expect("Link should expose a stable root selector");
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert_eq!(
        activations.borrow().as_slice(),
        &[("/docs".to_owned(), ActivationSource::Pointer)]
    );

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Link should publish a final accessibility tree");
    let link_node = node_with_label(&update, "Documentation");
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, link_node,)));

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_down.propagated());
    assert!(!space_down.default_prevented());
    assert!(space_up.propagated());
    assert!(!space_up.default_prevented());
    assert_eq!(activations.borrow().len(), 1);

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert_eq!(activations.borrow().len(), 1);
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ("/docs".to_owned(), ActivationSource::Pointer),
            (
                "/docs".to_owned(),
                ActivationSource::Keyboard(ActivationKey::Enter),
            ),
        ]
    );

    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, link_node,)));
    assert_eq!(
        activations.borrow().last(),
        Some(&("/docs".to_owned(), ActivationSource::Accessibility))
    );
}
