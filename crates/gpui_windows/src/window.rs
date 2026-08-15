#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    num::NonZeroIsize,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    rc::Rc,
    str::FromStr,
    sync::{
        Arc, Once,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ::open_gpui_util::ResultExt;
use anyhow::{Context as _, Result};
use futures::channel::oneshot::{self, Receiver};
use raw_window_handle as rwh;
use smallvec::SmallVec;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Dwm::*,
        Graphics::Gdi::*,
        System::{
            Com::*, Diagnostics::Debug::MessageBeep, LibraryLoader::*, Ole::*, SystemServices::*,
        },
        UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::direct_manipulation::DirectManipulationHandler;
use crate::*;
use open_gpui::*;

pub(crate) struct WindowsWindow(pub Rc<WindowsWindowInner>);

static NEXT_EMERGENCY_PRESENTATION_SHUTDOWN_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeWindowLifecycle {
    Live,
    Destroying,
    Destroyed,
}

const MAX_PROVISIONAL_Z_ORDER_WINDOWS: usize = 4096;

#[derive(Clone, Copy, Debug)]
struct NativeZOrderWindowIdentity {
    hwnd: HWND,
    thread_id: u32,
    process_id: u32,
    registered: Option<RegisteredWindow>,
}

impl PartialEq for NativeZOrderWindowIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.hwnd == other.hwnd
            && self.thread_id == other.thread_id
            && self.process_id == other.process_id
            && match (self.registered, other.registered) {
                (Some(left), Some(right)) => left.matches(right),
                (None, None) => true,
                (Some(_), None) | (None, Some(_)) => false,
            }
    }
}

impl Eq for NativeZOrderWindowIdentity {}

#[derive(Clone, Debug)]
struct PreparedProvisionalZOrderBand {
    point: Point<DevicePixels>,
    current: RegisteredWindow,
    peers: Arc<[RegisteredWindow]>,
    barrier: Option<NativeZOrderWindowIdentity>,
}

impl PreparedProvisionalZOrderBand {
    fn insert_after(&self) -> HWND {
        self.barrier.map_or(HWND_TOP, |barrier| barrier.hwnd)
    }

    fn is_peer(&self, identity: NativeZOrderWindowIdentity) -> bool {
        identity
            .registered
            .is_some_and(|registered| self.peers.iter().any(|peer| peer.matches(registered)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProvisionalPlacementRollback {
    rect: NativeRect,
    previous_above: Option<NativeZOrderWindowIdentity>,
    was_topmost: bool,
    physical_geometry: PlatformWindowPhysicalGeometry,
}

#[derive(Debug)]
struct AppliedProvisionalFinalPlacement {
    facts: WindowProvisionalPlacementNativeFacts,
    platform_facts: WindowPlatformFacts,
    rollback: ProvisionalPlacementRollback,
    applied: ProvisionalPlacementRollback,
    applied_epoch: u64,
}

impl AppliedProvisionalFinalPlacement {
    fn facts(&self) -> WindowProvisionalPlacementNativeFacts {
        self.facts
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        self.platform_facts.clone()
    }

    fn commit(self) -> WindowProvisionalPlacementNativeFacts {
        self.facts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProvisionalPlacementCompensationComponent {
    Restored,
    AuthorityChanged,
    Unproven,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProvisionalPlacementCompensationAuthority {
    RegisteredOnly,
    #[cfg(test)]
    ImmediateNativeStack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProvisionalPlacementCompensation {
    rect: ProvisionalPlacementCompensationComponent,
    z_order: ProvisionalPlacementCompensationComponent,
    physical_geometry: ProvisionalPlacementCompensationComponent,
}

impl ProvisionalPlacementCompensation {
    const fn authority_changed() -> Self {
        Self {
            rect: ProvisionalPlacementCompensationComponent::AuthorityChanged,
            z_order: ProvisionalPlacementCompensationComponent::AuthorityChanged,
            physical_geometry: ProvisionalPlacementCompensationComponent::AuthorityChanged,
        }
    }

    const fn fully_restored(self) -> bool {
        matches!(
            (self.rect, self.z_order),
            (
                ProvisionalPlacementCompensationComponent::Restored,
                ProvisionalPlacementCompensationComponent::Restored
            )
        ) && matches!(
            self.physical_geometry,
            ProvisionalPlacementCompensationComponent::Restored
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl NativeRect {
    fn try_from_rect(rect: RECT) -> Option<Self> {
        (rect.left < rect.right && rect.top < rect.bottom).then_some(Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        })
    }

    fn contains(self, point: Point<DevicePixels>) -> bool {
        point.x.0 >= self.left
            && point.x.0 < self.right
            && point.y.0 >= self.top
            && point.y.0 < self.bottom
    }

    fn intersection(self, other: Self) -> Option<Self> {
        Self::try_from_rect(RECT {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        })
    }

    fn subtract(self, cover: Self, output: &mut Vec<Self>) {
        let Some(overlap) = self.intersection(cover) else {
            output.push(self);
            return;
        };
        for remainder in [
            RECT {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: overlap.top,
            },
            RECT {
                left: self.left,
                top: overlap.bottom,
                right: self.right,
                bottom: self.bottom,
            },
            RECT {
                left: self.left,
                top: overlap.top,
                right: overlap.left,
                bottom: overlap.bottom,
            },
            RECT {
                left: overlap.right,
                top: overlap.top,
                right: self.right,
                bottom: overlap.bottom,
            },
        ] {
            if let Some(remainder) = Self::try_from_rect(remainder) {
                output.push(remainder);
            }
        }
    }
}

struct CreatedNativeWindowGuard {
    hwnd: Option<HWND>,
}

impl CreatedNativeWindowGuard {
    fn new(hwnd: HWND) -> Self {
        Self { hwnd: Some(hwnd) }
    }

    fn commit(mut self) {
        self.hwnd = None;
    }
}

impl Drop for CreatedNativeWindowGuard {
    fn drop(&mut self) {
        let Some(hwnd) = self.hwnd.take() else {
            return;
        };
        // This guard exists only for the internal-invariant path where
        // `WM_NCCREATE` did not produce a `WindowsWindowInner`. Once an inner
        // exists, `ConstructionRetirementGuard` must own retirement so the
        // renderer is quiesced before the HWND reaches `WM_NCDESTROY`.
        if unsafe { IsWindow(Some(hwnd)).as_bool() } {
            unsafe {
                DestroyWindow(hwnd)
                    .context("rolling back partially constructed native window")
                    .log_err();
            }
        }
    }
}

struct ProvisionalRevealVisibilityGuard {
    hwnd: Option<HWND>,
}

impl ProvisionalRevealVisibilityGuard {
    fn new(hwnd: HWND) -> Self {
        Self { hwnd: Some(hwnd) }
    }

    fn commit(mut self) {
        self.hwnd = None;
    }
}

impl Drop for ProvisionalRevealVisibilityGuard {
    fn drop(&mut self) {
        let Some(hwnd) = self.hwnd.take() else {
            return;
        };
        if unsafe { IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() } {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            if unsafe { IsWindowVisible(hwnd).as_bool() } {
                log::error!(
                    "failed to compensate a rejected provisional reveal by hiding its native window"
                );
            }
        }
    }
}

/// Owns a native window while fallible post-`CreateWindowExW` construction is
/// still in progress.
///
/// `WM_NCCREATE` installs an HWND-owned `Rc<WindowsWindowInner>` before
/// `CreateWindowExW` returns. Retiring that HWND through a raw `DestroyWindow`
/// after this point would allow `WM_NCDESTROY` to run before DirectX teardown.
/// This guard instead preserves the `Rc` until the exact presentation shutdown
/// protocol has reached native terminal, including across a failed first
/// `DestroyWindow` attempt.
struct ConstructionRetirementGuard {
    inner: Option<Rc<WindowsWindowInner>>,
}

impl ConstructionRetirementGuard {
    fn new(inner: Rc<WindowsWindowInner>) -> Self {
        Self { inner: Some(inner) }
    }

    fn inner(&self) -> &Rc<WindowsWindowInner> {
        self.inner
            .as_ref()
            .expect("a construction-retirement guard must own its window before commit")
    }

    fn commit(mut self) -> WindowsWindow {
        WindowsWindow(
            self.inner
                .take()
                .expect("a construction-retirement guard must own its window before commit"),
        )
    }

    fn retire(inner: &WindowsWindowInner) -> bool {
        inner.destroy_native_window()
    }
}

impl Drop for ConstructionRetirementGuard {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };

        let _ = Self::retire(&inner);
        if inner.is_native_window_terminal() {
            return;
        }

        if let Some(coordinator) = inner.native_retirement_coordinator.upgrade() {
            log::error!(
                "native window construction rollback did not reach native terminal; transferring the exact native window to the platform retirement coordinator"
            );
            coordinator.enqueue_construction_native_window(inner);
        } else {
            log::error!(
                "native window construction rollback lost its platform retirement coordinator; retaining the managed native owner fail-closed"
            );
            std::mem::forget(inner);
        }
    }
}

struct ModalParentGuard {
    hwnd: Option<HWND>,
}

impl ModalParentGuard {
    fn acquire(parent_hwnd: Option<HWND>) -> Self {
        let hwnd = parent_hwnd.filter(|parent_hwnd| unsafe {
            if !IsWindowEnabled(*parent_hwnd).as_bool() {
                return false;
            }
            let _ = EnableWindow(*parent_hwnd, false);
            true
        });
        Self { hwnd }
    }

    fn owns_disable(&self) -> bool {
        self.hwnd.is_some()
    }

    fn commit(mut self) {
        self.hwnd = None;
    }
}

impl Drop for ModalParentGuard {
    fn drop(&mut self) {
        let Some(hwnd) = self.hwnd.take() else {
            return;
        };
        unsafe {
            let _ = EnableWindow(hwnd, true);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

impl std::ops::Deref for WindowsWindow {
    type Target = WindowsWindowInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct WindowsWindowState {
    pub origin: Cell<Point<Pixels>>,
    pub logical_size: Cell<Size<Pixels>>,
    pub min_size: Option<Size<Pixels>>,
    pub fullscreen_restore_bounds: Cell<Bounds<Pixels>>,
    pub border_offset: WindowBorderOffset,
    pub appearance: Cell<WindowAppearance>,
    pub background_appearance: Cell<WindowBackgroundAppearance>,
    pub scale_factor: Cell<f32>,
    pub restore_from_minimized: Cell<Option<Box<dyn FnMut(RequestFrameOptions)>>>,

    pub callbacks: Callbacks,
    pub input_handler: PlatformInputHandlerSlot,
    pub ime_enabled: Cell<bool>,
    pub pending_surrogate: Cell<Option<u16>>,
    pub last_reported_modifiers: Cell<Option<Modifiers>>,
    pub last_reported_capslock: Cell<Option<Capslock>>,
    pub hovered: Cell<bool>,
    /// Last logical client position carried by an exact native pointer callback.
    pub(crate) last_client_pointer_position: Cell<Option<Point<Pixels>>>,
    pub direct_manipulation: DirectManipulationHandler,

    pub renderer: RefCell<DirectXRenderer>,

    pub click_state: ClickState,
    pub current_cursor: Cell<Option<HCURSOR>>,
    /// Shared with [`WindowsPlatformState::cursor_visible`].
    pub cursor_visible: Arc<AtomicBool>,
    /// Client-area pointer session and its native capture ownership.
    pub pointer_capture: Cell<WindowsPointerCaptureState>,
    /// Binds deferred framework release tokens to the native capture session they first target.
    pub native_pointer_capture_release: WindowsNativePointerCaptureReleaseState,
    native_pointer_physical_frame: WindowsNativePointerPhysicalFrameState,
    /// Prevents terminal pointer cancellation from re-entering the core input callback.
    pub input_dispatch: Cell<WindowsInputDispatchState>,
    pub pressed_caption_button: Cell<Option<WindowsCaptionButtonAction>>,
    accepts_pointer_input: Cell<bool>,
    pointer_input_observation_generation: Cell<u64>,
    activation_policy: Cell<WindowActivationPolicy>,
    taskbar_visible: bool,

    pub display: Cell<WindowsDisplay>,
    pub(crate) display_topology_generation: Cell<u64>,
    last_validated_platform_facts: RefCell<Option<WindowPlatformFacts>>,
    /// Flag to instruct the `VSyncProvider` thread to invalidate the directx devices
    /// as resizing them has failed, causing us to have lost at least the render target.
    pub invalidate_devices: Arc<AtomicBool>,
    native_placement_epoch: Cell<u64>,
    placement_mutation_generation: Cell<Option<u64>>,
    pointer_input_mutation_generation: Cell<Option<u64>>,
    activation_policy_mutation_generation: Cell<Option<u64>>,
    deferred_placement_mutation: Cell<Option<DeferredWindowPlacementMutation>>,
    #[cfg(test)]
    pub(crate) fail_next_pointer_input_frame_change: Cell<bool>,
    #[cfg(test)]
    pub(crate) fail_next_activation_policy_frame_change: Cell<bool>,
    #[cfg(test)]
    pub(crate) panic_next_pointer_cancel_reservation: Cell<bool>,
    #[cfg(test)]
    pub(crate) replace_next_pointer_capture_acquisition_with: Cell<Option<HWND>>,
    #[cfg(test)]
    pub(crate) pointer_capture_release_history: RefCell<Vec<u64>>,
    fullscreen: Cell<Option<StyleAndBounds>>,
    initial_placement: Cell<Option<WindowOpenStatus>>,
    hwnd: HWND,
    pub(crate) a11y: RefCell<Option<A11yState>>,
}

#[derive(Default)]
struct WindowsNativePointerPhysicalFrameState {
    current: Cell<Option<PlatformNativePointerPhysicalFrame>>,
    invalidation_epoch: Cell<u64>,
}

impl WindowsNativePointerPhysicalFrameState {
    fn invalidate_active_scopes(&self) {
        self.invalidation_epoch
            .set(self.invalidation_epoch.get().wrapping_add(1));
        self.current.set(None);
    }
}

pub(crate) struct WindowsNativePointerPhysicalFrameScope<'a> {
    state: &'a WindowsNativePointerPhysicalFrameState,
    previous: Option<PlatformNativePointerPhysicalFrame>,
    frame: Option<PlatformNativePointerPhysicalFrame>,
    entry_epoch: u64,
}

impl WindowsNativePointerPhysicalFrameScope<'_> {
    fn enter<'a>(
        state: &'a WindowsNativePointerPhysicalFrameState,
        frame: Option<PlatformNativePointerPhysicalFrame>,
    ) -> WindowsNativePointerPhysicalFrameScope<'a> {
        let previous = state.current.replace(frame);
        WindowsNativePointerPhysicalFrameScope {
            state,
            previous,
            frame,
            entry_epoch: state.invalidation_epoch.get(),
        }
    }

    pub(crate) fn frame(&self) -> Option<PlatformNativePointerPhysicalFrame> {
        self.frame
    }
}

impl Drop for WindowsNativePointerPhysicalFrameScope<'_> {
    fn drop(&mut self) {
        if self.state.invalidation_epoch.get() == self.entry_epoch {
            self.state.current.set(self.previous);
        } else {
            self.state.current.set(None);
        }
    }
}

pub(crate) struct WindowsWindowInner {
    pub(crate) hwnd: HWND,
    native_window_lifecycle: Cell<NativeWindowLifecycle>,
    drag_drop_registered: Cell<bool>,
    show_on_initial_presentation: Cell<bool>,
    provisional_session: Option<WindowProvisionalSession>,
    provisional_reveal_generation: Cell<Option<u64>>,
    presentation_shutdown_ticket: RefCell<Option<WindowPresentationShutdownTicket>>,
    creation_facts: WindowCreationFacts,
    drop_target_helper: IDropTargetHelper,
    pub(crate) state: WindowsWindowState,
    system_settings: WindowsSystemSettings,
    pub(crate) handle: AnyWindowHandle,
    pub(crate) hide_title_bar: bool,
    pub(crate) is_movable: bool,
    pub(crate) executor: ForegroundExecutor,
    pub(crate) validation_number: usize,
    pub(crate) registration: RegisteredWindow,
    pub(crate) recovered_directx_devices: Arc<parking_lot::RwLock<Option<DirectXDevices>>>,
    pub(crate) main_receiver: PriorityQueueReceiver<RunnableVariant>,
    pub(crate) platform_window_handle: HWND,
    raw_window_handles: std::sync::Weak<RegisteredWindows>,
    pub(crate) native_retirement_coordinator: std::rc::Weak<WindowsPlatformInner>,
    owner_hwnd: Option<HWND>,
    modal_parent_disabled: Cell<bool>,
    #[cfg(test)]
    lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
}

impl WindowsWindowState {
    fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        window_params: &CREATESTRUCTW,
        current_cursor: Option<HCURSOR>,
        cursor_visible: Arc<AtomicBool>,
        display: WindowsDisplay,
        display_topology_generation: u64,
        min_size: Option<Size<Pixels>>,
        appearance: WindowAppearance,
        disable_direct_composition: bool,
        invalidate_devices: Arc<AtomicBool>,
        accepts_pointer_input: bool,
        activation_policy: WindowActivationPolicy,
        taskbar_visible: bool,
    ) -> Result<Self> {
        let scale_factor = {
            let monitor_dpi = unsafe { GetDpiForWindow(hwnd) } as f32;
            monitor_dpi / USER_DEFAULT_SCREEN_DPI as f32
        };
        let origin = logical_point(window_params.x as f32, window_params.y as f32, scale_factor);
        let logical_size = {
            let physical_size = size(
                DevicePixels(window_params.cx),
                DevicePixels(window_params.cy),
            );
            physical_size.to_pixels(scale_factor)
        };
        let fullscreen_restore_bounds = Bounds {
            origin,
            size: logical_size,
        };
        let border_offset = WindowBorderOffset::default();
        let restore_from_minimized = None;
        let renderer = DirectXRenderer::new(hwnd, directx_devices, disable_direct_composition)
            .context("Creating DirectX renderer")?;
        let callbacks = Callbacks::default();
        let input_handler = PlatformInputHandlerSlot::default();
        let pending_surrogate = None;
        let last_reported_modifiers = None;
        let last_reported_capslock = None;
        let hovered = false;
        let click_state = ClickState::new();
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let native_pointer_capture_release = WindowsNativePointerCaptureReleaseState::default();
        let native_pointer_physical_frame = WindowsNativePointerPhysicalFrameState::default();
        let input_dispatch = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = None;
        let fullscreen = None;
        let initial_placement = None;
        let native_placement_epoch = Cell::new(0);
        let placement_mutation_generation = Cell::new(None);
        let pointer_input_mutation_generation = Cell::new(None);
        let pointer_input_observation_generation = Cell::new(1);
        let activation_policy_mutation_generation = Cell::new(None);
        let deferred_placement_mutation = Cell::new(None);

        let direct_manipulation = DirectManipulationHandler::new(hwnd, scale_factor)
            .context("initializing Direct Manipulation")?;

        Ok(Self {
            origin: Cell::new(origin),
            logical_size: Cell::new(logical_size),
            fullscreen_restore_bounds: Cell::new(fullscreen_restore_bounds),
            border_offset,
            appearance: Cell::new(appearance),
            background_appearance: Cell::new(WindowBackgroundAppearance::Opaque),
            scale_factor: Cell::new(scale_factor),
            restore_from_minimized: Cell::new(restore_from_minimized),
            min_size,
            callbacks,
            input_handler,
            ime_enabled: Cell::new(true),
            pending_surrogate: Cell::new(pending_surrogate),
            last_reported_modifiers: Cell::new(last_reported_modifiers),
            last_reported_capslock: Cell::new(last_reported_capslock),
            hovered: Cell::new(hovered),
            last_client_pointer_position: Cell::new(None),
            renderer: RefCell::new(renderer),
            click_state,
            current_cursor: Cell::new(current_cursor),
            cursor_visible,
            pointer_capture,
            native_pointer_capture_release,
            native_pointer_physical_frame,
            input_dispatch,
            pressed_caption_button: Cell::new(pressed_caption_button),
            accepts_pointer_input: Cell::new(accepts_pointer_input),
            pointer_input_observation_generation,
            activation_policy: Cell::new(activation_policy),
            taskbar_visible,
            display: Cell::new(display),
            display_topology_generation: Cell::new(display_topology_generation),
            last_validated_platform_facts: RefCell::new(None),
            native_placement_epoch,
            placement_mutation_generation,
            pointer_input_mutation_generation,
            activation_policy_mutation_generation,
            deferred_placement_mutation,
            #[cfg(test)]
            fail_next_pointer_input_frame_change: Cell::new(false),
            #[cfg(test)]
            fail_next_activation_policy_frame_change: Cell::new(false),
            #[cfg(test)]
            panic_next_pointer_cancel_reservation: Cell::new(false),
            #[cfg(test)]
            replace_next_pointer_capture_acquisition_with: Cell::new(None),
            #[cfg(test)]
            pointer_capture_release_history: RefCell::new(Vec::new()),
            fullscreen: Cell::new(fullscreen),
            initial_placement: Cell::new(initial_placement),
            hwnd,
            invalidate_devices,
            direct_manipulation,
            a11y: RefCell::new(None),
        })
    }

    #[inline]
    pub(crate) fn is_fullscreen(&self) -> bool {
        self.fullscreen.get().is_some()
    }

    pub(crate) fn advance_native_placement_epoch(&self) {
        self.native_placement_epoch.set(
            self.native_placement_epoch
                .get()
                .checked_add(1)
                .expect("Windows native placement epoch overflowed"),
        );
    }

    fn native_placement_epoch(&self) -> u64 {
        self.native_placement_epoch.get()
    }

    pub(crate) fn set_display_binding(&self, display: WindowsDisplay, topology_generation: u64) {
        self.display.set(display);
        self.display_topology_generation.set(topology_generation);
    }

    pub(crate) fn accepts_pointer_input(&self) -> bool {
        self.accepts_pointer_input.get()
    }

    pub(crate) fn activation_policy(&self) -> WindowActivationPolicy {
        self.activation_policy.get()
    }

    pub(crate) fn is_maximized(&self) -> bool {
        !self.is_fullscreen() && unsafe { IsZoomed(self.hwnd) }.as_bool()
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: self.origin.get(),
            size: self.logical_size.get(),
        }
    }

    // Calculate the bounds used for saving and whether the window is maximized.
    fn calculate_window_bounds(&self) -> (Bounds<Pixels>, bool) {
        let placement = unsafe {
            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(self.hwnd, &mut placement)
                .context("failed to get window placement")
                .log_err();
            placement
        };
        (
            calculate_client_rect(
                placement.rcNormalPosition,
                &self.border_offset,
                self.scale_factor.get(),
            ),
            placement.showCmd == SW_SHOWMAXIMIZED.0 as u32,
        )
    }

    fn window_bounds(&self) -> WindowBounds {
        let (bounds, maximized) = self.calculate_window_bounds();

        if self.is_fullscreen() {
            WindowBounds::Fullscreen(self.fullscreen_restore_bounds.get())
        } else if maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        }
    }

    /// get the logical size of the app's drawable area.
    ///
    /// Currently, GPUI uses the logical size of the app to handle mouse interactions (such as
    /// whether the mouse collides with other elements of GPUI).
    fn content_size(&self) -> Size<Pixels> {
        self.logical_size.get()
    }
}

impl WindowsWindowInner {
    #[cfg(test)]
    pub(crate) fn native_pointer_physical_frame_for_test(
        &self,
    ) -> Option<PlatformNativePointerPhysicalFrame> {
        self.state.native_pointer_physical_frame.current.get()
    }

    pub(crate) fn native_pointer_physical_frame_scope(
        &self,
        client_position: Option<Point<DevicePixels>>,
        expected_global_position: Option<Point<DevicePixels>>,
    ) -> WindowsNativePointerPhysicalFrameScope<'_> {
        let frame = client_position.and_then(|client_position| {
            self.native_pointer_physical_frame_from_client_position(
                client_position,
                expected_global_position,
            )
            .inspect_err(|error| {
                log::trace!("native pointer physical frame unavailable: {error:#}")
            })
            .ok()
        });
        WindowsNativePointerPhysicalFrameScope::enter(
            &self.state.native_pointer_physical_frame,
            frame,
        )
    }

    pub(crate) fn mask_native_pointer_physical_frame_scope(
        &self,
    ) -> WindowsNativePointerPhysicalFrameScope<'_> {
        WindowsNativePointerPhysicalFrameScope::enter(
            &self.state.native_pointer_physical_frame,
            None,
        )
    }

    pub(crate) fn invalidate_native_pointer_physical_frame_scopes(&self) {
        self.state
            .native_pointer_physical_frame
            .invalidate_active_scopes();
    }

    fn native_pointer_physical_frame_from_client_position(
        &self,
        client_position: Point<DevicePixels>,
        expected_global_position: Option<Point<DevicePixels>>,
    ) -> Result<PlatformNativePointerPhysicalFrame> {
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        let (first_geometry, _) = self.physical_geometry_native_sample(&snapshot)?;
        let mut screen_position = POINT {
            x: client_position.x.0,
            y: client_position.y.0,
        };
        unsafe { ClientToScreen(self.hwnd, &mut screen_position) }
            .ok()
            .context("converting native pointer position to physical desktop coordinates")?;
        let global_position = point(
            DevicePixels(screen_position.x),
            DevicePixels(screen_position.y),
        );
        let first_target_display = snapshot
            .physical_observation_at(global_position)
            .context("resolving the target display for the native pointer frame")?;
        let (second_geometry, second_native_display) =
            self.physical_geometry_native_sample(&snapshot)?;
        snapshot
            .validate_target_with_native_display(
                first_target_display,
                global_position,
                second_native_display,
            )
            .or_else(|| snapshot.validate_target(first_target_display, global_position))
            .context("the native pointer target no longer matches the display publication")?;
        let final_snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        anyhow::ensure!(
            first_geometry == second_geometry,
            "native pointer geometry changed while the callback frame was sampled"
        );
        anyhow::ensure!(
            snapshot.generation() == final_snapshot.generation(),
            "native pointer target display changed while the callback frame was sampled"
        );
        let expected_x = second_geometry
            .client_bounds()
            .origin
            .x
            .0
            .checked_add(client_position.x.0)
            .context("native pointer x-coordinate overflowed physical desktop space")?;
        let expected_y = second_geometry
            .client_bounds()
            .origin
            .y
            .0
            .checked_add(client_position.y.0)
            .context("native pointer y-coordinate overflowed physical desktop space")?;
        anyhow::ensure!(
            screen_position.x == expected_x && screen_position.y == expected_y,
            "native pointer point and client geometry were sampled from different frames"
        );
        if let Some(expected_global_position) = expected_global_position {
            anyhow::ensure!(
                screen_position.x == expected_global_position.x.0
                    && screen_position.y == expected_global_position.y.0,
                "native pointer point changed while the callback frame was sampled"
            );
        }
        PlatformNativePointerPhysicalFrame::new(global_position, second_geometry)
            .with_target_display(first_target_display)
            .context("native pointer point is outside its sampled target display")
    }

    fn new(context: &mut WindowCreateContext, hwnd: HWND, cs: &CREATESTRUCTW) -> Result<Rc<Self>> {
        let state = WindowsWindowState::new(
            hwnd,
            &context.directx_devices,
            cs,
            context.current_cursor,
            context.cursor_visible.clone(),
            context.display,
            context.display_topology_generation,
            context.min_size,
            context.appearance,
            context.disable_direct_composition,
            context.invalidate_devices.clone(),
            context.accepts_pointer_input,
            context.activation_policy,
            context.taskbar_visible,
        )?;

        Ok(Rc::new(Self {
            hwnd,
            native_window_lifecycle: Cell::new(NativeWindowLifecycle::Live),
            drag_drop_registered: Cell::new(false),
            show_on_initial_presentation: Cell::new(context.show_on_initial_presentation),
            provisional_session: context.provisional_session.clone(),
            provisional_reveal_generation: Cell::new(None),
            presentation_shutdown_ticket: RefCell::new(None),
            creation_facts: WindowCreationFacts {
                show: context.creation_show,
                focus_on_appearing: context.focus_on_appearing,
                transient_for: context.transient_for,
            },
            drop_target_helper: context.drop_target_helper.clone(),
            state,
            handle: context.handle,
            hide_title_bar: context.hide_title_bar,
            is_movable: context.is_movable,
            executor: context.executor.clone(),
            validation_number: context.validation_number,
            registration: RegisteredWindow::new(
                hwnd,
                context.native_window_generation,
                context.handle.window_id(),
            ),
            recovered_directx_devices: context.recovered_directx_devices.clone(),
            main_receiver: context.main_receiver.clone(),
            platform_window_handle: context.platform_window_handle,
            raw_window_handles: context.raw_window_handles.clone(),
            native_retirement_coordinator: context.native_retirement_coordinator.clone(),
            system_settings: WindowsSystemSettings::new(),
            owner_hwnd: context.owner_hwnd,
            modal_parent_disabled: Cell::new(context.modal_parent_disabled),
            #[cfg(test)]
            lifecycle_test_probe: context.lifecycle_test_probe.clone(),
        }))
    }

    fn retire_native_callbacks(&self) {
        self.state.input_handler.terminate();
        self.state.callbacks.input.terminate();
        self.state.callbacks.should_close.terminate();
    }

    pub(crate) fn accepts_generation_bound_message(&self, generation: usize) -> bool {
        self.native_window_lifecycle.get() == NativeWindowLifecycle::Live
            && self.registration.generation() == generation
    }

    pub(crate) fn native_owner_window_id(&self) -> Option<WindowId> {
        self.creation_facts
            .transient_for
            .map(|owner| owner.window_id())
    }

    fn revoke_drag_drop(&self) {
        if !self.drag_drop_registered.replace(false) {
            return;
        }
        unsafe {
            RevokeDragDrop(self.hwnd)
                .context("revoking native window drag-drop registration")
                .log_err();
        }
    }

    fn unregister_from_platform(&self) {
        let Some(raw_window_handles) = self.raw_window_handles.upgrade() else {
            return;
        };
        let mut raw_window_handles = raw_window_handles.write();
        if let Some(index) = raw_window_handles
            .iter()
            .position(|registered| registered.matches(self.registration))
        {
            raw_window_handles.remove(index);
        }
    }

    pub(crate) fn mark_native_window_destroying(&self) {
        if self.native_window_lifecycle.get() != NativeWindowLifecycle::Live {
            return;
        }
        self.native_window_lifecycle
            .set(NativeWindowLifecycle::Destroying);
        self.settle_pointer_capture_before_native_teardown();
        self.retire_native_callbacks();
        self.unregister_from_platform();
        self.revoke_drag_drop();
    }

    pub(crate) fn mark_native_window_destroyed(&self) {
        if self.native_window_lifecycle.get() == NativeWindowLifecycle::Destroyed {
            return;
        }
        let shutdown = self.presentation_shutdown_ticket();
        if shutdown.acknowledge_native_terminal() {
            #[cfg(any(test, feature = "test-support"))]
            {
                let snapshot = shutdown.snapshot();
                crate::native_test_observation::record_native_terminal(self, snapshot.generation());
            }
            #[cfg(test)]
            {
                let snapshot = shutdown.snapshot();
                self.lifecycle_test_probe.record_event(
                    NativeWindowLifecycleTestEvent::NativeTerminal {
                        window_id: snapshot.window_id(),
                        generation: snapshot.generation(),
                    },
                );
            }
        } else {
            let snapshot = shutdown.snapshot();
            log::error!(
                "native window reached WM_NCDESTROY before presentation quiescence was acknowledged for window {:?}, generation {}",
                snapshot.window_id(),
                snapshot.generation(),
            );
        }
        self.settle_pointer_capture_before_native_teardown();
        self.native_window_lifecycle
            .set(NativeWindowLifecycle::Destroyed);
        self.retire_native_callbacks();
        self.drag_drop_registered.set(false);
        self.release_modal_parent();
        if let Some(coordinator) = self.native_retirement_coordinator.upgrade() {
            coordinator.notify_native_window_terminal(self.registration);
        }
    }

    pub(crate) fn release_modal_parent(&self) {
        if !self.modal_parent_disabled.replace(false) {
            return;
        }
        let Some(parent_hwnd) = self.owner_hwnd else {
            return;
        };
        unsafe {
            let _ = EnableWindow(parent_hwnd, true);
            let _ = SetForegroundWindow(parent_hwnd);
        }
    }

    fn bind_presentation_shutdown_ticket(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> bool {
        if shutdown.snapshot().window_id() != self.handle.window_id() {
            return false;
        }
        let mut current = self.presentation_shutdown_ticket.borrow_mut();
        if let Some(current) = current.as_ref() {
            return current.same_authority(shutdown);
        }
        current.replace(shutdown.clone());
        true
    }

    fn claim_presentation_shutdown_ticket(
        &self,
        candidate: WindowPresentationShutdownTicket,
    ) -> Option<WindowPresentationShutdownTicket> {
        if candidate.snapshot().window_id() != self.handle.window_id() {
            return None;
        }
        let mut current = self.presentation_shutdown_ticket.borrow_mut();
        if let Some(current) = current.as_ref() {
            return Some(current.clone());
        }
        current.replace(candidate.clone());
        Some(candidate)
    }

    pub(crate) fn presentation_shutdown_claimed(&self) -> bool {
        self.presentation_shutdown_ticket.borrow().is_some()
    }
    pub(crate) fn presentation_shutdown_ticket(&self) -> WindowPresentationShutdownTicket {
        if let Some(shutdown) = self.presentation_shutdown_ticket.borrow().as_ref() {
            return shutdown.clone();
        }
        let generation =
            NEXT_EMERGENCY_PRESENTATION_SHUTDOWN_GENERATION.fetch_add(1, Ordering::Relaxed);
        assert_ne!(
            generation, 0,
            "emergency presentation-shutdown generation space exhausted"
        );
        let shutdown = WindowPresentationShutdownTicket::new(self.handle.window_id(), generation);
        self.claim_presentation_shutdown_ticket(shutdown)
            .expect("a new emergency shutdown ticket must match its native window")
    }

    pub(crate) fn quiesce_presentation(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformPresentationShutdownOutcome {
        if !self.bind_presentation_shutdown_ticket(shutdown) {
            return PlatformPresentationShutdownOutcome::Rejected;
        }
        #[cfg(any(test, feature = "test-support"))]
        let was_quiesced = shutdown.snapshot().quiesced();
        let Ok(mut renderer) = self.state.renderer.try_borrow_mut() else {
            return PlatformPresentationShutdownOutcome::Rejected;
        };
        if let Err(error) = renderer.quiesce_surface(shutdown) {
            log::error!("failed to quiesce native window presentation: {error:#}");
            return PlatformPresentationShutdownOutcome::Rejected;
        }
        drop(renderer);
        #[cfg(any(test, feature = "test-support"))]
        if !was_quiesced {
            let snapshot = shutdown.snapshot();
            crate::native_test_observation::record_presentation_quiesced(
                self,
                snapshot.generation(),
            );
        }
        #[cfg(test)]
        if !was_quiesced {
            let snapshot = shutdown.snapshot();
            self.lifecycle_test_probe.record_event(
                NativeWindowLifecycleTestEvent::PresentationQuiesced {
                    window_id: snapshot.window_id(),
                    generation: snapshot.generation(),
                },
            );
        }
        PlatformPresentationShutdownOutcome::Quiesced
    }

    pub(crate) fn destroy_native_window(&self) -> bool {
        if self.is_native_window_terminal() {
            return false;
        }
        let shutdown = self.presentation_shutdown_ticket();
        if self.quiesce_presentation(&shutdown) != PlatformPresentationShutdownOutcome::Quiesced {
            return false;
        }
        self.destroy_native_window_with_ticket(&shutdown)
    }

    fn destroy_native_window_with_ticket(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> bool {
        if !self.bind_presentation_shutdown_ticket(shutdown) || !shutdown.snapshot().quiesced() {
            return false;
        }
        let Ok(renderer) = self.state.renderer.try_borrow() else {
            return false;
        };
        if !renderer.is_quiesced_for(shutdown) {
            return false;
        }
        drop(renderer);
        if self.native_window_lifecycle.get() != NativeWindowLifecycle::Live {
            if unsafe { !IsWindow(Some(self.hwnd)).as_bool() } {
                self.mark_native_window_destroyed();
            }
            return false;
        }
        #[cfg(any(test, feature = "test-support"))]
        {
            let snapshot = shutdown.snapshot();
            crate::native_test_observation::record_destroy_entered(self, snapshot.generation());
        }
        #[cfg(test)]
        {
            let snapshot = shutdown.snapshot();
            self.lifecycle_test_probe.record_event(
                NativeWindowLifecycleTestEvent::DestroyEntered {
                    window_id: snapshot.window_id(),
                    generation: snapshot.generation(),
                },
            );
        }
        #[cfg(test)]
        if self.lifecycle_test_probe.take_fail_next_destroy() {
            return false;
        }

        if unsafe { IsWindow(Some(self.hwnd)).as_bool() } {
            self.settle_pointer_capture_before_native_teardown();
            if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                log::error!("failed to destroy native window: {error}");
                return false;
            }
        }
        if unsafe { !IsWindow(Some(self.hwnd)).as_bool() } {
            self.mark_native_window_destroyed();
        }
        true
    }

    pub(crate) fn is_native_window_terminal(&self) -> bool {
        if self.native_window_lifecycle.get() == NativeWindowLifecycle::Destroyed {
            return true;
        }
        if unsafe { !IsWindow(Some(self.hwnd)).as_bool() } {
            self.mark_native_window_destroyed();
            return true;
        }
        false
    }

    fn prepare_pending_initial_placement(&self) -> Result<()> {
        let Some(mut open_status) = self.take_validated_pending_initial_placement()? else {
            return Ok(());
        };
        let previous_open_status = open_status.clone();
        let previous_facts = self.state.last_validated_platform_facts.borrow().clone();
        let rect = open_status.placement.rcNormalPosition;
        let result = (|| {
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
                )
            }
            .context("failed to prepare hidden window placement")?;
            let settled_geometry =
                self.settle_physical_client_bounds_exactly(open_status.client_placement)?;
            self.checkpoint_settled_initial_placement(&mut open_status, settled_geometry)?;
            self.validate_initial_placement_authority(&open_status)
        })();
        if result.is_ok() {
            self.state.initial_placement.set(Some(open_status));
            self.refresh_pending_initial_platform_facts();
        } else {
            self.state.initial_placement.set(Some(previous_open_status));
            self.state
                .last_validated_platform_facts
                .replace(previous_facts);
        }
        result
    }

    fn present_pending_initial_placement(
        self: &Rc<Self>,
        activate: bool,
        force_show: bool,
        insert_after: Option<HWND>,
    ) -> Result<bool> {
        if !force_show && !self.show_on_initial_presentation.get() {
            return Ok(false);
        }
        let Some(open_status) = self.take_validated_pending_initial_placement()? else {
            return Ok(false);
        };
        #[cfg(test)]
        if self
            .lifecycle_test_probe
            .take_fail_next_initial_presentation()
        {
            self.state.initial_placement.set(Some(open_status));
            anyhow::bail!("injected initial-presentation failure");
        }
        let activate = activate && self.state.activation_policy.get().accepts_activation;
        anyhow::ensure!(
            insert_after.is_none() || matches!(open_status.state, WindowOpenState::Windowed),
            "relative provisional reveal requires a windowed initial placement"
        );
        let result = self.with_owner_detached_for_nonactivating_show(activate, || {
            let rect = open_status.placement.rcNormalPosition;
            let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER;
            if insert_after.is_some() {
                flags |= SWP_SHOWWINDOW;
            } else {
                flags |= SWP_NOZORDER;
            }
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    insert_after,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    flags,
                )
            }
            .context("failed to apply initial restore geometry")?;
            self.state
                .border_offset
                .update_restored(self.hwnd)
                .log_err();

            match open_status.state {
                WindowOpenState::Maximized if activate => unsafe {
                    let mut placement = open_status.placement;
                    placement.showCmd = SW_SHOWMAXIMIZED.0 as u32;
                    SetWindowPlacement(self.hwnd, &placement)
                        .context("failed to apply initial maximized placement")?;
                },
                WindowOpenState::Maximized => {
                    self.apply_nonactivating_initial_maximized_placement()?;
                }
                WindowOpenState::Fullscreen => {
                    if !self.state.is_fullscreen() {
                        self.toggle_fullscreen_now()?;
                    }
                }
                WindowOpenState::Windowed => {}
            }

            if insert_after.is_none() {
                let command = match (open_status.state, activate) {
                    (WindowOpenState::Maximized, true) => SW_MAXIMIZE,
                    (WindowOpenState::Maximized, false) => SW_SHOWNA,
                    (WindowOpenState::Fullscreen | WindowOpenState::Windowed, true) => {
                        SW_SHOWNORMAL
                    }
                    (WindowOpenState::Fullscreen | WindowOpenState::Windowed, false) => {
                        SW_SHOWNOACTIVATE
                    }
                };
                unsafe {
                    let _ = ShowWindow(self.hwnd, command);
                }
            }
            if activate && !self.state.activation_policy.get().focus_on_click {
                unsafe {
                    SetActiveWindow(self.hwnd).ok();
                    SetFocus(Some(self.hwnd)).ok();
                    let _ = SetForegroundWindow(self.hwnd);
                }
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                if let Err(error) = self.validate_initial_placement_authority(&open_status) {
                    unsafe {
                        let _ = ShowWindow(self.hwnd, SW_HIDE);
                    }
                    self.state.initial_placement.set(Some(open_status));
                    return Err(error).context(
                        "display topology changed while the initial placement was presented",
                    );
                }
                self.show_on_initial_presentation.set(false);
                Ok(true)
            }
            Err(error) => {
                self.state.initial_placement.set(Some(open_status));
                Err(error)
            }
        }
    }

    fn reveal_pending_initial_placement_without_geometry(
        self: &Rc<Self>,
        insert_after: HWND,
    ) -> Result<bool> {
        let Some(open_status) = self.take_validated_pending_initial_placement()? else {
            return Ok(false);
        };
        anyhow::ensure!(
            matches!(open_status.state, WindowOpenState::Windowed),
            "provisional reveal requires a retained windowed initial placement"
        );
        let result = self.with_owner_detached_for_nonactivating_show(false, || {
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    Some(insert_after),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                )
            }
            .context("failed to reveal the prepared provisional window without moving it")?;
            Ok(())
        });
        match result {
            Ok(()) => {
                if let Err(error) = self.validate_initial_placement_authority(&open_status) {
                    unsafe {
                        let _ = ShowWindow(self.hwnd, SW_HIDE);
                    }
                    self.state.initial_placement.set(Some(open_status));
                    return Err(error).context(
                        "display topology changed while the provisional placement was revealed",
                    );
                }
                self.show_on_initial_presentation.set(false);
                Ok(true)
            }
            Err(error) => {
                self.state.initial_placement.set(Some(open_status));
                Err(error)
            }
        }
    }

    fn present_pending_initial_placement_with_deferred_mutation(
        self: &Rc<Self>,
        activate: bool,
        force_show: bool,
    ) -> Result<bool> {
        if !force_show && !self.show_on_initial_presentation.get() {
            return Ok(false);
        }
        let deferred = self.state.deferred_placement_mutation.take();
        let (native_rollback, rollback_capture_error) = match deferred {
            Some(_) => match self.capture_window_placement_snapshot() {
                Ok(snapshot) => (Some(snapshot), None),
                Err(error) => (None, Some(error)),
            },
            None => (None, None),
        };
        let initial_placement_before_deferred = deferred
            .is_some()
            .then(|| self.pending_initial_placement())
            .flatten();
        let before_facts = deferred.map(|_| {
            self.project_pending_initial_placement(
                self.observed_platform_facts_from_native()
                    .unwrap_or_else(|_| self.last_validated_platform_facts()),
            )
        });
        let mut deferred_merged = false;
        let mut result = match rollback_capture_error {
            Some(error) => {
                Err(error).context("failed to capture deferred placement rollback state")
            }
            None => (|| {
                if let Some(deferred) = deferred {
                    self.merge_deferred_initial_placement(deferred.request)?;
                    deferred_merged = true;
                }
                self.present_pending_initial_placement(activate, force_show, None)
            })(),
        };

        if result.is_err() || matches!(&result, Ok(false)) {
            if let Some(native_rollback) = native_rollback.filter(|_| deferred_merged) {
                if let Err(rollback_error) = self.restore_window_placement_snapshot(native_rollback)
                {
                    let original_outcome = match &result {
                        Ok(false) => "initial presentation was deferred".to_string(),
                        Err(error) => format!("initial presentation failed: {error:#}"),
                        Ok(true) => unreachable!("successful presentation does not roll back"),
                    };
                    result = Err(rollback_error).with_context(|| {
                        format!("failed to restore deferred placement after {original_outcome}")
                    });
                }
            }
            if let Some(initial_placement) = initial_placement_before_deferred.clone() {
                self.state.initial_placement.set(Some(initial_placement));
            }
            if let Some(before_facts) = before_facts.clone() {
                self.state
                    .last_validated_platform_facts
                    .replace(Some(before_facts));
            }
        }

        let Some(deferred) = deferred else {
            return result;
        };
        if matches!(&result, Ok(false)) {
            let replacement = self.state.deferred_placement_mutation.take();
            if self.placement_mutation_is_current(deferred.generation) && replacement.is_none() {
                self.state.deferred_placement_mutation.set(Some(deferred));
            } else {
                self.state.deferred_placement_mutation.set(replacement);
            }
            return result;
        }

        if self.placement_mutation_is_current(deferred.generation)
            && (unsafe { IsWindow(Some(self.hwnd)).as_bool() })
        {
            let before_facts = before_facts.expect("deferred placement captured initial facts");
            let (terminal, facts) = if result.is_ok() {
                match self.observed_platform_facts_from_native() {
                    Ok(facts) => (PlatformWindowMutationTerminal::Observed, facts),
                    Err(error) => {
                        log::warn!(
                            "Windows deferred placement completed but terminal fact readback failed: {error:#}"
                        );
                        (PlatformWindowMutationTerminal::Rejected, before_facts)
                    }
                }
            } else {
                (PlatformWindowMutationTerminal::Rejected, before_facts)
            };
            self.emit_window_mutation_observation(
                WindowMutationDomain::Placement,
                deferred.generation,
                terminal,
                facts,
            );
        }
        result
    }

    fn apply_nonactivating_initial_maximized_placement(&self) -> Result<()> {
        let style = WINDOW_STYLE(
            self.get_window_long_checked(
                GWL_STYLE,
                "failed to read initial maximized window style",
            )? as _,
        ) | WS_MAXIMIZE;
        let maximized_outer_bounds = calculate_window_rect(
            self.state
                .display
                .get()
                .visible_bounds()
                .to_device_pixels(self.state.scale_factor.get()),
            &self.state.border_offset,
        );

        self.apply_window_style_and_bounds(StyleAndBounds {
            style,
            x: maximized_outer_bounds.left,
            y: maximized_outer_bounds.top,
            cx: maximized_outer_bounds.right - maximized_outer_bounds.left,
            cy: maximized_outer_bounds.bottom - maximized_outer_bounds.top,
        })
        .context("failed to apply non-activating initial maximized placement")
    }

    fn with_owner_detached_for_nonactivating_show<T>(
        &self,
        activate: bool,
        show: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        if activate {
            return show();
        }
        let Some(owner_hwnd) = self.owner_hwnd else {
            return show();
        };
        let observed_owner = unsafe { GetWindow(self.hwnd, GW_OWNER) }
            .context("failed to read transient owner before non-activating show")?;
        anyhow::ensure!(
            observed_owner == owner_hwnd,
            "transient owner changed before non-activating show"
        );
        self.set_window_long_checked(
            GWLP_HWNDPARENT,
            0,
            "failed to detach transient owner for non-activating show",
        )?;

        let show_result = show();
        let restore_result = self.set_window_long_checked(
            GWLP_HWNDPARENT,
            owner_hwnd.0 as isize,
            "failed to restore transient owner after non-activating show",
        );
        if let Err(error) = restore_result {
            return match show_result {
                Ok(_) => Err(error),
                Err(show_error) => Err(show_error).context(format!(
                    "restoring the transient owner also failed: {error:#}"
                )),
            };
        }
        let restored_owner = unsafe { GetWindow(self.hwnd, GW_OWNER) }
            .context("failed to read back restored transient owner")?;
        anyhow::ensure!(
            restored_owner == owner_hwnd,
            "restored transient owner did not match the committed creation fact"
        );
        show_result
    }

    fn complete_initial_presentation(
        self: &Rc<Self>,
        activate: bool,
    ) -> PlatformWindowCommandOutcome {
        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::Rejected;
        }
        #[cfg(test)]
        self.lifecycle_test_probe
            .run_initial_presentation_hook(self.hwnd);
        let result = self.present_pending_initial_placement_with_deferred_mutation(activate, false);
        match result {
            Ok(_) => PlatformWindowCommandOutcome::Accepted,
            Err(error) => {
                log::error!("failed to complete initial window presentation: {error:#}");
                PlatformWindowCommandOutcome::Rejected
            }
        }
    }

    pub(crate) fn provisional_accepts_interaction(&self) -> bool {
        self.provisional_session.as_ref().is_none_or(|session| {
            let snapshot = session.snapshot();
            snapshot.window_id() == Some(self.handle.window_id()) && snapshot.accepts_interaction()
        })
    }

    pub(crate) fn provisional_requires_hit_transparency(&self) -> bool {
        self.provisional_session.is_some() && !self.provisional_accepts_interaction()
    }

    pub(crate) fn provisional_session_snapshot(&self) -> Option<WindowProvisionalSessionSnapshot> {
        self.provisional_session
            .as_ref()
            .map(WindowProvisionalSession::snapshot)
    }

    fn registered_window_snapshot(&self, hwnd: HWND) -> Result<Option<RegisteredWindow>> {
        let registered = self
            .raw_window_handles
            .upgrade()
            .context("native window registry is no longer available")?;
        Ok(registered
            .read()
            .iter()
            .copied()
            .find(|candidate| candidate.as_raw() == hwnd))
    }

    fn registered_provisional_peers(
        &self,
        peer_windows: &[WindowId],
    ) -> Result<Arc<[RegisteredWindow]>> {
        let registry = self
            .raw_window_handles
            .upgrade()
            .context("native window registry is no longer available")?;
        let registered = registry.read();
        let mut peers = Vec::new();
        for window_id in peer_windows {
            anyhow::ensure!(
                *window_id != self.handle.window_id(),
                "provisional z-order peers cannot include the provisional window"
            );
            let mut matches = registered
                .iter()
                .copied()
                .filter(|candidate| candidate.window_id() == *window_id);
            let peer = matches
                .next()
                .context("a provisional z-order peer is no longer registered")?;
            anyhow::ensure!(
                matches.next().is_none(),
                "a provisional z-order peer has ambiguous native registration"
            );
            if !peers
                .iter()
                .any(|current: &RegisteredWindow| current.matches(peer))
            {
                peers.push(peer);
            }
        }
        Ok(peers.into())
    }

    fn observe_z_order_identity(&self, hwnd: HWND) -> Result<NativeZOrderWindowIdentity> {
        anyhow::ensure!(
            unsafe { IsWindow(Some(hwnd)).as_bool() },
            "z-order window became terminal"
        );
        let mut process_id = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        anyhow::ensure!(
            thread_id != 0 && process_id != 0,
            "failed to observe z-order window process identity"
        );
        Ok(NativeZOrderWindowIdentity {
            hwnd,
            thread_id,
            process_id,
            registered: self.registered_window_snapshot(hwnd)?,
        })
    }

    fn native_rect(hwnd: HWND) -> Result<Option<NativeRect>> {
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            anyhow::ensure!(
                !unsafe { IsWindow(Some(hwnd)).as_bool() },
                "failed to observe a live z-order window rectangle"
            );
            return Ok(None);
        }
        Ok(NativeRect::try_from_rect(rect))
    }

    fn next_z_order_window(candidate: HWND) -> Option<HWND> {
        unsafe { GetWindow(candidate, GW_HWNDNEXT) }.ok()
    }

    fn capture_provisional_placement_rollback(&self) -> Result<ProvisionalPlacementRollback> {
        let observation_epoch = self.state.native_placement_epoch();
        let physical_geometry = self.physical_geometry_from_native()?;
        let rect = Self::native_rect(self.hwnd)?
            .context("provisional native window has no rollback rectangle")?;
        let previous_above = unsafe { GetWindow(self.hwnd, GW_HWNDPREV) }
            .ok()
            .filter(|hwnd| !crate::platform::native_window_is_shell_desktop(*hwnd))
            .map(|hwnd| self.observe_z_order_identity(hwnd))
            .transpose()?;
        let style = WINDOW_EX_STYLE(self.get_window_long_checked(
            GWL_EXSTYLE,
            "failed to read provisional native extended style for rollback",
        )? as u32);
        anyhow::ensure!(
            self.state.native_placement_epoch() == observation_epoch,
            "native placement changed while provisional rollback authority was sampled"
        );
        Ok(ProvisionalPlacementRollback {
            rect,
            previous_above,
            was_topmost: style.contains(WS_EX_TOPMOST),
            physical_geometry,
        })
    }

    fn restore_provisional_placement_rollback(
        &self,
        rollback: ProvisionalPlacementRollback,
    ) -> Result<()> {
        let z_order = self.restore_provisional_z_order(rollback);
        let rect = self.restore_provisional_rect(rollback.rect);
        let physical_geometry =
            self.restore_provisional_physical_geometry(rollback.physical_geometry);
        let failures = [
            ("z-order", z_order),
            ("outer rectangle", rect),
            ("physical client geometry", physical_geometry),
        ]
        .into_iter()
        .filter_map(|(component, result)| {
            result.err().map(|error| format!("{component}: {error:#}"))
        })
        .collect::<Vec<_>>();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "failed to restore provisional native placement: {}",
                failures.join("; ")
            ))
        }
    }

    fn restore_provisional_physical_geometry(
        &self,
        expected: PlatformWindowPhysicalGeometry,
    ) -> Result<()> {
        let display_observation = expected
            .display_observation()
            .context("provisional rollback geometry did not identify its display")?;
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        anyhow::ensure!(
            snapshot.generation() == display_observation.topology_generation(),
            "display topology changed before provisional geometry rollback"
        );
        let request = WindowPhysicalPlacementRequest::try_new(expected.client_bounds())
            .context("provisional rollback client geometry is not representable")?;
        self.settle_physical_client_bounds_exactly(request)?;
        let restored = self
            .physical_geometry_from_native()
            .context("failed to read provisional geometry after rollback")?;
        anyhow::ensure!(
            restored == expected,
            "provisional physical client geometry did not restore exactly"
        );
        let display = snapshot
            .display(display_observation.display_id())
            .context("the restored provisional display is no longer available")?;
        self.state
            .set_display_binding(display, snapshot.generation());
        self.state.scale_factor.set(restored.scale_factor());
        Ok(())
    }

    fn restore_provisional_rect(&self, rect: NativeRect) -> Result<()> {
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .context("failed to restore provisional native geometry after rejection")?;
        anyhow::ensure!(
            Self::native_rect(self.hwnd)? == Some(rect),
            "provisional native geometry did not restore exactly"
        );
        Ok(())
    }

    fn restore_provisional_z_order(&self, rollback: ProvisionalPlacementRollback) -> Result<()> {
        let insert_after = if let Some(previous) = rollback.previous_above {
            anyhow::ensure!(
                unsafe { IsWindow(Some(previous.hwnd)).as_bool() },
                "the provisional rollback predecessor became terminal"
            );
            anyhow::ensure!(
                self.observe_z_order_identity(previous.hwnd)? == previous,
                "the provisional rollback predecessor identity changed"
            );
            previous.hwnd
        } else if rollback.was_topmost {
            HWND_TOPMOST
        } else {
            HWND_TOP
        };
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(insert_after),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_NOSIZE,
            )
        }
        .context("failed to restore provisional native z-order after rejection")?;
        let restored = self.capture_provisional_placement_rollback()?;
        anyhow::ensure!(
            restored.previous_above == rollback.previous_above
                && restored.was_topmost == rollback.was_topmost,
            "provisional native z-order did not restore exactly"
        );
        Ok(())
    }

    fn rollback_applied_provisional_final_placement(
        &self,
        applied: AppliedProvisionalFinalPlacement,
        authority: ProvisionalPlacementCompensationAuthority,
    ) -> Result<ProvisionalPlacementCompensation> {
        if self.is_native_window_terminal()
            || !unsafe { IsWindow(Some(self.hwnd)).as_bool() }
            || self
                .registered_window_snapshot(self.hwnd)?
                .is_none_or(|current| !current.matches(self.registration))
        {
            return Ok(ProvisionalPlacementCompensation::authority_changed());
        }
        if self.state.native_placement_epoch() != applied.applied_epoch {
            return Ok(ProvisionalPlacementCompensation::authority_changed());
        }

        let current_z_order = self.capture_provisional_placement_rollback()?;
        let z_order = if current_z_order.previous_above != applied.applied.previous_above
            || current_z_order.was_topmost != applied.applied.was_topmost
        {
            ProvisionalPlacementCompensationComponent::AuthorityChanged
        } else if current_z_order.previous_above == applied.rollback.previous_above
            && current_z_order.was_topmost == applied.rollback.was_topmost
        {
            ProvisionalPlacementCompensationComponent::Restored
        } else if (applied
            .rollback
            .previous_above
            .is_some_and(|previous| previous.registered.is_none())
            || applied
                .applied
                .previous_above
                .is_some_and(|previous| previous.registered.is_none()))
            && authority == ProvisionalPlacementCompensationAuthority::RegisteredOnly
        {
            ProvisionalPlacementCompensationComponent::Unproven
        } else {
            self.restore_provisional_z_order(applied.rollback)?;
            ProvisionalPlacementCompensationComponent::Restored
        };

        let current_rect = Self::native_rect(self.hwnd)?;
        let rect = if current_rect != Some(applied.applied.rect) {
            ProvisionalPlacementCompensationComponent::AuthorityChanged
        } else if applied.applied.rect == applied.rollback.rect {
            ProvisionalPlacementCompensationComponent::Restored
        } else {
            self.restore_provisional_rect(applied.rollback.rect)?;
            ProvisionalPlacementCompensationComponent::Restored
        };
        let physical_geometry =
            if current_z_order.physical_geometry != applied.applied.physical_geometry {
                ProvisionalPlacementCompensationComponent::AuthorityChanged
            } else if applied.applied.physical_geometry == applied.rollback.physical_geometry {
                ProvisionalPlacementCompensationComponent::Restored
            } else {
                self.restore_provisional_physical_geometry(applied.rollback.physical_geometry)?;
                ProvisionalPlacementCompensationComponent::Restored
            };
        Ok(ProvisionalPlacementCompensation {
            rect,
            z_order,
            physical_geometry,
        })
    }

    fn compensate_applied_provisional_final_placement(
        &self,
        applied: AppliedProvisionalFinalPlacement,
        reason: &str,
    ) {
        match self.rollback_applied_provisional_final_placement(
            applied,
            ProvisionalPlacementCompensationAuthority::RegisteredOnly,
        ) {
            Ok(outcome) if outcome.fully_restored() => {}
            Ok(outcome) => log::warn!("Windows only partially compensated {reason}: {outcome:?}"),
            Err(error) => log::error!("Windows failed to compensate {reason}: {error:#}"),
        }
    }

    fn prepare_provisional_z_order_band(
        &self,
        point: Point<DevicePixels>,
        requested_rect: NativeRect,
        peer_windows: &[WindowId],
    ) -> Result<PreparedProvisionalZOrderBand> {
        anyhow::ensure!(
            requested_rect.contains(point),
            "provisional z-order point is outside the requested window placement"
        );
        let current = self
            .registered_window_snapshot(self.hwnd)?
            .filter(|current| current.matches(self.registration))
            .context("provisional native-window registration is stale")?;
        let peers = self.registered_provisional_peers(peer_windows)?;

        let mut barrier = None;
        let mut candidate = unsafe { GetTopWindow(None) }
            .context("failed to begin provisional z-order preparation")?;
        let mut visited = HashSet::new();
        let mut completed = false;
        for _ in 0..MAX_PROVISIONAL_Z_ORDER_WINDOWS {
            if candidate.is_invalid() || !visited.insert(candidate.0 as usize) {
                anyhow::bail!("provisional z-order preparation encountered an invalid walk");
            }
            if candidate != self.hwnd
                && !crate::platform::native_window_is_shell_desktop(candidate)
                && unsafe { IsWindowVisible(candidate).as_bool() }
                && !unsafe { IsIconic(candidate).as_bool() }
                && let Some(rect) = Self::native_rect(candidate)?
                && rect.contains(point)
            {
                match crate::platform::native_window_cloak(candidate) {
                    crate::platform::NativeWindowCloak::Uncloaked => {
                        let identity = self.observe_z_order_identity(candidate)?;
                        let is_peer = identity.registered.is_some_and(|registered| {
                            peers.iter().any(|peer| peer.matches(registered))
                        });
                        if !is_peer {
                            barrier = Some(identity);
                            completed = true;
                            break;
                        }
                    }
                    crate::platform::NativeWindowCloak::Cloaked => {}
                    crate::platform::NativeWindowCloak::Unknown => {
                        anyhow::bail!("provisional z-order barrier cloak is unavailable")
                    }
                }
            }
            let Some(next) = Self::next_z_order_window(candidate) else {
                completed = true;
                break;
            };
            candidate = next;
        }
        anyhow::ensure!(
            completed,
            "provisional z-order preparation exceeded the native-window walk limit"
        );

        Ok(PreparedProvisionalZOrderBand {
            point,
            current,
            peers,
            barrier,
        })
    }

    fn verify_provisional_z_order_band(
        &self,
        prepared: &PreparedProvisionalZOrderBand,
    ) -> WindowProvisionalRevealZOrder {
        let verified = (|| -> Result<WindowProvisionalRevealZOrder> {
            let current = self
                .registered_window_snapshot(self.hwnd)?
                .filter(|current| current.matches(prepared.current))
                .context("provisional native-window registration changed during reveal")?;
            anyhow::ensure!(current.matches(self.registration));
            anyhow::ensure!(unsafe { IsWindowVisible(self.hwnd).as_bool() });
            anyhow::ensure!(!unsafe { IsIconic(self.hwnd).as_bool() });
            anyhow::ensure!(
                crate::platform::native_window_cloak(self.hwnd)
                    == crate::platform::NativeWindowCloak::Uncloaked
            );
            let current_rect = Self::native_rect(self.hwnd)?
                .context("provisional native window has no visible rectangle")?;
            anyhow::ensure!(current_rect.contains(prepared.point));
            if let Some(barrier) = prepared.barrier {
                anyhow::ensure!(
                    self.observe_z_order_identity(barrier.hwnd)? == barrier,
                    "provisional z-order barrier identity changed during reveal"
                );
            }

            let mut visible_fragments = vec![current_rect];
            let mut covering_above = Vec::new();
            let mut reached_current = false;
            let mut candidate = unsafe { GetTopWindow(None) }
                .context("failed to begin provisional z-order verification")?;
            let mut visited = HashSet::new();
            for _ in 0..MAX_PROVISIONAL_Z_ORDER_WINDOWS {
                if candidate.is_invalid() || !visited.insert(candidate.0 as usize) {
                    anyhow::bail!("provisional z-order verification encountered an invalid walk");
                }
                if candidate == self.hwnd {
                    reached_current = true;
                    break;
                }
                if !crate::platform::native_window_is_shell_desktop(candidate)
                    && unsafe { IsWindowVisible(candidate).as_bool() }
                    && !unsafe { IsIconic(candidate).as_bool() }
                    && let Some(rect) = Self::native_rect(candidate)?
                    && rect.intersection(current_rect).is_some()
                {
                    match crate::platform::native_window_cloak(candidate) {
                        crate::platform::NativeWindowCloak::Cloaked => {}
                        crate::platform::NativeWindowCloak::Unknown => {
                            anyhow::bail!("provisional occluder cloak is unavailable")
                        }
                        crate::platform::NativeWindowCloak::Uncloaked => {
                            if rect.contains(prepared.point) {
                                covering_above.push(self.observe_z_order_identity(candidate)?);
                            }
                            let mut next_fragments = Vec::new();
                            for fragment in visible_fragments.drain(..) {
                                fragment.subtract(rect, &mut next_fragments);
                            }
                            visible_fragments = next_fragments;
                        }
                    }
                }
                let Some(next) = Self::next_z_order_window(candidate) else {
                    break;
                };
                candidate = next;
            }
            anyhow::ensure!(
                reached_current,
                "provisional window is absent from native z-order"
            );
            anyhow::ensure!(
                !visible_fragments.is_empty(),
                "provisional window is fully obscured after reveal"
            );
            match prepared.barrier {
                None => {
                    anyhow::ensure!(
                        covering_above
                            .iter()
                            .copied()
                            .all(|identity| prepared.is_peer(identity)),
                        "a new opaque barrier appeared above the provisional reveal point"
                    );
                    Ok(if covering_above.is_empty() {
                        WindowProvisionalRevealZOrder::Exact
                    } else {
                        WindowProvisionalRevealZOrder::Adjusted
                    })
                }
                Some(barrier) => {
                    let mut saw_barrier = false;
                    for identity in covering_above {
                        if identity == barrier {
                            anyhow::ensure!(
                                !saw_barrier,
                                "the exact point barrier appeared more than once"
                            );
                            saw_barrier = true;
                        } else {
                            anyhow::ensure!(
                                !saw_barrier && prepared.is_peer(identity),
                                "the provisional window crossed an unrelated opaque barrier"
                            );
                        }
                    }
                    anyhow::ensure!(
                        saw_barrier,
                        "the exact point barrier is not above the provisional window"
                    );
                    Ok(WindowProvisionalRevealZOrder::Adjusted)
                }
            }
        })();
        verified.unwrap_or(WindowProvisionalRevealZOrder::Unavailable)
    }

    fn provisional_native_hit_is_transparent(&self, point: Point<DevicePixels>) -> bool {
        let Ok(Some(rect)) = Self::native_rect(self.hwnd) else {
            return false;
        };
        if !rect.contains(point) {
            return false;
        }
        let packed = ((point.y.0 as u32 & 0xffff) << 16) | (point.x.0 as u32 & 0xffff);
        unsafe {
            SendMessageW(
                self.hwnd,
                WM_NCHITTEST,
                Some(WPARAM::default()),
                Some(LPARAM(packed as isize)),
            )
        }
        .0 == HTTRANSPARENT as isize
    }

    fn reveal_deferred_initial_presentation(
        self: &Rc<Self>,
        session_generation: u64,
        presentation_generation: u64,
    ) -> PlatformWindowCommandOutcome {
        if self.is_native_window_terminal()
            || presentation_generation == 0
            || self.provisional_reveal_generation.get().is_some()
            || unsafe { IsWindowVisible(self.hwnd).as_bool() }
        {
            return PlatformWindowCommandOutcome::Rejected;
        }
        let Some(session) = self.provisional_session.as_ref() else {
            return PlatformWindowCommandOutcome::Rejected;
        };
        let snapshot = session.snapshot();
        if snapshot.generation() != session_generation
            || snapshot.window_id() != Some(self.handle.window_id())
            || snapshot.phase() != WindowProvisionalSessionPhase::Gated
        {
            return PlatformWindowCommandOutcome::Rejected;
        }
        let Ok(request) =
            session.claim_native_reveal(self.handle.window_id(), presentation_generation)
        else {
            return PlatformWindowCommandOutcome::Rejected;
        };
        let reveal_point = request.reveal_point();
        let initial_physical_geometry = request.initial_physical_geometry();
        let peer_windows = request.peer_windows();

        let foreground_before = unsafe { GetForegroundWindow() };
        let visibility_guard = ProvisionalRevealVisibilityGuard::new(self.hwnd);
        let show_result = (|| {
            if let Some(expected) = initial_physical_geometry {
                anyhow::ensure!(
                    self.physical_geometry_from_native()? == expected,
                    "provisional reveal target geometry changed after its accepted frame"
                );
            }
            let requested_rect = Self::native_rect(self.hwnd)?
                .context("provisional reveal has no live native window frame")?;
            let prepared =
                self.prepare_provisional_z_order_band(reveal_point, requested_rect, &peer_windows)?;
            let shown =
                self.reveal_pending_initial_placement_without_geometry(prepared.insert_after())?;
            let physical_client_bounds_exact = initial_physical_geometry
                .is_none_or(|expected| self.physical_geometry_from_native().ok() == Some(expected));
            anyhow::ensure!(
                physical_client_bounds_exact,
                "provisional reveal changed the accepted physical geometry"
            );
            Ok((
                shown,
                self.verify_provisional_z_order_band(&prepared),
                prepared,
                physical_client_bounds_exact,
            ))
        })();
        if let Err(error) = show_result.as_ref() {
            log::error!("failed to reveal deferred provisional presentation: {error:#}");
        }
        let native_visible = unsafe { IsWindowVisible(self.hwnd).as_bool() };
        let physical_client_bounds_exact = show_result
            .as_ref()
            .map(|(_, _, _, exact)| *exact)
            .unwrap_or(false);
        let z_order = show_result
            .as_ref()
            .map(|(_, z_order, _, _)| *z_order)
            .unwrap_or(WindowProvisionalRevealZOrder::Unavailable);
        let facts = WindowProvisionalRevealNativeFacts::new(
            native_visible,
            unsafe { GetForegroundWindow() } == foreground_before,
            self.provisional_native_hit_is_transparent(reveal_point),
            true,
            physical_client_bounds_exact,
            z_order,
        );
        let recorded = session
            .record_native_reveal(self.handle.window_id(), presentation_generation, facts)
            .is_ok();
        if matches!(show_result, Ok((true, _, _, _)))
            && recorded
            && facts.accepts_reveal()
            && facts.z_order() != WindowProvisionalRevealZOrder::Unavailable
        {
            self.provisional_reveal_generation
                .set(Some(presentation_generation));
            visibility_guard.commit();
            PlatformWindowCommandOutcome::Accepted
        } else {
            PlatformWindowCommandOutcome::Rejected
        }
    }

    /// Must stay synchronous because activation APIs can pump window messages and command
    /// dispatch runs only after the application has released its mutable borrow.
    fn activate_now(self: &Rc<Self>) -> PlatformWindowCommandOutcome {
        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        if !self.provisional_accepts_interaction()
            || !self.state.activation_policy.get().accepts_activation
        {
            return PlatformWindowCommandOutcome::Rejected;
        }

        let hwnd = self.hwnd;
        let had_initial_placement = self.has_pending_initial_placement();
        let placement_result = self
            .present_pending_initial_placement_with_deferred_mutation(true, true)
            .map(|_| ());
        let placement_failed = placement_result.is_err();
        placement_result.log_err();

        if placement_failed {
            return PlatformWindowCommandOutcome::Rejected;
        }

        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        if !had_initial_placement && unsafe { !IsWindowVisible(hwnd).as_bool() } {
            let command = if self.state.is_maximized() {
                SW_MAXIMIZE
            } else {
                SW_SHOWNORMAL
            };
            unsafe {
                let _ = ShowWindow(hwnd, command);
            }
        }

        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        // If the window is minimized, restore it.
        if unsafe { IsIconic(hwnd).as_bool() } {
            unsafe {
                ShowWindowAsync(hwnd, SW_RESTORE).ok().log_err();
            }
        }

        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        unsafe {
            SetActiveWindow(hwnd).ok();
        }

        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        unsafe {
            SetFocus(Some(hwnd)).ok();
        }

        if self.is_native_window_terminal() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        // Foreground activation remains subject to the operating system's focus-stealing policy.
        // Never synthesize keyboard input to bypass that policy: framework commands must not
        // fabricate user input or feed it back through GPUI's must-immediate input boundary.
        if unsafe { SetForegroundWindow(hwnd).as_bool() } {
            PlatformWindowCommandOutcome::Accepted
        } else {
            PlatformWindowCommandOutcome::Rejected
        }
    }

    /// Applies a fullscreen transition on the window-owning thread.
    ///
    /// Initial presentation and live placement call this only on the window-owning thread after
    /// GPUI has installed callbacks. Live placement reports the resulting coherent facts through a
    /// mutation ticket.
    fn toggle_fullscreen_now(&self) -> Result<()> {
        let previous_fullscreen = self.state.fullscreen.take();
        let previous_restore_bounds = self.state.fullscreen_restore_bounds.get();
        let StyleAndBounds {
            style,
            x,
            y,
            cx,
            cy,
        } = match previous_fullscreen {
            Some(state) => state,
            None => {
                let (window_bounds, _) = self.state.calculate_window_bounds();

                let style = WINDOW_STYLE(
                    self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as _,
                );
                let mut rc = RECT::default();
                unsafe { GetWindowRect(self.hwnd, &mut rc) }
                    .context("failed to get window rect")?;
                let fullscreen_restore = StyleAndBounds {
                    style,
                    x: rc.left,
                    y: rc.top,
                    cx: rc.right - rc.left,
                    cy: rc.bottom - rc.top,
                };
                self.state.fullscreen_restore_bounds.set(window_bounds);
                let style = style
                    & !(WS_THICKFRAME | WS_SYSMENU | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION);
                let physical_bounds = self.state.display.get().physical_bounds();
                let fullscreen_bounds = StyleAndBounds {
                    style,
                    x: physical_bounds.left().0,
                    y: physical_bounds.top().0,
                    cx: physical_bounds.size.width.0,
                    cy: physical_bounds.size.height.0,
                };
                let result = self.apply_window_style_and_bounds(fullscreen_bounds);
                if result.is_ok() {
                    self.state.fullscreen.set(Some(fullscreen_restore));
                    set_non_rude_hwnd(self.hwnd, false)?;
                } else if self.native_is_fullscreen_from_native().unwrap_or(false) {
                    self.state.fullscreen.set(Some(fullscreen_restore));
                    if let Err(non_rude_error) = set_non_rude_hwnd(self.hwnd, false) {
                        return Err(result.expect_err("fullscreen application failed")).context(
                            format!(
                                "fullscreen NonRudeHWND recovery also failed: {non_rude_error:#}"
                            ),
                        );
                    }
                } else {
                    self.state
                        .fullscreen_restore_bounds
                        .set(previous_restore_bounds);
                }
                return result;
            }
        };

        let result = self.apply_window_style_and_bounds(StyleAndBounds {
            style,
            x,
            y,
            cx,
            cy,
        });
        if let Err(error) = result {
            if self.native_is_fullscreen_from_native().unwrap_or(false) {
                self.state.fullscreen.set(previous_fullscreen);
                self.state
                    .fullscreen_restore_bounds
                    .set(previous_restore_bounds);
            } else if let Err(non_rude_error) = set_non_rude_hwnd(self.hwnd, true) {
                return Err(error).context(format!(
                    "fullscreen NonRudeHWND recovery also failed: {non_rude_error:#}"
                ));
            }
            return Err(error);
        }
        self.state
            .border_offset
            .update_restored(self.hwnd)
            .context("failed to refresh restored window frame insets")?;
        set_non_rude_hwnd(self.hwnd, true)?;
        Ok(())
    }

    fn apply_window_style_and_bounds(&self, style_and_bounds: StyleAndBounds) -> Result<()> {
        let rollback = self.current_style_and_bounds()?;
        let StyleAndBounds {
            style,
            x,
            y,
            cx,
            cy,
        } = style_and_bounds;
        self.set_window_long_checked(GWL_STYLE, style.0 as isize, "failed to update window style")?;
        let placement_result = unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                cx,
                cy,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .context("failed to apply window style and bounds");
        if let Err(error) = placement_result {
            if let Err(rollback_error) = self.restore_style_and_bounds(rollback) {
                return Err(error).context(format!(
                    "window style-and-bounds rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    fn current_style_and_bounds(&self) -> Result<StyleAndBounds> {
        let style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as _,
        );
        let mut rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut rect) }.context("failed to get window rect")?;
        Ok(StyleAndBounds {
            style,
            x: rect.left,
            y: rect.top,
            cx: rect.right - rect.left,
            cy: rect.bottom - rect.top,
        })
    }

    fn restore_style_and_bounds(&self, snapshot: StyleAndBounds) -> Result<()> {
        self.set_window_long_checked(
            GWL_STYLE,
            snapshot.style.0 as isize,
            "failed to restore window style",
        )?;
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                snapshot.x,
                snapshot.y,
                snapshot.cx,
                snapshot.cy,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .context("failed to restore window bounds")?;
        Ok(())
    }

    fn capture_window_placement_snapshot(&self) -> Result<WindowPlacementRollbackSnapshot> {
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(self.hwnd, &mut placement) }
            .context("failed to capture window placement rollback state")?;
        Ok(WindowPlacementRollbackSnapshot {
            placement,
            style_and_bounds: self.current_style_and_bounds()?,
            visible: unsafe { IsWindowVisible(self.hwnd).as_bool() },
            border_offset: self.state.border_offset.snapshot(),
            fullscreen: self.state.fullscreen.get(),
            fullscreen_restore_bounds: self.state.fullscreen_restore_bounds.get(),
            non_rude_hwnd: non_rude_hwnd_for_fullscreen(self.state.fullscreen.get()),
        })
    }

    fn restore_window_placement_snapshot(
        &self,
        snapshot: WindowPlacementRollbackSnapshot,
    ) -> Result<()> {
        self.state.border_offset.restore(snapshot.border_offset);
        self.restore_style_and_bounds(snapshot.style_and_bounds)?;
        let mut placement = snapshot.placement;
        if !snapshot.visible {
            placement.showCmd = SW_HIDE.0 as u32;
        }
        unsafe { SetWindowPlacement(self.hwnd, &placement) }
            .context("failed to restore native window placement")?;
        if !snapshot.visible {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
        self.state.fullscreen.set(snapshot.fullscreen);
        self.state
            .fullscreen_restore_bounds
            .set(snapshot.fullscreen_restore_bounds);
        set_non_rude_hwnd(self.hwnd, snapshot.non_rude_hwnd)?;
        let physical_geometry = self
            .physical_geometry_from_native()
            .context("failed to read the native geometry restored by placement rollback")?;
        let display_observation = physical_geometry
            .display_observation()
            .context("restored native geometry did not identify its display")?;
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        anyhow::ensure!(
            snapshot.generation() == display_observation.topology_generation(),
            "display topology changed while placement rollback was committed"
        );
        let display = snapshot
            .display(display_observation.display_id())
            .context("restored native display is no longer available")?;
        self.state
            .set_display_binding(display, snapshot.generation());
        self.state
            .scale_factor
            .set(physical_geometry.scale_factor());
        self.state.border_offset.update_restored(self.hwnd)?;
        Ok(())
    }

    fn window_placement_for_bounds(&self, bounds: Bounds<Pixels>) -> Result<WINDOWPLACEMENT> {
        let client_bounds = bounds.to_device_pixels(self.state.display.get().scale_factor());
        let request = WindowPhysicalPlacementRequest::try_new(client_bounds)
            .context("window placement bounds are not representable in physical coordinates")?;
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(self.hwnd, &mut placement) }
            .context("failed to read native window placement")?;
        placement.rcNormalPosition =
            self.initial_window_rect_for_physical_client_bounds(request)?;
        Ok(placement)
    }

    fn set_window_restore_bounds(
        &self,
        bounds: Bounds<Pixels>,
        state: WindowPlacementState,
    ) -> Result<()> {
        let mut placement = self.window_placement_for_bounds(bounds)?;
        placement.showCmd = match state {
            WindowPlacementState::Windowed | WindowPlacementState::Fullscreen => {
                SW_SHOWNORMAL.0 as u32
            }
            WindowPlacementState::Maximized => SW_SHOWMAXIMIZED.0 as u32,
            WindowPlacementState::Minimized => SW_SHOWMINIMIZED.0 as u32,
        };
        unsafe { SetWindowPlacement(self.hwnd, &placement) }
            .context("failed to set window restore placement")?;
        Ok(())
    }

    fn set_fullscreen_restore_bounds(&self, bounds: Bounds<Pixels>) -> Result<()> {
        let placement = self.window_placement_for_bounds(bounds)?;
        unsafe { SetWindowPlacement(self.hwnd, &placement) }
            .context("failed to set fullscreen restore placement")?;
        self.state.fullscreen_restore_bounds.set(bounds);
        if let Some(mut fullscreen) = self.state.fullscreen.take() {
            let rect = placement.rcNormalPosition;
            fullscreen.x = rect.left;
            fullscreen.y = rect.top;
            fullscreen.cx = rect.right - rect.left;
            fullscreen.cy = rect.bottom - rect.top;
            self.state.fullscreen.set(Some(fullscreen));
        }
        Ok(())
    }

    fn apply_windowed_placement(&self, bounds: Bounds<Pixels>) -> Result<()> {
        if self.state.is_fullscreen() {
            self.toggle_fullscreen_now()?;
        }
        self.set_window_restore_bounds(bounds, WindowPlacementState::Windowed)?;
        if unsafe {
            IsWindowVisible(self.hwnd).as_bool()
                && (IsZoomed(self.hwnd).as_bool() || IsIconic(self.hwnd).as_bool())
        } {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
        }
        Ok(())
    }

    fn apply_physical_windowed_placement(
        &self,
        request: WindowPhysicalPlacementRequest,
    ) -> Result<()> {
        self.validate_physical_placement_target(request)?;
        let rollback = self.capture_window_placement_snapshot()?;
        let mut mutation_started = false;
        match self.apply_physical_windowed_placement_body(request, &mut mutation_started) {
            Ok(_) => Ok(()),
            Err(error) if !mutation_started => Err(error),
            Err(error) => {
                if let Err(rollback_error) = self.restore_window_placement_snapshot(rollback) {
                    return Err(error).context(format!(
                        "physical window placement rollback also failed: {rollback_error:#}"
                    ));
                }
                Err(error)
            }
        }
    }

    fn apply_physical_windowed_placement_body(
        &self,
        request: WindowPhysicalPlacementRequest,
        mutation_started: &mut bool,
    ) -> Result<PlatformWindowPhysicalGeometry> {
        self.validate_physical_placement_target(request)?;
        if self.state.is_fullscreen() {
            *mutation_started = true;
            self.toggle_fullscreen_now()?;
        }
        if unsafe {
            IsWindowVisible(self.hwnd).as_bool()
                && (IsZoomed(self.hwnd).as_bool() || IsIconic(self.hwnd).as_bool())
        } {
            *mutation_started = true;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
        }

        let rect = self.initial_window_rect_for_physical_client_bounds(request)?;
        let style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as u32,
        );
        *mutation_started = true;
        self.apply_window_style_and_bounds(StyleAndBounds {
            style,
            x: rect.left,
            y: rect.top,
            cx: rect.right - rect.left,
            cy: rect.bottom - rect.top,
        })?;
        self.settle_physical_client_bounds_exactly(request)
    }

    fn apply_hidden_physical_initial_placement(
        &self,
        request: WindowPhysicalPlacementRequest,
    ) -> Result<()> {
        anyhow::ensure!(
            self.has_pending_initial_placement()
                && !self.is_native_window_terminal()
                && unsafe { !IsWindowVisible(self.hwnd).as_bool() },
            "hidden physical initial placement requires one live unmapped window"
        );
        self.validate_physical_placement_target(request)?;
        let rollback = self.capture_window_placement_snapshot()?;
        let previous_facts = self.state.last_validated_platform_facts.borrow().clone();
        let mut open_status = self
            .state
            .initial_placement
            .take()
            .context("hidden physical placement lost its retained initial placement")?;
        let previous_open_status = open_status.clone();
        let mut mutation_started = false;
        let result = (|| {
            let settled_geometry =
                self.apply_physical_windowed_placement_body(request, &mut mutation_started)?;
            self.state.border_offset.update_restored(self.hwnd)?;
            let target_display = settled_geometry
                .display_observation()
                .context("settled hidden placement did not identify its target display")?;
            let target_overlap = request.client_bounds().intersect(&target_display.bounds());
            anyhow::ensure!(
                target_overlap.size.width.0 > 0 && target_overlap.size.height.0 > 0,
                "settled hidden placement did not overlap its observed target display"
            );
            let client_placement = if request.target_display().is_some() {
                request
            } else {
                WindowPhysicalPlacementRequest::try_new_for_display(
                    request.client_bounds(),
                    target_overlap.center(),
                    target_display,
                )
                .context("settled hidden placement could not bind its observed target display")?
            };
            open_status.state = WindowOpenState::Windowed;
            open_status.client_placement = client_placement;
            self.checkpoint_settled_initial_placement(&mut open_status, settled_geometry)
        })();
        match result {
            Ok(()) => {
                self.state.initial_placement.set(Some(open_status));
                self.refresh_pending_initial_platform_facts();
                Ok(())
            }
            Err(error) if !mutation_started => {
                self.state.initial_placement.set(Some(previous_open_status));
                self.state
                    .last_validated_platform_facts
                    .replace(previous_facts);
                Err(error)
            }
            Err(error) => {
                let rollback_result = self.restore_window_placement_snapshot(rollback);
                self.state.initial_placement.set(Some(previous_open_status));
                self.state
                    .last_validated_platform_facts
                    .replace(previous_facts);
                if let Err(rollback_error) = rollback_result {
                    return Err(error).context(format!(
                        "hidden physical placement rollback also failed: {rollback_error:#}"
                    ));
                }
                Err(error)
            }
        }
    }

    fn checkpoint_settled_initial_placement(
        &self,
        open_status: &mut WindowOpenStatus,
        settled_geometry: PlatformWindowPhysicalGeometry,
    ) -> Result<()> {
        anyhow::ensure!(
            open_status
                .client_placement
                .matches_geometry(settled_geometry),
            "settled hidden geometry does not match its retained client placement"
        );
        let display_observation = settled_geometry
            .display_observation()
            .context("settled hidden placement did not identify its display")?;
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        anyhow::ensure!(
            snapshot.generation() == display_observation.topology_generation(),
            "display topology changed while the hidden placement was checkpointed"
        );
        let display = snapshot
            .display(display_observation.display_id())
            .context("settled hidden placement display is no longer available")?;
        let mut outer_bounds = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut outer_bounds) }
            .context("failed to observe the settled hidden target window frame")?;
        open_status.placement.rcNormalPosition = outer_bounds;
        open_status.placement.showCmd = match open_status.state {
            WindowOpenState::Windowed | WindowOpenState::Fullscreen => SW_SHOWNORMAL.0 as u32,
            WindowOpenState::Maximized => SW_SHOWMAXIMIZED.0 as u32,
        };
        self.state
            .set_display_binding(display, display_observation.topology_generation());
        self.state.scale_factor.set(settled_geometry.scale_factor());
        Ok(())
    }

    fn apply_provisional_final_placement(
        &self,
        request: &WindowProvisionalPlacementRequest,
    ) -> Result<AppliedProvisionalFinalPlacement> {
        anyhow::ensure!(
            !self.is_native_window_terminal()
                && unsafe { IsWindowVisible(self.hwnd).as_bool() }
                && !unsafe { IsIconic(self.hwnd).as_bool() }
                && !unsafe { IsZoomed(self.hwnd).as_bool() }
                && !self.state.is_fullscreen(),
            "final provisional placement requires one visible windowed native window"
        );
        anyhow::ensure!(
            self.provisional_reveal_generation.get().is_some(),
            "final provisional placement requires an accepted native reveal"
        );
        let rollback = self.capture_provisional_placement_rollback()?;
        let rect =
            self.initial_window_rect_for_physical_client_bounds(request.physical_request())?;
        let requested_rect = NativeRect::try_from_rect(rect)
            .context("final provisional placement is empty or inverted")?;
        let prepared = self.prepare_provisional_z_order_band(
            request.anchor_point(),
            requested_rect,
            request.peer_windows(),
        )?;
        let registration_before = self
            .registered_window_snapshot(self.hwnd)?
            .filter(|current| current.matches(self.registration))
            .context("provisional native-window registration is stale")?;
        let foreground_before = unsafe { GetForegroundWindow() };
        let set_window_pos = unsafe {
            SetWindowPos(
                self.hwnd,
                Some(prepared.insert_after()),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            )
        }
        .context("failed to atomically place the provisional window in its final z-order band");
        if let Err(error) = set_window_pos {
            if let Err(rollback_error) = self.restore_provisional_placement_rollback(rollback) {
                return Err(error).context(format!(
                    "provisional placement rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }

        if let Err(error) = self.settle_physical_client_bounds_exactly(request.physical_request()) {
            if let Err(rollback_error) = self.restore_provisional_placement_rollback(rollback) {
                return Err(error).context(format!(
                    "provisional DPI convergence rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }

        let observation_epoch = self.state.native_placement_epoch();
        let z_order = self.verify_provisional_z_order_band(&prepared);
        let platform_facts = match self.observed_platform_facts_from_native() {
            Ok(facts) => facts,
            Err(error) => {
                if let Err(rollback_error) = self.restore_provisional_placement_rollback(rollback) {
                    return Err(error).context(format!(
                        "failed to observe the applied provisional placement and rollback also failed: {rollback_error:#}"
                    ));
                }
                return Err(error)
                    .context("failed to observe the applied provisional placement authority");
            }
        };
        let physical_geometry_exact = platform_facts
            .physical_geometry
            .is_some_and(|geometry| request.physical_request().matches_geometry(geometry));
        let stable_native_window_identity = self
            .registered_window_snapshot(self.hwnd)
            .ok()
            .flatten()
            .is_some_and(|current| {
                current.matches(registration_before) && current.matches(self.registration)
            });
        let facts = WindowProvisionalPlacementNativeFacts::new(
            physical_geometry_exact,
            unsafe { IsWindowVisible(self.hwnd).as_bool() },
            unsafe { GetForegroundWindow() } == foreground_before,
            self.provisional_native_hit_is_transparent(request.anchor_point()),
            stable_native_window_identity,
            z_order,
        );
        if self.state.native_placement_epoch() != observation_epoch || !facts.accepts_placement() {
            if let Err(rollback_error) = self.restore_provisional_placement_rollback(rollback) {
                return Err(anyhow::anyhow!(
                    "native provisional placement facts were incomplete ({facts:?}) and rollback failed: {rollback_error:#}"
                ));
            }
            return Err(anyhow::anyhow!(
                "native provisional placement did not establish every mandatory fact: {facts:?}"
            ));
        }
        let applied = match self.capture_provisional_placement_rollback() {
            Ok(applied) => applied,
            Err(error) => {
                if let Err(rollback_error) = self.restore_provisional_placement_rollback(rollback) {
                    return Err(error).context(format!(
                        "failed to capture the applied provisional placement and rollback also failed: {rollback_error:#}"
                    ));
                }
                return Err(error)
                    .context("failed to capture the applied provisional placement authority");
            }
        };
        if self.state.native_placement_epoch() != observation_epoch {
            if let Err(rollback_error) = self.restore_provisional_placement_rollback(rollback) {
                return Err(anyhow::anyhow!(
                    "native placement changed while its committed rollback authority was captured, and rollback failed: {rollback_error:#}"
                ));
            }
            return Err(anyhow::anyhow!(
                "native placement changed while its committed rollback authority was captured"
            ));
        }
        Ok(AppliedProvisionalFinalPlacement {
            facts,
            platform_facts,
            rollback,
            applied,
            applied_epoch: observation_epoch,
        })
    }

    #[cfg(test)]
    pub(crate) fn apply_provisional_final_placement_for_test(
        &self,
        request: &WindowProvisionalPlacementRequest,
    ) -> Result<WindowProvisionalPlacementNativeFacts> {
        self.apply_provisional_final_placement(request)
            .map(AppliedProvisionalFinalPlacement::commit)
    }

    #[cfg(test)]
    pub(crate) fn apply_and_rollback_provisional_final_placement_for_test(
        &self,
        request: &WindowProvisionalPlacementRequest,
    ) -> Result<WindowProvisionalPlacementNativeFacts> {
        let applied = self.apply_provisional_final_placement(request)?;
        let facts = applied.facts();
        let compensation = self.rollback_applied_provisional_final_placement(
            applied,
            ProvisionalPlacementCompensationAuthority::ImmediateNativeStack,
        )?;
        anyhow::ensure!(
            compensation.fully_restored(),
            "the applied provisional placement changed before the rollback test: {compensation:?}"
        );
        Ok(facts)
    }

    #[cfg(test)]
    pub(crate) fn provisional_delayed_rollback_rejects_native_rect_aba_for_test(
        &self,
        request: &WindowProvisionalPlacementRequest,
    ) -> Result<()> {
        let applied = self.apply_provisional_final_placement(request)?;
        let applied_rect = applied.applied.rect;
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                applied_rect.left + 17,
                applied_rect.top + 13,
                applied_rect.right - applied_rect.left,
                applied_rect.bottom - applied_rect.top,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .context("failed to apply the newer native placement used by the ABA test")?;
        self.restore_provisional_rect(applied_rect)
            .context("failed to restore the applied value used by the ABA test")?;

        let compensation = self.rollback_applied_provisional_final_placement(
            applied,
            ProvisionalPlacementCompensationAuthority::RegisteredOnly,
        )?;
        anyhow::ensure!(
            compensation == ProvisionalPlacementCompensation::authority_changed(),
            "a delayed compensation must reject native placement ABA: {compensation:?}"
        );
        anyhow::ensure!(
            Self::native_rect(self.hwnd)? == Some(applied_rect),
            "a rejected delayed compensation must preserve the newer native authority"
        );
        Ok(())
    }

    fn apply_maximized_placement(&self, restore_bounds: Bounds<Pixels>) -> Result<()> {
        if self.state.is_fullscreen() {
            self.toggle_fullscreen_now()?;
        }
        self.set_window_restore_bounds(restore_bounds, WindowPlacementState::Maximized)?;
        if unsafe { IsWindowVisible(self.hwnd).as_bool() } {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_MAXIMIZE);
            }
        }
        Ok(())
    }

    fn apply_fullscreen_placement(&self, restore_bounds: Bounds<Pixels>) -> Result<()> {
        if self.state.is_fullscreen() {
            return self.set_fullscreen_restore_bounds(restore_bounds);
        }

        if self.state.is_maximized() && unsafe { IsWindowVisible(self.hwnd).as_bool() } {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
        }
        self.set_window_restore_bounds(restore_bounds, WindowPlacementState::Windowed)?;
        self.toggle_fullscreen_now()?;
        self.state.fullscreen_restore_bounds.set(restore_bounds);
        Ok(())
    }

    fn apply_window_placement_request(
        &self,
        request: WindowPlacementRequest,
        current_facts: &WindowPlatformFacts,
    ) -> Result<()> {
        let rollback = self.capture_window_placement_snapshot()?;
        let result = (|| {
            self.state.scale_factor.set(current_facts.scale_factor);
            if let Some(display_id) = current_facts.display_id
                && let Some(coordinator) = self.native_retirement_coordinator.upgrade()
                && let Ok(snapshot) = coordinator.exact_display_topology_snapshot()
                && let Some(display) = snapshot.display(display_id)
            {
                self.state
                    .set_display_binding(display, snapshot.generation());
            }

            let window_bounds = current_facts.window_bounds;
            let current_state = if current_facts.is_minimized {
                WindowPlacementState::Minimized
            } else if current_facts.is_fullscreen {
                WindowPlacementState::Fullscreen
            } else if current_facts.is_maximized {
                WindowPlacementState::Maximized
            } else {
                WindowPlacementState::Windowed
            };

            if request.state.is_none() {
                return match current_state {
                    WindowPlacementState::Windowed => {
                        let bounds = Bounds::new(
                            request
                                .position
                                .unwrap_or(window_bounds.get_bounds().origin),
                            request.size.unwrap_or(window_bounds.get_bounds().size),
                        );
                        self.apply_windowed_placement(bounds)
                    }
                    WindowPlacementState::Maximized => self.set_window_restore_bounds(
                        request
                            .restore_bounds
                            .unwrap_or_else(|| window_bounds.get_bounds()),
                        WindowPlacementState::Maximized,
                    ),
                    WindowPlacementState::Fullscreen => self.set_fullscreen_restore_bounds(
                        request
                            .restore_bounds
                            .unwrap_or_else(|| window_bounds.get_bounds()),
                    ),
                    WindowPlacementState::Minimized => {
                        let restore_bounds = request
                            .restore_bounds
                            .unwrap_or_else(|| window_bounds.get_bounds());
                        if current_facts.is_fullscreen {
                            self.set_fullscreen_restore_bounds(restore_bounds)
                        } else {
                            self.set_window_restore_bounds(
                                restore_bounds,
                                WindowPlacementState::Minimized,
                            )
                        }
                    }
                };
            }

            match request.state.expect("state checked above") {
                WindowPlacementState::Windowed => {
                    let bounds = Bounds::new(
                        request
                            .position
                            .unwrap_or(window_bounds.get_bounds().origin),
                        request.size.unwrap_or(window_bounds.get_bounds().size),
                    );
                    self.apply_windowed_placement(bounds)
                }
                WindowPlacementState::Maximized => {
                    let restore_bounds = request
                        .restore_bounds
                        .unwrap_or_else(|| window_bounds.get_bounds());
                    self.apply_maximized_placement(restore_bounds)
                }
                WindowPlacementState::Fullscreen => {
                    let restore_bounds = request
                        .restore_bounds
                        .unwrap_or_else(|| window_bounds.get_bounds());
                    self.apply_fullscreen_placement(restore_bounds)
                }
                WindowPlacementState::Minimized => {
                    Err(anyhow::anyhow!("live minimized placement is not supported"))
                }
            }
        })();

        if let Err(error) = result {
            if let Err(rollback_error) = self.restore_window_placement_snapshot(rollback) {
                return Err(error).context(format!(
                    "window placement rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    fn set_accepts_pointer_input_now(&self, accepts_pointer_input: bool) -> Result<()> {
        let current = self.native_accepts_pointer_input()?;
        self.observe_accepts_pointer_input(current);
        if current == accepts_pointer_input {
            return Ok(());
        }
        let original_style =
            self.get_window_long_checked(GWL_EXSTYLE, "failed to read pointer-input window style")?;
        let mut style = original_style;
        if accepts_pointer_input {
            style &= !(WS_EX_TRANSPARENT.0 as isize);
        } else {
            style |= WS_EX_TRANSPARENT.0 as isize;
        }
        self.set_window_long_checked(
            GWL_EXSTYLE,
            style,
            "failed to update pointer-input window style",
        )?;
        #[cfg(test)]
        let fail_frame_change = self
            .state
            .fail_next_pointer_input_frame_change
            .replace(false);
        #[cfg(not(test))]
        let fail_frame_change = false;
        let frame_result = if fail_frame_change {
            Err(anyhow::anyhow!(
                "injected pointer-input frame-change failure"
            ))
        } else {
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED
                        | SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOACTIVATE
                        | SWP_NOOWNERZORDER
                        | SWP_NOZORDER,
                )
            }
            .context("failed to apply pointer-input window style")
        };
        if let Err(error) = frame_result {
            let rollback_result = self.set_window_long_checked(
                GWL_EXSTYLE,
                original_style,
                "failed to roll back pointer-input window style",
            );
            if rollback_result.is_ok() {
                let _ = unsafe {
                    SetWindowPos(
                        self.hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_FRAMECHANGED
                            | SWP_NOMOVE
                            | SWP_NOSIZE
                            | SWP_NOACTIVATE
                            | SWP_NOOWNERZORDER
                            | SWP_NOZORDER,
                    )
                };
            }
            if let Ok(actual) = self.native_accepts_pointer_input() {
                self.observe_accepts_pointer_input(actual);
            }
            if let Err(rollback_error) = rollback_result {
                return Err(error).context(format!(
                    "pointer-input style rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }
        let actual = self.native_accepts_pointer_input()?;
        self.observe_accepts_pointer_input(actual);
        if actual != accepts_pointer_input {
            return Err(anyhow::anyhow!(
                "native pointer-input style did not match the requested value"
            ));
        }
        Ok(())
    }

    fn observe_accepts_pointer_input(&self, actual: bool) {
        let previous = self.state.accepts_pointer_input.replace(actual);
        if previous != actual {
            let generation = self
                .state
                .pointer_input_observation_generation
                .get()
                .checked_add(1)
                .expect("pointer-input observation generation exhausted");
            self.state
                .pointer_input_observation_generation
                .set(generation);
        }
    }

    pub(crate) fn pointer_input_observation_generation(&self) -> u64 {
        self.state.pointer_input_observation_generation.get()
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn invalidate_pointer_input_observation_for_native_test(&self) {
        let generation = self
            .state
            .pointer_input_observation_generation
            .get()
            .checked_add(1)
            .expect("pointer-input observation generation exhausted");
        self.state
            .pointer_input_observation_generation
            .set(generation);
    }

    fn get_window_long_checked(
        &self,
        index: WINDOW_LONG_PTR_INDEX,
        error_context: &'static str,
    ) -> Result<isize> {
        unsafe {
            SetLastError(WIN32_ERROR(0));
            let value = get_window_long(self.hwnd, index);
            if value == 0 && GetLastError().0 != 0 {
                return Err(windows::core::Error::from_thread()).context(error_context);
            }
            Ok(value)
        }
    }

    fn set_window_long_checked(
        &self,
        index: WINDOW_LONG_PTR_INDEX,
        value: isize,
        error_context: &'static str,
    ) -> Result<()> {
        unsafe {
            SetLastError(WIN32_ERROR(0));
            if set_window_long(self.hwnd, index, value) == 0 && GetLastError().0 != 0 {
                return Err(windows::core::Error::from_thread()).context(error_context);
            }
        }
        Ok(())
    }

    fn native_accepts_pointer_input(&self) -> Result<bool> {
        Ok((self
            .get_window_long_checked(GWL_EXSTYLE, "failed to read pointer-input window style")?
            & WS_EX_TRANSPARENT.0 as isize)
            == 0)
    }

    fn native_activation_policy(&self) -> Result<WindowActivationPolicy> {
        let focus_on_click = (self
            .get_window_long_checked(GWL_EXSTYLE, "failed to read activation window style")?
            & WS_EX_NOACTIVATE.0 as isize)
            == 0;
        let mut policy = self.state.activation_policy.get();
        policy.focus_on_click = focus_on_click;
        self.state.activation_policy.set(policy);
        Ok(policy)
    }

    fn refresh_activation_window_frame(&self, context: &'static str) -> Result<()> {
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED
                    | SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOACTIVATE
                    | SWP_NOOWNERZORDER
                    | SWP_NOZORDER,
            )
        }
        .context(context)
    }

    fn set_activation_policy_now(&self, requested: WindowActivationPolicy) -> Result<()> {
        let original = self.native_activation_policy()?;
        if original == requested {
            return Ok(());
        }
        if original.focus_on_click == requested.focus_on_click {
            self.state.activation_policy.set(requested);
            return Ok(());
        }

        let original_style =
            self.get_window_long_checked(GWL_EXSTYLE, "failed to read activation window style")?;
        let requested_style = if requested.focus_on_click {
            original_style & !(WS_EX_NOACTIVATE.0 as isize)
        } else {
            original_style | WS_EX_NOACTIVATE.0 as isize
        };
        self.set_window_long_checked(
            GWL_EXSTYLE,
            requested_style,
            "failed to update activation window style",
        )?;
        #[cfg(test)]
        let fail_frame_change = self
            .state
            .fail_next_activation_policy_frame_change
            .replace(false);
        #[cfg(not(test))]
        let fail_frame_change = false;
        let apply_result = if fail_frame_change {
            Err(anyhow::anyhow!(
                "injected activation-policy frame-change failure"
            ))
        } else {
            self.refresh_activation_window_frame("failed to apply activation window style")
        };

        if apply_result.is_err() {
            let rollback_result = self.set_window_long_checked(
                GWL_EXSTYLE,
                original_style,
                "failed to roll back activation window style",
            );
            if rollback_result.is_ok() {
                self.refresh_activation_window_frame(
                    "failed to refresh rolled-back activation window style",
                )
                .log_err();
            }
            rollback_result.log_err();
        }

        let native_focus_on_click = match self
            .get_window_long_checked(GWL_EXSTYLE, "failed to verify activation window style")
        {
            Ok(style) => (style & WS_EX_NOACTIVATE.0 as isize) == 0,
            Err(error) => {
                self.set_window_long_checked(
                    GWL_EXSTYLE,
                    original_style,
                    "failed to restore activation style after readback failure",
                )
                .log_err();
                self.state.activation_policy.set(original);
                return Err(error);
            }
        };
        if native_focus_on_click == requested.focus_on_click {
            self.state.activation_policy.set(requested);
            return Ok(());
        }

        self.state.activation_policy.set(original);
        if let Err(error) = apply_result {
            return Err(error);
        }
        Err(anyhow::anyhow!(
            "native activation style did not match the requested value"
        ))
    }

    fn native_is_fullscreen(
        window_rect: RECT,
        monitor: HMONITOR,
        window_style: WINDOW_STYLE,
    ) -> bool {
        if monitor.is_invalid() || !Self::has_fullscreen_window_style(window_style) {
            return false;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let read_monitor_info = unsafe { GetMonitorInfoW(monitor, &mut monitor_info).as_bool() };
        if !read_monitor_info {
            return false;
        }
        let monitor_rect = monitor_info.rcMonitor;
        window_rect.left <= monitor_rect.left
            && window_rect.top <= monitor_rect.top
            && window_rect.right >= monitor_rect.right
            && window_rect.bottom >= monitor_rect.bottom
    }

    fn native_is_fullscreen_from_native(&self) -> Result<bool> {
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }
            .context("failed to read native fullscreen bounds")?;
        let monitor = unsafe { MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONULL) };
        let window_style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as u32,
        );
        Ok(Self::native_is_fullscreen(
            window_rect,
            monitor,
            window_style,
        ))
    }

    fn has_fullscreen_window_style(window_style: WINDOW_STYLE) -> bool {
        !window_style.contains(WS_THICKFRAME)
            && !window_style.contains(WS_SYSMENU)
            && !window_style.contains(WS_MAXIMIZEBOX)
            && !window_style.contains(WS_MINIMIZEBOX)
            && !window_style.contains(WS_CAPTION)
    }

    fn has_pending_initial_placement(&self) -> bool {
        self.pending_initial_placement().is_some()
    }

    fn pending_initial_placement(&self) -> Option<WindowOpenStatus> {
        let initial_placement = self.state.initial_placement.take();
        self.state.initial_placement.set(initial_placement.clone());
        initial_placement
    }

    fn validate_initial_placement_authority(&self, open_status: &WindowOpenStatus) -> Result<()> {
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        let target_display = open_status
            .client_placement
            .target_display()
            .context("retained initial placement lost its exact target display observation")?;
        let target_point = open_status
            .client_placement
            .target_point()
            .context("retained initial placement lost its exact target point")?;
        snapshot
            .validate_target(target_display, target_point)
            .context(
                "display topology changed before the retained initial placement was committed",
            )?;
        Ok(())
    }

    fn take_validated_pending_initial_placement(&self) -> Result<Option<WindowOpenStatus>> {
        let Some(open_status) = self.state.initial_placement.take() else {
            return Ok(None);
        };
        let result = self.validate_initial_placement_authority(&open_status);
        if let Err(error) = result {
            self.state.initial_placement.set(Some(open_status));
            return Err(error);
        }
        Ok(Some(open_status))
    }

    fn merge_deferred_initial_placement(&self, request: DeferredPlacementRequest) -> Result<()> {
        match request {
            DeferredPlacementRequest::Logical(request) => {
                self.merge_deferred_logical_initial_placement(request)
            }
            DeferredPlacementRequest::Physical(request) => {
                self.apply_hidden_physical_initial_placement(request)
            }
        }
    }

    fn merge_deferred_logical_initial_placement(
        &self,
        request: WindowPlacementRequest,
    ) -> Result<()> {
        let Some(mut open_status) = self.take_validated_pending_initial_placement()? else {
            anyhow::bail!("pending creation placement disappeared before activation");
        };
        let previous = open_status.clone();
        let result = (|| {
            let mut restore_bounds = open_status.logical_client_bounds();
            if let Some(bounds) = request.restore_bounds {
                restore_bounds = bounds;
            }
            if let Some(position) = request.position {
                restore_bounds.origin = position;
            }
            if let Some(size) = request.size {
                restore_bounds.size = size;
            }
            if let Some(state) = request.state {
                open_status.state = match state {
                    WindowPlacementState::Windowed => WindowOpenState::Windowed,
                    WindowPlacementState::Maximized => WindowOpenState::Maximized,
                    WindowPlacementState::Fullscreen => WindowOpenState::Fullscreen,
                    WindowPlacementState::Minimized => {
                        anyhow::bail!("live minimized placement is not supported");
                    }
                };
            }

            let target_display = open_status.target_display();
            let physical_client_bounds =
                restore_bounds.to_device_pixels(target_display.scale_factor());
            open_status.client_placement = WindowPhysicalPlacementRequest::try_new_for_display(
                physical_client_bounds,
                physical_client_bounds.center(),
                target_display,
            )
            .context("deferred logical placement left its retained target display")?;
            open_status.placement.rcNormalPosition =
                self.initial_window_rect_for_physical_client_bounds(open_status.client_placement)?;
            open_status.placement.showCmd = match open_status.state {
                WindowOpenState::Windowed | WindowOpenState::Fullscreen => SW_SHOWNORMAL.0 as u32,
                WindowOpenState::Maximized => SW_SHOWMAXIMIZED.0 as u32,
            };
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.state.initial_placement.set(Some(open_status));
                self.refresh_pending_initial_platform_facts();
                Ok(())
            }
            Err(error) => {
                self.state.initial_placement.set(Some(previous));
                Err(error)
            }
        }
    }

    fn observed_platform_facts(&self) -> WindowPlatformFacts {
        match self.observed_platform_facts_from_native() {
            Ok(facts) => self.project_pending_initial_placement(facts),
            Err(error) => {
                log::warn!("Windows platform fact readback failed: {error:#}");
                self.state
                    .last_validated_platform_facts
                    .borrow()
                    .clone()
                    .map(|facts| self.project_pending_initial_placement(facts))
                    .expect("Windows window construction seeds coherent platform facts")
            }
        }
    }

    fn refresh_pending_initial_platform_facts(&self) {
        let facts = self.project_pending_initial_placement(self.cached_platform_facts());
        self.state
            .last_validated_platform_facts
            .replace(Some(facts));
    }

    fn last_validated_platform_facts(&self) -> WindowPlatformFacts {
        self.state
            .last_validated_platform_facts
            .borrow()
            .clone()
            .expect("Windows window construction seeds coherent platform facts")
    }

    fn project_pending_initial_placement(
        &self,
        mut facts: WindowPlatformFacts,
    ) -> WindowPlatformFacts {
        let Some(open_status) = self.pending_initial_placement() else {
            return facts;
        };
        let target_display = open_status.target_display();
        let physical_client_bounds = open_status.client_placement.client_bounds();
        let restore_bounds = open_status.logical_client_bounds();
        let physical_geometry = PlatformWindowPhysicalGeometry::try_new(
            physical_client_bounds,
            target_display.scale_factor(),
        )
        .and_then(|geometry| geometry.with_display_observation(target_display))
        .expect("retained initial placement facts remain physically coherent");
        let window_bounds = match open_status.state {
            WindowOpenState::Windowed => WindowBounds::Windowed(restore_bounds),
            WindowOpenState::Maximized => WindowBounds::Maximized(restore_bounds),
            WindowOpenState::Fullscreen => WindowBounds::Fullscreen(restore_bounds),
        };
        facts.bounds = restore_bounds;
        facts.physical_geometry = Some(physical_geometry);
        facts.window_bounds = window_bounds;
        facts.inner_window_bounds = window_bounds;
        facts.content_size = restore_bounds.size;
        facts.scale_factor = target_display.scale_factor();
        facts.display_id = Some(target_display.display_id());
        facts.is_minimized = false;
        facts.is_maximized = matches!(open_status.state, WindowOpenState::Maximized);
        facts.is_fullscreen = matches!(open_status.state, WindowOpenState::Fullscreen);
        facts.is_active = false;
        facts
    }

    #[cfg(test)]
    pub(crate) fn observed_platform_facts_for_test(&self) -> Result<WindowPlatformFacts> {
        self.observed_platform_facts_from_native()
    }

    fn observed_platform_facts_from_native(&self) -> Result<WindowPlatformFacts> {
        let observation_epoch = self.state.native_placement_epoch();
        let physical_geometry = self.physical_geometry_from_native()?;
        let scale_factor = physical_geometry.scale_factor();
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }
            .context("failed to read native window bounds")?;
        let bounds = physical_geometry.client_bounds().to_pixels(scale_factor);
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(self.hwnd, &mut placement) }
            .context("failed to read native window placement")?;
        let is_minimized = unsafe { IsIconic(self.hwnd).as_bool() };
        let display = physical_geometry
            .display_observation()
            .context("native physical geometry did not identify its display")?;
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        anyhow::ensure!(
            snapshot.generation() == display.topology_generation(),
            "native window facts crossed display topology generations"
        );
        let monitor = snapshot
            .native_monitor_for_display(display.display_id())
            .context("native window display is no longer available")?;
        let window_style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as u32,
        );
        let window_ex_style = WINDOW_EX_STYLE(
            self.get_window_long_checked(GWL_EXSTYLE, "failed to read extended window style")?
                as u32,
        );
        let is_fullscreen = self.state.is_fullscreen()
            && if is_minimized {
                Self::has_fullscreen_window_style(window_style)
            } else {
                Self::native_is_fullscreen(window_rect, monitor, window_style)
            };
        let is_maximized = !is_fullscreen
            && (placement.showCmd == SW_SHOWMAXIMIZED.0 as u32
                || (is_minimized && placement.flags.contains(WPF_RESTORETOMAXIMIZED)));
        let restore_bounds = calculate_client_rect(
            placement.rcNormalPosition,
            &self.state.border_offset,
            scale_factor,
        );
        let window_bounds = if is_fullscreen {
            WindowBounds::Fullscreen(restore_bounds)
        } else if is_maximized {
            WindowBounds::Maximized(restore_bounds)
        } else {
            WindowBounds::Windowed(restore_bounds)
        };
        let display_id = Some(display.display_id());
        let accepts_pointer_input = self.native_accepts_pointer_input()?;
        let activation_policy = self.state.activation_policy.get();
        let focus_on_click = window_ex_style.0 & WS_EX_NOACTIVATE.0 == 0;
        let taskbar_visible = window_ex_style.0 & WS_EX_APPWINDOW.0 != 0
            && window_ex_style.0 & WS_EX_TOOLWINDOW.0 == 0;
        let topmost = window_ex_style.0 & WS_EX_TOPMOST.0 != 0;

        let facts = WindowPlatformFacts {
            bounds,
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            physical_geometry: Some(physical_geometry),
            window_bounds,
            inner_window_bounds: window_bounds,
            content_size: bounds.size,
            scale_factor,
            display_id,
            is_minimized,
            is_maximized,
            is_fullscreen,
            accepts_pointer_input,
            accepts_activation: activation_policy.accepts_activation,
            focus_on_click,
            background_appearance: self.state.background_appearance.get(),
            topmost,
            taskbar_visible,
            is_active: self.hwnd == unsafe { GetForegroundWindow() },
        };
        anyhow::ensure!(
            self.state.native_placement_epoch() == observation_epoch,
            "native window placement changed while platform facts were sampled"
        );
        self.state
            .last_validated_platform_facts
            .replace(Some(facts.clone()));
        Ok(facts)
    }

    fn physical_client_bounds_observation(&self) -> Result<Bounds<DevicePixels>> {
        let mut client_rect = RECT::default();
        unsafe { GetClientRect(self.hwnd, &mut client_rect) }
            .context("failed to read native client bounds")?;
        let mut client_origin = POINT {
            x: client_rect.left,
            y: client_rect.top,
        };
        unsafe { ClientToScreen(self.hwnd, &mut client_origin) }
            .ok()
            .context("failed to read native client origin")?;
        let width = client_rect
            .right
            .checked_sub(client_rect.left)
            .context("native client width overflowed")?;
        let height = client_rect
            .bottom
            .checked_sub(client_rect.top)
            .context("native client height overflowed")?;
        anyhow::ensure!(
            width >= 0 && height >= 0,
            "native client bounds were inverted"
        );
        Ok(Bounds::new(
            Point::new(DevicePixels(client_origin.x), DevicePixels(client_origin.y)),
            size(DevicePixels(width), DevicePixels(height)),
        ))
    }

    fn validate_physical_placement_target(
        &self,
        request: WindowPhysicalPlacementRequest,
    ) -> Result<Option<HMONITOR>> {
        let Some(target_display) = request.target_display() else {
            return Ok(None);
        };
        let target_point = request
            .target_point()
            .context("display-bound physical placement is missing its target point")?;
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        let target = snapshot
            .validate_target(target_display, target_point)
            .context("physical placement target display changed before native commit")?;
        Ok(Some(target.monitor))
    }

    fn initial_window_rect_for_physical_client_bounds(
        &self,
        request: WindowPhysicalPlacementRequest,
    ) -> Result<RECT> {
        let target_monitor = self.validate_physical_placement_target(request)?;
        let client_bounds = request.client_bounds();
        let target_dpi = target_monitor_dpi_for_physical_placement(request, target_monitor)?;
        let style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read native window style")? as u32,
        );
        let extended_style =
            WINDOW_EX_STYLE(self.get_window_long_checked(
                GWL_EXSTYLE,
                "failed to read native extended window style",
            )? as u32);
        let has_menu = !unsafe { GetMenu(self.hwnd) }.is_invalid();
        adjusted_window_rect_for_dpi(client_bounds, style, extended_style, has_menu, target_dpi)
    }

    fn window_rect_for_current_physical_frame(
        &self,
        client_bounds: Bounds<DevicePixels>,
    ) -> Result<RECT> {
        let current_client = self.physical_client_bounds_observation()?;
        let mut current_window = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut current_window) }
            .context("failed to read the current native window frame")?;
        window_rect_from_observed_frame(client_bounds, current_client, current_window)
    }

    fn settle_physical_client_bounds_exactly(
        &self,
        request: WindowPhysicalPlacementRequest,
    ) -> Result<PlatformWindowPhysicalGeometry> {
        self.validate_physical_placement_target(request)?;
        let client_bounds = request.client_bounds();
        let observed = self.physical_geometry_from_native()?;
        if request.matches_geometry(observed) {
            return Ok(observed);
        }

        // Crossing a DPI boundary synchronously dispatches WM_DPICHANGED. The handler applies
        // Windows' suggested frame before the outer SetWindowPos returns, so derive one exact
        // correction from the now-current target-DPI frame instead of retrying on a timer.
        let corrected = self.window_rect_for_current_physical_frame(client_bounds)?;
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                corrected.left,
                corrected.top,
                corrected.right - corrected.left,
                corrected.bottom - corrected.top,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .context("failed to converge the native client bounds at the target DPI")?;

        self.validate_physical_placement_target(request)?;
        let observed = self.physical_geometry_from_native()?;
        anyhow::ensure!(
            request.matches_geometry(observed),
            "native physical client geometry did not converge exactly on the target display"
        );
        Ok(observed)
    }

    fn physical_scale_factor_observation(&self) -> Result<f32> {
        let dpi = unsafe { GetDpiForWindow(self.hwnd) };
        anyhow::ensure!(dpi != 0, "failed to read native window DPI");
        let scale_factor = dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32;
        anyhow::ensure!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "native window DPI scale was invalid"
        );
        Ok(scale_factor)
    }

    fn physical_geometry_observation(
        &self,
        snapshot: &WindowsDisplayTopologySnapshot,
    ) -> Result<PlatformWindowPhysicalGeometry> {
        self.physical_geometry_native_sample(snapshot)
            .map(|(geometry, _)| geometry)
    }

    fn physical_geometry_native_sample(
        &self,
        snapshot: &WindowsDisplayTopologySnapshot,
    ) -> Result<(
        PlatformWindowPhysicalGeometry,
        ValidatedWindowsNativeDisplay,
    )> {
        let bounds = self.physical_client_bounds_observation()?;
        let scale_factor = self.physical_scale_factor_observation()?;
        let monitor = unsafe { MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONULL) };
        let native_display = snapshot
            .validated_native_display(monitor)
            .context("failed to resolve the native window display")?;
        let geometry = PlatformWindowPhysicalGeometry::try_new(bounds, scale_factor)
            .and_then(|geometry| geometry.with_display_observation(native_display.observation()))
            .context("native physical client geometry and display were not coherent")?;
        Ok((geometry, native_display))
    }

    pub(crate) fn physical_geometry_from_native(&self) -> Result<PlatformWindowPhysicalGeometry> {
        let coordinator = self
            .native_retirement_coordinator
            .upgrade()
            .context("Windows platform topology authority is no longer available")?;
        let first_placement_epoch = self.state.native_placement_epoch();
        let snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        let first_geometry = self.physical_geometry_observation(&snapshot)?;
        let second_geometry = self.physical_geometry_observation(&snapshot)?;
        let final_snapshot = coordinator
            .exact_display_topology_snapshot()
            .context("display topology is unavailable")?;
        let final_placement_epoch = self.state.native_placement_epoch();
        anyhow::ensure!(
            physical_geometry_sample_is_stable(
                first_placement_epoch,
                final_placement_epoch,
                snapshot.generation(),
                final_snapshot.generation(),
                first_geometry,
                second_geometry,
            ),
            "native physical client geometry or display topology changed while it was sampled"
        );
        Ok(second_geometry)
    }

    fn cached_platform_facts(&self) -> WindowPlatformFacts {
        let window_bounds = self.state.window_bounds();
        WindowPlatformFacts {
            bounds: self.state.bounds(),
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            physical_geometry: None,
            window_bounds,
            inner_window_bounds: window_bounds,
            content_size: self.state.content_size(),
            scale_factor: self.state.scale_factor.get(),
            display_id: Some(self.state.display.get().id()),
            is_minimized: unsafe { IsIconic(self.hwnd).as_bool() },
            is_maximized: self.state.is_maximized(),
            is_fullscreen: self.state.is_fullscreen(),
            accepts_pointer_input: self.state.accepts_pointer_input(),
            accepts_activation: self.state.activation_policy.get().accepts_activation,
            focus_on_click: self.state.activation_policy.get().focus_on_click,
            background_appearance: self.state.background_appearance.get(),
            topmost: false,
            taskbar_visible: self.state.taskbar_visible,
            is_active: self.hwnd == unsafe { GetForegroundWindow() },
        }
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        match domain {
            WindowMutationDomain::Placement => {
                self.state
                    .placement_mutation_generation
                    .set(Some(generation));
                self.state.deferred_placement_mutation.set(None);
            }
            WindowMutationDomain::PointerInput => {
                self.state
                    .pointer_input_mutation_generation
                    .set(Some(generation));
            }
            WindowMutationDomain::ActivationPolicy => {
                self.state
                    .activation_policy_mutation_generation
                    .set(Some(generation));
            }
            WindowMutationDomain::Alpha
            | WindowMutationDomain::Topmost
            | WindowMutationDomain::TaskbarVisibility => {}
        }
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        match domain {
            WindowMutationDomain::Placement => {
                self.state.placement_mutation_generation.set(None);
                self.state.deferred_placement_mutation.set(None);
            }
            WindowMutationDomain::PointerInput => {
                self.state.pointer_input_mutation_generation.set(None);
            }
            WindowMutationDomain::ActivationPolicy => {
                self.state.activation_policy_mutation_generation.set(None);
            }
            WindowMutationDomain::Alpha
            | WindowMutationDomain::Topmost
            | WindowMutationDomain::TaskbarVisibility => {}
        }
    }

    fn placement_mutation_is_current(&self, generation: u64) -> bool {
        self.state.placement_mutation_generation.get() == Some(generation)
    }

    fn pointer_input_mutation_is_current(&self, generation: u64) -> bool {
        self.state.pointer_input_mutation_generation.get() == Some(generation)
    }

    fn activation_policy_mutation_is_current(&self, generation: u64) -> bool {
        self.state.activation_policy_mutation_generation.get() == Some(generation)
    }

    fn terminal_facts_after_mutation(
        &self,
        mutation: &str,
        result: Result<()>,
        before_facts: WindowPlatformFacts,
    ) -> (PlatformWindowMutationTerminal, WindowPlatformFacts) {
        match result {
            Ok(()) => match self.observed_platform_facts_from_native() {
                Ok(facts) => (PlatformWindowMutationTerminal::Observed, facts),
                Err(error) => {
                    log::warn!(
                        "Windows {mutation} completed but terminal fact readback failed: {error:#}"
                    );
                    (PlatformWindowMutationTerminal::Rejected, before_facts)
                }
            },
            Err(error) => {
                log::warn!("Windows {mutation} request failed: {error:#}");
                match self.observed_platform_facts_from_native() {
                    Ok(facts) => (PlatformWindowMutationTerminal::Rejected, facts),
                    Err(readback_error) => {
                        log::warn!(
                            "Windows {mutation} rejected and terminal fact readback failed: {readback_error:#}"
                        );
                        (PlatformWindowMutationTerminal::Rejected, before_facts)
                    }
                }
            }
        }
    }

    fn emit_window_mutation_observation(
        &self,
        domain: WindowMutationDomain,
        generation: u64,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) {
        let _ = with_windows_callback(
            &self.state.callbacks.window_mutation_observation,
            |callback| {
                callback(PlatformWindowMutationObservation::terminal(
                    domain, generation, terminal, facts,
                ));
            },
        );
    }

    pub(crate) fn system_settings(&self) -> &WindowsSystemSettings {
        &self.system_settings
    }
}

/// Invokes a temporarily checked-out native callback and restores it during unwinding.
/// A callback installed reentrantly is authoritative and replaces the checked-out callback.
pub(crate) fn with_windows_callback<T, R>(
    slot: &Cell<Option<T>>,
    invoke: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    let callback = slot.take()?;
    let mut checkout = WindowsCallbackCheckout {
        slot,
        callback: Some(callback),
    };
    Some(invoke(checkout.callback.as_mut().expect(
        "checked-out Windows callback must remain available",
    )))
}

struct WindowsCallbackCheckout<'a, T> {
    slot: &'a Cell<Option<T>>,
    callback: Option<T>,
}

impl<T> Drop for WindowsCallbackCheckout<'_, T> {
    fn drop(&mut self) {
        let replacement = self.slot.take();
        if let Some(replacement) = replacement {
            self.slot.set(Some(replacement));
        } else {
            self.slot.set(self.callback.take());
        }
    }
}

#[derive(Default)]
pub(crate) struct WindowsShouldCloseCallbackSlot {
    callback: Cell<Option<Box<dyn FnMut() -> bool>>>,
    checked_out: Cell<bool>,
    terminal: Cell<bool>,
}

impl WindowsShouldCloseCallbackSlot {
    fn set(&self, callback: Box<dyn FnMut() -> bool>) {
        if !self.terminal.get() {
            self.callback.set(Some(callback));
        }
    }

    pub(crate) fn invoke(&self) -> bool {
        if self.terminal.get() || self.checked_out.replace(true) {
            return false;
        }
        let Some(callback) = self.callback.take() else {
            self.checked_out.set(false);
            return false;
        };
        let mut checkout = WindowsShouldCloseCallbackCheckout {
            slot: self,
            callback: Some(callback),
        };
        checkout
            .callback
            .as_mut()
            .expect("checked-out should-close callback must remain available")()
    }

    fn terminate(&self) {
        self.terminal.set(true);
        self.callback.take();
    }
}

struct WindowsShouldCloseCallbackCheckout<'a> {
    slot: &'a WindowsShouldCloseCallbackSlot,
    callback: Option<Box<dyn FnMut() -> bool>>,
}

impl Drop for WindowsShouldCloseCallbackCheckout<'_> {
    fn drop(&mut self) {
        self.slot.checked_out.set(false);
        let replacement = self.slot.callback.take();
        if self.slot.terminal.get() {
            return;
        }
        if let Some(replacement) = replacement {
            self.slot.callback.set(Some(replacement));
        } else {
            self.slot.callback.set(self.callback.take());
        }
    }
}

#[derive(Default)]
pub(crate) struct Callbacks {
    pub(crate) request_frame: Cell<Option<Box<dyn FnMut(RequestFrameOptions)>>>,
    pub(crate) input: PlatformInputCallbackSlot,
    pub(crate) modifiers_changed: Cell<Option<Box<dyn FnMut(ModifiersChangedEvent)>>>,
    pub(crate) active_status_change:
        Cell<Option<Box<dyn FnMut(PlatformWindowActiveStatusObservation)>>>,
    pub(crate) hovered_status_change: Cell<Option<Box<dyn FnMut(bool)>>>,
    pub(crate) resize: Cell<Option<Box<dyn FnMut(Size<Pixels>, f32)>>>,
    pub(crate) moved: Cell<Option<Box<dyn FnMut()>>>,
    pub(crate) window_state_change: Cell<Option<Box<dyn FnMut()>>>,
    pub(crate) window_mutation_observation:
        Cell<Option<Box<dyn FnMut(PlatformWindowMutationObservation)>>>,
    pub(crate) should_close: WindowsShouldCloseCallbackSlot,
    pub(crate) close: Cell<Option<Box<dyn FnOnce()>>>,
    pub(crate) hit_test_window_control: Cell<Option<Box<dyn FnMut() -> Option<WindowControlArea>>>>,
    pub(crate) appearance_changed: Cell<Option<Box<dyn FnMut()>>>,
}

impl Callbacks {
    pub(crate) fn set_input(&self, callback: PlatformInputCallback) {
        self.input.set(callback);
    }

    #[cfg(test)]
    pub(crate) fn set_test_input(
        &self,
        callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    ) {
        self.set_input(PlatformInputCallback::new_unleased_for_test(callback));
    }
}

struct WindowCreateContext {
    inner: Option<Result<Rc<WindowsWindowInner>>>,
    handle: AnyWindowHandle,
    hide_title_bar: bool,
    display: WindowsDisplay,
    display_topology_generation: u64,
    is_movable: bool,
    min_size: Option<Size<Pixels>>,
    executor: ForegroundExecutor,
    current_cursor: Option<HCURSOR>,
    cursor_visible: Arc<AtomicBool>,
    drop_target_helper: IDropTargetHelper,
    validation_number: usize,
    native_window_generation: usize,
    recovered_directx_devices: Arc<parking_lot::RwLock<Option<DirectXDevices>>>,
    main_receiver: PriorityQueueReceiver<RunnableVariant>,
    platform_window_handle: HWND,
    raw_window_handles: std::sync::Weak<RegisteredWindows>,
    native_retirement_coordinator: std::rc::Weak<WindowsPlatformInner>,
    appearance: WindowAppearance,
    disable_direct_composition: bool,
    directx_devices: DirectXDevices,
    invalidate_devices: Arc<AtomicBool>,
    owner_hwnd: Option<HWND>,
    modal_parent_disabled: bool,
    creation_show: bool,
    show_on_initial_presentation: bool,
    provisional_session: Option<WindowProvisionalSession>,
    accepts_pointer_input: bool,
    focus_on_appearing: bool,
    activation_policy: WindowActivationPolicy,
    transient_for: Option<AnyWindowHandle>,
    taskbar_visible: bool,
    #[cfg(test)]
    lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
}

impl WindowsWindow {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        transient_owner_hwnd: Option<HWND>,
        creation_info: WindowCreationInfo,
    ) -> Result<Self> {
        let WindowCreationInfo {
            icon,
            executor,
            current_cursor,
            cursor_visible,
            drop_target_helper,
            validation_number,
            native_window_generation,
            recovered_directx_devices,
            main_receiver,
            platform_window_handle,
            raw_window_handles,
            native_retirement_coordinator,
            display_topology,
            disable_direct_composition,
            directx_devices,
            invalidate_devices,
            #[cfg(test)]
            lifecycle_test_probe,
        } = creation_info;
        let provisional_session = params.provisional_session.clone();
        register_window_class(icon);
        let hide_title_bar = params
            .titlebar
            .as_ref()
            .map(|titlebar| titlebar.appears_transparent)
            .unwrap_or(true);
        let window_name = HSTRING::from(
            params
                .titlebar
                .as_ref()
                .and_then(|titlebar| titlebar.title.as_ref())
                .map(|title| title.as_ref())
                .unwrap_or(""),
        );

        let (mut dwexstyle, dwstyle) = if params.kind == WindowKind::PopUp {
            (WS_EX_TOOLWINDOW, WINDOW_STYLE(0x0))
        } else {
            let mut dwstyle = WS_SYSMENU;

            if params.is_resizable {
                dwstyle |= WS_THICKFRAME | WS_MAXIMIZEBOX;
            }

            if params.is_minimizable {
                dwstyle |= WS_MINIMIZEBOX;
            }
            let dwexstyle = if params.kind == WindowKind::Dialog {
                dwstyle |= WS_POPUP | WS_CAPTION;
                WS_EX_DLGMODALFRAME
            } else {
                WS_EX_APPWINDOW
            };

            (dwexstyle, dwstyle)
        };
        if !disable_direct_composition {
            dwexstyle |= WS_EX_NOREDIRECTIONBITMAP;
        }
        if !params.accepts_pointer_input {
            dwexstyle |= WS_EX_TRANSPARENT;
        }
        if !params.activation_policy.focus_on_click {
            dwexstyle |= WS_EX_NOACTIVATE;
        }
        let focus_on_appearing = params.focus_on_appearing;
        let activation_policy = params.activation_policy;
        let transient_for = params.transient_for;
        let taskbar_visible = matches!(params.kind, WindowKind::Normal | WindowKind::Floating);

        let hinstance = get_module_handle();
        let display = params
            .display_id
            .and_then(|display_id| display_topology.display(display_id))
            .unwrap_or_else(|| display_topology.primary_display());
        let appearance = system_appearance().unwrap_or_default();
        anyhow::ensure!(
            dwstyle.0 & WS_CHILD.0 == 0,
            "GPUI top-level transient windows must never use WS_CHILD"
        );
        let owner_hwnd = transient_owner_hwnd;
        let modal_owner = if params.kind == WindowKind::Dialog {
            owner_hwnd
        } else {
            None
        };
        let modal_parent_guard = ModalParentGuard::acquire(modal_owner);
        let mut context = WindowCreateContext {
            inner: None,
            handle,
            hide_title_bar,
            display,
            display_topology_generation: display_topology.generation(),
            is_movable: params.is_movable,
            min_size: params.window_min_size,
            executor,
            current_cursor,
            cursor_visible,
            drop_target_helper,
            validation_number,
            native_window_generation,
            recovered_directx_devices,
            main_receiver,
            platform_window_handle,
            raw_window_handles,
            native_retirement_coordinator,
            appearance,
            disable_direct_composition,
            directx_devices,
            invalidate_devices,
            owner_hwnd,
            modal_parent_disabled: modal_parent_guard.owns_disable(),
            creation_show: params.show,
            show_on_initial_presentation: params.show && provisional_session.is_none(),
            provisional_session,
            accepts_pointer_input: params.accepts_pointer_input,
            focus_on_appearing,
            activation_policy,
            transient_for,
            taskbar_visible,
            #[cfg(test)]
            lifecycle_test_probe,
        };
        let creation_result = unsafe {
            CreateWindowExW(
                dwexstyle,
                WINDOW_CLASS_NAME,
                &window_name,
                dwstyle,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                owner_hwnd,
                None,
                Some(hinstance.into()),
                Some(&context as *const _ as *const _),
            )
        };

        let hwnd = match creation_result {
            Ok(hwnd) => hwnd,
            Err(create_error) => {
                if let Some(Err(inner_error)) = context.inner.take() {
                    return Err(inner_error);
                }
                return Err(create_error.into());
            }
        };
        let hwnd_guard = CreatedNativeWindowGuard::new(hwnd);
        let this = context
            .inner
            .take()
            .context("native window creation did not initialize window state")??;
        let construction_guard = ConstructionRetirementGuard::new(this);
        // `WindowsWindowInner` now owns the modal-parent disable state. Keep
        // the parent disabled if managed native retirement must retry after a
        // fallible construction step.
        hwnd_guard.commit();
        modal_parent_guard.commit();

        if let Some(owner_hwnd) = owner_hwnd {
            let observed_owner = unsafe { GetWindow(hwnd, GW_OWNER) }
                .context("failed to read back transient window owner")?;
            anyhow::ensure!(
                observed_owner == owner_hwnd,
                "native transient owner did not match the requested GPUI owner"
            );
        }

        #[cfg(test)]
        construction_guard
            .inner()
            .lifecycle_test_probe
            .record_created_hwnd(hwnd);

        register_drag_drop(construction_guard.inner())?;
        construction_guard.inner().drag_drop_registered.set(true);
        #[cfg(test)]
        if construction_guard
            .inner()
            .lifecycle_test_probe
            .take_fail_after_drag_drop_registration()
        {
            anyhow::bail!("injected failure after native drag-drop registration");
        }
        set_non_rude_hwnd(hwnd, true)?;
        configure_dwm_dark_mode(hwnd, appearance);
        construction_guard
            .inner()
            .state
            .border_offset
            .update_restored(hwnd)?;
        let open_status = WindowOpenStatus::new(
            construction_guard.inner(),
            display,
            display_topology.generation(),
            params.window_bounds.get_bounds(),
            WindowOpenState::from(params.window_bounds),
        )?;
        construction_guard
            .inner()
            .state
            .initial_placement
            .set(Some(open_status));
        construction_guard
            .inner()
            .refresh_pending_initial_platform_facts();

        Ok(construction_guard.commit())
    }
}

impl rwh::HasWindowHandle for WindowsWindow {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let raw = rwh::Win32WindowHandle::new(unsafe {
            NonZeroIsize::new_unchecked(self.0.hwnd.0 as isize)
        })
        .into();
        Ok(unsafe { rwh::WindowHandle::borrow_raw(raw) })
    }
}

impl rwh::HasDisplayHandle for WindowsWindow {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::windows())
    }
}

impl Drop for WindowsWindow {
    fn drop(&mut self) {
        let _ = self.0.destroy_native_window();
        if self.0.is_native_window_terminal() {
            return;
        }
        if let Some(coordinator) = self.0.native_retirement_coordinator.upgrade() {
            log::error!(
                "WindowsWindow drop did not reach native terminal; transferring it to the platform retirement coordinator"
            );
            coordinator.enqueue_app_owned_native_window(self.0.clone());
        } else {
            log::error!(
                "WindowsWindow drop lost its platform retirement coordinator; retaining the managed native owner fail-closed"
            );
            let inner = self.0.clone();
            std::mem::forget(inner);
        }
    }
}

impl WindowsWindow {
    fn request_physical_placement_mutation(
        &mut self,
        generation: u64,
        request: WindowPhysicalPlacementRequest,
    ) -> PlatformWindowDispatch {
        if !self.0.placement_mutation_is_current(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        if self.0.has_pending_initial_placement() {
            if self.0.provisional_session.is_none() {
                self.0.state.deferred_placement_mutation.set(Some(
                    DeferredWindowPlacementMutation {
                        generation,
                        request: DeferredPlacementRequest::Physical(request),
                    },
                ));
                return PlatformWindowDispatch::Queued;
            }
            let this = self.0.clone();
            let executor = this.executor.clone();
            executor
                .spawn(async move {
                    if !this.placement_mutation_is_current(generation)
                        || this.is_native_window_terminal()
                    {
                        return;
                    }
                    let before_facts = match this.observed_platform_facts_from_native() {
                        Ok(facts) => facts,
                        Err(error) => {
                            log::warn!(
                                "Windows hidden physical placement rejected before dispatch because native facts could not be read: {error:#}"
                            );
                            this.emit_window_mutation_observation(
                                WindowMutationDomain::Placement,
                                generation,
                                PlatformWindowMutationTerminal::Rejected,
                                this.last_validated_platform_facts(),
                            );
                            return;
                        }
                    };
                    let result = this.apply_hidden_physical_initial_placement(request);
                    if this.placement_mutation_is_current(generation)
                        && !this.is_native_window_terminal()
                    {
                        let (terminal, facts) = this.terminal_facts_after_mutation(
                            "hidden physical initial placement",
                            result,
                            before_facts,
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            generation,
                            terminal,
                            facts,
                        );
                    }
                })
                .detach();
            return PlatformWindowDispatch::Queued;
        }
        if unsafe { !IsWindowVisible(self.0.hwnd).as_bool() } {
            return PlatformWindowDispatch::Rejected;
        }

        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.placement_mutation_is_current(generation)
                    || (unsafe { !IsWindow(Some(this.hwnd)).as_bool() })
                {
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows physical placement rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.last_validated_platform_facts(),
                        );
                        return;
                    }
                };
                let result = this.apply_physical_windowed_placement(request);
                if this.placement_mutation_is_current(generation)
                    && (unsafe { IsWindow(Some(this.hwnd)).as_bool() })
                {
                    let (terminal, facts) = this.terminal_facts_after_mutation(
                        "physical placement",
                        result,
                        before_facts,
                    );
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::Placement,
                        generation,
                        terminal,
                        facts,
                    );
                }
            })
            .detach();
        PlatformWindowDispatch::Queued
    }

    fn request_provisional_placement_mutation(
        &mut self,
        generation: u64,
        request: WindowProvisionalPlacementRequest,
    ) -> PlatformWindowDispatch {
        let Some(session) = self.0.provisional_session.clone() else {
            return PlatformWindowDispatch::Rejected;
        };
        if !self.0.placement_mutation_is_current(generation)
            || self.0.has_pending_initial_placement()
            || unsafe { !IsWindowVisible(self.0.hwnd).as_bool() }
            || !matches!(
                session.final_placement_request(self.handle.window_id(), request.generation()),
                Ok(current) if current == request
            )
        {
            let _ = session.settle_native_final_placement(
                self.handle.window_id(),
                request.generation(),
                WindowProvisionalPlacementOutcome::Rejected,
            );
            return PlatformWindowDispatch::Rejected;
        }

        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.placement_mutation_is_current(generation) {
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::Stale,
                    );
                    return;
                }
                if unsafe { !IsWindow(Some(this.hwnd)).as_bool() } {
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::WindowTerminal,
                    );
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows provisional placement rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        let _ = session.settle_native_final_placement(
                            this.handle.window_id(),
                            request.generation(),
                            WindowProvisionalPlacementOutcome::Rejected,
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.last_validated_platform_facts(),
                        );
                        return;
                    }
                };
                let native_result = this.apply_provisional_final_placement(&request);
                if !this.placement_mutation_is_current(generation) {
                    if let Ok(applied) = native_result {
                        this.compensate_applied_provisional_final_placement(
                            applied,
                            "a stale provisional final placement",
                        );
                    }
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::Stale,
                    );
                    return;
                }
                if unsafe { !IsWindow(Some(this.hwnd)).as_bool() } {
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::WindowTerminal,
                    );
                    return;
                }
                let applied = match native_result {
                    Ok(applied) => applied,
                    Err(error) => {
                        let _ = session.settle_native_final_placement(
                            this.handle.window_id(),
                            request.generation(),
                            WindowProvisionalPlacementOutcome::Rejected,
                        );
                        let (terminal, facts) = this.terminal_facts_after_mutation(
                            "provisional final placement",
                            Err(error),
                            before_facts,
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            generation,
                            terminal,
                            facts,
                        );
                        return;
                    }
                };

                let platform_facts = applied.platform_facts();

                if !this.placement_mutation_is_current(generation) {
                    this.compensate_applied_provisional_final_placement(
                        applied,
                        "a provisional final placement superseded during native fact readback",
                    );
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::Stale,
                    );
                    return;
                }
                if unsafe { !IsWindow(Some(this.hwnd)).as_bool() } {
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::WindowTerminal,
                    );
                    return;
                }

                let provisional_facts = applied.facts();
                let recorded = session
                    .record_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        provisional_facts,
                    )
                    .is_ok();
                let settled = recorded
                    && session
                        .settle_native_final_placement(
                            this.handle.window_id(),
                            request.generation(),
                            WindowProvisionalPlacementOutcome::Settled,
                        )
                        .is_ok();
                if !settled {
                    this.compensate_applied_provisional_final_placement(
                        applied,
                        "a provisional final placement rejected by its session authority",
                    );
                    let _ = session.settle_native_final_placement(
                        this.handle.window_id(),
                        request.generation(),
                        WindowProvisionalPlacementOutcome::Rejected,
                    );
                    let facts = this
                        .observed_platform_facts_from_native()
                        .unwrap_or(before_facts);
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::Placement,
                        generation,
                        PlatformWindowMutationTerminal::Rejected,
                        facts,
                    );
                    return;
                }

                applied.commit();
                this.emit_window_mutation_observation(
                    WindowMutationDomain::Placement,
                    generation,
                    PlatformWindowMutationTerminal::Observed,
                    platform_facts,
                );
            })
            .detach();
        PlatformWindowDispatch::Queued
    }

    fn request_pointer_input_mutation(
        &mut self,
        generation: u64,
        accepts_pointer_input: bool,
    ) -> PlatformWindowDispatch {
        let current = match self.0.native_accepts_pointer_input() {
            Ok(current) => current,
            Err(error) => {
                log::warn!(
                    "Windows pointer-input request rejected because native facts could not be read: {error:#}"
                );
                return PlatformWindowDispatch::Rejected;
            }
        };
        self.observe_accepts_pointer_input(current);
        if current == accepts_pointer_input {
            return PlatformWindowDispatch::Unchanged;
        }
        if !self.0.pointer_input_mutation_is_current(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.pointer_input_mutation_is_current(generation)
                    || (unsafe { !IsWindow(Some(this.hwnd)).as_bool() })
                {
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows pointer-input mutation rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::PointerInput,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.last_validated_platform_facts(),
                        );
                        return;
                    }
                };
                let result = this.set_accepts_pointer_input_now(accepts_pointer_input);
                if this.pointer_input_mutation_is_current(generation)
                    && (unsafe { IsWindow(Some(this.hwnd)).as_bool() })
                {
                    let (terminal, facts) = this.terminal_facts_after_mutation(
                        "pointer-input",
                        result,
                        before_facts,
                    );
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::PointerInput,
                        generation,
                        terminal,
                        facts,
                    );
                }
            })
            .detach();
        PlatformWindowDispatch::Queued
    }

    fn request_activation_policy_mutation(
        &mut self,
        generation: u64,
        activation_policy: WindowActivationPolicy,
    ) -> PlatformWindowDispatch {
        let current = match self.0.native_activation_policy() {
            Ok(current) => current,
            Err(error) => {
                log::warn!(
                    "Windows activation-policy request rejected because native facts could not be read: {error:#}"
                );
                return PlatformWindowDispatch::Rejected;
            }
        };
        if current == activation_policy {
            return PlatformWindowDispatch::Unchanged;
        }
        if !self.0.activation_policy_mutation_is_current(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.activation_policy_mutation_is_current(generation)
                    || (unsafe { !IsWindow(Some(this.hwnd)).as_bool() })
                {
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows activation-policy mutation rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::ActivationPolicy,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.last_validated_platform_facts(),
                        );
                        return;
                    }
                };
                let result = this.set_activation_policy_now(activation_policy);
                if this.activation_policy_mutation_is_current(generation)
                    && (unsafe { IsWindow(Some(this.hwnd)).as_bool() })
                {
                    let (terminal, facts) = this.terminal_facts_after_mutation(
                        "activation-policy",
                        result,
                        before_facts,
                    );
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::ActivationPolicy,
                        generation,
                        terminal,
                        facts,
                    );
                }
            })
            .detach();
        PlatformWindowDispatch::Queued
    }
}

impl PlatformWindow for WindowsWindow {
    fn map_window(&mut self) -> Result<()> {
        let was_hidden = unsafe { !IsWindowVisible(self.hwnd).as_bool() };
        #[cfg(test)]
        self.lifecycle_test_probe
            .record_hidden_before_map(was_hidden);
        anyhow::ensure!(
            was_hidden,
            "native window became visible before PlatformWindow::map_window"
        );

        self.0.prepare_pending_initial_placement()?;
        anyhow::ensure!(
            unsafe { !IsWindowVisible(self.hwnd).as_bool() },
            "native window became visible during precommit map"
        );
        Ok(())
    }

    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let command_window = Rc::downgrade(&self.0);
        let capture_window = command_window.clone();
        PlatformWindowCommandDispatcher::new_with_pointer_capture_release(
            move |command| {
                let Some(window) = command_window.upgrade() else {
                    return PlatformWindowCommandOutcome::WindowClosed;
                };
                if window.is_native_window_terminal() {
                    return PlatformWindowCommandOutcome::WindowClosed;
                }

                match command {
                    PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                        window.complete_initial_presentation(activate)
                    }
                    PlatformWindowCommand::RevealDeferredInitialPresentation {
                        session_generation,
                        presentation_generation,
                    } => window.reveal_deferred_initial_presentation(
                        session_generation,
                        presentation_generation,
                    ),
                    PlatformWindowCommand::Activate { .. } => window.activate_now(),
                    // Preserve the existing Windows behavior for currently unsupported commands.
                    PlatformWindowCommand::ShowWindowMenu(_)
                    | PlatformWindowCommand::StartWindowMove
                    | PlatformWindowCommand::StartWindowResize(_) => {
                        PlatformWindowCommandOutcome::Rejected
                    }
                }
            },
            move |release_generation| {
                let expected_pointer_session_epoch = capture_window.upgrade().map(|window| {
                    window
                        .state
                        .native_pointer_capture_release
                        .current_pointer_session_epoch()
                });
                let capture_window = capture_window.clone();
                PreparedPlatformPointerCaptureRelease::new(move || {
                    let Some(window) = capture_window.upgrade() else {
                        return PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal;
                    };
                    let Some(expected_pointer_session_epoch) = expected_pointer_session_epoch
                    else {
                        return PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal;
                    };
                    if window.is_native_window_terminal() {
                        return PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal;
                    }

                    window.release_native_pointer_capture_after_framework_cancel(
                        release_generation,
                        expected_pointer_session_epoch,
                    )
                })
            },
        )
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        let window = self.0.clone();
        let shutdown = window
            .claim_presentation_shutdown_ticket(shutdown)
            .expect("a platform-window shutdown ticket must match its native window");
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            window.quiesce_presentation(shutdown)
        })
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        if !self.0.destroy_native_window_with_ticket(shutdown) {
            return if self.0.is_native_window_terminal() {
                if shutdown.snapshot().quiesced() {
                    PlatformNativeWindowRetirementOutcome::NativeWindowTerminal
                } else {
                    PlatformNativeWindowRetirementOutcome::Rejected
                }
            } else {
                PlatformNativeWindowRetirementOutcome::Rejected
            };
        }
        if self.0.is_native_window_terminal() {
            PlatformNativeWindowRetirementOutcome::NativeWindowTerminal
        } else {
            PlatformNativeWindowRetirementOutcome::Accepted
        }
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.state.bounds()
    }

    fn physical_geometry(&self) -> Option<PlatformWindowPhysicalGeometry> {
        self.0.physical_geometry_from_native().ok()
    }

    fn native_pointer_physical_frame(&self) -> Option<PlatformNativePointerPhysicalFrame> {
        self.state.native_pointer_physical_frame.current.get()
    }

    fn is_maximized(&self) -> bool {
        self.state.is_maximized()
    }

    fn is_minimized(&self) -> bool {
        unsafe { IsIconic(self.0.hwnd).as_bool() }
    }

    fn accepts_pointer_input(&self) -> bool {
        self.state.accepts_pointer_input.get()
    }

    fn is_current_pointer_input_observation(
        &self,
        accepts_pointer_input: bool,
        generation: u64,
    ) -> bool {
        self.state.accepts_pointer_input.get() == accepts_pointer_input
            && self.0.pointer_input_observation_generation() == generation
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.0.creation_facts.clone()
    }

    fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd).as_bool() }
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        self.0.observed_platform_facts()
    }

    fn request_window_mutation(
        &mut self,
        generation: u64,
        request: WindowMutationRequest,
    ) -> PlatformWindowDispatch {
        if let WindowMutationRequest::PhysicalPlacement(request) = request {
            return self.request_physical_placement_mutation(generation, request);
        }
        let WindowMutationRequest::Placement(request) = request else {
            if let WindowMutationRequest::PointerInput(accepts_pointer_input) = request {
                return self.request_pointer_input_mutation(generation, accepts_pointer_input);
            }
            if let WindowMutationRequest::ActivationPolicy(activation_policy) = request {
                return self.request_activation_policy_mutation(generation, activation_policy);
            }
            return PlatformWindowDispatch::Unsupported;
        };
        if request.state == Some(WindowPlacementState::Minimized) {
            return PlatformWindowDispatch::Unsupported;
        }
        if !self.0.placement_mutation_is_current(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        if self.0.has_pending_initial_placement() {
            self.state
                .deferred_placement_mutation
                .set(Some(DeferredWindowPlacementMutation {
                    generation,
                    request: DeferredPlacementRequest::Logical(request),
                }));
            return PlatformWindowDispatch::Queued;
        }
        if unsafe { !IsWindowVisible(self.0.hwnd).as_bool() } {
            return PlatformWindowDispatch::Rejected;
        }
        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.placement_mutation_is_current(generation)
                    || (unsafe { !IsWindow(Some(this.hwnd)).as_bool() })
                {
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows live placement rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.last_validated_platform_facts(),
                        );
                        return;
                    }
                };
                let result = this.apply_window_placement_request(request, &before_facts);
                if this.placement_mutation_is_current(generation)
                    && (unsafe { IsWindow(Some(this.hwnd)).as_bool() })
                {
                    let (terminal, facts) = this.terminal_facts_after_mutation(
                        "live placement",
                        result,
                        before_facts,
                    );
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::Placement,
                        generation,
                        terminal,
                        facts,
                    );
                }
            })
            .detach();
        PlatformWindowDispatch::Queued
    }

    fn window_bounds(&self) -> WindowBounds {
        self.state.window_bounds()
    }

    /// get the logical size of the app's drawable area.
    ///
    /// Currently, GPUI uses the logical size of the app to handle mouse interactions (such as
    /// whether the mouse collides with other elements of GPUI).
    fn content_size(&self) -> Size<Pixels> {
        self.state.content_size()
    }

    fn scale_factor(&self) -> f32 {
        self.state.scale_factor.get()
    }

    fn appearance(&self) -> WindowAppearance {
        self.state.appearance.get()
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(self.state.display.get()))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        let scale_factor = self.scale_factor();
        let point = unsafe {
            let mut point: POINT = std::mem::zeroed();
            GetCursorPos(&mut point)
                .context("unable to get cursor position")
                .log_err();
            ScreenToClient(self.0.hwnd, &mut point).ok().log_err();
            point
        };
        logical_point(point.x as f32, point.y as f32, scale_factor)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let hcursor = load_cursor(style);
        if self.state.current_cursor.get().map(|cursor| cursor.0) == hcursor.map(|cursor| cursor.0)
        {
            return;
        }

        self.state.current_cursor.set(hcursor);
        if self.state.hovered.get() && self.state.cursor_visible.load(Ordering::Relaxed) {
            unsafe {
                SetCursor(hcursor);
            }
        }
    }

    fn request_provisional_placement(
        &mut self,
        generation: u64,
        request: WindowProvisionalPlacementRequest,
    ) -> PlatformWindowDispatch {
        self.request_provisional_placement_mutation(generation, request)
    }

    fn modifiers(&self) -> Modifiers {
        current_modifiers()
    }

    fn capslock(&self) -> Capslock {
        current_capslock()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.state.input_handler.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.state.input_handler.take()
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<Receiver<usize>> {
        let (done_tx, done_rx) = oneshot::channel();
        let msg = msg.to_string();
        let detail_string = detail.map(|detail| detail.to_string());
        let handle = self.0.hwnd;
        let answers = answers.to_vec();
        self.0
            .executor
            .spawn(async move {
                unsafe {
                    let mut config = TASKDIALOGCONFIG::default();
                    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as _;
                    config.hwndParent = handle;
                    let title;
                    let main_icon;
                    match level {
                        PromptLevel::Info => {
                            title = windows::core::w!("Info");
                            main_icon = TD_INFORMATION_ICON;
                        }
                        PromptLevel::Warning => {
                            title = windows::core::w!("Warning");
                            main_icon = TD_WARNING_ICON;
                        }
                        PromptLevel::Critical => {
                            title = windows::core::w!("Critical");
                            main_icon = TD_ERROR_ICON;
                        }
                    };
                    config.pszWindowTitle = title;
                    config.Anonymous1.pszMainIcon = main_icon;
                    let instruction = HSTRING::from(msg);
                    config.pszMainInstruction = PCWSTR::from_raw(instruction.as_ptr());
                    let hints_encoded;
                    if let Some(ref hints) = detail_string {
                        hints_encoded = HSTRING::from(hints);
                        config.pszContent = PCWSTR::from_raw(hints_encoded.as_ptr());
                    };
                    let mut button_id_map = Vec::with_capacity(answers.len());
                    let mut buttons = Vec::new();
                    let mut btn_encoded = Vec::new();
                    for (index, btn) in answers.iter().enumerate() {
                        let encoded = HSTRING::from(btn.label().as_ref());
                        let button_id = match btn {
                            PromptButton::Ok(_) => IDOK.0,
                            PromptButton::Cancel(_) => IDCANCEL.0,
                            // the first few low integer values are reserved for known buttons
                            // so for simplicity we just go backwards from -1
                            PromptButton::Other(_) => -(index as i32) - 1,
                        };
                        button_id_map.push(button_id);
                        buttons.push(TASKDIALOG_BUTTON {
                            nButtonID: button_id,
                            pszButtonText: PCWSTR::from_raw(encoded.as_ptr()),
                        });
                        btn_encoded.push(encoded);
                    }
                    config.cButtons = buttons.len() as _;
                    config.pButtons = buttons.as_ptr();

                    config.pfCallback = None;
                    let mut res = std::mem::zeroed();
                    let _ = TaskDialogIndirect(&config, Some(&mut res), None, None)
                        .context("unable to create task dialog")
                        .log_err();

                    if let Some(clicked) =
                        button_id_map.iter().position(|&button_id| button_id == res)
                    {
                        let _ = done_tx.send(clicked);
                    }
                }
            })
            .detach();

        Some(done_rx)
    }

    fn is_active(&self) -> bool {
        self.0.hwnd == unsafe { GetForegroundWindow() }
    }

    fn is_hovered(&self) -> bool {
        self.state.hovered.get()
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.state.background_appearance.get()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        true
    }

    fn set_title(&mut self, title: &str) {
        unsafe { SetWindowTextW(self.0.hwnd, &HSTRING::from(title)) }
            .inspect_err(|e| log::error!("Set title failed: {e}"))
            .ok();
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.state.background_appearance.set(background_appearance);
        let hwnd = self.0.hwnd;

        // using Dwm APIs for Mica and MicaAlt backdrops.
        // others follow the set_window_composition_attribute approach
        match background_appearance {
            WindowBackgroundAppearance::Opaque => {
                set_window_composition_attribute(hwnd, None, 0);
            }
            WindowBackgroundAppearance::Transparent => {
                set_window_composition_attribute(hwnd, None, 2);
            }
            WindowBackgroundAppearance::Blurred => {
                set_window_composition_attribute(hwnd, Some((0, 0, 0, 0)), 4);
            }
            WindowBackgroundAppearance::MicaBackdrop => {
                // DWMSBT_MAINWINDOW => MicaBase
                dwm_set_window_composition_attribute(hwnd, 2);
            }
            WindowBackgroundAppearance::MicaAltBackdrop => {
                // DWMSBT_TABBEDWINDOW => MicaAlt
                dwm_set_window_composition_attribute(hwnd, 4);
            }
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.state.is_fullscreen()
    }

    fn request_frame(&self, options: RequestFrameOptions) {
        if self.0.is_native_window_terminal() {
            return;
        }
        let _ = with_windows_callback(&self.state.callbacks.request_frame, |callback| {
            callback(options)
        });
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.state.callbacks.request_frame.set(Some(callback));
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        self.state.callbacks.set_input(callback);
    }

    fn on_modifiers_changed(&self, callback: Box<dyn FnMut(ModifiersChangedEvent)>) {
        self.state.callbacks.modifiers_changed.set(Some(callback));
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn FnMut(PlatformWindowActiveStatusObservation)>,
    ) {
        self.0
            .state
            .callbacks
            .active_status_change
            .set(Some(callback));
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0
            .state
            .callbacks
            .hovered_status_change
            .set(Some(callback));
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.state.callbacks.resize.set(Some(callback));
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.state.callbacks.moved.set(Some(callback));
    }

    fn on_window_state_change(&self, callback: Box<dyn FnMut()>) {
        self.state.callbacks.window_state_change.set(Some(callback));
    }

    fn on_window_mutation_observation(
        &self,
        callback: Box<dyn FnMut(PlatformWindowMutationObservation)>,
    ) {
        self.state
            .callbacks
            .window_mutation_observation
            .set(Some(callback));
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        self.0.prepare_window_mutation(domain, generation);
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        self.0.invalidate_window_mutation(domain);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.state.callbacks.should_close.set(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.state.callbacks.close.set(Some(callback));
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.0
            .state
            .callbacks
            .hit_test_window_control
            .set(Some(callback));
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0
            .state
            .callbacks
            .appearance_changed
            .set(Some(callback));
    }

    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome {
        match self
            .state
            .renderer
            .borrow_mut()
            .draw(scene, self.state.background_appearance.get())
        {
            Ok(outcome) => outcome,
            Err(error) => {
                log::error!("failed to submit DirectX frame: {error:#}");
                PlatformWindowPresentOutcome::Rejected
            }
        }
    }

    #[cfg(feature = "test-support")]
    fn render_to_image(&self, scene: &Scene) -> Result<image::RgbaImage> {
        self.state
            .renderer
            .borrow_mut()
            .render_to_image(scene, self.state.background_appearance.get())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.state.renderer.borrow().sprite_atlas()
    }

    fn get_raw_handle(&self) -> HWND {
        self.0.hwnd
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.state.renderer.borrow().gpu_specs().log_err()
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let scale_factor = self.state.scale_factor.get();
        let caret_position = POINT {
            x: (bounds.origin.x.as_f32() * scale_factor) as i32,
            y: (bounds.origin.y.as_f32() * scale_factor) as i32
                + ((bounds.size.height.as_f32() * scale_factor) as i32 / 2),
        };

        self.0.update_ime_position(self.0.hwnd, caret_position);
    }

    fn play_system_bell(&self) {
        // MB_OK: The sound specified as the Windows Default Beep sound.
        let _ = unsafe { MessageBeep(MB_OK) };
    }

    fn a11y_init(&self, callbacks: open_gpui::A11yCallbacks) {
        let action_handler = A11yActionHandler {
            callback: callbacks.action,
            provisional_session: self.0.provisional_session.clone(),
            window_id: self.0.handle.window_id(),
        };
        let is_focused = self.0.provisional_accepts_interaction()
            && unsafe { GetForegroundWindow() } == self.0.hwnd;

        let adapter = accesskit_windows::Adapter::new(
            accesskit_windows::HWND(self.0.hwnd.0),
            is_focused,
            action_handler,
        );

        let activation_handler = A11yActivationHandler {
            callback: callbacks.activation,
            provisional_session: self.0.provisional_session.clone(),
            window_id: self.0.handle.window_id(),
        };

        *self.state.a11y.borrow_mut() = Some(A11yState {
            adapter,
            activation_handler,
        });
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        if !self.0.provisional_accepts_interaction() {
            return;
        }
        let events = {
            let mut a11y = self.state.a11y.borrow_mut();
            a11y.as_mut()
                .and_then(|a11y| a11y.adapter.update_if_active(|| tree_update))
        };
        // The borrow must be dropped before raising events, because
        // `events.raise()` calls `UiaRaiseAutomationPropertyChangedEvent`
        // which may send a nested `WM_GETOBJECT` back into this window
        // procedure, re-entering `handle_wm_getobject` which also borrows
        // `self.state.a11y`.
        if let Some(events) = events {
            events.raise();
        }
    }

    fn a11y_update_window_bounds(&self) {
        // Windows UIA handles window bounds tracking automatically.
    }
}

pub(crate) struct A11yState {
    pub(crate) adapter: accesskit_windows::Adapter,
    pub(crate) activation_handler: A11yActivationHandler,
}

pub(crate) struct A11yActivationHandler {
    callback: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
    provisional_session: Option<WindowProvisionalSession>,
    window_id: WindowId,
}

impl accesskit::ActivationHandler for A11yActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        if self.provisional_session.as_ref().is_some_and(|session| {
            let snapshot = session.snapshot();
            snapshot.window_id() != Some(self.window_id) || !snapshot.accepts_interaction()
        }) {
            return None;
        }
        (self.callback)()
    }
}

struct A11yActionHandler {
    callback: Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>,
    provisional_session: Option<WindowProvisionalSession>,
    window_id: WindowId,
}

impl accesskit::ActionHandler for A11yActionHandler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        if self.provisional_session.as_ref().is_some_and(|session| {
            let snapshot = session.snapshot();
            snapshot.window_id() != Some(self.window_id) || !snapshot.accepts_interaction()
        }) {
            return;
        }
        (self.callback)(request);
    }
}

#[implement(IDropTarget)]
struct WindowsDragDropHandler(pub Rc<WindowsWindowInner>);

impl WindowsDragDropHandler {
    fn handle_drag_drop(&self, input: PlatformInput) {
        let _ = self.0.dispatch_input(input);
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for WindowsDragDropHandler_Impl {
    fn DragEnter(
        &self,
        pdataobj: windows::core::Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if !self.0.provisional_accepts_interaction() {
            unsafe {
                *pdweffect = DROPEFFECT_NONE;
            }
            return Ok(());
        }
        unsafe {
            let idata_obj = pdataobj.ok()?;
            let config = FORMATETC {
                cfFormat: CF_HDROP.0,
                ptd: std::ptr::null_mut() as _,
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as _,
            };
            let cursor_position = POINT { x: pt.x, y: pt.y };
            if idata_obj.QueryGetData(&config as _) == S_OK {
                *pdweffect = DROPEFFECT_COPY;
                let Some(mut idata) = idata_obj.GetData(&config as _).log_err() else {
                    return Ok(());
                };
                if idata.u.hGlobal.is_invalid() {
                    return Ok(());
                }
                let hdrop = HDROP(idata.u.hGlobal.0);
                let mut paths = SmallVec::<[PathBuf; 2]>::new();
                with_file_names(hdrop, |file_name| {
                    if let Some(path) = PathBuf::from_str(&file_name).log_err() {
                        paths.push(path);
                    }
                });
                ReleaseStgMedium(&mut idata);
                let mut cursor_position = cursor_position;
                ScreenToClient(self.0.hwnd, &mut cursor_position)
                    .ok()
                    .log_err();
                let scale_factor = self.0.state.scale_factor.get();
                let input = PlatformInput::FileDrop(FileDropEvent::Entered {
                    position: logical_point(
                        cursor_position.x as f32,
                        cursor_position.y as f32,
                        scale_factor,
                    ),
                    paths: ExternalPaths(paths),
                });
                self.handle_drag_drop(input);
            } else {
                *pdweffect = DROPEFFECT_NONE;
            }
            self.0
                .drop_target_helper
                .DragEnter(self.0.hwnd, idata_obj, &cursor_position, *pdweffect)
                .log_err();
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if !self.0.provisional_accepts_interaction() {
            unsafe {
                *pdweffect = DROPEFFECT_NONE;
            }
            return Ok(());
        }
        let mut cursor_position = POINT { x: pt.x, y: pt.y };
        unsafe {
            *pdweffect = DROPEFFECT_COPY;
            self.0
                .drop_target_helper
                .DragOver(&cursor_position, *pdweffect)
                .log_err();
            ScreenToClient(self.0.hwnd, &mut cursor_position)
                .ok()
                .log_err();
        }
        let scale_factor = self.0.state.scale_factor.get();
        let input = PlatformInput::FileDrop(FileDropEvent::Pending {
            position: logical_point(
                cursor_position.x as f32,
                cursor_position.y as f32,
                scale_factor,
            ),
        });
        self.handle_drag_drop(input);

        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        unsafe {
            self.0.drop_target_helper.DragLeave().log_err();
        }
        let input = PlatformInput::FileDrop(FileDropEvent::Exited);
        self.handle_drag_drop(input);

        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: windows::core::Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if !self.0.provisional_accepts_interaction() {
            unsafe {
                *pdweffect = DROPEFFECT_NONE;
            }
            return Ok(());
        }
        let idata_obj = pdataobj.ok()?;
        let mut cursor_position = POINT { x: pt.x, y: pt.y };
        unsafe {
            *pdweffect = DROPEFFECT_COPY;
            self.0
                .drop_target_helper
                .Drop(idata_obj, &cursor_position, *pdweffect)
                .log_err();
            ScreenToClient(self.0.hwnd, &mut cursor_position)
                .ok()
                .log_err();
        }
        let scale_factor = self.0.state.scale_factor.get();
        let input = PlatformInput::FileDrop(FileDropEvent::Submit {
            position: logical_point(
                cursor_position.x as f32,
                cursor_position.y as f32,
                scale_factor,
            ),
        });
        self.handle_drag_drop(input);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClickState {
    button: Cell<MouseButton>,
    last_click: Cell<Instant>,
    last_position: Cell<Point<DevicePixels>>,
    double_click_spatial_tolerance_width: Cell<i32>,
    double_click_spatial_tolerance_height: Cell<i32>,
    double_click_interval: Cell<Duration>,
    pub(crate) current_count: Cell<usize>,
}

impl ClickState {
    pub fn new() -> Self {
        let double_click_spatial_tolerance_width = unsafe { GetSystemMetrics(SM_CXDOUBLECLK) };
        let double_click_spatial_tolerance_height = unsafe { GetSystemMetrics(SM_CYDOUBLECLK) };
        let double_click_interval = Duration::from_millis(unsafe { GetDoubleClickTime() } as u64);

        ClickState {
            button: Cell::new(MouseButton::Left),
            last_click: Cell::new(Instant::now()),
            last_position: Cell::new(Point::default()),
            double_click_spatial_tolerance_width: Cell::new(double_click_spatial_tolerance_width),
            double_click_spatial_tolerance_height: Cell::new(double_click_spatial_tolerance_height),
            double_click_interval: Cell::new(double_click_interval),
            current_count: Cell::new(0),
        }
    }

    /// update self and return the needed click count
    pub fn update(&self, button: MouseButton, new_position: Point<DevicePixels>) -> usize {
        if self.button.get() == button && self.is_double_click(new_position) {
            self.current_count.update(|it| it + 1);
        } else {
            self.current_count.set(1);
        }
        self.last_click.set(Instant::now());
        self.last_position.set(new_position);
        self.button.set(button);

        self.current_count.get()
    }

    pub fn system_update(&self, wparam: usize) {
        match wparam {
            // SPI_SETDOUBLECLKWIDTH
            29 => self
                .double_click_spatial_tolerance_width
                .set(unsafe { GetSystemMetrics(SM_CXDOUBLECLK) }),
            // SPI_SETDOUBLECLKHEIGHT
            30 => self
                .double_click_spatial_tolerance_height
                .set(unsafe { GetSystemMetrics(SM_CYDOUBLECLK) }),
            // SPI_SETDOUBLECLICKTIME
            32 => self
                .double_click_interval
                .set(Duration::from_millis(unsafe { GetDoubleClickTime() } as u64)),
            _ => {}
        }
    }

    #[inline]
    fn is_double_click(&self, new_position: Point<DevicePixels>) -> bool {
        let diff = self.last_position.get() - new_position;

        self.last_click.get().elapsed() < self.double_click_interval.get()
            && diff.x.0.abs() <= self.double_click_spatial_tolerance_width.get()
            && diff.y.0.abs() <= self.double_click_spatial_tolerance_height.get()
    }
}

#[derive(Copy, Clone)]
struct StyleAndBounds {
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
}

#[derive(Copy, Clone)]
struct WindowPlacementRollbackSnapshot {
    placement: WINDOWPLACEMENT,
    style_and_bounds: StyleAndBounds,
    visible: bool,
    border_offset: WindowBorderOffsetSnapshot,
    fullscreen: Option<StyleAndBounds>,
    fullscreen_restore_bounds: Bounds<Pixels>,
    non_rude_hwnd: bool,
}

#[repr(C)]
struct WINDOWCOMPOSITIONATTRIBDATA {
    attrib: u32,
    pv_data: *mut std::ffi::c_void,
    cb_data: usize,
}

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

type Color = (u8, u8, u8, u8);

#[derive(Debug, Default, Clone)]
pub(crate) struct WindowBorderOffset {
    left: Cell<i32>,
    top: Cell<i32>,
    right: Cell<i32>,
    bottom: Cell<i32>,
}

impl WindowBorderOffset {
    fn snapshot(&self) -> WindowBorderOffsetSnapshot {
        WindowBorderOffsetSnapshot {
            left: self.left.get(),
            top: self.top.get(),
            right: self.right.get(),
            bottom: self.bottom.get(),
        }
    }

    fn restore(&self, snapshot: WindowBorderOffsetSnapshot) {
        self.left.set(snapshot.left);
        self.top.set(snapshot.top);
        self.right.set(snapshot.right);
        self.bottom.set(snapshot.bottom);
    }

    pub(crate) fn width(&self) -> i32 {
        self.left.get() + self.right.get()
    }

    pub(crate) fn height(&self) -> i32 {
        self.top.get() + self.bottom.get()
    }

    pub(crate) fn update_restored(&self, hwnd: HWND) -> anyhow::Result<()> {
        let style = WINDOW_STYLE(unsafe { get_window_long(hwnd, GWL_STYLE) } as u32);
        if unsafe { IsZoomed(hwnd).as_bool() }
            || WindowsWindowInner::has_fullscreen_window_style(style)
        {
            return Ok(());
        }
        let window_rect = unsafe {
            let mut rect = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect)?;
            rect
        };
        let client_rect = unsafe {
            let mut rect = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect)?;
            rect
        };
        let mut client_origin = POINT {
            x: client_rect.left,
            y: client_rect.top,
        };
        unsafe { ClientToScreen(hwnd, &mut client_origin) }
            .ok()
            .context("failed to read the native client origin")?;
        let client_width = client_rect
            .right
            .checked_sub(client_rect.left)
            .context("native client width overflowed")?;
        let client_height = client_rect
            .bottom
            .checked_sub(client_rect.top)
            .context("native client height overflowed")?;
        let client_right = client_origin
            .x
            .checked_add(client_width)
            .context("native client right edge overflowed")?;
        let client_bottom = client_origin
            .y
            .checked_add(client_height)
            .context("native client bottom edge overflowed")?;
        self.left.set(
            client_origin
                .x
                .checked_sub(window_rect.left)
                .context("native left frame inset overflowed")?,
        );
        self.top.set(
            client_origin
                .y
                .checked_sub(window_rect.top)
                .context("native top frame inset overflowed")?,
        );
        self.right.set(
            window_rect
                .right
                .checked_sub(client_right)
                .context("native right frame inset overflowed")?,
        );
        self.bottom.set(
            window_rect
                .bottom
                .checked_sub(client_bottom)
                .context("native bottom frame inset overflowed")?,
        );
        Ok(())
    }
}

#[derive(Copy, Clone)]
struct WindowBorderOffsetSnapshot {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[derive(Clone)]
struct WindowOpenStatus {
    placement: WINDOWPLACEMENT,
    state: WindowOpenState,
    client_placement: WindowPhysicalPlacementRequest,
}

impl WindowOpenStatus {
    fn new(
        window: &WindowsWindowInner,
        display: WindowsDisplay,
        display_topology_generation: u64,
        initial_bounds: Bounds<Pixels>,
        state: WindowOpenState,
    ) -> Result<Self> {
        let logical_client_bounds = if display.check_given_bounds(initial_bounds) {
            initial_bounds
        } else {
            display.default_bounds()
        };
        let client_bounds = logical_client_bounds.to_device_pixels(display.scale_factor());
        let target_display = display
            .physical_observation(display_topology_generation)
            .context("initial display facts are not physically coherent")?;
        let client_placement = WindowPhysicalPlacementRequest::try_new_for_display(
            client_bounds,
            client_bounds.center(),
            target_display,
        )
        .context("initial client placement is outside its selected display")?;
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(window.hwnd, &mut placement) }
            .context("failed to read the initial native window placement")?;
        placement.rcNormalPosition =
            window.initial_window_rect_for_physical_client_bounds(client_placement)?;
        Ok(Self {
            placement,
            state,
            client_placement,
        })
    }

    fn target_display(&self) -> PlatformPhysicalDisplayObservation {
        self.client_placement
            .target_display()
            .expect("retained initial placement is display-bound")
    }

    fn logical_client_bounds(&self) -> Bounds<Pixels> {
        let target_display = self.target_display();
        self.client_placement
            .client_bounds()
            .to_pixels(target_display.scale_factor())
    }
}

#[derive(Clone, Copy)]
struct DeferredWindowPlacementMutation {
    generation: u64,
    request: DeferredPlacementRequest,
}

#[derive(Clone, Copy)]
enum DeferredPlacementRequest {
    Logical(WindowPlacementRequest),
    Physical(WindowPhysicalPlacementRequest),
}

#[derive(Clone, Copy)]
enum WindowOpenState {
    Maximized,
    Fullscreen,
    Windowed,
}

impl From<WindowBounds> for WindowOpenState {
    fn from(window_bounds: WindowBounds) -> Self {
        match window_bounds {
            WindowBounds::Windowed(_) => Self::Windowed,
            WindowBounds::Maximized(_) => Self::Maximized,
            WindowBounds::Fullscreen(_) => Self::Fullscreen,
        }
    }
}

const WINDOW_CLASS_NAME: PCWSTR = w!("OpenGPUI::Window");

fn register_window_class(icon_handle: HICON) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_procedure),
            hIcon: icon_handle,
            lpszClassName: PCWSTR(WINDOW_CLASS_NAME.as_ptr()),
            style: CS_HREDRAW | CS_VREDRAW,
            hInstance: get_module_handle().into(),
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x00000000)) },
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };
    });
}

unsafe extern "system" fn window_procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    #[cfg(any(test, feature = "test-support"))]
    let native_test_observation = {
        let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Rc<WindowsWindowInner>;
        if ptr.is_null() {
            None
        } else {
            crate::native_test_observation::begin_window_message_observation(
                unsafe { &*ptr },
                hwnd,
                msg,
                lparam,
            )
        }
    };
    let dispatch = catch_unwind(AssertUnwindSafe(|| unsafe {
        window_procedure_inner(hwnd, msg, wparam, lparam)
    }));
    match dispatch {
        Ok(result) => {
            #[cfg(any(test, feature = "test-support"))]
            if let Some(observation) = native_test_observation {
                observation.complete(
                    crate::native_test_observation::NativeWindowTestMessageDisposition::Returned(
                        result.0,
                    ),
                );
            }
            result
        }
        Err(payload) => {
            log::error!("caught a panic at the Win32 window-procedure boundary for message {msg}");
            let ptr =
                unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Rc<WindowsWindowInner>;
            if !ptr.is_null() {
                if let Err(recovery_payload) = catch_unwind(AssertUnwindSafe(|| unsafe {
                    (&*ptr).settle_pointer_input_after_callback_panic(msg, wparam);
                })) {
                    log::error!(
                        "Win32 window-procedure panic recovery also panicked for message {msg}"
                    );
                    std::mem::forget(recovery_payload);
                }
                if matches!(msg, WM_NCCREATE | WM_NCDESTROY)
                    && let Err(finalizer_payload) = catch_unwind(AssertUnwindSafe(|| unsafe {
                        release_native_window_owner(hwnd, ptr);
                    }))
                {
                    log::error!(
                        "Win32 native-window owner finalization panicked for message {msg}"
                    );
                    std::mem::forget(finalizer_payload);
                }
            }
            #[cfg(any(test, feature = "test-support"))]
            if let Some(observation) = native_test_observation {
                observation.complete(
                    crate::native_test_observation::NativeWindowTestMessageDisposition::Panicked,
                );
            }
            // An arbitrary panic payload may itself panic from Drop. Leaking only this failed
            // payload keeps Rust unwinding from ever crossing the system ABI boundary.
            std::mem::forget(payload);
            LRESULT(0)
        }
    }
}

unsafe fn window_procedure_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let window_params = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let window_creation_context = window_params.lpCreateParams as *mut WindowCreateContext;
        let window_creation_context = unsafe { &mut *window_creation_context };
        return match WindowsWindowInner::new(window_creation_context, hwnd, window_params) {
            Ok(window_state) => {
                // The native HWND owns one strong reference until WM_NCDESTROY. If a teardown
                // attempt fails after GPUI drops its PlatformWindow, callbacks and exact-generation
                // registry lookup therefore remain valid for a later retry.
                let native_owner = Box::new(window_state.clone());
                unsafe {
                    set_window_long(hwnd, GWLP_USERDATA, Box::into_raw(native_owner) as isize)
                };
                window_creation_context.inner = Some(Ok(window_state));
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            Err(error) => {
                window_creation_context.inner = Some(Err(error));
                LRESULT(0)
            }
        };
    }

    let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Rc<WindowsWindowInner>;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let inner = unsafe { &*ptr };
    if msg == WM_NCDESTROY {
        inner.mark_native_window_destroyed();
    }
    let result = inner.handle_msg(hwnd, msg, wparam, lparam);

    if msg == WM_NCDESTROY {
        unsafe { release_native_window_owner(hwnd, ptr) };
    }

    result
}

unsafe fn release_native_window_owner(hwnd: HWND, ptr: *mut Rc<WindowsWindowInner>) {
    unsafe { set_window_long(hwnd, GWLP_USERDATA, 0) };
    unsafe { drop(Box::from_raw(ptr)) };
}

pub(crate) fn window_from_hwnd(hwnd: HWND) -> Option<Rc<WindowsWindowInner>> {
    if hwnd.is_invalid() {
        return None;
    }

    let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Rc<WindowsWindowInner>;
    if !ptr.is_null() {
        let inner = unsafe { &*ptr };
        Some(inner.clone())
    } else {
        None
    }
}

fn get_module_handle() -> HMODULE {
    unsafe {
        let mut h_module = std::mem::zeroed();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            windows::core::w!("ZedModule"),
            &mut h_module,
        )
        .expect("Unable to get module handle"); // this should never fail

        h_module
    }
}

fn register_drag_drop(window: &Rc<WindowsWindowInner>) -> Result<()> {
    let window_handle = window.hwnd;
    let handler = WindowsDragDropHandler(window.clone());
    // The lifetime of `IDropTarget` is handled by Windows, it won't release until
    // we call `RevokeDragDrop`.
    // So, it's safe to drop it here.
    let drag_drop_handler: IDropTarget = handler.into();
    unsafe {
        RegisterDragDrop(window_handle, &drag_drop_handler)
            .context("unable to register drag-drop event")?;
    }
    Ok(())
}

fn calculate_window_rect(bounds: Bounds<DevicePixels>, border_offset: &WindowBorderOffset) -> RECT {
    RECT {
        left: bounds.left().0 - border_offset.left.get(),
        top: bounds.top().0 - border_offset.top.get(),
        right: bounds.right().0 + border_offset.right.get(),
        bottom: bounds.bottom().0 + border_offset.bottom.get(),
    }
}

fn calculate_client_rect(
    rect: RECT,
    border_offset: &WindowBorderOffset,
    scale_factor: f32,
) -> Bounds<Pixels> {
    let left = rect.left + border_offset.left.get();
    let top = rect.top + border_offset.top.get();
    let right = rect.right - border_offset.right.get();
    let bottom = rect.bottom - border_offset.bottom.get();
    let physical_size = size(DevicePixels(right - left), DevicePixels(bottom - top));
    Bounds {
        origin: logical_point(left as f32, top as f32, scale_factor),
        size: physical_size.to_pixels(scale_factor),
    }
}

fn target_monitor_dpi_for_physical_placement(
    request: WindowPhysicalPlacementRequest,
    validated_target_monitor: Option<HMONITOR>,
) -> Result<u32> {
    if let Some(display) = request.target_display() {
        let monitor = validated_target_monitor
            .context("display-bound placement did not retain its validated target monitor")?;
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
            .context("failed to read the exact target display DPI")?;
        anyhow::ensure!(
            dpi_x != 0
                && dpi_x == dpi_y
                && dpi_x as f32 / USER_DEFAULT_SCREEN_DPI as f32 == display.scale_factor(),
            "target display DPI no longer matches the placement observation"
        );
        return Ok(dpi_x);
    }

    let client_bounds = request.client_bounds();
    let client_rect = RECT {
        left: client_bounds.origin.x.0,
        top: client_bounds.origin.y.0,
        right: client_bounds.right().0,
        bottom: client_bounds.bottom().0,
    };
    anyhow::ensure!(
        client_rect.left < client_rect.right && client_rect.top < client_rect.bottom,
        "physical client placement requires non-empty bounds"
    );
    let monitor = unsafe { MonitorFromRect(&client_rect, MONITOR_DEFAULTTONEAREST) };
    anyhow::ensure!(
        !monitor.is_invalid(),
        "physical client placement did not resolve a target monitor"
    );

    let mut dpi_x = 0;
    let mut dpi_y = 0;
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
        .context("failed to read the target monitor DPI")?;
    anyhow::ensure!(
        dpi_x != 0 && dpi_x == dpi_y,
        "target monitor returned an invalid or non-square DPI"
    );
    Ok(dpi_x)
}

fn physical_geometry_sample_is_stable(
    first_placement_epoch: u64,
    final_placement_epoch: u64,
    first_topology_generation: u64,
    final_topology_generation: u64,
    first_geometry: PlatformWindowPhysicalGeometry,
    second_geometry: PlatformWindowPhysicalGeometry,
) -> bool {
    first_placement_epoch == final_placement_epoch
        && first_topology_generation == final_topology_generation
        && first_geometry == second_geometry
}

fn adjusted_window_rect_for_dpi(
    client_bounds: Bounds<DevicePixels>,
    style: WINDOW_STYLE,
    extended_style: WINDOW_EX_STYLE,
    has_menu: bool,
    dpi: u32,
) -> Result<RECT> {
    anyhow::ensure!(dpi != 0, "physical client placement requires a target DPI");
    let mut rect = RECT {
        left: client_bounds.origin.x.0,
        top: client_bounds.origin.y.0,
        right: client_bounds.right().0,
        bottom: client_bounds.bottom().0,
    };
    anyhow::ensure!(
        rect.left < rect.right && rect.top < rect.bottom,
        "physical client placement requires non-empty bounds"
    );
    unsafe { AdjustWindowRectExForDpi(&mut rect, style, has_menu, extended_style, dpi) }
        .context("failed to calculate the native frame at the target monitor DPI")?;
    anyhow::ensure!(
        rect.left < rect.right && rect.top < rect.bottom,
        "target-DPI adjustment produced an empty native window frame"
    );
    Ok(rect)
}

fn window_rect_from_observed_frame(
    client_bounds: Bounds<DevicePixels>,
    observed_client: Bounds<DevicePixels>,
    observed_window: RECT,
) -> Result<RECT> {
    let left_offset = observed_client.origin.x.0 - observed_window.left;
    let top_offset = observed_client.origin.y.0 - observed_window.top;
    let right_offset = observed_window.right - observed_client.right().0;
    let bottom_offset = observed_window.bottom - observed_client.bottom().0;
    anyhow::ensure!(
        left_offset >= 0 && top_offset >= 0 && right_offset >= 0 && bottom_offset >= 0,
        "native window frame offsets were invalid"
    );
    let rect = RECT {
        left: client_bounds.origin.x.0 - left_offset,
        top: client_bounds.origin.y.0 - top_offset,
        right: client_bounds.right().0 + right_offset,
        bottom: client_bounds.bottom().0 + bottom_offset,
    };
    anyhow::ensure!(
        rect.left < rect.right && rect.top < rect.bottom,
        "native physical client placement produced an empty window frame"
    );
    Ok(rect)
}

fn dwm_set_window_composition_attribute(hwnd: HWND, backdrop_type: u32) {
    let mut version = unsafe { std::mem::zeroed() };
    let status = unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut version) };

    // DWMWA_SYSTEMBACKDROP_TYPE is available only on version 22621 or later
    // using SetWindowCompositionAttributeType as a fallback
    if !status.is_ok() || version.dwBuildNumber < 22621 {
        return;
    }

    unsafe {
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop_type as *const _ as *const _,
            std::mem::size_of_val(&backdrop_type) as u32,
        );

        if !result.is_ok() {
            return;
        }
    }
}

fn set_window_composition_attribute(hwnd: HWND, color: Option<Color>, state: u32) {
    let mut version = unsafe { std::mem::zeroed() };
    let status = unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut version) };

    if !status.is_ok() || version.dwBuildNumber < 17763 {
        return;
    }

    unsafe {
        type SetWindowCompositionAttributeType =
            unsafe extern "system" fn(HWND, *mut WINDOWCOMPOSITIONATTRIBDATA) -> BOOL;
        let module_name = PCSTR::from_raw(c"user32.dll".as_ptr() as *const u8);
        if let Some(user32) = GetModuleHandleA(module_name)
            .context("Unable to get user32.dll handle")
            .log_err()
        {
            let func_name = PCSTR::from_raw(c"SetWindowCompositionAttribute".as_ptr() as *const u8);
            let set_window_composition_attribute: SetWindowCompositionAttributeType =
                std::mem::transmute(GetProcAddress(user32, func_name));
            let mut color = color.unwrap_or_default();
            let is_acrylic = state == 4;
            if is_acrylic && color.3 == 0 {
                color.3 = 1;
            }
            let accent = AccentPolicy {
                accent_state: state,
                accent_flags: if is_acrylic { 0 } else { 2 },
                gradient_color: (color.0 as u32)
                    | ((color.1 as u32) << 8)
                    | ((color.2 as u32) << 16)
                    | ((color.3 as u32) << 24),
                animation_id: 0,
            };
            let mut data = WINDOWCOMPOSITIONATTRIBDATA {
                attrib: 0x13,
                pv_data: &accent as *const _ as *mut _,
                cb_data: std::mem::size_of::<AccentPolicy>(),
            };
            let _ = set_window_composition_attribute(hwnd, &mut data as *mut _ as _);
        }
    }
}

// When the platform title bar is hidden, Windows may think that our application is meant to appear 'fullscreen'
// and will stop the taskbar from appearing on top of our window. Prevent this.
// https://devblogs.microsoft.com/oldnewthing/20250522-00/?p=111211
fn non_rude_hwnd_for_fullscreen(fullscreen: Option<StyleAndBounds>) -> bool {
    fullscreen.is_none()
}

fn set_non_rude_hwnd(hwnd: HWND, non_rude: bool) -> Result<()> {
    if non_rude {
        unsafe { SetPropW(hwnd, w!("NonRudeHWND"), Some(HANDLE(1 as _))) }
            .context("failed to set NonRudeHWND")?;
    } else {
        unsafe { RemovePropW(hwnd, w!("NonRudeHWND")) }.context("failed to remove NonRudeHWND")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ClickState, NativeRect, ProvisionalPlacementCompensation,
        ProvisionalPlacementCompensationComponent, StyleAndBounds, WindowOpenState,
        WindowsNativePointerPhysicalFrameScope, WindowsNativePointerPhysicalFrameState,
        adjusted_window_rect_for_dpi, non_rude_hwnd_for_fullscreen,
        physical_geometry_sample_is_stable, window_rect_from_observed_frame, with_windows_callback,
    };
    use open_gpui::{
        Bounds, DevicePixels, MouseButton, PlatformNativePointerPhysicalFrame,
        PlatformWindowPhysicalGeometry, WindowBounds, point, size,
    };
    use std::time::Duration;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    use windows::Win32::{
        Foundation::RECT,
        UI::WindowsAndMessaging::{WINDOW_EX_STYLE, WINDOW_STYLE, WS_OVERLAPPEDWINDOW},
    };

    #[test]
    fn target_dpi_changes_the_prepared_native_frame() {
        let client_bounds = Bounds::new(
            point(DevicePixels(2_000), DevicePixels(200)),
            size(DevicePixels(800), DevicePixels(600)),
        );
        let frame_at_96 = adjusted_window_rect_for_dpi(
            client_bounds,
            WS_OVERLAPPEDWINDOW,
            WINDOW_EX_STYLE(0),
            false,
            96,
        )
        .unwrap();
        let frame_at_144 = adjusted_window_rect_for_dpi(
            client_bounds,
            WS_OVERLAPPEDWINDOW,
            WINDOW_EX_STYLE(0),
            false,
            144,
        )
        .unwrap();

        assert!(frame_at_144.left < frame_at_96.left);
        assert!(frame_at_144.top < frame_at_96.top);
        assert!(frame_at_144.right > frame_at_96.right);
        assert!(frame_at_144.bottom > frame_at_96.bottom);
    }

    #[test]
    fn stable_physical_geometry_sample_rejects_native_placement_aba() {
        let geometry = PlatformWindowPhysicalGeometry::try_new(
            Bounds::new(
                point(DevicePixels(-1_600), DevicePixels(120)),
                size(DevicePixels(800), DevicePixels(600)),
            ),
            1.5,
        )
        .unwrap();

        assert!(physical_geometry_sample_is_stable(
            7, 7, 11, 11, geometry, geometry,
        ));
        assert!(!physical_geometry_sample_is_stable(
            7, 9, 11, 11, geometry, geometry,
        ));
    }

    #[test]
    fn provisional_compensation_requires_physical_geometry_restoration() {
        let restored = ProvisionalPlacementCompensationComponent::Restored;
        assert!(
            ProvisionalPlacementCompensation {
                rect: restored,
                z_order: restored,
                physical_geometry: restored,
            }
            .fully_restored()
        );
        assert!(
            !ProvisionalPlacementCompensation {
                rect: restored,
                z_order: restored,
                physical_geometry: ProvisionalPlacementCompensationComponent::AuthorityChanged,
            }
            .fully_restored()
        );
    }

    #[test]
    fn target_side_frame_offsets_produce_an_exact_client_rect_correction() {
        let observed_client = Bounds::new(
            point(DevicePixels(120), DevicePixels(170)),
            size(DevicePixels(800), DevicePixels(600)),
        );
        let observed_window = RECT {
            left: 108,
            top: 125,
            right: 932,
            bottom: 782,
        };
        let requested_client = Bounds::new(
            point(DevicePixels(1_800), DevicePixels(-300)),
            size(DevicePixels(640), DevicePixels(480)),
        );

        assert_eq!(
            window_rect_from_observed_frame(requested_client, observed_client, observed_window)
                .unwrap(),
            RECT {
                left: 1_788,
                top: -345,
                right: 2_452,
                bottom: 192,
            }
        );
    }

    #[test]
    fn native_rect_subtraction_distinguishes_partial_and_full_occlusion() {
        let viewport = NativeRect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 80,
        };
        let mut partial = Vec::new();
        viewport.subtract(
            NativeRect {
                left: 25,
                top: 20,
                right: 75,
                bottom: 60,
            },
            &mut partial,
        );
        assert_eq!(partial.len(), 4);
        assert!(partial.iter().all(|fragment| {
            fragment
                .intersection(NativeRect {
                    left: 25,
                    top: 20,
                    right: 75,
                    bottom: 60,
                })
                .is_none()
        }));

        let mut fully_obscured = Vec::new();
        viewport.subtract(viewport, &mut fully_obscured);
        assert!(fully_obscured.is_empty());
    }

    #[test]
    fn canonical_window_bounds_select_open_state() {
        assert!(matches!(
            WindowOpenState::from(WindowBounds::Windowed(Default::default())),
            WindowOpenState::Windowed
        ));
        assert!(matches!(
            WindowOpenState::from(WindowBounds::Maximized(Default::default())),
            WindowOpenState::Maximized
        ));
        assert!(matches!(
            WindowOpenState::from(WindowBounds::Fullscreen(Default::default())),
            WindowOpenState::Fullscreen
        ));
    }

    #[test]
    fn test_double_click_interval() {
        let state = ClickState::new();
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Right, point(DevicePixels(0), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            2
        );
        state
            .last_click
            .update(|it| it - Duration::from_millis(700));
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            1
        );
    }

    #[test]
    fn test_double_click_spatial_tolerance() {
        let state = ClickState::new();
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(-3), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(3))),
            2
        );
        assert_eq!(
            state.update(MouseButton::Right, point(DevicePixels(3), DevicePixels(2))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Right, point(DevicePixels(10), DevicePixels(0))),
            1
        );
    }

    #[test]
    fn non_rude_hwnd_is_the_inverse_of_fullscreen_state() {
        assert!(non_rude_hwnd_for_fullscreen(None));
        assert!(!non_rude_hwnd_for_fullscreen(Some(StyleAndBounds {
            style: WINDOW_STYLE(0),
            x: 0,
            y: 0,
            cx: 0,
            cy: 0,
        })));
    }

    #[test]
    fn native_pointer_physical_frame_scope_is_nested_and_callback_local() {
        let frame = |offset| {
            PlatformNativePointerPhysicalFrame::new(
                point(DevicePixels(offset + 1), DevicePixels(offset + 2)),
                PlatformWindowPhysicalGeometry::try_new(
                    Bounds::new(
                        point(DevicePixels(offset), DevicePixels(offset)),
                        size(DevicePixels(800), DevicePixels(600)),
                    ),
                    1.5,
                )
                .unwrap(),
            )
        };
        let outer_frame = frame(10);
        let inner_frame = frame(20);
        let state = WindowsNativePointerPhysicalFrameState::default();

        {
            let outer = WindowsNativePointerPhysicalFrameScope::enter(&state, Some(outer_frame));
            assert_eq!(outer.frame(), Some(outer_frame));
            assert_eq!(state.current.get(), Some(outer_frame));

            {
                let inner =
                    WindowsNativePointerPhysicalFrameScope::enter(&state, Some(inner_frame));
                assert_eq!(inner.frame(), Some(inner_frame));
                assert_eq!(state.current.get(), Some(inner_frame));
            }

            assert_eq!(state.current.get(), Some(outer_frame));
        }

        assert_eq!(state.current.get(), None);
    }

    #[test]
    fn unavailable_native_pointer_frame_masks_an_outer_frame_until_scope_exit() {
        let outer_frame = PlatformNativePointerPhysicalFrame::new(
            point(DevicePixels(11), DevicePixels(12)),
            PlatformWindowPhysicalGeometry::try_new(
                Bounds::new(
                    point(DevicePixels(10), DevicePixels(10)),
                    size(DevicePixels(800), DevicePixels(600)),
                ),
                1.5,
            )
            .unwrap(),
        );
        let state = WindowsNativePointerPhysicalFrameState {
            current: Cell::new(Some(outer_frame)),
            invalidation_epoch: Cell::new(0),
        };

        {
            let unavailable = WindowsNativePointerPhysicalFrameScope::enter(&state, None);
            assert_eq!(unavailable.frame(), None);
            assert_eq!(state.current.get(), None);
        }

        assert_eq!(state.current.get(), Some(outer_frame));
    }

    #[test]
    fn capture_loss_invalidates_the_entire_active_physical_frame_chain() {
        let outer_frame = PlatformNativePointerPhysicalFrame::new(
            point(DevicePixels(31), DevicePixels(32)),
            PlatformWindowPhysicalGeometry::try_new(
                Bounds::new(
                    point(DevicePixels(30), DevicePixels(30)),
                    size(DevicePixels(800), DevicePixels(600)),
                ),
                1.5,
            )
            .unwrap(),
        );
        let state = WindowsNativePointerPhysicalFrameState::default();

        {
            let outer = WindowsNativePointerPhysicalFrameScope::enter(&state, Some(outer_frame));
            assert_eq!(outer.frame(), Some(outer_frame));
            state.invalidate_active_scopes();
            assert_eq!(state.current.get(), None);
        }

        assert_eq!(state.current.get(), None);
    }

    #[test]
    fn ordinary_callback_checkout_restores_the_callback_after_panic() {
        let call_count = Rc::new(Cell::new(0usize));
        let callback = Cell::new(Some(Box::new({
            let call_count = call_count.clone();
            move || {
                let next = call_count.get() + 1;
                call_count.set(next);
                if next == 1 {
                    panic!("injected ordinary callback panic");
                }
            }
        }) as Box<dyn FnMut()>));

        let first = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_windows_callback(&callback, |callback| callback());
        }));
        assert!(first.is_err());

        let _ = with_windows_callback(&callback, |callback| callback());
        assert_eq!(call_count.get(), 2);
    }

    #[test]
    fn ordinary_callback_checkout_preserves_a_reentrant_replacement_during_unwind() {
        let callback = Cell::new(Some(1u8));

        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_windows_callback(&callback, |_| {
                callback.set(Some(2));
                panic!("injected callback panic after replacement");
            });
        }));

        assert!(panic.is_err());
        assert_eq!(callback.take(), Some(2));
    }
}
