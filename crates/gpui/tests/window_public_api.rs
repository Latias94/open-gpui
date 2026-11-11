#![cfg(feature = "test-support")]

use open_gpui::{
    App, AppContext as _, Bounds, Context, Empty, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, Keystroke, MouseButton, NativeBoundaryKind,
    NativeBoundaryTarget, NativePlatformCommandKind, ParentElement as _, PendingInputTimeoutStatus,
    PromptLevel, PromptResponse, Render, ResizeEdge, StatefulInteractiveElement as _, Styled as _,
    TestApp, TestAppWindow, Window, WindowBounds, WindowOptions, actions, div, point, px, size,
};
use std::{cell::Cell, rc::Rc, time::Duration};

#[test]
fn public_window_resize_respects_creation_options() {
    let mut cx = TestApp::new();
    let options = [(false, true), (true, false)];
    let mut windows = options.map(|(is_resizable, is_minimizable)| {
        cx.open_window_with_options(
            WindowOptions {
                is_resizable,
                is_minimizable,
                ..WindowOptions::default()
            },
            |_, _| Empty,
        )
    });
    cx.run_until_parked();
    let cursor = cx.read(|app| app.native_boundary_diagnostics(Default::default()).cursor);

    for (window, (is_resizable, is_minimizable)) in windows.iter_mut().zip(options) {
        window.update(|_, window, _| {
            assert_eq!(window.is_resizable(), is_resizable);
            assert_eq!(window.is_minimizable(), is_minimizable);
            window.start_window_resize(ResizeEdge::BottomRight);
        });
    }

    let commands = cx.read(|app| app.native_boundary_diagnostics(cursor));
    let resize_targets = commands
        .terminal
        .iter()
        .filter(|record| {
            record.kind == NativeBoundaryKind::Command(NativePlatformCommandKind::StartWindowResize)
        })
        .map(|record| record.target)
        .collect::<Vec<_>>();
    assert_eq!(
        resize_targets,
        [NativeBoundaryTarget::Window(
            windows[1].handle().window_id()
        )]
    );
}

struct QueryingPrompt {
    focus: FocusHandle,
    active_during_render: Rc<Cell<bool>>,
}

impl Focusable for QueryingPrompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<PromptResponse> for QueryingPrompt {}

impl Render for QueryingPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.active_during_render.set(window.has_active_prompt());
        div()
            .id("accept-prompt")
            .size_full()
            .track_focus(&self.focus)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(PromptResponse(0))))
    }
}

struct PromptBackground(Rc<Cell<usize>>);

impl Render for PromptBackground {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let outside_count = self.0.clone();
        div()
            .size_full()
            .child(div().size(px(24.)).on_mouse_down_out(move |_, _, _| {
                outside_count.set(outside_count.get() + 1);
            }))
    }
}

#[test]
fn public_prompt_query_tracks_only_window_rendered_prompts() {
    let mut cx = TestApp::new();
    let outside_count = Rc::new(Cell::new(0));
    let mut window = cx.open_window_with_options(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(160.), px(120.)),
            })),
            ..WindowOptions::default()
        },
        |_, _| PromptBackground(outside_count.clone()),
    );
    let outside = point(px(80.), px(80.));
    window.draw();
    window.simulate_click(outside, MouseButton::Left);
    assert_eq!(outside_count.get(), 1);
    let mut native_answer = window.update(|_, window, cx| {
        assert!(!window.has_active_prompt());
        let answer = window.prompt(PromptLevel::Info, "Native prompt", None, &["OK"], cx);
        assert!(!window.has_active_prompt());
        answer
    });
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Ok");
    assert_eq!(native_answer.try_recv().unwrap(), Some(0));

    let active_during_render = Rc::new(Cell::new(false));
    cx.update(|app| {
        let active_during_render = active_during_render.clone();
        app.set_prompt_builder(move |_, _, _, _, handle, window, app| {
            let view = app.new(|cx| QueryingPrompt {
                focus: cx.focus_handle(),
                active_during_render: active_during_render.clone(),
            });
            handle.with_view(view, window, app)
        });
    });
    let mut rendered_answer = window.update(|_, window, cx| {
        let answer = window.prompt(PromptLevel::Info, "Rendered prompt", None, &["OK"], cx);
        assert!(window.has_active_prompt());
        answer
    });
    assert!(!cx.has_pending_prompt());
    window.draw();
    assert!(active_during_render.get());

    window.simulate_click(outside, MouseButton::Left);
    assert_eq!(rendered_answer.try_recv().unwrap(), Some(0));
    window.update(|_, window, _| assert!(!window.has_active_prompt()));
    assert_eq!(outside_count.get(), 1);
    window.draw();
    window.simulate_click(outside, MouseButton::Left);
    assert_eq!(outside_count.get(), 2);
}

actions!(window_public_api, [ShortChord, MiddleChord, LongChord]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletedChord {
    Short,
    Middle,
    Long,
}

struct ChordTarget {
    focus: FocusHandle,
    key_context: &'static str,
    completed: Vec<CompletedChord>,
    observed_timeouts: Vec<Option<bool>>,
}

impl Render for ChordTarget {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(self.key_context)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ShortChord, _, _| {
                this.completed.push(CompletedChord::Short);
            }))
            .on_action(cx.listener(|this, _: &MiddleChord, _, _| {
                this.completed.push(CompletedChord::Middle);
            }))
            .on_action(cx.listener(|this, _: &LongChord, _, _| {
                this.completed.push(CompletedChord::Long);
            }))
    }
}

fn chord_window(cx: &mut TestApp) -> TestAppWindow<ChordTarget> {
    cx.update(|app| {
        app.bind_keys([
            KeyBinding::new("ctrl-b", ShortChord, Some("ChordTarget")),
            KeyBinding::new("ctrl-b h", MiddleChord, Some("ChordTarget")),
            KeyBinding::new("ctrl-b h j", LongChord, Some("ChordTarget")),
            KeyBinding::new("ctrl-x k", LongChord, Some("ChordTarget")),
        ]);
    });
    let mut window = cx.open_window(|_, cx| ChordTarget {
        focus: cx.focus_handle(),
        key_context: "ChordTarget",
        completed: Vec::new(),
        observed_timeouts: Vec::new(),
    });
    window.update(|target, window, cx| target.focus.focus(window, cx));
    window
}

fn timeout(window: &Window) -> PendingInputTimeoutStatus {
    window
        .pending_input()
        .and_then(|input| input.timeout())
        .expect("the current chord has a timeout")
}

fn advance(cx: &TestApp, duration: Duration) {
    cx.advance_clock(duration);
    cx.run_until_parked();
}

#[test]
fn public_pending_timeout_pause_preserves_remaining_time_and_observer_order() {
    let mut cx = TestApp::new();
    let mut chord = chord_window(&mut cx);
    let other_owner = cx.update(|app| app.new(|_| ()));
    let _subscription = chord.update(|_, window, cx| {
        cx.observe_pending_input(window, |target, window, _| {
            target.observed_timeouts.push(
                window
                    .pending_input()
                    .and_then(|input| input.timeout())
                    .map(|timeout| timeout.is_paused()),
            );
        })
    });
    chord.simulate_keystroke("ctrl-b");
    advance(&cx, Duration::from_millis(400));

    chord.update(|target, window, cx| {
        let input = window.pending_input().unwrap();
        assert_eq!(input.keystrokes(), &[Keystroke::parse("ctrl-b").unwrap()]);
        let running = input.timeout().unwrap();
        assert_eq!(running.duration(), Duration::from_secs(1));
        assert_eq!(running.remaining(cx), Duration::from_millis(600));
        assert!(!running.is_paused());

        let owner = cx.entity();
        assert!(window.set_pending_input_timeout_paused(&owner, true, cx));
        assert!(!window.set_pending_input_timeout_paused(&owner, true, cx));
        assert!(!window.set_pending_input_timeout_paused(&other_owner, true, cx));
        assert!(!window.set_pending_input_timeout_paused(&other_owner, false, cx));
        assert_eq!(target.observed_timeouts, [Some(false)]);
    });
    advance(&cx, Duration::from_secs(2));

    chord.update(|target, window, cx| {
        assert!(target.completed.is_empty());
        assert_eq!(target.observed_timeouts, [Some(false), Some(true)]);
        let paused = timeout(window);
        assert!(paused.is_paused());
        assert_eq!(paused.remaining(cx), Duration::from_millis(600));

        let owner = cx.entity();
        assert!(window.set_pending_input_timeout_paused(&owner, false, cx));
        let running = timeout(window);
        assert!(!running.is_paused());
        assert_eq!(running.remaining(cx), Duration::from_millis(600));
    });
    advance(&cx, Duration::from_millis(599));
    chord.read(|target, _| assert!(target.completed.is_empty()));
    advance(&cx, Duration::from_millis(1));
    chord.update(|target, window, _| {
        assert!(window.pending_input().is_none());
        assert_eq!(target.completed, [CompletedChord::Short]);
        assert_eq!(
            target.observed_timeouts,
            [Some(false), Some(true), Some(false), None]
        );
    });
}

#[test]
fn public_pending_timeout_owner_release_resumes_the_advanced_prefix() {
    let mut cx = TestApp::new();
    let mut chord = chord_window(&mut cx);
    let owner = cx.update(|app| app.new(|_| ()));
    chord.simulate_keystroke("ctrl-b");
    advance(&cx, Duration::from_millis(400));
    chord.update(|_, window, cx| {
        assert!(window.set_pending_input_timeout_paused(&owner, true, cx));
    });

    chord.simulate_keystroke("h");
    advance(&cx, Duration::from_secs(2));
    chord.update(|target, window, cx| {
        assert!(target.completed.is_empty());
        assert_eq!(window.pending_input().unwrap().keystrokes().len(), 2);
        let paused = timeout(window);
        assert!(paused.is_paused());
        assert_eq!(paused.remaining(cx), Duration::from_secs(1));
    });

    drop(owner);
    cx.update(|_| ());
    chord.update(|_, window, cx| {
        let running = timeout(window);
        assert!(!running.is_paused());
        assert_eq!(running.remaining(cx), Duration::from_secs(1));
    });
    advance(&cx, Duration::from_secs(1));
    chord.update(|target, window, _| {
        assert!(window.pending_input().is_none());
        assert_eq!(target.completed, [CompletedChord::Middle]);
    });
}

#[test]
fn public_untimed_chord_does_not_gain_a_timeout_when_paused() {
    let mut cx = TestApp::new();
    let mut chord = chord_window(&mut cx);
    let owner = cx.update(|app| app.new(|_| ()));
    chord.simulate_keystroke("ctrl-x");
    chord.update(|_, window, cx| {
        assert!(window.pending_input().unwrap().timeout().is_none());
        assert!(!window.set_pending_input_timeout_paused(&owner, true, cx));
    });
    advance(&cx, Duration::from_secs(2));
    chord.simulate_keystroke("k");
    chord.update(|target, window, _| {
        assert!(window.pending_input().is_none());
        assert_eq!(target.completed, [CompletedChord::Long]);
    });
}

#[test]
fn public_paused_chord_cannot_survive_an_accepted_context_change() {
    let mut cx = TestApp::new();
    let mut chord = chord_window(&mut cx);
    let owner = cx.update(|app| app.new(|_| ()));
    chord.simulate_keystroke("ctrl-b");
    chord.update(|target, window, cx| {
        assert!(window.set_pending_input_timeout_paused(&owner, true, cx));
        target.key_context = "OtherContext";
        cx.notify();
    });
    chord.update(|target, window, cx| {
        assert!(target.focus.is_focused(window));
        assert!(window.pending_input().is_none());
        assert!(!window.set_pending_input_timeout_paused(&owner, false, cx));
        target.key_context = "ChordTarget";
        cx.notify();
    });
    chord.update(|_, window, _| assert!(window.pending_input().is_none()));
    drop(owner);
    cx.update(|_| ());
    advance(&cx, Duration::from_secs(2));
    chord.simulate_keystrokes("h j");
    chord.read(|target, _| assert!(target.completed.is_empty()));

    chord.simulate_keystrokes("ctrl-b h j");
    chord.read(|target, _| assert_eq!(target.completed, [CompletedChord::Long]));
}
