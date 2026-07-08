use std::sync::Arc;

use super::descriptor::VirtualizedListItemDescriptor;

/// Renderer-neutral descriptor source for a `VirtualizedList`.
///
/// The data source owns the ordered rows that a concrete list will render. It is intended for
/// application code that wants to project domain records, section rows, and async status rows in
/// one place before handing the result to the component renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListDataSource {
    items: Arc<[VirtualizedListItemDescriptor]>,
}

/// Builder for [`VirtualizedListDataSource`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VirtualizedListDataSourceBuilder {
    items: Vec<VirtualizedListItemDescriptor>,
}

impl VirtualizedListDataSource {
    /// Creates a data source from virtualized-list descriptors.
    pub fn new(items: impl IntoIterator<Item = VirtualizedListItemDescriptor>) -> Self {
        Self {
            items: Arc::from(items.into_iter().collect::<Vec<_>>().into_boxed_slice()),
        }
    }

    /// Starts a data-source builder.
    pub fn builder() -> VirtualizedListDataSourceBuilder {
        VirtualizedListDataSourceBuilder::new()
    }

    /// Projects application records into virtualized-list descriptors.
    pub fn from_items<T>(
        items: impl IntoIterator<Item = T>,
        project: impl FnMut(T) -> VirtualizedListItemDescriptor,
    ) -> Self {
        Self::new(items.into_iter().map(project))
    }

    /// Creates an initial loading data source.
    pub fn loading(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new([VirtualizedListItemDescriptor::loading(key, message)])
    }

    /// Creates an empty data source.
    pub fn empty(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new([VirtualizedListItemDescriptor::empty(key, message)])
    }

    /// Creates an error data source.
    pub fn error(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new([VirtualizedListItemDescriptor::error(key, message)])
    }

    /// Creates a retry data source.
    pub fn retry(
        key: impl Into<String>,
        message: impl Into<String>,
        action_label: impl Into<String>,
    ) -> Self {
        Self::new([VirtualizedListItemDescriptor::retry(
            key,
            message,
            action_label,
        )])
    }

    /// Returns the ordered descriptors.
    pub fn items(&self) -> &[VirtualizedListItemDescriptor] {
        &self.items
    }

    /// Returns shared descriptor storage for `VirtualizedList::from_shared_items`.
    pub fn shared_items(&self) -> Arc<[VirtualizedListItemDescriptor]> {
        self.items.clone()
    }

    /// Consumes this source into shared descriptor storage.
    pub fn into_shared_items(self) -> Arc<[VirtualizedListItemDescriptor]> {
        self.items
    }

    /// Returns the number of rows in this data source.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true when the data source has no rows.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of selectable item rows.
    pub fn selectable_count(&self) -> usize {
        self.items.iter().filter(|item| item.selectable()).count()
    }
}

impl From<Vec<VirtualizedListItemDescriptor>> for VirtualizedListDataSource {
    fn from(items: Vec<VirtualizedListItemDescriptor>) -> Self {
        Self::new(items)
    }
}

impl From<Arc<[VirtualizedListItemDescriptor]>> for VirtualizedListDataSource {
    fn from(items: Arc<[VirtualizedListItemDescriptor]>) -> Self {
        Self { items }
    }
}

impl VirtualizedListDataSourceBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one descriptor.
    pub fn item(mut self, item: VirtualizedListItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Appends descriptors.
    pub fn items(mut self, items: impl IntoIterator<Item = VirtualizedListItemDescriptor>) -> Self {
        self.items.extend(items);
        self
    }

    /// Projects and appends application records.
    pub fn mapped_items<T>(
        mut self,
        items: impl IntoIterator<Item = T>,
        project: impl FnMut(T) -> VirtualizedListItemDescriptor,
    ) -> Self {
        self.items.extend(items.into_iter().map(project));
        self
    }

    /// Appends a non-selectable section row.
    pub fn section(self, key: impl Into<String>, label: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::section(key, label))
    }

    /// Appends a non-selectable separator row.
    pub fn separator(self, key: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::separator(key))
    }

    /// Appends a non-selectable initial loading row.
    pub fn loading(self, key: impl Into<String>, message: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::loading(key, message))
    }

    /// Appends a non-selectable prepend loading row.
    pub fn prepend_loading(self, key: impl Into<String>, message: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::prepend_loading(key, message))
    }

    /// Appends a non-selectable append loading row.
    pub fn append_loading(self, key: impl Into<String>, message: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::append_loading(key, message))
    }

    /// Appends a non-selectable exhausted row.
    pub fn exhausted(self, key: impl Into<String>, message: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::exhausted(key, message))
    }

    /// Appends a non-selectable empty row.
    pub fn empty(self, key: impl Into<String>, message: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::empty(key, message))
    }

    /// Appends a non-selectable error row.
    pub fn error(self, key: impl Into<String>, message: impl Into<String>) -> Self {
        self.item(VirtualizedListItemDescriptor::error(key, message))
    }

    /// Appends a non-selectable retry row.
    pub fn retry(
        self,
        key: impl Into<String>,
        message: impl Into<String>,
        action_label: impl Into<String>,
    ) -> Self {
        self.item(VirtualizedListItemDescriptor::retry(
            key,
            message,
            action_label,
        ))
    }

    /// Appends an empty row only when no selectable item rows have been added.
    pub fn empty_when_no_selectable(
        self,
        key: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        if self.items.iter().any(|item| item.selectable()) {
            self
        } else {
            self.empty(key, message)
        }
    }

    /// Builds the data source.
    pub fn build(self) -> VirtualizedListDataSource {
        VirtualizedListDataSource::new(self.items)
    }
}
