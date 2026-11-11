//! A small text receiver exercising the ordinary native input handler.
//!
//! Native text services use a field-level anchor; this receiver does not implement editor
//! caret layout or pointer hit testing.

use super::super::*;
use open_gpui::{
    ElementInputHandler, EntityInputHandler, FocusHandle, Pixels, Point, UTF16Selection,
};
use std::ops::Range;

pub(super) struct HostInput {
    pub(super) focus: FocusHandle,
    pub(super) text: String,
    pub(super) key_events: u32,
    pub(super) count: Rc<Cell<u32>>,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
}

impl HostInput {
    pub(super) fn new(count: Rc<Cell<u32>>, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            text: String::new(),
            key_events: 0,
            count,
            selection: 0..0,
            marked: None,
        }
    }

    fn byte_offset(&self, utf16: usize) -> usize {
        let mut offset = 0;
        for (byte, character) in self.text.char_indices() {
            if offset >= utf16 {
                return byte;
            }
            offset += character.len_utf16();
        }
        self.text.len()
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .or(self.marked.take())
            .unwrap_or(self.selection.clone());
        let start = self.byte_offset(range.start);
        let end = self.byte_offset(range.end).max(start);
        let start_utf16 = self.text[..start].encode_utf16().count();
        self.text.replace_range(start..end, text);
        let end_utf16 = start_utf16 + text.encode_utf16().count();
        self.selection = end_utf16..end_utf16;
        start_utf16..end_utf16
    }
}

impl Render for HostInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let view = cx.entity();
        div()
            .id("host-text-input")
            .relative()
            .w_full()
            .min_h(px(40.))
            .p_2()
            .bg(rgb(0x344358))
            .track_focus(&self.focus)
            .on_mouse_down(
                open_gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window, cx);
                    cx.notify();
                }),
            )
            .on_key_down(
                cx.listener(|this, event: &open_gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "f6" {
                        this.key_events += 1;
                        this.count.set(this.count.get().wrapping_add(1));
                        window.refresh();
                        cx.notify();
                    }
                }),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, view.clone()),
                            cx,
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(format!(
                "Type here; F6 changes the GPU image: {}",
                self.text
            ))
    }
}

impl EntityInputHandler for HostInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.byte_offset(range.start);
        let end = self.byte_offset(range.end).max(start);
        *adjusted = Some(
            self.text[..start].encode_utf16().count()..self.text[..end].encode_utf16().count(),
        );
        Some(self.text[start..end].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text);
        self.marked = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let marked = self.replace(range, text);
        self.selection = selection
            .map(|range| {
                (marked.start + range.start).min(marked.end)
                    ..(marked.start + range.end).min(marked.end)
            })
            .unwrap_or(marked.end..marked.end);
        self.marked = (!marked.is_empty()).then_some(marked);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
