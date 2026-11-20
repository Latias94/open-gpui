//! Pointer, keyboard and assistive-technology input share the original core scene.

use dockspace::{
    model::{
        DockspaceAxis, DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout,
        RootId,
    },
    policy::DockPolicy,
    runtime::{DockspaceReceiverRole, DockspaceSession},
};
use open_gpui::{
    Context, IntoElement, Modifiers, PlatformWindowPresentOutcome, Render, RequestFrameOptions,
    TestAppContext, VisualTestContext, Window, accesskit, div, px, size,
};

use super::*;
use crate::adapter::{DockPane, DockSurface, GpuiDockspaceConfig};

const SURFACE: SurfaceId = SurfaceId::new(51);
const FIRST: ItemId = ItemId::new(61);
const SECOND: ItemId = ItemId::new(62);

mod composable;
mod content_receivers;
mod input_recovery;
mod keyboard;
mod presentation_menu;
mod product_menu_keys;
mod product_menu_lifecycle;
mod splitter;
mod wheel;

struct Pane {
    focus: FocusHandle,
}
impl Render for Pane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("pane-content")
            .size_full()
            .track_focus(&self.focus)
            .child("Content")
    }
}

struct Root {
    dock: DockSurface,
}
impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dock.element(SURFACE))
    }
}

struct ExternalFocusRoot {
    dock: DockSurface,
    focus: FocusHandle,
    keys: Rc<RefCell<Vec<String>>>,
}

impl Render for ExternalFocusRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let keys = self.keys.clone();
        div().size_full().child(self.dock.element(SURFACE)).child(
            div()
                .id("application-keyboard-target")
                .absolute()
                .bottom_0()
                .right_0()
                .size(px(1.0))
                .track_focus(&self.focus)
                .on_key_down(move |event, _, _| {
                    keys.borrow_mut().push(event.keystroke.key.clone());
                }),
        )
    }
}

fn fixture(cx: &mut TestAppContext, split: bool) -> (DockSurface, [FocusHandle; 2]) {
    fixture_with_mode(cx, split, false)
}

fn fixture_with_mode(
    cx: &mut TestAppContext,
    split: bool,
    managed: bool,
) -> (DockSurface, [FocusHandle; 2]) {
    cx.update(|cx| {
        let handles = [cx.focus_handle(), cx.focus_handle()];
        let panes = [FIRST, SECOND]
            .into_iter()
            .zip(handles.iter())
            .map(|(item, focus)| {
                let focus = focus.clone();
                let view = cx.new(|_| Pane {
                    focus: focus.clone(),
                });
                DockPane::new(item, format!("Pane {}", item.get()), view).with_focus_handle(focus)
            })
            .collect::<Vec<_>>();
        let content = if split {
            DockspaceNode::split(
                DockspaceAxis::Horizontal,
                [
                    (DockspaceNode::tabs([FIRST]), 1.0),
                    (DockspaceNode::tabs([SECOND]), 1.0),
                ],
            )
            .unwrap()
        } else {
            DockspaceNode::tabs([FIRST, SECOND])
        };
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(71), content),
        )])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = if managed {
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx)
        } else {
            DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx)
        }
        .unwrap();
        (dock, handles)
    })
}

#[open_gpui::test]
fn prepared_chrome_action_waits_for_earlier_original_native_ingress(cx: &mut TestAppContext) {
    let (dock, _) = fixture_with_mode(cx, false, true);
    let window = cx
        .update(|cx| dock.open_native_root(SURFACE, true, Default::default(), cx))
        .unwrap();
    let mut visual = active_visual(window, cx);
    frames(&mut visual, &dock);
    visual.update(|window, cx| {
        let (serial, slot, version) = {
            let host = dock.host.borrow();
            let mount = &host.mounts[&window.window_handle().window_id()];
            let slot = mount.regions[&SURFACE]
                .packet
                .as_ref()
                .unwrap()
                .draws
                .iter()
                .find_map(|draw| match draw {
                    Draw::Control(control)
                        if matches!(control.kind, ControlKind::Tab(key) if key == SECOND.into()) =>
                    {
                        control.slot(ControlAction::Activate)
                    }
                    _ => None,
                })
                .unwrap();
            (mount.serial, slot, host.session.version())
        };
        let transport = dock.native_transport().unwrap();
        transport.device_removed(cx);
        assert!(!activate_action(
            &dock.host,
            &InteractionState::default(),
            SURFACE,
            serial,
            &slot,
            window,
            cx
        ));
        assert!(
            slot.borrow().is_some(),
            "waiting must not consume the original affine action"
        );
        assert_eq!(dock.host.borrow().session.version(), version);
        assert_eq!(transport.inspection().front_record, Some("device-removed"));
    });
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
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

fn active_visual(window: open_gpui::AnyWindowHandle, cx: &TestAppContext) -> VisualTestContext {
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, _| {
        let _ = window.activate_window();
    });
    visual.run_until_parked();
    assert!(
        visual.update(|window, _| window.is_window_active()),
        "pointer capture requires an actually active framework window"
    );
    visual
}

fn receiver_bounds(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    role: DockspaceReceiverRole,
) -> Bounds<Pixels> {
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let packet = region.packet.as_ref().unwrap();
        let receiver = packet
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Receiver(receiver) if receiver.role() == role => Some(receiver),
                _ => None,
            })
            .unwrap();
        to_gpui(receiver.bounds(), region.bounds.unwrap())
    })
}

fn tab_list_button_bounds(dock: &DockSurface, window: open_gpui::WindowId) -> Bounds<Pixels> {
    let host = dock.host.borrow();
    let region = &host.mounts[&window].regions[&SURFACE];
    let control = region
        .packet
        .as_ref()
        .unwrap()
        .draws
        .iter()
        .find_map(|draw| match draw {
            Draw::Control(control) if control.label == "Show all tabs" => Some(control),
            _ => None,
        })
        .unwrap();
    to_gpui(control.bounds, region.bounds.unwrap())
}

fn assert_accessibility_focus(visual: &VisualTestContext, role: Role, label: &str) {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (_, node) = tree
        .nodes
        .iter()
        .find(|(id, _)| *id == tree.focus)
        .expect("the published focus must name a published semantic node");
    assert_eq!(node.role(), role);
    assert_eq!(node.label(), Some(label));
}

#[open_gpui::test]
fn default_tab_close_has_visible_ink_and_delivers_a_core_close_plan(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    assert!(
        dock.host.borrow().mounts[&window.window_id()].regions[&SURFACE]
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .any(|draw| matches!(draw, Draw::Icon(_, Icon::Close, true)))
    );
    let close = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::TabClose);
    visual.simulate_click(close.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(!dock.pending_content_close_plans().unwrap().is_empty());
    assert!(
        !dock.has_active_gesture(),
        "close must not start a tab drag"
    );
}

#[open_gpui::test]
fn tab_keys_wrap_and_preserve_tab_focus_until_explicit_content_activation(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let tab = visual.debug_bounds("dockspace:tab:61").unwrap();
    visual.simulate_click(tab.center(), Modifiers::none());
    frames(&mut visual, &dock);
    // A tab body click may hand focus to content; Ctrl+Tab navigates from either scope.
    visual.simulate_keystrokes("ctrl-tab");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(!visual.update(|window, cx| handles[1].contains_focused(window, cx)));
    visual.simulate_keystrokes("right");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    visual.simulate_keystrokes("end");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    visual.simulate_keystrokes("enter");
    frames(&mut visual, &dock);
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
}

#[open_gpui::test]
fn deferred_content_never_takes_focus_before_its_original_output_is_presented(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let tab = visual.debug_bounds("dockspace:tab:62").unwrap();
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    visual.simulate_click(tab.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(!visual.update(|window, cx| handles[1].contains_focused(window, cx)));
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
    assert!(
        dock.host
            .borrow()
            .session
            .pane_focus_request(SURFACE)
            .is_none(),
        "the exact focused observation must settle the admitted request"
    );
}

#[open_gpui::test]
fn accessibility_splitter_increment_changes_the_core_partition(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, true);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (target, node) = tree
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == Role::Splitter && node.supports_action(AccessibleAction::Increment)
        })
        .unwrap();
    assert_eq!(node.orientation(), Some(open_gpui::Orientation::Vertical));
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: accesskit::TreeId::ROOT,
            target_node: *target,
            data: None,
        })
    );
    frames(&mut visual, &dock);
    let after = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!(after.origin.x > before.origin.x);
}

#[open_gpui::test]
fn an_unbound_content_focus_request_is_reported_unavailable(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture(cx, false);
    dock.host
        .borrow_mut()
        .panels
        .get_mut(&SECOND)
        .unwrap()
        .content_focus = None;
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let tab = visual.debug_bounds("dockspace:tab:62").unwrap();
    visual.simulate_click(tab.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(!visual.update(|window, cx| handles[1].contains_focused(window, cx)));
    assert!(
        dock.host
            .borrow()
            .session
            .pane_focus_request(SURFACE)
            .is_none()
    );
}

#[open_gpui::test]
fn a_control_from_a_superseded_output_cannot_navigate_tabs(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let (serial, control) = {
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_id()];
        let packet = mount.regions[&SURFACE].packet.as_ref().unwrap();
        let control = packet
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::Control(control)
                    if matches!(control.kind, ControlKind::Tab(key) if key == FIRST.into()) =>
                {
                    Some(control.clone())
                }
                _ => None,
            })
            .unwrap();
        (mount.serial, control)
    };
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                let _ = frame.select_item_current(SECOND)?;
                Ok(())
            },
            cx,
        )
        .unwrap()
    });
    frames(&mut visual, &dock);
    let version = dock.host.borrow().session.version();
    assert!(!visual.update(|window, cx| super::keyboard::handle(
        &dock.host,
        &Rc::new(InteractionState::default()),
        SURFACE,
        serial,
        &control,
        &open_gpui::Keystroke::parse("right").unwrap(),
        window,
        cx,
    )));
    assert_eq!(dock.host.borrow().session.version(), version);
}

#[open_gpui::test]
fn presentation_menu_activates_the_frozen_core_float_command(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let anchor = {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_id()].regions[&SURFACE];
        let menu = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::PresentationMenu(menu)
                    if menu.enabled && menu.scope == super::presentation_menu::Scope::Root =>
                {
                    Some(menu)
                }
                _ => None,
            })
            .unwrap();
        assert!(
            menu.choices.is_empty(),
            "closed menus must not prepare product commands each frame"
        );
        to_gpui(menu.bounds, region.bounds.unwrap())
    };
    visual.simulate_click(anchor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (target, _) = tree
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == Role::MenuItem
                && node.label() == Some("Float Layout")
                && node.supports_action(AccessibleAction::Click)
        })
        .unwrap();
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: *target,
            data: None,
        })
    );
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_some()));
}

#[open_gpui::test]
fn overflowing_strip_control_opens_the_core_tab_list(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(160.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    frames(&mut visual, &dock);
    let button = tab_list_button_bounds(&dock, window.window_id());
    visual.simulate_click(button.center(), Modifiers::none());
    frames(&mut visual, &dock);
    let host = dock.host.borrow();
    let packet = host.mounts[&window.window_id()].regions[&SURFACE]
        .packet
        .as_ref()
        .unwrap();
    assert!(
        packet
            .draws
            .iter()
            .any(|draw| matches!(draw, Draw::Receiver(receiver)
        if receiver.role() == DockspaceReceiverRole::TabListMenuRow))
    );
}

#[open_gpui::test]
fn tab_list_arrows_move_framework_focus_before_enter_activates_the_current_row(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture(cx, false);
    let window = cx.open_window(size(px(160.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let button = tab_list_button_bounds(&dock, window.window_id());
    visual.simulate_click(button.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 61");
    visual.simulate_keystrokes("down");
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 62");
    visual.simulate_keystrokes("enter");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
}

#[open_gpui::test]
fn dismissing_the_core_tab_list_restores_its_opener_and_can_reopen_from_the_keyboard(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(160.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let button = tab_list_button_bounds(&dock, window.window_id());
    visual.simulate_click(button.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 61");
    visual.simulate_keystrokes("escape");
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::Button, "Show all tabs");
    visual.simulate_keystrokes("down");
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 61");
}

#[test]
fn source_preview_uses_the_observed_physical_client_origin_and_its_own_dpi() {
    let preview = host_native::provisional::Preview {
        host_surface: SURFACE,
        target_surface: SurfaceId::new(52),
        placement: dockspace::geometry::PhysicalRect::new(-600.0, 700.0, 400.0, 300.0).unwrap(),
    };
    let geometry = open_gpui::PlatformWindowPhysicalGeometry::try_new(
        Bounds::new(
            point(open_gpui::DevicePixels(-800), open_gpui::DevicePixels(600)),
            size(open_gpui::DevicePixels(1600), open_gpui::DevicePixels(1200)),
        ),
        2.0,
    )
    .unwrap();
    let surface = Bounds::new(point(px(25.0), px(10.0)), size(px(400.0), px(400.0)));
    let projected = source_preview_bounds(preview, surface, Some(geometry)).unwrap();
    assert_eq!(
        projected,
        Bounds::new(point(px(100.0), px(50.0)), size(px(200.0), px(150.0)))
    );
    assert!(
        source_preview_bounds(preview, surface, None).is_none(),
        "a source cue must never manufacture a physical desktop origin"
    );
}

fn presentation_menu_with_content_focus(
    cx: &mut TestAppContext,
) -> (DockSurface, FocusHandle, VisualTestContext, Bounds<Pixels>) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    visual.update(|window, cx| window.focus(&handles[0], cx));
    frames(&mut visual, &dock);
    assert!(visual.update(|window, cx| handles[0].contains_focused(window, cx)));
    let anchor = visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&window.window_handle().window_id()].regions[&SURFACE];
        let menu = region
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .find_map(|draw| match draw {
                Draw::PresentationMenu(menu)
                    if menu.enabled && menu.scope == super::presentation_menu::Scope::Root =>
                {
                    Some(menu)
                }
                _ => None,
            })
            .unwrap();
        to_gpui(menu.bounds, region.bounds.unwrap())
    });
    (dock, handles[0].clone(), visual, anchor)
}

fn presentation_menu_is_open(visual: &VisualTestContext) -> bool {
    visual
        .latest_accessibility_tree_update()
        .unwrap()
        .nodes
        .iter()
        .any(|(_, node)| node.role() == Role::Menu && node.label() == Some("Layout actions"))
}

fn selected_presentation_menu_row(visual: &VisualTestContext) -> Option<String> {
    visual
        .latest_accessibility_tree_update()
        .unwrap()
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == Role::MenuItem && node.is_selected() == Some(true))
                .then(|| node.label().map(str::to_owned))
                .flatten()
        })
}

#[open_gpui::test]
fn presentation_menu_pointer_open_from_content_owns_escape_and_restores_its_opener(
    cx: &mut TestAppContext,
) {
    let (dock, content_focus, mut visual, anchor) = presentation_menu_with_content_focus(cx);
    visual.simulate_click(anchor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(presentation_menu_is_open(&visual));
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
    assert!(!visual.update(|window, cx| content_focus.contains_focused(window, cx)));
    visual.simulate_keystrokes("escape");
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
}

#[open_gpui::test]
fn presentation_menu_pointer_open_from_content_routes_arrows_before_enter(cx: &mut TestAppContext) {
    let (dock, _, mut visual, anchor) = presentation_menu_with_content_focus(cx);
    visual.simulate_click(anchor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(
        selected_presentation_menu_row(&visual).as_deref(),
        Some("Float Layout")
    );
    visual.simulate_keystrokes("down");
    frames(&mut visual, &dock);
    assert!(selected_presentation_menu_row(&visual).is_some());
    assert_ne!(
        selected_presentation_menu_row(&visual).as_deref(),
        Some("Float Layout")
    );
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
    visual.simulate_keystrokes("up");
    frames(&mut visual, &dock);
    assert_eq!(
        selected_presentation_menu_row(&visual).as_deref(),
        Some("Float Layout")
    );
    visual.simulate_keystrokes("enter");
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_some()));
}

#[open_gpui::test]
fn presentation_menu_accessibility_open_focuses_its_opener_and_preserves_later_claims(
    cx: &mut TestAppContext,
) {
    let (dock, content_focus, mut visual, _) = presentation_menu_with_content_focus(cx);
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let target = tree
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Layout actions")).then_some(*id)
        })
        .unwrap();
    let click = || accesskit::ActionRequest {
        action: AccessibleAction::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    };
    assert!(visual.dispatch_accessibility_action(click()));
    frames(&mut visual, &dock);
    assert!(presentation_menu_is_open(&visual));
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
    let (claim_revision, serial) = visual.update(|window, cx| {
        window.focus(&content_focus, cx);
        let serial = dock.host.borrow().mounts[&window.window_handle().window_id()].serial;
        (window.focus_claim_revision(), serial)
    });
    assert!(visual.dispatch_accessibility_action(click()));
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    visual.update(|window, cx| {
        assert_eq!(
            dock.host.borrow().mounts[&window.window_handle().window_id()].serial,
            serial,
            "dismissing adapter-owned chrome must retain the presented core output"
        );
        assert!(content_focus.contains_focused(window, cx));
        assert_eq!(window.focus_claim_revision(), claim_revision);
    });

    assert!(visual.dispatch_accessibility_action(click()));
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
    let float_target = visual
        .latest_accessibility_tree_update()
        .unwrap()
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == Role::MenuItem && node.label() == Some("Float Layout")).then_some(*id)
        })
        .unwrap();
    let claim_revision = visual.update(|window, cx| {
        window.dispatch_keystroke(open_gpui::Keystroke::parse("escape").unwrap(), cx);
        window.focus(&content_focus, cx);
        window.focus_claim_revision()
    });
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: float_target,
            data: None,
        })
    );
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
    visual.update(|window, cx| {
        assert!(content_focus.contains_focused(window, cx));
        assert_eq!(window.focus_claim_revision(), claim_revision);
    });
}

#[open_gpui::test]
fn presentation_menu_pointer_open_waits_for_the_exact_output_to_be_presented(
    cx: &mut TestAppContext,
) {
    let (dock, _, mut visual, anchor) = presentation_menu_with_content_focus(cx);
    let window = visual.update(|window, _| window.window_handle());
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                let _ = frame.select_item_current(SECOND)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut visual, &dock);
    let claim_revision = visual.update(|window, _| {
        let host = dock.host.borrow();
        let owner = window.window_handle().window_id();
        let serial = host.mounts[&owner].serial;
        assert!(!input::is_presented(&host, owner, SURFACE, serial));
        window.focus_claim_revision()
    });
    visual.simulate_click(anchor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    assert_eq!(
        visual.update(|window, _| window.focus_claim_revision()),
        claim_revision
    );
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    visual.simulate_click(anchor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert!(presentation_menu_is_open(&visual));
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
}
