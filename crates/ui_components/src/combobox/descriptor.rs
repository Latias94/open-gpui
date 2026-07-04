use open_gpui::SharedString;

use crate::choice::{self, ChoiceItemProjection};
use crate::listbox::{ListboxGroup, ListboxOption, ListboxOptionDescriptor};

/// Pure descriptor for one combobox option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxOptionDescriptor {
    value: String,
    label: String,
    keywords: Vec<String>,
    disabled: bool,
}

impl ComboboxOptionDescriptor {
    /// Creates a selectable combobox option descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            keywords: Vec::new(),
            disabled: false,
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Adds many filtering keywords.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Marks the option as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns stable option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible option label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns filtering keywords.
    pub fn keywords_ref(&self) -> &[String] {
        &self.keywords
    }

    /// Returns whether the option is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    pub(super) fn matches_normalized_query(&self, normalized_query: &str) -> bool {
        let base_sources = [self.value.as_str(), self.label.as_str()];
        choice::query_matches_sources(
            normalized_query,
            base_sources
                .into_iter()
                .chain(self.keywords.iter().map(String::as_str)),
        )
    }

    pub(super) fn to_listbox_descriptor(&self) -> ListboxOptionDescriptor {
        ListboxOptionDescriptor::option(self.value.clone(), self.label.clone())
            .disabled(self.disabled)
    }
}

/// Pure descriptor for one combobox option group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxGroupDescriptor {
    value: String,
    label: String,
    options: Vec<ComboboxOptionDescriptor>,
}

impl ComboboxGroupDescriptor {
    /// Creates an empty combobox group descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            options: Vec::new(),
        }
    }

    /// Adds one option descriptor.
    pub fn option(mut self, option: ComboboxOptionDescriptor) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many option descriptors.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOptionDescriptor>) -> Self {
        self.options.extend(options);
        self
    }

    /// Returns stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns group options.
    pub fn options_ref(&self) -> &[ComboboxOptionDescriptor] {
        &self.options
    }
}

pub(super) fn flatten_combobox_choice_options(
    groups: &[ComboboxGroupDescriptor],
    standalone_options: &[ComboboxOptionDescriptor],
) -> Vec<ChoiceItemProjection<()>> {
    let mut flattened = standalone_options
        .iter()
        .enumerate()
        .map(|(source_index, descriptor)| {
            let text_value = descriptor.label().to_owned();
            ChoiceItemProjection::new(
                source_index,
                None,
                descriptor.value(),
                text_value.clone(),
                descriptor.disabled_state(),
                (),
            )
            .text_value(text_value)
        })
        .collect::<Vec<_>>();

    for (group_index, group) in groups.iter().enumerate() {
        flattened.extend(group.options_ref().iter().enumerate().map(
            |(source_index, descriptor)| {
                let text_value = descriptor.label().to_owned();
                ChoiceItemProjection::new(
                    source_index,
                    Some(group_index),
                    descriptor.value(),
                    text_value.clone(),
                    descriptor.disabled_state(),
                    (),
                )
                .text_value(text_value)
            },
        ));
    }

    flattened
}

/// A concrete GPUI combobox option.
#[derive(Clone)]
pub struct ComboboxOption {
    descriptor: ComboboxOptionDescriptor,
}

impl ComboboxOption {
    /// Creates a selectable combobox option.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ComboboxOptionDescriptor::new(value, label.to_string()),
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.keyword(keyword);
        self
    }

    /// Marks the option as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> ComboboxOptionDescriptor {
        self.descriptor.clone()
    }

    pub(super) fn matches_normalized_query(&self, normalized_query: &str) -> bool {
        self.descriptor.matches_normalized_query(normalized_query)
    }

    pub(super) fn listbox_option(self) -> ListboxOption {
        ListboxOption::new(self.descriptor.value, self.descriptor.label)
            .disabled(self.descriptor.disabled)
    }
}

/// A concrete GPUI combobox group.
#[derive(Clone)]
pub struct ComboboxGroup {
    descriptor: ComboboxGroupDescriptor,
    options: Vec<ComboboxOption>,
}

impl ComboboxGroup {
    /// Creates an empty combobox group.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ComboboxGroupDescriptor::new(value, label.to_string()),
            options: Vec::new(),
        }
    }

    /// Adds one option.
    pub fn option(mut self, option: ComboboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many options.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Returns the group descriptor.
    pub fn descriptor(&self) -> ComboboxGroupDescriptor {
        self.options
            .iter()
            .fold(self.descriptor.clone(), |descriptor, option| {
                descriptor.option(option.descriptor())
            })
    }

    pub(super) fn filtered_listbox_group(self, normalized_query: &str) -> Option<ListboxGroup> {
        let mut group = ListboxGroup::new(self.descriptor.value, self.descriptor.label);
        let mut has_options = false;
        for option in self.options {
            if option.matches_normalized_query(normalized_query) {
                has_options = true;
                group = group.option(option.listbox_option());
            }
        }
        has_options.then_some(group)
    }
}
