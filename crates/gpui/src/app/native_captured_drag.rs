use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    fmt,
    rc::{Rc, Weak},
    sync::Arc,
};

use crate::{
    App, AppCell, Bounds, DevicePixels, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformNativePointerPhysicalFrame, PlatformWindowHitStack, PlatformWindowPhysicalGeometry,
    Point, PointerCancelEvent, PointerCancelReason, Subscription, Window, WindowId,
};

/// Monotonic identity assigned by GPUI's native ingress authority.
///
/// The value can be observed for ordering and diagnostics, but only GPUI can create one.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeIngressSequence(u64);

impl NativeIngressSequence {
    pub(super) fn new(sequence: u64) -> Self {
        Self(sequence)
    }

    pub(super) fn value(self) -> u64 {
        self.0
    }

    /// Returns the monotonic ordinal assigned at native ingress.
    pub fn ordinal(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NativeIngressSequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identity for one active framework drag.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeCapturedDragGeneration(u64);

/// Exact post-borrow native capture-release authority for one cancelled drag generation.
///
/// A barrier cannot be forged by callers. Its three identities prevent a delayed terminal from
/// authorizing cleanup for a replacement drag or a different source window.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeCapturedDragReleaseBarrier {
    source_window: WindowId,
    drag_generation: NativeCapturedDragGeneration,
    release_generation: u64,
}

impl NativeCapturedDragReleaseBarrier {
    pub(super) fn from_release_token(
        token: super::NativePointerCaptureReleaseToken,
    ) -> Option<Self> {
        Some(Self {
            source_window: token.window_id(),
            drag_generation: token.captured_drag_generation()?,
            release_generation: token.release_generation(),
        })
    }

    /// Returns the source window captured when the drag generation began.
    pub fn source_window(self) -> WindowId {
        self.source_window
    }

    /// Returns the exact cancelled captured-drag generation.
    pub fn drag_generation(self) -> NativeCapturedDragGeneration {
        self.drag_generation
    }

    /// Returns the private native release generation used to reject stale completions.
    pub fn release_generation(self) -> u64 {
        self.release_generation
    }
}

/// The only terminal outcomes that permit effects dependent on a cancelled native capture.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCapturedDragReleaseTerminal {
    /// The backend proved that native capture was released.
    Released,
    /// The native source window reached its terminal state.
    NativeWindowTerminal,
    /// A replacement logical owner still holds capture, so the old release is unnecessary.
    NotRequired,
}

pub(super) type NativeCapturedDragReleaseContinuation =
    Box<dyn FnOnce(NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal, &mut App)>;

pub(super) struct NativeCapturedDragReleaseCompletion {
    barrier: NativeCapturedDragReleaseBarrier,
    terminal: NativeCapturedDragReleaseTerminal,
    continuations: Vec<NativeCapturedDragReleaseContinuation>,
}

impl NativeCapturedDragReleaseCompletion {
    pub(super) fn new(
        barrier: NativeCapturedDragReleaseBarrier,
        terminal: NativeCapturedDragReleaseTerminal,
        continuations: Vec<NativeCapturedDragReleaseContinuation>,
    ) -> Self {
        Self {
            barrier,
            terminal,
            continuations,
        }
    }

    pub(super) fn barrier(&self) -> NativeCapturedDragReleaseBarrier {
        self.barrier
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        NativeCapturedDragReleaseBarrier,
        NativeCapturedDragReleaseTerminal,
        Vec<NativeCapturedDragReleaseContinuation>,
    ) {
        (self.barrier, self.terminal, self.continuations)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeCapturedDragStartStatus {
    Reserved,
    Active,
    Revoked,
}

struct NativeCapturedDragStartState {
    generation: NativeCapturedDragGeneration,
    source_ingress: Option<(WindowId, NativeIngressSequence)>,
    app: Weak<AppCell>,
    status: Cell<NativeCapturedDragStartStatus>,
    consumer_claimed: Cell<bool>,
    prepared_route: RefCell<Option<RegisteredConsumer>>,
}

#[derive(Clone)]
pub(crate) struct NativeCapturedDragStartToken(Rc<NativeCapturedDragStartState>);

/// Inert consumer authority prepared inside a drag listener.
///
/// The consumer may retain its complete route before the listener returns, but it cannot receive
/// native facts until GPUI atomically commits the corresponding active drag. A failed listener or
/// rejected start changes the same authority to revoked without invoking consumer code.
#[doc(hidden)]
#[derive(Clone)]
pub struct PreparedNativeCapturedDragConsumer(Rc<NativeCapturedDragStartState>);

impl fmt::Debug for PreparedNativeCapturedDragConsumer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedNativeCapturedDragConsumer")
            .field("generation", &self.generation())
            .field("active", &self.is_active())
            .field("revoked", &self.is_revoked())
            .finish()
    }
}

impl PreparedNativeCapturedDragConsumer {
    /// Returns the exact generation reserved before the drag listener ran.
    #[doc(hidden)]
    pub fn generation(&self) -> NativeCapturedDragGeneration {
        self.0.generation
    }

    /// Reports whether GPUI committed both the active drag and this prepared consumer.
    #[doc(hidden)]
    pub fn is_active(&self) -> bool {
        self.0.status.get() == NativeCapturedDragStartStatus::Active
    }

    /// Reports whether the drag start failed before activation.
    #[doc(hidden)]
    pub fn is_revoked(&self) -> bool {
        self.0.status.get() == NativeCapturedDragStartStatus::Revoked
    }
}

impl fmt::Debug for NativeCapturedDragStartToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("NativeCapturedDragStartToken")
            .field(&self.generation())
            .finish()
    }
}

impl NativeCapturedDragStartToken {
    fn new(
        generation: NativeCapturedDragGeneration,
        source_ingress: Option<(WindowId, NativeIngressSequence)>,
        app: Weak<AppCell>,
    ) -> Self {
        Self(Rc::new(NativeCapturedDragStartState {
            generation,
            source_ingress,
            app,
            status: Cell::new(NativeCapturedDragStartStatus::Reserved),
            consumer_claimed: Cell::new(false),
            prepared_route: RefCell::new(None),
        }))
    }

    pub(crate) fn generation(&self) -> NativeCapturedDragGeneration {
        self.0.generation
    }

    fn source_ingress(&self) -> Option<(WindowId, NativeIngressSequence)> {
        self.0.source_ingress
    }

    pub(crate) fn prepare_consumer(&self) -> PreparedNativeCapturedDragConsumer {
        assert!(
            self.0.status.get() == NativeCapturedDragStartStatus::Reserved,
            "captured-drag start is already resolved"
        );
        assert!(
            !self.0.consumer_claimed.replace(true),
            "captured-drag start accepts only one prepared consumer"
        );
        if let Some(app) = self.0.app.upgrade() {
            *self.0.prepared_route.borrow_mut() = app.prepare_native_captured_drag_route();
        }
        PreparedNativeCapturedDragConsumer(self.0.clone())
    }

    fn route_binding(&self) -> NativeCapturedDragRouteBinding {
        if self.0.consumer_claimed.get() {
            NativeCapturedDragRouteBinding::Prepared(self.0.prepared_route.borrow().clone())
        } else {
            NativeCapturedDragRouteBinding::Unprepared
        }
    }
}

pub(crate) struct NativeCapturedDragStartReservation {
    token: NativeCapturedDragStartToken,
}

impl NativeCapturedDragStartReservation {
    fn new(
        generation: NativeCapturedDragGeneration,
        source_ingress: Option<(WindowId, NativeIngressSequence)>,
        app: Weak<AppCell>,
    ) -> Self {
        Self {
            token: NativeCapturedDragStartToken::new(generation, source_ingress, app),
        }
    }

    pub(crate) fn token(&self) -> NativeCapturedDragStartToken {
        self.token.clone()
    }

    fn commit(self) -> NativeCapturedDragGeneration {
        assert!(
            self.token.0.status.get() == NativeCapturedDragStartStatus::Reserved,
            "captured-drag start is already resolved"
        );
        self.token
            .0
            .status
            .set(NativeCapturedDragStartStatus::Active);
        self.token.0.generation
    }
}

impl Drop for NativeCapturedDragStartReservation {
    fn drop(&mut self) {
        if self.token.0.status.get() == NativeCapturedDragStartStatus::Reserved {
            self.token
                .0
                .status
                .set(NativeCapturedDragStartStatus::Revoked);
        }
    }
}

/// Phase of a captured native pointer fact routed for an active drag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCapturedDragPhase {
    /// The captured pointer moved while the initiating button remained pressed.
    Moved,
    /// The initiating button was released.
    Released,
    /// The gesture ended without a matching button release.
    Cancelled(PointerCancelReason),
}

impl NativeCapturedDragPhase {
    fn is_terminal(self) -> bool {
        !matches!(self, Self::Moved)
    }
}

/// Immutable native pointer fact delivered after the source window update is returned to GPUI.
#[derive(Clone)]
pub struct NativeCapturedDragEvent {
    sequence: NativeIngressSequence,
    generation: NativeCapturedDragGeneration,
    source_window: WindowId,
    payload: Arc<dyn Any>,
    button: crate::MouseButton,
    phase: NativeCapturedDragPhase,
    source_local_position: Option<Point<Pixels>>,
    physical_frame: Option<PlatformNativePointerPhysicalFrame>,
    window_hit_stack: PlatformWindowHitStack,
    route_lock: Option<Arc<dyn Any>>,
}

impl fmt::Debug for NativeCapturedDragEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeCapturedDragEvent")
            .field("sequence", &self.sequence)
            .field("generation", &self.generation)
            .field("source_window", &self.source_window)
            .field("phase", &self.phase)
            .field("button", &self.button)
            .field("source_local_position", &self.source_local_position)
            .field("physical_frame", &self.physical_frame)
            .field("window_hit_stack", &self.window_hit_stack)
            .field("route_locked", &self.route_lock.is_some())
            .finish_non_exhaustive()
    }
}

impl NativeCapturedDragEvent {
    /// Returns the native ingress ordering identity retained from the source callback.
    pub fn sequence(&self) -> NativeIngressSequence {
        self.sequence
    }

    /// Returns the active-drag generation this fact belongs to.
    pub fn generation(&self) -> NativeCapturedDragGeneration {
        self.generation
    }

    /// Returns the source window that owns native pointer capture.
    pub fn source_window(&self) -> WindowId {
        self.source_window
    }

    /// Returns the immutable active-drag payload snapshot when it matches `T`.
    pub fn payload<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }

    /// Returns the mouse button that owns this drag generation.
    pub fn button(&self) -> crate::MouseButton {
        self.button
    }

    /// Returns the captured pointer phase.
    pub fn phase(&self) -> NativeCapturedDragPhase {
        self.phase
    }

    /// Returns the pointer position in source-window logical coordinates.
    pub fn source_local_position(&self) -> Option<Point<Pixels>> {
        self.source_local_position
    }

    /// Returns the callback-scoped physical pointer frame for move and release facts.
    pub fn physical_frame(&self) -> Option<PlatformNativePointerPhysicalFrame> {
        self.physical_frame
    }

    /// Returns the pointer position in physical desktop coordinates when supported.
    pub fn global_position(&self) -> Option<Point<DevicePixels>> {
        self.physical_frame.map(|frame| frame.global_position())
    }

    /// Returns the source window's stable physical geometry sampled with this fact.
    pub fn source_geometry(&self) -> Option<PlatformWindowPhysicalGeometry> {
        self.physical_frame.map(|frame| frame.source_geometry())
    }

    /// Returns the source window's scale factor when stable physical geometry was available.
    pub fn source_scale_factor(&self) -> Option<f32> {
        self.source_geometry()
            .map(|geometry| geometry.scale_factor())
    }

    /// Returns the source client bounds in physical desktop coordinates when supported.
    pub fn source_client_bounds(&self) -> Option<Bounds<DevicePixels>> {
        self.source_geometry()
            .map(|geometry| geometry.client_bounds())
    }

    /// Returns the complete point-scoped native top-level hit stack when supported.
    pub fn window_hit_stack(&self) -> &PlatformWindowHitStack {
        &self.window_hit_stack
    }

    /// Returns the generation-frozen route lock produced before release listeners ran.
    #[doc(hidden)]
    pub fn route_lock<T: 'static>(&self) -> Option<&T> {
        self.route_lock.as_deref()?.downcast_ref()
    }
}

pub(super) type NativeCapturedDragConsumer =
    Rc<dyn Fn(NativeCapturedDragEvent, &mut App) + 'static>;
pub(super) type NativeCapturedDragReleaseLocker =
    Rc<dyn Fn(&NativeCapturedDragEvent, &mut Window, &mut App) -> Arc<dyn Any> + 'static>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WindowUpdateProvenance {
    Ordinary,
    Native {
        source_window: WindowId,
        sequence: NativeIngressSequence,
        captured_drag_fact_claimed: bool,
    },
}

impl WindowUpdateProvenance {
    fn native_source_ingress(&self) -> Option<(WindowId, NativeIngressSequence)> {
        match self {
            Self::Native {
                source_window,
                sequence,
                ..
            } => Some((*source_window, *sequence)),
            Self::Ordinary => None,
        }
    }

    fn claim_native_captured_drag_fact(
        &mut self,
        window_id: WindowId,
    ) -> Option<NativeIngressSequence> {
        match self {
            Self::Native {
                source_window,
                sequence,
                captured_drag_fact_claimed,
            } if *source_window == window_id && !*captured_drag_fact_claimed => {
                *captured_drag_fact_claimed = true;
                Some(*sequence)
            }
            Self::Ordinary | Self::Native { .. } => None,
        }
    }
}

pub(super) struct ActiveNativeCapturedDragAuthority {
    generation: NativeCapturedDragGeneration,
    source_window: WindowId,
    source: Option<crate::PointerCaptureHandle>,
    button: crate::MouseButton,
    value: Arc<dyn Any>,
}

impl ActiveNativeCapturedDragAuthority {
    fn matches(&self, drag: &crate::AnyDrag) -> bool {
        self.source_window == drag.window_id
            && self.source == drag.source
            && self.button == drag.button
            && Arc::ptr_eq(&self.value, &drag.value)
    }

    pub(super) fn generation_for(
        &self,
        drag: &crate::AnyDrag,
    ) -> Option<NativeCapturedDragGeneration> {
        self.matches(drag).then_some(self.generation)
    }

    fn snapshot(&self) -> NativeCapturedDragAuthoritySnapshot {
        NativeCapturedDragAuthoritySnapshot {
            generation: self.generation,
            source_window: self.source_window,
            button: self.button,
            payload: self.value.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct NativeCapturedDragAuthoritySnapshot {
    generation: NativeCapturedDragGeneration,
    source_window: WindowId,
    button: crate::MouseButton,
    payload: Arc<dyn Any>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ConsumerKey {
    id: u64,
}

#[derive(Clone)]
pub(super) struct RegisteredConsumer {
    key: ConsumerKey,
    callback: NativeCapturedDragConsumer,
    release_locker: Option<NativeCapturedDragReleaseLocker>,
}

#[derive(Clone)]
pub(super) enum NativeCapturedDragRouteBinding {
    Unprepared,
    Prepared(Option<RegisteredConsumer>),
}

#[derive(Clone, Copy)]
enum NativeCapturedDragRegistration {
    Route,
    Observer,
}

pub(super) struct NativeCapturedDragEnvelope {
    pub(super) key: ConsumerKey,
    pub(super) callback: NativeCapturedDragConsumer,
    pub(super) event: NativeCapturedDragEvent,
}

impl NativeCapturedDragEnvelope {
    fn is_coalescible_with(&self, next: &Self) -> bool {
        self.key == next.key
            && self.event.source_window == next.event.source_window
            && self.event.generation == next.event.generation
            && self.event.phase == NativeCapturedDragPhase::Moved
            && next.event.phase == NativeCapturedDragPhase::Moved
            && self.event.window_hit_stack == next.event.window_hit_stack
    }
}

#[derive(Default)]
struct NativeCapturedDragOutboxState {
    next_consumer_id: u64,
    route: Option<RegisteredConsumer>,
    observer: Option<RegisteredConsumer>,
    active_route: Option<RegisteredConsumer>,
    pending: VecDeque<NativeCapturedDragEnvelope>,
    current_authority: Option<NativeCapturedDragAuthoritySnapshot>,
    terminal_generation: Option<NativeCapturedDragGeneration>,
    pending_pointer_cancels: HashMap<NativeIngressSequence, ReservedPointerCancelFact>,
    draining: bool,
}

struct ReservedPointerCancelFact {
    source_window: WindowId,
    terminal: Vec<NativeCapturedDragEnvelope>,
}

#[derive(Default)]
pub(super) struct NativeCapturedDragOutbox {
    state: RefCell<NativeCapturedDragOutboxState>,
}

impl NativeCapturedDragOutbox {
    pub(super) fn register_observer(
        &self,
        app: Weak<AppCell>,
        callback: NativeCapturedDragConsumer,
    ) -> Subscription {
        self.register(
            app,
            callback,
            None,
            NativeCapturedDragRegistration::Observer,
        )
    }

    pub(super) fn register_route(
        &self,
        app: Weak<AppCell>,
        callback: NativeCapturedDragConsumer,
        release_locker: NativeCapturedDragReleaseLocker,
    ) -> Subscription {
        self.register(
            app,
            callback,
            Some(release_locker),
            NativeCapturedDragRegistration::Route,
        )
    }

    fn register(
        &self,
        app: Weak<AppCell>,
        callback: NativeCapturedDragConsumer,
        release_locker: Option<NativeCapturedDragReleaseLocker>,
        registration: NativeCapturedDragRegistration,
    ) -> Subscription {
        let mut state = self.state.borrow_mut();
        let replaced = match registration {
            NativeCapturedDragRegistration::Route => state.route.take(),
            NativeCapturedDragRegistration::Observer => state.observer.take(),
        };
        if let Some(replaced) = replaced {
            let frozen_for_active_generation = state
                .active_route
                .as_ref()
                .is_some_and(|active| active.key == replaced.key);
            if !frozen_for_active_generation {
                retire_unlocked_for_consumer(&mut state.pending, replaced.key);
            }
            log::trace!(
                "native captured drag consumer={} registration={} disposition=Replaced",
                replaced.key.id,
                match registration {
                    NativeCapturedDragRegistration::Route => "Route",
                    NativeCapturedDragRegistration::Observer => "Observer",
                }
            );
        }
        let id = state.next_consumer_id;
        state.next_consumer_id = id
            .checked_add(1)
            .expect("native captured-drag consumer id overflowed");
        let key = ConsumerKey { id };
        let registered = RegisteredConsumer {
            key,
            callback,
            release_locker,
        };
        match registration {
            NativeCapturedDragRegistration::Route => state.route = Some(registered),
            NativeCapturedDragRegistration::Observer => state.observer = Some(registered),
        }
        drop(state);

        let subscription = Subscription::new(move || {
            if let Some(app) = app.upgrade() {
                app.unsubscribe_native_captured_drag(key);
            }
        });
        subscription
    }

    pub(super) fn prepared_route(&self) -> Option<RegisteredConsumer> {
        self.state.borrow().route.clone()
    }

    pub(super) fn lock_release(
        &self,
        event: &NativeCapturedDragEvent,
        source_window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<dyn Any>> {
        if event.phase != NativeCapturedDragPhase::Released {
            return None;
        }
        let locker = {
            let state = self.state.borrow();
            if Self::current_generation(&state) != Some(event.generation)
                || state.terminal_generation == Some(event.generation)
            {
                return None;
            }
            state.active_route.as_ref()?.release_locker.clone()?
        };
        Some(locker(event, source_window, cx))
    }

    pub(super) fn begin_generation(
        &self,
        authority: NativeCapturedDragAuthoritySnapshot,
        route_binding: NativeCapturedDragRouteBinding,
    ) {
        let mut state = self.state.borrow_mut();
        if let Some(replaced) = state.current_authority.replace(authority) {
            state.pending.retain(|envelope| {
                envelope.event.generation != replaced.generation
                    || envelope.event.phase.is_terminal()
            });
            log::trace!(
                "native captured drag generation={:?} disposition=Replaced",
                replaced.generation
            );
        }
        state.active_route = match route_binding {
            NativeCapturedDragRouteBinding::Unprepared => state.route.clone(),
            NativeCapturedDragRouteBinding::Prepared(route) => route,
        };
    }

    pub(super) fn enqueue(&self, event: NativeCapturedDragEvent) {
        let mut state = self.state.borrow_mut();
        enqueue_unlocked(&mut state, event);
    }

    pub(super) fn reserve_pointer_cancel(
        &self,
        sequence: NativeIngressSequence,
        source_window: WindowId,
        reason: PointerCancelReason,
    ) {
        let mut state = self.state.borrow_mut();
        state.pending_pointer_cancels.insert(
            sequence,
            ReservedPointerCancelFact {
                source_window,
                terminal: Vec::new(),
            },
        );
        let authority = state
            .current_authority
            .as_ref()
            .filter(|authority| authority.source_window == source_window)
            .cloned();
        let consumers = delivery_consumers(&state);
        let terminal = match authority {
            Some(authority)
                if !consumers.is_empty()
                    && state.terminal_generation != Some(authority.generation) =>
            {
                state.terminal_generation = Some(authority.generation);
                consumers
                    .into_iter()
                    .map(|registered| NativeCapturedDragEnvelope {
                        key: registered.key,
                        callback: registered.callback,
                        event: NativeCapturedDragEvent {
                            sequence,
                            generation: authority.generation,
                            source_window,
                            payload: authority.payload.clone(),
                            button: authority.button,
                            phase: NativeCapturedDragPhase::Cancelled(reason),
                            source_local_position: None,
                            physical_frame: None,
                            window_hit_stack: PlatformWindowHitStack::Unavailable,
                            route_lock: None,
                        },
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        state
            .pending_pointer_cancels
            .get_mut(&sequence)
            .expect("the reserved pointer cancel must remain present")
            .terminal = terminal;
    }

    pub(super) fn promote_pointer_cancel(&self, sequence: NativeIngressSequence) {
        let mut state = self.state.borrow_mut();
        let mut terminal = state
            .pending_pointer_cancels
            .get_mut(&sequence)
            .map(|reservation| std::mem::take(&mut reservation.terminal))
            .unwrap_or_default();
        if !terminal.is_empty() {
            state.pending.extend(terminal.drain(..));
        }
    }

    pub(super) fn start_is_unfenced(
        &self,
        source_window: WindowId,
        source_sequence: Option<NativeIngressSequence>,
    ) -> bool {
        let state = self.state.borrow();
        !state
            .pending_pointer_cancels
            .iter()
            .any(|(terminal_sequence, reservation)| {
                reservation.source_window == source_window
                    && source_sequence
                        .is_none_or(|source_sequence| *terminal_sequence > source_sequence)
            })
    }

    pub(super) fn finish_pointer_cancel(&self, sequence: NativeIngressSequence) {
        let mut state = self.state.borrow_mut();
        state.pending_pointer_cancels.remove(&sequence);
        clear_unowned_terminal_generation(&mut state);
    }

    fn current_generation(
        state: &NativeCapturedDragOutboxState,
    ) -> Option<NativeCapturedDragGeneration> {
        state
            .current_authority
            .as_ref()
            .map(|authority| authority.generation)
    }

    pub(super) fn unsubscribe(&self, key: ConsumerKey) {
        let mut state = self.state.borrow_mut();
        if state.route.as_ref().is_some_and(|entry| entry.key == key) {
            state.route = None;
        }
        if state
            .observer
            .as_ref()
            .is_some_and(|entry| entry.key == key)
        {
            state.observer = None;
        }
        if !state
            .active_route
            .as_ref()
            .is_some_and(|entry| entry.key == key)
        {
            retire_unlocked_for_consumer(&mut state.pending, key);
        }
    }

    pub(super) fn retire_generation(&self, generation: NativeCapturedDragGeneration) {
        let mut state = self.state.borrow_mut();
        if Self::current_generation(&state) == Some(generation) {
            state.current_authority = None;
            state.active_route = None;
        }
        state.pending.retain(|envelope| {
            envelope.event.generation != generation || envelope.event.phase.is_terminal()
        });
        log::trace!("native captured drag generation={generation:?} disposition=Retired");
    }

    pub(super) fn retire_sequence(&self, sequence: NativeIngressSequence) {
        let mut state = self.state.borrow_mut();
        state.pending.retain(|envelope| {
            envelope.event.sequence != sequence || envelope.event.phase.is_terminal()
        });
        state.pending_pointer_cancels.remove(&sequence);
        clear_unowned_terminal_generation(&mut state);
    }

    pub(super) fn retire_panicking_consumer(
        &self,
        key: ConsumerKey,
        generation: NativeCapturedDragGeneration,
    ) {
        let mut state = self.state.borrow_mut();
        state
            .pending
            .retain(|envelope| envelope.event.generation != generation);
        for reservation in state.pending_pointer_cancels.values_mut() {
            reservation
                .terminal
                .retain(|envelope| envelope.event.generation != generation);
        }
        if Self::current_generation(&state) == Some(generation) {
            state.current_authority = None;
            state.active_route = None;
        }
        if state.terminal_generation == Some(generation) {
            clear_unowned_terminal_generation(&mut state);
        }
        log::trace!(
            "native captured drag generation={generation:?} consumer={} disposition=ConsumerPanicked",
            key.id
        );
    }

    pub(super) fn begin_drain(&self) -> bool {
        let mut state = self.state.borrow_mut();
        if state.draining || state.pending.is_empty() {
            return false;
        }
        state.draining = true;
        true
    }

    pub(super) fn next_sequence(&self) -> Option<NativeIngressSequence> {
        self.state
            .borrow()
            .pending
            .front()
            .map(|envelope| envelope.event.sequence)
    }

    pub(super) fn pop_front(&self) -> Option<NativeCapturedDragEnvelope> {
        self.state.borrow_mut().pending.pop_front()
    }

    pub(super) fn push_front(&self, envelope: NativeCapturedDragEnvelope) {
        self.state.borrow_mut().pending.push_front(envelope);
    }

    pub(super) fn finish_delivery(&self, envelope: &NativeCapturedDragEnvelope) {
        if !envelope.event.phase.is_terminal() {
            return;
        }
        let mut state = self.state.borrow_mut();
        clear_unowned_terminal_generation(&mut state);
    }

    pub(super) fn finish_drain(&self) {
        self.state.borrow_mut().draining = false;
    }
}

fn clear_unowned_terminal_generation(state: &mut NativeCapturedDragOutboxState) {
    let Some(terminal_generation) = state.terminal_generation else {
        return;
    };
    let pending_terminal = state.pending.iter().any(|envelope| {
        envelope.event.generation == terminal_generation && envelope.event.phase.is_terminal()
    });
    let reserved_terminal = state.pending_pointer_cancels.values().any(|reservation| {
        reservation
            .terminal
            .iter()
            .any(|envelope| envelope.event.generation == terminal_generation)
    });
    if !pending_terminal && !reserved_terminal {
        state.terminal_generation = None;
    }
}

fn enqueue_unlocked(state: &mut NativeCapturedDragOutboxState, event: NativeCapturedDragEvent) {
    if state.terminal_generation == Some(event.generation) {
        log::trace!(
            "native captured drag sequence={} disposition=AfterTerminal",
            event.sequence
        );
        return;
    }
    if NativeCapturedDragOutbox::current_generation(state) != Some(event.generation) {
        log::trace!(
            "native captured drag sequence={} disposition=StaleGeneration",
            event.sequence
        );
        return;
    }
    let consumers = delivery_consumers(state);
    if consumers.is_empty() {
        log::trace!(
            "native captured drag sequence={} disposition=MissingConsumer",
            event.sequence
        );
        return;
    }
    let is_terminal = event.phase.is_terminal();
    let generation = event.generation;
    for registered in consumers {
        let envelope = NativeCapturedDragEnvelope {
            key: registered.key,
            callback: registered.callback,
            event: event.clone(),
        };
        if let Some(position) = state
            .pending
            .iter()
            .rposition(|previous| previous.is_coalescible_with(&envelope))
        {
            let replaced = state
                .pending
                .remove(position)
                .expect("checked captured-drag envelope must remain pending");
            log::trace!(
                "native captured drag sequence={} disposition=Coalesced into_sequence={}",
                replaced.event.sequence,
                envelope.event.sequence
            );
        }
        state.pending.push_back(envelope);
    }
    if is_terminal {
        state.terminal_generation = Some(generation);
    }
}

fn delivery_consumers(state: &NativeCapturedDragOutboxState) -> Vec<RegisteredConsumer> {
    state
        .active_route
        .iter()
        .chain(state.observer.iter())
        .cloned()
        .collect()
}

fn retire_unlocked_for_consumer(
    pending: &mut VecDeque<NativeCapturedDragEnvelope>,
    key: ConsumerKey,
) {
    pending.retain(|envelope| envelope.key != key || envelope.event.phase.is_terminal());
}

impl App {
    pub(crate) fn reserve_native_pointer_capture_release(
        &self,
        window_id: WindowId,
        captured_drag_generation: Option<NativeCapturedDragGeneration>,
    ) -> Option<super::NativePointerCaptureReleaseToken> {
        self.this.upgrade().map(|app| {
            app.reserve_native_pointer_capture_release(window_id, captured_drag_generation)
        })
    }

    /// Observes native captured-pointer facts for active drags in this application.
    ///
    /// Delivery occurs only after the source window transaction and the outer application borrow
    /// have completed. The observer receives immutable routing facts and cannot inject raw input.
    /// Registering another observer replaces the previous application-level observer.
    pub fn observe_native_captured_drag(
        &mut self,
        callback: impl Fn(NativeCapturedDragEvent, &mut App) + 'static,
    ) -> Subscription {
        let app = self
            .this
            .upgrade()
            .expect("live App must retain its AppCell while registering an observer");
        app.register_native_captured_drag_observer(Rc::new(callback))
    }

    /// Installs the application-level route consumer for captured native drag facts.
    ///
    /// Unlike ordinary observers, the route selected by a prepared drag start is frozen for that
    /// generation. Replacing or unsubscribing the installation only affects later generations.
    #[doc(hidden)]
    pub fn consume_native_captured_drag(
        &mut self,
        release_locker: impl Fn(&NativeCapturedDragEvent, &mut Window, &mut App) -> Arc<dyn Any>
        + 'static,
        callback: impl Fn(NativeCapturedDragEvent, &mut App) + 'static,
    ) -> Subscription {
        let app = self
            .this
            .upgrade()
            .expect("live App must retain its AppCell while registering a route consumer");
        app.register_native_captured_drag_route(Rc::new(callback), Rc::new(release_locker))
    }

    pub(crate) fn reserve_native_captured_drag_start(
        &mut self,
    ) -> NativeCapturedDragStartReservation {
        assert!(
            self.active_drag.is_none(),
            "a native captured-drag start cannot replace a live framework drag"
        );
        self.retire_native_captured_drag_authority();
        let generation = NativeCapturedDragGeneration(self.next_native_captured_drag_generation);
        self.next_native_captured_drag_generation = self
            .next_native_captured_drag_generation
            .checked_add(1)
            .expect("native captured-drag generation overflowed");
        NativeCapturedDragStartReservation::new(
            generation,
            self.window_update_provenance.native_source_ingress(),
            self.this.clone(),
        )
    }

    pub(crate) fn start_reserved_active_drag(
        &mut self,
        reservation: NativeCapturedDragStartReservation,
        drag: crate::AnyDrag,
    ) -> bool {
        if self.active_drag.is_some() || !self.window_handles.contains_key(&drag.window_id) {
            return false;
        }
        let Some(app) = self.this.upgrade() else {
            return false;
        };
        let source_sequence = match reservation.token.source_ingress() {
            Some((source_window, source_sequence)) if source_window == drag.window_id => {
                Some(source_sequence)
            }
            Some(_) => return false,
            None => None,
        };
        if !app.native_captured_drag_start_is_unfenced(drag.window_id, source_sequence) {
            return false;
        }
        let generation = reservation.token.generation();
        let authority = ActiveNativeCapturedDragAuthority {
            generation,
            source_window: drag.window_id,
            source: drag.source,
            button: drag.button,
            value: drag.value.clone(),
        };
        app.begin_native_captured_drag_generation(
            authority.snapshot(),
            reservation.token.route_binding(),
        );
        self.active_drag = Some(drag);
        self.active_native_captured_drag = Some(authority);
        assert_eq!(reservation.commit(), generation);
        true
    }

    pub(crate) fn start_active_drag(&mut self, drag: crate::AnyDrag) {
        let reservation = self.reserve_native_captured_drag_start();
        assert!(
            self.start_reserved_active_drag(reservation, drag),
            "active drag source window must remain live until start commit"
        );
    }

    /// Cancels the exact native captured-drag authority when it is still current.
    ///
    /// This is a narrow framework integration hook. Both the complete source-window identity and
    /// the non-forgeable drag generation must match; stale cancellation work is a strict no-op.
    #[doc(hidden)]
    pub fn cancel_native_captured_drag(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
        reason: PointerCancelReason,
    ) -> bool {
        self.cancel_native_captured_drag_with_release_continuation(
            source_window,
            generation,
            reason,
            None,
        )
        .is_some()
    }

    /// Cancels one exact native captured drag and invokes `on_terminal` after its native capture
    /// reaches a terminal release state.
    ///
    /// The callback is ordered through GPUI's post-borrow native work FIFO. It receives the
    /// complete source-window, drag, and release identities so a delayed G1 terminal cannot
    /// authorize effects for a replacement drag. Returning `None` means neither this exact drag
    /// nor an already-pending release for it exists.
    #[doc(hidden)]
    pub fn cancel_native_captured_drag_with_release_barrier(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
        reason: PointerCancelReason,
        on_terminal: impl FnOnce(
            NativeCapturedDragReleaseBarrier,
            NativeCapturedDragReleaseTerminal,
            &mut App,
        ) + 'static,
    ) -> Option<NativeCapturedDragReleaseBarrier> {
        let Some(app) = self.this.upgrade() else {
            return None;
        };
        let mut continuation: Option<NativeCapturedDragReleaseContinuation> =
            Some(Box::new(on_terminal));
        if let Some(barrier) = app.attach_native_captured_drag_release_continuation(
            source_window,
            generation,
            &mut continuation,
        ) {
            return Some(barrier);
        }
        self.cancel_native_captured_drag_with_release_continuation(
            source_window,
            generation,
            reason,
            continuation,
        )
    }

    fn cancel_native_captured_drag_with_release_continuation(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
        reason: PointerCancelReason,
        continuation: Option<NativeCapturedDragReleaseContinuation>,
    ) -> Option<NativeCapturedDragReleaseBarrier> {
        let Some((owner, button)) = self
            .active_native_captured_drag
            .as_ref()
            .filter(|authority| {
                authority.source_window == source_window
                    && authority.generation == generation
                    && self
                        .active_drag
                        .as_ref()
                        .is_some_and(|drag| authority.matches(drag))
            })
            .map(|authority| (authority.source, authority.button))
        else {
            return None;
        };
        let Some(app) = self.this.upgrade() else {
            return None;
        };
        if !app.reserve_native_captured_drag_cancel(source_window, generation, reason) {
            return None;
        }
        let (release, barrier) = match continuation {
            Some(continuation) => {
                app.reserve_native_captured_drag_release(source_window, generation, continuation)
            }
            None => {
                let release =
                    app.reserve_native_pointer_capture_release(source_window, Some(generation));
                let barrier = NativeCapturedDragReleaseBarrier::from_release_token(release)
                    .expect("captured-drag releases must retain their generation");
                (release, barrier)
            }
        };

        self.active_drag = None;
        self.active_native_captured_drag = None;
        app.retire_native_captured_drag_generation(generation);

        let cleanup = self.update_window_id(source_window, |_, window, cx| {
            window.queue_native_captured_drag_pointer_cancellation(
                owner, button, reason, release, cx,
            );
        });
        if cleanup.is_err() && self.windows.contains_key(source_window) {
            self.defer(move |cx| {
                cx.update_window_id(source_window, |_, window, cx| {
                    window.queue_native_captured_drag_pointer_cancellation(
                        owner, button, reason, release, cx,
                    );
                })
                .ok();
            });
        } else if cleanup.is_err() {
            app.abandon_native_pointer_capture_release(release);
        }
        Some(barrier)
    }

    pub(crate) fn cancel_active_native_captured_drag(
        &mut self,
        reason: PointerCancelReason,
    ) -> bool {
        let Some((source_window, generation)) = self
            .active_native_captured_drag
            .as_ref()
            .map(|authority| (authority.source_window, authority.generation))
        else {
            return false;
        };
        self.cancel_native_captured_drag(source_window, generation, reason)
    }

    pub(crate) fn reserve_active_native_captured_drag_pointer_cancellation(
        &mut self,
        source_window: WindowId,
        reason: PointerCancelReason,
    ) -> Option<crate::NativePointerCaptureReleaseToken> {
        let generation = self
            .active_native_captured_drag
            .as_ref()
            .filter(|authority| {
                authority.source_window == source_window
                    && self
                        .active_drag
                        .as_ref()
                        .is_some_and(|drag| authority.matches(drag))
            })?
            .generation;
        let app = self.this.upgrade()?;
        if !app.reserve_native_captured_drag_cancel(source_window, generation, reason) {
            return None;
        }
        let release = app.reserve_native_pointer_capture_release(source_window, Some(generation));
        self.active_drag = None;
        self.active_native_captured_drag = None;
        app.retire_native_captured_drag_generation(generation);
        Some(release)
    }

    #[cfg(test)]
    pub(crate) fn has_native_window_update_provenance(&self) -> bool {
        matches!(
            self.window_update_provenance,
            WindowUpdateProvenance::Native { .. }
        )
    }

    pub(crate) fn lock_native_captured_drag_event(
        &mut self,
        source_window: WindowId,
        source: &mut Window,
        event: &dyn Any,
    ) {
        let Some(sequence) = self
            .window_update_provenance
            .claim_native_captured_drag_fact(source_window)
        else {
            return;
        };
        let Some(drag) = self.active_drag.as_ref() else {
            return;
        };
        if drag.window_id != source_window {
            return;
        }
        let button = drag.button;
        let (phase, source_local_position) =
            if let Some(event) = event.downcast_ref::<MouseMoveEvent>() {
                if event.pressed_button != Some(button) {
                    return;
                }
                (NativeCapturedDragPhase::Moved, Some(event.position))
            } else if let Some(event) = event.downcast_ref::<MouseUpEvent>() {
                if event.button != button {
                    return;
                }
                (NativeCapturedDragPhase::Released, Some(event.position))
            } else if let Some(event) = event.downcast_ref::<PointerCancelEvent>() {
                (NativeCapturedDragPhase::Cancelled(event.reason), None)
            } else {
                return;
            };

        let Some(authority) = self.active_native_captured_drag.as_ref() else {
            return;
        };
        if !authority.matches(drag) {
            log::trace!("native captured drag sequence={sequence} disposition=StaleDragAuthority");
            return;
        }
        let generation = authority.generation;
        let payload = authority.value.clone();
        let button = authority.button;
        let physical_frame = matches!(
            phase,
            NativeCapturedDragPhase::Moved | NativeCapturedDragPhase::Released
        )
        .then(|| source.platform_window.native_pointer_physical_frame())
        .flatten();
        let window_hit_stack = physical_frame
            .map(|frame| self.platform.window_hit_stack_at(frame.global_position()))
            .unwrap_or(PlatformWindowHitStack::Unavailable);
        let mut captured = NativeCapturedDragEvent {
            sequence,
            generation,
            source_window,
            payload,
            button,
            phase,
            source_local_position,
            physical_frame,
            window_hit_stack,
            route_lock: None,
        };
        if let Some(app) = self.this.upgrade() {
            captured.route_lock = app.lock_native_captured_drag_release(&captured, source, self);
            app.enqueue_native_captured_drag(captured);
        }
    }

    pub(crate) fn retire_native_captured_drag_authority(&mut self) {
        let Some(authority) = self.active_native_captured_drag.take() else {
            return;
        };
        if let Some(app) = self.this.upgrade() {
            app.retire_native_captured_drag_generation(authority.generation);
        }
    }

    pub(super) fn settle_panicking_native_captured_drag(
        &mut self,
        generation: NativeCapturedDragGeneration,
        source_window: WindowId,
    ) {
        let matches = self
            .active_native_captured_drag
            .as_ref()
            .is_some_and(|authority| {
                authority.generation == generation && authority.source_window == source_window
            });
        if !matches {
            return;
        }

        self.settle_panicking_native_input_for_window(source_window);
    }

    pub(super) fn settle_panicking_native_input_for_window(&mut self, source_window: WindowId) {
        let generation = self
            .active_native_captured_drag
            .as_ref()
            .filter(|authority| authority.source_window == source_window)
            .map(|authority| authority.generation);

        let Some(generation) = generation else {
            return;
        };

        let delivered = self
            .update_window_id(source_window, |_, window, cx| {
                window.cancel_pointer_session(PointerCancelReason::CaptureRevoked, cx);
            })
            .is_ok();
        if delivered {
            return;
        }

        // A removed source cannot receive a real cancellation. Retire the exact generation so a
        // stale consumer failure cannot fence a later drag, and clear any remaining local state.
        if self
            .active_native_captured_drag
            .as_ref()
            .is_some_and(|authority| authority.generation == generation)
        {
            self.active_drag = None;
            self.active_native_captured_drag = None;
            if let Some(app) = self.this.upgrade() {
                app.retire_native_captured_drag_generation(generation);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlatformWindowHit, PlatformWindowPhysicalCoverage, point, size};
    use slotmap::SlotMap;

    fn window_id() -> WindowId {
        let mut windows = SlotMap::<WindowId, ()>::with_key();
        windows.insert(())
    }

    #[test]
    fn native_ingress_sequence_can_be_claimed_only_once_by_its_source_dispatch() {
        let mut windows = SlotMap::<WindowId, ()>::with_key();
        let source = windows.insert(());
        let other = windows.insert(());
        let sequence = NativeIngressSequence::new(17);
        let mut provenance = WindowUpdateProvenance::Native {
            source_window: source,
            sequence,
            captured_drag_fact_claimed: false,
        };

        assert_eq!(provenance.claim_native_captured_drag_fact(other), None);
        assert_eq!(
            provenance.claim_native_captured_drag_fact(source),
            Some(sequence)
        );
        assert_eq!(provenance.claim_native_captured_drag_fact(source), None);
    }

    #[test]
    fn committed_start_activates_the_prepared_consumer_without_revoking_it() {
        let generation = NativeCapturedDragGeneration(41);
        let reservation = NativeCapturedDragStartReservation::new(generation, None, Weak::new());
        let consumer = reservation.token().prepare_consumer();

        assert!(!consumer.is_active());
        assert!(!consumer.is_revoked());
        assert_eq!(reservation.commit(), generation);
        assert!(consumer.is_active());
        assert!(!consumer.is_revoked());
    }

    #[test]
    fn dropped_start_revokes_the_prepared_consumer_without_activating_it() {
        let reservation = NativeCapturedDragStartReservation::new(
            NativeCapturedDragGeneration(42),
            None,
            Weak::new(),
        );
        let consumer = reservation.token().prepare_consumer();

        drop(reservation);
        assert!(!consumer.is_active());
        assert!(consumer.is_revoked());
    }

    #[test]
    #[should_panic(expected = "captured-drag start accepts only one prepared consumer")]
    fn captured_drag_start_accepts_only_one_prepared_consumer() {
        let reservation = NativeCapturedDragStartReservation::new(
            NativeCapturedDragGeneration(43),
            None,
            Weak::new(),
        );
        let token = reservation.token();
        let _first = token.prepare_consumer();
        let _second = token.prepare_consumer();
    }

    fn event(
        sequence: u64,
        generation: NativeCapturedDragGeneration,
        source_window: WindowId,
        phase: NativeCapturedDragPhase,
        hits: PlatformWindowHitStack,
    ) -> NativeCapturedDragEvent {
        NativeCapturedDragEvent {
            sequence: NativeIngressSequence::new(sequence),
            generation,
            source_window,
            payload: Arc::new("payload"),
            button: crate::MouseButton::Left,
            phase,
            source_local_position: (!matches!(phase, NativeCapturedDragPhase::Cancelled(_)))
                .then(Point::default),
            physical_frame: None,
            window_hit_stack: hits,
            route_lock: None,
        }
    }

    fn authority(
        generation: NativeCapturedDragGeneration,
        source_window: WindowId,
    ) -> NativeCapturedDragAuthoritySnapshot {
        NativeCapturedDragAuthoritySnapshot {
            generation,
            source_window,
            button: crate::MouseButton::Left,
            payload: Arc::new("payload"),
        }
    }

    #[test]
    fn adjacent_moves_only_coalesce_for_the_same_route_source_and_generation() {
        let outbox = NativeCapturedDragOutbox::default();
        let generation = NativeCapturedDragGeneration(7);
        let source = window_id();
        let callback: NativeCapturedDragConsumer = Rc::new(|_, _| {});
        let subscription = outbox.register_observer(Weak::new(), callback);
        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );

        outbox.enqueue(event(
            1,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        outbox.enqueue(event(
            2,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        assert_eq!(outbox.state.borrow().pending.len(), 1);
        assert_eq!(outbox.state.borrow().pending[0].event.sequence.ordinal(), 2);

        let sampled_point = point(DevicePixels(0), DevicePixels(0));
        let distinct_route = PlatformWindowHitStack::try_available(
            sampled_point,
            vec![PlatformWindowHit::OpaqueBarrier {
                coverage: PlatformWindowPhysicalCoverage::try_new(Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(1), DevicePixels(1)),
                ))
                .expect("test coverage must be representable"),
            }],
        )
        .expect("test observation must cover its sampled point");
        outbox.enqueue(event(
            3,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            distinct_route,
        ));
        assert_eq!(outbox.state.borrow().pending.len(), 2);
        drop(subscription);
    }

    #[test]
    fn terminal_fact_survives_unsubscribe_and_blocks_later_cleanup() {
        let outbox = NativeCapturedDragOutbox::default();
        let generation = NativeCapturedDragGeneration(11);
        let source = window_id();
        let callback: NativeCapturedDragConsumer = Rc::new(|_, _| {});
        let subscription = outbox.register_observer(Weak::new(), callback);
        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );

        outbox.enqueue(event(
            4,
            generation,
            source,
            NativeCapturedDragPhase::Released,
            PlatformWindowHitStack::Unavailable,
        ));
        let key = outbox
            .state
            .borrow()
            .observer
            .as_ref()
            .expect("consumer should remain registered")
            .key;
        outbox.unsubscribe(key);
        outbox.enqueue(event(
            5,
            generation,
            source,
            NativeCapturedDragPhase::Cancelled(PointerCancelReason::PlatformCaptureLost),
            PlatformWindowHitStack::Unavailable,
        ));

        let state = outbox.state.borrow();
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.pending[0].event.sequence.ordinal(), 4);
        drop(subscription);
    }

    #[test]
    fn consumer_replacement_retires_unlocked_moves_but_not_a_locked_terminal() {
        let outbox = NativeCapturedDragOutbox::default();
        let generation = NativeCapturedDragGeneration(13);
        let source = window_id();
        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );
        let first = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        outbox.enqueue(event(
            1,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        let second = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        assert!(outbox.state.borrow().pending.is_empty());

        outbox.enqueue(event(
            2,
            generation,
            source,
            NativeCapturedDragPhase::Released,
            PlatformWindowHitStack::Unavailable,
        ));
        let third = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        assert_eq!(outbox.state.borrow().pending.len(), 1);
        assert_eq!(outbox.state.borrow().pending[0].event.sequence.ordinal(), 2);
        drop((first, second, third));
    }

    #[test]
    fn retired_generation_rejects_a_late_stale_fact() {
        let outbox = NativeCapturedDragOutbox::default();
        let generation = NativeCapturedDragGeneration(17);
        let source = window_id();
        let subscription = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );
        outbox.retire_generation(generation);
        outbox.enqueue(event(
            3,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));

        assert!(outbox.state.borrow().pending.is_empty());
        drop(subscription);
    }

    #[test]
    fn retiring_a_panicked_source_fact_keeps_the_app_observer_registered() {
        let outbox = NativeCapturedDragOutbox::default();
        let generation = NativeCapturedDragGeneration(19);
        let source = window_id();
        let subscription = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );
        outbox.enqueue(event(
            7,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        outbox.retire_sequence(NativeIngressSequence::new(7));

        assert!(outbox.state.borrow().observer.is_some());
        outbox.enqueue(event(
            8,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        assert_eq!(outbox.state.borrow().pending.len(), 1);
        assert_eq!(outbox.state.borrow().pending[0].event.sequence.ordinal(), 8);
        drop(subscription);
    }

    #[test]
    fn reserved_pointer_cancel_freezes_g1_and_fences_its_older_start() {
        let outbox = NativeCapturedDragOutbox::default();
        let source = window_id();
        let g1 = NativeCapturedDragGeneration(23);
        let g2 = NativeCapturedDragGeneration(24);
        let subscription = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        outbox.begin_generation(
            authority(g1, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );

        let source_sequence = NativeIngressSequence::new(40);
        let terminal_sequence = NativeIngressSequence::new(41);
        outbox.reserve_pointer_cancel(
            terminal_sequence,
            source,
            PointerCancelReason::PlatformCaptureLost,
        );

        assert!(!outbox.start_is_unfenced(source, Some(source_sequence)));
        assert!(outbox.state.borrow().pending.is_empty());
        outbox.begin_generation(
            authority(g2, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );
        outbox.promote_pointer_cancel(terminal_sequence);
        let terminal = outbox
            .pop_front()
            .expect("the exact G1 terminal must remain locked after G2 begins");
        assert_eq!(terminal.event.sequence(), terminal_sequence);
        assert_eq!(terminal.event.generation(), g1);
        assert_eq!(
            terminal.event.phase(),
            NativeCapturedDragPhase::Cancelled(PointerCancelReason::PlatformCaptureLost)
        );
        assert_eq!(terminal.event.payload::<&'static str>(), Some(&"payload"));
        assert_eq!(terminal.event.physical_frame(), None);
        assert_eq!(
            terminal.event.window_hit_stack(),
            &PlatformWindowHitStack::Unavailable
        );

        outbox.finish_pointer_cancel(terminal_sequence);
        assert!(outbox.start_is_unfenced(source, Some(terminal_sequence)));
        drop(subscription);
    }

    #[test]
    fn panicking_delivery_retires_only_g1_and_keeps_observer_for_g2() {
        let outbox = NativeCapturedDragOutbox::default();
        let source = window_id();
        let g1 = NativeCapturedDragGeneration(25);
        let g2 = NativeCapturedDragGeneration(26);
        let subscription = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        outbox.begin_generation(
            authority(g1, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );
        outbox.enqueue(event(
            50,
            g1,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        let failed = outbox
            .pop_front()
            .expect("the G1 delivery should be available to fail");

        outbox.retire_panicking_consumer(failed.key, g1);
        assert!(outbox.state.borrow().observer.is_some());
        assert!(outbox.state.borrow().current_authority.is_none());

        outbox.begin_generation(
            authority(g2, source),
            NativeCapturedDragRouteBinding::Unprepared,
        );
        outbox.enqueue(event(
            51,
            g2,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));
        let state = outbox.state.borrow();
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.pending[0].event.generation(), g2);
        assert_eq!(state.pending[0].event.sequence().ordinal(), 51);
        drop(state);
        drop(subscription);
    }

    #[test]
    fn prepared_route_remains_frozen_when_the_live_route_is_replaced() {
        let outbox = NativeCapturedDragOutbox::default();
        let source = window_id();
        let generation = NativeCapturedDragGeneration(27);
        let first = outbox.register_route(
            Weak::new(),
            Rc::new(|_, _| {}),
            Rc::new(|_, _, _| Arc::new(())),
        );
        let prepared_route = outbox
            .prepared_route()
            .expect("the installed route must be available to a prepared start");
        let first_key = prepared_route.key;

        let second = outbox.register_route(
            Weak::new(),
            Rc::new(|_, _| {}),
            Rc::new(|_, _, _| Arc::new(())),
        );
        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Prepared(Some(prepared_route)),
        );
        outbox.enqueue(event(
            61,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));

        let state = outbox.state.borrow();
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.pending[0].key, first_key);
        assert_eq!(
            state.active_route.as_ref().map(|route| route.key),
            Some(first_key)
        );
        drop(state);
        drop((first, second));
    }

    #[test]
    fn observer_replacement_cannot_replace_the_frozen_route() {
        let outbox = NativeCapturedDragOutbox::default();
        let source = window_id();
        let generation = NativeCapturedDragGeneration(28);
        let route = outbox.register_route(
            Weak::new(),
            Rc::new(|_, _| {}),
            Rc::new(|_, _, _| Arc::new(())),
        );
        let prepared_route = outbox
            .prepared_route()
            .expect("the installed route must be available to a prepared start");
        let observer = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        let replacement = outbox.register_observer(Weak::new(), Rc::new(|_, _| {}));
        let observer_key = outbox
            .state
            .borrow()
            .observer
            .as_ref()
            .expect("the replacement observer must remain installed")
            .key;

        outbox.begin_generation(
            authority(generation, source),
            NativeCapturedDragRouteBinding::Prepared(Some(prepared_route.clone())),
        );
        outbox.enqueue(event(
            62,
            generation,
            source,
            NativeCapturedDragPhase::Moved,
            PlatformWindowHitStack::Unavailable,
        ));

        let state = outbox.state.borrow();
        assert_eq!(state.pending.len(), 2);
        assert_eq!(state.pending[0].key, prepared_route.key);
        assert_eq!(state.pending[1].key, observer_key);
        drop(state);
        drop((route, observer, replacement));
    }
}
