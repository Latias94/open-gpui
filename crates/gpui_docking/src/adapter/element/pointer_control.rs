//! Window-scoped controls for the pointer press the adapter already owns.

use super::*;
use crate::adapter::native;
use open_gpui::Subscription;

pub(super) fn install(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    window: &mut Window,
) -> Subscription {
    let weak = Rc::downgrade(shared);
    let modifiers = window.observe_window_modifiers(move |event, window, cx| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let mut host = shared.borrow_mut();
        let enabled = host.config.input.docking_enabled(event.modifiers);
        let mode = host.config.input.group_dock_mode(event.modifiers);
        if let Some(binding) = managed_press(&host, window).map(native::NativeCallbacks::binding)
            && let Some(native) = host.native.as_mut()
        {
            native.queue_docking_controls(binding, enabled, mode, cx);
        } else if let Some(frame) = local_capture_frame(&host, window) {
            for directive in [
                SurfacePointerEvent::DockingEnabledChanged { enabled },
                SurfacePointerEvent::GroupDockModeChanged { mode },
            ] {
                input::queue(
                    &mut host,
                    frame.surface,
                    frame.serial,
                    directive,
                    window.mouse_position(),
                    frame.geometry,
                    &frame.hitboxes,
                    &frame.content_hitboxes,
                    true,
                    event.modifiers,
                    window,
                );
            }
            window.refresh();
        }
    });
    let weak = Rc::downgrade(shared);
    let interaction = Rc::downgrade(interaction);
    let cancel = window.intercept_window_key_down(move |event, window, cx| {
        let Some((shared, interaction)) = weak.upgrade().zip(interaction.upgrade()) else {
            return;
        };
        let (native, local) = {
            let host = shared.borrow();
            if !host.config.input.cancel_matches(&event.keystroke) {
                return;
            }
            (
                managed_press(&host, window).cloned(),
                local_capture(&host, window),
            )
        };
        let modifiers = event.keystroke.modifiers;
        let event =
            SurfacePointerEvent::StreamCancelled(SurfacePointerCancelReason::ExplicitCancellation);
        if let Some(callbacks) = native {
            if !callbacks.cancel_capture_from_keyboard(cx) {
                if host_native::routes_captured_drag(&shared, cx) {
                    // A platform capture already settling owns its real terminal.
                    return;
                }
                let Some(source) = callbacks.window_id() else {
                    return;
                };
                let mut host = shared.borrow_mut();
                let metadata = host
                    .config
                    .input
                    .pointer_metadata(host.observed_at(), Some(modifiers));
                if !host_native::queue_mouse_terminal(&mut host, source, false, event, metadata) {
                    return;
                }
                drop(host);
                host_native::schedule(&shared, cx);
            }
        } else if let Some(capture) = local {
            settle_primary_terminal(&shared, &interaction, event, modifiers, None, window, cx);
            let _ = window.release_pointer(&capture);
        } else {
            return;
        }
        interaction.splitter_press.borrow_mut().take();
        interaction.tab_list_press.borrow_mut().take();
        window.refresh();
        cx.stop_propagation();
    });
    Subscription::join(cancel, modifiers)
}

fn managed_press<'a>(host: &'a HostState, window: &Window) -> Option<&'a native::NativeCallbacks> {
    let native = host
        .native
        .as_ref()
        .filter(|native| !native.transport.is_frozen())?;
    let delivery = native.callbacks.get(&window.window_handle().window_id())?;
    let (source, lane) = native.transport.mouse_press()?;
    if lane == native::MousePressLane::Content
        || !delivery.accepts_pointer_controls()
        || (source != delivery.binding() && !window.is_window_active())
    {
        return None;
    }
    native
        .callbacks
        .values()
        .find(|callbacks| callbacks.binding() == source && callbacks.accepts_pointer_controls())
}

fn local_capture(host: &HostState, window: &Window) -> Option<open_gpui::PointerCaptureHandle> {
    if host.native.is_some() {
        return None;
    }
    let capture = host
        .mounts
        .get(&window.window_handle().window_id())?
        .capture;
    capture.is_captured(window).then_some(capture)
}

fn local_capture_frame(host: &HostState, window: &Window) -> Option<input::InputFrame> {
    local_capture(host, window)?;
    let owner = window.window_handle().window_id();
    let mount = host.mounts.get(&owner)?;
    let surface = displayed_surface_at(host, window.mouse_position(), window)
        .or_else(|| mount.primary_press.as_ref()?.captured_local_surface())?;
    mount
        .input_frames
        .iter()
        .find(|frame| frame.surface == surface)
        .cloned()
}

impl DockspaceElement {
    pub(super) fn listen_content_terminals(&self, window: &mut Window) {
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase.capture() {
                interaction.content_terminal.set(false);
                if event.button == MouseButton::Left {
                    settle_primary_terminal(
                        &shared,
                        &interaction,
                        SurfacePointerEvent::ButtonReleased(SurfacePointerButton::Primary),
                        event.modifiers,
                        Some(event.position),
                        window,
                        cx,
                    );
                }
            }
        });
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        window.on_pointer_cancel(move |_, phase, window, cx| {
            if phase.capture() {
                interaction.splitter_press.borrow_mut().take();
                interaction.tab_list_press.borrow_mut().take();
                settle_primary_terminal(
                    &shared,
                    &interaction,
                    SurfacePointerEvent::StreamCancelled(
                        SurfacePointerCancelReason::ExplicitPlatformCancellation,
                    ),
                    window.modifiers(),
                    None,
                    window,
                    cx,
                );
            }
        });
    }

    pub(super) fn listen(
        &self,
        prepared: &PreparedRegion,
        allow_new_input: bool,
        window: &mut Window,
    ) {
        if allow_new_input {
            wheel::listen(&self.host, &self.interaction, prepared, window);
        }
        let surface = prepared.surface;
        let capture = prepared.capture;
        let serial = prepared.serial;
        let geometry = prepared.geometry;
        let owner = window.window_handle().window_id();
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseExitEvent, phase, window, _| {
            if !phase.capture() {
                return;
            }
            if interaction.hovered_tab.take().is_some() {
                window.refresh();
            }
            let mut host = shared.borrow_mut();
            if host.native.is_none()
                && capture.is_captured(window)
                && routes_to(&host, surface, event.position, window)
                && input::queue(
                    &mut host,
                    surface,
                    serial,
                    SurfacePointerEvent::Moved,
                    event.position,
                    geometry,
                    &hitboxes,
                    &content_hitboxes,
                    true,
                    window.modifiers(),
                    window,
                )
            {
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let controls = prepared.controls.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !allow_new_input || event.button != MouseButton::Left {
                return;
            }
            let target = window.frontmost_hitbox_at(event.position);
            if phase.capture() {
                interaction.splitter_press.borrow_mut().take();
                interaction.tab_list_press.borrow_mut().take();
                if !capture.is_captured(window)
                    && input::is_presented(&shared.borrow(), owner, surface, serial)
                    && target.is_some_and(|id| content_hitboxes.contains(&id))
                    && input::queue(
                        &mut shared.borrow_mut(),
                        surface,
                        serial,
                        SurfacePointerEvent::ButtonPressed(SurfacePointerButton::Primary),
                        event.position,
                        geometry,
                        &hitboxes,
                        &content_hitboxes,
                        false,
                        event.modifiers,
                        window,
                    )
                {
                    // Observe pane activation without capturing or consuming the widget's input.
                    window.refresh();
                }
            }
            if !phase.bubble() {
                return;
            }
            let Some(target) = target.filter(|id| hitboxes.contains(id)) else {
                return;
            };
            if !input::is_presented(&shared.borrow(), owner, surface, serial) {
                return;
            }
            let managed = shared.borrow().native.is_some();
            if !managed && window.capture_pointer(&capture, MouseButton::Left).is_err() {
                return;
            }
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::ButtonPressed(SurfacePointerButton::Primary),
                event.position,
                geometry,
                &hitboxes,
                &content_hitboxes,
                !managed,
                event.modifiers,
                window,
            ) {
                let control = controls
                    .iter()
                    .rev()
                    .find(|(_, _, hitbox)| *hitbox == target);
                splitter::pressed(
                    &shared.borrow(),
                    &interaction,
                    surface,
                    control.map(|(control, _, _)| control.as_ref()),
                    event,
                );
                if let Some((control, focus, _)) = control {
                    window.focus(focus, cx);
                    if let Some(bar) = control.opens_menu {
                        if let Some(opener) = control
                            .slot(ControlAction::Activate)
                            .and_then(|slot| slot.borrow().as_ref()?.tab_list_opener.clone())
                            .filter(|opener| opener.opening.is_some())
                        {
                            *interaction.tab_list_press.borrow_mut() =
                                Some(TabListPress { surface, opener });
                        }
                        presentation_menu::revoke(&shared, &interaction, window);
                        keyboard::remember_menu_opener(&interaction, surface, bar, focus, window);
                    }
                } else {
                    let focus = shared.borrow().mounts[&owner].focus.clone();
                    window.focus(&focus, cx);
                }
                window.refresh();
                if !managed {
                    cx.stop_propagation();
                }
            } else {
                let _ = window.release_pointer(&capture);
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase.capture() {
                splitter::moved(&interaction, event.position);
            }
            if !phase.bubble()
                || !capture.is_captured(window)
                || !routes_to(&shared.borrow(), surface, event.position, window)
            {
                return;
            }
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::Moved,
                event.position,
                geometry,
                &hitboxes,
                &content_hitboxes,
                true,
                event.modifiers,
                window,
            ) {
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let receivers = prepared.receivers.clone();
        let controls = prepared.controls.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if !phase.bubble()
                || interaction.content_terminal.get()
                || event.button != MouseButton::Left
                || (!capture.is_captured(window) && shared.borrow().native.is_none())
                || !routes_to(&shared.borrow(), surface, event.position, window)
            {
                return;
            }
            if host_native::routes_captured_drag(&shared, cx) {
                return;
            }
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::ButtonReleased(SurfacePointerButton::Primary),
                event.position,
                geometry,
                &hitboxes,
                &content_hitboxes,
                capture.is_captured(window),
                event.modifiers,
                window,
            ) {
                let target = window.frontmost_hitbox_at(event.position);
                splitter::released(
                    &mut shared.borrow_mut(),
                    &interaction,
                    surface,
                    &receivers,
                    target,
                    event,
                    window,
                    cx,
                );
                let _ = window.release_pointer(&capture);
                let press = interaction.tab_list_press.borrow_mut().take();
                if let Some(press) = press.filter(|press| press.surface == surface)
                    && window.is_window_active()
                    && input::is_presented(&shared.borrow(), owner, surface, serial)
                    && controls.iter().any(|(control, _, hitbox)| {
                        control.expanded == Some(false)
                            && control.focus_id()
                                == ControlFocusId::Control(press.opener.input_id.clone())
                            && Some(*hitbox) == target
                    })
                {
                    // Hold the next keys until core presents the list opened by this
                    // press witness. A rejected release leaves the product menu intact.
                    begin_tab_list_opening(
                        &shared,
                        &interaction,
                        surface,
                        serial,
                        press.opener,
                        TabListOpenSource::PointerRelease,
                        window,
                        cx,
                    );
                    refresh_menu(&shared, surface, window);
                }
                if shared.borrow().native.is_some() {
                    host_native::schedule(&shared, cx);
                }
                window.refresh();
                cx.stop_propagation();
            }
        });
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        let controls = prepared.controls.clone();
        let pane_bars = prepared.pane_bars.clone();
        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            if !allow_new_input || !phase.bubble() || capture.is_captured(window) {
                return;
            }
            if keyboard::document_cycle(&event.keystroke).is_none() {
                return;
            }
            let control = {
                let host = shared.borrow();
                let scope = &host.mounts[&owner].focus;
                let focused_header = controls.iter().find(|(control, focus, _)| {
                    matches!(control.kind, ControlKind::Tab(_)) && focus.is_focused(window)
                });
                let focused_bar = focused_header
                    .and_then(|(control, _, _)| Some(control.tab_strip.as_ref()?.visual_id()))
                    .or_else(|| {
                        pane_bars.iter().find_map(|(item, bar)| {
                            host.panels
                                .get(item)
                                .and_then(|pane| pane.content_focus.as_ref())
                                .is_some_and(|focus| focus.contains_focused(window, cx))
                                .then_some(*bar)
                                .flatten()
                        })
                    });
                focused_header
                    .filter(|(control, _, _)| control.receiver.is_some())
                    .map(|(control, _, _)| control.clone())
                    .or_else(|| {
                        controls.iter().rev().find_map(|(control, _, _)| {
                            if !matches!(control.kind, ControlKind::Tab(_))
                                || control.receiver.is_none()
                            {
                                return None;
                            }
                            let owns_focus = focused_bar.map_or_else(
                                || scope.is_focused(window) && control.selected == Some(true),
                                |bar| {
                                    control
                                        .tab_strip
                                        .as_ref()
                                        .is_some_and(|strip| strip.visual_id() == bar)
                                },
                            );
                            owns_focus.then(|| control.clone())
                        })
                    })
            };
            if let Some(control) = control
                && keyboard::handle(
                    &shared,
                    &interaction,
                    surface,
                    serial,
                    &control,
                    &event.keystroke,
                    window,
                    cx,
                )
            {
                cx.stop_propagation();
            }
        });
    }
}

pub(super) fn hovered_tab_at(
    tabs: &[(TabKey, HitboxId)],
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> Option<TabKey> {
    if !window.is_mouse_in_window()
        || window.last_input_was_keyboard()
        || window.last_input_was_touch()
    {
        return None;
    }
    let target = window.frontmost_hitbox_at(position)?;
    tabs.iter()
        .find_map(|(item, hitbox)| (*hitbox == target).then_some(*item))
}

fn routes_to(
    host: &HostState,
    surface: SurfaceId,
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> bool {
    let target = displayed_surface_at(host, position, window).or_else(|| {
        if host.native.is_some() {
            return None;
        }
        let owner = window.window_handle().window_id();
        host.session
            .host_pointer_manipulation_surface(input::local_pointer(owner))
            .map(|source| source.surface())
            // GPUI capture starts on the callback, before core sees the press.
            .or_else(|| {
                host.mounts
                    .get(&owner)?
                    .primary_press
                    .as_ref()?
                    .captured_local_surface()
            })
    });
    target == Some(surface)
}

fn displayed_surface_at(
    host: &HostState,
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> Option<SurfaceId> {
    let owner = window.window_handle().window_id();
    let mount = host.mounts.get(&owner)?;
    let frames = mount
        .pending
        .as_ref()
        .and_then(presentation::PendingSubmission::accepted_input_frames)
        .unwrap_or(&mount.input_frames);
    frames
        .iter()
        .find(|frame| {
            frame.geometry.displayed_bounds().contains(&position) && frame.clip.contains(&position)
        })
        .map(|frame| frame.surface)
}

fn settle_primary_terminal(
    shared: &SharedHost,
    interaction: &InteractionState,
    event: SurfacePointerEvent,
    modifiers: open_gpui::Modifiers,
    release_position: Option<open_gpui::Point<Pixels>>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut host = shared.borrow_mut();
    let mut handled = input::finish_content_press(&mut host, event, modifiers, window);
    if !handled && host.native.is_some() && !host_native::routes_captured_drag(shared, cx) {
        let metadata = host
            .config
            .input
            .pointer_metadata(host.observed_at(), Some(modifiers));
        let routed = release_position
            .is_some_and(|point| displayed_surface_at(&host, point, window).is_some());
        handled = host_native::queue_mouse_terminal(
            &mut host,
            window.window_handle().window_id(),
            routed,
            event,
            metadata,
        );
        if handled {
            interaction.splitter_press.borrow_mut().take();
            interaction.tab_list_press.borrow_mut().take();
            window.refresh();
        }
    }
    interaction.content_terminal.set(handled);
    if handled
        && host.native.is_none()
        && !window.is_visible()
        && let Err(error) = presentation::settle_pending(&mut host, Some(window), cx)
    {
        host.fail(error.as_ref());
    }
    let wake_native = handled && host.native.is_some();
    drop(host);
    if wake_native {
        host_native::schedule(shared, cx);
    }
}
