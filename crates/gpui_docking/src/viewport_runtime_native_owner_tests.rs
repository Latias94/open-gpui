use crate::{
    DockController, DockGraph, DockSpaceId, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfaceViewportOpenOutcome, DockViewportRuntimeHandle, DockWorkspace,
    viewport_runtime_handle::DockViewportNativeOwnerError,
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Empty, PlatformWindowCreationCapabilities, QuitMode,
    TestAppContext, WindowCreationSupport, WindowInitialPresentationOrder, WindowOptions, px, size,
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

macro_rules! assert_owner_mismatch {
    ($error:expr, expected: $expected:expr, requested: $requested:expr) => {{
        let error = $error
            .downcast_ref::<DockViewportNativeOwnerError>()
            .expect("the owner mismatch should retain its domain error");
        assert_eq!(
            *error,
            DockViewportNativeOwnerError::ManagedOwnerMismatch {
                expected: $expected,
                requested: $requested,
            }
        );
    }};
}

#[open_gpui::test]
fn managed_viewports_use_the_exact_anchor_even_when_opened_from_a_child(cx: &mut TestAppContext) {
    let (surface, anchor, child) = cx.update(|cx| {
        let surface = managed_surface(cx);
        let anchor = open_primary(&surface, cx);
        let child = open_managed_viewport(&surface, "child", cx);
        (surface, anchor, child)
    });

    assert_eq!(transient_owner(child, cx), Some(anchor));

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
    assert_eq!(transient_owner(grandchild, cx), Some(anchor));
}

#[open_gpui::test]
fn managed_viewport_rejects_an_explicit_alien_owner(cx: &mut TestAppContext) {
    let (surface, runtime, anchor, alien, alien_owner) = cx.update(|cx| {
        let surface = managed_surface(cx);
        let anchor = open_primary(&surface, cx);
        let alien: AnyWindowHandle = cx
            .open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .expect("the alien top-level window should open")
            .into();
        let alien_owner = cx
            .transient_window_owner(alien)
            .expect("the committed alien window should produce a typed owner token");
        let runtime = surface.viewport_runtime(cx);
        (surface, runtime, anchor, alien, alien_owner)
    });

    let error = cx
        .update(|cx| {
            runtime.open_viewport_unchecked_policy(
                "alien-rejected",
                WindowOptions {
                    transient_for: Some(alien_owner),
                    ..Default::default()
                },
                cx,
            )
        })
        .expect_err("a managed viewport must reject a non-anchor owner");

    assert_owner_mismatch!(error, expected: anchor, requested: alien);
    let rejected_space = DockSpaceId::from("alien-rejected");
    assert!(!cx.update(|cx| surface.is_viewport_open(&rejected_space, cx)));
}

#[open_gpui::test]
fn stale_g1_anchor_token_cannot_own_a_g2_viewport(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    let (surface, g1_anchor, g1_owner) = cx.update(|cx| {
        let surface = managed_surface(cx);
        let g1_anchor = open_primary(&surface, cx);
        let g1_owner = cx
            .transient_window_owner(g1_anchor)
            .expect("the committed G1 anchor should produce an owner token");
        (surface, g1_anchor, g1_owner)
    });

    assert!(
        !cx.simulate_window_close(g1_anchor),
        "the surface guard should hold G1 until its shutdown converges"
    );
    cx.update(|_| {});
    cx.run_until_parked();

    let (g2_anchor, runtime) = cx.update(|cx| {
        let g2_anchor = open_primary(&surface, cx);
        (g2_anchor, surface.viewport_runtime(cx))
    });
    assert_ne!(g1_anchor, g2_anchor);

    let error = cx
        .update(|cx| {
            runtime.open_viewport_unchecked_policy(
                "stale-g1-owner",
                WindowOptions {
                    transient_for: Some(g1_owner),
                    ..Default::default()
                },
                cx,
            )
        })
        .expect_err("a stale G1 anchor token must not own a G2 viewport");

    assert_owner_mismatch!(error, expected: g2_anchor, requested: g1_anchor);
    let stale_space = DockSpaceId::from("stale-g1-owner");
    assert!(!cx.update(|cx| surface.is_viewport_open(&stale_space, cx)));
}

#[open_gpui::test]
fn managed_viewport_omits_default_owner_when_the_backend_is_unsupported(cx: &mut TestAppContext) {
    cx.set_platform_window_creation_capabilities(PlatformWindowCreationCapabilities {
        focus_on_appearing: WindowCreationSupport::Supported,
        transient_for: WindowCreationSupport::Unsupported,
        initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
    });

    let viewport = cx.update(|cx| {
        let surface = managed_surface(cx);
        let _anchor = open_primary(&surface, cx);
        open_managed_viewport(&surface, "unsupported-owner", cx)
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
