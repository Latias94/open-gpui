use std::{cell::Cell, rc::Rc};

use dockspace::{
    geometry::LogicalRect,
    model::{
        DockspaceContainedLayout, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
        DockspaceSurfaceLayout, FloatingPresentationId, ItemId, RootId, SurfaceId,
    },
    policy::{CloseCapability, DockPolicy},
    runtime::DockspaceSession,
};
use open_gpui::{
    Bounds, Context, IntoElement, Modifiers, Pixels, Render, RequestFrameOptions, Role,
    TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};
use open_gpui_docking::{DockPane, DockSurface, GpuiDockspaceConfig};

const SURFACE: SurfaceId = SurfaceId::new(83);
const CENTER: ItemId = ItemId::new(201);
const FLOATING: ItemId = ItemId::new(509);

struct Document(ItemId);

impl Render for Document {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let id = self.0;
        div()
            .size_full()
            .debug_selector(move || format!("application-pane:{}", id.get()))
    }
}

struct Mounted(DockSurface);

impl Render for Mounted {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_5().child(self.0.element(SURFACE))
    }
}

fn present(visual: &mut VisualTestContext, dock: &DockSurface) {
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
fn retained_application_pane_keeps_its_size_across_a_surface_edge(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let mut reference: Option<Bounds<Pixels>> = None;
    for top in [80.0, -80.0] {
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(37), DockspaceNode::central_tabs([CENTER])),
        )
        .with_contained(DockspaceContainedLayout::new(
            FloatingPresentationId::new(1),
            DockspaceRootLayout::new(RootId::new(38), DockspaceNode::tabs([FLOATING])),
            LogicalRect::new(200.0, top, 300.0, 240.0).unwrap(),
        ))])
        .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_close_capability(CloseCapability::Disabled);
        policy.set_allow_native_surfaces(false);
        let session = DockspaceSession::from_layout(layout, policy).unwrap();
        let dock = cx.update(|app| {
            let panes = [CENTER, FLOATING]
                .map(|id| DockPane::new(id, "Document", app.new(|_| Document(id))));
            DockSurface::new(session, panes, GpuiDockspaceConfig::default(), app).unwrap()
        });
        let handle = cx.open_window(size(px(1100.0), px(760.0)), |_, _| Mounted(dock.clone()));
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        visual.update(|window, _| {
            let _ = window.activate_window();
        });
        present(&mut visual, &dock);
        let actual = visual.debug_bounds("application-pane:509").unwrap();
        if let Some(original) = reference {
            assert_eq!(actual.size, original.size);
            assert_eq!(actual.origin, original.origin - point(px(0.0), px(160.0)));
        } else {
            reference = Some(actual);
        }
    }
}

#[open_gpui::test]
fn tab_content_controls_width_without_taking_input_from_the_tab(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dirty = Rc::new(Cell::new(false));
    let label_clicks = Rc::new(Cell::new(0));
    let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
        SURFACE,
        DockspaceRootLayout::new(RootId::new(37), DockspaceNode::tabs([CENTER, FLOATING])),
    )])
    .unwrap();
    let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
    let dock = cx.update(|app| {
        let unsaved = dirty.clone();
        let clicks = label_clicks.clone();
        let first = DockPane::new(CENTER, "Document", app.new(|_| Document(CENTER)));
        let editor = DockPane::new(FLOATING, "Editor", app.new(|_| Document(FLOATING)))
            .with_tab_content(move |_, _| {
                let clicks = clicks.clone();
                div()
                    .id("editor-tab-content")
                    .flex()
                    .w(px(if unsaved.get() { 132.0 } else { 108.0 }))
                    .child("Editor")
                    .when(unsaved.get(), |label| label.child("•"))
                    .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
            });
        DockSurface::new(
            session,
            [first, editor],
            GpuiDockspaceConfig::default(),
            app,
        )
        .unwrap()
    });
    let handle = cx.open_window(size(px(900.0), px(640.0)), |_, _| Mounted(dock.clone()));
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, _| {
        let _ = window.activate_window();
    });
    assert!(visual.activate_accessibility());
    present(&mut visual, &dock);
    let initial_width = visual.debug_bounds("dockspace:tab:509").unwrap().size.width;
    assert!(initial_width >= px(108.0));

    dirty.set(true);
    visual.update(|_, cx| dock.refresh_tab_content(FLOATING, cx).unwrap());
    present(&mut visual, &dock);
    let label = visual.debug_bounds("dockspace:tab:509").unwrap();
    assert_eq!(label.size.width - initial_width, px(24.0));
    let tree = visual.latest_accessibility_tree_update().unwrap();
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Tab && node.label() == Some("Editor"))
    );

    assert!(dock.with_view(|view| view.item(CENTER).unwrap().is_active()));
    visual.simulate_click(label.center(), Modifiers::none());
    present(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FLOATING).unwrap().is_active()));
    assert_eq!(label_clicks.get(), 0);
}

#[path = "pane_content_geometry/editor_input.rs"]
mod editor_input;
#[path = "pane_content_geometry/tab_labels.rs"]
mod tab_labels;
