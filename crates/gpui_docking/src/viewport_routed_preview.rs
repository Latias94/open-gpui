use crate::{
    DockSpaceId, DockViewportIdentity, DockViewportResolvedDropRoute,
    DockViewportResolvedDropTargetSnapshot, drop_preview::DockDropRoutePreview,
    interaction::DockRuntimeDragSession,
};
use open_gpui::{AnyWindowHandle, Point, WindowId};

#[derive(Debug, Default)]
pub(crate) struct DockViewportRoutedDropPreviewState {
    preview: Option<DockViewportRoutedDropPreview>,
    route_preview: Option<DockViewportRoutePreview>,
    resolution: Option<DockViewportResolvedDropRoute>,
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

impl DockViewportRoutedDropPreviewState {
    pub(crate) fn has_preview(&self) -> bool {
        self.preview.is_some() || self.route_preview.is_some()
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

    pub(crate) fn route_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockDropRoutePreview> {
        self.route_preview
            .as_ref()
            .filter(|preview| preview.matches(space, window_id))
            .map(|preview| preview.preview.clone())
    }

    pub(crate) fn replace(
        &mut self,
        next_preview: Option<DockViewportRoutedDropPreview>,
        next_route_preview: Option<DockViewportRoutePreview>,
        next_resolution: Option<DockViewportResolvedDropRoute>,
    ) -> DockViewportRoutedDropPreviewReplacement {
        if self.preview == next_preview
            && self.route_preview == next_route_preview
            && self.resolution == next_resolution
        {
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
        if self.route_preview != next_route_preview {
            if let Some(current) = self.route_preview.as_ref() {
                push_unique_space(&mut affected_spaces, current.space());
            }
            if let Some(next) = next_route_preview.as_ref() {
                push_unique_space(&mut affected_spaces, next.space());
            }
        }

        self.preview = next_preview;
        self.route_preview = next_route_preview;
        self.resolution = next_resolution;
        DockViewportRoutedDropPreviewReplacement::changed(affected_spaces)
    }

    pub(crate) fn targets_window(&self, window_id: WindowId) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|preview| preview.window_id() == window_id)
            || self
                .route_preview
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
            || self
                .route_preview
                .as_ref()
                .is_some_and(|preview| preview.drag_session_id() == Some(session.id()))
        {
            self.replace(None, None, None)
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
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRoutePreview {
    identity: DockViewportIdentity,
    preview: DockDropRoutePreview,
    drag_session_id: Option<u64>,
}

impl DockViewportRoutePreview {
    pub(crate) fn new(
        space: DockSpaceId,
        window_id: WindowId,
        preview: DockDropRoutePreview,
        drag_session_id: Option<u64>,
    ) -> Self {
        Self {
            identity: DockViewportIdentity::new(space, window_id),
            preview,
            drag_session_id,
        }
    }

    fn matches(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    fn space(&self) -> &DockSpaceId {
        self.identity.space()
    }

    fn window_id(&self) -> WindowId {
        self.identity.window_id()
    }

    fn drag_session_id(&self) -> Option<u64> {
        self.drag_session_id
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

pub(crate) fn routed_drop_route_preview_for_host(
    resolution: &DockViewportResolvedDropRoute,
    space: DockSpaceId,
    window_id: WindowId,
    host_position: Point<open_gpui::Pixels>,
    drag_session_id: Option<u64>,
) -> Option<DockViewportRoutePreview> {
    DockDropRoutePreview::from_route(resolution.route(), host_position)
        .map(|preview| DockViewportRoutePreview::new(space, window_id, preview, drag_session_id))
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
