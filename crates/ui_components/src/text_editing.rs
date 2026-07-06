//! Shared renderer-neutral text editing behavior for GPUI text adapters.

use std::{borrow::Cow, ops::Range};

use unicode_segmentation::UnicodeSegmentation;

/// Mask glyph used by password-style display projections.
pub(crate) const TEXT_PASSWORD_MASK_CHAR: char = '•';

/// Text display policy for editable text documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TextDisplayPolicy {
    /// Display stored text as-is.
    #[default]
    Plain,
    /// Display one mask glyph per stored grapheme.
    Masked,
}

/// Newline normalization policy for text edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TextNewlinePolicy {
    /// Replace CR/LF input with spaces.
    #[default]
    SingleLine,
    /// Preserve newlines while normalizing CRLF/CR to LF.
    MultiLine,
}

/// Editing policy for a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextEditingPolicy {
    newline_policy: TextNewlinePolicy,
}

impl TextEditingPolicy {
    /// Returns a single-line plain text editing policy.
    pub(crate) const fn single_line() -> Self {
        Self {
            newline_policy: TextNewlinePolicy::SingleLine,
        }
    }

    /// Returns a multiline plain text editing policy.
    pub(crate) const fn multiline() -> Self {
        Self {
            newline_policy: TextNewlinePolicy::MultiLine,
        }
    }

    /// Normalizes input text according to this policy.
    pub(crate) fn normalize_text(self, text: &str) -> String {
        match self.newline_policy {
            TextNewlinePolicy::SingleLine => sanitize_single_line(text),
            TextNewlinePolicy::MultiLine => normalize_multiline(text),
        }
    }
}

/// Byte-range text selection plus direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextSelection {
    range: Range<usize>,
    reversed: bool,
}

impl TextSelection {
    /// Creates a collapsed selection at a byte offset.
    pub(crate) fn caret(offset: usize) -> Self {
        Self {
            range: offset..offset,
            reversed: false,
        }
    }

    /// Creates a normalized selection from anchor and active byte offsets.
    pub(crate) fn from_offsets(anchor: usize, active: usize) -> Self {
        Self {
            range: anchor.min(active)..anchor.max(active),
            reversed: active < anchor,
        }
    }

    /// Returns the selected byte range.
    pub(crate) fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Returns whether the selection is directionally reversed.
    pub(crate) const fn reversed(&self) -> bool {
        self.reversed
    }

    /// Returns whether no bytes are selected.
    pub(crate) fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Returns the active cursor byte offset.
    pub(crate) fn cursor_offset(&self) -> usize {
        if self.reversed {
            self.range.start
        } else {
            self.range.end
        }
    }

    /// Clamps this selection to character boundaries for the provided text.
    pub(crate) fn clamp_to_text(&self, text: &str) -> Self {
        let start = clamp_to_char_boundary(text, self.range.start);
        let end = clamp_to_char_boundary(text, self.range.end);
        Self {
            range: start.min(end)..start.max(end),
            reversed: self.reversed,
        }
    }
}

/// Result of applying a text edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextEditingProjection {
    text: String,
    selection: TextSelection,
    marked_range: Option<Range<usize>>,
}

impl TextEditingProjection {
    /// Returns the edited text.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Returns the edited selection.
    pub(crate) fn selection(&self) -> &TextSelection {
        &self.selection
    }

    /// Returns the active marked range.
    pub(crate) fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
    }
}

/// Editable text document state independent of GPUI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditableTextDocument {
    text: String,
    selection: TextSelection,
    marked_range: Option<Range<usize>>,
    policy: TextEditingPolicy,
}

impl EditableTextDocument {
    /// Creates a document with selection at the end of normalized text.
    pub(crate) fn new(text: impl AsRef<str>, policy: TextEditingPolicy) -> Self {
        let text = policy.normalize_text(text.as_ref());
        let end = text.len();
        Self {
            text,
            selection: TextSelection::caret(end),
            marked_range: None,
            policy,
        }
    }

    /// Creates a document from existing controller state.
    pub(crate) fn from_parts(
        text: impl AsRef<str>,
        selection: TextSelection,
        marked_range: Option<Range<usize>>,
        policy: TextEditingPolicy,
    ) -> Self {
        let text = policy.normalize_text(text.as_ref());
        let selection = selection.clamp_to_text(text.as_str());
        let marked_range = marked_range
            .map(|range| {
                let start = clamp_to_char_boundary(text.as_str(), range.start);
                let end = clamp_to_char_boundary(text.as_str(), range.end);
                start.min(end)..start.max(end)
            })
            .filter(|range| !range.is_empty());

        Self {
            text,
            selection,
            marked_range,
            policy,
        }
    }

    /// Returns the previous grapheme boundary before an offset.
    pub(crate) fn previous_grapheme_boundary(&self, offset: usize) -> usize {
        previous_grapheme_boundary(self.text.as_str(), offset)
    }

    /// Returns the next grapheme boundary after an offset.
    pub(crate) fn next_grapheme_boundary(&self, offset: usize) -> usize {
        next_grapheme_boundary(self.text.as_str(), offset)
    }

    /// Moves the caret to a byte offset.
    pub(crate) fn move_to(mut self, offset: usize) -> TextEditingProjection {
        let offset = clamp_to_char_boundary(self.text.as_str(), offset);
        self.selection = TextSelection::caret(offset);
        self.marked_range = None;
        self.into_projection()
    }

    /// Extends selection to a byte offset.
    pub(crate) fn select_to(mut self, offset: usize) -> TextEditingProjection {
        let offset = clamp_to_char_boundary(self.text.as_str(), offset);
        let anchor = if self.selection.reversed {
            self.selection.range.end
        } else {
            self.selection.range.start
        };
        self.selection = TextSelection::from_offsets(anchor, offset);
        self.marked_range = None;
        self.into_projection()
    }

    /// Replaces a UTF-16 range, current marked range, or selected range.
    pub(crate) fn replace_text_in_range(
        mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
    ) -> TextEditingProjection {
        let range = range_utf16
            .as_ref()
            .map(|range| range_from_utf16(self.text.as_str(), range))
            .or(self.marked_range.clone())
            .unwrap_or_else(|| self.selection.range());
        let new_text = self.policy.normalize_text(new_text);
        let new_end = range.start + new_text.len();

        self.text = self.text[0..range.start].to_owned() + &new_text + &self.text[range.end..];
        self.selection = TextSelection::caret(new_end);
        self.marked_range = None;
        self.into_projection()
    }

    /// Replaces text and marks the inserted range for IME composition.
    pub(crate) fn replace_and_mark_text_in_range(
        mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        selected_utf16: Option<Range<usize>>,
    ) -> TextEditingProjection {
        let range = range_utf16
            .as_ref()
            .map(|range| range_from_utf16(self.text.as_str(), range))
            .or(self.marked_range.clone())
            .unwrap_or_else(|| self.selection.range());
        let new_text = self.policy.normalize_text(new_text);
        let new_end = range.start + new_text.len();

        self.text = self.text[0..range.start].to_owned() + &new_text + &self.text[range.end..];
        self.marked_range = (!new_text.is_empty()).then_some(range.start..new_end);
        self.selection = selected_utf16
            .as_ref()
            .map(|selected| {
                TextSelection::from_offsets(
                    range.start + offset_from_utf16(&new_text, selected.start),
                    range.start + offset_from_utf16(&new_text, selected.end),
                )
            })
            .unwrap_or_else(|| TextSelection::caret(new_end));
        self.into_projection()
    }

    /// Deletes the previous grapheme or current selection.
    pub(crate) fn delete_backward(mut self) -> Option<TextEditingProjection> {
        if self.selection.is_empty() {
            let previous = self.previous_grapheme_boundary(self.selection.cursor_offset());
            if previous == self.selection.cursor_offset() {
                return None;
            }
            self.selection = TextSelection::from_offsets(previous, self.selection.cursor_offset());
        }
        Some(self.replace_text_in_range(None, ""))
    }

    /// Deletes the next grapheme or current selection.
    pub(crate) fn delete_forward(mut self) -> Option<TextEditingProjection> {
        if self.selection.is_empty() {
            let next = self.next_grapheme_boundary(self.selection.cursor_offset());
            if next == self.selection.cursor_offset() {
                return None;
            }
            self.selection = TextSelection::from_offsets(self.selection.cursor_offset(), next);
        }
        Some(self.replace_text_in_range(None, ""))
    }

    pub(crate) fn into_projection(self) -> TextEditingProjection {
        TextEditingProjection {
            text: self.text,
            selection: self.selection,
            marked_range: self.marked_range,
        }
    }
}

/// Display projection for stored text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextDisplayProjection<'a> {
    stored_text: &'a str,
    display_text: Cow<'a, str>,
    display_policy: TextDisplayPolicy,
}

impl<'a> TextDisplayProjection<'a> {
    /// Creates a display projection for the given policy.
    pub(crate) fn for_policy(text: &'a str, display_policy: TextDisplayPolicy) -> Self {
        match display_policy {
            TextDisplayPolicy::Plain => Self::plain(text),
            TextDisplayPolicy::Masked => Self::masked(text),
        }
    }

    /// Creates an identity display projection.
    pub(crate) fn plain(text: &'a str) -> Self {
        Self {
            stored_text: text,
            display_text: Cow::Borrowed(text),
            display_policy: TextDisplayPolicy::Plain,
        }
    }

    /// Creates a masked display projection.
    pub(crate) fn masked(text: &'a str) -> Self {
        let display_text = text
            .graphemes(true)
            .map(|_| TEXT_PASSWORD_MASK_CHAR)
            .collect::<String>();
        Self {
            stored_text: text,
            display_text: Cow::Owned(display_text),
            display_policy: TextDisplayPolicy::Masked,
        }
    }

    /// Returns the display text.
    pub(crate) fn display_text(&self) -> Cow<'a, str> {
        self.display_text.clone()
    }

    /// Maps a stored byte offset to a display byte offset.
    pub(crate) fn stored_to_display_offset(&self, offset: usize) -> usize {
        match self.display_policy {
            TextDisplayPolicy::Plain => {
                debug_assert_eq!(self.stored_text, self.display_text.as_ref());
                clamp_to_char_boundary(self.display_text.as_ref(), offset)
            }
            TextDisplayPolicy::Masked => {
                let offset = clamp_to_grapheme_boundary(self.stored_text, offset);
                self.stored_text[..offset].graphemes(true).count()
                    * TEXT_PASSWORD_MASK_CHAR.len_utf8()
            }
        }
    }

    /// Maps a display byte offset to a stored byte offset.
    pub(crate) fn display_to_stored_offset(&self, offset: usize) -> usize {
        match self.display_policy {
            TextDisplayPolicy::Plain => {
                debug_assert_eq!(self.stored_text, self.display_text.as_ref());
                clamp_to_char_boundary(self.stored_text, offset)
            }
            TextDisplayPolicy::Masked => {
                if offset >= self.display_text.len() {
                    return self.stored_text.len();
                }
                let offset = clamp_to_char_boundary(self.display_text.as_ref(), offset);
                let grapheme_count = self.display_text[..offset].chars().count();
                stored_offset_after_graphemes(self.stored_text, grapheme_count)
            }
        }
    }

    /// Maps a stored byte range to a display byte range.
    pub(crate) fn stored_range_to_display_range(&self, range: &Range<usize>) -> Range<usize> {
        self.stored_to_display_offset(range.start)..self.stored_to_display_offset(range.end)
    }
}

/// Normalizes text for single-line inputs.
pub(crate) fn sanitize_single_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// Normalizes text for multiline inputs.
pub(crate) fn normalize_multiline(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Clamps a byte offset to the nearest preceding UTF-8 character boundary.
pub(crate) fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    if text.is_char_boundary(offset) {
        return offset;
    }
    let mut clamped = offset;
    while clamped > 0 && !text.is_char_boundary(clamped) {
        clamped -= 1;
    }
    clamped
}

/// Converts a UTF-16 code-unit offset into a UTF-8 byte offset.
pub(crate) fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;

    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }

    utf8_offset.min(text.len())
}

/// Converts a UTF-8 byte offset into a UTF-16 code-unit offset.
pub(crate) fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let offset = clamp_to_char_boundary(text, offset);
    let mut utf16_offset = 0;
    let mut utf8_count = 0;

    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }

    utf16_offset
}

/// Converts and normalizes a UTF-16 range into a UTF-8 byte range.
pub(crate) fn range_from_utf16(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    let start = offset_from_utf16(text, range_utf16.start);
    let end = offset_from_utf16(text, range_utf16.end);
    start.min(end)..start.max(end)
}

/// Converts a UTF-8 byte range into a UTF-16 code-unit range.
pub(crate) fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

/// Returns the previous grapheme boundary before an offset.
pub(crate) fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(idx, _)| (idx < offset).then_some(idx))
        .unwrap_or(0)
}

/// Returns the next grapheme boundary after an offset.
pub(crate) fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(idx, _)| (idx > offset).then_some(idx))
        .unwrap_or(text.len())
}

fn clamp_to_grapheme_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }

    let offset = clamp_to_char_boundary(text, offset);
    let mut boundary = 0;
    for (start, grapheme) in text.grapheme_indices(true) {
        let end = start + grapheme.len();
        if end <= offset {
            boundary = end;
        } else {
            break;
        }
    }
    boundary
}

fn stored_offset_after_graphemes(text: &str, grapheme_count: usize) -> usize {
    if grapheme_count == 0 {
        return 0;
    }

    let mut consumed = 0;
    for (start, grapheme) in text.grapheme_indices(true) {
        consumed += 1;
        if consumed >= grapheme_count {
            return start + grapheme.len();
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_round_trip_mixed_width_text() {
        let text = "a🙂中";

        assert_eq!(offset_from_utf16(text, 0), 0);
        assert_eq!(offset_from_utf16(text, 1), 1);
        assert_eq!(offset_from_utf16(text, 2), "a🙂".len());
        assert_eq!(offset_from_utf16(text, 3), "a🙂".len());
        assert_eq!(offset_from_utf16(text, 4), text.len());
        assert_eq!(offset_from_utf16(text, usize::MAX), text.len());

        assert_eq!(offset_to_utf16(text, 0), 0);
        assert_eq!(offset_to_utf16(text, 1), 1);
        assert_eq!(offset_to_utf16(text, 2), 1);
        assert_eq!(offset_to_utf16(text, "a🙂".len()), 3);
        assert_eq!(offset_to_utf16(text, text.len()), 4);
    }

    #[test]
    fn utf16_range_conversion_normalizes_reversed_ranges() {
        let text = "a🙂中";

        assert_eq!(range_from_utf16(text, &(4..1)), 1..text.len());
        assert_eq!(range_to_utf16(text, &(2..text.len())), 1..4);
    }

    #[test]
    fn char_boundary_clamp_uses_preceding_boundary() {
        let text = "a🙂中";

        assert_eq!(clamp_to_char_boundary(text, usize::MAX), text.len());
        assert_eq!(clamp_to_char_boundary(text, 2), 1);
        assert_eq!(clamp_to_char_boundary(text, text.len() - 1), "a🙂".len());
    }

    #[test]
    fn document_deletes_grapheme_clusters_as_user_visible_units() {
        let text = "a👨‍👩‍👧‍👦b";
        let document = EditableTextDocument::from_parts(
            text,
            TextSelection::caret("a👨‍👩‍👧‍👦".len()),
            None,
            TextEditingPolicy::single_line(),
        );

        let projection = document.delete_backward().expect("delete should edit");

        assert_eq!(projection.text(), "ab");
        assert_eq!(projection.selection().range(), 1..1);
    }

    #[test]
    fn document_replaces_selection_consistently() {
        let document = EditableTextDocument::from_parts(
            "alpha gamma",
            TextSelection::from_offsets(6, 11),
            None,
            TextEditingPolicy::single_line(),
        );

        let projection = document.replace_text_in_range(None, "beta");

        assert_eq!(projection.text(), "alpha beta");
        assert_eq!(projection.selection().range(), 10..10);
        assert_eq!(projection.marked_range(), None);
    }

    #[test]
    fn document_composition_marks_and_commits_without_corrupting_selection() {
        let document = EditableTextDocument::new("", TextEditingPolicy::single_line());
        let composing = document.replace_and_mark_text_in_range(None, "ni", Some(1..2));

        assert_eq!(composing.text(), "ni");
        assert_eq!(composing.marked_range(), Some(0..2));
        assert_eq!(composing.selection().range(), 1..2);

        let document = EditableTextDocument::from_parts(
            composing.text(),
            composing.selection().clone(),
            composing.marked_range(),
            TextEditingPolicy::single_line(),
        );
        let committed = document.replace_text_in_range(None, "你");

        assert_eq!(committed.text(), "你");
        assert_eq!(committed.marked_range(), None);
        assert_eq!(committed.selection().range(), "你".len().."你".len());
    }

    #[test]
    fn newline_policy_distinguishes_single_line_and_multiline_submission() {
        let single = TextEditingPolicy::single_line().normalize_text("a\r\nb\nc\rd");
        let multi = TextEditingPolicy::multiline().normalize_text("a\r\nb\nc\rd");

        assert_eq!(single, "a  b c d");
        assert_eq!(multi, "a\nb\nc\nd");
    }

    #[test]
    fn display_projection_masks_without_leaking_stored_text() {
        let projection = TextDisplayProjection::for_policy("a🙂中", TextDisplayPolicy::Masked);
        let mask_len = TEXT_PASSWORD_MASK_CHAR.len_utf8();

        assert_eq!(projection.display_text().as_ref(), "•••");
        assert_eq!(
            projection.stored_to_display_offset("a🙂".len()),
            mask_len * 2
        );
        assert_eq!(
            projection.display_to_stored_offset(mask_len * 2),
            "a🙂".len()
        );
        assert!(!projection.display_text().contains('🙂'));
    }
}
