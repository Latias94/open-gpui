//! Real platform exit and accepted occlusion revoke feedback without losing the drag.

use std::time::Duration;

use crate::adapter::element::wheel::ScrollTarget;
use dockspace::runtime::{SurfacePaintProgramOperation, SurfacePaintProgramStep};
use open_gpui::{Point, PointerCapture};

use super::{wheel::overflowing_surfaces, *};

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
        to_gpui(bounds, region.layout_bounds().unwrap()).center()
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
        let bounds = to_gpui(bounds, region.layout_bounds().unwrap()).scale(window.scale_factor());
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
                    Some(to_gpui(control.bounds, region.layout_bounds().unwrap()))
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
                if to_gpui(receiver.bounds(), region.layout_bounds().unwrap()).contains(&position))
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
    retire_window_with_borrowed_input(cx, BorrowedInputRetirement::CloseBorrower);
}

#[open_gpui::test]
fn closing_the_input_source_withdraws_its_unaccepted_sibling_candidate(cx: &mut TestAppContext) {
    retire_window_with_borrowed_input(
        cx,
        BorrowedInputRetirement::CloseSource {
            renderer_submitted: false,
        },
    );
}

#[open_gpui::test]
fn closing_the_input_source_before_delivering_the_renderer_receipt_rejects_its_input(
    cx: &mut TestAppContext,
) {
    retire_window_with_borrowed_input(
        cx,
        BorrowedInputRetirement::CloseSource {
            renderer_submitted: true,
        },
    );
}

#[open_gpui::test]
fn unmounting_an_input_source_discards_its_click_and_preserves_other_input_order(
    cx: &mut TestAppContext,
) {
    for renderer_submitted in [false, true] {
        retire_window_with_borrowed_input(
            cx,
            BorrowedInputRetirement::UnmountSource {
                renderer_submitted,
                remount: false,
            },
        );
    }
}

#[open_gpui::test]
fn remounting_an_input_source_cannot_revive_its_click_before_the_renderer_receipt(
    cx: &mut TestAppContext,
) {
    retire_window_with_borrowed_input(
        cx,
        BorrowedInputRetirement::UnmountSource {
            renderer_submitted: true,
            remount: true,
        },
    );
}

enum BorrowedInputRetirement {
    CloseBorrower,
    CloseSource {
        renderer_submitted: bool,
    },
    UnmountSource {
        renderer_submitted: bool,
        remount: bool,
    },
}

struct OptionalDockMount {
    dock: DockSurface,
    surface: SurfaceId,
    mounted: Rc<Cell<bool>>,
}

impl Render for OptionalDockMount {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().when(self.mounted.get(), |root| {
            root.child(self.dock.element(self.surface))
        })
    }
}

struct InsetDockMount(DockSurface);

impl Render for InsetDockMount {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .w(px(540.0))
                .h(px(300.0))
                .child(self.0.element(SURFACE)),
        )
    }
}

#[open_gpui::test]
fn releasing_outside_the_dock_before_the_press_commits_keeps_tabs_interactive(
    cx: &mut TestAppContext,
) {
    use open_gpui::{PlatformFrameDeliveryMode, PlatformWindowPresentOutcome};

    let dock = preview_fixture(cx);
    cx.defer_next_window_frame_requests();
    let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| {
        InsetDockMount(dock.clone())
    });
    let mut visual = active_visual(window.into(), cx);
    cx.set_platform_hovered_window(Some(window.into()));
    frames(&mut visual, &dock);
    let second = visual
        .debug_bounds(&format!("dockspace:tab:{}", SECOND.get()))
        .unwrap()
        .center();
    cx.set_window_frame_delivery_mode(window.into(), PlatformFrameDeliveryMode::Deferred);
    visual.simulate_mouse_down(second, MouseButton::Left, Modifiers::none());
    assert!(visual.update(|window, _| window.captured_pointer().is_some()));
    assert!(!dock.has_active_gesture());
    visual.simulate_mouse_up(
        point(px(575.0), px(350.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(visual.update(|window, _| window.captured_pointer().is_none()));
    if let Some(submission) = cx.pending_window_frame_submission(window.into()) {
        cx.update(|_| submission.complete(PlatformWindowPresentOutcome::Submitted));
    }
    cx.set_window_frame_delivery_mode(window.into(), PlatformFrameDeliveryMode::Synchronous);
    frames(&mut visual, &dock);
    assert!(!dock.has_active_gesture());

    let first = visual
        .debug_bounds(&format!("dockspace:tab:{}", FIRST.get()))
        .unwrap()
        .center();
    visual.simulate_mouse_down(first, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(first, MouseButton::Left, Modifiers::none());
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert!(!dock.has_active_gesture());
    assert!(dock.error().is_none());
    cx.update(|cx| dock.refresh_tab_content(FIRST, cx)).unwrap();
}

#[open_gpui::test]
fn managed_source_callback_releases_a_primary_press_from_either_ingress(cx: &mut TestAppContext) {
    use dockspace::{
        geometry::PhysicalPoint,
        runtime::{
            NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerButton,
            NativePointerEvent, NativePointerHover, NativePointerInput, NativePointerOwner,
            PointerDeviceKind, PointerEventMetadata,
        },
    };
    use open_gpui::{
        DevicePixels, PlatformInput, PlatformNativePointerCaptureOwner,
        PlatformNativePointerPhysicalFrame, PlatformWindowHit, PlatformWindowHitStack,
        PlatformWindowPhysicalCoverage, PointerCancelEvent, PointerCancelReason,
    };

    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let sibling_surface = SurfaceId::new(52);
    let sibling_item = ItemId::new(63);

    for (raw_press, outside, flush_press) in [
        (true, false, true),
        (true, true, false),
        (false, true, true),
    ] {
        let (dock, _) = super::content_receivers::content_fixture_layout(
            cx,
            true,
            [
                DockspaceSurfaceLayout::new(
                    SURFACE,
                    DockspaceRootLayout::new(RootId::new(71), DockspaceNode::tabs([FIRST, SECOND])),
                ),
                DockspaceSurfaceLayout::new(
                    sibling_surface,
                    DockspaceRootLayout::new(RootId::new(72), DockspaceNode::tabs([sibling_item])),
                ),
            ],
            &[FIRST, SECOND, sibling_item],
        );
        let window = cx.open_window(size(px(620.0), px(400.0)), |window, cx| {
            dock.bind_native_root(SURFACE, true, window, cx).unwrap();
            InsetDockMount(dock.clone())
        });
        let handle: open_gpui::AnyWindowHandle = window.into();
        let sibling = cx
            .update(|cx| dock.open_native_root(sibling_surface, false, Default::default(), cx))
            .unwrap();
        let mut sibling_visual = VisualTestContext::from_window(sibling, cx);
        let (mut sibling_bounds, sibling_scale) = sibling_visual.update(|window, _| {
            let facts = window.platform_facts();
            (
                facts.bounds.to_device_pixels(facts.scale_factor),
                facts.scale_factor,
            )
        });
        sibling_bounds.origin.x = DevicePixels(2000);
        cx.set_platform_window_physical_client_geometry(
            sibling,
            Some(sibling_bounds),
            sibling_scale,
        );
        cx.simulate_window_scale_factor_change(sibling, sibling_scale);
        frames(&mut sibling_visual, &dock);
        let mut visual = active_visual(handle, cx);
        cx.set_platform_hovered_window(Some(handle));
        super::content_receivers::prepare_native_input(&mut visual);
        frames(&mut visual, &dock);
        let point = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::TabClose).center();
        if raw_press {
            let binding = dock.native_binding_for_window(handle.window_id()).unwrap();
            let geometry =
                visual.update(|window, _| window.platform_facts().physical_geometry.unwrap());
            let global = geometry.local_to_global(point).unwrap();
            cx.set_platform_window_hit_stack(
                PlatformWindowHitStack::try_available(
                    global,
                    vec![PlatformWindowHit::RegisteredApplication {
                        window: handle,
                        geometry,
                        coverage: PlatformWindowPhysicalCoverage::try_new(geometry.client_bounds())
                            .unwrap(),
                    }],
                )
                .unwrap(),
            );
            let transport = dock.native_transport().unwrap();
            cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
            cx.update(|cx| {
                transport.pointer(
                    NativePointerInput::new(
                        dockspace::runtime::NativePointerId::new(1),
                        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
                        NativeDesktopPointerLocation::new(
                            NativeDesktopPosition::Exact(
                                PhysicalPoint::new(f64::from(global.x.0), f64::from(global.y.0))
                                    .unwrap(),
                            ),
                            NativePointerHover::Dock(binding),
                            None,
                        ),
                        NativePointerOwner::Native(binding),
                        NativePointerOwner::Unknown,
                    )
                    .with_metadata(PointerEventMetadata::untimed(PointerDeviceKind::Mouse)),
                    cx,
                )
            });
        } else {
            visual.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
        }
        if flush_press {
            frames(&mut visual, &dock);
            assert!(dock.has_active_gesture());
            sibling_visual.simulate_event(PointerCancelEvent {
                reason: PointerCancelReason::CaptureRevoked,
            });
            frames(&mut visual, &dock);
            assert!(
                dock.has_active_gesture(),
                "the sibling did not own the press"
            );
        }
        cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
        if outside {
            let source_geometry =
                visual.update(|window, _| window.platform_facts().physical_geometry.unwrap());
            let sibling_geometry = sibling_visual
                .update(|window, _| window.platform_facts().physical_geometry.unwrap());
            let global = sibling_geometry
                .local_to_global(open_gpui::point(px(20.0), px(20.0)))
                .unwrap();
            let position = source_geometry.global_to_local(global).unwrap();
            cx.set_platform_hovered_window(Some(sibling));
            cx.set_platform_window_hit_stack(
                PlatformWindowHitStack::try_available(
                    global,
                    vec![PlatformWindowHit::RegisteredApplication {
                        window: sibling,
                        geometry: sibling_geometry,
                        coverage: PlatformWindowPhysicalCoverage::try_new(
                            sibling_geometry.client_bounds(),
                        )
                        .unwrap(),
                    }],
                )
                .unwrap(),
            );
            let physical = PlatformNativePointerPhysicalFrame::new(global, source_geometry)
                .with_capture_owner(PlatformNativePointerCaptureOwner::Window(
                    handle.window_id(),
                ));
            cx.simulate_platform_input_with_native_pointer_physical_frame(
                handle,
                PlatformInput::MouseUp(MouseUpEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                }),
                Some(physical),
            );
        } else {
            visual.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
        }
        frames(&mut visual, &dock);
        assert!(
            !dock.has_active_gesture(),
            "raw_press={raw_press}, outside={outside}"
        );
        let requested = dock.pending_content_close_plans().unwrap().len();
        assert_eq!(requested, usize::from(!outside), "raw_press={raw_press}");
        cx.set_platform_hovered_window(Some(handle));
        let second = visual
            .debug_bounds(&format!("dockspace:tab:{}", SECOND.get()))
            .unwrap()
            .center();
        visual.simulate_click(second, Modifiers::none());
        frames(&mut visual, &dock);
        assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
        cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
        cx.run_until_parked();
        assert!(dock.native_is_quiescent());
    }
}

#[open_gpui::test]
fn closing_local_windows_retires_their_released_pointer_streams(cx: &mut TestAppContext) {
    use dockspace::runtime::{
        HostPointerInput, HostPointerRoute, PointerDeviceKind, PointerEventMetadata,
        SurfacePointerCancelReason, SurfacePointerCapture, SurfacePointerEvent,
        SurfacePointerReceiverFacts,
    };

    let dock = preview_fixture(cx);
    let mut retired = Vec::new();
    for item in [SECOND, FIRST, SECOND] {
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| {
            InsetDockMount(dock.clone())
        });
        let mut visual = active_visual(window.into(), cx);
        cx.set_platform_hovered_window(Some(window.into()));
        frames(&mut visual, &dock);
        let tab = visual
            .debug_bounds(&format!("dockspace:tab:{}", item.get()))
            .unwrap()
            .center();
        visual.simulate_mouse_down(tab, MouseButton::Left, Modifiers::none());
        visual.simulate_mouse_up(tab, MouseButton::Left, Modifiers::none());
        frames(&mut visual, &dock);
        assert!(dock.with_view(|view| view.item(item).unwrap().is_active()));
        assert!(!dock.has_active_gesture());
        retired.push(window.window_id());
        visual.update(|window, cx| window.remove_window(cx));
        cx.run_until_parked();
    }
    assert!(dock.inspection().unwrap().mounts.is_empty());

    // The retired numbers can start a different device lifetime without
    // contradicting a live mouse stream in core.
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.submit_host_pointer_batch(retired.into_iter().map(|owner| {
                    HostPointerInput::new(
                        input::local_pointer(owner),
                        SurfacePointerEvent::StreamCancelled(
                            SurfacePointerCancelReason::DeviceRemoved,
                        ),
                        HostPointerRoute::Unknown,
                        HostPointerRoute::Unknown,
                        SurfacePointerCapture::None,
                        SurfacePointerReceiverFacts::unknown(),
                    )
                    .with_metadata(PointerEventMetadata::untimed(PointerDeviceKind::Pen))
                }))?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
}

#[open_gpui::test]
fn a_hidden_window_finishes_its_content_press_without_another_frame(cx: &mut TestAppContext) {
    use open_gpui::{
        PlatformFrameDeliveryMode, PointerCancelEvent, PointerCancelReason, WindowVisibility,
    };

    for cancel in [false, true] {
        let dock = preview_fixture(cx);
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| {
            InsetDockMount(dock.clone())
        });
        let mut visual = active_visual(window.into(), cx);
        cx.set_platform_hovered_window(Some(window.into()));
        frames(&mut visual, &dock);
        let point = pane_center(&dock, &mut visual, FIRST);
        visual.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
        frames(&mut visual, &dock);
        assert!(
            dock.host.borrow().mounts[&window.window_id()]
                .primary_press
                .is_some()
        );
        assert!(dock.host.borrow().inputs.is_empty());

        cx.set_window_frame_delivery_mode(window.into(), PlatformFrameDeliveryMode::Deferred);
        cx.simulate_window_visibility_change(window.into(), WindowVisibility::Hidden);
        if cancel {
            visual.simulate_event(PointerCancelEvent {
                reason: PointerCancelReason::CaptureRevoked,
            });
        } else {
            visual.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
        }
        cx.run_until_parked();
        assert_eq!(
            dock.inspection().unwrap().queued_input_count,
            0,
            "cancel={cancel}"
        );
        cx.update(|cx| dock.refresh_tab_content(FIRST, cx)).unwrap();
        visual.update(|window, cx| window.remove_window(cx));
    }
}

#[open_gpui::test]
fn retiring_content_source_keeps_its_committed_pointer_terminal_in_order(cx: &mut TestAppContext) {
    use open_gpui::{
        PlatformFrameDeliveryMode, PlatformInput, PointerCancelEvent, PointerCancelReason,
    };

    use super::content_receivers::content_fixture_layout;

    let sibling = SurfaceId::new(52);
    let other = ItemId::new(63);
    for (terminal, borrowed, close) in [
        (None, false, false),
        (None, false, true),
        (Some(false), false, false),
        (Some(true), false, true),
        (Some(false), true, false),
        (Some(true), true, true),
    ] {
        let surfaces = [
            (SURFACE, DockspaceNode::tabs([FIRST])),
            (sibling, DockspaceNode::tabs([SECOND, other])),
        ]
        .map(|(surface, node)| {
            DockspaceSurfaceLayout::new(
                surface,
                DockspaceRootLayout::new(RootId::new(surface.get()), node),
            )
        });
        let (dock, _) = content_fixture_layout(cx, false, surfaces, &[FIRST, SECOND, other]);
        let [(source_window, mounted), (target_window, _)] = [SURFACE, sibling].map(|surface| {
            let mounted = Rc::new(Cell::new(true));
            cx.defer_next_window_frame_requests();
            let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| OptionalDockMount {
                dock: dock.clone(),
                surface,
                mounted: mounted.clone(),
            });
            (open_gpui::AnyWindowHandle::from(window), mounted)
        });
        let mut target = VisualTestContext::from_window(target_window, cx);
        frames(&mut target, &dock);
        let mut source = active_visual(source_window, cx);
        cx.set_platform_hovered_window(Some(source_window));
        frames(&mut source, &dock);
        let point = pane_center(&dock, &mut source, FIRST);
        source.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
        frames(&mut source, &dock);
        assert!(!dock.has_active_gesture());
        assert!(source.update(|window, _| window.captured_pointer().is_none()));
        assert!(
            dock.host.borrow().mounts[&source_window.window_id()]
                .primary_press
                .is_some()
        );
        assert!(dock.host.borrow().inputs.is_empty());

        cx.set_window_frame_delivery_mode(target_window, PlatformFrameDeliveryMode::Deferred);
        if borrowed {
            source.update(|window, cx| {
                let event = match terminal {
                    Some(true) => PlatformInput::PointerCanceled(PointerCancelEvent {
                        reason: PointerCancelReason::CaptureRevoked,
                    }),
                    _ => PlatformInput::MouseUp(MouseUpEvent {
                        position: point,
                        button: MouseButton::Left,
                        modifiers: Modifiers::none(),
                        click_count: 1,
                    }),
                };
                window.dispatch_event(event, cx);
                assert!(
                    dock.host
                        .borrow()
                        .inputs
                        .iter()
                        .any(|input| input.is_required_pointer_terminal()),
                    "terminal={terminal:?}",
                );
                target_window
                    .update(cx, |_, window, cx| {
                        let _ = window.draw(cx);
                    })
                    .unwrap();
            });
        } else {
            cx.simulate_window_resize(target_window, size(px(644.0), px(412.0)));
        }
        assert!(target.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        let pending = cx.pending_window_frame_submission(target_window).unwrap();
        assert!(dock.is_update_pending());
        if borrowed {
            assert!(
                dock.host.borrow().mounts[&target_window.window_id()]
                    .pending
                    .as_ref()
                    .unwrap()
                    .inputs
                    .iter()
                    .any(|input| input.is_required_pointer_terminal())
            );
        } else if let Some(cancel) = terminal {
            if cancel {
                source.simulate_event(PointerCancelEvent {
                    reason: PointerCancelReason::CaptureRevoked,
                });
            } else {
                source.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
            }
            assert!(
                dock.host
                    .borrow()
                    .inputs
                    .iter()
                    .any(|input| input.is_required_pointer_terminal())
            );
        }
        source.update(|window, cx| {
            if close {
                window.remove_window(cx);
            } else {
                mounted.set(false);
                let _ = window.draw(cx);
                assert!(!dock.host.borrow().mounts[&source_window.window_id()].attached);
            }
        });
        cx.update(|_| assert!(pending.complete(PlatformWindowPresentOutcome::Submitted)));
        cx.set_window_frame_delivery_mode(target_window, PlatformFrameDeliveryMode::Synchronous);
        let mut target = active_visual(target_window, cx);
        cx.set_platform_hovered_window(Some(target_window));
        frames(&mut target, &dock);
        for item in [other, SECOND] {
            let tab = target
                .debug_bounds(&format!("dockspace:tab:{}", item.get()))
                .unwrap()
                .center();
            target.simulate_click(tab, Modifiers::none());
            frames(&mut target, &dock);
            assert!(dock.with_view(|view| view.item(item).unwrap().is_active()));
        }
        if !close {
            source.update(|window, cx| window.remove_window(cx));
        }
        target.update(|window, cx| window.remove_window(cx));
    }
}

#[open_gpui::test]
fn an_embedded_surface_can_mount_in_another_open_window_after_detaching(cx: &mut TestAppContext) {
    let (dock, _) = overflowing_surfaces(&[SURFACE], cx);
    let [(source, source_mounted), (destination, destination_mounted)] =
        [true, false].map(|mounted| {
            let mounted = Rc::new(Cell::new(mounted));
            cx.defer_next_window_frame_requests();
            let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| OptionalDockMount {
                dock: dock.clone(),
                surface: SURFACE,
                mounted: mounted.clone(),
            });
            (window, mounted)
        });
    let mut source_visual = active_visual(source.into(), cx);
    let mut destination_visual = VisualTestContext::from_window(destination.into(), cx);
    frames(&mut source_visual, &dock);
    frames(&mut destination_visual, &dock);
    assert!(source_visual.debug_bounds("dockspace:tab:2").is_some());

    source_mounted.set(false);
    frames(&mut source_visual, &dock);
    assert!(
        dock.inspection()
            .unwrap()
            .mounts
            .iter()
            .all(|mount| !mount.attached)
    );
    destination_mounted.set(true);
    let mut destination_visual = active_visual(destination.into(), cx);
    cx.set_platform_hovered_window(Some(destination.into()));
    frames(&mut destination_visual, &dock);
    assert!(
        cx.windows()
            .iter()
            .any(|window| window.window_id() == source.window_id())
    );
    let tab = destination_visual
        .debug_bounds("dockspace:tab:2")
        .unwrap()
        .center();
    destination_visual.simulate_click(tab, Modifiers::none());
    frames(&mut destination_visual, &dock);
    assert!(dock.with_view(|view| view.item(ItemId::new(2)).unwrap().is_active()));
}

#[open_gpui::test]
fn handing_off_one_of_two_regions_works_in_either_paint_order(cx: &mut TestAppContext) {
    struct Regions {
        dock: DockSurface,
        regions: Rc<RefCell<Vec<DockRegion>>>,
    }

    impl Render for Regions {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let regions = self.regions.borrow();
            div().size_full().when(!regions.is_empty(), |root| {
                root.child(self.dock.regions(regions.iter().copied()))
            })
        }
    }

    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let sibling = SurfaceId::new(52);
    let region =
        |surface, y| DockRegion::new(surface, LogicalRect::new(0.0, y, 620.0, 190.0).unwrap());
    for source_first in [true, false] {
        let (dock, _) = overflowing_surfaces(&[SURFACE, sibling], cx);
        let [(source, source_regions), (target, target_regions)] = [
            vec![region(SURFACE, 0.0), region(sibling, 200.0)],
            Vec::new(),
        ]
        .map(|regions| {
            let regions = Rc::new(RefCell::new(regions));
            cx.defer_next_window_frame_requests();
            let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| Regions {
                dock: dock.clone(),
                regions: regions.clone(),
            });
            (window, regions)
        });
        let mut source_visual = VisualTestContext::from_window(source.into(), cx);
        let mut target_visual = VisualTestContext::from_window(target.into(), cx);
        frames(&mut source_visual, &dock);
        frames(&mut target_visual, &dock);
        source_regions.borrow_mut().clear();
        frames(&mut source_visual, &dock);

        *source_regions.borrow_mut() = vec![region(sibling, 0.0)];
        *target_regions.borrow_mut() = vec![region(SURFACE, 0.0)];
        let [first, second] = if source_first {
            [&mut source_visual, &mut target_visual]
        } else {
            [&mut target_visual, &mut source_visual]
        };
        frames(first, &dock);
        frames(second, &dock);
        assert!(source_visual.debug_bounds("dockspace:tab:42").is_some());
        assert!(target_visual.debug_bounds("dockspace:tab:2").is_some());
        assert_eq!(dock.host.borrow().owners[&sibling], source.window_id());
        assert_eq!(dock.host.borrow().owners[&SURFACE], target.window_id());
        source_visual.update(|window, cx| window.remove_window(cx));
        target_visual.update(|window, cx| window.remove_window(cx));
    }
}

#[open_gpui::test]
fn settling_an_abandoned_destination_frame_unblocks_other_windows(cx: &mut TestAppContext) {
    use dockspace::close::CloseDecision;
    use open_gpui::PlatformFrameDeliveryMode;

    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let sibling_surface = SurfaceId::new(52);
    for outcome in [
        PlatformWindowPresentOutcome::Rejected,
        PlatformWindowPresentOutcome::RepaintRequired,
        PlatformWindowPresentOutcome::Submitted,
    ] {
        let (dock, _) = overflowing_surfaces(&[SURFACE, sibling_surface], cx);
        let [
            (source, source_mounted),
            (target, target_mounted),
            (sibling, _),
        ] = [(SURFACE, true), (SURFACE, false), (sibling_surface, true)].map(
            |(surface, mounted)| {
                let mounted = Rc::new(Cell::new(mounted));
                cx.defer_next_window_frame_requests();
                let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| OptionalDockMount {
                    dock: dock.clone(),
                    surface,
                    mounted: mounted.clone(),
                });
                (window, mounted)
            },
        );
        let mut source_visual = VisualTestContext::from_window(source.into(), cx);
        let mut target_visual = VisualTestContext::from_window(target.into(), cx);
        let mut sibling_visual = active_visual(sibling.into(), cx);
        cx.set_platform_hovered_window(Some(sibling.into()));
        frames(&mut source_visual, &dock);
        frames(&mut target_visual, &dock);
        frames(&mut sibling_visual, &dock);

        let item = ItemId::new(2);
        cx.update(|cx| {
            dock.update(
                |frame| {
                    frame.request_close_item_current(item)?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        let request = dock.pending_content_close_plans().unwrap()[0].request();
        source_mounted.set(false);
        frames(&mut source_visual, &dock);
        assert!(!dock.host.borrow().owners.contains_key(&SURFACE));
        target_mounted.set(true);
        cx.set_window_frame_delivery_mode(target.into(), PlatformFrameDeliveryMode::Deferred);
        assert!(target_visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        let original = cx.pending_window_frame_submission(target.into()).unwrap();
        assert!(dock.is_update_pending());
        assert!(!dock.host.borrow().owners.contains_key(&SURFACE));

        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        cx.update(|cx| {
            dock.set_close_handler(
                move |dock, received, cx| {
                    assert_eq!(received.request(), request);
                    observed.set(observed.get() + 1);
                    dock.update(
                        |frame| {
                            frame.resolve_close(
                                request,
                                received.items()[0].token(),
                                CloseDecision::Allow,
                            )?;
                            Ok(())
                        },
                        cx,
                    )
                    .unwrap();
                },
                cx,
            )
            .unwrap();
        });
        cx.run_until_parked();
        assert_eq!(calls.get(), 0);

        target_mounted.set(false);
        target_visual.update(|window, _| window.refresh());
        cx.update(|_| assert!(original.complete(outcome)));
        cx.run_until_parked();
        assert!(!dock.is_update_pending(), "{outcome:?}");
        assert_eq!(calls.get(), 1, "{outcome:?}");
        assert!(dock.with_view(|view| view.item(item).is_none()));
        cx.set_window_frame_delivery_mode(target.into(), PlatformFrameDeliveryMode::Synchronous);
        frames(&mut target_visual, &dock);
        assert!(!dock.host.borrow().owners.contains_key(&SURFACE));
        assert!(!dock.host.borrow().mounts[&target.window_id()].attached);

        frames(&mut sibling_visual, &dock);
        let tab = sibling_visual
            .debug_bounds("dockspace:tab:42")
            .unwrap()
            .center();
        sibling_visual.simulate_click(tab, Modifiers::none());
        frames(&mut sibling_visual, &dock);
        assert!(dock.with_view(|view| view.item(ItemId::new(42)).unwrap().is_active()));
        assert_eq!(calls.get(), 1);
        sibling_visual.update(|window, cx| window.remove_window(cx));
        source_visual.update(|window, cx| window.remove_window(cx));
        target_visual.update(|window, cx| window.remove_window(cx));
    }
}

#[open_gpui::test]
fn removing_a_sibling_with_an_unaccepted_update_resumes_close_delivery(cx: &mut TestAppContext) {
    use dockspace::close::CloseDecision;
    use open_gpui::PlatformFrameDeliveryMode;

    let sibling = SurfaceId::new(52);
    let (dock, _) = overflowing_surfaces(&[SURFACE, sibling], cx);
    let [source, target] = [SURFACE, sibling].map(|surface| {
        cx.defer_next_window_frame_requests();
        cx.open_window(size(px(620.0), px(400.0)), |_, _| OptionalDockMount {
            dock: dock.clone(),
            surface,
            mounted: Rc::new(Cell::new(true)),
        })
    });
    frames(&mut active_visual(source.into(), cx), &dock);
    let mut target_visual = VisualTestContext::from_window(target.into(), cx);
    frames(&mut target_visual, &dock);
    let item = ItemId::new(2);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.request_close_item_current(item)?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    let plan = dock.pending_content_close_plans().unwrap().pop().unwrap();
    cx.set_window_frame_delivery_mode(target.into(), PlatformFrameDeliveryMode::Deferred);
    cx.simulate_window_resize(target.into(), size(px(700.0), px(470.0)));
    assert!(target_visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    assert!(dock.is_update_pending());
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    cx.update(|cx| {
        dock.set_close_handler(
            move |dock, received, cx| {
                assert_eq!(received.request(), plan.request());
                observed.set(observed.get() + 1);
                dock.update(
                    |frame| {
                        frame.resolve_close(
                            received.request(),
                            received.items()[0].token(),
                            CloseDecision::Allow,
                        )?;
                        Ok(())
                    },
                    cx,
                )
                .unwrap();
            },
            cx,
        )
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(calls.get(), 0);

    target_visual.update(|window, cx| window.remove_window(cx));
    cx.run_until_parked();
    assert!(!dock.is_update_pending());
    assert_eq!(calls.get(), 1);
    assert!(dock.with_view(|view| view.item(item).is_none()));
}

fn retire_window_with_borrowed_input(cx: &mut TestAppContext, retirement: BorrowedInputRetirement) {
    use dockspace::runtime::DockspaceEventKind;

    let retire_source = !matches!(retirement, BorrowedInputRetirement::CloseBorrower);
    let remount = matches!(
        retirement,
        BorrowedInputRetirement::UnmountSource { remount: true, .. }
    );
    let unmount = matches!(retirement, BorrowedInputRetirement::UnmountSource { .. });
    let renderer_submitted = matches!(
        retirement,
        BorrowedInputRetirement::CloseSource {
            renderer_submitted: true
        } | BorrowedInputRetirement::UnmountSource {
            renderer_submitted: true,
            ..
        }
    );
    let sibling = SurfaceId::new(52);
    let (dock, _) = overflowing_surfaces(&[SURFACE, sibling], cx);
    let [(first, source_mounted), (second, _)] = [SURFACE, sibling].map(|surface| {
        let mounted = Rc::new(Cell::new(true));
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| OptionalDockMount {
            dock: dock.clone(),
            surface,
            mounted: mounted.clone(),
        });
        (window, mounted)
    });
    let mut source = active_visual(first.into(), cx);
    let mut target = VisualTestContext::from_window(second.into(), cx);
    frames(&mut source, &dock);
    frames(&mut target, &dock);
    assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
    cx.set_platform_hovered_window(Some(first.into()));
    let tab = source.debug_bounds("dockspace:tab:2").unwrap().center();
    let sibling_action = |item| {
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
                    if matches!(control.kind, ControlKind::Tab(tab) if tab == ItemId::new(item).into()) =>
                {
                    Some(control.slot(ControlAction::Activate)?.borrow().as_ref()?.intent.clone())
                }
                _ => None,
            })
            .unwrap()
    };
    let queued_selection = retire_source.then(|| sibling_action(42));
    let later_selection = unmount.then(|| sibling_action(43));
    let selections = Rc::new(RefCell::new(Vec::new()));
    if unmount {
        let selections = selections.clone();
        cx.update(|cx| {
            dock.set_commit_handler(
                move |_, commit, _| {
                    selections.borrow_mut().extend(
                        commit.notification().events().iter().filter_map(|event| {
                            match event.kind() {
                                DockspaceEventKind::SelectionCommitted { tab } => Some(*tab),
                                _ => None,
                            }
                        }),
                    );
                },
                cx,
            )
            .unwrap();
        });
    }
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
        if let Some(action) = queued_selection {
            input::queue_action(&mut dock.host.borrow_mut(), second.window_id(), action, cx);
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
        assert_eq!(borrowed.len(), if retire_source { 3 } else { 2 });
        assert!(
            borrowed[..2]
                .iter()
                .all(|input| input.owner() == first.window_id())
        );
    }
    let surviving_window = if retire_source {
        source.update(|window, cx| {
            if let Some(action) = later_selection {
                input::queue_action(&mut dock.host.borrow_mut(), second.window_id(), action, cx);
            }
            if renderer_submitted {
                assert!(pending.complete(PlatformWindowPresentOutcome::Submitted));
                // The receipt is queued behind this update and the core candidate
                // still belongs to the surviving window.
                assert!(dock.is_update_pending());
            }
            if unmount {
                source_mounted.set(false);
                let _ = window.draw(cx);
                assert!(!dock.host.borrow().mounts[&first.window_id()].attached);
                if remount {
                    source_mounted.set(true);
                    let _ = window.draw(cx);
                    let host = dock.host.borrow();
                    assert!(host.mounts[&first.window_id()].waiting_for_update);
                    assert!(!host.owners.contains_key(&SURFACE));
                    drop(host);
                    assert!(dock.is_update_pending());
                }
            } else {
                window.remove_window(cx);
            }
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
    if retire_source {
        assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
        assert!(dock.with_view(|view| {
            view.item(ItemId::new(if unmount { 43 } else { 42 }))
                .unwrap()
                .is_active()
        }));
    } else {
        assert!(dock.with_view(|view| view.item(ItemId::new(2)).unwrap().is_active()));
    }
    assert!(dock.host.borrow().inputs.is_empty());
    if unmount {
        assert_eq!(
            *selections.borrow(),
            [ItemId::new(42).into(), ItemId::new(43).into()]
        );
    }
    if remount {
        frames(&mut source, &dock);
        assert_eq!(dock.host.borrow().owners[&SURFACE], first.window_id());
        let tab = source.debug_bounds("dockspace:tab:2").unwrap().center();
        source.simulate_click(tab, Modifiers::none());
        frames(&mut source, &dock);
        assert!(dock.with_view(|view| view.item(ItemId::new(2)).unwrap().is_active()));
    }
}
