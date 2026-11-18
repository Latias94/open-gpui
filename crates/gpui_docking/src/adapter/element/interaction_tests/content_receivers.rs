//! Declared content keeps widget input while participating in docking receiver queries.

mod continuity;

use open_gpui::{
    DevicePixels, Entity, HitboxTag, PlatformInput, PlatformNativeDragHysteresis,
    PlatformNativePointerCaptureOwner, PlatformNativePointerPhysicalFrame, PlatformWindowHit,
    PlatformWindowHitStack, PlatformWindowPhysicalCoverage, Point, ScrollDelta, ScrollEvent,
    ScrollSource, ScrollWheelIntent, TouchPhase, deferred,
};

use super::{
    input_recovery::{begin_drag, drag_feedback, pane_center},
    *,
};

pub(super) struct EditorPane {
    focus: FocusHandle,
    tag: HitboxTag,
    declared: bool,
    blocker: bool,
    popup: bool,
    text: String,
    clicks: usize,
    scrolls: Rc<Cell<usize>>,
}

impl Render for EditorPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tag = self.tag;
        let scrolls = self.scrolls.clone();
        div()
            .id("content-editor")
            .relative()
            .size_full()
            .track_focus(&self.focus)
            .when(self.declared, |editor| editor.hitbox_tag(tag))
            .on_click(cx.listener(|pane, _, _, cx| {
                pane.clicks += 1;
                cx.notify();
            }))
            .on_key_down(cx.listener(|pane, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.len() == 1 {
                    pane.text.push_str(&event.keystroke.key);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .on_scroll_wheel(move |_, _, _| {
                scrolls.set(scrolls.get() + 1);
                ScrollWheelIntent::handled().stop_propagation()
            })
            .child(self.text.clone())
            .when(self.blocker, |editor| {
                editor.child(div().absolute().size_full().occlude())
            })
            .when(self.popup, |editor| {
                // Even a matching label cannot enroll a deferred overlay in this pane's body.
                editor.child(deferred(
                    div().absolute().size_full().occlude().hitbox_tag(tag),
                ))
            })
    }
}

pub(super) fn content_fixture(
    cx: &mut TestAppContext,
    managed: bool,
) -> (DockSurface, Vec<Entity<EditorPane>>) {
    content_fixture_regions(cx, managed, false)
}

fn content_fixture_regions(
    cx: &mut TestAppContext,
    managed: bool,
    separate: bool,
) -> (DockSurface, Vec<Entity<EditorPane>>) {
    if managed {
        cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
        cx.set_platform_window_hit_stack(
            PlatformWindowHitStack::try_available_open_desktop(
                point(DevicePixels(0), DevicePixels(0)),
                vec![],
            )
            .unwrap(),
        );
        cx.set_platform_native_drag_hysteresis(Some(
            PlatformNativeDragHysteresis::try_new(DevicePixels(4), DevicePixels(4)).unwrap(),
        ));
    }
    cx.update(|cx| {
        let tag = HitboxTag::new();
        let views = [FIRST, SECOND].map(|_| {
            cx.new(|cx| EditorPane {
                focus: cx.focus_handle(),
                tag,
                declared: true,
                blocker: false,
                popup: false,
                text: String::new(),
                clicks: 0,
                scrolls: Rc::default(),
            })
        });
        let panes = [FIRST, SECOND]
            .into_iter()
            .zip(&views)
            .map(|(item, view)| {
                DockPane::new(item, format!("Pane {}", item.get()), view.clone())
                    .with_focus_handle(view.read(cx).focus.clone())
                    .with_content_receiver(tag)
            })
            .collect::<Vec<_>>();
        let content = DockspaceNode::equal_split(
            DockspaceAxis::Horizontal,
            [DockspaceNode::tabs([FIRST]), DockspaceNode::tabs([SECOND])],
        )
        .unwrap();
        let surfaces = if separate {
            [(SURFACE, FIRST), (SurfaceId::new(52), SECOND)]
                .map(|(surface, item)| {
                    DockspaceSurfaceLayout::new(
                        surface,
                        DockspaceRootLayout::new(
                            RootId::new(surface.get()),
                            DockspaceNode::tabs([item]),
                        ),
                    )
                })
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            vec![DockspaceSurfaceLayout::new(
                SURFACE,
                DockspaceRootLayout::new(RootId::new(71), content),
            )]
        };
        let layout = DockspaceLayout::new(surfaces).unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = if managed {
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx)
        } else {
            DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx)
        }
        .unwrap();
        (dock, views.into())
    })
}

fn open_content(cx: &mut TestAppContext, dock: &DockSurface, managed: bool) -> VisualTestContext {
    let window = if managed {
        cx.update(|cx| dock.open_native_root(SURFACE, true, Default::default(), cx))
            .unwrap()
    } else {
        cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
            dock: dock.clone(),
        })
        .into()
    };
    cx.set_platform_hovered_window(Some(window));
    let mut visual = active_visual(window, cx);
    if managed {
        let (bounds, scale) = visual.update(|window, _| {
            let facts = window.platform_facts();
            (
                facts.bounds.to_device_pixels(facts.scale_factor),
                facts.scale_factor,
            )
        });
        cx.set_platform_window_physical_client_geometry(window, Some(bounds), scale);
        cx.simulate_window_scale_factor_change(window, scale);
    }
    frames(&mut visual, dock);
    visual
}

fn native_input(visual: &mut VisualTestContext, position: Point<Pixels>, input: PlatformInput) {
    let (handle, geometry) = visual.update(|window, _| {
        (
            window.window_handle(),
            window.platform_facts().physical_geometry.unwrap(),
        )
    });
    let global = geometry.local_to_global(position).unwrap();
    visual.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            global,
            vec![PlatformWindowHit::RegisteredApplication {
                window: handle,
                coverage: PlatformWindowPhysicalCoverage::try_new(geometry.client_bounds())
                    .unwrap(),
                geometry,
            }],
        )
        .unwrap(),
    );
    visual.simulate_platform_input_with_native_pointer_physical_frame(
        handle,
        input,
        Some(
            PlatformNativePointerPhysicalFrame::new(global, geometry).with_capture_owner(
                PlatformNativePointerCaptureOwner::Window(handle.window_id()),
            ),
        ),
    );
}

fn move_drag(visual: &mut VisualTestContext, position: Point<Pixels>, managed: bool) {
    if managed {
        native_input(
            visual,
            position,
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
        );
    } else {
        visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
    }
}

fn start_drag(dock: &DockSurface, visual: &mut VisualTestContext, managed: bool) {
    if !managed {
        begin_drag(dock, visual, FIRST);
        return;
    }
    let pressed = visual.debug_bounds("dockspace:tab:61").unwrap().center();
    visual.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    native_input(
        visual,
        pressed,
        PlatformInput::MouseDown(MouseDownEvent {
            position: pressed,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        }),
    );
    frames(visual, dock);
    move_drag(visual, pressed + point(px(24.0), px(0.0)), true);
    frames(visual, dock);
    assert!(dock.has_active_gesture());
    assert!(visual.update(|window, _| window.captured_pointer().is_some()));
}

fn release_drag(visual: &mut VisualTestContext, position: Point<Pixels>, managed: bool) {
    if managed {
        visual.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
        native_input(
            visual,
            position,
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
        );
    } else {
        visual.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
    }
}

fn assert_feedback(dock: &DockSurface, managed: bool, expected: bool) {
    let (preview, guides, local_hover) = drag_feedback(dock);
    assert_eq!((preview, guides), (expected, expected));
    if !managed {
        assert_eq!(local_hover, expected);
    }
}

fn shutdown(cx: &mut TestAppContext, dock: &DockSurface, managed: bool) {
    if managed {
        cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
        cx.run_until_parked();
        assert!(dock.native_is_quiescent());
    }
}

fn edit_and_dock(cx: &mut TestAppContext, managed: bool) {
    cx.defer_next_window_frame_requests();
    let (dock, views) = content_fixture(cx, managed);
    let entities = views
        .iter()
        .map(|view| view.entity_id())
        .collect::<Vec<_>>();
    let mut visual = open_content(cx, &dock, managed);
    let content = pane_center(&dock, &mut visual, SECOND) + point(px(80.0), px(80.0));
    visual.simulate_mouse_move(content, None, Modifiers::none());
    frames(&mut visual, &dock);
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let frame = &host.mounts[&window.window_handle().window_id()].input_frames[0];
        assert!(
            frame
                .content_hitboxes
                .iter()
                .any(|id| id.window_point_target_rank(content, window) == Some(0))
        );
    });
    visual.simulate_click(content, Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(views[1].read_with(cx, |pane, _| pane.clicks), 1);
    assert!(visual.update(|window, cx| views[1].read(cx).focus.is_focused(window)));
    assert!(visual.update(|window, _| window.captured_pointer().is_none()));
    visual.simulate_keystrokes("x");
    visual.simulate_event(ScrollEvent {
        position: content,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-20.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    frames(&mut visual, &dock);
    assert_eq!(views[1].read_with(cx, |pane, _| pane.text.clone()), "x");
    assert_eq!(views[1].read_with(cx, |pane, _| pane.scrolls.get()), 1);
    start_drag(&dock, &mut visual, managed);
    move_drag(&mut visual, content, managed);
    frames(&mut visual, &dock);
    assert_feedback(&dock, managed, true);
    let center = visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Guide(bounds, guide)
                    if guide.direction == dockspace::runtime::DockspaceDropDirection::Center
                        && guide.eligible =>
                {
                    Some(to_gpui(*bounds, region.bounds.unwrap()).center())
                }
                _ => None,
            })
            .expect("core supplies an eligible center guide for the real content")
    });
    move_drag(&mut visual, center, managed);
    frames(&mut visual, &dock);
    release_drag(&mut visual, center, managed);
    frames(&mut visual, &dock);
    assert!(!dock.has_active_gesture());
    assert!(visual.update(|window, _| window.captured_pointer().is_none()));
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    dock.with_view(|view| {
        assert_eq!(
            view.item(FIRST).unwrap().tabs().unwrap().key(),
            view.item(SECOND).unwrap().tabs().unwrap().key()
        );
    });
    assert_eq!(views[1].read_with(cx, |pane, _| pane.text.clone()), "x");
    assert_eq!(
        views
            .iter()
            .map(|view| view.entity_id())
            .collect::<Vec<_>>(),
        entities
    );
    shutdown(cx, &dock, managed);
}

#[open_gpui::test]
fn local_content_preserves_edit_focus_scroll_and_entity_through_drop(cx: &mut TestAppContext) {
    edit_and_dock(cx, false);
}

#[open_gpui::test]
fn native_content_preserves_edit_focus_scroll_and_entity_through_drop(cx: &mut TestAppContext) {
    edit_and_dock(cx, true);
}

fn blocks_unknown_content(cx: &mut TestAppContext, managed: bool) {
    cx.defer_next_window_frame_requests();
    let (dock, views) = content_fixture(cx, managed);
    let mut visual = open_content(cx, &dock, managed);
    let content = pane_center(&dock, &mut visual, SECOND) + point(px(80.0), px(80.0));
    start_drag(&dock, &mut visual, managed);
    for mode in [0, 1, 2, 3, 0] {
        for view in &views {
            view.update(cx, |pane, cx| {
                pane.declared = mode != 1;
                pane.blocker = mode == 2;
                pane.popup = mode == 3;
                cx.notify();
            });
        }
        frames(&mut visual, &dock);
        move_drag(&mut visual, content, managed);
        frames(&mut visual, &dock);
        assert_feedback(&dock, managed, mode == 0);
        assert!(dock.has_active_gesture());
    }
    release_drag(&mut visual, content, managed);
    frames(&mut visual, &dock);
    assert!(!dock.has_active_gesture());
    shutdown(cx, &dock, managed);
}

#[open_gpui::test]
fn local_declarations_do_not_clear_unknown_children_or_deferred_popups(cx: &mut TestAppContext) {
    blocks_unknown_content(cx, false);
}

#[open_gpui::test]
fn native_declarations_do_not_clear_unknown_children_or_deferred_popups(cx: &mut TestAppContext) {
    blocks_unknown_content(cx, true);
}

struct ContentRegions(DockSurface);

impl Render for ContentRegions {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.regions([
            crate::adapter::DockRegion::new(
                SURFACE,
                LogicalRect::new(0.0, 0.0, 400.0, 600.0).unwrap(),
            ),
            crate::adapter::DockRegion::new(
                SurfaceId::new(52),
                LogicalRect::new(400.0, 0.0, 400.0, 600.0).unwrap(),
            ),
        ]))
    }
}

fn cross_region_terminals(cx: &mut TestAppContext, managed: bool) {
    cx.defer_next_window_frame_requests();
    let (dock, views) = content_fixture_regions(cx, managed, true);
    let window = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        if managed {
            dock.bind_native_root(SURFACE, true, window, cx).unwrap();
        }
        ContentRegions(dock.clone())
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let source = point(px(200.0), px(320.0));
    let target = point(px(600.0), px(320.0));
    for (cancel, committed) in [(false, true), (true, true), (false, false), (true, false)] {
        visual.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        if committed {
            frames(&mut visual, &dock);
        }
        let position = if cancel && !committed {
            point(px(-20.0), px(-20.0))
        } else {
            target
        };
        visual.simulate_mouse_move(position, Some(MouseButton::Left), Modifiers::none());
        if cancel {
            visual.simulate_event(open_gpui::PointerCancelEvent {
                reason: open_gpui::PointerCancelReason::CaptureRevoked,
            });
        } else {
            visual.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        }
        frames(&mut visual, &dock);
        assert!(!dock.has_active_gesture());
        assert!(visual.update(|window, _| window.captured_pointer().is_none()));
        // The next real click proves the original pressed-button ledger was terminated once.
        visual.simulate_click(source, Modifiers::none());
        frames(&mut visual, &dock);
    }
    assert_eq!(views[0].read_with(cx, |pane, _| pane.clicks), 4);
    shutdown(cx, &dock, managed);
}

#[open_gpui::test]
fn local_content_terminals_follow_the_press_across_logical_regions(cx: &mut TestAppContext) {
    cross_region_terminals(cx, false);
}

#[open_gpui::test]
fn native_content_terminals_are_submitted_once_across_logical_regions(cx: &mut TestAppContext) {
    cross_region_terminals(cx, true);
}
