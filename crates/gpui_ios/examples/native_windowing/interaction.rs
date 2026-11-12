use crate::ios::{native_view, submitted_drawable};
use objc2::{msg_send, rc::Retained};
use objc2_foundation::{NSRange, NSString};
use objc2_quartz_core::CAMetalLayer;
use objc2_ui_kit::{UITextPosition, UITextRange};
use open_gpui::{
    App, AppContext, AsyncApp, Bounds, Context, FocusHandle, InputHandler, IntoElement, Pixels,
    Platform, Point, Render, UTF16Selection, Window, WindowActivationTerminal, WindowOptions,
    canvas, div, fill, prelude::*, rgb,
};
use open_gpui_ios::IosPlatform;
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Document {
    text: Vec<u16>,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
    marks: usize,
    unmarks: usize,
    quiesce_on_mark: bool,
}

impl Document {
    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let text = text.encode_utf16().collect::<Vec<_>>();
        let inserted = range.start..range.start + text.len();
        self.text.splice(range, text);
        self.selection = inserted.end..inserted.end;
        self.marked = None;
        inserted
    }
}

struct DocumentHandler {
    document: Rc<RefCell<Document>>,
    bounds: Bounds<Pixels>,
}

impl InputHandler for DocumentHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.document.borrow().selection.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        self.document.borrow().marked.clone()
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        String::from_utf16(self.document.borrow().text.get(range)?).ok()
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        _: &mut App,
    ) {
        self.document.borrow_mut().replace(range, text);
        window.refresh();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let quiesce = {
            let mut document = self.document.borrow_mut();
            let inserted = document.replace(range, text);
            if let Some(selection) = selection {
                document.selection =
                    inserted.start + selection.start..inserted.start + selection.end;
            }
            document.marked = Some(inserted);
            document.marks += 1;
            std::mem::take(&mut document.quiesce_on_mark)
        };
        if quiesce {
            window.quiesce_interaction(cx);
        }
        window.refresh();
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {
        let mut document = self.document.borrow_mut();
        document.marked = None;
        document.unmarks += 1;
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        Some(
            ((f32::from(point.x - self.bounds.origin.x).max(0.) / 8.) as usize)
                .min(self.document.borrow().text.len()),
        )
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut App) -> Option<usize> {
        Some(self.document.borrow().text.len())
    }
}

struct InteractionProbe {
    focus: FocusHandle,
    document: Rc<RefCell<Document>>,
    positive_observations: Rc<Cell<usize>>,
}

impl Render for InteractionProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let document = self.document.clone();
        div().size_full().track_focus(&self.focus).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    window.paint_quad(fill(bounds, rgb(0xeeeeee)));
                    window.handle_input(
                        &focus,
                        DocumentHandler {
                            document: document.clone(),
                            bounds,
                        },
                        cx,
                    );
                },
            )
            .size_full(),
        )
    }
}

pub(super) async fn run(cx: &mut AsyncApp, platform: &IosPlatform, deadline: Instant) {
    for quiesce_on_mark in [false, true] {
        let document = Rc::new(RefCell::new(Document {
            quiesce_on_mark,
            ..Default::default()
        }));
        let positives = Rc::new(Cell::new(0));
        let window = cx
            .open_window(WindowOptions::default(), {
                let document = document.clone();
                let positives = positives.clone();
                move |window, cx| {
                    cx.new(|cx| {
                        cx.observe_window_activation(
                            window,
                            |this: &mut InteractionProbe, window, _| {
                                if window.is_window_active() {
                                    this.positive_observations
                                        .set(this.positive_observations.get() + 1);
                                }
                            },
                        )
                        .detach();
                        let focus = cx.focus_handle();
                        focus.focus(window, cx);
                        InteractionProbe {
                            focus,
                            document,
                            positive_observations: positives,
                        }
                    })
                }
            })
            .expect("open a native composition and quiescence probe");
        let view = window
            .update(cx, |_, window, _| native_view(window))
            .expect("retain the real GPUI rendering view");
        let native_window = view.window().expect("the native window is attached");
        let input = view
            .subviews()
            .firstObject()
            .expect("the native text responder exists");
        loop {
            let selection: Option<Retained<UITextRange>> =
                unsafe { msg_send![&*input, selectedTextRange] };
            let active = window
                .update(cx, |_, window, _| window.is_window_active())
                .expect("the composition probe remains live");
            if active
                && native_window.isKeyWindow()
                && input.isFirstResponder()
                && selection.is_some()
                && submitted_drawable(&view).0 > 0
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the real text handler never became native first responder"
            );
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
        let old_position: Retained<UITextPosition> =
            unsafe { msg_send![&*input, beginningOfDocument] };
        let old_range: Option<Retained<UITextRange>> =
            unsafe { msg_send![&*input, selectedTextRange] };
        let old_range = old_range.expect("the live native document has a selection");
        let text = NSString::from_str("候選");
        unsafe {
            let _: () =
                msg_send![&*input, setMarkedText: Some(&*text), selectedRange: NSRange::new(2, 0)];
        }
        assert_eq!(
            document.borrow().marks,
            1,
            "the real native protocol must reach Core's accepted handler"
        );
        if !quiesce_on_mark {
            assert!(document.borrow().marked.is_some());
            assert_eq!(document.borrow().unmarks, 0);
            cx.update_window(window.into(), |_, window, cx| {
                assert!(window.quiesce_interaction(cx));
            })
            .expect("quiesce an idle handler through Core");
        }
        assert!(document.borrow().marked.is_none());
        assert_eq!(
            document.borrow().unmarks,
            1,
            "Core must finish both idle and checked-out composition exactly once"
        );
        assert_eq!(String::from_utf16(&document.borrow().text).unwrap(), "候選");
        let positive_count = positives.get();
        let original_text = document.borrow().text.clone();
        assert!(!input.isFirstResponder());
        assert!(!input.canBecomeFirstResponder());
        assert!(!input.becomeFirstResponder());
        assert!(!native_window.isKeyWindow());
        assert!(!native_window.canBecomeKeyWindow());
        assert!(!native_window.isUserInteractionEnabled());
        assert!(!view.isUserInteractionEnabled());
        native_window.makeKeyWindow();
        native_window.becomeKeyWindow();
        assert!(
            !native_window.isKeyWindow(),
            "native key entry cannot reopen quiesced interaction"
        );
        let replacement = NSString::from_str("rejected");
        unsafe {
            let _: () = msg_send![&*input, insertText: &*replacement];
            let _: () = msg_send![&*input, replaceRange: &*old_range, withText: &*replacement];
            let stale: Option<Retained<UITextRange>> = msg_send![&*input, textRangeFromPosition: &*old_position, toPosition: &*old_position];
            assert!(
                stale.is_none(),
                "terminal FocusLost must invalidate the native text session"
            );
        }
        let baseline_serial = submitted_drawable(&view).0;
        let rejected = cx
            .update_window(window.into(), |_, window, cx| {
                assert!(
                    !window.quiesce_interaction(cx),
                    "Core revocation is idempotent"
                );
                assert!(!window.is_window_active());
                assert!(window.focused(cx).is_none());
                window.request_virtual_keyboard();
                window.refresh();
                window.activate_window()
            })
            .expect("the inert window remains available for painting");
        assert_eq!(
            rejected.snapshot().status().terminal(),
            Some(WindowActivationTerminal::Rejected)
        );
        assert!(
            !platform
                .active_window()
                .is_some_and(|active| active.window_id() == window.window_id())
        );
        assert!(
            !platform
                .focused_window()
                .window()
                .is_some_and(|active| active.window_id() == window.window_id())
        );
        loop {
            assert_eq!(document.borrow().text, original_text);
            assert_eq!(document.borrow().unmarks, 1);
            assert_eq!(
                positives.get(),
                positive_count,
                "late native activation must not publish a new positive"
            );
            assert!(!input.canBecomeFirstResponder());
            if submitted_drawable(&view).0 > baseline_serial {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "interaction quiescence stopped native presentation"
            );
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
        assert!(!native_window.isHidden());
        assert!(native_window.rootViewController().is_some());
        assert!(view.superview().is_some());
        let layer = view.layer();
        let layer = layer
            .downcast_ref::<CAMetalLayer>()
            .expect("the view retains its original Metal layer");
        assert!(
            layer.device().is_some(),
            "interaction revocation preserves renderer ownership"
        );
        let terminal = Rc::new(Cell::new(0));
        let _terminal_subscription = cx.update(|cx| {
            cx.on_window_native_terminal({
                let terminal = terminal.clone();
                move |_, id| {
                    if id == window.window_id() {
                        terminal.set(terminal.get() + 1);
                    }
                }
            })
        });
        cx.update_window(window.into(), |_, window, cx| window.remove_window(cx))
            .expect("retire the inert window through Core's exact owner");
        while terminal.get() == 0 {
            assert!(
                Instant::now() < deadline,
                "the inert window did not reach native terminal"
            );
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
        assert_eq!(terminal.get(), 1);
        assert!(layer.device().is_none());
        assert!(native_window.isHidden());
    }
}
