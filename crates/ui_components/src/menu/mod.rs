//! Menu component and shared menu state.

mod descriptor;
mod model;
mod render_plan;
pub(crate) mod runtime;
mod style;
#[cfg(test)]
mod theme_tests;

use crate::geometry::gpui_px_from_ui;
use crate::geometry::{ui_point_from_gpui, ui_size_from_gpui_size};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, AnyView, App, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_command::CommandDescriptor;
use open_gpui_ui_core::{
    AccessibleAction, DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent,
    OutsidePressPolicy, OverlayAnchorInput, OverlayLayerId, OverlayPlacementAlignment,
    OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Rect, Role, SemanticDescriptor,
    Sizable, Size, ThemeTokens, Toggled, UiPx, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::action::{ResolvedActionIcon, ResolvedActionState};
use crate::collection_typeahead::CollectionTypeaheadInput;
use crate::focus::focus_ring_shadow_with_theme;

use crate::overlay::{
    GpuiOverlayPlacement, GpuiOverlayState, OverlayInsideRegionId, OverlayLayerBinding,
    OverlayLayerLeaseStatus, OverlayLayerPhase, OverlayLayerRegistration, OverlayOpenIntent,
    OverlayOwnership, WindowOverlayRuntime, gpui_overlay_state, gpui_positioned_overlay_layer,
    gpui_relative_overlay_layer, resolve_overlay_open_state,
};
use crate::scroll_area::ScrollArea;
use crate::theme::{ThemeContext, ThemeResolver, gpui_elevation_shadow};
use crate::tooltip::Tooltip;
use runtime::{
    MenuBranchBinding, MenuBranchRuntime, MenuRuntime, handle_menu_submenu_surface_hover,
    update_menu_hover_target,
};

pub(crate) use descriptor::menu_open_mode_from_disclosure;
pub use descriptor::{MenuItemDescriptor, MenuItemKind, MenuOpenMode};
pub use model::{
    MenuItemState, MenuSelection, MenuState, MenuSubmenuNavigation, menu_navigation_target,
};
pub(crate) use model::{MenuKeyboardIntent, find_menu_item_state_by_path, menu_path_is_open};
pub use render_plan::{MenuSafeHoverCorridor, MenuSubmenuSurface};
pub use style::{MenuColors, MenuMetrics};

/// Default threshold where menu surfaces become locally scrollable.
pub const DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD: usize = 8;

pub(crate) type MenuBranchBindings = Rc<HashMap<String, OverlayLayerBinding>>;

pub(crate) fn menu_path_key(path: &[String]) -> String {
    path.iter()
        .map(|segment| segment.replace('%', "%25").replace('/', "%2F"))
        .collect::<Vec<_>>()
        .join("/")
}
fn menu_branch_layer_id(layer_prefix: &str, debug_id: &str, path: &[String]) -> String {
    format!("{layer_prefix}:{debug_id}:branch:{}", menu_path_key(path))
}

fn menu_branch_parent_id(
    layer_prefix: &str,
    debug_id: &str,
    root_binding: &OverlayLayerBinding,
    path: &[String],
) -> OverlayLayerId {
    if path.len() == 1 {
        root_binding.lease().layer_id().clone()
    } else {
        OverlayLayerId::new(menu_branch_layer_id(
            layer_prefix,
            debug_id,
            &path[..path.len() - 1],
        ))
    }
}

fn menu_branch_trigger_value(path: &[String]) -> String {
    path.last()
        .and_then(|segment| segment.split_once(':'))
        .map_or_else(String::new, |(_, value)| value.to_owned())
}

fn collect_menu_branch_paths(items: &[MenuItemState], paths: &mut Vec<Vec<String>>) {
    for item in items {
        if item.has_submenu() {
            paths.push(item.path().to_vec());
        }
        collect_menu_branch_paths(item.children(), paths);
    }
}

fn menu_branch_outside_policy(root: OutsidePressPolicy) -> OutsidePressPolicy {
    match root {
        OutsidePressPolicy::Consume | OutsidePressPolicy::DismissAndConsume => {
            OutsidePressPolicy::DismissAndPassThrough
        }
        policy => policy,
    }
}

fn menu_branch_registration<T: MenuBranchRuntime>(
    layer_prefix: &str,
    debug_id: &str,
    state: &MenuState,
    path: &[String],
    presence: OverlayPresence,
    root_binding: &OverlayLayerBinding,
    runtime: &open_gpui::Entity<T>,
) -> OverlayLayerRegistration {
    let path = path.to_vec();
    let focused_value = menu_branch_trigger_value(&path);
    let runtime = runtime.downgrade();
    let policy = state
        .overlay()
        .policy()
        .clone()
        .with_presence(presence)
        .with_outside_press_policy(menu_branch_outside_policy(state.outside_press_policy()));

    OverlayLayerRegistration::new(
        menu_branch_layer_id(layer_prefix, debug_id, &path),
        policy,
        OverlayOwnership::Uncontrolled,
    )
    .parent_id(menu_branch_parent_id(
        layer_prefix,
        debug_id,
        root_binding,
        &path,
    ))
    .uncontrolled_commit(move |open, _, cx| {
        if open {
            return;
        }
        let _ = runtime.update(cx, |runtime, _| {
            runtime.commit_branch_closed(&path, focused_value.clone());
        });
    })
}

fn schedule_menu_branch_cleanup_frame<T: MenuBranchRuntime>(
    runtime: &open_gpui::Entity<T>,
    window: &mut Window,
    cx: &mut App,
) {
    let should_schedule = runtime.update(cx, |runtime, _| {
        runtime.branch_layers_mut().schedule_cleanup_frame()
    });
    if !should_schedule {
        return;
    }

    let runtime = runtime.downgrade();
    window.on_next_frame(move |_, cx| {
        let _ = runtime.update(cx, |runtime, cx| {
            runtime.branch_layers_mut().finish_cleanup_frame();
            cx.notify();
        });
    });
    window.refresh();
}

fn menu_branch_lease_status(
    window_overlay_runtime: &WindowOverlayRuntime,
    branch: &MenuBranchBinding,
    window: &Window,
    cx: &App,
) -> OverlayLayerLeaseStatus {
    window_overlay_runtime
        .component_binding_status(&branch.binding, window, cx)
        .expect("menu branch binding should belong to its window runtime")
}

pub(crate) fn sync_menu_branch_bindings<T: MenuBranchRuntime>(
    layer_prefix: &str,
    debug_id: &str,
    state: &MenuState,
    runtime: &open_gpui::Entity<T>,
    window_overlay_runtime: &WindowOverlayRuntime,
    root_binding: &OverlayLayerBinding,
    window: &mut Window,
    cx: &mut App,
) -> MenuBranchBindings {
    let sync_epoch = runtime.update(cx, |runtime, _| {
        runtime.sync_resolved_open_path(state.open_path());
        runtime.branch_layers_mut().advance_sync_epoch()
    });
    let mut paths = Vec::new();
    collect_menu_branch_paths(state.items(), &mut paths);
    paths.sort_by_key(Vec::len);
    let valid_keys = paths
        .iter()
        .map(|path| menu_path_key(path))
        .collect::<HashSet<_>>();
    let open_keys = paths
        .iter()
        .filter(|path| state.open() && menu_path_is_open(path, state.open_path()))
        .map(|path| menu_path_key(path))
        .collect::<HashSet<_>>();
    let mut retained = runtime.read(cx).branch_layers().bindings().clone();
    let branch_opening_theme = root_binding
        .opening_theme()
        .unwrap_or_else(|| ThemeResolver::current(window, cx));

    let released_keys = retained
        .iter()
        .filter_map(|(key, branch)| {
            (menu_branch_lease_status(window_overlay_runtime, branch, window, cx)
                == OverlayLayerLeaseStatus::Released)
                .then(|| key.clone())
        })
        .collect::<Vec<_>>();
    for key in released_keys {
        retained.remove(&key);
    }

    for (key, branch) in &mut retained {
        if valid_keys.contains(key) {
            if menu_branch_lease_status(window_overlay_runtime, branch, window, cx)
                != OverlayLayerLeaseStatus::PendingUnregister
            {
                branch.stale_since_epoch = None;
            }
        } else if branch.stale_since_epoch.is_none() {
            branch.stale_since_epoch = Some(sync_epoch);
        }
    }

    let mut closing = retained
        .values()
        .filter(|branch| {
            let key = menu_path_key(&branch.path);
            matches!(
                menu_branch_lease_status(window_overlay_runtime, branch, window, cx),
                OverlayLayerLeaseStatus::Registered { phase }
                    if phase != OverlayLayerPhase::Hidden
            ) && (!valid_keys.contains(&key) || !open_keys.contains(&key))
        })
        .cloned()
        .collect::<Vec<_>>();
    closing.sort_by_key(|branch| std::cmp::Reverse(branch.path.len()));
    for branch in closing {
        window_overlay_runtime
            .bind_component_layer_with_theme(
                runtime,
                Some(&branch.binding),
                menu_branch_registration(
                    layer_prefix,
                    debug_id,
                    state,
                    &branch.path,
                    OverlayPresence::Hidden,
                    root_binding,
                    runtime,
                ),
                branch_opening_theme.clone(),
                window,
                cx,
            )
            .expect("inactive menu branches should become noninteractive");
    }

    let ready_stale_paths = retained
        .values()
        .filter(|branch| {
            matches!(
                menu_branch_lease_status(window_overlay_runtime, branch, window, cx),
                OverlayLayerLeaseStatus::Registered { .. }
            ) && !valid_keys.contains(&menu_path_key(&branch.path))
                && branch
                    .stale_since_epoch
                    .is_some_and(|epoch| epoch != sync_epoch)
        })
        .map(|branch| branch.path.clone())
        .collect::<Vec<_>>();
    let stale_roots = ready_stale_paths
        .iter()
        .filter(|path| {
            !ready_stale_paths.iter().any(|ancestor| {
                ancestor.len() < path.len() && path.starts_with(ancestor.as_slice())
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    for root_path in stale_roots {
        let root_key = menu_path_key(&root_path);
        let binding = retained
            .get(&root_key)
            .expect("stale menu root should retain its binding until cleanup")
            .binding
            .clone();
        window_overlay_runtime
            .unregister_component_subtree(&binding, window, cx)
            .expect("stale menu subtree should unregister from its window runtime");
    }

    let released_keys = retained
        .iter()
        .filter_map(|(key, branch)| {
            (menu_branch_lease_status(window_overlay_runtime, branch, window, cx)
                == OverlayLayerLeaseStatus::Released)
                .then(|| key.clone())
        })
        .collect::<Vec<_>>();
    for key in released_keys {
        retained.remove(&key);
    }

    let mut available_keys = HashSet::new();
    for path in &paths {
        let key = menu_path_key(&path);
        let parent_available =
            path.len() == 1 || available_keys.contains(&menu_path_key(&path[..path.len() - 1]));
        if !parent_available
            || retained.get(&key).is_some_and(|branch| {
                menu_branch_lease_status(window_overlay_runtime, branch, window, cx)
                    == OverlayLayerLeaseStatus::PendingUnregister
            })
        {
            continue;
        }
        let should_open = open_keys.contains(&key);
        let existing = retained.get(&key).map(|branch| &branch.binding);
        if should_open && existing.is_some() {
            available_keys.insert(key);
            continue;
        }
        let binding = window_overlay_runtime
            .bind_component_layer_with_theme(
                runtime,
                existing,
                menu_branch_registration(
                    layer_prefix,
                    debug_id,
                    state,
                    path,
                    OverlayPresence::Hidden,
                    root_binding,
                    runtime,
                ),
                branch_opening_theme.clone(),
                window,
                cx,
            )
            .expect("hidden menu branch registration should remain valid");
        retained.insert(
            key.clone(),
            MenuBranchBinding {
                path: path.clone(),
                binding,
                stale_since_epoch: None,
            },
        );
        available_keys.insert(key);
    }

    for path in paths.iter().filter(|path| {
        let key = menu_path_key(path);
        open_keys.contains(&key) && available_keys.contains(&key)
    }) {
        let key = menu_path_key(path);
        let existing = retained.get(&key).map(|branch| &branch.binding);
        let binding = window_overlay_runtime
            .bind_component_layer_with_theme(
                runtime,
                existing,
                menu_branch_registration(
                    layer_prefix,
                    debug_id,
                    state,
                    path,
                    OverlayPresence::Open,
                    root_binding,
                    runtime,
                ),
                branch_opening_theme.clone(),
                window,
                cx,
            )
            .expect("menu branch overlay registration should remain valid");
        retained.insert(
            key.clone(),
            MenuBranchBinding {
                path: path.clone(),
                binding: binding.clone(),
                stale_since_epoch: None,
            },
        );
    }

    let rendered = paths
        .iter()
        .filter_map(|path| {
            let key = menu_path_key(path);
            let binding = retained.get(&key).filter(|branch| {
                matches!(
                    menu_branch_lease_status(window_overlay_runtime, branch, window, cx),
                    OverlayLayerLeaseStatus::Registered { .. }
                )
            })?;
            Some((key, binding.binding.clone()))
        })
        .collect();

    let cleanup_pending = retained.values().any(|branch| {
        branch.stale_since_epoch.is_some()
            || menu_branch_lease_status(window_overlay_runtime, branch, window, cx)
                == OverlayLayerLeaseStatus::PendingUnregister
    });

    runtime.update(cx, |runtime, _| {
        runtime.branch_layers_mut().replace_bindings(retained);
    });
    if cleanup_pending {
        schedule_menu_branch_cleanup_frame(runtime, window, cx);
    }
    Rc::new(rendered)
}

fn menu_item_element(
    item: MenuItem,
    item_state: MenuItemState,
    debug_prefix: &'static str,
    debug_id: String,
    metrics: MenuMetrics,
    colors: MenuColors,
    runtime: open_gpui::Entity<MenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    theme: &ThemeContext,
) -> AnyElement {
    match item_state.kind() {
        MenuItemKind::Header => {
            let semantics = SemanticDescriptor::new(Role::Label).with_label(item_state.label());
            div()
                .id(format!("{debug_prefix}-header:{}", item_state.value()))
                .debug_selector({
                    let header_debug_id = debug_id.clone();
                    let header_value = item_state.value().to_owned();
                    move || format!("{debug_prefix}:{header_debug_id}:header:{header_value}")
                })
                .pl(gpui_px_from_ui(
                    metrics.item_padding_x() + metrics.submenu_indent() * item_state.depth() as f32,
                ))
                .pr(gpui_px_from_ui(metrics.item_padding_x()))
                .pt_2()
                .pb_1()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .text_color(theme.resolve(colors.header_foreground()))
                .ui_semantics(&semantics)
                .child(item_state.label().to_owned())
                .into_any_element()
        }
        MenuItemKind::Separator => {
            let separator_color = theme.resolve(colors.separator());
            let semantics = SemanticDescriptor::new(Role::Separator);

            div()
                .id(format!("{debug_prefix}-separator:{}", item_state.index()))
                .debug_selector({
                    let separator_debug_id = debug_id.clone();
                    let separator_index = item_state.index();
                    move || {
                        format!("{debug_prefix}:{separator_debug_id}:separator:{separator_index}")
                    }
                })
                .h(gpui_px_from_ui(metrics.separator_height()))
                .my_1()
                .bg(separator_color)
                .ui_semantics(&semantics)
                .into_any_element()
        }
        MenuItemKind::Action
        | MenuItemKind::Checkbox
        | MenuItemKind::Radio
        | MenuItemKind::Submenu => {
            let selection = MenuSelection::from_item(&item_state);
            let item_path_key = item_state.path_key();
            let child_binding = branch_bindings.get(&item_path_key).cloned();
            let item_handler = item.on_select.clone();
            let global_handler = on_select.clone();
            let window_overlay_runtime_for_click = window_overlay_runtime.clone();
            let root_binding_for_click = root_binding.clone();
            let item_label = item_state.label().to_owned();
            let icon_label = item_state.icon_label().map(str::to_owned);
            let shortcut = item_state.shortcut().map(str::to_owned);
            let disabled_reason = item_state.disabled_reason_ref().map(str::to_owned);
            let accessibility_description =
                item_state.accessibility_description().map(str::to_owned);
            let item_tooltip = item_state.tooltip().map(str::to_owned);
            let left_padding = metrics.item_padding_x();
            let focused = item_state.focused();
            let disabled = item_state.disabled();
            let focusable = item_state.focusable();
            let toggled = item_state.toggled();
            let has_submenu = item_state.has_submenu();
            let submenu_navigation = if has_submenu {
                item_state
                    .children()
                    .iter()
                    .find(|child| child.focusable())
                    .map(|child| {
                        MenuSubmenuNavigation::new(
                            item_state.path().to_vec(),
                            child.path().to_vec(),
                            child.value().to_owned(),
                        )
                    })
            } else {
                None
            };
            let hover_focusable = item_state.focusable();
            let hover_path = item_state.path().to_vec();
            let hover_value = item_state.value().to_owned();
            let hover_submenu_navigation = submenu_navigation.clone();
            let hover_runtime = runtime.clone();
            let item_background = theme.resolve(if focused {
                colors.item_focus_background()
            } else {
                colors.item_background()
            });
            let item_foreground = theme.resolve(if disabled {
                colors.item_disabled_foreground()
            } else {
                colors.foreground()
            });
            let item_hover_background = theme.resolve(colors.item_hover_background());
            let item_aria_label = accessibility_description
                .as_ref()
                .or(disabled_reason.as_ref())
                .map_or_else(
                    || item_label.clone(),
                    |description| format!("{item_label}, {description}"),
                );
            let item_actions: &[AccessibleAction] = if child_binding.is_some() && focusable {
                &[AccessibleAction::Click, AccessibleAction::Focus]
            } else {
                &[AccessibleAction::Click]
            };
            let mut semantics = SemanticDescriptor::new(Role::MenuItem)
                .with_label(&item_aria_label)
                .with_disabled(disabled)
                .with_actions(item_actions);
            if let Some(toggled) = toggled {
                semantics = semantics.with_toggled(toggled);
            }
            if has_submenu {
                semantics = semantics.with_expanded(item_state.submenu_open());
            }

            let element = div()
                .id(format!("{debug_prefix}-item:{item_path_key}"))
                .debug_selector({
                    let item_debug_id = debug_id.clone();
                    let item_debug_path_key = item_path_key.clone();
                    move || format!("{debug_prefix}:{item_debug_id}:item:{item_debug_path_key}")
                })
                .min_h(gpui_px_from_ui(metrics.item_height()))
                .pl(gpui_px_from_ui(left_padding))
                .pr(gpui_px_from_ui(metrics.item_padding_x()))
                .py(gpui_px_from_ui(metrics.item_padding_y()))
                .flex()
                .items_center()
                .justify_between()
                .rounded(gpui_px_from_ui(metrics.radius()))
                .bg(item_background)
                .text_color(item_foreground)
                .ui_semantics(&semantics)
                .when_some(
                    child_binding.clone().filter(|_| focusable),
                    |this, binding| {
                        this.focusable()
                            .tab_stop(false)
                            .track_focus(binding.trigger_focus())
                    },
                )
                .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                .when(!disabled, |this| {
                    this.cursor_pointer()
                        .hover(move |style| style.bg(item_hover_background))
                        .on_hover(move |hovered, window, cx| {
                            if hover_focusable {
                                update_menu_hover_target(
                                    hover_runtime.clone(),
                                    hover_path.clone(),
                                    hover_value.clone(),
                                    hover_submenu_navigation.clone(),
                                    *hovered,
                                    window,
                                    cx,
                                );
                            }
                        })
                        .on_click(move |_event, window, cx| {
                            cx.stop_propagation();
                            if let Some(submenu_navigation) = submenu_navigation.clone() {
                                runtime.update(cx, |runtime, _| {
                                    runtime.open_path = submenu_navigation.open_path().to_vec();
                                    runtime.focused_path =
                                        Some(submenu_navigation.focused_path().to_vec());
                                    runtime.focused_value =
                                        Some(submenu_navigation.focused_value().to_owned());
                                });
                                return;
                            }
                            let Some(selection) = selection.clone() else {
                                return;
                            };
                            window_overlay_runtime_for_click
                                .request_open_change_with_effect(
                                    &root_binding_for_click,
                                    false,
                                    DismissReason::Selection,
                                    window,
                                    cx,
                                    |window, cx| {
                                        if let Some(item_handler) = item_handler.as_ref() {
                                            item_handler(selection.clone(), window, cx);
                                        }
                                        if let Some(global_handler) = global_handler.as_ref() {
                                            global_handler(selection, window, cx);
                                        }
                                    },
                                )
                                .expect("menu selection should own its root overlay registration");
                        })
                })
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap_2()
                        .when_some(icon_label, |this, icon_label| {
                            this.child(div().flex_none().child(icon_label))
                        })
                        .child(item_label),
                )
                .when_some(shortcut, |this, shortcut| {
                    this.child(
                        div()
                            .ml_4()
                            .text_xs()
                            .text_color(item_foreground)
                            .child(shortcut),
                    )
                })
                .when_some(toggled, |this, toggled| {
                    let marker = if toggled == Toggled::True {
                        "checked"
                    } else {
                        ""
                    };
                    this.child(div().ml_2().child(marker))
                })
                .when(has_submenu, |this| this.child(div().ml_2().child(">")))
                .when_some(item_tooltip, |this, tooltip| {
                    this.tooltip(Tooltip::scoped(theme.clone(), Tooltip::text(tooltip)))
                })
                .into_any_element();

            if let Some(child_binding) = child_binding {
                window_overlay_runtime
                    .inside_region(
                        &child_binding,
                        OverlayInsideRegionId::new("trigger"),
                        format!("menu:{debug_id}:branch:{item_path_key}:trigger-region"),
                        element,
                    )
                    .into_any_element()
            } else {
                element
            }
        }
    }
}

/// A concrete GPUI menu item.
#[derive(Clone)]
pub struct MenuItem {
    descriptor: MenuItemDescriptor,
    children: Vec<MenuItem>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
}

impl MenuItem {
    /// Creates a menu item from a pure descriptor.
    pub fn from_descriptor(descriptor: MenuItemDescriptor) -> Self {
        let children = descriptor
            .children_ref()
            .iter()
            .cloned()
            .map(MenuItem::from_descriptor)
            .collect();
        Self {
            descriptor,
            children,
            on_select: None,
        }
    }

    /// Creates an action menu item from shared app-command metadata.
    pub fn from_command_descriptor(descriptor: &CommandDescriptor) -> Self {
        Self::from_descriptor(MenuItemDescriptor::from_command_descriptor(descriptor))
    }

    /// Creates an action menu item from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        Self::from_descriptor(MenuItemDescriptor::from_resolved_action(action))
    }

    /// Creates an action menu item.
    pub fn action(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::action(value, label.to_string()),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a static section header item.
    pub fn header(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::header(value, label.to_string()),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a checkbox menu item.
    pub fn checkbox(
        value: impl Into<String>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::checkbox(value, label.to_string(), checked),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a radio-style menu item.
    pub fn radio(value: impl Into<String>, label: impl Into<SharedString>, checked: bool) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::radio(value, label.to_string(), checked),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a separator item.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            descriptor: MenuItemDescriptor::separator(value),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a submenu trigger item.
    pub fn submenu(
        value: impl Into<String>,
        label: impl Into<SharedString>,
        children: impl IntoIterator<Item = MenuItem>,
    ) -> Self {
        let label = label.into();
        let children: Vec<MenuItem> = children.into_iter().collect();
        Self {
            descriptor: MenuItemDescriptor::submenu(
                value,
                label.to_string(),
                children.iter().map(MenuItem::descriptor),
            ),
            children,
            on_select: None,
        }
    }

    /// Marks the menu item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Marks the menu item as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.disabled_reason(reason);
        self
    }

    /// Applies app-resolved icon metadata.
    pub fn icon(mut self, icon: ResolvedActionIcon) -> Self {
        self.descriptor = self.descriptor.icon(icon);
        self
    }

    /// Applies caller-owned checked state to checkbox and radio items.
    pub fn checked(mut self, checked: bool) -> Self {
        self.descriptor = self.descriptor.checked(checked);
        self
    }

    /// Applies a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.shortcut(shortcut);
        self
    }

    /// Applies user-displayable tooltip metadata.
    pub fn tooltip_text(mut self, tooltip: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.tooltip(tooltip);
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.accessibility_description(description);
        self
    }

    /// Applies caller-owned availability metadata without evaluating it.
    pub fn when(mut self, when: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.when(when);
        self
    }

    /// Adds one submenu child.
    pub fn child(mut self, child: MenuItem) -> Self {
        if self.descriptor.kind() == MenuItemKind::Submenu {
            let child_descriptor = child.descriptor();
            self.children.push(child.clone());
            self.descriptor = self.descriptor.child(child_descriptor);
        }
        self
    }

    /// Adds many submenu children.
    pub fn children(mut self, children: impl IntoIterator<Item = MenuItem>) -> Self {
        if self.descriptor.kind() == MenuItemKind::Submenu {
            let children: Vec<MenuItem> = children.into_iter().collect();
            self.descriptor = self
                .descriptor
                .children(children.iter().map(MenuItem::descriptor));
            self.children.extend(children);
        }
        self
    }

    /// Registers an item selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(MenuSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns a pure descriptor for this item.
    pub fn descriptor(&self) -> MenuItemDescriptor {
        if self.descriptor.kind() == MenuItemKind::Submenu {
            let mut descriptor = MenuItemDescriptor::submenu(
                self.descriptor.value(),
                self.descriptor.label(),
                self.children.iter().map(MenuItem::descriptor),
            )
            .disabled(self.descriptor.disabled_state());
            if let Some(shortcut) = self.descriptor.shortcut_ref() {
                descriptor = descriptor.shortcut(shortcut);
            }
            if let Some(when) = self.descriptor.when_ref() {
                descriptor = descriptor.when(when);
            }
            return descriptor;
        }

        self.descriptor.clone()
    }

    pub(crate) fn select_handler(
        &self,
    ) -> Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>> {
        self.on_select.clone()
    }

    pub(crate) fn child_items(&self) -> &[MenuItem] {
        &self.children
    }
}

/// A concrete GPUI menu component.
#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    trigger_label: SharedString,
    trigger_icon: Option<SharedString>,
    items: Vec<MenuItem>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    focused_value: Option<String>,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    trigger_tooltip: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyView>>,
    overlay_children: Vec<AnyElement>,
}

impl Menu {
    /// Creates a menu with a trigger label.
    pub fn new(id: impl Into<ElementId>, trigger_label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            trigger_icon: None,
            items: Vec::new(),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            focused_value: None,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_select: None,
            trigger_tooltip: None,
            overlay_children: Vec::new(),
        }
    }

    /// Uses an icon-only trigger while preserving the trigger label for accessibility.
    pub fn trigger_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.trigger_icon = Some(icon.into());
        self
    }

    /// Adds one menu item.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many menu items.
    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Marks the menu trigger as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    /// Applies the default focused item value for adapter-owned runtime state.
    pub fn default_focused_value(mut self, value: impl Into<String>) -> Self {
        self.focused_value = Some(value.into());
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

    /// Applies Escape-key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
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
        let handler = Rc::new(handler);
        self.on_open_change = Some(handler);
        self
    }

    /// Registers a menu selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(MenuSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Adds a hover/focus tooltip to the menu trigger.
    pub fn trigger_tooltip(
        mut self,
        tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self {
        self.trigger_tooltip = Some(Rc::new(tooltip));
        self
    }

    /// Adds an overlay whose logical parent is this Menu's root layer.
    ///
    /// The child's inline trigger subtree also counts as inside the Menu for outside-press
    /// arbitration. Descendant overlay surfaces remain separate regions linked by parentage.
    pub fn overlay_child(mut self, child: impl IntoElement) -> Self {
        self.overlay_children.push(child.into_any_element());
        self
    }

    /// Returns resolved menu state.
    pub fn state(&self) -> MenuState {
        MenuState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.focused_value.as_deref(),
            self.items.iter().map(MenuItem::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Menu {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Menu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let descriptors: Vec<MenuItemDescriptor> =
            self.items.iter().map(MenuItem::descriptor).collect();
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            MenuRuntime::new(self.default_open, self.focused_value.clone())
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.sync_controlled_open(resolved_open);
            });
        }

        let focused_value = runtime_state.resolved_focused_value(self.focused_value.as_deref());
        let state = MenuState::resolve_with_paths(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            focused_value,
            runtime_state.focused_path.as_deref(),
            &runtime_state.open_path,
            descriptors.clone(),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let runtime_state = runtime.read(cx).clone();
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let trigger_label = self.trigger_label;
        let trigger_icon = self.trigger_icon;
        let trigger_tooltip = self.trigger_tooltip;
        let items = self.items;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let overlay_children = self.overlay_children;
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("menu:{debug_id}"),
            state.overlay().policy().clone(),
            ownership,
        );
        if let Some(on_open_change) = on_open_change {
            registration = registration.on_open_change(move |intent, window, cx| {
                on_open_change(intent, window, cx);
            });
        }
        if ownership == OverlayOwnership::Uncontrolled {
            let runtime = runtime.downgrade();
            registration = registration.uncontrolled_commit(move |open, _, cx| {
                let _ = runtime.update(cx, |runtime, _| {
                    runtime.open = open;
                    if !open {
                        runtime.reset_closed_state();
                    }
                });
            });
        }
        let existing_binding = runtime.read(cx).overlay_binding.clone();
        let root_binding = window_overlay_runtime
            .bind_component_layer(
                &runtime,
                existing_binding.as_ref(),
                registration,
                window,
                cx,
            )
            .expect("menu root overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(root_binding.clone());
            });
        }
        let branch_bindings = sync_menu_branch_bindings(
            "menu",
            &debug_id,
            &state,
            &runtime,
            &window_overlay_runtime,
            &root_binding,
            window,
            cx,
        );
        let scroll_handle = runtime_state.scroll_handle.clone();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let icon_only_trigger = trigger_icon.is_some();
        let trigger_content = trigger_icon.unwrap_or_else(|| trigger_label.clone());
        let trigger_border = theme.resolve(colors.trigger_border());
        let trigger_background = theme.resolve(colors.trigger_background());
        let trigger_foreground = theme.resolve(colors.trigger_foreground());
        let trigger_hover_background = theme.resolve(colors.trigger_hover_background());
        let trigger_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let trigger_tooltip_theme = theme.clone();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                open_gpui_ui_core::OverlayAnchorInput::from_layout_bounds(open_gpui_ui_core::rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(metrics.min_width(), metrics.trigger_height()),
                )),
                ui_size(metrics.min_width(), metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );
        let overlay_children = overlay_children
            .into_iter()
            .enumerate()
            .map(|(index, child)| {
                let child = window_overlay_runtime.parent_scope(
                    &root_binding,
                    format!("menu:{debug_id}:overlay-child:{index}"),
                    child,
                );
                window_overlay_runtime.inside_region(
                    &root_binding,
                    OverlayInsideRegionId::new(format!("overlay-child:{index}")),
                    format!("menu:{debug_id}:overlay-child-region:{index}"),
                    child,
                )
            })
            .collect::<Vec<_>>();
        let trigger_semantics = SemanticDescriptor::new(state.trigger_role())
            .with_label(trigger_label.as_ref())
            .with_selected(state.trigger_selected())
            .with_expanded(open)
            .with_disabled(disabled)
            .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);

        div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("menu:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                window_overlay_runtime.inside_region(
                    &root_binding,
                    OverlayInsideRegionId::new("trigger"),
                    format!("menu:{debug_id}:trigger-region"),
                    div()
                        .id(trigger_id)
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            move || format!("menu:{debug_id}:trigger")
                        })
                        .min_h(gpui_px_from_ui(metrics.trigger_height()))
                        .when(!icon_only_trigger, |this| {
                            this.px(gpui_px_from_ui(metrics.trigger_padding_x()))
                        })
                        .when(icon_only_trigger, |this| {
                            this.w(gpui_px_from_ui(metrics.trigger_height()))
                                .min_w(gpui_px_from_ui(metrics.trigger_height()))
                        })
                        .py(gpui_px_from_ui(metrics.trigger_padding_y()))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .border_1()
                        .border_color(trigger_border)
                        .bg(trigger_background)
                        .text_color(trigger_foreground)
                        .text_size(gpui_px_from_ui(metrics.text_size()))
                        .line_height(gpui_px_from_ui(metrics.text_size()))
                        .focusable()
                        .tab_stop(!disabled)
                        .ui_semantics(&trigger_semantics)
                        .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                        .track_focus(root_binding.trigger_focus())
                        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                        .when(!disabled, |this| {
                            let window_overlay_runtime = window_overlay_runtime.clone();
                            let root_binding = root_binding.clone();
                            this.cursor_pointer()
                                .hover(move |style| style.bg(trigger_hover_background))
                                .on_click(move |_event, window, cx| {
                                    cx.stop_propagation();
                                    window_overlay_runtime
                                        .request_open_change(
                                            &root_binding,
                                            !open,
                                            DismissReason::Trigger,
                                            window,
                                            cx,
                                        )
                                        .expect(
                                            "menu trigger should own its root overlay registration",
                                        );
                                })
                        })
                        .when_some(trigger_tooltip, |this, tooltip| {
                            this.tooltip(Tooltip::scoped(
                                trigger_tooltip_theme,
                                move |window, cx| tooltip(window, cx),
                            ))
                        })
                        .child(trigger_content),
                ),
            )
            .children(overlay_children)
            .when(open, |this| {
                this.child(gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    &root_binding,
                    |opening_theme| {
                        menu_content_element(
                            items,
                            content_id.clone(),
                            debug_id.clone(),
                            state.clone(),
                            runtime.clone(),
                            window_overlay_runtime.clone(),
                            root_binding.clone(),
                            branch_bindings.clone(),
                            scroll_handle.clone(),
                            on_select.clone(),
                            opening_theme,
                            cx,
                            overlay_adapter.snap_margin(),
                            overlay_adapter.deferred_priority(),
                        )
                        .into_any_element()
                    },
                ))
            })
    }
}

fn menu_content_element(
    items: Vec<MenuItem>,
    content_id: ElementId,
    debug_id: String,
    state: MenuState,
    runtime: open_gpui::Entity<MenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    scroll_handle: ScrollHandle,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    theme: &ThemeContext,
    cx: &mut App,
    snap_margin: open_gpui::Pixels,
    deferred_priority: usize,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let key_state = state.clone();
    let key_runtime = runtime.clone();
    let key_select = on_select.clone();
    let key_window_overlay_runtime = window_overlay_runtime.clone();
    let key_root_binding = root_binding.clone();
    let key_items = visible_menu_items(&items, state.open_path());
    let root_branch = menu_branch_surface(
        &items,
        &state,
        &[],
        None,
        debug_id.clone(),
        runtime.clone(),
        window_overlay_runtime.clone(),
        root_binding.clone(),
        branch_bindings.clone(),
        on_select.clone(),
        Some(scroll_handle),
        theme,
        cx,
        snap_margin,
        deferred_priority,
    );
    let content_semantics =
        SemanticDescriptor::new(state.content_role()).with_actions(&[AccessibleAction::Focus]);

    let content = div()
        .id(content_id)
        .debug_selector({
            let content_debug_id = debug_id.clone();
            move || format!("menu:{content_debug_id}:content")
        })
        .focusable()
        .relative()
        .tab_group()
        .track_focus(root_binding.surface_focus())
        .ui_semantics(&content_semantics)
        .text_color(theme.resolve(colors.foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if window.default_prevented() {
                return;
            }

            let current_path = key_runtime
                .read(cx)
                .focused_path
                .clone()
                .or_else(|| key_state.focused_path().map(|path| path.to_vec()));
            let command_input =
                !event.keystroke.modifiers.modified() && !event.prefer_character_input;
            if command_input
                && let Some(intent) = key_state.keyboard_intent_for_key_from_path(
                    event.keystroke.key.as_str(),
                    current_path.as_deref(),
                )
            {
                match intent {
                    MenuKeyboardIntent::NavigateSubmenu(target) => {
                        cx.stop_propagation();
                        window.prevent_default();
                        key_runtime.update(cx, |runtime, cx| {
                            runtime.apply_submenu_target(&target, cx);
                        });
                    }
                    MenuKeyboardIntent::FocusItem {
                        focused_path,
                        focused_value,
                    } => {
                        cx.stop_propagation();
                        window.prevent_default();
                        key_runtime.update(cx, |runtime, cx| {
                            runtime.focus_item(focused_path, focused_value, cx);
                        });
                    }
                    MenuKeyboardIntent::Activate(selection) => {
                        cx.stop_propagation();
                        window.prevent_default();
                        key_window_overlay_runtime
                            .request_open_change_with_effect(
                                &key_root_binding,
                                false,
                                DismissReason::Selection,
                                window,
                                cx,
                                |window, cx| {
                                    if let Some(item_handler) = key_items
                                        .iter()
                                        .zip(key_state.visible_items())
                                        .find(|(_, item_state)| {
                                            item_state.path() == selection.path()
                                        })
                                        .and_then(|(item, _)| item.select_handler())
                                        .as_ref()
                                    {
                                        item_handler(selection.clone(), window, cx);
                                    }
                                    if let Some(on_select) = key_select.as_ref() {
                                        on_select(selection, window, cx);
                                    }
                                },
                            )
                            .expect("menu keyboard selection should own the root registration");
                    }
                }
                return;
            }

            let now = cx.background_executor().now();
            let update = key_runtime.update(cx, |runtime, _| {
                runtime
                    .typeahead
                    .push(CollectionTypeaheadInput::from_key_down(event), now)
            });
            let Some(update) = update else {
                return;
            };

            cx.stop_propagation();
            window.prevent_default();
            if let Some(target) = key_state.typeahead_target_from_path(
                update.match_query(),
                current_path.as_deref(),
                update.searches_after_current(),
            ) {
                key_runtime.update(cx, |runtime, cx| {
                    runtime.focus_item(target.path().to_vec(), target.value().to_owned(), cx);
                });
            }
        })
        .child(root_branch);

    window_overlay_runtime.surface(
        &root_binding,
        OverlayInsideRegionId::new("root-surface"),
        format!("menu:{debug_id}:root-surface-region"),
        content,
    )
}

fn menu_branch_surface(
    items: &[MenuItem],
    state: &MenuState,
    branch_path: &[String],
    surface_id: Option<ElementId>,
    debug_id: String,
    runtime: open_gpui::Entity<MenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    scroll_handle: Option<ScrollHandle>,
    theme: &ThemeContext,
    cx: &mut App,
    snap_margin: open_gpui::Pixels,
    deferred_priority: usize,
) -> AnyElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let branch_key = if branch_path.is_empty() {
        "root".to_string()
    } else {
        menu_path_key(branch_path)
    };
    let branch_binding = if branch_path.is_empty() {
        None
    } else {
        let Some(binding) = branch_bindings.get(&branch_key).cloned() else {
            return div().into_any_element();
        };
        Some(binding)
    };
    let Some((branch_items, branch_states)) =
        menu_branch_items_and_states(items, state.items(), branch_path)
    else {
        return div().into_any_element();
    };
    let scroll_handle = match (branch_path.is_empty(), scroll_handle) {
        (true, Some(scroll_handle)) => scroll_handle,
        (true, None) => ScrollHandle::new(),
        (false, _) => runtime.update(cx, |runtime, _| runtime.submenu_scroll_handle(&branch_key)),
    };
    let branch_scrollable_content =
        branch_states.len() > DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD;
    let scroll_viewport_id = if branch_path.is_empty() {
        format!("menu:{debug_id}:content-scroll")
    } else {
        format!("menu:{debug_id}:submenu:{branch_key}:scroll")
    };
    let row_path_keys: Vec<String> = branch_states.iter().map(MenuItemState::path_key).collect();
    let rows = div()
        .flex()
        .flex_col()
        .gap_1()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let row_path_keys = row_path_keys.clone();
            move |row_bounds, _window, cx| {
                runtime.update(cx, |runtime, _| {
                    for (path_key, bounds) in row_path_keys.iter().zip(row_bounds.into_iter()) {
                        runtime
                            .submenu_trigger_bounds
                            .insert(path_key.clone(), menu_bounds_to_rect(bounds));
                    }
                });
            }
        })
        .children(menu_item_elements(
            branch_items,
            branch_states.clone(),
            debug_id.clone(),
            metrics,
            colors,
            runtime.clone(),
            window_overlay_runtime.clone(),
            root_binding.clone(),
            branch_bindings.clone(),
            on_select.clone(),
            theme,
        ));
    let submenu_layer = menu_submenu_layer(
        items,
        state,
        &branch_states,
        debug_id.clone(),
        runtime.clone(),
        window_overlay_runtime.clone(),
        root_binding.clone(),
        branch_bindings.clone(),
        on_select.clone(),
        cx,
        snap_margin,
        deferred_priority,
    );
    let surface_id =
        surface_id.unwrap_or_else(|| format!("menu:{debug_id}:panel:{branch_key}").into());
    let branch_semantics =
        SemanticDescriptor::new(state.content_role()).with_actions(&[AccessibleAction::Focus]);
    let shell = div()
        .id(surface_id)
        .debug_selector({
            let branch_debug_id = debug_id.clone();
            let branch_key = branch_key.clone();
            move || format!("menu:{branch_debug_id}:panel:{branch_key}")
        })
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .when(branch_scrollable_content, |this| {
            this.h(gpui_px_from_ui(metrics.max_height()))
        })
        .when(!branch_scrollable_content, |this| {
            this.max_h(gpui_px_from_ui(metrics.max_height()))
        })
        .p(gpui_px_from_ui(metrics.surface_padding()))
        .flex()
        .flex_col()
        .gap_1()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(theme.resolve(colors.border()))
        .bg(theme.resolve(colors.surface()))
        .shadow(gpui_elevation_shadow(
            ThemeResolver::overlay_surface_elevation(theme),
        ))
        .occlude()
        .overflow_hidden()
        .when_some(branch_binding.clone(), |this, binding| {
            this.focusable()
                .tab_stop(false)
                .track_focus(binding.surface_focus())
                .ui_semantics(&branch_semantics)
        })
        .when(!branch_path.is_empty(), |this| {
            let runtime = runtime.clone();
            this.on_hover(move |hovered, window, cx| {
                handle_menu_submenu_surface_hover(runtime.clone(), *hovered, window, cx);
            })
        })
        .child(
            ScrollArea::new(scroll_viewport_id, rows)
                .vertical()
                .preserve_scroll()
                .scroll_handle(&scroll_handle)
                .with_size(state.size()),
        );

    let shell = if let Some(branch_binding) = branch_binding {
        window_overlay_runtime
            .surface(
                &branch_binding,
                OverlayInsideRegionId::new("surface"),
                format!("menu:{debug_id}:branch:{branch_key}:surface-region"),
                shell,
            )
            .into_any_element()
    } else {
        shell.into_any_element()
    };

    div()
        .relative()
        .child(shell)
        .when_some(submenu_layer, |this, submenu_layer| {
            this.child(submenu_layer)
        })
        .into_any_element()
}

fn menu_submenu_layer(
    items: &[MenuItem],
    state: &MenuState,
    branch_states: &[MenuItemState],
    debug_id: String,
    runtime: open_gpui::Entity<MenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    cx: &mut App,
    snap_margin: open_gpui::Pixels,
    deferred_priority: usize,
) -> Option<AnyElement> {
    let open_child = branch_states
        .iter()
        .find(|item| item.submenu_open() && item.has_submenu())?;
    let child_branch_path = open_child.path().to_vec();
    let submenu_binding = branch_bindings
        .get(&menu_path_key(&child_branch_path))
        .cloned()?;
    let trigger_bounds = runtime
        .read(cx)
        .submenu_trigger_bounds
        .get(&open_child.path_key())
        .copied()?;
    let placement = GpuiOverlayPlacement::resolve(
        OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(trigger_bounds),
            ui_size(
                state.metrics().min_width(),
                state.metrics().trigger_height(),
            ),
        )
        .with_side(OverlayPlacementSide::Right)
        .with_alignment(OverlayPlacementAlignment::Start)
        .with_offset(UiPx::ZERO),
        snap_margin,
    );
    let submenu_adapter = GpuiOverlayState::resolve(
        state.overlay().policy().clone(),
        deferred_priority,
        snap_margin,
    );

    Some(gpui_positioned_overlay_layer(
        &submenu_adapter,
        &placement,
        placement.position().unwrap_or_default(),
        &submenu_binding,
        |opening_theme| {
            menu_branch_surface(
                items,
                state,
                &child_branch_path,
                None,
                debug_id,
                runtime,
                window_overlay_runtime,
                root_binding,
                branch_bindings,
                on_select,
                None,
                opening_theme,
                cx,
                snap_margin,
                deferred_priority,
            )
            .into_any_element()
        },
    ))
}

fn menu_item_elements(
    items: Vec<MenuItem>,
    states: Vec<MenuItemState>,
    debug_id: String,
    metrics: MenuMetrics,
    colors: MenuColors,
    runtime: open_gpui::Entity<MenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    theme: &ThemeContext,
) -> Vec<AnyElement> {
    items
        .into_iter()
        .zip(states)
        .map(|(item, item_state)| {
            menu_item_element(
                item,
                item_state,
                "menu",
                debug_id.clone(),
                metrics,
                colors,
                runtime.clone(),
                window_overlay_runtime.clone(),
                root_binding.clone(),
                branch_bindings.clone(),
                on_select.clone(),
                theme,
            )
        })
        .collect()
}

pub(crate) fn menu_branch_items_and_states(
    items: &[MenuItem],
    states: &[MenuItemState],
    branch_path: &[String],
) -> Option<(Vec<MenuItem>, Vec<MenuItemState>)> {
    if branch_path.is_empty() {
        return Some((items.to_vec(), states.to_vec()));
    }

    let branch_item = find_menu_item_by_path(items, branch_path)?;
    let branch_state = find_menu_item_state_by_path(states, branch_path)?;

    Some((
        branch_item.child_items().to_vec(),
        branch_state.children().to_vec(),
    ))
}

fn find_menu_item_by_path<'a>(items: &'a [MenuItem], path: &[String]) -> Option<&'a MenuItem> {
    let mut current = items;
    let mut resolved = None;

    for segment in path {
        let index = segment.split_once(':')?.0.parse::<usize>().ok()?;
        let item = current.get(index)?;
        resolved = Some(item);
        current = item.child_items();
    }

    resolved
}

fn menu_bounds_to_rect(bounds: open_gpui::Bounds<open_gpui::Pixels>) -> Rect {
    open_gpui_ui_core::rect(
        ui_point_from_gpui(bounds.origin),
        ui_size_from_gpui_size(bounds.size),
    )
}

/// Returns concrete menu items that are visible for the current submenu path.
pub(crate) fn visible_menu_items(items: &[MenuItem], open_path: &[String]) -> Vec<MenuItem> {
    let mut visible = Vec::new();
    let mut parent_path = Vec::new();
    flatten_visible_menu_items(items, &mut parent_path, open_path, &mut visible);
    visible
}

fn flatten_visible_menu_items(
    items: &[MenuItem],
    parent_path: &mut Vec<String>,
    open_path: &[String],
    visible: &mut Vec<MenuItem>,
) {
    for (index, item) in items.iter().enumerate() {
        parent_path.push(format!("{index}:{}", item.descriptor.value()));
        visible.push(item.clone());
        if item.descriptor.kind() == MenuItemKind::Submenu
            && !item.child_items().is_empty()
            && menu_path_is_open(parent_path, open_path)
        {
            flatten_visible_menu_items(item.child_items(), parent_path, open_path, visible);
        }
        parent_path.pop();
    }
}
