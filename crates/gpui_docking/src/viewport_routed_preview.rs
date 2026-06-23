use crate::{
    DockSpaceId, DockViewportDropRoute, DockViewportIdentity, DockViewportResolvedDropRoute,
    DockViewportResolvedDropTargetSnapshot, drop_target::DockDropTargetKey,
    interaction::DockRuntimeDragSession,
};
use open_gpui::{AnyWindowHandle, WindowId};

#[derive(Debug, Default)]
pub(crate) struct DockViewportRoutedDropPreviewState {
    preview: Option<DockViewportRoutedDropPreview>,
    resolution: Option<DockViewportResolvedDropRoute>,
    epoch: u64,
    accepted: Option<DockViewportAcceptedRoutedPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportRoutedDropPreviewReplacement {
    changed: bool,
    affected_spaces: Vec<DockSpaceId>,
}

impl DockViewportRoutedDropPreviewReplacement {
    fn unchanged() -> Self {
        Self {
            changed: false,
            affected_spaces: Vec::new(),
        }
    }

    fn changed(affected_spaces: Vec<DockSpaceId>) -> Self {
        Self {
            changed: true,
            affected_spaces,
        }
    }

    pub(crate) fn has_changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn affected_spaces(&self) -> &[DockSpaceId] {
        &self.affected_spaces
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportAcceptedRoutedPreview {
    epoch: u64,
    target: DockViewportResolvedDropTargetSnapshot,
    drag_session_id: Option<u64>,
}

impl DockViewportAcceptedRoutedPreview {
    fn new(
        epoch: u64,
        target: &DockViewportResolvedDropTargetSnapshot,
        drag_session_id: Option<u64>,
    ) -> Option<Self> {
        target.target_window_id()?;
        Some(Self {
            epoch,
            target: target.clone(),
            drag_session_id,
        })
    }

    pub(crate) fn target(&self) -> &DockViewportResolvedDropTargetSnapshot {
        &self.target
    }

    pub(crate) fn target_key(&self) -> &DockDropTargetKey {
        self.target.target_key()
    }

    pub(crate) fn target_window_id(&self) -> Option<WindowId> {
        self.target.target_window_id()
    }

    pub(crate) fn matches_target(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.target.target_space() == space && self.target.target_window_id() == Some(window_id)
    }
}

impl DockViewportRoutedDropPreviewState {
    pub(crate) fn has_preview(&self) -> bool {
        self.preview.is_some()
    }

    pub(crate) fn preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.preview
            .as_ref()
            .filter(|preview| preview.matches(space, window_id))
            .cloned()
    }

    pub(crate) fn resolution_target_snapshot(
        &self,
    ) -> Option<&DockViewportResolvedDropTargetSnapshot> {
        self.resolution
            .as_ref()
            .and_then(DockViewportResolvedDropRoute::routed_preview_target_snapshot)
    }

    pub(crate) fn start_acceptance_pass(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.accepted = None;
    }

    pub(crate) fn replace(
        &mut self,
        next_preview: Option<DockViewportRoutedDropPreview>,
        next_resolution: Option<DockViewportResolvedDropRoute>,
    ) -> DockViewportRoutedDropPreviewReplacement {
        if self.preview == next_preview && self.resolution == next_resolution {
            return DockViewportRoutedDropPreviewReplacement::unchanged();
        }

        let mut affected_spaces = Vec::new();
        if self.preview != next_preview {
            if let Some(current) = self.preview.as_ref() {
                push_unique_space(&mut affected_spaces, current.space());
            }
            if let Some(next) = next_preview.as_ref() {
                push_unique_space(&mut affected_spaces, next.space());
            }
        }

        self.preview = next_preview;
        self.resolution = next_resolution;
        self.accepted = None;
        DockViewportRoutedDropPreviewReplacement::changed(affected_spaces)
    }

    pub(crate) fn finish_acceptance_pass(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        // Mirrors ImGui's AcceptBeforeDelivery path: preview creates a candidate, target render
        // accepts it, and release may only replay the accepted candidate.
        let Some(preview) = self.preview.as_ref() else {
            return false;
        };
        if !preview.matches(space, window_id) {
            return false;
        }
        let Some(resolution) = self.resolution.as_ref() else {
            return false;
        };
        if !matches!(
            resolution.route(),
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. }
        ) {
            return false;
        }
        let Some(target) = resolution.routed_preview_target_snapshot() else {
            return false;
        };
        if target.target_space() != space || target.target_window_id() != Some(window_id) {
            return false;
        }
        let Some(token) =
            DockViewportAcceptedRoutedPreview::new(self.epoch, target, preview.drag_session_id())
        else {
            return false;
        };
        self.accepted = Some(token);
        true
    }

    pub(crate) fn accepted_for_drag_session(
        &self,
        drag_session_id: u64,
    ) -> Option<&DockViewportAcceptedRoutedPreview> {
        let accepted = self.accepted.as_ref()?;
        (accepted.epoch == self.epoch && accepted.drag_session_id == Some(drag_session_id))
            .then_some(accepted)
    }

    pub(crate) fn targets_window(&self, window_id: WindowId) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|preview| preview.window_id() == window_id)
            || self
                .resolution
                .as_ref()
                .is_some_and(|resolution| resolution_targets_window(resolution, window_id))
    }

    pub(crate) fn clear_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
    ) -> DockViewportRoutedDropPreviewReplacement {
        let Some(session) = session else {
            return DockViewportRoutedDropPreviewReplacement::unchanged();
        };
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.drag_session_id() == Some(session.id()))
        {
            self.replace(None, None)
        } else {
            DockViewportRoutedDropPreviewReplacement::unchanged()
        }
    }

    #[cfg(test)]
    pub(crate) fn has_preview_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> bool {
        let Some(session) = session else {
            return false;
        };
        self.preview
            .as_ref()
            .and_then(DockViewportRoutedDropPreview::drag_session_id)
            == Some(session.id())
    }

    #[cfg(test)]
    pub(crate) fn is_currently_accepted(&self) -> bool {
        self.accepted
            .as_ref()
            .is_some_and(|accepted| accepted.epoch == self.epoch)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRoutedDropPreview {
    identity: DockViewportIdentity,
    pub(crate) preview: crate::drop_preview::DockDropPreview,
    drag_session_id: Option<u64>,
    pub(crate) payload_title: String,
}

impl DockViewportRoutedDropPreview {
    fn new(
        space: DockSpaceId,
        window_id: WindowId,
        preview: crate::drop_preview::DockDropPreview,
        drag_session_id: Option<u64>,
        payload_title: impl Into<String>,
    ) -> Self {
        Self {
            identity: DockViewportIdentity::new(space, window_id),
            preview,
            drag_session_id,
            payload_title: payload_title.into(),
        }
    }

    pub(crate) fn matches(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        self.identity.space()
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.identity.window_id()
    }

    pub(crate) fn drag_session_id(&self) -> Option<u64> {
        self.drag_session_id
    }
}

pub(crate) fn routed_drop_preview_from_target(
    target: &DockViewportResolvedDropTargetSnapshot,
    drag_session_id: Option<u64>,
    payload_title: String,
) -> Option<DockViewportRoutedDropPreview> {
    let window_id = target.target_window_id()?;
    let space = target.target_space().clone();
    let preview = crate::drop_preview::DockDropPreview::from_resolved_target(target.target())?;
    Some(DockViewportRoutedDropPreview::new(
        space,
        window_id,
        preview,
        drag_session_id,
        payload_title,
    ))
}

pub(crate) fn routed_rejected_drop_preview_from_target(
    target: &DockViewportResolvedDropTargetSnapshot,
    drag_session_id: Option<u64>,
    payload_title: String,
) -> Option<DockViewportRoutedDropPreview> {
    let window_id = target.target_window_id()?;
    let space = target.target_space().clone();
    let preview = crate::drop_preview::DockDropPreview::from_rejected_target(target.target())?;
    Some(DockViewportRoutedDropPreview::new(
        space,
        window_id,
        preview,
        drag_session_id,
        payload_title,
    ))
}

pub(crate) fn push_unique_window(
    windows: &mut Vec<AnyWindowHandle>,
    window: Option<AnyWindowHandle>,
) {
    let Some(window) = window else {
        return;
    };
    if windows
        .iter()
        .any(|existing| existing.window_id() == window.window_id())
    {
        return;
    }
    windows.push(window);
}

fn push_unique_space(spaces: &mut Vec<DockSpaceId>, space: &DockSpaceId) {
    if spaces.iter().any(|existing| existing == space) {
        return;
    }
    spaces.push(space.clone());
}

/// Extracts the viewport identity that a routed preview or rejected route should remember.
pub(crate) fn last_routed_viewport_identity_from_resolution(
    resolution: &DockViewportResolvedDropRoute,
    drag_session: Option<&DockRuntimeDragSession>,
) -> Option<DockViewportIdentity> {
    route_selection_viewport_identity_from_resolution(resolution, drag_session)
}

pub(crate) fn route_selection_viewport_identity_from_resolution(
    resolution: &DockViewportResolvedDropRoute,
    drag_session: Option<&DockRuntimeDragSession>,
) -> Option<DockViewportIdentity> {
    let resolution_drag_session_id = resolution
        .delivery()
        .and_then(|delivery| delivery.drag_session_id())
        .or_else(|| drag_session.map(DockRuntimeDragSession::id))?;
    if drag_session.is_some_and(|session| session.id() != resolution_drag_session_id) {
        return None;
    }

    let (target_space, target_window_id) = match resolution.route() {
        crate::DockViewportDropRoute::KnownViewport { target, source } => {
            if !source.records_routed_viewport_identity() {
                return None;
            }
            (target.space().clone(), target.window_id())
        }
        crate::DockViewportDropRoute::Local {
            window_id, source, ..
        } => {
            if !source.records_routed_viewport_identity() {
                return None;
            }
            let target = resolution.routed_preview_target_snapshot()?;
            if target.target_window_id() != Some(*window_id) {
                return None;
            }
            (target.target_space().clone(), *window_id)
        }
        crate::DockViewportDropRoute::Rejected(_) => {
            let target = resolution.routed_preview_target_snapshot()?;
            let window_id = target.target_window_id()?;
            (target.target_space().clone(), window_id)
        }
        crate::DockViewportDropRoute::TearOff | crate::DockViewportDropRoute::Unavailable => {
            return None;
        }
    };

    Some(DockViewportIdentity::new(target_space, target_window_id))
}

pub(crate) fn resolution_targets_window(
    resolution: &DockViewportResolvedDropRoute,
    window_id: WindowId,
) -> bool {
    resolution
        .routed_preview_target_snapshot()
        .is_some_and(|target| target.target_window_id() == Some(window_id))
}
