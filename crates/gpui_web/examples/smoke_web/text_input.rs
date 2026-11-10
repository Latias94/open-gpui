//! Public text-session consumer. Model observations are published after frame acceptance.

use std::{ops::Range, rc::Rc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Autocapitalize, Bounds, Context, CursorStyle, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, KeyDownEvent, MouseButton, Pixels, Point,
    PrepaintPublicationId, ShapedLine, TextAlign, TextInputAction, TextInputConfiguration, TextRun,
    UTF16Selection, Window, WindowBounds, WindowOptions, canvas, div, fill, point, px, rgb, rgba,
    size,
};
use wasm_bindgen::JsValue;

const SEEDS: [&str; 2] = ["A😀e\u{301}中\nsecond", "β🦀two"];
const PREFIXES: [&str; 2] = ["A⟦", "B⟦"];
const SUFFIX: &str = "⟧";
const LINE_HEIGHT: f32 = 24.0;

fn byte_offset(text: &str, utf16_offset: usize) -> usize {
    let mut utf16 = 0;
    for (byte, ch) in text.char_indices() {
        if utf16 >= utf16_offset {
            return byte;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

fn byte_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = byte_offset(text, range.start);
    start..byte_offset(text, range.end).max(start)
}

fn utf16_range(text: &str, range: Range<usize>) -> Range<usize> {
    text[..range.start].encode_utf16().count()..text[..range.end].encode_utf16().count()
}

fn range_value(range: Range<usize>) -> JsValue {
    super::js_object(&[
        ("start", (range.start as f64).into()),
        ("end", (range.end as f64).into()),
    ])
}

fn bounds_value(bounds: Bounds<Pixels>) -> JsValue {
    super::js_object(&[
        ("x", f32::from(bounds.origin.x).into()),
        ("y", f32::from(bounds.origin.y).into()),
        ("width", f32::from(bounds.size.width).into()),
        ("height", f32::from(bounds.size.height).into()),
    ])
}

fn configuration(target: usize) -> TextInputConfiguration {
    TextInputConfiguration {
        autocorrect: target == 1,
        autocapitalize: if target == 0 {
            Autocapitalize::None
        } else {
            Autocapitalize::Words
        },
        suggestions: target == 1,
        input_action: if target == 0 {
            TextInputAction::Enter
        } else {
            TextInputAction::Send
        },
    }
}

struct TextLine {
    start: usize,
    shaped: ShapedLine,
    bounds: Bounds<Pixels>,
}

struct TextLayout {
    text: String,
    bounds: Bounds<Pixels>,
    lines: Vec<TextLine>,
}

impl TextLayout {
    fn rectangles(&self, range: Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut rectangles = Vec::new();
        for line in &self.lines {
            let end = line.start + line.shaped.text.len();
            if range.start > end || range.end < line.start {
                continue;
            }
            if !range.is_empty() && range.end == line.start {
                break;
            }
            let start_x = line
                .shaped
                .x_for_index(range.start.saturating_sub(line.start).min(end - line.start));
            let mut end_x = line
                .shaped
                .x_for_index(range.end.saturating_sub(line.start).min(end - line.start));
            if range.end > end {
                end_x += px(4.0);
            }
            rectangles.push(Bounds::new(
                point(line.bounds.left() + start_x, line.bounds.top()),
                size((end_x - start_x).max(px(2.0)), px(LINE_HEIGHT)),
            ));
            if range.is_empty() {
                break;
            }
        }
        rectangles
    }

    fn index_for_point(&self, position: Point<Pixels>) -> Option<usize> {
        let line = self
            .lines
            .iter()
            .find(|line| position.y < line.bounds.bottom())
            .or_else(|| self.lines.last())?;
        Some(
            line.start
                + line
                    .shaped
                    .closest_index_for_x(position.x - line.bounds.left()),
        )
    }
}

#[derive(Default)]
struct EditCounts {
    replacements: u32,
    compositions: u32,
    unmarks: u32,
    native_selections: u32,
    content_notifications: u32,
    selection_notifications: u32,
    programmatic_edits: u32,
    configuration_changes: u32,
}

struct TextField {
    target: usize,
    focus: FocusHandle,
    text: String,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
    configuration: TextInputConfiguration,
    layout: Option<Rc<TextLayout>>,
    publication: PrepaintPublicationId,
    counts: EditCounts,
    last_change: &'static str,
}

impl TextField {
    fn new(target: usize, focus: FocusHandle) -> Self {
        let text = format!("{}{}{}", PREFIXES[target], SEEDS[target], SUFFIX);
        let end = text.len() - SUFFIX.len();
        Self {
            target,
            focus,
            text,
            selection: end..end,
            marked: None,
            configuration: configuration(target),
            layout: None,
            publication: PrepaintPublicationId::new(),
            counts: EditCounts::default(),
            last_change: "initial",
        }
    }

    fn editable(&self) -> Range<usize> {
        PREFIXES[self.target].len()..self.text.len() - SUFFIX.len()
    }

    fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let editable = self.editable();
        let start = range.start.clamp(editable.start, editable.end);
        start..range.end.clamp(start, editable.end)
    }

    fn notify_selection(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.counts.selection_notifications += 1;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn notify_content(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.layout = None;
        self.counts.content_notifications += 1;
        window.notify_text_input_changed(&self.focus);
        self.notify_selection(window, cx);
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .map(|range| byte_range(&self.text, range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let range = self.clamp(range);
        self.text.replace_range(range.clone(), text);
        let inserted = range.start..range.start + text.len();
        self.selection = inserted.end..inserted.end;
        inserted
    }

    fn set_body(&mut self, text: &str, window: &Window, cx: &mut Context<Self>) {
        self.text.replace_range(self.editable(), text);
        let end = self.editable().end;
        self.selection = end..end;
        self.marked = None;
        self.counts.programmatic_edits += 1;
        self.last_change = "programmatic-content";
        self.notify_content(window, cx);
    }

    fn snapshot(&self, window: &Window) -> JsValue {
        let editable = self.editable();
        let counts = &self.counts;
        let selection_bounds: js_sys::Array = self
            .layout
            .as_ref()
            .filter(|layout| layout.text == self.text)
            .map(|layout| layout.rectangles(self.selection.clone()))
            .unwrap_or_default()
            .into_iter()
            .map(bounds_value)
            .collect();
        super::js_object(&[
            ("id", format!("field-{}", self.target).into()),
            ("text", self.text.as_str().into()),
            (
                "editableText",
                JsValue::from_str(&self.text[editable.clone()]),
            ),
            (
                "lengthUtf16",
                (self.text.encode_utf16().count() as f64).into(),
            ),
            (
                "editableRangeUtf16",
                range_value(utf16_range(&self.text, editable)),
            ),
            (
                "selectionUtf16",
                range_value(utf16_range(&self.text, self.selection.clone())),
            ),
            ("selectionReversed", false.into()),
            (
                "selectedText",
                JsValue::from_str(&self.text[self.selection.clone()]),
            ),
            (
                "markedRangeUtf16",
                self.marked
                    .clone()
                    .map(|range| range_value(utf16_range(&self.text, range)))
                    .unwrap_or(JsValue::NULL),
            ),
            (
                "markedText",
                self.marked
                    .as_ref()
                    .map(|range| JsValue::from_str(&self.text[range.clone()]))
                    .unwrap_or(JsValue::NULL),
            ),
            ("focused", self.focus.is_focused(window).into()),
            (
                "focusRendered",
                window.is_focus_handle_rendered(&self.focus).into(),
            ),
            (
                "paintedText",
                self.layout
                    .as_ref()
                    .map(|layout| JsValue::from_str(&layout.text))
                    .unwrap_or(JsValue::NULL),
            ),
            (
                "bounds",
                self.layout
                    .as_ref()
                    .map(|layout| bounds_value(layout.bounds))
                    .unwrap_or(JsValue::NULL),
            ),
            ("selectionBounds", selection_bounds.into()),
            (
                "configuration",
                super::js_object(&[
                    ("autocorrect", self.configuration.autocorrect.into()),
                    (
                        "autocapitalize",
                        format!("{:?}", self.configuration.autocapitalize).into(),
                    ),
                    ("suggestions", self.configuration.suggestions.into()),
                    (
                        "inputAction",
                        format!("{:?}", self.configuration.input_action).into(),
                    ),
                ]),
            ),
            ("lastChange", self.last_change.into()),
            (
                "counts",
                super::js_object(&[
                    ("replacements", counts.replacements.into()),
                    ("compositionUpdates", counts.compositions.into()),
                    ("unmarks", counts.unmarks.into()),
                    ("nativeSelections", counts.native_selections.into()),
                    ("contentNotifications", counts.content_notifications.into()),
                    (
                        "selectionNotifications",
                        counts.selection_notifications.into(),
                    ),
                    ("programmaticEdits", counts.programmatic_edits.into()),
                    ("configurationChanges", counts.configuration_changes.into()),
                ]),
            ),
        ])
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = byte_range(&self.text, range);
        *adjusted_range = Some(utf16_range(&self.text, range.clone()));
        Some(self.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: utf16_range(&self.text, self.selection.clone()),
            reversed: false,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = self.clamp(byte_range(&self.text, range));
        self.counts.native_selections += 1;
        self.last_change = "platform-selection";
        self.notify_selection(window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .clone()
            .map(|range| utf16_range(&self.text, range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        self.counts.unmarks += 1;
        self.last_change = "platform-unmark";
        self.notify_content(window, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text);
        self.marked = None;
        self.counts.replacements += 1;
        self.last_change = "platform-replace";
        self.notify_content(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.replace(range, text);
        self.marked = (!text.is_empty()).then_some(inserted.clone());
        if let Some(selected) = selected {
            let selected = byte_range(text, selected);
            self.selection = inserted.start + selected.start..inserted.start + selected.end;
        }
        self.counts.compositions += 1;
        self.last_change = "platform-composition";
        self.notify_content(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self
            .layout
            .as_ref()
            .filter(|layout| layout.text == self.text)?;
        layout
            .rectangles(byte_range(&self.text, range))
            .into_iter()
            .next()
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.layout
            .as_ref()
            .filter(|layout| layout.text == self.text)
            .map(|layout| layout.rectangles(byte_range(&self.text, range)))
            .unwrap_or_default()
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self
            .layout
            .as_ref()
            .filter(|layout| layout.text == self.text)?;
        let index = layout.index_for_point(position)?;
        Some(self.text[..index].encode_utf16().count())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        true
    }

    fn text_input_configuration(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> TextInputConfiguration {
        self.configuration.clone()
    }

    fn text_input_editable_range(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        Some(utf16_range(&self.text, self.editable()))
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field = cx.entity();
        let committed_field = field.clone();
        let discarded_field = field.clone();
        let focus = self.focus.clone();
        let text = self.text.clone();
        let selection = self.selection.clone();
        let marked = self.marked.clone();
        let publication = self.publication;
        let focused = self.focus.is_focused(window);
        let status = format!(
            "Field {} | selection {:?} | marked {:?}",
            self.target,
            utf16_range(&self.text, selection.clone()),
            marked.clone().map(|range| utf16_range(&self.text, range)),
        );
        div()
            .id(("text-input-field", self.target))
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .border_1()
            .border_color(rgb(if focused { 0x58a6ff } else { 0x3b4656 }))
            .p_2()
            .bg(rgb(0x18212d))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(
                    |this,
                     event: &open_gpui::TargetedEvent<open_gpui::MouseDownEvent>,
                     window,
                     cx| {
                        this.focus.focus(window, cx);
                        window.request_virtual_keyboard();
                        if let Ok(position) = event.target_layout_position()
                            && let Some(layout) = &this.layout
                            && layout.text == this.text
                            && let Some(index) = layout.index_for_point(position)
                        {
                            this.selection = this.clamp(index..index);
                            this.last_change = "pointer-selection";
                            this.notify_selection(window, cx);
                        }
                        cx.notify();
                    },
                ),
            )
            .child(status)
            .child(
                canvas(
                    move |bounds, window, _| {
                        let style = window.text_style();
                        let mut start = 0;
                        let lines = text
                            .split('\n')
                            .enumerate()
                            .map(|(row, text)| {
                                let shaped = window.text_system().shape_line(
                                    text.to_owned().into(),
                                    px(18.0),
                                    &[TextRun {
                                        len: text.len(),
                                        font: style.font(),
                                        color: rgb(0xf0f4fa).into(),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    }],
                                    None,
                                );
                                let line = TextLine {
                                    start,
                                    shaped,
                                    bounds: Bounds::new(
                                        point(
                                            bounds.left(),
                                            bounds.top() + px(row as f32 * LINE_HEIGHT),
                                        ),
                                        size(bounds.size.width, px(LINE_HEIGHT)),
                                    ),
                                };
                                start += text.len() + 1;
                                line
                            })
                            .collect();
                        let layout = Rc::new(TextLayout {
                            text,
                            bounds,
                            lines,
                        });
                        let committed_layout = layout.clone();
                        window.record_prepaint_window_transaction(
                            publication,
                            move |_, _, cx| {
                                committed_field.update(cx, |field, _| {
                                    field.layout = Some(committed_layout.clone());
                                });
                            },
                            move |_, _, cx| {
                                discarded_field.update(cx, |field, _| field.layout = None);
                            },
                        );
                        layout
                    },
                    move |bounds, layout, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, field), cx);
                        if !selection.is_empty() {
                            for rect in layout.rectangles(selection.clone()) {
                                window.paint_quad(fill(rect, rgba(0x3b82f680)));
                            }
                        }
                        for line in &layout.lines {
                            line.shaped
                                .paint(
                                    line.bounds.origin,
                                    px(LINE_HEIGHT),
                                    TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                )
                                .expect("smoke text must paint");
                        }
                        if let Some(marked) = marked {
                            for rect in layout.rectangles(marked) {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(rect.left(), rect.bottom() - px(2.0)),
                                        size(rect.size.width, px(2.0)),
                                    ),
                                    rgb(0xfbbf24),
                                ));
                            }
                        }
                        if focused && selection.is_empty() {
                            for rect in layout.rectangles(selection) {
                                window.paint_quad(fill(rect, rgb(0x58a6ff)));
                            }
                        }
                    },
                )
                .w_full()
                .h(px(96.0)),
            )
    }
}

#[derive(Clone, Copy)]
enum Command {
    FocusA,
    FocusB,
    Blur,
    Content,
    Selection,
    Configure,
    Repaint,
    Reset,
    Dismiss,
    Recover,
    Quiesce,
}

const COMMANDS: [(Command, &str); 11] = [
    (Command::FocusA, "F1: Field A"),
    (Command::FocusB, "F2: Field B"),
    (Command::Blur, "F3: Blur"),
    (Command::Content, "F4: Set text"),
    (Command::Selection, "F5: Select"),
    (Command::Configure, "F6: Config"),
    (Command::Repaint, "F7: Repaint"),
    (Command::Reset, "F8: Reset"),
    (Command::Dismiss, "F9: Hide keyboard"),
    (Command::Recover, "F10: Recover"),
    (Command::Quiesce, "F11: Quiesce"),
];

struct TextInputSmoke {
    fields: [Entity<TextField>; 2],
    parking_focus: FocusHandle,
    active: usize,
    graphics: super::GraphicsProbe,
    publication: PrepaintPublicationId,
    renders: u32,
}

impl TextInputSmoke {
    fn command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active) = self
            .fields
            .iter()
            .position(|field| field.read(cx).focus.is_focused(window))
        {
            self.active = active;
        }
        match command {
            Command::Quiesce => {
                window.defer(cx, |window, cx| {
                    window.quiesce_interaction(cx);
                });
            }
            Command::FocusA | Command::FocusB | Command::Recover => {
                match command {
                    Command::FocusA => self.active = 0,
                    Command::FocusB => self.active = 1,
                    _ => {}
                }
                let focus = self.fields[self.active].read(cx).focus.clone();
                focus.focus(window, cx);
                window.request_virtual_keyboard();
            }
            Command::Blur => self.parking_focus.focus(window, cx),
            Command::Content => self.fields[self.active].update(cx, |field, cx| {
                field.set_body("Programmatic 😀\nselection 中", window, cx);
            }),
            Command::Selection => self.fields[self.active].update(cx, |field, cx| {
                let start = field.text[..field.editable().start].encode_utf16().count();
                field.selection = field.clamp(byte_range(&field.text, start + 1..start + 3));
                field.last_change = "programmatic-selection";
                field.notify_selection(window, cx);
            }),
            Command::Configure => self.fields[self.active].update(cx, |field, cx| {
                let enabled = !field.configuration.autocorrect;
                field.configuration = TextInputConfiguration {
                    autocorrect: enabled,
                    autocapitalize: if enabled {
                        Autocapitalize::Sentences
                    } else {
                        Autocapitalize::None
                    },
                    suggestions: enabled,
                    input_action: if enabled {
                        TextInputAction::Search
                    } else {
                        TextInputAction::Enter
                    },
                };
                field.counts.configuration_changes += 1;
                field.last_change = "programmatic-configuration";
                cx.notify();
            }),
            Command::Repaint => {}
            Command::Reset => {
                for field in &self.fields {
                    field.update(cx, |field, cx| {
                        let configuration = configuration(field.target);
                        if field.configuration != configuration {
                            field.configuration = configuration;
                            field.counts.configuration_changes += 1;
                        }
                        field.set_body(SEEDS[field.target], window, cx);
                    });
                }
            }
            Command::Dismiss => window.dismiss_virtual_keyboard(),
        }
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let index = match event.keystroke.key.as_str() {
            "f1" => 0,
            "f2" => 1,
            "f3" => 2,
            "f4" => 3,
            "f5" => 4,
            "f6" => 5,
            "f7" => 6,
            "f8" => 7,
            "f9" => 8,
            "f10" => 9,
            "f11" => 10,
            "tab" => {
                if self.fields[0].read(cx).focus.is_focused(window) {
                    1
                } else {
                    0
                }
            }
            _ => return,
        };
        window.prevent_default();
        cx.stop_propagation();
        self.command(COMMANDS[index].0, window, cx);
    }
}

impl Render for TextInputSmoke {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let fields = self.fields.clone();
        let graphics = self.graphics;
        let renders = self.renders;
        let publication = self.publication;
        let viewport = viewport_value(window);
        let geometry = format!(
            "Layout {:?} | visible {:?} | safe {:?}",
            window.viewport_size(),
            window.visual_viewport_bounds(),
            window.insets().safe_area,
        );
        let content = div()
            .id("text-input-content")
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .overflow_y_scroll()
            .child("Public Web text session: blue selection/caret, amber composition underline")
            .child("The A⟦ / B⟦ prefixes and ⟧ suffixes are outside each editable range.")
            .child(
                div().flex().flex_wrap().gap_2().flex_shrink_0().children(
                    COMMANDS.into_iter().enumerate().map(|(index, (command, label))| {
                        div()
                            .id(("text-input-command", index))
                            .px_2()
                            .py_1()
                            .border_1()
                            .border_color(rgb(0x3b4656))
                            .cursor_pointer()
                            .child(label)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.command(command, window, cx);
                            }))
                    }),
                ),
            )
            .children(self.fields.iter().cloned())
            .child(geometry)
            .child("Use browser editing. Synthetic composition events do not establish OS IME behavior.");
        div()
            .id("text-input-smoke")
            .relative()
            .size_full()
            .bg(rgb(0x101318))
            .text_color(rgb(0xf0f4fa))
            .text_size(px(14.0))
            .track_focus(&self.parking_focus)
            .on_key_down(cx.listener(Self::key))
            .child(content)
            .child(
                canvas(
                    move |_, window, _| {
                        window.record_prepaint_window_transaction(
                            publication,
                            move |fence, window, cx| {
                                let snapshots: js_sys::Array = fields
                                    .iter()
                                    .map(|field| field.read(cx).snapshot(window))
                                    .collect();
                                let focused = fields
                                    .iter()
                                    .position(|field| field.read(cx).focus.is_focused(window));
                                let probe = super::js_object(&[
                                    (
                                        "ready",
                                        fields
                                            .iter()
                                            .any(|field| field.read(cx).layout.is_some())
                                            .into(),
                                    ),
                                    ("observation", "accepted-app-model".into()),
                                    ("acceptedGeneration", (fence.generation() as f64).into()),
                                    ("renderCalls", renders.into()),
                                    (
                                        "focusedField",
                                        focused
                                            .map(|index| (index as f64).into())
                                            .unwrap_or(JsValue::NULL),
                                    ),
                                    ("windowActive", window.is_window_active().into()),
                                    ("fields", snapshots.into()),
                                    ("viewport", viewport.clone()),
                                    (
                                        "notificationObservation",
                                        "Fixture calls to public Window notification methods"
                                            .into(),
                                    ),
                                ]);
                                graphics.publish(&probe);
                                publish(&probe);
                            },
                            move |fence, _, _| {
                                publish(&super::js_object(&[
                                    ("ready", false.into()),
                                    ("observation", "discarded-app-publication".into()),
                                    ("acceptedGeneration", (fence.generation() as f64).into()),
                                ]));
                            },
                        );
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(px(1.0)),
            )
    }
}

fn viewport_value(window: &Window) -> JsValue {
    let insets = window.insets();
    let effective = insets.effective();
    let edges = |edges: open_gpui::Edges<Pixels>| {
        super::js_object(&[
            ("top", f32::from(edges.top).into()),
            ("right", f32::from(edges.right).into()),
            ("bottom", f32::from(edges.bottom).into()),
            ("left", f32::from(edges.left).into()),
        ])
    };
    super::js_object(&[
        (
            "layout",
            bounds_value(Bounds::new(point(px(0.0), px(0.0)), window.viewport_size())),
        ),
        ("visual", bounds_value(window.visual_viewport_bounds())),
        ("fullyVisible", bounds_value(window.fully_visible_bounds())),
        ("safeArea", edges(insets.safe_area)),
        ("ime", edges(insets.ime)),
        ("effective", edges(effective)),
        (
            "cornerRadii",
            super::js_object(&[
                ("topLeft", f32::from(insets.corner_radii.top_left).into()),
                ("topRight", f32::from(insets.corner_radii.top_right).into()),
                (
                    "bottomRight",
                    f32::from(insets.corner_radii.bottom_right).into(),
                ),
                (
                    "bottomLeft",
                    f32::from(insets.corner_radii.bottom_left).into(),
                ),
            ]),
        ),
        ("scaleFactor", window.scale_factor().into()),
    ])
}

fn publish(probe: &JsValue) {
    if let Some(window) = web_sys::window() {
        for name in [
            "__OPEN_GPUI_WEB_TEXT_INPUT_MODEL__",
            "__OPEN_GPUI_WEB_SMOKE__",
        ] {
            js_sys::Reflect::set(window.as_ref(), &name.into(), probe)
                .expect("text input model probe must be writable");
        }
        if let Some(element) = window
            .document()
            .and_then(|document| document.get_element_by_id("text-input-model"))
        {
            let text = js_sys::JSON::stringify(probe)
                .ok()
                .and_then(|value| value.as_string());
            element.set_text_content(text.as_deref());
        }
    }
}

pub(super) fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .is_some_and(|params| params.get("text_input").as_deref() == Some("1"))
}

pub(super) fn open(cx: &mut App, graphics: super::GraphicsProbe) {
    let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            activation_policy: super::activation_probe_config()
                .map(|configuration| configuration.policy)
                .unwrap_or_default(),
            ..Default::default()
        },
        move |window, cx| {
            let fields = std::array::from_fn(|target| {
                cx.new(|cx| TextField::new(target, cx.focus_handle()))
            });
            let focus = fields[0].read(cx).focus.clone();
            focus.focus(window, cx);
            cx.new(|cx| {
                for field in &fields {
                    cx.observe(field, |_, _, cx| cx.notify()).detach();
                }
                cx.observe_window_activation(window, |_, _, cx| cx.notify())
                    .detach();
                TextInputSmoke {
                    fields,
                    parking_focus: cx.focus_handle(),
                    active: 0,
                    graphics,
                    publication: PrepaintPublicationId::new(),
                    renders: 0,
                }
            })
        },
    )
    .expect("text input smoke window must open");
    cx.activate(true);
}
