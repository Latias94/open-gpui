#[cfg(any(feature = "inspector", debug_assertions))]
use crate::Inspector;
#[cfg(not(target_family = "wasm"))]
use crate::MouseButton;
#[cfg(target_os = "macos")]
use crate::PlatformPixelBuffer;
use crate::app::{AppCell, PlatformWindowCommandSink};
use crate::{
    Action, AnyElement, AnyEntity, AnyImageCache, AnyTooltip, AnyView, App, AppContext, Arena,
    Asset, AsyncWindowContext, AtlasAccessDiagnostic, AtlasRemoveDiagnostic,
    AtlasTextureInstanceId, AtlasTextureLease, AtlasTextureLeaseError, AvailableSpace, Background,
    BorderStyle, Bounds, BoxShadow, Capslock, Context, Corners, CursorHideMode, CursorStyle,
    Decorations, DevicePixels, DispatchActionListener, DispatchNodeId, DispatchTree, DisplayId,
    Edges, ElementGeometry, Entity, EntityId, EventEmitter, FontId, Global, GlobalElementId,
    GlyphId, GpuSpecs, Hsla, InputHandler, IsZero, KeyBinding, KeyContext, KeyDownEvent, KeyEvent,
    KeyUpEvent, Keystroke, KeystrokeEvent, LayoutId, Modifiers, ModifiersChangedEvent,
    MonochromeSprite, MouseEvent, MouseMoveEvent, MouseUpEvent, Path, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow,
    PlatformWindowCapabilities, PlatformWindowCommand, PlatformWindowDispatch,
    PlatformWindowMutationObservation, PlatformWindowPresentOutcome, PlatformWindowProfile, Point,
    PointerCancelEvent, PointerCancelReason, PolychromeSprite,
    PreparedPlatformPresentationShutdown, Primitive, PrimitiveTransform, Priority, PromptButton,
    PromptLevel, Quad, Render, RenderGlyphParams, RenderImage, RenderImageParams, RenderSvgParams,
    Replay, RequestFrameOptions, ResizeEdge, SMOOTH_SVG_SCALE_FACTOR, SUBPIXEL_VARIANTS_X,
    SUBPIXEL_VARIANTS_Y, ScaledPixels, Shadow, SharedString, Size, StrikethroughStyle, Style,
    SubpixelSprite, SubscriberSet, Subscription, SubtreeClip, SubtreeClipError,
    SubtreePresentation, SubtreeTransform, SubtreeTransformError, SystemWindowTab,
    SystemWindowTabController, TaffyLayoutEngine, Task, TextRenderingMode, TextStyle,
    TextStyleRefinement, Underline, UnderlineStyle, WindowActivationPolicy, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControls, WindowCreationFacts,
    WindowDecorations, WindowInitialPresentationOrder, WindowInitialPresentationStatus, WindowKind,
    WindowMutationDispatch, WindowMutationDomain, WindowMutationOutcome, WindowOptions,
    WindowParams, WindowPlacementRequest, WindowPlacementState, WindowPlatformFacts,
    WindowPresentAttemptFacts, WindowPresentationFacts, WindowProvisionalOpeningClaim,
    WindowProvisionalRevealCancellationOutcome, WindowProvisionalRevealOutcome,
    WindowProvisionalRevealTicket, WindowProvisionalSemanticsOutcome,
    WindowProvisionalSemanticsSnapshot, WindowProvisionalSemanticsTicket, WindowProvisionalSession,
    WindowProvisionalSessionPhase, WindowTextSystem,
    geometry::{
        ClipStackSnapshot, ResolvedClip, ResolvedSubtreeTransform, SubtreeGeometryError,
        SubtreeGeometryValidity,
    },
    point,
    prelude::*,
    profiler, px, rems, size, transparent_black,
};
use anyhow::{Context as _, Result, anyhow};
use derive_more::{Deref, DerefMut};
use futures::FutureExt;
use futures::channel::oneshot;
#[cfg(feature = "input-latency-histogram")]
use hdrhistogram::Histogram;
use open_gpui_collections::{FxHashSet, TypeIdHashMap};
use open_gpui_core_util::post_inc;
use open_gpui_core_util::{ResultExt, measure};
use open_gpui_refineable::Refineable;
use open_gpui_scheduler::Instant;
use parking_lot::RwLock;
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use slotmap::SlotMap;
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::{Cell, RefCell},
    cmp,
    collections::VecDeque,
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    ops::{Deref as StdDeref, DerefMut as StdDerefMut, Range},
    rc::{Rc, Weak as RcWeak},
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, AtomicUsize, Ordering::SeqCst},
    },
    time::Duration,
};
use uuid::Uuid;

pub(crate) mod a11y;
mod bring_into_view;
mod frame_journal;
mod frame_pump;
mod input_dispatch;
mod invalidator;
mod pointer_session;
mod portal_anchor;
mod prompts;
#[doc(hidden)]
pub mod retained_visual;

use self::a11y::A11y;
pub use self::a11y::{
    AccessibilityAnnouncement, AccessibilityAnnouncementClearReason,
    AccessibilityAnnouncementDiagnostic, AccessibilityAnnouncementDropReason,
    AccessibilityAnnouncementLifecycle, AccessibilityAnnouncementOutcome,
    AccessibilityAnnouncementPoliteness, AccessibilityAnnouncementRequestId,
    AccessibilityAnnouncementSequence, AccessibilityTreeScope,
};
pub use self::bring_into_view::ScrollDirectMutationRevision;
use self::bring_into_view::{
    ActiveBringIntoViewRequest, BringIntoViewResolution, RevealTargetCapture, RevealTargetId,
    ScrollContainerBinding,
};
pub use self::bring_into_view::{
    BringIntoViewAlignment, BringIntoViewAxis, BringIntoViewBehavior, BringIntoViewCancelReason,
    BringIntoViewChainGeneration, BringIntoViewCompletion, BringIntoViewError,
    BringIntoViewMargins, BringIntoViewMarginsError, BringIntoViewOptions, BringIntoViewOutcome,
    BringIntoViewRequestId, DeferredBringIntoViewGuard, RevealTargetError, RevealTargetHandle,
    ScrollChainFence,
};
pub(crate) use self::frame_journal::{
    DeferredDraw, Frame, FrameOutput, PaintIndex, PrepaintCommit, PrepaintCommitCallback,
    PrepaintCommitPhase, PrepaintStateIndex, TooltipRequest, VisualPaintIndex, VisualPrepaintIndex,
};
use self::frame_pump::{FrameThrottleFacts, PresentFacts, frame_should_wait};
pub(crate) use self::invalidator::WindowInvalidator;
use self::pointer_session::{InputDispatchGuard, MouseEventTargetGuard, PressedMouseButtons};
pub use self::pointer_session::{
    PointerCapture, PointerCaptureError, PointerCaptureHandle, PointerCaptureId,
};
use self::portal_anchor::{PortalAnchorCapture, PortalAnchorId};
pub use self::portal_anchor::{PortalAnchorError, PortalAnchorHandle, PortalAnchorSnapshot};
use crate::util::{
    atomic_incr_if_not_zero, ceil_to_device_pixel, floor_to_device_pixel, round_half_toward_zero,
    round_half_toward_zero_f64, round_stroke_to_device_pixel, round_to_device_pixel,
};
use crate::window_platform_mutation::{
    WindowMutationRequest, WindowMutationState, WindowMutationTicketDelivery,
    WindowPlatformMutationAuthority, placement_request_is_valid, placement_state_from_facts,
    platform_dispatch_outcome,
};
pub use prompts::*;

/// Default window size used when no explicit size is provided.
pub const DEFAULT_WINDOW_SIZE: Size<Pixels> = size(px(1536.), px(1095.));

/// A 6:5 aspect ratio minimum window size for secondary functional windows,
/// like settings and rule-library windows.
pub const DEFAULT_ADDITIONAL_WINDOW_SIZE: Size<Pixels> = Size {
    width: Pixels(900.),
    height: Pixels(750.),
};

/// Represents the two different phases when dispatching events.
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub enum DispatchPhase {
    /// After the capture phase comes the bubble phase, in which mouse event listeners are
    /// invoked front to back and keyboard event listeners are invoked from the focused element
    /// to the root of the element tree. This is the phase you'll most commonly want to use when
    /// registering event listeners.
    #[default]
    Bubble,
    /// During the initial capture phase, mouse event listeners are invoked back to front, and keyboard
    /// listeners are invoked from the root of the tree downward toward the focused element. This phase
    /// is used for special purposes such as clearing the "pressed" state for click events. If
    /// you stop event propagation during this phase, you need to know what you're doing. Handlers
    /// outside of the immediate region may rely on detecting non-local events during this phase.
    Capture,
}

impl DispatchPhase {
    /// Returns true if this represents the "bubble" phase.
    #[inline]
    pub fn bubble(self) -> bool {
        self == DispatchPhase::Bubble
    }

    /// Returns true if this represents the "capture" phase.
    #[inline]
    pub fn capture(self) -> bool {
        self == DispatchPhase::Capture
    }
}

type AnyObserver = Box<dyn FnMut(&mut Window, &mut App) -> bool + 'static>;

type AnyWindowKeyDownInterceptor =
    Box<dyn FnMut(&KeyDownEvent, &mut Window, &mut App) -> bool + 'static>;

type AnyWindowMouseInterceptor =
    Box<dyn for<'a> FnMut(WindowMouseEvent<'a>, &mut Window, &mut App) -> bool + 'static>;

/// A typed view of a mouse or pointer event before it reaches frame-scoped listeners.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum WindowMouseEvent<'a> {
    /// A mouse button was pressed.
    Down(&'a crate::MouseDownEvent),
    /// A mouse button was released.
    Up(&'a MouseUpEvent),
    /// The pointer moved within the window.
    Move(&'a MouseMoveEvent),
    /// The pointer left the window.
    Exit(&'a crate::MouseExitEvent),
    /// The active pointer gesture was canceled without a matching mouse-up event.
    Cancel(&'a PointerCancelEvent),
    /// Mouse pressure changed.
    Pressure(&'a crate::MousePressureEvent),
    /// The scroll wheel was used.
    Scroll(&'a crate::ScrollWheelEvent),
    /// A pinch gesture was performed.
    Pinch(&'a crate::PinchEvent),
    /// A platform file drag event occurred.
    FileDrop(&'a crate::FileDropEvent),
}

impl<'a> WindowMouseEvent<'a> {
    fn from_any(event: &'a dyn Any) -> Option<Self> {
        if let Some(event) = event.downcast_ref() {
            Some(Self::Down(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Up(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Move(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Exit(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Cancel(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Pressure(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Scroll(event))
        } else if let Some(event) = event.downcast_ref() {
            Some(Self::Pinch(event))
        } else {
            event.downcast_ref().map(Self::FileDrop)
        }
    }
}

pub(crate) type AnyWindowFocusListener =
    Box<dyn FnMut(&WindowFocusEvent, &mut Window, &mut App) -> bool + 'static>;

pub(crate) struct WindowFocusEvent {
    pub(crate) previous_focus_path: SmallVec<[FocusId; 8]>,
    pub(crate) current_focus_path: SmallVec<[FocusId; 8]>,
    pub(crate) previous_committed_focus_path: SmallVec<[FocusId; 8]>,
    pub(crate) current_committed_focus_path: SmallVec<[FocusId; 8]>,
}

impl WindowFocusEvent {
    pub fn is_focus_in(&self, focus_id: FocusId) -> bool {
        !self.previous_focus_path.contains(&focus_id) && self.current_focus_path.contains(&focus_id)
    }

    pub fn is_focus_out(&self, focus_id: FocusId) -> bool {
        self.previous_focus_path.contains(&focus_id) && !self.current_focus_path.contains(&focus_id)
    }

    pub fn is_focus_committed(&self, focus_id: FocusId) -> bool {
        self.previous_committed_focus_path.last() != Some(&focus_id)
            && self.current_committed_focus_path.last() == Some(&focus_id)
    }

    pub fn is_focus_committed_in(&self, focus_id: FocusId) -> bool {
        !self.previous_committed_focus_path.contains(&focus_id)
            && self.current_committed_focus_path.contains(&focus_id)
    }
}

/// This is provided when subscribing for `Context::on_focus_out` events.
pub struct FocusOutEvent {
    /// A weak focus handle representing what was blurred.
    pub blurred: WeakFocusHandle,
}

/// The terminal result of a focus authority request observed through
/// [`Window::focus_with_completion`] or [`Window::blur_with_completion`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusClaimOutcome {
    /// The requested exact or empty focus authority committed.
    Committed,
    /// The requested focus authority did not qualify in its candidate render generation.
    Rejected,
    /// A later focus mutation replaced the request before it committed.
    Superseded,
}

slotmap::new_key_type! {
    /// A globally unique identifier for a focusable element.
    pub struct FocusId;
}

thread_local! {
    /// Fallback arena used when no app-specific arena is active.
    /// In production, each window draw sets CURRENT_ELEMENT_ARENA to the app's arena.
    pub(crate) static ELEMENT_ARENA: RefCell<Arena> = RefCell::new(Arena::new(1024 * 1024));

    /// Points to the current App's element arena during draw operations.
    /// This allows multiple test Apps to have isolated arenas, preventing
    /// cross-session corruption when the scheduler interleaves their tasks.
    static CURRENT_ELEMENT_ARENA: Cell<Option<*const RefCell<Arena>>> = const { Cell::new(None) };
}

/// Allocates an element in the current arena. Uses the app-specific arena if one
/// is active (during draw), otherwise falls back to the thread-local ELEMENT_ARENA.
pub(crate) fn with_element_arena<R>(f: impl FnOnce(&mut Arena) -> R) -> R {
    CURRENT_ELEMENT_ARENA.with(|current| {
        if let Some(arena_ptr) = current.get() {
            // SAFETY: The pointer is valid for the duration of the draw operation
            // that set it, and we're being called during that same draw.
            let arena_cell = unsafe { &*arena_ptr };
            f(&mut arena_cell.borrow_mut())
        } else {
            ELEMENT_ARENA.with_borrow_mut(f)
        }
    })
}

/// RAII guard that sets CURRENT_ELEMENT_ARENA for the duration of a draw operation.
/// When dropped, restores the previous arena (supporting nested draws).
pub(crate) struct ElementArenaScope {
    previous: Option<*const RefCell<Arena>>,
}

impl ElementArenaScope {
    /// Enter a scope where element allocations use the given arena.
    pub(crate) fn enter(arena: &RefCell<Arena>) -> Self {
        let previous = CURRENT_ELEMENT_ARENA.with(|current| {
            let prev = current.get();
            current.set(Some(arena as *const RefCell<Arena>));
            prev
        });
        Self { previous }
    }
}

impl Drop for ElementArenaScope {
    fn drop(&mut self) {
        CURRENT_ELEMENT_ARENA.with(|current| {
            current.set(self.previous);
        });
    }
}

/// Returned when the element arena has been used and so must be cleared before the next draw.
#[must_use]
pub struct ArenaClearNeeded {
    arena: *const RefCell<Arena>,
}

impl ArenaClearNeeded {
    /// Create a new ArenaClearNeeded that will clear the given arena.
    pub(crate) fn new(arena: &RefCell<Arena>) -> Self {
        Self {
            arena: arena as *const RefCell<Arena>,
        }
    }

    /// Clear the element arena.
    pub fn clear(self) {
        // SAFETY: The arena pointer is valid because ArenaClearNeeded is created
        // at the end of draw() and must be cleared before the next draw.
        let arena_cell = unsafe { &*self.arena };
        arena_cell.borrow_mut().clear();
    }
}

pub(crate) type FocusMap = RwLock<SlotMap<FocusId, FocusRef>>;
pub(crate) struct FocusRef {
    pub(crate) ref_count: AtomicUsize,
    pub(crate) tab_index: isize,
    pub(crate) tab_stop: bool,
}

impl FocusId {
    /// Obtains whether the element associated with this handle is currently focused.
    pub fn is_focused(&self, window: &Window) -> bool {
        window.focus == Some(*self)
    }

    /// Obtains whether the element associated with this handle contains the focused
    /// element or is itself focused.
    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        window
            .focused(cx)
            .is_some_and(|focused| self.contains(focused.id, window))
    }

    /// Obtains whether the element associated with this handle is contained within the
    /// focused element or is itself focused.
    pub fn within_focused(&self, window: &Window, cx: &App) -> bool {
        let focused = window.focused(cx);
        focused.is_some_and(|focused| focused.id.contains(*self, window))
    }

    /// Obtains whether this handle contains the given handle in the most recently rendered frame.
    pub(crate) fn contains(&self, other: Self, window: &Window) -> bool {
        window
            .rendered_frame
            .dispatch_tree
            .focus_contains(*self, other)
    }
}

/// A handle which can be used to track and manipulate the focused element in a window.
pub struct FocusHandle {
    pub(crate) id: FocusId,
    handles: Arc<FocusMap>,
    /// The index of this element in the tab order.
    pub tab_index: isize,
    /// Whether this element can be focused by tab navigation.
    pub tab_stop: bool,
}

impl std::fmt::Debug for FocusHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("FocusHandle({:?})", self.id))
    }
}

impl FocusHandle {
    pub(crate) fn new(handles: &Arc<FocusMap>) -> Self {
        let id = handles.write().insert(FocusRef {
            ref_count: AtomicUsize::new(1),
            tab_index: 0,
            tab_stop: false,
        });

        Self {
            id,
            tab_index: 0,
            tab_stop: false,
            handles: handles.clone(),
        }
    }

    pub(crate) fn for_id(id: FocusId, handles: &Arc<FocusMap>) -> Option<Self> {
        let lock = handles.read();
        let focus = lock.get(id)?;
        if atomic_incr_if_not_zero(&focus.ref_count) == 0 {
            return None;
        }
        Some(Self {
            id,
            tab_index: focus.tab_index,
            tab_stop: focus.tab_stop,
            handles: handles.clone(),
        })
    }

    /// Sets the tab index of the element associated with this handle.
    pub fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = index;
        if let Some(focus) = self.handles.write().get_mut(self.id) {
            focus.tab_index = index;
        }
        self
    }

    /// Sets whether the element associated with this handle is a tab stop.
    ///
    /// When `false`, the element will not be included in the tab order.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        if let Some(focus) = self.handles.write().get_mut(self.id) {
            focus.tab_stop = tab_stop;
        }
        self
    }

    /// Converts this focus handle into a weak variant, which does not prevent it from being released.
    pub fn downgrade(&self) -> WeakFocusHandle {
        WeakFocusHandle {
            id: self.id,
            handles: Arc::downgrade(&self.handles),
        }
    }

    /// Moves the focus to the element associated with this handle.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(self, cx)
    }

    /// Obtains whether the element associated with this handle is currently focused.
    pub fn is_focused(&self, window: &Window) -> bool {
        self.id.is_focused(window)
    }

    /// Obtains whether the element associated with this handle contains the focused
    /// element or is itself focused.
    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        self.id.contains_focused(window, cx)
    }

    /// Obtains whether the element associated with this handle is contained within the
    /// focused element or is itself focused.
    pub fn within_focused(&self, window: &Window, cx: &mut App) -> bool {
        self.id.within_focused(window, cx)
    }

    /// Obtains whether this handle contains the given handle in the most recently rendered frame.
    pub fn contains(&self, other: &Self, window: &Window) -> bool {
        self.id.contains(other.id, window)
    }

    /// Dispatch an action on the element that rendered this focus handle
    pub fn dispatch_action(&self, action: &dyn Action, window: &mut Window, cx: &mut App) {
        if let Some(node_id) = window
            .rendered_frame
            .dispatch_tree
            .focusable_node_id(self.id)
        {
            window.dispatch_action_on_node(node_id, action, cx)
        }
    }
}

impl Clone for FocusHandle {
    fn clone(&self) -> Self {
        Self::for_id(self.id, &self.handles).unwrap()
    }
}

impl PartialEq for FocusHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for FocusHandle {}

impl Drop for FocusHandle {
    fn drop(&mut self) {
        self.handles
            .read()
            .get(self.id)
            .unwrap()
            .ref_count
            .fetch_sub(1, SeqCst);
    }
}

/// A weak reference to a focus handle.
#[derive(Clone, Debug)]
pub struct WeakFocusHandle {
    pub(crate) id: FocusId,
    pub(crate) handles: Weak<FocusMap>,
}

impl WeakFocusHandle {
    /// Attempts to upgrade the [WeakFocusHandle] to a [FocusHandle].
    pub fn upgrade(&self) -> Option<FocusHandle> {
        let handles = self.handles.upgrade()?;
        FocusHandle::for_id(self.id, &handles)
    }
}

impl PartialEq for WeakFocusHandle {
    fn eq(&self, other: &WeakFocusHandle) -> bool {
        self.id == other.id
    }
}

impl Eq for WeakFocusHandle {}

impl PartialEq<FocusHandle> for WeakFocusHandle {
    fn eq(&self, other: &FocusHandle) -> bool {
        self.id == other.id
    }
}

impl PartialEq<WeakFocusHandle> for FocusHandle {
    fn eq(&self, other: &WeakFocusHandle) -> bool {
        self.id == other.id
    }
}

/// Focusable allows users of your view to easily
/// focus it (using window.focus_view(cx, view))
pub trait Focusable: 'static {
    /// Returns the focus handle associated with this view.
    fn focus_handle(&self, cx: &App) -> FocusHandle;
}

impl<V: Focusable> Focusable for Entity<V> {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }
}

/// ManagedView is a view (like a Modal, Popover, Menu, etc.)
/// where the lifecycle of the view is handled by another view.
pub trait ManagedView: Focusable + EventEmitter<DismissEvent> + Render {}

impl<M: Focusable + EventEmitter<DismissEvent> + Render> ManagedView for M {}

/// Emitted by implementers of [`ManagedView`] to indicate the view should be dismissed, such as when a view is presented as a modal.
pub struct DismissEvent;

type FrameCallback = Box<dyn FnOnce(&mut Window, &mut App)>;

pub(crate) type AnyMouseListener =
    Box<dyn FnMut(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static>;

pub(crate) type AnyPointerCancelListener = Rc<
    RefCell<Box<dyn FnMut(&PointerCancelEvent, DispatchPhase, &mut Window, &mut App) + 'static>>,
>;

struct PendingPointerCancellation {
    event: PointerCancelEvent,
    target: Option<HitboxId>,
    listeners: Vec<FrameOutput<Option<AnyPointerCancelListener>>>,
    native_release: Option<crate::NativePointerCaptureReleaseToken>,
}

#[derive(Clone)]
pub(crate) struct CursorStyleRequest {
    pub(crate) hitbox_id: Option<HitboxId>,
    pub(crate) style: CursorStyle,
    validity: Option<SubtreeGeometryValidity>,
}

#[derive(Default, Eq, PartialEq)]
pub(crate) struct HitTest {
    pub(crate) ids: SmallVec<[HitboxId; 8]>,
    pub(crate) hover_hitbox_count: usize,
}

/// A type of window control area that corresponds to the platform window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowControlArea {
    /// An area that allows dragging of the platform window.
    Drag,
    /// An area that allows closing of the platform window.
    Close,
    /// An area that allows maximizing of the platform window.
    Max,
    /// An area that allows minimizing of the platform window.
    Min,
}

/// An identifier for a [Hitbox] which also includes [HitboxBehavior].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct HitboxId(u64);

static NEXT_PREPAINT_PUBLICATION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_PRESENTATION_SHUTDOWN_GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_presentation_shutdown_ticket(
    window_id: WindowId,
) -> crate::WindowPresentationShutdownTicket {
    let generation = NEXT_PRESENTATION_SHUTDOWN_GENERATION.fetch_add(1, SeqCst);
    assert_ne!(
        generation, 0,
        "presentation-shutdown generation space exhausted"
    );
    crate::WindowPresentationShutdownTicket::new(window_id, generation)
}

/// A stable identity for one cross-frame publication produced during prepaint.
///
/// Reuse the same ID for one logical publication on every frame. GPUI commits or discards only
/// after accepting the candidate frame and passes the callback an [`AcceptedFrameFence`]. It also
/// invokes the previous frame's discard callback when the publication is absent from the next
/// accepted frame. The absence rule retracts state when a subtree is removed, skipped by an
/// invalid ancestor transform, or rolled back by [`Window::transact`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PrepaintPublicationId(u64);

impl PrepaintPublicationId {
    /// Allocates a process-unique publication identity.
    pub fn new() -> Self {
        let id = NEXT_PREPAINT_PUBLICATION_ID.fetch_add(1, SeqCst);
        assert_ne!(id, u64::MAX, "prepaint publication ID space exhausted");
        Self(id)
    }
}

impl Default for PrepaintPublicationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Proof that one candidate frame has become the rendered frame for a specific window.
///
/// GPUI creates this value only while committing or discarding a
/// [`Window::record_prepaint_window_transaction`] publication after the candidate frame swap.
/// Consumers may use it to distinguish work that is already backed by an accepted frame from
/// ordinary event-driven work that must wait for a future frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AcceptedFrameFence {
    window_id: WindowId,
    generation: u64,
}

impl AcceptedFrameFence {
    fn new(window_id: WindowId, generation: u64) -> Self {
        Self {
            window_id,
            generation,
        }
    }

    /// Returns the window that accepted this frame.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the accepted rendered-frame generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns whether this fence is satisfied by the window's current rendered frame.
    pub fn is_satisfied_by(self, window: &Window) -> bool {
        self.window_id == window.handle.window_id()
            && window.rendered_frame_revision() >= self.generation
    }
}

#[cfg(any(test, feature = "test-support"))]
impl HitboxId {
    /// A placeholder HitboxId exclusively for integration testing API's that
    /// need a hitbox but where the value of the hitbox does not matter. The
    /// alternative is to make the Hitbox optional but that complicates the
    /// implementation.
    pub const fn placeholder() -> Self {
        Self(0)
    }
}

impl HitboxId {
    /// Returns this hitbox's front-to-back target rank at a displayed window point.
    ///
    /// The query uses the complete committed-frame hit-test order. `Some(0)` is the frontmost
    /// target, while larger ranks remain in the foreground target set. `None` means the hitbox is
    /// absent from the committed frame, does not contain the point after transforms and clipping,
    /// is inactive, or is behind [`HitboxBehavior::BlockMouse`] or
    /// [`HitboxBehavior::BlockMouseExceptScroll`]. Pointer capture and the current input modality
    /// do not affect this physical point query.
    pub fn window_point_target_rank(self, point: Point<Pixels>, window: &Window) -> Option<usize> {
        let hit_test = window.rendered_frame.hit_test(point);
        hit_test
            .ids
            .iter()
            .take(hit_test.hover_hitbox_count)
            .position(|id| *id == self)
    }

    /// Checks if the hitbox with this ID is physically hovered. Returns `false` during keyboard
    /// input modality so that keyboard navigation suppresses hover highlights. Use this for visual
    /// hover, cursors, tooltips, and drag-over state; mouse-button handlers should use
    /// [`Self::is_mouse_event_target`] so pointer capture is respected.
    ///
    /// See [`Hitbox::is_hovered`] for details.
    pub fn is_hovered(self, window: &Window) -> bool {
        if window.last_input_was_keyboard() {
            return false;
        }
        self.hit_test(window)
    }

    /// Checks whether this hitbox is the routed target for the current mouse-button event.
    ///
    /// Pointer capture makes its bound hitbox the exclusive event target without changing
    /// physical hover, cursor, tooltip, or drag-over state.
    pub fn is_mouse_event_target(self, window: &Window) -> bool {
        window
            .mouse_event_target
            .get()
            .map_or_else(|| self.hit_test(window), |target| target == self)
    }

    /// Checks if the hitbox with this ID is currently hovered, regardless of the last
    /// input modality used.
    ///
    /// See [`HitboxId::is_hovered`] for more details.
    pub(crate) fn is_hovered_ignoring_last_input(self, window: &Window) -> bool {
        self.hit_test(window)
    }

    fn hit_test(self, window: &Window) -> bool {
        let hit_test = &window.mouse_hit_test;
        for id in hit_test.ids.iter().take(hit_test.hover_hitbox_count) {
            if self == *id {
                return true;
            }
        }
        false
    }

    /// Checks if the hitbox with this ID contains the mouse and should handle scroll events.
    /// See the documentation of [`Hitbox::is_hovered`] for the physical-hover distinction.
    pub fn should_handle_scroll(self, window: &Window) -> bool {
        window.mouse_hit_test.ids.contains(&self)
    }

    fn next(mut self) -> HitboxId {
        HitboxId(self.0.wrapping_add(1))
    }
}

/// An immutable committed geometry capability for exact initial hit testing.
#[derive(Clone, Debug)]
pub struct HitTestSnapshot {
    geometry: ElementGeometry,
    validity: Option<SubtreeGeometryValidity>,
    clip_stack: ClipStackSnapshot,
    active: bool,
}

/// An opaque, frame-bound subtree clip resolved during prepaint.
///
/// This token preserves the exact clip stack and geometry validity used by prepaint so paint
/// cannot reconstruct or inject window-space clip geometry independently.
#[derive(Clone, Debug)]
pub struct PreparedSubtreeClip {
    window_id: WindowId,
    frame_generation: u64,
    parent_transform: ResolvedSubtreeTransform,
    inherited: ClipStackSnapshot,
    resolved: ClipStackSnapshot,
    validity: SubtreeGeometryValidity,
}

impl PreparedSubtreeClip {
    pub(crate) fn is_valid(&self) -> bool {
        self.validity.is_valid()
    }
}

impl PartialEq for HitTestSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.geometry == other.geometry
            && self.clip_stack == other.clip_stack
            && self.active == other.active
    }
}

impl HitTestSnapshot {
    /// Returns the committed layout and displayed geometry.
    pub fn geometry(&self) -> ElementGeometry {
        self.geometry
    }

    /// Returns the conservative window-space AABB of the exact clip stack.
    pub fn displayed_clip_bounds(&self) -> Bounds<Pixels> {
        self.clip_stack.conservative_bounds()
    }

    /// Returns whether this snapshot is currently eligible and exactly contains the window point.
    pub fn is_window_point_target(&self, point: Point<Pixels>) -> bool {
        self.active
            && self
                .validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
            && self.geometry.displayed_bounds().contains(&point)
            && self.clip_stack.contains(point)
    }

    /// Creates an identity snapshot for test adapters.
    #[cfg(any(test, feature = "test-support"))]
    pub fn identity_for_test(bounds: Bounds<Pixels>) -> Self {
        Self {
            geometry: ElementGeometry::identity_for_test(bounds),
            validity: None,
            clip_stack: ClipStackSnapshot::root(bounds),
            active: true,
        }
    }
}

/// A region that potentially blocks hitboxes inserted prior.
/// See [Window::insert_hitbox] for more details.
#[derive(Clone, Debug)]
pub struct Hitbox {
    /// A unique identifier for the hitbox.
    pub id: HitboxId,
    geometry: ElementGeometry,
    validity: Option<SubtreeGeometryValidity>,
    clip_stack: ClipStackSnapshot,
    behavior: HitboxBehavior,
    active: bool,
}

impl Hitbox {
    /// Returns the untransformed post-layout bounds in window layout coordinates.
    pub fn layout_bounds(&self) -> Bounds<Pixels> {
        self.geometry.layout_bounds()
    }

    /// Returns the axis-aligned bounds displayed in window coordinates.
    pub fn displayed_bounds(&self) -> Bounds<Pixels> {
        self.geometry.displayed_bounds()
    }

    /// Returns this hitbox's immutable layout and displayed geometry snapshot.
    pub fn geometry(&self) -> ElementGeometry {
        self.geometry
    }

    /// Captures the immutable geometry and eligibility used for exact initial hit testing.
    pub fn hit_test_snapshot(&self) -> HitTestSnapshot {
        HitTestSnapshot {
            geometry: self.geometry,
            validity: self.validity.clone(),
            clip_stack: self.clip_stack.clone(),
            active: self.active,
        }
    }

    /// Returns the conservative window-space bounds of the clip captured with this hitbox.
    pub fn displayed_clip_bounds(&self) -> Bounds<Pixels> {
        self.clip_stack.conservative_bounds()
    }

    /// Returns how this hitbox participates in occlusion routing.
    pub fn behavior(&self) -> HitboxBehavior {
        self.behavior
    }

    /// Returns whether this hitbox belongs to the current interactive presentation and a transform
    /// scope that survived the frame.
    pub fn is_active(&self) -> bool {
        self.active
            && self
                .validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
    }

    fn retag_validity(&mut self, validity: Option<SubtreeGeometryValidity>) {
        self.validity = SubtreeGeometryValidity::replayed_under(self.validity.as_ref(), validity);
    }

    /// Projects a point relative to this hitbox into displayed window coordinates.
    pub fn local_to_window_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.geometry.local_to_window_point(point)
    }

    /// Inverse-projects a displayed window point relative to this hitbox.
    pub fn window_to_local_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.geometry.window_to_local_point(point)
    }

    /// Projects an absolute layout point into displayed window coordinates.
    pub fn layout_to_window_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.geometry.layout_to_window_point(point)
    }

    /// Inverse-projects a displayed window point into absolute layout coordinates.
    pub fn window_to_layout_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.geometry.window_to_layout_point(point)
    }

    /// Projects a vector from layout coordinates into displayed window coordinates.
    pub fn local_to_window_vector(
        &self,
        vector: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.geometry.local_to_window_vector(vector)
    }

    /// Inverse-projects a displayed window vector into layout coordinates.
    pub fn window_to_local_vector(
        &self,
        vector: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.geometry.window_to_local_vector(vector)
    }

    /// Returns whether a displayed window point lies inside this hitbox and its exact clip stack.
    pub fn contains_window_point(&self, point: Point<Pixels>) -> bool {
        self.geometry.displayed_bounds().contains(&point) && self.clip_stack.contains(point)
    }

    /// Returns whether this committed hitbox is an eligible target for a displayed window point.
    pub fn is_window_point_target(&self, point: Point<Pixels>) -> bool {
        self.hit_test_snapshot().is_window_point_target(point)
    }

    /// Checks if the hitbox is physically hovered. Returns `false` during keyboard input modality
    /// so that keyboard navigation suppresses hover highlights. Use this for visual hover, cursors,
    /// tooltips, and drag-over state; mouse-button handlers should use
    /// [`Self::is_mouse_event_target`] so pointer capture is respected.
    ///
    /// This can return `false` even when the hitbox contains the mouse, if a hitbox in front of
    /// this sets `HitboxBehavior::BlockMouse` (`InteractiveElement::occlude`) or
    /// `HitboxBehavior::BlockMouseExceptScroll` (`InteractiveElement::block_mouse_except_scroll`),
    /// or if the current input modality is keyboard (see [`Window::last_input_was_keyboard`]).
    ///
    /// Handling of `ScrollWheelEvent` should typically use `should_handle_scroll` instead.
    /// Concretely, this is due to use-cases like overlays that cause the elements under to be
    /// non-interactive while still allowing scrolling. More abstractly, this is because
    /// `is_hovered` is about physical state directly under the mouse, while scrolling is about
    /// finding the current outer scrollable container.
    pub fn is_hovered(&self, window: &Window) -> bool {
        self.id.is_hovered(window)
    }

    /// Checks whether this hitbox is the routed target for the current mouse-button event.
    pub fn is_mouse_event_target(&self, window: &Window) -> bool {
        self.id.is_mouse_event_target(window)
    }

    /// Checks if the hitbox contains the mouse and should handle scroll events. See the
    /// documentation of [`Hitbox::is_hovered`] for the physical-hover distinction.
    ///
    /// This can return `false` even when the hitbox contains the mouse, if a hitbox in front of
    /// this sets `HitboxBehavior::BlockMouse` (`InteractiveElement::occlude`).
    pub fn should_handle_scroll(&self, window: &Window) -> bool {
        self.id.should_handle_scroll(window)
    }
}

/// A window-space input event paired with the hitbox geometry of its current target.
///
/// Use the explicit projection helpers for target-local geometry; the raw event remains in window
/// coordinates and is available only through [`Self::window_event`].
pub struct TargetedEvent<E> {
    event: E,
    hitbox: Hitbox,
}

impl<E: Clone> TargetedEvent<E> {
    pub(crate) fn new(event: &E, hitbox: &Hitbox) -> Self {
        Self {
            event: event.clone(),
            hitbox: hitbox.clone(),
        }
    }
}

impl<E> TargetedEvent<E> {
    /// Returns the unchanged window-space platform event.
    pub fn window_event(&self) -> &E {
        &self.event
    }

    /// Returns the current committed target hitbox.
    pub fn hitbox(&self) -> &Hitbox {
        &self.hitbox
    }

    /// Returns target-local bounds with a zero origin and the untransformed layout size.
    pub fn target_local_bounds(&self) -> Bounds<Pixels> {
        Bounds::new(Point::default(), self.hitbox.layout_bounds().size)
    }

    /// Inverse-projects a window-space point relative to the target's layout origin.
    pub fn target_local_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.hitbox.window_to_local_point(point)
    }

    /// Inverse-projects a window-space point into absolute layout coordinates.
    pub fn target_layout_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.hitbox.window_to_layout_point(point)
    }

    /// Inverse-projects a window-space vector into target-local layout units.
    pub fn target_local_vector(
        &self,
        vector: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.hitbox.window_to_local_vector(vector)
    }
}

macro_rules! impl_targeted_event_position {
    ($($event:ty),+ $(,)?) => {
        $(
            impl TargetedEvent<$event> {
                /// Returns the pointer position relative to the target's layout origin.
                pub fn target_local_position(
                    &self,
                ) -> Result<Point<Pixels>, SubtreeTransformError> {
                    self.target_local_point(self.event.position)
                }

                /// Returns the pointer position in absolute target layout coordinates.
                pub fn target_layout_position(
                    &self,
                ) -> Result<Point<Pixels>, SubtreeTransformError> {
                    self.target_layout_point(self.event.position)
                }
            }
        )+
    };
}

impl_targeted_event_position!(
    crate::MouseDownEvent,
    crate::MouseUpEvent,
    crate::MouseMoveEvent,
    crate::MousePressureEvent,
    crate::ScrollWheelEvent,
    crate::PinchEvent,
);

impl TargetedEvent<crate::ClickEvent> {
    /// Returns the click position relative to the target's layout origin.
    pub fn target_local_position(&self) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.target_local_point(self.event.position())
    }

    /// Returns the click position in absolute target layout coordinates.
    pub fn target_layout_position(&self) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.target_layout_point(self.event.position())
    }
}

impl TargetedEvent<crate::ScrollWheelEvent> {
    /// Returns the scroll delta in target-local layout units.
    ///
    /// Pixel deltas are inverse-projected. Line deltas remain semantic line counts.
    pub fn target_local_delta(&self) -> Result<crate::ScrollDelta, SubtreeTransformError> {
        match self.event.delta {
            crate::ScrollDelta::Pixels(delta) => self
                .target_local_vector(delta)
                .map(crate::ScrollDelta::Pixels),
            crate::ScrollDelta::Lines(delta) => Ok(crate::ScrollDelta::Lines(delta)),
        }
    }
}

/// How the hitbox affects mouse behavior.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum HitboxBehavior {
    /// Normal hitbox mouse behavior, doesn't affect mouse handling for other hitboxes.
    #[default]
    Normal,

    /// All hitboxes behind this hitbox will be ignored and so will have `hitbox.is_hovered() ==
    /// false` and `hitbox.should_handle_scroll() == false`. Typically for elements this causes
    /// skipping of all mouse events, hover styles, and tooltips. This flag is set by
    /// [`InteractiveElement::occlude`].
    ///
    /// For mouse handlers that check those hitboxes, this behaves the same as registering a
    /// bubble-phase handler for every mouse event type:
    ///
    /// ```ignore
    /// window.on_mouse_event(move |_: &EveryMouseEventTypeHere, phase, window, cx| {
    ///     if phase == DispatchPhase::Capture && hitbox.is_mouse_event_target(window) {
    ///         cx.stop_propagation();
    ///     }
    /// })
    /// ```
    ///
    /// This has effects beyond event handling - any use of hitbox checking, such as hover
    /// styles and tooltips. These other behaviors are the main point of this mechanism. An
    /// alternative might be to not affect mouse event handling - but this would allow
    /// inconsistent UI where clicks and moves interact with elements that are not considered to
    /// be hovered.
    BlockMouse,

    /// All hitboxes behind this hitbox will have `hitbox.is_hovered() == false`, even when
    /// `hitbox.should_handle_scroll() == true`. Typically for elements this causes all mouse
    /// interaction except scroll events to be ignored - see the documentation of
    /// [`Hitbox::is_hovered`] for details. This flag is set by
    /// [`InteractiveElement::block_mouse_except_scroll`].
    ///
    /// For mouse handlers that check those hitboxes, this behaves the same as registering a
    /// bubble-phase handler for every mouse event type **except** `ScrollWheelEvent`:
    ///
    /// ```ignore
    /// window.on_mouse_event(move |_: &EveryMouseEventTypeExceptScroll, phase, window, cx| {
    ///     if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
    ///         cx.stop_propagation();
    ///     }
    /// })
    /// ```
    ///
    /// See the documentation of [`Hitbox::is_hovered`] for details of why `ScrollWheelEvent` is
    /// handled differently than other mouse events. If also blocking these scroll events is
    /// desired, then a `cx.stop_propagation()` handler like the one above can be used.
    ///
    /// This has effects beyond event handling - this affects any use of `is_hovered`, such as
    /// hover styles and tooltips. These other behaviors are the main point of this mechanism.
    /// An alternative might be to not affect mouse event handling - but this would allow
    /// inconsistent UI where clicks and moves interact with elements that are not considered to
    /// be hovered.
    BlockMouseExceptScroll,
}

/// An identifier for a tooltip.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TooltipId(usize);

impl TooltipId {
    /// Checks if the tooltip is currently hovered.
    pub fn is_hovered(&self, window: &Window) -> bool {
        window
            .tooltip_bounds
            .as_ref()
            .is_some_and(|tooltip_bounds| {
                tooltip_bounds.id == *self
                    && tooltip_bounds
                        .validity
                        .as_ref()
                        .is_none_or(SubtreeGeometryValidity::is_valid)
                    && tooltip_bounds.bounds.contains(&window.mouse_position())
            })
    }
}

#[derive(Clone)]
pub(crate) struct TooltipBounds {
    id: TooltipId,
    bounds: Bounds<Pixels>,
    validity: Option<SubtreeGeometryValidity>,
}

struct PreparedTooltip {
    element: AnyElement,
    validity: Option<SubtreeGeometryValidity>,
}

#[derive(Clone)]
pub(crate) struct AutoscrollIntent {
    bounds: Bounds<Pixels>,
    validity: Option<SubtreeGeometryValidity>,
}

impl AutoscrollIntent {
    pub(crate) fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum InputModality {
    Mouse,
    Keyboard,
}

/// Diagnostic facts recorded when a [`RenderImage`] is painted into a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
#[expect(missing_docs)]
pub struct ImagePaintDiagnostic {
    pub frame_generation: u64,
    pub image: RenderImageParams,
    pub bounds: Bounds<ScaledPixels>,
    pub tile: crate::AtlasTile,
    pub atlas_access: AtlasAccessDiagnostic,
}

/// A frame-local fact describing why a transformed subtree was reduced to layout-only output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(missing_docs)]
pub struct SubtreeTransformDiagnostic {
    pub frame_generation: u64,
    pub error: SubtreeTransformError,
}

/// Metadata describing a framework-rendered frame capture.
#[derive(Clone, Debug, PartialEq)]
#[expect(missing_docs)]
pub struct WindowFrameCaptureMetadata {
    pub window_id: WindowId,
    pub framework_frame_generation: u64,
    pub capture_generation: u64,
    pub scale_factor: f32,
    pub logical_viewport_size: Size<Pixels>,
    pub physical_viewport_size: Size<DevicePixels>,
}

/// Result of an offscreen frame capture.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug)]
#[expect(missing_docs)]
pub struct WindowFrameCapture {
    pub image: Option<image::RgbaImage>,
    pub metadata: WindowFrameCaptureMetadata,
    pub unsupported_reason: Option<String>,
}

#[cfg(any(test, feature = "test-support"))]
impl WindowFrameCapture {
    /// Returns the captured image, or the platform-specific unsupported reason.
    pub fn into_image(self) -> anyhow::Result<image::RgbaImage> {
        self.image.ok_or_else(|| {
            anyhow!(
                "{}",
                self.unsupported_reason
                    .unwrap_or_else(|| "render_to_image not available".to_string())
            )
        })
    }
}

#[derive(Clone)]
struct SubtreeTransformScope {
    transform: ResolvedSubtreeTransform,
    validity: Option<SubtreeGeometryValidity>,
}

#[derive(Clone, Copy)]
enum PrimitiveRasterSnap {
    NearestEdges,
    CoverEdges,
}

struct SubtreeTransformScopeGuard {
    stack: Rc<RefCell<SmallVec<[SubtreeTransformScope; 8]>>>,
    entered_depth: usize,
}

struct SubtreePresentationScopeGuard {
    stack: Rc<RefCell<SmallVec<[SubtreePresentation; 8]>>>,
    entered_depth: usize,
}

struct ClipStackScopeGuard {
    stack: Rc<RefCell<SmallVec<[ClipStackSnapshot; 8]>>>,
    entered_depth: usize,
}

struct AtlasTextureLeasePaintScopeGuard {
    stack: Rc<RefCell<SmallVec<[FxHashSet<AtlasTextureInstanceId>; 8]>>>,
    entered_depth: usize,
}

struct PrepaintLayoutScopeGuard {
    current: Rc<Cell<Option<LayoutId>>>,
    entered: LayoutId,
    previous: Option<LayoutId>,
}

struct PrepaintCommitPhaseScopeGuard {
    current: Rc<Cell<Option<PrepaintCommitPhase>>>,
    entered: PrepaintCommitPhase,
    previous: Option<PrepaintCommitPhase>,
}

impl Drop for SubtreeTransformScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert_eq!(stack.len(), self.entered_depth + 1);
        }
        stack.truncate(self.entered_depth);
    }
}

impl Drop for SubtreePresentationScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert_eq!(stack.len(), self.entered_depth + 1);
        }
        stack.truncate(self.entered_depth);
    }
}

impl Drop for ClipStackScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert_eq!(stack.len(), self.entered_depth + 1);
        }
        stack.truncate(self.entered_depth);
    }
}

impl Drop for AtlasTextureLeasePaintScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        if std::thread::panicking() {
            stack.truncate(self.entered_depth);
            return;
        }

        debug_assert_eq!(stack.len(), self.entered_depth + 1);
        let completed = stack
            .pop()
            .expect("an entered atlas paint scope must remain on the stack");
        if let Some(parent) = stack.last_mut() {
            parent.extend(completed);
        }
    }
}

impl Drop for PrepaintLayoutScopeGuard {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            debug_assert_eq!(self.current.get(), Some(self.entered));
        }
        self.current.set(self.previous);
    }
}

impl Drop for PrepaintCommitPhaseScopeGuard {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            debug_assert_eq!(self.current.get(), Some(self.entered));
        }
        self.current.set(self.previous);
    }
}

struct PlatformWindowCreationRollback {
    app_cell: RcWeak<AppCell>,
    window_id: WindowId,
    platform_window: Option<Box<dyn PlatformWindow>>,
}

impl PlatformWindowCreationRollback {
    fn new(
        window_id: WindowId,
        platform_window: Box<dyn PlatformWindow>,
        app_cell: RcWeak<AppCell>,
    ) -> Self {
        Self {
            app_cell,
            window_id,
            platform_window: Some(platform_window),
        }
    }

    fn into_platform_window(mut self) -> Box<dyn PlatformWindow> {
        self.platform_window
            .take()
            .expect("platform-window creation rollback must retain its backend owner before commit")
    }
}

impl StdDeref for PlatformWindowCreationRollback {
    type Target = dyn PlatformWindow;

    fn deref(&self) -> &Self::Target {
        self.platform_window
            .as_deref()
            .expect("platform-window creation rollback must retain its backend owner before commit")
    }
}

impl StdDerefMut for PlatformWindowCreationRollback {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.platform_window
            .as_deref_mut()
            .expect("platform-window creation rollback must retain its backend owner before commit")
    }
}

impl Drop for PlatformWindowCreationRollback {
    fn drop(&mut self) {
        let Some(platform_window) = self.platform_window.take() else {
            return;
        };
        let presentation_shutdown = platform_window
            .prepare_presentation_shutdown(next_presentation_shutdown_ticket(self.window_id));

        if let Some(app_cell) = self.app_cell.upgrade() {
            app_cell.enqueue_platform_window_retirement(
                self.window_id,
                platform_window,
                presentation_shutdown,
            );
        } else {
            log::error!(
                "native platform-window creation rollback lost its AppCell; retaining the backend owner until process teardown"
            );
            mem::forget((platform_window, presentation_shutdown));
        }
    }
}

#[derive(Default)]
struct WindowPresentationState {
    frame_accepted_generation: Option<u64>,
    present_submitted_generation: Option<u64>,
    non_empty_presented_generation: Option<u64>,
    latest_present_attempt: Option<WindowPresentAttemptFacts>,
    renderer_invalidated_generation: Option<u64>,
    initial_presentation: WindowInitialPresentationStatus,
    provisional_reveal_ticket: Option<WindowProvisionalRevealTicket>,
}

#[derive(Default)]
struct InitialPresentationRetryState {
    minimum_generation: Option<u64>,
    attempts_started: u8,
    presentation_retry_generation: Option<u64>,
    presentation_retries_started: u8,
}

const FRESH_INITIAL_PRESENTATION_ATTEMPT_LIMIT: u8 = 3;
const INITIAL_PRESENTATION_RETRY_LIMIT: u8 = 3;

/// Holds the state for a specific window.
pub struct Window {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) invalidator: WindowInvalidator,
    pub(crate) removed: bool,
    native_closed: Rc<Cell<bool>>,
    removal_state: WindowRemovalState,
    should_close_handler: WindowShouldCloseHandlerSlot,
    pub(crate) platform_window: Box<dyn PlatformWindow>,
    platform_command_sink: PlatformWindowCommandSink,
    initial_presentation_command: Option<PlatformWindowCommand>,
    initial_presentation_retry: InitialPresentationRetryState,
    presentation_shutdown: Option<PreparedPlatformPresentationShutdown>,
    provisional_session: Option<WindowProvisionalSession>,
    _provisional_opening_claim: Option<WindowProvisionalOpeningClaim>,
    creation_facts: WindowCreationFacts,
    presentation_state: WindowPresentationState,
    platform_facts: WindowPlatformFacts,
    window_kind: WindowKind,
    window_capabilities: PlatformWindowCapabilities,
    window_mutation_authority: Arc<WindowPlatformMutationAuthority>,
    window_mutations: WindowMutationState,
    display_id: Option<DisplayId>,
    sprite_atlas: Arc<dyn PlatformAtlas>,
    text_system: Arc<WindowTextSystem>,
    text_rendering_mode: Rc<Cell<TextRenderingMode>>,
    rem_size: Pixels,
    /// The stack of override values for the window's rem size.
    ///
    /// This is used by `with_rem_size` to allow rendering an element tree with
    /// a given rem size.
    rem_size_override_stack: SmallVec<[Pixels; 8]>,
    pub(crate) viewport_size: Size<Pixels>,
    layout_engine: Option<TaffyLayoutEngine>,
    pub(crate) root: Option<AnyView>,
    pub(crate) element_id_stack: SmallVec<[ElementId; 32]>,
    pub(crate) text_style_stack: Vec<TextStyleRefinement>,
    pub(crate) rendered_entity_stack: Vec<EntityId>,
    pub(crate) element_offset_stack: Vec<Point<Pixels>>,
    current_prepaint_layout_id: Rc<Cell<Option<LayoutId>>>,
    subtree_presentation_stack: Rc<RefCell<SmallVec<[SubtreePresentation; 8]>>>,
    subtree_transform_stack: Rc<RefCell<SmallVec<[SubtreeTransformScope; 8]>>>,
    pub(crate) element_opacity: f32,
    clip_stack: Rc<RefCell<SmallVec<[ClipStackSnapshot; 8]>>>,
    pub(crate) requested_autoscroll: Option<AutoscrollIntent>,
    pub(crate) image_cache_stack: Vec<AnyImageCache>,
    pub(crate) rendered_frame: Frame,
    pub(crate) next_frame: Frame,
    atlas_texture_lease_paint_scopes: Rc<RefCell<SmallVec<[FxHashSet<AtlasTextureInstanceId>; 8]>>>,
    candidate_frame_transfers: CandidateFrameTransfers,
    next_candidate_frame_attempt_id: u64,
    candidate_frame_transaction: Option<CandidateFrameTransaction>,
    candidate_atlas_lease_failure: Option<AtlasTextureLeaseError>,
    last_atlas_frame_rejection: Option<AtlasFrameRejection>,
    candidate_pending_input_clear: bool,
    candidate_pending_input_notification: bool,
    retained_visual_registry: retained_visual::Registry,
    #[cfg(any(test, feature = "test-support"))]
    capture_generation: Cell<u64>,
    atlas_remove_diagnostics: Vec<AtlasRemoveDiagnostic>,
    next_hitbox_id: HitboxId,
    next_pointer_capture_id: PointerCaptureId,
    next_portal_anchor_id: PortalAnchorId,
    portal_anchor_capture_stack: Rc<RefCell<Vec<PortalAnchorCapture>>>,
    next_reveal_target_id: RevealTargetId,
    reveal_target_capture_stack: Rc<RefCell<Vec<RevealTargetCapture>>>,
    scroll_ancestry_stack: Rc<RefCell<SmallVec<[ScrollContainerBinding; 8]>>>,
    next_bring_into_view_sequence: u64,
    next_bring_into_view_chain_generation: u64,
    active_bring_into_view_requests: Vec<ActiveBringIntoViewRequest>,
    bring_into_view_resolutions: Vec<BringIntoViewResolution>,
    pub(crate) next_tooltip_id: TooltipId,
    pub(crate) tooltip_bounds: Option<TooltipBounds>,
    next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>>,
    pub(crate) dirty_views: FxHashSet<EntityId>,
    focus_listeners: SubscriberSet<(), AnyWindowFocusListener>,
    key_down_interceptors: SubscriberSet<(), AnyWindowKeyDownInterceptor>,
    mouse_interceptors: SubscriberSet<(), AnyWindowMouseInterceptor>,
    window_states: Rc<RefCell<TypeIdHashMap<WindowStateSlot>>>,
    input_dispatch_active: Rc<Cell<bool>>,
    input_transaction_depth: Rc<Cell<usize>>,
    mouse_event_target: Rc<Cell<Option<HitboxId>>>,
    pub(crate) focus_lost_listeners: SubscriberSet<(), AnyObserver>,
    default_prevented: bool,
    last_dispatch_event_result: Option<DispatchEventResult>,
    mouse_position: Point<Pixels>,
    mouse_in_window: bool,
    mouse_hit_test: HitTest,
    modifiers: Modifiers,
    capslock: Capslock,
    scale_factor: f32,
    pub(crate) bounds_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) initial_presentation_observers: SubscriberSet<(), AnyObserver>,
    appearance: WindowAppearance,
    pub(crate) appearance_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) button_layout_observers: SubscriberSet<(), AnyObserver>,
    active: Rc<Cell<bool>>,
    hovered: Rc<Cell<bool>>,
    pub(crate) needs_present: Rc<Cell<bool>>,
    /// Tracks recent input event timestamps to determine if input is arriving at a high rate.
    /// Used to selectively enable VRR optimization only when input rate exceeds 60fps.
    pub(crate) input_rate_tracker: Rc<RefCell<InputRateTracker>>,
    last_frame_time: Cell<Option<Instant>>,
    #[cfg(feature = "input-latency-histogram")]
    input_latency_tracker: InputLatencyTracker,
    last_input_modality: InputModality,
    pub(crate) refreshing: bool,
    pub(crate) activation_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) focus: Option<FocusId>,
    candidate_accessibility_focus: Option<FocusId>,
    pending_focus_claim: Option<PendingFocusClaim>,
    pending_focus_reveal_fence: Option<PendingFocusRevealFence>,
    pending_blur_claim_generation: Option<u64>,
    provisional_focus_claim: Option<ProvisionalFocusClaim>,
    pending_focus_completion: Option<PendingFocusCompletion>,
    focus_claim_resolutions: Vec<FocusClaimResolution>,
    next_focus_claim_id: u64,
    focus_claim_revision: u64,
    prepaint_commit_phase: Rc<Cell<Option<PrepaintCommitPhase>>>,
    frame_focus_authority_sealed: bool,
    focus_followup_requested: bool,
    sealed_focus_retry_rejection: Option<FocusClaimTarget>,
    key_event_revision: u64,
    focus_enabled: bool,
    pending_input: Option<PendingInput>,
    pending_modifier: ModifierState,
    pub(crate) pending_input_observers: SubscriberSet<(), AnyObserver>,
    prompt: Option<RenderablePromptHandle>,
    pub(crate) client_inset: Option<Pixels>,
    pressed_mouse_buttons: PressedMouseButtons,
    /// The stable owner that has captured the pointer, if any.
    captured_pointer: Option<PointerCapture>,
    pending_pointer_cancellations: VecDeque<PendingPointerCancellation>,
    pointer_cancel_session_already_settled: bool,
    #[cfg(any(feature = "inspector", debug_assertions))]
    inspector: Option<Entity<Inspector>>,
    pub(crate) a11y: A11y,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowRemovalState {
    Open,
    PendingAfterInput,
    Removing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingFocusClaim {
    target: FocusId,
    target_generation: u64,
}

#[derive(Clone)]
struct PendingFocusRevealFence {
    target: FocusId,
    fence: ScrollChainFence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProvisionalFocusClaim {
    target: FocusId,
    fallback: Option<FocusId>,
}

#[derive(Default)]
struct CandidateFrameTransfers {
    element_states: Vec<(GlobalElementId, TypeId)>,
    mouse_listeners: Vec<(usize, usize)>,
    input_handlers: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateFrameAttemptId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateFrameTransactionPhase {
    Building,
    Accepted,
}

struct CandidatePrepaintCommitPlan {
    target_revision: u64,
    current_commits: Vec<FrameOutput<PrepaintCommit>>,
    previous_commits: Vec<FrameOutput<PrepaintCommit>>,
}

struct CandidateFrameTransaction {
    attempt_id: CandidateFrameAttemptId,
    frame_generation: u64,
    phase: CandidateFrameTransactionPhase,
    replayed_retained_visuals: FxHashSet<retained_visual::TicketIdentity>,
    focus_completion_ids: FxHashSet<u64>,
    prepaint_commit_plan: Option<Rc<CandidatePrepaintCommitPlan>>,
}

impl CandidateFrameTransaction {
    fn new(attempt_id: CandidateFrameAttemptId, frame_generation: u64) -> Self {
        Self {
            attempt_id,
            frame_generation,
            phase: CandidateFrameTransactionPhase::Building,
            replayed_retained_visuals: FxHashSet::default(),
            focus_completion_ids: FxHashSet::default(),
            prepaint_commit_plan: None,
        }
    }

    fn retained_visual_was_replayed(&self, ticket: retained_visual::TicketIdentity) -> bool {
        self.replayed_retained_visuals.contains(&ticket)
    }

    fn record_retained_visual_replay(&mut self, ticket: retained_visual::TicketIdentity) {
        assert!(
            self.replayed_retained_visuals.insert(ticket),
            "one candidate attempt must record each retained visual replay at most once"
        );
    }

    fn record_focus_completion(&mut self, id: u64) {
        assert!(
            self.focus_completion_ids.insert(id),
            "one candidate attempt must record each focus completion at most once"
        );
    }

    fn owns_focus_completion(&self, id: u64) -> bool {
        self.focus_completion_ids.contains(&id)
    }

    fn prepare_prepaint_commits(
        &mut self,
        current_commits: Vec<FrameOutput<PrepaintCommit>>,
        previous_commits: Vec<FrameOutput<PrepaintCommit>>,
    ) -> Rc<CandidatePrepaintCommitPlan> {
        assert_eq!(self.phase, CandidateFrameTransactionPhase::Building);
        assert!(
            self.prepaint_commit_plan.is_none(),
            "one candidate frame must prepare its prepaint commit plan exactly once"
        );
        let plan = Rc::new(CandidatePrepaintCommitPlan {
            target_revision: self.frame_generation,
            current_commits,
            previous_commits,
        });
        self.prepaint_commit_plan = Some(plan.clone());
        plan
    }

    fn mark_accepted(&mut self) {
        assert_eq!(self.phase, CandidateFrameTransactionPhase::Building);
        assert!(
            self.prepaint_commit_plan.is_some(),
            "a candidate frame must prepare publications before acceptance"
        );
        self.phase = CandidateFrameTransactionPhase::Accepted;
    }

    fn is_accepted(&self) -> bool {
        self.phase == CandidateFrameTransactionPhase::Accepted
    }
}

struct CandidateFrameAuthorityCheckpoint {
    focus: Option<FocusId>,
    pending_focus_claim: Option<PendingFocusClaim>,
    pending_focus_reveal_fence: Option<PendingFocusRevealFence>,
    pending_blur_claim_generation: Option<u64>,
    provisional_focus_claim: Option<ProvisionalFocusClaim>,
    pending_focus_completion: Option<PendingFocusCompletion>,
    focus_claim_resolutions_len: usize,
    focus_claim_revision: u64,
    requested_autoscroll: Option<AutoscrollIntent>,
    tooltip_bounds: Option<TooltipBounds>,
    focus_followup_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AtlasFrameRejection {
    generation: u64,
    error: AtlasTextureLeaseError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusClaimTarget {
    Exact(FocusId),
    Empty,
}

type AnyFocusClaimCompletion = Box<dyn FnOnce(FocusClaimOutcome, &mut Window, &mut App) + 'static>;
type SharedFocusClaimCompletion = Rc<RefCell<Option<AnyFocusClaimCompletion>>>;
type WindowShouldCloseCallback = Box<dyn FnMut(&mut Window, &mut App) -> bool + 'static>;

#[derive(Default)]
struct WindowShouldCloseHandlerState {
    generation: u64,
    checked_out_generation: Option<u64>,
    callback: Option<WindowShouldCloseCallback>,
    terminal: bool,
}

#[derive(Clone, Default)]
struct WindowShouldCloseHandlerSlot {
    state: Rc<RefCell<WindowShouldCloseHandlerState>>,
}

impl WindowShouldCloseHandlerSlot {
    fn set(&self, callback: WindowShouldCloseCallback) {
        let replaced = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.generation = state
                .generation
                .checked_add(1)
                .expect("window should-close handler generation overflow");
            state.callback.replace(callback)
        };
        drop(replaced);
    }

    fn invoke(&self, window: &mut Window, cx: &mut App) -> bool {
        let mut checkout = {
            let mut state = self.state.borrow_mut();
            if state.terminal || state.checked_out_generation == Some(state.generation) {
                return false;
            }
            let Some(callback) = state.callback.take() else {
                return true;
            };
            let generation = state.generation;
            state.checked_out_generation = Some(generation);
            WindowShouldCloseHandlerCheckout {
                slot: self.clone(),
                generation,
                callback: Some(callback),
            }
        };
        (checkout.callback_mut())(window, cx)
    }

    fn terminate(&self) {
        let callback = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.terminal = true;
            state.generation = state
                .generation
                .checked_add(1)
                .expect("window should-close handler generation overflow");
            state.callback.take()
        };
        drop(callback);
    }
}

struct WindowShouldCloseHandlerCheckout {
    slot: WindowShouldCloseHandlerSlot,
    generation: u64,
    callback: Option<WindowShouldCloseCallback>,
}

impl WindowShouldCloseHandlerCheckout {
    fn callback_mut(&mut self) -> &mut WindowShouldCloseCallback {
        self.callback
            .as_mut()
            .expect("checked-out window should-close handler must remain available")
    }
}

impl Drop for WindowShouldCloseHandlerCheckout {
    fn drop(&mut self) {
        let retired_callback = {
            let mut state = self.slot.state.borrow_mut();
            if state.checked_out_generation == Some(self.generation) {
                state.checked_out_generation = None;
                if !state.terminal
                    && state.generation == self.generation
                    && state.callback.is_none()
                {
                    state.callback = self.callback.take();
                }
            }
            self.callback.take()
        };
        drop(retired_callback);
    }
}

type WindowPanicPayload = Box<dyn Any + Send + 'static>;

fn retain_first_window_cleanup_panic(
    first: &mut Option<WindowPanicPayload>,
    result: std::thread::Result<()>,
    stage: &'static str,
) {
    let Err(payload) = result else {
        return;
    };
    if first.is_none() {
        *first = Some(payload);
    } else {
        log::error!("suppressed secondary panic while settling window removal stage `{stage}`");
    }
}

fn finish_after_window_cleanup<R>(
    primary: std::thread::Result<R>,
    cleanup: std::thread::Result<()>,
    stage: &'static str,
) -> R {
    match (primary, cleanup) {
        (Ok(result), Ok(())) => result,
        (Ok(_), Err(payload)) => std::panic::resume_unwind(payload),
        (Err(payload), Ok(())) => std::panic::resume_unwind(payload),
        (Err(primary), Err(_secondary)) => {
            log::error!("suppressed secondary panic while settling `{stage}`");
            std::panic::resume_unwind(primary)
        }
    }
}

#[derive(Clone)]
struct PendingFocusCompletion {
    id: u64,
    target: FocusClaimTarget,
    target_generation: u64,
    callback: SharedFocusClaimCompletion,
}

struct FocusClaimResolution {
    id: u64,
    outcome: FocusClaimOutcome,
    callback: SharedFocusClaimCompletion,
}

struct InputTransactionGuard {
    depth: Rc<Cell<usize>>,
}

impl InputTransactionGuard {
    fn enter(depth: Rc<Cell<usize>>) -> Self {
        depth.set(depth.get().saturating_add(1));
        Self { depth }
    }
}

impl Drop for InputTransactionGuard {
    fn drop(&mut self) {
        let depth = self.depth.get();
        debug_assert!(depth > 0, "input transaction depth must remain balanced");
        self.depth.set(depth.saturating_sub(1));
    }
}

enum WindowStateSlot {
    Initializing {
        type_name: &'static str,
        token: Rc<()>,
    },
    Ready(AnyEntity),
}

struct WindowStateInitializationGuard {
    slots: Rc<RefCell<TypeIdHashMap<WindowStateSlot>>>,
    state_type: TypeId,
    token: Rc<()>,
}

impl Drop for WindowStateInitializationGuard {
    fn drop(&mut self) {
        let should_remove = matches!(
            self.slots.borrow().get(&self.state_type),
            Some(WindowStateSlot::Initializing { token, .. })
                if Rc::ptr_eq(token, &self.token)
        );
        if should_remove {
            self.slots.borrow_mut().remove(&self.state_type);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ModifierState {
    modifiers: Modifiers,
    saw_keystroke: bool,
}

/// Tracks input event timestamps to determine if input is arriving at a high rate.
/// Used for selective VRR (Variable Refresh Rate) optimization.
#[derive(Clone, Debug)]
pub(crate) struct InputRateTracker {
    timestamps: Vec<Instant>,
    window: Duration,
    inputs_per_second: u32,
    sustain_until: Instant,
    sustain_duration: Duration,
}

impl Default for InputRateTracker {
    fn default() -> Self {
        Self {
            timestamps: Vec::new(),
            window: Duration::from_millis(100),
            inputs_per_second: 60,
            sustain_until: Instant::now(),
            sustain_duration: Duration::from_secs(1),
        }
    }
}

impl InputRateTracker {
    pub fn record_input(&mut self) {
        let now = Instant::now();
        self.timestamps.push(now);
        self.prune_old_timestamps(now);

        let min_events = self.inputs_per_second as u128 * self.window.as_millis() / 1000;
        if self.timestamps.len() as u128 >= min_events {
            self.sustain_until = now + self.sustain_duration;
        }
    }

    pub fn is_high_rate(&self) -> bool {
        Instant::now() < self.sustain_until
    }

    fn prune_old_timestamps(&mut self, now: Instant) {
        self.timestamps
            .retain(|&t| now.duration_since(t) <= self.window);
    }
}

/// A point-in-time snapshot of the input-latency histograms for a window,
/// suitable for external formatting.
#[cfg(feature = "input-latency-histogram")]
pub struct InputLatencySnapshot {
    /// Histogram of input-to-frame latency samples, in nanoseconds.
    pub latency_histogram: Histogram<u64>,
    /// Histogram of input events coalesced per rendered frame.
    pub events_per_frame_histogram: Histogram<u64>,
    /// Count of input events that arrived mid-draw and were excluded from
    /// latency recording.
    pub mid_draw_events_dropped: u64,
}

/// Records the time between when the first input event in a frame is dispatched
/// and when the resulting frame is presented, capturing worst-case latency when
/// multiple events are coalesced into a single frame.
#[cfg(feature = "input-latency-histogram")]
struct InputLatencyTracker {
    /// Timestamp of the first unrendered input event in the current frame;
    /// cleared when a frame is presented.
    first_input_at: Option<Instant>,
    /// Count of input events received since the last frame was presented.
    pending_input_count: u64,
    /// Histogram of input-to-frame latency samples, in nanoseconds.
    latency_histogram: Histogram<u64>,
    /// Histogram of input events coalesced per rendered frame.
    events_per_frame_histogram: Histogram<u64>,
    /// Count of input events that arrived mid-draw and were excluded from
    /// latency recording because their effects won't appear until the next frame.
    mid_draw_events_dropped: u64,
}

#[cfg(feature = "input-latency-histogram")]
impl InputLatencyTracker {
    fn new() -> Result<Self> {
        Ok(Self {
            first_input_at: None,
            pending_input_count: 0,
            latency_histogram: Histogram::new(3)
                .map_err(|e| anyhow!("Failed to create input latency histogram: {e}"))?,
            events_per_frame_histogram: Histogram::new(3)
                .map_err(|e| anyhow!("Failed to create events per frame histogram: {e}"))?,
            mid_draw_events_dropped: 0,
        })
    }

    /// Record that an input event was dispatched at the given time.
    /// Only the first event's timestamp per frame is retained (worst-case latency).
    fn record_input(&mut self, dispatch_time: Instant) {
        self.first_input_at.get_or_insert(dispatch_time);
        self.pending_input_count += 1;
    }

    /// Record that an input event arrived during a draw phase and was excluded
    /// from latency tracking.
    fn record_mid_draw_input(&mut self) {
        self.mid_draw_events_dropped += 1;
    }

    /// Record that a frame was presented, flushing pending latency and coalescing samples.
    fn record_frame_presented(&mut self) {
        if let Some(first_input_at) = self.first_input_at.take() {
            let latency_nanos = first_input_at.elapsed().as_nanos() as u64;
            self.latency_histogram.record(latency_nanos).ok();
        }
        if self.pending_input_count > 0 {
            self.events_per_frame_histogram
                .record(self.pending_input_count)
                .ok();
            self.pending_input_count = 0;
        }
    }

    fn snapshot(&self) -> InputLatencySnapshot {
        InputLatencySnapshot {
            latency_histogram: self.latency_histogram.clone(),
            events_per_frame_histogram: self.events_per_frame_histogram.clone(),
            mid_draw_events_dropped: self.mid_draw_events_dropped,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrawPhase {
    None,
    Prepaint,
    Paint,
    Focus,
}

#[derive(Default, Debug)]
struct PendingInput {
    keystrokes: SmallVec<[Keystroke; 1]>,
    focus: Option<FocusId>,
    context_stack: Option<Vec<KeyContext>>,
    timer: Option<Task<()>>,
    needs_timeout: bool,
}

pub(crate) struct ElementStateBox {
    pub(crate) inner: Box<dyn Any>,
    #[cfg(debug_assertions)]
    pub(crate) type_name: &'static str,
}

fn default_bounds(display_id: Option<DisplayId>, cx: &mut App) -> WindowBounds {
    // TODO, BUG: if you open a window with the currently active window
    // on the stack, this will erroneously fallback to `None`
    //
    // TODO these should be the initial window bounds not considering maximized/fullscreen
    let active_window_bounds = cx
        .active_window()
        .and_then(|w| w.update(cx, |_, window, _| window.window_bounds()).ok());

    const CASCADE_OFFSET: f32 = 25.0;

    let display = display_id
        .map(|id| cx.find_display(id))
        .unwrap_or_else(|| cx.primary_display());

    let default_placement = || Bounds::new(point(px(0.), px(0.)), DEFAULT_WINDOW_SIZE);

    // Use visible_bounds to exclude taskbar/dock areas
    let display_bounds = display
        .as_ref()
        .map(|d| d.visible_bounds())
        .unwrap_or_else(default_placement);

    let (
        Bounds {
            origin: base_origin,
            size: base_size,
        },
        window_bounds_ctor,
    ): (_, fn(Bounds<Pixels>) -> WindowBounds) = match active_window_bounds {
        Some(bounds) => match bounds {
            WindowBounds::Windowed(bounds) => (bounds, WindowBounds::Windowed),
            WindowBounds::Maximized(bounds) => (bounds, WindowBounds::Maximized),
            WindowBounds::Fullscreen(bounds) => (bounds, WindowBounds::Fullscreen),
        },
        None => (
            display
                .as_ref()
                .map(|d| d.default_bounds())
                .unwrap_or_else(default_placement),
            WindowBounds::Windowed,
        ),
    };

    let cascade_offset = point(px(CASCADE_OFFSET), px(CASCADE_OFFSET));
    let proposed_origin = base_origin + cascade_offset;
    let proposed_bounds = Bounds::new(proposed_origin, base_size);

    let display_right = display_bounds.origin.x + display_bounds.size.width;
    let display_bottom = display_bounds.origin.y + display_bounds.size.height;
    let window_right = proposed_bounds.origin.x + proposed_bounds.size.width;
    let window_bottom = proposed_bounds.origin.y + proposed_bounds.size.height;

    let fits_horizontally = window_right <= display_right;
    let fits_vertically = window_bottom <= display_bottom;

    let final_origin = match (fits_horizontally, fits_vertically) {
        (true, true) => proposed_origin,
        (false, true) => point(display_bounds.origin.x, base_origin.y),
        (true, false) => point(base_origin.x, display_bounds.origin.y),
        (false, false) => display_bounds.origin,
    };
    window_bounds_ctor(Bounds::new(final_origin, base_size))
}

impl Window {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<Self> {
        let WindowOptions {
            window_bounds,
            titlebar,
            focus_on_appearing,
            activation_policy,
            show,
            provisional_session,
            transient_for,
            kind,
            is_movable,
            is_resizable,
            is_minimizable,
            accepts_pointer_input,
            display_id,
            window_background,
            app_id,
            window_min_size,
            window_decorations,
            #[cfg_attr(
                not(any(target_os = "linux", target_os = "freebsd")),
                allow(unused_variables)
            )]
            icon,
            #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
            tabbing_identifier,
        } = options;

        let transient_for = transient_for
            .map(|owner| {
                anyhow::ensure!(
                    owner.belongs_to(&cx.this),
                    "transient owner belongs to a different application"
                );
                let owner = owner.window();
                anyhow::ensure!(
                    owner != handle,
                    "a top-level window cannot be transient for itself"
                );
                anyhow::ensure!(
                    cx.window_handles.get(&owner.window_id()) == Some(&owner),
                    "transient owner is closed or its generation is stale"
                );
                Ok(owner)
            })
            .transpose()?;

        if provisional_session.is_some() {
            anyhow::ensure!(
                show,
                "a provisional window session requires eventual native presentation"
            );
            anyhow::ensure!(
                !focus_on_appearing,
                "a provisional window session cannot request initial activation"
            );
        }

        let requested_display_id = display_id;
        let display_id = cx.resolve_display_id(display_id);
        if let Some(requested_display_id) = requested_display_id
            && display_id.is_none()
        {
            log::warn!(
                "requested display {} is unavailable; opening the window on the default display",
                u64::from(requested_display_id)
            );
        }
        let window_bounds = window_bounds.unwrap_or_else(|| default_bounds(display_id, cx));
        let window_kind = kind.clone();
        let window_capabilities = cx.window_capabilities_for(&kind, display_id);
        anyhow::ensure!(
            focus_on_appearing
                || window_capabilities
                    .creation
                    .focus_on_appearing
                    .is_supported(),
            "platform does not support non-activating first appearance"
        );
        anyhow::ensure!(
            transient_for.is_none() || window_capabilities.creation.transient_for.is_supported(),
            "platform does not support transient top-level owners"
        );
        anyhow::ensure!(
            provisional_session.is_none()
                || window_capabilities
                    .creation
                    .provisional_presentation
                    .is_supported(),
            "platform does not support provisional top-level presentation"
        );
        anyhow::ensure!(
            activation_policy == WindowActivationPolicy::default()
                || window_capabilities
                    .mutations
                    .activation_policy
                    .is_available_at_creation(),
            "platform does not support selecting a lifetime activation policy"
        );
        let provisional_opening_claim = provisional_session
            .as_ref()
            .map(WindowProvisionalSession::claim_opening)
            .transpose()?;
        let platform_window = cx.platform.open_window(
            handle,
            WindowParams {
                window_bounds,
                bounds: window_bounds.get_bounds(),
                titlebar,
                kind,
                is_movable,
                is_resizable,
                is_minimizable,
                accepts_pointer_input,
                focus_on_appearing,
                activation_policy,
                transient_for,
                show,
                provisional_session: provisional_session.clone(),
                display_id,
                window_min_size,
                icon,
                #[cfg(target_os = "macos")]
                tabbing_identifier,
            },
        )?;
        let mut platform_window = PlatformWindowCreationRollback::new(
            handle.window_id(),
            platform_window,
            cx.this.clone(),
        );

        platform_window
            .request_decorations(window_decorations.unwrap_or(WindowDecorations::Server));
        platform_window.set_background_appearance(window_background);

        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let invalidator = WindowInvalidator::new();
        let needs_present = Rc::new(Cell::new(false));
        let next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>> = Default::default();
        let input_rate_tracker = Rc::new(RefCell::new(InputRateTracker::default()));

        let accessibility_force_disabled = cx.accessibility_force_disabled;
        let a11y_active_state = Arc::new(AtomicU64::new(0));

        #[cfg(not(target_family = "wasm"))]
        if !accessibility_force_disabled {
            let window_id = handle.window_id();
            let accessibility_ingress = cx
                .this
                .upgrade()
                .expect("window construction requires a live AppCell")
                .native_accessibility_ingress();
            platform_window.a11y_init(crate::A11yCallbacks {
                activation: {
                    let active_state = a11y_active_state.clone();
                    let accessibility_ingress = accessibility_ingress.clone();
                    Box::new(move || {
                        accessibility_ingress.activated(window_id, &active_state);
                        log::info!("Accessibility activated");
                        // A complete tree cannot be produced synchronously here. The platform
                        // adapter owns any placeholder until the sequenced refresh is delivered.
                        None
                    })
                },
                action: {
                    let active_state = a11y_active_state.clone();
                    let accessibility_ingress = accessibility_ingress.clone();
                    Box::new(move |request| {
                        accessibility_ingress.action(window_id, &active_state, request);
                    })
                },
                deactivation: {
                    let active_state = a11y_active_state.clone();
                    Box::new(move || {
                        accessibility_ingress.deactivated(window_id, &active_state);
                        log::info!("Accessibility deactivated");
                    })
                },
            });
        }

        let native_closed = Rc::new(Cell::new(false));
        platform_window.on_close(Box::new({
            let window_id = handle.window_id();
            let cx = cx.to_async();
            let native_closed = native_closed.clone();
            move || {
                native_closed.set(true);
                cx.enqueue_window_closed(window_id);
            }
        }));
        platform_window.on_should_close(Box::new({
            let window_id = handle.window_id();
            let cx = cx.to_async();
            move || cx.dispatch_window_should_close(window_id)
        }));
        platform_window.on_request_frame(Box::new({
            let cx = cx.to_async();
            move |request_frame_options| {
                cx.enqueue_window_frame_requested(handle.window_id(), request_frame_options);
            }
        }));
        platform_window.on_resize(Box::new({
            let cx = cx.to_async();
            move |_, _| {
                cx.enqueue_window_resized(handle.window_id());
            }
        }));
        platform_window.on_moved(Box::new({
            let cx = cx.to_async();
            move || {
                cx.enqueue_window_moved(handle.window_id());
            }
        }));
        platform_window.on_window_state_change(Box::new({
            let cx = cx.to_async();
            move || {
                cx.enqueue_window_state_changed(handle.window_id());
            }
        }));
        platform_window.on_window_mutation_observation(Box::new({
            let cx = cx.to_async();
            move |facts| {
                cx.enqueue_window_mutation_observation(handle.window_id(), facts);
            }
        }));
        platform_window.on_appearance_changed(Box::new({
            let cx = cx.to_async();
            move || {
                cx.enqueue_window_appearance_changed(handle.window_id());
            }
        }));
        platform_window.on_button_layout_changed(Box::new({
            let cx = cx.to_async();
            move || {
                cx.enqueue_window_button_layout_changed(handle.window_id());
            }
        }));
        platform_window.on_active_status_change(Box::new({
            let cx = cx.to_async();
            move |active| {
                cx.enqueue_window_active_changed(handle.window_id(), active);
            }
        }));
        platform_window.on_modifiers_changed(Box::new({
            let cx = cx.to_async();
            move |event| {
                cx.enqueue_window_modifiers_changed(handle.window_id(), event);
            }
        }));
        platform_window.on_hover_status_change(Box::new({
            let cx = cx.to_async();
            move |active| {
                cx.enqueue_window_hover_changed(handle.window_id(), active);
            }
        }));
        platform_window.on_hit_test_window_control({
            let cx = cx.to_async();
            Box::new(move || cx.native_window_control_area(handle.window_id()))
        });
        platform_window.on_move_tab_to_new_window({
            let cx = cx.to_async();
            Box::new(move || {
                cx.enqueue_move_tab_to_new_window(handle.window_id());
            })
        });
        platform_window.on_merge_all_windows({
            let cx = cx.to_async();
            Box::new(move || {
                cx.enqueue_merge_all_windows(handle.window_id());
            })
        });
        platform_window.on_select_next_tab({
            let cx = cx.to_async();
            Box::new(move || {
                cx.enqueue_select_next_tab(handle.window_id());
            })
        });
        platform_window.on_select_previous_tab({
            let cx = cx.to_async();
            Box::new(move || {
                cx.enqueue_select_previous_tab(handle.window_id());
            })
        });
        platform_window.on_toggle_tab_bar({
            let cx = cx.to_async();
            Box::new(move || {
                cx.enqueue_toggle_tab_bar(handle.window_id());
            })
        });
        {
            let window_id = handle.window_id();
            let cx = cx.to_async();
            platform_window.on_input(crate::PlatformInputCallback::new_for_window(
                cx.clone(),
                window_id,
                Box::new(move |event| {
                    cx.dispatch_native_window_input(window_id, event)
                        .unwrap_or_else(|violation| std::panic::panic_any(violation))
                }),
            ));
        }

        if let Some(app_id) = app_id {
            platform_window.set_app_id(&app_id);
        }

        let platform_command_sink = PlatformWindowCommandSink::new(
            cx.this.clone(),
            handle.window_id(),
            platform_window.command_dispatcher(),
        );
        platform_window.map_window()?;

        // Mapping may resolve the actual display, scale, bounds, and native state. These coherent
        // post-map facts seed the root builder and first frame; presentation and activation still
        // wait for the exact registry commit.
        let creation_facts = platform_window.creation_facts();
        anyhow::ensure!(
            creation_facts.show == show,
            "platform did not preserve the requested initial visibility"
        );
        if window_capabilities
            .creation
            .focus_on_appearing
            .is_supported()
        {
            anyhow::ensure!(
                creation_facts.focus_on_appearing == focus_on_appearing,
                "platform did not establish the requested first-appearance policy"
            );
        }
        if window_capabilities.creation.transient_for.is_supported() {
            anyhow::ensure!(
                creation_facts.transient_for == transient_for,
                "platform did not establish the requested transient owner"
            );
        } else {
            anyhow::ensure!(
                creation_facts.transient_for.is_none(),
                "platform reported a transient owner despite unsupported capability"
            );
        }
        let mut platform_facts = platform_window.platform_facts();
        platform_facts.background_appearance = platform_window.background_appearance();
        if window_capabilities
            .mutations
            .activation_policy
            .is_available_at_creation()
        {
            anyhow::ensure!(
                platform_facts.accepts_activation == activation_policy.accepts_activation
                    && platform_facts.focus_on_click == activation_policy.focus_on_click,
                "platform did not establish the requested lifetime activation policy"
            );
        }
        let display_id = platform_facts.display_id;
        let sprite_atlas = platform_window.sprite_atlas();
        let mouse_position = platform_window.mouse_position();
        let modifiers = platform_window.modifiers();
        let capslock = platform_window.capslock();
        let content_size = platform_facts.content_size;
        let scale_factor = platform_facts.scale_factor;
        let appearance = platform_window.appearance();
        let active = Rc::new(Cell::new(platform_facts.is_active));
        let hovered = Rc::new(Cell::new(platform_window.is_hovered()));

        let platform_window = platform_window.into_platform_window();

        Ok(Window {
            handle,
            invalidator,
            removed: false,
            native_closed,
            removal_state: WindowRemovalState::Open,
            should_close_handler: WindowShouldCloseHandlerSlot::default(),
            platform_window,
            platform_command_sink,
            initial_presentation_command: Some(
                PlatformWindowCommand::CompleteInitialPresentation {
                    activate: provisional_session.is_none()
                        && show
                        && focus_on_appearing
                        && activation_policy.accepts_activation,
                },
            ),
            initial_presentation_retry: InitialPresentationRetryState::default(),
            presentation_shutdown: None,
            provisional_session,
            _provisional_opening_claim: provisional_opening_claim,
            creation_facts,
            presentation_state: WindowPresentationState::default(),
            platform_facts,
            window_kind,
            window_capabilities,
            window_mutation_authority: Arc::default(),
            window_mutations: WindowMutationState::default(),
            display_id,
            sprite_atlas,
            text_system,
            text_rendering_mode: cx.text_rendering_mode.clone(),
            rem_size: px(16.),
            rem_size_override_stack: SmallVec::new(),
            viewport_size: content_size,
            layout_engine: Some(TaffyLayoutEngine::new()),
            root: None,
            element_id_stack: SmallVec::default(),
            text_style_stack: Vec::new(),
            rendered_entity_stack: Vec::new(),
            element_offset_stack: Vec::new(),
            current_prepaint_layout_id: Rc::new(Cell::new(None)),
            subtree_presentation_stack: Rc::new(RefCell::new(SmallVec::new())),
            subtree_transform_stack: Rc::new(RefCell::new(SmallVec::new())),
            clip_stack: Rc::new(RefCell::new(SmallVec::new())),
            element_opacity: 1.0,
            requested_autoscroll: None,
            rendered_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
            next_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
            atlas_texture_lease_paint_scopes: Rc::new(RefCell::new(SmallVec::new())),
            candidate_frame_transfers: CandidateFrameTransfers::default(),
            next_candidate_frame_attempt_id: 0,
            candidate_frame_transaction: None,
            candidate_atlas_lease_failure: None,
            last_atlas_frame_rejection: None,
            candidate_pending_input_clear: false,
            candidate_pending_input_notification: false,
            retained_visual_registry: retained_visual::Registry::default(),
            #[cfg(any(test, feature = "test-support"))]
            capture_generation: Cell::new(0),
            atlas_remove_diagnostics: Vec::new(),
            next_frame_callbacks,
            next_hitbox_id: HitboxId(0),
            next_pointer_capture_id: PointerCaptureId(0),
            next_portal_anchor_id: PortalAnchorId::default(),
            portal_anchor_capture_stack: Rc::new(RefCell::new(Vec::new())),
            next_reveal_target_id: RevealTargetId::default(),
            reveal_target_capture_stack: Rc::new(RefCell::new(Vec::new())),
            scroll_ancestry_stack: Rc::new(RefCell::new(SmallVec::new())),
            next_bring_into_view_sequence: 0,
            next_bring_into_view_chain_generation: 0,
            active_bring_into_view_requests: Vec::new(),
            bring_into_view_resolutions: Vec::new(),
            next_tooltip_id: TooltipId::default(),
            tooltip_bounds: None,
            dirty_views: FxHashSet::default(),
            focus_listeners: SubscriberSet::new(),
            key_down_interceptors: SubscriberSet::new(),
            mouse_interceptors: SubscriberSet::new(),
            window_states: Rc::new(RefCell::new(TypeIdHashMap::default())),
            input_dispatch_active: Rc::new(Cell::new(false)),
            input_transaction_depth: Rc::new(Cell::new(0)),
            mouse_event_target: Rc::new(Cell::new(None)),
            focus_lost_listeners: SubscriberSet::new(),
            default_prevented: true,
            last_dispatch_event_result: None,
            mouse_position,
            mouse_in_window: hovered.get(),
            mouse_hit_test: HitTest::default(),
            modifiers,
            capslock,
            scale_factor,
            bounds_observers: SubscriberSet::new(),
            initial_presentation_observers: SubscriberSet::new(),
            appearance,
            appearance_observers: SubscriberSet::new(),
            button_layout_observers: SubscriberSet::new(),
            active,
            hovered,
            needs_present,
            input_rate_tracker,
            last_frame_time: Cell::new(None),
            #[cfg(feature = "input-latency-histogram")]
            input_latency_tracker: InputLatencyTracker::new()?,
            last_input_modality: InputModality::Mouse,
            refreshing: false,
            activation_observers: SubscriberSet::new(),
            focus: None,
            candidate_accessibility_focus: None,
            pending_focus_claim: None,
            pending_focus_reveal_fence: None,
            pending_blur_claim_generation: None,
            provisional_focus_claim: None,
            pending_focus_completion: None,
            focus_claim_resolutions: Vec::new(),
            next_focus_claim_id: 0,
            focus_claim_revision: 0,
            prepaint_commit_phase: Rc::new(Cell::new(None)),
            frame_focus_authority_sealed: false,
            focus_followup_requested: false,
            sealed_focus_retry_rejection: None,
            key_event_revision: 0,
            focus_enabled: true,
            pending_input: None,
            pending_modifier: ModifierState::default(),
            pending_input_observers: SubscriberSet::new(),
            prompt: None,
            client_inset: None,
            image_cache_stack: Vec::new(),
            pressed_mouse_buttons: PressedMouseButtons::default(),
            captured_pointer: None,
            pending_pointer_cancellations: VecDeque::new(),
            pointer_cancel_session_already_settled: false,
            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector: None,
            a11y: A11y::new(
                a11y_active_state,
                accessibility_force_disabled,
                handle.window_id(),
            ),
        })
    }

    pub(crate) fn creation_can_commit(&self) -> bool {
        !self.removed && !self.native_closed.get() && self.removal_state == WindowRemovalState::Open
    }

    pub(crate) fn bind_provisional_session(
        &self,
    ) -> std::result::Result<(), crate::WindowProvisionalSessionError> {
        if let Some(session) = self.provisional_session.as_ref() {
            session.bind(self.handle.window_id())?;
        }
        Ok(())
    }

    pub(crate) fn claim_presentation_shutdown(&mut self) -> PreparedPlatformPresentationShutdown {
        if self.presentation_shutdown.is_none() {
            self.retire_retained_visuals();
        }
        self.presentation_shutdown
            .get_or_insert_with(|| {
                self.platform_window.prepare_presentation_shutdown(
                    next_presentation_shutdown_ticket(self.handle.window_id()),
                )
            })
            .clone()
    }

    /// Arms the exact bound provisional session for reveal after its next non-empty submission.
    #[doc(hidden)]
    pub fn arm_provisional_presentation(
        &mut self,
        session: &WindowProvisionalSession,
        _cx: &mut App,
    ) -> Result<WindowProvisionalRevealTicket> {
        let owned = self
            .provisional_session
            .as_ref()
            .ok_or_else(|| anyhow!("window has no provisional presentation session"))?;
        anyhow::ensure!(
            owned.same_authority(session),
            "provisional presentation session authority does not match the window"
        );
        let snapshot = session.snapshot();
        anyhow::ensure!(
            snapshot.window_id() == Some(self.handle.window_id()),
            "provisional presentation session is not bound to this full window id"
        );
        anyhow::ensure!(
            snapshot.phase() == WindowProvisionalSessionPhase::Gated,
            "only a gated provisional session can arm presentation"
        );
        anyhow::ensure!(
            self.presentation_state.initial_presentation
                == WindowInitialPresentationStatus::Completed,
            "ordinary initial presentation must settle before provisional reveal is armed"
        );
        anyhow::ensure!(
            self.presentation_state.provisional_reveal_ticket.is_none(),
            "provisional presentation is already armed"
        );
        let ticket = WindowProvisionalRevealTicket::new(
            self.handle.window_id(),
            snapshot.generation(),
            self.rendered_frame.generation.saturating_add(1),
        );
        session.register_reveal_ticket(ticket.clone())?;
        self.presentation_state.provisional_reveal_ticket = Some(ticket.clone());
        self.refresh();
        self.platform_window.request_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        });
        Ok(ticket)
    }

    /// Atomically cancels one exact provisional reveal before a native command can win.
    #[doc(hidden)]
    pub fn cancel_provisional_presentation(
        &mut self,
        ticket: &WindowProvisionalRevealTicket,
        _cx: &mut App,
    ) -> Result<WindowProvisionalRevealCancellationOutcome> {
        let ticket_snapshot = ticket.snapshot();
        anyhow::ensure!(
            ticket_snapshot.window_id() == self.handle.window_id(),
            "provisional reveal ticket belongs to a different full window id"
        );
        let is_current = self
            .presentation_state
            .provisional_reveal_ticket
            .as_ref()
            .is_some_and(|current| current.same_authority(ticket));

        let outcome = ticket.cancel();
        if matches!(
            outcome,
            WindowProvisionalRevealCancellationOutcome::Cancelled(_)
        ) && is_current
        {
            self.presentation_state.provisional_reveal_ticket = None;
            if let Some(session) = self.provisional_session.as_ref() {
                let session_snapshot = session.snapshot();
                if session_snapshot.window_id() == Some(self.handle.window_id())
                    && session_snapshot.generation() == ticket_snapshot.session_generation()
                {
                    if let Err(error) = session.terminate(self.handle.window_id()) {
                        log::error!(
                            "failed to terminate a cancelled provisional presentation session: {error}"
                        );
                    }
                }
            }
        }
        Ok(outcome)
    }

    /// Begins projecting the destination tree while native and framework interaction remain gated.
    #[doc(hidden)]
    pub fn begin_provisional_destination_semantics(
        &mut self,
        session: &WindowProvisionalSession,
        destination_generation: u64,
        _cx: &mut App,
    ) -> Result<WindowProvisionalSemanticsTicket> {
        let owned = self
            .provisional_session
            .as_ref()
            .ok_or_else(|| anyhow!("window has no provisional presentation session"))?;
        anyhow::ensure!(
            owned.same_authority(session),
            "provisional presentation session authority does not match the window"
        );
        let snapshot = session.snapshot();
        anyhow::ensure!(
            snapshot.window_id() == Some(self.handle.window_id()),
            "provisional presentation session is not bound to this full window id"
        );
        anyhow::ensure!(
            self.creation_can_commit() && self.presentation_shutdown.is_none(),
            "a terminal window cannot project provisional destination semantics"
        );
        let reveal = self
            .presentation_state
            .provisional_reveal_ticket
            .as_ref()
            .ok_or_else(|| anyhow!("provisional presentation has not been armed"))?
            .snapshot();
        anyhow::ensure!(
            reveal.outcome() == WindowProvisionalRevealOutcome::Revealed
                && reveal
                    .native_facts()
                    .is_some_and(|facts| facts.accepts_reveal()),
            "provisional presentation has not completed its exact native reveal"
        );
        let reveal_generation = reveal.presentation_generation().ok_or_else(|| {
            anyhow!("provisional reveal has no committed presentation generation")
        })?;
        let minimum_frame_generation = self
            .rendered_frame
            .generation
            .max(reveal_generation)
            .checked_add(1)
            .ok_or_else(|| anyhow!("provisional semantics frame generation overflow"))?;
        let ticket = session.begin_destination_semantics(
            self.handle.window_id(),
            destination_generation,
            minimum_frame_generation,
        )?;
        self.refresh();
        self.platform_window.request_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        });
        Ok(ticket)
    }

    /// Accepts the exact destination-semantics marker produced by a focus-stable candidate frame.
    #[doc(hidden)]
    pub fn accept_provisional_destination_semantics_frame(
        &mut self,
        session: &WindowProvisionalSession,
        ticket: &WindowProvisionalSemanticsTicket,
        frame_generation: u64,
        _cx: &mut App,
    ) -> Result<WindowProvisionalSemanticsSnapshot> {
        anyhow::ensure!(
            self.prepaint_commit_phase.get() == Some(PrepaintCommitPhase::FocusStable),
            "destination semantics can only be accepted from a focus-stable prepaint commit"
        );
        anyhow::ensure!(
            frame_generation == self.sealed_focus_authority_frame().generation,
            "destination semantics must name the exact candidate frame"
        );
        let owned = self
            .provisional_session
            .as_ref()
            .ok_or_else(|| anyhow!("window has no provisional presentation session"))?;
        anyhow::ensure!(
            owned.same_authority(session),
            "provisional presentation session authority does not match the window"
        );
        let session_snapshot = session.snapshot();
        let ticket_snapshot = ticket.snapshot();
        anyhow::ensure!(
            session_snapshot.window_id() == Some(self.handle.window_id())
                && session_snapshot.phase()
                    == WindowProvisionalSessionPhase::ProjectingDestinationSemantics,
            "provisional session is not projecting destination semantics for this window"
        );
        anyhow::ensure!(
            ticket_snapshot.window_id() == self.handle.window_id()
                && ticket_snapshot.session_generation() == session_snapshot.generation()
                && ticket_snapshot.outcome() == WindowProvisionalSemanticsOutcome::Pending
                && frame_generation >= ticket_snapshot.minimum_frame_generation(),
            "destination semantics ticket does not admit the candidate frame"
        );
        session
            .commit_destination_semantics(self.handle.window_id(), ticket, frame_generation)
            .map_err(|error| anyhow!(error))?;
        Ok(ticket.snapshot())
    }

    /// Admits interaction after the exact destination-semantics frame has committed.
    #[doc(hidden)]
    pub fn admit_provisional_interaction(
        &mut self,
        session: &WindowProvisionalSession,
        ticket: &WindowProvisionalSemanticsTicket,
        _cx: &mut App,
    ) -> Result<()> {
        let owned = self
            .provisional_session
            .as_ref()
            .ok_or_else(|| anyhow!("window has no provisional presentation session"))?;
        anyhow::ensure!(
            owned.same_authority(session),
            "provisional presentation session authority does not match the window"
        );
        anyhow::ensure!(
            session.snapshot().window_id() == Some(self.handle.window_id()),
            "provisional presentation session is not bound to this full window id"
        );
        anyhow::ensure!(
            self.creation_can_commit() && self.presentation_shutdown.is_none(),
            "a terminal window cannot admit provisional interaction"
        );
        session.admit_interaction(self.handle.window_id(), ticket)?;
        Ok(())
    }

    fn provisional_session_accepts_interaction(&self) -> bool {
        self.provisional_session.as_ref().is_none_or(|session| {
            let snapshot = session.snapshot();
            snapshot.window_id() == Some(self.handle.window_id()) && snapshot.accepts_interaction()
        })
    }

    fn provisional_session_projects_destination_semantics(&self) -> bool {
        self.provisional_session.as_ref().is_none_or(|session| {
            let snapshot = session.snapshot();
            snapshot.window_id() == Some(self.handle.window_id())
                && snapshot.projects_destination_semantics()
        })
    }

    fn presentation_is_allowed(&self) -> bool {
        if self.presentation_shutdown.is_some() {
            return false;
        }
        self.provisional_session.as_ref().is_none_or(|session| {
            !matches!(
                session.snapshot().phase(),
                WindowProvisionalSessionPhase::Terminal
            )
        })
    }

    pub(crate) fn prepare_initial_presentation(&mut self) -> Result<()> {
        if self.window_capabilities.creation.initial_presentation_order
            != WindowInitialPresentationOrder::BeforeVisibility
        {
            return Ok(());
        }
        if self.presentation_state.frame_accepted_generation != Some(self.rendered_frame.generation)
        {
            let minimum_generation = self.last_atlas_frame_rejection.map_or_else(
                || self.rendered_frame.generation.saturating_add(1),
                |rejection| rejection.generation,
            );
            self.initial_presentation_retry = InitialPresentationRetryState {
                minimum_generation: Some(minimum_generation),
                attempts_started: 0,
                presentation_retry_generation: None,
                presentation_retries_started: 0,
            };
            self.request_fresh_initial_presentation_frame();
            return Ok(());
        }
        anyhow::ensure!(
            !self.platform_window.is_visible(),
            "platform window became visible before its first presentation"
        );
        let outcome = self.present();
        if outcome == PlatformWindowPresentOutcome::RepaintRequired {
            self.initial_presentation_retry.minimum_generation =
                Some(self.rendered_frame.generation.saturating_add(1));
            anyhow::ensure!(
                !self.platform_window.is_visible(),
                "platform window became visible during its hidden first presentation"
            );
            self.request_fresh_initial_presentation_frame();
            return Ok(());
        }
        anyhow::ensure!(
            outcome == PlatformWindowPresentOutcome::Submitted,
            "platform rejected or deferred the initial frame submission"
        );
        anyhow::ensure!(
            !self.platform_window.is_visible(),
            "platform window became visible during its hidden first presentation"
        );
        Ok(())
    }

    fn request_fresh_initial_presentation_frame(&mut self) {
        self.refresh();
        self.platform_window.request_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
        });
    }

    fn request_initial_presentation_retry(&self) {
        self.platform_window.request_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        });
    }

    fn renderer_repaint_is_pending(&self) -> bool {
        self.presentation_state
            .renderer_invalidated_generation
            .is_some_and(|generation| self.rendered_frame.generation <= generation)
    }

    fn request_renderer_repaint_frame(&mut self) {
        self.refresh();
        self.platform_window.request_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
        });
    }

    fn begin_fresh_initial_presentation_attempt(&mut self, cx: &mut App) -> bool {
        if !self.fresh_initial_presentation_is_pending() {
            return true;
        }
        if self.fresh_initial_presentation_attempts_exhausted() {
            self.fail_fresh_initial_presentation(cx);
            return false;
        }
        self.initial_presentation_retry.attempts_started += 1;
        true
    }

    fn finish_fresh_initial_presentation_if_ready(&mut self, cx: &mut App) {
        let Some(minimum_generation) = self.initial_presentation_retry.minimum_generation else {
            return;
        };
        let Some(generation) = self.presentation_state.frame_accepted_generation else {
            return;
        };
        if generation < minimum_generation
            || generation != self.rendered_frame.generation
            || self.presentation_state.present_submitted_generation != Some(generation)
            || self.presentation_state.non_empty_presented_generation != Some(generation)
        {
            return;
        }

        let Some(command) = self.initial_presentation_command.take() else {
            self.fail_fresh_initial_presentation(cx);
            return;
        };
        self.initial_presentation_retry = InitialPresentationRetryState::default();
        self.platform_command_sink.enqueue(command);
    }

    fn fresh_initial_presentation_is_pending(&self) -> bool {
        self.initial_presentation_retry.minimum_generation.is_some()
    }

    fn fresh_initial_presentation_frame_is_required(&self) -> bool {
        let Some(minimum_generation) = self.initial_presentation_retry.minimum_generation else {
            return false;
        };
        let generation = self.rendered_frame.generation;
        if self.presentation_state.frame_accepted_generation != Some(generation)
            || generation < minimum_generation
            || self.renderer_repaint_is_pending()
        {
            return true;
        }

        self.presentation_state
            .latest_present_attempt
            .filter(|attempt| attempt.generation == generation)
            .is_some_and(|attempt| match attempt.outcome {
                PlatformWindowPresentOutcome::Deferred => false,
                PlatformWindowPresentOutcome::Submitted => {
                    self.presentation_state.non_empty_presented_generation != Some(generation)
                }
                PlatformWindowPresentOutcome::RepaintRequired
                | PlatformWindowPresentOutcome::Rejected => true,
            })
    }

    fn fresh_initial_presentation_attempts_exhausted(&self) -> bool {
        self.fresh_initial_presentation_is_pending()
            && self.initial_presentation_retry.attempts_started
                >= FRESH_INITIAL_PRESENTATION_ATTEMPT_LIMIT
    }

    fn begin_initial_presentation_retry(&mut self, cx: &mut App) -> bool {
        let generation = self.rendered_frame.generation;
        if self
            .initial_presentation_retry
            .presentation_retry_generation
            != Some(generation)
        {
            self.initial_presentation_retry
                .presentation_retry_generation = Some(generation);
            self.initial_presentation_retry.presentation_retries_started = 0;
        }
        if self.initial_presentation_retries_exhausted() {
            self.fail_fresh_initial_presentation(cx);
            return false;
        }
        self.initial_presentation_retry.presentation_retries_started += 1;
        true
    }

    fn initial_presentation_retries_exhausted(&self) -> bool {
        self.initial_presentation_retry
            .presentation_retry_generation
            == Some(self.rendered_frame.generation)
            && self.initial_presentation_retry.presentation_retries_started
                >= INITIAL_PRESENTATION_RETRY_LIMIT
    }

    fn fresh_initial_presentation_is_deferred_retry(&self) -> bool {
        let generation = self.rendered_frame.generation;
        self.presentation_state
            .latest_present_attempt
            .is_some_and(|attempt| {
                attempt.generation == generation
                    && attempt.outcome == PlatformWindowPresentOutcome::Deferred
            })
    }

    fn fail_fresh_initial_presentation(&mut self, cx: &mut App) {
        if !self.fresh_initial_presentation_is_pending() {
            return;
        }
        let attempts = self.initial_presentation_retry.attempts_started;
        let presentation_retries = self.initial_presentation_retry.presentation_retries_started;
        let minimum_generation = self.initial_presentation_retry.minimum_generation;
        self.initial_presentation_retry = InitialPresentationRetryState::default();
        self.initial_presentation_command.take();
        log::error!(
            target: "open_gpui::presentation",
            "closing a hidden window after {attempts} fresh initial-presentation attempts and {presentation_retries} same-generation retries could not reach required generation {minimum_generation:?}"
        );
        let failure_notification = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.initial_presentation_failed(cx)
        }));
        let removal =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.remove_window(cx)));
        finish_after_window_cleanup(
            failure_notification,
            removal,
            "fresh initial-presentation failure",
        );
    }

    pub(crate) fn take_initial_presentation_command(
        &mut self,
    ) -> Option<(PlatformWindowCommandSink, PlatformWindowCommand)> {
        if self.fresh_initial_presentation_is_pending() {
            return None;
        }
        self.initial_presentation_command
            .take()
            .map(|command| (self.platform_command_sink.clone(), command))
    }

    pub(crate) fn initial_presentation_completed(&mut self, cx: &mut App) {
        if self.presentation_state.initial_presentation
            == WindowInitialPresentationStatus::Completed
        {
            return;
        }
        self.presentation_state.initial_presentation = WindowInitialPresentationStatus::Completed;
        let previous_facts = self.platform_facts.clone();
        self.refresh_platform_facts();
        let tab_bar_visible = self.platform_window.tab_bar_visible();
        SystemWindowTabController::init_visible(cx, tab_bar_visible);
        let tabs = self.platform_window.tabbed_windows();
        let tab_presentation_changed = tab_bar_visible
            || tabs
                .as_ref()
                .is_some_and(|tabs| tabs.iter().any(|tab| tab.id == self.handle.window_id()));
        if let Some(tabs) = tabs {
            SystemWindowTabController::add_tab(cx, self.handle.window_id(), tabs);
        }

        let bounds_changed = self.platform_facts.bounds != previous_facts.bounds
            || self.platform_facts.coordinate_space != previous_facts.coordinate_space
            || self.platform_facts.window_bounds != previous_facts.window_bounds
            || self.platform_facts.inner_window_bounds != previous_facts.inner_window_bounds
            || self.platform_facts.content_size != previous_facts.content_size
            || self.platform_facts.scale_factor != previous_facts.scale_factor
            || self.platform_facts.display_id != previous_facts.display_id;
        if self.platform_facts != previous_facts || tab_presentation_changed {
            self.refresh();
        }
        if bounds_changed {
            self.notify_bounds_observers(cx);
        }
        self.notify_initial_presentation_observers(cx);
    }

    pub(crate) fn initial_presentation_failed(&mut self, cx: &mut App) {
        if self.presentation_state.initial_presentation == WindowInitialPresentationStatus::Rejected
        {
            return;
        }
        self.presentation_state.initial_presentation = WindowInitialPresentationStatus::Rejected;
        self.refresh();
        self.notify_initial_presentation_observers(cx);
    }

    fn notify_initial_presentation_observers(&mut self, cx: &mut App) {
        self.initial_presentation_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    pub(crate) fn new_focus_listener(
        &self,
        value: AnyWindowFocusListener,
    ) -> (Subscription, impl FnOnce() + use<>) {
        self.focus_listeners.insert((), value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(missing_docs)]
pub struct DispatchEventResult {
    pub propagate: bool,
    pub default_prevented: bool,
}

impl Default for DispatchEventResult {
    fn default() -> Self {
        Self {
            propagate: true,
            default_prevented: false,
        }
    }
}

impl Window {
    fn mark_view_dirty(&mut self, view_id: EntityId) {
        // Mark ancestor views as dirty. If already in the `dirty_views` set, then all its ancestors
        // should already be dirty.
        for view_id in self
            .rendered_frame
            .dispatch_tree
            .view_path_reversed(view_id)
        {
            if !self.dirty_views.insert(view_id) {
                break;
            }
        }
    }

    /// Registers a callback to be invoked when the window appearance changes.
    pub fn observe_window_appearance(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.appearance_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Registers a callback for the terminal initial-presentation result of this window.
    ///
    /// The subscription is owned by the window rather than its current root entity, so replacing
    /// the root cannot detach lifecycle-critical presentation handling.
    pub fn observe_window_initial_presentation(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.initial_presentation_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Registers a callback to be invoked when the window button layout changes.
    pub fn observe_button_layout_changed(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.button_layout_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Replaces the root entity of the window with a new one.
    pub fn replace_root<E>(
        &mut self,
        cx: &mut App,
        build_view: impl FnOnce(&mut Window, &mut Context<E>) -> E,
    ) -> Entity<E>
    where
        E: 'static + Render,
    {
        let view = cx.new(|cx| build_view(self, cx));
        self.root = Some(view.clone().into());
        self.refresh();
        view
    }

    /// Returns the root entity of the window, if it has one.
    pub fn root<E>(&self) -> Option<Option<Entity<E>>>
    where
        E: 'static + Render,
    {
        self.root
            .as_ref()
            .map(|view| view.clone().downcast::<E>().ok())
    }

    /// Obtain a handle to the window that belongs to this context.
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.handle
    }

    /// Returns the state entity uniquely owned by this window for its concrete type.
    ///
    /// The initializer runs at most once per type and window. The state is independent from
    /// element identity and remains alive across frames until the window is dropped.
    pub fn use_window_state<S: 'static>(
        &mut self,
        cx: &mut App,
        initialize: impl FnOnce(&mut Window, &mut Context<S>) -> S,
    ) -> Entity<S> {
        let state_type = TypeId::of::<S>();
        {
            let states = self.window_states.borrow();
            match states.get(&state_type) {
                Some(WindowStateSlot::Ready(state)) => {
                    return state.clone().downcast::<S>().unwrap_or_else(|_| {
                        panic!("window state type id did not match its entity")
                    });
                }
                Some(WindowStateSlot::Initializing { type_name, .. }) => {
                    panic!(
                        "window state `{type_name}` recursively requested itself while initializing"
                    )
                }
                None => {}
            }
        }

        let token = Rc::new(());
        self.window_states.borrow_mut().insert(
            state_type,
            WindowStateSlot::Initializing {
                type_name: std::any::type_name::<S>(),
                token: token.clone(),
            },
        );
        let initialization = WindowStateInitializationGuard {
            slots: self.window_states.clone(),
            state_type,
            token,
        };
        let state = cx.new(|cx| initialize(self, cx));
        self.window_states
            .borrow_mut()
            .insert(state_type, WindowStateSlot::Ready(state.clone().into()));
        drop(initialization);
        state
    }

    /// Returns an already initialized state entity owned by this window.
    ///
    /// Unlike [`Window::use_window_state`], this query never initializes or mutates window state.
    /// It returns `None` when the state is absent or still being initialized.
    pub fn window_state<S: 'static>(&self) -> Option<Entity<S>> {
        let state_type = TypeId::of::<S>();
        let states = self.window_states.borrow();
        let WindowStateSlot::Ready(state) = states.get(&state_type)? else {
            return None;
        };
        Some(
            state
                .clone()
                .downcast::<S>()
                .unwrap_or_else(|_| panic!("window state type id did not match its entity")),
        )
    }

    /// Mark the window as dirty, scheduling it to be redrawn on the next frame.
    pub fn refresh(&mut self) {
        if self.invalidator.can_schedule_refresh() {
            self.refreshing = true;
            self.invalidator.set_dirty(true);
        }
    }

    fn refresh_focus_authority(&mut self) {
        let needs_effect_wakeup =
            self.frame_focus_authority_sealed || self.invalidator.is_focus_phase();
        if needs_effect_wakeup {
            // The current frame already sealed its input authority. Stage the request until every
            // focus listener has run, then leave one candidate frame for the platform scheduler.
            self.focus_followup_requested = true;
        } else {
            self.refresh();
        }
    }

    fn focus_followup_frame_needed(&self) -> bool {
        self.pending_focus_claim.is_some()
            || self.pending_blur_claim_generation.is_some()
            || self.focus != self.rendered_frame.focus_path().last().copied()
    }

    fn reconcile_focus_followup_refresh(&mut self) {
        if !self.focus_followup_frame_needed() && self.invalidator.clear_focus_only_dirty() {
            self.refreshing = false;
        }
    }

    #[cfg(test)]
    pub(crate) fn refresh_pending_for_test(&self) -> bool {
        self.invalidator.is_dirty()
    }

    /// Replaces the window atlas for deterministic candidate-frame tests.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn set_sprite_atlas_for_test(&mut self, atlas: Arc<dyn PlatformAtlas>) {
        self.sprite_atlas = atlas;
    }

    /// Runs an element subtree while bypassing cached-view journal reuse when requested.
    ///
    /// Wrapper elements use this when an inherited render input changed independently of the
    /// child view entity. The previous cache-refresh state is restored even if the subtree panics.
    pub fn with_cached_view_refresh<R>(
        &mut self,
        refresh: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if !refresh {
            return f(self);
        }

        let previous = std::mem::replace(&mut self.refreshing, true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.refreshing = previous;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Close this window.
    pub fn remove_window(&mut self, cx: &mut App) {
        if self.removed || self.removal_state != WindowRemovalState::Open {
            return;
        }
        let removal_state = if self.input_transaction_depth.get() > 0 {
            WindowRemovalState::PendingAfterInput
        } else {
            WindowRemovalState::Removing
        };
        self.claim_presentation_shutdown();
        self.removal_state = removal_state;
        let preparation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.invalidate_platform_window_mutations();
            let deliveries = self.window_mutations.settle_all(
                &self.window_mutation_authority,
                WindowMutationOutcome::WindowClosed,
                &self.platform_facts,
            );
            let ticket_delivery =
                Self::deliver_window_mutation_ticket_deliveries_panic_safe(deliveries);
            self.a11y.clear_announcements_for_window_close();
            if let Err(payload) = ticket_delivery {
                std::panic::resume_unwind(payload);
            }
        }));
        if self.input_transaction_depth.get() > 0 {
            if let Err(payload) = preparation {
                std::panic::resume_unwind(payload);
            }
            return;
        }

        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.finish_remove_window(cx)
        }));
        finish_after_window_cleanup(preparation, cleanup, "direct window removal");
    }

    fn finish_pending_window_removal(&mut self, cx: &mut App) {
        if self.removal_state == WindowRemovalState::PendingAfterInput
            && self.input_transaction_depth.get() == 0
        {
            debug_assert!(
                !self.input_dispatch_active.get(),
                "window removal must commit after the input dispatch guard is released"
            );
            self.finish_remove_window(cx);
        }
    }

    pub(crate) fn with_input_transaction<R>(
        &mut self,
        cx: &mut App,
        callback: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        let transaction = InputTransactionGuard::enter(self.input_transaction_depth.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(self, cx)));
        drop(transaction);
        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.finish_pending_window_removal(cx)
        }));
        finish_after_window_cleanup(result, cleanup, "input transaction window removal")
    }

    fn finish_remove_window(&mut self, cx: &mut App) {
        self.removal_state = WindowRemovalState::Removing;
        self.should_close_handler.terminate();
        let mut first_panic = None;

        if let Some(session) = self.provisional_session.as_ref() {
            let snapshot = session.snapshot();
            if snapshot.window_id() == Some(self.handle.window_id())
                && snapshot.phase() != WindowProvisionalSessionPhase::Terminal
            {
                let _ = session.terminate(self.handle.window_id());
            }
        }
        if let Some(ticket) = self.presentation_state.provisional_reveal_ticket.as_ref() {
            ticket.settle(WindowProvisionalRevealOutcome::WindowTerminal);
        }

        retain_first_window_cleanup_panic(
            &mut first_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.invalidate_platform_window_mutations();
            })),
            "platform mutation invalidation",
        );

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.window_mutations.settle_all(
                &self.window_mutation_authority,
                WindowMutationOutcome::WindowClosed,
                &self.platform_facts,
            )
        })) {
            Ok(deliveries) => {
                for delivery in deliveries {
                    retain_first_window_cleanup_panic(
                        &mut first_panic,
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            delivery.deliver();
                        })),
                        "window mutation ticket delivery",
                    );
                }
            }
            Err(payload) => retain_first_window_cleanup_panic(
                &mut first_panic,
                Err(payload),
                "window mutation settlement",
            ),
        }

        retain_first_window_cleanup_panic(
            &mut first_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.cancel_pointer_session(PointerCancelReason::WindowClosed, cx);
            })),
            "pointer-session cancellation",
        );
        retain_first_window_cleanup_panic(
            &mut first_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.clear_pointer_session(PointerCancelReason::WindowClosed, cx);
            })),
            "pointer-session terminal cleanup",
        );
        retain_first_window_cleanup_panic(
            &mut first_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.a11y.clear_announcements_for_window_close();
            })),
            "accessibility announcement cleanup",
        );
        retain_first_window_cleanup_panic(
            &mut first_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.close_bring_into_view_authority(cx);
            })),
            "bring-into-view terminal cleanup",
        );
        self.pending_focus_reveal_fence = None;
        self.pending_focus_completion = None;
        self.focus_claim_resolutions.clear();
        self.removed = true;
        if let Some(payload) = first_panic {
            std::panic::resume_unwind(payload);
        }
    }

    pub(crate) fn should_close(&mut self, cx: &mut App) -> bool {
        self.should_close_handler.clone().invoke(self, cx)
    }

    fn invalidate_platform_window_mutations(&self) {
        for domain in WindowMutationDomain::ALL {
            self.platform_window.invalidate_window_mutation(domain);
        }
    }

    /// Obtain the current requested [`FocusHandle`].
    ///
    /// During a candidate render this may expose provisional focus intent before it commits. Use
    /// [`Self::committed_focus`] for the last committed window-local focus leaf, or a typed
    /// completion when a retained transaction must know whether its own request committed.
    pub fn focused(&self, cx: &App) -> Option<FocusHandle> {
        self.focus
            .and_then(|id| FocusHandle::for_id(id, &cx.focus_handles))
    }

    /// Obtain the exact focus leaf from the last committed window-local focus tree.
    ///
    /// This is independent of platform-window activation and does not expose provisional focus
    /// intent from the candidate frame.
    pub fn committed_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.rendered_frame
            .focus_path()
            .last()
            .copied()
            .and_then(|id| FocusHandle::for_id(id, &cx.focus_handles))
    }

    /// Returns an opaque revision that changes whenever element focus is explicitly claimed.
    ///
    /// Reasserting the currently focused handle and explicitly clearing an already-empty focus
    /// both advance this revision. Deferred focus arbitration can therefore observe newer intent,
    /// not only a different final focus value.
    pub const fn focus_claim_revision(&self) -> u64 {
        self.focus_claim_revision
    }

    #[cfg(test)]
    pub(crate) fn retained_focus_claim_count_for_test(&self) -> usize {
        if self.pending_focus_claim.is_some() || self.pending_blur_claim_generation.is_some() {
            1
        } else {
            0
        }
    }

    /// Returns an opaque revision that advances before each key-down or key-up dispatch.
    ///
    /// The revision advances even when an interceptor or capture listener stops the event, so
    /// consumers can reject a stale key transaction without observing the stopped event itself.
    pub const fn key_event_revision(&self) -> u64 {
        self.key_event_revision
    }

    /// Returns an opaque revision for the most recently completed rendered frame.
    pub const fn rendered_frame_revision(&self) -> u64 {
        self.rendered_frame.generation
    }

    /// Returns whether a focus handle belongs to the most recently rendered dispatch tree.
    pub fn is_focus_handle_rendered(&self, handle: &FocusHandle) -> bool {
        self.rendered_frame
            .dispatch_tree
            .focusable_node_id(handle.id)
            .is_some()
    }

    /// Returns the first live tab stop contained by a rendered focus scope root.
    pub fn first_tab_stop_within(&self, scope: &FocusHandle) -> Option<FocusHandle> {
        self.first_tab_stop_where_within(scope, |_| true)
    }

    /// Returns the current rendered tab stops contained by a focus scope in traversal order.
    pub fn tab_stops_within(&self, scope: &FocusHandle) -> Vec<FocusHandle> {
        if !self.is_focus_handle_rendered(scope) {
            return Vec::new();
        }

        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let predicate =
            |candidate: &FocusHandle| dispatch_tree.focus_contains(scope.id, candidate.id);
        let mut result = Vec::new();
        let mut current = None;
        while let Some(candidate) = self
            .rendered_frame
            .tab_stops
            .next_where(current.as_ref(), predicate)
        {
            if result.contains(&candidate) {
                break;
            }
            current = Some(candidate.id);
            result.push(candidate);
        }
        result
    }

    /// Returns the first live tab stop contained by a rendered focus scope root that matches a
    /// renderer-adapter predicate.
    pub fn first_tab_stop_where_within(
        &self,
        scope: &FocusHandle,
        predicate: impl Fn(&FocusHandle) -> bool,
    ) -> Option<FocusHandle> {
        if !self.is_focus_handle_rendered(scope) {
            return None;
        }

        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        self.rendered_frame.tab_stops.next_where(None, |candidate| {
            dispatch_tree.focus_contains(scope.id, candidate.id) && predicate(candidate)
        })
    }

    /// Move focus to the element associated with the given [`FocusHandle`].
    pub fn focus(&mut self, handle: &FocusHandle, cx: &mut App) {
        let _ = self.focus_impl(handle, None, None, cx);
    }

    /// Move focus and observe the terminal result of this specific request.
    ///
    /// The callback runs at most once and only after the request either appears in the committed
    /// local focus tree, fails its candidate render generation, or is replaced by another focus
    /// mutation. Dropping the returned subscription cancels callback observation without
    /// cancelling the focus request.
    pub fn focus_with_completion(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        listener: impl FnOnce(FocusClaimOutcome, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let callback = Rc::new(RefCell::new(Some(
            Box::new(listener) as AnyFocusClaimCompletion
        )));
        let cancelled_callback = Rc::downgrade(&callback);
        let subscription = Subscription::new(move || {
            if let Some(callback) = cancelled_callback.upgrade() {
                callback.borrow_mut().take();
            }
        });
        self.next_focus_claim_id = self.next_focus_claim_id.wrapping_add(1).max(1);
        let completion = PendingFocusCompletion {
            id: self.next_focus_claim_id,
            target: FocusClaimTarget::Exact(handle.id),
            target_generation: 0,
            callback,
        };
        self.record_candidate_focus_completion(&completion);
        let _ = self.focus_impl(handle, Some(completion), None, cx);
        subscription
    }

    /// Moves focus with completion while preserving a prior scroll-input boundary for automatic
    /// focus reveal.
    ///
    /// The focus claim follows ordinary arbitration. If the fence was interrupted or no longer
    /// matches the committed focus target's scroll ancestry when that claim commits, focus still
    /// settles normally but GPUI does not enqueue the implicit physical reveal.
    pub fn focus_with_completion_and_scroll_fence(
        &mut self,
        handle: &FocusHandle,
        fence: ScrollChainFence,
        cx: &mut App,
        listener: impl FnOnce(FocusClaimOutcome, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let callback = Rc::new(RefCell::new(Some(
            Box::new(listener) as AnyFocusClaimCompletion
        )));
        let cancelled_callback = Rc::downgrade(&callback);
        let subscription = Subscription::new(move || {
            if let Some(callback) = cancelled_callback.upgrade() {
                callback.borrow_mut().take();
            }
        });
        self.next_focus_claim_id = self.next_focus_claim_id.wrapping_add(1).max(1);
        let completion = PendingFocusCompletion {
            id: self.next_focus_claim_id,
            target: FocusClaimTarget::Exact(handle.id),
            target_generation: 0,
            callback,
        };
        self.record_candidate_focus_completion(&completion);
        let _ = self.focus_impl(handle, Some(completion), Some(fence), cx);
        subscription
    }

    fn focus_impl(
        &mut self,
        handle: &FocusHandle,
        completion: Option<PendingFocusCompletion>,
        reveal_fence: Option<ScrollChainFence>,
        cx: &mut App,
    ) -> bool {
        if !self.focus_mutations_enabled() {
            if let Some(completion) = completion {
                self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Rejected);
                self.schedule_focus_claim_resolution_dispatch(cx);
            }
            return false;
        }

        if self.frame_focus_authority_sealed
            && self.sealed_focus_retry_rejection == Some(FocusClaimTarget::Exact(handle.id))
        {
            if let Some(completion) = completion {
                self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Rejected);
                self.schedule_focus_claim_resolution_dispatch(cx);
            }
            return false;
        }

        self.supersede_pending_focus_completion();
        self.restore_provisional_focus_claim();
        self.focus_claim_revision = self.focus_claim_revision.wrapping_add(1);
        self.pending_focus_claim = None;
        self.pending_focus_reveal_fence = None;
        self.pending_blur_claim_generation = None;

        if self.frame_focus_authority_sealed {
            let (accepted_generation, already_committed) = {
                let accepted_frame = self.sealed_focus_authority_frame();
                (
                    accepted_frame.generation,
                    self.focus == Some(handle.id)
                        && accepted_frame.focus == Some(handle.id)
                        && accepted_frame
                            .dispatch_tree
                            .valid_focusable_node_id(handle.id)
                            .is_some(),
                )
            };
            if already_committed {
                if let Some(completion) = completion {
                    self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Committed);
                }
                self.schedule_focus_claim_resolution_dispatch(cx);
                return true;
            }

            self.pending_focus_reveal_fence = reveal_fence.map(|fence| PendingFocusRevealFence {
                target: handle.id,
                fence,
            });
            let target_generation = accepted_generation.saturating_add(1);
            self.pending_focus_claim = Some(PendingFocusClaim {
                target: handle.id,
                target_generation,
            });
            if let Some(mut completion) = completion {
                completion.target_generation = target_generation;
                self.pending_focus_completion = Some(completion);
            }
            self.refresh_focus_authority();
            self.schedule_focus_claim_resolution_dispatch(cx);
            return true;
        }

        let candidate_frame_in_progress =
            self.next_frame.generation == self.rendered_frame.generation.saturating_add(1);
        if !candidate_frame_in_progress
            && self.focus == Some(handle.id)
            && self.rendered_frame.focus_path().last() == Some(&handle.id)
        {
            if let Some(completion) = completion {
                self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Committed);
            }
            self.schedule_focus_claim_resolution_dispatch(cx);
            self.reconcile_focus_followup_refresh();
            return true;
        }

        self.pending_focus_reveal_fence = reveal_fence.map(|fence| PendingFocusRevealFence {
            target: handle.id,
            fence,
        });
        let bound_in_rendered_frame = !candidate_frame_in_progress
            && self
                .rendered_frame
                .dispatch_tree
                .focusable_node_id(handle.id)
                .is_some();
        let bound_in_candidate_frame = self.candidate_frame_contains_focus(handle.id);
        let target_generation = if candidate_frame_in_progress {
            self.next_frame.generation
        } else {
            self.rendered_frame.generation.saturating_add(1)
        };
        if let Some(mut completion) = completion {
            completion.target_generation = target_generation;
            self.pending_focus_completion = Some(completion);
        }
        if !bound_in_rendered_frame && !bound_in_candidate_frame {
            self.pending_focus_claim = Some(PendingFocusClaim {
                target: handle.id,
                target_generation,
            });
            self.refresh_focus_authority();
            self.schedule_focus_claim_resolution_dispatch(cx);
            return true;
        }

        self.commit_focus(handle.id, bound_in_candidate_frame, cx);
        self.schedule_focus_claim_resolution_dispatch(cx);
        true
    }

    fn supersede_pending_focus_completion(&mut self) {
        if let Some(completion) = self.pending_focus_completion.take() {
            self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Superseded);
        }
    }

    fn record_candidate_focus_completion(&mut self, completion: &PendingFocusCompletion) {
        if let Some(candidate_frame) = self.candidate_frame_transaction.as_mut() {
            candidate_frame.record_focus_completion(completion.id);
        }
    }

    fn focus_mutations_enabled(&self) -> bool {
        self.focus_enabled
            && self.provisional_session_accepts_interaction()
            && self.subtree_presentation().is_interactive()
            && self.prepaint_commit_phase.get() != Some(PrepaintCommitPhase::FocusStable)
    }

    fn sealed_focus_authority_frame(&self) -> &Frame {
        debug_assert!(self.frame_focus_authority_sealed);
        let candidate_frame = self
            .candidate_frame_transaction
            .as_ref()
            .expect("sealed focus authority must belong to one candidate frame transaction");
        if candidate_frame.is_accepted() {
            &self.rendered_frame
        } else {
            &self.next_frame
        }
    }

    fn queue_focus_claim_resolution(
        &mut self,
        completion: PendingFocusCompletion,
        outcome: FocusClaimOutcome,
    ) {
        debug_assert!(
            self.focus_claim_resolutions
                .iter()
                .all(|resolution| resolution.id != completion.id),
            "a focus claim must have exactly one terminal result"
        );
        self.focus_claim_resolutions.push(FocusClaimResolution {
            id: completion.id,
            outcome,
            callback: completion.callback,
        });
    }

    fn schedule_focus_claim_resolution_dispatch(&self, cx: &mut App) {
        if self.focus_claim_resolutions.is_empty() {
            return;
        }

        let window_handle = self.handle;
        cx.spawn(async move |cx| {
            cx.update_window_when_available(window_handle, |_, window, cx| {
                window.dispatch_focus_claim_resolutions(cx);
            })
            .await
            .ok();
        })
        .detach();
    }

    fn dispatch_focus_claim_resolutions(&mut self, cx: &mut App) {
        let resolutions = mem::take(&mut self.focus_claim_resolutions);
        for resolution in resolutions {
            if self.removal_state != WindowRemovalState::Open {
                break;
            }
            let callback = resolution.callback.borrow_mut().take();
            if let Some(callback) = callback {
                callback(resolution.outcome, self, cx);
            }
        }
    }

    fn candidate_frame_contains_focus(&self, focus: FocusId) -> bool {
        self.next_frame.generation == self.rendered_frame.generation.saturating_add(1)
            && self
                .next_frame
                .dispatch_tree
                .focusable_node_id(focus)
                .is_some()
    }

    fn commit_focus(&mut self, focus: FocusId, bind_candidate_frame: bool, cx: &mut App) {
        if bind_candidate_frame {
            self.next_frame.focus = Some(focus);
            if self.focus != Some(focus) {
                self.provisional_focus_claim = Some(ProvisionalFocusClaim {
                    target: focus,
                    fallback: self.focus,
                });
                self.focus = Some(focus);
            }
            return;
        }
        if self.focus == Some(focus) {
            return;
        }

        self.focus = Some(focus);
        self.clear_pending_keystrokes();
        self.defer_pending_input_changed(cx);
        self.refresh_focus_authority();
    }

    fn take_pending_focus_reveal_fence(
        &mut self,
        committed_focus: Option<FocusId>,
    ) -> Option<ScrollChainFence> {
        if self
            .pending_focus_reveal_fence
            .as_ref()
            .is_none_or(|pending| committed_focus != Some(pending.target))
        {
            return None;
        }
        self.pending_focus_reveal_fence
            .take()
            .map(|pending| pending.fence)
    }

    fn defer_pending_input_changed(&mut self, cx: &mut App) {
        if self.invalidator.is_building_frame() {
            self.candidate_pending_input_notification = true;
            return;
        }

        // Avoid re-entrant entity updates by deferring observer notifications to the end of the
        // current effect cycle, and only for this window.
        let window_handle = self.handle;
        cx.defer(move |cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    window.pending_input_changed(cx);
                })
                .ok();
        });
    }

    fn promote_pending_focus_claim(&mut self) {
        let Some(claim) = self.pending_focus_claim else {
            return;
        };
        if claim.target_generation != self.next_frame.generation
            || !self.candidate_frame_contains_focus(claim.target)
        {
            return;
        }

        self.next_frame.focus = Some(claim.target);
        if self.focus != Some(claim.target) {
            self.provisional_focus_claim = Some(ProvisionalFocusClaim {
                target: claim.target,
                fallback: self.focus,
            });
            self.focus = Some(claim.target);
        }
    }

    fn restore_provisional_focus_claim(&mut self) {
        let Some(claim) = self.provisional_focus_claim.take() else {
            return;
        };
        if self.focus == Some(claim.target) {
            self.focus = claim.fallback;
            if self.next_frame.generation == self.rendered_frame.generation.saturating_add(1) {
                self.next_frame.focus = claim.fallback;
            }
        }
    }

    fn resolve_provisional_focus_claim(&mut self, cx: &mut App) {
        let Some(claim) = self.provisional_focus_claim.take() else {
            return;
        };
        if self.focus != Some(claim.target) {
            return;
        }

        if self
            .next_frame
            .dispatch_tree
            .valid_focusable_node_id(claim.target)
            .is_some()
        {
            self.next_frame.focus = Some(claim.target);
            self.clear_pending_keystrokes();
            self.defer_pending_input_changed(cx);
        } else {
            let fallback = claim.fallback.filter(|focus| {
                self.next_frame
                    .dispatch_tree
                    .valid_focusable_node_id(*focus)
                    .is_some()
            });
            self.focus = fallback;
            self.next_frame.focus = fallback;
            if fallback != claim.fallback {
                self.clear_pending_keystrokes();
                self.defer_pending_input_changed(cx);
            }
        }
    }

    fn discard_resolved_candidate_focus_claim(&mut self, accepted_generation: u64) {
        if self
            .pending_focus_claim
            .is_some_and(|claim| claim.target_generation <= accepted_generation)
        {
            self.pending_focus_claim = None;
        }
        if self
            .pending_blur_claim_generation
            .is_some_and(|generation| generation <= accepted_generation)
        {
            self.pending_blur_claim_generation = None;
        }
    }

    fn settle_focus_claim_for_candidate_generation(&mut self) -> Option<FocusClaimTarget> {
        let generation = self.next_frame.generation;
        let due_claim = self
            .pending_focus_claim
            .filter(|claim| claim.target_generation <= generation);
        let due_blur = self
            .pending_blur_claim_generation
            .filter(|target_generation| *target_generation <= generation);
        let due_completion_target = self
            .pending_focus_completion
            .as_ref()
            .filter(|completion| completion.target_generation <= generation)
            .map(|completion| completion.target);
        debug_assert!(
            due_claim.is_none()
                || due_completion_target.is_none()
                || due_claim.map(|claim| FocusClaimTarget::Exact(claim.target))
                    == due_completion_target,
            "a pending focus claim and completion must describe the same target"
        );
        debug_assert!(
            due_blur.is_none()
                || due_completion_target.is_none()
                || due_completion_target == Some(FocusClaimTarget::Empty),
            "a pending blur claim and completion must describe the same target"
        );

        let target = due_claim
            .map(|claim| FocusClaimTarget::Exact(claim.target))
            .or(due_blur.map(|_| FocusClaimTarget::Empty))
            .or(due_completion_target)?;
        let committed = match target {
            FocusClaimTarget::Exact(target) => {
                self.next_frame.focus_path().last() == Some(&target)
                    && self
                        .next_frame
                        .dispatch_tree
                        .valid_focusable_node_id(target)
                        .is_some()
            }
            FocusClaimTarget::Empty => self.next_frame.focus_path().is_empty(),
        };

        if !committed
            && let FocusClaimTarget::Exact(target) = target
            && self
                .pending_focus_reveal_fence
                .as_ref()
                .is_some_and(|pending| pending.target == target)
        {
            self.pending_focus_reveal_fence = None;
        }

        if due_claim.is_some() {
            self.pending_focus_claim = None;
        }
        if due_blur.is_some() {
            self.pending_blur_claim_generation = None;
        }
        if due_completion_target.is_some()
            && let Some(completion) = self.pending_focus_completion.take()
        {
            self.queue_focus_claim_resolution(
                completion,
                if committed {
                    FocusClaimOutcome::Committed
                } else {
                    FocusClaimOutcome::Rejected
                },
            );
        }

        (!committed).then_some(target)
    }

    fn promote_pending_blur_claim(&mut self) {
        if self.pending_blur_claim_generation != Some(self.next_frame.generation) {
            return;
        }
        self.provisional_focus_claim = None;
        self.focus = None;
        self.next_frame.focus = None;
    }

    /// Remove focus from all elements within this context's window.
    pub fn blur(&mut self, cx: &mut App) {
        let _ = self.blur_impl(None);
        self.schedule_focus_claim_resolution_dispatch(cx);
    }

    /// Remove focus and observe the terminal result of this specific request.
    ///
    /// The callback runs at most once after empty focus commits, the request is rejected, or a
    /// later focus mutation supersedes it. Dropping the returned subscription cancels callback
    /// observation without cancelling the blur request.
    pub fn blur_with_completion(
        &mut self,
        cx: &mut App,
        listener: impl FnOnce(FocusClaimOutcome, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let callback = Rc::new(RefCell::new(Some(
            Box::new(listener) as AnyFocusClaimCompletion
        )));
        let cancelled_callback = Rc::downgrade(&callback);
        let subscription = Subscription::new(move || {
            if let Some(callback) = cancelled_callback.upgrade() {
                callback.borrow_mut().take();
            }
        });
        self.next_focus_claim_id = self.next_focus_claim_id.wrapping_add(1).max(1);
        let completion = PendingFocusCompletion {
            id: self.next_focus_claim_id,
            target: FocusClaimTarget::Empty,
            target_generation: 0,
            callback,
        };
        self.record_candidate_focus_completion(&completion);
        let _ = self.blur_impl(Some(completion));
        self.schedule_focus_claim_resolution_dispatch(cx);
        subscription
    }

    fn blur_impl(&mut self, completion: Option<PendingFocusCompletion>) -> bool {
        if !self.focus_mutations_enabled() {
            if let Some(completion) = completion {
                self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Rejected);
            }
            return false;
        }

        if self.frame_focus_authority_sealed
            && self.sealed_focus_retry_rejection == Some(FocusClaimTarget::Empty)
        {
            if let Some(completion) = completion {
                self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Rejected);
            }
            return false;
        }

        self.supersede_pending_focus_completion();
        self.restore_provisional_focus_claim();
        self.focus_claim_revision = self.focus_claim_revision.wrapping_add(1);
        self.pending_focus_claim = None;
        self.pending_focus_reveal_fence = None;
        self.pending_blur_claim_generation = None;
        if self.frame_focus_authority_sealed {
            let (accepted_generation, already_committed) = {
                let accepted_frame = self.sealed_focus_authority_frame();
                (
                    accepted_frame.generation,
                    self.focus.is_none() && accepted_frame.focus.is_none(),
                )
            };
            if already_committed {
                if let Some(completion) = completion {
                    self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Committed);
                }
                self.reconcile_focus_followup_refresh();
                return true;
            }
            let target_generation = accepted_generation.saturating_add(1);
            self.pending_blur_claim_generation = Some(target_generation);
            if let Some(mut completion) = completion {
                completion.target_generation = target_generation;
                self.pending_focus_completion = Some(completion);
            }
            self.refresh_focus_authority();
            return true;
        }

        let candidate_frame_in_progress =
            self.next_frame.generation == self.rendered_frame.generation.saturating_add(1);
        if !candidate_frame_in_progress
            && self.focus.is_none()
            && self.rendered_frame.focus_path().is_empty()
        {
            if let Some(completion) = completion {
                self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Committed);
            }
            self.reconcile_focus_followup_refresh();
            return true;
        }

        let target_generation = if candidate_frame_in_progress {
            self.next_frame.generation
        } else {
            self.rendered_frame.generation.saturating_add(1)
        };
        if let Some(mut completion) = completion {
            completion.target_generation = target_generation;
            self.pending_focus_completion = Some(completion);
        }
        self.focus = None;
        if candidate_frame_in_progress {
            self.next_frame.focus = None;
        }
        self.refresh_focus_authority();
        true
    }

    pub(crate) fn clear_dropped_focus(&mut self, dropped: FocusId, cx: &mut App) {
        let mut changed = false;
        if self
            .pending_focus_completion
            .as_ref()
            .is_some_and(|completion| completion.target == FocusClaimTarget::Exact(dropped))
            && let Some(completion) = self.pending_focus_completion.take()
        {
            self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Rejected);
            changed = true;
        }
        if self
            .pending_focus_claim
            .is_some_and(|claim| claim.target == dropped)
        {
            self.pending_focus_claim = None;
            changed = true;
        }
        if let Some(mut claim) = self.provisional_focus_claim {
            if claim.target == dropped {
                self.provisional_focus_claim = None;
                if self.focus == Some(dropped) {
                    self.focus = claim.fallback;
                }
                if self.next_frame.focus == Some(dropped) {
                    self.next_frame.focus = claim.fallback;
                }
                changed = true;
            } else if claim.fallback == Some(dropped) {
                claim.fallback = None;
                self.provisional_focus_claim = Some(claim);
                changed = true;
            }
        }
        if self.focus == Some(dropped) {
            self.focus = None;
            self.pending_blur_claim_generation = None;
            changed = true;
        }
        if self.next_frame.focus == Some(dropped) {
            self.next_frame.focus = None;
            changed = true;
        }
        if changed {
            self.refresh();
        }
        self.schedule_focus_claim_resolution_dispatch(cx);
    }

    /// Blur the window and don't allow anything in it to be focused again.
    pub fn disable_focus(&mut self, cx: &mut App) {
        if !self.focus_mutations_enabled() {
            return;
        }
        self.blur(cx);
        self.focus_enabled = false;
    }

    /// Move focus to next tab stop.
    pub fn focus_next(&mut self, cx: &mut App) {
        if !self.focus_mutations_enabled() {
            return;
        }

        if let Some(handle) = self.rendered_frame.tab_stops.next(self.focus.as_ref()) {
            self.focus(&handle, cx)
        }
    }

    /// Moves focus to the next live tab stop contained by a rendered focus scope root.
    ///
    /// Returns `true` when a target was focused. The search wraps within the scope and never
    /// focuses a tab stop outside it.
    pub fn focus_next_within(&mut self, scope: &FocusHandle, cx: &mut App) -> bool {
        self.focus_next_where_within(scope, |_| true, cx)
    }

    /// Moves focus to the next matching live tab stop contained by a rendered focus scope root.
    pub fn focus_next_where_within(
        &mut self,
        scope: &FocusHandle,
        predicate: impl Fn(&FocusHandle) -> bool,
        cx: &mut App,
    ) -> bool {
        if !self.focus_mutations_enabled() {
            return false;
        }
        let Some(handle) = self.next_tab_stop_where_within(scope, false, predicate) else {
            return false;
        };
        self.focus_impl(&handle, None, None, cx)
    }

    /// Move focus to previous tab stop.
    pub fn focus_prev(&mut self, cx: &mut App) {
        if !self.focus_mutations_enabled() {
            return;
        }

        if let Some(handle) = self.rendered_frame.tab_stops.prev(self.focus.as_ref()) {
            self.focus(&handle, cx)
        }
    }

    /// Moves focus to the previous live tab stop contained by a rendered focus scope root.
    ///
    /// Returns `true` when a target was focused. The search wraps within the scope and never
    /// focuses a tab stop outside it.
    pub fn focus_prev_within(&mut self, scope: &FocusHandle, cx: &mut App) -> bool {
        self.focus_prev_where_within(scope, |_| true, cx)
    }

    /// Moves focus to the previous matching live tab stop contained by a rendered focus scope
    /// root.
    pub fn focus_prev_where_within(
        &mut self,
        scope: &FocusHandle,
        predicate: impl Fn(&FocusHandle) -> bool,
        cx: &mut App,
    ) -> bool {
        if !self.focus_mutations_enabled() {
            return false;
        }
        let Some(handle) = self.next_tab_stop_where_within(scope, true, predicate) else {
            return false;
        };
        self.focus_impl(&handle, None, None, cx)
    }

    fn next_tab_stop_where_within(
        &self,
        scope: &FocusHandle,
        reverse: bool,
        predicate: impl Fn(&FocusHandle) -> bool,
    ) -> Option<FocusHandle> {
        if !self.is_focus_handle_rendered(scope) {
            return None;
        }

        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let focused_within = self
            .focus
            .filter(|focused| dispatch_tree.focus_contains(scope.id, *focused));
        let predicate = |candidate: &FocusHandle| {
            dispatch_tree.focus_contains(scope.id, candidate.id) && predicate(candidate)
        };

        if reverse {
            self.rendered_frame
                .tab_stops
                .prev_where(focused_within.as_ref(), predicate)
        } else {
            self.rendered_frame
                .tab_stops
                .next_where(focused_within.as_ref(), predicate)
        }
    }

    /// Accessor for the text system.
    pub fn text_system(&self) -> &Arc<WindowTextSystem> {
        &self.text_system
    }

    /// The current text style. Which is composed of all the style refinements provided to `with_text_style`.
    pub fn text_style(&self) -> TextStyle {
        let mut style = TextStyle::default();
        for refinement in &self.text_style_stack {
            style.refine(refinement);
        }
        style
    }

    /// Check if the platform window is maximized.
    ///
    /// On some platforms (namely Windows) this is different than the bounds being the size of the display
    pub fn is_maximized(&self) -> bool {
        self.platform_facts.is_maximized
    }

    /// request a certain window decoration (Wayland)
    pub fn request_decorations(&self, decorations: WindowDecorations) {
        self.platform_window.request_decorations(decorations);
    }

    /// Start a window resize operation (Wayland)
    pub fn start_window_resize(&self, edge: ResizeEdge) {
        self.platform_command_sink
            .enqueue(PlatformWindowCommand::StartWindowResize(edge));
    }

    /// Return the `WindowBounds` to indicate that how a window should be opened
    /// after it has been closed
    pub fn window_bounds(&self) -> WindowBounds {
        self.platform_facts.window_bounds
    }

    /// Return the `WindowBounds` excluding insets (Wayland and X11)
    pub fn inner_window_bounds(&self) -> WindowBounds {
        self.platform_facts.inner_window_bounds
    }

    /// Dispatch the given action on the currently focused element.
    pub fn dispatch_action(&mut self, action: Box<dyn Action>, cx: &mut App) {
        let focus_id = self.focused(cx).map(|handle| handle.id);

        let window = self.handle;
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    let node_id = window.focus_node_id_in_rendered_frame(focus_id);
                    window.dispatch_action_on_node(node_id, action.as_ref(), cx);
                })
                .log_err();
        })
    }

    pub(crate) fn dispatch_keystroke_observers(
        &mut self,
        event: &dyn Any,
        action: Option<Box<dyn Action>>,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() else {
            return;
        };

        cx.keystroke_observers.clone().retain(&(), move |callback| {
            (callback)(
                &KeystrokeEvent {
                    keystroke: key_down_event.keystroke.clone(),
                    action: action.as_ref().map(|action| action.boxed_clone()),
                    context_stack: context_stack.clone(),
                },
                self,
                cx,
            )
        });
    }

    pub(crate) fn dispatch_keystroke_interceptors(
        &mut self,
        event: &dyn Any,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() else {
            return;
        };

        cx.keystroke_interceptors
            .clone()
            .retain(&(), move |callback| {
                (callback)(
                    &KeystrokeEvent {
                        keystroke: key_down_event.keystroke.clone(),
                        action: None,
                        context_stack: context_stack.clone(),
                    },
                    self,
                    cx,
                )
            });
    }

    /// Schedules the given function to be run at the end of the current effect cycle, allowing entities
    /// that are currently on the stack to be returned to the app.
    pub fn defer(&self, cx: &mut App, f: impl FnOnce(&mut Window, &mut App) + 'static) {
        let handle = self.handle;
        cx.defer(move |cx| {
            handle.update(cx, |_, window, cx| f(window, cx)).ok();
        });
    }

    /// Subscribe to events emitted by a entity.
    /// The entity to which you're subscribing must implement the [`EventEmitter`] trait.
    /// The callback will be invoked a handle to the emitting entity, the event, and a window context for the current window.
    pub fn observe<T: 'static>(
        &mut self,
        observed: &Entity<T>,
        cx: &mut App,
        mut on_notify: impl FnMut(Entity<T>, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let entity_id = observed.entity_id();
        let observed = observed.downgrade();
        let window_handle = self.handle;
        cx.new_observer(
            entity_id,
            Box::new(move |cx| {
                window_handle
                    .update(cx, |_, window, cx| {
                        if let Some(handle) = observed.upgrade() {
                            on_notify(handle, window, cx);
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false)
            }),
        )
    }

    /// Subscribe to events emitted by a entity.
    /// The entity to which you're subscribing must implement the [`EventEmitter`] trait.
    /// The callback will be invoked a handle to the emitting entity, the event, and a window context for the current window.
    pub fn subscribe<Emitter, Evt>(
        &mut self,
        entity: &Entity<Emitter>,
        cx: &mut App,
        mut on_event: impl FnMut(Entity<Emitter>, &Evt, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        Emitter: EventEmitter<Evt>,
        Evt: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        let window_handle = self.handle;
        cx.new_subscription(
            entity_id,
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    window_handle
                        .update(cx, |_, window, cx| {
                            if let Some(entity) = handle.upgrade() {
                                let event = event.downcast_ref().expect("invalid event type");
                                on_event(entity, event, window, cx);
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false)
                }),
            ),
        )
    }

    /// Register a callback to be invoked when the given `Entity` is released.
    pub fn observe_release<T>(
        &self,
        entity: &Entity<T>,
        cx: &mut App,
        mut on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let entity_id = entity.entity_id();
        let window_handle = self.handle;
        let (subscription, activate) = cx.release_listeners.insert(
            entity_id,
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                let _ = window_handle.update(cx, |_, window, cx| on_release(entity, window, cx));
            }),
        );
        activate();
        subscription
    }

    /// Creates an [`AsyncWindowContext`], which has a static lifetime and can be held across
    /// await points in async code.
    pub fn to_async(&self, cx: &App) -> AsyncWindowContext {
        AsyncWindowContext::new_context(cx.to_async(), self.handle)
    }

    /// Schedule the given closure to be run directly after the current frame is rendered.
    pub fn on_next_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static) {
        RefCell::borrow_mut(&self.next_frame_callbacks).push(Box::new(callback));
    }

    /// Drains callbacks queued with [`Self::on_next_frame`] for deterministic tests.
    #[cfg(any(test, feature = "test-support"))]
    pub fn drain_next_frame_callbacks_for_test(&mut self, cx: &mut App) -> usize {
        let callbacks = self.next_frame_callbacks.take();
        let count = callbacks.len();
        for callback in callbacks {
            callback(self, cx);
        }
        count
    }

    /// Schedule a frame to be drawn on the next animation frame.
    ///
    /// This is useful for elements that need to animate continuously, such as a video player or an animated GIF.
    /// It will cause the window to redraw on the next frame, even if no other changes have occurred.
    ///
    /// If called from within a view, it will notify that view on the next frame. Otherwise, it will refresh the entire window.
    pub fn request_animation_frame(&self) {
        let entity = self.current_view();
        self.on_next_frame(move |_, cx| cx.notify(entity));
    }

    /// Spawn the future returned by the given closure on the application thread pool.
    /// The closure is provided a handle to the current window and an `AsyncWindowContext` for
    /// use within your future.
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, cx: &App, f: AsyncFn) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
    {
        let handle = self.handle;
        cx.spawn(async move |app| {
            let mut async_window_cx = AsyncWindowContext::new_context(app.clone(), handle);
            f(&mut async_window_cx).await
        })
    }

    /// Spawn the future returned by the given closure on the application thread
    /// pool, with the given priority. The closure is provided a handle to the
    /// current window and an `AsyncWindowContext` for use within your future.
    #[track_caller]
    pub fn spawn_with_priority<AsyncFn, R>(
        &self,
        priority: Priority,
        cx: &App,
        f: AsyncFn,
    ) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
    {
        let handle = self.handle;
        cx.spawn_with_priority(priority, async move |app| {
            let mut async_window_cx = AsyncWindowContext::new_context(app.clone(), handle);
            f(&mut async_window_cx).await
        })
    }

    /// Notify the window that its bounds have changed.
    ///
    /// This updates internal state like `viewport_size` and `scale_factor` from
    /// the platform window, then notifies observers. Normally called automatically
    /// by the platform's resize callback, but exposed publicly for test infrastructure.
    pub fn bounds_changed(&mut self, cx: &mut App) {
        self.refresh_platform_facts();
        self.refresh();
        self.notify_bounds_observers(cx);
    }

    fn refresh_platform_facts(&mut self) {
        let facts = self.platform_window.platform_facts();
        self.commit_platform_facts(facts);
    }

    fn commit_platform_facts(&mut self, mut facts: WindowPlatformFacts) {
        let capabilities = self.window_capabilities.mutations;
        if !capabilities.pointer_input.is_live() {
            facts.accepts_pointer_input = self.platform_facts.accepts_pointer_input;
        }
        if !capabilities.activation_policy.is_live() {
            facts.accepts_activation = self.platform_facts.accepts_activation;
            facts.focus_on_click = self.platform_facts.focus_on_click;
        }
        if !capabilities.alpha.is_live() {
            facts.background_appearance = self.platform_facts.background_appearance;
        }
        if !capabilities.topmost.is_live() {
            facts.topmost = self.platform_facts.topmost;
        }
        if !capabilities.taskbar_visibility.is_live() {
            facts.taskbar_visible = self.platform_facts.taskbar_visible;
        }
        self.viewport_size = facts.content_size;
        self.scale_factor = facts.scale_factor;
        self.display_id = facts.display_id;
        self.active.set(facts.is_active);
        self.platform_facts = facts;
    }

    /// Commits a backend-provided coherent terminal observation without re-reading platform
    /// getters. Intermediate move and resize notifications deliberately use [`Self::bounds_changed`]
    /// instead, so they cannot settle a queued placement at an arbitrary window-manager step.
    pub(crate) fn window_mutation_observed(
        &mut self,
        observation: PlatformWindowMutationObservation,
        cx: &mut App,
    ) -> bool {
        if !self.window_mutations.is_current_generation(
            &self.window_mutation_authority,
            observation.domain,
            observation.generation,
        ) {
            return false;
        }
        self.commit_platform_facts(observation.facts);
        let deliveries = self.window_mutations.settle_from_terminal_facts(
            &self.window_mutation_authority,
            observation.domain,
            observation.generation,
            observation.terminal,
            &self.platform_facts,
        );
        self.refresh();
        self.notify_bounds_observers(cx);
        Self::deliver_window_mutation_ticket_deliveries(deliveries);
        true
    }

    fn notify_bounds_observers(&mut self, cx: &mut App) {
        self.bounds_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    fn deliver_window_mutation_ticket_deliveries(deliveries: Vec<WindowMutationTicketDelivery>) {
        for delivery in deliveries {
            delivery.deliver();
        }
    }

    fn deliver_window_mutation_ticket_deliveries_panic_safe(
        deliveries: Vec<WindowMutationTicketDelivery>,
    ) -> std::thread::Result<()> {
        let mut first_panic = None;
        for delivery in deliveries {
            retain_first_window_cleanup_panic(
                &mut first_panic,
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| delivery.deliver())),
                "initial window mutation ticket delivery",
            );
        }
        match first_panic {
            Some(payload) => Err(payload),
            None => Ok(()),
        }
    }

    fn current_placement_state(&self) -> WindowPlacementState {
        placement_state_from_facts(&self.platform_facts)
    }

    fn placement_request_is_live(&self, request: WindowPlacementRequest) -> bool {
        let capabilities = self.window_capabilities.mutations;
        let facts = &self.platform_facts;

        if request
            .position
            .is_some_and(|position| position != facts.bounds.origin)
            && (!capabilities.position.is_live() || !capabilities.coordinate_space.is_global())
        {
            return false;
        }
        if request.size.is_some_and(|size| size != facts.bounds.size)
            && !capabilities.size.is_live()
        {
            return false;
        }
        if request
            .restore_bounds
            .is_some_and(|restore_bounds| restore_bounds != facts.window_bounds.get_bounds())
            && !capabilities.restore_bounds.is_live()
        {
            return false;
        }

        let current_state = self.current_placement_state();
        if let Some(target_state) = request.state
            && target_state != current_state
        {
            let support = match target_state {
                WindowPlacementState::Windowed => capabilities.windowed,
                WindowPlacementState::Maximized => capabilities.maximized,
                WindowPlacementState::Fullscreen => capabilities.fullscreen,
                WindowPlacementState::Minimized => capabilities.minimized,
            };
            if !support.is_live() {
                return false;
            }
        }

        true
    }

    fn window_mutation_request_is_live(&self, request: WindowMutationRequest) -> bool {
        match request {
            WindowMutationRequest::Placement(request) => self.placement_request_is_live(request),
            WindowMutationRequest::PointerInput(_) => {
                self.window_capabilities.mutations.pointer_input.is_live()
            }
            WindowMutationRequest::ActivationPolicy(_) => self
                .window_capabilities
                .mutations
                .activation_policy
                .is_live(),
            WindowMutationRequest::Alpha(_) => self.window_capabilities.mutations.alpha.is_live(),
            WindowMutationRequest::Topmost(_) => {
                self.window_capabilities.mutations.topmost.is_live()
            }
            WindowMutationRequest::TaskbarVisibility(_) => self
                .window_capabilities
                .mutations
                .taskbar_visibility
                .is_live(),
        }
    }

    /// Returns the bounds of the current window in its committed platform coordinate space.
    ///
    /// Inspect [`WindowPlatformFacts::coordinate_space`] before comparing this value with bounds
    /// from another window.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.platform_facts.bounds
    }

    /// Returns the latest committed platform facts for this window.
    ///
    /// Queued placement and pointer-input requests never update this value. It changes only when
    /// the backend supplies an observed facts snapshot.
    pub fn platform_facts(&self) -> &WindowPlatformFacts {
        &self.platform_facts
    }

    /// Returns the immutable creation and mutation capabilities for this platform window.
    pub fn window_capabilities(&self) -> PlatformWindowCapabilities {
        self.window_capabilities
    }

    pub(crate) fn window_profile(&self) -> PlatformWindowProfile {
        PlatformWindowProfile {
            kind: self.window_kind.clone(),
            capabilities: self.window_capabilities,
        }
    }

    /// Returns immutable facts established by the backend during creation.
    pub fn creation_facts(&self) -> &WindowCreationFacts {
        &self.creation_facts
    }

    /// Returns the latest committed presentation-stage facts.
    pub fn presentation_facts(&self) -> WindowPresentationFacts {
        WindowPresentationFacts {
            native_created: true,
            frame_accepted_generation: self.presentation_state.frame_accepted_generation,
            present_submitted_generation: self.presentation_state.present_submitted_generation,
            non_empty_presented_generation: self.presentation_state.non_empty_presented_generation,
            latest_present_attempt: self.presentation_state.latest_present_attempt,
            initial_presentation: self.presentation_state.initial_presentation,
            native_visible: self.platform_window.is_visible(),
        }
    }

    /// Renders the current frame's scene to a texture and returns the pixel data as an RGBA image.
    /// This does not present the frame to screen - useful for visual testing where we want
    /// to capture what would be rendered without displaying it or requiring the window to be visible.
    #[cfg(any(test, feature = "test-support"))]
    pub fn render_to_image(&self) -> anyhow::Result<image::RgbaImage> {
        self.capture_frame().into_image()
    }

    /// Renders the current frame's scene and returns pixels plus capture metadata.
    ///
    /// This is an offscreen framework capture. It does not prove that the OS
    /// compositor presented the frame.
    #[cfg(any(test, feature = "test-support"))]
    pub fn capture_frame(&self) -> WindowFrameCapture {
        let metadata = self.next_capture_metadata();
        match self
            .platform_window
            .render_to_image(&self.rendered_frame.scene)
        {
            Ok(image) => WindowFrameCapture {
                image: Some(image),
                metadata,
                unsupported_reason: None,
            },
            Err(error) => WindowFrameCapture {
                image: None,
                metadata,
                unsupported_reason: Some(error.to_string()),
            },
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn next_capture_metadata(&self) -> WindowFrameCaptureMetadata {
        let capture_generation = self.capture_generation.get().saturating_add(1);
        self.capture_generation.set(capture_generation);
        WindowFrameCaptureMetadata {
            window_id: self.handle.window_id(),
            framework_frame_generation: self.rendered_frame.generation,
            capture_generation,
            scale_factor: self.scale_factor(),
            logical_viewport_size: self.viewport_size,
            physical_viewport_size: self.viewport_size.to_device_pixels(self.scale_factor()),
        }
    }

    /// Returns image paint facts for the current rendered framework frame.
    pub fn rendered_frame_image_paint_diagnostics(&self) -> &[ImagePaintDiagnostic] {
        &self.rendered_frame.image_paint_diagnostics
    }

    /// Returns atlas access facts for the current rendered framework frame.
    pub fn rendered_frame_atlas_access_diagnostics(&self) -> &[AtlasAccessDiagnostic] {
        &self.rendered_frame.atlas_access_diagnostics
    }

    /// Returns atlas removal facts observed by this window.
    pub fn atlas_remove_diagnostics(&self) -> &[AtlasRemoveDiagnostic] {
        &self.atlas_remove_diagnostics
    }

    /// Requests a coherent placement change using the legacy [`WindowBounds`] projection.
    ///
    /// Prefer [`Self::request_window_placement_request`] when a caller needs to explicitly
    /// request minimized state or a property-specific placement update.
    pub fn request_window_placement(
        &mut self,
        window_bounds: WindowBounds,
    ) -> WindowMutationDispatch {
        self.request_window_placement_request(WindowPlacementRequest::from_window_bounds(
            window_bounds,
        ))
    }

    /// Requests a structured placement change for this already-open window.
    ///
    /// The request is accepted only when every changed property is live on this platform. A
    /// queued result does not change public getter values until the backend later emits a
    /// coherent terminal observation.
    pub fn request_window_placement_request(
        &mut self,
        request: WindowPlacementRequest,
    ) -> WindowMutationDispatch {
        self.request_window_mutation(WindowMutationRequest::Placement(request))
    }

    /// Requests one typed mutation for this already-open window.
    ///
    /// Every request advances only its own conflict-domain generation. A queued result does not
    /// change public facts until the backend returns a terminal observation carrying that exact
    /// generation.
    pub fn request_window_mutation(
        &mut self,
        request: WindowMutationRequest,
    ) -> WindowMutationDispatch {
        if self.removal_state != WindowRemovalState::Open || self.removed {
            return WindowMutationDispatch::WindowClosed;
        }
        if let WindowMutationRequest::Placement(placement) = request
            && !placement_request_is_valid(placement, &self.platform_facts)
        {
            return WindowMutationDispatch::Rejected;
        }

        let Some(begin) = self.window_mutations.begin(
            &self.window_mutation_authority,
            request,
            &self.platform_facts,
        ) else {
            return WindowMutationDispatch::Rejected;
        };
        let ticket = begin.ticket;
        let mut deliveries = begin.deliveries;
        self.platform_window
            .prepare_window_mutation(ticket.domain(), ticket.generation());

        let dispatch = if request.matches_facts(&self.platform_facts) {
            deliveries.extend(self.window_mutations.settle_unqueued(
                &self.window_mutation_authority,
                &ticket,
                WindowMutationOutcome::Exact,
                &self.platform_facts,
            ));
            WindowMutationDispatch::Unchanged
        } else if !self.window_mutation_request_is_live(request) {
            deliveries.extend(self.window_mutations.settle_unqueued(
                &self.window_mutation_authority,
                &ticket,
                WindowMutationOutcome::Unsupported,
                &self.platform_facts,
            ));
            WindowMutationDispatch::Unsupported
        } else {
            match self
                .platform_window
                .request_window_mutation(ticket.generation(), request)
            {
                PlatformWindowDispatch::Queued => WindowMutationDispatch::Queued(ticket),
                PlatformWindowDispatch::Unchanged => {
                    self.refresh_platform_facts();
                    let outcome = if request.matches_facts(&self.platform_facts) {
                        WindowMutationOutcome::Exact
                    } else {
                        WindowMutationOutcome::Adjusted
                    };
                    deliveries.extend(self.window_mutations.settle_unqueued(
                        &self.window_mutation_authority,
                        &ticket,
                        outcome,
                        &self.platform_facts,
                    ));
                    WindowMutationDispatch::Unchanged
                }
                platform_dispatch => {
                    let outcome = platform_dispatch_outcome(platform_dispatch)
                        .expect("queued and unchanged dispatches returned above");
                    deliveries.extend(self.window_mutations.settle_unqueued(
                        &self.window_mutation_authority,
                        &ticket,
                        outcome,
                        &self.platform_facts,
                    ));
                    match platform_dispatch {
                        PlatformWindowDispatch::Unsupported => WindowMutationDispatch::Unsupported,
                        PlatformWindowDispatch::Rejected => WindowMutationDispatch::Rejected,
                        PlatformWindowDispatch::WindowClosed => {
                            WindowMutationDispatch::WindowClosed
                        }
                        PlatformWindowDispatch::Queued | PlatformWindowDispatch::Unchanged => {
                            unreachable!("handled before terminal dispatch mapping")
                        }
                    }
                }
            }
        };

        Self::deliver_window_mutation_ticket_deliveries(deliveries);
        dispatch
    }

    /// Requests whether this window accepts pointer input.
    ///
    /// A queued result is only an intent accepted by the backend. The committed facts cache
    /// changes only after a coherent terminal observation.
    pub fn request_pointer_input(&mut self, accepts_pointer_input: bool) -> WindowMutationDispatch {
        self.request_window_mutation(WindowMutationRequest::PointerInput(accepts_pointer_input))
    }

    /// Requests one coherent lifetime activation policy.
    pub fn request_activation_policy(
        &mut self,
        policy: crate::WindowActivationPolicy,
    ) -> WindowMutationDispatch {
        self.request_window_mutation(WindowMutationRequest::ActivationPolicy(policy))
    }

    /// Requests whether this window stays above ordinary application windows.
    pub fn request_topmost(&mut self, topmost: bool) -> WindowMutationDispatch {
        self.request_window_mutation(WindowMutationRequest::Topmost(topmost))
    }

    /// Requests whether this window appears in the taskbar or application switcher.
    pub fn request_taskbar_visibility(&mut self, visible: bool) -> WindowMutationDispatch {
        self.request_window_mutation(WindowMutationRequest::TaskbarVisibility(visible))
    }

    /// Requests a content-size change through the placement mutation authority.
    pub fn resize(&mut self, size: Size<Pixels>) -> WindowMutationDispatch {
        let current_bounds = self.platform_facts.window_bounds.get_bounds();
        let requested_bounds = Bounds::new(current_bounds.origin, size);
        let request = match self.current_placement_state() {
            WindowPlacementState::Windowed => WindowPlacementRequest {
                size: Some(size),
                ..WindowPlacementRequest::new()
            },
            WindowPlacementState::Maximized
            | WindowPlacementState::Fullscreen
            | WindowPlacementState::Minimized => WindowPlacementRequest {
                restore_bounds: Some(requested_bounds),
                ..WindowPlacementRequest::new()
            },
        };
        self.request_window_placement_request(request)
    }

    /// Returns whether or not the window is currently fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.platform_facts.is_fullscreen
    }

    /// Returns whether or not the window is currently minimized.
    pub fn is_minimized(&self) -> bool {
        self.platform_facts.is_minimized
    }

    /// Returns whether this platform window currently receives pointer input.
    pub fn accepts_pointer_input(&self) -> bool {
        self.platform_facts.accepts_pointer_input
    }

    /// Updates whether this platform window receives pointer input when the backend supports it.
    pub fn set_accepts_pointer_input(
        &mut self,
        accepts_pointer_input: bool,
    ) -> WindowMutationDispatch {
        self.request_pointer_input(accepts_pointer_input)
    }

    pub(crate) fn appearance_changed(&mut self, cx: &mut App) {
        self.appearance = self.platform_window.appearance();

        self.appearance_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    pub(crate) fn button_layout_changed(&mut self, cx: &mut App) {
        self.button_layout_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    pub(crate) fn native_active_status_changed(&mut self, active: bool, cx: &mut App) {
        if active && !self.provisional_session_accepts_interaction() {
            self.active.set(false);
            return;
        }
        self.active.set(active);
        if !active {
            self.cancel_pointer_session(PointerCancelReason::WindowDeactivated, cx);
        }
        self.modifiers = self.platform_window.modifiers();
        self.capslock = self.platform_window.capslock();
        self.activation_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
        self.bounds_changed(cx);
        self.refresh();
        SystemWindowTabController::update_last_active(cx, self.handle.id);
    }

    pub(crate) fn native_hover_status_changed(&mut self, hovered: bool, cx: &mut App) {
        self.hovered.set(hovered);
        self.mouse_in_window = hovered;
        if !hovered {
            self.reset_cursor_style(cx);
        }
        self.refresh();
    }

    /// Returns the appearance of the current window.
    pub fn appearance(&self) -> WindowAppearance {
        self.appearance
    }

    /// Returns the size of the drawable area within the window.
    pub fn viewport_size(&self) -> Size<Pixels> {
        self.viewport_size
    }

    /// Returns whether this window is focused by the operating system (receiving key events).
    pub fn is_window_active(&self) -> bool {
        self.active.get()
    }

    /// Returns whether accessibility is effectively active for the current frame.
    ///
    /// This is GPUI's frame-local accessibility state, not whether the operating system considers
    /// the window active. See [`Self::is_window_active`] for the latter.
    pub fn is_accessibility_active(&self) -> bool {
        self.a11y.is_active()
    }

    /// Queues a transient, window-scoped accessibility announcement.
    ///
    /// An accepted outcome means only that the request entered the bounded queue. If its
    /// accessibility activation generation remains current, the request enters the final AccessKit
    /// tree without moving focus or invoking a native speech API. Deactivation, activation
    /// replacement, or window close can clear an accepted request before publication; requests
    /// made while accessibility is inactive or the window is closing are dropped and never replayed.
    pub fn announce(
        &mut self,
        announcement: AccessibilityAnnouncement,
        _cx: &mut App,
    ) -> AccessibilityAnnouncementOutcome {
        if self.removal_state != WindowRemovalState::Open || self.removed {
            return self
                .a11y
                .reject_announcement_for_closed_window(announcement);
        }

        let outcome = self.a11y.enqueue_announcement(announcement);
        if outcome.is_accepted() {
            self.refresh();
        }
        outcome
    }

    /// Returns the bounded metadata-only announcement diagnostic history for this window.
    pub fn accessibility_announcement_diagnostics(&self) -> &[AccessibilityAnnouncementDiagnostic] {
        self.a11y.announcement_diagnostics()
    }

    /// Returns whether this window is considered to be the window
    /// that currently owns the mouse cursor.
    /// On mac, this is equivalent to `is_window_active`.
    pub fn is_window_hovered(&self) -> bool {
        if cfg!(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd"
        )) {
            self.hovered.get()
        } else {
            self.is_window_active()
        }
    }

    /// Toggles between maximized and windowed placement through the placement authority.
    ///
    /// A queued result does not update committed window facts until the backend emits a terminal
    /// observation.
    pub fn zoom_window(&mut self) -> WindowMutationDispatch {
        let state = if self.platform_facts.is_maximized {
            WindowPlacementState::Windowed
        } else {
            WindowPlacementState::Maximized
        };
        self.request_window_placement_request(WindowPlacementRequest {
            state: Some(state),
            ..WindowPlacementRequest::new()
        })
    }

    /// Opens the native title bar context menu, useful when implementing client side decorations (Wayland and X11)
    pub fn show_window_menu(&self, position: Point<Pixels>) {
        self.platform_command_sink
            .enqueue(PlatformWindowCommand::ShowWindowMenu(position));
    }

    /// Handle window movement for Linux and macOS.
    /// Tells the compositor to take control of window movement (Wayland and X11)
    ///
    /// Events may not be received during a move operation.
    pub fn start_window_move(&self) {
        self.platform_command_sink
            .enqueue(PlatformWindowCommand::StartWindowMove);
    }

    /// When using client side decorations, set this to the width of the invisible decorations (Wayland and X11)
    pub fn set_client_inset(&mut self, inset: Pixels) {
        self.client_inset = Some(inset);
        self.platform_window.set_client_inset(inset);
    }

    /// Returns the client_inset value by [`Self::set_client_inset`].
    pub fn client_inset(&self) -> Option<Pixels> {
        self.client_inset
    }

    /// Returns whether the title bar window controls need to be rendered by the application (Wayland and X11)
    pub fn window_decorations(&self) -> Decorations {
        self.platform_window.window_decorations()
    }

    /// Returns which window controls are currently visible (Wayland)
    pub fn window_controls(&self) -> WindowControls {
        self.platform_window.window_controls()
    }

    /// Updates the window's title at the platform level.
    pub fn set_window_title(&mut self, title: &str) {
        self.platform_window.set_title(title);
    }

    /// Sets the position of the macOS traffic light buttons.
    #[cfg(target_os = "macos")]
    pub fn set_traffic_light_position(&self, position: Point<Pixels>) {
        self.platform_window.set_traffic_light_position(position);
    }

    /// Sets the application identifier.
    pub fn set_app_id(&mut self, app_id: &str) {
        self.platform_window.set_app_id(app_id);
    }

    /// Requests a native background or alpha-treatment change.
    pub fn set_background_appearance(
        &mut self,
        background_appearance: WindowBackgroundAppearance,
    ) -> WindowMutationDispatch {
        self.request_window_mutation(WindowMutationRequest::Alpha(background_appearance))
    }

    /// Mark the window as dirty at the platform level.
    pub fn set_window_edited(&mut self, edited: bool) {
        self.platform_window.set_edited(edited);
    }

    /// Set the path of the file this window represents.
    /// On macOS, this sets the window's accessibility document property (AXDocument).
    pub fn set_document_path(&self, path: Option<&std::path::Path>) {
        self.platform_window.set_document_path(path);
    }

    /// Determine the display on which the window is visible.
    pub fn display(&self, cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
        cx.platform
            .displays()
            .into_iter()
            .find(|display| Some(display.id()) == self.display_id)
    }

    /// Show the platform character palette.
    pub fn show_character_palette(&self) {
        self.platform_window.show_character_palette();
    }

    /// The scale factor of the display associated with the window. For example, it could
    /// return 2.0 for a "retina" display, indicating that each logical pixel should actually
    /// be rendered as two pixels on screen.
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Returns the debug selectors and bounds from the most recently committed frame.
    ///
    /// This is test-only inspection data. Production code must not use debug selectors as
    /// interaction or layout authority.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn committed_debug_bounds_for_test(&self) -> Vec<(String, Bounds<Pixels>)> {
        let mut bounds = self
            .rendered_frame
            .debug_bounds
            .iter()
            .map(|(selector, bounds)| (selector.clone(), *bounds))
            .collect::<Vec<_>>();
        bounds.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        bounds
    }

    /// The size of an em for the base font of the application. Adjusting this value allows the
    /// UI to scale, just like zooming a web page.
    pub fn rem_size(&self) -> Pixels {
        self.rem_size_override_stack
            .last()
            .copied()
            .unwrap_or(self.rem_size)
    }

    /// Sets the size of an em for the base font of the application. Adjusting this value allows the
    /// UI to scale, just like zooming a web page.
    pub fn set_rem_size(&mut self, rem_size: impl Into<Pixels>) {
        self.rem_size = rem_size.into();
    }

    /// Acquire a globally unique identifier for the given ElementId.
    /// Only valid for the duration of the provided closure.
    pub fn with_global_id<R>(
        &mut self,
        element_id: ElementId,
        f: impl FnOnce(&GlobalElementId, &mut Self) -> R,
    ) -> R {
        self.with_id(element_id, |this| {
            let global_id = GlobalElementId(Arc::from(&*this.element_id_stack));

            f(&global_id, this)
        })
    }

    /// Calls the provided closure with the element ID pushed on the stack.
    #[inline]
    pub fn with_id<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.element_id_stack.push(element_id.into());
        let result = f(self);
        self.element_id_stack.pop();
        result
    }

    /// Executes the provided function with the specified rem size.
    ///
    /// This method must only be called as part of element drawing.
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_rem_size<F, R>(&mut self, rem_size: Option<impl Into<Pixels>>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        if let Some(rem_size) = rem_size {
            self.rem_size_override_stack.push(rem_size.into());
            let result = f(self);
            self.rem_size_override_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// The line height associated with the current text style.
    pub fn line_height(&self) -> Pixels {
        self.text_style().line_height_in_pixels(self.rem_size())
    }

    /// Rounds a logical value to the nearest device pixel.
    #[inline]
    pub fn pixel_snap(&self, value: Pixels) -> Pixels {
        px(round_to_device_pixel(value.0, self.scale_factor()) / self.scale_factor())
    }

    /// f64 variant of [`Self::pixel_snap`].
    #[inline]
    pub fn pixel_snap_f64(&self, value: f64) -> f64 {
        let scale_factor = f64::from(self.scale_factor());
        round_half_toward_zero_f64(value * scale_factor) / scale_factor
    }

    /// Snaps a bounds' origin and size to the nearest device pixel.
    #[inline]
    pub fn pixel_snap_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        bounds.map(|c| self.pixel_snap(c))
    }

    /// Snaps a point's coordinates to the nearest device pixel.
    #[inline]
    pub fn pixel_snap_point(&self, position: Point<Pixels>) -> Point<Pixels> {
        position.map(|c| self.pixel_snap(c))
    }

    #[inline]
    fn device_local_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<ScaledPixels> {
        bounds.scale(self.scale_factor())
    }

    #[inline]
    fn snap_device_bounds(bounds: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
        let left = round_half_toward_zero(bounds.left().0);
        let top = round_half_toward_zero(bounds.top().0);
        let right = round_half_toward_zero(bounds.right().0).max(left);
        let bottom = round_half_toward_zero(bounds.bottom().0).max(top);
        Bounds::from_corners(
            point(ScaledPixels(left), ScaledPixels(top)),
            point(ScaledPixels(right), ScaledPixels(bottom)),
        )
    }

    #[inline]
    fn cover_device_bounds(bounds: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
        let left = floor_to_device_pixel(bounds.left().0, 1.0);
        let top = floor_to_device_pixel(bounds.top().0, 1.0);
        let right = ceil_to_device_pixel(bounds.right().0, 1.0).max(left);
        let bottom = ceil_to_device_pixel(bounds.bottom().0, 1.0).max(top);
        Bounds::from_corners(
            point(ScaledPixels(left), ScaledPixels(top)),
            point(ScaledPixels(right), ScaledPixels(bottom)),
        )
    }

    /// Floors the near edge and ceils the far edge, producing a strict superset of the raw region.
    #[inline]
    fn cover_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<ScaledPixels> {
        Self::cover_device_bounds(self.device_local_bounds(bounds))
    }

    /// Call to prevent default handling for the event currently being dispatched.
    /// Built-in handlers consult this flag before applying behaviors such as
    /// automatic focus transfer or default scrolling.
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    /// Obtain whether default has been prevented for the event currently being dispatched.
    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }

    /// Return the most recent input dispatch result recorded by this window.
    ///
    /// This is a stable diagnostic snapshot for tests and tooling that need to assert default input
    /// consumption or propagation after simulated input.
    pub fn last_dispatch_event_result(&self) -> Option<DispatchEventResult> {
        self.last_dispatch_event_result
    }

    /// Determine whether the given action is available along the dispatch path to the currently focused element.
    pub fn is_action_available(&self, action: &dyn Action, cx: &App) -> bool {
        let node_id =
            self.focus_node_id_in_rendered_frame(self.focused(cx).map(|handle| handle.id));
        self.rendered_frame
            .dispatch_tree
            .is_action_available(action, node_id)
    }

    /// Determine whether the given action is available along the dispatch path to the given focus_handle.
    pub fn is_action_available_in(&self, action: &dyn Action, focus_handle: &FocusHandle) -> bool {
        let node_id = self.focus_node_id_in_rendered_frame(Some(focus_handle.id));
        self.rendered_frame
            .dispatch_tree
            .is_action_available(action, node_id)
    }

    /// The position of the mouse relative to the window.
    pub fn mouse_position(&self) -> Point<Pixels> {
        self.mouse_position
    }

    /// Whether the mouse is currently inside this window according to the last platform input.
    pub fn is_mouse_in_window(&self) -> bool {
        self.mouse_in_window
    }

    /// The current state of the keyboard's modifiers
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Returns true if the last input event was keyboard-based (key press, tab navigation, etc.)
    /// This is used for focus-visible styling to show focus indicators only for keyboard navigation.
    pub fn last_input_was_keyboard(&self) -> bool {
        self.last_input_modality == InputModality::Keyboard
    }

    /// The current state of the keyboard's capslock
    pub fn capslock(&self) -> Capslock {
        self.capslock
    }

    fn complete_frame(&self) {
        self.platform_window.completed_frame();
    }

    pub(crate) fn native_frame_requested(
        &mut self,
        request_frame_options: RequestFrameOptions,
        cx: &mut App,
    ) {
        let renderer_repaint_pending = self.renderer_repaint_is_pending();
        let initial_presentation_pending = self.fresh_initial_presentation_is_pending();
        let fresh_frame_required =
            self.fresh_initial_presentation_frame_is_required() || renderer_repaint_pending;
        // A force hint can outlive the recovery draw it requested. Once a suitable fresh scene is
        // already accepted, presenting that scene takes precedence over producing another one.
        let force_render = fresh_frame_required
            || (request_frame_options.force_render
                && !(initial_presentation_pending && !fresh_frame_required));
        let min_frame_interval = if initial_presentation_pending || renderer_repaint_pending {
            // Recovery authority must survive inactive and thermal throttling. A hidden window has
            // no ordinary frame pump, while an invalidated scene must never be submitted again.
            None
        } else {
            FrameThrottleFacts {
                force_render,
                require_presentation: request_frame_options.require_presentation,
                has_next_frame_callbacks: !self.next_frame_callbacks.borrow().is_empty(),
                active: self.active.get(),
                thermal_state: Some(cx.thermal_state()),
            }
            .min_frame_interval()
        };
        let now = Instant::now();
        if frame_should_wait(now, self.last_frame_time.get(), min_frame_interval) {
            self.complete_frame();
            return;
        }
        self.last_frame_time.set(Some(now));

        for callback in self.next_frame_callbacks.take() {
            callback(self, cx);
        }

        if !self.presentation_is_allowed() {
            self.complete_frame();
            return;
        }

        let needs_present = PresentFacts {
            require_presentation: request_frame_options.require_presentation,
            needs_present: self.needs_present.get(),
            active: self.active.get(),
            high_rate_input: self.input_rate_tracker.borrow_mut().is_high_rate(),
        }
        .needs_present();

        if self.invalidator.is_dirty() || force_render {
            measure("frame duration", || {
                if force_render {
                    self.refresh();
                }
                let accepted_generation = self.presentation_state.frame_accepted_generation;
                let arena_clear_needed = self.draw(cx);
                if self.presentation_state.frame_accepted_generation != accepted_generation {
                    self.present();
                }
                arena_clear_needed.clear();
            })
        } else if needs_present {
            if self.fresh_initial_presentation_is_pending()
                && !self.fresh_initial_presentation_frame_is_required()
                && self.fresh_initial_presentation_is_deferred_retry()
                && !self.begin_initial_presentation_retry(cx)
            {
                self.complete_frame();
                return;
            }
            self.present();
        }

        if !self.presentation_is_allowed() {
            self.complete_frame();
            return;
        }
        self.finish_fresh_initial_presentation_if_ready(cx);
        if self.fresh_initial_presentation_is_pending() {
            if self.fresh_initial_presentation_frame_is_required() {
                if self.fresh_initial_presentation_attempts_exhausted() {
                    self.fail_fresh_initial_presentation(cx);
                } else {
                    self.request_fresh_initial_presentation_frame();
                }
            } else if self.initial_presentation_retries_exhausted() {
                self.fail_fresh_initial_presentation(cx);
            } else {
                self.request_initial_presentation_retry();
            }
        } else if self.renderer_repaint_is_pending() && self.presentation_is_allowed() {
            self.request_renderer_repaint_frame();
        }

        self.complete_frame();
    }

    fn candidate_frame_authority_checkpoint(&self) -> CandidateFrameAuthorityCheckpoint {
        CandidateFrameAuthorityCheckpoint {
            focus: self.focus,
            pending_focus_claim: self.pending_focus_claim,
            pending_focus_reveal_fence: self.pending_focus_reveal_fence.clone(),
            pending_blur_claim_generation: self.pending_blur_claim_generation,
            provisional_focus_claim: self.provisional_focus_claim,
            pending_focus_completion: self.pending_focus_completion.clone(),
            focus_claim_resolutions_len: self.focus_claim_resolutions.len(),
            focus_claim_revision: self.focus_claim_revision,
            requested_autoscroll: self.requested_autoscroll.clone(),
            tooltip_bounds: self.tooltip_bounds.clone(),
            focus_followup_requested: self.focus_followup_requested,
        }
    }

    fn restore_candidate_frame_authority(&mut self, checkpoint: CandidateFrameAuthorityCheckpoint) {
        self.focus = checkpoint.focus;
        self.pending_focus_claim = checkpoint.pending_focus_claim;
        self.pending_focus_reveal_fence = checkpoint.pending_focus_reveal_fence;
        self.pending_blur_claim_generation = checkpoint.pending_blur_claim_generation;
        self.provisional_focus_claim = checkpoint.provisional_focus_claim;
        self.pending_focus_completion = checkpoint.pending_focus_completion;
        self.focus_claim_resolutions
            .truncate(checkpoint.focus_claim_resolutions_len);
        self.focus_claim_revision = checkpoint.focus_claim_revision;
        self.requested_autoscroll = checkpoint.requested_autoscroll;
        self.tooltip_bounds = checkpoint.tooltip_bounds;
        self.focus_followup_requested = checkpoint.focus_followup_requested;
        self.candidate_pending_input_clear = false;
        self.candidate_pending_input_notification = false;
    }

    fn focus_resolutions_for_rejected_candidate(
        &mut self,
        candidate: &CandidateFrameTransaction,
        checkpoint: &CandidateFrameAuthorityCheckpoint,
    ) -> Vec<FocusClaimResolution> {
        let candidate_resolutions = self
            .focus_claim_resolutions
            .split_off(checkpoint.focus_claim_resolutions_len);
        let mut terminal_resolutions = candidate_resolutions
            .into_iter()
            .filter(|resolution| candidate.owns_focus_completion(resolution.id))
            .map(|mut resolution| {
                if resolution.outcome == FocusClaimOutcome::Committed {
                    resolution.outcome = FocusClaimOutcome::Rejected;
                }
                resolution
            })
            .collect::<Vec<_>>();

        if self
            .pending_focus_completion
            .as_ref()
            .is_some_and(|completion| candidate.owns_focus_completion(completion.id))
            && let Some(completion) = self.pending_focus_completion.take()
        {
            terminal_resolutions.push(FocusClaimResolution {
                id: completion.id,
                outcome: FocusClaimOutcome::Rejected,
                callback: completion.callback,
            });
        }

        terminal_resolutions
    }

    fn append_focus_claim_resolutions(&mut self, resolutions: Vec<FocusClaimResolution>) {
        for resolution in resolutions {
            debug_assert!(
                self.focus_claim_resolutions
                    .iter()
                    .all(|queued| queued.id != resolution.id),
                "a focus claim must have exactly one terminal result"
            );
            self.focus_claim_resolutions.push(resolution);
        }
    }

    fn rollback_candidate_frame_transfers(&mut self) {
        for key in self.candidate_frame_transfers.element_states.drain(..) {
            let state = self
                .next_frame
                .element_states
                .remove(&key)
                .expect("candidate element state transfer must remain available for rollback");
            assert!(
                self.rendered_frame
                    .element_states
                    .insert(key, state)
                    .is_none(),
                "candidate element state rollback must restore one vacant committed slot"
            );
        }

        for (rendered_index, candidate_index) in
            self.candidate_frame_transfers.mouse_listeners.drain(..)
        {
            let listener = self.next_frame.mouse_listeners[candidate_index]
                .value
                .take();
            assert!(
                self.rendered_frame.mouse_listeners[rendered_index]
                    .value
                    .is_none(),
                "candidate mouse-listener rollback must restore one vacant committed slot"
            );
            self.rendered_frame.mouse_listeners[rendered_index].value = listener;
        }

        for (rendered_index, candidate_index) in
            self.candidate_frame_transfers.input_handlers.drain(..)
        {
            let mut handler = self.next_frame.input_handlers[candidate_index].value.take();
            if let Some(handler) = handler.as_mut() {
                handler.set_validity(
                    self.rendered_frame.input_handlers[rendered_index]
                        .validity
                        .clone(),
                );
            }
            assert!(
                self.rendered_frame.input_handlers[rendered_index]
                    .value
                    .is_none(),
                "candidate input-handler rollback must restore one vacant committed slot"
            );
            self.rendered_frame.input_handlers[rendered_index].value = handler;
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn accept_visual_test_frame_transfers(&mut self) {
        // VisualTestContext paints an element directly into `next_frame` and then asks the
        // platform test window to commit that frame. Those transfers are therefore accepted by
        // the test harness rather than owned by a later full-window candidate transaction.
        self.candidate_frame_transfers = CandidateFrameTransfers::default();
    }

    /// Produces a new frame and assigns it to `rendered_frame`. To actually show
    /// the contents of the new [`Scene`], use [`Self::present`].
    #[profiling::function]
    pub fn draw(&mut self, cx: &mut App) -> ArenaClearNeeded {
        // Set up the per-App arena for element allocation during this draw.
        // This ensures that multiple test Apps have isolated arenas.
        let _arena_scope = ElementArenaScope::enter(&cx.element_arena);

        if !self.begin_fresh_initial_presentation_attempt(cx) {
            return ArenaClearNeeded::new(&cx.element_arena);
        }

        self.invalidate_entities();
        cx.entities.clear_accessed();
        debug_assert!(self.rendered_entity_stack.is_empty());
        debug_assert!(self.subtree_presentation_stack.borrow().is_empty());
        debug_assert!(self.subtree_transform_stack.borrow().is_empty());
        debug_assert!(self.scroll_ancestry_stack.borrow().is_empty());
        debug_assert!(!self.frame_focus_authority_sealed);
        debug_assert!(!self.focus_followup_requested);
        debug_assert!(self.sealed_focus_retry_rejection.is_none());
        debug_assert!(self.candidate_frame_transfers.element_states.is_empty());
        debug_assert!(self.candidate_frame_transfers.mouse_listeners.is_empty());
        debug_assert!(self.candidate_frame_transfers.input_handlers.is_empty());
        debug_assert!(self.candidate_frame_transaction.is_none());
        debug_assert!(self.candidate_atlas_lease_failure.is_none());
        debug_assert!(!self.candidate_pending_input_clear);
        debug_assert!(!self.candidate_pending_input_notification);
        let authority_checkpoint = self.candidate_frame_authority_checkpoint();
        self.candidate_accessibility_focus = None;
        self.candidate_atlas_lease_failure = None;
        self.candidate_pending_input_clear = false;
        self.candidate_pending_input_notification = false;
        self.invalidator.set_dirty(false);
        self.requested_autoscroll = None;
        self.next_frame.generation = self.rendered_frame.generation.saturating_add(1);
        self.next_candidate_frame_attempt_id = self
            .next_candidate_frame_attempt_id
            .checked_add(1)
            .expect("candidate-frame attempt identity space exhausted");
        self.candidate_frame_transaction = Some(CandidateFrameTransaction::new(
            CandidateFrameAttemptId(self.next_candidate_frame_attempt_id),
            self.next_frame.generation,
        ));
        self.promote_pending_blur_claim();

        // Restore the previously-used input handler.
        // Place it back into a None slot (left by a previous .take()) so that
        // cached paint_range indices in reuse_paint find the handler at the
        // expected position.
        let rendered_input_handlers_len = self.rendered_frame.input_handlers.len();
        let mut restored_input_handler_index = None;
        if let Some(input_handler) = self.platform_window.take_input_handler() {
            let validity = input_handler.validity();
            if let Some((index, slot)) = self
                .rendered_frame
                .input_handlers
                .iter_mut()
                .enumerate()
                .rev()
                .find(|(_, output)| output.value.is_none())
            {
                slot.value = Some(input_handler);
                slot.validity = validity;
                restored_input_handler_index = Some(index);
            } else {
                restored_input_handler_index = Some(self.rendered_frame.input_handlers.len());
                self.rendered_frame
                    .input_handlers
                    .push(FrameOutput::new(Some(input_handler), validity));
            }
        }
        let (candidate_a11y_update, candidate_stale_pointer_owner) =
            if !cx.mode.skip_drawing() && self.presentation_is_allowed() {
                if self.provisional_session_projects_destination_semantics() {
                    self.draw_roots(cx)
                } else {
                    self.with_subtree_presentation(SubtreePresentation::Inert, |window| {
                        window.draw_roots(cx)
                    })
                }
            } else {
                (
                    None,
                    self.stale_pointer_session_owner_in_candidate_frame(cx)
                        .map(|owner| (owner, false)),
                )
            };
        if let Some(error) = self.candidate_atlas_lease_failure.take() {
            let rejected_generation = self.next_frame.generation;
            let rejected_attempt = self
                .candidate_frame_transaction
                .take()
                .expect("an atlas-rejected candidate must own a frame transaction");
            debug_assert_eq!(rejected_attempt.frame_generation, rejected_generation);
            let rejected_focus_resolutions = self
                .focus_resolutions_for_rejected_candidate(&rejected_attempt, &authority_checkpoint);
            self.last_atlas_frame_rejection = Some(AtlasFrameRejection {
                generation: rejected_generation,
                error,
            });
            self.refreshing = false;
            self.invalidator.set_phase(DrawPhase::None);
            self.layout_engine.as_mut().unwrap().clear();
            self.text_system().abort_frame();
            self.a11y.discard_candidate_frame();
            self.rollback_candidate_frame_transfers();
            self.next_frame.clear();
            self.restore_candidate_frame_authority(authority_checkpoint);
            self.append_focus_claim_resolutions(rejected_focus_resolutions);
            self.schedule_focus_claim_resolution_dispatch(cx);
            self.candidate_accessibility_focus = None;
            self.mouse_hit_test = self.rendered_frame.hit_test(self.mouse_position);
            if let Some(index) = restored_input_handler_index
                && let Some(input_handler) = self.rendered_frame.input_handlers[index].value.take()
            {
                self.platform_window.set_input_handler(input_handler);
            }
            self.rendered_frame
                .input_handlers
                .truncate(rendered_input_handlers_len);
            log::error!(
                target: "open_gpui::atlas",
                "rejected candidate attempt {:?} for frame generation {rejected_generation} after atlas lease failure: {error}",
                rejected_attempt.attempt_id
            );
            if self.fresh_initial_presentation_attempts_exhausted() {
                self.fail_fresh_initial_presentation(cx);
            } else {
                // The candidate did not commit, so every view rendered for it must remain dirty.
                self.refreshing = true;
                self.invalidator.set_dirty(true);
            }
            return ArenaClearNeeded::new(&cx.element_arena);
        }
        self.frame_focus_authority_sealed = true;
        debug_assert!(self.subtree_presentation_stack.borrow().is_empty());
        debug_assert!(self.subtree_transform_stack.borrow().is_empty());
        debug_assert!(self.scroll_ancestry_stack.borrow().is_empty());
        self.dirty_views.clear();
        self.next_frame.window_active = self.active.get();
        if mem::take(&mut self.candidate_pending_input_clear) {
            self.pending_input.take();
        }
        let notify_pending_input_changed =
            mem::take(&mut self.candidate_pending_input_notification);

        // Keep the frame slots in place because cached paint ranges address them by index.
        let mut rendered_input_handlers = self
            .rendered_frame
            .input_handlers
            .iter_mut()
            .map(|output| output.value.take())
            .collect::<Vec<_>>();
        let mut next_input_handlers = self
            .next_frame
            .input_handlers
            .iter_mut()
            .map(|output| (output.is_valid(), output.value.take()))
            .collect::<Vec<_>>();

        // Painting is complete. Cleanup callbacks may now schedule normal notifications.
        self.refreshing = false;
        self.invalidator.set_phase(DrawPhase::None);
        if notify_pending_input_changed {
            self.defer_pending_input_changed(cx);
        }
        if let Some(input_handler) = self.select_frame_input_handler_after_composition_cleanup(
            &mut rendered_input_handlers,
            &mut next_input_handlers,
            cx,
        ) {
            self.platform_window.set_input_handler(input_handler);
        }

        self.layout_engine.as_mut().unwrap().clear();
        self.text_system().finish_frame();
        if let Some((owner, refresh_after_drag_removal)) = candidate_stale_pointer_owner {
            if refresh_after_drag_removal {
                self.queue_pointer_session_cancellation(
                    owner,
                    PointerCancelReason::CaptureRevoked,
                    cx,
                );
            } else {
                self.queue_candidate_pointer_session_cancellation(
                    owner,
                    PointerCancelReason::CaptureRevoked,
                    cx,
                );
            }
        }
        self.next_frame.finish(&mut self.rendered_frame);
        self.candidate_frame_transfers = CandidateFrameTransfers::default();

        if self.focus.is_some_and(|focus| {
            self.next_frame
                .dispatch_tree
                .valid_focusable_node_id(focus)
                .is_none()
        }) {
            self.focus = None;
            self.clear_pending_keystrokes();
            self.refresh();
            let window = self.handle;
            cx.defer(move |cx| {
                window
                    .update(cx, |_, window, cx| window.pending_input_changed(cx))
                    .ok();
            });
        }

        // Settle one-generation focus authority before cached publications replay. A rejected
        // publication may observe that result, but it cannot renew the same request indefinitely.
        self.sealed_focus_retry_rejection = self.settle_focus_claim_for_candidate_generation();
        let prepaint_commit_plan = self.prepare_candidate_prepaint_commit_plan();

        // Ordinary normal-phase commits may prepare accepted-frame publications and follow-up
        // focus work, but cross-frame publications remain private until the candidate is swapped.
        self.commit_prepaint_non_publications(
            &prepaint_commit_plan,
            PrepaintCommitPhase::Normal,
            cx,
        );

        self.invalidator.set_phase(DrawPhase::Focus);
        let previous_committed_focus_path = self.rendered_frame.focus_path();
        let previous_window_active = self.rendered_frame.window_active;
        debug_assert_eq!(
            self.candidate_frame_transaction
                .as_ref()
                .expect("an accepted candidate must own a frame transaction")
                .frame_generation,
            self.next_frame.generation
        );
        mem::swap(&mut self.rendered_frame, &mut self.next_frame);
        self.candidate_frame_transaction
            .as_mut()
            .expect("the swapped candidate must retain its frame transaction")
            .mark_accepted();

        // Cross-frame publications now observe the frame that is actually visible. Focus-stable
        // commits remain ordered after every normal commit, including accepted publications.
        self.commit_prepaint_publications(&prepaint_commit_plan, cx);
        self.commit_prepaint_non_publications(
            &prepaint_commit_plan,
            PrepaintCommitPhase::FocusStable,
            cx,
        );
        self.sealed_focus_retry_rejection = None;
        let accepted_generation = self.rendered_frame.generation;
        self.discard_resolved_candidate_focus_claim(accepted_generation);
        let accepted_candidate = self
            .candidate_frame_transaction
            .take()
            .expect("an accepted candidate must own a frame transaction");
        debug_assert!(accepted_candidate.is_accepted());
        debug_assert_eq!(accepted_candidate.frame_generation, accepted_generation);
        self.next_frame.clear();
        if let Some((tree_update, activation_generation)) = candidate_a11y_update {
            log::debug!(
                "Sending a11y tree update: {} nodes",
                tree_update.nodes.len()
            );
            self.a11y.publish(&tree_update, activation_generation);
            self.platform_window.a11y_tree_update(tree_update);
        }
        self.frame_focus_authority_sealed = false;
        self.mouse_hit_test = self.rendered_frame.hit_test(self.mouse_position);
        self.commit_native_window_control_area(cx);
        let current_committed_focus_path = self.rendered_frame.focus_path();
        let previous_committed_focus = previous_committed_focus_path.last().copied();
        let current_committed_focus = current_committed_focus_path.last().copied();
        let current_window_active = self.rendered_frame.window_active;
        let focus_reveal_fence = self.take_pending_focus_reveal_fence(current_committed_focus);
        if previous_committed_focus != current_committed_focus
            && let Some(focus) = current_committed_focus
        {
            self.enqueue_focus_bring_into_view(focus, focus_reveal_fence, cx);
        }
        if previous_committed_focus_path != current_committed_focus_path
            || previous_window_active != current_window_active
        {
            if !previous_committed_focus_path.is_empty() && current_committed_focus_path.is_empty()
            {
                self.focus_lost_listeners
                    .clone()
                    .retain(&(), |listener| listener(self, cx));
            }

            let event = WindowFocusEvent {
                previous_focus_path: if previous_window_active {
                    previous_committed_focus_path.clone()
                } else {
                    Default::default()
                },
                current_focus_path: if current_window_active {
                    current_committed_focus_path.clone()
                } else {
                    Default::default()
                },
                previous_committed_focus_path,
                current_committed_focus_path,
            };
            self.focus_listeners
                .clone()
                .retain(&(), |listener| listener(&event, self, cx));
        }
        self.advance_bring_into_view_requests(cx);
        self.schedule_focus_claim_resolution_dispatch(cx);

        self.update_ime_position_from_committed_handler(cx);
        debug_assert!(self.rendered_entity_stack.is_empty());
        self.record_entities_accessed(cx);
        self.reset_cursor_style(cx);
        self.invalidator.set_phase(DrawPhase::None);
        let focus_followup_requested = mem::take(&mut self.focus_followup_requested);
        if focus_followup_requested && self.focus_followup_frame_needed() {
            self.refreshing = true;
            self.invalidator.set_focus_only_dirty();
        }
        if self.a11y.take_announcement_followup_refresh_required() {
            let window = self.handle;
            cx.defer(move |cx| {
                window.update(cx, |_, window, _| window.refresh()).ok();
            });
        }
        self.needs_present.set(true);
        self.presentation_state.frame_accepted_generation = Some(self.rendered_frame.generation);
        ArenaClearNeeded::new(&cx.element_arena)
    }

    fn select_frame_input_handler_after_composition_cleanup(
        &mut self,
        rendered: &mut [Option<PlatformInputHandler>],
        next: &mut [(bool, Option<PlatformInputHandler>)],
        cx: &mut App,
    ) -> Option<PlatformInputHandler> {
        loop {
            let selected = self.next_input_handler_index(next);
            let selected_focus = selected.map(|index| {
                next[index]
                    .1
                    .as_ref()
                    .expect("selected input handler must remain available")
                    .focus_id()
            });

            let mut selection_changed = false;
            for handler in rendered.iter_mut() {
                if handler
                    .as_ref()
                    .is_some_and(|handler| Some(handler.focus_id()) != selected_focus)
                {
                    handler
                        .take()
                        .expect("checked input handler must remain available")
                        .finish_composition(self, cx);
                    if self.next_input_handler_index(next) != selected {
                        selection_changed = true;
                        break;
                    }
                }
            }
            if selection_changed {
                continue;
            }

            for index in 0..next.len() {
                let handler = if Some(index) != selected {
                    next[index].1.take()
                } else {
                    None
                };
                if let Some(mut handler) = handler {
                    handler.finish_composition(self, cx);
                    if self.next_input_handler_index(next) != selected {
                        selection_changed = true;
                        break;
                    }
                }
            }
            if selection_changed {
                continue;
            }

            return selected.and_then(|index| next[index].1.take());
        }
    }

    fn next_input_handler_index(
        &self,
        handlers: &[(bool, Option<PlatformInputHandler>)],
    ) -> Option<usize> {
        let focus = self.focus?;
        self.next_frame.dispatch_tree.focusable_node_id(focus)?;
        handlers
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, (valid, handler))| {
                (*valid
                    && handler
                        .as_ref()
                        .is_some_and(|handler| handler.focus_id() == focus))
                .then_some(index)
            })
    }

    fn record_entities_accessed(&mut self, cx: &mut App) {
        let mut entities_ref = cx.entities.accessed_entities.get_mut();
        let mut entities = mem::take(entities_ref.deref_mut());
        let handle = self.handle;
        cx.record_entities_accessed(
            handle,
            // Try moving window invalidator into the Window
            self.invalidator.clone(),
            &entities,
        );
        let mut entities_ref = cx.entities.accessed_entities.get_mut();
        mem::swap(&mut entities, entities_ref.deref_mut());
    }

    fn invalidate_entities(&mut self) {
        let mut views = self.invalidator.take_views();
        for entity in views.drain() {
            self.mark_view_dirty(entity);
        }
        self.invalidator.replace_views(views);
    }

    #[profiling::function]
    fn present(&mut self) -> PlatformWindowPresentOutcome {
        let generation = self.rendered_frame.generation;
        if self.renderer_repaint_is_pending() {
            self.needs_present.set(false);
            profiling::finish_frame!();
            return PlatformWindowPresentOutcome::RepaintRequired;
        }
        let non_empty = self.rendered_frame.scene.has_primitives();
        let outcome = if self.presentation_is_allowed() {
            self.platform_window.draw(&self.rendered_frame.scene)
        } else {
            PlatformWindowPresentOutcome::Rejected
        };
        self.presentation_state.latest_present_attempt = Some(WindowPresentAttemptFacts {
            generation,
            outcome,
            contained_valid_primitives: non_empty,
        });
        if outcome == PlatformWindowPresentOutcome::RepaintRequired {
            let invalidated_generation = self
                .presentation_state
                .renderer_invalidated_generation
                .get_or_insert(generation);
            *invalidated_generation = (*invalidated_generation).max(generation);
            self.needs_present.set(false);
            self.refresh();
        } else if self
            .presentation_state
            .renderer_invalidated_generation
            .is_some_and(|invalidated_generation| generation > invalidated_generation)
        {
            self.presentation_state.renderer_invalidated_generation = None;
        }
        if outcome == PlatformWindowPresentOutcome::Submitted {
            self.presentation_state.present_submitted_generation = Some(generation);
            if non_empty {
                self.presentation_state.non_empty_presented_generation = Some(generation);
                if let Some(ticket) = self
                    .presentation_state
                    .provisional_reveal_ticket
                    .as_ref()
                    .filter(|ticket| ticket.bind_presentation(generation))
                {
                    if let Some(session) = self.provisional_session.as_ref() {
                        let snapshot = session.snapshot();
                        if snapshot.window_id() == Some(self.handle.window_id())
                            && snapshot.phase() == WindowProvisionalSessionPhase::Gated
                        {
                            self.platform_command_sink.enqueue_provisional_reveal(
                                PlatformWindowCommand::RevealDeferredInitialPresentation {
                                    session_generation: snapshot.generation(),
                                    presentation_generation: generation,
                                },
                                ticket.clone(),
                            );
                        } else if snapshot.phase() == WindowProvisionalSessionPhase::Terminal {
                            ticket.settle(WindowProvisionalRevealOutcome::WindowTerminal);
                        } else {
                            ticket.settle(WindowProvisionalRevealOutcome::Stale);
                        }
                    } else {
                        ticket.settle(WindowProvisionalRevealOutcome::Stale);
                    }
                }
            }
            self.needs_present.set(false);
        } else if outcome != PlatformWindowPresentOutcome::RepaintRequired {
            self.needs_present.set(true);
        }
        #[cfg(feature = "input-latency-histogram")]
        if outcome == PlatformWindowPresentOutcome::Submitted {
            self.input_latency_tracker.record_frame_presented();
        }
        profiling::finish_frame!();
        outcome
    }

    /// Returns a snapshot of the current input-latency histograms.
    #[cfg(feature = "input-latency-histogram")]
    pub fn input_latency_snapshot(&self) -> InputLatencySnapshot {
        self.input_latency_tracker.snapshot()
    }

    fn draw_roots(
        &mut self,
        cx: &mut App,
    ) -> (
        Option<(accesskit::TreeUpdate, u64)>,
        Option<(PointerCaptureHandle, bool)>,
    ) {
        self.invalidator.set_phase(DrawPhase::Prepaint);
        self.tooltip_bounds.take();

        self.a11y.sync_active_flag();
        if self.a11y.is_active() {
            self.a11y.begin_frame();
        }

        let _inspector_width: Pixels = rems(30.0).to_pixels(self.rem_size());
        let root_size = {
            #[cfg(any(feature = "inspector", debug_assertions))]
            {
                if self.inspector.is_some() {
                    let mut size = self.viewport_size;
                    size.width = (size.width - _inspector_width).max(px(0.0));
                    size
                } else {
                    self.viewport_size
                }
            }
            #[cfg(not(any(feature = "inspector", debug_assertions)))]
            {
                self.viewport_size
            }
        };

        // Layout all root elements.
        let mut root_element = self.root.as_ref().unwrap().clone().into_any();
        root_element.prepaint_as_root(Point::default(), root_size.into(), self, cx);

        #[cfg(any(feature = "inspector", debug_assertions))]
        let inspector_element = self.prepaint_inspector(_inspector_width, cx);

        self.prepaint_deferred_draws(cx);

        let mut prompt_element = None;
        if let Some(prompt) = self.prompt.take() {
            let mut element = prompt.view.any_view().into_any();
            element.prepaint_as_root(Point::default(), root_size.into(), self, cx);
            prompt_element = Some(element);
            self.prompt = Some(prompt);
        }

        let stale_pointer_owner = self.stale_pointer_session_owner_in_candidate_frame(cx);

        let mut active_drag_element = None;
        let mut tooltip_element = None;
        let window_owns_active_drag = cx
            .active_drag
            .as_ref()
            .is_some_and(|drag| drag.window_id == self.handle.window_id());
        if prompt_element.is_none() && window_owns_active_drag && stale_pointer_owner.is_none() {
            let active_drag = cx
                .active_drag
                .take()
                .expect("window-owned active drag should remain available");
            let mut element = active_drag.view.clone().into_any();
            let offset = self.mouse_position() - active_drag.window_preview_offset;
            element.prepaint_as_root(offset, AvailableSpace::min_size(), self, cx);
            active_drag_element = Some(element);
            cx.active_drag = Some(active_drag);
        } else if prompt_element.is_none() && !window_owns_active_drag {
            tooltip_element = self.prepaint_tooltip(cx);
        }

        // Cached subtrees replay dispatch nodes without calling `set_focus_handle`, so give a
        // one-frame claim one final prepaint qualification point before paint installs handlers.
        self.promote_pending_focus_claim();
        self.mouse_hit_test = self.next_frame.hit_test(self.mouse_position);

        // Now actually paint the elements.
        self.invalidator.set_phase(DrawPhase::Paint);
        root_element.paint(self, cx);

        #[cfg(any(feature = "inspector", debug_assertions))]
        self.paint_inspector(inspector_element, cx);

        self.paint_deferred_draws(cx);

        let mut active_drag_preview_painted = false;
        if let Some(mut prompt_element) = prompt_element {
            prompt_element.paint(self, cx);
        } else if let Some(mut drag_element) = active_drag_element {
            active_drag_preview_painted = true;
            drag_element.paint(self, cx);
        } else if let Some(mut tooltip) = tooltip_element
            && tooltip
                .validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
        {
            self.with_resolved_subtree_transform(
                ResolvedSubtreeTransform::IDENTITY,
                tooltip.validity,
                |window| tooltip.element.paint(window, cx),
            );
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        self.paint_inspector_hitbox(cx);

        // Paint can invalidate a transform that qualified during prepaint. Commit focus only
        // after those late validity results are known, then resolve accessibility from the same
        // final authority.
        self.resolve_provisional_focus_claim(cx);
        self.a11y
            .resolve_focus(self.candidate_accessibility_focus.or(self.focus));

        let stale_pointer_owner = stale_pointer_owner.map(|owner| (owner, false)).or_else(|| {
            self.stale_pointer_session_owner_in_candidate_frame(cx)
                .map(|owner| (owner, active_drag_preview_painted))
        });

        // a11y may have been activated/deactivated halfway through the frame
        let a11y_active_start_of_frame = self.a11y.is_active();
        let a11y_generation_start_of_frame = self.a11y.activation_generation();
        self.a11y.sync_active_flag();
        let a11y_active_end_of_frame = self.a11y.is_active();
        let a11y_generation_end_of_frame = self.a11y.activation_generation();

        let should_send_a11y_update = a11y_active_start_of_frame
            && a11y_active_end_of_frame
            && a11y_generation_start_of_frame == a11y_generation_end_of_frame;

        let a11y_update = if a11y_active_start_of_frame {
            // clear the builder state regardless
            let tree_update = self.a11y.end_frame();

            if should_send_a11y_update {
                Some((tree_update, a11y_generation_start_of_frame))
            } else {
                self.a11y.discard_candidate_frame();
                None
            }
        } else {
            None
        };
        (a11y_update, stale_pointer_owner)
    }

    fn stale_pointer_session_owner_in_candidate_frame(
        &self,
        cx: &App,
    ) -> Option<PointerCaptureHandle> {
        let stale_capture_owner = self.captured_pointer.and_then(|captured| {
            self.pointer_capture_hitbox_for_handle_in_frame(captured.handle(), &self.next_frame)
                .is_none()
                .then_some(captured.handle())
        });
        let stale_drag_owner = cx.active_drag.as_ref().and_then(|drag| {
            (drag.window_id == self.handle.window_id())
                .then_some(drag.source)
                .flatten()
                .filter(|owner| {
                    self.pointer_capture_hitbox_for_handle_in_frame(*owner, &self.next_frame)
                        .is_none()
                })
        });
        stale_capture_owner.or(stale_drag_owner)
    }

    fn prepare_candidate_prepaint_commit_plan(&mut self) -> Rc<CandidatePrepaintCommitPlan> {
        let current_commits = self.next_frame.prepaint_commits.clone();
        let previous_commits = self.rendered_frame.prepaint_commits.clone();
        self.candidate_frame_transaction
            .as_mut()
            .expect("one candidate frame transaction must own its prepaint commit plan")
            .prepare_prepaint_commits(current_commits, previous_commits)
    }

    fn commit_prepaint_non_publications(
        &mut self,
        plan: &CandidatePrepaintCommitPlan,
        phase: PrepaintCommitPhase,
        cx: &mut App,
    ) {
        for output in &plan.current_commits {
            if output.value.publication.is_some() || output.value.phase != phase {
                continue;
            }
            self.commit_prepaint_output(output, plan.target_revision, None, phase, cx);
        }
    }

    fn commit_prepaint_publications(&mut self, plan: &CandidatePrepaintCommitPlan, cx: &mut App) {
        assert_eq!(
            self.rendered_frame.generation, plan.target_revision,
            "accepted publications must be committed against the frame that was just swapped"
        );
        let accepted_frame =
            AcceptedFrameFence::new(self.handle.window_id(), self.rendered_frame.generation);
        let current_publications = plan
            .current_commits
            .iter()
            .filter_map(|output| output.value.publication)
            .collect::<FxHashSet<_>>();
        let mut expired_publications = FxHashSet::default();
        for output in &plan.previous_commits {
            let Some(publication) = output.value.publication else {
                continue;
            };
            if !output.is_valid()
                || current_publications.contains(&publication)
                || !expired_publications.insert(publication)
            {
                continue;
            }
            if let Some(discard) = output.value.discard.clone() {
                self.run_prepaint_commit_callback(
                    discard,
                    plan.target_revision,
                    Some(accepted_frame),
                    PrepaintCommitPhase::Normal,
                    SubtreePresentation::Hidden,
                    cx,
                );
            }
        }

        for output in &plan.current_commits {
            if output.value.publication.is_none() {
                continue;
            }
            self.commit_prepaint_output(
                output,
                plan.target_revision,
                Some(accepted_frame),
                PrepaintCommitPhase::Normal,
                cx,
            );
        }
    }

    fn commit_prepaint_output(
        &mut self,
        output: &FrameOutput<PrepaintCommit>,
        target_revision: u64,
        accepted_frame: Option<AcceptedFrameFence>,
        phase: PrepaintCommitPhase,
        cx: &mut App,
    ) {
        let (callback, presentation) = if output.is_valid() {
            (output.value.commit.clone(), output.value.presentation)
        } else if let Some(discard) = output.value.discard.clone() {
            (discard, SubtreePresentation::Hidden)
        } else {
            return;
        };
        self.run_prepaint_commit_callback(
            callback,
            target_revision,
            accepted_frame,
            phase,
            presentation,
            cx,
        );
    }

    fn run_prepaint_commit_callback(
        &mut self,
        callback: PrepaintCommitCallback,
        target_revision: u64,
        accepted_frame: Option<AcceptedFrameFence>,
        phase: PrepaintCommitPhase,
        presentation: SubtreePresentation,
        cx: &mut App,
    ) {
        self.with_prepaint_commit_phase(phase, |window| {
            window.with_subtree_presentation(presentation, |window| match callback {
                PrepaintCommitCallback::Revision(callback) => callback(target_revision, window, cx),
                PrepaintCommitCallback::AcceptedFrame(callback) => callback(
                    accepted_frame.expect(
                        "accepted-frame publication callbacks require a committed candidate fence",
                    ),
                    window,
                    cx,
                ),
            })
        });
    }

    fn with_prepaint_commit_phase<T>(
        &mut self,
        phase: PrepaintCommitPhase,
        callback: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let current = self.prepaint_commit_phase.clone();
        let _guard = PrepaintCommitPhaseScopeGuard {
            previous: current.replace(Some(phase)),
            current,
            entered: phase,
        };
        callback(self)
    }

    fn prepaint_tooltip(&mut self, cx: &mut App) -> Option<PreparedTooltip> {
        // Use indexing instead of iteration to avoid borrowing self for the duration of the loop.
        for tooltip_request_index in (0..self.next_frame.tooltip_requests.len()).rev() {
            let Some(Some(tooltip_request)) = self
                .next_frame
                .tooltip_requests
                .get(tooltip_request_index)
                .cloned()
            else {
                log::error!("Unexpectedly absent TooltipRequest");
                continue;
            };
            if tooltip_request
                .validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
            {
                continue;
            }
            let mut element = tooltip_request.tooltip.view.clone().into_any();
            let mouse_position = tooltip_request.tooltip.mouse_position;
            let tooltip_size = element.layout_as_root(AvailableSpace::min_size(), self, cx);

            let mut tooltip_bounds =
                Bounds::new(mouse_position + point(px(1.), px(1.)), tooltip_size);
            let window_bounds = Bounds {
                origin: Point::default(),
                size: self.viewport_size(),
            };

            if tooltip_bounds.right() > window_bounds.right() {
                let new_x = mouse_position.x - tooltip_bounds.size.width - px(1.);
                if new_x >= Pixels::ZERO {
                    tooltip_bounds.origin.x = new_x;
                } else {
                    tooltip_bounds.origin.x = cmp::max(
                        Pixels::ZERO,
                        tooltip_bounds.origin.x - tooltip_bounds.right() - window_bounds.right(),
                    );
                }
            }

            if tooltip_bounds.bottom() > window_bounds.bottom() {
                let new_y = mouse_position.y - tooltip_bounds.size.height - px(1.);
                if new_y >= Pixels::ZERO {
                    tooltip_bounds.origin.y = new_y;
                } else {
                    tooltip_bounds.origin.y = cmp::max(
                        Pixels::ZERO,
                        tooltip_bounds.origin.y - tooltip_bounds.bottom() - window_bounds.bottom(),
                    );
                }
            }

            // It's possible for an element to have an active tooltip while not being painted (e.g.
            // via the `visible_on_hover` method). Since mouse listeners are not active in this
            // case, instead update the tooltip's visibility here.
            let is_visible =
                (tooltip_request.tooltip.check_visible_and_update)(tooltip_bounds, self, cx);
            if !is_visible {
                continue;
            }

            self.with_resolved_subtree_transform(
                ResolvedSubtreeTransform::IDENTITY,
                tooltip_request.validity.clone(),
                |window| {
                    window.with_absolute_element_offset(tooltip_bounds.origin, |window| {
                        element.prepaint(window, cx)
                    })
                },
            );

            self.tooltip_bounds = Some(TooltipBounds {
                id: tooltip_request.id,
                bounds: tooltip_bounds,
                validity: tooltip_request.validity.clone(),
            });
            return Some(PreparedTooltip {
                element,
                validity: tooltip_request.validity,
            });
        }
        None
    }

    fn prepaint_deferred_draws(&mut self, cx: &mut App) {
        assert_eq!(self.element_id_stack.len(), 0);

        let mut completed_draws = Vec::new();

        // Process deferred draws in multiple rounds to support nesting.
        // Each round processes all current deferred draws, which may produce new ones.
        let mut depth = 0;
        loop {
            let deferred_count = self.next_frame.deferred_draws.len();
            if deferred_count == 0 {
                break;
            }
            // Limit maximum nesting depth to prevent infinite loops.
            assert!(depth < 10, "Exceeded maximum (10) deferred depth");
            depth += 1;

            // Sort by priority for this round
            let traversal_order = self.deferred_draw_traversal_order();
            let mut deferred_draws = mem::take(&mut self.next_frame.deferred_draws);

            for deferred_draw_ix in traversal_order {
                let deferred_draw = &mut deferred_draws[deferred_draw_ix];
                let accessibility_tree_scope = deferred_draw.accessibility_tree_scope;
                let subtree_presentation = deferred_draw.subtree_presentation;
                let subtree_transform = deferred_draw.subtree_transform;
                let subtree_geometry_validity = deferred_draw.subtree_geometry_validity.clone();
                let scroll_ancestry = deferred_draw.scroll_ancestry.clone();
                let accessibility_parent = deferred_draw.accessibility_parent.clone();
                let accessibility_proxy_clip_owner = deferred_draw.accessibility_proxy_clip_owner;
                self.element_id_stack
                    .clone_from(&deferred_draw.element_id_stack);
                self.text_style_stack
                    .clone_from(&deferred_draw.text_style_stack);
                self.next_frame
                    .dispatch_tree
                    .set_active_node(deferred_draw.parent_node);

                let prepaint_start = self.prepaint_index();
                if subtree_geometry_validity
                    .as_ref()
                    .is_some_and(|validity| !validity.is_valid())
                {
                    // The owning transform scope already failed elsewhere in this frame.
                } else if let Some(element) = deferred_draw.element.as_mut() {
                    let result = self.with_scroll_ancestry(scroll_ancestry, |window| {
                        window.transact_subtree_geometry(
                            subtree_geometry_validity.clone(),
                            |window| {
                                window.with_subtree_presentation(subtree_presentation, |window| {
                                    window.with_resolved_subtree_transform(
                                        subtree_transform,
                                        subtree_geometry_validity.clone(),
                                        |window| {
                                            window.with_rendered_view(
                                                deferred_draw.current_view,
                                                |window| {
                                                    window.with_rem_size(
                                                        Some(deferred_draw.rem_size),
                                                        |window| {
                                                            window.with_absolute_element_offset(
                                                                deferred_draw.absolute_offset,
                                                                |window| {
                                                                    window.with_resolved_clip_stack(
                                                                        deferred_draw
                                                                            .clip_stack
                                                                            .clone(),
                                                                        |window| {
                                                                            window.with_accessibility_tree_scope(
                                                                                accessibility_tree_scope,
                                                                                |window| {
                                                                                    window.with_accessibility_deferred_parent_scope(
                                                                                        accessibility_parent,
                                                                                        |window| {
                                                                                            window.with_accessibility_clip_owner_scope(
                                                                                                accessibility_proxy_clip_owner,
                                                                                                |window| element.prepaint(window, cx),
                                                                                            )
                                                                                        },
                                                                                    )
                                                                                },
                                                                            );
                                                                        },
                                                                    );
                                                                },
                                                            );
                                                        },
                                                    );
                                                },
                                            )
                                        },
                                    );
                                });
                            },
                        )
                    });
                    if result.is_err()
                        && let Some(validity) = subtree_geometry_validity.as_ref()
                    {
                        self.record_subtree_geometry_scope_diagnostic(validity);
                    }
                } else {
                    self.reuse_prepaint(deferred_draw.prepaint_range.clone());
                }
                let prepaint_end = self.prepaint_index();
                deferred_draw.prepaint_range = prepaint_start..prepaint_end;
            }

            // Save completed draws and continue with newly added ones
            completed_draws.append(&mut deferred_draws);

            self.element_id_stack.clear();
            self.text_style_stack.clear();
        }

        // Restore all completed draws
        self.next_frame.deferred_draws = completed_draws;
    }

    fn paint_deferred_draws(&mut self, cx: &mut App) {
        assert_eq!(self.element_id_stack.len(), 0);

        // Paint all deferred draws in priority order.
        // Since prepaint has already processed nested deferreds, we just paint them all.
        if self.next_frame.deferred_draws.len() == 0 {
            return;
        }

        let traversal_order = self.deferred_draw_traversal_order();
        let mut deferred_draws = mem::take(&mut self.next_frame.deferred_draws);
        for deferred_draw_ix in traversal_order {
            let mut deferred_draw = &mut deferred_draws[deferred_draw_ix];
            self.element_id_stack
                .clone_from(&deferred_draw.element_id_stack);
            self.next_frame
                .dispatch_tree
                .set_active_node(deferred_draw.parent_node);

            self.with_atlas_texture_lease_paint_scope(|window| {
                let paint_start = window.paint_index();
                let clip_stack = deferred_draw.clip_stack.clone();
                let subtree_presentation = deferred_draw.subtree_presentation;
                let paint_succeeded = if deferred_draw
                    .subtree_geometry_validity
                    .as_ref()
                    .is_some_and(|validity| !validity.is_valid())
                {
                    // The owning transform scope is layout-only for this frame.
                    true
                } else if let Some(element) = deferred_draw.element.as_mut() {
                    window.with_subtree_presentation(subtree_presentation, |window| {
                        window.with_resolved_subtree_transform(
                            deferred_draw.subtree_transform,
                            deferred_draw.subtree_geometry_validity.clone(),
                            |window| {
                                window.with_rendered_view(deferred_draw.current_view, |window| {
                                    window.with_resolved_clip_stack(clip_stack, |window| {
                                        window.with_rem_size(
                                            Some(deferred_draw.rem_size),
                                            |window| {
                                                element.paint(window, cx);
                                            },
                                        );
                                    })
                                })
                            },
                        );
                    });
                    if let Some(validity) = deferred_draw.subtree_geometry_validity.as_ref() {
                        window.record_subtree_geometry_scope_diagnostic(validity);
                    }
                    true
                } else {
                    let replayed = window.reuse_paint(deferred_draw.paint_range.clone());
                    if !replayed {
                        let view_id = deferred_draw.current_view;
                        window.on_next_frame(move |_, cx| cx.notify(view_id));
                    }
                    replayed
                };
                let paint_end = window.paint_index();
                if paint_succeeded {
                    deferred_draw.paint_range = paint_start..paint_end;
                }
            });
        }
        self.next_frame.deferred_draws = deferred_draws;
        self.element_id_stack.clear();
    }

    fn deferred_draw_traversal_order(&mut self) -> SmallVec<[usize; 8]> {
        let deferred_count = self.next_frame.deferred_draws.len();
        let mut sorted_indices = (0..deferred_count).collect::<SmallVec<[_; 8]>>();
        sorted_indices.sort_by_key(|ix| self.next_frame.deferred_draws[*ix].priority);
        sorted_indices
    }

    pub(crate) fn prepaint_index(&self) -> PrepaintStateIndex {
        PrepaintStateIndex {
            hitboxes_index: self.next_frame.hitboxes.len(),
            pointer_capture_bindings_index: self.next_frame.pointer_capture_bindings.len(),
            portal_anchor_bindings_index: self.next_frame.portal_anchor_bindings.len(),
            reveal_target_bindings_index: self.next_frame.reveal_target_bindings.len(),
            retained_resources_index: self.next_frame.retained_resources.len(),
            prepaint_commits_index: self.next_frame.prepaint_commits.len(),
            tooltips_index: self.next_frame.tooltip_requests.len(),
            deferred_draws_index: self.next_frame.deferred_draws.len(),
            dispatch_tree_index: self.next_frame.dispatch_tree.len(),
            accessed_element_states_index: self.next_frame.accessed_element_states.len(),
            line_layout_index: self.text_system.layout_index(),
            subtree_transform_diagnostics_index: self
                .next_frame
                .subtree_transform_diagnostics
                .len(),
        }
    }

    pub(crate) fn reuse_prepaint(&mut self, range: Range<PrepaintStateIndex>) {
        let validity = self.subtree_geometry_validity();
        let presentation = self.subtree_presentation();
        self.next_frame.hitboxes.extend(
            self.rendered_frame.hitboxes[range.start.hitboxes_index..range.end.hitboxes_index]
                .iter()
                .cloned()
                .map(|mut hitbox| {
                    hitbox.retag_validity(validity.clone());
                    hitbox
                }),
        );
        let reused_pointer_capture_bindings = self.rendered_frame.pointer_capture_bindings
            [range.start.pointer_capture_bindings_index..range.end.pointer_capture_bindings_index]
            .iter()
            .copied();
        for binding in reused_pointer_capture_bindings {
            assert!(
                !self
                    .next_frame
                    .pointer_capture_bindings
                    .iter()
                    .any(|(id, hitbox)| *id == binding.0 || *hitbox == binding.1),
                "a pointer capture handle or hitbox was bound more than once in one frame"
            );
            self.next_frame.pointer_capture_bindings.push(binding);
        }
        let frame_generation = self.next_frame.generation;
        let reused_portal_anchor_bindings = self.rendered_frame.portal_anchor_bindings
            [range.start.portal_anchor_bindings_index..range.end.portal_anchor_bindings_index]
            .iter()
            .map(|output| {
                FrameOutput::new(
                    output.value.replayed(frame_generation),
                    SubtreeGeometryValidity::replayed_under(
                        output.validity.as_ref(),
                        validity.clone(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        for binding in reused_portal_anchor_bindings {
            self.next_frame.record_portal_anchor_binding(binding);
        }
        let frame_generation = self.next_frame.generation;
        let reused_reveal_target_bindings = self.rendered_frame.reveal_target_bindings
            [range.start.reveal_target_bindings_index..range.end.reveal_target_bindings_index]
            .iter()
            .map(|output| {
                FrameOutput::new(
                    output.value.replayed(frame_generation),
                    SubtreeGeometryValidity::replayed_under(
                        output.validity.as_ref(),
                        validity.clone(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        for binding in reused_reveal_target_bindings {
            self.next_frame.record_reveal_target_binding(binding);
        }
        self.next_frame.retained_resources.extend(
            self.rendered_frame.retained_resources
                [range.start.retained_resources_index..range.end.retained_resources_index]
                .iter()
                .cloned(),
        );
        self.next_frame.prepaint_commits.extend(
            self.rendered_frame.prepaint_commits
                [range.start.prepaint_commits_index..range.end.prepaint_commits_index]
                .iter()
                .map(|output| {
                    let mut commit = output.value.clone();
                    commit.presentation = commit.presentation.resolve_under(presentation);
                    FrameOutput::new(
                        commit,
                        SubtreeGeometryValidity::replayed_under(
                            output.validity.as_ref(),
                            validity.clone(),
                        ),
                    )
                }),
        );
        self.next_frame.tooltip_requests.extend(
            self.rendered_frame.tooltip_requests
                [range.start.tooltips_index..range.end.tooltips_index]
                .iter()
                .map(|request| {
                    request.clone().map(|mut request| {
                        request.validity = SubtreeGeometryValidity::replayed_under(
                            request.validity.as_ref(),
                            validity.clone(),
                        );
                        request
                    })
                }),
        );
        let accessed_element_states = self.rendered_frame.accessed_element_states
            [range.start.accessed_element_states_index..range.end.accessed_element_states_index]
            .to_vec();
        for key in accessed_element_states {
            self.next_frame.accessed_element_states.push(key.clone());
            let recorded_validity = self
                .rendered_frame
                .element_state_validities
                .get(&key)
                .and_then(Option::as_ref);
            self.next_frame.element_state_validities.insert(
                key,
                SubtreeGeometryValidity::replayed_under(recorded_validity, validity.clone()),
            );
        }
        self.text_system
            .reuse_layouts(range.start.line_layout_index..range.end.line_layout_index);
        self.next_frame.subtree_transform_diagnostics.extend(
            self.rendered_frame.subtree_transform_diagnostics[range
                .start
                .subtree_transform_diagnostics_index
                ..range.end.subtree_transform_diagnostics_index]
                .iter()
                .copied()
                .map(|mut diagnostic| {
                    diagnostic.frame_generation = self.next_frame.generation;
                    diagnostic
                }),
        );

        let reused_subtree = self.next_frame.dispatch_tree.reuse_subtree(
            range.start.dispatch_tree_index..range.end.dispatch_tree_index,
            &self.rendered_frame.dispatch_tree,
            self.focus,
            validity.clone(),
        );

        if reused_subtree.contains_focus() {
            self.next_frame.focus = self.focus;
        }

        self.next_frame.deferred_draws.extend(
            self.rendered_frame.deferred_draws
                [range.start.deferred_draws_index..range.end.deferred_draws_index]
                .iter()
                .map(|deferred_draw| DeferredDraw {
                    current_view: deferred_draw.current_view,
                    parent_node: reused_subtree.refresh_node_id(deferred_draw.parent_node),
                    element_id_stack: deferred_draw.element_id_stack.clone(),
                    text_style_stack: deferred_draw.text_style_stack.clone(),
                    accessibility_tree_scope: deferred_draw.accessibility_tree_scope,
                    accessibility_parent: deferred_draw.accessibility_parent.clone(),
                    accessibility_proxy_clip_owner: deferred_draw.accessibility_proxy_clip_owner,
                    clip_stack: deferred_draw.clip_stack.clone(),
                    rem_size: deferred_draw.rem_size,
                    priority: deferred_draw.priority,
                    element: None,
                    absolute_offset: deferred_draw.absolute_offset,
                    subtree_presentation: deferred_draw.subtree_presentation,
                    subtree_transform: deferred_draw.subtree_transform,
                    subtree_geometry_validity: SubtreeGeometryValidity::replayed_under(
                        deferred_draw.subtree_geometry_validity.as_ref(),
                        validity.clone(),
                    ),
                    scroll_ancestry: deferred_draw.scroll_ancestry.clone(),
                    prepaint_range: deferred_draw.prepaint_range.clone(),
                    paint_range: deferred_draw.paint_range.clone(),
                }),
        );
    }

    pub(crate) fn paint_index(&self) -> PaintIndex {
        PaintIndex {
            scene_index: self.next_frame.scene.journal_len(),
            atlas_texture_lease_entries_index: self.next_frame.atlas_texture_lease_entries.len(),
            atlas_access_diagnostics_index: self.next_frame.atlas_access_diagnostic_entries.len(),
            image_paint_diagnostics_index: self.next_frame.image_paint_diagnostic_entries.len(),
            retained_visual_publications_index: self.next_frame.retained_visual_publications.len(),
            retained_visual_replays_index: self.next_frame.retained_visual_replays.len(),
            mouse_listeners_index: self.next_frame.mouse_listeners.len(),
            pointer_cancel_listeners_index: self.next_frame.pointer_cancel_listeners.len(),
            input_handlers_index: self.next_frame.input_handlers.len(),
            cursor_styles_index: self.next_frame.cursor_styles.len(),
            window_control_hitboxes_index: self.next_frame.window_control_hitboxes.len(),
            #[cfg(any(test, feature = "test-support"))]
            debug_bounds_entries_index: self.next_frame.debug_bounds_entries.len(),
            #[cfg(any(test, feature = "test-support"))]
            debug_focus_entries_index: self.next_frame.debug_focus_entries.len(),
            accessed_element_states_index: self.next_frame.accessed_element_states.len(),
            tab_handle_index: self.next_frame.tab_stops.paint_index(),
            line_layout_index: self.text_system.layout_index(),
            subtree_transform_diagnostics_index: self
                .next_frame
                .subtree_transform_diagnostics
                .len(),
        }
    }

    pub(crate) fn with_atlas_texture_lease_paint_scope<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let entered_depth = {
            let mut stack = self.atlas_texture_lease_paint_scopes.borrow_mut();
            let entered_depth = stack.len();
            stack.push(FxHashSet::default());
            entered_depth
        };
        let _guard = AtlasTextureLeasePaintScopeGuard {
            stack: self.atlas_texture_lease_paint_scopes.clone(),
            entered_depth,
        };
        f(self)
    }

    fn record_atlas_texture_lease_entry(&mut self, entry: frame_journal::AtlasTextureLeaseEntry) {
        let should_record = {
            let mut scopes = self.atlas_texture_lease_paint_scopes.borrow_mut();
            let Some(scope) = scopes.last_mut() else {
                return;
            };
            match &entry {
                Ok(lease) => lease
                    .texture_instances()
                    .iter()
                    .fold(false, |new_dependency, texture| {
                        scope.insert(*texture) || new_dependency
                    }),
                Err(
                    AtlasTextureLeaseError::TextureUnavailable { texture, .. }
                    | AtlasTextureLeaseError::LeaseCountOverflow { texture, .. },
                ) => scope.insert(*texture),
            }
        };
        if should_record {
            self.next_frame.atlas_texture_lease_entries.push(entry);
        }
    }

    pub(crate) fn can_reuse_paint(&self, range: &Range<PaintIndex>) -> bool {
        self.rendered_frame
            .atlas_texture_lease_entries
            .get(
                range.start.atlas_texture_lease_entries_index
                    ..range.end.atlas_texture_lease_entries_index,
            )
            .is_some_and(|entries| {
                let mut validated_instances = FxHashSet::default();
                entries.iter().all(|entry| {
                    let Ok(lease) = entry else {
                        return false;
                    };
                    let introduces_dependency =
                        lease
                            .texture_instances()
                            .iter()
                            .fold(false, |new_dependency, texture| {
                                validated_instances.insert(*texture) || new_dependency
                            });
                    !introduces_dependency || lease.validate().is_ok()
                })
            })
    }

    pub(crate) fn reuse_paint(&mut self, range: Range<PaintIndex>) -> bool {
        if !self.can_reuse_paint(&range) {
            return false;
        }

        let retained_visual_replays = self.rendered_frame.retained_visual_replays
            [range.start.retained_visual_replays_index..range.end.retained_visual_replays_index]
            .to_vec();
        let replay_identity_conflicts = {
            let candidate = self
                .candidate_frame_transaction
                .as_ref()
                .expect("cached paint replay must run inside one candidate frame transaction");
            let mut range_identities = FxHashSet::default();
            retained_visual_replays.iter().any(|identity| {
                !range_identities.insert(*identity)
                    || candidate.retained_visual_was_replayed(*identity)
            })
        };
        if replay_identity_conflicts {
            return false;
        }

        let validity = self.subtree_geometry_validity();
        let atlas_texture_lease_entries = self.rendered_frame.atlas_texture_lease_entries[range
            .start
            .atlas_texture_lease_entries_index
            ..range.end.atlas_texture_lease_entries_index]
            .to_vec();
        let retained_visual_publications = self.rendered_frame.retained_visual_publications[range
            .start
            .retained_visual_publications_index
            ..range.end.retained_visual_publications_index]
            .iter()
            .map(|publication| publication.replayed_under(validity.clone()))
            .collect::<Vec<_>>();
        let window_control_start = self.next_frame.window_control_hitboxes.len();
        self.next_frame.window_control_hitboxes.extend(
            self.rendered_frame.window_control_hitboxes[range.start.window_control_hitboxes_index
                ..range.end.window_control_hitboxes_index]
                .iter()
                .cloned(),
        );
        for (_, hitbox) in &mut self.next_frame.window_control_hitboxes[window_control_start..] {
            hitbox.retag_validity(validity.clone());
        }
        #[cfg(any(test, feature = "test-support"))]
        {
            let bounds_entries = self.rendered_frame.debug_bounds_entries
                [range.start.debug_bounds_entries_index..range.end.debug_bounds_entries_index]
                .to_vec();
            for (selector, bounds, recorded_validity) in bounds_entries {
                self.next_frame
                    .debug_bounds
                    .insert(selector.clone(), bounds);
                self.next_frame.debug_bounds_entries.push((
                    selector,
                    bounds,
                    SubtreeGeometryValidity::replayed_under(
                        recorded_validity.as_ref(),
                        validity.clone(),
                    ),
                ));
            }
            let focus_entries = self.rendered_frame.debug_focus_entries
                [range.start.debug_focus_entries_index..range.end.debug_focus_entries_index]
                .to_vec();
            for (selector, focus_id, recorded_validity) in focus_entries {
                self.next_frame
                    .debug_focus_handles
                    .insert(selector.clone(), focus_id);
                self.next_frame.debug_focus_entries.push((
                    selector,
                    focus_id,
                    SubtreeGeometryValidity::replayed_under(
                        recorded_validity.as_ref(),
                        validity.clone(),
                    ),
                ));
            }
        }
        self.next_frame.cursor_styles.extend(
            self.rendered_frame.cursor_styles
                [range.start.cursor_styles_index..range.end.cursor_styles_index]
                .iter()
                .cloned()
                .map(|mut request| {
                    request.validity = SubtreeGeometryValidity::replayed_under(
                        request.validity.as_ref(),
                        validity.clone(),
                    );
                    request
                }),
        );
        for rendered_index in range.start.input_handlers_index..range.end.input_handlers_index {
            let (mut handler, replayed_validity) = {
                let output = &mut self.rendered_frame.input_handlers[rendered_index];
                (
                    output.value.take(),
                    SubtreeGeometryValidity::replayed_under(
                        output.validity.as_ref(),
                        validity.clone(),
                    ),
                )
            };
            if let Some(handler) = handler.as_mut() {
                handler.set_validity(replayed_validity.clone());
            }
            let candidate_index = self.next_frame.input_handlers.len();
            if handler.is_some() {
                self.candidate_frame_transfers
                    .input_handlers
                    .push((rendered_index, candidate_index));
            }
            self.next_frame
                .input_handlers
                .push(FrameOutput::new(handler, replayed_validity));
        }
        for rendered_index in range.start.mouse_listeners_index..range.end.mouse_listeners_index {
            let (listener, replayed_validity) = {
                let output = &mut self.rendered_frame.mouse_listeners[rendered_index];
                (
                    output.value.take(),
                    SubtreeGeometryValidity::replayed_under(
                        output.validity.as_ref(),
                        validity.clone(),
                    ),
                )
            };
            let candidate_index = self.next_frame.mouse_listeners.len();
            if listener.is_some() {
                self.candidate_frame_transfers
                    .mouse_listeners
                    .push((rendered_index, candidate_index));
            }
            self.next_frame
                .mouse_listeners
                .push(FrameOutput::new(listener, replayed_validity));
        }
        self.next_frame.pointer_cancel_listeners.extend(
            self.rendered_frame.pointer_cancel_listeners[range.start.pointer_cancel_listeners_index
                ..range.end.pointer_cancel_listeners_index]
                .iter()
                .map(|output| {
                    FrameOutput::new(
                        output.value.clone(),
                        SubtreeGeometryValidity::replayed_under(
                            output.validity.as_ref(),
                            validity.clone(),
                        ),
                    )
                }),
        );
        let accessed_element_states = self.rendered_frame.accessed_element_states
            [range.start.accessed_element_states_index..range.end.accessed_element_states_index]
            .to_vec();
        for key in accessed_element_states {
            self.next_frame.accessed_element_states.push(key.clone());
            let recorded_validity = self
                .rendered_frame
                .element_state_validities
                .get(&key)
                .and_then(Option::as_ref);
            self.next_frame.element_state_validities.insert(
                key,
                SubtreeGeometryValidity::replayed_under(recorded_validity, validity.clone()),
            );
        }
        self.next_frame.tab_stops.replay_scoped(
            &self.rendered_frame.tab_stops.insertion_history
                [range.start.tab_handle_index..range.end.tab_handle_index],
            validity.clone(),
        );
        self.next_frame.atlas_access_diagnostic_entries.extend(
            self.rendered_frame.atlas_access_diagnostic_entries[range
                .start
                .atlas_access_diagnostics_index
                ..range.end.atlas_access_diagnostics_index]
                .iter()
                .map(|entry| {
                    FrameOutput::new(
                        entry.value,
                        SubtreeGeometryValidity::replayed_under(
                            entry.validity.as_ref(),
                            validity.clone(),
                        ),
                    )
                }),
        );
        self.next_frame.image_paint_diagnostic_entries.extend(
            self.rendered_frame.image_paint_diagnostic_entries[range
                .start
                .image_paint_diagnostics_index
                ..range.end.image_paint_diagnostics_index]
                .iter()
                .map(|entry| {
                    let mut diagnostic = entry.value;
                    diagnostic.frame_generation = self.next_frame.generation;
                    FrameOutput::new(
                        diagnostic,
                        SubtreeGeometryValidity::replayed_under(
                            entry.validity.as_ref(),
                            validity.clone(),
                        ),
                    )
                }),
        );

        self.text_system
            .reuse_layouts(range.start.line_layout_index..range.end.line_layout_index);
        self.next_frame.subtree_transform_diagnostics.extend(
            self.rendered_frame.subtree_transform_diagnostics[range
                .start
                .subtree_transform_diagnostics_index
                ..range.end.subtree_transform_diagnostics_index]
                .iter()
                .copied()
                .map(|mut diagnostic| {
                    diagnostic.frame_generation = self.next_frame.generation;
                    diagnostic
                }),
        );
        let replay_result = self.next_frame.scene.replay(
            range.start.scene_index..range.end.scene_index,
            &self.rendered_frame.scene,
            validity,
        );
        if let Err(error) = replay_result {
            self.record_subtree_geometry_failure(error);
        } else {
            for identity in &retained_visual_replays {
                self.candidate_frame_transaction
                    .as_mut()
                    .expect("cached paint replay must retain its candidate frame transaction")
                    .record_retained_visual_replay(*identity);
            }
            self.next_frame
                .retained_visual_replays
                .extend(retained_visual_replays);
            for entry in atlas_texture_lease_entries {
                if let Ok(lease) = &entry {
                    for texture in lease.texture_instances() {
                        self.next_frame
                            .atlas_texture_leases_by_instance
                            .entry(*texture)
                            .or_insert_with(|| lease.clone());
                    }
                }
                self.record_atlas_texture_lease_entry(entry);
            }
            self.next_frame
                .retained_visual_publications
                .extend(retained_visual_publications);
        }
        true
    }

    /// Push a text style onto the stack, and call a function with that style active.
    /// Use [`Window::text_style`] to get the current, combined text style. This method
    /// should only be called as part of element drawing.
    pub fn with_text_style<F, R>(&mut self, style: Option<TextStyleRefinement>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.invalidator.debug_assert_paint_or_prepaint();
        if let Some(style) = style {
            self.text_style_stack.push(style);
            let result = f(self);
            self.text_style_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// Updates the cursor style at the platform level. This method should only be called
    /// during the paint phase of element drawing.
    pub fn set_cursor_style(&mut self, style: CursorStyle, hitbox: &Hitbox) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame.cursor_styles.push(CursorStyleRequest {
            hitbox_id: Some(hitbox.id),
            style,
            validity: self.subtree_geometry_validity(),
        });
    }

    /// Updates the cursor style for the entire window at the platform level. A cursor
    /// style using this method will have precedence over any cursor style set using
    /// `set_cursor_style`. This method should only be called during the paint
    /// phase of element drawing.
    pub fn set_window_cursor_style(&mut self, style: CursorStyle) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame.cursor_styles.push(CursorStyleRequest {
            hitbox_id: None,
            style,
            validity: self.subtree_geometry_validity(),
        })
    }

    /// Sets a tooltip to be rendered for the upcoming frame. This method should only be called
    /// during the paint phase of element drawing.
    pub fn set_tooltip(&mut self, tooltip: AnyTooltip) -> TooltipId {
        self.invalidator.debug_assert_prepaint();
        let id = TooltipId(post_inc(&mut self.next_tooltip_id.0));
        if !self.subtree_presentation().is_interactive() {
            return id;
        }
        self.next_frame.tooltip_requests.push(Some(TooltipRequest {
            id,
            tooltip,
            validity: self.subtree_geometry_validity(),
        }));
        id
    }

    /// Resolves a checked child-local clip into an opaque token for this candidate frame.
    ///
    /// Custom elements should create the token during prepaint, store it in their prepaint state,
    /// and pass the same token to [`Self::with_prepared_subtree_clip`] during paint.
    pub fn prepare_subtree_clip(
        &mut self,
        clip: &SubtreeClip,
        child_bounds: Bounds<Pixels>,
    ) -> PreparedSubtreeClip {
        self.prepare_subtree_clip_with_accessibility_axes(clip, child_bounds, point(true, true))
    }

    /// Resolves an internal scroll viewport clip.
    ///
    /// Every false accessibility axis must describe a visual scroll axis: descendants may remain
    /// semantically reachable through `ScrollIntoView` once this viewport is reachable.
    pub(crate) fn prepare_subtree_clip_with_accessibility_axes(
        &mut self,
        clip: &SubtreeClip,
        child_bounds: Bounds<Pixels>,
        accessibility_axes: Point<bool>,
    ) -> PreparedSubtreeClip {
        self.invalidator.debug_assert_paint_or_prepaint();
        let inherited = self.clip_stack();
        let resolved = clip.resolve_with_accessibility_axes(
            child_bounds,
            self.subtree_transform(),
            inherited.conservative_bounds(),
            accessibility_axes,
        );
        self.prepare_subtree_clip_resolution(inherited, resolved)
    }

    pub(crate) fn prepare_failed_subtree_clip(
        &mut self,
        error: SubtreeClipError,
    ) -> PreparedSubtreeClip {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.prepare_subtree_clip_resolution(self.clip_stack(), Err(error))
    }

    fn prepare_subtree_clip_resolution(
        &mut self,
        inherited: ClipStackSnapshot,
        resolved: Result<ResolvedClip, SubtreeClipError>,
    ) -> PreparedSubtreeClip {
        let validity = self.new_subtree_geometry_validity();
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                validity.invalidate(error);
                ResolvedClip::rectangle(Bounds::default())
            }
        };
        PreparedSubtreeClip {
            window_id: self.handle.window_id(),
            frame_generation: self.next_frame.generation,
            parent_transform: self.subtree_transform(),
            resolved: inherited.push(resolved),
            inherited,
            validity,
        }
    }

    /// Re-enters a subtree clip captured by [`Self::prepare_subtree_clip`].
    ///
    /// Returns `None` without executing `f` when the token belongs to another window, frame, or
    /// parent geometry scope. Geometry failures raised while `f` runs roll back every affected
    /// candidate-frame output.
    pub fn with_prepared_subtree_clip<R>(
        &mut self,
        prepared: &PreparedSubtreeClip,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        self.with_prepared_subtree_clip_owned_by_accessibility_node(prepared, None, f)
    }

    pub(crate) fn with_prepared_subtree_clip_owned_by_accessibility_node<R>(
        &mut self,
        prepared: &PreparedSubtreeClip,
        owner_id: Option<accesskit::NodeId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        self.invalidator.debug_assert_paint_or_prepaint();
        if prepared.window_id != self.handle.window_id()
            || prepared.frame_generation != self.next_frame.generation
            || prepared.parent_transform != self.subtree_transform()
            || prepared.inherited != self.clip_stack()
        {
            debug_assert!(
                false,
                "prepared subtree clip used outside its captured scope"
            );
            return None;
        }

        let validity = prepared.validity.clone();
        if !validity.is_valid() {
            self.record_subtree_geometry_scope_diagnostic(&validity);
            return None;
        }
        if self.invalidator.is_prepaint() {
            let accessibility_owner = self.a11y.is_active()
                && self.subtree_presentation().is_interactive()
                && owner_id.is_some_and(|owner_id| self.a11y.nodes.is_current_node(owner_id));
            let mut output = None;
            let transaction = self.transact_subtree_geometry(Some(validity.clone()), |window| {
                window.with_resolved_subtree_transform(
                    prepared.parent_transform,
                    Some(validity.clone()),
                    |window| {
                        window.with_resolved_clip_stack(prepared.resolved.clone(), |window| {
                            output = Some(
                                window.with_accessibility_clip_owner_scope(!accessibility_owner, f),
                            );
                        })
                    },
                )
            });
            if transaction.is_ok() && validity.is_valid() {
                if accessibility_owner {
                    let owner_marked = owner_id.is_some_and(|owner_id| {
                        self.a11y.nodes.mark_current_node_clips_children(owner_id)
                    });
                    debug_assert!(
                        owner_marked,
                        "prepared subtree clip owner must remain current after a successful prepaint"
                    );
                }
                output
            } else {
                self.record_subtree_geometry_scope_diagnostic(&validity);
                None
            }
        } else {
            let output = self.with_resolved_subtree_transform(
                prepared.parent_transform,
                Some(validity.clone()),
                |window| {
                    window.with_resolved_clip_stack(prepared.resolved.clone(), |window| f(window))
                },
            );
            if validity.is_valid() {
                Some(output)
            } else {
                self.record_subtree_geometry_scope_diagnostic(&validity);
                None
            }
        }
    }

    fn with_resolved_clip_stack<R>(
        &mut self,
        snapshot: ClipStackSnapshot,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        let stack = self.clip_stack.clone();
        let entered_depth = stack.borrow().len();
        stack.borrow_mut().push(snapshot);
        let _guard = ClipStackScopeGuard {
            stack,
            entered_depth,
        };
        f(self)
    }

    /// Updates the global element offset relative to the current offset. This is used to implement
    /// scrolling. This method should only be called during the prepaint phase of element drawing.
    pub fn with_element_offset<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();

        if offset.is_zero() {
            return f(self);
        };

        let abs_offset = self.element_offset() + offset;
        self.with_absolute_element_offset(abs_offset, f)
    }

    /// Updates the global element offset based on the given offset. This is used to implement
    /// drag handles and other manual painting of elements. This method should only be called during
    /// the prepaint phase of element drawing.
    pub fn with_absolute_element_offset<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        self.element_offset_stack.push(offset);
        let result = f(self);
        self.element_offset_stack.pop();
        result
    }

    pub(crate) fn with_element_opacity<R>(
        &mut self,
        opacity: Option<f32>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();

        let Some(opacity) = opacity else {
            return f(self);
        };

        let previous_opacity = self.element_opacity;
        self.element_opacity = previous_opacity * opacity;
        let result = f(self);
        self.element_opacity = previous_opacity;
        result
    }

    /// Project frame-local accessibility membership while prepainting a surface subtree.
    ///
    /// This does not choose which surface is authoritative. Callers derive the scope from their
    /// owning runtime, while GPUI remains responsible for final tree membership and repair.
    pub fn with_accessibility_tree_scope<R>(
        &mut self,
        scope: AccessibilityTreeScope,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        if !self.a11y.is_active() || !self.subtree_presentation().is_interactive() {
            return f(self);
        }

        let _scope = self.a11y.nodes.enter_scope(scope);
        f(self)
    }

    fn with_accessibility_clip_owner_scope<R>(
        &mut self,
        enabled: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        if !enabled || !self.a11y.is_active() || !self.subtree_presentation().is_interactive() {
            return f(self);
        }

        let _scope = self.a11y.nodes.enter_clip_owner_scope();
        f(self)
    }

    fn with_accessibility_deferred_parent_scope<R>(
        &mut self,
        parent: Option<a11y::AccessibilityDeferredParent>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let Some(parent) = parent else {
            return f(self);
        };
        if !self.a11y.is_active() || !self.subtree_presentation().is_interactive() {
            return f(self);
        }

        let _scope = self.a11y.nodes.enter_deferred_parent_scope(parent);
        f(self)
    }

    fn with_accessibility_window_portal_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_prepaint();
        if !self.a11y.is_active() || !self.subtree_presentation().is_interactive() {
            return f(self);
        }

        let Some(parent) = self.a11y.nodes.reserve_window_portal_parent() else {
            return f(self);
        };
        let _scope = self.a11y.nodes.enter_window_portal_scope(parent);
        f(self)
    }

    #[cfg(test)]
    pub(crate) fn set_accessibility_active_for_test(&mut self, active: bool) {
        self.a11y.set_requested_active_for_test(active);
    }

    #[cfg(test)]
    pub(crate) fn accessibility_activation_generation_for_test(&self) -> u64 {
        self.a11y.activation_generation()
    }

    #[cfg(test)]
    pub(crate) fn install_tooltip_bounds_with_validity_for_test(
        &mut self,
        bounds: Bounds<Pixels>,
    ) -> (TooltipId, SubtreeGeometryValidity) {
        let id = self.next_tooltip_id;
        let validity = self.new_subtree_geometry_validity();
        self.tooltip_bounds = Some(TooltipBounds {
            id,
            bounds,
            validity: Some(validity.clone()),
        });
        (id, validity)
    }

    /// Perform prepaint on child elements in a "retryable" manner, so that any side effects
    /// of prepaints can be discarded before prepainting again. This is used to support autoscroll
    /// where we need to prepaint children to detect the autoscroll bounds, then adjust the
    /// element offset and prepaint again. See [`crate::List`] for an example. This method should only be
    /// called during the prepaint phase of element drawing.
    pub fn transact<T, U>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, U>) -> Result<T, U> {
        self.invalidator.debug_assert_prepaint();
        let index = self.prepaint_index();
        let candidate_focus = self.next_frame.focus;
        let committed_focus = self.focus;
        let pending_focus_claim = self.pending_focus_claim;
        let pending_focus_reveal_fence = self.pending_focus_reveal_fence.clone();
        let pending_blur_claim_generation = self.pending_blur_claim_generation;
        let provisional_focus_claim = self.provisional_focus_claim;
        let pending_focus_completion = self.pending_focus_completion.clone();
        let focus_claim_resolutions_len = self.focus_claim_resolutions.len();
        let focus_claim_revision = self.focus_claim_revision;
        let requested_autoscroll = self.requested_autoscroll.clone();
        #[cfg(any(test, feature = "test-support"))]
        let debug_bounds = self.next_frame.debug_bounds.clone();
        #[cfg(any(test, feature = "test-support"))]
        let debug_bounds_entries_len = self.next_frame.debug_bounds_entries.len();
        #[cfg(any(test, feature = "test-support"))]
        let debug_focus_handles = self.next_frame.debug_focus_handles.clone();
        #[cfg(any(test, feature = "test-support"))]
        let debug_focus_entries_len = self.next_frame.debug_focus_entries.len();
        #[cfg(any(feature = "inspector", debug_assertions))]
        let inspector_hitboxes = self.next_frame.inspector_hitboxes.clone();
        let a11y_checkpoint = self
            .a11y
            .is_active()
            .then(|| self.a11y.prepaint_checkpoint());
        let result = f(self);
        if result.is_err() {
            self.next_frame.focus = candidate_focus;
            self.focus = committed_focus;
            self.pending_focus_claim = pending_focus_claim;
            self.pending_focus_reveal_fence = pending_focus_reveal_fence;
            self.pending_blur_claim_generation = pending_blur_claim_generation;
            self.provisional_focus_claim = provisional_focus_claim;
            self.pending_focus_completion = pending_focus_completion;
            self.focus_claim_resolutions
                .truncate(focus_claim_resolutions_len);
            self.focus_claim_revision = focus_claim_revision;
            self.requested_autoscroll = requested_autoscroll;
            self.next_frame.hitboxes.truncate(index.hitboxes_index);
            self.next_frame
                .pointer_capture_bindings
                .truncate(index.pointer_capture_bindings_index);
            self.next_frame
                .truncate_portal_anchor_bindings(index.portal_anchor_bindings_index);
            self.next_frame
                .truncate_reveal_target_bindings(index.reveal_target_bindings_index);
            self.next_frame
                .retained_resources
                .truncate(index.retained_resources_index);
            self.next_frame
                .prepaint_commits
                .truncate(index.prepaint_commits_index);
            self.next_frame
                .tooltip_requests
                .truncate(index.tooltips_index);
            self.next_frame
                .deferred_draws
                .truncate(index.deferred_draws_index);
            self.next_frame
                .dispatch_tree
                .truncate(index.dispatch_tree_index);
            self.next_frame
                .accessed_element_states
                .truncate(index.accessed_element_states_index);
            self.next_frame
                .subtree_transform_diagnostics
                .truncate(index.subtree_transform_diagnostics_index);
            self.text_system.truncate_layouts(index.line_layout_index);
            #[cfg(any(test, feature = "test-support"))]
            {
                self.next_frame.debug_bounds = debug_bounds;
                self.next_frame
                    .debug_bounds_entries
                    .truncate(debug_bounds_entries_len);
                self.next_frame.debug_focus_handles = debug_focus_handles;
                self.next_frame
                    .debug_focus_entries
                    .truncate(debug_focus_entries_len);
            }
            #[cfg(any(feature = "inspector", debug_assertions))]
            {
                self.next_frame.inspector_hitboxes = inspector_hitboxes;
            }
            if let Some(checkpoint) = a11y_checkpoint {
                self.a11y.rollback_prepaint(checkpoint);
            }
        }
        result
    }

    pub(crate) fn transact_subtree_geometry<T>(
        &mut self,
        validity: Option<SubtreeGeometryValidity>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Result<T, SubtreeGeometryError> {
        let accessed_element_states_index = self.next_frame.accessed_element_states.len();
        let mut invalid_element_states = Vec::new();
        let result = self.transact(|window| {
            let result = f(window);
            if let Some(error) = validity.as_ref().and_then(SubtreeGeometryValidity::failure) {
                invalid_element_states.extend_from_slice(
                    &window.next_frame.accessed_element_states[accessed_element_states_index..],
                );
                Err(error)
            } else {
                Ok(result)
            }
        });

        if result.is_err() {
            // `transact` truncates the journal suffix. Keep invalid element-state accesses visible
            // to `Frame::finish`, which owns disposal of state bound to a failed geometry scope.
            self.next_frame
                .accessed_element_states
                .extend(invalid_element_states);
        }
        result
    }

    /// When you call this method during [`Element::prepaint`], containing elements will attempt to
    /// scroll to cause the specified bounds to become visible. When they decide to autoscroll, they will call
    /// [`Element::prepaint`] again with a new set of bounds. See [`crate::List`] for an example of an element
    /// that supports this method being called on the elements it contains. This is a local direct-scroll
    /// request, not a nested bring-into-view request; an accepted request supersedes older reveal work on
    /// its container. This method should only be called during the prepaint phase of element drawing.
    pub fn request_autoscroll(&mut self, bounds: Bounds<Pixels>) {
        self.invalidator.debug_assert_prepaint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        if let Ok(bounds) = self.try_project_subtree_bounds(bounds) {
            self.requested_autoscroll = Some(AutoscrollIntent {
                bounds,
                validity: self.subtree_geometry_validity(),
            });
        }
    }

    /// This method can be called from a containing element such as [`crate::List`] to support the autoscroll behavior
    /// described in [`Self::request_autoscroll`].
    pub(crate) fn take_autoscroll(&mut self) -> Option<AutoscrollIntent> {
        self.invalidator.debug_assert_prepaint();
        let intent = self.requested_autoscroll.take()?;
        if intent
            .validity
            .as_ref()
            .is_some_and(|validity| !validity.is_valid())
        {
            return None;
        }
        match self
            .subtree_transform()
            .try_inverse_project_bounds(intent.bounds)
        {
            Ok(bounds) => Some(AutoscrollIntent {
                bounds,
                validity: intent.validity,
            }),
            Err(error) => {
                self.record_subtree_transform_failure(error);
                None
            }
        }
    }

    /// Asynchronously load an asset, if the asset hasn't finished loading this will return None.
    /// Your view will be re-drawn once the asset has finished loading.
    ///
    /// Note that the multiple calls to this method will only result in one `Asset::load` call at a
    /// time.
    pub fn use_asset<A: Asset>(&mut self, source: &A::Source, cx: &mut App) -> Option<A::Output> {
        let (task, is_first) = cx.fetch_asset::<A>(source);
        task.clone().now_or_never().or_else(|| {
            if is_first {
                let entity_id = self.current_view();
                self.spawn(cx, {
                    let task = task.clone();
                    async move |cx| {
                        task.await;

                        cx.on_next_frame(move |_, cx| {
                            cx.notify(entity_id);
                        });
                    }
                })
                .detach();
            }

            None
        })
    }

    /// Asynchronously load an asset, if the asset hasn't finished loading or doesn't exist this will return None.
    /// Your view will not be re-drawn once the asset has finished loading.
    ///
    /// Note that the multiple calls to this method will only result in one `Asset::load` call at a
    /// time.
    pub fn get_asset<A: Asset>(&mut self, source: &A::Source, cx: &mut App) -> Option<A::Output> {
        let (task, _) = cx.fetch_asset::<A>(source);
        task.now_or_never()
    }
    /// Obtain the current element offset. This method should only be called during the
    /// prepaint phase of element drawing.
    pub fn element_offset(&self) -> Point<Pixels> {
        self.invalidator.debug_assert_prepaint();
        self.element_offset_stack
            .last()
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn with_prepaint_layout_id<R>(
        &mut self,
        layout_id: LayoutId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let current = self.current_prepaint_layout_id.clone();
        let previous = current.replace(Some(layout_id));
        let _guard = PrepaintLayoutScopeGuard {
            current,
            entered: layout_id,
            previous,
        };
        f(self)
    }

    pub(crate) fn current_prepaint_layout_id(&self) -> Option<LayoutId> {
        self.current_prepaint_layout_id.get()
    }

    pub(crate) fn subtree_transform(&self) -> ResolvedSubtreeTransform {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.subtree_transform_stack
            .borrow()
            .last()
            .map(|scope| scope.transform)
            .unwrap_or(ResolvedSubtreeTransform::IDENTITY)
    }

    /// Returns the effective layout-preserving presentation state for the current element subtree.
    pub fn subtree_presentation(&self) -> SubtreePresentation {
        self.subtree_presentation_stack
            .borrow()
            .last()
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn with_subtree_presentation<R>(
        &mut self,
        requested: SubtreePresentation,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let stack = self.subtree_presentation_stack.clone();
        let entered_depth = stack.borrow().len();
        let effective = requested.resolve_under(self.subtree_presentation());
        stack.borrow_mut().push(effective);
        let _guard = SubtreePresentationScopeGuard {
            stack,
            entered_depth,
        };
        f(self)
    }

    pub(crate) fn resolve_subtree_transform(
        &self,
        local: SubtreeTransform,
        child_bounds: Bounds<Pixels>,
    ) -> Result<ResolvedSubtreeTransform, SubtreeTransformError> {
        let resolved = ResolvedSubtreeTransform::try_from_local(
            self.subtree_transform(),
            local,
            child_bounds,
        )?;
        PrimitiveTransform::try_from_resolved(resolved, self.scale_factor())?;
        Ok(resolved)
    }

    pub(crate) fn new_subtree_geometry_validity(&self) -> SubtreeGeometryValidity {
        SubtreeGeometryValidity::new(self.subtree_geometry_validity())
    }

    pub(crate) fn subtree_geometry_validity(&self) -> Option<SubtreeGeometryValidity> {
        self.subtree_transform_stack
            .borrow()
            .last()
            .and_then(|scope| scope.validity.clone())
    }

    pub(crate) fn with_resolved_subtree_transform<R>(
        &mut self,
        transform: ResolvedSubtreeTransform,
        validity: Option<SubtreeGeometryValidity>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        if self.current_prepaint_layout_id().is_some() {
            self.update_portal_anchor_transform(transform, validity.clone());
            self.update_reveal_target_transform(transform, validity.clone());
        }
        let stack = self.subtree_transform_stack.clone();
        let entered_depth = stack.borrow().len();
        let _a11y_validity = self.a11y.nodes.enter_geometry_validity(validity.clone());
        stack.borrow_mut().push(SubtreeTransformScope {
            transform,
            validity,
        });
        let _guard = SubtreeTransformScopeGuard {
            stack,
            entered_depth,
        };
        f(self)
    }

    fn record_subtree_transform_failure(&self, error: SubtreeTransformError) {
        self.record_subtree_geometry_failure(SubtreeGeometryError::Transform(error));
    }

    fn record_subtree_geometry_failure(&self, error: SubtreeGeometryError) {
        self.invalidate_portal_anchor_capture();
        self.invalidate_reveal_target_capture();
        if let Some(scope) = self.subtree_transform_stack.borrow().last()
            && let Some(validity) = scope.validity.as_ref()
        {
            validity.invalidate(error);
        }
    }

    pub(crate) fn record_subtree_geometry_scope_diagnostic(
        &mut self,
        validity: &SubtreeGeometryValidity,
    ) {
        if let Some(error) = validity.take_unreported_failure() {
            match error {
                SubtreeGeometryError::Transform(error) => {
                    self.record_subtree_transform_diagnostic(error)
                }
                SubtreeGeometryError::Clip(error) => {
                    log::error!(
                        target: "open_gpui::subtree_clip",
                        "subtree clip suppressed a layout-only subtree: {error}"
                    );
                }
                SubtreeGeometryError::DeviceConversion => {
                    log::error!(
                        target: "open_gpui::subtree_geometry",
                        "device conversion suppressed a layout-only subtree"
                    );
                }
            }
        }
    }

    pub(crate) fn record_subtree_transform_diagnostic(&mut self, error: SubtreeTransformError) {
        self.next_frame
            .subtree_transform_diagnostics
            .push(SubtreeTransformDiagnostic {
                frame_generation: self.next_frame.generation,
                error,
            });
        log::error!(
            target: "open_gpui::subtree_transform",
            "subtree transform suppressed a layout-only subtree: {error}"
        );
    }

    pub(crate) fn try_project_subtree_bounds(
        &self,
        bounds: Bounds<Pixels>,
    ) -> Result<Bounds<Pixels>, SubtreeTransformError> {
        self.subtree_transform()
            .try_project_bounds(bounds)
            .inspect_err(|error| self.record_subtree_transform_failure(*error))
    }

    /// Resolves layout bounds through the active checked subtree transform.
    ///
    /// Custom elements may call this during prepaint when they need the same immutable geometry
    /// projection used by hitboxes, measurements, accessibility, and portal anchors. A failed
    /// projection invalidates the active transform scope; callers must omit the dependent channel.
    pub fn try_element_geometry(
        &self,
        layout_bounds: Bounds<Pixels>,
    ) -> Result<ElementGeometry, SubtreeTransformError> {
        let transform = self.subtree_transform();
        let displayed_bounds = transform
            .try_project_bounds(layout_bounds)
            .inspect_err(|error| self.record_subtree_transform_failure(*error))?;
        Ok(ElementGeometry::from_resolved(
            layout_bounds,
            displayed_bounds,
            transform,
        ))
    }

    pub(crate) fn try_project_subtree_point(
        &self,
        point: Point<Pixels>,
    ) -> Result<Point<Pixels>, SubtreeTransformError> {
        self.subtree_transform()
            .try_project_point(point)
            .inspect_err(|error| self.record_subtree_transform_failure(*error))
    }

    pub(crate) fn try_project_subtree_accessibility_bounds(
        &self,
        bounds: Bounds<Pixels>,
    ) -> Result<
        Option<(Bounds<Pixels>, accesskit::Rect, Option<Point<Pixels>>)>,
        SubtreeTransformError,
    > {
        let displayed = self.try_project_subtree_bounds(bounds)?;
        let Some((displayed, witness)) = self.clip_stack().accessibility_region(displayed) else {
            return Ok(None);
        };
        let scale = f64::from(self.scale_factor());
        let x0 = f64::from(displayed.origin.x.0) * scale;
        let y0 = f64::from(displayed.origin.y.0) * scale;
        let x1 = (f64::from(displayed.origin.x.0) + f64::from(displayed.size.width.0)) * scale;
        let y1 = (f64::from(displayed.origin.y.0) + f64::from(displayed.size.height.0)) * scale;
        if !scale.is_finite()
            || scale <= 0.0
            || !x0.is_finite()
            || !y0.is_finite()
            || !x1.is_finite()
            || !y1.is_finite()
        {
            let error = SubtreeTransformError::UnrepresentableResult;
            self.record_subtree_transform_failure(error);
            return Err(error);
        }
        Ok(Some((
            displayed,
            accesskit::Rect { x0, y0, x1, y1 },
            witness,
        )))
    }

    fn base_primitive_transform(&self) -> Option<PrimitiveTransform> {
        match PrimitiveTransform::try_from_resolved(self.subtree_transform(), self.scale_factor()) {
            Ok(transform) => Some(transform),
            Err(error) => {
                self.record_subtree_transform_failure(error);
                None
            }
        }
    }

    fn primitive_raster_transform(
        &self,
        base: PrimitiveTransform,
        local_raster_bounds: Bounds<ScaledPixels>,
        snap: PrimitiveRasterSnap,
    ) -> Option<PrimitiveTransform> {
        let projected_bounds = match base.try_project_bounds(local_raster_bounds) {
            Ok(bounds) => bounds,
            Err(error) => {
                self.record_subtree_transform_failure(error);
                return None;
            }
        };
        let snapped_bounds = match snap {
            PrimitiveRasterSnap::NearestEdges => Self::snap_device_bounds(projected_bounds),
            PrimitiveRasterSnap::CoverEdges => Self::cover_device_bounds(projected_bounds),
        };
        if local_raster_bounds.is_empty() || snapped_bounds.is_empty() {
            return None;
        }

        match base.try_retarget_bounds(local_raster_bounds, snapped_bounds) {
            Ok(transform) => Some(transform),
            Err(error) => {
                self.record_subtree_transform_failure(error);
                None
            }
        }
    }

    fn try_raster_local_stroke(
        local: ScaledPixels,
        projected_axis_scale: f32,
        raster_axis_scale: f32,
    ) -> Result<ScaledPixels, SubtreeTransformError> {
        if !local.0.is_finite()
            || !projected_axis_scale.is_normal()
            || projected_axis_scale <= 0.0
            || !raster_axis_scale.is_normal()
            || raster_axis_scale <= 0.0
        {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        let projected = local.0 * projected_axis_scale;
        let snapped = round_stroke_to_device_pixel(projected, 1.0);
        let raster_local = snapped / raster_axis_scale;
        let round_trip = raster_local * raster_axis_scale;
        if !projected.is_finite()
            || !snapped.is_finite()
            || !raster_local.is_finite()
            || !round_trip.is_finite()
            || (local.0 != 0.0
                && (projected == 0.0
                    || snapped == 0.0
                    || !raster_local.is_normal()
                    || !raster_local.recip().is_finite()
                    || round_trip == 0.0))
        {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        Ok(ScaledPixels(raster_local))
    }

    fn raster_border_widths(
        &self,
        edges: Edges<Pixels>,
        base: PrimitiveTransform,
        raster: PrimitiveTransform,
    ) -> Option<Edges<ScaledPixels>> {
        let edges = edges.scale(self.scale_factor());
        let base_scale = base.scale();
        let raster_scale = raster.scale();
        let result = (|| {
            Ok(Edges {
                top: Self::try_raster_local_stroke(
                    edges.top,
                    base_scale.height,
                    raster_scale.height,
                )?,
                right: Self::try_raster_local_stroke(
                    edges.right,
                    base_scale.width,
                    raster_scale.width,
                )?,
                bottom: Self::try_raster_local_stroke(
                    edges.bottom,
                    base_scale.height,
                    raster_scale.height,
                )?,
                left: Self::try_raster_local_stroke(
                    edges.left,
                    base_scale.width,
                    raster_scale.width,
                )?,
            })
        })();
        match result {
            Ok(edges) => Some(edges),
            Err(error) => {
                self.record_subtree_transform_failure(error);
                None
            }
        }
    }

    fn underline_raster_projection(
        &self,
        base: PrimitiveTransform,
        local_bounds: Bounds<ScaledPixels>,
        local_thickness: ScaledPixels,
        height_multiplier: f32,
    ) -> Option<(PrimitiveTransform, ScaledPixels)> {
        let projected_bounds = match base.try_project_bounds(local_bounds) {
            Ok(bounds) => bounds,
            Err(error) => {
                self.record_subtree_transform_failure(error);
                return None;
            }
        };
        let projected_thickness = local_thickness.0 * base.scale().height;
        let snapped_thickness = round_stroke_to_device_pixel(projected_thickness, 1.0);
        let snapped_width = round_stroke_to_device_pixel(projected_bounds.size.width.0, 1.0);
        if !projected_thickness.is_finite()
            || !snapped_thickness.is_finite()
            || !snapped_width.is_finite()
        {
            self.record_subtree_transform_failure(SubtreeTransformError::UnrepresentableResult);
            return None;
        }
        let target_bounds = Bounds::new(
            projected_bounds
                .origin
                .map(|value| ScaledPixels(round_half_toward_zero(value.0))),
            size(
                ScaledPixels(snapped_width),
                ScaledPixels(snapped_thickness * height_multiplier),
            ),
        );
        if local_bounds.is_empty() || target_bounds.is_empty() {
            return None;
        }
        let raster = match base.try_retarget_bounds(local_bounds, target_bounds) {
            Ok(transform) => transform,
            Err(error) => {
                self.record_subtree_transform_failure(error);
                return None;
            }
        };
        match Self::try_raster_local_stroke(
            local_thickness,
            base.scale().height,
            raster.scale().height,
        ) {
            Ok(thickness) => Some((raster, thickness)),
            Err(error) => {
                self.record_subtree_transform_failure(error);
                None
            }
        }
    }

    fn retain_frame_atlas_texture_instance(
        &mut self,
        texture: AtlasTextureInstanceId,
    ) -> Result<Rc<AtlasTextureLease>, AtlasTextureLeaseError> {
        if let Some(lease) = self
            .next_frame
            .atlas_texture_leases_by_instance
            .get(&texture)
        {
            return Ok(lease.clone());
        }

        let lease = Rc::new(
            self.sprite_atlas
                .clone()
                .retain_texture_instances(&[texture])?,
        );
        self.next_frame
            .atlas_texture_leases_by_instance
            .insert(texture, lease.clone());
        Ok(lease)
    }

    fn insert_scene_primitive(&mut self, primitive: impl Into<Primitive>) -> bool {
        let primitive = primitive.into();
        let atlas_texture_lease = match primitive.atlas_texture_instance() {
            Some(texture) => match self.retain_frame_atlas_texture_instance(texture) {
                Ok(lease) => Some(Ok(lease)),
                Err(error) => {
                    if self.candidate_atlas_lease_failure.is_none() {
                        self.candidate_atlas_lease_failure = Some(error);
                    }
                    self.record_atlas_texture_lease_entry(Err(error));
                    return false;
                }
            },
            None => None,
        };
        let clip_stack = self.clip_stack();
        let scale_factor = self.scale_factor();
        if let Err(error) = self.next_frame.scene.insert_primitive_scoped(
            primitive,
            &clip_stack,
            scale_factor,
            self.subtree_geometry_validity(),
        ) {
            self.record_subtree_geometry_failure(error);
            false
        } else if let Some(lease) = atlas_texture_lease {
            self.record_atlas_texture_lease_entry(lease);
            true
        } else {
            true
        }
    }

    /// Obtain the current element opacity. This method should only be called during the
    /// prepaint phase of element drawing.
    #[inline]
    pub(crate) fn element_opacity(&self) -> f32 {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.element_opacity
    }

    pub(crate) fn clip_stack(&self) -> ClipStackSnapshot {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.clip_stack.borrow().last().cloned().unwrap_or_else(|| {
            ClipStackSnapshot::root(Bounds::new(Point::default(), self.viewport_size))
        })
    }

    /// Returns the conservative window-space AABB of the current exact subtree clip stack.
    pub fn clip_bounds(&self) -> Bounds<Pixels> {
        self.clip_stack().conservative_bounds()
    }

    /// Provide elements in the called function with a new namespace in which their identifiers must be unique.
    /// This can be used within a custom element to distinguish multiple sets of child elements.
    pub fn with_element_namespace<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.element_id_stack.push(element_id.into());
        let result = f(self);
        self.element_id_stack.pop();
        result
    }

    /// Use a piece of state that exists as long this element is being rendered in consecutive frames.
    pub fn use_keyed_state<S: 'static>(
        &mut self,
        key: impl Into<ElementId>,
        cx: &mut App,
        init: impl FnOnce(&mut Self, &mut Context<S>) -> S,
    ) -> Entity<S> {
        let current_view = self.current_view();
        self.with_global_id(key.into(), |global_id, window| {
            window.with_element_state(global_id, |state: Option<Entity<S>>, window| {
                if let Some(state) = state {
                    (state.clone(), state)
                } else {
                    let new_state = cx.new(|cx| init(window, cx));
                    cx.observe(&new_state, move |_, cx| {
                        cx.notify(current_view);
                    })
                    .detach();
                    (new_state.clone(), new_state)
                }
            })
        })
    }

    /// Use a piece of state that exists as long this element is being rendered in consecutive frames, without needing to specify a key
    ///
    /// NOTE: This method uses the location of the caller to generate an ID for this state.
    ///       If this is not sufficient to identify your state (e.g. you're rendering a list item),
    ///       you can provide a custom ElementID using the `use_keyed_state` method.
    #[track_caller]
    pub fn use_state<S: 'static>(
        &mut self,
        cx: &mut App,
        init: impl FnOnce(&mut Self, &mut Context<S>) -> S,
    ) -> Entity<S> {
        self.use_keyed_state(
            ElementId::CodeLocation(*core::panic::Location::caller()),
            cx,
            init,
        )
    }

    /// Updates or initializes state for an element with the given id that lives across multiple
    /// frames. If an element with this ID existed in the rendered frame, its state will be passed
    /// to the given closure. The state returned by the closure will be stored so it can be referenced
    /// when drawing the next frame. This method should only be called as part of element drawing.
    pub fn with_element_state<S, R>(
        &mut self,
        global_id: &GlobalElementId,
        f: impl FnOnce(Option<S>, &mut Self) -> (R, S),
    ) -> R
    where
        S: 'static,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        let key = (global_id.clone(), TypeId::of::<S>());
        self.next_frame.accessed_element_states.push(key.clone());
        self.next_frame
            .element_state_validities
            .insert(key.clone(), self.subtree_geometry_validity());

        let candidate_state = self.next_frame.element_states.remove(&key);
        let state = candidate_state.or_else(|| {
            let state = self.rendered_frame.element_states.remove(&key);
            if state.is_some() {
                self.candidate_frame_transfers
                    .element_states
                    .push(key.clone());
            }
            state
        });
        if let Some(any) = state {
            let ElementStateBox {
                inner,
                #[cfg(debug_assertions)]
                type_name,
            } = any;
            // Using the extra inner option to avoid needing to reallocate a new box.
            let mut state_box = inner
                .downcast::<Option<S>>()
                .map_err(|_| {
                    #[cfg(debug_assertions)]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}, actual: {:?}",
                            std::any::type_name::<S>(),
                            type_name
                        )
                    }

                    #[cfg(not(debug_assertions))]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}",
                            std::any::type_name::<S>(),
                        )
                    }
                })
                .unwrap();

            let state = state_box.take().expect(
                "reentrant call to with_element_state for the same state type and element id",
            );
            let (result, state) = f(Some(state), self);
            state_box.replace(state);
            self.next_frame.element_states.insert(
                key,
                ElementStateBox {
                    inner: state_box,
                    #[cfg(debug_assertions)]
                    type_name,
                },
            );
            result
        } else {
            let (result, state) = f(None, self);
            self.next_frame.element_states.insert(
                key,
                ElementStateBox {
                    inner: Box::new(Some(state)),
                    #[cfg(debug_assertions)]
                    type_name: std::any::type_name::<S>(),
                },
            );
            result
        }
    }

    /// A variant of `with_element_state` that allows the element's id to be optional. This is a convenience
    /// method for elements where the element id may or may not be assigned. Prefer using `with_element_state`
    /// when the element is guaranteed to have an id.
    ///
    /// The first option means 'no ID provided'
    /// The second option means 'not yet initialized'
    pub fn with_optional_element_state<S, R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(Option<Option<S>>, &mut Self) -> (R, Option<S>),
    ) -> R
    where
        S: 'static,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        if let Some(global_id) = global_id {
            self.with_element_state(global_id, |state, cx| {
                let (result, state) = f(Some(state), cx);
                let state =
                    state.expect("you must return some state when you pass some element id");
                (result, state)
            })
        } else {
            let (result, state) = f(None, self);
            debug_assert!(
                state.is_none(),
                "you must not return an element state when passing None for the global id"
            );
            result
        }
    }

    /// Executes the given closure within the context of a tab group.
    #[inline]
    pub fn with_tab_group<R>(&mut self, index: Option<isize>, f: impl FnOnce(&mut Self) -> R) -> R {
        if !self.subtree_presentation().is_interactive() {
            return f(self);
        }
        if let Some(index) = index {
            self.next_frame.tab_stops.begin_group(index);
            let result = f(self);
            self.next_frame.tab_stops.end_group();
            result
        } else {
            f(self)
        }
    }

    /// Defers the drawing of the given element, scheduling it to be painted on top of the currently-drawn tree
    /// at a later time. The `priority` parameter determines the drawing order relative to other deferred elements,
    /// with higher values being drawn on top.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn defer_draw(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
    ) {
        self.invalidator.debug_assert_prepaint();
        let transform = self.subtree_transform();
        let validity = self.subtree_geometry_validity();
        self.defer_draw_with_transform(
            element,
            absolute_offset,
            priority,
            self.clip_stack(),
            transform,
            validity,
            self.current_scroll_ancestry_for_deferred(),
            true,
        );
    }

    /// Defers an element at a deliberate window-space portal boundary.
    ///
    /// Unlike [`Self::defer_draw`], this resets inherited subtree geometry, clipping, and
    /// accessibility parentage. Theme and presentation inheritance are unaffected. The portal
    /// starts with the full viewport clip.
    pub fn defer_draw_in_window_space(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
    ) {
        self.invalidator.debug_assert_prepaint();
        self.defer_draw_with_transform(
            element,
            absolute_offset,
            priority,
            self.window_portal_clip_stack(),
            ResolvedSubtreeTransform::IDENTITY,
            self.subtree_geometry_validity(),
            SmallVec::new(),
            false,
        );
    }

    pub(crate) fn with_window_space_portal_prepaint<R>(
        &mut self,
        absolute_offset: Point<Pixels>,
        validity: Option<SubtreeGeometryValidity>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let clip_stack = self.window_portal_clip_stack();
        self.with_accessibility_window_portal_scope(|window| {
            window.with_scroll_ancestry(SmallVec::new(), |window| {
                window.with_resolved_subtree_transform(
                    ResolvedSubtreeTransform::IDENTITY,
                    validity,
                    |window| {
                        window.with_absolute_element_offset(absolute_offset, |window| {
                            window.with_resolved_clip_stack(clip_stack, f)
                        })
                    },
                )
            })
        })
    }

    pub(crate) fn with_window_space_portal_paint<R>(
        &mut self,
        validity: Option<SubtreeGeometryValidity>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint();
        let clip_stack = self.window_portal_clip_stack();
        self.with_resolved_subtree_transform(
            ResolvedSubtreeTransform::IDENTITY,
            validity,
            |window| window.with_resolved_clip_stack(clip_stack, f),
        )
    }

    fn window_portal_clip_stack(&self) -> ClipStackSnapshot {
        ClipStackSnapshot::root(Bounds::new(Point::default(), self.viewport_size))
    }

    fn defer_draw_with_transform(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
        clip_stack: ClipStackSnapshot,
        subtree_transform: ResolvedSubtreeTransform,
        subtree_geometry_validity: Option<SubtreeGeometryValidity>,
        scroll_ancestry: SmallVec<[ScrollContainerBinding; 8]>,
        preserve_accessibility_parent: bool,
    ) {
        let parent_node = self.next_frame.dispatch_tree.active_node_id().unwrap();
        let (accessibility_parent, accessibility_proxy_clip_owner) =
            if self.a11y.is_active() && self.subtree_presentation().is_interactive() {
                let parent = if preserve_accessibility_parent {
                    self.a11y.nodes.reserve_deferred_parent()
                } else {
                    self.a11y.nodes.reserve_window_portal_parent()
                };
                (
                    parent,
                    preserve_accessibility_parent
                        && self.a11y.nodes.current_depth_has_clip_owner_scope(),
                )
            } else {
                (None, false)
            };
        self.next_frame.deferred_draws.push(DeferredDraw {
            current_view: self.current_view(),
            parent_node,
            element_id_stack: self.element_id_stack.clone(),
            text_style_stack: self.text_style_stack.clone(),
            accessibility_tree_scope: self.a11y.current_tree_scope(),
            accessibility_parent,
            accessibility_proxy_clip_owner,
            clip_stack,
            rem_size: self.rem_size(),
            priority,
            element: Some(element),
            absolute_offset,
            subtree_presentation: self.subtree_presentation(),
            subtree_transform,
            subtree_geometry_validity,
            scroll_ancestry,
            prepaint_range: PrepaintStateIndex::default()..PrepaintStateIndex::default(),
            paint_range: PaintIndex::default()..PaintIndex::default(),
        });
    }

    /// Creates a new painting layer for the specified bounds. A "layer" is a batch
    /// of geometry that are non-overlapping and have the same draw order. This is typically used
    /// for performance reasons.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_layer<R>(&mut self, bounds: Bounds<Pixels>, f: impl FnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_paint();

        let clip_stack = self.clip_stack();
        let clipped_bounds = self
            .try_project_subtree_bounds(bounds)
            .ok()
            .map(|bounds| bounds.intersect(&clip_stack.conservative_bounds()));
        if let Some(clipped_bounds) = clipped_bounds.filter(|bounds| !bounds.is_empty()) {
            self.next_frame.scene.push_layer_scoped(
                self.cover_bounds(clipped_bounds),
                self.subtree_geometry_validity(),
            );
        }

        let result = f(self);

        if clipped_bounds.is_some_and(|bounds| !bounds.is_empty()) {
            self.next_frame
                .scene
                .pop_layer_scoped(self.subtree_geometry_validity());
        }

        result
    }

    /// Paint the drop (non-inset) shadows from `shadows` into the scene at the current
    /// z-index. Inset shadows are skipped; paint those with [`Self::paint_inset_shadows`]
    /// after the element's background so they layer on top of the fill.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_drop_shadows(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        shadows: &[BoxShadow],
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let opacity = self.element_opacity();
        let Some(base_transform) = self.base_primitive_transform() else {
            return;
        };
        let element_bounds = self.device_local_bounds(bounds);
        let element_corner_radii = corner_radii.scale(scale_factor);
        for shadow in shadows {
            if shadow.inset {
                continue;
            }
            let shadow_bounds = (bounds + shadow.offset).dilate(shadow.spread_radius);
            let mut primitive = Shadow {
                order: 0,
                blur_radius: shadow.blur_radius.scale(scale_factor),
                bounds: self.device_local_bounds(shadow_bounds),
                clip: Default::default(),
                corner_radii: corner_radii.scale(scale_factor),
                color: shadow.color.opacity(opacity),
                element_bounds,
                element_corner_radii,
                inset: 0,
                pad: 0,
                transform: base_transform,
            };
            let Some(transform) = self.primitive_raster_transform(
                base_transform,
                primitive.local_raster_bounds(),
                PrimitiveRasterSnap::CoverEdges,
            ) else {
                continue;
            };
            primitive.transform = transform;
            self.insert_scene_primitive(primitive);
        }
    }

    /// Paint the inset shadows from `shadows` into the scene at the current z-index. Should
    /// be called after the element's background so the shadow layers on top of the fill.
    /// Drop shadows are skipped; paint those with [`Self::paint_drop_shadows`] before the background.
    pub fn paint_inset_shadows(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        shadows: &[BoxShadow],
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let opacity = self.element_opacity();
        let Some(base_transform) = self.base_primitive_transform() else {
            return;
        };
        let element_bounds = self.device_local_bounds(bounds);
        let element_corner_radii = corner_radii.scale(scale_factor);
        for shadow in shadows {
            if !shadow.inset {
                continue;
            }
            let hole = (bounds + shadow.offset).dilate(-shadow.spread_radius);
            // Clamp at zero so a large spread can't produce negative radii, which would
            // break the SDF in the shader.
            let zero = Pixels::ZERO;
            let hole_corner_radii = Corners {
                top_left: (corner_radii.top_left - shadow.spread_radius).max(zero),
                top_right: (corner_radii.top_right - shadow.spread_radius).max(zero),
                bottom_right: (corner_radii.bottom_right - shadow.spread_radius).max(zero),
                bottom_left: (corner_radii.bottom_left - shadow.spread_radius).max(zero),
            };
            let mut primitive = Shadow {
                order: 0,
                blur_radius: shadow.blur_radius.scale(scale_factor),
                bounds: self.device_local_bounds(hole),
                clip: Default::default(),
                corner_radii: hole_corner_radii.scale(scale_factor),
                color: shadow.color.opacity(opacity),
                element_bounds,
                element_corner_radii,
                inset: 1,
                pad: 0,
                transform: base_transform,
            };
            let Some(transform) = self.primitive_raster_transform(
                base_transform,
                primitive.local_raster_bounds(),
                PrimitiveRasterSnap::CoverEdges,
            ) else {
                continue;
            };
            primitive.transform = transform;
            self.insert_scene_primitive(primitive);
        }
    }

    /// Paint one or more quads into the scene for the next frame at the current stacking context.
    /// Quads are colored rectangular regions with an optional background, border, and corner radius.
    /// see [`fill`], [`outline`], and [`quad`] to construct this type.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    ///
    /// Note that the `quad.corner_radii` are allowed to exceed the bounds, creating sharp corners
    /// where the circular arcs meet. This will not display well when combined with dashed borders.
    /// Use `Corners::clamp_radii_for_quad_size` if the radii should fit within the bounds.
    pub fn paint_quad(&mut self, quad: PaintQuad) {
        self.invalidator.debug_assert_paint();

        let opacity = self.element_opacity();
        let Some(base_transform) = self.base_primitive_transform() else {
            return;
        };
        let bounds = self.device_local_bounds(quad.bounds);
        let Some(transform) = self.primitive_raster_transform(
            base_transform,
            bounds,
            PrimitiveRasterSnap::NearestEdges,
        ) else {
            return;
        };
        let Some(border_widths) =
            self.raster_border_widths(quad.border_widths, base_transform, transform)
        else {
            return;
        };
        self.insert_scene_primitive(Quad {
            order: 0,
            bounds,
            clip: Default::default(),
            background: quad.background.opacity(opacity),
            border_color: quad.border_color.opacity(opacity),
            corner_radii: quad.corner_radii.scale(self.scale_factor()),
            border_widths,
            border_style: quad.border_style,
            transform,
        });
    }

    /// Paint the given `Path` into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_path(&mut self, mut path: Path<Pixels>, color: impl Into<Background>) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let opacity = self.element_opacity();
        let Some(transform) = self.base_primitive_transform() else {
            return;
        };
        let color: Background = color.into();
        path.color = color.opacity(opacity);
        let mut path = path.scale(scale_factor);
        path.transform = transform;
        self.insert_scene_primitive(path);
    }

    /// Paint an underline into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_underline(
        &mut self,
        origin: Point<Pixels>,
        width: Pixels,
        style: &UnderlineStyle,
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let local_thickness = style.thickness.scale(scale_factor);
        let height_multiplier = if style.wavy { 3.0 } else { 1.0 };
        let bounds = Bounds {
            origin: origin.scale(scale_factor),
            size: size(
                width.scale(scale_factor),
                ScaledPixels(local_thickness.0 * height_multiplier),
            ),
        };
        let element_opacity = self.element_opacity();
        let Some(base_transform) = self.base_primitive_transform() else {
            return;
        };
        let Some((transform, thickness)) = self.underline_raster_projection(
            base_transform,
            bounds,
            local_thickness,
            height_multiplier,
        ) else {
            return;
        };

        self.insert_scene_primitive(Underline {
            order: 0,
            pad: 0,
            bounds,
            clip: Default::default(),
            color: style.color.unwrap_or_default().opacity(element_opacity),
            thickness,
            wavy: if style.wavy { 1 } else { 0 },
            transform,
        });
    }

    /// Paint a strikethrough into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_strikethrough(
        &mut self,
        origin: Point<Pixels>,
        width: Pixels,
        style: &StrikethroughStyle,
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let local_thickness = style.thickness.scale(scale_factor);
        let bounds = Bounds {
            origin: origin.scale(scale_factor),
            size: size(width.scale(scale_factor), local_thickness),
        };
        let opacity = self.element_opacity();
        let Some(base_transform) = self.base_primitive_transform() else {
            return;
        };
        let Some((transform, thickness)) =
            self.underline_raster_projection(base_transform, bounds, local_thickness, 1.0)
        else {
            return;
        };

        self.insert_scene_primitive(Underline {
            order: 0,
            pad: 0,
            bounds,
            clip: Default::default(),
            thickness,
            color: style.color.unwrap_or_default().opacity(opacity),
            wavy: 0,
            transform,
        });
    }

    /// Paints a monochrome (non-emoji) glyph into the scene for the next frame at the current z-index.
    ///
    /// The y component of the origin is the baseline of the glyph.
    /// You should generally prefer to use the [`ShapedLine::paint`](crate::ShapedLine::paint) or
    /// [`WrappedLine::paint`](crate::WrappedLine::paint) methods in the [`TextSystem`](crate::TextSystem).
    /// This method is only useful if you need to paint a single glyph that has already been shaped.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_glyph(
        &mut self,
        origin: Point<Pixels>,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
        color: Hsla,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let scale_factor = self.scale_factor();
        let Some(base_transform) = self.base_primitive_transform() else {
            return Ok(());
        };
        let glyph_origin = origin.scale(scale_factor);

        let quantized_origin = Point::new(
            round_half_toward_zero(glyph_origin.x.0 * SUBPIXEL_VARIANTS_X as f32)
                / SUBPIXEL_VARIANTS_X as f32,
            round_half_toward_zero(glyph_origin.y.0 * SUBPIXEL_VARIANTS_Y as f32)
                / SUBPIXEL_VARIANTS_Y as f32,
        );
        let subpixel_variant = Point::new(
            (quantized_origin.x.fract() * SUBPIXEL_VARIANTS_X as f32) as u8,
            (quantized_origin.y.fract() * SUBPIXEL_VARIANTS_Y as f32) as u8,
        );
        let integer_origin = quantized_origin.map(|c| ScaledPixels(c.trunc()));
        let subpixel_rendering =
            base_transform.is_identity() && self.should_use_subpixel_rendering(font_id, font_size);
        let dilation = self.text_system().glyph_dilation_for_color(color);
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            subpixel_variant,
            scale_factor,
            is_emoji: false,
            subpixel_rendering,
            dilation,
        };

        let raster_bounds = self.text_system().raster_bounds(&params)?;
        if !raster_bounds.is_zero() {
            let tile = self
                .sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(&params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");
            let bounds = Bounds {
                origin: integer_origin + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let Some(transform) = self.primitive_raster_transform(
                base_transform,
                bounds,
                PrimitiveRasterSnap::NearestEdges,
            ) else {
                return Ok(());
            };
            if subpixel_rendering {
                self.insert_scene_primitive(SubpixelSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    clip: Default::default(),
                    color: color.opacity(element_opacity),
                    tile,
                    transform,
                });
            } else {
                self.insert_scene_primitive(MonochromeSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    clip: Default::default(),
                    color: color.opacity(element_opacity),
                    tile,
                    transform,
                });
            }
        }
        Ok(())
    }

    fn should_use_subpixel_rendering(&self, font_id: FontId, font_size: Pixels) -> bool {
        if self.platform_window.background_appearance() != WindowBackgroundAppearance::Opaque {
            return false;
        }

        if !self.platform_window.is_subpixel_rendering_supported() {
            return false;
        }

        let mode = match self.text_rendering_mode.get() {
            TextRenderingMode::PlatformDefault => self
                .text_system()
                .recommended_rendering_mode(font_id, font_size),
            mode => mode,
        };

        mode == TextRenderingMode::Subpixel
    }

    /// Paints an emoji glyph into the scene for the next frame at the current z-index.
    ///
    /// The y component of the origin is the baseline of the glyph.
    /// You should generally prefer to use the [`ShapedLine::paint`](crate::ShapedLine::paint) or
    /// [`WrappedLine::paint`](crate::WrappedLine::paint) methods in the [`TextSystem`](crate::TextSystem).
    /// This method is only useful if you need to paint a single emoji that has already been shaped.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_emoji(
        &mut self,
        origin: Point<Pixels>,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let Some(base_transform) = self.base_primitive_transform() else {
            return Ok(());
        };
        let glyph_origin = origin.scale(scale_factor);
        let integer_origin = glyph_origin.map(|c| ScaledPixels(round_half_toward_zero(c.0)));
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            subpixel_variant: Default::default(),
            scale_factor,
            is_emoji: true,
            subpixel_rendering: false,
            dilation: 0,
        };

        let raster_bounds = self.text_system().raster_bounds(&params)?;
        if !raster_bounds.is_zero() {
            let tile = self
                .sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(&params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");

            let bounds = Bounds {
                origin: integer_origin + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let Some(transform) = self.primitive_raster_transform(
                base_transform,
                bounds,
                PrimitiveRasterSnap::NearestEdges,
            ) else {
                return Ok(());
            };
            let opacity = self.element_opacity();

            self.insert_scene_primitive(PolychromeSprite {
                order: 0,
                pad: 0,
                grayscale: false,
                bounds,
                corner_radii: Default::default(),
                clip: Default::default(),
                tile,
                opacity,
                transform,
            });
        }
        Ok(())
    }

    /// Paint a monochrome SVG into the scene for the next frame at the current stacking context.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_svg(
        &mut self,
        bounds: Bounds<Pixels>,
        path: SharedString,
        mut data: Option<&[u8]>,
        color: Hsla,
        cx: &App,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let Some(base_transform) = self.base_primitive_transform() else {
            return Ok(());
        };
        let bounds = self.device_local_bounds(bounds);

        let params = RenderSvgParams {
            path,
            size: bounds.size.map(|pixels| {
                DevicePixels::from((pixels.0 * SMOOTH_SVG_SCALE_FACTOR).ceil() as i32)
            }),
        };

        let Some(tile) =
            self.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let Some((size, bytes)) = cx.svg_renderer.render_alpha_mask(&params, data)?
                    else {
                        return Ok(None);
                    };
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
        else {
            return Ok(());
        };
        let svg_bounds = Bounds {
            origin: bounds.center()
                - Point::new(
                    ScaledPixels(tile.bounds.size.width.0 as f32 / SMOOTH_SVG_SCALE_FACTOR / 2.),
                    ScaledPixels(tile.bounds.size.height.0 as f32 / SMOOTH_SVG_SCALE_FACTOR / 2.),
                ),
            size: tile
                .bounds
                .size
                .map(|value| ScaledPixels(value.0 as f32 / SMOOTH_SVG_SCALE_FACTOR)),
        };
        let Some(transform) = self.primitive_raster_transform(
            base_transform,
            svg_bounds,
            PrimitiveRasterSnap::NearestEdges,
        ) else {
            return Ok(());
        };

        self.insert_scene_primitive(MonochromeSprite {
            order: 0,
            pad: 0,
            bounds: svg_bounds,
            clip: Default::default(),
            color: color.opacity(element_opacity),
            tile,
            transform,
        });

        Ok(())
    }

    /// Paint an image into the scene for the next frame at the current z-index.
    /// This method will panic if the frame_index is not valid
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_image(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        data: Arc<RenderImage>,
        frame_index: usize,
        grayscale: bool,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let Some(base_transform) = self.base_primitive_transform() else {
            return Ok(());
        };
        let bounds = self.device_local_bounds(bounds);
        let Some(transform) = self.primitive_raster_transform(
            base_transform,
            bounds,
            PrimitiveRasterSnap::NearestEdges,
        ) else {
            return Ok(());
        };
        let params = RenderImageParams {
            image_id: data.id,
            frame_index,
        };

        let atlas_access =
            self.sprite_atlas
                .get_or_insert_with_diagnostics(&params.into(), &mut || {
                    Ok(Some((
                        data.size(frame_index),
                        Cow::Borrowed(
                            data.as_bytes(frame_index)
                                .expect("It's the caller's job to pass a valid frame index"),
                        ),
                    )))
                })?;
        let tile = atlas_access.tile.expect("Callback above only returns Some");
        let corner_radii = corner_radii.scale(self.scale_factor());
        let opacity = self.element_opacity();
        let atlas_diagnostic = atlas_access.diagnostic;

        let displayed_bounds = match transform.try_project_bounds(bounds) {
            Ok(bounds) => bounds,
            Err(error) => {
                self.record_subtree_transform_failure(error);
                return Ok(());
            }
        };
        if !self.insert_scene_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale,
            bounds,
            clip: Default::default(),
            corner_radii,
            tile,
            opacity,
            transform,
        }) {
            return Ok(());
        }
        let validity = self.subtree_geometry_validity();
        self.next_frame
            .atlas_access_diagnostic_entries
            .push(FrameOutput::new(atlas_diagnostic, validity.clone()));
        self.next_frame
            .image_paint_diagnostic_entries
            .push(FrameOutput::new(
                ImagePaintDiagnostic {
                    frame_generation: self.next_frame.generation,
                    image: params,
                    bounds: displayed_bounds,
                    tile,
                    atlas_access: atlas_diagnostic,
                },
                validity,
            ));
        Ok(())
    }

    /// Paint a surface into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    #[cfg(target_os = "macos")]
    pub fn paint_surface(&mut self, bounds: Bounds<Pixels>, image_buffer: PlatformPixelBuffer) {
        use crate::PaintSurface;

        self.invalidator.debug_assert_paint();

        let bounds = self.device_local_bounds(bounds);
        let Some(base_transform) = self.base_primitive_transform() else {
            return;
        };
        let Some(transform) = self.primitive_raster_transform(
            base_transform,
            bounds,
            PrimitiveRasterSnap::NearestEdges,
        ) else {
            return;
        };
        self.insert_scene_primitive(PaintSurface {
            order: 0,
            bounds,
            clip: Default::default(),
            image_buffer,
            transform,
        });
    }

    /// Removes an image from the sprite atlas.
    pub fn drop_image(&mut self, data: Arc<RenderImage>) -> Result<()> {
        for frame_index in 0..data.frame_count() {
            let params = RenderImageParams {
                image_id: data.id,
                frame_index,
            };

            let diagnostic = self.sprite_atlas.remove_with_diagnostics(&params.into());
            self.atlas_remove_diagnostics.push(diagnostic);
        }

        Ok(())
    }

    /// Records a side effect to commit after the current frame finishes prepainting and painting.
    ///
    /// Cached subtrees retain the record and enqueue it again when their prepaint journal is reused.
    /// Records created inside a failed [`Self::transact`] attempt are discarded before commit.
    pub fn record_prepaint_commit(&mut self, commit: impl Fn(u64, &mut App) + 'static) {
        self.record_prepaint_window_commit(move |revision, _, cx| commit(revision, cx));
    }

    /// Records a validity-gated side effect that may also update the current window.
    ///
    /// Use this when a prepaint measurement must not become observable until painting confirms
    /// that its transformed subtree is representable. The callback runs after painting and may
    /// request the follow-up frame that displays the committed state. It runs under the effective
    /// presentation state captured where the record was created.
    pub fn record_prepaint_window_commit(
        &mut self,
        commit: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) {
        self.record_prepaint_window_commit_in_phase(PrepaintCommitPhase::Normal, commit);
    }

    /// Records a validity-gated side effect after every normal prepaint commit for this frame.
    ///
    /// This callback observes focus and other window authority mutations made by
    /// [`Self::record_prepaint_window_commit`] and
    /// [`Self::record_prepaint_window_transaction`] in the same frame. Focus and blur mutations
    /// made from this callback are rejected, which keeps the observed focus authority stable for
    /// the callback's entire phase.
    pub fn record_prepaint_focus_stable_commit(
        &mut self,
        commit: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) {
        self.record_prepaint_window_commit_in_phase(PrepaintCommitPhase::FocusStable, commit);
    }

    fn record_prepaint_window_commit_in_phase(
        &mut self,
        phase: PrepaintCommitPhase,
        commit: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_prepaint();
        self.next_frame.prepaint_commits.push(FrameOutput::new(
            PrepaintCommit {
                phase,
                publication: None,
                presentation: self.subtree_presentation(),
                commit: PrepaintCommitCallback::Revision(Rc::new(commit)),
                discard: None,
            },
            self.subtree_geometry_validity(),
        ));
    }

    /// Records a validity-gated, cross-frame publication transaction.
    ///
    /// The commit callback runs only after a valid candidate has replaced the window's rendered
    /// frame and receives an [`AcceptedFrameFence`] proving that acceptance. The discard callback
    /// receives the same proof after an accepted frame establishes that the recorded subtree
    /// geometry is invalid. It also runs when a valid publication from the previous accepted frame
    /// is absent from the newly accepted frame, including when an enclosing [`Self::transact`]
    /// rolls back or an ancestor transform prevents this subtree from prepainting. A candidate
    /// rejected before the frame swap produces no fence, runs neither callback, and preserves the
    /// previously accepted publication.
    ///
    /// Use one stable [`PrepaintPublicationId`] for each logical publication and record it at most
    /// once per frame. Cached subtrees retain both the ID and callbacks in their frame journal.
    /// Valid commits run under their captured presentation state. Discards run suppressed because
    /// their producer has no interactive authority in the committed frame. Use this transaction,
    /// rather than [`Self::record_prepaint_window_commit`], for state whose public meaning must
    /// always agree with the currently rendered frame.
    pub fn record_prepaint_window_transaction(
        &mut self,
        publication: PrepaintPublicationId,
        commit: impl Fn(AcceptedFrameFence, &mut Window, &mut App) + 'static,
        discard: impl Fn(AcceptedFrameFence, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_prepaint();
        self.next_frame.prepaint_commits.push(FrameOutput::new(
            PrepaintCommit {
                phase: PrepaintCommitPhase::Normal,
                publication: Some(publication),
                presentation: self.subtree_presentation(),
                commit: PrepaintCommitCallback::AcceptedFrame(Rc::new(commit)),
                discard: Some(PrepaintCommitCallback::AcceptedFrame(Rc::new(discard))),
            },
            self.subtree_geometry_validity(),
        ));
    }

    pub(crate) fn record_autoscroll_commit(
        &mut self,
        intent: AutoscrollIntent,
        commit: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_prepaint();
        self.next_frame.prepaint_commits.push(FrameOutput::new(
            PrepaintCommit {
                phase: PrepaintCommitPhase::Normal,
                publication: None,
                presentation: self.subtree_presentation(),
                commit: PrepaintCommitCallback::Revision(Rc::new(commit)),
                discard: None,
            },
            intent.validity,
        ));
    }

    /// Add a node to the layout tree for the current frame. Takes the `Style` of the element for which
    /// layout is being requested, along with the layout ids of any children. This method is called during
    /// calls to the [`Element::request_layout`] trait method and enables any element to participate in layout.
    ///
    /// This method should only be called as part of the request_layout or prepaint phase of element drawing.
    #[must_use]
    pub fn request_layout(
        &mut self,
        style: Style,
        children: impl IntoIterator<Item = LayoutId>,
        cx: &mut App,
    ) -> LayoutId {
        self.invalidator.debug_assert_prepaint();

        cx.layout_id_buffer.clear();
        cx.layout_id_buffer.extend(children);
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();

        self.layout_engine.as_mut().unwrap().request_layout(
            style,
            rem_size,
            scale_factor,
            &cx.layout_id_buffer,
        )
    }

    /// Add a node to the layout tree for the current frame. Instead of taking a `Style` and children,
    /// this variant takes a function that is invoked during layout so you can use arbitrary logic to
    /// determine the element's size. One place this is used internally is when measuring text.
    ///
    /// The given closure is invoked at layout time with the known dimensions and available space and
    /// returns a `Size`.
    ///
    /// This method should only be called as part of the request_layout or prepaint phase of element drawing.
    pub fn request_measured_layout<F>(&mut self, style: Style, measure: F) -> LayoutId
    where
        F: Fn(Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App) -> Size<Pixels>
            + 'static,
    {
        self.invalidator.debug_assert_prepaint();

        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        let presentation = self.subtree_presentation();
        self.layout_engine
            .as_mut()
            .unwrap()
            .request_measured_layout(
                style,
                rem_size,
                scale_factor,
                move |known_dimensions, available_space, window, cx| {
                    window.with_subtree_presentation(presentation, |window| {
                        measure(known_dimensions, available_space, window, cx)
                    })
                },
            )
    }

    /// Compute the layout for the given id within the given available space.
    /// This method is called for its side effect, typically by the framework prior to painting.
    /// After calling it, you can request the bounds of the given layout node id or any descendant.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn compute_layout(
        &mut self,
        layout_id: LayoutId,
        available_space: Size<AvailableSpace>,
        cx: &mut App,
    ) {
        self.invalidator.debug_assert_prepaint();

        let mut layout_engine = self.layout_engine.take().unwrap();
        layout_engine.compute_layout(layout_id, available_space, self, cx);
        self.layout_engine = Some(layout_engine);
    }

    /// Obtain the bounds computed for the given LayoutId relative to the window. This method will usually be invoked by
    /// GPUI itself automatically in order to pass your element its `Bounds` automatically.
    ///
    /// This method should only be called as part of element drawing.
    pub fn layout_bounds(&mut self, layout_id: LayoutId) -> Bounds<Pixels> {
        self.invalidator.debug_assert_prepaint();

        let scale_factor = self.scale_factor();
        let mut bounds = self
            .layout_engine
            .as_mut()
            .unwrap()
            .layout_bounds(layout_id, scale_factor)
            .map(Into::into);
        let snapped_offset = self.pixel_snap_point(self.element_offset());
        bounds.origin += snapped_offset;
        bounds
    }

    /// Captures immutable geometry and exact initial-hit eligibility for `bounds`.
    ///
    /// This method should be called during `prepaint`. It does not register a pointer target;
    /// call [`Window::insert_hitbox`] when the element must participate in normal event routing.
    /// Adapters that only retain an exact region proof may use the returned snapshot during paint
    /// or later runtime arbitration.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn hit_test_snapshot(&self, bounds: Bounds<Pixels>) -> HitTestSnapshot {
        self.invalidator.debug_assert_prepaint();

        let clip_stack = self.clip_stack();
        let transform = self.subtree_transform();
        let validity = self.subtree_geometry_validity();
        let geometry = self.try_element_geometry(bounds);
        let active = geometry.is_ok() && self.subtree_presentation().is_interactive();
        HitTestSnapshot {
            geometry: geometry.unwrap_or_else(|_| {
                ElementGeometry::from_resolved(bounds, Bounds::default(), transform)
            }),
            validity,
            clip_stack,
            active,
        }
    }

    /// Inserts a region that can participate in pointer routing.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn insert_hitbox(&mut self, bounds: Bounds<Pixels>, behavior: HitboxBehavior) -> Hitbox {
        let snapshot = self.hit_test_snapshot(bounds);
        let mut id = self.next_hitbox_id;
        self.next_hitbox_id = self.next_hitbox_id.next();
        let hitbox = Hitbox {
            id,
            geometry: snapshot.geometry,
            validity: snapshot.validity,
            clip_stack: snapshot.clip_stack,
            behavior,
            active: snapshot.active,
        };
        if hitbox.active {
            self.next_frame.hitboxes.push(hitbox.clone());
        }
        hitbox
    }

    pub(crate) fn committed_hitbox(&self, id: HitboxId) -> Option<Hitbox> {
        self.rendered_frame
            .hitboxes
            .iter()
            .find(|hitbox| hitbox.id == id && hitbox.is_active())
            .cloned()
    }

    pub(crate) fn preparing_frame_generation(&self) -> u64 {
        self.next_frame.generation
    }

    pub(crate) fn preparing_frame_attempt_id(&self) -> u64 {
        self.candidate_frame_transaction
            .as_ref()
            .expect(
                "frame-attempt identity is available only while building or committing a candidate",
            )
            .attempt_id
            .0
    }

    pub(super) fn current_interaction_frame(&self) -> &Frame {
        if self.next_frame.generation > self.rendered_frame.generation {
            &self.next_frame
        } else {
            &self.rendered_frame
        }
    }

    pub(crate) fn prepared_hitbox(&self, id: HitboxId) -> Option<Hitbox> {
        self.next_frame
            .hitboxes
            .iter()
            .find(|hitbox| hitbox.id == id && hitbox.is_active())
            .cloned()
    }

    /// Set a hitbox which will act as a control area of the platform window.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn insert_window_control_hitbox(&mut self, area: WindowControlArea, hitbox: Hitbox) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame.window_control_hitboxes.push((area, hitbox));
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn record_debug_bounds(&mut self, selector: String, bounds: Bounds<Pixels>) {
        self.invalidator.debug_assert_paint_or_prepaint();
        let layout_bounds = bounds;
        let Ok(displayed_bounds) = self.try_project_subtree_bounds(layout_bounds) else {
            return;
        };
        self.next_frame
            .debug_bounds
            .insert(selector.clone(), displayed_bounds);
        self.next_frame.debug_bounds_entries.push((
            selector,
            displayed_bounds,
            self.subtree_geometry_validity(),
        ));
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn record_debug_focus(&mut self, selector: String, focus_id: FocusId) {
        self.invalidator.debug_assert_paint_or_prepaint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame
            .debug_focus_handles
            .insert(selector.clone(), focus_id);
        self.next_frame.debug_focus_entries.push((
            selector,
            focus_id,
            self.subtree_geometry_validity(),
        ));
    }

    /// Sets the key context for the current element. This context will be used to translate
    /// keybindings into actions.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn set_key_context(&mut self, context: KeyContext) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame.dispatch_tree.set_key_context(context);
    }

    /// Sets the focus handle for the current element. This handle will be used to manage focus state
    /// and keyboard event dispatch for the element.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn set_focus_handle(&mut self, focus_handle: &FocusHandle, _: &App) {
        self.invalidator.debug_assert_prepaint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame.dispatch_tree.set_focus_id(focus_handle.id);
        if let Some(layout_id) = self.current_prepaint_layout_id() {
            let bounds = self.layout_bounds(layout_id);
            self.bind_focus_reveal_target(focus_handle.id, bounds);
        }
        self.promote_pending_focus_claim();
        if focus_handle.is_focused(self) {
            self.next_frame.focus = Some(focus_handle.id);
        }
    }

    /// Associates the current accessibility node with tree focus without admitting input.
    ///
    /// This is a candidate-frame-only override. It intentionally does not add the focus identity
    /// to the dispatch tree, bind a reveal target, or mutate GPUI's committed input focus.
    pub(crate) fn set_accessibility_focus_handle(&mut self, focus_handle: &FocusHandle) {
        self.invalidator.debug_assert_prepaint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        match self.candidate_accessibility_focus {
            None => self.candidate_accessibility_focus = Some(focus_handle.id),
            Some(current) => debug_assert_eq!(
                current, focus_handle.id,
                "one candidate frame cannot publish conflicting accessibility-only focus owners"
            ),
        }
    }

    /// Sets the view id for the current element, which will be used to manage view caching.
    ///
    /// This method should only be called as part of element prepaint. We plan on removing this
    /// method eventually when we solve some issues that require us to construct editor elements
    /// directly instead of always using editors via views.
    pub fn set_view_id(&mut self, view_id: EntityId) {
        self.invalidator.debug_assert_prepaint();
        self.next_frame.dispatch_tree.set_view_id(view_id);
    }

    /// Get the entity ID for the currently rendering view
    pub fn current_view(&self) -> EntityId {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.rendered_entity_stack.last().copied().unwrap()
    }

    #[inline]
    pub(crate) fn with_rendered_view<R>(
        &mut self,
        id: EntityId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.rendered_entity_stack.push(id);
        let result = f(self);
        self.rendered_entity_stack.pop();
        result
    }

    /// Executes the provided function with the specified image cache.
    pub fn with_image_cache<F, R>(&mut self, image_cache: Option<AnyImageCache>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        if let Some(image_cache) = image_cache {
            self.image_cache_stack.push(image_cache);
            let result = f(self);
            self.image_cache_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// Sets an input handler, such as [`ElementInputHandler`][element_input_handler], which interfaces with the
    /// platform to receive textual input with proper integration with concerns such
    /// as IME interactions. This handler will be active for the upcoming frame until the following frame is
    /// rendered.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    ///
    /// [element_input_handler]: crate::ElementInputHandler
    pub fn handle_input(
        &mut self,
        focus_handle: &FocusHandle,
        input_handler: impl InputHandler,
        cx: &App,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }

        if focus_handle.is_focused(self) {
            let cx = self.to_async(cx);
            let transform = self.subtree_transform();
            let validity = self.subtree_geometry_validity();
            self.next_frame.input_handlers.push(FrameOutput::new(
                Some(PlatformInputHandler::new(
                    cx,
                    focus_handle.id,
                    Box::new(input_handler),
                    transform,
                    validity.clone(),
                )),
                validity,
            ));
        }
    }

    /// Register a mouse event listener on the window for the next frame. The type of event
    /// is determined by the first parameter of the given listener. When the next frame is rendered
    /// the listener will be cleared.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_mouse_event<Event: MouseEvent>(
        &mut self,
        mut listener: impl FnMut(&Event, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }

        self.next_frame.mouse_listeners.push(FrameOutput::new(
            Some(Box::new(
                move |event: &dyn Any, phase: DispatchPhase, window: &mut Window, cx: &mut App| {
                    if let Some(event) = event.downcast_ref() {
                        listener(event, phase, window, cx)
                    }
                },
            )),
            self.subtree_geometry_validity(),
        ));
    }

    /// Register a pointer-cancellation listener on the window for the next frame.
    ///
    /// Cancellation listeners run in both dispatch phases and cannot stop later cancellation
    /// listeners. This method should only be called as part of the paint phase of element drawing.
    pub fn on_pointer_cancel(
        &mut self,
        listener: impl FnMut(&PointerCancelEvent, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.next_frame
            .pointer_cancel_listeners
            .push(FrameOutput::new(
                Some(Rc::new(RefCell::new(Box::new(listener)))),
                self.subtree_geometry_validity(),
            ));
    }

    /// Registers a persistent mouse and pointer interceptor owned by this window.
    ///
    /// Interceptors run after platform input normalization and before frame-scoped capture and
    /// bubble listeners. Stop propagation to keep the event from reaching those listeners;
    /// otherwise the original event continues once. Mouse-up cleanup, including active drag and
    /// captured-pointer release, still runs when an interceptor consumes the event.
    pub fn intercept_window_mouse_events(
        &mut self,
        mut listener: impl for<'a> FnMut(WindowMouseEvent<'a>, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.mouse_interceptors.insert(
            (),
            Box::new(move |event, window, cx| {
                listener(event, window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Register a key event listener on this node for the next frame. The type of event
    /// is determined by the first parameter of the given listener. When the next frame is rendered
    /// the listener will be cleared.
    ///
    /// This is a fairly low-level method, so prefer using event handlers on elements unless you have
    /// a specific need to register a listener yourself.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_key_event<Event: KeyEvent>(
        &mut self,
        listener: impl Fn(&Event, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }

        self.next_frame.dispatch_tree.on_key_event(Rc::new(
            move |event: &dyn Any, phase, window: &mut Window, cx: &mut App| {
                if let Some(event) = event.downcast_ref::<Event>() {
                    listener(event, phase, window, cx)
                }
            },
        ));
    }

    /// Registers a persistent key-down interceptor owned by this window.
    ///
    /// Interceptors run before application interceptors, key bindings, actions, and node listeners.
    pub fn intercept_window_key_down(
        &mut self,
        mut listener: impl FnMut(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.key_down_interceptors.insert(
            (),
            Box::new(move |event, window, cx| {
                listener(event, window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Register a modifiers changed event listener on the window for the next frame.
    ///
    /// This is a fairly low-level method, so prefer using event handlers on elements unless you have
    /// a specific need to register a global listener.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_modifiers_changed(
        &mut self,
        listener: impl Fn(&ModifiersChangedEvent, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }

        self.next_frame.dispatch_tree.on_modifiers_changed(Rc::new(
            move |event: &ModifiersChangedEvent, window: &mut Window, cx: &mut App| {
                listener(event, window, cx)
            },
        ));
    }

    /// Register a listener to be called when the given focus handle or one of its descendants receives focus.
    /// This does not fire if the given focus handle - or one of its descendants - was previously focused.
    /// Returns a subscription and persists until the subscription is dropped.
    pub fn on_focus_in(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        mut listener: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if event.is_focus_in(focus_id) {
                    listener(window, cx);
                }
                true
            }));
        cx.defer(move |_| activate());
        subscription
    }

    /// Register a listener for the given focus handle becoming the exact committed local focus.
    ///
    /// Unlike [`Self::on_focus_in`], this observes window-local focus independently of platform
    /// window activation. It does not fire again when an already-committed focus path merely
    /// becomes platform-active.
    pub fn on_focus_committed(
        &mut self,
        handle: &FocusHandle,
        _cx: &mut App,
        mut listener: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if event.is_focus_committed(focus_id) {
                    listener(window, cx);
                }
                true
            }));
        activate();
        subscription
    }

    /// Register a listener for the given focus handle or one of its descendants becoming part of
    /// this window's committed local focus path.
    ///
    /// Unlike [`Self::on_focus_in`], this observes window-local focus independently of platform
    /// window activation. It does not fire again when an already-committed focus path merely
    /// becomes platform-active.
    pub fn on_focus_committed_in(
        &mut self,
        handle: &FocusHandle,
        _cx: &mut App,
        mut listener: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if event.is_focus_committed_in(focus_id) {
                    listener(window, cx);
                }
                true
            }));
        activate();
        subscription
    }

    /// Register a listener to be called when the given focus handle or one of its descendants loses focus.
    /// Returns a subscription and persists until the subscription is dropped.
    pub fn on_focus_out(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        mut listener: impl FnMut(FocusOutEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if let Some(blurred_id) = event.previous_focus_path.last().copied()
                    && event.is_focus_out(focus_id)
                {
                    let event = FocusOutEvent {
                        blurred: WeakFocusHandle {
                            id: blurred_id,
                            handles: Arc::downgrade(&cx.focus_handles),
                        },
                    };
                    listener(event, window, cx)
                }
                true
            }));
        cx.defer(move |_| activate());
        subscription
    }

    fn reset_cursor_style(&self, _cx: &mut App) {
        let style = self
            .rendered_frame
            .cursor_style(self)
            .unwrap_or(CursorStyle::Arrow);
        self.platform_window.set_cursor_style(style);
    }

    /// Dispatch a given keystroke as though the user had typed it.
    /// You can create a keystroke with Keystroke::parse("").
    pub fn dispatch_keystroke(&mut self, keystroke: Keystroke, cx: &mut App) -> bool {
        self.with_input_transaction(cx, move |window, cx| {
            let keystroke = keystroke.with_simulated_ime();
            let result = window.dispatch_event(
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );
            if !result.propagate {
                return true;
            }
            if window.removal_state != WindowRemovalState::Open {
                return true;
            }

            if let Some(input) = keystroke.key_char
                && let Some(mut input_handler) = window.platform_window.take_input_handler()
            {
                input_handler.dispatch_input(&input, window, cx);
                window.platform_window.set_input_handler(input_handler);
                return true;
            }

            false
        })
    }

    /// Return a key binding string for an action, to display in the UI. Uses the highest precedence
    /// binding for the action (last binding added to the keymap).
    pub fn keystroke_text_for(&self, action: &dyn Action) -> String {
        self.highest_precedence_binding_for_action(action)
            .map(|binding| {
                binding
                    .keystrokes()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| action.name().to_string())
    }

    /// Dispatch a mouse or keyboard event on the window.
    ///
    /// Synchronous reentrant dispatch is rejected as consumed. This keeps nested calls from
    /// bypassing persistent interceptors or overwriting the outer event's shared dispatch state.
    #[profiling::function]
    pub fn dispatch_event(&mut self, event: PlatformInput, cx: &mut App) -> DispatchEventResult {
        let incoming_pointer_cancel = matches!(&event, PlatformInput::PointerCanceled(_));
        if self.flush_pending_pointer_cancellations(cx) && incoming_pointer_cancel {
            return self
                .last_dispatch_event_result
                .unwrap_or(DispatchEventResult {
                    propagate: true,
                    default_prevented: false,
                });
        }
        if !incoming_pointer_cancel && !self.provisional_session_accepts_interaction() {
            return DispatchEventResult {
                propagate: false,
                default_prevented: true,
            };
        }
        self.dispatch_event_without_pending_pointer_cancellations(event, cx)
    }

    fn dispatch_event_without_pending_pointer_cancellations(
        &mut self,
        event: PlatformInput,
        cx: &mut App,
    ) -> DispatchEventResult {
        self.with_input_transaction(cx, move |window, cx| window.dispatch_event_inner(event, cx))
    }

    fn dispatch_event_inner(&mut self, event: PlatformInput, cx: &mut App) -> DispatchEventResult {
        let Some(dispatch_guard) =
            InputDispatchGuard::try_enter(self.input_dispatch_active.clone())
        else {
            return DispatchEventResult {
                propagate: false,
                default_prevented: true,
            };
        };

        #[cfg(feature = "input-latency-histogram")]
        let dispatch_time = Instant::now();
        let update_count_before = self.invalidator.update_count();
        let event = input_dispatch::prepare_platform_input(self, cx, event);

        if let Some(any_mouse_event) = event.mouse_event() {
            self.dispatch_mouse_event(any_mouse_event, cx);
            self.commit_native_window_control_area(cx);
        } else if let Some(any_key_event) = event.keyboard_event() {
            self.dispatch_key_event(any_key_event, cx);
        }

        if self.invalidator.update_count() > update_count_before {
            self.input_rate_tracker.borrow_mut().record_input();
            #[cfg(feature = "input-latency-histogram")]
            if self.invalidator.can_schedule_refresh() && !self.invalidator.is_focus_phase() {
                self.input_latency_tracker.record_input(dispatch_time);
            } else {
                self.input_latency_tracker.record_mid_draw_input();
            }
        }

        let result = DispatchEventResult {
            propagate: cx.propagate_event,
            default_prevented: self.default_prevented,
        };
        self.last_dispatch_event_result = Some(result);
        drop(dispatch_guard);
        result
    }

    fn commit_native_window_control_area(&self, cx: &App) {
        let area = self
            .rendered_frame
            .window_control_hitboxes
            .iter()
            .find_map(|(area, hitbox)| {
                (hitbox.is_active() && self.mouse_hit_test.ids.contains(&hitbox.id))
                    .then_some(*area)
            });
        cx.set_native_window_control_area(self.handle.window_id(), area);
    }

    fn dispatch_mouse_event(&mut self, event: &dyn Any, cx: &mut App) {
        if let Some(event) = event.downcast_ref::<crate::MouseDownEvent>() {
            self.pressed_mouse_buttons.insert(event.button);
        }
        cx.lock_native_captured_drag_event(self.handle.window_id(), self, event);
        let dispatch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let is_pointer_cancel = event.is::<PointerCancelEvent>();
            let exited_window = event.is::<crate::MouseExitEvent>();
            let hit_test = if exited_window {
                HitTest::default()
            } else {
                self.rendered_frame.hit_test(self.mouse_position())
            };
            if exited_window || hit_test != self.mouse_hit_test {
                self.mouse_hit_test = hit_test;
                self.reset_cursor_style(cx);
            }

            let routes_to_captured_target = !self.pointer_cancel_session_already_settled
                && (event.is::<crate::MouseDownEvent>()
                    || event.is::<MouseUpEvent>()
                    || event.is::<MouseMoveEvent>()
                    || event.is::<crate::MousePressureEvent>()
                    || is_pointer_cancel);
            let _captured_target = routes_to_captured_target
                .then(|| self.captured_pointer_hitbox())
                .flatten()
                .map(|hitbox| {
                    MouseEventTargetGuard::enter(self.mouse_event_target.clone(), hitbox)
                });

            #[cfg(any(feature = "inspector", debug_assertions))]
            let inspector_picking = !is_pointer_cancel && self.is_inspector_picking(cx);
            #[cfg(not(any(feature = "inspector", debug_assertions)))]
            let inspector_picking = false;
            #[cfg(any(feature = "inspector", debug_assertions))]
            if inspector_picking {
                self.handle_inspector_mouse_event(event, cx);
            }

            if !inspector_picking && let Some(event) = WindowMouseEvent::from_any(event) {
                self.mouse_interceptors.clone().retain(&(), |interceptor| {
                    if is_pointer_cancel || cx.propagate_event {
                        interceptor(event, self, cx)
                    } else {
                        true
                    }
                });
            }

            if !inspector_picking && let Some(event) = event.downcast_ref::<PointerCancelEvent>() {
                let mut listeners = mem::take(&mut self.rendered_frame.pointer_cancel_listeners);
                let listener_result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        for output in &mut listeners {
                            if output.is_valid()
                                && let Some(listener) = output.value.as_ref()
                            {
                                listener.borrow_mut()(event, DispatchPhase::Capture, self, cx);
                            }
                        }
                        for output in listeners.iter_mut().rev() {
                            if output.is_valid()
                                && let Some(listener) = output.value.as_ref()
                            {
                                listener.borrow_mut()(event, DispatchPhase::Bubble, self, cx);
                            }
                        }
                    }));
                self.rendered_frame.pointer_cancel_listeners = listeners;
                if let Err(payload) = listener_result {
                    std::panic::resume_unwind(payload);
                }
            } else if !inspector_picking {
                let mut listeners = mem::take(&mut self.rendered_frame.mouse_listeners);
                let listener_result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        // Capture phase, events bubble from back to front. Handlers for this phase are
                        // used for special purposes, such as detecting events outside of a given Bounds.
                        if cx.propagate_event {
                            for output in &mut listeners {
                                if !output.is_valid() {
                                    continue;
                                }
                                let Some(listener) = output.value.as_mut() else {
                                    continue;
                                };
                                listener(event, DispatchPhase::Capture, self, cx);
                                if !cx.propagate_event {
                                    break;
                                }
                            }
                        }

                        // Bubble phase, where most normal handlers do their work.
                        if cx.propagate_event {
                            for output in listeners.iter_mut().rev() {
                                if !output.is_valid() {
                                    continue;
                                }
                                let Some(listener) = output.value.as_mut() else {
                                    continue;
                                };
                                listener(event, DispatchPhase::Bubble, self, cx);
                                if !cx.propagate_event {
                                    break;
                                }
                            }
                        }
                    }));
                self.rendered_frame.mouse_listeners = listeners;
                if let Err(payload) = listener_result {
                    std::panic::resume_unwind(payload);
                }
            }
        }));
        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.finish_mouse_session_event(event, cx);
        }));
        match (dispatch, cleanup) {
            (Ok(()), Ok(())) => {}
            (Err(payload), Ok(())) | (Err(payload), Err(_)) => {
                std::panic::resume_unwind(payload);
            }
            (Ok(()), Err(payload)) => std::panic::resume_unwind(payload),
        }
    }

    fn dispatch_key_event(&mut self, event: &dyn Any, cx: &mut App) {
        if event.is::<KeyDownEvent>() || event.is::<KeyUpEvent>() {
            self.key_event_revision = self.key_event_revision.wrapping_add(1);
        }

        if self.invalidator.is_dirty() {
            self.draw(cx).clear();
        }

        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);

        let mut keystroke: Option<Keystroke> = None;

        if let Some(event) = event.downcast_ref::<ModifiersChangedEvent>() {
            if event.modifiers.number_of_modifiers() == 0
                && self.pending_modifier.modifiers.number_of_modifiers() == 1
                && !self.pending_modifier.saw_keystroke
            {
                let key = match self.pending_modifier.modifiers {
                    modifiers if modifiers.shift => Some("shift"),
                    modifiers if modifiers.control => Some("control"),
                    modifiers if modifiers.alt => Some("alt"),
                    modifiers if modifiers.platform => Some("platform"),
                    modifiers if modifiers.function => Some("function"),
                    _ => None,
                };
                if let Some(key) = key {
                    keystroke = Some(Keystroke {
                        key: key.to_string(),
                        key_char: None,
                        modifiers: Modifiers::default(),
                    });
                }
            }

            if self.pending_modifier.modifiers.number_of_modifiers() == 0
                && event.modifiers.number_of_modifiers() == 1
            {
                self.pending_modifier.saw_keystroke = false
            }
            self.pending_modifier.modifiers = event.modifiers
        } else if let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() {
            self.pending_modifier.saw_keystroke = true;
            keystroke = Some(key_down_event.keystroke.clone());
            if key_down_event.keystroke.key_char.is_some()
                && matches!(
                    cx.cursor_hide_mode,
                    CursorHideMode::OnTyping | CursorHideMode::OnTypingAndAction
                )
            {
                cx.platform.hide_cursor_until_mouse_moves();
            }
        }

        let Some(keystroke) = keystroke else {
            self.finish_dispatch_key_event(event, dispatch_path, self.context_stack(), cx);
            return;
        };

        cx.propagate_event = true;
        if let Some(event) = event.downcast_ref::<KeyDownEvent>() {
            self.key_down_interceptors
                .clone()
                .retain(&(), |interceptor| {
                    if cx.propagate_event {
                        interceptor(event, self, cx)
                    } else {
                        true
                    }
                });
        }
        if !cx.propagate_event {
            let current_context_stack = self.context_stack();
            let to_replay = self.pending_input.take().and_then(|pending| {
                (pending.focus == self.focus
                    && pending.context_stack.as_ref() == Some(&current_context_stack))
                .then(|| {
                    self.rendered_frame
                        .dispatch_tree
                        .flush_dispatch(pending.keystrokes, &dispatch_path)
                })
            });
            self.pending_input_changed(cx);
            if let Some(to_replay) = to_replay {
                self.replay_pending_input(to_replay, cx);
            }
            cx.stop_propagation();
            return;
        }
        self.dispatch_keystroke_interceptors(event, self.context_stack(), cx);
        if !cx.propagate_event {
            self.finish_dispatch_key_event(event, dispatch_path, self.context_stack(), cx);
            return;
        }

        let mut currently_pending = self.pending_input.take().unwrap_or_default();
        let current_context_stack = self.context_stack();
        if currently_pending.context_stack.is_some()
            && (currently_pending.focus != self.focus
                || currently_pending
                    .context_stack
                    .as_ref()
                    .is_some_and(|pending_context| pending_context != &current_context_stack))
        {
            currently_pending = PendingInput::default();
        }

        let match_result = self.rendered_frame.dispatch_tree.dispatch_key(
            currently_pending.keystrokes,
            keystroke,
            &dispatch_path,
        );

        if !match_result.to_replay.is_empty() {
            self.replay_pending_input(match_result.to_replay, cx);
            cx.propagate_event = true;
        }

        if !match_result.pending.is_empty() {
            currently_pending.timer.take();
            currently_pending.keystrokes = match_result.pending;
            currently_pending.focus = self.focus;
            currently_pending.context_stack = Some(current_context_stack);

            let text_input_requires_timeout = event
                .downcast_ref::<KeyDownEvent>()
                .filter(|key_down| key_down.keystroke.key_char.is_some())
                .and_then(|_| self.platform_window.take_input_handler())
                .map_or(false, |mut input_handler| {
                    let accepts = input_handler.accepts_text_input(self, cx);
                    self.platform_window.set_input_handler(input_handler);
                    accepts
                });

            currently_pending.needs_timeout |=
                match_result.pending_has_binding || text_input_requires_timeout;

            if currently_pending.needs_timeout {
                currently_pending.timer = Some(self.spawn(cx, async move |cx| {
                    cx.background_executor.timer(Duration::from_secs(1)).await;
                    cx.update_when_available(move |window, cx| {
                        let current_context_stack = window.context_stack();
                        let Some(currently_pending) = window.pending_input.take() else {
                            return;
                        };
                        if currently_pending.focus != window.focus
                            || currently_pending.context_stack.as_ref()
                                != Some(&current_context_stack)
                        {
                            window.pending_input_changed(cx);
                            return;
                        }

                        let node_id = window.focus_node_id_in_rendered_frame(window.focus);
                        let dispatch_path =
                            window.rendered_frame.dispatch_tree.dispatch_path(node_id);

                        let to_replay = window
                            .rendered_frame
                            .dispatch_tree
                            .flush_dispatch(currently_pending.keystrokes, &dispatch_path);

                        window.pending_input_changed(cx);
                        window.replay_pending_input(to_replay, cx)
                    })
                    .await
                    .log_err();
                }));
            } else {
                currently_pending.timer = None;
            }
            self.pending_input = Some(currently_pending);
            self.pending_input_changed(cx);
            cx.propagate_event = false;
            return;
        }

        let skip_bindings = event
            .downcast_ref::<KeyDownEvent>()
            .filter(|key_down_event| key_down_event.prefer_character_input)
            .map(|_| {
                self.platform_window
                    .take_input_handler()
                    .map_or(false, |mut input_handler| {
                        let accepts = input_handler.accepts_text_input(self, cx);
                        self.platform_window.set_input_handler(input_handler);
                        // If modifiers are not excessive (e.g. AltGr), and the input handler is accepting text input,
                        // we prefer the text input over bindings.
                        accepts
                    })
            })
            .unwrap_or(false);

        if !skip_bindings {
            for binding in match_result.bindings {
                self.dispatch_action_on_node(node_id, binding.action.as_ref(), cx);
                if !cx.propagate_event {
                    self.dispatch_keystroke_observers(
                        event,
                        Some(binding.action),
                        match_result.context_stack,
                        cx,
                    );
                    self.pending_input_changed(cx);
                    return;
                }
            }
        }

        self.finish_dispatch_key_event(event, dispatch_path, match_result.context_stack, cx);
        self.pending_input_changed(cx);
    }

    fn finish_dispatch_key_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: SmallVec<[DispatchNodeId; 32]>,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        self.dispatch_key_down_up_event(event, &dispatch_path, cx);
        if !cx.propagate_event {
            return;
        }

        self.dispatch_modifiers_changed_event(event, &dispatch_path, cx);
        if !cx.propagate_event {
            return;
        }

        self.dispatch_keystroke_observers(event, None, context_stack, cx);
    }

    pub(crate) fn pending_input_changed(&mut self, cx: &mut App) {
        self.pending_input_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    fn dispatch_key_down_up_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
        cx: &mut App,
    ) {
        // Capture phase
        for node_id in dispatch_path {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);

            for key_listener in node.key_listeners.clone() {
                key_listener(event, DispatchPhase::Capture, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }

        // Bubble phase
        for node_id in dispatch_path.iter().rev() {
            // Handle low level key events
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for key_listener in node.key_listeners.clone() {
                key_listener(event, DispatchPhase::Bubble, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }
    }

    fn dispatch_modifiers_changed_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
        cx: &mut App,
    ) {
        let Some(event) = event.downcast_ref::<ModifiersChangedEvent>() else {
            return;
        };
        for node_id in dispatch_path.iter().rev() {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for listener in node.modifiers_changed_listeners.clone() {
                listener(event, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }
    }

    /// Determine whether a potential multi-stroke key binding is in progress on this window.
    pub fn has_pending_keystrokes(&self) -> bool {
        self.pending_input.is_some()
    }

    pub(crate) fn clear_pending_keystrokes(&mut self) {
        if self.invalidator.is_building_frame() {
            self.candidate_pending_input_clear = true;
        } else {
            self.pending_input.take();
        }
    }

    /// Returns the currently pending input keystrokes that might result in a multi-stroke key binding.
    pub fn pending_input_keystrokes(&self) -> Option<&[Keystroke]> {
        self.pending_input
            .as_ref()
            .map(|pending_input| pending_input.keystrokes.as_slice())
    }

    fn replay_pending_input(&mut self, replays: SmallVec<[Replay; 1]>, cx: &mut App) {
        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);

        'replay: for replay in replays {
            let event = KeyDownEvent {
                keystroke: replay.keystroke.clone(),
                is_held: false,
                prefer_character_input: true,
            };

            cx.propagate_event = true;
            for binding in replay.bindings {
                self.dispatch_action_on_node(node_id, binding.action.as_ref(), cx);
                if !cx.propagate_event {
                    self.dispatch_keystroke_observers(
                        &event,
                        Some(binding.action),
                        Vec::default(),
                        cx,
                    );
                    continue 'replay;
                }
            }

            self.dispatch_key_down_up_event(&event, &dispatch_path, cx);
            if !cx.propagate_event {
                continue 'replay;
            }
            if let Some(input) = replay.keystroke.key_char.as_ref().cloned()
                && let Some(mut input_handler) = self.platform_window.take_input_handler()
            {
                input_handler.dispatch_input(&input, self, cx);
                self.platform_window.set_input_handler(input_handler)
            }
        }
    }

    fn focus_node_id_in_rendered_frame(&self, focus_id: Option<FocusId>) -> DispatchNodeId {
        focus_id
            .and_then(|focus_id| {
                self.rendered_frame
                    .dispatch_tree
                    .focusable_node_id(focus_id)
            })
            .unwrap_or_else(|| self.rendered_frame.dispatch_tree.root_node_id())
    }

    fn dispatch_action_on_node(
        &mut self,
        node_id: DispatchNodeId,
        action: &dyn Action,
        cx: &mut App,
    ) {
        self.dispatch_action_on_node_inner(node_id, action, cx);

        if !cx.propagate_event
            && cx.cursor_hide_mode == CursorHideMode::OnTypingAndAction
            && self.last_input_was_keyboard()
        {
            cx.platform.hide_cursor_until_mouse_moves();
        }
    }

    fn dispatch_action_on_node_inner(
        &mut self,
        node_id: DispatchNodeId,
        action: &dyn Action,
        cx: &mut App,
    ) {
        let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);

        // Capture phase for global actions.
        cx.propagate_event = true;
        if let Some(mut global_listeners) = cx
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in &global_listeners {
                profiler::update_running_action(action, cx);
                listener(action.as_any(), DispatchPhase::Capture, cx);
                profiler::save_action_timing();
                if !cx.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                cx.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            cx.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }

        if !cx.propagate_event {
            return;
        }

        // Capture phase for window actions.
        for node_id in &dispatch_path {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for DispatchActionListener {
                action_type,
                listener,
            } in node.action_listeners.clone()
            {
                let any_action = action.as_any();
                if action_type == any_action.type_id() {
                    profiler::update_running_action(action, cx);
                    listener(any_action, DispatchPhase::Capture, self, cx);
                    profiler::save_action_timing();

                    if !cx.propagate_event {
                        return;
                    }
                }
            }
        }

        // Bubble phase for window actions.
        for node_id in dispatch_path.iter().rev() {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for DispatchActionListener {
                action_type,
                listener,
            } in node.action_listeners.clone()
            {
                let any_action = action.as_any();
                if action_type == any_action.type_id() {
                    cx.propagate_event = false; // Actions stop propagation by default during the bubble phase
                    profiler::update_running_action(action, cx);
                    listener(any_action, DispatchPhase::Bubble, self, cx);
                    profiler::save_action_timing();

                    if !cx.propagate_event {
                        return;
                    }
                }
            }
        }

        // Bubble phase for global actions.
        if let Some(mut global_listeners) = cx
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in global_listeners.iter().rev() {
                cx.propagate_event = false; // Actions stop propagation by default during the bubble phase

                profiler::update_running_action(action, cx);
                listener(action.as_any(), DispatchPhase::Bubble, cx);
                profiler::save_action_timing();
                if !cx.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                cx.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            cx.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }
    }

    /// Register the given handler to be invoked whenever the global of the given type
    /// is updated.
    pub fn observe_global<G: Global>(
        &mut self,
        cx: &mut App,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let window_handle = self.handle;
        let (subscription, activate) = cx.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                window_handle
                    .update(cx, |_, window, cx| f(window, cx))
                    .is_ok()
            }),
        );
        cx.defer(move |_| activate());
        subscription
    }

    /// Focus the current window and bring it to the foreground at the platform level.
    pub fn activate_window(&self) {
        if self.provisional_session_accepts_interaction() {
            self.platform_command_sink
                .enqueue(PlatformWindowCommand::Activate);
        }
    }

    /// Requests minimized placement through the placement authority.
    pub fn minimize_window(&mut self) -> WindowMutationDispatch {
        self.request_window_placement_request(WindowPlacementRequest::minimized())
    }

    /// Toggles fullscreen placement through the placement authority.
    pub fn toggle_fullscreen(&mut self) -> WindowMutationDispatch {
        let state = if self.platform_facts.is_fullscreen {
            WindowPlacementState::Windowed
        } else {
            WindowPlacementState::Fullscreen
        };
        self.request_window_placement_request(WindowPlacementRequest {
            state: Some(state),
            ..WindowPlacementRequest::new()
        })
    }

    /// Updates the IME panel position suggestions for languages like japanese, chinese.
    pub fn invalidate_character_coordinates(&mut self) {
        self.refresh();
    }

    fn update_ime_position_from_committed_handler(&mut self, cx: &mut App) {
        let Some(mut input_handler) = self.platform_window.take_input_handler() else {
            return;
        };
        if let Some(bounds) = input_handler.selected_bounds(self, cx) {
            self.platform_window.update_ime_position(bounds);
        }
        self.platform_window.set_input_handler(input_handler);
    }

    /// Present a platform dialog.
    /// The provided message will be presented, along with buttons for each answer.
    /// When a button is clicked, the returned Receiver will receive the index of the clicked button.
    pub fn prompt<T>(
        &mut self,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[T],
        cx: &mut App,
    ) -> oneshot::Receiver<usize>
    where
        T: Clone + Into<PromptButton>,
    {
        let prompt_builder = cx.prompt_builder.take();
        let Some(prompt_builder) = prompt_builder else {
            unreachable!("Re-entrant window prompting is not supported by GPUI");
        };

        let answers = answers
            .iter()
            .map(|answer| answer.clone().into())
            .collect::<Vec<_>>();

        let receiver = match &prompt_builder {
            PromptBuilder::Default => self
                .platform_window
                .prompt(level, message, detail, &answers)
                .unwrap_or_else(|| {
                    self.build_custom_prompt(&prompt_builder, level, message, detail, &answers, cx)
                }),
            PromptBuilder::Custom(_) => {
                self.build_custom_prompt(&prompt_builder, level, message, detail, &answers, cx)
            }
        };

        cx.prompt_builder = Some(prompt_builder);

        receiver
    }

    fn build_custom_prompt(
        &mut self,
        prompt_builder: &PromptBuilder,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
        cx: &mut App,
    ) -> oneshot::Receiver<usize> {
        let (sender, receiver) = oneshot::channel();
        let handle = PromptHandle::new(sender);
        let handle = (prompt_builder)(level, message, detail, answers, handle, self, cx);
        self.prompt = Some(handle);
        receiver
    }

    /// Returns the current context stack.
    pub fn context_stack(&self) -> Vec<KeyContext> {
        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        dispatch_tree
            .dispatch_path(node_id)
            .iter()
            .filter_map(move |&node_id| dispatch_tree.node(node_id).context.clone())
            .collect()
    }

    /// Returns all available actions for the focused element.
    pub fn available_actions(&self, cx: &App) -> Vec<Box<dyn Action>> {
        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let mut actions = self.rendered_frame.dispatch_tree.available_actions(node_id);
        for action_type in cx.global_action_listeners.keys() {
            if let Err(ix) = actions.binary_search_by_key(action_type, |a| a.as_any().type_id()) {
                let action = cx.actions.build_action_type(action_type).ok();
                if let Some(action) = action {
                    actions.insert(ix, action);
                }
            }
        }
        actions
    }

    /// Returns key bindings that invoke an action on the currently focused element. Bindings are
    /// returned in the order they were added. For display, the last binding should take precedence.
    pub fn bindings_for_action(&self, action: &dyn Action) -> Vec<KeyBinding> {
        self.rendered_frame
            .dispatch_tree
            .bindings_for_action(action, &self.rendered_frame.dispatch_tree.context_stack)
    }

    /// Returns the highest precedence key binding that invokes an action on the currently focused
    /// element. This is more efficient than getting the last result of `bindings_for_action`.
    pub fn highest_precedence_binding_for_action(&self, action: &dyn Action) -> Option<KeyBinding> {
        self.rendered_frame
            .dispatch_tree
            .highest_precedence_binding_for_action(
                action,
                &self.rendered_frame.dispatch_tree.context_stack,
            )
    }

    /// Returns the key bindings for an action in a context.
    pub fn bindings_for_action_in_context(
        &self,
        action: &dyn Action,
        context: KeyContext,
    ) -> Vec<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        dispatch_tree.bindings_for_action(action, &[context])
    }

    /// Returns the highest precedence key binding for an action in a context. This is more
    /// efficient than getting the last result of `bindings_for_action_in_context`.
    pub fn highest_precedence_binding_for_action_in_context(
        &self,
        action: &dyn Action,
        context: KeyContext,
    ) -> Option<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        dispatch_tree.highest_precedence_binding_for_action(action, &[context])
    }

    /// Returns any bindings that would invoke an action on the given focus handle if it were
    /// focused. Bindings are returned in the order they were added. For display, the last binding
    /// should take precedence.
    pub fn bindings_for_action_in(
        &self,
        action: &dyn Action,
        focus_handle: &FocusHandle,
    ) -> Vec<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let Some(context_stack) = self.context_stack_for_focus_handle(focus_handle) else {
            return vec![];
        };
        dispatch_tree.bindings_for_action(action, &context_stack)
    }

    /// Returns the highest precedence key binding that would invoke an action on the given focus
    /// handle if it were focused. This is more efficient than getting the last result of
    /// `bindings_for_action_in`.
    pub fn highest_precedence_binding_for_action_in(
        &self,
        action: &dyn Action,
        focus_handle: &FocusHandle,
    ) -> Option<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let context_stack = self.context_stack_for_focus_handle(focus_handle)?;
        dispatch_tree.highest_precedence_binding_for_action(action, &context_stack)
    }

    /// Find the bindings that can follow the current input sequence for the current context stack.
    pub fn possible_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        self.rendered_frame
            .dispatch_tree
            .possible_next_bindings_for_input(input, &self.context_stack())
    }

    fn context_stack_for_focus_handle(
        &self,
        focus_handle: &FocusHandle,
    ) -> Option<Vec<KeyContext>> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let node_id = dispatch_tree.focusable_node_id(focus_handle.id)?;
        let context_stack: Vec<_> = dispatch_tree
            .dispatch_path(node_id)
            .into_iter()
            .filter_map(|node_id| dispatch_tree.node(node_id).context.clone())
            .collect();
        Some(context_stack)
    }

    /// Returns a generic event listener that invokes the given listener with the view and context associated with the given view handle.
    pub fn listener_for<T: 'static, E>(
        &self,
        view: &Entity<T>,
        f: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let view = view.downgrade();
        move |e: &E, window: &mut Window, cx: &mut App| {
            view.update(cx, |view, cx| f(view, e, window, cx)).ok();
        }
    }

    /// Returns a generic handler that invokes the given handler with the view and context associated with the given view handle.
    pub fn handler_for<E: 'static, Callback: Fn(&mut E, &mut Window, &mut Context<E>) + 'static>(
        &self,
        entity: &Entity<E>,
        f: Callback,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let entity = entity.downgrade();
        move |window: &mut Window, cx: &mut App| {
            entity.update(cx, |entity, cx| f(entity, window, cx)).ok();
        }
    }

    /// Register a callback that can interrupt the closing of the current window based the returned boolean.
    /// If the callback returns false, the window won't be closed.
    pub fn on_window_should_close(
        &mut self,
        _cx: &App,
        f: impl FnMut(&mut Window, &mut App) -> bool + 'static,
    ) {
        self.should_close_handler.set(Box::new(f));
    }

    /// Register an action listener on this node for the next frame. The type of action
    /// is determined by the first parameter of the given listener. When the next frame is rendered
    /// the listener will be cleared.
    ///
    /// This is a fairly low-level method, so prefer using action handlers on elements unless you have
    /// a specific need to register a listener yourself.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_action(
        &mut self,
        action_type: TypeId,
        listener: impl Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }

        self.next_frame
            .dispatch_tree
            .on_action(action_type, Rc::new(listener));
    }

    /// Register a capturing action listener on this node for the next frame if the condition is true.
    /// The type of action is determined by the first parameter of the given listener. When the next
    /// frame is rendered the listener will be cleared.
    ///
    /// This is a fairly low-level method, so prefer using action handlers on elements unless you have
    /// a specific need to register a listener yourself.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_action_when(
        &mut self,
        condition: bool,
        action_type: TypeId,
        listener: impl Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.subtree_presentation().is_interactive() {
            return;
        }

        if condition {
            self.next_frame
                .dispatch_tree
                .on_action(action_type, Rc::new(listener));
        }
    }

    /// Read information about the GPU backing this window.
    /// Currently returns None on Mac and Windows.
    pub fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.platform_window.gpu_specs()
    }

    /// Perform titlebar double-click action.
    /// This is macOS specific.
    pub fn titlebar_double_click(&self) {
        self.platform_window.titlebar_double_click();
    }

    /// Gets the window's title at the platform level.
    /// This is macOS specific.
    pub fn window_title(&self) -> String {
        self.platform_window.get_title()
    }

    /// Returns a list of all tabbed windows and their titles.
    /// This is macOS specific.
    pub fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.platform_window.tabbed_windows()
    }

    /// Returns the tab bar visibility.
    /// This is macOS specific.
    pub fn tab_bar_visible(&self) -> bool {
        self.platform_window.tab_bar_visible()
    }

    /// Merges all open windows into a single tabbed window.
    /// This is macOS specific.
    pub fn merge_all_windows(&self) {
        self.platform_window.merge_all_windows()
    }

    /// Moves the tab to a new containing window.
    /// This is macOS specific.
    pub fn move_tab_to_new_window(&self) {
        self.platform_window.move_tab_to_new_window()
    }

    /// Shows or hides the window tab overview.
    /// This is macOS specific.
    pub fn toggle_window_tab_overview(&self) {
        self.platform_window.toggle_window_tab_overview()
    }

    /// Sets the tabbing identifier for the window.
    /// This is macOS specific.
    pub fn set_tabbing_identifier(&self, tabbing_identifier: Option<String>) {
        self.platform_window
            .set_tabbing_identifier(tabbing_identifier)
    }

    /// Request the OS to play an alert sound. On some platforms this is associated
    /// with the window, for others it's just a simple global function call.
    pub fn play_system_bell(&self) {
        self.platform_window.play_system_bell()
    }

    /// Register a listener for an accessibility action on a specific node.
    /// The listener will be called when a screen reader requests the given
    /// action on the node identified by `node_id`.
    ///
    /// See the [accessibility guide](crate::_accessibility) for an overview.
    pub fn on_a11y_action(
        &mut self,
        node_id: accesskit::NodeId,
        action: accesskit::Action,
        listener: impl FnMut(Option<&accesskit::ActionData>, &mut Window, &mut App) + 'static,
    ) {
        if !self.subtree_presentation().is_interactive() {
            return;
        }
        self.a11y
            .record_action_listener(node_id, action, Box::new(listener));
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn handle_a11y_action(
        &mut self,
        request_activation_generation: u64,
        request: accesskit::ActionRequest,
        cx: &mut App,
    ) -> bool {
        if !self.provisional_session_accepts_interaction() {
            return false;
        }
        if !self.a11y.accepts_action(
            request_activation_generation,
            request.target_tree,
            request.target_node,
            request.action,
        ) {
            log::debug!(
                "Rejected a11y action {:?} on unavailable tree {:?} node {:?}",
                request.action,
                request.target_tree,
                request.target_node
            );
            return false;
        }

        // Take listeners out temporarily so the closures can borrow Window
        // mutably, then restore them afterward.
        if let Some((published_revision, mut listeners)) = self
            .a11y
            .take_published_action_listeners(request.target_node)
        {
            let extra_data = request.data.as_ref();
            let mut matched = false;
            for (action, listener) in &mut listeners {
                if *action == request.action {
                    listener(extra_data, self, cx);
                    matched = true;
                }
            }
            self.a11y.restore_published_action_listeners(
                published_revision,
                request.target_node,
                listeners,
            );
            if matched {
                return true;
            }
        }

        // Fall back to built-in action handling.
        match request.action {
            accesskit::Action::Click => {
                if let Some(position) = self.a11y.published_node_witness(request.target_node) {
                    let mouse_down = PlatformInput::MouseDown(crate::MouseDownEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    });
                    let mouse_up = PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    });
                    self.dispatch_event(mouse_down, cx);
                    if self.removal_state != WindowRemovalState::Open {
                        return true;
                    }
                    self.dispatch_event(mouse_up, cx);
                }
            }
            accesskit::Action::Focus => {
                if let Some(focus_id) = self.a11y.published_focus_id(request.target_node)
                    && let Some(handle) = FocusHandle::for_id(focus_id, &cx.focus_handles)
                {
                    self.focus(&handle, cx);
                }
            }
            accesskit::Action::Blur => {
                self.blur(cx);
            }
            accesskit::Action::ScrollIntoView => {
                let options = match request.data.as_ref() {
                    Some(accesskit::ActionData::ScrollHint(accesskit::ScrollHint::TopLeft)) => {
                        BringIntoViewOptions::aligned(BringIntoViewAlignment::MinEdge)
                    }
                    Some(accesskit::ActionData::ScrollHint(accesskit::ScrollHint::BottomRight)) => {
                        BringIntoViewOptions::aligned(BringIntoViewAlignment::MaxEdge)
                    }
                    Some(accesskit::ActionData::ScrollHint(accesskit::ScrollHint::TopEdge)) => {
                        BringIntoViewOptions::vertical(BringIntoViewAlignment::MinEdge)
                    }
                    Some(accesskit::ActionData::ScrollHint(accesskit::ScrollHint::BottomEdge)) => {
                        BringIntoViewOptions::vertical(BringIntoViewAlignment::MaxEdge)
                    }
                    Some(accesskit::ActionData::ScrollHint(accesskit::ScrollHint::LeftEdge)) => {
                        BringIntoViewOptions::horizontal(BringIntoViewAlignment::MinEdge)
                    }
                    Some(accesskit::ActionData::ScrollHint(accesskit::ScrollHint::RightEdge)) => {
                        BringIntoViewOptions::horizontal(BringIntoViewAlignment::MaxEdge)
                    }
                    _ => BringIntoViewOptions::nearest(),
                };
                self.enqueue_accessibility_bring_into_view(
                    request.target_node,
                    request_activation_generation,
                    options,
                    cx,
                );
            }
            _ => {
                log::debug!(
                    "Unhandled a11y action: {:?} on {:?}",
                    request.action,
                    request.target_node
                );
            }
        }
        true
    }

    /// Toggles the inspector mode on this window.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn toggle_inspector(&mut self, cx: &mut App) {
        self.inspector = match self.inspector {
            None => Some(cx.new(|_| Inspector::new())),
            Some(_) => None,
        };
        self.refresh();
    }

    /// Returns true if the window is in inspector mode.
    pub fn is_inspector_picking(&self, _cx: &App) -> bool {
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            if let Some(inspector) = &self.inspector {
                return inspector.read(_cx).is_picking();
            }
        }
        false
    }

    /// Returns the active Inspector element for test assertions.
    #[cfg(any(test, feature = "test-support"))]
    pub fn inspector_active_element_id_for_test(
        &self,
        _cx: &App,
    ) -> Option<crate::InspectorElementId> {
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            return self
                .inspector
                .as_ref()?
                .read(_cx)
                .active_element_id()
                .cloned();
        }
        #[cfg(not(any(feature = "inspector", debug_assertions)))]
        {
            None
        }
    }

    /// Executes the provided function with mutable access to an inspector state.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn with_inspector_state<T: 'static, R>(
        &mut self,
        _inspector_id: Option<&crate::InspectorElementId>,
        cx: &mut App,
        f: impl FnOnce(&mut Option<T>, &mut Self) -> R,
    ) -> R {
        if let Some(inspector_id) = _inspector_id
            && let Some(inspector) = &self.inspector
        {
            let inspector = inspector.clone();
            let active_element_id = inspector.read(cx).active_element_id();
            if Some(inspector_id) == active_element_id {
                return inspector.update(cx, |inspector, _cx| {
                    inspector.with_active_element_state(self, f)
                });
            }
        }
        f(&mut None, self)
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) fn build_inspector_element_id(
        &mut self,
        path: crate::InspectorElementPath,
    ) -> crate::InspectorElementId {
        self.invalidator.debug_assert_paint_or_prepaint();
        let path = Rc::new(path);
        let next_instance_id = self
            .next_frame
            .next_inspector_instance_ids
            .entry(path.clone())
            .or_insert(0);
        let instance_id = *next_instance_id;
        *next_instance_id += 1;
        crate::InspectorElementId { path, instance_id }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn prepaint_inspector(&mut self, inspector_width: Pixels, cx: &mut App) -> Option<AnyElement> {
        if let Some(inspector) = self.inspector.take() {
            let mut inspector_element = AnyView::from(inspector.clone()).into_any_element();
            inspector_element.prepaint_as_root(
                point(self.viewport_size.width - inspector_width, px(0.0)),
                size(inspector_width, self.viewport_size.height).into(),
                self,
                cx,
            );
            self.inspector = Some(inspector);
            Some(inspector_element)
        } else {
            None
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn paint_inspector(&mut self, mut inspector_element: Option<AnyElement>, cx: &mut App) {
        if let Some(mut inspector_element) = inspector_element {
            inspector_element.paint(self, cx);
        };
    }

    /// Registers a hitbox that can be used for inspector picking mode, allowing users to select and
    /// inspect UI elements by clicking on them.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn insert_inspector_hitbox(
        &mut self,
        hitbox_id: HitboxId,
        inspector_id: Option<&crate::InspectorElementId>,
        cx: &App,
    ) {
        self.invalidator.debug_assert_paint_or_prepaint();
        if !self.subtree_presentation().is_interactive() || !self.is_inspector_picking(cx) {
            return;
        }
        if let Some(inspector_id) = inspector_id {
            self.next_frame
                .inspector_hitboxes
                .insert(hitbox_id, inspector_id.clone());
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn paint_inspector_hitbox(&mut self, cx: &App) {
        if let Some(inspector) = self.inspector.as_ref() {
            let inspector = inspector.read(cx);
            if let Some((hitbox_id, _)) = self.hovered_inspector_hitbox(inspector, &self.next_frame)
                && let Some(hitbox) = self
                    .next_frame
                    .hitboxes
                    .iter()
                    .find(|hitbox| hitbox.id == hitbox_id && hitbox.is_active())
            {
                self.paint_quad(crate::fill(
                    hitbox.displayed_bounds(),
                    crate::rgba(0x61afef4d),
                ));
            }
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn handle_inspector_mouse_event(&mut self, event: &dyn Any, cx: &mut App) {
        let Some(inspector) = self.inspector.clone() else {
            return;
        };
        if event.downcast_ref::<MouseMoveEvent>().is_some() {
            inspector.update(cx, |inspector, _cx| {
                if let Some((_, inspector_id)) =
                    self.hovered_inspector_hitbox(inspector, &self.rendered_frame)
                {
                    inspector.hover(inspector_id, self);
                }
            });
        } else if event.downcast_ref::<crate::MouseDownEvent>().is_some() {
            inspector.update(cx, |inspector, _cx| {
                if let Some((_, inspector_id)) =
                    self.hovered_inspector_hitbox(inspector, &self.rendered_frame)
                {
                    inspector.select(inspector_id, self);
                }
            });
        } else if let Some(event) = event.downcast_ref::<crate::ScrollWheelEvent>() {
            // This should be kept in sync with SCROLL_LINES in x11 platform.
            const SCROLL_LINES: f32 = 3.0;
            const SCROLL_PIXELS_PER_LAYER: f32 = 36.0;
            let delta_y = event
                .delta
                .pixel_delta(px(SCROLL_PIXELS_PER_LAYER / SCROLL_LINES))
                .y;
            if let Some(inspector) = self.inspector.clone() {
                inspector.update(cx, |inspector, _cx| {
                    if let Some(depth) = inspector.pick_depth.as_mut() {
                        *depth += f32::from(delta_y) / SCROLL_PIXELS_PER_LAYER;
                        let max_depth = self.mouse_hit_test.ids.len() as f32 - 0.5;
                        if *depth < 0.0 {
                            *depth = 0.0;
                        } else if *depth > max_depth {
                            *depth = max_depth;
                        }
                        if let Some((_, inspector_id)) =
                            self.hovered_inspector_hitbox(inspector, &self.rendered_frame)
                        {
                            inspector.set_active_element_id(inspector_id, self);
                        }
                    }
                });
            }
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn hovered_inspector_hitbox(
        &self,
        inspector: &Inspector,
        frame: &Frame,
    ) -> Option<(HitboxId, crate::InspectorElementId)> {
        if let Some(pick_depth) = inspector.pick_depth {
            let depth = (pick_depth as i64).try_into().unwrap_or(0);
            let max_skipped = self.mouse_hit_test.ids.len().saturating_sub(1);
            let skip_count = (depth as usize).min(max_skipped);
            for hitbox_id in self.mouse_hit_test.ids.iter().skip(skip_count) {
                if let Some(inspector_id) = frame.inspector_hitboxes.get(hitbox_id) {
                    return Some((*hitbox_id, inspector_id.clone()));
                }
            }
        }
        None
    }

    /// For testing: set the current modifier keys state.
    /// This does not generate any events.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    /// For testing: simulate a mouse move event to the given position.
    /// This dispatches the event through the normal event handling path,
    /// which will trigger hover states and tooltips.
    #[cfg(any(test, feature = "test-support"))]
    pub fn simulate_mouse_move(&mut self, position: Point<Pixels>, cx: &mut App) {
        let event = PlatformInput::MouseMove(MouseMoveEvent {
            position,
            modifiers: self.modifiers,
            pressed_button: None,
        });
        let _ = self.dispatch_event(event, cx);
    }
}

// #[derive(Clone, Copy, Eq, PartialEq, Hash)]
slotmap::new_key_type! {
    /// A unique identifier for a window.
    pub struct WindowId;
}

impl WindowId {
    /// Converts this window ID to a `u64`.
    pub fn as_u64(&self) -> u64 {
        self.0.as_ffi()
    }
}

impl From<u64> for WindowId {
    fn from(value: u64) -> Self {
        WindowId(slotmap::KeyData::from_ffi(value))
    }
}

/// A handle to a window with a specific root view type.
/// Note that this does not keep the window alive on its own.
#[derive(Deref, DerefMut)]
pub struct WindowHandle<V> {
    #[deref]
    #[deref_mut]
    pub(crate) any_handle: AnyWindowHandle,
    state_type: PhantomData<fn(V) -> V>,
}

impl<V> Debug for WindowHandle<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowHandle")
            .field("any_handle", &self.any_handle.id.as_u64())
            .finish()
    }
}

impl<V: 'static + Render> WindowHandle<V> {
    /// Creates a new handle from a window ID.
    /// This does not check if the root type of the window is `V`.
    pub fn new(id: WindowId) -> Self {
        WindowHandle {
            any_handle: AnyWindowHandle {
                id,
                state_type: TypeId::of::<V>(),
            },
            state_type: PhantomData,
        }
    }

    /// Get the root view out of this window.
    ///
    /// This will fail if the window is closed or if the root view's type does not match `V`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn root<C>(&self, cx: &mut C) -> Result<Entity<V>>
    where
        C: AppContext,
    {
        cx.update_window(self.any_handle, |root_view, _, _| {
            root_view
                .downcast::<V>()
                .map_err(|_| anyhow!("the type of the window's root view has changed"))
        })?
    }

    /// Updates the root view of this window.
    ///
    /// This will fail if the window has been closed or if the root view's type does not match
    pub fn update<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> R,
    ) -> Result<R>
    where
        C: AppContext,
    {
        cx.update_window(self.any_handle, |root_view, window, cx| {
            let view = root_view
                .downcast::<V>()
                .map_err(|_| anyhow!("the type of the window's root view has changed"))?;

            Ok(view.update(cx, |view, cx| update(view, window, cx)))
        })?
    }

    /// Read the root view out of this window.
    ///
    /// This will fail if the window is closed or if the root view's type does not match `V`.
    pub fn read<'a>(&self, cx: &'a App) -> Result<&'a V> {
        let x = cx
            .windows
            .get(self.id)
            .and_then(|window| {
                window
                    .as_deref()
                    .and_then(|window| window.root.clone())
                    .map(|root_view| root_view.downcast::<V>())
            })
            .context("window not found")?
            .map_err(|_| anyhow!("the type of the window's root view has changed"))?;

        Ok(x.read(cx))
    }

    /// Read the root view out of this window, with a callback
    ///
    /// This will fail if the window is closed or if the root view's type does not match `V`.
    pub fn read_with<C, R>(&self, cx: &C, read_with: impl FnOnce(&V, &App) -> R) -> Result<R>
    where
        C: AppContext,
    {
        cx.read_window(self, |root_view, cx| read_with(root_view.read(cx), cx))
    }

    /// Read the root view pointer off of this window.
    ///
    /// This will fail if the window is closed or if the root view's type does not match `V`.
    pub fn entity<C>(&self, cx: &C) -> Result<Entity<V>>
    where
        C: AppContext,
    {
        cx.read_window(self, |root_view, _cx| root_view)
    }

    /// Check if this window is 'active'.
    ///
    /// Will return `None` if the window is closed or currently
    /// borrowed.
    pub fn is_active(&self, cx: &mut App) -> Option<bool> {
        cx.update_window(self.any_handle, |_, window, _| window.is_window_active())
            .ok()
    }
}

impl<V> Copy for WindowHandle<V> {}

impl<V> Clone for WindowHandle<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> PartialEq for WindowHandle<V> {
    fn eq(&self, other: &Self) -> bool {
        self.any_handle == other.any_handle
    }
}

impl<V> Eq for WindowHandle<V> {}

impl<V> Hash for WindowHandle<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.any_handle.hash(state);
    }
}

impl<V: 'static> From<WindowHandle<V>> for AnyWindowHandle {
    fn from(val: WindowHandle<V>) -> Self {
        val.any_handle
    }
}

/// A handle to a window with any root view type, which can be downcast to a window with a specific root view type.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AnyWindowHandle {
    pub(crate) id: WindowId,
    state_type: TypeId,
}

impl AnyWindowHandle {
    /// Get the ID of this window.
    pub fn window_id(&self) -> WindowId {
        self.id
    }

    /// Attempt to convert this handle to a window handle with a specific root view type.
    /// If the types do not match, this will return `None`.
    pub fn downcast<T: 'static>(&self) -> Option<WindowHandle<T>> {
        if TypeId::of::<T>() == self.state_type {
            Some(WindowHandle {
                any_handle: *self,
                state_type: PhantomData,
            })
        } else {
            None
        }
    }

    /// Updates the state of the root view of this window.
    ///
    /// This will fail if the window has been closed.
    pub fn update<C, R>(
        self,
        cx: &mut C,
        update: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R>
    where
        C: AppContext,
    {
        cx.update_window(self, update)
    }

    /// Read the state of the root view of this window.
    ///
    /// This will fail if the window has been closed.
    pub fn read<T, C, R>(self, cx: &C, read: impl FnOnce(Entity<T>, &App) -> R) -> Result<R>
    where
        C: AppContext,
        T: 'static,
    {
        let view = self
            .downcast::<T>()
            .context("the type of the window's root view has changed")?;

        cx.read_window(&view, read)
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        self.platform_window.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        self.platform_window.display_handle()
    }
}

/// An identifier for an [`Element`].
///
/// Can be constructed with a string, a number, or both, as well
/// as other internal representations.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ElementId {
    /// The ID of a View element
    View(EntityId),
    /// An integer ID.
    Integer(u64),
    /// A string based ID.
    Name(SharedString),
    /// A UUID.
    Uuid(Uuid),
    /// An ID that's equated with a focus handle.
    FocusHandle(FocusId),
    /// A combination of a name and an integer.
    NamedInteger(SharedString, u64),
    /// A path.
    Path(Arc<std::path::Path>),
    /// A code location.
    CodeLocation(core::panic::Location<'static>),
    /// A labeled child of an element.
    NamedChild(Arc<ElementId>, SharedString),
    /// A byte array ID (used for text-anchors)
    OpaqueId([u8; 20]),
}

impl ElementId {
    /// Constructs an `ElementId::NamedInteger` from a name and `usize`.
    pub fn named_usize(name: impl Into<SharedString>, integer: usize) -> ElementId {
        Self::NamedInteger(name.into(), integer as u64)
    }
}

impl Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElementId::View(entity_id) => write!(f, "view-{}", entity_id)?,
            ElementId::Integer(ix) => write!(f, "{}", ix)?,
            ElementId::Name(name) => write!(f, "{}", name)?,
            ElementId::FocusHandle(_) => write!(f, "FocusHandle")?,
            ElementId::NamedInteger(s, i) => write!(f, "{}-{}", s, i)?,
            ElementId::Uuid(uuid) => write!(f, "{}", uuid)?,
            ElementId::Path(path) => write!(f, "{}", path.display())?,
            ElementId::CodeLocation(location) => write!(f, "{}", location)?,
            ElementId::NamedChild(id, name) => write!(f, "{}-{}", id, name)?,
            ElementId::OpaqueId(opaque_id) => write!(f, "{:x?}", opaque_id)?,
        }

        Ok(())
    }
}

impl TryInto<SharedString> for ElementId {
    type Error = anyhow::Error;

    fn try_into(self) -> anyhow::Result<SharedString> {
        if let ElementId::Name(name) = self {
            Ok(name)
        } else {
            anyhow::bail!("element id is not string")
        }
    }
}

impl From<usize> for ElementId {
    fn from(id: usize) -> Self {
        ElementId::Integer(id as u64)
    }
}

impl From<i32> for ElementId {
    fn from(id: i32) -> Self {
        Self::Integer(id as u64)
    }
}

impl From<SharedString> for ElementId {
    fn from(name: SharedString) -> Self {
        ElementId::Name(name)
    }
}

impl From<String> for ElementId {
    fn from(name: String) -> Self {
        ElementId::Name(name.into())
    }
}

impl From<Arc<str>> for ElementId {
    fn from(name: Arc<str>) -> Self {
        ElementId::Name(name.into())
    }
}

impl From<Arc<std::path::Path>> for ElementId {
    fn from(path: Arc<std::path::Path>) -> Self {
        ElementId::Path(path)
    }
}

impl From<&'static str> for ElementId {
    fn from(name: &'static str) -> Self {
        ElementId::Name(SharedString::new_static(name))
    }
}

impl<'a> From<&'a FocusHandle> for ElementId {
    fn from(handle: &'a FocusHandle) -> Self {
        ElementId::FocusHandle(handle.id)
    }
}

impl From<(&'static str, EntityId)> for ElementId {
    fn from((name, id): (&'static str, EntityId)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), id.as_u64())
    }
}

impl From<(&'static str, usize)> for ElementId {
    fn from((name, id): (&'static str, usize)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), id as u64)
    }
}

impl From<(SharedString, usize)> for ElementId {
    fn from((name, id): (SharedString, usize)) -> Self {
        ElementId::NamedInteger(name, id as u64)
    }
}

impl From<(&'static str, u64)> for ElementId {
    fn from((name, id): (&'static str, u64)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), id)
    }
}

impl From<Uuid> for ElementId {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

impl From<(&'static str, u32)> for ElementId {
    fn from((name, id): (&'static str, u32)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), u64::from(id))
    }
}

impl<T: Into<SharedString>> From<(ElementId, T)> for ElementId {
    fn from((id, name): (ElementId, T)) -> Self {
        ElementId::NamedChild(Arc::new(id), name.into())
    }
}

impl From<&'static core::panic::Location<'static>> for ElementId {
    fn from(location: &'static core::panic::Location<'static>) -> Self {
        ElementId::CodeLocation(*location)
    }
}

impl From<[u8; 20]> for ElementId {
    fn from(opaque_id: [u8; 20]) -> Self {
        ElementId::OpaqueId(opaque_id)
    }
}

/// A rectangle to be rendered in the window at the given position and size.
/// Passed as an argument [`Window::paint_quad`].
#[derive(Clone)]
pub struct PaintQuad {
    /// The bounds of the quad within the window.
    pub bounds: Bounds<Pixels>,
    /// The radii of the quad's corners.
    pub corner_radii: Corners<Pixels>,
    /// The background color of the quad.
    pub background: Background,
    /// The widths of the quad's borders.
    pub border_widths: Edges<Pixels>,
    /// The color of the quad's borders.
    pub border_color: Hsla,
    /// The style of the quad's borders.
    pub border_style: BorderStyle,
}

impl PaintQuad {
    /// Sets the corner radii of the quad.
    pub fn corner_radii(self, corner_radii: impl Into<Corners<Pixels>>) -> Self {
        PaintQuad {
            corner_radii: corner_radii.into(),
            ..self
        }
    }

    /// Sets the border widths of the quad.
    pub fn border_widths(self, border_widths: impl Into<Edges<Pixels>>) -> Self {
        PaintQuad {
            border_widths: border_widths.into(),
            ..self
        }
    }

    /// Sets the border color of the quad.
    pub fn border_color(self, border_color: impl Into<Hsla>) -> Self {
        PaintQuad {
            border_color: border_color.into(),
            ..self
        }
    }

    /// Sets the background color of the quad.
    pub fn background(self, background: impl Into<Background>) -> Self {
        PaintQuad {
            background: background.into(),
            ..self
        }
    }
}

/// Creates a quad with the given parameters.
pub fn quad(
    bounds: Bounds<Pixels>,
    corner_radii: impl Into<Corners<Pixels>>,
    background: impl Into<Background>,
    border_widths: impl Into<Edges<Pixels>>,
    border_color: impl Into<Hsla>,
    border_style: BorderStyle,
) -> PaintQuad {
    PaintQuad {
        bounds,
        corner_radii: corner_radii.into(),
        background: background.into(),
        border_widths: border_widths.into(),
        border_color: border_color.into(),
        border_style,
    }
}

/// Creates a filled quad with the given bounds and background color.
pub fn fill(bounds: impl Into<Bounds<Pixels>>, background: impl Into<Background>) -> PaintQuad {
    PaintQuad {
        bounds: bounds.into(),
        corner_radii: (0.).into(),
        background: background.into(),
        border_widths: (0.).into(),
        border_color: transparent_black(),
        border_style: BorderStyle::default(),
    }
}

/// Creates a rectangle outline with the given bounds, border color, and a 1px border width
pub fn outline(
    bounds: impl Into<Bounds<Pixels>>,
    border_color: impl Into<Hsla>,
    border_style: BorderStyle,
) -> PaintQuad {
    PaintQuad {
        bounds: bounds.into(),
        corner_radii: (0.).into(),
        background: transparent_black().into(),
        border_widths: (1.).into(),
        border_color: border_color.into(),
        border_style,
    }
}

#[cfg(test)]
mod cached_paint_atlas_tests {
    use super::*;
    use crate::{
        AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTextureLeaseEpoch, AtlasTile, Empty,
        ImageSource, PlatformWindowCommandOutcome, StyleRefinement, TestAppContext, TileId, canvas,
        div, img, red,
        retained_visual::{
            self, Invalidation as RetainedVisualInvalidation,
            ReplayReceipt as RetainedReplayReceipt, SourceId as RetainedVisualSourceId,
            Ticket as RetainedVisualTicket,
        },
    };
    use image::{Frame, ImageBuffer, Rgba};
    use parking_lot::Mutex;

    struct EpochAtlas(Mutex<AtlasTextureLeaseEpoch>);

    impl EpochAtlas {
        fn new() -> Self {
            Self(Mutex::new(AtlasTextureLeaseEpoch::INITIAL))
        }

        fn reset(&self) {
            let mut epoch = self.0.lock();
            *epoch = epoch.next();
        }
    }

    impl PlatformAtlas for EpochAtlas {
        fn get_or_insert_with<'a>(
            &self,
            _key: &AtlasKey,
            _build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
        ) -> Result<Option<crate::AtlasTile>> {
            Ok(None)
        }

        fn remove(&self, _key: &AtlasKey) {}

        fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
            *self.0.lock()
        }

        unsafe fn acquire_atlas_texture_leases(
            &self,
            _textures: &[AtlasTextureInstanceId],
        ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
            Ok(*self.0.lock())
        }

        unsafe fn release_atlas_texture_leases(
            &self,
            _epoch: AtlasTextureLeaseEpoch,
            _textures: &[AtlasTextureInstanceId],
        ) {
        }
    }

    struct CachedImageChild {
        image: Arc<RenderImage>,
        renders: Rc<Cell<usize>>,
    }

    impl Render for CachedImageChild {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            img(ImageSource::Render(self.image.clone()))
        }
    }

    struct CachedImageRoot {
        child: Entity<CachedImageChild>,
    }

    impl Render for CachedImageRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full())
        }
    }

    struct RepeatedAtlasPrimitiveChild {
        tile: AtlasTile,
        primitive_count: u32,
    }

    impl Render for RepeatedAtlasPrimitiveChild {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let tile = self.tile;
            let primitive_count = self.primitive_count;
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let bounds = window.device_local_bounds(bounds);
                    for order in 0..primitive_count {
                        assert!(window.insert_scene_primitive(MonochromeSprite {
                            order,
                            pad: 0,
                            bounds,
                            clip: Default::default(),
                            color: red(),
                            tile,
                            transform: PrimitiveTransform::IDENTITY,
                        }));
                    }
                },
            )
            .size_full()
        }
    }

    struct RepeatedAtlasPrimitiveRoot {
        child: Entity<RepeatedAtlasPrimitiveChild>,
    }

    impl Render for RepeatedAtlasPrimitiveRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full())
        }
    }

    #[derive(Clone, Copy)]
    enum RetainedReplayAtlasMode {
        Source,
        Replay(RetainedVisualTicket),
    }

    struct RetainedReplayAtlasRoot {
        mode: RetainedReplayAtlasMode,
        source_id: RetainedVisualSourceId,
        source_tile: AtlasTile,
        candidate_tile: AtlasTile,
        replay_outcomes: Rc<
            RefCell<Vec<std::result::Result<RetainedReplayReceipt, RetainedVisualInvalidation>>>,
        >,
        prior_receipt_matches_candidate: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for RetainedReplayAtlasRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            match self.mode {
                RetainedReplayAtlasMode::Source => {
                    let tile = self.source_tile;
                    retained_visual::source(
                        self.source_id.clone(),
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let bounds = window.device_local_bounds(bounds);
                                assert!(window.insert_scene_primitive(MonochromeSprite {
                                    order: 0,
                                    pad: 0,
                                    bounds,
                                    clip: Default::default(),
                                    color: red(),
                                    tile,
                                    transform: PrimitiveTransform::IDENTITY,
                                }));
                            },
                        )
                        .size_full(),
                    )
                    .into_any_element()
                }
                RetainedReplayAtlasMode::Replay(ticket) => {
                    let candidate_tile = self.candidate_tile;
                    let replay_outcomes = self.replay_outcomes.clone();
                    let prior_receipt_matches_candidate =
                        self.prior_receipt_matches_candidate.clone();
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let prior_receipt = replay_outcomes
                                .borrow()
                                .iter()
                                .rev()
                                .find_map(|outcome| outcome.as_ref().ok().copied());
                            if let Some(prior_receipt) = prior_receipt {
                                prior_receipt_matches_candidate
                                    .borrow_mut()
                                    .push(prior_receipt.matches_candidate(window));
                            }
                            replay_outcomes
                                .borrow_mut()
                                .push(retained_visual::replay(window, &ticket));
                            let bounds = window.device_local_bounds(bounds);
                            let _ = window.insert_scene_primitive(PolychromeSprite {
                                order: 1,
                                pad: 0,
                                grayscale: false,
                                opacity: 1.0,
                                bounds,
                                clip: Default::default(),
                                corner_radii: Default::default(),
                                tile: candidate_tile,
                                transform: PrimitiveTransform::IDENTITY,
                            });
                        },
                    )
                    .size_full()
                    .into_any_element()
                }
            }
        }
    }

    struct CachedRetainedReplayChild {
        ticket: RetainedVisualTicket,
        renders: Rc<Cell<usize>>,
        outcomes: Rc<
            RefCell<Vec<std::result::Result<RetainedReplayReceipt, RetainedVisualInvalidation>>>,
        >,
    }

    impl Render for CachedRetainedReplayChild {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let ticket = self.ticket;
            let outcomes = self.outcomes.clone();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    outcomes
                        .borrow_mut()
                        .push(retained_visual::replay(window, &ticket));
                },
            )
            .size_full()
        }
    }

    struct CachedRetainedReplayRoot {
        source_id: RetainedVisualSourceId,
        source_tile: AtlasTile,
        cached_child: Option<Entity<CachedRetainedReplayChild>>,
        direct_ticket: Option<RetainedVisualTicket>,
        direct_outcomes: Rc<
            RefCell<Vec<std::result::Result<RetainedReplayReceipt, RetainedVisualInvalidation>>>,
        >,
    }

    impl Render for CachedRetainedReplayRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let Some(cached_child) = self.cached_child.as_ref() else {
                let tile = self.source_tile;
                return retained_visual::source(
                    self.source_id.clone(),
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let bounds = window.device_local_bounds(bounds);
                            assert!(window.insert_scene_primitive(MonochromeSprite {
                                order: 0,
                                pad: 0,
                                bounds,
                                clip: Default::default(),
                                color: red(),
                                tile,
                                transform: PrimitiveTransform::IDENTITY,
                            }));
                        },
                    )
                    .size_full(),
                )
                .into_any_element();
            };

            let direct_ticket = self.direct_ticket;
            let direct_outcomes = self.direct_outcomes.clone();
            div()
                .size_full()
                .child(
                    AnyView::from(cached_child.clone())
                        .cached(StyleRefinement::default().size_full()),
                )
                .when_some(direct_ticket, |this, ticket| {
                    this.child(
                        canvas(
                            |_, _, _| {},
                            move |_, _, window, _| {
                                direct_outcomes
                                    .borrow_mut()
                                    .push(retained_visual::replay(window, &ticket));
                            },
                        )
                        .size_full(),
                    )
                })
                .into_any_element()
        }
    }

    struct CandidateFocusCompletionRoot {
        focus: FocusHandle,
        tile: AtlasTile,
        remaining_requests: Rc<Cell<usize>>,
        next_request_id: Rc<Cell<usize>>,
        outcomes: Rc<RefCell<Vec<(usize, FocusClaimOutcome)>>>,
        subscriptions: Rc<RefCell<Vec<Subscription>>>,
    }

    impl Render for CandidateFocusCompletionRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let focus = self.focus.clone();
            let completion_focus = focus.clone();
            let remaining_requests = self.remaining_requests.clone();
            let next_request_id = self.next_request_id.clone();
            let outcomes = self.outcomes.clone();
            let subscriptions = self.subscriptions.clone();
            let tile = self.tile;

            div()
                .child(
                    div()
                        .id("candidate-focus-completion-target")
                        .focusable()
                        .track_focus(&focus),
                )
                .child(
                    canvas(
                        move |_, window, cx| {
                            let remaining = remaining_requests.get();
                            if remaining == 0 {
                                return;
                            }
                            remaining_requests.set(remaining - 1);
                            let request_id = next_request_id.get() + 1;
                            next_request_id.set(request_id);
                            let outcomes = outcomes.clone();
                            let subscription = window.focus_with_completion(
                                &completion_focus,
                                cx,
                                move |outcome, _, _| {
                                    outcomes.borrow_mut().push((request_id, outcome));
                                },
                            );
                            subscriptions.borrow_mut().push(subscription);
                        },
                        move |bounds, _, window, _| {
                            let bounds = window.device_local_bounds(bounds);
                            let _ = window.insert_scene_primitive(MonochromeSprite {
                                order: 0,
                                pad: 0,
                                bounds,
                                clip: Default::default(),
                                color: red(),
                                tile,
                                transform: PrimitiveTransform::IDENTITY,
                            });
                        },
                    )
                    .size_full(),
                )
        }
    }

    struct RejectOnceAtlasState {
        fail_next_lease: bool,
        lease_attempts: usize,
    }

    struct RejectOnceAtlas(Mutex<RejectOnceAtlasState>);

    impl RejectOnceAtlas {
        fn new() -> Self {
            Self(Mutex::new(RejectOnceAtlasState {
                fail_next_lease: false,
                lease_attempts: 0,
            }))
        }

        fn fail_next_lease(&self) {
            self.0.lock().fail_next_lease = true;
        }

        fn lease_attempts(&self) -> usize {
            self.0.lock().lease_attempts
        }

        fn tile(kind: AtlasTextureKind) -> AtlasTile {
            AtlasTile {
                texture_id: AtlasTextureId { index: 1, kind },
                tile_id: TileId(1),
                padding: 0,
                bounds: Bounds::new(Point::default(), size(DevicePixels(1), DevicePixels(1))),
                texture_generation: 1,
                texture_generation_padding: 0,
            }
        }
    }

    impl PlatformAtlas for RejectOnceAtlas {
        fn get_or_insert_with<'a>(
            &self,
            key: &AtlasKey,
            _build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
        ) -> Result<Option<AtlasTile>> {
            Ok(Some(Self::tile(key.texture_kind())))
        }

        fn remove(&self, _key: &AtlasKey) {}

        fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
            AtlasTextureLeaseEpoch::INITIAL
        }

        unsafe fn acquire_atlas_texture_leases(
            &self,
            textures: &[AtlasTextureInstanceId],
        ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
            let mut state = self.0.lock();
            state.lease_attempts += 1;
            if state.fail_next_lease {
                state.fail_next_lease = false;
                return Err(AtlasTextureLeaseError::TextureUnavailable {
                    texture: *textures
                        .first()
                        .expect("a sprite lease must name one texture instance"),
                    epoch: AtlasTextureLeaseEpoch::INITIAL,
                });
            }
            Ok(AtlasTextureLeaseEpoch::INITIAL)
        }

        unsafe fn release_atlas_texture_leases(
            &self,
            _epoch: AtlasTextureLeaseEpoch,
            _textures: &[AtlasTextureInstanceId],
        ) {
        }
    }

    struct AlwaysRejectAtlas {
        lease_attempts: Mutex<usize>,
    }

    impl AlwaysRejectAtlas {
        fn new() -> Self {
            Self {
                lease_attempts: Mutex::new(0),
            }
        }

        fn lease_attempts(&self) -> usize {
            *self.lease_attempts.lock()
        }
    }

    impl PlatformAtlas for AlwaysRejectAtlas {
        fn get_or_insert_with<'a>(
            &self,
            key: &AtlasKey,
            _build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
        ) -> Result<Option<AtlasTile>> {
            Ok(Some(RejectOnceAtlas::tile(key.texture_kind())))
        }

        fn remove(&self, _key: &AtlasKey) {}

        fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
            AtlasTextureLeaseEpoch::INITIAL
        }

        unsafe fn acquire_atlas_texture_leases(
            &self,
            textures: &[AtlasTextureInstanceId],
        ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
            *self.lease_attempts.lock() += 1;
            Err(AtlasTextureLeaseError::TextureUnavailable {
                texture: *textures
                    .first()
                    .expect("an image lease must name one texture instance"),
                epoch: AtlasTextureLeaseEpoch::INITIAL,
            })
        }

        unsafe fn release_atlas_texture_leases(
            &self,
            _epoch: AtlasTextureLeaseEpoch,
            _textures: &[AtlasTextureInstanceId],
        ) {
        }
    }

    struct InitialAtlasImageRoot {
        image: Arc<RenderImage>,
        _initial_presentation_subscription: Option<Subscription>,
    }

    impl InitialAtlasImageRoot {
        fn new(
            image: Arc<RenderImage>,
            window: &mut Window,
            observations: Option<Rc<RefCell<Vec<WindowInitialPresentationStatus>>>>,
            cx: &mut Context<Self>,
        ) -> Self {
            let subscription = observations.map(|observations| {
                cx.observe_window_initial_presentation(window, move |_, window, _| {
                    observations
                        .borrow_mut()
                        .push(window.presentation_facts().initial_presentation);
                })
            });
            Self {
                image,
                _initial_presentation_subscription: subscription,
            }
        }
    }

    impl Render for InitialAtlasImageRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            img(ImageSource::Render(self.image.clone())).size_full()
        }
    }

    struct RendererRepaintRoot;

    impl Render for RendererRepaintRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(crate::white())
        }
    }

    struct EmptyRendererRepaintRoot;

    impl Render for EmptyRendererRepaintRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    struct CachedListenerChild {
        renders: Rc<Cell<usize>>,
    }

    impl Render for CachedListenerChild {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            div()
                .id("atlas-frame-rejection-listener")
                .size_full()
                .on_mouse_down(MouseButton::Left, |_, _, _| {})
        }
    }

    #[derive(Clone)]
    enum CandidateAtlasPrimitive {
        Image(Arc<RenderImage>),
        Monochrome(AtlasTile),
    }

    struct AtlasFrameRejectionRoot {
        cached_child: Entity<CachedListenerChild>,
        enabled: Rc<Cell<bool>>,
        primitive: CandidateAtlasPrimitive,
        commits: Rc<Cell<usize>>,
        publication: PrepaintPublicationId,
        publication_callbacks: Rc<RefCell<Vec<(bool, u64, u64)>>>,
    }

    impl Render for AtlasFrameRejectionRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let enabled = self.enabled.get();
            let primitive = self.primitive.clone();
            let commits = self.commits.clone();
            let publication = self.publication;
            let publication_commits = self.publication_callbacks.clone();
            let publication_discards = self.publication_callbacks.clone();
            div()
                .relative()
                .w(px(128.0))
                .h(px(96.0))
                .child(
                    AnyView::from(self.cached_child.clone())
                        .cached(StyleRefinement::default().size_full()),
                )
                .child(
                    canvas(
                        move |_, window, _| {
                            window.record_prepaint_commit(move |_, _| {
                                commits.set(commits.get() + 1);
                            });
                            window.record_prepaint_window_transaction(
                                publication,
                                move |fence, window, _| {
                                    assert!(fence.is_satisfied_by(window));
                                    publication_commits.borrow_mut().push((
                                        true,
                                        fence.generation(),
                                        window.rendered_frame_revision(),
                                    ));
                                },
                                move |fence, window, _| {
                                    assert!(fence.is_satisfied_by(window));
                                    publication_discards.borrow_mut().push((
                                        false,
                                        fence.generation(),
                                        window.rendered_frame_revision(),
                                    ));
                                },
                            );
                        },
                        move |bounds, _, window, _| {
                            window.paint_quad(fill(bounds, red()));
                            if !enabled {
                                return;
                            }
                            match primitive {
                                CandidateAtlasPrimitive::Image(image) => window
                                    .paint_image(bounds, Corners::default(), image, 0, false)
                                    .expect(
                                        "test image paint should remain fallible only by asset",
                                    ),
                                CandidateAtlasPrimitive::Monochrome(tile) => {
                                    let bounds = window.device_local_bounds(bounds);
                                    window.insert_scene_primitive(MonochromeSprite {
                                        order: 0,
                                        pad: 0,
                                        bounds,
                                        clip: Default::default(),
                                        color: red(),
                                        tile,
                                        transform: PrimitiveTransform::IDENTITY,
                                    });
                                }
                            }
                        },
                    )
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(16.0))
                    .h(px(16.0)),
                )
        }
    }

    fn assert_fresh_atlas_failure_rejects_candidate(
        primitive: CandidateAtlasPrimitive,
        accessibility_active: bool,
        assert_recovered: impl Fn(&Window),
    ) {
        let expected_texture = match &primitive {
            CandidateAtlasPrimitive::Image(_) => {
                RejectOnceAtlas::tile(AtlasTextureKind::Polychrome).texture_instance()
            }
            CandidateAtlasPrimitive::Monochrome(tile) => tile.texture_instance(),
        };
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(RejectOnceAtlas::new());
        let enabled = Rc::new(Cell::new(false));
        let cached_renders = Rc::new(Cell::new(0));
        let commits = Rc::new(Cell::new(0));
        let publication_callbacks = Rc::new(RefCell::new(Vec::new()));
        let (root, cx) = test_app.add_window_view({
            let enabled = enabled.clone();
            let cached_renders = cached_renders.clone();
            let commits = commits.clone();
            let publication_callbacks = publication_callbacks.clone();
            move |_, cx| AtlasFrameRejectionRoot {
                cached_child: cx.new(|_| CachedListenerChild {
                    renders: cached_renders,
                }),
                enabled,
                primitive,
                commits,
                publication: PrepaintPublicationId::new(),
                publication_callbacks,
            }
        });
        cx.update({
            let atlas = atlas.clone();
            move |window, _| {
                window.sprite_atlas = atlas;
                if accessibility_active {
                    window.set_accessibility_active_for_test(true);
                }
            }
        });
        cx.simulate_resize(size(px(128.0), px(96.0)));

        cx.update(|window, cx| window.draw(cx).clear());
        let (
            committed_generation,
            committed_scene_len,
            committed_listener_count,
            committed_image_diagnostics,
            committed_a11y_revision,
        ) = cx.update(|window, _| {
            (
                window.rendered_frame.generation,
                window.rendered_frame.scene.len(),
                window
                    .rendered_frame
                    .mouse_listeners
                    .iter()
                    .filter(|listener| listener.value.is_some())
                    .count(),
                window.rendered_frame.image_paint_diagnostics.len(),
                window.a11y.published_revision_for_test(),
            )
        });
        let committed_renders = cached_renders.get();
        let committed_commits = commits.get();
        let committed_publication_callbacks = publication_callbacks.borrow().clone();
        assert!(!committed_publication_callbacks.is_empty());
        assert!(
            committed_publication_callbacks
                .iter()
                .all(|(committed, fence, rendered)| *committed && fence == rendered),
            "every baseline callback should carry its exact accepted rendered generation"
        );
        assert_eq!(
            committed_publication_callbacks.last(),
            Some(&(true, committed_generation, committed_generation)),
            "the latest baseline publication should describe the committed frame"
        );
        enabled.set(true);

        cx.update(|window, cx| {
            window.mark_view_dirty(root.entity_id());
            atlas.fail_next_lease();
            window.draw(cx).clear();
            assert_eq!(window.rendered_frame.generation, committed_generation);
            assert_eq!(window.rendered_frame.scene.len(), committed_scene_len);
            assert_eq!(
                window.rendered_frame.image_paint_diagnostics.len(),
                committed_image_diagnostics,
                "a rejected image candidate must not publish success diagnostics"
            );
            assert_eq!(
                window
                    .rendered_frame
                    .mouse_listeners
                    .iter()
                    .filter(|listener| listener.value.is_some())
                    .count(),
                committed_listener_count,
                "candidate rollback must restore cached committed listeners"
            );
            assert_eq!(
                window.a11y.published_revision_for_test(),
                committed_a11y_revision,
                "a rejected candidate must not replace published accessibility authority"
            );
            assert!(window.refresh_pending_for_test());
            assert_eq!(
                window.last_atlas_frame_rejection,
                Some(AtlasFrameRejection {
                    generation: committed_generation + 1,
                    error: AtlasTextureLeaseError::TextureUnavailable {
                        texture: expected_texture,
                        epoch: AtlasTextureLeaseEpoch::INITIAL,
                    },
                })
            );
            if !accessibility_active {
                assert_eq!(cached_renders.get(), committed_renders);
            }
            assert_eq!(commits.get(), committed_commits);
            assert_eq!(
                publication_callbacks.borrow().as_slice(),
                committed_publication_callbacks.as_slice(),
                "an atlas-rejected candidate must emit no fence and run no publication callback"
            );
            assert_eq!(atlas.lease_attempts(), 1);

            window.draw(cx).clear();
            assert_eq!(atlas.lease_attempts(), 2);
            assert_eq!(window.rendered_frame.generation, committed_generation + 1);
            if accessibility_active {
                assert_eq!(
                    window.a11y.published_revision_for_test(),
                    Some(
                        committed_a11y_revision
                            .expect("the baseline accessibility frame must be published")
                            .wrapping_add(1)
                    ),
                    "the recovery frame must publish exactly one accessibility candidate"
                );
            }
            assert_recovered(window);
        });
        assert!(cached_renders.get() > committed_renders);
        assert_eq!(commits.get(), committed_commits + 1);
        let recovered_publication_callbacks = publication_callbacks.borrow();
        assert_eq!(
            recovered_publication_callbacks.len(),
            committed_publication_callbacks.len() + 1,
            "the accepted recovery frame should append exactly one fence"
        );
        assert_eq!(
            &recovered_publication_callbacks[..committed_publication_callbacks.len()],
            committed_publication_callbacks.as_slice(),
            "recovery must preserve the entire pre-rejection publication history"
        );
        assert_eq!(
            recovered_publication_callbacks.last(),
            Some(&(true, committed_generation + 1, committed_generation + 1,)),
            "only the accepted recovery frame may append a post-rejection fence"
        );
    }

    #[test]
    fn fresh_image_lease_failure_rejects_the_candidate_until_repaint_recovers() {
        let image = Arc::new(RenderImage::new(smallvec::smallvec![Frame::new(
            ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0xff])),
        )]));
        assert_fresh_atlas_failure_rejects_candidate(
            CandidateAtlasPrimitive::Image(image),
            false,
            |window| {
                assert_eq!(window.rendered_frame.scene.polychrome_sprites.len(), 1);
                assert_eq!(window.rendered_frame.image_paint_diagnostics.len(), 1);
            },
        );
    }

    #[test]
    fn fresh_monochrome_lease_failure_rejects_the_candidate_until_repaint_recovers() {
        assert_fresh_atlas_failure_rejects_candidate(
            CandidateAtlasPrimitive::Monochrome(RejectOnceAtlas::tile(
                AtlasTextureKind::Monochrome,
            )),
            true,
            |window| assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1),
        );
    }

    #[test]
    fn retained_visual_replay_is_retryable_after_atlas_rejected_candidate() {
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(RejectOnceAtlas::new());
        let source_id = RetainedVisualSourceId::new("atlas-retry-retained-visual");
        let replay_outcomes = Rc::new(RefCell::new(Vec::new()));
        let prior_receipt_matches_candidate = Rc::new(RefCell::new(Vec::new()));
        let (root, cx) = test_app.add_window_view({
            let atlas = atlas.clone();
            let source_id = source_id.clone();
            let replay_outcomes = replay_outcomes.clone();
            let prior_receipt_matches_candidate = prior_receipt_matches_candidate.clone();
            move |window, _| {
                window.sprite_atlas = atlas.clone();
                RetainedReplayAtlasRoot {
                    mode: RetainedReplayAtlasMode::Source,
                    source_id,
                    source_tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                    candidate_tile: RejectOnceAtlas::tile(AtlasTextureKind::Polychrome),
                    replay_outcomes,
                    prior_receipt_matches_candidate,
                }
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        let (ticket, committed_generation) = cx.update(|window, _| {
            (
                retained_visual::lease_committed(window, &source_id)
                    .expect("the committed source should be leasable"),
                window.rendered_frame.generation,
            )
        });
        atlas.fail_next_lease();
        root.update(cx, |root, cx| {
            root.mode = RetainedReplayAtlasMode::Replay(ticket);
            cx.notify();
        });

        cx.update(|window, cx| {
            assert_eq!(
                window.rendered_frame.generation, committed_generation,
                "the atlas failure must reject the candidate after retained replay succeeds"
            );

            window.draw(cx).clear();
            assert_eq!(
                window.rendered_frame.generation,
                committed_generation + 1,
                "the next candidate must be able to replay the same ticket and commit"
            );
        });

        let outcomes = replay_outcomes.borrow();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0].is_ok());
        assert!(
            outcomes[1].is_ok(),
            "a rejected attempt must not permanently consume replay identity: {:?}",
            outcomes[1]
        );
        assert_eq!(
            outcomes[0]
                .as_ref()
                .expect("the rejected candidate replay should have succeeded")
                .replay_frame_generation(),
            committed_generation + 1
        );
        assert_eq!(
            outcomes[1]
                .as_ref()
                .expect("the recovery candidate replay should succeed")
                .replay_frame_generation(),
            committed_generation + 1,
            "frame generation may repeat, but candidate attempt identity must not"
        );
        assert_ne!(
            outcomes[0]
                .as_ref()
                .expect("the rejected candidate replay should have succeeded"),
            outcomes[1]
                .as_ref()
                .expect("the recovery candidate replay should succeed"),
            "same-generation replay receipts must retain exact candidate-attempt identity"
        );
        assert_eq!(
            prior_receipt_matches_candidate.borrow().as_slice(),
            &[false],
            "a receipt leaked from the rejected attempt must not validate in the retry"
        );
    }

    #[test]
    fn cached_retained_replay_reserves_identity_before_direct_sibling_replay() {
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(RejectOnceAtlas::new());
        let source_id = RetainedVisualSourceId::new("cached-retained-replay-identity");
        let cached_renders = Rc::new(Cell::new(0));
        let cached_outcomes = Rc::new(RefCell::new(Vec::new()));
        let direct_outcomes = Rc::new(RefCell::new(Vec::new()));
        let (root, cx) = test_app.add_window_view({
            let atlas = atlas.clone();
            let source_id = source_id.clone();
            let direct_outcomes = direct_outcomes.clone();
            move |window, _| {
                window.sprite_atlas = atlas;
                CachedRetainedReplayRoot {
                    source_id,
                    source_tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                    cached_child: None,
                    direct_ticket: None,
                    direct_outcomes,
                }
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        let ticket = cx.update(|window, _| {
            retained_visual::lease_committed(window, &source_id)
                .expect("the committed cached source should be leasable")
        });
        root.update(cx, |root, cx| {
            root.cached_child = Some(cx.new(|_| CachedRetainedReplayChild {
                ticket,
                renders: cached_renders.clone(),
                outcomes: cached_outcomes.clone(),
            }));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(cached_renders.get(), 1);
        assert!(
            cached_outcomes.borrow().first().is_some_and(Result::is_ok),
            "the initial cached child replay should commit"
        );

        cx.update(|window, cx| {
            root.update(cx, |root, cx| {
                root.direct_ticket = Some(ticket);
                cx.notify();
            });
            window.draw(cx).clear();
        });

        assert_eq!(
            cached_renders.get(),
            1,
            "the unchanged child must replay its paint journal"
        );
        assert_eq!(
            direct_outcomes.borrow().as_slice(),
            &[Err(RetainedVisualInvalidation::DuplicateReplay)],
            "cached replay must reserve the ticket before a direct sibling can replay it"
        );
    }

    #[test]
    fn atlas_rejected_candidate_reports_rejected_focus_completion_exactly_once() {
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(RejectOnceAtlas::new());
        let remaining_requests = Rc::new(Cell::new(0));
        let next_request_id = Rc::new(Cell::new(0));
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let subscriptions = Rc::new(RefCell::new(Vec::new()));
        let (root, cx) = test_app.add_window_view({
            let atlas = atlas.clone();
            let remaining_requests = remaining_requests.clone();
            let next_request_id = next_request_id.clone();
            let outcomes = outcomes.clone();
            let subscriptions = subscriptions.clone();
            move |window, cx| {
                window.sprite_atlas = atlas;
                CandidateFocusCompletionRoot {
                    focus: cx.focus_handle(),
                    tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                    remaining_requests,
                    next_request_id,
                    outcomes,
                    subscriptions,
                }
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        let committed_generation = cx.update(|window, _| window.rendered_frame.generation);
        remaining_requests.set(2);
        atlas.fail_next_lease();
        root.update(cx, |_, cx| cx.notify());

        cx.update(|window, cx| {
            assert_eq!(window.rendered_frame.generation, committed_generation);
            window.draw(cx).clear();
            assert_eq!(window.rendered_frame.generation, committed_generation + 1);
        });
        test_app.run_until_parked();

        assert_eq!(
            outcomes.borrow().as_slice(),
            &[
                (1, FocusClaimOutcome::Rejected),
                (2, FocusClaimOutcome::Committed),
            ],
            "the rejected attempt and accepted retry must each settle exactly once"
        );
        assert_eq!(subscriptions.borrow().len(), 2);
    }

    #[test]
    fn rejected_first_image_candidate_stays_hidden_until_fresh_repaint_is_presented() {
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(RejectOnceAtlas::new());
        atlas.fail_next_lease();
        let image = Arc::new(RenderImage::new(smallvec::smallvec![Frame::new(
            ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0xff])),
        )]));
        let command_observed = Rc::new(Cell::new(false));
        let retained_platform_window = Rc::new(RefCell::new(None));
        let app = Rc::downgrade(&test_app.app);

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let atlas = atlas.clone();
                    let image = image.clone();
                    let command_observed = command_observed.clone();
                    let retained_platform_window = retained_platform_window.clone();
                    let app = app.clone();
                    move |window, cx| {
                        window.sprite_atlas = atlas;
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        retained_platform_window
                            .borrow_mut()
                            .replace(platform_window.clone());
                        platform_window.set_platform_command_callback(
                            move |command, platform_window| {
                                assert_eq!(
                                    command,
                                    PlatformWindowCommand::CompleteInitialPresentation {
                                        activate: true,
                                    }
                                );
                                let app = app
                                    .upgrade()
                                    .expect("the App must outlive initial presentation");
                                let mut app_borrow = app.try_borrow_mut().expect(
                                    "initial presentation must run after the App borrow is released",
                                );
                                let facts = app_borrow
                                    .update_window(platform_window.handle(), |_, window, _| {
                                        window.presentation_facts()
                                    })
                                    .expect("the recovered window must remain registered");
                                drop(app_borrow);
                                let generation = facts
                                    .frame_accepted_generation
                                    .expect("the fresh candidate must be accepted");
                                assert!(generation > 0);
                                assert_eq!(
                                    facts.initial_presentation,
                                    WindowInitialPresentationStatus::Pending,
                                    "completion must remain pending until the platform accepts the command"
                                );
                                assert_eq!(
                                    facts.present_submitted_generation,
                                    Some(generation),
                                    "the completion command must follow the accepted generation"
                                );
                                assert_eq!(
                                    facts.non_empty_presented_generation,
                                    Some(generation),
                                    "the completion command must follow a non-empty presentation"
                                );
                                assert_eq!(
                                    platform_window.draw_count(),
                                    1,
                                    "generation zero must never be submitted while recovery is pending"
                                );
                                command_observed.set(true);
                                PlatformWindowCommandOutcome::Accepted
                            },
                        );
                        cx.new(|cx| InitialAtlasImageRoot::new(image, window, None, cx))
                    }
                })
            })
            .expect("a recoverable first candidate must not fail synchronous window creation")
            .into();
        test_app.run_until_parked();

        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        assert!(command_observed.get());
        assert!(test_app.windows().contains(&handle));
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::CompleteInitialPresentation { activate: true }]
        );
        assert_eq!(
            platform_window.initial_presentation_state(),
            (true, true, 1)
        );
        assert_eq!(
            test_app
                .update_window(handle, |_, window, _| {
                    window.presentation_facts().initial_presentation
                })
                .expect("the recovered window must remain registered"),
            WindowInitialPresentationStatus::Completed
        );
    }

    #[test]
    fn persistent_first_image_lease_failure_rejects_and_closes_the_hidden_window() {
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(AlwaysRejectAtlas::new());
        let image = Arc::new(RenderImage::new(smallvec::smallvec![Frame::new(
            ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0xff])),
        )]));
        let observations = Rc::new(RefCell::new(Vec::new()));
        let retained_platform_window = Rc::new(RefCell::new(None));

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let atlas = atlas.clone();
                    let image = image.clone();
                    let observations = observations.clone();
                    let retained_platform_window = retained_platform_window.clone();
                    move |window, cx| {
                        window.sprite_atlas = atlas;
                        retained_platform_window.borrow_mut().replace(
                            window
                                .platform_window
                                .as_test()
                                .expect("the test backend must provide a TestWindow")
                                .clone(),
                        );
                        cx.new(|cx| {
                            InitialAtlasImageRoot::new(image, window, Some(observations), cx)
                        })
                    }
                })
            })
            .expect("resource rejection must settle after the window commits")
            .into();
        test_app.run_until_parked();

        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        assert!(!test_app.windows().contains(&handle));
        assert_eq!(
            observations.borrow().as_slice(),
            [WindowInitialPresentationStatus::Rejected],
            "the terminal retry budget must settle observers exactly once"
        );
        assert_eq!(
            atlas.lease_attempts(),
            usize::from(FRESH_INITIAL_PRESENTATION_ATTEMPT_LIMIT) + 1,
            "the initial candidate plus the bounded retry budget must be attempted"
        );
        assert!(platform_window.platform_command_history().is_empty());
        assert_eq!(platform_window.draw_count(), 0);
        assert_eq!(
            platform_window.initial_presentation_state(),
            (false, false, 0)
        );
    }

    #[test]
    fn cached_view_repaints_instead_of_inheriting_a_failed_atlas_lease() {
        let mut test_app = TestAppContext::single();
        let renders = Rc::new(Cell::new(0));
        let image = Arc::new(RenderImage::new(smallvec::smallvec![Frame::new(
            ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0xff])),
        )]));
        let (_root, cx) = test_app.add_window_view({
            let renders = renders.clone();
            move |_, cx| CachedImageRoot {
                child: cx.new(|_| CachedImageChild { image, renders }),
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(renders.get(), 1);
        cx.update(|window, _| {
            assert!(
                !window.rendered_frame.atlas_texture_lease_entries.is_empty(),
                "the image paint should own a frame atlas lease"
            );
            for entry in &mut window.rendered_frame.atlas_texture_lease_entries {
                let (texture, epoch) = {
                    let lease = entry
                        .as_ref()
                        .expect("the freshly painted image should hold a live atlas lease");
                    (
                        *lease
                            .texture_instances()
                            .first()
                            .expect("the image lease should retain one texture instance"),
                        lease.epoch(),
                    )
                };
                *entry = Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch });
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(
            renders.get(),
            2,
            "a failed cached lease must force a fresh image paint"
        );
        assert!(cx.update(|window, _| {
            window
                .rendered_frame
                .atlas_texture_lease_entries
                .iter()
                .all(Result::is_ok)
        }));
    }

    #[test]
    fn cached_paint_journals_one_atlas_lease_per_texture_instance() {
        let mut test_app = TestAppContext::single();
        let atlas = Arc::new(RejectOnceAtlas::new());
        let tile = RejectOnceAtlas::tile(AtlasTextureKind::Monochrome);
        let (_root, cx) = test_app.add_window_view({
            let atlas = atlas.clone();
            move |window, cx| {
                window.sprite_atlas = atlas;
                RepeatedAtlasPrimitiveRoot {
                    child: cx.new(|_| RepeatedAtlasPrimitiveChild {
                        tile,
                        primitive_count: 512,
                    }),
                }
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(
            cx.update(|window, _| window.rendered_frame.atlas_texture_lease_entries.len()),
            1,
            "one cached paint scope must journal each atlas texture instance only once"
        );

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(
            cx.update(|window, _| window.rendered_frame.atlas_texture_lease_entries.len()),
            1,
            "cached replay must preserve unique atlas dependencies"
        );
    }

    #[test]
    fn renderer_resource_invalidation_forces_a_new_generation_before_deferred_retry() {
        let mut test_app = TestAppContext::single();
        let (_root, cx) = test_app.add_window_view(|_, _| RendererRepaintRoot);
        let platform_window = cx.update(|window, _| {
            window
                .platform_window
                .as_test()
                .expect("the test backend must provide a TestWindow")
                .clone()
        });

        cx.update(|window, cx| {
            platform_window.set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
            window.refresh();
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: true,
                    require_presentation: true,
                },
                cx,
            );
            let invalidated_generation = window
                .presentation_facts()
                .latest_present_attempt
                .expect("the invalidated scene must be observable")
                .generation;
            assert_eq!(
                window
                    .presentation_facts()
                    .latest_present_attempt
                    .expect("the invalidated scene must be observable")
                    .outcome,
                PlatformWindowPresentOutcome::RepaintRequired
            );
            assert!(window.renderer_repaint_is_pending());
            let draw_count_after_invalidation = platform_window.draw_count();

            window.active.set(false);
            window.last_frame_time.set(Some(Instant::now()));
            platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: false,
                    require_presentation: true,
                },
                cx,
            );
            let fresh_generation = window
                .presentation_facts()
                .frame_accepted_generation
                .expect("the recovery frame must be accepted");
            assert!(fresh_generation > invalidated_generation);
            assert_eq!(
                platform_window.draw_count(),
                draw_count_after_invalidation + 1,
                "the invalidated scene must not be handed to the renderer again"
            );
            assert_eq!(
                window
                    .presentation_facts()
                    .latest_present_attempt
                    .expect("the fresh present attempt must be observable")
                    .outcome,
                PlatformWindowPresentOutcome::Deferred
            );
            assert!(!window.renderer_repaint_is_pending());

            let draw_count_after_fresh_attempt = platform_window.draw_count();
            window.last_frame_time.set(None);
            platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: false,
                    require_presentation: true,
                },
                cx,
            );
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                Some(fresh_generation),
                "ordinary Deferred must remain retryable with the same fresh scene"
            );
            assert_eq!(
                platform_window.draw_count(),
                draw_count_after_fresh_attempt + 1
            );
            assert_eq!(
                window
                    .presentation_facts()
                    .latest_present_attempt
                    .expect("the retried presentation must be observable")
                    .outcome,
                PlatformWindowPresentOutcome::Submitted
            );
        });
    }

    #[test]
    fn repaint_required_first_present_stays_hidden_until_a_fresh_scene_submits() {
        let mut test_app = TestAppContext::single();
        let retained_platform_window = Rc::new(RefCell::new(None));

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let retained_platform_window = retained_platform_window.clone();
                    move |window, cx| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window.defer_frame_requests_for_test();
                        platform_window
                            .set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
                        retained_platform_window
                            .borrow_mut()
                            .replace(platform_window);
                        cx.new(|_| RendererRepaintRoot)
                    }
                })
            })
            .expect("renderer resource invalidation must not roll back window creation")
            .into();
        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        let invalidated_generation = test_app
            .update_window(handle, |_, window, _| {
                let facts = window.presentation_facts();
                assert!(!facts.native_visible);
                assert_eq!(
                    facts.initial_presentation,
                    WindowInitialPresentationStatus::Pending
                );
                assert_eq!(
                    facts
                        .latest_present_attempt
                        .expect("the initial present attempt must be observable")
                        .outcome,
                    PlatformWindowPresentOutcome::RepaintRequired
                );
                facts
                    .frame_accepted_generation
                    .expect("the initial scene must have been accepted")
            })
            .expect("the hidden window must remain registered");
        assert_eq!(platform_window.draw_count(), 1);
        assert!(platform_window.platform_command_history().is_empty());

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        assert!(platform_window.release_deferred_frame_request_for_test());
        test_app.run_until_parked();

        let facts = test_app
            .update_window(handle, |_, window, _| window.presentation_facts())
            .expect("the recovered window must remain registered");
        assert!(facts.native_visible);
        assert!(
            facts
                .frame_accepted_generation
                .is_some_and(|generation| generation > invalidated_generation)
        );
        assert_eq!(
            facts.initial_presentation,
            WindowInitialPresentationStatus::Completed
        );
        assert_eq!(platform_window.draw_count(), 2);
    }

    #[test]
    fn deferred_fresh_initial_scene_retries_without_advancing_generation() {
        let mut test_app = TestAppContext::single();
        let retained_platform_window = Rc::new(RefCell::new(None));

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let retained_platform_window = retained_platform_window.clone();
                    move |window, cx| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window.defer_frame_requests_for_test();
                        platform_window
                            .set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
                        retained_platform_window
                            .borrow_mut()
                            .replace(platform_window);
                        cx.new(|_| RendererRepaintRoot)
                    }
                })
            })
            .expect("renderer resource invalidation must begin hidden recovery")
            .into();
        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        let fresh_generation = test_app
            .update_window(handle, |_, window, _| {
                let facts = window.presentation_facts();
                assert!(window.fresh_initial_presentation_is_pending());
                facts
                    .frame_accepted_generation
                    .expect("test flush must accept the requested fresh scene")
            })
            .expect("the hidden window must remain registered");
        assert_eq!(
            platform_window.draw_count(),
            1,
            "the accepted fresh scene must not reach the deferred platform request early"
        );

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
        assert!(platform_window.step_deferred_frame_request_for_test());
        test_app.run_until_parked();
        test_app
            .update_window(handle, |_, window, _| {
                let facts = window.presentation_facts();
                assert!(!facts.native_visible);
                assert_eq!(
                    facts.initial_presentation,
                    WindowInitialPresentationStatus::Pending
                );
                assert_eq!(
                    facts
                        .latest_present_attempt
                        .expect("the fresh deferred attempt must be observable")
                        .outcome,
                    PlatformWindowPresentOutcome::Deferred
                );
                assert!(!window.fresh_initial_presentation_frame_is_required());
                assert_eq!(
                    facts.frame_accepted_generation,
                    Some(fresh_generation),
                    "a stale force-render hint must present the accepted fresh scene without replacing it"
                );
            })
            .expect("the hidden window must remain registered");

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        assert!(platform_window.release_deferred_frame_request_for_test());
        test_app.run_until_parked();
        test_app
            .update_window(handle, |_, window, _| {
                let facts = window.presentation_facts();
                assert_eq!(
                    facts.present_submitted_generation,
                    Some(fresh_generation),
                    "the deferred retry must submit the already accepted fresh scene"
                );
                assert_eq!(
                    facts.non_empty_presented_generation,
                    Some(fresh_generation),
                    "the exact retried scene must satisfy non-empty initial presentation"
                );
                assert_eq!(
                    facts
                        .latest_present_attempt
                        .expect("the submitted retry must be observable")
                        .generation,
                    fresh_generation
                );
                assert!(!window.fresh_initial_presentation_is_pending());
            })
            .expect("the recovered window must remain registered");
        assert_eq!(platform_window.draw_count(), 3);
    }

    #[test]
    fn persistent_deferred_initial_scene_exhausts_same_generation_retry_budget() {
        let mut test_app = TestAppContext::single();
        let retained_platform_window = Rc::new(RefCell::new(None));

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let retained_platform_window = retained_platform_window.clone();
                    move |window, cx| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window.defer_frame_requests_for_test();
                        platform_window
                            .set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
                        retained_platform_window
                            .borrow_mut()
                            .replace(platform_window);
                        cx.new(|_| RendererRepaintRoot)
                    }
                })
            })
            .expect("renderer resource invalidation must begin hidden recovery")
            .into();
        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);

        for retry in 0..=INITIAL_PRESENTATION_RETRY_LIMIT {
            assert!(platform_window.step_deferred_frame_request_for_test());
            test_app.run_until_parked();
            assert_eq!(
                test_app.windows().contains(&handle),
                retry < INITIAL_PRESENTATION_RETRY_LIMIT,
                "the window must close only after the final same-generation retry"
            );
        }

        assert!(!test_app.windows().contains(&handle));
        assert_eq!(
            platform_window.draw_count(),
            usize::from(INITIAL_PRESENTATION_RETRY_LIMIT) + 2,
            "the initial invalidation and one fresh scene precede the bounded present retries"
        );
        assert!(platform_window.platform_command_history().is_empty());
    }

    #[test]
    fn persistent_empty_initial_submission_exhausts_fresh_generation_budget() {
        let mut test_app = TestAppContext::single();
        let retained_platform_window = Rc::new(RefCell::new(None));

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let retained_platform_window = retained_platform_window.clone();
                    move |window, cx| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window
                            .set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
                        retained_platform_window
                            .borrow_mut()
                            .replace(platform_window);
                        cx.new(|_| EmptyRendererRepaintRoot)
                    }
                })
            })
            .expect("renderer resource invalidation must begin hidden recovery")
            .into();
        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);

        test_app.run_until_parked();

        assert!(!test_app.windows().contains(&handle));
        assert_eq!(
            platform_window.draw_count(),
            usize::from(FRESH_INITIAL_PRESENTATION_ATTEMPT_LIMIT) + 1
        );
        assert!(platform_window.platform_command_history().is_empty());
    }

    #[test]
    fn persistent_first_present_resource_invalidation_closes_the_hidden_window() {
        let mut test_app = TestAppContext::single();
        let retained_platform_window = Rc::new(RefCell::new(None));

        let handle: AnyWindowHandle = test_app
            .update(|cx| {
                cx.open_window(WindowOptions::default(), {
                    let retained_platform_window = retained_platform_window.clone();
                    move |window, cx| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window
                            .set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
                        retained_platform_window
                            .borrow_mut()
                            .replace(platform_window);
                        cx.new(|_| RendererRepaintRoot)
                    }
                })
            })
            .expect("bounded recovery starts after the hidden window commits")
            .into();
        test_app.run_until_parked();

        let platform_window = retained_platform_window
            .borrow()
            .clone()
            .expect("the test must retain its platform window");
        assert!(!test_app.windows().contains(&handle));
        assert_eq!(
            platform_window.draw_count(),
            usize::from(FRESH_INITIAL_PRESENTATION_ATTEMPT_LIMIT) + 1
        );
        assert!(platform_window.platform_command_history().is_empty());
        assert_eq!(
            platform_window.initial_presentation_state(),
            (false, false, 0)
        );
    }

    #[test]
    fn cached_paint_rejects_a_lease_from_an_older_renderer_epoch() {
        let mut test_app = TestAppContext::single();
        let (_root, cx) = test_app.add_window_view(|_, _| Empty);
        let atlas = Arc::new(EpochAtlas::new());
        let texture = AtlasTextureInstanceId {
            texture_id: AtlasTextureId {
                index: 1,
                kind: AtlasTextureKind::Polychrome,
            },
            generation: 1,
        };
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
        let lease = Rc::new(
            platform_atlas
                .retain_texture_instances(&[texture])
                .expect("the current atlas instance should be retainable"),
        );

        cx.update(|window, _| {
            window.rendered_frame.atlas_texture_lease_entries = vec![Ok(lease)];
            let mut end = PaintIndex::default();
            end.atlas_texture_lease_entries_index = 1;
            assert!(window.can_reuse_paint(&(PaintIndex::default()..end.clone())));

            atlas.reset();
            assert!(
                !window.can_reuse_paint(&(PaintIndex::default()..end)),
                "renderer reset must invalidate cached paint even if the slot id is unchanged"
            );
        });
    }
}

#[cfg(test)]
mod raster_projection_tests {
    use super::*;

    #[test]
    fn raster_stroke_round_trips_through_the_corrected_axis_scale() {
        assert_eq!(
            Window::try_raster_local_stroke(ScaledPixels(0.25), 2.0, 2.0),
            Ok(ScaledPixels(0.5))
        );
        assert_eq!(
            Window::try_raster_local_stroke(ScaledPixels::default(), f32::MAX, f32::MAX),
            Ok(ScaledPixels::default())
        );
    }

    #[test]
    fn raster_stroke_rejects_projection_and_inverse_projection_underflow() {
        assert_eq!(
            Window::try_raster_local_stroke(
                ScaledPixels(f32::MIN_POSITIVE),
                f32::MIN_POSITIVE,
                1.0,
            ),
            Err(SubtreeTransformError::UnrepresentableResult)
        );
        assert_eq!(
            Window::try_raster_local_stroke(ScaledPixels(1.0), 1.0, f32::MAX),
            Err(SubtreeTransformError::UnrepresentableResult)
        );
    }
}
