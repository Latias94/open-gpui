use crate::{
    DockSpaceId, DockViewportIdentity, DockViewportResolvedDropRoute,
    DockViewportResolvedDropTargetSnapshot, DockViewportRouteProof, DockViewportRuntimeIdentity,
    drag::DockDragPayload, drop_preview::DockDropRoutePreview, interaction::DockRuntimeDragSession,
    viewport_drop_scene::DockViewportHostSceneFrame,
    viewport_registry::DockViewportRegistrationKey,
    viewport_runtime_effects::extend_unique_windows,
};
use open_gpui::{
    AnyWindowHandle, NativeCapturedDragGeneration, NativeIngressSequence, Point, WindowId,
};
use std::{cell::Cell, fmt, rc::Rc};

/// Exact authority that owns one routed preview projection.
#[derive(Clone)]
pub(crate) enum DockViewportRoutedPreviewOwner {
    Local(DockRuntimeDragSession),
    CapturedNative {
        source_runtime: DockViewportRuntimeIdentity,
        generation: NativeCapturedDragGeneration,
        sequence: NativeIngressSequence,
        session: DockRuntimeDragSession,
        latest_sequence: Rc<Cell<Option<NativeIngressSequence>>>,
    },
}

impl DockViewportRoutedPreviewOwner {
    pub(crate) fn captured_native(
        source_runtime: DockViewportRuntimeIdentity,
        generation: NativeCapturedDragGeneration,
        sequence: NativeIngressSequence,
        session: DockRuntimeDragSession,
        latest_sequence: Rc<Cell<Option<NativeIngressSequence>>>,
    ) -> Self {
        Self::CapturedNative {
            source_runtime,
            generation,
            sequence,
            session,
            latest_sequence,
        }
    }

    pub(crate) fn captured_native_parts(
        &self,
    ) -> Option<(
        DockViewportRuntimeIdentity,
        NativeCapturedDragGeneration,
        NativeIngressSequence,
        &DockRuntimeDragSession,
    )> {
        match self {
            Self::CapturedNative {
                source_runtime,
                generation,
                sequence,
                session,
                ..
            } => Some((*source_runtime, *generation, *sequence, session)),
            Self::Local(_) => None,
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        match self {
            Self::Local(_) => true,
            Self::CapturedNative {
                sequence,
                latest_sequence,
                ..
            } => latest_sequence.get() == Some(*sequence),
        }
    }

    fn matches_session(&self, session: &DockRuntimeDragSession) -> bool {
        match self {
            Self::Local(owner) => owner == session,
            Self::CapturedNative { session: owner, .. } => owner == session,
        }
    }
}

impl PartialEq for DockViewportRoutedPreviewOwner {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local(left), Self::Local(right)) => left == right,
            (
                Self::CapturedNative {
                    source_runtime: left_runtime,
                    generation: left_generation,
                    sequence: left_sequence,
                    session: left_session,
                    latest_sequence: left_latest,
                },
                Self::CapturedNative {
                    source_runtime: right_runtime,
                    generation: right_generation,
                    sequence: right_sequence,
                    session: right_session,
                    latest_sequence: right_latest,
                },
            ) => {
                left_runtime == right_runtime
                    && left_generation == right_generation
                    && left_sequence == right_sequence
                    && left_session == right_session
                    && Rc::ptr_eq(left_latest, right_latest)
            }
            (Self::Local(_), Self::CapturedNative { .. })
            | (Self::CapturedNative { .. }, Self::Local(_)) => false,
        }
    }
}

impl Eq for DockViewportRoutedPreviewOwner {}

impl fmt::Debug for DockViewportRoutedPreviewOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(session) => formatter.debug_tuple("Local").field(session).finish(),
            Self::CapturedNative {
                source_runtime,
                generation,
                sequence,
                session,
                ..
            } => formatter
                .debug_struct("CapturedNative")
                .field("source_runtime", source_runtime)
                .field("generation", generation)
                .field("sequence", sequence)
                .field("session", session)
                .finish(),
        }
    }
}

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
    #[cfg(test)]
    pub(crate) fn resolved_target_for_registration(
        &self,
        registration: &DockViewportRegistrationKey,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        self.resolution
            .as_ref()?
            .routed_preview_target_snapshot()
            .filter(|target| target.route_proof().registration_key() == registration)
            .map(|target| target.target().clone())
    }

    pub(crate) fn tab_reorder_hold_for_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<crate::DockViewportTabReorderHold> {
        let session = session?;
        let resolution = self.resolution.as_ref()?;
        if resolution.drag_session() != Some(session) {
            return None;
        }
        resolution.tab_reorder_hold()
    }

    pub(crate) fn preview_for_registration(
        &self,
        registration: &DockViewportRegistrationKey,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.preview
            .as_ref()
            .filter(|preview| preview.matches_registration(registration))
            .cloned()
    }

    pub(crate) fn route_preview_for_registration(
        &self,
        registration: &DockViewportRegistrationKey,
    ) -> Option<DockDropRoutePreview> {
        self.route_preview
            .as_ref()
            .filter(|preview| preview.matches_registration(registration))
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
            let render_changed = match (self.preview.as_ref(), next_preview.as_ref()) {
                (Some(current), Some(next)) => !current.renders_same_as(next),
                (None, None) => false,
                (Some(_), None) | (None, Some(_)) => true,
            };
            if render_changed {
                if let Some(current) = self.preview.as_ref() {
                    push_unique_space(&mut affected_spaces, current.space());
                }
                if let Some(next) = next_preview.as_ref() {
                    push_unique_space(&mut affected_spaces, next.space());
                }
            }
        }
        if self.route_preview != next_route_preview {
            let render_changed = match (self.route_preview.as_ref(), next_route_preview.as_ref()) {
                (Some(current), Some(next)) => !current.renders_same_as(next),
                (None, None) => false,
                (Some(_), None) | (None, Some(_)) => true,
            };
            if render_changed {
                if let Some(current) = self.route_preview.as_ref() {
                    push_unique_space(&mut affected_spaces, current.space());
                }
                if let Some(next) = next_route_preview.as_ref() {
                    push_unique_space(&mut affected_spaces, next.space());
                }
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
            .is_some_and(|preview| preview.is_owned_by_session(session))
            || self
                .route_preview
                .as_ref()
                .is_some_and(|preview| preview.is_owned_by_session(session))
        {
            self.replace(None, None, None)
        } else {
            DockViewportRoutedDropPreviewReplacement::unchanged()
        }
    }

    pub(crate) fn clear_for_owner(
        &mut self,
        owner: &DockViewportRoutedPreviewOwner,
    ) -> DockViewportRoutedDropPreviewReplacement {
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.owner.as_ref() == Some(owner))
            || self
                .route_preview
                .as_ref()
                .is_some_and(|preview| preview.owner.as_ref() == Some(owner))
        {
            self.replace(None, None, None)
        } else {
            DockViewportRoutedDropPreviewReplacement::unchanged()
        }
    }

    pub(crate) fn clear_for_target_scene_frame(
        &mut self,
        frame: &DockViewportHostSceneFrame,
    ) -> DockViewportRoutedDropPreviewReplacement {
        let targets_frame = self
            .resolution
            .as_ref()
            .and_then(DockViewportResolvedDropRoute::routed_preview_target_snapshot)
            .is_some_and(|target| target.frame() == frame);
        if targets_frame {
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
            .is_some_and(|preview| preview.is_owned_by_session(session))
            || self
                .route_preview
                .as_ref()
                .is_some_and(|preview| preview.is_owned_by_session(session))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRoutePreview {
    route_proof: DockViewportRouteProof,
    preview: DockDropRoutePreview,
    owner: Option<DockViewportRoutedPreviewOwner>,
}

impl DockViewportRoutePreview {
    pub(crate) fn new(
        route_proof: DockViewportRouteProof,
        preview: DockDropRoutePreview,
        owner: Option<DockViewportRoutedPreviewOwner>,
    ) -> Self {
        Self {
            route_proof,
            preview,
            owner,
        }
    }

    fn matches_registration(&self, registration: &DockViewportRegistrationKey) -> bool {
        self.route_proof.registration_key() == registration
    }

    fn renders_same_as(&self, other: &Self) -> bool {
        self.space() == other.space()
            && self.window_id() == other.window_id()
            && self.preview == other.preview
    }

    fn space(&self) -> &DockSpaceId {
        self.route_proof.space()
    }

    fn window_id(&self) -> WindowId {
        self.route_proof.window_id()
    }

    fn is_owned_by_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| owner.matches_session(session))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRoutedDropPreview {
    route_proof: DockViewportRouteProof,
    pub(crate) preview: crate::drop_preview::DockDropPreview,
    owner: Option<DockViewportRoutedPreviewOwner>,
}

impl DockViewportRoutedDropPreview {
    fn new(
        route_proof: DockViewportRouteProof,
        preview: crate::drop_preview::DockDropPreview,
        owner: Option<DockViewportRoutedPreviewOwner>,
    ) -> Self {
        Self {
            route_proof,
            preview,
            owner,
        }
    }

    fn matches_registration(&self, registration: &DockViewportRegistrationKey) -> bool {
        self.route_proof.registration_key() == registration
    }

    fn renders_same_as(&self, other: &Self) -> bool {
        self.space() == other.space()
            && self.window_id() == other.window_id()
            && self.preview == other.preview
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        self.route_proof.space()
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.route_proof.window_id()
    }

    fn is_owned_by_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| owner.matches_session(session))
    }
}

pub(crate) fn routed_drop_preview_from_target(
    target: &DockViewportResolvedDropTargetSnapshot,
    owner: Option<DockViewportRoutedPreviewOwner>,
    payload: &DockDragPayload,
) -> Option<DockViewportRoutedDropPreview> {
    let window_id = target.target_window_id()?;
    let route_proof = target.route_proof();
    if route_proof.space() != target.target_space() || route_proof.window_id() != window_id {
        return None;
    }
    let mut preview = if target.is_preview_only() {
        crate::drop_preview::DockDropPreview::from_guide_target(
            target.target(),
            target.drop_guide_metrics(),
        )?
    } else {
        crate::drop_preview::DockDropPreview::from_resolved_target(
            target.target(),
            target.drop_guide_metrics(),
        )?
    };
    preview.populate_payload_tabs(payload);
    Some(DockViewportRoutedDropPreview::new(
        route_proof.clone(),
        preview,
        owner,
    ))
}

pub(crate) fn routed_rejected_drop_preview_from_target(
    target: &DockViewportResolvedDropTargetSnapshot,
    owner: Option<DockViewportRoutedPreviewOwner>,
    payload: &DockDragPayload,
) -> Option<DockViewportRoutedDropPreview> {
    let window_id = target.target_window_id()?;
    let route_proof = target.route_proof();
    if route_proof.space() != target.target_space() || route_proof.window_id() != window_id {
        return None;
    }
    let mut preview = crate::drop_preview::DockDropPreview::from_rejected_target(
        target.target(),
        target.drop_guide_metrics(),
    )?;
    preview.populate_payload_tabs(payload);
    Some(DockViewportRoutedDropPreview::new(
        route_proof.clone(),
        preview,
        owner,
    ))
}

pub(crate) fn routed_drop_route_preview_for_host(
    resolution: &DockViewportResolvedDropRoute,
    route_proof: DockViewportRouteProof,
    host_position: Point<open_gpui::Pixels>,
    owner: Option<DockViewportRoutedPreviewOwner>,
) -> Option<DockViewportRoutePreview> {
    DockDropRoutePreview::from_route(resolution.route(), host_position)
        .map(|preview| DockViewportRoutePreview::new(route_proof, preview, owner))
}

pub(crate) fn push_unique_window(
    windows: &mut Vec<AnyWindowHandle>,
    window: Option<AnyWindowHandle>,
) {
    extend_unique_windows(windows, window);
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
            (
                target.route_proof().space().clone(),
                target.route_proof().window_id(),
            )
        }
        crate::DockViewportDropRoute::Local {
            route_proof,
            source,
            ..
        } => {
            if !source.records_routed_viewport_identity() {
                return None;
            }
            let target = resolution.routed_preview_target_snapshot()?;
            if target.target_window_id() != Some(route_proof.window_id())
                || target.target_space() != route_proof.space()
            {
                return None;
            }
            (route_proof.space().clone(), route_proof.window_id())
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
