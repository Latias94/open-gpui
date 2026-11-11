//! Input-preference regressions from Zed 86c20cc911, as of
//! 635026fabc38440fce5e1767d125026a17b12012 (crates/gpui/src/key_dispatch.rs).
//! Normal AltGr dispatch already preferred text; the new event hint exposes that policy.

use super::*;
use crate::{TestApp, TestAppWindow, TestInputDispatchSnapshot, UTF16Selection, canvas, div};
use std::{cell::RefCell, ops::Range, rc::Rc};

crate::actions!(input_preference_regression, [TestAction, EnableTextInput]);

type Preferences = Rc<RefCell<Vec<(InputPreference, Option<&'static str>)>>>;

#[derive(Clone)]
struct PendingTextInputTestView {
    focus_handle: FocusHandle,
    accepts_text_input: Rc<Cell<bool>>,
    text: Rc<RefCell<String>>,
    actions: Rc<RefCell<Vec<&'static str>>>,
    intercepted: Preferences,
    observed: Preferences,
}

impl Render for PendingTextInputTestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let input = self.clone();
        let actions = self.actions.clone();
        let replay_actions = self.actions.clone();
        let accepts_text_input = self.accepts_text_input.clone();
        div()
            .size_full()
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .on_action(move |_: &TestAction, _, _| actions.borrow_mut().push(TestAction.name()))
            .on_action(move |_: &EnableTextInput, _, _| {
                replay_actions.borrow_mut().push(EnableTextInput.name());
                accepts_text_input.set(true);
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), window, cx| {
                        window.handle_input(&input.focus_handle, input.clone(), cx);
                    },
                )
                .size_full(),
            )
    }
}

impl InputHandler for PendingTextInputTestView {
    fn accepts_text_input(&mut self, _: &mut Window, _: &mut App) -> bool {
        self.accepts_text_input.get()
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut App,
    ) {
        assert!(range.is_none());
        self.text.borrow_mut().push_str(text);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
        unreachable!("these tests do not start IME composition");
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }
}

fn setup_altgr_test() -> (
    TestApp,
    TestAppWindow<PendingTextInputTestView>,
    PendingTextInputTestView,
) {
    let mut app = TestApp::new();
    app.update(|cx| cx.bind_keys([KeyBinding::new("ctrl-alt-a", TestAction, Some("Terminal"))]));
    let mut window = app.open_window(|window, cx| {
        let input = PendingTextInputTestView {
            focus_handle: cx.focus_handle(),
            accepts_text_input: Rc::new(Cell::new(true)),
            text: Rc::default(),
            actions: Rc::default(),
            intercepted: Rc::default(),
            observed: Rc::default(),
        };
        window.focus(&input.focus_handle, cx);
        let _ = window.activate_window();
        input
    });
    window.draw();
    let input = window.read(|input, _| input.clone());
    app.update(|cx| {
        let intercepted = input.intercepted.clone();
        cx.intercept_keystrokes(move |event, _, _| {
            intercepted.borrow_mut().push((
                event.input_preference,
                event.action.as_ref().map(|a| a.name()),
            ));
        })
        .detach();
        let observed = input.observed.clone();
        cx.observe_keystrokes(move |event, _, _| {
            observed.borrow_mut().push((
                event.input_preference,
                event.action.as_ref().map(|a| a.name()),
            ));
        })
        .detach();
    });
    (app, window, input)
}

fn altgr_key_down(prefer_character_input: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            key_char: Some("ą".into()),
            ..Keystroke::parse("ctrl-alt-a").expect("valid keystroke")
        },
        is_held: false,
        prefer_character_input,
    }
}

fn dispatch_altgr_key_down(
    window: &mut TestAppWindow<PendingTextInputTestView>,
    prefer: bool,
) -> TestInputDispatchSnapshot {
    let result = window.simulate_event_with_dispatch_snapshot(altgr_key_down(prefer));
    if result.propagated() {
        window.update(|_, window, cx| {
            let mut handler = window
                .platform_window
                .take_input_handler()
                .expect("focused input handler");
            handler.dispatch_input("ą", window, cx);
            window.restore_input_handler_if_admitted(handler, cx);
        });
    }
    result
}

#[test]
fn test_keystroke_input_preference() {
    let (mut app, mut window, input) = setup_altgr_test();
    for (prefer, accepts, preference) in [
        (true, true, InputPreference::CharacterInput),
        (false, true, InputPreference::KeyBindings),
        (true, false, InputPreference::KeyBindings),
        (false, false, InputPreference::KeyBindings),
    ] {
        input.accepts_text_input.set(accepts);
        input.actions.borrow_mut().clear();
        input.text.borrow_mut().clear();
        input.intercepted.borrow_mut().clear();
        input.observed.borrow_mut().clear();
        let result = dispatch_altgr_key_down(&mut window, prefer);
        let character_input = preference == InputPreference::CharacterInput;
        let action = (!character_input).then_some(TestAction.name());
        assert_eq!(result.propagated(), character_input);
        assert_eq!(input.intercepted.borrow().as_slice(), &[(preference, None)]);
        assert_eq!(input.observed.borrow().as_slice(), &[(preference, action)]);
        assert_eq!(input.actions.borrow().as_slice(), action.as_slice());
        assert_eq!(
            input.text.borrow().as_str(),
            if character_input { "ą" } else { "" }
        );
    }
    app.update(|cx| cx.shutdown());
}

#[test]
fn test_input_preference_rechecked_after_interception() {
    let (mut app, mut window, input) = setup_altgr_test();
    let accepts = input.accepts_text_input.clone();
    let _interceptor = app.update(|cx| {
        cx.intercept_keystrokes(move |event, _, _| {
            assert_eq!(
                event.input_preference == InputPreference::CharacterInput,
                accepts.get()
            );
            accepts.set(!accepts.get());
        })
    });
    for (initially_accepts, before, after) in [
        (
            true,
            InputPreference::CharacterInput,
            InputPreference::KeyBindings,
        ),
        (
            false,
            InputPreference::KeyBindings,
            InputPreference::CharacterInput,
        ),
    ] {
        input.accepts_text_input.set(initially_accepts);
        input.actions.borrow_mut().clear();
        input.text.borrow_mut().clear();
        input.intercepted.borrow_mut().clear();
        input.observed.borrow_mut().clear();
        let result = dispatch_altgr_key_down(&mut window, true);
        let action = initially_accepts.then_some(TestAction.name());
        assert_eq!(result.propagated(), !initially_accepts);
        assert_eq!(input.intercepted.borrow().as_slice(), &[(before, None)]);
        assert_eq!(input.observed.borrow().as_slice(), &[(after, action)]);
        assert_eq!(input.actions.borrow().as_slice(), action.as_slice());
        assert_eq!(
            input.text.borrow().as_str(),
            if initially_accepts { "" } else { "ą" }
        );
    }
    app.update(|cx| cx.shutdown());
}

#[test]
fn test_keystroke_interceptors_can_consume_preferred_character_input() {
    let (mut app, mut window, input) = setup_altgr_test();
    let _interceptor = app.update(|cx| {
        cx.intercept_keystrokes(|event, _, cx| {
            assert_eq!(event.input_preference, InputPreference::CharacterInput);
            cx.stop_propagation();
        })
    });
    assert!(!dispatch_altgr_key_down(&mut window, true).propagated());
    assert_eq!(
        input.intercepted.borrow().as_slice(),
        &[(InputPreference::CharacterInput, None)]
    );
    assert!(input.observed.borrow().is_empty());
    assert!(input.actions.borrow().is_empty());
    assert_eq!(input.text.borrow().as_str(), "");
    app.update(|cx| cx.shutdown());
}

#[test]
fn test_input_preference_rechecked_after_replayed_action() {
    let (mut app, mut window, input) = setup_altgr_test();
    app.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-b", EnableTextInput, Some("Terminal")),
            KeyBinding::new("ctrl-b h", TestAction, Some("Terminal")),
        ])
    });
    input.accepts_text_input.set(false);
    window.simulate_keystrokes("ctrl-b");
    window.update(|_, window, _| assert!(window.has_pending_keystrokes()));
    assert!(input.observed.borrow().is_empty());
    assert!(input.actions.borrow().is_empty());
    assert_eq!(input.text.borrow().as_str(), "");
    assert!(dispatch_altgr_key_down(&mut window, true).propagated());
    assert_eq!(
        input.intercepted.borrow().as_slice(),
        &[
            (InputPreference::KeyBindings, None),
            (InputPreference::KeyBindings, None),
        ]
    );
    assert_eq!(
        input.observed.borrow().as_slice(),
        &[
            (InputPreference::KeyBindings, Some(EnableTextInput.name())),
            (InputPreference::CharacterInput, None),
        ]
    );
    assert!(input.accepts_text_input.get());
    assert_eq!(input.actions.borrow().as_slice(), &[EnableTextInput.name()]);
    assert_eq!(input.text.borrow().as_str(), "ą");
    window.update(|_, window, _| assert!(!window.has_pending_keystrokes()));
    app.update(|cx| cx.shutdown());
}

#[test]
fn test_close_during_interception_blocks_later_input_side_effects() {
    let (mut app, mut window, input) = setup_altgr_test();
    let handle = window.handle();
    let slot = window.update(|_, window, _| {
        window
            .platform_window
            .input_handler_slot_for_test()
            .unwrap()
    });
    let _closer = app.update(|cx| {
        cx.intercept_keystrokes(|event, window, cx| {
            assert_eq!(event.input_preference, InputPreference::CharacterInput);
            window.remove_window(cx);
            let rejected = window.dispatch_event(PlatformInput::KeyDown(altgr_key_down(true)), cx);
            assert!(!rejected.propagate);
            assert!(rejected.default_prevented);
        })
    });
    let later_calls = Rc::new(Cell::new(0));
    let _later = app.update(|cx| {
        cx.intercept_keystrokes({
            let later_calls = later_calls.clone();
            move |_, _, _| later_calls.set(later_calls.get() + 1)
        })
    });
    let result = dispatch_altgr_key_down(&mut window, true);
    assert!(!result.propagated());
    assert!(result.default_prevented());
    assert_eq!(later_calls.get(), 0);
    assert_eq!(
        input.intercepted.borrow().as_slice(),
        &[(InputPreference::CharacterInput, None)]
    );
    assert!(input.observed.borrow().is_empty());
    assert!(input.actions.borrow().is_empty());
    assert_eq!(input.text.borrow().as_str(), "");
    assert!(slot.take().is_none());
    let panic =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| slot.with_handler(|_| ())))
            .expect_err("a retired native callback slot must refuse entry");
    let violation = panic
        .downcast_ref::<crate::NativeInputInvariantViolation>()
        .expect("native callback refusal retains its typed diagnostic");
    assert_eq!(violation.window_id, handle.window_id());
    assert_eq!(violation.boundary, crate::NativeInputBoundary::InputHandler);
    assert_eq!(
        violation.failure,
        crate::NativeInvariantFailure::RetiredSlot
    );
    assert!(!app.windows().contains(&handle.into()));
    app.update(|cx| assert!(handle.update(cx, |_, _, _| ()).is_err()));
    app.update(|cx| cx.shutdown());
}
