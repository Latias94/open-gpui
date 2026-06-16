//! Context menu component.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels,
    Point, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Role, Sizable, Size, ThemeTokens,
};

use crate::focus::focus_ring_shadow;
use crate::menu::{
    MenuColors, MenuItem, MenuItemDescriptor, MenuItemKind, MenuMetrics, MenuOpenMode,
    MenuSelection, MenuState,
};
use crate::overlay::{
    GpuiOverlayPlacement, GpuiOverlayState, outside_press_open_change, point_anchor_placement,
};
use crate::theme::ThemeResolver;

/// Resolved context-menu state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuState {
    size: Size,
    open: bool,
    default_open: bool,
    open_mode: MenuOpenMode,
    anchor_point: Point<Pixels>,
    menu: MenuState,
    placement: GpuiOverlayPlacement,
}

impl ContextMenuState {
    /// Resolves public state for a point-anchored context menu.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        open: Option<bool>,
        default_open: bool,
        anchor_point: Point<Pixels>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = MenuItemDescriptor>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open_mode = if open.is_some() {
            MenuOpenMode::Controlled
        } else {
            MenuOpenMode::Uncontrolled
        };
        let descriptors: Vec<MenuItemDescriptor> = items.into_iter().collect();
        let menu = MenuState::resolve(
            size,
            false,
            open,
            default_open,
            focused_value,
            descriptors,
            OverlayPlacementSide::Bottom,
            OverlayPlacementAlignment::Start,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        );
        let placement = GpuiOverlayPlacement::resolve(
            point_anchor_placement(
                anchor_point,
                open_gpui_ui_core::OverlaySize::new(
                    menu.metrics().min_width(),
                    menu.metrics().item_height(),
                ),
            )
            .with_side(OverlayPlacementSide::Bottom)
            .with_alignment(OverlayPlacementAlignment::Start)
            .with_offset(px(0.0)),
            menu.overlay().snap_margin(),
        );

        Self {
            size,
            open: menu.open(),
            default_open,
            open_mode,
            anchor_point,
            menu,
            placement,
        }
    }

    /// Returns context-menu size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether context-menu content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> MenuOpenMode {
        self.open_mode
    }

    /// Returns the point anchor.
    pub const fn anchor_point(&self) -> Point<Pixels> {
        self.anchor_point
    }

    /// Returns the shared menu state.
    pub const fn menu(&self) -> &MenuState {
        &self.menu
    }

    /// Returns resolved GPUI placement.
    pub const fn placement(&self) -> GpuiOverlayPlacement {
        self.placement
    }

    /// Returns resolved overlay adapter state.
    pub const fn overlay(&self) -> &GpuiOverlayState {
        self.menu.overlay()
    }

    /// Returns resolved menu metrics.
    pub const fn metrics(&self) -> MenuMetrics {
        self.menu.metrics()
    }

    /// Returns resolved menu colors.
    pub const fn colors(&self) -> MenuColors {
        self.menu.colors()
    }

    /// Returns content accessibility role.
    pub const fn content_role(&self) -> Role {
        Role::Menu
    }
}

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
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
}

#[derive(Debug, Clone)]
struct ContextMenuRuntime {
    open: bool,
    anchor_point: Point<Pixels>,
    focused_value: Option<String>,
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
            on_escape_close: None,
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

    /// Applies the initially focused item value.
    pub fn focused_value(mut self, value: impl Into<String>) -> Self {
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

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        let handler = Rc::new(handler);
        self.on_escape_close = Some(handler.clone());
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
            self.anchor_point,
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
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| ContextMenuRuntime {
            open: self.default_open,
            anchor_point: self.anchor_point,
            focused_value: self.focused_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let controlled_open = self.open;
        let resolved_open = controlled_open.unwrap_or(runtime_state.open);
        let resolved_anchor = if resolved_open {
            runtime_state.anchor_point
        } else {
            self.anchor_point
        };

        if controlled_open.is_some() && runtime_state.open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let focused_value = self
            .focused_value
            .as_deref()
            .or(runtime_state.focused_value.as_deref());
        let state = ContextMenuState::resolve(
            self.size,
            Some(resolved_open),
            self.default_open,
            resolved_anchor,
            focused_value,
            self.items.iter().map(MenuItem::descriptor),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let id = self.id;
        let surface_id: ElementId = (id.clone(), "surface").into();
        let hotspot_id: ElementId = (id.clone(), "hotspot").into();
        let items = self.items;
        let label = self.label;
        let on_escape_close = self.on_escape_close;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let placement = state.placement();
        let first_focusable_value = first_focusable_value(state.menu());
        let open_runtime = runtime.clone();
        let open_change = on_open_change.clone();

        div()
            .id(id)
            .relative()
            .min_h(px(80.0))
            .min_w(px(220.0))
            .rounded(state.metrics().radius())
            .border_1()
            .border_color(ThemeResolver::resolve(state.colors().border()))
            .bg(ThemeResolver::resolve(state.colors().item_background()))
            .p_3()
            .cursor_context_menu()
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                open_runtime.update(cx, |runtime, _| {
                    runtime.open = true;
                    runtime.anchor_point = event.position;
                    runtime.focused_value = first_focusable_value.clone();
                });
                if let Some(on_open_change) = open_change.as_ref() {
                    on_open_change(true, window, cx);
                }
            })
            .child(
                div()
                    .id(hotspot_id)
                    .role(Role::Button)
                    .aria_label(label.clone())
                    .focusable()
                    .tab_stop(true)
                    .focus_visible({
                        let focus_ring = state.menu().focus_ring();
                        move |style| style.shadow(focus_ring_shadow(focus_ring))
                    })
                    .child(label),
            )
            .when(state.open(), |this| {
                this.child(
                    deferred(
                        anchored()
                            .position(placement.position().unwrap_or(state.anchor_point()))
                            .snap_to_window_with_margin(placement.snap_margin())
                            .child(context_menu_surface(
                                items,
                                surface_id.clone(),
                                state.clone(),
                                runtime.clone(),
                                on_escape_close.clone(),
                                on_open_change.clone(),
                                on_select.clone(),
                            )),
                    )
                    .priority(state.overlay().deferred_priority()),
                )
            })
    }
}

fn context_menu_surface(
    items: Vec<MenuItem>,
    surface_id: ElementId,
    state: ContextMenuState,
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let key_state = state.menu().clone();
    let key_runtime = runtime.clone();
    let key_open_change = on_open_change.clone();
    let key_select = on_select.clone();
    let escape_runtime = runtime.clone();
    let escape_open_change = on_escape_close.clone();

    div()
        .id(surface_id)
        .min_w(metrics.min_width())
        .max_w(metrics.max_width())
        .p(metrics.surface_padding())
        .flex()
        .flex_col()
        .gap_1()
        .rounded(metrics.radius())
        .border_1()
        .border_color(ThemeResolver::resolve(colors.border()))
        .bg(ThemeResolver::resolve(colors.surface()))
        .text_color(ThemeResolver::resolve(colors.foreground()))
        .text_size(metrics.text_size())
        .line_height(metrics.text_size())
        .shadow_lg()
        .occlude()
        .tab_group()
        .focusable()
        .role(state.content_role())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            if key == "escape" {
                cx.stop_propagation();
                window.prevent_default();
                close_context_menu(
                    escape_runtime.clone(),
                    escape_open_change.clone(),
                    window,
                    cx,
                );
                return;
            }

            if let Some(target) = key_state.navigation_target(key) {
                cx.stop_propagation();
                window.prevent_default();
                let value = target.value().to_owned();
                key_runtime.update(cx, |runtime, _| {
                    runtime.focused_value = Some(value);
                });
                return;
            }

            if let Some(selection) = key_state.activation_for_key(key) {
                cx.stop_propagation();
                window.prevent_default();
                if let Some(on_select) = key_select.as_ref() {
                    on_select(selection, window, cx);
                }
                close_context_menu(key_runtime.clone(), key_open_change.clone(), window, cx);
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_context_menu(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .children(context_menu_item_elements(
            items,
            state,
            runtime,
            on_open_change,
            on_select,
        ))
}

fn context_menu_item_elements(
    items: Vec<MenuItem>,
    state: ContextMenuState,
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> Vec<open_gpui::AnyElement> {
    let metrics = state.metrics();
    let colors = state.colors();
    let states = state.menu().items().to_vec();

    items
        .into_iter()
        .zip(states)
        .map(|(item, item_state)| match item_state.kind() {
            MenuItemKind::Separator => div()
                .id(format!("context-menu-separator:{}", item_state.index()))
                .h(metrics.separator_height())
                .my_1()
                .bg(ThemeResolver::resolve(colors.separator()))
                .into_any_element(),
            MenuItemKind::Action => {
                let selection = MenuSelection::from_item(&item_state);
                let item_handler = item.select_handler();
                let global_handler = on_select.clone();
                let runtime = runtime.clone();
                let on_open_change = on_open_change.clone();
                let focused = item_state.focused();
                let disabled = item_state.disabled();
                div()
                    .id(format!("context-menu-item:{}", item_state.value()))
                    .min_h(metrics.item_height())
                    .px(metrics.item_padding_x())
                    .py(metrics.item_padding_y())
                    .flex()
                    .items_center()
                    .rounded(metrics.radius())
                    .bg(ThemeResolver::resolve(if focused {
                        colors.item_focus_background()
                    } else {
                        colors.item_background()
                    }))
                    .text_color(ThemeResolver::resolve(if disabled {
                        colors.item_disabled_foreground()
                    } else {
                        colors.foreground()
                    }))
                    .role(Role::MenuItem)
                    .aria_label(item_state.label().to_owned())
                    .aria_disabled(disabled)
                    .focusable()
                    .tab_stop(item_state.tab_stop())
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.item_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let Some(selection) = selection.clone() else {
                                    return;
                                };
                                if let Some(item_handler) = item_handler.as_ref() {
                                    item_handler(selection.clone(), window, cx);
                                }
                                if let Some(global_handler) = global_handler.as_ref() {
                                    global_handler(selection, window, cx);
                                }
                                close_context_menu(
                                    runtime.clone(),
                                    on_open_change.clone(),
                                    window,
                                    cx,
                                );
                            })
                    })
                    .child(item_state.label().to_owned())
                    .into_any_element()
            }
        })
        .collect()
}

fn close_context_menu(
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.open = false;
        runtime.focused_value = None;
    });
    if let Some(on_open_change) = on_open_change.as_ref() {
        on_open_change(false, window, cx);
    }
}

fn first_focusable_value(state: &MenuState) -> Option<String> {
    state
        .items()
        .iter()
        .find(|item| item.focusable())
        .map(|item| item.value().to_owned())
}
