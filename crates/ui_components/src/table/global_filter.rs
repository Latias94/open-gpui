use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::geometry::gpui_px_from_ui;
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use open_gpui_ui_core::{Sizable, Size, TableState, ThemeTokens};

type TableGlobalFilterChangeHandler = Rc<dyn Fn(TableGlobalFilterChange, &mut Window, &mut App)>;
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
    fn resolve(
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

#[derive(Debug, Clone)]
struct TableGlobalFilterRuntime {
    query: String,
}

/// A compact text input recipe for controlling a table global filter.
#[derive(IntoElement)]
pub struct TableGlobalFilter {
    id: String,
    label: SharedString,
    query: Option<String>,
    default_query: String,
    placeholder: SharedString,
    clear_label: SharedString,
    size: Size,
    disabled: bool,
    tokens: ThemeTokens,
    on_change: Option<TableGlobalFilterChangeHandler>,
}

impl TableGlobalFilter {
    /// Creates a global filter recipe for a table.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            query: None,
            default_query: String::new(),
            placeholder: "Search rows".into(),
            clear_label: "Clear search".into(),
            size: Size::Medium,
            disabled: false,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Applies controlled query text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Applies the default query for adapter-owned input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        self.default_query = query.into();
        self
    }

    /// Applies input placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies the clear button label.
    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = label.into();
        self
    }

    /// Marks the filter input and clear action as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a global-filter query-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TableGlobalFilterChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state.
    pub fn state(&self) -> TableGlobalFilterState {
        let query = self.query.as_deref().unwrap_or(self.default_query.as_str());
        TableGlobalFilterState::resolve(
            self.id.clone(),
            self.label.to_string(),
            query,
            self.placeholder.to_string(),
            self.clear_label.to_string(),
            self.size,
            self.disabled,
            self.tokens,
        )
    }
}

impl Sizable for TableGlobalFilter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableGlobalFilter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableGlobalFilterRuntime {
            query: self.default_query.clone(),
        });
        let controlled_query = self.query.clone();
        let runtime_query = runtime.read(cx).query.clone();
        let query = controlled_query.clone().unwrap_or(runtime_query);

        if controlled_query.is_some() && runtime.read(cx).query != query {
            runtime.update(cx, |runtime, _| {
                runtime.query = query.clone();
            });
        }

        let state = TableGlobalFilterState::resolve(
            self.id.clone(),
            self.label.clone(),
            query.clone(),
            self.placeholder.clone(),
            self.clear_label.clone(),
            self.size,
            self.disabled,
            self.tokens,
        );
        let debug_id = state.id().to_owned();
        let label = state.label().to_owned();
        let placeholder = state.placeholder().to_owned();
        let clear_label = state.clear_label().to_owned();
        let clear_enabled = state.clear_enabled();
        let disabled = state.disabled();
        let size = state.size();
        let text_color = ThemeResolver::resolve(state.input().colors().foreground());
        let input_id = format!("{}-input", self.id);
        let clear_id = format!("{}-clear", self.id);
        let runtime_for_input = runtime.clone();
        let runtime_for_clear = runtime.clone();
        let on_change_for_input = self.on_change.clone();
        let on_change_for_clear = self.on_change.clone();

        div()
            .id(self.id)
            .debug_selector(move || format!("table-global-filter:{debug_id}:root"))
            .min_w(px(0.0))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .text_size(gpui_px_from_ui(size.control_text_px()))
            .text_color(text_color)
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            )
            .child(
                div().min_w(px(0.0)).flex_1().child(
                    TextInput::new(input_id, label)
                        .with_size(size)
                        .value(query)
                        .placeholder(placeholder)
                        .disabled(disabled)
                        .tokens(self.tokens)
                        .on_change(move |next_query, window, cx| {
                            runtime_for_input.update(cx, |runtime, _| {
                                runtime.query = next_query.clone();
                            });
                            if let Some(on_change) = on_change_for_input.as_ref() {
                                on_change(TableGlobalFilterChange::new(next_query), window, cx);
                            }
                        }),
                ),
            )
            .when(clear_enabled, |this| {
                this.child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            runtime_for_clear.update(cx, |runtime, _| {
                                runtime.query.clear();
                            });
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                on_change(TableGlobalFilterChange::clear(), window, cx);
                            }
                        }),
                )
            })
    }
}
