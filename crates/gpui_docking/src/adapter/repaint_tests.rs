//! Managed cancellation repaints retained feedback without waking unrelated surfaces.

use super::*;
use dockspace::runtime::{
    DockspacePreviewVisual, DockspaceReceiverRole, SurfacePaintProgramStep as Step,
};
use open_gpui::{
    AnyWindowHandle, DevicePixels, MouseButton, MouseDownEvent, MouseMoveEvent,
    NativeCapturedDragPhase, PlatformInput, PlatformNativeDragHysteresis,
    PlatformNativePointerCaptureOwner, PlatformNativePointerPhysicalFrame, PlatformWindowHit,
    PlatformWindowHitStack, PlatformWindowPhysicalCoverage, Point, PointerCancelReason, point,
};

const THIRD: SurfaceId = SurfaceId::new(13);
const C: ItemId = ItemId::new(23);

fn pointer_frame(
    cx: &mut TestAppContext,
    source: AnyWindowHandle,
    hovered: AnyWindowHandle,
    position: Point<open_gpui::Pixels>,
) -> PlatformNativePointerPhysicalFrame {
    let target_geometry = cx
        .update_window(hovered, |_, window, _| {
            window.platform_facts().physical_geometry.unwrap()
        })
        .unwrap();
    let source_geometry = cx
        .update_window(source, |_, window, _| {
            window.platform_facts().physical_geometry.unwrap()
        })
        .unwrap();
    let global = target_geometry.local_to_global(position).unwrap();
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            global,
            vec![PlatformWindowHit::RegisteredApplication {
                window: hovered,
                coverage: PlatformWindowPhysicalCoverage::try_new(target_geometry.client_bounds())
                    .unwrap(),
                geometry: target_geometry,
            }],
        )
        .unwrap(),
    );
    PlatformNativePointerPhysicalFrame::new(global, source_geometry).with_capture_owner(
        PlatformNativePointerCaptureOwner::Window(source.window_id()),
    )
}

fn target_feedback(dock: &DockSurface) -> (bool, bool) {
    let mut host = dock.host.borrow_mut();
    let mut frame = host
        .session
        .begin_native_host_frame(|_| {
            panic!("feedback inspection must not resolve uncommitted native input")
        })
        .unwrap();
    let mut preview = false;
    let mut guides = false;
    frame
        .paint_plan(SECOND)
        .unwrap()
        .unwrap()
        .visit_surface_program(|step| {
            match step {
                Step::DragPreview(candidate) => {
                    assert!(matches!(
                        candidate.visual(),
                        DockspacePreviewVisual::Dock {
                            surface: SECOND,
                            ..
                        }
                    ));
                    preview = true;
                }
                Step::DropGuide {
                    affordance: Some(cluster),
                    ..
                } => guides |= cluster.targets().next().is_some(),
                _ => {}
            }
            Ok::<(), ()>(())
        })
        .unwrap();
    (preview, guides)
}

#[open_gpui::test]
fn managed_cancellation_clears_target_feedback_without_reencoding_an_unrelated_window(
    cx: &mut TestAppContext,
) {
    // TestPlatform derives these capability declarations from its configured observers.
    // Install them before new_managed freezes the native provider's capabilities.
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(0), DevicePixels(0)),
            vec![],
        )
        .unwrap(),
    );
    let surfaces = [FIRST, SECOND, THIRD];
    let items = [A, B, C];
    let (dock, views, document) = cx.update(|cx| {
        let document = Rc::new(RefCell::new(Document {
            text: "shared".into(),
        }));
        let views = items.map(|item| {
            cx.new(|_| View {
                document: document.clone(),
                item,
                clicks: 0,
            })
        });
        let layout =
            DockspaceLayout::new(surfaces.into_iter().zip(items).map(|(surface, item)| {
                DockspaceSurfaceLayout::new(
                    surface,
                    DockspaceRootLayout::new(
                        RootId::new(surface.get()),
                        DockspaceNode::tabs([item]),
                    ),
                )
            }))
            .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        let session = DockspaceSession::from_layout(layout, policy).unwrap();
        let panes = items
            .into_iter()
            .zip(&views)
            .map(|(item, view)| DockPane::new(item, "Original View", view.clone()));
        let dock =
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, views, document)
    });
    let windows: [AnyWindowHandle; 3] = surfaces.map(|surface| {
        cx.defer_next_window_frame_requests();
        cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
            dock.bind_native_root(surface, surface == FIRST, window, cx)
                .unwrap();
            Root {
                dock: dock.clone(),
                surfaces: vec![surface],
            }
        })
        .into()
    });
    for (window, origin) in windows.into_iter().zip([0, 2000, 4000]) {
        let (mut bounds, scale) = cx
            .update_window(window, |_, window, _| {
                let facts = window.platform_facts();
                (
                    facts.bounds.to_device_pixels(facts.scale_factor),
                    facts.scale_factor,
                )
            })
            .unwrap();
        bounds.origin = point(DevicePixels(origin), DevicePixels(0));
        cx.set_platform_window_physical_client_geometry(window, Some(bounds), scale);
        cx.simulate_window_scale_factor_change(window, scale);
    }
    cx.set_platform_native_drag_hysteresis(Some(
        PlatformNativeDragHysteresis::try_new(DevicePixels(4), DevicePixels(4)).unwrap(),
    ));
    let [source, target, unrelated] = windows;
    let mut source_visual = VisualTestContext::from_window(source, cx);
    source_visual.update(|window, _| {
        let _ = window.activate_window();
    });
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    let edit = source_visual.debug_bounds("view:21").unwrap();
    source_visual.simulate_click(edit.center(), Modifiers::none());
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    let identities = views.each_ref().map(|view| view.entity_id());
    let version = dock.host.borrow().session.version();
    let unrelated_serial = dock.host.borrow().mounts[&unrelated.window_id()].serial;
    let unrelated_packet = dock.host.borrow().mounts[&unrelated.window_id()].regions[&THIRD]
        .packet
        .clone()
        .unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update({
        let events = events.clone();
        move |cx| cx.observe_native_captured_drag(move |event, _| events.borrow_mut().push(event))
    });

    let pressed = source_visual
        .debug_bounds("dockspace:tab:21")
        .unwrap()
        .center();
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    let frame = pointer_frame(cx, source, source, pressed);
    cx.simulate_platform_input_with_native_pointer_physical_frame(
        source,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: pressed,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        }),
        Some(frame),
    );
    frames(&mut source_visual, &dock);
    for (hovered, position) in [
        (source, pressed + point(px(24.0), px(0.0))),
        (target, point(px(400.0), px(300.0))),
    ] {
        let frame = pointer_frame(cx, source, hovered, position);
        cx.simulate_platform_input_with_native_pointer_physical_frame(
            source,
            PlatformInput::MouseMove(MouseMoveEvent {
                position: frame
                    .source_geometry()
                    .global_to_local(frame.global_position())
                    .unwrap(),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            Some(frame),
        );
        frames(&mut source_visual, &dock);
    }
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    assert!(dock.has_active_gesture());
    assert_eq!(
        target_feedback(&dock),
        (true, true),
        "the captured move must establish target feedback; windows={windows:?}\ninspection={:#?}\ncaptured_events={:#?}\ntarget_receiver_ranks={:?}",
        dock.inspection(),
        events.borrow(),
        cx.update_window(target, |_, window, _| {
            let host = dock.host.borrow();
            host.mounts[&target.window_id()]
                .input_frames
                .iter()
                .filter(|frame| frame.surface == SECOND)
                .flat_map(|frame| &frame.receivers)
                .map(|(receiver, hitbox)| {
                    (
                        receiver.role(),
                        receiver.bounds(),
                        hitbox.window_point_target_rank(point(px(400.0), px(300.0)), window),
                    )
                })
                .collect::<Vec<_>>()
        }),
    );
    let (target_serial, target_packet) = {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target.window_id()];
        assert!(mount.input_frames.iter().any(|frame| {
            frame.surface == SECOND
                && frame
                    .receivers
                    .iter()
                    .any(|(receiver, _)| receiver.role() == DockspaceReceiverRole::DropTarget)
        }));
        assert_eq!(host.session.version(), version);
        assert_eq!(host.mounts[&unrelated.window_id()].serial, unrelated_serial);
        (mount.serial, mount.regions[&SECOND].packet.clone().unwrap())
    };
    let callbacks = dock
        .native_callbacks_for_window(source.window_id())
        .unwrap();
    let capture = callbacks
        .active_capture()
        .expect("the original managed drag");
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    source_visual.update(|window, cx| {
        window.cancel_pointer_session(PointerCancelReason::CaptureRevoked, cx);
    });
    cx.run_until_parked();
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    assert!(!dock.has_active_gesture());
    assert_eq!(callbacks.active_capture(), None);
    assert_eq!(
        target_feedback(&dock),
        (false, false),
        "cancellation must clear the old target feedback; inspection={:#?}\ncaptured_events={:#?}",
        dock.inspection(),
        events.borrow(),
    );
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&target.window_id()];
        assert!(
            mount.serial > target_serial,
            "the old target must be reencoded"
        );
        assert!(!Rc::ptr_eq(
            mount.regions[&SECOND].packet.as_ref().unwrap(),
            &target_packet,
        ));
        assert!(mount.input_frames.iter().all(|frame| {
            frame
                .receivers
                .iter()
                .all(|(receiver, _)| receiver.role() != DockspaceReceiverRole::DropTarget)
        }));
        assert_eq!(host.mounts[&unrelated.window_id()].serial, unrelated_serial);
        assert!(Rc::ptr_eq(
            host.mounts[&unrelated.window_id()].regions[&THIRD]
                .packet
                .as_ref()
                .unwrap(),
            &unrelated_packet,
        ));
        assert_eq!(host.session.version(), version);
        assert!(
            host.inputs.is_empty(),
            "managed input keeps its sole producer"
        );
        for (item, identity) in items.into_iter().zip(identities) {
            assert_eq!(host.panels[&item].view.entity_id(), identity);
        }
    }
    for (window, (surface, item)) in windows.into_iter().zip(surfaces.into_iter().zip(items)) {
        assert_painted_pane(&dock, window, surface, item);
    }
    let stable_serials =
        windows.map(|window| dock.host.borrow().mounts[&window.window_id()].serial);
    for window in windows {
        frames(&mut VisualTestContext::from_window(window, cx), &dock);
    }
    assert_eq!(
        windows.map(|window| dock.host.borrow().mounts[&window.window_id()].serial),
        stable_serials,
        "settled cancellation and presentation provenance must not create an encoding loop"
    );
    assert!(
        dock.host
            .borrow()
            .mounts
            .values()
            .all(|mount| mount.pending.is_none())
    );
    {
        let events = events.borrow();
        let cancelled = events
            .iter()
            .filter(|event| matches!(event.phase(), NativeCapturedDragPhase::Cancelled(_)))
            .collect::<Vec<_>>();
        assert_eq!(cancelled.len(), 1, "the original drag cancels exactly once");
        assert_eq!(cancelled[0].generation(), capture.generation());
    }
    let edit = source_visual.debug_bounds("view:21").unwrap();
    source_visual.simulate_click(edit.center(), Modifiers::none());
    frames(&mut source_visual, &dock);
    assert_eq!(views[0].read_with(&source_visual, |view, _| view.clicks), 2);
    assert_eq!(views[1].read_with(&source_visual, |view, _| view.clicks), 0);
    assert_eq!(views[2].read_with(&source_visual, |view, _| view.clicks), 0);
    assert_eq!(document.borrow().text, "sharedxx");
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
    assert!(dock.error().is_none(), "{:?}", dock.error());
}
