use dockspace::{
    geometry::{PhysicalPoint, PhysicalRect},
    runtime::{
        NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerButton,
        NativePointerCancelReason, NativePointerEvent, NativePointerHover, NativePointerId,
        NativePointerInput, NativePointerLocation, NativePointerOwner, NativeWindowBinding,
        PointerEventMetadata,
    },
};
use open_gpui::{
    App, Bounds, DevicePixels, MouseButton, NativeCapturedDragEvent, NativeCapturedDragGeneration,
    NativeCapturedDragPhase, PlatformNativePointerCaptureOwner, PlatformWindowHit,
    PlatformWindowHitStack, PlatformWindowHitTerminus, PointerCaptureHandle, Window, WindowId,
};

use super::{NativeBridge, NativeBridgeError, NativeCallbacks};

/// Event-time facts from the native owner; delivery, hover and capture are independent.
#[derive(Clone, Copy, Debug)]
pub struct NativePointerFacts {
    /// Stable pointer identity in this managed provider.
    pub pointer: NativePointerId,
    /// Actual input lifecycle edge.
    pub event: NativePointerEvent,
    /// Original event-time coordinate lane and geometry capability.
    pub location: NativePointerLocation,
    /// Independently observed native event delivery owner.
    pub delivery: NativePointerOwner,
    /// Independently observed native capture owner; logical hitbox capture is insufficient.
    pub capture: NativePointerOwner,
    /// Event-time device, monotonic observation time and targeting directive.
    pub metadata: PointerEventMetadata,
}

impl NativePointerFacts {
    /// Preserves the native lane, including an explicit local-to-desktop handoff.
    pub fn into_input(self) -> NativePointerInput {
        match self.location {
            NativePointerLocation::Desktop(location) => NativePointerInput::new(
                self.pointer,
                self.event,
                location,
                self.delivery,
                self.capture,
            ),
            NativePointerLocation::NativeLocal(location) => NativePointerInput::new_local(
                self.pointer,
                self.event,
                location,
                self.delivery,
                self.capture,
            ),
            NativePointerLocation::LocalToDesktop(location) => NativePointerInput::new_handoff(
                self.pointer,
                self.event,
                location,
                self.delivery,
                self.capture,
            ),
        }
        .with_metadata(self.metadata)
    }
}

/// Exact GPUI captured-drag correlation, independent of the docking reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCapture {
    pub(super) source: WindowId,
    pub(super) generation: NativeCapturedDragGeneration,
}

impl NativeCapture {
    /// Captures the identities from an actual GPUI native drag callback.
    pub fn from_event(event: &NativeCapturedDragEvent) -> Self {
        Self {
            source: event.source_window(),
            generation: event.generation(),
        }
    }

    /// Returns GPUI's original full source-window id.
    pub fn source_window(self) -> WindowId {
        self.source
    }
    /// Returns the original unforgeable GPUI drag generation.
    pub fn generation(self) -> NativeCapturedDragGeneration {
        self.generation
    }
}

impl NativeCallbacks {
    /// Captures a framework hitbox owner. This grants no native OS-capture fact.
    ///
    /// Bind this handle in every U5 frame through `track_pointer_capture` or
    /// `bind_pointer_capture`. Physical cross-window delivery uses the original
    /// GPUI drag/native callback owner, not this hitbox capture result.
    pub fn capture_framework_pointer(
        &self,
        handle: &PointerCaptureHandle,
        button: MouseButton,
        window: &mut Window,
    ) -> Result<(), NativeBridgeError> {
        if self.window_id() != Some(handle.window_id()) {
            return Err(NativeBridgeError::UnknownBinding);
        }
        window
            .capture_pointer(handle, button)
            .map_err(|error| NativeBridgeError::WindowUpdate(error.to_string()))
    }
}

impl NativeBridge {
    pub(crate) fn continuation_input(
        &self,
        session: &dockspace::runtime::DockspaceSession,
        request: dockspace::runtime::NativePointerContinuation,
        frame: open_gpui::PlatformNativePointerPhysicalFrame,
        stack: &PlatformWindowHitStack,
        metadata: PointerEventMetadata,
    ) -> Option<NativePointerInput> {
        use dockspace::{
            geometry::{PhysicalSize, ScaleFactor},
            runtime::{NativeLocalGeometry, NativeLocalPointerFact},
        };

        let binding = request.source_binding().window();
        if !session.is_current_native_binding(request.source_binding())
            || native_capture_owner(frame.capture_owner(), |owner| {
                self.binding_for_window(owner)
            }) != NativePointerOwner::Native(binding)
        {
            return None;
        }
        let input = if let Some(coordinates) = request.local_coordinates() {
            let geometry = frame.source_geometry();
            let client = geometry.client_bounds();
            let scale = ScaleFactor::new(f64::from(geometry.scale_factor())).ok()?;
            let size = PhysicalSize::new(
                f64::from(client.size.width.0),
                f64::from(client.size.height.0),
            )
            .ok()?;
            let point = PhysicalPoint::new(
                f64::from(frame.global_position().x.0) - f64::from(client.origin.x.0),
                f64::from(frame.global_position().y.0) - f64::from(client.origin.y.0),
            )
            .ok()?;
            let local = coordinates
                .capture(
                    binding,
                    NativeLocalPointerFact::new(point, size, scale),
                    NativeLocalGeometry::new(size, scale),
                )
                .ok()?;
            NativePointerInput::new_local(
                request.pointer(),
                request.event(),
                dockspace::runtime::NativeLocalPointerLocation::Exact(local),
                NativePointerOwner::Native(binding),
                NativePointerOwner::Native(binding),
            )
        } else {
            let position = frame.global_position();
            let hover = native_hover(stack, Some(position), |owner| {
                self.binding_for_window(owner)
            });
            let work_area = matches!(hover, NativePointerHover::OutsideAll)
                .then(|| self.desktop.work_area(session, frame))
                .flatten();
            NativePointerInput::new(
                request.pointer(),
                request.event(),
                NativeDesktopPointerLocation::new(
                    NativeDesktopPosition::Exact(
                        PhysicalPoint::new(f64::from(position.x.0), f64::from(position.y.0))
                            .ok()?,
                    ),
                    hover,
                    work_area,
                ),
                NativePointerOwner::Native(binding),
                NativePointerOwner::Native(binding),
            )
        };
        Some(input.with_metadata(metadata))
    }

    /// Translates an actual GPUI captured event from the application's existing router.
    ///
    /// Install one application router and route by full source-window id to the
    /// owning bridge. This method does not replace GPUI's application-level
    /// observer, start a second drag, poll hover later, or create a semantic drop.
    /// Initial press/local events must use this same managed producer; never call
    /// `submit_host_pointer` in a managed session.
    pub fn captured_drag(
        &mut self,
        event: &NativeCapturedDragEvent,
        pointer: NativePointerId,
        metadata: PointerEventMetadata,
        cx: &mut App,
    ) -> Result<(), NativeBridgeError> {
        let binding = self
            .binding_for_window(event.source_window())
            .ok_or(NativeBridgeError::UnknownBinding)?;
        let capture = native_capture_owner(event.capture_owner(), |owner| {
            self.binding_for_window(owner)
        });
        self.transport
            .captured_drag(binding, capture, event.clone(), pointer, metadata, cx);
        Ok(())
    }

    pub(super) fn captured_pointer_input(
        &self,
        session: &dockspace::runtime::DockspaceSession,
        binding: NativeWindowBinding,
        capture: NativePointerOwner,
        event: &NativeCapturedDragEvent,
        pointer: NativePointerId,
        metadata: PointerEventMetadata,
    ) -> NativePointerInput {
        let local = event.physical_frame().and_then(|frame| {
            use dockspace::{
                geometry::{PhysicalSize, ScaleFactor},
                runtime::{NativeLocalGeometry, NativeLocalPointerFact},
            };
            let geometry = frame.source_geometry();
            let bounds = geometry.client_bounds();
            let scale = ScaleFactor::new(f64::from(geometry.scale_factor())).ok()?;
            let size = PhysicalSize::new(
                f64::from(bounds.size.width.0),
                f64::from(bounds.size.height.0),
            )
            .ok()?;
            let point = PhysicalPoint::new(
                f64::from(frame.global_position().x.0) - f64::from(bounds.origin.x.0),
                f64::from(frame.global_position().y.0) - f64::from(bounds.origin.y.0),
            )
            .ok()?;
            session
                .native_local_coordinates(binding)?
                .capture(
                    binding,
                    NativeLocalPointerFact::new(point, size, scale),
                    NativeLocalGeometry::new(size, scale),
                )
                .ok()
        });
        let position = event
            .global_position()
            .and_then(|point| PhysicalPoint::new(f64::from(point.x.0), f64::from(point.y.0)).ok());
        let hover = native_hover(
            event.window_hit_stack(),
            event.global_position(),
            |window_id| self.binding_for_window(window_id),
        );
        let work_area = matches!(hover, NativePointerHover::OutsideAll)
            .then(|| {
                event
                    .physical_frame()
                    .and_then(|frame| self.desktop.work_area(session, frame))
            })
            .flatten();
        let location = NativeDesktopPointerLocation::new(
            position.map_or(NativeDesktopPosition::Unknown, NativeDesktopPosition::Exact),
            hover,
            work_area,
        );
        let edge = match event.phase() {
            NativeCapturedDragPhase::Moved => NativePointerEvent::Moved,
            NativeCapturedDragPhase::Released => {
                NativePointerEvent::ButtonReleased(pointer_button(event.button()))
            }
            NativeCapturedDragPhase::Cancelled(_) => NativePointerEvent::StreamCancelled(
                NativePointerCancelReason::ExplicitPlatformCancellation,
            ),
        };
        let input = if let Some(local) = local {
            NativePointerInput::new_handoff(
                pointer,
                edge,
                dockspace::runtime::NativeLocalToDesktopHandoff::new(local, location),
                NativePointerOwner::Native(binding),
                capture,
            )
        } else {
            NativePointerInput::new(
                pointer,
                edge,
                location,
                NativePointerOwner::Native(binding),
                capture,
            )
        };
        input.with_metadata(metadata)
    }
}

pub(crate) fn native_capture_owner(
    owner: PlatformNativePointerCaptureOwner,
    mut resolve: impl FnMut(WindowId) -> Option<NativeWindowBinding>,
) -> NativePointerOwner {
    match owner {
        PlatformNativePointerCaptureOwner::Unavailable => NativePointerOwner::Unknown,
        PlatformNativePointerCaptureOwner::None => NativePointerOwner::None,
        PlatformNativePointerCaptureOwner::Window(window) => {
            resolve(window).map_or(NativePointerOwner::Foreign, NativePointerOwner::Native)
        }
        PlatformNativePointerCaptureOwner::UnregisteredWindow => NativePointerOwner::Foreign,
    }
}

pub(super) fn native_hover(
    stack: &PlatformWindowHitStack,
    point: Option<open_gpui::Point<DevicePixels>>,
    mut resolve: impl FnMut(WindowId) -> Option<NativeWindowBinding>,
) -> NativePointerHover {
    let Some(observation) = stack.observation() else {
        return NativePointerHover::Unknown;
    };
    if point != Some(observation.sampled_point()) {
        return NativePointerHover::Unknown;
    }
    match observation.terminus() {
        PlatformWindowHitTerminus::OpenDesktop => NativePointerHover::OutsideAll,
        PlatformWindowHitTerminus::Window(PlatformWindowHit::RegisteredApplication {
            window,
            ..
        }) => resolve(window.window_id())
            .map_or(NativePointerHover::Foreign, NativePointerHover::Dock),
        PlatformWindowHitTerminus::Window(PlatformWindowHit::OpaqueBarrier { .. }) => {
            NativePointerHover::Foreign
        }
        _ => NativePointerHover::Unknown,
    }
}

pub(super) fn physical_rect(bounds: Bounds<DevicePixels>) -> Option<PhysicalRect> {
    PhysicalRect::new(
        f64::from(bounds.origin.x.0),
        f64::from(bounds.origin.y.0),
        f64::from(bounds.size.width.0),
        f64::from(bounds.size.height.0),
    )
    .ok()
}

pub(super) fn device_bounds(rect: PhysicalRect) -> Option<Bounds<DevicePixels>> {
    let values = [rect.x(), rect.y(), rect.width(), rect.height()].map(f64::round);
    if values
        .iter()
        .any(|value| *value < f64::from(i32::MIN) || *value > f64::from(i32::MAX))
    {
        return None;
    }
    Some(Bounds::new(
        open_gpui::point(
            DevicePixels(values[0] as i32),
            DevicePixels(values[1] as i32),
        ),
        open_gpui::size(
            DevicePixels(values[2] as i32),
            DevicePixels(values[3] as i32),
        ),
    ))
}

fn pointer_button(button: MouseButton) -> NativePointerButton {
    match button {
        MouseButton::Left => NativePointerButton::Primary,
        MouseButton::Right => NativePointerButton::Secondary,
        MouseButton::Middle => NativePointerButton::Middle,
        MouseButton::Navigate(open_gpui::NavigationDirection::Back) => {
            NativePointerButton::Other(1)
        }
        MouseButton::Navigate(open_gpui::NavigationDirection::Forward) => {
            NativePointerButton::Other(2)
        }
    }
}
