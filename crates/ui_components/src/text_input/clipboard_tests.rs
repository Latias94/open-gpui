use std::{
    cell::RefCell,
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
};

use open_gpui::{ClipboardReadError, InputHandler, Task, TestAppContext, VisualTestContext};

use super::*;

type ClipboardReadResult = Result<Option<ClipboardItem>, ClipboardReadError>;

struct ClipboardTestView {
    controller: Option<Entity<TextInputController>>,
    sibling: Option<Entity<TextInputController>>,
    disabled: bool,
    read_only: bool,
    changes: Rc<RefCell<Vec<String>>>,
    sibling_changes: Rc<RefCell<Vec<String>>>,
}

impl Render for ClipboardTestView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input = self.controller.as_ref().map(|controller| {
            let changes = self.changes.clone();
            TextInput::new("clipboard", "Clipboard")
                .value(controller.read(cx).value().to_owned())
                .controller(controller.clone())
                .disabled(self.disabled)
                .read_only(self.read_only)
                .on_change(move |value, _, _| changes.borrow_mut().push(value))
        });
        let sibling = self.sibling.as_ref().map(|controller| {
            let changes = self.sibling_changes.clone();
            TextInput::new("clipboard-sibling", "Sibling")
                .value(controller.read(cx).value().to_owned())
                .controller(controller.clone())
                .on_change(move |value, _, _| changes.borrow_mut().push(value))
        });
        div().size_full().children(input).children(sibling)
    }
}

fn clipboard_test_view(
    cx: &mut TestAppContext,
) -> (
    Entity<ClipboardTestView>,
    Entity<TextInputController>,
    &mut VisualTestContext,
) {
    cx.update(adapter::init);
    let controller = cx.new(|cx| TextInputController::with_value("a🙂z", cx));
    let (view, cx) = cx.add_window_view(|_, _| ClipboardTestView {
        controller: Some(controller.clone()),
        sibling: None,
        disabled: false,
        read_only: false,
        changes: Rc::default(),
        sibling_changes: Rc::default(),
    });
    cx.update(|window, cx| {
        window.focus(&controller.focus_handle(cx), cx);
        window.draw(cx).clear(cx);
    });
    (view, controller, cx)
}

fn start_pending_paste(cx: &mut VisualTestContext) -> impl FnOnce(ClipboardReadResult) + use<> {
    let completion = Rc::new(RefCell::new((None::<ClipboardReadResult>, None::<Waker>)));
    let pending = completion.clone();
    let read = cx.foreground_executor.spawn(poll_fn(move |cx| {
        let mut pending = pending.borrow_mut();
        if let Some(result) = pending.0.take() {
            Poll::Ready(result)
        } else {
            pending.1 = Some(cx.waker().clone());
            Poll::Pending
        }
    }));
    cx.set_next_clipboard_read(read);
    cx.dispatch_action(Paste);

    let fallback = cx.to_async().update(|cx| cx.read_from_clipboard_async());
    assert!(
        fallback.is_ready(),
        "the Paste action must consume the configured pending read"
    );

    move |result| {
        let waker = {
            let mut completion = completion.borrow_mut();
            completion.0 = Some(result);
            completion.1.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

#[open_gpui::test]
fn desktop_ready_paste_keeps_the_action_time_clipboard_snapshot(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    cx.update_entity(&controller, |controller, cx| {
        controller.select_range(1.."a🙂".len(), cx);
    });
    cx.write_to_clipboard(ClipboardItem::new_string("left\nright".into()));

    cx.update(|window, cx| {
        window.dispatch_action(Box::new(Paste), cx);
        cx.defer(|cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("later clipboard".into()));
        });
    });
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "aleft rightz");
        assert_eq!(controller.selected_range(), 11..11);
        assert_eq!(controller.user_edit_revision(), 1);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.changes.borrow().as_slice(), ["aleft rightz"]);
    });
}

#[open_gpui::test]
fn pending_paste_does_not_edit_a_blurred_but_rendered_input(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    let sibling = cx.new(|cx| TextInputController::with_value("sibling", cx));
    cx.update_window_entity(&view, |view, _, cx| {
        view.sibling = Some(sibling.clone());
        cx.notify();
    });
    cx.update_entity(&controller, |controller, cx| {
        controller.select_range(1..5, cx)
    });
    cx.update_entity(&sibling, |controller, cx| controller.select_range(1..3, cx));
    let complete = start_pending_paste(cx);
    cx.update(|window, cx| {
        window.focus(&sibling.focus_handle(cx), cx);
        window.draw(cx).clear(cx);
        let original_focus = controller.focus_handle(cx);
        let sibling_focus = sibling.focus_handle(cx);
        assert!(window.is_focus_handle_rendered(&original_focus));
        assert!(window.is_focus_handle_rendered(&sibling_focus));
        assert!(!original_focus.is_focused(window));
        assert!(sibling_focus.is_focused(window));
    });

    complete(Ok(Some(ClipboardItem::new_string("late paste".into()))));
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.selected_range(), 1..5);
        assert_eq!(controller.user_edit_revision(), 0);
    });
    sibling.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "sibling");
        assert_eq!(controller.selected_range(), 1..3);
        assert_eq!(controller.user_edit_revision(), 0);
    });
    view.read_with(cx, |view, _| {
        assert!(view.changes.borrow().is_empty());
        assert!(view.sibling_changes.borrow().is_empty());
    });
}

#[open_gpui::test]
fn pending_paste_uses_the_selection_at_delivery_and_notifies_once(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    cx.update_entity(&controller, |controller, cx| {
        controller.select_range(1.."a🙂".len(), cx);
    });
    let complete = start_pending_paste(cx);

    cx.update_entity(&controller, |controller, cx| {
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.user_edit_revision(), 0);
        controller.select_range(0..1, cx);
    });
    complete(Ok(Some(ClipboardItem::new_string("X\nY".into()))));
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "X Y🙂z");
        assert_eq!(controller.selected_range(), 3..3);
        assert_eq!(controller.user_edit_revision(), 1);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.changes.borrow().as_slice(), ["X Y🙂z"]);
    });
}

#[open_gpui::test]
fn pending_paste_does_not_edit_a_disabled_input(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    let complete = start_pending_paste(cx);
    view.update(cx, |view, cx| {
        view.disabled = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });

    complete(Ok(Some(ClipboardItem::new_string("blocked".into()))));
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert!(controller.disabled());
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.selected_range(), "a🙂z".len().."a🙂z".len());
        assert_eq!(controller.user_edit_revision(), 0);
    });
    view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
}

#[open_gpui::test]
fn pending_paste_does_not_edit_a_read_only_input(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    let complete = start_pending_paste(cx);
    view.update(cx, |view, cx| {
        view.read_only = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });

    complete(Ok(Some(ClipboardItem::new_string("blocked".into()))));
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert!(controller.read_only());
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.user_edit_revision(), 0);
    });
    view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
}

#[open_gpui::test]
fn pending_paste_does_not_keep_a_released_controller_alive(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    let complete = start_pending_paste(cx);
    let weak = controller.downgrade();
    drop(controller);
    view.update(cx, |view, cx| {
        view.controller = None;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());

    complete(Ok(Some(ClipboardItem::new_string("released".into()))));
    cx.run_until_parked();

    assert!(weak.upgrade().is_none());
    view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
}

#[open_gpui::test]
fn pending_paste_does_not_edit_a_replaced_but_live_controller(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    let complete = start_pending_paste(cx);
    let replacement = cx.new(|cx| TextInputController::with_value("replacement", cx));
    view.update(cx, |view, cx| {
        view.controller = Some(replacement.clone());
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert!(!window.is_focus_handle_rendered(&controller.focus_handle(cx)));
    });

    complete(Ok(Some(ClipboardItem::new_string("stale".into()))));
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.user_edit_revision(), 0);
    });
    replacement.read_with(cx, |replacement, _| {
        assert_eq!(replacement.value(), "replacement");
        assert_eq!(replacement.user_edit_revision(), 0);
    });
    view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
}

#[open_gpui::test]
fn pending_paste_does_not_deliver_to_a_closed_window(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    let complete = start_pending_paste(cx);
    cx.update(|window, cx| window.remove_window(cx));

    complete(Ok(Some(ClipboardItem::new_string("closed".into()))));
    cx.run_until_parked();

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.user_edit_revision(), 0);
    });
    view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
}

#[open_gpui::test]
fn clipboard_read_errors_leave_selection_unchanged_and_allow_retry(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    cx.update_entity(&controller, |controller, cx| {
        controller.select_range(1.."a🙂".len(), cx);
    });

    for error in [
        ClipboardReadError::Unavailable,
        ClipboardReadError::Denied("test denial".into()),
        ClipboardReadError::UnsupportedContent,
    ] {
        let complete = start_pending_paste(cx);
        complete(Err(error));
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert_eq!(controller.value(), "a🙂z");
            assert_eq!(controller.selected_range(), 1.."a🙂".len());
            assert_eq!(controller.user_edit_revision(), 0);
        });
        view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
    }

    cx.write_to_clipboard(ClipboardItem::new_string("retry".into()));
    cx.dispatch_action(Paste);
    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "aretryz");
        assert_eq!(controller.user_edit_revision(), 1);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.changes.borrow().as_slice(), ["aretryz"]);
    });
}

#[open_gpui::test]
fn empty_clipboard_read_keeps_the_selected_text(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    cx.update_entity(&controller, |controller, cx| {
        controller.select_range(1.."a🙂".len(), cx);
    });
    cx.set_next_clipboard_read(Task::ready(Ok(None)));
    cx.dispatch_action(Paste);

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "a🙂z");
        assert_eq!(controller.selected_range(), 1.."a🙂".len());
        assert_eq!(controller.user_edit_revision(), 0);
    });
    view.read_with(cx, |view, _| assert!(view.changes.borrow().is_empty()));
}

#[open_gpui::test]
fn platform_paste_uses_the_same_single_line_edit_and_disabled_guard(cx: &mut TestAppContext) {
    let (view, controller, cx) = clipboard_test_view(cx);
    cx.update_entity(&controller, |controller, cx| {
        controller.select_range(1.."a🙂".len(), cx);
    });
    cx.update(|window, cx| {
        let mut handler = ElementInputHandler::new(Bounds::default(), controller.clone());
        InputHandler::paste(
            &mut handler,
            ClipboardItem::new_string("DOM\npaste".into()),
            window,
            cx,
        );
        controller.update(cx, |controller, cx| controller.set_disabled(true, cx));
        InputHandler::paste(
            &mut handler,
            ClipboardItem::new_string("blocked".into()),
            window,
            cx,
        );
    });

    controller.read_with(cx, |controller, _| {
        assert_eq!(controller.value(), "aDOM pastez");
        assert_eq!(controller.selected_range(), 10..10);
        assert_eq!(controller.user_edit_revision(), 1);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.changes.borrow().as_slice(), ["aDOM pastez"]);
    });
}
