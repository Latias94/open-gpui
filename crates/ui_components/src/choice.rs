//! Shared helpers for stable-value choice surfaces.

/// Normalizes query text for case-insensitive matching.
pub(crate) fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
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
    if is_multiple {
        let groups = groups.into_iter().collect::<Vec<_>>();
        dedupe_stable_values(selected_values.into_iter().map(Into::into).filter(|value| {
            find_value_in_flat_groups(items, groups.iter().copied(), value, &value_of)
                .is_some_and(|item| !disabled(item))
        }))
    } else {
        selected_value.map(str::to_owned).into_iter().collect()
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
}
