use crate::metal_compat as metal;
use crate::metal_compat::Device;
use anyhow::{Context as _, Result};
use derive_more::{Deref, DerefMut};
use etagere::BucketedAtlasAllocator;
use open_gpui::{
    AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome, AtlasKey, AtlasRemoveDiagnostic,
    AtlasRemoveOutcome, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind,
    AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTextureList, AtlasTile, Bounds,
    DevicePixels, PlatformAtlas, Point, Size,
};
use open_gpui_collections::FxHashMap;
use parking_lot::Mutex;
use std::borrow::Cow;

pub(crate) struct MetalAtlas(Mutex<MetalAtlasState>);

impl MetalAtlas {
    pub(crate) fn new(device: Device, is_apple_gpu: bool) -> Self {
        MetalAtlas(Mutex::new(MetalAtlasState {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_texture_generation: 1,
            device: AssertSend(device),
            is_apple_gpu,
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
            tiles_by_key: Default::default(),
        }))
    }

    pub(crate) fn metal_texture(&self, texture: AtlasTextureInstanceId) -> metal::Texture {
        self.0.lock().texture(texture).metal_texture.clone()
    }
}

struct MetalAtlasState {
    epoch: AtlasTextureLeaseEpoch,
    next_texture_generation: u32,
    device: AssertSend<Device>,
    is_apple_gpu: bool,
    monochrome_textures: AtlasTextureList<MetalAtlasTexture>,
    polychrome_textures: AtlasTextureList<MetalAtlasTexture>,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
}

impl PlatformAtlas for MetalAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(*tile))
        } else {
            let Some((size, bytes)) = build()? else {
                return Ok(None);
            };
            let tile = lock
                .allocate(size, key.texture_kind())
                .context("failed to allocate")?;
            let texture = lock.texture(tile.texture_instance());
            texture.upload(tile.bounds, &bytes);
            lock.tiles_by_key.insert(key.clone(), tile);
            Ok(Some(tile))
        }
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<AtlasAccess> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(AtlasAccess {
                tile: Some(*tile),
                diagnostic: AtlasAccessDiagnostic::new(
                    key,
                    AtlasAccessOutcome::Hit,
                    Some(*tile),
                    Some(tile.bounds.size),
                ),
            })
        } else {
            let Some((size, bytes)) = build()? else {
                return Ok(AtlasAccess {
                    tile: None,
                    diagnostic: AtlasAccessDiagnostic::new(
                        key,
                        AtlasAccessOutcome::Unavailable,
                        None,
                        None,
                    ),
                });
            };
            let tile = lock
                .allocate(size, key.texture_kind())
                .context("failed to allocate")?;
            let texture = lock.texture(tile.texture_instance());
            texture.upload(tile.bounds, &bytes);
            lock.tiles_by_key.insert(key.clone(), tile);
            Ok(AtlasAccess {
                tile: Some(tile),
                diagnostic: AtlasAccessDiagnostic::new(
                    key,
                    AtlasAccessOutcome::Inserted,
                    Some(tile),
                    Some(size),
                ),
            })
        }
    }

    fn remove(&self, key: &AtlasKey) {
        let mut lock = self.0.lock();
        let Some(texture) = lock
            .tiles_by_key
            .remove(key)
            .map(AtlasTile::texture_instance)
        else {
            return;
        };
        let id = texture.texture_id;

        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &mut lock.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut lock.polychrome_textures,
            AtlasTextureKind::Subpixel => unreachable!(),
        };

        let Some(texture_slot) = textures.textures.get_mut(id.index as usize) else {
            return;
        };
        if !texture_slot
            .as_ref()
            .is_some_and(|resident| resident.id == texture)
        {
            return;
        }

        if let Some(mut texture) = texture_slot.take() {
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                textures.free_list.push(id.index as usize);
            } else {
                *texture_slot = Some(texture);
            }
        }
    }

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        let mut lock = self.0.lock();
        let Some(texture) = lock
            .tiles_by_key
            .remove(key)
            .map(AtlasTile::texture_instance)
        else {
            return AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveNoop, None);
        };
        let id = texture.texture_id;

        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &mut lock.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut lock.polychrome_textures,
            AtlasTextureKind::Subpixel => unreachable!(),
        };

        let Some(texture_slot) = textures.textures.get_mut(id.index as usize) else {
            return AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveHit, Some(id));
        };
        if !texture_slot
            .as_ref()
            .is_some_and(|resident| resident.id == texture)
        {
            return AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveHit, Some(id));
        }

        if let Some(mut texture) = texture_slot.take() {
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                textures.free_list.push(id.index as usize);
                AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::TextureFreed, Some(id))
            } else {
                *texture_slot = Some(texture);
                AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::TextureRetained, Some(id))
            }
        } else {
            AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveHit, Some(id))
        }
    }

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        self.0.lock().epoch
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
        if lock.epoch != epoch {
            return;
        }
        for texture in textures.iter().copied() {
            lock.release_visual_lease(texture);
        }
    }
}

impl MetalAtlasState {
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
    ) -> Option<AtlasTile> {
        {
            let textures = match texture_kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
                AtlasTextureKind::Subpixel => unreachable!(),
            };

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return Some(tile);
            }
        }

        let texture = self.push_texture(size, texture_kind);
        texture.allocate(size)
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> &mut MetalAtlasTexture {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        // Max texture size on all modern Apple GPUs. Anything bigger than that crashes in validateWithDevice.
        const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(16384),
            height: DevicePixels(16384),
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
            AtlasTextureKind::Subpixel => unreachable!(),
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
        let metal_texture = self.device.new_texture(&texture_descriptor);

        let generation = self.next_texture_generation;
        self.next_texture_generation = self
            .next_texture_generation
            .checked_add(1)
            .expect("metal atlas texture generation exhausted");

        let texture_list = match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Subpixel => unreachable!(),
        };

        let index = texture_list.free_list.pop();

        let atlas_texture = MetalAtlasTexture {
            id: AtlasTextureInstanceId {
                texture_id: AtlasTextureId {
                    index: index.unwrap_or(texture_list.textures.len()) as u32,
                    kind,
                },
                generation,
            },
            allocator: etagere::BucketedAtlasAllocator::new(size_to_etagere(size)),
            metal_texture: AssertSend(metal_texture),
            live_atlas_keys: 0,
            live_visual_leases: 0,
        };

        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list.textures.get_mut(ix)
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list.textures.last_mut()
        }
        .unwrap()
        .as_mut()
        .unwrap()
    }

    fn texture(&self, texture: AtlasTextureInstanceId) -> &MetalAtlasTexture {
        self.texture_if_resident(texture)
            .expect("texture must be resident after atlas allocation")
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
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn upload(&self, bounds: Bounds<DevicePixels>, bytes: &[u8]) {
        let region = metal::region_2d(
            bounds.origin.x.into(),
            bounds.origin.y.into(),
            bounds.size.width.into(),
            bounds.size.height.into(),
        );
        self.metal_texture.replace_region(
            region,
            0,
            bytes.as_ptr() as *const _,
            bounds.size.width.to_bytes(self.bytes_per_pixel()) as u64,
        );
    }

    fn bytes_per_pixel(&self) -> u8 {
        match self.metal_texture.pixel_format() {
            metal::MTLPixelFormat::A8Unorm | metal::MTLPixelFormat::R8Unorm => 1,
            metal::MTLPixelFormat::RGBA8Unorm | metal::MTLPixelFormat::BGRA8Unorm => 4,
            _ => unimplemented!(),
        }
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
        Some(MetalAtlas::new(device, true))
    }

    fn make_image_key(image_id: usize, frame_index: usize) -> AtlasKey {
        AtlasKey::Image(open_gpui::RenderImageParams {
            image_id: open_gpui::ImageId(image_id),
            frame_index,
        })
    }

    fn insert_tile(atlas: &MetalAtlas, key: &AtlasKey, size: Size<DevicePixels>) -> AtlasTile {
        atlas
            .get_or_insert_with(key, &mut || {
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
        let _texture = atlas.metal_texture(tile_a2.texture_instance());
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
