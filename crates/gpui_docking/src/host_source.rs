use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockGraph, DockHost,
    DockHostOptions, DockPanelRegistry, DockPolicy, DockSpaceId, DockWorkspace,
};
use open_gpui::{AppContext as _, Context, Entity};
use thiserror::Error;

#[derive(Debug)]
pub(crate) enum DockHostSource {
    Owned(Box<DockWorkspace>),
    Controller {
        controller: Entity<DockController>,
        space: DockSpaceId,
    },
}

/// Error returned when callers request owned workspace state from a controller-backed host.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockHostAccessError {
    /// The host renders from a shared controller instead of owning workspace state directly.
    #[error("controller-backed hosts expose docking state through DockController")]
    ControllerBackedHost,
}

impl DockHostSource {
    fn workspace(&self) -> Result<&DockWorkspace, DockHostAccessError> {
        match self {
            DockHostSource::Owned(workspace) => Ok(workspace),
            DockHostSource::Controller { .. } => Err(DockHostAccessError::ControllerBackedHost),
        }
    }

    fn controller(&self) -> Option<&Entity<DockController>> {
        match self {
            DockHostSource::Owned(_) => None,
            DockHostSource::Controller { controller, .. } => Some(controller),
        }
    }

    fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match self {
            DockHostSource::Owned(workspace) => workspace.apply_action(action),
            DockHostSource::Controller { .. } => Err(DockActionApplyError::ControllerBackedHost),
        }
    }

    fn space(&self) -> &DockSpaceId {
        match self {
            DockHostSource::Owned(workspace) => workspace.space(),
            DockHostSource::Controller { space, .. } => space,
        }
    }

    fn with_workspace<R>(
        &self,
        cx: &Context<DockHost>,
        read: impl FnOnce(&DockWorkspace) -> R,
    ) -> R {
        match self {
            DockHostSource::Owned(workspace) => read(workspace),
            DockHostSource::Controller { controller, .. } => {
                cx.read_entity(controller, |controller, _| read(controller.workspace()))
            }
        }
    }

    fn apply_action_from_host(
        &mut self,
        action: &DockAction,
        cx: &mut Context<DockHost>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match self {
            DockHostSource::Owned(workspace) => workspace.apply_action(action),
            DockHostSource::Controller { controller, .. } => {
                let controller = controller.clone();
                cx.update_entity(&controller, |controller, cx| {
                    let outcome = controller.apply_action(action);
                    if outcome
                        .as_ref()
                        .map(|outcome| outcome.changed())
                        .unwrap_or(false)
                    {
                        cx.notify();
                    }
                    outcome
                })
            }
        }
    }
}

impl DockHost {
    /// Returns the owned workspace rendered by this host.
    ///
    /// This accessor is available for compatibility hosts created with [`Self::from_workspace`].
    /// Controller-backed hosts expose shared state through [`DockController`].
    pub fn workspace(&self) -> Result<&DockWorkspace, DockHostAccessError> {
        self.source.workspace()
    }

    /// Returns the shared controller when this host is controller-backed.
    pub fn controller(&self) -> Option<&Entity<DockController>> {
        self.source.controller()
    }

    /// Applies a docking action through the host's owned workspace.
    ///
    /// Controller-backed hosts apply UI actions through their shared [`DockController`] during
    /// rendering callbacks. Call [`DockController::apply_action`] directly for external commits.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.source.apply_action(action)
    }

    /// Returns the logical dock space rendered by this host.
    pub fn space(&self) -> &DockSpaceId {
        self.source.space()
    }

    /// Returns the graph for an owned-workspace host.
    ///
    /// Controller-backed hosts expose shared graph state through [`DockController::graph`].
    pub fn graph(&self) -> Result<&DockGraph, DockHostAccessError> {
        Ok(self.workspace()?.graph())
    }

    /// Returns the panel registry for an owned-workspace host.
    ///
    /// Controller-backed hosts expose shared panel state through [`DockController::panels`].
    pub fn panels(&self) -> Result<&DockPanelRegistry, DockHostAccessError> {
        Ok(self.workspace()?.panels())
    }

    /// Returns the static rendering options for an owned-workspace host.
    ///
    /// Controller-backed hosts expose shared options through [`DockController::options`].
    pub fn options(&self) -> Result<&DockHostOptions, DockHostAccessError> {
        Ok(self.workspace()?.options())
    }

    /// Returns the docking interaction policy for an owned-workspace host.
    ///
    /// Controller-backed hosts expose shared policy through [`DockController::policy`].
    pub fn policy(&self) -> Result<&DockPolicy, DockHostAccessError> {
        Ok(self.workspace()?.policy())
    }

    pub(crate) fn with_workspace<R>(
        &self,
        cx: &Context<Self>,
        read: impl FnOnce(&DockWorkspace) -> R,
    ) -> R {
        self.source.with_workspace(cx, read)
    }

    pub(crate) fn apply_action_from_host(
        &mut self,
        action: &DockAction,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.source.apply_action_from_host(action, cx)
    }
}
