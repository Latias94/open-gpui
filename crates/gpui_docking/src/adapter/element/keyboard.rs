//! Preserve core-authorized header input until activation yields to pane content.

use dockspace::{
    model::WorkspaceVersion,
    runtime::{SurfacePaintPlan, SurfaceTabInput, SurfaceTabNavigation, TabBarPaintRecord},
};
use open_gpui::Keystroke;

use super::*;

/// Each destination keeps actions from the original operable receiver, including
/// members with no visible control. Core resolves every move and validates its
/// frozen witness; the adapter only chooses the focused or selected starting item.
pub(super) struct TabKeys {
    members: BTreeMap<DockspaceVisualId, TabMember>,
}

struct TabMember {
    item: ItemId,
    actions: Vec<(ControlAction, chrome::ActionSlot)>,
}

impl TabKeys {
    pub(super) fn record(plan: SurfacePaintPlan<'_>, bar: TabBarPaintRecord<'_>) -> Option<Self> {
        let source = bar.members().find_map(|member| {
            plan.prepare_tab_input_sequence(
                member.item(),
                member.item(),
                [SurfaceTabInput::Activate],
            )
            .map(|_| member.item())
        })?;
        let mut members = BTreeMap::new();
        for member in bar.members() {
            let mut actions = Vec::new();
            for (key, input) in [
                (ControlAction::Activate, SurfaceTabInput::Activate),
                (
                    ControlAction::Previous,
                    SurfaceTabInput::Navigate(SurfaceTabNavigation::Previous),
                ),
                (
                    ControlAction::Next,
                    SurfaceTabInput::Navigate(SurfaceTabNavigation::Next),
                ),
                (
                    ControlAction::First,
                    SurfaceTabInput::Navigate(SurfaceTabNavigation::First),
                ),
                (
                    ControlAction::Last,
                    SurfaceTabInput::Navigate(SurfaceTabNavigation::Last),
                ),
            ] {
                let Some(action) = plan.prepare_tab_input_sequence(source, member.item(), [input])
                else {
                    continue;
                };
                let mut action = chrome::Action::from(action);
                action.yields_content = key == ControlAction::Activate;
                actions.push((key, Rc::new(RefCell::new(Some(action)))));
            }
            members.insert(
                member.visual_id(),
                TabMember {
                    item: member.item(),
                    actions,
                },
            );
        }
        Some(Self { members })
    }

    fn slot(
        &self,
        focused: DockspaceVisualId,
        key: ControlAction,
        selected_start: bool,
        host: &HostState,
    ) -> Option<chrome::ActionSlot> {
        let member = if selected_start {
            self.members.values().find(|member| {
                host.session
                    .view()
                    .item(member.item)
                    .is_some_and(|item| item.is_selected())
            })?
        } else {
            self.members.get(&focused)?
        };
        member
            .actions
            .iter()
            .find_map(|(candidate, action)| (*candidate == key).then(|| action.clone()))
    }
}

#[derive(Clone)]
pub(super) struct Continuation {
    surface: SurfaceId,
    serial: u64,
    version: WorkspaceVersion,
    control: Rc<Control>,
    target: DockspaceVisualId,
    claim_revision: u64,
    pending: Option<chrome::ActionSlot>,
    submitted: bool,
    activate_on_present: bool,
}

impl Continuation {
    fn is_current(&self, host: &HostState, window: &Window) -> bool {
        self.version == host.session.version()
            && self.claim_revision == window.focus_claim_revision()
    }

    fn owns(&self, control: &Control) -> bool {
        self.control.id == control.id
            || self.target == control.id
            || self
                .control
                .tab_keys
                .as_ref()
                .zip(control.tab_keys.as_ref())
                .is_some_and(|(left, right)| Rc::ptr_eq(left, right))
    }
}

pub(super) fn admits_action(
    host: &HostState,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    continuing: bool,
    window: &Window,
) -> bool {
    let shown = host
        .mounts
        .get(&window.window_handle().window_id())
        .and_then(|mount| mount.regions.get(&surface))
        .and_then(|region| region.presented)
        .is_some_and(|(presented_serial, _)| presented_serial == serial);
    let continuing = continuing
        && interaction
            .keyboard
            .borrow()
            .as_ref()
            .is_some_and(|current| {
                current.submitted
                    && current.surface == surface
                    && current.serial == serial
                    && current.is_current(host, window)
            });
    (shown || continuing)
        && !host.has_unaccepted_update()
        && host.inputs.is_empty()
        && !host.session.has_pending_native_input()
        && host
            .native
            .as_ref()
            .is_none_or(|native| native.transport.is_empty())
        && host.ensure_application_admission().is_ok()
}

pub(super) fn document_cycle(keystroke: &Keystroke) -> Option<ControlAction> {
    let modifiers = keystroke.modifiers;
    if !modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    match keystroke.key.as_str() {
        "tab" if modifiers.shift => Some(ControlAction::Previous),
        "tab" | "pagedown" => Some(ControlAction::Next),
        "pageup" => Some(ControlAction::Previous),
        _ => None,
    }
}

pub(super) fn handle(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    surface: SurfaceId,
    serial: u64,
    control: &Rc<Control>,
    keystroke: &Keystroke,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let (key, selected_start) = if keystroke.modifiers == open_gpui::Modifiers::none() {
        let Some(key) = control.key_action(&keystroke.key) else {
            return false;
        };
        (key, false)
    } else if matches!(control.kind, ControlKind::Tab(_)) {
        let Some(key) = document_cycle(keystroke) else {
            return false;
        };
        (key, true)
    } else {
        return false;
    };
    let (previous, slot) = {
        let host = shared.borrow();
        let mut retained = interaction.keyboard.borrow_mut();
        if retained
            .as_ref()
            .is_some_and(|current| !current.is_current(&host, window))
        {
            retained.take();
        }
        let previous = retained
            .as_ref()
            .filter(|current| current.surface == surface && current.owns(control))
            .cloned();
        if previous
            .as_ref()
            .is_some_and(|current| current.activate_on_present)
        {
            return false;
        }
        let origin = previous
            .as_ref()
            .map_or(control, |current| &current.control);
        let target = previous
            .as_ref()
            .map_or(control.id, |current| current.target);
        let slot = if matches!(origin.kind, ControlKind::Tab(_))
            && matches!(
                key,
                ControlAction::Activate
                    | ControlAction::Previous
                    | ControlAction::Next
                    | ControlAction::First
                    | ControlAction::Last
            ) {
            let selected_start = selected_start
                && previous
                    .as_ref()
                    .is_none_or(|current| current.pending.is_none());
            origin
                .tab_keys
                .as_ref()
                .and_then(|keys| keys.slot(target, key, selected_start, &host))
        } else {
            (target == control.id).then(|| control.slot(key)).flatten()
        };
        (previous, slot)
    };
    let Some(slot) = slot else {
        return false;
    };
    let Some(action) = slot.borrow().clone() else {
        return false;
    };
    if !action
        .intent
        .expected_version()
        .is_same_epoch(shared.borrow().session.version())
    {
        return false;
    }
    let source_serial = previous.as_ref().map_or(serial, |current| current.serial);
    let continuing = previous.as_ref().is_some_and(|current| current.submitted);
    let target = action
        .tab_focus
        .map(|(visual, _)| visual)
        .unwrap_or_else(|| {
            previous
                .as_ref()
                .map_or(control.id, |current| current.target)
        });
    if action.yields_content
        && let Some(mut current) = previous.clone().filter(|current| current.submitted)
        && !input::is_presented(
            &shared.borrow(),
            window.window_handle().window_id(),
            surface,
            source_serial,
        )
    {
        // Navigation can reveal a fully clipped member after its update commits.
        // Keep its activation intent until that exact replacement is presented;
        // neither the old scroll witness nor a merely encoded target is input.
        let scope = shared.borrow().mounts[&window.window_handle().window_id()]
            .focus
            .clone();
        interaction.pending_tab_focus.borrow_mut().take();
        window.focus(&scope, cx);
        current.activate_on_present = true;
        current.claim_revision = window.focus_claim_revision();
        current.pending = None;
        *interaction.keyboard.borrow_mut() = Some(current);
        window.refresh();
        return true;
    }
    if matches!(control.kind, ControlKind::Tab(_))
        && !action.yields_content
        && action.tab_focus.is_some()
    {
        if !admits_action(
            &shared.borrow(),
            interaction,
            surface,
            source_serial,
            continuing,
            window,
        ) {
            return false;
        }
        let schedule = previous
            .as_ref()
            .is_none_or(|current| current.pending.is_none());
        *interaction.keyboard.borrow_mut() = Some(Continuation {
            surface,
            serial: source_serial,
            version: shared.borrow().session.version(),
            control: previous.map_or_else(|| control.clone(), |current| current.control),
            target,
            claim_revision: window.focus_claim_revision(),
            pending: Some(slot),
            submitted: continuing,
            activate_on_present: false,
        });
        if schedule {
            let host = Rc::downgrade(shared);
            let interaction = Rc::downgrade(interaction);
            window.defer(cx, move |window, cx| {
                if let Some((host, interaction)) = host.upgrade().zip(interaction.upgrade()) {
                    flush(&host, &interaction, window, cx);
                }
            });
        }
        return true;
    }
    if !activate_action_with_continuation(
        shared,
        interaction,
        surface,
        source_serial,
        &slot,
        continuing,
        window,
        cx,
    ) {
        return false;
    }
    if !action.yields_content
        && matches!(
            control.kind,
            ControlKind::Tab(_) | ControlKind::Splitter(_) | ControlKind::Junction
        )
    {
        *interaction.keyboard.borrow_mut() = Some(Continuation {
            surface,
            serial: source_serial,
            version: shared.borrow().session.version(),
            control: previous.map_or_else(|| control.clone(), |current| current.control),
            target,
            claim_revision: window.focus_claim_revision(),
            pending: None,
            submitted: true,
            activate_on_present: false,
        });
    }
    true
}

/// Coalesce the current framework update's tab prefix before core changes its
/// scroll geometry. Activation submits its final destination synchronously and
/// retires this pending navigation, so later keys already belong to content.
fn flush(shared: &SharedHost, interaction: &InteractionState, window: &mut Window, cx: &mut App) {
    let current = interaction.keyboard.borrow().clone();
    let Some(current) = current else {
        return;
    };
    if !current.is_current(&shared.borrow(), window) {
        interaction.keyboard.borrow_mut().take();
        return;
    }
    let Some(slot) = current.pending else {
        return;
    };
    if activate_action_with_continuation(
        shared,
        interaction,
        current.surface,
        current.serial,
        &slot,
        current.submitted,
        window,
        cx,
    ) {
        if let Some(current) = interaction.keyboard.borrow_mut().as_mut() {
            current.pending = None;
            current.submitted = true;
            current.version = shared.borrow().session.version();
            current.claim_revision = window.focus_claim_revision();
        }
    } else {
        interaction.keyboard.borrow_mut().take();
    }
}

pub(super) fn install(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    window: &mut Window,
    cx: &mut App,
) {
    if interaction.keyboard_subscription.borrow().is_some() {
        return;
    }
    let weak_host = Rc::downgrade(shared);
    let weak_interaction = Rc::downgrade(interaction);
    let owner = window.window_handle().window_id();
    let subscription = cx.intercept_keystrokes(move |event, window, cx| {
        if owner != window.window_handle().window_id() || window.focused(cx).is_some() {
            return;
        }
        let Some((shared, interaction)) = weak_host.upgrade().zip(weak_interaction.upgrade())
        else {
            return;
        };
        let current = interaction.keyboard.borrow().clone();
        let Some(current) = current else {
            return;
        };
        {
            let host = shared.borrow();
            if !host.mounts.get(&owner).is_some_and(|mount| {
                mount.attached
                    && window.is_focus_handle_rendered(&mount.focus)
                    && !mount.capture.is_captured(window)
            }) || !current.is_current(&host, window)
            {
                interaction.keyboard.borrow_mut().take();
                return;
            }
        }
        if handle(
            &shared,
            &interaction,
            current.surface,
            current.serial,
            &current.control,
            &event.keystroke,
            window,
            cx,
        ) {
            cx.stop_propagation();
        }
    });
    *interaction.keyboard_subscription.borrow_mut() = Some(subscription);
}

pub(super) fn restore(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    serial: u64,
    admitted: &[SurfaceId],
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    window: &mut Window,
    cx: &mut App,
) {
    let Some(mut current) = interaction.keyboard.borrow_mut().take() else {
        return;
    };
    if !current.is_current(&shared.borrow(), window) {
        return;
    }
    if !admitted.contains(&current.surface)
        || !input::is_presented(
            &shared.borrow(),
            window.window_handle().window_id(),
            current.surface,
            serial,
        )
    {
        *interaction.keyboard.borrow_mut() = Some(current);
        return;
    }
    let Some((_, control, focus)) = controls.iter().find(|(surface, control, focus)| {
        *surface == current.surface
            && control.id == current.target
            && window.is_focus_handle_rendered(focus)
    }) else {
        return;
    };
    if current.activate_on_present {
        let action = control.tab_keys.as_ref().and_then(|keys| {
            keys.slot(control.id, ControlAction::Activate, false, &shared.borrow())
        });
        if let Some(action) = action {
            let weak = Rc::downgrade(shared);
            let weak_interaction = Rc::downgrade(interaction);
            let surface = current.surface;
            let version = current.version;
            let claim = current.claim_revision;
            // Publish focus intent after this output's focus-stable observers.
            // Creating it inside Normal prepaint could let the same frame
            // acknowledge NotFocused before the new request can be applied.
            window.defer(cx, move |window, cx| {
                let Some((shared, interaction)) = weak.upgrade().zip(weak_interaction.upgrade())
                else {
                    return;
                };
                if shared.borrow().session.version() != version
                    || window.focus_claim_revision() != claim
                {
                    return;
                }
                activate_action(&shared, &interaction, surface, serial, &action, window, cx);
            });
            *interaction.keyboard.borrow_mut() = Some(current);
        }
        return;
    }
    if !focus.is_focused(window) {
        window.focus(focus, cx);
    }
    current.claim_revision = window.focus_claim_revision();
    current.serial = serial;
    current.control = control.clone();
    *interaction.keyboard.borrow_mut() = Some(current);
}
