//! Text input component.

use crate::geometry::gpui_px_from_ui;
use std::{borrow::Cow, fmt, ops::Range, rc::Rc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PaintQuad, ParentElement, Pixels, Point, RenderOnce, ShapedLine, SharedString, Style, Styled,
    TextRun, UTF16Selection, Window, actions, div, fill, point, px, relative, rgba,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx,
};

use crate::a11y::{
    AccessibleTextInputHandler, AccessibleTextReplacementTarget, AccessibleTextRunRange,
    TextControlSemanticProjection, UiA11yElementExt, dispatch_accessible_text_replacement,
    dispatch_accessible_text_selection, project_accessible_text_selection_in_runs,
};
use crate::color::ColorIntent;
use crate::field::adapter::{FieldControl, FieldControlSemantics};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::form_control::FormControlState;
use crate::text_editing::{
    self, EditableTextDocument, TextDisplayPolicy, TextDisplayProjection, TextEditingPolicy,
    TextEditingProjection, TextSelection,
};
use crate::theme::ThemeResolver;

type TextInputChangeHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// Controls how a text input value is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextInputDisplayMode {
    /// Render the stored value as-is.
    #[default]
    Plain,
    /// Render one mask glyph per stored grapheme while preserving the real value for editing.
    Password,
}

impl TextInputDisplayMode {
    /// Returns whether this mode masks the stored value.
    pub const fn masks_value(self) -> bool {
        matches!(self, Self::Password)
    }

    /// Returns a stable label for diagnostics and gallery metadata.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Password => "password",
        }
    }
}

impl From<TextInputDisplayMode> for TextDisplayPolicy {
    fn from(value: TextInputDisplayMode) -> Self {
        match value {
            TextInputDisplayMode::Plain => Self::Plain,
            TextInputDisplayMode::Password => Self::Masked,
        }
    }
}

/// Resolved text input color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextInputColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) placeholder: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl TextInputColors {
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

/// Resolved text input metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextInputMetrics {
    height: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl TextInputMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            height: size.input_h(),
            padding_x: size.input_px(),
            padding_y: size.input_py(),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the input height.
    pub const fn height(self) -> UiPx {
        self.height
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
}

pub(crate) mod adapter {
    use super::*;

    actions!(
        text_input,
        [
            /// Delete the previous grapheme.
            Backspace,
            /// Delete the next grapheme.
            Delete,
            /// Move the caret one grapheme left.
            Left,
            /// Move the caret one grapheme right.
            Right,
            /// Extend selection one grapheme left.
            SelectLeft,
            /// Extend selection one grapheme right.
            SelectRight,
            /// Select the entire input value.
            SelectAll,
            /// Move the caret to the start.
            Home,
            /// Move the caret to the end.
            End,
            /// Paste text from the clipboard.
            Paste,
            /// Copy selected text to the clipboard.
            Copy,
            /// Cut selected text to the clipboard.
            Cut,
            /// Open the platform character palette.
            ShowCharacterPalette,
        ]
    );

    pub(super) const TEXT_INPUT_KEY_CONTEXT: &str = "TextInput";

    /// Registers default key bindings for editable text inputs.
    pub fn init(cx: &mut App) {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("shift-backspace", Backspace, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("delete", Delete, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("shift-delete", Delete, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("left", Left, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("right", Right, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("shift-left", SelectLeft, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("shift-right", SelectRight, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("home", Home, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("end", End, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-a", SelectAll, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-a", SelectAll, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-c", Copy, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-c", Copy, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-x", Cut, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-x", Cut, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-v", Paste, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-v", Paste, Some(TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new(
                "ctrl-cmd-space",
                ShowCharacterPalette,
                Some(TEXT_INPUT_KEY_CONTEXT),
            ),
        ]);
    }

    /// Editable single-line text input controller.
    ///
    /// The controller owns editing state and implements GPUI's platform text input handler. It is kept
    /// separate from `TextInputState` so resolved component state remains renderer-neutral.
    ///
    /// This is a GPUI adapter-only API. A future headless text model should not depend on
    /// `EntityInputHandler`, `FocusHandle`, shaped lines, or GPUI window/layout types exposed here.
    pub struct TextInputController {
        focus_handle: FocusHandle,
        content: SharedString,
        placeholder: SharedString,
        selected_range: Range<usize>,
        selection_reversed: bool,
        marked_range: Option<Range<usize>>,
        pub(super) last_layout: Option<ShapedLine>,
        pub(super) last_bounds: Option<Bounds<Pixels>>,
        display_mode: TextInputDisplayMode,
        is_selecting: bool,
        disabled: bool,
        read_only: bool,
        user_edit_revision: u64,
        on_change: Option<TextInputChangeHandler>,
    }

    impl fmt::Debug for TextInputController {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TextInputController")
                .field("focus_handle", &self.focus_handle)
                .field("content", &self.content)
                .field("placeholder", &self.placeholder)
                .field("selected_range", &self.selected_range)
                .field("selection_reversed", &self.selection_reversed)
                .field("marked_range", &self.marked_range)
                .field("last_layout", &self.last_layout)
                .field("last_bounds", &self.last_bounds)
                .field("display_mode", &self.display_mode)
                .field("is_selecting", &self.is_selecting)
                .field("disabled", &self.disabled)
                .field("read_only", &self.read_only)
                .field("on_change", &self.on_change.as_ref().map(|_| "<handler>"))
                .finish()
        }
    }

    impl TextInputController {
        /// Creates a new editable text input controller.
        pub fn new(cx: &mut Context<Self>) -> Self {
            Self {
                focus_handle: cx.focus_handle(),
                content: SharedString::default(),
                placeholder: SharedString::default(),
                selected_range: 0..0,
                selection_reversed: false,
                marked_range: None,
                last_layout: None,
                last_bounds: None,
                display_mode: TextInputDisplayMode::default(),
                is_selecting: false,
                disabled: false,
                read_only: false,
                user_edit_revision: 0,
                on_change: None,
            }
        }

        /// Creates a new controller with an initial value.
        pub fn with_value(value: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
            let mut this = Self::new(cx);
            let value = value.into();
            let projection =
                EditableTextDocument::new(value.as_ref(), TextEditingPolicy::single_line())
                    .into_projection();
            this.apply_editing_projection(projection);
            this
        }

        /// Returns the current input value.
        pub fn value(&self) -> &str {
            self.content.as_ref()
        }

        pub(crate) const fn user_edit_revision(&self) -> u64 {
            self.user_edit_revision
        }

        /// Returns the current display mode.
        pub const fn display_mode(&self) -> TextInputDisplayMode {
            self.display_mode
        }

        /// Sets the display mode used by hit testing and shaped display text.
        pub fn set_display_mode(
            &mut self,
            display_mode: TextInputDisplayMode,
            cx: &mut Context<Self>,
        ) {
            if self.display_mode != display_mode {
                self.display_mode = display_mode;
                cx.notify();
            }
        }

        /// Returns the placeholder value used by editable rendering.
        pub fn placeholder(&self) -> &str {
            self.placeholder.as_ref()
        }

        /// Sets the placeholder value.
        pub fn set_placeholder(
            &mut self,
            placeholder: impl Into<SharedString>,
            cx: &mut Context<Self>,
        ) {
            self.placeholder = placeholder.into();
            cx.notify();
        }

        /// Sets the full input value and moves the caret to the end.
        pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
            let value = value.into();
            let projection =
                EditableTextDocument::new(value.as_ref(), TextEditingPolicy::single_line())
                    .into_projection();
            self.apply_editing_projection(projection);
            cx.notify();
        }

        /// Returns whether the controller is disabled.
        pub const fn disabled(&self) -> bool {
            self.disabled
        }

        /// Sets disabled state.
        pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
            if self.disabled != disabled {
                self.disabled = disabled;
                cx.notify();
            }
        }

        /// Returns whether the controller is read-only.
        pub const fn read_only(&self) -> bool {
            self.read_only
        }

        /// Sets read-only state.
        pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
            if self.read_only != read_only {
                self.read_only = read_only;
                cx.notify();
            }
        }

        /// Returns whether platform text input should mutate this controller.
        pub const fn accepts_editing(&self) -> bool {
            !self.disabled && !self.read_only
        }

        /// Returns the selected byte range.
        pub fn selected_range(&self) -> Range<usize> {
            self.selected_range.clone()
        }

        /// Returns the selected UTF-16 range.
        pub fn selected_range_utf16(&self) -> Range<usize> {
            self.range_to_utf16(&self.selected_range)
        }

        /// Returns the marked UTF-16 range.
        pub fn marked_range_utf16(&self) -> Option<Range<usize>> {
            self.marked_range
                .as_ref()
                .map(|range| self.range_to_utf16(range))
        }

        /// Replaces a UTF-16 range, the marked range, or the current selection.
        pub fn replace_text_in_range_utf16(
            &mut self,
            range_utf16: Option<Range<usize>>,
            text: &str,
            cx: &mut Context<Self>,
        ) {
            if self.accepts_editing() {
                self.replace_text_in_range_inner(range_utf16, text);
                cx.notify();
            }
        }

        /// Replaces and marks text using UTF-16 ranges.
        pub fn replace_and_mark_text_in_range_utf16(
            &mut self,
            range_utf16: Option<Range<usize>>,
            text: &str,
            selected_utf16: Option<Range<usize>>,
            cx: &mut Context<Self>,
        ) {
            if self.accepts_editing() {
                self.replace_and_mark_text_in_range_inner(range_utf16, text, selected_utf16);
                cx.notify();
            }
        }

        /// Moves the caret to a byte offset clamped to a user-visible grapheme boundary.
        pub fn move_to_offset(&mut self, offset: usize, cx: &mut Context<Self>) {
            self.move_to(offset, cx);
        }

        /// Selects a byte range clamped to user-visible grapheme boundaries.
        pub fn select_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
            let projection = EditableTextDocument::from_parts(
                self.value(),
                TextSelection::from_offsets(range.start, range.end),
                self.marked_range.clone(),
                self.editing_policy(),
            )
            .into_projection();
            self.apply_editing_projection(projection);
            cx.notify();
        }

        /// Returns text for a UTF-16 range and updates the adjusted range.
        pub fn text_for_range_utf16(
            &self,
            range_utf16: Range<usize>,
            adjusted_range: &mut Option<Range<usize>>,
        ) -> Option<String> {
            let range = self.range_from_utf16(&range_utf16);
            adjusted_range.replace(self.range_to_utf16(&range));
            self.content.get(range).map(ToString::to_string)
        }

        /// Deletes the previous grapheme or selected text.
        pub fn delete_backward(&mut self, cx: &mut Context<Self>) {
            if !self.accepts_editing() {
                return;
            }
            if let Some(projection) = self.document().delete_backward() {
                self.apply_editing_projection(projection);
                cx.notify();
            }
        }

        /// Deletes the next grapheme or selected text.
        pub fn delete_forward(&mut self, cx: &mut Context<Self>) {
            if !self.accepts_editing() {
                return;
            }
            if let Some(projection) = self.document().delete_forward() {
                self.apply_editing_projection(projection);
                cx.notify();
            }
        }

        pub(crate) fn sync_adapter_state(
            &mut self,
            controlled_value: Option<&str>,
            placeholder: Option<SharedString>,
            disabled: bool,
            read_only: bool,
            display_mode: TextInputDisplayMode,
            on_change: Option<TextInputChangeHandler>,
        ) {
            self.disabled = disabled;
            self.read_only = read_only;
            self.display_mode = display_mode;
            self.on_change = on_change;

            if let Some(placeholder) = placeholder {
                self.placeholder = placeholder;
            }

            if let Some(value) = controlled_value {
                let value = text_editing::sanitize_single_line(value);
                if self.content.as_ref() != value {
                    let projection = EditableTextDocument::from_parts(
                        value,
                        TextSelection::caret(self.cursor_offset()),
                        None,
                        self.editing_policy(),
                    )
                    .into_projection();
                    self.apply_editing_projection(projection);
                }
            }
        }

        pub(super) fn cursor_offset(&self) -> usize {
            if self.selection_reversed {
                self.selected_range.start
            } else {
                self.selected_range.end
            }
        }

        pub(super) const fn selection_reversed(&self) -> bool {
            self.selection_reversed
        }

        pub(super) fn accepts_accessible_selection(&self) -> bool {
            !self.disabled && self.display_mode != TextInputDisplayMode::Password
        }

        pub(super) fn set_accessible_selection_bytes(
            &mut self,
            anchor: usize,
            focus: usize,
            cx: &mut Context<Self>,
        ) {
            if !self.accepts_accessible_selection() {
                return;
            }
            let projection = self.document().set_accessible_selection(anchor, focus);
            self.apply_editing_projection(projection);
            cx.notify();
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
            TextEditingPolicy::single_line()
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

        fn previous_boundary(&self, offset: usize) -> usize {
            self.document().previous_grapheme_boundary(offset)
        }

        fn next_boundary(&self, offset: usize) -> usize {
            self.document().next_grapheme_boundary(offset)
        }

        fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
            if self.content.is_empty() {
                return 0;
            }

            let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
            else {
                return 0;
            };
            if position.y < bounds.top() {
                return 0;
            }
            if position.y > bounds.bottom() {
                return self.content.len();
            }
            let display_index = line.closest_index_for_x(position.x - bounds.left());
            TextDisplayProjection::for_policy(self.content.as_ref(), self.display_mode.into())
                .display_to_stored_offset(display_index)
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

        fn replace_text_in_range_inner(
            &mut self,
            range_utf16: Option<Range<usize>>,
            new_text: &str,
        ) {
            let projection = self.document().replace_text_in_range(range_utf16, new_text);
            self.apply_editing_projection(projection);
        }

        fn replace_and_mark_text_in_range_inner(
            &mut self,
            range_utf16: Option<Range<usize>>,
            new_text: &str,
            selected_utf16: Option<Range<usize>>,
        ) {
            let projection = self.document().replace_and_mark_text_in_range(
                range_utf16,
                new_text,
                selected_utf16,
            );
            self.apply_editing_projection(projection);
        }

        fn dispatch_change(&self, window: &mut Window, cx: &mut App) {
            if let Some(on_change) = self.on_change.as_ref().cloned() {
                on_change(self.content.to_string(), window, cx);
            }
        }

        pub(super) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
            if self.selected_range.is_empty() {
                self.move_to(self.previous_boundary(self.cursor_offset()), cx);
            } else {
                self.move_to(self.selected_range.start, cx);
            }
        }

        pub(super) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
            if self.selected_range.is_empty() {
                self.move_to(self.next_boundary(self.cursor_offset()), cx);
            } else {
                self.move_to(self.selected_range.end, cx);
            }
        }

        pub(super) fn select_left(
            &mut self,
            _: &SelectLeft,
            _: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }

        pub(super) fn select_right(
            &mut self,
            _: &SelectRight,
            _: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }

        pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
            self.selected_range = 0..self.content.len();
            self.selection_reversed = false;
            cx.notify();
        }

        pub(super) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
            self.move_to(0, cx);
        }

        pub(super) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
            self.move_to(self.content.len(), cx);
        }

        pub(super) fn backspace(
            &mut self,
            _: &Backspace,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            if !self.accepts_editing() {
                return;
            }
            if self.selected_range.is_empty() {
                let previous = self.previous_boundary(self.cursor_offset());
                if previous == self.cursor_offset() {
                    window.play_system_bell();
                    return;
                }
                self.selected_range = previous..self.cursor_offset();
            }
            self.replace_text_in_range(None, "", window, cx);
        }

        pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
            if !self.accepts_editing() {
                return;
            }
            if self.selected_range.is_empty() {
                let next = self.next_boundary(self.cursor_offset());
                if next == self.cursor_offset() {
                    window.play_system_bell();
                    return;
                }
                self.selected_range = self.cursor_offset()..next;
            }
            self.replace_text_in_range(None, "", window, cx);
        }

        pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
            if !self.accepts_editing() {
                return;
            }
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.replace_text_in_range(None, &text, window, cx);
            }
        }

        pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
            if !self.selected_range.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.content[self.selected_range.clone()].to_string(),
                ));
            }
        }

        pub(super) fn cut(&mut self, action: &Cut, window: &mut Window, cx: &mut Context<Self>) {
            if !self.accepts_editing() {
                return;
            }
            self.copy(&Copy, window, cx);
            if !self.selected_range.is_empty() {
                let _ = action;
                self.replace_text_in_range(None, "", window, cx);
            }
        }

        pub(super) fn show_character_palette(
            &mut self,
            _: &ShowCharacterPalette,
            window: &mut Window,
            _: &mut Context<Self>,
        ) {
            window.show_character_palette();
        }

        pub(super) fn on_mouse_down(
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

        pub(super) fn on_mouse_up(
            &mut self,
            _: &MouseUpEvent,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
            self.is_selecting = false;
        }

        pub(super) fn on_mouse_move(
            &mut self,
            event: &MouseMoveEvent,
            _: &mut Window,
            cx: &mut Context<Self>,
        ) {
            if self.is_selecting {
                self.select_to(self.index_for_mouse_position(event.position), cx);
            }
        }
    }

    impl Focusable for TextInputController {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl EntityInputHandler for TextInputController {
        fn text_for_range(
            &mut self,
            range_utf16: Range<usize>,
            adjusted_range: &mut Option<Range<usize>>,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> Option<String> {
            self.text_for_range_utf16(range_utf16, adjusted_range)
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
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            if !self.accepts_editing() {
                return;
            }
            let previous = self.content.clone();
            self.replace_text_in_range_inner(range_utf16, new_text);
            if previous != self.content {
                self.user_edit_revision = self.user_edit_revision.wrapping_add(1);
                self.dispatch_change(_window, cx);
            }
            cx.notify();
        }

        fn replace_and_mark_text_in_range(
            &mut self,
            range_utf16: Option<Range<usize>>,
            new_text: &str,
            selected_utf16: Option<Range<usize>>,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            if !self.accepts_editing() {
                return;
            }
            let previous = self.content.clone();
            self.replace_and_mark_text_in_range_inner(range_utf16, new_text, selected_utf16);
            if previous != self.content {
                self.user_edit_revision = self.user_edit_revision.wrapping_add(1);
                self.dispatch_change(_window, cx);
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
            let last_layout = self.last_layout.as_ref()?;
            let range = self.range_from_utf16(&range_utf16);
            let display_range =
                TextDisplayProjection::for_policy(self.content.as_ref(), self.display_mode.into())
                    .stored_range_to_display_range(&range);
            Some(Bounds::from_corners(
                point(
                    bounds.left() + last_layout.x_for_index(display_range.start),
                    bounds.top(),
                ),
                point(
                    bounds.left() + last_layout.x_for_index(display_range.end),
                    bounds.bottom(),
                ),
            ))
        }

        fn character_index_for_point(
            &mut self,
            point: Point<Pixels>,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> Option<usize> {
            let bounds = self.last_bounds?;
            let last_layout = self.last_layout.as_ref()?;
            if point.y < bounds.top() {
                return Some(0);
            }
            if point.y > bounds.bottom() {
                return Some(self.offset_to_utf16(self.content.len()));
            }
            let display_index = last_layout.index_for_x(point.x - bounds.left())?;
            let stored_index =
                TextDisplayProjection::for_policy(self.content.as_ref(), self.display_mode.into())
                    .display_to_stored_offset(display_index);
            Some(self.offset_to_utf16(stored_index))
        }

        fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
            self.accepts_editing()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[open_gpui::test]
        fn controlled_sync_repairs_selection_to_a_grapheme_boundary(
            cx: &mut open_gpui::TestAppContext,
        ) {
            let controller = cx.new(|cx| TextInputController::with_value("ab", cx));

            cx.update_entity(&controller, |controller, cx| {
                controller.move_to_offset(1, cx);
                controller.sync_adapter_state(
                    Some("e\u{301}"),
                    None,
                    false,
                    false,
                    TextInputDisplayMode::Plain,
                    None,
                );

                assert_eq!(controller.selected_range(), 0..0);
                let text_run = crate::a11y::AccessibleTextRunRange::from_text(
                    open_gpui::accesskit::NodeId(7),
                    0..controller.value().len(),
                    controller.value(),
                )
                .expect("repaired value should expose text-run metadata");
                let selection = crate::a11y::project_accessible_text_selection_in_runs(
                    controller,
                    std::slice::from_ref(&text_run),
                )
                .expect("repaired selection should remain accessible");
                assert_eq!(selection.anchor().character_index(), 0);
                assert_eq!(selection.focus().character_index(), 0);

                controller.delete_backward(cx);
                assert_eq!(controller.value(), "e\u{301}");
                controller.delete_forward(cx);
                assert_eq!(controller.value(), "");
            });
        }

        #[open_gpui::test]
        fn accessible_selection_rejects_a_stale_published_value(
            cx: &mut open_gpui::TestAppContext,
        ) {
            let controller = cx.new(|cx| TextInputController::with_value("ab", cx));
            cx.update_entity(&controller, |controller, cx| {
                controller.set_value("xy", cx);
                assert_eq!(controller.selected_range(), 2..2);
            });

            let text_run_id = open_gpui::accesskit::NodeId(7);
            let data = open_gpui::accesskit::ActionData::SetTextSelection(
                open_gpui::accesskit::TextSelection {
                    anchor: open_gpui::accesskit::TextPosition {
                        node: text_run_id,
                        character_index: 0,
                    },
                    focus: open_gpui::accesskit::TextPosition {
                        node: text_run_id,
                        character_index: 1,
                    },
                },
            );
            cx.update(|cx| {
                dispatch_accessible_text_selection(&controller, "ab", Some(&data), text_run_id, cx);
            });

            cx.update_entity(&controller, |controller, _| {
                assert_eq!(controller.value(), "xy");
                assert_eq!(controller.selected_range(), 2..2);
            });
        }
    }
}

use adapter::{
    Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
    SelectRight, ShowCharacterPalette, TEXT_INPUT_KEY_CONTEXT, TextInputController,
};

impl AccessibleTextInputHandler for TextInputController {
    fn value(&self) -> &str {
        TextInputController::value(self)
    }

    fn selected_range_bytes(&self) -> Range<usize> {
        self.selected_range()
    }

    fn selection_reversed(&self) -> bool {
        TextInputController::selection_reversed(self)
    }

    fn accepts_accessible_selection(&self) -> bool {
        TextInputController::accepts_accessible_selection(self)
    }

    fn set_accessible_selection(&mut self, anchor: usize, focus: usize, cx: &mut Context<Self>) {
        self.set_accessible_selection_bytes(anchor, focus, cx);
    }
}

/// Resolved text input state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputState {
    value: String,
    placeholder: Option<String>,
    control: FormControlState,
    display_mode: TextInputDisplayMode,
    metrics: TextInputMetrics,
    colors: TextInputColors,
    focus_ring: FocusRing,
}

impl TextInputState {
    /// Resolves the public state for a text input.
    pub fn resolve(
        value: impl Into<String>,
        placeholder: Option<impl Into<String>>,
        size: Size,
        disabled: bool,
        read_only: bool,
        invalid: bool,
        required: bool,
        controller_driven: bool,
        tokens: ThemeTokens,
    ) -> Self {
        Self::resolve_with_display_mode(
            value,
            placeholder,
            size,
            disabled,
            read_only,
            invalid,
            required,
            controller_driven,
            TextInputDisplayMode::default(),
            tokens,
        )
    }

    /// Resolves the public state for a text input with an explicit display mode.
    pub fn resolve_with_display_mode(
        value: impl Into<String>,
        placeholder: Option<impl Into<String>>,
        size: Size,
        disabled: bool,
        read_only: bool,
        invalid: bool,
        required: bool,
        controller_driven: bool,
        display_mode: TextInputDisplayMode,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::text_input_colors(tokens, disabled, read_only, invalid);
        let value = value.into();
        let control = FormControlState::resolve(
            size,
            disabled,
            read_only,
            invalid,
            required,
            controller_driven,
        );

        Self {
            value: TextEditingPolicy::single_line().normalize_text(value.as_str()),
            placeholder: placeholder.map(Into::into),
            control,
            display_mode,
            metrics: TextInputMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns this state with asynchronous activity updated.
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.control = self.control.with_busy(busy);
        self
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

    /// Returns whether the input has a non-empty value.
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
    pub fn display_text(&self) -> Cow<'_, str> {
        if self.placeholder_visible() {
            Cow::Borrowed(self.placeholder().unwrap_or(""))
        } else {
            self.accessible_value()
        }
    }

    fn accessible_value(&self) -> Cow<'_, str> {
        TextDisplayProjection::for_policy(self.value(), self.display_mode.into())
            .into_display_text()
    }

    /// Derives an owned, renderer-neutral semantic projection from this resolved state.
    pub fn semantic_projection<NodeId>(&self) -> TextControlSemanticProjection<NodeId> {
        self.semantic_projection_for_value(self.value(), self.placeholder())
    }

    pub(crate) fn semantic_projection_for_value<NodeId>(
        &self,
        value: &str,
        placeholder: Option<&str>,
    ) -> TextControlSemanticProjection<NodeId> {
        let semantic_value =
            TextDisplayProjection::for_policy(value, self.display_mode.into()).into_display_text();
        let exposes_text_runs = !self.display_mode.masks_value()
            && text_editing::supports_accessible_character_lengths(semantic_value.as_ref());
        TextControlSemanticProjection::new(
            self.role(),
            semantic_value.into_owned(),
            placeholder,
            self.control,
            exposes_text_runs,
        )
    }

    /// Returns the display mode.
    pub const fn display_mode(&self) -> TextInputDisplayMode {
        self.display_mode
    }

    /// Returns the shared form-control state.
    pub const fn control_state(&self) -> FormControlState {
        self.control
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.control.size()
    }

    /// Returns whether the input is disabled.
    pub const fn disabled(&self) -> bool {
        self.control.disabled()
    }

    /// Returns whether the input is read-only.
    pub const fn read_only(&self) -> bool {
        self.control.read_only()
    }

    /// Returns whether the input is invalid.
    pub const fn invalid(&self) -> bool {
        self.control.invalid()
    }

    /// Returns whether asynchronous work is pending for this input.
    pub const fn busy(&self) -> bool {
        self.control.busy()
    }

    /// Returns whether the input is required.
    pub const fn required(&self) -> bool {
        self.control.required()
    }

    /// Returns whether this state is backed by an editable controller.
    pub const fn controller_driven(&self) -> bool {
        self.control.controller_driven()
    }

    /// Returns whether text editing should be accepted.
    pub const fn input_enabled(&self) -> bool {
        self.control.input_enabled()
    }

    /// Returns whether text editing should be accepted.
    pub const fn editable(&self) -> bool {
        self.control.editable()
    }

    /// Returns whether activation/edit handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        self.control.activation_enabled()
    }

    /// Returns whether the element should be included in tab traversal.
    pub const fn tab_stop_enabled(&self) -> bool {
        self.control.tab_stop_enabled()
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        match self.display_mode {
            TextInputDisplayMode::Plain => Role::TextInput,
            TextInputDisplayMode::Password => Role::PasswordInput,
        }
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TextInputMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> TextInputColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI text input component shell.
#[derive(IntoElement)]
pub struct TextInput {
    id: ElementId,
    label: SharedString,
    value: SharedString,
    placeholder: Option<SharedString>,
    controller: Option<Entity<TextInputController>>,
    control: FormControlState,
    display_mode: TextInputDisplayMode,
    tokens: ThemeTokens,
    on_change: Option<TextInputChangeHandler>,
    field_semantics: Option<FieldControlSemantics>,
}

impl TextInput {
    /// Creates a new text input with an id and accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: SharedString::default(),
            placeholder: None,
            controller: None,
            control: FormControlState::default(),
            display_mode: TextInputDisplayMode::default(),
            tokens: ThemeTokens::default(),
            on_change: None,
            field_semantics: None,
        }
    }

    /// Binds this input to an editable GPUI controller entity.
    ///
    /// The controller is adapter-owned and intentionally stays out of [`TextInputState`].
    pub fn controller(mut self, controller: Entity<TextInputController>) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Sets the displayed value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        let value = value.into();
        self.value = TextEditingPolicy::single_line()
            .normalize_text(value.as_ref())
            .into();
        self
    }

    /// Registers a controlled value-change handler.
    ///
    /// When set, the input creates an adapter-owned controller, accepts text editing, and calls the
    /// handler with the next sanitized single-line value. Callers should feed that value back through
    /// [`TextInput::value`] on the next render.
    pub fn on_change(mut self, handler: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Sets the placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets how the input value should be displayed.
    pub fn display_mode(mut self, display_mode: TextInputDisplayMode) -> Self {
        self.display_mode = display_mode;
        self
    }

    /// Marks the input as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.control = self.control.with_disabled(disabled);
        self
    }

    /// Marks the input as read-only.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.control = self.control.with_read_only(read_only);
        self
    }

    /// Marks the input as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.control = self.control.with_invalid(invalid);
        self
    }

    /// Marks the input as required.
    pub fn required(mut self, required: bool) -> Self {
        self.control = self.control.with_required(required);
        self
    }

    /// Marks the input as having pending asynchronous work.
    pub fn busy(mut self, busy: bool) -> Self {
        self.control = self.control.with_busy(busy);
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved text input state.
    pub fn state(&self) -> TextInputState {
        TextInputState::resolve_with_display_mode(
            self.value.to_string(),
            self.placeholder.as_ref().map(ToString::to_string),
            self.control.size(),
            self.control.disabled(),
            self.control.read_only(),
            self.control.invalid(),
            self.control.required(),
            self.controller.is_some() || self.on_change.is_some(),
            self.display_mode,
            self.tokens,
        )
        .with_busy(self.control.busy())
    }
}

impl Sizable for TextInput {
    fn with_size(mut self, size: Size) -> Self {
        self.control = self.control.with_size(size);
        self
    }
}

impl FieldControl for TextInput {
    fn field_control_state(&self) -> FormControlState {
        self.control
    }

    fn with_field_semantics(mut self, semantics: FieldControlSemantics) -> Self {
        self.control = semantics.apply_control_state(self.control);
        self.field_semantics = Some(semantics);
        self
    }
}

fn resolve_text_input_character_lengths_when_active(
    accessibility_active: bool,
    exposes_text_runs: bool,
    resolve: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    (accessibility_active && exposes_text_runs)
        .then(resolve)
        .flatten()
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let debug_id = self.id.to_string();
        let runtime_id = format!("text-input:{debug_id}:controller");
        let text_run_id: ElementId = (self.id.clone(), "text-run").into();
        let text_run_node_id = window.with_id(self.id.clone(), |window| {
            window.with_global_id(text_run_id.clone(), |global_id, _| {
                global_id.accesskit_node_id()
            })
        });
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let theme = ThemeResolver::current(window, cx);
        let controller_is_external = self.controller.is_some();
        let controller = self.controller.clone().or_else(|| {
            self.on_change.as_ref().map(|_| {
                let initial_value = self.value.clone();
                window.use_keyed_state(runtime_id, cx, |_, cx| {
                    TextInputController::with_value(initial_value, cx)
                })
            })
        });
        let builder_placeholder = self.placeholder.clone().unwrap_or_default();
        if let Some(controller) = controller.as_ref() {
            let controlled_value = self.on_change.as_ref().map(|_| self.value.as_ref());
            let placeholder = if controller_is_external {
                self.placeholder.clone()
            } else {
                Some(builder_placeholder.clone())
            };
            let on_change = self.on_change.clone();
            controller.update(cx, |controller, _cx| {
                controller.sync_adapter_state(
                    controlled_value,
                    placeholder,
                    state.disabled(),
                    state.read_only(),
                    state.display_mode(),
                    on_change,
                );
            });
        }
        let controller_text = controller.as_ref().map(|controller| controller.read(cx));
        let semantic_value = controller_text
            .as_ref()
            .map(|controller| controller.value())
            .unwrap_or_else(|| state.value());
        let placeholder = controller_text
            .as_ref()
            .map(|controller| controller.placeholder().to_owned().into())
            .filter(|placeholder: &SharedString| !placeholder.is_empty())
            .unwrap_or(builder_placeholder.clone());
        let semantic_projection = state
            .semantic_projection_for_value::<open_gpui::accesskit::NodeId>(
                semantic_value,
                (!placeholder.is_empty()).then_some(placeholder.as_ref()),
            );
        let character_lengths = resolve_text_input_character_lengths_when_active(
            window.is_accessibility_active(),
            semantic_projection.exposes_text_runs(),
            || text_editing::accessible_character_lengths(semantic_projection.value()),
        )
        .map(Rc::<[u8]>::from);
        let text_run_range = character_lengths.as_ref().and_then(|character_lengths| {
            AccessibleTextRunRange::from_character_lengths(
                text_run_node_id,
                0..semantic_projection.value().len(),
                character_lengths.clone(),
            )
        });
        let text_selection = text_run_range.as_ref().and_then(|text_run_range| {
            controller_text.as_ref().and_then(|controller| {
                project_accessible_text_selection_in_runs(
                    &**controller,
                    std::slice::from_ref(text_run_range),
                )
            })
        });
        let semantic_projection = semantic_projection.with_text_selection(text_selection);
        let published_semantic_value = semantic_projection.value().to_owned();
        let semantics = FieldControlSemantics::project_text_control_descriptor(
            self.field_semantics.as_ref(),
            self.label.as_ref(),
            semantic_projection.descriptor(),
        );
        let show_placeholder = controller_text
            .as_ref()
            .map(|controller| controller.value().is_empty() && !placeholder.is_empty())
            .unwrap_or_else(|| state.placeholder_visible());
        let static_display_text = if show_placeholder {
            placeholder.clone()
        } else {
            TextDisplayProjection::for_policy(self.value.as_ref(), state.display_mode().into())
                .into_display_text()
                .into_owned()
                .into()
        };
        let text_color_intent = if show_placeholder {
            colors.placeholder()
        } else {
            colors.foreground()
        };
        let text_color = theme.resolve(text_color_intent);
        let border_color = theme.resolve(colors.border());
        let background = theme.resolve(colors.background());
        let placeholder_color = theme.resolve(colors.placeholder());
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let text_run_semantics = character_lengths.as_ref().map(|character_lengths| {
            SemanticDescriptor::new(Role::TextRun)
                .with_value(semantic_projection.value())
                .with_character_lengths(character_lengths)
        });
        let text_content = div()
            .id(text_run_id)
            .min_w(px(0.0))
            .w_full()
            .truncate()
            .children(
                controller
                    .clone()
                    .into_iter()
                    .map(|controller| EditableTextElement {
                        controller,
                        placeholder: placeholder.clone(),
                        text_color: text_color.into(),
                        placeholder_color: placeholder_color.into(),
                        selection_color: rgba(0x2f80ed33).into(),
                        caret_color: text_color.into(),
                        text_size: gpui_px_from_ui(metrics.text_size()).into(),
                        display_mode: state.display_mode(),
                    }),
            )
            .when(!state.controller_driven(), |this| {
                this.child(static_display_text)
            })
            .when_some(text_run_semantics.as_ref(), |this, semantics| {
                this.ui_semantics(semantics)
            });
        div()
            .id(self.id)
            .debug_selector(move || format!("text-input:{debug_id}:root"))
            .min_h(gpui_px_from_ui(metrics.height()))
            .w_full()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(border_color)
            .bg(background)
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .text_color(text_color)
            .focusable()
            .tab_stop(state.tab_stop_enabled())
            .ui_semantics_with_relations(&semantics, |node_id| *node_id)
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
                let backspace = controller.clone();
                let delete = controller.clone();
                let left = controller.clone();
                let right = controller.clone();
                let select_left = controller.clone();
                let select_right = controller.clone();
                let select_all = controller.clone();
                let home = controller.clone();
                let end = controller.clone();
                let paste = controller.clone();
                let copy = controller.clone();
                let cut = controller.clone();
                let character_palette = controller.clone();
                let mouse_down = controller.clone();
                let mouse_up = controller.clone();
                let mouse_up_out = controller.clone();
                let mouse_move = controller.clone();
                let replace_selected_text = controller.clone();
                let set_text_selection = controller.clone();
                let set_text_selection_published_value = published_semantic_value;
                let set_value = controller.clone();

                this.key_context(TEXT_INPUT_KEY_CONTEXT)
                    .track_focus(&focus)
                    .on_ui_a11y_action(
                        AccessibleAction::ReplaceSelectedText,
                        move |data, window, cx| {
                            dispatch_accessible_text_replacement(
                                &replace_selected_text,
                                data,
                                AccessibleTextReplacementTarget::SelectedText,
                                window,
                                cx,
                            );
                        },
                    )
                    .on_ui_a11y_action(
                        AccessibleAction::SetTextSelection,
                        move |data, _window, cx| {
                            dispatch_accessible_text_selection(
                                &set_text_selection,
                                set_text_selection_published_value.as_str(),
                                data,
                                text_run_node_id,
                                cx,
                            );
                        },
                    )
                    .on_ui_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                        dispatch_accessible_text_replacement(
                            &set_value,
                            data,
                            AccessibleTextReplacementTarget::EntireValue,
                            window,
                            cx,
                        );
                    })
                    .on_action(move |action: &Backspace, window, cx| {
                        backspace.update(cx, |controller, cx| {
                            controller.backspace(action, window, cx);
                        });
                    })
                    .on_action(move |action: &Delete, window, cx| {
                        delete.update(cx, |controller, cx| {
                            controller.delete(action, window, cx);
                        });
                    })
                    .on_action(move |action: &Left, window, cx| {
                        left.update(cx, |controller, cx| controller.left(action, window, cx));
                    })
                    .on_action(move |action: &Right, window, cx| {
                        right.update(cx, |controller, cx| controller.right(action, window, cx));
                    })
                    .on_action(move |action: &SelectLeft, window, cx| {
                        select_left.update(cx, |controller, cx| {
                            controller.select_left(action, window, cx);
                        });
                    })
                    .on_action(move |action: &SelectRight, window, cx| {
                        select_right.update(cx, |controller, cx| {
                            controller.select_right(action, window, cx);
                        });
                    })
                    .on_action(move |action: &SelectAll, window, cx| {
                        select_all.update(cx, |controller, cx| {
                            controller.select_all(action, window, cx);
                        });
                    })
                    .on_action(move |action: &Home, window, cx| {
                        home.update(cx, |controller, cx| controller.home(action, window, cx));
                    })
                    .on_action(move |action: &End, window, cx| {
                        end.update(cx, |controller, cx| controller.end(action, window, cx));
                    })
                    .on_action(move |action: &Paste, window, cx| {
                        paste.update(cx, |controller, cx| controller.paste(action, window, cx));
                    })
                    .on_action(move |action: &Copy, window, cx| {
                        copy.update(cx, |controller, cx| controller.copy(action, window, cx));
                    })
                    .on_action(move |action: &Cut, window, cx| {
                        cut.update(cx, |controller, cx| controller.cut(action, window, cx));
                    })
                    .on_action(move |action: &ShowCharacterPalette, window, cx| {
                        character_palette.update(cx, |controller, cx| {
                            controller.show_character_palette(action, window, cx);
                        });
                    })
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
            .child(text_content)
    }
}

struct EditableTextElement {
    controller: Entity<TextInputController>,
    placeholder: SharedString,
    text_color: Hsla,
    placeholder_color: Hsla,
    selection_color: Hsla,
    caret_color: Hsla,
    text_size: Pixels,
    display_mode: TextInputDisplayMode,
}

struct EditableTextPrepaint {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for EditableTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditableTextElement {
    type RequestLayoutState = ();
    type PrepaintState = EditableTextPrepaint;

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
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
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
        let (display_text, text_color, display_cursor, display_selection): (
            SharedString,
            Hsla,
            usize,
            Range<usize>,
        ) = if content.is_empty() {
            (self.placeholder.clone(), self.placeholder_color, 0, 0..0)
        } else {
            let projection = TextDisplayProjection::for_policy(content, self.display_mode.into());
            (
                projection.display_text().into_owned().into(),
                self.text_color,
                projection.stored_to_display_offset(cursor),
                projection.stored_range_to_display_range(&selected_range),
            )
        };
        let font = window.text_style().font();
        let line = window.text_system().shape_line(
            display_text.clone(),
            self.text_size,
            &[TextRun {
                len: display_text.len(),
                font,
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );

        let cursor = selected_range.is_empty().then(|| {
            let x = line.x_for_index(display_cursor);
            fill(
                Bounds::from_corners(
                    point(bounds.left() + x, bounds.top()),
                    point(bounds.left() + x + px(1.0), bounds.bottom()),
                ),
                self.caret_color,
            )
        });
        let selection = (!selected_range.is_empty()).then(|| {
            fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(display_selection.start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(display_selection.end),
                        bounds.bottom(),
                    ),
                ),
                self.selection_color,
            )
        });

        let _ = controller;

        EditableTextPrepaint {
            line: Some(line),
            cursor,
            selection,
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

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }

        let line = prepaint.line.take().unwrap_or_default();
        let _ = line.paint(
            bounds.origin,
            window.line_height(),
            open_gpui::TextAlign::Left,
            None,
            window,
            cx,
        );

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.controller.update(cx, |controller, _cx| {
            controller.last_layout = Some(line);
            controller.last_bounds = Some(bounds);
        });
    }
}

#[cfg(test)]
mod render_tests {
    use std::cell::Cell;

    use super::resolve_text_input_character_lengths_when_active;

    #[test]
    fn inactive_accessibility_skips_text_run_metadata_derivation() {
        let calls = Cell::new(0);
        let resolve = || {
            calls.set(calls.get() + 1);
            Some(vec![1_u8])
        };

        assert_eq!(
            resolve_text_input_character_lengths_when_active(false, true, resolve),
            None
        );
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn active_accessibility_derives_only_exposed_text_runs() {
        let calls = Cell::new(0);
        let hidden = resolve_text_input_character_lengths_when_active(true, false, || {
            calls.set(calls.get() + 1);
            Some(vec![1_u8])
        });
        let exposed = resolve_text_input_character_lengths_when_active(true, true, || {
            calls.set(calls.get() + 1);
            Some(vec![1_u8])
        });

        assert_eq!(hidden, None);
        assert_eq!(exposed, Some(vec![1_u8]));
        assert_eq!(calls.get(), 1);
    }
}
