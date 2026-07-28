use crate::{
    DockActionApplyError, DockDragVisualStyle, DockItemId, DockViewportIdentity,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    interaction::DockRuntimeDragSession,
};
use open_gpui::{AnyWindowHandle, WindowId};

#[derive(Debug, Default)]
pub(crate) struct DockViewportPayloadDragState {
    active: Option<DockViewportActivePayloadDrag>,
    tear_off_geometry: Option<DockRuntimeDragTearOffGeometry>,
    next_session_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockRuntimeDragTearOffGeometry {
    drag_session_id: u64,
    geometry: DockDragTearOffGeometry,
}

#[derive(Debug, Clone, PartialEq)]
struct DockViewportActivePayloadDrag {
    session: DockRuntimeDragSession,
    drag_visual_style: DockDragVisualStyle,
    source_window: Option<AnyWindowHandle>,
    last_routed_viewport_identity: Option<DockViewportIdentity>,
    last_hovered_viewport_identity: Option<DockViewportIdentity>,
}

impl DockViewportPayloadDragState {
    pub(crate) fn begin(
        &mut self,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
        source_window: Option<AnyWindowHandle>,
        drag_visual_style: DockDragVisualStyle,
    ) -> DockRuntimeDragSession {
        let session = self.next_session(payload, focus_item);
        self.active = Some(DockViewportActivePayloadDrag::new(
            session.clone(),
            source_window,
            drag_visual_style,
        ));
        self.tear_off_geometry = None;
        session
    }

    pub(crate) fn finish(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> Option<DockViewportPayloadDragFinish> {
        if !self.matches_session(Some(session)) {
            return None;
        }
        let active = self
            .active
            .take()
            .expect("active drag should match the requested session");
        let last_routed_viewport_identity = active.last_routed_viewport_identity.clone();
        let last_hovered_viewport_identity = active.last_hovered_viewport_identity.clone();
        if self
            .tear_off_geometry
            .is_some_and(|geometry| geometry.matches_drag_session(session))
        {
            self.tear_off_geometry = None;
        }
        Some(DockViewportPayloadDragFinish {
            last_routed_viewport_identity,
            last_hovered_viewport_identity,
        })
    }

    pub(crate) fn update_tear_off_geometry(
        &mut self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        if !self.matches_session(Some(session)) {
            return false;
        }
        let next = Some(DockRuntimeDragTearOffGeometry::new(session.id(), geometry));
        if self.tear_off_geometry == next {
            return false;
        }
        self.tear_off_geometry = next;
        true
    }

    pub(crate) fn tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        let session = session?;
        self.tear_off_geometry
            .filter(|geometry| geometry.matches_drag_session(session))
            .map(|geometry| geometry.geometry)
    }

    pub(crate) fn active_session_for_payload(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.active
            .as_ref()
            .filter(|drag| drag.accepts_payload(payload))
            .map(|drag| drag.session().clone())
    }

    pub(crate) fn active_source_window_id_for_payload(
        &self,
        payload: &DockDragPayload,
    ) -> Option<WindowId> {
        self.active
            .as_ref()
            .filter(|drag| drag.accepts_payload(payload))
            .and_then(DockViewportActivePayloadDrag::source_window)
            .map(|window| window.window_id())
    }

    pub(crate) fn validate_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Result<(), DockActionApplyError> {
        let Some(session) = session else {
            return Err(DockActionApplyError::DropDragSessionMissing);
        };
        if self.matches_session(Some(session)) {
            return Ok(());
        }
        Err(DockActionApplyError::DropDragSessionStale {
            session: session.id(),
        })
    }

    pub(crate) fn active_session(&self) -> Option<&DockRuntimeDragSession> {
        self.active
            .as_ref()
            .map(DockViewportActivePayloadDrag::session)
    }

    pub(crate) fn drag_visual_style(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<&DockDragVisualStyle> {
        let session = session?;
        self.active
            .as_ref()
            .filter(|drag| drag.matches_session(session))
            .map(DockViewportActivePayloadDrag::drag_visual_style)
    }

    pub(crate) fn matches_session(&self, session: Option<&DockRuntimeDragSession>) -> bool {
        let Some(session) = session else {
            return false;
        };
        self.active
            .as_ref()
            .is_some_and(|drag| drag.matches_session(session))
    }

    pub(crate) fn record_last_routed_viewport_identity(
        &mut self,
        identity: Option<DockViewportIdentity>,
    ) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        active.record_last_routed_viewport_identity(identity);
        true
    }

    pub(crate) fn record_last_hovered_viewport_identity(
        &mut self,
        identity: Option<DockViewportIdentity>,
    ) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        active.record_last_hovered_viewport_identity(identity);
        true
    }

    pub(crate) fn last_hovered_viewport_identity(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<&DockViewportIdentity> {
        let session = session?;
        self.active
            .as_ref()
            .filter(|drag| drag.matches_session(session))
            .and_then(DockViewportActivePayloadDrag::last_hovered_viewport_identity)
    }

    #[cfg(test)]
    pub(crate) fn last_routed_viewport_identity(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<&DockViewportIdentity> {
        let session = session?;
        self.active
            .as_ref()
            .filter(|drag| drag.matches_session(session))
            .and_then(DockViewportActivePayloadDrag::last_routed_viewport_identity)
    }

    pub(crate) fn clear_last_viewport_identity_if_window_matches(&mut self, window_id: WindowId) {
        if let Some(active) = self.active.as_mut() {
            active.clear_last_viewport_identity_if_window_matches(window_id);
        }
    }

    pub(crate) fn clear_last_viewport_identity_for_session(
        &mut self,
        session: &DockRuntimeDragSession,
    ) {
        if let Some(active) = self.active.as_mut() {
            active.clear_last_viewport_identity_for_session(session);
        }
    }

    fn next_session(
        &mut self,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
    ) -> DockRuntimeDragSession {
        let id = self.next_session_id.wrapping_add(1);
        self.next_session_id = id;
        DockRuntimeDragSession::with_focus_item(id, payload, focus_item)
    }
}

pub(crate) struct DockViewportPayloadDragFinish {
    last_routed_viewport_identity: Option<DockViewportIdentity>,
    last_hovered_viewport_identity: Option<DockViewportIdentity>,
}

impl DockViewportPayloadDragFinish {
    pub(crate) fn last_routed_viewport_identity(&self) -> Option<&DockViewportIdentity> {
        self.last_routed_viewport_identity.as_ref()
    }

    pub(crate) fn last_hovered_viewport_identity(&self) -> Option<&DockViewportIdentity> {
        self.last_hovered_viewport_identity.as_ref()
    }
}

impl DockRuntimeDragTearOffGeometry {
    fn new(drag_session_id: u64, geometry: DockDragTearOffGeometry) -> Self {
        Self {
            drag_session_id,
            geometry,
        }
    }

    fn matches_drag_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.drag_session_id == session.id()
    }
}

impl DockViewportActivePayloadDrag {
    fn new(
        session: DockRuntimeDragSession,
        source_window: Option<AnyWindowHandle>,
        drag_visual_style: DockDragVisualStyle,
    ) -> Self {
        Self {
            session,
            drag_visual_style,
            source_window,
            last_routed_viewport_identity: None,
            last_hovered_viewport_identity: None,
        }
    }

    fn session(&self) -> &DockRuntimeDragSession {
        &self.session
    }

    fn drag_visual_style(&self) -> &DockDragVisualStyle {
        &self.drag_visual_style
    }

    fn source_window(&self) -> Option<AnyWindowHandle> {
        self.source_window
    }

    fn matches_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.session == *session
    }

    fn accepts_payload(&self, payload: &DockDragPayload) -> bool {
        self.session.accepts_payload(payload)
    }

    fn record_last_routed_viewport_identity(&mut self, identity: Option<DockViewportIdentity>) {
        self.last_routed_viewport_identity = identity;
    }

    fn record_last_hovered_viewport_identity(&mut self, identity: Option<DockViewportIdentity>) {
        if let Some(identity) = identity {
            self.last_hovered_viewport_identity = Some(identity);
        }
    }

    #[cfg(test)]
    fn last_routed_viewport_identity(&self) -> Option<&DockViewportIdentity> {
        self.last_routed_viewport_identity.as_ref()
    }

    fn last_hovered_viewport_identity(&self) -> Option<&DockViewportIdentity> {
        self.last_hovered_viewport_identity.as_ref()
    }

    fn clear_last_viewport_identity_if_window_matches(&mut self, window_id: WindowId) {
        if self
            .last_routed_viewport_identity
            .as_ref()
            .is_some_and(|identity| identity.window_id() == window_id)
        {
            self.last_routed_viewport_identity = None;
        }
        if self
            .last_hovered_viewport_identity
            .as_ref()
            .is_some_and(|identity| identity.window_id() == window_id)
        {
            self.last_hovered_viewport_identity = None;
        }
    }

    fn clear_last_viewport_identity_for_session(&mut self, session: &DockRuntimeDragSession) {
        if self.matches_session(session) {
            self.last_routed_viewport_identity = None;
            self.last_hovered_viewport_identity = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockNodeId, DockSpaceId, DockVisualPalette};
    use open_gpui::rgb;
    use slotmap::Key;

    fn payload() -> DockDragPayload {
        DockDragPayload::new_tabs(
            DockSpaceId::from("main"),
            DockNodeId::null(),
            "Source tabs".to_string(),
        )
    }

    fn drag_style(surface: u32) -> DockDragVisualStyle {
        let mut palette = DockVisualPalette::built_in();
        palette.surface = rgb(surface);
        palette.surface_muted = rgb(surface);
        crate::DockVisualStyle::from_palette(palette).drag
    }

    #[test]
    fn drag_visual_style_is_keyed_by_session_and_cleared_on_finish() {
        let payload = payload();
        let first_style = drag_style(0x112233);
        let second_style = drag_style(0x445566);
        let mut state = DockViewportPayloadDragState::default();

        let first = state.begin(&payload, None, None, first_style.clone());
        assert_eq!(state.drag_visual_style(Some(&first)), Some(&first_style));

        let second = state.begin(&payload, None, None, second_style.clone());
        assert_ne!(first.id(), second.id());
        assert_eq!(state.drag_visual_style(Some(&first)), None);
        assert_eq!(state.drag_visual_style(Some(&second)), Some(&second_style));

        assert!(state.finish(&second).is_some());
        assert_eq!(state.drag_visual_style(Some(&second)), None);
    }

    #[test]
    fn visual_snapshots_do_not_change_payload_identity() {
        let payload = payload();
        let identity = payload.identity();
        let mut state = DockViewportPayloadDragState::default();

        let first = state.begin(&payload, None, None, drag_style(0x112233));
        assert_eq!(payload.identity(), identity);
        assert!(state.validate_session(Some(&first)).is_ok());
        assert!(state.finish(&first).is_some());

        let second = state.begin(&payload, None, None, drag_style(0x445566));
        assert_eq!(payload.identity(), identity);
        assert_eq!(
            state.validate_session(Some(&first)),
            Err(DockActionApplyError::DropDragSessionStale {
                session: first.id(),
            })
        );
        assert!(state.validate_session(Some(&second)).is_ok());
        assert!(state.matches_session(Some(&second)));
    }
}
