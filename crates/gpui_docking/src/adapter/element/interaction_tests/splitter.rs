//! Splitter pointer resets use the original ordered press/release stream.

use dockspace::geometry::PaneSizing;

use super::*;
use crate::adapter::SplitterDoubleClick;

fn split_fixture(
    cx: &mut TestAppContext,
    reset: Option<SplitterDoubleClick>,
    preferred: bool,
    managed: bool,
) -> DockSurface {
    cx.update(|cx| {
        let panes = [FIRST, SECOND].map(|item| {
            let view = cx.new(|cx| Pane {
                focus: cx.focus_handle(),
            });
            let sizing = if preferred && item == FIRST {
                PaneSizing::builder()
                    .min_width(120.0)
                    .min_height(90.0)
                    .preferred_width(280.0)
                    .build()
                    .unwrap()
            } else {
                LogicalSize::new(120.0, 90.0).unwrap().into()
            };
            DockPane::new(item, format!("Pane {}", item.get()), view).with_sizing(sizing)
        });
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(
                RootId::new(71),
                DockspaceNode::split(
                    DockspaceAxis::Horizontal,
                    [
                        (DockspaceNode::tabs([FIRST]), 1.0),
                        (DockspaceNode::tabs([SECOND]), 3.0),
                    ],
                )
                .unwrap(),
            ),
        )])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let mut config = GpuiDockspaceConfig::default();
        config.input.splitter_double_click = reset;
        if managed {
            DockSurface::new_managed(session, panes, config, cx)
        } else {
            DockSurface::new(session, panes, config, cx)
        }
        .unwrap()
    })
}

fn click(visual: &mut VisualTestContext, position: open_gpui::Point<Pixels>, count: usize) {
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position,
        modifiers: Modifiers::none(),
        click_count: count,
        first_mouse: false,
    });
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position,
        modifiers: Modifiers::none(),
        click_count: count,
    });
}

#[open_gpui::test]
fn splitter_double_click_equalizes_after_both_original_releases(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), true, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    click(&mut visual, before.center(), 1);
    frames(&mut visual, &dock);
    assert_eq!(
        receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter),
        before
    );
    click(&mut visual, before.center(), 2);
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!(after.center().x > before.center().x + px(150.0));
    assert!((after.center().x - px(400.0)).abs() < px(2.0));
    assert!(
        !dock.has_active_gesture(),
        "both original releases must commit"
    );
}

#[open_gpui::test]
fn splitter_double_click_restores_preferred_extents(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Preferred), true, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    click(&mut visual, before.center(), 2);
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!(after.center().x > before.center().x + px(50.0));
    let content_width = {
        let host = dock.host.borrow();
        host.mounts[&window.window_id()].regions[&SURFACE]
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Pane(FIRST, geometry) => Some(geometry.layout_bounds().width()),
                _ => None,
            })
            .unwrap()
    };
    assert!((content_width - 280.0).abs() < 0.1, "{content_width}");
    assert!(!dock.has_active_gesture());
}

#[open_gpui::test]
fn disabling_splitter_double_click_preserves_user_proportions(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, None, false, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    click(&mut visual, before.center(), 2);
    frames(&mut visual, &dock);
    assert_eq!(
        receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter),
        before
    );
    assert!(!dock.has_active_gesture());
}

#[open_gpui::test]
fn a_drag_with_double_click_count_never_resets_the_splitter(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), false, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    let start = before.center();
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: start,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    frames(&mut visual, &dock);
    let moved = start + point(px(40.0), px(0.0));
    visual.simulate_mouse_move(moved, Some(MouseButton::Left), Modifiers::none());
    visual.simulate_mouse_move(start, Some(MouseButton::Left), Modifiers::none());
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: start,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!(
        after.center().x < px(300.0),
        "a dragged click must not equalize"
    );
    assert!(!dock.has_active_gesture());
}

#[open_gpui::test]
fn managed_splitter_double_click_follows_its_original_native_pointer_prefix(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Preferred), false, true);
    let window = cx
        .update(|cx| dock.open_native_root(SURFACE, true, Default::default(), cx))
        .unwrap();
    let mut visual = active_visual(window, cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    click(&mut visual, before.center(), 2);
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!((after.center().x - before.center().x) > px(100.0));
    assert!(!dock.has_active_gesture());
    assert!(dock.native_transport().unwrap().is_empty());
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn splitter_reset_retains_its_release_and_action_while_presentation_is_deferred(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), false, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!((before.center().x - px(200.0)).abs() < px(2.0));
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    click(&mut visual, before.center(), 2);
    frames(&mut visual, &dock);
    let version = dock.host.borrow().session.version();
    frames(&mut visual, &dock);
    assert_eq!(dock.host.borrow().session.version(), version);
    assert_eq!(
        receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter),
        before
    );
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!((after.center().x - px(400.0)).abs() < px(2.0));
    assert!(!dock.has_active_gesture());
    let committed = dock.host.borrow().session.version();
    assert_ne!(committed, version);
    frames(&mut visual, &dock);
    assert_eq!(dock.host.borrow().session.version(), committed);
}

#[open_gpui::test]
fn cancelling_the_original_splitter_press_revokes_its_pointer_reset(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), false, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: before.center(),
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    frames(&mut visual, &dock);
    visual.update(|window, cx| {
        window.cancel_pointer_session(open_gpui::PointerCancelReason::CaptureRevoked, cx);
    });
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: before.center(),
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    frames(&mut visual, &dock);
    assert_eq!(
        receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter),
        before
    );
    assert!(!dock.has_active_gesture());
}

struct CoveredRoot {
    dock: DockSurface,
    covered: Rc<Cell<bool>>,
}

impl Render for CoveredRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .child(self.dock.element(SURFACE))
            .when(self.covered.get(), |root| {
                root.child(
                    div()
                        .id("splitter-release-cover")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .occlude(),
                )
            })
    }
}

#[open_gpui::test]
fn a_covering_overlay_blocks_reset_but_preserves_the_original_release(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), false, false);
    let covered = Rc::new(Cell::new(false));
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| CoveredRoot {
        dock: dock.clone(),
        covered: covered.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: before.center(),
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    visual.update(|window, _| {
        covered.set(true);
        window.refresh();
    });
    frames(&mut visual, &dock);
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: before.center(),
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    frames(&mut visual, &dock);
    assert_eq!(
        receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter),
        before
    );
    assert!(!dock.has_active_gesture());
}

#[open_gpui::test]
fn splitter_double_click_with_pointer_jitter_still_equalizes_exactly(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), false, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: before.center(),
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    let release = before.center() + point(px(1.0), px(0.0));
    visual.simulate_mouse_move(release, Some(MouseButton::Left), Modifiers::none());
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: release,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!((after.center().x - px(400.0)).abs() < px(0.1), "{after:?}");
    assert!(!dock.has_active_gesture());
}

#[open_gpui::test]
fn splitter_reset_survives_a_presented_resize_from_the_same_double_click(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = split_fixture(cx, Some(SplitterDoubleClick::Equalize), false, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: before.center(),
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    frames(&mut visual, &dock);
    let release = before.center() + point(px(1.0), px(0.0));
    visual.simulate_mouse_move(release, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: release,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!((after.center().x - px(400.0)).abs() < px(0.1), "{after:?}");
    assert!(!dock.has_active_gesture());
}
