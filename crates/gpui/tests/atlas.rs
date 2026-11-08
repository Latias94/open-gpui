use anyhow::{Result, anyhow, ensure};
use open_gpui::{
    AtlasAccessOutcome, AtlasBackend, AtlasKey, AtlasRemoveOutcome, AtlasState, AtlasTextureId,
    AtlasTextureInstanceId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, ImageId, Point,
    RenderImageParams, RenderSvgParams, Size, TileId,
};
use std::{borrow::Cow, cell::Cell, collections::HashMap, rc::Rc};

const PIXEL_SIZE: Size<DevicePixels> = Size {
    width: DevicePixels(2),
    height: DevicePixels(1),
};
const IMAGE_PIXELS: [u8; 8] = [17, 34, 51, 255, 68, 85, 102, 127];

struct CpuTexture {
    tile: AtlasTile,
    pixels: Box<[u8]>,
    releases: Rc<Cell<usize>>,
}

impl Drop for CpuTexture {
    fn drop(&mut self) {
        self.releases.set(self.releases.get() + 1);
    }
}

#[derive(Default)]
struct CpuAtlasBackend {
    textures: HashMap<AtlasTextureInstanceId, CpuTexture>,
    insert_attempts: usize,
    next_id: u32,
    releases: Rc<Cell<usize>>,
}

impl AtlasBackend for CpuAtlasBackend {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> Result<AtlasTile> {
        self.insert_attempts += 1;
        let channels = match kind {
            AtlasTextureKind::Monochrome => 1,
            AtlasTextureKind::Polychrome | AtlasTextureKind::Subpixel => 4,
        };
        let expected_bytes = usize::try_from(size.width.0)?
            .checked_mul(usize::try_from(size.height.0)?)
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| anyhow!("CPU texture dimensions overflow"))?;
        ensure!(
            bytes.len() == expected_bytes,
            "CPU pixel buffer length does not match the texture"
        );
        self.next_id += 1;
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: self.next_id,
                kind,
            },
            tile_id: TileId(self.next_id),
            padding: 0,
            bounds: Bounds {
                origin: Point::default(),
                size,
            },
            texture_generation: self.next_id,
            texture_generation_padding: 0,
        };
        self.textures.insert(
            tile.texture_instance(),
            CpuTexture {
                tile,
                pixels: bytes.into(),
                releases: Rc::clone(&self.releases),
            },
        );
        Ok(tile)
    }

    fn remove(&mut self, tile: AtlasTile) {
        let texture = self
            .textures
            .remove(&tile.texture_instance())
            .expect("removal must address a resource owned by this CPU backend");
        assert_eq!(texture.tile, tile);
    }
}

#[test]
fn external_cpu_backend_owns_pixels_through_publication_retry_and_reset() -> Result<()> {
    let mut atlas = AtlasState::new(CpuAtlasBackend::default());
    let releases = Rc::clone(&atlas.backend.releases);
    let image = RenderImageParams {
        image_id: ImageId(41),
        frame_index: 2,
    };
    let image_key = AtlasKey::from(image);

    let unavailable = atlas.get_or_insert_with_diagnostics(image_key.clone(), &mut || Ok(None))?;
    assert_eq!(unavailable.tile, None);
    assert_eq!(
        unavailable.diagnostic.outcome,
        AtlasAccessOutcome::Unavailable
    );
    assert!(!atlas.contains(&image_key));

    let decode_error = atlas
        .get_or_insert_with(image_key.clone(), &mut || {
            Err(anyhow!("consumer image decoder refused the frame"))
        })
        .expect_err("a decoder failure must remain retryable");
    assert_eq!(
        decode_error.to_string(),
        "consumer image decoder refused the frame"
    );
    assert!(!atlas.contains(&image_key));
    assert_eq!(atlas.backend.insert_attempts, 0);

    let upload_error = atlas
        .get_or_insert_with_diagnostics(image_key.clone(), &mut || {
            Ok(Some((PIXEL_SIZE, Cow::Borrowed(&IMAGE_PIXELS[..4]))))
        })
        .expect_err("a short buffer must fail before the CPU backend owns a texture");
    assert_eq!(
        upload_error.to_string(),
        "CPU pixel buffer length does not match the texture"
    );
    assert!(!atlas.contains(&image_key));
    assert!(atlas.backend.textures.is_empty());
    assert_eq!(releases.get(), 0);
    assert_eq!(atlas.backend.insert_attempts, 1);

    let mut source_pixels = IMAGE_PIXELS.to_vec();
    let inserted = atlas.get_or_insert_with_diagnostics(image_key.clone(), &mut || {
        Ok(Some((PIXEL_SIZE, Cow::Borrowed(source_pixels.as_slice()))))
    })?;
    let image_tile = inserted.tile.expect("the valid upload must publish a tile");
    assert_eq!(inserted.diagnostic.outcome, AtlasAccessOutcome::Inserted);
    assert_eq!(inserted.diagnostic.image, Some(image));
    assert_eq!(inserted.diagnostic.tile, Some(image_tile));
    assert_eq!(inserted.diagnostic.texture_id, Some(image_tile.texture_id));
    assert_eq!(inserted.diagnostic.tile_id, Some(image_tile.tile_id));
    assert_eq!(inserted.diagnostic.size, Some(PIXEL_SIZE));
    assert_eq!(image_tile.texture_id.kind, AtlasTextureKind::Polychrome);
    assert_eq!(image_tile.bounds.size, PIXEL_SIZE);
    assert_ne!(image_tile.texture_generation, 0);
    assert_eq!(image_tile.texture_generation_padding, 0);
    assert!(atlas.contains(&image_key));

    source_pixels.fill(0);
    let uploaded = &atlas.backend.textures[&image_tile.texture_instance()];
    assert_eq!(uploaded.tile, image_tile);
    assert_eq!(uploaded.pixels.as_ref(), IMAGE_PIXELS.as_slice());
    assert_eq!(
        atlas.get_or_insert_with(image_key.clone(), &mut || {
            Err(anyhow!("a cached CPU texture must bypass decoding"))
        })?,
        Some(image_tile)
    );
    assert_eq!(atlas.backend.insert_attempts, 2);

    let icon_key = AtlasKey::from(RenderSvgParams {
        path: "consumer/icon.svg".into(),
        size: PIXEL_SIZE,
    });
    let icon_tile = atlas
        .get_or_insert_with(icon_key.clone(), &mut || {
            Ok(Some((PIXEL_SIZE, Cow::Owned(vec![3, 5]))))
        })?
        .expect("the same typed backend must own monochrome icon pixels");
    assert_eq!(icon_tile.texture_id.kind, AtlasTextureKind::Monochrome);
    assert_eq!(
        atlas.backend.textures[&icon_tile.texture_instance()]
            .pixels
            .as_ref(),
        &[3, 5]
    );

    let removed = atlas.remove_with_diagnostics(&image_key);
    assert_eq!(removed.outcome, AtlasRemoveOutcome::RemoveHit);
    assert_eq!(removed.image, Some(image));
    assert_eq!(removed.texture_id, Some(image_tile.texture_id));
    assert!(!atlas.contains(&image_key));
    assert!(atlas.contains(&icon_key));
    assert_eq!(releases.get(), 1);
    assert_eq!(atlas.backend.textures.len(), 1);
    assert!(
        atlas
            .backend
            .textures
            .contains_key(&icon_tile.texture_instance())
    );
    atlas.remove(&image_key);
    assert_eq!(releases.get(), 1);

    let mut resets = 0;
    atlas.clear(|backend: &mut CpuAtlasBackend| {
        resets += 1;
        backend.textures.clear();
    });
    assert_eq!(resets, 1);
    assert_eq!(releases.get(), 2);
    assert!(!atlas.contains(&icon_key));
    assert!(atlas.backend.textures.is_empty());
    let missing = atlas.remove_with_diagnostics(&icon_key);
    assert_eq!(missing.outcome, AtlasRemoveOutcome::RemoveNoop);
    assert_eq!(missing.texture_id, None);

    let rebuilt = atlas
        .get_or_insert_with(image_key, &mut || {
            Ok(Some((PIXEL_SIZE, Cow::Owned(IMAGE_PIXELS.to_vec()))))
        })?
        .expect("the reset state must accept a fresh owned pixel buffer");
    assert_ne!(rebuilt.texture_instance(), image_tile.texture_instance());
    assert_eq!(
        atlas.backend.textures[&rebuilt.texture_instance()]
            .pixels
            .as_ref(),
        IMAGE_PIXELS.as_slice()
    );
    drop(atlas);
    assert_eq!(releases.get(), 3);
    Ok(())
}
