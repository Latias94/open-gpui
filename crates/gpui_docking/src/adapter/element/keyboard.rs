//! Preserve core-authorized chrome prefixes until input yields to content or an opener.

use std::collections::VecDeque;

use dockspace::{
    model::{DockspaceTabView, DockspaceTabsView, DockspaceView, WorkspaceVersion},
    runtime::{
        RetainedTabListMenu, SurfaceTabInput, SurfaceTabListInput, SurfaceTabListNavigation,
        SurfaceTabNavigation, TabListMenuRowId,
    },
};
use open_gpui::Keystroke;

use super::*;

mod menu_opening;
pub(super) use menu_opening::{MenuOpening, restore as restore_menu_opening};

fn tab_input(key: ControlAction) -> Option<SurfaceTabInput> {
    let navigation = match key {
        ControlAction::Activate => return Some(SurfaceTabInput::Activate),
        ControlAction::Previous => SurfaceTabNavigation::Previous,
        ControlAction::Next => SurfaceTabNavigation::Next,
        ControlAction::First => SurfaceTabNavigation::First,
        ControlAction::Last => SurfaceTabNavigation::Last,
        _ => return None,
    };
    Some(SurfaceTabInput::Navigate(navigation))
}

fn menu_key(keystroke: &Keystroke) -> Option<ControlAction> {
    if keystroke.modifiers != open_gpui::Modifiers::none() {
        return None;
    }
    Some(match keystroke.key.as_str() {
        "up" => ControlAction::Previous,
        "down" => ControlAction::Next,
        "home" => ControlAction::First,
        "end" => ControlAction::Last,
        "enter" | "space" => ControlAction::Activate,
        "escape" => ControlAction::Dismiss,
        _ => return None,
    })
}

fn ends_menu(key: ControlAction) -> bool {
    matches!(key, ControlAction::Activate | ControlAction::Dismiss)
}

fn resolve_tab_inputs(
    control: &Control,
    start: TabKey,
    inputs: &[SurfaceTabInput],
) -> Option<chrome::ActionSlot> {
    let ControlKind::Tab(receiver) = control.kind else {
        return None;
    };
    let deferred = control
        .tab_strip
        .as_ref()?
        .resolve(receiver, start, inputs.iter().copied())?;
    let mut action = chrome::Action::from(deferred);
    action.yields_content = inputs.last() == Some(&SurfaceTabInput::Activate);
    Some(Rc::new(RefCell::new(Some(action))))
}

/// Query direct membership rather than choosing a representative descendant.
/// The result supplies focus information only; original core actions retain authority.
pub(super) fn tab_group(
    view: DockspaceView<'_>,
    surface: SurfaceId,
    key: TabKey,
) -> Option<DockspaceTabsView<'_>> {
    let surface = view.surface(surface)?;
    let mut nodes = surface
        .main_root()
        .into_iter()
        .chain(surface.contained().filter_map(|contained| contained.root()))
        .filter_map(|root| root.content())
        .collect::<Vec<_>>();
    while let Some(node) = nodes.pop() {
        if let Some(tabs) = node.tabs() {
            if tabs.tabs().contains(&key) {
                return Some(tabs);
            }
            nodes.extend(tabs.children().filter_map(|child| match child {
                DockspaceTabView::Layout(layout) => Some(layout),
                DockspaceTabView::Item(_) => None,
            }));
        } else if let Some(split) = node.split() {
            nodes.extend(split.children());
        } else if let Some(grid) = node.grid() {
            nodes.extend(grid.children());
        }
    }
    None
}

pub(super) fn tab_is_selected(view: DockspaceView<'_>, surface: SurfaceId, key: TabKey) -> bool {
    tab_group(view, surface, key).is_some_and(|tabs| tabs.visible_selected() == Some(key))
}

pub(super) struct MenuRow {
    pub(super) id: TabListMenuRowId,
    pub(super) focused: bool,
    pub(super) keys: Rc<MenuKeys>,
}

/// One menu borrows core's immutable input authority across all visible rows.
pub(super) struct MenuKeys {
    pub(super) bar: DockspaceVisualId,
    opener: Option<Rc<Control>>,
    retained: RetainedTabListMenu,
}

impl MenuKeys {
    pub(super) fn new(
        retained: RetainedTabListMenu,
        bar: DockspaceVisualId,
        opener: Option<Rc<Control>>,
    ) -> Self {
        Self {
            bar,
            opener,
            retained,
        }
    }

    pub(super) fn action(
        &self,
        row: TabListMenuRowId,
        end: Option<ControlAction>,
    ) -> Option<chrome::ActionSlot> {
        let input = match end {
            None => SurfaceTabListInput::Focus,
            Some(ControlAction::Activate) => SurfaceTabListInput::Activate,
            Some(ControlAction::Dismiss) => SurfaceTabListInput::Dismiss,
            _ => return None,
        };
        self.resolve(row, input).map(|(_, action)| action)
    }

    fn step(
        &self,
        start: TabListMenuRowId,
        key: ControlAction,
    ) -> Option<(TabListMenuRowId, chrome::ActionSlot)> {
        let navigation = match key {
            ControlAction::Previous => SurfaceTabListNavigation::Previous,
            ControlAction::Next => SurfaceTabListNavigation::Next,
            ControlAction::First => SurfaceTabListNavigation::First,
            ControlAction::Last => SurfaceTabListNavigation::Last,
            ControlAction::Activate | ControlAction::Dismiss => {
                return self.action(start, Some(key)).map(|action| (start, action));
            }
            _ => return None,
        };
        self.resolve(start, SurfaceTabListInput::Navigate(navigation))
    }

    fn resolve(
        &self,
        row: TabListMenuRowId,
        input: SurfaceTabListInput,
    ) -> Option<(TabListMenuRowId, chrome::ActionSlot)> {
        let (target, intent) = self.retained.resolve(row, input)?;
        let mut action = chrome::Action::from(intent);
        action.yields_content = input == SurfaceTabListInput::Activate;
        Some((target, Rc::new(RefCell::new(Some(action)))))
    }
}

pub(super) fn remember_menu_opener(
    interaction: &InteractionState,
    surface: SurfaceId,
    bar: DockspaceVisualId,
    opener: &FocusHandle,
    window: &Window,
) {
    *interaction.tab_list_focus.borrow_mut() = Some(TabListFocus {
        surface,
        bar,
        row: None,
        opener: opener.clone(),
        claim_revision: window.focus_claim_revision(),
        opening: None,
    });
}

/// Keep the framework focus and the core's keyboard row on the same target.
pub(super) fn focus_menu_row(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    control: &Rc<Control>,
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(row) = control.menu_row.as_ref() else {
        return;
    };
    let keys = &row.keys;
    let Some(action) = keys.action(row.id, None) else {
        return;
    };
    if !activate_action(shared, interaction, surface, serial, &action, window, cx) {
        return;
    }
    window.focus(focus, cx);
    let claim_revision = window.focus_claim_revision();
    // The focus action revokes the menu record that delivered it. Preserve the
    // row and the new focus claim until an exact replacement can deliver keys.
    *interaction.keyboard.borrow_mut() = window.is_window_active().then(|| Continuation {
        surface,
        serial,
        version: shared.borrow().session.version(),
        control: control.clone(),
        target: control.id,
        tab_target: None,
        menu_target: Some(row.id),
        claim_revision,
        phase: PrefixPhase::Committed,
        menu_end: None,
        opener: None,
    });
    let opener = keys
        .opener
        .as_ref()
        .and_then(|opener| interaction.focus.borrow().get(&opener.focus_id()).cloned());
    if let Some(opener) = opener {
        *interaction.tab_list_focus.borrow_mut() = Some(TabListFocus {
            surface,
            bar: keys.bar,
            row: Some(row.id),
            opener,
            claim_revision,
            opening: None,
        });
    }
}

#[derive(Clone)]
pub(super) struct Continuation {
    surface: SurfaceId,
    serial: u64,
    version: WorkspaceVersion,
    control: Rc<Control>,
    target: DockspaceVisualId,
    tab_target: Option<TabKey>,
    menu_target: Option<TabListMenuRowId>,
    claim_revision: u64,
    phase: PrefixPhase,
    menu_end: Option<ControlAction>,
    opener: Option<OpenerPrefix>,
}

#[derive(Clone)]
enum PrefixPhase {
    Presented,
    Pending {
        action: chrome::ActionSlot,
        after_commit: bool,
    },
    Committed,
    AwaitingTabPresentation {
        inputs: Vec<SurfaceTabInput>,
    },
}

#[derive(Clone)]
struct OpenerPrefix {
    source: Rc<Control>,
    inputs: VecDeque<chrome::Action>,
    expect_closed: bool,
}

impl Continuation {
    fn is_current(&self, host: &HostState, window: &Window) -> bool {
        self.version == host.session.version() && self.owns_focus_claim(window)
    }

    fn owns_focus_claim(&self, window: &Window) -> bool {
        self.claim_revision == window.focus_claim_revision()
    }

    pub(super) fn owns_menu_focus(&self, window: &Window) -> bool {
        self.owns_focus_claim(window) && (self.opener.is_some() || self.menu_target.is_some())
    }

    fn holds_tab_prefix(&self, window: &Window) -> bool {
        self.tab_target.is_some() && self.owns_focus_claim(window)
    }

    fn committed_tab(&self) -> Option<TabKey> {
        match self.phase {
            PrefixPhase::Committed | PrefixPhase::AwaitingTabPresentation { .. } => self.tab_target,
            _ => None,
        }
    }

    fn pending(&self) -> Option<&chrome::ActionSlot> {
        match &self.phase {
            PrefixPhase::Pending { action, .. } => Some(action),
            _ => None,
        }
    }

    fn has_committed_prefix(&self) -> bool {
        matches!(
            self.phase,
            PrefixPhase::Committed
                | PrefixPhase::Pending {
                    after_commit: true,
                    ..
                }
        )
    }

    fn awaits_menu_presentation(&self) -> bool {
        self.menu_target.is_some() && self.has_committed_prefix()
    }

    fn owns(&self, control: &Control) -> bool {
        match (&self.control.menu_row, &control.menu_row) {
            (Some(left), Some(right)) => Rc::ptr_eq(&left.keys, &right.keys),
            (None, None) => {
                self.control.id == control.id
                    || self.target == control.id
                    || self
                        .control
                        .tab_strip
                        .as_ref()
                        .zip(control.tab_strip.as_ref())
                        .is_some_and(|(left, right)| Rc::ptr_eq(left, right))
            }
            _ => false,
        }
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
                current.has_committed_prefix()
                    && current.surface == surface
                    && current.serial == serial
                    && current.is_current(host, window)
            });
    (shown || continuing) && !host.has_unaccepted_update() && allows_chrome_ingress(host)
}

fn allows_chrome_ingress(host: &HostState) -> bool {
    host.inputs.is_empty()
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
    if !window.is_window_active() {
        return false;
    }
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
    let previous = {
        let host = shared.borrow();
        let mut retained = interaction.keyboard.borrow_mut();
        if let Some(current) = retained
            .as_ref()
            .filter(|current| !current.is_current(&host, window))
        {
            if current.holds_tab_prefix(window)
                && current.surface == surface
                && current.owns(control)
            {
                return matches!(
                    (control.kind, tab_input(key)),
                    (ControlKind::Tab(_), Some(SurfaceTabInput::Activate))
                );
            }
            retained.take();
        }
        retained
            .as_ref()
            .filter(|current| current.surface == surface && current.owns(control))
            .cloned()
    };
    if let Some(current) = previous.as_ref()
        && current.tab_target.is_some()
        && (matches!(current.phase, PrefixPhase::AwaitingTabPresentation { .. })
            || (current.has_committed_prefix()
                && !input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    current.serial,
                )))
    {
        return tab_input(key).is_some_and(|input| {
            await_tab_presentation(shared, interaction, current.clone(), input, window, cx)
        });
    }
    let slot = {
        let host = shared.borrow();
        let origin = previous
            .as_ref()
            .map_or(control, |current| &current.control);
        let target = previous
            .as_ref()
            .map_or(control.id, |current| current.target);
        if let (ControlKind::Tab(receiver), Some(input)) = (origin.kind, tab_input(key)) {
            let selected_start = selected_start
                && previous
                    .as_ref()
                    .is_none_or(|current| current.pending().is_none());
            let start = if selected_start {
                tab_group(host.session.view(), surface, receiver)
                    .and_then(|group| group.visible_selected())
            } else {
                Some(
                    previous
                        .as_ref()
                        .and_then(|current| current.tab_target)
                        .unwrap_or(receiver),
                )
            };
            if input == SurfaceTabInput::Activate
                && let Some(pending) = previous.as_ref().and_then(Continuation::pending)
            {
                pending
                    .borrow()
                    .as_ref()
                    .cloned()
                    .and_then(|action| action.intent.activate_navigated_tab())
                    .map(|intent| {
                        let mut action = chrome::Action::from(intent);
                        action.yields_content = true;
                        Rc::new(RefCell::new(Some(action)))
                    })
            } else {
                start.and_then(|start| resolve_tab_inputs(origin, start, &[input]))
            }
        } else {
            (target == control.id).then(|| control.slot(key)).flatten()
        }
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
    let continuing = previous
        .as_ref()
        .is_some_and(Continuation::has_committed_prefix);
    let target = action
        .tab_focus()
        .map(|(visual, _)| visual)
        .unwrap_or_else(|| {
            previous
                .as_ref()
                .map_or(control.id, |current| current.target)
        });
    if matches!(control.kind, ControlKind::Tab(_))
        && !action.yields_content
        && action.tab_focus().is_some()
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
            .is_none_or(|current| current.pending().is_none());
        *interaction.keyboard.borrow_mut() = Some(Continuation {
            surface,
            serial: source_serial,
            version: shared.borrow().session.version(),
            control: previous.map_or_else(|| control.clone(), |current| current.control),
            target,
            tab_target: action.tab_focus().map(|(_, key)| key),
            menu_target: None,
            claim_revision: window.focus_claim_revision(),
            phase: PrefixPhase::Pending {
                action: slot,
                after_commit: continuing,
            },
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
            tab_target: action.tab_focus().map(|(_, key)| key),
            menu_target: None,
            claim_revision: window.focus_claim_revision(),
            phase: PrefixPhase::Committed,
            menu_end: None,
            opener: None,
        });
    }
    true
}

fn await_tab_presentation(
    shared: &SharedHost,
    interaction: &InteractionState,
    mut current: Continuation,
    input: SurfaceTabInput,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if let PrefixPhase::AwaitingTabPresentation { inputs } = &mut current.phase {
        if inputs.last() == Some(&SurfaceTabInput::Activate) {
            return false;
        }
        inputs.push(input);
    } else {
        // The previous selection can change the scroll witness. Keep the keys
        // until that layout is presented, with the mount owning focus meanwhile.
        let scope = shared.borrow().mounts[&window.window_handle().window_id()]
            .focus
            .clone();
        interaction.pending_tab_focus.borrow_mut().take();
        window.focus(&scope, cx);
        current.claim_revision = window.focus_claim_revision();
        current.phase = PrefixPhase::AwaitingTabPresentation {
            inputs: vec![input],
        };
        window.refresh();
    }
    *interaction.keyboard.borrow_mut() = Some(current);
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
    let Some(key) = menu_key(keystroke) else {
        return false;
    };
    let (previous, keys, start) = {
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
        let Some(row) = origin.menu_row.as_ref() else {
            return false;
        };
        let keys = row.keys.clone();
        let start = previous
            .as_ref()
            .and_then(|current| current.menu_target)
            .unwrap_or(row.id);
        (previous, keys, start)
    };
    let source_serial = previous.as_ref().map_or(serial, |current| current.serial);
    let submitted = previous
        .as_ref()
        .is_some_and(Continuation::has_committed_prefix);
    {
        let host = shared.borrow();
        let owner = window.window_handle().window_id();
        // A completed wheel scroll can leave the old pixels available while
        // the corresponding menu record is stale. Only our own committed
        // prefix can hold keys through that gap and resolve on a later output.
        let ready = host.mounts.get(&owner).is_some_and(|mount| {
            mount.serial == source_serial
                && mount
                    .regions
                    .get(&surface)
                    .is_some_and(|region| !region.dirty)
                && input::is_presented(&host, owner, surface, source_serial)
        });
        // A committed prefix can retain keys while a renderer owns the next
        // candidate. restore_menu resolves them against the presented replay.
        let admitted = if submitted {
            allows_chrome_ingress(&host)
        } else {
            ready && admits_action(&host, interaction, surface, source_serial, false, window)
        };
        if !admitted {
            return false;
        }
    }
    let Some((target, slot)) = keys.step(start, key) else {
        return false;
    };
    let end = ends_menu(key).then_some(key);
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
        .is_none_or(|current| current.pending().is_none());
    let mut current = Continuation {
        surface,
        serial: source_serial,
        version: shared.borrow().session.version(),
        control: previous.map_or_else(|| control.clone(), |current| current.control),
        target: target.visual_id(),
        tab_target: None,
        menu_target: Some(target),
        claim_revision: window.focus_claim_revision(),
        phase: PrefixPhase::Pending {
            action: slot,
            after_commit: submitted,
        },
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
            .menu_row
            .as_ref()
            .and_then(|row| row.keys.opener.as_ref())
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
        current.has_committed_prefix(),
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
    // The adapter's own focus handoff still belongs to this menu. Remember
    // its new claim so discarding its queued keys can leave the menu usable.
    if let Some(previous) = interaction.tab_list_focus.borrow_mut().as_mut()
        && previous.surface == surface
        && previous.bar == bar
    {
        previous.claim_revision = window.focus_claim_revision();
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
    } else {
        interaction.tab_list_focus.borrow_mut().take();
    }
}

/// Coalesce the current framework update's tab prefix before core changes its
/// scroll geometry. Activation submits its final destination synchronously and
/// retires this pending navigation, so later keys already belong to content.
fn flush(shared: &SharedHost, interaction: &InteractionState, window: &mut Window, cx: &mut App) {
    if !window.is_window_active() {
        return;
    }
    let current = interaction.keyboard.borrow().clone();
    let Some(current) = current else {
        return;
    };
    if !current.is_current(&shared.borrow(), window) {
        if !current.holds_tab_prefix(window) {
            interaction.keyboard.borrow_mut().take();
        }
        return;
    }
    if current.awaits_menu_presentation() {
        // Its original menu record became stale when the preceding focus move
        // committed. restore() will rebind this intent only after presentation.
        return;
    }
    let Some(slot) = current.pending() else {
        return;
    };
    if activate_action_with_continuation(
        shared,
        interaction,
        current.surface,
        current.serial,
        slot,
        current.has_committed_prefix(),
        window,
        cx,
    ) {
        if let Some(end) = current.menu_end
            && let Some(row) = current.control.menu_row.as_ref()
        {
            finish_menu(
                shared,
                interaction,
                current.surface,
                row.keys.bar,
                end,
                window,
                cx,
            );
            if let Some(opener) = current.opener.as_ref() {
                let mut current = current.clone();
                current.control = opener.source.clone();
                current.target = opener.source.id;
                current.menu_target = None;
                current.menu_end = None;
                current.phase = PrefixPhase::Committed;
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
                if let Some(focus) = interaction.tab_list_focus.borrow_mut().as_mut() {
                    focus.opening = None;
                }
                let mut current = current;
                current.opener = Some(opener);
                current.phase = PrefixPhase::Committed;
                current.version = shared.borrow().session.version();
                current.claim_revision = window.focus_claim_revision();
                *interaction.keyboard.borrow_mut() = Some(current);
            }
            return;
        }
        if let Some(current) = interaction.keyboard.borrow_mut().as_mut() {
            current.phase = PrefixPhase::Committed;
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
    let activation = window.observe_window_activation(move |window, _| {
        if !window.is_window_active()
            && let Some((shared, interaction)) = weak_host.upgrade().zip(weak_interaction.upgrade())
        {
            super::presentation_menu::revoke(&shared, &interaction, window);
            if let Some(focus) = interaction.tab_list_focus.borrow_mut().as_mut() {
                focus.opening = None;
            }
            let mut retained = interaction.keyboard.borrow_mut();
            if let Some(current) = retained
                .as_mut()
                .filter(|current| current.committed_tab().is_some())
            {
                current.phase = PrefixPhase::Committed;
            } else {
                retained.take();
            }
        }
    });
    let weak_host = Rc::downgrade(shared);
    let weak_interaction = Rc::downgrade(interaction);
    let owner = window.window_handle().window_id();
    let subscription = cx.intercept_keystrokes(move |event, window, cx| {
        if owner != window.window_handle().window_id() || !window.is_window_active() {
            return;
        }
        let Some((shared, interaction)) = weak_host.upgrade().zip(weak_interaction.upgrade())
        else {
            return;
        };
        if super::presentation_menu::intercept(&shared, &interaction, &event.keystroke, window, cx)
            || menu_opening::retain_key(&shared, &interaction, &event.keystroke, window, cx)
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
                    .then(|| current.control.menu_row.as_ref()?.keys.opener.clone())
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
            {
                interaction.keyboard.borrow_mut().take();
                return;
            }
            if !current.is_current(&host, window) {
                if current.holds_tab_prefix(window) {
                    // The old header can still receive input until the new
                    // layout is presented. Enter belongs to its stale prefix.
                    if event.keystroke.modifiers == open_gpui::Modifiers::none()
                        && current.control.key_action(&event.keystroke.key)
                            == Some(ControlAction::Activate)
                    {
                        cx.stop_propagation();
                    }
                } else {
                    interaction.keyboard.borrow_mut().take();
                }
                return;
            }
            // Dismissal may return focus to this exact mount while its opener
            // is inert. Only the still-current terminal/opener prefix can own
            // semantic input there; any later application claim already won.
            let owns_scope = mount.focus.is_focused(window)
                && (opener.is_some()
                    || current.holds_tab_prefix(window)
                    || current.awaits_menu_presentation());
            if window.focused(cx).is_some() && !owns_scope {
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
    *interaction.keyboard_subscription.borrow_mut() =
        Some(open_gpui::Subscription::join(subscription, activation));
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
    if !window.is_window_active() {
        return;
    }
    let Some(mut current) = interaction.keyboard.borrow_mut().take() else {
        return;
    };
    let presented = admitted.contains(&current.surface)
        && input::is_presented(
            &shared.borrow(),
            window.window_handle().window_id(),
            current.surface,
            serial,
        );
    if !current.is_current(&shared.borrow(), window) {
        if !current.holds_tab_prefix(window) {
            return;
        }
        if !presented || serial == current.serial {
            *interaction.keyboard.borrow_mut() = Some(current);
            return;
        }
        let host = shared.borrow();
        let version = host.session.version();
        if !current.version.is_same_epoch(version)
            || !current
                .committed_tab()
                .is_some_and(|tab| tab_is_selected(host.session.view(), current.surface, tab))
        {
            return;
        }
        current.phase = PrefixPhase::Committed;
    }
    if !presented {
        *interaction.keyboard.borrow_mut() = Some(current);
        return;
    }
    if current.opener.is_some() && current.menu_end.is_none() {
        restore_opener(shared, interaction, current, serial, controls, window, cx);
        return;
    }
    if current.menu_target.is_some() {
        restore_menu(shared, interaction, current, serial, controls, window, cx);
        return;
    }
    if matches!(current.phase, PrefixPhase::AwaitingTabPresentation { .. }) {
        if serial == current.serial {
            *interaction.keyboard.borrow_mut() = Some(current);
            return;
        }
        let Some(original) = current.control.tab_strip.as_ref() else {
            return;
        };
        let key = current.tab_target.and_then(|target| {
            let strip = controls.iter().find_map(|(surface, control, _)| {
                (*surface == current.surface)
                    .then_some(control.tab_strip.as_deref())
                    .flatten()
                    .filter(|strip| strip.visual_id() == original.visual_id())
            })?;
            strip.continuation_receiver(original, target)
        });
        let receiver = controls
            .iter()
            .find(|(surface, control, _)| {
                *surface == current.surface
                    && control.receiver.is_some()
                    && matches!(control.kind, ControlKind::Tab(tab) if Some(tab) == key)
            })
            .map(|(_, control, _)| control.clone());
        if let Some(receiver) = receiver {
            let weak = Rc::downgrade(shared);
            let weak_interaction = Rc::downgrade(interaction);
            let origin = (current.surface, current.serial, current.claim_revision);
            let header = controls
                .iter()
                .find(|(surface, control, focus)| {
                    *surface == current.surface
                        && control.id == current.target
                        && window.is_focus_handle_rendered(focus)
                })
                .map(|(_, _, focus)| focus.clone());
            *interaction.keyboard.borrow_mut() = Some(current);
            // Publish the focus request after this output's focus observers.
            window.defer(cx, move |window, cx| {
                let Some((shared, interaction)) = weak.upgrade().zip(weak_interaction.upgrade())
                else {
                    return;
                };
                resume_tab_input(
                    &shared,
                    &interaction,
                    origin,
                    serial,
                    receiver,
                    header,
                    window,
                    cx,
                );
            });
            return;
        }
        // The committed selection still owns header focus. Only the queued keys
        // lose authority when the replacement strip is outside their scope.
        current.phase = PrefixPhase::Committed;
    }
    let Some((_, control, focus)) = controls.iter().find(|(surface, control, focus)| {
        *surface == current.surface
            && control.id == current.target
            && window.is_focus_handle_rendered(focus)
    }) else {
        return;
    };
    if !focus.is_focused(window) {
        window.focus(focus, cx);
    }
    if current.tab_target.is_some() && matches!(current.phase, PrefixPhase::Committed) {
        interaction.pending_tab_focus.borrow_mut().take();
        return;
    }
    current.claim_revision = window.focus_claim_revision();
    current.serial = serial;
    current.control = control.clone();
    *interaction.keyboard.borrow_mut() = Some(current);
}

fn resume_tab_input(
    shared: &SharedHost,
    interaction: &InteractionState,
    origin: (SurfaceId, u64, u64),
    serial: u64,
    receiver: Rc<Control>,
    header: Option<FocusHandle>,
    window: &mut Window,
    cx: &mut App,
) {
    if !input::is_presented(
        &shared.borrow(),
        window.window_handle().window_id(),
        origin.0,
        serial,
    ) {
        return;
    }
    let mut current = {
        let mut retained = interaction.keyboard.borrow_mut();
        if !retained.as_ref().is_some_and(|current| {
            (current.surface, current.serial, current.claim_revision) == origin
                && matches!(current.phase, PrefixPhase::AwaitingTabPresentation { .. })
        }) {
            return;
        }
        retained.take().expect("the queued tab inputs were checked")
    };
    if !current.is_current(&shared.borrow(), window) {
        return;
    }
    let PrefixPhase::AwaitingTabPresentation { inputs } = current.phase else {
        return;
    };
    let Some(start) = current.tab_target else {
        return;
    };
    let Some(action) = resolve_tab_inputs(&receiver, start, &inputs) else {
        if let Some(header) = header.filter(|focus| window.is_focus_handle_rendered(focus)) {
            window.focus(&header, cx);
        }
        return;
    };
    let target = action
        .borrow()
        .as_ref()
        .filter(|action| !action.yields_content)
        .and_then(|action| action.tab_focus());
    if activate_action(
        shared,
        interaction,
        current.surface,
        serial,
        &action,
        window,
        cx,
    ) && let Some((visual, key)) = target
    {
        current.serial = serial;
        current.control = receiver;
        current.target = visual;
        current.tab_target = Some(key);
        current.version = shared.borrow().session.version();
        current.claim_revision = window.focus_claim_revision();
        current.phase = PrefixPhase::Committed;
        *interaction.keyboard.borrow_mut() = Some(current);
    }
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
    let focused_row = controls.iter().find_map(|(surface, control, _)| {
        let row = control.menu_row.as_ref()?;
        (*surface == current.surface && row.focused && row.keys.bar == bar).then_some(row.id)
    });
    *interaction.tab_list_focus.borrow_mut() = focused_row.map(|row| TabListFocus {
        surface: current.surface,
        bar,
        row: Some(row),
        opener: focus.clone(),
        claim_revision: window.focus_claim_revision(),
        opening: None,
    });
    current.control = control.clone();
    current.serial = serial;
    current.claim_revision = window.focus_claim_revision();
    current.phase = PrefixPhase::Pending {
        action: slot,
        after_commit: false,
    };
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
    let Some(original) = current.control.menu_row.as_ref() else {
        return;
    };
    let Some(target) = current.menu_target else {
        return;
    };
    if serial == current.serial {
        *interaction.keyboard.borrow_mut() = Some(current);
        return;
    }
    let Some((_, control, focus)) = controls.iter().find(|(surface, control, focus)| {
        *surface == current.surface
            && control.menu_row.as_ref().is_some_and(|row| {
                row.focused && row.keys.retained.can_continue_from(&original.keys.retained)
            })
            && window.is_focus_handle_rendered(focus)
    }) else {
        return;
    };
    let Some(row) = control.menu_row.as_ref() else {
        return;
    };
    let keys = &row.keys;
    if current.pending().is_some() {
        let Some(action) = keys.action(target, current.menu_end) else {
            return;
        };
        current.phase = PrefixPhase::Pending {
            action,
            after_commit: false,
        };
    } else if row.id != target {
        // A different core focus owner has superseded this completed prefix.
        return;
    }
    current.control = control.clone();
    current.serial = serial;
    if current.pending().is_none() {
        current.phase = PrefixPhase::Presented;
    }
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
                row: Some(row.id),
                opener,
                claim_revision: current.claim_revision,
                opening: None,
            });
        }
    }
    let pending = current.pending().is_some();
    *interaction.keyboard.borrow_mut() = Some(current);
    if pending {
        // Keep terminal pane-focus requests outside Normal prepaint, after
        // this output's focus-stable observers have seen the earlier state.
        defer_flush(shared, interaction, window, cx);
    }
}
