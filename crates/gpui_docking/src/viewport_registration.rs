use crate::{
    DockSpaceId, DockViewportAdapter, DockViewportRuntimeLineage, DockViewportUnregisterOutcome,
    DockViewportUnregisterReason,
    viewport_registry::{DockViewportRegistrationConflict, DockViewportRegistrationKey},
};
use open_gpui::AnyWindowHandle;

/// Runtime result of registering or replacing a platform viewport mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportRegisterOutcome {
    /// Logical dock space now rendered by the registered window.
    space: DockSpaceId,
    /// GPUI window now rendering the logical dock space.
    window: AnyWindowHandle,
    /// Exact registration generation issued for this binding.
    registration_key: DockViewportRegistrationKey,
    /// Runtime mappings removed to preserve one-to-one space/window ownership.
    replaced: Vec<DockViewportUnregisterOutcome>,
}

impl DockViewportRegisterOutcome {
    #[cfg(test)]
    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn registration_key(&self) -> &DockViewportRegistrationKey {
        &self.registration_key
    }

    pub(crate) fn replaced(&self) -> &[DockViewportUnregisterOutcome] {
        &self.replaced
    }
}

impl DockViewportAdapter {
    /// Registers or replaces the window for a logical dock space and reports every removed mapping.
    ///
    /// A single registration can replace two mappings: the previous window for `space`, and the
    /// previous space that already owned `window`.
    pub(crate) fn register_viewport_with_outcome(
        &mut self,
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
        lineage: DockViewportRuntimeLineage,
    ) -> Result<DockViewportRegisterOutcome, DockViewportRegistrationConflict> {
        let space = space.into();
        let window = window.into();
        let replaced = self
            .registry
            .register_with_replacements(space.clone(), window, lineage)?
            .into_iter()
            .map(|(space, snapshot)| DockViewportUnregisterOutcome {
                space,
                window: snapshot.window,
                reason: DockViewportUnregisterReason::Replaced,
            })
            .collect();
        let registration_key = self
            .registry
            .registration_key(&space)
            .expect("a successful viewport registration must issue an exact key");

        Ok(DockViewportRegisterOutcome {
            space,
            window,
            registration_key,
            replaced,
        })
    }
}
