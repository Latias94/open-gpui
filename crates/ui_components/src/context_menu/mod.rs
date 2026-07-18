//! Context menu component.

mod model;

use crate::geometry::gpui_px_from_ui;
use crate::menu::runtime::ContextMenuRuntime;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    Pixels, Point, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use open_gpui_ui_core::{
    AccessibleAction, DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent,
    OutsidePressPolicy, Role, SemanticDescriptor, Sizable, Size, ThemeTokens,
};

use crate::a11y::UiA11yElementExt;
use crate::collection_typeahead::CollectionTypeaheadInput;
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::{gpui_point_from_ui, ui_point_from_gpui};
use crate::menu::{
    MenuBranchBindings, MenuItem, MenuItemKind, MenuItemState, MenuKeyboardIntent, MenuSelection,
    MenuSubmenuNavigation, menu_path_key, sync_menu_branch_bindings, visible_menu_items,
};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerRegistration,
    OverlayOpenIntent, OverlayOwnership, WindowOverlayRuntime, gpui_overlay_state,
    gpui_positioned_overlay_layer, resolve_overlay_open_state,
};
use crate::scroll_area::ScrollArea;
use crate::theme::{ThemeContext, ThemeResolver, gpui_elevation_shadow};

pub use model::ContextMenuState;

/// A concrete GPUI context menu component.
#[derive(IntoElement)]
pub struct ContextMenu {
    id: ElementId,
    label: SharedString,
    items: Vec<MenuItem>,
    size: Size,
    open: Option<bool>,
    default_open: bool,
    anchor_point: Point<Pixels>,
    focused_value: Option<String>,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
}

impl ContextMenu {
    /// Creates a context menu.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items: Vec::new(),
            size: Size::Medium,
            open: None,
            default_open: false,
            anchor_point: Point::default(),
            focused_value: None,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_select: None,
        }
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

    /// Applies the point where the context menu should open.
    pub fn anchor_point(mut self, point: Point<Pixels>) -> Self {
        self.anchor_point = point;
        self
    }

    /// Applies the default focused item value for adapter-owned runtime state.
    pub fn default_focused_value(mut self, value: impl Into<String>) -> Self {
        self.focused_value = Some(value.into());
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

    /// Returns resolved context-menu state.
    pub fn state(&self) -> ContextMenuState {
        ContextMenuState::resolve(
            self.size,
            self.open,
            self.default_open,
            ui_point_from_gpui(self.anchor_point),
            self.focused_value.as_deref(),
            self.items.iter().map(MenuItem::descriptor),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for ContextMenu {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            ContextMenuRuntime::new(
                self.default_open,
                self.anchor_point,
                self.focused_value.clone(),
            )
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();
        let resolved_anchor = if resolved_open {
            runtime_state.anchor_point
        } else {
            self.anchor_point
        };

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.sync_controlled_open(resolved_open);
            });
        }

        let focused_value = runtime_state.resolved_focused_value(self.focused_value.as_deref());
        let state = ContextMenuState::resolve_with_paths(
            self.size,
            Some(resolved_open),
            self.default_open,
            ui_point_from_gpui(resolved_anchor),
            focused_value,
            runtime_state.focused_path.as_deref(),
            &runtime_state.open_path,
            self.items.iter().map(MenuItem::descriptor),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let id = self.id;
        let debug_id = id.to_string();
        let surface_id: ElementId = (id.clone(), "surface").into();
        let hotspot_id: ElementId = (id.clone(), "hotspot").into();
        let items = self.items;
        let label = self.label;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("context-menu:{debug_id}"),
            state.overlay().policy().clone(),
            ownership,
        );
        if let Some(on_open_change) = on_open_change.clone() {
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
            .expect("context menu root overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(root_binding.clone());
            });
        }
        let branch_bindings = sync_menu_branch_bindings(
            "context-menu",
            &debug_id,
            state.menu(),
            &runtime,
            &window_overlay_runtime,
            &root_binding,
            window,
            cx,
        );
        let runtime_state = runtime.read(cx).clone();
        let scroll_handle = runtime_state.scroll_handle.clone();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement =
            GpuiOverlayPlacement::resolve(state.placement_input(), overlay_adapter.snap_margin());
        let open_runtime = runtime.clone();
        let open_window_overlay_runtime = window_overlay_runtime.clone();
        let open_root_binding = root_binding.clone();
        let hotspot_focus_shadow = focus_ring_shadow_with_theme(state.menu().focus_ring(), &theme);
        let hotspot_semantics = SemanticDescriptor::new(Role::Button)
            .with_label(label.as_ref())
            .with_actions(&[AccessibleAction::Focus]);

        let source = div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("context-menu:{debug_id}:root")
            })
            .relative()
            .min_h(px(80.0))
            .min_w(px(220.0))
            .rounded(gpui_px_from_ui(state.metrics().radius()))
            .border_1()
            .border_color(theme.resolve(state.colors().border()))
            .bg(theme.resolve(state.colors().item_background()))
            .p_3()
            .cursor_context_menu()
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                let anchor_point = event.position;
                open_runtime.update(cx, |runtime, cx| {
                    runtime.prepare_open_at(anchor_point);
                    cx.notify();
                });
                open_window_overlay_runtime
                    .request_open_change(
                        &open_root_binding,
                        true,
                        DismissReason::Trigger,
                        window,
                        cx,
                    )
                    .expect("context menu source should own its root overlay registration");
            })
            .child(
                div()
                    .id(hotspot_id)
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("context-menu:{debug_id}:hotspot")
                    })
                    .ui_semantics(&hotspot_semantics)
                    .focusable()
                    .track_focus(root_binding.trigger_focus())
                    .tab_stop(true)
                    .focus_visible(move |style| style.shadow(hotspot_focus_shadow.clone()))
                    .child(label),
            )
            .when(state.open(), |this| {
                this.child(gpui_positioned_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    gpui_point_from_ui(state.anchor_point()),
                    &root_binding,
                    |opening_theme| {
                        context_menu_surface(
                            items,
                            surface_id.clone(),
                            debug_id.clone(),
                            state.clone(),
                            runtime.clone(),
                            window_overlay_runtime.clone(),
                            root_binding.clone(),
                            branch_bindings.clone(),
                            scroll_handle.clone(),
                            on_select.clone(),
                            opening_theme,
                        )
                        .into_any_element()
                    },
                ))
            });

        window_overlay_runtime.inside_region_for_button(
            &root_binding,
            OverlayInsideRegionId::new("source"),
            MouseButton::Right,
            format!("context-menu:{debug_id}:source-region"),
            source,
        )
    }
}

fn context_menu_surface(
    items: Vec<MenuItem>,
    surface_id: ElementId,
    debug_id: String,
    state: ContextMenuState,
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    scroll_handle: ScrollHandle,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let key_state = state.menu().clone();
    let key_runtime = runtime.clone();
    let key_select = on_select.clone();
    let key_window_overlay_runtime = window_overlay_runtime.clone();
    let key_root_binding = root_binding.clone();
    let surface_debug_id = debug_id.clone();
    let scroll_viewport_id = format!("context-menu:{debug_id}:surface-scroll");
    let scrollable_content = state.menu().scrollable_content();
    let key_items = visible_menu_items(&items, state.menu().open_path());
    let rows = div()
        .flex()
        .flex_col()
        .gap_1()
        .children(context_menu_branch_elements(
            &items,
            state.menu().items(),
            &state,
            debug_id.clone(),
            runtime.clone(),
            window_overlay_runtime.clone(),
            root_binding.clone(),
            branch_bindings.clone(),
            on_select.clone(),
            theme,
        ));
    let surface_semantics =
        SemanticDescriptor::new(state.content_role()).with_actions(&[AccessibleAction::Focus]);

    let surface = div()
        .id(surface_id)
        .debug_selector(move || format!("context-menu:{surface_debug_id}:surface"))
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .when(scrollable_content, |this| {
            this.h(gpui_px_from_ui(metrics.max_height()))
        })
        .when(!scrollable_content, |this| {
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
        .text_color(theme.resolve(colors.foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow(gpui_elevation_shadow(
            ThemeResolver::overlay_surface_elevation(theme),
        ))
        .occlude()
        .tab_group()
        .focusable()
        .track_focus(root_binding.surface_focus())
        .ui_semantics(&surface_semantics)
        .on_key_down({
            move |event: &KeyDownEvent, window, cx| {
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
                                .expect(
                                    "context menu keyboard selection should own the root registration",
                                );
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
                        runtime.focus_item(
                            target.path().to_vec(),
                            target.value().to_owned(),
                            cx,
                        );
                    });
                }
            }
        })
        .overflow_hidden()
        .child(
            ScrollArea::new(scroll_viewport_id, rows)
                .vertical()
                .preserve_scroll()
                .scroll_handle(&scroll_handle)
                .with_size(state.size()),
        );

    window_overlay_runtime.surface(
        &root_binding,
        OverlayInsideRegionId::new("root-surface"),
        format!("context-menu:{debug_id}:root-surface-region"),
        surface,
    )
}

fn context_menu_branch_elements(
    items: &[MenuItem],
    states: &[MenuItemState],
    state: &ContextMenuState,
    debug_id: String,
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    window_overlay_runtime: WindowOverlayRuntime,
    root_binding: OverlayLayerBinding,
    branch_bindings: MenuBranchBindings,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    theme: &ThemeContext,
) -> Vec<AnyElement> {
    let metrics = state.metrics();
    let colors = state.colors();
    let mut elements = Vec::new();

    for (item, item_state) in items.iter().cloned().zip(states.iter().cloned()) {
        let child_branch_open = item_state.has_submenu() && item_state.submenu_open();
        let child_branch_path = item_state.path().to_vec();
        let child_items = item.child_items().to_vec();
        let child_states = item_state.children().to_vec();
        let element = match item_state.kind() {
            MenuItemKind::Header => {
                let semantics = SemanticDescriptor::new(Role::Label).with_label(item_state.label());
                div()
                    .id(format!("context-menu-header:{}", item_state.value()))
                    .debug_selector({
                        let header_debug_id = debug_id.clone();
                        let header_value = item_state.value().to_owned();
                        move || format!("context-menu:{header_debug_id}:header:{header_value}")
                    })
                    .pl(gpui_px_from_ui(
                        metrics.item_padding_x()
                            + metrics.submenu_indent() * item_state.depth() as f32,
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
                    .id(format!("context-menu-separator:{}", item_state.index()))
                    .debug_selector({
                        let separator_debug_id = debug_id.clone();
                        let separator_index = item_state.index();
                        move || {
                            format!("context-menu:{separator_debug_id}:separator:{separator_index}")
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
                let item_handler = item.select_handler();
                let global_handler = on_select.clone();
                let click_runtime = runtime.clone();
                let click_window_overlay_runtime = window_overlay_runtime.clone();
                let click_root_binding = root_binding.clone();
                let item_value = item_state.value().to_owned();
                let item_path = item_state.path().to_vec();
                let item_label = item_state.label().to_owned();
                let shortcut = item_state.shortcut().map(str::to_owned);
                let item_path_key = item_state.path_key();
                let child_binding = branch_bindings.get(&item_path_key).cloned();
                let left_padding =
                    metrics.item_padding_x() + metrics.submenu_indent() * item_state.depth() as f32;
                let focused = item_state.focused();
                let disabled = item_state.disabled();
                let focusable = item_state.focusable();
                let toggled = item_state.toggled();
                let has_submenu = item_state.has_submenu();
                let submenu_open = item_state.submenu_open();
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
                let item_actions: &[AccessibleAction] = if child_binding.is_some() && focusable {
                    &[AccessibleAction::Click, AccessibleAction::Focus]
                } else {
                    &[AccessibleAction::Click]
                };
                let mut semantics = SemanticDescriptor::new(Role::MenuItem)
                    .with_label(&item_label)
                    .with_disabled(disabled)
                    .with_actions(item_actions);
                if let Some(toggled) = toggled {
                    semantics = semantics.with_toggled(toggled);
                }
                if has_submenu {
                    semantics = semantics.with_expanded(submenu_open);
                }
                let element = div()
                    .id(format!("context-menu-item:{item_path_key}"))
                    .debug_selector({
                        let item_debug_id = debug_id.clone();
                        let item_path_key = item_path_key.clone();
                        move || format!("context-menu:{item_debug_id}:item:{item_path_key}")
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
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                if let Some(submenu_navigation) = submenu_navigation.clone() {
                                    click_runtime.update(cx, |runtime, cx| {
                                        runtime.apply_submenu_target(&submenu_navigation, cx);
                                    });
                                    return;
                                }
                                let Some(selection) = selection.clone() else {
                                    return;
                                };
                                click_runtime.update(cx, |runtime, cx| {
                                    runtime.focus_item(item_path.clone(), item_value.clone(), cx);
                                });
                                click_window_overlay_runtime
                                    .request_open_change_with_effect(
                                        &click_root_binding,
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
                                    .expect(
                                        "context menu selection should own the root registration",
                                    );
                            })
                    })
                    .child(div().flex_1().child(item_label))
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
                        let marker = if toggled == open_gpui_ui_core::Toggled::True {
                            "checked"
                        } else {
                            ""
                        };
                        this.child(div().ml_2().child(marker))
                    })
                    .when(has_submenu, |this| this.child(div().ml_2().child(">")))
                    .into_any_element();

                if let Some(child_binding) = child_binding {
                    window_overlay_runtime
                        .inside_region(
                            &child_binding,
                            OverlayInsideRegionId::new("trigger"),
                            format!(
                                "context-menu:{debug_id}:branch:{item_path_key}:trigger-region"
                            ),
                            element,
                        )
                        .into_any_element()
                } else {
                    element
                }
            }
        };
        elements.push(element);

        if child_branch_open
            && let Some(branch_binding) = branch_bindings
                .get(&menu_path_key(&child_branch_path))
                .cloned()
        {
            let branch_key = menu_path_key(&child_branch_path);
            let branch_semantics = SemanticDescriptor::new(state.content_role())
                .with_actions(&[AccessibleAction::Focus]);
            let branch_rows =
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(context_menu_branch_elements(
                        &child_items,
                        &child_states,
                        state,
                        debug_id.clone(),
                        runtime.clone(),
                        window_overlay_runtime.clone(),
                        root_binding.clone(),
                        branch_bindings.clone(),
                        on_select.clone(),
                        theme,
                    ));
            let panel = div()
                .id(format!("context-menu-panel:{branch_key}"))
                .debug_selector({
                    let branch_debug_id = debug_id.clone();
                    let branch_key = branch_key.clone();
                    move || format!("context-menu:{branch_debug_id}:panel:{branch_key}")
                })
                .w_full()
                .focusable()
                .tab_stop(false)
                .track_focus(branch_binding.surface_focus())
                .ui_semantics(&branch_semantics)
                .child(branch_rows);
            elements.push(
                window_overlay_runtime
                    .surface(
                        &branch_binding,
                        OverlayInsideRegionId::new("surface"),
                        format!("context-menu:{debug_id}:branch:{branch_key}:surface-region"),
                        panel,
                    )
                    .into_any_element(),
            );
        }
    }

    elements
}
