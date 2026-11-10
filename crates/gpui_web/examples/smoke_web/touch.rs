//! Recognized Core outputs and real scrolling, separate from raw DOM pointer metadata.

use std::{cell::RefCell, ops::Range, rc::Rc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Pixels, Point,
    ScrollEvent, ScrollHandle, ScrollSource, Subscription, TouchPhase, UTF16Selection, Window,
    WindowBounds, WindowMouseEvent, WindowOptions, canvas, div, px, rgb, size,
};
use wasm_bindgen::JsValue;

const TRACE_LIMIT: usize = 256;

struct Observation {
    graphics: super::GraphicsProbe,
    gestures: Vec<JsValue>,
    scroll: Vec<JsValue>,
    clicks: u32,
    painted: bool,
    overflow: bool,
    offset_y: f32,
}

fn phase_name(phase: TouchPhase) -> &'static str {
    match phase {
        TouchPhase::Started => "started",
        TouchPhase::Moved => "moved",
        TouchPhase::Ended => "ended",
        TouchPhase::Cancelled => "cancelled",
    }
}

fn scroll_metadata(event: &ScrollEvent) -> JsValue {
    super::js_object(&[
        ("kind", "scroll".into()),
        ("phase", phase_name(event.touch_phase).into()),
        (
            "source",
            match event.source {
                ScrollSource::Pointer => "pointer",
                ScrollSource::Touch => "touch",
            }
            .into(),
        ),
    ])
}

impl Observation {
    fn record_gesture(&mut self, kind: &str, phase: TouchPhase, position: Point<Pixels>) {
        if self.gestures.len() < TRACE_LIMIT {
            self.gestures.push(super::js_object(&[
                ("kind", kind.into()),
                ("phase", phase_name(phase).into()),
                ("x", f32::from(position.x).into()),
                ("y", f32::from(position.y).into()),
            ]));
        } else {
            self.overflow = true;
        }
        self.publish();
    }

    fn record_scroll(&mut self, event: &ScrollEvent) {
        if self.scroll.len() < TRACE_LIMIT {
            self.scroll.push(scroll_metadata(event));
        } else {
            self.overflow = true;
        }
        self.publish();
    }

    fn publish(&self) {
        let gestures: js_sys::Array = self.gestures.iter().collect();
        let scroll: js_sys::Array = self.scroll.iter().collect();
        let probe = super::js_object(&[
            ("ready", self.painted.into()),
            ("gestures", gestures.into()),
            ("scroll", scroll.into()),
            ("clicks", self.clicks.into()),
            ("overflow", self.overflow.into()),
            ("offsetY", self.offset_y.into()),
            ("clickObservation", "core-element-on-click".into()),
            ("gestureObservation", "core-window-interceptor".into()),
            ("scrollObservation", "core-window-interceptor".into()),
        ]);
        self.graphics.publish(&probe);
        if let Some(window) = web_sys::window() {
            for name in ["__OPEN_GPUI_WEB_TOUCH_CORE__", "__OPEN_GPUI_WEB_SMOKE__"] {
                js_sys::Reflect::set(window.as_ref(), &name.into(), &probe)
                    .expect("touch smoke probe must be writable");
            }
        }
    }
}

struct TouchSmoke {
    focus: FocusHandle,
    scroll: ScrollHandle,
    observation: Rc<RefCell<Observation>>,
    text: String,
    _subscription: Subscription,
}

impl Render for TouchSmoke {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().clone();
        let focus = self.focus.clone();
        let scroll = self.scroll.clone();
        let observation = self.observation.clone();
        div()
            .id("touch-smoke")
            .size_full()
            .bg(rgb(0x101318))
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_click(cx.listener(|this, _, _, _| {
                let mut observation = this.observation.borrow_mut();
                observation.clicks += 1;
                observation.publish();
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                        let mut observation = observation.borrow_mut();
                        observation.painted = true;
                        observation.offset_y = scroll.offset().y.into();
                        observation.publish();
                    },
                )
                .w_full()
                .flex_shrink_0()
                .h(px(1600.)),
            )
    }
}

impl EntityInputHandler for TouchSmoke {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        Some(self.text.clone())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.text.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.text.push_str(text);
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
        Some(self.text.encode_utf16().count())
    }
}

pub(super) fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .is_some_and(|params| params.get("touch").as_deref() == Some("1"))
}

pub(super) fn open(cx: &mut App, graphics: super::GraphicsProbe) {
    let bounds = Bounds::centered(None, size(px(640.), px(420.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        move |window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            let observation = Rc::new(RefCell::new(Observation {
                graphics,
                gestures: Vec::new(),
                scroll: Vec::new(),
                clicks: 0,
                painted: false,
                overflow: false,
                offset_y: 0.,
            }));
            let recognized = window.intercept_window_mouse_events({
                let observation = observation.clone();
                move |event, _, _| match event {
                    WindowMouseEvent::Scroll(event) => {
                        observation.borrow_mut().record_scroll(event);
                    }
                    WindowMouseEvent::LongPress(event) => {
                        observation.borrow_mut().record_gesture(
                            "longpress",
                            event.phase,
                            event.position,
                        );
                    }
                    WindowMouseEvent::TouchDrag(event) => {
                        observation.borrow_mut().record_gesture(
                            "touchdrag",
                            event.phase,
                            event.position,
                        );
                    }
                    _ => {}
                }
            });
            cx.new(|_| TouchSmoke {
                focus,
                scroll: ScrollHandle::new(),
                observation,
                text: String::new(),
                _subscription: recognized,
            })
        },
    )
    .expect("touch smoke window must open");
    cx.activate(true);
}
