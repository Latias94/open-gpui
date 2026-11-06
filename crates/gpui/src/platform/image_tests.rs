use super::*;
use image::{Delay, ImageBuffer, ImageEncoder as _, Rgb, RgbaImage};

const EXIF_JPEG: &[u8] =
    include_bytes!("../../../../tests/fixtures/images/exif-orientation-rotate-180.jpg");
const ANIMATED_WEBP: &[u8] =
    include_bytes!("../../../../tests/fixtures/images/red-blue-100ms.webp");

fn decode(format: ImageFormat, bytes: &[u8]) -> Arc<RenderImage> {
    Image::from_bytes(format, bytes.to_vec())
        .to_image_data(SvgRenderer::new(Arc::new(())))
        .unwrap()
}

#[test]
fn test_svg_image_to_image_data_converts_to_bgra() {
    let render_image = decode(
        ImageFormat::Svg,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">
<rect width="1" height="1" fill="#38BDF8"/>
</svg>"##,
    );

    for pixel in render_image.as_bytes(0).unwrap().chunks_exact(4) {
        assert_eq!(pixel, &[0xF8, 0xBD, 0x38, 0xFF]);
    }
}

#[test]
fn test_image_to_image_data_applies_exif_orientation() {
    let render_image = decode(ImageFormat::Jpeg, EXIF_JPEG);
    assert_eq!(render_image.size(0), size(16.into(), 32.into()));
    let bytes = render_image.as_bytes(0).unwrap();
    assert_eq!(&bytes[..4], &[255, 255, 255, 255]);
    assert_eq!(&bytes[(16 * 32 - 1) * 4..], &[0, 0, 0, 255]);
}

#[test]
fn test_animated_webp_image_to_image_data_decodes_frames() {
    let render_image = decode(ImageFormat::Webp, ANIMATED_WEBP);
    assert_eq!(render_image.frame_count(), 2);
    for frame in 0..2 {
        assert_eq!(render_image.size(frame), size(1.into(), 1.into()));
        assert_eq!(
            Duration::from(render_image.delay(frame)),
            Duration::from_millis(100)
        );
    }
    let red = render_image.as_bytes(0).unwrap();
    let blue = render_image.as_bytes(1).unwrap();
    assert!(red[2] > 240 && red[0] < 10 && red[1] < 10);
    assert!(blue[0] > 240 && blue[1] < 10 && blue[2] < 10);
    assert_eq!(red[3], 255);
    assert_eq!(blue[3], 255);
}

#[test]
fn static_images_apply_all_eight_exif_transforms_before_bgra_conversion() {
    let asymmetric = image::DynamicImage::ImageRgb8(ImageBuffer::from_fn(5, 3, |x, y| {
        Rgb([20 + x as u8 * 45, 30 + y as u8 * 80, (x + y * 3) as u8 * 17])
    }));
    let mut jpeg = Cursor::new(Vec::new());
    asymmetric
        .write_to(&mut jpeg, image::ImageFormat::Jpeg)
        .unwrap();
    let jpeg = jpeg.into_inner();
    assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
    let mut encoded = jpeg[..2].to_vec();
    encoded.extend_from_slice(&EXIF_JPEG[20..56]);
    encoded.extend_from_slice(&jpeg[2..]);
    let raw = image::load_from_memory_with_format(&encoded, image::ImageFormat::Jpeg)
        .unwrap()
        .into_rgba8();
    let (width, height) = raw.dimensions();
    assert_ne!(width, height);
    assert_eq!(&EXIF_JPEG[40..52], &[1, 18, 0, 3, 0, 0, 0, 1, 0, 3, 0, 0]);
    assert_eq!(&encoded[22..34], &EXIF_JPEG[40..52]);
    let mut distinct = std::collections::HashSet::new();

    for orientation in 1..=8 {
        encoded[31] = orientation;
        let image = decode(ImageFormat::Jpeg, &encoded);
        let (out_width, out_height) = if orientation < 5 {
            (width, height)
        } else {
            (height, width)
        };
        assert_eq!(image.frame_count(), 1);
        assert_eq!(image.size(0), size(out_width.into(), out_height.into()));
        let bytes = image.as_bytes(0).unwrap();
        assert!(
            distinct.insert((out_width, out_height, bytes.to_vec())),
            "the fixture must distinguish all eight transforms: {orientation}"
        );
        for y in 0..out_height {
            for x in 0..out_width {
                let (source_x, source_y) = match orientation {
                    1 => (x, y),
                    2 => (width - 1 - x, y),
                    3 => (width - 1 - x, height - 1 - y),
                    4 => (x, height - 1 - y),
                    5 => (y, x),
                    6 => (y, height - 1 - x),
                    7 => (width - 1 - y, height - 1 - x),
                    8 => (width - 1 - y, x),
                    _ => unreachable!(),
                };
                let source = raw.get_pixel(source_x, source_y).0;
                let offset = ((y * out_width + x) * 4) as usize;
                assert_eq!(
                    &bytes[offset..offset + 4],
                    &[source[2], source[1], source[0], source[3]],
                    "orientation {orientation} at ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn animated_webp_uses_transparent_canvas_outside_encoded_frames() {
    let mut bytes = ANIMATED_WEBP.to_vec();
    assert_eq!(&bytes[12..16], b"VP8X");
    assert_eq!(bytes[20], 2);
    assert_eq!(&bytes[24..30], &[0; 6]);
    bytes[20] |= 0x10;
    bytes[24] = 1;
    let image = decode(ImageFormat::Webp, &bytes);
    assert_eq!(image.frame_count(), 2);
    for frame in 0..2 {
        assert_eq!(image.size(frame), size(2.into(), 1.into()));
        let actual = image.as_bytes(frame).unwrap();
        assert_eq!(actual.len(), 8);
        assert_eq!(actual[3], 255);
        assert_eq!(&actual[4..], &[0, 0, 0, 0]);
    }
}

#[test]
fn static_png_and_webp_preserve_straight_alpha_and_channel_order() {
    let rgba = vec![240, 30, 70, 128, 1, 150, 201, 255, 31, 63, 95, 0];
    let pixels = RgbaImage::from_raw(3, 1, rgba.clone()).unwrap();
    for format in [ImageFormat::Png, ImageFormat::Webp] {
        let mut bytes = Vec::new();
        match format {
            ImageFormat::Png => image::codecs::png::PngEncoder::new(&mut bytes)
                .write_image(&rgba, 3, 1, image::ColorType::Rgba8.into())
                .unwrap(),
            ImageFormat::Webp => image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
                .write_image(&rgba, 3, 1, image::ColorType::Rgba8.into())
                .unwrap(),
            _ => unreachable!(),
        }
        let image = decode(format, &bytes);
        assert_eq!(image.frame_count(), 1);
        assert_eq!(image.size(0), size(3.into(), 1.into()));
        for (pixel, source) in image
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .zip(pixels.pixels())
        {
            assert_eq!(pixel, &[source[2], source[1], source[0], source[3]]);
        }
    }
}

#[test]
fn remaining_static_formats_keep_dimensions_and_bgra_pixels() {
    let source = image::DynamicImage::ImageRgb8(ImageBuffer::from_fn(3, 2, |x, y| {
        Rgb([20 + x as u8 * 50, 40 + y as u8 * 70, 150])
    }));
    for (public, codec) in [
        (ImageFormat::Bmp, image::ImageFormat::Bmp),
        (ImageFormat::Tiff, image::ImageFormat::Tiff),
        (ImageFormat::Ico, image::ImageFormat::Ico),
        (ImageFormat::Pnm, image::ImageFormat::Pnm),
    ] {
        let mut encoded = Cursor::new(Vec::new());
        if codec == image::ImageFormat::Ico {
            image::DynamicImage::ImageRgba8(source.to_rgba8())
                .write_to(&mut encoded, codec)
                .unwrap();
        } else {
            source.write_to(&mut encoded, codec).unwrap();
        }
        let image = decode(public, encoded.get_ref());
        assert_eq!(image.frame_count(), 1);
        assert_eq!(image.size(0), size(3.into(), 2.into()));
        let expected = source.to_rgba8();
        for (pixel, source) in image
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .zip(expected.pixels())
        {
            assert_eq!(pixel, &[source[2], source[1], source[0], 255]);
        }
    }
}

#[test]
fn animated_gif_keeps_frames_delays_and_transparency() {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for (pixel, millis) in [([240, 20, 60, 255], 20), ([20, 180, 240, 255], 50)] {
            let buffer = RgbaImage::from_fn(2, 1, |x, _| {
                if x == 0 {
                    Rgba(pixel)
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
            encoder
                .encode_frame(Frame::from_parts(
                    buffer,
                    0,
                    0,
                    Delay::from_numer_denom_ms(millis, 1),
                ))
                .unwrap();
        }
    }
    let image = decode(ImageFormat::Gif, &bytes);
    assert_eq!(image.frame_count(), 2);
    for (index, expected, millis) in [(0, [60, 20, 240, 255], 20), (1, [240, 180, 20, 255], 50)] {
        assert_eq!(image.size(index), size(2.into(), 1.into()));
        assert_eq!(
            Duration::from(image.delay(index)),
            Duration::from_millis(millis)
        );
        let actual = image.as_bytes(index).unwrap();
        assert_eq!(&actual[..4], &expected);
        assert_eq!(actual[7], 0);
    }
}

struct AnimationFrames(Vec<image::ImageResult<Frame>>);

impl<'a> image::AnimationDecoder<'a> for AnimationFrames {
    fn into_frames(self) -> image::Frames<'a> {
        image::Frames::new(Box::new(self.0.into_iter()))
    }
}

fn broken_frame() -> image::ImageResult<Frame> {
    Err(image::ImageError::IoError(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid frame",
    )))
}

#[test]
fn shared_animation_policy_keeps_successful_frames_and_refuses_empty_output() {
    let frame = Frame::from_parts(
        RgbaImage::from_pixel(1, 1, Rgba([220, 30, 70, 128])),
        2,
        4,
        Delay::from_numer_denom_ms(123, 1),
    );
    let frames = decode_animated_frames(
        AnimationFrames(vec![broken_frame(), Ok(frame), broken_frame()]),
        "Test",
    )
    .unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!((frames[0].left(), frames[0].top()), (2, 4));
    assert_eq!(frames[0].buffer().as_raw(), &[70, 30, 220, 128]);
    assert_eq!(
        Duration::from(frames[0].delay()),
        Duration::from_millis(123)
    );
    for entries in [vec![], vec![broken_frame(), broken_frame()]] {
        let error = decode_animated_frames(AnimationFrames(entries), "Test")
            .err()
            .expect("empty animation must fail");
        assert_eq!(
            error.to_string(),
            "Test could not be decoded: all frames failed"
        );
    }
}

#[test]
fn malformed_encoded_images_return_errors() {
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Gif,
        ImageFormat::Webp,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::Ico,
        ImageFormat::Pnm,
        ImageFormat::Svg,
    ] {
        assert!(
            Image::from_bytes(format, b"invalid encoded image".to_vec())
                .to_image_data(SvgRenderer::new(Arc::new(())))
                .is_err(),
            "{format:?}"
        );
    }
}
