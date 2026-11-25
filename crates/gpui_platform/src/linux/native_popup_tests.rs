use super::{INITIAL_COLOR, Painted, is_probe_child};
use anyhow::Context as _;
use open_gpui::{
    AnyWindowHandle, AppContext as _, Application, AsyncApp, Bounds, GraphicalEnvironment, Pixels,
    PlatformWindowPresentOutcome, QuitMode, Size, WindowBounds, WindowCoordinateSpace,
    WindowHandle, WindowKind, WindowMutationDispatch, WindowMutationDomain,
    WindowMutationObservation, WindowMutationOutcome, WindowMutationRequest, WindowMutationSupport,
    WindowMutationTicket, WindowOpenFailureStage, WindowOptions, WindowPlacementRequest,
    WindowingModes, WindowingRequest, point,
    popup::{PopupAnchor, PopupConstraintAdjustment, PopupGravity, PopupOptions},
    px, size,
};
use raw_window_handle::RawWindowHandle;
use std::{
    cell::{Cell, RefCell},
    env,
    path::Path,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "maps native popups; requires GPUI_LINUX_TEST_WAYLAND_DISPLAY, live Wayland and WGPU"]
fn native_wayland_popup_lifecycle_roundtrip() {
    if !is_probe_child(
        "native_popup_tests::native_wayland_popup_lifecycle_roundtrip",
        "NATIVE_WAYLAND_POPUP_ROUNDTRIP_COMPLETE",
    ) {
        return;
    }

    let display = env::var_os("GPUI_LINUX_TEST_WAYLAND_DISPLAY")
        .expect("supply an absolute compositor socket in GPUI_LINUX_TEST_WAYLAND_DISPLAY");
    assert!(
        Path::new(&display).is_absolute(),
        "GPUI_LINUX_TEST_WAYLAND_DISPLAY must name an absolute compositor socket"
    );
    let environment = GraphicalEnvironment {
        wayland_display: Some(display),
        xdg_runtime_dir: GraphicalEnvironment::detect().xdg_runtime_dir,
        ..GraphicalEnvironment::default()
    };
    let platform = Rc::new(super::super::LinuxPlatform::new_with_startup(
        WindowingModes::all(),
        super::super::StartupEnvironment::owned(None, None),
    ));
    let weak_platform = Rc::downgrade(&platform);
    let completed = Rc::new(Cell::new(false));
    let outcome = Rc::new(RefCell::new(None));
    Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .with_windowing(WindowingRequest::Headless)
        .run({
            let completed = completed.clone();
            let outcome = outcome.clone();
            let platform = Rc::downgrade(&platform);
            move |cx| {
                assert_eq!(cx.compositor_name(), "headless");
                assert!(cx.windows().is_empty());
                cx.spawn(async move |cx| {
                    *outcome.borrow_mut() = Some(popup_roundtrip(cx, environment, platform).await);
                    completed.set(true);
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
    assert!(
        completed.get(),
        "the native loop exited before the popup lifecycle finished"
    );
    outcome
        .borrow_mut()
        .take()
        .unwrap()
        .expect("native Wayland popup lifecycle");
    drop(platform);
    assert!(
        weak_platform.upgrade().is_none(),
        "the closed popup runtime retained its platform"
    );
    println!(
        "NATIVE_WAYLAND_POPUP_UNEXECUTED grab=true input_injection input_region_pass_through layer_shell"
    );
    println!("NATIVE_WAYLAND_POPUP_ROUNDTRIP_COMPLETE");
}

fn popup_options(parent: AnyWindowHandle) -> PopupOptions {
    PopupOptions {
        parent,
        anchor_rect: Bounds::new(point(px(12.0), px(16.0)), size(px(80.0), px(24.0))),
        anchor: PopupAnchor::BottomLeft,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment: PopupConstraintAdjustment::SLIDE_X
            | PopupConstraintAdjustment::SLIDE_Y
            | PopupConstraintAdjustment::FLIP_X
            | PopupConstraintAdjustment::FLIP_Y
            | PopupConstraintAdjustment::RESIZE_X
            | PopupConstraintAdjustment::RESIZE_Y,
        offset: point(px(2.0), px(4.0)),
        grab: false,
    }
}

async fn popup_roundtrip(
    cx: &mut AsyncApp,
    environment: GraphicalEnvironment,
    platform: Weak<super::super::LinuxPlatform>,
) -> anyhow::Result<()> {
    let retained = cx.new(|_| 41_usize);
    let retained_id = retained.entity_id();
    let text_system = cx.update(|cx| cx.text_system().clone());
    let render_count = Rc::new(Cell::new(0));
    cx.update(|cx| cx.request_windowing(WindowingRequest::Windowed(environment.clone())))
        .await
        .context("attach the real Wayland compositor for the popup lifecycle")?;
    cx.update(|cx| {
        assert_eq!(cx.compositor_name(), "Wayland");
        assert!(!cx.displays().is_empty());
        assert!(Arc::ptr_eq(&text_system, cx.text_system()));
        let connected = cx.graphical_environment().unwrap();
        assert_eq!(connected.wayland_display, environment.wayland_display);
        assert_eq!(connected.xdg_runtime_dir, environment.xdg_runtime_dir);
        assert!(connected.x11_display.is_none());
        assert!(connected.activation_token.is_none());
    });
    retained.update(cx, |value, _| *value += 1);

    let hidden = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                show: false,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(30.0), px(30.0)),
                    size(px(160.0), px(100.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
        )
    })?;
    let hidden_id = hidden.window_id();
    let root_built = Cell::new(false);
    let refusal = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                kind: WindowKind::AnchoredPopup(popup_options(hidden.into())),
                focus_on_appearing: false,
                show: true,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(120.0), px(80.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| {
                root_built.set(true);
                cx.new(|cx| Painted::new(render_count.clone(), window, cx))
            },
        )
    });
    let error =
        refusal.expect_err("an unmapped parent must refuse a native popup before allocation");
    assert!(
        format!("{error:#}").contains("popup parent has not completed native mapping"),
        "unexpected hidden-parent refusal: {error:#}"
    );
    assert!(!root_built.get());
    cx.update(|cx| assert_eq!(cx.windows(), [hidden.into()]));
    cx.update_window(hidden.into(), |_, window, cx| {
        assert_eq!(window.window_handle().window_id(), hidden_id);
        assert!(matches!(
            raw_window_handle::HasWindowHandle::window_handle(window)?.as_raw(),
            RawWindowHandle::Wayland(_)
        ));
        assert!(window.gpu_specs().is_some());
        let clear = window.draw(cx);
        clear.clear(cx);
        assert!(
            window
                .presentation_facts()
                .frame_accepted_generation
                .is_some()
        );
        assert!(!window.presentation_facts().native_visible);
        Ok::<_, anyhow::Error>(())
    })??;
    cx.update(|cx| {
        hidden.update(cx, |view, _, _| {
            assert_eq!(view.version, 0);
            assert_eq!(view.paint.get().color, INITIAL_COLOR);
        })
    })?;
    println!("NATIVE_WAYLAND_POPUP_HIDDEN_PARENT_REFUSED parent={hidden_id:?} root_built=false");
    cx.update(|cx| hidden.update(cx, |_, window, cx| window.remove_window(cx)))?;
    await_no_windows(cx).await?;

    let parent = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                show: true,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(30.0), px(30.0)),
                    size(px(320.0), px(220.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
        )
    })?;
    assert_ne!(parent.window_id(), hidden_id);
    await_submitted(cx, parent, 0, None).await?;
    let popup = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                kind: WindowKind::AnchoredPopup(popup_options(parent.into())),
                focus_on_appearing: false,
                show: true,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(160.0), px(100.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
        )
    })?;
    await_submitted(cx, popup, 0, None).await?;
    println!(
        "NATIVE_WAYLAND_POPUP_CREATED window={:?} parent={:?} requested_focus_on_appearing=false effective_focus_on_appearing=false grab=false show=true activation_policy=default",
        popup.window_id(),
        parent.window_id()
    );
    let nested = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                kind: WindowKind::AnchoredPopup(popup_options(popup.into())),
                show: true,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(80.0), px(56.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
        )
    })?;
    await_submitted(cx, nested, 0, None).await?;
    println!(
        "NATIVE_WAYLAND_POPUP_CREATED window={:?} parent={:?} requested_focus_on_appearing=true effective_focus_on_appearing=false grab=false show=true activation_policy=default",
        nested.window_id(),
        popup.window_id()
    );
    let live: [AnyWindowHandle; 3] = [parent.into(), popup.into(), nested.into()];
    cx.update(|cx| {
        assert_eq!(cx.windows().len(), live.len());
        for handle in live {
            assert!(cx.windows().contains(&handle));
        }
        for handle in [popup, nested] {
            handle
                .update(cx, |_, window, _| {
                    assert_eq!(window.window_handle().window_id(), handle.window_id());
                    assert!(window.creation_facts().show);
                    assert!(!window.creation_facts().focus_on_appearing);
                    assert!(!window.platform_facts().accepts_activation);
                    assert!(!window.platform_facts().focus_on_click);
                    assert_eq!(
                        window.window_capabilities().mutations.size,
                        WindowMutationSupport::Live
                    );
                })
                .expect("both passive popup roles must remain live");
        }
    });

    let popup_size = cx.update(|cx| popup.update(cx, |_, window, _| window.bounds().size))?;
    for (label, region) in [
        ("none", None),
        ("empty", Some(Vec::new())),
        (
            "full",
            Some(vec![Bounds::new(Default::default(), popup_size)]),
        ),
    ] {
        let previous_generation = cx.update(|cx| {
            popup.update(cx, |_, window, _| {
                let previous = window.rendered_frame_revision();
                window.set_input_region(region.as_deref());
                window.refresh();
                previous
            })
        })?;
        let generation = await_submitted(cx, popup, previous_generation, None).await?;
        println!(
            "NATIVE_WAYLAND_POPUP_INPUT_REGION_REQUEST_ONLY window={:?} region={label} subsequent_submitted_generation={generation}",
            popup.window_id()
        );
    }

    let mut resize_receipts: Vec<(WindowMutationTicket, WindowMutationObservation)> = Vec::new();
    for requested_size in [size(px(192.0), px(120.0)), size(px(224.0), px(144.0))] {
        let (previous_generation, dispatch) = cx.update(|cx| {
            popup.update(cx, |_, window, _| {
                (
                    window.rendered_frame_revision(),
                    window.resize(requested_size),
                )
            })
        })?;
        let WindowMutationDispatch::Queued(ticket) = dispatch else {
            anyhow::bail!("native popup resize must queue an observed request: {dispatch:?}");
        };
        assert_eq!(ticket.domain(), WindowMutationDomain::Placement);
        assert_eq!(
            ticket.request(),
            WindowMutationRequest::Placement(WindowPlacementRequest {
                size: Some(requested_size),
                ..WindowPlacementRequest::new()
            })
        );
        if let Some((previous, _)) = resize_receipts.last() {
            assert!(ticket.generation() > previous.generation());
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let observation = loop {
            if let Some(observation) = ticket.observation() {
                break observation;
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "popup {:?} never observed resize generation {}: {ticket:?}",
                popup.window_id(),
                ticket.generation()
            );
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        };
        assert_eq!(observation.domain, ticket.domain());
        assert_eq!(observation.generation, ticket.generation());
        assert_eq!(observation.request, ticket.request());
        assert_eq!(observation.outcome, WindowMutationOutcome::Exact);
        assert_eq!(
            observation.facts.coordinate_space,
            WindowCoordinateSpace::WindowLocal
        );
        assert_eq!(observation.facts.bounds.size, requested_size);
        assert_eq!(observation.facts.content_size, requested_size);
        let generation =
            await_submitted(cx, popup, previous_generation, Some(requested_size)).await?;
        assert_eq!(ticket.observation(), Some(observation.clone()));
        println!(
            "NATIVE_WAYLAND_POPUP_RESIZE_OBSERVED window={:?} mutation_generation={} submitted_generation={generation} receipt={observation:?}",
            popup.window_id(),
            ticket.generation()
        );
        resize_receipts.push((ticket, observation));
    }

    let dismissed = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                kind: WindowKind::AnchoredPopup(popup_options(parent.into())),
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(96.0), px(64.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
        )
    })?;
    await_submitted(cx, dismissed, 0, None).await?;
    cx.update(|cx| {
        dismissed.update(cx, |_, window, cx| {
            assert!(window.quiesce_interaction(cx));
        })?;
        assert!(cx.windows().contains(&dismissed.into()));
        let native = {
            let platform = platform
                .upgrade()
                .expect("the native platform must remain live");
            let connection = platform.connection.borrow();
            let crate::linux::display_connection::DisplayConnection::Wayland(connection) =
                &*connection
            else {
                panic!("the dismissal control requires the current Wayland connection");
            };
            connection
                .window_for_test(dismissed.into())
                .expect("the quiesced popup must retain its exact native surface")
        };
        assert!(
            native.handle_popup_event(
                wayland_protocols::xdg::shell::client::xdg_popup::Event::PopupDone,
            ),
            "compositor dismissal must survive interaction quiescence"
        );
        native.require_close();
        assert!(cx.windows().contains(&dismissed.into()));
        Ok::<_, anyhow::Error>(())
    })?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while cx.update(|cx| cx.windows().contains(&dismissed.into())) {
        anyhow::ensure!(
            Instant::now() < deadline,
            "the quiesced popup did not close"
        );
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
    }
    println!(
        "NATIVE_WAYLAND_POPUP_SIMULATED_DISMISSAL_AFTER_QUIESCENCE window={:?}",
        dismissed.window_id()
    );

    cx.update(|cx| {
        assert_eq!(cx.windows().len(), live.len());
        for handle in live {
            assert!(cx.windows().contains(&handle));
            handle
                .update(cx, |_, window, _| {
                    assert_eq!(window.window_handle().window_id(), handle.window_id());
                    assert!(window.presentation_facts().native_visible);
                })
                .expect("the complete native popup tree must be live before closing its parent");
        }
        let failed = cx.open_window_detailed(
            WindowOptions {
                kind: WindowKind::AnchoredPopup(popup_options(parent.into())),
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(96.0), px(64.0)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| {
                parent
                    .update(cx, |_, window, cx| window.remove_window(cx))
                    .expect("the original parent must close during its new child's builder");
                cx.new(|cx| Painted::new(render_count.clone(), window, cx))
            },
        );
        let error = failed.expect_err("required-close during build must refuse child publication");
        assert_eq!(error.stage(), WindowOpenFailureStage::ClosedDuringBuild);
        println!("NATIVE_WAYLAND_POPUP_PARENT_CLOSE_DURING_BUILD_REFUSED");
    });
    await_no_windows(cx).await?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match cx
            .update(|cx| cx.request_windowing(WindowingRequest::Headless))
            .await
        {
            Ok(()) => break,
            Err(error) => {
                anyhow::ensure!(
                    format!("{error:#}").contains("retiring"),
                    "unexpected detach refusal after the popup tree closed: {error:#}"
                );
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "popup native resource retirement never finished: {error:#}"
                );
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
            }
        }
    }
    cx.update(|cx| {
        assert_eq!(cx.compositor_name(), "headless");
        assert!(cx.windows().is_empty());
        assert!(cx.graphical_environment().is_none());
        assert!(cx.displays().is_empty());
        assert!(Arc::ptr_eq(&text_system, cx.text_system()));
        for handle in live {
            assert!(handle.update(cx, |_, _, _| ()).is_err());
        }
    });
    for (ticket, observation) in resize_receipts {
        assert_eq!(ticket.observation(), Some(observation));
    }
    assert_eq!(retained.entity_id(), retained_id);
    assert_eq!(retained.read_with(cx, |value, _| *value), 42);
    retained.update(cx, |value, _| *value += 1);
    assert_eq!(retained.read_with(cx, |value, _| *value), 43);
    assert_eq!(cx.background_executor().spawn(async { 42 }).await, 42);
    let foreground = Rc::new(Cell::new(false));
    cx.spawn({
        let foreground = foreground.clone();
        async move |_| foreground.set(true)
    })
    .await;
    assert!(foreground.get());
    assert!(render_count.get() >= 4);
    println!("NATIVE_WAYLAND_POPUP_TREE_RETIRED windows={live:?} model={retained_id:?}");
    Ok(())
}

async fn await_submitted(
    cx: &mut AsyncApp,
    handle: WindowHandle<Painted>,
    after_generation: u64,
    expected_size: Option<Size<Pixels>>,
) -> anyhow::Result<u64> {
    cx.update(|cx| {
        handle.update(cx, |_, window, _| {
            assert_eq!(window.window_handle().window_id(), handle.window_id());
            anyhow::ensure!(
                matches!(
                    raw_window_handle::HasWindowHandle::window_handle(window)?.as_raw(),
                    RawWindowHandle::Wayland(_)
                ),
                "the popup trajectory requires real Wayland surfaces"
            );
            let gpu = window
                .gpu_specs()
                .context("the native surface must expose its actual GPU")?;
            if after_generation == 0 {
                println!(
                    "NATIVE_WAYLAND_POPUP_GPU window={:?} gpu={gpu:?}",
                    handle.window_id()
                );
            }
            Ok::<_, anyhow::Error>(())
        })
    })??;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (stamp, facts, bounds, viewport, submitted_bounds) = cx.update(|cx| {
            handle.update(cx, |view, window, _| {
                let stamp = view.paint.get();
                let submitted_bounds = window
                    .submitted_frame_diagnostics_for_test()
                    .filter(|diagnostics| diagnostics.generation() == stamp.generation)
                    .and_then(|diagnostics| {
                        diagnostics
                            .debug_bounds()
                            .iter()
                            .find(|(selector, _)| selector == "native-windowing-0-315ea8")
                            .map(|(_, bounds)| *bounds)
                    });
                (
                    stamp,
                    window.presentation_facts(),
                    window.bounds(),
                    window.viewport_size(),
                    submitted_bounds,
                )
            })
        })?;
        if stamp.generation > after_generation
            && stamp.version == 0
            && stamp.color == INITIAL_COLOR
            && bounds.size == viewport
            && viewport.width > px(0.0)
            && viewport.height > px(0.0)
            && expected_size.is_none_or(|expected| bounds.size == expected)
            && submitted_bounds.is_some_and(|submitted| submitted.size == viewport)
            && facts.native_visible
            && facts.frame_accepted_generation == Some(stamp.generation)
            && facts.non_empty_presented_generation == Some(stamp.generation)
            && facts.present_submitted_generation == Some(stamp.generation)
            && facts.latest_present_attempt.is_some_and(|attempt| {
                attempt.generation == stamp.generation
                    && attempt.contained_valid_primitives
                    && attempt.outcome == PlatformWindowPresentOutcome::Submitted
            })
        {
            println!(
                "NATIVE_WAYLAND_POPUP_NONEMPTY_SUBMITTED window={:?} generation={} size={viewport:?}",
                handle.window_id(),
                stamp.generation
            );
            return Ok(stamp.generation);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "Wayland window {:?} never submitted a matching nonempty frame after generation {after_generation}: paint={stamp:?}, facts={facts:?}, bounds={bounds:?}, viewport={viewport:?}, submitted_bounds={submitted_bounds:?}, expected_size={expected_size:?}",
            handle.window_id()
        );
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
    }
}

async fn await_no_windows(cx: &mut AsyncApp) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = cx.update(|cx| cx.windows());
        if remaining.is_empty() {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "native popup closure left logical windows open: {remaining:?}"
        );
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
    }
}
