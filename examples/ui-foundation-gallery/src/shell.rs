//! Gallery shell that consumes the UI foundation directly.

use open_gpui::prelude::*;

use open_gpui::{
    Anchor, App, AppContext, Bounds, Context, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Render, ScrollAnchor, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, anchored, deferred,
    div, px, rgb, size,
};

use open_gpui_ui_components::{
    AlertDialog, Avatar, AvatarState, BadgeState, Button, ButtonState, ButtonVariant, Checkbox,
    CheckboxState, ColorIntent, Combobox, ComboboxGroup, ComboboxOpenMode, ComboboxOption,
    ComboboxState, Command, CommandGroup, CommandItem, CommandOpenMode, CommandState, ContextMenu,
    Dialog, Field, FieldState, FocusRing, HoverCard, IconButtonState, Kbd, KbdState, Label,
    LabelState, Listbox, ListboxGroup, ListboxOption, ListboxState, Menu, MenuItem,
    OverlayResolvedState, Popover, Progress, ProgressState, ScrollArea, Select, SelectOpenMode,
    SelectState, Separator, SeparatorState, Sheet, Skeleton, SkeletonState, SwitchState, TextInput,
    TextInputState, ToggleState, Tooltip,
    gpui_adapter::{
        DEFAULT_OVERLAY_SAFE_MARGIN, TextInputController, UiA11yElementExt, focus_ring_shadow,
        gpui_overlay_state, gpui_point_from_ui, gpui_px_from_ui, init_text_input,
    },
};

use open_gpui_ui_core::{
    AccessibleAction, Density, DeviceAdaptivePolicy, DeviceShellMode, DeviceShellSwitchPolicy,
    Orientation, Rect, Role, Sizable, Size, ThemeTokens, Toggled, UiPx,
};

use crate::pages::{
    self, GALLERY_SECTIONS, GalleryPage, focus_a11y::FocusA11yPageState, overlay::OverlayPageState,
};

/// Default gallery window width.

pub const DEFAULT_GALLERY_WIDTH: Pixels = px(1040.0);

/// Default gallery window height.

pub const DEFAULT_GALLERY_HEIGHT: Pixels = px(680.0);

/// Compact gallery width used by the manual adaptive switch.

pub const COMPACT_GALLERY_WIDTH: Pixels = px(720.0);

/// Desktop gallery width used by the manual adaptive switch.

pub const DESKTOP_GALLERY_WIDTH: Pixels = DEFAULT_GALLERY_WIDTH;

const GALLERY_SAMPLE_MOUNT_OPEN: bool = false;

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
    let neutral_width = ui_px_from_gpui(width);

    let shell_mode = DeviceShellSwitchPolicy::default().mode(neutral_width);

    let density = DeviceAdaptivePolicy::default()
        .classify(neutral_width)
        .density();

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
    root_focus: FocusHandle,
    page_scroll_handle: ScrollHandle,
    editable_text_input: open_gpui::Entity<TextInputController>,
    focus_controls: [FocusHandle; 3],
    tooltip_focus_controls: [FocusHandle; 4],
    focus_a11y: FocusA11yPageState,
    overlay: OverlayPageState,
}

impl GalleryShell {
    fn build(selected_page: GalleryPage, cx: &mut Context<Self>) -> Self {
        Self {
            selected_page,

            width: DEFAULT_GALLERY_WIDTH,

            root_focus: cx.focus_handle(),
            page_scroll_handle: ScrollHandle::new(),

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
            focus_a11y: FocusA11yPageState::default(),
            overlay: OverlayPageState::default(),
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

    pub(crate) fn editable_text_input(&self) -> &open_gpui::Entity<TextInputController> {
        &self.editable_text_input
    }

    pub(crate) fn page_scroll_handle(&self) -> &ScrollHandle {
        &self.page_scroll_handle
    }

    /// Returns the current foundation snapshot.

    pub fn snapshot(&self) -> GalleryShellSnapshot {
        foundation_snapshot(self.width, self.selected_page)
    }

    fn select_page(&mut self, page: GalleryPage, cx: &mut Context<Self>) {
        if self.selected_page != page {
            self.selected_page = page;
            self.overlay.reset_on_page_change();
            cx.notify();
        }
    }

    fn set_viewport_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.width != width {
            self.width = width;

            cx.notify();
        }
    }

    fn mutate_focus_a11y(
        &mut self,
        mutate: impl FnOnce(&mut FocusA11yPageState) -> bool,
        cx: &mut Context<Self>,
    ) {
        if mutate(&mut self.focus_a11y) {
            cx.notify();
        }
    }

    fn mutate_overlay(
        &mut self,
        mutate: impl FnOnce(&mut OverlayPageState) -> bool,
        cx: &mut Context<Self>,
    ) {
        if mutate(&mut self.overlay) {
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
            .debug_selector(|| "gallery:shell".into())
            .size_full()
            .flex()
            .bg(rgb(0xf6f7f2))
            .text_color(rgb(0x18202a))
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    this.mutate_overlay(
                        |state| {
                            let mut changed = false;
                            changed |= state.set_overlay_open(false);
                            changed |= state.set_hovered_tooltip_sample(None);
                            changed |= state.close_controlled_overlays();
                            changed
                        },
                        cx,
                    );
                }
            }))
            .child(self.render_navigation(snapshot, page, cx))
            .child(self.render_content(snapshot, window, cx))
    }
}

impl GalleryShell {
    fn render_navigation(
        &self,

        snapshot: GalleryShellSnapshot,

        selected_page: GalleryPage,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("gallery-navigation")
            .debug_selector(|| "gallery:navigation".into())
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
                    .debug_selector(|| "gallery:navigation-scroll".into())
                    .flex_1()
                    .min_h(px(0.0))
                    .child(
                        ScrollArea::new(
                            "gallery-navigation-scroll-viewport",
                            div().flex().flex_col().gap_2().children(
                                GALLERY_SECTIONS.into_iter().map(|section| {
                                    let selected = section.page == selected_page;

                                    div()
                                        .id(section.id)
                                        .debug_selector(move || {
                                            format!("gallery:navigation-item:{}", section.id)
                                        })
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
                                }),
                            ),
                        )
                        .with_size(snapshot.control_size),
                    ),
            )
    }

    fn render_content(
        &self,

        snapshot: GalleryShellSnapshot,

        window: &mut Window,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let page = snapshot.selected_page;
        let component_page_anchors =
            pages::components::ComponentPageAnchors::new(self.page_scroll_handle());

        div()
            .id("gallery-content")
            .debug_selector(|| "gallery:content".into())
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
            .when(page == GalleryPage::Components, |this| {
                this.child(pages::components::render_components_directory(
                    &component_page_anchors,
                    snapshot,
                ))
            })
            .child(
                div()
                    .id("gallery-page-scroll")
                    .debug_selector(|| "gallery:page-scroll".into())
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(
                        ScrollArea::new(
                            "gallery-page-scroll-viewport",
                            self.render_page_body(snapshot, window, cx, &component_page_anchors),
                        )
                        .scroll_handle(&self.page_scroll_handle)
                        .with_size(snapshot.control_size)
                        .reset_on_key(snapshot.selected_page.id()),
                    ),
            )
    }

    fn render_page_body(
        &self,

        snapshot: GalleryShellSnapshot,

        window: &mut Window,

        cx: &mut Context<Self>,
        component_page_anchors: &pages::components::ComponentPageAnchors,
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

            GalleryPage::Components => {
                pages::components::render_components_page(self, snapshot, component_page_anchors)
                    .into_any_element()
            }
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
                            .border_color(if snapshot.control_size == sample.size {
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
                                    sample.default_size.as_str()
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
                            .border_color(
                                if gpui_px_from_ui(sample.width) == snapshot.viewport_width {
                                    rgb(0x1f7a66)
                                } else {
                                    rgb(0xd6d8ce)
                                },
                            )
                            .bg(
                                if gpui_px_from_ui(sample.width) == snapshot.viewport_width {
                                    rgb(0xe8f3ef)
                                } else {
                                    rgb(0xffffff)
                                },
                            )
                            .px_4()
                            .py_2()
                            .text_sm()
                            .text_color(rgb(0x263240))
                            .child(
                                div()
                                    .w(px(88.0))
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(format_ui_px(sample.width)),
                            )
                            .child(label_pill(sample.shell_mode.as_str()))
                            .child(label_pill(sample.class.as_str()))
                            .child(label_pill(sample.density.as_str()))
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
                                        .child(format_ui_px(sample.width)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(label_pill(sample.class.as_str())),
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
        let a11y = self.focus_a11y.demo_state();
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
                            .ui_role(Role::SpinButton)
                            .aria_label(format!("Counter {}", self.focus_a11y.counter()))
                            .aria_numeric_value(self.focus_a11y.counter() as f64)
                            .aria_min_numeric_value(0.0)
                            .on_ui_a11y_action(AccessibleAction::Increment, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity
                                        .update(cx, |this, cx| {
                                            this.mutate_focus_a11y(
                                                |state| state.increment_counter(),
                                                cx,
                                            )
                                        })
                                        .ok();
                                }
                            })
                            .on_ui_a11y_action(AccessibleAction::Decrement, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity
                                        .update(cx, |this, cx| {
                                            this.mutate_focus_a11y(
                                                |state| state.decrement_counter(),
                                                cx,
                                            )
                                        })
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
                                this.mutate_focus_a11y(|state| state.increment_counter(), cx);
                            }))
                            .child(format!("counter: {}", self.focus_a11y.counter())),
                    )
                    .child(
                        div()
                            .id("gallery-a11y-reset")
                            .focusable()
                            .tab_stop(true)
                            .ui_role(Role::Button)
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
                                this.mutate_focus_a11y(|state| state.reset_counter(), cx);
                            }))
                            .child("reset counter"),
                    )
                    .child(
                        div()
                            .id("gallery-a11y-switch")
                            .focusable()
                            .tab_stop(true)
                            .ui_role(Role::Switch)
                            .aria_label("Enable foundation switch")
                            .ui_aria_toggled(a11y.toggled)
                            .w(px(224.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded_sm()
                            .border_1()
                            .border_color(if self.focus_a11y.enabled() {
                                rgb(0x1f7a66)
                            } else {
                                rgb(0xd6d8ce)
                            })
                            .bg(if self.focus_a11y.enabled() {
                                rgb(0xe8f3ef)
                            } else {
                                rgb(0xffffff)
                            })
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mutate_focus_a11y(|state| state.toggle_enabled(), cx);
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
                    .child(self.focus_a11y.focus_message()),
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
            .ui_role(spec.role)
            .aria_label(spec.label)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0xf1f5ee)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.mutate_focus_a11y(|state| state.set_focus_message(spec.label), cx);
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

        let hover_card_samples = pages::overlay::hover_card_samples(snapshot.tokens);

        let popover_samples = pages::overlay::popover_samples(snapshot.tokens);

        let dialog_samples = pages::overlay::dialog_samples(snapshot.tokens);

        let alert_dialog_samples = pages::overlay::alert_dialog_samples(snapshot.tokens);

        let sheet_samples = pages::overlay::sheet_samples(snapshot.tokens);

        let menu_samples = pages::overlay::menu_samples(snapshot.tokens);

        let context_menu_samples = pages::overlay::context_menu_samples(snapshot.tokens);

        let overlay_catalog_cards = pages::overlay::OVERLAY_CATALOG
            .iter()
            .map(overlay_catalog_card);

        div()
            .id("gallery-overlay-page")
            .debug_selector(|| "gallery:overlay-page".into())
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

                                    .left(gpui_px_from_ui(geometry.trigger_point.x))

                                    .top(gpui_px_from_ui(geometry.trigger_point.y))

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
                                        this.mutate_overlay(|state| state.set_overlay_open(true), cx);
                                    }))
                                    .child("open overlay")
                                    .when(self.overlay.overlay_open(), |trigger| {
                                        trigger.child(
                                            deferred(
                                                anchored()
                                                    .anchor(Anchor::TopLeft)

                                                    .position(gpui_point_from_ui(

                                                        geometry.anchor_rect.origin,

                                                    ))

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

                                                                        format_ui_px(

                                                                            geometry

                                                                                .anchor_rect

                                                                                .size

                                                                                .width

                                                                        ),

                                                                        format_ui_px(

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
                                                                            this.mutate_overlay(
                                                                                |state| state
                                                                                    .set_overlay_open(
                                                                                        false,
                                                                                    ),
                                                                                cx,
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
                                    .bg(if self.overlay.overlay_open() {
                                        rgb(0xe8f3ef)
                                    } else {
                                        rgb(0xffffff)
                                    })
                                    .text_sm()
                                    .child(if self.overlay.overlay_open() { "open" } else { "closed" }),
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
                            .child("Overlay catalog"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .flex_wrap()
                            .children(overlay_catalog_cards),
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

                            .child("HoverCard samples"),

                    )

                    .child(

                        div()

                            .grid()
                            .grid_cols(3)
                            .gap_3()
                            .child(self.render_hover_card_sample_card(
                                &hover_card_samples[0],
                                false,
                                cx,
                            ))
                            .child(self.render_hover_card_sample_card(

                                &hover_card_samples[1],

                                false,

                                cx,

                            ))

                            .child(
                                self.render_hover_card_sample_card(
                                    &hover_card_samples[2],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::HoverCard,
                                    ),
                                    cx,
                                ),
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
                            .child(
                                self.render_popover_sample_card(
                                    &popover_samples[1],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::Popover,
                                    ),
                                    cx,
                                ),
                            )
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
                            .child(
                                self.render_dialog_sample_card(
                                    &dialog_samples[0],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::Dialog,
                                    ),
                                    cx,
                                ),
                            )
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

                            .child("AlertDialog samples"),

                    )

                    .child(

                        div()

                            .grid()

                            .grid_cols(2)

                            .gap_3()
                            .child(
                                self.render_alert_dialog_sample_card(
                                    &alert_dialog_samples[0],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::AlertDialog,
                                    ),
                                    cx,
                                ),
                            )
                            .child(self.render_alert_dialog_sample_card(

                                &alert_dialog_samples[1],

                                false,

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

                            .child("Sheet samples"),

                    )

                    .child(

                        div()

                            .grid()

                            .grid_cols(3)

                            .gap_3()
                            .child(self.render_sheet_sample_card(&sheet_samples[0], false, cx))
                            .child(
                                self.render_sheet_sample_card(
                                    &sheet_samples[1],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::Sheet,
                                    ),
                                    cx,
                                ),
                            )
                            .child(self.render_sheet_sample_card(&sheet_samples[2], false, cx)),

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
                            .child(
                                self.render_menu_sample_card(
                                    &menu_samples[1],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::Menu,
                                    ),
                                    cx,
                                ),
                            )
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
                            .child(
                                self.render_context_menu_sample_card(
                                    &context_menu_samples[1],
                                    self.overlay.is_controlled_open(
                                        pages::overlay::OverlayControlledSample::ContextMenu,
                                    ),
                                    cx,
                                ),
                            )
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

        let debug_selector = sample.debug_selector();

        let label = sample.label;

        let tooltip_text = sample.tooltip_text;

        let focused =
            focus_handle_is_focused && state.open_intent().opens_on_focus() && !state.disabled();

        let hovered = self.overlay.hovered_tooltip_sample() == Some(sample_id)
            && state.open_intent().opens_on_hover()
            && !state.disabled();
        let forced_open = state.open() && !state.disabled();

        let open = focused || hovered || forced_open;

        let focus_ring = FocusRing::from_color(ColorIntent::new(
            ThemeTokens::default().focus_ring,
            0x2f80ed,
        ));

        overlay_sample_card_shell(
            format!("overlay-tooltip-sample:{}", sample_id),
            Some(debug_selector),
        )
        .child(
            div()
                .id(format!("overlay-tooltip-trigger:{}", sample_id))
                .debug_selector(move || format!("gallery:overlay-tooltip-trigger:{sample_id}"))
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
                .ui_role(Role::Button)
                .aria_label(label)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0xf1f5ee)))
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.mutate_overlay(
                        |state| state.set_hovered_tooltip_sample(hovered.then_some(sample_id)),
                        cx,
                    );
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

    fn render_hover_card_sample_card(
        &self,

        sample: &pages::overlay::HoverCardSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::HoverCardOpenMode::Controlled
        ) {
            HoverCard::new(
                format!("overlay-hover-card-sample:{}", sample.id),
                sample.label,
                sample.content_text,
            )
            .open(controlled_open)
            .open_intent(sample.state.open_intent())
            .delay(sample.state.delay())
            .outside_press_policy(sample.state.outside_press_policy())
            .placement_side(sample.state.placement_side())
            .placement_alignment(sample.state.placement_alignment())
            .state()
        } else {
            sample.state.clone()
        };

        let sample_id = sample.id;

        let debug_selector = sample.debug_selector();

        let label = sample.label;

        let content_text = sample.content_text;

        let forced_open = state.open() && !state.disabled();

        let effective_open = forced_open;

        let shell = cx.entity().downgrade();

        let hover_card = HoverCard::new(
            format!("overlay-hover-card-demo:{}", sample_id),
            label,
            content_text,
        )
        .open_intent(state.open_intent())
        .delay(state.delay())
        .outside_press_policy(state.outside_press_policy())
        .placement_side(state.placement_side())
        .placement_alignment(state.placement_alignment())
        .with_size(state.size());
        let hover_card = match state.open_mode() {
            open_gpui_ui_components::HoverCardOpenMode::Controlled => hover_card
                .open(state.open())
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::HoverCard,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                }),
            open_gpui_ui_components::HoverCardOpenMode::Uncontrolled => hover_card,
        };

        overlay_sample_card_shell(
            format!("overlay-hover-card-sample-card:{}", sample_id),
            Some(debug_selector),
        )
        .child(hover_card)
        .when(
            matches!(
                state.open_mode(),
                open_gpui_ui_components::HoverCardOpenMode::Controlled
            ),
            |card| {
                card.child(
                    div()
                        .id("overlay-hover-card-controlled-toggle")
                        .debug_selector(|| {
                            "gallery:overlay-hover-card-controlled-toggle".to_string()
                        })
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::HoverCard,
                                        !controlled_open,
                                    )
                                },
                                cx,
                            );
                        }))
                        .child(if controlled_open {
                            "close hover card"
                        } else {
                            "open hover card"
                        }),
                )
            },
        )
        .child(hover_card_state_row(&state, effective_open))
    }

    fn render_popover_sample_card(
        &self,

        sample: &pages::overlay::PopoverSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::PopoverOpenMode::Controlled
        ) {
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

        let debug_selector = sample.debug_selector();

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

        let popover = match state.open_mode() {
            open_gpui_ui_components::PopoverOpenMode::Controlled => popover
                .open(state.open())
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::Popover,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                }),

            open_gpui_ui_components::PopoverOpenMode::Uncontrolled => popover,
        };

        overlay_sample_card_shell(
            format!("overlay-popover-sample-card:{}", sample_id),
            Some(debug_selector),
        )
        .child(popover)
        .when(
            matches!(
                state.open_mode(),
                open_gpui_ui_components::PopoverOpenMode::Controlled
            ),
            |card| {
                card.child(
                    div()
                        .id("overlay-popover-controlled-toggle")
                        .debug_selector({
                            let sample_id = sample_id.to_owned();

                            move || format!("gallery:overlay-popover-control:{sample_id}")
                        })
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::Popover,
                                        !controlled_open,
                                    )
                                },
                                cx,
                            );
                        }))
                        .child(if controlled_open {
                            "close controlled"
                        } else {
                            "open controlled"
                        }),
                )
            },
        )
        .child(popover_state_row(&state))
    }

    fn render_dialog_sample_card(
        &self,

        sample: &pages::overlay::DialogSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::DialogOpenMode::Controlled
        ) {
            Dialog::new(
                format!("overlay-dialog-sample:{}", sample.id),
                sample.label,
                sample.state.title(),
                sample.content_text,
            )
            .description(
                sample
                    .state
                    .description()
                    .expect("controlled dialog sample should define a description"),
            )
            .open(controlled_open)
            .outside_press_policy(sample.state.outside_press_policy())
            .escape_key_policy(sample.state.escape_key_policy())
            .state()
        } else {
            sample.state.clone()
        };

        let sample_id = sample.id;

        let debug_selector = sample.debug_selector();

        let label = sample.label;

        let content_text = sample.content_text;

        let shell = cx.entity().downgrade();

        let dialog = Dialog::new(
            format!("overlay-dialog-demo:{}", sample_id),
            label,
            sample.state.title(),
            content_text,
        )
        .disabled(state.disabled())
        .outside_press_policy(state.outside_press_policy())
        .escape_key_policy(state.escape_key_policy());

        let dialog = match state.open_mode() {
            open_gpui_ui_components::DialogOpenMode::Controlled => dialog
                .open(state.open())
                .description(
                    state
                        .description()
                        .expect("controlled dialog sample should define a description"),
                )
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::Dialog,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                }),

            open_gpui_ui_components::DialogOpenMode::Uncontrolled => dialog,
        };

        overlay_sample_card_shell(
            format!("overlay-dialog-sample-card:{}", sample_id),
            Some(debug_selector),
        )
        .child(dialog)
        .when(
            matches!(
                state.open_mode(),
                open_gpui_ui_components::DialogOpenMode::Controlled
            ),
            |card| {
                card.child(
                    div()
                        .id("overlay-dialog-controlled-toggle")
                        .debug_selector({
                            let sample_id = sample_id.to_owned();

                            move || format!("gallery:overlay-dialog-control:{sample_id}")
                        })
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::Dialog,
                                        !controlled_open,
                                    )
                                },
                                cx,
                            );
                        }))
                        .child(if controlled_open {
                            "close dialog"
                        } else {
                            "open dialog"
                        }),
                )
            },
        )
        .child(dialog_state_row(&state))
    }

    fn render_alert_dialog_sample_card(
        &self,

        sample: &pages::overlay::AlertDialogSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::AlertDialogOpenMode::Controlled
        ) {
            AlertDialog::new(
                format!("overlay-alert-dialog-sample:{}", sample.id),
                sample.label,
                sample.state.title(),
                sample.state.description(),
                sample.state.action().label(),
            )
            .cancel_label(sample.state.cancel().label().to_owned())
            .intent(sample.state.intent())
            .open(controlled_open)
            .outside_press_policy(sample.state.outside_press_policy())
            .escape_key_policy(sample.state.escape_key_policy())
            .state()
        } else {
            sample.state.clone()
        };

        let sample_id = sample.id;

        let debug_selector = sample.debug_selector();

        let shell = cx.entity().downgrade();

        let alert_dialog = AlertDialog::new(
            format!("overlay-alert-dialog-demo:{}", sample_id),
            sample.label,
            sample.state.title(),
            sample.state.description(),
            sample.state.action().label(),
        )
        .cancel_label(state.cancel().label().to_owned())
        .intent(state.intent())
        .disabled(state.disabled())
        .outside_press_policy(state.outside_press_policy())
        .escape_key_policy(state.escape_key_policy());

        let alert_dialog = match state.open_mode() {
            open_gpui_ui_components::AlertDialogOpenMode::Controlled => alert_dialog
                .open(state.open())
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::AlertDialog,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                }),

            open_gpui_ui_components::AlertDialogOpenMode::Uncontrolled => alert_dialog,
        };

        overlay_sample_card_shell(
            format!("overlay-alert-dialog-sample-card:{}", sample_id),
            Some(debug_selector),
        )
        .child(alert_dialog)
        .when(
            matches!(
                state.open_mode(),
                open_gpui_ui_components::AlertDialogOpenMode::Controlled
            ),
            |card| {
                card.child(
                    div()
                        .id("overlay-alert-dialog-controlled-toggle")
                        .debug_selector({
                            let sample_id = sample_id.to_owned();

                            move || format!("gallery:overlay-alert-dialog-control:{sample_id}")
                        })
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xd6d8ce))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::AlertDialog,
                                        !controlled_open,
                                    )
                                },
                                cx,
                            );
                        }))
                        .child(if controlled_open {
                            "close alert"
                        } else {
                            "open alert"
                        }),
                )
            },
        )
        .child(alert_dialog_state_row(&state))
    }

    fn render_sheet_sample_card(
        &self,

        sample: &pages::overlay::SheetSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::SheetOpenMode::Controlled
        ) {
            Sheet::new(
                format!("overlay-sheet-sample:{}", sample.id),
                sample.label,
                sample.state.title(),
                sample.content_text,
            )
            .description(
                sample
                    .state
                    .description()
                    .expect("right-non-modal sheet sample should define a description"),
            )
            .open(controlled_open)
            .side(sample.state.side())
            .modal_mode(sample.state.modal_mode())
            .outside_press_policy(sample.state.outside_press_policy())
            .state()
        } else {
            sample.state.clone()
        };

        let sample_id = sample.id;

        let debug_selector = sample.debug_selector();

        let shell = cx.entity().downgrade();

        let sheet = Sheet::new(
            format!("overlay-sheet-demo:{}", sample_id),
            sample.label,
            sample.state.title(),
            sample.content_text,
        )
        .disabled(state.disabled())
        .side(state.side())
        .modal_mode(state.modal_mode())
        .close_affordance(state.close_affordance())
        .outside_press_policy(state.outside_press_policy())
        .escape_key_policy(state.escape_key_policy());

        let sheet = if let Some(description) = state.description() {
            sheet.description(description.to_owned())
        } else {
            sheet
        };

        let sheet = match state.open_mode() {
            open_gpui_ui_components::SheetOpenMode::Controlled => {
                sheet.open(state.open()).on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::Sheet,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                })
            }

            open_gpui_ui_components::SheetOpenMode::Uncontrolled => sheet,
        };

        div()
            .id(format!("overlay-sheet-sample-card:{}", sample_id))
            .debug_selector(move || debug_selector)
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
            .child(sheet)
            .when(
                matches!(
                    state.open_mode(),
                    open_gpui_ui_components::SheetOpenMode::Controlled
                ),
                |card| {
                    card.child(
                        div()
                            .id("overlay-sheet-controlled-toggle")
                            .debug_selector({
                                let sample_id = sample_id.to_owned();

                                move || format!("gallery:overlay-sheet-control:{sample_id}")
                            })
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mutate_overlay(
                                    |state| {
                                        state.set_controlled_open(
                                            pages::overlay::OverlayControlledSample::Sheet,
                                            !controlled_open,
                                        )
                                    },
                                    cx,
                                );
                            }))
                            .child(if controlled_open {
                                "close sheet"
                            } else {
                                "open sheet"
                            }),
                    )
                },
            )
            .child(sheet_state_row(&state))
    }

    fn render_menu_sample_card(
        &self,

        sample: &pages::overlay::MenuSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state_items = resolved_menu_items(sample.state.items());

        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::MenuOpenMode::Controlled
        ) {
            let focused_value = sample.focused_value;

            let menu = Menu::new(format!("overlay-menu-sample:{}", sample.id), sample.label)
                .open(controlled_open);

            let menu = menu.when_some(focused_value, |menu, focused_value| {
                menu.default_focused_value(focused_value)
            });

            menu.items(state_items.clone()).state()
        } else {
            sample.state.clone()
        };

        let sample_id = sample.id;

        let debug_selector = sample.debug_selector();

        let label = sample.label;

        let shell = cx.entity().downgrade();

        let focused_value = sample.focused_value;

        let menu = Menu::new(format!("overlay-menu-demo:{}", sample_id), label)
            .items(state_items)
            .disabled(state.disabled())
            .outside_press_policy(state.outside_press_policy())
            .escape_key_policy(state.escape_key_policy());

        let menu = menu.when_some(focused_value, |menu, focused_value| {
            menu.default_focused_value(focused_value)
        });

        let menu = match state.open_mode() {
            open_gpui_ui_components::MenuOpenMode::Controlled => {
                menu.open(state.open()).on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::Menu,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                })
            }

            open_gpui_ui_components::MenuOpenMode::Uncontrolled => menu,
        };

        div()
            .id(format!("overlay-menu-sample-card:{}", sample_id))
            .debug_selector(move || debug_selector)
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
            .when(
                matches!(
                    state.open_mode(),
                    open_gpui_ui_components::MenuOpenMode::Controlled
                ),
                |card| {
                    card.child(
                        div()
                            .id("overlay-menu-controlled-toggle")
                            .debug_selector({
                                let sample_id = sample_id.to_owned();

                                move || format!("gallery:overlay-menu-control:{sample_id}")
                            })
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mutate_overlay(
                                    |state| {
                                        state.set_controlled_open(
                                            pages::overlay::OverlayControlledSample::Menu,
                                            !controlled_open,
                                        )
                                    },
                                    cx,
                                );
                            }))
                            .child(if controlled_open {
                                "close menu"
                            } else {
                                "open menu"
                            }),
                    )
                },
            )
            .child(menu_state_row(&state))
    }

    fn render_context_menu_sample_card(
        &self,

        sample: &pages::overlay::ContextMenuSample,

        controlled_open: bool,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state_items = resolved_menu_items(sample.state.menu().items());

        let state = if matches!(
            sample.state.open_mode(),
            open_gpui_ui_components::MenuOpenMode::Controlled
        ) {
            let focused_value = sample.focused_value;

            let context_menu = ContextMenu::new(
                format!("overlay-context-menu-sample:{}", sample.id),
                sample.label,
            )
            .open(controlled_open);

            let context_menu = context_menu
                .when_some(focused_value, |context_menu, focused_value| {
                    context_menu.default_focused_value(focused_value)
                });

            context_menu
                .anchor_point(gpui_point_from_ui(sample.state.anchor_point()))
                .items(state_items.clone())
                .state()
        } else {
            sample.state.clone()
        };

        let sample_id = sample.id;

        let debug_selector = sample.debug_selector();

        let label = sample.label;

        let shell = cx.entity().downgrade();

        let focused_value = sample.focused_value;

        let context_menu =
            ContextMenu::new(format!("overlay-context-menu-demo:{}", sample_id), label)
                .items(state_items)
                .anchor_point(gpui_point_from_ui(state.anchor_point()))
                .outside_press_policy(state.menu().outside_press_policy())
                .escape_key_policy(state.menu().escape_key_policy());

        let context_menu = context_menu.when_some(focused_value, |context_menu, focused_value| {
            context_menu.default_focused_value(focused_value)
        });

        let context_menu = match state.open_mode() {
            open_gpui_ui_components::MenuOpenMode::Controlled => context_menu
                .open(state.open())
                .on_open_change(move |open, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            this.mutate_overlay(
                                |state| {
                                    state.set_controlled_open(
                                        pages::overlay::OverlayControlledSample::ContextMenu,
                                        open,
                                    )
                                },
                                cx,
                            )
                        })
                        .ok();
                }),

            open_gpui_ui_components::MenuOpenMode::Uncontrolled => context_menu,
        };

        div()
            .id(format!("overlay-context-menu-sample-card:{}", sample_id))
            .debug_selector(move || debug_selector)
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
            .when(
                matches!(
                    state.open_mode(),
                    open_gpui_ui_components::MenuOpenMode::Controlled
                ),
                |card| {
                    card.child(
                        div()
                            .id("overlay-context-menu-controlled-toggle")
                            .debug_selector({
                                let sample_id = sample_id.to_owned();

                                move || format!("gallery:overlay-context-menu-control:{sample_id}")
                            })
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mutate_overlay(
                                    |state| {
                                        state.set_controlled_open(
                                            pages::overlay::OverlayControlledSample::ContextMenu,
                                            !controlled_open,
                                        )
                                    },
                                    cx,
                                );
                            }))
                            .child(if controlled_open {
                                "close context menu"
                            } else {
                                "open context menu"
                            }),
                    )
                },
            )
            .child(context_menu_state_row(&state))
    }

    fn render_overlay_bounds(&self, label: &'static str, bounds: Rect) -> impl IntoElement {
        div()
            .absolute()
            .left(gpui_px_from_ui(bounds.origin.x))
            .top(gpui_px_from_ui(bounds.origin.y))
            .w(gpui_px_from_ui(bounds.size.width))
            .h(gpui_px_from_ui(bounds.size.height))
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

    pub(crate) fn render_signal_list(&self, page: GalleryPage) -> impl IntoElement {
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

    fn render_metric(&self, label: &'static str, value: impl DisplayPx) -> impl IntoElement {
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
            .child(format!("shell: {}", snapshot.shell_mode.as_str()))
            .child(format!("density: {}", snapshot.density.as_str()))
            .child(format!("size: {}", snapshot.control_size.as_str()))
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
            .debug_selector(move || format!("gallery:viewport-switch:{label}"))
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

pub(crate) fn label_pill(label: &'static str) -> impl IntoElement {
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

pub(crate) fn component_catalog_status_pill(
    status: pages::components::ComponentCatalogStatus,
) -> impl IntoElement {
    let (background, border, text) = status.badge_colors();

    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(background))
        .text_xs()
        .text_color(rgb(text))
        .child(status.as_str())
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

pub(crate) fn component_button_state_row(state: ButtonState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.variant().as_str(),
            state.size().as_str(),
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

pub(crate) fn component_badge_state_row(state: BadgeState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / display",
            state.variant().as_str(),
            state.size().as_str()
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().min_height()),
            format_px(state.metrics().padding_x())
        ))
}

pub(crate) fn component_separator_state_row(state: SeparatorState) -> impl IntoElement {
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
            if state.decorative() {
                "decorative"
            } else {
                "semantic"
            },
            state.size().as_str()
        ))
        .child(format!(
            "role {} / thickness {}",
            state
                .role()
                .map(|role| format!("{role:?}"))
                .unwrap_or_else(|| "none".to_owned()),
            format_px(state.metrics().thickness())
        ))
}

pub(crate) fn component_kbd_state_row(state: KbdState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!("{} / {}", state.label(), state.size().as_str()))
        .child(format!(
            "min {} px {}",
            format_px(state.metrics().min_width()),
            format_px(state.metrics().padding_x())
        ))
}

pub(crate) fn component_progress_state_row(state: ProgressState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {}",
            state.role(),
            state.size().as_str(),
            if state.indeterminate() {
                "indeterminate".to_owned()
            } else {
                format!("{:.0}%", state.value_percent().unwrap_or(0.0))
            }
        ))
        .child(format!(
            "h {} radius {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().radius())
        ))
        .child(format!(
            "indicator start {:.0}% width {:.0}%",
            state.indicator_start_fraction() * 100.0,
            state.indicator_fraction() * 100.0
        ))
}

pub(crate) fn component_skeleton_state_row(state: SkeletonState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {}",
            state.size().as_str(),
            if state.subtle() { "subtle" } else { "default" }
        ))
        .child(format!(
            "{} x {} / radius {}",
            format_px(state.metrics().width()),
            format_px(state.metrics().height()),
            format_px(state.metrics().radius())
        ))
}

pub(crate) fn component_avatar_state_row(state: &AvatarState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / fallback {} / {}",
            state.size().as_str(),
            state.fallback(),
            if state.has_source() {
                "source"
            } else {
                "fallback"
            }
        ))
        .child(format!(
            "{:?} / aria {} / box {}",
            state.role(),
            state.accessible_label(),
            format_px(state.metrics().diameter())
        ))
}

pub(crate) fn component_icon_button_state_row(
    accessible_label: &str,

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
            state.size().as_str(),
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

pub(crate) fn component_switch_state_row(state: SwitchState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            toggled_label_text(state.toggled()),
            state.size().as_str(),
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

pub(crate) fn component_checkbox(
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

pub(crate) fn component_checkbox_state_row(state: CheckboxState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            toggled_label_text(state.toggled()),
            state.size().as_str(),
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

pub(crate) fn component_label(id: String, state: &LabelState, tokens: ThemeTokens) -> Label {
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

pub(crate) fn component_label_state_row(state: &LabelState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.size().as_str(),
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

pub(crate) fn component_text_input(
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

pub(crate) fn component_field(
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

pub(crate) fn component_text_input_state_row(state: &TextInputState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.size().as_str(),
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
        .child(if state.controller_driven() {
            "controller"
        } else {
            "static"
        })
}

pub(crate) fn component_field_state_row(
    field: &FieldState,

    input: &TextInputState,
) -> impl IntoElement {
    let support = field.support_text().unwrap_or("no support text");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            field.size().as_str(),
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

pub(crate) fn gallery_card_shell(
    id: impl Into<open_gpui::ElementId>,

    debug_selector: Option<String>,
) -> open_gpui::Stateful<open_gpui::Div> {
    let card = div().id(id);

    let card = match debug_selector {
        Some(debug_selector) => card.debug_selector(move || debug_selector),

        None => card,
    };

    card.rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .p_3()
}

fn overlay_sample_card_shell(
    id: impl Into<open_gpui::ElementId>,

    debug_selector: Option<String>,
) -> open_gpui::Stateful<open_gpui::Div> {
    gallery_card_shell(id, debug_selector)
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap_3()
        .text_xs()
        .text_color(rgb(0x3f4a57))
}

pub(crate) fn component_primitive_samples_section(
    separators: [pages::components::SeparatorSample; 3],

    kbds: [pages::components::KbdSample; 3],

    progress: [pages::components::ProgressSample; 3],

    skeletons: [pages::components::SkeletonSample; 3],

    avatars: [pages::components::AvatarSample; 4],

    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Low-state primitives"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(separators.into_iter().map(move |sample| {
                    let state = sample.state;

                    let debug_selector = sample.debug_selector();

                    let separator = Separator::new(format!("component-separator:{}", sample.id))
                        .orientation(state.orientation())
                        .decorative(state.decorative())
                        .with_size(state.size())
                        .tokens(tokens);

                    gallery_card_shell(
                        format!("component-separator-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(220.0))
                    .min_h(px(132.0))
                    .flex()
                    .flex_col()
                    .gap_2()
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
                            .child(label_pill(match state.orientation() {
                                Orientation::Horizontal => "horizontal",

                                Orientation::Vertical => "vertical",
                            })),
                    )
                    .child(
                        div()
                            .h(px(46.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xe2e4dc))
                            .bg(rgb(0xfcfcf8))
                            .child(if state.orientation() == Orientation::Vertical {
                                div().h_full().child(separator).into_any_element()
                            } else {
                                div().w_full().child(separator).into_any_element()
                            }),
                    )
                    .child(component_separator_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(kbds.into_iter().map(move |sample| {
                    let debug_selector = sample.debug_selector();

                    let state = sample.state;

                    gallery_card_shell(
                        format!("component-kbd-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(170.0))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_2()
                    .child(
                        Kbd::new(format!("component-kbd:{}", sample.id), state.label())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_kbd_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(progress.into_iter().map(move |sample| {
                    let state = sample.state;

                    let debug_selector = sample.debug_selector();

                    let progress =
                        Progress::new(format!("component-progress:{}", sample.id), sample.label)
                            .with_size(state.size())
                            .tokens(tokens);

                    let progress = match state.value_percent() {
                        Some(value) => progress.value(value),

                        None => progress.indeterminate(),
                    };

                    gallery_card_shell(
                        format!("component-progress-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(280.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(sample.label),
                    )
                    .child(progress)
                    .child(component_progress_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(skeletons.into_iter().map(move |sample| {
                    let state = sample.state;

                    let debug_selector = sample.debug_selector();

                    gallery_card_shell(
                        format!("component-skeleton-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(250.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(sample.title),
                    )
                    .child(
                        Skeleton::new(format!("component-skeleton:{}", sample.id))
                            .subtle(state.subtle())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_skeleton_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(avatars.into_iter().map(move |sample| {
                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let avatar_name = state.name().to_owned();

                    let accessible_label = state.accessible_label().to_owned();

                    let fallback = state.fallback().to_owned();

                    let source = state.source().map(|source| source.uri().to_owned());

                    let avatar = Avatar::new(
                        format!("component-avatar:{}", sample.id),
                        avatar_name.clone(),
                    )
                    .accessible_label(accessible_label.clone())
                    .with_size(state.size())
                    .tokens(tokens);

                    let avatar = match source {
                        Some(source) => avatar.source(source),

                        None => avatar,
                    };

                    let avatar = avatar.fallback(fallback);

                    gallery_card_shell(
                        format!("component-avatar-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(220.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div().flex().items_center().gap_3().child(avatar).child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(open_gpui::FontWeight::BOLD)
                                        .child(if avatar_name.trim().is_empty() {
                                            "Empty name".to_owned()
                                        } else {
                                            avatar_name.clone()
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(accessible_label),
                                ),
                        ),
                    )
                    .child(component_avatar_state_row(&state))
                })),
        )
}

pub(crate) fn component_page_section(
    id: &'static str,
    anchor: ScrollAnchor,
) -> open_gpui::Stateful<open_gpui::Div> {
    div()
        .id(format!("gallery-components-section:{id}"))
        .debug_selector(move || format!("gallery:components-section:{id}"))
        .anchor_scroll(Some(anchor))
}

pub(crate) fn component_page_jump(
    id: &'static str,
    label: &'static str,
    anchor: ScrollAnchor,
    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .id(format!("gallery-components-jump:{id}"))
        .debug_selector(move || format!("gallery:component-page-jump:{id}"))
        .flex_none()
        .child(
            Button::new(format!("gallery-components-jump-button:{id}"), label)
                .variant(ButtonVariant::Ghost)
                .with_size(Size::Small)
                .tokens(tokens)
                .on_click(move |_, _, _| anchor.scroll_now()),
        )
}

pub(crate) fn component_listbox_samples_section(
    samples: [pages::components::ListboxSample; 2],

    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Listbox"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let listbox_options: Vec<_> = state
                        .standalone_options()
                        .map(resolved_listbox_option)
                        .collect();

                    let listbox_groups: Vec<_> = state
                        .groups()
                        .iter()
                        .map(|group_state| resolved_listbox_group(group_state, &state))
                        .collect();

                    let mut listbox =
                        Listbox::new(format!("component-listbox:{}", sample.id), label.clone())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        listbox = listbox.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        listbox = listbox.active(active);
                    }

                    for option in listbox_options.iter() {
                        listbox = listbox.option(option.clone());
                    }

                    for group in listbox_groups.iter() {
                        listbox = listbox.group(group.clone());
                    }

                    div()
                        .id(format!("component-listbox-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
                        .w(px(320.0))
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
                                        .child(label.clone()),
                                )
                                .child(label_pill(state.size().as_str())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(listbox)
                        .child(component_listbox_state_row(&state))
                })),
        )
}

pub(crate) fn component_select_samples_section(
    samples: [pages::components::SelectSample; 3],

    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Select"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let title = label.clone();

                    let listbox_options: Vec<_> = state
                        .listbox()
                        .standalone_options()
                        .map(resolved_listbox_option)
                        .collect();

                    let listbox_groups: Vec<_> = state
                        .listbox()
                        .groups()
                        .iter()
                        .map(|group_state| resolved_listbox_group(group_state, state.listbox()))
                        .collect();

                    // Keep the gallery sample closed on mount so the page stays scrollable.

                    let mut select =
                        Select::new(format!("component-select:{}", sample.id), label.clone())
                            .placeholder(state.placeholder())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        select = select.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        select = select.active(active);
                    }

                    select = match state.open_mode() {
                        SelectOpenMode::Controlled => select.open(GALLERY_SAMPLE_MOUNT_OPEN),

                        SelectOpenMode::Uncontrolled => {
                            select.default_open(GALLERY_SAMPLE_MOUNT_OPEN)
                        }
                    };

                    for group in listbox_groups.iter() {
                        select = select.group(group.clone());
                    }

                    for option in listbox_options.iter() {
                        select = select.option(option.clone());
                    }

                    div()
                        .id(format!("component-select-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
                        .w(px(340.0))
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
                                .child(label_pill(if GALLERY_SAMPLE_MOUNT_OPEN {
                                    "mount open"
                                } else {
                                    "mount closed"
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(select)
                        .child(component_select_state_row(&state))
                })),
        )
}

pub(crate) fn component_combobox_samples_section(
    samples: [pages::components::ComboboxSample; 3],

    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Combobox"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let title = label.clone();

                    let combobox_options: Vec<_> = state
                        .listbox()
                        .standalone_options()
                        .map(resolved_combobox_option)
                        .collect();

                    let combobox_groups: Vec<_> = state
                        .listbox()
                        .groups()
                        .iter()
                        .map(|group_state| resolved_combobox_group(group_state, state.listbox()))
                        .collect();

                    // Keep the gallery sample closed on mount so the page stays scrollable.

                    let mut combobox =
                        Combobox::new(format!("component-combobox:{}", sample.id), label.clone())
                            .placeholder(state.placeholder())
                            .default_query(state.query())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        combobox = combobox.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        combobox = combobox.active(active);
                    }

                    combobox = match state.open_mode() {
                        ComboboxOpenMode::Controlled => combobox.open(GALLERY_SAMPLE_MOUNT_OPEN),

                        ComboboxOpenMode::Uncontrolled => {
                            combobox.default_open(GALLERY_SAMPLE_MOUNT_OPEN)
                        }
                    };

                    for option in combobox_options.iter() {
                        combobox = combobox.option(option.clone());
                    }

                    for group in combobox_groups.iter() {
                        combobox = combobox.group(group.clone());
                    }

                    div()
                        .id(format!("component-combobox-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
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
                                .child(label_pill(if GALLERY_SAMPLE_MOUNT_OPEN {
                                    "mount open"
                                } else {
                                    "mount closed"
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(combobox)
                        .child(component_combobox_state_row(&state))
                })),
        )
}

pub(crate) fn component_command_samples_section(
    samples: [pages::components::CommandSample; 3],

    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Command"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let title = label.clone();

                    let command_items: Vec<_> = state
                        .standalone_items()
                        .map(resolved_command_item)
                        .collect();

                    let command_groups: Vec<CommandGroup> = state
                        .grouped_groups()
                        .map(|group_state| resolved_command_group(group_state, &state))
                        .collect();

                    // Keep the gallery sample closed on mount so the page stays scrollable.

                    let mut command =
                        Command::new(format!("component-command:{}", sample.id), label.clone())
                            .placeholder(state.placeholder())
                            .default_query(state.query())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        command = command.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        command = command.active(active);
                    }

                    if let Some(dialog) = state.dialog() {
                        command = command.dialog(dialog.title());

                        if let Some(description) = dialog.description() {
                            command = command.dialog_description(description);
                        }
                    }

                    if let Some(loading) = state.loading() {
                        command = command.loading(loading.message(), loading.progress_percent());
                    }

                    command = match state.open_mode() {
                        CommandOpenMode::Controlled => command.open(GALLERY_SAMPLE_MOUNT_OPEN),

                        CommandOpenMode::Uncontrolled => {
                            command.default_open(GALLERY_SAMPLE_MOUNT_OPEN)
                        }
                    };

                    for item in command_items.iter() {
                        command = command.item(item.clone());
                    }

                    for group in command_groups.iter() {
                        command = command.group(group.clone());
                    }

                    div()
                        .id(format!("component-command-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
                        .w(px(420.0))
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
                                .child(label_pill(if state.dialog().is_some() {
                                    "dialog"
                                } else {
                                    "inline"
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(command)
                        .child(component_command_state_row(&state))
                })),
        )
}

fn resolved_listbox_option(
    option_state: &open_gpui_ui_components::ListboxOptionState,
) -> ListboxOption {
    match option_state.kind() {
        open_gpui_ui_components::ListboxOptionKind::Separator => {
            ListboxOption::separator(option_state.value())
        }

        open_gpui_ui_components::ListboxOptionKind::Option => {
            ListboxOption::new(option_state.value(), option_state.label())
                .disabled(option_state.disabled())
        }
    }
}

fn resolved_listbox_group(
    group_state: &open_gpui_ui_components::ListboxGroupState,

    state: &ListboxState,
) -> ListboxGroup {
    state.group_options(group_state.index()).fold(
        ListboxGroup::new(group_state.value(), group_state.label()),
        |group, option_state| group.option(resolved_listbox_option(option_state)),
    )
}

fn resolved_combobox_option(
    option_state: &open_gpui_ui_components::ListboxOptionState,
) -> ComboboxOption {
    ComboboxOption::new(option_state.value(), option_state.label())
        .disabled(option_state.disabled())
}

fn resolved_combobox_group(
    group_state: &open_gpui_ui_components::ListboxGroupState,

    state: &ListboxState,
) -> ComboboxGroup {
    state.group_options(group_state.index()).fold(
        ComboboxGroup::new(group_state.value(), group_state.label()),
        |group, option_state| group.option(resolved_combobox_option(option_state)),
    )
}

fn resolved_command_item(item_state: &open_gpui_ui_components::CommandItemState) -> CommandItem {
    let mut command_item =
        CommandItem::new(item_state.value(), item_state.label()).disabled(item_state.disabled());

    if let Some(shortcut) = item_state.shortcut() {
        command_item = command_item.shortcut(shortcut);
    }

    command_item
}

fn resolved_command_group(
    group_state: &open_gpui_ui_components::command::CommandGroupState,

    state: &CommandState,
) -> CommandGroup {
    state.group_items(group_state.index()).fold(
        CommandGroup::new(group_state.value(), group_state.label()),
        |group, item_state| group.item(resolved_command_item(item_state)),
    )
}

fn component_listbox_state_row(state: &ListboxState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");

    let disabled_count = state
        .options()
        .iter()
        .filter(|option| option.disabled())
        .count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!("{:?} / {}", state.role(), state.size().as_str()))
        .child(format!("selected {} / active {}", selected, active))
        .child(format!(
            "{} groups / {} options / {} disabled",
            state.groups().len(),
            state.options().len(),
            disabled_count
        ))
}

fn component_select_state_row(state: &SelectState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {:?} / {}",
            state.trigger_role(),
            state.content_role(),
            state.size().as_str()
        ))
        .child(format!(
            "{} / selected {} / active {}",
            if state.open() { "open" } else { "closed" },
            selected,
            active
        ))
        .child(format!(
            "{} options / scroll {} / {:?}",
            state.listbox().options().len(),
            if state.scrollable_content() {
                "enabled"
            } else {
                "not needed"
            },
            state.outside_press_policy()
        ))
}

fn component_combobox_state_row(state: &ComboboxState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {:?} / {}",
            state.input_role(),
            state.content_role(),
            state.size().as_str()
        ))
        .child(format!(
            "query '{}' / selected {} / active {}",
            state.query(),
            selected,
            active
        ))
        .child(format!(
            "{} of {} options / {:?}",
            state.filtered_option_count(),
            state.total_option_count(),
            state.outside_press_policy()
        ))
}

fn component_command_state_row(state: &CommandState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {:?} / {}",
            state.input_role(),
            state.list_role(),
            state.size().as_str()
        ))
        .child(format!(
            "query '{}' / selected {} / active {}",
            state.query(),
            selected,
            active
        ))
        .child(format!(
            "{} groups / {} of {} commands / {}",
            state.groups().len(),
            state.filtered_item_count(),
            state.total_item_count(),
            if state.dialog().is_some() {
                "dialog"
            } else {
                "inline"
            }
        ))
}

pub(crate) fn component_radio_state_row(
    state: &open_gpui_ui_components::RadioGroupState,
) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let focused = state.focused_value().unwrap_or("none");

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
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

pub(crate) fn component_toggle_state_row(state: &ToggleState) -> impl IntoElement {
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
            state.size().as_str()
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().padding_x())
        ))
}

fn overlay_behavior_card(sample: &pages::overlay::OverlayBehaviorSample) -> impl IntoElement {
    let policy = &sample.policy;

    let resolved = OverlayResolvedState::resolve(policy.clone());

    let adapter = gpui_overlay_state(&resolved);

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
        .child(format!("kind: {}", policy.kind().as_str()))
        .child(format!(
            "presence: open {} / present {} / interactive {}",
            bool_label(presence.is_open()),
            bool_label(presence.present()),
            bool_label(presence.interactive())
        ))
        .child(format!(
            "outside: {}",
            policy.outside_press_policy().as_str()
        ))
        .child(format!("escape: {}", policy.escape_key_policy().as_str()))
        .child(format!(
            "focus: open {} / close {}",
            policy.initial_focus_intent().as_str(),
            policy.focus_restore_intent().as_str()
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

fn overlay_catalog_card(entry: &pages::overlay::OverlayCatalogEntry) -> impl IntoElement {
    let (status_bg, status_border, status_text) = entry.status.badge_colors();
    let catalog_selector = entry.catalog_selector();
    let gates = entry.behavior_gates.join(" / ");

    div()
        .id(catalog_selector)
        .debug_selector(move || catalog_selector.into())
        .w(px(260.0))
        .min_h(px(164.0))
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
                        .text_color(rgb(0x24313f))
                        .child(entry.name),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(status_border))
                        .bg(rgb(status_bg))
                        .text_color(rgb(status_text))
                        .text_xs()
                        .child(entry.status.as_str()),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(format!("family: {}", entry.family)),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(format!("state: {}", entry.state)),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(entry.coverage),
        )
        .child(
            div()
                .text_xs()
                .line_height(px(18.0))
                .text_color(rgb(0x5a6472))
                .child(format!("gates: {gates}")),
        )
        .child(
            div()
                .text_xs()
                .line_height(px(18.0))
                .text_color(rgb(0x5a6472))
                .child(format!("selector: {}", entry.sample_selector)),
        )
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
            state.open_intent().as_str(),
            state.content_kind().as_str()
        ))
        .child(format!(
            "placement: {} {} / disabled {} / descriptive {}",
            state.overlay().policy().kind().as_str(),
            state.placement_side().as_str(),
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

fn hover_card_state_row(
    state: &open_gpui_ui_components::HoverCardState,

    effective_open: bool,
) -> impl IntoElement {
    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / intent {}",
            bool_label(effective_open),
            state.open_mode().as_str(),
            state.open_intent().as_str()
        ))
        .child(format!(
            "placement: {} {} / interactive {} / descriptive {}",
            state.placement_side().as_str(),
            state.placement_alignment().as_str(),
            bool_label(state.interactive_content()),
            bool_label(state.descriptive())
        ))
        .child(format!(
            "delay: open {} / close {} / trigger selected {}",
            format_duration_ms(state.delay().open_delay()),
            format_duration_ms(state.delay().close_delay()),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
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
            state.open_mode().as_str(),
            bool_label(state.disabled())
        ))
        .child(format!(
            "placement: {} {} / trigger selected {}",
            state.placement_side().as_str(),
            state.placement_alignment().as_str(),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "focus: open {} / close {} / layer {}",
            state.initial_focus_intent().as_str(),
            state.focus_restore_intent().as_str(),
            state.overlay().policy().kind().as_str()
        ))
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
            state.open_mode().as_str(),
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
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / layer {}",
            state.escape_key_policy().as_str(),
            bool_label(layer_state.blocks_underlay_input()),
            state.overlay().policy().kind().as_str()
        ))
}

fn alert_dialog_state_row(state: &open_gpui_ui_components::AlertDialogState) -> impl IntoElement {
    let layer_state = state.overlay().layer_state();

    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / intent {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            state.intent().as_str()
        ))
        .child(format!(
            "actions: cancel {} / action {} / cancel focus {}",
            state.cancel().label(),
            state.action().label(),
            bool_label(state.cancel().default_focus())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / role alert {}",
            state.escape_key_policy().as_str(),
            bool_label(layer_state.blocks_underlay_input()),
            bool_label(state.content_role() == Role::AlertDialog)
        ))
}

fn sheet_state_row(state: &open_gpui_ui_components::SheetState) -> impl IntoElement {
    let layer_state = state.overlay().layer_state();

    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / side {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            state.side().as_str()
        ))
        .child(format!(
            "surface: {} / close {} / title {}",
            state.modal_mode().as_str(),
            bool_label(state.close_affordance().visible()),
            state.title()
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / layer {}",
            state.escape_key_policy().as_str(),
            bool_label(layer_state.blocks_underlay_input()),
            state.overlay().policy().kind().as_str()
        ))
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
            state.open_mode().as_str(),
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
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event())
        ))
        .child(format!(
            "escape: {} / layer {}",
            state.escape_key_policy().as_str(),
            state.overlay().policy().kind().as_str()
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
            state.open_mode().as_str(),
            focused
        ))
        .child(format!(
            "anchor: {} x {} / snap {}",
            format_ui_px(state.anchor_point().x),
            format_ui_px(state.anchor_point().y),
            format_px(DEFAULT_OVERLAY_SAFE_MARGIN)
        ))
        .child(format!(
            "items: {} / layer {} / outside {}",
            menu.items().len(),
            state.overlay().policy().kind().as_str(),
            menu.outside_press_policy().as_str()
        ))
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
            format_ui_px(rect.origin.x),
            format_ui_px(rect.origin.y),
            format_ui_px(rect.size.width),
            format_ui_px(rect.size.height)
        ))
}

fn ui_px_from_gpui(value: Pixels) -> UiPx {
    UiPx::new(value.as_f32())
}

fn format_ui_px(value: UiPx) -> String {
    format!("{:.0}px", value.as_f32())
}

pub(crate) trait DisplayPx {
    fn display_px(self) -> f32;
}

impl DisplayPx for Pixels {
    fn display_px(self) -> f32 {
        self.as_f32()
    }
}

impl DisplayPx for UiPx {
    fn display_px(self) -> f32 {
        self.as_f32()
    }
}

pub(crate) fn format_px(value: impl DisplayPx) -> String {
    format!("{:.0}px", value.display_px())
}

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn resolved_menu_items(items: &[open_gpui_ui_components::MenuItemState]) -> Vec<MenuItem> {
    items
        .iter()
        .map(|item_state| match item_state.kind() {
            open_gpui_ui_components::MenuItemKind::Separator => {
                MenuItem::separator(item_state.value())
            }

            open_gpui_ui_components::MenuItemKind::Action => {
                MenuItem::action(item_state.value(), item_state.label().to_owned())
                    .disabled(item_state.disabled())
            }
        })
        .collect()
}
