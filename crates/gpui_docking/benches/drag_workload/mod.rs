//! The same retained-host gesture for latency and allocation measurements.

use anyhow::{Result, ensure};
use open_gpui::{
    Context, IntoElement, Modifiers, MouseButton, Pixels, Point, Render, RequestFrameOptions,
    TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};
use open_gpui_docking::{
    DockPane, DockRegion, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        geometry::LogicalRect,
        model::{
            DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
            RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    },
};
use serde_json::json;

const LEFT: SurfaceId = SurfaceId::new(1);
const RIGHT: SurfaceId = SurfaceId::new(2);
const MOVING: ItemId = ItemId::new(3);
const FRAMES_PER_EDGE: usize = 4;

struct Root(DockSurface);

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.regions([
            DockRegion::new(LEFT, LogicalRect::new(0.0, 0.0, 400.0, 600.0).unwrap()),
            DockRegion::new(RIGHT, LogicalRect::new(400.0, 0.0, 400.0, 600.0).unwrap()),
        ]))
    }
}

pub(crate) struct DragWorkload {
    dock: DockSurface,
    visual: VisualTestContext,
}

impl DragWorkload {
    pub(crate) fn new() -> Result<Self> {
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(
                LEFT,
                DockspaceRootLayout::new(
                    RootId::new(1),
                    DockspaceNode::tabs_with_selection([ItemId::new(1), MOVING], Some(MOVING))?,
                ),
            ),
            DockspaceSurfaceLayout::new(
                RIGHT,
                DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([ItemId::new(2)])),
            ),
        ])?;
        let session = DockspaceSession::from_layout(layout, DockPolicy::default())?;
        let mut app = TestAppContext::single();
        app.defer_next_window_frame_requests();
        let dock = app.update(|cx| {
            let panes = [
                (ItemId::new(1), "Left"),
                (ItemId::new(2), "Right"),
                (MOVING, "Moving"),
            ]
            .map(|(item, title)| DockPane::new(item, title, cx.new(|_| open_gpui::Empty)));
            DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx)
        })?;
        let window = app.open_window(size(px(800.0), px(600.0)), |_, _| Root(dock.clone()));
        app.set_platform_hovered_window(Some(window.into()));
        let mut visual = VisualTestContext::from_window(window.into(), &app);
        visual.update(|window, _| {
            let _ = window.activate_window();
        });
        let mut work = Self { dock, visual };
        work.frames();
        work.cycle()?;
        Ok(work)
    }

    fn frames(&mut self) {
        for _ in 0..FRAMES_PER_EDGE {
            assert!(self.visual.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
        }
    }

    pub(crate) fn cycle(&mut self) -> Result<()> {
        self.drag(point(px(600.0), px(313.0)), RIGHT)?;
        self.drag(point(px(200.0), px(313.0)), LEFT)
    }

    fn drag(&mut self, destination: Point<Pixels>, surface: SurfaceId) -> Result<()> {
        let source = self
            .visual
            .debug_bounds("dockspace:tab:3")
            .ok_or_else(|| anyhow::anyhow!("moving tab must be visible"))?
            .center();
        self.visual
            .simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        self.frames();
        self.visual
            .simulate_mouse_move(destination, Some(MouseButton::Left), Modifiers::none());
        self.frames();
        self.visual
            .simulate_mouse_up(destination, MouseButton::Left, Modifiers::none());
        self.frames();
        ensure!(
            self.dock.with_view(|view| view
                .item(MOVING)
                .is_some_and(|pane| pane.surface() == surface)),
            "drag did not move the pane to {surface:?}"
        );
        ensure!(
            !self.dock.has_active_gesture(),
            "the release must settle the drag"
        );
        ensure!(self.dock.error().is_none(), "{:?}", self.dock.error());
        Ok(())
    }
}

pub(crate) fn report(iterations: usize, measurement: serde_json::Value) -> serde_json::Value {
    json!({
        "schema_version": 1,
        "adapter": "shared_session",
        "measurement_level": "gpui_retained_host",
        "debug_assertions": cfg!(debug_assertions),
        "renderer": "gpui_test_platform",
        "scenario_id": "two_region_tab_drag_round_trip",
        "workload_contract": "same-window|two-400x600-regions|three-panes|press-move-release|right-center>left-center",
        "frames_per_pointer_edge": FRAMES_PER_EDGE,
        "drag_count_per_cycle": 2,
        "iterations_per_sample": iterations,
        "benchmark_passed": 1,
        "measurement": measurement,
    })
}
