use super::*;

#[open_gpui::test]
fn activation_handle_rejects_requests_from_another_window(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        handle: ActivationHandle,
        activations: Rc<Cell<usize>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Button::new("owned-window-button", "Owned")
                .on_activate(move |_, _, _| activations.set(activations.get() + 1))
                .activation_handle(&self.handle)
        }
    }

    struct EmptyView;

    impl Render for EmptyView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    let handle = ActivationHandle::new();
    let activations = Rc::new(Cell::new(0));
    let owner = cx.add_window({
        let handle = handle.clone();
        let activations = activations.clone();
        move |_, _| Probe {
            handle,
            activations,
        }
    });
    let foreign = cx.add_window(|_, _| EmptyView);
    let unbound_handle = ActivationHandle::new();
    foreign
        .update(cx, |_, window, cx| {
            assert_eq!(
                unbound_handle.request(window, cx),
                ActivationRequestResult::Unavailable
            );
        })
        .expect("fresh handles should be observable as unavailable");
    cx.update_window(owner.clone().into(), |_, window, cx| {
        window.draw(cx).clear()
    })
    .expect("owner window should draw");
    foreign
        .update(cx, |_, window, cx| {
            assert_eq!(
                handle.request(window, cx),
                ActivationRequestResult::WrongWindow
            );
        })
        .expect("foreign window should remain available");
    assert_eq!(activations.get(), 0);
    owner
        .update(cx, |_, window, cx| {
            assert_eq!(
                handle.request(window, cx),
                ActivationRequestResult::Dispatched
            );
        })
        .expect("owner window should accept its handle");
    assert_eq!(activations.get(), 1);
}
