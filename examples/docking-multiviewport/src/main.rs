use open_gpui::{
    AnyView, App, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::prelude::{
    DockPanelPlacement, DockSurface, DockSurfaceChange, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfacePrimaryWindowOpened, DockSurfaceViewportOpenOutcome, DockSurfaceViewportUnavailable,
};
use open_gpui_platform::application;
use std::{cell::Cell, rc::Rc, time::Duration};

const MAIN_SPACE: &str = "main";
const SECONDARY_SPACE: &str = "preview-window";
const SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(250);

struct ExamplePanel {
    title: &'static str,
    accent: u32,
    lines: &'static [&'static str],
}

impl ExamplePanel {
    fn new(title: &'static str, accent: u32, lines: &'static [&'static str]) -> Self {
        Self {
            title,
            accent,
            lines,
        }
    }
}

impl Render for ExamplePanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(28.0)).bg(rgb(self.accent)))
                    .child(div().text_lg().child(self.title)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.lines.iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf3f4f6))
                            .text_color(rgb(0x253041))
                            .child(*line)
                    })),
            )
    }
}

fn panel(
    title: &'static str,
    accent: u32,
    lines: &'static [&'static str],
) -> impl Fn(&mut App) -> AnyView {
    move |cx| cx.new(|_| ExamplePanel::new(title, accent, lines)).into()
}

fn build_surface(cx: &mut App) -> DockSurface {
    DockSurface::builder(MAIN_SPACE)
        .panel_placements([
            DockPanelPlacement::left_rail("project").fraction(0.25),
            DockPanelPlacement::center("editor").selected(),
            DockPanelPlacement::right_rail("inspector").fraction(0.26),
            DockPanelPlacement::center("preview"),
        ])
        .panel_factory(
            "project",
            "Project",
            panel("Project", 0x2563eb, &["src", "examples", "docs"]),
        )
        .panel_factory(
            "editor",
            "Editor",
            panel(
                "Editor",
                0x7c3aed,
                &["Facade owns host wiring", "Panels use durable ids"],
            ),
        )
        .panel_factory(
            "inspector",
            "Inspector",
            panel(
                "Inspector",
                0x0f766e,
                &[
                    "Policy gates platform viewports",
                    "Unsupported backends stay single-window",
                ],
            ),
        )
        .panel_factory(
            "preview",
            "Preview",
            panel(
                "Preview",
                0xb45309,
                &[
                    "Detached into its own dock space",
                    "Opened through the DockSurfaceViewports facade",
                ],
            ),
        )
        .allow_floating(true)
        .allow_platform_viewports(true)
        .build(cx)
        .expect("multi-viewport docking surface should validate")
}

fn build_isolated_surface(cx: &mut App) -> DockSurface {
    DockSurface::builder(MAIN_SPACE)
        .panel_placements([
            DockPanelPlacement::center("activity").selected(),
            DockPanelPlacement::right_rail("notes").fraction(0.32),
        ])
        .panel_factory(
            "activity",
            "Activity",
            panel(
                "Activity",
                0xdc2626,
                &["Independent controller state", "Independent window session"],
            ),
        )
        .panel_factory(
            "notes",
            "Notes",
            panel(
                "Notes",
                0x0891b2,
                &[
                    "The logical space id is also main",
                    "State remains surface-local",
                ],
            ),
        )
        .allow_floating(true)
        .build(cx)
        .expect("isolated docking surface should validate")
}

fn main_window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1040.0), px(680.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Multiviewport".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn secondary_window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(560.0), px(420.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus_on_appearing: false,
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Secondary".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn handle_secondary_open_outcome(outcome: DockSurfaceViewportOpenOutcome) {
    match outcome {
        DockSurfaceViewportOpenOutcome::Opened(_) => {}
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::BackendUnsupported
            | DockSurfaceViewportUnavailable::PolicyDisabled(_),
        ) => {}
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::SessionInactive { status },
        ) => {
            log::warn!(
                "secondary docking viewport rejected by inactive surface session: phase={:?} generation={}",
                status.phase(),
                status.generation()
            );
        }
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::OpenFailed(error),
        ) => {
            log::warn!("secondary docking viewport did not open: {error}");
        }
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::InvalidPlacement { error },
        ) => {
            log::warn!("secondary docking viewport placement is invalid: {error}");
        }
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::FlagUnsupported { flags },
        ) => {
            log::warn!("secondary docking viewport uses unsupported window flags: {flags:?}");
        }
    }
}

fn isolated_window_options(cx: &App) -> WindowOptions {
    let mut bounds = Bounds::centered(None, size(px(620.0), px(440.0)), cx);
    bounds.origin.x += px(260.0);
    bounds.origin.y += px(80.0);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus_on_appearing: false,
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Isolated Surface".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn open_primary(
    surface: &DockSurface,
    options: WindowOptions,
    cx: &mut App,
) -> DockSurfacePrimaryWindowOpened {
    match surface.open_primary_window(options, cx) {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
        outcome => panic!("failed to open DockSurface primary window: {outcome:?}"),
    }
}

fn log_platform_window_contract(surface: &DockSurface, cx: &App) {
    let status = surface.viewports().runtime_status(cx);
    for viewport in status.window_profiles {
        let creation = viewport.capabilities.creation;
        let mutations = viewport.capabilities.mutations;
        log::info!(
            "window profile: space={} window={} kind={} nonactivating-appear={:?} transient-owner={:?} first-present={:?} position={:?} size={:?} windowed={:?} maximized={:?} fullscreen={:?} minimized={:?} restore={:?} pointer={:?} activation-policy={:?} alpha={:?} topmost={:?} taskbar={:?} coordinates={:?}",
            viewport.space,
            viewport.window_id.as_u64(),
            viewport.window_kind.as_str(),
            creation.focus_on_appearing,
            creation.transient_for,
            creation.initial_presentation_order,
            mutations.position,
            mutations.size,
            mutations.windowed,
            mutations.maximized,
            mutations.fullscreen,
            mutations.minimized,
            mutations.restore_bounds,
            mutations.pointer_input,
            mutations.activation_policy,
            mutations.alpha,
            mutations.topmost,
            mutations.taskbar_visibility,
            mutations.coordinate_space,
        );
    }
    if let Some(dispatch) = status.last_platform_dispatch {
        log::info!("last platform request: {:?}", dispatch.dispatches);
    }
    if let Some(observed) = status.recent_platform_observations.last() {
        log::info!(
            "last platform observation: request={:?} outcome={:?} facts={:?}",
            observed.observation.request,
            observed.observation.outcome,
            observed.observation.facts,
        );
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let surface = build_surface(cx);
        let isolated_surface = build_isolated_surface(cx);
        let snapshot_export_pending = Rc::new(Cell::new(false));
        let pending_for_events = snapshot_export_pending.clone();
        let surface_for_events = surface.clone();
        surface
            .subscribe_changes(cx, move |event, cx| {
                log::info!(
                    "dock commit revision={} categories={:?}",
                    event.revision(),
                    event.categories()
                );
                if pending_for_events.replace(true) {
                    return;
                }

                let pending = pending_for_events.clone();
                let surface = surface_for_events.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(SNAPSHOT_DEBOUNCE).await;
                    cx.update(|cx| {
                        let snapshot = surface.export_snapshot(cx);
                        log::info!(
                            "application exported dock snapshot revision={} spaces={} viewport_placements={}",
                            snapshot.revision(),
                            snapshot.layout().space_count(),
                            snapshot.viewport_placement().viewports.len()
                        );
                        log_platform_window_contract(&surface, cx);
                        pending.set(false);
                    });
                })
                .detach();
            })
            .detach();

        let primary = open_primary(&surface, main_window_options(cx), cx);
        let isolated_primary =
            open_primary(&isolated_surface, isolated_window_options(cx), cx);
        assert_ne!(primary.window(), isolated_primary.window());
        log::info!(
            "isolated DockSurface sessions opened: shared-space={} primary-generation={} isolated-generation={}",
            MAIN_SPACE,
            primary.generation(),
            isolated_primary.generation()
        );

        let viewports = surface.viewports();
        if cx.viewport_capabilities().platform_viewport_windows {
            match surface.detach_panel_to_space(MAIN_SPACE, "preview", SECONDARY_SPACE, cx) {
                Ok(DockSurfaceChange::Changed | DockSurfaceChange::Unchanged) => {
                    handle_secondary_open_outcome(viewports.open(
                        SECONDARY_SPACE,
                        secondary_window_options(cx),
                        cx,
                    ));
                }
                Err(error) => {
                    log::warn!("preview panel did not detach into a secondary space: {error}")
                }
            }
        } else {
            handle_secondary_open_outcome(viewports.open(
                SECONDARY_SPACE,
                secondary_window_options(cx),
                cx,
            ));
        }

        let snapshot = surface.export_snapshot(cx);
        log::info!(
            "dock snapshot exported: {} spaces, {} viewport placements",
            snapshot.layout().space_count(),
            snapshot.viewport_placement().viewports.len()
        );
        match viewports.check_restore(snapshot.viewport_placement(), cx) {
            Ok(readiness) => log::info!("viewport placement restore readiness: {readiness:?}"),
            Err(error) => log::warn!("viewport placement restore check failed: {error}"),
        }
        log_platform_window_contract(&surface, cx);

        let surface_for_activation = surface.clone();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            cx.update(|cx| {
                let (request, completion) = surface_for_activation
                    .activate_panel_with_completion("editor", cx, move |outcome, _cx| {
                        log::info!("editor activation settled: {outcome:?}");
                    });
                log::info!("requested editor activation sequence={}", request.sequence());
                completion.detach();
            });
        })
        .detach();

        cx.activate(true);
    });
}
