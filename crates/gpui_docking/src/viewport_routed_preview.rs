use crate::{
    DockSpaceId, DockViewportIdentity, DockViewportResolvedDropRoute,
    DockViewportResolvedDropTargetSnapshot, interaction::DockRuntimeDragSession,
};
use open_gpui::{AnyWindowHandle, WindowId};

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

pub(crate) fn routed_drop_preview_from_delivery(
    delivery: &crate::DockDropDelivery,
    payload_title: String,
) -> Option<DockViewportRoutedDropPreview> {
    let (space, window_id, resolved) = delivery.routed_preview_target()?;
    let space = space.clone();
    let preview = crate::drop_preview::DockDropPreview::from_resolved_target(resolved)?;
    Some(DockViewportRoutedDropPreview::new(
        space,
        window_id,
        preview,
        delivery.drag_session_id(),
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

/// Extracts the viewport identity that a routed preview or rejected route should remember.
pub(crate) fn last_routed_viewport_identity_from_resolution(
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
        crate::DockViewportDropRoute::KnownViewport { target, authority } => {
            if !matches!(
                authority,
                crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow
                    | crate::DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene
                    | crate::DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback
                    | crate::DockViewportAuthorizedRouteAuthority::ZOrderFallback
                    | crate::DockViewportAuthorizedRouteAuthority::UniqueGeometryFallback
            ) {
                return None;
            }
            (target.space().clone(), target.window_id())
        }
        crate::DockViewportDropRoute::Local {
            window_id,
            authority,
            ..
        } => {
            if !matches!(
                authority,
                crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow
                    | crate::DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene
                    | crate::DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback
                    | crate::DockViewportAuthorizedRouteAuthority::ZOrderFallback
                    | crate::DockViewportAuthorizedRouteAuthority::UniqueGeometryFallback
            ) {
                return None;
            }
            let preview_target = resolution.delivery()?.routed_preview_target()?;
            let (target_space, routed_window_id, _) = preview_target;
            if routed_window_id != *window_id {
                return None;
            }
            (target_space.clone(), *window_id)
        }
        crate::DockViewportDropRoute::Rejected(_) => {
            let target = resolution.preview_target()?;
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
        .delivery()
        .and_then(|delivery| delivery.routed_preview_target())
        .is_some_and(|(_, target_window_id, _)| target_window_id == window_id)
}
