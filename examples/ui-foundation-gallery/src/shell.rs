//! Gallery shell that consumes the UI foundation directly.

use open_gpui::prelude::*;

use open_gpui::{
    Anchor, App, AppContext, BorrowAppContext, Bounds, Context, FocusHandle, InteractiveElement,
    IntoElement, ListAlignment, ListState, ParentElement, Pixels, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, anchored, deferred,
    div, px, rgb, size,
};
use open_gpui_devtools::DevtoolsInspectorController;

use open_gpui_ui_components::{
    AlertDialog, Avatar, AvatarGroup, AvatarState, BadgeState, Button, ButtonState, ButtonVariant,
    Checkbox, CheckboxState, ColorIntent, Combobox, ComboboxGroup, ComboboxOpenMode,
    ComboboxOption, ComboboxState, Command, CommandGroup, CommandItem, CommandOpenMode,
    CommandSelectionMode, CommandState, ContextMenu, Dialog, FieldState, FocusRing, HoverCard,
    IconButtonState, Kbd, KbdState, Label, LabelState, Listbox, ListboxGroup, ListboxState, Menu,
    MenuItem, OverlayResolvedState, Popover, Progress, ProgressState, ScrollArea, Select,
    SelectOpenMode, SelectState, Separator, SeparatorState, Sheet, Skeleton, SkeletonState,
    SwitchState, TextInputState, TextareaState, ThemeResolver, ToggleState, Tooltip,
    gpui_adapter::{
        DEFAULT_OVERLAY_SAFE_MARGIN, TextInputController, UiA11yElementExt,
        focus_ring_shadow_with_theme, gpui_overlay_state, gpui_point_from_ui, gpui_px_from_ui,
        init_text_input,
    },
    listbox::ListboxOption,
};

use open_gpui_ui_core::{
    AccessibleAction, Density, DeviceAdaptivePolicy, DeviceShellMode, DeviceShellSwitchPolicy,
    Orientation, OverlayPlacementAlignment, OverlayPlacementSide, Rect, Role, Sizable, Size,
    ThemeTokens, Toggled, UiPx,
};

use crate::pages::{
    self, GALLERY_SECTIONS, GalleryPage, focus_a11y::FocusA11yPageState, overlay::OverlayPageState,
};

mod components;
mod focus_a11y;
mod overlay;
mod support;

pub(crate) use components::*;
use overlay::*;
pub(crate) use support::{DisplayPx, component_catalog_status_pill, format_px, label_pill};
use support::{format_ui_px, geometry_row, toggled_label, ui_px_from_gpui};

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

    /// Focused Components page mode.
    pub components_focus: pages::components::ComponentFocusMode,
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

        components_focus: pages::components::ComponentFocusMode::All,
    }
}

/// Top-level gallery view.

#[derive(Debug)]

pub struct GalleryShell {
    selected_page: GalleryPage,
    width: Pixels,
    root_focus: FocusHandle,
    page_scroll_handle: ScrollHandle,
    components_list_state: ListState,
    editable_text_input: open_gpui::Entity<TextInputController>,
    focus_controls: [FocusHandle; pages::focus_a11y::FOCUS_CONTROLS.len()],
    tooltip_focus_controls: [FocusHandle; 4],
    focus_a11y: FocusA11yPageState,
    overlay: OverlayPageState,
    components_focus: pages::components::ComponentFocusMode,
    devtools_workbench: pages::devtools::GalleryDevtoolsWorkbench,
    devtools_inspector: open_gpui::Entity<DevtoolsInspectorController>,
}

impl GalleryShell {
    fn build(selected_page: GalleryPage, cx: &mut Context<Self>) -> Self {
        cx.set_global(pages::components::TableSampleRuntimeLog::default());
        cx.set_global(pages::components::TreeSampleRuntimeLog::default());
        let initial_snapshot = foundation_snapshot(DEFAULT_GALLERY_WIDTH, selected_page);
        let devtools_facts = Self::devtools_live_facts(initial_snapshot);
        let devtools_workbench = pages::devtools::GalleryDevtoolsWorkbench::new(devtools_facts);
        let devtools_state = devtools_workbench.inspector_state();

        Self {
            selected_page,

            width: DEFAULT_GALLERY_WIDTH,

            root_focus: cx.focus_handle(),
            page_scroll_handle: ScrollHandle::new(),
            components_list_state: ListState::new(
                pages::components::component_page_section_count(
                    pages::components::ComponentFocusMode::All,
                ),
                ListAlignment::Top,
                px(900.0),
            ),

            editable_text_input: cx.new(|cx| {
                let mut controller = TextInputController::with_value("", cx);

                controller.set_placeholder("Type in the gallery", cx);

                controller
            }),

            focus_controls: pages::focus_a11y::FOCUS_CONTROLS
                .map(|spec| cx.focus_handle().tab_index(spec.tab_index).tab_stop(true)),

            tooltip_focus_controls: [
                cx.focus_handle().tab_index(10).tab_stop(true),
                cx.focus_handle().tab_index(11).tab_stop(true),
                cx.focus_handle().tab_index(12).tab_stop(true),
                cx.focus_handle().tab_index(13).tab_stop(true),
            ],
            focus_a11y: FocusA11yPageState::default(),
            overlay: OverlayPageState::default(),
            components_focus: pages::components::ComponentFocusMode::All,
            devtools_workbench,
            devtools_inspector: cx.new(|cx| {
                DevtoolsInspectorController::new("gallery-devtools-inspector", devtools_state, cx)
                    .title("Gallery DevTools Inspector")
            }),
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

    pub fn devtools_inspector(&self) -> &open_gpui::Entity<DevtoolsInspectorController> {
        &self.devtools_inspector
    }

    pub fn devtools_workbench(&self) -> &pages::devtools::GalleryDevtoolsWorkbench {
        &self.devtools_workbench
    }

    pub fn refresh_devtools(&mut self, cx: &mut Context<Self>) {
        let facts = Self::devtools_live_facts(self.snapshot());
        let selected_before = self
            .devtools_inspector
            .read(cx)
            .state()
            .selected_event_identity()
            .cloned();

        if let Ok(frame) = self.devtools_workbench.refresh_with_facts(facts) {
            let selection_status = self.devtools_inspector.update(cx, |inspector, cx| {
                inspector.update_session_frame(frame, cx);
                let selected_after = inspector.state().selected_event_identity().cloned();
                match selected_before {
                    None => pages::devtools::GalleryDevtoolsSelectionStatus::None,
                    Some(before) if selected_after.as_ref() == Some(&before) => {
                        pages::devtools::GalleryDevtoolsSelectionStatus::Preserved
                    }
                    Some(_) => pages::devtools::GalleryDevtoolsSelectionStatus::Remapped,
                }
            });
            self.devtools_workbench
                .set_selection_status(selection_status);
        }
        cx.notify();
    }

    /// Returns the page scroll handle used by gallery smoke tests and anchored jumps.
    pub fn page_scroll_handle(&self) -> &ScrollHandle {
        &self.page_scroll_handle
    }

    /// Returns the Components page lazy section list state used by tests.
    pub fn components_list_state(&self) -> &ListState {
        &self.components_list_state
    }

    /// Returns the current foundation snapshot.

    pub fn snapshot(&self) -> GalleryShellSnapshot {
        let mut snapshot = foundation_snapshot(self.width, self.selected_page);
        snapshot.components_focus = self.components_focus;
        snapshot
    }

    fn devtools_live_facts(
        snapshot: GalleryShellSnapshot,
    ) -> pages::devtools::GalleryDevtoolsLiveFacts {
        pages::devtools::GalleryDevtoolsLiveFacts::new(
            snapshot.selected_page.id(),
            snapshot.viewport_width.as_f32(),
            snapshot.shell_mode.as_str(),
            snapshot.density.as_str(),
            snapshot.control_size.as_str(),
        )
    }

    fn select_page(&mut self, page: GalleryPage, cx: &mut Context<Self>) {
        if self.selected_page != page {
            if self.selected_page == GalleryPage::Components && page != GalleryPage::Components {
                self.components_focus = pages::components::ComponentFocusMode::All;
                self.components_list_state
                    .reset(pages::components::component_page_section_count(
                        pages::components::ComponentFocusMode::All,
                    ));
            }
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

    /// Updates the Components page focus mode used by catalog cards and focus controls.
    pub fn set_components_focus(
        &mut self,
        focus: pages::components::ComponentFocusMode,
        cx: &mut Context<Self>,
    ) {
        if self.components_focus != focus {
            self.components_focus = focus;
            self.components_list_state
                .reset(pages::components::component_page_section_count(focus));
            cx.notify();
        }
    }

    pub fn jump_to_components_section(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let focus = pages::components::ComponentFocusMode::section(id)
            .unwrap_or(pages::components::ComponentFocusMode::All);
        if self.components_focus != focus {
            self.components_focus = focus;
            self.components_list_state
                .reset(pages::components::component_page_section_count(focus));
        }
        if let Some(index) =
            pages::components::component_page_section_index(self.components_focus, id)
        {
            self.components_list_state.scroll_to_reveal_item(index);
        }
        cx.notify();
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
                this.child(pages::components::render_components_directory(snapshot, cx))
            })
            .child(
                div()
                    .id("gallery-page-scroll")
                    .debug_selector(|| "gallery:page-scroll".into())
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .when(page == GalleryPage::Components, |this| {
                        this.child(self.render_page_body(snapshot, window, cx))
                    })
                    .when(page != GalleryPage::Components, |this| {
                        this.child(
                            ScrollArea::new(
                                "gallery-page-scroll-viewport",
                                self.render_page_body(snapshot, window, cx),
                            )
                            .scroll_handle(&self.page_scroll_handle)
                            .with_size(snapshot.control_size)
                            .reset_on_key(self.page_scroll_reset_key(snapshot)),
                        )
                    }),
            )
    }

    fn page_scroll_reset_key(&self, snapshot: GalleryShellSnapshot) -> String {
        if snapshot.selected_page == GalleryPage::Components {
            format!(
                "{}:{}",
                snapshot.selected_page.id(),
                snapshot.components_focus.reset_key()
            )
        } else {
            snapshot.selected_page.id().to_owned()
        }
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

            GalleryPage::Components => {
                pages::components::render_components_page(self, snapshot, cx).into_any_element()
            }

            GalleryPage::Devtools => self.render_devtools_page(snapshot, cx).into_any_element(),
        }
    }

    fn render_devtools_page(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("gallery-devtools-page")
            .debug_selector(|| "gallery:devtools-page".into())
            .flex()
            .flex_col()
            .gap_4()
            .child(self.render_devtools_toolbar(snapshot, cx))
            .child(self.devtools_inspector.clone())
            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_devtools_toolbar(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let generation = self
            .devtools_workbench
            .current_generation()
            .map_or_else(|| "none".to_owned(), |generation| generation.to_string());
        let previous_generation = self
            .devtools_workbench
            .previous_generation()
            .map_or_else(|| "none".to_owned(), |generation| generation.to_string());
        let frame_count = self.devtools_workbench.retained_frames();
        let history_limit = self.devtools_workbench.history_limit();
        let diff_rows = self.devtools_workbench.diff_row_count();
        let refresh_state = self.devtools_workbench.refresh_status().as_label();
        let selection_state = self.devtools_workbench.selection_status().as_label();
        let diff_state = self.devtools_workbench.diff_state_label();
        let active_page = snapshot.selected_page.id();

        div()
            .id("gallery-devtools-toolbar")
            .debug_selector(|| "gallery-devtools:toolbar".into())
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .debug_selector(|| "gallery-devtools:refresh".into())
                    .child(
                        Button::new("gallery-devtools-refresh-button", "Refresh")
                            .variant(ButtonVariant::Secondary)
                            .with_size(Size::Small)
                            .accessibility_description("Refresh Gallery DevTools session")
                            .on_activate(cx.processor(|this, _, _, cx| {
                                this.refresh_devtools(cx);
                            })),
                    ),
            )
            .child(devtools_status_pill(
                "refresh-state",
                "refresh",
                refresh_state,
            ))
            .child(devtools_status_pill(
                "frame-history",
                "history",
                format!("{frame_count}/{history_limit} frames"),
            ))
            .child(devtools_status_pill(
                "generation",
                "generation",
                format!("{generation} prev {previous_generation}"),
            ))
            .child(devtools_status_pill(
                "diff-state",
                "diff",
                format!("{diff_state} / {diff_rows} rows"),
            ))
            .child(devtools_status_pill(
                "selection-state",
                "selection",
                selection_state,
            ))
            .child(devtools_status_pill("active-page", "page", active_page))
            .when_some(self.devtools_workbench.last_error(), |this, error| {
                this.child(devtools_status_pill("capture-error", "error", error))
            })
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
                div().flex().gap_3().flex_wrap().children(
                    pages::focus_a11y::FOCUS_CONTROLS
                        .into_iter()
                        .zip(self.focus_controls.iter())
                        .map(|(spec, handle)| {
                            self.render_focus_control(handle, spec, cx)
                                .into_any_element()
                        }),
                ),
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
            .child(focus_a11y::render_focus_a11y_text_form_scenarios(
                self,
                snapshot.tokens,
                cx,
            ))
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

        handle: &FocusHandle,

        spec: pages::focus_a11y::FocusControlSpec,

        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let focus_ring = FocusRing::from_color(ColorIntent::new(
            ThemeTokens::default().focus_ring,
            0x2f80ed,
        ));
        let theme = ThemeResolver::current(cx);
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

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
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
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
            .child(self.render_nested_overlay_runtime_card(
                self.overlay
                    .is_controlled_open(pages::overlay::OverlayControlledSample::NestedDialog),
                cx,
            ))
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
                            .child(self.render_menu_sample_card(&menu_samples[3], false, cx))
                            .child(self.render_menu_sample_card(&menu_samples[4], false, cx))
                            .child(self.render_menu_sample_card(&menu_samples[5], false, cx))
                            .child(self.render_menu_sample_card(&menu_samples[6], false, cx)),

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
                            ))
                            .child(self.render_context_menu_sample_card(
                                &context_menu_samples[3],
                                false,
                                cx,
                            ))
                            .child(self.render_context_menu_sample_card(
                                &context_menu_samples[4],
                                false,
                                cx,
                            )),

                    ),

            )

            .child(self.render_signal_list(snapshot.selected_page))
    }

    fn render_nested_overlay_runtime_card(
        &self,
        dialog_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let update_dialog = cx.entity().downgrade();

        let nested_dialog = Dialog::new(
            "overlay-runtime-nested-dialog",
            "Review dialog",
            "Review changes",
            "Review the pending workspace changes before continuing.",
        )
        .open(dialog_open)
        .on_open_change(move |intent, _, cx| {
            let open = intent.desired_open();
            update_dialog
                .update(cx, |this, cx| {
                    this.mutate_overlay(
                        |state| {
                            state.set_controlled_open(
                                pages::overlay::OverlayControlledSample::NestedDialog,
                                open,
                            )
                        },
                        cx,
                    )
                })
                .ok();
        });

        let nested_content = div()
            .w(px(272.0))
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .child(
                Menu::new("overlay-runtime-nested-menu", "Review options")
                    .item(MenuItem::action("inspect", "Inspect changes"))
                    .placement(
                        OverlayPlacementSide::Right,
                        OverlayPlacementAlignment::Start,
                    )
                    .overlay_child(nested_dialog),
            );

        div()
            .id("overlay-runtime-nested-card")
            .debug_selector(|| "gallery:overlay-runtime-nested-card".to_owned())
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child("Workspace review"),
            )
            .child(
                div()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xd6d8ce))
                    .bg(rgb(0xffffff))
                    .p_3()
                    .text_xs()
                    .text_color(rgb(0x3f4a57))
                    .child(
                        Popover::element(
                            "overlay-runtime-nested-popover",
                            "Workspace actions",
                            nested_content,
                        )
                        .placement_side(OverlayPlacementSide::Bottom)
                        .placement_alignment(OverlayPlacementAlignment::Start),
                    ),
            )
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

        let theme = ThemeResolver::current(cx);
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

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
                .focus_visible(move |style| style.shadow(focus_shadow.clone()))
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
        .placement_side(state.placement_side())
        .placement_alignment(state.placement_alignment())
        .with_size(state.size());
        let hover_card = match state.open_mode() {
            open_gpui_ui_components::HoverCardOpenMode::Controlled => hover_card
                .open(state.open())
                .on_open_change(move |intent, _, cx| {
                    let open = intent.desired_open();
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
                .on_open_change(move |intent, _, cx| {
                    let open = intent.desired_open();
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

        let refuse_dialog_close = self.overlay.refuses_dialog_close();

        let shell = cx.entity().downgrade();
        let toggle_dialog_refusal = cx.entity().downgrade();

        let dialog = if sample_id == "controlled-modal" && refuse_dialog_close {
            let commit_dialog = cx.entity().downgrade();
            Dialog::element(
                format!("overlay-dialog-demo:{}", sample_id),
                label,
                sample.state.title(),
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_3()
                    .child(content_text)
                    .child(
                        div()
                            .id("overlay-dialog-owner-commit-close-probe")
                            .debug_selector(|| {
                                "gallery:overlay-dialog-owner-commit-close:controlled-modal"
                                    .to_owned()
                            })
                            .flex()
                            .child(
                                Button::new(
                                    "overlay-dialog-owner-commit-close",
                                    "Confirm close",
                                )
                                .on_activate(move |_, _, cx| {
                                    commit_dialog
                                        .update(cx, |this, cx| {
                                            this.mutate_overlay(
                                                |state| {
                                                    let mut changed = false;
                                                    changed |= state.set_refuse_dialog_close(false);
                                                    changed |= state.set_controlled_open(
                                                        pages::overlay::OverlayControlledSample::Dialog,
                                                        false,
                                                    );
                                                    changed
                                                },
                                                cx,
                                            )
                                        })
                                        .ok();
                                }),
                            ),
                    ),
            )
        } else {
            Dialog::new(
                format!("overlay-dialog-demo:{}", sample_id),
                label,
                sample.state.title(),
                content_text,
            )
        }
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
                .on_open_change(move |intent, window, cx| {
                    let open = intent.desired_open();
                    let should_reject = shell
                        .update(cx, |this, cx| {
                            if !open && this.overlay.refuses_dialog_close() {
                                true
                            } else {
                                this.mutate_overlay(
                                    |state| {
                                        state.set_controlled_open(
                                            pages::overlay::OverlayControlledSample::Dialog,
                                            open,
                                        )
                                    },
                                    cx,
                                );
                                false
                            }
                        })
                        .unwrap_or(false);
                    if should_reject {
                        intent.reject(window, cx).expect(
                            "controlled Gallery dialog must reject its current close intent",
                        );
                    }
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
        .when(sample_id == "controlled-modal", |card| {
            card.child(
                div()
                    .id("overlay-dialog-refuse-close-toggle")
                    .debug_selector(|| {
                        "gallery:overlay-dialog-refuse-close:controlled-modal".to_owned()
                    })
                    .flex()
                    .child(
                        Checkbox::new("overlay-dialog-refuse-close:controlled-modal")
                            .label("Require close confirmation")
                            .checked(refuse_dialog_close)
                            .on_toggle(move |toggled, _, cx| {
                                toggle_dialog_refusal
                                    .update(cx, |this, cx| {
                                        this.mutate_overlay(
                                            |state| {
                                                state.set_refuse_dialog_close(
                                                    toggled == Toggled::True,
                                                )
                                            },
                                            cx,
                                        )
                                    })
                                    .ok();
                            }),
                    ),
            )
        })
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
                .on_open_change(move |intent, _, cx| {
                    let open = intent.desired_open();
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
            open_gpui_ui_components::SheetOpenMode::Controlled => sheet
                .open(state.open())
                .on_open_change(move |intent, _, cx| {
                    let open = intent.desired_open();
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
                }),

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
            open_gpui_ui_components::MenuOpenMode::Controlled => menu
                .open(state.open())
                .on_open_change(move |intent, _, cx| {
                    let open = intent.desired_open();
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
                }),

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
                .on_open_change(move |intent, _, cx| {
                    let open = intent.desired_open();
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

fn devtools_status_pill(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
) -> impl IntoElement {
    let value = value.into();
    div()
        .id(format!("gallery-devtools-{id}"))
        .debug_selector(move || format!("gallery-devtools:{id}"))
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe1e4da))
        .bg(rgb(0xfcfcf8))
        .px_2()
        .py_1()
        .child(div().text_xs().text_color(rgb(0x5a6472)).child(label))
        .child(
            div()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child(value),
        )
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
