mod app_menu;
mod keyboard;
mod keystroke;

#[cfg(all(target_os = "linux", feature = "wayland"))]
#[expect(missing_docs)]
pub mod layer_shell;

#[cfg(any(test, feature = "test-support"))]
mod test;

#[cfg(any(test, feature = "test-support"))]
mod visual_test;

#[cfg(all(
    feature = "screen-capture",
    any(target_os = "windows", target_os = "linux", target_os = "freebsd",)
))]
pub mod scap_screen_capture;

#[cfg(all(
    any(target_os = "windows", target_os = "linux"),
    feature = "screen-capture"
))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(feature = "screen-capture"))]
pub(crate) type PlatformScreenCaptureFrame = ();
#[cfg(all(target_os = "macos", feature = "screen-capture"))]
pub(crate) type PlatformScreenCaptureFrame = PlatformPixelBuffer;
/// A retained macOS CoreVideo pixel buffer used by screen capture and surface painting.
#[cfg(target_os = "macos")]
pub type PlatformPixelBuffer = objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBuffer>;

use crate::{
    Action, AnyWindowHandle, App, AppCell, AsyncApp, AsyncWindowContext, BackgroundExecutor,
    Bounds, DEFAULT_WINDOW_SIZE, DevicePixels, DispatchEventResult, FocusId, Font, FontId,
    FontMetrics, FontRun, ForegroundExecutor, GlyphId, GpuSpecs, Hsla, ImageSource, Keymap,
    LineLayout, ModifiersChangedEvent, MouseButton, NativeInputBoundary,
    NativeInputHandlerOperation, NativeInvariantFailure, Pixels, PlatformInput, Point,
    PointerCancelReason, Priority, RenderGlyphParams, RenderImage, RenderImageParams,
    RenderSvgParams, Scene, ShapedGlyph, ShapedRun, SharedString, Size, SvgRenderer,
    SystemWindowTab, Task, Window, WindowControlArea, WindowId, WindowMutationRequest,
    geometry::ResolvedSubtreeTransform, hash, point, px, size,
};
use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use anyhow::bail;
use async_task::Runnable;
use futures::channel::oneshot;
#[cfg(any(test, feature = "test-support"))]
use image::RgbaImage;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder as _, Frame};
use open_gpui_scheduler::Instant;
pub use open_gpui_scheduler::RunnableMeta;
use parking_lot::Mutex as ParkingMutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use schemars::JsonSchema;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::ops;
use std::time::Duration;
use std::{
    fmt::{self, Debug},
    ops::Range,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::Arc,
};
use strum::EnumIter;
use uuid::Uuid;

pub use app_menu::*;
pub use keyboard::*;
pub use keystroke::*;

#[cfg(any(test, feature = "test-support"))]
pub(crate) use test::*;

#[cfg(any(test, feature = "test-support"))]
pub use test::{TestDispatcher, TestScreenCaptureSource, TestScreenCaptureStream};

#[cfg(any(test, feature = "test-support"))]
pub use visual_test::VisualTestPlatform;

/// One immutable observation of a platform window's physical client geometry.
///
/// The client bounds and scale factor belong to the same validated observation. Consumers should
/// retain this value while converting related points instead of sampling those facts separately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlatformWindowPhysicalGeometry {
    client_bounds: Bounds<DevicePixels>,
    scale_factor: f32,
}

/// One checked native top-level coverage rectangle in physical desktop coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlatformWindowPhysicalCoverage {
    bounds: Bounds<DevicePixels>,
}

/// One physical pointer observation retained only for the active native input callback.
///
/// Platform backends use this frame to prevent a logical-coordinate round trip from mixing the
/// input event's DPI with a later window DPI observation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlatformNativePointerPhysicalFrame {
    global_position: Point<DevicePixels>,
    source_geometry: PlatformWindowPhysicalGeometry,
}

impl PlatformNativePointerPhysicalFrame {
    /// Creates a callback-scoped physical pointer observation.
    #[doc(hidden)]
    pub fn new(
        global_position: Point<DevicePixels>,
        source_geometry: PlatformWindowPhysicalGeometry,
    ) -> Self {
        Self {
            global_position,
            source_geometry,
        }
    }

    /// Returns the pointer position in the physical desktop coordinate space.
    pub fn global_position(self) -> Point<DevicePixels> {
        self.global_position
    }

    /// Returns the source geometry interpreted with the input event's DPI.
    pub fn source_geometry(self) -> PlatformWindowPhysicalGeometry {
        self.source_geometry
    }
}

impl PlatformWindowPhysicalGeometry {
    /// Creates a physical geometry observation when its bounds and scale are representable.
    pub fn try_new(client_bounds: Bounds<DevicePixels>, scale_factor: f32) -> Option<Self> {
        if checked_physical_bounds_end(client_bounds).is_none()
            || !scale_factor.is_finite()
            || scale_factor <= 0.0
        {
            return None;
        }
        Some(Self {
            client_bounds,
            scale_factor,
        })
    }

    /// Returns the client bounds in physical desktop coordinates.
    pub fn client_bounds(self) -> Bounds<DevicePixels> {
        self.client_bounds
    }

    /// Returns the DPI scale sampled with the client bounds.
    pub fn scale_factor(self) -> f32 {
        self.scale_factor
    }

    /// Returns whether a physical desktop point is inside the observed client area.
    pub fn contains_global(self, point: Point<DevicePixels>) -> bool {
        checked_physical_bounds_contains(self.client_bounds, point)
    }

    /// Converts a client-local logical point into physical desktop coordinates.
    pub fn local_to_global(self, point: Point<Pixels>) -> Option<Point<DevicePixels>> {
        Some(crate::point(
            checked_logical_to_global_device_coordinate(
                self.client_bounds.origin.x,
                point.x,
                self.scale_factor,
            )?,
            checked_logical_to_global_device_coordinate(
                self.client_bounds.origin.y,
                point.y,
                self.scale_factor,
            )?,
        ))
    }

    /// Converts a physical desktop point into client-local logical coordinates.
    pub fn global_to_local(self, point: Point<DevicePixels>) -> Option<Point<Pixels>> {
        Some(crate::point(
            checked_global_device_to_logical_coordinate(
                point.x,
                self.client_bounds.origin.x,
                self.scale_factor,
            )?,
            checked_global_device_to_logical_coordinate(
                point.y,
                self.client_bounds.origin.y,
                self.scale_factor,
            )?,
        ))
    }
}

impl PlatformWindowPhysicalCoverage {
    /// Creates a checked physical coverage rectangle.
    pub fn try_new(bounds: Bounds<DevicePixels>) -> Option<Self> {
        checked_physical_bounds_end(bounds)?;
        Some(Self { bounds })
    }

    /// Returns the checked physical bounds.
    pub fn bounds(self) -> Bounds<DevicePixels> {
        self.bounds
    }

    /// Returns whether the sampled physical point is inside this coverage.
    pub fn contains(self, point: Point<DevicePixels>) -> bool {
        checked_physical_bounds_contains(self.bounds, point)
    }
}

fn checked_physical_bounds_end(
    bounds: Bounds<DevicePixels>,
) -> Option<(DevicePixels, DevicePixels)> {
    if bounds.size.width.0 < 0 || bounds.size.height.0 < 0 {
        return None;
    }
    Some((
        DevicePixels(bounds.origin.x.0.checked_add(bounds.size.width.0)?),
        DevicePixels(bounds.origin.y.0.checked_add(bounds.size.height.0)?),
    ))
}

fn checked_physical_bounds_contains(
    bounds: Bounds<DevicePixels>,
    point: Point<DevicePixels>,
) -> bool {
    let Some((right, bottom)) = checked_physical_bounds_end(bounds) else {
        return false;
    };
    point.x.0 >= bounds.origin.x.0
        && point.x.0 < right.0
        && point.y.0 >= bounds.origin.y.0
        && point.y.0 < bottom.0
}

fn checked_logical_to_global_device_coordinate(
    origin: DevicePixels,
    value: Pixels,
    scale_factor: f32,
) -> Option<DevicePixels> {
    let value = f64::from(value.as_f32());
    let scale_factor = f64::from(scale_factor);
    let physical = f64::from(origin.0) + value * scale_factor;
    if !physical.is_finite() || physical < f64::from(i32::MIN) || physical > f64::from(i32::MAX) {
        return None;
    }
    Some(DevicePixels(physical.round() as i32))
}

fn checked_global_device_to_logical_coordinate(
    value: DevicePixels,
    origin: DevicePixels,
    scale_factor: f32,
) -> Option<Pixels> {
    let logical = (f64::from(value.0) - f64::from(origin.0)) / f64::from(scale_factor);
    if !logical.is_finite() || logical < -(f32::MAX as f64) || logical > f32::MAX as f64 {
        return None;
    }
    Some(px(logical as f32))
}

/// One classified native top-level window covering a sampled physical desktop point.
///
/// Native handles remain backend-private. Registered application windows retain their complete
/// GPUI handle, while every other covering top-level is an opaque routing barrier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlatformWindowHit {
    /// A currently registered application window and its same-observation native geometry.
    RegisteredApplication {
        /// The complete GPUI handle matched to the exact native window generation.
        window: AnyWindowHandle,
        /// The native top-level bounds that cover the sampled physical point.
        coverage: PlatformWindowPhysicalCoverage,
        /// The target client geometry sampled as one immutable observation.
        geometry: PlatformWindowPhysicalGeometry,
    },
    /// A visible ordinary, foreign-process, unregistered, or otherwise unknown top-level window.
    OpaqueBarrier {
        /// The native top-level bounds that cover the sampled physical point.
        coverage: PlatformWindowPhysicalCoverage,
    },
}

impl PlatformWindowHit {
    /// Returns the checked top-level coverage for this entry.
    pub fn coverage(self) -> PlatformWindowPhysicalCoverage {
        match self {
            Self::RegisteredApplication { coverage, .. } | Self::OpaqueBarrier { coverage } => {
                coverage
            }
        }
    }
}

/// One immutable hit observation bound to exactly one sampled physical point.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformWindowHitObservation {
    sampled_point: Point<DevicePixels>,
    hits: Vec<PlatformWindowHit>,
}

impl PlatformWindowHitObservation {
    /// Creates an observation when every entry covers the sampled point.
    pub fn try_new(
        sampled_point: Point<DevicePixels>,
        hits: Vec<PlatformWindowHit>,
    ) -> Option<Self> {
        if hits
            .iter()
            .any(|hit| !hit.coverage().contains(sampled_point))
        {
            return None;
        }
        Some(Self {
            sampled_point,
            hits,
        })
    }

    /// Returns the physical desktop point this observation classifies.
    pub fn sampled_point(&self) -> Point<DevicePixels> {
        self.sampled_point
    }

    /// Returns the classified entries in front-to-back order through the first terminal.
    pub fn hits(&self) -> &[PlatformWindowHit] {
        &self.hits
    }
}

/// Availability and contents of a point-scoped native window hit stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum PlatformWindowHitStack {
    /// The backend cannot provide a complete classified stack for the sampled point.
    #[default]
    Unavailable,
    /// A complete front-to-back observation bound to its sampled physical desktop point.
    Available(PlatformWindowHitObservation),
}

impl PlatformWindowHitStack {
    /// Creates an available point-bound observation or fails closed when it is malformed.
    pub fn try_available(
        sampled_point: Point<DevicePixels>,
        hits: Vec<PlatformWindowHit>,
    ) -> Option<Self> {
        Some(Self::Available(PlatformWindowHitObservation::try_new(
            sampled_point,
            hits,
        )?))
    }

    /// Returns the available observation, if the backend produced one.
    pub fn observation(&self) -> Option<&PlatformWindowHitObservation> {
        match self {
            Self::Available(observation) => Some(observation),
            Self::Unavailable => None,
        }
    }
}

/// Platform support relevant to ImGui-style multi-viewport docking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlatformViewportCapabilities {
    /// Independent application viewport windows can be opened for docking tear-off.
    pub platform_viewport_windows: bool,
    /// Window bounds are reported in a shared desktop coordinate space.
    pub global_window_bounds: bool,
    /// The platform can report application windows in front-to-back order.
    pub window_stack: bool,
    /// The platform can classify every native top-level covering a sampled physical point.
    pub window_hit_stack: bool,
    /// Display visible bounds exclude system-reserved work areas.
    pub display_work_area: bool,
    /// Per-window DPI scale facts are reliable for placement decisions.
    pub dpi_scale: bool,
    /// Hovered-window queries pass through native no-input/click-through application windows.
    pub hovered_window_ignores_no_input: bool,
}

/// The level of support a backend provides for one window mutation property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowMutationSupport {
    /// The backend cannot apply this property.
    #[default]
    Unsupported,
    /// The backend can only apply this property while opening a window.
    CreationOnly,
    /// The backend can request and observe this property for an open window.
    Live,
}

impl WindowMutationSupport {
    /// Returns whether this property can be selected while opening a window.
    pub const fn is_available_at_creation(self) -> bool {
        !matches!(self, Self::Unsupported)
    }

    /// Returns whether this property can be requested for an already-open window.
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }
}

/// The coordinate system used by window geometry facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowCoordinateSpace {
    /// Bounds use a backend-local geometry frame because no shared desktop coordinate system is
    /// available. Origins in this frame must not be compared across application windows.
    #[default]
    WindowLocal,
    /// Bounds are expressed in a shared desktop coordinate system.
    GlobalScreen,
}

impl WindowCoordinateSpace {
    /// Returns whether bounds can be compared across application windows.
    pub const fn is_global(self) -> bool {
        matches!(self, Self::GlobalScreen)
    }
}

/// A requested primary window placement state.
///
/// A minimized window can retain maximized or fullscreen restore facts. Those restore facts remain
/// explicit on [`WindowPlacementRequest`] rather than being folded into this state enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPlacementState {
    /// A normal, restored window.
    Windowed,
    /// A maximized window.
    Maximized,
    /// A fullscreen window.
    Fullscreen,
    /// A minimized window.
    Minimized,
}

/// A structured request to mutate one coherent placement domain.
///
/// Position, size, primary state, and restore bounds remain one conflict domain even when only a
/// subset is present. Callers must not submit contradictory geometry for a non-windowed state;
/// [`crate::Window`] rejects such requests before sending a partial native mutation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowPlacementRequest {
    /// A requested window origin in [`WindowCoordinateSpace::GlobalScreen`] when position is live.
    pub position: Option<Point<Pixels>>,
    /// A requested content size.
    pub size: Option<Size<Pixels>>,
    /// A requested primary placement state.
    pub state: Option<WindowPlacementState>,
    /// A requested restore bounds value for a minimized, maximized, or fullscreen window.
    pub restore_bounds: Option<Bounds<Pixels>>,
}

impl WindowPlacementRequest {
    /// Creates an empty placement request.
    pub const fn new() -> Self {
        Self {
            position: None,
            size: None,
            state: None,
            restore_bounds: None,
        }
    }

    /// Creates a complete request for a windowed placement.
    pub const fn windowed(bounds: Bounds<Pixels>) -> Self {
        Self {
            position: Some(bounds.origin),
            size: Some(bounds.size),
            state: Some(WindowPlacementState::Windowed),
            restore_bounds: None,
        }
    }

    /// Creates a complete request for a maximized placement and its restore bounds.
    pub const fn maximized(restore_bounds: Bounds<Pixels>) -> Self {
        Self {
            position: None,
            size: None,
            state: Some(WindowPlacementState::Maximized),
            restore_bounds: Some(restore_bounds),
        }
    }

    /// Creates a complete request for a fullscreen placement and its restore bounds.
    pub const fn fullscreen(restore_bounds: Bounds<Pixels>) -> Self {
        Self {
            position: None,
            size: None,
            state: Some(WindowPlacementState::Fullscreen),
            restore_bounds: Some(restore_bounds),
        }
    }

    /// Creates a request to minimize the window.
    pub const fn minimized() -> Self {
        Self {
            position: None,
            size: None,
            state: Some(WindowPlacementState::Minimized),
            restore_bounds: None,
        }
    }

    /// Converts a legacy [`WindowBounds`] value into a complete structured placement request.
    pub const fn from_window_bounds(window_bounds: WindowBounds) -> Self {
        match window_bounds {
            WindowBounds::Windowed(bounds) => Self::windowed(bounds),
            WindowBounds::Maximized(bounds) => Self::maximized(bounds),
            WindowBounds::Fullscreen(bounds) => Self::fullscreen(bounds),
        }
    }

    /// Returns whether this request contains at least one placement property.
    pub const fn is_empty(self) -> bool {
        self.position.is_none()
            && self.size.is_none()
            && self.state.is_none()
            && self.restore_bounds.is_none()
    }

    /// Projects a complete representable request to the legacy [`WindowBounds`] API.
    ///
    /// Minimized and partial requests deliberately have no legacy projection.
    pub fn as_window_bounds(self) -> Option<WindowBounds> {
        match (self.position, self.size, self.state, self.restore_bounds) {
            (Some(origin), Some(size), Some(WindowPlacementState::Windowed), None) => {
                Some(WindowBounds::Windowed(Bounds::new(origin, size)))
            }
            (None, None, Some(WindowPlacementState::Maximized), Some(restore_bounds)) => {
                Some(WindowBounds::Maximized(restore_bounds))
            }
            (None, None, Some(WindowPlacementState::Fullscreen), Some(restore_bounds)) => {
                Some(WindowBounds::Fullscreen(restore_bounds))
            }
            _ => None,
        }
    }
}

/// Lifetime activation policy for a top-level window.
///
/// Programmatic activation and click-triggered focus are independent facts, but they mutate as one
/// coherent policy so a backend can never publish a partially applied pair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowActivationPolicy {
    /// Whether GPUI may activate the window programmatically during its lifetime.
    pub accepts_activation: bool,
    /// Whether clicking the window may activate it.
    pub focus_on_click: bool,
}

impl Default for WindowActivationPolicy {
    fn default() -> Self {
        Self {
            accepts_activation: true,
            focus_on_click: true,
        }
    }
}

/// A conflict domain for mutations of an already-open window.
///
/// Placement deliberately groups position, size, state, and restore bounds because native state
/// transitions can change all of those facts together. Lifetime activation is also coherent:
/// `accepts_activation` and `focus_on_click` share one generation and terminal observation.
/// Pointer input, alpha, topmost, and taskbar visibility remain independent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WindowMutationDomain {
    /// Window position, size, state, and restore bounds.
    Placement,
    /// Whether the window accepts pointer input.
    PointerInput,
    /// The coherent lifetime activation policy.
    ActivationPolicy,
    /// The native window background or alpha treatment.
    Alpha,
    /// Whether the window stays above ordinary application windows.
    Topmost,
    /// Whether the window appears in the taskbar or application switcher.
    TaskbarVisibility,
}

impl WindowMutationDomain {
    /// Every independently generated mutation domain.
    pub const ALL: [Self; 6] = [
        Self::Placement,
        Self::PointerInput,
        Self::ActivationPolicy,
        Self::Alpha,
        Self::Topmost,
        Self::TaskbarVisibility,
    ];
}

/// Whether a backend can honor a property during top-level window creation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowCreationSupport {
    /// The backend cannot represent the property.
    #[default]
    Unsupported,
    /// The backend can apply and report the property during creation.
    Supported,
}

impl WindowCreationSupport {
    /// Returns whether the creation property is supported.
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// Ordering constraint for the first submitted frame and native visibility.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowInitialPresentationOrder {
    /// The backend can submit the first frame while the native window remains hidden.
    BeforeVisibility,
    /// The backend must make the native surface visible or mapped before submitting a frame.
    #[default]
    AfterVisibility,
    /// Submitting the first native frame establishes visibility or mapping.
    ///
    /// This is used by protocols such as Wayland where a toplevel cannot become mapped before its
    /// first buffer commit, while that buffer also cannot be submitted before initial configure.
    PresentationEstablishesVisibility,
}

/// Creation-only capabilities for a top-level platform window.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformWindowCreationCapabilities {
    /// Support for a non-activating first appearance.
    pub focus_on_appearing: WindowCreationSupport,
    /// Support for a typed top-level transient owner relationship.
    pub transient_for: WindowCreationSupport,
    /// Support for a hidden first presentation followed by an exact-generation reveal in place.
    pub provisional_presentation: WindowCreationSupport,
    /// Required ordering of the first submitted frame and native visibility.
    pub initial_presentation_order: WindowInitialPresentationOrder,
}

/// Capability-specific support for applying platform window properties.
///
/// Support may be limited to window creation or extend to an already-open window. Placement is
/// deliberately split into its observable properties. A caller must not infer that position is
/// mutable merely because a backend can resize a window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlatformWindowMutationCapabilities {
    /// Support for changing a window's desktop position.
    pub position: WindowMutationSupport,
    /// Support for changing a window's content size.
    pub size: WindowMutationSupport,
    /// Support for restoring a window to its windowed state.
    pub windowed: WindowMutationSupport,
    /// Support for entering or observing maximized state.
    pub maximized: WindowMutationSupport,
    /// Support for entering or observing fullscreen state.
    pub fullscreen: WindowMutationSupport,
    /// Support for entering or observing minimized state.
    pub minimized: WindowMutationSupport,
    /// Support for changing or observing windowed restore bounds.
    pub restore_bounds: WindowMutationSupport,
    /// Support for changing whether a window accepts pointer input.
    pub pointer_input: WindowMutationSupport,
    /// Support for coherently changing lifetime activation and click-focus policy.
    pub activation_policy: WindowMutationSupport,
    /// Support for window alpha or a transparent/blurred background.
    pub alpha: WindowMutationSupport,
    /// Support for keeping a window above ordinary application windows.
    pub topmost: WindowMutationSupport,
    /// Support for controlling whether a window appears in the taskbar or application switcher.
    pub taskbar_visibility: WindowMutationSupport,
    /// The coordinate system used for reported window geometry.
    pub coordinate_space: WindowCoordinateSpace,
}

/// Complete capabilities for a top-level platform window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlatformWindowCapabilities {
    /// Creation-only capabilities.
    pub creation: PlatformWindowCreationCapabilities,
    /// Live and creation-only mutation capabilities.
    pub mutations: PlatformWindowMutationCapabilities,
}

/// Capabilities captured for the actual kind of an opened platform window.
///
/// The profile is fixed when GPUI creates the window. It therefore remains an honest description
/// of that window even if a backend supports different properties for another [`WindowKind`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformWindowProfile {
    /// The platform window kind used at creation.
    pub kind: WindowKind,
    /// Creation and mutation support for [`Self::kind`].
    pub capabilities: PlatformWindowCapabilities,
}

/// Immutable facts established by the backend for one committed window creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowCreationFacts {
    /// Whether the native window was requested to become visible after its first presentation gate.
    pub show: bool,
    /// Whether the backend accepted an activating first-appearance policy.
    ///
    /// This records the applied show policy, not whether the operating system ultimately granted
    /// foreground ownership.
    pub focus_on_appearing: bool,
    /// The applied top-level transient owner, if the backend supports and established one.
    pub transient_for: Option<AnyWindowHandle>,
}

/// Observable stages of a window's first and latest presentation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowPresentationFacts {
    /// The native top-level window has been created.
    pub native_created: bool,
    /// Latest frame generation accepted by GPUI's frame journal.
    pub frame_accepted_generation: Option<u64>,
    /// Latest frame generation submitted to a platform renderer.
    pub present_submitted_generation: Option<u64>,
    /// Latest submitted generation containing at least one valid paint primitive.
    pub non_empty_presented_generation: Option<u64>,
    /// Latest attempt to submit an accepted frame to the platform renderer.
    pub latest_present_attempt: Option<WindowPresentAttemptFacts>,
    /// Terminal state of the bounded native initial-presentation command.
    pub initial_presentation: WindowInitialPresentationStatus,
    /// Whether the backend currently reports the native window as visible.
    pub native_visible: bool,
}

/// Facts for one renderer submission attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowPresentAttemptFacts {
    /// Accepted framework frame generation handed to the renderer.
    pub generation: u64,
    /// Renderer or native-surface outcome for this exact attempt.
    pub outcome: PlatformWindowPresentOutcome,
    /// Whether the submitted scene contained at least one valid paint primitive.
    pub contained_valid_primitives: bool,
}

/// Terminal state of the bounded native initial-presentation command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowInitialPresentationStatus {
    /// The command has not yet settled.
    #[default]
    Pending,
    /// The backend accepted the command and GPUI observed its completion event.
    Completed,
    /// The backend rejected both bounded command attempts.
    Rejected,
}

/// The exact lifecycle phase of a private provisional top-level window session.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowProvisionalSessionPhase {
    /// The session exists before a committed full window id is available.
    Unbound,
    /// The exact window is bound but remains non-interactive.
    Gated,
    /// The exact window was promoted in place and may accept interaction.
    Promoted,
    /// Presentation and interaction are terminal for this generation.
    Terminal,
}

/// An immutable synchronous observation of one provisional-window generation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowProvisionalSessionSnapshot {
    generation: u64,
    window_id: Option<WindowId>,
    phase: WindowProvisionalSessionPhase,
}

impl WindowProvisionalSessionSnapshot {
    /// Returns the immutable provisional-session generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns the committed full window id once binding succeeds.
    pub const fn window_id(self) -> Option<WindowId> {
        self.window_id
    }

    /// Returns the exact lifecycle phase.
    pub const fn phase(self) -> WindowProvisionalSessionPhase {
        self.phase
    }

    /// Returns whether native and framework interaction are admitted.
    pub const fn accepts_interaction(self) -> bool {
        matches!(self.phase, WindowProvisionalSessionPhase::Promoted)
    }

    /// Returns whether the window must remain natively hit-transparent.
    pub const fn requires_native_hit_transparency(self) -> bool {
        !matches!(self.phase, WindowProvisionalSessionPhase::Promoted)
    }
}

/// A failed exact-generation provisional-window state transition.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WindowProvisionalSessionError {
    /// The session generation must be non-zero.
    #[error("provisional window-session generation must be non-zero")]
    ZeroGeneration,
    /// A different full window id already owns the session.
    #[error("provisional window session is bound to a different window")]
    WindowMismatch,
    /// The requested transition is not valid for the current phase.
    #[error("provisional window-session transition is not valid for the current phase")]
    InvalidPhase,
    /// Another pending window opening already owns this session.
    #[error("provisional window session is already claimed by another opening")]
    AlreadyClaimed,
}

#[derive(Debug)]
struct WindowProvisionalSessionState {
    window_id: Option<WindowId>,
    phase: WindowProvisionalSessionPhase,
    opening_claimed: bool,
    reveal_ticket: Option<WindowProvisionalRevealTicket>,
}

pub(crate) struct WindowProvisionalOpeningClaim {
    session: WindowProvisionalSession,
}

impl Drop for WindowProvisionalOpeningClaim {
    fn drop(&mut self) {
        let mut state = self.session.state.lock();
        if state.phase == WindowProvisionalSessionPhase::Unbound && state.window_id.is_none() {
            state.opening_claimed = false;
        }
    }
}

/// A generation-bound interaction and presentation gate for one provisional top-level window.
///
/// This is intentionally a hidden cross-crate capability used by framework-owned window
/// authorities. It is not a general-purpose visibility or activation API.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct WindowProvisionalSession {
    generation: u64,
    state: Arc<ParkingMutex<WindowProvisionalSessionState>>,
}

/// Terminal status of one exact provisional reveal request.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowProvisionalRevealOutcome {
    /// The request has not reached a terminal native boundary yet.
    Pending,
    /// The exact HWND became visible without activation.
    Revealed,
    /// The owning backend rejected the exact reveal request.
    Rejected,
    /// The backend accepted the command without publishing the required native observation.
    NativeObservationMissing,
    /// The target full window generation was no longer current.
    Stale,
    /// The application or native window became terminal before reveal.
    WindowTerminal,
}

/// Relative native Z-order result observed for one provisional reveal.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowProvisionalRevealZOrder {
    /// The requested placement was retained exactly.
    Exact,
    /// The platform adjusted the requested placement while retaining a usable visible window.
    Adjusted,
    /// The backend could not observe a meaningful relative placement.
    Unavailable,
}

/// Native facts observed synchronously for one exact provisional reveal command.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowProvisionalRevealNativeFacts {
    native_visible: bool,
    foreground_unchanged: bool,
    native_hit_transparent: bool,
    stable_native_window_identity: bool,
    z_order: WindowProvisionalRevealZOrder,
}

impl WindowProvisionalRevealNativeFacts {
    /// Creates one backend observation for the exact reveal command being dispatched.
    pub const fn new(
        native_visible: bool,
        foreground_unchanged: bool,
        native_hit_transparent: bool,
        stable_native_window_identity: bool,
        z_order: WindowProvisionalRevealZOrder,
    ) -> Self {
        Self {
            native_visible,
            foreground_unchanged,
            native_hit_transparent,
            stable_native_window_identity,
            z_order,
        }
    }

    /// Returns whether the exact native window was visible after the command.
    pub const fn native_visible(self) -> bool {
        self.native_visible
    }

    /// Returns whether the command preserved the foreground window.
    pub const fn foreground_unchanged(self) -> bool {
        self.foreground_unchanged
    }

    /// Returns whether native hit testing observed the provisional as transparent.
    pub const fn native_hit_transparent(self) -> bool {
        self.native_hit_transparent
    }

    /// Returns whether the command retained the original native-window identity.
    pub const fn stable_native_window_identity(self) -> bool {
        self.stable_native_window_identity
    }

    /// Returns the relative native Z-order observation.
    pub const fn z_order(self) -> WindowProvisionalRevealZOrder {
        self.z_order
    }

    /// Returns whether the mandatory no-activation reveal facts were all satisfied.
    pub const fn accepts_reveal(self) -> bool {
        self.native_visible
            && self.foreground_unchanged
            && self.native_hit_transparent
            && self.stable_native_window_identity
    }
}

/// Immutable facts for one exact provisional reveal request.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowProvisionalRevealSnapshot {
    window_id: WindowId,
    session_generation: u64,
    minimum_presentation_generation: u64,
    presentation_generation: Option<u64>,
    native_facts: Option<WindowProvisionalRevealNativeFacts>,
    outcome: WindowProvisionalRevealOutcome,
}

impl WindowProvisionalRevealSnapshot {
    /// Returns the exact committed full window id.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the owning provisional-session generation.
    pub const fn session_generation(self) -> u64 {
        self.session_generation
    }

    /// Returns the first renderer generation eligible to reveal the window.
    pub const fn minimum_presentation_generation(self) -> u64 {
        self.minimum_presentation_generation
    }

    /// Returns the exact renderer generation bound to the native reveal.
    pub const fn presentation_generation(self) -> Option<u64> {
        self.presentation_generation
    }

    /// Returns the native observation published by the backend.
    pub const fn native_facts(self) -> Option<WindowProvisionalRevealNativeFacts> {
        self.native_facts
    }

    /// Returns the terminal or pending reveal outcome.
    pub const fn outcome(self) -> WindowProvisionalRevealOutcome {
        self.outcome
    }
}

#[derive(Debug)]
struct WindowProvisionalRevealState {
    presentation_generation: Option<u64>,
    native_facts: Option<WindowProvisionalRevealNativeFacts>,
    outcome: WindowProvisionalRevealOutcome,
}

/// A cloneable exact-generation receipt that survives the target [`Window`].
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct WindowProvisionalRevealTicket {
    window_id: WindowId,
    session_generation: u64,
    minimum_presentation_generation: u64,
    state: Arc<ParkingMutex<WindowProvisionalRevealState>>,
}

impl WindowProvisionalRevealTicket {
    pub(crate) fn new(
        window_id: WindowId,
        session_generation: u64,
        minimum_presentation_generation: u64,
    ) -> Self {
        Self {
            window_id,
            session_generation,
            minimum_presentation_generation,
            state: Arc::new(ParkingMutex::new(WindowProvisionalRevealState {
                presentation_generation: None,
                native_facts: None,
                outcome: WindowProvisionalRevealOutcome::Pending,
            })),
        }
    }

    /// Returns an immutable snapshot that survives the target window.
    pub fn snapshot(&self) -> WindowProvisionalRevealSnapshot {
        let state = self.state.lock();
        WindowProvisionalRevealSnapshot {
            window_id: self.window_id,
            session_generation: self.session_generation,
            minimum_presentation_generation: self.minimum_presentation_generation,
            presentation_generation: state.presentation_generation,
            native_facts: state.native_facts,
            outcome: state.outcome,
        }
    }

    pub(crate) fn bind_presentation(&self, generation: u64) -> bool {
        let mut state = self.state.lock();
        if state.outcome != WindowProvisionalRevealOutcome::Pending
            || state.presentation_generation.is_some()
            || generation < self.minimum_presentation_generation
        {
            return false;
        }
        state.presentation_generation = Some(generation);
        true
    }

    fn record_native_facts(
        &self,
        generation: u64,
        facts: WindowProvisionalRevealNativeFacts,
    ) -> bool {
        let mut state = self.state.lock();
        if state.outcome != WindowProvisionalRevealOutcome::Pending
            || state.presentation_generation != Some(generation)
            || state.native_facts.is_some()
        {
            return false;
        }
        state.native_facts = Some(facts);
        true
    }

    pub(crate) fn settle(&self, outcome: WindowProvisionalRevealOutcome) -> bool {
        debug_assert_ne!(outcome, WindowProvisionalRevealOutcome::Pending);
        let mut state = self.state.lock();
        if state.outcome != WindowProvisionalRevealOutcome::Pending {
            return false;
        }
        state.outcome = if outcome == WindowProvisionalRevealOutcome::Revealed
            && state.native_facts.is_none()
        {
            WindowProvisionalRevealOutcome::NativeObservationMissing
        } else {
            outcome
        };
        true
    }
}

impl WindowProvisionalSession {
    /// Creates one unbound, non-interactive provisional generation.
    pub fn new(generation: u64) -> Result<Self, WindowProvisionalSessionError> {
        if generation == 0 {
            return Err(WindowProvisionalSessionError::ZeroGeneration);
        }
        Ok(Self {
            generation,
            state: Arc::new(ParkingMutex::new(WindowProvisionalSessionState {
                window_id: None,
                phase: WindowProvisionalSessionPhase::Unbound,
                opening_claimed: false,
                reveal_ticket: None,
            })),
        })
    }

    pub(crate) fn claim_opening(
        &self,
    ) -> Result<WindowProvisionalOpeningClaim, WindowProvisionalSessionError> {
        let mut state = self.state.lock();
        if state.phase != WindowProvisionalSessionPhase::Unbound || state.window_id.is_some() {
            return Err(WindowProvisionalSessionError::InvalidPhase);
        }
        if state.opening_claimed {
            return Err(WindowProvisionalSessionError::AlreadyClaimed);
        }
        state.opening_claimed = true;
        drop(state);
        Ok(WindowProvisionalOpeningClaim {
            session: self.clone(),
        })
    }

    /// Returns one synchronous immutable snapshot without borrowing [`App`].
    pub fn snapshot(&self) -> WindowProvisionalSessionSnapshot {
        let state = self.state.lock();
        WindowProvisionalSessionSnapshot {
            generation: self.generation,
            window_id: state.window_id,
            phase: state.phase,
        }
    }

    pub(crate) fn register_reveal_ticket(
        &self,
        ticket: WindowProvisionalRevealTicket,
    ) -> Result<(), WindowProvisionalSessionError> {
        let mut state = self.state.lock();
        if state.window_id != Some(ticket.window_id)
            || state.phase != WindowProvisionalSessionPhase::Gated
            || self.generation != ticket.session_generation
            || state.reveal_ticket.is_some()
        {
            return Err(WindowProvisionalSessionError::InvalidPhase);
        }
        state.reveal_ticket = Some(ticket);
        Ok(())
    }

    /// Records the native facts for the exact generation currently armed by GPUI.
    #[doc(hidden)]
    pub fn record_native_reveal(
        &self,
        window_id: WindowId,
        presentation_generation: u64,
        facts: WindowProvisionalRevealNativeFacts,
    ) -> Result<(), WindowProvisionalSessionError> {
        let ticket = {
            let state = self.state.lock();
            if state.window_id != Some(window_id) {
                return Err(WindowProvisionalSessionError::WindowMismatch);
            }
            if state.phase != WindowProvisionalSessionPhase::Gated {
                return Err(WindowProvisionalSessionError::InvalidPhase);
            }
            state
                .reveal_ticket
                .clone()
                .ok_or(WindowProvisionalSessionError::InvalidPhase)?
        };
        if ticket.record_native_facts(presentation_generation, facts) {
            Ok(())
        } else {
            Err(WindowProvisionalSessionError::InvalidPhase)
        }
    }

    /// Binds the exact committed full window id while preserving the interaction gate.
    pub(crate) fn bind(&self, window_id: WindowId) -> Result<(), WindowProvisionalSessionError> {
        let mut state = self.state.lock();
        match (state.window_id, state.phase) {
            (None, WindowProvisionalSessionPhase::Unbound) if state.opening_claimed => {
                state.window_id = Some(window_id);
                state.phase = WindowProvisionalSessionPhase::Gated;
                state.opening_claimed = false;
                Ok(())
            }
            (Some(current), WindowProvisionalSessionPhase::Gated) if current == window_id => Ok(()),
            (Some(current), _) if current != window_id => {
                Err(WindowProvisionalSessionError::WindowMismatch)
            }
            _ => Err(WindowProvisionalSessionError::InvalidPhase),
        }
    }

    /// Promotes the exact bound window in place and admits interaction.
    pub(crate) fn promote(&self, window_id: WindowId) -> Result<(), WindowProvisionalSessionError> {
        let mut state = self.state.lock();
        match (state.window_id, state.phase) {
            (Some(current), WindowProvisionalSessionPhase::Gated) if current == window_id => {
                state.phase = WindowProvisionalSessionPhase::Promoted;
                Ok(())
            }
            (Some(current), WindowProvisionalSessionPhase::Promoted) if current == window_id => {
                Ok(())
            }
            (Some(current), _) if current != window_id => {
                Err(WindowProvisionalSessionError::WindowMismatch)
            }
            _ => Err(WindowProvisionalSessionError::InvalidPhase),
        }
    }

    /// Makes presentation and interaction terminal for the exact bound window.
    pub(crate) fn terminate(
        &self,
        window_id: WindowId,
    ) -> Result<(), WindowProvisionalSessionError> {
        let mut state = self.state.lock();
        match state.window_id {
            Some(current) if current != window_id => {
                Err(WindowProvisionalSessionError::WindowMismatch)
            }
            Some(_) => {
                state.phase = WindowProvisionalSessionPhase::Terminal;
                Ok(())
            }
            None => Err(WindowProvisionalSessionError::InvalidPhase),
        }
    }

    pub(crate) fn same_authority(&self, other: &Self) -> bool {
        self.generation == other.generation && Arc::ptr_eq(&self.state, &other.state)
    }
}

/// A coherent snapshot of facts observed from a platform window.
///
/// This value describes committed platform state. It never represents an unobserved mutation
/// request that was merely queued for a backend.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowPlatformFacts {
    /// The current bounds reported by the platform.
    pub bounds: Bounds<Pixels>,
    /// The coordinate system used by [`Self::bounds`].
    pub coordinate_space: WindowCoordinateSpace,
    /// The current window state and its restore bounds.
    pub window_bounds: WindowBounds,
    /// The current window state and bounds excluding platform insets when available.
    pub inner_window_bounds: WindowBounds,
    /// The drawable content size.
    pub content_size: Size<Pixels>,
    /// The current platform scale factor.
    pub scale_factor: f32,
    /// The display currently associated with the window, when known.
    pub display_id: Option<DisplayId>,
    /// Whether the window is minimized.
    pub is_minimized: bool,
    /// Whether the window is maximized.
    pub is_maximized: bool,
    /// Whether the window is fullscreen.
    pub is_fullscreen: bool,
    /// Whether the window currently accepts pointer input.
    pub accepts_pointer_input: bool,
    /// Whether the window accepts programmatic activation.
    pub accepts_activation: bool,
    /// Whether the window is configured to take focus when clicked.
    pub focus_on_click: bool,
    /// The committed native window background treatment.
    pub background_appearance: WindowBackgroundAppearance,
    /// Whether the window is configured to stay above ordinary application windows.
    pub topmost: bool,
    /// Whether the window is configured to appear in the taskbar or application switcher.
    pub taskbar_visible: bool,
    /// Whether the window is active.
    pub is_active: bool,
}

/// A coherent terminal observation emitted by a platform window for one mutation domain.
///
/// The facts snapshot is immutable event data captured by the backend at the observation point.
/// Consumers must commit this value directly rather than re-reading potentially newer getters.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformWindowMutationObservation {
    /// The request conflict domain this observation can settle.
    pub domain: WindowMutationDomain,
    /// The generation supplied when the backend accepted this request.
    pub generation: u64,
    /// The terminal result reported by the backend.
    ///
    /// [`PlatformWindowMutationTerminal::Observed`] leaves exact-versus-adjusted classification to
    /// GPUI, using the request and this facts snapshot. Every other value is an explicit backend
    /// terminal failure and must not be reclassified as a window-manager adjustment.
    pub terminal: PlatformWindowMutationTerminal,
    /// The coherent observed platform facts.
    pub facts: WindowPlatformFacts,
}

impl PlatformWindowMutationObservation {
    /// Creates a successful terminal observation.
    pub fn observed(
        domain: WindowMutationDomain,
        generation: u64,
        facts: WindowPlatformFacts,
    ) -> Self {
        Self {
            domain,
            generation,
            terminal: PlatformWindowMutationTerminal::Observed,
            facts,
        }
    }

    /// Creates an explicit backend terminal observation.
    pub fn terminal(
        domain: WindowMutationDomain,
        generation: u64,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) -> Self {
        Self {
            domain,
            generation,
            terminal,
            facts,
        }
    }
}

/// The terminal result reported by a backend after accepting an asynchronous mutation request.
///
/// `Observed` means the supplied facts represent a completed platform operation; GPUI then
/// distinguishes an exact outcome from an OS-adjusted one. The other variants preserve a backend
/// failure or close result exactly as reported.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum PlatformWindowMutationTerminal {
    /// The backend observed completed platform facts.
    #[default]
    Observed,
    /// The backend rejected the request after accepting it asynchronously.
    Rejected,
    /// The backend determined that the open-window mutation is unsupported.
    Unsupported,
    /// The native window closed before the operation could complete.
    WindowClosed,
}

/// The synchronous result of handing a platform window mutation to a backend.
///
/// `Queued` means only that the backend accepted the request path. It does not mean that the
/// operating system applied the request; callers must wait for an observed platform fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformWindowDispatch {
    /// The backend accepted the request for asynchronous observation.
    Queued,
    /// The currently observed value already matches the requested value.
    Unchanged,
    /// The backend does not support this mutation for an open window.
    Unsupported,
    /// The backend rejected the request before it could be observed.
    Rejected,
    /// The window was closed before the request could be dispatched.
    WindowClosed,
}

/// Backend hovered-window signal for multi-viewport routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlatformHoveredWindow {
    /// The backend cannot reliably report the window under the pointer for this snapshot.
    #[default]
    Unavailable,
    /// The backend reliably reported that no application window is under the pointer.
    NoWindow,
    /// The backend reliably reported the application window under the pointer.
    Window(AnyWindowHandle),
}

impl PlatformHoveredWindow {
    /// Converts a reliable optional window into a hovered-window signal.
    pub fn from_window(window: Option<AnyWindowHandle>) -> Self {
        window.map_or(Self::NoWindow, Self::Window)
    }

    /// Returns the hovered application window when the backend reported one.
    pub fn window(self) -> Option<AnyWindowHandle> {
        match self {
            Self::Window(window) => Some(window),
            Self::Unavailable | Self::NoWindow => None,
        }
    }

    /// Returns true when the backend can distinguish no hovered application window from unknown.
    pub fn is_available(self) -> bool {
        !matches!(self, Self::Unavailable)
    }
}

/// Backend focused-window signal for multi-viewport focus reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlatformFocusedWindow {
    /// The backend cannot reliably report the focused application window for this snapshot.
    #[default]
    Unavailable,
    /// The backend reliably reported that no application window is focused.
    NoWindow,
    /// The backend reliably reported the focused application window.
    Window(AnyWindowHandle),
}

impl PlatformFocusedWindow {
    /// Converts a reliable optional window into a focused-window signal.
    pub fn from_window(window: Option<AnyWindowHandle>) -> Self {
        window.map_or(Self::NoWindow, Self::Window)
    }

    /// Returns the focused application window when the backend reported one.
    pub fn window(self) -> Option<AnyWindowHandle> {
        match self {
            Self::Window(window) => Some(window),
            Self::Unavailable | Self::NoWindow => None,
        }
    }

    /// Returns true when the backend can distinguish no focused window from unknown.
    pub fn is_available(self) -> bool {
        !matches!(self, Self::Unavailable)
    }
}

// TODO(jk): return an enum instead of a string
/// Return which compositor we're guessing we'll use.
/// Does not attempt to connect to the given compositor.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[inline]
pub fn guess_compositor() -> &'static str {
    if std::env::var_os("ZED_HEADLESS").is_some() {
        return "Headless";
    }

    #[cfg(feature = "wayland")]
    let wayland_display = std::env::var_os("WAYLAND_DISPLAY");
    #[cfg(not(feature = "wayland"))]
    let wayland_display: Option<std::ffi::OsString> = None;

    #[cfg(feature = "x11")]
    let x11_display = std::env::var_os("DISPLAY");
    #[cfg(not(feature = "x11"))]
    let x11_display: Option<std::ffi::OsString> = None;

    let use_wayland = wayland_display.is_some_and(|display| !display.is_empty());
    let use_x11 = x11_display.is_some_and(|display| !display.is_empty());

    if use_wayland {
        "Wayland"
    } else if use_x11 {
        "X11"
    } else {
        "Headless"
    }
}

#[expect(missing_docs)]
pub trait Platform: 'static {
    fn background_executor(&self) -> BackgroundExecutor;
    fn foreground_executor(&self) -> ForegroundExecutor;
    fn text_system(&self) -> Arc<dyn PlatformTextSystem>;

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>);
    fn quit(&self);
    fn restart(&self, binary_path: Option<PathBuf>);
    fn activate(&self, ignoring_other_apps: bool);
    fn hide(&self);
    fn hide_other_apps(&self);
    fn unhide_other_apps(&self);

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    fn hovered_window(&self) -> PlatformHoveredWindow {
        PlatformHoveredWindow::Unavailable
    }
    fn active_window(&self) -> Option<AnyWindowHandle>;
    fn focused_window(&self) -> PlatformFocusedWindow {
        PlatformFocusedWindow::Unavailable
    }
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        None
    }
    fn window_hit_stack_at(&self, _point: Point<DevicePixels>) -> PlatformWindowHitStack {
        PlatformWindowHitStack::Unavailable
    }
    fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        PlatformViewportCapabilities::default()
    }
    fn window_capabilities(
        &self,
        _kind: &WindowKind,
        _display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        PlatformWindowCapabilities::default()
    }
    fn mouse_button_is_pressed(&self, _button: MouseButton) -> Option<bool> {
        None
    }

    fn is_screen_capture_supported(&self) -> bool {
        false
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        let (sources_tx, sources_rx) = oneshot::channel();
        sources_tx
            .send(Err(anyhow::anyhow!(
                "gpui was compiled without the screen-capture feature"
            )))
            .ok();
        sources_rx
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>>;

    /// Returns the appearance of the application's windows.
    fn window_appearance(&self) -> WindowAppearance;

    /// Returns the window button layout configuration when supported.
    fn button_layout(&self) -> Option<WindowButtonLayout> {
        None
    }

    fn open_url(&self, url: &str);
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>);
    fn register_url_scheme(&self, url: &str) -> Task<Result<()>>;

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>>;
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>>;
    fn can_select_mixed_files_and_dirs(&self) -> bool;
    fn reveal_path(&self, path: &Path);
    fn open_with_system(&self, path: &Path);

    fn on_quit(&self, callback: Box<dyn FnMut()>);
    fn on_reopen(&self, callback: Box<dyn FnMut()>);
    fn on_system_wake(&self, callback: Box<dyn FnMut()>);

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap);
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        None
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap);
    fn perform_dock_menu_action(&self, _action: usize) {}
    fn add_recent_document(&self, _path: &Path) {}
    fn update_jump_list(
        &self,
        _menus: Vec<MenuItem>,
        _entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        Task::ready(Vec::new())
    }
    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>);
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>);
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>);

    fn thermal_state(&self) -> ThermalState;
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>);

    fn compositor_name(&self) -> &'static str {
        ""
    }
    fn app_path(&self) -> Result<PathBuf>;
    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf>;

    /// Hides the mouse cursor until the user moves the mouse over one of
    /// this application's windows.
    fn hide_cursor_until_mouse_moves(&self);

    /// Returns whether the mouse cursor is currently visible.
    fn is_cursor_visible(&self) -> bool;

    fn should_auto_hide_scrollbars(&self) -> bool;

    fn read_from_clipboard(&self) -> Option<ClipboardItem>;
    fn write_to_clipboard(&self, item: ClipboardItem);

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem>;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem);

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem>;
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem);

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>>;
    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>>;
    fn delete_credentials(&self, url: &str) -> Task<Result<()>>;

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout>;
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper>;
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>);
}

/// A handle to a platform's display, e.g. a monitor or laptop screen.
pub trait PlatformDisplay: Debug {
    /// Get the ID for this display
    fn id(&self) -> DisplayId;

    /// Returns a stable identifier for this display that can be persisted and used
    /// across system restarts.
    fn uuid(&self) -> Result<Uuid>;

    /// Get the bounds for this display
    fn bounds(&self) -> Bounds<Pixels>;

    /// Get the visible bounds for this display, excluding taskbar/dock areas.
    /// This is the usable area where windows can be placed without being obscured.
    /// Defaults to the full display bounds if not overridden.
    fn visible_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }

    /// Get the default bounds for this display to place a window
    fn default_bounds(&self) -> Bounds<Pixels> {
        let bounds = self.bounds();
        let center = bounds.center();
        let clipped_window_size = DEFAULT_WINDOW_SIZE.min(&bounds.size);

        let offset = clipped_window_size / 2.0;
        let origin = point(center.x - offset.width, center.y - offset.height);
        Bounds::new(origin, clipped_window_size)
    }
}

/// Thermal state of the system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    /// System has no thermal constraints
    Nominal,
    /// System is slightly constrained, reduce discretionary work
    Fair,
    /// System is moderately constrained, reduce CPU/GPU intensive work
    Serious,
    /// System is critically constrained, minimize all resource usage
    Critical,
}

/// Metadata for a given [ScreenCaptureSource]
#[derive(Clone)]
pub struct SourceMetadata {
    /// Opaque identifier of this screen.
    pub id: u64,
    /// Human-readable label for this source.
    pub label: Option<SharedString>,
    /// Whether this source is the main display.
    pub is_main: Option<bool>,
    /// Video resolution of this source.
    pub resolution: Size<DevicePixels>,
}

/// A source of on-screen video content that can be captured.
pub trait ScreenCaptureSource {
    /// Returns metadata for this source.
    fn metadata(&self) -> Result<SourceMetadata>;

    /// Start capture video from this source, invoking the given callback
    /// with each frame.
    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>>;
}

/// A video stream captured from a screen.
pub trait ScreenCaptureStream {
    /// Returns metadata for this source.
    fn metadata(&self) -> Result<SourceMetadata>;
}

/// A frame of video captured from a screen.
pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame);

/// An opaque identifier for a hardware display
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct DisplayId(pub(crate) u64);

impl DisplayId {
    /// Create a new `DisplayId` from a raw platform display identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl From<u64> for DisplayId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<DisplayId> for u64 {
    fn from(id: DisplayId) -> Self {
        id.0
    }
}

impl Debug for DisplayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisplayId({})", self.0)
    }
}

/// Which part of the window to resize
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    /// The top edge
    Top,
    /// The top right corner
    TopRight,
    /// The right edge
    Right,
    /// The bottom right corner
    BottomRight,
    /// The bottom edge
    Bottom,
    /// The bottom left corner
    BottomLeft,
    /// The left edge
    Left,
    /// The top left corner
    TopLeft,
}

/// A platform-window operation that may synchronously pump native input or delegate callbacks.
///
/// GPUI executes this closed command set only after releasing the outer application borrow.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformWindowCommand {
    CompleteInitialPresentation {
        activate: bool,
    },
    RevealDeferredInitialPresentation {
        session_generation: u64,
        presentation_generation: u64,
    },
    Activate,
    ShowWindowMenu(Point<Pixels>),
    StartWindowMove,
    StartWindowResize(ResizeEdge),
}

/// The synchronous terminal result of a platform-window command.
///
/// A rejected initial-presentation command is retried by the native command authority and must
/// not publish an initial-presentation completion event.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformWindowCommandOutcome {
    Accepted,
    Rejected,
}

/// The synchronous result of a post-borrow native pointer-capture release.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformPointerCaptureReleaseOutcome {
    /// The backend released the native capture owned by this window.
    Released,
    /// The native window was already terminal, so no capture can remain.
    NativeWindowTerminal,
    /// The backend could not prove terminal release and GPUI must retry.
    Rejected,
}

/// The synchronous result of asking a platform window to begin native retirement.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformNativeWindowRetirementOutcome {
    /// Native retirement was accepted. GPUI may release the platform-window owner while awaiting
    /// the terminal callback.
    Accepted,
    /// The native window is already terminal.
    NativeWindowTerminal,
    /// Retirement was not accepted and must be retried while retaining the platform-window owner.
    Rejected,
}

/// The synchronous result of asking a prepared platform window to quiesce presentation.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformPresentationShutdownOutcome {
    /// All surface-bound renderer work acknowledged quiescence for the exact shutdown ticket.
    Quiesced,
    /// Quiescence could not be proven and must be retried while retaining the native owner.
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowPresentationShutdownPhase {
    Claimed,
    Quiesced,
    NativeTerminal,
    TerminalBeforeQuiesce,
}

/// Immutable facts for one exact presentation-shutdown generation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowPresentationShutdownSnapshot {
    window_id: WindowId,
    generation: u64,
    quiesced: bool,
    native_terminal: bool,
    protocol_violation: bool,
}

impl WindowPresentationShutdownSnapshot {
    /// Returns the exact full window id owned by the shutdown generation.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the opaque shutdown generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns whether surface-bound presentation work acknowledged quiescence.
    pub const fn quiesced(self) -> bool {
        self.quiesced
    }

    /// Returns whether the native window published terminal state.
    pub const fn native_terminal(self) -> bool {
        self.native_terminal
    }

    /// Returns whether native terminal was observed before renderer quiescence.
    pub const fn protocol_violation(self) -> bool {
        self.protocol_violation
    }
}

/// A cloneable exact-generation receipt that orders renderer quiescence before native terminal.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct WindowPresentationShutdownTicket {
    window_id: WindowId,
    generation: u64,
    phase: Arc<ParkingMutex<WindowPresentationShutdownPhase>>,
}

impl WindowPresentationShutdownTicket {
    /// Creates one exact presentation-shutdown generation for framework-owned backend work.
    #[doc(hidden)]
    pub fn new(window_id: WindowId, generation: u64) -> Self {
        assert_ne!(
            generation, 0,
            "presentation-shutdown generation must be non-zero"
        );
        Self {
            window_id,
            generation,
            phase: Arc::new(ParkingMutex::new(WindowPresentationShutdownPhase::Claimed)),
        }
    }

    /// Returns immutable shutdown facts without borrowing [`App`].
    pub fn snapshot(&self) -> WindowPresentationShutdownSnapshot {
        let phase = *self.phase.lock();
        WindowPresentationShutdownSnapshot {
            window_id: self.window_id,
            generation: self.generation,
            quiesced: matches!(
                phase,
                WindowPresentationShutdownPhase::Quiesced
                    | WindowPresentationShutdownPhase::NativeTerminal
            ),
            native_terminal: matches!(
                phase,
                WindowPresentationShutdownPhase::NativeTerminal
                    | WindowPresentationShutdownPhase::TerminalBeforeQuiesce
            ),
            protocol_violation: phase == WindowPresentationShutdownPhase::TerminalBeforeQuiesce,
        }
    }

    /// Returns whether both handles name the same exact shutdown authority.
    #[doc(hidden)]
    pub fn same_authority(&self, other: &Self) -> bool {
        self.window_id == other.window_id
            && self.generation == other.generation
            && Arc::ptr_eq(&self.phase, &other.phase)
    }

    /// Acknowledges release of surface-bound presentation work for this exact generation.
    #[doc(hidden)]
    pub fn acknowledge_quiesced(&self) -> bool {
        let mut phase = self.phase.lock();
        match *phase {
            WindowPresentationShutdownPhase::Claimed => {
                *phase = WindowPresentationShutdownPhase::Quiesced;
                true
            }
            WindowPresentationShutdownPhase::Quiesced
            | WindowPresentationShutdownPhase::NativeTerminal => true,
            WindowPresentationShutdownPhase::TerminalBeforeQuiesce => false,
        }
    }

    /// Acknowledges the exact native-window terminal after renderer quiescence.
    #[doc(hidden)]
    pub fn acknowledge_native_terminal(&self) -> bool {
        let mut phase = self.phase.lock();
        match *phase {
            WindowPresentationShutdownPhase::Claimed => {
                *phase = WindowPresentationShutdownPhase::TerminalBeforeQuiesce;
                false
            }
            WindowPresentationShutdownPhase::Quiesced => {
                *phase = WindowPresentationShutdownPhase::NativeTerminal;
                true
            }
            WindowPresentationShutdownPhase::NativeTerminal => true,
            WindowPresentationShutdownPhase::TerminalBeforeQuiesce => false,
        }
    }
}

/// A backend-owned presentation shutdown prepared against one exact window generation.
///
/// Preparation may run while GPUI owns its application borrow, so it must only snapshot
/// backend-owned memory. The retained operation performs renderer quiescence later, after the
/// application borrow has been returned. Retries reuse the same exact authority.
#[doc(hidden)]
#[derive(Clone)]
pub struct PreparedPlatformPresentationShutdown {
    ticket: WindowPresentationShutdownTicket,
    quiesce: Rc<dyn Fn(&WindowPresentationShutdownTicket) -> PlatformPresentationShutdownOutcome>,
}

impl PreparedPlatformPresentationShutdown {
    /// Creates a prepared presentation shutdown for a platform backend.
    #[doc(hidden)]
    pub fn new(
        ticket: WindowPresentationShutdownTicket,
        quiesce: impl Fn(&WindowPresentationShutdownTicket) -> PlatformPresentationShutdownOutcome
        + 'static,
    ) -> Self {
        Self {
            ticket,
            quiesce: Rc::new(quiesce),
        }
    }

    /// Returns immutable facts for the exact shutdown authority.
    pub fn snapshot(&self) -> WindowPresentationShutdownSnapshot {
        self.ticket.snapshot()
    }

    pub(crate) fn ticket(&self) -> &WindowPresentationShutdownTicket {
        &self.ticket
    }

    pub(crate) fn same_authority(&self, other: &Self) -> bool {
        self.ticket.same_authority(&other.ticket)
    }

    pub(crate) fn quiesce(&self) -> PlatformPresentationShutdownOutcome {
        let outcome = (self.quiesce)(&self.ticket);
        if outcome == PlatformPresentationShutdownOutcome::Quiesced
            && !self.ticket.snapshot().quiesced()
        {
            PlatformPresentationShutdownOutcome::Rejected
        } else {
            outcome
        }
    }
}

/// A backend-owned pointer-capture release prepared against one exact pointer session.
///
/// Preparation may run while GPUI owns its application borrow, so it must only snapshot
/// backend-owned memory. The retained operation performs the native effect later, after the
/// application borrow has been returned. Retries clone and reuse the same snapshot.
#[doc(hidden)]
#[derive(Clone)]
pub struct PreparedPlatformPointerCaptureRelease {
    dispatch: Rc<dyn Fn() -> PlatformPointerCaptureReleaseOutcome>,
}

impl PreparedPlatformPointerCaptureRelease {
    #[doc(hidden)]
    pub fn new(dispatch: impl Fn() -> PlatformPointerCaptureReleaseOutcome + 'static) -> Self {
        Self {
            dispatch: Rc::new(dispatch),
        }
    }

    pub(crate) fn dispatch(&self) -> PlatformPointerCaptureReleaseOutcome {
        (self.dispatch)()
    }
}

/// One must-immediate native input callback installed into a platform backend.
#[doc(hidden)]
pub struct PlatformInputCallback {
    cx: Option<AsyncApp>,
    diagnostic_target: Option<NativeInputDiagnosticTarget>,
    callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    allow_unleased_test_dispatch: bool,
}

impl PlatformInputCallback {
    /// Creates a callback for a platform adapter or backend integration test.
    #[doc(hidden)]
    pub fn new(
        cx: AsyncApp,
        callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    ) -> Self {
        Self {
            cx: Some(cx),
            diagnostic_target: None,
            callback,
            allow_unleased_test_dispatch: false,
        }
    }

    pub(crate) fn new_for_window(
        cx: AsyncApp,
        window_id: WindowId,
        callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    ) -> Self {
        let diagnostic_target = Some(NativeInputDiagnosticTarget {
            app: cx.app.clone(),
            window_id,
        });
        Self {
            cx: Some(cx),
            diagnostic_target,
            callback,
            allow_unleased_test_dispatch: false,
        }
    }

    /// Creates an isolated callback for backend state-machine tests that do not construct an app.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn new_unleased_for_test(
        callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    ) -> Self {
        Self {
            cx: None,
            diagnostic_target: None,
            callback,
            allow_unleased_test_dispatch: true,
        }
    }

    fn begin_native_callback_lease(
        &self,
        generation: u64,
    ) -> Option<crate::app::NativeCallbackLease> {
        self.cx.as_ref()?.begin_platform_input_lease(generation)
    }
}

/// A must-immediate native input callback that could not return a real handler result.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeInputInvariantViolation {
    pub window_id: WindowId,
    pub boundary: NativeInputBoundary,
    pub slot_generation: Option<u64>,
    pub failure: NativeInvariantFailure,
}

impl NativeInputInvariantViolation {
    pub(crate) fn new(
        window_id: WindowId,
        boundary: NativeInputBoundary,
        slot_generation: Option<u64>,
        failure: NativeInvariantFailure,
    ) -> Self {
        Self {
            window_id,
            boundary,
            slot_generation,
            failure,
        }
    }
}

impl std::fmt::Display for NativeInputInvariantViolation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "must-immediate native input invariant failed: window={:?} boundary={:?} slot_generation={:?} failure={:?}",
            self.window_id, self.boundary, self.slot_generation, self.failure
        )
    }
}

impl std::error::Error for NativeInputInvariantViolation {}

#[derive(Clone)]
struct NativeInputDiagnosticTarget {
    app: Weak<AppCell>,
    window_id: WindowId,
}

impl NativeInputDiagnosticTarget {
    fn reserve_reentrant_pointer_cancel(
        &self,
        slot_generation: u64,
        reason: PointerCancelReason,
    ) -> NativePointerCancelReservation {
        let Some(app) = self.app.upgrade() else {
            return NativePointerCancelReservation::ApplicationGone;
        };
        if app.reserve_reentrant_pointer_cancel(self.window_id, slot_generation, reason) {
            NativePointerCancelReservation::Reserved
        } else {
            NativePointerCancelReservation::IngressClosed
        }
    }

    fn record_invariant(
        &self,
        boundary: NativeInputBoundary,
        generation: u64,
        failure: NativeInvariantFailure,
    ) {
        if let Some(app) = self.app.upgrade() {
            app.record_native_input_slot_invariant(self.window_id, boundary, generation, failure);
        }
    }

    fn panic_invariant(
        &self,
        boundary: NativeInputBoundary,
        generation: u64,
        failure: NativeInvariantFailure,
    ) -> ! {
        self.record_invariant(boundary, generation, failure);
        std::panic::panic_any(NativeInputInvariantViolation::new(
            self.window_id,
            boundary,
            Some(generation),
            failure,
        ))
    }
}

/// Result of locking a terminal pointer cancellation at a native callback boundary.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePointerCancelReservation {
    Reserved,
    UnleasedTestFallback,
    NoActiveCallback,
    MissingDiagnosticTarget,
    ApplicationGone,
    IngressClosed,
    RetiredSlot,
}

#[derive(Clone)]
struct PlatformInputPanicRecovery {
    generation: u64,
    target: NativeInputDiagnosticTarget,
}

#[derive(Default)]
struct PlatformInputCallbackSlotState {
    generation: u64,
    checked_out_generation: Option<u64>,
    callback: Option<PlatformInputCallback>,
    diagnostic_target: Option<NativeInputDiagnosticTarget>,
    panic_recovery: Option<PlatformInputPanicRecovery>,
    allow_unleased_test_dispatch: bool,
    terminal: bool,
}

/// A generation-aware slot for the must-immediate native input callback.
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct PlatformInputCallbackSlot {
    state: Rc<RefCell<PlatformInputCallbackSlotState>>,
}

impl PlatformInputCallbackSlot {
    /// Installs a callback and invalidates any callback currently checked out by a native stack.
    pub fn set(&self, callback: PlatformInputCallback) {
        let previous = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.generation = state
                .generation
                .checked_add(1)
                .expect("platform input callback generation overflowed");
            state.panic_recovery = None;
            state.allow_unleased_test_dispatch = callback.allow_unleased_test_dispatch;
            state.diagnostic_target = callback.diagnostic_target.clone();
            state.callback.replace(callback)
        };
        drop(previous);
    }

    /// Dispatches one native input and returns the exact current handler result.
    ///
    /// A missing or retired callback is an invariant violation because no fixed propagation
    /// result is equivalent to the current handler result.
    pub fn dispatch(&self, input: PlatformInput) -> DispatchEventResult {
        let mut checkout = PlatformInputCallbackCheckout::new(self.clone());
        if checkout.callback_lease.is_none()
            && !checkout.callback_mut().allow_unleased_test_dispatch
        {
            checkout.panic_invariant(NativeInvariantFailure::MissingLease);
        }
        (checkout.callback_mut().callback)(input)
    }

    /// Reserves a terminal pointer-cancel fact while the ordinary input callback is checked out.
    ///
    /// This is intentionally narrower than deferred raw-input dispatch. Native backends use it
    /// only when capture loss synchronously re-enters the active callback stack.
    #[doc(hidden)]
    pub fn reserve_reentrant_pointer_cancel(
        &self,
        reason: PointerCancelReason,
    ) -> NativePointerCancelReservation {
        let (slot_generation, target) = {
            let state = self.state.borrow();
            let Some(slot_generation) = state.checked_out_generation else {
                return NativePointerCancelReservation::NoActiveCallback;
            };
            let Some(target) = state.diagnostic_target.clone() else {
                return if state.allow_unleased_test_dispatch {
                    NativePointerCancelReservation::UnleasedTestFallback
                } else {
                    NativePointerCancelReservation::MissingDiagnosticTarget
                };
            };
            (slot_generation, target)
        };
        target.reserve_reentrant_pointer_cancel(slot_generation, reason)
    }

    /// Locks a terminal cancellation after a native backend catches an input panic.
    #[doc(hidden)]
    pub fn reserve_pointer_cancel_after_callback_panic(
        &self,
        reason: PointerCancelReason,
    ) -> NativePointerCancelReservation {
        let (slot_generation, target) = {
            let mut state = self.state.borrow_mut();
            if let Some(slot_generation) = state.checked_out_generation {
                let Some(target) = state.diagnostic_target.clone() else {
                    return if state.allow_unleased_test_dispatch {
                        NativePointerCancelReservation::UnleasedTestFallback
                    } else {
                        NativePointerCancelReservation::MissingDiagnosticTarget
                    };
                };
                (slot_generation, target)
            } else if let Some(recovery) = state.panic_recovery.take() {
                if state.terminal || recovery.generation != state.generation {
                    return NativePointerCancelReservation::NoActiveCallback;
                }
                (recovery.generation, recovery.target)
            } else if state.terminal {
                return NativePointerCancelReservation::RetiredSlot;
            } else {
                return NativePointerCancelReservation::NoActiveCallback;
            }
        };
        target.reserve_reentrant_pointer_cancel(slot_generation, reason)
    }

    /// Permanently retires this window's callback slot.
    pub fn terminate(&self) {
        let callback = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.terminal = true;
            state.generation = state
                .generation
                .checked_add(1)
                .expect("platform input callback generation overflowed");
            state.callback.take()
        };
        drop(callback);
    }
}

struct PlatformInputCallbackCheckout {
    slot: PlatformInputCallbackSlot,
    generation: u64,
    callback: Option<PlatformInputCallback>,
    callback_lease: Option<crate::app::NativeCallbackLease>,
}

impl PlatformInputCallbackCheckout {
    fn new(slot: PlatformInputCallbackSlot) -> Self {
        let (generation, callback) = {
            let mut state = slot.state.borrow_mut();
            if state.terminal {
                let generation = state.generation;
                let diagnostic_target = state.diagnostic_target.clone();
                drop(state);
                if let Some(target) = diagnostic_target {
                    target.panic_invariant(
                        NativeInputBoundary::PlatformInput,
                        generation,
                        NativeInvariantFailure::RetiredSlot,
                    );
                }
                panic!(
                    "retired platform input callback slot generation={generation} has no diagnostic target"
                );
            }
            if state.checked_out_generation.is_some() {
                let generation = state.generation;
                let diagnostic_target = state.diagnostic_target.clone();
                drop(state);
                if let Some(target) = diagnostic_target {
                    target.panic_invariant(
                        NativeInputBoundary::PlatformInput,
                        generation,
                        NativeInvariantFailure::SlotReentry,
                    );
                }
                panic!(
                    "unbound platform input callback slot generation={generation} re-entered without a diagnostic target"
                );
            }
            if state.callback.is_none() {
                let generation = state.generation;
                let diagnostic_target = state.diagnostic_target.clone();
                drop(state);
                if let Some(target) = diagnostic_target {
                    target.panic_invariant(
                        NativeInputBoundary::PlatformInput,
                        generation,
                        NativeInvariantFailure::MissingSlot,
                    );
                }
                panic!(
                    "unbound platform input callback slot generation={generation} has no diagnostic target"
                );
            }
            let callback = state
                .callback
                .take()
                .expect("checked platform input callback must remain installed");
            let generation = state.generation;
            state.panic_recovery = None;
            state.checked_out_generation = Some(generation);
            (generation, callback)
        };
        let callback_lease = callback.begin_native_callback_lease(generation);
        Self {
            slot,
            generation,
            callback: Some(callback),
            callback_lease,
        }
    }

    fn callback_mut(&mut self) -> &mut PlatformInputCallback {
        self.callback
            .as_mut()
            .expect("checked-out platform input callback must remain available")
    }

    fn panic_invariant(&self, failure: NativeInvariantFailure) -> ! {
        let target = self
            .callback
            .as_ref()
            .and_then(|callback| callback.diagnostic_target.as_ref())
            .expect("leased platform input callback must have a diagnostic target");
        target.panic_invariant(NativeInputBoundary::PlatformInput, self.generation, failure)
    }
}

impl Drop for PlatformInputCallbackCheckout {
    fn drop(&mut self) {
        let panic_recovery = std::thread::panicking().then(|| {
            self.callback
                .as_ref()
                .and_then(|callback| callback.diagnostic_target.clone())
                .map(|target| PlatformInputPanicRecovery {
                    generation: self.generation,
                    target,
                })
        });
        let retired_callback = {
            let mut state = self.slot.state.borrow_mut();
            if state.checked_out_generation == Some(self.generation) {
                state.checked_out_generation = None;
                if state.generation == self.generation && state.callback.is_none() {
                    state.callback = self.callback.take();
                }
            }
            if let Some(panic_recovery) = panic_recovery.flatten()
                && !state.terminal
                && state.generation == self.generation
            {
                state.panic_recovery = Some(panic_recovery);
            }
            self.callback.take()
        };
        let callback_lease = self.callback_lease.take();
        drop(retired_callback);
        drop(callback_lease);
    }
}

/// A cloneable backend dispatcher for [`PlatformWindowCommand`].
///
/// Backend implementations should capture weak native-window state so queued commands cannot
/// extend native window lifetime.
#[doc(hidden)]
#[derive(Clone)]
pub struct PlatformWindowCommandDispatcher {
    dispatch_command: Rc<dyn Fn(PlatformWindowCommand) -> PlatformWindowCommandOutcome>,
    prepare_pointer_capture_release: Rc<dyn Fn(u64) -> PreparedPlatformPointerCaptureRelease>,
}

impl PlatformWindowCommandDispatcher {
    #[doc(hidden)]
    pub fn new(
        dispatch: impl Fn(PlatformWindowCommand) -> PlatformWindowCommandOutcome + 'static,
    ) -> Self {
        Self {
            dispatch_command: Rc::new(dispatch),
            prepare_pointer_capture_release: Rc::new(|_| {
                PreparedPlatformPointerCaptureRelease::new(|| {
                    PlatformPointerCaptureReleaseOutcome::Released
                })
            }),
        }
    }

    /// Creates a dispatcher with a backend-owned native pointer-capture release operation.
    #[doc(hidden)]
    pub fn new_with_pointer_capture_release(
        dispatch: impl Fn(PlatformWindowCommand) -> PlatformWindowCommandOutcome + 'static,
        prepare_pointer_capture_release: impl Fn(u64) -> PreparedPlatformPointerCaptureRelease + 'static,
    ) -> Self {
        Self {
            dispatch_command: Rc::new(dispatch),
            prepare_pointer_capture_release: Rc::new(prepare_pointer_capture_release),
        }
    }

    pub(crate) fn dispatch(&self, command: PlatformWindowCommand) -> PlatformWindowCommandOutcome {
        (self.dispatch_command)(command)
    }

    pub(crate) fn prepare_pointer_capture_release(
        &self,
        release_generation: u64,
    ) -> PreparedPlatformPointerCaptureRelease {
        (self.prepare_pointer_capture_release)(release_generation)
    }
}

impl Debug for PlatformWindowCommandDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlatformWindowCommandDispatcher")
            .finish_non_exhaustive()
    }
}

/// A type to describe the appearance of a window
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum WindowDecorations {
    #[default]
    /// Server side decorations
    Server,
    /// Client side decorations
    Client,
}

/// A type to describe how this window is currently configured
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum Decorations {
    /// The window is configured to use server side decorations
    #[default]
    Server,
    /// The window is configured to use client side decorations
    Client {
        /// The edge tiling state
        tiling: Tiling,
    },
}

/// What window controls this platform supports
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct WindowControls {
    /// Whether this platform supports fullscreen
    pub fullscreen: bool,
    /// Whether this platform supports maximize
    pub maximize: bool,
    /// Whether this platform supports minimize
    pub minimize: bool,
    /// Whether this platform supports a window menu
    pub window_menu: bool,
}

impl Default for WindowControls {
    fn default() -> Self {
        // Assume that we can do anything, unless told otherwise
        Self {
            fullscreen: true,
            maximize: true,
            minimize: true,
            window_menu: true,
        }
    }
}

/// A window control button type used in [`WindowButtonLayout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowButton {
    /// The minimize button
    Minimize,
    /// The maximize button
    Maximize,
    /// The close button
    Close,
}

impl WindowButton {
    /// Returns a stable element ID for rendering this button.
    pub fn id(&self) -> &'static str {
        match self {
            WindowButton::Minimize => "minimize",
            WindowButton::Maximize => "maximize",
            WindowButton::Close => "close",
        }
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn index(&self) -> usize {
        match self {
            WindowButton::Minimize => 0,
            WindowButton::Maximize => 1,
            WindowButton::Close => 2,
        }
    }
}

/// Maximum number of [`WindowButton`]s per side in the titlebar.
pub const MAX_BUTTONS_PER_SIDE: usize = 3;

/// Describes which [`WindowButton`]s appear on each side of the titlebar.
///
/// On Linux, this is read from the desktop environment's configuration
/// (e.g. GNOME's `gtk-decoration-layout` gsetting) via [`WindowButtonLayout::parse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowButtonLayout {
    /// Buttons on the left side of the titlebar.
    pub left: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
    /// Buttons on the right side of the titlebar.
    pub right: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl WindowButtonLayout {
    /// Returns Open GPUI's built-in fallback button layout for Linux titlebars.
    pub fn linux_default() -> Self {
        Self {
            left: [None; MAX_BUTTONS_PER_SIDE],
            right: [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close),
            ],
        }
    }

    /// Parses a GNOME-style `button-layout` string (e.g. `"close,minimize:maximize"`).
    pub fn parse(layout_string: &str) -> Result<Self> {
        fn parse_side(
            s: &str,
            seen_buttons: &mut [bool; MAX_BUTTONS_PER_SIDE],
            unrecognized: &mut Vec<String>,
        ) -> [Option<WindowButton>; MAX_BUTTONS_PER_SIDE] {
            let mut result = [None; MAX_BUTTONS_PER_SIDE];
            let mut i = 0;
            for name in s.split(',') {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let button = match trimmed {
                    "minimize" => Some(WindowButton::Minimize),
                    "maximize" => Some(WindowButton::Maximize),
                    "close" => Some(WindowButton::Close),
                    other => {
                        unrecognized.push(other.to_string());
                        None
                    }
                };
                if let Some(button) = button {
                    if seen_buttons[button.index()] {
                        continue;
                    }
                    if let Some(slot) = result.get_mut(i) {
                        *slot = Some(button);
                        seen_buttons[button.index()] = true;
                        i += 1;
                    }
                }
            }
            result
        }

        let (left_str, right_str) = layout_string.split_once(':').unwrap_or(("", layout_string));
        let mut unrecognized = Vec::new();
        let mut seen_buttons = [false; MAX_BUTTONS_PER_SIDE];
        let layout = Self {
            left: parse_side(left_str, &mut seen_buttons, &mut unrecognized),
            right: parse_side(right_str, &mut seen_buttons, &mut unrecognized),
        };

        if !unrecognized.is_empty()
            && layout.left.iter().all(Option::is_none)
            && layout.right.iter().all(Option::is_none)
        {
            bail!(
                "button layout string {:?} contains no valid buttons (unrecognized: {})",
                layout_string,
                unrecognized.join(", ")
            );
        }

        Ok(layout)
    }

    /// Formats the layout back into a GNOME-style `button-layout` string.
    #[cfg(test)]
    pub fn format(&self) -> String {
        fn format_side(buttons: &[Option<WindowButton>; MAX_BUTTONS_PER_SIDE]) -> String {
            buttons
                .iter()
                .flatten()
                .map(|button| match button {
                    WindowButton::Minimize => "minimize",
                    WindowButton::Maximize => "maximize",
                    WindowButton::Close => "close",
                })
                .collect::<Vec<_>>()
                .join(",")
        }

        format!("{}:{}", format_side(&self.left), format_side(&self.right))
    }
}

/// A type to describe which sides of the window are currently tiled in some way
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct Tiling {
    /// Whether the top edge is tiled
    pub top: bool,
    /// Whether the left edge is tiled
    pub left: bool,
    /// Whether the right edge is tiled
    pub right: bool,
    /// Whether the bottom edge is tiled
    pub bottom: bool,
}

impl Tiling {
    /// Initializes a [`Tiling`] type with all sides tiled
    pub fn tiled() -> Self {
        Self {
            top: true,
            left: true,
            right: true,
            bottom: true,
        }
    }

    /// Whether any edge is tiled
    pub fn is_tiled(&self) -> bool {
        self.top || self.left || self.right || self.bottom
    }
}

/// Callbacks for the accessibility adapter.
pub struct A11yCallbacks {
    /// Called when the adapter is activated (a screen reader connects).
    pub activation: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
    /// Called when an action is requested by the screen reader.
    pub action: Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>,
    /// Called when the adapter is deactivated (screen reader disconnects).
    pub deactivation: Box<dyn Fn() + Send + 'static>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[expect(missing_docs)]
pub struct RequestFrameOptions {
    /// Whether a presentation is required.
    pub require_presentation: bool,
    /// Force refresh of all rendering states when true.
    pub force_render: bool,
}

#[expect(missing_docs)]
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher;
    /// Prepares renderer quiescence without performing native or renderer effects.
    ///
    /// The returned operation is dispatched only after GPUI has released its application borrow.
    #[doc(hidden)]
    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown;
    /// Begins native-window retirement without consuming the platform-window owner.
    ///
    /// Backends whose object drop is an infallible retirement request may use the default. A
    /// backend with a fallible native destroy operation must report rejection so GPUI can retain
    /// the owner and retry.
    #[doc(hidden)]
    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        if shutdown.snapshot().quiesced() {
            PlatformNativeWindowRetirementOutcome::Accepted
        } else {
            PlatformNativeWindowRetirementOutcome::Rejected
        }
    }
    fn bounds(&self) -> Bounds<Pixels>;
    /// Returns one stable physical client-geometry observation when supported.
    fn physical_geometry(&self) -> Option<PlatformWindowPhysicalGeometry> {
        None
    }
    /// Returns the physical pointer frame scoped to the active native input callback.
    #[doc(hidden)]
    fn native_pointer_physical_frame(&self) -> Option<PlatformNativePointerPhysicalFrame> {
        None
    }
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool {
        false
    }
    fn window_bounds(&self) -> WindowBounds;
    fn content_size(&self) -> Size<Pixels>;
    fn scale_factor(&self) -> f32;
    fn appearance(&self) -> WindowAppearance;
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    fn mouse_position(&self) -> Point<Pixels>;
    fn set_cursor_style(&self, style: CursorStyle);
    fn modifiers(&self) -> Modifiers;
    fn capslock(&self) -> Capslock;
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler);
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler>;
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    fn input_handler_slot_for_test(&self) -> Option<PlatformInputHandlerSlot> {
        None
    }
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>>;
    fn is_active(&self) -> bool;
    fn is_hovered(&self) -> bool;
    fn accepts_pointer_input(&self) -> bool {
        true
    }
    /// Returns immutable facts established by the backend during window creation.
    fn creation_facts(&self) -> WindowCreationFacts;
    /// Returns whether the native window is currently visible or mapped.
    fn is_visible(&self) -> bool;
    /// Returns one coherent snapshot of the facts currently observed by this backend.
    ///
    /// Backends that can report a shared desktop coordinate system should override this method
    /// and set [`WindowPlatformFacts::coordinate_space`] to
    /// [`WindowCoordinateSpace::GlobalScreen`].
    fn platform_facts(&self) -> WindowPlatformFacts {
        WindowPlatformFacts {
            bounds: self.bounds(),
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            window_bounds: self.window_bounds(),
            inner_window_bounds: self.inner_window_bounds(),
            content_size: self.content_size(),
            scale_factor: self.scale_factor(),
            display_id: self.display().map(|display| display.id()),
            is_minimized: self.is_minimized(),
            is_maximized: self.is_maximized(),
            is_fullscreen: self.is_fullscreen(),
            accepts_pointer_input: self.accepts_pointer_input(),
            accepts_activation: true,
            focus_on_click: true,
            background_appearance: self.background_appearance(),
            topmost: false,
            taskbar_visible: true,
            is_active: self.is_active(),
        }
    }
    /// Prepares a generation to become the only current request in one conflict domain.
    ///
    /// GPUI calls this before deciding whether the request is unchanged, unsupported, or handed
    /// to [`Self::request_window_mutation`]. Backends with queued native work must make every
    /// older generation in `domain` unable to mutate the window or emit a terminal observation.
    fn prepare_window_mutation(&self, _domain: WindowMutationDomain, _generation: u64) {}
    /// Requests one typed mutation for an already-open window.
    ///
    /// The default preserves the getter-only contract. It may report an unchanged request from
    /// committed facts, but never infers live support from a legacy unit or boolean setter.
    fn request_window_mutation(
        &mut self,
        _generation: u64,
        request: WindowMutationRequest,
    ) -> PlatformWindowDispatch {
        if request.matches_facts(&self.platform_facts()) {
            PlatformWindowDispatch::Unchanged
        } else {
            PlatformWindowDispatch::Unsupported
        }
    }
    /// Invalidates any backend work that could later emit a terminal observation for `domain`.
    ///
    /// GPUI calls this when a window closes without installing a replacement generation.
    ///
    /// Backends must prevent queued work in `domain` from mutating the native window or emitting
    /// a terminal observation after this call.
    fn invalidate_window_mutation(&self, _domain: WindowMutationDomain) {}
    fn background_appearance(&self) -> WindowBackgroundAppearance;
    fn set_title(&mut self, title: &str);
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance);
    fn is_fullscreen(&self) -> bool;
    /// Requests one backend frame callback without requiring native visibility.
    #[doc(hidden)]
    fn request_frame(&self, _options: RequestFrameOptions) {}
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>);
    fn on_input(&self, callback: PlatformInputCallback);
    fn on_modifiers_changed(&self, _callback: Box<dyn FnMut(ModifiersChangedEvent)>) {}
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>);
    fn on_moved(&self, callback: Box<dyn FnMut()>);
    /// Registers a callback for externally observed window-state changes that need not resize or
    /// move the window, such as minimization.
    ///
    /// This callback refreshes committed platform facts. It does not settle a mutation ticket;
    /// queued mutations settle only through [`Self::on_window_mutation_observation`].
    fn on_window_state_change(&self, _callback: Box<dyn FnMut()>) {}
    /// Registers a callback for a coherent terminal window-mutation observation.
    ///
    /// This callback must not be used for intermediate move or resize notifications. Backends
    /// invoke it only after they can read one coherent [`WindowPlatformFacts`] snapshot for a
    /// queued placement or independent-flag request. The supplied facts snapshot must be the
    /// exact observation that settled the native operation; GPUI must not re-read getters later.
    /// Backends report asynchronous errors with the observation's explicit
    /// [`PlatformWindowMutationTerminal`] instead of presenting the unchanged facts as an
    /// OS-adjusted success.
    fn on_window_mutation_observation(
        &self,
        _callback: Box<dyn FnMut(PlatformWindowMutationObservation)>,
    ) {
    }
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>);
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>);
    /// Registers the single-shot terminal callback for this platform-window authority.
    ///
    /// Every backend must invoke it exactly once after its native surface or drop-only window
    /// authority can no longer produce events. Backends without an external window server invoke
    /// it while dropping their platform-window owner.
    fn on_close(&self, callback: Box<dyn FnOnce()>);
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>);
    fn on_button_layout_changed(&self, _callback: Box<dyn FnMut()>) {}
    /// Submits one frame to the platform renderer.
    ///
    /// A deferred or rejected submission must not be reported as presented by GPUI.
    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome;
    fn completed_frame(&self) {}
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
    fn is_subpixel_rendering_supported(&self) -> bool;

    // macOS specific methods
    fn get_title(&self) -> String {
        String::new()
    }
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        None
    }
    fn tab_bar_visible(&self) -> bool {
        false
    }
    fn set_edited(&mut self, _edited: bool) {}
    fn set_document_path(&self, _path: Option<&std::path::Path>) {}
    #[cfg(target_os = "macos")]
    fn set_traffic_light_position(&self, _position: Point<Pixels>) {}
    fn show_character_palette(&self) {}
    fn titlebar_double_click(&self) {}
    fn on_move_tab_to_new_window(&self, _callback: Box<dyn FnMut()>) {}
    fn on_merge_all_windows(&self, _callback: Box<dyn FnMut()>) {}
    fn on_select_previous_tab(&self, _callback: Box<dyn FnMut()>) {}
    fn on_select_next_tab(&self, _callback: Box<dyn FnMut()>) {}
    fn on_toggle_tab_bar(&self, _callback: Box<dyn FnMut()>) {}
    fn merge_all_windows(&self) {}
    fn move_tab_to_new_window(&self) {}
    fn toggle_window_tab_overview(&self) {}
    fn set_tabbing_identifier(&self, _identifier: Option<String>) {}

    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND;

    // Linux specific methods
    fn inner_window_bounds(&self) -> WindowBounds {
        self.window_bounds()
    }
    fn request_decorations(&self, _decorations: WindowDecorations) {}
    fn window_decorations(&self) -> Decorations {
        Decorations::Server
    }
    fn set_app_id(&mut self, _app_id: &str) {}
    fn map_window(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn window_controls(&self) -> WindowControls {
        WindowControls::default()
    }
    fn set_client_inset(&self, _inset: Pixels) {}
    fn gpu_specs(&self) -> Option<GpuSpecs>;

    fn update_ime_position(&self, _bounds: Bounds<Pixels>);

    fn play_system_bell(&self) {}

    /// Initialize the accessibility adapter with callbacks.
    fn a11y_init(&self, _callbacks: A11yCallbacks) {}

    /// Provide a TreeUpdate to the accessibility adapter.
    fn a11y_tree_update(&self, _tree_update: accesskit::TreeUpdate) {}

    /// Inform the adapter of updated window bounds.
    fn a11y_update_window_bounds(&self) {}

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&mut self) -> Option<&mut TestWindow> {
        None
    }

    /// Renders the given scene to a texture and returns the pixel data as an RGBA image.
    /// This does not present the frame to screen - useful for visual testing where we want
    /// to capture what would be rendered without displaying it or requiring the window to be visible.
    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(&self, _scene: &Scene) -> Result<RgbaImage> {
        anyhow::bail!("render_to_image not implemented for this platform")
    }
}

/// Result of handing a rendered scene to a platform window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformWindowPresentOutcome {
    /// The renderer submitted the frame to its presentation path.
    Submitted,
    /// The native surface is temporarily unable to accept a frame.
    Deferred,
    /// The renderer or native surface rejected the frame.
    Rejected,
}

/// A renderer for headless windows that can produce real rendered output.
#[cfg(any(test, feature = "test-support"))]
pub trait PlatformHeadlessRenderer {
    /// Render a scene and return the result as an RGBA image.
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage>;

    /// Returns the sprite atlas used by this renderer.
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
}

/// Type alias for runnables with metadata.
/// Previously an enum with a single variant, now simplified to a direct type alias.
#[doc(hidden)]
pub type RunnableVariant = Runnable<RunnableMeta>;

#[doc(hidden)]
pub type TimerResolutionGuard = open_gpui_core_util::Deferred<Box<dyn FnOnce() + Send>>;

#[doc(hidden)]
pub enum TasksIncluded {
    OnlyCompleted,
    CompletedAndRunning,
}

/// This type is public so that our test macro can generate and use it, but it should not
/// be considered part of our public API.
#[doc(hidden)]
pub trait PlatformDispatcher: Send + Sync {
    fn is_main_thread(&self) -> bool;
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority);
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority);
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant);

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>);

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn increase_timer_resolution(&self) -> TimerResolutionGuard {
        open_gpui_core_util::defer(Box::new(|| {}))
    }

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&self) -> Option<&TestDispatcher> {
        None
    }
}

#[expect(missing_docs)]
pub trait PlatformTextSystem: Send + Sync {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>;
    /// Get all available font names.
    fn all_font_names(&self) -> Vec<String>;
    /// Get the font ID for a font descriptor.
    fn font_id(&self, descriptor: &Font) -> Result<FontId>;
    /// Get metrics for a font.
    fn font_metrics(&self, font_id: FontId) -> FontMetrics;
    /// Get typographic bounds for a glyph.
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>>;
    /// Get the advance width for a glyph.
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>>;
    /// Get the glyph ID for a character.
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId>;
    /// Get raster bounds for a glyph.
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>>;
    /// Rasterize a glyph.
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)>;
    /// Layout a line of text with the given font runs.
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout;
    /// Returns the recommended text rendering mode for the given font and size.
    fn recommended_rendering_mode(&self, _font_id: FontId, _font_size: Pixels)
    -> TextRenderingMode;
    /// Returns the dilation level to use for a glyph painted in the given color.
    fn glyph_dilation_for_color(&self, _color: Hsla) -> u8 {
        0
    }
}

#[expect(missing_docs)]
pub struct NoopTextSystem;

#[expect(missing_docs)]
impl NoopTextSystem {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl PlatformTextSystem for NoopTextSystem {
    fn add_fonts(&self, _fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn font_id(&self, _descriptor: &Font) -> Result<FontId> {
        Ok(FontId(1))
    }

    fn font_metrics(&self, _font_id: FontId) -> FontMetrics {
        FontMetrics {
            units_per_em: 1000,
            ascent: 1025.0,
            descent: -275.0,
            line_gap: 0.0,
            underline_position: -95.0,
            underline_thickness: 60.0,
            cap_height: 698.0,
            x_height: 516.0,
            bounding_box: Bounds {
                origin: Point {
                    x: -260.0,
                    y: -245.0,
                },
                size: Size {
                    width: 1501.0,
                    height: 1364.0,
                },
            },
        }
    }

    fn typographic_bounds(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Bounds<f32>> {
        Ok(Bounds {
            origin: Point { x: 54.0, y: 0.0 },
            size: size(392.0, 528.0),
        })
    }

    fn advance(&self, _font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        Ok(size(600.0 * glyph_id.0 as f32, 0.0))
    }

    fn glyph_for_char(&self, _font_id: FontId, ch: char) -> Option<GlyphId> {
        Some(GlyphId(ch.len_utf16() as u32))
    }

    fn glyph_raster_bounds(&self, _params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        Ok(Default::default())
    }

    fn rasterize_glyph(
        &self,
        _params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        Ok((raster_bounds.size, Vec::new()))
    }

    fn layout_line(&self, text: &str, font_size: Pixels, _runs: &[FontRun]) -> LineLayout {
        let mut position = px(0.);
        let metrics = self.font_metrics(FontId(0));
        let em_width = font_size
            * self
                .advance(FontId(0), self.glyph_for_char(FontId(0), 'm').unwrap())
                .unwrap()
                .width
            / metrics.units_per_em as f32;
        let mut glyphs = Vec::new();
        for (ix, c) in text.char_indices() {
            if let Some(glyph) = self.glyph_for_char(FontId(0), c) {
                glyphs.push(ShapedGlyph {
                    id: glyph,
                    position: point(position, px(0.)),
                    index: ix,
                    is_emoji: glyph.0 == 2,
                });
                if glyph.0 == 2 {
                    position += em_width * 2.0;
                } else {
                    position += em_width;
                }
            } else {
                position += em_width
            }
        }
        let mut runs = Vec::default();
        if !glyphs.is_empty() {
            runs.push(ShapedRun {
                font_id: FontId(0),
                glyphs,
            });
        } else {
            position = px(0.);
        }

        LineLayout {
            font_size,
            width: position,
            ascent: font_size * (metrics.ascent / metrics.units_per_em as f32),
            descent: font_size * (metrics.descent / metrics.units_per_em as f32),
            runs,
            len: text.len(),
        }
    }

    fn recommended_rendering_mode(
        &self,
        _font_id: FontId,
        _font_size: Pixels,
    ) -> TextRenderingMode {
        TextRenderingMode::Grayscale
    }
}

// Adapted from https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.cpp
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
/// Compute gamma correction ratios for subpixel text rendering.
#[allow(dead_code)]
pub fn get_gamma_correction_ratios(gamma: f32) -> [f32; 4] {
    const GAMMA_INCORRECT_TARGET_RATIOS: [[f32; 4]; 13] = [
        [0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0], // gamma = 1.0
        [0.0166 / 4.0, -0.0807 / 4.0, 0.2227 / 4.0, -0.0751 / 4.0], // gamma = 1.1
        [0.0350 / 4.0, -0.1760 / 4.0, 0.4325 / 4.0, -0.1370 / 4.0], // gamma = 1.2
        [0.0543 / 4.0, -0.2821 / 4.0, 0.6302 / 4.0, -0.1876 / 4.0], // gamma = 1.3
        [0.0739 / 4.0, -0.3963 / 4.0, 0.8167 / 4.0, -0.2287 / 4.0], // gamma = 1.4
        [0.0933 / 4.0, -0.5161 / 4.0, 0.9926 / 4.0, -0.2616 / 4.0], // gamma = 1.5
        [0.1121 / 4.0, -0.6395 / 4.0, 1.1588 / 4.0, -0.2877 / 4.0], // gamma = 1.6
        [0.1300 / 4.0, -0.7649 / 4.0, 1.3159 / 4.0, -0.3080 / 4.0], // gamma = 1.7
        [0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0], // gamma = 1.8
        [0.1627 / 4.0, -1.0170 / 4.0, 1.6051 / 4.0, -0.3347 / 4.0], // gamma = 1.9
        [0.1773 / 4.0, -1.1420 / 4.0, 1.7385 / 4.0, -0.3426 / 4.0], // gamma = 2.0
        [0.1908 / 4.0, -1.2652 / 4.0, 1.8650 / 4.0, -0.3476 / 4.0], // gamma = 2.1
        [0.2031 / 4.0, -1.3864 / 4.0, 1.9851 / 4.0, -0.3501 / 4.0], // gamma = 2.2
    ];

    const NORM13: f32 = ((0x10000 as f64) / (255.0 * 255.0) * 4.0) as f32;
    const NORM24: f32 = ((0x100 as f64) / (255.0) * 4.0) as f32;

    let index = ((gamma * 10.0).round() as usize).clamp(10, 22) - 10;
    let ratios = GAMMA_INCORRECT_TARGET_RATIOS[index];

    [
        ratios[0] * NORM13,
        ratios[1] * NORM24,
        ratios[2] * NORM13,
        ratios[3] * NORM24,
    ]
}

#[derive(PartialEq, Eq, Hash, Clone)]
#[expect(missing_docs)]
pub enum AtlasKey {
    Glyph(RenderGlyphParams),
    Svg(RenderSvgParams),
    Image(RenderImageParams),
}

impl AtlasKey {
    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    /// Returns the texture kind for this atlas key.
    pub fn texture_kind(&self) -> AtlasTextureKind {
        match self {
            AtlasKey::Glyph(params) => {
                if params.is_emoji {
                    AtlasTextureKind::Polychrome
                } else if params.subpixel_rendering {
                    AtlasTextureKind::Subpixel
                } else {
                    AtlasTextureKind::Monochrome
                }
            }
            AtlasKey::Svg(_) => AtlasTextureKind::Monochrome,
            AtlasKey::Image(_) => AtlasTextureKind::Polychrome,
        }
    }

    /// Returns image rendering parameters when this atlas key is image-backed.
    pub fn image_params(&self) -> Option<RenderImageParams> {
        match self {
            AtlasKey::Image(params) => Some(*params),
            AtlasKey::Glyph(_) | AtlasKey::Svg(_) => None,
        }
    }
}

impl From<RenderGlyphParams> for AtlasKey {
    fn from(params: RenderGlyphParams) -> Self {
        Self::Glyph(params)
    }
}

impl From<RenderSvgParams> for AtlasKey {
    fn from(params: RenderSvgParams) -> Self {
        Self::Svg(params)
    }
}

impl From<RenderImageParams> for AtlasKey {
    fn from(params: RenderImageParams) -> Self {
        Self::Image(params)
    }
}

/// Describes how an atlas key was resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(missing_docs)]
pub enum AtlasAccessOutcome {
    Hit,
    Inserted,
    Unavailable,
    Failed,
    Unknown,
}

/// Diagnostic facts for a single atlas lookup or insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(missing_docs)]
pub struct AtlasAccessDiagnostic {
    pub image: Option<RenderImageParams>,
    pub outcome: AtlasAccessOutcome,
    pub tile: Option<AtlasTile>,
    pub texture_id: Option<AtlasTextureId>,
    pub tile_id: Option<TileId>,
    pub size: Option<Size<DevicePixels>>,
}

impl AtlasAccessDiagnostic {
    /// Builds diagnostic facts for an atlas access involving the given key.
    pub fn new(
        key: &AtlasKey,
        outcome: AtlasAccessOutcome,
        tile: Option<AtlasTile>,
        size: Option<Size<DevicePixels>>,
    ) -> Self {
        Self {
            image: key.image_params(),
            outcome,
            tile,
            texture_id: tile.map(|tile| tile.texture_id),
            tile_id: tile.map(|tile| tile.tile_id),
            size,
        }
    }
}

/// Atlas access result paired with diagnostic facts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(missing_docs)]
pub struct AtlasAccess {
    pub tile: Option<AtlasTile>,
    pub diagnostic: AtlasAccessDiagnostic,
}

/// Describes the observable result of removing an atlas key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(missing_docs)]
pub enum AtlasRemoveOutcome {
    RemoveHit,
    RemoveNoop,
    TextureRetained,
    TextureFreed,
    Unknown,
}

/// Diagnostic facts for a single atlas removal request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(missing_docs)]
pub struct AtlasRemoveDiagnostic {
    pub image: Option<RenderImageParams>,
    pub outcome: AtlasRemoveOutcome,
    pub texture_id: Option<AtlasTextureId>,
}

impl AtlasRemoveDiagnostic {
    /// Builds diagnostic facts for an atlas removal involving the given key.
    pub fn new(
        key: &AtlasKey,
        outcome: AtlasRemoveOutcome,
        texture_id: Option<AtlasTextureId>,
    ) -> Self {
        Self {
            image: key.image_params(),
            outcome,
            texture_id,
        }
    }
}

#[expect(missing_docs)]
pub trait PlatformAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>>;

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<AtlasAccess> {
        let tile = self.get_or_insert_with(key, build)?;
        let outcome = if tile.is_some() {
            AtlasAccessOutcome::Unknown
        } else {
            AtlasAccessOutcome::Unavailable
        };
        Ok(AtlasAccess {
            tile,
            diagnostic: AtlasAccessDiagnostic::new(
                key,
                outcome,
                tile,
                tile.map(|tile| tile.bounds.size),
            ),
        })
    }

    fn remove(&self, key: &AtlasKey);

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        self.remove(key);
        AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::Unknown, None)
    }
}

#[doc(hidden)]
pub struct AtlasTextureList<T> {
    pub textures: Vec<Option<T>>,
    pub free_list: Vec<usize>,
}

impl<T> Default for AtlasTextureList<T> {
    fn default() -> Self {
        Self {
            textures: Vec::default(),
            free_list: Vec::default(),
        }
    }
}

impl<T> ops::Index<usize> for AtlasTextureList<T> {
    type Output = Option<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.textures[index]
    }
}

impl<T> AtlasTextureList<T> {
    #[allow(unused)]
    pub fn drain(&mut self) -> std::vec::Drain<'_, Option<T>> {
        self.free_list.clear();
        self.textures.drain(..)
    }

    #[allow(dead_code)]
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        self.textures.iter_mut().flatten()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct AtlasTile {
    /// The texture this tile belongs to.
    pub texture_id: AtlasTextureId,
    /// The unique ID of this tile within its texture.
    pub tile_id: TileId,
    /// Padding around the tile content in pixels.
    pub padding: u32,
    /// The bounds of this tile within the texture.
    pub bounds: Bounds<DevicePixels>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
#[expect(missing_docs)]
pub struct AtlasTextureId {
    // We use u32 instead of usize for Metal Shader Language compatibility
    /// The index of this texture in the atlas.
    pub index: u32,
    /// The kind of content stored in this texture.
    pub kind: AtlasTextureKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[expect(missing_docs)]
pub enum AtlasTextureKind {
    Monochrome = 0,
    Polychrome = 1,
    Subpixel = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
#[expect(missing_docs)]
pub struct TileId(pub u32);

impl From<etagere::AllocId> for TileId {
    fn from(id: etagere::AllocId) -> Self {
        Self(id.serialize())
    }
}

impl From<TileId> for etagere::AllocId {
    fn from(id: TileId) -> Self {
        Self::deserialize(id.0)
    }
}

#[expect(missing_docs)]
pub struct PlatformInputHandler {
    cx: AsyncWindowContext,
    focus_id: FocusId,
    handler: Box<dyn InputHandler>,
    transform: ResolvedSubtreeTransform,
    validity: Option<crate::geometry::SubtreeGeometryValidity>,
}

#[derive(Default)]
struct PlatformInputHandlerSlotState {
    generation: u64,
    checked_out_generation: Option<u64>,
    handler: Option<PlatformInputHandler>,
    diagnostic_target: Option<NativeInputDiagnosticTarget>,
    terminal: bool,
}

/// A generation-aware platform input-handler slot.
///
/// Native backends must invoke handlers through [`Self::with_handler`]. If focus, teardown, or a
/// nested GPUI update replaces the handler while a callback is active, the checked-out handler is
/// retired instead of overwriting the replacement when the callback returns.
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct PlatformInputHandlerSlot {
    state: Rc<RefCell<PlatformInputHandlerSlotState>>,
}

impl PlatformInputHandlerSlot {
    /// Replaces the current handler and invalidates any handler checked out by an active callback.
    pub fn set(&self, handler: PlatformInputHandler) {
        let previous = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.generation = state
                .generation
                .checked_add(1)
                .expect("platform input-handler generation overflowed");
            state.diagnostic_target = Some(handler.diagnostic_target());
            state.handler.replace(handler)
        };
        drop(previous);
    }

    /// Takes the current handler and invalidates any handler checked out by an active callback.
    pub fn take(&self) -> Option<PlatformInputHandler> {
        let mut state = self.state.borrow_mut();
        if state.terminal {
            return None;
        }
        state.generation = state
            .generation
            .checked_add(1)
            .expect("platform input-handler generation overflowed");
        state.handler.take()
    }

    /// Runs one native callback against the current handler.
    ///
    /// Reentrant use of the same slot or entering after window retirement is an invariant
    /// violation. `None` means the live window currently has no focused text-input handler;
    /// backends retain their operation-specific absence semantics for that valid state.
    pub fn with_handler<R>(
        &self,
        callback: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        let mut checkout = PlatformInputHandlerCheckout::new(self.clone())?;
        if checkout.callback_lease.is_none() {
            checkout.panic_invariant(NativeInvariantFailure::MissingLease);
        }
        Some(callback(checkout.handler_mut()))
    }

    /// Permanently retires this window's handler slot.
    pub fn terminate(&self) {
        let handler = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.terminal = true;
            state.generation = state
                .generation
                .checked_add(1)
                .expect("platform input-handler generation overflowed");
            state.handler.take()
        };
        drop(handler);
    }
}

struct PlatformInputHandlerCheckout {
    slot: PlatformInputHandlerSlot,
    generation: u64,
    handler: Option<PlatformInputHandler>,
    callback_lease: Option<crate::app::NativeCallbackLease>,
}

impl PlatformInputHandlerCheckout {
    fn new(slot: PlatformInputHandlerSlot) -> Option<Self> {
        let (generation, handler) = {
            let mut state = slot.state.borrow_mut();
            if state.terminal {
                let generation = state.generation;
                let diagnostic_target = state.diagnostic_target.clone();
                drop(state);
                if let Some(target) = diagnostic_target {
                    target.panic_invariant(
                        NativeInputBoundary::InputHandler,
                        generation,
                        NativeInvariantFailure::RetiredSlot,
                    );
                }
                panic!(
                    "retired platform input-handler slot generation={generation} has no diagnostic target"
                );
            }
            if state.checked_out_generation.is_some() {
                let generation = state.generation;
                let diagnostic_target = state.diagnostic_target.clone();
                drop(state);
                if let Some(target) = diagnostic_target {
                    target.panic_invariant(
                        NativeInputBoundary::InputHandler,
                        generation,
                        NativeInvariantFailure::SlotReentry,
                    );
                }
                panic!(
                    "unbound platform input-handler slot generation={generation} re-entered without a diagnostic target"
                );
            }
            let handler = state.handler.take()?;
            let generation = state.generation;
            state.checked_out_generation = Some(generation);
            (generation, handler)
        };
        let callback_lease = handler.begin_native_callback_lease(generation);
        Some(Self {
            slot,
            generation,
            handler: Some(handler),
            callback_lease,
        })
    }

    fn handler_mut(&mut self) -> &mut PlatformInputHandler {
        self.handler
            .as_mut()
            .expect("checked-out platform input handler must remain available")
    }

    fn panic_invariant(&self, failure: NativeInvariantFailure) -> ! {
        self.handler
            .as_ref()
            .expect("leased platform input handler must remain available")
            .diagnostic_target()
            .panic_invariant(NativeInputBoundary::InputHandler, self.generation, failure)
    }
}

impl Drop for PlatformInputHandlerCheckout {
    fn drop(&mut self) {
        let retired_handler = {
            let mut state = self.slot.state.borrow_mut();
            if state.checked_out_generation == Some(self.generation) {
                state.checked_out_generation = None;
                if state.generation == self.generation && state.handler.is_none() {
                    state.handler = self.handler.take();
                }
            }
            self.handler.take()
        };
        let callback_lease = self.callback_lease.take();
        drop(retired_handler);
        drop(callback_lease);
    }
}

#[expect(missing_docs)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
impl PlatformInputHandler {
    pub(crate) fn new(
        cx: AsyncWindowContext,
        focus_id: FocusId,
        handler: Box<dyn InputHandler>,
        transform: ResolvedSubtreeTransform,
        validity: Option<crate::geometry::SubtreeGeometryValidity>,
    ) -> Self {
        Self {
            cx,
            focus_id,
            handler,
            transform,
            validity,
        }
    }

    fn begin_native_callback_lease(
        &self,
        generation: u64,
    ) -> Option<crate::app::NativeCallbackLease> {
        self.cx.begin_input_handler_lease(generation)
    }

    fn diagnostic_target(&self) -> NativeInputDiagnosticTarget {
        let (app, window_id) = self.cx.native_input_diagnostic_target();
        NativeInputDiagnosticTarget { app, window_id }
    }

    pub(crate) fn validity(&self) -> Option<crate::geometry::SubtreeGeometryValidity> {
        self.validity.clone()
    }

    pub(crate) fn focus_id(&self) -> FocusId {
        self.focus_id
    }

    pub(crate) fn set_validity(
        &mut self,
        validity: Option<crate::geometry::SubtreeGeometryValidity>,
    ) {
        self.validity = validity;
    }

    pub(crate) fn finish_composition(&mut self, window: &mut Window, cx: &mut App) {
        if self.handler.marked_text_range(window, cx).is_some() {
            self.handler.unmark_text(window, cx);
        }
    }

    fn update_in_input_transaction<R>(
        &mut self,
        operation: NativeInputHandlerOperation,
        callback: impl FnOnce(&mut dyn InputHandler, &mut Window, &mut App) -> R,
    ) -> std::result::Result<R, NativeInputInvariantViolation> {
        let Self { cx, handler, .. } = self;
        cx.update_native_input_handler(operation, |window, app| {
            window
                .with_input_transaction(app, |window, app| callback(handler.as_mut(), window, app))
        })
    }

    fn native_callback<R>(
        &mut self,
        operation: NativeInputHandlerOperation,
        callback: impl FnOnce(&mut dyn InputHandler, &mut Window, &mut App) -> R,
    ) -> R {
        self.update_in_input_transaction(operation, callback)
            .unwrap_or_else(|violation| std::panic::panic_any(violation))
    }

    pub fn selected_text_range(&mut self, ignore_disabled_input: bool) -> Option<UTF16Selection> {
        self.native_callback(
            NativeInputHandlerOperation::SelectedTextRange,
            |handler, window, cx| handler.selected_text_range(ignore_disabled_input, window, cx),
        )
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn marked_text_range(&mut self) -> Option<Range<usize>> {
        self.native_callback(
            NativeInputHandlerOperation::MarkedTextRange,
            |handler, window, cx| handler.marked_text_range(window, cx),
        )
    }

    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "windows"),
        allow(dead_code)
    )]
    pub fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
    ) -> Option<String> {
        self.native_callback(
            NativeInputHandlerOperation::TextForRange,
            |handler, window, cx| handler.text_for_range(range_utf16, adjusted, window, cx),
        )
    }

    pub fn replace_text_in_range(&mut self, replacement_range: Option<Range<usize>>, text: &str) {
        self.native_callback(
            NativeInputHandlerOperation::ReplaceTextInRange,
            |handler, window, cx| {
                handler.replace_text_in_range(replacement_range, text, window, cx);
            },
        )
    }

    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
    ) {
        self.native_callback(
            NativeInputHandlerOperation::ReplaceAndMarkTextInRange,
            |handler, window, cx| {
                handler.replace_and_mark_text_in_range(
                    range_utf16,
                    new_text,
                    new_selected_range,
                    window,
                    cx,
                )
            },
        )
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn unmark_text(&mut self) {
        self.native_callback(
            NativeInputHandlerOperation::UnmarkText,
            |handler, window, cx| handler.unmark_text(window, cx),
        )
    }

    pub fn bounds_for_range(&mut self, range_utf16: Range<usize>) -> Option<Bounds<Pixels>> {
        let transform = self.transform;
        self.native_callback(
            NativeInputHandlerOperation::BoundsForRange,
            |handler, window, cx| handler.bounds_for_range(range_utf16, window, cx),
        )
        .and_then(|bounds| transform.try_project_bounds(bounds).ok())
    }

    #[allow(dead_code)]
    pub fn apple_press_and_hold_enabled(&mut self) -> bool {
        self.native_callback(
            NativeInputHandlerOperation::ApplePressAndHoldEnabled,
            |handler, _, _| handler.apple_press_and_hold_enabled(),
        )
    }

    pub fn dispatch_input(&mut self, input: &str, window: &mut Window, cx: &mut App) {
        let handler = self.handler.as_mut();
        window.with_input_transaction(cx, |window, cx| {
            handler.replace_text_in_range(None, input, window, cx);
        });
    }

    pub fn compute_ime_candidate_bounds(
        marked_range: Option<Range<usize>>,
        selection: &UTF16Selection,
        mut bounds_for_range: impl FnMut(Range<usize>) -> Option<Bounds<Pixels>>,
    ) -> Option<Bounds<Pixels>> {
        if let Some(marked_range) = marked_range {
            // Default to the start of the marked (composing) range.
            let mut line_start = marked_range.start;

            // Walk backward from the caret looking for a line break. A change in
            // the Y coordinate means we crossed into the previous visual line, so
            // the line start is one position after the break point.
            let caret = selection.range.end;
            if let Some(caret_bounds) = bounds_for_range(caret..caret) {
                for i in (marked_range.start..caret).rev() {
                    if let Some(b) = bounds_for_range(i..i) {
                        if (b.origin.y - caret_bounds.origin.y).abs() > px(0.1) {
                            line_start = i + 1;
                            break;
                        }
                    }
                }
            }
            bounds_for_range(line_start..line_start)
        } else {
            // No active composition — use the selection endpoint.
            let offset = if selection.reversed {
                selection.range.start
            } else {
                selection.range.end
            };
            bounds_for_range(offset..offset)
        }
    }

    pub fn selected_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        let handler = self.handler.as_mut();
        let transform = self.transform;
        window.with_input_transaction(cx, |window, cx| {
            let marked_range = handler.marked_text_range(window, cx);
            let selection = handler.selected_text_range(true, window, cx)?;
            Self::compute_ime_candidate_bounds(marked_range, &selection, |range| {
                handler
                    .bounds_for_range(range, window, cx)
                    .and_then(|bounds| transform.try_project_bounds(bounds).ok())
            })
        })
    }

    pub fn ime_candidate_bounds(&mut self) -> Option<Bounds<Pixels>> {
        let transform = self.transform;
        self.native_callback(
            NativeInputHandlerOperation::ImeCandidateBounds,
            |handler, window, cx| {
                let marked_range = handler.marked_text_range(window, cx);
                let selection = handler.selected_text_range(true, window, cx)?;
                Self::compute_ime_candidate_bounds(marked_range, &selection, |range| {
                    handler
                        .bounds_for_range(range, window, cx)
                        .and_then(|bounds| transform.try_project_bounds(bounds).ok())
                })
            },
        )
    }

    #[allow(unused)]
    pub fn character_index_for_point(&mut self, point: Point<Pixels>) -> Option<usize> {
        let point = self.transform.try_inverse_project_point(point).ok();
        self.native_callback(
            NativeInputHandlerOperation::CharacterIndexForPoint,
            |handler, window, cx| {
                point.and_then(|point| handler.character_index_for_point(point, window, cx))
            },
        )
    }

    #[allow(dead_code)]
    pub fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        let handler = self.handler.as_mut();
        window.with_input_transaction(cx, |window, cx| handler.accepts_text_input(window, cx))
    }

    #[allow(dead_code)]
    pub fn query_accepts_text_input(&mut self) -> bool {
        self.native_callback(
            NativeInputHandlerOperation::AcceptsTextInput,
            |handler, window, cx| handler.accepts_text_input(window, cx),
        )
    }

    #[allow(dead_code)]
    pub fn query_prefers_ime_for_printable_keys(&mut self) -> bool {
        self.native_callback(
            NativeInputHandlerOperation::PrefersImeForPrintableKeys,
            |handler, window, cx| handler.prefers_ime_for_printable_keys(window, cx),
        )
    }
}

/// A struct representing a selection in a text buffer, in UTF16 characters.
/// This is different from a range because the head may be before the tail.
#[derive(Debug)]
pub struct UTF16Selection {
    /// The range of text in the document this selection corresponds to
    /// in UTF16 characters.
    pub range: Range<usize>,
    /// Whether the head of this selection is at the start (true), or end (false)
    /// of the range
    pub reversed: bool,
}

/// Open GPUI's interface for handling text input from the platform's IME system.
/// This is currently a 1:1 exposure of the NSTextInputClient API:
///
/// <https://developer.apple.com/documentation/appkit/nstextinputclient>
pub trait InputHandler: 'static {
    /// Get the range of the user's currently selected text, if any
    /// Corresponds to [selectedRange()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438242-selectedrange)
    ///
    /// Return value is in terms of UTF-16 characters, from 0 to the length of the document
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection>;

    /// Get the range of the currently marked text, if any
    /// Corresponds to [markedRange()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438250-markedrange)
    ///
    /// Return value is in terms of UTF-16 characters, from 0 to the length of the document
    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>>;

    /// Get the text for the given document range in UTF-16 characters
    /// Corresponds to [attributedSubstring(forProposedRange: actualRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438238-attributedsubstring)
    ///
    /// range_utf16 is in terms of UTF-16 characters
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String>;

    /// Replace the text in the given document range with the given text
    /// Corresponds to [insertText(_:replacementRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438258-inserttext)
    ///
    /// replacement_range is in terms of UTF-16 characters
    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    );

    /// Replace the text in the given document range with the given text,
    /// and mark the given text as part of an IME 'composing' state
    /// Corresponds to [setMarkedText(_:selectedRange:replacementRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438246-setmarkedtext)
    ///
    /// range_utf16 is in terms of UTF-16 characters
    /// new_selected_range is in terms of UTF-16 characters
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    );

    /// Remove the IME 'composing' state from the document
    /// Corresponds to [unmarkText()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438239-unmarktext)
    fn unmark_text(&mut self, window: &mut Window, cx: &mut App);

    /// Get the bounds of the given document range in untransformed window-layout coordinates.
    /// Corresponds to [firstRect(forCharacterRange:actualRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438240-firstrect)
    ///
    /// GPUI projects the result through the focused element's committed subtree transform before
    /// passing it to the platform for IME candidate-window positioning.
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>>;

    /// Get the character offset for an untransformed window-layout point in UTF-16 characters.
    ///
    /// Corresponds to [characterIndexForPoint:](https://developer.apple.com/documentation/appkit/nstextinputclient/characterindex(for:))
    /// GPUI inverse-projects platform window coordinates before calling this method.
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize>;

    /// Allows a given input context to opt into getting raw key repeats instead of
    /// sending these to the platform.
    /// TODO: Ideally we should be able to set ApplePressAndHoldEnabled in NSUserDefaults
    /// (which is how iTerm does it) but it doesn't seem to work for me.
    #[allow(dead_code)]
    fn apple_press_and_hold_enabled(&mut self) -> bool {
        true
    }

    /// Returns whether this handler is accepting text input to be inserted.
    fn accepts_text_input(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }

    /// Returns whether printable keys should be routed to the IME before keybinding
    /// matching when a non-ASCII input source (e.g. Japanese, Korean, Chinese IME)
    /// is active. This prevents multi-stroke keybindings like `jj` from intercepting
    /// keys that the IME should compose.
    ///
    /// Defaults to `false`. The editor overrides this based on whether it expects
    /// character input (e.g. Vim insert mode returns `true`, normal mode returns `false`).
    /// The terminal keeps the default `false` so that raw keys reach the terminal process.
    fn prefers_ime_for_printable_keys(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

/// An application-bound, generation-safe reference to a live top-level owner window.
///
/// This token does not keep the owner alive. GPUI validates the application identity and full
/// window generation again when opening the transient window.
#[derive(Clone)]
pub struct WindowTransientOwner {
    app: Weak<AppCell>,
    window: AnyWindowHandle,
}

impl WindowTransientOwner {
    pub(crate) fn new(app: Weak<AppCell>, window: AnyWindowHandle) -> Self {
        Self { app, window }
    }

    pub(crate) fn belongs_to(&self, app: &Weak<AppCell>) -> bool {
        Weak::ptr_eq(&self.app, app)
    }

    /// Returns the referenced owner handle.
    pub fn window(&self) -> AnyWindowHandle {
        self.window
    }
}

impl Debug for WindowTransientOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowTransientOwner")
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

/// The variables that can be configured when creating a new window
#[derive(Debug)]
pub struct WindowOptions {
    /// Specifies the state and bounds of the window in screen coordinates.
    /// - `None`: Inherit the bounds.
    /// - `Some(WindowBounds)`: Open a window with corresponding state and its restore size.
    pub window_bounds: Option<WindowBounds>,

    /// The titlebar configuration of the window
    pub titlebar: Option<TitlebarOptions>,

    /// Whether the window should request focus when it first appears.
    ///
    /// This is a one-shot creation policy and does not affect later activation.
    pub focus_on_appearing: bool,

    /// Lifetime activation and click-focus policy.
    pub activation_policy: WindowActivationPolicy,

    /// Whether the window should be shown when created
    pub show: bool,

    /// Private exact-generation provisional presentation and interaction authority.
    #[doc(hidden)]
    pub provisional_session: Option<WindowProvisionalSession>,

    /// Typed top-level owner relationship for native grouping and z-order behavior.
    ///
    /// Native ownership does not imply application lifecycle ownership.
    pub transient_for: Option<WindowTransientOwner>,

    /// The kind of window to create
    pub kind: WindowKind,

    /// Whether the window should be movable by the user
    pub is_movable: bool,

    /// Whether the window should be resizable by the user
    pub is_resizable: bool,

    /// Whether the window should be minimized by the user
    pub is_minimizable: bool,

    /// Whether the window should receive pointer input. When false and supported by the
    /// platform, the window is click-through and route resolution may target the window beneath it.
    pub accepts_pointer_input: bool,

    /// The display to create the window on, if this is None,
    /// the window will be created on the main display
    pub display_id: Option<DisplayId>,

    /// The appearance of the window background.
    pub window_background: WindowBackgroundAppearance,

    /// Application identifier of the window. Can by used by desktop environments to group applications together.
    pub app_id: Option<String>,

    /// Window minimum size
    pub window_min_size: Option<Size<Pixels>>,

    /// Whether to use client or server side decorations. Wayland only
    /// Note that this may be ignored.
    pub window_decorations: Option<WindowDecorations>,

    /// Icon image (X11 only)
    pub icon: Option<Arc<image::RgbaImage>>,

    /// Tab group name, allows opening the window as a native tab on macOS 10.12+. Windows with the same tabbing identifier will be grouped together.
    pub tabbing_identifier: Option<String>,
}

/// The variables that can be configured when creating a new window
#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[allow(missing_docs)]
pub struct WindowParams {
    /// The canonical creation placement, including windowed/maximized/fullscreen state and
    /// restore bounds.
    pub window_bounds: WindowBounds,
    /// The legacy geometry projection of [`Self::window_bounds`].
    pub bounds: Bounds<Pixels>,

    /// The titlebar configuration of the window
    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub titlebar: Option<TitlebarOptions>,

    /// The kind of window to create
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub kind: WindowKind,

    /// Whether the window should be movable by the user
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_movable: bool,

    /// Whether the window should be resizable by the user
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_resizable: bool,

    /// Whether the window should be minimized by the user
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_minimizable: bool,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub accepts_pointer_input: bool,

    pub focus_on_appearing: bool,

    pub activation_policy: WindowActivationPolicy,

    pub transient_for: Option<AnyWindowHandle>,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub show: bool,

    #[doc(hidden)]
    pub provisional_session: Option<WindowProvisionalSession>,

    /// An image to set as the window icon (x11 only)
    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub icon: Option<Arc<image::RgbaImage>>,

    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub display_id: Option<DisplayId>,

    pub window_min_size: Option<Size<Pixels>>,
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,
}

/// Represents the status of how a window should be opened.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowBounds {
    /// Indicates that the window should open in a windowed state with the given bounds.
    Windowed(Bounds<Pixels>),
    /// Indicates that the window should open in a maximized state.
    /// The bounds provided here represent the restore size of the window.
    Maximized(Bounds<Pixels>),
    /// Indicates that the window should open in fullscreen mode.
    /// The bounds provided here represent the restore size of the window.
    Fullscreen(Bounds<Pixels>),
}

impl Default for WindowBounds {
    fn default() -> Self {
        WindowBounds::Windowed(Bounds::default())
    }
}

impl WindowBounds {
    /// Retrieve the inner bounds
    pub fn get_bounds(&self) -> Bounds<Pixels> {
        match self {
            WindowBounds::Windowed(bounds) => *bounds,
            WindowBounds::Maximized(bounds) => *bounds,
            WindowBounds::Fullscreen(bounds) => *bounds,
        }
    }

    /// Creates a new window bounds that centers the window on the screen.
    pub fn centered(size: Size<Pixels>, cx: &App) -> Self {
        WindowBounds::Windowed(Bounds::centered(None, size, cx))
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            window_bounds: None,
            titlebar: Some(TitlebarOptions {
                title: Default::default(),
                appears_transparent: Default::default(),
                traffic_light_position: Default::default(),
            }),
            focus_on_appearing: true,
            activation_policy: WindowActivationPolicy::default(),
            show: true,
            provisional_session: None,
            transient_for: None,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            accepts_pointer_input: true,
            display_id: None,
            window_background: WindowBackgroundAppearance::default(),
            icon: None,
            app_id: None,
            window_min_size: None,
            window_decorations: None,
            tabbing_identifier: None,
        }
    }
}

/// The options that can be configured for a window's titlebar
#[derive(Debug, Default)]
pub struct TitlebarOptions {
    /// The initial title of the window
    pub title: Option<SharedString>,

    /// Should the default system titlebar be hidden to allow for a custom-drawn titlebar? (macOS and Windows only)
    /// Refer to [`WindowOptions::window_decorations`] on Linux
    pub appears_transparent: bool,

    /// The position of the macOS traffic light buttons
    pub traffic_light_position: Option<Point<Pixels>>,
}

/// The kind of window to create
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowKind {
    /// A normal application window
    Normal,

    /// A window that appears above all other windows, usually used for alerts or popups
    /// use sparingly!
    PopUp,

    /// A floating window that appears on top of its parent window
    Floating,

    /// A Wayland LayerShell window, used to draw overlays or backgrounds for applications such as
    /// docks, notifications or wallpapers.
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    LayerShell(layer_shell::LayerShellOptions),

    /// A window that appears on top of its parent window and blocks interaction with it
    /// until the modal window is closed
    Dialog,
}

impl WindowKind {
    /// Returns a stable diagnostic label for this window kind.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::PopUp => "pop-up",
            Self::Floating => "floating",
            #[cfg(all(target_os = "linux", feature = "wayland"))]
            Self::LayerShell(_) => "layer-shell",
            Self::Dialog => "dialog",
        }
    }
}

/// The appearance of the window, as defined by the operating system.
///
/// On macOS, this corresponds to named [`NSAppearance`](https://developer.apple.com/documentation/appkit/nsappearance)
/// values.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum WindowAppearance {
    /// A light appearance.
    ///
    /// On macOS, this corresponds to the `aqua` appearance.
    #[default]
    Light,

    /// A light appearance with vibrant colors.
    ///
    /// On macOS, this corresponds to the `NSAppearanceNameVibrantLight` appearance.
    VibrantLight,

    /// A dark appearance.
    ///
    /// On macOS, this corresponds to the `darkAqua` appearance.
    Dark,

    /// A dark appearance with vibrant colors.
    ///
    /// On macOS, this corresponds to the `NSAppearanceNameVibrantDark` appearance.
    VibrantDark,
}

/// The appearance of the background of the window itself, when there is
/// no content or the content is transparent.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum WindowBackgroundAppearance {
    /// Opaque.
    ///
    /// This lets the window manager know that content behind this
    /// window does not need to be drawn.
    ///
    /// Actual color depends on the system and themes should define a fully
    /// opaque background color instead.
    #[default]
    Opaque,
    /// Plain alpha transparency.
    Transparent,
    /// Transparency, but the contents behind the window are blurred.
    ///
    /// Not always supported.
    Blurred,
    /// The Mica backdrop material, supported on Windows 11.
    MicaBackdrop,
    /// The Mica Alt backdrop material, supported on Windows 11.
    MicaAltBackdrop,
}

/// The text rendering mode to use for drawing glyphs.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TextRenderingMode {
    /// Use the platform's default text rendering mode.
    #[default]
    PlatformDefault,
    /// Use subpixel (ClearType-style) text rendering.
    Subpixel,
    /// Use grayscale text rendering.
    Grayscale,
}

/// The options that can be configured for a file dialog prompt
#[derive(Clone, Debug)]
pub struct PathPromptOptions {
    /// Should the prompt allow files to be selected?
    pub files: bool,
    /// Should the prompt allow directories to be selected?
    pub directories: bool,
    /// Should the prompt allow multiple files to be selected?
    pub multiple: bool,
    /// The prompt to show to a user when selecting a path
    pub prompt: Option<SharedString>,
}

/// What kind of prompt styling to show
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PromptLevel {
    /// A prompt that is shown when the user should be notified of something
    Info,

    /// A prompt that is shown when the user needs to be warned of a potential problem
    Warning,

    /// A prompt that is shown when a critical problem has occurred
    Critical,
}

/// Prompt Button
#[derive(Clone, Debug, PartialEq)]
pub enum PromptButton {
    /// Ok button
    Ok(SharedString),
    /// Cancel button
    Cancel(SharedString),
    /// Other button
    Other(SharedString),
}

impl PromptButton {
    /// Create a button with label
    pub fn new(label: impl Into<SharedString>) -> Self {
        PromptButton::Other(label.into())
    }

    /// Create an Ok button
    pub fn ok(label: impl Into<SharedString>) -> Self {
        PromptButton::Ok(label.into())
    }

    /// Create a Cancel button
    pub fn cancel(label: impl Into<SharedString>) -> Self {
        PromptButton::Cancel(label.into())
    }

    /// Returns true if this button is a cancel button.
    #[allow(dead_code)]
    pub fn is_cancel(&self) -> bool {
        matches!(self, PromptButton::Cancel(_))
    }

    /// Returns the label of the button
    pub fn label(&self) -> &SharedString {
        match self {
            PromptButton::Ok(label) => label,
            PromptButton::Cancel(label) => label,
            PromptButton::Other(label) => label,
        }
    }
}

impl From<&str> for PromptButton {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "ok" => PromptButton::Ok("Ok".into()),
            "cancel" => PromptButton::Cancel("Cancel".into()),
            _ => PromptButton::Other(SharedString::from(value.to_owned())),
        }
    }
}

/// The style of the cursor (pointer)
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum CursorStyle {
    /// The default cursor
    #[default]
    Arrow,

    /// A text input cursor
    /// corresponds to the CSS cursor value `text`
    IBeam,

    /// A crosshair cursor
    /// corresponds to the CSS cursor value `crosshair`
    Crosshair,

    /// A closed hand cursor
    /// corresponds to the CSS cursor value `grabbing`
    ClosedHand,

    /// An open hand cursor
    /// corresponds to the CSS cursor value `grab`
    OpenHand,

    /// A pointing hand cursor
    /// corresponds to the CSS cursor value `pointer`
    PointingHand,

    /// A resize left cursor
    /// corresponds to the CSS cursor value `w-resize`
    ResizeLeft,

    /// A resize right cursor
    /// corresponds to the CSS cursor value `e-resize`
    ResizeRight,

    /// A resize cursor to the left and right
    /// corresponds to the CSS cursor value `ew-resize`
    ResizeLeftRight,

    /// A resize up cursor
    /// corresponds to the CSS cursor value `n-resize`
    ResizeUp,

    /// A resize down cursor
    /// corresponds to the CSS cursor value `s-resize`
    ResizeDown,

    /// A resize cursor directing up and down
    /// corresponds to the CSS cursor value `ns-resize`
    ResizeUpDown,

    /// A resize cursor directing up-left and down-right
    /// corresponds to the CSS cursor value `nesw-resize`
    ResizeUpLeftDownRight,

    /// A resize cursor directing up-right and down-left
    /// corresponds to the CSS cursor value `nwse-resize`
    ResizeUpRightDownLeft,

    /// A cursor indicating that the item/column can be resized horizontally.
    /// corresponds to the CSS cursor value `col-resize`
    ResizeColumn,

    /// A cursor indicating that the item/row can be resized vertically.
    /// corresponds to the CSS cursor value `row-resize`
    ResizeRow,

    /// A text input cursor for vertical layout
    /// corresponds to the CSS cursor value `vertical-text`
    IBeamCursorForVerticalLayout,

    /// A cursor indicating that the operation is not allowed
    /// corresponds to the CSS cursor value `not-allowed`
    OperationNotAllowed,

    /// A cursor indicating that the operation will result in a link
    /// corresponds to the CSS cursor value `alias`
    DragLink,

    /// A cursor indicating that the operation will result in a copy
    /// corresponds to the CSS cursor value `copy`
    DragCopy,

    /// A cursor indicating that the operation will result in a context menu
    /// corresponds to the CSS cursor value `context-menu`
    ContextualMenu,
}

/// A clipboard item that should be copied to the clipboard
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardItem {
    /// The entries in this clipboard item.
    pub entries: Vec<ClipboardEntry>,
}

/// Either a ClipboardString or a ClipboardImage
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardEntry {
    /// A string entry
    String(ClipboardString),
    /// An image entry
    Image(Image),
    /// A file entry
    ExternalPaths(crate::ExternalPaths),
}

impl ClipboardItem {
    /// Create a new ClipboardItem::String with no associated metadata
    pub fn new_string(text: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString::new(text))],
        }
    }

    /// Create a new ClipboardItem::String with the given text and associated metadata
    pub fn new_string_with_metadata(text: String, metadata: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString {
                text,
                metadata: Some(metadata),
            })],
        }
    }

    /// Create a new ClipboardItem::String with the given text and associated metadata
    pub fn new_string_with_json_metadata<T: Serialize>(text: String, metadata: T) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(
                ClipboardString::new(text).with_json_metadata(metadata),
            )],
        }
    }

    /// Create a new ClipboardItem::Image with the given image with no associated metadata
    pub fn new_image(image: &Image) -> Self {
        Self {
            entries: vec![ClipboardEntry::Image(image.clone())],
        }
    }

    /// Concatenates together all the ClipboardString entries in the item.
    /// Returns None if there were no ClipboardString entries.
    pub fn text(&self) -> Option<String> {
        let mut answer = String::new();

        for entry in self.entries.iter() {
            if let ClipboardEntry::String(ClipboardString { text, metadata: _ }) = entry {
                answer.push_str(text);
            }
        }

        if answer.is_empty() {
            for entry in self.entries.iter() {
                if let ClipboardEntry::ExternalPaths(paths) = entry {
                    for path in &paths.0 {
                        use std::fmt::Write as _;
                        _ = write!(answer, "{}", path.display());
                    }
                }
            }
        }

        if !answer.is_empty() {
            Some(answer)
        } else {
            None
        }
    }

    /// If this item is one ClipboardEntry::String, returns its metadata.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn metadata(&self) -> Option<&String> {
        match self.entries().first() {
            Some(ClipboardEntry::String(clipboard_string)) if self.entries.len() == 1 => {
                clipboard_string.metadata.as_ref()
            }
            _ => None,
        }
    }

    /// Get the item's entries
    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    /// Get owned versions of the item's entries
    pub fn into_entries(self) -> impl Iterator<Item = ClipboardEntry> {
        self.entries.into_iter()
    }
}

impl From<ClipboardString> for ClipboardEntry {
    fn from(value: ClipboardString) -> Self {
        Self::String(value)
    }
}

impl From<String> for ClipboardEntry {
    fn from(value: String) -> Self {
        Self::from(ClipboardString::from(value))
    }
}

impl From<Image> for ClipboardEntry {
    fn from(value: Image) -> Self {
        Self::Image(value)
    }
}

impl From<ClipboardEntry> for ClipboardItem {
    fn from(value: ClipboardEntry) -> Self {
        Self {
            entries: vec![value],
        }
    }
}

impl From<String> for ClipboardItem {
    fn from(value: String) -> Self {
        Self::from(ClipboardEntry::from(value))
    }
}

impl From<Image> for ClipboardItem {
    fn from(value: Image) -> Self {
        Self::from(ClipboardEntry::from(value))
    }
}

/// One of the editor's supported image formats (e.g. PNG, JPEG) - used when dealing with images in the clipboard
#[derive(Clone, Copy, Debug, Eq, PartialEq, EnumIter, Hash)]
pub enum ImageFormat {
    // Sorted from most to least likely to be pasted into an editor,
    // which matters when we iterate through them trying to see if
    // clipboard content matches them.
    /// .png
    Png,
    /// .jpeg or .jpg
    Jpeg,
    /// .webp
    Webp,
    /// .gif
    Gif,
    /// .svg
    Svg,
    /// .bmp
    Bmp,
    /// .tif or .tiff
    Tiff,
    /// .ico
    Ico,
    /// Netpbm image formats (.pbm, .ppm, .pgm).
    Pnm,
}

impl ImageFormat {
    /// Returns the mime type for the ImageFormat
    pub const fn mime_type(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Ico => "image/ico",
            ImageFormat::Pnm => "image/x-portable-anymap",
        }
    }

    /// Returns the ImageFormat for the given mime type, including known aliases.
    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        use strum::IntoEnumIterator;
        Self::iter()
            .find(|format| format.mime_type() == mime_type)
            .or_else(|| Self::from_mime_type_alias(mime_type))
    }

    /// Non-canonical mime types that some producers use in the wild.
    /// Unlike `mime_type()` which returns the single canonical form,
    /// these are legacy or shortened variants we still need to recognize.
    fn from_mime_type_alias(mime_type: &str) -> Option<Self> {
        match mime_type {
            "image/jpg" => Some(Self::Jpeg),
            "image/tif" => Some(Self::Tiff),
            _ => None,
        }
    }
}

/// An image, with a format and certain bytes
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// The image format the bytes represent (e.g. PNG)
    pub format: ImageFormat,
    /// The raw image bytes
    pub bytes: Vec<u8>,
    /// The unique ID for the image
    pub id: u64,
}

impl Hash for Image {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.id);
    }
}

impl Image {
    /// An empty image containing no data
    pub fn empty() -> Self {
        Self::from_bytes(ImageFormat::Png, Vec::new())
    }

    /// Create an image from a format and bytes
    pub fn from_bytes(format: ImageFormat, bytes: Vec<u8>) -> Self {
        Self {
            id: hash(&bytes),
            format,
            bytes,
        }
    }

    /// Get this image's ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Use the GPUI `use_asset` API to make this image renderable
    pub fn use_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .use_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    /// Use the GPUI `get_asset` API to make this image renderable
    pub fn get_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .get_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    /// Use the GPUI `remove_asset` API to drop this image, if possible.
    pub fn remove_asset(self: Arc<Self>, cx: &mut App) {
        ImageSource::Image(self).remove_asset(cx);
    }

    /// Convert the clipboard image to an `ImageData` object.
    pub fn to_image_data(&self, svg_renderer: SvgRenderer) -> Result<Arc<RenderImage>> {
        fn frames_for_image(
            bytes: &[u8],
            format: image::ImageFormat,
        ) -> Result<SmallVec<[Frame; 1]>> {
            let mut data = image::load_from_memory_with_format(bytes, format)?.into_rgba8();

            // Convert from RGBA to BGRA.
            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }

            Ok(SmallVec::from_elem(Frame::new(data), 1))
        }

        let frames = match self.format {
            ImageFormat::Gif => {
                let decoder = GifDecoder::new(Cursor::new(&self.bytes))?;
                let mut frames = SmallVec::new();

                for frame in decoder.into_frames() {
                    match frame {
                        Ok(mut frame) => {
                            // Convert from RGBA to BGRA.
                            for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                                pixel.swap(0, 2);
                            }
                            frames.push(frame);
                        }
                        Err(err) => {
                            log::debug!("Skipping GIF frame due to decode error: {err}");
                        }
                    }
                }

                if frames.is_empty() {
                    anyhow::bail!("GIF could not be decoded: all frames failed");
                }

                frames
            }
            ImageFormat::Png => frames_for_image(&self.bytes, image::ImageFormat::Png)?,
            ImageFormat::Jpeg => frames_for_image(&self.bytes, image::ImageFormat::Jpeg)?,
            ImageFormat::Webp => frames_for_image(&self.bytes, image::ImageFormat::WebP)?,
            ImageFormat::Bmp => frames_for_image(&self.bytes, image::ImageFormat::Bmp)?,
            ImageFormat::Tiff => frames_for_image(&self.bytes, image::ImageFormat::Tiff)?,
            ImageFormat::Ico => frames_for_image(&self.bytes, image::ImageFormat::Ico)?,
            ImageFormat::Svg => {
                return svg_renderer
                    .render_single_frame(&self.bytes, 1.0)
                    .map_err(Into::into);
            }
            ImageFormat::Pnm => frames_for_image(&self.bytes, image::ImageFormat::Pnm)?,
        };

        Ok(Arc::new(RenderImage::new(frames)))
    }

    /// Get the format of the clipboard image
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    /// Get the raw bytes of the clipboard image
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// A clipboard item that should be copied to the clipboard
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardString {
    /// The text content.
    pub text: String,
    /// Optional metadata associated with this clipboard string.
    pub metadata: Option<String>,
}

impl ClipboardString {
    /// Create a new clipboard string with the given text
    pub fn new(text: String) -> Self {
        Self {
            text,
            metadata: None,
        }
    }

    /// Return a new clipboard item with the metadata replaced by the given metadata,
    /// after serializing it as JSON.
    pub fn with_json_metadata<T: Serialize>(mut self, metadata: T) -> Self {
        self.metadata = Some(serde_json::to_string(&metadata).unwrap());
        self
    }

    /// Get the text of the clipboard string
    pub fn text(&self) -> &String {
        &self.text
    }

    /// Get the owned text of the clipboard string
    pub fn into_text(self) -> String {
        self.text
    }

    /// Get the metadata of the clipboard string, formatted as JSON
    pub fn metadata_json<T>(&self) -> Option<T>
    where
        T: for<'a> Deserialize<'a>,
    {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    /// Compute a hash of the given text for clipboard change detection.
    pub fn text_hash(text: &str) -> u64 {
        let mut hasher = SeaHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<String> for ClipboardString {
    fn from(value: String) -> Self {
        Self {
            text: value,
            metadata: None,
        }
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_svg_image_to_image_data_converts_to_bgra() {
        let image = Image::from_bytes(
            ImageFormat::Svg,
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">
<rect width="1" height="1" fill="#38BDF8"/>
</svg>"##
                .to_vec(),
        );

        let render_image = image.to_image_data(SvgRenderer::new(Arc::new(()))).unwrap();
        let bytes = render_image.as_bytes(0).unwrap();

        for pixel in bytes.chunks_exact(4) {
            assert_eq!(pixel, &[0xF8, 0xBD, 0x38, 0xFF]);
        }
    }
}

#[cfg(test)]
mod presentation_shutdown_tests {
    use super::*;

    #[test]
    fn terminal_before_quiescence_permanently_poison_shutdown_ticket() {
        let ticket = WindowPresentationShutdownTicket::new(WindowId::from(7), 1);

        assert!(!ticket.acknowledge_native_terminal());
        assert_eq!(
            ticket.snapshot(),
            WindowPresentationShutdownSnapshot {
                window_id: WindowId::from(7),
                generation: 1,
                quiesced: false,
                native_terminal: true,
                protocol_violation: true,
            }
        );

        assert!(!ticket.acknowledge_quiesced());
        assert!(!ticket.acknowledge_native_terminal());
        assert!(!ticket.snapshot().quiesced());
        assert!(ticket.snapshot().protocol_violation());
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "freebsd")))]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_window_button_layout_parse_standard() {
        let layout = WindowButtonLayout::parse("close,minimize:maximize").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_right_only() {
        let layout = WindowButtonLayout::parse("minimize,maximize,close").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close)
            ]
        );
    }

    #[test]
    fn test_window_button_layout_parse_left_only() {
        let layout = WindowButtonLayout::parse("close,minimize,maximize:").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize)
            ]
        );
        assert_eq!(layout.right, [None, None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_with_whitespace() {
        let layout = WindowButtonLayout::parse(" close , minimize : maximize ").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_empty() {
        let layout = WindowButtonLayout::parse("").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(layout.right, [None, None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_intentionally_empty() {
        let layout = WindowButtonLayout::parse(":").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(layout.right, [None, None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_invalid_buttons() {
        let layout = WindowButtonLayout::parse("close,invalid,minimize:maximize,foo").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_deduplicates_same_side_buttons() {
        let layout = WindowButtonLayout::parse("close,close,minimize").unwrap();
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.format(), ":close,minimize");
    }

    #[test]
    fn test_window_button_layout_parse_deduplicates_buttons_across_sides() {
        let layout = WindowButtonLayout::parse("close:maximize,close,minimize").unwrap();
        assert_eq!(layout.left, [Some(WindowButton::Close), None, None]);
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Maximize),
                Some(WindowButton::Minimize),
                None
            ]
        );

        let button_ids: Vec<_> = layout
            .left
            .iter()
            .chain(layout.right.iter())
            .flatten()
            .map(WindowButton::id)
            .collect();
        let unique_button_ids = button_ids.iter().copied().collect::<HashSet<_>>();
        assert_eq!(unique_button_ids.len(), button_ids.len());
        assert_eq!(layout.format(), "close:maximize,minimize");
    }

    #[test]
    fn test_window_button_layout_parse_gnome_style() {
        let layout = WindowButtonLayout::parse("close").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(layout.right, [Some(WindowButton::Close), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_elementary_style() {
        let layout = WindowButtonLayout::parse("close:maximize").unwrap();
        assert_eq!(layout.left, [Some(WindowButton::Close), None, None]);
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_round_trip() {
        let cases = [
            "close:minimize,maximize",
            "minimize,maximize,close:",
            ":close",
            "close:",
            "close:maximize",
            ":",
        ];

        for case in cases {
            let layout = WindowButtonLayout::parse(case).unwrap();
            assert_eq!(layout.format(), case, "Round-trip failed for: {}", case);
        }
    }

    #[test]
    fn test_window_button_layout_linux_default() {
        let layout = WindowButtonLayout::linux_default();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close)
            ]
        );

        let round_tripped = WindowButtonLayout::parse(&layout.format()).unwrap();
        assert_eq!(round_tripped, layout);
    }

    #[test]
    fn test_window_button_layout_parse_all_invalid() {
        assert!(WindowButtonLayout::parse("asdfghjkl").is_err());
    }
}
