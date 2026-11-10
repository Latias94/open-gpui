//! Independent consumers of Zed 389c22e1c747d1a433c3b51c176936166c9cc1cf at
//! 635026fabc38440fce5e1767d125026a17b12012. No GPUI test-support is enabled.
//! OS-explicit controls run on any host; bench-support executes a real App and
//! Window with the benchmark platform's actual host identity and discarded scenes.

use open_gpui::{
    KeyContext, KeybindingKeystroke, Keystroke, OperatingSystem, PlatformInfo, PlatformTarget,
};

fn conventions() -> [(OperatingSystem, &'static str, &'static str, &'static str); 7] {
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
fn downstream_dynamic_parser_display_and_context_follow_every_explicit_os() {
    for (os, context_os, display, unparse) in conventions() {
        for target in [PlatformTarget::Native, PlatformTarget::Web] {
            let info = PlatformInfo { os, target };
            assert_eq!(info.is_mac(), os == OperatingSystem::Mac);
            assert_eq!(info.target, target);
        }
        let dynamic = String::from("secondary-s");
        let parsed = Keystroke::parse_with_os(&dynamic, os).unwrap();
        let expected = Keystroke::parse(if os == OperatingSystem::Mac {
            "cmd-s"
        } else {
            "ctrl-s"
        })
        .unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(
            KeyContext::new_with_defaults(os)
                .get("os")
                .unwrap()
                .as_ref(),
            context_os,
        );
        let platform = Keystroke::parse("cmd-shift-left").unwrap();
        let binding = KeybindingKeystroke::from_keystroke(platform.clone());
        assert_eq!(platform.display(os).to_string(), display);
        assert_eq!(binding.display(os).to_string(), display);
        assert_eq!(platform.unparse(os), unparse);
        assert_eq!(binding.unparse(os), unparse);
        assert_eq!(
            Keystroke::parse_with_os(&binding.unparse(os), os).unwrap(),
            platform,
        );
    }
}

#[test]
fn downstream_static_parsing_rejects_secondary_and_explicit_modifiers_never_change() {
    for source in [
        "secondary-s",
        "SECONDARY-s",
        "secondary",
        "ctrl-secondary-s",
    ] {
        assert_eq!(Keystroke::parse(source).unwrap_err().keystroke, source);
    }
    for (os, _, _, _) in conventions() {
        for source in [
            "ctrl-f6",
            "cmd-f7",
            "alt-shift-tab",
            "fn-ctrl-alt-cmd-shift-x",
        ] {
            let parsed = Keystroke::parse(source).unwrap();
            assert_eq!(
                Keystroke::parse_with_os(&source.to_owned(), os).unwrap(),
                parsed,
            );
            assert_eq!(
                Keystroke::parse_with_os(&parsed.unparse(os), os).unwrap(),
                parsed,
            );
        }
        let invalid = String::from("secondary-invalid-extra");
        assert_eq!(
            Keystroke::parse_with_os(&invalid, os)
                .unwrap_err()
                .keystroke,
            invalid,
        );
    }
}

#[cfg(all(feature = "bench-support", not(target_family = "wasm")))]
mod runtime {
    use super::*;
    use open_gpui::{
        Action, App, AppContext as _, Application, Context, FocusHandle, Global,
        InteractiveElement as _, IntoElement, KeyBinding, KeyBindingContextPredicate, KeyDownEvent,
        Modifiers, NoopTextSystem, ParentElement as _, QuitMode, Render, Styled as _, Subscription,
        Window, WindowHandle, WindowOptions, actions, bench_platform, div, px,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::Arc,
    };

    actions!(
        independent_platform_consumer,
        [Save, Chord, Control, PlatformKey, Contextual]
    );

    #[derive(Default)]
    struct Trace {
        calls: RefCell<Vec<(&'static str, usize)>>,
        labels: RefCell<Vec<String>>,
        pending: RefCell<Vec<bool>>,
        drops: Cell<usize>,
        releases: Cell<usize>,
    }

    struct Consumer {
        focus: [FocusHandle; 2],
        trace: Rc<Trace>,
        _pending: Subscription,
    }

    impl Consumer {
        fn record(&self, name: &'static str, window: &Window) {
            let focus = self
                .focus
                .iter()
                .position(|focus| focus.is_focused(window))
                .unwrap();
            self.trace.calls.borrow_mut().push((name, focus));
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
            context.add("IndependentPlatformConsumer");
            let labels = [&Save as &dyn Action, &Chord, &Control, &PlatformKey]
                .map(|action| window.keystroke_text_for(action, cx));
            *self.trace.labels.borrow_mut() = labels.to_vec();
            div()
                .size_full()
                .key_context(context)
                .on_action(cx.listener(|root, _: &Save, window, cx| {
                    root.record("save", window);
                    cx.notify();
                }))
                .on_action(cx.listener(|root, _: &Chord, window, cx| {
                    root.record("chord", window);
                    cx.notify();
                }))
                .on_action(cx.listener(|root, _: &Control, window, cx| {
                    root.record("control", window);
                    cx.notify();
                }))
                .on_action(cx.listener(|root, _: &PlatformKey, window, cx| {
                    root.record("platform", window);
                    cx.notify();
                }))
                .on_action(cx.listener(|root, _: &Contextual, window, cx| {
                    root.record("context", window);
                    cx.notify();
                }))
                .children(
                    labels
                        .into_iter()
                        .map(|label| div().h(px(20.)).child(label)),
                )
                .children(self.focus.iter().enumerate().map(|(index, focus)| {
                    div()
                        .id(("independent-field", index))
                        .h(px(24.))
                        .track_focus(focus)
                }))
        }
    }

    struct AppOwner(Rc<Cell<usize>>);

    impl Global for AppOwner {}

    impl Drop for AppOwner {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    fn draw(app: &mut App, handle: WindowHandle<Consumer>) {
        app.update_window(handle.into(), |_, window, app| window.draw(app).clear(app))
            .unwrap();
    }

    fn key(app: &mut App, handle: WindowHandle<Consumer>, source: &str, held: bool) -> bool {
        let keystroke = Keystroke::parse_with_os(source, app.platform_info().os).unwrap();
        app.update_window(handle.into(), |_, window, app| {
            window
                .dispatch_event(
                    open_gpui::PlatformInput::KeyDown(KeyDownEvent {
                        keystroke,
                        is_held: held,
                        prefer_character_input: false,
                    }),
                    app,
                )
                .propagate
        })
        .unwrap()
    }

    fn open(app: &mut App, trace: &Rc<Trace>) -> WindowHandle<Consumer> {
        app.open_window(
            WindowOptions {
                focus_on_appearing: false,
                ..Default::default()
            },
            |window, app| {
                app.new(|cx| {
                    let focus = [cx.focus_handle(), cx.focus_handle()];
                    window.focus(&focus[0], cx);
                    let pending = trace.clone();
                    let released = trace.clone();
                    cx.on_release(move |_: &mut Consumer, _| {
                        released.releases.set(released.releases.get() + 1);
                    })
                    .detach();
                    Consumer {
                        focus,
                        trace: trace.clone(),
                        _pending: cx.observe_pending_input(window, move |_, window, _| {
                            pending
                                .pending
                                .borrow_mut()
                                .push(window.has_pending_keystrokes());
                        }),
                    }
                })
            },
        )
        .unwrap()
    }

    #[test]
    fn downstream_actual_host_app_loads_routes_labels_and_retires_without_test_support() {
        let os = match std::env::consts::OS {
            "macos" => OperatingSystem::Mac,
            "windows" => OperatingSystem::Windows,
            "linux" => OperatingSystem::Linux,
            "freebsd" => OperatingSystem::FreeBsd,
            "ios" => OperatingSystem::Ios,
            "android" => OperatingSystem::Android,
            _ => OperatingSystem::Unknown,
        };
        let expected_info = PlatformInfo {
            os,
            target: PlatformTarget::Native,
        };
        let platform = bench_platform(None, Arc::new(NoopTextSystem));
        assert_eq!(platform.platform_info(), expected_info);
        let platform_lifetime = Rc::downgrade(&platform);
        let app = Application::with_platform(platform.clone())
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|_| {});
        let app_drops = Rc::new(Cell::new(0));
        let trace = Rc::new(Trace::default());
        let handle = app.update(|app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
            assert_eq!(app.platform_info(), expected_info);
            assert_eq!(
                Modifiers::command().secondary(app),
                os == OperatingSystem::Mac
            );
            assert_eq!(
                Modifiers::control().secondary(app),
                os != OperatingSystem::Mac
            );
            assert_eq!(
                Modifiers::secondary_key(app),
                if os == OperatingSystem::Mac {
                    Modifiers::command()
                } else {
                    Modifiers::control()
                },
            );
            app.set_global(AppOwner(app_drops.clone()));
            let os_name = KeyContext::new_with_defaults(os)
                .get("os")
                .unwrap()
                .to_string();
            let predicate = Rc::new(
                KeyBindingContextPredicate::parse(&format!(
                    "IndependentPlatformConsumer && os == {os_name}"
                ))
                .unwrap(),
            );
            for (source, action) in [
                ("secondary-s", Box::new(Save) as Box<dyn Action>),
                ("secondary-k secondary-c", Box::new(Chord)),
                ("ctrl-f6", Box::new(Control)),
                ("cmd-f7", Box::new(PlatformKey)),
            ] {
                let dynamic = source.to_owned();
                let binding = KeyBinding::load(&dynamic, action, None, true, None, app).unwrap();
                app.bind_keys([binding]);
            }
            let contextual =
                KeyBinding::load("f8", Box::new(Contextual), Some(predicate), true, None, app)
                    .unwrap();
            app.bind_keys([contextual]);
            open(app, &trace)
        });
        let root = app.update(|app| handle.entity(app).unwrap().downgrade());
        app.update(|app| draw(app, handle));
        let activation = app.update(|app| {
            app.update_window(handle.into(), |_, window, _| window.activate_window())
                .unwrap()
        });
        platform
            .foreground_executor()
            .dispatcher()
            .as_threaded()
            .unwrap()
            .run_until_idle();
        assert_eq!(
            activation.snapshot().status(),
            open_gpui::WindowActivationStatus::Terminal(
                open_gpui::WindowActivationTerminal::Activated
            ),
        );
        app.update(|app| {
            draw(app, handle);
            app.update_window(handle.into(), |_, window, app| {
                assert!(window.is_window_active());
                assert_eq!(app.platform_info(), expected_info);
                for (index, action) in [&Save as &dyn Action, &Chord, &Control, &PlatformKey]
                    .into_iter()
                    .enumerate()
                {
                    assert_eq!(
                        trace.labels.borrow()[index],
                        window.keystroke_text_for(action, app),
                    );
                }
            })
            .unwrap();
        });
        let expected_labels = if os == OperatingSystem::Mac {
            ["⌘S", "⌘K ⌘C", "^f6", "⌘f7"]
        } else {
            let platform = match os {
                OperatingSystem::Windows => "⊞f7",
                OperatingSystem::Linux | OperatingSystem::FreeBsd => "❖f7",
                _ => "cmd-f7",
            };
            ["ctrl-S", "ctrl-K ctrl-C", "ctrl-f6", platform]
        };
        assert_eq!(*trace.labels.borrow(), expected_labels);
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
        app.update(|app| assert!(key(app, handle, wrong, false)));
        assert!(trace.calls.borrow().is_empty());
        for (source, held) in [
            (format!("{secondary}-s"), false),
            (format!("{secondary}-s"), true),
            ("ctrl-f6".into(), false),
            ("cmd-f7".into(), false),
            ("f8".into(), false),
        ] {
            app.update(|app| assert!(!key(app, handle, &source, held)));
        }
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
        app.update(|app| assert!(!key(app, handle, &prefix, false)));
        app.update(|app| {
            app.update_window(handle.into(), |_, window, _| {
                assert_eq!(
                    window.pending_input_keystrokes(),
                    Some([Keystroke::parse_with_os(&prefix, os).unwrap()].as_slice()),
                );
            })
            .unwrap();
        });
        app.update(|app| assert!(!key(app, handle, &suffix, false)));
        assert_eq!(trace.calls.borrow().last(), Some(&("chord", 0)));
        assert_eq!(*trace.pending.borrow(), [true, false]);
        app.update(|app| assert!(!key(app, handle, &prefix, false)));
        app.update(|app| {
            handle
                .update(app, |root, window, cx| window.focus(&root.focus[1], cx))
                .unwrap();
        });
        assert_eq!(*trace.pending.borrow(), [true, false, true, false]);
        app.update(|app| {
            draw(app, handle);
            assert!(key(app, handle, &suffix, false));
            app.update_window(handle.into(), |_, window, _| {
                assert!(!window.has_pending_keystrokes());
            })
            .unwrap();
        });
        assert_eq!(trace.calls.borrow().last(), Some(&("chord", 0)));
        app.update(|app| assert!(!key(app, handle, &format!("{secondary}-s"), false)));
        assert_eq!(trace.calls.borrow().last(), Some(&("save", 1)));
        app.update(|app| assert!(!key(app, handle, &prefix, false)));
        let calls = trace.calls.borrow().clone();
        let pending = trace.pending.borrow().clone();
        let mut async_app = app.to_async();
        let mut async_window = app
            .update(|app| app.update_window(handle.into(), |_, window, app| window.to_async(app)))
            .unwrap();
        app.update(|app| {
            app.update_window(handle.into(), |_, window, app| {
                window.remove_window(app);
            })
            .unwrap();
        });
        assert!(root.upgrade().is_none());
        assert_eq!(trace.drops.get(), 1);
        assert_eq!(trace.releases.get(), 1);
        assert!(async_window.update(|_, _| ()).is_err());
        assert!(app.update(|app| app.windows().is_empty()));
        let successor_trace = Rc::new(Trace::default());
        let successor = app.update(|app| open(app, &successor_trace));
        assert_ne!(successor.window_id(), handle.window_id());
        let successor_root = app.update(|app| successor.entity(app).unwrap().downgrade());
        app.update(|app| {
            draw(app, successor);
            assert!(key(app, successor, &suffix, false));
            assert!(successor_trace.calls.borrow().is_empty());
            assert!(!key(app, successor, &format!("{secondary}-s"), false));
        });
        assert_eq!(*successor_trace.calls.borrow(), [("save", 0)]);
        app.update(|app| {
            app.update_window(successor.into(), |_, window, app| window.remove_window(app))
                .unwrap();
        });
        assert!(successor_root.upgrade().is_none());
        assert_eq!(successor_trace.drops.get(), 1);
        assert_eq!(successor_trace.releases.get(), 1);
        app.update(|app| app.shutdown());
        drop(app);
        drop(platform);
        assert_eq!(app_drops.get(), 1);
        assert!(platform_lifetime.upgrade().is_none());
        assert!(
            async_app
                .update_window(handle.into(), |_, _, _| ())
                .is_err()
        );
        assert!(async_window.update(|_, _| ()).is_err());
        assert_eq!(*trace.calls.borrow(), calls);
        assert_eq!(*trace.pending.borrow(), pending);
    }
}
