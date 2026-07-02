//! Textarea component.

use crate::geometry::gpui_px_from_ui;
use std::{ops::Range, rc::Rc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, Context, CursorStyle, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    ParentElement, Pixels, Point, RenderOnce, ScrollHandle, ShapedLine, SharedString, Style,
    Styled, TextRun, UTF16Selection, Window, div, fill, point, px, relative, rgba,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::text_editing::{
    self, EditableTextDocument, TextEditingPolicy, TextEditingProjection, TextSelection,
};
use crate::theme::ThemeResolver;

type TextareaChangeHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// Resolved textarea color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextareaColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) placeholder: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl TextareaColors {
    /// Returns the background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the placeholder color intent.
    pub const fn placeholder(self) -> ColorIntent {
        self.placeholder
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved textarea metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextareaMetrics {
    rows: usize,
    min_height: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
    line_height: UiPx,
    scrollbar_width: UiPx,
}

impl TextareaMetrics {
    /// Resolves metrics from the shared foundation size vocabulary and row count.
    pub fn from_size_and_rows(size: Size, rows: usize) -> Self {
        let rows = rows.max(1);
        let padding_y = size.input_py().max(ui_px(6.0));
        let line_height = textarea_line_height(size);

        Self {
            rows,
            min_height: line_height * rows as f32 + padding_y * 2.0 + ui_px(2.0),
            padding_x: size.input_px(),
            padding_y,
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            line_height,
            scrollbar_width: match size {
                Size::XSmall => ui_px(6.0),
                Size::Small => ui_px(8.0),
                Size::Medium => ui_px(8.0),
                Size::Large => ui_px(10.0),
            },
        }
    }

    /// Returns the preferred row count.
    pub const fn rows(self) -> usize {
        self.rows
    }

    /// Returns the minimum viewport height.
    pub const fn min_height(self) -> UiPx {
        self.min_height
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }

    /// Returns the corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns the text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns the line height.
    pub const fn line_height(self) -> UiPx {
        self.line_height
    }

    /// Returns the layout space reserved for the scrollbar.
    pub const fn scrollbar_width(self) -> UiPx {
        self.scrollbar_width
    }
}

/// Resolved textarea state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TextareaState {
    value: String,
    placeholder: Option<String>,
    size: Size,
    rows: usize,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    controller_driven: bool,
    metrics: TextareaMetrics,
    colors: TextareaColors,
    focus_ring: FocusRing,
}

impl TextareaState {
    /// Resolves the public state for a textarea.
    pub fn resolve(
        value: impl Into<String>,
        placeholder: Option<impl Into<String>>,
        size: Size,
        rows: usize,
        disabled: bool,
        read_only: bool,
        invalid: bool,
        required: bool,
        controller_driven: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::textarea_colors(tokens, disabled, read_only, invalid);

        let value = value.into();

        Self {
            value: TextEditingPolicy::multiline().normalize_text(value.as_str()),
            placeholder: placeholder.map(Into::into),
            size,
            rows: rows.max(1),
            disabled,
            read_only,
            invalid,
            required,
            controller_driven,
            metrics: TextareaMetrics::from_size_and_rows(size, rows),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }

    /// Returns whether the value is empty.
    pub fn value_is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Returns whether the textarea has a non-empty value.
    pub fn has_value(&self) -> bool {
        !self.value_is_empty()
    }

    /// Returns whether placeholder text should be visible.
    pub fn placeholder_visible(&self) -> bool {
        self.value.is_empty() && self.placeholder.is_some()
    }

    /// Returns whether display text comes from the placeholder.
    pub fn displaying_placeholder(&self) -> bool {
        self.placeholder_visible()
    }

    /// Returns the text that should be rendered by the display adapter.
    pub fn display_text(&self) -> &str {
        if self.placeholder_visible() {
            self.placeholder().unwrap_or("")
        } else {
            self.value()
        }
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the preferred row count.
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Returns whether the textarea is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the textarea is read-only.
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns whether the textarea is invalid.
    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    /// Returns whether the textarea is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether this state is backed by an editable controller.
    pub const fn controller_driven(&self) -> bool {
        self.controller_driven
    }

    /// Returns whether text editing should be accepted.
    pub const fn input_enabled(&self) -> bool {
        !self.disabled && !self.read_only
    }

    /// Returns whether text editing should be accepted.
    pub const fn editable(&self) -> bool {
        self.input_enabled()
    }

    /// Returns whether activation/edit handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        self.input_enabled()
    }

    /// Returns whether the element should be included in tab traversal.
    pub const fn tab_stop_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::TextInput
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TextareaMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> TextareaColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

#[derive(Debug, Clone)]
struct TextareaRuntime {
    scroll_handle: ScrollHandle,
}

impl Default for TextareaRuntime {
    fn default() -> Self {
        Self {
            scroll_handle: ScrollHandle::new(),
        }
    }
}

struct TextareaController {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Vec<TextareaLayoutLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    disabled: bool,
    read_only: bool,
    on_change: Option<TextareaChangeHandler>,
}

impl std::fmt::Debug for TextareaController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextareaController")
            .field("focus_handle", &self.focus_handle)
            .field("content", &self.content)
            .field("placeholder", &self.placeholder)
            .field("selected_range", &self.selected_range)
            .field("selection_reversed", &self.selection_reversed)
            .field("marked_range", &self.marked_range)
            .field("last_layout", &self.last_layout)
            .field("last_bounds", &self.last_bounds)
            .field("is_selecting", &self.is_selecting)
            .field("disabled", &self.disabled)
            .field("read_only", &self.read_only)
            .field("on_change", &self.on_change.as_ref().map(|_| "<handler>"))
            .finish()
    }
}

impl TextareaController {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            is_selecting: false,
            disabled: false,
            read_only: false,
            on_change: None,
        }
    }

    fn with_value(value: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let mut this = Self::new(cx);
        let value = value.into();
        let projection = EditableTextDocument::new(value.as_ref(), TextEditingPolicy::multiline())
            .into_projection();
        this.apply_editing_projection(projection);
        this
    }

    fn value(&self) -> &str {
        self.content.as_ref()
    }

    fn placeholder(&self) -> &str {
        self.placeholder.as_ref()
    }

    fn selected_range(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    fn accepts_editing(&self) -> bool {
        !self.disabled && !self.read_only
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn sync_adapter_state(
        &mut self,
        controlled_value: Option<&str>,
        placeholder: Option<SharedString>,
        disabled: bool,
        read_only: bool,
        on_change: Option<TextareaChangeHandler>,
    ) {
        self.disabled = disabled;
        self.read_only = read_only;
        self.on_change = on_change;

        if let Some(placeholder) = placeholder {
            self.placeholder = placeholder;
        }

        if let Some(value) = controlled_value {
            let value = text_editing::normalize_multiline(value);
            if self.content.as_ref() != value {
                self.content = value.into();
                let cursor =
                    text_editing::clamp_to_char_boundary(self.value(), self.cursor_offset());
                self.selected_range = cursor..cursor;
                self.selection_reversed = false;
                self.marked_range = None;
            }
        }
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        text_editing::offset_to_utf16(self.value(), offset)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        text_editing::range_from_utf16(self.value(), range_utf16)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        text_editing::range_to_utf16(self.value(), range)
    }

    fn editing_policy(&self) -> TextEditingPolicy {
        TextEditingPolicy::multiline()
    }

    fn selection(&self) -> TextSelection {
        let anchor = if self.selection_reversed {
            self.selected_range.end
        } else {
            self.selected_range.start
        };
        TextSelection::from_offsets(anchor, self.cursor_offset())
    }

    fn document(&self) -> EditableTextDocument {
        EditableTextDocument::from_parts(
            self.value(),
            self.selection(),
            self.marked_range.clone(),
            self.editing_policy(),
        )
    }

    fn apply_editing_projection(&mut self, projection: TextEditingProjection) {
        self.content = projection.text().into();
        self.selected_range = projection.selection().range();
        self.selection_reversed = projection.selection().reversed();
        self.marked_range = projection.marked_range();
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        let line_height = textarea_layout_line_height(bounds, &self.last_layout);
        let local_y = (position.y - bounds.top()).max(px(0.0));
        let line_index = ((local_y / line_height).floor() as usize)
            .min(self.last_layout.len().saturating_sub(1));
        let Some(line) = self.last_layout.get(line_index) else {
            return self.content.len();
        };

        let local_x = (position.x - bounds.left()).max(px(0.0));
        line.stored_offset_for_x(local_x, self.content.as_ref())
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let projection = self.document().move_to(offset);
        self.apply_editing_projection(projection);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let projection = self.document().select_to(offset);
        self.apply_editing_projection(projection);
        cx.notify();
    }

    fn replace_text_in_range_inner(&mut self, range_utf16: Option<Range<usize>>, new_text: &str) {
        let projection = self.document().replace_text_in_range(range_utf16, new_text);
        self.apply_editing_projection(projection);
    }

    fn replace_and_mark_text_in_range_inner(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        selected_utf16: Option<Range<usize>>,
    ) {
        let projection =
            self.document()
                .replace_and_mark_text_in_range(range_utf16, new_text, selected_utf16);
        self.apply_editing_projection(projection);
    }

    fn dispatch_change(&self, window: &mut Window, cx: &mut App) {
        if let Some(on_change) = self.on_change.as_ref().cloned() {
            on_change(self.content.to_string(), window, cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }
}

impl Focusable for TextareaController {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TextareaController {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.range_to_utf16(&range));
        self.content.get(range).map(ToString::to_string)
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.disabled && !ignore_disabled_input {
            return None;
        }
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_editing() {
            return;
        }
        let previous = self.content.clone();
        self.replace_text_in_range_inner(range_utf16, new_text);
        if previous != self.content {
            self.dispatch_change(window, cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        selected_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_editing() {
            return;
        }
        let previous = self.content.clone();
        self.replace_and_mark_text_in_range_inner(range_utf16, new_text, selected_utf16);
        if previous != self.content {
            self.dispatch_change(window, cx);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let bounds = self.last_bounds.unwrap_or(bounds);
        let line_height = textarea_layout_line_height(&bounds, &self.last_layout);
        let line = layout_line_for_offset(&self.last_layout, range.end)?;
        let x = line.x_for_stored_offset(range.end, self.content.as_ref());
        Some(Bounds::from_corners(
            point(
                bounds.left() + x,
                bounds.top() + line_height * line.index as f32,
            ),
            point(
                bounds.left() + x + px(1.0),
                bounds.top() + line_height * (line.index + 1) as f32,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.accepts_editing()
    }
}

/// A concrete GPUI textarea component shell.
#[derive(IntoElement)]
pub struct Textarea {
    id: ElementId,
    label: SharedString,
    value: SharedString,
    placeholder: Option<SharedString>,
    rows: usize,
    size: Size,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    tokens: ThemeTokens,
    on_change: Option<TextareaChangeHandler>,
}

impl Textarea {
    /// Creates a new textarea with an id and accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: SharedString::default(),
            placeholder: None,
            rows: 3,
            size: Size::Medium,
            disabled: false,
            read_only: false,
            invalid: false,
            required: false,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Sets the displayed value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        let value = value.into();
        self.value = EditableTextDocument::new(value.as_ref(), TextEditingPolicy::multiline())
            .into_projection()
            .text()
            .into();
        self
    }

    /// Registers a controlled value-change handler.
    ///
    /// Newline input is preserved as `\n`. Callers should feed the accepted value back through
    /// [`Textarea::value`] on the next render.
    pub fn on_change(mut self, handler: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Sets the placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets the preferred visible row count.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Marks the textarea as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the textarea as read-only.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Marks the textarea as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Marks the textarea as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved textarea state.
    pub fn state(&self) -> TextareaState {
        TextareaState::resolve(
            self.value.to_string(),
            self.placeholder.as_ref().map(ToString::to_string),
            self.size,
            self.rows,
            self.disabled,
            self.read_only,
            self.invalid,
            self.required,
            self.on_change.is_some(),
            self.tokens,
        )
    }
}

impl Sizable for Textarea {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Textarea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let debug_id = self.id.to_string();
        let controller_id = format!("textarea:{debug_id}:controller");
        let runtime_id: ElementId = (self.id.clone(), "runtime").into();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let text_color = theme.resolve(if state.placeholder_visible() {
            colors.placeholder()
        } else {
            colors.foreground()
        });
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TextareaRuntime::default());
        let scroll_handle = runtime.read(cx).scroll_handle.clone();
        let controller = self.on_change.as_ref().map(|_| {
            let initial_value = self.value.clone();
            window.use_keyed_state(controller_id, cx, |_, cx| {
                TextareaController::with_value(initial_value, cx)
            })
        });

        if let Some(controller) = controller.as_ref() {
            let controlled_value = self.on_change.as_ref().map(|_| self.value.as_ref());
            let placeholder = self.placeholder.clone();
            let on_change = self.on_change.clone();
            controller.update(cx, |controller, _cx| {
                controller.sync_adapter_state(
                    controlled_value,
                    placeholder,
                    state.disabled(),
                    state.read_only(),
                    on_change,
                );
            });
        }

        let controller_text = controller.as_ref().map(|controller| controller.read(cx));
        let placeholder = controller_text
            .as_ref()
            .map(|controller| controller.placeholder().to_owned().into())
            .filter(|placeholder: &SharedString| !placeholder.is_empty())
            .or(self.placeholder.clone())
            .unwrap_or_default();
        let show_placeholder = controller_text
            .as_ref()
            .map(|controller| controller.value().is_empty() && !placeholder.is_empty())
            .unwrap_or_else(|| state.placeholder_visible());
        let static_display_text = if show_placeholder {
            placeholder.clone()
        } else {
            self.value.clone()
        };

        let root_debug_id = debug_id.clone();

        div()
            .id(self.id)
            .debug_selector(move || format!("textarea:{root_debug_id}:root"))
            .h(gpui_px_from_ui(metrics.min_height()))
            .w_full()
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.background()))
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.line_height()))
            .text_color(text_color)
            .overflow_hidden()
            .overflow_y_scroll()
            .scrollbar_width(gpui_px_from_ui(metrics.scrollbar_width()))
            .track_scroll(&scroll_handle)
            .on_scroll_wheel(|_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .focusable()
            .tab_stop(state.tab_stop_enabled())
            .ui_role(state.role())
            .aria_label(self.label)
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(state.disabled(), |this| {
                this.opacity(0.56).cursor_not_allowed()
            })
            .when(state.input_enabled(), |this| {
                this.cursor(CursorStyle::IBeam)
            })
            .when(state.read_only() && !state.disabled(), |this| {
                this.cursor_default()
            })
            .when_some(controller.clone(), |this, controller| {
                let focus = controller.focus_handle(cx);
                let mouse_down = controller.clone();
                let mouse_up = controller.clone();
                let mouse_up_out = controller.clone();
                let mouse_move = controller.clone();

                this.track_focus(&focus)
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        mouse_down.update(cx, |controller, cx| {
                            controller.on_mouse_down(event, window, cx);
                        });
                    })
                    .on_mouse_up(MouseButton::Left, move |event, window, cx| {
                        mouse_up.update(cx, |controller, cx| {
                            controller.on_mouse_up(event, window, cx);
                        });
                    })
                    .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
                        mouse_up_out.update(cx, |controller, cx| {
                            controller.on_mouse_up(event, window, cx);
                        });
                    })
                    .on_mouse_move(move |event, window, cx| {
                        mouse_move.update(cx, |controller, cx| {
                            controller.on_mouse_move(event, window, cx);
                        });
                    })
            })
            .child(
                div()
                    .min_w(px(0.0))
                    .w_full()
                    .children(
                        controller
                            .into_iter()
                            .map(|controller| EditableTextareaElement {
                                controller,
                                placeholder: placeholder.clone(),
                                text_color: theme.resolve(colors.foreground()).into(),
                                placeholder_color: theme.resolve(colors.placeholder()).into(),
                                selection_color: rgba(0x2f80ed33).into(),
                                caret_color: theme.resolve(colors.foreground()).into(),
                                text_size: gpui_px_from_ui(metrics.text_size()).into(),
                                line_height: gpui_px_from_ui(metrics.line_height()),
                                min_rows: metrics.rows(),
                            }),
                    )
                    .when(!state.controller_driven(), |this| {
                        this.children(render_static_textarea_lines(
                            debug_id.as_ref(),
                            static_display_text.as_ref(),
                            show_placeholder,
                        ))
                    }),
            )
    }
}

struct EditableTextareaElement {
    controller: Entity<TextareaController>,
    placeholder: SharedString,
    text_color: Hsla,
    placeholder_color: Hsla,
    selection_color: Hsla,
    caret_color: Hsla,
    text_size: Pixels,
    line_height: Pixels,
    min_rows: usize,
}

struct EditableTextareaPrepaint {
    lines: Vec<TextareaLayoutLine>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for EditableTextareaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditableTextareaElement {
    type RequestLayoutState = ();
    type PrepaintState = EditableTextareaPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let controller = self.controller.read(cx);
        let line_count = text_line_count(controller.value()).max(self.min_rows);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (self.line_height * line_count as f32)
            .max(window.line_height())
            .into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let controller = self.controller.read(cx);
        let content = controller.value();
        let selected_range = controller.selected_range();
        let cursor = controller.cursor_offset();
        let show_placeholder = content.is_empty() && !self.placeholder.is_empty();
        let display_text = if show_placeholder {
            self.placeholder.as_ref()
        } else {
            content
        };
        let text_color = if show_placeholder {
            self.placeholder_color
        } else {
            self.text_color
        };
        let font = window.text_style().font();
        let mut lines = Vec::new();

        for (index, line) in textarea_line_slices(display_text).into_iter().enumerate() {
            let text: SharedString = line.text.to_owned().into();
            let shaped = window.text_system().shape_line(
                text.clone(),
                self.text_size,
                &[TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: text_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            lines.push(TextareaLayoutLine {
                index,
                start: if show_placeholder { 0 } else { line.start },
                end: if show_placeholder { 0 } else { line.end },
                shaped,
            });
        }

        while lines.len() < self.min_rows {
            let text: SharedString = "".into();
            let shaped = window.text_system().shape_line(
                text.clone(),
                self.text_size,
                &[TextRun {
                    len: 0,
                    font: font.clone(),
                    color: text_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            lines.push(TextareaLayoutLine {
                index: lines.len(),
                start: content.len(),
                end: content.len(),
                shaped,
            });
        }

        let cursor = selected_range.is_empty().then(|| {
            let line = layout_line_for_offset(&lines, cursor).unwrap_or_else(|| &lines[0]);
            let x = line.x_for_stored_offset(cursor, content);
            let y = self.line_height * line.index as f32;
            fill(
                Bounds::from_corners(
                    point(bounds.left() + x, bounds.top() + y),
                    point(
                        bounds.left() + x + px(1.0),
                        bounds.top() + y + self.line_height,
                    ),
                ),
                self.caret_color,
            )
        });
        let selections = selection_quads_for_range(
            &lines,
            &selected_range,
            content,
            bounds,
            self.line_height,
            self.selection_color,
        );

        let _ = controller;

        EditableTextareaPrepaint {
            lines,
            cursor,
            selections,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.controller.focus_handle(cx);
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.controller.clone()),
            cx,
        );

        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }

        let lines = std::mem::take(&mut prepaint.lines);
        for line in &lines {
            let origin = point(
                bounds.left(),
                bounds.top() + self.line_height * line.index as f32,
            );
            let _ = line.shaped.paint(
                origin,
                self.line_height,
                open_gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.controller.update(cx, |controller, _cx| {
            controller.last_layout = lines;
            controller.last_bounds = Some(bounds);
        });
    }
}

struct TextareaLineSlice<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct TextareaLayoutLine {
    index: usize,
    start: usize,
    end: usize,
    shaped: ShapedLine,
}

impl TextareaLayoutLine {
    fn x_for_stored_offset(&self, offset: usize, content: &str) -> Pixels {
        let offset =
            text_editing::clamp_to_char_boundary(content, offset.clamp(self.start, self.end));
        self.shaped.x_for_index(offset.saturating_sub(self.start))
    }

    fn stored_offset_for_x(&self, x: Pixels, content: &str) -> usize {
        let line_offset = self.shaped.closest_index_for_x(x);
        text_editing::clamp_to_char_boundary(
            content,
            self.start + line_offset.min(self.end - self.start),
        )
    }
}

fn render_static_textarea_lines(
    debug_id: &str,
    text: &str,
    placeholder: bool,
) -> Vec<impl IntoElement> {
    let debug_id = debug_id.to_owned();
    let lines = textarea_line_slices(text);
    lines
        .into_iter()
        .enumerate()
        .map(move |(line_index, line)| {
            let selector = format!("textarea:{debug_id}:line:{line_index}");
            div()
                .debug_selector(move || selector.clone())
                .min_h(px(0.0))
                .when(placeholder, |this| this.opacity(0.92))
                .child(line.text.to_owned())
        })
        .collect()
}

fn textarea_line_height(size: Size) -> UiPx {
    match size {
        Size::XSmall => ui_px(18.0),
        Size::Small => ui_px(19.0),
        Size::Medium => ui_px(20.0),
        Size::Large => ui_px(22.0),
    }
}

fn text_line_count(text: &str) -> usize {
    textarea_line_slices(text).len()
}

fn textarea_line_slices(text: &str) -> Vec<TextareaLineSlice<'_>> {
    if text.is_empty() {
        return vec![TextareaLineSlice {
            text: "",
            start: 0,
            end: 0,
        }];
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            lines.push(TextareaLineSlice {
                text: &text[start..idx],
                start,
                end: idx,
            });
            start = idx + ch.len_utf8();
        }
    }
    lines.push(TextareaLineSlice {
        text: &text[start..],
        start,
        end: text.len(),
    });
    lines
}

fn textarea_layout_line_height(bounds: &Bounds<Pixels>, lines: &[TextareaLayoutLine]) -> Pixels {
    if lines.is_empty() {
        return bounds.size.height.max(px(1.0));
    }
    (bounds.size.height / lines.len() as f32).max(px(1.0))
}

fn layout_line_for_offset(
    lines: &[TextareaLayoutLine],
    offset: usize,
) -> Option<&TextareaLayoutLine> {
    lines
        .iter()
        .find(|line| offset >= line.start && offset <= line.end)
        .or_else(|| lines.last())
}

fn selection_quads_for_range(
    lines: &[TextareaLayoutLine],
    range: &Range<usize>,
    content: &str,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    color: Hsla,
) -> Vec<PaintQuad> {
    if range.is_empty() {
        return Vec::new();
    }

    let start = range.start.min(range.end);
    let end = range.start.max(range.end);
    lines
        .iter()
        .filter_map(|line| {
            if end < line.start || start > line.end {
                return None;
            }

            let line_start = start.max(line.start);
            let line_end = end.min(line.end);
            let x1 = if start <= line.start {
                px(0.0)
            } else {
                line.x_for_stored_offset(line_start, content)
            };
            let x2 = if end >= line.end {
                line.shaped.width.max(x1 + px(1.0))
            } else {
                line.x_for_stored_offset(line_end, content)
                    .max(x1 + px(1.0))
            };
            let y = line_height * line.index as f32;
            Some(fill(
                Bounds::from_corners(
                    point(bounds.left() + x1, bounds.top() + y),
                    point(bounds.left() + x2, bounds.top() + y + line_height),
                ),
                color,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textarea_line_slices_preserve_trailing_empty_line() {
        let lines = textarea_line_slices("a\nb\n");

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "a");
        assert_eq!(lines[0].start..lines[0].end, 0..1);
        assert_eq!(lines[1].text, "b");
        assert_eq!(lines[2].text, "");
        assert_eq!(lines[2].start..lines[2].end, 4..4);
    }

    #[test]
    fn textarea_normalizes_crlf_to_lf() {
        assert_eq!(text_editing::normalize_multiline("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn textarea_metrics_rows_raise_min_height() {
        let three = TextareaMetrics::from_size_and_rows(Size::Medium, 3);
        let six = TextareaMetrics::from_size_and_rows(Size::Medium, 6);

        assert_eq!(three.rows(), 3);
        assert!(six.min_height() > three.min_height());
    }
}
