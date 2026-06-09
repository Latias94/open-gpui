use crate::{
    DockController, DockPanelViewError, DockWorkspace, debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{AnyView, App, AppContext as _, TestAppContext, VisualTestContext, px, size};
use std::{cell::Cell, rc::Rc};

#[open_gpui::test]
fn registry_replaces_registered_panel(cx: &mut TestAppContext) {
    let mut registry = crate::DockPanelRegistry::new();
    let first = test_view(cx, "first");
    let second = test_view(cx, "second");

    assert!(registry.register_view(item("a"), "First", first).is_none());
    let previous = registry
        .register_view(item("a"), "Second", second)
        .expect("second registration should return previous panel");

    assert_eq!(previous.title(), "First");
    assert_eq!(
        registry
            .get(&item("a"))
            .map(|panel| panel.title().to_string()),
        Some("Second".to_string())
    );
    assert_eq!(registry.len(), 1);
}

#[open_gpui::test]
fn registry_descriptor_lookup_does_not_instantiate_lazy_panel(_cx: &mut TestAppContext) {
    let mut registry = crate::DockPanelRegistry::new();
    let calls = Rc::new(Cell::new(0));
    let factory_calls = calls.clone();
    registry.register_factory("lazy", "Lazy", move |cx| {
        factory_calls.set(factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "lazy" }).into()
    });

    let descriptor = registry
        .descriptor(&item("lazy"))
        .expect("lazy panel metadata should be registered");
    assert_eq!(descriptor.title(), "Lazy");
    assert!(descriptor.is_closable());
    assert_eq!(
        calls.get(),
        0,
        "metadata lookup should not instantiate lazy panel view"
    );
    assert!(
        !registry
            .get(&item("lazy"))
            .expect("lazy panel should remain registered")
            .has_view()
    );
    assert!(matches!(
        registry
            .get(&item("lazy"))
            .expect("lazy panel should remain registered")
            .view(),
        Err(DockPanelViewError::LazyViewNotInstantiated)
    ));
}

#[open_gpui::test]
fn lazy_panel_factory_instantiates_on_first_render_and_reuses_view(cx: &mut TestAppContext) {
    let calls = Rc::new(Cell::new(0));
    let (graph, _root) = tabs_graph(&["lazy"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    let factory_calls = calls.clone();
    workspace.register_panel_factory("lazy", "Lazy", move |cx| {
        factory_calls.set(factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "lazy" }).into()
    });
    let panel = workspace
        .panels()
        .get(&item("lazy"))
        .expect("panel should be registered")
        .clone();

    assert!(!panel.has_view());
    assert_eq!(calls.get(), 0);

    let (window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));
    assert_eq!(calls.get(), 1);
    assert!(panel.has_view());
    assert!(panel.view().is_ok());
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel { item: item("lazy") }
        )
        .is_some(),
        "lazy panel should render after first instantiation"
    );

    let _visual = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    assert_eq!(calls.get(), 1, "lazy panel view should be reused");
}

#[open_gpui::test]
fn panel_factory_accepts_app_context_without_host_context(cx: &mut TestAppContext) {
    fn app_context_panel(cx: &mut App) -> AnyView {
        cx.new(|_| TestPanel { label: "app" }).into()
    }

    let (graph, _root) = tabs_graph(&["app"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_factory("app", "App", app_context_panel);

    let (_window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("app") }).is_some(),
        "app-context panel factory should render through DockHost without depending on its context type"
    );
}

#[open_gpui::test]
fn inactive_lazy_panel_title_does_not_instantiate_view(cx: &mut TestAppContext) {
    let active_calls = Rc::new(Cell::new(0));
    let inactive_calls = Rc::new(Cell::new(0));
    let (graph, _root) = tabs_graph(&["active", "inactive"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    let active_factory_calls = active_calls.clone();
    workspace.register_panel_factory("active", "Active", move |cx| {
        active_factory_calls.set(active_factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "active" }).into()
    });
    let inactive_factory_calls = inactive_calls.clone();
    workspace.register_panel_factory("inactive", "Inactive", move |cx| {
        inactive_factory_calls.set(inactive_factory_calls.get() + 1);
        cx.new(|_| TestPanel { label: "inactive" }).into()
    });

    let (_window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    assert_eq!(active_calls.get(), 1);
    assert_eq!(
        inactive_calls.get(),
        0,
        "inactive tab title lookup should not instantiate panel view"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("active")
            }
        )
        .is_some()
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Panel {
                item: item("inactive")
            }
        )
        .is_none()
    );
}

#[open_gpui::test]
fn lazy_panel_state_stays_out_of_layout_export(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["lazy"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_factory("lazy", "Lazy Panel", |cx| {
        cx.new(|_| TestPanel { label: "lazy" }).into()
    });
    let controller = cx.new(|_| DockController::new(workspace));

    let (_window, _host, _visual) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));
    let json = cx.read_entity(&controller, |controller, _| {
        serde_json::to_string(&controller.graph().export_layout()).expect("layout should serialize")
    });

    assert!(!json.contains("Lazy Panel"));
    assert!(!json.contains("TestPanel"));
    assert!(!json.contains("AnyView"));
}
