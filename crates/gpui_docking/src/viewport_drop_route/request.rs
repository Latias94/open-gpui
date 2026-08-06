use crate::{
    DockNodeId, DockSpaceId, DockViewportDropPayload, DockViewportFocusStampFallbackPermit,
    DockViewportPlatformSignals, DockViewportTargetContext,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    interaction::{DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
    viewport_drop_scene::DockViewportHostSceneFrame,
    viewport_registry::DockViewportWindowBoundsFrame,
};
#[cfg(test)]
use open_gpui::Bounds;
use open_gpui::{
    AnyWindowHandle, App, NativeCapturedDragGeneration, NativeIngressSequence, Pixels, Point,
    WindowBounds, WindowId,
};

/// Exact route selected from one GPUI captured-drag native hit-stack fact.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockCapturedNativeDropRoute {
    /// The frontmost registered window contains one current Dock host scene.
    Host(DockCapturedNativeHostTarget),
    /// The current host scene belongs to another independent Dock surface.
    ForbiddenTarget(DockCapturedNativeHostTarget),
    /// The point is not covered by an eligible Dock host and may tear off to the desktop.
    Desktop,
    /// The native point observation could not prove one current route target.
    Unavailable,
}

/// Current host-scene proof selected inside the registered target window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockCapturedNativeHostTarget {
    target_window: AnyWindowHandle,
    target_space: DockSpaceId,
    host_position: Point<Pixels>,
    scene_frame: DockViewportHostSceneFrame,
}

impl DockCapturedNativeHostTarget {
    pub(crate) fn new(
        target_window: AnyWindowHandle,
        target_space: DockSpaceId,
        host_position: Point<Pixels>,
        scene_frame: DockViewportHostSceneFrame,
    ) -> Self {
        Self {
            target_window,
            target_space,
            host_position,
            scene_frame,
        }
    }

    pub(crate) fn target_window(&self) -> AnyWindowHandle {
        self.target_window
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn host_position(&self) -> Point<Pixels> {
        self.host_position
    }

    pub(crate) fn scene_frame(&self) -> &DockViewportHostSceneFrame {
        &self.scene_frame
    }
}

/// Coordinate space used by `DockViewportDropRouteRequest::release_position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportPointerCoordinateSpace {
    /// `release_position` is a screen-space point and may be geometry hit-tested globally.
    GlobalScreen,
    /// `release_position` is local to the trusted hovered window.
    TrustedHoveredWindowLocal,
    /// `release_position` is local to the event-receiver window, but no trusted hovered-window
    /// signal proves that the receiver is the hovered window.
    EventReceiverLocal,
    /// `release_position` is local to the source host only.
    SourceLocalOnly,
}

/// All routing and payload facts needed to route one rendered drop release.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportDropRouteRequest {
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    drag_session: Option<DockRuntimeDragSession>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    suggested_window_bounds: Option<WindowBounds>,
    release_position: Point<Pixels>,
    coordinate_space: DockViewportPointerCoordinateSpace,
    release_origin: DockPayloadDropReleaseOrigin,
    event_receiver_local_scene_proof: Option<DockViewportHostSceneFrame>,
    captured_native_route: Option<DockCapturedNativeDropRoute>,
    captured_native_generation: Option<NativeCapturedDragGeneration>,
    captured_native_sequence: Option<NativeIngressSequence>,
    platform_signals: DockViewportPlatformSignals,
}

/// Raw pointer release facts before viewport routing normalizes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportDropReleasePoint {
    host_position: Point<Pixels>,
    host_window_bounds: DockViewportWindowBoundsFrame,
}

impl DockViewportDropRouteRequest {
    fn new(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        suggested_window_bounds: Option<WindowBounds>,
        release_position: Point<Pixels>,
        coordinate_space: DockViewportPointerCoordinateSpace,
        release_origin: DockPayloadDropReleaseOrigin,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_node,
            payload,
            drag_session: None,
            tear_off_geometry: None,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            event_receiver_local_scene_proof: None,
            captured_native_route: None,
            captured_native_generation: None,
            captured_native_sequence: None,
            platform_signals,
        }
    }

    pub(crate) fn from_captured_native_route(
        payload: &DockDragPayload,
        drag_session: DockRuntimeDragSession,
        source_window: AnyWindowHandle,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
        suggested_window_bounds: Option<WindowBounds>,
        source_local_position: Point<Pixels>,
        route: DockCapturedNativeDropRoute,
        generation: NativeCapturedDragGeneration,
        sequence: NativeIngressSequence,
        cx: &App,
    ) -> Self {
        let platform_signals = DockViewportPlatformSignals::from_captured_native_transport(cx)
            .with_frame_sampling_exclusion_window(source_window);
        Self::new(
            payload.source_space.clone(),
            payload.source_node,
            DockViewportDropPayload::from_drag_payload(payload),
            suggested_window_bounds,
            source_local_position,
            DockViewportPointerCoordinateSpace::SourceLocalOnly,
            DockPayloadDropReleaseOrigin::SourceOnly,
            platform_signals,
        )
        .with_drag_session(Some(drag_session))
        .with_tear_off_geometry(tear_off_geometry)
        .with_captured_native_route(route, generation, sequence)
    }

    fn with_captured_native_route(
        mut self,
        route: DockCapturedNativeDropRoute,
        generation: NativeCapturedDragGeneration,
        sequence: NativeIngressSequence,
    ) -> Self {
        self.captured_native_route = Some(route);
        self.captured_native_generation = Some(generation);
        self.captured_native_sequence = Some(sequence);
        self
    }

    #[cfg(test)]
    pub(crate) fn from_platform_signals(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        let release_origin = DockPayloadDropReleaseOrigin::HoveredHost;
        let coordinate_space = if platform_signals.has_global_window_bounds() {
            DockViewportPointerCoordinateSpace::GlobalScreen
        } else {
            Self::local_coordinate_space_for_origin(
                release_origin,
                &platform_signals.target_context(),
                platform_signals.event_receiver_window(),
            )
        };
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            platform_signals,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_platform_signals_with_origin(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        release_origin: DockPayloadDropReleaseOrigin,
    ) -> Self {
        let coordinate_space = if platform_signals.has_global_window_bounds() {
            DockViewportPointerCoordinateSpace::GlobalScreen
        } else {
            Self::local_coordinate_space_for_origin(
                release_origin,
                &platform_signals.target_context(),
                platform_signals.event_receiver_window(),
            )
        };
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            platform_signals,
        )
    }

    pub(crate) fn from_host_release(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_point: DockViewportDropReleasePoint,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        release_origin: DockPayloadDropReleaseOrigin,
    ) -> Self {
        let (release_position, coordinate_space) = if platform_signals.has_global_window_bounds() {
            if let Some(host_window_bounds) =
                release_point.host_window_bounds.global_screen_bounds()
            {
                (
                    open_gpui::point(
                        host_window_bounds.origin.x + release_point.host_position.x,
                        host_window_bounds.origin.y + release_point.host_position.y,
                    ),
                    DockViewportPointerCoordinateSpace::GlobalScreen,
                )
            } else {
                (
                    release_point.host_position,
                    Self::local_coordinate_space_for_origin(
                        release_origin,
                        &platform_signals.target_context(),
                        platform_signals.event_receiver_window(),
                    ),
                )
            }
        } else {
            (
                release_point.host_position,
                Self::local_coordinate_space_for_origin(
                    release_origin,
                    &platform_signals.target_context(),
                    platform_signals.event_receiver_window(),
                ),
            )
        };
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            platform_signals,
        )
    }

    pub(crate) fn with_drag_session(
        mut self,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        self.drag_session = drag_session;
        self
    }

    pub(crate) fn with_tear_off_geometry(
        mut self,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
    ) -> Self {
        self.tear_off_geometry = tear_off_geometry;
        self
    }

    pub(crate) fn with_event_receiver_local_scene_proof(
        mut self,
        proof: Option<DockViewportHostSceneFrame>,
    ) -> Self {
        self.event_receiver_local_scene_proof =
            if self.release_origin == DockPayloadDropReleaseOrigin::HoveredHost {
                proof
            } else {
                None
            };
        self
    }

    pub(crate) fn with_resampled_platform_target_context_from_app(
        mut self,
        cx: &open_gpui::App,
    ) -> Self {
        self.platform_signals = self
            .platform_signals
            .with_resampled_target_context_from_app(cx);
        self
    }

    pub(crate) fn with_focus_stamp_window_stack(
        mut self,
        windows: impl IntoIterator<Item = WindowId>,
    ) -> Self {
        self.platform_signals = self.platform_signals.with_focus_stamp_window_stack(windows);
        self
    }

    pub(crate) fn with_focus_stamp_fallback_permit(
        mut self,
        permit: DockViewportFocusStampFallbackPermit,
    ) -> Self {
        self.platform_signals = self
            .platform_signals
            .with_focus_stamp_fallback_permit(permit);
        self
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    pub(crate) fn source_node(&self) -> DockNodeId {
        self.source_node
    }

    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        &self.payload
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
    }

    pub(crate) fn release_position(&self) -> Point<Pixels> {
        self.release_position
    }

    pub(crate) fn tear_off_geometry(&self) -> Option<DockDragTearOffGeometry> {
        self.tear_off_geometry
    }

    pub(crate) fn suggested_window_bounds(&self) -> Option<WindowBounds> {
        self.suggested_window_bounds
    }

    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        self.platform_signals.target_context()
    }

    pub(crate) fn event_receiver_window(&self) -> Option<WindowId> {
        self.platform_signals.event_receiver_window()
    }

    pub(crate) fn frame_sampling_exclusion_window(&self) -> Option<WindowId> {
        self.platform_signals.frame_sampling_exclusion_window()
    }

    pub(crate) fn allows_focus_stamp_fallback(&self) -> bool {
        self.platform_signals.allows_focus_stamp_fallback()
    }

    pub(crate) fn supports_platform_viewport_windows(&self) -> bool {
        self.platform_signals.supports_platform_viewport_windows()
    }

    pub(crate) fn coordinate_space(&self) -> DockViewportPointerCoordinateSpace {
        self.coordinate_space
    }

    pub(crate) fn release_origin(&self) -> DockPayloadDropReleaseOrigin {
        self.release_origin
    }

    pub(crate) fn event_receiver_local_scene_proof(&self) -> Option<&DockViewportHostSceneFrame> {
        self.event_receiver_local_scene_proof.as_ref()
    }

    pub(crate) fn captured_native_route(&self) -> Option<&DockCapturedNativeDropRoute> {
        self.captured_native_route.as_ref()
    }

    pub(crate) fn captured_native_generation(&self) -> Option<NativeCapturedDragGeneration> {
        self.captured_native_generation
    }

    pub(crate) fn captured_native_sequence(&self) -> Option<NativeIngressSequence> {
        self.captured_native_sequence
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: DockViewportTargetContext,
    ) -> Self {
        let platform_signals =
            DockViewportPlatformSignals::from_target_context(target_context.clone());
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            DockViewportPointerCoordinateSpace::GlobalScreen,
            DockPayloadDropReleaseOrigin::HoveredHost,
            platform_signals,
        )
    }

    fn local_coordinate_space_for_origin(
        release_origin: DockPayloadDropReleaseOrigin,
        target_context: &DockViewportTargetContext,
        event_receiver_window: Option<WindowId>,
    ) -> DockViewportPointerCoordinateSpace {
        match release_origin {
            DockPayloadDropReleaseOrigin::HoveredHost => {
                if target_context
                    .trusted_hovered_window_matches_event_receiver(event_receiver_window)
                {
                    DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal
                } else {
                    DockViewportPointerCoordinateSpace::EventReceiverLocal
                }
            }
            DockPayloadDropReleaseOrigin::SourceOnly => {
                DockViewportPointerCoordinateSpace::SourceLocalOnly
            }
        }
    }
}

impl DockViewportDropReleasePoint {
    #[cfg(test)]
    pub(crate) fn host_local(
        host_position: Point<Pixels>,
        host_window_bounds: Bounds<Pixels>,
    ) -> Self {
        Self::host_local_with_bounds_frame(
            host_position,
            DockViewportWindowBoundsFrame::GlobalScreen(host_window_bounds),
        )
    }

    pub(crate) fn host_local_with_bounds_frame(
        host_position: Point<Pixels>,
        host_window_bounds: DockViewportWindowBoundsFrame,
    ) -> Self {
        Self {
            host_position,
            host_window_bounds,
        }
    }
}
