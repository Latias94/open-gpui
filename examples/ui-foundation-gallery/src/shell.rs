//! Gallery shell that consumes the UI foundation directly.

use open_gpui::prelude::*;
use open_gpui::{
    AccessibleAction, Anchor, App, AppContext, Bounds, Context, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Pixels, Render, Role, ScrollHandle,
    StatefulInteractiveElement, Styled, Toggled, Window, WindowBounds, WindowOptions, anchored,
    deferred, div, point, px, rgb, size,
};
use open_gpui_ui_components::{
    Button, ButtonState, ColorIntent, Field, FieldState, FocusRing, Switch, SwitchState, TextInput,
    TextInputState, focus_ring_shadow,
};
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceAdaptivePolicy, DeviceShellMode, DeviceShellSwitchPolicy,
    PanelAdaptiveClass, Rect, Sizable, Size, ThemeTokens,
};

use crate::pages::{self, GALLERY_SECTIONS, GalleryPage};

/// Default gallery window width.
pub const DEFAULT_GALLERY_WIDTH: Pixels = px(1040.0);
/// Default gallery window height.
pub const DEFAULT_GALLERY_HEIGHT: Pixels = px(680.0);
/// Compact gallery width used by the manual adaptive switch.
pub const COMPACT_GALLERY_WIDTH: Pixels = px(720.0);
/// Desktop gallery width used by the manual adaptive switch.
pub const DESKTOP_GALLERY_WIDTH: Pixels = DEFAULT_GALLERY_WIDTH;

/// Derived foundation state shown by the gallery shell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GalleryShellSnapshot {
    /// The selected gallery page.
    pub selected_page: GalleryPage,
    /// The width currently used for foundation classification.
    pub viewport_width: Pixels,
    /// The binary shell mode derived from the foundation switch policy.
    pub shell_mode: DeviceShellMode,
    /// Density derived from the device adaptive class.
    pub density: Density,
    /// The default size chosen by the derived density.
    pub control_size: Size,
    /// The default token bundle consumed by the shell.
    pub tokens: ThemeTokens,
}

/// Returns the foundation snapshot for a gallery viewport width.
pub fn foundation_snapshot(width: Pixels, selected_page: GalleryPage) -> GalleryShellSnapshot {
    let shell_mode = DeviceShellSwitchPolicy::default().mode(width);
    let density = DeviceAdaptivePolicy::default().classify(width).density();

    GalleryShellSnapshot {
        selected_page,
        viewport_width: width,
        shell_mode,
        density,
        control_size: density.default_size(),
        tokens: ThemeTokens::default(),
    }
}

/// Top-level gallery view.
#[derive(Debug)]
pub struct GalleryShell {
    selected_page: GalleryPage,
    width: Pixels,
    navigation_scroll: ScrollHandle,
    page_scroll: ScrollHandle,
    root_focus: FocusHandle,
    focus_controls: [FocusHandle; 3],
    focus_message: &'static str,
    a11y_counter: i32,
    a11y_enabled: bool,
    overlay_open: bool,
}

impl GalleryShell {
    fn build(cx: &mut Context<Self>) -> Self {
        Self {
            selected_page: GalleryPage::Tokens,
            width: DEFAULT_GALLERY_WIDTH,
            navigation_scroll: ScrollHandle::new(),
            page_scroll: ScrollHandle::new(),
            root_focus: cx.focus_handle(),
            focus_controls: [
                cx.focus_handle().tab_index(1).tab_stop(true),
                cx.focus_handle().tab_index(2).tab_stop(true),
                cx.focus_handle().tab_index(3).tab_stop(true),
            ],
            focus_message: "Ready for keyboard focus.",
            a11y_counter: 0,
            a11y_enabled: false,
            overlay_open: false,
        }
    }
}

impl GalleryShell {
    /// Creates a gallery shell entity.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::build(cx)
    }

    /// Returns the currently selected page.
    pub const fn selected_page(&self) -> GalleryPage {
        self.selected_page
    }

    /// Returns the current foundation snapshot.
    pub fn snapshot(&self) -> GalleryShellSnapshot {
        foundation_snapshot(self.width, self.selected_page)
    }

    fn select_page(&mut self, page: GalleryPage, cx: &mut Context<Self>) {
        if self.selected_page != page {
            self.selected_page = page;
            self.page_scroll.set_offset(point(px(0.0), px(0.0)));
            cx.notify();
        }
    }

    fn set_viewport_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.width != width {
            self.width = width;
            cx.notify();
        }
    }

    fn set_focus_message(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.focus_message = message;
        cx.notify();
    }

    fn increment_a11y_counter(&mut self, cx: &mut Context<Self>) {
        self.a11y_counter += 1;
        cx.notify();
    }

    fn decrement_a11y_counter(&mut self, cx: &mut Context<Self>) {
        self.a11y_counter = (self.a11y_counter - 1).max(0);
        cx.notify();
    }

    fn reset_a11y_counter(&mut self, cx: &mut Context<Self>) {
        self.a11y_counter = 0;
        cx.notify();
    }

    fn toggle_a11y_enabled(&mut self, cx: &mut Context<Self>) {
        self.a11y_enabled = !self.a11y_enabled;
        cx.notify();
    }

    fn set_overlay_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.overlay_open != open {
            self.overlay_open = open;
            cx.notify();
        }
    }
}

impl Render for GalleryShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.snapshot();
        let page = snapshot.selected_page;

        div()
            .id("ui-foundation-gallery")
            .size_full()
            .flex()
            .bg(rgb(0xf6f7f2))
            .text_color(rgb(0x18202a))
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    this.set_overlay_open(false, cx);
                }
            }))
            .child(self.render_navigation(page, cx))
            .child(self.render_content(snapshot, cx))
    }
}

impl GalleryShell {
    fn render_navigation(
        &self,
        selected_page: GalleryPage,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("gallery-navigation")
            .w(px(268.0))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_hidden()
            .border_r_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_4()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("UI Foundation"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.0))
                            .text_color(rgb(0x5a6472))
                            .child("Pure foundation consumer for Open GPUI UI core."),
                    ),
            )
            .child(
                div()
                    .id("gallery-navigation-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_y_scroll()
                    .track_scroll(&self.navigation_scroll)
                    .children(GALLERY_SECTIONS.into_iter().map(|section| {
                        let selected = section.page == selected_page;
                        div()
                            .id(section.id)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(if selected {
                                rgb(0x1f7a66)
                            } else {
                                rgb(0xe1e4da)
                            })
                            .bg(if selected {
                                rgb(0xe8f3ef)
                            } else {
                                rgb(0xffffff)
                            })
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0xf1f5ee)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_page(section.page, cx);
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(section.title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(px(18.0))
                                    .text_color(rgb(0x5a6472))
                                    .child(section.summary),
                            )
                    })),
            )
    }

    fn render_content(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let page = snapshot.selected_page;

        div()
            .id("gallery-content")
            .flex_1()
            .min_w(px(0.0))
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_hidden()
            .p_5()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(page.title()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(px(20.0))
                                    .text_color(rgb(0x4d5968))
                                    .child(page.summary()),
                            ),
                    )
                    .child(self.render_snapshot_summary(snapshot, cx)),
            )
            .child(
                div()
                    .id("gallery-page-scroll")
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.page_scroll)
                    .child(self.render_page_body(snapshot, cx)),
            )
    }

    fn render_page_body(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match snapshot.selected_page {
            GalleryPage::Tokens => self.render_tokens_page(snapshot).into_any_element(),
            GalleryPage::SizingDensity => self.render_sizing_page(snapshot).into_any_element(),
            GalleryPage::Adaptive => self.render_adaptive_page(snapshot).into_any_element(),
            GalleryPage::FocusAccessibility => {
                self.render_focus_a11y_page(snapshot, cx).into_any_element()
            }
            GalleryPage::Overlay => self.render_overlay_page(snapshot, cx).into_any_element(),
            GalleryPage::Components => self.render_components_page(snapshot).into_any_element(),
        }
    }

    fn render_tokens_page(&self, snapshot: GalleryShellSnapshot) -> impl IntoElement {
        let registry_status = if pages::tokens::matches_semantic_registry(snapshot.tokens) {
            "semantic registry aligned"
        } else {
            "custom token bundle"
        };

        div()
            .id("gallery-tokens-page")
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x4d5968))
                    .child(registry_status),
            )
            .child(
                div().flex().gap_3().children(
                    pages::tokens::theme_mode_samples(snapshot.tokens)
                        .into_iter()
                        .map(|sample| {
                            div()
                                .id(format!("theme-mode:{}", sample.mode.as_str()))
                                .min_w(px(180.0))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .size(px(16.0))
                                                .rounded_sm()
                                                .border_1()
                                                .border_color(rgb(0xc8ccbf))
                                                .bg(rgb(sample.surface_rgb)),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(open_gpui::FontWeight::BOLD)
                                                .child(sample.mode.as_str()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(format!("rev {}", sample.revision)),
                                )
                                .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                                    "surface {:06x} text {:06x} accent {:06x} focus {:06x}",
                                    sample.surface_rgb,
                                    sample.text_rgb,
                                    sample.accent_rgb,
                                    sample.focus_ring_rgb
                                )))
                        }),
                ),
            )
            .child(
                div().grid().grid_cols(3).gap_3().children(
                    pages::tokens::token_samples(snapshot.tokens)
                        .into_iter()
                        .map(|sample| {
                            div()
                                .id(format!("token-sample:{}", sample.key.as_str()))
                                .min_h(px(92.0))
                                .flex()
                                .flex_col()
                                .justify_between()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .size(px(16.0))
                                                .rounded_sm()
                                                .border_1()
                                                .border_color(rgb(0xc8ccbf))
                                                .bg(rgb(sample.preview_rgb)),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(open_gpui::FontWeight::BOLD)
                                                .child(sample.label),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(sample.key.to_string()),
                                )
                        }),
                ),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_components_page(&self, snapshot: GalleryShellSnapshot) -> impl IntoElement {
        div()
            .id("gallery-components-page")
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Button"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::button_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state;
                                    div()
                                        .id(format!("component-button-sample:{}", sample.id))
                                        .min_w(px(180.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(
                                            Button::new(
                                                format!("component-button:{}", sample.id),
                                                sample.label,
                                            )
                                            .variant(state.variant())
                                            .with_size(state.size())
                                            .disabled(state.disabled())
                                            .selected(state.selected())
                                            .tokens(snapshot.tokens),
                                        )
                                        .child(component_button_state_row(state))
                                }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Switch"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::switch_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state;
                                    div()
                                        .id(format!("component-switch-sample:{}", sample.id))
                                        .min_w(px(200.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(
                                            Switch::new(format!("component-switch:{}", sample.id))
                                                .label(sample.label)
                                                .checked(state.checked())
                                                .disabled(state.disabled())
                                                .with_size(state.size())
                                                .tokens(snapshot.tokens),
                                        )
                                        .child(component_switch_state_row(state))
                                }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("TextInput"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::text_input_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state.clone();
                                    div()
                                        .id(format!("component-text-input-sample:{}", sample.id))
                                        .min_w(px(240.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(open_gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x3f4a57))
                                                .child(sample.label),
                                        )
                                        .child(component_text_input(
                                            format!("component-text-input:{}", sample.id),
                                            sample.label,
                                            &state,
                                            snapshot.tokens,
                                        ))
                                        .child(component_text_input_state_row(&state))
                                }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Field"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::field_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let field_state = sample.state.clone();
                                    let input_state = sample.input_state.clone();
                                    div()
                                        .id(format!("component-field-sample:{}", sample.id))
                                        .min_w(px(280.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(component_field(
                                            format!("component-field:{}", sample.id),
                                            &field_state,
                                            component_text_input(
                                                format!("component-field-input:{}", sample.id),
                                                field_state.label(),
                                                &input_state,
                                                snapshot.tokens,
                                            ),
                                            snapshot.tokens,
                                        ))
                                        .child(component_field_state_row(
                                            &field_state,
                                            &input_state,
                                        ))
                                }),
                        ),
                    ),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_sizing_page(&self, snapshot: GalleryShellSnapshot) -> impl IntoElement {
        div()
            .id("gallery-sizing-page")
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Size scale"),
                    )
                    .children(pages::sizing::SIZE_SAMPLES.into_iter().map(|sample| {
                        div()
                            .id(format!("size-sample:{}", sample.label))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .rounded_sm()
                            .border_1()
                            .border_color(if sample.size == snapshot.control_size {
                                rgb(0x1f7a66)
                            } else {
                                rgb(0xd6d8ce)
                            })
                            .bg(rgb(0xffffff))
                            .px_4()
                            .py_2()
                            .child(
                                div()
                                    .w(px(92.0))
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(sample.label),
                            )
                            .child(self.render_metric("button", sample.button_h))
                            .child(self.render_metric("input", sample.input_h))
                            .child(self.render_metric("icon", sample.icon_button_size))
                            .child(self.render_metric("radius", sample.radius))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Density defaults"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        pages::sizing::DENSITY_SAMPLES.into_iter().map(|sample| {
                            div()
                                .id(format!("density-sample:{}", sample.label))
                                .min_w(px(180.0))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .rounded_sm()
                                .border_1()
                                .border_color(if sample.density == snapshot.density {
                                    rgb(0x1f7a66)
                                } else {
                                    rgb(0xd6d8ce)
                                })
                                .bg(if sample.density == snapshot.density {
                                    rgb(0xe8f3ef)
                                } else {
                                    rgb(0xffffff)
                                })
                                .p_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(open_gpui::FontWeight::BOLD)
                                        .child(sample.label),
                                )
                                .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                                    "default size: {}",
                                    size_label(sample.default_size)
                                )))
                        }),
                    )),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_adaptive_page(&self, snapshot: GalleryShellSnapshot) -> impl IntoElement {
        div()
            .id("gallery-adaptive-page")
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Device samples"),
                    )
                    .children(pages::adaptive::device_samples().into_iter().map(|sample| {
                        div()
                            .id(format!("device-sample:{:.0}", sample.width.as_f32()))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .rounded_sm()
                            .border_1()
                            .border_color(if sample.width == snapshot.viewport_width {
                                rgb(0x1f7a66)
                            } else {
                                rgb(0xd6d8ce)
                            })
                            .bg(if sample.width == snapshot.viewport_width {
                                rgb(0xe8f3ef)
                            } else {
                                rgb(0xffffff)
                            })
                            .px_4()
                            .py_2()
                            .text_sm()
                            .text_color(rgb(0x263240))
                            .child(
                                div()
                                    .w(px(88.0))
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(format_px(sample.width)),
                            )
                            .child(label_pill(shell_mode_label(sample.shell_mode)))
                            .child(label_pill(device_class_label(sample.class)))
                            .child(label_pill(density_label(sample.density)))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Panel samples"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        pages::adaptive::panel_samples().into_iter().map(|sample| {
                            div()
                                .id(format!("panel-sample:{:.0}", sample.width.as_f32()))
                                .min_w(px(180.0))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(open_gpui::FontWeight::BOLD)
                                        .child(format_px(sample.width)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(panel_class_label(sample.class)),
                                )
                        }),
                    )),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_focus_a11y_page(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let a11y = pages::focus_a11y::a11y_demo_state(self.a11y_counter, self.a11y_enabled);
        let entity = cx.entity().downgrade();

        div()
            .id("gallery-focus-a11y-page")
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .flex_wrap()
                    .child(self.render_focus_control(0, pages::focus_a11y::FOCUS_CONTROLS[0], cx))
                    .child(self.render_focus_control(1, pages::focus_a11y::FOCUS_CONTROLS[1], cx))
                    .child(self.render_focus_control(2, pages::focus_a11y::FOCUS_CONTROLS[2], cx)),
            )
            .child(
                div()
                    .id("gallery-a11y-state")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xcfd5cc))
                    .bg(rgb(0xffffff))
                    .p_4()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Accessibility state"),
                    )
                    .child(
                        div()
                            .id("gallery-a11y-counter")
                            .focusable()
                            .tab_stop(true)
                            .role(Role::SpinButton)
                            .aria_label(format!("Counter {}", a11y.counter))
                            .aria_numeric_value(a11y.counter as f64)
                            .aria_min_numeric_value(0.0)
                            .on_a11y_action(AccessibleAction::Increment, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity
                                        .update(cx, |this, cx| this.increment_a11y_counter(cx))
                                        .ok();
                                }
                            })
                            .on_a11y_action(AccessibleAction::Decrement, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity
                                        .update(cx, |this, cx| this.decrement_a11y_counter(cx))
                                        .ok();
                                }
                            })
                            .px_3()
                            .py_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .bg(rgb(0xf6f7f2))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.increment_a11y_counter(cx);
                            }))
                            .child(format!("counter: {}", a11y.counter)),
                    )
                    .child(
                        div()
                            .id("gallery-a11y-reset")
                            .focusable()
                            .tab_stop(true)
                            .role(Role::Button)
                            .aria_label("Reset counter")
                            .px_3()
                            .py_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .bg(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0xf1f5ee)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reset_a11y_counter(cx);
                            }))
                            .child("reset counter"),
                    )
                    .child(
                        div()
                            .id("gallery-a11y-switch")
                            .focusable()
                            .tab_stop(true)
                            .role(Role::Switch)
                            .aria_label("Enable foundation switch")
                            .aria_toggled(a11y.toggled)
                            .w(px(224.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded_sm()
                            .border_1()
                            .border_color(if self.a11y_enabled {
                                rgb(0x1f7a66)
                            } else {
                                rgb(0xd6d8ce)
                            })
                            .bg(if self.a11y_enabled {
                                rgb(0xe8f3ef)
                            } else {
                                rgb(0xffffff)
                            })
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_a11y_enabled(cx);
                            }))
                            .child("feature switch")
                            .child(toggled_label(a11y.toggled)),
                    ),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xd6d8ce))
                    .bg(rgb(0xffffff))
                    .p_3()
                    .text_sm()
                    .line_height(px(20.0))
                    .text_color(rgb(0x4d5968))
                    .child(self.focus_message),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_focus_control(
        &self,
        index: usize,
        spec: pages::focus_a11y::FocusControlSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let handle = &self.focus_controls[index];
        let focus_ring = FocusRing::from_color(ColorIntent::new(
            ThemeTokens::default().focus_ring,
            0x2f80ed,
        ));

        div()
            .id(spec.id)
            .min_w(px(180.0))
            .flex()
            .flex_col()
            .gap_2()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
            .track_focus(handle)
            .focusable()
            .tab_stop(true)
            .role(spec.role)
            .aria_label(spec.label)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0xf1f5ee)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_focus_message(spec.label, cx);
            }))
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(spec.label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x5a6472))
                    .child(format!("tab index: {}", spec.tab_index)),
            )
    }

    fn render_overlay_page(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let geometry = pages::overlay::demo_geometry();

        div()
            .id("gallery-overlay-page")
            .relative()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_4()
                    .child(
                        div()
                            .id("gallery-overlay-stage")
                            .relative()
                            .w(px(640.0))
                            .h(px(360.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xcfd5cc))
                            .bg(rgb(0xffffff))
                            .child(
                                self.render_overlay_bounds(
                                    "safe window",
                                    geometry.safe_window_rect,
                                ),
                            )
                            .child(self.render_overlay_bounds("visual rect", geometry.visual_rect))
                            .child(
                                div()
                                    .id("gallery-overlay-trigger")
                                    .absolute()
                                    .left(geometry.trigger_point.x)
                                    .top(geometry.trigger_point.y)
                                    .w(px(176.0))
                                    .h(px(40.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0x1f7a66))
                                    .bg(rgb(0xe8f3ef))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_overlay_open(true, cx);
                                    }))
                                    .child("open overlay")
                                    .when(self.overlay_open, |trigger| {
                                        trigger.child(
                                            deferred(
                                                anchored()
                                                    .anchor(Anchor::TopLeft)
                                                    .position(geometry.anchor_rect.origin)
                                                    .snap_to_window_with_margin(px(12.0))
                                                    .child(
                                                        div()
                                                            .id("gallery-overlay-popover")
                                                            .w(px(240.0))
                                                            .flex()
                                                            .flex_col()
                                                            .gap_2()
                                                            .rounded_sm()
                                                            .border_1()
                                                            .border_color(rgb(0x1f7a66))
                                                            .bg(rgb(0xffffff))
                                                            .shadow_lg()
                                                            .p_3()
                                                            .text_sm()
                                                            .child("Anchored overlay")
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x5a6472))
                                                                    .child(format!(
                                                                        "anchor: {} x {}",
                                                                        format_px(
                                                                            geometry
                                                                                .anchor_rect
                                                                                .size
                                                                                .width
                                                                        ),
                                                                        format_px(
                                                                            geometry
                                                                                .anchor_rect
                                                                                .size
                                                                                .height
                                                                        )
                                                                    )),
                                                            )
                                                            .child(
                                                                div()
                                                                    .id("gallery-overlay-close")
                                                                    .px_2()
                                                                    .py_1()
                                                                    .rounded_sm()
                                                                    .border_1()
                                                                    .border_color(rgb(0xd6d8ce))
                                                                    .cursor_pointer()
                                                                    .on_click(cx.listener(
                                                                        |this, _, _, cx| {
                                                                            this.set_overlay_open(
                                                                                false, cx,
                                                                            );
                                                                        },
                                                                    ))
                                                                    .child("close"),
                                                            ),
                                                    ),
                                            )
                                            .priority(1),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(geometry_row("anchor", geometry.anchor_rect))
                            .child(geometry_row("layout", geometry.layout_rect))
                            .child(geometry_row("visual", geometry.visual_rect))
                            .child(geometry_row("preferred", geometry.preferred_rect))
                            .child(geometry_row("safe window", geometry.safe_window_rect))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0xd6d8ce))
                                    .bg(if self.overlay_open {
                                        rgb(0xe8f3ef)
                                    } else {
                                        rgb(0xffffff)
                                    })
                                    .text_sm()
                                    .child(if self.overlay_open { "open" } else { "closed" }),
                            ),
                    ),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_overlay_bounds(&self, label: &'static str, bounds: Rect) -> impl IntoElement {
        div()
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(if label == "visual rect" {
                rgb(0x2f80ed)
            } else {
                rgb(0xd6d8ce)
            })
            .bg(if label == "visual rect" {
                rgb(0xeaf2ff)
            } else {
                rgb(0xf6f7f2)
            })
            .opacity(0.8)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(0x3f4a57))
                    .child(label),
            )
    }

    fn render_signal_list(&self, page: GalleryPage) -> impl IntoElement {
        div()
            .id("gallery-foundation-signals")
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child("Foundation signals"),
            )
            .children(page.signals().iter().map(|signal| {
                div()
                    .px_3()
                    .py_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xd6d8ce))
                    .bg(rgb(0xffffff))
                    .text_sm()
                    .text_color(rgb(0x263240))
                    .child(*signal)
            }))
    }

    fn render_metric(&self, label: &'static str, value: Pixels) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(label))
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(format_px(value)),
            )
    }

    fn render_snapshot_summary(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("gallery-foundation-summary")
            .flex()
            .flex_col()
            .gap_2()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .px_3()
            .py_2()
            .text_xs()
            .text_color(rgb(0x3f4a57))
            .child(self.render_viewport_switch(snapshot.viewport_width, cx))
            .child(format!("width: {}", format_px(snapshot.viewport_width)))
            .child(format!("shell: {}", shell_mode_label(snapshot.shell_mode)))
            .child(format!("density: {}", density_label(snapshot.density)))
            .child(format!("size: {}", size_label(snapshot.control_size)))
            .child(format!("focus token: {}", snapshot.tokens.focus_ring))
    }

    fn render_viewport_switch(
        &self,
        viewport_width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("gallery-viewport-switch")
            .flex()
            .gap_1()
            .child(self.render_viewport_button(
                "compact",
                COMPACT_GALLERY_WIDTH,
                viewport_width,
                cx,
            ))
            .child(self.render_viewport_button(
                "desktop",
                DESKTOP_GALLERY_WIDTH,
                viewport_width,
                cx,
            ))
    }

    fn render_viewport_button(
        &self,
        label: &'static str,
        width: Pixels,
        active_width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = width == active_width;

        div()
            .id(format!("viewport-switch:{label}"))
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(if active { rgb(0x1f7a66) } else { rgb(0xd6d8ce) })
            .bg(if active { rgb(0xe8f3ef) } else { rgb(0xffffff) })
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0xf1f5ee)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_viewport_width(width, cx);
            }))
            .child(label)
    }
}

/// Opens the foundation gallery window.
pub fn open_gallery(cx: &mut App) {
    let bounds = Bounds::centered(
        None,
        size(DEFAULT_GALLERY_WIDTH, DEFAULT_GALLERY_HEIGHT),
        cx,
    );

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |_, cx| cx.new(GalleryShell::new),
    )
    .expect("failed to open UI foundation gallery window");

    cx.activate(true);
}

/// Returns a stable shell-mode label.
pub const fn shell_mode_label(mode: DeviceShellMode) -> &'static str {
    match mode {
        DeviceShellMode::Desktop => "desktop",
        DeviceShellMode::Mobile => "mobile",
    }
}

/// Returns a stable density label.
pub const fn density_label(density: Density) -> &'static str {
    match density {
        Density::Compact => "compact",
        Density::Comfortable => "comfortable",
        Density::Spacious => "spacious",
    }
}

/// Returns a stable size label.
pub const fn size_label(size: Size) -> &'static str {
    match size {
        Size::XSmall => "xs",
        Size::Small => "sm",
        Size::Medium => "md",
        Size::Large => "lg",
    }
}

/// Returns a stable device class label.
pub const fn device_class_label(class: DeviceAdaptiveClass) -> &'static str {
    match class {
        DeviceAdaptiveClass::Compact => "compact device",
        DeviceAdaptiveClass::Regular => "regular device",
        DeviceAdaptiveClass::Expanded => "expanded device",
    }
}

/// Returns a stable panel class label.
pub const fn panel_class_label(class: PanelAdaptiveClass) -> &'static str {
    match class {
        PanelAdaptiveClass::Compact => "compact panel",
        PanelAdaptiveClass::Medium => "medium panel",
        PanelAdaptiveClass::Wide => "wide panel",
    }
}

fn label_pill(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf6f7f2))
        .text_xs()
        .text_color(rgb(0x3f4a57))
        .child(label)
}

fn toggled_label(toggled: Toggled) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf6f7f2))
        .text_xs()
        .text_color(rgb(0x3f4a57))
        .child(toggled_label_text(toggled))
}

fn component_button_state_row(state: ButtonState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.variant().as_str(),
            size_label(state.size()),
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().padding_x())
        ))
}

fn component_switch_state_row(state: SwitchState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            toggled_label_text(state.toggled()),
            size_label(state.size()),
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "{} x {} / thumb {}",
            format_px(state.metrics().track_width()),
            format_px(state.metrics().track_height()),
            format_px(state.metrics().thumb_size())
        ))
}

fn component_text_input(
    id: String,
    label: impl Into<open_gpui::SharedString>,
    state: &TextInputState,
    tokens: ThemeTokens,
) -> TextInput {
    let input = TextInput::new(id, label)
        .value(state.value())
        .with_size(state.size())
        .disabled(state.disabled())
        .read_only(state.read_only())
        .required(state.required())
        .invalid(state.invalid())
        .tokens(tokens);

    if let Some(placeholder) = state.placeholder() {
        input.placeholder(placeholder)
    } else {
        input
    }
}

fn component_field(
    id: String,
    state: &FieldState,
    control: impl IntoElement,
    tokens: ThemeTokens,
) -> Field {
    let field = Field::new(id, state.control_id(), state.label())
        .with_size(state.size())
        .required(state.required())
        .disabled(state.disabled())
        .invalid(state.invalid())
        .tokens(tokens)
        .control(control);
    let field = if let Some(help) = state.help() {
        field.help(help)
    } else {
        field
    };

    if let Some(error) = state.error() {
        field.error(error)
    } else {
        field
    }
}

fn component_text_input_state_row(state: &TextInputState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            size_label(state.size()),
            if state.editable() {
                "editable"
            } else {
                "locked"
            },
            if state.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "{} / {}",
            if state.has_value() { "value" } else { "empty" },
            if state.displaying_placeholder() {
                "placeholder"
            } else {
                "display value"
            }
        ))
}

fn component_field_state_row(field: &FieldState, input: &TextInputState) -> impl IntoElement {
    let support = field.support_text().unwrap_or("no support text");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            size_label(field.size()),
            if field.required() {
                "required"
            } else {
                "optional"
            },
            if field.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "{} / {}",
            if field.support_is_error() {
                "error"
            } else {
                "help"
            },
            support
        ))
        .child(if input.editable() {
            "control editable"
        } else {
            "control locked"
        })
}

fn toggled_label_text(toggled: Toggled) -> &'static str {
    match toggled {
        Toggled::True => "on",
        Toggled::False => "off",
        Toggled::Mixed => "mixed",
    }
}

fn geometry_row(label: &'static str, rect: Rect) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .text_xs()
        .text_color(rgb(0x3f4a57))
        .child(format!(
            "{}: {}, {} / {} x {}",
            label,
            format_px(rect.origin.x),
            format_px(rect.origin.y),
            format_px(rect.size.width),
            format_px(rect.size.height)
        ))
}

fn format_px(value: Pixels) -> String {
    format!("{:.0}px", value.as_f32())
}
