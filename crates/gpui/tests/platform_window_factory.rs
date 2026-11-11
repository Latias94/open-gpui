#![cfg(feature = "bench-support")]

//! Public factory consumers using an ordinary App and the existing headless TestPlatform.
//! TestWindow submission and input observations qualify the creation transaction only;
//! they do not qualify a borrowed native handle, an external event loop or GPU presentation.

use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Application, ApplicationHandle, Bounds, Context,
    DevicePixels, Empty, HeadlessAtlas, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, NoopTextSystem, Platform, PlatformAtlas, PlatformHeadlessRenderer,
    PlatformInput, PlatformWindow, PlatformWindowPresentOutcome, QuitMode, Render,
    RequestFrameOptions, Scene, Size, Styled as _, Window, WindowBounds,
    WindowInitialPresentationStatus, WindowKind, WindowOpenFailureStage, WindowOptions,
    bench_platform, div, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(24.0), px(36.0)),
            size(px(160.0), px(120.0)),
        ))),
        focus_on_appearing: false,
        ..Default::default()
    }
}

fn headless_app(platform: Rc<dyn Platform>) -> ApplicationHandle {
    Application::with_platform(platform)
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
        })
}

fn assert_released_and_retry(app: &ApplicationHandle, original: AnyWindowHandle) {
    let retry = app
        .update(|app| {
            assert!(app.windows().is_empty());
            assert!(app.window_profile(original).is_none());
            assert!(app.update_window(original, |_, _, _| ()).is_err());
            app.open_window_detailed(options(), |_, app| app.new(|_| Empty))
        })
        .expect("rollback must leave the default factory path usable");
    assert_ne!(retry.window_id(), original.window_id());
    app.update(|app| {
        assert_eq!(app.windows(), [retry.into()]);
        retry.update(app, |_, window, _| {
            assert_eq!(window.window_handle(), retry.into());
        })
    })
    .expect("the replacement must accept an ordinary typed update");
    app.update(|app| app.update_window(retry.into(), |_, window, app| window.remove_window(app)))
        .expect("the replacement must close normally");
    app.update(|app| assert!(app.windows().is_empty()));
}

struct Counter {
    value: usize,
    rendered: Rc<Cell<Option<usize>>>,
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered.set(Some(self.value));
        div()
            .id("factory-counter")
            .size_full()
            .bg(rgb(if self.value == 0 { 0x123456 } else { 0x654321 }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.value += 1;
                    cx.notify();
                }),
            )
    }
}

#[test]
fn public_factory_receives_reserved_identity_resolved_params_and_snapshot_once() {
    let platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
    let expected_displays = platform.display_snapshot();
    let app = headless_app(platform.clone());
    let reserved = Cell::new(None);
    let factory_calls = Cell::new(0);
    let root_calls = Cell::new(0);
    let native = RefCell::new(None);
    let captured_displays = RefCell::new(None);
    let rendered = Rc::new(Cell::new(None));
    let factory_capture = String::from("single-use factory capture");
    let root_capture = String::from("single-use root capture");
    let missing_display = u64::MAX.into();
    assert!(expected_displays.find_display(missing_display).is_none());

    let handle = app
        .update(|app| {
            app.open_window_with_platform_window(
                WindowOptions {
                    display_id: Some(missing_display),
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    window_min_size: Some(size(px(80.0), px(60.0))),
                    ..options()
                },
                |handle, params, displays| {
                    drop(factory_capture);
                    factory_calls.set(factory_calls.get() + 1);
                    assert_eq!(root_calls.get(), 0);
                    reserved.set(Some(handle));
                    assert_eq!(params.display_id, None);
                    assert_eq!(params.window_bounds, options().window_bounds.unwrap());
                    assert_eq!(params.bounds, params.window_bounds.get_bounds());
                    assert!(matches!(&params.kind, WindowKind::Normal));
                    assert!(!params.is_movable);
                    assert!(!params.is_resizable);
                    assert!(!params.is_minimizable);
                    assert!(!params.focus_on_appearing);
                    assert!(params.show);
                    assert!(params.accepts_pointer_input);
                    assert_eq!(params.window_min_size, Some(size(px(80.0), px(60.0))));
                    assert_eq!(displays.generation(), expected_displays.generation());
                    assert_eq!(
                        displays
                            .displays()
                            .iter()
                            .map(|display| display.id())
                            .collect::<Vec<_>>(),
                        expected_displays
                            .displays()
                            .iter()
                            .map(|display| display.id())
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(
                        displays.primary_display().map(|display| display.id()),
                        expected_displays
                            .primary_display()
                            .map(|display| display.id())
                    );
                    let primary = displays.primary_display().unwrap();
                    assert!(Rc::ptr_eq(
                        &primary,
                        &displays.find_display(primary.id()).unwrap()
                    ));
                    captured_displays.replace(Some(displays.clone()));
                    let mut window =
                        platform.open_window_with_display_snapshot(handle, params, displays)?;
                    native.replace(Some(
                        window
                            .as_test()
                            .expect("use the public TestPlatform window")
                            .clone(),
                    ));
                    Ok(window)
                },
                |window, app| {
                    drop(root_capture);
                    root_calls.set(root_calls.get() + 1);
                    assert_eq!(factory_calls.get(), 1);
                    assert_eq!(reserved.get(), Some(window.window_handle()));
                    assert!(!window.is_resizable());
                    assert!(!window.is_minimizable());
                    app.new(|_| Counter {
                        value: 0,
                        rendered: rendered.clone(),
                    })
                },
            )
        })
        .expect("the supplied factory must complete the normal root/draw/presentation transaction");
    assert_eq!(factory_calls.get(), 1);
    assert_eq!(root_calls.get(), 1);
    assert_eq!(reserved.get(), Some(handle.into()));
    assert_eq!(rendered.get(), Some(0));
    let mut native = native.into_inner().unwrap();
    assert_eq!(
        native.display().map(|display| display.id()),
        captured_displays
            .borrow()
            .as_ref()
            .unwrap()
            .primary_display()
            .map(|display| display.id())
    );
    let initial_generation = app
        .update(|app| {
            app.update_window(handle.into(), |_, window, app| {
                assert!(app.window_profile(handle.into()).is_some());
                assert!(!window.rendered_scene().quads.is_empty());
                let facts = window.presentation_facts();
                let generation = facts
                    .frame_accepted_generation
                    .expect("the root must render before return");
                assert_eq!(facts.present_submitted_generation, Some(generation));
                assert_eq!(facts.non_empty_presented_generation, Some(generation));
                assert_eq!(
                    facts.latest_present_attempt.unwrap().outcome,
                    PlatformWindowPresentOutcome::Submitted
                );
                assert_eq!(
                    facts.initial_presentation,
                    WindowInitialPresentationStatus::Completed
                );
                generation
            })
        })
        .unwrap();

    app.update(|app| {
        handle.update(app, |root, _, cx| {
            root.value = 4;
            cx.notify();
        })
    })
    .expect("the factory-created root must support ordinary updates");
    assert!(native.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
    app.update(|_| ());
    assert_eq!(rendered.get(), Some(4));
    native.simulate_input(PlatformInput::MouseDown(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(20.0), px(20.0)),
        ..Default::default()
    }));
    assert_eq!(app.update(|app| handle.read(app).unwrap().value), 5);
    assert!(native.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
    app.update(|_| ());
    assert_eq!(rendered.get(), Some(5));
    app.update(|app| {
        app.update_window(handle.into(), |_, window, _| {
            let facts = window.presentation_facts();
            assert!(facts.frame_accepted_generation.unwrap() > initial_generation);
            assert_eq!(
                facts.present_submitted_generation,
                facts.frame_accepted_generation
            );
        })
    })
    .unwrap();
    assert_eq!(factory_calls.get(), 1);
    assert_eq!(root_calls.get(), 1);
    app.update(|app| app.update_window(handle.into(), |_, window, app| window.remove_window(app)))
        .unwrap();
    assert!(!native.fire_close_required());
    app.update(App::shutdown);
}

#[test]
fn public_factory_typed_error_preserves_native_stage_without_building_root() {
    let app = headless_app(bench_platform(None, Arc::new(NoopTextSystem::new())));
    let reserved = Cell::new(None);
    let calls = Cell::new(0);
    let root_called = Cell::new(false);
    let error = app
        .update(|app| {
            app.open_window_with_platform_window(
                options(),
                |handle, _, _| {
                    reserved.set(Some(handle));
                    calls.set(calls.get() + 1);
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "factory refused attachment",
                    )
                    .into())
                },
                |_, app| {
                    root_called.set(true);
                    app.new(|_| Empty)
                },
            )
        })
        .expect_err("factory refusal must remain a typed window-open error");
    assert_eq!(calls.get(), 1);
    assert!(!root_called.get());
    assert_eq!(error.stage(), WindowOpenFailureStage::NativeCreateOrMap);
    let source = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<io::Error>())
        .expect("the factory error must retain its original source type");
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
    assert_released_and_retry(&app, reserved.get().unwrap());
    app.update(App::shutdown);
}

#[test]
fn public_root_panic_and_close_retire_the_original_factory_window_before_retry() {
    for panic_in_root in [true, false] {
        let platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
        let app = headless_app(platform.clone());
        let reserved = Cell::new(None);
        let native = RefCell::new(None);
        let calls = Cell::new(0);
        let terminal = Rc::new(RefCell::new(Vec::new()));
        let _terminal = app.update(|app| {
            let terminal = terminal.clone();
            app.on_window_native_terminal(move |_, id| terminal.borrow_mut().push(id))
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            app.update(|app| {
                app.open_window_with_platform_window(
                    options(),
                    |handle, params, displays| {
                        reserved.set(Some(handle));
                        calls.set(calls.get() + 1);
                        let mut window =
                            platform.open_window_with_display_snapshot(handle, params, displays)?;
                        native.replace(Some(window.as_test().unwrap().clone()));
                        Ok(window)
                    },
                    |window, app| {
                        assert_eq!(Some(window.window_handle()), reserved.get());
                        if panic_in_root {
                            panic!("factory root panic");
                        }
                        window.remove_window(app);
                        app.new(|_| Empty)
                    },
                )
            })
        }));
        if panic_in_root {
            let panic = result.expect_err("the original root panic must propagate");
            assert_eq!(panic.downcast_ref::<&str>(), Some(&"factory root panic"));
        } else {
            assert_eq!(
                result
                    .unwrap()
                    .expect_err("a root-closed reservation must not commit")
                    .stage(),
                WindowOpenFailureStage::ClosedDuringBuild
            );
        }
        app.update(|app| assert!(app.windows().is_empty()));
        let original = reserved.get().unwrap();
        assert_eq!(calls.get(), 1);
        let native = native.into_inner().unwrap();
        assert!(!native.fire_close_required());
        assert!(!native.simulate_frame(RequestFrameOptions::default()));
        assert_eq!(terminal.borrow().as_slice(), &[original.window_id()]);
        assert_released_and_retry(&app, original);
        app.update(App::shutdown);
    }
}

struct RefusingRenderer {
    atlas: Arc<dyn PlatformAtlas>,
    refuse: Rc<Cell<bool>>,
    dropped: Rc<Cell<usize>>,
}

impl PlatformHeadlessRenderer for RefusingRenderer {
    fn render_scene(&mut self, _: &Scene, _: Size<DevicePixels>) -> anyhow::Result<()> {
        anyhow::ensure!(!self.refuse.get(), "headless initial presentation refused");
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        _: &Scene,
        _: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        anyhow::bail!("this lifecycle consumer does not render pixels")
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

impl Drop for RefusingRenderer {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

#[test]
fn public_initial_presentation_refusal_releases_original_window_and_renderer() {
    let refuse = Rc::new(Cell::new(true));
    let dropped = Rc::new(Cell::new(0));
    let platform = bench_platform(
        Some(Box::new({
            let refuse = refuse.clone();
            let dropped = dropped.clone();
            move || {
                Ok(Some(Box::new(RefusingRenderer {
                    atlas: Arc::new(HeadlessAtlas::default()),
                    refuse: refuse.clone(),
                    dropped: dropped.clone(),
                })))
            }
        })),
        Arc::new(NoopTextSystem::new()),
    );
    let app = headless_app(platform.clone());
    let native = RefCell::new(None);
    let reserved = Cell::new(None);
    let rendered = Rc::new(Cell::new(None));
    let terminal = Rc::new(RefCell::new(Vec::new()));
    let _terminal = app.update(|app| {
        let terminal = terminal.clone();
        app.on_window_native_terminal(move |_, id| terminal.borrow_mut().push(id))
    });
    let error = app
        .update(|app| {
            app.open_window_with_platform_window(
                options(),
                |handle, params, displays| {
                    reserved.set(Some(handle));
                    let mut window =
                        platform.open_window_with_display_snapshot(handle, params, displays)?;
                    native.replace(Some(window.as_test().unwrap().clone()));
                    Ok(window)
                },
                |_, app| {
                    app.new(|_| Counter {
                        value: 0,
                        rendered: rendered.clone(),
                    })
                },
            )
        })
        .expect_err("a refused hidden first submission must prevent publication");
    assert_eq!(
        error.stage(),
        WindowOpenFailureStage::BeforeVisibilityPresentation
    );
    assert_eq!(rendered.get(), Some(0));
    let original = reserved.get().unwrap();
    assert_eq!(dropped.get(), 1);
    let native = native.into_inner().unwrap();
    assert!(!native.fire_close_required());
    assert!(!native.simulate_frame(RequestFrameOptions::default()));
    assert_eq!(terminal.borrow().as_slice(), &[original.window_id()]);
    refuse.set(false);
    assert_released_and_retry(&app, original);
    assert_eq!(dropped.get(), 2);
    app.update(App::shutdown);
}

#[test]
fn public_owner_and_kind_preflight_refuse_before_invoking_the_factory() {
    use open_gpui::popup::{PopupNotSupportedError, PopupOptions};

    let app = headless_app(bench_platform(None, Arc::new(NoopTextSystem::new())));
    let parent = app
        .update(|app| app.open_window_detailed(options(), |_, app| app.new(|_| Empty)))
        .unwrap();
    let owner = app
        .update(|app| app.transient_window_owner(parent.into()))
        .unwrap();
    let factory_called = Cell::new(false);
    let root_called = Cell::new(false);
    let popup = WindowKind::AnchoredPopup(PopupOptions {
        parent: parent.into(),
        anchor_rect: Bounds::new(Default::default(), size(px(20.0), px(16.0))),
        anchor: Default::default(),
        gravity: Default::default(),
        constraint_adjustment: Default::default(),
        offset: Default::default(),
        grab: false,
    });
    let unsupported = app
        .update(|app| {
            app.open_window_with_platform_window(
                WindowOptions {
                    kind: popup,
                    ..options()
                },
                |_, _, _| {
                    factory_called.set(true);
                    anyhow::bail!("kind preflight must precede this factory")
                },
                |_, app| {
                    root_called.set(true);
                    app.new(|_| Empty)
                },
            )
        })
        .expect_err("the actual platform must reject an unsupported kind before attachment");
    assert_eq!(
        unsupported.stage(),
        WindowOpenFailureStage::NativeCreateOrMap
    );
    assert!(
        std::error::Error::source(&unsupported)
            .and_then(|source| source.downcast_ref::<PopupNotSupportedError>())
            .is_some()
    );
    assert!(!factory_called.get());
    assert!(!root_called.get());
    app.update(|app| assert_eq!(app.windows(), [parent.into()]));
    app.update(|app| app.update_window(parent.into(), |_, window, app| window.remove_window(app)))
        .unwrap();
    let stale_owner = app
        .update(|app| {
            app.open_window_with_platform_window(
                WindowOptions {
                    transient_for: Some(owner),
                    ..options()
                },
                |_, _, _| {
                    factory_called.set(true);
                    anyhow::bail!("owner preflight must precede this factory")
                },
                |_, app| {
                    root_called.set(true);
                    app.new(|_| Empty)
                },
            )
        })
        .expect_err("a closed owner's full identity must fail before attachment");
    assert_eq!(
        stale_owner.stage(),
        WindowOpenFailureStage::NativeCreateOrMap
    );
    assert!(
        stale_owner
            .to_string()
            .contains("closed or its generation is stale")
    );
    assert!(!factory_called.get());
    assert!(!root_called.get());
    assert_released_and_retry(&app, parent.into());
    app.update(App::shutdown);
}
