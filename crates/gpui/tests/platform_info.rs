#![cfg(feature = "test-support")]

//! Public runtime-OS consumers of Zed 389c22e1c747d1a433c3b51c176936166c9cc1cf,
//! as present at fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! Input runs through a rendered Window's callbacks on one host. Test-platform frames
//! establish dispatch and lifetime behavior, not native input, GPU output or presentation.

use open_gpui::{
    Action, App, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyBinding,
    KeyBindingContextPredicate, KeyContext, KeyDownEvent, Keystroke, Modifiers, OperatingSystem,
    ParentElement as _, PlatformInfo, PlatformTarget, QuitMode, Render, Styled as _, Subscription,
    TestAppContext, VisualTestContext, Window, actions, div, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

actions!(
    runtime_platform_consumer,
    [Save, Chord, Control, PlatformKey, Prefix, Contextual]
);

fn operating_systems() -> [(OperatingSystem, &'static str, &'static str, &'static str); 7] {
    [
        (OperatingSystem::Mac, "macos", "⌘⇧←", "cmd-shift-left"),
        (
            OperatingSystem::Windows,
            "windows",
            "⊞shift-left",
            "win-shift-left",
        ),
        (
            OperatingSystem::Linux,
            "linux",
            "❖shift-left",
            "super-shift-left",
        ),
        (
            OperatingSystem::FreeBsd,
            "linux",
            "❖shift-left",
            "super-shift-left",
        ),
        (
            OperatingSystem::Ios,
            "ios",
            "cmd-shift-left",
            "cmd-shift-left",
        ),
        (
            OperatingSystem::Android,
            "android",
            "cmd-shift-left",
            "super-shift-left",
        ),
        (
            OperatingSystem::Unknown,
            "unknown",
            "cmd-shift-left",
            "super-shift-left",
        ),
    ]
}

#[test]
fn public_static_and_dynamic_parsing_and_labels_use_explicit_host_conventions() {
    for source in [
        "secondary-s",
        "SECONDARY-s",
        "secondary",
        "ctrl-secondary-s",
    ] {
        let error = Keystroke::parse(source).expect_err("static input needs explicit modifiers");
        assert_eq!(error.keystroke, source);
    }
    let platform = Keystroke::parse("cmd-shift-left").unwrap();
    for (os, context_os, display, unparse) in operating_systems() {
        let dynamic = String::from("secondary-s");
        let secondary = Keystroke::parse_with_os(&dynamic, os).unwrap();
        let explicit = if os == OperatingSystem::Mac {
            "cmd-s"
        } else {
            "ctrl-s"
        };
        assert_eq!(secondary, Keystroke::parse(explicit).unwrap());
        assert_eq!(platform.display(os).to_string(), display);
        assert_eq!(platform.unparse(os), unparse);
        assert_eq!(
            Keystroke::parse_with_os(&platform.unparse(os), os).unwrap(),
            platform,
        );
        let context = KeyContext::new_with_defaults(os);
        assert_eq!(
            context.get("os").map(|value| value.as_ref()),
            Some(context_os)
        );
        assert!(context.primary().is_none());
        for source in [
            "ctrl-f6",
            "cmd-f7",
            "alt-shift-tab",
            "fn-ctrl-alt-cmd-shift-x",
        ] {
            let explicit = Keystroke::parse(source).unwrap();
            assert_eq!(Keystroke::parse_with_os(source, os).unwrap(), explicit);
            assert_eq!(
                Keystroke::parse_with_os(&explicit.unparse(os), os).unwrap(),
                explicit,
            );
        }
        let dynamic_invalid = String::from("secondary-invalid-extra");
        assert_eq!(
            Keystroke::parse_with_os(&dynamic_invalid, os)
                .unwrap_err()
                .keystroke,
            dynamic_invalid,
        );
    }
}

#[test]
fn public_app_identity_is_runtime_local_and_independent_of_delivery_target() {
    let mut cx = TestAppContext::single();
    let mut other = cx.new_app();
    let other_info = PlatformInfo {
        os: OperatingSystem::Windows,
        target: PlatformTarget::Native,
    };
    other.set_platform_info(other_info);
    for (os, _, _, _) in operating_systems() {
        for target in [PlatformTarget::Native, PlatformTarget::Web] {
            let info = PlatformInfo { os, target };
            cx.set_platform_info(info);
            cx.update(|app| {
                assert_eq!(app.platform_info(), info);
                assert_eq!(app.platform_info().is_mac(), os == OperatingSystem::Mac);
                let secondary = if os == OperatingSystem::Mac {
                    Modifiers::command()
                } else {
                    Modifiers::control()
                };
                assert_eq!(Modifiers::secondary_key(app), secondary);
                for bits in 0..32 {
                    let modifiers = Modifiers {
                        control: bits & 1 != 0,
                        alt: bits & 2 != 0,
                        shift: bits & 4 != 0,
                        platform: bits & 8 != 0,
                        function: bits & 16 != 0,
                    };
                    assert_eq!(
                        modifiers.secondary(app),
                        if os == OperatingSystem::Mac {
                            modifiers.platform
                        } else {
                            modifiers.control
                        },
                    );
                }
            });
            assert_eq!(other.read(App::platform_info), other_info);
        }
    }
    other.quit();
    cx.quit();
}

#[derive(Default)]
struct Trace {
    calls: RefCell<Vec<(&'static str, usize)>>,
    pending: RefCell<Vec<bool>>,
    labels: RefCell<Vec<String>>,
    drops: Cell<usize>,
    releases: Cell<usize>,
}

struct Consumer {
    focus: [FocusHandle; 2],
    trace: Rc<Trace>,
    _pending: Subscription,
}

impl Consumer {
    fn record(&self, action: &'static str, window: &Window) {
        let focused = self
            .focus
            .iter()
            .position(|focus| focus.is_focused(window))
            .unwrap();
        self.trace.calls.borrow_mut().push((action, focused));
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        self.trace.drops.set(self.trace.drops.get() + 1);
    }
}

impl Render for Consumer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut context = KeyContext::new_with_defaults(cx.platform_info().os);
        context.add("RuntimePlatformConsumer");
        let labels = [&Save as &dyn Action, &Chord, &Control, &PlatformKey]
            .map(|action| window.keystroke_text_for(action, cx));
        *self.trace.labels.borrow_mut() = labels.to_vec();
        div()
            .size_full()
            .key_context(context)
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                this.record("save", window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Chord, window, cx| {
                this.record("chord", window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Control, window, cx| {
                this.record("control", window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PlatformKey, window, cx| {
                this.record("platform", window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Prefix, window, cx| {
                this.record("prefix", window);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Contextual, window, cx| {
                this.record("context", window);
                cx.notify();
            }))
            .children(
                labels
                    .into_iter()
                    .map(|label| div().h(px(20.)).child(label)),
            )
            .children(self.focus.iter().enumerate().map(|(index, focus)| {
                div()
                    .id(("runtime-field", index))
                    .h(px(24.))
                    .track_focus(focus)
            }))
    }
}

fn bind(app: &mut App, os_name: &str) {
    app.clear_key_bindings();
    let predicate = Rc::new(
        KeyBindingContextPredicate::parse(&format!("RuntimePlatformConsumer && os == {os_name}"))
            .unwrap(),
    );
    for (source, action) in [
        ("secondary-s", Box::new(Save) as Box<dyn Action>),
        ("secondary-k", Box::new(Prefix)),
        ("secondary-k secondary-c", Box::new(Chord)),
        ("ctrl-f6", Box::new(Control)),
        ("cmd-f7", Box::new(PlatformKey)),
    ] {
        let dynamic = source.to_owned();
        let binding = KeyBinding::load(&dynamic, action, None, true, None, app)
            .expect("public dynamic loading must use the App's actual keyboard mapper");
        app.bind_keys([binding]);
    }
    let contextual =
        KeyBinding::load("f8", Box::new(Contextual), Some(predicate), true, None, app).unwrap();
    app.bind_keys([contextual]);
}

fn press(visual: &mut VisualTestContext, source: &str, held: bool) {
    visual.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse_with_os(source, visual.read(|app| app.platform_info().os))
            .unwrap(),
        is_held: held,
        prefer_character_input: false,
    });
}

#[test]
fn public_loaded_bindings_dispatch_and_render_labels_for_every_runtime_identity() {
    let mut cx = TestAppContext::single();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    for (os, os_name, _, _) in operating_systems() {
        for target in [PlatformTarget::Native, PlatformTarget::Web] {
            let info = PlatformInfo { os, target };
            cx.set_platform_info(info);
            cx.update(|app| bind(app, os_name));
            let trace = Rc::new(Trace::default());
            let handle = cx.open_window(size(px(360.), px(200.)), |window, cx| {
                let focus = [cx.focus_handle(), cx.focus_handle()];
                window.focus(&focus[0], cx);
                let pending_trace = trace.clone();
                let release_trace = trace.clone();
                cx.on_release(move |_: &mut Consumer, _| {
                    release_trace.releases.set(release_trace.releases.get() + 1);
                })
                .detach();
                Consumer {
                    focus,
                    trace: trace.clone(),
                    _pending: cx.observe_pending_input(window, move |_, window, _| {
                        pending_trace
                            .pending
                            .borrow_mut()
                            .push(window.has_pending_keystrokes());
                    }),
                }
            });
            let root = handle.entity(&cx).unwrap().downgrade();
            let mut visual = VisualTestContext::from_window(handle.into(), &cx);
            visual.update(|window, _| {
                let _ = window.activate_window();
            });
            visual.run_until_parked();
            visual.update(|window, app| {
                assert!(
                    window.is_window_active(),
                    "repeat input requires actual active-window state",
                );
                window.refresh();
                window.draw(app).clear(app);
            });
            let secondary = if os == OperatingSystem::Mac {
                "cmd"
            } else {
                "ctrl"
            };
            let wrong = if os == OperatingSystem::Mac {
                "ctrl-s"
            } else {
                "cmd-s"
            };
            let expected = if os == OperatingSystem::Mac {
                ["⌘S", "⌘K ⌘C", "^f6", "⌘f7"]
            } else {
                let platform_label = match os {
                    OperatingSystem::Windows => "⊞f7",
                    OperatingSystem::Linux | OperatingSystem::FreeBsd => "❖f7",
                    _ => "cmd-f7",
                };
                ["ctrl-S", "ctrl-K ctrl-C", "ctrl-f6", platform_label]
            };
            visual.update(|window, app| {
                assert_eq!(app.platform_info(), info);
                assert_eq!(
                    window
                        .context_stack()
                        .last()
                        .unwrap()
                        .get("os")
                        .unwrap()
                        .as_ref(),
                    os_name,
                );
                assert_eq!(window.keystroke_text_for(&Save, app), expected[0]);
                assert_eq!(window.keystroke_text_for(&Chord, app), expected[1]);
            });
            assert_eq!(*trace.labels.borrow(), expected);
            press(&mut visual, wrong, false);
            assert!(trace.calls.borrow().is_empty());
            press(&mut visual, &format!("{secondary}-s"), false);
            press(&mut visual, &format!("{secondary}-s"), true);
            press(&mut visual, "ctrl-f6", false);
            press(&mut visual, "cmd-f7", false);
            press(&mut visual, "f8", false);
            assert_eq!(
                *trace.calls.borrow(),
                [
                    ("save", 0),
                    ("save", 0),
                    ("control", 0),
                    ("platform", 0),
                    ("context", 0)
                ],
            );
            trace.pending.borrow_mut().clear();

            let prefix = format!("{secondary}-k");
            let suffix = format!("{secondary}-c");
            press(&mut visual, &prefix, false);
            visual.update(|window, _| {
                assert_eq!(
                    window.pending_input_keystrokes(),
                    Some([Keystroke::parse_with_os(&prefix, os).unwrap()].as_slice()),
                );
            });
            handle
                .update(&mut visual, |root, window, cx| {
                    window.focus(&root.focus[0], cx)
                })
                .unwrap();
            visual.update(|window, _| assert!(window.has_pending_keystrokes()));
            press(&mut visual, &suffix, false);
            assert_eq!(trace.calls.borrow().last(), Some(&("chord", 0)));
            assert_eq!(*trace.pending.borrow(), [true, false]);
            trace.pending.borrow_mut().clear();

            press(&mut visual, &prefix, false);
            handle
                .update(&mut visual, |root, window, cx| {
                    window.focus(&root.focus[1], cx)
                })
                .unwrap();
            visual.update(|window, _| assert!(window.pending_input_keystrokes().is_none()));
            assert_eq!(*trace.pending.borrow(), [true, false]);
            let calls = trace.calls.borrow().len();
            press(&mut visual, &suffix, false);
            assert_eq!(trace.calls.borrow().len(), calls);
            assert_eq!(*trace.pending.borrow(), [true, false, false]);
            trace.pending.borrow_mut().clear();
            press(&mut visual, &prefix, false);
            cx.executor().advance_clock(Duration::from_millis(999));
            cx.run_until_parked();
            assert_eq!(trace.calls.borrow().len(), calls);
            assert_eq!(*trace.pending.borrow(), [true]);
            cx.executor().advance_clock(Duration::from_millis(1));
            cx.run_until_parked();
            assert_eq!(trace.calls.borrow().last(), Some(&("prefix", 1)));
            visual.update(|window, _| assert!(!window.has_pending_keystrokes()));
            assert_eq!(*trace.pending.borrow(), [true, false]);
            trace.pending.borrow_mut().clear();

            press(&mut visual, &prefix, false);
            visual.update(|window, app| {
                window.blur(app);
                window.blur(app);
            });
            assert_eq!(*trace.pending.borrow(), [true, false]);
            handle
                .update(&mut visual, |root, window, cx| {
                    window.focus(&root.focus[0], cx)
                })
                .unwrap();
            press(&mut visual, &suffix, false);
            assert_eq!(trace.calls.borrow().last(), Some(&("prefix", 1)));
            assert_eq!(*trace.pending.borrow(), [true, false, false]);
            trace.pending.borrow_mut().clear();
            press(&mut visual, &prefix, false);
            let before_close = trace.calls.borrow().clone();
            let notifications = trace.pending.borrow().len();
            visual.update(|window, app| window.remove_window(app));
            drop(visual);
            cx.run_until_parked();
            cx.update(|_| {});
            root.assert_released();
            assert_eq!(trace.drops.get(), 1);
            assert_eq!(trace.releases.get(), 1);
            assert_eq!(*trace.pending.borrow(), [true]);
            cx.executor().advance_clock(Duration::from_secs(2));
            cx.run_until_parked();
            assert_eq!(*trace.calls.borrow(), before_close);
            assert_eq!(trace.pending.borrow().len(), notifications);
            assert!(handle.update(&mut cx, |_, _, _| ()).is_err());
            assert!(cx.windows().is_empty());
        }
    }
    cx.quit();
}
