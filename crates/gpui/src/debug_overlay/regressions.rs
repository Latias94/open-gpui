use super::*;
use crate::geometry::ResolvedClip;

#[test]
fn rolling_window_evicts_old_outliers_without_resetting_lifetime_count() {
    let mut overlay = DebugFrameOverlay::new();
    overlay.set_mode(DebugFrameOverlayMode::Full);
    for _ in 0..5 {
        overlay.record_frame(Duration::from_secs(10));
    }
    for milliseconds in 1..=1000 {
        overlay.record_frame(Duration::from_millis(milliseconds));
    }

    assert_eq!(overlay.stats_for_test(), (1000, 1005));
    assert_eq!(
        overlay.draw_durations.front(),
        Some(&Duration::from_millis(1))
    );
    assert_eq!(
        overlay.lines(),
        [
            "CUR 1000.0 MS",
            "1%  990.0 MS",
            "10% 900.0 MS",
            "MAX 1000.0 MS",
            "FRAMES  1005",
        ]
    );

    overlay.reset_stats();
    assert_eq!(overlay.stats_for_test(), (0, 1005));
    overlay.record_frame(Duration::from_millis(7));
    assert_eq!(overlay.stats_for_test(), (1, 1006));
    assert_eq!(overlay.lines()[3], "MAX   7.0 MS");
}

#[test]
fn mode_cycle_keeps_samples_and_switches_complete_readouts() {
    let mut overlay = DebugFrameOverlay::new();
    assert_eq!(overlay.mode(), DebugFrameOverlayMode::Hidden);
    for (expected_mode, expected_lines) in [
        (DebugFrameOverlayMode::Minimal, 1),
        (DebugFrameOverlayMode::Full, 5),
        (DebugFrameOverlayMode::Hidden, 0),
    ] {
        overlay.record_frame(Duration::from_millis(7));
        overlay.set_mode(overlay.mode().next());
        assert_eq!(overlay.mode(), expected_mode);
        assert_eq!(overlay.lines().len(), expected_lines);
        assert_eq!(
            overlay.is_enabled(),
            expected_mode != DebugFrameOverlayMode::Hidden
        );
    }
    assert_eq!(overlay.stats_for_test(), (3, 3));
    overlay.set_mode(overlay.mode().next());
    assert_eq!(overlay.lines(), ["  7.0 MS"]);
}

#[test]
fn hidden_paint_leaves_existing_scene_and_history_untouched() {
    let viewport_size = size(Pixels(800.0), Pixels(600.0));
    let clip_stack =
        ClipStackSnapshot::root(Bounds::new(point(Pixels(0.0), Pixels(0.0)), viewport_size));
    let mut scene = Scene::default();
    scene
        .insert_primitive_scoped(
            solid_quad(scaled_bounds(10.0, 20.0, 30.0, 40.0), text_color()),
            &clip_stack,
            1.0,
            None,
        )
        .unwrap();
    let original_quad = scene.quads[0];
    let original_clips = scene.clip_shapes().to_vec();
    let mut overlay = DebugFrameOverlay::new();
    overlay.record_frame(Duration::from_millis(12));

    overlay.paint(&mut scene, viewport_size, 1.0).unwrap();

    assert_eq!(overlay.mode(), DebugFrameOverlayMode::Hidden);
    assert_eq!(overlay.stats_for_test(), (1, 1));
    assert_eq!(scene.len(), 1);
    assert_eq!(scene.journal_len(), 1);
    assert_eq!(scene.quads.len(), 1);
    assert_eq!(scene.quads[0].bounds, original_quad.bounds);
    assert_eq!(scene.quads[0].clip, original_quad.clip);
    assert_eq!(scene.quads[0].transform, original_quad.transform);
    assert!(scene.quads[0].background == original_quad.background);
    assert_eq!(scene.clip_shapes(), original_clips);
}

#[test]
fn paint_scales_panel_and_cells_once_with_a_physical_pixel_minimum() {
    let mut overlay = DebugFrameOverlay::new();
    overlay.set_mode(DebugFrameOverlayMode::Minimal);
    overlay.record_frame(Duration::from_micros(12_500));
    let viewport_size = size(Pixels(800.25), Pixels(120.5));

    for (scale_factor, cell) in [(0.25, 1.0), (1.0, 2.0), (1.25, 2.5), (2.0, 4.0)] {
        let mut scene = Scene::default();
        overlay
            .paint(&mut scene, viewport_size, scale_factor)
            .unwrap();

        assert!(scene.quads.len() > 1);
        assert_eq!(
            scene.quads[0].bounds,
            scaled_bounds(
                viewport_size.width.0 * scale_factor - cell * 56.0,
                cell * 4.0,
                cell * 52.0,
                cell * 13.0,
            )
        );
        assert_eq!(scene.clip_shapes().len(), 1);
        let viewport = Bounds::new(
            point(ScaledPixels(0.0), ScaledPixels(0.0)),
            viewport_size.scale(scale_factor),
        );
        assert_eq!(scene.clip_shapes()[0].bounds, viewport);
        for quad in &scene.quads {
            assert_eq!(quad.transform, PrimitiveTransform::IDENTITY);
            assert_eq!(quad.clip.conservative_bounds, viewport);
            assert_eq!(quad.clip.first_clip, 0);
            assert_eq!(quad.clip.clip_count, 1);
        }
        for quad in &scene.quads[1..] {
            assert_eq!(quad.bounds.size.height, ScaledPixels(cell));
            assert!(quad.bounds.size.width.0 >= cell);
            assert!(quad.bounds.size.width.0 <= cell * 5.0);
            assert_eq!((quad.bounds.size.width.0 / cell).fract(), 0.0);
        }
        assert_eq!(overlay.stats_for_test(), (1, 1));
    }
}

#[test]
fn oversized_panel_and_glyphs_keep_the_exact_fractional_viewport_clip() {
    let mut overlay = DebugFrameOverlay::new();
    overlay.set_mode(DebugFrameOverlayMode::Full);
    overlay.record_frame(Duration::from_micros(12_500));
    let viewport_size = size(Pixels(80.25), Pixels(35.5));
    let scale_factor = 1.25;
    let viewport = Bounds::new(
        point(ScaledPixels(0.0), ScaledPixels(0.0)),
        viewport_size.scale(scale_factor),
    );
    let mut scene = Scene::default();

    overlay
        .paint(&mut scene, viewport_size, scale_factor)
        .unwrap();

    assert!(scene.quads.len() > 1);
    assert!(scene.quads[0].bounds.left() < viewport.left());
    assert!(scene.quads[0].bounds.bottom() > viewport.bottom());
    assert_eq!(scene.clip_shapes().len(), 1);
    let exact_clip = &scene.clip_shapes()[0];
    assert_eq!(exact_clip.bounds, viewport);
    assert_eq!(exact_clip.radii_x, Corners::default());
    assert_eq!(exact_clip.radii_y, Corners::default());
    for quad in &scene.quads {
        assert_eq!(quad.clip.first_clip, 0);
        assert_eq!(quad.clip.clip_count, 1);
        assert_eq!(quad.clip.conservative_bounds, viewport);
        assert!(!quad.bounds.intersect(&viewport).is_empty());
    }
    assert!(
        scene.quads[1..]
            .iter()
            .any(|quad| quad.bounds.intersect(&viewport) != quad.bounds)
    );
}

#[test]
fn painting_preserves_prior_transformed_primitives_and_subtree_clips() {
    let viewport_size = size(Pixels(800.0), Pixels(600.0));
    let viewport = Bounds::new(point(Pixels(0.0), Pixels(0.0)), viewport_size);
    let clip_stack = ClipStackSnapshot::root(viewport).push(ResolvedClip::rectangle(Bounds::new(
        point(Pixels(50.0), Pixels(50.0)),
        size(Pixels(100.0), Pixels(100.0)),
    )));
    let mut scene = Scene::default();
    scene
        .insert_primitive_scoped(
            Quad {
                transform: PrimitiveTransform::try_new(
                    size(2.0, 3.0),
                    point(ScaledPixels(100.0), ScaledPixels(100.0)),
                )
                .unwrap(),
                ..solid_quad(scaled_bounds(0.0, 0.0, 10.0, 10.0), text_color())
            },
            &clip_stack,
            1.5,
            None,
        )
        .unwrap();
    let original_quad = scene.quads[0];
    let original_clips = scene.clip_shapes().to_vec();
    let mut overlay = DebugFrameOverlay::new();
    overlay.set_mode(DebugFrameOverlayMode::Full);
    overlay.record_frame(Duration::from_millis(12));

    overlay.paint(&mut scene, viewport_size, 1.5).unwrap();
    scene.finish();

    assert_eq!(overlay.mode(), DebugFrameOverlayMode::Full);
    assert_eq!(overlay.stats_for_test(), (1, 1));
    assert!(scene.quads.len() > 2);
    assert_eq!(scene.len(), scene.quads.len());
    assert_eq!(scene.journal_len(), scene.quads.len());
    assert_eq!(scene.quads[0].bounds, original_quad.bounds);
    assert_eq!(scene.quads[0].order, original_quad.order);
    assert_eq!(scene.quads[0].clip, original_quad.clip);
    assert_eq!(scene.quads[0].transform, original_quad.transform);
    assert!(scene.quads[0].background == original_quad.background);
    assert_eq!(&scene.clip_shapes()[..2], original_clips);
    assert_eq!(scene.clip_shapes().len(), 3);
    for quad in &scene.quads[1..] {
        assert_eq!(quad.transform, PrimitiveTransform::IDENTITY);
        assert_eq!(quad.clip.first_clip, 2);
        assert_eq!(quad.clip.clip_count, 1);
        assert_eq!(quad.clip.conservative_bounds, viewport.scale(1.5));
    }
}

#[test]
fn unrepresentable_viewport_propagates_scene_rejection_without_recording() {
    let mut overlay = DebugFrameOverlay::new();
    overlay.set_mode(DebugFrameOverlayMode::Full);
    overlay.record_frame(Duration::from_millis(12));
    let mut scene = Scene::default();

    assert_eq!(
        overlay.paint(&mut scene, size(Pixels(f32::MAX), Pixels(200.0)), 2.0),
        Err(SubtreeGeometryError::DeviceConversion)
    );
    assert_eq!(overlay.stats_for_test(), (1, 1));
    assert_eq!(scene.len(), 0);
    assert_eq!(scene.journal_len(), 0);
    assert!(scene.quads.is_empty());
    assert!(scene.clip_shapes().is_empty());
}
