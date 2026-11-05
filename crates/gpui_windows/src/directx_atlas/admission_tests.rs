use super::*;
use open_gpui::{point, size};

fn pixels(width: i32, height: i32) -> Size<DevicePixels> {
    size(DevicePixels(width), DevicePixels(height))
}

#[test]
fn source_admission_checks_exact_rows_without_allocating_large_buffers() {
    for bytes_per_pixel in [1, 4] {
        let required = 3 * 5 * bytes_per_pixel as usize;
        let upload = AtlasUpload::for_source(pixels(3, 5), bytes_per_pixel, required)
            .expect("an exact complete image is admissible");
        assert_eq!(upload.row_pitch, 3 * bytes_per_pixel);
        assert!(AtlasUpload::for_source(pixels(3, 5), bytes_per_pixel, required - 1).is_err());
        assert!(AtlasUpload::for_source(pixels(3, 5), bytes_per_pixel, required + 1).is_ok());
        let maximum_bytes = 16384_usize * 16384 * bytes_per_pixel as usize;
        let maximum = AtlasUpload::for_source(
            pixels(MAX_ATLAS_DIMENSION, MAX_ATLAS_DIMENSION),
            bytes_per_pixel,
            maximum_bytes,
        )
        .expect("the existing maximum texture dimensions retain checked arithmetic");
        assert_eq!(maximum.row_pitch, 16384 * bytes_per_pixel);
        assert!(
            AtlasUpload::for_source(
                pixels(MAX_ATLAS_DIMENSION, MAX_ATLAS_DIMENSION),
                bytes_per_pixel,
                maximum_bytes - 1,
            )
            .is_err()
        );
    }
}

#[test]
fn source_admission_refuses_invalid_dimensions_and_pixel_formats() {
    for dimensions in [
        pixels(0, 1),
        pixels(1, 0),
        pixels(-1, 1),
        pixels(1, -1),
        pixels(16385, 1),
        pixels(1, 16385),
        pixels(i32::MAX, i32::MAX),
        pixels(i32::MIN, i32::MIN),
    ] {
        assert!(AtlasUpload::for_source(dimensions, 4, usize::MAX).is_err());
    }
    for bytes_per_pixel in [0, 2, 3, 8, u32::MAX] {
        assert!(AtlasUpload::for_source(pixels(1, 1), bytes_per_pixel, usize::MAX).is_err());
    }
}

#[test]
fn destination_admission_preserves_content_extent_and_refuses_outside_boxes() {
    let upload = AtlasUpload::for_source(pixels(3, 5), 4, 60).expect("complete source");
    let texture_size = pixels(16, 16);
    let bounds = Bounds {
        origin: point(DevicePixels(13), DevicePixels(11)),
        size: pixels(3, 5),
    };
    let destination = upload
        .destination(bounds, texture_size)
        .expect("an exact bottom/right edge remains inside the texture");
    assert_eq!(
        (
            destination.left,
            destination.top,
            destination.right,
            destination.bottom,
            destination.front,
            destination.back,
        ),
        (13, 11, 16, 16, 0, 1),
    );
    for (left, top) in [(-1, 0), (0, -1), (14, 11), (13, 12), (i32::MAX, 0)] {
        assert!(
            upload
                .destination(
                    Bounds {
                        origin: point(DevicePixels(left), DevicePixels(top)),
                        size: pixels(3, 5),
                    },
                    texture_size,
                )
                .is_err()
        );
    }
    assert!(
        upload
            .destination(
                Bounds {
                    origin: point(DevicePixels(0), DevicePixels(0)),
                    size: pixels(4, 5),
                },
                texture_size,
            )
            .is_err()
    );
    assert!(upload.destination(bounds, pixels(-1, 16)).is_err());
}
