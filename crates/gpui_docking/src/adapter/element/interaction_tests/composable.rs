//! Composed headers activate real retained content without borrowing a descendant's identity.

use std::num::NonZeroUsize;

use dockspace::model::{DockAnchor, DockPlacement, DockspaceGridShape};
use open_gpui::Entity;

use super::*;

const THIRD: ItemId = ItemId::new(63);
const FOURTH: ItemId = ItemId::new(64);

struct EditablePane {
    item: ItemId,
    focus: FocusHandle,
    edits: usize,
    renders: usize,
}

impl Render for EditablePane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let item = self.item;
        div().size_full().child(
            div()
                .id("editable-content")
                .track_focus(&self.focus)
                .debug_selector(move || format!("composed-pane:{}", item.get()))
                .size_full()
                .on_click(cx.listener(|pane, _, _, cx| {
                    pane.edits += 1;
                    cx.notify();
                }))
                .child(format!("Edits: {}", self.edits)),
        )
    }
}

fn composed_fixture(
    cx: &mut TestAppContext,
    content: DockspaceNode,
) -> (DockSurface, Vec<Entity<EditablePane>>) {
    cx.update(|cx| {
        let views = [FIRST, SECOND, THIRD, FOURTH]
            .into_iter()
            .map(|item| {
                let focus = cx.focus_handle();
                cx.new(|_| EditablePane {
                    item,
                    focus,
                    edits: 0,
                    renders: 0,
                })
            })
            .collect::<Vec<_>>();
        let panes = views
            .iter()
            .map(|view| {
                let pane = view.read(cx);
                DockPane::new(pane.item, format!("Pane {}", pane.item.get()), view.clone())
                    .with_focus_handle(pane.focus.clone())
            })
            .collect::<Vec<_>>();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(71), content),
        )])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, views)
    })
}

fn tools_grid() -> DockspaceNode {
    DockspaceNode::grid(
        DockspaceGridShape::Fixed {
            rows: NonZeroUsize::new(1).unwrap(),
            columns: NonZeroUsize::new(2).unwrap(),
        },
        [DockspaceNode::pane(FIRST), DockspaceNode::pane(SECOND)],
    )
    .unwrap()
    .named("Tools")
}

fn tab_action(visual: &mut VisualTestContext, label: &str, action: AccessibleAction) {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let target = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Tab && node.label() == Some(label))
        .unwrap()
        .0;
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        })
    );
}

#[open_gpui::test]
fn layout_header_keyboard_switches_all_panes_and_keeps_original_edits(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views) = composed_fixture(
        cx,
        DockspaceNode::composed_tabs([tools_grid(), DockspaceNode::pane(THIRD)]),
    );
    let entities = views
        .iter()
        .map(|view| view.entity_id())
        .collect::<Vec<_>>();
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    assert!(visual.debug_bounds("dockspace:tab:61").is_none());
    assert!(visual.debug_bounds("dockspace:tab:62").is_none());
    assert!(visual.debug_bounds("composed-pane:63").is_none());
    let editor = visual.debug_bounds("composed-pane:61").unwrap();
    visual.simulate_click(editor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(views[0].read_with(cx, |pane, _| pane.edits), 1);
    tab_action(&mut visual, "Tools", AccessibleAction::Focus);
    visual.simulate_keystrokes("end enter");
    frames(&mut visual, &dock);
    assert!(visual.debug_bounds("composed-pane:61").is_none());
    assert!(visual.debug_bounds("composed-pane:62").is_none());
    assert!(visual.debug_bounds("composed-pane:63").is_some());
    let inactive_renders = views[..2]
        .iter()
        .map(|view| view.read_with(cx, |pane, _| pane.renders))
        .collect::<Vec<_>>();
    frames(&mut visual, &dock);
    assert_eq!(
        views[..2]
            .iter()
            .map(|view| view.read_with(cx, |pane, _| pane.renders))
            .collect::<Vec<_>>(),
        inactive_renders,
        "inactive layout bodies must not receive render callbacks",
    );
    tab_action(&mut visual, "Pane 63", AccessibleAction::Focus);
    visual.simulate_keystrokes("home enter");
    frames(&mut visual, &dock);
    assert!(visual.debug_bounds("composed-pane:61").is_some());
    assert!(visual.debug_bounds("composed-pane:62").is_some());
    assert!(visual.debug_bounds("composed-pane:63").is_none());
    assert_eq!(views[0].read_with(cx, |pane, _| pane.edits), 1);
    assert_eq!(
        views
            .iter()
            .map(|view| view.entity_id())
            .collect::<Vec<_>>(),
        entities
    );
}

#[open_gpui::test]
fn layout_header_close_requests_every_real_descendant_atomically(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = composed_fixture(
        cx,
        DockspaceNode::composed_tabs([tools_grid(), DockspaceNode::pane(THIRD)]),
    );
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    tab_action(&mut visual, "Tools", AccessibleAction::Focus);
    visual.simulate_keystrokes("delete");
    frames(&mut visual, &dock);
    let plans = dock.pending_content_close_plans().unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0]
            .items()
            .iter()
            .map(|item| item.item())
            .collect::<Vec<_>>(),
        [FIRST, SECOND],
    );
    dock.with_view(|view| {
        for item in [FIRST, SECOND, THIRD] {
            assert!(view.item(item).is_some());
        }
    });
}

#[open_gpui::test]
fn layout_header_close_accessibility_expires_when_a_pane_joins(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = composed_fixture(
        cx,
        DockspaceNode::composed_tabs([
            DockspaceNode::tabs([FIRST, SECOND]).named("Tools"),
            DockspaceNode::pane(THIRD),
        ]),
    );
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let close_button = |visual: &VisualTestContext| {
        visual
            .latest_accessibility_tree_update()
            .unwrap()
            .nodes
            .into_iter()
            .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Close Tools"))
            .unwrap()
            .0
    };
    let old = close_button(&visual);
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.open_item_current(FOURTH, DockPlacement::Center(DockAnchor::Item(FIRST)))?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut visual, &dock);
    let current = close_button(&visual);
    assert_ne!(old, current);
    for target in [old, current] {
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        });
        frames(&mut visual, &dock);
        let plans = dock.pending_content_close_plans().unwrap();
        if target == old {
            assert!(plans.is_empty());
        } else {
            assert_eq!(plans.len(), 1);
            assert_eq!(
                plans[0]
                    .items()
                    .iter()
                    .map(|item| item.item())
                    .collect::<Vec<_>>(),
                [FIRST, SECOND, FOURTH],
            );
        }
    }
}

#[open_gpui::test]
fn content_document_cycle_uses_the_nearest_tabs_before_its_outer_layout(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views) = composed_fixture(
        cx,
        DockspaceNode::composed_tabs([
            DockspaceNode::tabs([FIRST, SECOND]).named("Documents"),
            DockspaceNode::pane(THIRD),
        ]),
    );
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let focus = views[0].read_with(cx, |pane, _| pane.focus.clone());
    visual.update(|window, cx| window.focus(&focus, cx));
    visual.simulate_keystrokes("ctrl-tab");
    frames(&mut visual, &dock);
    dock.with_view(|view| {
        assert!(view.item(SECOND).unwrap().is_active());
        assert!(!view.item(THIRD).unwrap().is_active());
    });
    tab_action(&mut visual, "Documents", AccessibleAction::Focus);
    visual.simulate_keystrokes("ctrl-tab");
    frames(&mut visual, &dock);
    dock.with_view(|view| {
        assert!(!view.item(SECOND).unwrap().is_active());
        assert!(view.item(THIRD).unwrap().is_active());
    });
}

#[open_gpui::test]
fn hidden_inner_header_cannot_lend_content_focus_to_its_outer_layout(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views) = composed_fixture(
        cx,
        DockspaceNode::composed_tabs([
            DockspaceNode::tabs([FIRST]).named("Documents"),
            DockspaceNode::pane(THIRD),
        ]),
    );
    cx.update(|cx| {
        use dockspace::policy::{TabBarInteraction, TabBarPolicy, TabBarVisibility};
        let mut policy = DockPolicy::default();
        policy.set_tab_bar(TabBarPolicy::new(
            TabBarVisibility::AutoHideSingle,
            TabBarInteraction::Enabled,
        ));
        dock.update(
            |frame| {
                frame.replace_policy(policy)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    assert!(visual.debug_bounds("dockspace:tab:61").is_none());
    let focus = views[0].read_with(cx, |pane, _| pane.focus.clone());
    visual.update(|window, cx| window.focus(&focus, cx));
    let version = dock.host.borrow().session.version();
    visual.simulate_keystrokes("ctrl-tab");
    frames(&mut visual, &dock);
    assert_eq!(dock.host.borrow().session.version(), version);
    dock.with_view(|view| {
        assert!(view.item(FIRST).unwrap().is_active());
        assert!(!view.item(THIRD).unwrap().is_active());
    });
    assert!(visual.update(|window, cx| focus.contains_focused(window, cx)));
}

#[open_gpui::test]
fn shared_grid_divider_resizes_both_rows_without_rebuilding_views(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let grid = DockspaceNode::grid(
        DockspaceGridShape::Fixed {
            rows: NonZeroUsize::new(2).unwrap(),
            columns: NonZeroUsize::new(2).unwrap(),
        },
        [FIRST, SECOND, THIRD, FOURTH].map(DockspaceNode::pane),
    )
    .unwrap();
    let (dock, views) = composed_fixture(cx, grid);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let pane_bounds = |visual: &mut VisualTestContext| {
        [FIRST, SECOND, THIRD, FOURTH].map(|item| {
            visual
                .debug_bounds(&format!("composed-pane:{}", item.get()))
                .unwrap()
        })
    };
    let before = pane_bounds(&mut visual);
    let divider = visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let control = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Control(control)
                    if matches!(
                        control.kind,
                        ControlKind::Splitter(DockspaceAxis::Horizontal)
                    ) =>
                {
                    Some(control)
                }
                _ => None,
            })
            .unwrap();
        to_gpui(control.bounds, region.layout_bounds().unwrap()).center()
    });
    visual.simulate_mouse_down(divider, MouseButton::Left, Modifiers::none());
    frames(&mut visual, &dock);
    let destination = divider + point(px(50.0), px(0.0));
    visual.simulate_mouse_move(destination, Some(MouseButton::Left), Modifiers::none());
    frames(&mut visual, &dock);
    visual.simulate_mouse_up(destination, MouseButton::Left, Modifiers::none());
    frames(&mut visual, &dock);
    let after = pane_bounds(&mut visual);
    assert!(after[0].size.width > before[0].size.width);
    assert!(after[1].size.width < before[1].size.width);
    assert!((f32::from(after[0].size.width - after[2].size.width)).abs() < 0.1);
    assert!((f32::from(after[1].size.width - after[3].size.width)).abs() < 0.1);
    assert!(dock.inspection().unwrap().queued_input_count == 0);
    let host = dock.host.borrow();
    for view in &views {
        let item = view.read_with(cx, |pane, _| pane.item);
        assert_eq!(host.panels[&item].view.entity_id(), view.entity_id());
    }
}
