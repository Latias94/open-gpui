#[cfg(test)]
use crate::DockViewportSnapshot;
use crate::{
    DockSpaceId, DockViewportAdapter, DockViewportUnregisterOutcome, DockViewportUnregisterReason,
};
use open_gpui::AnyWindowHandle;

/// Runtime result of registering or replacing a platform viewport mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportRegisterOutcome {
    /// Logical dock space now rendered by the registered window.
    space: DockSpaceId,
    /// GPUI window now rendering the logical dock space.
    window: AnyWindowHandle,
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

    pub(crate) fn replaced(&self) -> &[DockViewportUnregisterOutcome] {
        &self.replaced
    }
}

impl DockViewportAdapter {
    /// Registers or replaces the window for a logical dock space.
    ///
    /// A window can belong to only one dock space at a time. Registering the same window for a
    /// different space removes its previous space mapping.
    #[cfg(test)]
    pub(crate) fn register_viewport(
        &mut self,
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<DockViewportSnapshot> {
        let space = space.into();
        let window = window.into();
        self.registry.register(space, window)
    }

    /// Registers or replaces the window for a logical dock space and reports every removed mapping.
    ///
    /// A single registration can replace two mappings: the previous window for `space`, and the
    /// previous space that already owned `window`.
    pub(crate) fn register_viewport_with_outcome(
        &mut self,
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
    ) -> DockViewportRegisterOutcome {
        let space = space.into();
        let window = window.into();
        let replaced = self
            .registry
            .register_with_replacements(space.clone(), window)
            .into_iter()
            .map(|(space, snapshot)| DockViewportUnregisterOutcome {
                space,
                window: snapshot.window,
                reason: DockViewportUnregisterReason::Replaced,
            })
            .collect();

        DockViewportRegisterOutcome {
            space,
            window,
            replaced,
        }
    }
}
