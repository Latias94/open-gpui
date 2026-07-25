use crate::{
    App, Bounds, ElementGeometry, FocusId, LayoutId, ListState, Overflow, Pixels, Point,
    ScrollHandle, Subscription, SubtreePresentation, Window, WindowId,
    geometry::{ResolvedSubtreeTransform, SubtreeGeometryValidity},
    point,
};
use open_gpui_collections::FxHashSet;
use open_gpui_motion::{MotionProgressRun, MotionTransition};
use open_gpui_scheduler::Instant;
use smallvec::SmallVec;
use std::{cell::RefCell, mem, rc::Rc, time::Duration};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) struct RevealTargetId(u64);

impl RevealTargetId {
    fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("reveal target handle space exhausted"),
        )
    }
}

/// A stable, window-owned capability identifying one physical reveal target.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RevealTargetHandle {
    window_id: WindowId,
    id: RevealTargetId,
}

impl RevealTargetHandle {
    /// Returns the window that created this handle.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }
}

/// A one-use capability for atomically submitting a deferred physical reveal.
///
/// Capture this from prepaint within the intended final scroll ancestry as soon as a logical
/// adapter has materialized the target. The target does not need to be bound yet. Submission
/// validates that the same target chain is later bound and that direct scrolling has not
/// intervened before the request enters window authority.
#[must_use = "a deferred bring-into-view guard must be submitted or deliberately discarded"]
pub struct DeferredBringIntoViewGuard {
    target: RevealTargetHandle,
    fence: ScrollChainFence,
}

impl std::fmt::Debug for DeferredBringIntoViewGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredBringIntoViewGuard")
            .field("target", &self.target)
            .field("options", &self.fence.options)
            .finish_non_exhaustive()
    }
}

impl DeferredBringIntoViewGuard {
    /// Returns a retained interruption fence for the same scroll chain and requested axes.
    ///
    /// The returned fence cannot submit a reveal. It is intended for adapters that must reject a
    /// stale geometry retry after this one-use guard has entered window authority.
    pub fn scroll_chain_fence(&self) -> ScrollChainFence {
        self.fence.clone()
    }
}

/// Opaque snapshot of one committed or prepainted scroll chain and its direct-scroll revisions.
///
/// A fence observes interruption only. It cannot enqueue a reveal or expose container geometry.
/// Virtual adapters capture it at an input boundary and check it again before performing their own
/// logical materialization or retrying a completed physical request.
#[must_use = "a scroll-chain fence must be retained until the pending adapter operation settles"]
#[derive(Clone)]
pub struct ScrollChainFence {
    window_id: WindowId,
    options: BringIntoViewOptions,
    expected_chain: ScrollChainExpectation,
}

impl std::fmt::Debug for ScrollChainFence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScrollChainFence")
            .field("window_id", &self.window_id)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

/// An error produced while binding a reveal target.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RevealTargetError {
    /// The handle was used with a different window.
    #[error("reveal target belongs to window {handle_window:?}, not window {target_window:?}")]
    WrongWindow {
        /// The window that created the handle.
        handle_window: WindowId,
        /// The window on which the operation was attempted.
        target_window: WindowId,
    },
    /// The handle claimed more than one physical target in a candidate frame.
    #[error("reveal target {handle:?} is already bound in the current frame")]
    HandleAlreadyBound {
        /// The duplicate handle.
        handle: RevealTargetHandle,
    },
}

/// Physical alignment of a reveal target within one scroll viewport.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BringIntoViewAlignment {
    /// Move the target only when needed, using the nearest physical edge.
    #[default]
    Nearest,
    /// Align the target's minimum physical edge: left or top.
    MinEdge,
    /// Center the target on the physical axis.
    Center,
    /// Align the target's maximum physical edge: right or bottom.
    MaxEdge,
}

/// Policy for one physical axis of a bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BringIntoViewAxis {
    /// Preserve this axis exactly.
    Preserve,
    /// Reveal and align on this axis.
    Align(BringIntoViewAlignment),
}

impl Default for BringIntoViewAxis {
    fn default() -> Self {
        Self::Align(BringIntoViewAlignment::Nearest)
    }
}

/// Checked physical margins reserved around a reveal target.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BringIntoViewMargins {
    left: Pixels,
    right: Pixels,
    top: Pixels,
    bottom: Pixels,
}

impl BringIntoViewMargins {
    /// No reserved margin.
    pub const ZERO: Self = Self {
        left: Pixels::ZERO,
        right: Pixels::ZERO,
        top: Pixels::ZERO,
        bottom: Pixels::ZERO,
    };

    /// Creates non-negative finite physical margins.
    pub fn try_new(
        left: Pixels,
        right: Pixels,
        top: Pixels,
        bottom: Pixels,
    ) -> Result<Self, BringIntoViewMarginsError> {
        for (edge, value) in [
            ("left", left),
            ("right", right),
            ("top", top),
            ("bottom", bottom),
        ] {
            if !value.0.is_finite() || value < Pixels::ZERO {
                return Err(BringIntoViewMarginsError { edge, value });
            }
        }
        Ok(Self {
            left,
            right,
            top,
            bottom,
        })
    }

    /// Returns the left margin.
    pub const fn left(self) -> Pixels {
        self.left
    }

    /// Returns the right margin.
    pub const fn right(self) -> Pixels {
        self.right
    }

    /// Returns the top margin.
    pub const fn top(self) -> Pixels {
        self.top
    }

    /// Returns the bottom margin.
    pub const fn bottom(self) -> Pixels {
        self.bottom
    }
}

/// Invalid physical margin supplied to [`BringIntoViewMargins::try_new`].
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[error("bring-into-view {edge} margin must be finite and non-negative, got {value:?}")]
pub struct BringIntoViewMarginsError {
    edge: &'static str,
    value: Pixels,
}

/// Timing behavior of a bring-into-view request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BringIntoViewBehavior {
    /// Commit each required scroll offset immediately.
    Instant,
    /// Sample the supplied renderer-neutral transition for each container.
    Animated(MotionTransition),
}

impl Default for BringIntoViewBehavior {
    fn default() -> Self {
        Self::Instant
    }
}

/// Complete physical-axis policy for one bring-into-view request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BringIntoViewOptions {
    horizontal: BringIntoViewAxis,
    vertical: BringIntoViewAxis,
    margins: BringIntoViewMargins,
    behavior: BringIntoViewBehavior,
}

impl BringIntoViewOptions {
    /// Reveals both physical axes using nearest-edge alignment.
    pub const fn nearest() -> Self {
        Self {
            horizontal: BringIntoViewAxis::Align(BringIntoViewAlignment::Nearest),
            vertical: BringIntoViewAxis::Align(BringIntoViewAlignment::Nearest),
            margins: BringIntoViewMargins::ZERO,
            behavior: BringIntoViewBehavior::Instant,
        }
    }

    /// Reveals both physical axes with the same alignment.
    pub const fn aligned(alignment: BringIntoViewAlignment) -> Self {
        Self {
            horizontal: BringIntoViewAxis::Align(alignment),
            vertical: BringIntoViewAxis::Align(alignment),
            margins: BringIntoViewMargins::ZERO,
            behavior: BringIntoViewBehavior::Instant,
        }
    }

    /// Reveals only the physical vertical axis.
    pub const fn vertical(alignment: BringIntoViewAlignment) -> Self {
        Self {
            horizontal: BringIntoViewAxis::Preserve,
            vertical: BringIntoViewAxis::Align(alignment),
            margins: BringIntoViewMargins::ZERO,
            behavior: BringIntoViewBehavior::Instant,
        }
    }

    /// Reveals only the physical horizontal axis.
    pub const fn horizontal(alignment: BringIntoViewAlignment) -> Self {
        Self {
            horizontal: BringIntoViewAxis::Align(alignment),
            vertical: BringIntoViewAxis::Preserve,
            margins: BringIntoViewMargins::ZERO,
            behavior: BringIntoViewBehavior::Instant,
        }
    }

    /// Replaces the horizontal physical-axis policy.
    pub const fn with_horizontal(mut self, horizontal: BringIntoViewAxis) -> Self {
        self.horizontal = horizontal;
        self
    }

    /// Replaces the vertical physical-axis policy.
    pub const fn with_vertical(mut self, vertical: BringIntoViewAxis) -> Self {
        self.vertical = vertical;
        self
    }

    /// Replaces the checked physical margins.
    pub const fn with_margins(mut self, margins: BringIntoViewMargins) -> Self {
        self.margins = margins;
        self
    }

    /// Replaces the timing behavior.
    pub const fn with_behavior(mut self, behavior: BringIntoViewBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    /// Returns the horizontal physical-axis policy.
    pub const fn horizontal_axis(self) -> BringIntoViewAxis {
        self.horizontal
    }

    /// Returns the vertical physical-axis policy.
    pub const fn vertical_axis(self) -> BringIntoViewAxis {
        self.vertical
    }

    /// Returns the physical margins.
    pub const fn margins(self) -> BringIntoViewMargins {
        self.margins
    }

    /// Returns the timing behavior.
    pub const fn behavior(self) -> BringIntoViewBehavior {
        self.behavior
    }
}

impl Default for BringIntoViewOptions {
    fn default() -> Self {
        Self::nearest()
    }
}

/// Opaque ancestry generation claimed by a bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BringIntoViewChainGeneration(u64);

/// Stable identity of one window-owned bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BringIntoViewRequestId {
    window_id: WindowId,
    sequence: u64,
    chain_generation: BringIntoViewChainGeneration,
}

impl BringIntoViewRequestId {
    /// Returns the owning window.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the monotonic window-local request sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Returns the ancestry generation claimed by this request.
    pub const fn chain_generation(self) -> BringIntoViewChainGeneration {
        self.chain_generation
    }
}

impl BringIntoViewChainGeneration {
    /// Returns the opaque generation as a diagnostic scalar.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Successful terminal state of a bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BringIntoViewCompletion {
    /// Every requested axis was already visible.
    AlreadyVisible,
    /// At least one scroll container committed a new offset.
    Revealed,
}

/// Deterministic cancellation reason for a bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BringIntoViewCancelReason {
    /// A newer request claimed an overlapping scroll ancestry.
    Superseded,
    /// Direct wheel, scrollbar, keyboard, touch, or programmatic scrolling intervened.
    ScrollOverridden,
    /// The target no longer has a committed physical binding.
    TargetUnlinked,
    /// The target is under a hidden or inert presentation scope.
    TargetSuppressed,
    /// The committed scroll ancestry changed while the request was active.
    AncestryChanged,
    /// A required scroll delta could not change the owning container.
    NoProgress,
    /// The owning window closed.
    WindowClosed,
}

/// Terminal outcome observed for a bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BringIntoViewOutcome {
    /// The request completed successfully.
    Completed(BringIntoViewCompletion),
    /// The request was cancelled before completion.
    Cancelled(BringIntoViewCancelReason),
}

/// Synchronous rejection of a bring-into-view request.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BringIntoViewError {
    /// The target belongs to a different window.
    #[error("reveal target belongs to window {handle_window:?}, not window {target_window:?}")]
    WrongWindow {
        /// The window that created the handle.
        handle_window: WindowId,
        /// The window receiving the request.
        target_window: WindowId,
    },
    /// The window has begun closing.
    #[error("the target window is closing or closed")]
    WindowClosed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum RevealTargetKey {
    Public(RevealTargetId),
    Focus(FocusId),
    Accessibility(accesskit::NodeId),
}

#[derive(Clone)]
pub(super) struct RevealTargetBinding {
    key: RevealTargetKey,
    state: RevealTargetBindingState,
}

#[derive(Clone)]
enum RevealTargetBindingState {
    Linked(RevealTargetSnapshot),
    Suppressed,
    Unavailable,
}

#[derive(Clone)]
struct RevealTargetSnapshot {
    frame_generation: u64,
    geometry: ElementGeometry,
    ancestry: SmallVec<[ScrollContainerBinding; 4]>,
}

impl RevealTargetBinding {
    pub(super) const fn key(&self) -> RevealTargetKey {
        self.key
    }

    pub(super) fn replayed(&self, frame_generation: u64) -> Self {
        let mut binding = self.clone();
        if let RevealTargetBindingState::Linked(snapshot) = &mut binding.state {
            snapshot.frame_generation = frame_generation;
        }
        binding
    }
}

/// Opaque revision of explicit scroll mutations on a tracked scroll container.
///
/// Bring-into-view adapters may retain this value across a deferred materialization boundary and
/// stop if the user or application scrolls before the physical request is submitted. Authority
/// driven bring-into-view motion does not advance this revision.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollDirectMutationRevision {
    pub(crate) horizontal: u64,
    pub(crate) vertical: u64,
}

impl ScrollDirectMutationRevision {
    /// Returns whether the horizontal direct-scroll intent changed after `earlier` was captured.
    pub const fn horizontal_changed_since(self, earlier: Self) -> bool {
        self.horizontal != earlier.horizontal
    }

    /// Returns whether the vertical direct-scroll intent changed after `earlier` was captured.
    pub const fn vertical_changed_since(self, earlier: Self) -> bool {
        self.vertical != earlier.vertical
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ScrollContainerId(usize);

#[derive(Clone)]
enum ScrollContainerDriver {
    Handle(ScrollHandle),
    List(ListState),
}

impl ScrollContainerDriver {
    fn id(&self) -> ScrollContainerId {
        match self {
            Self::Handle(handle) => ScrollContainerId(handle.scroll_container_identity()),
            Self::List(state) => ScrollContainerId(state.scroll_container_identity()),
        }
    }

    fn offset(&self) -> Point<Pixels> {
        match self {
            Self::Handle(handle) => handle.offset(),
            Self::List(state) => state.scroll_px_offset_for_scrollbar(),
        }
    }

    fn max_offset(&self) -> Point<Pixels> {
        match self {
            Self::Handle(handle) => handle.max_offset(),
            Self::List(state) => state.max_offset_for_scrollbar(),
        }
    }

    fn direct_revision(&self) -> ScrollDirectMutationRevision {
        match self {
            Self::Handle(handle) => handle.direct_scroll_revision(),
            Self::List(state) => state.direct_scroll_revision(),
        }
    }

    fn apply_authority_offset(&self, offset: Point<Pixels>, axes: ScrollAxes) -> bool {
        match self {
            Self::Handle(handle) => {
                handle.apply_bring_into_view_offset(offset, axes.horizontal, axes.vertical)
            }
            Self::List(state) => state.apply_bring_into_view_offset(offset, axes.vertical),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScrollAxes {
    horizontal: bool,
    vertical: bool,
}

impl ScrollAxes {
    fn requested(options: BringIntoViewOptions) -> Self {
        Self {
            horizontal: !matches!(options.horizontal, BringIntoViewAxis::Preserve),
            vertical: !matches!(options.vertical, BringIntoViewAxis::Preserve),
        }
    }

    fn intersect(self, other: Self) -> Self {
        Self {
            horizontal: self.horizontal && other.horizontal,
            vertical: self.vertical && other.vertical,
        }
    }

    fn any(self) -> bool {
        self.horizontal || self.vertical
    }
}

#[derive(Clone)]
pub(crate) struct ScrollContainerBinding {
    id: ScrollContainerId,
    geometry: ElementGeometry,
    visible_bounds: Bounds<Pixels>,
    axes: ScrollAxes,
    driver: ScrollContainerDriver,
}

impl ScrollContainerBinding {
    fn for_handle(
        handle: ScrollHandle,
        geometry: ElementGeometry,
        visible_bounds: Bounds<Pixels>,
        overflow: Point<Overflow>,
    ) -> Self {
        let driver = ScrollContainerDriver::Handle(handle);
        Self {
            id: driver.id(),
            geometry,
            visible_bounds,
            axes: ScrollAxes {
                horizontal: overflow.x == Overflow::Scroll,
                vertical: overflow.y == Overflow::Scroll,
            },
            driver,
        }
    }

    fn for_list(
        state: ListState,
        geometry: ElementGeometry,
        visible_bounds: Bounds<Pixels>,
    ) -> Self {
        let driver = ScrollContainerDriver::List(state);
        Self {
            id: driver.id(),
            geometry,
            visible_bounds,
            axes: ScrollAxes {
                horizontal: false,
                vertical: true,
            },
            driver,
        }
    }
}

#[derive(Clone)]
struct ScrollChainExpectation {
    entries: SmallVec<[ScrollChainExpectationEntry; 4]>,
}

#[derive(Clone)]
struct ScrollChainExpectationEntry {
    id: ScrollContainerId,
    axes: ScrollAxes,
    driver: ScrollContainerDriver,
    direct_revision: ScrollDirectMutationRevision,
}

impl ScrollChainExpectation {
    fn capture(snapshot: &RevealTargetSnapshot) -> Self {
        Self {
            entries: snapshot.ancestry.iter().map(Self::entry).collect(),
        }
    }

    fn capture_current_ancestry(ancestry: &[ScrollContainerBinding]) -> Self {
        Self {
            entries: ancestry.iter().rev().map(Self::entry).collect(),
        }
    }

    fn entry(container: &ScrollContainerBinding) -> ScrollChainExpectationEntry {
        ScrollChainExpectationEntry {
            id: container.id,
            axes: container.axes,
            driver: container.driver.clone(),
            direct_revision: container.driver.direct_revision(),
        }
    }

    fn matches_chain(&self, snapshot: &RevealTargetSnapshot) -> bool {
        self.entries.len() == snapshot.ancestry.len()
            && self
                .entries
                .iter()
                .zip(&snapshot.ancestry)
                .all(|(expected, current)| {
                    expected.id == current.id && expected.axes == current.axes
                })
    }

    fn matches_current_ancestry(&self, ancestry: &[ScrollContainerBinding]) -> bool {
        self.entries.len() == ancestry.len()
            && self
                .entries
                .iter()
                .zip(ancestry.iter().rev())
                .all(|(expected, current)| {
                    expected.id == current.id && expected.axes == current.axes
                })
    }

    fn direct_scroll_was_overridden(&self, requested_axes: ScrollAxes) -> bool {
        self.entries.iter().any(|entry| {
            let axes = entry.axes.intersect(requested_axes);
            let current = entry.driver.direct_revision();
            (axes.horizontal && current.horizontal_changed_since(entry.direct_revision))
                || (axes.vertical && current.vertical_changed_since(entry.direct_revision))
        })
    }
}

pub(super) struct RevealTargetCapture {
    handle: RevealTargetHandle,
    root_layout_ids: SmallVec<[LayoutId; 2]>,
    layout_bounds: Bounds<Pixels>,
    transform: ResolvedSubtreeTransform,
    validity: Option<SubtreeGeometryValidity>,
    presentation: SubtreePresentation,
    ancestry: SmallVec<[ScrollContainerBinding; 4]>,
    failed: bool,
}

impl RevealTargetCapture {
    fn new(
        handle: RevealTargetHandle,
        layout_id: LayoutId,
        layout_bounds: Bounds<Pixels>,
        transform: ResolvedSubtreeTransform,
        validity: Option<SubtreeGeometryValidity>,
        presentation: SubtreePresentation,
        ancestry: SmallVec<[ScrollContainerBinding; 4]>,
    ) -> Self {
        let mut root_layout_ids = SmallVec::new();
        root_layout_ids.push(layout_id);
        Self {
            handle,
            root_layout_ids,
            layout_bounds,
            transform,
            validity,
            presentation,
            ancestry,
            failed: false,
        }
    }

    fn contains_root_layout(&self, layout_id: LayoutId) -> bool {
        self.root_layout_ids.contains(&layout_id)
    }

    fn add_root_layout_alias(&mut self, layout_id: LayoutId) {
        if !self.contains_root_layout(layout_id) {
            self.root_layout_ids.push(layout_id);
        }
    }
}

struct RevealTargetCaptureGuard {
    stack: Rc<RefCell<Vec<RevealTargetCapture>>>,
    entered_depth: usize,
    armed: bool,
}

impl Drop for RevealTargetCaptureGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert_eq!(stack.len(), self.entered_depth + 1);
        }
        stack.truncate(self.entered_depth);
    }
}

struct ScrollAncestryGuard {
    stack: Rc<RefCell<SmallVec<[ScrollContainerBinding; 8]>>>,
    entered_depth: usize,
}

impl Drop for ScrollAncestryGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert!(stack.len() >= self.entered_depth);
        }
        stack.truncate(self.entered_depth);
    }
}

type BringIntoViewCallback = Box<dyn FnOnce(BringIntoViewOutcome, &mut Window, &mut App) + 'static>;
type SharedBringIntoViewCallback = Rc<RefCell<Option<BringIntoViewCallback>>>;

pub(super) struct ActiveBringIntoViewRequest {
    id: BringIntoViewRequestId,
    key: RevealTargetKey,
    source: BringIntoViewRequestSource,
    options: BringIntoViewOptions,
    eligible_after_generation: u64,
    expected_chain: Option<ScrollChainExpectation>,
    moved: bool,
    last_advanced_generation: Option<u64>,
    motion: Option<ActiveRevealMotion>,
    callback: Option<SharedBringIntoViewCallback>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BringIntoViewRequestSource {
    Application,
    Focus(FocusId),
    Accessibility {
        node: accesskit::NodeId,
        activation_generation: u64,
    },
}

struct ActiveRevealMotion {
    container_id: ScrollContainerId,
    axes: ScrollAxes,
    from: Point<Pixels>,
    target: Point<Pixels>,
    started_at: Instant,
    progress: MotionProgressRun,
}

pub(super) struct BringIntoViewResolution {
    outcome: BringIntoViewOutcome,
    callback: SharedBringIntoViewCallback,
}

impl Window {
    /// Creates a stable reveal target owned by this window.
    pub fn new_reveal_target(&mut self) -> RevealTargetHandle {
        let id = self.next_reveal_target_id;
        self.next_reveal_target_id = id.next();
        RevealTargetHandle {
            window_id: self.handle.window_id(),
            id,
        }
    }

    /// Binds a reveal target directly to final prepaint bounds.
    ///
    /// Custom elements may call this once per candidate frame after their root geometry is final.
    /// Most callers should use [`crate::RevealTargetExt::track_reveal_target`].
    pub fn bind_reveal_target(
        &mut self,
        handle: &RevealTargetHandle,
        bounds: Bounds<Pixels>,
    ) -> Result<(), RevealTargetError> {
        self.invalidator.debug_assert_prepaint();
        self.ensure_reveal_target_window(handle)?;
        let key = RevealTargetKey::Public(handle.id);
        if self.next_frame.has_reveal_target_binding(key) {
            return Err(RevealTargetError::HandleAlreadyBound { handle: *handle });
        }
        self.record_reveal_target_binding(key, bounds);
        Ok(())
    }

    /// Requests physical reveal of a committed target.
    pub fn bring_into_view(
        &mut self,
        handle: &RevealTargetHandle,
        options: BringIntoViewOptions,
        cx: &mut App,
    ) -> Result<BringIntoViewRequestId, BringIntoViewError> {
        self.enqueue_bring_into_view(
            RevealTargetKey::Public(handle.id),
            handle.window_id,
            BringIntoViewRequestSource::Application,
            options,
            None,
            self.rendered_frame.generation.saturating_add(1),
            cx,
        )
    }

    /// Requests reveal and observes the terminal outcome of this exact request.
    ///
    /// Dropping the returned subscription cancels callback observation without cancelling the
    /// request itself.
    pub fn bring_into_view_with_completion(
        &mut self,
        handle: &RevealTargetHandle,
        options: BringIntoViewOptions,
        cx: &mut App,
        listener: impl FnOnce(BringIntoViewOutcome, &mut Window, &mut App) + 'static,
    ) -> Result<(BringIntoViewRequestId, Subscription), BringIntoViewError> {
        let callback = Rc::new(RefCell::new(Some(
            Box::new(listener) as BringIntoViewCallback
        )));
        let cancelled_callback = Rc::downgrade(&callback);
        let subscription = Subscription::new(move || {
            if let Some(callback) = cancelled_callback.upgrade() {
                callback.borrow_mut().take();
            }
        });
        let id = self.enqueue_bring_into_view(
            RevealTargetKey::Public(handle.id),
            handle.window_id,
            BringIntoViewRequestSource::Application,
            options,
            Some(callback),
            self.rendered_frame.generation.saturating_add(1),
            cx,
        )?;
        Ok((id, subscription))
    }

    /// Captures a fence from an anchor's committed scroll chain.
    ///
    /// Unlike [`Window::capture_current_scroll_chain_fence`], this method may be called from an
    /// input handler. `Ok(None)` means the anchor has no valid linked binding in the committed
    /// frame, so an adapter must fail closed rather than materialize against an unknown chain.
    pub fn capture_committed_scroll_chain_fence(
        &self,
        anchor: &RevealTargetHandle,
        options: BringIntoViewOptions,
    ) -> Result<Option<ScrollChainFence>, RevealTargetError> {
        self.ensure_reveal_target_window(anchor)?;
        let key = RevealTargetKey::Public(anchor.id);
        let Some(binding) = self
            .rendered_frame
            .reveal_target_binding(key)
            .filter(|binding| binding.is_valid())
        else {
            return Ok(None);
        };
        let RevealTargetBindingState::Linked(snapshot) = &binding.value.state else {
            return Ok(None);
        };
        Ok(Some(ScrollChainFence {
            window_id: self.handle.window_id(),
            options,
            expected_chain: ScrollChainExpectation::capture(snapshot),
        }))
    }

    /// Captures a fence from the scroll ancestry active at the current prepaint location.
    ///
    /// Logical adapters use this immediately after their own materialization to establish the
    /// next input boundary without requiring their final physical target to be bound yet.
    pub fn capture_current_scroll_chain_fence(
        &self,
        options: BringIntoViewOptions,
    ) -> ScrollChainFence {
        self.invalidator.debug_assert_prepaint();
        ScrollChainFence {
            window_id: self.handle.window_id(),
            options,
            expected_chain: ScrollChainExpectation::capture_current_ancestry(
                &self.current_scroll_ancestry(),
            ),
        }
    }

    /// Returns whether direct scrolling invalidated a scroll-chain fence.
    ///
    /// A fence from another window is always interrupted so adapters fail closed.
    pub fn scroll_chain_fence_was_interrupted(&self, fence: &ScrollChainFence) -> bool {
        fence.window_id != self.handle.window_id()
            || fence
                .expected_chain
                .direct_scroll_was_overridden(ScrollAxes::requested(fence.options))
    }

    /// Returns whether a fence still describes the scroll ancestry active at this prepaint site.
    ///
    /// The check includes container identity and available physical axes. It does not recapture
    /// direct-scroll revisions, so callers can reject topology drift without resetting an input
    /// boundary.
    pub fn scroll_chain_fence_matches_current_ancestry(&self, fence: &ScrollChainFence) -> bool {
        self.invalidator.debug_assert_prepaint();
        fence.window_id == self.handle.window_id()
            && fence
                .expected_chain
                .matches_current_ancestry(&self.current_scroll_ancestry())
    }

    /// Captures a scroll-chain guard for a deferred physical reveal.
    ///
    /// Call this during prepaint within the intended final scroll ancestry, after any logical
    /// materialization has positioned the target. The target may bind in a later frame;
    /// submission validates its binding and exact scroll chain before queuing a request.
    pub fn capture_deferred_bring_into_view_guard(
        &self,
        handle: &RevealTargetHandle,
        options: BringIntoViewOptions,
    ) -> Result<DeferredBringIntoViewGuard, RevealTargetError> {
        self.invalidator.debug_assert_prepaint();
        self.ensure_reveal_target_window(handle)?;
        Ok(DeferredBringIntoViewGuard {
            target: *handle,
            fence: self.capture_current_scroll_chain_fence(options),
        })
    }

    /// Returns whether direct scrolling invalidated a deferred reveal guard.
    ///
    /// A guard from another window is always treated as interrupted so adapters fail closed.
    pub fn deferred_bring_into_view_guard_was_interrupted(
        &self,
        guard: &DeferredBringIntoViewGuard,
    ) -> bool {
        guard.target.window_id != self.handle.window_id()
            || self.scroll_chain_fence_was_interrupted(&guard.fence)
    }

    /// Atomically validates and submits a deferred physical reveal.
    ///
    /// `Ok(None)` means that direct scrolling intervened, the target is no longer uniquely bound,
    /// or its scroll ancestry changed. In all of those cases no request enters window authority.
    pub fn try_bring_into_view_with_guard_and_completion(
        &mut self,
        guard: DeferredBringIntoViewGuard,
        cx: &mut App,
        listener: impl FnOnce(BringIntoViewOutcome, &mut Window, &mut App) + 'static,
    ) -> Result<Option<(BringIntoViewRequestId, Subscription)>, BringIntoViewError> {
        let target_window = self.handle.window_id();
        if guard.target.window_id != target_window {
            return Err(BringIntoViewError::WrongWindow {
                handle_window: guard.target.window_id,
                target_window,
            });
        }
        if self.deferred_bring_into_view_guard_was_interrupted(&guard) {
            return Ok(None);
        }

        let key = RevealTargetKey::Public(guard.target.id);
        let Some(binding) = self
            .rendered_frame
            .reveal_target_binding(key)
            .filter(|binding| binding.is_valid())
        else {
            return Ok(None);
        };
        let RevealTargetBindingState::Linked(snapshot) = &binding.value.state else {
            return Ok(None);
        };
        if !guard.fence.expected_chain.matches_chain(snapshot) {
            return Ok(None);
        }

        let callback = Rc::new(RefCell::new(Some(
            Box::new(listener) as BringIntoViewCallback
        )));
        let cancelled_callback = Rc::downgrade(&callback);
        let subscription = Subscription::new(move || {
            if let Some(callback) = cancelled_callback.upgrade() {
                callback.borrow_mut().take();
            }
        });
        let id = self.enqueue_bring_into_view(
            key,
            guard.target.window_id,
            BringIntoViewRequestSource::Application,
            guard.fence.options,
            Some(callback),
            self.rendered_frame.generation,
            cx,
        )?;
        Ok(Some((id, subscription)))
    }

    /// Returns the most recently allocated window bring-into-view authority generation.
    ///
    /// Retained virtual adapters use this as a conservative fence for a completed retry: any
    /// later request, even on an unrelated chain, prevents an older intent from re-entering
    /// authority after asynchronous completion dispatch.
    pub fn bring_into_view_authority_generation(&self) -> BringIntoViewChainGeneration {
        BringIntoViewChainGeneration(self.next_bring_into_view_chain_generation)
    }

    fn enqueue_bring_into_view(
        &mut self,
        key: RevealTargetKey,
        handle_window: WindowId,
        source: BringIntoViewRequestSource,
        options: BringIntoViewOptions,
        callback: Option<SharedBringIntoViewCallback>,
        eligible_after_generation: u64,
        cx: &mut App,
    ) -> Result<BringIntoViewRequestId, BringIntoViewError> {
        let target_window = self.handle.window_id();
        if handle_window != target_window {
            return Err(BringIntoViewError::WrongWindow {
                handle_window,
                target_window,
            });
        }
        if self.removal_state != super::WindowRemovalState::Open || self.removed {
            return Err(BringIntoViewError::WindowClosed);
        }

        self.next_bring_into_view_sequence = self
            .next_bring_into_view_sequence
            .checked_add(1)
            .expect("bring-into-view request sequence exhausted");
        self.next_bring_into_view_chain_generation = self
            .next_bring_into_view_chain_generation
            .checked_add(1)
            .expect("bring-into-view chain generation exhausted");
        let id = BringIntoViewRequestId {
            window_id: target_window,
            sequence: self.next_bring_into_view_sequence,
            chain_generation: BringIntoViewChainGeneration(
                self.next_bring_into_view_chain_generation,
            ),
        };
        let mut request = ActiveBringIntoViewRequest {
            id,
            key,
            source,
            options,
            eligible_after_generation,
            expected_chain: None,
            moved: false,
            last_advanced_generation: None,
            motion: None,
            callback,
        };
        if let Some(binding) = self.rendered_frame.reveal_target_binding(key).cloned()
            && let RevealTargetBindingState::Linked(snapshot) = binding.value.state
        {
            request.capture_chain_expectations(&snapshot);
        }
        self.active_bring_into_view_requests.push(request);
        self.refresh();
        self.schedule_bring_into_view_resolution_dispatch(cx);
        Ok(id)
    }

    pub(super) fn enqueue_focus_bring_into_view(
        &mut self,
        focus: FocusId,
        fence: Option<ScrollChainFence>,
        cx: &mut App,
    ) {
        let key = RevealTargetKey::Focus(focus);
        let Some(binding) = self
            .rendered_frame
            .reveal_target_binding(key)
            .filter(|binding| binding.is_valid())
        else {
            return;
        };
        let RevealTargetBindingState::Linked(snapshot) = &binding.value.state else {
            return;
        };
        if snapshot.ancestry.is_empty() {
            return;
        }
        if let Some(fence) = fence.as_ref()
            && !self.focus_reveal_fence_allows_snapshot(snapshot, fence)
        {
            return;
        }
        let window_id = self.handle.window_id();
        let _ = self.enqueue_bring_into_view(
            key,
            window_id,
            BringIntoViewRequestSource::Focus(focus),
            BringIntoViewOptions::nearest(),
            None,
            self.rendered_frame.generation,
            cx,
        );
    }

    fn focus_reveal_fence_allows_snapshot(
        &self,
        snapshot: &RevealTargetSnapshot,
        fence: &ScrollChainFence,
    ) -> bool {
        if self.scroll_chain_fence_was_interrupted(fence) {
            return false;
        }
        fence.expected_chain.matches_chain(snapshot)
    }

    pub(super) fn enqueue_accessibility_bring_into_view(
        &mut self,
        node: accesskit::NodeId,
        activation_generation: u64,
        options: BringIntoViewOptions,
        cx: &mut App,
    ) {
        let window_id = self.handle.window_id();
        let _ = self.enqueue_bring_into_view(
            RevealTargetKey::Accessibility(node),
            window_id,
            BringIntoViewRequestSource::Accessibility {
                node,
                activation_generation,
            },
            options,
            None,
            self.rendered_frame.generation.saturating_add(1),
            cx,
        );
    }

    pub(super) fn advance_bring_into_view_requests(&mut self, cx: &mut App) {
        if self.active_bring_into_view_requests.is_empty() {
            return;
        }

        let generation = self.rendered_frame.generation;
        let mut candidates = Vec::new();
        let requests = mem::take(&mut self.active_bring_into_view_requests);
        for mut request in requests {
            if let Some(reason) = self.stale_bring_into_view_source(&request) {
                self.finish_bring_into_view_request(
                    request,
                    BringIntoViewOutcome::Cancelled(reason),
                );
                continue;
            }
            let binding = self
                .rendered_frame
                .reveal_target_binding(request.key)
                .cloned();
            let duplicate = self
                .rendered_frame
                .reveal_target_binding_is_duplicate(request.key);
            if duplicate && generation >= request.eligible_after_generation {
                self.finish_bring_into_view_request(
                    request,
                    BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::TargetUnlinked),
                );
                continue;
            }
            let Some(binding) = binding.filter(|binding| binding.is_valid()) else {
                if generation >= request.eligible_after_generation {
                    self.finish_bring_into_view_request(
                        request,
                        BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::TargetUnlinked),
                    );
                } else {
                    self.active_bring_into_view_requests.push(request);
                }
                continue;
            };
            let snapshot = match binding.value.state {
                RevealTargetBindingState::Linked(snapshot) => snapshot,
                RevealTargetBindingState::Suppressed => {
                    if generation >= request.eligible_after_generation {
                        self.finish_bring_into_view_request(
                            request,
                            BringIntoViewOutcome::Cancelled(
                                BringIntoViewCancelReason::TargetSuppressed,
                            ),
                        );
                    } else {
                        self.active_bring_into_view_requests.push(request);
                    }
                    continue;
                }
                RevealTargetBindingState::Unavailable => {
                    if generation >= request.eligible_after_generation {
                        self.finish_bring_into_view_request(
                            request,
                            BringIntoViewOutcome::Cancelled(
                                BringIntoViewCancelReason::TargetUnlinked,
                            ),
                        );
                    } else {
                        self.active_bring_into_view_requests.push(request);
                    }
                    continue;
                }
            };

            if let Some(expected) = request.expected_chain.as_ref() {
                if !expected.matches_chain(&snapshot) {
                    self.finish_bring_into_view_request(
                        request,
                        BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::AncestryChanged),
                    );
                    continue;
                }
            } else {
                request.capture_chain_expectations(&snapshot);
            }

            if request.direct_scroll_was_overridden() {
                self.finish_bring_into_view_request(
                    request,
                    BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::ScrollOverridden),
                );
                continue;
            }
            candidates.push((request, snapshot));
        }

        candidates.sort_by_key(|(request, _)| std::cmp::Reverse(request.id.sequence));
        let mut claimed = FxHashSet::default();
        for (mut request, snapshot) in candidates {
            let overlaps = snapshot
                .ancestry
                .iter()
                .any(|container| claimed.contains(&container.id));
            if overlaps {
                self.finish_bring_into_view_request(
                    request,
                    BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::Superseded),
                );
                continue;
            }
            claimed.extend(snapshot.ancestry.iter().map(|container| container.id));

            if generation < request.eligible_after_generation
                || request.last_advanced_generation == Some(generation)
            {
                self.active_bring_into_view_requests.push(request);
                continue;
            }
            request.last_advanced_generation = Some(generation);
            match self.advance_bring_into_view_request(&mut request, &snapshot, cx) {
                RequestAdvance::Pending => self.active_bring_into_view_requests.push(request),
                RequestAdvance::Complete(completion) => self.finish_bring_into_view_request(
                    request,
                    BringIntoViewOutcome::Completed(completion),
                ),
                RequestAdvance::Cancel(reason) => self.finish_bring_into_view_request(
                    request,
                    BringIntoViewOutcome::Cancelled(reason),
                ),
            }
        }
        self.schedule_bring_into_view_resolution_dispatch(cx);
    }

    fn advance_bring_into_view_request(
        &mut self,
        request: &mut ActiveBringIntoViewRequest,
        snapshot: &RevealTargetSnapshot,
        cx: &mut App,
    ) -> RequestAdvance {
        if let Some(motion) = request.motion.take() {
            let Some(container) = snapshot
                .ancestry
                .iter()
                .find(|container| container.id == motion.container_id)
            else {
                return RequestAdvance::Cancel(BringIntoViewCancelReason::AncestryChanged);
            };
            let elapsed = cx
                .background_executor()
                .now()
                .saturating_duration_since(motion.started_at);
            let sample = motion.progress.sample_elapsed(elapsed);
            let progress = sample.progress();
            let offset = point(
                motion.from.x + (motion.target.x - motion.from.x) * progress,
                motion.from.y + (motion.target.y - motion.from.y) * progress,
            );
            let changed = container.driver.apply_authority_offset(offset, motion.axes);
            request.moved |= changed;
            if sample.complete() {
                container
                    .driver
                    .apply_authority_offset(motion.target, motion.axes);
            } else {
                request.motion = Some(motion);
            }
            self.refresh();
            return RequestAdvance::Pending;
        }

        let requested_axes = request.requested_axes();
        for container in &snapshot.ancestry {
            let axes = requested_axes.intersect(container.axes);
            if !axes.any() {
                continue;
            }
            let window_delta = reveal_window_delta(
                snapshot.geometry.displayed_bounds(),
                container.visible_bounds,
                request.options,
                axes,
            );
            if window_delta == Point::default() {
                continue;
            }
            let Ok(mut local_delta) = container.geometry.window_to_local_vector(window_delta)
            else {
                return RequestAdvance::Cancel(BringIntoViewCancelReason::NoProgress);
            };
            if !axes.horizontal {
                local_delta.x = Pixels::ZERO;
            }
            if !axes.vertical {
                local_delta.y = Pixels::ZERO;
            }
            if local_delta == Point::default() {
                continue;
            }
            let current = container.driver.offset();
            let max = container.driver.max_offset();
            let exact_target = point(
                if axes.horizontal {
                    (current.x + local_delta.x).clamp(-max.x, Pixels::ZERO)
                } else {
                    current.x
                },
                if axes.vertical {
                    (current.y + local_delta.y).clamp(-max.y, Pixels::ZERO)
                } else {
                    current.y
                },
            );
            let Some(target) = self.quantize_reveal_target(
                current,
                exact_target,
                max,
                snapshot.geometry.displayed_bounds(),
                container.visible_bounds,
                container.geometry,
                request.options,
                axes,
            ) else {
                return RequestAdvance::Cancel(BringIntoViewCancelReason::NoProgress);
            };
            if target == current {
                let visibility_delta = nearest_window_delta(
                    snapshot.geometry.displayed_bounds(),
                    container.visible_bounds,
                    request.options.margins,
                    axes,
                );
                let Ok(mut local_visibility_delta) =
                    container.geometry.window_to_local_vector(visibility_delta)
                else {
                    return RequestAdvance::Cancel(BringIntoViewCancelReason::NoProgress);
                };
                local_visibility_delta =
                    self.pixel_snap_point_away_from_zero(local_visibility_delta);
                if !axes.horizontal {
                    local_visibility_delta.x = Pixels::ZERO;
                }
                if !axes.vertical {
                    local_visibility_delta.y = Pixels::ZERO;
                }
                if local_visibility_delta != Point::default() {
                    return RequestAdvance::Cancel(BringIntoViewCancelReason::NoProgress);
                }
                continue;
            }

            match request.options.behavior {
                BringIntoViewBehavior::Instant => {
                    if !container.driver.apply_authority_offset(target, axes) {
                        return RequestAdvance::Cancel(BringIntoViewCancelReason::NoProgress);
                    }
                    request.moved = true;
                }
                BringIntoViewBehavior::Animated(transition)
                    if !transition.is_immediate() && !transition.preference().is_immediate() =>
                {
                    request.motion = Some(ActiveRevealMotion {
                        container_id: container.id,
                        axes,
                        from: current,
                        target,
                        started_at: cx.background_executor().now(),
                        progress: transition.progress_run(Duration::ZERO),
                    });
                }
                BringIntoViewBehavior::Animated(_) => {
                    if !container.driver.apply_authority_offset(target, axes) {
                        return RequestAdvance::Cancel(BringIntoViewCancelReason::NoProgress);
                    }
                    request.moved = true;
                }
            }
            self.refresh();
            return RequestAdvance::Pending;
        }

        RequestAdvance::Complete(if request.moved {
            BringIntoViewCompletion::Revealed
        } else {
            BringIntoViewCompletion::AlreadyVisible
        })
    }

    fn stale_bring_into_view_source(
        &self,
        request: &ActiveBringIntoViewRequest,
    ) -> Option<BringIntoViewCancelReason> {
        match request.source {
            BringIntoViewRequestSource::Application => None,
            BringIntoViewRequestSource::Focus(focus)
                if self.rendered_frame.focus_path().last() == Some(&focus)
                    && self.focus == Some(focus)
                    && self
                        .pending_focus_claim
                        .as_ref()
                        .is_none_or(|claim| claim.target == focus)
                    && self.pending_blur_claim_generation.is_none() =>
            {
                None
            }
            BringIntoViewRequestSource::Focus(_) => Some(BringIntoViewCancelReason::Superseded),
            BringIntoViewRequestSource::Accessibility {
                node,
                activation_generation,
            } if self.a11y.accepts_action(
                activation_generation,
                accesskit::TreeId::ROOT,
                node,
                accesskit::Action::ScrollIntoView,
            ) =>
            {
                None
            }
            BringIntoViewRequestSource::Accessibility { .. } => {
                Some(BringIntoViewCancelReason::TargetUnlinked)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn quantize_reveal_target(
        &self,
        current: Point<Pixels>,
        exact_target: Point<Pixels>,
        max: Point<Pixels>,
        target_bounds: Bounds<Pixels>,
        viewport_bounds: Bounds<Pixels>,
        container_geometry: ElementGeometry,
        options: BringIntoViewOptions,
        axes: ScrollAxes,
    ) -> Option<Point<Pixels>> {
        let mut quantized = self.pixel_snap_point(exact_target);
        if axes.horizontal
            && options.horizontal_axis()
                == BringIntoViewAxis::Align(BringIntoViewAlignment::Nearest)
        {
            quantized.x = self.nearest_device_pixel_offset(
                current.x,
                exact_target.x,
                max.x,
                target_bounds,
                viewport_bounds,
                container_geometry,
                options.margins,
                true,
            )?;
        }
        if axes.vertical
            && options.vertical_axis() == BringIntoViewAxis::Align(BringIntoViewAlignment::Nearest)
        {
            quantized.y = self.nearest_device_pixel_offset(
                current.y,
                exact_target.y,
                max.y,
                target_bounds,
                viewport_bounds,
                container_geometry,
                options.margins,
                false,
            )?;
        }
        if axes.horizontal {
            quantized.x = quantized.x.clamp(-max.x, Pixels::ZERO);
        } else {
            quantized.x = current.x;
        }
        if axes.vertical {
            quantized.y = quantized.y.clamp(-max.y, Pixels::ZERO);
        } else {
            quantized.y = current.y;
        }
        Some(quantized)
    }

    #[allow(clippy::too_many_arguments)]
    fn nearest_device_pixel_offset(
        &self,
        current: Pixels,
        exact_target: Pixels,
        max: Pixels,
        target_bounds: Bounds<Pixels>,
        viewport_bounds: Bounds<Pixels>,
        container_geometry: ElementGeometry,
        margins: BringIntoViewMargins,
        horizontal: bool,
    ) -> Option<Pixels> {
        let minimum = -max;
        let maximum = Pixels::ZERO;
        let scale_factor = self.scale_factor();
        let scaled = exact_target.0 * scale_factor;
        let candidates = [
            current,
            crate::px(scaled.floor() / scale_factor).clamp(minimum, maximum),
            crate::px(scaled.ceil() / scale_factor).clamp(minimum, maximum),
        ];
        let mut best = current;
        let mut best_residual = self.nearest_candidate_residual(
            current,
            current,
            target_bounds,
            viewport_bounds,
            container_geometry,
            margins,
            horizontal,
        )?;
        let mut best_exact_distance = (current.0 - exact_target.0).abs();

        for candidate in candidates.into_iter().skip(1) {
            if candidate == current {
                continue;
            }
            let residual = self.nearest_candidate_residual(
                current,
                candidate,
                target_bounds,
                viewport_bounds,
                container_geometry,
                margins,
                horizontal,
            )?;
            let exact_distance = (candidate.0 - exact_target.0).abs();
            let improves_visibility = residual.total_cmp(&best_residual).is_lt();
            let improves_quantization = best != current
                && residual.total_cmp(&best_residual).is_eq()
                && exact_distance.total_cmp(&best_exact_distance).is_lt();
            if improves_visibility || improves_quantization {
                best = candidate;
                best_residual = residual;
                best_exact_distance = exact_distance;
            }
        }

        Some(best)
    }

    #[allow(clippy::too_many_arguments)]
    fn nearest_candidate_residual(
        &self,
        current: Pixels,
        candidate: Pixels,
        target_bounds: Bounds<Pixels>,
        viewport_bounds: Bounds<Pixels>,
        container_geometry: ElementGeometry,
        margins: BringIntoViewMargins,
        horizontal: bool,
    ) -> Option<f32> {
        let local_movement = if horizontal {
            point(candidate - current, Pixels::ZERO)
        } else {
            point(Pixels::ZERO, candidate - current)
        };
        let window_movement = container_geometry
            .local_to_window_vector(local_movement)
            .ok()?;
        let nearest = BringIntoViewAxis::Align(BringIntoViewAlignment::Nearest);
        let residual = if horizontal {
            alignment_delta(
                target_bounds.left() + window_movement.x,
                target_bounds.right() + window_movement.x,
                viewport_bounds.left() + margins.left,
                viewport_bounds.right() - margins.right,
                nearest,
            )
        } else {
            alignment_delta(
                target_bounds.top() + window_movement.y,
                target_bounds.bottom() + window_movement.y,
                viewport_bounds.top() + margins.top,
                viewport_bounds.bottom() - margins.bottom,
                nearest,
            )
        };
        residual.0.is_finite().then_some(residual.0.abs())
    }

    fn pixel_snap_point_away_from_zero(&self, point: Point<Pixels>) -> Point<Pixels> {
        point.map(|coordinate| self.pixel_snap_away_from_zero(coordinate))
    }

    fn pixel_snap_away_from_zero(&self, value: Pixels) -> Pixels {
        let scale_factor = self.scale_factor();
        let scaled = value.0 * scale_factor;
        let snapped = if scaled > 0.0 {
            scaled.ceil()
        } else if scaled < 0.0 {
            scaled.floor()
        } else {
            0.0
        };
        crate::px(snapped / scale_factor)
    }

    fn finish_bring_into_view_request(
        &mut self,
        mut request: ActiveBringIntoViewRequest,
        outcome: BringIntoViewOutcome,
    ) {
        let Some(callback) = request.callback.take() else {
            return;
        };
        self.bring_into_view_resolutions
            .push(BringIntoViewResolution { outcome, callback });
    }

    fn schedule_bring_into_view_resolution_dispatch(&self, cx: &mut App) {
        if self.bring_into_view_resolutions.is_empty() {
            return;
        }
        let handle = self.handle;
        cx.spawn(async move |cx| {
            handle
                .update(cx, |_, window, cx| {
                    window.dispatch_bring_into_view_resolutions(cx)
                })
                .ok();
        })
        .detach();
    }

    fn dispatch_bring_into_view_resolutions(&mut self, cx: &mut App) {
        let resolutions = mem::take(&mut self.bring_into_view_resolutions);
        for resolution in resolutions {
            let callback = resolution.callback.borrow_mut().take();
            if let Some(callback) = callback {
                callback(resolution.outcome, self, cx);
            }
        }
    }

    pub(super) fn close_bring_into_view_authority(&mut self, cx: &mut App) {
        let requests = mem::take(&mut self.active_bring_into_view_requests);
        for request in requests {
            self.finish_bring_into_view_request(
                request,
                BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::WindowClosed),
            );
        }
        self.dispatch_bring_into_view_resolutions(cx);
    }

    pub(crate) fn with_reveal_target<R>(
        &mut self,
        handle: &RevealTargetHandle,
        layout_id: LayoutId,
        layout_bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, RevealTargetError> {
        self.invalidator.debug_assert_prepaint();
        self.ensure_reveal_target_window(handle)?;
        let key = RevealTargetKey::Public(handle.id);
        if self.next_frame.has_reveal_target_binding(key) {
            return Err(RevealTargetError::HandleAlreadyBound { handle: *handle });
        }

        let stack = self.reveal_target_capture_stack.clone();
        let entered_depth = stack.borrow().len();
        stack.borrow_mut().push(RevealTargetCapture::new(
            *handle,
            layout_id,
            layout_bounds,
            self.subtree_transform(),
            self.subtree_geometry_validity(),
            self.subtree_presentation(),
            self.current_scroll_ancestry(),
        ));
        let guard = RevealTargetCaptureGuard {
            stack: stack.clone(),
            entered_depth,
            armed: true,
        };
        let result = f(self);
        let capture = stack
            .borrow_mut()
            .pop()
            .expect("reveal target capture stack entry missing");
        let mut guard = guard;
        guard.armed = false;
        drop(guard);
        self.bind_reveal_target_capture(capture)?;
        Ok(result)
    }

    fn bind_reveal_target_capture(
        &mut self,
        capture: RevealTargetCapture,
    ) -> Result<(), RevealTargetError> {
        let key = RevealTargetKey::Public(capture.handle.id);
        if self.next_frame.has_reveal_target_binding(key) {
            return Err(RevealTargetError::HandleAlreadyBound {
                handle: capture.handle,
            });
        }
        let state = if !capture.presentation.is_interactive() {
            RevealTargetBindingState::Suppressed
        } else if capture.failed
            || capture
                .validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
        {
            RevealTargetBindingState::Unavailable
        } else {
            match capture.transform.try_project_bounds(capture.layout_bounds) {
                Ok(displayed_bounds) => RevealTargetBindingState::Linked(RevealTargetSnapshot {
                    frame_generation: self.next_frame.generation,
                    geometry: ElementGeometry::from_resolved(
                        capture.layout_bounds,
                        displayed_bounds,
                        capture.transform,
                    ),
                    ancestry: capture.ancestry.into_iter().rev().collect(),
                }),
                Err(error) => {
                    if let Some(validity) = capture.validity.as_ref() {
                        validity.invalidate(error);
                    }
                    self.record_subtree_transform_diagnostic(error);
                    RevealTargetBindingState::Unavailable
                }
            }
        };
        self.next_frame
            .record_reveal_target_binding(super::FrameOutput::new(
                RevealTargetBinding { key, state },
                capture.validity,
            ));
        Ok(())
    }

    pub(crate) fn bind_focus_reveal_target(&mut self, focus: FocusId, bounds: Bounds<Pixels>) {
        self.bind_internal_reveal_target(RevealTargetKey::Focus(focus), bounds);
    }

    pub(crate) fn bind_accessibility_reveal_target(
        &mut self,
        node: accesskit::NodeId,
        bounds: Bounds<Pixels>,
    ) {
        self.bind_internal_reveal_target(RevealTargetKey::Accessibility(node), bounds);
    }

    fn bind_internal_reveal_target(&mut self, key: RevealTargetKey, bounds: Bounds<Pixels>) {
        self.invalidator.debug_assert_prepaint();
        self.record_reveal_target_binding(key, bounds);
    }

    fn record_reveal_target_binding(&mut self, key: RevealTargetKey, bounds: Bounds<Pixels>) {
        let state = if !self.subtree_presentation().is_interactive() {
            RevealTargetBindingState::Suppressed
        } else {
            self.try_element_geometry(bounds)
                .map(|geometry| {
                    RevealTargetBindingState::Linked(RevealTargetSnapshot {
                        frame_generation: self.next_frame.generation,
                        geometry,
                        ancestry: self.current_scroll_ancestry().into_iter().rev().collect(),
                    })
                })
                .unwrap_or(RevealTargetBindingState::Unavailable)
        };
        self.next_frame
            .record_reveal_target_binding(super::FrameOutput::new(
                RevealTargetBinding { key, state },
                self.subtree_geometry_validity(),
            ));
    }

    pub(crate) fn update_reveal_target_presentation(&mut self, presentation: SubtreePresentation) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .reveal_target_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.presentation = presentation;
        }
    }

    pub(crate) fn update_reveal_target_transform(
        &mut self,
        transform: ResolvedSubtreeTransform,
        validity: Option<SubtreeGeometryValidity>,
    ) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .reveal_target_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.transform = transform;
            capture.validity = validity.clone();
        }
    }

    pub(crate) fn invalidate_reveal_target_capture(&self) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .reveal_target_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.failed = true;
        }
    }

    pub(crate) fn register_reveal_target_root_layout_alias(&mut self, alias: LayoutId) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .reveal_target_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.add_root_layout_alias(alias);
        }
    }

    pub(crate) fn reveal_target_capture_requires_fresh_prepaint(&self) -> bool {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return false;
        };
        self.reveal_target_capture_stack
            .borrow()
            .iter()
            .any(|capture| capture.contains_root_layout(layout_id))
    }

    pub(crate) fn with_scroll_handle_container<R>(
        &mut self,
        handle: ScrollHandle,
        bounds: Bounds<Pixels>,
        overflow: Point<Overflow>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let Ok(geometry) = self.try_element_geometry(bounds) else {
            return f(self);
        };
        let visible_bounds = geometry.displayed_bounds();
        let binding =
            ScrollContainerBinding::for_handle(handle, geometry, visible_bounds, overflow);
        self.with_scroll_container(binding, f)
    }

    pub(crate) fn with_list_scroll_container<R>(
        &mut self,
        state: ListState,
        bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let Ok(geometry) = self.try_element_geometry(bounds) else {
            return f(self);
        };
        let visible_bounds = geometry.displayed_bounds();
        let binding = ScrollContainerBinding::for_list(state, geometry, visible_bounds);
        self.with_scroll_container(binding, f)
    }

    fn with_scroll_container<R>(
        &mut self,
        binding: ScrollContainerBinding,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let stack = self.scroll_ancestry_stack.clone();
        let entered_depth = stack.borrow().len();
        stack.borrow_mut().push(binding);
        let _guard = ScrollAncestryGuard {
            stack,
            entered_depth,
        };
        f(self)
    }

    pub(crate) fn with_scroll_ancestry<R>(
        &mut self,
        ancestry: SmallVec<[ScrollContainerBinding; 8]>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let stack = self.scroll_ancestry_stack.clone();
        let previous = mem::replace(&mut *stack.borrow_mut(), ancestry);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        *stack.borrow_mut() = previous;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn current_scroll_ancestry(&self) -> SmallVec<[ScrollContainerBinding; 4]> {
        self.scroll_ancestry_stack
            .borrow()
            .iter()
            .cloned()
            .collect()
    }

    pub(crate) fn current_scroll_ancestry_for_deferred(
        &self,
    ) -> SmallVec<[ScrollContainerBinding; 8]> {
        self.scroll_ancestry_stack.borrow().clone()
    }

    fn ensure_reveal_target_window(
        &self,
        handle: &RevealTargetHandle,
    ) -> Result<(), RevealTargetError> {
        let target_window = self.handle.window_id();
        if handle.window_id != target_window {
            return Err(RevealTargetError::WrongWindow {
                handle_window: handle.window_id,
                target_window,
            });
        }
        Ok(())
    }
}

impl ActiveBringIntoViewRequest {
    fn requested_axes(&self) -> ScrollAxes {
        ScrollAxes::requested(self.options)
    }

    fn capture_chain_expectations(&mut self, snapshot: &RevealTargetSnapshot) {
        self.expected_chain = Some(ScrollChainExpectation::capture(snapshot));
    }

    fn direct_scroll_was_overridden(&self) -> bool {
        self.expected_chain
            .as_ref()
            .is_some_and(|expected| expected.direct_scroll_was_overridden(self.requested_axes()))
    }
}

enum RequestAdvance {
    Pending,
    Complete(BringIntoViewCompletion),
    Cancel(BringIntoViewCancelReason),
}

fn reveal_window_delta(
    target: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    options: BringIntoViewOptions,
    axes: ScrollAxes,
) -> Point<Pixels> {
    let horizontal = if axes.horizontal {
        alignment_delta(
            target.left(),
            target.right(),
            viewport.left() + options.margins.left,
            viewport.right() - options.margins.right,
            options.horizontal,
        )
    } else {
        Pixels::ZERO
    };
    let vertical = if axes.vertical {
        alignment_delta(
            target.top(),
            target.bottom(),
            viewport.top() + options.margins.top,
            viewport.bottom() - options.margins.bottom,
            options.vertical,
        )
    } else {
        Pixels::ZERO
    };
    point(horizontal, vertical)
}

fn nearest_window_delta(
    target: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    margins: BringIntoViewMargins,
    axes: ScrollAxes,
) -> Point<Pixels> {
    let nearest = BringIntoViewAxis::Align(BringIntoViewAlignment::Nearest);
    point(
        if axes.horizontal {
            alignment_delta(
                target.left(),
                target.right(),
                viewport.left() + margins.left,
                viewport.right() - margins.right,
                nearest,
            )
        } else {
            Pixels::ZERO
        },
        if axes.vertical {
            alignment_delta(
                target.top(),
                target.bottom(),
                viewport.top() + margins.top,
                viewport.bottom() - margins.bottom,
                nearest,
            )
        } else {
            Pixels::ZERO
        },
    )
}

fn alignment_delta(
    target_min: Pixels,
    target_max: Pixels,
    viewport_min: Pixels,
    viewport_max: Pixels,
    policy: BringIntoViewAxis,
) -> Pixels {
    let BringIntoViewAxis::Align(alignment) = policy else {
        return Pixels::ZERO;
    };
    if viewport_min > viewport_max {
        return (viewport_min + viewport_max) / 2.0 - (target_min + target_max) / 2.0;
    }
    match alignment {
        BringIntoViewAlignment::MinEdge => viewport_min - target_min,
        BringIntoViewAlignment::Center => {
            (viewport_min + viewport_max) / 2.0 - (target_min + target_max) / 2.0
        }
        BringIntoViewAlignment::MaxEdge => viewport_max - target_max,
        BringIntoViewAlignment::Nearest => {
            if target_min <= viewport_min && target_max >= viewport_max {
                Pixels::ZERO
            } else if target_max < viewport_min || target_min < viewport_min {
                viewport_min - target_min
            } else if target_min > viewport_max || target_max > viewport_max {
                viewport_max - target_max
            } else {
                Pixels::ZERO
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    #[test]
    fn nearest_alignment_handles_visibility_and_oversized_targets() {
        assert_eq!(
            alignment_delta(
                px(20.),
                px(40.),
                px(0.),
                px(100.),
                BringIntoViewAxis::default()
            ),
            px(0.)
        );
        assert_eq!(
            alignment_delta(
                px(-20.),
                px(20.),
                px(0.),
                px(100.),
                BringIntoViewAxis::default()
            ),
            px(20.)
        );
        assert_eq!(
            alignment_delta(
                px(90.),
                px(120.),
                px(0.),
                px(100.),
                BringIntoViewAxis::default()
            ),
            px(-20.)
        );
        assert_eq!(
            alignment_delta(
                px(-20.),
                px(120.),
                px(0.),
                px(100.),
                BringIntoViewAxis::default()
            ),
            px(0.)
        );
    }

    #[test]
    fn margins_reject_negative_and_non_finite_values() {
        assert!(BringIntoViewMargins::try_new(px(-1.), px(0.), px(0.), px(0.)).is_err());
        assert!(BringIntoViewMargins::try_new(Pixels(f32::NAN), px(0.), px(0.), px(0.)).is_err());
    }
}
