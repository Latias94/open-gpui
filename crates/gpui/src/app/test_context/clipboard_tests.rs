use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    rc::Rc,
    sync::Arc,
};

use crate::window::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    AnyWindowHandle, App, AppContext as _, Bounds, ClipboardItem, Context, Empty, Entity,
    FocusHandle, InputHandler, InteractiveElement as _, IntoElement, NativeBoundaryDiagnostic,
    NativeBoundaryDiagnosticCursor, NativeBoundaryDisposition, NativeBoundaryGeneration,
    NativeBoundaryKind, NativeBoundaryTarget, NativeCallbackKind, NativeInputBoundary,
    NativeInputHandlerOperation, NativeInputInvariantViolation, NativeInvariantFailure,
    ParentElement as _, Pixels, PlatformInputHandlerSlot, PlatformWindowCommand,
    PlatformWindowCommandOutcome, Point, Render, RenderImage, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestAppContext, UTF16Selection, WeakEntity, Window, WindowHandle,
    canvas, div, px, size,
};
use image::{Frame, ImageBuffer, Rgba};

#[derive(Clone, Copy, Default)]
enum PasteBehavior {
    #[default]
    None,
    SwitchFocus,
    RemoveWindow,
    Panic,
}

#[derive(Default)]
struct PasteState {
    behavior: Cell<PasteBehavior>,
    writes: RefCell<Vec<(&'static str, String)>>,
    painted_handlers: RefCell<Vec<&'static str>>,
    lifecycle: RefCell<Vec<&'static str>>,
    closes: Cell<usize>,
    panic_payload: Arc<()>,
}

struct PasteProbe {
    focus: [FocusHandle; 2],
    names: [&'static str; 2],
    state: Rc<PasteState>,
    image: Option<Arc<RenderImage>>,
}

struct PasteHandler {
    name: &'static str,
    next_focus: FocusHandle,
    root: WeakEntity<PasteProbe>,
    state: Rc<PasteState>,
}

impl InputHandler for PasteHandler {
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
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
        panic!("the paste lifecycle probe must receive InputHandler::paste");
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut App) {
        self.state.writes.borrow_mut().push((
            self.name,
            item.text().expect("the lifecycle paste has text"),
        ));
        self.state.lifecycle.borrow_mut().push(self.name);
        match self.state.behavior.replace(PasteBehavior::None) {
            PasteBehavior::None => {}
            PasteBehavior::SwitchFocus => {
                let revision = window.rendered_frame_revision();
                self.next_focus.focus(window, cx);
                self.root
                    .update(cx, |_, cx| cx.notify())
                    .expect("the paste probe must survive its callback");
                window.draw(cx).clear(cx);
                assert_eq!(window.rendered_frame_revision(), revision + 1);
                assert_eq!(window.committed_focus(cx).as_ref(), Some(&self.next_focus));
                self.state.lifecycle.borrow_mut().push("redrawn");
            }
            PasteBehavior::RemoveWindow => {
                window.remove_window(cx);
                window.remove_window(cx);
                assert!(
                    !window.removed,
                    "removal must wait for the paste transaction"
                );
                assert_eq!(self.state.closes.get(), 0);
                self.state.lifecycle.borrow_mut().push("remove-returned");
            }
            PasteBehavior::Panic => panic_any(self.state.panic_payload.clone()),
        }
    }

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

impl Render for PasteProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let names = self.names;
        let root = cx.entity().downgrade();
        let state = self.state.clone();
        let image = self.image.clone();
        div()
            .size_full()
            .child(
                div()
                    .id("first-paste-handler")
                    .focusable()
                    .track_focus(&self.focus[0]),
            )
            .child(
                div()
                    .id("second-paste-handler")
                    .focusable()
                    .track_focus(&self.focus[1]),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        for (index, handle) in focus.iter().enumerate() {
                            if handle.is_focused(window) {
                                state.painted_handlers.borrow_mut().push(names[index]);
                            }
                            window.handle_input(
                                handle,
                                PasteHandler {
                                    name: names[index],
                                    next_focus: focus[1].clone(),
                                    root: root.clone(),
                                    state: state.clone(),
                                },
                                cx,
                            );
                        }
                        if let Some(image) = image.as_ref() {
                            window
                                .paint_image(
                                    Bounds::new(bounds.origin, size(px(8.), px(8.))),
                                    Bounds::new(bounds.origin, size(px(8.), px(8.))),
                                    Default::default(),
                                    image.clone(),
                                    0,
                                    false,
                                )
                                .expect("the candidate image must reach atlas admission");
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

struct PasteFixture {
    window: WindowHandle<PasteProbe>,
    slot: PlatformInputHandlerSlot,
    state: Rc<PasteState>,
}

impl PasteFixture {
    fn open(cx: &mut TestAppContext) -> Self {
        let state = Rc::new(PasteState::default());
        cx.defer_next_window_frame_requests();
        let window = cx.add_window({
            let state = state.clone();
            move |_, cx| PasteProbe {
                focus: [cx.focus_handle(), cx.focus_handle()],
                names: ["first", "second"],
                state,
                image: None,
            }
        });
        let slot = cx
            .update_window(window.into(), |root, window, cx| {
                let root = root.downcast::<PasteProbe>().unwrap();
                root.update(cx, |_, cx| cx.notify());
                window.draw(cx).clear(cx);
                let focus = root.read(cx).focus[0].clone();
                focus.focus(window, cx);
                root.update(cx, |_, cx| cx.notify());
                window.draw(cx).clear(cx);
                assert_eq!(window.committed_focus(cx).as_ref(), Some(&focus));
                window
                    .platform_window
                    .input_handler_slot_for_test()
                    .expect("the test window must expose its input-handler slot")
            })
            .expect("the paste window must open");
        Self {
            window,
            slot,
            state,
        }
    }

    fn update<R>(
        &self,
        cx: &mut TestAppContext,
        update: impl FnOnce(Entity<PasteProbe>, &mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |root, window, cx| {
            update(root.downcast::<PasteProbe>().unwrap(), window, cx)
        })
        .expect("the paste window must remain live")
    }

    fn paste(&self, text: &str) {
        self.slot
            .with_handler(|handler| handler.paste(ClipboardItem::new_string(text.to_owned())))
            .expect("the admitted paste handler must be installed");
    }

    fn diagnostic(
        &self,
        cx: &TestAppContext,
        cursor: NativeBoundaryDiagnosticCursor,
        disposition: NativeBoundaryDisposition,
    ) -> NativeBoundaryDiagnostic {
        let snapshot = cx.app.native_boundary_diagnostics(cursor);
        let target = NativeBoundaryTarget::Window(self.window.window_id());
        let kind = NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInputHandler(
            NativeInputHandlerOperation::Paste,
        ));
        assert!(
            snapshot
                .pending
                .iter()
                .all(|diagnostic| diagnostic.target != target || diagnostic.kind != kind)
        );
        let diagnostics = snapshot
            .terminal
            .iter()
            .filter(|diagnostic| diagnostic.target == target && diagnostic.kind == kind)
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 1, "one paste must settle exactly once");
        let diagnostic = *diagnostics[0];
        assert_eq!(diagnostic.disposition, disposition);
        handler_generation(&diagnostic);
        diagnostic
    }
}

fn diagnostic_cursor(cx: &TestAppContext) -> NativeBoundaryDiagnosticCursor {
    cx.app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor
}

fn handler_generation(diagnostic: &NativeBoundaryDiagnostic) -> u64 {
    let Some(NativeBoundaryGeneration::InputSlot {
        boundary: NativeInputBoundary::InputHandler,
        generation,
    }) = diagnostic.domain_generation
    else {
        panic!("paste must diagnose its exact input-handler slot generation: {diagnostic:?}");
    };
    generation
}

fn observe_close(
    cx: &mut TestAppContext,
    target: AnyWindowHandle,
    state: Rc<PasteState>,
) -> Subscription {
    cx.update(move |cx| {
        cx.on_window_closed(move |cx, window_id| {
            if window_id == target.window_id() {
                state.closes.set(state.closes.get() + 1);
                state.lifecycle.borrow_mut().push("closed");
                assert!(cx.update_window_id(window_id, |_, _, _| ()).is_err());
            }
        })
    })
}

#[open_gpui::test]
fn native_paste_preserves_handler_installed_by_accepted_redraw(cx: &mut TestAppContext) {
    let fixture = PasteFixture::open(cx);
    let revision = fixture.update(cx, |_, window, _| window.rendered_frame_revision());
    fixture.state.behavior.set(PasteBehavior::SwitchFocus);
    let cursor = diagnostic_cursor(cx);

    fixture.paste("switch");

    let first = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    fixture.update(cx, |root, window, cx| {
        assert_eq!(window.rendered_frame_revision(), revision + 1);
        assert_eq!(
            window.committed_focus(cx).as_ref(),
            Some(&root.read(cx).focus[1])
        );
    });
    let cursor = diagnostic_cursor(cx);
    fixture.paste("replacement");
    let second = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert!(handler_generation(&second) > handler_generation(&first));

    let cursor = diagnostic_cursor(cx);
    fixture.paste("still-current");
    let third = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert_eq!(handler_generation(&third), handler_generation(&second));
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[
            ("first", "switch".to_owned()),
            ("second", "replacement".to_owned()),
            ("second", "still-current".to_owned()),
        ]
    );
    assert_eq!(
        fixture.state.lifecycle.borrow().as_slice(),
        &["first", "redrawn", "second", "second"]
    );
}

#[open_gpui::test]
fn native_paste_removes_window_after_callback_and_retires_once(cx: &mut TestAppContext) {
    let fixture = PasteFixture::open(cx);
    let platform_window = cx.test_window(fixture.window.into());
    let _close_subscription = observe_close(cx, fixture.window.into(), fixture.state.clone());
    fixture.state.behavior.set(PasteBehavior::RemoveWindow);
    let cursor = diagnostic_cursor(cx);

    fixture.paste("remove");
    cx.run_until_parked();

    let paste = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert!(!cx.windows().contains(&fixture.window.into()));
    assert_eq!(fixture.state.closes.get(), 1);
    assert_eq!(
        fixture.state.lifecycle.borrow().as_slice(),
        &["first", "remove-returned", "closed"]
    );
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[("first", "remove".to_owned())]
    );
    assert_eq!(platform_window.presentation_shutdown_counts().2, 1);
    assert!(platform_window.is_native_terminal());
    assert!(!platform_window.simulate_close());
    let closed = cx
        .app
        .native_boundary_diagnostics(cursor)
        .terminal
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(fixture.window.window_id())
                && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::Closed)
        })
        .collect::<Vec<_>>();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].disposition, NativeBoundaryDisposition::Closed);

    let cursor = diagnostic_cursor(cx);
    let panic = catch_unwind(AssertUnwindSafe(|| fixture.paste("retired")))
        .expect_err("a retired paste slot must reject entry");
    let violation = panic
        .downcast_ref::<NativeInputInvariantViolation>()
        .expect("retired paste must preserve its typed invariant failure");
    assert_eq!(violation.window_id, fixture.window.window_id());
    assert_eq!(violation.boundary, NativeInputBoundary::InputHandler);
    assert_eq!(violation.failure, NativeInvariantFailure::RetiredSlot);
    let snapshot = cx.app.native_boundary_diagnostics(cursor);
    assert_eq!(snapshot.terminal.len(), 1);
    let retired = snapshot.terminal[0];
    assert_eq!(
        retired.target,
        NativeBoundaryTarget::Window(fixture.window.window_id())
    );
    assert_eq!(
        retired.kind,
        NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInputHandlerSlot)
    );
    assert_eq!(
        retired.disposition,
        NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::RetiredSlot)
    );
    assert_eq!(
        Some(handler_generation(&retired)),
        violation.slot_generation
    );
    assert!(handler_generation(&retired) > handler_generation(&paste));
    assert_eq!(fixture.state.closes.get(), 1);
    assert_eq!(fixture.state.writes.borrow().len(), 1);
}

#[open_gpui::test]
fn native_paste_panic_restores_slot_and_preserves_original_payload(cx: &mut TestAppContext) {
    let fixture = PasteFixture::open(cx);
    let revision = fixture.update(cx, |_, window, _| window.rendered_frame_revision());
    fixture.state.behavior.set(PasteBehavior::Panic);
    let cursor = diagnostic_cursor(cx);

    let panic = catch_unwind(AssertUnwindSafe(|| fixture.paste("panic")))
        .expect_err("the original paste panic must escape");

    let payload = panic
        .downcast_ref::<Arc<()>>()
        .expect("paste cleanup must retain the original panic type");
    assert!(Arc::ptr_eq(payload, &fixture.state.panic_payload));
    let failed = fixture.diagnostic(
        cx,
        cursor,
        NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::CallbackPanicked),
    );
    let cursor = diagnostic_cursor(cx);
    fixture.paste("recovered");
    let recovered = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert_eq!(handler_generation(&recovered), handler_generation(&failed));
    fixture.update(cx, |_, window, _| {
        assert_eq!(window.rendered_frame_revision(), revision);
        assert!(!window.removed);
    });
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[
            ("first", "panic".to_owned()),
            ("first", "recovered".to_owned())
        ]
    );
    fixture.update(cx, |_, window, cx| {
        window.remove_window(cx);
        assert!(
            window.removed,
            "unwinding must release the paste transaction"
        );
    });
}

#[open_gpui::test]
fn native_paste_drains_older_close_before_delivery(cx: &mut TestAppContext) {
    let fixture = PasteFixture::open(cx);
    let target: AnyWindowHandle = cx.add_window(|_, _| Empty).into();
    let target_window = cx.test_window(target);
    let _close_subscription = observe_close(cx, target, fixture.state.clone());
    let cursor = diagnostic_cursor(cx);
    fixture.paste("before-close");
    let before = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    fixture.state.lifecycle.borrow_mut().clear();
    let cursor = diagnostic_cursor(cx);
    let source_window = cx.test_window(fixture.window.into());
    let app = cx.app.clone();
    let slot = fixture.slot.clone();
    let state = fixture.state.clone();
    let close_sequence = Rc::new(Cell::new(None));
    source_window.set_platform_command_callback({
        let close_sequence = close_sequence.clone();
        move |command, _| {
            if command == PlatformWindowCommand::StartWindowMove {
                assert!(target_window.simulate_close());
                let snapshot = app.native_boundary_diagnostics(cursor);
                let closed = snapshot
                    .pending
                    .iter()
                    .find(|diagnostic| {
                        diagnostic.target == NativeBoundaryTarget::Window(target.window_id())
                            && diagnostic.kind
                                == NativeBoundaryKind::Callback(NativeCallbackKind::Closed)
                    })
                    .expect("the older close must still be queued before paste enters");
                close_sequence.set(Some(closed.sequence));
                assert_eq!(state.closes.get(), 0);
                slot.with_handler(|handler| {
                    handler.paste(ClipboardItem::new_string("after-close".to_owned()));
                })
                .expect("the source paste handler must remain installed");
                assert_eq!(state.closes.get(), 1);
            }
            PlatformWindowCommandOutcome::Accepted
        }
    });

    fixture.update(cx, |_, window, _| window.start_window_move());

    let paste = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert!(close_sequence.get().unwrap() < paste.sequence);
    assert_eq!(handler_generation(&paste), handler_generation(&before));
    assert_eq!(
        fixture.state.lifecycle.borrow().as_slice(),
        &["closed", "first"]
    );
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[
            ("first", "before-close".to_owned()),
            ("first", "after-close".to_owned()),
        ]
    );
    assert!(!cx.windows().contains(&target));
    assert!(cx.windows().contains(&fixture.window.into()));
}

#[open_gpui::test]
fn native_paste_older_same_window_close_prevents_stale_writes(cx: &mut TestAppContext) {
    let fixture = PasteFixture::open(cx);
    let _close_subscription = observe_close(cx, fixture.window.into(), fixture.state.clone());
    let cursor = diagnostic_cursor(cx);
    fixture.paste("admitted");
    let admitted = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    fixture.state.lifecycle.borrow_mut().clear();
    let cursor = diagnostic_cursor(cx);
    let source_window = cx.test_window(fixture.window.into());
    let source_id = fixture.window.window_id();
    let app = cx.app.clone();
    let slot = fixture.slot.clone();
    let state = fixture.state.clone();
    let close_sequence = Rc::new(Cell::new(None));
    source_window.set_platform_command_callback({
        let close_sequence = close_sequence.clone();
        move |command, platform_window| {
            if command == PlatformWindowCommand::StartWindowMove {
                slot.with_handler(|handler| {
                    assert!(platform_window.simulate_close());
                    let snapshot = app.native_boundary_diagnostics(cursor);
                    let closed = snapshot
                        .pending
                        .iter()
                        .find(|diagnostic| {
                            diagnostic.target == NativeBoundaryTarget::Window(source_id)
                                && diagnostic.kind
                                    == NativeBoundaryKind::Callback(NativeCallbackKind::Closed)
                        })
                        .expect("the same-window close must be queued before paste enters");
                    close_sequence.set(Some(closed.sequence));
                    assert_eq!(state.closes.get(), 0);
                    let panic = catch_unwind(AssertUnwindSafe(|| {
                        handler.paste(ClipboardItem::new_string("stale".to_owned()));
                    }))
                    .expect_err("the older close must invalidate the checked-out paste target");
                    let violation = panic
                        .downcast_ref::<NativeInputInvariantViolation>()
                        .expect("the stale paste must report its exact native invariant");
                    assert_eq!(violation.window_id, source_id);
                    assert_eq!(violation.boundary, NativeInputBoundary::InputHandler);
                    assert_eq!(violation.failure, NativeInvariantFailure::StaleWindow);
                    assert_eq!(
                        violation.slot_generation,
                        Some(handler_generation(&admitted))
                    );
                })
                .expect("paste must check out the handler before native retirement");
            }
            PlatformWindowCommandOutcome::Accepted
        }
    });

    fixture.update(cx, |_, window, _| window.start_window_move());

    let paste = fixture.diagnostic(
        cx,
        cursor,
        NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::StaleWindow),
    );
    assert!(close_sequence.get().unwrap() < paste.sequence);
    assert_eq!(handler_generation(&paste), handler_generation(&admitted));
    assert_eq!(fixture.state.lifecycle.borrow().as_slice(), &["closed"]);
    assert_eq!(fixture.state.closes.get(), 1);
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[("first", "admitted".to_owned())]
    );
    assert!(!cx.windows().contains(&fixture.window.into()));
}

#[open_gpui::test]
fn native_paste_quiesced_window_rejects_checked_out_handler(cx: &mut TestAppContext) {
    let fixture = PasteFixture::open(cx);
    let cursor = diagnostic_cursor(cx);
    fixture.paste("admitted");
    let admitted = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    let cursor = diagnostic_cursor(cx);

    fixture
        .slot
        .with_handler(|handler| {
            fixture.update(cx, |_, window, cx| {
                assert!(window.quiesce_interaction(cx));
                assert!(!window.removed);
            });
            handler.paste(ClipboardItem::new_string("quiesced".to_owned()));
        })
        .expect("the handler must be checked out before interaction is revoked");

    let rejected = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::Rejected);
    assert_eq!(handler_generation(&rejected), handler_generation(&admitted));
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[("first", "admitted".to_owned())]
    );
    assert!(cx.windows().contains(&fixture.window.into()));
    assert!(fixture.slot.with_handler(|_| ()).is_none());
}

#[open_gpui::test]
fn native_paste_rejected_image_candidate_preserves_admitted_handler(cx: &mut TestAppContext) {
    struct FinishUpdate(Rc<crate::AppCell>);

    impl Drop for FinishUpdate {
        fn drop(&mut self) {
            let already_unwinding = std::thread::panicking();
            let cleanup = catch_unwind(AssertUnwindSafe(|| self.0.borrow_mut().finish_update()));
            if let Err(panic) = cleanup
                && !already_unwinding
            {
                std::panic::resume_unwind(panic);
            }
        }
    }

    let fixture = PasteFixture::open(cx);
    let atlas = Arc::new(RejectOnceAtlas::new());
    let cursor = diagnostic_cursor(cx);
    fixture.paste("before-candidate");
    let admitted = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    cx.app.borrow_mut().start_update();
    let _finish_update = FinishUpdate(cx.app.clone());
    fixture.state.painted_handlers.borrow_mut().clear();
    let revision = fixture.update(cx, |root, window, cx| {
        window.set_sprite_atlas_for_test(atlas.clone());
        root.update(cx, |root, cx| {
            root.names[0] = "candidate";
            root.image = Some(Arc::new(RenderImage::new(smallvec::smallvec![Frame::new(
                ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0xff])),
            )])));
            cx.notify();
        });
        let revision = window.rendered_frame_revision();
        atlas.fail_next_lease();
        root.update(cx, |_, cx| cx.notify());
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), revision);
        assert!(window.rendered_frame_image_paint_diagnostics().is_empty());
        revision
    });
    assert_eq!(
        fixture.state.painted_handlers.borrow().as_slice(),
        &["candidate"]
    );

    let cursor = diagnostic_cursor(cx);
    fixture.paste("after-rejection");
    let restored = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert!(handler_generation(&restored) > handler_generation(&admitted));
    fixture.update(cx, |_, window, _| {
        assert_eq!(window.rendered_frame_revision(), revision);
    });
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[
            ("first", "before-candidate".to_owned()),
            ("first", "after-rejection".to_owned()),
        ]
    );

    fixture.update(cx, |root, window, cx| {
        root.update(cx, |_, cx| cx.notify());
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), revision + 1);
        assert_eq!(window.rendered_frame_image_paint_diagnostics().len(), 1);
    });
    let cursor = diagnostic_cursor(cx);
    fixture.paste("accepted-candidate");
    let accepted = fixture.diagnostic(cx, cursor, NativeBoundaryDisposition::DELIVERED);
    assert!(handler_generation(&accepted) > handler_generation(&restored));
    assert_eq!(
        fixture.state.writes.borrow().as_slice(),
        &[
            ("first", "before-candidate".to_owned()),
            ("first", "after-rejection".to_owned()),
            ("candidate", "accepted-candidate".to_owned()),
        ]
    );
    assert_eq!(
        fixture.state.painted_handlers.borrow().as_slice(),
        &["candidate", "candidate"]
    );
}
