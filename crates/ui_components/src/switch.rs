//! Switch component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;

/// Resolved switch metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchMetrics {
    track_width: UiPx,
    track_height: UiPx,
    thumb_size: UiPx,
    thumb_offset: UiPx,
    label_text_size: UiPx,
}

impl SwitchMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        let (track_width, track_height, thumb_size) = match size {
            Size::XSmall => (ui_px(28.0), ui_px(16.0), ui_px(12.0)),
            Size::Small => (ui_px(32.0), ui_px(18.0), ui_px(14.0)),
            Size::Medium => (ui_px(36.0), ui_px(20.0), ui_px(16.0)),
            Size::Large => (ui_px(40.0), ui_px(22.0), ui_px(18.0)),
        };

        Self {
            track_width,
            track_height,
            thumb_size,
            thumb_offset: ui_px(2.0),
            label_text_size: size.control_text_px(),
        }
    }

    /// Returns the track width.
    pub const fn track_width(self) -> UiPx {
        self.track_width
    }

    /// Returns the track height.
    pub const fn track_height(self) -> UiPx {
        self.track_height
    }

    /// Returns the thumb size.
    pub const fn thumb_size(self) -> UiPx {
        self.thumb_size
    }

    /// Returns the thumb offset inside the track.
    pub const fn thumb_offset(self) -> UiPx {
        self.thumb_offset
    }

    /// Returns the checked thumb x position.
    pub fn checked_thumb_x(self) -> UiPx {
        self.track_width - self.thumb_size - self.thumb_offset
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> UiPx {
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
    on_change: Option<Rc<dyn Fn(bool, &ClickEvent, &mut Window, &mut App)>>,
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
            on_change: None,
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

    /// Registers a change handler with the next checked value.
    pub fn on_change(
        mut self,
        handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let next_checked = !state.checked();
        let label = self.label.clone();
        let debug_id = self.id.to_string();
        let theme_context = ThemeResolver::current(cx);
        let theme = &theme_context;
        let border_color = theme.resolve(colors.border());
        let track_color = theme.resolve(colors.track());
        let thumb_color = theme.resolve(colors.thumb());
        let label_color = theme.resolve(colors.label());
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, theme);

        div()
            .id(self.id)
            .debug_selector(move || format!("switch:{debug_id}:root"))
            .flex()
            .items_center()
            .gap_2()
            .focusable()
            .tab_stop(!disabled)
            .ui_role(state.role())
            .aria_label(
                label
                    .clone()
                    .unwrap_or_else(|| SharedString::from("Switch")),
            )
            .ui_aria_toggled(state.toggled())
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| this.cursor_pointer())
            .when_some(
                self.on_change.filter(|_| !disabled),
                move |this, on_change| {
                    this.on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        on_change(next_checked, event, window, cx);
                    })
                },
            )
            .child(
                div()
                    .relative()
                    .w(gpui_px_from_ui(metrics.track_width()))
                    .h(gpui_px_from_ui(metrics.track_height()))
                    .rounded(gpui_px_from_ui(metrics.track_height()))
                    .border_1()
                    .border_color(border_color)
                    .bg(track_color)
                    .child(
                        div()
                            .absolute()
                            .left(gpui_px_from_ui(if state.checked() {
                                metrics.checked_thumb_x()
                            } else {
                                metrics.thumb_offset()
                            }))
                            .top(gpui_px_from_ui(metrics.thumb_offset()))
                            .w(gpui_px_from_ui(metrics.thumb_size()))
                            .h(gpui_px_from_ui(metrics.thumb_size()))
                            .rounded(gpui_px_from_ui(metrics.thumb_size()))
                            .bg(thumb_color)
                            .shadow_sm(),
                    ),
            )
            .when_some(label, |this, label| {
                this.child(
                    div()
                        .text_size(gpui_px_from_ui(metrics.label_text_size()))
                        .line_height(gpui_px_from_ui(metrics.track_height()))
                        .text_color(label_color)
                        .child(label),
                )
            })
    }
}
