//! Output backpressure preserves the displayed cohort while queued input commits.

use std::{convert::Infallible, num::NonZeroUsize, time::Duration};

use dockspace::runtime::{PaintedSurfaceOutput, SurfacePresentationResult};
use open_gpui::{AnyWindowHandle, WindowFrameSubmissionOutcome, WindowFrameSubmissionTicket};

use super::*;
use crate::adapter::{HostState, host_native, input};

const THIRD: SurfaceId = SurfaceId::new(13);
const C: ItemId = ItemId::new(23);
const D: ItemId = ItemId::new(24);

struct CapacityRoot {
    dock: DockSurface,
    surfaces: Vec<SurfaceId>,
    bounds: Rc<Cell<Option<LogicalRect>>>,
}

impl Render for CapacityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let regions = if let Some(bounds) = self.bounds.get() {
            vec![DockRegion::new(self.surfaces[0], bounds)]
        } else if self.surfaces.len() == 1 {
            vec![DockRegion::fill(self.surfaces[0])]
        } else {
            self.surfaces
                .iter()
                .enumerate()
                .map(|(index, surface)| {
                    DockRegion::new(
                        *surface,
                        LogicalRect::new(index as f64 * 400.0, 0.0, 400.0, 600.0).unwrap(),
                    )
                })
                .collect()
        };
        div().size_full().child(self.dock.regions(regions))
    }
}

fn fixture(cx: &mut TestAppContext, cohort: bool) -> (DockSurface, [AnyWindowHandle; 2]) {
    fixture_with_bounds(cx, cohort, Rc::new(Cell::new(None)))
}

fn fixture_with_bounds(
    cx: &mut TestAppContext,
    cohort: bool,
    bounds: Rc<Cell<Option<LogicalRect>>>,
) -> (DockSurface, [AnyWindowHandle; 2]) {
    let dock = cx.update(|cx| {
        let document = Rc::new(RefCell::new(Document {
            text: "shared".into(),
        }));
        let panes = [A, B, C, D].map(|item| {
            let view = cx.new(|_| View {
                document: document.clone(),
                item,
                clicks: 0,
            });
            DockPane::new(item, format!("View {}", item.get()), view)
        });
        let layout = DockspaceLayout::new(
            [(FIRST, vec![A, C]), (SECOND, vec![B]), (THIRD, vec![D])].map(|(surface, items)| {
                DockspaceSurfaceLayout::new(
                    surface,
                    DockspaceRootLayout::new(
                        RootId::new(surface.get()),
                        DockspaceNode::tabs(items),
                    ),
                )
            }),
        )
        .unwrap();
        let mut session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        session
            .configure_output_capacity(NonZeroUsize::new(if cohort { 3 } else { 2 }).unwrap())
            .unwrap();
        DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    });
    cx.defer_next_window_frame_requests();
    let target = cx.open_window(size(px(800.0), px(600.0)), |_, _| CapacityRoot {
        dock: dock.clone(),
        bounds,
        surfaces: if cohort {
            vec![FIRST, SECOND]
        } else {
            vec![FIRST]
        },
    });
    cx.defer_next_window_frame_requests();
    let sibling = cx.open_window(size(px(800.0), px(600.0)), |_, _| CountedRoot {
        dock: dock.clone(),
        surface: THIRD,
        renders: Rc::new(Cell::new(0)),
    });
    let windows = [target.into(), sibling.into()];
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    let mut target = VisualTestContext::from_window(windows[0], cx);
    target.update(|window, _| {
        let _ = window.activate_window();
    });
    target.run_until_parked();
    assert!(
        target.update(|window, _| window.is_window_active()),
        "local docking pointer capture needs the target window to be active"
    );
    (dock, windows)
}

#[open_gpui::test]
fn moving_a_capacity_blocked_region_uses_its_accepted_input_coordinates(cx: &mut TestAppContext) {
    let bounds = Rc::new(Cell::new(Some(
        LogicalRect::new(0.0, 0.0, 300.0, 600.0).unwrap(),
    )));
    let (dock, [target_window, sibling_window]) = fixture_with_bounds(cx, false, bounds.clone());
    let mut target = VisualTestContext::from_window(target_window, cx);
    let mut sibling = VisualTestContext::from_window(sibling_window, cx);
    let (held, ticket) = occupy_sibling(&dock, &mut sibling, sibling_window, THIRD);
    {
        let mut host = dock.host.borrow_mut();
        host.mounts
            .get_mut(&target_window.window_id())
            .unwrap()
            .regions
            .get_mut(&FIRST)
            .unwrap()
            .dirty = true;
    }
    frames(&mut target, &dock);
    let (serial, packet) = {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target_window.window_id()];
        assert!(mount.capacity_blocked);
        (mount.serial, mount.regions[&FIRST].packet.clone().unwrap())
    };
    bounds.set(Some(LogicalRect::new(400.0, 0.0, 300.0, 600.0).unwrap()));
    target.update(|window, _| window.refresh());
    frames(&mut target, &dock);
    let tab = target.debug_bounds("dockspace:tab:23").unwrap();
    assert!(tab.origin.x >= px(400.0));
    target.simulate_click(tab.center(), Modifiers::none());
    frames(&mut target, &dock);
    drain_scheduled(&mut target, &dock, "translated region at full capacity");
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target_window.window_id()];
        assert!(host.session.view().item(C).unwrap().is_active());
        assert!(host.inputs.is_empty());
        assert_eq!(mount.serial, serial);
        assert!(Rc::ptr_eq(
            mount.regions[&FIRST].packet.as_ref().unwrap(),
            &packet
        ));
        assert!(mount.pending.is_none());
    }
    dock.host
        .borrow_mut()
        .session
        .report_surface_presentation(held, SurfacePresentationResult::Dropped)
        .unwrap();
    sibling.set_window_present_outcome(sibling_window, PlatformWindowPresentOutcome::Submitted);
    frames(&mut sibling, &dock);
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Submitted
    );
    drain_scheduled(
        &mut target,
        &dock,
        "translated region after capacity release",
    );
    assert_painted_pane(&dock, target_window, FIRST, C);
}

fn occupy_sibling(
    dock: &DockSurface,
    sibling: &mut VisualTestContext,
    window: AnyWindowHandle,
    surface: SurfaceId,
) -> (PaintedSurfaceOutput, WindowFrameSubmissionTicket) {
    // Hold an earlier complete core emission from this same sibling stream.
    // Its next real GPUI submission then occupies a second bounded output slot.
    let held = sibling.update(|window, cx| {
        let mut host = dock.host.borrow_mut();
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame =
            host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx).unwrap();
        let receipt = frame
            .paint_plan(surface)
            .unwrap()
            .unwrap()
            .execute_surface_program(|_| Ok::<_, Infallible>(()))
            .unwrap();
        frame.confirm_surface_painted(receipt).unwrap();
        let mut report = frame.commit().unwrap();
        let [output] = report.take_painted_outputs().try_into().unwrap();
        host.apply_report(report);
        output
    });
    sibling.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&window.window_id())
        .unwrap()
        .regions
        .get_mut(&surface)
        .unwrap()
        .dirty = true;
    frames(sibling, dock);
    let host = dock.host.borrow();
    let pending = host.mounts[&window.window_id()].pending.as_ref().unwrap();
    assert!(pending.update.is_none());
    assert_eq!(
        pending.ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Accepted
    );
    (held, pending.ticket.clone())
}

fn drain_scheduled(visual: &mut VisualTestContext, dock: &DockSurface, phase: &str) {
    for _ in 0..16 {
        // Inactive-window throttling and the high-rate input hold are real
        // scheduler deadlines, not repeated docking refreshes. Let them expire.
        visual
            .background_executor
            .advance_clock(Duration::from_millis(512));
        if !visual.simulate_scheduled_frame() {
            break;
        }
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }
    assert!(
        !visual.frame_scheduled(),
        "{phase}: scheduled frames must park without repeated docking refreshes"
    );
}

#[open_gpui::test]
fn full_capacity_commits_pointer_selection_and_preserves_the_displayed_input_identity(
    cx: &mut TestAppContext,
) {
    let (dock, windows) = fixture(cx, false);
    let [target_window, sibling_window] = windows;
    let mut target = VisualTestContext::from_window(target_window, cx);
    let mut sibling = VisualTestContext::from_window(sibling_window, cx);
    let (held, ticket) = occupy_sibling(&dock, &mut sibling, sibling_window, THIRD);
    let (serial, packet, presented) = {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target_window.window_id()];
        let region = &mount.regions[&FIRST];
        (
            mount.serial,
            region.packet.clone().unwrap(),
            region.presented,
        )
    };
    for item in [C, A, C] {
        let tab = target
            .debug_bounds(&format!("dockspace:tab:{}", item.get()))
            .unwrap();
        target.simulate_click(tab.center(), Modifiers::none());
        frames(&mut target, &dock);
        drain_scheduled(&mut target, &dock, "pointer input at full capacity");
        let host = dock.host.borrow();
        assert!(
            host.session.view().item(item).unwrap().is_active(),
            "the queued click must select {item:?} while output remains blocked"
        );
        assert!(
            host.inputs.is_empty(),
            "accepted pointer input must not replay"
        );
        assert!(!host.session.has_active_gesture());
        assert!(!host.has_unaccepted_update());
        let mount = &host.mounts[&target_window.window_id()];
        assert!(
            mount.pending.is_none(),
            "no output means no new docking submission"
        );
        assert_eq!(mount.serial, serial);
        assert!(Rc::ptr_eq(
            mount.regions[&FIRST].packet.as_ref().unwrap(),
            &packet
        ));
        assert_eq!(mount.regions[&FIRST].presented, presented);
        assert!(input::is_presented(
            &host,
            target_window.window_id(),
            FIRST,
            serial
        ));
        assert!(
            mount
                .input_frames
                .iter()
                .all(|frame| frame.serial == serial)
        );
        assert_eq!(
            ticket.snapshot().outcome(),
            WindowFrameSubmissionOutcome::Accepted
        );
        drop(host);
        assert!(
            target.debug_bounds("view:23").is_none(),
            "unpainted content is not installed"
        );
    }
    dock.host
        .borrow_mut()
        .session
        .report_surface_presentation(held, SurfacePresentationResult::Dropped)
        .unwrap();
    sibling.set_window_present_outcome(sibling_window, PlatformWindowPresentOutcome::Submitted);
    frames(&mut sibling, &dock);
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Submitted
    );
    // Only the sibling gets an explicit frame. The target must be woken by its terminal.
    drain_scheduled(&mut target, &dock, "capacity released after pointer input");
    assert_painted_pane(&dock, target_window, FIRST, C);
    assert!(target.debug_bounds("view:23").is_some());
}

#[open_gpui::test]
fn later_region_capacity_restarts_the_whole_physical_cohort(cx: &mut TestAppContext) {
    let (dock, windows) = fixture(cx, true);
    let [target_window, sibling_window] = windows;
    let mut target = VisualTestContext::from_window(target_window, cx);
    let mut sibling = VisualTestContext::from_window(sibling_window, cx);
    let (held, ticket) = occupy_sibling(&dock, &mut sibling, sibling_window, THIRD);
    let (serial, originals) = {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target_window.window_id()];
        (
            mount.serial,
            [FIRST, SECOND].map(|surface| {
                let region = &mount.regions[&surface];
                (region.packet.clone().unwrap(), region.presented)
            }),
        )
    };
    let tab = target.debug_bounds("dockspace:tab:23").unwrap();
    target.simulate_click(tab.center(), Modifiers::none());
    frames(&mut target, &dock);
    drain_scheduled(&mut target, &dock, "partial cohort at full capacity");
    {
        let host = dock.host.borrow();
        assert!(host.session.view().item(C).unwrap().is_active());
        assert!(host.inputs.is_empty());
        assert!(!host.has_unaccepted_update());
        let mount = &host.mounts[&target_window.window_id()];
        assert!(mount.pending.is_none());
        assert_eq!(mount.serial, serial);
        for (surface, (packet, presented)) in [FIRST, SECOND].into_iter().zip(&originals) {
            let region = &mount.regions[&surface];
            assert!(Rc::ptr_eq(region.packet.as_ref().unwrap(), packet));
            assert_eq!(region.presented, *presented);
            assert!(
                region.dirty,
                "the unpainted cohort still needs a replacement"
            );
        }
    }
    dock.host
        .borrow_mut()
        .session
        .report_surface_presentation(held, SurfacePresentationResult::Dropped)
        .unwrap();
    sibling.set_window_present_outcome(sibling_window, PlatformWindowPresentOutcome::Submitted);
    frames(&mut sibling, &dock);
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Submitted
    );
    drain_scheduled(&mut target, &dock, "capacity released for whole cohort");
    for (surface, item) in [(FIRST, C), (SECOND, B)] {
        assert_painted_pane(&dock, target_window, surface, item);
    }
    let host = dock.host.borrow();
    let mount = &host.mounts[&target_window.window_id()];
    assert_eq!(
        mount.regions[&FIRST].presented.unwrap().0,
        mount.regions[&SECOND].presented.unwrap().0
    );
}

#[open_gpui::test]
fn managed_capacity_probes_park_with_the_original_native_input_binding(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _, _, windows) = managed_fixture(cx);
    let [target_window, sibling_window] = windows;
    let mut target = VisualTestContext::from_window(target_window, cx);
    let mut sibling = VisualTestContext::from_window(sibling_window, cx);
    frames(&mut target, &dock);
    frames(&mut sibling, &dock);
    dock.host
        .borrow_mut()
        .session
        .configure_output_capacity(NonZeroUsize::new(2).unwrap())
        .unwrap();
    let (held, ticket) = occupy_sibling(&dock, &mut sibling, sibling_window, SECOND);
    let (serial, packet, presented, binding) = {
        let mut host = dock.host.borrow_mut();
        let mount = host.mounts.get_mut(&target_window.window_id()).unwrap();
        let region = mount.regions.get_mut(&FIRST).unwrap();
        region.dirty = true;
        (
            mount.serial,
            region.packet.clone().unwrap(),
            region.presented,
            mount
                .input_frames
                .iter()
                .find(|frame| frame.surface == FIRST)
                .unwrap()
                .native_binding,
        )
    };
    assert!(
        binding.is_some(),
        "the original artifact has an exact native binding"
    );
    frames(&mut target, &dock);
    drain_scheduled(&mut target, &dock, "managed host at full capacity");
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target_window.window_id()];
        assert_eq!(mount.serial, serial);
        assert!(mount.pending.is_none());
        assert!(Rc::ptr_eq(
            mount.regions[&FIRST].packet.as_ref().unwrap(),
            &packet
        ));
        assert_eq!(mount.regions[&FIRST].presented, presented);
        assert!(mount.input_frames.iter().any(|frame| {
            frame.surface == FIRST && frame.serial == serial && frame.native_binding == binding
        }));
        assert!(input::is_presented(
            &host,
            target_window.window_id(),
            FIRST,
            serial
        ));
    }
    dock.host
        .borrow_mut()
        .session
        .report_surface_presentation(held, SurfacePresentationResult::Dropped)
        .unwrap();
    sibling.set_window_present_outcome(sibling_window, PlatformWindowPresentOutcome::Submitted);
    frames(&mut sibling, &dock);
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Submitted
    );
    drain_scheduled(&mut target, &dock, "capacity released for managed host");
    assert_painted_pane(&dock, target_window, FIRST, A);
    assert!(dock.host.borrow().mounts[&target_window.window_id()].serial > serial);
}
