use super::*;
use objc2_app_kit::NSEventType;
use open_gpui::{
    PlatformNativePointerPhysicalFrame, PlatformPhysicalDisplayObservation, WindowPlatformFacts,
};

#[derive(Default)]
pub(super) struct MacWindowMutations {
    // The boolean records whether the prepared generation owns queued native work.
    generations: HashMap<WindowMutationDomain, (u64, bool)>,
}

impl MacWindowMutations {
    pub(super) fn prepare(&mut self, domain: WindowMutationDomain, generation: u64) {
        self.generations.insert(domain, (generation, false));
    }

    fn queue(&mut self, domain: WindowMutationDomain, generation: u64) -> bool {
        let Some(current) = self.generations.get_mut(&domain) else {
            return false;
        };
        if *current != (generation, false) {
            return false;
        }
        current.1 = true;
        true
    }

    pub(super) fn is_current(&self, domain: WindowMutationDomain, generation: u64) -> bool {
        self.generations.get(&domain) == Some(&(generation, true))
    }

    pub(super) fn retire(&mut self, domain: WindowMutationDomain, generation: u64) {
        if self
            .generations
            .get(&domain)
            .is_some_and(|current| current.0 == generation)
        {
            self.generations.remove(&domain);
        }
    }

    pub(super) fn invalidate(&mut self, domain: WindowMutationDomain) {
        self.generations.remove(&domain);
    }

    pub(super) fn clear(&mut self) {
        self.generations.clear();
    }
}

#[derive(Default)]
pub(super) struct MacNativePointerPhysicalFrameState {
    current: Cell<Option<PlatformNativePointerPhysicalFrame>>,
    invalidation_epoch: Cell<u64>,
}

impl MacNativePointerPhysicalFrameState {
    pub(super) fn current(&self) -> Option<PlatformNativePointerPhysicalFrame> {
        self.current.get()
    }

    pub(super) fn invalidate(&self) {
        self.invalidation_epoch
            .set(self.invalidation_epoch.get().wrapping_add(1));
        self.current.set(None);
    }
}

pub(super) struct MacNativePointerPhysicalFrameScope {
    state: Rc<MacNativePointerPhysicalFrameState>,
    previous: Option<PlatformNativePointerPhysicalFrame>,
    entry_epoch: u64,
}

impl MacNativePointerPhysicalFrameScope {
    pub(super) fn enter(
        state: &Rc<MacNativePointerPhysicalFrameState>,
        frame: Option<PlatformNativePointerPhysicalFrame>,
    ) -> Self {
        Self {
            previous: state.current.replace(frame),
            entry_epoch: state.invalidation_epoch.get(),
            state: state.clone(),
        }
    }
}

impl Drop for MacNativePointerPhysicalFrameScope {
    fn drop(&mut self) {
        if self.state.invalidation_epoch.get() == self.entry_epoch {
            self.state.current.set(self.previous);
        } else {
            self.state.current.set(None);
        }
    }
}

fn common_scale(mut scales: impl Iterator<Item = f32>) -> Option<f32> {
    let scale = scales.next()?;
    (scale.is_finite() && scale > 0.0 && scales.all(|other| other == scale)).then_some(scale)
}

fn topology_scale(topology: &MacDisplayTopologySnapshot) -> Option<f32> {
    // The detached topology has logical desktop origins. A common scale proves a single
    // physical desktop mapping; mixed backing scales need an explicit origin mapping.
    common_scale(
        topology
            .platform_snapshot()
            .displays()
            .iter()
            .map(|display| {
                topology
                    .display(display.id())
                    .map(|display| display.scale_factor())
                    .unwrap_or(f32::NAN)
            }),
    )
}

fn device_coordinate(value: f64) -> Option<DevicePixels> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < i32::MIN as f64
        || value > i32::MAX as f64
    {
        return None;
    }
    Some(DevicePixels(value as i32))
}

fn physical_bounds(left: f64, top: f64, right: f64, bottom: f64) -> Option<Bounds<DevicePixels>> {
    let left = device_coordinate(left)?;
    let top = device_coordinate(top)?;
    let right = device_coordinate(right)?;
    let bottom = device_coordinate(bottom)?;
    let width = right.0.checked_sub(left.0)?;
    let height = bottom.0.checked_sub(top.0)?;
    if width < 0 || height < 0 {
        return None;
    }
    Some(Bounds::new(
        point(left, top),
        size(DevicePixels(width), DevicePixels(height)),
    ))
}

fn display_rect(bounds: Bounds<Pixels>, scale: f32) -> Option<Bounds<DevicePixels>> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let scale = f64::from(scale);
    let left = f64::from(bounds.origin.x.as_f32()) * scale;
    let top = f64::from(bounds.origin.y.as_f32()) * scale;
    physical_bounds(
        left,
        top,
        left + f64::from(bounds.size.width.as_f32()) * scale,
        top + f64::from(bounds.size.height.as_f32()) * scale,
    )
}

fn physical_display(
    topology_generation: u64,
    display: MacDisplay,
    desktop_scale: f32,
) -> Option<PlatformPhysicalDisplayObservation> {
    if display.scale_factor() != desktop_scale {
        return None;
    }
    PlatformPhysicalDisplayObservation::try_new(
        topology_generation,
        display.id(),
        display_rect(display.bounds(), desktop_scale)?,
        display_rect(display.visible_bounds(), desktop_scale)?,
        desktop_scale,
    )
}

fn native_to_physical(
    rect: NSRect,
    screen: NSRect,
    display: PlatformPhysicalDisplayObservation,
) -> Option<Bounds<DevicePixels>> {
    if !native_rect_is_valid(rect) || !screen_matches_display(screen, display) {
        return None;
    }
    let scale = f64::from(display.scale_factor());
    let left = f64::from(display.bounds().origin.x.0) + (rect.origin.x - screen.origin.x) * scale;
    let top = f64::from(display.bounds().origin.y.0)
        + (screen.origin.y + screen.size.height - rect.origin.y - rect.size.height) * scale;
    physical_bounds(
        left,
        top,
        left + rect.size.width * scale,
        top + rect.size.height * scale,
    )
}

fn screen_matches_display(screen: NSRect, display: PlatformPhysicalDisplayObservation) -> bool {
    let scale = f64::from(display.scale_factor());
    native_rect_is_valid(screen)
        && (screen.origin.x + screen.size.width).is_finite()
        && (screen.origin.y + screen.size.height).is_finite()
        && screen.size.width * scale == f64::from(display.bounds().size.width.0)
        && screen.size.height * scale == f64::from(display.bounds().size.height.0)
}

fn physical_to_native(
    bounds: Bounds<DevicePixels>,
    screen: NSRect,
    display: PlatformPhysicalDisplayObservation,
) -> Option<NSRect> {
    WindowPhysicalPlacementRequest::try_new(bounds)?;
    let scale = f64::from(display.scale_factor());
    let relative_x =
        (f64::from(bounds.origin.x.0) - f64::from(display.bounds().origin.x.0)) / scale;
    let relative_y =
        (f64::from(bounds.origin.y.0) - f64::from(display.bounds().origin.y.0)) / scale;
    let rect = NSRect::new(
        NSPoint::new(
            screen.origin.x + relative_x,
            screen.origin.y + screen.size.height
                - relative_y
                - f64::from(bounds.size.height.0) / scale,
        ),
        NSSize::new(
            f64::from(bounds.size.width.0) / scale,
            f64::from(bounds.size.height.0) / scale,
        ),
    );
    (native_to_physical(rect, screen, display) == Some(bounds)).then_some(rect)
}

pub(super) fn observe_geometry(
    topology: &MacDisplayTopologySnapshot,
    display: MacDisplay,
    screen: NSRect,
    outer: NSRect,
    client: NSRect,
) -> Option<PlatformWindowPhysicalGeometry> {
    let display = physical_display(topology.generation(), display, topology_scale(topology)?)?;
    geometry_for_display(screen, outer, client, display)
}

fn geometry_for_display(
    screen: NSRect,
    outer: NSRect,
    client: NSRect,
    display: PlatformPhysicalDisplayObservation,
) -> Option<PlatformWindowPhysicalGeometry> {
    PlatformWindowPhysicalGeometry::try_new(
        native_to_physical(client, screen, display)?,
        display.scale_factor(),
    )?
    .with_outer_bounds(native_to_physical(outer, screen, display)?)?
    .with_display_observation(display)
}

fn center(bounds: Bounds<DevicePixels>) -> Option<Point<DevicePixels>> {
    WindowPhysicalPlacementRequest::try_new(bounds)?;
    Some(point(
        DevicePixels(bounds.origin.x.0.checked_add(bounds.size.width.0 / 2)?),
        DevicePixels(bounds.origin.y.0.checked_add(bounds.size.height.0 / 2)?),
    ))
}

fn unique_display_at(
    displays: impl IntoIterator<Item = PlatformPhysicalDisplayObservation>,
    anchor: Point<DevicePixels>,
) -> Option<PlatformPhysicalDisplayObservation> {
    let mut matches = displays
        .into_iter()
        .filter(|display| display.contains(anchor));
    let display = matches.next()?;
    matches.next().is_none().then_some(display)
}

fn physical_display_at(
    topology: &MacDisplayTopologySnapshot,
    anchor: Point<DevicePixels>,
) -> Option<PlatformPhysicalDisplayObservation> {
    let scale = topology_scale(topology)?;
    let displays = topology.platform_snapshot();
    let physical = displays
        .displays()
        .iter()
        .map(|display| {
            physical_display(
                topology.generation(),
                topology.display(display.id())?,
                scale,
            )
        })
        .collect::<Option<SmallVec<[_; 4]>>>()?;
    unique_display_at(physical, anchor)
}

fn target_for_point(
    topology: &MacDisplayTopologySnapshot,
    anchor: Point<DevicePixels>,
) -> Option<(
    ValidatedMacDisplayTarget,
    PlatformPhysicalDisplayObservation,
)> {
    let physical = physical_display_at(topology, anchor)?;
    let native = topology
        .resolve_native_target(Some(physical.display_id()))
        .ok()?;
    Some((native, physical))
}

fn target_still_matches(
    topology: &MacDisplayTopologySnapshot,
    target: &ValidatedMacDisplayTarget,
    screen: NSRect,
) -> bool {
    target.generation() == topology.generation()
        && target.screen().frame() == screen
        && topology
            .validate_native_screen(target.screen())
            .is_ok_and(|current| current.display() == target.display())
}

pub(super) fn sample_pointer_frame(
    state: &MacWindowState,
    event: &NSEvent,
    window_height: Pixels,
) -> Option<PlatformNativePointerPhysicalFrame> {
    if state.is_closed()
        || state.interaction_is_quiesced()
        || !matches!(
            event.r#type(),
            NSEventType::LeftMouseDown
                | NSEventType::RightMouseDown
                | NSEventType::OtherMouseDown
                | NSEventType::LeftMouseUp
                | NSEventType::RightMouseUp
                | NSEventType::OtherMouseUp
                | NSEventType::MouseMoved
                | NSEventType::LeftMouseDragged
                | NSEventType::RightMouseDragged
                | NSEventType::OtherMouseDragged
        )
    {
        return None;
    }
    let event_window = event.window(crate::main_thread_marker()?)?;
    if Retained::as_ptr(&event_window).cast::<AnyObject>() != state.native_window.cast_const() {
        return None;
    }
    let topology = state.display_topology.exact_snapshot().ok()?;
    let source = state
        .native_observation_for_generation(topology.generation())
        .ok()?;
    if source.topology_generation != topology.generation()
        || source.bounds.size.height != window_height
    {
        return None;
    }
    let geometry = source.physical_geometry?;
    let source_display = geometry.display_observation()?;
    let screen = event_window.screen()?;
    let native_source = topology.validate_native_screen(&screen).ok()?;
    if native_source.display() != source.display {
        return None;
    }
    let screen_frame = screen.frame();
    // Preserve the NSEvent's native point. Public logical mouse events and later cursor polls
    // cannot establish an event-time desktop position.
    let native_point = event_window.convertPointToScreen(event.locationInWindow());
    let global = native_to_physical(
        NSRect::new(native_point, NSSize::new(0.0, 0.0)),
        screen_frame,
        source_display,
    )?
    .origin;
    let (target, target_display) = target_for_point(&topology, global)?;
    if !target_still_matches(&topology, &native_source, screen_frame)
        || !target_still_matches(&topology, &target, target.screen().frame())
        || state
            .native_observation_for_generation(topology.generation())
            .ok()?
            != source
    {
        return None;
    }
    PlatformNativePointerPhysicalFrame::new(global, geometry).with_target_display(target_display)
}

fn is_windowed(observation: MacWindowNativeObservation) -> bool {
    !observation.is_minimized
        && !observation.is_maximized
        && !observation.is_fullscreen
        && observation.simple_fullscreen_restore_bounds.is_none()
}

fn admits_placement(state: &MacWindowState) -> bool {
    state.supports_live_mutations
        && !state.initial_presentation.allows_automatic_window_tabbing
        && state.creation_facts.transient_for.is_none()
        && !has_native_tab_peers(state.native_window())
}

fn has_native_tab_peers(window: &NSWindow) -> bool {
    // Runtime tab commands can group a window that was created without a tabbing identifier.
    unsafe {
        let windows: *mut AnyObject = msg_send![window, tabbedWindows];
        if windows.is_null() {
            return false;
        }
        let count: NSUInteger = msg_send![windows, count];
        count > 1
    }
}

pub(super) fn request_for_outer_bounds(
    window_state: &Arc<Mutex<MacWindowState>>,
    bounds: Bounds<DevicePixels>,
) -> Option<WindowPhysicalPlacementRequest> {
    let (window, topology, source) = {
        let state = window_state.lock();
        if state.is_closed()
            || state.interaction_is_quiesced()
            || !admits_placement(&state)
            || !is_windowed(state.committed_native_observation())
            || state.fullscreen_transition.is_some()
            || state.pending_fullscreen_restore_bounds.is_some()
        {
            return None;
        }
        let topology = state.display_topology.exact_snapshot().ok()?;
        let source = state
            .native_observation_for_generation(topology.generation())
            .ok()?;
        if source.topology_generation != topology.generation() || !is_windowed(source) {
            return None;
        }
        (
            unsafe { Retained::<NSWindow>::retain(state.native_window.cast()) }?,
            topology,
            source,
        )
    };
    let (target, display) = target_for_point(&topology, center(bounds)?)?;
    let screen = target.screen().frame();
    let outer = physical_to_native(bounds, screen, display)?;
    // Ask this window's native decoration policy at the validated target display.
    let client: NSRect = unsafe { msg_send![&*window, contentRectForFrameRect: outer] };
    let client = geometry_for_display(screen, outer, client, display)?.client_bounds();
    let anchor = center(client)?;
    if physical_display_at(&topology, anchor)? != display
        || !target_still_matches(&topology, &target, screen)
    {
        return None;
    }
    let request = WindowPhysicalPlacementRequest::try_new_for_display(client, anchor, display)?;
    let state = window_state.lock();
    if state.is_closed()
        || state.interaction_is_quiesced()
        || state.pending_fullscreen_restore_bounds.is_some()
        || !admits_placement(&state)
    {
        return None;
    }
    (state
        .native_observation_for_generation(topology.generation())
        .ok()?
        == source)
        .then_some(request)
}

impl MacWindowState {
    pub(super) fn platform_facts_for_observation(
        &self,
        observation: MacWindowNativeObservation,
    ) -> WindowPlatformFacts {
        let window_bounds = self.window_bounds_from_client_bounds(
            observation.bounds,
            observation.is_fullscreen,
            observation.is_maximized,
            observation.simple_fullscreen_restore_bounds,
        );
        let interactive = !self.interaction_is_quiesced();
        WindowPlatformFacts {
            bounds: observation.bounds,
            coordinate_space: open_gpui::WindowCoordinateSpace::GlobalScreen,
            physical_geometry: observation.physical_geometry,
            window_bounds,
            inner_window_bounds: window_bounds,
            content_size: observation.bounds.size,
            scale_factor: observation.display.scale_factor(),
            display_id: Some(observation.display.id()),
            is_minimized: observation.is_minimized,
            is_maximized: observation.is_maximized,
            is_fullscreen: observation.is_fullscreen,
            accepts_pointer_input: interactive && observation.accepts_pointer_input,
            accepts_activation: interactive && self.activation_policy.accepts_activation,
            focus_on_click: interactive && self.activation_policy.focus_on_click,
            background_appearance: self.background_appearance,
            topmost: self.topmost,
            taskbar_visible: self.taskbar_visible,
            is_active: interactive && observation.is_active,
        }
    }
}

pub(super) fn queue_mutation(
    window_state: &Arc<Mutex<MacWindowState>>,
    generation: u64,
    request: WindowMutationRequest,
) -> PlatformWindowDispatch {
    if !matches!(
        request,
        WindowMutationRequest::PhysicalPlacement(_) | WindowMutationRequest::PointerInput(_)
    ) {
        return PlatformWindowDispatch::Unsupported;
    }
    let domain = request.domain();
    let executor = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return PlatformWindowDispatch::WindowClosed;
        }
        if !state.supports_live_mutations {
            return PlatformWindowDispatch::Unsupported;
        }
        if state.interaction_is_quiesced()
            || state.fullscreen_transition.is_some()
            || state.pending_fullscreen_restore_bounds.is_some()
            || !is_windowed(state.committed_native_observation())
            || state.mutation_callback.is_none()
        {
            return PlatformWindowDispatch::Rejected;
        }
        if matches!(request, WindowMutationRequest::PhysicalPlacement(_))
            && (!admits_placement(&state)
                || state
                    .committed_native_observation()
                    .physical_geometry
                    .is_none())
        {
            return PlatformWindowDispatch::Unsupported;
        }
        if !state.mutations.queue(domain, generation) {
            return PlatformWindowDispatch::Rejected;
        }
        state.foreground_executor.clone()
    };
    let weak = Arc::downgrade(window_state);
    executor
        .spawn(async move {
            if let Some(window_state) = weak.upgrade() {
                perform_mutation(&window_state, generation, request);
            }
        })
        .detach();
    PlatformWindowDispatch::Queued
}

struct MacWindowMutationTarget {
    window: Retained<NSWindow>,
    topology: MacDisplayTopologySnapshot,
    observation: MacWindowNativeObservation,
}

fn capture_mutation_target(
    window_state: &Arc<Mutex<MacWindowState>>,
    generation: u64,
    request: WindowMutationRequest,
) -> Option<MacWindowMutationTarget> {
    let state = window_state.lock();
    if state.is_closed()
        || state.interaction_is_quiesced()
        || state.pending_fullscreen_restore_bounds.is_some()
        || !state.mutations.is_current(request.domain(), generation)
    {
        return None;
    }
    let observation = state.native_observation_for_generation(0).ok()?;
    if !state.supports_live_mutations
        || !is_windowed(observation)
        || matches!(request, WindowMutationRequest::PhysicalPlacement(_))
            && (!admits_placement(&state) || observation.physical_geometry.is_none())
    {
        return None;
    }
    let topology = state.display_topology.exact_snapshot().ok()?;
    if topology.generation() != observation.topology_generation {
        return None;
    }
    Some(MacWindowMutationTarget {
        window: unsafe { Retained::<NSWindow>::retain(state.native_window.cast()) }?,
        topology,
        observation,
    })
}

fn placement_frame(
    window: &NSWindow,
    topology: &MacDisplayTopologySnapshot,
    request: WindowPhysicalPlacementRequest,
) -> Option<NSRect> {
    let anchor = request
        .target_point()
        .or_else(|| center(request.client_bounds()))?;
    let (target, display) = target_for_point(topology, anchor)?;
    if request
        .target_display()
        .is_some_and(|expected| expected != display)
    {
        return None;
    }
    let screen = target.screen().frame();
    let content = physical_to_native(request.client_bounds(), screen, display)?;
    let frame: NSRect = unsafe { msg_send![window, frameRectForContentRect: content] };
    let round_trip: NSRect = unsafe { msg_send![window, contentRectForFrameRect: frame] };
    geometry_for_display(screen, frame, content, display)?;
    (round_trip == content && target_still_matches(topology, &target, screen)).then_some(frame)
}

fn perform_mutation(
    window_state: &Arc<Mutex<MacWindowState>>,
    generation: u64,
    request: WindowMutationRequest,
) {
    let terminal = if apply_mutation(window_state, generation, request).is_some() {
        PlatformWindowMutationTerminal::Observed
    } else {
        PlatformWindowMutationTerminal::Rejected
    };
    report_mutation(window_state, generation, request.domain(), terminal);
}

fn apply_mutation(
    window_state: &Arc<Mutex<MacWindowState>>,
    generation: u64,
    request: WindowMutationRequest,
) -> Option<()> {
    let target = capture_mutation_target(window_state, generation, request)?;
    let frame = match request {
        WindowMutationRequest::PhysicalPlacement(request) => {
            Some(placement_frame(&target.window, &target.topology, request)?)
        }
        WindowMutationRequest::PointerInput(_) => None,
        _ => return None,
    };
    {
        let state = window_state.lock();
        if state.is_closed()
            || state.interaction_is_quiesced()
            || state.pending_fullscreen_restore_bounds.is_some()
            || !state.mutations.is_current(request.domain(), generation)
            || matches!(request, WindowMutationRequest::PhysicalPlacement(_))
                && !admits_placement(&state)
        {
            return None;
        }
        if state
            .native_observation_for_generation(target.topology.generation())
            .ok()?
            != target.observation
        {
            return None;
        }
    }
    // AppKit setters can synchronously deliver delegate events. Neither effects nor user
    // callbacks may run under the window lock; the serial observation drain owns their order.
    unsafe {
        if let Some(frame) = frame {
            let _: () = msg_send![&*target.window, setFrame: frame, display: Bool::YES];
        } else if let WindowMutationRequest::PointerInput(accepts) = request {
            let _: () = msg_send![&*target.window, setIgnoresMouseEvents: (!accepts).to_objc()];
        }
    }
    Some(())
}

fn report_mutation(
    window_state: &Arc<Mutex<MacWindowState>>,
    generation: u64,
    domain: WindowMutationDomain,
    terminal: PlatformWindowMutationTerminal,
) {
    let (topology_generation, event) = {
        let state = window_state.lock();
        if state.is_closed() || !state.mutations.is_current(domain, generation) {
            return;
        }
        let kind = MacWindowObservationEvent::Mutation {
            domain,
            generation,
            terminal,
        };
        (
            state.observation_target_generation(),
            state.pending_observation_event(kind),
        )
    };
    request_window_observation_commit(window_state, topology_generation, Some(event), true);
}

pub(super) fn deliver_mutation_observation(
    window_state: &Arc<Mutex<MacWindowState>>,
    domain: WindowMutationDomain,
    generation: u64,
    terminal: PlatformWindowMutationTerminal,
    facts: WindowPlatformFacts,
    panic_boundary: &mut MacWindowCallbackPanicBoundary,
) {
    let callback = {
        let mut state = window_state.lock();
        if state.is_closed() || !state.mutations.is_current(domain, generation) {
            return;
        }
        state.mutations.retire(domain, generation);
        state.mutation_callback.clone()
    };
    if let Some(callback) = callback {
        let terminal = terminal_for_geometry(domain, terminal, facts.physical_geometry);
        let observation =
            PlatformWindowMutationObservation::terminal(domain, generation, terminal, facts);
        panic_boundary.deliver(|| callback(observation));
    }
}

fn terminal_for_geometry(
    domain: WindowMutationDomain,
    terminal: PlatformWindowMutationTerminal,
    geometry: Option<PlatformWindowPhysicalGeometry>,
) -> PlatformWindowMutationTerminal {
    if domain == WindowMutationDomain::Placement
        && terminal == PlatformWindowMutationTerminal::Observed
        && geometry.is_none()
    {
        // A completed setter without a provable physical readback cannot settle placement as
        // an OS adjustment. Exact-versus-adjusted classification remains GPUI's responsibility.
        PlatformWindowMutationTerminal::Rejected
    } else {
        terminal
    }
}

#[cfg(test)]
mod tests {
    use super::super::creation_projection_tests::complete_native_observation;
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn device_bounds(x: i32, y: i32, width: i32, height: i32) -> Bounds<DevicePixels> {
        Bounds::new(
            point(DevicePixels(x), DevicePixels(y)),
            size(DevicePixels(width), DevicePixels(height)),
        )
    }

    fn display(id: u64, x: f32, y: f32, scale: f32) -> PlatformPhysicalDisplayObservation {
        physical_display(
            7,
            crate::display::tests::display(u128::from(id) << 64, x, y, scale, 1920.0),
            scale,
        )
        .expect("the test display has integral physical bounds")
    }

    #[test]
    fn native_geometry_round_trips_on_retina_and_signed_display_origins() {
        let cases = [
            (
                0.0,
                0.0,
                1.0,
                rect(0.0, 0.0, 1920.0, 1080.0),
                rect(120.0, 240.0, 640.0, 480.0),
                device_bounds(120, 360, 640, 480),
            ),
            (
                -1920.0,
                0.0,
                2.0,
                rect(-1920.0, 0.0, 1920.0, 1080.0),
                rect(-1800.0, 240.0, 640.0, 480.0),
                device_bounds(-3600, 720, 1280, 960),
            ),
            (
                0.0,
                -1080.0,
                2.0,
                rect(0.0, 1080.0, 1920.0, 1080.0),
                rect(120.0, 1320.0, 640.0, 480.0),
                device_bounds(240, -1440, 1280, 960),
            ),
            (
                -1920.0,
                -180.0,
                2.0,
                rect(-1920.0, 180.0, 1920.0, 1080.0),
                rect(-1800.0, 420.0, 640.0, 480.0),
                device_bounds(-3600, 360, 1280, 960),
            ),
        ];
        for (x, y, scale, screen, native, expected) in cases {
            let display = display(1, x, y, scale);
            assert_eq!(native_to_physical(native, screen, display), Some(expected));
            assert_eq!(physical_to_native(expected, screen, display), Some(native));
        }
    }

    #[test]
    fn same_sample_outer_and_client_keep_native_decoration_geometry() {
        let display = display(1, 0.0, 0.0, 2.0);
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        let outer = rect(100.0, 200.0, 800.0, 622.0);
        let client = rect(100.0, 200.0, 800.0, 600.0);
        let geometry = geometry_for_display(screen, outer, client, display).unwrap();
        assert_eq!(
            geometry.client_bounds(),
            device_bounds(200, 560, 1600, 1200)
        );
        assert_eq!(
            geometry.outer_bounds(),
            Some(device_bounds(200, 516, 1600, 1244))
        );
        assert_eq!(geometry.scale_factor(), 2.0);
        assert_eq!(geometry.display_observation(), Some(display));
        assert_eq!(
            physical_to_native(geometry.outer_bounds().unwrap(), screen, display),
            Some(outer)
        );
        assert!(
            geometry_for_display(screen, rect(100.0, 200.0, 800.0, 599.0), client, display)
                .is_none()
        );
    }

    #[test]
    fn mixed_or_invalid_scales_cannot_prove_a_physical_desktop() {
        assert_eq!(common_scale([2.0, 2.0].into_iter()), Some(2.0));
        for scales in [
            &[][..],
            &[1.0, 2.0][..],
            &[0.0][..],
            &[-1.0][..],
            &[f32::NAN][..],
            &[f32::INFINITY][..],
            &[2.0, f32::NAN][..],
        ] {
            assert_eq!(common_scale(scales.iter().copied()), None);
        }
        let retina = crate::display::tests::display(1_u128 << 64, 0.0, 0.0, 2.0, 1920.0);
        assert!(physical_display(7, retina, 1.0).is_none());
        assert!(physical_display(0, retina, 2.0).is_none());
        assert!(display_rect(retina.bounds(), f32::NAN).is_none());
        assert!(display_rect(retina.bounds(), 0.0).is_none());
    }

    #[test]
    fn native_projection_rejects_stale_extents_nonfinite_and_fractional_pixels() {
        let display = display(1, 0.0, 0.0, 2.0);
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        for native in [
            rect(f64::NAN, 10.0, 100.0, 100.0),
            rect(10.0, f64::INFINITY, 100.0, 100.0),
            rect(10.25, 10.0, 100.0, 100.0),
            rect(10.0, 10.0, -1.0, 100.0),
        ] {
            assert!(native_to_physical(native, screen, display).is_none());
        }
        assert!(
            native_to_physical(
                rect(10.0, 10.0, 100.0, 100.0),
                rect(0.0, 0.0, 1919.0, 1080.0),
                display
            )
            .is_none()
        );
        assert!(physical_to_native(device_bounds(0, 0, 0, 100), screen, display).is_none());
        assert!(physical_to_native(device_bounds(i32::MAX, 0, 1, 100), screen, display).is_none());
    }

    #[test]
    fn physical_endpoints_and_dimensions_must_fit_without_rounding() {
        assert_eq!(
            physical_bounds(-100.0, -200.0, 300.0, 400.0),
            Some(device_bounds(-100, -200, 400, 600))
        );
        assert_eq!(
            physical_bounds(i32::MAX as f64 - 2.0, 0.0, i32::MAX as f64, 1.0),
            Some(device_bounds(i32::MAX - 2, 0, 2, 1))
        );
        for (left, right) in [
            (0.5, 100.0),
            (0.0, i32::MAX as f64 + 1.0),
            (i32::MIN as f64, i32::MAX as f64),
            (100.0, 99.0),
        ] {
            assert!(physical_bounds(left, 0.0, right, 100.0).is_none());
        }
    }

    #[test]
    fn display_selection_uses_half_open_bounds_and_rejects_ambiguity() {
        let left = display(1, 0.0, 0.0, 2.0);
        let right = display(2, 1920.0, 0.0, 2.0);
        assert_eq!(
            unique_display_at([left, right], point(DevicePixels(3839), DevicePixels(10))),
            Some(left)
        );
        assert_eq!(
            unique_display_at([left, right], point(DevicePixels(3840), DevicePixels(10))),
            Some(right)
        );
        assert_eq!(
            unique_display_at([left, right], point(DevicePixels(-1), DevicePixels(10))),
            None
        );
        let overlap = display(3, 0.0, 0.0, 2.0);
        assert_eq!(
            unique_display_at([left, overlap], point(DevicePixels(10), DevicePixels(10))),
            None
        );
    }

    fn pointer_frame(x: i32) -> PlatformNativePointerPhysicalFrame {
        let geometry =
            PlatformWindowPhysicalGeometry::try_new(device_bounds(0, 0, 800, 600), 2.0).unwrap();
        PlatformNativePointerPhysicalFrame::new(point(DevicePixels(x), DevicePixels(100)), geometry)
    }

    #[test]
    fn native_pointer_scopes_mask_nested_non_native_input_and_restore_the_callback() {
        let state = Rc::new(MacNativePointerPhysicalFrameState::default());
        let frame = pointer_frame(10);
        assert_eq!(state.current(), None);
        {
            let _outer = MacNativePointerPhysicalFrameScope::enter(&state, Some(frame));
            assert_eq!(state.current(), Some(frame));
            {
                // Keyboard, synthetic drag and an unprovable native sample must mask an outer
                // pointer callback instead of inheriting its desktop fact.
                let _mask = MacNativePointerPhysicalFrameScope::enter(&state, None);
                assert_eq!(state.current(), None);
                {
                    let nested = pointer_frame(20);
                    let _inner = MacNativePointerPhysicalFrameScope::enter(&state, Some(nested));
                    assert_eq!(state.current(), Some(nested));
                }
                assert_eq!(state.current(), None);
            }
            assert_eq!(state.current(), Some(frame));
        }
        assert_eq!(state.current(), None);
    }

    #[test]
    fn closing_and_unwinding_never_leave_a_physical_pointer_frame_installed() {
        let state = Rc::new(MacNativePointerPhysicalFrameState::default());
        {
            let _outer = MacNativePointerPhysicalFrameScope::enter(&state, Some(pointer_frame(10)));
            {
                let _inner =
                    MacNativePointerPhysicalFrameScope::enter(&state, Some(pointer_frame(20)));
                state.invalidate();
                assert_eq!(state.current(), None);
            }
            assert_eq!(state.current(), None);
        }
        assert_eq!(state.current(), None);
        let mut boundary = MacWindowCallbackPanicBoundary::default();
        boundary.deliver(|| {
            let _frame = MacNativePointerPhysicalFrameScope::enter(&state, Some(pointer_frame(30)));
            panic!("native pointer callback panic");
        });
        assert!(boundary.into_first_panic().is_some());
        assert_eq!(state.current(), None);
    }

    #[test]
    fn native_screen_point_can_bind_a_target_outside_the_source_window() {
        let source = display(1, 0.0, 0.0, 2.0);
        let target = display(2, 1920.0, 0.0, 2.0);
        let point = native_to_physical(
            rect(2100.0, 800.0, 0.0, 0.0),
            rect(0.0, 0.0, 1920.0, 1080.0),
            source,
        )
        .unwrap()
        .origin;
        assert_eq!(
            point,
            open_gpui::point(DevicePixels(4200), DevicePixels(560))
        );
        let geometry =
            PlatformWindowPhysicalGeometry::try_new(device_bounds(200, 200, 800, 600), 2.0)
                .unwrap()
                .with_display_observation(source)
                .unwrap();
        let frame = PlatformNativePointerPhysicalFrame::new(point, geometry)
            .with_target_display(unique_display_at([source, target], point).unwrap())
            .unwrap();
        assert_eq!(frame.source_geometry(), geometry);
        assert_eq!(frame.target_display(), Some(target));
        assert!(!geometry.contains_global(point));
    }

    #[test]
    fn preparation_and_synchronous_retirement_cancel_older_queued_work() {
        let domain = WindowMutationDomain::Placement;
        let mut mutations = MacWindowMutations::default();
        assert!(!mutations.queue(domain, 1));
        mutations.prepare(domain, 1);
        assert!(mutations.queue(domain, 1));
        assert!(!mutations.queue(domain, 1));
        mutations.prepare(domain, 2);
        assert!(!mutations.is_current(domain, 1));
        assert!(!mutations.queue(domain, 1));
        mutations.retire(domain, 1);
        assert!(mutations.queue(domain, 2));
        mutations.prepare(domain, 3);
        mutations.retire(domain, 3);
        assert!(!mutations.is_current(domain, 2));
        assert!(!mutations.queue(domain, 3));
    }

    #[test]
    fn mutation_domains_remain_independent_and_invalidation_stops_delivery() {
        let placement = WindowMutationDomain::Placement;
        let pointer = WindowMutationDomain::PointerInput;
        let mut mutations = MacWindowMutations::default();
        mutations.prepare(placement, 1);
        mutations.prepare(pointer, 1);
        assert!(mutations.queue(placement, 1));
        assert!(mutations.queue(pointer, 1));
        mutations.invalidate(placement);
        assert!(!mutations.is_current(placement, 1));
        assert!(mutations.is_current(pointer, 1));
        mutations.retire(pointer, 1);
        assert!(!mutations.is_current(pointer, 1));
        assert!(!mutations.queue(pointer, 1));
        mutations.prepare(placement, 2);
        assert!(mutations.queue(placement, 2));
        mutations.clear();
        assert!(!mutations.is_current(placement, 2));
    }

    #[test]
    fn live_mutations_require_windowed_native_state() {
        let windowed = complete_native_observation();
        assert!(is_windowed(windowed));
        for observation in [
            MacWindowNativeObservation {
                is_minimized: true,
                ..windowed
            },
            MacWindowNativeObservation {
                is_maximized: true,
                ..windowed
            },
            MacWindowNativeObservation {
                is_fullscreen: true,
                ..windowed
            },
            MacWindowNativeObservation {
                simple_fullscreen_restore_bounds: Some(windowed.bounds),
                ..windowed
            },
        ] {
            assert!(!is_windowed(observation));
        }
    }

    #[test]
    fn missing_physical_readback_is_a_rejection_and_preserves_typed_failures() {
        use PlatformWindowMutationTerminal::{Observed, Rejected, Unsupported, WindowClosed};
        let geometry = PlatformWindowPhysicalGeometry::try_new(device_bounds(0, 0, 800, 600), 2.0);
        assert_eq!(
            terminal_for_geometry(WindowMutationDomain::Placement, Observed, None),
            Rejected
        );
        assert_eq!(
            terminal_for_geometry(WindowMutationDomain::Placement, Observed, geometry),
            Observed
        );
        assert_eq!(
            terminal_for_geometry(WindowMutationDomain::PointerInput, Observed, None),
            Observed
        );
        for terminal in [Rejected, Unsupported, WindowClosed] {
            assert_eq!(
                terminal_for_geometry(WindowMutationDomain::Placement, terminal, None),
                terminal
            );
            assert_eq!(
                terminal_for_geometry(WindowMutationDomain::Placement, terminal, geometry),
                terminal
            );
        }
    }

    #[test]
    fn typed_mutation_events_retain_physical_and_pointer_facts_in_native_order() {
        let initial = complete_native_observation();
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        let client = rect(-50.0, 480.0, 400.0, 300.0);
        let outer = rect(-50.0, 480.0, 400.0, 322.0);
        let display = physical_display(initial.topology_generation, initial.display, 2.0).unwrap();
        let placed = MacWindowNativeObservation {
            bounds: appkit_content_rect_to_global_client_bounds(
                client,
                screen,
                initial.display.bounds(),
            ),
            native_outer_rect: outer,
            native_client_rect: client,
            physical_geometry: geometry_for_display(screen, outer, client, display),
            ..initial
        };
        let pointer_disabled = MacWindowNativeObservation {
            accepts_pointer_input: false,
            ..placed
        };
        let terminal = |domain, generation| MacWindowObservationEvent::Mutation {
            domain,
            generation,
            terminal: PlatformWindowMutationTerminal::Observed,
        };
        let mut events = MacWindowPendingObservationEvents::default();
        events.record(MacWindowPendingObservationEvent::observed(
            MacWindowObservationEvent::Moved,
            initial,
        ));
        events.record(MacWindowPendingObservationEvent::observed(
            terminal(WindowMutationDomain::Placement, 3),
            placed,
        ));
        events.record(MacWindowPendingObservationEvent::observed(
            terminal(WindowMutationDomain::PointerInput, 8),
            pointer_disabled,
        ));
        let batches = events.into_effect_batches(initial);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.observation)
                .collect::<Vec<_>>(),
            vec![initial, placed, pointer_disabled, initial]
        );
        assert_eq!(
            batches[1].event.unwrap().kind,
            terminal(WindowMutationDomain::Placement, 3)
        );
        assert_eq!(
            batches[2].event.unwrap().kind,
            terminal(WindowMutationDomain::PointerInput, 8)
        );
        assert!(batches[3].event.is_none());
        let changes = MacWindowNativeObservationChanges::between(Some(placed), pointer_disabled);
        assert!(changes.state_changed);
        assert!(!changes.renderer_geometry_changed);
    }
}
