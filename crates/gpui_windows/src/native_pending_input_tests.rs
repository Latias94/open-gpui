//! Native consumer coverage for the pending-input regressions exercised by Core's fake clock.
//!
//! A public Render view uses the real WindowsPlatform, DirectWrite, DX11 and HWND. Keyboard
//! events are synthetic PlatformInput values delivered through the native window's production
//! input callback. Explicit WM_CHAR messages exercise the installed native text-handler slot;
//! they do not prove OS keyboard translation or a live IME composition session. The native
//! handler's leased IME-preference query is exercised directly. Timeout behavior belongs to
//! Core's deterministic tests, so this consumer never waits for the one-second chord deadline.

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    Action, App, FocusHandle, InputHandler, InteractiveElement, KeyBinding, KeyContext,
    KeyDownEvent, Keystroke, Modifiers, ModifiersChangedEvent, NativeInputHandlerOperation, Pixels,
    Point, Subscription, UTF16Selection, WindowHandle, WindowInitialPresentationStatus, black,
};
use std::ops::Range;
use windows::Win32::UI::WindowsAndMessaging::WM_CHAR;

open_gpui::actions!(
    native_pending_input,
    [NativeModifierAction, NativeChordAction]
);

#[derive(Default)]
struct NativePendingTextState {
    text: RefCell<String>,
    ime_queries: Cell<usize>,
    handlers_created: Cell<usize>,
    handlers_retired: Cell<usize>,
}

impl NativePendingTextState {
    fn live_handlers(&self) -> usize {
        self.handlers_created.get() - self.handlers_retired.get()
    }
}

struct NativePendingTextHandler(Rc<NativePendingTextState>);

impl NativePendingTextHandler {
    fn new(state: Rc<NativePendingTextState>) -> Self {
        state.handlers_created.set(state.handlers_created.get() + 1);
        Self(state)
    }
}

impl Drop for NativePendingTextHandler {
    fn drop(&mut self) {
        self.0
            .handlers_retired
            .set(self.0.handlers_retired.get() + 1);
    }
}

impl InputHandler for NativePendingTextHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        let end = self.0.text.borrow().encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        let text = self.0.text.borrow().encode_utf16().collect::<Vec<_>>();
        String::from_utf16(text.get(range)?).ok()
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        _: &mut App,
    ) {
        assert!(range.is_none(), "the consumer only appends committed text");
        self.0.text.borrow_mut().push_str(text);
        window.refresh();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
        panic!("live IME composition is outside this synthetic consumer's scope");
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

    fn prefers_ime_for_printable_keys(&mut self, _: &mut Window, _: &mut App) -> bool {
        self.0.ime_queries.set(self.0.ime_queries.get() + 1);
        true
    }
}

struct NativePendingInputRoot {
    primary_focus: FocusHandle,
    secondary_focus: FocusHandle,
    primary: Rc<NativePendingTextState>,
    secondary: Rc<NativePendingTextState>,
    modifier_actions: Rc<Cell<usize>>,
    chord_actions: Rc<Cell<usize>>,
    _pending_subscription: Subscription,
}

fn native_pending_text_field(
    id: &'static str,
    focus: FocusHandle,
    input: Rc<NativePendingTextState>,
) -> impl IntoElement {
    let label = format!("{id}: {}", input.text.borrow());
    div()
        .id(id)
        .track_focus(&focus)
        .h(px(64.))
        .child(label)
        .child(
            canvas(
                |_, _, _| {},
                move |_, _, window, cx| {
                    if focus.is_focused(window) {
                        window.handle_input(&focus, NativePendingTextHandler::new(input), cx);
                    }
                },
            )
            .size(px(1.)),
        )
}

impl Render for NativePendingInputRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(white())
            .text_color(black())
            .key_context("NativePendingInput")
            .on_action(cx.listener(|this, _: &NativeModifierAction, _, _| {
                this.modifier_actions.set(this.modifier_actions.get() + 1);
            }))
            .on_action(cx.listener(|this, _: &NativeChordAction, _, _| {
                this.chord_actions.set(this.chord_actions.get() + 1);
            }))
            .child(native_pending_text_field(
                "primary-input",
                self.primary_focus.clone(),
                self.primary.clone(),
            ))
            .child(native_pending_text_field(
                "secondary-input",
                self.secondary_focus.clone(),
                self.secondary.clone(),
            ))
    }
}

#[derive(Debug, PartialEq)]
struct NativePendingKeystroke {
    keystroke: Keystroke,
    action: Option<&'static str>,
    context_stack: Vec<KeyContext>,
}

fn wait_for_native_pending_input_submission(
    platform: &WindowsPlatform,
    app: &mut Application,
    handle: WindowHandle<NativePendingInputRoot>,
    previous_generation: u64,
) -> u64 {
    let mut submitted = None;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native pending-input accepted nonempty submission",
        Duration::from_millis(5),
        || {
            let (facts, rendered_generation, diagnostics_generation) = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        (
                            window.presentation_facts(),
                            window.rendered_frame_revision(),
                            window
                                .submitted_frame_diagnostics_for_test()
                                .map(|diagnostics| diagnostics.generation()),
                        )
                    })
                })
                .expect("the native input consumer must remain open until submission");
            let Some(attempt) = facts.latest_present_attempt else {
                return false;
            };
            if attempt.generation > previous_generation
                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                && attempt.contained_valid_primitives
                && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                && facts.native_visible
                && facts.frame_accepted_generation == Some(attempt.generation)
                && facts.present_submitted_generation == Some(attempt.generation)
                && facts.non_empty_presented_generation == Some(attempt.generation)
                && rendered_generation == attempt.generation
                && diagnostics_generation == Some(attempt.generation)
            {
                submitted = Some(attempt.generation);
                true
            } else {
                false
            }
        },
    );
    submitted.expect("the native input consumer must submit its accepted, nonempty frame")
}

#[test]
fn native_pending_input_routes_modifiers_chords_and_focus_between_real_text_handlers() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let observation_cursor = observation.cursor();
    let platform = Rc::new(
        WindowsPlatform::new_windowed_for_test()
            .expect("the native DirectWrite/DirectX input platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let weak_platform_text_system = Arc::downgrade(&platform.text_system());
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let weak_text_system = Arc::downgrade(&app.text_system());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");

    let primary = Rc::new(NativePendingTextState::default());
    let secondary = Rc::new(NativePendingTextState::default());
    let modifier_actions = Rc::new(Cell::new(0));
    let chord_actions = Rc::new(Cell::new(0));
    let pending_changes = Rc::new(RefCell::new(Vec::new()));
    let handle = app
        .update_for_test(|cx| {
            cx.bind_keys([
                KeyBinding::new("shift", NativeModifierAction, Some("NativePendingInput")),
                KeyBinding::new("ctrl-x k", NativeChordAction, Some("NativePendingInput")),
                KeyBinding::new("j k", NativeChordAction, Some("NativePendingInput")),
            ]);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(480.), px(200.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let primary_focus = cx.focus_handle();
                        let secondary_focus = cx.focus_handle();
                        window.focus(&primary_focus, cx);
                        let changes = pending_changes.clone();
                        let pending_subscription =
                            cx.observe_pending_input(window, move |_, window, _| {
                                changes.borrow_mut().push(window.has_pending_keystrokes());
                            });
                        NativePendingInputRoot {
                            primary_focus,
                            secondary_focus,
                            primary: primary.clone(),
                            secondary: secondary.clone(),
                            modifier_actions: modifier_actions.clone(),
                            chord_actions: chord_actions.clone(),
                            _pending_subscription: pending_subscription,
                        }
                    })
                },
            )
        })
        .expect("the native pending-input consumer must open");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|registration| registration.window_id() == handle.window_id())
        .expect("the input consumer must own its exact HWND registration");
    let hwnd = registration.as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("the input consumer HWND must remain registered");
    assert_eq!(native_window.registration, registration);
    let input_slot = native_window.state.input_handler.clone();
    let initial_paint_cursor = observation.cursor();
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native pending-input initialization WM_PAINT",
        Duration::from_millis(5),
        || {
            observation.any_after(initial_paint_cursor, |event| {
                event.window().window_id() == registration.window_id()
                    && event.window().native_generation() == registration.generation()
                    && matches!(
                        event.kind(),
                        NativeWindowTestEventKind::WindowMessage {
                            message: NativeWindowTestMessage::Other(WM_PAINT),
                            disposition: NativeWindowTestMessageDisposition::Returned(0),
                            ..
                        }
                    )
            })
        },
    );
    let mut generation = wait_for_native_pending_input_submission(&platform, &mut app, handle, 0);
    assert_eq!(primary.live_handlers(), 1);
    assert_eq!(secondary.handlers_created.get(), 0);
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(true)
    );
    assert_eq!(primary.ime_queries.get(), 1);
    app.update_for_test(|cx| {
        handle.update(cx, |root, window, cx| {
            assert_eq!(window.focused(cx), Some(root.primary_focus.clone()));
            assert!(!window.has_pending_keystrokes());
            assert!(window.pending_input_keystrokes().is_none());
        })
    })
    .expect("the rendered primary input must own semantic focus");

    let intercepted = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let consume_modifier = Rc::new(Cell::new(true));
    let context = KeyContext::parse("NativePendingInput").expect("valid native key context");
    let (_interceptor, _observer) = app.update_for_test(|cx| {
        let intercepted = intercepted.clone();
        let observed = observed.clone();
        let consume_modifier = consume_modifier.clone();
        (
            cx.intercept_keystrokes(move |event, _, cx| {
                intercepted.borrow_mut().push(NativePendingKeystroke {
                    keystroke: event.keystroke.clone(),
                    action: event.action.as_ref().map(|action| action.name()),
                    context_stack: event.context_stack.clone(),
                });
                if consume_modifier.get() && event.keystroke.key == "shift" {
                    cx.stop_propagation();
                }
            }),
            cx.observe_keystrokes(move |event, _, _| {
                observed.borrow_mut().push(NativePendingKeystroke {
                    keystroke: event.keystroke.clone(),
                    action: event.action.as_ref().map(|action| action.name()),
                    context_stack: event.context_stack.clone(),
                });
            }),
        )
    });
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let modifier_change = |modifiers| {
        native_window.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
            modifiers,
            ..Default::default()
        }))
    };
    let os = app.update_for_test(|cx| cx.platform_info().os);
    let key_down = |source| {
        native_window.dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse_with_os(source, os)
                .expect("valid synthetic native consumer keystroke")
                .with_simulated_ime(),
            is_held: false,
            prefer_character_input: false,
        }))
    };

    assert!(modifier_change(Modifiers::shift()).propagate);
    assert!(intercepted.borrow().is_empty());
    assert!(observed.borrow().is_empty());
    assert!(!modifier_change(Modifiers::none()).propagate);
    let shift = Keystroke::parse("shift").expect("valid standalone modifier");
    assert_eq!(
        intercepted.borrow().as_slice(),
        &[NativePendingKeystroke {
            keystroke: shift.clone(),
            action: None,
            context_stack: vec![context.clone()],
        }]
    );
    assert_eq!(modifier_actions.get(), 0);
    assert!(observed.borrow().is_empty());

    consume_modifier.set(false);
    assert!(modifier_change(Modifiers::shift()).propagate);
    assert_eq!(intercepted.borrow().len(), 1);
    assert!(!modifier_change(Modifiers::none()).propagate);
    assert_eq!(intercepted.borrow().len(), 2);
    assert_eq!(intercepted.borrow()[0], intercepted.borrow()[1]);
    assert_eq!(modifier_actions.get(), 1);
    assert_eq!(
        observed.borrow().as_slice(),
        &[NativePendingKeystroke {
            keystroke: shift,
            action: Some(NativeModifierAction.name()),
            context_stack: vec![context.clone()],
        }]
    );

    let pending_before_chord = pending_changes.borrow().len();
    assert!(!key_down("ctrl-x").propagate);
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert!(window.has_pending_keystrokes());
            assert_eq!(
                window.pending_input_keystrokes(),
                Some([Keystroke::parse("ctrl-x").expect("valid chord prefix")].as_slice())
            );
        })
    })
    .expect("the primary input must retain its chord prefix");
    let queries_before_pending = primary.ime_queries.get();
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(false)
    );
    assert_eq!(primary.ime_queries.get(), queries_before_pending);
    assert_eq!(observed.borrow().len(), 1);
    assert!(!key_down("k").propagate);
    assert_eq!(chord_actions.get(), 1);
    assert_eq!(
        observed.borrow()[1],
        NativePendingKeystroke {
            keystroke: Keystroke::parse("k")
                .expect("valid chord suffix")
                .with_simulated_ime(),
            action: Some(NativeChordAction.name()),
            context_stack: vec![context.clone()],
        }
    );
    assert_eq!(
        &pending_changes.borrow()[pending_before_chord..],
        &[true, false]
    );
    app.update_for_test(|cx| {
        handle.update(cx, |root, window, cx| {
            assert_eq!(window.focused(cx), Some(root.primary_focus.clone()));
            assert!(!window.has_pending_keystrokes());
            assert!(window.pending_input_keystrokes().is_none());
        })
    })
    .expect("completing the chord must clear its pending input without changing focus");
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(true)
    );
    assert!(primary.text.borrow().is_empty());
    assert!(secondary.text.borrow().is_empty());

    assert!(!key_down("j").propagate);
    assert_eq!(observed.borrow().len(), 2);
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(false)
    );
    let changes_before_switch = pending_changes.borrow().len();
    app.update_for_test(|cx| {
        handle.update(cx, |root, window, cx| {
            assert!(window.has_pending_keystrokes());
            window.focus(&root.secondary_focus, cx);
            assert_eq!(window.focused(cx), Some(root.secondary_focus.clone()));
            assert!(!window.has_pending_keystrokes());
            assert!(window.pending_input_keystrokes().is_none());
            assert_eq!(pending_changes.borrow().len(), changes_before_switch);
        })
    })
    .expect("switching to the rendered sibling input must settle the old chord");
    assert_eq!(pending_changes.borrow().len(), changes_before_switch + 1);
    assert_eq!(pending_changes.borrow().last(), Some(&false));
    generation = wait_for_native_pending_input_submission(&platform, &mut app, handle, generation);
    assert_eq!(primary.live_handlers(), 0);
    assert!(secondary.handlers_created.get() > 0);
    assert_eq!(secondary.live_handlers(), 1);
    let primary_queries_before_switch = primary.ime_queries.get();
    let secondary_queries_before_switch = secondary.ime_queries.get();
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(true)
    );
    assert_eq!(primary.ime_queries.get(), primary_queries_before_switch);
    assert_eq!(
        secondary.ime_queries.get(),
        secondary_queries_before_switch + 1
    );
    assert!(primary.text.borrow().is_empty());
    assert!(secondary.text.borrow().is_empty());
    assert!(key_down("k").propagate);
    assert_eq!(chord_actions.get(), 1);
    assert_eq!(observed.borrow().len(), 3);
    assert_eq!(
        observed.borrow()[2],
        NativePendingKeystroke {
            keystroke: Keystroke::parse("k")
                .expect("valid new input key")
                .with_simulated_ime(),
            action: None,
            context_stack: vec![context.clone()],
        }
    );
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_CHAR, Some(WPARAM('k' as usize)), Some(LPARAM(1))) }.0,
        0
    );
    assert_eq!(secondary.text.borrow().as_str(), "k");
    assert!(primary.text.borrow().is_empty());
    generation = wait_for_native_pending_input_submission(&platform, &mut app, handle, generation);

    assert!(!key_down("ctrl-x").propagate);
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(false)
    );
    let changes_before_blur = pending_changes.borrow().len();
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, cx| {
            assert!(window.has_pending_keystrokes());
            window.blur(cx);
            window.blur(cx);
            assert!(window.focused(cx).is_none());
            assert!(!window.has_pending_keystrokes());
            assert!(window.pending_input_keystrokes().is_none());
            assert_eq!(pending_changes.borrow().len(), changes_before_blur);
        })
    })
    .expect("blur must clear pending input before notifying the consumer");
    assert_eq!(pending_changes.borrow().len(), changes_before_blur + 1);
    assert_eq!(pending_changes.borrow().last(), Some(&false));
    wait_for_native_pending_input_submission(&platform, &mut app, handle, generation);
    assert_eq!(primary.live_handlers(), 0);
    assert_eq!(secondary.live_handlers(), 0);
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        None
    );
    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.blur(cx)))
        .expect("repeated empty blur must remain valid");
    assert_eq!(pending_changes.borrow().len(), changes_before_blur + 1);
    assert_eq!(modifier_actions.get(), 1);
    assert_eq!(chord_actions.get(), 1);
    assert!(primary.text.borrow().is_empty());
    assert_eq!(secondary.text.borrow().as_str(), "k");
    assert_eq!(observed.borrow().len(), 3);
    assert!(
        intercepted
            .borrow()
            .iter()
            .all(|event| { event.action.is_none() && event.context_stack == [context.clone()] })
    );

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact native pending-input consumer must close");
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "exact native pending-input registration retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !is_registered(&platform, hwnd)
                && platform.inner.native_retirement_is_settled()
        },
    );
    let lifecycle = observation
        .events_after(observation_cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                assert_ne!(
                    disposition,
                    NativeWindowTestMessageDisposition::Panicked,
                    "the native input boundary must not swallow a panic"
                );
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
        panic!("unexpected native pending-input retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|diagnostic| {
        !matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )
    }));
    let target = NativeBoundaryTarget::Window(handle.window_id());
    for propagate in [false, true] {
        assert!(diagnostics.terminal.iter().any(|diagnostic| {
            diagnostic.target == target
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
                && matches!(
                    diagnostic.disposition,
                    NativeBoundaryDisposition::Delivered { input_result: Some(result) }
                        if result.propagate == propagate
                )
        }));
    }
    for operation in [
        NativeInputHandlerOperation::PrefersImeForPrintableKeys,
        NativeInputHandlerOperation::ReplaceTextInRange,
    ] {
        assert!(diagnostics.terminal.iter().any(|diagnostic| {
            diagnostic.target == target
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInputHandler(
                        operation,
                    ))
                && matches!(
                    diagnostic.disposition,
                    NativeBoundaryDisposition::Delivered { .. }
                )
        }));
    }
    assert!(app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        cx.windows().is_empty()
    }));
    assert!(platform.raw_window_handles.read().is_empty());
    for input in [&primary, &secondary] {
        assert!(input.handlers_created.get() > 0);
        assert_eq!(input.handlers_created.get(), input.handlers_retired.get());
    }
    drop(input_slot);
    drop(native_window);

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "native pending-input platform message window retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform_text_system.upgrade().is_none());
}
