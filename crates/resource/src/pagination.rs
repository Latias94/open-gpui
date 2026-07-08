use serde_json::Value;

use crate::{QueryKey, RedactedResourceValue, ResourceRedactionPolicy};

/// One raw page of resource data before redaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePage {
    cursor: Option<String>,
    items: Vec<Value>,
}

impl ResourcePage {
    /// Creates a resource page.
    pub fn new(cursor: Option<String>, items: impl IntoIterator<Item = Value>) -> Self {
        Self {
            cursor,
            items: items.into_iter().collect(),
        }
    }
}

/// Page snapshot after redaction has been applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePageSnapshot {
    /// Cursor for fetching the next page.
    pub cursor: Option<String>,
    /// Redacted page items.
    pub items: Vec<RedactedResourceValue>,
}

/// Ordered paginated resource data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedResourceSnapshot {
    key: QueryKey,
    pages: Vec<ResourcePage>,
}

impl PaginatedResourceSnapshot {
    /// Creates an empty paginated resource.
    pub fn new(key: QueryKey) -> Self {
        Self {
            key,
            pages: Vec::new(),
        }
    }

    /// Appends a page.
    pub fn push_page(&mut self, page: ResourcePage) {
        self.pages.push(page);
    }

    /// Returns a redaction-aware snapshot.
    pub fn snapshot(&self, redaction: ResourceRedactionPolicy) -> PaginatedResourceSnapshotView {
        PaginatedResourceSnapshotView {
            key: self.key.clone(),
            pages: self
                .pages
                .iter()
                .map(|page| ResourcePageSnapshot {
                    cursor: page.cursor.clone(),
                    items: page
                        .items
                        .iter()
                        .cloned()
                        .map(|item| redaction.apply(item))
                        .collect(),
                })
                .collect(),
        }
    }
}

/// Redaction-aware paginated resource snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedResourceSnapshotView {
    /// Stable query key.
    pub key: QueryKey,
    /// Pages in insertion order.
    pub pages: Vec<ResourcePageSnapshot>,
}
