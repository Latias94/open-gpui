//! Real platform exit and accepted occlusion revoke feedback without losing the drag.

use std::time::Duration;

use crate::adapter::element::wheel::ScrollTarget;
use dockspace::runtime::{SurfacePaintProgramOperation, SurfacePaintProgramStep};
use open_gpui::{Point, PointerCapture};

use super::{
    wheel::{SingleRegion, overflowing_surfaces},
    *,
};

struct OverlayRoot {
    dock: DockSurface,
    covered: Rc<Cell<bool>>,
}

impl Render for OverlayRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .child(self.dock.element(SURFACE))
            .when(self.covered.get(), |root| {
                root.child(
                    div()
                        .id("input-recovery-cover")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .occlude(),
                )
            })
    }
}

pub(super) fn begin_drag(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    item: ItemId,
) -> PointerCapture {
    let pressed = visual
        .debug_bounds(&format!("dockspace:tab:{}", item.get()))
        .unwrap()
        .center();
    visual.simulate_mouse_down(pressed, MouseButton::Left, Modifiers::none());
    frames(visual, dock);
    assert!(dock.with_view(|view| view.item(item).unwrap().is_active()));
    visual.simulate_mouse_move(
        pressed + point(px(24.0), px(0.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    frames(visual, dock);
    assert!(dock.has_active_gesture());
    visual.update(|window, _| {
        let capture = window.captured_pointer().expect("the original tab drag");
        assert_eq!(capture.button(), MouseButton::Left);
        if dock.host.borrow().native.is_none() {
            assert_eq!(
                capture.handle(),
                dock.host.borrow().mounts[&window.window_handle().window_id()].capture
            );
        }
        capture
    })
}

pub(super) fn pane_center(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    item: ItemId,
) -> Point<Pixels> {
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let bounds = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Pane(candidate, geometry) if *candidate == item => Some(geometry.clip()),
                _ => None,
            })
            .unwrap();
        to_gpui(bounds, region.bounds.unwrap()).center()
    })
}

fn preview_fixture(cx: &mut TestAppContext) -> DockSurface {
    super::content_receivers::content_fixture(cx, false).0
}

fn assert_tab_ink(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    item: ItemId,
    expected: Hsla,
) {
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let bounds = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Tab(bounds, key, _, false) if *key == item.into() => Some(*bounds),
                _ => None,
            })
            .expect("the observed tab must remain inactive");
        let bounds = to_gpui(bounds, region.bounds.unwrap()).scale(window.scale_factor());
        let quads = window.painted_quads();
        let background = quads
            .iter()
            .find(|quad| quad.bounds == bounds)
            .expect("the accepted frame must paint the inactive tab's background");
        assert_eq!(background.background.as_solid(), Some(expected));
    });
}

pub(super) fn drag_feedback(dock: &DockSurface) -> (bool, bool, bool) {
    let mut host = dock.host.borrow_mut();
    let mut frame = host.session.begin_host_frame().unwrap();
    let retained_hover = frame.host_pointer_hover_position(SURFACE).is_some();
    let plan = frame.paint_plan(SURFACE).unwrap().unwrap();
    let mut guides = false;
    plan.visit_surface_program(|step| {
        if let SurfacePaintProgramStep::DropGuide {
            affordance: Some(cluster),
            ..
        } = step
        {
            guides |= cluster.targets().next().is_some();
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    (plan.has_manipulation_preview(), guides, retained_hover)
}

fn assert_feedback_revoked(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    capture: PointerCapture,
) {
    assert!(
        dock.has_active_gesture(),
        "occlusion must preserve the gesture"
    );
    visual.update(|window, _| {
        assert_eq!(window.captured_pointer(), Some(capture));
        let host = dock.host.borrow();
        assert_eq!(host.session.pointer_continuation(), None);
        assert!(
            host.mounts[&window.window_handle().window_id()]
                .wake
                .is_none()
        );
    });
    assert_eq!(
        drag_feedback(dock),
        (false, false, false),
        "preview, guides, and retained hover work must all be revoked"
    );
}

fn assert_pointer_route(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    position: Point<Pixels>,
    clear: bool,
) {
    visual.update(|window, _| {
        assert_eq!(window.mouse_position(), position);
        assert!(window.is_mouse_in_window());
        let host = dock.host.borrow();
        assert!(host.native.is_none());
        assert!(host.inputs.is_empty());
        let owner = window.window_handle().window_id();
        let mount = &host.mounts[&owner];
        assert!(mount.pending.is_none());
        let frame = mount
            .input_frames
            .iter()
            .find(|frame| frame.surface == SURFACE)
            .unwrap();
        assert_eq!(frame.serial, mount.serial);
        assert!(input::is_presented(&host, owner, SURFACE, frame.serial));
        assert_eq!(
            frame
                .hitboxes
                .iter()
                .chain(&frame.content_hitboxes)
                .any(|hitbox| hitbox.window_point_target_rank(position, window) == Some(0)),
            clear,
            "the accepted hit graph must observe the actual overlay"
        );
    });
}

#[open_gpui::test]
fn mouse_exit_at_the_last_in_window_position_clears_inactive_tab_ink(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let position = visual.debug_bounds("dockspace:tab:62").unwrap().center();
    let colors = dock.host.borrow().config.colors.clone();
    assert_ne!(colors.tab, colors.hovered_tab);
    assert_tab_ink(&dock, &mut visual, SECOND, colors.tab);
    visual.simulate_mouse_move(position, None, Modifiers::none());
    frames(&mut visual, &dock);
    assert_tab_ink(&dock, &mut visual, SECOND, colors.hovered_tab);

    visual.simulate_mouse_exit(position, None, Modifiers::none());
    frames(&mut visual, &dock);
    visual.update(|window, _| {
        assert_eq!(window.mouse_position(), position);
        assert!(!window.is_mouse_in_window());
        assert!(window.captured_pointer().is_none());
    });
    assert_tab_ink(&dock, &mut visual, SECOND, colors.tab);
    assert!(!dock.has_active_gesture());

    visual.simulate_mouse_move(position, None, Modifiers::none());
    frames(&mut visual, &dock);
    assert_tab_ink(&dock, &mut visual, SECOND, colors.hovered_tab);
}

#[open_gpui::test]
fn captured_exit_revokes_tab_hover_deadline_until_fresh_reentry(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    cx.set_platform_hovered_window(Some(window.into()));
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let capture = begin_drag(&dock, &mut visual, SECOND);
    let position = visual.debug_bounds("dockspace:tab:61").unwrap().center();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let previous = dock.host.borrow().session.pointer_continuation().unwrap();
    let delay = dock
        .host
        .borrow()
        .session
        .presentation_config()
        .hover_selection_delay()
        .unwrap();
    assert!(drag_feedback(&dock).2);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));

    visual.simulate_mouse_exit(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    visual.update(|window, _| {
        assert_eq!(window.mouse_position(), position);
        assert!(!window.is_mouse_in_window());
    });
    assert_feedback_revoked(&dock, &mut visual, capture);
    cx.background_executor.advance_clock(delay);
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert_feedback_revoked(&dock, &mut visual, capture);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));

    let reentered_at = dock.host.borrow().observed_at();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let fresh = dock.host.borrow().session.pointer_continuation().unwrap();
    assert_ne!(fresh.token(), previous.token());
    assert_eq!(fresh.deadline(), reentered_at + delay);
    assert!(drag_feedback(&dock).2);
    assert_eq!(
        visual.update(|window, _| window.captured_pointer()),
        Some(capture)
    );
    let remaining = fresh.deadline() - dock.host.borrow().observed_at();
    cx.background_executor
        .advance_clock(remaining - Duration::from_millis(1));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_eq!(
        dock.host.borrow().session.pointer_continuation(),
        Some(fresh)
    );
    cx.background_executor
        .advance_clock(Duration::from_millis(2));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_eq!(
        visual.update(|window, _| window.captured_pointer()),
        Some(capture)
    );
}

#[open_gpui::test]
fn captured_exit_revokes_drop_preview_until_a_fresh_move(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = preview_fixture(cx);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let position = pane_center(&dock, &mut visual, SECOND);
    let capture = begin_drag(&dock, &mut visual, FIRST);
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(drag_feedback(&dock), (true, true, true));

    visual.simulate_mouse_exit(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    assert_feedback_revoked(&dock, &mut visual, capture);
    frames(&mut visual, &dock);
    assert_feedback_revoked(&dock, &mut visual, capture);

    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(drag_feedback(&dock), (true, true, true));
    assert_eq!(
        visual.update(|window, _| window.captured_pointer()),
        Some(capture)
    );
}

#[open_gpui::test]
fn accepted_overlay_revokes_stationary_tab_hover_until_a_full_new_dwell(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let covered = Rc::new(Cell::new(false));
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| OverlayRoot {
        dock: dock.clone(),
        covered: covered.clone(),
    });
    cx.set_platform_hovered_window(Some(window.into()));
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let capture = begin_drag(&dock, &mut visual, SECOND);
    let position = visual.debug_bounds("dockspace:tab:61").unwrap().center();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let previous = dock.host.borrow().session.pointer_continuation().unwrap();
    let delay = dock
        .host
        .borrow()
        .session
        .presentation_config()
        .hover_selection_delay()
        .unwrap();
    assert!(drag_feedback(&dock).2);
    assert_pointer_route(&dock, &mut visual, position, true);
    cx.background_executor.advance_clock(delay / 2);
    cx.run_until_parked();
    assert_eq!(
        dock.host.borrow().session.pointer_continuation(),
        Some(previous)
    );

    visual.update(|window, _| {
        covered.set(true);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, false);
    assert_feedback_revoked(&dock, &mut visual, capture);
    cx.background_executor.advance_clock(delay);
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_feedback_revoked(&dock, &mut visual, capture);

    visual.update(|window, _| {
        covered.set(false);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, true);
    assert_feedback_revoked(&dock, &mut visual, capture);
    cx.background_executor.advance_clock(delay);
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_feedback_revoked(&dock, &mut visual, capture);

    let moved_at = dock.host.borrow().observed_at();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let fresh = dock.host.borrow().session.pointer_continuation().unwrap();
    assert_ne!(fresh.token(), previous.token());
    assert_eq!(fresh.deadline(), moved_at + delay);
    let remaining = fresh.deadline() - dock.host.borrow().observed_at();
    cx.background_executor
        .advance_clock(remaining - Duration::from_millis(1));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_eq!(
        dock.host.borrow().session.pointer_continuation(),
        Some(fresh)
    );
    cx.background_executor
        .advance_clock(Duration::from_millis(2));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert!(!dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert_eq!(
        visual.update(|window, _| window.captured_pointer()),
        Some(capture)
    );
}

#[open_gpui::test]
fn pending_output_retains_same_frame_blocking_before_the_overlay_disappears(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let covered = Rc::new(Cell::new(false));
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| OverlayRoot {
        dock: dock.clone(),
        covered: covered.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let capture = begin_drag(&dock, &mut visual, SECOND);
    let position = visual.debug_bounds("dockspace:tab:61").unwrap().center();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    visual.update(|window, _| {
        covered.set(true);
        window.refresh();
    });
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    assert!(visual.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
    {
        let host = dock.host.borrow();
        let pending = host.mounts[&window.window_id()].pending.as_ref().unwrap();
        assert!(pending.accepted_input_frames().is_some());
        assert_eq!(
            host.inputs.len(),
            1,
            "accepted occlusion must survive its pending output"
        );
    }
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    visual.update(|window, _| {
        covered.set(false);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_feedback_revoked(&dock, &mut visual, capture);
    let delay = dock
        .host
        .borrow()
        .session
        .presentation_config()
        .hover_selection_delay()
        .unwrap();
    cx.background_executor.advance_clock(delay * 2);
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert_feedback_revoked(&dock, &mut visual, capture);
    assert!(!dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    let moved_at = dock.host.borrow().observed_at();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let fresh = dock.host.borrow().session.pointer_continuation().unwrap();
    assert_eq!(fresh.deadline(), moved_at + delay);
}

fn overflowing_fixture(cx: &mut TestAppContext) -> DockSurface {
    cx.update(|cx| {
        let items = (FIRST.get()..FIRST.get() + 16)
            .map(ItemId::new)
            .collect::<Vec<_>>();
        let panes = items
            .iter()
            .map(|&item| {
                let focus = cx.focus_handle();
                let pane = cx.new(|_| Pane { focus });
                DockPane::new(item, format!("Recovery tab number {}", item.get()), pane)
            })
            .collect::<Vec<_>>();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(71), DockspaceNode::tabs(items)),
        )])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    })
}

fn strip_offset(dock: &DockSurface) -> f64 {
    let mut host = dock.host.borrow_mut();
    let mut frame = host.session.begin_host_frame().unwrap();
    let plan = frame.paint_plan(SURFACE).unwrap().unwrap();
    let mut offset = None;
    plan.visit_surface_program(|step| {
        if let SurfacePaintProgramStep::Operation {
            operation: SurfacePaintProgramOperation::TabBar(bar),
            ..
        } = step
        {
            assert!(bar.maximum_scroll_offset() > 0.0);
            assert!(offset.replace(bar.scroll_offset()).is_none());
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    offset.expect("the overflowing strip must remain in the core paint program")
}

#[open_gpui::test]
fn accepted_overlay_revokes_stationary_edge_scroll_until_a_full_new_interval(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let dock = overflowing_fixture(cx);
    let covered = Rc::new(Cell::new(false));
    let window = cx.open_window(size(px(320.0), px(600.0)), |_, _| OverlayRoot {
        dock: dock.clone(),
        covered: covered.clone(),
    });
    cx.set_platform_hovered_window(Some(window.into()));
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let capture = begin_drag(&dock, &mut visual, FIRST);
    let position = visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let forward = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Control(control) if control.label == "Scroll tabs forward" => {
                    Some(to_gpui(control.bounds, region.bounds.unwrap()))
                }
                _ => None,
            })
            .unwrap();
        // Stay in the viewport's edge band without crossing a tab close button.
        point(forward.origin.x - px(2.0), forward.origin.y + px(1.0))
    });
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        assert!(region.packet.as_ref().unwrap().draws.iter().any(|draw| {
            matches!(draw, Draw::Scroll(ScrollTarget::Strip(receiver))
                if to_gpui(receiver.bounds(), region.bounds.unwrap()).contains(&position))
        }));
    });
    let before = strip_offset(&dock);
    let interval = dock
        .host
        .borrow()
        .session
        .presentation_config()
        .tab_strip_edge_scroll_interval()
        .unwrap();
    let moved_at = dock.host.borrow().observed_at();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let previous = dock.host.borrow().session.pointer_continuation().unwrap();
    assert_eq!(previous.deadline(), moved_at + interval);
    assert_eq!(strip_offset(&dock), before);
    cx.background_executor.advance_clock(interval / 2);
    cx.run_until_parked();

    visual.update(|window, _| {
        covered.set(true);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, false);
    assert_feedback_revoked(&dock, &mut visual, capture);
    cx.background_executor.advance_clock(interval * 3);
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert_eq!(strip_offset(&dock), before);
    assert_feedback_revoked(&dock, &mut visual, capture);

    visual.update(|window, _| {
        covered.set(false);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, true);
    assert_feedback_revoked(&dock, &mut visual, capture);
    cx.background_executor.advance_clock(interval * 3);
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert_eq!(strip_offset(&dock), before);
    assert_feedback_revoked(&dock, &mut visual, capture);

    let moved_at = dock.host.borrow().observed_at();
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    let fresh = dock.host.borrow().session.pointer_continuation().unwrap();
    assert_ne!(fresh.token(), previous.token());
    assert_eq!(fresh.deadline(), moved_at + interval);
    let remaining = fresh.deadline() - dock.host.borrow().observed_at();
    cx.background_executor
        .advance_clock(remaining - Duration::from_millis(1));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert_eq!(strip_offset(&dock), before);
    assert_eq!(
        dock.host.borrow().session.pointer_continuation(),
        Some(fresh)
    );
    cx.background_executor
        .advance_clock(Duration::from_millis(2));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert!(strip_offset(&dock) > before);
    assert_eq!(
        visual.update(|window, _| window.captured_pointer()),
        Some(capture)
    );
    assert!(dock.has_active_gesture());
}

#[open_gpui::test]
fn accepted_overlay_revokes_stationary_drop_preview_until_a_fresh_move(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = preview_fixture(cx);
    let covered = Rc::new(Cell::new(false));
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| OverlayRoot {
        dock: dock.clone(),
        covered: covered.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let position = pane_center(&dock, &mut visual, SECOND);
    let capture = begin_drag(&dock, &mut visual, FIRST);
    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, true);
    assert_eq!(drag_feedback(&dock), (true, true, true));

    visual.update(|window, _| {
        covered.set(true);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, false);
    assert_feedback_revoked(&dock, &mut visual, capture);
    visual.update(|window, _| {
        covered.set(false);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, true);
    assert_feedback_revoked(&dock, &mut visual, capture);
    cx.background_executor.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    frames(&mut visual, &dock);
    assert_feedback_revoked(&dock, &mut visual, capture);

    visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    assert_pointer_route(&dock, &mut visual, position, true);
    assert_eq!(drag_feedback(&dock), (true, true, true));
    assert_eq!(
        visual.update(|window, _| window.captured_pointer()),
        Some(capture)
    );
    assert!(dock.has_active_gesture());
}

#[open_gpui::test]
fn closing_a_window_returns_unaccepted_input_from_another_window(cx: &mut TestAppContext) {
    close_window_with_borrowed_input(cx, BorrowedInputClose::Borrower);
}

#[open_gpui::test]
fn closing_the_input_source_withdraws_its_unaccepted_sibling_candidate(cx: &mut TestAppContext) {
    close_window_with_borrowed_input(cx, BorrowedInputClose::Source);
}

#[open_gpui::test]
fn closing_the_input_source_before_delivering_the_renderer_receipt_rejects_its_input(
    cx: &mut TestAppContext,
) {
    close_window_with_borrowed_input(cx, BorrowedInputClose::SourceAfterSubmission);
}

#[derive(Clone, Copy)]
enum BorrowedInputClose {
    Borrower,
    Source,
    SourceAfterSubmission,
}

fn close_window_with_borrowed_input(cx: &mut TestAppContext, close: BorrowedInputClose) {
    let close_source = !matches!(close, BorrowedInputClose::Borrower);
    let renderer_submitted = matches!(close, BorrowedInputClose::SourceAfterSubmission);
    let sibling = SurfaceId::new(52);
    let (dock, _) = overflowing_surfaces(&[SURFACE, sibling], cx);
    let [first, second] = [SURFACE, sibling].map(|surface| {
        cx.defer_next_window_frame_requests();
        cx.open_window(size(px(620.0), px(400.0)), |_, _| {
            SingleRegion(dock.clone(), surface)
        })
    });
    let mut source = active_visual(first.into(), cx);
    let mut target = VisualTestContext::from_window(second.into(), cx);
    frames(&mut source, &dock);
    frames(&mut target, &dock);
    assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
    cx.set_platform_hovered_window(Some(first.into()));
    let tab = source.debug_bounds("dockspace:tab:2").unwrap().center();
    let sibling_action = {
        let host = dock.host.borrow();
        let packet = host.mounts[&second.window_id()].regions[&sibling]
            .packet
            .as_ref()
            .unwrap();
        packet
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Control(control)
                    if matches!(control.kind, ControlKind::Tab(tab) if tab == ItemId::new(42).into()) =>
                {
                    Some(control.slot(ControlAction::Activate)?.borrow().as_ref()?.intent.clone())
                }
                _ => None,
            })
            .unwrap()
    };
    cx.set_window_frame_delivery_mode(
        second.into(),
        open_gpui::PlatformFrameDeliveryMode::Deferred,
    );
    source.update(|window, cx| {
        window.dispatch_event(
            open_gpui::PlatformInput::MouseDown(MouseDownEvent {
                position: tab,
                button: MouseButton::Left,
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            open_gpui::PlatformInput::MouseUp(MouseUpEvent {
                position: tab,
                button: MouseButton::Left,
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
        assert_eq!(dock.host.borrow().inputs.len(), 2);
        if close_source {
            input::queue_action(
                &mut dock.host.borrow_mut(),
                second.window_id(),
                sibling_action,
                cx,
            );
        }
        // Construct the sibling candidate before the input transaction's
        // normal work can draw and commit the source window.
        open_gpui::AnyWindowHandle::from(second)
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
    });
    assert!(target.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    let pending = cx.pending_window_frame_submission(second.into()).unwrap();
    assert!(dock.is_update_pending());
    {
        let host = dock.host.borrow();
        assert!(host.inputs.is_empty());
        let borrowed = &host.mounts[&second.window_id()]
            .pending
            .as_ref()
            .unwrap()
            .inputs;
        assert_eq!(borrowed.len(), if close_source { 3 } else { 2 });
        assert!(
            borrowed[..2]
                .iter()
                .all(|input| input.owner() == first.window_id())
        );
    }
    let surviving_window = if close_source {
        source.update(|window, cx| {
            if renderer_submitted {
                assert!(pending.complete(PlatformWindowPresentOutcome::Submitted));
                // The receipt is queued behind this update and the core candidate
                // still belongs to the surviving window.
                assert!(dock.is_update_pending());
            }
            window.remove_window(cx);
        });
        cx.set_window_frame_delivery_mode(
            second.into(),
            open_gpui::PlatformFrameDeliveryMode::Synchronous,
        );
        second
    } else {
        target.update(|window, cx| window.remove_window(cx));
        first
    };
    assert_eq!(
        pending.status(),
        if renderer_submitted {
            open_gpui::PlatformFrameSubmissionStatus::Submitted
        } else {
            open_gpui::PlatformFrameSubmissionStatus::Cancelled
        }
    );
    let mut ran = false;
    for _ in 0..8 {
        cx.run_until_parked();
        if !cx.simulate_window_scheduled_frame(surviving_window.into()) {
            break;
        }
        ran = true;
    }
    assert!(ran);
    if close_source {
        assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
        assert!(dock.with_view(|view| view.item(ItemId::new(42)).unwrap().is_active()));
    } else {
        assert!(dock.with_view(|view| view.item(ItemId::new(2)).unwrap().is_active()));
    }
    assert!(dock.host.borrow().inputs.is_empty());
}
