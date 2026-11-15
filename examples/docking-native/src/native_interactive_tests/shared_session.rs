//! Shared managed native-session fixture for Windows interactive journeys.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use anyhow::{Context as _, Result, bail, ensure};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Empty, NativeCapturedDragEvent,
    NativeCapturedDragGeneration, NativeCapturedDragReleaseTerminal, px,
};
use open_gpui_docking::{
    DockPane, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        model::{
            DockspaceAxis, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
            DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    },
    native::NativeReport,
};
use open_gpui_windows::{
    NativeTestDisplay, NativeTestPointerAction, NativeTestSystemPointerGuard,
    NativeWindowTestIdentity, NativeWindowTestMessage, NativeWindowTestObservation,
    native_test_acquire_foreground_window, native_test_logical_client_point_to_screen,
    native_test_non_shell_root_window_at,
};
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, VK_LBUTTON},
        WindowsAndMessaging::{IsWindow, IsWindowVisible},
    },
};

use super::process_harness;
use super::{
    native_generation_label, native_system_mouse_message_observed, raw_hwnd,
    unique_matching_debug_bounds, unique_native_identity_for_window, wait_for_cursor, wait_until,
    window_has_non_empty_frame, window_scale_factor,
};

pub(super) const SHARED_SOURCE_SURFACE: SurfaceId = SurfaceId::new(1);
pub(super) const SHARED_TARGET_SURFACE: SurfaceId = SurfaceId::new(2);
pub(super) const SHARED_MIDDLE_SURFACE: SurfaceId = SurfaceId::new(3);
pub(super) const SHARED_SURVIVOR_SURFACE: SurfaceId = SurfaceId::new(10);
pub(super) const SHARED_SOURCE_ITEM: ItemId = ItemId::new(101);
pub(super) const SHARED_TARGET_ITEM: ItemId = ItemId::new(102);
pub(super) const SHARED_MIDDLE_ITEM: ItemId = ItemId::new(103);
pub(super) const SHARED_TARGET_SECONDARY_ITEM: ItemId = ItemId::new(104);
pub(super) const SHARED_SOURCE_SECONDARY_ITEM: ItemId = ItemId::new(105);
pub(super) const SHARED_MIDDLE_SECONDARY_ITEM: ItemId = ItemId::new(106);
pub(super) const SHARED_SURVIVOR_ITEM: ItemId = ItemId::new(110);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SharedFixtureShape {
    TwoWindows,
    ThreeWindows,
    SurvivorWindow,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SharedWindow {
    pub(super) surface: SurfaceId,
    pub(super) item: ItemId,
    pub(super) window: AnyWindowHandle,
    pub(super) hwnd: HWND,
    pub(super) identity: Option<NativeWindowTestIdentity>,
}

pub(super) struct SharedCapturedDrag {
    pub(super) generation: NativeCapturedDragGeneration,
    pub(super) source: SharedWindow,
    pub(super) press_point: POINT,
    pub(super) threshold_point: POINT,
}

pub(super) struct SharedSession {
    pub(super) dock: DockSurface,
    pub(super) windows: Vec<SharedWindow>,
    pub(super) observation: NativeWindowTestObservation,
    pub(super) reports: Vec<NativeReport>,
    pub(super) drag_events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    pub(super) drag_observer: Option<open_gpui::Subscription>,
    pub(super) pointer: Option<NativeTestSystemPointerGuard>,
    pub(super) initial_revision: u64,
    pub(super) initial_window_count: usize,
    pub(super) initial_binding_count: usize,
    pub(super) milestones: Vec<String>,
}

impl SharedSession {
    pub(super) fn build(
        cx: &mut App,
        observation: NativeWindowTestObservation,
        shape: SharedFixtureShape,
        bounds: &[open_gpui::Bounds<open_gpui::Pixels>],
    ) -> Result<Self> {
        let (layout, panes, surfaces) = match shape {
            SharedFixtureShape::TwoWindows => {
                ensure!(
                    bounds.len() == 2,
                    "two-window shared fixture needs two bounds"
                );
                let layout = DockspaceLayout::new([
                    DockspaceSurfaceLayout::new(
                        SHARED_SOURCE_SURFACE,
                        DockspaceRootLayout::new(
                            RootId::new(SHARED_SOURCE_SURFACE.get()),
                            DockspaceNode::tabs([SHARED_SOURCE_ITEM, SHARED_SOURCE_SECONDARY_ITEM]),
                        ),
                    ),
                    DockspaceSurfaceLayout::new(
                        SHARED_TARGET_SURFACE,
                        DockspaceRootLayout::new(
                            RootId::new(SHARED_TARGET_SURFACE.get()),
                            DockspaceNode::tabs([SHARED_TARGET_ITEM]),
                        ),
                    ),
                ])?;
                (
                    layout,
                    vec![
                        DockPane::new(SHARED_SOURCE_ITEM, "Shared source", cx.new(|_| Empty)),
                        DockPane::new(
                            SHARED_SOURCE_SECONDARY_ITEM,
                            "Shared source secondary",
                            cx.new(|_| Empty),
                        ),
                        DockPane::new(SHARED_TARGET_ITEM, "Shared target", cx.new(|_| Empty)),
                    ],
                    vec![
                        (SHARED_SOURCE_SURFACE, SHARED_SOURCE_ITEM, bounds[0]),
                        (SHARED_TARGET_SURFACE, SHARED_TARGET_ITEM, bounds[1]),
                    ],
                )
            }
            SharedFixtureShape::ThreeWindows => {
                ensure!(
                    bounds.len() == 3,
                    "three-window shared fixture needs three bounds"
                );
                let target_content = DockspaceNode::split(
                    DockspaceAxis::Vertical,
                    [
                        (DockspaceNode::tabs([SHARED_TARGET_ITEM]), 0.5),
                        (DockspaceNode::tabs([SHARED_TARGET_SECONDARY_ITEM]), 0.5),
                    ],
                )?;
                let layout = DockspaceLayout::new([
                    DockspaceSurfaceLayout::new(
                        SHARED_SOURCE_SURFACE,
                        DockspaceRootLayout::new(
                            RootId::new(SHARED_SOURCE_SURFACE.get()),
                            DockspaceNode::tabs([SHARED_SOURCE_ITEM, SHARED_SOURCE_SECONDARY_ITEM]),
                        ),
                    ),
                    DockspaceSurfaceLayout::new(
                        SHARED_TARGET_SURFACE,
                        DockspaceRootLayout::new(
                            RootId::new(SHARED_TARGET_SURFACE.get()),
                            target_content,
                        ),
                    ),
                    DockspaceSurfaceLayout::new(
                        SHARED_MIDDLE_SURFACE,
                        DockspaceRootLayout::new(
                            RootId::new(SHARED_MIDDLE_SURFACE.get()),
                            DockspaceNode::tabs([SHARED_MIDDLE_ITEM, SHARED_MIDDLE_SECONDARY_ITEM]),
                        ),
                    ),
                ])?;
                (
                    layout,
                    vec![
                        DockPane::new(SHARED_SOURCE_ITEM, "Shared source", cx.new(|_| Empty)),
                        DockPane::new(SHARED_TARGET_ITEM, "Shared target", cx.new(|_| Empty)),
                        DockPane::new(SHARED_MIDDLE_ITEM, "Shared middle", cx.new(|_| Empty)),
                        DockPane::new(
                            SHARED_TARGET_SECONDARY_ITEM,
                            "Shared target secondary",
                            cx.new(|_| Empty),
                        ),
                        DockPane::new(
                            SHARED_SOURCE_SECONDARY_ITEM,
                            "Shared source secondary",
                            cx.new(|_| Empty),
                        ),
                        DockPane::new(
                            SHARED_MIDDLE_SECONDARY_ITEM,
                            "Shared middle secondary",
                            cx.new(|_| Empty),
                        ),
                    ],
                    vec![
                        (SHARED_SOURCE_SURFACE, SHARED_SOURCE_ITEM, bounds[0]),
                        (SHARED_TARGET_SURFACE, SHARED_TARGET_ITEM, bounds[1]),
                        (SHARED_MIDDLE_SURFACE, SHARED_MIDDLE_ITEM, bounds[2]),
                    ],
                )
            }
            SharedFixtureShape::SurvivorWindow => {
                ensure!(bounds.len() == 1, "survivor fixture needs one bound");
                let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
                    SHARED_SURVIVOR_SURFACE,
                    DockspaceRootLayout::new(
                        RootId::new(SHARED_SURVIVOR_SURFACE.get()),
                        DockspaceNode::tabs([SHARED_SURVIVOR_ITEM]),
                    ),
                )])?;
                (
                    layout,
                    vec![DockPane::new(
                        SHARED_SURVIVOR_ITEM,
                        "Shared survivor",
                        cx.new(|_| Empty),
                    )],
                    vec![(SHARED_SURVIVOR_SURFACE, SHARED_SURVIVOR_ITEM, bounds[0])],
                )
            }
        };
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        let dock = DockSurface::new_managed(
            DockspaceSession::from_layout(layout, policy)?,
            panes,
            GpuiDockspaceConfig::default(),
            cx,
        )?;
        let initial_revision = 0;
        let initial_window_count = cx.windows().len();
        let initial_binding_count = dock.inspection()?.native_window_binding_count;
        let drag_events = Rc::new(RefCell::new(Vec::new()));
        let drag_events_for_observer = drag_events.clone();
        let drag_observer = cx.observe_native_captured_drag(move |event, _| {
            drag_events_for_observer.borrow_mut().push(event);
        });
        let mut session = Self {
            dock,
            windows: Vec::new(),
            observation,
            reports: Vec::new(),
            drag_events,
            drag_observer: Some(drag_observer),
            pointer: None,
            initial_revision,
            initial_window_count,
            initial_binding_count,
            milestones: Vec::new(),
        };
        for (index, (surface, item, bounds)) in surfaces.into_iter().enumerate() {
            let mut options = super::viewport_window_options(bounds);
            options.focus_on_appearing = index == 0;
            let window = session
                .dock
                .open_native_root(surface, index == 0, options, cx)
                .with_context(|| format!("shared root {surface:?} failed to open"))?;
            let hwnd = raw_hwnd(cx, window)?;
            session.windows.push(SharedWindow {
                surface,
                item,
                window,
                hwnd,
                identity: None,
            });
        }
        Ok(session)
    }

    pub(super) async fn wait_ready(&mut self, cx: &mut AsyncApp) -> Result<()> {
        wait_until(
            cx,
            "shared managed roots to submit non-empty frames",
            |cx| {
                self.pump(cx);
                self.check_health()?;
                cx.update(|app| {
                    self.windows.iter().try_fold(true, |ready, window| {
                        Ok::<_, anyhow::Error>(
                            ready
                                && window_has_non_empty_frame(app, window.window)?
                                && unsafe { IsWindowVisible(window.hwnd).as_bool() },
                        )
                    })
                })
            },
        )
        .await?;
        for window in &mut self.windows {
            window.hwnd = cx.update(|app| raw_hwnd(app, window.window))?;
            window.identity = unique_native_identity_for_window(
                &self.observation.events(),
                window.window.window_id(),
                "shared managed root",
            )?
            .into();
            ensure!(
                self.dock
                    .native_binding_for_window(window.window.window_id())
                    .is_some(),
                "shared managed root did not retain a native binding"
            );
        }
        ensure!(
            self.windows
                .iter()
                .map(|window| window.hwnd.0 as isize)
                .collect::<BTreeSet<_>>()
                .len()
                == self.windows.len(),
            "shared fixture roots must own distinct HWNDs"
        );
        self.milestones.push("shared.roots-ready".to_owned());
        Ok(())
    }

    pub(super) fn pump(&mut self, cx: &mut AsyncApp) {
        cx.update(|app| self.dock.pump_native(app));
        self.reports.extend(self.dock.take_native_reports());
    }

    pub(super) fn check_health(&self) -> Result<()> {
        if let Some(error) = self.dock.failure() {
            bail!("shared managed dock failed: {error:?}");
        }
        for report in &self.reports {
            match report {
                NativeReport::OpenFailed { stage, message, .. } => {
                    bail!("shared native create failed at {stage:?}: {message}")
                }
                NativeReport::Unsupported { operation } => {
                    bail!("shared native operation unsupported: {operation}")
                }
                NativeReport::CaptureTerminal {
                    terminal: NativeCapturedDragReleaseTerminal::Failed,
                    ..
                } => bail!("shared native capture release failed: {report:?}"),
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn window(&self, surface: SurfaceId) -> Result<SharedWindow> {
        self.windows
            .iter()
            .copied()
            .find(|window| window.surface == surface)
            .with_context(|| format!("shared surface {surface:?} has no physical window"))
    }

    pub(super) fn target_window(&self) -> Result<SharedWindow> {
        self.window(SHARED_TARGET_SURFACE)
    }

    pub(super) fn source_window(&self) -> Result<SharedWindow> {
        self.window(SHARED_SOURCE_SURFACE)
    }

    pub(super) async fn begin_captured_drag(
        &mut self,
        cx: &mut AsyncApp,
        source: SharedWindow,
        item: ItemId,
    ) -> Result<SharedCapturedDrag> {
        ensure!(
            unsafe { GetCapture() } == HWND::default(),
            "shared drag began with native capture"
        );
        if unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 != 0 {
            bail!("shared drag began with the physical primary button pressed");
        }
        let mut pointer = NativeTestSystemPointerGuard::capture()?;
        let bounds = cx.update(|app| {
            unique_matching_debug_bounds(
                app,
                source.window,
                "dockspace:tab:",
                &item.get().to_string(),
            )
        })?;
        let scale = cx.update(|app| window_scale_factor(app, source.window))?;
        let press_point =
            native_test_logical_client_point_to_screen(source.hwnd, bounds.center(), scale)?;
        let threshold_point = native_test_logical_client_point_to_screen(
            source.hwnd,
            open_gpui::point(bounds.center().x + px(28.0), bounds.center().y),
            scale,
        )?;
        for point in [threshold_point, press_point] {
            ensure!(
                native_test_non_shell_root_window_at(point) == Some(source.hwnd),
                "shared drag point is occluded: point={point:?}, source={:?}",
                source.hwnd
            );
            pointer.inject(point, NativeTestPointerAction::Move)?;
            let delivered = wait_for_cursor(cx, point, "shared source pointer canary").await?;
            wait_until(cx, "shared source WndProc pointer canary", |_| {
                Ok(native_system_mouse_message_observed(
                    &self.observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered,
                ))
            })
            .await?;
        }
        native_test_acquire_foreground_window(source.hwnd.0 as isize)?;
        pointer.inject(press_point, NativeTestPointerAction::PrimaryDown)?;
        wait_until(cx, "shared source native capture", |_| {
            Ok(unsafe { GetCapture() } == source.hwnd
                && native_system_mouse_message_observed(
                    &self.observation,
                    source.window,
                    NativeWindowTestMessage::PrimaryButtonDown,
                    press_point,
                ))
        })
        .await?;
        pointer.inject(threshold_point, NativeTestPointerAction::Move)?;
        wait_until(cx, "shared managed drag activation", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(cx.update(|app| app.has_active_drag())
                && cx.update(|app| self.dock.has_active_gesture())
                && unsafe { GetCapture() } == source.hwnd)
        })
        .await?;
        let generation = self
            .drag_events
            .borrow()
            .iter()
            .find(|event| event.source_window() == source.window.window_id())
            .map(NativeCapturedDragEvent::generation)
            .context("shared managed drag did not publish a native capture generation")?;
        self.pointer = Some(pointer);
        self.milestones.push("shared.drag-captured".to_owned());
        Ok(SharedCapturedDrag {
            generation,
            source,
            press_point,
            threshold_point,
        })
    }

    pub(super) async fn move_captured_pointer(
        &mut self,
        cx: &mut AsyncApp,
        point: POINT,
        description: &str,
    ) -> Result<POINT> {
        let pointer = self
            .pointer
            .as_mut()
            .context("shared drag pointer guard is unavailable")?;
        pointer.inject(point, NativeTestPointerAction::Move)?;
        let delivered = wait_for_cursor(cx, point, description).await?;
        self.pump(cx);
        self.check_health()?;
        Ok(delivered)
    }

    pub(super) fn release_captured_pointer(&mut self, point: POINT) -> Result<()> {
        self.pointer
            .as_mut()
            .context("shared drag pointer guard is unavailable")?
            .inject(point, NativeTestPointerAction::PrimaryUp)
            .context("shared drag release injection failed")
    }

    pub(super) async fn wait_drag_released(&mut self, cx: &mut AsyncApp) -> Result<()> {
        wait_until(cx, "shared native capture release", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(cx.update(|app| !app.has_active_drag())
                && self.dock.native_pointer_continuation().is_none()
                && unsafe { GetCapture() } == HWND::default())
        })
        .await
    }

    pub(super) fn assert_release_terminal(
        &self,
        source: SharedWindow,
        generation: NativeCapturedDragGeneration,
    ) -> Result<()> {
        ensure!(
            self.reports.iter().any(|report| matches!(
                report,
                NativeReport::CaptureTerminal { barrier, terminal, .. }
                    if barrier.source_window() == source.window.window_id()
                        && barrier.drag_generation() == generation
                        && *terminal == NativeCapturedDragReleaseTerminal::Released
            )),
            "shared session did not publish the original Released capture terminal: source={source:?}, generation={generation:?}, reports={:?}",
            self.reports
        );
        Ok(())
    }

    pub(super) async fn shutdown(&mut self, cx: &mut AsyncApp) -> Result<()> {
        if let Some(mut pointer) = self.pointer.take() {
            if !pointer.release_primary_button_best_effort() {
                bail!("shared session could not restore the physical primary button");
            }
        }
        cx.update(|app| self.dock.begin_native_shutdown(app))?;
        wait_until(cx, "shared managed session quiescence", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(self.dock.native_is_quiescent()
                && cx.update(|app| app.windows().is_empty() && !app.has_active_drag())
                && unsafe { GetCapture() } == HWND::default())
        })
        .await
    }

    pub(super) fn capture_census(
        &self,
        cx: &mut AsyncApp,
    ) -> process_harness::NativeWorkerAppCensus {
        cx.update(|app| {
            let inspection = self.dock.inspection().ok();
            process_harness::NativeWorkerAppCensus {
                window_registry_count: app.windows().len(),
                active_drag: app.has_active_drag(),
                native_exit_authority_settled: app.native_exit_authority_is_settled_for_test(),
                surface: process_harness::NativeWorkerSurfaceCensus::constructed(
                    self.dock.native_is_quiescent(),
                    inspection
                        .as_ref()
                        .map(|inspection| !inspection.pending_native_input),
                    inspection.as_ref().map_or(0, |inspection| {
                        inspection
                            .native
                            .as_ref()
                            .map_or(0, |native| native.pending_root_terminal_count)
                    }),
                    inspection.as_ref().map_or(0, |inspection| {
                        inspection
                            .native
                            .as_ref()
                            .map_or(0, |native| native.retiring_binding_count)
                    }),
                ),
            }
        })
    }

    pub(super) fn native_generation_sets(&self) -> (BTreeSet<String>, BTreeSet<String>) {
        let events = self.observation.events();
        let observed = events
            .iter()
            .map(|event| native_generation_label(event.window()))
            .collect::<BTreeSet<_>>();
        let terminal = events
            .iter()
            .filter(|event| {
                matches!(
                    event.kind(),
                    open_gpui_windows::NativeWindowTestEventKind::NativeTerminal { .. }
                )
            })
            .map(|event| native_generation_label(event.window()))
            .collect::<BTreeSet<_>>();
        (observed, terminal)
    }
}

pub(super) fn two_window_bounds(cx: &mut App) -> Result<Vec<open_gpui::Bounds<open_gpui::Pixels>>> {
    let display = cx
        .primary_display()
        .context("shared fixture needs a primary display")?;
    two_window_bounds_from_bounds(display.bounds())
}

pub(super) fn two_window_bounds_on_display(
    cx: &mut App,
    display: NativeTestDisplay,
) -> Result<Vec<open_gpui::Bounds<open_gpui::Pixels>>> {
    let display = cx
        .find_display(display.display_id())
        .context("shared fixture could not resolve its selected display")?;
    two_window_bounds_from_bounds(display.bounds())
}

fn two_window_bounds_from_bounds(
    display_bounds: open_gpui::Bounds<open_gpui::Pixels>,
) -> Result<Vec<open_gpui::Bounds<open_gpui::Pixels>>> {
    let gap = 32.0;
    let width = ((display_bounds.size.width.as_f32() - gap - 64.0) / 2.0).min(640.0);
    let height = (display_bounds.size.height.as_f32() - 80.0).min(520.0);
    ensure!(
        width >= 420.0 && height >= 360.0,
        "shared fixture display is too small"
    );
    let total_width = width * 2.0 + gap;
    let left =
        display_bounds.origin.x + px((display_bounds.size.width.as_f32() - total_width) / 2.0);
    let top = display_bounds.origin.y + px((display_bounds.size.height.as_f32() - height) / 2.0);
    Ok(vec![
        open_gpui::Bounds::new(
            open_gpui::point(left, top),
            open_gpui::size(px(width), px(height)),
        ),
        open_gpui::Bounds::new(
            open_gpui::point(left + px(width + gap), top),
            open_gpui::size(px(width), px(height)),
        ),
    ])
}

pub(super) fn three_window_bounds(
    cx: &mut App,
) -> Result<Vec<open_gpui::Bounds<open_gpui::Pixels>>> {
    let display = cx
        .primary_display()
        .context("shared fixture needs a primary display")?;
    let display_bounds = display.bounds();
    let gap = 24.0;
    let width = ((display_bounds.size.width.as_f32() - 2.0 * gap - 96.0) / 3.0).min(480.0);
    let height = (display_bounds.size.height.as_f32() - 80.0).min(520.0);
    ensure!(
        width >= 300.0 && height >= 360.0,
        "shared three-window display is too small"
    );
    let total_width = width * 3.0 + gap * 2.0;
    let left =
        display_bounds.origin.x + px((display_bounds.size.width.as_f32() - total_width) / 2.0);
    let top = display_bounds.origin.y + px((display_bounds.size.height.as_f32() - height) / 2.0);
    Ok((0..3)
        .map(|index| {
            open_gpui::Bounds::new(
                open_gpui::point(left + px(index as f32 * (width + gap)), top),
                open_gpui::size(px(width), px(height)),
            )
        })
        .collect())
}

pub(super) fn is_shared_window_live(window: SharedWindow) -> bool {
    unsafe { IsWindow(Some(window.hwnd)).as_bool() }
}

pub(super) fn shared_surface_contains_item(
    session: &SharedSession,
    item: ItemId,
    surface: SurfaceId,
) -> bool {
    session.dock.with_view(|view| {
        view.item(item)
            .is_some_and(|item_view| item_view.surface() == surface)
    })
}

pub(super) fn shared_item_selected(session: &SharedSession, item: ItemId) -> bool {
    session
        .dock
        .with_view(|view| view.item(item).is_some_and(|item| item.is_selected()))
}
