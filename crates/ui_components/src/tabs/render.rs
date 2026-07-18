use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role, SemanticDescriptor, Sizable};

use super::{
    Tabs, TabsItemDescriptor, TabsItemState, TabsSelection, TabsSelectionAuthority, TabsState,
    tabs_choice_policy,
};
use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationKeyPolicy};
use crate::choice::ChoiceActivationMode;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeResolver;

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let Tabs {
            id,
            orientation,
            activation_mode,
            selection,
            default_selected_value,
            size,
            tokens,
            items,
            on_selection_change,
            activation_handles,
        } = self;
        let tabs_id = id.to_string();
        let panel_id = tabs_panel_id();

        window.with_id(id.clone(), |window| {
            let descriptors: Vec<TabsItemDescriptor> =
                items.iter().map(super::TabsItem::descriptor).collect();
            let selection_controlled = selection.controlled();
            let selected_seed = selection.initial_value(default_selected_value.as_ref());
            let runtime = window.use_keyed_state("runtime", cx, |_, _| TabsRuntime {
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
                TabsSelectionAuthority::Uncontrolled(runtime_snapshot.0.as_deref())
            };
            let state = TabsState::resolve(
                orientation,
                activation_mode,
                size,
                selection_authority,
                runtime_snapshot.1.as_deref(),
                descriptors.clone(),
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let is_vertical = matches!(orientation, Orientation::Vertical);
            let tablist_element_id: ElementId = "tablist".into();
            let tablist_scroll_id = format!("tabs:{tabs_id}:tablist-scroll");
            let panel_node_id = window.with_global_id(panel_id.clone(), |global_id, _| {
                global_id.accesskit_node_id()
            });
            let tab_node_ids = window.with_id(tablist_element_id.clone(), |window| {
                let resolve_trigger_ids = |window: &mut Window| {
                    state
                        .items()
                        .iter()
                        .map(|item| {
                            window.with_global_id(tabs_trigger_id(item.value()), |global_id, _| {
                                global_id.accesskit_node_id()
                            })
                        })
                        .collect::<Vec<_>>()
                };

                if is_vertical {
                    ScrollArea::with_content_global_id_scope(
                        window,
                        &tablist_scroll_id,
                        resolve_trigger_ids,
                    )
                } else {
                    resolve_trigger_ids(window)
                }
            });

            let selected_panel = if let Some(selected_index) = state.selected_index() {
                items
                    .into_iter()
                    .enumerate()
                    .find_map(|(index, item)| (index == selected_index).then_some(item.panel))
                    .unwrap_or_else(|| div().into_any_element())
            } else {
                div().into_any_element()
            };

            let item_descriptors = Rc::new(descriptors);
            let disabled = Rc::new(
                state
                    .items()
                    .iter()
                    .map(TabsItemState::disabled)
                    .collect::<Vec<_>>(),
            );
            let selected_index = state.selected_index();
            let selected_tab_node_id = selected_index.map(|index| tab_node_ids[index]);
            let selected_tab_node_ids = selected_tab_node_id.into_iter().collect::<Vec<_>>();
            let controlled_panel_node_ids = selected_index
                .map(|_| panel_node_id)
                .into_iter()
                .collect::<Vec<_>>();
            let tablist_semantics =
                SemanticDescriptor::new(Role::TabList).with_orientation(orientation);
            let panel_semantics = selected_index.map(|_| {
                SemanticDescriptor::new(Role::TabPanel).with_labelled_by(&selected_tab_node_ids)
            });
            let colors = state.colors();
            let metrics = state.metrics();
            let focus_ring = FocusRing::from_color(colors.focus_ring());
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
                            format!("tab-activation:{}", descriptor.value()),
                            !item.disabled(),
                            ActivationKeyPolicy::EnterOrSpace,
                            move |_, window, cx| {
                                let outcome = activation_runtime.update(cx, |runtime, cx| {
                                    runtime.activate(index, &descriptor, selection_controlled, cx)
                                });

                                if let Some(focus_handle) = outcome.focus_handle {
                                    focus_handle.focus(window, cx);
                                }
                                if let Some(selection) = outcome.selection
                                    && let Some(handler) = activation_handler.clone()
                                {
                                    handler(selection, window, cx);
                                }
                            },
                        )
                        .with_programmatic_handle(activation_handle)
                    })
                    .collect::<Vec<_>>(),
            );
            let tab_stop_index = state.tab_stop_index();
            let tab_triggers = state
                .items()
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let descriptor = item_descriptors[index].clone();
                    let disabled = disabled.clone();
                    let key_runtime = runtime.clone();
                    let key_item_descriptors = item_descriptors.clone();
                    let activation = activation_bindings[index].clone();
                    let navigation_activations = activation_bindings.clone();
                    let item_index = index;
                    let is_selected = item.selected();
                    let is_tab_stop = Some(index) == tab_stop_index;
                    let focus_handle = focus_handles[index].clone();
                    let tab_border = theme.resolve(if is_selected {
                        colors.tab_border_selected()
                    } else {
                        colors.tab_border()
                    });
                    let tab_background = theme.resolve(if is_selected {
                        colors.tab_background_selected()
                    } else {
                        colors.tab_background()
                    });
                    let tab_text = theme.resolve(if is_selected {
                        colors.tab_text()
                    } else {
                        colors.tab_text_muted()
                    });
                    let tab_hover_background = theme.resolve(colors.tab_hover_background());
                    let tab_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
                    let tab_semantics = SemanticDescriptor::new(Role::Tab)
                        .with_label(item.label())
                        .with_selected(is_selected)
                        .with_disabled(item.disabled())
                        .with_controls(&controlled_panel_node_ids)
                        .with_position_in_set(item_index + 1)
                        .with_size_of_set(state.items().len())
                        .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);

                    activation
                        .bind(
                            div()
                                .id(tabs_trigger_id(item.value()))
                                .debug_selector({
                                    let tabs_id = tabs_id.clone();
                                    let value = descriptor.value().to_owned();
                                    move || format!("tabs:{tabs_id}:trigger:{value}")
                                })
                                .focusable()
                                .tab_stop(is_tab_stop)
                                .when_some(focus_handle, |this, focus_handle| {
                                    this.track_focus(&focus_handle)
                                })
                                .ui_semantics_with_relations(&tab_semantics, |node_id| *node_id)
                                .flex_none()
                                .min_h(gpui_px_from_ui(metrics.tab_min_height()))
                                .px(gpui_px_from_ui(metrics.tab_padding_x()))
                                .py(gpui_px_from_ui(metrics.tab_padding_y()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(gpui_px_from_ui(metrics.radius()))
                                .border_1()
                                .border_color(tab_border)
                                .bg(tab_background)
                                .text_size(gpui_px_from_ui(metrics.text_size()))
                                .line_height(gpui_px_from_ui(metrics.text_size()))
                                .text_color(tab_text)
                                .font_weight(if is_selected {
                                    open_gpui::FontWeight::BOLD
                                } else {
                                    open_gpui::FontWeight::NORMAL
                                })
                                .focus_visible(move |style| style.shadow(tab_focus_shadow.clone()))
                                .when(!item.disabled(), |this| {
                                    this.cursor_pointer()
                                        .hover(move |style| style.bg(tab_hover_background))
                                })
                                .when(item.disabled(), |this| {
                                    this.opacity(0.56).cursor_not_allowed()
                                })
                                .on_key_down({
                                    let descriptor = descriptor.clone();
                                    let disabled = disabled.clone();
                                    move |event: &KeyDownEvent, window, cx| {
                                        if descriptor.disabled_state()
                                            || event.keystroke.modifiers.modified()
                                            || window.default_prevented()
                                        {
                                            return;
                                        }

                                        let key = event.keystroke.key.as_str();
                                        let Some(target_index) =
                                            tabs_choice_policy(orientation, activation_mode)
                                                .navigation_target_index(
                                                    key, item_index, &disabled,
                                                )
                                        else {
                                            return;
                                        };

                                        let target = &key_item_descriptors[target_index];
                                        let target_value = target.value().to_owned();
                                        let activate =
                                            tabs_choice_policy(orientation, activation_mode)
                                                .activation_mode()
                                                == ChoiceActivationMode::Automatic;
                                        let handled = if activate {
                                            navigation_activations[target_index]
                                                .programmatic(window, cx)
                                        } else {
                                            let focus_handle =
                                                key_runtime.update(cx, |runtime, cx| {
                                                    runtime.set_focused_only(&target_value, cx)
                                                });
                                            if let Some(focus_handle) = focus_handle {
                                                focus_handle.focus(window, cx);
                                            }
                                            true
                                        };

                                        if handled {
                                            cx.stop_propagation();
                                        }
                                    }
                                })
                                .child(descriptor.label().to_string()),
                        )
                        .into_any_element()
                })
                .collect::<Vec<AnyElement>>();
            let tablist = if is_vertical {
                div()
                    .id(tablist_element_id.clone())
                    .debug_selector({
                        let tabs_id = tabs_id.clone();
                        move || format!("tabs:{tabs_id}:tablist")
                    })
                    .ui_semantics(&tablist_semantics)
                    .flex()
                    .flex_col()
                    .flex_none()
                    .h_full()
                    .min_h(open_gpui::px(0.0))
                    .border_r_1()
                    .border_color(theme.resolve(colors.shell_border()))
                    .child(
                        ScrollArea::new(
                            tablist_scroll_id,
                            div()
                                .flex()
                                .flex_col()
                                .gap(gpui_px_from_ui(metrics.tab_gap()))
                                .p_1()
                                .children(tab_triggers),
                        )
                        .vertical()
                        .with_size(size),
                    )
                    .into_any_element()
            } else {
                div()
                    .id(tablist_element_id)
                    .debug_selector({
                        let tabs_id = tabs_id.clone();
                        move || format!("tabs:{tabs_id}:tablist")
                    })
                    .ui_semantics(&tablist_semantics)
                    .flex()
                    .flex_none()
                    .gap(gpui_px_from_ui(metrics.tab_gap()))
                    .p_1()
                    .border_color(theme.resolve(colors.shell_border()))
                    .flex_row()
                    .flex_wrap()
                    .border_b_1()
                    .children(tab_triggers)
                    .into_any_element()
            };

            div()
                .id(id.clone())
                .w_full()
                .flex()
                .rounded(gpui_px_from_ui(metrics.radius()))
                .border_1()
                .border_color(theme.resolve(colors.shell_border()))
                .bg(theme.resolve(colors.shell_background()))
                .overflow_hidden()
                .when(is_vertical, |this| this.flex_row().h_full())
                .when(!is_vertical, |this| this.flex_col())
                .child(tablist)
                .child(
                    div()
                        .id(panel_id)
                        .when_some(panel_semantics, |this, panel_semantics| {
                            this.ui_semantics_with_relations(&panel_semantics, |node_id| *node_id)
                        })
                        .flex()
                        .flex_1()
                        .min_w(open_gpui::px(0.0))
                        .border_color(theme.resolve(colors.shell_border()))
                        .bg(theme.resolve(colors.panel_background()))
                        .px(gpui_px_from_ui(metrics.panel_padding()))
                        .py(gpui_px_from_ui(metrics.panel_padding()))
                        .when(is_vertical, |this| this.min_w(open_gpui::px(0.0)))
                        .when(!is_vertical, |this| this.border_t_1())
                        .child(selected_panel),
                )
        })
    }
}

#[derive(Debug, Default)]
struct TabsRuntime {
    selected_value: Option<String>,
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

#[derive(Debug)]
struct TabsActivationOutcome {
    selection: Option<TabsSelection>,
    focus_handle: Option<FocusHandle>,
}

impl TabsRuntime {
    fn sync(&mut self, state: &TabsState, items: &[TabsItemDescriptor], cx: &mut Context<Self>) {
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
        descriptor: &TabsItemDescriptor,
        controlled: bool,
        cx: &mut Context<Self>,
    ) -> TabsActivationOutcome {
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

        TabsActivationOutcome {
            selection: selection_changed.then(|| TabsSelection::from_descriptor(index, descriptor)),
            focus_handle: self.focus_handles.get(descriptor.value()).cloned(),
        }
    }

    fn set_focused_only(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.focused_value.as_deref() != Some(value.as_str());
        self.focused_value = Some(value.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }
}

fn tabs_panel_id() -> ElementId {
    "panel".into()
}

fn tabs_trigger_id(value: &str) -> ElementId {
    format!("tab-{value}").into()
}
