//! Switch component.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, Toggled};

use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::theme::ThemeResolver;

/// Resolved switch metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchMetrics {
    track_width: open_gpui::Pixels,
    track_height: open_gpui::Pixels,
    thumb_size: open_gpui::Pixels,
    thumb_offset: open_gpui::Pixels,
    label_text_size: open_gpui::Pixels,
}

impl SwitchMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        let (track_width, track_height, thumb_size) = match size {
            Size::XSmall => (px(28.0), px(16.0), px(12.0)),
            Size::Small => (px(32.0), px(18.0), px(14.0)),
            Size::Medium => (px(36.0), px(20.0), px(16.0)),
            Size::Large => (px(40.0), px(22.0), px(18.0)),
        };

        Self {
            track_width,
            track_height,
            thumb_size,
            thumb_offset: px(2.0),
            label_text_size: size.control_text_px(),
        }
    }

    /// Returns the track width.
    pub const fn track_width(self) -> open_gpui::Pixels {
        self.track_width
    }

    /// Returns the track height.
    pub const fn track_height(self) -> open_gpui::Pixels {
        self.track_height
    }

    /// Returns the thumb size.
    pub const fn thumb_size(self) -> open_gpui::Pixels {
        self.thumb_size
    }

    /// Returns the thumb offset inside the track.
    pub const fn thumb_offset(self) -> open_gpui::Pixels {
        self.thumb_offset
    }

    /// Returns the checked thumb x position.
    pub fn checked_thumb_x(self) -> open_gpui::Pixels {
        self.track_width - self.thumb_size - self.thumb_offset
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> open_gpui::Pixels {
        self.label_text_size
    }
}

/// Resolved switch color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchColors {
    pub(crate) track: ColorIntent,
    pub(crate) thumb: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) label: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl SwitchColors {
    /// Returns the track color intent.
    pub const fn track(self) -> ColorIntent {
        self.track
    }

    /// Returns the thumb color intent.
    pub const fn thumb(self) -> ColorIntent {
        self.thumb
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns the label color intent.
    pub const fn label(self) -> ColorIntent {
        self.label
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved switch state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchState {
    checked: bool,
    disabled: bool,
    size: Size,
    metrics: SwitchMetrics,
    colors: SwitchColors,
    focus_ring: FocusRing,
}

impl SwitchState {
    /// Resolves the public state for a switch.
    pub fn resolve(checked: bool, disabled: bool, size: Size, tokens: ThemeTokens) -> Self {
        let colors = ThemeResolver::switch_colors(tokens, checked);

        Self {
            checked,
            disabled,
            size,
            metrics: SwitchMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns whether the switch is checked.
    pub const fn checked(self) -> bool {
        self.checked
    }

    /// Returns whether the switch is disabled.
    pub const fn disabled(self) -> bool {
        self.disabled
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(self) -> Role {
        Role::Switch
    }

    /// Returns the toggled accessibility state.
    pub const fn toggled(self) -> Toggled {
        if self.checked {
            Toggled::True
        } else {
            Toggled::False
        }
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> SwitchMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> SwitchColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI switch component.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    label: Option<SharedString>,
    checked: bool,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    on_click: Option<Rc<dyn Fn(bool, &ClickEvent, &mut Window, &mut App)>>,
}

impl Switch {
    /// Creates a new switch with an id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            checked: false,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            on_click: None,
        }
    }

    /// Sets the visible label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the checked state.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Marks the switch as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a click handler with the next checked value.
    pub fn on_click(
        mut self,
        handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved switch state.
    pub fn state(&self) -> SwitchState {
        SwitchState::resolve(self.checked, self.disabled, self.size, self.tokens)
    }
}

impl Sizable for Switch {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let next_checked = !state.checked();
        let label = self.label.clone();

        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .focusable()
            .tab_stop(!disabled)
            .role(state.role())
            .aria_label(
                label
                    .clone()
                    .unwrap_or_else(|| SharedString::from("Switch")),
            )
            .aria_toggled(state.toggled())
            .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| this.cursor_pointer())
            .when_some(
                self.on_click.filter(|_| !disabled),
                move |this, on_click| {
                    this.on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        on_click(next_checked, event, window, cx);
                    })
                },
            )
            .child(
                div()
                    .relative()
                    .w(metrics.track_width())
                    .h(metrics.track_height())
                    .rounded(metrics.track_height())
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.border()))
                    .bg(ThemeResolver::resolve(colors.track()))
                    .child(
                        div()
                            .absolute()
                            .left(if state.checked() {
                                metrics.checked_thumb_x()
                            } else {
                                metrics.thumb_offset()
                            })
                            .top(metrics.thumb_offset())
                            .w(metrics.thumb_size())
                            .h(metrics.thumb_size())
                            .rounded(metrics.thumb_size())
                            .bg(ThemeResolver::resolve(colors.thumb()))
                            .shadow_sm(),
                    ),
            )
            .when_some(label, |this, label| {
                this.child(
                    div()
                        .text_size(metrics.label_text_size())
                        .line_height(metrics.track_height())
                        .text_color(ThemeResolver::resolve(colors.label()))
                        .child(label),
                )
            })
    }
}
