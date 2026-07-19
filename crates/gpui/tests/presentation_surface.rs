use open_gpui::{
    Bounds, ElementGeometry, Hitbox, MeasuredElementSnapshot, Pixels, Point, PrepaintPublicationId,
    SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, TargetedEvent, div,
    measured_element, point, px, size,
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
    .with_subtree_transform(transform);

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
