//! Custom native roots retain staging, first-live and group-lifetime authority.

#[path = "host_native_factory_tests/group_menu.rs"]
mod group_menu;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dockspace::{
    geometry::PhysicalRect,
    model::{
        DockMoveSource, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
        DockspaceSurfaceLayout, ItemId, NativeWindowPlacement, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::DockspaceSession,
};
use open_gpui::{
    AppContext as _, Bounds, Context, DevicePixels, DisplayId, IntoElement, Modifiers,
    PlatformWindowPresentOutcome, Render, RequestFrameOptions, TestAppContext, VisualTestContext,
    Window, WindowBounds, WindowMutationDomain, WindowOptions, div, point, prelude::*, px, size,
};

use super::{NativeBridgeError, NativeReport};
use crate::adapter::{DockPane, DockSurface, GpuiDockspaceConfig};

const SOURCE: SurfaceId = SurfaceId::new(81);
const TARGET: SurfaceId = SurfaceId::new(82);
const MOVED: ItemId = ItemId::new(91);
const REMAINING: ItemId = ItemId::new(92);
const TARGET_ITEM: ItemId = ItemId::new(93);
const PANE_SELECTOR: &str = "custom-factory-pane:91";

struct Pane {
    item: ItemId,
    clicks: usize,
}

impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let item = self.item;
        div().size_full().child(
            div()
                .id("edit")
                .debug_selector(move || format!("custom-factory-pane:{}", item.get()))
                .w(px(160.0))
                .h(px(32.0))
                .on_click(cx.listener(|pane, _, _, cx| {
                    pane.clicks += 1;
                    cx.notify();
                }))
                .child(format!("Edits: {}", self.clicks)),
        )
    }
}

struct Root {
    dock: DockSurface,
    surface: SurfaceId,
    renders: Rc<Cell<usize>>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().child(self.dock.element(self.surface))
    }
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

#[open_gpui::test]
fn a_custom_factory_renders_a_new_staging_surface_and_admits_the_original_pane(
    cx: &mut TestAppContext,
) {
    cx.simulate_display_added(DisplayId::new(2), None);
    let (dock, pane) = cx.update(|cx| {
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(
                SOURCE,
                DockspaceRootLayout::new(RootId::new(1), DockspaceNode::tabs([MOVED, REMAINING])),
            ),
            DockspaceSurfaceLayout::new(
                TARGET,
                DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([TARGET_ITEM])),
            ),
        ])
        .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        let session = DockspaceSession::from_layout(layout, policy).unwrap();
        let pane = cx.new(|_| Pane {
            item: MOVED,
            clicks: 0,
        });
        let remaining = cx.new(|_| Pane {
            item: REMAINING,
            clicks: 0,
        });
        let target = cx.new(|_| Pane {
            item: TARGET_ITEM,
            clicks: 0,
        });
        let dock = DockSurface::new_managed(
            session,
            [
                DockPane::new(MOVED, "Original pane", pane.clone()),
                DockPane::new(REMAINING, "Remaining pane", remaining),
                DockPane::new(TARGET_ITEM, "Target pane", target),
            ],
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap();
        (dock, pane)
    });
    cx.defer_next_window_frame_requests();
    let source = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(SOURCE, false, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surface: SOURCE,
            renders: Rc::default(),
        }
    });
    // TestWindow has no decorations: this is both its client and outer geometry.
    cx.set_platform_window_physical_client_geometry(
        source.into(),
        Some(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(800), DevicePixels(600)),
        )),
        1.0,
    );
    cx.update_window(source.into(), |_, window, cx| window.bounds_changed(cx))
        .unwrap();
    cx.defer_next_window_frame_requests();
    let target = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(TARGET, true, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surface: TARGET,
            renders: Rc::default(),
        }
    });
    cx.set_platform_window_physical_client_geometry(
        target.into(),
        Some(Bounds::new(
            point(DevicePixels(0), DevicePixels(700)),
            size(DevicePixels(800), DevicePixels(600)),
        )),
        1.0,
    );
    cx.update_window(target.into(), |_, window, cx| window.bounds_changed(cx))
        .unwrap();
    let mut source_visual = VisualTestContext::from_window(source.into(), cx);
    frames(&mut source_visual, &dock);
    let mut target_visual = VisualTestContext::from_window(target.into(), cx);
    frames(&mut target_visual, &dock);
    assert!(
        target_visual
            .debug_bounds("custom-factory-pane:93")
            .is_some()
    );
    let source_callbacks = dock
        .native_callbacks_for_window(source.window_id())
        .expect("the non-anchor source must have its original native route");
    let target_callbacks = dock
        .native_callbacks_for_window(target.window_id())
        .expect("the independent global anchor must have its original native route");
    let content = source_visual
        .debug_bounds(PANE_SELECTOR)
        .expect("original source pane");
    source_visual.simulate_click(content.center(), Modifiers::default());
    frames(&mut source_visual, &dock);
    assert_eq!(pane.read_with(&source_visual, |pane, _| pane.clicks), 1);
    let identity = pane.entity_id();

    let created = Rc::new(RefCell::new(None));
    let created_by_factory = created.clone();
    let child_renders = Rc::new(Cell::new(0));
    let factory_renders = child_renders.clone();
    dock.set_native_window_factory(move |dock, request, callbacks, cx| {
        assert!(
            created_by_factory.borrow().is_none(),
            "one semantic tear-off creates one window"
        );
        assert_ne!(request.binding.surface(), SOURCE);
        assert_ne!(request.binding.surface(), TARGET);
        assert!(dock.with_view(|view| view.surface(request.binding.surface()).is_none()));
        assert_eq!(
            dock.with_view(|view| view.item(MOVED).unwrap().surface()),
            SOURCE
        );
        assert_eq!(callbacks.binding(), request.binding.window());
        assert_eq!(callbacks.create_request().unwrap().binding, request.binding);
        let window = callbacks.open(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.0), px(0.0)),
                    size(px(640.0), px(480.0)),
                ))),
                display_id: Some(DisplayId::new(2)),
                focus_on_appearing: false,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Root {
                    dock: dock.clone(),
                    surface: request.binding.surface(),
                    renders: factory_renders.clone(),
                })
            },
            cx,
        )?;
        // GPUI naturally draws during open, before the bridge gets this handle.
        assert!(
            factory_renders.get() > 0,
            "the custom root must render before its factory returns"
        );
        assert!(dock.error().is_none(), "{:?}", dock.error());
        assert!(dock.with_view(|view| view.surface(request.binding.surface()).is_none()));
        let enrolled = dock
            .native_callbacks_for_window(window.window_id())
            .expect("the first render must enroll the original callback route during dispatch");
        assert_eq!(enrolled.binding(), callbacks.binding());
        assert_eq!(enrolled.window_id(), Some(window.window_id()));
        assert_eq!(enrolled.create_request().unwrap().binding, request.binding);
        *created_by_factory.borrow_mut() = Some((request, callbacks.clone(), window));
        Ok(window)
    })
    .unwrap();

    let tear_off = dock
        .host
        .borrow()
        .session
        .native_window_command(
            DockMoveSource::Item(MOVED),
            NativeWindowPlacement::new(PhysicalRect::new(900.0, 40.0, 640.0, 480.0).unwrap()),
        )
        .unwrap()
        .into_prepared()
        .unwrap();
    dock.take_native_reports();
    cx.defer_next_window_frame_requests();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.submit_prepared_action(tear_off)?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    let (request, original_callbacks, child) = created
        .borrow()
        .as_ref()
        .expect("the public tear-off command must invoke the custom native factory")
        .clone();
    let surface = request.binding.surface();
    assert_ne!(child, source.into());
    assert_ne!(child, target.into());
    assert_eq!(cx.read(|cx| cx.windows().len()), 3);
    assert!(dock.with_view(|view| view.surface(surface).is_none()));
    assert_eq!(
        dock.with_view(|view| view.item(MOVED).unwrap().surface()),
        SOURCE
    );
    assert_eq!(original_callbacks.window_id(), Some(child.window_id()));
    let mut child_visual = VisualTestContext::from_window(child, cx);
    frames(&mut child_visual, &dock);
    {
        let host = dock.host.borrow();
        let region = &host.mounts[&child.window_id()].regions[&surface];
        assert!(region.packet.as_ref().unwrap().is_deferred());
        assert!(region.presented.is_none());
        assert!(host.session.presented_surface(surface).is_none());
        assert_eq!(host.panels[&MOVED].view.entity_id(), identity);
    }
    assert!(child_visual.debug_bounds(PANE_SELECTOR).is_none());
    assert!(
        !dock
            .take_native_reports()
            .iter()
            .any(|report| matches!(report, NativeReport::Admitted(_)))
    );

    // Placement readiness alone cannot transfer the pane; first-live needs the
    // original renderer submission, which is deliberately deferred here.
    child_visual.set_window_present_outcome(child, PlatformWindowPresentOutcome::Deferred);
    assert!(cx.flush_window_mutation(child, WindowMutationDomain::Placement));
    frames(&mut child_visual, &dock);
    assert_eq!(
        dock.with_view(|view| view.item(MOVED).unwrap().surface()),
        SOURCE
    );
    assert!(dock.with_view(|view| view.surface(surface).is_none()));
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&child.window_id()];
        let region = &mount.regions[&surface];
        let packet = region.packet.as_ref().unwrap();
        assert!(!packet.is_deferred());
        assert!(packet.pane_items().contains(&MOVED));
        assert!(mount.pending.is_some());
        assert!(region.presented.is_none());
        assert!(host.session.presented_surface(surface).is_none());
    }
    assert!(
        !dock
            .take_native_reports()
            .iter()
            .any(|report| matches!(report, NativeReport::Admitted(_)))
    );

    child_visual.set_window_present_outcome(child, PlatformWindowPresentOutcome::Submitted);
    frames(&mut child_visual, &dock);
    let reports = dock.take_native_reports();
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::Admitted(binding) if *binding == request.binding
            ))
            .count(),
        1,
        "only the exact first-live binding may be admitted: {reports:?}"
    );
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::Submission { binding, snapshot }
            if *binding == original_callbacks.binding() && snapshot.window_id() == child.window_id()
    )), "the original callback route must report the actual renderer submission: {reports:?}");
    assert!(
        !reports.iter().any(|report| matches!(
            report,
            NativeReport::StaleCallback(_) | NativeReport::OpenFailed { .. }
        )),
        "{reports:?}"
    );
    {
        let host = dock.host.borrow();
        assert_eq!(host.panels[&MOVED].view.entity_id(), identity);
        let item = host.session.view().item(MOVED).unwrap();
        assert_eq!(item.surface(), surface);
        assert!(item.is_active());
        let region = &host.mounts[&child.window_id()].regions[&surface];
        let packet = region.packet.as_ref().unwrap();
        assert!(!packet.is_deferred());
        assert!(packet.pane_items().contains(&MOVED));
        let (_, artifact) = region
            .presented
            .expect("the original pane must be presented");
        assert!(
            host.session
                .presented_surface(surface)
                .unwrap()
                .matches_presentation_artifact(artifact)
        );
    }
    frames(&mut source_visual, &dock);
    assert!(source_visual.debug_bounds(PANE_SELECTOR).is_none());
    assert!(
        source_visual
            .debug_bounds("custom-factory-pane:92")
            .is_some()
    );
    let content = child_visual
        .debug_bounds(PANE_SELECTOR)
        .expect("the retained pane in the custom root");
    child_visual.simulate_click(content.center(), Modifiers::default());
    frames(&mut child_visual, &dock);
    assert_eq!(pane.read_with(&child_visual, |pane, _| pane.clicks), 2);

    dock.take_native_reports();
    let held = cx.hold_window_native_terminal(child);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    let mut reports = dock.take_native_reports();
    assert!(
        reports.iter().any(|report| matches!(report,
            NativeReport::LogicalClosed(binding) if *binding == original_callbacks.binding()
        )),
        "the child must start retirement before either application root: {reports:?}"
    );
    // SOURCE is the child's recovery host even though TARGET is the global anchor.
    // Both physical application roots must outlive the child's unresolved cleanup.
    cx.read(|cx| {
        let windows = cx.windows();
        assert!(!windows.contains(&child));
        assert!(
            windows.contains(&source.into()),
            "the original non-anchor SOURCE must remain live for child recovery"
        );
        assert!(
            windows.contains(&target.into()),
            "the independent global TARGET anchor must remain live"
        );
    });
    for (window, callbacks) in [
        (source.into(), &source_callbacks),
        (target.into(), &target_callbacks),
        (child, &original_callbacks),
    ] {
        assert_eq!(
            dock.native_callbacks_for_window(window.window_id())
                .expect("unresolved retirement must retain the original native route")
                .binding(),
            callbacks.binding()
        );
        assert!(matches!(
            callbacks.seal(),
            Err(NativeBridgeError::NotQuiescent)
        ));
        assert!(
            !reports.iter().any(|report| matches!(report,
                NativeReport::NativeTerminal(binding) if *binding == callbacks.binding()
            )),
            "no original native lifetime may finish while the child terminal is held: {reports:?}"
        );
    }
    {
        let host = dock.host.borrow();
        for (window, callbacks) in [
            (source.window_id(), &source_callbacks),
            (target.window_id(), &target_callbacks),
        ] {
            assert!(host.mounts.contains_key(&window));
            assert!(
                host.session
                    .native_window_bindings()
                    .any(|binding| binding == callbacks.binding())
            );
        }
    }
    assert!(!dock.native_is_quiescent());

    assert!(held.release());
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    reports.extend(dock.take_native_reports());
    for (window, callbacks) in [
        (source.into(), &source_callbacks),
        (target.into(), &target_callbacks),
        (child, &original_callbacks),
    ] {
        assert_eq!(
            reports
                .iter()
                .filter(|report| matches!(report,
                    NativeReport::NativeTerminal(binding) if *binding == callbacks.binding()
                ))
                .count(),
            1,
            "each original native lifetime must reach terminal exactly once: {reports:?}"
        );
        callbacks
            .seal()
            .expect("the original callbacks must observe their own native terminal");
        assert!(
            dock.native_callbacks_for_window(window.window_id())
                .is_none()
        );
        assert!(
            dock.host
                .borrow()
                .session
                .retained_native_surface_bindings_for_window(callbacks.binding())
                .next()
                .is_none()
        );
    }
    assert!(
        !reports.iter().any(|report| matches!(
            report,
            NativeReport::StaleCallback(_) | NativeReport::OpenFailed { .. }
        )),
        "{reports:?}"
    );
    for surface in [SOURCE, TARGET, surface] {
        assert!(
            dock.host
                .borrow()
                .session
                .presented_surface(surface)
                .is_none()
        );
    }
    assert!(dock.native_is_quiescent());
    assert!(cx.read(|cx| cx.windows().is_empty()));
}
