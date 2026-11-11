use std::{rc::Weak, time::Duration};

use crate::{
    NativeBoundaryGeneration, NativeCapturedDragGeneration, PlatformNativeWindowRetirementOutcome,
    PlatformPointerCaptureReleaseOutcome, PlatformPresentationShutdownOutcome,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    PreparedPlatformPointerCaptureRelease, PreparedPlatformPresentationShutdown, Window,
    WindowActivationTerminal, WindowActivationTicket, WindowId, WindowProvisionalRevealOutcome,
    WindowProvisionalRevealTicket,
    app::{
        AppCell,
        native_callback_diagnostics::{
            NativeBoundaryDiagnostic, NativeBoundaryKind, NativeBoundaryTarget,
            NativePlatformCommandKind,
        },
    },
};

const MAX_INITIAL_PRESENTATION_COMMAND_ATTEMPTS: u8 = 2;
const NATIVE_WINDOW_RETIREMENT_RETRY_DELAYS: [Duration; 5] = [
    Duration::ZERO,
    Duration::from_millis(8),
    Duration::from_millis(32),
    Duration::from_millis(128),
    Duration::from_millis(512),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativePointerCaptureReleaseToken {
    window_id: WindowId,
    captured_drag_generation: Option<NativeCapturedDragGeneration>,
    release_generation: u64,
}

impl NativePointerCaptureReleaseToken {
    pub(super) fn new(
        window_id: WindowId,
        captured_drag_generation: Option<NativeCapturedDragGeneration>,
        release_generation: u64,
    ) -> Self {
        Self {
            window_id,
            captured_drag_generation,
            release_generation,
        }
    }

    pub(super) fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(super) fn captured_drag_generation(self) -> Option<NativeCapturedDragGeneration> {
        self.captured_drag_generation
    }

    pub(super) fn release_generation(self) -> u64 {
        self.release_generation
    }
}

#[derive(Clone)]
pub(crate) struct PlatformWindowCommandSink {
    app: Weak<AppCell>,
    window_id: WindowId,
    dispatcher: PlatformWindowCommandDispatcher,
}

impl PlatformWindowCommandSink {
    pub(crate) fn new(
        app: Weak<AppCell>,
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
    ) -> Self {
        Self {
            app,
            window_id,
            dispatcher,
        }
    }

    pub(crate) fn enqueue(&self, command: PlatformWindowCommand) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.enqueue_platform_window_command(self.window_id, self.dispatcher.clone(), command);
    }

    pub(crate) fn request_activation(
        &self,
        activation_policy_generation: u64,
    ) -> WindowActivationTicket {
        let Some(app) = self.app.upgrade() else {
            return WindowActivationTicket::terminal(
                self.app.clone(),
                self.window_id,
                0,
                activation_policy_generation,
                WindowActivationTerminal::WindowClosed,
            );
        };
        app.begin_native_window_activation(
            self.window_id,
            self.dispatcher.clone(),
            activation_policy_generation,
        )
    }

    pub(crate) fn terminal_activation(
        &self,
        activation_policy_generation: u64,
        terminal: WindowActivationTerminal,
    ) -> WindowActivationTicket {
        let Some(app) = self.app.upgrade() else {
            return WindowActivationTicket::terminal(
                self.app.clone(),
                self.window_id,
                0,
                activation_policy_generation,
                WindowActivationTerminal::WindowClosed,
            );
        };
        app.begin_terminal_native_window_activation(
            self.window_id,
            activation_policy_generation,
            terminal,
        )
    }

    pub(crate) fn activation_policy_committed(&self, generation: u64, accepts_activation: bool) {
        if let Some(app) = self.app.upgrade() {
            app.native_window_activation_policy_committed(
                self.window_id,
                generation,
                accepts_activation,
            );
        }
    }

    pub(crate) fn enqueue_provisional_reveal(
        &self,
        command: PlatformWindowCommand,
        ticket: WindowProvisionalRevealTicket,
    ) {
        let Some(app) = self.app.upgrade() else {
            ticket.settle(WindowProvisionalRevealOutcome::WindowTerminal);
            return;
        };
        app.enqueue_provisional_window_reveal(
            self.window_id,
            self.dispatcher.clone(),
            command,
            ticket,
        );
    }

    pub(crate) fn settle_pointer_capture_release(
        &self,
        token: NativePointerCaptureReleaseToken,
        required: bool,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.settle_native_pointer_capture_release(token, self.dispatcher.clone(), required);
    }
}

#[derive(Clone)]
pub(super) struct NativePointerCaptureRelease {
    token: NativePointerCaptureReleaseToken,
    prepared: PreparedPlatformPointerCaptureRelease,
}

impl NativePointerCaptureRelease {
    pub(super) fn new(
        token: NativePointerCaptureReleaseToken,
        dispatcher: PlatformWindowCommandDispatcher,
    ) -> Self {
        let prepared = dispatcher.prepare_pointer_capture_release(token.release_generation());
        Self { token, prepared }
    }

    pub(super) fn token(&self) -> NativePointerCaptureReleaseToken {
        self.token
    }

    pub(super) fn pending_diagnostic(&self, sequence: u64) -> NativeBoundaryDiagnostic {
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Window(self.token.window_id),
            NativeBoundaryKind::Command(NativePlatformCommandKind::ReleasePointerCapture),
            Some(super::native_callback_diagnostics::NativeBoundaryGeneration::PointerCaptureRelease {
                captured_drag: self.token.captured_drag_generation,
                release: self.token.release_generation,
            }),
        )
    }

    pub(super) fn dispatch(&self) -> PlatformPointerCaptureReleaseOutcome {
        self.prepared.dispatch()
    }
}

enum NativeWindowRetirementOwner {
    Window(Option<Box<Window>>),
    Platform(Option<Box<dyn crate::PlatformWindow>>),
}

pub(super) struct NativeWindowRetirement {
    window_id: WindowId,
    owner: NativeWindowRetirementOwner,
    presentation_shutdown: Option<PreparedPlatformPresentationShutdown>,
    retry_attempts: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeWindowRetirementAttempt {
    Accepted,
    NativeWindowTerminal,
    BindingDetached,
    Rejected,
}

#[derive(Clone, Copy)]
pub(super) struct NativeShutdownCompletion {
    generation: u64,
}

impl NativeShutdownCompletion {
    pub(super) fn new(generation: u64) -> Self {
        Self { generation }
    }

    pub(super) fn generation(self) -> u64 {
        self.generation
    }

    pub(super) fn pending_diagnostic(self, sequence: u64) -> NativeBoundaryDiagnostic {
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Application,
            NativeBoundaryKind::Command(NativePlatformCommandKind::CompleteShutdown),
            Some(
                super::native_callback_diagnostics::NativeBoundaryGeneration::AppShutdown(
                    self.generation,
                ),
            ),
        )
    }
}

impl NativeWindowRetirement {
    pub(super) fn new(window_id: WindowId, window: Box<Window>) -> Self {
        Self::with_owner(
            window_id,
            NativeWindowRetirementOwner::Window(Some(window)),
            None,
        )
    }

    pub(super) fn from_platform_window(
        window_id: WindowId,
        platform_window: Box<dyn crate::PlatformWindow>,
        presentation_shutdown: PreparedPlatformPresentationShutdown,
    ) -> Self {
        Self::with_owner(
            window_id,
            NativeWindowRetirementOwner::Platform(Some(platform_window)),
            Some(presentation_shutdown),
        )
    }

    fn with_owner(
        window_id: WindowId,
        owner: NativeWindowRetirementOwner,
        presentation_shutdown: Option<PreparedPlatformPresentationShutdown>,
    ) -> Self {
        Self {
            window_id,
            owner,
            presentation_shutdown,
            retry_attempts: 0,
        }
    }

    pub(super) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(super) fn presentation_shutdown(&self) -> Option<PreparedPlatformPresentationShutdown> {
        self.presentation_shutdown.clone()
    }

    /// Prepares the exact presentation authority while borrowing the retained window owner.
    /// Retained-resource cleanup and backend preparation may unwind; the owner remains in
    /// this retirement so the existing native queue can retry preparation.
    pub(super) fn prepare_presentation_shutdown(&mut self) -> PreparedPlatformPresentationShutdown {
        if self.presentation_shutdown.is_none() {
            let NativeWindowRetirementOwner::Window(Some(window)) = &mut self.owner else {
                panic!("unprepared retirement must retain its GPUI window owner");
            };
            self.presentation_shutdown = Some(window.claim_presentation_shutdown());
        }
        self.presentation_shutdown
            .as_ref()
            .expect("prepared retirement must retain its presentation authority")
            .clone()
    }

    pub(super) fn pending_diagnostic(&self, sequence: u64) -> NativeBoundaryDiagnostic {
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Window(self.window_id),
            NativeBoundaryKind::Command(NativePlatformCommandKind::RetireNativeWindow),
            self.presentation_shutdown.as_ref().map(|shutdown| {
                NativeBoundaryGeneration::PresentationShutdown(shutdown.snapshot().generation())
            }),
        )
    }

    pub(super) fn retains_window_owner(&self) -> bool {
        match &self.owner {
            NativeWindowRetirementOwner::Window(window) => window.is_some(),
            NativeWindowRetirementOwner::Platform(platform_window) => platform_window.is_some(),
        }
    }

    fn retire_native_window(&self) -> PlatformNativeWindowRetirementOutcome {
        let shutdown = self
            .presentation_shutdown
            .as_ref()
            .expect("native retirement must follow presentation preparation");
        match &self.owner {
            NativeWindowRetirementOwner::Window(Some(window)) => window
                .platform_window
                .retire_native_window(shutdown.ticket()),
            NativeWindowRetirementOwner::Platform(Some(platform_window)) => {
                platform_window.retire_native_window(shutdown.ticket())
            }
            NativeWindowRetirementOwner::Window(None)
            | NativeWindowRetirementOwner::Platform(None) => {
                panic!("pending native retirement must retain its platform-window owner")
            }
        }
    }

    fn drop_owner(&mut self) {
        match &mut self.owner {
            NativeWindowRetirementOwner::Window(window) => drop(window.take()),
            NativeWindowRetirementOwner::Platform(platform_window) => drop(platform_window.take()),
        }
    }

    pub(super) fn retire(&mut self) -> NativeWindowRetirementAttempt {
        let shutdown = self
            .presentation_shutdown
            .as_ref()
            .expect("retirement dispatch must publish its prepared presentation authority");
        if !shutdown.snapshot().quiesced()
            && shutdown.quiesce() != PlatformPresentationShutdownOutcome::Quiesced
        {
            return NativeWindowRetirementAttempt::Rejected;
        }
        if !shutdown.snapshot().quiesced() {
            return NativeWindowRetirementAttempt::Rejected;
        }
        match self.retire_native_window() {
            PlatformNativeWindowRetirementOutcome::Accepted => {
                self.drop_owner();
                NativeWindowRetirementAttempt::Accepted
            }
            PlatformNativeWindowRetirementOutcome::NativeWindowTerminal => {
                if !shutdown.ticket().acknowledge_native_terminal() {
                    return NativeWindowRetirementAttempt::Rejected;
                }
                self.drop_owner();
                NativeWindowRetirementAttempt::NativeWindowTerminal
            }
            PlatformNativeWindowRetirementOutcome::BindingDetached => {
                if !shutdown.ticket().acknowledge_detached() {
                    return NativeWindowRetirementAttempt::Rejected;
                }
                self.drop_owner();
                NativeWindowRetirementAttempt::BindingDetached
            }
            PlatformNativeWindowRetirementOutcome::Rejected => {
                NativeWindowRetirementAttempt::Rejected
            }
        }
    }

    pub(super) fn next_retry_delay(&mut self) -> Duration {
        let index =
            usize::from(self.retry_attempts).min(NATIVE_WINDOW_RETIREMENT_RETRY_DELAYS.len() - 1);
        self.retry_attempts = self.retry_attempts.saturating_add(1);
        NATIVE_WINDOW_RETIREMENT_RETRY_DELAYS[index]
    }
}

pub(super) struct NativePlatformCommand {
    window_id: WindowId,
    dispatcher: PlatformWindowCommandDispatcher,
    command: PlatformWindowCommand,
    provisional_reveal_ticket: Option<WindowProvisionalRevealTicket>,
    attempt: u8,
}

pub(super) enum NativePlatformCommandRejection {
    Retry(NativePlatformCommand),
    InitialPresentationFailed,
    Terminal,
}

impl NativePlatformCommand {
    pub(super) fn new(
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
        command: PlatformWindowCommand,
    ) -> Self {
        Self {
            window_id,
            dispatcher,
            command,
            provisional_reveal_ticket: None,
            attempt: 1,
        }
    }

    pub(super) fn new_provisional_reveal(
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
        command: PlatformWindowCommand,
        ticket: WindowProvisionalRevealTicket,
    ) -> Self {
        Self {
            window_id,
            dispatcher,
            command,
            provisional_reveal_ticket: Some(ticket),
            attempt: 1,
        }
    }

    pub(super) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(super) fn completes_initial_presentation(&self) -> bool {
        matches!(
            self.command,
            PlatformWindowCommand::CompleteInitialPresentation { .. }
        )
    }

    pub(super) fn provisional_reveal_is_pending(&self) -> bool {
        self.provisional_reveal_ticket
            .as_ref()
            .is_none_or(|ticket| {
                ticket.snapshot().outcome() == WindowProvisionalRevealOutcome::Pending
            })
    }

    pub(super) fn activation_request_generation(&self) -> Option<u64> {
        match self.command {
            PlatformWindowCommand::Activate { request_generation } => Some(request_generation),
            _ => None,
        }
    }

    pub(super) fn requires_interaction_authority(&self) -> bool {
        self.command.requires_interaction_authority()
    }

    pub(super) fn pending_diagnostic(&self, sequence: u64) -> NativeBoundaryDiagnostic {
        let (kind, generation) = match self.command {
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
            PlatformWindowCommand::CompleteNativeFrame => {
                (NativePlatformCommandKind::CompleteNativeFrame, None)
            }
            PlatformWindowCommand::CompleteInitialPresentation { .. } => {
                (NativePlatformCommandKind::CompleteInitialPresentation, None)
            }
            PlatformWindowCommand::RevealDeferredInitialPresentation {
                session_generation,
                presentation_generation,
                ..
            } => (
                NativePlatformCommandKind::RevealDeferredInitialPresentation,
                Some(NativeBoundaryGeneration::ProvisionalPresentation {
                    session_generation,
                    presentation_generation,
                }),
            ),
            PlatformWindowCommand::Activate { request_generation } => (
                NativePlatformCommandKind::Activate,
                Some(NativeBoundaryGeneration::WindowActivation(
                    request_generation,
                )),
            ),
            PlatformWindowCommand::ShowWindowMenu(_) => {
                (NativePlatformCommandKind::ShowWindowMenu, None)
            }
            PlatformWindowCommand::StartWindowMove => {
                (NativePlatformCommandKind::StartWindowMove, None)
            }
            PlatformWindowCommand::StartWindowResize(_) => {
                (NativePlatformCommandKind::StartWindowResize, None)
            }
            PlatformWindowCommand::RestoreNativeWindowState(_) => {
                (NativePlatformCommandKind::RestoreNativeWindowState, None)
            }
            PlatformWindowCommand::ToggleSimpleFullscreen => {
                (NativePlatformCommandKind::ToggleSimpleFullscreen, None)
            }
            PlatformWindowCommand::ShowSoftKeyboard => {
                (NativePlatformCommandKind::ShowSoftKeyboard, None)
            }
            PlatformWindowCommand::HideSoftKeyboard => {
                (NativePlatformCommandKind::HideSoftKeyboard, None)
            }
            PlatformWindowCommand::TextInputStateChanged(_) => {
                (NativePlatformCommandKind::TextInputStateChanged, None)
            }
        };
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Window(self.window_id),
            NativeBoundaryKind::Command(kind),
            generation,
        )
    }

    pub(super) fn settle_provisional_reveal(&self, outcome: WindowProvisionalRevealOutcome) {
        if let Some(ticket) = self.provisional_reveal_ticket.as_ref() {
            ticket.settle(outcome);
        }
    }

    pub(super) fn dispatch(&self) -> PlatformWindowCommandOutcome {
        self.dispatcher.dispatch(self.command.clone())
    }

    pub(super) fn settle_rejection(mut self) -> NativePlatformCommandRejection {
        if !self.completes_initial_presentation() {
            return NativePlatformCommandRejection::Terminal;
        }
        if self.attempt >= MAX_INITIAL_PRESENTATION_COMMAND_ATTEMPTS {
            return NativePlatformCommandRejection::InitialPresentationFailed;
        }
        self.attempt += 1;
        NativePlatformCommandRejection::Retry(self)
    }
}

impl Drop for NativePlatformCommand {
    fn drop(&mut self) {
        self.settle_provisional_reveal(WindowProvisionalRevealOutcome::Rejected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DevicePixels, PlatformWindowCommandOutcome, WindowProvisionalRevealCancellationOutcome,
        point,
    };

    #[test]
    fn cancelled_provisional_reveal_command_is_not_dispatchable() {
        let window_id = WindowId::from(71);
        let reveal_point = point(DevicePixels(40), DevicePixels(50));
        let ticket = WindowProvisionalRevealTicket::new(
            window_id,
            9,
            reveal_point,
            None,
            14,
            Vec::<WindowId>::new().into(),
        );
        let command = NativePlatformCommand::new_provisional_reveal(
            window_id,
            PlatformWindowCommandDispatcher::new(|_| PlatformWindowCommandOutcome::Accepted),
            PlatformWindowCommand::RevealDeferredInitialPresentation {
                session_generation: 9,
                presentation_generation: 14,
            },
            ticket.clone(),
        );
        assert!(command.provisional_reveal_is_pending());

        assert!(matches!(
            ticket.cancel(),
            WindowProvisionalRevealCancellationOutcome::Cancelled(_)
        ));
        assert!(!command.provisional_reveal_is_pending());
    }
}
