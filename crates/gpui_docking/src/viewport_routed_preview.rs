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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportLastHoveredIdentity {
    identity: DockViewportIdentity,
    drag_session_id: u64,
}

impl DockViewportLastHoveredIdentity {
    pub(crate) fn window_id(&self) -> WindowId {
        self.identity.window_id()
    }

    pub(crate) fn drag_session_id(&self) -> u64 {
        self.drag_session_id
    }

    pub(crate) fn identity(&self) -> &DockViewportIdentity {
        &self.identity
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
    payload_title: String,
) -> Option<DockViewportRoutedDropPreview> {
    let window_id = target.target_window_id()?;
    let space = target.target_space().clone();
    let preview = crate::drop_preview::DockDropPreview::from_rejected_target(target.target())?;
    Some(DockViewportRoutedDropPreview::new(
        space,
        window_id,
        preview,
        None,
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

pub(crate) fn last_hovered_identity_from_resolution(
    resolution: &DockViewportResolvedDropRoute,
    drag_session: Option<&DockRuntimeDragSession>,
) -> Option<DockViewportLastHoveredIdentity> {
    let (target_space, target_window_id, authority) = match resolution.route() {
        crate::DockViewportDropRoute::KnownViewport { target, authority } => {
            (target.space().clone(), target.window_id(), *authority)
        }
        crate::DockViewportDropRoute::Local {
            window_id,
            authority,
            ..
        } => {
            let preview_target = resolution.delivery()?.routed_preview_target()?;
            let (target_space, routed_window_id, _) = preview_target;
            if routed_window_id != *window_id {
                return None;
            }
            (target_space.clone(), *window_id, *authority)
        }
        crate::DockViewportDropRoute::Rejected(_) => {
            let target = resolution.preview_target()?;
            let window_id = target.target_window_id()?;
            (
                target.target_space().clone(),
                window_id,
                crate::DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
            )
        }
        crate::DockViewportDropRoute::TearOff | crate::DockViewportDropRoute::Unavailable => {
            return None;
        }
    };
    let drag_session_id = resolution
        .delivery()
        .and_then(|delivery| delivery.drag_session_id())
        .or_else(|| drag_session.map(DockRuntimeDragSession::id))?;
    if !matches!(
        authority,
        crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow
            | crate::DockViewportAuthorizedRouteAuthority::DragLastHoveredViewport
            | crate::DockViewportAuthorizedRouteAuthority::BackendHoverFallback
    ) {
        return None;
    }
    Some(DockViewportLastHoveredIdentity {
        identity: DockViewportIdentity::new(target_space, target_window_id),
        drag_session_id,
    })
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
