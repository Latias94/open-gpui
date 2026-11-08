//! Native atlas preflight controls using the production `Scene::batches` iterator.
//! The ABI fixtures exercise preflight only; they do not provide a Scene clip arena.
//! Windows source review leaves macOS compilation and execution UNQUALIFIED.

use super::*;
use open_gpui::{
    AtlasKey, AtlasRemoveOutcome, AtlasTile, Corners, Hsla, ImageId, PaddedBool32, PlatformAtlas,
    RenderImageParams, RenderSvgParams,
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
            texture.get_bytes(
                bytes.as_mut_ptr().cast(),
                (3 * channels) as u64,
                metal::region_2d(
                    tile.bounds.origin.x.into(),
                    tile.bounds.origin.y.into(),
                    3,
                    2,
                ),
                0,
            );
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
