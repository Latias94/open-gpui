//! Shared helpers for stable-value choice surfaces.

use std::collections::BTreeMap;

use open_gpui_ui_core::Orientation;

use crate::roving_focus::{first_enabled, last_enabled, next_enabled};

/// Flat stable-value item projected from a choice surface descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceItemProjection<T> {
    source_index: usize,
    group_index: Option<usize>,
    value: String,
    label: String,
    text_value: Option<String>,
    disabled: bool,
    ambiguous_value: bool,
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
            text_value: None,
            disabled,
            ambiguous_value: false,
            item,
        }
    }

    /// Sets the text candidate used by typeahead matching.
    pub(crate) fn text_value(mut self, text_value: impl Into<String>) -> Self {
        self.text_value = Some(text_value.into());
        self
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

    /// Returns the typeahead candidate text, falling back to the visible label.
    pub(crate) fn typeahead_text(&self) -> &str {
        self.text_value.as_deref().unwrap_or(self.label())
    }

    /// Returns whether this item can be focused or selected.
    pub(crate) const fn enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns whether this value occurs more than once in a uniqueness-enforcing collection.
    pub(crate) const fn ambiguous_value(&self) -> bool {
        self.ambiguous_value
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

/// Selection cardinality semantics for choice surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceSelectionMode {
    /// The collection has active movement but no selected value.
    None,
    /// The collection projects at most one selected stable value.
    Single,
    /// The collection may project multiple stable values.
    Multiple,
}

/// Activation timing semantics for choice surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceActivationMode {
    /// Movement changes active item only; explicit activation commits selection.
    Manual,
    /// Movement may commit the active item immediately.
    Automatic,
}

/// Disabled-item traversal semantics for choice surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceDisabledItemStrategy {
    /// Disabled items are skipped for movement, typeahead, and selection.
    Skip,
    /// Disabled items remain addressable by movement and typeahead.
    #[allow(dead_code)]
    Include,
}

/// Renderer-neutral interaction policy for stable-value choice collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChoiceInteractionPolicy {
    orientation: Orientation,
    wrap: bool,
    typeahead: bool,
    selection_mode: ChoiceSelectionMode,
    selection_required: bool,
    activation_mode: ChoiceActivationMode,
    disabled_item_strategy: ChoiceDisabledItemStrategy,
}

impl ChoiceInteractionPolicy {
    /// Returns the APG-style listbox policy used by Listbox, Select, and Combobox.
    pub(crate) const fn listbox() -> Self {
        Self::single_optional(Orientation::Vertical).with_typeahead(true)
    }

    /// Returns a policy for value-selecting composites with one required selection.
    pub(crate) const fn single_required(orientation: Orientation) -> Self {
        Self::single_optional(orientation).with_selection_required(true)
    }

    /// Returns a policy for value-selecting composites with optional selection.
    pub(crate) const fn single_optional(orientation: Orientation) -> Self {
        Self {
            orientation,
            wrap: false,
            typeahead: false,
            selection_mode: ChoiceSelectionMode::Single,
            selection_required: false,
            activation_mode: ChoiceActivationMode::Manual,
            disabled_item_strategy: ChoiceDisabledItemStrategy::Skip,
        }
        .with_wrap(true)
    }

    /// Returns a policy for multi-select composites.
    pub(crate) const fn multiple(orientation: Orientation) -> Self {
        Self::single_optional(orientation).with_selection_mode(ChoiceSelectionMode::Multiple)
    }

    /// Returns a policy for roving-focus composites without selected values.
    pub(crate) const fn roving(orientation: Orientation) -> Self {
        Self::single_optional(orientation).with_selection_mode(ChoiceSelectionMode::None)
    }

    /// Returns this policy with wrapping movement overridden.
    pub(crate) const fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Returns this policy with typeahead enabled or disabled.
    pub(crate) const fn with_typeahead(mut self, typeahead: bool) -> Self {
        self.typeahead = typeahead;
        self
    }

    /// Returns this policy with selection cardinality overridden.
    pub(crate) const fn with_selection_mode(mut self, selection_mode: ChoiceSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Returns this policy with required-selection behavior overridden.
    pub(crate) const fn with_selection_required(mut self, selection_required: bool) -> Self {
        self.selection_required = selection_required;
        self
    }

    /// Returns this policy with activation timing overridden.
    pub(crate) const fn with_activation_mode(
        mut self,
        activation_mode: ChoiceActivationMode,
    ) -> Self {
        self.activation_mode = activation_mode;
        self
    }

    /// Returns this policy with disabled-item handling overridden.
    #[allow(dead_code)]
    pub(crate) const fn with_disabled_item_strategy(
        mut self,
        disabled_item_strategy: ChoiceDisabledItemStrategy,
    ) -> Self {
        self.disabled_item_strategy = disabled_item_strategy;
        self
    }

    /// Returns whether typeahead matching is enabled.
    pub(crate) const fn typeahead(self) -> bool {
        self.typeahead
    }

    /// Returns selection cardinality semantics.
    pub(crate) const fn selection_mode(self) -> ChoiceSelectionMode {
        self.selection_mode
    }

    /// Returns whether selection should fall back to the first enabled item.
    pub(crate) const fn selection_required(self) -> bool {
        self.selection_required
    }

    /// Returns activation timing semantics.
    pub(crate) const fn activation_mode(self) -> ChoiceActivationMode {
        self.activation_mode
    }

    /// Resolves an APG-style movement key to a candidate index.
    pub(crate) fn navigation_target_index(
        self,
        key: &str,
        current: usize,
        disabled: &[bool],
    ) -> Option<usize> {
        match (self.orientation, key) {
            (_, "home") => first_enabled(disabled),
            (_, "end") => last_enabled(disabled),
            (Orientation::Horizontal, "left") => next_enabled(disabled, current, false, self.wrap),
            (Orientation::Horizontal, "right") => next_enabled(disabled, current, true, self.wrap),
            (Orientation::Vertical, "up") => next_enabled(disabled, current, false, self.wrap),
            (Orientation::Vertical, "down") => next_enabled(disabled, current, true, self.wrap),
            _ => None,
        }
    }

    fn item_addressable<T>(self, item: &ChoiceItemProjection<T>) -> bool {
        if item.ambiguous_value() {
            return false;
        }

        match self.disabled_item_strategy {
            ChoiceDisabledItemStrategy::Skip => item.enabled(),
            ChoiceDisabledItemStrategy::Include => true,
        }
    }
}

impl Default for ChoiceInteractionPolicy {
    fn default() -> Self {
        Self::listbox()
    }
}

/// Selected and active indexes resolved from stable-value choice projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChoiceSelectionResolution {
    selected_index: Option<usize>,
    active_index: Option<usize>,
}

impl ChoiceSelectionResolution {
    /// Creates a selection resolution from already-resolved indexes.
    pub(crate) const fn new(selected_index: Option<usize>, active_index: Option<usize>) -> Self {
        Self {
            selected_index,
            active_index,
        }
    }

    /// Returns the resolved selected index.
    pub(crate) const fn selected_index(self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the resolved active index.
    pub(crate) const fn active_index(self) -> Option<usize> {
        self.active_index
    }
}

/// Renderer-neutral stable-value collection for choice-like components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceCollection<T> {
    disabled: bool,
    policy: ChoiceInteractionPolicy,
    items: Vec<ChoiceItemProjection<T>>,
    selection: ChoiceSelectionResolution,
}

impl<T> ChoiceCollection<T> {
    /// Resolves a collection from raw projections and stable selected/active values.
    pub(crate) fn resolve(
        disabled: bool,
        items: Vec<ChoiceItemProjection<T>>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        policy: ChoiceInteractionPolicy,
    ) -> Self {
        let selection = resolve_selection_indexes_with_policy(
            disabled,
            &items,
            selected_value,
            None,
            active_value,
            policy,
        );
        Self::from_resolved(disabled, items, selection, policy)
    }

    /// Resolves a collection whose stable values must be globally unique.
    ///
    /// Every occurrence of an ambiguous value remains present but fails closed for selection,
    /// focus, and activation. Callers may inspect [`ChoiceItemProjection::ambiguous_value`] to
    /// project diagnostics without reimplementing value counting.
    pub(crate) fn resolve_unique(
        disabled: bool,
        mut items: Vec<ChoiceItemProjection<T>>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        policy: ChoiceInteractionPolicy,
    ) -> Self {
        mark_ambiguous_values(&mut items);
        Self::resolve(disabled, items, selected_value, active_value, policy)
    }

    /// Resolves a collection with a secondary selected-value candidate.
    pub(crate) fn resolve_with_selected_fallback(
        disabled: bool,
        items: Vec<ChoiceItemProjection<T>>,
        selected_value: Option<&str>,
        selected_fallback_value: Option<&str>,
        active_value: Option<&str>,
        policy: ChoiceInteractionPolicy,
    ) -> Self {
        let selection = resolve_selection_indexes_with_policy(
            disabled,
            &items,
            selected_value,
            selected_fallback_value,
            active_value,
            policy,
        );
        Self::from_resolved(disabled, items, selection, policy)
    }

    /// Builds a collection around already-resolved selected/active indexes.
    pub(crate) fn from_resolved(
        disabled: bool,
        items: Vec<ChoiceItemProjection<T>>,
        selection: ChoiceSelectionResolution,
        policy: ChoiceInteractionPolicy,
    ) -> Self {
        Self {
            disabled,
            policy,
            items,
            selection,
        }
    }

    /// Returns projected items.
    pub(crate) fn items(&self) -> &[ChoiceItemProjection<T>] {
        &self.items
    }

    /// Consumes the collection into projected items.
    pub(crate) fn into_items(self) -> Vec<ChoiceItemProjection<T>> {
        self.items
    }

    /// Returns the selected index.
    pub(crate) const fn selected_index(&self) -> Option<usize> {
        self.selection.selected_index()
    }

    /// Returns the active index.
    pub(crate) const fn active_index(&self) -> Option<usize> {
        self.selection.active_index()
    }

    /// Returns the selected item.
    pub(crate) fn selected_item(&self) -> Option<&ChoiceItemProjection<T>> {
        self.selected_index()
            .and_then(|index| self.items.get(index))
    }

    /// Returns the active item.
    pub(crate) fn active_item(&self) -> Option<&ChoiceItemProjection<T>> {
        self.active_index().and_then(|index| self.items.get(index))
    }

    /// Returns the selected stable value.
    pub(crate) fn selected_value(&self) -> Option<&str> {
        self.selected_item().map(ChoiceItemProjection::value)
    }

    /// Returns the active stable value.
    pub(crate) fn active_value(&self) -> Option<&str> {
        self.active_item().map(ChoiceItemProjection::value)
    }

    /// Returns the disabled map expected by movement helpers.
    pub(crate) fn disabled_map(&self) -> Vec<bool> {
        disabled_map_for_policy(self.policy, &self.items)
    }

    /// Resolves an APG-style navigation target from the active item.
    pub(crate) fn navigation_target(&self, key: &str) -> Option<&ChoiceItemProjection<T>> {
        if self.disabled {
            return None;
        }

        let current = self.active_index()?;
        let disabled = self.disabled_map();
        self.policy
            .navigation_target_index(key, current, &disabled)
            .and_then(|index| self.items.get(index))
    }

    /// Resolves a typeahead target by scanning from the active item.
    pub(crate) fn typeahead_target(&self, query: &str) -> Option<&ChoiceItemProjection<T>> {
        if self.disabled || !self.policy.typeahead() {
            return None;
        }

        typeahead_target_with_policy(self.items(), self.active_index(), query, self.policy)
    }
}

fn mark_ambiguous_values<T>(items: &mut [ChoiceItemProjection<T>]) {
    let value_counts = items.iter().fold(BTreeMap::new(), |mut counts, item| {
        *counts.entry(item.value.clone()).or_insert(0usize) += 1;
        counts
    });

    for item in items {
        item.ambiguous_value = value_counts
            .get(item.value())
            .is_some_and(|count| *count > 1);
        item.disabled |= item.ambiguous_value;
    }
}

fn disabled_map_for_policy<T>(
    policy: ChoiceInteractionPolicy,
    items: &[ChoiceItemProjection<T>],
) -> Vec<bool> {
    items
        .iter()
        .map(|item| !policy.item_addressable(item))
        .collect()
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

fn resolve_selection_indexes_with_policy<T>(
    disabled: bool,
    items: &[ChoiceItemProjection<T>],
    selected_value: Option<&str>,
    selected_fallback_value: Option<&str>,
    active_value: Option<&str>,
    policy: ChoiceInteractionPolicy,
) -> ChoiceSelectionResolution {
    if disabled || items.is_empty() {
        return ChoiceSelectionResolution::new(None, None);
    }

    let find_valid = |value: &str| {
        items
            .iter()
            .position(|item| item.value() == value && policy.item_addressable(item))
    };

    let first_enabled = || items.iter().position(|item| policy.item_addressable(item));
    let selected_index = match policy.selection_mode() {
        ChoiceSelectionMode::None => None,
        ChoiceSelectionMode::Single | ChoiceSelectionMode::Multiple => selected_value
            .and_then(find_valid)
            .or_else(|| selected_fallback_value.and_then(find_valid))
            .or_else(|| policy.selection_required().then(first_enabled).flatten()),
    };
    let active_index = active_value
        .and_then(find_valid)
        .or(selected_index)
        .or_else(first_enabled);

    ChoiceSelectionResolution::new(selected_index, active_index)
}

fn typeahead_target_with_policy<'a, T>(
    items: &'a [ChoiceItemProjection<T>],
    current: Option<usize>,
    query: &str,
    policy: ChoiceInteractionPolicy,
) -> Option<&'a ChoiceItemProjection<T>> {
    if !policy.typeahead() {
        return None;
    }

    let query = normalize_query(query);
    if query.is_empty() || items.is_empty() {
        return None;
    }

    let len = items.len();
    let start = current.map_or(0, |index| (index + 1) % len);
    (0..len)
        .map(|step| (start + step) % len)
        .filter_map(|index| items.get(index))
        .find(|item| {
            policy.item_addressable(item)
                && normalized_text_starts_with(item.typeahead_text(), query.as_str())
        })
}

/// Resolves selected stable values from already-projected choice items.
pub(crate) fn resolve_projected_selected_values<T, S>(
    selection_mode: ChoiceSelectionMode,
    items: &[ChoiceItemProjection<T>],
    selected_value: Option<&str>,
    selected_values: impl IntoIterator<Item = S>,
) -> Vec<String>
where
    S: Into<String>,
{
    match selection_mode {
        ChoiceSelectionMode::None => Vec::new(),
        ChoiceSelectionMode::Single => selected_value
            .map(str::to_owned)
            .into_iter()
            .chain(selected_values.into_iter().map(Into::into))
            .find(|value| {
                items
                    .iter()
                    .any(|item| item.value() == value && item.enabled())
            })
            .into_iter()
            .collect(),
        ChoiceSelectionMode::Multiple => {
            dedupe_stable_values(selected_values.into_iter().map(Into::into).filter(|value| {
                items
                    .iter()
                    .any(|item| item.value() == value && item.enabled())
            }))
        }
    }
}

/// Resolves the next selected stable values for an activated choice item.
pub(crate) fn next_selected_values(
    selection_mode: ChoiceSelectionMode,
    selection_required: bool,
    current: &[String],
    value: &str,
) -> Vec<String> {
    let selected = current.iter().any(|selected| selected == value);

    match selection_mode {
        ChoiceSelectionMode::None => Vec::new(),
        ChoiceSelectionMode::Single if selected && selection_required => current.to_vec(),
        ChoiceSelectionMode::Single if selected => Vec::new(),
        ChoiceSelectionMode::Single => vec![value.to_owned()],
        ChoiceSelectionMode::Multiple if selected && selection_required && current.len() <= 1 => {
            current.to_vec()
        }
        ChoiceSelectionMode::Multiple if selected => current
            .iter()
            .filter(|selected| selected.as_str() != value)
            .cloned()
            .collect(),
        ChoiceSelectionMode::Multiple => {
            let mut next = current.to_vec();
            next.push(value.to_owned());
            next
        }
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
    fn projected_selected_values_dedupes_and_filters_disabled_multi_select() {
        let items = [
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
            ChoiceItemProjection::new(1, None, "beta", "Beta", true, ()),
            ChoiceItemProjection::new(0, Some(0), "gamma", "Gamma", false, ()),
            ChoiceItemProjection::new(1, Some(0), "delta", "Delta", false, ()),
        ];

        let values = resolve_projected_selected_values(
            ChoiceSelectionMode::Multiple,
            &items,
            Some("alpha"),
            vec!["alpha", "gamma", "alpha", "beta", "delta"],
        );

        assert_eq!(values, vec!["alpha", "gamma", "delta"]);
    }

    #[test]
    fn projected_selected_values_keeps_first_valid_single_value() {
        let items = [
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", true, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", false, ()),
        ];

        let values = resolve_projected_selected_values(
            ChoiceSelectionMode::Single,
            &items,
            Some("alpha"),
            vec!["bravo", "alpha"],
        );

        assert_eq!(values, vec!["bravo"]);
    }

    #[test]
    fn projected_selected_values_filters_disabled_single_value() {
        let items = [ChoiceItemProjection::new(
            0,
            None,
            "alpha",
            "Alpha",
            true,
            (),
        )];

        let values = resolve_projected_selected_values(
            ChoiceSelectionMode::Single,
            &items,
            Some("alpha"),
            Vec::<String>::new(),
        );

        assert!(values.is_empty());
    }

    #[test]
    fn selection_resolution_uses_stable_value_and_enabled_fallbacks() {
        let items = vec![
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", true, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", false, ()),
            ChoiceItemProjection::new(2, None, "charlie", "Charlie", false, ()),
        ];

        let selected = ChoiceCollection::resolve(
            false,
            items.clone(),
            Some("bravo"),
            None,
            ChoiceInteractionPolicy::listbox(),
        );
        assert_eq!(selected.selected_index(), Some(1));
        assert_eq!(selected.active_index(), Some(1));

        let active = ChoiceCollection::resolve(
            false,
            items.clone(),
            Some("missing"),
            Some("charlie"),
            ChoiceInteractionPolicy::listbox(),
        );
        assert_eq!(active.selected_index(), None);
        assert_eq!(active.active_index(), Some(2));

        let fallback = ChoiceCollection::resolve(
            false,
            items.clone(),
            Some("alpha"),
            Some("alpha"),
            ChoiceInteractionPolicy::listbox(),
        );
        assert_eq!(fallback.selected_index(), None);
        assert_eq!(fallback.active_index(), Some(1));

        let disabled_surface = ChoiceCollection::resolve(
            true,
            items,
            Some("bravo"),
            None,
            ChoiceInteractionPolicy::listbox(),
        );
        assert_eq!(disabled_surface.selected_index(), None);
        assert_eq!(disabled_surface.active_index(), None);
    }

    #[test]
    fn collection_projects_selection_and_movement_from_policy() {
        let items = vec![
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", true, ()),
            ChoiceItemProjection::new(2, None, "charlie", "Charlie", false, ()),
        ];
        let collection = ChoiceCollection::resolve(
            false,
            items,
            Some("charlie"),
            Some("missing"),
            ChoiceInteractionPolicy::listbox(),
        );

        assert_eq!(collection.selected_value(), Some("charlie"));
        assert_eq!(collection.active_value(), Some("charlie"));
        assert_eq!(
            collection
                .navigation_target("down")
                .map(ChoiceItemProjection::value),
            Some("alpha")
        );
        assert_eq!(
            collection
                .typeahead_target(" al")
                .map(ChoiceItemProjection::value),
            Some("alpha")
        );
        assert_eq!(collection.disabled_map(), vec![false, true, false]);
    }

    #[test]
    fn unique_collection_keeps_ambiguous_values_visible_but_unaddressable() {
        let collection = ChoiceCollection::resolve_unique(
            false,
            vec![
                ChoiceItemProjection::new(0, Some(0), "shared", "First", false, ()),
                ChoiceItemProjection::new(1, Some(1), "shared", "Second", false, ()),
                ChoiceItemProjection::new(2, Some(1), "unique", "Unique", false, ()),
            ],
            Some("shared"),
            Some("shared"),
            ChoiceInteractionPolicy::single_optional(Orientation::Vertical),
        );

        assert_eq!(collection.selected_value(), None);
        assert_eq!(collection.active_value(), Some("unique"));
        assert_eq!(collection.disabled_map(), vec![true, true, false]);
        assert!(collection.items()[0].ambiguous_value());
        assert!(collection.items()[1].ambiguous_value());
        assert!(!collection.items()[2].ambiguous_value());
    }

    #[test]
    fn unique_collection_never_includes_ambiguous_values_as_disabled_targets() {
        let collection = ChoiceCollection::resolve_unique(
            false,
            vec![
                ChoiceItemProjection::new(0, Some(0), "shared", "First", false, ()),
                ChoiceItemProjection::new(1, Some(1), "shared", "Second", false, ()),
                ChoiceItemProjection::new(2, Some(1), "unique", "Unique", false, ()),
            ],
            Some("shared"),
            Some("shared"),
            ChoiceInteractionPolicy::single_optional(Orientation::Vertical)
                .with_typeahead(true)
                .with_disabled_item_strategy(ChoiceDisabledItemStrategy::Include),
        );

        assert_eq!(collection.selected_value(), None);
        assert_eq!(collection.active_value(), Some("unique"));
        assert_eq!(collection.disabled_map(), vec![true, true, false]);
        assert!(collection.typeahead_target("first").is_none());
    }

    #[test]
    fn collection_can_disable_wrapping_movement() {
        let items = vec![
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", true, ()),
            ChoiceItemProjection::new(2, None, "charlie", "Charlie", false, ()),
        ];
        let collection = ChoiceCollection::resolve(
            false,
            items,
            None,
            Some("charlie"),
            ChoiceInteractionPolicy::listbox().with_wrap(false),
        );

        assert!(collection.navigation_target("down").is_none());
        assert_eq!(
            collection
                .navigation_target("up")
                .map(ChoiceItemProjection::value),
            Some("alpha")
        );
    }

    #[test]
    fn collection_policies_cover_real_choice_consumers() {
        let items = vec![
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", true, ()),
            ChoiceItemProjection::new(2, None, "charlie", "Charlie", false, ()),
        ];

        let single_optional = ChoiceCollection::resolve(
            false,
            items.clone(),
            Some("missing"),
            None,
            ChoiceInteractionPolicy::single_optional(Orientation::Horizontal),
        );
        assert_eq!(single_optional.selected_index(), None);
        assert_eq!(single_optional.active_index(), Some(0));
        assert_eq!(
            single_optional
                .navigation_target("right")
                .map(ChoiceItemProjection::value),
            Some("charlie")
        );

        let single_required = ChoiceCollection::resolve(
            false,
            items.clone(),
            Some("missing"),
            None,
            ChoiceInteractionPolicy::single_required(Orientation::Vertical),
        );
        assert_eq!(single_required.selected_value(), Some("alpha"));
        assert_eq!(single_required.active_value(), Some("alpha"));

        let multiple = ChoiceCollection::resolve(
            false,
            items.clone(),
            Some("charlie"),
            Some("missing"),
            ChoiceInteractionPolicy::multiple(Orientation::Vertical),
        );
        assert_eq!(multiple.selected_value(), Some("charlie"));
        assert_eq!(
            next_selected_values(
                ChoiceSelectionMode::Multiple,
                false,
                &["alpha".to_string()],
                "charlie",
            ),
            vec!["alpha".to_string(), "charlie".to_string()]
        );

        let roving = ChoiceCollection::resolve(
            false,
            items,
            Some("charlie"),
            Some("missing"),
            ChoiceInteractionPolicy::roving(Orientation::Vertical),
        );
        assert_eq!(roving.selected_value(), None);
        assert_eq!(roving.active_value(), Some("alpha"));
    }

    #[test]
    fn collection_policy_controls_typeahead_and_disabled_items() {
        let items = vec![
            ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
            ChoiceItemProjection::new(1, None, "bravo", "Bravo", true, ()),
            ChoiceItemProjection::new(2, None, "beta", "Beta", false, ()),
        ];
        let disabled_typeahead = ChoiceCollection::resolve(
            false,
            items.clone(),
            None,
            Some("alpha"),
            ChoiceInteractionPolicy::listbox().with_typeahead(false),
        );
        let include_disabled = ChoiceCollection::resolve(
            false,
            items,
            None,
            Some("alpha"),
            ChoiceInteractionPolicy::listbox()
                .with_disabled_item_strategy(ChoiceDisabledItemStrategy::Include),
        );

        assert!(disabled_typeahead.typeahead_target("br").is_none());
        assert_eq!(
            include_disabled
                .typeahead_target("br")
                .map(ChoiceItemProjection::value),
            Some("bravo")
        );
    }

    #[test]
    fn typeahead_scans_from_active_item_and_skips_disabled_items() {
        let collection = ChoiceCollection::resolve(
            false,
            vec![
                ChoiceItemProjection::new(0, None, "alpha", "Alpha", false, ()),
                ChoiceItemProjection::new(1, None, "bravo", "Bravo", true, ()),
                ChoiceItemProjection::new(2, None, "beta", "Beta", false, ()),
            ],
            None,
            Some("alpha"),
            ChoiceInteractionPolicy::listbox(),
        );

        assert_eq!(
            collection
                .typeahead_target(" b")
                .map(ChoiceItemProjection::value),
            Some("beta")
        );

        let wrapped = ChoiceCollection::resolve(
            false,
            collection.into_items(),
            None,
            Some("beta"),
            ChoiceInteractionPolicy::listbox(),
        );
        assert_eq!(
            wrapped
                .typeahead_target("a")
                .map(ChoiceItemProjection::value),
            Some("alpha")
        );
        assert!(wrapped.typeahead_target("missing").is_none());
    }
}
