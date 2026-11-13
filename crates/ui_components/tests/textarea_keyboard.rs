use std::{
    cell::RefCell,
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
};

use open_gpui::{
    ClipboardItem, ClipboardReadError, Context, Entity, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, Keystroke, ParentElement, Render, Styled, TestAppContext,
    VisualContext, VisualTestContext, Window, div, px,
};
use open_gpui_ui_components::{Textarea, gpui_adapter::init_text_input};

struct TextareaProbe {
    value: String,
    peer: String,
    changes: Vec<String>,
    disabled: bool,
    read_only: bool,
    visible: bool,
    accept_changes: bool,
    copy_bubbles: usize,
}

impl Render for TextareaProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let primary = self.visible.then(|| {
            Textarea::new("keyboard-notes", "Notes")
                .value(self.value.clone())
                .rows(4)
                .disabled(self.disabled)
                .read_only(self.read_only)
                .on_change(cx.processor(|this, value: String, _, cx| {
                    if this.accept_changes {
                        this.value = value.clone();
                    }
                    this.changes.push(value);
                    cx.notify();
                }))
        });
        div()
            .flex()
            .flex_col()
            .w(px(400.))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, _| {
                if event.keystroke.key == "c" && event.keystroke.modifiers.control {
                    this.copy_bubbles += 1;
                }
            }))
            .children(primary)
            .child(
                Textarea::new("keyboard-peer", "Peer")
                    .value(self.peer.clone())
                    .rows(2)
                    .on_change(cx.processor(|this, value: String, _, cx| {
                        this.peer = value;
                        cx.notify();
                    })),
            )
    }
}

fn mount<'context>(
    cx: &'context mut TestAppContext,
    initial: &str,
) -> (Entity<TextareaProbe>, &'context mut VisualTestContext) {
    cx.update(init_text_input);
    let (view, cx) = cx.add_window_view(|_, _| TextareaProbe {
        value: initial.into(),
        peer: String::new(),
        changes: Vec::new(),
        disabled: false,
        read_only: false,
        visible: true,
        accept_changes: true,
        copy_bubbles: 0,
    });
    cx.update(|window, cx| {
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
    focus(cx, "keyboard-notes");
    (view, cx)
}

fn focus(cx: &mut VisualTestContext, id: &str) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let bounds = cx
        .debug_bounds(&format!("textarea:{id}:root"))
        .expect("the textarea should have accepted control geometry");
    cx.simulate_click(bounds.center(), Default::default());
}

fn press(cx: &mut VisualTestContext, source: &'static str) {
    let mut keystroke = Keystroke::parse(source).expect("valid editing keystroke");
    keystroke.key_char = None;
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
}

fn press_keys(cx: &mut VisualTestContext, sources: &'static str) {
    for source in sources.split_whitespace() {
        press(cx, source);
    }
}

fn settle_frames(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

fn assert_caret_visible(cx: &mut VisualTestContext) {
    settle_frames(cx);
    let viewport = cx.debug_bounds("textarea:keyboard-notes:root").unwrap();
    let window = cx.window_handle();
    let caret = cx
        .with_window_input_handler(window, |handler| {
            let selection = handler.selected_text_range(false).unwrap();
            let cursor = if selection.reversed {
                selection.range.start
            } else {
                selection.range.end
            };
            handler.bounds_for_range(cursor..cursor).unwrap()
        })
        .unwrap();
    assert!(
        caret.top() >= viewport.top(),
        "caret above viewport: {caret:?}, {viewport:?}"
    );
    assert!(
        caret.bottom() <= viewport.bottom(),
        "caret below viewport: {caret:?}, {viewport:?}"
    );
}

#[open_gpui::test]
fn textarea_scroll_follows_keyboard_selection_and_multiline_input(cx: &mut TestAppContext) {
    let initial = (0..30)
        .map(|index| format!("Line {index}\n"))
        .collect::<String>();
    let (view, cx) = mount(cx, &initial);
    press(cx, "ctrl-end");
    assert_caret_visible(cx);
    press(cx, "ctrl-home");
    assert_caret_visible(cx);
    press(cx, "ctrl-shift-end");
    assert_caret_visible(cx);
    press_keys(cx, "up up up up up up");
    assert_caret_visible(cx);
    press_keys(cx, "ctrl-end enter");
    cx.simulate_input("追加");
    assert_caret_visible(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, format!("{initial}\n追加"))
    });
}

#[open_gpui::test]
fn textarea_keyboard_edits_newlines_and_whole_graphemes(cx: &mut TestAppContext) {
    let initial = "A👨‍👩‍👧‍👦B";
    let (view, cx) = mount(cx, initial);
    press_keys(cx, "ctrl-end enter");
    view.read_with(cx, |view, _| assert_eq!(view.value, format!("{initial}\n")));
    cx.simulate_input("中");
    press_keys(
        cx,
        "backspace backspace home right delete shift-right backspace",
    );
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "A");
        assert_eq!(
            view.changes,
            [
                format!("{initial}\n"),
                format!("{initial}\n中"),
                format!("{initial}\n"),
                initial.to_owned(),
                "AB".to_owned(),
                "A".to_owned(),
            ]
        );
        assert!(view.peer.is_empty());
    });
}

#[open_gpui::test]
fn textarea_vertical_navigation_retains_column_and_directional_selection(cx: &mut TestAppContext) {
    let initial = "MMMMMM\nM\nMMMMMM\n尾";
    let (view, cx) = mount(cx, initial);
    press_keys(
        cx,
        "ctrl-home right right right right right down down shift-home ctrl-c",
    );
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("MMMMM")
    );
    press_keys(cx, "home shift-end ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("MMMMMM")
    );
    press_keys(cx, "home ctrl-shift-end ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("MMMMMM\n尾")
    );
    press_keys(cx, "ctrl-home shift-down shift-down ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("MMMMMM\nM\n")
    );
    press_keys(cx, "shift-up ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("MMMMMM\n")
    );
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, initial);
        assert!(view.changes.is_empty());
    });
}

#[open_gpui::test]
fn textarea_keyboard_copies_cuts_and_pastes_multiline_text_once(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "left\nright");
    press_keys(cx, "ctrl-a ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("left\nright")
    );
    press(cx, "ctrl-x");
    view.read_with(cx, |view, _| assert!(view.value.is_empty()));
    cx.write_to_clipboard(ClipboardItem::new_string("First\r\n第二行\rThird".into()));
    press(cx, "ctrl-v");
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "First\n第二行\nThird");
        assert_eq!(view.changes, ["", "First\n第二行\nThird"]);
    });
    press_keys(cx, "ctrl-shift-home shift-right");
    cx.simulate_input("!");
    view.read_with(cx, |view, _| assert_eq!(view.value, "F!"));
}

#[open_gpui::test]
fn textarea_read_only_allows_copy_and_disabled_blocks_keyboard_changes(cx: &mut TestAppContext) {
    let initial = "locked\n文本";
    let (view, cx) = mount(cx, initial);
    cx.update_window_entity(&view, |view, _, cx| {
        view.read_only = true;
        cx.notify();
    });
    press_keys(cx, "ctrl-a ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some(initial)
    );
    press_keys(cx, "enter backspace delete ctrl-x ctrl-v");
    cx.simulate_input("blocked");
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, initial);
        assert!(view.changes.is_empty());
    });
    cx.update_window_entity(&view, |view, _, cx| {
        view.read_only = false;
        view.disabled = true;
        cx.notify();
    });
    cx.write_to_clipboard(ClipboardItem::new_string("unchanged clipboard".into()));
    press_keys(cx, "left ctrl-a ctrl-c enter delete ctrl-v");
    cx.run_until_parked();
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("unchanged clipboard")
    );
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, initial);
        assert!(view.changes.is_empty());
    });
}

#[open_gpui::test]
fn textarea_keyboard_preserves_ime_mark_until_original_commit(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "");
    cx.simulate_marked_text(None, "ni", Some(1..2));
    press_keys(cx, "enter backspace delete left shift-end ctrl-a");
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "ni");
        assert_eq!(view.changes, ["ni"]);
    });
    cx.simulate_marked_text(None, "你", Some(1..1));
    cx.simulate_input("你");
    press(cx, "enter");
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "你\n");
        assert_eq!(view.changes, ["ni", "你", "你\n"]);
    });
}

type ClipboardResult = Result<Option<ClipboardItem>, ClipboardReadError>;

fn pending_paste(cx: &mut VisualTestContext) -> impl FnOnce(ClipboardResult) + use<> {
    let completion = Rc::new(RefCell::new((None::<ClipboardResult>, None::<Waker>)));
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
    press(cx, "ctrl-v");
    assert!(
        cx.to_async()
            .update(|cx| cx.read_from_clipboard_async())
            .is_ready()
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
fn textarea_late_paste_cannot_edit_a_peer_or_an_unmounted_control(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "original");
    let complete = pending_paste(cx);
    focus(cx, "keyboard-peer");
    complete(Ok(Some(ClipboardItem::new_string("late paste".into()))));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "original");
        assert!(view.peer.is_empty());
        assert!(view.changes.is_empty());
    });
    focus(cx, "keyboard-notes");
    let complete = pending_paste(cx);
    cx.update_window_entity(&view, |view, window, cx| {
        view.visible = false;
        cx.notify();
        window.refresh();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    complete(Ok(Some(ClipboardItem::new_string(
        "unmounted paste".into(),
    ))));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "original");
        assert!(view.peer.is_empty());
        assert!(view.changes.is_empty());
    });
}

#[open_gpui::test]
fn textarea_pending_paste_rechecks_read_only_state_on_return(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "original");
    let complete = pending_paste(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.read_only = true;
        cx.notify();
    });
    complete(Ok(Some(ClipboardItem::new_string("blocked paste".into()))));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.changes.is_empty()));
    settle_frames(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "original");
        assert!(view.changes.is_empty());
    });
}

#[open_gpui::test]
fn textarea_paste_cancels_when_selection_changes_even_if_it_returns(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "left\nright");
    press_keys(cx, "ctrl-home shift-right");
    let complete = pending_paste(cx);
    press_keys(cx, "shift-right shift-left");
    complete(Ok(Some(ClipboardItem::new_string("stale".into()))));
    settle_frames(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "left\nright");
        assert!(view.changes.is_empty());
    });
}

#[open_gpui::test]
fn textarea_paste_cancels_when_user_types_or_owner_replaces_text(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "original");
    press(cx, "ctrl-end");
    let complete = pending_paste(cx);
    cx.simulate_input("!");
    complete(Ok(Some(ClipboardItem::new_string("stale".into()))));
    settle_frames(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "original!");
        assert_eq!(view.changes, ["original!"]);
    });
    let complete = pending_paste(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.value = "replacement".into();
        cx.notify();
    });
    complete(Ok(Some(ClipboardItem::new_string("stale owner".into()))));
    settle_frames(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "replacement");
        assert_eq!(view.changes, ["original!"]);
    });
}

#[open_gpui::test]
fn textarea_ready_paste_waits_for_current_owner_frame(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "original");
    press(cx, "ctrl-end");
    let complete = pending_paste(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.disabled = true;
        cx.notify();
    });
    complete(Ok(Some(ClipboardItem::new_string("disabled paste".into()))));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.changes.is_empty()));
    settle_frames(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.disabled = false;
        cx.notify();
    });
    settle_frames(cx);
    focus(cx, "keyboard-notes");
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "original");
        assert!(view.changes.is_empty());
    });
    let complete = pending_paste(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.visible = false;
        cx.notify();
    });
    complete(Ok(Some(ClipboardItem::new_string(
        "unmounted paste".into(),
    ))));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.changes.is_empty()));
    settle_frames(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.visible = true;
        cx.notify();
    });
    focus(cx, "keyboard-notes");
    settle_frames(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "original");
        assert!(view.changes.is_empty());
    });
}

#[open_gpui::test]
fn textarea_paste_cancels_when_ime_composition_starts(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "");
    let complete = pending_paste(cx);
    cx.simulate_marked_text(None, "ni", Some(1..2));
    complete(Ok(Some(ClipboardItem::new_string("stale paste".into()))));
    settle_frames(cx);
    cx.simulate_input("你");
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "你");
        assert_eq!(view.changes, ["ni", "你"]);
    });
}

#[open_gpui::test]
fn textarea_pending_paste_commits_once_on_unchanged_accepted_selection(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "left\nright");
    press_keys(cx, "ctrl-home shift-end");
    let complete = pending_paste(cx);
    complete(Ok(Some(ClipboardItem::new_string("替换\n正文".into()))));
    settle_frames(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "替换\n正文\nright");
        assert_eq!(view.changes, ["替换\n正文\nright"]);
    });
}

#[open_gpui::test]
fn textarea_copy_yields_to_parent_during_ime_and_when_disabled(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "");
    cx.write_to_clipboard(ClipboardItem::new_string("unchanged".into()));
    cx.simulate_marked_text(None, "ni", Some(0..2));
    press(cx, "ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("unchanged")
    );
    view.read_with(cx, |view, _| assert_eq!(view.copy_bubbles, 1));
    cx.simulate_input("你");
    press(cx, "ctrl-a");
    cx.update_window_entity(&view, |view, _, cx| {
        view.disabled = true;
        cx.notify();
    });
    press(cx, "ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("unchanged")
    );
    view.read_with(cx, |view, _| assert_eq!(view.copy_bubbles, 2));
}

#[open_gpui::test]
fn textarea_controlled_rejection_preserves_next_navigation_and_edit(cx: &mut TestAppContext) {
    let (view, cx) = mount(cx, "ab\ncd");
    cx.update_window_entity(&view, |view, _, cx| {
        view.accept_changes = false;
        cx.notify();
    });
    press(cx, "ctrl-end");
    cx.simulate_input("!");
    settle_frames(cx);
    cx.update_window_entity(&view, |view, _, cx| {
        view.accept_changes = true;
        cx.notify();
    });
    press_keys(cx, "home right enter");
    view.read_with(cx, |view, _| {
        assert_eq!(view.value, "ab\nc\nd");
        assert_eq!(view.changes, ["ab\ncd!", "ab\nc\nd"]);
    });
}
