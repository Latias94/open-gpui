//! Native Metal renderer controls using the production `Scene::batches` iterator.
//! The ABI fixtures exercise preflight only; they do not provide a Scene clip arena.
//! Windows source review leaves macOS compilation and execution UNQUALIFIED.

use super::*;
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString, CFType};
use objc2_core_video::{
    CVPixelBufferCreate, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
};
use open_gpui::{
    AppContext as _, AtlasKey, AtlasRemoveOutcome, AtlasTile, Context, Corners, HeadlessAppContext,
    Hsla, ImageId, IntoElement, MonochromeSprite, NoopTextSystem, PaddedBool32, Pixels,
    PlatformAtlas, PlatformPixelBuffer, PolychromeSprite, Render, RenderImageParams,
    RenderSvgParams, Styled as _, SubtreeClip, SubtreeTransform, SubtreeTransformExt as _, Window,
    canvas, px, rgb,
};
use std::borrow::Cow;

#[repr(C)]
struct MonochromeFixture {
    order: u32,
    pad: u32,
    bounds: Bounds<ScaledPixels>,
    clip: ClipEnvelope,
    color: Hsla,
    tile: AtlasTile,
    transform: PrimitiveTransform,
}

#[repr(C)]
struct PolychromeFixture {
    order: u32,
    pad: u32,
    grayscale: PaddedBool32,
    opacity: f32,
    bounds: Bounds<ScaledPixels>,
    clip: ClipEnvelope,
    corner_radii: Corners<ScaledPixels>,
    tile: AtlasTile,
    transform: PrimitiveTransform,
}

fn mixed_scene(monochrome: AtlasTile, polychrome: AtlasTile, monochrome_first: bool) -> Scene {
    let bounds = Bounds::new(
        point(ScaledPixels(0.0), ScaledPixels(0.0)),
        size(ScaledPixels(3.0), ScaledPixels(2.0)),
    );
    let monochrome = MonochromeFixture {
        order: u32::from(!monochrome_first),
        pad: 0,
        bounds,
        clip: ClipEnvelope::default(),
        color: Hsla::default(),
        tile: monochrome,
        transform: PrimitiveTransform::IDENTITY,
    };
    let polychrome = PolychromeFixture {
        order: u32::from(monochrome_first),
        pad: 0,
        grayscale: false.into(),
        opacity: 1.0,
        bounds,
        clip: ClipEnvelope::default(),
        corner_radii: Corners::default(),
        tile: polychrome,
        transform: PrimitiveTransform::IDENTITY,
    };
    let mut scene = Scene::default();
    // SAFETY: Both fixtures mirror the corresponding repr(C) sprite's exact fields and order,
    // including its terminal identity transform. Transmute also checks the complete byte sizes.
    unsafe {
        scene
            .monochrome_sprites
            .push(mem::transmute::<MonochromeFixture, MonochromeSprite>(
                monochrome,
            ));
        scene
            .polychrome_sprites
            .push(mem::transmute::<PolychromeFixture, PolychromeSprite>(
                polychrome,
            ));
    }
    scene.finish();
    scene
}

fn keys() -> (AtlasKey, AtlasKey) {
    (
        AtlasKey::Svg(RenderSvgParams {
            path: "metal-preflight.svg".into(),
            size: size(DevicePixels(3), DevicePixels(2)),
        }),
        AtlasKey::Image(RenderImageParams {
            image_id: ImageId(1),
            frame_index: 0,
        }),
    )
}

fn insert(atlas: &MetalAtlas, key: AtlasKey) -> Result<AtlasTile> {
    let channels = match key.texture_kind() {
        open_gpui::AtlasTextureKind::Monochrome => 1,
        _ => 4,
    };
    atlas
        .get_or_insert_with(key, &mut || {
            Ok(Some((
                size(DevicePixels(3), DevicePixels(2)),
                Cow::Owned(vec![97; 3 * 2 * channels]),
            )))
        })?
        .context("inserting preflight texture")
}

#[test]
fn mixed_missing_preflight_refuses_both_orders_and_recovers_after_reallocation() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let (monochrome_key, polychrome_key) = keys();
        let mut monochrome = insert(&renderer.sprite_atlas, monochrome_key.clone())?;
        let polychrome = insert(&renderer.sprite_atlas, polychrome_key.clone())?;
        renderer.sprite_atlas.remove(&polychrome_key);
        let buffer_size = renderer.instance_buffer_pool.lock().buffer_size;
        for monochrome_first in [false, true] {
            let scene = mixed_scene(monochrome, polychrome, monochrome_first);
            assert_eq!(scene.batches().count(), 2);
            assert!(renderer.prepare_scene(&scene).is_err());
            assert_eq!(
                renderer.instance_buffer_pool.lock().buffer_size,
                buffer_size
            );
            assert!(renderer.path_intermediate_texture.is_none());
            assert!(renderer.headless_render_target.is_none());
            assert!(renderer.last_submitted_command_buffer.is_none());
        }
        assert_eq!(
            renderer
                .sprite_atlas
                .remove_with_diagnostics(&monochrome_key)
                .outcome,
            AtlasRemoveOutcome::TextureFreed
        );
        monochrome = insert(&renderer.sprite_atlas, monochrome_key)?;
        let replacement = insert(&renderer.sprite_atlas, polychrome_key)?;
        assert_eq!(replacement.texture_id, polychrome.texture_id);
        assert_ne!(
            replacement.texture_instance(),
            polychrome.texture_instance()
        );
        let stale = mixed_scene(monochrome, polychrome, true);
        assert!(renderer.prepare_scene(&stale).is_err());
        let recovered = mixed_scene(monochrome, replacement, true);
        let receipt = renderer.prepare_scene(&recovered)?;
        assert_eq!(receipt.batches.len(), 2);
        assert_eq!(receipt.atlas_lease.texture_instances().len(), 2);
        drop(receipt);
        let image = renderer
            .render_scene_to_image(&Scene::default(), size(DevicePixels(7), DevicePixels(3)))?;
        assert_eq!(image.dimensions(), (7, 3));
        assert!(image.pixels().all(|pixel| pixel.0 == [0, 0, 0, 255]));
        renderer.destroy();
        Ok(())
    })
}

#[test]
fn preflight_receipt_pins_actual_mixed_textures_after_key_removal() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let (monochrome_key, polychrome_key) = keys();
        let monochrome = insert(&renderer.sprite_atlas, monochrome_key.clone())?;
        let polychrome = insert(&renderer.sprite_atlas, polychrome_key.clone())?;
        let scene = mixed_scene(monochrome, polychrome, true);
        let receipt = renderer.prepare_scene(&scene)?;
        for key in [&monochrome_key, &polychrome_key] {
            assert_eq!(
                renderer.sprite_atlas.remove_with_diagnostics(key).outcome,
                AtlasRemoveOutcome::TextureRetained
            );
        }
        for (batch, tile) in receipt.batches.iter().zip([monochrome, polychrome]) {
            let texture = match batch {
                PreparedMetalBatch::MonochromeSprites { texture, .. }
                | PreparedMetalBatch::PolychromeSprites { texture, .. } => texture,
                _ => panic!("the receipt must contain the exact mixed sprite batches"),
            };
            let resident = renderer
                .sprite_atlas
                .metal_texture(tile.texture_instance())
                .unwrap();
            assert!(ptr::eq(&**texture, &*resident));
            let channels = if tile == monochrome { 1 } else { 4 };
            let mut bytes = vec![0; 3 * 2 * channels];
            // SAFETY: The preflight pins this exact tile and the destination has all six texels at the correct stride.
            unsafe {
                texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    ptr::NonNull::from(bytes.as_mut_slice()).cast(),
                    3 * channels,
                    MTLRegion {
                        origin: MTLOrigin {
                            x: tile.bounds.origin.x.into(),
                            y: tile.bounds.origin.y.into(),
                            z: 0,
                        },
                        size: MTLSize {
                            width: 3,
                            height: 2,
                            depth: 1,
                        },
                    },
                    0,
                );
            }
            assert_eq!(bytes, vec![97; 3 * 2 * channels]);
        }
        drop(receipt);
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(monochrome.texture_instance())
                .is_none()
        );
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(polychrome.texture_instance())
                .is_none()
        );
        renderer.destroy();
        Ok(())
    })
}

#[test]
fn repeated_batches_pin_each_instance_once_and_preserve_every_draw_range() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let (monochrome_key, polychrome_key) = keys();
        let monochrome = insert(&renderer.sprite_atlas, monochrome_key.clone())?;
        let polychrome = insert(&renderer.sprite_atlas, polychrome_key.clone())?;
        let mut scene = mixed_scene(monochrome, polychrome, true);
        let mut repeated = scene.monochrome_sprites[0];
        repeated.order = 2;
        scene.monochrome_sprites.push(repeated);
        scene.finish();
        assert_eq!(scene.batches().count(), 3);
        let receipt = renderer.prepare_scene(&scene)?;
        assert_eq!(receipt.atlas_lease.texture_instances().len(), 2);
        assert!(matches!(
            &receipt.batches[..],
            [
                PreparedMetalBatch::MonochromeSprites { range: first, .. },
                PreparedMetalBatch::PolychromeSprites { range: second, .. },
                PreparedMetalBatch::MonochromeSprites { range: third, .. },
            ] if *first == (0..1) && *second == (0..1) && *third == (1..2)
        ));
        renderer.sprite_atlas.remove(&monochrome_key);
        renderer.sprite_atlas.remove(&polychrome_key);
        drop(receipt);
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(monochrome.texture_instance())
                .is_none()
        );
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(polychrome.texture_instance())
                .is_none()
        );
        renderer.destroy();
        Ok(())
    })
}

fn video_buffer(luma: u8, chroma: [u8; 2]) -> Result<PlatformPixelBuffer> {
    let io_surface = CFDictionary::<CFString, CFType>::empty();
    let mut buffer = ptr::null_mut();
    // SAFETY: The attributes have their documented CF types and the output pointer is writable.
    let status = unsafe {
        let attributes = CFDictionary::<CFString, CFType>::from_slices(
            &[
                kCVPixelBufferIOSurfacePropertiesKey,
                kCVPixelBufferMetalCompatibilityKey,
            ],
            &[&io_surface, CFBoolean::new(true)],
        );
        CVPixelBufferCreate(
            None,
            16,
            16,
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            Some(attributes.as_opaque()),
            ptr::NonNull::from(&mut buffer),
        )
    };
    anyhow::ensure!(
        status == kCVReturnSuccess,
        "creating test video buffer: {status}"
    );
    let buffer = ptr::NonNull::new(buffer).context("retrieving test video buffer")?;
    let flags = CVPixelBufferLockFlags::empty();
    // SAFETY: CoreVideo transfers ownership on success and no other user can access this buffer.
    let (buffer, status) = unsafe {
        let buffer = CFRetained::from_raw(buffer);
        let status = CVPixelBufferLockBaseAddress(&buffer, flags);
        (buffer, status)
    };
    anyhow::ensure!(
        status == kCVReturnSuccess,
        "locking test video buffer: {status}"
    );

    let fill: Result<()> = (|| {
        for (plane, texel) in [(0, &[luma][..]), (1, &chroma[..])] {
            let base = CVPixelBufferGetBaseAddressOfPlane(&buffer, plane).cast::<u8>();
            let stride = CVPixelBufferGetBytesPerRowOfPlane(&buffer, plane);
            let width = CVPixelBufferGetWidthOfPlane(&buffer, plane);
            anyhow::ensure!(
                !base.is_null() && width * texel.len() <= stride,
                "invalid test video plane"
            );
            for row in 0..CVPixelBufferGetHeightOfPlane(&buffer, plane) {
                for column in 0..width {
                    let offset = row * stride + column * texel.len();
                    // SAFETY: The destination texel is within the locked plane and disjoint from the source.
                    unsafe {
                        ptr::copy_nonoverlapping(texel.as_ptr(), base.add(offset), texel.len());
                    }
                }
            }
        }
        Ok(())
    })();
    // SAFETY: The successful lock used the same flags, and the CPU writes are complete.
    let status = unsafe { CVPixelBufferUnlockBaseAddress(&buffer, flags) };
    fill?;
    anyhow::ensure!(
        status == kCVReturnSuccess,
        "unlocking test video buffer: {status}"
    );
    Ok(buffer)
}

struct SurfaceBatchFixture([PlatformPixelBuffer; 3]);

impl Render for SurfaceBatchFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let [first, second, third] = self.0.clone();
        let bounds_at = |x| Bounds::new(point(px(x), px(0.0)), size(px(12.0), px(12.0)));
        let clip = SubtreeClip::try_rounded_rect(
            bounds_at(0.0),
            Corners::all(px(6.0)).map(|radius| size(*radius, *radius)),
        )
        .expect("valid rounded clip for the second surface");
        canvas(
            move |_, window, _| window.prepare_subtree_clip(&clip, bounds_at(16.0)),
            move |_, clip, window, _| {
                window.paint_surface(bounds_at(0.0), first);
                let mut separator = Path::new(point(px(8.0), px(0.0)));
                for (x, y) in [(40.0, 0.0), (40.0, 12.0), (8.0, 12.0), (8.0, 0.0)] {
                    separator.line_to(point(px(x), px(y)));
                }
                window.paint_path(separator, rgb(0xff0000));
                window
                    .with_prepared_subtree_clip(&clip, |window| {
                        window.paint_surface(bounds_at(16.0), second);
                    })
                    .expect("the second surface uses its prepaint clip");
                window.paint_surface(bounds_at(32.0), third);
            },
        )
        .size_full()
        .with_subtree_transform(
            SubtreeTransform::try_translation(point(px(2.0), px(2.0)))
                .expect("valid fixture translation"),
        )
    }
}

#[test]
fn surface_batches_read_back_with_global_instance_indices() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let fixture = SurfaceBatchFixture([
            video_buffer(64, [128, 128])?,
            video_buffer(96, [128, 192])?,
            video_buffer(128, [192, 128])?,
        ]);
        let mut app =
            HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), || {
                Ok(Some(Box::new(MetalHeadlessRenderer::new()?)))
            });
        let viewport: Size<Pixels> = size(px(48.0), px(20.0));
        let handle = app.open_window(viewport, |_, app| app.new(|_| fixture))?;
        app.update_window(handle.into(), |_, window, _| {
            let _ = window.activate_window();
        })?;
        app.run_until_parked();
        app.simulate_window_scheduled_frame(handle.into())?;
        let capture = app.capture_frame(handle.into())?;
        let scale = capture.metadata.scale_factor;
        let image = capture.into_image()?;

        for (x, y, expected) in [
            (1.0, 8.0, [0, 0, 0, 255]),
            (5.0, 8.0, [65, 63, 65, 255]),
            (12.0, 8.0, [255, 0, 0, 255]),
            (19.0, 3.0, [255, 0, 0, 255]),
            (24.0, 8.0, [186, 50, 97, 255]),
            (40.0, 8.0, [129, 105, 242, 255]),
        ] {
            let actual = image.get_pixel((x * scale) as u32, (y * scale) as u32).0;
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "pixel at ({x}, {y}) was {actual:?}, expected {expected:?}",
            );
        }
        Ok(())
    })
}
