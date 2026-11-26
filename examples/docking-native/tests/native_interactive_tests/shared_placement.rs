//! Target-DPI placement through shared native creation and first-live admission.

use std::{cell::RefCell, rc::Rc};

use anyhow::{Context as _, ensure};
use open_gpui::{
    AppContext as _, Bounds, Context, DevicePixels, IntoElement, Render, Window, WindowId,
    WindowOptions, WindowPhysicalPlacementRequest, div, point, prelude::*, size,
};
use open_gpui_docking::{
    DockSurface,
    dockspace::{
        geometry::PhysicalRect,
        model::{DockMoveSource, NativeWindowPlacement, SurfaceId},
    },
    native::{NativeReport, NativeWindowRequest},
};
use open_gpui_windows::{
    native_test_client_screen_bounds, native_test_mixed_dpi_display_pair, native_test_window_rect,
};

use super::{
    native_generation_label,
    shared_native::run_shared_native_test,
    shared_recovery::{seed_source_view, wait_admitted_destination},
    shared_session::two_window_bounds_on_display,
    unique_native_identity_for_window, window_platform_facts,
};

struct PlacementRoot {
    dock: DockSurface,
    surface: SurfaceId,
}

impl Render for PlacementRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dock.element(self.surface))
    }
}

#[test]
#[ignore = "requires a real mixed-DPI display pair with a negative-coordinate monitor"]
fn real_windows_shared_native_placement_matches_target_dpi_in_both_directions() {
    let pair = native_test_mixed_dpi_display_pair()
        .expect("shared native placement requires distinct DPI displays and a negative monitor");
    let negative_display = pair.negative_display().display_id();
    for (source_display, target_display) in
        [(pair.lower(), pair.higher()), (pair.higher(), pair.lower())]
    {
        run_shared_native_test(Some(source_display), move |cx, session| {
            Box::pin(async move {
                let visible = target_display.physical_visible_bounds();
                let margin = 32;
                let width = (visible.size.width.0 - 2 * margin).min(720);
                let height = (visible.size.height.0 - 2 * margin).min(540);
                ensure!(
                    width >= 320 && height >= 240,
                    "selected target work area is too small for native placement: {visible:?}"
                );
                let planned = Bounds::new(
                    point(
                        DevicePixels(visible.origin.x.0 + (visible.size.width.0 - width) / 2),
                        DevicePixels(visible.origin.y.0 + (visible.size.height.0 - height) / 2),
                    ),
                    size(DevicePixels(width), DevicePixels(height)),
                );
                ensure!(
                    target_display.display_id() != negative_display
                        || planned.origin.x.0 < 0
                        || planned.origin.y.0 < 0,
                    "the selected negative monitor must receive a negative-coordinate rectangle: {planned:?}"
                );
                let planned_outer = PhysicalRect::new(
                    f64::from(planned.origin.x.0),
                    f64::from(planned.origin.y.0),
                    f64::from(width),
                    f64::from(height),
                )?;
                let root_bounds =
                    cx.update(|app| two_window_bounds_on_display(app, source_display))?;
                cx.update(|app| {
                    session.assert_roots_on_display(app, source_display, &root_bounds)
                })?;
                seed_source_view(cx, session).await?;
                let source = session.source_window()?;
                let target = session.target_window()?;
                let source_view = session
                    .source_view
                    .clone()
                    .context("shared placement requires the original stateful source View")?;
                let view_identity = source_view.entity_id();
                let roots = cx.update(|app| session.snapshot_roots(app))?;
                let root_bindings = [source, target].map(|owner| {
                    session
                        .dock
                        .native_binding_for_window(owner.window.window_id())
                });
                let (original_generations, _) = session.native_generation_sets();
                let recorded: Rc<
                    RefCell<
                        Option<(
                            WindowId,
                            NativeWindowRequest,
                            WindowPhysicalPlacementRequest,
                        )>,
                    >,
                > = Rc::new(RefCell::new(None));
                let capture = recorded.clone();
                session.milestones.push(format!(
                    "shared.native-placement.{:?}-to-{:?}.planned",
                    source_display.display_id(),
                    target_display.display_id(),
                ));
                // A custom factory explicitly opts out of the default native preview.
                session.dock.set_native_window_factory(move |dock, request, callbacks, app| {
                    assert!(capture.borrow().is_none(), "native creation duplicated its destination");
                    assert_eq!(request.outer_bounds, planned_outer, "core changed the frozen intent");
                    let dock = dock.clone();
                    let window = callbacks.open(
                        WindowOptions {
                            display_id: Some(source_display.display_id()),
                            focus_on_appearing: false,
                            ..Default::default()
                        },
                        move |_, cx| cx.new(|_| PlacementRoot {
                            dock,
                            surface: request.binding.surface(),
                        }),
                        app,
                    )?;
                    // Capture the owning backend's target-DPI conversion before the bridge mutates it.
                    let prepared = window
                        .update(app, |_, window, _| window.physical_placement_for_outer_bounds(planned))
                        .expect("factory destination must remain live before placement")
                        .expect("factory destination must support target-DPI outer-to-client conversion");
                    *capture.borrow_mut() = Some((window.window_id(), request, prepared));
                    Ok(window)
                })?;
                cx.update(|app| {
                    session.dock.update(
                        |frame| {
                            frame.move_to_new_window_current(
                                DockMoveSource::Item(source.item),
                                NativeWindowPlacement::new(planned_outer),
                            )?;
                            Ok(())
                        },
                        app,
                    )
                })?;
                let destination = wait_admitted_destination(cx, session).await?;
                let (window_id, request, prepared) = (*recorded.borrow())
                    .context("shared native creation did not capture its original request")?;
                let binding = session
                    .dock
                    .native_binding_for_window(destination.window.window_id())
                    .context("admitted placement lost its original native binding")?;
                ensure!(
                    request.outer_bounds == planned_outer
                        && window_id == destination.window.window_id()
                        && request.binding.surface() == destination.surface
                        && request.binding.window() == binding
                        && !root_bindings.contains(&Some(binding))
                        && destination.window != source.window
                        && destination.window != target.window
                        && destination.hwnd != source.hwnd
                        && destination.hwnd != target.hwnd,
                    "placement admitted a different destination from its original factory request: {request:?}, destination={destination:?}"
                );
                ensure!(
                    session
                        .reports
                        .iter()
                        .filter(|report| matches!(
                            report, NativeReport::Admitted(admitted) if *admitted == request.binding
                        ))
                        .count()
                        == 1,
                    "the original native request must have exactly one first-live admission"
                );
                let target_observation = prepared
                    .target_display()
                    .context("pre-mutation client conversion lost its target display")?;
                ensure!(
                    target_observation.display_id() == target_display.display_id()
                        && target_observation.display_id() != source_display.display_id()
                        && target_observation.bounds() == target_display.physical_bounds()
                        && target_observation.visible_bounds() == visible
                        && (target_observation.scale_factor() - target_display.scale_factor())
                            .abs()
                            <= 0.001,
                    "pre-mutation conversion did not carry the selected target DPI: prepared={prepared:?}, target={target_display:?}"
                );
                let facts = cx.update(|app| window_platform_facts(app, destination.window))?;
                let geometry = facts
                    .physical_geometry
                    .context("admitted placement has no physical geometry")?;
                let client = native_test_client_screen_bounds(destination.hwnd)?;
                ensure!(
                    client == prepared.client_bounds()
                        && geometry.client_bounds() == prepared.client_bounds()
                        && geometry.display_observation() == Some(target_observation)
                        && facts.display_id == Some(target_display.display_id())
                        && (facts.scale_factor - target_display.scale_factor()).abs() <= 0.001
                        && (geometry.scale_factor() - target_display.scale_factor()).abs() <= 0.001,
                    "admitted native geometry diverged from the pre-mutation conversion: prepared={prepared:?}, facts={facts:?}, native_client={client:?}"
                );
                // This independent outer check catches conversion with the wrong decoration DPI.
                let outer = native_test_window_rect(destination.hwnd)?;
                ensure!(
                    (outer.left, outer.top, outer.right, outer.bottom)
                        == (
                            planned.origin.x.0,
                            planned.origin.y.0,
                            planned.right().0,
                            planned.bottom().0
                        )
                        && geometry.outer_bounds() == Some(planned),
                    "native outer frame diverged from the original core intent: planned={planned:?}, native_outer={outer:?}, geometry={geometry:?}"
                );
                cx.update(|app| {
                    session.assert_roots_on_display(app, source_display, &root_bounds)
                })?;
                cx.update(|app| session.assert_roots_unchanged(app, &roots))?;
                let identity = destination
                    .identity
                    .context("admitted placement has no native generation")?;
                let (generations, _) = session.native_generation_sets();
                ensure!(
                    generations.len() == original_generations.len() + 1
                        && generations.is_superset(&original_generations)
                        && !original_generations.contains(&native_generation_label(identity))
                        && unique_native_identity_for_window(
                            &session.observation.events(),
                            window_id,
                            "placement destination",
                        )? == identity
                        && cx.update(|app| {
                            let windows = app.windows();
                            windows.len() == session.preexisting_window_count + 3
                                && [source.window, target.window, destination.window]
                                    .iter()
                                    .all(|window| windows.contains(window))
                        }),
                    "shared placement duplicated or replaced a native destination generation"
                );
                ensure!(
                    session
                        .source_view
                        .as_ref()
                        .is_some_and(|view| view.entity_id() == view_identity)
                        && cx.update(|app| {
                            let view = source_view.read(app);
                            view.local_edits == 41 && view.last_render == Some((window_id, 41))
                        }),
                    "shared placement replaced or reset the original typed source View"
                );
                eprintln!(
                    "shared native placement {:?}@{} -> {:?}@{}: outer={planned:?}, client={client:?}",
                    source_display.display_id(),
                    source_display.scale_factor(),
                    target_display.display_id(),
                    target_display.scale_factor(),
                );
                session.milestones.push(format!(
                    "shared.native-placement.{:?}-to-{:?}.admitted",
                    source_display.display_id(),
                    target_display.display_id(),
                ));
                Ok(())
            })
        });
    }
}
