use dockspace::{
    geometry::{LogicalPoint, LogicalRect},
    model::{
        DockspaceAxis, DockspaceContainedLayout, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSurfaceLayout, FloatingPresentationId, ItemId, RootId,
        SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceReceiverRole, DockspaceRuntimeError, DockspaceSession, NativeLocalPointerLocation,
        NativePointerButton, NativePointerEvent, NativePointerId, NativePointerInput,
        NativePointerOwner, NativeReceiverAnswer, NativeReceiverPurpose, NativeReceiverQuery,
        NativeWindowHit, PointerDeviceKind, PointerEventMetadata, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AppContext as _, Context, Empty, IntoElement, Pixels, Point, Render, RequestFrameOptions,
    TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};

use super::{DockSurface, HostState, local_sample, resolve, resolve_in_window};
use crate::adapter::{DockPane, GpuiDockspaceConfig};

const SURFACE: SurfaceId = SurfaceId::new(1);

struct Root(DockSurface);

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .h(px(540.0))
                .overflow_hidden()
                .child(div().h(px(600.0)).child(self.0.element(SURFACE))),
        )
    }
}

fn fixture(cx: &mut TestAppContext) -> (DockSurface, VisualTestContext) {
    cx.defer_next_window_frame_requests();
    let dock = cx.update(|cx| {
        let items = [ItemId::new(1), ItemId::new(2)];
        let content = DockspaceNode::equal_split(
            DockspaceAxis::Horizontal,
            items.map(|item| DockspaceNode::tabs([item])),
        )
        .unwrap();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::rootless(SURFACE)
            .with_contained(DockspaceContainedLayout::new(
                FloatingPresentationId::new(1),
                DockspaceRootLayout::new(RootId::new(1), content),
                LogicalRect::new(100.0, 100.0, 500.0, 350.0).unwrap(),
            ))])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let panes = items.map(|item| DockPane::new(item, "Pane", cx.new(|_| Empty)));
        DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    });
    let window = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(SURFACE, true, window, cx).unwrap();
        Root(dock.clone())
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    for _ in 0..4 {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }
    assert!(
        dock.host
            .borrow()
            .session
            .presented_surface(SURFACE)
            .is_some()
    );
    (dock, visual)
}

fn pointer(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    position: Point<Pixels>,
    event: NativePointerEvent,
) -> (
    Vec<(NativeReceiverQuery, NativeReceiverAnswer)>,
    Result<(), DockspaceRuntimeError>,
) {
    let (mut queries, result) = pointer_queries(dock, visual, position, event);
    queries.retain(|(query, _)| matches!(query, NativeReceiverQuery::Surface(_)));
    (queries, result)
}

fn pointer_queries(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    position: Point<Pixels>,
    event: NativePointerEvent,
) -> (
    Vec<(NativeReceiverQuery, NativeReceiverAnswer)>,
    Result<(), DockspaceRuntimeError>,
) {
    visual.update(|window, cx| {
        let mut host = dock.host.borrow_mut();
        let binding =
            host.native.as_ref().unwrap().callbacks[&window.window_handle().window_id()].binding();
        let sample = local_sample(&host.session, binding, position, window).unwrap();
        let metadata = PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at());
        host.session
            .record_native_pointer(
                NativePointerInput::new_local(
                    NativePointerId::new(1),
                    event,
                    NativeLocalPointerLocation::Exact(sample),
                    NativePointerOwner::Native(binding),
                    if matches!(event, NativePointerEvent::Moved) {
                        NativePointerOwner::Native(binding)
                    } else {
                        NativePointerOwner::Unknown
                    },
                )
                .with_metadata(metadata),
            )
            .unwrap();
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut queries = Vec::new();
        let result = session
            .begin_native_host_frame(|query| {
                let answer = resolve(query, mounts, native.as_ref().unwrap(), Some(window), cx);
                queries.push((query, answer));
                answer
            })
            .and_then(|mut frame| {
                frame.complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)?;
                frame.commit()
            });
        let result = result.map(|report| host.apply_report(report));
        (queries, result)
    })
}

#[open_gpui::test]
fn clipped_native_regions_leave_confirmed_window_background(cx: &mut TestAppContext) {
    let (dock, mut visual) = fixture(cx);
    let position = point(px(24.0), px(550.0));
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let frame = &host.mounts[&window.window_handle().window_id()].input_frames[0];
        assert!(frame.geometry.displayed_bounds().contains(&position));
        assert!(!frame.clip.contains(&position));
    });
    let (queries, result) = pointer_queries(
        &dock,
        &mut visual,
        position,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    result.unwrap();
    assert!(
        matches!(
            queries.as_slice(),
            [(
                NativeReceiverQuery::WindowHit(_),
                NativeReceiverAnswer::WindowHit(NativeWindowHit::WindowBackground)
            )]
        ),
        "{queries:?}"
    );
    assert!(!dock.has_active_gesture());
}

#[open_gpui::test]
fn window_hit_waits_for_the_region_roster_and_reports_foreign_overlays(cx: &mut TestAppContext) {
    let (dock, mut visual) = fixture(cx);
    let position = point(px(24.0), px(550.0));
    let (queries, result) = pointer_queries(
        &dock,
        &mut visual,
        position,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    result.unwrap();
    let [(query @ NativeReceiverQuery::WindowHit(_), _)] = queries.as_slice() else {
        panic!("expected a window hit query at the clipped edge: {queries:?}");
    };
    visual.update(|window, _| {
        let mut host = dock.host.borrow_mut();
        let mount = host
            .mounts
            .get_mut(&window.window_handle().window_id())
            .unwrap();
        let foreign_overlay = open_gpui::HitboxId::placeholder();
        assert!(mount.input_frames.iter().all(|frame| {
            !frame.hitboxes.contains(&foreign_overlay)
                && !frame.content_hitboxes.contains(&foreign_overlay)
        }));
        // The point is outside the region's clip. It still hits a framework
        // overlay, so it must not confirm that the pointer reached background.
        assert_eq!(
            resolve_in_window(*query, mount, |_| Some(Some(foreign_overlay))),
            NativeReceiverAnswer::WindowHit(NativeWindowHit::Blocked),
        );
        let sibling = SurfaceId::new(2);
        mount.regions.insert(sibling, Default::default());
        assert_eq!(
            resolve_in_window(*query, mount, |_| Some(None)),
            NativeReceiverAnswer::WindowHit(NativeWindowHit::Unknown),
            "an unpresented sibling cannot prove window background",
        );
        let (serial, artifact) = mount.regions[&SURFACE].presented.unwrap();
        let mut stale = mount.input_frames[0].clone();
        stale.surface = sibling;
        mount.input_frames.push(stale);
        mount.regions.get_mut(&sibling).unwrap().presented = Some((serial + 1, artifact));
        assert_eq!(
            resolve_in_window(*query, mount, |_| Some(None)),
            NativeReceiverAnswer::WindowHit(NativeWindowHit::Unknown),
            "a stale sibling hit graph cannot prove window background",
        );
    });
}

#[open_gpui::test]
fn a_frontmost_drag_only_splitter_blocks_click_and_accepts_drag(cx: &mut TestAppContext) {
    let (dock, mut visual) = fixture(cx);
    let (descriptor, position) = visual.update(|window, _| {
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_handle().window_id()];
        let frame = &mount.input_frames[0];
        let (descriptor, hitbox) = frame
            .receivers
            .iter()
            .find(|(descriptor, _)| descriptor.role() == DockspaceReceiverRole::Splitter)
            .unwrap();
        let bounds = descriptor.bounds();
        let logical = LogicalPoint::new(
            bounds.x() + bounds.width() * 0.25,
            bounds.y() + bounds.height() * 0.75,
        )
        .unwrap();
        let position = frame
            .geometry
            .local_to_window_point(point(px(logical.x() as f32), px(logical.y() as f32)))
            .unwrap();
        assert_eq!(hitbox.window_point_target_rank(position, window), Some(0));
        let semantic = host
            .session
            .host_pointer_hit_for_device(SURFACE, logical, PointerDeviceKind::Mouse)
            .unwrap();
        assert_eq!(
            semantic.delivery().unwrap().descriptor().role(),
            DockspaceReceiverRole::PaneBody
        );
        assert_eq!(semantic.drag_delivery().unwrap().descriptor(), *descriptor);
        (*descriptor, position)
    });
    let (queries, result) = pointer(
        &dock,
        &mut visual,
        position,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    let [
        (NativeReceiverQuery::Surface(click), click_answer),
        (NativeReceiverQuery::Surface(drag), drag_answer),
    ] = queries.as_slice()
    else {
        panic!("expected independent click and drag queries: {queries:?}");
    };
    assert_eq!(click.purpose(), &NativeReceiverPurpose::ClickDelivery);
    assert!(click.bind_receiver(&descriptor).is_none());
    assert_eq!(
        *click_answer,
        NativeReceiverAnswer::Blocked(click.presented_surface())
    );
    assert_eq!(drag.purpose(), &NativeReceiverPurpose::DragDelivery);
    let receiver = drag.bind_receiver(&descriptor).unwrap();
    assert_eq!(*drag_answer, NativeReceiverAnswer::Dock(receiver));
    result.unwrap();
    assert!(dock.host.borrow().session.has_active_gesture());
}

#[open_gpui::test]
fn an_exposed_empty_background_keeps_both_delivery_lanes_without_receivers(
    cx: &mut TestAppContext,
) {
    let (dock, mut visual) = fixture(cx);
    let position = point(px(24.0), px(24.0));
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let frame = &host.mounts[&window.window_handle().window_id()].input_frames[0];
        assert!(
            frame
                .hitboxes
                .iter()
                .any(|hitbox| hitbox.window_point_target_rank(position, window) == Some(0))
        );
        assert!(
            frame.receivers.iter().all(|(_, hitbox)| {
                hitbox.window_point_target_rank(position, window) != Some(0)
            })
        );
        let logical = LogicalPoint::new(24.0, 24.0).unwrap();
        let semantic = host
            .session
            .host_pointer_hit_for_device(SURFACE, logical, PointerDeviceKind::Mouse)
            .unwrap();
        assert!(semantic.delivery().is_none());
        assert!(semantic.drag_delivery().is_none());
    });
    let (queries, result) = pointer(
        &dock,
        &mut visual,
        position,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    assert_eq!(queries.len(), 2, "{queries:?}");
    for (query, answer) in queries {
        let NativeReceiverQuery::Surface(query) = query else {
            unreachable!()
        };
        assert_eq!(
            answer,
            NativeReceiverAnswer::EmptyLane(query.presented_surface())
        );
    }
    result.unwrap();
    assert!(!dock.host.borrow().session.has_active_gesture());
}

#[open_gpui::test]
fn an_exposed_rootless_background_clears_hover_without_inventing_a_drop_target(
    cx: &mut TestAppContext,
) {
    let (dock, mut visual) = fixture(cx);
    let source = visual.update(|window, _| {
        let host = dock.host.borrow();
        let frame = &host.mounts[&window.window_handle().window_id()].input_frames[0];
        let (descriptor, hitbox) = frame
            .receivers
            .iter()
            .find(|(descriptor, _)| descriptor.role() == DockspaceReceiverRole::TabBody)
            .unwrap();
        let center = descriptor.center();
        let position = frame
            .geometry
            .local_to_window_point(point(px(center.x() as f32), px(center.y() as f32)))
            .unwrap();
        assert_eq!(hitbox.window_point_target_rank(position, window), Some(0));
        position
    });
    pointer(
        &dock,
        &mut visual,
        source,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    )
    .1
    .unwrap();
    assert!(dock.host.borrow().session.has_active_gesture());

    let position = point(px(24.0), px(24.0));
    let (queries, result) = pointer(&dock, &mut visual, position, NativePointerEvent::Moved);
    result.expect("framework clearance must let the core resolve the hover winner");
    let [(NativeReceiverQuery::Surface(query), answer)] = queries.as_slice() else {
        panic!("expected one native hover query: {queries:?}");
    };
    assert_eq!(query.purpose(), &NativeReceiverPurpose::HoverHit);
    let clear = NativeReceiverAnswer::ClearHoverLane(query.presented_surface());
    assert_eq!(*answer, clear);

    visual.update(|window, _| {
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_handle().window_id()];
        let frame = &mount.input_frames[0];
        let background = frame.hitboxes[0];
        let (_, artifact) = mount.regions[&SURFACE].presented.unwrap();
        assert!(query.matches_presentation_artifact(artifact));
        assert_eq!(
            background.window_point_target_rank(position, window),
            Some(0)
        );
        assert!(
            frame.receivers.iter().all(|(_, hitbox)| {
                hitbox.window_point_target_rank(position, window) != Some(0)
            })
        );
        let semantic = host
            .session
            .host_pointer_hit_for_device(SURFACE, query.point().unwrap(), PointerDeviceKind::Mouse)
            .expect("the presented surface exposes its background hit facts");
        assert!(semantic.delivery().is_none());
        assert!(semantic.drag_delivery().is_none());
        assert!(
            semantic.hover().is_none(),
            "framework clearance must not invent a rootless background target"
        );
        assert_eq!(
            resolve_in_window((*query).into(), mount, |_| Some(Some(background))),
            clear
        );
        assert_ne!(
            clear,
            NativeReceiverAnswer::EmptyLane(query.presented_surface())
        );
        assert_eq!(
            resolve_in_window((*query).into(), mount, |_| Some(None)),
            NativeReceiverAnswer::Blocked(query.presented_surface()),
            "an external frontmost widget must still block hover"
        );
    });
    assert!(dock.error().is_none(), "{:?}", dock.error());
}

#[open_gpui::test]
fn a_real_hit_without_matching_presentation_authority_remains_unknown(cx: &mut TestAppContext) {
    let (dock, mut visual) = fixture(cx);
    let (queries, result) = pointer(
        &dock,
        &mut visual,
        point(px(24.0), px(24.0)),
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    result.unwrap();
    assert_eq!(queries.len(), 2, "{queries:?}");
    visual.update(|window, _| {
        let mut host = dock.host.borrow_mut();
        let mount = host
            .mounts
            .get_mut(&window.window_handle().window_id())
            .unwrap();
        let presented = mount
            .regions
            .get_mut(&SURFACE)
            .unwrap()
            .presented
            .take()
            .unwrap();
        for (query, _) in queries {
            assert_eq!(
                resolve_in_window(query, mount, |point| Some(
                    window.frontmost_hitbox_at(point)
                )),
                NativeReceiverAnswer::Unknown
            );
        }
        mount.regions.get_mut(&SURFACE).unwrap().presented = Some(presented);
    });
}

#[open_gpui::test]
fn managed_desktop_drag_delivers_to_another_presented_window(cx: &mut TestAppContext) {
    managed_desktop_drag(cx, DesktopDrag::Tab);
}

#[open_gpui::test]
fn managed_desktop_drag_without_button_authority_keeps_the_source(cx: &mut TestAppContext) {
    managed_desktop_drag(cx, DesktopDrag::UnobservedButton);
}

#[open_gpui::test]
fn managed_desktop_drag_outside_all_records_a_native_preview(cx: &mut TestAppContext) {
    managed_desktop_drag(cx, DesktopDrag::OutsideAll);
}

#[open_gpui::test]
fn changing_modifiers_preserves_the_group_during_a_managed_desktop_drag(cx: &mut TestAppContext) {
    managed_desktop_drag(cx, DesktopDrag::PreserveGroup);
}

enum DesktopDrag {
    Tab,
    UnobservedButton,
    OutsideAll,
    PreserveGroup,
}

fn managed_desktop_drag(cx: &mut TestAppContext, drag: DesktopDrag) {
    use std::time::Duration;

    use dockspace::{
        geometry::PhysicalPoint,
        runtime::{NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerHover},
    };
    use open_gpui::{
        Bounds, DevicePixels, Modifiers, MouseButton, PlatformPhysicalDisplayObservation,
        PlatformWindowHitStack, WindowOptions,
    };

    let button_state_available = !matches!(drag, DesktopDrag::UnobservedButton);
    let outside_all = matches!(drag, DesktopDrag::OutsideAll);
    let preserve_group = matches!(drag, DesktopDrag::PreserveGroup);
    let source = SurfaceId::new(1);
    let target = SurfaceId::new(2);
    let item = ItemId::new(1);
    cx.defer_next_window_frame_requests();
    cx.set_platform_mouse_button_is_pressed(
        MouseButton::Left,
        button_state_available.then_some(false),
    );
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(0), DevicePixels(0)),
            vec![],
        )
        .unwrap(),
    );
    let display = cx.update(|cx| {
        let snapshot = cx.display_snapshot();
        let bounds = Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(3200), DevicePixels(1800)),
        );
        PlatformPhysicalDisplayObservation::try_new(
            snapshot.generation().unwrap(),
            snapshot.displays()[0].id(),
            bounds,
            bounds,
            1.0,
        )
        .unwrap()
    });
    cx.simulate_physical_display_roster(Some(vec![display]));
    let dock = cx.update(|cx| {
        let mut items = vec![item, ItemId::new(2), ItemId::new(3)];
        let mut source_node = DockspaceNode::tabs([item, ItemId::new(2)]);
        if preserve_group {
            let remaining = ItemId::new(4);
            items.push(remaining);
            source_node = DockspaceNode::equal_split(
                DockspaceAxis::Horizontal,
                [source_node, DockspaceNode::tabs([remaining])],
            )
            .unwrap();
        }
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(
                source,
                DockspaceRootLayout::new(RootId::new(1), source_node),
            ),
            DockspaceSurfaceLayout::new(
                target,
                DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([ItemId::new(3)])),
            ),
        ])
        .unwrap();
        let panes = items
            .into_iter()
            .map(|item| DockPane::new(item, "Pane", cx.new(|_| Empty)))
            .collect::<Vec<_>>();
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        DockSurface::new_managed(
            DockspaceSession::from_layout(layout, policy).unwrap(),
            panes,
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap()
    });
    let windows = [source, target].map(|surface| {
        cx.update(|cx| {
            dock.open_native_root(surface, surface == source, WindowOptions::default(), cx)
        })
        .unwrap()
    });
    for (index, window) in windows.iter().enumerate() {
        cx.set_platform_window_physical_client_geometry(
            *window,
            Some(Bounds::new(
                point(DevicePixels(index as i32 * 1000), DevicePixels(0)),
                size(DevicePixels(800), DevicePixels(600)),
            )),
            1.0,
        );
    }
    let mut visuals = windows.map(|window| VisualTestContext::from_window(window, cx));
    let paint = |visuals: &mut [VisualTestContext; 2], cx: &mut TestAppContext| {
        for _ in 0..4 {
            for visual in &mut *visuals {
                visual.simulate_frame(RequestFrameOptions {
                    require_presentation: true,
                    force_render: true,
                    ..Default::default()
                });
            }
            cx.run_until_parked();
            cx.update(|cx| dock.pump_native(cx));
            assert!(dock.error().is_none(), "{:?}", dock.error());
        }
    };
    paint(&mut visuals, cx);
    let [source_binding, target_binding] =
        windows.map(|window| dock.native_binding_for_window(window.window_id()).unwrap());
    let source_group = dock.with_view(|view| view.item(item).unwrap().tabs().unwrap().key());
    let pressed = if preserve_group {
        let host = dock.host.borrow();
        let input = &host.mounts[&windows[0].window_id()].input_frames[0];
        let grip = input
            .receivers
            .iter()
            .find(|(receiver, _)| receiver.role() == DockspaceReceiverRole::TabGroupGrip)
            .unwrap()
            .0
            .center();
        (grip.x(), grip.y())
    } else {
        (32.0, 14.0)
    };
    for (index, (event, x, y, hover)) in [
        (
            NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
            pressed.0,
            pressed.1,
            source_binding,
        ),
        (
            NativePointerEvent::Moved,
            pressed.0 + 32.0,
            pressed.1,
            source_binding,
        ),
        (NativePointerEvent::Moved, 1032.0, 14.0, target_binding),
        (
            NativePointerEvent::ButtonReleased(NativePointerButton::Primary),
            1032.0,
            14.0,
            target_binding,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if outside_all && matches!(event, NativePointerEvent::ButtonReleased(_)) {
            break;
        }
        if preserve_group && index == 2 {
            assert!(dock.has_active_gesture());
            visuals[0].simulate_modifiers_change(Modifiers {
                alt: true,
                ..Modifiers::none()
            });
        }
        let (x, y, hover, work_area) = if outside_all && index == 2 {
            let work_area = dock.host.borrow().session.native_work_area(
                dockspace::runtime::HostWorkAreaToken::new(u64::from(display.display_id())),
            );
            assert!(work_area.is_some());
            (900.0, 700.0, NativePointerHover::OutsideAll, work_area)
        } else {
            (x, y, NativePointerHover::Dock(hover), None)
        };
        let input = NativePointerInput::new(
            NativePointerId::new(1),
            event,
            NativeDesktopPointerLocation::new(
                NativeDesktopPosition::Exact(PhysicalPoint::new(x, y).unwrap()),
                hover,
                work_area,
            ),
            NativePointerOwner::Native(source_binding),
            NativePointerOwner::Native(source_binding),
        )
        .with_metadata(PointerEventMetadata::new(
            PointerDeviceKind::Mouse,
            Duration::from_millis(index as u64),
        ));
        cx.update(|cx| dock.native_transport().unwrap().pointer(input, cx));
        paint(&mut visuals, cx);
    }
    if outside_all {
        assert!(dock.has_active_gesture());
        assert!(
            dock.host.borrow().mounts[&windows[0].window_id()].regions[&source]
                .packet
                .as_ref()
                .unwrap()
                .native_preview
                .is_some(),
            "an authoritative outside-all move must repaint the source's native preview"
        );
        let serial = dock.host.borrow().mounts[&windows[0].window_id()].serial;
        paint(&mut visuals, cx);
        assert_eq!(
            dock.host.borrow().mounts[&windows[0].window_id()].serial,
            serial,
            "presentation feedback must not continuously re-encode the preview"
        );
        cx.update(|cx| dock.native_transport().unwrap().device_removed(cx));
        paint(&mut visuals, cx);
        assert!(
            dock.host.borrow().mounts[&windows[0].window_id()].regions[&source]
                .packet
                .as_ref()
                .unwrap()
                .native_preview
                .is_none(),
            "cancellation must repaint the source to clear the retained native preview"
        );
    } else {
        dock.with_view(|view| {
            let moved = view.item(item).unwrap();
            assert_eq!(
                moved.surface(),
                if button_state_available {
                    target
                } else {
                    source
                },
                "desktop delivery must respect the backend's button authority"
            );
            assert!(moved.is_active());
            if preserve_group {
                assert_eq!(moved.tabs().unwrap().key(), source_group);
                assert_eq!(
                    moved.tabs().unwrap().tabs(),
                    [item.into(), ItemId::new(2).into()]
                );
                assert_eq!(view.item(ItemId::new(4)).unwrap().surface(), source);
            }
        });
    }
    assert!(!dock.has_active_gesture());
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    assert!(dock.native_is_quiescent());
}
