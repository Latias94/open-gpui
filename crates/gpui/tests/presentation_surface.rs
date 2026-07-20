use open_gpui::{
    Bounds, ElementGeometry, Hitbox, MeasuredElementSnapshot, Pixels, Point, PrepaintPublicationId,
    SubtreePresentation, SubtreePresentationExt as _, SubtreeTransform, SubtreeTransformExt as _,
    SubtreeTransformOrigin, TargetedEvent, Window, div, measured_element, point, px, size,
};
use std::{ffi::OsStr, fs, path::Path};

#[test]
fn checked_presentation_import_paths_compile() {
    let _: PrepaintPublicationId = PrepaintPublicationId::new();
    let transform = SubtreeTransform::try_new(
        size(1.25, 0.9),
        point(px(8.0), px(-4.0)),
        SubtreeTransformOrigin::CENTER,
    )
    .unwrap();

    let _element = measured_element("public-presentation-surface", div(), |snapshot, _| {
        let geometry: ElementGeometry = snapshot.geometry();
        let _: Bounds<Pixels> = geometry.layout_bounds();
        let _: Bounds<Pixels> = geometry.displayed_bounds();
        let _: Result<Point<Pixels>, _> = geometry.window_to_local_point(Point::default());
    })
    .with_subtree_transform(transform)
    .with_subtree_presentation(SubtreePresentation::Inert);

    assert!(SubtreePresentation::Visible.paints());
    assert!(SubtreePresentation::Inert.paints());
    assert!(!SubtreePresentation::Hidden.paints());
    assert!(SubtreePresentation::Visible.is_interactive());
    assert!(!SubtreePresentation::Inert.is_interactive());
    let _ = Window::subtree_presentation as fn(&Window) -> SubtreePresentation;

    fn consume_snapshot(snapshot: &MeasuredElementSnapshot) -> (u64, ElementGeometry) {
        (snapshot.frame_generation(), snapshot.geometry())
    }
    fn consume_hitbox(hitbox: &Hitbox) -> ElementGeometry {
        hitbox.geometry()
    }
    fn consume_targeted_event<E>(event: &TargetedEvent<E>) -> Bounds<Pixels> {
        event.target_local_bounds()
    }

    let _ = consume_snapshot as fn(&MeasuredElementSnapshot) -> (u64, ElementGeometry);
    let _ = consume_hitbox as fn(&Hitbox) -> ElementGeometry;
    let _ = consume_targeted_event::<()> as fn(&TargetedEvent<()>) -> Bounds<Pixels>;
}

#[test]
fn legacy_transform_names_do_not_reenter_production_source() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);

    for forbidden in [
        concat!("Trans", "formation"),
        concat!("Trans", "formationMatrix"),
        concat!("with_", "transformation"),
    ] {
        let offenders = sources
            .iter()
            .filter_map(|path| {
                let source = fs::read_to_string(path).ok()?;
                source
                    .contains(forbidden)
                    .then(|| path.display().to_string())
            })
            .collect::<Vec<_>>();
        assert!(
            offenders.is_empty(),
            "legacy transform name `{forbidden}` reappeared in {}",
            offenders.join(", ")
        );
    }
}

#[test]
fn legacy_presentation_authorities_do_not_reenter_production_source() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_roots = [manifest.join("src"), manifest.join("../gpui_macros/src")];
    let mut sources = Vec::new();
    for source_root in source_roots {
        collect_rust_sources(&source_root, &mut sources);
    }

    for forbidden in [
        concat!("Visibility", "::Hidden"),
        concat!("visibility_", "style_methods"),
        concat!(".invis", "ible()"),
        concat!("a11y_", "hidden"),
        concat!("aria_", "hidden"),
        concat!("enter_", "hidden_subtree"),
    ] {
        let offenders = sources
            .iter()
            .filter_map(|path| {
                let source = fs::read_to_string(path).ok()?;
                source
                    .contains(forbidden)
                    .then(|| path.display().to_string())
            })
            .collect::<Vec<_>>();
        assert!(
            offenders.is_empty(),
            "legacy presentation authority `{forbidden}` reappeared in {}",
            offenders.join(", ")
        );
    }

    let public_visibility_offenders = sources
        .iter()
        .filter_map(|path| {
            let source = fs::read_to_string(path).ok()?;
            source_has_public_visibility_surface(&source).then(|| path.display().to_string())
        })
        .collect::<Vec<_>>();
    assert!(
        public_visibility_offenders.is_empty(),
        "legacy public `Visibility` surface reappeared in {}",
        public_visibility_offenders.join(", ")
    );
}

fn source_has_public_visibility_surface(source: &str) -> bool {
    syn::parse_file(source).is_ok_and(|file| items_have_public_visibility_surface(&file.items))
}

fn items_have_public_visibility_surface(items: &[syn::Item]) -> bool {
    items.iter().any(|item| match item {
        syn::Item::Enum(item) => is_public(&item.vis) && item.ident == "Visibility",
        syn::Item::Struct(item) => is_public(&item.vis) && item.ident == "Visibility",
        syn::Item::Type(item) => is_public(&item.vis) && item.ident == "Visibility",
        syn::Item::Use(item) => is_public(&item.vis) && use_tree_exports_visibility(&item.tree),
        syn::Item::Mod(item) => item
            .content
            .as_ref()
            .is_some_and(|(_, items)| items_have_public_visibility_surface(items)),
        _ => false,
    })
}

fn is_public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
}

fn use_tree_exports_visibility(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Name(name) => name.ident == "Visibility",
        syn::UseTree::Rename(rename) => rename.rename == "Visibility",
        syn::UseTree::Path(path) => use_tree_exports_visibility(&path.tree),
        syn::UseTree::Group(group) => group.items.iter().any(use_tree_exports_visibility),
        syn::UseTree::Glob(_) => false,
    }
}

#[test]
fn renderer_transform_abi_stays_opaque_to_applications() {
    let scene = include_str!("../src/scene.rs");
    let declaration = scene
        .split("pub struct PrimitiveTransform")
        .nth(1)
        .and_then(|tail| tail.split('}').next())
        .expect("PrimitiveTransform declaration should remain present for renderer crates");

    for field in ["scale_x", "scale_y", "translation_x", "translation_y"] {
        assert!(declaration.contains(&format!("{field}: f32")));
        assert!(
            !declaration.contains(&format!("pub {field}: f32")),
            "renderer ABI field `{field}` must not be publicly writable"
        );
    }
    assert!(scene.contains("pub(crate) fn try_new("));
    assert!(scene.contains("pub(crate) transform: PrimitiveTransform"));
}

fn collect_rust_sources(directory: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).expect("GPUI source directory should be readable") {
        let path = entry.expect("GPUI source entry should be readable").path();
        if path.is_dir() {
            collect_rust_sources(&path, output);
        } else if path
            .extension()
            .is_some_and(|extension| extension == OsStr::new("rs"))
        {
            output.push(path);
        }
    }
}
