use std::{borrow::Cow, collections::VecDeque, ops::Range};

use open_gpui::SharedString;
use unicode_segmentation::UnicodeSegmentation as _;

const HISTORY_LIMIT: usize = 64;

#[derive(Clone)]
struct Snapshot {
    text: SharedString,
    anchor: usize,
    head: usize,
}

struct Composition {
    original: Snapshot,
    span: Range<usize>,
}

pub(super) struct TextBuffer {
    state: Snapshot,
    composition: Option<Composition>,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
}

impl TextBuffer {
    pub(super) fn new(value: SharedString) -> Self {
        let text = normalize(value);
        let end = text.len();
        Self {
            state: Snapshot {
                text,
                anchor: end,
                head: end,
            },
            composition: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }

    pub(super) fn value(&self) -> &SharedString {
        self.composition
            .as_ref()
            .map_or(&self.state.text, |ime| &ime.original.text)
    }

    pub(super) fn text(&self) -> &SharedString {
        &self.state.text
    }

    pub(super) fn marked(&self) -> Option<Range<usize>> {
        self.composition.as_ref().map(|ime| ime.span.clone())
    }

    pub(super) fn head(&self) -> usize {
        self.state.head
    }

    pub(super) fn reversed(&self) -> bool {
        self.state.head < self.state.anchor
    }

    pub(super) fn selection(&self) -> Range<usize> {
        self.state.anchor.min(self.state.head)..self.state.anchor.max(self.state.head)
    }

    pub(super) fn select(&mut self, anchor: usize, head: usize) {
        self.state.anchor = char_boundary(&self.state.text, anchor);
        self.state.head = char_boundary(&self.state.text, head);
    }

    pub(super) fn move_to(&mut self, head: usize, extend: bool) {
        self.select(if extend { self.state.anchor } else { head }, head);
    }

    pub(super) fn adjacent_grapheme(&self, forward: bool) -> usize {
        let head = self.head();
        let mut graphemes = self.text().grapheme_indices(true);
        if forward {
            graphemes
                .find_map(|(offset, _)| (offset > head).then_some(offset))
                .unwrap_or(self.text().len())
        } else {
            graphemes
                .rev()
                .find_map(|(offset, _)| (offset < head).then_some(offset))
                .unwrap_or(0)
        }
    }

    pub(super) fn grapheme_boundary(&self, offset: usize) -> usize {
        if offset >= self.text().len() {
            return self.text().len();
        }
        self.text()
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index <= offset).then_some(index))
            .unwrap_or(0)
    }

    pub(super) fn select_word(&mut self, offset: usize) {
        let text = self.text();
        let offset = offset.min(text.len().saturating_sub(1));
        if let Some((start, word)) = text
            .split_word_bound_indices()
            .find(|(start, word)| *start + word.len() > offset)
        {
            self.select(start, start + word.len());
        }
    }

    pub(super) fn byte_range(&self, utf16: Range<usize>) -> Range<usize> {
        byte_range(self.text(), utf16)
    }

    pub(super) fn utf16_range(&self, bytes: Range<usize>) -> Range<usize> {
        self.utf16_offset(bytes.start)..self.utf16_offset(bytes.end)
    }

    pub(super) fn utf16_offset(&self, byte: usize) -> usize {
        self.text()[..char_boundary(self.text(), byte)]
            .encode_utf16()
            .count()
    }

    pub(super) fn replace(&mut self, utf16: Option<Range<usize>>, text: &str) -> bool {
        if text.is_empty() && utf16.is_none() && self.cancel_composition() {
            return false;
        }
        let range = self.replacement_range(utf16);
        let original = self
            .composition
            .take()
            .map_or_else(|| self.state.clone(), |ime| ime.original);
        let inserted = self.splice(range, &single_line(text));
        self.move_to(inserted.end, false);
        self.commit(original)
    }

    pub(super) fn mark(
        &mut self,
        utf16: Option<Range<usize>>,
        text: &str,
        relative_selection: Option<Range<usize>>,
    ) {
        if text.is_empty() {
            self.cancel_composition();
            return;
        }
        let range = self.replacement_range(utf16);
        let original = self
            .composition
            .take()
            .map_or_else(|| self.state.clone(), |ime| ime.original);
        let text = single_line(text);
        let span = self.splice(range, &text);
        let selection = relative_selection.map_or(span.end..span.end, |utf16| {
            let relative = byte_range(&text, utf16);
            span.start + relative.start..span.start + relative.end
        });
        self.select(selection.start, selection.end);
        self.composition = Some(Composition { original, span });
    }

    pub(super) fn unmark(&mut self) -> bool {
        let Some(ime) = self.composition.take() else {
            return false;
        };
        self.commit(ime.original)
    }

    pub(super) fn cancel_composition(&mut self) -> bool {
        let Some(ime) = self.composition.take() else {
            return false;
        };
        self.state = ime.original;
        true
    }

    pub(super) fn cancel_composition_for_caret(&mut self, display_offset: Option<usize>) -> usize {
        let display_len = self.state.text.len();
        let Some(ime) = self.composition.take() else {
            return self.head();
        };
        self.state = ime.original;
        let Some(offset) = display_offset else {
            return self.head();
        };
        let restored_offset = if offset >= ime.span.end {
            let suffix_len = display_len.saturating_sub(offset);
            self.state.text.len().saturating_sub(suffix_len)
        } else {
            offset.min(ime.span.start)
        };
        self.grapheme_boundary(restored_offset)
    }

    pub(super) fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.state, previous));
        true
    }

    pub(super) fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push_back(std::mem::replace(&mut self.state, next));
        true
    }

    fn replacement_range(&self, utf16: Option<Range<usize>>) -> Range<usize> {
        utf16
            .map(|range| self.byte_range(range))
            .or_else(|| self.marked())
            .unwrap_or_else(|| self.selection())
    }

    fn splice(&mut self, range: Range<usize>, value: &str) -> Range<usize> {
        let text = self.text();
        if &text[range.clone()] != value {
            let mut edited = String::with_capacity(text.len() - range.len() + value.len());
            edited.push_str(&text[..range.start]);
            edited.push_str(value);
            edited.push_str(&text[range.end..]);
            self.state.text = edited.into();
        }
        range.start..range.start + value.len()
    }

    fn commit(&mut self, before: Snapshot) -> bool {
        if before.text == self.state.text {
            return false;
        }
        self.redo.clear();
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(before);
        true
    }
}

pub(super) fn normalize(text: SharedString) -> SharedString {
    match single_line(&text) {
        Cow::Owned(normalized) => normalized.into(),
        Cow::Borrowed(_) => text,
    }
}

fn single_line(text: &str) -> Cow<'_, str> {
    let separators = ['\r', '\n', '\u{0085}', '\u{2028}', '\u{2029}'];
    if text.contains(separators) {
        Cow::Owned(text.replace(separators, " "))
    } else {
        Cow::Borrowed(text)
    }
}

fn char_boundary(text: &str, byte: usize) -> usize {
    text.floor_char_boundary(byte.min(text.len()))
}

fn byte_range(text: &str, utf16: Range<usize>) -> Range<usize> {
    let mut start = None;
    let mut units = 0;
    let end = utf16.end.max(utf16.start);
    for (byte, character) in text.char_indices() {
        let next = units + character.len_utf16();
        if utf16.start < next {
            start.get_or_insert(byte);
        }
        if utf16.start == end
            && let Some(start) = start
        {
            return start..start;
        }
        if end <= units {
            return start.unwrap_or(byte)..byte;
        }
        units = next;
    }
    start.unwrap_or(text.len())..text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_selection_is_relative_and_its_commit_is_one_undo() {
        let mut buffer = TextBuffer::new("前😀後".into());
        buffer.select(7, 3);
        buffer.mark(None, "𠮷野", Some(2..3));
        assert_eq!(buffer.value().as_ref(), "前😀後");
        assert_eq!(buffer.text().as_ref(), "前𠮷野後");
        assert_eq!(buffer.utf16_range(buffer.selection()), 3..4);
        assert_eq!(buffer.marked(), Some(3..10));
        buffer.mark(None, "よしの", Some(1..1));
        assert!(buffer.replace(None, "吉野"));
        assert_eq!(buffer.value().as_ref(), "前吉野後");
        assert!(buffer.undo());
        assert!(buffer.reversed());
        assert_eq!(buffer.selection(), 3..7);
        assert_eq!(buffer.value().as_ref(), "前😀後");
        assert!(buffer.redo());
        assert_eq!(buffer.value().as_ref(), "前吉野後");
    }

    #[test]
    fn empty_composition_restores_selected_content() {
        let mut buffer = TextBuffer::new("abc".into());
        buffer.select(1, 2);
        buffer.mark(None, "β", None);
        assert!(!buffer.replace(None, ""));
        assert_eq!(buffer.text().as_ref(), "abc");
        assert_eq!(buffer.selection(), 1..2);
        assert!(!buffer.undo());
    }

    #[test]
    fn canceled_composition_rebases_the_clicked_caret() {
        for (display_offset, expected_utf16) in [
            (Some(0), 0),
            (Some(4), 1),
            (Some(7), 3),
            (Some(8), 4),
            (None, 3),
        ] {
            let mut buffer = TextBuffer::new("a😀b".into());
            buffer.select(1, 5);
            buffer.mark(None, "候補", None);
            let caret = buffer.cancel_composition_for_caret(display_offset);
            assert_eq!(buffer.value().as_ref(), "a😀b");
            assert_eq!(buffer.utf16_offset(caret), expected_utf16);
            assert!(buffer.marked().is_none());
            assert!(!buffer.undo());
        }
    }

    #[test]
    fn utf16_ranges_adjust_inside_surrogates_and_clamp() {
        assert_eq!(byte_range("a😀z", 2..2), 1..1);
        assert_eq!(byte_range("a😀z", 1..2), 1..5);
        assert_eq!(byte_range("a😀z", 2..3), 1..5);
        assert_eq!(byte_range("a😀z", 99..usize::MAX), 6..6);
    }

    #[test]
    fn keyboard_stops_on_graphemes_and_input_stays_single_line() {
        let mut buffer = TextBuffer::new("e\u{301}😀\n後".into());
        assert_eq!(buffer.value().as_ref(), "e\u{301}😀 後");
        buffer.move_to(0, false);
        assert_eq!(buffer.adjacent_grapheme(true), 3);
        assert!(buffer.replace(Some(0..2), "one\rtwo\u{2028}three"));
        assert_eq!(buffer.value().as_ref(), "one two three😀 後");
    }
}
