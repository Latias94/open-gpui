use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, Orientation, SemanticDescriptor};

use super::{
    ToggleGroup, ToggleGroupItemDescriptor, ToggleGroupItemState, ToggleGroupSelectionChange,
    ToggleGroupSelectionMode, ToggleGroupState, resolve_toggle_group_selection_change,
    toggle_group_navigation_target,
};
use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationKeyPolicy};
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;

impl RenderOnce for ToggleGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let ToggleGroup {
            id,
            label,
            orientation,
            mode,
            selected_values,
            default_selected_values,
            focused_value,
            selection_required,
            disabled,
            size,
            tokens,
            items,
            on_change,
            activation_handles,
        } = self;

        window.with_id(id.clone(), |window| {
            let label_text = label.to_string();
            let descriptors: Vec<ToggleGroupItemDescriptor> = items
                .iter()
                .map(super::ToggleGroupItem::descriptor)
                .collect();
            let selection_controlled = selected_values.is_some();
            let selected_seed = selected_values
                .clone()
                .unwrap_or_else(|| default_selected_values.clone());
            let focused_seed = focused_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| ToggleGroupRuntime {
                selected_values: selected_seed,
                focused_value: focused_seed,
                focus_handles: BTreeMap::new(),
            });
            let (runtime_selected, runtime_focused) = {
                let runtime = runtime.read(cx);
                (
                    runtime.selected_values.clone(),
                    runtime.focused_value.clone(),
                )
            };
            let state = ToggleGroupState::resolve(
                orientation,
                mode,
                selection_required,
                disabled,
                label_text.clone(),
                selected_values.clone().unwrap_or(runtime_selected),
                runtime_focused.as_deref(),
                descriptors.clone(),
                size,
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let metrics = state.metrics();
            let colors = state.colors();
            let selected_colors = state.selected_colors();
            let focus_ring = state.focus_ring();
            let is_vertical = matches!(orientation, Orientation::Vertical);
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| !item.focusable())
                    .collect::<Vec<_>>(),
            );
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let focusable_set_size = state.items().iter().filter(|item| item.focusable()).count();
            let tab_stop_index = state.tab_stop_index();
            let item_descriptors = Rc::new(descriptors);
            let activation_bindings = Rc::new(
                state
                    .items()
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let descriptor = item_descriptors[index].clone();
                        let activation_runtime = runtime.clone();
                        let activation_handler = on_change.clone();
                        let activation_handle = activation_handles.get(descriptor.value()).cloned();

                        ActivationBinding::new(
                            window,
                            cx,
                            format!("toggle-group-activation:{}", descriptor.value()),
                            item.focusable(),
                            ActivationKeyPolicy::Space,
                            move |_, window, cx| {
                                let outcome = activation_runtime.update(cx, |runtime, cx| {
                                    runtime.activate(
                                        index,
                                        &descriptor,
                                        mode,
                                        selection_required,
                                        selection_controlled,
                                        cx,
                                    )
                                });

                                if let Some(focus_handle) = outcome.focus_handle {
                                    focus_handle.focus(window, cx);
                                }
                                if let Some(change) = outcome.change
                                    && let Some(handler) = activation_handler.clone()
                                {
                                    handler(change, window, cx);
                                }
                            },
                        )
                        .with_programmatic_handle(activation_handle)
                    })
                    .collect::<Vec<_>>(),
            );
            let mut focusable_position = 0usize;
            let semantics = SemanticDescriptor::new(state.role())
                .with_label(state.label())
                .with_orientation(orientation)
                .with_disabled(state.disabled());

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = id.to_string();
                    move || format!("toggle-group:{debug_id}")
                })
                .ui_semantics(&semantics)
                .flex()
                .gap(gpui_px_from_ui(metrics.gap()))
                .p(gpui_px_from_ui(metrics.padding()))
                .rounded(gpui_px_from_ui(metrics.radius()))
                .border_1()
                .border_color(theme.resolve(colors.border()))
                .bg(theme.resolve(colors.background()))
                .when(is_vertical, |this| this.flex_col().items_stretch())
                .when(!is_vertical, |this| this.flex_row().items_center())
                .children(state.items().iter().enumerate().map(|(index, item)| {
                    let descriptor = item_descriptors[index].clone();
                    let key_descriptors = item_descriptors.clone();
                    let key_runtime = runtime.clone();
                    let disabled_items = disabled_items.clone();
                    let activation = activation_bindings[index].clone();
                    let focus_handle = focus_handles[index].clone();
                    let item_tab_stop = Some(index) == tab_stop_index;
                    let item_disabled = item.disabled();
                    let item_selected = item.selected();
                    let item_label = item.label().to_owned();
                    let item_value = item.value().to_owned();
                    let item_position = if item.focusable() {
                        focusable_position += 1;
                        Some(focusable_position)
                    } else {
                        None
                    };
                    let item_border = theme.resolve(if item_selected {
                        selected_colors.border()
                    } else {
                        colors.border()
                    });
                    let item_background = theme.resolve(if item_selected {
                        selected_colors.background()
                    } else {
                        colors.background()
                    });
                    let item_foreground = theme.resolve(if item_selected {
                        selected_colors.foreground()
                    } else {
                        colors.foreground()
                    });
                    let item_hover_background = theme.resolve(colors.hover_background());
                    let item_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
                    let item_actions: &[AccessibleAction] = if item.focusable() {
                        &[AccessibleAction::Click, AccessibleAction::Focus]
                    } else {
                        &[AccessibleAction::Focus]
                    };
                    let mut item_semantics = SemanticDescriptor::new(item.role())
                        .with_label(&item_label)
                        .with_toggled(item.toggled())
                        .with_disabled(item_disabled)
                        .with_actions(item_actions);
                    if let Some(position) = item_position {
                        item_semantics = item_semantics
                            .with_position_in_set(position)
                            .with_size_of_set(focusable_set_size);
                    }

                    activation.bind(
                        div()
                            .id(format!("toggle-group-item:{item_value}"))
                            .debug_selector({
                                let group_id = id.to_string();
                                let item_value = item_value.clone();
                                move || format!("toggle-group:{group_id}:item:{item_value}")
                            })
                            .focusable()
                            .tab_stop(item_tab_stop)
                            .ui_semantics(&item_semantics)
                            .when_some(focus_handle, |this, focus_handle| {
                                this.track_focus(&focus_handle)
                            })
                            .min_h(gpui_px_from_ui(metrics.item().height()))
                            .px(gpui_px_from_ui(metrics.item().padding_x()))
                            .py(gpui_px_from_ui(metrics.item().padding_y()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(gpui_px_from_ui(metrics.item().radius()))
                            .border_1()
                            .border_color(item_border)
                            .bg(item_background)
                            .text_color(item_foreground)
                            .text_size(gpui_px_from_ui(metrics.item().text_size()))
                            .line_height(gpui_px_from_ui(metrics.item().text_size()))
                            .focus_visible(move |style| style.shadow(item_focus_shadow.clone()))
                            .when(!item_disabled, |this| {
                                this.cursor_pointer()
                                    .hover(move |style| style.bg(item_hover_background))
                            })
                            .when(item_disabled, |this| {
                                this.opacity(0.56).cursor_not_allowed()
                            })
                            .on_key_down({
                                let disabled_items = disabled_items.clone();
                                move |event: &KeyDownEvent, window, cx| {
                                    if item_disabled
                                        || event.keystroke.modifiers.modified()
                                        || window.default_prevented()
                                    {
                                        return;
                                    }

                                    let Some(target_index) = toggle_group_navigation_target(
                                        orientation,
                                        event.keystroke.key.as_str(),
                                        index,
                                        &disabled_items,
                                    ) else {
                                        return;
                                    };

                                    let target = &key_descriptors[target_index];
                                    let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                        runtime.set_focused(target.value(), cx)
                                    });

                                    if let Some(focus_handle) = focus_handle {
                                        focus_handle.focus(window, cx);
                                    }
                                    cx.stop_propagation();
                                }
                            })
                            .child(descriptor.label().to_string()),
                    )
                }))
        })
    }
}

#[derive(Debug, Default)]
struct ToggleGroupRuntime {
    selected_values: Vec<String>,
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

#[derive(Debug)]
struct ToggleGroupActivationOutcome {
    change: Option<ToggleGroupSelectionChange>,
    focus_handle: Option<FocusHandle>,
}

impl ToggleGroupRuntime {
    fn sync(
        &mut self,
        state: &ToggleGroupState,
        items: &[ToggleGroupItemDescriptor],
        cx: &mut Context<Self>,
    ) {
        self.focus_handles.retain(|value, _| {
            items
                .iter()
                .any(|item| item.value() == value && !item.disabled_state())
        });

        for item in items.iter().filter(|item| !item.disabled_state()) {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.selected_values = state.selected_values().to_vec();
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    fn set_focused(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let changed = self.focused_value.as_deref() != Some(value);
        self.focused_value = Some(value.to_owned());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(value).cloned()
    }

    fn activate(
        &mut self,
        index: usize,
        descriptor: &ToggleGroupItemDescriptor,
        mode: ToggleGroupSelectionMode,
        selection_required: bool,
        controlled: bool,
        cx: &mut Context<Self>,
    ) -> ToggleGroupActivationOutcome {
        let change = resolve_toggle_group_selection_change(
            mode,
            selection_required,
            &self.selected_values,
            ToggleGroupItemState {
                index,
                value: descriptor.value().to_owned(),
                label: descriptor.label().to_owned(),
                selected: self
                    .selected_values
                    .iter()
                    .any(|value| value == descriptor.value()),
                disabled: descriptor.disabled_state(),
                focused: self.focused_value.as_deref() == Some(descriptor.value()),
            },
        );
        let selection_changed = change.is_some();
        let focus_changed = self.focused_value.as_deref() != Some(descriptor.value());

        if let Some(change) = change.as_ref()
            && !controlled
        {
            self.selected_values = change.selected_values().to_vec();
        }
        if focus_changed {
            self.focused_value = Some(descriptor.value().to_owned());
        }
        if (selection_changed && !controlled) || focus_changed {
            cx.notify();
        }

        ToggleGroupActivationOutcome {
            change,
            focus_handle: self.focus_handles.get(descriptor.value()).cloned(),
        }
    }
}
