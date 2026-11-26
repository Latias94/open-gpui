use std::{cell::Cell, rc::Rc};

use open_gpui::{
    AccessibleAction, Along, AnyElement, AnyView, App, Axis, Context, CursorStyle, Div, ElementId,
    EventEmitter, FocusHandle, Focusable, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Orientation, Pixels, Point, Refineable, Role, ScrollHandle, SharedString,
    Stateful, StyleRefinement, TargetedEvent, Window, accesskit::ActionData, canvas, div,
    prelude::*, px, relative,
};

use crate::{
    Button,
    numeric::{NUMBER_CONTEXT, NumberCommand, install_keys},
    pointer::{PointerDrag, on_pointer_cancel},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScrollAxes {
    Horizontal,
    #[default]
    Vertical,
    Both,
}

/// A scroll container backed by GPUI's clipping, wheel routing, and scroll handle.
#[derive(IntoElement)]
pub struct ScrollArea(Stateful<Div>);

impl ScrollArea {
    pub fn new(id: impl Into<ElementId>, axes: ScrollAxes) -> Self {
        let container = div().id(id).min_w_0().min_h_0();
        Self(match axes {
            ScrollAxes::Horizontal => container.overflow_x_scroll(),
            ScrollAxes::Vertical => container.overflow_y_scroll(),
            ScrollAxes::Both => container.overflow_scroll(),
        })
    }

    pub fn track_scroll(self, handle: &ScrollHandle) -> Self {
        Self(self.0.track_scroll(handle))
    }
}

impl ParentElement for ScrollArea {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.0.extend(elements);
    }
}

impl Styled for ScrollArea {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}

/// A labeled layout for a set of action controls.
#[derive(IntoElement)]
pub struct Toolbar(Stateful<Div>);

impl Toolbar {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self(
            div()
                .id(id)
                .role(Role::Toolbar)
                .aria_label(label)
                .flex()
                .items_center(),
        )
    }
}

impl ParentElement for Toolbar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.0.extend(elements);
    }
}

impl Styled for Toolbar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}

type ToggleHandler = Box<dyn Fn(bool, &mut Window, &mut App)>;

/// A controlled disclosure. The caller supplies the expanded state and content.
#[derive(IntoElement)]
pub struct Collapsible {
    outer: Stateful<Div>,
    header: Button,
    body: AnyElement,
    expanded: bool,
    toggle: Option<ToggleHandler>,
}

impl Collapsible {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        expanded: bool,
        body: impl IntoElement,
    ) -> Self {
        let label = label.into();
        let header = Button::with_content(
            "disclosure",
            label.clone(),
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(if expanded { "▾" } else { "▸" })
                .child(label),
        )
        .expanded(expanded);
        Self {
            outer: div().id(id).flex().flex_col(),
            header,
            body: body.into_any_element(),
            expanded,
            toggle: None,
        }
    }

    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.toggle = Some(Box::new(handler));
        self
    }

    pub fn header_style(mut self, style: StyleRefinement) -> Self {
        self.header.style().refine(&style);
        self
    }
}

impl Styled for Collapsible {
    fn style(&mut self) -> &mut StyleRefinement {
        self.outer.style()
    }
}

impl RenderOnce for Collapsible {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self {
            outer,
            mut header,
            body,
            expanded,
            toggle,
        } = self;
        if let Some(toggle) = toggle {
            header = header.on_click(move |_, window, cx| toggle(!expanded, window, cx));
        }
        let outer = outer.child(header);
        if expanded { outer.child(body) } else { outer }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitEvent {
    Change(f32),
    Commit(f32),
    Cancel(f32),
}

struct SplitDrag {
    original: f32,
    start: Point<Pixels>,
}

/// Two views separated by a pointer- and keyboard-resizable divider.
pub struct Split {
    first: AnyView,
    second: AnyView,
    axis: Axis,
    ratio: f32,
    minimum: f32,
    maximum: f32,
    divider_width: Pixels,
    extent: Rc<Cell<Pixels>>,
    focus: FocusHandle,
    drag: PointerDrag<SplitDrag>,
    style: StyleRefinement,
    divider_style: StyleRefinement,
}

impl Split {
    pub fn new(
        axis: Axis,
        first: impl Into<AnyView>,
        second: impl Into<AnyView>,
        cx: &mut Context<Self>,
    ) -> Self {
        install_keys(cx);
        Self {
            first: first.into(),
            second: second.into(),
            axis,
            ratio: 0.5,
            minimum: 0.05,
            maximum: 0.95,
            divider_width: px(6.),
            extent: Rc::new(Cell::new(px(0.))),
            focus: cx.focus_handle(),
            drag: PointerDrag::default(),
            style: StyleRefinement::default(),
            divider_style: StyleRefinement::default(),
        }
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    pub fn set_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if ratio.is_finite() {
            self.ratio = ratio.clamp(self.minimum, self.maximum);
            cx.notify();
        }
    }

    pub fn set_limits(&mut self, minimum: f32, maximum: f32, cx: &mut Context<Self>) -> bool {
        if !(0.0..=1.0).contains(&minimum) || !(minimum..=1.0).contains(&maximum) {
            return false;
        }
        self.minimum = minimum;
        self.maximum = maximum;
        self.set_ratio(self.ratio, cx);
        true
    }

    pub fn set_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    pub fn set_divider_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.divider_style = style;
        cx.notify();
    }

    fn change(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if ratio.is_finite() {
            let ratio = ratio.clamp(self.minimum, self.maximum);
            if self.ratio != ratio {
                self.ratio = ratio;
                cx.emit(SplitEvent::Change(ratio));
                cx.notify();
            }
        }
    }

    fn commit(&mut self, ratio: f32, cx: &mut Context<Self>) {
        self.change(ratio, cx);
        cx.emit(SplitEvent::Commit(self.ratio));
    }

    fn pointer_down(
        &mut self,
        event: &TargetedEvent<MouseDownEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.begin(
            SplitDrag {
                original: self.ratio,
                start: event.window_event().position,
            },
            window,
        ) {
            window.focus(&self.focus, cx);
            cx.stop_propagation();
        }
    }

    fn pointer_move(
        &mut self,
        event: &TargetedEvent<MouseMoveEvent>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preview_drag(event, event.window_event().position, cx);
    }

    fn pointer_up(
        &mut self,
        event: &TargetedEvent<MouseUpEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preview_drag(event, event.window_event().position, cx);
        if self.drag.take(window).is_some() {
            cx.emit(SplitEvent::Commit(self.ratio));
        }
    }

    fn preview_drag<E>(
        &mut self,
        event: &TargetedEvent<E>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.drag.state.as_ref() else {
            return;
        };
        let extent = self.extent.get();
        if extent > px(0.)
            && let Ok(vector) = event.target_local_vector(position - drag.start)
        {
            self.change(drag.original + vector.along(self.axis) / extent, cx);
        }
    }

    fn cancel_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take(window) {
            self.ratio = drag.original;
            cx.emit(SplitEvent::Cancel(self.ratio));
            cx.notify();
        }
    }

    fn command(&mut self, command: &NumberCommand, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            NumberCommand::Cancel if self.drag.state.is_some() => self.cancel_drag(window, cx),
            NumberCommand::Step(step) if self.drag.state.is_none() => {
                self.commit(self.ratio + *step as f32 * 0.05, cx)
            }
            NumberCommand::Edge(end) if self.drag.state.is_none() => {
                self.commit(if *end { self.maximum } else { self.minimum }, cx)
            }
            _ => cx.propagate(),
        }
    }
}

impl Focusable for Split {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<SplitEvent> for Split {}

impl Render for Split {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.drag.bind(window);
        let mut root = div().relative().flex().size_full().min_w_0().min_h_0();
        let first = div()
            .flex_none()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(self.first.clone());
        let second = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(self.second.clone());
        let divider = div().absolute().bg(window.text_style().color.opacity(0.1));
        let (first, mut divider, orientation) = match self.axis {
            Axis::Horizontal => (
                first.w(relative(self.ratio)).h_full(),
                divider
                    .left(relative(self.ratio))
                    .top_0()
                    .ml(-self.divider_width / 2.)
                    .w(self.divider_width)
                    .h_full()
                    .cursor(CursorStyle::ResizeLeftRight),
                Orientation::Vertical,
            ),
            Axis::Vertical => {
                root = root.flex_col();
                (
                    first.h(relative(self.ratio)).w_full(),
                    divider
                        .top(relative(self.ratio))
                        .left_0()
                        .mt(-self.divider_width / 2.)
                        .h(self.divider_width)
                        .w_full()
                        .cursor(CursorStyle::ResizeUpDown),
                    Orientation::Horizontal,
                )
            }
        };
        root.style().refine(&self.style);
        divider.style().refine(&self.divider_style);
        let view = cx.weak_entity();
        let mut divider = divider
            .id("split-divider")
            .role(Role::Splitter)
            .aria_label("Pane divider")
            .aria_orientation(orientation)
            .aria_numeric_value(f64::from(self.ratio))
            .aria_min_numeric_value(f64::from(self.minimum))
            .aria_max_numeric_value(f64::from(self.maximum))
            .track_focus(&self.focus)
            .tab_stop(true)
            .track_pointer_capture(&handle)
            .key_context(NUMBER_CONTEXT)
            .on_action(cx.listener(Self::command))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::pointer_down))
            .on_mouse_move(cx.listener(Self::pointer_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_a11y_action(AccessibleAction::SetValue, move |data, _, cx| {
                if let Some(ActionData::NumericValue(value)) = data {
                    let _ = view.update(cx, |split, cx| split.commit(*value as f32, cx));
                }
            });
        for (action, command) in [
            (AccessibleAction::Increment, NumberCommand::Step(1.)),
            (AccessibleAction::Decrement, NumberCommand::Step(-1.)),
        ] {
            let view = cx.weak_entity();
            divider = divider.on_a11y_action(action, move |_, window, cx| {
                let _ = view.update(cx, |split, cx| split.command(&command, window, cx));
            });
        }
        let extent = self.extent.clone();
        let axis = self.axis;
        root.child(first)
            .child(second)
            .child(divider)
            .child(
                canvas(
                    move |bounds, _, _| extent.set(bounds.size.along(axis)),
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(on_pointer_cancel(cx, Self::cancel_drag))
    }
}
