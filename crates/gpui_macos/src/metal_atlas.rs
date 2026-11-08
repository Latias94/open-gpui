use crate::metal_compat as metal;
use crate::metal_compat::Device;
use anyhow::{Context as _, Result, ensure};
use derive_more::{Deref, DerefMut};
use etagere::BucketedAtlasAllocator;
use open_gpui::{
    AtlasAccess, AtlasBackend, AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasState,
    AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind, AtlasTextureLease,
    AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTextureList, AtlasTile, Bounds,
    DevicePixels, PlatformAtlas, Point, Size,
};
use open_gpui_collections::FxHashMap;
use parking_lot::Mutex;
use std::{borrow::Cow, sync::Arc};

const MAX_ATLAS_DIMENSION: i32 = 16384;

pub(crate) struct MetalAtlas(Mutex<AtlasState<MetalAtlasTextures>>);

impl MetalAtlas {
    pub(crate) fn new(device: Device, is_apple_gpu: bool) -> Self {
        MetalAtlas(Mutex::new(AtlasState::new(MetalAtlasTextures {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_texture_generation: 1,
            device: AssertSend(device),
            is_apple_gpu,
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
        })))
    }

    /// Returns the exact resident texture instance, including a lease-retained instance.
    /// Released slots and replacement generations return `None`.
    pub(crate) fn metal_texture(&self, texture: AtlasTextureInstanceId) -> Option<metal::Texture> {
        Some(
            self.0
                .lock()
                .backend
                .texture_if_resident(texture)?
                .metal_texture
                .clone(),
        )
    }

    pub(crate) fn retain_metal_textures(
        self: &Arc<Self>,
        textures: &[AtlasTextureInstanceId],
    ) -> Result<(
        AtlasTextureLease,
        FxHashMap<AtlasTextureInstanceId, metal::Texture>,
    )> {
        let atlas: Arc<dyn PlatformAtlas> = self.clone();
        let lease = atlas.retain_texture_instances(textures)?;
        let snapshot = {
            let state = self.0.lock();
            if state.backend.epoch != lease.epoch() {
                Err(anyhow::anyhow!(
                    "Metal atlas epoch changed during scene preflight"
                ))
            } else {
                lease
                    .texture_instances()
                    .iter()
                    .map(|&instance| {
                        let texture =
                            state
                                .backend
                                .texture_if_resident(instance)
                                .with_context(|| {
                                    format!("Metal atlas texture {instance:?} is unavailable")
                                })?;
                        Ok((instance, texture.metal_texture.clone()))
                    })
                    .collect::<Result<FxHashMap<_, _>>>()
            }
        };
        Ok((lease, snapshot?))
    }
}

struct MetalAtlasTextures {
    epoch: AtlasTextureLeaseEpoch,
    next_texture_generation: u32,
    device: AssertSend<Device>,
    is_apple_gpu: bool,
    monochrome_textures: AtlasTextureList<MetalAtlasTexture>,
    polychrome_textures: AtlasTextureList<MetalAtlasTexture>,
}

impl PlatformAtlas for MetalAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        self.0.lock().get_or_insert_with(key, build)
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<AtlasAccess> {
        self.0.lock().get_or_insert_with_diagnostics(key, build)
    }

    fn remove(&self, key: &AtlasKey) {
        self.0.lock().remove(key);
    }

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        self.0.lock().remove_with_diagnostics(key)
    }

    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    fn contains(&self, key: &AtlasKey) -> bool {
        self.0.lock().contains(key)
    }

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        self.0.lock().backend.epoch
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        debug_assert!(
            textures
                .iter()
                .enumerate()
                .all(|(index, texture)| !textures[..index].contains(texture)),
            "atlas texture lease acquisition requires deduplicated texture instances"
        );
        let mut lock = self.0.lock();
        let lock = &mut lock.backend;
        let epoch = lock.epoch;
        for texture in textures.iter().copied() {
            let Some(resident) = lock.texture_if_resident(texture) else {
                return Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch });
            };
            if resident.live_visual_leases == u32::MAX {
                return Err(AtlasTextureLeaseError::LeaseCountOverflow { texture, epoch });
            }
        }
        for texture in textures.iter().copied() {
            if let Some(resident) = lock.texture_if_resident_mut(texture) {
                resident.live_visual_leases += 1;
            }
        }
        Ok(epoch)
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        epoch: AtlasTextureLeaseEpoch,
        textures: &[AtlasTextureInstanceId],
    ) {
        debug_assert!(
            textures
                .iter()
                .enumerate()
                .all(|(index, texture)| !textures[..index].contains(texture)),
            "atlas texture lease release requires deduplicated texture instances"
        );
        let mut lock = self.0.lock();
        let lock = &mut lock.backend;
        if lock.epoch != epoch {
            return;
        }
        for texture in textures.iter().copied() {
            lock.release_visual_lease(texture);
        }
    }
}

impl AtlasBackend for MetalAtlasTextures {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> Result<AtlasTile> {
        let upload = AtlasUpload::new(kind, size, bytes)?;
        let tile = self.allocate(size, kind)?;
        let result = self
            .texture_if_resident(tile.texture_instance())
            .context("looking up allocated atlas texture")
            .and_then(|texture| texture.upload(tile.bounds.origin, &upload));
        if let Err(error) = result {
            self.cancel_unpublished_allocation(tile);
            return Err(error);
        }
        Ok(tile)
    }

    fn remove(&mut self, tile: AtlasTile) {
        self.remove_with_diagnostics(tile);
    }

    /// Published regions remain allocated until the texture has no keys or leases.
    /// Reusing them earlier requires proof that their final GPU read has completed.
    fn remove_with_diagnostics(&mut self, tile: AtlasTile) -> AtlasRemoveOutcome {
        let instance = tile.texture_instance();
        let id = instance.texture_id;
        let Some(textures) = self.texture_list_mut(id.kind) else {
            return AtlasRemoveOutcome::RemoveHit;
        };
        let Some(texture_slot) = textures.textures.get_mut(id.index as usize) else {
            return AtlasRemoveOutcome::RemoveHit;
        };
        let Some(texture) = texture_slot
            .as_mut()
            .filter(|texture| texture.id == instance)
        else {
            return AtlasRemoveOutcome::RemoveHit;
        };
        texture.decrement_ref_count();
        if texture.is_unreferenced() {
            texture_slot.take();
            textures.free_list.push(id.index as usize);
            AtlasRemoveOutcome::TextureFreed
        } else {
            AtlasRemoveOutcome::TextureRetained
        }
    }
}

impl MetalAtlasTextures {
    fn cancel_unpublished_allocation(&mut self, tile: AtlasTile) {
        let Some(textures) = self.texture_list_mut(tile.texture_id.kind) else {
            return;
        };
        let Some(slot) = textures.textures.get_mut(tile.texture_id.index as usize) else {
            return;
        };
        let Some(texture) = slot
            .as_mut()
            .filter(|texture| texture.id == tile.texture_instance())
        else {
            return;
        };
        texture.allocator.deallocate(tile.tile_id.into());
        texture.decrement_ref_count();
        if texture.is_unreferenced() {
            slot.take();
            textures.free_list.push(tile.texture_id.index as usize);
        }
    }

    fn texture_list(&self, kind: AtlasTextureKind) -> Option<&AtlasTextureList<MetalAtlasTexture>> {
        match kind {
            AtlasTextureKind::Monochrome => Some(&self.monochrome_textures),
            AtlasTextureKind::Polychrome => Some(&self.polychrome_textures),
            AtlasTextureKind::Subpixel => None,
        }
    }

    fn texture_list_mut(
        &mut self,
        kind: AtlasTextureKind,
    ) -> Option<&mut AtlasTextureList<MetalAtlasTexture>> {
        match kind {
            AtlasTextureKind::Monochrome => Some(&mut self.monochrome_textures),
            AtlasTextureKind::Polychrome => Some(&mut self.polychrome_textures),
            AtlasTextureKind::Subpixel => None,
        }
    }

    fn texture_if_resident(&self, texture: AtlasTextureInstanceId) -> Option<&MetalAtlasTexture> {
        self.texture_list(texture.texture_id.kind)?
            .textures
            .get(texture.texture_id.index as usize)
            .and_then(|texture| texture.as_ref())
            .filter(|resident| resident.id == texture)
    }

    fn texture_if_resident_mut(
        &mut self,
        texture: AtlasTextureInstanceId,
    ) -> Option<&mut MetalAtlasTexture> {
        self.texture_list_mut(texture.texture_id.kind)?
            .textures
            .get_mut(texture.texture_id.index as usize)
            .and_then(|texture| texture.as_mut())
            .filter(|resident| resident.id == texture)
    }

    fn release_visual_lease(&mut self, instance: AtlasTextureInstanceId) {
        let id = instance.texture_id;
        let Some(texture_list) = self.texture_list_mut(id.kind) else {
            return;
        };
        let Some(texture_slot) = texture_list.textures.get_mut(id.index as usize) else {
            return;
        };
        let Some(mut texture) = texture_slot.take() else {
            return;
        };
        if texture.id != instance {
            *texture_slot = Some(texture);
            return;
        }
        if !texture.release_visual_lease() {
            *texture_slot = Some(texture);
            return;
        }
        if texture.is_unreferenced() {
            texture_list
                .free_list
                .push(texture.id.texture_id.index as usize);
        } else {
            *texture_slot = Some(texture);
        }
    }

    fn allocate(
        &mut self,
        size: Size<DevicePixels>,
        texture_kind: AtlasTextureKind,
    ) -> Result<AtlasTile> {
        {
            let textures = match texture_kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
                AtlasTextureKind::Subpixel => {
                    anyhow::bail!("Metal does not support subpixel atlas textures")
                }
            };

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return Ok(tile);
            }
        }

        let texture = self.push_texture(size, texture_kind)?;
        texture.allocate(size).context("allocating atlas tile")
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> Result<&mut MetalAtlasTexture> {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        // Max texture size on all modern Apple GPUs. Anything bigger than that crashes in validateWithDevice.
        const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(MAX_ATLAS_DIMENSION),
            height: DevicePixels(MAX_ATLAS_DIMENSION),
        };
        let size = min_size.min(&MAX_ATLAS_SIZE).max(&DEFAULT_ATLAS_SIZE);
        let texture_descriptor = metal::TextureDescriptor::new();
        texture_descriptor.set_width(size.width.into());
        texture_descriptor.set_height(size.height.into());
        let pixel_format;
        let usage;
        match kind {
            AtlasTextureKind::Monochrome => {
                pixel_format = metal::MTLPixelFormat::A8Unorm;
                usage = metal::MTLTextureUsage::ShaderRead;
            }
            AtlasTextureKind::Polychrome => {
                pixel_format = metal::MTLPixelFormat::BGRA8Unorm;
                usage = metal::MTLTextureUsage::ShaderRead;
            }
            AtlasTextureKind::Subpixel => {
                anyhow::bail!("Metal does not support subpixel atlas textures")
            }
        }
        texture_descriptor.set_pixel_format(pixel_format);
        texture_descriptor.set_usage(usage);
        // Shared memory mode can be used only on Apple GPU families
        // https://developer.apple.com/documentation/metal/mtlresourceoptions/storagemodeshared
        texture_descriptor.set_storage_mode(if self.is_apple_gpu {
            metal::MTLStorageMode::Shared
        } else {
            metal::MTLStorageMode::Managed
        });
        let generation = self.next_texture_generation;
        let next_generation = self
            .next_texture_generation
            .checked_add(1)
            .context("Metal atlas texture generation exhausted")?;
        let metal_texture = self
            .device
            .try_new_texture(&texture_descriptor)
            .context("allocating atlas texture")?;

        let texture_list = match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Subpixel => {
                anyhow::bail!("Metal does not support subpixel atlas textures")
            }
        };

        let index = texture_list
            .free_list
            .last()
            .copied()
            .unwrap_or(texture_list.textures.len());
        let texture_index = u32::try_from(index).context("Metal atlas texture index exhausted")?;
        if texture_list.free_list.pop().is_none() {
            texture_list.textures.push(None);
        }

        let atlas_texture = MetalAtlasTexture {
            id: AtlasTextureInstanceId {
                texture_id: AtlasTextureId {
                    index: texture_index,
                    kind,
                },
                generation,
            },
            allocator: etagere::BucketedAtlasAllocator::new(size_to_etagere(size)),
            metal_texture: AssertSend(metal_texture),
            live_atlas_keys: 0,
            live_visual_leases: 0,
        };

        self.next_texture_generation = next_generation;
        Ok(texture_list.textures[index].insert(atlas_texture))
    }
}

struct MetalAtlasTexture {
    id: AtlasTextureInstanceId,
    allocator: BucketedAtlasAllocator,
    metal_texture: AssertSend<metal::Texture>,
    live_atlas_keys: u32,
    live_visual_leases: u32,
}

impl MetalAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let live_atlas_keys = self.live_atlas_keys.checked_add(1)?;
        let allocation = self.allocator.allocate(size_to_etagere(size))?;
        let tile = AtlasTile {
            texture_id: self.id.texture_id,
            tile_id: allocation.id.into(),
            bounds: Bounds {
                origin: point_from_etagere(allocation.rectangle.min),
                size,
            },
            padding: 0,
            texture_generation: self.id.generation,
            texture_generation_padding: 0,
        };
        self.live_atlas_keys = live_atlas_keys;
        Some(tile)
    }

    fn upload(&self, origin: Point<DevicePixels>, upload: &AtlasUpload<'_>) -> Result<()> {
        ensure!(
            self.id.texture_id.kind == upload.kind,
            "atlas upload kind does not match texture"
        );
        let pixel_format = match upload.kind {
            AtlasTextureKind::Monochrome => metal::MTLPixelFormat::A8Unorm,
            AtlasTextureKind::Polychrome => metal::MTLPixelFormat::BGRA8Unorm,
            AtlasTextureKind::Subpixel => {
                anyhow::bail!("Metal does not support subpixel atlas textures")
            }
        };
        ensure!(
            self.metal_texture.pixel_format() == pixel_format,
            "atlas upload pixel format does not match texture"
        );
        ensure!(
            origin.x.0 >= 0
                && origin.y.0 >= 0
                && (origin.x.0 as usize)
                    .checked_add(upload.size.width.0 as usize)
                    .is_some_and(|end| end <= self.metal_texture.width())
                && (origin.y.0 as usize)
                    .checked_add(upload.size.height.0 as usize)
                    .is_some_and(|end| end <= self.metal_texture.height()),
            "atlas upload is outside texture bounds"
        );
        let region = metal::region_2d(
            origin.x.into(),
            origin.y.into(),
            upload.size.width.into(),
            upload.size.height.into(),
        );
        self.metal_texture.replace_region(
            region,
            0,
            upload.bytes.as_ptr() as *const _,
            upload.bytes_per_row as u64,
        );
        Ok(())
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys -= 1;
    }

    fn release_visual_lease(&mut self) -> bool {
        let Some(next) = self.live_visual_leases.checked_sub(1) else {
            debug_assert!(false, "atlas visual lease count underflowed");
            return false;
        };
        self.live_visual_leases = next;
        true
    }

    fn is_unreferenced(&self) -> bool {
        self.live_atlas_keys == 0 && self.live_visual_leases == 0
    }
}

struct AtlasUpload<'a> {
    kind: AtlasTextureKind,
    size: Size<DevicePixels>,
    bytes: &'a [u8],
    bytes_per_row: usize,
}

impl<'a> AtlasUpload<'a> {
    fn new(kind: AtlasTextureKind, size: Size<DevicePixels>, bytes: &'a [u8]) -> Result<Self> {
        ensure!(
            size.width.0 > 0 && size.height.0 > 0,
            "atlas dimensions must be positive"
        );
        ensure!(
            size.width.0 <= MAX_ATLAS_DIMENSION && size.height.0 <= MAX_ATLAS_DIMENSION,
            "atlas dimensions exceed {MAX_ATLAS_DIMENSION}"
        );
        let bytes_per_pixel = match kind {
            AtlasTextureKind::Monochrome => 1,
            AtlasTextureKind::Polychrome => 4,
            AtlasTextureKind::Subpixel => {
                anyhow::bail!("Metal does not support subpixel atlas textures")
            }
        };
        let bytes_per_row = (size.width.0 as usize)
            .checked_mul(bytes_per_pixel)
            .context("calculating atlas row byte count")?;
        let byte_count = bytes_per_row
            .checked_mul(size.height.0 as usize)
            .context("calculating atlas byte count")?;
        ensure!(
            bytes.len() >= byte_count,
            "atlas upload byte length is {}, expected at least {byte_count}",
            bytes.len()
        );
        Ok(Self {
            kind,
            size,
            bytes,
            bytes_per_row,
        })
    }
}

fn size_to_etagere(size: Size<DevicePixels>) -> etagere::Size {
    etagere::Size::new(size.width.into(), size.height.into())
}

fn point_from_etagere(value: etagere::Point) -> Point<DevicePixels> {
    Point {
        x: DevicePixels::from(value.x),
        y: DevicePixels::from(value.y),
    }
}

#[derive(Deref, DerefMut)]
struct AssertSend<T>(T);

unsafe impl<T> Send for AssertSend<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::PlatformAtlas;
    use std::borrow::Cow;

    fn create_atlas() -> Option<MetalAtlas> {
        let device = metal::Device::system_default()?;
        let is_apple_gpu = device.supports_family(metal::MTLGPUFamily::Apple1);
        Some(MetalAtlas::new(device, is_apple_gpu))
    }

    fn make_image_key(image_id: usize, frame_index: usize) -> AtlasKey {
        AtlasKey::Image(open_gpui::RenderImageParams {
            image_id: open_gpui::ImageId(image_id),
            frame_index,
        })
    }

    fn insert_tile(atlas: &MetalAtlas, key: &AtlasKey, size: Size<DevicePixels>) -> AtlasTile {
        atlas
            .get_or_insert_with(key.clone(), &mut || {
                let byte_count = (size.width.0 as usize) * (size.height.0 as usize) * 4;
                Ok(Some((size, Cow::Owned(vec![0u8; byte_count]))))
            })
            .expect("allocation should succeed")
            .expect("callback returns Some")
    }

    #[test]
    fn test_remove_clears_stale_keys_from_tiles_by_key() {
        let Some(atlas) = create_atlas() else {
            return;
        };

        let small = Size {
            width: DevicePixels(64),
            height: DevicePixels(64),
        };

        let key_a = make_image_key(1, 0);
        let key_b = make_image_key(2, 0);
        let key_c = make_image_key(3, 0);

        let tile_a = insert_tile(&atlas, &key_a, small);
        let tile_b = insert_tile(&atlas, &key_b, small);
        let tile_c = insert_tile(&atlas, &key_c, small);

        assert_eq!(tile_a.texture_id, tile_b.texture_id);
        assert_eq!(tile_b.texture_id, tile_c.texture_id);

        // Remove A: texture still has B and C, so it stays.
        // The key for A must be removed from tiles_by_key.
        atlas.remove(&key_a);

        // Remove B: texture still has C.
        atlas.remove(&key_b);

        // Remove C: texture becomes unreferenced and is deleted.
        atlas.remove(&key_c);

        // Re-inserting A must allocate a fresh tile on a new texture,
        // NOT return a stale tile referencing the deleted texture.
        let tile_a2 = insert_tile(&atlas, &key_a, small);

        assert_eq!(tile_a.texture_id, tile_a2.texture_id);
        assert_ne!(tile_a.texture_instance(), tile_a2.texture_instance());

        // The exact replacement instance must exist — this would panic before the fix.
        assert!(atlas.metal_texture(tile_a2.texture_instance()).is_some());
    }

    #[test]
    fn test_remove_nonexistent_key_is_noop() {
        let Some(atlas) = create_atlas() else {
            return;
        };
        let key = make_image_key(999, 0);
        atlas.remove(&key);
    }
}

#[cfg(test)]
#[path = "metal_atlas/tests.rs"]
mod resource_tests;
