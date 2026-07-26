#[cfg(test)]
use crate::DisplayId;
use crate::{
    A11yCallbacks, AnyWindowHandle, AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome,
    AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasTextureId, AtlasTile, Bounds,
    CursorStyle, DevicePixels, DispatchEventResult, GpuSpecs, Pixels, Platform, PlatformAtlas,
    PlatformDisplay, PlatformHeadlessRenderer, PlatformInput, PlatformInputHandler, PlatformWindow,
    PlatformWindowDispatch, PlatformWindowMutationObservation, PlatformWindowMutationTerminal,
    Point, PromptButton, RequestFrameOptions, Scene, Size, TestPlatform, TileId, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowMutationDomain,
    WindowMutationRequest, WindowParams, WindowPlacementState, WindowPlatformFacts,
};
use image::RgbaImage;
use open_gpui_collections::HashMap;
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
    pub(crate) should_close_handler: Option<Box<dyn FnMut() -> bool>>,
    hit_test_window_control_callback: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
    input_callback: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    hover_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    request_frame_callback: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    resize_callback: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved_callback: Option<Box<dyn FnMut()>>,
    window_state_change_callback: Option<Box<dyn FnMut()>>,
    mutation_observation_callback: Option<Box<dyn FnMut(PlatformWindowMutationObservation)>>,
    input_handler: Option<PlatformInputHandler>,
    ime_position_history: Vec<Bounds<Pixels>>,
    is_minimized: bool,
    is_maximized: bool,
    is_fullscreen: bool,
    is_active: bool,
    accepts_pointer_input: bool,
    focus_on_appearing: bool,
    focus_on_click: bool,
    background_appearance: WindowBackgroundAppearance,
    topmost: bool,
    taskbar_visible: bool,
    window_bounds: WindowBounds,
    pending_mutations: Vec<TestWindowMutationRequest>,
    mutation_generations: HashMap<WindowMutationDomain, u64>,
    next_mutation_dispatches: HashMap<WindowMutationDomain, PlatformWindowDispatch>,
    pub(crate) cursor_style: CursorStyle,
    accessibility: TestAccessibilityState,
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

#[derive(Clone)]
pub struct TestWindow(pub(crate) Rc<Mutex<TestWindowState>>);

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
    ) -> Self {
        let sprite_atlas: Arc<dyn PlatformAtlas> = match &renderer {
            Some(r) => r.sprite_atlas(),
            None => Arc::new(TestAtlas::new()),
        };
        Self(Rc::new(Mutex::new(TestWindowState {
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
            hit_test_window_control_callback: None,
            input_callback: None,
            active_status_change_callback: None,
            hover_status_change_callback: None,
            request_frame_callback: None,
            resize_callback: None,
            moved_callback: None,
            window_state_change_callback: None,
            mutation_observation_callback: None,
            input_handler: None,
            ime_position_history: Vec::new(),
            is_minimized: false,
            is_maximized: matches!(params.window_bounds, WindowBounds::Maximized(_)),
            is_fullscreen: matches!(params.window_bounds, WindowBounds::Fullscreen(_)),
            is_active: false,
            accepts_pointer_input: params.accepts_pointer_input,
            focus_on_appearing: params.focus,
            focus_on_click: true,
            background_appearance: WindowBackgroundAppearance::Opaque,
            topmost: false,
            taskbar_visible: true,
            window_bounds: params.window_bounds,
            pending_mutations: Vec::new(),
            mutation_generations: HashMap::default(),
            next_mutation_dispatches: HashMap::default(),
            cursor_style: CursorStyle::Arrow,
            accessibility: TestAccessibilityState::default(),
        })))
    }

    #[cfg(test)]
    fn requested_display_id(&self) -> Option<DisplayId> {
        self.0.lock().requested_display_id
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
        self.0.lock().resize_callback = Some(callback);
    }

    pub fn simulate_minimize(&mut self) {
        let mut lock = self.0.lock();
        lock.is_minimized = true;
        let Some(mut callback) = lock.window_state_change_callback.take() else {
            return;
        };
        drop(lock);
        callback();
        self.0.lock().window_state_change_callback = Some(callback);
    }

    pub(crate) fn simulate_active_status_change(&self, active: bool) {
        let mut lock = self.0.lock();
        lock.is_active = active;
        let Some(mut callback) = lock.active_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(active);
        self.0.lock().active_status_change_callback = Some(callback);
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
        self.0.lock().mutation_observation_callback = Some(callback);
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
        self.0.lock().mutation_observation_callback = Some(callback);
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
        self.0.lock().mutation_observation_callback = Some(callback);
        true
    }

    pub(crate) fn simulate_hover_status_change(&self, hovered: bool) {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.hover_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(hovered);
        self.0.lock().hover_status_change_callback = Some(callback);
    }

    pub fn simulate_input_result(&mut self, event: PlatformInput) -> DispatchEventResult {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.input_callback.take() else {
            return DispatchEventResult::default();
        };
        drop(lock);
        let result = callback(event);
        self.0.lock().input_callback = Some(callback);
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
        self.0.lock().request_frame_callback = Some(callback);
        true
    }
}

impl PlatformWindow for TestWindow {
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

    fn platform_facts(&self) -> WindowPlatformFacts {
        window_platform_facts(&self.0.lock())
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        let mut lock = self.0.lock();
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
        if lock.mutation_generations.get(&domain).copied() != Some(generation) {
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
        self.0.lock().input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.lock().input_handler.take()
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

    fn activate(&self) {
        self.0
            .lock()
            .platform
            .upgrade()
            .unwrap()
            .set_active_window(Some(self.clone()))
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
        self.0.lock().request_frame_callback = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(crate::PlatformInput) -> DispatchEventResult>) {
        self.0.lock().input_callback = Some(callback)
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().active_status_change_callback = Some(callback)
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().hover_status_change_callback = Some(callback)
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.lock().resize_callback = Some(callback)
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().moved_callback = Some(callback)
    }

    fn on_window_state_change(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().window_state_change_callback = Some(callback)
    }

    fn on_window_mutation_observation(
        &self,
        callback: Box<dyn FnMut(PlatformWindowMutationObservation)>,
    ) {
        self.0.lock().mutation_observation_callback = Some(callback)
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.lock().should_close_handler = Some(callback);
    }

    fn on_close(&self, _callback: Box<dyn FnOnce()>) {}

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.0.lock().hit_test_window_control_callback = Some(callback);
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn draw(&self, _scene: &Scene) {}

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

    fn show_window_menu(&self, _position: Point<Pixels>) {
        unimplemented!()
    }

    fn start_window_move(&self) {
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
        focus_on_appearing: state.focus_on_appearing,
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
        WindowMutationRequest::FocusOnAppearing(focus) => {
            state.focus_on_appearing = focus;
        }
        WindowMutationRequest::FocusOnClick(focus) => {
            state.focus_on_click = focus;
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
    state.focus_on_appearing = facts.focus_on_appearing;
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
        AppContext, DisplayId, Empty, PlatformWindowMutationCapabilities, TestAppContext,
        WindowKind, WindowMutationDispatch, WindowMutationOutcome, WindowMutationSupport,
        WindowMutationTicket, WindowOptions, WindowPlacementRequest, px, size,
    };
    use std::{cell::Cell, rc::Rc};

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(Point::new(px(x), px(y)), size(px(width), px(height)))
    }

    fn open_test_window(cx: &mut TestAppContext) -> (AnyWindowHandle, TestWindow) {
        let handle = cx.open_window(size(px(320.0), px(240.0)), |_, _| Empty);
        let handle = handle.into();
        let platform_window = cx.test_window(handle);
        (handle, platform_window)
    }

    #[crate::test]
    fn app_retains_actual_window_kind_mutation_profile_through_updates_and_close(
        cx: &mut TestAppContext,
    ) {
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
            .update_window(handle, |_, window, _| window.window_mutation_capabilities())
            .expect("floating test window should remain live");

        let profile = cx
            .read(|app| app.window_mutation_profile(handle).cloned())
            .expect("opened window should have a mutation profile");
        assert_eq!(profile.kind, WindowKind::Floating);
        assert_eq!(profile.capabilities, expected_capabilities);
        assert_eq!(
            cx.update_window(handle, |_, _, app| {
                app.window_mutation_profile(handle).cloned()
            })
            .expect("profile should remain readable while the window is being updated"),
            Some(profile)
        );

        cx.update_window(handle, |_, window, app| window.remove_window(app))
            .expect("floating test window should close");
        assert!(
            cx.read(|app| app.window_mutation_profile(handle).is_none()),
            "closed windows must not retain stale mutation profiles"
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
        let (handle, mut platform_window) = open_test_window(cx);
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
        platform_window.simulate_resize(intermediate_size);
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

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));

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

        assert!(
            platform_window.simulate_window_mutation_observation(
                WindowMutationDomain::Placement,
                adjusted_facts,
            )
        );
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
        assert!(platform_window.simulate_window_mutation_terminal(
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
        assert!(platform_window.simulate_window_mutation_terminal(
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
    fn mutation_domains_are_isolated_and_invalid_placement_preserves_existing_ticket(
        cx: &mut TestAppContext,
    ) {
        let (handle, platform_window) = open_test_window(cx);
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

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::PointerInput));
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

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
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
        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
        assert_eq!(
            partial_ticket.observation().unwrap().outcome,
            WindowMutationOutcome::Exact
        );
    }

    #[crate::test]
    fn invalid_numeric_placement_preserves_existing_ticket(cx: &mut TestAppContext) {
        let (handle, platform_window) = open_test_window(cx);
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

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
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

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
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
        let (handle, platform_window) = open_test_window(cx);
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

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
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
        let (handle, mut platform_window) = open_test_window(cx);
        assert!(
            !cx.update_window(handle, |_, window, _| window.is_minimized())
                .unwrap()
        );

        platform_window.simulate_minimize();

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
        let (handle, platform_window) = open_test_window(cx);
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

        assert!(
            platform_window.simulate_window_mutation_terminal_for_generation(
                WindowMutationDomain::Placement,
                first.generation(),
                PlatformWindowMutationTerminal::Observed,
                stale_facts,
            )
        );
        assert!(second.observation().is_none());
        assert_eq!(
            cx.update_window(handle, |_, window, _| window.platform_facts().clone())
                .unwrap(),
            committed_before,
            "a stale terminal callback must be ignored before committing its facts"
        );

        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
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
    fn independent_flag_domains_queue_and_settle_without_superseding_each_other(
        cx: &mut TestAppContext,
    ) {
        cx.set_platform_window_mutation_capabilities(PlatformWindowMutationCapabilities {
            focus_on_appearing: WindowMutationSupport::Live,
            focus_on_click: WindowMutationSupport::Live,
            alpha: WindowMutationSupport::Live,
            topmost: WindowMutationSupport::Live,
            taskbar_visibility: WindowMutationSupport::Live,
            ..Default::default()
        });
        let (handle, platform_window) = open_test_window(cx);
        let requests = [
            WindowMutationRequest::FocusOnAppearing(false),
            WindowMutationRequest::FocusOnClick(false),
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
            WindowMutationDomain::FocusOnAppearing,
            WindowMutationDomain::FocusOnClick,
            WindowMutationDomain::Alpha,
            WindowMutationDomain::Topmost,
            WindowMutationDomain::TaskbarVisibility,
        ] {
            assert!(platform_window.flush_window_mutation(domain));
        }
        assert!(tickets.iter().all(|ticket| {
            ticket
                .observation()
                .is_some_and(|observation| observation.outcome == WindowMutationOutcome::Exact)
        }));
        let facts = cx
            .update_window(handle, |_, window, _| window.platform_facts().clone())
            .unwrap();
        assert!(!facts.focus_on_appearing);
        assert!(!facts.focus_on_click);
        assert_eq!(
            facts.background_appearance,
            WindowBackgroundAppearance::Transparent
        );
        assert!(facts.topmost);
        assert!(!facts.taskbar_visible);
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
        let (handle, mut platform_window) = open_test_window(cx);
        platform_window.simulate_minimize();

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
        assert!(platform_window.flush_window_mutation(WindowMutationDomain::Placement));
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
