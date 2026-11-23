//! Output backpressure preserves the displayed cohort while queued input commits.

use std::{convert::Infallible, num::NonZeroUsize, time::Duration};

use dockspace::runtime::{PaintedSurfaceOutput, SurfacePresentationResult};
use open_gpui::{
    AnyWindowHandle, AvailableSpace, PlatformFrameDeliveryMode, StyleRefinement,
    WindowFrameSubmissionOutcome, WindowFrameSubmissionTicket, canvas,
};

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
        div().size_full().child(
            self.dock
                .regions(regions(&self.surfaces, self.bounds.get())),
        )
    }
}

fn regions(surfaces: &[SurfaceId], bounds: Option<LogicalRect>) -> Vec<DockRegion> {
    if let Some(bounds) = bounds {
        vec![DockRegion::new(surfaces[0], bounds)]
    } else if surfaces.len() == 1 {
        vec![DockRegion::fill(surfaces[0])]
    } else {
        surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| {
                DockRegion::new(
                    *surface,
                    LogicalRect::new(index as f64 * 400.0, 0.0, 400.0, 600.0).unwrap(),
                )
            })
            .collect()
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
    let dock = capacity_workspace(cx, if cohort { 3 } else { 2 });
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

fn capacity_workspace(cx: &mut TestAppContext, capacity: usize) -> DockSurface {
    cx.update(|cx| {
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
            .configure_output_capacity(NonZeroUsize::new(capacity).unwrap())
            .unwrap();
        DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    })
}

struct MutableMount {
    handle: AnyWindowHandle,
    regions: Rc<Cell<&'static [SurfaceId]>>,
    visual: VisualTestContext,
}

struct MutableMountRoot {
    dock: DockSurface,
    regions: Rc<Cell<&'static [SurfaceId]>>,
}

impl Render for MutableMountRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let surfaces = self.regions.get();
        div().size_full().when(!surfaces.is_empty(), |root| {
            root.child(self.dock.regions(regions(surfaces, None)))
        })
    }
}

impl MutableMount {
    fn open(cx: &mut TestAppContext, dock: &DockSurface, regions: &'static [SurfaceId]) -> Self {
        let regions = Rc::new(Cell::new(regions));
        cx.defer_next_window_frame_requests();
        let handle = cx
            .open_window(size(px(800.0), px(600.0)), |_, _| MutableMountRoot {
                dock: dock.clone(),
                regions: regions.clone(),
            })
            .into();
        Self {
            handle,
            regions,
            visual: VisualTestContext::from_window(handle, cx),
        }
    }

    fn drain(&mut self) {
        for _ in 0..16 {
            self.visual
                .background_executor
                .advance_clock(Duration::from_millis(512));
            let requested = self
                .visual
                .cx
                .step_deferred_window_frame_request(self.handle);
            let scheduled = self.visual.simulate_scheduled_frame();
            if !requested && !scheduled {
                return;
            }
        }
        panic!("the physical mount did not reach an idle frame schedule");
    }
}

#[open_gpui::test]
fn output_capacity_release_wakes_a_first_mount(cx: &mut TestAppContext) {
    let dock = capacity_workspace(cx, 1);
    let mut source = MutableMount::open(cx, &dock, &[FIRST]);
    let mut target = MutableMount::open(cx, &dock, &[]);
    frames(&mut source.visual, &dock);
    frames(&mut target.visual, &dock);
    cx.set_window_present_outcome(source.handle, PlatformWindowPresentOutcome::Deferred);
    cx.simulate_window_resize(source.handle, size(px(780.0), px(580.0)));
    frames(&mut source.visual, &dock);
    let ticket = dock.host.borrow().mounts[&source.handle.window_id()]
        .pending
        .as_ref()
        .unwrap()
        .ticket
        .clone();
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Accepted
    );
    cx.update(|app| {
        dock.update(
            |frame| {
                for item in [A, C] {
                    frame.dock_item_current(item, DockPlacement::Center(DockAnchor::Item(B)))?;
                }
                frame.select_item_current(B)?;
                Ok(())
            },
            app,
        )
        .unwrap();
    });

    target.regions.set(&[SECOND]);
    frames(&mut target.visual, &dock);
    target.drain();
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target.handle.window_id()];
        assert!(!mount.attached);
        assert!(mount.capacity_blocked);
        assert!(mount.pending.is_none());
    }

    cx.set_window_present_outcome(source.handle, PlatformWindowPresentOutcome::Submitted);
    frames(&mut source.visual, &dock);
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Submitted
    );
    target.drain();
    assert_painted_pane(&dock, target.handle, SECOND, B);
}

#[open_gpui::test]
fn a_rejected_first_mount_wakes_a_waiting_cohort(cx: &mut TestAppContext) {
    let dock = capacity_workspace(cx, 8);
    let mut claimant = MutableMount::open(cx, &dock, &[FIRST, SECOND]);
    let mut trial = MutableMount::open(cx, &dock, &[]);
    frames(&mut claimant.visual, &dock);
    frames(&mut trial.visual, &dock);
    claimant.regions.set(&[]);
    frames(&mut claimant.visual, &dock);

    cx.set_window_frame_delivery_mode(trial.handle, PlatformFrameDeliveryMode::Deferred);
    trial.regions.set(&[FIRST]);
    assert!(trial.visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    let receipt = cx.pending_window_frame_submission(trial.handle).unwrap();
    assert!(dock.is_update_pending());
    claimant.regions.set(&[FIRST, SECOND]);
    frames(&mut claimant.visual, &dock);
    claimant.drain();
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&claimant.handle.window_id()];
        assert!(!mount.attached);
        assert!(mount.waiting_for_update);
        assert!(host.owners.is_empty());
    }

    trial.regions.set(&[]);
    assert!(receipt.complete(PlatformWindowPresentOutcome::Rejected));
    cx.run_until_parked();
    cx.set_window_frame_delivery_mode(trial.handle, PlatformFrameDeliveryMode::Synchronous);
    frames(&mut trial.visual, &dock);
    claimant.drain();
    assert_painted_pane(&dock, claimant.handle, FIRST, A);
    assert_painted_pane(&dock, claimant.handle, SECOND, B);
}

#[open_gpui::test]
fn a_pruned_region_can_remount_with_a_live_sibling(cx: &mut TestAppContext) {
    let dock = capacity_workspace(cx, 8);
    dock.host
        .borrow_mut()
        .panels
        .get_mut(&B)
        .unwrap()
        .tab_content = Some(Rc::new(|_, _| div().w(px(60.0)).into_any_element()));
    let mut mount = MutableMount::open(cx, &dock, &[FIRST, SECOND]);
    frames(&mut mount.visual, &dock);
    cx.update(|app| {
        dock.update(
            |frame| {
                for item in [A, C] {
                    frame.dock_item_current(item, DockPlacement::Center(DockAnchor::Item(B)))?;
                }
                frame.select_item_current(B)?;
                Ok(())
            },
            app,
        )
        .unwrap();
    });
    assert!(dock.with_view(|view| view.surface(FIRST).is_none()));
    mount.regions.set(&[]);
    frames(&mut mount.visual, &dock);
    mount.regions.set(&[FIRST, SECOND]);
    frames(&mut mount.visual, &dock);
    assert_painted_pane(&dock, mount.handle, SECOND, B);
    let host = dock.host.borrow();
    assert!(!host.owners.contains_key(&FIRST));
    let mount = &host.mounts[&mount.handle.window_id()];
    assert!(
        mount
            .input_frames
            .iter()
            .all(|frame| frame.surface != FIRST)
    );
    assert_eq!(
        mount.regions[&SECOND].layout_bounds().unwrap().origin.x,
        px(400.0)
    );
}

struct RetriedMount {
    dock: DockSurface,
    show: Rc<Cell<bool>>,
    trial: bool,
    chosen: SurfaceId,
}

impl Render for RetriedMount {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let dock = self.dock.clone();
        let show = self.show.get();
        let trial = show && std::mem::take(&mut self.trial);
        let chosen = self.chosen;
        canvas(
            move |bounds, window, cx| {
                if !show {
                    return None;
                }
                let space = size(
                    AvailableSpace::Definite(bounds.size.width),
                    AvailableSpace::Definite(bounds.size.height),
                );
                if trial {
                    let mut rejected = dock.element(FIRST).into_any_element();
                    let result = window.transact(|window| {
                        rejected.prepaint_as_root(bounds.origin, space, window, cx);
                        assert!(dock.is_update_pending());
                        Err::<(), _>(())
                    });
                    assert_eq!(result, Err(()));
                }
                let mut accepted = dock.element(chosen).into_any_element();
                accepted.prepaint_as_root(bounds.origin, space, window, cx);
                Some(accepted)
            },
            |_, accepted, window, cx| {
                if let Some(mut accepted) = accepted {
                    accepted.paint(window, cx);
                }
            },
        )
        .size_full()
    }
}

#[open_gpui::test]
fn a_rolled_back_first_mount_allows_a_same_frame_retry(cx: &mut TestAppContext) {
    for (surface, item) in [(FIRST, A), (SECOND, B)] {
        let dock = capacity_workspace(cx, 8);
        let show = Rc::new(Cell::new(false));
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| RetriedMount {
            dock: dock.clone(),
            show: show.clone(),
            trial: true,
            chosen: surface,
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        frames(&mut visual, &dock);
        show.set(true);
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.error().is_none(), "{:?}", dock.error());
        assert!(!dock.is_update_pending());
        assert_painted_pane(&dock, window.into(), surface, item);
        assert_eq!(dock.host.borrow().owners.len(), 1);
    }
}

struct CachedMountRoot {
    dock: DockSurface,
    child: Entity<CountedRoot>,
    trial: bool,
    recorded: Rc<Cell<bool>>,
}

impl Render for CachedMountRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let trial = std::mem::take(&mut self.trial);
        let child = self.child.clone();
        let dock = self.dock.clone();
        let recorded = self.recorded.clone();
        div()
            .size_full()
            .child(
                canvas(
                    move |_, window, cx| {
                        if trial {
                            let mut measured = child.into_any_element();
                            let result = window.transact(|window| {
                                measured.prepaint_as_root(
                                    Default::default(),
                                    size(
                                        AvailableSpace::Definite(px(360.0)),
                                        AvailableSpace::Definite(px(210.0)),
                                    ),
                                    window,
                                    cx,
                                );
                                recorded.set(dock.is_update_pending());
                                Err::<(), _>(())
                            });
                            assert_eq!(result, Err(()));
                        }
                    },
                    |_, (), _, _| {},
                )
                .size(px(4.0)),
            )
            .child(
                self.child
                    .clone()
                    .cached(StyleRefinement::default().w(px(610.0)).h(px(360.0))),
            )
    }
}

#[open_gpui::test]
fn a_cached_docking_frame_releases_the_rolled_back_update(cx: &mut TestAppContext) {
    let dock = capacity_workspace(cx, 8);
    let renders = Rc::new(Cell::new(0));
    let recorded = Rc::new(Cell::new(false));
    let child = cx.new(|_| CountedRoot {
        dock: dock.clone(),
        surface: FIRST,
        renders: renders.clone(),
    });
    cx.defer_next_window_frame_requests();
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| CachedMountRoot {
        dock: dock.clone(),
        child,
        trial: false,
        recorded: recorded.clone(),
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    drain_scheduled(&mut visual, &dock, "cached baseline");
    let handle = AnyWindowHandle::from(window);
    cx.update(|app| {
        window
            .update(app, |root, window, cx| {
                let mut host = dock.host.borrow_mut();
                let action = {
                    let mut frame = host.session.begin_host_frame().unwrap();
                    frame
                        .paint_plan(FIRST)
                        .unwrap()
                        .unwrap()
                        .prepare_tab_focus(C)
                        .unwrap()
                        .into_deferred()
                };
                input::queue_action(&mut host, window.window_handle().window_id(), action, cx);
                root.trial = true;
                cx.notify();
            })
            .unwrap();
        handle
            .update(app, |_, window, cx| {
                let baseline_renders = renders.get();
                let revision = window.rendered_frame_revision();
                window.draw(cx).clear(cx);
                assert_eq!(window.rendered_frame_revision(), revision + 1);
                assert!(recorded.get(), "the trial must suspend a real update");
                assert_eq!(
                    renders.get(),
                    baseline_renders + 1,
                    "only the trial renders"
                );
                assert_painted_pane(&dock, handle, FIRST, A);
            })
            .unwrap();
        let dock = dock.clone();
        app.defer(move |_| {
            assert!(!dock.is_update_pending());
            assert_eq!(dock.inspection().unwrap().queued_input_count, 1);
        });
    });
    frames(&mut visual, &dock);
    assert_eq!(dock.inspection().unwrap().queued_input_count, 0);
    assert_painted_pane(&dock, handle, FIRST, C);
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
