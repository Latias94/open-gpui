use std::time::Duration;

use open_gpui::{
    AnyView, App, Context, Div, ElementId, EventEmitter, FocusHandle, Global, KeyBinding,
    MouseButton, Refineable, Role, SharedString, Stateful, StyleRefinement, Window, anchored, div,
    point, prelude::*, px, window_portal,
};

mod select;

pub use select::{Menu, MenuEvent, MenuItem, Select};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalEvent {
    Dismissed,
}

/// A window-level dialog with a local tab cycle and focus restoration.
pub struct Modal {
    body: AnyView,
    label: SharedString,
    focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    open: bool,
    dismiss_on_backdrop: bool,
    panel_style: StyleRefinement,
    backdrop_style: StyleRefinement,
}

impl Modal {
    pub fn new(
        label: impl Into<SharedString>,
        body: impl Into<AnyView>,
        cx: &mut Context<Self>,
    ) -> Self {
        install_keys(cx);
        Self {
            body: body.into(),
            label: label.into(),
            focus: cx.focus_handle(),
            previous_focus: None,
            open: false,
            dismiss_on_backdrop: true,
            panel_style: StyleRefinement::default(),
            backdrop_style: StyleRefinement::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }
        self.open = open;
        cx.notify();
        if open {
            self.previous_focus = window.focused(cx);
            window.focus(&self.focus, cx);
        } else if let Some(previous) = self
            .previous_focus
            .take()
            .filter(|handle| window.is_focus_handle_rendered(handle))
        {
            window.focus(&previous, cx);
        } else {
            window.blur(cx);
        }
    }

    pub fn set_dismiss_on_backdrop(&mut self, dismiss: bool) {
        self.dismiss_on_backdrop = dismiss;
    }

    pub fn set_panel_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.panel_style = style;
        cx.notify();
    }

    pub fn set_backdrop_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.backdrop_style = style;
        cx.notify();
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(false, window, cx);
        cx.emit(ModalEvent::Dismissed);
    }

    fn command(&mut self, command: &PopupCommand, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            PopupCommand::Escape => self.dismiss(window, cx),
            PopupCommand::Tab { backward: true } => {
                window.focus_prev_within(&self.focus, cx);
            }
            PopupCommand::Tab { backward: false } => {
                window.focus_next_within(&self.focus, cx);
            }
            _ => cx.propagate(),
        }
    }
}

impl EventEmitter<ModalEvent> for Modal {}

impl Render for Modal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let viewport = window.viewport_size();
        let mut backdrop = div().absolute().top_0().left_0().size_full();
        backdrop.style().refine(&self.backdrop_style);
        let backdrop = backdrop.id("modal-backdrop").occlude().on_mouse_down(
            MouseButton::Left,
            cx.listener(|modal, _, window, cx| {
                if modal.dismiss_on_backdrop {
                    modal.dismiss(window, cx);
                }
            }),
        );
        let mut body = div().max_w_full().max_h_full().overflow_hidden();
        body.style().refine(&self.panel_style);
        let body = body
            .id("modal")
            .role(Role::Dialog)
            .aria_modal(true)
            .aria_label(self.label.clone())
            .occlude()
            .track_focus(&self.focus)
            .tab_group()
            .key_context(MODAL_CONTEXT)
            .on_action(cx.listener(Self::command))
            .child(self.body.clone());
        window_portal(
            anchored()
                .position(point(px(0.), px(0.)))
                .snap_to_window()
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(viewport.width)
                        .h(viewport.height)
                        .child(backdrop)
                        .child(body),
                ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

/// Attaches a label to GPUI's native tooltip timing and positioning.
#[derive(IntoElement)]
pub struct Tooltip {
    trigger: Stateful<Div>,
    label: SharedString,
    content_style: StyleRefinement,
    delay: Option<Duration>,
}

impl Tooltip {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        trigger: impl IntoElement,
    ) -> Self {
        Self {
            trigger: div().id(id).child(trigger),
            label: label.into(),
            content_style: StyleRefinement::default(),
            delay: None,
        }
    }

    pub fn content_style(mut self, style: StyleRefinement) -> Self {
        self.content_style = style;
        self
    }

    pub fn show_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

impl Styled for Tooltip {
    fn style(&mut self) -> &mut StyleRefinement {
        self.trigger.style()
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self {
            mut trigger,
            label,
            content_style,
            delay,
        } = self;
        if let Some(delay) = delay {
            trigger = trigger.tooltip_show_delay(delay);
        }
        trigger.tooltip(move |_, cx| {
            cx.new(|_| TooltipView(label.clone(), content_style.clone()))
                .into()
        })
    }
}

struct TooltipView(SharedString, StyleRefinement);

impl Render for TooltipView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div();
        root.style().refine(&self.1);
        root.id("tooltip").role(Role::Tooltip).child(self.0.clone())
    }
}

const SELECT_CONTEXT: &str = "OpenGpuiBaseSelect";
const MODAL_CONTEXT: &str = "OpenGpuiBaseModal";

#[derive(Clone, Default, PartialEq, open_gpui::Action)]
#[action(namespace = open_gpui_base, no_json, no_register)]
enum PopupCommand {
    Step(bool),
    Edge(bool),
    Tab {
        backward: bool,
    },
    #[default]
    Enter,
    Escape,
}

struct PopupKeysInstalled;
impl Global for PopupKeysInstalled {}

fn install_keys(cx: &mut App) {
    if cx.has_global::<PopupKeysInstalled>() {
        return;
    }
    cx.set_global(PopupKeysInstalled);
    let tab = [
        ("tab", PopupCommand::Tab { backward: false }),
        ("shift-tab", PopupCommand::Tab { backward: true }),
    ];
    let modal = tab
        .clone()
        .into_iter()
        .chain([("escape", PopupCommand::Escape)])
        .map(|(key, action)| KeyBinding::new(key, action, Some(MODAL_CONTEXT)));
    let select = tab
        .into_iter()
        .chain([
            ("up", PopupCommand::Step(false)),
            ("down", PopupCommand::Step(true)),
            ("home", PopupCommand::Edge(false)),
            ("end", PopupCommand::Edge(true)),
            ("enter", PopupCommand::Enter),
            ("space", PopupCommand::Enter),
            ("escape", PopupCommand::Escape),
        ])
        .map(|(key, action)| KeyBinding::new(key, action, Some(SELECT_CONTEXT)));
    cx.bind_keys(modal.chain(select));
}
