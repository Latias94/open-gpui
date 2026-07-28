use std::rc::Weak;

use crate::{
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome, WindowId,
    app::{
        AppCell,
        native_callback_diagnostics::{
            NativeBoundaryDiagnostic, NativeBoundaryKind, NativeBoundaryTarget,
            NativePlatformCommandKind,
        },
    },
};

const MAX_INITIAL_PRESENTATION_COMMAND_ATTEMPTS: u8 = 2;

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
