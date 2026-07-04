use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayAnchorInput,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, Sizable, Size,
    ThemeTokens, rect, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::listbox::{Listbox, ListboxGroup, ListboxOption};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayCloseRuntimeRequest, OverlayOpenRuntimeRequest,
    apply_overlay_open_change, close_overlay_runtime, close_overlay_runtime_with_after_update,
    consume_overlay_event, gpui_overlay_state, gpui_relative_overlay_layer,
    outside_press_open_change, resolve_overlay_open_state, set_overlay_open,
};
use crate::scroll_area::ScrollArea;
use crate::theme::{ThemeContext, ThemeResolver};

use super::model::{SelectSelection, SelectState};
use super::render_plan::SelectRenderPlan;

type SelectOpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type SelectSelectionHandler = Rc<dyn Fn(SelectSelection, &mut Window, &mut App)>;

#[derive(Debug, Clone)]
struct SelectRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
    trigger_focus: FocusHandle,
}

/// A concrete GPUI select component.
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    size: Size,
    disabled: bool,
    full_width: bool,
    open: Option<bool>,
    default_open: bool,
    selected_value: Option<String>,
    active_value: Option<String>,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<SelectOpenChangeHandler>,
    on_select: Option<SelectSelectionHandler>,
}

impl Select {
    /// Creates an empty select.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Select an option".into(),
            options: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            full_width: false,
            open: None,
            default_open: false,
            selected_value: None,
            active_value: None,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_select: None,
        }
    }

    /// Applies placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Adds one standalone option.
    pub fn option(mut self, option: ListboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many standalone options.
    pub fn options(mut self, options: impl IntoIterator<Item = ListboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Adds one option group.
    pub fn group(mut self, group: ListboxGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many option groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = ListboxGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Marks the select as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes the trigger expand to the full width of its parent.
    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies selected option value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies active option value.
    pub fn active(mut self, value: impl Into<String>) -> Self {
        self.active_value = Some(value.into());
        self
    }

    /// Applies preferred placement.
    pub fn placement(
        mut self,
        side: OverlayPlacementSide,
        alignment: OverlayPlacementAlignment,
    ) -> Self {
        self.placement_side = side;
        self.placement_alignment = alignment;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a select selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(SelectSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved select state.
    pub fn state(&self) -> SelectState {
        SelectState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.label.to_string(),
            self.placeholder.to_string(),
            self.selected_value.as_deref(),
            self.active_value.as_deref(),
            self.groups.iter().map(ListboxGroup::descriptor),
            self.options.iter().map(ListboxOption::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Select {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| SelectRuntime {
            open: self.default_open,
            active_value: self.active_value.clone(),
            selected_value: self.selected_value.clone(),
            trigger_focus: cx.focus_handle(),
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref())
            .or(selected_value);
        let state = SelectState::resolve(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            self.label.to_string(),
            self.placeholder.to_string(),
            selected_value,
            active_value,
            self.groups.iter().map(ListboxGroup::descriptor),
            self.options.iter().map(ListboxOption::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let explicit_active_value = self.active_value.clone();
        let plan = SelectRenderPlan::from_state(self.id, &state);
        let trigger_border = theme.resolve(plan.colors.trigger_border());
        let trigger_background = theme.resolve(plan.colors.trigger_background());
        let trigger_foreground = theme.resolve(if plan.selected {
            plan.colors.trigger_foreground()
        } else {
            plan.colors.trigger_placeholder_foreground()
        });
        let trigger_hover_background = theme.resolve(plan.colors.trigger_hover_background());
        let trigger_focus_shadow = focus_ring_shadow_with_theme(plan.focus_ring, &theme);
        let trigger_focus = runtime_state.trigger_focus.clone();
        let open = plan.open;
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                OverlayAnchorInput::from_layout_bounds(rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(plan.metrics.min_width(), plan.metrics.trigger_height()),
                )),
                ui_size(plan.metrics.min_width(), plan.metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );

        div()
            .id(plan.root_id.clone())
            .debug_selector({
                let debug_id = plan.debug_id.clone();
                move || format!("select:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .when(self.full_width, |this| this.w_full().items_stretch())
            .when(!self.full_width, |this| this.items_start())
            .when(self.full_width, |this| this.occlude())
            .child(
                div()
                    .id(plan.trigger_id)
                    .debug_selector({
                        let debug_id = plan.debug_id.clone();
                        move || format!("select:{debug_id}:trigger")
                    })
                    .when(self.full_width, |this| this.w_full())
                    .when(!self.full_width, |this| {
                        this.min_w(gpui_px_from_ui(plan.metrics.min_width()))
                            .max_w(gpui_px_from_ui(plan.metrics.max_width()))
                    })
                    .min_h(gpui_px_from_ui(plan.metrics.trigger_height()))
                    .px(gpui_px_from_ui(plan.metrics.trigger_padding_x()))
                    .py(gpui_px_from_ui(plan.metrics.trigger_padding_y()))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .rounded(gpui_px_from_ui(plan.metrics.radius()))
                    .border_1()
                    .border_color(trigger_border)
                    .bg(trigger_background)
                    .text_color(trigger_foreground)
                    .text_size(gpui_px_from_ui(plan.metrics.text_size()))
                    .line_height(gpui_px_from_ui(plan.metrics.text_size()))
                    .focusable()
                    .track_focus(&trigger_focus)
                    .tab_stop(!plan.disabled)
                    .ui_role(plan.trigger_role)
                    .aria_label(state.label().to_owned())
                    .aria_selected(plan.trigger_selected)
                    .aria_expanded(plan.open)
                    .aria_disabled(plan.disabled)
                    .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                    .on_key_down({
                        let runtime = runtime.clone();
                        let on_open_change = self.on_open_change.clone();
                        let focus_restore = state.focus_restore_intent().clone();
                        move |event: &KeyDownEvent, window, cx| {
                            let key = event.keystroke.key.as_str();
                            if matches!(key, "enter" | "space" | "down" | "up") {
                                consume_overlay_event(window, cx);
                                if !open {
                                    apply_overlay_open_change(
                                        OverlayOpenRuntimeRequest::new(
                                            runtime.clone(),
                                            true,
                                            on_open_change.as_deref(),
                                        ),
                                        window,
                                        cx,
                                        |runtime| {
                                            set_overlay_open(&mut runtime.open, true);
                                        },
                                    );
                                }
                            } else if key == "escape" {
                                consume_overlay_event(window, cx);
                                close_select(
                                    runtime.clone(),
                                    focus_restore.clone(),
                                    on_open_change.clone(),
                                    window,
                                    cx,
                                );
                            }
                        }
                    })
                    .when(plan.disabled, |this| {
                        this.opacity(0.56).cursor_not_allowed()
                    })
                    .when(!plan.disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = self.on_open_change.clone();
                        let open = plan.open;
                        this.cursor_pointer()
                            .hover(move |style| style.bg(trigger_hover_background))
                            .capture_any_mouse_up(move |_, window, cx| {
                                consume_overlay_event(window, cx);
                                let next_open = !open;
                                apply_overlay_open_change(
                                    OverlayOpenRuntimeRequest::new(
                                        runtime.clone(),
                                        next_open,
                                        on_open_change.as_deref(),
                                    ),
                                    window,
                                    cx,
                                    |runtime| {
                                        set_overlay_open(&mut runtime.open, next_open);
                                    },
                                );
                            })
                    })
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .truncate()
                            .child(plan.trigger_label),
                    )
                    .child(div().child(if plan.open { "^" } else { "v" })),
            )
            .when(plan.open, |this| {
                this.child(gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    select_content_element(
                        plan.content_id.clone(),
                        plan.listbox_id.clone(),
                        state.clone(),
                        explicit_active_value.clone(),
                        self.options,
                        self.groups,
                        runtime.clone(),
                        self.on_open_change.clone(),
                        self.on_select.clone(),
                        self.tokens,
                        &theme,
                    ),
                ))
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn select_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    state: SelectState,
    explicit_active_value: Option<String>,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    runtime: open_gpui::Entity<SelectRuntime>,
    on_open_change: Option<SelectOpenChangeHandler>,
    on_select: Option<SelectSelectionHandler>,
    tokens: ThemeTokens,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let focus_restore = state.focus_restore_intent().clone();
    let escape_runtime = runtime.clone();
    let escape_open_change = on_open_change.clone();
    let escape_focus_restore = focus_restore.clone();
    let listbox_runtime = runtime.clone();
    let listbox_open_change = on_open_change.clone();
    let listbox_select = on_select.clone();
    let listbox_focus_restore = focus_restore.clone();
    let selected_value = state.selected_value().map(str::to_owned);
    let label = state.label().to_owned();
    let listbox = options
        .into_iter()
        .fold(
            Listbox::new(listbox_id, label.clone()),
            |listbox, option| listbox.option(option),
        )
        .groups(groups)
        .tokens(tokens)
        .with_size(state.size())
        .embedded(true)
        .on_select(move |selection, window, cx| {
            let selection = SelectSelection::from(selection);
            let selected_value = selection.value().to_owned();
            let on_select = listbox_select.clone();
            let trigger_focus = listbox_runtime.read(cx).trigger_focus.clone();
            close_overlay_runtime_with_after_update(
                OverlayCloseRuntimeRequest::new(
                    listbox_runtime.clone(),
                    &listbox_focus_restore,
                    trigger_focus,
                    listbox_open_change.as_deref(),
                ),
                window,
                cx,
                {
                    let selected_value = selected_value.clone();
                    move |runtime| {
                        runtime.selected_value = Some(selected_value.clone());
                        runtime.active_value = Some(selected_value);
                        set_overlay_open(&mut runtime.open, false);
                    }
                },
                move |window, cx| {
                    if let Some(on_select) = on_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                },
            );
        });
    let mut listbox = listbox;
    if let Some(selected_value) = selected_value {
        listbox = listbox.selected(selected_value);
    }
    if let Some(active_value) = explicit_active_value {
        listbox = listbox.active(active_value);
    }

    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();

    div()
        .id(content_id)
        .debug_selector({
            let viewport_id = scroll_viewport_id.clone();
            move || format!("select:{viewport_id}:content")
        })
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .p(gpui_px_from_ui(metrics.content_padding()))
        .h(gpui_px_from_ui(metrics.max_height()))
        .flex()
        .flex_col()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(theme.resolve(colors.content_border()))
        .bg(theme.resolve(colors.content_background()))
        .text_color(theme.resolve(colors.content_foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow_lg()
        .occlude()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                consume_overlay_event(window, cx);
                close_select(
                    escape_runtime.clone(),
                    escape_focus_restore.clone(),
                    escape_open_change.clone(),
                    window,
                    cx,
                );
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            let focus_restore = focus_restore.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_select(
                    runtime.clone(),
                    focus_restore.clone(),
                    on_open_change.clone(),
                    window,
                    cx,
                );
            })
        })
        .child(
            ScrollArea::new(scroll_viewport_id, listbox)
                .vertical()
                .preserve_scroll()
                .with_size(state.size()),
        )
}

fn close_select(
    runtime: open_gpui::Entity<SelectRuntime>,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<SelectOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let trigger_focus = runtime.read(cx).trigger_focus.clone();
    close_overlay_runtime(
        OverlayCloseRuntimeRequest::new(
            runtime,
            &focus_restore,
            trigger_focus,
            on_open_change.as_deref(),
        ),
        window,
        cx,
        |runtime| {
            set_overlay_open(&mut runtime.open, false);
        },
    );
}
