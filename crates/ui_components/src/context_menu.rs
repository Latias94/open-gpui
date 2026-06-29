//! Context menu component.

use crate::geometry::gpui_px_from_ui;
use crate::menu_runtime::ContextMenuRuntime;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, FocusHandle, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    Pixels, Point, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, anchored, deferred, div, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    Role, Sizable, Size, ThemeTokens, UiPoint, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::focus::focus_ring_shadow;
use crate::geometry::{gpui_point_from_ui, ui_point_from_gpui};
use crate::menu::{
    MenuColors, MenuItem, MenuItemDescriptor, MenuItemKind, MenuMetrics, MenuOpenMode,
    MenuSelection, MenuState, MenuSubmenuNavigation, visible_menu_items,
};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayResolvedState, focus_restore_requests_trigger, gpui_overlay_state,
    outside_press_open_change,
};
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeResolver;

/// Resolved context-menu state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuState {
    size: Size,
    open: bool,
    default_open: bool,
    open_mode: MenuOpenMode,
    anchor_point: UiPoint,
    menu: MenuState,
    placement_input: OverlayPlacementInput,
}

impl ContextMenuState {
    /// Resolves public state for a point-anchored context menu.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        open: Option<bool>,
        default_open: bool,
        anchor_point: UiPoint,
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
        let placement_input = context_menu_placement_input(anchor_point, &menu);

        Self {
            size,
            open: menu.open(),
            default_open,
            open_mode,
            anchor_point,
            menu,
            placement_input,
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
    pub const fn anchor_point(&self) -> UiPoint {
        self.anchor_point
    }

    /// Returns the shared menu state.
    pub const fn menu(&self) -> &MenuState {
        &self.menu
    }

    /// Returns renderer-neutral placement input for the context-menu surface.
    pub const fn placement_input(&self) -> OverlayPlacementInput {
        self.placement_input
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
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

fn context_menu_placement_input(anchor_point: UiPoint, menu: &MenuState) -> OverlayPlacementInput {
    let metrics = menu.metrics();
    let visible_count = menu.visible_items().len();
    let row_gap = ui_px(4.0);
    let gap_height = row_gap.as_f32() * visible_count.saturating_sub(1) as f32;
    let content_height = ui_px(metrics.max_height().as_f32().min(
        metrics.surface_padding().as_f32() * 2.0
            + metrics.item_height().as_f32() * visible_count as f32
            + gap_height,
    ));

    OverlayPlacementInput::new(
        OverlayAnchorInput::from_point(anchor_point),
        ui_size(metrics.min_width(), content_height),
    )
    .with_side(OverlayPlacementSide::Bottom)
    .with_alignment(OverlayPlacementAlignment::Start)
    .with_offset(ui_px(0.0))
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
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
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

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
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
        let content_focus = cx.focus_handle();
        let trigger_focus = cx.focus_handle();
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            ContextMenuRuntime::new(
                self.default_open,
                self.anchor_point,
                content_focus.clone(),
                trigger_focus.clone(),
                self.focused_value.clone(),
            )
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
                runtime.sync_controlled_open(resolved_open);
            });
        }

        let focused_value = runtime_state.resolved_focused_value(self.focused_value.as_deref());
        let mut state = ContextMenuState::resolve(
            self.size,
            Some(resolved_open),
            self.default_open,
            ui_point_from_gpui(resolved_anchor),
            focused_value,
            self.items.iter().map(MenuItem::descriptor),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        state.menu = MenuState::resolve_with_paths(
            self.size,
            false,
            Some(resolved_open),
            self.default_open,
            focused_value,
            runtime_state.focused_path.as_deref(),
            &runtime_state.open_path,
            self.items.iter().map(MenuItem::descriptor),
            OverlayPlacementSide::Bottom,
            OverlayPlacementAlignment::Start,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        state.placement_input = context_menu_placement_input(state.anchor_point, &state.menu);
        let id = self.id;
        let debug_id = id.to_string();
        let surface_id: ElementId = (id.clone(), "surface").into();
        let hotspot_id: ElementId = (id.clone(), "hotspot").into();
        let items = self.items;
        let label = self.label;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let focus_restore = state.menu().focus_restore_intent().clone();
        let initial_focus = state.menu().initial_focus_intent().clone();
        let runtime_state = runtime.read(cx).clone();
        let trigger_focus = runtime_state.trigger_focus.clone();
        let content_focus = runtime_state.content_focus.clone();
        let scroll_handle = runtime_state.scroll_handle.clone();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement =
            GpuiOverlayPlacement::resolve(state.placement_input(), overlay_adapter.snap_margin());
        let open_runtime = runtime.clone();
        let open_change = on_open_change.clone();

        if state.open() && !runtime_state.did_initial_focus {
            runtime.update(cx, |runtime, _| {
                runtime.did_initial_focus = true;
            });
            if let Some(focus) = context_menu_initial_focus_handle(&runtime, &initial_focus, cx) {
                window.defer(cx, move |window, cx| focus.focus(window, cx));
            }
        }

        div()
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
            .border_color(ThemeResolver::resolve(state.colors().border()))
            .bg(ThemeResolver::resolve(state.colors().item_background()))
            .p_3()
            .cursor_context_menu()
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                open_runtime.update(cx, |runtime, _| {
                    runtime.open_at(event.position);
                });
                if let Some(on_open_change) = open_change.as_ref() {
                    on_open_change(true, window, cx);
                }
            })
            .child(
                div()
                    .id(hotspot_id)
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("context-menu:{debug_id}:hotspot")
                    })
                    .ui_role(Role::Button)
                    .aria_label(label.clone())
                    .focusable()
                    .track_focus(&trigger_focus)
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
                            .position(
                                placement
                                    .position()
                                    .unwrap_or(gpui_point_from_ui(state.anchor_point())),
                            )
                            .snap_to_window_with_margin(placement.snap_margin())
                            .child(context_menu_surface(
                                items,
                                surface_id.clone(),
                                debug_id.clone(),
                                state.clone(),
                                runtime.clone(),
                                trigger_focus.clone(),
                                content_focus.clone(),
                                scroll_handle.clone(),
                                focus_restore.clone(),
                                on_open_change.clone(),
                                on_select.clone(),
                            )),
                    )
                    .priority(overlay_adapter.deferred_priority()),
                )
            })
    }
}

fn context_menu_surface(
    items: Vec<MenuItem>,
    surface_id: ElementId,
    debug_id: String,
    state: ContextMenuState,
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    trigger_focus: FocusHandle,
    content_focus: FocusHandle,
    scroll_handle: ScrollHandle,
    focus_restore: FocusRestoreIntent,
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
    let surface_debug_id = debug_id.clone();
    let scroll_viewport_id = format!("context-menu:{debug_id}:surface-scroll");
    let scrollable_content = state.menu().scrollable_content();
    let key_items = visible_menu_items(&items, state.menu().open_path());
    let rows = div()
        .flex()
        .flex_col()
        .gap_1()
        .children(context_menu_item_elements(
            items,
            state.clone(),
            debug_id.clone(),
            runtime.clone(),
            trigger_focus.clone(),
            focus_restore.clone(),
            on_open_change.clone(),
            on_select.clone(),
        ));

    div()
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
        .border_color(ThemeResolver::resolve(colors.border()))
        .bg(ThemeResolver::resolve(colors.surface()))
        .text_color(ThemeResolver::resolve(colors.foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow_lg()
        .occlude()
        .tab_group()
        .focusable()
        .track_focus(&content_focus)
        .ui_role(state.content_role())
        .on_key_down({
            let trigger_focus = trigger_focus.clone();
            let focus_restore = focus_restore.clone();
            move |event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "escape" {
                    cx.stop_propagation();
                    window.prevent_default();
                    if let Some(target) = key_state.close_submenu_target() {
                        key_runtime.update(cx, |runtime, _| {
                            runtime.apply_submenu_target(&target);
                        });
                        return;
                    }
                    close_context_menu(
                        key_runtime.clone(),
                        trigger_focus.clone(),
                        focus_restore.clone(),
                        key_open_change.clone(),
                        window,
                        cx,
                    );
                    return;
                }

                if let Some(target) = key_state.submenu_navigation_target(key) {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.apply_submenu_target(&target);
                    });
                    return;
                }

                if let Some(target) = key_state.navigation_target(key) {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.focus_item(target.path().to_vec(), target.value().to_owned());
                    });
                    return;
                }

                if let Some(selection) = key_state.activation_for_key(key) {
                    cx.stop_propagation();
                    window.prevent_default();
                    if let Some(item_handler) = key_items
                        .iter()
                        .zip(key_state.visible_items())
                        .find(|(_, item_state)| item_state.path() == selection.path())
                        .and_then(|(item, _)| item.select_handler())
                        .as_ref()
                    {
                        item_handler(selection.clone(), window, cx);
                    }
                    if let Some(on_select) = key_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                    close_context_menu(
                        key_runtime.clone(),
                        trigger_focus.clone(),
                        focus_restore.clone(),
                        key_open_change.clone(),
                        window,
                        cx,
                    );
                }
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            let trigger_focus = trigger_focus.clone();
            let focus_restore = focus_restore.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_context_menu(
                    runtime.clone(),
                    trigger_focus.clone(),
                    focus_restore.clone(),
                    on_open_change.clone(),
                    window,
                    cx,
                );
            })
        })
        .overflow_hidden()
        .child(
            ScrollArea::new(scroll_viewport_id, rows)
                .vertical()
                .preserve_scroll()
                .scroll_handle(&scroll_handle)
                .with_size(state.size()),
        )
}

fn context_menu_item_elements(
    items: Vec<MenuItem>,
    state: ContextMenuState,
    debug_id: String,
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> Vec<open_gpui::AnyElement> {
    let metrics = state.metrics();
    let colors = state.colors();
    let states = state.menu().visible_items().to_vec();
    let items = visible_menu_items(&items, state.menu().open_path());

    items
        .into_iter()
        .zip(states)
        .map(|(item, item_state)| match item_state.kind() {
            MenuItemKind::Separator => div()
                .id(format!("context-menu-separator:{}", item_state.index()))
                .debug_selector({
                    let separator_debug_id = debug_id.clone();
                    let separator_index = item_state.index();
                    move || format!("context-menu:{separator_debug_id}:separator:{separator_index}")
                })
                .h(gpui_px_from_ui(metrics.separator_height()))
                .my_1()
                .bg(ThemeResolver::resolve(colors.separator()))
                .into_any_element(),
            MenuItemKind::Action
            | MenuItemKind::Checkbox
            | MenuItemKind::Radio
            | MenuItemKind::Submenu => {
                let selection = MenuSelection::from_item(&item_state);
                let item_handler = item.select_handler();
                let global_handler = on_select.clone();
                let runtime = runtime.clone();
                let on_open_change = on_open_change.clone();
                let trigger_focus = trigger_focus.clone();
                let focus_restore = focus_restore.clone();
                let item_value = item_state.value().to_owned();
                let item_label = item_state.label().to_owned();
                let item_path_key = item_state.path_key();
                let left_padding =
                    metrics.item_padding_x() + metrics.submenu_indent() * item_state.depth() as f32;
                let focused = item_state.focused();
                let disabled = item_state.disabled();
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
                div()
                    .id(format!("context-menu-item:{item_path_key}"))
                    .debug_selector({
                        let item_debug_id = debug_id.clone();
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
                    .ui_role(Role::MenuItem)
                    .aria_label(item_label.clone())
                    .aria_disabled(disabled)
                    .when_some(toggled, |this, toggled| this.ui_aria_toggled(toggled))
                    .when(has_submenu, |this| {
                        this.aria_expanded(item_state.submenu_open())
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.item_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                if let Some(submenu_navigation) = submenu_navigation.clone() {
                                    runtime.update(cx, |runtime, _| {
                                        runtime.apply_submenu_target(&submenu_navigation);
                                    });
                                    return;
                                }
                                let Some(selection) = selection.clone() else {
                                    return;
                                };
                                if let Some(item_handler) = item_handler.as_ref() {
                                    item_handler(selection.clone(), window, cx);
                                }
                                if let Some(global_handler) = global_handler.as_ref() {
                                    global_handler(selection, window, cx);
                                }
                                runtime.update(cx, |runtime, _| {
                                    runtime
                                        .focus_item(item_state.path().to_vec(), item_value.clone());
                                });
                                close_context_menu(
                                    runtime.clone(),
                                    trigger_focus.clone(),
                                    focus_restore.clone(),
                                    on_open_change.clone(),
                                    window,
                                    cx,
                                );
                            })
                    })
                    .child(item_label)
                    .when_some(toggled, |this, toggled| {
                        let marker = if toggled == open_gpui_ui_core::Toggled::True {
                            "checked"
                        } else {
                            ""
                        };
                        this.child(div().ml_2().child(marker))
                    })
                    .when(has_submenu, |this| this.child(div().ml_2().child(">")))
                    .into_any_element()
            }
        })
        .collect()
}

fn close_context_menu(
    runtime: open_gpui::Entity<ContextMenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.open = false;
        runtime.reset_closed_state();
    });
    if let Some(on_open_change) = on_open_change.as_ref() {
        on_open_change(false, window, cx);
    }
    if focus_restore_requests_trigger(&focus_restore) {
        window.defer(cx, move |window, cx| trigger_focus.focus(window, cx));
    }
}

fn context_menu_initial_focus_handle(
    runtime: &open_gpui::Entity<ContextMenuRuntime>,
    intent: &InitialFocusIntent,
    cx: &App,
) -> Option<FocusHandle> {
    match intent {
        InitialFocusIntent::None => None,
        InitialFocusIntent::FirstFocusable => Some(runtime.read(cx).content_focus.clone()),
        InitialFocusIntent::Target(_) => None,
        InitialFocusIntent::TargetOrFirstFocusable(_) => {
            Some(runtime.read(cx).content_focus.clone())
        }
    }
}
