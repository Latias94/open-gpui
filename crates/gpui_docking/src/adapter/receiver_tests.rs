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
        PointerDeviceKind, PointerEventMetadata, SurfaceUnavailableReason,
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
        div().size_full().child(self.0.element(SURFACE))
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

fn press(
    dock: &DockSurface,
    visual: &mut VisualTestContext,
    position: Point<Pixels>,
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
                    NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
                    NativeLocalPointerLocation::Exact(sample),
                    NativePointerOwner::Native(binding),
                    NativePointerOwner::Unknown,
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
                if matches!(query, NativeReceiverQuery::Surface(_)) {
                    queries.push((query, answer));
                }
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
        let position = frame.bounds.origin + point(px(logical.x() as f32), px(logical.y() as f32));
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
    let (queries, result) = press(&dock, &mut visual, position);
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
    let (queries, result) = press(&dock, &mut visual, position);
    assert_eq!(queries.len(), 2, "{queries:?}");
    for (query, answer) in queries {
        let NativeReceiverQuery::Surface(query) = query else {
            unreachable!()
        };
        assert_eq!(
            answer,
            NativeReceiverAnswer::NoReceiver(query.presented_surface())
        );
    }
    result.unwrap();
    assert!(!dock.host.borrow().session.has_active_gesture());
}

#[open_gpui::test]
fn a_real_hit_without_matching_presentation_authority_remains_unknown(cx: &mut TestAppContext) {
    let (dock, mut visual) = fixture(cx);
    let (queries, result) = press(&dock, &mut visual, point(px(24.0), px(24.0)));
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
                resolve_in_window(query, mount, |hitbox, point| {
                    hitbox.window_point_target_rank(point, window)
                }),
                NativeReceiverAnswer::Unknown
            );
        }
        mount.regions.get_mut(&SURFACE).unwrap().presented = Some(presented);
    });
}
