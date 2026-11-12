use super::*;

#[test]
fn physical_attachment_extents_round_trip_at_fractional_scales() {
    for width in [0, 1, 843, 1351, 1919, 3841] {
        for scale in [0.75, 1.0, 1.25, 1.5, 1.75, 2.25] {
            let extent = size(DevicePixels(width), DevicePixels(width + 2));
            let geometry = HostedSurfaceGeometry::from_physical_size(extent, scale).unwrap();
            assert_eq!(
                geometry.physical_size(),
                extent,
                "width={width}, scale={scale}"
            );
            assert_eq!(
                HostedSurfaceGeometry::new(geometry.size(), scale)
                    .unwrap()
                    .physical_size(),
                extent
            );
        }
    }
    for scale in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        assert!(
            HostedSurfaceGeometry::from_physical_size(
                size(DevicePixels(843), DevicePixels(641)),
                scale
            )
            .is_err()
        );
    }
    assert!(
        HostedSurfaceGeometry::from_physical_size(size(DevicePixels(-1), DevicePixels(1)), 1.0)
            .is_err()
    );
    assert!(
        HostedSurfaceGeometry::from_physical_size(
            size(DevicePixels(i32::MAX), DevicePixels(1)),
            1.0
        )
        .is_err()
    );
}
use crate::{
    HeadlessAtlas, MouseMoveEvent, PlatformFrameDeliveryMode, PlatformFrameSubmissionStatus,
    PlatformInputCallback, PlatformNativeWindowRetirementOutcome,
    PlatformPresentationShutdownOutcome, PlatformWindowCommand, PlatformWindowCommandOutcome,
    PointerCancelEvent, PointerCancelReason, WindowHandle,
};
use open_gpui_scheduler::Instant;
use raw_window_handle::HandleError;
use slotmap::SlotMap;
use std::time::Duration;

struct Renderer {
    atlas: Arc<HeadlessAtlas>,
    outcome: Cell<PlatformWindowPresentOutcome>,
    geometries: RefCell<Vec<HostedSurfaceGeometry>>,
    retained_token: RefCell<Option<PlatformFrameSubmission>>,
    during_draw: RefCell<Option<Box<dyn FnOnce()>>>,
    shutdown_ready: Rc<Cell<bool>>,
    shutdown_preparations: Cell<usize>,
    gpu_prefix: Rc<RefCell<Option<Rc<()>>>>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            atlas: Arc::new(HeadlessAtlas::default()),
            outcome: Cell::new(PlatformWindowPresentOutcome::Deferred),
            geometries: RefCell::new(Vec::new()),
            retained_token: RefCell::new(None),
            during_draw: RefCell::new(None),
            shutdown_ready: Rc::new(Cell::new(true)),
            shutdown_preparations: Cell::new(0),
            gpu_prefix: Rc::new(RefCell::new(None)),
        }
    }
}

impl HostedSurfaceRenderer for Renderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn draw(
        &self,
        _scene: &Scene,
        geometry: HostedSurfaceGeometry,
        submission: &PlatformFrameSubmission,
    ) -> PlatformWindowPresentOutcome {
        self.geometries.borrow_mut().push(geometry);
        self.retained_token.replace(Some(submission.clone()));
        let callback = self.during_draw.take();
        if let Some(callback) = callback {
            callback();
        }
        self.outcome.get()
    }

    fn prepare_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        self.shutdown_preparations
            .set(self.shutdown_preparations.get() + 1);
        let ready = self.shutdown_ready.clone();
        let prefix = self.gpu_prefix.clone();
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            if !ready.get() {
                return PlatformPresentationShutdownOutcome::Rejected;
            }
            let prefix = prefix.take();
            drop(prefix);
            if shutdown.acknowledge_quiesced() {
                PlatformPresentationShutdownOutcome::Quiesced
            } else {
                PlatformPresentationShutdownOutcome::Rejected
            }
        })
    }
}

fn surface(options: HostedSurfaceOptions) -> (HostedSurface, Rc<Renderer>, Rc<Cell<usize>>) {
    let renderer = Rc::new(Renderer::default());
    let wakes = Rc::new(Cell::new(0));
    let surface = HostedSurface::new(options, renderer.clone(), {
        let wakes = wakes.clone();
        move || wakes.set(wakes.get() + 1)
    })
    .unwrap();
    (surface, renderer, wakes)
}

fn window_id() -> WindowId {
    SlotMap::<WindowId, ()>::with_key().insert(())
}

fn params(surface: &HostedSurface) -> WindowParams {
    let options = surface.window_options();
    let window_bounds = options.window_bounds.unwrap();
    WindowParams {
        window_bounds,
        bounds: Bounds::new(Point::default(), surface.geometry().size()),
        titlebar: options.titlebar,
        kind: options.kind,
        is_movable: options.is_movable,
        app_owns_titlebar_drag: options.app_owns_titlebar_drag,
        is_resizable: options.is_resizable,
        is_minimizable: options.is_minimizable,
        accepts_pointer_input: options.accepts_pointer_input,
        focus_on_appearing: options.focus_on_appearing,
        activation_policy: options.activation_policy,
        transient_for: None,
        show: options.show,
        provisional_session: None,
        icon: None,
        display_id: None,
        window_min_size: None,
        #[cfg(target_os = "macos")]
        tabbing_identifier: None,
        #[cfg(target_os = "macos")]
        use_display_p3: false,
    }
}

fn bind(surface: &HostedSurface, id: WindowId) -> Box<dyn PlatformWindow> {
    surface
        .create_platform_window(
            WindowHandle::<crate::Empty>::new(id).into(),
            params(surface),
        )
        .unwrap()
}

fn submission(id: WindowId, generation: u64) -> PlatformFrameSubmission {
    PlatformFrameSubmission::new(id, generation, 1, Rc::new(|_, _| true))
}

fn retire(binding: Box<dyn PlatformWindow>, id: WindowId) {
    let shutdown =
        binding.prepare_presentation_shutdown(WindowPresentationShutdownTicket::new(id, 1));
    assert_eq!(
        shutdown.quiesce(),
        PlatformPresentationShutdownOutcome::Quiesced
    );
    assert_eq!(
        binding.retire_native_window(shutdown.ticket()),
        PlatformNativeWindowRetirementOutcome::BindingDetached
    );
    assert!(shutdown.ticket().acknowledge_detached());
    drop(binding);
}

#[test]
fn physical_extent_covers_fractional_pixels_and_preserves_zero() {
    let geometry = HostedSurfaceGeometry::new(size(px(10.25), px(0.0)), 1.5).unwrap();
    assert_eq!(geometry.size(), size(px(10.25), px(0.0)));
    assert_eq!(geometry.scale_factor(), 1.5);
    assert_eq!(
        geometry.physical_size(),
        size(DevicePixels(16), DevicePixels(0))
    );
    assert!(geometry.is_suspended());
}

#[test]
fn invalid_geometry_fails_before_lossy_conversion() {
    for dimension in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        assert!(HostedSurfaceGeometry::new(size(px(dimension), px(1.0)), 1.0).is_err());
        assert!(HostedSurfaceGeometry::new(size(px(1.0), px(dimension)), 1.0).is_err());
    }
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(HostedSurfaceGeometry::new(size(px(1.0), px(1.0)), scale).is_err());
    }
    assert!(HostedSurfaceGeometry::new(size(px(i32::MAX as f32), px(1.0)), 1.0).is_err());
    assert!(HostedSurfaceGeometry::new(size(px(1.0), px(1.0)), f32::MAX).is_err());
    let geometry = HostedSurfaceGeometry::new(size(px(f32::MIN_POSITIVE), px(1.0)), 1.0).unwrap();
    assert_eq!(geometry.physical_size().width, DevicePixels(1));
}

#[test]
fn frame_demand_uses_original_merge_and_drives_one_turn_without_borrows() {
    let (surface, _, wakes) = surface(HostedSurfaceOptions::default());
    let binding = bind(&surface, window_id());
    let delivered = Rc::new(RefCell::new(Vec::new()));
    binding.on_request_frame(Box::new({
        let surface = surface.clone();
        let delivered = delivered.clone();
        move |options| {
            assert!(!surface.needs_frame());
            let first = delivered.borrow().is_empty();
            delivered.borrow_mut().push(options);
            if first {
                surface
                    .request_frame(RequestFrameOptions::default())
                    .unwrap();
            }
        }
    }));
    let earlier = Instant::now();
    let later = earlier + Duration::from_millis(1);
    let requests = [
        RequestFrameOptions {
            force_render: true,
            signal_at: Some(later),
            signal_source: FrameRequestSource::LocalSchedule,
            ..RequestFrameOptions::default()
        },
        RequestFrameOptions {
            require_presentation: true,
            signal_at: Some(earlier),
            signal_source: FrameRequestSource::NativeCallback,
            ..RequestFrameOptions::default()
        },
    ];
    for request in requests {
        surface.request_frame(request).unwrap();
    }
    assert!(surface.drive_frame().unwrap());
    assert_eq!(delivered.borrow().len(), 1);
    assert_eq!(
        delivered.borrow()[0],
        RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            signal_at: Some(earlier),
            signal_source: FrameRequestSource::NativeCallback,
        }
    );
    assert!(surface.needs_frame());
    assert!(surface.drive_frame().unwrap());
    assert!(!surface.needs_frame());
    assert!(!surface.drive_frame().unwrap());
    assert_eq!(
        delivered.borrow()[1].signal_source,
        FrameRequestSource::LocalSchedule
    );
    #[cfg(feature = "profiler")]
    assert!(delivered.borrow()[1].signal_at.is_some());
    #[cfg(not(feature = "profiler"))]
    assert!(delivered.borrow()[1].signal_at.is_none());
    assert_eq!(wakes.get(), 3);

    let weak_waker = binding.frame_waker().unwrap();
    weak_waker();
    binding.schedule_frame();
    assert!(surface.needs_frame());
    assert!(surface.drive_frame().unwrap());
    assert_eq!(delivered.borrow().len(), 3);
    drop(binding);
    let wake_count = wakes.get();
    weak_waker();
    assert!(!surface.needs_frame());
    assert_eq!(wakes.get(), wake_count);
}

#[test]
fn geometry_aba_cancels_original_token_before_resize_callback() {
    let (surface, renderer, _) = surface(HostedSurfaceOptions::default());
    let id = window_id();
    let binding = bind(&surface, id);
    let original = submission(id, 1);
    assert_eq!(
        binding.draw_with_submission(&Scene::default(), &original),
        PlatformWindowPresentOutcome::Deferred
    );
    let retained = renderer.retained_token.borrow().as_ref().unwrap().clone();
    let observed = Rc::new(Cell::new(0));
    binding.on_resize(Box::new({
        let retained = retained.clone();
        let surface = surface.clone();
        let observed = observed.clone();
        move |size, scale| {
            assert_eq!(retained.status(), PlatformFrameSubmissionStatus::Cancelled);
            assert_eq!(surface.geometry().size(), size);
            assert_eq!(surface.geometry().scale_factor(), scale);
            observed.set(observed.get() + 1);
        }
    }));
    let geometry = surface.geometry();
    surface
        .set_geometry(geometry.size(), geometry.scale_factor())
        .unwrap();
    assert_eq!(observed.get(), 1);
    assert!(!retained.complete(PlatformWindowPresentOutcome::Submitted));
    let second = submission(id, 2);
    assert_eq!(
        binding.draw_with_submission(&Scene::default(), &second),
        PlatformWindowPresentOutcome::Deferred
    );
    surface
        .set_geometry(size(px(640.0), px(480.0)), 2.0)
        .unwrap();
    surface
        .set_geometry(geometry.size(), geometry.scale_factor())
        .unwrap();
    assert_eq!(second.status(), PlatformFrameSubmissionStatus::Cancelled);
    assert_eq!(surface.geometry(), geometry);
    assert!(surface.needs_frame());
    retire(binding, id);
}

#[test]
fn invalid_geometry_does_not_invalidate_a_current_delivery() {
    let (surface, _, wakes) = surface(HostedSurfaceOptions::default());
    let id = window_id();
    let binding = bind(&surface, id);
    let original = submission(id, 1);
    binding.draw_with_submission(&Scene::default(), &original);
    let geometry = surface.geometry();
    assert!(surface.set_geometry(size(px(-1.0), px(2.0)), 1.0).is_err());
    assert_eq!(surface.geometry(), geometry);
    assert!(original.is_pending());
    assert!(!surface.needs_frame());
    assert_eq!(wakes.get(), 0);
    retire(binding, id);
}

#[test]
fn zero_extent_parks_without_renderer_or_idle_retries_and_resize_repaints() {
    let (surface, renderer, _) = surface(HostedSurfaceOptions {
        size: size(px(0.0), px(600.0)),
        ..HostedSurfaceOptions::default()
    });
    let id = window_id();
    let binding = bind(&surface, id);
    let original = submission(id, 1);
    assert_eq!(
        binding.frame_delivery_mode(),
        PlatformFrameDeliveryMode::Deferred
    );
    assert_eq!(
        binding.draw_with_submission(&Scene::default(), &original),
        PlatformWindowPresentOutcome::Deferred
    );
    assert_eq!(
        binding.draw_with_submission(&Scene::default(), &original),
        PlatformWindowPresentOutcome::Deferred
    );
    assert!(renderer.geometries.borrow().is_empty());
    assert!(original.is_pending());
    assert!(!surface.needs_frame());
    surface
        .set_geometry(size(px(640.25), px(480.0)), 1.5)
        .unwrap();
    assert_eq!(original.status(), PlatformFrameSubmissionStatus::Cancelled);
    let received = Rc::new(RefCell::new(None));
    binding.on_request_frame(Box::new({
        let received = received.clone();
        move |request| {
            received.replace(Some(request));
        }
    }));
    assert!(surface.drive_frame().unwrap());
    let request = received.borrow().unwrap();
    assert!(request.force_render && request.require_presentation);
    let fresh = submission(id, 2);
    binding.draw_with_submission(&Scene::default(), &fresh);
    assert_eq!(renderer.geometries.borrow().len(), 1);
    assert_eq!(
        renderer.geometries.borrow()[0].physical_size(),
        size(DevicePixels(961), DevicePixels(720))
    );
    assert!(!surface.needs_frame());
    retire(binding, id);
}

#[test]
fn foreign_full_window_generation_is_rejected_and_binding_is_single_use() {
    let (surface, renderer, _) = surface(HostedSurfaceOptions::default());
    let mut ids = SlotMap::<WindowId, ()>::with_key();
    let id = ids.insert(());
    let binding = bind(&surface, id);
    ids.remove(id);
    let reused = ids.insert(());
    assert_ne!(id, reused);
    let foreign = submission(reused, 1);
    assert_eq!(
        binding.draw_with_submission(&Scene::default(), &foreign),
        PlatformWindowPresentOutcome::Rejected
    );
    assert!(renderer.geometries.borrow().is_empty());
    assert!(
        surface
            .create_platform_window(
                WindowHandle::<crate::Empty>::new(reused).into(),
                params(&surface)
            )
            .is_err()
    );
    drop(binding);
    assert!(
        surface
            .create_platform_window(
                WindowHandle::<crate::Empty>::new(reused).into(),
                params(&surface)
            )
            .is_err()
    );
}

#[test]
fn drivers_do_not_own_close_and_binding_terminal_callback_runs_once() {
    let (surface, _, _) = surface(HostedSurfaceOptions::default());
    let surviving = surface.clone();
    let binding = bind(&surface, window_id());
    let closed = Rc::new(Cell::new(0));
    binding.on_close(Box::new({
        let closed = closed.clone();
        let surviving = surviving.clone();
        move || {
            assert!(surviving.is_closed());
            closed.set(closed.get() + 1);
        }
    }));
    drop(surface);
    assert!(!surviving.is_closed());
    assert_eq!(closed.get(), 0);
    drop(binding);
    assert!(surviving.is_closed());
    surviving.close().unwrap();
    assert_eq!(closed.get(), 1);
    assert!(
        surviving
            .dispatch_input(PlatformInput::MouseMove(MouseMoveEvent::default()))
            .is_err()
    );
    assert!(surviving.with_input_handler(|_| ()).is_err());
}

#[test]
fn forced_close_retains_gpu_prefix_until_exact_shutdown_detaches_binding() {
    let (surface, renderer, _) = surface(HostedSurfaceOptions::default());
    let id = window_id();
    let binding = bind(&surface, id);
    let prefix = Rc::new(());
    let weak_prefix = Rc::downgrade(&prefix);
    renderer.gpu_prefix.replace(Some(prefix));
    renderer.shutdown_ready.set(false);
    let original = submission(id, 1);
    binding.draw_with_submission(&Scene::default(), &original);
    let requests = Rc::new(Cell::new(0));
    binding.on_should_close(Box::new(|| panic!("forced close must not consult a veto")));
    binding.on_close_required(Box::new({
        let requests = requests.clone();
        let surface = surface.clone();
        move || {
            assert!(surface.is_closed());
            assert!(!surface.needs_frame());
            requests.set(requests.get() + 1);
        }
    }));
    surface.close().unwrap();
    surface.close().unwrap();
    assert_eq!(requests.get(), 1);
    assert!(
        surface
            .dispatch_input(PlatformInput::MouseMove(MouseMoveEvent::default()))
            .is_err()
    );
    assert!(
        surface
            .with_input_handler(|_| panic!("closing surface must reject host text input"))
            .is_err()
    );
    assert_eq!(original.status(), PlatformFrameSubmissionStatus::Cancelled);
    let ticket = WindowPresentationShutdownTicket::new(id, 7);
    let prepared = binding.prepare_presentation_shutdown(ticket.clone());
    assert_eq!(renderer.shutdown_preparations.get(), 1);
    assert!(prepared.ticket().same_authority(&ticket));
    let repeated =
        binding.prepare_presentation_shutdown(WindowPresentationShutdownTicket::new(id, 8));
    assert!(repeated.ticket().same_authority(&ticket));
    assert_eq!(renderer.shutdown_preparations.get(), 1);
    assert_eq!(
        prepared.quiesce(),
        PlatformPresentationShutdownOutcome::Rejected
    );
    assert_eq!(
        binding.retire_native_window(&ticket),
        PlatformNativeWindowRetirementOutcome::Rejected
    );
    assert!(weak_prefix.upgrade().is_some());
    renderer.shutdown_ready.set(true);
    assert_eq!(
        prepared.quiesce(),
        PlatformPresentationShutdownOutcome::Quiesced
    );
    assert!(weak_prefix.upgrade().is_none());
    let foreign = WindowPresentationShutdownTicket::new(id, 7);
    assert_eq!(
        binding.retire_native_window(&foreign),
        PlatformNativeWindowRetirementOutcome::Rejected
    );
    assert_eq!(
        binding.retire_native_window(&ticket),
        PlatformNativeWindowRetirementOutcome::BindingDetached
    );
    assert!(!ticket.snapshot().native_terminal());
    let terminal_callbacks = Rc::new(Cell::new(0));
    binding.on_close(Box::new({
        let terminal_callbacks = terminal_callbacks.clone();
        let ticket = ticket.clone();
        move || {
            assert!(ticket.snapshot().binding_terminal());
            assert!(ticket.snapshot().detached());
            assert!(!ticket.snapshot().native_terminal());
            terminal_callbacks.set(terminal_callbacks.get() + 1);
        }
    }));
    assert!(ticket.acknowledge_detached());
    drop(binding);
    assert_eq!(terminal_callbacks.get(), 1);
}

#[test]
fn native_handles_and_commands_are_unsupported_without_desktop_facts() {
    let (surface, _, _) = surface(HostedSurfaceOptions::default());
    let binding = bind(&surface, window_id());
    assert!(!binding.has_native_window());
    assert!(matches!(
        binding.window_handle(),
        Err(HandleError::NotSupported)
    ));
    assert!(matches!(
        binding.display_handle(),
        Err(HandleError::NotSupported)
    ));
    let facts = binding.platform_facts();
    assert_eq!(facts.coordinate_space, WindowCoordinateSpace::WindowLocal);
    assert!(facts.physical_geometry.is_none() && facts.display_id.is_none());
    assert!(
        !facts.accepts_activation
            && !facts.focus_on_click
            && !facts.taskbar_visible
            && !facts.topmost
    );
    let capabilities = surface.capabilities();
    assert_eq!(
        capabilities.mutations.position,
        WindowMutationSupport::Unsupported
    );
    assert_eq!(
        capabilities.mutations.size,
        WindowMutationSupport::Unsupported
    );
    assert_eq!(
        capabilities.mutations.activation_policy,
        WindowMutationSupport::CreationOnly
    );
    assert_eq!(
        capabilities.creation.focus_on_appearing,
        WindowCreationSupport::Supported
    );
    assert_eq!(
        capabilities.creation.initial_presentation_order,
        WindowInitialPresentationOrder::BeforeVisibility
    );
    for command in [
        PlatformWindowCommand::StartWindowMove,
        PlatformWindowCommand::Activate {
            request_generation: 1,
        },
        PlatformWindowCommand::RestoreNativeWindowState(Arc::from([])),
        PlatformWindowCommand::ToggleSimpleFullscreen,
    ] {
        assert_eq!(
            binding.command_dispatcher().dispatch(command),
            PlatformWindowCommandOutcome::Unsupported
        );
    }
    assert_eq!(
        binding.draw(&Scene::default()),
        PlatformWindowPresentOutcome::Rejected
    );
}

#[test]
fn logical_reveal_and_reentrant_visibility_callbacks_preserve_host_facts() {
    let (surface, renderer, _) = surface(HostedSurfaceOptions::default());
    let id = window_id();
    let binding = bind(&surface, id);
    assert!(binding.creation_facts().show);
    assert!(!binding.creation_facts().focus_on_appearing);
    assert!(!binding.is_visible());
    surface.set_visible(true).unwrap();
    assert!(!binding.is_visible());
    let changes = Rc::new(RefCell::new(Vec::new()));
    let nested = Rc::new(Cell::new(false));
    binding.on_visibility_change(Box::new({
        let surface = surface.clone();
        let changes = changes.clone();
        let nested = nested.clone();
        move |visibility| {
            changes.borrow_mut().push(visibility);
            if visibility.is_visible() && !nested.replace(true) {
                surface.set_visible(false).unwrap();
            }
        }
    }));
    assert_eq!(
        binding
            .command_dispatcher()
            .dispatch(PlatformWindowCommand::CompleteInitialPresentation { activate: false }),
        PlatformWindowCommandOutcome::Accepted
    );
    assert_eq!(
        *changes.borrow(),
        vec![WindowVisibility::Visible, WindowVisibility::Hidden]
    );
    surface.set_visible(true).unwrap();
    assert!(binding.is_visible());
    assert_eq!(
        *changes.borrow(),
        vec![
            WindowVisibility::Visible,
            WindowVisibility::Hidden,
            WindowVisibility::Visible
        ]
    );
    let original = submission(id, 1);
    binding.draw_with_submission(&Scene::default(), &original);
    surface.set_visible(false).unwrap();
    surface.set_visible(true).unwrap();
    assert_eq!(original.status(), PlatformFrameSubmissionStatus::Cancelled);
    assert_eq!(renderer.geometries.borrow().len(), 1);
    assert!(binding.creation_facts().show);
    retire(binding, id);
}

#[test]
fn input_preserves_exact_result_and_snapshots_through_original_slot() {
    let (surface, _, _) = surface(HostedSurfaceOptions::default());
    let binding = bind(&surface, window_id());
    assert!(
        surface
            .dispatch_input(PlatformInput::MouseMove(MouseMoveEvent::default()))
            .is_err()
    );
    let result = DispatchEventResult {
        propagate: false,
        default_prevented: true,
    };
    let cancellations = Rc::new(Cell::new(0));
    binding.on_input(PlatformInputCallback::new_unleased_for_test(Box::new({
        let state = surface.state.clone();
        let cancellations = cancellations.clone();
        move |input| {
            assert!(state.pending_frame.try_borrow_mut().is_ok());
            if let PlatformInput::PointerCanceled(_) = input {
                cancellations.set(cancellations.get() + 1);
            }
            result
        }
    })));
    let modifiers = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    assert_eq!(
        surface
            .dispatch_input(PlatformInput::MouseMove(MouseMoveEvent {
                position: crate::point(px(4.0), px(8.0)),
                modifiers,
                ..MouseMoveEvent::default()
            }))
            .unwrap(),
        result
    );
    assert_eq!(binding.mouse_position(), crate::point(px(4.0), px(8.0)));
    assert!(binding.is_hovered());
    assert_eq!(binding.modifiers(), modifiers);
    surface
        .dispatch_input(PlatformInput::ModifiersChanged(
            crate::ModifiersChangedEvent {
                modifiers,
                capslock: Capslock { on: true },
            },
        ))
        .unwrap();
    assert!(binding.capslock().on);
    surface
        .dispatch_input(PlatformInput::MouseMove(MouseMoveEvent {
            position: crate::point(px(-4.0), px(8.0)),
            ..MouseMoveEvent::default()
        }))
        .unwrap();
    assert!(!binding.is_hovered());
    assert_eq!(
        surface
            .dispatch_input(PlatformInput::PointerCanceled(PointerCancelEvent {
                reason: PointerCancelReason::WindowDeactivated,
            }))
            .unwrap(),
        result
    );
    assert_eq!(cancellations.get(), 1);
    assert!(surface.with_input_handler(|_| ()).unwrap().is_none());
}

#[test]
fn host_active_edges_cancel_delivery_without_native_activation_evidence() {
    let (surface, _, _) = surface(HostedSurfaceOptions {
        active: false,
        ..HostedSurfaceOptions::default()
    });
    let id = window_id();
    let binding = bind(&surface, id);
    let original = submission(id, 1);
    binding.draw_with_submission(&Scene::default(), &original);
    let observations = Rc::new(RefCell::new(Vec::new()));
    binding.on_active_status_change(Box::new({
        let original = original.clone();
        let observations = observations.clone();
        move |observation| {
            assert_eq!(original.status(), PlatformFrameSubmissionStatus::Cancelled);
            observations.borrow_mut().push(observation);
        }
    }));
    surface.set_active(true).unwrap();
    surface.set_active(true).unwrap();
    surface.set_active(false).unwrap();
    let observations = observations.borrow();
    assert_eq!(observations.len(), 2);
    assert!(observations[0].active());
    assert!(!observations[1].active());
    assert!(
        observations
            .iter()
            .all(|observation| !observation.exact_native_positive())
    );
    retire(binding, id);
}

#[test]
fn text_configuration_caret_and_changes_are_coalesced_without_another_session() {
    let (surface, _, wakes) = surface(HostedSurfaceOptions::default());
    let mut binding = bind(&surface, window_id());
    let configuration = TextInputConfiguration {
        autocorrect: true,
        ..TextInputConfiguration::default()
    };
    binding.set_text_input_configuration(configuration.clone());
    let revision = surface.text_input_state().revision;
    binding.set_text_input_configuration(configuration.clone());
    assert_eq!(surface.text_input_state().revision, revision);
    let caret = Bounds::new(crate::point(px(3.0), px(4.0)), size(px(1.0), px(20.0)));
    binding.update_ime_position(caret);
    let dispatcher = binding.command_dispatcher();
    for command in [
        PlatformWindowCommand::TextInputStateChanged(TextInputStateChange::FocusGained),
        PlatformWindowCommand::TextInputStateChanged(TextInputStateChange::ContentChanged),
        PlatformWindowCommand::TextInputStateChanged(TextInputStateChange::SelectionChanged),
        PlatformWindowCommand::ShowSoftKeyboard,
    ] {
        assert_eq!(
            dispatcher.dispatch(command),
            PlatformWindowCommandOutcome::Accepted
        );
    }
    let snapshot = surface.text_input_state();
    assert_eq!(snapshot.configuration, configuration);
    assert_eq!(snapshot.caret_bounds, Some(caret));
    assert!(snapshot.focused && snapshot.soft_keyboard_visible);
    assert!(snapshot.content_revision > revision);
    assert!(snapshot.selection_revision > snapshot.content_revision);
    assert!(wakes.get() >= 6);
    assert!(surface.with_input_handler(|_| ()).unwrap().is_none());
    assert_eq!(
        dispatcher.dispatch(PlatformWindowCommand::HideSoftKeyboard),
        PlatformWindowCommandOutcome::Accepted
    );
    binding.text_input_state_changed(TextInputStateChange::FocusLost);
    let snapshot = surface.text_input_state();
    assert!(!snapshot.focused && !snapshot.soft_keyboard_visible);
    assert!(snapshot.caret_bounds.is_none());
    assert!(snapshot.content_revision > 0 && snapshot.selection_revision > 0);
}

#[test]
fn a_renderer_cannot_report_submitted_after_reentrant_source_invalidation() {
    let (surface, renderer, _) = surface(HostedSurfaceOptions::default());
    let id = window_id();
    let binding = bind(&surface, id);
    renderer
        .outcome
        .set(PlatformWindowPresentOutcome::Submitted);
    renderer.during_draw.replace(Some(Box::new({
        let surface = surface.clone();
        move || {
            let geometry = surface.geometry();
            surface
                .set_geometry(geometry.size(), geometry.scale_factor())
                .unwrap();
        }
    })));
    let original = submission(id, 1);
    assert_eq!(
        binding.draw_with_submission(&Scene::default(), &original),
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert_eq!(original.status(), PlatformFrameSubmissionStatus::Cancelled);
    retire(binding, id);
}
