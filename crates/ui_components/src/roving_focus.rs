//! Shared roving-focus helpers for composite components.

use open_gpui_ui_core::Orientation;

/// Returns the first enabled item index.
pub fn first_enabled(disabled: &[bool]) -> Option<usize> {
    disabled.iter().position(|disabled| !*disabled)
}

/// Returns the last enabled item index.
pub fn last_enabled(disabled: &[bool]) -> Option<usize> {
    disabled.iter().rposition(|disabled| !*disabled)
}

/// Returns the next enabled index from the current item.
pub fn next_enabled(disabled: &[bool], current: usize, forward: bool, wrap: bool) -> Option<usize> {
    let len = disabled.len();
    let is_disabled = |idx: usize| disabled.get(idx).copied().unwrap_or(false);
    next_matching_index(len, current, forward, wrap, |idx| !is_disabled(idx))
}

/// Returns the next matching index from the current item.
pub(crate) fn next_matching_index(
    len: usize,
    current: usize,
    forward: bool,
    wrap: bool,
    is_match: impl Fn(usize) -> bool,
) -> Option<usize> {
    if len == 0 || current >= len {
        return None;
    }

    if wrap {
        for step in 1..=len {
            let idx = if forward {
                (current + step) % len
            } else {
                (current + len - (step % len)) % len
            };
            if is_match(idx) {
                return Some(idx);
            }
        }
        None
    } else if forward {
        ((current + 1)..len).find(|&index| is_match(index))
    } else if current > 0 {
        (0..current).rev().find(|&index| is_match(index))
    } else {
        None
    }
}

/// Resolves a selected index from stable string keys.
pub fn active_index_from_str_keys(
    keys: &[String],
    selected: Option<&str>,
    disabled: &[bool],
) -> Option<usize> {
    selection_index_from_str_keys(keys, disabled, selected, None)
}

/// Resolves an index from primary and secondary stable string keys.
pub(crate) fn selection_index_from_str_keys(
    keys: &[String],
    disabled: &[bool],
    primary: Option<&str>,
    secondary: Option<&str>,
) -> Option<usize> {
    if keys.len() != disabled.len() {
        return first_enabled(disabled);
    }

    let is_valid = |candidate: &str| {
        keys.iter()
            .position(|key| key.as_str() == candidate)
            .filter(|index| !disabled.get(*index).copied().unwrap_or(true))
    };

    primary
        .and_then(is_valid)
        .or_else(|| secondary.and_then(is_valid))
        .or_else(|| first_enabled(disabled))
}

/// Resolves a roving-focus navigation target from an APG-style key name.
pub(crate) fn roving_navigation_target(
    orientation: Orientation,
    key: &str,
    current: usize,
    disabled: &[bool],
) -> Option<usize> {
    match (orientation, key) {
        (_, "home") => first_enabled(disabled),
        (_, "end") => last_enabled(disabled),
        (Orientation::Horizontal, "left") => next_enabled(disabled, current, false, true),
        (Orientation::Horizontal, "right") => next_enabled(disabled, current, true, true),
        (Orientation::Vertical, "up") => next_enabled(disabled, current, false, true),
        (Orientation::Vertical, "down") => next_enabled(disabled, current, true, true),
        _ => None,
    }
}

/// Resolves the standard vertical roving target used by listbox-like surfaces.
pub(crate) fn vertical_roving_navigation_target(
    key: &str,
    current: usize,
    disabled: &[bool],
) -> Option<usize> {
    roving_navigation_target(Orientation::Vertical, key, current, disabled)
}

/// Resolves bounded sequential navigation, including optional page jumps.
pub(crate) fn paged_navigation_target(
    key: &str,
    current: usize,
    item_count: usize,
    page_step: usize,
) -> Option<usize> {
    if item_count == 0 || current >= item_count {
        return None;
    }

    match key {
        "home" => Some(0),
        "end" => item_count.checked_sub(1),
        "up" => Some(current.saturating_sub(1)),
        "down" => Some((current + 1).min(item_count - 1)),
        "pageup" => Some(current.saturating_sub(page_step.max(1))),
        "pagedown" => Some((current + page_step.max(1)).min(item_count - 1)),
        _ => None,
    }
}

/// Resolves a typeahead target by scanning from the item after the current index.
pub(crate) fn typeahead_target<'a, T>(
    items: &'a [T],
    current: Option<usize>,
    query: &str,
    focusable: impl Fn(&T) -> bool,
    label: impl Fn(&T) -> &str,
) -> Option<&'a T> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }

    let len = items.len();
    if len == 0 {
        return None;
    }

    let start = current.map_or(0, |index| (index + 1) % len);
    (0..len)
        .map(|step| (start + step) % len)
        .filter_map(|index| items.get(index))
        .find(|item| focusable(item) && label(item).to_lowercase().starts_with(query.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_navigation_skips_disabled_items_and_wraps() {
        let disabled = [false, true, false];

        assert_eq!(first_enabled(&disabled), Some(0));
        assert_eq!(last_enabled(&disabled), Some(2));
        assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
        assert_eq!(next_enabled(&disabled, 2, true, true), Some(0));
        assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    }

    #[test]
    fn stable_key_selection_prefers_primary_then_secondary_then_first_enabled() {
        let keys = [
            "overview".to_owned(),
            "details".to_owned(),
            "history".to_owned(),
        ];
        let disabled = [false, false, true];

        assert_eq!(
            selection_index_from_str_keys(&keys, &disabled, Some("details"), Some("overview")),
            Some(1)
        );
        assert_eq!(
            selection_index_from_str_keys(&keys, &disabled, Some("history"), Some("overview")),
            Some(0)
        );
        assert_eq!(
            active_index_from_str_keys(&keys, Some("missing"), &disabled),
            Some(0)
        );
    }

    #[test]
    fn oriented_navigation_maps_apg_keys() {
        let disabled = [false, true, false];

        assert_eq!(
            roving_navigation_target(Orientation::Horizontal, "right", 0, &disabled),
            Some(2)
        );
        assert_eq!(
            roving_navigation_target(Orientation::Horizontal, "left", 0, &disabled),
            Some(2)
        );
        assert_eq!(
            roving_navigation_target(Orientation::Vertical, "down", 2, &disabled),
            Some(0)
        );
        assert_eq!(
            roving_navigation_target(Orientation::Vertical, "left", 0, &disabled),
            None
        );
        assert_eq!(
            vertical_roving_navigation_target("down", 2, &disabled),
            Some(0)
        );
    }

    #[test]
    fn typeahead_target_scans_focusable_items_after_current() {
        #[derive(Clone, Copy)]
        struct Item {
            label: &'static str,
            focusable: bool,
        }

        let items = [
            Item {
                label: "Alpha",
                focusable: true,
            },
            Item {
                label: "Bravo",
                focusable: false,
            },
            Item {
                label: "Charlie",
                focusable: true,
            },
            Item {
                label: "Delta",
                focusable: true,
            },
        ];

        assert_eq!(
            typeahead_target(
                &items,
                Some(0),
                " ch",
                |item| item.focusable,
                |item| item.label
            )
            .map(|item| item.label),
            Some("Charlie")
        );
        assert_eq!(
            typeahead_target(
                &items,
                Some(2),
                "al",
                |item| item.focusable,
                |item| item.label
            )
            .map(|item| item.label),
            Some("Alpha")
        );
        assert_eq!(
            typeahead_target(
                &items,
                Some(0),
                "",
                |item| item.focusable,
                |item| item.label
            )
            .map(|item| item.label),
            None
        );
    }

    #[test]
    fn paged_navigation_clamps_to_bounds() {
        assert_eq!(paged_navigation_target("home", 6, 12, 4), Some(0));
        assert_eq!(paged_navigation_target("end", 6, 12, 4), Some(11));
        assert_eq!(paged_navigation_target("up", 6, 12, 4), Some(5));
        assert_eq!(paged_navigation_target("down", 6, 12, 4), Some(7));
        assert_eq!(paged_navigation_target("pageup", 6, 12, 4), Some(2));
        assert_eq!(paged_navigation_target("pagedown", 6, 12, 4), Some(10));
        assert_eq!(paged_navigation_target("pagedown", 11, 12, 4), Some(11));
        assert_eq!(paged_navigation_target("pageup", 0, 12, 4), Some(0));
        assert_eq!(paged_navigation_target("down", 0, 0, 4), None);
    }
}
