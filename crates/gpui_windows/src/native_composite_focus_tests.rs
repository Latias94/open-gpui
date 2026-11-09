//! Public composite behavior on a real Windows Application, HWND, renderer and text system.
//!
//! Synthetic keys enter the installed production native input callback outside App borrows;
//! synthetic accessibility activation uses the installed production activation handler. Exact
//! nonempty Submitted generations qualify the accepted tree, not OS input, AT consumption,
//! physical foreground focus, UIA event receipt or compositor scanout.
//! This keyboard-only fixture declines native pointer input so incidental desktop mouse messages
//! cannot change input modality during its passive-frame assertions.

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use accesskit::{ActivationHandler, Role};
use open_gpui::{
    ClickEvent, DispatchEventResult, FocusHandle, Global, InteractiveElement, KeyDownEvent,
    KeyUpEvent, Keystroke, NativeInputDeliveryResult, StatefulInteractiveElement, TargetedEvent,
    WindowHandle, WindowInitialPresentationStatus, black,
    prelude::FluentBuilder as _,
    private::serde_json::{self, Value},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompositeSource {
    role: Role,
    open: bool,
    selected: usize,
    openings: usize,
    confirmations: usize,
    version: usize,
}

struct CompositeAppLifetime {
    _token: Rc<()>,
}
impl Global for CompositeAppLifetime {}

struct NativeComposite {
    source: CompositeSource,
    focus: [FocusHandle; 4],
    renders: Rc<Cell<usize>>,
}

impl Render for NativeComposite {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let source = format!("{:?}", self.source);
        let option_role = match self.source.role {
            Role::Menu => Role::MenuItem,
            _ => Role::ListBoxOption,
        };
        div()
            .id("native-composite-source")
            .role(Role::Group)
            .aria_label("Composite source")
            .aria_value(source.clone())
            .size_full()
            .bg(white())
            .text_color(black())
            .capture_key_down(|event, window, cx| {
                if event.keystroke.key == "x" {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .id("native-composite-trigger")
                    .role(Role::Button)
                    .aria_label("Composite trigger")
                    .track_focus(&self.focus[0])
                    .tab_index(0)
                    .on_click(
                        cx.listener(|root, event: &TargetedEvent<ClickEvent>, window, cx| {
                            assert!(event.window_event().is_keyboard());
                            root.source.open = true;
                            root.source.openings += 1;
                            window.focus(&root.focus[1], cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child("Open composite"),
            )
            .when(self.source.open, |view| {
                view.child(
                    div()
                        .id("native-composite-container")
                        .role(self.source.role)
                        .aria_label("Composite container")
                        .track_focus(&self.focus[1])
                        .tab_group()
                        .tab_stop(false)
                        .on_key_down(cx.listener(|root, event: &KeyDownEvent, window, cx| {
                            match event.keystroke.key.as_str() {
                                "down" => root.source.selected = 1 - root.source.selected,
                                "enter" => {
                                    root.source.confirmations += 1;
                                    root.source.open = false;
                                    window.focus(&root.focus[0], cx);
                                }
                                _ => return,
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .children(["Choice A", "Choice B"].into_iter().enumerate().map(
                            |(index, label)| {
                                div()
                                    .id(("native-composite-choice", index))
                                    .role(option_role)
                                    .aria_label(label)
                                    .aria_selected(index == self.source.selected)
                                    .when(index == self.source.selected, |choice| {
                                        choice.aria_active_descendant()
                                    })
                                    .child(label)
                            },
                        ))
                        .child(
                            div()
                                .id("native-composite-tab-child")
                                .role(Role::Button)
                                .aria_label("Composite tab child")
                                .track_focus(&self.focus[3])
                                .tab_index(0)
                                .child("Tab target"),
                        ),
                )
            })
            .child(
                div()
                    .id("native-composite-unrelated")
                    .role(Role::Button)
                    .aria_label("Unrelated focus")
                    .track_focus(&self.focus[2])
                    .tab_index(0)
                    .child("Unrelated focus"),
            )
            .child(source)
    }
}

fn labelled<'a>(snapshot: &'a Value, label: &str) -> (&'a str, &'a Value) {
    let mut matches = snapshot["nodes"]
        .as_object()
        .expect("accepted nodes")
        .iter()
        .filter(|(_, node)| node["aria"]["label"] == label);
    let (id, node) = matches.next().expect("the labelled node must be published");
    assert!(
        matches.next().is_none(),
        "ambiguous composite label: {label}"
    );
    (id, node)
}

fn checkpoint(
    app: &mut Application,
    handle: WindowHandle<NativeComposite>,
    expected: CompositeSource,
    focus: usize,
    generation: &mut u64,
) -> Value {
    let mut accepted = None;
    pump_messages_until_with_delay(
        &format!("composite Submitted after {generation}: {expected:?}, focus {focus}"),
        Duration::from_millis(5),
        || {
            accepted = app
                .update_for_test(|cx| {
                    handle.update(cx, |root, window, cx| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        let receipt = window.submitted_frame_diagnostics_for_test()?;
                        let raw = window.debug_a11y_tree_json()?;
                        let snapshot: Value =
                            serde_json::from_str(&raw).expect("official tree JSON");
                        (root.source == expected
                            && window.focused(cx) == Some(root.focus[focus].clone())
                            && window.committed_focus(cx) == Some(root.focus[focus].clone())
                            && window.is_a11y_active()
                            && attempt.generation > *generation
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && receipt.generation() == attempt.generation
                            && snapshot["frame"]["frame_generation"].as_u64()
                                == Some(attempt.generation)
                            && labelled(&snapshot, "Composite source").1["aria"]["value"]
                                == format!("{expected:?}"))
                        .then_some(snapshot)
                    })
                })
                .expect("the exact composite window must remain live");
            accepted.is_some()
        },
    );
    let snapshot =
        accepted.expect("the source and accepted tree must share a real Submitted frame");
    *generation = snapshot["frame"]["frame_generation"].as_u64().unwrap();
    assert!(snapshot["frame"]["activation_generation"].as_u64().unwrap() > 0);
    assert_eq!(
        snapshot["frame"]["node_count"].as_u64(),
        Some(snapshot["nodes"].as_object().unwrap().len() as u64)
    );
    let focus_label = [
        "Composite trigger",
        "Composite container",
        "Unrelated focus",
        "Composite tab child",
    ][focus];
    assert_eq!(snapshot["gpui_focus"], labelled(&snapshot, focus_label).0);
    if expected.open {
        let (container_id, container) = labelled(&snapshot, "Composite container");
        assert_eq!(container["aria"]["role"], format!("{:?}", expected.role));
        for (index, label) in ["Choice A", "Choice B"].into_iter().enumerate() {
            let (id, node) = labelled(&snapshot, label);
            assert!(
                container["children"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|child| child == id)
            );
            assert_eq!(node["aria"]["selected"], index == expected.selected);
            assert_eq!(
                node["aria"]["role"],
                if expected.role == Role::Menu {
                    "MenuItem"
                } else {
                    "ListBoxOption"
                }
            );
        }
        if focus == 1 {
            let selected = labelled(&snapshot, ["Choice A", "Choice B"][expected.selected]).0;
            assert_ne!(container_id, selected);
            assert_eq!(snapshot["active_descendant_focus"], selected);
        }
    } else {
        assert!(
            snapshot["nodes"]
                .as_object()
                .unwrap()
                .values()
                .all(|node| !matches!(
                    node["aria"]["label"].as_str(),
                    Some("Composite container" | "Choice A" | "Choice B" | "Composite tab child")
                )),
            "a closed composite must leave no stale semantic subtree"
        );
    }
    if focus != 1 {
        assert!(snapshot["active_descendant_focus"].is_null());
    }
    snapshot
}

fn key(
    app: &mut Application,
    handle: WindowHandle<NativeComposite>,
    native: &WindowsWindowInner,
    key: &str,
    down: bool,
) -> DispatchEventResult {
    let (cursor, revision) = app.update_for_test(|cx| {
        (
            cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
                .cursor,
            handle
                .update(cx, |_, window, _| window.key_event_revision())
                .unwrap(),
        )
    });
    let keystroke = Keystroke::parse(key)
        .expect("composite key")
        .with_simulated_ime();
    let input = if down {
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        })
    } else {
        PlatformInput::KeyUp(KeyUpEvent { keystroke })
    };
    let result = native.dispatch_input(input);
    app.update_for_test(|cx| {
        let next_revision = handle
            .update(cx, |_, window, _| window.key_event_revision())
            .unwrap();
        assert_eq!(next_revision, revision.wrapping_add(1));
        let diagnostics = cx.native_boundary_diagnostics(cursor);
        assert_eq!(diagnostics.omitted_before_cursor, 0);
        let delivered = diagnostics
            .terminal
            .iter()
            .filter(|entry| {
                entry.target == NativeBoundaryTarget::Window(handle.window_id())
                    && entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
            })
            .collect::<Vec<_>>();
        assert_eq!(delivered.len(), 1, "one production native callback per key");
        assert_eq!(
            delivered[0].disposition,
            NativeBoundaryDisposition::Delivered {
                input_result: Some(NativeInputDeliveryResult {
                    propagate: result.propagate,
                    default_prevented: result.default_prevented,
                }),
            }
        );
        assert!(diagnostics.terminal.iter().all(|entry| !matches!(
            entry.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )));
    });
    result
}

fn passive(
    app: &mut Application,
    handle: WindowHandle<NativeComposite>,
    snapshot: &Value,
    renders: &Cell<usize>,
    requests: &Cell<usize>,
) {
    let read = |app: &mut Application| {
        app.update_for_test(|cx| {
            handle.update(cx, |root, window, _| {
                (
                    window.debug_a11y_tree_json(),
                    window.presentation_facts(),
                    format!("{:?}", root.source),
                    window.focus_claim_revision(),
                    window.key_event_revision(),
                )
            })
        })
        .unwrap()
    };
    let before = read(app);
    assert_eq!(
        serde_json::from_str::<Value>(before.0.as_deref().unwrap()).unwrap(),
        *snapshot
    );
    assert_eq!(
        before.2,
        labelled(snapshot, "Composite source").1["aria"]["value"]
            .as_str()
            .unwrap()
    );
    let counts = (renders.get(), requests.get());
    for _ in 0..3 {
        assert_eq!(read(app), before);
    }
    pump_messages_until_with_delay(
        "passive composite frame opportunities",
        Duration::from_millis(5),
        || requests.get() >= counts.1 + 2,
    );
    assert_eq!(read(app), before);
    assert_eq!(renders.get(), counts.0);
}

#[test]
fn native_composite_focus_keyboard_continuity_and_release() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let cursor = observation.cursor();
    let platform = Rc::new(WindowsPlatform::new(false).expect("native composite Windows backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let text_lifetime = Arc::downgrade(&app.text_system());
    let app_lifetime = app.update_for_test(|cx| {
        let token = Rc::new(());
        let lifetime = Rc::downgrade(&token);
        cx.set_global(CompositeAppLifetime { _token: token });
        lifetime
    });
    platform.begin_vsync_thread();
    let renders = Rc::new(Cell::new(0));
    let mut expected = CompositeSource {
        role: Role::Menu,
        open: false,
        selected: 0,
        openings: 0,
        confirmations: 0,
        version: 0,
    };
    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(480.), px(300.)), cx)),
                    focus_on_appearing: false,
                    accepts_pointer_input: false,
                    show: true,
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let focus: [FocusHandle; 4] = std::array::from_fn(|_| cx.focus_handle());
                        window.focus(&focus[0], cx);
                        NativeComposite {
                            source: expected,
                            focus,
                            renders: renders.clone(),
                        }
                    })
                },
            )
        })
        .expect("native composite window");
    let root_lifetime = app.update_for_test(|cx| handle.root(cx).unwrap().downgrade());
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("exact composite registration");
    let hwnd = registration.as_raw();
    let native = platform
        .window_from_hwnd(hwnd)
        .expect("exact native composite owner");
    assert_eq!(native.registration, registration);
    let native_lifetime = Rc::downgrade(&native);
    let requests = observe_frame_requests(&native);
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert!(!window.accepts_pointer_input());
            assert!(!window.is_a11y_active());
            assert!(window.debug_a11y_tree_json().is_none());
        })
    })
    .unwrap();
    let initial = native
        .state
        .a11y
        .borrow_mut()
        .as_mut()
        .expect("installed native a11y")
        .activation_handler
        .request_initial_tree()
        .expect("synchronous initial Window snapshot");
    assert_eq!(initial.nodes.len(), 1);
    assert_eq!(initial.nodes[0].1.role(), Role::Window);
    assert_eq!(initial.focus, accesskit::NodeId(0));
    let mut generation = 0;
    let mut snapshot = checkpoint(&mut app, handle, expected, 0, &mut generation);
    passive(&mut app, handle, &snapshot, &renders, &requests);

    for role in [Role::Menu, Role::ListBox] {
        if expected.role != role {
            expected.role = role;
            app.update_for_test(|cx| {
                handle.update(cx, |root, _, cx| {
                    root.source.role = role;
                    cx.notify();
                })
            })
            .unwrap();
            checkpoint(&mut app, handle, expected, 0, &mut generation);
        }
        for opening_key in ["space", "enter"] {
            key(&mut app, handle, &native, opening_key, true);
            app.update_for_test(|cx| {
                handle.update(cx, |root, window, _| {
                    assert_eq!(root.source, expected);
                    let raw = window.debug_a11y_tree_json();
                    let revision = window.key_event_revision();
                    for _ in 0..3 {
                        assert_eq!(window.debug_a11y_tree_json(), raw);
                        assert_eq!(window.key_event_revision(), revision);
                    }
                })
            })
            .unwrap();
            key(&mut app, handle, &native, opening_key, false);
            expected.open = true;
            expected.openings += 1;
            checkpoint(&mut app, handle, expected, 1, &mut generation);
            if opening_key == "space" {
                key(&mut app, handle, &native, "down", true);
                key(&mut app, handle, &native, "down", false);
                expected.selected = 1;
                checkpoint(&mut app, handle, expected, 1, &mut generation);
                expected.selected = 0;
                expected.version += 1;
                app.update_for_test(|cx| {
                    handle.update(cx, |root, _, cx| {
                        root.source.selected = 0;
                        root.source.version += 1;
                        cx.notify();
                    })
                })
                .unwrap();
                let snapshot = checkpoint(&mut app, handle, expected, 1, &mut generation);
                passive(&mut app, handle, &snapshot, &renders, &requests);
                for focus in [2, 1] {
                    app.update_for_test(|cx| {
                        handle.update(cx, |root, window, cx| {
                            window.focus(&root.focus[focus], cx);
                        })
                    })
                    .unwrap();
                    checkpoint(&mut app, handle, expected, focus, &mut generation);
                }
                key(&mut app, handle, &native, "tab", true);
                key(&mut app, handle, &native, "tab", false);
                checkpoint(&mut app, handle, expected, 3, &mut generation);
                key(&mut app, handle, &native, "shift-tab", true);
                key(&mut app, handle, &native, "shift-tab", false);
                checkpoint(&mut app, handle, expected, 2, &mut generation);
                app.update_for_test(|cx| {
                    handle.update(cx, |root, window, cx| {
                        window.focus(&root.focus[1], cx);
                    })
                })
                .unwrap();
                checkpoint(&mut app, handle, expected, 1, &mut generation);
            }
            key(&mut app, handle, &native, "enter", true);
            expected.open = false;
            expected.confirmations += 1;
            snapshot = checkpoint(&mut app, handle, expected, 0, &mut generation);
            for _ in 0..2 {
                key(&mut app, handle, &native, "enter", false);
            }
            passive(&mut app, handle, &snapshot, &renders, &requests);
        }
    }
    for activation_key in ["space", "enter"] {
        key(&mut app, handle, &native, activation_key, true);
        app.update_for_test(|cx| {
            handle.update(cx, |root, window, cx| {
                for focus in [2, 0] {
                    window.focus(&root.focus[focus], cx);
                    assert_eq!(window.focused(cx), Some(root.focus[focus].clone()));
                }
            })
        })
        .unwrap();
        snapshot = checkpoint(&mut app, handle, expected, 0, &mut generation);
        key(&mut app, handle, &native, activation_key, false);
        passive(&mut app, handle, &snapshot, &renders, &requests);
        key(&mut app, handle, &native, activation_key, true);
        let stopped = key(&mut app, handle, &native, "x", true);
        assert!(
            !stopped.propagate && stopped.default_prevented,
            "the other key must stop in capture, before the trigger"
        );
        key(&mut app, handle, &native, activation_key, false);
        key(&mut app, handle, &native, "x", false);
        passive(&mut app, handle, &snapshot, &renders, &requests);
    }

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, cx| {
            window.remove_window(cx);
            assert!(window.debug_a11y_tree_json().is_none());
        })
    })
    .expect("close the exact composite owner");
    pump_messages_until_with_delay(
        "exact composite HWND retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !is_registered(&platform, hwnd)
                && platform.inner.native_retirement_is_settled()
        },
    );
    let lifecycle = observation
        .events_after(cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                assert_ne!(disposition, NativeWindowTestMessageDisposition::Panicked);
                None
            }
            lifecycle => Some(lifecycle),
        })
        .collect::<Vec<_>>();
    let [
        NativeWindowTestEventKind::PresentationQuiesced {
            ticket_generation: quiesced,
        },
        NativeWindowTestEventKind::DestroyEntered {
            ticket_generation: destroyed,
        },
        NativeWindowTestEventKind::NativeTerminal {
            ticket_generation: terminal,
        },
    ] = lifecycle.as_slice()
    else {
        panic!("unexpected composite retirement: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(app.update_for_test(|cx| handle.update(cx, |_, _, _| ()).is_err()));
    assert!(platform.raw_window_handles.read().is_empty());
    assert!(root_lifetime.upgrade().is_none());
    assert_eq!(Rc::strong_count(&renders), 1);
    drop(native);
    assert!(native_lifetime.upgrade().is_none());
    assert!(app_lifetime.upgrade().is_some());
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 0);
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    assert!(app_lifetime.upgrade().is_none());
    drop(platform);
    pump_messages_until_with_delay(
        "composite App and platform release",
        Duration::from_millis(5),
        || {
            platform_lifetime.upgrade().is_none()
                && !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
        },
    );
    assert!(text_lifetime.upgrade().is_none());
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
