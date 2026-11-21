//! Preserve input after the list opener acts and before core presents its first row.

use dockspace::runtime::RetainedTabListOpener;

use super::*;

/// Keys following a committed or enqueued open wait for the first matching menu.
pub(in crate::adapter::element) struct MenuOpening {
    serial: u64,
    proof: RetainedTabListOpener,
    version: WorkspaceVersion,
    keys: Vec<ControlAction>,
    product_fallback: Option<presentation_menu::OpenMenu>,
}

impl MenuOpening {
    pub(in crate::adapter::element) fn new(
        proof: RetainedTabListOpener,
        version: WorkspaceVersion,
        serial: u64,
        product_fallback: Option<presentation_menu::OpenMenu>,
    ) -> Self {
        Self {
            serial,
            proof,
            version,
            keys: Vec::new(),
            product_fallback,
        }
    }

    fn is_current(&self, host: &HostState) -> bool {
        self.version == host.session.version()
    }

    pub(in crate::adapter::element) fn into_witness(
        self,
    ) -> (WorkspaceVersion, RetainedTabListOpener) {
        (self.version, self.proof)
    }
}

pub(super) fn retain_key(
    shared: &SharedHost,
    interaction: &InteractionState,
    keystroke: &Keystroke,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(key) = menu_key(keystroke) else {
        return false;
    };
    let host = shared.borrow();
    let Some(mount) = host.mounts.get(&window.window_handle().window_id()) else {
        return false;
    };
    let mut focus = interaction.tab_list_focus.borrow_mut();
    let Some(focus) = focus.as_mut() else {
        return false;
    };
    let Some(opening) = focus.opening.as_mut() else {
        return false;
    };
    if !mount.attached
        || mount.capture.is_captured(window)
        || !window.is_focus_handle_rendered(&mount.focus)
        || focus.claim_revision != window.focus_claim_revision()
        || !opening.is_current(&host)
        || host.ensure_application_admission().is_err()
    {
        if opening.product_fallback.is_none() {
            focus.opening = None;
        }
        return false;
    }
    if window.focused(cx).is_some() && !focus.opener.is_focused(window)
        || opening.keys.last().is_some_and(|key| ends_menu(*key))
    {
        return false;
    }
    opening.keys.push(key);
    window.refresh();
    true
}

pub(in crate::adapter::element) fn restore(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    serial: u64,
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    presented: &impl Fn(SurfaceId) -> bool,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let (opening, surface, bar, owns_focus) = {
        let mut focus = interaction.tab_list_focus.borrow_mut();
        let Some(focus) = focus.as_mut() else {
            return false;
        };
        let Some(opening) = focus.opening.as_ref() else {
            return false;
        };
        let owns_focus = focus.claim_revision == window.focus_claim_revision();
        let host = shared.borrow();
        if !opening.is_current(&host)
            || host.session.view().surface(focus.surface).is_none()
            || (!owns_focus && opening.product_fallback.is_none())
        {
            focus.opening = None;
            return false;
        }
        drop(host);
        if serial == opening.serial || !presented(focus.surface) {
            return true;
        }
        (
            focus.opening.take().expect("pending menu opening"),
            focus.surface,
            focus.bar,
            owns_focus,
        )
    };
    let shown = controls.iter().find(|(owner, control, focus_handle)| {
        *owner == surface
            && window.is_focus_handle_rendered(focus_handle)
            && control.menu_row.as_ref().is_some_and(|row| {
                row.focused
                    && row.keys.bar == bar
                    && row.keys.retained.can_continue_opening(&opening.proof)
            })
    });
    if let Some(previous) = opening.product_fallback
        && presentation_menu::open(interaction) == Some(previous)
    {
        if shown.is_some() {
            presentation_menu::revoke(shared, interaction, window);
        } else {
            interaction.tab_list_focus.borrow_mut().take();
            if owns_focus {
                presentation_menu::restore_rejected_handoff(shared, interaction, window, cx);
            }
        }
    }
    let Some((_, control, row_focus)) = shown.filter(|_| owns_focus) else {
        return false;
    };
    let Some(row) = control.menu_row.as_ref() else {
        return false;
    };
    let mut target = row.id;
    let mut action = None;
    let mut end = None;
    for key in opening.keys {
        let Some((next, slot)) = row.keys.step(target, key) else {
            return false;
        };
        target = next;
        action = Some(slot);
        end = ends_menu(key).then_some(key);
    }
    let Some(action) = action else {
        return false;
    };
    if let Some(end) = end {
        yield_menu_prefix(shared, interaction, surface, bar, end, window, cx);
    } else {
        window.focus(row_focus, cx);
    }
    let claim_revision = window.focus_claim_revision();
    if let Some(focus) = interaction.tab_list_focus.borrow_mut().as_mut() {
        focus.row = Some(row.id);
        focus.claim_revision = claim_revision;
    }
    *interaction.keyboard.borrow_mut() = Some(Continuation {
        surface,
        serial,
        version: opening.version,
        control: control.clone(),
        target: target.visual_id(),
        tab_target: None,
        menu_target: Some(target),
        claim_revision,
        phase: PrefixPhase::Pending {
            action,
            after_commit: false,
        },
        menu_end: end,
        opener: None,
    });
    defer_flush(shared, interaction, window, cx);
    true
}
