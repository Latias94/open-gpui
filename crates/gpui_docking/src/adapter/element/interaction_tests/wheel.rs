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

pub(super) fn target(
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

#[open_gpui::test]
fn a_tab_list_without_popup_space_leaves_its_sibling_region_interactive(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let sibling = SurfaceId::new(52);
    let (dock, _) = overflowing_surfaces(&[SURFACE, sibling], cx);
    let window = cx.open_window(size(px(620.0), px(400.0)), |_, _| RegionsRoot {
        dock: dock.clone(),
        regions: vec![
            DockRegion::new(SURFACE, LogicalRect::new(0.0, 0.0, 620.0, 32.0).unwrap()),
            DockRegion::new(sibling, LogicalRect::new(0.0, 42.0, 620.0, 350.0).unwrap()),
        ],
        scrolls: Rc::default(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let point = |visual: &mut VisualTestContext| {
        let tree = visual.latest_accessibility_tree_update().unwrap();
        let bounds = tree.nodes.iter().find_map(|(_, node)| {
            (node.role() == Role::Tab && node.label() == Some("Tab number 42"))
                .then(|| node.bounds())
                .flatten()
        })?;
        let scale = visual.update(|window, _| window.scale_factor());
        Some(open_gpui::point(
            px((bounds.x0 + bounds.x1) as f32 / (2.0 * scale)),
            px((bounds.y0 + bounds.y1) as f32 / (2.0 * scale)),
        ))
    };
    let tab = point(&mut visual).unwrap();
    visual.simulate_click(
        tab_list_button_bounds(&dock, window.window_id()).center(),
        Modifiers::none(),
    );
    frames(&mut visual, &dock);
    assert_eq!(point(&mut visual), Some(tab));
    assert_eq!(offsets(&dock, SURFACE).1, None);
    visual.simulate_click(tab, Modifiers::none());
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(ItemId::new(42)).unwrap().is_active()));
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
fn single_short_tab_list_recovers_through_scheduled_frames(cx: &mut TestAppContext) {
    for accessibility in [false, true] {
        let (dock, _) = overflowing(cx);
        let window = cx.open_window(size(px(620.0), px(32.0)), |_, _| Root {
            dock: dock.clone(),
        });
        let mut visual = active_visual(window.into(), cx);
        assert!(visual.activate_accessibility());
        frames(&mut visual, &dock);
        let tab_count = |visual: &VisualTestContext| {
            visual
                .latest_accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .filter(|(_, node)| node.role() == Role::Tab)
                .count()
        };
        let before = tab_count(&visual);
        assert!(before > 0);
        if accessibility {
            tab_list_accessible_action(
                &visual,
                Role::Button,
                "Show all tabs",
                AccessibleAction::Click,
            );
        } else {
            let button = tab_list_button_bounds(&dock, window.window_id());
            visual.simulate_click(button.center(), Modifiers::none());
        }
        for _ in 0..10 {
            if !visual.simulate_scheduled_frame() {
                break;
            }
        }
        assert_eq!(tab_count(&visual), before);
        assert_eq!(offsets(&dock, SURFACE).1, None);
        assert!(
            !visual.frame_scheduled(),
            "popup recovery must settle after repainting"
        );
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_id()].regions[&SURFACE];
        assert!(!region.dirty && !region.packet.as_ref().unwrap().is_deferred());
        drop(host);
        assert!(dock.error().is_none());
    }
}

fn tab_list_accessible_action(
    visual: &VisualTestContext,
    role: Role,
    label: &str,
    action: AccessibleAction,
) {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let target_node = tree
        .nodes
        .iter()
        .find_map(|(id, node)| (node.role() == role && node.label() == Some(label)).then_some(*id))
        .unwrap();
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node,
            data: None,
        })
    );
}

fn scroll_tab_list_past_focus(visual: &mut VisualTestContext, dock: &DockSurface, row: u64) {
    let owner = visual.update(|window, _| window.window_handle().window_id());
    assert!(visual.activate_accessibility());
    frames(visual, dock);
    visual.simulate_click(
        tab_list_button_bounds(dock, owner).center(),
        Modifiers::none(),
    );
    frames(visual, dock);
    let name = format!("Tab number {row}");
    if row != 1 {
        tab_list_accessible_action(visual, Role::MenuItem, &name, AccessibleAction::Focus);
        frames(visual, dock);
    }
    assert_accessibility_focus(visual, Role::MenuItem, &name);
    let menu = target(dock, owner, SURFACE, true).center();
    visual.simulate_event(scroll(
        menu,
        ScrollDelta::Lines(point(0.0, -12.0)),
        TouchPhase::Moved,
    ));
    frames(visual, dock);
    let offset = offsets(dock, SURFACE).1.unwrap();
    assert!(offset > 28.0);
    assert!(
        !visual
            .latest_accessibility_tree_update()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.role() == Role::MenuItem && node.label() == Some(name.as_str()))
    );
    assert_accessibility_focus(visual, Role::Menu, "Tab list");
    visual.simulate_event(scroll(
        menu,
        ScrollDelta::Lines(point(0.0, -1.0)),
        TouchPhase::Moved,
    ));
    frames(visual, dock);
    assert!(offsets(dock, SURFACE).1.unwrap() > offset);
}

#[open_gpui::test]
fn tab_list_keeps_keyboard_focus_when_wheel_scrolling_hides_the_focused_row(
    cx: &mut TestAppContext,
) {
    for (row, navigate) in [(1, false), (1, true), (3, true)] {
        cx.defer_next_window_frame_requests();
        let (dock, _) = overflowing(cx);
        let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
            dock: dock.clone(),
        });
        let mut visual = active_visual(window.into(), cx);
        scroll_tab_list_past_focus(&mut visual, &dock, row);
        if navigate {
            visual.simulate_keystrokes("down");
            frames(&mut visual, &dock);
            assert_accessibility_focus(&visual, Role::MenuItem, &format!("Tab number {}", row + 1));
        }
        assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
        visual.simulate_keystrokes("escape");
        frames(&mut visual, &dock);
        assert_eq!(offsets(&dock, SURFACE).1, None);
        assert_accessibility_focus(&visual, Role::Button, "Show all tabs");
    }
}

#[open_gpui::test]
fn opening_a_sibling_tab_list_moves_keyboard_focus_to_its_first_row(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let sibling = SurfaceId::new(52);
    let (dock, _) = overflowing_surfaces(&[SURFACE, sibling], cx);
    let window = cx.open_window(size(px(620.0), px(600.0)), |_, _| RegionsRoot {
        dock: dock.clone(),
        regions: vec![
            DockRegion::new(SURFACE, LogicalRect::new(0.0, 0.0, 620.0, 290.0).unwrap()),
            DockRegion::new(sibling, LogicalRect::new(0.0, 300.0, 620.0, 290.0).unwrap()),
        ],
        scrolls: Rc::default(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let openers = {
        let host = dock.host.borrow();
        [SURFACE, sibling].map(|surface| {
            let region = &host.mounts[&window.window_id()].regions[&surface];
            let control = region
                .packet
                .as_ref()
                .unwrap()
                .draws
                .iter()
                .find_map(|draw| match draw {
                    Draw::Control(control) if control.opens_menu.is_some() => Some(control),
                    _ => None,
                })
                .unwrap();
            to_gpui(control.bounds, region.bounds.unwrap()).center()
        })
    };
    visual.simulate_click(openers[0], Modifiers::none());
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Tab number 1");
    visual.simulate_click(openers[1], Modifiers::none());
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Tab number 41");
    visual.simulate_keystrokes("down enter");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(ItemId::new(42)).unwrap().is_active()));
    assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
}

#[open_gpui::test]
fn tab_list_yields_keyboard_input_to_an_application_focus_claim_while_its_row_is_hidden(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = overflowing(cx);
    let focus = cx.update(|cx| cx.focus_handle());
    let keys = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| ExternalFocusRoot {
        dock: dock.clone(),
        focus: focus.clone(),
        keys: keys.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    scroll_tab_list_past_focus(&mut visual, &dock, 1);
    let offset = offsets(&dock, SURFACE).1;
    visual.update(|window, cx| window.focus(&focus, cx));
    frames(&mut visual, &dock);
    visual.simulate_keystrokes("escape down");
    frames(&mut visual, &dock);
    assert_eq!(&*keys.borrow(), &["escape", "down"]);
    assert_eq!(offsets(&dock, SURFACE).1, offset);
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
