use crate::{DockItemId, DockSpaceId, surface::DockSurfaceActivationBinding};
use open_gpui::FocusHandle;
use std::collections::HashMap;

/// Last dock-panel focus state used to restore focus after platform viewport activation.
#[derive(Debug, Default)]
pub(crate) struct DockViewportFocusCoordinator {
    focus_by_space: HashMap<DockSpaceId, DockViewportRecordedFocus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportRecordedFocus {
    Panel(DockItemId),
    NoPanelFocus,
}

impl DockViewportFocusCoordinator {
    pub(crate) fn record_panel_focus(&mut self, space: DockSpaceId, item: DockItemId) {
        self.focus_by_space
            .insert(space, DockViewportRecordedFocus::Panel(item));
    }

    pub(crate) fn record_no_panel_focus(&mut self, space: &DockSpaceId) {
        self.focus_by_space
            .insert(space.clone(), DockViewportRecordedFocus::NoPanelFocus);
    }

    pub(crate) fn remove_space(&mut self, space: &DockSpaceId) {
        self.focus_by_space.remove(space);
    }

    #[cfg(test)]
    pub(crate) fn had_panel_focus(&self, space: &DockSpaceId) -> Option<bool> {
        self.focus_by_space
            .get(space)
            .map(|focus| matches!(focus, DockViewportRecordedFocus::Panel(_)))
    }

    pub(crate) fn focused_panel(&self, space: &DockSpaceId) -> Option<&DockItemId> {
        match self.focus_by_space.get(space) {
            Some(DockViewportRecordedFocus::Panel(item)) => Some(item),
            Some(DockViewportRecordedFocus::NoPanelFocus) | None => None,
        }
    }

    pub(crate) fn request_for_platform_activation(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportFocusRequest> {
        match self.focus_by_space.get(space) {
            Some(DockViewportRecordedFocus::Panel(item)) => {
                Some(DockViewportFocusRequest::panel(item.clone()))
            }
            Some(DockViewportRecordedFocus::NoPanelFocus) => {
                Some(DockViewportFocusRequest::no_panel_focus())
            }
            None => None,
        }
    }
}

/// Explicit viewport focus request tracked by activation and render state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockViewportFocusRequest {
    /// Focus a concrete dock item.
    Panel(DockItemId),
    /// Clear focus from dock panels without falling back to another item.
    NoPanelFocus,
}

impl DockViewportFocusRequest {
    /// Requests focus for a concrete dock item after viewport activation.
    pub fn panel(item: impl Into<DockItemId>) -> Self {
        Self::Panel(item.into())
    }

    /// Requests clearing dock panel focus without restoring another panel.
    pub fn no_panel_focus() -> Self {
        Self::NoPanelFocus
    }
}

impl PartialEq<DockViewportFocusRequest> for DockViewportFocusCommand {
    fn eq(&self, other: &DockViewportFocusRequest) -> bool {
        self.request == *other
    }
}

impl PartialEq<DockViewportFocusCommand> for DockViewportFocusRequest {
    fn eq(&self, other: &DockViewportFocusCommand) -> bool {
        *self == *other.request()
    }
}

/// Origin of a pending viewport focus command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportFocusCommandSource {
    /// Platform window activation requested focus restoration.
    PlatformActivation,
    /// A viewport drop, tear-off, or explicit caller requested focus.
    ViewportActivation,
    /// A platform close moved content back into another viewport.
    CloseRecovery,
}

/// Exact rendered focus authority retained across an asynchronous viewport activation.
///
/// The logical [`DockViewportFocusRequest`] still identifies the panel whose selection must be
/// restored. This value additionally preserves the exact focused descendant when the caller has
/// already captured it, so the render path never has to reconstruct focus from optional panel
/// metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportExactFocusTarget {
    focus_handle: FocusHandle,
    claim_revision: u64,
}

impl DockViewportExactFocusTarget {
    pub(crate) const fn new(focus_handle: FocusHandle, claim_revision: u64) -> Self {
        Self {
            focus_handle,
            claim_revision,
        }
    }

    pub(crate) const fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub(crate) const fn claim_revision(&self) -> u64 {
        self.claim_revision
    }
}

/// Pending viewport focus command consumed by the next host render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportFocusCommand {
    source: DockViewportFocusCommandSource,
    request: DockViewportFocusRequest,
    exact_focus_target: Option<DockViewportExactFocusTarget>,
    surface_activation: Option<DockSurfaceActivationBinding>,
}

impl DockViewportFocusCommand {
    pub(crate) fn new(
        source: DockViewportFocusCommandSource,
        request: DockViewportFocusRequest,
    ) -> Self {
        Self {
            source,
            request,
            exact_focus_target: None,
            surface_activation: None,
        }
    }

    pub(crate) fn platform_activation(request: DockViewportFocusRequest) -> Self {
        Self::new(DockViewportFocusCommandSource::PlatformActivation, request)
    }

    pub(crate) fn viewport_activation(request: DockViewportFocusRequest) -> Self {
        Self::new(DockViewportFocusCommandSource::ViewportActivation, request)
    }

    pub(crate) fn surface_activation(
        request: DockViewportFocusRequest,
        binding: DockSurfaceActivationBinding,
    ) -> Self {
        Self {
            source: DockViewportFocusCommandSource::ViewportActivation,
            request,
            exact_focus_target: None,
            surface_activation: Some(binding),
        }
    }

    pub(crate) fn with_exact_focus_target(mut self, target: DockViewportExactFocusTarget) -> Self {
        debug_assert!(
            matches!(self.request, DockViewportFocusRequest::Panel(_)),
            "an exact focus descendant must belong to a concrete panel request"
        );
        self.exact_focus_target = Some(target);
        self
    }

    pub(crate) fn request(&self) -> &DockViewportFocusRequest {
        &self.request
    }

    pub(crate) fn source(&self) -> DockViewportFocusCommandSource {
        self.source
    }

    pub(crate) fn exact_focus_target(&self) -> Option<&DockViewportExactFocusTarget> {
        self.exact_focus_target.as_ref()
    }

    pub(crate) fn surface_activation_binding(&self) -> Option<&DockSurfaceActivationBinding> {
        self.surface_activation.as_ref()
    }
}
