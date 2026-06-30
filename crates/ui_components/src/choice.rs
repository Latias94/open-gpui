//! Shared helpers for stable-value choice surfaces.

/// Flat stable-value item projected from a choice surface descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceItemProjection<T> {
    source_index: usize,
    group_index: Option<usize>,
    value: String,
    label: String,
    disabled: bool,
    item: T,
}

impl<T> ChoiceItemProjection<T> {
    /// Creates a projected item with stable identity and navigation metadata.
    pub(crate) fn new(
        source_index: usize,
        group_index: Option<usize>,
        value: impl Into<String>,
        label: impl Into<String>,
        disabled: bool,
        item: T,
    ) -> Self {
        Self {
            source_index,
            group_index,
            value: value.into(),
            label: label.into(),
            disabled,
            item,
        }
    }

    /// Returns the item index in its source collection.
    pub(crate) const fn source_index(&self) -> usize {
        self.source_index
    }

    /// Returns the owning group index, if grouped.
    pub(crate) const fn group_index(&self) -> Option<usize> {
        self.group_index
    }

    /// Returns the stable value.
    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this item can be focused or selected.
    pub(crate) const fn enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the original item payload.
    pub(crate) const fn item(&self) -> &T {
        &self.item
    }

    /// Consumes the projection and returns the original item payload.
    pub(crate) fn into_item(self) -> T {
        self.item
    }
}

/// Selected and active indexes resolved from stable-value choice projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChoiceSelectionResolution {
    selected_index: Option<usize>,
    active_index: Option<usize>,
}

impl ChoiceSelectionResolution {
    /// Returns the resolved selected index.
    pub(crate) const fn selected_index(self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the resolved active index.
    pub(crate) const fn active_index(self) -> Option<usize> {
        self.active_index
    }
}

/// Normalizes query text for case-insensitive matching.
pub(crate) fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

/// Returns whether the text contains the already-normalized query.
pub(crate) fn normalized_text_contains(text: &str, normalized_query: &str) -> bool {
    normalize_query(text).contains(normalized_query)
}

/// Returns whether the text starts with the already-normalized query.
pub(crate) fn normalized_text_starts_with(text: &str, normalized_query: &str) -> bool {
    normalize_query(text).starts_with(normalized_query)
}

/// Returns whether any stable-value text source contains the normalized query.
pub(crate) fn query_matches_sources<'a>(
    normalized_query: &str,
    sources: impl IntoIterator<Item = &'a str>,
) -> bool {
    normalized_query.is_empty()
        || sources
            .into_iter()
            .any(|source| normalized_text_contains(source, normalized_query))
}

/// Finds a stable value in a flat list.
pub(crate) fn find_value<'a, T>(
    items: &'a [T],
    value: &str,
    value_of: impl Fn(&T) -> &str,
) -> Option<&'a T> {
    items.iter().find(|item| value_of(item) == value)
}

/// Finds a stable value across already-flattened groups.
pub(crate) fn find_value_in_flat_groups<'a, T, I>(
    items: &'a [T],
    groups: I,
    value: &str,
    value_of: impl Fn(&T) -> &str,
) -> Option<&'a T>
where
    I: IntoIterator<Item = &'a [T]>,
{
    let value_of = &value_of;

    find_value(items, value, value_of).or_else(|| {
        groups
            .into_iter()
            .flat_map(|group| group.iter())
            .find(|item| value_of(item) == value)
    })
}

/// Resolves a stable value only if the item still exists and remains enabled.
pub(crate) fn resolve_enabled_value<'a, T, I>(
    items: &'a [T],
    groups: I,
    selected_value: Option<&str>,
    value_of: impl Fn(&T) -> &str,
    disabled: impl Fn(&T) -> bool,
) -> Option<&'a T>
where
    I: IntoIterator<Item = &'a [T]>,
{
    find_value_in_flat_groups(items, groups, selected_value?, value_of)
        .filter(|item| !disabled(item))
}

/// Deduplicates stable values while preserving first-seen order.
pub(crate) fn dedupe_stable_values<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    values.into_iter().fold(Vec::new(), |mut acc, value| {
        if !acc.iter().any(|existing| existing == &value) {
            acc.push(value);
        }
        acc
    })
}

/// Resolves selected and active indexes from projected choice items.
pub(crate) fn resolve_selection_indexes<T>(
    disabled: bool,
    items: &[ChoiceItemProjection<T>],
    selected_value: Option<&str>,
    active_value: Option<&str>,
) -> ChoiceSelectionResolution {
    if disabled || items.is_empty() {
        return ChoiceSelectionResolution {
            selected_index: None,
            active_index: None,
        };
    }

    let selected_index = selected_value.and_then(|value| {
        items
            .iter()
            .position(|item| item.value() == value && item.enabled())
    });
    let active_index = active_value
        .and_then(|value| {
            items
                .iter()
                .position(|item| item.value() == value && item.enabled())
        })
        .or(selected_index)
        .or_else(|| items.iter().position(ChoiceItemProjection::enabled));

    ChoiceSelectionResolution {
        selected_index,
        active_index,
    }
}

/// Resolves a typeahead target by scanning from the item after the current index.
pub(crate) fn typeahead_target<'a, T>(
    items: &'a [ChoiceItemProjection<T>],
    current: Option<usize>,
    query: &str,
) -> Option<&'a ChoiceItemProjection<T>> {
    let query = normalize_query(query);
    if query.is_empty() || items.is_empty() {
        return None;
    }

    let len = items.len();
    let start = current.map_or(0, |index| (index + 1) % len);
    (0..len)
        .map(|step| (start + step) % len)
        .filter_map(|index| items.get(index))
        .find(|item| item.enabled() && normalized_text_starts_with(item.label(), query.as_str()))
}

/// Resolves selected values for single-select and multi-select choice surfaces.
pub(crate) fn resolve_selected_values<'a, T, I, S>(
    is_multiple: bool,
    items: &'a [T],
    groups: I,
    selected_value: Option<&str>,
    selected_values: impl IntoIterator<Item = S>,
    value_of: impl Fn(&T) -> &str,
    disabled: impl Fn(&T) -> bool,
) -> Vec<String>
where
    I: IntoIterator<Item = &'a [T]>,
    S: Into<String>,
{
    let groups = groups.into_iter().collect::<Vec<_>>();
    if is_multiple {
        dedupe_stable_values(selected_values.into_iter().map(Into::into).filter(|value| {
            find_value_in_flat_groups(items, groups.iter().copied(), value, &value_of)
                .is_some_and(|item| !disabled(item))
        }))
    } else {
        selected_value
            .filter(|value| {
                find_value_in_flat_groups(items, groups.iter().copied(), value, &value_of)
                    .is_some_and(|item| !disabled(item))
            })
            .map(str::to_owned)
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Choice {
        value: &'static str,
        disabled: bool,
    }

    impl Choice {
        const fn new(value: &'static str, disabled: bool) -> Self {
            Self { value, disabled }
        }

        fn value(&self) -> &str {
            self.value
        }

        const fn disabled(&self) -> bool {
            self.disabled
        }
    }

    struct Group<'a> {
        items: &'a [Choice],
    }

    #[test]
    fn normalize_query_trims_and_lowercases() {
        assert_eq!(normalize_query("  ReAcT  "), "react");
    }

    #[test]
    fn query_matching_shares_case_and_whitespace_normalization() {
        let query = normalize_query("  Re ");

        assert!(query_matches_sources(
            query.as_str(),
            ["React", "library"].into_iter()
        ));
        assert!(normalized_text_starts_with("  Remix", query.as_str()));
        assert!(!query_matches_sources(
            query.as_str(),
            ["Solid"].into_iter()
        ));
    }

    #[test]
    fn find_value_in_groups_prefers_flat_items() {
        let standalone = [Choice::new("shared", false), Choice::new("alpha", false)];
        let grouped = [Choice::new("shared", true), Choice::new("beta", false)];
        let groups = [Group { items: &grouped }];

        let found = find_value_in_flat_groups(
            &standalone,
            groups.iter().map(|group| group.items),
            "shared",
            Choice::value,
        );

        assert_eq!(found.map(Choice::value), Some("shared"));
        assert!(!found.is_some_and(Choice::disabled));
    }

    #[test]
    fn resolve_selected_values_dedupes_and_filters_disabled_multi_select() {
        let standalone = [Choice::new("alpha", false), Choice::new("beta", true)];
        let grouped = [Choice::new("gamma", false), Choice::new("delta", false)];
        let groups = [Group { items: &grouped }];

        let values = resolve_selected_values(
            true,
            &standalone,
            groups.iter().map(|group| group.items),
            Some("alpha"),
            vec!["alpha", "gamma", "alpha", "beta", "delta"],
            Choice::value,
            Choice::disabled,
        );

        assert_eq!(values, vec!["alpha", "gamma", "delta"]);
    }

    #[test]
    fn resolve_selected_values_keeps_single_value() {
        let standalone = [Choice::new("alpha", false)];
        let groups: [Group<'_>; 0] = [];

        let values = resolve_selected_values(
            false,
            &standalone,
            groups.iter().map(|group| group.items),
            Some("alpha"),
            Vec::<String>::new(),
            Choice::value,
            Choice::disabled,
        );

        assert_eq!(values, vec!["alpha"]);
    }

    #[test]
    fn resolve_selected_values_filters_disabled_single_value() {
        let standalone = [Choice::new("alpha", true)];
        let groups: [Group<'_>; 0] = [];

        let values = resolve_selected_values(
            false,
            &standalone,
            groups.iter().map(|group| group.items),
            Some("alpha"),
            Vec::<String>::new(),
            Choice::value,
            Choice::disabled,
        );

        assert!(values.is_empty());
    }

    #[test]
    fn selection_resolution_uses_stable_value_and_enabled_fallbacks() {
        let items = [
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", true, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", false, ()),
            ChoiceItemProjection::new(2, None, "charlie", "Charlie", false, ()),
        ];

        let selected = resolve_selection_indexes(false, &items, Some("bravo"), None);
        assert_eq!(selected.selected_index(), Some(1));
        assert_eq!(selected.active_index(), Some(1));

        let active = resolve_selection_indexes(false, &items, Some("missing"), Some("charlie"));
        assert_eq!(active.selected_index(), None);
        assert_eq!(active.active_index(), Some(2));

        let fallback = resolve_selection_indexes(false, &items, Some("alpha"), Some("alpha"));
        assert_eq!(fallback.selected_index(), None);
        assert_eq!(fallback.active_index(), Some(1));

        let disabled_surface = resolve_selection_indexes(true, &items, Some("bravo"), None);
        assert_eq!(disabled_surface.selected_index(), None);
        assert_eq!(disabled_surface.active_index(), None);
    }

    #[test]
    fn typeahead_scans_from_active_item_and_skips_disabled_items() {
        let items = [
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", true, ()),
            ChoiceItemProjection::new(2, None, "beta", "Beta", false, ()),
        ];

        assert_eq!(
            typeahead_target(&items, Some(0), " b").map(ChoiceItemProjection::value),
            Some("beta")
        );
        assert_eq!(
            typeahead_target(&items, Some(2), "a").map(ChoiceItemProjection::value),
            Some("alpha")
        );
        assert!(typeahead_target(&items, None, "missing").is_none());
    }
}
