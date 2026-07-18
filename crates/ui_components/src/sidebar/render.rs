use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_ui_core::{AccessibleAction, SemanticDescriptor, Sizable, ui_px};

use super::{
    Sidebar, SidebarActivation, SidebarItem, SidebarItemState, SidebarSection,
    SidebarSectionDescriptor, SidebarSide, SidebarState, SidebarVariant, sidebar_navigation_target,
};
use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationKeyPolicy};
use crate::debug_selector::{
    AuthoredSnapshot, AuthoredSnapshotFingerprint, StableValueItemRenderIdentity,
    StableValueRenderIdentity, StableValueRenderIdentityInput, debug_selector_element_id,
};
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::stable_value_focus::StableValueFocusRuntime;
use crate::theme::ThemeResolver;
use crate::tooltip::Tooltip;

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let Sidebar {
            id,
            label,
            side,
            variant,
            collapse_mode,
            collapsed,
            disabled,
            selected_value,
            focused_value,
            size,
            tokens,
            sections,
            on_activate,
            mut activation_handles,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = debug_selector_element_id(&id);
            let descriptors: Vec<SidebarSectionDescriptor> =
                sections.iter().map(SidebarSection::descriptor).collect();
            let item_models: Vec<SidebarItem> = sections
                .iter()
                .flat_map(|section| section.item_models().iter().cloned())
                .collect();
            let focused_seed = focused_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| {
                StableValueFocusRuntime::new(focused_seed)
            });
            let physically_focused = window.focused(cx);
            let runtime_snapshot = {
                let runtime = runtime.read(cx);
                runtime
                    .resolved_value(physically_focused.as_ref())
                    .map(str::to_owned)
            };
            let state = SidebarState::resolve(
                side,
                variant,
                collapse_mode,
                collapsed,
                disabled,
                label.to_string(),
                selected_value.as_deref(),
                runtime_snapshot.as_deref(),
                descriptors.clone(),
                size,
                tokens,
            );
            let section_render_identities = StableValueRenderIdentity::resolve(
                "sidebar",
                &debug_id,
                "section",
                descriptors.iter().map(|section| {
                    StableValueRenderIdentityInput::new(
                        section.value(),
                        sidebar_section_authored_snapshot(section),
                    )
                }),
            );
            let item_identity_inputs = descriptors
                .iter()
                .enumerate()
                .flat_map(|(section_index, section)| {
                    section
                        .item_descriptors()
                        .iter()
                        .map(move |item| (section_index, item))
                })
                .map(|(section_index, item)| {
                    let snapshot = AuthoredSnapshot::new()
                        .opaque_fingerprint(
                            section_render_identities[section_index].occurrence_fingerprint(),
                        )
                        .opaque_fingerprint(&sidebar_item_authored_snapshot(item))
                        .finish();
                    StableValueRenderIdentityInput::new(item.value(), snapshot)
                });
            let item_render_identities = StableValueRenderIdentity::resolve(
                "sidebar",
                &debug_id,
                "item",
                item_identity_inputs,
            )
            .into_iter()
            .map(|identity| StableValueItemRenderIdentity::from_render_identity(identity, "item"))
            .collect::<Vec<_>>();
            let fallback_focus_handle = runtime.update(cx, |runtime, cx| {
                runtime.sync(
                    state
                        .items()
                        .iter()
                        .filter(|item| item.focusable())
                        .map(SidebarItemState::value),
                    state.focused_value(),
                    physically_focused.as_ref(),
                    cx,
                )
            });
            if let Some(focus_handle) = fallback_focus_handle {
                focus_handle.focus(window, cx);
            }

            let metrics = state.metrics();
            let colors = state.colors();
            let focus_ring = state.focus_ring();
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| !item.focusable())
                    .collect::<Vec<_>>(),
            );
            let navigation_values = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| item.value().to_owned())
                    .collect::<Vec<_>>(),
            );
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
                        let activation = SidebarActivation::for_item(item);
                        let activation_runtime = runtime.clone();
                        let activation_handler = item_models[index]
                            .on_activate
                            .clone()
                            .or_else(|| on_activate.clone());
                        let activation_handle = activation_handles.remove(item.value());

                        ActivationBinding::new(
                            window,
                            cx,
                            item_render_identities[index].activation_state_key.clone(),
                            item.activation_enabled(),
                            ActivationKeyPolicy::EnterOrSpace,
                            move |input, window, cx| {
                                let focus_handle = activation_runtime.update(cx, |runtime, cx| {
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
                        .with_programmatic_handle(activation_handle)
                    })
                    .collect::<Vec<_>>(),
            );
            let icon_collapsed = state.icon_collapsed();
            let item_states = Rc::new(state.items().to_vec());
            let section_states = state.sections().to_vec();
            let sections_content = div()
                .flex()
                .flex_col()
                .gap(gpui_px_from_ui(metrics.section_gap()))
                .p(gpui_px_from_ui(metrics.padding()))
                .children(
                    section_states
                        .into_iter()
                        .map(|section| {
                            let section_items = item_states
                                .iter()
                                .filter(|item| item.section_index() == section.index())
                                .cloned()
                                .collect::<Vec<_>>();
                            let section_identity =
                                section_render_identities[section.index()].clone();
                            let section_element_id = section_identity.element_id;
                            let section_debug_selector = section_identity.debug_selector;
                            let item_render_identities = item_render_identities.clone();
                            let activation_bindings = activation_bindings.clone();
                            let focus_handles = focus_handles.clone();
                            let runtime = runtime.clone();
                            let disabled_items = disabled_items.clone();
                            let navigation_values = navigation_values.clone();
                            let section_label_color = theme.resolve(colors.muted_foreground());
                            let item_theme = theme.clone();
                            let section_semantics =
                                SemanticDescriptor::new(section.role()).with_label(section.label());

                            div()
                                .id(section_element_id)
                                .debug_selector(move || section_debug_selector.clone())
                                .ui_semantics(&section_semantics)
                                .flex()
                                .flex_col()
                                .gap(gpui_px_from_ui(metrics.item_gap()))
                                .when(!icon_collapsed, |this| {
                                    this.child(
                                        div()
                                            .px(gpui_px_from_ui(metrics.item_padding_x()))
                                            .text_xs()
                                            .line_height(gpui_px_from_ui(metrics.text_size()))
                                            .text_color(section_label_color)
                                            .child(section.label().to_owned()),
                                    )
                                })
                                .children(
                                    section_items
                                        .into_iter()
                                        .map(move |item| {
                                            let item_index = item.index();
                                            let activation =
                                                activation_bindings[item_index].clone();
                                            let item_identity =
                                                item_render_identities[item_index].clone();
                                            let item_element_id = item_identity.element_id;
                                            let item_debug_selector = item_identity.debug_selector;
                                            let focus_handle = focus_handles[item_index].clone();
                                            let key_runtime = runtime.clone();
                                            let disabled_items = disabled_items.clone();
                                            let navigation_values = navigation_values.clone();
                                            let item_disabled = item.disabled();
                                            let item_focusable = item.focusable();
                                            let item_selected = item.selected();
                                            let item_tab_stop = item.focused();
                                            let item_icon = item
                                                .icon_label()
                                                .map(SharedString::from)
                                                .unwrap_or_else(|| {
                                                    fallback_icon_label(item.label())
                                                });
                                            let item_label = item.label().to_owned();
                                            let item_badge = item.badge_label().map(str::to_owned);
                                            let item_action = item
                                                .action_label_text()
                                                .map(str::to_owned)
                                                .or_else(|| item.shortcut().map(str::to_owned));
                                            let item_tooltip = item.tooltip().map(str::to_owned);
                                            let item_accessibility_description =
                                                item.accessibility_description().map(str::to_owned);
                                            let item_disabled_reason =
                                                item.disabled_reason_ref().map(str::to_owned);
                                            let item_aria_label = item_accessibility_description
                                                .as_ref()
                                                .or(item_disabled_reason.as_ref())
                                                .map_or_else(
                                                    || item.label().to_owned(),
                                                    |description| {
                                                        format!("{}, {description}", item.label())
                                                    },
                                                );
                                            let item_position = item.position_in_set();
                                            let item_size_of_set = item.size_of_set();
                                            let item_background =
                                                item_theme.resolve(if item_selected {
                                                    colors.item_selected_background()
                                                } else {
                                                    colors.item_background()
                                                });
                                            let item_foreground =
                                                item_theme.resolve(if item_disabled {
                                                    colors.item_disabled_foreground()
                                                } else {
                                                    colors.foreground()
                                                });
                                            let item_hover_background =
                                                item_theme.resolve(colors.item_hover_background());
                                            let badge_background =
                                                item_theme.resolve(colors.badge_background());
                                            let badge_foreground =
                                                item_theme.resolve(colors.badge_foreground());
                                            let action_foreground =
                                                item_theme.resolve(colors.muted_foreground());
                                            let item_focus_shadow = focus_ring_shadow_with_theme(
                                                focus_ring,
                                                &item_theme,
                                            );
                                            let item_actions: &[AccessibleAction] =
                                                if item_focusable {
                                                    &[
                                                        AccessibleAction::Click,
                                                        AccessibleAction::Focus,
                                                    ]
                                                } else {
                                                    &[]
                                                };
                                            let mut item_semantics =
                                                SemanticDescriptor::new(item.role())
                                                    .with_label(&item_aria_label)
                                                    .with_selected(item_selected)
                                                    .with_disabled(item_disabled)
                                                    .with_actions(item_actions);
                                            if let Some(position) = item_position {
                                                item_semantics = item_semantics
                                                    .with_position_in_set(position)
                                                    .with_size_of_set(item_size_of_set);
                                            }

                                            let item_element = div()
                                                .id(item_element_id)
                                                .debug_selector(move || item_debug_selector.clone())
                                                .when(item_focusable, |this| {
                                                    this.focusable().tab_stop(item_tab_stop)
                                                })
                                                .ui_semantics(&item_semantics)
                                                .when_some(focus_handle, |this, focus_handle| {
                                                    this.track_focus(&focus_handle)
                                                })
                                                .min_h(gpui_px_from_ui(metrics.item_height()))
                                                .px(gpui_px_from_ui(if icon_collapsed {
                                                    ui_px(0.0)
                                                } else {
                                                    metrics.item_padding_x()
                                                }))
                                                .py(gpui_px_from_ui(metrics.item_padding_y()))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .gap_2()
                                                .rounded(gpui_px_from_ui(metrics.radius()))
                                                .bg(item_background)
                                                .text_size(gpui_px_from_ui(metrics.text_size()))
                                                .line_height(gpui_px_from_ui(metrics.text_size()))
                                                .text_color(item_foreground)
                                                .focus_visible(move |style| {
                                                    style.shadow(item_focus_shadow.clone())
                                                })
                                                .when(!item_disabled, |this| {
                                                    this.cursor_pointer().hover(move |style| {
                                                        style.bg(item_hover_background)
                                                    })
                                                })
                                                .when(item_disabled, |this| {
                                                    this.opacity(0.56).cursor_not_allowed()
                                                })
                                                .on_key_down({
                                                    move |event: &KeyDownEvent, window, cx| {
                                                        if item_disabled
                                                            || event.keystroke.modifiers.modified()
                                                            || window.default_prevented()
                                                        {
                                                            return;
                                                        }

                                                        let Some(target_index) =
                                                            sidebar_navigation_target(
                                                                event.keystroke.key.as_str(),
                                                                item_index,
                                                                &disabled_items,
                                                            )
                                                        else {
                                                            return;
                                                        };

                                                        let target_value =
                                                            &navigation_values[target_index];
                                                        let focus_handle = key_runtime.update(
                                                            cx,
                                                            |runtime, cx| {
                                                                runtime
                                                                    .set_focused(target_value, cx)
                                                            },
                                                        );

                                                        if let Some(focus_handle) = focus_handle {
                                                            focus_handle.focus(window, cx);
                                                        }

                                                        cx.stop_propagation();
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .min_w(gpui_px_from_ui(metrics.icon_size()))
                                                        .text_size(gpui_px_from_ui(
                                                            metrics.icon_size(),
                                                        ))
                                                        .line_height(gpui_px_from_ui(
                                                            metrics.icon_size(),
                                                        ))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(item_icon),
                                                )
                                                .when(!icon_collapsed, |this| {
                                                    this.child(
                                                        div()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .overflow_hidden()
                                                            .child(item_label),
                                                    )
                                                    .when_some(item_badge, |this, badge| {
                                                        this.child(
                                                            div()
                                                                .min_h(gpui_px_from_ui(
                                                                    metrics.badge_min_height(),
                                                                ))
                                                                .px(gpui_px_from_ui(ui_px(7.0)))
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .rounded(gpui_px_from_ui(ui_px(
                                                                    999.0,
                                                                )))
                                                                .bg(badge_background)
                                                                .text_color(badge_foreground)
                                                                .text_xs()
                                                                .child(badge),
                                                        )
                                                    })
                                                    .when_some(item_action, |this, action| {
                                                        this.child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(action_foreground)
                                                                .child(action),
                                                        )
                                                    })
                                                })
                                                .when_some(item_tooltip, |this, tooltip| {
                                                    this.tooltip(Tooltip::scoped(
                                                        item_theme.clone(),
                                                        Tooltip::text(tooltip),
                                                    ))
                                                });

                                            activation.bind(item_element)
                                        })
                                        .collect::<Vec<_>>(),
                                )
                        })
                        .collect::<Vec<_>>(),
                );
            let offcanvas_collapsed = state.offcanvas_collapsed();
            let semantics = SemanticDescriptor::new(state.role())
                .with_label(state.label())
                .with_disabled(state.disabled());

            let scroll_id = format!("sidebar:{debug_id}:scroll");
            let sidebar = div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("sidebar:{debug_id}")
                })
                .ui_semantics(&semantics)
                .w(gpui_px_from_ui(metrics.resolved_width()))
                .h_full()
                .flex_none()
                .flex()
                .flex_col()
                .overflow_hidden()
                .border_color(theme.resolve(colors.border()))
                .bg(theme.resolve(match variant {
                    SidebarVariant::Docked => colors.surface(),
                    SidebarVariant::Floating | SidebarVariant::Inset => colors.floating_surface(),
                }))
                .text_color(theme.resolve(colors.foreground()))
                .when(
                    variant == SidebarVariant::Docked && side == SidebarSide::Left,
                    |this| this.border_r_1(),
                )
                .when(
                    variant == SidebarVariant::Docked && side == SidebarSide::Right,
                    |this| this.border_l_1(),
                )
                .when(variant != SidebarVariant::Docked, |this| {
                    this.border_1().rounded(gpui_px_from_ui(metrics.radius()))
                })
                .when(!offcanvas_collapsed, move |this| {
                    this.child(
                        ScrollArea::new(scroll_id, sections_content)
                            .vertical()
                            .with_size(size),
                    )
                });

            if offcanvas_collapsed {
                activation_bindings
                    .iter()
                    .cloned()
                    .fold(sidebar, |sidebar, binding| {
                        binding.bind_programmatic(sidebar)
                    })
            } else {
                sidebar
            }
        })
    }
}

fn sidebar_section_authored_snapshot(
    section: &SidebarSectionDescriptor,
) -> AuthoredSnapshotFingerprint {
    let mut snapshot = AuthoredSnapshot::new()
        .field(section.value())
        .field(section.label());
    for item in section.item_descriptors() {
        snapshot = snapshot.opaque_fingerprint(&sidebar_item_authored_snapshot(item));
    }
    snapshot.finish()
}

fn sidebar_item_authored_snapshot(
    item: &super::SidebarItemDescriptor,
) -> AuthoredSnapshotFingerprint {
    AuthoredSnapshot::new()
        .field(item.value())
        .field(item.label())
        .resolved_icon(item.icon_ref())
        .optional_field(item.badge_label())
        .optional_field(item.action_label_text())
        .optional_field(item.disabled_reason_ref())
        .optional_field(item.shortcut_ref())
        .optional_field(item.tooltip_ref())
        .optional_field(item.accessibility_description_ref())
        .finish()
}

fn fallback_icon_label(label: &str) -> SharedString {
    label
        .chars()
        .next()
        .map(|ch| ch.to_string())
        .unwrap_or_default()
        .into()
}
