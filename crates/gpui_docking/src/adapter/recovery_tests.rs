//! Native dock-back uses the recovery scene owned by the current GPUI host.

use super::*;
use dockspace::{
    geometry::PhysicalRect,
    model::{DockspaceActionRejection, NativeTearOffSource, NativeWindowPlacement},
    runtime::SurfaceUnavailableReason,
};
use open_gpui::{WindowMutationDomain, WindowPlacementRequest, WindowPlacementState};

#[open_gpui::test]
fn native_dock_back_preserves_recovery_geometry_until_the_host_marks_it_unavailable(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, views, _) = managed_workspace(cx);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(B, DockPlacement::Center(DockAnchor::Item(A)))?;
                frame.select_item_current(B)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    let root = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(FIRST, true, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surfaces: vec![FIRST],
        }
    });
    cx.update_window(root.into(), |_, window, _| {
        let bounds = open_gpui::Bounds::new(
            open_gpui::point(open_gpui::DevicePixels(0), open_gpui::DevicePixels(0)),
            size(open_gpui::DevicePixels(800), open_gpui::DevicePixels(600)),
        );
        let placement = window.physical_placement_for_outer_bounds(bounds).unwrap();
        assert!(matches!(
            window.request_physical_window_placement(placement),
            open_gpui::WindowMutationDispatch::Queued(_)
        ));
    })
    .unwrap();
    assert!(cx.flush_window_mutation(root.into(), WindowMutationDomain::Placement));
    let mut source = VisualTestContext::from_window(root.into(), cx);
    frames(&mut source, &dock);
    let identity = views[1].entity_id();
    let tear_off = dock
        .host
        .borrow()
        .session
        .native_window_command(
            NativeTearOffSource::Item(B),
            NativeWindowPlacement::new(PhysicalRect::new(20.0, 30.0, 600.0, 450.0).unwrap()),
        )
        .unwrap()
        .into_prepared()
        .unwrap();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.submit_prepared_action(tear_off)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    cx.run_until_parked();
    let child = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| *window != root.into())
            .expect("the tear-off must create its own physical window")
    });
    assert!(dock.error().is_none(), "{:?}", dock.error());
    assert_eq!(
        dock.with_view(|view| view.item(B).unwrap().surface()),
        FIRST,
        "staging cannot transfer the source View before exact first-live admission"
    );
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&child.window_id()];
        assert!(mount.regions.values().all(|region| {
            region
                .packet
                .as_ref()
                .is_some_and(|packet| packet.is_deferred())
                && region.presented.is_none()
        }));
    }
    assert!(cx.flush_window_mutation(child, WindowMutationDomain::Placement));
    frames(&mut VisualTestContext::from_window(child, cx), &dock);
    let (surface, native_root) = dock.with_view(|view| {
        let item = view.item(B).unwrap();
        (item.surface(), item.root())
    });
    assert_ne!(surface, FIRST);
    assert_painted_pane(&dock, child, surface, B);
    let dock_back = || {
        dock.host
            .borrow()
            .session
            .presentation_commands(native_root)
            .dock_back()
            .unwrap()
    };
    let unavailable = Some(DockspaceActionRejection::PresentationUnavailable { surface: FIRST });
    frames(&mut source, &dock);
    assert!(dock_back().is_ready());

    cx.update(|cx| dock.update(|_| Ok(()), cx)).unwrap();
    assert!(
        dock_back().is_ready(),
        "an input-only commit must retain the current recovery geometry"
    );
    cx.update(|cx| dock.pump_native(cx));
    assert!(dock_back().is_ready());

    cx.simulate_window_minimize(root.into());
    cx.update(|cx| dock.pump_native(cx));
    assert_eq!(dock_back().unavailable_reason(), unavailable);
    cx.update_window(root.into(), |_, window, _| {
        assert!(matches!(
            window.request_window_placement_request(WindowPlacementRequest {
                state: Some(WindowPlacementState::Windowed),
                ..WindowPlacementRequest::new()
            }),
            open_gpui::WindowMutationDispatch::Queued(_)
        ));
    })
    .unwrap();
    assert!(cx.flush_window_mutation(root.into(), WindowMutationDomain::Placement));
    frames(&mut source, &dock);
    assert!(dock_back().is_ready());

    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.defer_surface(FIRST, SurfaceUnavailableReason::Deferred)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    assert_eq!(
        dock_back().unavailable_reason(),
        unavailable,
        "an explicit unavailable contribution must revoke recovery readiness"
    );
    frames(&mut source, &dock);
    let action = dock_back().into_prepared().unwrap();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.submit_prepared_action(action)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    assert!(
        VisualTestContext::from_window(child, cx).simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        })
    );
    frames(&mut source, &dock);
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    assert_eq!(
        dock.with_view(|view| view.item(B).unwrap().surface()),
        FIRST
    );
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.select_item_current(B)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut source, &dock);
    assert_painted_pane(&dock, root.into(), FIRST, B);
    let content = source.debug_bounds("view:22").unwrap();
    source.simulate_click(content.center(), Modifiers::default());
    frames(&mut source, &dock);
    assert_eq!(views[1].entity_id(), identity);
    assert_eq!(views[1].read_with(&source, |view, _| view.clicks), 1);

    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}
