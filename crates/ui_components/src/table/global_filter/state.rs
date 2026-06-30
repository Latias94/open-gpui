use crate::text_input::TextInputState;
use open_gpui_ui_core::{Size, TableState, ThemeTokens};

/// Controlled payload emitted when a table global text filter changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableGlobalFilterChange {
    query: String,
    cleared: bool,
}

impl TableGlobalFilterChange {
    /// Creates a global-filter payload from the current query text.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            cleared: false,
        }
    }

    /// Creates a payload that clears the table global filter.
    pub fn clear() -> Self {
        Self {
            query: String::new(),
            cleared: true,
        }
    }

    /// Returns the query text exactly as entered by the user.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns true when this payload was created by a clear action.
    pub const fn cleared(&self) -> bool {
        self.cleared
    }

    /// Returns whether this payload carries a non-empty global query after trimming.
    pub fn active(&self) -> bool {
        !self.cleared && !self.query.trim().is_empty()
    }

    /// Applies this global-filter change to a table state and resets pagination to the first page.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let next_pagination = state.pagination().with_page_index(0);
        let state = if self.active() {
            state.with_global_filter(self.query.clone())
        } else {
            state.without_global_filter()
        };

        state.with_pagination(next_pagination)
    }
}

/// Resolved renderer-neutral state for a table global text filter recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGlobalFilterState {
    id: String,
    label: String,
    query: String,
    placeholder: String,
    clear_label: String,
    input: TextInputState,
}

impl TableGlobalFilterState {
    pub(in crate::table::global_filter) fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        query: impl Into<String>,
        placeholder: impl Into<String>,
        clear_label: impl Into<String>,
        size: Size,
        disabled: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let query = query.into();
        let placeholder = placeholder.into();
        let input = TextInputState::resolve(
            query.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );

        Self {
            id: id.into(),
            label: label.into(),
            query,
            placeholder,
            clear_label: clear_label.into(),
            input,
        }
    }

    /// Returns stable recipe id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible filter label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the current global query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns whether the global filter is active after trimming whitespace.
    pub fn active(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Returns whether the clear action should be available.
    pub fn clear_enabled(&self) -> bool {
        !self.query.is_empty()
    }

    /// Returns the input placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns the clear button label.
    pub fn clear_label(&self) -> &str {
        &self.clear_label
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.input.size()
    }

    /// Returns whether the filter input and clear action are disabled.
    pub const fn disabled(&self) -> bool {
        self.input.disabled()
    }

    /// Returns resolved text input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }
}
