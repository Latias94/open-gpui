use std::{cell::RefCell, rc::Rc};

use dockspace::{
    close::{CloseDecision, ClosePlanTarget, SurfaceCloseDisposition},
    model::{
        DockspaceAxis, DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout,
        ItemId, RootId, SurfaceId,
    },
    policy::{CloseCapability, DockPolicy},
    runtime::{
        DockspaceClosePlan, DockspaceCloseResolution, DockspaceSession, HostInputOutcome,
        NativeSurfaceCloseAction,
    },
};
use open_gpui::{
    AppContext as _, Bounds, DevicePixels, DisplayId, Entity, Modifiers,
    PlatformWindowPresentOutcome, Role, TestAppContext, VisualTestContext, WindowBounds,
    WindowMutationDomain, WindowOptions, accesskit, point, px, size,
};

use super::{Pane, Root, SOURCE, frames};
use crate::adapter::{
    DockPane, DockSurface, GpuiDockspaceConfig,
    native::{NativeCloseDecision, NativeReport},
};

const MAIN_ROOT: RootId = RootId::new(1);
const MOVED_GROUP: [ItemId; 2] = [ItemId::new(101), ItemId::new(102)];
const SIBLING_GROUP: [ItemId; 2] = [ItemId::new(103), ItemId::new(104)];
const ITEMS: [ItemId; 4] = [
    MOVED_GROUP[0],
    MOVED_GROUP[1],
    SIBLING_GROUP[0],
    SIBLING_GROUP[1],
];

fn assert_group(dock: &DockSurface, items: [ItemId; 2], surface: SurfaceId) -> RootId {
    dock.with_view(|view| {
        let root = view.item(items[0]).unwrap().root();
        for (index, item) in items.into_iter().enumerate() {
            let location = view.item(item).unwrap();
            assert_eq!(location.surface(), surface);
            assert_eq!(location.root(), root);
            assert_eq!(location.tabs().unwrap().items().collect::<Vec<_>>(), items);
            assert_eq!(location.tabs().unwrap().selected(), Some(items[1].into()));
            assert_eq!(location.tab_index(), Some(index));
            assert_eq!(location.is_active(), index == 1);
            assert!(location.is_visible());
            assert!(location.contained().is_none());
        }
        root
    })
}

fn assert_original_views(
    dock: &DockSurface,
    views: &[Entity<Pane>; 4],
    edits: [usize; 4],
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let host = dock.host.borrow();
        for ((item, view), edits) in ITEMS.into_iter().zip(views).zip(edits) {
            assert_eq!(host.panels[&item].view.entity_id(), view.entity_id());
            assert_eq!(view.read(cx).clicks, edits);
        }
    });
}

fn click_accessible(visual: &VisualTestContext, role: Role, label: &str) {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (target, node) = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == role && node.label() == Some(label))
        .min_by(|(_, left), (_, right)| {
            left.bounds()
                .unwrap()
                .x0
                .total_cmp(&right.bounds().unwrap().x0)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing accessible {role:?}: {label}; nodes={:?}",
                tree.nodes
                    .iter()
                    .map(|(_, node)| (node.role(), node.label()))
                    .collect::<Vec<_>>()
            )
        });
    assert!(node.supports_action(accesskit::Action::Click));
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: accesskit::Action::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: *target,
            data: None,
        })
    );
}

fn edit_visible_pane(visual: &mut VisualTestContext, dock: &DockSurface, item: ItemId) {
    let content = visual
        .debug_bounds(&format!("custom-factory-pane:{}", item.get()))
        .expect("the original selected View must remain interactive");
    visual.simulate_click(content.center(), Modifiers::none());
    frames(visual, dock);
}

fn finish(cx: &mut TestAppContext, dock: &DockSurface) {
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
}

fn approve_close_item(
    dock: &DockSurface,
    plan: &DockspaceClosePlan,
    index: usize,
    cx: &mut TestAppContext,
) -> DockspaceCloseResolution {
    cx.update(|cx| {
        dock.update_action(
            |frame| {
                Ok(frame.resolve_close(
                    plan.request(),
                    plan.items()[index].token(),
                    CloseDecision::Allow,
                )?)
            },
            |outcome| match outcome {
                Some(HostInputOutcome::CloseDecisionProcessed { resolution, .. }) => *resolution,
                outcome => panic!("expected the original content decision, got {outcome:?}"),
            },
            cx,
        )
    })
    .unwrap()
}

#[open_gpui::test]
fn tabs_group_menu_waits_for_exact_first_live_and_closes_only_its_native_group(
    cx: &mut TestAppContext,
) {
    cx.simulate_display_added(DisplayId::new(2), None);
    let displays = cx.update(|cx| {
        let snapshot = cx.display_snapshot();
        snapshot
            .displays()
            .iter()
            .map(|display| {
                open_gpui::PlatformPhysicalDisplayObservation::try_new(
                    snapshot.generation().unwrap(),
                    display.id(),
                    display.bounds().to_device_pixels(1.0),
                    display.visible_bounds().to_device_pixels(1.0),
                    1.0,
                )
                .unwrap()
            })
            .collect()
    });
    cx.simulate_physical_display_roster(Some(displays));
    let plans = Rc::new(RefCell::new(Vec::new()));
    let received = plans.clone();
    let (dock, views) = cx.update(|cx| {
        let content = DockspaceNode::equal_split(
            DockspaceAxis::Horizontal,
            [MOVED_GROUP, SIBLING_GROUP]
                .map(|items| DockspaceNode::tabs_with_selection(items, Some(items[1])).unwrap()),
        )
        .unwrap();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SOURCE,
            DockspaceRootLayout::new(MAIN_ROOT, content),
        )])
        .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        policy.set_close_capability(CloseCapability::DeferredAllowed);
        let views: [Entity<Pane>; 4] = std::array::from_fn(|index| {
            cx.new(|_| Pane {
                item: ITEMS[index],
                clicks: index + 1,
            })
        });
        let panes = ITEMS
            .into_iter()
            .zip(&views)
            .map(|(item, view)| DockPane::new(item, format!("Pane {}", item.get()), view.clone()));
        let dock = DockSurface::new_managed(
            DockspaceSession::from_layout(layout, policy).unwrap(),
            panes,
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap();
        dock.set_close_handler(move |_, plan, _| received.borrow_mut().push(plan), cx)
            .unwrap();
        (dock, views)
    });
    cx.defer_next_window_frame_requests();
    let source = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(SOURCE, true, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surface: SOURCE,
            renders: Rc::default(),
        }
    });
    cx.set_platform_window_physical_client_geometry(
        source.into(),
        Some(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(800), DevicePixels(600)),
        )),
        1.0,
    );
    cx.update_window(source.into(), |_, window, cx| {
        window.bounds_changed(cx);
        let _ = window.activate_window();
    })
    .unwrap();
    let mut source_visual = VisualTestContext::from_window(source.into(), cx);
    assert!(source_visual.activate_accessibility());
    frames(&mut source_visual, &dock);
    let source_callbacks = dock
        .native_callbacks_for_window(source.window_id())
        .unwrap();
    assert_eq!(assert_group(&dock, MOVED_GROUP, SOURCE), MAIN_ROOT);
    assert_eq!(assert_group(&dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
    edit_visible_pane(&mut source_visual, &dock, MOVED_GROUP[1]);
    edit_visible_pane(&mut source_visual, &dock, SIBLING_GROUP[1]);
    assert_original_views(&dock, &views, [1, 3, 3, 5], cx);

    let created = Rc::new(RefCell::new(None));
    let created_by_factory = created.clone();
    dock.set_native_window_factory(move |dock, request, callbacks, cx| {
        assert!(created_by_factory.borrow().is_none());
        assert_ne!(request.binding.surface(), SOURCE);
        assert!(dock.with_view(|view| view.surface(request.binding.surface()).is_none()));
        assert_eq!(assert_group(dock, MOVED_GROUP, SOURCE), MAIN_ROOT);
        assert_eq!(assert_group(dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
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
                    renders: Rc::default(),
                })
            },
            cx,
        )?;
        assert!(dock.with_view(|view| view.surface(request.binding.surface()).is_none()));
        *created_by_factory.borrow_mut() = Some((request, callbacks.clone(), window));
        Ok(window)
    })
    .unwrap();

    click_accessible(&source_visual, Role::Button, "Tab group actions");
    frames(&mut source_visual, &dock);
    dock.take_native_reports();
    cx.defer_next_window_frame_requests();
    click_accessible(
        &source_visual,
        Role::MenuItem,
        "Move Tab Group to New Window",
    );
    finish(cx, &dock);
    let (request, callbacks, child) = created
        .borrow()
        .as_ref()
        .expect("the real group-menu action must invoke the native factory")
        .clone();
    let surface = request.binding.surface();
    assert_ne!(child, source.into());
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);
    assert_eq!(callbacks.window_id(), Some(child.window_id()));
    assert_eq!(callbacks.binding(), request.binding.window());
    assert_eq!(assert_group(&dock, MOVED_GROUP, SOURCE), MAIN_ROOT);
    assert_eq!(assert_group(&dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
    assert!(dock.with_view(|view| view.surface(surface).is_none()));
    let mut child_visual = VisualTestContext::from_window(child, cx);
    frames(&mut child_visual, &dock);
    {
        let host = dock.host.borrow();
        let region = &host.mounts[&child.window_id()].regions[&surface];
        assert!(region.packet.as_ref().unwrap().is_deferred());
        assert!(region.presented.is_none());
        assert!(host.session.presented_surface(surface).is_none());
    }
    assert!(
        child_visual
            .debug_bounds("custom-factory-pane:102")
            .is_none()
    );
    assert!(
        !dock
            .take_native_reports()
            .iter()
            .any(|report| matches!(report, NativeReport::Admitted(_)))
    );

    child_visual.set_window_present_outcome(child, PlatformWindowPresentOutcome::Deferred);
    assert!(cx.flush_window_mutation(child, WindowMutationDomain::Placement));
    frames(&mut child_visual, &dock);
    assert_eq!(assert_group(&dock, MOVED_GROUP, SOURCE), MAIN_ROOT);
    assert_eq!(assert_group(&dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
    assert!(dock.with_view(|view| view.surface(surface).is_none()));
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&child.window_id()];
        let region = &mount.regions[&surface];
        let packet = region.packet.as_ref().unwrap();
        assert!(!packet.is_deferred());
        assert_eq!(packet.pane_items(), [MOVED_GROUP[1]]);
        assert!(mount.pending.is_some());
        assert!(region.presented.is_none());
        assert!(host.session.presented_surface(surface).is_none());
    }
    assert_original_views(&dock, &views, [1, 3, 3, 5], cx);
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
        "the exact first-live group binding must be admitted once: {reports:?}"
    );
    assert!(
        reports.iter().any(|report| matches!(report,
            NativeReport::Submission { binding, snapshot }
                if *binding == callbacks.binding() && snapshot.window_id() == child.window_id()
        )),
        "first-live must use the original renderer submission: {reports:?}"
    );
    assert_ne!(assert_group(&dock, MOVED_GROUP, surface), MAIN_ROOT);
    assert_eq!(assert_group(&dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
    {
        let host = dock.host.borrow();
        let region = &host.mounts[&child.window_id()].regions[&surface];
        let (_, artifact) = region.presented.expect("the exact group must be presented");
        assert!(
            host.session
                .presented_surface(surface)
                .unwrap()
                .matches_presentation_artifact(artifact)
        );
    }
    frames(&mut source_visual, &dock);
    // The child's independent DPI can clip inactive tab labels. Check actual
    // selected content here; assert_group above retains both complete rosters.
    for item in [MOVED_GROUP[1], SIBLING_GROUP[1]] {
        let selector = format!("custom-factory-pane:{}", item.get());
        let moved = MOVED_GROUP.contains(&item);
        assert_eq!(source_visual.debug_bounds(&selector).is_some(), !moved);
        assert_eq!(child_visual.debug_bounds(&selector).is_some(), moved);
    }
    child_visual.update(|window, _| {
        let _ = window.activate_window();
    });
    frames(&mut child_visual, &dock);
    edit_visible_pane(&mut child_visual, &dock, MOVED_GROUP[1]);
    assert_original_views(&dock, &views, [1, 4, 3, 5], cx);

    dock.set_native_close_policy(NativeCloseDecision::Defer)
        .unwrap();
    assert!(!cx.simulate_window_close(child));
    finish(cx, &dock);
    assert!(plans.borrow().is_empty());
    let close = dock
        .take_native_reports()
        .iter()
        .find_map(|report| match report {
            NativeReport::CloseRequested(close) if close.binding() == request.binding => {
                Some(*close)
            }
            _ => None,
        })
        .expect("the child must supply its original intercepted close edge");
    cx.update(|cx| {
        dock.decide_native_close(
            close,
            NativeCloseDecision::Allow(NativeSurfaceCloseAction::CloseContent),
            cx,
        )
    })
    .unwrap();
    finish(cx, &dock);
    let plan = {
        let plans = plans.borrow();
        assert_eq!(plans.len(), 1);
        plans[0].clone()
    };
    assert_eq!(
        plan.target(),
        ClosePlanTarget::Surface {
            surface,
            disposition: SurfaceCloseDisposition::CloseContent,
        }
    );
    assert_eq!(
        plan.items()
            .iter()
            .map(|item| item.item())
            .collect::<Vec<_>>(),
        MOVED_GROUP
    );
    assert_group(&dock, MOVED_GROUP, surface);
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);
    assert_eq!(
        approve_close_item(&dock, &plan, 0, cx),
        DockspaceCloseResolution::Recorded {
            request: plan.request(),
            item: MOVED_GROUP[0],
        }
    );
    finish(cx, &dock);
    assert_group(&dock, MOVED_GROUP, surface);
    assert_eq!(assert_group(&dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);
    assert_eq!(
        approve_close_item(&dock, &plan, 1, cx),
        DockspaceCloseResolution::Approved {
            request: plan.request(),
        }
    );
    finish(cx, &dock);
    assert!(dock.with_view(|view| {
        MOVED_GROUP
            .into_iter()
            .all(|item| view.item(item).is_none())
    }));
    assert!(dock.with_view(|view| view.surface(surface).is_none()));
    assert_eq!(assert_group(&dock, SIBLING_GROUP, SOURCE), MAIN_ROOT);
    dock.with_view(|view| {
        let root = view.surface(SOURCE).unwrap().main_root().unwrap();
        assert_eq!(
            root.content()
                .unwrap()
                .tabs()
                .unwrap()
                .items()
                .collect::<Vec<_>>(),
            SIBLING_GROUP
        );
    });
    assert_eq!(cx.read(|cx| cx.windows()), [source.into()]);
    assert_eq!(
        dock.native_callbacks_for_window(source.window_id())
            .unwrap()
            .binding(),
        source_callbacks.binding()
    );
    assert!(
        dock.native_callbacks_for_window(child.window_id())
            .is_none()
    );
    let reports = dock.take_native_reports();
    for terminal in [false, true] {
        assert_eq!(
            reports
                .iter()
                .filter(|report| match report {
                    NativeReport::LogicalClosed(binding) if !terminal =>
                        *binding == callbacks.binding(),
                    NativeReport::NativeTerminal(binding) if terminal =>
                        *binding == callbacks.binding(),
                    _ => false,
                })
                .count(),
            1,
            "only the child must retire: {reports:?}"
        );
    }
    assert!(
        !reports.iter().any(|report| matches!(
            report,
            NativeReport::GroupShutdownRequested
                | NativeReport::StaleCallback(_)
                | NativeReport::OpenFailed { .. }
        )),
        "{reports:?}"
    );
    assert_original_views(&dock, &views, [1, 4, 3, 5], cx);
    source_visual.update(|window, _| {
        let _ = window.activate_window();
    });
    frames(&mut source_visual, &dock);
    edit_visible_pane(&mut source_visual, &dock, SIBLING_GROUP[1]);
    assert_original_views(&dock, &views, [1, 4, 3, 6], cx);

    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
    assert!(cx.read(|cx| cx.windows().is_empty()));
}
