//! Retained GPUI host CPU work through the public shared-session adapter.
//!
//! Uses TestPlatform, NoopTextSystem and simulated presentation, with no native
//! windows or GPU renderer. This programmatic two-step rehome workload has no
//! equivalent row in the legacy graph/controller or Dear ImGui comparator.

use anyhow::{Result, ensure};
use open_gpui::{
    Context, Entity, IntoElement, Render, RequestFrameOptions, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px, size,
};
use open_gpui_docking::{
    DockPane, DockRegion, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        geometry::LogicalRect,
        model::{
            DockPlacement, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
            DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    },
};
use serde_json::json;
use std::{cell::Cell, hint::black_box, rc::Rc, time::Instant};

const LEFT: SurfaceId = SurfaceId::new(1);
const RIGHT: SurfaceId = SurfaceId::new(2);
const LEFT_ANCHOR: ItemId = ItemId::new(1);
const RIGHT_ANCHOR: ItemId = ItemId::new(2);
const MOVING: ItemId = ItemId::new(3);
const FRAMES_PER_STEP: usize = 4;
const STEPS_PER_CYCLE: usize = 2;
const WARMUP_CYCLES: usize = 20;
const ITERATIONS: usize = 100;
const SAMPLES: usize = 7;

struct Pane {
    label: &'static str,
    renders: Rc<Cell<usize>>,
}

impl Render for Pane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let label = self.label;
        div()
            .size_full()
            .debug_selector(move || label.to_owned())
            .child(label)
    }
}

struct Root {
    dock: DockSurface,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dock.regions([
            DockRegion::new(LEFT, LogicalRect::new(0.0, 0.0, 400.0, 600.0).unwrap()),
            DockRegion::new(RIGHT, LogicalRect::new(400.0, 0.0, 400.0, 600.0).unwrap()),
        ]))
    }
}

struct Benchmark {
    dock: DockSurface,
    visual: VisualTestContext,
    moving_view: Entity<Pane>,
}

impl Benchmark {
    fn new() -> Result<Self> {
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(
                LEFT,
                DockspaceRootLayout::new(
                    RootId::new(1),
                    DockspaceNode::tabs_with_selection([LEFT_ANCHOR, MOVING], Some(MOVING))?,
                ),
            ),
            DockspaceSurfaceLayout::new(
                RIGHT,
                DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([RIGHT_ANCHOR])),
            ),
        ])?;
        let session = DockspaceSession::from_layout(layout, DockPolicy::default())?;
        let mut app = TestAppContext::single();
        app.defer_next_window_frame_requests();
        let (dock, moving_view) = app.update(|cx| -> Result<_> {
            let views = ["left-anchor", "right-anchor", "moving-pane"].map(|label| {
                cx.new(|_| Pane {
                    label,
                    renders: Rc::new(Cell::new(0)),
                })
            });
            let panes = [LEFT_ANCHOR, RIGHT_ANCHOR, MOVING]
                .into_iter()
                .zip(&views)
                .map(|(item, view)| DockPane::new(item, view.read(cx).label, view.clone()))
                .collect::<Vec<_>>();
            let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx)?;
            Ok((dock, views[2].clone()))
        })?;
        let window = app.open_window(size(px(800.0), px(600.0)), |_, _| Root {
            dock: dock.clone(),
        });
        let mut benchmark = Self {
            dock,
            visual: VisualTestContext::from_window(window.into(), &app),
            moving_view,
        };
        benchmark.frames()?;
        benchmark.validate()?;
        Ok(benchmark)
    }

    fn frames(&mut self) -> Result<()> {
        for _ in 0..FRAMES_PER_STEP {
            ensure!(
                self.visual.simulate_frame(RequestFrameOptions {
                    require_presentation: true,
                    force_render: true,
                    ..Default::default()
                }),
                "the test platform must deliver each forced host frame"
            );
        }
        Ok(())
    }

    fn cycle(&mut self) -> Result<()> {
        for anchor in [RIGHT_ANCHOR, LEFT_ANCHOR] {
            self.rehome(anchor)?;
        }
        Ok(())
    }

    fn rehome(&mut self, anchor: ItemId) -> Result<()> {
        self.visual.cx.update(|cx| {
            self.dock.update(
                |frame| {
                    frame.dock_item_current(MOVING, DockPlacement::After(anchor))?;
                    frame.select_item_current(MOVING)?;
                    Ok(())
                },
                cx,
            )
        })?;
        self.frames()
    }

    fn serial(&self) -> Result<u64> {
        let inspection = self.dock.inspection()?;
        ensure!(
            inspection.mounts.len() == 1,
            "one test window must remain mounted"
        );
        Ok(inspection.mounts[0].serial)
    }

    fn moving_renders(&self) -> usize {
        self.moving_view
            .read_with(&self.visual, |view, _| view.renders.get())
    }

    fn validate_destination(&mut self, surface: SurfaceId) -> Result<()> {
        ensure!(
            self.dock.with_view(|view| view
                .item(MOVING)
                .is_some_and(|item| { item.surface() == surface && item.is_active() })),
            "the moving item must be selected in its destination surface"
        );
        let bounds = self.visual.debug_bounds("moving-pane");
        ensure!(
            bounds.is_some_and(|bounds| {
                let center = f32::from(bounds.center().x);
                if surface == LEFT {
                    (0.0..400.0).contains(&center)
                } else {
                    (400.0..800.0).contains(&center)
                }
            }),
            "the same retained View must paint inside its destination region"
        );
        Ok(())
    }

    fn validate(&mut self) -> Result<()> {
        let expected: [(SurfaceId, &[ItemId], ItemId); 2] = [
            (LEFT, &[LEFT_ANCHOR, MOVING], MOVING),
            (RIGHT, &[RIGHT_ANCHOR], RIGHT_ANCHOR),
        ];
        ensure!(
            self.dock
                .with_view(|view| expected.into_iter().all(|(id, items, selected)| {
                    view.surface(id).is_some_and(|surface| {
                        surface.contained_count() == 0
                            && surface.main_root().is_some_and(|root| {
                                root.id() == RootId::new(id.get())
                                    && root.central().is_none()
                                    && root.content().and_then(|node| node.tabs()).is_some_and(
                                        |tabs| {
                                            tabs.items().eq(items.iter().copied())
                                                && tabs.selected() == Some(selected.into())
                                        },
                                    )
                            })
                    })
                })),
            "each cycle must restore exact topology, item order and selection"
        );
        let inspection = self.dock.inspection()?;
        ensure!(
            inspection.last_error.is_none(),
            "shared host failure: {:?}",
            inspection.last_error
        );
        ensure!(
            inspection.model_surface_count == 2
                && inspection.open_pane_count == 3
                && inspection.registered_pane_count == 3
                && !inspection.update_pending,
            "the shared host must retain both surfaces and all three pane entities"
        );
        ensure!(
            inspection.mounts.len() == 1,
            "one test window must remain mounted"
        );
        let mount = &inspection.mounts[0];
        ensure!(
            mount.regions.len() == 2
                && mount.regions.iter().all(|region| {
                    !region.dirty
                        && region.packet_deferred == Some(false)
                        && region.current_presentation
                        && region.last_presented_serial == Some(mount.serial)
                }),
            "both regions must execute their paint programs and submit current packets"
        );
        self.validate_destination(LEFT)
    }
}

fn main() -> Result<()> {
    // Match the test renderer's stack budget on Windows. Executable main
    // threads otherwise have a smaller stack than the library's test threads.
    std::thread::Builder::new()
        .name("shared-docking-benchmark".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(run)?
        .join()
        .map_err(|_| anyhow::anyhow!("the shared docking benchmark thread panicked"))?
}

fn run() -> Result<()> {
    let mut benchmark = Benchmark::new()?;
    benchmark.rehome(RIGHT_ANCHOR)?;
    benchmark.validate_destination(RIGHT)?;
    benchmark.rehome(LEFT_ANCHOR)?;
    benchmark.validate()?;
    for _ in 0..WARMUP_CYCLES {
        benchmark.cycle()?;
    }
    benchmark.validate()?;

    let mut measurements = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let serial = benchmark.serial()?;
        let renders = benchmark.moving_renders();
        let started = Instant::now();
        for _ in 0..ITERATIONS {
            black_box(benchmark.cycle())?;
        }
        let elapsed = started.elapsed();
        benchmark.validate()?;
        ensure!(
            benchmark.serial()? >= serial + (ITERATIONS * STEPS_PER_CYCLE) as u64,
            "every rehome must prepare fresh host output instead of replaying only cached packets"
        );
        ensure!(
            benchmark.moving_renders() >= renders + ITERATIONS * STEPS_PER_CYCLE,
            "the same retained View must render at each destination"
        );
        measurements.push(elapsed.as_nanos() as f64 / ITERATIONS as f64);
    }
    let mut ordered = measurements.clone();
    ordered.sort_by(f64::total_cmp);
    let median = ordered[SAMPLES / 2];
    println!(
        "{}",
        json!({
            "schema_version": 1,
            "adapter": "shared_session",
            "measurement_level": "gpui_retained_host",
            "debug_assertions": cfg!(debug_assertions),
            "renderer": "gpui_test_platform",
            "text_system": "noop",
            "native_windows": false,
            "gpu_submission": false,
            "benchmark_passed": 1,
            "comparison_eligible": false,
            "rows": [{
                "scenario_id": "shared_retained_host_two_region_rehome_cycle",
                "operation_family": "programmatic_item_rehome_sequence",
                "measurement_layer": "gpui_retained_host_update_prepare_commit_scene_encode",
                "scope": "one_test_window_two_logical_surfaces_three_retained_panes",
                "workload_contract": "shared-host-v1|800x600|two-400x600-regions|moving-after-right-anchor>moving-after-left-anchor|select-moving|four-forced-frames-per-step",
                "operation_step_count": STEPS_PER_CYCLE,
                "forced_frames_per_step": FRAMES_PER_STEP,
                "preflight_cycles": 1,
                "warmup_cycles": WARMUP_CYCLES,
                "iterations_per_sample": ITERATIONS,
                "sample_count": SAMPLES,
                "samples_ns_per_cycle": measurements,
                "ns_per_cycle": median,
                "ns_per_step": median / STEPS_PER_CYCLE as f64,
                "measurement_includes": [
                    "public_application_intent",
                    "shared_session_prepare_commit",
                    "surface_paint_program_execution",
                    "gpui_retained_pane_layout_prepaint_paint",
                    "test_platform_frame_submission",
                    "test_executor_drain"
                ],
                "validation_outside_timing": true
            }]
        })
    );
    benchmark.visual.cx.quit();
    Ok(())
}
