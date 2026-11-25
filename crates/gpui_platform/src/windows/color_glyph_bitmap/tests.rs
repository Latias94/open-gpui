use super::{ColorGlyphBitmap, union_glyph_bounds};
use open_gpui::{DevicePixels, Rgba, bounds, point, size};

#[test]
fn source_over_preserves_layer_order_and_returns_straight_bgra() {
    let red = Rgba {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 0.5,
    };
    let blue = Rgba {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 0.5,
    };
    for (layers, expected) in [
        ([red, blue], [170, 0, 85, 191]),
        ([blue, red], [85, 0, 170, 191]),
    ] {
        let mut bitmap = ColorGlyphBitmap::new(size(DevicePixels(2), DevicePixels(2)));
        for color in layers {
            bitmap
                .composite(
                    color,
                    bounds(point(0, 0), size(1, 1)),
                    &[255],
                    [0.0; 4],
                    0.0,
                )
                .unwrap();
        }
        let bytes = bitmap.into_straight_bgra();
        assert_eq!(&bytes[..4], &expected);
        assert_eq!(&bytes[4..], &[0; 12]);
    }
}

#[test]
fn coverage_and_palette_alpha_multiply_without_coloring_transparent_pixels() {
    let mut bitmap = ColorGlyphBitmap::new(size(DevicePixels(3), DevicePixels(1)));
    let color = Rgba {
        r: 0.2,
        g: 0.4,
        b: 0.8,
        a: 0.5,
    };
    bitmap
        .composite(
            color,
            bounds(point(0, 0), size(3, 1)),
            &[0, 128, 255],
            [0.0; 4],
            0.0,
        )
        .unwrap();
    bitmap
        .composite(
            Rgba { a: 0.0, ..color },
            bounds(point(0, 0), size(3, 1)),
            &[255; 3],
            [0.0; 4],
            0.0,
        )
        .unwrap();
    assert_eq!(
        bitmap.into_straight_bgra(),
        [0, 0, 0, 0, 204, 102, 51, 64, 204, 102, 51, 128]
    );
}

#[test]
fn clipped_layers_keep_the_original_coverage_stride_on_all_four_edges() {
    let mut bitmap = ColorGlyphBitmap::new(size(DevicePixels(3), DevicePixels(3)));
    let coverage: Vec<u8> = (1..=25).collect();
    bitmap
        .composite(
            Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            bounds(point(-1, -1), size(5, 5)),
            &coverage,
            [0.0; 4],
            0.0,
        )
        .unwrap();
    let expected: Vec<u8> = [7, 8, 9, 12, 13, 14, 17, 18, 19]
        .into_iter()
        .flat_map(|alpha| [255, 255, 255, alpha])
        .collect();
    assert_eq!(bitmap.into_straight_bgra(), expected);
}

#[test]
fn disjoint_and_empty_layers_leave_the_bitmap_transparent() {
    let mut bitmap = ColorGlyphBitmap::new(size(DevicePixels(3), DevicePixels(3)));
    for layer in [
        bounds(point(-4, 0), size(2, 2)),
        bounds(point(3, 0), size(2, 2)),
        bounds(point(0, -4), size(2, 2)),
        bounds(point(0, 3), size(2, 2)),
        bounds(point(i32::MAX, i32::MAX), size(2, 2)),
        bounds(point(i32::MIN, i32::MIN), size(2, 2)),
        bounds(point(1, 1), size(0, 2)),
        bounds(point(1, 1), size(2, 0)),
    ] {
        bitmap
            .composite(
                Rgba {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                layer,
                &[],
                [0.0; 4],
                0.0,
            )
            .unwrap();
    }
    assert_eq!(bitmap.into_straight_bgra(), [0; 36]);
    assert!(
        ColorGlyphBitmap::new(size(DevicePixels(0), DevicePixels(3)))
            .into_straight_bgra()
            .is_empty()
    );
}

#[test]
fn truncated_layer_coverage_is_rejected() {
    let mut bitmap = ColorGlyphBitmap::new(size(DevicePixels(3), DevicePixels(2)));
    assert_eq!(
        bitmap.composite(
            Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0
            },
            bounds(point(0, 0), size(3, 2)),
            &[255; 5],
            [0.0; 4],
            0.0
        ),
        Err("color glyph layer coverage is smaller than its bounds"),
    );
}

#[test]
fn contrast_and_gamma_correction_use_layer_brightness() {
    let mut bitmap = ColorGlyphBitmap::new(size(DevicePixels(2), DevicePixels(1)));
    for (column, value) in [0.0, 1.0].into_iter().enumerate() {
        bitmap
            .composite(
                Rgba {
                    r: value,
                    g: value,
                    b: value,
                    a: 1.0,
                },
                bounds(point(column as i32, 0), size(1, 1)),
                &[85],
                [0.25, 0.5, -0.5, 0.25],
                1.0,
            )
            .unwrap();
    }
    assert_eq!(
        bitmap.into_straight_bgra(),
        [0, 0, 0, 159, 255, 255, 255, 85]
    );
}

#[test]
fn layer_bounds_extend_the_base_glyph_without_adding_the_origin() {
    let base = bounds(
        point(DevicePixels(10), DevicePixels(11)),
        size(DevicePixels(2), DevicePixels(2)),
    );
    let layers = [
        bounds(
            point(DevicePixels(8), DevicePixels(9)),
            size(DevicePixels(3), DevicePixels(5)),
        ),
        bounds(
            point(DevicePixels(10), DevicePixels(12)),
            size(DevicePixels(5), DevicePixels(8)),
        ),
    ];
    let union = union_glyph_bounds([Default::default(), layers[0], layers[1]]);
    assert_eq!(
        union,
        bounds(
            point(DevicePixels(8), DevicePixels(9)),
            size(DevicePixels(7), DevicePixels(11))
        )
    );
    assert!(union.left() < base.left() && union.top() < base.top());
    assert!(union.right() > base.right() && union.bottom() > base.bottom());
    assert_eq!(union_glyph_bounds([Default::default()]), Default::default());
    let mut bitmap = ColorGlyphBitmap::new(union.size);
    for layer in layers {
        bitmap
            .composite(
                Rgba {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                bounds(
                    (layer.origin - union.origin).map(|coordinate| coordinate.0),
                    layer.size.map(|coordinate| coordinate.0),
                ),
                &vec![255; (layer.size.width.0 * layer.size.height.0) as usize],
                [0.0; 4],
                0.0,
            )
            .unwrap();
    }
    let bytes = bitmap.into_straight_bgra();
    assert_eq!(&bytes[..4], &[0, 0, 255, 255]);
    assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 255, 255]);
}
