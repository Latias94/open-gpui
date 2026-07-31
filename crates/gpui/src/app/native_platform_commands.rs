use std::{rc::Weak, time::Duration};

use crate::{
    NativeCapturedDragGeneration, PlatformNativeWindowRetirementOutcome,
    PlatformPointerCaptureReleaseOutcome, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PreparedPlatformPointerCaptureRelease, Window, WindowId,
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

pub(super) struct NativeWindowRetirement {
    window_id: WindowId,
    window: Option<Box<Window>>,
    retry_attempts: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeWindowRetirementAttempt {
    Accepted,
    NativeWindowTerminal,
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
        Self {
            window_id,
            window: Some(window),
            retry_attempts: 0,
        }
    }

    pub(super) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(super) fn pending_diagnostic(&self, sequence: u64) -> NativeBoundaryDiagnostic {
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Window(self.window_id),
            NativeBoundaryKind::Command(NativePlatformCommandKind::RetireNativeWindow),
            None,
        )
    }

    pub(super) fn retains_window_owner(&self) -> bool {
        self.window.is_some()
    }

    pub(super) fn retire(&mut self) -> NativeWindowRetirementAttempt {
        let outcome = self
            .window
            .as_ref()
            .expect("pending native retirement must retain its platform-window owner")
            .platform_window
            .retire_native_window();
        match outcome {
            PlatformNativeWindowRetirementOutcome::Accepted => {
                drop(self.window.take());
                NativeWindowRetirementAttempt::Accepted
            }
            PlatformNativeWindowRetirementOutcome::NativeWindowTerminal => {
                drop(self.window.take());
                NativeWindowRetirementAttempt::NativeWindowTerminal
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

    pub(super) fn pending_diagnostic(&self, sequence: u64) -> NativeBoundaryDiagnostic {
        let kind = match self.command {
            PlatformWindowCommand::CompleteInitialPresentation { .. } => {
                NativePlatformCommandKind::CompleteInitialPresentation
            }
            PlatformWindowCommand::Activate => NativePlatformCommandKind::Activate,
            PlatformWindowCommand::ShowWindowMenu(_) => NativePlatformCommandKind::ShowWindowMenu,
            PlatformWindowCommand::StartWindowMove => NativePlatformCommandKind::StartWindowMove,
            PlatformWindowCommand::StartWindowResize(_) => {
                NativePlatformCommandKind::StartWindowResize
            }
        };
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Window(self.window_id),
            NativeBoundaryKind::Command(kind),
            None,
        )
    }

    pub(super) fn dispatch(&self) -> PlatformWindowCommandOutcome {
        self.dispatcher.dispatch(self.command)
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
