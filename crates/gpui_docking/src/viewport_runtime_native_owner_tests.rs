use crate::{
    DockController, DockGraph, DockSpaceId, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfaceViewportOpenOutcome, DockViewportRuntimeHandle, DockWorkspace,
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Empty, TestAppContext, WindowOptions, px, size,
};

fn managed_surface(cx: &mut App) -> DockSurface {
    DockSurface::builder("main")
        .allow_platform_viewports(true)
        .build(cx)
        .expect("the managed surface should validate")
}

fn open_primary(surface: &DockSurface, cx: &mut App) -> AnyWindowHandle {
    match surface.open_primary_window(WindowOptions::default(), cx) {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
        outcome => panic!("the primary anchor should open, got {outcome:?}"),
    }
}

fn open_managed_viewport(
    surface: &DockSurface,
    space: impl Into<DockSpaceId>,
    cx: &mut App,
) -> AnyWindowHandle {
    match surface.open_viewport(space, WindowOptions::default(), cx) {
        DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
        outcome => panic!("the managed viewport should open, got {outcome:?}"),
    }
}

fn transient_owner(window: AnyWindowHandle, cx: &mut TestAppContext) -> Option<AnyWindowHandle> {
    window
        .update(cx, |_, window, _| window.creation_facts().transient_for)
        .expect("the viewport window should remain live")
}

#[open_gpui::test]
fn managed_viewports_default_to_peer_top_levels_even_when_opened_from_a_child(
    cx: &mut TestAppContext,
) {
    let (surface, anchor, child) = cx.update(|cx| {
        let surface = managed_surface(cx);
        let anchor = open_primary(&surface, cx);
        let child = open_managed_viewport(&surface, "child", cx);
        (surface, anchor, child)
    });

    assert_eq!(transient_owner(child, cx), None);

    let runtime = cx.update(|cx| surface.viewport_runtime(cx));
    let grandchild = child
        .update(cx, |_, child_window, cx| {
            runtime.open_viewport_from_window(
                "grandchild",
                WindowOptions::default(),
                child_window,
                cx,
            )
        })
        .expect("the child should remain live")
        .expect("the grandchild viewport should open")
        .window();

    assert_ne!(child, anchor);
    assert_eq!(transient_owner(grandchild, cx), None);
}

#[open_gpui::test]
fn managed_viewport_preserves_an_explicit_owner(cx: &mut TestAppContext) {
    let (surface, runtime, alien, alien_owner) = cx.update(|cx| {
        let surface = managed_surface(cx);
        let _anchor = open_primary(&surface, cx);
        let alien: AnyWindowHandle = cx
            .open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .expect("the alien top-level window should open")
            .into();
        let alien_owner = cx
            .transient_window_owner(alien)
            .expect("the committed alien window should produce a typed owner token");
        let runtime = surface.viewport_runtime(cx);
        (surface, runtime, alien, alien_owner)
    });

    let viewport = cx
        .update(|cx| {
            runtime.open_viewport_unchecked_policy(
                "explicit-owner",
                WindowOptions {
                    transient_for: Some(alien_owner),
                    ..Default::default()
                },
                cx,
            )
        })
        .expect("a managed viewport should preserve an explicit owner")
        .window();

    assert_eq!(transient_owner(viewport, cx), Some(alien));
    let explicit_space = DockSpaceId::from("explicit-owner");
    assert!(cx.update(|cx| surface.is_viewport_open(&explicit_space, cx)));
}

#[open_gpui::test]
fn managed_viewport_uses_no_owner_by_default(cx: &mut TestAppContext) {
    let viewport = cx.update(|cx| {
        let surface = managed_surface(cx);
        let _anchor = open_primary(&surface, cx);
        open_managed_viewport(&surface, "peer", cx)
    });

    assert_eq!(transient_owner(viewport, cx), None);
}

#[open_gpui::test]
fn unmanaged_runtime_preserves_the_callers_explicit_owner(cx: &mut TestAppContext) {
    let owner: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(240.0)), |_, _| Empty)
        .into();
    let owner_token = cx
        .read(|cx| cx.transient_window_owner(owner))
        .expect("the committed owner should produce a typed token");

    let viewport = cx
        .update(|cx| {
            let mut workspace = DockWorkspace::new(DockSpaceId::from("main"), DockGraph::new());
            workspace.policy_mut().set_allow_platform_viewports(true);
            let controller = cx.new(|_| DockController::new(workspace));
            DockViewportRuntimeHandle::new(controller).open_viewport(
                "caller-owned",
                WindowOptions {
                    transient_for: Some(owner_token),
                    ..Default::default()
                },
                cx,
            )
        })
        .expect("the unmanaged viewport should preserve the caller's owner")
        .window();

    assert_eq!(transient_owner(viewport, cx), Some(owner));
}
