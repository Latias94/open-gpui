use crate::{
    A11yCallbacks, AnyWindowHandle, AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome,
    AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasTextureId, AtlasTextureInstanceId,
    AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTile, Bounds, CursorStyle, DevicePixels,
    DispatchEventResult, GpuSpecs, Pixels, Platform, PlatformAtlas, PlatformDisplay,
    PlatformHeadlessRenderer, PlatformInput, PlatformInputCallback, PlatformInputCallbackSlot,
    PlatformInputHandler, PlatformInputHandlerSlot, PlatformNativePointerPhysicalFrame,
    PlatformNativeWindowRetirementOutcome, PlatformPhysicalDisplayObservation,
    PlatformPointerCaptureReleaseOutcome, PlatformPresentationShutdownOutcome, PlatformWindow,
    PlatformWindowActiveStatusObservation, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PlatformWindowDispatch, PlatformWindowInteractionQuiescence,
    PlatformWindowMutationObservation, PlatformWindowMutationTerminal,
    PlatformWindowMutationUnobservedTerminal, PlatformWindowPhysicalGeometry,
    PlatformWindowPresentOutcome, Point, PreparedPlatformPointerCaptureRelease,
    PreparedPlatformPresentationShutdown, PromptButton, RequestFrameOptions, Scene, Size,
    TestPlatform, TileId, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowCreationFacts, WindowMutationDomain, WindowMutationRequest,
    WindowParams, WindowPlacementState, WindowPlatformFacts, WindowPresentationShutdownTicket,
    WindowProvisionalPlacementNativeFacts, WindowProvisionalPlacementOutcome,
    WindowProvisionalPlacementRequest, WindowProvisionalRevealNativeFacts,
    WindowProvisionalRevealZOrder, WindowProvisionalSession,
};
#[cfg(test)]
use crate::{
    DisplayId, NativePointerCancelReservation, PointerCancelReason,
    WindowProvisionalPlacementPurpose,
};
use image::RgbaImage;
use open_gpui_collections::{HashMap, VecDeque};
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    rc::{Rc, Weak},
    sync::{
        self, Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_EMERGENCY_PRESENTATION_SHUTDOWN_GENERATION: AtomicU64 = AtomicU64::new(1);

fn test_display_observation(state: &TestWindowState) -> Option<PlatformPhysicalDisplayObservation> {
    PlatformPhysicalDisplayObservation::try_new(
        1,
        state.display.id(),
        state.display.bounds().to_device_pixels(state.scale_factor),
        state
            .display
            .visible_bounds()
            .to_device_pixels(state.scale_factor),
        state.scale_factor,
    )
}

fn test_physical_geometry(state: &TestWindowState) -> Option<PlatformWindowPhysicalGeometry> {
    let geometry =
        PlatformWindowPhysicalGeometry::try_new(state.physical_client_bounds?, state.scale_factor)?;
    match state.physical_display_observation {
        Some(display) => geometry.with_display_observation(display),
        None => Some(geometry),
    }
}

pub(crate) struct TestWindowState {
    pub(crate) bounds: Bounds<Pixels>,
    physical_client_bounds: Option<Bounds<DevicePixels>>,
    physical_display_observation: Option<PlatformPhysicalDisplayObservation>,
    scale_factor: f32,
    native_pointer_physical_frame: Option<PlatformNativePointerPhysicalFrame>,
    last_native_pointer_physical_frame: Option<PlatformNativePointerPhysicalFrame>,
    native_pointer_capture_release_count: usize,
    native_pointer_capture_release_prepare_history: Vec<u64>,
    native_pointer_capture_release_history: Vec<u64>,
    pub(crate) handle: AnyWindowHandle,
    display: Rc<dyn PlatformDisplay>,
    #[cfg(test)]
    requested_display_id: Option<DisplayId>,
    pub(crate) title: Option<String>,
    pub(crate) edited: bool,
    pub(crate) document_path: Option<std::path::PathBuf>,
    platform: Weak<TestPlatform>,
    // TODO: Replace with `Rc`
    sprite_atlas: Arc<dyn PlatformAtlas>,
    renderer: Option<Box<dyn PlatformHeadlessRenderer>>,
    should_close_handler: Option<Box<dyn FnMut() -> bool>>,
    close_callback: Option<Box<dyn FnOnce()>>,
    native_drop_callback: Option<Box<dyn FnMut()>>,
    hit_test_window_control_callback: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
    input_callback: PlatformInputCallbackSlot,
    active_status_change_callback: Option<Box<dyn FnMut(PlatformWindowActiveStatusObservation)>>,
    hover_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    request_frame_callback: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    defer_frame_requests: bool,
    deferred_frame_request: Option<RequestFrameOptions>,
    resize_callback: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved_callback: Option<Box<dyn FnMut()>>,
    window_state_change_callback: Option<Box<dyn FnMut()>>,
    mutation_observation_callback: Option<Box<dyn FnMut(PlatformWindowMutationObservation)>>,
    input_handler: PlatformInputHandlerSlot,
    ime_position_history: Vec<Bounds<Pixels>>,
    is_minimized: bool,
    is_maximized: bool,
    is_fullscreen: bool,
    is_active: bool,
    interaction_quiesced: bool,
    accepts_pointer_input: bool,
    focus_on_appearing: bool,
    accepts_activation: bool,
    focus_on_click: bool,
    transient_for: Option<AnyWindowHandle>,
    background_appearance: WindowBackgroundAppearance,
    topmost: bool,
    taskbar_visible: bool,
    window_bounds: WindowBounds,
    pending_mutations: Vec<TestWindowMutationRequest>,
    mutation_generations: HashMap<WindowMutationDomain, u64>,
    #[cfg(test)]
    mutation_unobserved_finish_history: Vec<(
        WindowMutationDomain,
        u64,
        PlatformWindowMutationUnobservedTerminal,
    )>,
    next_mutation_dispatches: HashMap<WindowMutationDomain, PlatformWindowDispatch>,
    platform_command_callback:
        Option<Box<dyn FnMut(PlatformWindowCommand, TestWindow) -> PlatformWindowCommandOutcome>>,
    pointer_capture_release_callback:
        Option<Box<dyn FnMut(u64, TestWindow) -> PlatformPointerCaptureReleaseOutcome>>,
    platform_command_history: Vec<PlatformWindowCommand>,
    initial_presentation_command_outcomes: VecDeque<PlatformWindowCommandOutcome>,
    show_on_initial_presentation: bool,
    provisional_session: Option<WindowProvisionalSession>,
    provisional_reveal_generation: Option<u64>,
    creation_show_fact: bool,
    mapped: bool,
    initial_presentation_completed: bool,
    present_outcome: PlatformWindowPresentOutcome,
    reveal_on_next_present: bool,
    close_on_next_present: bool,
    activation_count: usize,
    pub(crate) cursor_style: CursorStyle,
    accessibility: TestAccessibilityState,
    map_error: Option<String>,
    close_during_map: bool,
    closed: bool,
    defer_close_callback: bool,
    deferred_close_callback: Option<Box<dyn FnOnce()>>,
    presentation_shutdown_ticket: Option<WindowPresentationShutdownTicket>,
    presentation_shutdown_prepare_count: usize,
    presentation_shutdown_quiesce_attempt_count: usize,
    presentation_shutdown_retire_count: usize,
    native_retirement_rejections_remaining: usize,
    presentation_shutdown_blocked: bool,
    draw_count: usize,
}

struct TestNativePointerPhysicalFrameScope {
    state: Rc<Mutex<TestWindowState>>,
    previous: Option<PlatformNativePointerPhysicalFrame>,
}

impl TestNativePointerPhysicalFrameScope {
    fn enter(
        state: Rc<Mutex<TestWindowState>>,
        frame: Option<PlatformNativePointerPhysicalFrame>,
    ) -> Self {
        let previous = {
            let mut state = state.lock();
            if let Some(frame) = frame {
                state.last_native_pointer_physical_frame = Some(frame);
            }
            std::mem::replace(&mut state.native_pointer_physical_frame, frame)
        };
        Self { state, previous }
    }
}

impl Drop for TestNativePointerPhysicalFrameScope {
    fn drop(&mut self) {
        self.state.lock().native_pointer_physical_frame = self.previous;
    }
}

fn test_native_pointer_physical_frame(
    event: &PlatformInput,
    state: &TestWindowState,
) -> Option<PlatformNativePointerPhysicalFrame> {
    if matches!(event, PlatformInput::PointerCanceled(_)) {
        return state.last_native_pointer_physical_frame;
    }
    let position = match event {
        PlatformInput::MouseDown(event) => event.position,
        PlatformInput::MouseUp(event) => event.position,
        PlatformInput::MouseMove(event) => event.position,
        PlatformInput::MouseExited(event) => event.position,
        _ => return None,
    };
    let target_display = test_display_observation(state)?;
    let geometry = test_physical_geometry(state)?;
    let global_position = geometry.local_to_global(position)?;
    PlatformNativePointerPhysicalFrame::new(global_position, geometry)
        .with_target_display(target_display)
}

#[derive(Clone)]
struct TestWindowMutationRequest {
    generation: u64,
    request: WindowMutationRequest,
    provisional_placement: Option<WindowProvisionalPlacementRequest>,
}

fn test_provisional_placement_native_facts(
    state: &TestWindowState,
    request: &WindowProvisionalPlacementRequest,
) -> WindowProvisionalPlacementNativeFacts {
    WindowProvisionalPlacementNativeFacts::new(
        window_platform_facts(state)
            .physical_geometry
            .is_some_and(|geometry| request.physical_request().matches_geometry(geometry)),
        state.mapped,
        true,
        true,
        true,
        WindowProvisionalRevealZOrder::Exact,
    )
}

fn settle_test_provisional_placement(
    session: WindowProvisionalSession,
    window_id: crate::WindowId,
    request: WindowProvisionalPlacementRequest,
    native_facts: Option<WindowProvisionalPlacementNativeFacts>,
    outcome: WindowProvisionalPlacementOutcome,
) {
    if let Some(native_facts) = native_facts {
        let _ =
            session.record_native_final_placement(window_id, request.generation(), native_facts);
    }
    let _ = session.settle_native_final_placement(window_id, request.generation(), outcome);
}

#[derive(Default)]
struct TestAccessibilityState {
    callbacks: Option<Rc<A11yCallbacks>>,
    active: bool,
    updates: Vec<accesskit::TreeUpdate>,
}

impl TestAccessibilityState {
    fn record_platform_delivery(&mut self, update: accesskit::TreeUpdate) {
        self.updates.push(update);
    }

    fn retain_activation_result(
        &mut self,
        callbacks: &Rc<A11yCallbacks>,
        update: Option<accesskit::TreeUpdate>,
    ) {
        let is_current_adapter = self
            .callbacks
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, callbacks));
        if self.active
            && is_current_adapter
            && let Some(update) = update
        {
            self.record_platform_delivery(update);
        }
    }
}

pub struct TestWindow(pub(crate) Rc<Mutex<TestWindowState>>, bool);

impl Clone for TestWindow {
    fn clone(&self) -> Self {
        Self(self.0.clone(), false)
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        if !self.1 {
            return;
        }
        self.close();
    }
}

impl HasWindowHandle for TestWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        unimplemented!("Test Windows are not backed by a real platform window")
    }
}

impl HasDisplayHandle for TestWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        unimplemented!("Test Windows are not backed by a real platform window")
    }
}

impl TestWindow {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        platform: Weak<TestPlatform>,
        display: Rc<dyn PlatformDisplay>,
        renderer: Option<Box<dyn PlatformHeadlessRenderer>>,
        map_error: Option<String>,
        close_during_map: bool,
        creation_show_fact: Option<bool>,
        initial_presentation_command_outcomes: Option<VecDeque<PlatformWindowCommandOutcome>>,
        close_on_next_present: bool,
        defer_frame_requests: bool,
    ) -> Self {
        let provisional_session = params.provisional_session.clone();
        let show_on_initial_presentation = params.show && provisional_session.is_none();
        let sprite_atlas: Arc<dyn PlatformAtlas> = match &renderer {
            Some(r) => r.sprite_atlas(),
            None => Arc::new(TestAtlas::new()),
        };
        Self(
            Rc::new(Mutex::new(TestWindowState {
                bounds: params.bounds,
                physical_client_bounds: None,
                physical_display_observation: None,
                scale_factor: 2.0,
                native_pointer_physical_frame: None,
                last_native_pointer_physical_frame: None,
                native_pointer_capture_release_count: 0,
                native_pointer_capture_release_prepare_history: Vec::new(),
                native_pointer_capture_release_history: Vec::new(),
                display,
                #[cfg(test)]
                requested_display_id: params.display_id,
                platform,
                handle,
                sprite_atlas,
                renderer,
                title: Default::default(),
                edited: false,
                document_path: None,
                should_close_handler: None,
                close_callback: None,
                native_drop_callback: None,
                hit_test_window_control_callback: None,
                input_callback: PlatformInputCallbackSlot::default(),
                active_status_change_callback: None,
                hover_status_change_callback: None,
                request_frame_callback: None,
                resize_callback: None,
                moved_callback: None,
                window_state_change_callback: None,
                mutation_observation_callback: None,
                input_handler: PlatformInputHandlerSlot::default(),
                ime_position_history: Vec::new(),
                is_minimized: false,
                is_maximized: matches!(params.window_bounds, WindowBounds::Maximized(_)),
                is_fullscreen: matches!(params.window_bounds, WindowBounds::Fullscreen(_)),
                is_active: false,
                interaction_quiesced: false,
                accepts_pointer_input: params.accepts_pointer_input,
                focus_on_appearing: params.focus_on_appearing,
                accepts_activation: params.activation_policy.accepts_activation,
                focus_on_click: params.activation_policy.focus_on_click,
                transient_for: params.transient_for,
                background_appearance: WindowBackgroundAppearance::Opaque,
                topmost: false,
                taskbar_visible: true,
                window_bounds: params.window_bounds,
                pending_mutations: Vec::new(),
                mutation_generations: HashMap::default(),
                #[cfg(test)]
                mutation_unobserved_finish_history: Vec::new(),
                next_mutation_dispatches: HashMap::default(),
                platform_command_callback: None,
                pointer_capture_release_callback: None,
                platform_command_history: Vec::new(),
                initial_presentation_command_outcomes: initial_presentation_command_outcomes
                    .unwrap_or_default(),
                show_on_initial_presentation,
                provisional_session,
                provisional_reveal_generation: None,
                creation_show_fact: creation_show_fact.unwrap_or(params.show),
                mapped: false,
                initial_presentation_completed: false,
                present_outcome: PlatformWindowPresentOutcome::Submitted,
                reveal_on_next_present: false,
                close_on_next_present,
                activation_count: 0,
                cursor_style: CursorStyle::Arrow,
                accessibility: TestAccessibilityState::default(),
                map_error,
                close_during_map,
                closed: false,
                defer_close_callback: false,
                deferred_close_callback: None,
                presentation_shutdown_ticket: None,
                presentation_shutdown_prepare_count: 0,
                presentation_shutdown_quiesce_attempt_count: 0,
                presentation_shutdown_retire_count: 0,
                native_retirement_rejections_remaining: 0,
                presentation_shutdown_blocked: false,
                draw_count: 0,
                defer_frame_requests,
                deferred_frame_request: None,
            })),
            true,
        )
    }

    #[cfg(test)]
    fn requested_display_id(&self) -> Option<DisplayId> {
        self.0.lock().requested_display_id
    }

    #[cfg(test)]
    pub(crate) fn set_platform_command_callback(
        &self,
        callback: impl FnMut(PlatformWindowCommand, TestWindow) -> PlatformWindowCommandOutcome
        + 'static,
    ) {
        let mut state = self.0.lock();
        if !state.closed {
            state.platform_command_callback = Some(Box::new(callback));
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_pointer_capture_release_callback(
        &self,
        callback: impl FnMut(u64, TestWindow) -> PlatformPointerCaptureReleaseOutcome + 'static,
    ) {
        let mut state = self.0.lock();
        if !state.closed {
            state.pointer_capture_release_callback = Some(Box::new(callback));
        }
    }

    #[cfg(test)]
    pub(crate) fn set_native_drop_callback(&self, callback: impl FnMut() + 'static) {
        let mut state = self.0.lock();
        if !state.closed {
            state.native_drop_callback = Some(Box::new(callback));
        }
    }

    #[cfg(test)]
    pub(crate) fn platform_command_history(&self) -> Vec<PlatformWindowCommand> {
        self.0.lock().platform_command_history.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn activation_count(&self) -> usize {
        self.0.lock().activation_count
    }

    #[cfg(test)]
    pub(crate) fn handle(&self) -> AnyWindowHandle {
        self.0.lock().handle
    }

    #[cfg(test)]
    pub(crate) fn native_pointer_capture_release_history(&self) -> Vec<u64> {
        self.0.lock().native_pointer_capture_release_history.clone()
    }

    #[cfg(test)]
    pub(crate) fn native_pointer_capture_release_prepare_history(&self) -> Vec<u64> {
        self.0
            .lock()
            .native_pointer_capture_release_prepare_history
            .clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_present_outcome(&self, outcome: PlatformWindowPresentOutcome) {
        self.0.lock().present_outcome = outcome;
    }

    #[cfg(test)]
    pub(crate) fn close_on_next_present_for_test(&self) {
        self.0.lock().close_on_next_present = true;
    }

    #[cfg(test)]
    pub(crate) fn defer_frame_requests_for_test(&self) {
        self.0.lock().defer_frame_requests = true;
    }

    #[cfg(test)]
    pub(crate) fn release_deferred_frame_request_for_test(&self) -> bool {
        let options = {
            let mut state = self.0.lock();
            state.defer_frame_requests = false;
            state.deferred_frame_request.take()
        };
        options.is_some_and(|options| self.simulate_frame(options))
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn step_deferred_frame_request_for_test(&self) -> bool {
        let options = {
            let mut state = self.0.lock();
            assert!(
                state.defer_frame_requests,
                "stepping one frame request requires deferred delivery"
            );
            state.deferred_frame_request.take()
        };
        options.is_some_and(|options| self.simulate_frame(options))
    }

    #[cfg(test)]
    pub(crate) fn set_visible_for_test(&self, visible: bool) {
        self.0.lock().mapped = visible;
    }

    #[cfg(test)]
    pub(crate) fn reveal_on_next_present(&self) {
        self.0.lock().reveal_on_next_present = true;
    }

    #[cfg(test)]
    pub(crate) fn clear_platform_command_history(&self) {
        self.0.lock().platform_command_history.clear();
    }

    #[cfg(test)]
    pub(crate) fn initial_presentation_state(&self) -> (bool, bool, usize) {
        let state = self.0.lock();
        (
            state.mapped,
            state.initial_presentation_completed,
            state.activation_count,
        )
    }

    fn execute_platform_command(
        &self,
        command: PlatformWindowCommand,
    ) -> PlatformWindowCommandOutcome {
        let (callback, scripted_outcome) = {
            let mut state = self.0.lock();
            if state.closed {
                return PlatformWindowCommandOutcome::WindowClosed;
            }
            state.platform_command_history.push(command);
            let scripted_outcome = matches!(
                command,
                PlatformWindowCommand::CompleteInitialPresentation { .. }
            )
            .then(|| state.initial_presentation_command_outcomes.pop_front())
            .flatten();
            (state.platform_command_callback.take(), scripted_outcome)
        };
        let outcome = if let Some(mut callback) = callback {
            let outcome = callback(command, self.clone());
            let mut state = self.0.lock();
            if !state.closed && state.platform_command_callback.is_none() {
                state.platform_command_callback = Some(callback);
            }
            outcome
        } else {
            scripted_outcome.unwrap_or(PlatformWindowCommandOutcome::Accepted)
        };
        if outcome != PlatformWindowCommandOutcome::Accepted {
            return outcome;
        }

        match command {
            PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                let should_activate = {
                    let mut state = self.0.lock();
                    state.initial_presentation_completed = true;
                    state.mapped = state.show_on_initial_presentation;
                    let should_activate = activate && state.mapped && state.accepts_activation;
                    if should_activate {
                        state.activation_count += 1;
                    }
                    should_activate
                };
                if should_activate {
                    self.activate_for_test();
                }
            }
            PlatformWindowCommand::RevealDeferredInitialPresentation {
                session_generation,
                presentation_generation,
                ..
            } => {
                let mut state = self.0.lock();
                let session = state.provisional_session.clone();
                let window_id = state.handle.window_id();
                let accepts_reveal = session.as_ref().is_some_and(|session| {
                    let snapshot = session.snapshot();
                    snapshot.generation() == session_generation
                        && snapshot.window_id() == Some(window_id)
                        && snapshot.phase() == crate::WindowProvisionalSessionPhase::Gated
                }) && state.initial_presentation_completed
                    && !state.mapped
                    && state.provisional_reveal_generation.is_none();
                if !accepts_reveal {
                    return PlatformWindowCommandOutcome::Rejected;
                }
                let session = session.expect("accepted provisional reveal must retain its session");
                if session
                    .claim_native_reveal(window_id, presentation_generation)
                    .is_err()
                {
                    return PlatformWindowCommandOutcome::Rejected;
                }
                state.mapped = true;
                state.provisional_reveal_generation = Some(presentation_generation);
                drop(state);
                let recorded = session
                    .record_native_reveal(
                        window_id,
                        presentation_generation,
                        WindowProvisionalRevealNativeFacts::new(
                            true,
                            true,
                            true,
                            true,
                            true,
                            WindowProvisionalRevealZOrder::Exact,
                        ),
                    )
                    .is_ok();
                if !recorded {
                    let mut state = self.0.lock();
                    state.mapped = false;
                    state.provisional_reveal_generation = None;
                    return PlatformWindowCommandOutcome::Rejected;
                }
            }
            PlatformWindowCommand::Activate { .. } => {
                let accepts_activation = {
                    let state = self.0.lock();
                    state.accepts_activation
                        && state.provisional_session.as_ref().is_none_or(|session| {
                            let snapshot = session.snapshot();
                            snapshot.window_id() == Some(state.handle.window_id())
                                && snapshot.accepts_interaction()
                        })
                };
                if !accepts_activation {
                    return PlatformWindowCommandOutcome::Rejected;
                }
                self.0.lock().activation_count += 1;
                self.activate_for_test();
            }
            PlatformWindowCommand::ShowWindowMenu(_)
            | PlatformWindowCommand::StartWindowMove
            | PlatformWindowCommand::StartWindowResize(_) => {}
        }
        outcome
    }

    fn execute_pointer_capture_release(
        &self,
        release_generation: u64,
    ) -> PlatformPointerCaptureReleaseOutcome {
        let callback = {
            let mut state = self.0.lock();
            if state.closed {
                return PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal;
            }
            state
                .native_pointer_capture_release_history
                .push(release_generation);
            state.pointer_capture_release_callback.take()
        };
        let outcome = if let Some(mut callback) = callback {
            let outcome = callback(release_generation, self.clone());
            let mut state = self.0.lock();
            if !state.closed && state.pointer_capture_release_callback.is_none() {
                state.pointer_capture_release_callback = Some(callback);
            }
            outcome
        } else {
            PlatformPointerCaptureReleaseOutcome::Released
        };
        if outcome == PlatformPointerCaptureReleaseOutcome::Released {
            self.0.lock().native_pointer_capture_release_count += 1;
        }
        outcome
    }

    fn activate_for_test(&self) {
        self.0
            .lock()
            .platform
            .upgrade()
            .expect("platform dropped")
            .set_active_window(Some(self.clone()));
    }

    pub(crate) fn activate_accessibility(&self) -> bool {
        let callbacks = {
            let mut state = self.0.lock();
            let Some(callbacks) = state.accessibility.callbacks.clone() else {
                return false;
            };
            state.accessibility.active = true;
            callbacks
        };

        let initial_update = (callbacks.activation)();
        self.0
            .lock()
            .accessibility
            .retain_activation_result(&callbacks, initial_update);
        true
    }

    #[cfg(test)]
    pub(crate) fn ime_position_history(&self) -> Vec<Bounds<Pixels>> {
        self.0.lock().ime_position_history.clone()
    }

    #[cfg(test)]
    pub(crate) fn clear_ime_position_history(&self) {
        self.0.lock().ime_position_history.clear();
    }

    pub(crate) fn deactivate_accessibility(&self) -> bool {
        let callbacks = {
            let mut state = self.0.lock();
            if !state.accessibility.active {
                return false;
            }
            let Some(callbacks) = state.accessibility.callbacks.clone() else {
                return false;
            };
            state.accessibility.active = false;
            callbacks
        };

        (callbacks.deactivation)();
        true
    }

    pub(crate) fn dispatch_accessibility_action(&self, request: accesskit::ActionRequest) -> bool {
        let callbacks = {
            let state = self.0.lock();
            if !state.accessibility.active {
                return false;
            }
            let Some(callbacks) = state.accessibility.callbacks.clone() else {
                return false;
            };
            callbacks
        };

        (callbacks.action)(request);
        true
    }

    pub(crate) fn latest_accessibility_tree_update(&self) -> Option<accesskit::TreeUpdate> {
        self.0
            .lock()
            .accessibility
            .updates
            .last()
            .cloned()
            .map(normalize_accessibility_tree_update)
    }

    pub(crate) fn accessibility_tree_update_history(&self) -> Vec<accesskit::TreeUpdate> {
        self.0
            .lock()
            .accessibility
            .updates
            .iter()
            .cloned()
            .map(normalize_accessibility_tree_update)
            .collect()
    }

    pub fn simulate_resize(&mut self, size: Size<Pixels>) {
        let scale_factor = self.scale_factor();
        let mut lock = self.0.lock();
        // Always update bounds, even if no callback is registered
        lock.bounds.size = size;
        if !lock.is_minimized && !lock.is_maximized && !lock.is_fullscreen {
            lock.window_bounds = WindowBounds::Windowed(lock.bounds);
        }
        let Some(mut callback) = lock.resize_callback.take() else {
            return;
        };
        drop(lock);
        callback(size, scale_factor);
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.resize_callback = Some(callback);
        }
    }

    pub(crate) fn set_physical_client_geometry(
        &self,
        bounds: Option<Bounds<DevicePixels>>,
        scale_factor: f32,
    ) {
        if let Some(bounds) = bounds {
            PlatformWindowPhysicalGeometry::try_new(bounds, scale_factor)
                .expect("test physical geometry must be representable");
        } else {
            assert!(
                scale_factor.is_finite() && scale_factor > 0.0,
                "test window scale factor must be finite and positive"
            );
        }
        let mut state = self.0.lock();
        state.physical_client_bounds = bounds;
        state.scale_factor = scale_factor;
        state.physical_display_observation = bounds.and_then(|_| test_display_observation(&state));
    }

    #[cfg(test)]
    pub(crate) fn native_pointer_capture_release_count(&self) -> usize {
        self.0.lock().native_pointer_capture_release_count
    }

    pub fn simulate_minimize(&mut self) {
        let mut lock = self.0.lock();
        lock.is_minimized = true;
        let Some(mut callback) = lock.window_state_change_callback.take() else {
            return;
        };
        drop(lock);
        callback();
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.window_state_change_callback = Some(callback);
        }
    }

    pub(crate) fn simulate_active_status_change(&self, active: bool) {
        self.simulate_active_status_observation(PlatformWindowActiveStatusObservation::new(
            active, active,
        ));
    }

    pub(crate) fn simulate_active_status_observation(
        &self,
        observation: PlatformWindowActiveStatusObservation,
    ) {
        let mut lock = self.0.lock();
        lock.is_active = observation.active();
        let Some(mut callback) = lock.active_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(observation);
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.active_status_change_callback = Some(callback);
        }
    }

    #[cfg(test)]
    pub(crate) fn reserve_reentrant_pointer_cancel_for_test(
        &self,
        reason: PointerCancelReason,
    ) -> NativePointerCancelReservation {
        self.0
            .lock()
            .input_callback
            .clone()
            .reserve_reentrant_pointer_cancel(reason)
    }

    #[cfg(test)]
    pub(crate) fn reserve_pointer_cancel_after_callback_panic_for_test(
        &self,
        reason: PointerCancelReason,
    ) -> NativePointerCancelReservation {
        self.0
            .lock()
            .input_callback
            .clone()
            .reserve_pointer_cancel_after_callback_panic(reason)
    }

    /// Configures the next structured placement dispatch result.
    ///
    /// The default is [`PlatformWindowDispatch::Queued`]. A queued request still leaves the
    /// test window facts unchanged until a test emits an explicit terminal observation.
    pub fn set_next_placement_dispatch(&self, dispatch: PlatformWindowDispatch) {
        self.set_next_window_mutation_dispatch(WindowMutationDomain::Placement, dispatch);
    }

    /// Configures the next pointer-input dispatch result.
    ///
    /// The default is [`PlatformWindowDispatch::Queued`]. A queued request still leaves the
    /// test window facts unchanged until a test emits an explicit terminal observation.
    pub fn set_next_pointer_input_dispatch(&self, dispatch: PlatformWindowDispatch) {
        self.set_next_window_mutation_dispatch(WindowMutationDomain::PointerInput, dispatch);
    }

    /// Configures the next dispatch result for one typed mutation domain.
    pub fn set_next_window_mutation_dispatch(
        &self,
        domain: WindowMutationDomain,
        dispatch: PlatformWindowDispatch,
    ) {
        self.0
            .lock()
            .next_mutation_dispatches
            .insert(domain, dispatch);
    }

    #[cfg(test)]
    pub(crate) fn window_mutation_unobserved_finish_history(
        &self,
    ) -> Vec<(
        WindowMutationDomain,
        u64,
        PlatformWindowMutationUnobservedTerminal,
    )> {
        self.0.lock().mutation_unobserved_finish_history.clone()
    }

    /// Emits the current backend facts as a coherent terminal observation for one domain.
    pub fn emit_window_mutation_observation(&self, domain: WindowMutationDomain) -> bool {
        let mut lock = self.0.lock();
        let Some(generation) = lock.mutation_generations.get(&domain).copied() else {
            return false;
        };
        let provisional_placement = lock
            .pending_mutations
            .iter()
            .rfind(|queued| queued.request.domain() == domain && queued.generation == generation)
            .and_then(|queued| queued.provisional_placement.clone())
            .and_then(|request| {
                let session = lock.provisional_session.clone()?;
                let native_facts = test_provisional_placement_native_facts(&lock, &request);
                Some((session, lock.handle.window_id(), request, native_facts))
            });
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain);
        let facts = window_platform_facts(&lock);
        let mut callback = lock.mutation_observation_callback.take();
        drop(lock);
        if let Some((session, window_id, request, native_facts)) = provisional_placement {
            settle_test_provisional_placement(
                session,
                window_id,
                request,
                Some(native_facts),
                WindowProvisionalPlacementOutcome::Settled,
            );
        }
        let Some(mut callback) = callback else {
            return false;
        };
        callback(PlatformWindowMutationObservation::observed(
            domain, generation, facts,
        ));
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.mutation_observation_callback = Some(callback);
        }
        true
    }

    /// Applies the newest queued request in `domain`, then emits one coherent terminal
    /// observation. Tests can instead change backend facts directly and call
    /// [`Self::emit_window_mutation_observation`] to model adjusted or external results.
    pub fn flush_window_mutation(&self, domain: WindowMutationDomain) -> bool {
        let mut lock = self.0.lock();
        let Some(generation) = lock.mutation_generations.get(&domain).copied() else {
            return false;
        };
        let Some(index) = lock.pending_mutations.iter().rposition(|queued| {
            queued.request.domain() == domain && queued.generation == generation
        }) else {
            return false;
        };
        let queued = lock.pending_mutations.remove(index);
        apply_test_window_mutation(&mut lock, queued.request);
        let provisional_placement = queued.provisional_placement.and_then(|request| {
            let session = lock.provisional_session.clone()?;
            let facts = test_provisional_placement_native_facts(&lock, &request);
            Some((session, lock.handle.window_id(), request, facts))
        });
        let facts = window_platform_facts(&lock);
        let mut callback = lock.mutation_observation_callback.take();
        drop(lock);
        if let Some((session, window_id, request, native_facts)) = provisional_placement {
            settle_test_provisional_placement(
                session,
                window_id,
                request,
                Some(native_facts),
                WindowProvisionalPlacementOutcome::Settled,
            );
        }
        let Some(mut callback) = callback else {
            return false;
        };
        callback(PlatformWindowMutationObservation::observed(
            domain, generation, facts,
        ));
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.mutation_observation_callback = Some(callback);
        }
        true
    }

    /// Replaces the test backend's facts and emits them as one coherent terminal observation.
    pub fn simulate_window_mutation_observation(
        &self,
        domain: WindowMutationDomain,
        facts: WindowPlatformFacts,
    ) -> bool {
        self.simulate_window_mutation_terminal(
            domain,
            PlatformWindowMutationTerminal::Observed,
            facts,
        )
    }

    /// Replaces the test backend's facts and emits one explicit terminal mutation result.
    ///
    /// This is useful for modeling an asynchronously rejected request without incorrectly
    /// classifying the unchanged backend facts as an OS adjustment.
    pub fn simulate_window_mutation_terminal(
        &self,
        domain: WindowMutationDomain,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) -> bool {
        let Some(generation) = self.0.lock().mutation_generations.get(&domain).copied() else {
            return false;
        };
        self.simulate_window_mutation_terminal_for_generation(domain, generation, terminal, facts)
    }

    /// Emits an explicitly generation-bound terminal callback.
    ///
    /// Tests use this to deliver an older callback after a newer request has superseded it.
    pub fn simulate_window_mutation_terminal_for_generation(
        &self,
        domain: WindowMutationDomain,
        generation: u64,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) -> bool {
        let mut lock = self.0.lock();
        let provisional_placement = lock
            .pending_mutations
            .iter()
            .find(|queued| queued.request.domain() == domain && queued.generation == generation)
            .and_then(|queued| queued.provisional_placement.clone())
            .and_then(|request| {
                let session = lock.provisional_session.clone()?;
                Some((session, lock.handle.window_id(), request))
            });
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain || queued.generation != generation);
        apply_window_platform_facts(&mut lock, &facts);
        let provisional_placement = provisional_placement.map(|(session, window_id, request)| {
            let (native_facts, outcome) = match terminal {
                PlatformWindowMutationTerminal::Observed => (
                    Some(test_provisional_placement_native_facts(&lock, &request)),
                    WindowProvisionalPlacementOutcome::Settled,
                ),
                PlatformWindowMutationTerminal::Rejected
                | PlatformWindowMutationTerminal::Unsupported => {
                    (None, WindowProvisionalPlacementOutcome::Rejected)
                }
                PlatformWindowMutationTerminal::WindowClosed => {
                    (None, WindowProvisionalPlacementOutcome::WindowTerminal)
                }
            };
            (session, window_id, request, native_facts, outcome)
        });
        let mut callback = lock.mutation_observation_callback.take();
        drop(lock);
        if let Some((session, window_id, request, native_facts, outcome)) = provisional_placement {
            settle_test_provisional_placement(session, window_id, request, native_facts, outcome);
        }
        let Some(mut callback) = callback else {
            return false;
        };
        callback(PlatformWindowMutationObservation::terminal(
            domain, generation, terminal, facts,
        ));
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.mutation_observation_callback = Some(callback);
        }
        true
    }

    pub(crate) fn simulate_hover_status_change(&self, hovered: bool) {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.hover_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(hovered);
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.hover_status_change_callback = Some(callback);
        }
    }

    fn close(&self) -> bool {
        let (input_callback, input_handler, mut native_drop_callback, callback, retired_callbacks) = {
            let mut state = self.0.lock();
            if state.closed {
                return false;
            }
            state.closed = true;
            state.mapped = false;
            state.is_active = false;
            state.pending_mutations.clear();
            state.mutation_generations.clear();
            state.next_mutation_dispatches.clear();
            state.initial_presentation_command_outcomes.clear();
            state.reveal_on_next_present = false;
            state.close_on_next_present = false;
            state.accessibility.active = false;
            let shutdown = if let Some(shutdown) = state.presentation_shutdown_ticket.as_ref() {
                shutdown.clone()
            } else {
                let generation =
                    NEXT_EMERGENCY_PRESENTATION_SHUTDOWN_GENERATION.fetch_add(1, Ordering::Relaxed);
                assert_ne!(
                    generation, 0,
                    "emergency presentation-shutdown generation space exhausted"
                );
                let shutdown =
                    WindowPresentationShutdownTicket::new(state.handle.window_id(), generation);
                state.presentation_shutdown_ticket = Some(shutdown.clone());
                shutdown
            };
            if !shutdown.acknowledge_native_terminal() {
                log::error!(
                    "test native window reached terminal before presentation quiescence was acknowledged"
                );
            }
            let callback = state.close_callback.take();
            let callback = if state.defer_close_callback {
                state.deferred_close_callback = callback;
                None
            } else {
                callback
            };
            (
                state.input_callback.clone(),
                state.input_handler.clone(),
                state.native_drop_callback.take(),
                callback,
                (
                    state.should_close_handler.take(),
                    state.hit_test_window_control_callback.take(),
                    state.active_status_change_callback.take(),
                    state.hover_status_change_callback.take(),
                    state.request_frame_callback.take(),
                    state.resize_callback.take(),
                    state.moved_callback.take(),
                    state.window_state_change_callback.take(),
                    state.mutation_observation_callback.take(),
                    state.platform_command_callback.take(),
                    state.accessibility.callbacks.take(),
                ),
            )
        };
        input_callback.terminate();
        input_handler.terminate();
        drop(retired_callbacks);
        if let Some(callback) = native_drop_callback.as_mut() {
            callback();
        }
        if let Some(callback) = callback {
            callback();
        }
        true
    }

    pub(crate) fn defer_native_terminal(&self) -> bool {
        let mut state = self.0.lock();
        if state.closed || state.defer_close_callback {
            return false;
        }
        state.defer_close_callback = true;
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn block_presentation_shutdown(&self, blocked: bool) {
        self.0.lock().presentation_shutdown_blocked = blocked;
    }

    #[cfg(test)]
    pub(crate) fn presentation_shutdown_counts(&self) -> (usize, usize, usize) {
        let state = self.0.lock();
        (
            state.presentation_shutdown_prepare_count,
            state.presentation_shutdown_quiesce_attempt_count,
            state.presentation_shutdown_retire_count,
        )
    }

    #[cfg(test)]
    pub(crate) fn reject_native_retirement_attempts(&self, attempts: usize) {
        self.0.lock().native_retirement_rejections_remaining = attempts;
    }

    #[cfg(test)]
    pub(crate) fn draw_count(&self) -> usize {
        self.0.lock().draw_count
    }

    pub(crate) fn release_deferred_native_terminal(&self) -> bool {
        let callback = {
            let mut state = self.0.lock();
            state.defer_close_callback = false;
            state.deferred_close_callback.take()
        };
        let Some(callback) = callback else {
            return false;
        };
        callback();
        true
    }

    pub(crate) fn simulate_close(&self) -> bool {
        self.close()
    }

    pub(crate) fn is_native_terminal(&self) -> bool {
        self.0.lock().closed
    }

    pub(crate) fn should_close(&self) -> bool {
        self.simulate_should_close().unwrap_or(true)
    }

    pub(crate) fn simulate_should_close(&self) -> Option<bool> {
        let mut lock = self.0.lock();
        let mut callback = lock.should_close_handler.take()?;
        drop(lock);
        let result = callback();
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.should_close_handler = Some(callback);
        }
        Some(result)
    }

    #[cfg(test)]
    pub(crate) fn simulate_window_control_hit_test(&self) -> Option<WindowControlArea> {
        let mut lock = self.0.lock();
        if lock.interaction_quiesced {
            return None;
        }
        let mut callback = lock.hit_test_window_control_callback.take()?;
        drop(lock);
        let result = callback();
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.hit_test_window_control_callback = Some(callback);
        }
        result
    }

    pub fn simulate_input_result(&mut self, event: PlatformInput) -> DispatchEventResult {
        let frame = {
            let state = self.0.lock();
            test_native_pointer_physical_frame(&event, &state)
        };
        self.simulate_input_result_with_native_pointer_physical_frame(event, frame)
    }

    /// Simulates input with an exact callback-scoped physical pointer frame.
    #[doc(hidden)]
    pub fn simulate_input_result_with_native_pointer_physical_frame(
        &mut self,
        event: PlatformInput,
        frame: Option<PlatformNativePointerPhysicalFrame>,
    ) -> DispatchEventResult {
        let scope = TestNativePointerPhysicalFrameScope::enter(self.0.clone(), frame);
        let callback = self.0.lock().input_callback.clone();
        let result = callback.dispatch(event);
        drop(scope);
        result
    }

    pub fn simulate_input(&mut self, event: PlatformInput) -> bool {
        !self.simulate_input_result(event).propagate
    }

    /// Simulates the platform delivering a frame request.
    pub fn simulate_frame(&self, options: RequestFrameOptions) -> bool {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.request_frame_callback.take() else {
            return false;
        };
        drop(lock);
        callback(options);
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.request_frame_callback = Some(callback);
        }
        true
    }
}

impl PlatformWindow for TestWindow {
    fn map_window(&mut self) -> anyhow::Result<()> {
        let close_during_map = {
            let mut state = self.0.lock();
            if let Some(message) = state.map_error.take() {
                anyhow::bail!(message);
            }
            std::mem::take(&mut state.close_during_map)
        };
        if close_during_map {
            self.close();
        }
        Ok(())
    }

    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let command_window = Rc::downgrade(&self.0);
        let capture_window = command_window.clone();
        PlatformWindowCommandDispatcher::new_with_pointer_capture_release(
            move |command| {
                if let Some(window) = command_window.upgrade() {
                    TestWindow(window, false).execute_platform_command(command)
                } else {
                    PlatformWindowCommandOutcome::Rejected
                }
            },
            move |release_generation| {
                if let Some(window) = capture_window.upgrade() {
                    window
                        .lock()
                        .native_pointer_capture_release_prepare_history
                        .push(release_generation);
                }
                let capture_window = capture_window.clone();
                PreparedPlatformPointerCaptureRelease::new(move || {
                    if let Some(window) = capture_window.upgrade() {
                        TestWindow(window, false)
                            .execute_pointer_capture_release(release_generation)
                    } else {
                        PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal
                    }
                })
            },
        )
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let window = Rc::downgrade(&self.0);
        PlatformWindowInteractionQuiescence::new(move || {
            if let Some(window) = window.upgrade() {
                let mut window = window.lock();
                window.interaction_quiesced = true;
                window.accepts_pointer_input = false;
                window.is_active = false;
                window.accepts_activation = false;
                window.focus_on_click = false;
            }
        })
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        let shutdown = {
            let mut state = self.0.lock();
            if let Some(current) = state.presentation_shutdown_ticket.as_ref() {
                let current = current.clone();
                state.presentation_shutdown_prepare_count += 1;
                current
            } else {
                state.presentation_shutdown_ticket = Some(shutdown.clone());
                state.presentation_shutdown_prepare_count += 1;
                shutdown
            }
        };
        let window = self.0.clone();
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            let mut state = window.lock();
            state.presentation_shutdown_quiesce_attempt_count += 1;
            if state.presentation_shutdown_blocked {
                return PlatformPresentationShutdownOutcome::Rejected;
            }
            drop(state);
            if shutdown.acknowledge_quiesced() {
                PlatformPresentationShutdownOutcome::Quiesced
            } else {
                PlatformPresentationShutdownOutcome::Rejected
            }
        })
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        let mut state = self.0.lock();
        let exact = state
            .presentation_shutdown_ticket
            .as_ref()
            .is_some_and(|current| current.same_authority(shutdown));
        if !exact || !shutdown.snapshot().quiesced() {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        if state.native_retirement_rejections_remaining > 0 {
            state.native_retirement_rejections_remaining -= 1;
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        state.presentation_shutdown_retire_count += 1;
        PlatformNativeWindowRetirementOutcome::Accepted
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.lock().bounds
    }

    fn physical_geometry(&self) -> Option<PlatformWindowPhysicalGeometry> {
        test_physical_geometry(&self.0.lock())
    }

    fn native_pointer_physical_frame(&self) -> Option<PlatformNativePointerPhysicalFrame> {
        self.0.lock().native_pointer_physical_frame
    }

    fn window_bounds(&self) -> WindowBounds {
        self.0.lock().window_bounds
    }

    fn is_maximized(&self) -> bool {
        self.0.lock().is_maximized
    }

    fn is_minimized(&self) -> bool {
        self.0.lock().is_minimized
    }

    fn accepts_pointer_input(&self) -> bool {
        self.0.lock().accepts_pointer_input
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        let state = self.0.lock();
        WindowCreationFacts {
            show: state.creation_show_fact,
            focus_on_appearing: state.focus_on_appearing,
            transient_for: state.transient_for,
        }
    }

    fn is_visible(&self) -> bool {
        self.0.lock().mapped
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        window_platform_facts(&self.0.lock())
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        let mut lock = self.0.lock();
        if lock.closed {
            return;
        }
        let stale_provisional_placement = lock
            .pending_mutations
            .iter()
            .find(|queued| queued.request.domain() == domain)
            .and_then(|queued| queued.provisional_placement.clone())
            .and_then(|request| {
                let session = lock.provisional_session.clone()?;
                Some((session, lock.handle.window_id(), request))
            });
        lock.mutation_generations.insert(domain, generation);
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain);
        drop(lock);
        if let Some((session, window_id, request)) = stale_provisional_placement {
            settle_test_provisional_placement(
                session,
                window_id,
                request,
                None,
                WindowProvisionalPlacementOutcome::Stale,
            );
        }
    }

    fn finish_window_mutation_without_observation(
        &self,
        domain: WindowMutationDomain,
        generation: u64,
        _terminal: PlatformWindowMutationUnobservedTerminal,
    ) {
        let mut lock = self.0.lock();
        #[cfg(test)]
        lock.mutation_unobserved_finish_history
            .push((domain, generation, _terminal));
        if lock.mutation_generations.get(&domain).copied() == Some(generation) {
            lock.mutation_generations.remove(&domain);
        }
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain || queued.generation != generation);
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        let mut lock = self.0.lock();
        lock.mutation_generations.remove(&domain);
        let stale_provisional_placement = lock
            .pending_mutations
            .iter()
            .find(|queued| queued.request.domain() == domain)
            .and_then(|queued| queued.provisional_placement.clone())
            .and_then(|request| {
                let session = lock.provisional_session.clone()?;
                Some((session, lock.handle.window_id(), request))
            });
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain);
        drop(lock);
        if let Some((session, window_id, request)) = stale_provisional_placement {
            settle_test_provisional_placement(
                session,
                window_id,
                request,
                None,
                WindowProvisionalPlacementOutcome::Stale,
            );
        }
    }

    fn request_window_mutation(
        &mut self,
        generation: u64,
        request: WindowMutationRequest,
    ) -> PlatformWindowDispatch {
        let mut lock = self.0.lock();
        let domain = request.domain();
        if lock.closed || lock.mutation_generations.get(&domain).copied() != Some(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        let dispatch = lock
            .next_mutation_dispatches
            .remove(&domain)
            .unwrap_or(PlatformWindowDispatch::Queued);
        if matches!(dispatch, PlatformWindowDispatch::Queued) {
            lock.pending_mutations
                .retain(|queued| queued.request.domain() != domain);
            lock.pending_mutations.push(TestWindowMutationRequest {
                generation,
                request,
                provisional_placement: None,
            });
        }
        dispatch
    }

    fn request_provisional_placement(
        &mut self,
        generation: u64,
        request: WindowProvisionalPlacementRequest,
    ) -> PlatformWindowDispatch {
        let mut lock = self.0.lock();
        let Some(session) = lock.provisional_session.clone() else {
            return PlatformWindowDispatch::Rejected;
        };
        let domain = WindowMutationDomain::Placement;
        let request_is_current = matches!(
            session.final_placement_request(lock.handle.window_id(), request.generation()),
            Ok(current) if current == request
        );
        let physical_request = request.physical_request();
        if lock.closed
            || !lock.mapped
            || lock.mutation_generations.get(&domain).copied() != Some(generation)
            || !request_is_current
        {
            return PlatformWindowDispatch::Rejected;
        }

        let dispatch = lock
            .next_mutation_dispatches
            .remove(&domain)
            .unwrap_or(PlatformWindowDispatch::Queued);
        match dispatch {
            PlatformWindowDispatch::Queued => {
                lock.pending_mutations
                    .retain(|queued| queued.request.domain() != domain);
                lock.pending_mutations.push(TestWindowMutationRequest {
                    generation,
                    request: WindowMutationRequest::PhysicalPlacement(physical_request),
                    provisional_placement: Some(request),
                });
            }
            PlatformWindowDispatch::Unchanged => {
                let facts = test_provisional_placement_native_facts(&lock, &request);
                let window_id = lock.handle.window_id();
                drop(lock);
                settle_test_provisional_placement(
                    session,
                    window_id,
                    request,
                    Some(facts),
                    WindowProvisionalPlacementOutcome::Settled,
                );
            }
            PlatformWindowDispatch::Unsupported
            | PlatformWindowDispatch::Rejected
            | PlatformWindowDispatch::WindowClosed => {}
        }
        dispatch
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn scale_factor(&self) -> f32 {
        self.0.lock().scale_factor
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn display(&self) -> Option<std::rc::Rc<dyn crate::PlatformDisplay>> {
        Some(self.0.lock().display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let platform = {
            let mut lock = self.0.lock();
            lock.cursor_style = style;
            lock.platform.upgrade()
        };
        if let Some(platform) = platform {
            platform.set_window_cursor_style(self, style);
        }
    }

    fn modifiers(&self) -> crate::Modifiers {
        crate::Modifiers::default()
    }

    fn capslock(&self) -> crate::Capslock {
        crate::Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = self.0.lock().input_handler.clone();
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = self.0.lock().input_handler.clone();
        input_handler_slot.take()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn input_handler_slot_for_test(&self) -> Option<PlatformInputHandlerSlot> {
        Some(self.0.lock().input_handler.clone())
    }

    fn prompt(
        &self,
        _level: crate::PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        Some(
            self.0
                .lock()
                .platform
                .upgrade()
                .expect("platform dropped")
                .prompt(msg, detail, answers),
        )
    }

    fn is_active(&self) -> bool {
        self.0.lock().is_active
    }

    fn is_hovered(&self) -> bool {
        let (platform, handle) = {
            let lock = self.0.lock();
            (lock.platform.upgrade(), lock.handle)
        };
        platform.is_some_and(|platform| platform.hovered_window().window() == Some(handle))
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.0.lock().background_appearance
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn set_title(&mut self, title: &str) {
        self.0.lock().title = Some(title.to_owned());
    }

    fn set_app_id(&mut self, _app_id: &str) {}

    fn set_background_appearance(&self, background: WindowBackgroundAppearance) {
        self.0.lock().background_appearance = background;
    }

    fn set_edited(&mut self, edited: bool) {
        self.0.lock().edited = edited;
    }

    fn set_document_path(&self, path: Option<&std::path::Path>) {
        self.0.lock().document_path = path.map(|p| p.to_path_buf());
    }

    fn show_character_palette(&self) {
        unimplemented!()
    }

    fn is_fullscreen(&self) -> bool {
        self.0.lock().is_fullscreen
    }

    fn request_frame(&self, options: RequestFrameOptions) {
        let deferred = {
            let mut state = self.0.lock();
            if state.defer_frame_requests {
                let pending = state.deferred_frame_request.take();
                state.deferred_frame_request =
                    Some(pending.map_or(options, |pending| RequestFrameOptions {
                        require_presentation: pending.require_presentation
                            || options.require_presentation,
                        force_render: pending.force_render || options.force_render,
                    }));
                true
            } else {
                false
            }
        };
        if deferred {
            return;
        }
        let _ = self.simulate_frame(options);
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.request_frame_callback = Some(callback);
        }
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        let input_callback = self.0.lock().input_callback.clone();
        input_callback.set(callback)
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn FnMut(PlatformWindowActiveStatusObservation)>,
    ) {
        let mut state = self.0.lock();
        if !state.closed {
            state.active_status_change_callback = Some(callback);
        }
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.hover_status_change_callback = Some(callback);
        }
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.resize_callback = Some(callback);
        }
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.moved_callback = Some(callback);
        }
    }

    fn on_window_state_change(&self, callback: Box<dyn FnMut()>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.window_state_change_callback = Some(callback);
        }
    }

    fn on_window_mutation_observation(
        &self,
        callback: Box<dyn FnMut(PlatformWindowMutationObservation)>,
    ) {
        let mut state = self.0.lock();
        if !state.closed {
            state.mutation_observation_callback = Some(callback);
        }
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.should_close_handler = Some(callback);
        }
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.close_callback = Some(callback);
        }
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        let mut state = self.0.lock();
        if !state.closed {
            state.hit_test_window_control_callback = Some(callback);
        }
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn draw(&self, _scene: &Scene) -> PlatformWindowPresentOutcome {
        let mut state = self.0.lock();
        state.draw_count += 1;
        if std::mem::take(&mut state.reveal_on_next_present) {
            state.mapped = true;
        }
        let close_on_next_present = std::mem::take(&mut state.close_on_next_present);
        let outcome = state.present_outcome;
        drop(state);
        if close_on_next_present {
            self.close();
        }
        outcome
    }

    fn sprite_atlas(&self) -> sync::Arc<dyn crate::PlatformAtlas> {
        self.0.lock().sprite_atlas.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(&self, scene: &Scene) -> anyhow::Result<RgbaImage> {
        let mut state = self.0.lock();
        let size = state.bounds.size;
        let scale_factor = state.scale_factor;
        if let Some(renderer) = &mut state.renderer {
            let device_size: Size<DevicePixels> = size.to_device_pixels(scale_factor);
            renderer.render_scene_to_image(scene, device_size)
        } else {
            anyhow::bail!("render_to_image not available: no HeadlessRenderer configured")
        }
    }

    fn as_test(&mut self) -> Option<&mut TestWindow> {
        Some(self)
    }

    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        unimplemented!()
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.0.lock().ime_position_history.push(bounds);
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    fn a11y_init(&self, callbacks: A11yCallbacks) {
        let mut state = self.0.lock();
        if state.closed {
            return;
        }
        debug_assert!(
            state.accessibility.callbacks.is_none(),
            "accessibility callbacks initialized more than once for a test window"
        );
        state.accessibility.callbacks = Some(Rc::new(callbacks));
        state.accessibility.active = false;
        state.accessibility.updates.clear();
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        self.0
            .lock()
            .accessibility
            .record_platform_delivery(tree_update);
    }
}

fn window_platform_facts(state: &TestWindowState) -> WindowPlatformFacts {
    let physical_geometry = test_physical_geometry(state);
    WindowPlatformFacts {
        bounds: state.bounds,
        coordinate_space: crate::WindowCoordinateSpace::GlobalScreen,
        physical_geometry,
        window_bounds: state.window_bounds,
        inner_window_bounds: state.window_bounds,
        content_size: state.bounds.size,
        scale_factor: state.scale_factor,
        display_id: state
            .physical_display_observation
            .map(PlatformPhysicalDisplayObservation::display_id)
            .or_else(|| Some(state.display.id())),
        is_minimized: state.is_minimized,
        is_maximized: state.is_maximized,
        is_fullscreen: state.is_fullscreen,
        accepts_pointer_input: state.accepts_pointer_input,
        accepts_activation: state.accepts_activation,
        focus_on_click: state.focus_on_click,
        background_appearance: state.background_appearance,
        topmost: state.topmost,
        taskbar_visible: state.taskbar_visible,
        is_active: state.is_active,
    }
}

fn apply_test_window_mutation(state: &mut TestWindowState, request: WindowMutationRequest) {
    match request {
        WindowMutationRequest::Placement(request) => {
            if let Some(position) = request.position {
                state.bounds.origin = position;
            }
            if let Some(size) = request.size {
                state.bounds.size = size;
            }
            if let Some(placement_state) = request.state {
                match placement_state {
                    WindowPlacementState::Windowed => {
                        state.is_minimized = false;
                        state.is_maximized = false;
                        state.is_fullscreen = false;
                        state.window_bounds = WindowBounds::Windowed(state.bounds);
                    }
                    WindowPlacementState::Maximized => {
                        state.is_minimized = false;
                        state.is_maximized = true;
                        state.is_fullscreen = false;
                        state.window_bounds = WindowBounds::Maximized(state.bounds);
                    }
                    WindowPlacementState::Fullscreen => {
                        state.is_minimized = false;
                        state.is_maximized = false;
                        state.is_fullscreen = true;
                        state.window_bounds = WindowBounds::Fullscreen(state.bounds);
                    }
                    WindowPlacementState::Minimized => {
                        state.is_minimized = true;
                    }
                }
            }
            if let Some(restore_bounds) = request.restore_bounds {
                state.window_bounds = if state.is_fullscreen {
                    WindowBounds::Fullscreen(restore_bounds)
                } else if state.is_maximized {
                    WindowBounds::Maximized(restore_bounds)
                } else {
                    match state.window_bounds {
                        WindowBounds::Windowed(_) => WindowBounds::Windowed(restore_bounds),
                        WindowBounds::Maximized(_) => WindowBounds::Maximized(restore_bounds),
                        WindowBounds::Fullscreen(_) => WindowBounds::Fullscreen(restore_bounds),
                    }
                };
            } else if !state.is_minimized && !state.is_maximized && !state.is_fullscreen {
                state.window_bounds = WindowBounds::Windowed(state.bounds);
            }
        }
        WindowMutationRequest::PhysicalPlacement(request) => {
            let physical_bounds = request.client_bounds();
            if let Some(target_display) = request.target_display() {
                state.scale_factor = target_display.scale_factor();
                state.physical_display_observation = Some(target_display);
            }
            let bounds = physical_bounds.to_pixels(state.scale_factor);
            state.physical_client_bounds = Some(physical_bounds);
            state.bounds = bounds;
            state.is_minimized = false;
            state.is_maximized = false;
            state.is_fullscreen = false;
            state.window_bounds = WindowBounds::Windowed(bounds);
        }
        WindowMutationRequest::PointerInput(accepts_pointer_input) => {
            state.accepts_pointer_input = accepts_pointer_input;
        }
        WindowMutationRequest::ActivationPolicy(policy) => {
            state.accepts_activation = policy.accepts_activation;
            state.focus_on_click = policy.focus_on_click;
        }
        WindowMutationRequest::Alpha(background) => {
            state.background_appearance = background;
        }
        WindowMutationRequest::Topmost(topmost) => {
            state.topmost = topmost;
        }
        WindowMutationRequest::TaskbarVisibility(visible) => {
            state.taskbar_visible = visible;
        }
    }
}

fn apply_window_platform_facts(state: &mut TestWindowState, facts: &WindowPlatformFacts) {
    state.bounds = facts.bounds;
    state.physical_client_bounds = facts
        .physical_geometry
        .map(PlatformWindowPhysicalGeometry::client_bounds);
    state.physical_display_observation = facts
        .physical_geometry
        .and_then(PlatformWindowPhysicalGeometry::display_observation);
    state.scale_factor = facts.scale_factor;
    state.window_bounds = facts.window_bounds;
    state.is_minimized = facts.is_minimized;
    state.is_maximized = facts.is_maximized;
    state.is_fullscreen = facts.is_fullscreen;
    state.is_active = facts.is_active;
    state.accepts_pointer_input = facts.accepts_pointer_input;
    state.accepts_activation = facts.accepts_activation;
    state.focus_on_click = facts.focus_on_click;
    state.background_appearance = facts.background_appearance;
    state.topmost = facts.topmost;
    state.taskbar_visible = facts.taskbar_visible;
}

fn normalize_accessibility_tree_update(mut update: accesskit::TreeUpdate) -> accesskit::TreeUpdate {
    update.nodes.sort_unstable_by_key(|(id, _)| *id);
    update
}

#[cfg(test)]
mod window_mutation_tests {
    use super::*;
    use crate::{
        AppContext, Context, DisplayId, Empty, InteractiveElement, IntoElement, Modifiers,
        MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement,
        PlatformWindowCreationCapabilities, PlatformWindowMutationCapabilities, QuitMode, Render,
        Styled, Subscription, TestAppContext, Window, WindowActivationPolicy,
        WindowCreationSupport, WindowInitialPresentationOrder, WindowInitialPresentationStatus,
        WindowKind, WindowMouseEvent, WindowMutationDispatch, WindowMutationOutcome,
        WindowMutationSupport, WindowMutationTicket, WindowOpenFailureStage, WindowOptions,
        WindowPlacementRequest, WindowProvisionalSemanticsTicket, WindowProvisionalSession, canvas,
        div, point, px, size,
    };
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(Point::new(px(x), px(y)), size(px(width), px(height)))
    }

    fn open_test_window(cx: &mut TestAppContext) -> (AnyWindowHandle, TestWindow) {
        let handle = cx.open_window(size(px(320.0), px(240.0)), |_, _| Empty);
        let handle = handle.into();
        let platform_window = cx.test_window(handle);
        platform_window.clear_platform_command_history();
        (handle, platform_window)
    }

    fn open_revealed_provisional_window(
        cx: &mut TestAppContext,
        generation: u64,
    ) -> (WindowProvisionalSession, AnyWindowHandle, TestWindow) {
        let session = WindowProvisionalSession::new(generation)
            .expect("the provisional generation should be valid");
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| PaintedRoot),
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        let reveal = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            reveal.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );
        (session, handle, platform_window)
    }

    fn provisional_placement_request(generation: u64) -> WindowProvisionalPlacementRequest {
        WindowProvisionalPlacementRequest::try_new(
            generation,
            WindowProvisionalPlacementPurpose::FinalRelease,
            Bounds::new(
                point(DevicePixels(80), DevicePixels(90)),
                size(DevicePixels(360), DevicePixels(260)),
            ),
            point(DevicePixels(120), DevicePixels(130)),
            PlatformPhysicalDisplayObservation::try_new(
                1,
                DisplayId::from(1),
                Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(3_840), DevicePixels(2_160)),
                ),
                Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(3_840), DevicePixels(2_080)),
                ),
                2.0,
            )
            .expect("the test target display should be representable"),
        )
        .expect("the test final placement should be structurally valid")
    }

    fn settle_provisional_final_placement(
        cx: &mut TestAppContext,
        handle: AnyWindowHandle,
        session: &WindowProvisionalSession,
    ) {
        let (dispatch, ticket) = cx
            .update_window(handle, |_, window, _| {
                window.request_provisional_placement(session, provisional_placement_request(1))
            })
            .expect("the provisional window should remain live")
            .expect("the revealed session should admit final placement");
        assert!(matches!(dispatch, WindowMutationDispatch::Queued(_)));
        assert_eq!(
            ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::Pending,
            "dispatch alone must not manufacture native placement evidence"
        );
        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));

        let placement = ticket.snapshot();
        assert_eq!(
            placement.purpose(),
            WindowProvisionalPlacementPurpose::FinalRelease
        );
        assert_eq!(
            placement.outcome(),
            WindowProvisionalPlacementOutcome::Settled
        );
        assert!(
            placement
                .native_facts()
                .is_some_and(|facts| facts.accepts_placement()),
            "final placement must retain exact accepted native facts"
        );
    }

    #[crate::test]
    fn physical_client_geometry_converts_through_the_window_scale(cx: &mut TestAppContext) {
        let (window, platform_window) = open_test_window(cx);
        platform_window.0.lock().bounds = bounds(-100.0, 50.0, 320.0, 240.0);

        assert_eq!(platform_window.physical_geometry(), None);

        cx.set_platform_window_physical_client_geometry(
            window,
            Some(Bounds::new(
                point(DevicePixels(-1200), DevicePixels(700)),
                size(DevicePixels(480), DevicePixels(360)),
            )),
            1.5,
        );

        let geometry = platform_window
            .physical_geometry()
            .expect("explicit test physical geometry should be observable");
        assert_eq!(
            geometry.client_bounds(),
            Bounds::new(
                point(DevicePixels(-1200), DevicePixels(700)),
                size(DevicePixels(480), DevicePixels(360)),
            )
        );
        assert_eq!(
            geometry.local_to_global(point(px(10.0), px(20.0))),
            Some(point(DevicePixels(-1185), DevicePixels(730)))
        );
        assert_eq!(
            geometry.global_to_local(point(DevicePixels(-1185), DevicePixels(730))),
            Some(point(px(10.0), px(20.0)))
        );
        assert_eq!(geometry.local_to_global(point(px(f32::NAN), px(0.0))), None);
        assert_eq!(geometry.local_to_global(point(px(f32::MAX), px(0.0))), None);
        assert_eq!(
            PlatformWindowPhysicalGeometry::try_new(
                Bounds::new(
                    point(DevicePixels(i32::MAX), DevicePixels(0)),
                    size(DevicePixels(1), DevicePixels(1)),
                ),
                1.0,
            ),
            None,
            "client edges that overflow physical coordinates must fail closed"
        );
        let extreme = PlatformWindowPhysicalGeometry::try_new(
            Bounds::new(
                point(DevicePixels(i32::MIN), DevicePixels(i32::MIN)),
                size(DevicePixels(1), DevicePixels(1)),
            ),
            1.0,
        )
        .expect("extreme integer origins remain representable");
        assert_eq!(
            extreme.global_to_local(point(DevicePixels(i32::MAX), DevicePixels(i32::MAX))),
            Some(point(px(u32::MAX as f32), px(u32::MAX as f32)))
        );
    }

    struct WindowControlView;

    impl Render for WindowControlView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .window_control_area(WindowControlArea::Close)
        }
    }

    struct PaintedRoot;

    impl Render for PaintedRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(crate::white())
        }
    }

    type ProvisionalSemanticsAuthority =
        Rc<RefCell<Option<(WindowProvisionalSession, WindowProvisionalSemanticsTicket)>>>;

    struct ProvisionalSemanticsMarkerRoot {
        authority: ProvisionalSemanticsAuthority,
    }

    impl Render for ProvisionalSemanticsMarkerRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let authority = self.authority.clone();
            div().size_full().bg(crate::white()).child(
                canvas(
                    move |_, window, _| {
                        let authority = authority.clone();
                        window.record_prepaint_focus_stable_commit(move |frame, window, app| {
                            let Some((session, ticket)) = authority.borrow_mut().take() else {
                                return;
                            };
                            window
                                .accept_provisional_destination_semantics_frame(
                                    &session, &ticket, frame, app,
                                )
                                .expect("the exact marker should accept its candidate frame");
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
        }
    }

    struct InteractiveProvisionalRoot {
        mouse_downs: Rc<Cell<usize>>,
        semantics_authority: ProvisionalSemanticsAuthority,
    }

    impl Render for InteractiveProvisionalRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let mouse_downs = self.mouse_downs.clone();
            let semantics_authority = self.semantics_authority.clone();
            div()
                .size_full()
                .bg(crate::white())
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    mouse_downs.set(mouse_downs.get().saturating_add(1));
                })
                .child(
                    canvas(
                        move |_, window, _| {
                            let semantics_authority = semantics_authority.clone();
                            window.record_prepaint_focus_stable_commit(
                                move |frame, window, app| {
                                    let Some((session, ticket)) =
                                        semantics_authority.borrow_mut().take()
                                    else {
                                        return;
                                    };
                                    window
                                        .accept_provisional_destination_semantics_frame(
                                            &session, &ticket, frame, app,
                                        )
                                        .expect(
                                            "the exact interactive marker should accept its candidate frame",
                                        );
                                },
                            );
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
        }
    }

    fn open_revealed_provisional_semantics_window(
        cx: &mut TestAppContext,
        generation: u64,
    ) -> (
        WindowProvisionalSession,
        AnyWindowHandle,
        TestWindow,
        ProvisionalSemanticsAuthority,
    ) {
        let session = WindowProvisionalSession::new(generation)
            .expect("the provisional generation should be valid");
        let semantics_authority = Rc::new(RefCell::new(None));
        let handle: AnyWindowHandle = cx
            .update(|app| {
                let semantics_authority = semantics_authority.clone();
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| {
                        app.new(|_| ProvisionalSemanticsMarkerRoot {
                            authority: semantics_authority,
                        })
                    },
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        let reveal = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            reveal.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );
        settle_provisional_final_placement(cx, handle, &session);
        (session, handle, platform_window, semantics_authority)
    }

    struct InitialPresentationObserverProbe {
        _subscription: Subscription,
    }

    impl InitialPresentationObserverProbe {
        fn new(
            window: &mut Window,
            observations: Rc<RefCell<Vec<WindowInitialPresentationStatus>>>,
            cx: &mut Context<Self>,
        ) -> Self {
            let subscription =
                cx.observe_window_initial_presentation(window, move |_, window, _| {
                    observations
                        .borrow_mut()
                        .push(window.presentation_facts().initial_presentation);
                });
            Self {
                _subscription: subscription,
            }
        }
    }

    impl Render for InitialPresentationObserverProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            Empty
        }
    }

    #[crate::test]
    fn transient_owner_token_is_live_generation_and_application_bound(cx: &mut TestAppContext) {
        let owner: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        let owner_token = cx
            .read(|app| app.transient_window_owner(owner))
            .expect("a committed window should produce an owner token");
        let child: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        transient_for: Some(owner_token.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| Empty),
                )
            })
            .expect("the live same-application owner should be accepted")
            .into();
        assert_eq!(
            cx.update_window(child, |_, window, _| {
                window.creation_facts().transient_for
            })
            .expect("the child should remain live"),
            Some(owner)
        );

        cx.update_window(owner, |_, window, app| window.remove_window(app))
            .expect("the owner should close");
        let replacement: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        assert_ne!(
            replacement, owner,
            "a reused slot must carry a new generation"
        );
        let stale_result: anyhow::Result<crate::WindowHandle<Empty>> = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    transient_for: Some(owner_token),
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        });
        assert!(
            stale_result
                .expect_err("a closed owner generation must be rejected")
                .to_string()
                .contains("closed or its generation is stale")
        );

        let mut foreign = TestAppContext::single();
        let foreign_owner: AnyWindowHandle = foreign
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        let foreign_token = foreign
            .read(|app| app.transient_window_owner(foreign_owner))
            .expect("the foreign application should create its own token");
        let foreign_result: anyhow::Result<crate::WindowHandle<Empty>> = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    transient_for: Some(foreign_token),
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        });
        assert!(
            foreign_result
                .expect_err("an owner token from another application must be rejected")
                .to_string()
                .contains("different application")
        );
    }

    #[crate::test]
    fn transient_owner_rejects_the_exact_window_before_native_creation(cx: &mut TestAppContext) {
        let owner: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        let owner_token = cx
            .read(|app| app.transient_window_owner(owner))
            .expect("a committed window should produce an owner token");
        let native_before = cx
            .last_created_test_window()
            .expect("the owner should be the latest native test window");

        let result = cx.update(|app| {
            Window::new(
                owner,
                WindowOptions {
                    transient_for: Some(owner_token),
                    ..Default::default()
                },
                crate::window::WindowInteractionAuthority::new(),
                app,
            )
        });
        let error = match result {
            Ok(_) => panic!("a top-level window must reject itself as transient owner"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("cannot be transient for itself"));

        let native_after = cx
            .last_created_test_window()
            .expect("self-owner rejection must preserve the previous native window");
        assert!(
            Rc::ptr_eq(&native_before.0, &native_after.0),
            "self-owner validation must fail before opening another native window"
        );
    }

    #[crate::test]
    fn unsupported_transient_owner_is_rejected_before_native_creation(cx: &mut TestAppContext) {
        cx.set_platform_window_creation_capabilities(PlatformWindowCreationCapabilities {
            focus_on_appearing: WindowCreationSupport::Supported,
            transient_for: WindowCreationSupport::Unsupported,
            provisional_presentation: WindowCreationSupport::Supported,
            initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
        });
        let owner: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        let owner = cx
            .read(|app| app.transient_window_owner(owner))
            .expect("the owner token itself is backend-independent");
        let result: anyhow::Result<crate::WindowHandle<Empty>> = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    transient_for: Some(owner),
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        });

        assert!(
            result
                .expect_err("unsupported ownership must not be silently ignored")
                .to_string()
                .contains("does not support transient top-level owners")
        );
    }

    #[crate::test]
    fn presentation_facts_distinguish_submitted_empty_and_non_empty_frames(
        cx: &mut TestAppContext,
    ) {
        let empty: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        let empty_facts = cx
            .update_window(empty, |_, window, _| window.presentation_facts())
            .expect("the empty window should remain live");
        assert!(empty_facts.native_created);
        assert!(empty_facts.native_visible);
        assert_eq!(
            empty_facts.present_submitted_generation,
            empty_facts.frame_accepted_generation
        );
        assert_eq!(
            empty_facts
                .latest_present_attempt
                .expect("the first present attempt should be observable")
                .outcome,
            PlatformWindowPresentOutcome::Submitted
        );
        assert_eq!(empty_facts.non_empty_presented_generation, None);

        let painted: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| PaintedRoot)
            .into();
        let painted_facts = cx
            .update_window(painted, |_, window, _| window.presentation_facts())
            .expect("the painted window should remain live");
        assert_eq!(
            painted_facts.non_empty_presented_generation,
            painted_facts.frame_accepted_generation
        );
        assert_eq!(
            painted_facts.present_submitted_generation,
            painted_facts.frame_accepted_generation
        );
    }

    #[crate::test]
    fn presentation_facts_read_current_native_visibility_without_a_present(
        cx: &mut TestAppContext,
    ) {
        let handle: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
            .into();
        let platform_window = cx.test_window(handle);
        platform_window.set_visible_for_test(false);

        let facts = cx
            .update_window(handle, |_, window, _| window.presentation_facts())
            .expect("the test window should remain live");
        assert!(
            !facts.native_visible,
            "a native hide must not require another presentation to become observable"
        );
    }

    #[crate::test]
    fn visible_during_hidden_initial_present_rolls_back_window_creation(cx: &mut TestAppContext) {
        let result: anyhow::Result<crate::WindowHandle<PaintedRoot>> = cx.update(|app| {
            app.open_window(WindowOptions::default(), |window, app| {
                window
                    .platform_window
                    .as_test()
                    .expect("the test backend should expose TestWindow")
                    .reveal_on_next_present();
                app.new(|_| PaintedRoot)
            })
        });

        assert!(
            result
                .expect_err("visibility during a hidden first present must fail creation")
                .to_string()
                .contains("became visible during its hidden first presentation")
        );
    }

    #[crate::test]
    fn mismatched_creation_visibility_fact_is_rejected_before_root_builder(
        cx: &mut TestAppContext,
    ) {
        cx.set_next_window_creation_show_fact(false);
        let root_builder_ran = Rc::new(Cell::new(false));
        let result: anyhow::Result<crate::WindowHandle<Empty>> = cx.update(|app| {
            app.open_window(WindowOptions::default(), {
                let root_builder_ran = root_builder_ran.clone();
                move |_, app| {
                    root_builder_ran.set(true);
                    app.new(|_| Empty)
                }
            })
        });

        assert!(
            result
                .expect_err("a false creation visibility fact must not commit")
                .to_string()
                .contains("did not preserve the requested initial visibility")
        );
        assert!(!root_builder_ran.get());
    }

    #[crate::test]
    fn rejected_or_deferred_initial_present_rolls_back_window_creation(cx: &mut TestAppContext) {
        for outcome in [
            PlatformWindowPresentOutcome::Deferred,
            PlatformWindowPresentOutcome::Rejected,
        ] {
            let reserved_id = Rc::new(Cell::new(None));
            let result: anyhow::Result<crate::WindowHandle<Empty>> = cx.update(|app| {
                app.open_window(WindowOptions::default(), {
                    let reserved_id = reserved_id.clone();
                    move |window, app| {
                        reserved_id.set(Some(Window::window_handle(window).window_id()));
                        window
                            .platform_window
                            .as_test()
                            .expect("the test backend should expose TestWindow")
                            .set_present_outcome(outcome);
                        app.new(|_| Empty)
                    }
                })
            });
            assert!(
                result
                    .expect_err("an unsubmitted initial frame must fail creation")
                    .to_string()
                    .contains("rejected or deferred")
            );
            let reserved_id = reserved_id
                .get()
                .expect("the root builder should observe its reservation");
            assert!(
                cx.read(|app| app.window_handles.get(&reserved_id).copied())
                    .is_none()
            );
        }
    }

    #[crate::test]
    fn app_retains_actual_window_profile_through_updates_and_close(cx: &mut TestAppContext) {
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        kind: WindowKind::Floating,
                        ..Default::default()
                    },
                    |_, app| app.new(|_| Empty),
                )
            })
            .expect("floating test window should open")
            .into();
        let expected_capabilities = cx
            .update_window(handle, |_, window, _| window.window_capabilities())
            .expect("floating test window should remain live");

        let profile = cx
            .read(|app| app.window_profile(handle).cloned())
            .expect("opened window should have a platform profile");
        assert_eq!(profile.kind, WindowKind::Floating);
        assert_eq!(profile.capabilities, expected_capabilities);
        assert_eq!(
            cx.update_window(handle, |_, _, app| { app.window_profile(handle).cloned() })
                .expect("profile should remain readable while the window is being updated"),
            Some(profile)
        );

        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .expect("floating test window should close");
        assert!(
            cx.read(|app| app.window_profile(handle).is_none()),
            "closed windows must not retain stale platform profiles"
        );
    }

    #[crate::test]
    fn unavailable_display_uses_one_snapshot_primary_before_window_creation(
        cx: &mut TestAppContext,
    ) {
        let unavailable_display = DisplayId::from(999);
        let default_display = cx.read(|app| {
            app.primary_display()
                .expect("test platform should expose a primary display")
                .id()
        });
        cx.reset_platform_display_snapshot_query_count();
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        display_id: Some(unavailable_display),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| Empty),
                )
            })
            .expect("window with an unavailable display should fall back to the default display")
            .into();
        let platform_window = cx.test_window(handle);

        assert_eq!(platform_window.requested_display_id(), None);
        assert_eq!(
            cx.platform_display_snapshot_query_count(),
            1,
            "window resolution, default bounds, capabilities, and platform creation must share one display publication"
        );
        assert_eq!(
            cx.update_window(handle, |_, window, _| {
                window.platform_facts().display_id
            })
            .expect("fallback test window should remain open"),
            Some(default_display)
        );
    }

    fn queue_placement(
        cx: &mut TestAppContext,
        handle: AnyWindowHandle,
        request: WindowPlacementRequest,
    ) -> WindowMutationTicket {
        cx.update_window(handle, |_, window, _| {
            match window.request_window_placement_request(request) {
                WindowMutationDispatch::Queued(ticket) => ticket,
                dispatch => panic!("expected queued placement dispatch, got {dispatch:?}"),
            }
        })
        .expect("test window should be open")
    }

    #[crate::test]
    fn queued_placement_keeps_getters_stable_until_exact_terminal_observation(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open_test_window(cx);
        let initial_bounds = cx
            .update_window(handle, |_, window, _| window.bounds())
            .unwrap();
        let requested_bounds = bounds(24.0, 36.0, 500.0, 300.0);

        let ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(requested_bounds),
        );

        assert_eq!(
            cx.update_window(handle, |_, window, _| window.bounds())
                .unwrap(),
            initial_bounds,
            "a queued backend request must not mutate the committed facts cache"
        );
        assert!(ticket.observation().is_none());

        let intermediate_size = size(px(340.0), px(220.0));
        cx.simulate_window_resize(handle, intermediate_size);
        assert_eq!(
            cx.update_window(handle, |_, window, _| window.bounds())
                .unwrap(),
            Bounds::new(initial_bounds.origin, intermediate_size),
            "ordinary platform resize notifications still refresh committed facts"
        );
        assert!(
            ticket.observation().is_none(),
            "an intermediate resize callback must not settle a queued placement"
        );

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));

        assert_eq!(
            cx.update_window(handle, |_, window, _| window.bounds())
                .unwrap(),
            requested_bounds
        );
        assert_eq!(
            ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
    }

    #[crate::test]
    fn terminal_observation_distinguishes_adjustment_from_async_rejection(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(50.0, 60.0, 450.0, 280.0)),
        );
        let adjusted_bounds = bounds(55.0, 65.0, 440.0, 270.0);
        let mut adjusted_facts = platform_window.platform_facts();
        adjusted_facts.bounds = adjusted_bounds;
        adjusted_facts.content_size = adjusted_bounds.size;
        adjusted_facts.window_bounds = WindowBounds::Windowed(adjusted_bounds);
        adjusted_facts.inner_window_bounds = WindowBounds::Windowed(adjusted_bounds);

        assert!(cx.simulate_window_mutation_observation(
            handle,
            WindowMutationDomain::Placement,
            adjusted_facts,
        ));
        assert_eq!(
            ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Adjusted
        );

        let rejected_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(80.0, 90.0, 420.0, 260.0)),
        );
        let rejected_facts = platform_window.platform_facts();
        assert!(cx.simulate_window_mutation_terminal(
            handle,
            WindowMutationDomain::Placement,
            PlatformWindowMutationTerminal::Rejected,
            rejected_facts,
        ));
        assert_eq!(
            rejected_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Rejected,
            "a backend error must never be collapsed into an adjusted observation"
        );

        let unsupported_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(100.0, 110.0, 400.0, 240.0)),
        );
        let unsupported_facts = platform_window.platform_facts();
        assert!(cx.simulate_window_mutation_terminal(
            handle,
            WindowMutationDomain::Placement,
            PlatformWindowMutationTerminal::Unsupported,
            unsupported_facts,
        ));
        assert_eq!(
            unsupported_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Unsupported
        );
    }

    #[crate::test]
    fn physical_placement_exactness_includes_the_target_display_observation(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        let request = provisional_placement_request(1).physical_request();
        let ticket = cx
            .update_window(handle, |_, window, _| {
                match window.request_physical_window_placement(request) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected queued physical placement, got {dispatch:?}"),
                }
            })
            .expect("the physical placement test window should remain live");

        let target_display = request
            .target_display()
            .expect("the physical placement request should remain display-bound");
        let stale_display = PlatformPhysicalDisplayObservation::try_new(
            target_display.topology_generation() + 1,
            target_display.display_id(),
            target_display.bounds(),
            target_display.visible_bounds(),
            target_display.scale_factor(),
        )
        .expect("the stale display observation should remain structurally valid");
        let stale_geometry = PlatformWindowPhysicalGeometry::try_new(
            request.client_bounds(),
            target_display.scale_factor(),
        )
        .and_then(|geometry| geometry.with_display_observation(stale_display))
        .expect("the stale physical geometry should remain structurally valid");
        let mut facts = platform_window.platform_facts();
        facts.physical_geometry = Some(stale_geometry);

        assert!(cx.simulate_window_mutation_observation(
            handle,
            WindowMutationDomain::Placement,
            facts,
        ));
        assert_eq!(
            ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Adjusted,
            "matching client bounds from a different display publication must not settle exact",
        );
    }

    #[crate::test]
    fn mutation_terminal_queued_while_app_is_borrowed_settles_after_update(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        let ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(50.0, 60.0, 450.0, 280.0)),
        );
        let rejected_facts = platform_window.platform_facts();

        cx.update_window(handle, |_, _, _| {
            assert!(platform_window.simulate_window_mutation_terminal(
                WindowMutationDomain::Placement,
                PlatformWindowMutationTerminal::Rejected,
                rejected_facts,
            ));
            assert!(
                ticket.observation().is_none(),
                "a reentrant native callback must not dispatch while the app update is active"
            );
        })
        .unwrap();

        cx.run_until_parked();

        assert_eq!(
            ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Rejected,
            "the queued terminal observation must settle after the app update releases its borrow"
        );
    }

    #[crate::test]
    fn native_close_queued_while_app_is_borrowed_removes_window_once(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let close_count = Rc::new(Cell::new(0));
        let close_count_for_observer = close_count.clone();
        let _subscription = cx.update(|app| {
            app.on_window_closed(move |_, closed_window| {
                assert_eq!(closed_window, handle.window_id());
                close_count_for_observer.set(close_count_for_observer.get() + 1);
            })
        });

        cx.update_window(handle, |_, _, _| {
            assert!(platform_window.simulate_close());
            assert_eq!(
                close_count.get(),
                0,
                "native close must not reenter App while an update owns it"
            );
        })
        .unwrap();

        cx.run_until_parked();

        assert!(
            !cx.windows().contains(&handle),
            "the queued native close must remove the committed window"
        );
        assert_eq!(
            close_count.get(),
            1,
            "the terminal native close must notify observers exactly once"
        );
    }

    #[crate::test]
    fn gpui_owned_async_window_update_waits_for_outer_app_borrow(cx: &mut TestAppContext) {
        let (handle, _) = open_test_window(cx);
        cx.run_until_parked();
        let completed = Rc::new(Cell::new(false));
        let dispatcher = cx.dispatcher.clone();

        cx.update_window(handle, |_, _, app| {
            app.spawn({
                let completed = completed.clone();
                async move |cx| {
                    cx.update_window_when_available(handle, move |_, _, _| {
                        completed.set(true);
                    })
                    .await
                    .expect("the queued window update must complete after App is released");
                }
            })
            .detach();

            assert!(
                dispatcher.tick(false),
                "the foreground task must be polled while the outer App borrow is active"
            );
            assert!(
                !completed.get(),
                "a borrow-conflicted foreground update must wait instead of being discarded"
            );
        })
        .expect("test window should remain live");

        cx.run_until_parked();
        assert!(
            completed.get(),
            "the queued foreground update must resume after AppRefMut is dropped"
        );
    }

    #[crate::test]
    fn programmatic_remove_emits_native_terminal_and_retires_should_close(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let should_close_calls = Rc::new(Cell::new(0usize));
        cx.update_window(handle, |_, window, app| {
            let should_close_calls = should_close_calls.clone();
            window.on_window_should_close(app, move |_, _| {
                should_close_calls.set(should_close_calls.get().saturating_add(1));
                false
            });
        })
        .expect("test window should remain live");
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .expect("test window should remain live until programmatic removal");
        cx.run_until_parked();

        assert!(!cx.windows().contains(&handle));
        assert_eq!(platform_window.simulate_should_close(), None);
        assert_eq!(should_close_calls.get(), 0);
        assert!(
            !platform_window.simulate_close(),
            "the owning TestWindow drop must consume the native close callback exactly once"
        );
        let diagnostics = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        let native_terminals = diagnostics
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == crate::NativeBoundaryKind::Callback(crate::NativeCallbackKind::Closed)
            })
            .collect::<Vec<_>>();
        assert_eq!(native_terminals.len(), 1);
        assert_eq!(
            native_terminals[0].disposition,
            crate::NativeBoundaryDisposition::Closed
        );
    }

    #[crate::test]
    fn close_request_prevents_native_reentry_and_queues_handler_while_app_is_borrowed(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        let handler_count = Rc::new(Cell::new(0));
        let handler_count_for_callback = handler_count.clone();
        cx.update_window(handle, |_, window, app| {
            window.on_window_should_close(app, move |_, _| {
                handler_count_for_callback.set(handler_count_for_callback.get() + 1);
                true
            });
        })
        .unwrap();

        cx.update_window(handle, |_, _, _| {
            assert_eq!(
                platform_window.simulate_should_close(),
                Some(false),
                "an App-busy close query must synchronously prevent native destruction"
            );
            assert_eq!(
                handler_count.get(),
                0,
                "the close handler must wait until the active App update completes"
            );
        })
        .unwrap();

        cx.run_until_parked();

        assert!(
            !cx.windows().contains(&handle),
            "an approved queued close intent must remove the window"
        );
        assert_eq!(
            handler_count.get(),
            1,
            "the queued close handler must run exactly once"
        );
    }

    #[crate::test]
    fn should_close_handler_survives_panic_and_remains_protective(cx: &mut TestAppContext) {
        let (handle, _) = open_test_window(cx);
        let calls = Rc::new(Cell::new(0usize));
        cx.update_window(handle, |_, window, app| {
            let calls = calls.clone();
            window.on_window_should_close(app, move |_, _| {
                let call = calls.get().saturating_add(1);
                calls.set(call);
                if call == 1 {
                    panic!("injected should-close panic");
                }
                false
            });
        })
        .expect("test window should remain live");

        let first = catch_unwind(AssertUnwindSafe(|| {
            cx.app.dispatch_window_should_close(handle.window_id())
        }));
        assert!(first.is_err());
        assert!(
            !cx.app.dispatch_window_should_close(handle.window_id()),
            "a panicking close query must retain its previous protective policy"
        );
        assert_eq!(calls.get(), 2);
        assert!(cx.windows().contains(&handle));
    }

    #[crate::test]
    fn should_close_handler_reentrant_replacement_wins(cx: &mut TestAppContext) {
        let (handle, _) = open_test_window(cx);
        let old_calls = Rc::new(Cell::new(0usize));
        let replacement_calls = Rc::new(Cell::new(0usize));
        cx.update_window(handle, |_, window, app| {
            let old_calls = old_calls.clone();
            let replacement_calls = replacement_calls.clone();
            window.on_window_should_close(app, move |window, app| {
                old_calls.set(old_calls.get().saturating_add(1));
                let replacement_calls = replacement_calls.clone();
                window.on_window_should_close(app, move |_, _| {
                    replacement_calls.set(replacement_calls.get().saturating_add(1));
                    false
                });
                false
            });
        })
        .expect("test window should remain live");

        assert!(!cx.app.dispatch_window_should_close(handle.window_id()));
        assert!(!cx.app.dispatch_window_should_close(handle.window_id()));
        assert_eq!(old_calls.get(), 1);
        assert_eq!(
            replacement_calls.get(),
            1,
            "the callback-installed replacement must not be overwritten by the old registration"
        );
    }

    #[crate::test]
    fn panicking_input_transaction_commits_pending_window_removal_once(cx: &mut TestAppContext) {
        let (handle, mut platform_window) = open_test_window(cx);
        let close_count = Rc::new(Cell::new(0usize));
        let _close_subscription = cx.update(|app| {
            let close_count = close_count.clone();
            app.on_window_closed(move |_, closed_window| {
                assert_eq!(closed_window, handle.window_id());
                close_count.set(close_count.get().saturating_add(1));
            })
        });
        let _input_interceptor = cx
            .update_window(handle, |_, window, _| {
                window.intercept_window_mouse_events(|event, window, app| {
                    if matches!(event, WindowMouseEvent::Down(_)) {
                        window.remove_window(app);
                        panic!("injected input callback panic after window removal");
                    }
                })
            })
            .expect("test window should remain live");

        let result = catch_unwind(AssertUnwindSafe(|| {
            platform_window.simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(12.0), px(18.0)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }))
        }));

        assert!(result.is_err());
        assert!(!cx.windows().contains(&handle));
        assert_eq!(close_count.get(), 1);
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;
        assert!(
            cx.app.dispatch_window_should_close(handle.window_id()),
            "window removal must also retire the native query snapshot before panic resumes"
        );
        let close_diagnostics = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        assert!(close_diagnostics.terminal.iter().any(|diagnostic| {
            diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                && diagnostic.kind
                    == crate::NativeBoundaryKind::Callback(crate::NativeCallbackKind::ShouldClose)
                && diagnostic.disposition == crate::NativeBoundaryDisposition::Closed
        }));
    }

    #[crate::test]
    fn panicking_pointer_cancel_listener_still_commits_terminal_window_removal(
        cx: &mut TestAppContext,
    ) {
        let (handle, mut platform_window) = open_test_window(cx);
        let close_count = Rc::new(Cell::new(0usize));
        let _close_subscription = cx.update(|app| {
            let close_count = close_count.clone();
            app.on_window_closed(move |_, closed_window| {
                assert_eq!(closed_window, handle.window_id());
                close_count.set(close_count.get().saturating_add(1));
            })
        });
        let _cancel_interceptor = cx
            .update_window(handle, |_, window, _| {
                window.intercept_window_mouse_events(|event, _, _| {
                    if matches!(event, WindowMouseEvent::Cancel(_)) {
                        panic!("injected pointer-cancel listener panic");
                    }
                })
            })
            .expect("test window should remain live");
        platform_window.simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(20.0), px(24.0)),
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        assert!(
            cx.update_window(handle, |_, window, app| {
                window.has_active_pointer_session(app)
            })
            .expect("test window should remain live")
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update_window(handle, |_, window, app| window.remove_window(app))
        }));

        assert!(result.is_err());
        assert!(!cx.windows().contains(&handle));
        assert_eq!(close_count.get(), 1);
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;
        assert!(cx.app.dispatch_window_should_close(handle.window_id()));
        let close_diagnostics = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        assert!(close_diagnostics.terminal.iter().any(|diagnostic| {
            diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                && diagnostic.kind
                    == crate::NativeBoundaryKind::Callback(crate::NativeCallbackKind::ShouldClose)
                && diagnostic.disposition == crate::NativeBoundaryDisposition::Closed
        }));
    }

    #[crate::test]
    fn panicking_window_mutation_ticket_does_not_skip_later_terminal_delivery(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open_test_window(cx);
        let placement_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(40.0, 50.0, 420.0, 260.0)),
        );
        let pointer_ticket = cx
            .update_window(handle, |_, window, _| {
                match window.request_pointer_input(false) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected queued pointer-input dispatch, got {dispatch:?}"),
                }
            })
            .expect("test window should remain live");
        let placement_deliveries = Rc::new(Cell::new(0usize));
        let pointer_deliveries = Rc::new(Cell::new(0usize));
        let _placement_subscription = placement_ticket.subscribe({
            let placement_deliveries = placement_deliveries.clone();
            move |_| {
                placement_deliveries.set(placement_deliveries.get().saturating_add(1));
                panic!("injected placement ticket observer panic");
            }
        });
        let _pointer_subscription = pointer_ticket.subscribe({
            let pointer_deliveries = pointer_deliveries.clone();
            move |_| pointer_deliveries.set(pointer_deliveries.get().saturating_add(1))
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update_window(handle, |_, window, app| window.remove_window(app))
        }));

        assert!(result.is_err());
        assert_eq!(placement_deliveries.get(), 1);
        assert_eq!(
            pointer_deliveries.get(),
            1,
            "a prior ticket observer panic must not skip a later settled ticket delivery"
        );
        assert_eq!(
            placement_ticket
                .observation()
                .expect("placement ticket must be terminal")
                .outcome,
            WindowMutationOutcome::WindowClosed
        );
        assert_eq!(
            pointer_ticket
                .observation()
                .expect("pointer-input ticket must be terminal")
                .outcome,
            WindowMutationOutcome::WindowClosed
        );
        assert!(!cx.windows().contains(&handle));
    }

    #[crate::test]
    fn panicking_window_mutation_observer_does_not_skip_sibling_observer(cx: &mut TestAppContext) {
        let (handle, _) = open_test_window(cx);
        let ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(40.0, 50.0, 420.0, 260.0)),
        );
        let first_deliveries = Rc::new(Cell::new(0usize));
        let second_deliveries = Rc::new(Cell::new(0usize));
        let _first_subscription = ticket.subscribe({
            let first_deliveries = first_deliveries.clone();
            move |_| {
                first_deliveries.set(first_deliveries.get().saturating_add(1));
                panic!("injected first ticket observer panic");
            }
        });
        let _second_subscription = ticket.subscribe({
            let second_deliveries = second_deliveries.clone();
            move |_| second_deliveries.set(second_deliveries.get().saturating_add(1))
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update_window(handle, |_, window, app| window.remove_window(app))
        }));

        assert!(result.is_err());
        assert_eq!(first_deliveries.get(), 1);
        assert_eq!(
            second_deliveries.get(),
            1,
            "one ticket observer panic must not skip a later observer for the same ticket"
        );
        assert_eq!(
            ticket
                .observation()
                .expect("placement ticket must be terminal")
                .outcome,
            WindowMutationOutcome::WindowClosed
        );
        assert!(!cx.windows().contains(&handle));
    }

    #[crate::test]
    fn window_control_hit_test_reads_committed_snapshot_while_app_is_borrowed(
        cx: &mut TestAppContext,
    ) {
        let handle: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(240.0)), |_, _| WindowControlView)
            .into();
        let mut platform_window = cx.test_window(handle);
        platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(10.0), px(10.0)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));

        assert_eq!(
            platform_window.simulate_window_control_hit_test(),
            Some(WindowControlArea::Close)
        );
        cx.update_window(handle, |_, _, _| {
            assert_eq!(
                platform_window.simulate_window_control_hit_test(),
                Some(WindowControlArea::Close),
                "native hit testing must not reborrow App while an update owns it"
            );
        })
        .unwrap();
    }

    #[crate::test]
    fn coalescible_native_facts_converge_after_borrowed_app_update(cx: &mut TestAppContext) {
        let (handle, mut platform_window) = open_test_window(cx);
        let final_size = size(px(510.0), px(330.0));

        cx.update_window(handle, |_, window, _| {
            platform_window.simulate_resize(size(px(420.0), px(280.0)));
            platform_window.simulate_resize(final_size);
            platform_window.simulate_active_status_change(true);
            platform_window.simulate_hover_status_change(true);

            assert_ne!(
                window.viewport_size(),
                final_size,
                "queued native facts must not reenter the active App update"
            );
            assert!(!window.is_window_active());
            assert!(!window.is_window_hovered());
        })
        .unwrap();

        cx.run_until_parked();

        cx.update_window(handle, |_, window, _| {
            assert_eq!(window.viewport_size(), final_size);
            assert!(window.is_window_active());
            assert!(window.is_window_hovered());
        })
        .unwrap();
    }

    #[crate::test]
    fn activation_edges_remain_ordered_while_app_is_borrowed(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        platform_window.simulate_active_status_change(true);
        cx.run_until_parked();

        let activation_count = Rc::new(Cell::new(0));
        let _subscription = cx
            .update_window(handle, |_, window, _| {
                let activation_count = activation_count.clone();
                let (subscription, activate) = window.activation_observers.insert(
                    (),
                    Box::new(move |_, _| {
                        activation_count.set(activation_count.get() + 1);
                        true
                    }),
                );
                activate();
                subscription
            })
            .expect("test window should remain live");
        activation_count.set(0);

        cx.update_window(handle, |_, window, _| {
            platform_window.simulate_active_status_change(false);
            platform_window.simulate_active_status_change(true);
            assert!(window.is_window_active());
        })
        .expect("test window should remain live");

        assert_eq!(
            activation_count.get(),
            2,
            "deactivation is a pointer-session fence and must not coalesce into reactivation"
        );
        assert!(
            cx.update_window(handle, |_, window, _| window.is_window_active())
                .expect("test window should remain live")
        );
    }

    #[crate::test]
    fn inline_close_releases_blocked_ingress_before_returning(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let (handle, platform_window) = open_test_window(cx);
        let deliveries = Rc::new(Cell::new(0));
        let _subscription = cx.update(|app| {
            let deliveries = deliveries.clone();
            app.on_keyboard_layout_change(move |_| {
                deliveries.set(deliveries.get() + 1);
            })
        });
        let app_cell = cx.app.clone();
        cx.update_window(handle, |_, window, app| {
            let app_cell = app_cell.clone();
            window.on_window_should_close(app, move |_, _| {
                app_cell.enqueue_keyboard_layout_changed_for_test();
                app_cell.drain_native_work_for_test();
                true
            });
        })
        .expect("test window should remain live");

        assert_eq!(
            platform_window.simulate_should_close(),
            Some(false),
            "an approved live close must suppress native default teardown while GPUI retires the window"
        );
        app_cell.enqueue_keyboard_layout_changed_for_test();
        cx.run_until_parked();

        assert!(!cx.windows().contains(&handle));
        assert_eq!(
            deliveries.get(),
            2,
            "a nested wake blocked on the close query must resume when its App borrow is released"
        );
    }

    #[crate::test]
    fn shutdown_keeps_the_registry_until_every_presentation_is_quiesced(cx: &mut TestAppContext) {
        let (first, first_platform_window) = open_test_window(cx);
        let (second, second_platform_window) = open_test_window(cx);
        first_platform_window.block_presentation_shutdown(true);

        cx.update(|app| app.shutdown());

        assert!(cx.windows().contains(&first));
        assert!(cx.windows().contains(&second));
        assert_eq!(first_platform_window.presentation_shutdown_counts().0, 1);
        assert!(first_platform_window.presentation_shutdown_counts().1 >= 1);
        assert_eq!(first_platform_window.presentation_shutdown_counts().2, 0);
        assert_eq!(
            second_platform_window.presentation_shutdown_counts(),
            (1, 1, 0)
        );

        first_platform_window.block_presentation_shutdown(false);
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(512));
        cx.run_until_parked();

        assert!(!cx.windows().contains(&first));
        assert!(!cx.windows().contains(&second));
        let first_counts = first_platform_window.presentation_shutdown_counts();
        assert_eq!(first_counts.0, 1);
        assert!(first_counts.1 >= 2);
        assert_eq!(first_counts.2, 1);
        assert_eq!(
            second_platform_window.presentation_shutdown_counts(),
            (1, 1, 1)
        );
    }

    #[crate::test]
    fn next_frame_close_prevents_any_later_draw_or_present(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let accepted_draws = platform_window.draw_count();
        cx.update_window(handle, |_, window, _| {
            window.on_next_frame(|window, app| window.remove_window(app));
        })
        .expect("the test window should remain live before the next frame");

        assert!(platform_window.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
        }));
        cx.run_until_parked();

        assert!(!cx.windows().contains(&handle));
        assert_eq!(platform_window.draw_count(), accepted_draws);
        assert_eq!(platform_window.presentation_shutdown_counts(), (1, 1, 1));
    }

    #[crate::test]
    fn native_terminal_before_quiescence_is_not_washed_by_later_close_processing(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);

        assert!(platform_window.simulate_close());
        cx.run_until_parked();

        let shutdown = platform_window
            .0
            .lock()
            .presentation_shutdown_ticket
            .clone()
            .expect("the native terminal must bind an emergency shutdown authority");
        let snapshot = shutdown.snapshot();
        assert_eq!(snapshot.window_id(), handle.window_id());
        assert!(snapshot.native_terminal());
        assert!(!snapshot.quiesced());
        assert!(snapshot.protocol_violation());
        assert!(!cx.windows().contains(&handle));
    }

    #[crate::test]
    fn shutdown_includes_a_logically_removed_window_with_pending_native_retirement(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let (handle, platform_window) = open_test_window(cx);
        platform_window.block_presentation_shutdown(true);
        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .expect("the test window should remain live until logical removal commits");

        assert!(!cx.windows().contains(&handle));
        let pending_counts = platform_window.presentation_shutdown_counts();
        assert_eq!(pending_counts.0, 1);
        assert!(pending_counts.1 >= 1);
        assert_eq!(pending_counts.2, 0);

        cx.update(|app| app.shutdown());
        let blocked_open = cx.update(|app| {
            app.open_window_detailed(WindowOptions::default(), |_, app| app.new(|_| Empty))
        });
        assert_eq!(
            blocked_open
                .expect_err("pending presentation shutdown must retain the app shutdown barrier")
                .stage(),
            WindowOpenFailureStage::AppShutdown
        );

        platform_window.block_presentation_shutdown(false);
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(512));
        cx.run_until_parked();

        let settled_counts = platform_window.presentation_shutdown_counts();
        assert_eq!(settled_counts.0, 1);
        assert!(settled_counts.1 >= 2);
        assert_eq!(settled_counts.2, 1);
        let replacement: AnyWindowHandle = cx
            .update(|app| app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty)))
            .expect("the app shutdown barrier must reopen after exact quiescence")
            .into();
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
    }

    #[crate::test]
    fn failed_window_map_rolls_back_reserved_window_state(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::LastWindowClosed));
        cx.fail_next_window_map("injected TestPlatform map failure");

        let failed =
            cx.update(|app| app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty)));
        assert!(
            failed.is_err(),
            "map failure must return through open_window"
        );
        assert!(
            cx.windows().is_empty(),
            "a failed map must not leave a visible or reserved window"
        );

        let handle: AnyWindowHandle = cx
            .update(|app| app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty)))
            .expect("a later window should open after rollback")
            .into();
        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .expect("the committed replacement window should close");

        assert!(
            cx.did_quit(),
            "the failed reservation must not keep last-window shutdown alive"
        );
    }

    #[crate::test]
    fn initial_presentation_stays_hidden_until_committed_command_runs_at_app_idle(
        cx: &mut TestAppContext,
    ) {
        let root_builder_observed = Rc::new(Cell::new(false));
        let completion_observed = Rc::new(Cell::new(false));
        let app = Rc::downgrade(&cx.app);
        let handle: AnyWindowHandle = cx
            .update(|app_context| {
                app_context.open_window(WindowOptions::default(), {
                    let root_builder_observed = root_builder_observed.clone();
                    let completion_observed = completion_observed.clone();
                    move |window, app_context| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        let handle = platform_window.0.lock().handle;
                        assert!(
                            !app_context.windows().contains(&handle),
                            "the root builder must run before the window registry commit"
                        );
                        assert_eq!(
                            platform_window.initial_presentation_state(),
                            (false, false, 0),
                            "native mapping must not expose an uncommitted window"
                        );
                        assert!(
                            platform_window.platform_command_history().is_empty(),
                            "initial presentation must wait for the registry commit"
                        );
                        root_builder_observed.set(true);

                        platform_window.set_platform_command_callback({
                            let completion_observed = completion_observed.clone();
                            let app = app.clone();
                            move |command, platform_window| {
                                assert_eq!(
                                    command,
                                    PlatformWindowCommand::CompleteInitialPresentation {
                                        activate: true,
                                    }
                                );
                                assert_eq!(
                                    platform_window.initial_presentation_state(),
                                    (false, false, 0),
                                    "the window must stay hidden until the completion command executes"
                                );
                                let app = app
                                    .upgrade()
                                    .expect("the App must outlive initial presentation");
                                let handle = platform_window.0.lock().handle;
                                let app_borrow = app.try_borrow_mut().expect(
                                    "initial presentation must run after the outer App borrow is released",
                                );
                                assert!(
                                    app_borrow.windows().contains(&handle),
                                    "initial presentation must run after the window registry commit"
                                );
                                drop(app_borrow);
                                completion_observed.set(true);
                                PlatformWindowCommandOutcome::Accepted
                            }
                        });

                        app_context.new(|_| Empty)
                    }
                })
            })
            .expect("the test window should open")
            .into();
        let platform_window = cx.test_window(handle);

        assert!(root_builder_observed.get());
        assert!(completion_observed.get());
        assert_eq!(
            platform_window.initial_presentation_state(),
            (true, true, 1)
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::CompleteInitialPresentation { activate: true }]
        );
    }

    #[crate::test]
    fn rejected_initial_presentation_retries_before_applying_completion(cx: &mut TestAppContext) {
        let attempts = Rc::new(Cell::new(0usize));
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(WindowOptions::default(), {
                    let attempts = attempts.clone();
                    move |window, app| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window.set_platform_command_callback({
                            let attempts = attempts.clone();
                            move |command, platform_window| {
                                assert_eq!(
                                    command,
                                    PlatformWindowCommand::CompleteInitialPresentation {
                                        activate: true,
                                    }
                                );
                                assert_eq!(
                                    platform_window.initial_presentation_state(),
                                    (false, false, 0),
                                    "a rejected attempt must not apply presentation state"
                                );
                                let attempt = attempts.get().saturating_add(1);
                                attempts.set(attempt);
                                if attempt == 1 {
                                    PlatformWindowCommandOutcome::Rejected
                                } else {
                                    PlatformWindowCommandOutcome::Accepted
                                }
                            }
                        });
                        app.new(|_| Empty)
                    }
                })
            })
            .expect("the accepted retry should complete window creation")
            .into();
        let platform_window = cx.test_window(handle);

        assert_eq!(attempts.get(), 2);
        assert_eq!(
            platform_window.initial_presentation_state(),
            (true, true, 1)
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [
                PlatformWindowCommand::CompleteInitialPresentation { activate: true },
                PlatformWindowCommand::CompleteInitialPresentation { activate: true },
            ]
        );
        assert_eq!(
            cx.update_window(handle, |_, window, _| {
                window.presentation_facts().initial_presentation
            })
            .expect("the accepted retry should remain registered"),
            crate::WindowInitialPresentationStatus::Completed
        );
    }

    #[crate::test]
    fn initial_presentation_stops_after_two_rejections_without_completion(cx: &mut TestAppContext) {
        let attempts = Rc::new(Cell::new(0usize));
        let observations = Rc::new(RefCell::new(Vec::new()));
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(WindowOptions::default(), {
                    let attempts = attempts.clone();
                    let observations = observations.clone();
                    move |window, app| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window.set_platform_command_callback({
                            let attempts = attempts.clone();
                            move |command, platform_window| {
                                assert_eq!(
                                    command,
                                    PlatformWindowCommand::CompleteInitialPresentation {
                                        activate: true,
                                    }
                                );
                                assert_eq!(
                                    platform_window.initial_presentation_state(),
                                    (false, false, 0),
                                    "a rejected attempt must leave the native target hidden"
                                );
                                attempts.set(attempts.get().saturating_add(1));
                                PlatformWindowCommandOutcome::Rejected
                            }
                        });
                        app.new(|cx| {
                            InitialPresentationObserverProbe::new(window, observations, cx)
                        })
                    }
                })
            })
            .expect("command rejection must not roll back a committed window")
            .into();
        cx.run_until_parked();

        let platform_window = cx.test_window(handle);
        assert_eq!(attempts.get(), 2);
        assert_eq!(
            observations.borrow().as_slice(),
            [WindowInitialPresentationStatus::Rejected]
        );
        assert_eq!(
            platform_window.initial_presentation_state(),
            (false, false, 0)
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [
                PlatformWindowCommand::CompleteInitialPresentation { activate: true },
                PlatformWindowCommand::CompleteInitialPresentation { activate: true },
            ]
        );

        let diagnostics = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        let rejected_commands = diagnostics
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == crate::NativeBoundaryKind::Command(
                            crate::NativePlatformCommandKind::CompleteInitialPresentation,
                        )
            })
            .collect::<Vec<_>>();
        assert_eq!(rejected_commands.len(), 2);
        assert!(rejected_commands.iter().all(|diagnostic| {
            diagnostic.disposition == crate::NativeBoundaryDisposition::Rejected
        }));
        assert!(diagnostics.terminal.iter().all(|diagnostic| {
            diagnostic.target != crate::NativeBoundaryTarget::Window(handle.window_id())
                || diagnostic.kind
                    != crate::NativeBoundaryKind::Callback(
                        crate::NativeCallbackKind::InitialPresentationCompleted,
                    )
        }));
        assert!(diagnostics.pending.iter().all(|diagnostic| {
            diagnostic.target != crate::NativeBoundaryTarget::Window(handle.window_id())
                || !matches!(
                    diagnostic.kind,
                    crate::NativeBoundaryKind::Command(
                        crate::NativePlatformCommandKind::CompleteInitialPresentation,
                    ) | crate::NativeBoundaryKind::Callback(
                        crate::NativeCallbackKind::InitialPresentationCompleted,
                    )
                )
        }));
    }

    #[crate::test]
    fn panicking_second_initial_presentation_attempt_settles_failure_before_propagation(
        cx: &mut TestAppContext,
    ) {
        let attempts = Rc::new(Cell::new(0usize));
        let observations = Rc::new(RefCell::new(Vec::new()));
        let platform_window = Rc::new(RefCell::new(None));
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                app.open_window(WindowOptions::default(), {
                    let attempts = attempts.clone();
                    let observations = observations.clone();
                    let platform_window_slot = platform_window.clone();
                    move |window, app| {
                        let test_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window_slot.replace(Some(test_window.clone()));
                        test_window.set_platform_command_callback({
                            let attempts = attempts.clone();
                            move |command, _| {
                                assert_eq!(
                                    command,
                                    PlatformWindowCommand::CompleteInitialPresentation {
                                        activate: true,
                                    }
                                );
                                let attempt = attempts.get().saturating_add(1);
                                attempts.set(attempt);
                                if attempt == 1 {
                                    PlatformWindowCommandOutcome::Rejected
                                } else {
                                    panic!("injected initial-presentation command panic");
                                }
                            }
                        });
                        app.new(|cx| {
                            InitialPresentationObserverProbe::new(window, observations, cx)
                        })
                    }
                })
                .expect("the committed window must survive command dispatch failure")
            });
        }))
        .expect_err("the dispatcher panic must propagate after terminal convergence");
        let panic_message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
        assert_eq!(
            panic_message,
            Some("injected initial-presentation command panic")
        );

        assert_eq!(attempts.get(), 2);
        assert_eq!(
            observations.borrow().as_slice(),
            [WindowInitialPresentationStatus::Rejected],
            "the unique failure terminal must publish before the dispatcher panic escapes"
        );
        let platform_window = platform_window
            .borrow()
            .clone()
            .expect("the root builder must retain the test window");
        let handle = platform_window.0.lock().handle;
        assert_eq!(
            platform_window.initial_presentation_state(),
            (false, false, 0)
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [
                PlatformWindowCommand::CompleteInitialPresentation { activate: true },
                PlatformWindowCommand::CompleteInitialPresentation { activate: true },
            ]
        );
        assert_eq!(
            cx.update_window(handle, |_, window, _| {
                window.presentation_facts().initial_presentation
            })
            .expect("the failed window must remain registered"),
            WindowInitialPresentationStatus::Rejected
        );

        let diagnostics = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        let command_terminals = diagnostics
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == crate::NativeBoundaryKind::Command(
                            crate::NativePlatformCommandKind::CompleteInitialPresentation,
                        )
            })
            .collect::<Vec<_>>();
        assert_eq!(command_terminals.len(), 2);
        assert_eq!(
            command_terminals[0].disposition,
            crate::NativeBoundaryDisposition::Rejected
        );
        assert_eq!(
            command_terminals[1].disposition,
            crate::NativeBoundaryDisposition::InvariantFailure(
                crate::NativeInvariantFailure::CallbackPanicked,
            )
        );
        let failure_terminals = diagnostics
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == crate::NativeBoundaryKind::Callback(
                            crate::NativeCallbackKind::InitialPresentationFailed,
                        )
            })
            .collect::<Vec<_>>();
        assert_eq!(failure_terminals.len(), 1);
        assert_eq!(
            failure_terminals[0].disposition,
            crate::NativeBoundaryDisposition::DELIVERED
        );
        assert!(diagnostics.pending.iter().all(|diagnostic| {
            diagnostic.target != crate::NativeBoundaryTarget::Window(handle.window_id())
                || !matches!(
                    diagnostic.kind,
                    crate::NativeBoundaryKind::Command(
                        crate::NativePlatformCommandKind::CompleteInitialPresentation,
                    ) | crate::NativeBoundaryKind::Callback(
                        crate::NativeCallbackKind::InitialPresentationFailed,
                    )
                )
        }));
    }

    #[crate::test]
    fn committed_window_completes_initial_presentation_once(cx: &mut TestAppContext) {
        let handle: AnyWindowHandle = cx
            .update(|app| app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty)))
            .expect("the test window should open")
            .into();
        let platform_window = cx.test_window(handle);

        assert_eq!(
            platform_window.initial_presentation_state(),
            (true, true, 1)
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::CompleteInitialPresentation { activate: true }]
        );
        assert_eq!(
            cx.update_window(handle, |_, window, _| {
                window.presentation_facts().initial_presentation
            })
            .expect("the committed window should remain live"),
            crate::WindowInitialPresentationStatus::Completed
        );
    }

    #[crate::test]
    fn hidden_window_completes_without_mapping_or_activation(cx: &mut TestAppContext) {
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        show: false,
                        focus_on_appearing: true,
                        ..Default::default()
                    },
                    |_, app| app.new(|_| Empty),
                )
            })
            .expect("the hidden test window should open")
            .into();
        let platform_window = cx.test_window(handle);

        assert_eq!(
            platform_window.initial_presentation_state(),
            (false, true, 0)
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::CompleteInitialPresentation { activate: false }]
        );
    }

    #[crate::test]
    fn activating_initial_presentation_is_rejected_after_window_quiescence(
        cx: &mut TestAppContext,
    ) {
        let command_callback_count = Rc::new(Cell::new(0usize));
        let (handle, platform_window) = cx.update(|app| {
            let handle: AnyWindowHandle = app
                .open_window(WindowOptions::default(), |_, app| app.new(|_| Empty))
                .expect("the test window should commit before quiescence")
                .into();
            let platform_window = handle
                .update(app, {
                    let command_callback_count = command_callback_count.clone();
                    move |_, window, app| {
                        let platform_window = window
                            .platform_window
                            .as_test()
                            .expect("the test backend must provide a TestWindow")
                            .clone();
                        platform_window.set_platform_command_callback({
                            let command_callback_count = command_callback_count.clone();
                            move |_, _| {
                                command_callback_count
                                    .set(command_callback_count.get().saturating_add(1));
                                PlatformWindowCommandOutcome::Accepted
                            }
                        });
                        assert!(window.quiesce_interaction(app));
                        platform_window
                    }
                })
                .expect("the committed window should remain available for quiescence");
            (handle, platform_window)
        });
        cx.run_until_parked();

        assert_eq!(command_callback_count.get(), 0);
        assert!(platform_window.platform_command_history().is_empty());
        assert_eq!(
            platform_window.initial_presentation_state(),
            (false, false, 0)
        );
        assert_eq!(
            cx.update_window(handle, |_, window, _| {
                window.presentation_facts().initial_presentation
            })
            .expect("the quiesced window must remain registered for retirement"),
            crate::WindowInitialPresentationStatus::Rejected
        );
    }

    #[crate::test]
    fn pending_provisional_final_placement_rejects_competing_placement_mutations(
        cx: &mut TestAppContext,
    ) {
        let (session, handle, platform_window) = open_revealed_provisional_window(cx, 401);
        let (dispatch, provisional_ticket) = cx
            .update_window(handle, |_, window, _| {
                window.request_provisional_placement(&session, provisional_placement_request(1))
            })
            .expect("the provisional window should remain live")
            .expect("the revealed session should admit final placement");
        let first_ticket = match dispatch {
            WindowMutationDispatch::Queued(ticket) => ticket,
            dispatch => panic!("expected queued provisional placement, got {dispatch:?}"),
        };

        let logical_replacement = cx
            .update_window(handle, |_, window, _| {
                window.request_window_placement_request(WindowPlacementRequest::windowed(bounds(
                    25.0, 35.0, 440.0, 300.0,
                )))
            })
            .expect("the provisional window should remain live");
        assert!(matches!(
            logical_replacement,
            WindowMutationDispatch::Rejected
        ));
        let physical_replacement = cx
            .update_window(handle, |_, window, _| {
                window.request_physical_window_placement(
                    crate::WindowPhysicalPlacementRequest::try_new(Bounds::new(
                        point(DevicePixels(25), DevicePixels(35)),
                        size(DevicePixels(440), DevicePixels(300)),
                    ))
                    .expect("the competing physical placement should be valid"),
                )
            })
            .expect("the provisional window should remain live");
        assert!(matches!(
            physical_replacement,
            WindowMutationDispatch::Rejected
        ));
        assert_eq!(
            first_ticket.observation(),
            None,
            "a competing mutation must not terminalize the in-flight exact placement"
        );
        assert_eq!(
            provisional_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::Pending,
            "the special placement must retain authority until its native transaction settles"
        );

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
        cx.run_until_parked();
        assert_eq!(
            first_ticket
                .observation()
                .map(|observation| observation.outcome),
            Some(WindowMutationOutcome::Exact)
        );
        assert_eq!(
            provisional_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::Settled
        );
    }

    #[crate::test]
    fn settled_provisional_placement_cannot_authorize_semantics_after_newer_placement(
        cx: &mut TestAppContext,
    ) {
        let (session, handle, platform_window) = open_revealed_provisional_window(cx, 403);
        let (dispatch, provisional_ticket) = cx
            .update_window(handle, |_, window, _| {
                window.request_provisional_placement(&session, provisional_placement_request(1))
            })
            .expect("the provisional window should remain live")
            .expect("the revealed session should admit final placement");
        assert!(matches!(dispatch, WindowMutationDispatch::Queued(_)));
        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
        assert_eq!(
            provisional_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::Settled
        );

        let replacement = cx
            .update_window(handle, |_, window, _| {
                window.request_window_placement_request(WindowPlacementRequest::windowed(bounds(
                    25.0, 35.0, 440.0, 300.0,
                )))
            })
            .expect("the provisional window should remain live");
        assert!(matches!(replacement, WindowMutationDispatch::Queued(_)));
        assert_eq!(
            provisional_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::Settled,
            "the immutable receipt may retain its historical terminal outcome"
        );

        cx.update_window(handle, |_, window, app| {
            window
                .begin_provisional_destination_semantics(&session, 1, app)
                .expect_err(
                    "a historical receipt must not authorize semantics after a newer placement",
                );
        })
        .expect("the provisional window should remain live");

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
        cx.run_until_parked();
    }

    #[crate::test]
    fn provisional_final_placement_terminal_settles_both_receipts(cx: &mut TestAppContext) {
        let (session, handle, platform_window) = open_revealed_provisional_window(cx, 402);
        let (dispatch, provisional_ticket) = cx
            .update_window(handle, |_, window, _| {
                window.request_provisional_placement(&session, provisional_placement_request(1))
            })
            .expect("the provisional window should remain live")
            .expect("the revealed session should admit final placement");
        let mutation_ticket = match dispatch {
            WindowMutationDispatch::Queued(ticket) => ticket,
            dispatch => panic!("expected queued provisional placement, got {dispatch:?}"),
        };
        let facts = platform_window.platform_facts();

        assert!(platform_window.simulate_window_mutation_terminal(
            WindowMutationDomain::Placement,
            PlatformWindowMutationTerminal::WindowClosed,
            facts,
        ));
        cx.run_until_parked();

        assert_eq!(
            mutation_ticket
                .observation()
                .map(|observation| observation.outcome),
            Some(WindowMutationOutcome::WindowClosed)
        );
        assert_eq!(
            provisional_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::WindowTerminal
        );
    }

    #[crate::test]
    fn window_close_settles_provisional_authority_before_generic_observers(
        cx: &mut TestAppContext,
    ) {
        let (session, handle, _platform_window) = open_revealed_provisional_window(cx, 404);
        let (dispatch, provisional_ticket) = cx
            .update_window(handle, |_, window, _| {
                window.request_provisional_placement(&session, provisional_placement_request(1))
            })
            .expect("the provisional window should remain live")
            .expect("the revealed session should admit final placement");
        let mutation_ticket = match dispatch {
            WindowMutationDispatch::Queued(ticket) => ticket,
            dispatch => panic!("expected queued provisional placement, got {dispatch:?}"),
        };
        let observed_terminal_order = Rc::new(Cell::new(false));
        let observed_terminal_order_for_callback = Rc::clone(&observed_terminal_order);
        let provisional_ticket_for_callback = provisional_ticket.clone();
        let _subscription = mutation_ticket.subscribe(move |observation| {
            assert_eq!(observation.outcome, WindowMutationOutcome::WindowClosed);
            assert_eq!(
                provisional_ticket_for_callback.snapshot().outcome(),
                WindowProvisionalPlacementOutcome::WindowTerminal,
                "generic observers must see the special provisional authority already terminal"
            );
            observed_terminal_order_for_callback.set(true);
        });

        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .expect("the provisional window should remain addressable during close");
        assert!(observed_terminal_order.get());
        assert_eq!(
            provisional_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::WindowTerminal
        );
    }

    #[crate::test]
    fn provisional_session_reveals_one_non_empty_generation_without_activation_then_promotes(
        cx: &mut TestAppContext,
    ) {
        let session =
            WindowProvisionalSession::new(41).expect("the provisional generation should be valid");
        let semantics_authority = Rc::new(RefCell::new(None));
        let handle: AnyWindowHandle = cx
            .update(|app| {
                let semantics_authority = semantics_authority.clone();
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| {
                        app.new(|_| ProvisionalSemanticsMarkerRoot {
                            authority: semantics_authority,
                        })
                    },
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);

        assert!(
            !platform_window.is_visible(),
            "a provisional must stay hidden after the ordinary initial-presentation command"
        );
        assert!(
            !session.snapshot().accepts_interaction(),
            "construction and initial root work must observe the provisional gate"
        );

        let reveal_peer = crate::WindowId::from(900);
        let reveal_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [reveal_peer],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();

        let facts = cx
            .update_window(handle, |_, window, _| window.presentation_facts())
            .expect("the revealed provisional should remain live");
        let generation = facts
            .non_empty_presented_generation
            .expect("the reveal must be backed by a non-empty submitted frame");
        assert!(facts.native_visible);
        assert_eq!(platform_window.activation_count(), 0);
        let reveal = reveal_ticket.snapshot();
        assert_eq!(reveal.window_id(), handle.window_id());
        assert_eq!(reveal.session_generation(), session.snapshot().generation());
        assert_eq!(reveal.presentation_generation(), Some(generation));
        assert_eq!(
            reveal.outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [
                PlatformWindowCommand::CompleteInitialPresentation { activate: false },
                PlatformWindowCommand::RevealDeferredInitialPresentation {
                    session_generation: session.snapshot().generation(),
                    presentation_generation: generation,
                },
            ]
        );
        assert!(
            !session.snapshot().accepts_interaction(),
            "visibility alone must not admit provisional interaction"
        );

        let placement_bounds = Bounds::new(
            point(DevicePixels(80), DevicePixels(90)),
            size(DevicePixels(360), DevicePixels(260)),
        );
        let placement_request = WindowProvisionalPlacementRequest::try_new(
            91,
            WindowProvisionalPlacementPurpose::FinalRelease,
            placement_bounds,
            point(DevicePixels(120), DevicePixels(130)),
            PlatformPhysicalDisplayObservation::try_new(
                1,
                DisplayId::from(1),
                Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(3_840), DevicePixels(2_160)),
                ),
                Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(3_840), DevicePixels(2_080)),
                ),
                2.0,
            )
            .expect("the test target display should be representable"),
        )
        .expect("the test final placement should be structurally valid");
        let (placement_dispatch, placement_ticket) = cx
            .update_window(handle, |_, window, _| {
                window.request_provisional_placement(&session, placement_request)
            })
            .expect("the provisional window should remain live")
            .expect("the revealed session should admit final placement");
        assert!(matches!(
            placement_dispatch,
            WindowMutationDispatch::Queued(_)
        ));
        assert_eq!(
            placement_ticket.snapshot().outcome(),
            WindowProvisionalPlacementOutcome::Pending,
            "queued platform work must not manufacture final-placement evidence"
        );
        assert_eq!(
            session
                .final_placement_request(handle.window_id(), 91)
                .expect("the test backend should observe the exact pending request")
                .peer_windows(),
            &[reveal_peer],
            "final placement must consume the peer authority frozen by reveal"
        );
        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        let placement = placement_ticket.snapshot();
        assert_eq!(
            placement.outcome(),
            WindowProvisionalPlacementOutcome::Settled
        );
        assert_eq!(placement.client_bounds(), placement_bounds);
        assert!(
            placement
                .native_facts()
                .is_some_and(|facts| facts.physical_geometry_exact()),
            "the test backend must settle native facts before mutation observers run"
        );

        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 73, app)
                    .expect("the revealed provisional should begin projecting its destination")
            })
            .expect("the provisional window should remain live");
        assert!(
            semantics_ticket.snapshot().minimum_frame_generation() > generation,
            "destination semantics must commit strictly after the exact reveal frame"
        );
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::ProjectingDestinationSemantics,
            "beginning projection must not itself manufacture a semantics receipt"
        );
        assert!(!session.snapshot().accepts_interaction());
        let frozen_facts = platform_window.platform_facts();
        cx.update_window(handle, |_, window, _| {
            assert!(matches!(
                window.request_pointer_input(frozen_facts.accepts_pointer_input),
                WindowMutationDispatch::Unchanged
            ));
            assert!(matches!(
                window.request_pointer_input(!frozen_facts.accepts_pointer_input),
                WindowMutationDispatch::Rejected
            ));
            assert!(matches!(
                window.request_topmost(!frozen_facts.topmost),
                WindowMutationDispatch::Rejected
            ));
            assert!(matches!(
                window.request_window_placement_request(WindowPlacementRequest::windowed(bounds(
                    25.0, 35.0, 440.0, 300.0,
                ))),
                WindowMutationDispatch::Rejected
            ));
            assert!(matches!(
                window.request_physical_window_placement(
                    crate::WindowPhysicalPlacementRequest::try_new(Bounds::new(
                        point(DevicePixels(25), DevicePixels(35)),
                        size(DevicePixels(440), DevicePixels(300)),
                    ))
                    .expect("the competing physical placement should be valid"),
                ),
                WindowMutationDispatch::Rejected
            ));
        })
        .expect("the provisional window should remain live");
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted,
            "focus-stable acceptance must precede renderer submission"
        );
        cx.update_window(handle, |_, window, _| {
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect("the accepted destination frame should be presented");
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let semantics = semantics_ticket.snapshot();
        assert_eq!(
            semantics.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted
        );
        assert_eq!(semantics.window_id(), handle.window_id());
        assert_eq!(
            semantics.session_generation(),
            session.snapshot().generation()
        );
        assert_eq!(semantics.destination_generation(), 73);
        assert!(
            semantics
                .accepted_frame_generation()
                .is_some_and(|generation| generation >= semantics.minimum_frame_generation())
        );
        assert_eq!(
            semantics.submitted_frame_generation(),
            semantics.accepted_frame_generation(),
            "submission must name the exact accepted destination frame"
        );
        assert!(!session.snapshot().accepts_interaction());
        cx.update_window(handle, |_, window, _| {
            assert!(matches!(
                window.request_pointer_input(!frozen_facts.accepts_pointer_input),
                WindowMutationDispatch::Rejected
            ));
            assert!(matches!(
                window.request_topmost(!frozen_facts.topmost),
                WindowMutationDispatch::Rejected
            ));
        })
        .expect("the provisional window should remain live");

        cx.update_window(handle, |_, window, app| {
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect("the submitted destination receipt should open the exact gate");
        })
        .expect("the provisional window should remain live");
        assert!(session.snapshot().accepts_interaction());
        assert_eq!(
            platform_window.handle(),
            handle,
            "promotion must retain the original native window identity"
        );

        assert!(platform_window.simulate_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
        }));
        cx.run_until_parked();
        assert_eq!(
            platform_window
                .platform_command_history()
                .iter()
                .filter(|command| matches!(
                    command,
                    PlatformWindowCommand::RevealDeferredInitialPresentation { .. }
                ))
                .count(),
            1,
            "later submitted frames must not reveal the same provisional generation twice"
        );
    }

    #[crate::test]
    fn panicking_provisional_reveal_dispatch_settles_ticket_before_propagation(
        cx: &mut TestAppContext,
    ) {
        let session =
            WindowProvisionalSession::new(414).expect("the provisional generation should be valid");
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| PaintedRoot),
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        platform_window.set_platform_command_callback(|command, _| {
            assert!(matches!(
                command,
                PlatformWindowCommand::RevealDeferredInitialPresentation { .. }
            ));
            panic!("injected provisional reveal command panic");
        });
        let ticket = Rc::new(RefCell::new(None));

        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update_window(handle, |_, window, app| {
                let reveal = window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation");
                ticket.replace(Some(reveal));
            })
            .expect("the provisional window should remain live");
            cx.run_until_parked();
        }))
        .expect_err("the provisional dispatcher panic must propagate after settlement");
        let panic_message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
        assert_eq!(
            panic_message,
            Some("injected provisional reveal command panic")
        );
        assert_eq!(
            ticket
                .borrow()
                .as_ref()
                .expect("arming must publish the reveal ticket before dispatch")
                .snapshot()
                .outcome(),
            crate::WindowProvisionalRevealOutcome::Rejected
        );
        assert!(!platform_window.is_visible());
    }

    #[crate::test]
    fn cancelling_exact_provisional_reveal_blocks_late_frame_and_native_command(
        cx: &mut TestAppContext,
    ) {
        let session =
            WindowProvisionalSession::new(411).expect("the provisional generation should be valid");
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| PaintedRoot),
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        assert!(!platform_window.is_visible());
        platform_window.defer_frame_requests_for_test();

        let ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        assert_eq!(
            ticket.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Pending
        );

        let cancellation = cx
            .update_window(handle, |_, window, app| {
                window
                    .cancel_provisional_presentation(&ticket, app)
                    .expect("the exact pending reveal should cancel")
            })
            .expect("the provisional window should remain live");
        let crate::WindowProvisionalRevealCancellationOutcome::Cancelled(snapshot) = cancellation
        else {
            panic!("cancellation must win before the deferred reveal frame");
        };
        assert_eq!(
            snapshot.outcome(),
            crate::WindowProvisionalRevealOutcome::Cancelled
        );
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::Terminal
        );

        assert!(
            platform_window.release_deferred_frame_request_for_test(),
            "the armed reveal must retain one late frame request"
        );
        cx.run_until_parked();

        assert_eq!(
            ticket.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Cancelled,
            "late presentation cannot overwrite the cancellation winner"
        );
        assert!(!platform_window.is_visible());
        assert!(
            platform_window
                .platform_command_history()
                .iter()
                .all(|command| !matches!(
                    command,
                    PlatformWindowCommand::RevealDeferredInitialPresentation { .. }
                )),
            "a cancelled ticket must never dispatch its native reveal command"
        );
    }

    #[crate::test]
    fn queued_provisional_reveal_is_fenced_before_native_drain(cx: &mut TestAppContext) {
        let session =
            WindowProvisionalSession::new(413).expect("the provisional generation should be valid");
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| PaintedRoot),
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        platform_window.clear_platform_command_history();
        platform_window.defer_frame_requests_for_test();
        let app_cell = cx.app.clone();

        let ticket = cx
            .update_window(handle, |_, window, app| {
                let ticket = window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation");
                let presentation_generation = ticket.snapshot().minimum_presentation_generation();
                app_cell.enqueue_provisional_window_reveal(
                    handle.window_id(),
                    platform_window.command_dispatcher(),
                    PlatformWindowCommand::RevealDeferredInitialPresentation {
                        session_generation: session.snapshot().generation(),
                        presentation_generation,
                    },
                    ticket.clone(),
                );
                assert!(
                    platform_window.platform_command_history().is_empty(),
                    "the reveal command must remain queued while the App update owns the borrow"
                );
                assert!(matches!(
                    window.cancel_provisional_presentation(&ticket, app),
                    Ok(crate::WindowProvisionalRevealCancellationOutcome::Cancelled(_))
                ));
                ticket
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();

        assert_eq!(
            ticket.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Cancelled
        );
        assert!(
            platform_window
                .platform_command_history()
                .iter()
                .all(|command| !matches!(
                    command,
                    PlatformWindowCommand::RevealDeferredInitialPresentation { .. }
                )),
            "AppCell must re-check the exact ticket immediately before native dispatch"
        );
        assert!(!platform_window.is_visible());
    }

    #[crate::test]
    fn cancelling_after_native_reveal_preserves_the_reveal_winner(cx: &mut TestAppContext) {
        let session =
            WindowProvisionalSession::new(412).expect("the provisional generation should be valid");
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| PaintedRoot),
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        let ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            ticket.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );

        let cancellation = cx
            .update_window(handle, |_, window, app| {
                window
                    .cancel_provisional_presentation(&ticket, app)
                    .expect("the exact settled reveal should remain addressable")
            })
            .expect("the provisional window should remain live");
        let crate::WindowProvisionalRevealCancellationOutcome::AlreadySettled(snapshot) =
            cancellation
        else {
            panic!("native reveal must remain the single terminal winner");
        };
        assert_eq!(
            snapshot.outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::Gated,
            "losing cancellation must not terminate a successfully revealed session"
        );
        assert!(platform_window.is_visible());
    }

    #[crate::test]
    fn provisional_destination_semantics_require_an_exact_frame_marker(cx: &mut TestAppContext) {
        let session =
            WindowProvisionalSession::new(42).expect("the provisional generation should be valid");
        let handle: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| app.new(|_| PaintedRoot),
                )
            })
            .expect("the provisional test window should open")
            .into();

        let reveal = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            reveal.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );
        settle_provisional_final_placement(cx, handle, &session);

        let semantics = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 74, app)
                    .expect("the revealed provisional should begin semantics projection")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();

        assert_eq!(
            semantics.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Pending,
            "an ordinary non-empty frame must not stand in for a destination marker"
        );
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::ProjectingDestinationSemantics
        );
        assert!(!session.snapshot().accepts_interaction());
    }

    #[crate::test]
    fn provisional_interaction_requires_submitted_exact_semantics_frame(cx: &mut TestAppContext) {
        let (session, handle, platform_window, semantics_authority) =
            open_revealed_provisional_semantics_window(cx, 84);
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 90, app)
                    .expect("the revealed provisional should project its destination")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();

        let accepted = semantics_ticket.snapshot();
        assert_eq!(
            accepted.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted,
            "the accepted semantics frame should remain observable while submission is deferred"
        );
        assert!(accepted.accepted_frame_generation().is_some());
        assert_eq!(accepted.submitted_frame_generation(), None);
        cx.update_window(handle, |_, window, app| {
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect_err("a deferred renderer submission must keep interaction gated");
        })
        .expect("the provisional window should remain live");
        assert!(!session.snapshot().accepts_interaction());

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        cx.update_window(handle, |_, window, _| {
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect("the accepted semantics frame should remain retryable");
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let submitted = semantics_ticket.snapshot();
        assert_eq!(
            submitted.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted
        );
        assert_eq!(
            submitted.submitted_frame_generation(),
            accepted.accepted_frame_generation(),
            "only submission of the exact accepted frame may open the gate"
        );
        cx.update_window(handle, |_, window, app| {
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect("the exact submitted semantics frame should admit interaction");
        })
        .expect("the provisional window should remain live");
        assert!(session.snapshot().accepts_interaction());
    }

    #[crate::test]
    fn renderer_rejection_terminally_rejects_semantics_and_keeps_interaction_gated(
        cx: &mut TestAppContext,
    ) {
        let (session, handle, platform_window, semantics_authority) =
            open_revealed_provisional_semantics_window(cx, 88);
        let window_id = handle.window_id();
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Rejected);
        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 94, app)
                    .expect("the revealed provisional should project its destination")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();

        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted,
            "focus-stable acceptance must precede the renderer attempt"
        );
        cx.update_window(handle, |_, window, _| {
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect("the accepted semantics frame should reach the rejecting renderer");
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();

        let rejected = semantics_ticket.snapshot();
        assert_eq!(
            rejected.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Rejected
        );
        assert!(rejected.accepted_frame_generation().is_some());
        assert_eq!(rejected.submitted_frame_generation(), None);
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::DestinationSemanticsRejected
        );

        assert!(!session.snapshot().accepts_interaction());
        cx.update_window(handle, |_, window, app| {
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect_err("a renderer rejection must keep interaction gated");
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect_err("a renderer rejection must not remain retryable");
        })
        .expect("the provisional window should remain live");

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        assert!(platform_window.simulate_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        }));
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot(),
            rejected,
            "a later renderer submission must not revive rejected semantics authority"
        );
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::DestinationSemanticsRejected
        );

        cx.update_window(handle, |_, _, _| {
            session
                .terminate(window_id)
                .expect("the rejected provisional session should terminate exactly once");
        })
        .expect("the rejected provisional window should remain live while terminating");
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Rejected,
            "window termination must not overwrite the first renderer terminal outcome"
        );
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::Terminal
        );
    }

    #[crate::test]
    fn newer_focus_stable_semantics_frame_replaces_unsubmitted_acceptance(cx: &mut TestAppContext) {
        let (session, handle, platform_window, semantics_authority) =
            open_revealed_provisional_semantics_window(cx, 87);
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 93, app)
                    .expect("the revealed provisional should project its destination")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let first_acceptance = semantics_ticket.snapshot();
        let first_generation = first_acceptance
            .accepted_frame_generation()
            .expect("the first focus-stable frame should be accepted");
        assert_eq!(
            first_acceptance.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted
        );

        platform_window.defer_frame_requests_for_test();
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let replacement = semantics_ticket.snapshot();
        let replacement_generation = replacement
            .accepted_frame_generation()
            .expect("the newer focus-stable frame should replace the old acceptance");
        assert!(replacement_generation > first_generation);
        assert_eq!(replacement.submitted_frame_generation(), None);
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::DestinationSemanticsAccepted
        );

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        cx.update_window(handle, |_, window, _| {
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect("the replacement frame should remain presentation-ready");
        })
        .expect("the provisional window should remain live");
        assert!(
            platform_window.release_deferred_frame_request_for_test(),
            "the exact replacement frame should retain one deferred presentation request"
        );
        cx.run_until_parked();
        let submitted = semantics_ticket.snapshot();
        assert_eq!(
            submitted.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted
        );
        assert_eq!(
            submitted.submitted_frame_generation(),
            Some(replacement_generation),
            "only the replacement acceptance may become the submitted semantics proof"
        );
    }

    #[crate::test]
    fn native_terminal_during_semantics_present_cannot_publish_submission(cx: &mut TestAppContext) {
        let (session, handle, platform_window, semantics_authority) =
            open_revealed_provisional_semantics_window(cx, 86);
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 92, app)
                    .expect("the revealed provisional should project its destination")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let accepted = semantics_ticket.snapshot();
        assert_eq!(
            accepted.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted
        );
        let accepted_generation = accepted
            .accepted_frame_generation()
            .expect("the deferred semantics frame should remain accepted");

        platform_window.close_on_next_present_for_test();
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        assert!(platform_window.simulate_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        }));
        cx.run_until_parked();
        assert!(platform_window.is_native_terminal());
        assert!(!cx.windows().contains(&handle));
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::Terminal
        );
        let terminal = semantics_ticket.snapshot();
        assert_eq!(
            terminal.outcome(),
            crate::WindowProvisionalSemanticsOutcome::WindowTerminal
        );
        assert_eq!(
            terminal.accepted_frame_generation(),
            Some(accepted_generation),
            "terminal settlement may retain the accepted fact without manufacturing submission"
        );
        assert_eq!(terminal.submitted_frame_generation(), None);
    }

    #[crate::test]
    fn renderer_repaint_invalidates_accepted_semantics_and_requires_a_higher_frame(
        cx: &mut TestAppContext,
    ) {
        let (session, handle, platform_window, semantics_authority) =
            open_revealed_provisional_semantics_window(cx, 85);
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 91, app)
                    .expect("the revealed provisional should project its destination")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let first_acceptance = semantics_ticket.snapshot();
        assert_eq!(
            first_acceptance.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted
        );
        let rejected_generation = first_acceptance
            .accepted_frame_generation()
            .expect("the deferred attempt should retain its accepted frame");

        platform_window.defer_frame_requests_for_test();
        platform_window.set_present_outcome(PlatformWindowPresentOutcome::RepaintRequired);
        assert!(platform_window.simulate_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        }));
        cx.run_until_parked();
        let invalidated = semantics_ticket.snapshot();
        assert_eq!(
            invalidated.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Pending
        );
        assert_eq!(invalidated.accepted_frame_generation(), None);
        assert_eq!(invalidated.submitted_frame_generation(), None);
        assert!(
            invalidated.minimum_frame_generation() > rejected_generation,
            "renderer invalidation must prohibit reuse of the rejected accepted generation"
        );
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::ProjectingDestinationSemantics
        );
        cx.update_window(handle, |_, window, app| {
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect_err("an invalidated accepted frame cannot admit interaction");
        })
        .expect("the provisional window should remain live");

        platform_window.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        assert!(
            platform_window.release_deferred_frame_request_for_test(),
            "renderer invalidation should retain one forced higher-generation frame request"
        );
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Pending,
            "renderer recovery alone must not recreate destination-semantics authority"
        );

        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        let replacement = semantics_ticket.snapshot();
        assert_eq!(
            replacement.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted,
            "accepting the replacement marker must not manufacture renderer submission"
        );
        assert!(
            replacement
                .accepted_frame_generation()
                .is_some_and(|generation| generation > rejected_generation)
        );

        assert!(platform_window.simulate_frame(RequestFrameOptions {
            force_render: false,
            require_presentation: true,
        }));
        cx.run_until_parked();
        let submitted = semantics_ticket.snapshot();
        assert_eq!(
            submitted.outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted
        );
        assert!(
            submitted
                .submitted_frame_generation()
                .is_some_and(|generation| generation > rejected_generation)
        );
    }

    #[crate::test]
    fn provisional_session_rejects_stale_identity_and_input_without_replay(
        cx: &mut TestAppContext,
    ) {
        assert_eq!(
            WindowProvisionalSession::new(0).expect_err("zero is not a valid authority generation"),
            crate::WindowProvisionalSessionError::ZeroGeneration
        );

        let session = WindowProvisionalSession::new(7).expect("the generation should be valid");
        let element_pointer_calls = Rc::new(Cell::new(0usize));
        let semantics_authority = Rc::new(RefCell::new(None));
        let handle: AnyWindowHandle = cx
            .update(|app| {
                let element_pointer_calls = element_pointer_calls.clone();
                let semantics_authority = semantics_authority.clone();
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    move |_, app| {
                        app.new(move |_| InteractiveProvisionalRoot {
                            mouse_downs: element_pointer_calls,
                            semantics_authority,
                        })
                    },
                )
            })
            .expect("the provisional test window should open")
            .into();
        let other: AnyWindowHandle = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .into();
        let duplicate: anyhow::Result<crate::WindowHandle<PaintedRoot>> = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    focus_on_appearing: false,
                    provisional_session: Some(session.clone()),
                    ..Default::default()
                },
                |_, app| app.new(|_| PaintedRoot),
            )
        });
        duplicate.expect_err("one provisional session must own at most one window generation");
        assert_eq!(
            session
                .begin_destination_semantics(other.window_id(), 11, 1, 1)
                .expect_err("a stale full window id must not project destination semantics"),
            crate::WindowProvisionalSessionError::WindowMismatch
        );
        cx.update_window(handle, |_, window, app| {
            window
                .begin_provisional_destination_semantics(&session, 11, app)
                .expect_err("an unrevealed provisional must not project destination semantics");
        })
        .expect("the provisional window should remain live");
        let reveal_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            reveal_ticket.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );

        let pointer_calls = Rc::new(Cell::new(0usize));
        let _interceptor = cx
            .update_window(handle, |_, window, _| {
                let pointer_calls = pointer_calls.clone();
                window.intercept_window_mouse_events(move |event, _, _| {
                    if matches!(event, WindowMouseEvent::Down(_)) {
                        pointer_calls.set(pointer_calls.get().saturating_add(1));
                    }
                })
            })
            .expect("the provisional window should remain live");
        let mut platform_window = cx.test_window(handle);
        platform_window.clear_platform_command_history();

        let gated =
            platform_window.simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(12.0), px(18.0)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }));
        assert!(!gated.propagate);
        assert!(gated.default_prevented);
        assert_eq!(pointer_calls.get(), 0);
        assert_eq!(element_pointer_calls.get(), 0);
        let gated_activation = cx
            .update_window(handle, |_, window, _| window.activate_window())
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            gated_activation.snapshot().status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::Rejected),
            "the provisional interaction gate must settle its exact activation ticket"
        );
        assert!(
            platform_window.platform_command_history().is_empty(),
            "a gated window must not enqueue native activation"
        );
        settle_provisional_final_placement(cx, handle, &session);

        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 11, app)
                    .expect("the exact same window should project its destination in place")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<InteractiveProvisionalRoot>()
                .expect("the interactive provisional root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        let projecting =
            platform_window.simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(12.0), px(18.0)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }));
        assert!(!projecting.propagate);
        assert!(projecting.default_prevented);
        assert_eq!(pointer_calls.get(), 0);
        assert_eq!(element_pointer_calls.get(), 0);
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted,
            "focus-stable acceptance must precede renderer submission"
        );
        cx.update_window(handle, |_, window, _| {
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect("the accepted destination frame should be presented");
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted
        );
        let submitted_but_gated =
            platform_window.simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(12.0), px(18.0)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }));
        assert!(!submitted_but_gated.propagate);
        assert!(submitted_but_gated.default_prevented);
        assert_eq!(pointer_calls.get(), 0);
        assert_eq!(element_pointer_calls.get(), 0);
        cx.update_window(handle, |_, window, app| {
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect("the exact submitted destination should promote in place");
        })
        .expect("the provisional window should remain live");
        let promoted =
            platform_window.simulate_input_result(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(12.0), px(18.0)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }));
        assert_eq!(pointer_calls.get(), 1);
        assert_eq!(
            element_pointer_calls.get(),
            1,
            "promotion must expose the element handlers from the submitted destination frame"
        );
        assert!(
            promoted.propagate || !promoted.default_prevented,
            "promotion admits only new input; it does not replay the gated event"
        );
        let promoted_activation = cx
            .update_window(handle, |_, window, _| window.activate_window())
            .expect("the promoted window should remain live");
        cx.run_until_parked();
        let promoted_generation = promoted_activation.snapshot().request_generation();
        assert!(
            promoted_generation > gated_activation.snapshot().request_generation(),
            "every activation attempt must retain a distinct monotonic generation"
        );
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::Activate {
                request_generation: promoted_generation,
            }]
        );
    }

    #[crate::test]
    fn provisional_interaction_rejects_native_terminal_after_semantics_submission(
        cx: &mut TestAppContext,
    ) {
        let session =
            WindowProvisionalSession::new(83).expect("the provisional generation should be valid");
        let semantics_authority = Rc::new(RefCell::new(None));
        let handle: AnyWindowHandle = cx
            .update(|app| {
                let semantics_authority = semantics_authority.clone();
                app.open_window(
                    WindowOptions {
                        focus_on_appearing: false,
                        provisional_session: Some(session.clone()),
                        ..Default::default()
                    },
                    |_, app| {
                        app.new(|_| ProvisionalSemanticsMarkerRoot {
                            authority: semantics_authority,
                        })
                    },
                )
            })
            .expect("the provisional test window should open")
            .into();
        let platform_window = cx.test_window(handle);
        let reveal_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .arm_provisional_presentation(
                        &session,
                        point(DevicePixels(40), DevicePixels(50)),
                        [],
                        app,
                    )
                    .expect("the matching bound session should arm presentation")
            })
            .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            reveal_ticket.snapshot().outcome(),
            crate::WindowProvisionalRevealOutcome::Revealed
        );
        settle_provisional_final_placement(cx, handle, &session);
        let semantics_ticket = cx
            .update_window(handle, |_, window, app| {
                window
                    .begin_provisional_destination_semantics(&session, 89, app)
                    .expect("the revealed provisional should project its destination")
            })
            .expect("the provisional window should remain live");
        *semantics_authority.borrow_mut() = Some((session.clone(), semantics_ticket.clone()));
        cx.update_window(handle, |root, _, app| {
            root.downcast::<ProvisionalSemanticsMarkerRoot>()
                .expect("the provisional semantics root should retain its exact type")
                .update(app, |_, root_cx| root_cx.notify());
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Accepted,
            "focus-stable acceptance must precede renderer submission"
        );
        cx.update_window(handle, |_, window, _| {
            window
                .request_provisional_destination_semantics_presentation(&session, &semantics_ticket)
                .expect("the accepted destination frame should be presented");
        })
        .expect("the provisional window should remain live");
        cx.run_until_parked();
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted
        );

        cx.update_window(handle, |_, window, app| {
            assert!(platform_window.simulate_close());
            window
                .admit_provisional_interaction(&session, &semantics_ticket, app)
                .expect_err("native terminal must win before queued logical removal drains");
        })
        .expect("the logical window should remain present during the native callback");
        assert!(!session.snapshot().accepts_interaction());

        cx.run_until_parked();
        assert!(!cx.windows().contains(&handle));
        assert_eq!(
            session.snapshot().phase(),
            crate::WindowProvisionalSessionPhase::Terminal
        );
        assert_eq!(
            semantics_ticket.snapshot().outcome(),
            crate::WindowProvisionalSemanticsOutcome::Submitted,
            "the receipt should preserve the submitted frame fact after later window loss"
        );
    }

    #[crate::test]
    fn initial_appearance_is_independent_from_lifetime_activation_policy(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        for focus_on_appearing in [false, true] {
            for (accepts_activation, focus_on_click) in
                [(false, false), (false, true), (true, false), (true, true)]
            {
                let policy = WindowActivationPolicy {
                    accepts_activation,
                    focus_on_click,
                };
                let handle: AnyWindowHandle = cx
                    .update(|app| {
                        app.open_window(
                            WindowOptions {
                                focus_on_appearing,
                                activation_policy: policy,
                                ..Default::default()
                            },
                            |_, app| app.new(|_| Empty),
                        )
                    })
                    .expect("the independent policy combination should open")
                    .into();
                let platform_window = cx.test_window(handle);
                let expected_initial_activation =
                    usize::from(focus_on_appearing && accepts_activation);
                assert_eq!(
                    platform_window.initial_presentation_state(),
                    (true, true, expected_initial_activation)
                );

                let (creation, facts) = cx
                    .update_window(handle, |_, window, _| {
                        (
                            window.creation_facts().clone(),
                            window.platform_facts().clone(),
                        )
                    })
                    .expect("the policy test window should remain live");
                assert_eq!(creation.focus_on_appearing, focus_on_appearing);
                assert_eq!(facts.accepts_activation, accepts_activation);
                assert_eq!(facts.focus_on_click, focus_on_click);

                let _ = cx
                    .update_window(handle, |_, window, _| window.activate_window())
                    .expect("the policy test window should accept a framework command");
                cx.run_until_parked();
                assert_eq!(
                    platform_window.initial_presentation_state().2,
                    expected_initial_activation + usize::from(accepts_activation),
                    "programmatic activation must depend only on lifetime activation policy"
                );

                cx.update_window(handle, |_, window, app| window.remove_window(app))
                    .expect("the policy test window should close");
            }
        }
    }

    #[crate::test]
    fn frame_requested_while_app_is_borrowed_draws_after_update(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let generation_before = cx
            .update_window(handle, |_, window, _| window.rendered_frame.generation)
            .unwrap();

        cx.update_window(handle, |_, window, _| {
            assert!(platform_window.simulate_frame(RequestFrameOptions {
                force_render: true,
                require_presentation: true,
            }));
            assert_eq!(
                window.rendered_frame.generation, generation_before,
                "a queued frame must not draw recursively inside the active update"
            );
        })
        .unwrap();

        cx.run_until_parked();

        assert!(
            cx.update_window(handle, |_, window, _| window.rendered_frame.generation)
                .unwrap()
                > generation_before,
            "the accepted queued frame must eventually draw after the App borrow is released"
        );
    }

    #[crate::test]
    fn mutation_domains_are_isolated_and_invalid_placement_preserves_existing_ticket(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open_test_window(cx);
        let placement_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(30.0, 40.0, 410.0, 290.0)),
        );
        let pointer_ticket = cx
            .update_window(handle, |_, window, _| {
                match window.request_pointer_input(false) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected queued pointer-input dispatch, got {dispatch:?}"),
                }
            })
            .unwrap();

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::PointerInput));
        assert!(placement_ticket.observation().is_none());
        assert_eq!(
            pointer_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );

        let contradictory = WindowPlacementRequest {
            position: Some(Point::new(px(1.0), px(2.0))),
            state: Some(WindowPlacementState::Maximized),
            ..WindowPlacementRequest::new()
        };
        assert!(matches!(
            cx.update_window(handle, |_, window, _| {
                window.request_window_placement_request(contradictory)
            })
            .unwrap(),
            WindowMutationDispatch::Rejected
        ));
        assert!(
            placement_ticket.observation().is_none(),
            "validation failure must not supersede an already pending placement"
        );

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert_eq!(
            placement_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );

        let partial_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest {
                size: Some(size(px(415.0), px(295.0))),
                ..WindowPlacementRequest::new()
            },
        );
        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert_eq!(
            partial_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
    }

    #[crate::test]
    fn invalid_numeric_placement_preserves_existing_ticket(cx: &mut TestAppContext) {
        let (handle, _) = open_test_window(cx);
        let pending_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(30.0, 40.0, 410.0, 290.0)),
        );
        let invalid_requests = [
            WindowPlacementRequest {
                position: Some(Point::new(px(f32::NAN), px(2.0))),
                ..WindowPlacementRequest::new()
            },
            WindowPlacementRequest {
                size: Some(size(px(0.0), px(240.0))),
                ..WindowPlacementRequest::new()
            },
            WindowPlacementRequest {
                size: Some(size(px(320.0), px(-1.0))),
                ..WindowPlacementRequest::new()
            },
            WindowPlacementRequest::maximized(bounds(0.0, 0.0, f32::INFINITY, 240.0)),
        ];

        for request in invalid_requests {
            assert!(matches!(
                cx.update_window(handle, |_, window, _| {
                    window.request_window_placement_request(request)
                })
                .unwrap(),
                WindowMutationDispatch::Rejected
            ));
            assert!(
                pending_ticket.observation().is_none(),
                "invalid numeric input must not supersede a pending placement"
            );
        }

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert_eq!(
            pending_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
    }

    #[crate::test]
    fn valid_requests_supersede_same_domain_and_subscription_drop_only_cancels_delivery(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        let superseded_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(12.0, 18.0, 420.0, 260.0)),
        );
        let ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(20.0, 28.0, 430.0, 270.0)),
        );

        assert_eq!(
            superseded_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );

        let delivered = Rc::new(Cell::new(false));
        let subscription = ticket.subscribe({
            let delivered = delivered.clone();
            move |_| delivered.set(true)
        });
        drop(subscription);

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert!(!delivered.get());
        assert_eq!(
            ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
        assert!(
            !platform_window.flush_window_mutation(WindowMutationDomain::Placement),
            "the test backend must not retain the superseded placement callback"
        );
    }

    #[crate::test]
    fn synchronous_dispatches_supersede_pending_requests_and_close_settles_tickets(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        let unsupported_predecessor = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(12.0, 18.0, 420.0, 260.0)),
        );
        platform_window.set_next_placement_dispatch(PlatformWindowDispatch::Unsupported);
        assert!(matches!(
            cx.update_window(handle, |_, window, _| {
                window.request_window_placement_request(WindowPlacementRequest::windowed(bounds(
                    24.0, 36.0, 440.0, 280.0,
                )))
            })
            .unwrap(),
            WindowMutationDispatch::Unsupported
        ));
        assert_eq!(
            unsupported_predecessor.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );
        let finishes = platform_window.window_mutation_unobserved_finish_history();
        assert_eq!(finishes.len(), 1);
        assert_eq!(finishes[0].0, WindowMutationDomain::Placement);
        assert_eq!(
            finishes[0].2,
            PlatformWindowMutationUnobservedTerminal::Unsupported
        );

        let unchanged_predecessor = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(30.0, 42.0, 460.0, 300.0)),
        );
        let current_bounds = cx
            .update_window(handle, |_, window, _| window.bounds())
            .unwrap();
        assert!(matches!(
            cx.update_window(handle, |_, window, _| {
                window.request_window_placement_request(WindowPlacementRequest::windowed(
                    current_bounds,
                ))
            })
            .unwrap(),
            WindowMutationDispatch::Unchanged
        ));
        assert_eq!(
            unchanged_predecessor.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );
        assert!(
            !platform_window.flush_window_mutation(WindowMutationDomain::Placement),
            "an unchanged replacement must invalidate the older backend placement task"
        );
        let finishes = platform_window.window_mutation_unobserved_finish_history();
        assert_eq!(finishes.len(), 2);
        assert!(finishes[0].1 < finishes[1].1);
        assert_eq!(finishes[1].0, WindowMutationDomain::Placement);
        assert_eq!(
            finishes[1].2,
            PlatformWindowMutationUnobservedTerminal::Unchanged
        );

        platform_window.set_next_placement_dispatch(PlatformWindowDispatch::Rejected);
        assert!(matches!(
            cx.update_window(handle, |_, window, _| {
                window.request_window_placement_request(WindowPlacementRequest::windowed(bounds(
                    34.0, 46.0, 470.0, 310.0,
                )))
            })
            .unwrap(),
            WindowMutationDispatch::Rejected
        ));
        platform_window.set_next_placement_dispatch(PlatformWindowDispatch::WindowClosed);
        assert!(matches!(
            cx.update_window(handle, |_, window, _| {
                window.request_window_placement_request(WindowPlacementRequest::windowed(bounds(
                    36.0, 47.0, 475.0, 315.0,
                )))
            })
            .unwrap(),
            WindowMutationDispatch::WindowClosed
        ));
        let finishes = platform_window.window_mutation_unobserved_finish_history();
        assert_eq!(
            finishes
                .iter()
                .map(|(_, _, terminal)| *terminal)
                .collect::<Vec<_>>(),
            [
                PlatformWindowMutationUnobservedTerminal::Unsupported,
                PlatformWindowMutationUnobservedTerminal::Unchanged,
                PlatformWindowMutationUnobservedTerminal::Rejected,
                PlatformWindowMutationUnobservedTerminal::WindowClosed,
            ]
        );
        assert!(finishes.windows(2).all(|pair| pair[0].1 < pair[1].1));

        let close_ticket = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(38.0, 48.0, 480.0, 320.0)),
        );
        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .unwrap();
        assert_eq!(
            close_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::WindowClosed
        );
    }

    #[crate::test]
    fn placement_helpers_share_generation_and_preserve_committed_facts_until_observation(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open_test_window(cx);
        let initial_bounds = cx
            .update_window(handle, |_, window, _| window.bounds())
            .unwrap();
        let predecessor = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(20.0, 30.0, 420.0, 260.0)),
        );
        let fullscreen = cx
            .update_window(handle, |_, window, _| match window.toggle_fullscreen() {
                WindowMutationDispatch::Queued(ticket) => ticket,
                dispatch => panic!("expected queued fullscreen dispatch, got {dispatch:?}"),
            })
            .unwrap();
        let maximized = cx
            .update_window(handle, |_, window, _| match window.zoom_window() {
                WindowMutationDispatch::Queued(ticket) => ticket,
                dispatch => panic!("expected queued zoom dispatch, got {dispatch:?}"),
            })
            .unwrap();
        let minimized = cx
            .update_window(handle, |_, window, _| match window.minimize_window() {
                WindowMutationDispatch::Queued(ticket) => ticket,
                dispatch => panic!("expected queued minimize dispatch, got {dispatch:?}"),
            })
            .unwrap();

        assert_eq!(
            predecessor.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );
        assert_eq!(
            fullscreen.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );
        assert_eq!(
            maximized.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );
        assert_eq!(
            [
                fullscreen.generation(),
                maximized.generation(),
                minimized.generation()
            ],
            [
                predecessor.generation() + 1,
                predecessor.generation() + 2,
                predecessor.generation() + 3
            ]
        );
        cx.update_window(handle, |_, window, _| {
            assert_eq!(window.bounds(), initial_bounds);
            assert!(!window.is_fullscreen());
            assert!(!window.is_minimized());
        })
        .unwrap();

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert_eq!(
            minimized.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
        assert!(
            cx.update_window(handle, |_, window, _| window.is_minimized())
                .unwrap()
        );
    }

    #[crate::test]
    fn external_window_state_callback_refreshes_committed_facts(cx: &mut TestAppContext) {
        let (handle, _) = open_test_window(cx);
        assert!(
            !cx.update_window(handle, |_, window, _| window.is_minimized())
                .unwrap()
        );

        cx.simulate_window_minimize(handle);

        assert!(
            cx.update_window(handle, |_, window, _| window.is_minimized())
                .unwrap(),
            "an external state callback must refresh the same facts cache used by public getters"
        );
    }

    #[crate::test]
    fn stale_terminal_generation_cannot_settle_new_ticket_or_replace_committed_facts(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open_test_window(cx);
        let first = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(20.0, 30.0, 400.0, 260.0)),
        );
        let second_bounds = bounds(50.0, 60.0, 520.0, 340.0);
        let second = queue_placement(cx, handle, WindowPlacementRequest::windowed(second_bounds));
        assert_eq!(
            first.observation().unwrap().outcome,
            WindowMutationOutcome::Superseded
        );

        let committed_before = cx
            .update_window(handle, |_, window, _| window.platform_facts().clone())
            .unwrap();
        let mut stale_facts = committed_before.clone();
        let stale_bounds = bounds(5.0, 7.0, 200.0, 140.0);
        stale_facts.bounds = stale_bounds;
        stale_facts.window_bounds = WindowBounds::Windowed(stale_bounds);
        stale_facts.inner_window_bounds = WindowBounds::Windowed(stale_bounds);
        stale_facts.content_size = stale_bounds.size;

        assert!(cx.simulate_window_mutation_terminal_for_generation(
            handle,
            WindowMutationDomain::Placement,
            first.generation(),
            PlatformWindowMutationTerminal::Observed,
            stale_facts,
        ));
        assert!(second.observation().is_none());
        assert_eq!(
            cx.update_window(handle, |_, window, _| window.platform_facts().clone())
                .unwrap(),
            committed_before,
            "a stale terminal callback must be ignored before committing its facts"
        );

        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert_eq!(
            second.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
        assert_eq!(
            cx.update_window(handle, |_, window, _| window.bounds())
                .unwrap(),
            second_bounds
        );
    }

    #[crate::test]
    fn activation_policy_is_one_coherent_domain_independent_from_other_flags(
        cx: &mut TestAppContext,
    ) {
        cx.set_platform_window_mutation_capabilities(PlatformWindowMutationCapabilities {
            activation_policy: WindowMutationSupport::Live,
            alpha: WindowMutationSupport::Live,
            topmost: WindowMutationSupport::Live,
            taskbar_visibility: WindowMutationSupport::Live,
            ..Default::default()
        });
        let (handle, _) = open_test_window(cx);
        let requests = [
            WindowMutationRequest::ActivationPolicy(WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: true,
            }),
            WindowMutationRequest::Alpha(WindowBackgroundAppearance::Transparent),
            WindowMutationRequest::Topmost(true),
            WindowMutationRequest::TaskbarVisibility(false),
        ];
        let tickets = cx
            .update_window(handle, |_, window, _| {
                requests
                    .into_iter()
                    .map(|request| match window.request_window_mutation(request) {
                        WindowMutationDispatch::Queued(ticket) => ticket,
                        dispatch => panic!("expected queued flag dispatch, got {dispatch:?}"),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap();

        for ticket in &tickets {
            assert_eq!(ticket.generation(), 1);
            assert!(ticket.observation().is_none());
        }
        for domain in [
            WindowMutationDomain::ActivationPolicy,
            WindowMutationDomain::Alpha,
            WindowMutationDomain::Topmost,
            WindowMutationDomain::TaskbarVisibility,
        ] {
            assert!(cx.flush_window_mutation(handle, domain));
        }
        assert!(tickets.iter().all(|ticket| {
            ticket
                .observation()
                .is_some_and(|observation| observation.outcome == WindowMutationOutcome::Exact)
        }));
        let facts = cx
            .update_window(handle, |_, window, _| window.platform_facts().clone())
            .unwrap();
        assert!(!facts.accepts_activation);
        assert!(facts.focus_on_click);
        assert_eq!(
            facts.background_appearance,
            WindowBackgroundAppearance::Transparent
        );
        assert!(facts.topmost);
        assert!(!facts.taskbar_visible);
    }

    #[crate::test]
    fn activation_policy_preserves_all_independent_field_combinations(cx: &mut TestAppContext) {
        cx.set_platform_window_mutation_capabilities(PlatformWindowMutationCapabilities {
            activation_policy: WindowMutationSupport::Live,
            ..Default::default()
        });
        let (handle, _) = open_test_window(cx);

        for (generation, accepts_activation, focus_on_click) in [
            (1, false, false),
            (2, false, true),
            (3, true, false),
            (4, true, true),
        ] {
            let policy = WindowActivationPolicy {
                accepts_activation,
                focus_on_click,
            };
            let ticket = cx
                .update_window(handle, |_, window, _| {
                    match window.request_activation_policy(policy) {
                        WindowMutationDispatch::Queued(ticket) => ticket,
                        dispatch => {
                            panic!("expected queued activation-policy dispatch, got {dispatch:?}")
                        }
                    }
                })
                .unwrap();
            assert_eq!(ticket.generation(), generation);
            assert_eq!(ticket.domain(), WindowMutationDomain::ActivationPolicy);
            assert!(cx.flush_window_mutation(handle, WindowMutationDomain::ActivationPolicy));
            assert_eq!(
                ticket.observation().unwrap().outcome,
                WindowMutationOutcome::Exact
            );

            let facts = cx
                .update_window(handle, |_, window, _| window.platform_facts().clone())
                .unwrap();
            assert_eq!(facts.accepts_activation, accepts_activation);
            assert_eq!(facts.focus_on_click, focus_on_click);
        }
    }

    #[crate::test]
    fn closing_invalidates_all_queued_backend_domains_before_delivery(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let placement = queue_placement(
            cx,
            handle,
            WindowPlacementRequest::windowed(bounds(40.0, 50.0, 430.0, 280.0)),
        );
        let pointer = cx
            .update_window(handle, |_, window, _| {
                match window.request_pointer_input(false) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected queued pointer dispatch, got {dispatch:?}"),
                }
            })
            .unwrap();
        let native_before = platform_window.platform_facts();

        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .unwrap();

        assert_eq!(
            placement.observation().unwrap().outcome,
            WindowMutationOutcome::WindowClosed
        );
        assert_eq!(
            pointer.observation().unwrap().outcome,
            WindowMutationOutcome::WindowClosed
        );
        assert!(!platform_window.flush_window_mutation(WindowMutationDomain::Placement));
        assert!(!platform_window.flush_window_mutation(WindowMutationDomain::PointerInput));
        let mut expected_after_close = native_before;
        expected_after_close.accepts_pointer_input = false;
        expected_after_close.accepts_activation = false;
        expected_after_close.focus_on_click = false;
        expected_after_close.is_active = false;
        assert_eq!(platform_window.platform_facts(), expected_after_close);
    }

    #[crate::test]
    fn unsupported_minimize_entry_does_not_block_live_restore(cx: &mut TestAppContext) {
        cx.set_platform_window_mutation_capabilities(PlatformWindowMutationCapabilities {
            windowed: WindowMutationSupport::Live,
            ..Default::default()
        });
        let (handle, _) = open_test_window(cx);
        cx.simulate_window_minimize(handle);

        let ticket = cx
            .update_window(handle, |_, window, _| {
                match window.request_window_placement_request(WindowPlacementRequest {
                    state: Some(WindowPlacementState::Windowed),
                    ..WindowPlacementRequest::new()
                }) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected queued restore dispatch, got {dispatch:?}"),
                }
            })
            .unwrap();
        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::Placement));
        assert_eq!(
            ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
        assert!(
            !cx.update_window(handle, |_, window, _| window.is_minimized())
                .unwrap()
        );
    }

    #[crate::test]
    fn creation_placement_seeds_the_committed_facts_cache(cx: &mut TestAppContext) {
        let restore_bounds = bounds(100.0, 120.0, 600.0, 400.0);
        let handle = {
            let mut app = cx.app.borrow_mut();
            app.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Maximized(restore_bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
            .unwrap()
        };
        let handle: AnyWindowHandle = handle.into();
        let facts = cx
            .update_window(handle, |_, window, _| window.platform_facts().clone())
            .unwrap();

        assert_eq!(facts.bounds, restore_bounds);
        assert_eq!(facts.window_bounds, WindowBounds::Maximized(restore_bounds));
        assert!(facts.is_maximized);
        assert_eq!(
            cx.update_window(handle, |_, window, _| window.window_bounds())
                .unwrap(),
            WindowBounds::Maximized(restore_bounds)
        );
    }
}

#[cfg(test)]
mod platform_command_tests {
    use super::*;
    use crate::{
        AppContext, Empty, FileDropEvent, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
        ModifiersChangedEvent, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MousePressureEvent,
        MouseUpEvent, PinchEvent, PointerCancelEvent, PointerCancelReason, ScrollWheelEvent,
        TestAppContext, WindowActivationCancellationOutcome, WindowActivationPolicy,
        WindowActivationStatus, WindowActivationTerminal, WindowMutationDispatch,
        WindowMutationOutcome, point, px, size,
    };
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    fn open_test_window(cx: &mut TestAppContext) -> (AnyWindowHandle, TestWindow) {
        let handle = cx.open_window(size(px(320.0), px(240.0)), |_, _| Empty);
        let handle = handle.into();
        let platform_window = cx.test_window(handle);
        platform_window.clear_platform_command_history();
        (handle, platform_window)
    }

    fn mouse_move_input(x: f32) -> PlatformInput {
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(10.0)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        })
    }

    fn every_platform_input_class() -> Vec<(&'static str, PlatformInput)> {
        let keystroke = Keystroke::parse("escape").expect("escape should parse");
        vec![
            (
                "key-down",
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                }),
            ),
            (
                "key-up",
                PlatformInput::KeyUp(KeyUpEvent {
                    keystroke: keystroke.clone(),
                }),
            ),
            (
                "modifiers-changed",
                PlatformInput::ModifiersChanged(ModifiersChangedEvent::default()),
            ),
            (
                "mouse-down",
                PlatformInput::MouseDown(MouseDownEvent::default()),
            ),
            ("mouse-up", PlatformInput::MouseUp(MouseUpEvent::default())),
            (
                "mouse-pressure",
                PlatformInput::MousePressure(MousePressureEvent::default()),
            ),
            ("mouse-move", mouse_move_input(10.0)),
            (
                "mouse-exited",
                PlatformInput::MouseExited(MouseExitEvent::default()),
            ),
            (
                "pointer-cancelled",
                PlatformInput::PointerCanceled(PointerCancelEvent {
                    reason: PointerCancelReason::CaptureRevoked,
                }),
            ),
            (
                "scroll-wheel",
                PlatformInput::ScrollWheel(ScrollWheelEvent::default()),
            ),
            ("pinch", PlatformInput::Pinch(PinchEvent::default())),
            (
                "file-drop",
                PlatformInput::FileDrop(FileDropEvent::Pending {
                    position: point(px(10.0), px(10.0)),
                }),
            ),
        ]
    }

    #[crate::test]
    fn platform_window_command_runs_after_outer_app_borrow_is_released(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let callback_ran = Rc::new(Cell::new(false));
        let app = cx.app.clone();
        platform_window.set_platform_command_callback({
            let callback_ran = callback_ran.clone();
            move |command, _| {
                assert!(matches!(command, PlatformWindowCommand::Activate { .. }));
                let app_borrow = app
                    .try_borrow_mut()
                    .expect("platform commands must run after the outer AppRefMut is released");
                drop(app_borrow);
                callback_ran.set(true);
                PlatformWindowCommandOutcome::Accepted
            }
        });

        cx.update_window(handle, |_, window, _| {
            let _ = window.activate_window();
            assert!(
                !callback_ran.get(),
                "platform commands must stay queued while the outer AppRefMut is active"
            );
        })
        .expect("test window should remain live");

        assert!(callback_ran.get());
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::Activate {
                request_generation: 1,
            }]
        );
    }

    #[crate::test]
    fn synchronous_exact_activation_before_accepted_dispatch_settles_ticket(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        platform_window.set_platform_command_callback(|command, mut platform_window| {
            assert!(matches!(command, PlatformWindowCommand::Activate { .. }));
            platform_window.simulate_active_status_observation(
                PlatformWindowActiveStatusObservation::new(true, true),
            );
            let _ = platform_window.simulate_input_result(mouse_move_input(24.0));
            PlatformWindowCommandOutcome::Accepted
        });

        let ticket = cx
            .update_window(handle, |_, window, _| window.activate_window())
            .expect("test window should remain live");
        cx.run_until_parked();

        assert_eq!(
            ticket.snapshot().status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::Activated)
        );
    }

    #[crate::test]
    fn synchronous_exact_activation_cannot_override_rejected_dispatch(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        platform_window.set_platform_command_callback(|command, mut platform_window| {
            assert!(matches!(command, PlatformWindowCommand::Activate { .. }));
            platform_window.simulate_active_status_observation(
                PlatformWindowActiveStatusObservation::new(true, true),
            );
            let _ = platform_window.simulate_input_result(mouse_move_input(28.0));
            PlatformWindowCommandOutcome::Rejected
        });

        let ticket = cx
            .update_window(handle, |_, window, _| window.activate_window())
            .expect("test window should remain live");
        cx.run_until_parked();

        assert_eq!(
            ticket.snapshot().status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::Rejected)
        );
    }

    #[crate::test]
    fn explicit_activation_cancellation_terminals_are_first_and_delivered_once(
        cx: &mut TestAppContext,
    ) {
        cx.set_platform_focused_window_available(false);

        for terminal in [
            WindowActivationTerminal::Cancelled,
            WindowActivationTerminal::TargetReplaced,
        ] {
            let (handle, platform_window) = open_test_window(cx);
            platform_window.set_platform_command_callback(|command, _| {
                assert!(matches!(command, PlatformWindowCommand::Activate { .. }));
                PlatformWindowCommandOutcome::Accepted
            });
            let ticket = cx
                .update_window(handle, |_, window, _| window.activate_window())
                .expect("test window should remain live");
            assert_eq!(
                ticket.snapshot().status(),
                WindowActivationStatus::Dispatched
            );

            let delivered = Rc::new(RefCell::new(Vec::new()));
            let _subscription = ticket.subscribe({
                let delivered = delivered.clone();
                move |snapshot| delivered.borrow_mut().push(snapshot.status())
            });
            let cancellation = match terminal {
                WindowActivationTerminal::Cancelled => ticket.cancel(),
                WindowActivationTerminal::TargetReplaced => ticket.cancel_for_target_replacement(),
                _ => unreachable!("the test only covers explicit cancellation terminals"),
            };
            assert_eq!(
                cancellation,
                WindowActivationCancellationOutcome::Installed(terminal)
            );
            cx.run_until_parked();
            assert_eq!(
                ticket.snapshot().status(),
                WindowActivationStatus::Terminal(terminal)
            );
            assert_eq!(
                delivered.borrow().as_slice(),
                &[WindowActivationStatus::Terminal(terminal)]
            );

            platform_window.simulate_active_status_observation(
                PlatformWindowActiveStatusObservation::new(true, true),
            );
            cx.run_until_parked();
            assert_eq!(
                ticket.snapshot().status(),
                WindowActivationStatus::Terminal(terminal),
                "a later exact native positive must not replace explicit cancellation"
            );
            assert_eq!(delivered.borrow().len(), 1);
            assert_eq!(
                ticket.cancel(),
                WindowActivationCancellationOutcome::AlreadyTerminal(terminal)
            );
        }
    }

    #[crate::test]
    fn panicking_activation_dispatch_settles_ticket_before_propagation(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        platform_window.set_platform_command_callback(|command, _| {
            assert!(matches!(command, PlatformWindowCommand::Activate { .. }));
            panic!("injected activation command panic");
        });
        let ticket = Rc::new(RefCell::new(None));

        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update_window(handle, |_, window, _| {
                ticket.replace(Some(window.activate_window()));
            })
            .expect("the activation target must remain live");
        }))
        .expect_err("the activation dispatcher panic must propagate after settlement");
        let panic_message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
        assert_eq!(panic_message, Some("injected activation command panic"));
        assert_eq!(
            ticket
                .borrow()
                .as_ref()
                .expect("the command must publish its ticket before dispatch")
                .snapshot()
                .status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::Rejected)
        );
    }

    #[crate::test]
    fn dropping_pending_activation_subscription_cancels_terminal_delivery(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        platform_window.set_platform_command_callback(|command, _| {
            assert!(matches!(command, PlatformWindowCommand::Activate { .. }));
            PlatformWindowCommandOutcome::Rejected
        });
        let delivered = Rc::new(Cell::new(false));
        let ticket = cx
            .update_window(handle, {
                let delivered = delivered.clone();
                move |_, window, _| {
                    let ticket = window.activate_window();
                    let subscription = ticket.subscribe(move |_| delivered.set(true));
                    drop(subscription);
                    ticket
                }
            })
            .expect("test window should remain live");
        cx.run_until_parked();

        assert!(!delivered.get());
        assert_eq!(
            ticket.snapshot().status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::Rejected)
        );
    }

    #[crate::test]
    fn dropping_already_terminal_activation_subscription_cancels_deferred_delivery(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open_test_window(cx);
        let dispatch = cx
            .update_window(handle, |_, window, _| {
                window.request_activation_policy(crate::WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: true,
                })
            })
            .expect("test window should remain live");
        assert!(matches!(dispatch, crate::WindowMutationDispatch::Queued(_)));
        assert!(cx.flush_window_mutation(handle, WindowMutationDomain::ActivationPolicy));
        let delivered = Rc::new(Cell::new(false));
        let ticket = cx
            .update_window(handle, {
                let delivered = delivered.clone();
                move |_, window, _| {
                    let ticket = window.activate_window();
                    let subscription = ticket.subscribe(move |_| delivered.set(true));
                    drop(subscription);
                    ticket
                }
            })
            .expect("test window should remain live");
        cx.run_until_parked();

        assert!(!delivered.get());
        assert_eq!(
            ticket.snapshot().status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::Rejected)
        );
    }

    #[crate::test]
    fn nested_platform_window_commands_are_fifo_and_non_recursive(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let menu_position = point(px(24.0), px(36.0));
        let nested_command =
            PlatformWindowCommand::StartWindowResize(crate::ResizeEdge::BottomRight);
        let callback_active = Rc::new(Cell::new(false));
        let callback_history = Rc::new(RefCell::new(Vec::new()));
        let app = cx.app.clone();
        let window_id = handle.window_id();
        platform_window.set_platform_command_callback({
            let callback_active = callback_active.clone();
            let callback_history = callback_history.clone();
            move |command, platform_window| {
                assert!(
                    !callback_active.replace(true),
                    "nested platform commands must not dispatch recursively"
                );
                callback_history.borrow_mut().push(command);
                if command == PlatformWindowCommand::StartWindowMove {
                    app.enqueue_platform_window_command(
                        window_id,
                        platform_window.command_dispatcher(),
                        nested_command,
                    );
                }
                callback_active.set(false);
                PlatformWindowCommandOutcome::Accepted
            }
        });

        cx.update_window(handle, |_, window, _| {
            window.start_window_move();
            window.show_window_menu(menu_position);
            assert!(
                platform_window.platform_command_history().is_empty(),
                "platform commands must not execute inside the outer window update"
            );
        })
        .expect("test window should remain live");

        let expected = [
            PlatformWindowCommand::StartWindowMove,
            PlatformWindowCommand::ShowWindowMenu(menu_position),
            nested_command,
        ];
        assert_eq!(platform_window.platform_command_history(), expected);
        assert_eq!(&*callback_history.borrow(), &expected);
        assert!(!callback_active.get());
    }

    #[crate::test]
    fn queued_interactive_platform_commands_are_rejected_after_window_quiescence(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
        let menu_position = point(px(24.0), px(36.0));
        let callback_count = Rc::new(Cell::new(0));
        platform_window.set_platform_command_callback({
            let callback_count = callback_count.clone();
            move |_, _| {
                callback_count.set(callback_count.get() + 1);
                PlatformWindowCommandOutcome::Accepted
            }
        });

        cx.update_window(handle, |_, window, _| {
            assert!(matches!(
                window.request_pointer_input(false),
                WindowMutationDispatch::Queued(_)
            ));
        })
        .expect("test window should remain live");
        assert!(platform_window.flush_window_mutation(WindowMutationDomain::PointerInput));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, _| {
            assert!(!window.platform_facts().accepts_pointer_input);
            assert!(window.platform_facts().accepts_activation);
        })
        .expect("disabled pointer-input facts should commit before the queued requests");

        let (pointer_ticket, activation_policy_ticket, activation_command_ticket) = cx
            .update_window(handle, |_, window, cx| {
                let pointer_ticket = match window.request_pointer_input(true) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected queued pointer-input mutation, got {dispatch:?}"),
                };
                let activation_policy_ticket =
                    match window.request_activation_policy(WindowActivationPolicy {
                        accepts_activation: false,
                        focus_on_click: false,
                    }) {
                        WindowMutationDispatch::Queued(ticket) => ticket,
                        dispatch => {
                            panic!("expected queued activation-policy mutation, got {dispatch:?}")
                        }
                    };
                let activation_command_ticket = window.activate_window();
                window.start_window_move();
                window.show_window_menu(menu_position);
                window.start_window_resize(crate::ResizeEdge::BottomRight);
                assert!(window.quiesce_interaction(cx));
                window.bounds_changed(cx);
                assert!(!window.is_window_active());
                assert!(matches!(
                    window.request_pointer_input(true),
                    WindowMutationDispatch::Rejected
                ));
                assert!(matches!(
                    window.request_activation_policy(WindowActivationPolicy::default()),
                    WindowMutationDispatch::Rejected
                ));
                (
                    pointer_ticket,
                    activation_policy_ticket,
                    activation_command_ticket,
                )
            })
            .expect("test window should remain live");

        assert_eq!(callback_count.get(), 0);
        assert!(platform_window.platform_command_history().is_empty());
        assert_eq!(
            activation_command_ticket.snapshot().status(),
            crate::WindowActivationStatus::Terminal(crate::WindowActivationTerminal::PolicyChanged,)
        );
        assert_eq!(
            pointer_ticket
                .observation()
                .expect("quiescence must settle pending pointer input")
                .outcome,
            WindowMutationOutcome::Rejected
        );
        assert_eq!(
            activation_policy_ticket
                .observation()
                .expect("quiescence must settle pending activation policy")
                .outcome,
            WindowMutationOutcome::Rejected
        );
        assert!(!platform_window.flush_window_mutation(WindowMutationDomain::PointerInput));
        assert!(!platform_window.flush_window_mutation(WindowMutationDomain::ActivationPolicy));
        let facts = platform_window.platform_facts();
        assert!(!facts.accepts_pointer_input);
        assert!(!facts.accepts_activation);
        assert!(!facts.focus_on_click);
        assert!(!facts.is_active);
    }

    #[crate::test]
    fn native_input_returns_exact_dispatch_result_without_busy_invariant_violations(
        cx: &mut TestAppContext,
    ) {
        let (handle, mut platform_window) = open_test_window(cx);
        let consume = Rc::new(Cell::new(false));
        let _interceptor = cx
            .update_window(handle, |_, window, _| {
                window.intercept_window_mouse_events({
                    let consume = consume.clone();
                    move |_, window, app| {
                        if consume.get() {
                            app.stop_propagation();
                            window.prevent_default();
                        }
                    }
                })
            })
            .expect("test window should remain live");
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        assert_eq!(
            platform_window.simulate_input_result(mouse_move_input(10.0)),
            DispatchEventResult {
                propagate: true,
                default_prevented: false,
            }
        );

        consume.set(true);
        assert_eq!(
            platform_window.simulate_input_result(mouse_move_input(20.0)),
            DispatchEventResult {
                propagate: false,
                default_prevented: true,
            }
        );
        let diagnostic_delta = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        assert_eq!(diagnostic_delta.omitted_before_cursor, 0);
        assert!(diagnostic_delta.terminal.iter().all(|diagnostic| !matches!(
            diagnostic.disposition,
            crate::NativeBoundaryDisposition::InvariantFailure(_)
        )));
        let input_diagnostics = diagnostic_delta
            .terminal
            .into_iter()
            .filter(|diagnostic| {
                diagnostic.kind
                    == crate::NativeBoundaryKind::Callback(crate::NativeCallbackKind::PlatformInput)
            })
            .collect::<Vec<_>>();
        assert_eq!(input_diagnostics.len(), 2);
        assert!(matches!(
            input_diagnostics[0].disposition,
            crate::NativeBoundaryDisposition::Delivered {
                input_result: Some(crate::NativeInputDeliveryResult {
                    propagate: true,
                    default_prevented: false,
                }),
            }
        ));
        assert!(matches!(
            input_diagnostics[1].disposition,
            crate::NativeBoundaryDisposition::Delivered {
                input_result: Some(crate::NativeInputDeliveryResult {
                    propagate: false,
                    default_prevented: true,
                }),
            }
        ));
        assert!(input_diagnostics.iter().all(|diagnostic| matches!(
            diagnostic.domain_generation,
            Some(crate::NativeBoundaryGeneration::InputSlot {
                boundary: crate::NativeInputBoundary::PlatformInput,
                ..
            })
        )));
    }

    #[crate::test]
    fn every_platform_input_class_returns_exact_propagated_and_consumed_results(
        cx: &mut TestAppContext,
    ) {
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        for (class, input) in every_platform_input_class() {
            let (handle, mut platform_window) = open_test_window(cx);
            assert_eq!(
                platform_window.simulate_input_result(input.clone()),
                DispatchEventResult {
                    propagate: true,
                    default_prevented: false,
                },
                "{class} must return its propagated handler result"
            );

            assert!(
                cx.update_window(handle, |_, window, app| window.quiesce_interaction(app))
                    .expect("test window should remain live"),
                "{class} should observe the one-way interaction boundary"
            );
            assert_eq!(
                platform_window.simulate_input_result(input),
                DispatchEventResult {
                    propagate: false,
                    default_prevented: true,
                },
                "{class} must return its consumed handler result"
            );
        }

        let diagnostic_delta = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        assert_eq!(diagnostic_delta.omitted_before_cursor, 0);
        assert!(diagnostic_delta.terminal.iter().all(|diagnostic| !matches!(
            diagnostic.disposition,
            crate::NativeBoundaryDisposition::InvariantFailure(_)
        )));
        let input_diagnostics = diagnostic_delta
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.kind
                    == crate::NativeBoundaryKind::Callback(crate::NativeCallbackKind::PlatformInput)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            input_diagnostics.len(),
            2 * every_platform_input_class().len()
        );
        assert!(input_diagnostics.iter().all(|diagnostic| matches!(
            diagnostic.domain_generation,
            Some(crate::NativeBoundaryGeneration::InputSlot {
                boundary: crate::NativeInputBoundary::PlatformInput,
                ..
            })
        )));
    }

    #[crate::test]
    fn panicking_native_input_records_one_terminal_and_restores_the_slot(cx: &mut TestAppContext) {
        let (handle, mut platform_window) = open_test_window(cx);
        let panic_once = Rc::new(Cell::new(true));
        let _interceptor = cx
            .update_window(handle, |_, window, _| {
                window.intercept_window_mouse_events({
                    let panic_once = panic_once.clone();
                    move |_, _, _| {
                        if panic_once.replace(false) {
                            panic!("injected native input callback panic");
                        }
                    }
                })
            })
            .expect("test window should remain live");
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        let panic = catch_unwind(AssertUnwindSafe(|| {
            platform_window.simulate_input_result(mouse_move_input(10.0))
        }));
        assert!(panic.is_err());

        let diagnostic_delta = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        let input_diagnostics = diagnostic_delta
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == crate::NativeBoundaryKind::Callback(
                            crate::NativeCallbackKind::PlatformInput,
                        )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            input_diagnostics.len(),
            1,
            "one panicking native callback must publish exactly one terminal diagnostic"
        );
        assert_eq!(
            input_diagnostics[0].disposition,
            crate::NativeBoundaryDisposition::InvariantFailure(
                crate::NativeInvariantFailure::CallbackPanicked,
            )
        );
        assert!(matches!(
            input_diagnostics[0].domain_generation,
            Some(crate::NativeBoundaryGeneration::InputSlot {
                boundary: crate::NativeInputBoundary::PlatformInput,
                ..
            })
        ));
        assert!(diagnostic_delta.pending.iter().all(|diagnostic| {
            diagnostic.target != crate::NativeBoundaryTarget::Window(handle.window_id())
                || diagnostic.kind
                    != crate::NativeBoundaryKind::Callback(crate::NativeCallbackKind::PlatformInput)
        }));

        assert_eq!(
            platform_window.simulate_input_result(mouse_move_input(20.0)),
            DispatchEventResult {
                propagate: true,
                default_prevented: false,
            },
            "unwinding must restore the checked-out platform input callback"
        );
    }

    #[crate::test]
    fn reentrant_native_input_reports_slot_reentry_instead_of_a_missing_slot(
        cx: &mut TestAppContext,
    ) {
        let (handle, mut platform_window) = open_test_window(cx);
        let failure = Rc::new(Cell::new(None));
        let mut nested_window = platform_window.clone();
        let _interceptor = cx
            .update_window(handle, {
                let failure = failure.clone();
                move |_, window, _| {
                    window.intercept_window_mouse_events(move |_, _, _| {
                        let panic = catch_unwind(AssertUnwindSafe(|| {
                            nested_window.simulate_input_result(mouse_move_input(20.0))
                        }))
                        .expect_err("reentering a checked-out native input slot must panic");
                        let violation = panic
                            .downcast_ref::<crate::NativeInputInvariantViolation>()
                            .expect("slot reentry must preserve the typed invariant violation");
                        failure.set(Some(violation.failure));
                    })
                }
            })
            .expect("test window should remain live");

        assert_eq!(
            platform_window.simulate_input_result(mouse_move_input(10.0)),
            DispatchEventResult {
                propagate: true,
                default_prevented: false,
            }
        );
        assert_eq!(
            failure.get(),
            Some(crate::NativeInvariantFailure::SlotReentry)
        );
    }

    #[crate::test]
    fn replacing_a_panicked_native_input_slot_invalidates_its_recovery_generation(
        cx: &mut TestAppContext,
    ) {
        let (handle, mut platform_window) = open_test_window(cx);
        let input_slot = platform_window.0.lock().input_callback.clone();
        let _interceptor = cx
            .update_window(handle, |_, window, _| {
                window.intercept_window_mouse_events(move |_, _, _| {
                    panic!("injected native input callback panic before slot replacement");
                })
            })
            .expect("test window should remain live");

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                platform_window.simulate_input_result(mouse_move_input(10.0))
            }))
            .is_err()
        );

        input_slot.set(PlatformInputCallback::new_for_window(
            cx.to_async(),
            handle.window_id(),
            Box::new(|_| DispatchEventResult {
                propagate: false,
                default_prevented: true,
            }),
        ));
        assert_eq!(
            input_slot.reserve_pointer_cancel_after_callback_panic(
                crate::PointerCancelReason::CaptureRevoked,
            ),
            crate::NativePointerCancelReservation::NoActiveCallback,
            "an old panic recovery must not reserve against a replacement generation"
        );
        assert_eq!(
            input_slot.dispatch(mouse_move_input(20.0)),
            DispatchEventResult {
                propagate: false,
                default_prevented: true,
            }
        );
    }

    #[crate::test]
    fn retired_native_input_slot_records_one_typed_terminal_diagnostic(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let input_slot = platform_window.0.lock().input_callback.clone();
        let diagnostic_cursor = cx
            .app
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        assert!(platform_window.simulate_close());
        cx.run_until_parked();
        assert!(!cx.windows().contains(&handle));
        let panic = catch_unwind(AssertUnwindSafe(|| {
            input_slot.dispatch(mouse_move_input(10.0))
        }))
        .expect_err("dispatching through a retired native input slot must panic");
        let violation = panic
            .downcast_ref::<crate::NativeInputInvariantViolation>()
            .expect("retired slot panic must preserve the typed invariant violation");
        assert_eq!(violation.window_id, handle.window_id());
        assert_eq!(
            violation.boundary,
            crate::NativeInputBoundary::PlatformInput
        );
        assert_eq!(
            violation.failure,
            crate::NativeInvariantFailure::RetiredSlot
        );

        let diagnostic_delta = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        let retired_diagnostics = diagnostic_delta
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == crate::NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == crate::NativeBoundaryKind::Callback(
                            crate::NativeCallbackKind::PlatformInput,
                        )
                    && diagnostic.disposition
                        == crate::NativeBoundaryDisposition::InvariantFailure(
                            crate::NativeInvariantFailure::RetiredSlot,
                        )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            retired_diagnostics.len(),
            1,
            "one retired-slot entry must publish exactly one terminal diagnostic"
        );
        assert!(matches!(
            retired_diagnostics[0].domain_generation,
            Some(crate::NativeBoundaryGeneration::InputSlot {
                boundary: crate::NativeInputBoundary::PlatformInput,
                generation,
            }) if Some(generation) == violation.slot_generation
        ));
    }

    #[crate::test]
    fn native_input_settles_more_than_one_async_drain_budget_of_older_events(
        cx: &mut TestAppContext,
    ) {
        let (_handle, mut platform_window) = open_test_window(cx);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let _subscriptions = cx.update(|app| {
            let keyboard = app.on_keyboard_layout_change({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("keyboard")
            });
            let thermal = app.on_thermal_state_change({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("thermal")
            });
            (keyboard, thermal)
        });
        let app_cell = cx.app.clone();
        let diagnostic_cursor = app_cell
            .native_boundary_diagnostics(crate::NativeBoundaryDiagnosticCursor::default())
            .cursor;

        cx.update(|_| {
            for _ in 0..65 {
                app_cell.enqueue_keyboard_layout_changed_for_test();
                app_cell.enqueue_thermal_state_changed_for_test();
            }
        });

        assert_eq!(
            platform_window.simulate_input_result(mouse_move_input(30.0)),
            DispatchEventResult {
                propagate: true,
                default_prevented: false,
            }
        );
        assert_eq!(observed.borrow().len(), 130);
        let diagnostic_delta = cx.app.native_boundary_diagnostics(diagnostic_cursor);
        assert_eq!(diagnostic_delta.omitted_before_cursor, 0);
        assert!(diagnostic_delta.terminal.iter().all(|diagnostic| !matches!(
            diagnostic.disposition,
            crate::NativeBoundaryDisposition::InvariantFailure(_)
        )));
    }
}

#[cfg(test)]
mod native_app_event_tests {
    use crate::{
        NativeBoundaryDiagnosticCursor, NativeBoundaryDisposition, NativeBoundaryKind,
        NativeCallbackKind, TestAppContext,
    };
    use std::{cell::RefCell, future, rc::Rc};

    #[crate::test]
    fn app_callbacks_wait_for_outer_borrow_and_quit_terminates_bounded_ingress(
        cx: &mut TestAppContext,
    ) {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let _subscriptions = cx.update(|app| {
            let keyboard = app.on_keyboard_layout_change({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("keyboard")
            });
            let thermal = app.on_thermal_state_change({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("thermal")
            });
            let quit = app.on_app_quit({
                let observed = observed.clone();
                move |_| {
                    observed.borrow_mut().push("quit");
                    future::ready(())
                }
            });
            (keyboard, thermal, quit)
        });

        let app = cx.app.clone();
        cx.update(|_| {
            app.enqueue_keyboard_layout_changed_for_test();
            app.enqueue_keyboard_layout_changed_for_test();
            app.enqueue_thermal_state_changed_for_test();
            app.enqueue_thermal_state_changed_for_test();

            for _ in 0..31 {
                app.enqueue_keyboard_layout_changed_for_test();
                app.enqueue_thermal_state_changed_for_test();
            }

            app.enqueue_quit_for_test();
            app.enqueue_thermal_state_changed_for_test();

            let diagnostics =
                app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default());
            assert_eq!(diagnostics.pending.len(), 66);
            assert!(diagnostics.pending.iter().all(|diagnostic| {
                diagnostic.disposition == NativeBoundaryDisposition::Pending
            }));
            assert_eq!(
                diagnostics
                    .terminal
                    .iter()
                    .filter(|diagnostic| matches!(
                        diagnostic.disposition,
                        NativeBoundaryDisposition::Coalesced { .. }
                    ))
                    .count(),
                2
            );
            assert!(
                observed.borrow().is_empty(),
                "native app callbacks must wait until the outer AppRefMut is released"
            );
        });

        cx.run_until_parked();

        let mut expected = Vec::with_capacity(65);
        for _ in 0..32 {
            expected.push("keyboard");
            expected.push("thermal");
        }
        expected.push("quit");
        assert_eq!(*observed.borrow(), expected);

        let diagnostics =
            app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default());
        assert!(diagnostics.pending.is_empty());
        let quit = diagnostics
            .terminal
            .iter()
            .find(|diagnostic| {
                diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::Quit)
            })
            .expect("quit callback should have a terminal diagnostic");
        assert_eq!(
            quit.disposition,
            NativeBoundaryDisposition::Delivered { input_result: None }
        );
        let after_quit = diagnostics
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.sequence > quit.sequence
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::ThermalStateChanged)
            })
            .collect::<Vec<_>>();
        assert_eq!(after_quit.len(), 1);
        assert_eq!(after_quit[0].disposition, NativeBoundaryDisposition::Closed);
    }
}

#[cfg(test)]
mod accessibility_tests {
    use super::*;
    use accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate};

    fn tree_update(nodes: Vec<(NodeId, Node)>) -> TreeUpdate {
        TreeUpdate {
            nodes,
            tree: Some(Tree::new(NodeId(0))),
            tree_id: TreeId::ROOT,
            focus: NodeId(0),
        }
    }

    fn callbacks() -> Rc<A11yCallbacks> {
        Rc::new(A11yCallbacks {
            activation: Box::new(|| None),
            action: Box::new(|_| {}),
            deactivation: Box::new(|| {}),
        })
    }

    #[test]
    fn accessibility_state_retains_only_current_active_activation_result() {
        let current = callbacks();
        let replacement = callbacks();
        let first = tree_update(vec![(NodeId(0), Node::new(Role::Window))]);
        let second = tree_update(vec![
            (NodeId(0), Node::new(Role::Window)),
            (NodeId(1), Node::new(Role::Button)),
        ]);
        let mut state = TestAccessibilityState {
            callbacks: Some(current.clone()),
            active: true,
            updates: Vec::new(),
        };

        state.retain_activation_result(&current, None);
        assert!(state.updates.is_empty());
        state.retain_activation_result(&current, Some(first.clone()));
        assert_eq!(state.updates, [first.clone()]);

        state.active = false;
        state.retain_activation_result(&current, Some(second.clone()));
        assert_eq!(state.updates, [first.clone()]);

        state.active = true;
        state.callbacks = Some(replacement);
        state.retain_activation_result(&current, Some(second));
        assert_eq!(state.updates, [first]);
    }

    #[test]
    fn accessibility_state_preserves_platform_delivery_order() {
        let first = tree_update(vec![(NodeId(0), Node::new(Role::Window))]);
        let second = tree_update(vec![
            (NodeId(0), Node::new(Role::Window)),
            (NodeId(1), Node::new(Role::Button)),
        ]);
        let mut state = TestAccessibilityState::default();

        state.record_platform_delivery(first.clone());
        state.record_platform_delivery(second.clone());
        assert_eq!(state.updates, [first.clone(), second.clone()]);

        state.active = true;
        state.record_platform_delivery(first.clone());
        assert_eq!(state.updates, [first.clone(), second, first]);
    }

    #[test]
    fn accessibility_normalization_preserves_relationship_order() {
        let mut root = Node::new(Role::Window);
        root.set_children([NodeId(2), NodeId(1)]);
        let update = tree_update(vec![
            (NodeId(2), Node::new(Role::Label)),
            (NodeId(0), root),
            (NodeId(1), Node::new(Role::Button)),
        ]);

        let normalized = normalize_accessibility_tree_update(update);
        assert_eq!(
            normalized
                .nodes
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [NodeId(0), NodeId(1), NodeId(2)]
        );
        assert_eq!(normalized.nodes[0].1.children(), &[NodeId(2), NodeId(1)]);
    }
}

pub(crate) struct TestAtlasState {
    epoch: AtlasTextureLeaseEpoch,
    next_id: u32,
    lease_acquire_calls: usize,
    lease_release_calls: usize,
    tiles: HashMap<AtlasKey, AtlasTile>,
    texture_refs: HashMap<AtlasTextureInstanceId, TestAtlasTextureRefs>,
}

#[derive(Default)]
struct TestAtlasTextureRefs {
    live_atlas_keys: u32,
    live_visual_leases: u32,
}

pub(crate) struct TestAtlas(Mutex<TestAtlasState>);

impl TestAtlas {
    pub fn new() -> Self {
        TestAtlas(Mutex::new(TestAtlasState {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_id: 0,
            lease_acquire_calls: 0,
            lease_release_calls: 0,
            tiles: HashMap::default(),
            texture_refs: HashMap::default(),
        }))
    }
}

impl PlatformAtlas for TestAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &crate::AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<crate::DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<crate::AtlasTile>> {
        let mut state = self.0.lock();
        if let Some(&tile) = state.tiles.get(key) {
            return Ok(Some(tile));
        }
        drop(state);

        let Some((size, _)) = build()? else {
            return Ok(None);
        };

        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;

        let tile = crate::AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind: key.texture_kind(),
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: crate::Bounds {
                origin: Point::default(),
                size,
            },
            texture_generation: texture_id,
            texture_generation_padding: 0,
        };
        state.insert_tile(key.clone(), tile);

        Ok(Some(tile))
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<AtlasAccess> {
        let mut state = self.0.lock();
        if let Some(&tile) = state.tiles.get(key) {
            return Ok(AtlasAccess {
                tile: Some(tile),
                diagnostic: AtlasAccessDiagnostic::new(
                    key,
                    AtlasAccessOutcome::Hit,
                    Some(tile),
                    Some(tile.bounds.size),
                ),
            });
        }
        drop(state);

        let Some((size, _)) = build()? else {
            return Ok(AtlasAccess {
                tile: None,
                diagnostic: AtlasAccessDiagnostic::new(
                    key,
                    AtlasAccessOutcome::Unavailable,
                    None,
                    None,
                ),
            });
        };

        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;
        let tile = crate::AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind: key.texture_kind(),
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: crate::Bounds {
                origin: Point::default(),
                size,
            },
            texture_generation: texture_id,
            texture_generation_padding: 0,
        };
        state.insert_tile(key.clone(), tile);

        Ok(AtlasAccess {
            tile: Some(tile),
            diagnostic: AtlasAccessDiagnostic::new(
                key,
                AtlasAccessOutcome::Inserted,
                Some(tile),
                Some(size),
            ),
        })
    }

    fn remove(&self, key: &AtlasKey) {
        let mut state = self.0.lock();
        state.remove_tile(key);
    }

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        let mut state = self.0.lock();
        let removed = state.remove_tile(key);
        let outcome = removed.map_or(AtlasRemoveOutcome::RemoveNoop, |tile| {
            if state.texture_refs.contains_key(&tile.texture_instance()) {
                AtlasRemoveOutcome::TextureRetained
            } else {
                AtlasRemoveOutcome::TextureFreed
            }
        });
        AtlasRemoveDiagnostic::new(key, outcome, removed.map(|tile| tile.texture_id))
    }

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        self.0.lock().epoch
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        debug_assert!(
            textures
                .iter()
                .enumerate()
                .all(|(index, texture)| !textures[..index].contains(texture)),
            "atlas texture lease acquisition requires deduplicated texture instances"
        );
        let mut state = self.0.lock();
        state.lease_acquire_calls += 1;
        let epoch = state.epoch;
        for texture in textures.iter().copied() {
            let Some(texture_refs) = state.texture_refs.get(&texture) else {
                return Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch });
            };
            if texture_refs.live_visual_leases == u32::MAX {
                return Err(AtlasTextureLeaseError::LeaseCountOverflow { texture, epoch });
            }
        }
        for texture in textures.iter().copied() {
            if let Some(texture_refs) = state.texture_refs.get_mut(&texture) {
                texture_refs.live_visual_leases += 1;
            }
        }
        Ok(epoch)
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        epoch: AtlasTextureLeaseEpoch,
        textures: &[AtlasTextureInstanceId],
    ) {
        debug_assert!(
            textures
                .iter()
                .enumerate()
                .all(|(index, texture)| !textures[..index].contains(texture)),
            "atlas texture lease release requires deduplicated texture instances"
        );
        let mut state = self.0.lock();
        state.lease_release_calls += 1;
        if state.epoch != epoch {
            return;
        }
        for texture in textures.iter().copied() {
            state.release_visual_lease(texture);
        }
    }
}

impl TestAtlasState {
    fn insert_tile(&mut self, key: AtlasKey, tile: AtlasTile) {
        self.texture_refs.insert(
            tile.texture_instance(),
            TestAtlasTextureRefs {
                live_atlas_keys: 1,
                live_visual_leases: 0,
            },
        );
        self.tiles.insert(key, tile);
    }

    fn remove_tile(&mut self, key: &AtlasKey) -> Option<AtlasTile> {
        let tile = self.tiles.remove(key)?;
        let remove_refs = self
            .texture_refs
            .get_mut(&tile.texture_instance())
            .is_some_and(|texture_refs| {
                if texture_refs.live_atlas_keys == 0 {
                    debug_assert!(false, "test atlas key count underflowed");
                } else {
                    texture_refs.live_atlas_keys -= 1;
                }
                texture_refs.live_atlas_keys == 0 && texture_refs.live_visual_leases == 0
            });
        if remove_refs {
            self.texture_refs.remove(&tile.texture_instance());
        }
        Some(tile)
    }

    fn release_visual_lease(&mut self, texture: AtlasTextureInstanceId) {
        let remove_refs = self
            .texture_refs
            .get_mut(&texture)
            .is_some_and(|texture_refs| {
                if texture_refs.live_visual_leases == 0 {
                    debug_assert!(false, "test atlas visual lease count underflowed");
                } else {
                    texture_refs.live_visual_leases -= 1;
                }
                texture_refs.live_atlas_keys == 0 && texture_refs.live_visual_leases == 0
            });
        if remove_refs {
            self.texture_refs.remove(&texture);
        }
    }
}

#[cfg(test)]
mod atlas_texture_lease_tests {
    use super::*;
    use crate::{ImageId, RenderImageParams};
    use std::borrow::Cow;

    fn insert_image(atlas: &TestAtlas, image_id: usize) -> anyhow::Result<(AtlasKey, AtlasTile)> {
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(image_id),
            frame_index: 0,
        });
        let mut build = || {
            Ok(Some((
                Size {
                    width: DevicePixels(1),
                    height: DevicePixels(1),
                },
                Cow::Borrowed(&[0_u8, 0, 0, 255][..]),
            )))
        };
        let tile = atlas
            .get_or_insert_with(&key, &mut build)?
            .expect("test image should allocate an atlas tile");
        Ok((key, tile))
    }

    #[test]
    fn texture_lease_deduplicates_and_delays_texture_retirement() -> anyhow::Result<()> {
        let atlas = Arc::new(TestAtlas::new());
        let (key, tile) = insert_image(&atlas, 1)?;
        let texture = tile.texture_instance();
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

        let lease = platform_atlas
            .retain_texture_instances(&[texture, texture])
            .expect("resident texture should be retainable");
        assert_eq!(lease.texture_instances(), &[texture]);

        atlas.remove(&key);
        {
            let state = atlas.0.lock();
            let refs = state
                .texture_refs
                .get(&texture)
                .expect("leased texture must remain resident after key removal");
            assert_eq!(refs.live_atlas_keys, 0);
            assert_eq!(refs.live_visual_leases, 1);
        }

        drop(lease);
        assert!(!atlas.0.lock().texture_refs.contains_key(&texture));
        Ok(())
    }

    #[test]
    fn empty_texture_lease_has_no_unsafe_acquire_or_release_obligation() {
        let atlas = Arc::new(TestAtlas::new());
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

        let lease = platform_atlas
            .retain_texture_instances(&[])
            .expect("an empty texture set should produce an inert lease");
        assert!(lease.texture_instances().is_empty());
        drop(lease);

        let state = atlas.0.lock();
        assert_eq!(state.lease_acquire_calls, 0);
        assert_eq!(state.lease_release_calls, 0);
    }

    #[test]
    fn texture_lease_acquisition_is_atomic_when_one_texture_is_unavailable() -> anyhow::Result<()> {
        let atlas = Arc::new(TestAtlas::new());
        let (_, tile) = insert_image(&atlas, 2)?;
        let texture = tile.texture_instance();
        let unavailable = AtlasTextureInstanceId {
            texture_id: AtlasTextureId {
                index: tile.texture_id.index + 100,
                kind: tile.texture_id.kind,
            },
            generation: texture.generation + 100,
        };
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

        assert!(matches!(
            platform_atlas.retain_texture_instances(&[texture, unavailable]),
            Err(AtlasTextureLeaseError::TextureUnavailable { texture: rejected, .. })
                if rejected == unavailable
        ));
        assert_eq!(atlas.0.lock().texture_refs[&texture].live_visual_leases, 0);
        Ok(())
    }

    #[test]
    fn texture_lease_reports_epoch_invalidation_after_renderer_reset() -> anyhow::Result<()> {
        let atlas = Arc::new(TestAtlas::new());
        let (_, tile) = insert_image(&atlas, 3)?;
        let texture = tile.texture_instance();
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
        let lease = platform_atlas
            .retain_texture_instances(&[texture])
            .expect("resident texture should be retainable");
        let expected_epoch = lease.epoch();
        let replacement = AtlasTextureInstanceId {
            texture_id: tile.texture_id,
            generation: texture.generation + 1,
        };

        let actual_epoch = {
            let mut state = atlas.0.lock();
            state.epoch = state.epoch.next();
            state.texture_refs.clear();
            state.tiles.clear();
            state.texture_refs.insert(
                replacement,
                TestAtlasTextureRefs {
                    live_atlas_keys: 1,
                    live_visual_leases: 7,
                },
            );
            state.epoch
        };

        assert_eq!(
            lease.validate(),
            Err(crate::AtlasTextureLeaseInvalidation {
                expected_epoch,
                actual_epoch,
            })
        );
        drop(lease);
        assert_eq!(
            atlas.0.lock().texture_refs[&replacement].live_visual_leases,
            7,
            "dropping an old-epoch lease must not mutate a replacement in the same slot"
        );
        Ok(())
    }
}
