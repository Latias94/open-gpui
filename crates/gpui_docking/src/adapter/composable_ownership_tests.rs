//! Complete pane ownership across composable layouts, diagnostics and persistence.

use dockspace::{
    geometry::LogicalRect,
    model::{
        DockAnchor, DockPlacement, DockspaceAxis, DockspaceContainedLayout, DockspaceGridShape,
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout,
        FloatingPresentationId, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceDocumentBootstrap, DockspaceDocumentId, DockspaceSession, SurfaceUnavailableReason,
    },
};
use open_gpui::{AppContext as _, Empty, TestAppContext};

use super::{DockPane, DockSurface};
use crate::adapter::{
    GpuiDockspaceConfig,
    persistence::{
        DockHostDocument, DockPaneBinding, DockSurfaceKeyBinding, DockWorkspaceDocument,
        open_pane_bindings,
    },
};

const DOCUMENT: DockspaceDocumentId = DockspaceDocumentId::from_bytes([0x94; 16]);
const FIRST: SurfaceId = SurfaceId::new(10);
const SECOND: SurfaceId = SurfaceId::new(20);
const ROOTLESS: SurfaceId = SurfaceId::new(30);
const KEYS: [&str; 12] = [
    "main:bare",
    "grid:bare",
    "tabs:before",
    "inactive:bare",
    "inactive:nested:hidden",
    "tabs:after",
    "grid:last",
    "floating:bare",
    "floating:before",
    "floating:inactive:hidden",
    "rootless:bare",
    "closed:history",
];
const OPEN_PANE_COUNT: usize = KEYS.len() - 1;

fn workspace() -> (DockspaceSession, [ItemId; KEYS.len()]) {
    let mut bootstrap = DockspaceDocumentBootstrap::new(DOCUMENT);
    bootstrap.ensure_items(KEYS.into_iter().rev()).unwrap();
    let items = KEYS.map(|key| bootstrap.item_id(key).unwrap());
    let nested = DockspaceNode::equal_split(
        DockspaceAxis::Vertical,
        [
            DockspaceNode::pane(items[3]),
            DockspaceNode::composed_tabs([DockspaceNode::tabs([items[4]])]),
        ],
    )
    .unwrap();
    let grid = DockspaceNode::grid(
        DockspaceGridShape::Auto,
        [
            DockspaceNode::pane(items[1]),
            DockspaceNode::composed_tabs([
                DockspaceNode::pane(items[2]),
                nested,
                DockspaceNode::pane(items[5]),
            ]),
            DockspaceNode::pane(items[6]),
        ],
    )
    .unwrap();
    let floating = DockspaceNode::composed_tabs([
        DockspaceNode::pane(items[8]),
        DockspaceNode::grid(DockspaceGridShape::Auto, [DockspaceNode::tabs([items[9]])]).unwrap(),
    ]);
    let rect = LogicalRect::new(10.0, 20.0, 300.0, 200.0).unwrap();
    let layout = DockspaceLayout::new([
        DockspaceSurfaceLayout::rootless(ROOTLESS).with_contained(DockspaceContainedLayout::new(
            FloatingPresentationId::new(20),
            DockspaceRootLayout::new(RootId::new(50), DockspaceNode::pane(items[10])),
            rect,
        )),
        DockspaceSurfaceLayout::new(SECOND, DockspaceRootLayout::new(RootId::new(30), grid))
            .with_contained(DockspaceContainedLayout::new(
                FloatingPresentationId::new(30),
                DockspaceRootLayout::new(RootId::new(20), DockspaceNode::pane(items[7])),
                rect,
            ))
            .with_contained(DockspaceContainedLayout::new(
                FloatingPresentationId::new(10),
                DockspaceRootLayout::new(RootId::new(10), floating),
                rect,
            )),
        DockspaceSurfaceLayout::new(
            FIRST,
            DockspaceRootLayout::new(RootId::new(40), DockspaceNode::pane(items[0])),
        ),
    ])
    .unwrap()
    .with_hidden_items([items[4], items[9], items[10]])
    .unwrap();
    let session =
        DockspaceSession::from_persistent_layout(layout, DockPolicy::default(), bootstrap).unwrap();
    (session, items)
}

#[open_gpui::test]
fn registration_rejects_missing_bare_grid_and_inactive_nested_panes(cx: &mut TestAppContext) {
    cx.update(|cx| {
        for managed in [false, true] {
            for missing in 0..OPEN_PANE_COUNT {
                let (session, items) = workspace();
                let panes = items
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| *index != missing)
                    .map(|(index, item)| DockPane::new(item, KEYS[index], cx.new(|_| Empty)))
                    .collect::<Vec<_>>();
                let result = if managed {
                    DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx)
                } else {
                    DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx)
                };
                let error = result.err().expect("every owned pane needs a registration");
                assert_eq!(
                    error.to_string(),
                    format!("open item {:?} has no GPUI View", items[missing]),
                    "missing key {}, managed {managed}",
                    KEYS[missing],
                );
            }
        }
    });
}

#[open_gpui::test]
fn inspection_counts_owned_composable_panes_and_moves_keep_entities(cx: &mut TestAppContext) {
    let (dock, views, items) = cx.update(|cx| {
        let (session, items) = workspace();
        let views = KEYS.map(|_| cx.new(|_| Empty));
        let panes = items
            .into_iter()
            .zip(&views)
            .zip(KEYS)
            .map(|((item, view), key)| DockPane::new(item, key, view.clone()))
            .collect::<Vec<_>>();
        let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, views, items)
    });
    let original_entities = views.each_ref().map(|view| view.entity_id());
    let registered_entities = || {
        let host = dock.host.borrow();
        items.map(|item| host.panels[&item].view.entity_id())
    };
    let version = dock.host.borrow().session.version();
    let before = dock.inspection().unwrap();
    assert_eq!(before, dock.inspection().unwrap());
    assert_eq!(dock.host.borrow().session.version(), version);
    assert_eq!(before.model_surface_count, 3);
    assert_eq!(before.open_pane_count, OPEN_PANE_COUNT);
    assert_eq!(before.registered_pane_count, KEYS.len());
    assert_eq!(
        before
            .surfaces
            .iter()
            .map(|surface| (
                surface.surface,
                surface.open_pane_count,
                surface.contained_count
            ))
            .collect::<Vec<_>>(),
        [(FIRST, 1, 0), (SECOND, 9, 2), (ROOTLESS, 1, 1)],
    );
    assert_eq!(registered_entities(), original_entities);
    dock.with_view(|view| {
        assert!(!view.item(items[3]).unwrap().is_active());
        for index in [4, 9, 10] {
            assert!(view.item(items[index]).unwrap().is_hidden());
        }
        assert!(view.item(items[OPEN_PANE_COUNT]).is_none());
    });

    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(
                    items[3],
                    DockPlacement::Center(DockAnchor::Item(items[0])),
                )?;
                frame.complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    let after = dock.inspection().unwrap();
    assert_eq!(after.open_pane_count, OPEN_PANE_COUNT);
    assert_eq!(after.registered_pane_count, KEYS.len());
    assert_eq!(after.surfaces[0].open_pane_count, 2);
    assert_eq!(after.surfaces[1].open_pane_count, 8);
    assert_eq!(after.surfaces[2].open_pane_count, 1);
    assert_eq!(registered_entities(), original_entities);
    dock.with_view(|view| assert_eq!(view.item(items[3]).unwrap().surface(), FIRST));
    assert_eq!(before.surfaces[0].open_pane_count, 1);
}

#[test]
fn bindings_preserve_owned_preorder_through_document_roundtrip() {
    let (mut session, items) = workspace();
    let expected = KEYS
        .into_iter()
        .zip(items)
        .take(OPEN_PANE_COUNT)
        .map(|(pane_key, item)| DockPaneBinding { pane_key, item })
        .collect::<Vec<_>>();
    assert_eq!(open_pane_bindings(&session).unwrap(), expected);
    assert_eq!(open_pane_bindings(&session).unwrap(), expected);
    let hints = DockHostDocument {
        surfaces: [FIRST, SECOND, ROOTLESS]
            .into_iter()
            .map(|surface| DockSurfaceKeyBinding {
                space_key: format!("surface:{}", surface.get()),
                surface,
                logical_placement: None,
                central_policy: None,
            })
            .collect(),
        migration: None,
    };
    let document = DockWorkspaceDocument::capture(&mut session, hints.clone()).unwrap();
    let restored = DockWorkspaceDocument::restore_json(
        &document.to_json().unwrap(),
        DockPolicy::default(),
        |document, key| document == DOCUMENT && KEYS.contains(&key),
    )
    .unwrap();
    assert_eq!(restored.host, hints);
    assert_eq!(open_pane_bindings(&restored.session).unwrap(), expected);
    assert!(!restored.session.view().item(items[3]).unwrap().is_active());
    for index in [4, 9, 10] {
        assert!(
            restored
                .session
                .view()
                .item(items[index])
                .unwrap()
                .is_hidden()
        );
    }
    assert_eq!(
        restored
            .session
            .item_id_for_external_key(KEYS[OPEN_PANE_COUNT]),
        Some(items[OPEN_PANE_COUNT]),
    );
    assert!(
        restored
            .session
            .view()
            .item(items[OPEN_PANE_COUNT])
            .is_none(),
    );
}
