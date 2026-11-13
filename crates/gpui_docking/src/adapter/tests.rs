//! Public content integration through GPUI's deterministic test renderer.

#[path = "recovery_tests.rs"]
mod recovery;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dockspace::{
    geometry::LogicalRect,
    model::{
        DockAnchor, DockPlacement, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
        DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::DockspaceSession,
};
use open_gpui::{
    Context, Entity, IntoElement, Modifiers, PlatformWindowPresentOutcome, Render,
    RequestFrameOptions, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size,
};

use super::{DockPane, DockRegion, DockSurface, GpuiDockspaceConfig};

const FIRST: SurfaceId = SurfaceId::new(11);
const SECOND: SurfaceId = SurfaceId::new(12);
const A: ItemId = ItemId::new(21);
const B: ItemId = ItemId::new(22);

struct Document {
    text: String,
}
struct View {
    document: Rc<RefCell<Document>>,
    item: ItemId,
    clicks: usize,
}
impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let item = self.item;
        div().size_full().child(
            div()
                .id("edit")
                .debug_selector(move || format!("view:{}", item.get()))
                .w(px(160.0))
                .h(px(32.0))
                .on_click(cx.listener(|view, _, _, cx| {
                    view.clicks += 1;
                    view.document.borrow_mut().text.push('x');
                    cx.notify();
                }))
                .child(format!("{}:{}", self.document.borrow().text, self.clicks)),
        )
    }
}

struct Root {
    dock: DockSurface,
    surfaces: Vec<SurfaceId>,
}
impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let regions = if self.surfaces.len() == 1 {
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

struct CountedRoot {
    dock: DockSurface,
    surface: SurfaceId,
    renders: Rc<Cell<usize>>,
}

impl Render for CountedRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let renders = self.renders.get() + 1;
        self.renders.set(renders);
        assert!(
            renders <= 32,
            "presentation-only progress continuously requested fresh frames"
        );
        div().size_full().child(self.dock.element(self.surface))
    }
}

fn session() -> DockspaceSession {
    session_with_native(false)
}

fn session_with_native(allow_native: bool) -> DockspaceSession {
    let layout = DockspaceLayout::new([(FIRST, A), (SECOND, B)].map(|(surface, item)| {
        DockspaceSurfaceLayout::new(
            surface,
            DockspaceRootLayout::new(RootId::new(surface.get()), DockspaceNode::tabs([item])),
        )
    }))
    .unwrap();
    let mut policy = DockPolicy::default();
    policy.set_allow_native_surfaces(allow_native);
    DockspaceSession::from_layout(layout, policy).unwrap()
}

fn fixture(cx: &mut TestAppContext) -> (DockSurface, [Entity<View>; 2], Rc<RefCell<Document>>) {
    cx.update(|cx| {
        let document = Rc::new(RefCell::new(Document {
            text: "shared".into(),
        }));
        let views = [A, B].map(|item| {
            cx.new(|_| View {
                document: document.clone(),
                item,
                clicks: 0,
            })
        });
        let dock = DockSurface::new(
            session(),
            [
                DockPane::new(A, "First View", views[0].clone()),
                DockPane::new(B, "Second View", views[1].clone()),
            ],
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap();
        (dock, views, document)
    })
}

fn frames(visual: &mut VisualTestContext, dock: &DockSurface) {
    for _ in 0..4 {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }
}

fn assert_painted_pane(
    dock: &DockSurface,
    window: open_gpui::AnyWindowHandle,
    surface: SurfaceId,
    item: ItemId,
) {
    let host = dock.host.borrow();
    let current = host.session.view().item(item).expect("open pane item");
    assert_eq!(current.surface(), surface);
    assert!(current.is_selected(), "the core selects the content item");
    let region = &host.mounts[&window.window_id()].regions[&surface];
    let packet = region.packet.as_ref().expect("accepted region packet");
    assert!(
        !packet.is_deferred(),
        "native readiness must replace the Bootstrap staging packet"
    );
    assert!(
        packet.pane_items().contains(&item),
        "the core paint program must include the selected pane"
    );
    let (_, artifact) = region
        .presented
        .expect("an actual painted output was submitted");
    assert!(
        host.session
            .presented_surface(surface)
            .is_some_and(|presented| presented.matches_presentation_artifact(artifact)),
        "retained content uses its exact submitted core output"
    );
}

#[open_gpui::test]
fn an_accepted_but_deferred_pane_cannot_receive_content_clicks(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, document) = fixture(cx);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![FIRST],
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let first_bounds = visual.debug_bounds("view:21").unwrap();
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(B, DockPlacement::Center(DockAnchor::Item(A)))?;
                frame.select_item_current(B)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut visual, &dock);
    let second_bounds = visual
        .debug_bounds("view:22")
        .expect("the new pane has visual geometry");
    visual.simulate_click(second_bounds.center(), Modifiers::default());
    visual.simulate_click(first_bounds.center(), Modifiers::default());
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 0);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 0);
    assert_eq!(document.borrow().text, "shared");
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    let submitted_bounds = visual.debug_bounds("view:22").unwrap();
    visual.simulate_click(submitted_bounds.center(), Modifiers::default());
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 1);
    assert_eq!(document.borrow().text, "sharedx");
}

#[open_gpui::test]
fn failed_initial_presentation_retires_its_mount_before_retry(cx: &mut TestAppContext) {
    let (dock, _, _) = fixture(cx);
    cx.set_next_window_present_outcome(PlatformWindowPresentOutcome::Rejected);
    let error = cx.update(|cx| {
        cx.open_window_detailed(open_gpui::WindowOptions::default(), |_, cx| {
            cx.new(|_| Root {
                dock: dock.clone(),
                surfaces: vec![FIRST],
            })
        })
        .expect_err("initial presentation must fail after the mount is established")
    });
    let ticket = error
        .retirement_ticket()
        .expect("the original native rollback owns a ticket")
        .clone();
    cx.run_until_parked();
    assert!(ticket.snapshot().quiesced(), "{:?}", ticket.snapshot());
    assert!(ticket.snapshot().native_terminal());
    assert!(
        dock.host.borrow().mounts.is_empty(),
        "rollback releases its physical mount"
    );
    assert!(
        dock.host.borrow().owners.is_empty(),
        "rollback releases the logical surface owner"
    );
    let retry = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![FIRST],
    });
    frames(&mut VisualTestContext::from_window(retry.into(), cx), &dock);
    assert_painted_pane(&dock, retry.into(), FIRST, A);
}

#[open_gpui::test]
fn two_regions_share_one_ticket_and_keep_independent_document_views(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, document) = fixture(cx);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![FIRST, SECOND],
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let button = visual
        .debug_bounds("view:21")
        .expect("real first View content");
    visual.simulate_click(button.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 1);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 0);
    assert_eq!(document.borrow().text, "sharedx");
    let host = dock.host.borrow();
    assert_eq!(host.mounts.len(), 1);
    let mount = host.mounts.values().next().unwrap();
    assert_eq!(mount.regions.len(), 2);
    for region in mount.regions.values() {
        assert!(region.presented.is_some());
    }
    assert_eq!(
        mount.regions[&FIRST].presented.unwrap().0,
        mount.regions[&SECOND].presented.unwrap().0
    );
}

#[open_gpui::test]
fn a_deferred_window_does_not_hold_the_session_update_lease(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, _) = fixture(cx);
    let first = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![FIRST],
    });
    let renders = Rc::new(Cell::new(0));
    let second = cx.open_window(size(px(800.0), px(600.0)), |_, _| CountedRoot {
        dock: dock.clone(),
        surface: SECOND,
        renders: renders.clone(),
    });
    let mut first_visual = VisualTestContext::from_window(first.into(), cx);
    first_visual.set_window_present_outcome(first.into(), PlatformWindowPresentOutcome::Deferred);
    frames(&mut first_visual, &dock);
    let mut second_visual = VisualTestContext::from_window(second.into(), cx);
    frames(&mut second_visual, &dock);
    let deferred_ticket = dock.host.borrow().mounts[&first.window_id()]
        .pending
        .as_ref()
        .unwrap()
        .ticket
        .clone();
    assert!(
        dock.host.borrow().mounts[&first.window_id()]
            .pending
            .as_ref()
            .unwrap()
            .update
            .is_none(),
        "renderer deferral must release the unpublished session update"
    );
    let stable_serial = dock.host.borrow().mounts[&second.window_id()].serial;
    let button = second_visual.debug_bounds("view:22").unwrap();
    second_visual.simulate_click(button.center(), Modifiers::default());
    frames(&mut second_visual, &dock);
    assert_eq!(views[1].read_with(&second_visual, |view, _| view.clicks), 1);
    let host = dock.host.borrow();
    assert!(host.mounts[&first.window_id()].pending.is_some());
    assert!(
        host.mounts[&second.window_id()].regions[&SECOND]
            .presented
            .is_some()
    );
    let deferred = &host.mounts[&first.window_id()];
    assert_eq!(
        deferred.pending.as_ref().unwrap().ticket.snapshot(),
        deferred_ticket.snapshot(),
        "a Deferred renderer keeps the original window and frame facts"
    );
    assert_eq!(
        host.mounts[&second.window_id()].serial,
        stable_serial,
        "retained content must not encode a new core output for provenance-only changes"
    );
    assert!(
        deferred.regions[&FIRST]
            .presented
            .is_none_or(|(serial, _)| serial < deferred.serial)
    );
    drop(host);
    first_visual.set_window_present_outcome(first.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut first_visual, &dock);
    assert_eq!(
        deferred_ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted
    );
    assert!(
        dock.host.borrow().mounts[&first.window_id()]
            .pending
            .is_none()
    );
}

#[open_gpui::test]
fn an_original_submitted_output_waits_for_a_sibling_candidate_before_admission(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, views, _) = fixture(cx);
    let first = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![FIRST],
    });
    cx.defer_next_window_frame_requests();
    let second = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![SECOND],
    });
    let mut first_visual = VisualTestContext::from_window(first.into(), cx);
    let mut second_visual = VisualTestContext::from_window(second.into(), cx);
    frames(&mut first_visual, &dock);
    frames(&mut second_visual, &dock);
    first_visual.set_window_present_outcome(first.into(), PlatformWindowPresentOutcome::Deferred);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(B, DockPlacement::Center(DockAnchor::Item(A)))?;
                frame.select_item_current(B)?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    frames(&mut first_visual, &dock);
    frames(&mut second_visual, &dock);
    let ticket = {
        let host = dock.host.borrow();
        let pending = host.mounts[&first.window_id()].pending.as_ref().unwrap();
        assert!(
            pending.update.is_none(),
            "renderer deferral has released the affine update"
        );
        pending.ticket.clone()
    };
    cx.set_window_frame_delivery_mode(
        second.into(),
        open_gpui::PlatformFrameDeliveryMode::Deferred,
    );
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&second.window_id())
        .unwrap()
        .regions
        .get_mut(&SECOND)
        .unwrap()
        .dirty = true;
    assert!(second_visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    let sibling_submission = cx.pending_window_frame_submission(second.into()).unwrap();
    assert!(
        dock.is_update_pending(),
        "the real sibling candidate retains its original affine update"
    );
    first_visual.set_window_present_outcome(first.into(), PlatformWindowPresentOutcome::Submitted);
    assert!(first_visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    let output = {
        let host = dock.host.borrow();
        assert!(host.error.is_none(), "{:?}", host.error);
        assert_eq!(
            ticket.snapshot().outcome(),
            open_gpui::WindowFrameSubmissionOutcome::Submitted
        );
        assert!(
            host.presentation_pending,
            "the original terminal waits behind the sibling lease"
        );
        let (_, output) = host.mounts[&first.window_id()].regions[&FIRST]
            .presented
            .unwrap();
        assert!(
            !host
                .session
                .presented_surface(FIRST)
                .is_some_and(|presented| { presented.matches_presentation_artifact(output) }),
            "a queued terminal has not yet granted input authority"
        );
        output
    };
    cx.update(|_| assert!(sibling_submission.complete(PlatformWindowPresentOutcome::Submitted)));
    cx.run_until_parked();
    {
        let host = dock.host.borrow();
        assert!(!host.presentation_pending);
        assert!(
            host.session
                .presented_surface(FIRST)
                .is_some_and(|presented| { presented.matches_presentation_artifact(output) }),
            "sibling acceptance admits the original Submitted output without repeating it"
        );
    }
    frames(&mut first_visual, &dock);
    let button = first_visual.debug_bounds("view:22").unwrap();
    first_visual.simulate_click(button.center(), Modifiers::default());
    assert_eq!(views[1].read_with(&first_visual, |view, _| view.clicks), 1);
    assert!(dock.error().is_none(), "{:?}", dock.error());
}

#[open_gpui::test]
fn registering_one_entity_as_two_views_is_rejected(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let view = cx.new(|_| View {
            document: Rc::new(RefCell::new(Document {
                text: String::new(),
            })),
            item: A,
            clicks: 0,
        });
        assert!(
            DockSurface::new(
                session(),
                [
                    DockPane::new(A, "A", view.clone()),
                    DockPane::new(B, "B", view)
                ],
                GpuiDockspaceConfig::default(),
                cx
            )
            .is_err()
        );
    });
}

#[open_gpui::test]
fn moving_a_view_to_another_surface_keeps_its_entity_and_local_state(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, document) = fixture(cx);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surfaces: vec![FIRST, SECOND],
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let first = visual.debug_bounds("view:21").unwrap();
    visual.simulate_click(first.center(), Modifiers::default());
    frames(&mut visual, &dock);
    let identity = views[0].entity_id();
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(A, DockPlacement::Center(DockAnchor::Item(B)))?;
                frame.select_item_current(A)?;
                let moved = frame.view().item(A).expect("the move preserves the item");
                assert_eq!(moved.surface(), SECOND);
                assert!(moved.is_selected());
                Ok(())
            },
            cx,
        )
        .unwrap()
    });
    dock.with_view(|view| {
        assert!(
            view.surface(FIRST).is_none(),
            "moving the final ordinary root prunes its logical surface"
        );
        assert_eq!(view.item(A).unwrap().surface(), SECOND);
        assert!(view.item(A).unwrap().is_selected());
    });
    frames(&mut visual, &dock);
    assert_painted_pane(&dock, window.into(), SECOND, A);
    assert!(
        dock.host.borrow().mounts[&window.window_id()].regions[&FIRST]
            .packet
            .as_ref()
            .unwrap()
            .pane_items()
            .is_empty(),
        "the vacant source region cannot hide the destination or duplicate its View"
    );
    let moved = visual
        .debug_bounds("view:21")
        .expect("original View in destination");
    assert!(moved.origin.x >= px(400.0));
    visual.simulate_click(moved.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(views[0].entity_id(), identity);
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 2);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 0);
    assert_eq!(document.borrow().text, "sharedxx");
}

fn managed_workspace(
    cx: &mut TestAppContext,
) -> (DockSurface, [Entity<View>; 2], Rc<RefCell<Document>>) {
    cx.update(|cx| {
        let document = Rc::new(RefCell::new(Document {
            text: "shared".into(),
        }));
        let views = [A, B].map(|item| {
            cx.new(|_| View {
                document: document.clone(),
                item,
                clicks: 0,
            })
        });
        let dock = DockSurface::new_managed(
            session_with_native(true),
            [
                DockPane::new(A, "First View", views[0].clone()),
                DockPane::new(B, "Second View", views[1].clone()),
            ],
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap();
        (dock, views, document)
    })
}

fn managed_fixture(
    cx: &mut TestAppContext,
) -> (
    DockSurface,
    [Entity<View>; 2],
    Rc<RefCell<Document>>,
    [open_gpui::AnyWindowHandle; 2],
) {
    let (dock, views, document) = managed_workspace(cx);
    let first = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(FIRST, true, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surfaces: vec![FIRST],
        }
    });
    let second = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(SECOND, false, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surfaces: vec![SECOND],
        }
    });
    (dock, views, document, [first.into(), second.into()])
}

#[open_gpui::test]
fn managed_two_windows_move_the_original_view_and_receive_real_content_clicks(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, views, document, windows) = managed_fixture(cx);
    let mut source = VisualTestContext::from_window(windows[0], cx);
    let mut destination = VisualTestContext::from_window(windows[1], cx);
    frames(&mut source, &dock);
    frames(&mut destination, &dock);
    assert_painted_pane(&dock, windows[0], FIRST, A);
    assert_painted_pane(&dock, windows[1], SECOND, B);
    let edit = source.debug_bounds("view:21").expect("first retained View");
    source.simulate_click(edit.center(), Modifiers::default());
    frames(&mut source, &dock);
    let identity = views[0].entity_id();
    destination.update(|_, cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(A, DockPlacement::Center(DockAnchor::Item(B)))?;
                frame.select_item_current(A)?;
                let moved = frame.view().item(A).expect("the move preserves the item");
                assert_eq!(moved.surface(), SECOND);
                assert!(moved.is_selected());
                Ok(())
            },
            cx,
        )
        .unwrap()
    });
    frames(&mut source, &dock);
    frames(&mut destination, &dock);
    assert_painted_pane(&dock, windows[1], SECOND, A);
    assert!(source.debug_bounds("view:21").is_none());
    let edit = destination
        .debug_bounds("view:21")
        .expect("the same View is rendered by the second real Window");
    destination.simulate_click(edit.center(), Modifiers::default());
    frames(&mut destination, &dock);
    assert_eq!(views[0].entity_id(), identity);
    assert_eq!(views[0].read_with(&destination, |view, _| view.clicks), 2);
    assert_eq!(views[1].read_with(&destination, |view, _| view.clicks), 0);
    assert_eq!(document.borrow().text, "sharedxx");
    assert!(
        dock.host.borrow().inputs.is_empty(),
        "managed input cannot use the local producer queue"
    );
    assert_eq!(dock.host.borrow().mounts.len(), 2);
    let stable_serial = dock.host.borrow().mounts[&windows[1].window_id()].serial;
    frames(&mut destination, &dock);
    assert_eq!(
        dock.host.borrow().mounts[&windows[1].window_id()].serial,
        stable_serial,
        "managed submission callbacks must stop requesting new core outputs once settled"
    );
}

#[open_gpui::test]
fn managed_deferred_source_does_not_starve_the_other_window_content(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, _, windows) = managed_fixture(cx);
    let mut source = VisualTestContext::from_window(windows[0], cx);
    source.set_window_present_outcome(windows[0], PlatformWindowPresentOutcome::Deferred);
    frames(&mut source, &dock);
    let mut sibling = VisualTestContext::from_window(windows[1], cx);
    frames(&mut sibling, &dock);
    assert_painted_pane(&dock, windows[1], SECOND, B);
    let edit = sibling
        .debug_bounds("view:22")
        .expect("healthy managed sibling contains its actual View");
    sibling.simulate_click(edit.center(), Modifiers::default());
    frames(&mut sibling, &dock);
    assert_eq!(views[1].read_with(&sibling, |view, _| view.clicks), 1);
    assert!(
        dock.host.borrow().mounts[&windows[0].window_id()]
            .pending
            .is_some()
    );
    assert!(
        dock.host.borrow().mounts[&windows[1].window_id()].regions[&SECOND]
            .presented
            .is_some()
    );
    let source_binding = {
        let host = dock.host.borrow();
        let window = dock
            .native_binding_for_window(windows[0].window_id())
            .unwrap();
        host.session
            .native_surface_bindings_for_window(window)
            .find(|binding| binding.surface() == FIRST)
            .unwrap()
    };
    source.set_window_present_outcome(windows[0], PlatformWindowPresentOutcome::Submitted);
    frames(&mut source, &dock);
    assert!(
        dock.host.borrow().mounts[&windows[0].window_id()]
            .input_frames
            .iter()
            .any(|frame| frame.surface == FIRST && frame.native_binding == Some(source_binding)),
        "a retained Deferred retry must publish the original output's exact native binding"
    );
}

#[open_gpui::test]
fn managed_child_creation_paints_the_original_view_after_bootstrap(cx: &mut TestAppContext) {
    use dockspace::{geometry::PhysicalRect, model::NativeWindowPlacement};

    cx.defer_next_window_frame_requests();
    let (dock, views, _) = managed_workspace(cx);
    let root = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(FIRST, true, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surfaces: vec![FIRST],
        }
    });
    frames(&mut VisualTestContext::from_window(root.into(), cx), &dock);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.create_native_surface_current(
                    SECOND,
                    FIRST,
                    NativeWindowPlacement::new(PhysicalRect::new(20.0, 30.0, 600.0, 450.0)?),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    let child = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| *window != root.into())
            .expect("the core's create effect opens a distinct physical window")
    });
    let mut visual = VisualTestContext::from_window(child, cx);
    frames(&mut visual, &dock);
    {
        let host = dock.host.borrow();
        let region = &host.mounts[&child.window_id()].regions[&SECOND];
        assert!(
            region
                .packet
                .as_ref()
                .is_some_and(|packet| packet.is_deferred())
        );
        assert!(
            region.presented.is_none(),
            "bootstrap has no painted content artifact"
        );
        assert!(host.session.presented_surface(SECOND).is_none());
    }
    assert!(cx.flush_window_mutation(child, open_gpui::WindowMutationDomain::Placement));
    frames(&mut visual, &dock);
    assert_painted_pane(&dock, child, SECOND, B);
    assert!(dock.take_native_reports().iter().any(|report| {
        matches!(report, super::native::NativeReport::Admitted(binding) if binding.surface() == SECOND)
    }));
    let content = visual
        .debug_bounds("view:22")
        .expect("the child's real View");
    visual.simulate_click(content.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 1);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn native_ingress_waits_for_the_original_unaccepted_candidate(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _, _, windows) = managed_fixture(cx);
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    cx.run_until_parked();
    let transport = dock.native_transport().unwrap();
    cx.update_window(windows[0], |_, window, cx| {
        let mut host = dock.host.borrow_mut();
        let owner = window.window_handle().window_id();
        assert!(host.mounts[&owner].pending.is_none());
        let super::host::HostState { session, mounts, native, .. } = &mut *host;
        let update = super::host_native::begin_frame(
            session,
            mounts,
            native.as_ref(),
            Some(window),
            cx,
        )
        .unwrap()
        .suspend();
        let ticket = window.begin_exact_frame_submission(cx).unwrap();
        let mount = host.mounts.get_mut(&owner).unwrap();
        let packets = mount
            .regions
            .iter()
            .map(|(surface, region)| (*surface, region.packet.clone().unwrap()))
            .collect();
        mount.serial += 1;
        mount.pending = Some(super::presentation::PendingSubmission::new(
            mount.serial,
            ticket,
            packets,
            update,
            Vec::new(),
        ));
        drop(host);
        assert!(dock.is_update_pending());
        windows[1].update(cx, |_, sibling, cx| {
            sibling.refresh();
            let _ = sibling.draw(cx);
        }).unwrap();
        {
            let host = dock.host.borrow();
            let sibling = &host.mounts[&windows[1].window_id()];
            assert!(sibling.waiting_for_update);
            assert!(sibling.input_frames.is_empty(), "a waiting frame supplies no new docking hit facts");
        }
        transport.focus(dockspace::runtime::NativeGlobalFocus::Foreign, cx);
        super::host_native::prepare_window(&dock.host, window, cx);
        assert!(dock.error().is_none(), "{:?}", dock.error());
        assert!(
            !transport.is_empty(),
            "native ingress must retain its ordered queue while the original candidate owns the lease"
        );
        super::presentation::discard(&dock.host, window, cx);
        assert!(!dock.is_update_pending());
        super::host_native::prepare_window(&dock.host, window, cx);
        assert!(dock.error().is_none(), "{:?}", dock.error());
        assert!(transport.is_empty(), "the original queue resumes after withdrawal");
    })
    .unwrap();
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    frames(&mut VisualTestContext::from_window(windows[1], cx), &dock);
    assert_painted_pane(&dock, windows[1], SECOND, B);
    assert!(!dock.host.borrow().mounts[&windows[1].window_id()].waiting_for_update);
    cx.update(|cx| dock.update(|_| Ok(()), cx)).unwrap();
}

#[open_gpui::test]
fn managed_chrome_clicks_use_the_native_producer_and_submitted_receivers(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, _, windows) = managed_fixture(cx);
    let mut first = VisualTestContext::from_window(windows[0], cx);
    let mut second = VisualTestContext::from_window(windows[1], cx);
    frames(&mut first, &dock);
    frames(&mut second, &dock);
    first.update(|_, cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(B, DockPlacement::Center(DockAnchor::Item(A)))?;
                frame.select_item_current(B)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut first, &dock);
    frames(&mut second, &dock);
    assert!(first.debug_bounds("view:22").is_some());
    let tab = first
        .debug_bounds("dockspace:tab:21")
        .expect("the first tab has actual retained chrome");
    let before = dock.host.borrow().session.native_input_recorded_count();
    first.simulate_click(tab.center(), Modifiers::default());
    frames(&mut first, &dock);
    assert!(dock.host.borrow().session.native_input_recorded_count() > before);
    assert!(
        dock.host.borrow().inputs.is_empty(),
        "managed chrome must never fall back to submit_host_pointer"
    );
    let content = first
        .debug_bounds("view:21")
        .expect("native receiver selection exposed the original View");
    first.simulate_click(content.center(), Modifiers::default());
    frames(&mut first, &dock);
    assert_eq!(views[0].read_with(&first, |view, _| view.clicks), 1);
    assert_eq!(views[1].read_with(&first, |view, _| view.clicks), 0);
}

#[open_gpui::test]
fn managed_anchor_close_drains_both_windows_after_delayed_native_terminal(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _, _, windows) = managed_fixture(cx);
    let mut source = VisualTestContext::from_window(windows[0], cx);
    frames(&mut source, &dock);
    let mut sibling = VisualTestContext::from_window(windows[1], cx);
    frames(&mut sibling, &dock);
    let hold = cx.hold_window_native_terminal(windows[1]);
    assert!(!cx.simulate_window_close(windows[0]));
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.update(|cx| {
        assert!(
            cx.windows().contains(&windows[0]),
            "anchor awaits its exact dependent terminal"
        )
    });
    assert!(hold.release());
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!cx.windows().contains(&windows[0]));
        assert!(!cx.windows().contains(&windows[1]));
    });
    assert!(dock.error().is_none(), "{:?}", dock.error());
    assert!(
        dock.native_is_quiescent(),
        "logical removal alone cannot finish native retirement"
    );
}

#[open_gpui::test]
fn managed_pre_root_map_failure_keeps_original_retirement_authority(cx: &mut TestAppContext) {
    use super::native::{NativeBridgeError, NativeReport};
    use dockspace::{geometry::PhysicalRect, model::NativeWindowPlacement};

    cx.defer_next_window_frame_requests();
    let (dock, views, _, windows) = managed_fixture(cx);
    let mut first = VisualTestContext::from_window(windows[0], cx);
    let mut second = VisualTestContext::from_window(windows[1], cx);
    frames(&mut first, &dock);
    frames(&mut second, &dock);
    cx.fail_next_window_map("native bridge pre-root rollback proof");
    let ticket = cx.update(|cx| {
        let target = SurfaceId::new(30);
        dock.update(
            |frame| {
                frame.attach_empty_surface_current(target, RootId::new(30))?;
                frame.create_native_surface_current(
                    target,
                    FIRST,
                    NativeWindowPlacement::new(PhysicalRect::new(20.0, 30.0, 600.0, 450.0)?),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        let ticket = dock
            .take_native_reports()
            .into_iter()
            .find_map(|report| match report {
                NativeReport::OpenFailed {
                    stage,
                    retirement_ticket,
                    ..
                } => {
                    assert_eq!(
                        stage,
                        Some(open_gpui::WindowOpenFailureStage::PlatformCreateOrMap)
                    );
                    retirement_ticket
                }
                _ => None,
            })
            .expect("a pre-root native map failure retains the original GPUI rollback ticket");
        assert!(
            !ticket.snapshot().native_terminal(),
            "returning an open error cannot manufacture native terminal"
        );
        let callbacks = dock
            .native_callbacks_for_window(ticket.snapshot().window_id())
            .expect("the bridge keeps native custody even though the root builder never ran");
        assert!(matches!(
            callbacks.seal(),
            Err(NativeBridgeError::NotQuiescent)
        ));
        assert!(
            dock.native_binding_for_window(ticket.snapshot().window_id())
                .is_some()
        );
        ticket
    });
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    let terminal = ticket.snapshot();
    assert!(terminal.quiesced() && terminal.native_terminal());
    assert!(!terminal.protocol_violation());
    assert!(
        dock.native_binding_for_window(terminal.window_id())
            .is_none(),
        "true terminal and exact binding quiescence retire failed native custody"
    );
    frames(&mut first, &dock);
    frames(&mut second, &dock);
    let button = second
        .debug_bounds("view:22")
        .expect("a failed create leaves the real sibling View usable");
    second.simulate_click(button.center(), Modifiers::default());
    frames(&mut second, &dock);
    assert_eq!(views[1].read_with(&second, |view, _| view.clicks), 1);
}
