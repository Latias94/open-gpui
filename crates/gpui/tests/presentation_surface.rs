use open_gpui::{
    Bounds, ElementGeometry, Hitbox, MeasuredElementSnapshot, Pixels, Point, PrepaintPublicationId,
    SubtreePresentation, SubtreePresentationExt as _, SubtreeTransform, SubtreeTransformError,
    SubtreeTransformExt as _, SubtreeTransformOrigin, TargetedEvent, Window, div, measured_element,
    point, px, size,
};

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
    let _ = Window::try_element_geometry
        as fn(&Window, Bounds<Pixels>) -> Result<ElementGeometry, SubtreeTransformError>;
}

#[test]
fn svg_raster_poses_compose_with_checked_subtree_presentation() {
    use open_gpui::{TimedTransformation, Transformation, radians, svg};
    use std::time::Duration;

    let _element = svg()
        .with_transformation(Transformation::rotate(radians(0.5)))
        .with_timed_transformation(TimedTransformation::rotation(
            Duration::from_secs(1),
            Transformation::alpha(0.8),
        ))
        .with_subtree_transform(SubtreeTransform::IDENTITY);
}

#[test]
fn retired_presentation_public_surface_does_not_compile() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/presentation/*.rs");
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
