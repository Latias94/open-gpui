use std::cell::{Cell, RefCell};

use anyhow::{Context as _, Result};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
    WindowsAndMessaging::SetForegroundWindow,
};

use crate::RegisteredWindow;

use super::{RegisteredWindowAuthority, RegisteredWindowAuthorityStatus};

pub(super) struct ModalParentClaim {
    authority: RegisteredWindowAuthority,
    released: Cell<bool>,
}

impl ModalParentClaim {
    fn owner_registration(&self) -> RegisteredWindow {
        self.authority.registration
    }

    fn claim_count(&self) -> usize {
        self.authority.window.modal_parent_claim_count.get()
    }

    fn decrement_claim_count(&self) -> Result<usize> {
        let count = self.claim_count();
        anyhow::ensure!(count > 0, "registered modal-parent claim count underflow");
        let remaining = count - 1;
        self.authority
            .window
            .modal_parent_claim_count
            .set(remaining);
        Ok(remaining)
    }

    fn restore_claim_count_after_failed_release(&self) -> Result<()> {
        let count = self.claim_count();
        let restored = count
            .checked_add(1)
            .context("registered modal-parent claim count overflow")?;
        self.authority.window.modal_parent_claim_count.set(restored);
        Ok(())
    }

    fn release(&self) -> Result<()> {
        if self.released.get() {
            return Ok(());
        }
        match self.authority.status() {
            RegisteredWindowAuthorityStatus::Current => {
                let remaining = self.decrement_claim_count()?;
                if remaining > 0 {
                    self.released.set(true);
                    return Ok(());
                }
                if self.authority.window.presentation_shutdown_claimed() {
                    self.released.set(true);
                    return Ok(());
                }
                let hwnd = self
                    .authority
                    .ensure_current("releasing the registered modal parent")?;
                unsafe {
                    let _ = EnableWindow(hwnd, true);
                }
                match self.authority.status() {
                    RegisteredWindowAuthorityStatus::Current => {
                        if self.authority.window.presentation_shutdown_claimed() {
                            unsafe {
                                let _ = EnableWindow(hwnd, false);
                            }
                            self.released.set(true);
                            return Ok(());
                        }
                        if self.claim_count() == 0 {
                            if !unsafe { IsWindowEnabled(hwnd).as_bool() } {
                                self.restore_claim_count_after_failed_release()?;
                                anyhow::bail!(
                                    "registered modal parent remained disabled after final release"
                                );
                            }
                            unsafe {
                                let _ = SetForegroundWindow(hwnd);
                            }
                        } else {
                            unsafe {
                                let _ = EnableWindow(hwnd, false);
                            }
                            anyhow::ensure!(
                                !unsafe { IsWindowEnabled(hwnd).as_bool() },
                                "registered modal parent became enabled while another modal claim remained"
                            );
                        }
                    }
                    RegisteredWindowAuthorityStatus::RegistrationMissing
                    | RegisteredWindowAuthorityStatus::RegistrationReplaced
                    | RegisteredWindowAuthorityStatus::NativeOwnerReplaced
                    | RegisteredWindowAuthorityStatus::NativeWindowTerminal => {}
                    status => {
                        self.restore_claim_count_after_failed_release()?;
                        anyhow::bail!(
                            "cannot validate a released registered modal-parent claim: {status:?}"
                        );
                    }
                }
            }
            RegisteredWindowAuthorityStatus::RegistrationMissing
            | RegisteredWindowAuthorityStatus::RegistrationReplaced
            | RegisteredWindowAuthorityStatus::NativeOwnerReplaced
            | RegisteredWindowAuthorityStatus::NativeWindowTerminal => {
                self.decrement_claim_count()?;
            }
            status => anyhow::bail!(
                "cannot release an ambiguous registered modal-parent claim: {status:?}"
            ),
        }
        self.released.set(true);
        Ok(())
    }

    fn abandon_for_owner_retirement(&self, owner: &RegisteredWindowAuthority) -> Result<()> {
        anyhow::ensure!(
            self.authority.registration.matches(owner.registration)
                && std::rc::Rc::ptr_eq(&self.authority.window, &owner.window)
                && std::sync::Arc::ptr_eq(&self.authority.registry, &owner.registry),
            "modal-parent claim did not belong to the exact retiring owner"
        );
        self.decrement_claim_count()?;
        self.released.set(true);
        Ok(())
    }
}

impl Drop for ModalParentClaim {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            log::error!("failed to release registered modal-parent claim: {error:#}");
        }
    }
}

pub(super) struct ModalParentGuard {
    claim: Option<ModalParentClaim>,
}

impl ModalParentGuard {
    pub(super) fn acquire(parent: Option<&RegisteredWindowAuthority>) -> Result<Self> {
        let Some(parent) = parent else {
            return Ok(Self { claim: None });
        };
        let hwnd = parent.ensure_current("disabling the registered modal parent")?;
        let existing_claims = parent.window.modal_parent_claim_count.get();
        if existing_claims == 0 && !unsafe { IsWindowEnabled(hwnd).as_bool() } {
            return Ok(Self { claim: None });
        }
        let claim_count = existing_claims
            .checked_add(1)
            .context("registered modal-parent claim count overflow")?;
        parent.window.modal_parent_claim_count.set(claim_count);
        let guard = Self {
            claim: Some(ModalParentClaim {
                authority: parent.clone(),
                released: Cell::new(false),
            }),
        };
        unsafe {
            let _ = EnableWindow(hwnd, false);
        }
        if let Err(error) = parent.ensure_current("validating the disabled registered modal parent")
        {
            drop(guard);
            return Err(error);
        }
        if unsafe { IsWindowEnabled(hwnd).as_bool() } {
            drop(guard);
            anyhow::bail!("registered modal parent remained enabled after acquiring a claim");
        }
        Ok(guard)
    }

    pub(super) fn commit(mut self) -> Option<ModalParentClaim> {
        self.claim.take()
    }
}

impl Drop for ModalParentGuard {
    fn drop(&mut self) {
        drop(self.claim.take());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeTransientOwnerState {
    Ownerless,
    Attached(RegisteredWindow),
    TemporarilyDetached(RegisteredWindow),
}

pub(super) struct NativeTransientOwnerBinding {
    state: Cell<NativeTransientOwnerState>,
    modal_parent_claim: RefCell<Option<ModalParentClaim>>,
}

impl NativeTransientOwnerBinding {
    pub(super) fn new(
        owner: Option<RegisteredWindow>,
        modal_parent_claim: Option<ModalParentClaim>,
    ) -> Self {
        debug_assert!(modal_parent_claim.as_ref().is_none_or(|claim| {
            owner.is_some_and(|owner| claim.owner_registration().matches(owner))
        }));
        Self {
            state: Cell::new(owner.map_or(
                NativeTransientOwnerState::Ownerless,
                NativeTransientOwnerState::Attached,
            )),
            modal_parent_claim: RefCell::new(modal_parent_claim),
        }
    }

    pub(super) fn state(&self) -> NativeTransientOwnerState {
        self.state.get()
    }

    pub(super) fn owner_registration(&self) -> Option<RegisteredWindow> {
        match self.state() {
            NativeTransientOwnerState::Ownerless => None,
            NativeTransientOwnerState::Attached(owner)
            | NativeTransientOwnerState::TemporarilyDetached(owner) => Some(owner),
        }
    }

    pub(super) fn attached_owner_registration(&self) -> Option<RegisteredWindow> {
        let NativeTransientOwnerState::Attached(owner) = self.state() else {
            return None;
        };
        Some(owner)
    }

    pub(super) fn begin_temporary_detach(&self, owner: RegisteredWindow) -> bool {
        if self.state() != NativeTransientOwnerState::Attached(owner) {
            return false;
        }
        self.state
            .set(NativeTransientOwnerState::TemporarilyDetached(owner));
        true
    }

    pub(super) fn restore_after_temporary_detach(&self, owner: RegisteredWindow) -> bool {
        if self.state() != NativeTransientOwnerState::TemporarilyDetached(owner) {
            return false;
        }
        self.state.set(NativeTransientOwnerState::Attached(owner));
        true
    }

    pub(super) fn release_modal_parent(&self) -> Result<()> {
        let Some(claim) = self.modal_parent_claim.borrow_mut().take() else {
            return Ok(());
        };
        match claim.release() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.modal_parent_claim.borrow_mut().replace(claim);
                Err(error)
            }
        }
    }

    pub(super) fn retire(&self) -> Result<()> {
        self.release_modal_parent()?;
        self.state.set(NativeTransientOwnerState::Ownerless);
        Ok(())
    }

    pub(super) fn retire_for_owner_retirement(
        &self,
        owner: &RegisteredWindowAuthority,
    ) -> Result<()> {
        anyhow::ensure!(
            self.owner_registration()
                .is_some_and(|current| current.matches(owner.registration)),
            "native transient binding did not belong to the exact retiring owner"
        );
        let mut claim = self.modal_parent_claim.borrow_mut();
        if let Some(current) = claim.as_ref() {
            current.abandon_for_owner_retirement(owner)?;
        }
        claim.take();
        self.state.set(NativeTransientOwnerState::Ownerless);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeTransientChildRetirementPreparation {
    Unrelated,
    Detached,
    RetirementPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeOwnerRetirementPreparation {
    Ready,
    Retryable,
    WaitingForChild,
}

#[cfg(test)]
mod tests {
    use open_gpui::WindowId;
    use windows::Win32::Foundation::HWND;

    use super::{NativeTransientOwnerBinding, NativeTransientOwnerState, RegisteredWindow};

    #[test]
    fn transient_owner_binding_never_restores_after_permanent_retirement() {
        let owner = RegisteredWindow::new(HWND(1 as _), 7, WindowId::from(9));
        let binding = NativeTransientOwnerBinding::new(Some(owner), None);

        assert_eq!(binding.attached_owner_registration(), Some(owner));
        assert!(binding.begin_temporary_detach(owner));
        assert_eq!(
            binding.state(),
            NativeTransientOwnerState::TemporarilyDetached(owner)
        );

        binding.retire().unwrap();

        assert!(!binding.restore_after_temporary_detach(owner));
        assert_eq!(binding.state(), NativeTransientOwnerState::Ownerless);
        assert_eq!(binding.owner_registration(), None);
        assert!(binding.modal_parent_claim.borrow().is_none());
    }
}
