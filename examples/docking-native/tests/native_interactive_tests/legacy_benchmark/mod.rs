//! Optional physical latency comparator for the legacy docking example.

use super::*;
use crate::legacy_style::dock_visual_style_resolver;
use futures::FutureExt as _;
use open_gpui::{Application, Point, QuitMode, Subscription, WindowId, WindowMouseEvent, px};
use open_gpui_docking::legacy::{
    DockItemId, DockPanel, DockSpaceId, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfaceViewportOpenOutcome, DockSurfaceWindowSessionPhase,
};
use open_gpui_windows::{
    NativeTestPointerAction, NativeTestSystemPointerGuard, WindowsPlatform,
    begin_native_window_test_observation, native_test_acquire_foreground_window,
    native_test_logical_client_point_to_screen as logical_client_point_to_screen,
};
use std::{
    cell::RefCell,
    fs,
    panic::{AssertUnwindSafe, set_hook, take_hook},
    rc::Rc,
    sync::{Arc, Mutex},
};
use windows::Win32::UI::WindowsAndMessaging::IsWindow;

mod scenario_runner;

const DRAG_LATENCY_OUTPUT_ENV: &str = "OPEN_GPUI_DOCK_DRAG_LATENCY_OUTPUT";
const DRAG_LATENCY_BENCHMARK_ID: &str = "benchmark.docking.tab-drag-input-to-submit";
const DRAG_LATENCY_BENCHMARK_TEST: &str =
    "native_interactive_tests::native_interactive_tab_drag_input_to_submit_latency_benchmark";
const NATIVE_LIFECYCLE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum NativeLifecycleBoundary {
    DragActivation,
    AcceptedPreviewSubmitted,
    ReleaseLock,
    GraphReceipt,
    DestinationSemanticsSubmitted,
    FocusSettled,
}

impl NativeLifecycleBoundary {
    const ALL: [Self; 6] = [
        Self::DragActivation,
        Self::AcceptedPreviewSubmitted,
        Self::ReleaseLock,
        Self::GraphReceipt,
        Self::DestinationSemanticsSubmitted,
        Self::FocusSettled,
    ];

    const fn as_str(self) -> &'static str {
        match self {
            Self::DragActivation => "drag_activation",
            Self::AcceptedPreviewSubmitted => "accepted_preview_submitted",
            Self::ReleaseLock => "release_lock",
            Self::GraphReceipt => "graph_receipt",
            Self::DestinationSemanticsSubmitted => "destination_semantics_submitted",
            Self::FocusSettled => "focus_settled",
        }
    }

    const fn origin(self) -> NativeLifecycleOrigin {
        match self {
            Self::DragActivation => NativeLifecycleOrigin::DragThresholdMove,
            Self::AcceptedPreviewSubmitted => NativeLifecycleOrigin::TargetRouteMove,
            Self::ReleaseLock
            | Self::GraphReceipt
            | Self::DestinationSemanticsSubmitted
            | Self::FocusSettled => NativeLifecycleOrigin::PrimaryRelease,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum NativeLifecycleOrigin {
    DragThresholdMove,
    TargetRouteMove,
    PrimaryRelease,
}

impl NativeLifecycleOrigin {
    const ALL: [Self; 3] = [
        Self::DragThresholdMove,
        Self::TargetRouteMove,
        Self::PrimaryRelease,
    ];

    const fn as_str(self) -> &'static str {
        match self {
            Self::DragThresholdMove => "drag_threshold_move",
            Self::TargetRouteMove => "target_route_move",
            Self::PrimaryRelease => "primary_release",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeLifecycleBoundaryStatus {
    Observed,
    Estimated,
    Unavailable,
}

impl NativeLifecycleBoundaryStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Estimated => "estimated",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum NativeLifecycleTimingKind {
    ExactCallback,
    InputInjection,
    ObservationUpperBound,
    Unavailable,
}

impl NativeLifecycleTimingKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExactCallback => "exact_callback",
            Self::InputInjection => "input_injection",
            Self::ObservationUpperBound => "observation_upper_bound",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug)]
struct NativeLifecycleSampleContext {
    cycle_index: usize,
    step_index: usize,
    step_id: String,
    source_space: String,
    target_space: String,
    item: String,
    source_window: WindowId,
    target_window: WindowId,
    source_hwnd: isize,
    target_hwnd: isize,
    base_revision: u64,
}

#[derive(Clone, Debug)]
struct NativeLifecycleOriginObservation {
    at: Instant,
    source: String,
    detail: String,
}

#[derive(Clone, Debug)]
struct NativeLifecycleBoundaryObservation {
    status: NativeLifecycleBoundaryStatus,
    timing: NativeLifecycleTimingKind,
    origin: NativeLifecycleOrigin,
    elapsed_ns: Option<u64>,
    source: String,
    detail: String,
    generation: Option<u64>,
    sequence: Option<u64>,
    revision: Option<u64>,
}

#[derive(Clone, Debug)]
struct NativeLifecycleRawEvent {
    name: String,
    timeline_elapsed_ns: u64,
    timing: NativeLifecycleTimingKind,
    source: String,
    generation: Option<u64>,
    sequence: Option<u64>,
    revision: Option<u64>,
    detail: String,
}

#[derive(Debug, Default)]
struct NativeLifecycleRecorder {
    started_at: Option<Instant>,
    context: Option<NativeLifecycleSampleContext>,
    origins: BTreeMap<NativeLifecycleOrigin, NativeLifecycleOriginObservation>,
    boundaries: BTreeMap<NativeLifecycleBoundary, NativeLifecycleBoundaryObservation>,
    raw_events: Vec<NativeLifecycleRawEvent>,
}

impl NativeLifecycleRecorder {
    fn begin_sample(&mut self, context: NativeLifecycleSampleContext, now: Instant) {
        self.started_at = Some(now);
        self.context = Some(context);
        self.origins.clear();
        self.boundaries.clear();
        self.raw_events.clear();
    }

    fn is_active(&self) -> bool {
        self.started_at.is_some()
    }

    fn observes_new_revision(&self, revision: u64) -> bool {
        self.context
            .as_ref()
            .is_some_and(|context| revision > context.base_revision)
    }

    fn elapsed_ns(&self, now: Instant) -> Option<u64> {
        self.started_at.map(|started_at| {
            now.saturating_duration_since(started_at)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64
        })
    }

    fn record_raw(
        &mut self,
        name: impl Into<String>,
        now: Instant,
        timing: NativeLifecycleTimingKind,
        source: impl Into<String>,
        generation: Option<u64>,
        sequence: Option<u64>,
        revision: Option<u64>,
        detail: impl Into<String>,
    ) {
        let Some(timeline_elapsed_ns) = self.elapsed_ns(now) else {
            return;
        };
        self.raw_events.push(NativeLifecycleRawEvent {
            name: name.into(),
            timeline_elapsed_ns,
            timing,
            source: source.into(),
            generation,
            sequence,
            revision,
            detail: detail.into(),
        });
    }

    fn record_origin(
        &mut self,
        origin: NativeLifecycleOrigin,
        now: Instant,
        source: impl Into<String>,
        detail: impl Into<String>,
    ) {
        if !self.is_active() {
            return;
        }
        let source = source.into();
        let detail = detail.into();
        self.origins
            .entry(origin)
            .or_insert_with(|| NativeLifecycleOriginObservation {
                at: now,
                source: source.clone(),
                detail: detail.clone(),
            });
        self.record_raw(
            format!("{}_injected", origin.as_str()),
            now,
            NativeLifecycleTimingKind::InputInjection,
            source,
            None,
            None,
            None,
            detail,
        );
    }

    fn record_observed_boundary(
        &mut self,
        boundary: NativeLifecycleBoundary,
        now: Instant,
        source: impl Into<String>,
        detail: impl Into<String>,
        generation: Option<u64>,
        sequence: Option<u64>,
        revision: Option<u64>,
    ) {
        self.record_boundary(
            boundary,
            NativeLifecycleBoundaryStatus::Observed,
            NativeLifecycleTimingKind::ExactCallback,
            now,
            source,
            detail,
            generation,
            sequence,
            revision,
        );
    }

    fn record_estimated_boundary(
        &mut self,
        boundary: NativeLifecycleBoundary,
        now: Instant,
        source: impl Into<String>,
        detail: impl Into<String>,
        generation: Option<u64>,
        sequence: Option<u64>,
        revision: Option<u64>,
    ) {
        self.record_boundary(
            boundary,
            NativeLifecycleBoundaryStatus::Estimated,
            NativeLifecycleTimingKind::ObservationUpperBound,
            now,
            source,
            detail,
            generation,
            sequence,
            revision,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn record_boundary(
        &mut self,
        boundary: NativeLifecycleBoundary,
        status: NativeLifecycleBoundaryStatus,
        timing: NativeLifecycleTimingKind,
        now: Instant,
        source: impl Into<String>,
        detail: impl Into<String>,
        generation: Option<u64>,
        sequence: Option<u64>,
        revision: Option<u64>,
    ) {
        let origin = boundary.origin();
        let Some(origin_observation) = self.origins.get(&origin) else {
            return;
        };
        let source = source.into();
        let detail = detail.into();
        let elapsed_ns = now
            .saturating_duration_since(origin_observation.at)
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        self.record_raw(
            format!("{}_{}", boundary.as_str(), status.as_str()),
            now,
            timing,
            source.as_str(),
            generation,
            sequence,
            revision,
            detail.as_str(),
        );
        self.boundaries
            .entry(boundary)
            .or_insert_with(|| NativeLifecycleBoundaryObservation {
                status,
                timing,
                origin,
                elapsed_ns: Some(elapsed_ns),
                source,
                detail,
                generation,
                sequence,
                revision,
            });
    }

    fn has_observed_boundary(&self, boundary: NativeLifecycleBoundary) -> bool {
        self.boundaries.get(&boundary).is_some_and(|observation| {
            observation.status == NativeLifecycleBoundaryStatus::Observed
        })
    }

    fn finish_sample(&mut self) -> Option<serde_json::Value> {
        let context = self.context.take()?;
        let started_at = self.started_at.take()?;
        for boundary in NativeLifecycleBoundary::ALL {
            self.boundaries
                .entry(boundary)
                .or_insert_with(|| NativeLifecycleBoundaryObservation {
                    status: NativeLifecycleBoundaryStatus::Unavailable,
                    timing: NativeLifecycleTimingKind::Unavailable,
                    origin: boundary.origin(),
                    elapsed_ns: None,
                    source: "native-test-recorder".to_owned(),
                    detail:
                        "the workflow completed without a timestamped observation for this boundary"
                            .to_owned(),
                    generation: None,
                    sequence: None,
                    revision: None,
                });
        }
        let boundaries = NativeLifecycleBoundary::ALL
            .into_iter()
            .map(|boundary| {
                let observation = self
                    .boundaries
                    .get(&boundary)
                    .expect("all lifecycle boundaries were completed above");
                serde_json::json!({
                    "name": boundary.as_str(),
                    "status": observation.status.as_str(),
                    "timing": observation.timing.as_str(),
                    "origin": observation.origin.as_str(),
                    "elapsed_ns": observation.elapsed_ns,
                    "source": observation.source.as_str(),
                    "detail": observation.detail.as_str(),
                    "generation": observation.generation,
                    "sequence": observation.sequence,
                    "revision": observation.revision,
                })
            })
            .collect::<Vec<_>>();
        let origins = NativeLifecycleOrigin::ALL
            .into_iter()
            .filter_map(|origin| {
                let observation = self.origins.get(&origin)?;
                let timeline_elapsed_ns = observation
                    .at
                    .saturating_duration_since(started_at)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                Some(serde_json::json!({
                    "name": origin.as_str(),
                    "timeline_elapsed_ns": timeline_elapsed_ns,
                    "source": observation.source.as_str(),
                    "detail": observation.detail.as_str(),
                }))
            })
            .collect::<Vec<_>>();
        let raw_events = self
            .raw_events
            .iter()
            .map(|event| {
                serde_json::json!({
                    "name": event.name.as_str(),
                    "timeline_elapsed_ns": event.timeline_elapsed_ns,
                    "timing": event.timing.as_str(),
                    "source": event.source.as_str(),
                    "generation": event.generation,
                    "sequence": event.sequence,
                    "revision": event.revision,
                    "detail": event.detail.as_str(),
                })
            })
            .collect::<Vec<_>>();
        let report = serde_json::json!({
            "cycle_index": context.cycle_index,
            "step_index": context.step_index,
            "step_id": context.step_id,
            "source_space": context.source_space,
            "target_space": context.target_space,
            "item": context.item,
            "source_window": format!("{:?}", context.source_window),
            "target_window": format!("{:?}", context.target_window),
            "source_hwnd": context.source_hwnd,
            "target_hwnd": context.target_hwnd,
            "base_revision": context.base_revision,
            "clock": "std::time::Instant",
            "timeline_origin": "drag_threshold_move_injection",
            "origins": origins,
            "boundaries": boundaries,
            "raw_events": raw_events,
        });
        self.origins.clear();
        self.boundaries.clear();
        self.raw_events.clear();
        Some(report)
    }
}

fn is_native_mouse_message(message: NativeWindowTestMessage) -> bool {
    matches!(
        message,
        NativeWindowTestMessage::MouseMove
            | NativeWindowTestMessage::PrimaryButtonDown
            | NativeWindowTestMessage::PrimaryButtonUp
    )
}

fn write_native_lifecycle_report(report: &serde_json::Value) -> Result<()> {
    let output = std::env::var(DRAG_LATENCY_OUTPUT_ENV)
        .context("the native lifecycle output path disappeared during measurement")?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("failed to write native lifecycle report `{output}`"))?;
    println!("OPEN_GPUI_DOCK_DRAG_LATENCY_REPORT {report}");
    Ok(())
}

pub(super) fn run_latency_benchmark() {
    let registration = NativeScenarioRegistration {
        id: DRAG_LATENCY_BENCHMARK_ID.to_owned(),
        test: DRAG_LATENCY_BENCHMARK_TEST.to_owned(),
        behavior: NativeDockBehavior::TabDragLatencyBenchmark,
    };
    ensure_native_interactive_runner(&registration.id);
    if process_harness::is_worker(&registration.id, registration.behavior) {
        run_native_interactive_worker(registration);
    }
    process_harness::run_case_in_worker(&registration);
}

pub(super) fn run_worker(registration: NativeScenarioRegistration) -> ! {
    let scenario_id = registration.id;
    let subcase = process_harness::worker_subcase(&scenario_id, registration.behavior);
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let before_application = process_harness::capture_process_window_census()
        .expect("legacy benchmark must capture its pre-application HWND census");
    let panics = Arc::new(Mutex::new(Vec::new()));
    let panics_for_hook = Arc::clone(&panics);
    let previous_hook = take_hook();
    set_hook(Box::new(move |panic| {
        if let Ok(mut panics) = panics_for_hook.lock() {
            panics.push(panic_payload_message(panic.payload()));
        }
        previous_hook(panic);
    }));
    let platform = WindowsPlatform::new_returning_for_test(false)
        .expect("legacy benchmark Windows platform should initialize");
    let mut application =
        Application::with_platform(Rc::new(platform)).with_quit_mode(QuitMode::Explicit);
    let completion = Rc::new(RefCell::new(None));
    let completion_for_application = Rc::clone(&completion);
    let scenario_for_application = scenario_id.clone();
    let worker_observation = observation.clone();
    application.run_returning_for_test(move |app| {
        let workflow = scenario_runner::NativeDockspaceWorkflow::open(app, worker_observation);
        app.spawn(async move |cx| {
            let (result, mut milestones, app_census) = match workflow {
                Ok(mut workflow) => {
                    let result = AssertUnwindSafe(
                        workflow.run_latency_baseline(cx, &scenario_for_application),
                    )
                    .catch_unwind()
                    .await
                    .map_err(|panic| anyhow::anyhow!(panic_message(panic)))
                    .and_then(|result| result);
                    let (result, milestones) = match result {
                        Ok((milestones, report)) => {
                            (write_native_lifecycle_report(&report), milestones)
                        }
                        Err(error) => (Err(error), Vec::new()),
                    };
                    let shutdown = workflow.shutdown(cx).await;
                    let census = workflow.census(cx);
                    let result = match (result, shutdown) {
                        (Err(error), Err(shutdown_error)) => Err(error.context(format!(
                            "legacy benchmark application also failed to shut down: {shutdown_error:#}"
                        ))),
                        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
                        (Ok(()), Ok(())) => Ok(()),
                    };
                    (result, milestones, census)
                }
                Err(error) => {
                    let shutdown = shutdown_native_worker_application_without_surface(cx).await;
                    let error = match shutdown {
                        Ok(()) => error,
                        Err(shutdown_error) => error.context(format!(
                            "legacy benchmark application also failed to shut down: {shutdown_error:#}"
                        )),
                    };
                    (
                        Err(error),
                        Vec::new(),
                        capture_worker_app_census_without_surface(cx),
                    )
                }
            };
            let outcome = match result {
                Ok(()) => {
                    milestones.push("scenario.completed".to_owned());
                    process_harness::NativeWorkerOutcome::Passed
                }
                Err(error) => process_harness::NativeWorkerOutcome::Failed(format!("{error:#}")),
            };
            completion_for_application.replace(Some(NativeWorkerCompletion {
                outcome,
                milestones,
                exit_path: process_harness::NativeWorkerExitPath::ExplicitCleanup,
                app_census,
            }));
            cx.update(|app| app.shutdown_and_quit_for_native_exit_test());
        })
        .detach();
    });
    let mut completion = completion
        .take()
        .expect("legacy benchmark must publish its completion");
    if matches!(
        completion.outcome,
        process_harness::NativeWorkerOutcome::Passed
    ) {
        let panics = panics
            .lock()
            .expect("legacy benchmark panic log must be readable");
        if !panics.is_empty() {
            completion.outcome = process_harness::NativeWorkerOutcome::Failed(format!(
                "legacy benchmark observed unexpected panics: {panics:?}"
            ));
        }
    }
    let events = observation.events();
    let observed = events
        .iter()
        .map(|event| native_generation_label(event.window()))
        .collect::<BTreeSet<_>>();
    let terminal = events
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::NativeTerminal { .. }
            )
        })
        .map(|event| native_generation_label(event.window()))
        .collect::<BTreeSet<_>>();
    let report = process_harness::finish_worker_report(
        &scenario_id,
        subcase,
        completion.outcome,
        completion.milestones,
        completion.exit_path,
        before_application,
        completion.app_census,
        observed.len(),
        terminal.len(),
        observed.difference(&terminal).cloned().collect(),
        None,
    )
    .expect("legacy benchmark must capture its post-application HWND census");
    process_harness::publish_worker_report_and_wait_for_release(&report);
    std::process::exit(0);
}
