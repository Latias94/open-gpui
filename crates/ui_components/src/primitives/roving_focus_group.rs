//! GPUI-facing roving-focus helpers.

/// Returns the first enabled item index.
pub fn first_enabled(disabled: &[bool]) -> Option<usize> {
    crate::roving_focus::first_enabled(disabled)
}

/// Returns the last enabled item index.
pub fn last_enabled(disabled: &[bool]) -> Option<usize> {
    crate::roving_focus::last_enabled(disabled)
}

/// Returns the next enabled index from the current item.
pub fn next_enabled(disabled: &[bool], current: usize, forward: bool, wrap: bool) -> Option<usize> {
    crate::roving_focus::next_enabled(disabled, current, forward, wrap)
}

/// Resolves a selected index from stable string keys.
pub fn active_index_from_str_keys(
    keys: &[String],
    selected: Option<&str>,
    disabled: &[bool],
) -> Option<usize> {
    crate::roving_focus::active_index_from_str_keys(keys, selected, disabled)
}

/// Resolves a roving-focus navigation target from an APG-style key name.
pub fn roving_navigation_target(
    orientation: open_gpui_ui_core::Orientation,
    key: &str,
    current: usize,
    disabled: &[bool],
) -> Option<usize> {
    crate::roving_focus::roving_navigation_target(orientation, key, current, disabled)
}

/// Resolves the standard vertical roving target used by listbox-like surfaces.
pub fn vertical_roving_navigation_target(
    key: &str,
    current: usize,
    disabled: &[bool],
) -> Option<usize> {
    crate::roving_focus::vertical_roving_navigation_target(key, current, disabled)
}

/// Resolves bounded sequential navigation, including optional page jumps.
pub fn paged_navigation_target(
    key: &str,
    current: usize,
    item_count: usize,
    page_step: usize,
) -> Option<usize> {
    crate::roving_focus::paged_navigation_target(key, current, item_count, page_step)
}

/// Resolves a typeahead target by scanning from the item after the current index.
pub fn typeahead_target<'a, T>(
    items: &'a [T],
    current: Option<usize>,
    query: &str,
    focusable: impl Fn(&T) -> bool,
    label: impl Fn(&T) -> &str,
) -> Option<&'a T> {
    crate::roving_focus::typeahead_target(items, current, query, focusable, label)
}
