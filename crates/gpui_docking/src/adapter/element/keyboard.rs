//! Preserve core-authorized chrome prefixes until input yields to content or an opener.

use std::collections::VecDeque;

use dockspace::{
    model::WorkspaceVersion,
    runtime::{
        SurfacePaintPlan, SurfaceTabInput, SurfaceTabNavigation, TabBarPaintRecord,
        TabListMenuPaintRecord,
    },
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

/// One menu snapshot retains its ordered rows and exact prepared capabilities.
/// The cursor only folds a keyboard prefix; core still owns focus, scrolling,
/// permissions and the lifetime of the popup session.
pub(super) struct MenuKeys {
    id: DockspaceVisualId,
    bar: DockspaceVisualId,
    opener: Option<Rc<Control>>,
    rows: Vec<MenuMember>,
    dismiss: chrome::ActionSlot,
}

struct MenuMember {
    visual: DockspaceVisualId,
    focus: chrome::ActionSlot,
    activate: chrome::ActionSlot,
}

impl MenuKeys {
    pub(super) fn record(
        plan: SurfacePaintPlan<'_>,
        menu: TabListMenuPaintRecord<'_>,
        opener: Option<Rc<Control>>,
    ) -> Option<Self> {
        let mut rows = Vec::new();
        for row in menu.rows() {
            let focus = chrome::Action::from(plan.prepare_tab_list_menu_focus(menu, row.item())?);
            let mut activate =
                chrome::Action::from(plan.prepare_tab_list_menu_row_activation(row)?);
            activate.yields_content = true;
            rows.push(MenuMember {
                visual: row.visual_id(),
                focus: Rc::new(RefCell::new(Some(focus))),
                activate: Rc::new(RefCell::new(Some(activate))),
            });
        }
        Some(Self {
            id: menu.visual_id(),
            bar: menu.tab_bar_visual_id(),
            opener,
            rows,
            dismiss: Rc::new(RefCell::new(Some(chrome::Action::from(
                plan.prepare_tab_list_menu_dismiss(menu.backdrop())?,
            )))),
        })
    }

    fn action(
        &self,
        target: DockspaceVisualId,
        end: Option<ControlAction>,
    ) -> Option<chrome::ActionSlot> {
        let row = self.rows.iter().find(|row| row.visual == target)?;
        match end {
            None => Some(row.focus.clone()),
            Some(ControlAction::Activate) => Some(row.activate.clone()),
            Some(ControlAction::Dismiss) => Some(self.dismiss.clone()),
            _ => None,
        }
    }

    fn step(
        &self,
        start: DockspaceVisualId,
        key: ControlAction,
    ) -> Option<(DockspaceVisualId, chrome::ActionSlot)> {
        let cursor = self.rows.iter().position(|row| row.visual == start)?;
        let last = self.rows.len().checked_sub(1)?;
        let target = match key {
            ControlAction::Previous => cursor.saturating_sub(1),
            ControlAction::Next => cursor.saturating_add(1).min(last),
            ControlAction::First => 0,
            ControlAction::Last => last,
            ControlAction::Activate | ControlAction::Dismiss => cursor,
            _ => return None,
        };
        let target = self.rows[target].visual;
        let end = matches!(key, ControlAction::Activate | ControlAction::Dismiss).then_some(key);
        Some((target, self.action(target, end)?))
    }

    fn same_roster(&self, other: &Self) -> bool {
        self.id == other.id
            && self
                .rows
                .iter()
                .map(|row| row.visual)
                .eq(other.rows.iter().map(|row| row.visual))
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
    menu_end: Option<ControlAction>,
    opener: Option<OpenerPrefix>,
}

#[derive(Clone)]
struct OpenerPrefix {
    source: Rc<Control>,
    inputs: VecDeque<chrome::Action>,
    expect_closed: bool,
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
            || self
                .control
                .menu_keys
                .as_ref()
                .zip(control.menu_keys.as_ref())
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
    if matches!(control.kind, ControlKind::MenuRow) {
        return handle_menu(
            shared,
            interaction,
            surface,
            serial,
            control,
            keystroke,
            window,
            cx,
        );
    }
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
    if retain_opener_input(shared, interaction, surface, control, key, window) {
        return true;
    }
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
            menu_end: None,
            opener: None,
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
            menu_end: None,
            opener: None,
        });
    }
    true
}

fn handle_menu(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    surface: SurfaceId,
    serial: u64,
    control: &Rc<Control>,
    keystroke: &Keystroke,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if keystroke.modifiers != open_gpui::Modifiers::none() {
        return false;
    }
    let Some(key) = control.key_action(&keystroke.key) else {
        return false;
    };
    let (previous, keys, target, slot) = {
        let host = shared.borrow();
        let mut retained = interaction.keyboard.borrow_mut();
        if retained
            .as_ref()
            .is_some_and(|current| !current.is_current(&host, window))
        {
            retained.take();
            return false;
        }
        let previous = retained
            .as_ref()
            .filter(|current| current.surface == surface && current.owns(control))
            .cloned();
        if previous
            .as_ref()
            .is_some_and(|current| current.menu_end.is_some())
        {
            return false;
        }
        let origin = previous
            .as_ref()
            .map_or(control, |current| &current.control);
        let Some(keys) = origin.menu_keys.clone() else {
            return false;
        };
        let start = previous
            .as_ref()
            .map_or(control.id, |current| current.target);
        let Some((target, slot)) = keys.step(start, key) else {
            return false;
        };
        (previous, keys, target, slot)
    };
    let source_serial = previous.as_ref().map_or(serial, |current| current.serial);
    let submitted = previous.as_ref().is_some_and(|current| current.submitted);
    if !admits_action(
        &shared.borrow(),
        interaction,
        surface,
        source_serial,
        submitted,
        window,
    ) {
        return false;
    }
    let end = matches!(key, ControlAction::Activate | ControlAction::Dismiss).then_some(key);
    if end.is_some() && !submitted {
        if !activate_action_with_continuation(
            shared,
            interaction,
            surface,
            source_serial,
            &slot,
            false,
            window,
            cx,
        ) {
            return false;
        }
        finish_menu(shared, interaction, surface, keys.bar, key, window, cx);
        return true;
    }
    let schedule = previous
        .as_ref()
        .is_none_or(|current| current.pending.is_none());
    let mut current = Continuation {
        surface,
        serial: source_serial,
        version: shared.borrow().session.version(),
        control: previous.map_or_else(|| control.clone(), |current| current.control),
        target,
        claim_revision: window.focus_claim_revision(),
        pending: Some(slot),
        submitted,
        activate_on_present: false,
        menu_end: end,
        opener: None,
    };
    if let Some(end) = end {
        // A committed focus move changes the menu's frozen focus/scroll record.
        // End the prefix now, then use the next actually presented snapshot for
        // its terminal command. Encoded replacement rows supply no authority.
        yield_menu_prefix(shared, interaction, surface, keys.bar, end, window, cx);
        current.claim_revision = window.focus_claim_revision();
    }
    *interaction.keyboard.borrow_mut() = Some(current);
    if schedule {
        defer_flush(shared, interaction, window, cx);
    }
    if submitted {
        window.refresh();
    }
    true
}

/// Only the returned opener's semantic commands wait behind menu dismissal.
/// Every event retains its original capability; unrelated keys still propagate.
fn retain_opener_input(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    control: &Rc<Control>,
    key: ControlAction,
    window: &Window,
) -> bool {
    if key != ControlAction::Activate || control.opens_menu.is_none() {
        return false;
    }
    let Some(mut current) = interaction.keyboard.borrow().clone() else {
        return false;
    };
    let source = if let Some(opener) = current.opener.as_ref() {
        &opener.source
    } else if current.menu_end == Some(ControlAction::Dismiss)
        && let Some(source) = current
            .control
            .menu_keys
            .as_ref()
            .and_then(|keys| keys.opener.as_ref())
    {
        source
    } else {
        return false;
    };
    if current.surface != surface
        || source.id != control.id
        || source.opens_menu != control.opens_menu
    {
        return false;
    }
    let host = shared.borrow();
    if !current.is_current(&host, window) {
        interaction.keyboard.borrow_mut().take();
        return false;
    }
    if !admits_action(
        &host,
        interaction,
        surface,
        current.serial,
        current.submitted,
        window,
    ) {
        return false;
    }
    let Some(input) = control.slot(key).and_then(|slot| slot.borrow().clone()) else {
        return false;
    };
    let same_source = source.slot(key).is_some_and(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|action| action.intent.same_intent(&input.intent))
    });
    if !same_source
        || !input
            .intent
            .expected_version()
            .is_same_epoch(host.session.version())
    {
        return false;
    }
    let source = source.clone();
    current
        .opener
        .get_or_insert_with(|| OpenerPrefix {
            source,
            inputs: VecDeque::new(),
            expect_closed: true,
        })
        .inputs
        .push_back(input);
    *interaction.keyboard.borrow_mut() = Some(current);
    true
}

fn defer_flush(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    window: &Window,
    cx: &mut App,
) {
    let host = Rc::downgrade(shared);
    let interaction = Rc::downgrade(interaction);
    window.defer(cx, move |window, cx| {
        if let Some((host, interaction)) = host.upgrade().zip(interaction.upgrade()) {
            flush(&host, &interaction, window, cx);
        }
    });
}

fn yield_menu_prefix(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    bar: DockspaceVisualId,
    end: ControlAction,
    window: &mut Window,
    cx: &mut App,
) {
    let opener = (end == ControlAction::Dismiss)
        .then(|| {
            interaction
                .tab_list_focus
                .borrow()
                .as_ref()
                .filter(|previous| previous.surface == surface && previous.bar == bar)
                .map(|previous| previous.opener.clone())
        })
        .flatten()
        .filter(|focus| window.is_focus_handle_rendered(focus));
    let focus = opener.or_else(|| {
        shared
            .borrow()
            .mounts
            .get(&window.window_handle().window_id())
            .map(|mount| mount.focus.clone())
            .filter(|focus| window.is_focus_handle_rendered(focus))
    });
    if let Some(focus) = focus
        && !focus.is_focused(window)
    {
        window.focus(&focus, cx);
    }
}

fn finish_menu(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    bar: DockspaceVisualId,
    end: ControlAction,
    window: &mut Window,
    cx: &mut App,
) {
    interaction.keyboard.borrow_mut().take();
    interaction.pending_tab_focus.borrow_mut().take();
    if end == ControlAction::Dismiss {
        yield_menu_prefix(shared, interaction, surface, bar, end, window, cx);
        // Keep the opener's claim through the intervening inert replay. The
        // existing presented-menu settlement restores it once the closed
        // popup's replacement is interactive, unless application focus wins.
        if let Some(previous) = interaction.tab_list_focus.borrow_mut().as_mut()
            && previous.surface == surface
            && previous.bar == bar
        {
            previous.claim_revision = window.focus_claim_revision();
        }
    } else {
        interaction.tab_list_focus.borrow_mut().take();
    }
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
    if current.control.menu_keys.is_some() && current.submitted {
        // Its original menu record became stale when the preceding focus move
        // committed. restore() will rebind this intent only after presentation.
        return;
    }
    let Some(slot) = current.pending.clone() else {
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
        if let Some(end) = current.menu_end
            && let Some(keys) = current.control.menu_keys.as_ref()
        {
            finish_menu(
                shared,
                interaction,
                current.surface,
                keys.bar,
                end,
                window,
                cx,
            );
            if let Some(opener) = current.opener.as_ref() {
                let mut current = current.clone();
                current.control = opener.source.clone();
                current.target = opener.source.id;
                current.pending = None;
                current.menu_end = None;
                current.submitted = true;
                current.version = shared.borrow().session.version();
                current.claim_revision = window.focus_claim_revision();
                // The continuation now owns restoring this exact opener.
                // Generic closed-menu settlement must not renew its claim.
                interaction.tab_list_focus.borrow_mut().take();
                *interaction.keyboard.borrow_mut() = Some(current);
            }
            return;
        }
        if let Some(mut opener) = current.opener.clone() {
            opener.inputs.pop_front();
            opener.expect_closed = false;
            if opener.inputs.is_empty() {
                interaction.keyboard.borrow_mut().take();
            } else {
                let mut current = current;
                current.opener = Some(opener);
                current.pending = None;
                current.submitted = true;
                current.version = shared.borrow().session.version();
                current.claim_revision = window.focus_claim_revision();
                *interaction.keyboard.borrow_mut() = Some(current);
            }
            return;
        }
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
        if owner != window.window_handle().window_id() {
            return;
        }
        let Some((shared, interaction)) = weak_host.upgrade().zip(weak_interaction.upgrade())
        else {
            return;
        };
        if super::presentation_menu::intercept(&shared, &interaction, &event.keystroke, window, cx)
        {
            cx.stop_propagation();
            return;
        }
        let current = interaction.keyboard.borrow().clone();
        let Some(current) = current else {
            return;
        };
        let opener = current
            .opener
            .as_ref()
            .map(|opener| opener.source.clone())
            .or_else(|| {
                (current.menu_end == Some(ControlAction::Dismiss))
                    .then(|| current.control.menu_keys.as_ref()?.opener.clone())
                    .flatten()
            });
        {
            let host = shared.borrow();
            let Some(mount) = host.mounts.get(&owner) else {
                interaction.keyboard.borrow_mut().take();
                return;
            };
            if !mount.attached
                || !window.is_focus_handle_rendered(&mount.focus)
                || mount.capture.is_captured(window)
                || !current.is_current(&host, window)
            {
                interaction.keyboard.borrow_mut().take();
                return;
            }
            // Dismissal may return focus to this exact mount while its opener
            // is inert. Only the still-current terminal/opener prefix can own
            // semantic input there; any later application claim already won.
            if window.focused(cx).is_some() && !(opener.is_some() && mount.focus.is_focused(window))
            {
                return;
            }
        }
        let control = opener.unwrap_or_else(|| current.control.clone());
        if handle(
            &shared,
            &interaction,
            current.surface,
            current.serial,
            &control,
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
    if current.opener.is_some() && current.menu_end.is_none() {
        restore_opener(shared, interaction, current, serial, controls, window, cx);
        return;
    }
    if current.control.menu_keys.is_some() {
        restore_menu(shared, interaction, current, serial, controls, window, cx);
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

fn restore_opener(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    mut current: Continuation,
    serial: u64,
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    window: &mut Window,
    cx: &mut App,
) {
    if serial == current.serial {
        *interaction.keyboard.borrow_mut() = Some(current);
        return;
    }
    let Some(opener) = current.opener.as_ref() else {
        return;
    };
    let Some((_, control, focus)) = controls.iter().find(|(surface, control, focus)| {
        *surface == current.surface
            && control.id == opener.source.id
            && control.opens_menu == opener.source.opens_menu
            && (!opener.expect_closed || control.expanded == Some(false))
            && window.is_focus_handle_rendered(focus)
    }) else {
        return;
    };
    let Some(slot) = control.slot(ControlAction::Activate) else {
        return;
    };
    let Some(bar) = control.opens_menu else {
        return;
    };
    if !opener.inputs.front().is_some_and(|original| {
        slot.borrow()
            .as_ref()
            .is_some_and(|action| original.intent.same_intent(&action.intent))
    }) {
        return;
    }
    if !focus.is_focused(window) {
        window.focus(focus, cx);
    }
    let focused_row = controls.iter().find(|(surface, row, _)| {
        *surface == current.surface && row.focused && row.menu_owner == Some(bar)
    });
    *interaction.tab_list_focus.borrow_mut() = focused_row.map(|(_, row, _)| TabListFocus {
        surface: current.surface,
        bar,
        row: row.id,
        opener: focus.clone(),
        claim_revision: window.focus_claim_revision(),
    });
    current.control = control.clone();
    current.serial = serial;
    current.claim_revision = window.focus_claim_revision();
    current.pending = Some(slot);
    current.submitted = false;
    *interaction.keyboard.borrow_mut() = Some(current);
    defer_flush(shared, interaction, window, cx);
}

fn restore_menu(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    mut current: Continuation,
    serial: u64,
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    window: &mut Window,
    cx: &mut App,
) {
    let Some(original) = current.control.menu_keys.as_ref() else {
        return;
    };
    if serial == current.serial {
        *interaction.keyboard.borrow_mut() = Some(current);
        return;
    }
    let Some((_, control, focus)) = controls.iter().find(|(surface, control, focus)| {
        *surface == current.surface
            && control.focused
            && control
                .menu_keys
                .as_ref()
                .is_some_and(|keys| original.same_roster(keys))
            && window.is_focus_handle_rendered(focus)
    }) else {
        return;
    };
    let Some(keys) = control.menu_keys.as_ref() else {
        return;
    };
    if current.pending.is_some() {
        current.pending = keys.action(current.target, current.menu_end);
        if current.pending.is_none() {
            return;
        }
    } else if control.id != current.target {
        // A different core focus owner has superseded this completed prefix.
        return;
    }
    current.control = control.clone();
    current.serial = serial;
    current.submitted = false;
    if current.menu_end.is_none() {
        if !focus.is_focused(window) {
            window.focus(focus, cx);
        }
        current.claim_revision = window.focus_claim_revision();
        let opener = controls
            .iter()
            .find(|(surface, control, _)| {
                *surface == current.surface && control.opens_menu == Some(keys.bar)
            })
            .map(|(_, _, focus)| focus.clone());
        if let Some(opener) = opener {
            *interaction.tab_list_focus.borrow_mut() = Some(TabListFocus {
                surface: current.surface,
                bar: keys.bar,
                row: control.id,
                opener,
                claim_revision: current.claim_revision,
            });
        }
    }
    let pending = current.pending.is_some();
    *interaction.keyboard.borrow_mut() = Some(current);
    if pending {
        // Keep terminal pane-focus requests outside Normal prepaint, after
        // this output's focus-stable observers have seen the earlier state.
        defer_flush(shared, interaction, window, cx);
    }
}
