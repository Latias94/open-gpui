//! Host-driven logical windows with no native window, display, device or event loop ownership.
//!
//! Hosts drive callbacks after releasing all App and Window borrows. Wake notifications may run
//! while App is borrowed and must only schedule host work. Native accessibility, desktop window
//! operations and native handles are unsupported; the host owns any accessibility bridge.

mod binding;
mod visibility;

#[cfg(test)]
mod tests;

use crate::{
    AnyWindowHandle, Bounds, Capslock, CursorStyle, DevicePixels, DispatchEventResult,
    FileDropEvent, FrameRequestSource, Modifiers, Pixels, PlatformAtlas, PlatformFrameSignal,
    PlatformFrameSubmission, PlatformInput, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformWindow, PlatformWindowActiveStatusObservation,
    PlatformWindowCapabilities, PlatformWindowCreationCapabilities,
    PlatformWindowMutationCapabilities, PlatformWindowPresentOutcome, Point,
    PreparedPlatformPresentationShutdown, RequestFrameOptions, Scene, Size, TextInputConfiguration,
    TextInputStateChange, WindowActivationPolicy, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowCoordinateSpace, WindowCreationFacts, WindowCreationSupport, WindowId,
    WindowInitialPresentationOrder, WindowKind, WindowMutationSupport, WindowOptions, WindowParams,
    WindowPresentationShutdownTicket, WindowVisibility, px, size,
};
use anyhow::{Context as _, Result, ensure};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use binding::HostedSurfaceWindow;
use visibility::VisibilityCallback;

/// Initial logical geometry and host-supplied presentation facts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostedSurfaceOptions {
    /// Content size in logical pixels. Either dimension may be zero to suspend rendering.
    pub size: Size<Pixels>,
    /// Positive, finite physical pixels per logical pixel.
    pub scale_factor: f32,
    /// Appearance selected by the host.
    pub appearance: WindowAppearance,
    /// Whether the host currently directs keyboard interaction to this surface.
    pub active: bool,
    /// Whether the host currently permits presenting this surface.
    pub visible: bool,
}

impl Default for HostedSurfaceOptions {
    fn default() -> Self {
        Self {
            size: size(px(800.0), px(600.0)),
            scale_factor: 1.0,
            appearance: WindowAppearance::default(),
            active: true,
            visible: true,
        }
    }
}

/// An immutable, validated logical geometry snapshot for one renderer delivery.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostedSurfaceGeometry {
    size: Size<Pixels>,
    scale_factor: f32,
    physical_size: Size<DevicePixels>,
}

impl HostedSurfaceGeometry {
    /// Converts an actual host attachment extent without adding a pixel through float rounding.
    ///
    /// The returned logical dimensions round upward to exactly the supplied physical extent.
    /// Rejects negative dimensions, invalid scales and extents not representable at this scale.
    pub fn from_physical_size(
        physical_size: Size<DevicePixels>,
        scale_factor: f32,
    ) -> Result<Self> {
        ensure!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "hosted surface scale must be finite and positive"
        );
        let dimension = |physical: DevicePixels| -> Result<Pixels> {
            ensure!(
                physical.0 >= 0,
                "hosted surface physical dimensions must be nonnegative"
            );
            let expected = f64::from(physical.0);
            let scale = f64::from(scale_factor);
            let mut logical = (expected / scale) as f32;
            ensure!(
                logical.is_finite(),
                "hosted surface logical dimension exceeds Pixels"
            );
            if (f64::from(logical) * scale).ceil() > expected {
                logical = logical.next_down();
            }
            ensure!(
                logical >= 0.0 && (f64::from(logical) * scale).ceil() == expected,
                "hosted surface physical extent is not representable at this scale"
            );
            Ok(px(logical))
        };
        Self::new(
            size(
                dimension(physical_size.width)?,
                dimension(physical_size.height)?,
            ),
            scale_factor,
        )
    }

    /// Validates finite nonnegative dimensions and a positive finite scale.
    ///
    /// Physical dimensions round upward in double precision and must fit `DevicePixels`.
    /// This preserves fractional coverage without truncation, saturation or integer overflow.
    pub fn new(size: Size<Pixels>, scale_factor: f32) -> Result<Self> {
        ensure!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "hosted surface scale must be finite and positive"
        );
        let physical_dimension = |logical: Pixels| -> Result<DevicePixels> {
            let logical = f64::from(logical);
            ensure!(
                logical.is_finite() && logical >= 0.0,
                "hosted surface dimensions must be finite and nonnegative"
            );
            let physical = (logical * f64::from(scale_factor)).ceil();
            ensure!(
                physical.is_finite() && physical <= f64::from(i32::MAX),
                "hosted surface physical dimension exceeds DevicePixels"
            );
            Ok(DevicePixels(physical as i32))
        };
        let physical_size = crate::size(
            physical_dimension(size.width)?,
            physical_dimension(size.height)?,
        );
        Ok(Self {
            size,
            scale_factor,
            physical_size,
        })
    }

    /// Returns the content size in logical pixels.
    pub fn size(self) -> Size<Pixels> {
        self.size
    }

    /// Returns physical pixels per logical pixel.
    pub fn scale_factor(self) -> f32 {
        self.scale_factor
    }

    /// Returns the checked, upward-rounded physical extent, including zero for suspension.
    pub fn physical_size(self) -> Size<DevicePixels> {
        self.physical_size
    }

    fn is_suspended(self) -> bool {
        self.physical_size.width.0 == 0 || self.physical_size.height.0 == 0
    }
}

/// The host's atlas, exact scene delivery and presentation shutdown boundary.
///
/// This interface owns no device, queue, scheduler or native surface. A deferred implementation
/// retains its original recording, required resources and submission token, checks
/// `submission.is_pending()` before recording and actual submission, and completes that token
/// only after actual submission. A Scene clone alone does not retain renderer resources.
/// Failed recording requires discarding its output and reporting rejection or repaint required.
pub trait HostedSurfaceRenderer {
    /// Returns the renderer-local atlas used by the original Window.
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;

    /// Delivers a candidate with its immutable geometry and original submission authority.
    ///
    /// `Submitted` requires actual submission before returning. `Deferred` leaves the original
    /// token pending; recording, empty output and discarded encoders cannot establish submission.
    /// This call may run under the App borrow and must not drive host callbacks or reenter App.
    fn draw(
        &self,
        scene: &Scene,
        geometry: HostedSurfaceGeometry,
        submission: &PlatformFrameSubmission,
    ) -> PlatformWindowPresentOutcome;

    /// Snapshots shutdown against the supplied exact ticket without running GPU effects.
    ///
    /// The prepared operation runs outside the App borrow. It must quiesce every retained GPU
    /// use of this surface, including previously submitted work, before acknowledging this ticket.
    /// Refusal retains the original binding for a retry through the existing retirement owner.
    fn prepare_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown;
}

/// Coalesced text-input facts for the host's existing IME connection.
///
/// The host reads this snapshot after a wake, then queries or edits the original leased handler
/// through `HostedSurface::with_input_handler`. Revisions preserve content and selection changes
/// even when several notifications coalesce. No second text session or event queue is created.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostedSurfaceTextInputState {
    /// Revision of any configuration, caret, focus, text or keyboard request change.
    pub revision: u64,
    /// Configuration of the original Window's accepted text handler.
    pub configuration: TextInputConfiguration,
    /// Latest caret or selection bounds in surface-local logical pixels.
    pub caret_bounds: Option<Bounds<Pixels>>,
    /// Whether the original Window reports a focused text-input session.
    pub focused: bool,
    /// Latest host-facing soft-keyboard visibility request.
    pub soft_keyboard_visible: bool,
    /// Last revision at which document content changed.
    pub content_revision: u64,
    /// Last revision at which the caret or selection changed.
    pub selection_revision: u64,
}

/// Cloneable host driver for one logical Window binding.
///
/// A surface binds once to a full WindowId and cannot be reopened, including after retirement.
/// Dropping any driver clone leaves the binding alive. The original Window owns retirement.
/// Call driving, input, geometry, activity, visibility and close methods outside all App and
/// Window borrows. The supplied wake function may run inside those borrows: it must enqueue a
/// host turn and must not synchronously drive the surface or dispatch input.
#[derive(Clone)]
pub struct HostedSurface {
    state: Rc<HostedSurfaceState>,
}

struct HostedSurfaceState {
    geometry: Cell<HostedSurfaceGeometry>,
    appearance: WindowAppearance,
    active: Cell<bool>,
    host_visible: Cell<bool>,
    initially_presented: Cell<bool>,
    bound_id: Cell<Option<WindowId>>,
    closing: Cell<bool>,
    terminal: Cell<bool>,
    interaction_quiesced: Cell<bool>,
    renderer_used: Cell<bool>,
    renderer: Rc<dyn HostedSurfaceRenderer>,
    wake: Box<dyn Fn()>,
    pending_frame: RefCell<Option<RequestFrameOptions>>,
    pending_submission: RefCell<Option<PlatformFrameSubmission>>,
    shutdown: RefCell<Option<PreparedPlatformPresentationShutdown>>,
    creation_facts: RefCell<Option<WindowCreationFacts>>,
    request_frame_callback: RefCell<Option<Rc<dyn Fn(RequestFrameOptions)>>>,
    active_callback: RefCell<Option<Rc<dyn Fn(PlatformWindowActiveStatusObservation)>>>,
    visibility_callback: VisibilityCallback,
    hover_callback: RefCell<Option<Rc<dyn Fn(bool)>>>,
    resize_callback: RefCell<Option<Rc<dyn Fn(Size<Pixels>, f32)>>>,
    close_required_callback: RefCell<Option<Box<dyn FnOnce()>>>,
    close_callback: RefCell<Option<Box<dyn FnOnce()>>>,
    input_callback: PlatformInputCallbackSlot,
    input_installed: Cell<bool>,
    input_handler: PlatformInputHandlerSlot,
    mouse_position: Cell<Point<Pixels>>,
    modifiers: Cell<Modifiers>,
    capslock: Cell<Capslock>,
    hovered: Cell<bool>,
    cursor_style: Cell<CursorStyle>,
    text_input: RefCell<HostedSurfaceTextInputState>,
    background: Cell<WindowBackgroundAppearance>,
}

impl HostedSurface {
    /// Creates an unbound surface, validating geometry before retaining the renderer.
    pub fn new(
        options: HostedSurfaceOptions,
        renderer: Rc<dyn HostedSurfaceRenderer>,
        wake: impl Fn() + 'static,
    ) -> Result<Self> {
        let geometry = HostedSurfaceGeometry::new(options.size, options.scale_factor)?;
        Ok(Self {
            state: Rc::new(HostedSurfaceState {
                geometry: Cell::new(geometry),
                appearance: options.appearance,
                active: Cell::new(options.active),
                host_visible: Cell::new(options.visible),
                initially_presented: Cell::new(false),
                bound_id: Cell::new(None),
                closing: Cell::new(false),
                terminal: Cell::new(false),
                interaction_quiesced: Cell::new(false),
                renderer_used: Cell::new(false),
                renderer,
                wake: Box::new(wake),
                pending_frame: RefCell::new(None),
                pending_submission: RefCell::new(None),
                shutdown: RefCell::new(None),
                creation_facts: RefCell::new(None),
                request_frame_callback: RefCell::new(None),
                active_callback: RefCell::new(None),
                visibility_callback: VisibilityCallback::default(),
                hover_callback: RefCell::new(None),
                resize_callback: RefCell::new(None),
                close_required_callback: RefCell::new(None),
                close_callback: RefCell::new(None),
                input_callback: PlatformInputCallbackSlot::default(),
                input_installed: Cell::new(false),
                input_handler: PlatformInputHandlerSlot::default(),
                mouse_position: Cell::new(Point::default()),
                modifiers: Cell::new(Modifiers::default()),
                capslock: Cell::new(Capslock::default()),
                hovered: Cell::new(false),
                cursor_style: Cell::new(CursorStyle::default()),
                text_input: RefCell::new(HostedSurfaceTextInputState::default()),
                background: Cell::new(WindowBackgroundAppearance::default()),
            }),
        })
    }

    /// Returns the latest validated host geometry.
    pub fn geometry(&self) -> HostedSurfaceGeometry {
        self.state.geometry.get()
    }

    /// Merges frame demand and notifies the host without drawing or borrowing App.
    pub fn request_frame(&self, options: RequestFrameOptions) -> Result<()> {
        self.state.ensure_open()?;
        self.state.request_frame(options);
        Ok(())
    }

    /// Returns whether one explicit host turn has pending frame demand.
    pub fn needs_frame(&self) -> bool {
        !self.is_closed() && self.state.pending_frame.borrow().is_some()
    }

    /// Drains one merged request through the original Window callback outside the App borrow.
    ///
    /// Returns false when idle. Demand raised during the callback remains for a later host turn;
    /// this method does not loop or retry a parked deferred submission.
    pub fn drive_frame(&self) -> Result<bool> {
        self.state.ensure_bound()?;
        let callback = self
            .state
            .request_frame_callback
            .borrow()
            .clone()
            .context("hosted surface has no frame callback")?;
        let options = self.state.pending_frame.take();
        let Some(options) = options else {
            return Ok(false);
        };
        callback(options);
        Ok(true)
    }

    /// Dispatches input through the original must-immediate slot with its exact propagation result.
    ///
    /// Call outside the App borrow. Pointer cancellation is ordinary original-executor input.
    pub fn dispatch_input(&self, input: PlatformInput) -> Result<DispatchEventResult> {
        self.state.ensure_bound()?;
        ensure!(
            !self.state.interaction_quiesced.get(),
            "hosted surface input is quiesced"
        );
        ensure!(
            self.state.input_installed.get(),
            "hosted surface has no input callback"
        );
        self.state.update_input_snapshot(&input);
        self.state.ensure_bound()?;
        let result = self.state.input_callback.dispatch(input);
        Ok(result)
    }

    /// Queries or edits the original text handler under its existing native callback lease.
    ///
    /// Call outside the App borrow. A live surface without a focused handler returns None.
    pub fn with_input_handler<R>(
        &self,
        callback: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Result<Option<R>> {
        self.state.ensure_bound()?;
        ensure!(
            !self.state.interaction_quiesced.get(),
            "hosted surface input is quiesced"
        );
        Ok(self.state.input_handler.with_handler(callback))
    }

    /// Invalidates the original pending attempt before updating geometry, even for equal values.
    pub fn set_geometry(&self, size: Size<Pixels>, scale_factor: f32) -> Result<()> {
        self.state.ensure_open()?;
        let geometry = HostedSurfaceGeometry::new(size, scale_factor)?;
        self.state.invalidate_submission();
        self.state.geometry.set(geometry);
        let callback = self.state.resize_callback.borrow().clone();
        if let Some(callback) = callback {
            callback(size, scale_factor);
        }
        self.state.request_repaint();
        Ok(())
    }

    /// Reports a host activity edge without claiming exact native activation.
    pub fn set_active(&self, active: bool) -> Result<()> {
        self.state.ensure_open()?;
        if self.state.active.get() == active {
            return Ok(());
        }
        self.state.invalidate_submission();
        self.state.active.set(active);
        let callback = self.state.active_callback.borrow().clone();
        if let Some(callback) = callback {
            callback(PlatformWindowActiveStatusObservation::new(active, false));
        }
        self.state.request_repaint();
        Ok(())
    }

    /// Changes host presentation availability without bypassing the original first-reveal gate.
    pub fn set_visible(&self, visible: bool) -> Result<()> {
        self.state.ensure_open()?;
        self.state.invalidate_submission();
        let previous = self.state.visibility();
        self.state.host_visible.set(visible);
        let current = self.state.visibility();
        if current != previous {
            self.state.visibility_callback.call(current);
        }
        self.state.request_repaint();
        Ok(())
    }

    /// Forces close through the original Window retirement authority without consulting a veto.
    ///
    /// Call outside the App borrow. GPU quiescence and terminal binding release may finish later.
    pub fn close(&self) -> Result<()> {
        if self.is_closed() {
            return Ok(());
        }
        if self.state.bound_id.get().is_none() {
            self.state.finish_close();
            return Ok(());
        }
        let callback = self
            .state
            .close_required_callback
            .take()
            .context("hosted surface has no close-required callback")?;
        self.state.begin_close();
        callback();
        Ok(())
    }

    /// Returns whether close was requested or the binding has retired.
    pub fn is_closed(&self) -> bool {
        self.state.closing.get() || self.state.terminal.get()
    }

    /// Returns the latest coalesced text/IME state without invoking a handler.
    pub fn text_input_state(&self) -> HostedSurfaceTextInputState {
        self.state.text_input.borrow().clone()
    }

    /// Returns the cursor requested by the original Window for host-side cursor selection.
    pub fn cursor_style(&self) -> CursorStyle {
        self.state.cursor_style.get()
    }

    pub(crate) fn window_options(&self) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                Point::default(),
                self.geometry().size(),
            ))),
            titlebar: None,
            focus_on_appearing: false,
            activation_policy: WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: false,
            },
            show: self.state.host_visible.get(),
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            ..WindowOptions::default()
        }
    }

    pub(crate) fn capabilities(&self) -> PlatformWindowCapabilities {
        PlatformWindowCapabilities {
            creation: PlatformWindowCreationCapabilities {
                focus_on_appearing: WindowCreationSupport::Supported,
                initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
                ..PlatformWindowCreationCapabilities::default()
            },
            mutations: PlatformWindowMutationCapabilities {
                activation_policy: WindowMutationSupport::CreationOnly,
                coordinate_space: WindowCoordinateSpace::WindowLocal,
                ..PlatformWindowMutationCapabilities::default()
            },
            ..PlatformWindowCapabilities::default()
        }
    }

    pub(crate) fn create_platform_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        self.state.ensure_open()?;
        ensure!(
            self.state.bound_id.get().is_none(),
            "hosted surface was already bound"
        );
        ensure!(
            matches!(params.kind, WindowKind::Normal)
                && params.transient_for.is_none()
                && params.provisional_session.is_none()
                && params.display_id.is_none(),
            "hosted surfaces do not support native window kinds, relationships or displays"
        );
        ensure!(
            !params.focus_on_appearing
                && !params.activation_policy.accepts_activation
                && !params.activation_policy.focus_on_click,
            "hosted surfaces do not support native activation"
        );
        ensure!(
            matches!(params.window_bounds, WindowBounds::Windowed(_)),
            "hosted surfaces do not support native window states"
        );
        self.state.creation_facts.replace(Some(WindowCreationFacts {
            show: params.show,
            focus_on_appearing: params.focus_on_appearing,
            transient_for: params.transient_for,
        }));
        self.state.bound_id.set(Some(handle.window_id()));
        Ok(Box::new(HostedSurfaceWindow::new(self.state.clone())))
    }
}

impl HostedSurfaceState {
    fn ensure_open(&self) -> Result<()> {
        ensure!(
            !self.closing.get() && !self.terminal.get(),
            "hosted surface is closed"
        );
        Ok(())
    }

    fn ensure_bound(&self) -> Result<()> {
        self.ensure_open()?;
        ensure!(
            self.bound_id.get().is_some(),
            "hosted surface is not bound to a Window"
        );
        Ok(())
    }

    fn invalidate_submission(&self) {
        let pending = self.pending_submission.take();
        if let Some(pending) = pending {
            pending.cancel();
        }
    }

    fn request_frame(&self, mut options: RequestFrameOptions) {
        if self.closing.get() || self.terminal.get() {
            return;
        }
        if options.signal_at.is_none() {
            options.signal_at = PlatformFrameSignal::capture(open_gpui_scheduler::Instant::now);
            options.signal_source = FrameRequestSource::LocalSchedule;
        }
        {
            let mut pending = self.pending_frame.borrow_mut();
            match pending.as_mut() {
                Some(pending) => pending.merge(options),
                None => *pending = Some(options),
            }
        }
        (self.wake)();
    }

    fn request_repaint(&self) {
        self.request_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
            signal_source: FrameRequestSource::LocalSchedule,
            ..RequestFrameOptions::default()
        });
    }

    fn visibility(&self) -> WindowVisibility {
        if !self.closing.get()
            && !self.terminal.get()
            && self.host_visible.get()
            && self.initially_presented.get()
        {
            WindowVisibility::Visible
        } else {
            WindowVisibility::Hidden
        }
    }

    fn begin_close(&self) {
        self.invalidate_submission();
        self.closing.set(true);
        self.pending_frame.take();
        self.interaction_quiesced.set(true);
        self.input_callback.terminate();
    }

    fn finish_close(&self) {
        if self.terminal.get() {
            return;
        }
        self.begin_close();
        self.terminal.set(true);
        self.input_handler.terminate();
        self.active.set(false);
        self.hovered.set(false);
        self.visibility_callback.terminate();
        self.request_frame_callback.take();
        self.active_callback.take();
        self.hover_callback.take();
        self.resize_callback.take();
        self.close_required_callback.take();
        self.update_text_input(|state| {
            state.focused = false;
            state.soft_keyboard_visible = false;
            state.caret_bounds = None;
        });
        let callback = self.close_callback.take();
        if let Some(callback) = callback {
            callback();
        }
    }

    fn update_text_input(&self, update: impl FnOnce(&mut HostedSurfaceTextInputState)) {
        {
            let mut state = self.text_input.borrow_mut();
            state.revision = state
                .revision
                .checked_add(1)
                .expect("hosted text-input revision overflowed");
            update(&mut state);
        }
        (self.wake)();
    }

    fn text_input_changed(&self, change: TextInputStateChange) {
        self.update_text_input(|state| match change {
            TextInputStateChange::FocusGained => state.focused = true,
            TextInputStateChange::FocusLost => {
                state.focused = false;
                state.caret_bounds = None;
            }
            TextInputStateChange::SelectionChanged => state.selection_revision = state.revision,
            TextInputStateChange::ContentChanged => state.content_revision = state.revision,
        });
    }

    fn update_input_snapshot(&self, input: &PlatformInput) {
        let (position, modifiers, hovered) = match input {
            PlatformInput::KeyDown(event) => (None, Some(event.keystroke.modifiers), None),
            PlatformInput::KeyUp(event) => (None, Some(event.keystroke.modifiers), None),
            PlatformInput::ModifiersChanged(event) => {
                self.capslock.set(event.capslock);
                (None, Some(event.modifiers), None)
            }
            PlatformInput::MouseDown(event) => (Some(event.position), Some(event.modifiers), None),
            PlatformInput::MouseUp(event) => (Some(event.position), Some(event.modifiers), None),
            PlatformInput::MouseMove(event) => (Some(event.position), Some(event.modifiers), None),
            PlatformInput::MousePressure(event) => {
                (Some(event.position), Some(event.modifiers), None)
            }
            PlatformInput::MouseExited(event) => {
                (Some(event.position), Some(event.modifiers), Some(false))
            }
            PlatformInput::Scroll(event) => (Some(event.position), Some(event.modifiers), None),
            PlatformInput::Pinch(event) => (Some(event.position), Some(event.modifiers), None),
            PlatformInput::FileDrop(FileDropEvent::Entered { position, .. })
            | PlatformInput::FileDrop(FileDropEvent::Pending { position })
            | PlatformInput::FileDrop(FileDropEvent::Submit { position }) => {
                (Some(*position), None, None)
            }
            PlatformInput::FileDrop(FileDropEvent::Exited | FileDropEvent::Ended) => {
                (None, None, Some(false))
            }
            PlatformInput::PointerCanceled(_)
            | PlatformInput::Touch(_)
            | PlatformInput::LongPress(_)
            | PlatformInput::TouchDrag(_)
            | PlatformInput::BackGesture(_) => (None, None, None),
        };
        if let Some(position) = position {
            self.mouse_position.set(position);
        }
        if let Some(modifiers) = modifiers {
            self.modifiers.set(modifiers);
        }
        let hovered = hovered.or_else(|| {
            position.map(|position| {
                Bounds::new(Point::default(), self.geometry.get().size()).contains(&position)
            })
        });
        if let Some(hovered) = hovered
            && self.hovered.replace(hovered) != hovered
        {
            let callback = self.hover_callback.borrow().clone();
            if let Some(callback) = callback {
                callback(hovered);
            }
        }
    }
}
