//! Div regressions from Zed 635026fabc38440fce5e1767d125026a17b12012.
//!
//! Upstream source: crates/gpui/src/elements/div.rs.
//! - 83d4847462e57a6f72dc314abea05d57d3bdcfb6: descendant accessibility focus and keyboard pairing.
//! - 2268045a119030735f762e5afaf59da0bda869f4: tab-group container insertion.

use super::*;
use crate::{Context, InputEvent, Keystroke, TestAppContext, VisualTestContext};

struct KeyboardActivationTest {
    focus_a: FocusHandle,
    focus_b: FocusHandle,
    clicks: Vec<(&'static str, KeyboardButton)>,
    renders: usize,
    stopped_key: Option<&'static str>,
    stopped_events: usize,
    prevent_key_down: bool,
    prevent_key_up: bool,
}

impl Render for KeyboardActivationTest {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        div()
            .size_full()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.stopped_key == Some(event.keystroke.key.as_str()) {
                    this.stopped_events += 1;
                    cx.stop_propagation();
                }
                if this.prevent_key_down {
                    window.prevent_default();
                }
            }))
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                if this.stopped_key == Some(event.keystroke.key.as_str()) {
                    this.stopped_events += 1;
                    cx.stop_propagation();
                }
                if this.prevent_key_up {
                    window.prevent_default();
                }
            }))
            .child(
                div()
                    .id("a")
                    .size(px(50.))
                    .track_focus(&self.focus_a)
                    .on_click(
                        cx.listener(|this, event: &TargetedEvent<ClickEvent>, _, _| {
                            let ClickEvent::Keyboard(click) = event.window_event() else {
                                panic!("expected a keyboard click");
                            };
                            this.clicks.push(("a", click.button));
                        }),
                    ),
            )
            .child(
                div()
                    .id("b")
                    .size(px(50.))
                    .track_focus(&self.focus_b)
                    .on_click(
                        cx.listener(|this, event: &TargetedEvent<ClickEvent>, _, _| {
                            let ClickEvent::Keyboard(click) = event.window_event() else {
                                panic!("expected a keyboard click");
                            };
                            this.clicks.push(("b", click.button));
                        }),
                    ),
            )
    }
}

fn setup(cx: &mut TestAppContext) -> (Entity<KeyboardActivationTest>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| KeyboardActivationTest {
        focus_a: cx.focus_handle(),
        focus_b: cx.focus_handle(),
        clicks: Vec::new(),
        renders: 0,
        stopped_key: None,
        stopped_events: 0,
        prevent_key_down: false,
        prevent_key_up: false,
    });
    let focus_a = view.read_with(cx, |view, _| view.focus_a.clone());
    cx.update(|window, _| {
        let _ = window.activate_window();
    });
    focus_and_draw(cx, &focus_a);
    (view, cx)
}

fn focus_and_draw(cx: &mut VisualTestContext, handle: &FocusHandle) {
    cx.update(|window, cx| window.focus(handle, cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn key_down(cx: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).expect("valid test keystroke");
    cx.update(|window, cx| {
        window.dispatch_event(
            KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
    });
}

fn key_up(cx: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).expect("valid test keystroke");
    cx.update(|window, cx| {
        window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
    });
}

fn assert_clicks(
    view: &Entity<KeyboardActivationTest>,
    cx: &VisualTestContext,
    expected: &[(&'static str, KeyboardButton)],
) {
    view.read_with(cx, |view, _| assert_eq!(view.clicks, expected));
}

#[crate::test]
fn keyboard_activation_fires_once_across_notified_redraw(cx: &mut TestAppContext) {
    for (key, button) in [
        ("enter", KeyboardButton::Enter),
        ("space", KeyboardButton::Space),
    ] {
        let (view, cx) = setup(cx);
        key_down(cx, key);
        assert_clicks(&view, cx, &[]);

        let renders = view.read_with(cx, |view, _| view.renders);
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            view.read_with(cx, |view, _| view.renders) > renders,
            "notify must rebuild the view"
        );

        key_up(cx, key);
        key_up(cx, key);
        assert_clicks(&view, cx, &[("a", button)]);
    }
}

#[crate::test]
fn keyboard_activation_requires_key_down(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    key_up(cx, "enter");
    key_up(cx, "space");
    assert_clicks(&view, cx, &[]);
}

#[crate::test]
fn keyboard_activation_does_not_leak_across_focus_change(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let (view, cx) = setup(cx);
        key_down(cx, key);
        let focus_b = view.read_with(cx, |view, _| view.focus_b.clone());
        focus_and_draw(cx, &focus_b);
        key_up(cx, key);
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_does_not_leak_when_focus_returns(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let (view, cx) = setup(cx);
        key_down(cx, key);
        let focus_a = view.read_with(cx, |view, _| view.focus_a.clone());
        let focus_b = view.read_with(cx, |view, _| view.focus_b.clone());
        focus_and_draw(cx, &focus_b);
        focus_and_draw(cx, &focus_a);
        key_up(cx, key);
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_survives_same_focus_reassertion(cx: &mut TestAppContext) {
    for (key, button) in [
        ("enter", KeyboardButton::Enter),
        ("space", KeyboardButton::Space),
    ] {
        let (view, cx) = setup(cx);
        key_down(cx, key);
        let focus_a = view.read_with(cx, |view, _| view.focus_a.clone());
        let (generation, claim) =
            cx.update(|window, _| (window.focus_generation, window.focus_claim_revision()));
        focus_and_draw(cx, &focus_a);
        cx.update(|window, _| {
            assert_eq!(window.focus_generation, generation);
            assert_ne!(window.focus_claim_revision(), claim);
        });
        key_up(cx, key);
        assert_clicks(&view, cx, &[("a", button)]);
    }
}

#[crate::test]
fn keyboard_activation_cleared_by_intervening_key_press(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let (view, cx) = setup(cx);
        key_down(cx, key);
        key_down(cx, "escape");
        key_up(cx, key);
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_cleared_by_intervening_key_release(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let (view, cx) = setup(cx);
        key_down(cx, "escape");
        key_down(cx, key);
        key_up(cx, "escape");
        key_up(cx, key);
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_does_not_distinguish_space_and_enter(cx: &mut TestAppContext) {
    for (down, up, button) in [
        ("space", "enter", KeyboardButton::Enter),
        ("enter", "space", KeyboardButton::Space),
    ] {
        let (view, cx) = setup(cx);
        key_down(cx, down);
        key_up(cx, up);
        key_up(cx, up);
        assert_clicks(&view, cx, &[("a", button)]);
    }
}

#[crate::test]
fn keyboard_activation_requires_unmodified_press_and_release(cx: &mut TestAppContext) {
    for modifier in ["ctrl", "alt", "shift", "cmd", "fn"] {
        let (view, cx) = setup(cx);
        let modified = format!("{modifier}-enter");
        key_down(cx, &modified);
        key_up(cx, "enter");
        key_down(cx, "enter");
        key_up(cx, &modified);
        key_up(cx, "enter");
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_cleared_by_stopped_key_events(cx: &mut TestAppContext) {
    for release in [false, true] {
        let (view, cx) = setup(cx);
        view.update(cx, |view, _| view.stopped_key = Some("escape"));
        key_down(cx, "space");
        if release {
            key_up(cx, "escape");
        } else {
            key_down(cx, "escape");
        }
        key_up(cx, "space");
        assert_eq!(view.read_with(cx, |view, _| view.stopped_events), 1);
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_does_not_reuse_stopped_press_or_release(cx: &mut TestAppContext) {
    for stop_down in [false, true] {
        let (view, cx) = setup(cx);
        view.update(cx, |view, _| {
            view.stopped_key = stop_down.then_some("enter");
        });
        key_down(cx, "enter");
        view.update(cx, |view, _| {
            view.stopped_key = (!stop_down).then_some("enter");
        });
        key_up(cx, "enter");
        view.update(cx, |view, _| view.stopped_key = None);
        key_up(cx, "enter");
        assert_eq!(view.read_with(cx, |view, _| view.stopped_events), 1);
        assert_clicks(&view, cx, &[]);
    }
}

#[crate::test]
fn keyboard_activation_cleared_by_intercepted_key_press(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let intercepted = Rc::new(Cell::new(0));
    let _subscription = cx.update(|window, _| {
        let intercepted = intercepted.clone();
        window.intercept_window_key_down(move |event, _, cx| {
            if event.keystroke.key == "escape" {
                intercepted.set(intercepted.get() + 1);
                cx.stop_propagation();
            }
        })
    });
    key_down(cx, "enter");
    key_down(cx, "escape");
    key_up(cx, "enter");
    assert_eq!(intercepted.get(), 1);
    assert_clicks(&view, cx, &[]);
}

#[crate::test]
fn keyboard_activation_respects_prevent_default(cx: &mut TestAppContext) {
    for prevent_down in [false, true] {
        let (view, cx) = setup(cx);
        view.update(cx, |view, _| {
            view.prevent_key_down = prevent_down;
            view.prevent_key_up = !prevent_down;
        });
        key_down(cx, "enter");
        key_up(cx, "enter");
        assert_clicks(&view, cx, &[]);
    }
}

struct TabGroupFocus {
    group_a: FocusHandle,
    item_a: FocusHandle,
    group_b: FocusHandle,
    item_b: FocusHandle,
}

impl Render for TabGroupFocus {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        fn group(container: &FocusHandle, item: &FocusHandle) -> Div {
            div()
                .track_focus(container)
                .tab_group()
                .child(div().track_focus(item))
        }
        div()
            .child(group(&self.group_a, &self.item_a))
            .child(group(&self.group_b, &self.item_b))
    }
}

#[crate::test]
fn focus_next_from_tab_group_container_enters_that_group(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| TabGroupFocus {
        group_a: cx.focus_handle(),
        item_a: cx.focus_handle().tab_stop(true),
        group_b: cx.focus_handle(),
        item_b: cx.focus_handle().tab_stop(true),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let group_b = view.read(cx).group_b.clone();
        window.focus(&group_b, cx);
        window.focus_next(cx);
        assert_eq!(
            window.focused(cx).map(|handle| handle.id),
            Some(view.read(cx).item_b.id)
        );
    });
}
