use std::panic::{AssertUnwindSafe, catch_unwind};

use dockspace::{
    close::{CloseDecision, ClosePlanTarget},
    model::DockPlacement,
};

use super::super::presentation_menu::Scope;
use super::*;

const THIRD: ItemId = ItemId::new(63);
const FOURTH: ItemId = ItemId::new(64);
const LAYOUT: RootId = RootId::new(71);
const ITEMS: [ItemId; 4] = [FIRST, SECOND, THIRD, FOURTH];

#[derive(Default)]
struct MenuProbe {
    interaction: RefCell<Option<Rc<InteractionState>>>,
    discard_next_prepaint: Cell<bool>,
}

struct MenuRoot {
    dock: DockSurface,
    probe: Rc<MenuProbe>,
}

impl Render for MenuRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let dock = self.dock.clone();
        let probe = self.probe.clone();
        div()
            .size_full()
            .child(self.dock.element(SURFACE))
            .on_children_prepainted(move |_, window, _| {
                let id = dock.host.borrow().mounts[&window.window_handle().window_id()]
                    .element
                    .clone()
                    .unwrap();
                window.with_element_state::<Rc<InteractionState>, _>(&id, |state, _| {
                    let state = state.unwrap();
                    *probe.interaction.borrow_mut() = Some(state.clone());
                    ((), state)
                });
                assert!(
                    !probe.discard_next_prepaint.replace(false),
                    "discard the prepared menu candidate"
                );
            })
    }
}

fn split_groups(cx: &mut TestAppContext) -> (DockSurface, VisualTestContext, Rc<MenuProbe>) {
    cx.defer_next_window_frame_requests();
    let dock = cx.update(|cx| {
        let panes = ITEMS.map(|item| {
            let focus = cx.focus_handle();
            let view = cx.new(|_| Pane {
                focus: focus.clone(),
            });
            DockPane::new(item, format!("Pane {}", item.get()), view)
                .with_focus_handle(focus)
                .with_sizing(LogicalSize::new(0.0, 0.0).unwrap())
        });
        let content = DockspaceNode::split(
            DockspaceAxis::Horizontal,
            [
                (DockspaceNode::tabs([FIRST, SECOND]), 1.0),
                (DockspaceNode::tabs([THIRD, FOURTH]), 2.0),
            ],
        )
        .unwrap();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(LAYOUT, content),
        )])
        .unwrap();
        DockSurface::new(
            DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap(),
            panes,
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap()
    });
    let probe = Rc::new(MenuProbe::default());
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| MenuRoot {
        dock: dock.clone(),
        probe: probe.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    (dock, visual, probe)
}

fn anchor(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    scope: Scope,
    ordinal: usize,
) -> (Rc<PresentationMenu>, Bounds<Pixels>) {
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let menu = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::PresentationMenu(menu) if menu.enabled && menu.scope == scope => Some(menu),
                _ => None,
            })
            .nth(ordinal)
            .expect("the requested scoped menu must be painted")
            .clone();
        let bounds = to_gpui(menu.bounds, region.bounds.unwrap());
        (menu, bounds)
    })
}

fn semantic_target(visual: &VisualTestContext, role: Role, label: &str) -> accesskit::NodeId {
    visual
        .latest_accessibility_tree_update()
        .unwrap()
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == role && node.label() == Some(label))
        .min_by(|(_, left), (_, right)| {
            left.bounds()
                .unwrap()
                .x0
                .total_cmp(&right.bounds().unwrap().x0)
        })
        .map(|(target, _)| *target)
        .expect("the real accessibility tree must expose the requested control")
}

fn accessibility_action(
    visual: &VisualTestContext,
    target: accesskit::NodeId,
    action: AccessibleAction,
) -> bool {
    visual.dispatch_accessibility_action(accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    })
}

fn menu_is_open(visual: &VisualTestContext, label: &str) -> bool {
    visual
        .latest_accessibility_tree_update()
        .unwrap()
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::Menu && node.label() == Some(label))
}

fn click_row(visual: &mut VisualTestContext, label: &str) {
    let scale = visual.update(|window, _| f64::from(window.scale_factor()));
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let bounds = tree
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == Role::MenuItem && node.label() == Some(label))
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap();
    visual.simulate_click(
        point(
            px(((bounds.x0 + bounds.x1) * 0.5 / scale) as f32),
            px(((bounds.y0 + bounds.y1) * 0.5 / scale) as f32),
        ),
        Modifiers::none(),
    );
}

fn assert_sibling_group(dock: &DockSurface) {
    dock.with_view(|view| {
        for item in [THIRD, FOURTH] {
            let item = view.item(item).unwrap();
            assert_eq!(item.surface(), SURFACE);
            assert_eq!(item.root(), LAYOUT);
            assert!(item.contained().is_none());
            assert_eq!(
                item.tabs().unwrap().items().collect::<Vec<_>>(),
                [THIRD, FOURTH]
            );
        }
        assert!(view.item(THIRD).unwrap().is_active());
    });
}

#[open_gpui::test]
fn pointer_accessibility_and_keyboard_float_only_the_captured_tab_group(cx: &mut TestAppContext) {
    for input in ["pointer", "accessibility", "keyboard"] {
        let (dock, mut visual, probe) = split_groups(cx);
        let (menu, bounds) = anchor(&dock, &mut visual, Scope::Tabs, 0);
        assert!(menu.choices.is_empty());
        if input == "keyboard" {
            let focus = probe.interaction.borrow().as_ref().unwrap().focus.borrow()
                [&ControlFocusId::Visual(menu.id)]
                .clone();
            visual.update(|window, cx| window.focus(&focus, cx));
            visual.simulate_keystrokes("down");
        } else if input == "accessibility" {
            assert!(accessibility_action(
                &visual,
                semantic_target(&visual, Role::Button, "Tab group actions"),
                AccessibleAction::Click,
            ));
        } else {
            visual.simulate_click(bounds.center(), Modifiers::none());
        }
        frames(&mut visual, &dock);
        assert!(menu_is_open(&visual, "Tab group actions"));
        assert!(!menu_is_open(&visual, "Layout actions"));
        assert_eq!(
            super::selected_presentation_menu_row(&visual).as_deref(),
            Some("Float Tab Group")
        );
        match input {
            "pointer" => click_row(&mut visual, "Float Tab Group"),
            "accessibility" => assert!(accessibility_action(
                &visual,
                semantic_target(&visual, Role::MenuItem, "Float Tab Group"),
                AccessibleAction::Click,
            )),
            "keyboard" => visual.simulate_keystrokes("enter"),
            _ => unreachable!(),
        }
        frames(&mut visual, &dock);
        assert!(!menu_is_open(&visual, "Tab group actions"));
        assert!(!dock.has_active_gesture());
        dock.with_view(|view| {
            let first = view.item(FIRST).unwrap();
            let second = view.item(SECOND).unwrap();
            assert!(first.contained().is_some(), "{input}");
            assert_eq!(first.contained(), second.contained());
            assert_eq!(first.root(), second.root());
            assert_ne!(first.root(), LAYOUT);
            assert_eq!(first.surface(), SURFACE);
            assert_eq!(second.surface(), SURFACE);
            assert_eq!(
                first.tabs().unwrap().items().collect::<Vec<_>>(),
                [FIRST, SECOND]
            );
            assert!(first.is_active());
            assert_eq!(view.surface(SURFACE).unwrap().contained_count(), 1);
            assert_eq!(
                view.root(LAYOUT)
                    .unwrap()
                    .content()
                    .unwrap()
                    .tabs()
                    .unwrap()
                    .items()
                    .collect::<Vec<_>>(),
                [THIRD, FOURTH]
            );
        });
        assert_sibling_group(&dock);
    }
}

#[open_gpui::test]
fn root_float_preserves_both_tab_groups_and_the_complete_split(cx: &mut TestAppContext) {
    let (dock, mut visual, _) = split_groups(cx);
    let weights = dock.with_view(|view| {
        view.root(LAYOUT)
            .unwrap()
            .content()
            .unwrap()
            .split()
            .unwrap()
            .weights()
            .collect::<Vec<_>>()
    });
    let (_, bounds) = anchor(&dock, &mut visual, Scope::Root, 0);
    visual.simulate_click(bounds.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(menu_is_open(&visual, "Layout actions"));
    assert!(!menu_is_open(&visual, "Tab group actions"));
    assert!(accessibility_action(
        &visual,
        semantic_target(&visual, Role::MenuItem, "Float Layout"),
        AccessibleAction::Click,
    ));
    frames(&mut visual, &dock);
    dock.with_view(|view| {
        let contained = view.item(FIRST).unwrap().contained();
        assert!(contained.is_some());
        for item in ITEMS {
            let item = view.item(item).unwrap();
            assert_eq!(item.root(), LAYOUT);
            assert_eq!(item.surface(), SURFACE);
            assert_eq!(item.contained(), contained);
        }
        let surface = view.surface(SURFACE).unwrap();
        assert!(surface.is_rootless());
        assert_eq!(surface.contained_count(), 1);
        let split = view
            .root(LAYOUT)
            .unwrap()
            .content()
            .unwrap()
            .split()
            .unwrap();
        assert_eq!(split.axis(), DockspaceAxis::Horizontal);
        assert_eq!(split.weights().collect::<Vec<_>>(), weights);
        assert_eq!(
            split
                .children()
                .map(|child| child.tabs().unwrap().items().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            [vec![FIRST, SECOND], vec![THIRD, FOURTH]]
        );
    });
}

#[open_gpui::test]
fn keyboard_and_accessibility_group_close_use_exact_atomic_decision_tokens(
    cx: &mut TestAppContext,
) {
    for veto in [false, true] {
        let (dock, mut visual, _) = split_groups(cx);
        let (_, bounds) = anchor(&dock, &mut visual, Scope::Tabs, 0);
        visual.simulate_click(bounds.center(), Modifiers::none());
        frames(&mut visual, &dock);
        if veto {
            assert!(accessibility_action(
                &visual,
                semantic_target(&visual, Role::MenuItem, "Close Tab Group"),
                AccessibleAction::Click,
            ));
        } else {
            visual.simulate_keystrokes("end");
            frames(&mut visual, &dock);
            assert_eq!(
                super::selected_presentation_menu_row(&visual).as_deref(),
                Some("Close Tab Group")
            );
            visual.simulate_keystrokes("enter");
        }
        frames(&mut visual, &dock);
        assert!(!menu_is_open(&visual, "Tab group actions"));
        let plans = dock.pending_content_close_plans().unwrap();
        assert_eq!(plans.len(), 1);
        let plan = &plans[0];
        assert_eq!(
            plan.target(),
            ClosePlanTarget::Tabs {
                root: LAYOUT,
                group: dock.with_view(|view| view.item(FIRST).unwrap().tabs().unwrap().key()),
            }
        );
        assert_eq!(
            plan.items()
                .iter()
                .map(|item| item.item())
                .collect::<Vec<_>>(),
            [FIRST, SECOND]
        );
        visual.update(|_, cx| {
            dock.update(
                |frame| {
                    frame.resolve_close(
                        plan.request(),
                        plan.items()[0].token(),
                        CloseDecision::Allow,
                    )?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        frames(&mut visual, &dock);
        dock.with_view(|view| {
            assert_eq!(
                view.item(FIRST)
                    .unwrap()
                    .tabs()
                    .unwrap()
                    .items()
                    .collect::<Vec<_>>(),
                [FIRST, SECOND]
            );
            assert!(view.item(SECOND).is_some());
        });
        assert_sibling_group(&dock);
        visual.update(|_, cx| {
            dock.update(
                |frame| {
                    frame.resolve_close(
                        plan.request(),
                        plan.items()[1].token(),
                        if veto {
                            CloseDecision::Veto
                        } else {
                            CloseDecision::Allow
                        },
                    )?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        frames(&mut visual, &dock);
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
        dock.with_view(|view| {
            assert_eq!(view.item(FIRST).is_some(), veto);
            assert_eq!(view.item(SECOND).is_some(), veto);
            assert_eq!(
                view.root(LAYOUT)
                    .unwrap()
                    .content()
                    .unwrap()
                    .split()
                    .is_some(),
                veto
            );
        });
        assert_sibling_group(&dock);
    }
}

#[open_gpui::test]
fn selection_and_topology_changes_invalidate_old_menu_targets_before_another_group_opens(
    cx: &mut TestAppContext,
) {
    for topology_change in [false, true] {
        let (dock, mut visual, probe) = split_groups(cx);
        let (menu, bounds) = anchor(&dock, &mut visual, Scope::Tabs, 0);
        visual.simulate_click(bounds.center(), Modifiers::none());
        frames(&mut visual, &dock);
        let old_float = semantic_target(&visual, Role::MenuItem, "Float Tab Group");
        let old_close = semantic_target(&visual, Role::MenuItem, "Close Tab Group");
        let interaction = probe.interaction.borrow().as_ref().unwrap().clone();
        assert!(
            interaction
                .menu
                .get()
                .is_some_and(|open| open.same_menu(menu.key(SURFACE)))
        );
        visual.update(|_, cx| {
            dock.update(
                |frame| {
                    if topology_change {
                        frame.dock_item_current(SECOND, DockPlacement::After(THIRD))?;
                    } else {
                        frame.select_item_current(SECOND)?;
                    }
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        frames(&mut visual, &dock);
        assert!(interaction.menu.get().is_none());
        assert!(!menu_is_open(&visual, "Tab group actions"));
        let (replacement, bounds) = anchor(&dock, &mut visual, Scope::Tabs, 1);
        assert_ne!(replacement.key(SURFACE), menu.key(SURFACE));
        visual.simulate_click(bounds.center(), Modifiers::none());
        frames(&mut visual, &dock);
        assert!(menu_is_open(&visual, "Tab group actions"));
        let version = dock.host.borrow().session.version();
        for target in [old_float, old_close] {
            let _ = accessibility_action(&visual, target, AccessibleAction::Click);
        }
        frames(&mut visual, &dock);
        assert_eq!(dock.host.borrow().session.version(), version);
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
        assert!(
            interaction
                .menu
                .get()
                .is_some_and(|open| open.same_menu(replacement.key(SURFACE)))
        );
        dock.with_view(|view| {
            for item in ITEMS {
                assert!(view.item(item).unwrap().contained().is_none());
            }
        });
        assert!(accessibility_action(
            &visual,
            semantic_target(&visual, Role::MenuItem, "Float Tab Group"),
            AccessibleAction::Click,
        ));
        frames(&mut visual, &dock);
        dock.with_view(|view| {
            assert!(view.item(FIRST).unwrap().contained().is_none());
            assert!(view.item(THIRD).unwrap().contained().is_some());
            assert_eq!(
                view.item(THIRD).unwrap().contained(),
                view.item(FOURTH).unwrap().contained()
            );
            assert_eq!(
                view.item(SECOND).unwrap().contained().is_some(),
                topology_change
            );
            let expected = if topology_change {
                vec![THIRD, SECOND, FOURTH]
            } else {
                vec![THIRD, FOURTH]
            };
            assert_eq!(
                view.item(THIRD)
                    .unwrap()
                    .tabs()
                    .unwrap()
                    .items()
                    .collect::<Vec<_>>(),
                expected
            );
        });
    }
}

#[open_gpui::test]
fn discarded_prepaint_keeps_the_installed_menu_until_a_replacement_is_accepted(
    cx: &mut TestAppContext,
) {
    let (dock, mut visual, probe) = split_groups(cx);
    let (menu, bounds) = anchor(&dock, &mut visual, Scope::Tabs, 0);
    visual.simulate_click(bounds.center(), Modifiers::none());
    frames(&mut visual, &dock);
    let interaction = probe.interaction.borrow().as_ref().unwrap().clone();
    let installed = visual.update(|window, _| {
        dock.host.borrow().mounts[&window.window_handle().window_id()].regions[&SURFACE]
            .packet
            .clone()
            .unwrap()
    });
    visual.update(|window, cx| {
        dock.update(
            |frame| {
                frame.select_item_current(SECOND)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        let revision = window.rendered_frame_revision();
        probe.discard_next_prepaint.set(true);
        window.refresh();
        let rejected = catch_unwind(AssertUnwindSafe(|| window.draw(cx).clear(cx)));
        assert!(
            rejected.is_err(),
            "the candidate must reach the prepaint discard seam"
        );
        assert!(!probe.discard_next_prepaint.get());
        assert_eq!(window.rendered_frame_revision(), revision);
        assert!(
            interaction
                .menu
                .get()
                .is_some_and(|open| open.same_menu(menu.key(SURFACE)))
        );
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_handle().window_id()];
        assert!(Rc::ptr_eq(
            mount.regions[&SURFACE].packet.as_ref().unwrap(),
            &installed
        ));
        let pending = mount
            .pending
            .as_ref()
            .expect("the discarded candidate was prepared");
        assert!(pending.update.is_some());
        assert!(pending.packets[&SURFACE].draws.iter().any(|draw| {
            matches!(draw, Draw::PresentationMenu(candidate)
                if candidate.scope == Scope::Tabs
                    && candidate.id == menu.id
                    && candidate.key(SURFACE) != menu.key(SURFACE))
        }));
        drop(host);
        presentation::abandon_unaccepted(&mut dock.host.borrow_mut(), window, cx);
        assert!(
            interaction
                .menu
                .get()
                .is_some_and(|open| open.same_menu(menu.key(SURFACE)))
        );
    });
    frames(&mut visual, &dock);
    assert!(interaction.menu.get().is_none());
    assert!(!menu_is_open(&visual, "Tab group actions"));
    dock.with_view(|view| {
        assert!(view.item(SECOND).unwrap().is_active());
        for item in ITEMS {
            assert!(view.item(item).unwrap().contained().is_none());
        }
    });
}

#[open_gpui::test]
fn narrow_strips_keep_group_root_list_and_grip_hits_inside_their_visible_chrome(
    cx: &mut TestAppContext,
) {
    let (dock, mut visual, probe) = split_groups(cx);
    let mut saw_group = false;
    let mut saw_root = false;
    let mut saw_grip = false;
    let mut saw_list = false;
    let mut saw_compact_strip = false;
    for width in [800.0, 360.0, 208.0, 96.0, 48.0] {
        visual.simulate_resize(size(px(width), px(600.0)));
        frames(&mut visual, &dock);
        let (menus, lists, tabs) = visual.update(|window, _| {
            let host = dock.host.borrow();
            let mount = &host.mounts[&window.window_handle().window_id()];
            let region = &mount.regions[&SURFACE];
            let bounds = region.bounds.unwrap();
            let packet = region.packet.as_ref().unwrap();
            let mut chrome = Vec::new();
            let mut menus = Vec::new();
            let mut lists = Vec::new();
            let mut tabs = Vec::new();
            for draw in &packet.draws {
                match draw {
                    Draw::PresentationMenu(menu) => {
                        let rect = to_gpui(menu.bounds, bounds);
                        chrome.push(rect);
                        menus.push((menu.scope, rect));
                    }
                    Draw::Icon(rect, Icon::Grip, true) => chrome.push(to_gpui(*rect, bounds)),
                    Draw::Control(control) if control.label == "Show all tabs" => {
                        let rect = to_gpui(control.bounds, bounds);
                        chrome.push(rect);
                        lists.push(rect);
                    }
                    Draw::Control(control) if control.slot(ControlAction::Activate).is_some() => {
                        if let ControlKind::Tab(key) = control.kind {
                            let item = key.item().expect("the fixture has only item tabs");
                            tabs.push((item, to_gpui(control.bounds, bounds)));
                        }
                    }
                    _ => {}
                }
            }
            for (index, rect) in chrome.iter().enumerate() {
                assert!(!rect.is_empty(), "width={width}");
                let clipped = rect.intersect(&bounds);
                assert!(
                    (rect.origin - clipped.origin).magnitude() < 0.001,
                    "width={width}"
                );
                assert!(
                    (rect.size.width - clipped.size.width).abs() < px(0.001),
                    "width={width}"
                );
                assert!(
                    (rect.size.height - clipped.size.height).abs() < px(0.001),
                    "width={width}"
                );
                for other in &chrome[index + 1..] {
                    assert!(
                        !rect.intersects(other),
                        "width={width}: {rect:?} / {other:?}"
                    );
                }
                for (_, tab) in &tabs {
                    assert!(
                        !rect.intersects(tab),
                        "width={width}: chrome covers a tab hit"
                    );
                }
            }
            for input in mount
                .input_frames
                .iter()
                .filter(|input| input.surface == SURFACE)
            {
                for (receiver, hitbox) in &input.receivers {
                    if matches!(
                        receiver.role(),
                        DockspaceReceiverRole::TabGroupGrip
                            | DockspaceReceiverRole::TabGroupTrailingEmpty
                    ) {
                        let rect = to_gpui(receiver.bounds(), bounds);
                        for other in menus.iter().map(|(_, bounds)| bounds).chain(&lists) {
                            assert!(
                                !rect.intersects(other),
                                "width={width}: group dragging covers an action button"
                            );
                        }
                    }
                    if receiver.role() == DockspaceReceiverRole::TabGroupGrip {
                        saw_grip = true;
                        assert!(packet.draws.iter().any(|draw| {
                            matches!(draw, Draw::Icon(rect, Icon::Grip, true)
                                if *rect == receiver.bounds())
                        }));
                        let rect = to_gpui(receiver.bounds(), bounds);
                        assert_eq!(
                            hitbox.window_point_target_rank(rect.center(), window),
                            Some(0)
                        );
                    }
                }
            }
            (menus, lists, tabs)
        });
        assert_eq!(
            visual
                .debug_selectors_with_prefix("dockspace:presentation-menu:")
                .len(),
            menus.len()
        );
        saw_compact_strip |= menus.is_empty();
        for (scope, bounds) in menus {
            saw_group |= scope == Scope::Tabs;
            saw_root |= scope == Scope::Root;
            let label = match scope {
                Scope::Root => "Layout actions",
                Scope::Tabs => "Tab group actions",
            };
            visual.simulate_click(bounds.center(), Modifiers::none());
            frames(&mut visual, &dock);
            assert!(menu_is_open(&visual, label), "width={width}");
            assert!(!dock.has_active_gesture());
            visual.simulate_keystrokes("escape");
            frames(&mut visual, &dock);
        }
        for bounds in lists {
            saw_list = true;
            visual.simulate_click(bounds.center(), Modifiers::none());
            frames(&mut visual, &dock);
            assert!(
                probe
                    .interaction
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .menu
                    .get()
                    .is_none()
            );
            assert!(visual.update(|window, _| {
                dock.host.borrow().mounts[&window.window_handle().window_id()].regions[&SURFACE]
                    .packet
                    .as_ref()
                    .unwrap()
                    .draws
                    .iter()
                    .any(|draw| {
                        matches!(draw, Draw::Receiver(receiver)
                        if receiver.role() == DockspaceReceiverRole::TabListMenuRow)
                    })
            }));
            visual.simulate_keystrokes("escape");
            frames(&mut visual, &dock);
        }
        for (item, _) in tabs {
            let bounds =
                visual.update(|window, _| {
                    let host = dock.host.borrow();
                    let region =
                        &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
                    region.packet.as_ref().unwrap().draws.iter().find_map(|draw| {
                    match draw {
                        Draw::Control(control)
                            if matches!(control.kind, ControlKind::Tab(key) if key == item.into())
                                && control.slot(ControlAction::Activate).is_some() =>
                        {
                            Some(to_gpui(control.bounds, region.bounds.unwrap()))
                        }
                        _ => None,
                    }
                })
                });
            let Some(bounds) = bounds else {
                continue;
            };
            let position = point(
                bounds.left() + px(1.0).min(bounds.size.width * 0.25),
                bounds.center().y,
            );
            visual.simulate_click(position, Modifiers::none());
            frames(&mut visual, &dock);
            assert!(
                dock.with_view(|view| view.item(item).unwrap().is_active()),
                "width={width}"
            );
            assert!(
                probe
                    .interaction
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .menu
                    .get()
                    .is_none()
            );
            assert!(dock.pending_content_close_plans().unwrap().is_empty());
            assert!(!dock.has_active_gesture());
        }
    }
    assert!(saw_group && saw_root && saw_grip && saw_list && saw_compact_strip);
}
