//! Shared text editing helpers for GPUI text adapters.

use std::ops::Range;

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
}
