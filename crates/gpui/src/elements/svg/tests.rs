use super::*;
use crate::{
    MouseButton, Point, SubtreeTransform, SubtreeTransformExt, SubtreeTransformOrigin,
    TestAppContext, point, px, red, size,
};

#[open_gpui::test]
fn raw_data_preserves_transformed_exact_clip(cx: &mut TestAppContext) {
    let transform = SubtreeTransform::try_new(
        size(2.0, 3.0),
        point(px(5.0), px(7.0)),
        SubtreeTransformOrigin::TOP_LEFT,
    )
    .unwrap();
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(20.0), px(10.0)), |_, _| {
        svg()
            .data(
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10"/></svg>"#,
            )
            .path("missing-svg-asset.svg")
            .external_path("missing-external-svg.svg")
            .size_full()
            .text_color(red())
            .overflow_hidden()
            .rounded(px(5.0))
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .with_subtree_transform(transform)
    });

    visual.update(|window, _| {
        let layout_bounds = Bounds::new(Point::default(), size(px(20.0), px(10.0)));
        let displayed_bounds = Bounds::new(point(px(5.0), px(7.0)), size(px(40.0), px(30.0)));
        let scene = &window.rendered_frame.scene;
        assert_eq!(scene.monochrome_sprites.len(), 1);
        let sprite = &scene.monochrome_sprites[0];
        assert_eq!(
            sprite.transform.try_project_bounds(sprite.bounds).unwrap(),
            displayed_bounds.scale(window.scale_factor())
        );
        assert!(sprite.clip.clip_count > 0);
        let clips = &scene.clip_shapes()[sprite.clip.first_clip as usize
            ..(sprite.clip.first_clip + sprite.clip.clip_count) as usize];
        let clip = clips
            .iter()
            .find(|clip| clip.radii_x.top_left.0 > 0.0)
            .expect("the SVG's exact rounded clip should reach the scene");
        assert_eq!(clip.bounds, displayed_bounds.scale(window.scale_factor()));
        assert_eq!(clip.radii_x.top_left, px(10.0).scale(window.scale_factor()));
        assert_eq!(clip.radii_y.top_left, px(15.0).scale(window.scale_factor()));

        let hitbox = window
            .rendered_frame
            .hitboxes
            .iter()
            .find(|hitbox| hitbox.layout_bounds() == layout_bounds)
            .expect("the SVG's hitbox should survive the source adaptation");
        assert_eq!(hitbox.displayed_bounds(), displayed_bounds);
        assert!(hitbox.contains_window_point(displayed_bounds.center()));
        assert!(!hitbox.contains_window_point(point(px(6.0), px(8.0))));
    });
}
