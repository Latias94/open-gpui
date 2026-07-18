use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role, SemanticDescriptor};

use super::{
    Toolbar, ToolbarActivation, ToolbarColors, ToolbarItem, ToolbarItemKind, ToolbarItemState,
    ToolbarState, toolbar_activation_key_policy, toolbar_navigation_target,
};
use crate::a11y::UiA11yElementExt;
use crate::activation::ActivationBinding;
use crate::button::ButtonVariant;
use crate::color::ColorIntent;
use crate::debug_selector::{
    AuthoredSnapshot, AuthoredSnapshotFingerprint, StableValueItemRenderIdentity,
    StableValueRenderIdentity, StableValueRenderIdentityInput, debug_selector_element_id,
};
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::stable_value_focus::StableValueFocusRuntime;
use crate::theme::ThemeResolver;
use crate::tooltip::Tooltip;

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let Toolbar {
            id,
            label,
            orientation,
            focused_value,
            disabled,
            size,
            tokens,
            items,
            on_activate,
            mut activation_handles,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = debug_selector_element_id(&id);
            let focused_seed = focused_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| {
                StableValueFocusRuntime::new(focused_seed)
            });
            let physically_focused = window.focused(cx);
            let runtime_snapshot = runtime
                .read(cx)
                .resolved_value(physically_focused.as_ref())
                .map(str::to_owned);
            let descriptors = items
                .iter()
                .map(ToolbarItem::descriptor)
                .collect::<Vec<_>>();
            let state = ToolbarState::resolve(
                orientation,
                size,
                disabled,
                label.to_string(),
                runtime_snapshot.as_deref(),
                descriptors.iter().cloned(),
                tokens,
            );
            let item_render_identities = StableValueRenderIdentity::resolve(
                "toolbar",
                &debug_id,
                "item",
                items.iter().zip(&descriptors).map(|(item, descriptor)| {
                    StableValueRenderIdentityInput::new(
                        descriptor.value(),
                        toolbar_item_authored_snapshot(item, descriptor),
                    )
                }),
            )
            .into_iter()
            .zip(state.items())
            .map(|(identity, item)| {
                StableValueItemRenderIdentity::from_render_identity(identity, item.kind().as_str())
            })
            .collect::<Vec<_>>();
            let fallback_focus_handle = runtime.update(cx, |runtime, cx| {
                runtime.sync(
                    state
                        .items()
                        .iter()
                        .filter(|item| item.focusable())
                        .map(ToolbarItemState::value),
                    state.focused_value(),
                    physically_focused.as_ref(),
                    cx,
                )
            });
            if let Some(focus_handle) = fallback_focus_handle {
                focus_handle.focus(window, cx);
            }

            let disabled_items = Rc::new(state.disabled_map());
            let navigation_values = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| item.value().to_owned())
                    .collect::<Vec<_>>(),
            );
            let metrics = state.metrics();
            let colors = state.colors();
            let pressed_colors = ThemeResolver::button_colors(tokens, ButtonVariant::Ghost, true);
            let focus_ring = state.focus_ring();
            let is_vertical = matches!(orientation, Orientation::Vertical);
            let item_border = theme.resolve(colors.border());
            let item_foreground = theme.resolve(colors.foreground());
            let item_hover_background = theme.resolve(colors.hover_background());
            let item_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handle(item.value()))
                    .collect::<Vec<_>>()
            };
            let activation_bindings = Rc::new(
                state
                    .items()
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let key_policy = toolbar_activation_key_policy(item.kind())?;
                        let activation = ToolbarActivation::for_item(item);
                        let activation_runtime = runtime.clone();
                        let activation_handler = items[index]
                            .on_activate
                            .clone()
                            .or_else(|| on_activate.clone());
                        let activation_handle = activation_handles.remove(item.value());

                        Some(
                            ActivationBinding::new(
                                window,
                                cx,
                                item_render_identities[index].activation_state_key.clone(),
                                item.activation_enabled(),
                                key_policy,
                                move |input, window, cx| {
                                    let focus_handle = activation_runtime
                                        .update(cx, |runtime, cx| {
                                            runtime.set_focused(activation.value(), cx)
                                        });

                                    if let Some(focus_handle) = focus_handle {
                                        focus_handle.focus(window, cx);
                                    }
                                    if let Some(handler) = activation_handler.clone() {
                                        handler(activation.clone(), input, window, cx);
                                    }
                                },
                            )
                            .with_programmatic_handle(activation_handle),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            let focusable_set_size = state.items().iter().filter(|item| item.focusable()).count();
            let mut focusable_position = 0usize;
            let tab_stop_index = state.tab_stop_index();
            let semantics = SemanticDescriptor::new(state.role())
                .with_label(state.label())
                .with_orientation(orientation)
                .with_disabled(state.disabled());

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("toolbar:{debug_id}")
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
                .when(!is_vertical, |this| {
                    this.flex_row().items_center().flex_wrap()
                })
                .children(state.items().iter().enumerate().map(|(index, item)| {
                    let activation = activation_bindings[index].clone();
                    let item_selector = item_render_identities[index].debug_selector.clone();
                    let item_id = item_render_identities[index].element_id.clone();
                    let Some(activation) = activation else {
                        return div()
                            .id(item_id)
                            .debug_selector(move || item_selector.clone())
                            .flex_none()
                            .bg(item_border)
                            .when(is_vertical, |this| {
                                this.w_full()
                                    .h(gpui_px_from_ui(metrics.separator_thickness()))
                            })
                            .when(!is_vertical, |this| {
                                this.w(gpui_px_from_ui(metrics.separator_thickness()))
                                    .h(gpui_px_from_ui(metrics.separator_length()))
                            })
                            .into_any_element();
                    };

                    let visible_label = items[index]
                        .visible_label
                        .clone()
                        .or_else(|| item.icon_label().map(SharedString::from));
                    let item_tooltip =
                        toolbar_custom_tooltip(&items[index], item.duplicate_value());
                    let item_tooltip_text = item
                        .tooltip()
                        .map(str::to_owned)
                        .filter(|_| item_tooltip.is_none());
                    let custom_tooltip_theme = theme.clone();
                    let text_tooltip_theme = theme.clone();
                    let navigation_values = navigation_values.clone();
                    let disabled_items = disabled_items.clone();
                    let focus_handle = focus_handles[index].clone();
                    let key_runtime = runtime.clone();
                    let item_index = index;
                    let item_kind = item.kind();
                    let item_disabled = item.disabled();
                    let item_focusable = item.focusable();
                    let item_tab_stop = Some(index) == tab_stop_index;
                    let item_pressed = item.pressed();
                    let item_accessibility_description =
                        item.accessibility_description().map(str::to_owned);
                    let item_disabled_reason = item.disabled_reason_ref().map(str::to_owned);
                    let item_aria_label = item_accessibility_description
                        .as_ref()
                        .or(item_disabled_reason.as_ref())
                        .map_or_else(
                            || item.label().to_owned(),
                            |description| format!("{}, {description}", item.label()),
                        );
                    let item_position = if item.focusable() {
                        focusable_position += 1;
                        Some(focusable_position)
                    } else {
                        None
                    };
                    let item_background = theme.resolve(toolbar_item_background(
                        colors,
                        pressed_colors,
                        item_kind,
                        item_pressed,
                    ));
                    let item_focus_shadow = item_focus_shadow.clone();
                    let item_actions: &[AccessibleAction] = if item_focusable {
                        &[AccessibleAction::Click, AccessibleAction::Focus]
                    } else {
                        &[]
                    };
                    let mut item_semantics =
                        SemanticDescriptor::new(item.role().unwrap_or(Role::Button))
                            .with_label(&item_aria_label)
                            .with_disabled(item_disabled)
                            .with_actions(item_actions);
                    if let Some(position) = item_position {
                        item_semantics = item_semantics
                            .with_position_in_set(position)
                            .with_size_of_set(focusable_set_size);
                    }
                    if let Some(toggled) = item.toggled() {
                        item_semantics = item_semantics.with_toggled(toggled);
                    }

                    activation
                        .bind(
                            div()
                                .id(item_id)
                                .debug_selector(move || item_selector.clone())
                                .when(item_focusable, |this| {
                                    this.focusable().tab_stop(item_tab_stop)
                                })
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
                                .gap_2()
                                .rounded(gpui_px_from_ui(metrics.item().radius()))
                                .border_1()
                                .border_color(item_border)
                                .bg(item_background)
                                .text_size(gpui_px_from_ui(metrics.item().text_size()))
                                .line_height(gpui_px_from_ui(metrics.item().text_size()))
                                .text_color(item_foreground)
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

                                        let Some(target_index) = toolbar_navigation_target(
                                            orientation,
                                            event.keystroke.key.as_str(),
                                            item_index,
                                            &disabled_items,
                                        ) else {
                                            return;
                                        };

                                        let target_value = &navigation_values[target_index];
                                        let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                            runtime.set_focused(target_value, cx)
                                        });
                                        if let Some(focus_handle) = focus_handle {
                                            focus_handle.focus(window, cx);
                                        }

                                        cx.stop_propagation();
                                    }
                                })
                                .when_some(item_tooltip, |this, tooltip| {
                                    this.tooltip(Tooltip::scoped(
                                        custom_tooltip_theme,
                                        move |window, cx| tooltip(window, cx),
                                    ))
                                })
                                .when_some(item_tooltip_text, |this, tooltip| {
                                    this.tooltip(Tooltip::scoped(
                                        text_tooltip_theme,
                                        Tooltip::text(tooltip),
                                    ))
                                })
                                .child(visible_label.unwrap_or_else(|| item.label().into())),
                        )
                        .into_any_element()
                }))
        })
    }
}

fn toolbar_item_background(
    colors: ToolbarColors,
    pressed_colors: ToolbarColors,
    kind: ToolbarItemKind,
    pressed: bool,
) -> ColorIntent {
    match kind {
        ToolbarItemKind::Toggle if pressed => pressed_colors.background(),
        _ => colors.background(),
    }
}

fn toolbar_item_authored_snapshot(
    item: &ToolbarItem,
    descriptor: &super::ToolbarItemDescriptor,
) -> AuthoredSnapshotFingerprint {
    AuthoredSnapshot::new()
        .field(descriptor.value())
        .field(descriptor.label())
        .field(descriptor.kind().as_str())
        .resolved_icon(descriptor.icon_ref())
        .optional_field(item.visible_label.as_deref())
        .optional_field(descriptor.disabled_reason_ref())
        .optional_field(descriptor.shortcut_ref())
        .optional_field(descriptor.tooltip_ref())
        .optional_field(descriptor.accessibility_description_ref())
        .finish()
}

fn toolbar_custom_tooltip(
    item: &ToolbarItem,
    ambiguous: bool,
) -> Option<Rc<dyn Fn(&mut Window, &mut App) -> open_gpui::AnyView>> {
    if ambiguous {
        None
    } else {
        item.tooltip.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_toolbar_items_do_not_bind_unstable_custom_tooltips() {
        let item =
            ToolbarItem::action("duplicate", "Duplicate").tooltip(Tooltip::text("Custom tooltip"));

        assert!(toolbar_custom_tooltip(&item, false).is_some());
        assert!(toolbar_custom_tooltip(&item, true).is_none());
    }
}
