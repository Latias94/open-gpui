//! Feature-free downstream CPU/value consumption of the production HostedSurface constructor.
//!
//! App/Window interface closures are deliberately unexecuted. The unbound controls below need
//! no platform or executor; the renderer never draws. This target does not qualify GPU output,
//! logical-root execution, a native window, an event loop or an independent host implementation.

use open_gpui::{
    App, AppContext as _, DevicePixels, DispatchEventResult, Empty, HeadlessAtlas, HostedSurface,
    HostedSurfaceGeometry, HostedSurfaceOptions, HostedSurfaceRenderer, MouseDownEvent, Pixels,
    PlatformAtlas, PlatformFrameSubmission, PlatformInput, PlatformPresentationShutdownOutcome,
    PlatformWindowPresentOutcome, PreparedPlatformPresentationShutdown, RequestFrameOptions, Scene,
    Size, WindowHandle, WindowOpenError, WindowPresentationShutdownTicket, px, size,
};
use std::{cell::Cell, rc::Rc, sync::Arc};

struct ConstructorRenderer {
    atlas: Arc<dyn PlatformAtlas>,
    draws: Cell<usize>,
    shutdowns: Cell<usize>,
}

impl ConstructorRenderer {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            atlas: Arc::new(HeadlessAtlas::default()),
            draws: Cell::new(0),
            shutdowns: Cell::new(0),
        })
    }
}

impl HostedSurfaceRenderer for ConstructorRenderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn draw(
        &self,
        _: &Scene,
        _: HostedSurfaceGeometry,
        _: &PlatformFrameSubmission,
    ) -> PlatformWindowPresentOutcome {
        self.draws.set(self.draws.get() + 1);
        PlatformWindowPresentOutcome::Rejected
    }

    fn prepare_shutdown(
        &self,
        ticket: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        self.shutdowns.set(self.shutdowns.get() + 1);
        PreparedPlatformPresentationShutdown::new(ticket, |ticket| {
            assert!(ticket.acknowledge_quiesced());
            PlatformPresentationShutdownOutcome::Quiesced
        })
    }
}

#[test]
fn downstream_constructor_and_geometry_are_available_without_optional_features() {
    let renderer = ConstructorRenderer::new();
    let options = HostedSurfaceOptions {
        size: size(px(10.25), px(20.5)),
        scale_factor: 1.5,
        ..Default::default()
    };
    let surface = HostedSurface::new(options, renderer.clone(), || {})
        .expect("construct the actual unbound production surface without a platform");
    let geometry = surface.geometry();
    assert_eq!(geometry.size(), options.size);
    assert_eq!(geometry.scale_factor(), options.scale_factor);
    assert_eq!(
        geometry.physical_size(),
        size(DevicePixels(16), DevicePixels(31))
    );
    assert_eq!(
        geometry,
        HostedSurfaceGeometry::new(options.size, options.scale_factor).unwrap()
    );

    let _: fn(HostedSurfaceGeometry) -> Size<Pixels> = HostedSurfaceGeometry::size;
    let _: fn(HostedSurfaceGeometry) -> f32 = HostedSurfaceGeometry::scale_factor;
    let _: fn(HostedSurfaceGeometry) -> Size<DevicePixels> = HostedSurfaceGeometry::physical_size;
    assert!(!surface.is_closed());
    assert!(!surface.needs_frame());
    assert_eq!(renderer.draws.get(), 0);
    surface.close().unwrap();
    assert_eq!(renderer.shutdowns.get(), 0);
}

#[test]
fn downstream_unbound_driver_preserves_demand_but_refuses_execution_and_closed_controls() {
    let renderer = ConstructorRenderer::new();
    let wakes = Rc::new(Cell::new(0));
    let wake_count = wakes.clone();
    let surface = HostedSurface::new(
        HostedSurfaceOptions::default(),
        renderer.clone(),
        move || {
            wake_count.set(wake_count.get() + 1);
        },
    )
    .unwrap();
    let clone = surface.clone();
    clone
        .request_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
            ..Default::default()
        })
        .unwrap();
    assert!(surface.needs_frame());
    assert!(wakes.get() > 0);
    assert!(surface.drive_frame().is_err());
    assert!(surface.needs_frame());
    assert!(
        surface
            .dispatch_input(PlatformInput::MouseDown(MouseDownEvent::default()))
            .is_err()
    );
    let handler_called = Cell::new(false);
    assert!(
        surface
            .with_input_handler(|_| handler_called.set(true))
            .is_err()
    );
    assert!(!handler_called.get());
    clone.set_geometry(size(px(0.0), px(48.0)), 2.0).unwrap();
    assert_eq!(
        surface.geometry().physical_size(),
        size(DevicePixels(0), DevicePixels(96))
    );
    clone.set_geometry(size(px(32.0), px(48.0)), 2.0).unwrap();
    assert_eq!(surface.geometry().size(), size(px(32.0), px(48.0)));
    clone.set_active(false).unwrap();
    clone.set_visible(false).unwrap();
    clone.set_active(true).unwrap();
    clone.set_visible(true).unwrap();
    assert_eq!(renderer.draws.get(), 0);
    clone.close().unwrap();
    assert!(surface.is_closed());
    assert!(!surface.needs_frame());
    assert!(
        surface
            .request_frame(RequestFrameOptions::default())
            .is_err()
    );
    assert!(surface.drive_frame().is_err());
    assert!(surface.with_input_handler(|_| ()).is_err());
    assert!(surface.set_geometry(size(px(16.0), px(16.0)), 1.0).is_err());
    assert!(surface.set_active(true).is_err());
    assert!(surface.set_visible(true).is_err());
    surface.close().unwrap();
    assert_eq!(renderer.draws.get(), 0);
    assert_eq!(renderer.shutdowns.get(), 0);
}

#[test]
fn downstream_invalid_geometry_returns_errors_without_binding_or_retaining_a_renderer() {
    let renderer = ConstructorRenderer::new();
    for (size, scale_factor) in [
        (size(px(-1.0), px(20.0)), 1.0),
        (size(px(f32::NAN), px(20.0)), 1.0),
        (size(px(10.0), px(f32::INFINITY)), 1.0),
        (size(px(10.0), px(20.0)), 0.0),
        (size(px(10.0), px(20.0)), -1.0),
        (size(px(10.0), px(20.0)), f32::NAN),
        (size(px(f32::MAX), px(20.0)), 2.0),
    ] {
        let result = HostedSurface::new(
            HostedSurfaceOptions {
                size,
                scale_factor,
                ..Default::default()
            },
            renderer.clone(),
            || {},
        );
        assert!(result.is_err());
        assert!(HostedSurfaceGeometry::new(size, scale_factor).is_err());
        assert_eq!(Rc::strong_count(&renderer), 1);
    }
    let surface =
        HostedSurface::new(HostedSurfaceOptions::default(), renderer.clone(), || {}).unwrap();
    let original = surface.geometry();
    assert!(
        surface
            .set_geometry(size(px(40.0), px(30.0)), f32::INFINITY)
            .is_err()
    );
    assert_eq!(surface.geometry(), original);
    assert!(!surface.needs_frame());
    surface.close().unwrap();
    assert_eq!(renderer.draws.get(), 0);
    assert_eq!(renderer.shutdowns.get(), 0);
}

#[test]
fn downstream_typed_app_and_driver_interfaces_compile_without_executing_a_platform() {
    let capture = String::from("single-use downstream root capture");
    let _open_type_only = move |app: &mut App,
                                surface: &HostedSurface|
          -> Result<WindowHandle<Empty>, WindowOpenError> {
        app.open_surface(surface, move |_, app| {
            drop(capture);
            app.new(|_| Empty)
        })
    };
    let _drive_type_only =
        |surface: &HostedSurface, input: PlatformInput| -> anyhow::Result<DispatchEventResult> {
            surface.request_frame(RequestFrameOptions::default())?;
            let _: bool = surface.needs_frame();
            let _: bool = surface.drive_frame()?;
            let _: Option<Option<String>> =
                surface.with_input_handler(|handler| handler.text_for_range(0..0, &mut None))?;
            surface.dispatch_input(input)
        };
    let _renderer_type_only = |renderer: &dyn HostedSurfaceRenderer,
                               scene: &Scene,
                               geometry: HostedSurfaceGeometry,
                               original: &PlatformFrameSubmission| {
        let _: Arc<dyn PlatformAtlas> = renderer.sprite_atlas();
        let _: PlatformWindowPresentOutcome = renderer.draw(scene, geometry, original);
    };
}
