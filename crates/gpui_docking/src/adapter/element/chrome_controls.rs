//! Activation and focus handoff for the docking chrome.

use super::*;

impl DockspaceElement {
    pub(super) fn control_focus(&self, id: ControlFocusId, cx: &mut App) -> FocusHandle {
        self.interaction
            .focus
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    pub(super) fn prepare_control(
        &self,
        control: &Rc<Control>,
        focus: FocusHandle,
        surface: SurfaceId,
        serial: u64,
        bounds: Bounds<Pixels>,
        allow_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> (Option<(AnyElement, PreparedSubtreeClip)>, Option<HitboxId>) {
        let admitted = allow_input
            && input::is_presented(
                &self.host.borrow(),
                window.window_handle().window_id(),
                surface,
                serial,
            );
        let menu_row = control.menu_row.is_some();
        let enabled = menu_row
            || control
                .actions
                .iter()
                .any(|(_, action)| action.borrow().is_some());
        let mut actions = Vec::new();
        if enabled {
            actions.push(AccessibleAction::Focus);
            if control.has_action(ControlAction::Activate) {
                actions.push(AccessibleAction::Click);
            }
            if control.has_action(ControlAction::Increment) {
                actions.push(AccessibleAction::Increment);
            }
            if control.has_action(ControlAction::Decrement) {
                actions.push(AccessibleAction::Decrement);
            }
        }
        let rect = to_gpui(control.bounds, bounds);
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        let keyboard_control = control.clone();
        let mut element = div()
            .id(&focus)
            .debug_selector({
                let selector = format!("dockspace:control:{focus:?}");
                move || selector.clone()
            })
            .w(rect.size.width)
            .h(rect.size.height)
            .role(control.role())
            .aria_label(control.label.clone())
            .aria_disabled(!enabled)
            .aria_actions(actions)
            .when(enabled, |element| {
                element.track_focus(&focus).tab_stop(match control.kind {
                    ControlKind::Tab(_) => control.selected == Some(true),
                    _ => true,
                })
            })
            .when_some(control.selected, |element, selected| {
                element.aria_selected(selected)
            })
            .when_some(control.expanded, |element, expanded| {
                element.aria_expanded(expanded)
            })
            .when_some(control.value, |element, (value, minimum, maximum)| {
                element
                    .aria_numeric_value(value)
                    .aria_min_numeric_value(minimum)
                    .aria_max_numeric_value(maximum)
            })
            .on_key_down(move |event, window, cx| {
                if event.keystroke.modifiers != open_gpui::Modifiers::none() {
                    return;
                }
                if keyboard::handle(
                    &shared,
                    &interaction,
                    surface,
                    serial,
                    &keyboard_control,
                    &event.keystroke,
                    window,
                    cx,
                ) {
                    cx.stop_propagation();
                }
            });
        if let ControlKind::Splitter(axis) = control.kind {
            element = element.aria_orientation(match axis {
                dockspace::model::DockspaceAxis::Horizontal => open_gpui::Orientation::Vertical,
                dockspace::model::DockspaceAxis::Vertical => open_gpui::Orientation::Horizontal,
            });
        }
        if enabled {
            element = element.cursor(match control.kind {
                ControlKind::Splitter(dockspace::model::DockspaceAxis::Horizontal) => {
                    open_gpui::CursorStyle::ResizeLeftRight
                }
                ControlKind::Splitter(dockspace::model::DockspaceAxis::Vertical) => {
                    open_gpui::CursorStyle::ResizeUpDown
                }
                ControlKind::Junction => open_gpui::CursorStyle::Crosshair,
                _ => open_gpui::CursorStyle::PointingHand,
            });
        }
        for (action, key) in [
            (AccessibleAction::Click, ControlAction::Activate),
            (AccessibleAction::Increment, ControlAction::Increment),
            (AccessibleAction::Decrement, ControlAction::Decrement),
        ] {
            if control.has_action(key) {
                let shared = self.host.clone();
                let interaction = self.interaction.clone();
                let control = control.clone();
                element = element.on_a11y_action(action, move |_, window, cx| {
                    if let Some(slot) = control.slot(key) {
                        activate_action(&shared, &interaction, surface, serial, &slot, window, cx);
                    }
                });
            }
        }
        if self.host.borrow().native.is_none()
            && matches!(control.kind, ControlKind::Button | ControlKind::MenuRow)
            && control.has_action(ControlAction::Activate)
        {
            let shared = self.host.clone();
            let pointer_focus = focus.clone();
            element = element.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                ) {
                    window.focus(&pointer_focus, cx);
                }
                cx.stop_propagation();
            });
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let control = control.clone();
            element = element.on_click(move |_, window, cx| {
                if let Some(slot) = control.slot(ControlAction::Activate) {
                    activate_action(&shared, &interaction, surface, serial, &slot, window, cx);
                }
                cx.stop_propagation();
            });
        }
        if enabled {
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let control = control.clone();
            element = element.on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                if control.menu_row.is_some() {
                    keyboard::focus_menu_row(
                        &shared,
                        &interaction,
                        surface,
                        serial,
                        &control,
                        &focus,
                        window,
                        cx,
                    );
                } else if input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                ) {
                    window.focus(&focus, cx);
                }
            });
        }
        let hitbox = Rc::new(Cell::new(None));
        let mut element = chrome::ControlElement {
            inner: element,
            hitbox: hitbox.clone(),
        }
        .into_any_element()
        .with_subtree_presentation(if admitted {
            SubtreePresentation::Visible
        } else {
            SubtreePresentation::Inert
        })
        .into_any_element();
        let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect);
        let prepared = window
            .with_prepared_subtree_clip(&clip, |window| {
                element.prepaint_as_root(rect.origin, definite(rect), window, cx);
            })
            .map(|()| (element, clip));
        (prepared, hitbox.get())
    }
}

pub(super) fn activate_action(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    slot: &chrome::ActionSlot,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    activate_action_with_continuation(
        shared,
        interaction,
        surface,
        serial,
        slot,
        false,
        window,
        cx,
    )
}

pub(super) fn activate_action_with_continuation(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    slot: &chrome::ActionSlot,
    continuing: bool,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    // Semantic events retain the witness of the output still shown by GPUI.
    // A preceding selection may revoke current pointer receivers before the
    // next paint, but core can still validate that original deferred command.
    // A replaced output must never lend its witness to a later callback.
    let Some(action) = slot.borrow().clone() else {
        return false;
    };
    if action.tab_list_opener.is_some() && !window.is_window_active() {
        return false;
    }
    if !keyboard::admits_action(&host, interaction, surface, serial, continuing, window) {
        // The displayed opener can replace a product menu even while the
        // renderer is settling a candidate. Retain its original action in the
        // ordered input queue and retire the product menu's uncommitted keys.
        if let Some(opener) = action.tab_list_opener
            && host.has_unaccepted_update()
            && presentation_menu::open(interaction).is_some()
            && action.intent.expected_version() == host.session.version()
            && host.ensure_application_admission().is_ok()
            && input::is_presented(&host, owner, surface, serial)
        {
            input::queue_action(&mut host, owner, action.intent, cx);
            drop(host);
            presentation_menu::revoke(shared, interaction, window);
            begin_tab_list_opening(
                shared,
                interaction,
                surface,
                serial,
                opener,
                TabListOpenSource::Action,
                window,
                cx,
            );
            window.refresh();
            host_native::schedule(shared, cx);
            return true;
        }
        return false;
    }
    let tab_focus = action
        .intent
        .tab_focus_input_id()
        .zip(action.intent.tab_focus_target());
    let version = action.intent.expected_version();
    let yields_content = action.yields_content;
    let previous_focus = host.session.pane_focus_request(surface);
    let result = (|| -> Result<_> {
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame =
            host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx)?;
        frame.submit_deferred_surface_action(action.intent)?;
        Ok(frame.commit()?)
    })();
    match result {
        Ok(report) => {
            let rejected = report.inputs().iter().any(|input| {
                matches!(
                    input,
                    dockspace::runtime::HostInputOutcome::StaleRejected { .. }
                        | dockspace::runtime::HostInputOutcome::ProductActionRejected(_)
                        | dockspace::runtime::HostInputOutcome::CloseRejected(_)
                )
            });
            host.apply_report(report);
            // Independent events keep their original witness until presentation
            // changes. Core revalidates that target after each earlier event;
            // a fresh packet must reflect the resulting chrome and focus state.
            if let Some(region) = host
                .mounts
                .get_mut(&owner)
                .and_then(|mount| mount.regions.get_mut(&surface))
            {
                region.dirty = true;
            }
            if rejected {
                interaction.keyboard.borrow_mut().take();
                interaction.pending_tab_focus.borrow_mut().take();
                window.refresh();
                return false;
            }
        }
        Err(error) => {
            host.fail(error.as_ref());
            return false;
        }
    }
    let request = host
        .session
        .pane_focus_request(surface)
        .filter(|request| yields_content && Some(*request) != previous_focus);
    let content_focus = request.and_then(|request| {
        request
            .item()
            .and_then(|item| host.panels.get(&item))
            .and_then(|pane| pane.content_focus.as_ref())
            .filter(|focus| window.is_focus_handle_rendered(focus))
            .cloned()
            .or_else(|| host.mounts.get(&owner).map(|mount| mount.focus.clone()))
    });
    let focus = tab_focus
        .filter(|_| !yields_content)
        .filter(|(_, key)| keyboard::tab_is_selected(host.session.view(), surface, *key))
        .map(|(input_id, _)| {
            let target = ControlFocusId::Control(input_id);
            (
                target.clone(),
                interaction
                    .focus
                    .borrow_mut()
                    .entry(target)
                    .or_insert_with(|| cx.focus_handle())
                    .clone(),
            )
        });
    drop(host);
    if yields_content {
        // Activation ends the header's input prefix even before another paint.
        // A clipped pane receives focus through the ordinary presentation path;
        // its rendered mount scope owns the intervening suffix.
        interaction.keyboard.borrow_mut().take();
        interaction.pending_tab_focus.borrow_mut().take();
        if let Some(focus) = content_focus {
            window.focus(&focus, cx);
        }
        if let Some(request) = request {
            let attempt = PaneFocusAttempt::new(
                request,
                Some(PaneFocusClaim {
                    owner,
                    revision: window.focus_claim_revision(),
                }),
            );
            interaction.pane_focus.borrow_mut().insert(surface, attempt);
        }
    } else if let Some((target, focus)) = focus {
        window.focus(&focus, cx);
        *interaction.pending_tab_focus.borrow_mut() = Some(PendingTabFocus {
            surface,
            target,
            version,
            claim_revision: window.focus_claim_revision(),
        });
    } else if let Some(opener) = action.tab_list_opener {
        presentation_menu::revoke(shared, interaction, window);
        begin_tab_list_opening(
            shared,
            interaction,
            surface,
            serial,
            opener,
            TabListOpenSource::Action,
            window,
            cx,
        );
    }
    window.refresh();
    cx.refresh_windows();
    host_native::schedule(shared, cx);
    true
}

pub(super) fn begin_tab_list_opening(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    opener: chrome::TabListOpener,
    source: TabListOpenSource,
    window: &mut Window,
    cx: &mut App,
) {
    let focus = interaction
        .focus
        .borrow()
        .get(&ControlFocusId::Control(opener.input_id))
        .cloned();
    if let Some(focus) = focus.filter(|focus| window.is_focus_handle_rendered(focus)) {
        let direct = matches!(source, TabListOpenSource::Action);
        if direct && !focus.is_focused(window) {
            window.focus(&focus, cx);
        }
        // A pointer release may be rejected by core. Keep the current focus
        // claim, and only hold keys from an opener or menu that already has it.
        let retains_keys = direct
            || focus.is_focused(window)
            || presentation_menu::owns_keys(shared, interaction, window);
        let opening = opener.opening.map(|proof| {
            keyboard::MenuOpening::new(
                proof,
                shared.borrow().session.version(),
                serial,
                retains_keys,
                presentation_menu::open(interaction),
            )
        });
        *interaction.tab_list_focus.borrow_mut() = Some(TabListFocus {
            surface,
            bar: opener.bar,
            row: None,
            opener: focus,
            claim_revision: window.focus_claim_revision(),
            opening,
        });
    }
}

pub(super) fn refresh_menu(shared: &SharedHost, surface: SurfaceId, window: &mut Window) {
    if let Some(region) = shared
        .borrow_mut()
        .mounts
        .get_mut(&window.window_handle().window_id())
        .and_then(|mount| mount.regions.get_mut(&surface))
    {
        region.dirty = true;
    }
    window.refresh();
}

/// Restores framework chrome focus only after the matching output's interactive
/// replay. A later application focus claim always wins over retained UI intent.
pub(super) fn apply_chrome_focus(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    serial: u64,
    admitted: &[SurfaceId],
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    window: &mut Window,
    cx: &mut App,
) {
    if !window.is_window_active() || shared.borrow().ensure_application_admission().is_err() {
        return;
    }
    let owner = window.window_handle().window_id();
    let presented = |surface| {
        admitted.contains(&surface) && input::is_presented(&shared.borrow(), owner, surface, serial)
    };
    if presentation_menu::open(interaction).is_none() {
        keyboard::restore(shared, interaction, serial, admitted, controls, window, cx);
    }
    if keyboard::restore_menu_opening(
        shared,
        interaction,
        serial,
        controls,
        &presented,
        window,
        cx,
    ) || presentation_menu::open(interaction).is_some()
    {
        return;
    }
    let pending = interaction.pending_tab_focus.borrow_mut().take();
    if let Some(pending) = pending {
        if pending
            .version
            .is_same_epoch(shared.borrow().session.version())
            && pending.claim_revision == window.focus_claim_revision()
        {
            if presented(pending.surface) {
                if let Some((_, _, focus)) = controls.iter().find(|(surface, control, _)| {
                    *surface == pending.surface && control.focus_id() == pending.target
                }) && window.is_focus_handle_rendered(focus)
                {
                    window.focus(focus, cx);
                }
            } else {
                *interaction.pending_tab_focus.borrow_mut() = Some(pending);
            }
        }
    }
    if interaction
        .keyboard
        .borrow()
        .as_ref()
        .is_some_and(|prefix| prefix.owns_menu_focus(window))
    {
        return;
    }
    let focused_row = controls.iter().find_map(|(surface, control, focus)| {
        let row = control.menu_row.as_ref()?;
        (row.focused && presented(*surface)).then_some((surface, row, focus))
    });
    if let Some((surface, row, focus)) = focused_row {
        let bar = row.keys.bar;
        let mut previous = interaction.tab_list_focus.borrow_mut();
        if let Some(previous) = previous.as_ref() {
            if previous.surface != *surface
                || previous.bar != bar
                || previous.claim_revision != window.focus_claim_revision()
                || (previous.row == Some(row.id) && focus.is_focused(window))
            {
                return;
            }
        } else {
            let scope = shared.borrow().mounts[&owner].focus.clone();
            if !scope.contains_focused(window, cx) && window.focused(cx).is_some() {
                return;
            }
        }
        let opener = controls
            .iter()
            .find(|(owner_surface, control, _)| {
                owner_surface == surface && control.opens_menu == Some(bar)
            })
            .map(|(_, _, focus)| focus.clone());
        if let Some(opener) = opener
            && previous
                .as_ref()
                .is_none_or(|previous| previous.opener == opener)
            && window.is_focus_handle_rendered(focus)
        {
            window.focus(focus, cx);
            *previous = Some(TabListFocus {
                surface: *surface,
                bar,
                row: Some(row.id),
                opener,
                claim_revision: window.focus_claim_revision(),
                opening: None,
            });
        }
    } else {
        let mut previous = interaction.tab_list_focus.borrow_mut();
        if previous
            .as_ref()
            .is_some_and(|previous| presented(previous.surface))
            && let Some(previous) = previous.take()
        {
            let pane_requested = shared
                .borrow()
                .session
                .pane_focus_request(previous.surface)
                .is_some();
            if !pane_requested
                && previous.claim_revision == window.focus_claim_revision()
                && window.is_focus_handle_rendered(&previous.opener)
            {
                window.focus(&previous.opener, cx);
            }
        }
    }
}
