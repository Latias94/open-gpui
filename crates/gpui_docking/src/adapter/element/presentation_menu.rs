//! Scoped product menus retain their original workspace and surface witnesses.

use super::*;

#[derive(Clone, Copy)]
enum MenuKey {
    Next,
    Previous,
    First,
    Last,
    Confirm,
}

/// The open run owns its selection, focus claim and pending keys. Commands
/// are resolved from the exact presented menu.
pub(super) struct MenuState {
    open: OpenMenu,
    focus: FocusHandle,
    claim: Rc<Cell<Option<u64>>>,
    version: dockspace::model::WorkspaceVersion,
    keys: std::collections::VecDeque<MenuKey>,
    row: usize,
}

impl MenuState {
    fn is_current(&self, shared: &SharedHost, window: &Window) -> bool {
        window.is_window_active()
            && self.claim.get() == Some(window.focus_claim_revision())
            && self.version == shared.borrow().session.version()
    }
}

pub(super) fn open(interaction: &InteractionState) -> Option<OpenMenu> {
    interaction.menu.borrow().as_ref().map(|input| input.open)
}

pub(super) fn cancel_pending_keys(interaction: &InteractionState) {
    if let Some(input) = interaction.menu.borrow_mut().as_mut() {
        input.keys.clear();
    }
}

pub(super) fn retire_for_tab_list(
    shared: &SharedHost,
    interaction: &InteractionState,
    window: &mut Window,
) {
    if let Some(open) = retire(interaction) {
        refresh_menu(shared, open.surface, window);
    }
}

fn retire(interaction: &InteractionState) -> Option<OpenMenu> {
    let previous = interaction.menu.borrow_mut().take()?;
    previous.claim.set(None);
    Some(previous.open)
}

fn dismiss_tab_list(
    shared: &SharedHost,
    interaction: &InteractionState,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let shown = {
        let host = shared.borrow();
        let owner = window.window_handle().window_id();
        let Some(mount) = host.mounts.get(&owner) else {
            return false;
        };
        mount.regions.iter().find_map(|(&surface, region)| {
            let action = region.packet.as_ref()?.tab_list_dismiss.as_ref()?;
            Some((
                surface,
                mount.serial,
                input::is_presented(&host, owner, surface, mount.serial),
                action.clone(),
            ))
        })
    };
    if let Some((surface, serial, presented, action)) = shown
        && !(presented
            && activate_action(shared, interaction, surface, serial, &action, window, cx))
    {
        return false;
    }
    interaction.tab_list_focus.borrow_mut().take();
    interaction.keyboard.borrow_mut().take();
    true
}

fn begin(
    shared: &SharedHost,
    interaction: &InteractionState,
    mut key: OpenMenu,
    focus: &FocusHandle,
    claim: &Rc<Cell<Option<u64>>>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(run) = interaction.menu_sequence.get().checked_add(1) else {
        return false;
    };
    if !dismiss_tab_list(shared, interaction, window, cx) {
        return false;
    }
    interaction.menu_sequence.set(run);
    key.run = run;
    if let Some(previous) = retire(interaction)
        && previous.surface != key.surface
    {
        refresh_menu(shared, previous.surface, window);
    }
    window.focus(focus, cx);
    claim.set(Some(window.focus_claim_revision()));
    *interaction.menu.borrow_mut() = Some(MenuState {
        open: key,
        focus: focus.clone(),
        claim: claim.clone(),
        version: shared.borrow().session.version(),
        keys: [MenuKey::First].into(),
        row: 0,
    });
    refresh_menu(shared, key.surface, window);
    true
}

fn dismiss(interaction: &InteractionState, expected: OpenMenu, window: &mut Window, cx: &mut App) {
    let Some(input) = interaction
        .menu
        .borrow_mut()
        .take_if(|input| input.open == expected)
    else {
        return;
    };
    if input.claim.get() == Some(window.focus_claim_revision())
        && window.is_focus_handle_rendered(&input.focus)
    {
        window.focus(&input.focus, cx);
        input.claim.set(Some(window.focus_claim_revision()));
    } else {
        input.claim.set(None);
    }
    window.refresh();
}

/// The shown menu has already supplied its original witness. Preserve that
/// command behind earlier local/native input instead of waiting on a UI retry.
fn submit(
    shared: &SharedHost,
    interaction: &InteractionState,
    expected: OpenMenu,
    serial: u64,
    action: &chrome::ActionSlot,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let mut host = shared.borrow_mut();
    let current = interaction
        .menu
        .borrow()
        .as_ref()
        .is_some_and(|input| input.open == expected && input.version == host.session.version());
    if !current
        || host.ensure_application_admission().is_err()
        || !input::is_presented(
            &host,
            window.window_handle().window_id(),
            expected.surface,
            serial,
        )
    {
        return false;
    }
    let Some(action) = action.borrow().clone() else {
        return false;
    };
    input::queue_action(&mut host, action.intent, cx);
    drop(host);
    window.refresh();
    host_native::schedule(shared, cx);
    true
}

fn queue_key(interaction: &InteractionState, key: MenuKey) -> bool {
    let mut input = interaction.menu.borrow_mut();
    let Some(input) = input.as_mut() else {
        return false;
    };
    if input.keys.iter().any(|key| matches!(key, MenuKey::Confirm)) {
        return false;
    }
    input.keys.push_back(key);
    true
}

fn handle_key(
    shared: &SharedHost,
    interaction: &InteractionState,
    key: &open_gpui::Keystroke,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if !window.is_window_active() || key.modifiers != open_gpui::Modifiers::none() {
        return false;
    }
    let valid = {
        let mut input = interaction.menu.borrow_mut();
        input.as_mut().is_some_and(|input| {
            if !input.is_current(shared, window) {
                // An explicit new key on the refocused opener starts a fresh
                // prefix. Never revive commands queued under its old claim.
                if input.version != shared.borrow().session.version()
                    || !window.is_focus_handle_rendered(&input.focus)
                    || !input.focus.is_focused(window)
                {
                    return false;
                }
                input.keys.clear();
                input.claim.set(Some(window.focus_claim_revision()));
            }
            true
        })
    };
    if !valid {
        return false;
    }
    let key = match key.key.as_str() {
        "down" => MenuKey::Next,
        "up" => MenuKey::Previous,
        "home" => MenuKey::First,
        "end" => MenuKey::Last,
        "enter" | "space" => MenuKey::Confirm,
        "escape" => {
            let terminal =
                interaction.menu.borrow().as_ref().is_some_and(|input| {
                    input.keys.iter().any(|key| matches!(key, MenuKey::Confirm))
                });
            if terminal {
                return false;
            }
            if let Some(open) = open(interaction) {
                dismiss(interaction, open, window, cx);
                return true;
            }
            return false;
        }
        _ => return false,
    };
    let queued = queue_key(interaction, key);
    if queued {
        window.refresh();
    }
    queued
}

/// An inert replay can temporarily remove the opener's focus handle. Continue
/// only its exact unrevoked focus claim; later application focus always wins.
pub(super) fn intercept(
    shared: &SharedHost,
    interaction: &InteractionState,
    key: &open_gpui::Keystroke,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if window.focused(cx).is_some() {
        return false;
    }
    let host = shared.borrow();
    let Some(mount) = host.mounts.get(&window.window_handle().window_id()) else {
        return false;
    };
    if !mount.attached
        || mount.capture.is_captured(window)
        || !window.is_focus_handle_rendered(&mount.focus)
    {
        return false;
    }
    drop(host);
    handle_key(shared, interaction, key, window, cx)
}

fn flush_keys(
    shared: &SharedHost,
    interaction: &InteractionState,
    menu: &PresentationMenu,
    surface: SurfaceId,
    serial: u64,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(open) = open(interaction).filter(|open| Some(*open) == menu.open) else {
        return;
    };
    if !input::is_presented(
        &shared.borrow(),
        window.window_handle().window_id(),
        surface,
        serial,
    ) {
        return;
    }
    loop {
        let (key, row) = {
            let mut input = interaction.menu.borrow_mut();
            let Some(input) = input.as_mut().filter(|input| {
                input.open == open
                    && input.is_current(shared, window)
                    && input.focus.is_focused(window)
            }) else {
                return;
            };
            (input.keys.pop_front(), input.row)
        };
        let Some(key) = key else {
            return;
        };
        let destination = match key {
            MenuKey::Next => Some(navigate(menu, row, true)),
            MenuKey::Previous => Some(navigate(menu, row, false)),
            MenuKey::First => enabled_rows(menu).next(),
            MenuKey::Last => enabled_rows(menu).next_back(),
            MenuKey::Confirm => {
                if let Some((_, Some(action))) = menu.choices.get(row) {
                    submit(shared, interaction, open, serial, action, window, cx);
                    dismiss(interaction, open, window, cx);
                }
                return;
            }
        };
        if let Some(destination) = destination
            && let Some(input) = interaction.menu.borrow_mut().as_mut()
        {
            input.row = destination;
            window.refresh();
        }
    }
}

/// A visual identity alone must never reopen a replacement group or session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OpenMenu {
    id: DockspaceVisualId,
    surface: SurfaceId,
    workspace: dockspace::runtime::DockspaceWorkspaceStamp,
    run: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    Root,
    Tabs,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::Root => "Layout actions",
            Self::Tabs => "Tab group actions",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Root => "Actions for the entire layout, including all split groups",
            Self::Tabs => "Actions for this tab group; other split groups stay in place",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Root => Icon::Layout,
            Self::Tabs => Icon::Menu,
        }
    }
}

impl PresentationMenu {
    pub(super) fn key(&self, surface: SurfaceId) -> OpenMenu {
        OpenMenu {
            id: self.id,
            surface,
            workspace: self.workspace,
            run: 0,
        }
    }
}

impl OpenMenu {
    pub(super) fn same_menu(self, other: Self) -> bool {
        self.id == other.id && self.surface == other.surface && self.workspace == other.workspace
    }

    pub(super) fn matches(
        self,
        id: DockspaceVisualId,
        plan: dockspace::runtime::SurfacePaintPlan<'_>,
    ) -> bool {
        self.id == id && self.surface == plan.surface() && self.workspace == plan.workspace_stamp()
    }
}

/// Called only when the candidate is installed; a discarded prepaint cannot
/// dismiss a menu belonging to the artifact still on screen.
pub(super) fn accept(shared: &SharedHost, interaction: &InteractionState, window: &mut Window) {
    let Some(open) = open(interaction) else {
        return;
    };
    let host = shared.borrow();
    let mount = host.mounts.get(&window.window_handle().window_id());
    if mount.is_some_and(|mount| {
        mount.regions.values().any(|region| {
            region
                .packet
                .as_ref()
                .is_some_and(|packet| packet.tab_list_dismiss.is_some())
        })
    }) {
        drop(host);
        retire_for_tab_list(shared, interaction, window);
        return;
    }
    let installed = mount
        .and_then(|mount| mount.regions.get(&open.surface))
        .and_then(|region| region.packet.as_ref())
        .and_then(|packet| {
            packet.draws.iter().find_map(|draw| match draw {
                Draw::PresentationMenu(menu)
                    if menu.enabled && open.same_menu(menu.key(open.surface)) =>
                {
                    Some(menu)
                }
                _ => None,
            })
        });
    match installed {
        Some(menu) => {
            let mut current = interaction.menu.borrow_mut();
            let Some(input) = current.as_mut() else {
                return;
            };
            let row = input.row;
            if !menu
                .choices
                .get(row)
                .is_some_and(|(_, action)| action.is_some())
                && let Some(first) = enabled_rows(menu).next()
                && row != first
            {
                input.row = first;
                window.refresh();
            }
        }
        None => {
            retire(interaction);
            window.refresh();
        }
    }
}

fn enabled_rows(menu: &PresentationMenu) -> impl DoubleEndedIterator<Item = usize> + '_ {
    menu.choices
        .iter()
        .enumerate()
        .filter_map(|(index, (_, action))| action.is_some().then_some(index))
}

fn navigate(menu: &PresentationMenu, row: usize, forward: bool) -> usize {
    if forward {
        enabled_rows(menu)
            .find(|index| *index > row)
            .or_else(|| enabled_rows(menu).next())
    } else {
        enabled_rows(menu)
            .rev()
            .find(|index| *index < row)
            .or_else(|| enabled_rows(menu).next_back())
    }
    .unwrap_or(row)
}

impl DockspaceElement {
    pub(super) fn prepare_presentation_menu(
        &self,
        menu: &Rc<PresentationMenu>,
        surface: SurfaceId,
        serial: u64,
        bounds: Bounds<Pixels>,
        allow_input: bool,
        draws: &mut Vec<PreparedDraw>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let rect = to_gpui(menu.bounds, bounds);
        let focus = self.control_focus(ControlFocusId::Visual(menu.id), cx);
        let admitted = allow_input
            && input::is_presented(
                &self.host.borrow(),
                window.window_handle().window_id(),
                surface,
                serial,
            );
        let enabled = admitted && menu.enabled;
        let key = menu.key(surface);
        let popup = self.interaction.menu.borrow().as_ref().and_then(|input| {
            (enabled && Some(input.open) == menu.open).then_some((input.open, input.row))
        });
        draws.push(PreparedDraw::Icon(rect, menu.scope.icon(), menu.enabled));
        let focus_claim = window.with_global_id(
            ElementId::NamedChild(std::sync::Arc::new((&focus).into()), "menu-focus".into()),
            |id, window| {
                window.with_element_state::<Rc<Cell<Option<u64>>>, _>(id, |state, _| {
                    let state = state.unwrap_or_default();
                    (state.clone(), state)
                })
            },
        );
        let close_menu = {
            let interaction = self.interaction.clone();
            move |window: &mut Window, cx: &mut App| {
                if let Some(open) = open(&interaction).filter(|open| open.same_menu(key)) {
                    dismiss(&interaction, open, window, cx);
                }
            }
        };
        let toggle_menu = {
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let focus = focus.clone();
            let focus_claim = focus_claim.clone();
            let close_menu = close_menu.clone();
            move |window: &mut Window, cx: &mut App| {
                if !enabled
                    || !input::is_presented(
                        &shared.borrow(),
                        window.window_handle().window_id(),
                        surface,
                        serial,
                    )
                {
                    return false;
                }
                if open(&interaction).is_some_and(|open| open.same_menu(key)) {
                    close_menu(window, cx);
                    true
                } else {
                    begin(&shared, &interaction, key, &focus, &focus_claim, window, cx)
                }
            }
        };
        let pointer_toggle = toggle_menu.clone();
        let selector = format!("dockspace:presentation-menu:{focus:?}");
        let mut button = div()
            .id(ElementId::NamedChild(
                std::sync::Arc::new((&focus).into()),
                "presentation-menu".into(),
            ))
            .debug_selector(move || selector.clone())
            .w(rect.size.width)
            .h(rect.size.height)
            .role(Role::Button)
            .aria_label(menu.scope.label())
            .aria_description(menu.scope.description())
            .aria_expanded(popup.is_some())
            .aria_disabled(!enabled)
            .aria_actions(if enabled {
                vec![AccessibleAction::Click, AccessibleAction::Focus]
            } else {
                vec![]
            })
            .when(enabled, |button| button.track_focus(&focus).tab_stop(true))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |event, window, cx| {
                // KeyDown owns menu navigation and confirmation. GPUI also
                // synthesizes a button click for a matching real KeyUp.
                if event.window_event().is_keyboard() || pointer_toggle(window, cx) {
                    cx.stop_propagation();
                }
            });
        let interaction = self.interaction.clone();
        let shared = self.host.clone();
        let keyboard_menu = menu.clone();
        let keyboard_toggle = toggle_menu.clone();
        button = button.on_key_down(move |event, window, cx| {
            if !window.is_window_active()
                || event.keystroke.modifiers != open_gpui::Modifiers::none()
            {
                return;
            }
            let is_open = open(&interaction).is_some_and(|open| open.same_menu(key));
            if is_open {
                if handle_key(&shared, &interaction, &event.keystroke, window, cx) {
                    flush_keys(
                        &shared,
                        &interaction,
                        &keyboard_menu,
                        surface,
                        serial,
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            } else if enabled
                && input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                )
                && matches!(
                    event.keystroke.key.as_str(),
                    "enter" | "space" | "down" | "up"
                )
                && keyboard_toggle(window, cx)
            {
                if event.keystroke.key == "up" {
                    queue_key(&interaction, MenuKey::Last);
                }
                cx.stop_propagation();
            }
        });
        button = button.on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
            toggle_menu(window, cx);
        });
        let shared = self.host.clone();
        let accessible_focus = focus.clone();
        button = button.on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
            if enabled
                && input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                )
            {
                window.focus(&accessible_focus, cx);
            }
        });
        let mut button = button
            .with_subtree_presentation(if admitted {
                SubtreePresentation::Visible
            } else {
                SubtreePresentation::Inert
            })
            .into_any_element();
        button.prepaint_as_root(rect.origin, definite(rect), window, cx);
        draws.push(PreparedDraw::ClippedElement(
            button,
            window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect),
        ));
        if enabled {
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let focus = focus.clone();
            let menu = menu.clone();
            window.record_prepaint_window_commit(move |_, window, cx| {
                if !window.is_window_active() {
                    return;
                }
                let Some(revision) = focus_claim.get() else {
                    return;
                };
                if revision != window.focus_claim_revision() {
                    focus_claim.set(None);
                    return;
                }
                if input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                ) && window.is_focus_handle_rendered(&focus)
                {
                    if !focus.is_focused(window) {
                        window.focus(&focus, cx);
                        focus_claim.set(Some(window.focus_claim_revision()));
                    }
                    if !open(&interaction).is_some_and(|open| open.same_menu(key)) {
                        focus_claim.set(None);
                    }
                    let shared = Rc::downgrade(&shared);
                    let interaction = Rc::downgrade(&interaction);
                    let menu = menu.clone();
                    // Command admission follows this frame's focus-stable
                    // observations and its exact interactive presentation.
                    window.defer(cx, move |window, cx| {
                        if let Some((shared, interaction)) =
                            shared.upgrade().zip(interaction.upgrade())
                        {
                            flush_keys(&shared, &interaction, &menu, surface, serial, window, cx);
                        }
                    });
                }
            });
        }
        let Some((popup_run, selected_row)) = popup else {
            return;
        };

        let colors = self.host.borrow().config.colors.clone();
        let width = px(260.0).min(bounds.size.width);
        let height = px(28.0) * menu.choices.len() as f32;
        let popup_bounds = Bounds::new(
            point(
                rect.right().min(bounds.right() - width).max(bounds.left()),
                rect.bottom()
                    .min((bounds.bottom() - height).max(bounds.top())),
            ),
            size(width, height.min(bounds.size.height)),
        );
        let shared = self.host.clone();
        let outside_close = close_menu.clone();
        let outside_interaction = self.interaction.clone();
        let popup_id = ElementId::NamedChild(
            std::sync::Arc::new((&focus).into()),
            format!("popup:{}", popup_run.run).into(),
        );
        let mut popup = div()
            .id(popup_id)
            .w(width)
            .h(popup_bounds.size.height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .overflow_y_scroll()
            .occlude()
            .bg(colors.selected_tab)
            .border_1()
            .border_color(colors.border)
            .role(Role::Menu)
            .aria_label(menu.scope.label())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(move |event, window, cx| {
                if open(&outside_interaction) == Some(popup_run)
                    && !rect.contains(&event.window_event().position)
                    && input::is_presented(
                        &shared.borrow(),
                        window.window_handle().window_id(),
                        surface,
                        serial,
                    )
                {
                    outside_close(window, cx);
                }
            });
        for (index, (label, action)) in menu.choices.iter().enumerate() {
            let action = action.clone();
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let mut row = div()
                .id(index)
                .w_full()
                .h(px(28.0))
                .flex_shrink_0()
                .px(px(8.0))
                .flex()
                .items_center()
                .role(Role::MenuItem)
                .aria_label(*label)
                .aria_disabled(action.is_none())
                .aria_selected(action.is_some() && selected_row == index)
                .aria_actions(if action.is_some() {
                    vec![AccessibleAction::Click]
                } else {
                    vec![]
                })
                .text_color(if action.is_some() {
                    colors.text
                } else {
                    colors.muted_text
                })
                .child(*label);
            if action.is_some() && selected_row == index {
                row = row.bg(colors.hovered_tab);
            }
            if let Some(action) = action {
                let accessible_action = action.clone();
                let accessible_shared = shared.clone();
                let accessible_interaction = interaction.clone();
                row = row
                    .on_click(move |_, window, cx| {
                        if submit(
                            &shared,
                            &interaction,
                            popup_run,
                            serial,
                            &action,
                            window,
                            cx,
                        ) {
                            dismiss(&interaction, popup_run, window, cx);
                        }
                        cx.stop_propagation();
                    })
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        if submit(
                            &accessible_shared,
                            &accessible_interaction,
                            popup_run,
                            serial,
                            &accessible_action,
                            window,
                            cx,
                        ) {
                            dismiss(&accessible_interaction, popup_run, window, cx);
                        }
                    });
            }
            popup = popup.child(row);
        }
        let mut popup = popup.into_any_element();
        let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), popup_bounds);
        let _ = window.with_prepared_subtree_clip(&clip, |window| {
            popup.prepaint_as_root(popup_bounds.origin, definite(popup_bounds), window, cx);
        });
        draws.push(PreparedDraw::ClippedElement(popup, clip));
    }
}
