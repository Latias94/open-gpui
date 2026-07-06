use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens};
/// Resolved renderer-neutral state for a table toolbar recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableToolbarState {
    id: String,
    label: String,
    size: Size,
    primary_control_count: usize,
    secondary_control_count: usize,
    summary: Option<String>,
    tokens: ThemeTokens,
    colors: TableToolbarColors,
}

impl TableToolbarState {
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        size: Size,
        primary_control_count: usize,
        secondary_control_count: usize,
        summary: Option<impl Into<String>>,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::table_toolbar_colors(tokens);
        Self {
            id: id.into(),
            label: label.into(),
            size,
            primary_control_count,
            secondary_control_count,
            summary: summary.map(Into::into),
            tokens,
            colors,
        }
    }

    /// Returns stable recipe id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible or accessible toolbar label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the foundation size used for toolbar text and child recipes.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the number of primary controls in the first toolbar row.
    pub const fn primary_control_count(&self) -> usize {
        self.primary_control_count
    }

    /// Returns the number of secondary controls in the second toolbar row.
    pub const fn secondary_control_count(&self) -> usize {
        self.secondary_control_count
    }

    /// Returns the total number of slotted controls.
    pub const fn control_count(&self) -> usize {
        self.primary_control_count + self.secondary_control_count
    }

    /// Returns whether the toolbar has at least one slotted control.
    pub const fn has_controls(&self) -> bool {
        self.control_count() > 0
    }

    /// Returns the optional trailing summary text.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Returns whether the toolbar exposes a trailing summary.
    pub const fn has_summary(&self) -> bool {
        self.summary.is_some()
    }

    /// Returns accessibility role.
    pub const fn role(&self) -> Role {
        Role::Toolbar
    }

    /// Returns the token bundle used to resolve toolbar text colors.
    pub const fn tokens(&self) -> ThemeTokens {
        self.tokens
    }

    /// Returns resolved toolbar color intents.
    pub const fn colors(&self) -> TableToolbarColors {
        self.colors
    }

    /// Returns the foreground color intent for toolbar labels and controls.
    pub const fn foreground(&self) -> ColorIntent {
        self.colors.foreground()
    }

    /// Returns the muted foreground color intent for summary text.
    pub const fn muted_foreground(&self) -> ColorIntent {
        self.colors.muted_foreground()
    }
}

/// Resolved table toolbar color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableToolbarColors {
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
}

impl TableToolbarColors {
    /// Returns the foreground color intent for toolbar labels and controls.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the muted foreground color intent for summary text.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }
}

/// A table toolbar recipe for composing table filter controls and summary text.
#[derive(IntoElement)]
pub struct TableToolbar {
    id: String,
    label: SharedString,
    size: Size,
    primary_controls: Vec<AnyElement>,
    secondary_controls: Vec<AnyElement>,
    summary: Option<SharedString>,
    tokens: ThemeTokens,
}

impl TableToolbar {
    /// Creates a table toolbar recipe.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            size: Size::Medium,
            primary_controls: Vec::new(),
            secondary_controls: Vec::new(),
            summary: None,
            tokens: ThemeTokens::default(),
        }
    }

    /// Adds a primary control to the first toolbar row.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.primary_controls.push(control.into_any_element());
        self
    }

    /// Adds primary controls to the first toolbar row.
    pub fn controls(mut self, controls: impl IntoIterator<Item = impl IntoElement>) -> Self {
        for control in controls {
            self = self.control(control);
        }
        self
    }

    /// Adds a secondary control to the second toolbar row.
    pub fn secondary_control(mut self, control: impl IntoElement) -> Self {
        self.secondary_controls.push(control.into_any_element());
        self
    }

    /// Adds secondary controls to the second toolbar row.
    pub fn secondary_controls(
        mut self,
        controls: impl IntoIterator<Item = impl IntoElement>,
    ) -> Self {
        for control in controls {
            self = self.secondary_control(control);
        }
        self
    }

    /// Applies trailing summary text.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns resolved recipe state without exposing renderer-owned child elements.
    pub fn state(&self) -> TableToolbarState {
        TableToolbarState::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.size,
            self.primary_controls.len(),
            self.secondary_controls.len(),
            self.summary.as_ref().map(ToString::to_string),
            self.tokens,
        )
    }
}

impl Sizable for TableToolbar {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableToolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let debug_id = state.id().to_owned();
        let primary_debug_id = debug_id.clone();
        let secondary_debug_id = debug_id.clone();
        let summary_debug_id = debug_id.clone();
        let label = state.label().to_owned();
        let text_color = theme.resolve(state.foreground());
        let summary_text_color = theme.resolve(state.muted_foreground());
        let size = state.size();
        let has_primary_controls = state.primary_control_count() > 0;
        let has_secondary_controls = state.secondary_control_count() > 0;
        let has_summary = state.has_summary();
        let primary_controls = self.primary_controls;
        let secondary_controls = self.secondary_controls;
        let summary = self.summary;

        div()
            .id(self.id)
            .debug_selector(move || format!("table-toolbar:{debug_id}:root"))
            .ui_role(state.role())
            .aria_label(label)
            .min_w(px(0.0))
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(gpui_px_from_ui(size.control_text_px()))
            .text_color(text_color)
            .when(has_primary_controls, |this| {
                this.child(
                    div()
                        .debug_selector(move || {
                            format!("table-toolbar:{primary_debug_id}:primary-controls")
                        })
                        .min_w(px(0.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .flex_wrap()
                        .children(primary_controls),
                )
            })
            .when(has_secondary_controls || has_summary, |this| {
                this.child(
                    div()
                        .min_w(px(0.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .flex_wrap()
                        .when(has_secondary_controls, |row| {
                            row.child(
                                div()
                                    .debug_selector(move || {
                                        format!(
                                            "table-toolbar:{secondary_debug_id}:secondary-controls"
                                        )
                                    })
                                    .min_w(px(0.0))
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .flex_wrap()
                                    .children(secondary_controls),
                            )
                        })
                        .when_some(summary, |row, summary| {
                            row.child(
                                div()
                                    .debug_selector(move || {
                                        format!("table-toolbar:{summary_debug_id}:summary")
                                    })
                                    .flex_none()
                                    .text_xs()
                                    .text_color(summary_text_color)
                                    .child(summary),
                            )
                        }),
                )
            })
    }
}
