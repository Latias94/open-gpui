use std::ops::Range;

use open_gpui::{
    AccessibleAction, App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Global,
    GlobalElementId, InspectorElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, Role, ShapedLine, SharedString, Style, StyleRefinement,
    Subscription, TargetedEvent, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window,
    WindowId, accesskit::ActionData, div, fill, point, prelude::*, px, relative,
};

mod buffer;

use crate::pointer::{PointerDrag, on_pointer_cancel};
use buffer::TextBuffer;

const KEY_CONTEXT: &str = "OpenGpuiBaseTextInput";

/// User edits and editing intent from a [`TextInput`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    /// The committed text changed. An active IME preedit is excluded.
    Change(SharedString),
    /// Enter was pressed outside an IME composition.
    Submit(SharedString),
    /// Escape was pressed outside an IME composition.
    Cancel,
    /// The input lost focus.
    Blur,
}

/// A single-line text editor using GPUI's native text-input session.
pub struct TextInput {
    focus: FocusHandle,
    blur: Option<(WindowId, Subscription)>,
    buffer: TextBuffer,
    placeholder: SharedString,
    label: Option<SharedString>,
    invalid: bool,
    style: StyleRefinement,
    geometry: Option<TextGeometry>,
    drag: PointerDrag<()>,
}

impl TextInput {
    /// Creates a text input inside `cx.new` and installs its scoped key bindings once per app.
    pub fn new(value: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        install_keys(cx);
        Self {
            focus: cx.focus_handle(),
            blur: None,
            buffer: TextBuffer::new(value.into()),
            placeholder: "".into(),
            label: None,
            invalid: false,
            style: StyleRefinement::default(),
            geometry: None,
            drag: PointerDrag::default(),
        }
    }

    /// The committed value, excluding preedit text while an IME is composing.
    pub fn value(&self) -> &str {
        self.buffer.value()
    }

    /// Synchronizes an external value without emitting an editing event.
    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.sync_value(value.into(), cx);
    }

    /// Synchronizes an external value and notifies native text services in the given window.
    pub fn set_value_in(
        &mut self,
        value: impl Into<SharedString>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.sync_value(value.into(), cx) {
            window.notify_text_input_changed(&self.focus);
        }
    }

    fn sync_value(&mut self, value: SharedString, cx: &mut Context<Self>) -> bool {
        let value = buffer::normalize(value);
        if &value == self.buffer.value() {
            return false;
        }
        self.buffer = TextBuffer::new(value);
        self.geometry = None;
        cx.notify();
        true
    }

    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = buffer::normalize(text.into());
        cx.notify();
    }

    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.label = Some(label.into());
        cx.notify();
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        if self.invalid != invalid {
            self.invalid = invalid;
            cx.notify();
        }
    }

    /// Updates the GPUI style of the outer control.
    pub fn set_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    pub fn select_all(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.buffer.select(0, self.buffer.text().len());
        self.selection_changed(window, cx);
    }

    fn selection_changed(&self, window: &Window, cx: &mut Context<Self>) {
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn edited(&self, changed: bool, window: &Window, cx: &mut Context<Self>) {
        if changed {
            window.notify_text_input_changed(&self.focus);
            cx.emit(TextInputEvent::Change(self.buffer.value().clone()));
        } else {
            window.notify_text_selection_changed(&self.focus);
        }
        cx.notify();
    }

    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        value: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.buffer.replace(range, value);
        self.edited(changed, window, cx);
    }

    fn mouse_down(
        &mut self,
        event: &TargetedEvent<MouseDownEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        window.request_virtual_keyboard();
        cx.stop_propagation();
        let Ok(position) = event.target_layout_position() else {
            return;
        };
        let offset = if self.buffer.marked().is_some() {
            if !window.try_cancel_text_composition(&self.focus) {
                return;
            }
            let hit = self
                .current_geometry()
                .map(|geometry| geometry.index_at(position));
            self.buffer.cancel_composition_for_caret(hit)
        } else {
            self.index_at(position)
        };
        let event = event.window_event();
        if event.click_count < 2 || event.modifiers.shift {
            self.drag.begin((), window);
        } else {
            self.end_selection(window, cx);
        }
        if event.modifiers.shift {
            self.buffer.move_to(offset, true);
        } else if event.click_count >= 3 {
            self.select_all(window, cx);
            return;
        } else if event.click_count == 2 {
            self.buffer.select_word(offset);
        } else {
            self.buffer.move_to(offset, false);
        }
        self.selection_changed(window, cx);
    }

    fn mouse_move(
        &mut self,
        event: &TargetedEvent<MouseMoveEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.state.is_none() {
            return;
        }
        if !event.window_event().dragging() {
            self.end_selection(window, cx);
        } else if let Ok(position) = event.target_layout_position() {
            self.buffer.move_to(self.index_at(position), true);
            self.selection_changed(window, cx);
        }
    }

    fn index_at(&self, position: Point<Pixels>) -> usize {
        self.current_geometry()
            .map_or(self.buffer.head(), |geometry| {
                self.buffer.grapheme_boundary(geometry.index_at(position))
            })
    }

    fn end_selection(&mut self, window: &mut Window, _: &mut Context<Self>) {
        let _ = self.drag.take(window);
    }

    fn current_geometry(&self) -> Option<&TextGeometry> {
        self.geometry
            .as_ref()
            .filter(|geometry| geometry.text == *self.buffer.text())
    }

    fn on_command(&mut self, command: &TextCommand, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.marked().is_some() {
            return;
        }
        match *command {
            TextCommand::Move { forward, extend } => {
                let selection = self.buffer.selection();
                let offset = if extend || selection.is_empty() {
                    self.buffer.adjacent_grapheme(forward)
                } else if forward {
                    selection.end
                } else {
                    selection.start
                };
                self.buffer.move_to(offset, extend);
                self.selection_changed(window, cx);
            }
            TextCommand::Edge { end, extend } => {
                self.buffer
                    .move_to(if end { self.buffer.text().len() } else { 0 }, extend);
                self.selection_changed(window, cx);
            }
            TextCommand::Delete { forward } => {
                let mut range = self.buffer.selection();
                if range.is_empty() {
                    let adjacent = self.buffer.adjacent_grapheme(forward);
                    range = adjacent.min(range.start)..adjacent.max(range.end);
                }
                self.replace(Some(self.buffer.utf16_range(range)), "", window, cx);
            }
            TextCommand::SelectAll => self.select_all(window, cx),
            TextCommand::Copy | TextCommand::Cut => {
                let selection = self.buffer.selection();
                if !selection.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.buffer.text()[selection].to_owned(),
                    ));
                    if *command == TextCommand::Cut {
                        self.replace(None, "", window, cx);
                    }
                }
            }
            #[cfg(not(target_family = "wasm"))]
            TextCommand::Paste => self.paste_from_clipboard(window, cx),
            TextCommand::Undo | TextCommand::Redo => {
                let changed = if *command == TextCommand::Undo {
                    self.buffer.undo()
                } else {
                    self.buffer.redo()
                };
                self.edited(changed, window, cx);
            }
            TextCommand::Submit => cx.emit(TextInputEvent::Submit(self.buffer.value().clone())),
            TextCommand::Cancel => cx.emit(TextInputEvent::Cancel),
            TextCommand::CharacterPalette => window.show_character_palette(),
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn paste_from_clipboard(&self, window: &mut Window, cx: &mut Context<Self>) {
        let read = cx.read_from_clipboard_async();
        let text = self.buffer.text().clone();
        let selection = self.buffer.selection();
        cx.spawn_in(window, async move |this, cx| {
            if let Some(item) = read.await? {
                let _ = cx.update(|window, cx| {
                    this.update(cx, |input, cx| {
                        if input.focus.is_focused(window)
                            && window.is_focus_handle_rendered(&input.focus)
                            && input.buffer.marked().is_none()
                            && *input.buffer.text() == text
                            && input.buffer.selection() == selection
                        {
                            input.paste(item, window, cx);
                        }
                    })
                });
            }
            Ok::<_, open_gpui::ClipboardReadError>(())
        })
        .detach_and_log_err(cx);
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let window_id = window.window_handle().window_id();
        if self.blur.as_ref().is_none_or(|(id, _)| *id != window_id) {
            let subscription = cx.on_blur(&self.focus, window, |input, window, cx| {
                input.end_selection(window, cx);
                input.buffer.cancel_composition();
                cx.emit(TextInputEvent::Blur);
                cx.notify();
            });
            self.blur = Some((window_id, subscription));
        }
        let capture = self.drag.bind(window);
        let mut root = div();
        root.style().clone_from(&self.style);
        let mut root = root
            .id("text-input")
            .role(Role::TextInput)
            .aria_value(self.buffer.value().clone())
            .aria_placeholder(self.placeholder.clone())
            .aria_invalid(self.invalid)
            .track_focus(&self.focus)
            .track_pointer_capture(&capture)
            .tab_stop(true)
            .key_context(KEY_CONTEXT)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::on_command))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|input, _, window, cx| input.end_selection(window, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|input, _, window, cx| input.end_selection(window, cx)),
            );
        if let Some(label) = &self.label {
            root = root.aria_label(label.clone());
        }
        let input = cx.weak_entity();
        root.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
            if let Some(ActionData::Value(value)) = data {
                let _ = input.update(cx, |input, cx| {
                    let end = input.buffer.utf16_offset(input.buffer.text().len());
                    input.replace(Some(0..end), value, window, cx);
                });
            }
        })
        .child(
            div()
                .relative()
                .min_w_0()
                .overflow_hidden()
                .child(TextLine { input: cx.entity() })
                .child(on_pointer_cancel(cx, Self::end_selection)),
        )
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        utf16: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let bytes = self.buffer.byte_range(utf16);
        *actual = Some(self.buffer.utf16_range(bytes.clone()));
        Some(self.buffer.text()[bytes].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.utf16_range(self.buffer.selection()),
            reversed: self.buffer.reversed(),
        })
    }

    fn set_selected_text_range(
        &mut self,
        utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bytes = self.buffer.byte_range(utf16);
        self.buffer.select(bytes.start, bytes.end);
        self.selection_changed(window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buffer.utf16_offset(self.buffer.text().len()))
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked()
            .map(|range| self.buffer.utf16_range(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.buffer.unmark();
        self.edited(changed, window, cx);
    }

    fn replace_text_in_range(
        &mut self,
        utf16: Option<Range<usize>>,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(utf16, value, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        utf16: Option<Range<usize>>,
        value: &str,
        relative_selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.mark(utf16, value, relative_selection);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.buffer.byte_range(utf16);
        self.current_geometry()
            .map(|geometry| geometry.range_bounds(range))
    }

    fn selection_bounds_for_range(
        &mut self,
        utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(utf16, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let geometry = self.current_geometry()?;
        geometry
            .bounds
            .contains(&point)
            .then(|| self.buffer.utf16_offset(geometry.index_at(point)))
    }
}

struct TextGeometry {
    line: ShapedLine,
    text: SharedString,
    bounds: Bounds<Pixels>,
    scroll: Pixels,
}

impl TextGeometry {
    fn index_at(&self, position: Point<Pixels>) -> usize {
        self.line
            .closest_index_for_x(position.x - self.bounds.left() + self.scroll)
            .min(self.text.len())
    }

    fn range_bounds(&self, bytes: Range<usize>) -> Bounds<Pixels> {
        let edge = |index| {
            let x = if self.text.is_empty() {
                px(0.)
            } else {
                self.line.x_for_index(index)
            };
            (self.bounds.left() + x - self.scroll).clamp(self.bounds.left(), self.bounds.right())
        };
        let start = edge(bytes.start);
        let end = edge(bytes.end);
        let left = start
            .min(end)
            .min(self.bounds.right() - px(1.).min(self.bounds.size.width));
        let right = start.max(end).max((left + px(1.)).min(self.bounds.right()));
        Bounds::from_corners(
            point(left, self.bounds.top()),
            point(right, self.bounds.bottom()),
        )
    }
}

struct TextLine {
    input: Entity<TextInput>,
}

impl Element for TextLine {
    type RequestLayoutState = ();
    type PrepaintState = Option<TextGeometry>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let text = input.buffer.text().clone();
        let style = window.text_style();
        let (display, color) = if text.is_empty() {
            (input.placeholder.clone(), style.color.opacity(0.5))
        } else {
            (text.clone(), style.color)
        };
        let run = TextRun {
            len: display.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match input.buffer.marked() {
            Some(span) => [
                TextRun {
                    len: span.start,
                    ..run.clone()
                },
                TextRun {
                    len: span.len(),
                    underline: Some(UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display.len() - span.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len != 0)
            .collect::<Vec<_>>(),
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display, font_size, &runs, None);
        let scroll = if text.is_empty() {
            px(0.)
        } else {
            let old_scroll = input
                .geometry
                .as_ref()
                .map_or(px(0.), |geometry| geometry.scroll);
            let caret = line.x_for_index(input.buffer.head());
            let visible = (bounds.size.width - px(1.)).max(px(0.));
            let maximum = (line.width() - visible).max(px(0.));
            old_scroll
                .max(caret - visible)
                .min(caret)
                .clamp(px(0.), maximum)
        };
        Some(TextGeometry {
            line,
            text,
            bounds,
            scroll,
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(geometry) = prepaint.take() else {
            return;
        };
        let input = self.input.read(cx);
        let focused = input.focus.is_focused(window);
        let selection = input.buffer.selection();
        let color = window.text_style().color;
        if focused {
            window.handle_input(
                &input.focus,
                ElementInputHandler::new(bounds, self.input.clone()),
                cx,
            );
            if !selection.is_empty() {
                window.paint_quad(fill(
                    geometry.range_bounds(selection.clone()),
                    color.opacity(0.25),
                ));
            }
        }
        let origin = point(bounds.left() - geometry.scroll, bounds.top());
        let _ = geometry.line.paint(
            origin,
            bounds.size.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
        if focused && selection.is_empty() {
            window.paint_quad(fill(geometry.range_bounds(selection), color));
        }
        self.input
            .update(cx, |input, _| input.geometry = Some(geometry));
    }
}

#[derive(Clone, Default, PartialEq, open_gpui::Action)]
#[action(namespace = open_gpui_base, no_json, no_register)]
enum TextCommand {
    Move {
        forward: bool,
        extend: bool,
    },
    Edge {
        end: bool,
        extend: bool,
    },
    Delete {
        forward: bool,
    },
    SelectAll,
    Copy,
    Cut,
    #[cfg(not(target_family = "wasm"))]
    Paste,
    Undo,
    Redo,
    #[default]
    Submit,
    Cancel,
    CharacterPalette,
}

struct TextKeysInstalled;
impl Global for TextKeysInstalled {}

fn install_keys(cx: &mut App) {
    if cx.has_global::<TextKeysInstalled>() {
        return;
    }
    cx.set_global(TextKeysInstalled);
    use TextCommand::*;
    let keys = [
        (
            "left",
            Move {
                forward: false,
                extend: false,
            },
        ),
        (
            "right",
            Move {
                forward: true,
                extend: false,
            },
        ),
        (
            "shift-left",
            Move {
                forward: false,
                extend: true,
            },
        ),
        (
            "shift-right",
            Move {
                forward: true,
                extend: true,
            },
        ),
        (
            "home",
            Edge {
                end: false,
                extend: false,
            },
        ),
        (
            "end",
            Edge {
                end: true,
                extend: false,
            },
        ),
        (
            "shift-home",
            Edge {
                end: false,
                extend: true,
            },
        ),
        (
            "shift-end",
            Edge {
                end: true,
                extend: true,
            },
        ),
        ("backspace", Delete { forward: false }),
        ("delete", Delete { forward: true }),
        ("enter", Submit),
        ("escape", Cancel),
    ];
    let editing = if cx.platform_info().is_mac() {
        [
            ("cmd-a", SelectAll),
            ("cmd-c", Copy),
            ("cmd-x", Cut),
            ("cmd-z", Undo),
            ("cmd-shift-z", Redo),
            (
                "cmd-left",
                Edge {
                    end: false,
                    extend: false,
                },
            ),
            (
                "cmd-right",
                Edge {
                    end: true,
                    extend: false,
                },
            ),
            ("ctrl-cmd-space", CharacterPalette),
        ]
        .as_slice()
    } else {
        [
            ("ctrl-a", SelectAll),
            ("ctrl-c", Copy),
            ("ctrl-x", Cut),
            ("ctrl-z", Undo),
            ("ctrl-shift-z", Redo),
            ("ctrl-y", Redo),
        ]
        .as_slice()
    };
    let bindings = keys.into_iter().chain(editing.iter().cloned());
    #[cfg(not(target_family = "wasm"))]
    let bindings = bindings.chain([(
        if cx.platform_info().is_mac() {
            "cmd-v"
        } else {
            "ctrl-v"
        },
        Paste,
    )]);
    cx.bind_keys(bindings.map(|(key, command)| KeyBinding::new(key, command, Some(KEY_CONTEXT))));
}
