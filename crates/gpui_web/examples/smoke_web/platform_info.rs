use std::{cell::RefCell, rc::Rc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, KeyDownEvent, Keystroke, MouseButton,
    PlatformInfo, Window, WindowBounds, WindowOptions, actions, canvas, div, px, rgb, size,
};

actions!(
    platform_info_smoke,
    [RunSingle, RunChord, ExplicitCommand, ExplicitControl]
);

struct Observation {
    identity: PlatformInfo,
    labels: [String; 4],
    parsed_single_label: String,
    actions: [usize; 4],
    focused: bool,
    action_focused: bool,
    focus_clicks: usize,
    key_events: usize,
    last_key: String,
    last_secondary: bool,
    pending_chord: bool,
    renders: usize,
    paints: usize,
}

impl Observation {
    fn publish(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let probe = super::js_object(&[
            ("os", format!("{:?}", self.identity.os).into()),
            ("target", format!("{:?}", self.identity.target).into()),
            ("isMac", self.identity.is_mac().into()),
            ("singleLabel", self.labels[0].as_str().into()),
            ("chordLabel", self.labels[1].as_str().into()),
            ("commandLabel", self.labels[2].as_str().into()),
            ("controlLabel", self.labels[3].as_str().into()),
            (
                "parsedSingleLabel",
                self.parsed_single_label.as_str().into(),
            ),
            ("singleActions", (self.actions[0] as f64).into()),
            ("chordActions", (self.actions[1] as f64).into()),
            ("commandActions", (self.actions[2] as f64).into()),
            ("controlActions", (self.actions[3] as f64).into()),
            ("focused", self.focused.into()),
            ("actionFocused", self.action_focused.into()),
            ("focusClicks", (self.focus_clicks as f64).into()),
            ("keyEvents", (self.key_events as f64).into()),
            ("lastKey", self.last_key.as_str().into()),
            ("lastSecondary", self.last_secondary.into()),
            ("pendingChord", self.pending_chord.into()),
            ("renderCalls", (self.renders as f64).into()),
            ("paintCalls", (self.paints as f64).into()),
            (
                "counterScope",
                "Fixture render and paint callbacks, not GPU submission, scanout or pixel readback"
                    .into(),
            ),
        ]);
        js_sys::Reflect::set(
            window.as_ref(),
            &"__OPEN_GPUI_PLATFORM_INFO__".into(),
            &probe,
        )
        .expect("platform info probe must be writable");
    }
}

struct PlatformInfoSmoke {
    focus: FocusHandle,
    observation: Rc<RefCell<Observation>>,
}

impl PlatformInfoSmoke {
    fn action(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut observation = self.observation.borrow_mut();
        observation.actions[index] += 1;
        observation.action_focused = self.focus.is_focused(window);
        observation.publish();
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mut observation = self.observation.borrow_mut();
        observation.key_events += 1;
        observation.last_key.clone_from(&event.keystroke.key);
        observation.last_secondary = event.keystroke.modifiers.secondary(cx);
        observation.focused = self.focus.is_focused(window);
        observation.publish();
        if matches!(event.keystroke.key.as_str(), "u" | "j" | "k") {
            window.prevent_default();
            cx.stop_propagation();
        }
        cx.notify();
    }
}

impl Render for PlatformInfoSmoke {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let labels = [
            window.keystroke_text_for(&RunSingle, cx),
            window.keystroke_text_for(&RunChord, cx),
            window.keystroke_text_for(&ExplicitCommand, cx),
            window.keystroke_text_for(&ExplicitControl, cx),
        ];
        let mut observation = self.observation.borrow_mut();
        observation.identity = cx.platform_info();
        observation.parsed_single_label =
            Keystroke::parse_with_os("secondary-j", observation.identity.os)
                .expect("secondary keystroke must parse with the backend host OS")
                .display(observation.identity.os)
                .to_string();
        observation.labels = labels.clone();
        observation.focused = self.focus.is_focused(window);
        observation.pending_chord = window.has_pending_keystrokes();
        observation.renders += 1;
        observation.publish();
        let identity = format!(
            "Host: {:?} | Target: {:?}",
            observation.identity.os, observation.identity.target
        );
        let counts = format!(
            "Single: {} | Chord: {} | Command: {} | Control: {}",
            observation.actions[0],
            observation.actions[1],
            observation.actions[2],
            observation.actions[3]
        );
        let paints = self.observation.clone();
        div()
            .id("platform-shortcuts")
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .bg(rgb(0x101318))
            .text_color(rgb(0xffffff))
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window, cx);
                    this.observation.borrow_mut().focus_clicks += 1;
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(Self::key))
            .on_action(cx.listener(|this, _: &RunSingle, window, cx| this.action(0, window, cx)))
            .on_action(cx.listener(|this, _: &RunChord, window, cx| this.action(1, window, cx)))
            .on_action(
                cx.listener(|this, _: &ExplicitCommand, window, cx| this.action(2, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ExplicitControl, window, cx| this.action(3, window, cx)),
            )
            .child("Platform shortcuts")
            .child(identity)
            .child(format!("Run command: {}", labels[0]))
            .child(format!("Run chord: {}", labels[1]))
            .child(format!("Explicit Command: {}", labels[2]))
            .child(format!("Explicit Control: {}", labels[3]))
            .child("Click to focus. Use U with either modifier to inspect the secondary key.")
            .child(counts)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), _, _| {
                        let mut observation = paints.borrow_mut();
                        observation.paints += 1;
                        observation.publish();
                    },
                )
                .w_full()
                .h(px(1.0)),
            )
    }
}

pub(super) fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .is_some_and(|params| params.get("platform_info").as_deref() == Some("1"))
}

pub(super) fn open(cx: &mut App) {
    let single = KeyBinding::load("secondary-j", Box::new(RunSingle), None, true, None, cx)
        .expect("secondary single binding must load from the current app");
    let chord = KeyBinding::load(
        "secondary-k secondary-j",
        Box::new(RunChord),
        None,
        true,
        None,
        cx,
    )
    .expect("secondary chord binding must load from the current app");
    cx.bind_keys([
        single,
        chord,
        KeyBinding::new("cmd-y", ExplicitCommand, None),
        KeyBinding::new("ctrl-y", ExplicitControl, None),
    ]);
    let identity = cx.platform_info();
    let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            cx.new(|cx| {
                cx.observe_pending_input(window, |this: &mut PlatformInfoSmoke, window, cx| {
                    let mut observation = this.observation.borrow_mut();
                    observation.pending_chord = window.has_pending_keystrokes();
                    observation.publish();
                    cx.notify();
                })
                .detach();
                PlatformInfoSmoke {
                    focus,
                    observation: Rc::new(RefCell::new(Observation {
                        identity,
                        labels: Default::default(),
                        parsed_single_label: String::new(),
                        actions: [0; 4],
                        focused: false,
                        action_focused: false,
                        focus_clicks: 0,
                        key_events: 0,
                        last_key: String::new(),
                        last_secondary: false,
                        pending_chord: false,
                        renders: 0,
                        paints: 0,
                    })),
                }
            })
        },
    )
    .expect("platform info smoke window must open");
    cx.activate(true);
}
