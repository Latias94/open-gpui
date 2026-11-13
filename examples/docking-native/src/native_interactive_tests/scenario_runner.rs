use super::*;

use open_gpui_docking::legacy::native_scenario_support::{
    NativeDockspaceOracle as ProjectedNativeDockspaceOracle, NativeDockspaceScenario,
    NativeDockspaceStep, NativeDockspaceTarget, NativeDockspaceZone,
    load_native_dockspace_scenario,
};

#[derive(Clone, Debug)]
pub(super) struct NativeDockspaceRunBinding {
    pub(super) scenario_id: String,
    pub(super) oracles: BTreeSet<NativeDockspaceManifestOracle>,
}

pub(super) struct NativeDockspaceWorkflow {
    fixture: NativeDockspaceScenario,
    surface: DockSurface,
    native_observation: NativeWindowTestObservation,
    lifecycle: Arc<Mutex<NativeLifecycleRecorder>>,
    windows: Vec<NativeDockspaceWindow>,
}

struct NativeDockspaceWindow {
    space: DockSpaceId,
    window: AnyWindowHandle,
    hwnd: HWND,
    bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy)]
struct NativeDockspaceWindowRef {
    window: AnyWindowHandle,
    hwnd: HWND,
}

#[derive(Clone, Debug)]
struct NativeDockspaceTargetGeometry {
    point: Point<Pixels>,
    active_selector: NativeSubmittedDebugSelector,
    guide_bounds: Bounds<Pixels>,
}

#[derive(Clone, Debug)]
enum NativeSubmittedDebugSelector {
    Exact(String),
    PrefixSuffix { prefix: String, suffix: String },
}

impl NativeSubmittedDebugSelector {
    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact(selector) => candidate == selector,
            Self::PrefixSuffix { prefix, suffix } => {
                candidate.starts_with(prefix) && candidate.ends_with(suffix)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeSubmittedDebugFrame {
    generation: u64,
    selector_bounds: Option<Bounds<Pixels>>,
    non_empty: bool,
}

#[derive(Debug)]
struct NativeDockspaceStepObservation {
    step_id: String,
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    item: DockItemId,
    preview_bounds: Bounds<Pixels>,
    revision_before: u64,
    revision_after: u64,
    preview_generation: u64,
    committed_generation: u64,
    focused_window: WindowId,
    lifecycle: Option<serde_json::Value>,
}

pub(super) fn build_native_dockspace_workflow_scenario(
    cx: &mut App,
    native_observation: NativeWindowTestObservation,
    binding: NativeDockspaceRunBinding,
) -> Result<NativeDockScenario> {
    let fixture = load_native_dockspace_scenario(&binding.scenario_id).with_context(|| {
        format!(
            "failed to load native dockspace fixture {}",
            binding.scenario_id
        )
    })?;
    let covered_oracles = BTreeSet::from([
        ProjectedNativeDockspaceOracle::Graph,
        ProjectedNativeDockspaceOracle::Layout,
        ProjectedNativeDockspaceOracle::Presentation,
        ProjectedNativeDockspaceOracle::FocusSelection,
        ProjectedNativeDockspaceOracle::ViewportRoute,
        ProjectedNativeDockspaceOracle::NativeLifetime,
    ]);
    let declared_oracles = binding
        .oracles
        .iter()
        .copied()
        .map(project_manifest_oracle)
        .collect::<BTreeSet<_>>();
    ensure!(
        declared_oracles == covered_oracles,
        "native workflow manifest oracles must exactly match runner coverage: declared={declared_oracles:?}, covered={covered_oracles:?}"
    );
    for required in &covered_oracles {
        ensure!(
            fixture.oracles().contains(required),
            "native workflow fixture {} is missing required oracle {required:?}",
            fixture.id()
        );
    }

    let mut builder = DockSurface::builder(fixture.primary_space().clone())
        .try_layout(fixture.layout())?
        .allow_floating(true)
        .allow_platform_viewports(true)
        .platform_focus_sets_dock_focus(true)
        .visual_style_resolver(dock_visual_style_resolver());
    for item in fixture.items() {
        builder = builder.panel(
            item.clone(),
            DockPanel::lazy(item.as_str().to_owned(), |cx| cx.new(|_| Empty).into()),
        );
    }
    let surface = builder
        .build(cx)
        .context("native dockspace workflow surface should validate")?;
    let display = cx
        .primary_display()
        .context("the native dockspace workflow requires a primary display")?;
    let bounds = native_dockspace_window_bounds(
        display.bounds(),
        fixture.primary_space(),
        fixture.spaces(),
    )?;

    let mut windows = Vec::with_capacity(fixture.spaces().len());
    for (space, window_bounds) in bounds {
        let window = if &space == fixture.primary_space() {
            match surface.open_primary_window(viewport_window_options(window_bounds), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => {
                    bail!("native dockspace primary viewport {space} failed to open: {outcome:?}")
                }
            }
        } else {
            match surface.viewports().open(
                space.clone(),
                viewport_window_options(window_bounds),
                cx,
            ) {
                DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
                outcome => bail!("native dockspace viewport {space} failed to open: {outcome:?}"),
            }
        };
        windows.push(NativeDockspaceWindow {
            space,
            window,
            hwnd: raw_hwnd(cx, window)?,
            bounds: window_bounds,
        });
    }
    let unique_hwnds = windows
        .iter()
        .map(|window| window.hwnd.0 as isize)
        .collect::<BTreeSet<_>>();
    ensure!(
        unique_hwnds.len() == fixture.spaces().len(),
        "native dockspace workflow must create one distinct HWND per space: {unique_hwnds:?}"
    );

    let first_step = fixture
        .steps()
        .first()
        .context("native dockspace workflow requires at least one operation")?;
    let source = native_window_for_space(&windows, first_step.source_space())?;
    let target = native_window_for_space(&windows, first_step.target().space())?;
    let initial_revision = surface.export_snapshot(cx).revision();
    let initial_window_count = cx.windows().len();
    let initial_owned_window_count = surface
        .viewports()
        .runtime_status(cx)
        .window_ownership
        .owned_window_count;
    let lifecycle = Arc::new(Mutex::new(NativeLifecycleRecorder::default()));
    let mut mouse_interceptors = Vec::with_capacity(windows.len());
    for native_window in &windows {
        let lifecycle_for_window = Arc::clone(&lifecycle);
        let window_id = native_window.window.window_id();
        let source = format!("WindowMouseEvent::{}", native_window.space);
        let interceptor = cx.update_window(native_window.window, move |_, window, _| {
            window.intercept_window_mouse_events(move |event, _, _| {
                let name = match event {
                    WindowMouseEvent::Move(_) => "window_mouse_move",
                    WindowMouseEvent::Down(_) => "window_mouse_down",
                    WindowMouseEvent::Up(_) => "window_mouse_up",
                    _ => return,
                };
                if let Ok(mut lifecycle) = lifecycle_for_window.lock() {
                    lifecycle.record_raw(
                        name,
                        Instant::now(),
                        NativeLifecycleTimingKind::ExactCallback,
                        source.as_str(),
                        None,
                        None,
                        None,
                        format!("real GPUI mouse interceptor callback for {window_id:?}"),
                    );
                }
            })
        })?;
        mouse_interceptors.push(interceptor);
    }
    let surface_changes = Rc::new(RefCell::new(Vec::new()));
    let surface_changes_for_subscription = Rc::clone(&surface_changes);
    let lifecycle_for_subscription = Arc::clone(&lifecycle);
    let surface_change_subscription = surface.subscribe_changes(cx, move |event, _| {
        surface_changes_for_subscription
            .borrow_mut()
            .push(event.clone());
        if let Ok(mut lifecycle) = lifecycle_for_subscription.lock() {
            let now = Instant::now();
            lifecycle.record_raw(
                "dock_surface_change",
                now,
                NativeLifecycleTimingKind::ExactCallback,
                "DockSurface::subscribe_changes",
                None,
                None,
                Some(event.revision()),
                "durable surface change callback",
            );
            if lifecycle.observes_new_revision(event.revision()) {
                lifecycle.record_observed_boundary(
                    NativeLifecycleBoundary::GraphReceipt,
                    now,
                    "DockSurface::subscribe_changes",
                    "durable graph receipt callback observed after the sample base revision",
                    None,
                    None,
                    Some(event.revision()),
                );
            }
        }
    });
    let live_undock_observation = surface.observe_live_undock_for_test(cx);
    let shutdown_observation = surface.observe_shutdown_for_test(cx);
    let renderer_payload_probe =
        cx.new(|_| NativeRendererPayloadProbe::new(native_renderer_payload_image()));
    let target_bounds = windows
        .iter()
        .find(|window| window.window == target.window)
        .map(|window| window.bounds)
        .context("native dockspace target window lost its planned bounds")?;

    Ok(NativeDockScenario {
        surface: surface.clone(),
        process_convergence_surface: None,
        renderer_payload_probe,
        source_window: source.window,
        target_window: target.window,
        source_hwnd: source.hwnd,
        target_hwnd: target.hwnd,
        target_bounds,
        mixed_dpi_displays: None,
        initial_revision,
        initial_window_count,
        initial_owned_window_count,
        trace: Arc::new(Mutex::new(NativeMouseTrace::default())),
        surface_changes,
        native_observation: native_observation.clone(),
        native_identities: Rc::new(RefCell::new(BTreeMap::new())),
        app_shutdown_probe: Rc::new(RefCell::new(NativeAppShutdownProbeState::default())),
        live_undock_observation,
        shutdown_observation,
        dockspace_workflow: Some(NativeDockspaceWorkflow {
            fixture,
            surface,
            native_observation,
            lifecycle,
            windows,
        }),
        workflow_milestones: Vec::new(),
        _mouse_interceptors: mouse_interceptors,
        _surface_change_subscription: surface_change_subscription,
    })
}

fn project_manifest_oracle(
    oracle: NativeDockspaceManifestOracle,
) -> ProjectedNativeDockspaceOracle {
    match oracle {
        NativeDockspaceManifestOracle::Graph => ProjectedNativeDockspaceOracle::Graph,
        NativeDockspaceManifestOracle::Layout => ProjectedNativeDockspaceOracle::Layout,
        NativeDockspaceManifestOracle::Presentation => ProjectedNativeDockspaceOracle::Presentation,
        NativeDockspaceManifestOracle::FocusSelection => {
            ProjectedNativeDockspaceOracle::FocusSelection
        }
        NativeDockspaceManifestOracle::ViewportRoute => {
            ProjectedNativeDockspaceOracle::ViewportRoute
        }
        NativeDockspaceManifestOracle::NativeLifetime => {
            ProjectedNativeDockspaceOracle::NativeLifetime
        }
        NativeDockspaceManifestOracle::Performance => ProjectedNativeDockspaceOracle::Performance,
    }
}

impl NativeDockspaceWorkflow {
    pub(super) async fn run(
        &mut self,
        cx: &mut AsyncApp,
        scenario_id: &str,
    ) -> Result<Vec<String>> {
        self.wait_until_ready(cx).await?;
        let (milestones, _) = self.run_cycle(cx, scenario_id, 0, false).await?;
        self.assert_cycle_terminal(cx)?;
        Ok(milestones)
    }

    pub(super) async fn run_latency_baseline(
        &mut self,
        cx: &mut AsyncApp,
        scenario_id: &str,
    ) -> Result<(Vec<String>, serde_json::Value)> {
        self.wait_until_ready(cx).await?;
        ensure!(
            self.windows.len() == 3,
            "native latency baseline requires exactly three real HWNDs"
        );
        let (milestones, samples) = self.run_cycle(cx, scenario_id, 0, true).await?;
        self.assert_cycle_terminal(cx)?;
        ensure!(
            !self
                .lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?
                .is_active(),
            "native lifecycle recorder remained active after the measured workflow"
        );
        let windows = cx.update(|app| {
            self.windows
                .iter()
                .map(|native_window| {
                    let facts = window_platform_facts(app, native_window.window)?;
                    let scale_factor = window_scale_factor(app, native_window.window)?;
                    let refresh_rate_hz = facts.display_id.and_then(|display_id| {
                        native_test_displays()
                            .into_iter()
                            .find(|display| display.display_id() == display_id)
                            .and_then(NativeTestDisplay::refresh_rate_hz)
                    });
                    Ok::<_, anyhow::Error>(serde_json::json!({
                        "space": native_window.space.as_str(),
                        "window_id": format!("{:?}", native_window.window.window_id()),
                        "hwnd": native_window.hwnd.0 as isize,
                        "display_id": facts.display_id.map(u64::from),
                        "refresh_rate_hz": refresh_rate_hz,
                        "scale_factor": scale_factor,
                        "logical_bounds": {
                            "x": native_window.bounds.origin.x.as_f32(),
                            "y": native_window.bounds.origin.y.as_f32(),
                            "width": native_window.bounds.size.width.as_f32(),
                            "height": native_window.bounds.size.height.as_f32(),
                        },
                    }))
                })
                .collect::<Result<Vec<_>>>()
        })?;
        let report = serde_json::json!({
            "schema_version": NATIVE_LIFECYCLE_SCHEMA_VERSION,
            "scenario_id": scenario_id,
            "fixture_id": self.fixture.id(),
            "adapter": "windows_real_three_hwnd",
            "clock": "std::time::Instant",
            "sample_count": samples.len(),
            "window_count": self.windows.len(),
            "distinct_hwnd_count": self.windows.iter().map(|window| window.hwnd.0 as isize).collect::<BTreeSet<_>>().len(),
            "boundary_names": NativeLifecycleBoundary::ALL.map(NativeLifecycleBoundary::as_str),
            "environment": {
                "platform": "windows",
                "input": "SendInput through NativeTestSystemPointerGuard",
                "input_assist": false,
                "windows": windows,
            },
            "samples": samples,
        });
        Ok((milestones, report))
    }

    async fn wait_until_ready(&self, cx: &mut AsyncApp) -> Result<()> {
        wait_until(cx, "all native dockspace HWNDs to present", |cx| {
            cx.update(|app| {
                self.windows.iter().try_fold(true, |ready, window| {
                    Ok::<_, anyhow::Error>(
                        ready
                            && window_has_non_empty_frame(app, window.window)?
                            && unsafe { IsWindowVisible(window.hwnd).as_bool() },
                    )
                })
            })
        })
        .await
    }

    async fn run_cycle(
        &mut self,
        cx: &mut AsyncApp,
        scenario_id: &str,
        cycle_index: usize,
        record_lifecycle: bool,
    ) -> Result<(Vec<String>, Vec<serde_json::Value>)> {
        let step_count = self.fixture.steps().len();
        ensure!(
            step_count > 0,
            "native dockspace workflow requires at least one fixture step"
        );
        let mut milestones = Vec::with_capacity(step_count * 2);
        let mut samples = Vec::with_capacity(if record_lifecycle { step_count } else { 0 });
        for index in 0..step_count {
            let step = self.fixture.steps()[index].clone();
            let observation = self
                .run_step(cx, cycle_index, index, &step, record_lifecycle)
                .await?;
            milestones.push(format!("dockspace.{}.preview-submitted", step.id()));
            milestones.push(format!("dockspace.{}.committed", step.id()));
            if let Some(lifecycle) = observation.lifecycle.clone() {
                samples.push(lifecycle);
            }
            log::info!(
                "scenario={scenario_id} workflow={} step={} source={} target={} item={} preview={:?} revision={}..{} generation={}..{} focus={:?}",
                self.fixture.id(),
                observation.step_id,
                observation.source_space,
                observation.target_space,
                observation.item,
                observation.preview_bounds,
                observation.revision_before,
                observation.revision_after,
                observation.preview_generation,
                observation.committed_generation,
                observation.focused_window,
            );
        }
        Ok((milestones, samples))
    }

    fn assert_cycle_terminal(&self, cx: &mut AsyncApp) -> Result<()> {
        let step_count = self.fixture.steps().len();
        let final_layout = cx.update(|app| self.surface().export_snapshot(app).layout().clone());
        let expected = self
            .fixture
            .expected_layout_after_step(step_count - 1)
            .context("native workflow lost its final headless checkpoint")?;
        ensure!(
            &final_layout == expected,
            "native workflow final layout diverged from the headless production checkpoint: actual={final_layout:?}, expected={expected:?}"
        );
        ensure!(
            self.windows
                .iter()
                .all(|window| unsafe { IsWindow(Some(window.hwnd)).as_bool() }),
            "all three native dockspace HWNDs must remain live through the complete workflow"
        );
        Ok(())
    }

    async fn run_step(
        &mut self,
        cx: &mut AsyncApp,
        cycle_index: usize,
        index: usize,
        step: &NativeDockspaceStep,
        record_lifecycle: bool,
    ) -> Result<NativeDockspaceStepObservation> {
        let source = self.window(step.source_space())?;
        let target = self.window(step.target().space())?;
        ensure!(
            source.hwnd != target.hwnd,
            "native workflow step {} must cross distinct HWNDs",
            step.id()
        );
        ensure_native_pointer_idle(step.id())?;
        raise_native_window(target.hwnd)?;
        let (source_tab_bounds, target_geometry, source_scale, target_scale) =
            cx.update(|app| {
                Ok::<_, anyhow::Error>((
                    unique_tab_bounds_for_item(
                        app,
                        source.window,
                        step.source_space(),
                        step.item(),
                    )?,
                    native_target_geometry(app, target.window, step.target())?,
                    window_scale_factor(app, source.window)?,
                    window_scale_factor(app, target.window)?,
                ))
            })?;
        let source_point =
            logical_client_point_to_screen(source.hwnd, source_tab_bounds.center(), source_scale)?;
        let activation_local = drag_activation_point(source_tab_bounds)?;
        let activation_point =
            logical_client_point_to_screen(source.hwnd, activation_local, source_scale)?;
        let target_point =
            logical_client_point_to_screen(target.hwnd, target_geometry.point, target_scale)?;
        for (label, point, hwnd) in [
            ("source", source_point, source.hwnd),
            ("activation", activation_point, source.hwnd),
            ("target", target_point, target.hwnd),
        ] {
            raise_native_window(hwnd)?;
            let observed = native_test_non_shell_root_window_at(point);
            ensure!(
                observed == Some(hwnd),
                "native workflow step {} {label} point is occluded: point={point:?}, expected={hwnd:?}, observed={observed:?}",
                step.id()
            );
        }

        let (revision_before, target_generation_before, release_before) = cx.update(|app| {
            Ok::<_, anyhow::Error>((
                self.surface().export_snapshot(app).revision(),
                submitted_debug_frame(app, target.window, &target_geometry.active_selector)?
                    .map(|frame| frame.generation),
                native_captured_release_observation_for_test(source.window.window_id(), app)
                    .map(|observation| (observation.generation(), observation.sequence())),
            ))
        })?;
        let native_event_cursor = self.native_observation().cursor();
        native_test_acquire_foreground_window(source.hwnd.0 as isize)?;
        let mut pointer_guard = NativeTestSystemPointerGuard::capture()?;
        inject_system_pointer(activation_point, NativeTestPointerAction::Move)?;
        wait_for_cursor(cx, activation_point, "native workflow source pre-route").await?;
        inject_system_pointer(source_point, NativeTestPointerAction::Move)?;
        wait_for_cursor(cx, source_point, "native workflow source tab").await?;
        pointer_guard.inject(source_point, NativeTestPointerAction::PrimaryDown)?;
        wait_until(cx, "native workflow source capture", |_| {
            Ok(unsafe { GetCapture() } == source.hwnd
                && native_primary_down_captured(
                    self.native_observation(),
                    source.window,
                    source_point,
                    native_event_cursor,
                ))
        })
        .await?;
        if record_lifecycle {
            let now = Instant::now();
            let mut lifecycle = self
                .lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?;
            lifecycle.begin_sample(
                NativeLifecycleSampleContext {
                    cycle_index,
                    step_index: index,
                    step_id: step.id().to_owned(),
                    source_space: step.source_space().as_str().to_owned(),
                    target_space: step.target().space().as_str().to_owned(),
                    item: step.item().as_str().to_owned(),
                    source_window: source.window.window_id(),
                    target_window: target.window.window_id(),
                    source_hwnd: source.hwnd.0 as isize,
                    target_hwnd: target.hwnd.0 as isize,
                    base_revision: revision_before,
                },
                now,
            );
            lifecycle.record_origin(
                NativeLifecycleOrigin::DragThresholdMove,
                now,
                "NativeTestSystemPointerGuard::inject",
                "drag-threshold move injection requested",
            );
        }
        inject_system_pointer(activation_point, NativeTestPointerAction::Move)?;
        wait_until(cx, "native workflow drag activation", |cx| {
            Ok(cx.update(|app| app.has_active_drag()) && unsafe { GetCapture() } == source.hwnd)
        })
        .await?;
        if record_lifecycle {
            self.lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?
                .record_estimated_boundary(
                NativeLifecycleBoundary::DragActivation,
                Instant::now(),
                "wait_until(App::has_active_drag)",
                "first post-input state sample that observed active drag; timestamp is a polling upper bound",
                None,
                None,
                None,
            );
        }

        raise_native_window(target.hwnd)?;
        let routed_hwnd = native_test_non_shell_root_window_at(target_point);
        ensure!(
            routed_hwnd == Some(target.hwnd),
            "native workflow step {} target point became occluded before the captured route: point={target_point:?}, expected={:?}, observed={routed_hwnd:?}",
            step.id(),
            target.hwnd
        );
        if record_lifecycle {
            self.lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?
                .record_origin(
                    NativeLifecycleOrigin::TargetRouteMove,
                    Instant::now(),
                    "native_test_inject_system_pointer",
                    "captured pointer move toward the accepted target was requested",
                );
        }
        inject_system_pointer(target_point, NativeTestPointerAction::Move)?;
        wait_for_cursor(cx, target_point, "native workflow target route").await?;
        let preview_wait = wait_until(cx, "native workflow preview submission", |cx| {
            let frame = cx.update(|app| {
                submitted_debug_frame(app, target.window, &target_geometry.active_selector)
            })?;
            Ok(frame.is_some_and(|frame| {
                frame.non_empty
                    && target_generation_before
                        .is_none_or(|generation| frame.generation > generation)
                    && frame.selector_bounds == Some(target_geometry.guide_bounds)
            }) && native_captured_move(
                self.native_observation(),
                source.window,
                target_point,
                native_event_cursor,
            ))
        })
        .await;
        if let Err(error) = preview_wait {
            let submitted = cx.update(|app| {
                submitted_dock_debug_diagnostic(app, target.window, step.target().space())
            })?;
            let captured_move = native_captured_move(
                self.native_observation(),
                source.window,
                target_point,
                native_event_cursor,
            );
            return Err(error).with_context(|| {
                format!(
                    "native workflow step {} expected active selector {:?} at {:?}; captured_move={captured_move}; {submitted}",
                    step.id(),
                    target_geometry.active_selector,
                    target_geometry.guide_bounds
                )
            });
        }
        let preview_frame = cx
            .update(|app| {
                submitted_debug_frame(app, target.window, &target_geometry.active_selector)
            })?
            .context("native workflow preview disappeared before release")?;
        ensure!(
            preview_frame.non_empty
                && preview_frame.selector_bounds == Some(target_geometry.guide_bounds),
            "native workflow preview frame drifted before release: expected={:?}, observed={preview_frame:?}",
            target_geometry.guide_bounds
        );
        let preview_bounds = preview_frame
            .selector_bounds
            .context("native workflow preview frame omitted its selector bounds")?;
        let preview_generation = preview_frame.generation;
        if record_lifecycle {
            self.lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?
                .record_estimated_boundary(
                NativeLifecycleBoundary::AcceptedPreviewSubmitted,
                Instant::now(),
                "Window::submitted_frame_diagnostics_for_test",
                "first observation of the exact accepted preview generation; timestamp is a polling upper bound",
                Some(preview_generation),
                None,
                None,
            );
        }

        if record_lifecycle {
            self.lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?
                .record_origin(
                    NativeLifecycleOrigin::PrimaryRelease,
                    Instant::now(),
                    "NativeTestSystemPointerGuard::inject",
                    "primary-button release injection requested at the accepted target point",
                );
        }
        pointer_guard.inject(target_point, NativeTestPointerAction::PrimaryUp)?;
        let expected_layout = self
            .fixture
            .expected_layout_after_step(index)
            .context("native workflow lost a headless checkpoint")?
            .clone();
        wait_until(cx, "native workflow topology commit", |cx| {
            let settled = cx.update(|app| {
                let snapshot = self.surface().export_snapshot(app);
                let submitted =
                    submitted_debug_frame(app, target.window, &target_geometry.active_selector)?;
                Ok::<_, anyhow::Error>(
                    snapshot.layout() == &expected_layout
                        && !app.has_active_drag()
                        && submitted.is_some_and(|frame| {
                            frame.generation > preview_generation
                                && frame.non_empty
                                && frame.selector_bounds.is_none()
                        }),
                )
            })?;
            Ok(settled && unsafe { GetCapture() } == HWND::default())
        })
        .await?;
        if record_lifecycle {
            let release = cx.update(|app| {
                native_captured_release_observation_for_test(source.window.window_id(), app)
            });
            if let Some(release) = release.filter(|observation| {
                Some((observation.generation(), observation.sequence())) != release_before
            }) {
                self.lifecycle
                    .lock()
                    .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?
                    .record_estimated_boundary(
                        NativeLifecycleBoundary::ReleaseLock,
                        Instant::now(),
                        "native_captured_release_observation_for_test",
                        "exact callback-scoped release identity observed after delivery; timestamp is a polling upper bound",
                        Some(release.generation()),
                        Some(release.sequence()),
                        None,
                    );
            }
        }
        let destination_semantics_observed_at = Instant::now();
        wait_until(cx, "native workflow target focus", |cx| {
            Ok(cx.update(|app| {
                app.focused_window().window() == Some(target.window)
                    && app.active_window() == Some(target.window)
            }))
        })
        .await?;
        let focus_observed_at = Instant::now();
        assert_native_capture_trace(
            self.native_observation(),
            source.window,
            target.window,
            target_point,
            native_event_cursor,
        )?;
        let (revision_after, committed_frame, focused_window) = cx.update(|app| {
            let graph = DockGraph::import_layout(self.surface().export_snapshot(app).layout())?;
            let (tabs, _) = graph
                .find_item_in_space(step.target().space(), step.item())
                .with_context(|| {
                    format!(
                        "native workflow step {} did not move {} into {}",
                        step.id(),
                        step.item(),
                        step.target().space()
                    )
                })?;
            let selected = match graph.node(tabs) {
                Some(open_gpui_docking::legacy::model::DockNode::Tabs { selected, .. }) => selected,
                node => bail!(
                    "native workflow target node is not tabs after step {}: {node:?}",
                    step.id()
                ),
            };
            ensure!(
                selected.as_ref() == Some(step.item()),
                "native workflow step {} did not select moved item {}: selected={selected:?}",
                step.id(),
                step.item()
            );
            Ok::<_, anyhow::Error>((
                self.surface().export_snapshot(app).revision(),
                submitted_debug_frame(app, target.window, &target_geometry.active_selector)?
                    .context("native workflow commit has no submitted frame")?,
                app.focused_window()
                    .window()
                    .context("native workflow target focus disappeared")?
                    .window_id(),
            ))
        })?;
        ensure!(
            revision_after > revision_before,
            "native workflow step {} did not publish a newer surface revision",
            step.id()
        );
        ensure!(
            committed_frame.generation > preview_generation
                && committed_frame.non_empty
                && committed_frame.selector_bounds.is_none(),
            "native workflow step {} lacks a newer submitted commit frame without its preview: preview={preview_generation}, committed={committed_frame:?}",
            step.id()
        );
        let lifecycle = if record_lifecycle {
            let mut lifecycle = self
                .lifecycle
                .lock()
                .map_err(|_| anyhow!("native lifecycle recorder lock was poisoned"))?;
            lifecycle.record_estimated_boundary(
                NativeLifecycleBoundary::DestinationSemanticsSubmitted,
                destination_semantics_observed_at,
                "Window::submitted_frame_diagnostics_for_test",
                "first observation of the post-commit target frame with durable layout semantics and no preview; timestamp is a polling upper bound",
                Some(committed_frame.generation),
                None,
                Some(revision_after),
            );
            lifecycle.record_estimated_boundary(
                NativeLifecycleBoundary::FocusSettled,
                focus_observed_at,
                "App::focused_window + App::active_window",
                "first observation that platform focus and GPUI active-window authority agreed on the destination; timestamp is a polling upper bound",
                Some(committed_frame.generation),
                None,
                Some(revision_after),
            );
            ensure!(
                lifecycle.has_observed_boundary(NativeLifecycleBoundary::GraphReceipt),
                "native workflow step {} completed without an exact graph receipt callback",
                step.id()
            );
            Some(
                lifecycle
                    .finish_sample()
                    .context("native lifecycle recorder had no active sample")?,
            )
        } else {
            None
        };

        Ok(NativeDockspaceStepObservation {
            step_id: step.id().to_owned(),
            source_space: step.source_space().clone(),
            target_space: step.target().space().clone(),
            item: step.item().clone(),
            preview_bounds,
            revision_before,
            revision_after,
            preview_generation,
            committed_generation: committed_frame.generation,
            focused_window,
            lifecycle,
        })
    }

    fn surface(&self) -> &DockSurface {
        &self.surface
    }

    fn native_observation(&self) -> &NativeWindowTestObservation {
        &self.native_observation
    }

    fn window(&self, space: &DockSpaceId) -> Result<NativeDockspaceWindowRef> {
        native_window_for_space(&self.windows, space)
    }
}

fn native_dockspace_window_bounds(
    display: Bounds<Pixels>,
    primary_space: &DockSpaceId,
    spaces: &[DockSpaceId],
) -> Result<Vec<(DockSpaceId, Bounds<Pixels>)>> {
    ensure!(
        spaces.len() == 3,
        "the complex native dockspace workflow currently requires exactly three spaces"
    );
    let gap = 24.0;
    let margin = 48.0;
    let width = ((display.size.width.as_f32() - gap - margin * 2.0) / 2.0).min(620.0);
    let height = ((display.size.height.as_f32() - gap - margin * 2.0) / 2.0).min(420.0);
    ensure!(
        width >= 360.0 && height >= 280.0,
        "the native dockspace display is too small for three non-overlapping HWNDs: {display:?}"
    );
    let grid_width = width * 2.0 + gap;
    let grid_height = height * 2.0 + gap;
    let left = display.origin.x + px((display.size.width.as_f32() - grid_width) / 2.0);
    let top = display.origin.y + px((display.size.height.as_f32() - grid_height) / 2.0);
    let slots = [
        Bounds::new(point(left, top), size(px(width), px(height))),
        Bounds::new(
            point(left + px(width + gap), top),
            size(px(width), px(height)),
        ),
        Bounds::new(
            point(left, top + px(height + gap)),
            size(px(width), px(height)),
        ),
    ];
    let mut ordered = Vec::with_capacity(3);
    ordered.push(primary_space.clone());
    ordered.extend(
        spaces
            .iter()
            .filter(|space| *space != primary_space)
            .cloned(),
    );
    Ok(ordered.into_iter().zip(slots).collect())
}

fn native_window_for_space(
    windows: &[NativeDockspaceWindow],
    space: &DockSpaceId,
) -> Result<NativeDockspaceWindowRef> {
    windows
        .iter()
        .find(|window| &window.space == space)
        .map(|window| NativeDockspaceWindowRef {
            window: window.window,
            hwnd: window.hwnd,
        })
        .with_context(|| format!("native dockspace workflow has no window for space {space}"))
}

fn unique_tab_bounds_for_item(
    app: &mut App,
    window: AnyWindowHandle,
    space: &DockSpaceId,
    item: &DockItemId,
) -> Result<Bounds<Pixels>> {
    let prefix = format!("dock:{space}:tabs:");
    let suffix = format!(":tab:{item}");
    unique_submitted_matching_debug_bounds(app, window, &prefix, &suffix)
}

fn unique_tabs_bounds_for_item(
    app: &mut App,
    window: AnyWindowHandle,
    space: &DockSpaceId,
    item: &DockItemId,
) -> Result<Bounds<Pixels>> {
    let prefix = format!("dock:{space}:tabs:");
    let suffix = format!(":tab:{item}");
    let submitted = app.update_window(window, |_, window, _| {
        window
            .submitted_frame_diagnostics_for_test()
            .map(|frame| frame.debug_bounds().to_vec())
    })?;
    let submitted = submitted.context("native target window has no submitted frame diagnostics")?;
    let matches = submitted
        .iter()
        .filter(|(selector, _)| selector.starts_with(&prefix) && selector.ends_with(&suffix))
        .collect::<Vec<_>>();
    let selector = match matches.as_slice() {
        [(selector, _)] => selector,
        [] => bail!("no native target tab matches {prefix}*{suffix}"),
        _ => bail!("ambiguous native target tabs match {prefix}*{suffix}: {matches:?}"),
    };
    let tabs_selector = selector
        .strip_suffix(&suffix)
        .context("native target tab selector lost its tabs suffix")?;
    submitted
        .iter()
        .find_map(|(candidate, bounds)| (candidate == tabs_selector).then_some(*bounds))
        .with_context(|| format!("native target tabs selector {tabs_selector} is missing"))
}

fn unique_submitted_matching_debug_bounds(
    app: &mut App,
    window: AnyWindowHandle,
    prefix: &str,
    suffix: &str,
) -> Result<Bounds<Pixels>> {
    let matches = app.update_window(window, |_, window, _| {
        window.submitted_frame_diagnostics_for_test().map(|frame| {
            frame
                .debug_bounds()
                .iter()
                .filter(|(selector, _)| selector.starts_with(prefix) && selector.ends_with(suffix))
                .cloned()
                .collect::<Vec<_>>()
        })
    })?;
    let matches = matches.context("native window has no submitted frame diagnostics")?;
    match matches.as_slice() {
        [(_, bounds)] => Ok(*bounds),
        [] => bail!("no submitted debug selector matches {prefix}*{suffix}"),
        _ => bail!("ambiguous submitted debug selectors match {prefix}*{suffix}: {matches:?}"),
    }
}

fn native_target_geometry(
    app: &mut App,
    window: AnyWindowHandle,
    target: &NativeDockspaceTarget,
) -> Result<NativeDockspaceTargetGeometry> {
    match target {
        NativeDockspaceTarget::Item { space, item, zone } => {
            let bounds = unique_tabs_bounds_for_item(app, window, space, item)?;
            let geometry = zone.target_geometry(bounds).with_context(|| {
                format!("production drop-guide geometry rejected {zone:?} in {bounds:?}")
            })?;
            Ok(NativeDockspaceTargetGeometry {
                point: geometry.target_point(),
                active_selector: NativeSubmittedDebugSelector::PrefixSuffix {
                    prefix: format!("dock:{space}:drop-guide-active:inner:"),
                    suffix: format!(":{zone:?}"),
                },
                guide_bounds: app.update_window(window, |_, window, _| {
                    window.pixel_snap_bounds(geometry.guide_bounds())
                })?,
            })
        }
        NativeDockspaceTarget::EmptySpace { space } => {
            let selector = format!("dock:{space}:empty");
            let bounds = app
                .update_window(window, |_, window, _| {
                    window
                        .submitted_frame_diagnostics_for_test()
                        .and_then(|frame| {
                            frame.debug_bounds().iter().find_map(|(candidate, bounds)| {
                                (candidate == &selector).then_some(*bounds)
                            })
                        })
                })?
                .with_context(|| {
                    format!("native empty target space {space} has no submitted bounds")
                })?;
            let geometry = NativeDockspaceZone::Center
                .target_geometry(bounds)
                .with_context(|| {
                    format!("production center drop-guide geometry rejected {bounds:?}")
                })?;
            Ok(NativeDockspaceTargetGeometry {
                point: geometry.target_point(),
                active_selector: NativeSubmittedDebugSelector::Exact(format!(
                    "dock:{space}:drop-guide-active:inner:Center"
                )),
                guide_bounds: app.update_window(window, |_, window, _| {
                    window.pixel_snap_bounds(geometry.guide_bounds())
                })?,
            })
        }
    }
}

fn drag_activation_point(bounds: Bounds<Pixels>) -> Result<Point<Pixels>> {
    let center = bounds.center();
    let inset = px(8.0);
    let right = point(
        (center.x + inset).min(bounds.origin.x + bounds.size.width - px(1.0)),
        center.y,
    );
    let left = point((center.x - inset).max(bounds.origin.x + px(1.0)), center.y);
    [right, left]
        .into_iter()
        .find(|candidate| *candidate != center && bounds.contains(candidate))
        .context("native source tab is too small to cross the GPUI drag threshold")
}

fn ensure_native_pointer_idle(step_id: &str) -> Result<()> {
    ensure!(
        unsafe { GetCapture() } == HWND::default(),
        "native workflow step {step_id} started with an unrelated capture owner"
    );
    if unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 != 0 {
        let _ = inject_primary_button_up_best_effort();
        bail!("native workflow step {step_id} started with the primary button pressed");
    }
    Ok(())
}

fn submitted_debug_frame(
    app: &mut App,
    window: AnyWindowHandle,
    selector: &NativeSubmittedDebugSelector,
) -> Result<Option<NativeSubmittedDebugFrame>> {
    app.update_window(window, |_, window, _| -> Result<_> {
        let Some(submitted) = window.submitted_frame_diagnostics_for_test() else {
            return Ok(None);
        };
        let mut selector_matches = submitted
            .debug_bounds()
            .iter()
            .filter_map(|(candidate, bounds)| selector.matches(candidate).then_some(*bounds));
        let selector_bounds = selector_matches.next();
        ensure!(
            selector_matches.next().is_none(),
            "submitted frame {} contains duplicate debug selector {selector:?}",
            submitted.generation()
        );
        let generation = submitted.generation();
        let facts = window.presentation_facts();
        let non_empty = facts.native_visible
            && facts.present_submitted_generation == Some(generation)
            && facts.non_empty_presented_generation == Some(generation)
            && facts.latest_present_attempt.is_some_and(|attempt| {
                attempt.outcome == PlatformWindowPresentOutcome::Submitted
                    && attempt.contained_valid_primitives
                    && attempt.generation == generation
            });
        Ok(Some(NativeSubmittedDebugFrame {
            generation,
            selector_bounds,
            non_empty,
        }))
    })?
}

fn submitted_dock_debug_diagnostic(
    app: &mut App,
    window: AnyWindowHandle,
    space: &DockSpaceId,
) -> Result<String> {
    app.update_window(window, |_, window, _| {
        let facts = window.presentation_facts();
        let prefix = format!("dock:{space}:drop-");
        let submitted = window.submitted_frame_diagnostics_for_test().map(|frame| {
            let selectors = frame
                .debug_bounds()
                .iter()
                .filter(|(selector, _)| selector.starts_with(&prefix))
                .cloned()
                .collect::<Vec<_>>();
            (frame.generation(), selectors)
        });
        format!("submitted={submitted:?}; presentation={facts:?}")
    })
}

fn native_primary_down_captured(
    observation: &NativeWindowTestObservation,
    source: AnyWindowHandle,
    point: POINT,
    cursor: NativeWindowTestObservationCursor,
) -> bool {
    observation.any_after(cursor, |event| {
        event.window().window_id() == source.window_id()
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::PrimaryButtonDown,
                    extra_info,
                    screen_point: Some(observed),
                    capture_before: NativeWindowTestCaptureOwner::None,
                    capture_after: NativeWindowTestCaptureOwner::Recipient,
                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                    ..
                } if extra_info == INPUT_CANARY as isize && observed == native_point(point)
            )
    })
}

fn native_captured_move(
    observation: &NativeWindowTestObservation,
    source: AnyWindowHandle,
    point: POINT,
    cursor: NativeWindowTestObservationCursor,
) -> bool {
    observation.any_after(cursor, |event| {
        event.window().window_id() == source.window_id()
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::MouseMove,
                    extra_info,
                    screen_point: Some(observed),
                    capture_before: NativeWindowTestCaptureOwner::Recipient,
                    capture_after: NativeWindowTestCaptureOwner::Recipient,
                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                    ..
                } if extra_info == INPUT_CANARY as isize && observed == native_point(point)
            )
    })
}

fn assert_native_capture_trace(
    observation: &NativeWindowTestObservation,
    source: AnyWindowHandle,
    target: AnyWindowHandle,
    release_point: POINT,
    cursor: NativeWindowTestObservationCursor,
) -> Result<()> {
    let events = observation.events_after(cursor);
    let source_id = source.window_id();
    let target_id = target.window_id();
    ensure!(
        !events.iter().any(|event| {
            event.window().window_id() == target_id
                && matches!(
                    event.kind(),
                    NativeWindowTestEventKind::WindowMessage {
                        message,
                        extra_info,
                        ..
                    } if is_native_mouse_message(message) && extra_info == INPUT_CANARY as isize
                )
        }),
        "captured native workflow input entered the target WndProc: {events:?}"
    );
    let source_mouse = events
        .iter()
        .filter(|event| event.window().window_id() == source_id)
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage { message, .. }
                    if is_native_mouse_message(message)
            )
        })
        .collect::<Vec<_>>();
    let down = source_mouse.iter().position(|event| {
        matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::PrimaryButtonDown,
                extra_info,
                capture_before: NativeWindowTestCaptureOwner::None,
                capture_after: NativeWindowTestCaptureOwner::Recipient,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if extra_info == INPUT_CANARY as isize
        )
    });
    let captured_move = source_mouse.iter().position(|event| {
        matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::MouseMove,
                extra_info,
                screen_point: Some(point),
                capture_before: NativeWindowTestCaptureOwner::Recipient,
                capture_after: NativeWindowTestCaptureOwner::Recipient,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if extra_info == INPUT_CANARY as isize && point == native_point(release_point)
        )
    });
    let up = source_mouse.iter().position(|event| {
        matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::PrimaryButtonUp,
                extra_info,
                screen_point: Some(point),
                capture_before: NativeWindowTestCaptureOwner::Recipient,
                capture_after: NativeWindowTestCaptureOwner::None,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if extra_info == INPUT_CANARY as isize && point == native_point(release_point)
        )
    });
    let (Some(down), Some(captured_move), Some(up)) = (down, captured_move, up) else {
        bail!(
            "source WndProc did not expose captured down/move/up at {release_point:?}: {source_mouse:?}"
        );
    };
    ensure!(
        down < captured_move && captured_move < up,
        "captured native workflow input was reordered: {source_mouse:?}"
    );
    Ok(())
}
