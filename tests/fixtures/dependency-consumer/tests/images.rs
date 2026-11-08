//! Public image decoding consumers without the `test-support` feature.

use open_gpui::{Image, ImageFormat, SvgRenderer, size};
use std::{sync::Arc, time::Duration};

const ANIMATED_WEBP: &[u8] = include_bytes!("../../images/red-blue-100ms.webp");
const EXIF_JPEG: &[u8] = include_bytes!("../../images/exif-orientation-rotate-180.jpg");

#[test]
fn downstream_decodes_animated_webp_frames_delays_and_bgra() {
    let image = Image::from_bytes(ImageFormat::Webp, ANIMATED_WEBP.to_vec());
    let decoded = image
        .to_image_data(SvgRenderer::new(Arc::new(())))
        .expect("the public decoder must preserve animated WebP frames");

    assert_eq!(decoded.frame_count(), 2);
    for frame in 0..2 {
        assert_eq!(decoded.size(frame), size(1.into(), 1.into()));
        assert_eq!(
            Duration::from(decoded.delay(frame)),
            Duration::from_millis(100),
        );
        let pixel = decoded
            .as_bytes(frame)
            .expect("each frame must have pixels");
        assert_eq!(pixel.len(), 4);
        assert_eq!(pixel[3], 255);
        let dominant_channel = if frame == 0 { 2 } else { 0 };
        let other_channel = if frame == 0 { 0 } else { 2 };
        assert!(
            pixel[dominant_channel] > 240,
            "expected red then blue in BGRA"
        );
        assert!(pixel[other_channel] < 10 && pixel[1] < 10);
    }
}

#[test]
fn downstream_applies_jpeg_exif_orientation_before_returning_bgra() {
    let image = Image::from_bytes(ImageFormat::Jpeg, EXIF_JPEG.to_vec());
    let decoded = image
        .to_image_data(SvgRenderer::new(Arc::new(())))
        .expect("the public decoder must apply EXIF orientation 3");

    assert_eq!(decoded.frame_count(), 1);
    assert_eq!(decoded.size(0), size(16.into(), 32.into()));
    assert_eq!(Duration::from(decoded.delay(0)), Duration::ZERO);
    let pixels = decoded.as_bytes(0).expect("the JPEG must have pixels");
    assert_eq!(pixels.len(), 16 * 32 * 4);
    assert_eq!(&pixels[..4], &[255, 255, 255, 255]);
    assert_eq!(&pixels[pixels.len() - 4..], &[0, 0, 0, 255]);
    assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
}

#[test]
fn downstream_reports_malformed_and_truncated_images_as_errors() {
    let renderer = SvgRenderer::new(Arc::new(()));
    for format in [ImageFormat::Webp, ImageFormat::Jpeg, ImageFormat::Gif] {
        for bytes in [b"".as_slice(), b"not an encoded image".as_slice()] {
            let image = Image::from_bytes(format, bytes.to_vec());
            assert!(
                image.to_image_data(renderer.clone()).is_err(),
                "malformed {format:?} must return an error",
            );
        }
    }
    for (format, bytes) in [
        (ImageFormat::Webp, &ANIMATED_WEBP[..12]),
        (ImageFormat::Jpeg, &EXIF_JPEG[..2]),
        (ImageFormat::Gif, b"GIF89a".as_slice()),
    ] {
        let image = Image::from_bytes(format, bytes.to_vec());
        assert!(
            image.to_image_data(renderer.clone()).is_err(),
            "a truncated {format:?} header must return an error",
        );
    }
}

#[cfg(feature = "avif-encoding")]
#[test]
fn downstream_opts_into_avif_encoding_alongside_gpui_decoding() {
    use image::{ExtendedColorType, ImageEncoder, codecs::avif::AvifEncoder};

    let image = Image::from_bytes(ImageFormat::Webp, ANIMATED_WEBP.to_vec());
    let decoded = image
        .to_image_data(SvgRenderer::new(Arc::new(())))
        .expect("GPUI WebP decoding must coexist with application-owned AVIF encoding");

    assert_eq!(decoded.frame_count(), 2);
    assert_eq!(decoded.size(0), size(1.into(), 1.into()));
    let bgra = decoded
        .as_bytes(0)
        .expect("the WebP frame must have pixels");
    assert_eq!(bgra.len(), 4);
    let rgba = [bgra[2], bgra[1], bgra[0], bgra[3]];

    let mut encoded = Vec::new();
    AvifEncoder::new_with_speed_quality(&mut encoded, 10, 80)
        .with_num_threads(Some(1))
        .write_image(&rgba, 1, 1, ExtendedColorType::Rgba8)
        .expect("the application-owned AVIF encoder must encode actual pixels");

    assert!(!encoded.is_empty(), "the AVIF encoder must write bytes");
    assert_eq!(
        image::guess_format(&encoded).expect("encoded bytes must have a recognized image format"),
        image::ImageFormat::Avif,
    );
}
