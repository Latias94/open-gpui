//! Gallery shell that consumes the UI foundation directly.

use open_gpui::prelude::*;
use open_gpui::{
    AccessibleAction, Anchor, App, AppContext, Bounds, Context, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Pixels, Render, Role, ScrollHandle,
    StatefulInteractiveElement, Styled, Toggled, Window, WindowBounds, WindowOptions, anchored,
    deferred, div, point, px, rgb, size,
};
use open_gpui_ui_components::{
    Badge, BadgeState, Button, ButtonState, Checkbox, CheckboxState, ColorIntent, ContextMenu,
    Dialog, DialogOpenMode, Field, FieldState, FocusRing, IconButton, IconButtonState, Label,
    LabelState, Menu, MenuItem, Popover, PopoverOpenMode, RadioGroup, RadioItem, ScrollArea,
    ScrollAreaAxis, ScrollAreaState, Switch, SwitchState, Tabs, TabsActivationMode, TabsItem,
    TabsState, TextInput, TextInputController, TextInputState, Toggle, ToggleState, Tooltip,
    TooltipContentKind, TooltipOpenIntent, focus_ring_shadow, init_text_input,
};
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceAdaptivePolicy, DeviceShellMode, DeviceShellSwitchPolicy,
    Orientation, PanelAdaptiveClass, Rect, Sizable, Size, ThemeTokens,
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
    editable_text_input: open_gpui::Entity<TextInputController>,
    focus_controls: [FocusHandle; 3],
    tooltip_focus_controls: [FocusHandle; 4],
    focus_message: &'static str,
    a11y_counter: i32,
    a11y_enabled: bool,
    overlay_open: bool,
    hovered_tooltip_sample: Option<&'static str>,
    overlay_controlled_popover_open: bool,
    overlay_controlled_dialog_open: bool,
    overlay_controlled_menu_open: bool,
    overlay_controlled_context_menu_open: bool,
}

impl GalleryShell {
    fn build(selected_page: GalleryPage, cx: &mut Context<Self>) -> Self {
        Self {
            selected_page,
            width: DEFAULT_GALLERY_WIDTH,
            navigation_scroll: ScrollHandle::new(),
            page_scroll: ScrollHandle::new(),
            root_focus: cx.focus_handle(),
            editable_text_input: cx.new(|cx| {
                let mut controller = TextInputController::with_value("", cx);
                controller.set_placeholder("Type in the gallery", cx);
                controller
            }),
            focus_controls: [
                cx.focus_handle().tab_index(1).tab_stop(true),
                cx.focus_handle().tab_index(2).tab_stop(true),
                cx.focus_handle().tab_index(3).tab_stop(true),
            ],
            tooltip_focus_controls: [
                cx.focus_handle().tab_index(10).tab_stop(true),
                cx.focus_handle().tab_index(11).tab_stop(true),
                cx.focus_handle().tab_index(12).tab_stop(true),
                cx.focus_handle().tab_index(13).tab_stop(true),
            ],
            focus_message: "Ready for keyboard focus.",
            a11y_counter: 0,
            a11y_enabled: false,
            overlay_open: false,
            hovered_tooltip_sample: None,
            overlay_controlled_popover_open: false,
            overlay_controlled_dialog_open: false,
            overlay_controlled_menu_open: false,
            overlay_controlled_context_menu_open: false,
        }
    }
}

impl GalleryShell {
    /// Creates a gallery shell entity.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_selected_page(GalleryPage::Tokens, cx)
    }

    /// Creates a gallery shell entity with an initial page.
    pub fn with_selected_page(page: GalleryPage, cx: &mut Context<Self>) -> Self {
        Self::build(page, cx)
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
            self.hovered_tooltip_sample = None;
            self.overlay_controlled_popover_open = false;
            self.overlay_controlled_dialog_open = false;
            self.overlay_controlled_menu_open = false;
            self.overlay_controlled_context_menu_open = false;
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

    fn set_hovered_tooltip_sample(&mut self, sample: Option<&'static str>, cx: &mut Context<Self>) {
        if self.hovered_tooltip_sample != sample {
            self.hovered_tooltip_sample = sample;
            cx.notify();
        }
    }

    fn set_controlled_popover_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.overlay_controlled_popover_open != open {
            self.overlay_controlled_popover_open = open;
            cx.notify();
        }
    }

    fn set_controlled_dialog_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.overlay_controlled_dialog_open != open {
            self.overlay_controlled_dialog_open = open;
            cx.notify();
        }
    }

    fn set_controlled_menu_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.overlay_controlled_menu_open != open {
            self.overlay_controlled_menu_open = open;
            cx.notify();
        }
    }

    fn set_controlled_context_menu_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.overlay_controlled_context_menu_open != open {
            self.overlay_controlled_context_menu_open = open;
            cx.notify();
        }
    }
}

impl Render for GalleryShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    this.set_hovered_tooltip_sample(None, cx);
                    this.set_controlled_popover_open(false, cx);
                    this.set_controlled_dialog_open(false, cx);
                    this.set_controlled_menu_open(false, cx);
                    this.set_controlled_context_menu_open(false, cx);
                }
            }))
            .child(self.render_navigation(page, cx))
            .child(self.render_content(snapshot, window, cx))
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
        window: &mut Window,
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
                    .child(self.render_page_body(snapshot, window, cx)),
            )
    }

    fn render_page_body(
        &self,
        snapshot: GalleryShellSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match snapshot.selected_page {
            GalleryPage::Tokens => self.render_tokens_page(snapshot).into_any_element(),
            GalleryPage::SizingDensity => self.render_sizing_page(snapshot).into_any_element(),
            GalleryPage::Adaptive => self.render_adaptive_page(snapshot).into_any_element(),
            GalleryPage::FocusAccessibility => {
                self.render_focus_a11y_page(snapshot, cx).into_any_element()
            }
            GalleryPage::Overlay => self
                .render_overlay_page(snapshot, window, cx)
                .into_any_element(),
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
        let tabs_samples = pages::components::tabs_samples(snapshot.tokens);
        let radio_samples = pages::components::radio_group_samples(snapshot.tokens);
        let toggle_samples = pages::components::toggle_samples(snapshot.tokens);
        let badge_samples = pages::components::badge_samples(snapshot.tokens);
        let icon_button_samples = pages::components::icon_button_samples(snapshot.tokens);
        let scroll_area_samples = pages::components::scroll_area_samples(snapshot.tokens);

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
                            .child("ScrollArea"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        scroll_area_samples.into_iter().map(|sample| {
                            let sample_id = sample.id;
                            let title = sample.title;
                            let summary = sample.summary;
                            let items = sample.items;
                            let state = sample.state.clone();
                            let horizontal = state.axis() == ScrollAreaAxis::Horizontal;
                            let two_axis = state.axis() == ScrollAreaAxis::Both;
                            let content = div()
                                .when(horizontal, |this| this.flex().gap_2().min_w(px(860.0)))
                                .when(two_axis, |this| {
                                    this.flex().flex_col().gap_1().min_w(px(620.0))
                                })
                                .when(!horizontal && !two_axis, |this| {
                                    this.flex().flex_col().gap_1()
                                })
                                .children(items.into_iter().enumerate().map(
                                    move |(index, item)| {
                                        let vertical_only = !horizontal && !two_axis;
                                        div()
                                            .id(format!(
                                                "component-scroll-area-item:{}:{}",
                                                sample_id, index
                                            ))
                                            .when(horizontal, |this| {
                                                this.w(px(132.0)).min_h(px(88.0))
                                            })
                                            .when(two_axis, |this| {
                                                this.w(px(620.0)).min_h(px(34.0))
                                            })
                                            .when(vertical_only, |this| this.min_h(px(28.0)))
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xf8f9f3))
                                            .px_3()
                                            .py_2()
                                            .text_xs()
                                            .text_color(rgb(0x3f4a57))
                                            .child(item)
                                    },
                                ));
                            let scroll_area = ScrollArea::new(
                                format!("component-scroll-area:{}", sample_id),
                                content,
                            )
                            .axis(state.axis())
                            .with_size(state.size());
                            let scroll_area = if let Some(reset_key) = state.reset_key() {
                                scroll_area.reset_on_key(reset_key)
                            } else {
                                scroll_area
                            };

                            div()
                                .id(format!("component-scroll-area-sample:{}", sample_id))
                                .w(px(360.0))
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
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(open_gpui::FontWeight::BOLD)
                                                .child(title),
                                        )
                                        .child(label_pill(state.axis().as_str())),
                                )
                                .child(div().text_xs().text_color(rgb(0x5a6472)).child(summary))
                                .child(
                                    div()
                                        .h(px(154.0))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xe2e4dc))
                                        .bg(rgb(0xfcfcf8))
                                        .child(scroll_area),
                                )
                                .child(component_scroll_area_state_row(&state))
                        }),
                    )),
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
                            .child("Badge"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        badge_samples.into_iter().map(|sample| {
                            let state = sample.state;
                            div()
                                .id(format!("component-badge-sample:{}", sample.id))
                                .min_w(px(160.0))
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(
                                    Badge::new(
                                        format!("component-badge:{}", sample.id),
                                        sample.label,
                                    )
                                    .variant(state.variant())
                                    .with_size(state.size())
                                    .tokens(snapshot.tokens),
                                )
                                .child(component_badge_state_row(state))
                        }),
                    )),
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
                            .child("Checkbox"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::checkbox_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state;
                                    div()
                                        .id(format!("component-checkbox-sample:{}", sample.id))
                                        .min_w(px(220.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(component_checkbox(
                                            format!("component-checkbox:{}", sample.id),
                                            sample.label,
                                            state,
                                            snapshot.tokens,
                                        ))
                                        .child(component_checkbox_state_row(state))
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
                            .child("RadioGroup"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        radio_samples.into_iter().map(|sample| {
                            let state = sample.state.clone();
                            let mut radio =
                                RadioGroup::new(format!("component-radio:{}", sample.id))
                                    .label(sample.title)
                                    .orientation(sample.orientation)
                                    .selected(sample.selected)
                                    .required(sample.required)
                                    .disabled(sample.disabled)
                                    .with_size(state.size())
                                    .tokens(snapshot.tokens);
                            for item in sample.items.iter() {
                                radio = radio.item(
                                    RadioItem::new(item.value, item.label).disabled(item.disabled),
                                );
                            }

                            div()
                                .id(format!("component-radio-sample:{}", sample.id))
                                .min_w(px(240.0))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(radio)
                                .child(component_radio_state_row(&state))
                        }),
                    )),
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
                            .child("Toggle"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        toggle_samples.into_iter().map(|sample| {
                            let state = sample.state;
                            div()
                                .id(format!("component-toggle-sample:{}", sample.id))
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
                                    Toggle::new(
                                        format!("component-toggle:{}", sample.id),
                                        sample.label,
                                    )
                                    .variant(state.variant())
                                    .pressed(state.pressed())
                                    .disabled(state.disabled())
                                    .with_size(state.size())
                                    .tokens(snapshot.tokens),
                                )
                                .child(component_toggle_state_row(&state))
                        }),
                    )),
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
                            .child("IconButton"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        icon_button_samples.into_iter().map(|sample| {
                            let state = sample.state;
                            div()
                                .id(format!("component-icon-button-sample:{}", sample.id))
                                .min_w(px(170.0))
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(
                                    IconButton::new(
                                        format!("component-icon-button:{}", sample.id),
                                        sample.icon,
                                        sample.accessible_label,
                                    )
                                    .variant(state.variant())
                                    .disabled(state.disabled())
                                    .with_size(state.size())
                                    .tokens(snapshot.tokens),
                                )
                                .child(component_icon_button_state_row(
                                    sample.accessible_label,
                                    state,
                                ))
                        }),
                    )),
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
                            .child("Label"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::label_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state.clone();
                                    div()
                                        .id(format!("component-label-sample:{}", sample.id))
                                        .min_w(px(220.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(component_label(
                                            format!("component-label:{}", sample.id),
                                            &state,
                                            snapshot.tokens,
                                        ))
                                        .child(component_label_state_row(&state))
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
                                    let controller = sample
                                        .controller_driven
                                        .then(|| self.editable_text_input.clone());
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
                                            controller,
                                        ))
                                        .child(component_text_input_state_row(
                                            &state,
                                            sample.controller_driven,
                                        ))
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
                                                None,
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Tabs"),
                    )
                    .child(div().flex().gap_3().flex_wrap().children(
                        tabs_samples.into_iter().map(|sample| {
                            let state = sample.state.clone();
                            let tabs = sample.items.into_iter().fold(
                                Tabs::new(format!("component-tabs:{}", sample.id))
                                    .orientation(sample.orientation)
                                    .activation_mode(sample.activation_mode)
                                    .with_size(sample.size)
                                    .selected(sample.selected)
                                    .tokens(snapshot.tokens),
                                |tabs, item| {
                                    tabs.item(
                                        TabsItem::new(
                                            format!(
                                                "component-tabs-item:{}:{}",
                                                sample.id, item.value
                                            ),
                                            item.label,
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(open_gpui::FontWeight::BOLD)
                                                        .child(item.label),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x5a6472))
                                                        .child(item.panel),
                                                ),
                                        )
                                        .disabled(item.disabled),
                                    )
                                },
                            );

                            div()
                                .id(format!("component-tabs-sample:{}", sample.id))
                                .min_w(px(360.0))
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
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(open_gpui::FontWeight::BOLD)
                                                .child(sample.title),
                                        )
                                        .child(label_pill(sample.activation_mode.as_str())),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(sample.summary),
                                )
                                .child(
                                    div()
                                        .when(sample.orientation == Orientation::Vertical, |this| {
                                            this.h(px(240.0))
                                        })
                                        .child(tabs),
                                )
                                .child(component_tabs_state_row(
                                    sample.orientation,
                                    sample.activation_mode,
                                    sample.size,
                                    &state,
                                ))
                        }),
                    )),
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let geometry = pages::overlay::demo_geometry();
        let behavior_samples = pages::overlay::behavior_samples();
        let tooltip_samples = pages::overlay::tooltip_samples(snapshot.tokens);
        let popover_samples = pages::overlay::popover_samples(snapshot.tokens);
        let dialog_samples = pages::overlay::dialog_samples(snapshot.tokens);
        let menu_samples = pages::overlay::menu_samples(snapshot.tokens);
        let context_menu_samples = pages::overlay::context_menu_samples(snapshot.tokens);

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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Behavior contracts"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_3()
                            .children(behavior_samples.iter().map(overlay_behavior_card)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Tooltip samples"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_3()
                            .child(self.render_tooltip_sample_card(
                                &tooltip_samples[0],
                                &self.tooltip_focus_controls[0],
                                self.tooltip_focus_controls[0].is_focused(window),
                                cx,
                            ))
                            .child(self.render_tooltip_sample_card(
                                &tooltip_samples[1],
                                &self.tooltip_focus_controls[1],
                                self.tooltip_focus_controls[1].is_focused(window),
                                cx,
                            ))
                            .child(self.render_tooltip_sample_card(
                                &tooltip_samples[2],
                                &self.tooltip_focus_controls[2],
                                self.tooltip_focus_controls[2].is_focused(window),
                                cx,
                            ))
                            .child(self.render_tooltip_sample_card(
                                &tooltip_samples[3],
                                &self.tooltip_focus_controls[3],
                                self.tooltip_focus_controls[3].is_focused(window),
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Popover samples"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_3()
                            .child(self.render_popover_sample_card(&popover_samples[0], false, cx))
                            .child(self.render_popover_sample_card(
                                &popover_samples[1],
                                self.overlay_controlled_popover_open,
                                cx,
                            ))
                            .child(self.render_popover_sample_card(&popover_samples[2], false, cx))
                            .child(self.render_popover_sample_card(&popover_samples[3], false, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Dialog samples"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_3()
                            .child(self.render_dialog_sample_card(
                                &dialog_samples[0],
                                self.overlay_controlled_dialog_open,
                                cx,
                            ))
                            .child(self.render_dialog_sample_card(&dialog_samples[1], false, cx))
                            .child(self.render_dialog_sample_card(&dialog_samples[2], false, cx))
                            .child(self.render_dialog_sample_card(&dialog_samples[3], false, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Menu samples"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_3()
                            .child(self.render_menu_sample_card(&menu_samples[0], false, cx))
                            .child(self.render_menu_sample_card(
                                &menu_samples[1],
                                self.overlay_controlled_menu_open,
                                cx,
                            ))
                            .child(self.render_menu_sample_card(&menu_samples[2], false, cx))
                            .child(self.render_menu_sample_card(&menu_samples[3], false, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("ContextMenu samples"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap_3()
                            .child(self.render_context_menu_sample_card(
                                &context_menu_samples[0],
                                false,
                                cx,
                            ))
                            .child(self.render_context_menu_sample_card(
                                &context_menu_samples[1],
                                self.overlay_controlled_context_menu_open,
                                cx,
                            ))
                            .child(self.render_context_menu_sample_card(
                                &context_menu_samples[2],
                                false,
                                cx,
                            )),
                    ),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_tooltip_sample_card(
        &self,
        sample: &pages::overlay::TooltipSample,
        focus_handle: &FocusHandle,
        focus_handle_is_focused: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = sample.state.clone();
        let sample_id = sample.id;
        let label = sample.label;
        let tooltip_text = sample.tooltip_text;
        let focused =
            focus_handle_is_focused && state.open_intent().opens_on_focus() && !state.disabled();
        let hovered = self.hovered_tooltip_sample == Some(sample_id)
            && state.open_intent().opens_on_hover()
            && !state.disabled();
        let forced_open = state.open() && !state.disabled();
        let open = focused || hovered || forced_open;
        let focus_ring = FocusRing::from_color(ColorIntent::new(
            ThemeTokens::default().focus_ring,
            0x2f80ed,
        ));

        div()
            .id(format!("overlay-tooltip-sample:{}", sample_id))
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .text_xs()
            .text_color(rgb(0x3f4a57))
            .child(
                div()
                    .id(format!("overlay-tooltip-trigger:{}", sample_id))
                    .min_h(px(44.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(if open { rgb(0x1f7a66) } else { rgb(0xd6d8ce) })
                    .bg(if state.disabled() {
                        rgb(0xf1f2ed)
                    } else if open {
                        rgb(0xe8f3ef)
                    } else {
                        rgb(0xffffff)
                    })
                    .px_3()
                    .py_2()
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .track_focus(focus_handle)
                    .focusable()
                    .tab_stop(!state.disabled())
                    .role(Role::Button)
                    .aria_label(label)
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xf1f5ee)))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.set_hovered_tooltip_sample(hovered.then_some(sample_id), cx);
                    }))
                    .child(label),
            )
            .when(open, |card| {
                card.child(
                    Tooltip::new(
                        format!("overlay-tooltip-content:{}", sample_id),
                        tooltip_text,
                    )
                    .open(true)
                    .open_intent(state.open_intent())
                    .placement_side(state.placement_side())
                    .placement_alignment(state.placement_alignment())
                    .delay(state.delay())
                    .with_size(state.size()),
                )
            })
            .child(tooltip_state_row(&state, open))
    }

    fn render_popover_sample_card(
        &self,
        sample: &pages::overlay::PopoverSample,
        controlled_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if sample.id == "controlled" {
            Popover::new(
                format!("overlay-popover-sample:{}", sample.id),
                sample.label,
                sample.content_text,
            )
            .open(controlled_open)
            .placement_side(sample.state.placement_side())
            .placement_alignment(sample.state.placement_alignment())
            .state()
        } else {
            sample.state.clone()
        };
        let sample_id = sample.id;
        let label = sample.label;
        let content_text = sample.content_text;
        let shell = cx.entity().downgrade();
        let popover = Popover::new(
            format!("overlay-popover-demo:{}", sample_id),
            label,
            content_text,
        )
        .disabled(state.disabled())
        .placement_side(state.placement_side())
        .placement_alignment(state.placement_alignment())
        .outside_press_policy(state.outside_press_policy());
        let popover = match sample_id {
            "controlled" => popover
                .open(state.open())
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| this.set_controlled_popover_open(open, cx))
                        .ok();
                }),
            "default-open" => popover.default_open(state.default_open()),
            _ => popover.open(state.open()),
        };

        div()
            .id(format!("overlay-popover-sample-card:{}", sample_id))
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .text_xs()
            .text_color(rgb(0x3f4a57))
            .child(popover)
            .when(sample_id == "controlled", |card| {
                card.child(
                    div()
                        .id("overlay-popover-controlled-toggle")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_controlled_popover_open(!controlled_open, cx);
                        }))
                        .child(if controlled_open {
                            "close controlled"
                        } else {
                            "open controlled"
                        }),
                )
            })
            .child(popover_state_row(&state))
    }

    fn render_dialog_sample_card(
        &self,
        sample: &pages::overlay::DialogSample,
        controlled_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if sample.id == "controlled-modal" {
            Dialog::new(
                format!("overlay-dialog-sample:{}", sample.id),
                sample.label,
                sample.title,
                sample.content_text,
            )
            .description("Escape and the modal barrier can close it.")
            .open(controlled_open)
            .outside_press_policy(sample.state.outside_press_policy())
            .escape_key_policy(sample.state.escape_key_policy())
            .state()
        } else {
            sample.state.clone()
        };
        let sample_id = sample.id;
        let label = sample.label;
        let title = sample.title;
        let content_text = sample.content_text;
        let shell = cx.entity().downgrade();
        let dialog = Dialog::new(
            format!("overlay-dialog-demo:{}", sample_id),
            label,
            title,
            content_text,
        )
        .disabled(state.disabled())
        .outside_press_policy(state.outside_press_policy())
        .escape_key_policy(state.escape_key_policy());
        let dialog = match sample_id {
            "controlled-modal" => dialog
                .open(state.open())
                .description("Escape and the modal barrier can close it.")
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| this.set_controlled_dialog_open(open, cx))
                        .ok();
                }),
            "default-open" => dialog.default_open(state.default_open()),
            _ => dialog.open(state.open()),
        };

        div()
            .id(format!("overlay-dialog-sample-card:{}", sample_id))
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .text_xs()
            .text_color(rgb(0x3f4a57))
            .child(dialog)
            .when(sample_id == "controlled-modal", |card| {
                card.child(
                    div()
                        .id("overlay-dialog-controlled-toggle")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_controlled_dialog_open(!controlled_open, cx);
                        }))
                        .child(if controlled_open {
                            "close dialog"
                        } else {
                            "open dialog"
                        }),
                )
            })
            .child(dialog_state_row(&state))
    }

    fn render_menu_sample_card(
        &self,
        sample: &pages::overlay::MenuSample,
        controlled_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if sample.id == "controlled" {
            Menu::new(format!("overlay-menu-sample:{}", sample.id), sample.label)
                .open(controlled_open)
                .focused_value(state_focused_value(&sample.state).unwrap_or("copy"))
                .items(menu_items_for_sample(sample.id))
                .state()
        } else {
            sample.state.clone()
        };
        let sample_id = sample.id;
        let label = sample.label;
        let shell = cx.entity().downgrade();
        let menu = Menu::new(format!("overlay-menu-demo:{}", sample_id), label)
            .items(menu_items_for_sample(sample_id))
            .disabled(state.disabled())
            .outside_press_policy(state.outside_press_policy())
            .escape_key_policy(state.escape_key_policy());
        let menu = match sample_id {
            "controlled" => menu
                .open(state.open())
                .focused_value(state_focused_value(&state).unwrap_or("copy"))
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| this.set_controlled_menu_open(open, cx))
                        .ok();
                }),
            "default-open" => menu
                .default_open(state.default_open())
                .focused_value(state_focused_value(&state).unwrap_or("save")),
            _ => menu.open(state.open()),
        };

        div()
            .id(format!("overlay-menu-sample-card:{}", sample_id))
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .text_xs()
            .text_color(rgb(0x3f4a57))
            .child(menu)
            .when(sample_id == "controlled", |card| {
                card.child(
                    div()
                        .id("overlay-menu-controlled-toggle")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_controlled_menu_open(!controlled_open, cx);
                        }))
                        .child(if controlled_open {
                            "close menu"
                        } else {
                            "open menu"
                        }),
                )
            })
            .child(menu_state_row(&state))
    }

    fn render_context_menu_sample_card(
        &self,
        sample: &pages::overlay::ContextMenuSample,
        controlled_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if sample.id == "controlled" {
            ContextMenu::new(
                format!("overlay-context-menu-sample:{}", sample.id),
                sample.label,
            )
            .open(controlled_open)
            .anchor_point(sample.state.anchor_point())
            .focused_value(state_focused_value(sample.state.menu()).unwrap_or("inspect"))
            .items(context_menu_items_for_sample(sample.id))
            .state()
        } else {
            sample.state.clone()
        };
        let sample_id = sample.id;
        let label = sample.label;
        let shell = cx.entity().downgrade();
        let context_menu =
            ContextMenu::new(format!("overlay-context-menu-demo:{}", sample_id), label)
                .items(context_menu_items_for_sample(sample_id))
                .anchor_point(state.anchor_point())
                .outside_press_policy(state.menu().outside_press_policy())
                .escape_key_policy(state.menu().escape_key_policy());
        let context_menu = match sample_id {
            "controlled" => context_menu
                .open(state.open())
                .focused_value(state_focused_value(state.menu()).unwrap_or("inspect"))
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.set_controlled_context_menu_open(open, cx)
                        })
                        .ok();
                }),
            "default-open" => context_menu.default_open(state.default_open()),
            _ => context_menu.open(state.open()),
        };

        div()
            .id(format!("overlay-context-menu-sample-card:{}", sample_id))
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .text_xs()
            .text_color(rgb(0x3f4a57))
            .child(context_menu)
            .when(sample_id == "controlled", |card| {
                card.child(
                    div()
                        .id("overlay-context-menu-controlled-toggle")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_controlled_context_menu_open(!controlled_open, cx);
                        }))
                        .child(if controlled_open {
                            "close context menu"
                        } else {
                            "open context menu"
                        }),
                )
            })
            .child(context_menu_state_row(&state))
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
    open_gallery_page(GalleryPage::Tokens, cx);
}

/// Opens the foundation gallery window on a specific page.
pub fn open_gallery_page(page: GalleryPage, cx: &mut App) {
    init_text_input(cx);

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
        move |_, cx| cx.new(|cx| GalleryShell::with_selected_page(page, cx)),
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

fn component_badge_state_row(state: BadgeState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / display",
            state.variant().as_str(),
            size_label(state.size())
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().min_height()),
            format_px(state.metrics().padding_x())
        ))
}

fn component_icon_button_state_row(
    accessible_label: &'static str,
    state: IconButtonState,
) -> impl IntoElement {
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
            "box {} icon {}",
            format_px(state.metrics().size()),
            format_px(state.metrics().icon_size())
        ))
        .child(format!("aria {}", accessible_label))
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

fn component_checkbox(
    id: String,
    label: impl Into<open_gpui::SharedString>,
    state: CheckboxState,
    tokens: ThemeTokens,
) -> Checkbox {
    Checkbox::new(id)
        .label(label)
        .checked(state.checked())
        .indeterminate(state.indeterminate())
        .disabled(state.disabled())
        .required(state.required())
        .invalid(state.invalid())
        .with_size(state.size())
        .tokens(tokens)
}

fn component_checkbox_state_row(state: CheckboxState) -> impl IntoElement {
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
            "{} / {}",
            if state.required() {
                "required"
            } else {
                "optional"
            },
            if state.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "box {} indicator {}",
            format_px(state.metrics().box_size()),
            format_px(state.metrics().indicator_size())
        ))
}

fn component_label(id: String, state: &LabelState, tokens: ThemeTokens) -> Label {
    let label = Label::new(id, state.text())
        .with_size(state.size())
        .required(state.required())
        .disabled(state.disabled())
        .tokens(tokens);

    if let Some(control_id) = state.control_id() {
        label.for_control(control_id)
    } else {
        label
    }
}

fn component_label_state_row(state: &LabelState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            size_label(state.size()),
            if state.required() {
                "required"
            } else {
                "optional"
            },
            if state.disabled() {
                "disabled"
            } else {
                "enabled"
            }
        ))
        .child(format!(
            "{}",
            state.control_id().unwrap_or("no control association")
        ))
}

fn component_text_input(
    id: String,
    label: impl Into<open_gpui::SharedString>,
    state: &TextInputState,
    tokens: ThemeTokens,
    controller: Option<open_gpui::Entity<TextInputController>>,
) -> TextInput {
    let input = TextInput::new(id, label)
        .value(state.value())
        .with_size(state.size())
        .disabled(state.disabled())
        .read_only(state.read_only())
        .required(state.required())
        .invalid(state.invalid())
        .tokens(tokens);
    let input = if let Some(controller) = controller {
        input.controller(controller)
    } else {
        input
    };

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

fn component_text_input_state_row(
    state: &TextInputState,
    controller_driven: bool,
) -> impl IntoElement {
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
        .child(if controller_driven || state.controller_driven() {
            "controller"
        } else {
            "static"
        })
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

fn component_tabs_state_row(
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    size: Size,
    state: &TabsState,
) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");
    let focused = state.focused_value().unwrap_or("none");
    let tab_stop = state.tab_stop_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            match orientation {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            activation_mode.as_str(),
            size_label(size)
        ))
        .child(format!(
            "selected {} / focus {} / tab stop {}",
            selected, focused, tab_stop
        ))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

fn component_scroll_area_state_row(state: &ScrollAreaState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.axis().as_str(),
            state.reset_policy().as_str(),
            size_label(state.size())
        ))
        .child(format!(
            "viewport {} / scrollbar {}",
            state.viewport_id(),
            format_px(state.metrics().scrollbar_width())
        ))
        .child(format!(
            "x {} / y {}",
            if state.scrolls_x() { "scroll" } else { "clip" },
            if state.scrolls_y() { "scroll" } else { "clip" }
        ))
}

fn component_radio_state_row(state: &open_gpui_ui_components::RadioGroupState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");
    let focused = state.focused_value().unwrap_or("none");
    let tab_stop = state.tab_stop_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            if state.required() {
                "required"
            } else {
                "optional"
            },
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "selected {} / focus {} / tab stop {}",
            selected, focused, tab_stop
        ))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

fn component_toggle_state_row(state: &ToggleState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            if state.pressed() {
                "pressed"
            } else {
                "released"
            },
            state.variant().as_str(),
            size_label(state.size())
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().padding_x())
        ))
}

fn overlay_behavior_card(sample: &pages::overlay::OverlayBehaviorSample) -> impl IntoElement {
    let policy = &sample.policy;
    let adapter = &sample.adapter;
    let presence = policy.presence();
    let layer_state = policy.layer_state();
    let outside = policy.outside_press_policy().resolve();

    div()
        .id(format!("overlay-behavior:{}", sample.id))
        .flex()
        .flex_col()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .p_3()
        .text_xs()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .text_color(rgb(0x24313f))
                .child(sample.label),
        )
        .child(format!(
            "kind: {}",
            pages::overlay::layer_kind_label(policy.kind())
        ))
        .child(format!(
            "presence: open {} / present {} / interactive {}",
            bool_label(presence.is_open()),
            bool_label(presence.present()),
            bool_label(presence.interactive())
        ))
        .child(format!(
            "outside: {}",
            pages::overlay::outside_press_label(policy.outside_press_policy())
        ))
        .child(format!(
            "escape: {}",
            pages::overlay::escape_key_label(policy.escape_key_policy())
        ))
        .child(format!(
            "focus: open {} / close {}",
            pages::overlay::initial_focus_label(policy.initial_focus_intent()),
            pages::overlay::focus_restore_label(policy.focus_restore_intent())
        ))
        .child(format!(
            "layer: visible {} / hit {} / underlay {} / outside {}",
            bool_label(layer_state.visible()),
            bool_label(layer_state.hit_testable()),
            bool_label(layer_state.blocks_underlay_input()),
            bool_label(layer_state.wants_outside_press())
        ))
        .child(format!(
            "outside outcome: dismiss {} / consume {} / underlay {}",
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "gpui: priority {} / margin {} / layer {} / outside handler {}",
            adapter.deferred_priority(),
            format_px(adapter.snap_margin()),
            bool_label(adapter.should_render_deferred_layer()),
            bool_label(adapter.wants_outside_press_handler())
        ))
}

fn tooltip_state_row(
    state: &open_gpui_ui_components::TooltipState,
    open: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / intent {} / content {}",
            bool_label(open),
            tooltip_open_intent_label(state.open_intent()),
            tooltip_content_kind_label(state.content_kind())
        ))
        .child(format!(
            "placement: {} {} / disabled {} / descriptive {}",
            pages::overlay::layer_kind_label(state.overlay().policy().kind()),
            tooltip_placement_label(state.placement_side()),
            bool_label(state.disabled()),
            bool_label(state.descriptive())
        ))
        .child(format!(
            "delay: open {} / close {} / skip {}",
            format_duration_ms(state.delay().open_delay()),
            format_duration_ms(state.delay().close_delay()),
            format_duration_ms(state.delay().skip_delay())
        ))
}

fn popover_state_row(state: &open_gpui_ui_components::PopoverState) -> impl IntoElement {
    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / disabled {}",
            bool_label(state.open()),
            popover_open_mode_label(state.open_mode()),
            bool_label(state.disabled())
        ))
        .child(format!(
            "placement: {} {} / trigger selected {}",
            tooltip_placement_label(state.placement_side()),
            popover_alignment_label(state.placement_alignment()),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            pages::overlay::outside_press_label(state.outside_press_policy()),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "focus: open {} / close {} / layer {}",
            pages::overlay::initial_focus_label(state.initial_focus_intent()),
            pages::overlay::focus_restore_label(state.focus_restore_intent()),
            pages::overlay::layer_kind_label(state.overlay().policy().kind())
        ))
}

fn popover_open_mode_label(mode: PopoverOpenMode) -> &'static str {
    match mode {
        PopoverOpenMode::Uncontrolled => "uncontrolled",
        PopoverOpenMode::Controlled => "controlled",
    }
}

fn popover_alignment_label(
    alignment: open_gpui_ui_core::OverlayPlacementAlignment,
) -> &'static str {
    match alignment {
        open_gpui_ui_core::OverlayPlacementAlignment::Start => "start",
        open_gpui_ui_core::OverlayPlacementAlignment::Center => "center",
        open_gpui_ui_core::OverlayPlacementAlignment::End => "end",
    }
}

fn dialog_state_row(state: &open_gpui_ui_components::DialogState) -> impl IntoElement {
    let layer_state = state.overlay().layer_state();
    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / disabled {}",
            bool_label(state.open()),
            dialog_open_mode_label(state.open_mode()),
            bool_label(state.disabled())
        ))
        .child(format!(
            "title: {} / description {} / trigger selected {}",
            state.title(),
            bool_label(state.description().is_some()),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            pages::overlay::outside_press_label(state.outside_press_policy()),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / layer {}",
            pages::overlay::escape_key_label(state.escape_key_policy()),
            bool_label(layer_state.blocks_underlay_input()),
            pages::overlay::layer_kind_label(state.overlay().policy().kind())
        ))
}

fn dialog_open_mode_label(mode: DialogOpenMode) -> &'static str {
    match mode {
        DialogOpenMode::Uncontrolled => "uncontrolled",
        DialogOpenMode::Controlled => "controlled",
    }
}

fn menu_items_for_sample(sample_id: &str) -> Vec<MenuItem> {
    match sample_id {
        "controlled" => vec![
            MenuItem::action("cut", "Cut"),
            MenuItem::action("copy", "Copy"),
            MenuItem::action("paste", "Paste").disabled(true),
        ],
        "outside-ignore" => vec![
            MenuItem::action("rename", "Rename"),
            MenuItem::action("duplicate", "Duplicate"),
        ],
        "disabled" => vec![MenuItem::action("open", "Open")],
        _ => vec![
            MenuItem::action("new", "New"),
            MenuItem::action("save", "Save"),
            MenuItem::separator("separator"),
            MenuItem::action("delete", "Delete").disabled(true),
        ],
    }
}

fn context_menu_items_for_sample(sample_id: &str) -> Vec<MenuItem> {
    match sample_id {
        "controlled" => vec![
            MenuItem::action("inspect", "Inspect"),
            MenuItem::action("copy-link", "Copy link"),
        ],
        "default-open" => vec![
            MenuItem::action("open", "Open"),
            MenuItem::action("close", "Close"),
        ],
        _ => vec![
            MenuItem::action("duplicate", "Duplicate"),
            MenuItem::separator("separator"),
            MenuItem::action("delete", "Delete").disabled(true),
        ],
    }
}

fn state_focused_value(state: &open_gpui_ui_components::MenuState) -> Option<&str> {
    state.focused_value()
}

fn menu_state_row(state: &open_gpui_ui_components::MenuState) -> impl IntoElement {
    let outside = state.outside_press_policy().resolve();
    let focused = state.focused_value().unwrap_or("none");
    let active_items = state.items().iter().filter(|item| item.focusable()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / disabled {}",
            bool_label(state.open()),
            pages::overlay::menu_open_mode_label(state.open_mode()),
            bool_label(state.disabled())
        ))
        .child(format!(
            "items: {} / active {} / focused {}",
            state.items().len(),
            active_items,
            focused
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {}",
            pages::overlay::outside_press_label(state.outside_press_policy()),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event())
        ))
        .child(format!(
            "escape: {} / layer {}",
            pages::overlay::escape_key_label(state.escape_key_policy()),
            pages::overlay::layer_kind_label(state.overlay().policy().kind())
        ))
}

fn context_menu_state_row(state: &open_gpui_ui_components::ContextMenuState) -> impl IntoElement {
    let menu = state.menu();
    let focused = menu.focused_value().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / focused {}",
            bool_label(state.open()),
            pages::overlay::menu_open_mode_label(state.open_mode()),
            focused
        ))
        .child(format!(
            "anchor: {} x {} / snap {}",
            format_px(state.anchor_point().x),
            format_px(state.anchor_point().y),
            format_px(state.overlay().snap_margin())
        ))
        .child(format!(
            "items: {} / layer {} / outside {}",
            menu.items().len(),
            pages::overlay::layer_kind_label(state.overlay().policy().kind()),
            pages::overlay::outside_press_label(menu.outside_press_policy())
        ))
}

fn tooltip_open_intent_label(intent: TooltipOpenIntent) -> &'static str {
    match intent {
        TooltipOpenIntent::HoverOrFocus => "hover or focus",
        TooltipOpenIntent::Hover => "hover",
        TooltipOpenIntent::Focus => "focus",
        TooltipOpenIntent::Manual => "manual",
    }
}

fn tooltip_content_kind_label(kind: TooltipContentKind) -> &'static str {
    match kind {
        TooltipContentKind::Text => "text",
        TooltipContentKind::Element => "element",
    }
}

fn tooltip_placement_label(side: open_gpui_ui_core::OverlayPlacementSide) -> &'static str {
    match side {
        open_gpui_ui_core::OverlayPlacementSide::Top => "top",
        open_gpui_ui_core::OverlayPlacementSide::Right => "right",
        open_gpui_ui_core::OverlayPlacementSide::Bottom => "bottom",
        open_gpui_ui_core::OverlayPlacementSide::Left => "left",
    }
}

fn format_duration_ms(duration: std::time::Duration) -> String {
    format!("{}ms", duration.as_millis())
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

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
