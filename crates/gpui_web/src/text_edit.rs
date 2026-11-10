use std::ops::Range;

pub(crate) struct ImportedTextEdit {
    pub(crate) removed_before_selection: usize,
    pub(crate) removed_after_selection: usize,
    pub(crate) inserted_text: String,
}

pub(crate) fn imported_text_edit(
    old: &str,
    new: &str,
    selection: Range<usize>,
    post_edit_caret: Option<usize>,
) -> ImportedTextEdit {
    let old_units: Vec<_> = old.encode_utf16().collect();
    let new_units: Vec<_> = new.encode_utf16().collect();
    let anchored_suffix_length = post_edit_caret
        .map(|caret| new_units.len().saturating_sub(caret))
        .filter(|&suffix_length| {
            suffix_length <= old_units.len()
                && old_units[old_units.len() - suffix_length..]
                    == new_units[new_units.len() - suffix_length..]
        });
    let mut suffix_length = anchored_suffix_length
        .unwrap_or_else(|| {
            old_units
                .iter()
                .rev()
                .zip(new_units.iter().rev())
                .take_while(|(old_unit, new_unit)| old_unit == new_unit)
                .count()
        })
        .min(old_units.len().saturating_sub(selection.end));
    while suffix_length > 0
        && (splits_surrogate(&old_units, old_units.len() - suffix_length)
            || splits_surrogate(&new_units, new_units.len() - suffix_length))
    {
        suffix_length -= 1;
    }
    let mut prefix_length = old_units
        .iter()
        .zip(&new_units)
        .take_while(|(old_unit, new_unit)| old_unit == new_unit)
        .count()
        .min(old_units.len() - suffix_length)
        .min(new_units.len() - suffix_length)
        .min(selection.start);
    while prefix_length > 0
        && (splits_surrogate(&old_units, prefix_length)
            || splits_surrogate(&new_units, prefix_length))
    {
        prefix_length -= 1;
    }
    ImportedTextEdit {
        removed_before_selection: selection.start.saturating_sub(prefix_length),
        removed_after_selection: (old_units.len() - suffix_length).saturating_sub(selection.end),
        inserted_text: String::from_utf16_lossy(
            &new_units[prefix_length..new_units.len() - suffix_length],
        ),
    }
}

fn splits_surrogate(text: &[u16], index: usize) -> bool {
    index > 0
        && text
            .get(index)
            .is_some_and(|unit| (0xdc00..=0xdfff).contains(unit))
        && (0xd800..=0xdbff).contains(&text[index - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_import(
        old: &str,
        new: &str,
        selection: Range<usize>,
        caret: Option<usize>,
    ) -> ImportedTextEdit {
        let edit = imported_text_edit(old, new, selection.clone(), caret);
        let mut result: Vec<_> = old.encode_utf16().collect();
        result.splice(
            selection.start - edit.removed_before_selection
                ..selection.end + edit.removed_after_selection,
            edit.inserted_text.encode_utf16(),
        );
        assert_eq!(
            String::from_utf16(&result).expect("valid imported UTF-16"),
            new
        );
        edit
    }

    #[test]
    fn selected_emoji_replacement_keeps_the_shared_high_surrogate() {
        let edit = assert_import("😀😀😀", "🕸🕸🕸", 0..6, Some(6));
        assert_eq!(edit.inserted_text, "🕸🕸🕸");
        assert_eq!(
            (edit.removed_before_selection, edit.removed_after_selection),
            (0, 0)
        );
    }

    #[test]
    fn replacement_includes_unchanged_text_inside_the_selection() {
        let edit = assert_import("pact", "pactor", 0..4, Some(6));
        assert_eq!(edit.inserted_text, "pactor");
        assert_import("prefix old suffix", "prefix new suffix", 7..10, Some(10));
    }

    #[test]
    fn a_shared_low_surrogate_stays_with_its_replacement_scalar() {
        let edit = assert_import("𐀀x", "𐐀x", 0..0, None);
        assert_eq!(edit.inserted_text, "𐐀");
        assert_eq!(edit.removed_after_selection, 2);
    }

    #[test]
    fn collapsed_autocorrection_preserves_the_shared_high_surrogate() {
        let edit = assert_import("A😀", "A🕸", 3..3, Some(3));
        assert_eq!(edit.inserted_text, "🕸");
        assert_eq!(edit.removed_before_selection, 2);
    }

    #[test]
    fn post_edit_caret_disambiguates_repeated_suffix_text() {
        let edit = assert_import("pact", "pactor pact", 0..0, Some(7));
        assert_eq!(edit.inserted_text, "pactor ");
        assert_eq!(
            (edit.removed_before_selection, edit.removed_after_selection),
            (0, 0)
        );
    }

    #[test]
    fn deletion_and_same_length_selection_edits_keep_complete_source() {
        assert_import("a😀b", "ab", 1..3, Some(1));
        assert_import("a😀b", "a🕸b", 1..3, Some(3));
        assert_import("hello world", "hello", 11..11, Some(5));
        assert_import("aéz", "ae\u{301}z", 1..2, Some(3));
    }
}
