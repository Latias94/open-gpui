use super::{AtlasBackend, AtlasState};
use crate::{
    AtlasAccess, AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasTextureId,
    AtlasTextureInstanceId, AtlasTextureKind, AtlasTextureLeaseEpoch, AtlasTextureLeaseError,
    AtlasTile, Bounds, DevicePixels, PlatformAtlas, Point, Size, TileId,
};
use anyhow::{Context as _, Result};
use open_gpui_collections::HashMap;
use parking_lot::Mutex;
use std::borrow::Cow;

/// A sprite atlas for windows without a GPU. It hands out uniquely identified
/// tiles without uploading any pixels, so glyph, SVG, and image painting can
/// run to completion in tests and headless platforms.
#[derive(Default)]
pub struct HeadlessAtlas(Mutex<AtlasState<HeadlessAtlasBackend>>);

#[doc(hidden)]
#[derive(Default)]
pub struct HeadlessAtlasBackend {
    epoch: AtlasTextureLeaseEpoch,
    next_id: u32,
    texture_refs: HashMap<AtlasTextureInstanceId, HeadlessAtlasTextureRefs>,
    #[cfg(test)]
    lease_acquire_calls: usize,
    #[cfg(test)]
    lease_release_calls: usize,
}

struct HeadlessAtlasTextureRefs {
    live_atlas_keys: u32,
    live_visual_leases: u32,
}

impl AtlasBackend for HeadlessAtlasBackend {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        _bytes: &[u8],
    ) -> Result<AtlasTile> {
        let texture_id = self
            .next_id
            .checked_add(1)
            .context("headless atlas identifier space exhausted")?;
        let tile_id = texture_id
            .checked_add(1)
            .context("headless atlas identifier space exhausted")?;
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind,
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: Bounds {
                origin: Point::default(),
                size,
            },
            texture_generation: texture_id,
            texture_generation_padding: 0,
        };
        self.texture_refs.insert(
            tile.texture_instance(),
            HeadlessAtlasTextureRefs {
                live_atlas_keys: 1,
                live_visual_leases: 0,
            },
        );
        self.next_id = tile_id;
        Ok(tile)
    }

    fn remove(&mut self, tile: AtlasTile) {
        self.remove_with_diagnostics(tile);
    }

    fn remove_with_diagnostics(&mut self, tile: AtlasTile) -> AtlasRemoveOutcome {
        let texture = tile.texture_instance();
        let remove_refs = self
            .texture_refs
            .get_mut(&texture)
            .is_some_and(|texture_refs| {
                if texture_refs.live_atlas_keys == 0 {
                    debug_assert!(false, "headless atlas key count underflowed");
                } else {
                    texture_refs.live_atlas_keys -= 1;
                }
                texture_refs.live_atlas_keys == 0 && texture_refs.live_visual_leases == 0
            });
        if remove_refs {
            self.texture_refs.remove(&texture);
        }
        if self.texture_refs.contains_key(&texture) {
            AtlasRemoveOutcome::TextureRetained
        } else {
            AtlasRemoveOutcome::TextureFreed
        }
    }
}

impl PlatformAtlas for HeadlessAtlas {
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
        let mut state = self.0.lock();
        let backend = &mut state.backend;
        #[cfg(test)]
        {
            backend.lease_acquire_calls += 1;
        }
        let epoch = backend.epoch;
        for texture in textures.iter().copied() {
            let Some(texture_refs) = backend.texture_refs.get(&texture) else {
                return Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch });
            };
            if texture_refs.live_visual_leases == u32::MAX {
                return Err(AtlasTextureLeaseError::LeaseCountOverflow { texture, epoch });
            }
        }
        for texture in textures.iter().copied() {
            if let Some(texture_refs) = backend.texture_refs.get_mut(&texture) {
                texture_refs.live_visual_leases += 1;
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
        let mut state = self.0.lock();
        let backend = &mut state.backend;
        #[cfg(test)]
        {
            backend.lease_release_calls += 1;
        }
        if backend.epoch != epoch {
            return;
        }
        for texture in textures.iter().copied() {
            backend.release_visual_lease(texture);
        }
    }
}

impl HeadlessAtlasBackend {
    fn release_visual_lease(&mut self, texture: AtlasTextureInstanceId) {
        let remove_refs = self
            .texture_refs
            .get_mut(&texture)
            .is_some_and(|texture_refs| {
                if texture_refs.live_visual_leases == 0 {
                    debug_assert!(false, "headless atlas visual lease count underflowed");
                } else {
                    texture_refs.live_visual_leases -= 1;
                }
                texture_refs.live_atlas_keys == 0 && texture_refs.live_visual_leases == 0
            });
        if remove_refs {
            self.texture_refs.remove(&texture);
        }
    }
}

#[cfg(test)]
mod tests;
