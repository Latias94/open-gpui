use dockspace::{
    model::{
        DockAnchor, DockPlacement, DockspaceAxis, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceDocumentBootstrap, DockspaceDocumentId, DockspaceRuntimeError,
        DockspaceRuntimeErrorKind, DockspaceSession,
    },
};
use open_gpui::{
    Context, Entity, IntoElement, Modifiers, PlatformWindowPresentOutcome, Render,
    RequestFrameOptions, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size,
};

use crate::adapter::{
    GpuiDockspaceConfig,
    persistence::{
        DockHostDocument, DockPersistenceError, DockSurfaceKeyBinding, DockWorkspaceDocument,
        open_pane_bindings,
    },
    presentation::PendingSubmission,
};

use super::{DockPane, DockSurface};

const SURFACE: SurfaceId = SurfaceId::new(1);
const DOCUMENT: DockspaceDocumentId = DockspaceDocumentId::from_bytes([0x83; 16]);
const KEYS: [&str; 4] = ["view:left", "view:right", "view:closed", "view:other"];

struct View {
    selector: &'static str,
    clicks: usize,
}

impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selector = self.selector;
        div().size_full().child(
            div()
                .id("edit")
                .debug_selector(move || selector.to_owned())
                .w(px(160.0))
                .h(px(32.0))
                .on_click(cx.listener(|view, _, _, cx| {
                    view.clicks += 1;
                    cx.notify();
                }))
                .child(format!("{}:{}", self.selector, self.clicks)),
        )
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

struct Fixture {
    dock: DockSurface,
    views: [Entity<View>; 2],
    items: [ItemId; 2],
    hints: DockHostDocument,
}

fn recognize(document: DockspaceDocumentId, key: &str) -> bool {
    document == DOCUMENT && KEYS.contains(&key)
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    cx.update(|cx| {
        let mut bootstrap = DockspaceDocumentBootstrap::new(DOCUMENT);
        bootstrap.ensure_items(KEYS).unwrap();
        let items = [KEYS[0], KEYS[1]].map(|key| bootstrap.item_id(key).unwrap());
        let content = DockspaceNode::split(
            DockspaceAxis::Horizontal,
            items.map(|item| (DockspaceNode::tabs([item]), 1.0)),
        )
        .unwrap();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(1), content),
        )])
        .unwrap();
        let mut session =
            DockspaceSession::from_persistent_layout(layout, DockPolicy::default(), bootstrap)
                .unwrap();
        let hints = DockHostDocument {
            surfaces: vec![DockSurfaceKeyBinding {
                space_key: "main".into(),
                surface: SURFACE,
                logical_placement: None,
                central_policy: None,
            }],
            migration: None,
        };
        let document = DockWorkspaceDocument::capture(&mut session, hints).unwrap();
        let restored = document.restore(DockPolicy::default(), recognize).unwrap();
        let views = ["left-view", "right-view"].map(|selector| {
            cx.new(|_| View {
                selector,
                clicks: 0,
            })
        });
        let panes = open_pane_bindings(&restored.session)
            .unwrap()
            .into_iter()
            .zip(&views)
            .map(|(binding, view)| {
                DockPane::new(binding.item, binding.pane_key.to_owned(), view.clone())
            })
            .collect::<Vec<_>>();
        let dock =
            DockSurface::new(restored.session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        Fixture {
            dock,
            views,
            items,
            hints: restored.host,
        }
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

#[open_gpui::test]
fn restored_closed_pane_opens_its_registered_view_and_receives_content_clicks(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture {
        dock, views, items, ..
    } = fixture(cx);
    let closed = dock.item_id_for_external_key(KEYS[2]).unwrap();
    assert!(dock.with_view(|view| view.item(closed).is_none()));
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    let reopened = cx.update(|cx| {
        let view = cx.new(|_| View {
            selector: "reopened-view",
            clicks: 0,
        });
        dock.open_pane(
            DockPane::new(closed, "Restored View", view.clone()),
            DockPlacement::Center(DockAnchor::Item(items[0])),
            cx,
        )
        .unwrap();
        view
    });
    frames(&mut visual, &dock);
    let content = visual
        .debug_bounds("reopened-view")
        .expect("a reopened historical pane mounts its actual View");
    visual.simulate_click(content.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(reopened.read_with(&visual, |view, _| view.clicks), 1);
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 0);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 0);
    assert!(dock.with_view(|view| view.item(closed).unwrap().is_selected()));
    assert_eq!(dock.item_id_for_external_key(KEYS[2]), Some(closed));
    assert_eq!(
        dock.host.borrow().panels[&closed].view.entity_id(),
        reopened.entity_id()
    );
}

#[open_gpui::test]
fn pane_open_failures_preserve_registration_history_and_allow_an_exact_retry(
    cx: &mut TestAppContext,
) {
    let Fixture {
        dock, views, items, ..
    } = fixture(cx);
    let closed = dock.item_id_for_external_key(KEYS[2]).unwrap();
    let other = dock.item_id_for_external_key(KEYS[3]).unwrap();
    cx.update(|cx| {
        let reopened = cx.new(|_| View {
            selector: "reopened-view",
            clicks: 0,
        });
        let version = dock.host.borrow().session.version();
        let generation = dock.host.borrow().session.next_document_generation();
        assert!(
            dock.open_pane(
                DockPane::new(closed, "Duplicate entity", views[0].clone()),
                DockPlacement::Center(DockAnchor::Item(items[0])),
                cx,
            )
            .is_err()
        );
        assert!(
            dock.open_pane(
                DockPane::new(closed, "Rejected placement", reopened.clone()),
                DockPlacement::Center(DockAnchor::Item(ItemId::new(9999))),
                cx,
            )
            .is_err()
        );
        assert!(
            dock.open_pane(
                DockPane::new(ItemId::new(9999), "Foreign identity", reopened.clone()),
                DockPlacement::Center(DockAnchor::Item(items[0])),
                cx,
            )
            .is_err()
        );
        {
            let host = dock.host.borrow();
            assert_eq!(host.panels.len(), 2);
            assert_eq!(host.session.version(), version);
            assert_eq!(host.session.next_document_generation(), generation);
            assert!(host.session.view().item(closed).is_none());
        }
        assert_eq!(dock.item_id_for_external_key(KEYS[2]), Some(closed));
        assert_eq!(dock.item_id_for_external_key("unallocated-view"), None);
        dock.open_pane(
            DockPane::new(closed, "Exact retry", reopened.clone()),
            DockPlacement::Center(DockAnchor::Item(items[0])),
            cx,
        )
        .unwrap();
        let replacement = cx.new(|_| View {
            selector: "replacement-view",
            clicks: 0,
        });
        assert!(
            dock.open_pane(
                DockPane::new(closed, "Cannot replace", replacement),
                DockPlacement::Center(DockAnchor::Item(items[0])),
                cx,
            )
            .is_err()
        );
        assert!(
            dock.open_pane(
                DockPane::new(other, "Cannot share entity", reopened.clone()),
                DockPlacement::Center(DockAnchor::Item(items[0])),
                cx,
            )
            .is_err()
        );
        let host = dock.host.borrow();
        assert_eq!(host.panels.len(), 3);
        assert_eq!(host.panels[&closed].view.entity_id(), reopened.entity_id());
        assert!(host.session.view().item(other).is_none());
        assert_eq!(host.session.item_id_for_external_key(KEYS[3]), Some(other));
    });
}

#[open_gpui::test]
fn capture_and_pane_open_respect_the_original_unaccepted_update(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let Fixture {
        dock, items, hints, ..
    } = fixture(cx);
    let closed = dock.item_id_for_external_key(KEYS[2]).unwrap();
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    frames(
        &mut VisualTestContext::from_window(window.into(), cx),
        &dock,
    );
    cx.update_window(window.into(), |_, window, cx| {
        let reopened = cx.new(|_| View {
            selector: "reopened-view",
            clicks: 0,
        });
        let generation = dock.host.borrow().session.next_document_generation();
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
        assert!(dock.is_update_pending());
        assert!(matches!(
            dock.capture_document(hints.clone()),
            Err(DockPersistenceError::Busy)
        ));
        let error = dock
            .open_pane(
                DockPane::new(closed, "Busy View", reopened.clone()),
                DockPlacement::Center(DockAnchor::Item(items[0])),
                cx,
            )
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<DockspaceRuntimeError>()
                .unwrap()
                .kind(),
            DockspaceRuntimeErrorKind::UpdateInProgress
        );
        {
            let mut host = dock.host.borrow_mut();
            assert_eq!(host.panels.len(), 2);
            assert_eq!(host.session.next_document_generation(), generation);
            assert!(host.session.view().item(closed).is_none());
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
        assert!(!dock.is_update_pending());
        assert!(matches!(
            dock.with_view(|_| dock.capture_document(hints.clone())),
            Err(DockPersistenceError::Busy)
        ));
        assert_eq!(
            dock.host.borrow().session.next_document_generation(),
            generation
        );
        let document = dock.capture_document(hints.clone()).unwrap();
        let restored = document.restore(DockPolicy::default(), recognize).unwrap();
        assert!(
            restored
                .session
                .view()
                .root(RootId::new(1))
                .unwrap()
                .content()
                .unwrap()
                .split()
                .is_some()
        );
        assert!(restored.session.view().item(closed).is_none());
        dock.open_pane(
            DockPane::new(closed, "Retry after withdrawal", reopened),
            DockPlacement::Center(DockAnchor::Item(items[0])),
            cx,
        )
        .unwrap();
    })
    .unwrap();
}

#[open_gpui::test]
fn live_capture_roundtrips_committed_layout_and_closed_keys_while_submission_is_deferred(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let Fixture {
        dock,
        views,
        items,
        hints,
    } = fixture(cx);
    let closed = [KEYS[2], KEYS[3]].map(|key| dock.item_id_for_external_key(key).unwrap());
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    frames(&mut visual, &dock);
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(
                    items[1],
                    DockPlacement::Center(DockAnchor::Item(items[0])),
                )?;
                frame.select_item_current(items[1])?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
    frames(&mut visual, &dock);
    assert!(!dock.is_update_pending());
    let generation = dock
        .host
        .borrow()
        .session
        .next_document_generation()
        .unwrap();
    {
        let host = dock.host.borrow();
        let pending = host.mounts[&window.window_id()].pending.as_ref().unwrap();
        assert!(pending.update.is_none());
    }
    let mut invalid_hints = hints.clone();
    invalid_hints.surfaces[0].surface = SurfaceId::new(9999);
    assert!(matches!(
        dock.capture_document(invalid_hints),
        Err(DockPersistenceError::Invalid { .. })
    ));
    assert_eq!(
        dock.host.borrow().session.next_document_generation(),
        Some(generation)
    );
    let document = dock.capture_document(hints.clone()).unwrap();
    assert_eq!(
        dock.host.borrow().session.next_document_generation(),
        Some(generation + 1)
    );
    let restored = DockWorkspaceDocument::restore_json(
        &document.to_json().unwrap(),
        DockPolicy::default(),
        recognize,
    )
    .unwrap();
    assert_eq!(restored.host, hints);
    assert_eq!(restored.session.document_id(), Some(DOCUMENT));
    assert_eq!(
        restored
            .session
            .view()
            .item(items[0])
            .unwrap()
            .tabs()
            .items(),
        items
    );
    assert!(
        restored
            .session
            .view()
            .item(items[1])
            .unwrap()
            .is_selected()
    );
    assert_eq!(open_pane_bindings(&restored.session).unwrap().len(), 2);
    for (key, item) in [KEYS[2], KEYS[3]].into_iter().zip(closed) {
        assert_eq!(restored.session.item_id_for_external_key(key), Some(item));
        assert!(restored.session.view().item(item).is_none());
    }
    visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    let content = visual.debug_bounds("right-view").unwrap();
    visual.simulate_click(content.center(), Modifiers::default());
    frames(&mut visual, &dock);
    assert_eq!(views[1].read_with(&visual, |view, _| view.clicks), 1);
    assert_eq!(views[0].read_with(&visual, |view, _| view.clicks), 0);
}
