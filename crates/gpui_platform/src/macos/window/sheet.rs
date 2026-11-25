//! Native modal presentation for dialogs with an explicit transient owner.

use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SheetPhase {
    #[default]
    Prepared,
    Presenting,
    Committed,
    Ended,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SheetEnd {
    Closed,
    Rollback,
    ParentRetired,
    OwnerRetired,
}

impl SheetPhase {
    fn visibility_after_end(self, reason: SheetEnd, prior_visible: bool) -> Option<bool> {
        if self == Self::Ended {
            return None;
        }
        Some(match reason {
            SheetEnd::Closed | SheetEnd::ParentRetired => false,
            SheetEnd::Rollback => prior_visible,
            SheetEnd::OwnerRetired => self == Self::Committed || prior_visible,
        })
    }
}

pub(super) struct MacSheetSession {
    owner: MacTransientOwnerLease,
    parent: Retained<NSWindow>,
    parent_handle: Option<AnyWindowHandle>,
    window: Retained<NSWindow>,
    window_state: Weak<Mutex<MacWindowState>>,
    handle: AnyWindowHandle,
    prior_parent: Option<Retained<NSWindow>>,
    prior_frame: NSRect,
    prior_level: NSInteger,
    prior_visible: bool,
    phase: Cell<SheetPhase>,
    presentation_in_progress: Cell<bool>,
    cleanup_in_progress: Cell<bool>,
    pending_end: Cell<Option<(SheetEnd, bool)>>,
    reattachment_ready: Cell<bool>,
}

impl MacSheetSession {
    fn native_id(&self) -> *mut AnyObject {
        Retained::as_ptr(&self.window).cast_mut().cast()
    }

    fn parent_is_current(&self) -> bool {
        if !self.owner.is_presentable() {
            return false;
        }
        if let Some(handle) = self.parent_handle
            && !unsafe {
                native_window_matches_handle(
                    Retained::as_ptr(&self.parent).cast_mut().cast(),
                    handle,
                )
            }
        {
            return false;
        }
        let mut parent = self.parent.clone();
        loop {
            if Retained::as_ptr(&parent).cast_mut().cast::<AnyObject>()
                == self.owner.native_window()
            {
                return true;
            }
            let Some(ancestor) = parent.sheetParent() else {
                return false;
            };
            parent = ancestor;
        }
    }

    fn child_is_current(&self) -> bool {
        unsafe { native_window_matches_handle(self.native_id(), self.handle) }
    }

    fn is_current(&self) -> bool {
        matches!(
            self.phase.get(),
            SheetPhase::Presenting | SheetPhase::Committed
        ) && self.parent_is_current()
            && self.child_is_current()
            && self.window.isVisible()
            && self
                .window
                .sheetParent()
                .is_some_and(|parent| std::ptr::eq(&*parent, &*self.parent))
    }

    fn end(self: &Rc<Self>, reason: SheetEnd) {
        let phase = self.phase.replace(SheetPhase::Ended);
        let reason = if matches!(reason, SheetEnd::ParentRetired | SheetEnd::OwnerRetired)
            && !matches!(phase, SheetPhase::Committed | SheetPhase::Ended)
        {
            SheetEnd::Rollback
        } else {
            reason
        };
        if let Some(visible) = phase.visibility_after_end(reason, self.prior_visible) {
            self.pending_end.set(Some((reason, visible)));
        } else if reason == SheetEnd::OwnerRetired && self.needs_reattachment() {
            self.pending_end.set(Some((SheetEnd::OwnerRetired, true)));
        } else if reason == SheetEnd::Closed && self.pending_end.get().is_some() {
            self.pending_end.set(Some((SheetEnd::Closed, false)));
        } else {
            return;
        }
        self.finish_end();
    }

    fn finish_end(self: &Rc<Self>) {
        let Some((reason, visible)) = self.pending_end.get() else {
            return;
        };
        if self.cleanup_in_progress.replace(true) {
            return;
        }
        self.end_native(reason, visible);
        if reason != SheetEnd::Closed
            && self
                .pending_end
                .get()
                .is_some_and(|(reason, _)| reason == SheetEnd::Closed)
        {
            self.end_native(SheetEnd::Closed, false);
        }
        self.cleanup_in_progress.set(false);
        // AppKit may finish attaching a sheet after a synchronous close callback returns.
        // Keep this session installed until the initiating native call has unwound.
        if !self.presentation_in_progress.get() && !self.needs_reattachment() {
            self.remove_from_window();
        }
    }

    fn needs_reattachment(&self) -> bool {
        self.pending_end
            .get()
            .is_some_and(|(reason, _)| reason == SheetEnd::ParentRetired)
    }

    fn end_native(&self, reason: SheetEnd, visible: bool) {
        if !unsafe { native_window_has_handle(self.native_id(), self.handle) } {
            return;
        }
        if self
            .window
            .sheetParent()
            .is_some_and(|parent| std::ptr::eq(&*parent, &*self.parent))
        {
            self.parent.endSheet(&self.window);
            self.window.orderOut(None);
        }
        if !unsafe { native_window_is_exact_live_handle(self.native_id(), self.handle) } {
            return;
        }
        self.window.setLevel(self.prior_level);
        if !unsafe { native_window_is_exact_live_handle(self.native_id(), self.handle) } {
            return;
        }
        if matches!(reason, SheetEnd::Rollback) {
            self.window.setFrame_display(self.prior_frame, false);
        }
        if !unsafe { native_window_is_exact_live_handle(self.native_id(), self.handle) } {
            return;
        }
        if matches!(reason, SheetEnd::Rollback)
            && let Some(parent) = &self.prior_parent
            && self.owner.is_current()
            && Retained::as_ptr(parent).cast_mut().cast::<AnyObject>() == self.owner.native_window()
            && self.window.parentWindow().is_none()
        {
            unsafe {
                parent.addChildWindow_ordered(&self.window, NSWindowOrderingMode::Above);
            }
        }
        if !unsafe { native_window_is_exact_live_handle(self.native_id(), self.handle) } {
            return;
        }
        if visible && (matches!(reason, SheetEnd::Rollback) || self.child_is_current()) {
            self.window.orderFront(None);
        } else {
            self.window.orderOut(None);
        }
    }

    fn remove_from_window(self: &Rc<Self>) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };
        let mut state = window_state.lock();
        if state
            .sheet_session
            .as_ref()
            .is_some_and(|session| Rc::ptr_eq(session, self))
        {
            state.sheet_session.take();
        }
    }
}

struct SheetPresentation {
    session: Rc<MacSheetSession>,
    committed: bool,
}

struct SheetReattachment {
    session: Rc<MacSheetSession>,
    committed: bool,
}

impl Drop for SheetReattachment {
    fn drop(&mut self) {
        if self.committed || !self.session.needs_reattachment() {
            return;
        }
        let Some(window_state) = self.session.window_state.upgrade() else {
            return;
        };
        let restored = {
            let mut state = window_state.lock();
            if state.handle != self.session.handle
                || state.native_window != self.session.native_id()
                || state.is_closed()
                || state.interaction_is_quiesced()
                || state
                    .sheet_session
                    .as_ref()
                    .is_some_and(|current| !Rc::ptr_eq(current, &self.session))
            {
                false
            } else {
                state.sheet_session = Some(self.session.clone());
                true
            }
        };
        if restored && !self.session.owner.is_current() {
            self.session.end(SheetEnd::OwnerRetired);
        }
    }
}

#[derive(Default)]
#[must_use = "keep the retirement guard until the native parent has finished closing"]
pub(super) struct SheetRetirement {
    children: Vec<Rc<MacSheetSession>>,
}

impl Drop for SheetRetirement {
    fn drop(&mut self) {
        let Some(executor) = self.children.iter().find_map(|session| {
            session
                .window_state
                .upgrade()
                .map(|state| state.lock().foreground_executor.clone())
        }) else {
            return;
        };
        let children = self
            .children
            .iter()
            .filter(|session| session.needs_reattachment())
            .map(|session| {
                session.reattachment_ready.set(true);
                (session.window_state.clone(), Rc::downgrade(session))
            })
            .collect::<Vec<_>>();
        if children.is_empty() {
            return;
        }
        // The actual native parent has finished closing. Restore the chain in one foreground
        // task so nested AppKit loops cannot present a descendant before its new parent commits.
        executor
            .spawn(async move {
                for (window_state, session) in children {
                    let Some(window_state) = window_state.upgrade() else {
                        continue;
                    };
                    let Some(session) = session.upgrade() else {
                        continue;
                    };
                    let is_current = window_state
                        .lock()
                        .sheet_session
                        .as_ref()
                        .is_some_and(|current| Rc::ptr_eq(current, &session));
                    if is_current && let Err(error) = present(&window_state, false) {
                        log::debug!("cannot reattach macOS dialog sheet: {error}");
                    }
                }
            })
            .detach();
    }
}

impl Drop for SheetPresentation {
    fn drop(&mut self) {
        if !self.committed {
            self.session.end(SheetEnd::Rollback);
        }
        self.session.presentation_in_progress.set(false);
        self.session.finish_end();
    }
}

pub(super) fn present(
    window_state: &Arc<Mutex<MacWindowState>>,
    activate: bool,
) -> Result<Option<bool>> {
    let (registry, owner_handle, native_window, handle, existing) = {
        let state = window_state.lock();
        if !state.initial_presentation.dialog {
            return Ok(None);
        }
        let Some(owner_handle) = state.creation_facts.transient_for else {
            return Ok(None);
        };
        anyhow::ensure!(
            !state.is_closed() && !state.interaction_is_quiesced(),
            "macOS dialog is retiring before sheet presentation"
        );
        (
            state.window_registry.clone(),
            owner_handle,
            unsafe { Retained::retain(state.native_window.cast::<NSWindow>()) }
                .ok_or_else(|| anyhow!("macOS dialog has no native window"))?,
            state.handle,
            state.sheet_session.clone(),
        )
    };
    let mut reattachment = match existing {
        Some(session) if session.needs_reattachment() && session.reattachment_ready.get() => {
            Some(SheetReattachment {
                session,
                committed: false,
            })
        }
        Some(session) => {
            if !session.is_current() {
                return Ok(Some(false));
            }
            if activate {
                session.window.makeKeyAndOrderFront(None);
            }
            return Ok(Some(session.is_current()));
        }
        None => None,
    };
    let owner = match registry.observe_transient_owner(owner_handle) {
        MacTransientOwnerResolution::Current(owner) => owner,
        MacTransientOwnerResolution::Retired(_) | MacTransientOwnerResolution::Gone => {
            if let Some(reattachment) = &reattachment {
                reattachment.session.end(SheetEnd::OwnerRetired);
            }
            return Ok(None);
        }
        MacTransientOwnerResolution::Busy => {
            return Err(anyhow!(
                "macOS dialog owner is busy before sheet presentation"
            ));
        }
    };
    anyhow::ensure!(
        owner.is_presentable(),
        "macOS dialog owner is not visible on the active Space"
    );
    let _ordering_scope = MacTransientOrderingScope::begin(window_state)
        .ok_or_else(|| anyhow!("macOS dialog presentation is already in progress"))?;
    let mut parent = unsafe { Retained::retain(owner.native_window().cast::<NSWindow>()) }
        .ok_or_else(|| anyhow!("macOS dialog owner has no native window"))?;
    while let Some(attached) = parent.attachedSheet() {
        anyhow::ensure!(
            !std::ptr::eq(&*attached, &*native_window),
            "macOS dialog already belongs to another sheet presentation"
        );
        parent = attached;
    }
    let parent_id = Retained::as_ptr(&parent).cast_mut().cast();
    let parent_handle = if unsafe { is_gpui_window(parent_id) } {
        let state = unsafe { get_window_state(&*parent_id) };
        let state = state
            .try_lock()
            .ok_or_else(|| anyhow!("macOS dialog sheet parent is busy"))?;
        anyhow::ensure!(
            !state.is_closed() && !state.interaction_is_quiesced(),
            "macOS dialog sheet parent is retiring"
        );
        Some(state.handle)
    } else {
        None
    };
    let prior_parent = native_window.parentWindow();
    anyhow::ensure!(
        native_window.sheetParent().is_none()
            && prior_parent.as_ref().is_none_or(|parent| {
                Retained::as_ptr(parent).cast_mut().cast::<AnyObject>() == owner.native_window()
            }),
        "macOS dialog has an unexpected native parent before sheet presentation"
    );
    let session = Rc::new(MacSheetSession {
        owner,
        parent,
        parent_handle,
        prior_frame: native_window.frame(),
        prior_level: native_window.level(),
        prior_visible: native_window.isVisible(),
        prior_parent,
        window: native_window,
        window_state: Arc::downgrade(window_state),
        handle,
        phase: Cell::default(),
        presentation_in_progress: Cell::new(true),
        cleanup_in_progress: Cell::new(false),
        pending_end: Cell::new(None),
        reattachment_ready: Cell::new(false),
    });
    let mut presentation = SheetPresentation {
        session: session.clone(),
        committed: false,
    };
    {
        let mut state = window_state.lock();
        anyhow::ensure!(
            state.handle == handle
                && state.native_window == session.native_id()
                && !state.is_closed()
                && !state.interaction_is_quiesced()
                && state.sheet_session.as_ref().is_none_or(|current| {
                    reattachment
                        .as_ref()
                        .is_some_and(|reattachment| Rc::ptr_eq(current, &reattachment.session))
                }),
            "macOS dialog changed before its sheet session was installed"
        );
        state.sheet_session = Some(session.clone());
    }
    if let Some(parent) = &session.prior_parent {
        parent.removeChildWindow(&session.window);
    }
    anyhow::ensure!(
        session.phase.get() == SheetPhase::Prepared
            && session.parent_is_current()
            && session.child_is_current(),
        "macOS dialog or owner retired before beginning the sheet"
    );
    session.phase.set(SheetPhase::Presenting);
    session
        .parent
        .beginSheet_completionHandler(&session.window, None);
    if session.is_current() && activate {
        session.window.makeKeyAndOrderFront(None);
    }
    anyhow::ensure!(
        session.is_current(),
        "macOS sheet presentation lost its exact native dialog or owner"
    );
    session.phase.set(SheetPhase::Committed);
    presentation.committed = true;
    if let Some(reattachment) = &mut reattachment {
        reattachment.committed = true;
    }
    Ok(Some(true))
}

pub(super) fn detach_related(window_state: &Arc<Mutex<MacWindowState>>) -> SheetRetirement {
    let mut retirement = SheetRetirement::default();
    let (native_window, session) = {
        let state = window_state.lock();
        (state.native_window, state.sheet_session.clone())
    };
    let Some(window) = (unsafe { Retained::retain(native_window.cast::<NSWindow>()) }) else {
        return retirement;
    };
    detach_children(&window, &mut retirement);
    if let Some(session) = session {
        session.end(SheetEnd::Closed);
    }
    retirement
}

fn detach_children(parent: &NSWindow, retirement: &mut SheetRetirement) {
    let children = parent.sheets();
    for child in children {
        let native_child = Retained::as_ptr(&child).cast_mut().cast();
        let session = if unsafe { is_gpui_window(native_child) } {
            let state = unsafe { get_window_state(&*native_child) };
            let state = state.lock();
            if state.sheet_session.as_ref().is_some_and(|session| {
                std::ptr::eq(&*session.parent, parent) && session.native_id() == native_child
            }) {
                state.sheet_session.clone()
            } else {
                None
            }
        } else {
            None
        };
        // Restore the remaining sheet chain from its root after native cleanup, while still
        // ending the old native relationships from the leaves up.
        if let Some(session) = &session {
            retirement.children.push(session.clone());
        }
        detach_children(&child, retirement);
        if let Some(session) = session {
            let reason = if session.owner.is_current() {
                SheetEnd::ParentRetired
            } else {
                SheetEnd::OwnerRetired
            };
            session.end(reason);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retiring_an_owner_only_reveals_a_committed_sheet() {
        for phase in [SheetPhase::Prepared, SheetPhase::Presenting] {
            assert_eq!(
                phase.visibility_after_end(SheetEnd::OwnerRetired, false),
                Some(false)
            );
        }
        assert_eq!(
            SheetPhase::Committed.visibility_after_end(SheetEnd::OwnerRetired, false),
            Some(true)
        );
        assert_eq!(
            SheetPhase::Committed.visibility_after_end(SheetEnd::ParentRetired, false),
            Some(false)
        );
    }

    #[test]
    fn rollback_preserves_prior_visibility_while_close_always_hides() {
        for prior_visible in [false, true] {
            for phase in [
                SheetPhase::Prepared,
                SheetPhase::Presenting,
                SheetPhase::Committed,
            ] {
                assert_eq!(
                    phase.visibility_after_end(SheetEnd::Rollback, prior_visible),
                    Some(prior_visible)
                );
                assert_eq!(
                    phase.visibility_after_end(SheetEnd::Closed, prior_visible),
                    Some(false)
                );
            }
        }
    }

    #[test]
    fn repeated_native_cleanup_cannot_reveal_or_restore_a_sheet() {
        for reason in [
            SheetEnd::Closed,
            SheetEnd::Rollback,
            SheetEnd::ParentRetired,
            SheetEnd::OwnerRetired,
        ] {
            assert_eq!(SheetPhase::Ended.visibility_after_end(reason, true), None);
        }
    }
}
