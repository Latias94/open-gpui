//! Real scroll callbacks route overflowing chrome without taking pane input.

use open_gpui::{ScrollDelta, ScrollEvent, ScrollSource, ScrollWheelIntent, TouchPhase, point};

use crate::adapter::DockRegion;

use super::*;

struct ScrollPane(Rc<Cell<usize>>);

impl Render for ScrollPane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let scrolls = self.0.clone();
        div().size_full().on_scroll(move |_, _, _| {
            scrolls.set(scrolls.get() + 1);
            open_gpui::ScrollWheelIntent::handled().stop_propagation()
        })
    }
}

struct RegionsRoot {
    dock: DockSurface,
    regions: Vec<DockRegion>,
    scrolls: Rc<Cell<usize>>,
}

impl Render for RegionsRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let scrolls = self.scrolls.clone();
        div()
            .size_full()
            .child(self.dock.regions(self.regions.clone()))
            .on_scroll(move |_, _, _| {
                scrolls.set(scrolls.get() + 1);
                ScrollWheelIntent::allow_default()
            })
    }
}

struct OverlayRoot {
    dock: DockSurface,
    overlay: bool,
    scrolls: Rc<Cell<usize>>,
}

impl Render for OverlayRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let scrolls = self.scrolls.clone();
        div()
            .relative()
            .size_full()
            .child(self.dock.element(SURFACE))
            .when(self.overlay, |root| {
                root.child(
                    div()
                        .id("wheel-overlay")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(GpuiDockspaceConfig::default().colors.background)
                        .occlude()
                        .on_scroll(move |_, _, _| {
                            scrolls.set(scrolls.get() + 1);
                            // Keep dispatching so only actual hit facts can
                            // prevent the covered chrome from scrolling.
                            ScrollWheelIntent::allow_default()
                        }),
                )
            })
    }
}

fn overflowing(cx: &mut TestAppContext) -> (DockSurface, Rc<Cell<usize>>) {
    overflowing_surfaces(&[SURFACE], cx)
}

fn overflowing_surfaces(
    surfaces: &[SurfaceId],
    cx: &mut TestAppContext,
) -> (DockSurface, Rc<Cell<usize>>) {
    cx.update(|cx| {
        let scrolls = Rc::new(Cell::new(0));
        let items = (1..=40 * surfaces.len() as u64)
            .map(ItemId::new)
            .collect::<Vec<_>>();
        let panes = items
            .iter()
            .map(|&item| {
                let view = cx.new(|_| ScrollPane(scrolls.clone()));
                DockPane::new(item, format!("Tab number {}", item.get()), view)
            })
            .collect::<Vec<_>>();
        let layout = DockspaceLayout::new(surfaces.iter().zip(items.chunks(40)).map(
            |(surface, items)| {
                DockspaceSurfaceLayout::new(
                    *surface,
                    DockspaceRootLayout::new(
                        RootId::new(surface.get()),
                        DockspaceNode::tabs(items.iter().copied()),
                    ),
                )
            },
        ))
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, scrolls)
    })
}

fn offsets(dock: &DockSurface, surface: SurfaceId) -> (f64, Option<f64>) {
    let mut host = dock.host.borrow_mut();
    let mut frame = host.session.begin_host_frame().unwrap();
    let plan = frame.paint_plan(surface).unwrap().unwrap();
    let mut offsets = (0.0, None);
    plan.visit_surface_program(|step| {
        match step {
            dockspace::runtime::SurfacePaintProgramStep::Operation {
                operation: dockspace::runtime::SurfacePaintProgramOperation::TabBar(bar),
                ..
            } => offsets.0 = bar.scroll_offset(),
            dockspace::runtime::SurfacePaintProgramStep::PopupMenu { menu, .. } => {
                offsets.1 = Some(menu.scroll_offset());
            }
            _ => {}
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    offsets
}

fn target(
    dock: &DockSurface,
    window: open_gpui::WindowId,
    surface: SurfaceId,
    menu: bool,
) -> Bounds<Pixels> {
    let host = dock.host.borrow();
    let region = &host.mounts[&window].regions[&surface];
    let rect = region
        .packet
        .as_ref()
        .unwrap()
        .draws
        .iter()
        .find_map(|draw| match draw {
            Draw::Scroll(super::super::wheel::ScrollTarget::Menu(receiver)) if menu => {
                Some(receiver.bounds())
            }
            Draw::Scroll(super::super::wheel::ScrollTarget::Strip(receiver)) if !menu => {
                Some(receiver.bounds())
            }
            _ => None,
        })
        .unwrap();
    to_gpui(rect, region.bounds.unwrap())
}

fn scroll(
    position: open_gpui::Point<Pixels>,
    delta: ScrollDelta,
    phase: TouchPhase,
) -> ScrollEvent {
    ScrollEvent {
        position,
        delta,
        modifiers: Modifiers::none(),
        touch_phase: phase,
        source: ScrollSource::Pointer,
    }
}

#[open_gpui::test]
fn wheel_browses_strip_and_tab_list_without_selecting_or_scrolling_content(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let selected = dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active());
    assert!(selected);
    let strip = target(&dock, window.window_id(), SURFACE, false);
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    visual.simulate_event(scroll(
        strip.center(),
        ScrollDelta::Lines(point(0.0, -3.0)),
        TouchPhase::Moved,
    ));
    frames(&mut visual, &dock);
    assert!(
        offsets(&dock, SURFACE).0 > 0.0,
        "the actual core strip must scroll"
    );
    assert_eq!(content_scrolls.get(), 0);
    assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));

    let button = tab_list_button_bounds(&dock, window.window_id());
    visual.simulate_click(button.center(), Modifiers::none());
    frames(&mut visual, &dock);
    let menu = target(&dock, window.window_id(), SURFACE, true);
    assert_eq!(offsets(&dock, SURFACE).1, Some(0.0));
    visual.simulate_event(scroll(
        menu.center(),
        ScrollDelta::Pixels(point(px(0.0), px(-100.0))),
        TouchPhase::Started,
    ));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).1.unwrap() > 0.0);
    assert_eq!(content_scrolls.get(), 0);
    assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
}

#[open_gpui::test]
fn precise_content_scroll_tail_cannot_acquire_a_tab_strip(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let content = point(px(200.0), px(300.0));
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    visual.simulate_event(scroll(content, delta, TouchPhase::Started));
    visual.simulate_event(scroll(content, ScrollDelta::default(), TouchPhase::Ended));
    assert_eq!(content_scrolls.get(), 2);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Moved));
    frames(&mut visual, &dock);
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Started));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > 0.0);
}

#[open_gpui::test]
fn precise_scroll_started_outside_all_regions_cannot_acquire_chrome(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let app_scrolls = Rc::new(Cell::new(0));
    let region = LogicalRect::new(100.0, 80.0, 440.0, 500.0).unwrap();
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| RegionsRoot {
        dock: dock.clone(),
        regions: vec![DockRegion::new(SURFACE, region)],
        scrolls: app_scrolls.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let outside = point(px(20.0), px(40.0));
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    // An unobserved precise tail carries no chrome ownership either.
    assert!(
        visual
            .simulate_event_with_dispatch_snapshot(scroll(strip, delta, TouchPhase::Moved))
            .propagated()
    );
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    // A later outside start must also revoke an existing chrome owner.
    visual.simulate_event(scroll(
        strip,
        ScrollDelta::Pixels(point(px(0.0), px(0.0))),
        TouchPhase::Started,
    ));
    let app_scrolls_before = app_scrolls.get();
    visual.simulate_event(scroll(outside, delta, TouchPhase::Started));
    visual.simulate_event(scroll(outside, ScrollDelta::default(), TouchPhase::Ended));
    assert_eq!(app_scrolls.get(), app_scrolls_before + 2);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Moved));
    frames(&mut visual, &dock);
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    assert_eq!(content_scrolls.get(), 0);
    // A coarse notch is independent of that blocked precise gesture.
    visual.simulate_event(scroll(
        strip,
        ScrollDelta::Lines(point(0.0, -3.0)),
        TouchPhase::Moved,
    ));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > 0.0);
}

#[open_gpui::test]
fn control_scroll_start_cannot_lend_its_tail_to_chrome(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    let mut started = scroll(strip, delta, TouchPhase::Started);
    started.modifiers = Modifiers::control();
    assert!(
        visual
            .simulate_event_with_dispatch_snapshot(started)
            .propagated()
    );
    visual.simulate_modifiers_change(Modifiers::none());
    visual.simulate_event(scroll(strip, ScrollDelta::default(), TouchPhase::Ended));
    assert!(
        visual
            .simulate_event_with_dispatch_snapshot(scroll(strip, delta, TouchPhase::Moved))
            .propagated()
    );
    frames(&mut visual, &dock);
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    assert_eq!(content_scrolls.get(), 0);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Started));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > 0.0);
}

#[open_gpui::test]
fn a_same_layout_reset_blocks_old_precise_tails_until_a_new_start(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let layout = dock.with_view(|view| view.to_layout());
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let scroll_visual = || {
        let host = dock.host.borrow();
        host.mounts[&window.window_id()].regions[&SURFACE]
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Scroll(super::super::wheel::ScrollTarget::Strip(receiver)) => {
                    Some(receiver.visual_id())
                }
                _ => None,
            })
            .unwrap()
    };
    let original_visual = scroll_visual();
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    visual.simulate_event(scroll(strip, delta, TouchPhase::Started));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > 0.0);
    let previous = dock.host.borrow().session.version();
    let reset = dock
        .host
        .borrow()
        .session
        .prepare_layout_reset(layout)
        .unwrap();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.submit_layout_reset(&reset)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    assert!(!previous.is_same_epoch(dock.host.borrow().session.version()));
    frames(&mut visual, &dock);
    assert_eq!(scroll_visual(), original_visual);
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let before = offsets(&dock, SURFACE).0;
    assert!(
        visual
            .simulate_event_with_dispatch_snapshot(scroll(strip, delta, TouchPhase::Moved))
            .propagated()
    );
    frames(&mut visual, &dock);
    assert_eq!(offsets(&dock, SURFACE).0, before);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Started));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > before);
    assert_eq!(content_scrolls.get(), 0);
}

#[open_gpui::test]
fn phased_scroll_owner_survives_other_region_listeners_and_cross_surface_tails(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let surfaces = [SURFACE, SurfaceId::new(52)];
    let (dock, content_scrolls) = overflowing_surfaces(&surfaces, cx);
    let app_scrolls = Rc::new(Cell::new(0));
    let regions = surfaces
        .iter()
        .enumerate()
        .map(|(index, surface)| {
            DockRegion::new(
                *surface,
                LogicalRect::new(index as f64 * 340.0, 0.0, 300.0, 600.0).unwrap(),
            )
        })
        .collect();
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| RegionsRoot {
        dock: dock.clone(),
        regions,
        scrolls: app_scrolls.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    let gap = point(px(320.0), px(200.0));
    for (index, surface) in surfaces.iter().copied().enumerate() {
        let other = surfaces[1 - index];
        let strip = target(&dock, window.window_id(), surface, false).center();
        let other_strip = target(&dock, window.window_id(), other, false).center();
        let before = offsets(&dock, surface).0;
        let other_before = offsets(&dock, other).0;
        visual.simulate_event(scroll(
            strip,
            ScrollDelta::Pixels(point(px(0.0), px(0.0))),
            TouchPhase::Started,
        ));
        assert!(
            !visual
                .simulate_event_with_dispatch_snapshot(scroll(strip, delta, TouchPhase::Moved))
                .propagated()
        );
        frames(&mut visual, &dock);
        let after = offsets(&dock, surface).0;
        assert!(after > before);
        assert_eq!(offsets(&dock, other).0, other_before);
        visual.simulate_event(scroll(gap, ScrollDelta::default(), TouchPhase::Ended));
        visual.simulate_event(scroll(other_strip, delta, TouchPhase::Moved));
        frames(&mut visual, &dock);
        assert_eq!(offsets(&dock, other).0, other_before);
        visual.simulate_event(scroll(strip, delta, TouchPhase::Moved));
        frames(&mut visual, &dock);
        assert!(offsets(&dock, surface).0 > after);
    }
    assert_eq!(content_scrolls.get(), 0);
}

#[open_gpui::test]
fn a_clamped_strip_consumes_the_edge_and_retains_precise_tail_ownership(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let clamped = ScrollDelta::Pixels(point(px(0.0), px(60.0)));
    assert!(
        !visual
            .simulate_event_with_dispatch_snapshot(scroll(strip, clamped, TouchPhase::Started))
            .propagated()
    );
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    visual.simulate_event(scroll(strip, ScrollDelta::default(), TouchPhase::Ended));
    assert!(
        !visual
            .simulate_event_with_dispatch_snapshot(scroll(strip, clamped, TouchPhase::Moved))
            .propagated()
    );
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    let reverse = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    visual.simulate_event(scroll(strip, reverse, TouchPhase::Moved));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > 0.0);
    assert_eq!(content_scrolls.get(), 0);
}

#[open_gpui::test]
fn an_actual_overlay_blocks_chrome_and_its_tail_after_the_overlay_is_removed(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, content_scrolls) = overflowing(cx);
    let overlay_scrolls = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| OverlayRoot {
        dock: dock.clone(),
        overlay: true,
        scrolls: overlay_scrolls.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let strip = target(&dock, window.window_id(), SURFACE, false).center();
    let delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    assert!(
        visual
            .simulate_event_with_dispatch_snapshot(scroll(strip, delta, TouchPhase::Started))
            .propagated()
    );
    visual.simulate_event(scroll(strip, ScrollDelta::default(), TouchPhase::Ended));
    visual.simulate_event(scroll(strip, delta, TouchPhase::Moved));
    assert_eq!(overlay_scrolls.get(), 3);
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    window
        .update(cx, |root, _, cx| {
            root.overlay = false;
            cx.notify();
        })
        .unwrap();
    frames(&mut visual, &dock);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Moved));
    assert_eq!(offsets(&dock, SURFACE).0, 0.0);
    assert_eq!(content_scrolls.get(), 0);
    visual.simulate_event(scroll(strip, delta, TouchPhase::Started));
    frames(&mut visual, &dock);
    assert!(offsets(&dock, SURFACE).0 > 0.0);
    assert_eq!(overlay_scrolls.get(), 3);
}
