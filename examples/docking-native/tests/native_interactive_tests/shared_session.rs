//! Shared managed native-session fixture for Windows interactive journeys.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use anyhow::{Context as _, Result, bail, ensure};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Context, Empty, Entity, IntoElement,
    NativeCapturedDragEvent, NativeCapturedDragGeneration, NativeCapturedDragPhase,
    NativeCapturedDragReleaseTerminal, Pixels, PlatformWindowPhysicalGeometry, Point, Render,
    Window, WindowId, div, prelude::*, px,
};
use open_gpui_docking::{
    DockPane, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        model::{
            DockspaceAxis, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
            DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::{DockspaceSession, NativeWindowBinding},
    },
    native::NativeReport,
};
use open_gpui_windows::{
    NativeTestDisplay, NativeTestPointerAction, NativeTestSystemPointerGuard,
    NativeWindowTestEventKind, NativeWindowTestIdentity, NativeWindowTestMessage,
    NativeWindowTestObservation, native_test_acquire_foreground_window,
    native_test_logical_client_point_to_screen, native_test_non_shell_root_window_at,
    native_test_window_rect,
};
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, VK_LBUTTON},
        WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, IsWindowVisible, WM_NCMOUSEMOVE},
    },
};

use super::process_harness;
use super::{
    native_generation_label, native_system_mouse_message_observed,
    native_system_mouse_message_observed_for_admission, raw_hwnd,
    unique_native_identity_for_window, wait_for_cursor, wait_until, window_has_non_empty_frame,
    window_scale_factor,
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

pub(super) struct SharedRootSnapshot {
    owner: SharedWindow,
    root: RootId,
    binding: NativeWindowBinding,
    geometry: PlatformWindowPhysicalGeometry,
}

pub(super) struct SharedCapturedDrag {
    pub(super) generation: NativeCapturedDragGeneration,
    pub(super) source: SharedWindow,
}

pub(super) struct SharedSession {
    pub(super) dock: DockSurface,
    pub(super) source_view: Option<Entity<SharedSourceView>>,
    pub(super) windows: Vec<SharedWindow>,
    pub(super) observation: NativeWindowTestObservation,
    pub(super) reports: Vec<NativeReport>,
    pub(super) drag_events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    pub(super) pointer: Option<NativeTestSystemPointerGuard>,
    pub(super) preexisting_window_count: usize,
    pub(super) milestones: Vec<String>,
}

#[derive(Default)]
pub(super) struct SharedSourceView {
    pub(super) local_edits: u64,
    pub(super) last_render: Option<(WindowId, u64)>,
}

impl Render for SharedSourceView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.last_render = Some((window.window_handle().window_id(), self.local_edits));
        let edits = self.local_edits;
        div()
            .size_full()
            .debug_selector(move || format!("shared-source-view:{edits}"))
            .child(format!("Retained View: {edits}"))
    }
}

struct SharedDragObservation {
    events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    _subscription: open_gpui::Subscription,
}

impl open_gpui::Global for SharedDragObservation {}

impl SharedSession {
    pub(super) fn build(
        cx: &mut App,
        observation: NativeWindowTestObservation,
        shape: SharedFixtureShape,
        bounds: &[open_gpui::Bounds<open_gpui::Pixels>],
    ) -> Result<Self> {
        let display_id = cx
            .primary_display()
            .context("shared fixture needs a primary display")?
            .id();
        Self::build_on_display(cx, observation, shape, bounds, display_id)
    }

    pub(super) fn build_on_display(
        cx: &mut App,
        observation: NativeWindowTestObservation,
        shape: SharedFixtureShape,
        bounds: &[open_gpui::Bounds<open_gpui::Pixels>],
        display_id: open_gpui::DisplayId,
    ) -> Result<Self> {
        ensure!(
            cx.find_display(display_id).is_some(),
            "shared fixture could not resolve selected display {display_id:?}"
        );
        let mut source_view = None;
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
                let view = cx.new(|_| SharedSourceView::default());
                source_view = Some(view.clone());
                (
                    layout,
                    vec![
                        DockPane::new(SHARED_SOURCE_ITEM, "Shared source", view),
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
                let view = cx.new(|_| SharedSourceView::default());
                source_view = Some(view.clone());
                (
                    layout,
                    vec![
                        DockPane::new(SHARED_SOURCE_ITEM, "Shared source", view),
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
        let preexisting_window_count = cx.windows().len();
        if !cx.has_global::<SharedDragObservation>() {
            let events = Rc::new(RefCell::new(Vec::new()));
            let observed = events.clone();
            let subscription = cx.observe_native_captured_drag(move |event, _| {
                observed.borrow_mut().push(event);
            });
            cx.set_global(SharedDragObservation {
                events,
                _subscription: subscription,
            });
        }
        let drag_events = cx.global::<SharedDragObservation>().events.clone();
        let mut session = Self {
            dock,
            source_view,
            windows: Vec::new(),
            observation,
            reports: Vec::new(),
            drag_events,
            pointer: None,
            preexisting_window_count,
            milestones: Vec::new(),
        };
        for (index, (surface, item, bounds)) in surfaces.into_iter().enumerate() {
            let mut options = super::viewport_window_options(bounds);
            options.display_id = Some(display_id);
            options.focus_on_appearing = index == 0;
            let anchor =
                shape == SharedFixtureShape::SurvivorWindow || surface == SHARED_TARGET_SURFACE;
            let window = session
                .dock
                .open_native_root(surface, anchor, options, cx)
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
            "shared managed roots to present their exact docking contents",
            |cx| {
                self.pump(cx);
                self.check_health()?;
                let inspection = self.dock.inspection()?;
                cx.update(|app| {
                    for window in &self.windows {
                        let presented = inspection
                            .mounts
                            .iter()
                            .find(|mount| mount.window_id == window.window.window_id().as_u64())
                            .and_then(|mount| {
                                mount
                                    .regions
                                    .iter()
                                    .find(|region| region.surface == window.surface)
                            })
                            .is_some_and(|region| {
                                region.packet_deferred == Some(false) && region.current_presentation
                            });
                        if !presented
                            || !window_has_non_empty_frame(app, window.window)?
                            || !unsafe { IsWindowVisible(window.hwnd).as_bool() }
                        {
                            return Ok(false);
                        }
                        let selector = format!("dockspace:tab:{}", window.item.get());
                        let matches = app.update_window(window.window, |_, window, _| {
                            window
                                .committed_debug_bounds_for_test()
                                .into_iter()
                                .filter(|(candidate, _)| candidate == &selector)
                                .collect::<Vec<_>>()
                        })?;
                        ensure!(
                            matches.len() <= 1,
                            "shared root has ambiguous committed selector `{selector}`: {matches:?}"
                        );
                        if !matches.first().is_some_and(|(_, bounds)| {
                            bounds.size.width > px(0.0) && bounds.size.height > px(0.0)
                        }) {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                })
            },
        )
        .await
        .with_context(|| format!("shared root presentation: {:?}", self.dock.inspection()))?;
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

    pub(super) fn assert_roots_on_display(
        &self,
        cx: &mut App,
        display: NativeTestDisplay,
        bounds: &[open_gpui::Bounds<open_gpui::Pixels>],
    ) -> Result<()> {
        ensure!(
            self.windows.len() == bounds.len(),
            "shared fixture root bounds do not match its windows"
        );
        for (window, bounds) in self.windows.iter().zip(bounds) {
            let expected_bounds = bounds.to_device_pixels(display.scale_factor());
            let facts = super::window_platform_facts(cx, window.window)?;
            let geometry = facts
                .physical_geometry
                .context("shared fixture root has no physical client geometry")?;
            let native_bounds = super::native_client_screen_bounds(window.hwnd)?;
            ensure!(
                facts.display_id == Some(display.display_id())
                    && (facts.scale_factor - display.scale_factor()).abs() <= 0.001
                    && (geometry.scale_factor() - display.scale_factor()).abs() <= 0.001
                    && geometry.client_bounds() == expected_bounds
                    && native_bounds == expected_bounds,
                "shared fixture root diverged from its selected display and requested client bounds: window={window:?}, display={display:?}, expected={expected_bounds:?}, native_client={native_bounds:?}, facts={facts:?}"
            );
        }
        Ok(())
    }

    pub(super) fn snapshot_roots(&self, app: &mut App) -> Result<Vec<SharedRootSnapshot>> {
        let snapshots = self
            .windows
            .iter()
            .map(|owner| {
                let facts = super::window_platform_facts(app, owner.window)?;
                let geometry = facts
                    .physical_geometry
                    .context("original root has no physical geometry")?;
                ensure!(
                    geometry.outer_bounds().is_some() && geometry.display_observation().is_some(),
                    "original root has no coherent native frame/display observation"
                );
                Ok(SharedRootSnapshot {
                    owner: *owner,
                    root: self
                        .dock
                        .with_view(|view| {
                            view.surface(owner.surface)
                                .and_then(|surface| surface.main_root())
                                .map(|root| root.id())
                        })
                        .context("original surface has no main root")?,
                    binding: self
                        .dock
                        .native_binding_for_window(owner.window.window_id())
                        .context("original root has no native binding")?,
                    geometry,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.assert_roots_unchanged(app, &snapshots)?;
        Ok(snapshots)
    }

    pub(super) fn assert_roots_unchanged(
        &self,
        app: &mut App,
        snapshots: &[SharedRootSnapshot],
    ) -> Result<()> {
        let events = self.observation.events();
        for snapshot in snapshots {
            let owner = snapshot.owner;
            let identity = owner
                .identity
                .context("original root has no native identity")?;
            ensure!(
                app.windows().contains(&owner.window)
                    && raw_hwnd(app, owner.window)? == owner.hwnd
                    && unique_native_identity_for_window(
                        &events,
                        owner.window.window_id(),
                        "surviving original root"
                    )? == identity
                    && !events.iter().any(|event| event.window() == identity
                        && matches!(
                            event.kind(),
                            NativeWindowTestEventKind::NativeTerminal { .. }
                        ))
                    && self
                        .dock
                        .native_binding_for_window(owner.window.window_id())
                        == Some(snapshot.binding)
                    && self.dock.with_view(|view| view
                        .surface(owner.surface)
                        .and_then(|surface| surface.main_root())
                        .map(|root| root.id()))
                        == Some(snapshot.root),
                "native placement replaced or retired an original root: {owner:?}"
            );
            let facts = super::window_platform_facts(app, owner.window)?;
            let client = super::native_client_screen_bounds(owner.hwnd)?;
            let outer = native_test_window_rect(owner.hwnd)?;
            let expected_outer = snapshot.geometry.outer_bounds().unwrap();
            ensure!(
                facts.physical_geometry == Some(snapshot.geometry)
                    && facts.display_id
                        == snapshot
                            .geometry
                            .display_observation()
                            .map(|display| display.display_id())
                    && facts.scale_factor == snapshot.geometry.scale_factor()
                    && client == snapshot.geometry.client_bounds()
                    && (outer.left, outer.top, outer.right, outer.bottom)
                        == (
                            expected_outer.origin.x.0,
                            expected_outer.origin.y.0,
                            expected_outer.right().0,
                            expected_outer.bottom().0
                        ),
                "native placement moved or rescaled an original root: owner={owner:?}, original={:?}, facts={facts:?}, client={client:?}, outer={outer:?}",
                snapshot.geometry
            );
        }
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
        let selector = format!("dockspace:tab:{}", item.get());
        let mut bounds = None;
        wait_until(cx, "shared source tab presentation", |cx| {
            self.pump(cx);
            self.check_health()?;
            bounds = cx.update(|app| {
                app.update_window(source.window, |_, window, _| {
                    window
                        .committed_debug_bounds_for_test()
                        .into_iter()
                        .find_map(|(candidate, bounds)| (candidate == selector).then_some(bounds))
                })
            })?;
            Ok(bounds.is_some())
        })
        .await
        .with_context(|| format!("source {:?} did not present {selector}", source.surface))?;
        let bounds = bounds.context("shared source tab presentation lost its bounds")?;
        self.begin_captured_drag_at(cx, source, bounds.center())
            .await
    }

    pub(super) async fn begin_captured_drag_at(
        &mut self,
        cx: &mut AsyncApp,
        source: SharedWindow,
        position: Point<Pixels>,
    ) -> Result<SharedCapturedDrag> {
        ensure!(
            unsafe { GetCapture() } == HWND::default(),
            "shared drag began with native capture"
        );
        if unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 != 0 {
            bail!("shared drag began with the physical primary button pressed");
        }
        // Restoration belongs to the whole journey. Replacing a previous guard
        // after starting the next drag would release that new generation's capture.
        let mut pointer = match self.pointer.take() {
            Some(pointer) => pointer,
            None => NativeTestSystemPointerGuard::capture()?,
        };
        let scale = cx.update(|app| window_scale_factor(app, source.window))?;
        let press_point = native_test_logical_client_point_to_screen(source.hwnd, position, scale)?;
        native_test_acquire_foreground_window(source.hwnd.0 as isize)?;
        let mut ambient_cursor = POINT::default();
        unsafe { GetCursorPos(&mut ambient_cursor) }
            .context("failed to sample the cursor before shared drag admission")?;
        // An absolute SendInput move to the current pixel can omit WM_MOUSEMOVE.
        let threshold_offset = if position.x < px(28.0) || ambient_cursor.x <= press_point.x {
            px(28.0)
        } else {
            px(-28.0)
        };
        let threshold_point = native_test_logical_client_point_to_screen(
            source.hwnd,
            open_gpui::point(position.x + threshold_offset, position.y),
            scale,
        )?;
        for point in [threshold_point, press_point] {
            ensure!(
                native_test_non_shell_root_window_at(point) == Some(source.hwnd),
                "shared drag point is occluded: point={point:?}, source={:?}",
                source.hwnd
            );
            let positioning_context = || {
                let mut cursor = POINT::default();
                let cursor = unsafe { GetCursorPos(&mut cursor) }.ok().map(|_| cursor);
                let messages: Vec<_> = self
                    .observation
                    .events()
                    .into_iter()
                    .rev()
                    .filter(|event| {
                        matches!(
                            event.kind(),
                            NativeWindowTestEventKind::WindowMessage {
                                message: NativeWindowTestMessage::MouseMove
                                    | NativeWindowTestMessage::Other(WM_NCMOUSEMOVE),
                                ..
                            }
                        )
                    })
                    .take(12)
                    .collect();
                format!(
                    "shared hover positioning requested={point:?}, cursor={cursor:?}, source={source:?}, root={:?}, foreground={:?}, messages={messages:?}",
                    native_test_non_shell_root_window_at(point),
                    unsafe { GetForegroundWindow() }
                )
            };
            pointer.inject(point, NativeTestPointerAction::Move)?;
            let delivered = wait_for_cursor(cx, point, "shared source pointer positioning")
                .await
                .with_context(&positioning_context)?;
            // User32 can replace an idle hover with its own untagged move.
            // The button-down below remains the first tagged input proof.
            wait_until(cx, "shared source WndProc hover positioning", |_| {
                Ok(native_system_mouse_message_observed_for_admission(
                    &self.observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered,
                    false,
                ))
            })
            .await
            .with_context(positioning_context)?;
        }
        let event_cursor = self.drag_events.borrow().len();
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
        let activation = wait_until(cx, "shared managed drag activation", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(cx.update(|app| app.has_active_drag())
                && cx.update(|_| self.dock.has_active_gesture())
                && unsafe { GetCapture() } == source.hwnd)
        })
        .await;
        if let Err(error) = activation {
            bail!(
                "{error}; app_drag={}, dock={:?}, capture={:?}, events={:?}",
                cx.update(|app| app.has_active_drag()),
                self.dock.inspection(),
                unsafe { GetCapture() },
                self.drag_events.borrow()
            );
        }
        pointer.inject(press_point, NativeTestPointerAction::Move)?;
        wait_until(cx, "shared current-generation native captured move", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(self.drag_events.borrow()[event_cursor..]
                .iter()
                .any(|event| {
                    event.source_window() == source.window.window_id()
                        && event.phase() == NativeCapturedDragPhase::Moved
                        && event.global_position().is_some_and(|point| {
                            point.x.0 == press_point.x && point.y.0 == press_point.y
                        })
                }))
        })
        .await?;
        let generation = self
            .drag_events
            .borrow()
            .get(event_cursor..)
            .unwrap_or_default()
            .iter()
            .find(|event| {
                event.source_window() == source.window.window_id()
                    && event.phase() == NativeCapturedDragPhase::Moved
            })
            .map(NativeCapturedDragEvent::generation)
            .context("shared managed drag did not publish a native capture generation")?;
        self.pointer = Some(pointer);
        self.milestones.push("shared.drag-captured".to_owned());
        Ok(SharedCapturedDrag { generation, source })
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
            Ok(self.dock.native_is_quiescent())
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
        .with_view(|view| view.item(item).is_some_and(|item| item.is_active()))
}
