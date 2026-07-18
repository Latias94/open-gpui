use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, Context, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_ui_core::{AccessibleAction, SemanticDescriptor};

use super::{
    RadioGroup, RadioGroupState, RadioItemDescriptor, RadioItemState, RadioSelection,
    RadioSelectionAuthority,
};
use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationKeyPolicy};
use crate::choice::ChoiceInteractionPolicy;
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let RadioGroup {
            id,
            label,
            orientation,
            selection,
            default_selected_value,
            disabled,
            read_only,
            required,
            size,
            tokens,
            items,
            on_selection_change,
            activation_handles,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = id.to_string();
            let descriptors: Vec<RadioItemDescriptor> =
                items.iter().map(super::RadioItem::descriptor).collect();
            let selection_controlled = selection.controlled();
            let selected_seed = selection.initial_value(default_selected_value.as_ref());
            let runtime = window.use_keyed_state("runtime", cx, |_, _| RadioRuntime {
                selected_value: selected_seed.clone(),
                focused_value: selected_seed,
                focus_handles: BTreeMap::new(),
            });
            let runtime_snapshot = {
                let runtime = runtime.read(cx);
                (
                    runtime.selected_value.clone(),
                    runtime.focused_value.clone(),
                )
            };
            let selection_authority = if selection_controlled {
                selection.authority(None)
            } else {
                RadioSelectionAuthority::Uncontrolled(runtime_snapshot.0.as_deref())
            };
            let state = RadioGroupState::resolve(
                orientation,
                size,
                disabled,
                required,
                selection_authority,
                runtime_snapshot.1.as_deref(),
                descriptors.clone(),
                tokens,
            )
            .with_read_only(read_only);
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let item_descriptors = Rc::new(descriptors);
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(RadioItemState::disabled)
                    .collect::<Vec<_>>(),
            );
            let metrics = state.metrics();
            let colors = state.colors();
            let focus_ring = state.focus_ring();
            let is_vertical = matches!(orientation, open_gpui_ui_core::Orientation::Vertical);
            let tab_stop_index = state.tab_stop_index();
            let label = label.unwrap_or_else(|| "Radio group".into());
            let semantics = SemanticDescriptor::new(state.role())
                .with_label(label.as_ref())
                .with_orientation(orientation)
                .with_required(state.required())
                .with_read_only(state.read_only())
                .with_disabled(state.disabled());
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let activation_bindings = Rc::new(
                state
                    .items()
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let descriptor = item_descriptors[index].clone();
                        let activation_runtime = runtime.clone();
                        let activation_handler = on_selection_change.clone();
                        let activation_handle = activation_handles.get(descriptor.value()).cloned();

                        ActivationBinding::new(
                            window,
                            cx,
                            format!("radio-activation:{}", descriptor.value()),
                            item.activation_enabled(),
                            ActivationKeyPolicy::Space,
                            move |_activation, window, cx| {
                                let outcome = activation_runtime.update(cx, |runtime, cx| {
                                    runtime.activate(index, &descriptor, selection_controlled, cx)
                                });

                                if let Some(selection) = outcome.selection
                                    && let Some(handler) = activation_handler.clone()
                                {
                                    handler(selection, window, cx);
                                }
                                if let Some(focus_handle) = outcome.focus_handle {
                                    focus_handle.focus(window, cx);
                                }
                            },
                        )
                        .with_programmatic_handle(activation_handle)
                    })
                    .collect::<Vec<_>>(),
            );

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("radio-group:{debug_id}")
                })
                .ui_semantics(&semantics)
                .flex()
                .gap(gpui_px_from_ui(metrics.item_gap()))
                .when(is_vertical, |this| this.flex_col())
                .when(!is_vertical, |this| this.flex_row().flex_wrap())
                .children(state.items().iter().enumerate().map(|(index, item)| {
                    let descriptor = item_descriptors[index].clone();
                    let disabled_items = disabled_items.clone();
                    let focus_handle = focus_handles[index].clone();
                    let activation = activation_bindings[index].clone();
                    let navigation_activations = activation_bindings.clone();
                    let item_index = index;
                    let is_selected = item.selected();
                    let is_tab_stop = Some(index) == tab_stop_index;
                    let activation_enabled = item.activation_enabled();
                    let item_value = item.value().to_owned();
                    let label_color = theme.resolve(if item.disabled() {
                        colors.label_muted()
                    } else {
                        colors.label()
                    });
                    let hover_background = theme.resolve(colors.hover_background());
                    let control_border = theme.resolve(if is_selected {
                        colors.control_border_selected()
                    } else {
                        colors.control_border()
                    });
                    let control_background = theme.resolve(if is_selected {
                        colors.control_background_selected()
                    } else {
                        colors.control_background()
                    });
                    let indicator_color = theme.resolve(colors.indicator());
                    let item_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
                    let item_actions: &[AccessibleAction] = if item.activation_enabled() {
                        &[AccessibleAction::Click, AccessibleAction::Focus]
                    } else {
                        &[AccessibleAction::Focus]
                    };
                    let item_semantics = SemanticDescriptor::new(item.role())
                        .with_label(item.label())
                        .with_selected(is_selected)
                        .with_read_only(item.read_only())
                        .with_disabled(item.disabled())
                        .with_position_in_set(item_index + 1)
                        .with_size_of_set(state.items().len())
                        .with_actions(item_actions);

                    activation.bind(
                        div()
                            .id(radio_item_id(item.value()))
                            .debug_selector({
                                let debug_id = debug_id.clone();
                                let item_value = item_value.clone();
                                move || format!("radio-group:{debug_id}:item:{item_value}")
                            })
                            .focusable()
                            .tab_stop(is_tab_stop)
                            .ui_semantics(&item_semantics)
                            .when_some(focus_handle, |this, focus_handle| {
                                this.track_focus(&focus_handle)
                            })
                            .flex()
                            .items_center()
                            .gap_2()
                            .px(gpui_px_from_ui(metrics.item_padding_x()))
                            .py(gpui_px_from_ui(metrics.item_padding_y()))
                            .rounded(gpui_px_from_ui(metrics.radius()))
                            .text_size(gpui_px_from_ui(metrics.label_text_size()))
                            .line_height(gpui_px_from_ui(metrics.label_text_size()))
                            .text_color(label_color)
                            .focus_visible(move |style| style.shadow(item_focus_shadow.clone()))
                            .when(item.activation_enabled(), |this| {
                                this.cursor_pointer()
                                    .hover(move |style| style.bg(hover_background))
                            })
                            .when(item.disabled(), |this| {
                                this.opacity(0.56).cursor_not_allowed()
                            })
                            .on_key_down({
                                let disabled_items = disabled_items.clone();
                                move |event: &KeyDownEvent, window, cx| {
                                    if !activation_enabled || event.keystroke.modifiers.modified() {
                                        return;
                                    }

                                    let Some(target_index) =
                                        ChoiceInteractionPolicy::single_required(orientation)
                                            .navigation_target_index(
                                                event.keystroke.key.as_str(),
                                                item_index,
                                                &disabled_items,
                                            )
                                    else {
                                        return;
                                    };

                                    if navigation_activations[target_index].programmatic(window, cx)
                                    {
                                        cx.stop_propagation();
                                    }
                                }
                            })
                            .child(
                                div()
                                    .w(gpui_px_from_ui(metrics.control_size()))
                                    .h(gpui_px_from_ui(metrics.control_size()))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(gpui_px_from_ui(metrics.control_size()))
                                    .border_1()
                                    .border_color(control_border)
                                    .bg(control_background)
                                    .child(if is_selected {
                                        div()
                                            .w(gpui_px_from_ui(metrics.indicator_size()))
                                            .h(gpui_px_from_ui(metrics.indicator_size()))
                                            .rounded(gpui_px_from_ui(metrics.indicator_size()))
                                            .bg(indicator_color)
                                    } else {
                                        div().w(px(0.0)).h(px(0.0))
                                    }),
                            )
                            .child(descriptor.label().to_string()),
                    )
                }))
        })
    }
}

#[derive(Debug, Default)]
struct RadioRuntime {
    selected_value: Option<String>,
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

#[derive(Debug)]
struct RadioActivationOutcome {
    selection: Option<RadioSelection>,
    focus_handle: Option<FocusHandle>,
}

impl RadioRuntime {
    fn sync(
        &mut self,
        state: &RadioGroupState,
        items: &[RadioItemDescriptor],
        cx: &mut Context<Self>,
    ) {
        self.focus_handles
            .retain(|value, _| items.iter().any(|item| item.value() == value));

        for item in items {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.selected_value = state.selected_value().map(str::to_owned);
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    fn activate(
        &mut self,
        index: usize,
        descriptor: &RadioItemDescriptor,
        controlled: bool,
        cx: &mut Context<Self>,
    ) -> RadioActivationOutcome {
        let selection_changed = self.selected_value.as_deref() != Some(descriptor.value());
        let focus_changed = self.focused_value.as_deref() != Some(descriptor.value());

        if selection_changed && !controlled {
            self.selected_value = Some(descriptor.value().to_owned());
        }
        if focus_changed {
            self.focused_value = Some(descriptor.value().to_owned());
        }
        if (selection_changed && !controlled) || focus_changed {
            cx.notify();
        }

        RadioActivationOutcome {
            selection: selection_changed
                .then(|| RadioSelection::from_descriptor(index, descriptor)),
            focus_handle: self.focus_handles.get(descriptor.value()).cloned(),
        }
    }
}

fn radio_item_id(value: &str) -> ElementId {
    format!("radio-{value}").into()
}
