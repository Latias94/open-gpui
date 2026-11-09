use std::{cell::RefCell, ops::Range, rc::Rc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, ClipboardEntry, ClipboardItem, ClipboardReadError, Context, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, KeyDownEvent, Pixels, Point, PrepaintPublicationId,
    Task, UTF16Selection, Window, WindowBounds, WindowOptions, canvas, div, px, rgb, size,
};
#[derive(Default)]
struct Observation {
    text: [String; 2],
    pastes: [usize; 2],
    image_bytes: [usize; 2],
    last_entries: [String; 2],
    last_image_digest: [u32; 2],
    installed_target: Option<usize>,
    accepted_generation: u64,
    async_reads: usize,
    async_completions: usize,
    read_status: String,
    activation_at_read: bool,
    active: usize,
    consumed_paste: bool,
    renders: usize,
    paints: usize,
    closed: bool,
}

impl Observation {
    fn publish(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let probe = super::js_object(&[
            ("text0", self.text[0].as_str().into()),
            ("text1", self.text[1].as_str().into()),
            ("pastes0", (self.pastes[0] as f64).into()),
            ("pastes1", (self.pastes[1] as f64).into()),
            ("imageBytes0", (self.image_bytes[0] as f64).into()),
            ("imageBytes1", (self.image_bytes[1] as f64).into()),
            ("entries0", self.last_entries[0].as_str().into()),
            ("entries1", self.last_entries[1].as_str().into()),
            ("imageDigest0", self.last_image_digest[0].into()),
            ("imageDigest1", self.last_image_digest[1].into()),
            (
                "installedTarget",
                self.installed_target
                    .map(|target| (target as f64).into())
                    .unwrap_or(wasm_bindgen::JsValue::NULL),
            ),
            (
                "acceptedGeneration",
                (self.accepted_generation as f64).into(),
            ),
            ("asyncReads", (self.async_reads as f64).into()),
            ("asyncCompletions", (self.async_completions as f64).into()),
            ("readStatus", self.read_status.as_str().into()),
            ("activationAtRead", self.activation_at_read.into()),
            ("active", (self.active as f64).into()),
            ("consumedPaste", self.consumed_paste.into()),
            ("renders", (self.renders as f64).into()),
            ("paints", (self.paints as f64).into()),
            ("closed", self.closed.into()),
        ]);
        js_sys::Reflect::set(window.as_ref(), &"__OPEN_GPUI_CLIPBOARD__".into(), &probe)
            .expect("clipboard probe must be writable");
    }

    fn paste(&mut self, target: usize, item: ClipboardItem) {
        self.pastes[target] += 1;
        if let Some(text) = item.text() {
            self.text[target].push_str(&text);
        }
        let mut kinds = Vec::new();
        for entry in item.into_entries() {
            match entry {
                ClipboardEntry::Image(image) => {
                    self.image_bytes[target] += image.bytes().len();
                    self.last_image_digest[target] =
                        image.bytes().iter().fold(2166136261_u32, |hash, byte| {
                            (hash ^ u32::from(*byte)).wrapping_mul(16777619)
                        });
                    kinds.push(format!("{:?}", image.format()));
                }
                ClipboardEntry::String(_) => kinds.push("text".into()),
                ClipboardEntry::ExternalPaths(_) => kinds.push("paths".into()),
            }
        }
        self.last_entries[target] = kinds.join(",");
        self.publish();
    }
}

struct ClipboardSmoke {
    focus: [FocusHandle; 2],
    fields: [Entity<ClipboardField>; 2],
    publication: PrepaintPublicationId,
    observation: Rc<RefCell<Observation>>,
    read_task: Option<Task<()>>,
}

struct ClipboardField {
    target: usize,
    observation: Rc<RefCell<Observation>>,
}

impl ClipboardSmoke {
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let command = matches!(
            event.keystroke.key.as_str(),
            "r" | "s" | "k" | "p" | "c" | "x"
        ) || (event.keystroke.key == "v" && self.observation.borrow().consumed_paste);
        match event.keystroke.key.as_str() {
            "r" => {
                let read = cx.read_from_clipboard_async();
                {
                    let mut observation = self.observation.borrow_mut();
                    observation.async_reads += 1;
                    observation.read_status = "pending".into();
                    let navigator = web_sys::window().unwrap().navigator();
                    observation.activation_at_read =
                        js_sys::Reflect::get(navigator.as_ref(), &"userActivation".into())
                            .ok()
                            .and_then(|activation| {
                                js_sys::Reflect::get(&activation, &"isActive".into()).ok()
                            })
                            .and_then(|active| active.as_bool())
                            .unwrap_or(false);
                    observation.publish();
                }
                self.read_task = Some(cx.spawn_in(window, async move |this, cx| {
                    let result = read.await;
                    this.update_in(cx, |this, _, cx| {
                        let mut observation = this.observation.borrow_mut();
                        observation.async_completions += 1;
                        observation.read_status = match result {
                            Ok(Some(item)) => {
                                let target = observation.active;
                                observation.paste(target, item);
                                "item".into()
                            }
                            Ok(None) => "empty".into(),
                            Err(ClipboardReadError::Unavailable) => "unavailable".into(),
                            Err(ClipboardReadError::Denied(_)) => "denied".into(),
                            Err(ClipboardReadError::UnsupportedContent) => "unsupported".into(),
                        };
                        observation.publish();
                        cx.notify();
                    })
                    .ok();
                }));
            }
            "s" => {
                let mut observation = self.observation.borrow_mut();
                observation.active = 1 - observation.active;
                self.focus[observation.active].focus(window, cx);
                observation.publish();
                cx.notify();
            }
            "k" => {
                let mut observation = self.observation.borrow_mut();
                observation.consumed_paste = !observation.consumed_paste;
                observation.publish();
            }
            "v" if self.observation.borrow().consumed_paste => {
                window.prevent_default();
                cx.stop_propagation();
            }
            "p" => cx.write_to_clipboard(ClipboardItem::new_string("Written by GPUI 文本".into())),
            "c" => {
                self.read_task.take();
            }
            "x" => {
                let mut observation = self.observation.borrow_mut();
                observation.closed = true;
                observation.publish();
                window.remove_window(cx);
            }
            _ => {}
        }
        if command {
            window.prevent_default();
            cx.stop_propagation();
        }
    }
}

impl Render for ClipboardSmoke {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut observation = self.observation.borrow_mut();
        observation.renders += 1;
        observation.publish();
        let focus = self.focus[observation.active].clone();
        let text = format!("{} | {}", observation.text[0], observation.text[1]);
        let target = observation.active;
        let entity = self.fields[target].clone();
        let _ = entity.read(cx);
        let publication = self.publication;
        let accepted = self.observation.clone();
        let paints = self.observation.clone();
        div()
            .size_full()
            .bg(rgb(0x101318))
            .text_color(rgb(0xffffff))
            .track_focus(&focus)
            .on_key_down(cx.listener(Self::key))
            .child("Clipboard: R read, S switch field, K consume paste, P write, C cancel, X close")
            .child(text)
            .child(
                canvas(
                    move |_, window, _| {
                        let committed = accepted.clone();
                        let discarded = accepted.clone();
                        window.record_prepaint_window_transaction(
                            publication,
                            move |fence, _, _| {
                                let mut observation = committed.borrow_mut();
                                observation.installed_target = Some(target);
                                observation.accepted_generation = fence.generation();
                                observation.publish();
                            },
                            move |_, _, _| {
                                let mut observation = discarded.borrow_mut();
                                observation.installed_target = None;
                                observation.publish();
                            },
                        );
                    },
                    move |bounds, _, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                        let mut observation = paints.borrow_mut();
                        observation.paints += 1;
                        observation.publish();
                    },
                )
                .size_full(),
            )
    }
}

impl EntityInputHandler for ClipboardField {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let observation = self.observation.borrow();
        Some(observation.text[self.target].clone())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let observation = self.observation.borrow();
        let end = observation.text[self.target].encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        self.observation.borrow_mut().paste(self.target, item);
        window.refresh();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observation
            .borrow_mut()
            .paste(self.target, ClipboardItem::new_string(text.into()));
        window.refresh();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

pub(super) fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .is_some_and(|params| params.get("clipboard").as_deref() == Some("1"))
}

pub(super) fn open(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| {
            let focus = [cx.focus_handle(), cx.focus_handle()];
            focus[0].focus(window, cx);
            let observation: Rc<RefCell<Observation>> = Rc::default();
            let fields = std::array::from_fn(|target| {
                cx.new(|_| ClipboardField {
                    target,
                    observation: observation.clone(),
                })
            });
            cx.new(|_| ClipboardSmoke {
                focus,
                fields,
                publication: PrepaintPublicationId::new(),
                observation,
                read_task: None,
            })
        },
    )
    .expect("clipboard smoke window must open");
    cx.activate(true);
}
