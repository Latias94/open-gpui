use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ElementId, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, DismissReason, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    SemanticDescriptor, Sizable, Size, ThemeTokens, rect, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::choice::{ChoiceSelectionOwnership, SingleChoiceSelectionControl};
use crate::choice_overlay_runtime::request_registered_choice_selection;
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::listbox::{Listbox, ListboxGroup, ListboxOption};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerRegistration,
    OverlayOpenIntent, OverlayOwnership, WindowOverlayRuntime, gpui_overlay_state,
    gpui_relative_overlay_layer, resolve_overlay_open_state,
};
use crate::scroll_area::ScrollArea;
use crate::theme::{ThemeContext, ThemeResolver, gpui_elevation_shadow};

use super::model::{SelectSelection, SelectState, SelectStateRequest};
use super::render_plan::SelectRenderPlan;

type SelectOpenChangeHandler = Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>;
type SelectSelectionHandler = Rc<dyn Fn(SelectSelection, &mut Window, &mut App)>;

#[derive(Clone)]
struct SelectRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
    selection_ownership: ChoiceSelectionOwnership,
    overlay_binding: Option<OverlayLayerBinding>,
}

impl SelectRuntime {
    fn sync_selection(&mut self, selection: &SingleChoiceSelectionControl) {
        if selection.is_controlled() && self.selected_value.as_ref() != selection.value().as_ref() {
            self.selected_value = selection.value().clone();
        }
        self.selection_ownership =
            ChoiceSelectionOwnership::from_controlled(selection.is_controlled());
    }

    fn commit_selection(&mut self, value: String) {
        if !self.selection_ownership.caller_owned() {
            self.selected_value = Some(value.clone());
        }
        self.active_value = Some(value);
    }
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
    selection: SingleChoiceSelectionControl,
    default_active_value: Option<String>,
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
            selection: SingleChoiceSelectionControl::uncontrolled(None),
            default_active_value: None,
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

    /// Applies the caller-owned selected option value.
    pub fn selected(mut self, value: Option<String>) -> Self {
        self.selection = SingleChoiceSelectionControl::controlled(value);
        self
    }

    /// Applies the default selected option value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if !self.selection.is_controlled() {
            self.selection = SingleChoiceSelectionControl::uncontrolled(Some(value));
        }
        self
    }

    /// Applies the initial active option for adapter-owned popup state.
    pub fn default_active(mut self, value: impl Into<String>) -> Self {
        self.default_active_value = Some(value.into());
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

    /// Registers an open-change handler with the runtime-issued intent.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(OverlayOpenIntent, &mut Window, &mut App) + 'static,
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
        SelectState::resolve(SelectStateRequest {
            size: self.size,
            disabled: self.disabled,
            open: self.open,
            default_open: self.default_open,
            label: self.label.to_string(),
            placeholder: self.placeholder.to_string(),
            selected_value: self.selection.value().clone(),
            active_value: self.default_active_value.clone(),
            groups: self.groups.iter().map(ListboxGroup::descriptor).collect(),
            options: self.options.iter().map(ListboxOption::descriptor).collect(),
            placement_side: self.placement_side,
            placement_alignment: self.placement_alignment,
            outside_press_policy: self.outside_press_policy,
            initial_focus_intent: self.initial_focus_intent.clone(),
            focus_restore_intent: self.focus_restore_intent.clone(),
            tokens: self.tokens,
        })
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
        let theme = ThemeResolver::current(window, cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| SelectRuntime {
            open: self.default_open,
            active_value: self.default_active_value.clone(),
            selected_value: self.selection.value().clone(),
            selection_ownership: ChoiceSelectionOwnership::from_controlled(
                self.selection.is_controlled(),
            ),
            overlay_binding: None,
        });
        runtime.update(cx, |runtime, _| {
            runtime.sync_selection(&self.selection);
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let selected_value = runtime_state.selected_value.as_deref();
        let active_value = runtime_state.active_value.as_deref().or(selected_value);
        let state = SelectState::resolve(SelectStateRequest {
            size: self.size,
            disabled: self.disabled,
            open: Some(resolved_open),
            default_open: self.default_open,
            label: self.label.to_string(),
            placeholder: self.placeholder.to_string(),
            selected_value: selected_value.map(str::to_owned),
            active_value: active_value.map(str::to_owned),
            groups: self.groups.iter().map(ListboxGroup::descriptor).collect(),
            options: self.options.iter().map(ListboxOption::descriptor).collect(),
            placement_side: self.placement_side,
            placement_alignment: self.placement_alignment,
            outside_press_policy: self.outside_press_policy,
            initial_focus_intent: self.initial_focus_intent.clone(),
            focus_restore_intent: self.focus_restore_intent.clone(),
            tokens: self.tokens,
        });
        let plan = SelectRenderPlan::from_state(self.id, &state);
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("select:{}", plan.debug_id),
            state.overlay().policy().clone(),
            ownership,
        );
        if let Some(on_open_change) = self.on_open_change.clone() {
            registration = registration.on_open_change(move |intent, window, cx| {
                on_open_change(intent, window, cx);
            });
        }
        if ownership == OverlayOwnership::Uncontrolled {
            let runtime = runtime.downgrade();
            registration = registration.uncontrolled_commit(move |open, _, cx| {
                let _ = runtime.update(cx, |runtime, _| {
                    runtime.open = open;
                });
            });
        }
        let existing_binding = runtime.read(cx).overlay_binding.clone();
        let overlay_binding = window_overlay_runtime
            .bind_component_layer(
                &runtime,
                existing_binding.as_ref(),
                registration,
                window,
                cx,
            )
            .expect("Select overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(overlay_binding.clone());
            });
        }
        let trigger_border = theme.resolve(plan.colors.trigger_border());
        let trigger_background = theme.resolve(plan.colors.trigger_background());
        let trigger_foreground = theme.resolve(if plan.selected {
            plan.colors.trigger_foreground()
        } else {
            plan.colors.trigger_placeholder_foreground()
        });
        let trigger_hover_background = theme.resolve(plan.colors.trigger_hover_background());
        let trigger_focus_shadow = focus_ring_shadow_with_theme(plan.focus_ring, &theme);
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
        let trigger_semantics = SemanticDescriptor::new(plan.trigger_role)
            .with_label(state.label())
            .with_selected(plan.trigger_selected)
            .with_expanded(plan.open)
            .with_disabled(plan.disabled)
            .with_actions(&[AccessibleAction::Focus]);

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
                window_overlay_runtime.inside_region(
                    &overlay_binding,
                    OverlayInsideRegionId::new("trigger"),
                    format!("select:{}:trigger-region", plan.debug_id),
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
                        .track_focus(overlay_binding.trigger_focus())
                        .tab_stop(!plan.disabled)
                        .ui_semantics(&trigger_semantics)
                        .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                        .on_key_down({
                            let window_overlay_runtime = window_overlay_runtime.clone();
                            let overlay_binding = overlay_binding.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                let key = event.keystroke.key.as_str();
                                if !event.keystroke.modifiers.modified()
                                    && !event.prefer_character_input
                                    && matches!(key, "enter" | "space" | "down" | "up")
                                {
                                    cx.stop_propagation();
                                    window.prevent_default();
                                    if !open {
                                        window_overlay_runtime
                                        .request_open_change(
                                            &overlay_binding,
                                            true,
                                            DismissReason::Trigger,
                                            window,
                                            cx,
                                        )
                                        .expect(
                                            "Select keyboard trigger should own its registration",
                                        );
                                    }
                                }
                            }
                        })
                        .when(plan.disabled, |this| {
                            this.opacity(0.56).cursor_not_allowed()
                        })
                        .when(!plan.disabled, |this| {
                            let open = plan.open;
                            let window_overlay_runtime = window_overlay_runtime.clone();
                            let overlay_binding = overlay_binding.clone();
                            this.cursor_pointer()
                                .hover(move |style| style.bg(trigger_hover_background))
                                .capture_any_mouse_up(move |_, window, cx| {
                                    cx.stop_propagation();
                                    window.prevent_default();
                                    window_overlay_runtime
                                        .request_open_change(
                                            &overlay_binding,
                                            !open,
                                            DismissReason::Trigger,
                                            window,
                                            cx,
                                        )
                                        .expect("Select trigger should own its registration");
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
                ),
            )
            .when(plan.open, |this| {
                this.child(gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    &overlay_binding,
                    |opening_theme| {
                        select_content_element(
                            plan.content_id.clone(),
                            plan.listbox_id.clone(),
                            plan.debug_id.clone(),
                            state.clone(),
                            window_overlay_runtime.clone(),
                            overlay_binding.clone(),
                            self.options,
                            self.groups,
                            runtime.clone(),
                            self.on_select.clone(),
                            self.tokens,
                            opening_theme,
                        )
                        .into_any_element()
                    },
                ))
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn select_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: SelectState,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    runtime: open_gpui::Entity<SelectRuntime>,
    on_select: Option<SelectSelectionHandler>,
    tokens: ThemeTokens,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let listbox_runtime = runtime.clone();
    let listbox_window_overlay_runtime = window_overlay_runtime.clone();
    let listbox_overlay_binding = overlay_binding.clone();
    let selected_value = state.selected_value().map(str::to_owned);
    let active_value = state.active_value().map(str::to_owned);
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
        .active_focus_handle(overlay_binding.surface_focus().clone())
        .selection_transaction(move |intent, window, cx| {
            let selected_value = intent.selection().value().to_owned();
            let listbox_runtime = listbox_runtime.clone();
            request_registered_choice_selection(
                &listbox_window_overlay_runtime,
                &listbox_overlay_binding,
                window,
                cx,
                move |window, cx| {
                    listbox_runtime.update(cx, |runtime, _| {
                        runtime.commit_selection(selected_value);
                    });
                    intent.deliver(window, cx);
                },
            );
        });
    let listbox = if let Some(on_select) = on_select {
        listbox.on_select(move |selection, window, cx| {
            on_select(SelectSelection::from(selection), window, cx);
        })
    } else {
        listbox
    };
    let mut listbox = listbox.selected(selected_value);
    if let Some(active_value) = active_value {
        listbox = listbox.default_active(active_value);
    }

    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();

    window_overlay_runtime.surface(
        &overlay_binding,
        OverlayInsideRegionId::new("surface"),
        format!("select:{debug_id}:surface-region"),
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
            .shadow(gpui_elevation_shadow(
                ThemeResolver::overlay_surface_elevation(theme),
            ))
            .occlude()
            .child(
                ScrollArea::new(scroll_viewport_id, listbox)
                    .vertical()
                    .preserve_scroll()
                    .with_size(state.size()),
            ),
    )
}
