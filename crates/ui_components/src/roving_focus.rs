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
    if len == 0 || current >= len {
        return None;
    }

    let is_disabled = |idx: usize| disabled.get(idx).copied().unwrap_or(false);

    if wrap {
        for step in 1..=len {
            let idx = if forward {
                (current + step) % len
            } else {
                (current + len - (step % len)) % len
            };
            if !is_disabled(idx) {
                return Some(idx);
            }
        }
        None
    } else if forward {
        ((current + 1)..len).find(|&index| !is_disabled(index))
    } else if current > 0 {
        (0..current).rev().find(|&index| !is_disabled(index))
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
    }
}
