use open_gpui::{
    AnyElement, Bounds, ElementGeometry, IntoElement, Pixels, PortalAnchorError,
    PortalAnchorExt as _, PortalAnchorHandle, PortalAnchorSnapshot, SubtreePresentation, Window,
    div, portal_anchor_follower,
};

#[test]
fn portal_anchor_capability_import_paths_compile() {
    fn bind(
        window: &mut Window,
        handle: &PortalAnchorHandle,
        bounds: Bounds<Pixels>,
    ) -> Result<(), PortalAnchorError> {
        window.bind_portal_anchor(handle, bounds)
    }

    fn resolve(
        window: &mut Window,
        handle: &PortalAnchorHandle,
    ) -> Result<Option<u64>, PortalAnchorError> {
        window.resolve_portal_anchor(handle, |snapshot, _| {
            snapshot.map(PortalAnchorSnapshot::frame_generation)
        })
    }

    fn inspect(snapshot: PortalAnchorSnapshot) -> (ElementGeometry, SubtreePresentation) {
        let _: Bounds<Pixels> = snapshot.effective_clip_bounds();
        let _ = snapshot.window_id();
        (snapshot.geometry(), snapshot.presentation())
    }

    fn target(handle: &PortalAnchorHandle) -> impl IntoElement {
        div().track_portal_anchor(handle)
    }

    fn follower(handle: &PortalAnchorHandle) -> impl IntoElement {
        portal_anchor_follower(handle, |snapshot, _, _| {
            let _: Option<PortalAnchorSnapshot> = snapshot;
            None::<AnyElement>
        })
    }

    let _ = Window::new_portal_anchor as fn(&mut Window) -> PortalAnchorHandle;
    let _ = bind as fn(&mut Window, &PortalAnchorHandle, Bounds<Pixels>) -> Result<(), _>;
    let _ = resolve as fn(&mut Window, &PortalAnchorHandle) -> Result<Option<u64>, _>;
    let _ = inspect as fn(PortalAnchorSnapshot) -> (ElementGeometry, SubtreePresentation);
    let _ = target;
    let _ = follower;
}

#[test]
fn portal_anchor_handle_and_snapshot_fields_remain_opaque() {
    let source = include_str!("../src/window/portal_anchor.rs");
    let file = syn::parse_file(source).expect("portal-anchor source should parse");

    for name in ["PortalAnchorHandle", "PortalAnchorSnapshot"] {
        let item = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Struct(item) if item.ident == name => Some(item),
                _ => None,
            })
            .unwrap_or_else(|| panic!("public `{name}` declaration should remain present"));
        assert!(matches!(item.vis, syn::Visibility::Public(_)));
        assert!(
            item.fields
                .iter()
                .all(|field| !matches!(field.vis, syn::Visibility::Public(_))),
            "`{name}` must not expose mutable geometry or window identity fields"
        );
    }
}

#[test]
fn generic_or_cross_window_portal_surfaces_do_not_appear() {
    let source = include_str!("../src/window/portal_anchor.rs").to_ascii_lowercase();
    for forbidden in [
        "last_known",
        "last-known",
        "portalnode",
        "portal_node",
        "noderef",
        "node_ref",
        "selection",
        "cross_window",
        "cross-window conversion",
        "convert_window",
        "transform_matrix",
        "raw_matrix",
    ] {
        assert!(
            !source.contains(forbidden),
            "portal-anchor surface must not expose `{forbidden}`"
        );
    }
}
