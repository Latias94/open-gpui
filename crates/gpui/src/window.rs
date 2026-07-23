#[cfg(any(feature = "inspector", debug_assertions))]
use crate::Inspector;
#[cfg(not(target_family = "wasm"))]
use crate::MouseButton;
#[cfg(target_os = "macos")]
use crate::PlatformPixelBuffer;
use crate::{
    Action, AnyElement, AnyEntity, AnyImageCache, AnyTooltip, AnyView, App, AppContext, Arena,
    Asset, AsyncWindowContext, AtlasAccessDiagnostic, AtlasRemoveDiagnostic, AvailableSpace,
    Background, BorderStyle, Bounds, BoxShadow, Capslock, Context, Corners, CursorHideMode,
    CursorStyle, Decorations, DevicePixels, DispatchActionListener, DispatchNodeId, DispatchTree,
    DisplayId, Edges, ElementGeometry, Entity, EntityId, EventEmitter, FontId, Global,
    GlobalElementId, GlyphId, GpuSpecs, Hsla, InputHandler, IsZero, KeyBinding, KeyContext,
    KeyDownEvent, KeyEvent, KeyUpEvent, Keystroke, KeystrokeEvent, LayoutId, Modifiers,
    ModifiersChangedEvent, MonochromeSprite, MouseEvent, MouseMoveEvent, MouseUpEvent, Path,
    Pixels, PlatformAtlas, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow,
    Point, PointerCancelEvent, PointerCancelReason, PolychromeSprite, Primitive,
    PrimitiveTransform, Priority, PromptButton, PromptLevel, Quad, Render, RenderGlyphParams,
    RenderImage, RenderImageParams, RenderSvgParams, Replay, ResizeEdge, SMOOTH_SVG_SCALE_FACTOR,
    SUBPIXEL_VARIANTS_X, SUBPIXEL_VARIANTS_Y, ScaledPixels, Shadow, SharedString, Size,
    StrikethroughStyle, Style, SubpixelSprite, SubscriberSet, Subscription, SubtreePresentation,
    SubtreeTransform, SubtreeTransformError, SystemWindowTab, SystemWindowTabController,
    TaffyLayoutEngine, Task, TextRenderingMode, TextStyle, TextStyleRefinement, Underline,
    UnderlineStyle, WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControls,
    WindowDecorations, WindowOptions, WindowParams, WindowTextSystem,
    geometry::{ResolvedSubtreeTransform, SubtreeTransformValidity},
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
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    ops::{DerefMut, Range},
    rc::Rc,
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
    DeferredDraw, Frame, FrameOutput, PaintIndex, PrepaintCommit, PrepaintCommitPhase,
    PrepaintStateIndex, TooltipRequest,
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
}

#[derive(Clone)]
pub(crate) struct CursorStyleRequest {
    pub(crate) hitbox_id: Option<HitboxId>,
    pub(crate) style: CursorStyle,
    validity: Option<SubtreeTransformValidity>,
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

/// A stable identity for one cross-frame publication produced during prepaint.
///
/// Reuse the same ID for one logical publication on every frame. GPUI commits a valid current
/// frame, discards an invalid one, and also invokes the previous frame's discard callback when the
/// publication is absent from the next frame. The absence rule retracts state when a subtree is
/// removed, skipped by an invalid ancestor transform, or rolled back by [`Window::transact`].
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

/// A rectangular region that potentially blocks hitboxes inserted prior.
/// See [Window::insert_hitbox] for more details.
#[derive(Clone, Debug)]
pub struct Hitbox {
    /// A unique identifier for the hitbox.
    pub id: HitboxId,
    geometry: ElementGeometry,
    validity: Option<SubtreeTransformValidity>,
    content_mask: ContentMask<Pixels>,
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

    /// Returns the window-space content mask captured with this hitbox.
    pub fn displayed_content_mask(&self) -> ContentMask<Pixels> {
        self.content_mask
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
                .is_none_or(SubtreeTransformValidity::is_valid)
    }

    fn retag_validity(&mut self, validity: Option<SubtreeTransformValidity>) {
        self.validity = SubtreeTransformValidity::replayed_under(self.validity.as_ref(), validity);
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

    /// Returns whether a displayed window point lies inside this hitbox and its content mask.
    pub fn contains_window_point(&self, point: Point<Pixels>) -> bool {
        self.geometry
            .displayed_bounds()
            .intersect(&self.content_mask.bounds)
            .contains(&point)
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
                        .is_none_or(SubtreeTransformValidity::is_valid)
                    && tooltip_bounds.bounds.contains(&window.mouse_position())
            })
    }
}

pub(crate) struct TooltipBounds {
    id: TooltipId,
    bounds: Bounds<Pixels>,
    validity: Option<SubtreeTransformValidity>,
}

struct PreparedTooltip {
    element: AnyElement,
    validity: Option<SubtreeTransformValidity>,
}

#[derive(Clone)]
pub(crate) struct AutoscrollIntent {
    bounds: Bounds<Pixels>,
    validity: Option<SubtreeTransformValidity>,
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
    validity: Option<SubtreeTransformValidity>,
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

/// Holds the state for a specific window.
pub struct Window {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) invalidator: WindowInvalidator,
    pub(crate) removed: bool,
    removal_state: WindowRemovalState,
    pub(crate) platform_window: Box<dyn PlatformWindow>,
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
    pub(crate) content_mask_stack: Vec<ContentMask<Pixels>>,
    pub(crate) requested_autoscroll: Option<AutoscrollIntent>,
    pub(crate) image_cache_stack: Vec<AnyImageCache>,
    pub(crate) rendered_frame: Frame,
    pub(crate) next_frame: Frame,
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
    appearance: WindowAppearance,
    pub(crate) appearance_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) button_layout_observers: SubscriberSet<(), AnyObserver>,
    active: Rc<Cell<bool>>,
    hovered: Rc<Cell<bool>>,
    pub(crate) needs_present: Rc<Cell<bool>>,
    /// Tracks recent input event timestamps to determine if input is arriving at a high rate.
    /// Used to selectively enable VRR optimization only when input rate exceeds 60fps.
    pub(crate) input_rate_tracker: Rc<RefCell<InputRateTracker>>,
    #[cfg(feature = "input-latency-histogram")]
    input_latency_tracker: InputLatencyTracker,
    last_input_modality: InputModality,
    pub(crate) refreshing: bool,
    pub(crate) activation_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) focus: Option<FocusId>,
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
    pending_pointer_cancellation: Option<PendingPointerCancellation>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusClaimTarget {
    Exact(FocusId),
    Empty,
}

type AnyFocusClaimCompletion = Box<dyn FnOnce(FocusClaimOutcome, &mut Window, &mut App) + 'static>;
type SharedFocusClaimCompletion = Rc<RefCell<Option<AnyFocusClaimCompletion>>>;

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
            focus,
            show,
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

        let window_bounds = window_bounds.unwrap_or_else(|| default_bounds(display_id, cx));
        let mut platform_window = cx.platform.open_window(
            handle,
            WindowParams {
                bounds: window_bounds.get_bounds(),
                titlebar,
                kind,
                is_movable,
                is_resizable,
                is_minimizable,
                accepts_pointer_input,
                focus,
                show,
                display_id,
                window_min_size,
                icon,
                #[cfg(target_os = "macos")]
                tabbing_identifier,
            },
        )?;

        let tab_bar_visible = platform_window.tab_bar_visible();
        SystemWindowTabController::init_visible(cx, tab_bar_visible);
        if let Some(tabs) = platform_window.tabbed_windows() {
            SystemWindowTabController::add_tab(cx, handle.window_id(), tabs);
        }

        let display_id = platform_window.display().map(|display| display.id());
        let sprite_atlas = platform_window.sprite_atlas();
        let mouse_position = platform_window.mouse_position();
        let modifiers = platform_window.modifiers();
        let capslock = platform_window.capslock();
        let content_size = platform_window.content_size();
        let scale_factor = platform_window.scale_factor();
        let appearance = platform_window.appearance();
        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let invalidator = WindowInvalidator::new();
        let active = Rc::new(Cell::new(platform_window.is_active()));
        let hovered = Rc::new(Cell::new(platform_window.is_hovered()));
        let needs_present = Rc::new(Cell::new(false));
        let next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>> = Default::default();
        let input_rate_tracker = Rc::new(RefCell::new(InputRateTracker::default()));
        let last_frame_time = Rc::new(Cell::new(None));

        platform_window
            .request_decorations(window_decorations.unwrap_or(WindowDecorations::Server));
        platform_window.set_background_appearance(window_background);

        match window_bounds {
            WindowBounds::Fullscreen(_) => platform_window.toggle_fullscreen(),
            WindowBounds::Maximized(_) => platform_window.zoom(),
            WindowBounds::Windowed(_) => {}
        }

        let accessibility_force_disabled = cx.accessibility_force_disabled;
        let a11y_active_state = Arc::new(AtomicU64::new(0));

        #[cfg(not(target_family = "wasm"))]
        if !accessibility_force_disabled {
            let (activation_sender, activation_receiver) = async_channel::unbounded::<()>();
            let (deactivation_sender, deactivation_receiver) = async_channel::unbounded::<()>();
            let (action_sender, action_receiver) =
                async_channel::unbounded::<(u64, accesskit::ActionRequest)>();

            platform_window.a11y_init(crate::A11yCallbacks {
                activation: {
                    let active_state = a11y_active_state.clone();
                    Box::new(move || {
                        log::info!("Accessibility activated");
                        a11y::set_requested_active(&active_state, true);
                        activation_sender.send_blocking(()).log_err();
                        None
                    })
                },
                action: {
                    let active_state = a11y_active_state.clone();
                    Box::new(move |request| {
                        let generation = a11y::requested_generation(&active_state);
                        action_sender.send_blocking((generation, request)).log_err();
                    })
                },
                deactivation: {
                    let active_state = a11y_active_state.clone();
                    Box::new(move || {
                        log::info!("Accessibility deactivated");
                        a11y::set_requested_active(&active_state, false);
                        deactivation_sender.send_blocking(()).log_err();
                    })
                },
            });

            // Accessibility can be activated at any time, so a complete tree cannot be
            // produced synchronously here. Returning `None` lets the platform adapter own
            // any temporary placeholder while this refresh produces the real full tree.
            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while activation_receiver.recv().await.is_ok() {
                        handle
                            .update(&mut async_cx, |_, window, _| window.refresh())
                            .log_err();
                    }
                })
                .detach();

            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while deactivation_receiver.recv().await.is_ok() {
                        handle
                            .update(&mut async_cx, |_, window, _| window.refresh())
                            .log_err();
                    }
                })
                .detach();

            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while let Ok((activation_generation, request)) = action_receiver.recv().await {
                        handle
                            .update(&mut async_cx, |_, window, cx| {
                                window.with_input_transaction(cx, |window, cx| {
                                    window.handle_a11y_action(activation_generation, request, cx);
                                });
                            })
                            .log_err();
                    }
                })
                .detach();
        }

        platform_window.on_close(Box::new({
            let window_id = handle.window_id();
            let mut cx = cx.to_async();
            move || {
                let _ = handle.update(&mut cx, |_, window, cx| window.remove_window(cx));
                let _ = cx.update(|cx| {
                    SystemWindowTabController::remove_tab(cx, window_id);
                });
            }
        }));
        platform_window.on_request_frame(Box::new({
            let mut cx = cx.to_async();
            let invalidator = invalidator.clone();
            let active = active.clone();
            let needs_present = needs_present.clone();
            let next_frame_callbacks = next_frame_callbacks.clone();
            let input_rate_tracker = input_rate_tracker.clone();
            move |request_frame_options| {
                let thermal_state = handle
                    .update(&mut cx, |_, _, cx| cx.thermal_state())
                    .log_err();

                let min_frame_interval = FrameThrottleFacts {
                    force_render: request_frame_options.force_render,
                    require_presentation: request_frame_options.require_presentation,
                    has_next_frame_callbacks: !next_frame_callbacks.borrow().is_empty(),
                    active: active.get(),
                    thermal_state,
                }
                .min_frame_interval();
                let now = Instant::now();
                if frame_should_wait(now, last_frame_time.get(), min_frame_interval) {
                    // Must still complete the frame on platforms that require it.
                    // On Wayland, `surface.frame()` was already called to request the
                    // next frame callback, so we must call `surface.commit()` (via
                    // `complete_frame`) or the compositor won't send another callback.
                    handle
                        .update(&mut cx, |_, window, _| window.complete_frame())
                        .log_err();
                    return;
                }
                last_frame_time.set(Some(now));

                let next_frame_callbacks = next_frame_callbacks.take();
                if !next_frame_callbacks.is_empty() {
                    handle
                        .update(&mut cx, |_, window, cx| {
                            for callback in next_frame_callbacks {
                                callback(window, cx);
                            }
                        })
                        .log_err();
                }

                // Keep presenting if input was recently arriving at a high rate (>= 60fps).
                // Once high-rate input is detected, we sustain presentation for 1 second
                // to prevent display underclocking during active input.
                let needs_present = PresentFacts {
                    require_presentation: request_frame_options.require_presentation,
                    needs_present: needs_present.get(),
                    active: active.get(),
                    high_rate_input: input_rate_tracker.borrow_mut().is_high_rate(),
                }
                .needs_present();

                if invalidator.is_dirty() || request_frame_options.force_render {
                    measure("frame duration", || {
                        handle
                            .update(&mut cx, |_, window, cx| {
                                if request_frame_options.force_render {
                                    // Bypass cached view reuse so we don't replay stale
                                    // atlas tile references after a GPU device recovery.
                                    window.refresh();
                                }
                                let arena_clear_needed = window.draw(cx);
                                window.present();
                                arena_clear_needed.clear();
                            })
                            .log_err();
                    })
                } else if needs_present {
                    handle
                        .update(&mut cx, |_, window, _| window.present())
                        .log_err();
                }

                handle
                    .update(&mut cx, |_, window, _| {
                        window.complete_frame();
                    })
                    .log_err();
            }
        }));
        platform_window.on_resize(Box::new({
            let mut cx = cx.to_async();
            move |_, _| {
                handle
                    .update(&mut cx, |_, window, cx| window.bounds_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_moved(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.bounds_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_appearance_changed(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.appearance_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_button_layout_changed(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.button_layout_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_active_status_change(Box::new({
            let mut cx = cx.to_async();
            move |active| {
                handle
                    .update(&mut cx, |_, window, cx| {
                        window.active.set(active);
                        if !active {
                            window
                                .cancel_pointer_session(PointerCancelReason::WindowDeactivated, cx);
                        }
                        window.modifiers = window.platform_window.modifiers();
                        window.capslock = window.platform_window.capslock();
                        window
                            .activation_observers
                            .clone()
                            .retain(&(), |callback| callback(window, cx));

                        window.bounds_changed(cx);
                        window.refresh();

                        SystemWindowTabController::update_last_active(cx, window.handle.id);
                    })
                    .log_err();
            }
        }));
        platform_window.on_hover_status_change(Box::new({
            let mut cx = cx.to_async();
            move |active| {
                handle
                    .update(&mut cx, |_, window, cx| {
                        window.hovered.set(active);
                        window.mouse_in_window = active;
                        if !active {
                            window.reset_cursor_style(cx);
                        }
                        window.refresh();
                    })
                    .log_err();
            }
        }));
        platform_window.on_input({
            let mut cx = cx.to_async();
            Box::new(move |event| {
                handle
                    .update(&mut cx, |_, window, cx| window.dispatch_event(event, cx))
                    .log_err()
                    .unwrap_or(DispatchEventResult::default())
            })
        });
        platform_window.on_hit_test_window_control({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, window, _cx| {
                        for (area, hitbox) in &window.rendered_frame.window_control_hitboxes {
                            if hitbox.is_active() && window.mouse_hit_test.ids.contains(&hitbox.id)
                            {
                                return Some(*area);
                            }
                        }
                        None
                    })
                    .log_err()
                    .unwrap_or(None)
            })
        });
        platform_window.on_move_tab_to_new_window({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::move_tab_to_new_window(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_merge_all_windows({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::merge_all_windows(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_select_next_tab({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::select_next_tab(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_select_previous_tab({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::select_previous_tab(cx, handle.window_id())
                    })
                    .log_err();
            })
        });
        platform_window.on_toggle_tab_bar({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, window, cx| {
                        let tab_bar_visible = window.platform_window.tab_bar_visible();
                        SystemWindowTabController::set_visible(cx, tab_bar_visible);
                    })
                    .log_err();
            })
        });

        if let Some(app_id) = app_id {
            platform_window.set_app_id(&app_id);
        }

        platform_window.map_window().unwrap();

        Ok(Window {
            handle,
            invalidator,
            removed: false,
            removal_state: WindowRemovalState::Open,
            platform_window,
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
            content_mask_stack: Vec::new(),
            element_opacity: 1.0,
            requested_autoscroll: None,
            rendered_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
            next_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
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
            appearance,
            appearance_observers: SubscriberSet::new(),
            button_layout_observers: SubscriberSet::new(),
            active,
            hovered,
            needs_present,
            input_rate_tracker,
            #[cfg(feature = "input-latency-histogram")]
            input_latency_tracker: InputLatencyTracker::new()?,
            last_input_modality: InputModality::Mouse,
            refreshing: false,
            activation_observers: SubscriberSet::new(),
            focus: None,
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
            pending_pointer_cancellation: None,
            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector: None,
            a11y: A11y::new(
                a11y_active_state,
                accessibility_force_disabled,
                handle.window_id(),
            ),
        })
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

/// Indicates which region of the window is visible. Content falling outside of this mask will not be
/// rendered. Currently, only rectangular content masks are supported, but we give the mask its own type
/// to leave room to support more complex shapes in the future.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ContentMask<P: Clone + Debug + Default + PartialEq> {
    /// The bounds
    pub bounds: Bounds<P>,
}

impl ContentMask<Pixels> {
    /// Scale the content mask's pixel units by the given scaling factor.
    pub fn scale(&self, factor: f32) -> ContentMask<ScaledPixels> {
        ContentMask {
            bounds: self.bounds.scale(factor),
        }
    }

    /// Intersect the content mask with the given content mask.
    pub fn intersect(&self, other: &Self) -> Self {
        let bounds = self.bounds.intersect(&other.bounds);
        ContentMask { bounds }
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
        if self.removed || self.removal_state == WindowRemovalState::Removing {
            return;
        }
        self.a11y.clear_announcements_for_window_close();
        if self.input_transaction_depth.get() > 0 {
            self.removal_state = WindowRemovalState::PendingAfterInput;
            return;
        }

        self.finish_remove_window(cx);
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
        let result = callback(self, cx);
        drop(transaction);
        self.finish_pending_window_removal(cx);
        result
    }

    fn finish_remove_window(&mut self, cx: &mut App) {
        self.removal_state = WindowRemovalState::Removing;
        self.cancel_pointer_session(PointerCancelReason::WindowClosed, cx);
        self.close_bring_into_view_authority(cx);
        self.pending_focus_reveal_fence = None;
        self.pending_focus_completion = None;
        self.focus_claim_resolutions.clear();
        self.removed = true;
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
            let already_committed = self.focus == Some(handle.id)
                && self.next_frame.focus == Some(handle.id)
                && self
                    .next_frame
                    .dispatch_tree
                    .valid_focusable_node_id(handle.id)
                    .is_some();
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
            let target_generation = self.next_frame.generation.saturating_add(1);
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

    fn focus_mutations_enabled(&self) -> bool {
        self.focus_enabled
            && self.subtree_presentation().is_interactive()
            && self.prepaint_commit_phase.get() != Some(PrepaintCommitPhase::FocusStable)
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
            window_handle
                .update(cx, |_, window, cx| {
                    window.dispatch_focus_claim_resolutions(cx);
                })
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

    fn defer_pending_input_changed(&self, cx: &mut App) {
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

    fn discard_resolved_candidate_focus_claim(&mut self) {
        if self
            .pending_focus_claim
            .is_some_and(|claim| claim.target_generation <= self.next_frame.generation)
        {
            self.pending_focus_claim = None;
        }
        if self
            .pending_blur_claim_generation
            .is_some_and(|generation| generation <= self.next_frame.generation)
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
            let already_committed = self.focus.is_none() && self.next_frame.focus.is_none();
            if already_committed {
                if let Some(completion) = completion {
                    self.queue_focus_claim_resolution(completion, FocusClaimOutcome::Committed);
                }
                self.reconcile_focus_followup_refresh();
                return true;
            }
            let target_generation = self.next_frame.generation.saturating_add(1);
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
        self.platform_window.is_maximized()
    }

    /// request a certain window decoration (Wayland)
    pub fn request_decorations(&self, decorations: WindowDecorations) {
        self.platform_window.request_decorations(decorations);
    }

    /// Start a window resize operation (Wayland)
    pub fn start_window_resize(&self, edge: ResizeEdge) {
        self.platform_window.start_window_resize(edge);
    }

    /// Return the `WindowBounds` to indicate that how a window should be opened
    /// after it has been closed
    pub fn window_bounds(&self) -> WindowBounds {
        self.platform_window.window_bounds()
    }

    /// Return the `WindowBounds` excluding insets (Wayland and X11)
    pub fn inner_window_bounds(&self) -> WindowBounds {
        self.platform_window.inner_window_bounds()
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
        self.scale_factor = self.platform_window.scale_factor();
        self.viewport_size = self.platform_window.content_size();
        self.display_id = self.platform_window.display().map(|display| display.id());

        self.refresh();

        self.bounds_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    /// Returns the bounds of the current window in the global coordinate space, which could span across multiple displays.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.platform_window.bounds()
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

    /// Set the content size of the window.
    pub fn resize(&mut self, size: Size<Pixels>) {
        self.platform_window.resize(size);
    }

    /// Returns whether or not the window is currently fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.platform_window.is_fullscreen()
    }

    /// Returns whether or not the window is currently minimized.
    pub fn is_minimized(&self) -> bool {
        self.platform_window.is_minimized()
    }

    /// Returns whether this platform window currently receives pointer input.
    pub fn accepts_pointer_input(&self) -> bool {
        self.platform_window.accepts_pointer_input()
    }

    /// Updates whether this platform window receives pointer input when the backend supports it.
    pub fn set_accepts_pointer_input(&mut self, accepts_pointer_input: bool) -> bool {
        self.platform_window
            .set_accepts_pointer_input(accepts_pointer_input)
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

    /// Toggle zoom on the window.
    pub fn zoom_window(&self) {
        self.platform_window.zoom();
    }

    /// Opens the native title bar context menu, useful when implementing client side decorations (Wayland and X11)
    pub fn show_window_menu(&self, position: Point<Pixels>) {
        self.platform_window.show_window_menu(position)
    }

    /// Handle window movement for Linux and macOS.
    /// Tells the compositor to take control of window movement (Wayland and X11)
    ///
    /// Events may not be received during a move operation.
    pub fn start_window_move(&self) {
        self.platform_window.start_window_move()
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

    /// Sets the window background appearance.
    pub fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.platform_window
            .set_background_appearance(background_appearance);
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

    #[inline]
    fn snapped_content_mask(&self) -> ContentMask<ScaledPixels> {
        ContentMask {
            bounds: self.cover_bounds(self.content_mask().bounds),
        }
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

    /// Produces a new frame and assigns it to `rendered_frame`. To actually show
    /// the contents of the new [`Scene`], use [`Self::present`].
    #[profiling::function]
    pub fn draw(&mut self, cx: &mut App) -> ArenaClearNeeded {
        // Set up the per-App arena for element allocation during this draw.
        // This ensures that multiple test Apps have isolated arenas.
        let _arena_scope = ElementArenaScope::enter(&cx.element_arena);

        self.invalidate_entities();
        cx.entities.clear_accessed();
        debug_assert!(self.rendered_entity_stack.is_empty());
        debug_assert!(self.subtree_presentation_stack.borrow().is_empty());
        debug_assert!(self.subtree_transform_stack.borrow().is_empty());
        debug_assert!(self.scroll_ancestry_stack.borrow().is_empty());
        debug_assert!(!self.frame_focus_authority_sealed);
        debug_assert!(!self.focus_followup_requested);
        debug_assert!(self.sealed_focus_retry_rejection.is_none());
        self.invalidator.set_dirty(false);
        self.requested_autoscroll = None;
        self.next_frame.generation = self.rendered_frame.generation.saturating_add(1);
        self.promote_pending_blur_claim();

        // Restore the previously-used input handler.
        // Place it back into a None slot (left by a previous .take()) so that
        // cached paint_range indices in reuse_paint find the handler at the
        // expected position.
        if let Some(input_handler) = self.platform_window.take_input_handler() {
            let validity = input_handler.validity();
            if let Some(slot) = self
                .rendered_frame
                .input_handlers
                .iter_mut()
                .rev()
                .find(|output| output.value.is_none())
            {
                slot.value = Some(input_handler);
                slot.validity = validity;
            } else {
                self.rendered_frame
                    .input_handlers
                    .push(FrameOutput::new(Some(input_handler), validity));
            }
        }
        if !cx.mode.skip_drawing() {
            self.draw_roots(cx);
        }
        self.frame_focus_authority_sealed = true;
        debug_assert!(self.subtree_presentation_stack.borrow().is_empty());
        debug_assert!(self.subtree_transform_stack.borrow().is_empty());
        debug_assert!(self.scroll_ancestry_stack.borrow().is_empty());
        self.dirty_views.clear();
        self.next_frame.window_active = self.active.get();

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
        if let Some(input_handler) = self.select_frame_input_handler_after_composition_cleanup(
            &mut rendered_input_handlers,
            &mut next_input_handlers,
            cx,
        ) {
            self.platform_window.set_input_handler(input_handler);
        }

        self.layout_engine.as_mut().unwrap().clear();
        self.text_system().finish_frame();
        self.next_frame.finish(&mut self.rendered_frame);

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
        if let Some(owner) = stale_capture_owner.or(stale_drag_owner) {
            self.queue_pointer_session_cancellation(owner, PointerCancelReason::CaptureRevoked, cx);
        }

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

        // Publications must observe the input authority of the frame they publish.
        self.commit_prepaint(cx);
        self.sealed_focus_retry_rejection = None;
        self.discard_resolved_candidate_focus_claim();

        self.invalidator.set_phase(DrawPhase::Focus);
        let previous_committed_focus_path = self.rendered_frame.focus_path();
        let previous_window_active = self.rendered_frame.window_active;
        mem::swap(&mut self.rendered_frame, &mut self.next_frame);
        self.next_frame.clear();
        self.frame_focus_authority_sealed = false;
        self.mouse_hit_test = self.rendered_frame.hit_test(self.mouse_position);
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
        if mem::take(&mut self.focus_followup_requested) && self.focus_followup_frame_needed() {
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
    fn present(&mut self) {
        self.platform_window.draw(&self.rendered_frame.scene);
        #[cfg(feature = "input-latency-histogram")]
        self.input_latency_tracker.record_frame_presented();
        self.needs_present.set(false);
        profiling::finish_frame!();
    }

    /// Returns a snapshot of the current input-latency histograms.
    #[cfg(feature = "input-latency-histogram")]
    pub fn input_latency_snapshot(&self) -> InputLatencySnapshot {
        self.input_latency_tracker.snapshot()
    }

    fn draw_roots(&mut self, cx: &mut App) {
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

        let stale_drag_owner = cx.active_drag.as_ref().and_then(|drag| {
            (drag.window_id == self.handle.window_id())
                .then_some(drag.source)
                .flatten()
                .filter(|owner| {
                    self.pointer_capture_hitbox_for_handle_in_frame(*owner, &self.next_frame)
                        .is_none()
                })
        });
        if let Some(owner) = stale_drag_owner {
            self.queue_pointer_session_cancellation(owner, PointerCancelReason::CaptureRevoked, cx);
        }

        let mut active_drag_element = None;
        let mut tooltip_element = None;
        if prompt_element.is_none()
            && cx
                .active_drag
                .as_ref()
                .is_some_and(|drag| drag.window_id == self.handle.window_id())
        {
            let active_drag = cx
                .active_drag
                .take()
                .expect("window-owned active drag should remain available");
            let mut element = active_drag.view.clone().into_any();
            let offset = self.mouse_position() - active_drag.window_preview_offset;
            element.prepaint_as_root(offset, AvailableSpace::min_size(), self, cx);
            active_drag_element = Some(element);
            cx.active_drag = Some(active_drag);
        } else if prompt_element.is_none() {
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

        if let Some(mut prompt_element) = prompt_element {
            prompt_element.paint(self, cx);
        } else if let Some(mut drag_element) = active_drag_element {
            drag_element.paint(self, cx);
        } else if let Some(mut tooltip) = tooltip_element
            && tooltip
                .validity
                .as_ref()
                .is_none_or(SubtreeTransformValidity::is_valid)
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
        self.a11y.resolve_focus(self.focus);

        // a11y may have been activated/deactivated halfway through the frame
        let a11y_active_start_of_frame = self.a11y.is_active();
        let a11y_generation_start_of_frame = self.a11y.activation_generation();
        self.a11y.sync_active_flag();
        let a11y_active_end_of_frame = self.a11y.is_active();
        let a11y_generation_end_of_frame = self.a11y.activation_generation();

        let should_send_a11y_update = a11y_active_start_of_frame
            && a11y_active_end_of_frame
            && a11y_generation_start_of_frame == a11y_generation_end_of_frame;

        if a11y_active_start_of_frame {
            // clear the builder state regardless
            let tree_update = self.a11y.end_frame();

            if should_send_a11y_update {
                log::debug!(
                    "Sending a11y tree update: {} nodes",
                    tree_update.nodes.len()
                );
                self.a11y
                    .publish(&tree_update, a11y_generation_start_of_frame);
                self.platform_window.a11y_tree_update(tree_update);
            }
        }
    }

    fn commit_prepaint(&mut self, cx: &mut App) {
        let target_revision = self.next_frame.generation;
        let commits = self.next_frame.prepaint_commits.clone();
        let current_publications = commits
            .iter()
            .filter_map(|output| output.value.publication)
            .collect::<FxHashSet<_>>();
        let previous_commits = self.rendered_frame.prepaint_commits.clone();
        let mut expired_publications = FxHashSet::default();
        for output in previous_commits {
            let Some(publication) = output.value.publication else {
                continue;
            };
            if !output.is_valid()
                || current_publications.contains(&publication)
                || !expired_publications.insert(publication)
            {
                continue;
            }
            if let Some(discard) = output.value.discard {
                self.with_subtree_presentation(SubtreePresentation::Hidden, |window| {
                    discard(target_revision, window, cx)
                });
            }
        }
        for phase in [
            PrepaintCommitPhase::Normal,
            PrepaintCommitPhase::FocusStable,
        ] {
            for output in &commits {
                if output.value.phase != phase {
                    continue;
                }
                if output.is_valid() {
                    let presentation = output.value.presentation;
                    self.with_prepaint_commit_phase(phase, |window| {
                        window.with_subtree_presentation(presentation, |window| {
                            (output.value.commit)(target_revision, window, cx)
                        })
                    });
                } else if let Some(discard) = output.value.discard.clone() {
                    self.with_prepaint_commit_phase(phase, |window| {
                        window.with_subtree_presentation(SubtreePresentation::Hidden, |window| {
                            discard(target_revision, window, cx)
                        })
                    });
                }
            }
        }
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
                let subtree_transform_validity = deferred_draw.subtree_transform_validity.clone();
                let scroll_ancestry = deferred_draw.scroll_ancestry.clone();
                self.element_id_stack
                    .clone_from(&deferred_draw.element_id_stack);
                self.text_style_stack
                    .clone_from(&deferred_draw.text_style_stack);
                self.next_frame
                    .dispatch_tree
                    .set_active_node(deferred_draw.parent_node);

                let prepaint_start = self.prepaint_index();
                if subtree_transform_validity
                    .as_ref()
                    .is_some_and(|validity| !validity.is_valid())
                {
                    // The owning transform scope already failed elsewhere in this frame.
                } else if let Some(element) = deferred_draw.element.as_mut() {
                    let result = self.with_scroll_ancestry(scroll_ancestry, |window| {
                        window.transact_subtree_transform(
                            subtree_transform_validity.clone(),
                            |window| {
                                window.with_subtree_presentation(subtree_presentation, |window| {
                                    window.with_resolved_subtree_transform(
                                        subtree_transform,
                                        subtree_transform_validity.clone(),
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
                                                                window.with_resolved_content_mask(
                                                    deferred_draw.content_mask,
                                                    |window| {
                                                        window.with_accessibility_tree_scope(
                                                            accessibility_tree_scope,
                                                            |window| element.prepaint(window, cx),
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
                        && let Some(validity) = subtree_transform_validity.as_ref()
                    {
                        self.record_subtree_transform_scope_diagnostic(validity);
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

            let paint_start = self.paint_index();
            let content_mask = deferred_draw.content_mask;
            let subtree_presentation = deferred_draw.subtree_presentation;
            if deferred_draw
                .subtree_transform_validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
            {
                // The owning transform scope is layout-only for this frame.
            } else if let Some(element) = deferred_draw.element.as_mut() {
                self.with_subtree_presentation(subtree_presentation, |window| {
                    window.with_resolved_subtree_transform(
                        deferred_draw.subtree_transform,
                        deferred_draw.subtree_transform_validity.clone(),
                        |window| {
                            window.with_rendered_view(deferred_draw.current_view, |window| {
                                window.with_resolved_content_mask(content_mask, |window| {
                                    window.with_rem_size(Some(deferred_draw.rem_size), |window| {
                                        element.paint(window, cx);
                                    });
                                })
                            })
                        },
                    );
                });
                if let Some(validity) = deferred_draw.subtree_transform_validity.as_ref() {
                    self.record_subtree_transform_scope_diagnostic(validity);
                }
            } else {
                self.reuse_paint(deferred_draw.paint_range.clone());
            }
            let paint_end = self.paint_index();
            deferred_draw.paint_range = paint_start..paint_end;
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
        let validity = self.subtree_transform_validity();
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
                    SubtreeTransformValidity::replayed_under(
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
                    SubtreeTransformValidity::replayed_under(
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
                        SubtreeTransformValidity::replayed_under(
                            output.validity.as_ref(),
                            validity.clone(),
                        ),
                    )
                }),
        );
        self.next_frame.tooltip_requests.extend(
            self.rendered_frame.tooltip_requests
                [range.start.tooltips_index..range.end.tooltips_index]
                .iter_mut()
                .map(|request| {
                    request.take().map(|mut request| {
                        request.validity = SubtreeTransformValidity::replayed_under(
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
                SubtreeTransformValidity::replayed_under(recorded_validity, validity.clone()),
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
            &mut self.rendered_frame.dispatch_tree,
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
                    content_mask: deferred_draw.content_mask,
                    rem_size: deferred_draw.rem_size,
                    priority: deferred_draw.priority,
                    element: None,
                    absolute_offset: deferred_draw.absolute_offset,
                    subtree_presentation: deferred_draw.subtree_presentation,
                    subtree_transform: deferred_draw.subtree_transform,
                    subtree_transform_validity: SubtreeTransformValidity::replayed_under(
                        deferred_draw.subtree_transform_validity.as_ref(),
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
            atlas_access_diagnostics_index: self.next_frame.atlas_access_diagnostic_entries.len(),
            image_paint_diagnostics_index: self.next_frame.image_paint_diagnostic_entries.len(),
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

    pub(crate) fn reuse_paint(&mut self, range: Range<PaintIndex>) {
        let validity = self.subtree_transform_validity();
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
                    SubtreeTransformValidity::replayed_under(
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
                    SubtreeTransformValidity::replayed_under(
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
                    request.validity = SubtreeTransformValidity::replayed_under(
                        request.validity.as_ref(),
                        validity.clone(),
                    );
                    request
                }),
        );
        self.next_frame.input_handlers.extend(
            self.rendered_frame.input_handlers
                [range.start.input_handlers_index..range.end.input_handlers_index]
                .iter_mut()
                .map(|output| {
                    let mut handler = output.value.take();
                    let replayed_validity = SubtreeTransformValidity::replayed_under(
                        output.validity.as_ref(),
                        validity.clone(),
                    );
                    if let Some(handler) = handler.as_mut() {
                        handler.set_validity(replayed_validity.clone());
                    }
                    FrameOutput::new(handler, replayed_validity)
                }),
        );
        self.next_frame.mouse_listeners.extend(
            self.rendered_frame.mouse_listeners
                [range.start.mouse_listeners_index..range.end.mouse_listeners_index]
                .iter_mut()
                .map(|output| {
                    FrameOutput::new(
                        output.value.take(),
                        SubtreeTransformValidity::replayed_under(
                            output.validity.as_ref(),
                            validity.clone(),
                        ),
                    )
                }),
        );
        self.next_frame.pointer_cancel_listeners.extend(
            self.rendered_frame.pointer_cancel_listeners[range.start.pointer_cancel_listeners_index
                ..range.end.pointer_cancel_listeners_index]
                .iter_mut()
                .map(|output| {
                    FrameOutput::new(
                        output.value.clone(),
                        SubtreeTransformValidity::replayed_under(
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
                SubtreeTransformValidity::replayed_under(recorded_validity, validity.clone()),
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
                        SubtreeTransformValidity::replayed_under(
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
                        SubtreeTransformValidity::replayed_under(
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
            self.subtree_transform_validity(),
        );
        if let Err(error) = replay_result {
            self.record_subtree_transform_failure(error);
        }
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
            validity: self.subtree_transform_validity(),
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
            validity: self.subtree_transform_validity(),
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
            validity: self.subtree_transform_validity(),
        }));
        id
    }

    /// Invoke the given function with the given content mask after intersecting it
    /// with the current mask. This method should only be called during element drawing.
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_content_mask<R>(
        &mut self,
        mask: Option<ContentMask<Pixels>>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        if let Some(mask) = mask {
            let Ok(displayed_bounds) = self.try_project_subtree_bounds(mask.bounds) else {
                return f(self);
            };
            let mask = ContentMask {
                bounds: displayed_bounds,
            }
            .intersect(&self.content_mask());
            self.content_mask_stack.push(mask);
            let result = f(self);
            self.content_mask_stack.pop();
            result
        } else {
            f(self)
        }
    }

    fn with_resolved_content_mask<R>(
        &mut self,
        mask: ContentMask<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.content_mask_stack.push(mask);
        let result = f(self);
        self.content_mask_stack.pop();
        result
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
    ) -> (TooltipId, SubtreeTransformValidity) {
        let id = self.next_tooltip_id;
        let validity = self.new_subtree_transform_validity();
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

    pub(crate) fn transact_subtree_transform<T>(
        &mut self,
        validity: Option<SubtreeTransformValidity>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Result<T, SubtreeTransformError> {
        let accessed_element_states_index = self.next_frame.accessed_element_states.len();
        let mut invalid_element_states = Vec::new();
        let result = self.transact(|window| {
            let result = f(window);
            if let Some(error) = validity
                .as_ref()
                .and_then(SubtreeTransformValidity::failure)
            {
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
            // to `Frame::finish`, which owns disposal of state bound to a failed transform scope.
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
                validity: self.subtree_transform_validity(),
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

    pub(crate) fn new_subtree_transform_validity(&self) -> SubtreeTransformValidity {
        SubtreeTransformValidity::new(self.subtree_transform_validity())
    }

    pub(crate) fn subtree_transform_validity(&self) -> Option<SubtreeTransformValidity> {
        self.subtree_transform_stack
            .borrow()
            .last()
            .and_then(|scope| scope.validity.clone())
    }

    pub(crate) fn with_resolved_subtree_transform<R>(
        &mut self,
        transform: ResolvedSubtreeTransform,
        validity: Option<SubtreeTransformValidity>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        if self.current_prepaint_layout_id().is_some() {
            self.update_portal_anchor_transform(transform, validity.clone());
            self.update_reveal_target_transform(transform, validity.clone());
        }
        let stack = self.subtree_transform_stack.clone();
        let entered_depth = stack.borrow().len();
        let _a11y_validity = self.a11y.nodes.enter_transform_validity(validity.clone());
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
        self.invalidate_portal_anchor_capture();
        self.invalidate_reveal_target_capture();
        if let Some(scope) = self.subtree_transform_stack.borrow().last()
            && let Some(validity) = scope.validity.as_ref()
        {
            validity.invalidate(error);
        }
    }

    pub(crate) fn record_subtree_transform_scope_diagnostic(
        &mut self,
        validity: &SubtreeTransformValidity,
    ) {
        if let Some(error) = validity.take_unreported_failure() {
            self.record_subtree_transform_diagnostic(error);
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
    ) -> Result<(Bounds<Pixels>, accesskit::Rect), SubtreeTransformError> {
        let displayed = self.try_project_subtree_bounds(bounds)?;
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
        Ok((displayed, accesskit::Rect { x0, y0, x1, y1 }))
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

    fn insert_scene_primitive(&mut self, primitive: impl Into<Primitive>) {
        if let Err(error) = self
            .next_frame
            .scene
            .insert_primitive_scoped(primitive, self.subtree_transform_validity())
        {
            self.record_subtree_transform_failure(error);
        }
    }

    /// Obtain the current element opacity. This method should only be called during the
    /// prepaint phase of element drawing.
    #[inline]
    pub(crate) fn element_opacity(&self) -> f32 {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.element_opacity
    }

    /// Obtain the current content mask. This method should only be called during element drawing.
    pub fn content_mask(&self) -> ContentMask<Pixels> {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.content_mask_stack
            .last()
            .cloned()
            .unwrap_or_else(|| ContentMask {
                bounds: Bounds {
                    origin: Point::default(),
                    size: self.viewport_size,
                },
            })
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
            .insert(key.clone(), self.subtree_transform_validity());

        if let Some(any) = self
            .next_frame
            .element_states
            .remove(&key)
            .or_else(|| self.rendered_frame.element_states.remove(&key))
        {
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
    /// When `content_mask` is provided, it is resolved under the current geometry and intersected
    /// with the inherited clip. When `None`, the current effective clip is inherited unchanged.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn defer_draw(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
        content_mask: Option<ContentMask<Pixels>>,
    ) {
        self.invalidator.debug_assert_prepaint();
        let transform = self.subtree_transform();
        let validity = self.subtree_transform_validity();
        let content_mask = content_mask
            .and_then(|mask| {
                self.try_project_subtree_bounds(mask.bounds)
                    .ok()
                    .map(|bounds| ContentMask { bounds })
            })
            .map(|mask| mask.intersect(&self.content_mask()))
            .unwrap_or_else(|| self.content_mask());
        self.defer_draw_with_transform(
            element,
            absolute_offset,
            priority,
            content_mask,
            transform,
            validity,
            self.current_scroll_ancestry_for_deferred(),
        );
    }

    /// Defers an element at a deliberate window-space portal boundary.
    ///
    /// Unlike [`Self::defer_draw`], this resets inherited subtree geometry and clipping. Theme and
    /// presentation inheritance are unaffected; callers must project portal anchors and optional
    /// clip bounds into window space first. `None` restores the full viewport clip.
    pub fn defer_draw_in_window_space(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
        content_mask: Option<ContentMask<Pixels>>,
    ) {
        self.invalidator.debug_assert_prepaint();
        let content_mask = self.window_portal_content_mask(content_mask);
        self.defer_draw_with_transform(
            element,
            absolute_offset,
            priority,
            content_mask,
            ResolvedSubtreeTransform::IDENTITY,
            self.subtree_transform_validity(),
            SmallVec::new(),
        );
    }

    pub(crate) fn with_window_space_portal_prepaint<R>(
        &mut self,
        absolute_offset: Point<Pixels>,
        content_mask: Option<ContentMask<Pixels>>,
        validity: Option<SubtreeTransformValidity>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let content_mask = self.window_portal_content_mask(content_mask);
        self.with_scroll_ancestry(SmallVec::new(), |window| {
            window.with_resolved_subtree_transform(
                ResolvedSubtreeTransform::IDENTITY,
                validity,
                |window| {
                    window.with_absolute_element_offset(absolute_offset, |window| {
                        window.with_resolved_content_mask(content_mask, f)
                    })
                },
            )
        })
    }

    pub(crate) fn with_window_space_portal_paint<R>(
        &mut self,
        content_mask: Option<ContentMask<Pixels>>,
        validity: Option<SubtreeTransformValidity>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint();
        let content_mask = self.window_portal_content_mask(content_mask);
        self.with_resolved_subtree_transform(
            ResolvedSubtreeTransform::IDENTITY,
            validity,
            |window| window.with_resolved_content_mask(content_mask, f),
        )
    }

    fn window_portal_content_mask(
        &self,
        content_mask: Option<ContentMask<Pixels>>,
    ) -> ContentMask<Pixels> {
        let viewport_mask = ContentMask {
            bounds: Bounds::new(Point::default(), self.viewport_size),
        };
        content_mask
            .map(|mask| mask.intersect(&viewport_mask))
            .unwrap_or(viewport_mask)
    }

    fn defer_draw_with_transform(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
        content_mask: ContentMask<Pixels>,
        subtree_transform: ResolvedSubtreeTransform,
        subtree_transform_validity: Option<SubtreeTransformValidity>,
        scroll_ancestry: SmallVec<[ScrollContainerBinding; 8]>,
    ) {
        let parent_node = self.next_frame.dispatch_tree.active_node_id().unwrap();
        self.next_frame.deferred_draws.push(DeferredDraw {
            current_view: self.current_view(),
            parent_node,
            element_id_stack: self.element_id_stack.clone(),
            text_style_stack: self.text_style_stack.clone(),
            accessibility_tree_scope: self.a11y.current_tree_scope(),
            content_mask,
            rem_size: self.rem_size(),
            priority,
            element: Some(element),
            absolute_offset,
            subtree_presentation: self.subtree_presentation(),
            subtree_transform,
            subtree_transform_validity,
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

        let content_mask = self.content_mask();
        let clipped_bounds = self
            .try_project_subtree_bounds(bounds)
            .ok()
            .map(|bounds| bounds.intersect(&content_mask.bounds));
        if let Some(clipped_bounds) = clipped_bounds.filter(|bounds| !bounds.is_empty()) {
            self.next_frame.scene.push_layer_scoped(
                self.cover_bounds(clipped_bounds),
                self.subtree_transform_validity(),
            );
        }

        let result = f(self);

        if clipped_bounds.is_some_and(|bounds| !bounds.is_empty()) {
            self.next_frame
                .scene
                .pop_layer_scoped(self.subtree_transform_validity());
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
        let content_mask = self.snapped_content_mask();
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
                content_mask,
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
        let content_mask = self.snapped_content_mask();
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
                content_mask,
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
            content_mask: self.snapped_content_mask(),
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
        let content_mask = self.content_mask();
        let opacity = self.element_opacity();
        let Some(transform) = self.base_primitive_transform() else {
            return;
        };
        path.content_mask = content_mask;
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
            content_mask: self.snapped_content_mask(),
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
            content_mask: self.snapped_content_mask(),
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
            let content_mask = self.snapped_content_mask();

            if subpixel_rendering {
                self.insert_scene_primitive(SubpixelSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask,
                    color: color.opacity(element_opacity),
                    tile,
                    transform,
                });
            } else {
                self.insert_scene_primitive(MonochromeSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask,
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
            let content_mask = self.snapped_content_mask();
            let opacity = self.element_opacity();

            self.insert_scene_primitive(PolychromeSprite {
                order: 0,
                pad: 0,
                grayscale: false,
                bounds,
                corner_radii: Default::default(),
                content_mask,
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
        let content_mask = self.snapped_content_mask();
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
            content_mask,
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
        let content_mask = self.snapped_content_mask();
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
        self.insert_scene_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale,
            bounds,
            content_mask,
            corner_radii,
            tile,
            opacity,
            transform,
        });
        let validity = self.subtree_transform_validity();
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
        let content_mask = self.snapped_content_mask();
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
            content_mask,
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
                commit: Rc::new(commit),
                discard: None,
            },
            self.subtree_transform_validity(),
        ));
    }

    /// Records a validity-gated, cross-frame publication transaction.
    ///
    /// The commit callback runs after painting when the current transform stack is valid. The
    /// discard callback runs instead when painting proves that the recorded subtree geometry is
    /// invalid. It also runs when a valid publication from the previous frame is absent from the
    /// current frame, including when an enclosing [`Self::transact`] rolls back or an ancestor
    /// transform prevents this subtree from prepainting.
    ///
    /// Use one stable [`PrepaintPublicationId`] for each logical publication and record it at most
    /// once per frame. Cached subtrees retain both the ID and callbacks in their frame journal.
    /// Valid commits run under their captured presentation state. Discards run suppressed because
    /// their producer has no interactive authority in the committed frame.
    pub fn record_prepaint_window_transaction(
        &mut self,
        publication: PrepaintPublicationId,
        commit: impl Fn(u64, &mut Window, &mut App) + 'static,
        discard: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_prepaint();
        let commit: Rc<dyn Fn(u64, &mut Window, &mut App)> = Rc::new(commit);
        let discard: Rc<dyn Fn(u64, &mut Window, &mut App)> = Rc::new(discard);
        self.next_frame.prepaint_commits.push(FrameOutput::new(
            PrepaintCommit {
                phase: PrepaintCommitPhase::Normal,
                publication: Some(publication),
                presentation: self.subtree_presentation(),
                commit,
                discard: Some(discard),
            },
            self.subtree_transform_validity(),
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
                commit: Rc::new(commit),
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

    /// This method should be called during `prepaint`. You can use
    /// the returned [Hitbox] during `paint` or in an event handler
    /// to determine whether the inserted hitbox was the topmost.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn insert_hitbox(&mut self, bounds: Bounds<Pixels>, behavior: HitboxBehavior) -> Hitbox {
        self.invalidator.debug_assert_prepaint();

        let content_mask = self.content_mask();
        let transform = self.subtree_transform();
        let validity = self.subtree_transform_validity();
        let geometry = self.try_element_geometry(bounds);
        let active = geometry.is_ok() && self.subtree_presentation().is_interactive();
        let mut id = self.next_hitbox_id;
        self.next_hitbox_id = self.next_hitbox_id.next();
        let hitbox = Hitbox {
            id,
            geometry: geometry.unwrap_or_else(|_| {
                ElementGeometry::from_resolved(bounds, Bounds::default(), transform)
            }),
            validity,
            content_mask,
            behavior,
            active,
        };
        if active {
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
            self.subtree_transform_validity(),
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
            self.subtree_transform_validity(),
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
            let validity = self.subtree_transform_validity();
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
            self.subtree_transform_validity(),
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
                self.subtree_transform_validity(),
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
        if self.flush_pending_pointer_cancellation(cx) && incoming_pointer_cancel {
            return self
                .last_dispatch_event_result
                .unwrap_or(DispatchEventResult {
                    propagate: true,
                    default_prevented: false,
                });
        }
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

    fn dispatch_mouse_event(&mut self, event: &dyn Any, cx: &mut App) {
        if let Some(event) = event.downcast_ref::<crate::MouseDownEvent>() {
            self.pressed_mouse_buttons.insert(event.button);
        }
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

        let routes_to_captured_target = event.is::<crate::MouseDownEvent>()
            || event.is::<MouseUpEvent>()
            || event.is::<MouseMoveEvent>()
            || event.is::<crate::MousePressureEvent>()
            || is_pointer_cancel;
        let _captured_target = routes_to_captured_target
            .then(|| self.captured_pointer_hitbox())
            .flatten()
            .map(|hitbox| MouseEventTargetGuard::enter(self.mouse_event_target.clone(), hitbox));

        #[cfg(any(feature = "inspector", debug_assertions))]
        if !is_pointer_cancel && self.is_inspector_picking(cx) {
            self.handle_inspector_mouse_event(event, cx);
            // When inspector is picking, all other mouse handling is skipped.
            self.finish_mouse_session_event(event, cx);
            return;
        }

        if let Some(event) = WindowMouseEvent::from_any(event) {
            self.mouse_interceptors.clone().retain(&(), |interceptor| {
                if is_pointer_cancel || cx.propagate_event {
                    interceptor(event, self, cx)
                } else {
                    true
                }
            });
        }

        if let Some(event) = event.downcast_ref::<PointerCancelEvent>() {
            let mut listeners = mem::take(&mut self.rendered_frame.pointer_cancel_listeners);
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
            self.rendered_frame.pointer_cancel_listeners = listeners;
        } else {
            let mut listeners = mem::take(&mut self.rendered_frame.mouse_listeners);

            // Capture phase, events bubble from back to front. Handlers for this phase are used for
            // special purposes, such as detecting events outside of a given Bounds.
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

            self.rendered_frame.mouse_listeners = listeners;
        }

        self.finish_mouse_session_event(event, cx);
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
                    cx.update(move |window, cx| {
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
        self.pending_input.take();
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
        self.platform_window.activate();
    }

    /// Minimize the current window at the platform level.
    pub fn minimize_window(&self) {
        self.platform_window.minimize();
    }

    /// Toggle full screen status on the current window at the platform level.
    pub fn toggle_fullscreen(&self) {
        self.platform_window.toggle_fullscreen();
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
        &self,
        cx: &App,
        f: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) {
        let mut cx = self.to_async(cx);
        self.platform_window.on_should_close(Box::new(move || {
            cx.update(|window, cx| f(window, cx)).unwrap_or(true)
        }))
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
    ) {
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
            return;
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
                return;
            }
        }

        // Fall back to built-in action handling.
        match request.action {
            accesskit::Action::Click => {
                if let Some(bounds) = self.a11y.published_node_bounds(request.target_node) {
                    let center = bounds.center();
                    let mouse_down = PlatformInput::MouseDown(crate::MouseDownEvent {
                        button: MouseButton::Left,
                        position: center,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    });
                    let mouse_up = PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: center,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    });
                    self.dispatch_event(mouse_down, cx);
                    if self.removal_state != WindowRemovalState::Open {
                        return;
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
