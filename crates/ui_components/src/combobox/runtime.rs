use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, DismissReason, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    Role, SemanticDescriptor, Sizable, Size, ThemeTokens, rect, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::choice;
use crate::choice_overlay_runtime::{
    ChoiceOverlayRuntimeState, commit_registered_choice_overlay_single_value,
};
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::listbox::Listbox;
use crate::overlay::{
    GpuiOverlayPlacement, OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerRegistration,
    OverlayOpenIntent, OverlayOwnership, WindowOverlayRuntime, gpui_overlay_state,
    gpui_relative_overlay_layer, resolve_overlay_open_state,
};
use crate::scroll_area::ScrollArea;
use crate::text_editing::TextEditingPolicy;
use crate::text_input::TextInput;
use crate::text_input::adapter::TextInputController;
use crate::theme::{ThemeContext, ThemeResolver};

use super::descriptor::{ComboboxGroup, ComboboxOption};
use super::model::{
    ComboboxKeyboardAction, ComboboxSelection, ComboboxState, ComboboxStateRequest,
    combobox_keyboard_action,
};
use super::render_plan::ComboboxRenderPlan;

type ComboboxOpenChangeHandler = Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>;
type ComboboxSelectionHandler = Rc<dyn Fn(ComboboxSelection, &mut Window, &mut App)>;

#[derive(Clone)]
struct ComboboxRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
    overlay_binding: Option<OverlayLayerBinding>,
}

impl ChoiceOverlayRuntimeState for ComboboxRuntime {
    fn commit_single_value(&mut self, value: String) {
        self.selected_value = Some(value.clone());
        self.active_value = Some(value);
    }
}

/// A concrete GPUI combobox component.
#[derive(IntoElement)]
pub struct Combobox {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    options: Vec<ComboboxOption>,
    groups: Vec<ComboboxGroup>,
    size: Size,
    disabled: bool,
    required: bool,
    open: Option<bool>,
    default_open: bool,
    default_query: String,
    selected_value: Option<String>,
    active_value: Option<String>,
    empty_label: SharedString,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<ComboboxOpenChangeHandler>,
    on_select: Option<ComboboxSelectionHandler>,
}

impl Combobox {
    /// Creates an empty combobox.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Search".into(),
            options: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            required: false,
            open: None,
            default_open: false,
            default_query: String::new(),
            selected_value: None,
            active_value: None,
            empty_label: "No results".into(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            initial_focus_intent: InitialFocusIntent::None,
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
    pub fn option(mut self, option: ComboboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many standalone options.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Adds one option group.
    pub fn group(mut self, group: ComboboxGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many option groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = ComboboxGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Marks the combobox as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the combobox as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
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

    /// Applies the default query text for adapter-owned input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        let query = query.into();
        self.default_query = TextEditingPolicy::single_line().normalize_text(query.as_str());
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

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
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

    /// Registers a combobox selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ComboboxSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved combobox state.
    pub fn state(&self) -> ComboboxState {
        ComboboxState::resolve(ComboboxStateRequest {
            size: self.size,
            disabled: self.disabled,
            required: self.required,
            open: self.open,
            default_open: self.default_open,
            label: self.label.to_string(),
            placeholder: self.placeholder.to_string(),
            query: self.default_query.to_string(),
            selected_value: self.selected_value.clone(),
            active_value: self.active_value.clone(),
            empty_label: self.empty_label.to_string(),
            groups: self.groups.iter().map(ComboboxGroup::descriptor).collect(),
            options: self
                .options
                .iter()
                .map(ComboboxOption::descriptor)
                .collect(),
            placement_side: self.placement_side,
            placement_alignment: self.placement_alignment,
            outside_press_policy: self.outside_press_policy,
            initial_focus_intent: self.initial_focus_intent.clone(),
            focus_restore_intent: self.focus_restore_intent.clone(),
            tokens: self.tokens,
        })
    }
}

impl Sizable for Combobox {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Combobox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| ComboboxRuntime {
            open: self.default_open,
            active_value: self.active_value.clone(),
            selected_value: self.selected_value.clone(),
            overlay_binding: None,
        });
        let input_state_key: ElementId = (self.id.clone(), "input-state").into();
        let input_controller = window.use_keyed_state(input_state_key, cx, |_, cx| {
            let mut input = TextInputController::with_value(self.default_query.clone(), cx);
            input.set_placeholder(self.placeholder.clone(), cx);
            input
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let query = input_controller.read(cx).value().to_owned();
        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref())
            .or(selected_value);
        let state = ComboboxState::resolve(ComboboxStateRequest {
            size: self.size,
            disabled: self.disabled,
            required: self.required,
            open: Some(resolved_open),
            default_open: self.default_open,
            label: self.label.to_string(),
            placeholder: self.placeholder.to_string(),
            query: query.clone(),
            selected_value: selected_value.map(str::to_owned),
            active_value: active_value.map(str::to_owned),
            empty_label: self.empty_label.to_string(),
            groups: self.groups.iter().map(ComboboxGroup::descriptor).collect(),
            options: self
                .options
                .iter()
                .map(ComboboxOption::descriptor)
                .collect(),
            placement_side: self.placement_side,
            placement_alignment: self.placement_alignment,
            outside_press_policy: self.outside_press_policy,
            initial_focus_intent: self.initial_focus_intent.clone(),
            focus_restore_intent: self.focus_restore_intent.clone(),
            tokens: self.tokens,
        });
        input_controller.update(cx, |controller, cx| {
            if controller.placeholder() != self.placeholder.as_ref() {
                controller.set_placeholder(self.placeholder.clone(), cx);
            }
        });

        let plan = ComboboxRenderPlan::from_state(self.id, &state);
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("combobox:{}", plan.debug_id),
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
        let editor_focus = input_controller.focus_handle(cx);
        let overlay_binding = window_overlay_runtime
            .bind_component_layer_with_trigger_focus(
                &runtime,
                existing_binding.as_ref(),
                registration,
                editor_focus,
                window,
                cx,
            )
            .expect("Combobox overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(overlay_binding.clone());
            });
        }
        let open = plan.open;
        let disabled = plan.disabled;
        let toggle_focus_shadow = focus_ring_shadow_with_theme(plan.focus_ring, &theme);
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                OverlayAnchorInput::from_layout_bounds(rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(plan.metrics.popup_min_width(), plan.input_height),
                )),
                ui_size(plan.metrics.popup_min_width(), plan.input_height),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );
        let input_semantics = SemanticDescriptor::new(plan.input_role)
            .with_label(plan.label.as_ref())
            .with_expanded(open)
            .with_disabled(disabled);
        let toggle_semantics = SemanticDescriptor::new(Role::Button)
            .with_label("Toggle combobox popup")
            .with_expanded(open)
            .with_disabled(disabled)
            .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);

        div()
            .id(plan.root_id.clone())
            .debug_selector({
                let debug_id = plan.debug_id.clone();
                move || format!("combobox:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                window_overlay_runtime.inside_region(
                    &overlay_binding,
                    OverlayInsideRegionId::new("trigger"),
                    format!("combobox:{}:trigger-region", plan.debug_id),
                    div()
                        .id(plan.input_row_id.clone())
                        .debug_selector({
                            let debug_id = plan.debug_id.clone();
                            move || format!("combobox:{debug_id}:input-row")
                        })
                        .min_w(gpui_px_from_ui(plan.metrics.popup_min_width()))
                        .max_w(gpui_px_from_ui(plan.metrics.popup_max_width()))
                        .flex()
                        .items_center()
                        .gap_1()
                        .ui_semantics(&input_semantics)
                        .on_key_down({
                            let runtime = runtime.clone();
                            let input_controller = input_controller.clone();
                            let on_select = self.on_select.clone();
                            let key_state = state.clone();
                            let window_overlay_runtime = window_overlay_runtime.clone();
                            let overlay_binding = overlay_binding.clone();
                            move |event: &KeyDownEvent, window, cx| match combobox_keyboard_action(
                                &key_state,
                                event.keystroke.key.as_str(),
                            ) {
                                ComboboxKeyboardAction::Navigate(value) => {
                                    cx.stop_propagation();
                                    window.prevent_default();
                                    if !key_state.open() {
                                        let effect_runtime = runtime.clone();
                                        window_overlay_runtime
                                            .request_open_change_with_effect(
                                                &overlay_binding,
                                                true,
                                                DismissReason::Trigger,
                                                window,
                                                cx,
                                                move |_, cx| {
                                                    effect_runtime.update(cx, |runtime, _| {
                                                        runtime.active_value = Some(value);
                                                    });
                                                },
                                            )
                                            .expect(
                                                "Combobox navigation should own its registration",
                                            );
                                    } else {
                                        runtime.update(cx, |runtime, _| {
                                            runtime.active_value = Some(value);
                                        });
                                    }
                                }
                                ComboboxKeyboardAction::Select(selection) => {
                                    cx.stop_propagation();
                                    window.prevent_default();
                                    let selected_value = selection.value().to_owned();
                                    let selected_label = selection.label().to_owned();
                                    let input_controller = input_controller.clone();
                                    let on_select = on_select.clone();
                                    commit_registered_choice_overlay_single_value(
                                        &window_overlay_runtime,
                                        &overlay_binding,
                                        runtime.clone(),
                                        selected_value,
                                        window,
                                        cx,
                                        move |window, cx| {
                                            input_controller.update(cx, |controller, cx| {
                                                controller.set_value(selected_label, cx);
                                            });
                                            if let Some(on_select) = on_select.as_ref() {
                                                on_select(selection, window, cx);
                                            }
                                        },
                                    );
                                }
                                ComboboxKeyboardAction::Open => {
                                    cx.stop_propagation();
                                    window.prevent_default();
                                    if !key_state.open() {
                                        window_overlay_runtime
                                            .request_open_change(
                                                &overlay_binding,
                                                true,
                                                DismissReason::Trigger,
                                                window,
                                                cx,
                                            )
                                            .expect("Combobox open should own its registration");
                                    }
                                }
                                ComboboxKeyboardAction::Ignore => {}
                            }
                        })
                        .child(
                            TextInput::new(plan.input_id.clone(), plan.label.clone())
                                .controller(input_controller.clone())
                                .placeholder(plan.placeholder.clone())
                                .value(query)
                                .disabled(state.disabled())
                                .required(state.required())
                                .tokens(self.tokens)
                                .with_size(state.size()),
                        )
                        .child(
                            div()
                                .id(plan.toggle_id.clone())
                                .debug_selector({
                                    let debug_id = plan.debug_id.clone();
                                    move || format!("combobox:{debug_id}:toggle")
                                })
                                .px_2()
                                .py_1()
                                .rounded(gpui_px_from_ui(plan.input_radius))
                                .border_1()
                                .border_color(theme.resolve(plan.colors.popup_border()))
                                .text_color(theme.resolve(plan.colors.popup_foreground()))
                                .ui_semantics(&toggle_semantics)
                                .focus_visible(move |style| style.shadow(toggle_focus_shadow))
                                .focusable()
                                .tab_stop(!disabled)
                                .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                                .when(!disabled, |this| {
                                    let input_controller = input_controller.clone();
                                    let window_overlay_runtime = window_overlay_runtime.clone();
                                    let overlay_binding = overlay_binding.clone();
                                    this.cursor_pointer().on_click(
                                        move |_event: &ClickEvent, window, cx| {
                                            cx.stop_propagation();
                                            window.prevent_default();
                                            input_controller.focus_handle(cx).focus(window, cx);
                                            window_overlay_runtime
                                                .request_open_change(
                                                    &overlay_binding,
                                                    !open,
                                                    DismissReason::Trigger,
                                                    window,
                                                    cx,
                                                )
                                                .expect(
                                                    "Combobox toggle should own its registration",
                                                );
                                        },
                                    )
                                })
                                .child(if open { "^" } else { "v" }),
                        ),
                ),
            )
            .when(open, |this| {
                this.child(gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    combobox_content_element(
                        plan.content_id.clone(),
                        plan.listbox_id.clone(),
                        plan.debug_id.clone(),
                        state.clone(),
                        window_overlay_runtime.clone(),
                        overlay_binding.clone(),
                        self.options,
                        self.groups,
                        input_controller.clone(),
                        runtime.clone(),
                        self.on_select.clone(),
                        self.tokens,
                        &theme,
                    ),
                ))
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn combobox_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: ComboboxState,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    options: Vec<ComboboxOption>,
    groups: Vec<ComboboxGroup>,
    input_controller: Entity<TextInputController>,
    runtime: Entity<ComboboxRuntime>,
    on_select: Option<ComboboxSelectionHandler>,
    tokens: ThemeTokens,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let selected_value = state.selected_value().map(str::to_owned);
    let active_value = state.active_value().map(str::to_owned);
    let query = state.query().to_owned();
    let normalized_query = choice::normalize_query(query.as_str());
    let label = state.label().to_owned();
    let listbox_window_overlay_runtime = window_overlay_runtime.clone();
    let listbox_overlay_binding = overlay_binding.clone();
    let listbox = options
        .into_iter()
        .filter(|option| option.matches_normalized_query(normalized_query.as_str()))
        .fold(
            Listbox::new(listbox_id, label.clone()),
            |listbox, option| listbox.option(option.listbox_option()),
        )
        .groups(
            groups
                .into_iter()
                .filter_map(|group| group.filtered_listbox_group(normalized_query.as_str())),
        )
        .tokens(tokens)
        .with_size(state.size())
        .empty_label(state.empty_label().to_owned())
        .embedded(true)
        .editor_owned_focus()
        .on_select({
            let input_controller = input_controller.clone();
            let runtime = runtime.clone();
            let on_select = on_select.clone();
            move |selection, window, cx| {
                let payload = ComboboxSelection::new(
                    selection.value().to_owned(),
                    selection.label().to_owned(),
                );
                let payload_value = payload.value().to_owned();
                let payload_label = payload.label().to_owned();
                let input_controller = input_controller.clone();
                let on_select = on_select.clone();
                commit_registered_choice_overlay_single_value(
                    &listbox_window_overlay_runtime,
                    &listbox_overlay_binding,
                    runtime.clone(),
                    payload_value,
                    window,
                    cx,
                    move |window, cx| {
                        input_controller.update(cx, |controller, cx| {
                            controller.set_value(payload_label, cx);
                        });
                        if let Some(on_select) = on_select.as_ref() {
                            on_select(payload, window, cx);
                        }
                    },
                );
            }
        });
    let listbox = if let Some(selected_value) = selected_value {
        listbox.selected(selected_value)
    } else {
        listbox
    };
    let listbox = if let Some(active_value) = active_value {
        listbox.active(active_value)
    } else {
        listbox
    };
    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();
    let content_semantics =
        SemanticDescriptor::new(state.content_role()).with_label(label.as_ref());

    window_overlay_runtime.surface(
        &overlay_binding,
        OverlayInsideRegionId::new("surface"),
        format!("combobox:{debug_id}:surface-region"),
        div()
            .id(content_id)
            .debug_selector(move || format!("combobox:{debug_id}:content"))
            .min_w(gpui_px_from_ui(metrics.popup_min_width()))
            .max_w(gpui_px_from_ui(metrics.popup_max_width()))
            .p(gpui_px_from_ui(metrics.popup_padding()))
            .h(gpui_px_from_ui(metrics.popup_max_height()))
            .flex()
            .flex_col()
            .rounded(gpui_px_from_ui(metrics.popup_radius()))
            .border_1()
            .border_color(theme.resolve(colors.popup_border()))
            .bg(theme.resolve(colors.popup_background()))
            .text_color(theme.resolve(colors.popup_foreground()))
            .shadow_lg()
            .occlude()
            .ui_semantics(&content_semantics)
            .child(
                ScrollArea::new(scroll_viewport_id, listbox)
                    .vertical()
                    .preserve_scroll()
                    .with_size(state.size()),
            ),
    )
}
