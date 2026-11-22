//! Pane content ownership across metadata, semantic close and physical presentation.

use dockspace::{
    close::CloseDecision,
    geometry::{LogicalSize, PaneSizing},
    model::{
        DockAnchor, DockPlacement, DockspaceAxis, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceDocumentBootstrap, DockspaceDocumentId, DockspaceRuntimeErrorKind,
        DockspaceSession, NativeHostErrorKind, SurfaceWork, SurfaceWorkDeferred,
    },
};
use open_gpui::{
    Context, Entity, FocusHandle, IntoElement, Modifiers, PlatformWindowPresentOutcome, Render,
    RequestFrameOptions, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size,
};

use crate::adapter::{
    GpuiDockspaceConfig,
    native::NativeBridgeError,
    persistence::{DockHostDocument, DockSurfaceKeyBinding},
    presentation::PendingSubmission,
};

use super::{DockHostAdmissionError, DockPane, DockPaneError, DockSurface};

const FIRST: SurfaceId = SurfaceId::new(1);
const SECOND: SurfaceId = SurfaceId::new(2);
const DOCUMENT: DockspaceDocumentId = DockspaceDocumentId::from_bytes([0x84; 16]);
const KEYS: [&str; 4] = ["pane:a", "pane:b", "pane:c", "pane:closed"];

struct Content {
    item: ItemId,
    focus: FocusHandle,
    clicks: usize,
}

impl Render for Content {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let item = self.item;
        div()
            .size_full()
            .track_focus(&self.focus)
            .debug_selector(move || format!("pane-content:{}", item.get()))
            .child(
                div()
                    .id("edit")
                    .debug_selector(move || format!("pane-button:{}", item.get()))
                    .w(px(100.0))
                    .h(px(32.0))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.clicks += 1;
                        cx.notify();
                    }))
                    .child(format!("{}:{}", item.get(), self.clicks)),
            )
    }
}

struct Root {
    dock: DockSurface,
    surface: SurfaceId,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dock.element(self.surface))
    }
}

struct Fixture {
    dock: DockSurface,
    views: [Entity<Content>; 4],
    items: [ItemId; 4],
}

fn fixture(cx: &mut TestAppContext, managed: bool) -> Fixture {
    cx.update(|cx| {
        let mut bootstrap = DockspaceDocumentBootstrap::new(DOCUMENT);
        bootstrap.ensure_items(KEYS).unwrap();
        let items = KEYS.map(|key| bootstrap.item_id(key).unwrap());
        let split = DockspaceNode::split(
            DockspaceAxis::Horizontal,
            items[..2]
                .iter()
                .map(|item| (DockspaceNode::tabs([*item]), 1.0)),
        )
        .unwrap();
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(FIRST, DockspaceRootLayout::new(RootId::new(1), split)),
            DockspaceSurfaceLayout::new(
                SECOND,
                DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([items[2]])),
            ),
        ])
        .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(managed);
        let session = DockspaceSession::from_persistent_layout(layout, policy, bootstrap).unwrap();
        let views = items.map(|item| {
            cx.new(|cx| Content {
                item,
                focus: cx.focus_handle(),
                clicks: 0,
            })
        });
        let panes = items
            .into_iter()
            .zip(&views)
            .map(|(item, view)| {
                DockPane::new(item, "Pane", view.clone())
                    .with_focus_handle(view.read(cx).focus.clone())
            })
            .collect::<Vec<_>>();
        let dock = if managed {
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx)
        } else {
            DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx)
        }
        .unwrap();
        Fixture { dock, views, items }
    })
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

fn assert_pane_error<T>(result: anyhow::Result<T>, expected: DockPaneError) {
    match result {
        Ok(_) => panic!("expected {expected:?}"),
        Err(error) => assert_eq!(error.downcast_ref::<DockPaneError>(), Some(&expected)),
    }
}

fn close(dock: &DockSurface, item: ItemId, cx: &mut open_gpui::App) {
    dock.update(
        |frame| {
            frame.request_close_item_current(item)?;
            Ok(())
        },
        cx,
    )
    .unwrap();
    let plans = dock.pending_content_close_plans().unwrap();
    assert_eq!(plans.len(), 1);
    dock.update(
        |frame| {
            let decisions = frame
                .pending_close_decisions(plans[0].request())
                .collect::<Vec<_>>();
            for decision in decisions {
                frame.resolve_close(plans[0].request(), decision.token(), CloseDecision::Allow)?;
            }
            Ok(())
        },
        cx,
    )
    .unwrap();
    assert!(dock.with_view(|view| view.item(item).is_none()));
}

fn assert_unmeasured(dock: &DockSurface, surface: SurfaceId) {
    let mut host = dock.host.borrow_mut();
    let mut frame = host.session.begin_host_frame().unwrap();
    assert!(matches!(
        frame.surface_work(surface).unwrap(),
        SurfaceWork::Deferred(SurfaceWorkDeferred::Unmeasured)
    ));
}

#[open_gpui::test]
fn live_restore_rejects_an_open_pane_without_a_registered_view(cx: &mut TestAppContext) {
    let Fixture { dock, items, views } = fixture(cx, false);
    let missing = items[1];
    cx.update(|cx| {
        let saved = dock
            .capture_document(DockHostDocument {
                surfaces: [FIRST, SECOND]
                    .map(|surface| DockSurfaceKeyBinding {
                        space_key: format!("surface-{}", surface.get()),
                        surface,
                        logical_placement: None,
                        central_policy: None,
                    })
                    .into(),
                migration: None,
            })
            .unwrap();
        close(&dock, missing, cx);
        let pane = dock.take_closed_pane(missing).unwrap();
        let prepared = dock
            .host
            .borrow()
            .session
            .prepare_document_restore_json(saved.document_json().as_bytes(), |document, key| {
                document == DOCUMENT && KEYS.contains(&key)
            })
            .unwrap();
        let before = dock.inspection().unwrap();
        let version = dock.host.borrow().session.version();
        assert_pane_error(
            dock.update(|frame| Ok(frame.submit_document_restore(&prepared)?), cx),
            DockPaneError::UnknownPane(missing),
        );
        assert_eq!(dock.inspection().unwrap(), before);
        assert_eq!(dock.host.borrow().session.version(), version);
        assert!(dock.with_view(|view| view.item(missing).is_none()));
        dock.open_pane(pane, DockPlacement::Center(DockAnchor::Item(items[0])), cx)
            .unwrap();
        assert_eq!(
            dock.host.borrow().panels[&missing].view.entity_id(),
            views[1].entity_id()
        );
    });
}

#[open_gpui::test]
fn metadata_remeasures_titles_and_minima_without_replacing_content(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let item = items[0];
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let tab = format!("dockspace:tab:{}", item.get());
    let content = format!("pane-content:{}", item.get());
    let old_tab = visual.debug_bounds(&tab).unwrap();
    let old_content = visual.debug_bounds(&content).unwrap();
    let minimum = dock.host.borrow().panels[&item].sizing.minimum_size();
    let title = "A much longer pane title";
    let version = dock.host.borrow().session.version();
    let generation = dock.host.borrow().session.next_document_generation();
    visual.update(|_, cx| {
        dock.update_pane_metadata(item, title, minimum, cx).unwrap();
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_id()];
        assert!(mount.regions[&FIRST].dirty);
        assert!(crate::adapter::input::is_presented(
            &host,
            window.window_id(),
            FIRST,
            mount.serial
        ));
    });
    frames(&mut visual, &dock);
    assert!(visual.debug_bounds(&tab).unwrap().size.width > old_tab.size.width);

    let minimum = LogicalSize::new(600.0, 180.0).unwrap();
    visual.update(|_, cx| {
        dock.update_pane_metadata(item, title, minimum, cx).unwrap();
        assert_unmeasured(&dock, FIRST);
    });
    frames(&mut visual, &dock);
    let resized = visual.debug_bounds(&content).unwrap();
    assert!(resized.size.width > old_content.size.width);
    assert!(resized.size.width >= px(600.0));
    let button = visual
        .debug_bounds(&format!("pane-button:{}", item.get()))
        .unwrap();
    visual.simulate_click(button.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 1);
    let host = dock.host.borrow();
    let pane = &host.panels[&item];
    assert_eq!(pane.view.entity_id(), views[0].entity_id());
    assert_eq!(
        pane.content_focus,
        Some(views[0].read_with(&visual, |view, _| view.focus.clone()))
    );
    assert_eq!(pane.title.as_ref(), title);
    assert_eq!(pane.sizing.minimum_size(), minimum);
    assert_eq!(host.session.version(), version);
    assert_eq!(host.session.next_document_generation(), generation);
}

#[open_gpui::test]
fn complete_sizing_tracks_the_selected_tab_and_refreshes_inactive_metadata(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let fixed = |width, height| {
        PaneSizing::builder()
            .fixed_width(width)
            .fixed_height(height)
            .build()
            .unwrap()
    };
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(
                    items[1],
                    DockPlacement::Center(DockAnchor::Item(items[0])),
                )?;
                frame.select_item_current(items[0])?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        dock.update_pane_metadata(items[0], "First", fixed(240.0, 160.0), cx)
            .unwrap();
        dock.update_pane_metadata(items[1], "Second", fixed(360.0, 220.0), cx)
            .unwrap();
    });
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, _| {
        let _ = window.activate_window();
    });
    frames(&mut visual, &dock);
    let first = format!("pane-content:{}", items[0].get());
    let second = format!("pane-content:{}", items[1].get());
    let before = visual.debug_bounds(&first).unwrap();
    assert_eq!(before.size, size(px(240.0), px(160.0)));
    assert!(visual.debug_bounds(&second).is_none());

    // An inactive visible tab must be measured too. Its hard limits must not
    // constrain the selected content, but must apply on its first selection.
    let sizing = fixed(80.0, 60.0);
    visual.update(|_, cx| {
        dock.update_pane_metadata(items[1], "Second", sizing, cx)
            .unwrap();
    });
    frames(&mut visual, &dock);
    assert_eq!(visual.debug_bounds(&first).unwrap(), before);
    let tab = visual
        .debug_bounds(&format!("dockspace:tab:{}", items[1].get()))
        .unwrap();
    visual.simulate_click(tab.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(
        visual.debug_bounds(&second).unwrap().size,
        size(px(80.0), px(60.0))
    );
    assert!(visual.debug_bounds(&first).is_none());
    let host = dock.host.borrow();
    assert_eq!(host.panels[&items[1]].sizing(), sizing);
    assert_eq!(
        host.panels[&items[1]].view.entity_id(),
        views[1].entity_id()
    );
}

#[open_gpui::test]
fn preferred_sizing_preserves_user_weights_and_maxima_cap_the_content(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let content = format!("pane-content:{}", items[0].get());
    let before = visual.debug_bounds(&content).unwrap();
    let weights = || {
        dock.with_view(|view| {
            view.surface(FIRST)
                .unwrap()
                .main_root()
                .unwrap()
                .content()
                .unwrap()
                .split()
                .unwrap()
                .weights()
                .collect::<Vec<_>>()
        })
    };
    let before_weights = weights();
    let preferred = PaneSizing::builder()
        .min_width(120.0)
        .min_height(90.0)
        .preferred_width(250.0)
        .build()
        .unwrap();
    visual.update(|_, cx| {
        dock.update_pane_metadata(items[0], "Pane", preferred, cx)
            .unwrap();
    });
    frames(&mut visual, &dock);
    assert_eq!(visual.debug_bounds(&content).unwrap(), before);
    assert_eq!(weights(), before_weights);

    let capped = PaneSizing::builder()
        .min_width(120.0)
        .max_width(280.0)
        .max_height(190.0)
        .preferred_width(250.0)
        .build()
        .unwrap();
    visual.update(|_, cx| {
        dock.update_pane_metadata(items[0], "Pane", capped, cx)
            .unwrap();
    });
    frames(&mut visual, &dock);
    assert_eq!(
        visual.debug_bounds(&content).unwrap().size,
        size(px(280.0), px(190.0))
    );
    assert_eq!(weights(), before_weights);
    assert_eq!(
        dock.host.borrow().panels[&items[0]].view.entity_id(),
        views[0].entity_id()
    );
}

#[open_gpui::test]
fn unchanged_and_closed_metadata_preserve_measurements_and_document_history(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let before = dock.inspection().unwrap();
    let generation = dock.host.borrow().session.next_document_generation();
    let minimum = dock.host.borrow().panels[&items[0]].sizing.minimum_size();
    visual.update(|_, cx| {
        dock.update_pane_metadata(items[0], "Pane", minimum, cx)
            .unwrap();
        dock.update_pane_metadata(
            items[3],
            "Closed metadata",
            LogicalSize::new(200.0, 100.0).unwrap(),
            cx,
        )
        .unwrap();
    });
    assert_eq!(dock.inspection().unwrap(), before);
    assert_eq!(
        dock.host.borrow().session.next_document_generation(),
        generation
    );
    let pane = dock.take_closed_pane(items[3]).unwrap();
    assert_eq!(pane.title.as_ref(), "Closed metadata");
    assert_eq!(pane.view.entity_id(), views[3].entity_id());
    assert_eq!(
        pane.sizing.minimum_size(),
        LogicalSize::new(200.0, 100.0).unwrap()
    );
    assert_eq!(dock.item_id_for_external_key(KEYS[3]), Some(items[3]));
    assert_pane_error(
        dock.take_closed_pane(items[3]),
        DockPaneError::UnknownPane(items[3]),
    );
    visual.update(|_, cx| {
        dock.open_pane(pane, DockPlacement::Center(DockAnchor::Item(items[0])), cx)
            .unwrap();
    });
    frames(&mut visual, &dock);
    assert!(
        visual
            .debug_bounds(&format!("pane-content:{}", items[3].get()))
            .is_some()
    );
}

#[open_gpui::test]
fn unknown_and_reentrant_lifecycle_calls_leave_the_host_unchanged(cx: &mut TestAppContext) {
    let Fixture { dock, items, .. } = fixture(cx, false);
    let unknown = ItemId::new(9999);
    let before = dock.inspection().unwrap();
    let generation = dock.host.borrow().session.next_document_generation();
    cx.update(|cx| {
        assert_pane_error(
            dock.update_pane_metadata(unknown, "Unknown", LogicalSize::new(1.0, 1.0).unwrap(), cx),
            DockPaneError::UnknownPane(unknown),
        );
        assert_pane_error(
            dock.take_closed_pane(unknown),
            DockPaneError::UnknownPane(unknown),
        );
        assert_pane_error(
            dock.take_closed_pane(items[0]),
            DockPaneError::PaneOpen(items[0]),
        );
        dock.with_view(|_| {
            assert_pane_error(
                dock.update_pane_metadata(
                    items[0],
                    "Reentrant",
                    LogicalSize::new(1.0, 1.0).unwrap(),
                    cx,
                ),
                DockPaneError::Busy,
            );
            assert_pane_error(dock.take_closed_pane(items[3]), DockPaneError::Busy);
        });
    });
    assert_eq!(dock.inspection().unwrap(), before);
    assert_eq!(
        dock.host.borrow().session.next_document_generation(),
        generation
    );
    assert_eq!(dock.host.borrow().panels[&items[0]].title.as_ref(), "Pane");
    assert_eq!(dock.item_id_for_external_key(KEYS[3]), Some(items[3]));
}

#[open_gpui::test]
fn original_candidate_lease_blocks_metadata_and_closed_registration_release(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, items, .. } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    frames(
        &mut VisualTestContext::from_window(window.into(), cx),
        &dock,
    );
    cx.update_window(window.into(), |_, window, cx| {
        let before = dock.inspection().unwrap();
        let version = dock.host.borrow().session.version();
        let generation = dock.host.borrow().session.next_document_generation();
        let minimum = dock.host.borrow().panels[&items[0]].sizing.minimum_size();
        {
            let mut host = dock.host.borrow_mut();
            let mut frame = host.session.begin_host_frame().unwrap();
            frame
                .dock_item_current(items[1], DockPlacement::Center(DockAnchor::Item(items[0])))
                .unwrap();
            let update = frame.suspend();
            let ticket = window.begin_exact_frame_submission(cx).unwrap();
            let mount = host
                .mounts
                .get_mut(&window.window_handle().window_id())
                .unwrap();
            assert!(mount.pending.is_none());
            mount.pending = Some(PendingSubmission::new(
                mount.serial,
                ticket,
                Default::default(),
                update,
                Vec::new(),
            ));
        }
        assert_pane_error(
            dock.update_pane_metadata(items[0], "Blocked", minimum, cx),
            DockPaneError::Busy,
        );
        assert_pane_error(
            dock.update_pane_metadata(items[0], "Pane", minimum, cx),
            DockPaneError::Busy,
        );
        assert_pane_error(dock.take_closed_pane(items[3]), DockPaneError::Busy);
        {
            let mut host = dock.host.borrow_mut();
            assert_eq!(host.session.version(), version);
            assert_eq!(host.session.next_document_generation(), generation);
            assert_eq!(host.panels[&items[0]].title.as_ref(), "Pane");
            assert_eq!(host.panels.len(), 4);
            let pending = host
                .mounts
                .get_mut(&window.window_handle().window_id())
                .unwrap()
                .pending
                .take()
                .unwrap();
            window.retire_exact_frame_submission(&pending.ticket);
            drop(pending);
        }
        assert_eq!(dock.inspection().unwrap(), before);
        dock.update_pane_metadata(items[0], "Exact retry", minimum, cx)
            .unwrap();
    })
    .unwrap();
}

#[open_gpui::test]
fn metadata_can_invalidate_an_accepted_deferred_output_without_replacing_its_owner(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, items, .. } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    visual.update(|_, cx| {
        dock.update_pane_metadata(
            items[0],
            "First title",
            LogicalSize::new(120.0, 90.0).unwrap(),
            cx,
        )
        .unwrap();
    });
    frames(&mut visual, &dock);
    let ticket = {
        let host = dock.host.borrow();
        let pending = host.mounts[&window.window_id()].pending.as_ref().unwrap();
        assert!(pending.update.is_none());
        pending.ticket.clone()
    };
    visual.update(|_, cx| {
        dock.update_pane_metadata(
            items[0],
            "Latest title",
            LogicalSize::new(500.0, 140.0).unwrap(),
            cx,
        )
        .unwrap();
    });
    assert_unmeasured(&dock, FIRST);
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_id()];
        assert_eq!(
            mount.pending.as_ref().unwrap().ticket.snapshot(),
            ticket.snapshot()
        );
        assert!(mount.regions[&FIRST].dirty);
    }
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    assert!(!dock.host.borrow().mounts[&window.window_id()].regions[&FIRST].dirty);
    assert_eq!(
        dock.host.borrow().panels[&items[0]].title.as_ref(),
        "Latest title"
    );
    assert!(
        visual
            .debug_bounds(&format!("pane-content:{}", items[0].get()))
            .unwrap()
            .size
            .width
            >= px(500.0)
    );
}

#[open_gpui::test]
fn closed_view_waits_for_packet_replacement_and_renderer_terminal_before_take(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    visual.update(|_, cx| {
        close(&dock, items[0], cx);
        assert!(
            dock.host.borrow().mounts[&window.window_id()].regions[&FIRST]
                .packet
                .as_ref()
                .unwrap()
                .pane_items()
                .contains(&items[0])
        );
        assert_pane_error(dock.take_closed_pane(items[0]), DockPaneError::Busy);
    });
    let version = dock.host.borrow().session.version();
    let generation = dock.host.borrow().session.next_document_generation();
    assert_pane_error(dock.take_closed_pane(items[0]), DockPaneError::Busy);
    assert_eq!(
        dock.host.borrow().panels[&items[0]].view.entity_id(),
        views[0].entity_id()
    );

    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    frames(&mut visual, &dock);
    {
        let host = dock.host.borrow();
        let mount = &host.mounts[&window.window_id()];
        assert!(
            !mount.regions[&FIRST]
                .packet
                .as_ref()
                .unwrap()
                .pane_items()
                .contains(&items[0])
        );
        assert!(mount.pending.as_ref().unwrap().update.is_none());
    }
    assert_pane_error(dock.take_closed_pane(items[0]), DockPaneError::Busy);
    assert_eq!(dock.host.borrow().session.version(), version);
    assert_eq!(
        dock.host.borrow().session.next_document_generation(),
        generation
    );
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    let pane = dock.take_closed_pane(items[0]).unwrap();
    assert_eq!(pane.view.entity_id(), views[0].entity_id());
    assert_eq!(
        pane.content_focus,
        Some(views[0].read_with(&visual, |view, _| view.focus.clone()))
    );
    assert!(!dock.host.borrow().panels.contains_key(&items[0]));
    assert_eq!(dock.item_id_for_external_key(KEYS[0]), Some(items[0]));
    assert_eq!(dock.host.borrow().session.version(), version);
    assert_eq!(
        dock.host.borrow().session.next_document_generation(),
        generation
    );
    assert!(
        visual
            .debug_bounds(&format!("pane-content:{}", items[0].get()))
            .is_none()
    );

    let document = dock
        .capture_document(DockHostDocument {
            surfaces: [FIRST, SECOND]
                .map(|surface| DockSurfaceKeyBinding {
                    space_key: format!("surface:{}", surface.get()),
                    surface,
                    logical_placement: None,
                    central_policy: None,
                })
                .into(),
            migration: None,
        })
        .unwrap();
    let restored = document
        .restore(DockPolicy::default(), |document, key| {
            document == DOCUMENT && KEYS.contains(&key)
        })
        .unwrap();
    assert_eq!(
        restored.session.item_id_for_external_key(KEYS[0]),
        Some(items[0])
    );
    assert!(restored.session.view().item(items[0]).is_none());
    visual.update(|_, cx| {
        dock.open_pane(pane, DockPlacement::Center(DockAnchor::Item(items[1])), cx)
            .unwrap();
    });
    frames(&mut visual, &dock);
    let button = visual
        .debug_bounds(&format!("pane-button:{}", items[0].get()))
        .unwrap();
    visual.simulate_click(button.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 1);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 0);
}

#[open_gpui::test]
fn every_mounted_packet_must_be_replaced_before_closed_registration_release(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, items, .. } = fixture(cx, false);
    let windows = [FIRST, SECOND].map(|surface| {
        cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
            dock: dock.clone(),
            surface,
        })
    });
    for window in windows {
        frames(
            &mut VisualTestContext::from_window(window.into(), cx),
            &dock,
        );
    }
    cx.set_window_present_outcome(windows[1].into(), PlatformWindowPresentOutcome::Deferred);
    cx.update(|cx| close(&dock, items[0], cx));
    frames(
        &mut VisualTestContext::from_window(windows[0].into(), cx),
        &dock,
    );
    assert!(
        dock.host.borrow().mounts[&windows[1].window_id()]
            .pending
            .is_some()
    );
    assert_pane_error(dock.take_closed_pane(items[0]), DockPaneError::Busy);
    cx.set_window_present_outcome(windows[1].into(), PlatformWindowPresentOutcome::Submitted);
    frames(
        &mut VisualTestContext::from_window(windows[1].into(), cx),
        &dock,
    );
    assert_eq!(dock.take_closed_pane(items[0]).unwrap().id(), items[0]);
}

#[open_gpui::test]
fn a_taken_closed_view_releases_after_its_original_packets_retire(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let content = views[0].downgrade();
    drop(views);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    visual.update(|_, cx| close(&dock, items[0], cx));
    frames(&mut visual, &dock);
    assert_pane_error(dock.take_closed_pane(items[0]), DockPaneError::Busy);
    assert!(content.upgrade().is_some());
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    let pane = dock.take_closed_pane(items[0]).unwrap();
    assert!(
        content.upgrade().is_some(),
        "the returned pane retains its original View"
    );
    visual.update(|_, _| drop(pane));
    visual.run_until_parked();
    assert!(
        content.upgrade().is_none(),
        "retired packets must not pin the released View"
    );
    assert_eq!(dock.item_id_for_external_key(KEYS[0]), Some(items[0]));
}

#[open_gpui::test]
fn queued_pointer_input_retains_registration_until_its_original_frame_commits(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, items, .. } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let tab = visual
        .debug_bounds(&format!("dockspace:tab:{}", items[0].get()))
        .unwrap();
    visual.simulate_mouse_move(tab.center(), None, Modifiers::none());
    visual.update(|window, cx| {
        // Retain input from the original submitted frame before releasing the
        // App borrow, which can immediately dispatch the next ordinary frame.
        let frame =
            dock.host.borrow().mounts[&window.window_handle().window_id()].input_frames[0].clone();
        assert!(crate::adapter::input::queue(
            &mut dock.host.borrow_mut(),
            FIRST,
            frame.serial,
            dockspace::runtime::SurfacePointerEvent::Moved,
            tab.center(),
            frame.geometry,
            &frame.hitboxes,
            &frame.content_hitboxes,
            false,
            Modifiers::none(),
            window,
        ));
        let queued = dock.host.borrow().inputs.len();
        assert!(queued > 0);
        assert_pane_error(dock.take_closed_pane(items[3]), DockPaneError::Busy);
        assert_pane_error(
            dock.update_pane_metadata(
                items[0],
                "Blocked input",
                LogicalSize::new(120.0, 90.0).unwrap(),
                cx,
            ),
            DockPaneError::Busy,
        );
        assert_eq!(dock.host.borrow().inputs.len(), queued);
        window.refresh();
    });
    frames(&mut visual, &dock);
    assert!(dock.host.borrow().inputs.is_empty());
    assert_eq!(dock.take_closed_pane(items[3]).unwrap().id(), items[3]);
}

#[open_gpui::test]
fn closing_the_last_pane_replaces_the_old_content_with_an_empty_slot_before_take(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: SECOND,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    visual.update(|_, cx| close(&dock, items[2], cx));
    assert_pane_error(dock.take_closed_pane(items[2]), DockPaneError::Busy);
    frames(&mut visual, &dock);
    assert!(
        dock.host.borrow().mounts[&window.window_id()].regions[&SECOND]
            .packet
            .as_ref()
            .unwrap()
            .pane_items()
            .is_empty()
    );
    assert!(
        visual
            .debug_bounds(&format!("pane-content:{}", items[2].get()))
            .is_none()
    );
    assert_eq!(
        dock.take_closed_pane(items[2]).unwrap().view.entity_id(),
        views[2].entity_id()
    );
}

#[open_gpui::test]
fn mount_retirement_releases_pre_close_packets_before_registration_take(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let Fixture { dock, views, items } = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
        surface: FIRST,
    });
    frames(
        &mut VisualTestContext::from_window(window.into(), cx),
        &dock,
    );
    cx.update(|cx| close(&dock, items[0], cx));
    assert_pane_error(dock.take_closed_pane(items[0]), DockPaneError::Busy);
    cx.update_window(window.into(), |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(dock.host.borrow().mounts.is_empty());
    assert_eq!(
        dock.take_closed_pane(items[0]).unwrap().view.entity_id(),
        views[0].entity_id()
    );
}

#[open_gpui::test]
fn shutdown_rejects_metadata_and_registration_changes_atomically(cx: &mut TestAppContext) {
    let Fixture { dock, views, items } = fixture(cx, true);
    let version = dock.host.borrow().session.version();
    let generation = dock.host.borrow().session.next_document_generation();
    cx.update(|cx| {
        dock.begin_native_shutdown(cx).unwrap();
        let error = dock
            .update_pane_metadata(
                items[0],
                "After shutdown",
                LogicalSize::new(1.0, 1.0).unwrap(),
                cx,
            )
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<NativeBridgeError>(),
            Some(NativeBridgeError::Draining)
        ));
        let error = dock
            .take_closed_pane(items[3])
            .err()
            .expect("shutdown must retain the original registration");
        assert!(matches!(
            error.downcast_ref::<NativeBridgeError>(),
            Some(NativeBridgeError::Draining)
        ));
    });
    let host = dock.host.borrow();
    assert_eq!(host.panels.len(), 4);
    assert_eq!(host.panels[&items[0]].title.as_ref(), "Pane");
    assert_eq!(
        host.panels[&items[3]].view.entity_id(),
        views[3].entity_id()
    );
    assert_eq!(host.session.version(), version);
    assert_eq!(host.session.next_document_generation(), generation);
    assert_eq!(
        host.session.item_id_for_external_key(KEYS[3]),
        Some(items[3])
    );
}

#[open_gpui::test]
fn primary_failure_rejects_callbacks_and_registration_without_consuming_the_diagnostic(
    cx: &mut TestAppContext,
) {
    let Fixture { dock, items, .. } = fixture(cx, false);
    let pane = dock.take_closed_pane(items[3]).unwrap();
    dock.host
        .borrow_mut()
        .fail(&NativeBridgeError::EffectReport(
            NativeHostErrorKind::InvalidFacts,
        ));
    let failure = dock.failure().unwrap();
    assert_eq!(
        failure.runtime_kind,
        Some(DockspaceRuntimeErrorKind::HostProtocol)
    );
    let before = dock.inspection().unwrap();
    let version = dock.host.borrow().session.version();
    let generation = dock.host.borrow().session.next_document_generation();
    cx.update(|cx| {
        let mut invoked = false;
        let error = dock
            .update(
                |_| {
                    invoked = true;
                    Ok(())
                },
                cx,
            )
            .unwrap_err();
        assert!(!invoked);
        assert_eq!(
            error
                .downcast_ref::<DockHostAdmissionError>()
                .unwrap()
                .failure,
            failure
        );
        let error = dock
            .open_pane(pane, DockPlacement::Center(DockAnchor::Item(items[0])), cx)
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<DockHostAdmissionError>()
                .unwrap()
                .failure,
            failure
        );
        let error = dock
            .update_pane_metadata(
                items[0],
                "Rejected",
                LogicalSize::new(1.0, 1.0).unwrap(),
                cx,
            )
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<DockHostAdmissionError>()
                .unwrap()
                .failure,
            failure
        );
        let error = dock
            .take_closed_pane(items[0])
            .err()
            .expect("a primary failure must reject new intent first");
        assert_eq!(
            error
                .downcast_ref::<DockHostAdmissionError>()
                .unwrap()
                .failure,
            failure
        );
    });
    let host = dock.host.borrow();
    assert!(!host.panels.contains_key(&items[3]));
    assert!(host.session.view().item(items[3]).is_none());
    assert_eq!(host.panels[&items[0]].title.as_ref(), "Pane");
    assert_eq!(host.session.version(), version);
    assert_eq!(host.session.next_document_generation(), generation);
    assert_eq!(
        host.session.item_id_for_external_key(KEYS[3]),
        Some(items[3])
    );
    drop(host);
    assert_eq!(dock.inspection().unwrap(), before);
    assert_eq!(dock.failure(), Some(failure));
}

#[open_gpui::test]
fn transient_operation_conflict_allows_retry_without_clearing_its_diagnostic(
    cx: &mut TestAppContext,
) {
    let Fixture { dock, views, items } = fixture(cx, false);
    dock.host
        .borrow_mut()
        .fail(&NativeBridgeError::EffectReport(
            NativeHostErrorKind::OperationConflict,
        ));
    let failure = dock.failure().unwrap();
    assert_eq!(
        failure.runtime_kind,
        Some(DockspaceRuntimeErrorKind::OperationConflict)
    );
    cx.update(|cx| {
        let mut invoked = false;
        dock.update(
            |_| {
                invoked = true;
                Ok(())
            },
            cx,
        )
        .unwrap();
        assert!(invoked);
        let pane = dock.take_closed_pane(items[3]).unwrap();
        dock.open_pane(pane, DockPlacement::Center(DockAnchor::Item(items[0])), cx)
            .unwrap();
        dock.update_pane_metadata(
            items[0],
            "Retry admitted",
            LogicalSize::new(200.0, 100.0).unwrap(),
            cx,
        )
        .unwrap();
    });
    assert!(dock.with_view(|view| view.item(items[3]).is_some()));
    assert_eq!(
        dock.host.borrow().panels[&items[3]].view.entity_id(),
        views[3].entity_id()
    );
    assert_eq!(
        dock.host.borrow().panels[&items[0]].title.as_ref(),
        "Retry admitted"
    );
    assert_eq!(dock.failure(), Some(failure));
}

#[open_gpui::test]
fn primary_failure_keeps_original_native_shutdown_admissible(cx: &mut TestAppContext) {
    let Fixture { dock, .. } = fixture(cx, true);
    dock.host
        .borrow_mut()
        .fail(&NativeBridgeError::EffectReport(
            NativeHostErrorKind::Internal,
        ));
    let failure = dock.failure().unwrap();
    assert_eq!(
        failure.runtime_kind,
        Some(DockspaceRuntimeErrorKind::Internal)
    );
    cx.update(|cx| {
        dock.begin_native_shutdown(cx).unwrap();
        dock.pump_native(cx);
    });
    assert!(
        dock.host
            .borrow()
            .native
            .as_ref()
            .unwrap()
            .transport
            .is_frozen()
    );
    assert_eq!(dock.failure(), Some(failure));
}
