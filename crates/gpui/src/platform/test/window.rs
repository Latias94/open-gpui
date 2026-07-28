#[cfg(test)]
use crate::DisplayId;
use crate::{
    A11yCallbacks, AnyWindowHandle, AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome,
    AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasTextureId, AtlasTile, Bounds,
    CursorStyle, DevicePixels, DispatchEventResult, GpuSpecs, Pixels, Platform, PlatformAtlas,
    PlatformDisplay, PlatformHeadlessRenderer, PlatformInput, PlatformInputCallback,
    PlatformInputCallbackSlot, PlatformInputHandler, PlatformInputHandlerSlot, PlatformWindow,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    PlatformWindowDispatch, PlatformWindowMutationObservation, PlatformWindowMutationTerminal,
    PlatformWindowPresentOutcome, Point, PromptButton, RequestFrameOptions, Scene, Size,
    TestPlatform, TileId, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowCreationFacts, WindowMutationDomain, WindowMutationRequest,
    WindowParams, WindowPlacementState, WindowPlatformFacts,
};
use image::RgbaImage;
use open_gpui_collections::{HashMap, VecDeque};
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    rc::{Rc, Weak},
    sync::{self, Arc},
};

pub(crate) struct TestWindowState {
    pub(crate) bounds: Bounds<Pixels>,
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
    hit_test_window_control_callback: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
    input_callback: PlatformInputCallbackSlot,
    active_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    hover_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    request_frame_callback: Option<Box<dyn FnMut(RequestFrameOptions)>>,
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
    next_mutation_dispatches: HashMap<WindowMutationDomain, PlatformWindowDispatch>,
    platform_command_callback:
        Option<Box<dyn FnMut(PlatformWindowCommand, TestWindow) -> PlatformWindowCommandOutcome>>,
    platform_command_history: Vec<PlatformWindowCommand>,
    initial_presentation_command_outcomes: VecDeque<PlatformWindowCommandOutcome>,
    show_on_initial_presentation: bool,
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
}

#[derive(Clone, Copy)]
struct TestWindowMutationRequest {
    generation: u64,
    request: WindowMutationRequest,
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
    ) -> Self {
        let sprite_atlas: Arc<dyn PlatformAtlas> = match &renderer {
            Some(r) => r.sprite_atlas(),
            None => Arc::new(TestAtlas::new()),
        };
        Self(
            Rc::new(Mutex::new(TestWindowState {
                bounds: params.bounds,
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
                next_mutation_dispatches: HashMap::default(),
                platform_command_callback: None,
                platform_command_history: Vec::new(),
                initial_presentation_command_outcomes: initial_presentation_command_outcomes
                    .unwrap_or_default(),
                show_on_initial_presentation: params.show,
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

    #[cfg(test)]
    pub(crate) fn platform_command_history(&self) -> Vec<PlatformWindowCommand> {
        self.0.lock().platform_command_history.clone()
    }

    #[cfg(test)]
    pub(crate) fn set_present_outcome(&self, outcome: PlatformWindowPresentOutcome) {
        self.0.lock().present_outcome = outcome;
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
                return PlatformWindowCommandOutcome::Rejected;
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
        if outcome == PlatformWindowCommandOutcome::Rejected {
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
            PlatformWindowCommand::Activate => {
                if !self.0.lock().accepts_activation {
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
        let mut lock = self.0.lock();
        lock.is_active = active;
        let Some(mut callback) = lock.active_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(active);
        let mut lock = self.0.lock();
        if !lock.closed {
            lock.active_status_change_callback = Some(callback);
        }
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

    /// Emits the current backend facts as a coherent terminal observation for one domain.
    pub fn emit_window_mutation_observation(&self, domain: WindowMutationDomain) -> bool {
        let mut lock = self.0.lock();
        let Some(generation) = lock.mutation_generations.get(&domain).copied() else {
            return false;
        };
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain);
        let facts = window_platform_facts(&lock);
        let Some(mut callback) = lock.mutation_observation_callback.take() else {
            return false;
        };
        drop(lock);
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
        let facts = window_platform_facts(&lock);
        let Some(mut callback) = lock.mutation_observation_callback.take() else {
            return false;
        };
        drop(lock);
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
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain || queued.generation != generation);
        apply_window_platform_facts(&mut lock, &facts);
        let Some(mut callback) = lock.mutation_observation_callback.take() else {
            return false;
        };
        drop(lock);
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
        let (input_callback, input_handler, callback, retired_callbacks) = {
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
        let callback = self.0.lock().input_callback.clone();
        callback.dispatch(event)
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
        let window = Rc::downgrade(&self.0);
        PlatformWindowCommandDispatcher::new(move |command| {
            if let Some(window) = window.upgrade() {
                TestWindow(window, false).execute_platform_command(command)
            } else {
                PlatformWindowCommandOutcome::Rejected
            }
        })
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.lock().bounds
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
        lock.mutation_generations.insert(domain, generation);
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain);
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        let mut lock = self.0.lock();
        lock.mutation_generations.remove(&domain);
        lock.pending_mutations
            .retain(|queued| queued.request.domain() != domain);
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
            });
        }
        dispatch
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn scale_factor(&self) -> f32 {
        2.0
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

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
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
        if let Some(renderer) = &mut state.renderer {
            let scale_factor = 2.0;
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
    WindowPlatformFacts {
        bounds: state.bounds,
        coordinate_space: crate::WindowCoordinateSpace::GlobalScreen,
        window_bounds: state.window_bounds,
        inner_window_bounds: state.window_bounds,
        content_size: state.bounds.size,
        scale_factor: 2.0,
        display_id: Some(state.display.id()),
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
        MouseButton, MouseDownEvent, MouseMoveEvent, PlatformWindowCreationCapabilities,
        PlatformWindowMutationCapabilities, QuitMode, Render, Styled, Subscription, TestAppContext,
        Window, WindowActivationPolicy, WindowCreationSupport, WindowInitialPresentationOrder,
        WindowInitialPresentationStatus, WindowKind, WindowMouseEvent, WindowMutationDispatch,
        WindowMutationOutcome, WindowMutationSupport, WindowMutationTicket, WindowOptions,
        WindowPlacementRequest, div, point, px, size,
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
    fn unsupported_transient_owner_is_rejected_before_native_creation(cx: &mut TestAppContext) {
        cx.set_platform_window_creation_capabilities(PlatformWindowCreationCapabilities {
            focus_on_appearing: WindowCreationSupport::Supported,
            transient_for: WindowCreationSupport::Unsupported,
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
    fn unavailable_display_falls_back_to_default_before_window_creation(cx: &mut TestAppContext) {
        let unavailable_display = DisplayId::from(999);
        let default_display = cx.read(|app| {
            app.primary_display()
                .expect("test platform should expose a primary display")
                .id()
        });
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

        assert_eq!(platform_window.simulate_should_close(), Some(true));
        app_cell.enqueue_keyboard_layout_changed_for_test();
        cx.run_until_parked();

        assert_eq!(
            deliveries.get(),
            2,
            "a nested wake blocked on the close query must resume when its App borrow is released"
        );
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
    fn initial_appearance_is_independent_from_lifetime_activation_policy(cx: &mut TestAppContext) {
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

                cx.update_window(handle, |_, window, _| window.activate_window())
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
        assert_eq!(platform_window.platform_facts(), native_before);
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
    use crate::{AppContext, Empty, Modifiers, MouseMoveEvent, TestAppContext, point, px, size};
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

    #[crate::test]
    fn platform_window_command_runs_after_outer_app_borrow_is_released(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
        let callback_ran = Rc::new(Cell::new(false));
        let app = cx.app.clone();
        platform_window.set_platform_command_callback({
            let callback_ran = callback_ran.clone();
            move |command, _| {
                assert_eq!(command, PlatformWindowCommand::Activate);
                let app_borrow = app
                    .try_borrow_mut()
                    .expect("platform commands must run after the outer AppRefMut is released");
                drop(app_borrow);
                callback_ran.set(true);
                PlatformWindowCommandOutcome::Accepted
            }
        });

        cx.update_window(handle, |_, window, _| {
            window.activate_window();
            assert!(
                !callback_ran.get(),
                "platform commands must stay queued while the outer AppRefMut is active"
            );
        })
        .expect("test window should remain live");

        assert!(callback_ran.get());
        assert_eq!(
            platform_window.platform_command_history(),
            [PlatformWindowCommand::Activate]
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
    next_id: u32,
    tiles: HashMap<AtlasKey, AtlasTile>,
}

pub(crate) struct TestAtlas(Mutex<TestAtlasState>);

impl TestAtlas {
    pub fn new() -> Self {
        TestAtlas(Mutex::new(TestAtlasState {
            next_id: 0,
            tiles: HashMap::default(),
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

        state.tiles.insert(
            key.clone(),
            crate::AtlasTile {
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
            },
        );

        Ok(Some(state.tiles[key]))
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
        };
        state.tiles.insert(key.clone(), tile);

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
        state.tiles.remove(key);
    }

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        let mut state = self.0.lock();
        let removed = state.tiles.remove(key);
        AtlasRemoveDiagnostic::new(
            key,
            if removed.is_some() {
                AtlasRemoveOutcome::RemoveHit
            } else {
                AtlasRemoveOutcome::RemoveNoop
            },
            removed.map(|tile| tile.texture_id),
        )
    }
}
