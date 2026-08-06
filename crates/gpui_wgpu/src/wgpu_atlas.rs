use anyhow::{Context as _, Result};
use etagere::{BucketedAtlasAllocator, size2};
use open_gpui::{
    AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome, AtlasKey, AtlasRemoveDiagnostic,
    AtlasRemoveOutcome, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind,
    AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTextureList, AtlasTile, Bounds,
    DevicePixels, PlatformAtlas, Point, Size,
};
use open_gpui_collections::FxHashMap;
use parking_lot::Mutex;
use std::{borrow::Cow, ops, sync::Arc};

use crate::WgpuContext;

fn device_size_to_etagere(size: Size<DevicePixels>) -> etagere::Size {
    size2(size.width.0, size.height.0)
}

fn etagere_point_to_device(point: etagere::Point) -> Point<DevicePixels> {
    Point {
        x: DevicePixels(point.x),
        y: DevicePixels(point.y),
    }
}

pub struct WgpuAtlas(Mutex<WgpuAtlasState>);

struct PendingUpload {
    texture: AtlasTextureInstanceId,
    bounds: Bounds<DevicePixels>,
    data: Vec<u8>,
}

struct WgpuAtlasState {
    epoch: AtlasTextureLeaseEpoch,
    next_texture_generation: u32,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    max_texture_size: u32,
    color_texture_format: wgpu::TextureFormat,
    storage: WgpuAtlasStorage,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    pending_uploads: Vec<PendingUpload>,
}

pub(crate) struct WgpuAtlasPresentationReceipt {
    texture_views: FxHashMap<AtlasTextureInstanceId, wgpu::TextureView>,
}

impl WgpuAtlasPresentationReceipt {
    pub(crate) fn texture_view(
        &self,
        texture: AtlasTextureInstanceId,
    ) -> Option<&wgpu::TextureView> {
        self.texture_views.get(&texture)
    }
}

impl WgpuAtlas {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        color_texture_format: wgpu::TextureFormat,
    ) -> Self {
        let max_texture_size = device.limits().max_texture_dimension_2d;
        WgpuAtlas(Mutex::new(WgpuAtlasState {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_texture_generation: 1,
            device,
            queue,
            max_texture_size,
            color_texture_format,
            storage: WgpuAtlasStorage::default(),
            tiles_by_key: Default::default(),
            pending_uploads: Vec::new(),
        }))
    }

    pub fn from_context(context: &WgpuContext) -> Self {
        Self::new(
            context.device.clone(),
            context.queue.clone(),
            context.color_texture_format(),
        )
    }

    pub fn before_frame(&self) {
        let mut lock = self.0.lock();
        lock.flush_uploads();
    }

    pub(crate) fn prepare_presentation(
        &self,
        textures: impl IntoIterator<Item = AtlasTextureInstanceId>,
    ) -> std::result::Result<WgpuAtlasPresentationReceipt, AtlasTextureLeaseError> {
        let lock = self.0.lock();
        let mut texture_views = FxHashMap::default();
        for texture in textures {
            if texture_views.contains_key(&texture) {
                continue;
            }
            let Some(resident) = lock.storage.get(texture) else {
                return Err(AtlasTextureLeaseError::TextureUnavailable {
                    texture,
                    epoch: lock.epoch,
                });
            };
            texture_views.insert(texture, resident.view.clone());
        }
        Ok(WgpuAtlasPresentationReceipt { texture_views })
    }

    /// Clears all cached textures and tiles, forcing them to be recreated.
    /// Use this for incremental recovery when the device is still valid.
    pub fn clear(&self) {
        let mut lock = self.0.lock();
        lock.epoch = lock.epoch.next();
        lock.storage = WgpuAtlasStorage::default();
        lock.tiles_by_key.clear();
        lock.pending_uploads.clear();
    }

    /// Handles device lost by clearing all textures and cached tiles.
    /// The atlas will lazily recreate textures as needed on subsequent frames.
    pub fn handle_device_lost(&self, context: &WgpuContext) {
        let mut lock = self.0.lock();
        lock.epoch = lock.epoch.next();
        lock.device = context.device.clone();
        lock.queue = context.queue.clone();
        lock.max_texture_size = context.device.limits().max_texture_dimension_2d;
        lock.color_texture_format = context.color_texture_format();
        lock.storage = WgpuAtlasStorage::default();
        lock.tiles_by_key.clear();
        lock.pending_uploads.clear();
    }
}

impl PlatformAtlas for WgpuAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(*tile))
        } else {
            profiling::scope!("new tile");
            let Some((size, bytes)) = build()? else {
                return Ok(None);
            };
            let tile = lock
                .allocate(size, key.texture_kind())
                .context("failed to allocate")?;
            lock.upload_texture(tile.texture_instance(), tile.bounds, &bytes);
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
            profiling::scope!("new tile");
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
            lock.upload_texture(tile.texture_instance(), tile.bounds, &bytes);
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

        let Some(texture_slot) = lock.storage[id.kind].textures.get_mut(id.index as usize) else {
            return;
        };
        if texture_slot
            .as_ref()
            .is_none_or(|resident| resident.id != texture)
        {
            return;
        }

        if let Some(mut resident) = texture_slot.take() {
            resident.decrement_ref_count();
            if resident.is_unreferenced() {
                lock.pending_uploads
                    .retain(|upload| upload.texture != resident.id);
                lock.storage[id.kind]
                    .free_list
                    .push(resident.id.texture_id.index as usize);
            } else {
                *texture_slot = Some(resident);
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

        let Some(texture_slot) = lock.storage[id.kind].textures.get_mut(id.index as usize) else {
            return AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveHit, Some(id));
        };
        if texture_slot
            .as_ref()
            .is_none_or(|resident| resident.id != texture)
        {
            return AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveHit, Some(id));
        }

        if let Some(mut resident) = texture_slot.take() {
            resident.decrement_ref_count();
            if resident.is_unreferenced() {
                lock.pending_uploads
                    .retain(|upload| upload.texture != resident.id);
                lock.storage[id.kind]
                    .free_list
                    .push(resident.id.texture_id.index as usize);
                AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::TextureFreed, Some(id))
            } else {
                *texture_slot = Some(resident);
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
            let Some(resident) = lock.storage.get(texture) else {
                return Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch });
            };
            if resident.live_visual_leases == u32::MAX {
                return Err(AtlasTextureLeaseError::LeaseCountOverflow { texture, epoch });
            }
        }
        for texture in textures.iter().copied() {
            lock.storage
                .get_mut(texture)
                .expect("validated atlas texture must remain resident while locked")
                .live_visual_leases += 1;
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

impl WgpuAtlasState {
    fn release_visual_lease(&mut self, texture: AtlasTextureInstanceId) {
        let Self {
            storage,
            pending_uploads,
            ..
        } = self;
        let id = texture.texture_id;
        let texture_list = &mut storage[id.kind];
        let Some(texture_slot) = texture_list.textures.get_mut(id.index as usize) else {
            return;
        };
        if texture_slot
            .as_ref()
            .is_none_or(|resident| resident.id != texture)
        {
            return;
        }
        let Some(mut resident) = texture_slot.take() else {
            return;
        };
        if !resident.release_visual_lease() {
            *texture_slot = Some(resident);
            return;
        }
        if resident.is_unreferenced() {
            pending_uploads.retain(|upload| upload.texture != resident.id);
            texture_list
                .free_list
                .push(resident.id.texture_id.index as usize);
        } else {
            *texture_slot = Some(resident);
        }
    }

    fn allocate(
        &mut self,
        size: Size<DevicePixels>,
        texture_kind: AtlasTextureKind,
    ) -> Option<AtlasTile> {
        {
            let textures = &mut self.storage[texture_kind];

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
    ) -> &mut WgpuAtlasTexture {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        let max_texture_size = self.max_texture_size as i32;
        let max_atlas_size = Size {
            width: DevicePixels(max_texture_size),
            height: DevicePixels(max_texture_size),
        };

        let size = min_size.min(&max_atlas_size).max(&DEFAULT_ATLAS_SIZE);
        let format = match kind {
            AtlasTextureKind::Monochrome => wgpu::TextureFormat::R8Unorm,
            AtlasTextureKind::Subpixel | AtlasTextureKind::Polychrome => self.color_texture_format,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d {
                width: size.width.0 as u32,
                height: size.height.0 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_list = &mut self.storage[kind];
        let index = texture_list.free_list.pop();
        let generation = self.next_texture_generation;
        self.next_texture_generation = self
            .next_texture_generation
            .checked_add(1)
            .expect("wgpu atlas texture generation exhausted");
        let texture_id = AtlasTextureId {
            index: index.unwrap_or(texture_list.textures.len()) as u32,
            kind,
        };

        let atlas_texture = WgpuAtlasTexture {
            id: AtlasTextureInstanceId {
                texture_id,
                generation,
            },
            allocator: BucketedAtlasAllocator::new(device_size_to_etagere(size)),
            format,
            texture,
            view,
            live_atlas_keys: 0,
            live_visual_leases: 0,
        };

        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list
                .textures
                .get_mut(ix)
                .and_then(|t| t.as_mut())
                .expect("texture must exist")
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list
                .textures
                .last_mut()
                .and_then(|t| t.as_mut())
                .expect("texture must exist")
        }
    }

    fn upload_texture(
        &mut self,
        texture: AtlasTextureInstanceId,
        bounds: Bounds<DevicePixels>,
        bytes: &[u8],
    ) {
        let data = self
            .storage
            .get(texture)
            .map(|texture| swizzle_upload_data(bytes, texture.format))
            .unwrap_or_else(|| bytes.to_vec());

        self.pending_uploads.push(PendingUpload {
            texture,
            bounds,
            data,
        });
    }

    fn flush_uploads(&mut self) {
        for upload in self.pending_uploads.drain(..) {
            let Some(texture) = self.storage.get(upload.texture) else {
                continue;
            };
            let bytes_per_pixel = texture.bytes_per_pixel();

            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: upload.bounds.origin.x.0 as u32,
                        y: upload.bounds.origin.y.0 as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(upload.bounds.size.width.0 as u32 * bytes_per_pixel as u32),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: upload.bounds.size.width.0 as u32,
                    height: upload.bounds.size.height.0 as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

#[derive(Default)]
struct WgpuAtlasStorage {
    monochrome_textures: AtlasTextureList<WgpuAtlasTexture>,
    subpixel_textures: AtlasTextureList<WgpuAtlasTexture>,
    polychrome_textures: AtlasTextureList<WgpuAtlasTexture>,
}

impl ops::Index<AtlasTextureKind> for WgpuAtlasStorage {
    type Output = AtlasTextureList<WgpuAtlasTexture>;
    fn index(&self, kind: AtlasTextureKind) -> &Self::Output {
        match kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Subpixel => &self.subpixel_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
        }
    }
}

impl ops::IndexMut<AtlasTextureKind> for WgpuAtlasStorage {
    fn index_mut(&mut self, kind: AtlasTextureKind) -> &mut Self::Output {
        match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        }
    }
}

impl WgpuAtlasStorage {
    fn get(&self, texture: AtlasTextureInstanceId) -> Option<&WgpuAtlasTexture> {
        let id = texture.texture_id;
        self[id.kind]
            .textures
            .get(id.index as usize)
            .and_then(|t| t.as_ref())
            .filter(|resident| resident.id == texture)
    }

    fn get_mut(&mut self, texture: AtlasTextureInstanceId) -> Option<&mut WgpuAtlasTexture> {
        let id = texture.texture_id;
        self[id.kind]
            .textures
            .get_mut(id.index as usize)
            .and_then(|slot| slot.as_mut())
            .filter(|resident| resident.id == texture)
    }
}

struct WgpuAtlasTexture {
    id: AtlasTextureInstanceId,
    allocator: BucketedAtlasAllocator,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    live_atlas_keys: u32,
    live_visual_leases: u32,
}

impl WgpuAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let allocation = self.allocator.allocate(device_size_to_etagere(size))?;
        let tile = AtlasTile {
            texture_id: self.id.texture_id,
            tile_id: allocation.id.into(),
            padding: 0,
            bounds: Bounds {
                origin: etagere_point_to_device(allocation.rectangle.min),
                size,
            },
            texture_generation: self.id.generation,
            texture_generation_padding: 0,
        };
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn bytes_per_pixel(&self) -> u8 {
        match self.format {
            wgpu::TextureFormat::R8Unorm => 1,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm => 4,
            _ => 4,
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

fn swizzle_upload_data(bytes: &[u8], format: wgpu::TextureFormat) -> Vec<u8> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => {
            let mut data = bytes.to_vec();
            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            data
        }
        _ => bytes.to_vec(),
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use open_gpui::block_on;
    use open_gpui::{ImageId, RenderImageParams};
    use std::sync::Arc;

    fn test_device_and_queue() -> anyhow::Result<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
        block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                flags: wgpu::InstanceFlags::default(),
                backend_options: wgpu::BackendOptions::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                display: None,
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|error| anyhow::anyhow!("failed to request adapter: {error}"))?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("wgpu_atlas_test_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits())
                        .using_alignment(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                })
                .await
                .map_err(|error| anyhow::anyhow!("failed to request device: {error}"))?;
            Ok((Arc::new(device), Arc::new(queue)))
        })
    }

    fn insert_image(
        atlas: &WgpuAtlas,
        image_id: usize,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<(AtlasKey, AtlasTile)> {
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(image_id),
            frame_index: 0,
        });
        let byte_count = (size.width.0 as usize) * (size.height.0 as usize) * 4;
        let mut build = || Ok(Some((size, Cow::Owned(vec![0; byte_count]))));
        let tile = atlas
            .get_or_insert_with(&key, &mut build)?
            .expect("test image should allocate an atlas tile");
        Ok((key, tile))
    }

    #[test]
    fn before_frame_skips_uploads_for_removed_texture() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;

        let atlas = WgpuAtlas::new(device, queue, wgpu::TextureFormat::Bgra8Unorm);
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(1),
            frame_index: 0,
        });
        let size = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let mut build = || Ok(Some((size, Cow::Owned(vec![0, 0, 0, 255]))));

        // Regression test: before the fix, this panicked in flush_uploads
        atlas
            .get_or_insert_with(&key, &mut build)?
            .expect("tile should be created");
        atlas.remove(&key);
        atlas.before_frame();
        Ok(())
    }

    #[test]
    fn texture_slot_reuse_advances_instance_generation() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device,
            queue,
            wgpu::TextureFormat::Bgra8Unorm,
        ));
        let size = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let (key_a, tile_a) = insert_image(&atlas, 10, size)?;

        assert_eq!(
            atlas.remove_with_diagnostics(&key_a).outcome,
            AtlasRemoveOutcome::TextureFreed
        );
        let (_, tile_b) = insert_image(&atlas, 11, size)?;

        assert_eq!(tile_a.texture_id, tile_b.texture_id);
        assert_ne!(tile_a.texture_instance(), tile_b.texture_instance());

        let platform_atlas: Arc<dyn PlatformAtlas> = atlas;
        assert!(matches!(
            platform_atlas
                .clone()
                .retain_texture_instances(&[tile_a.texture_instance()]),
            Err(AtlasTextureLeaseError::TextureUnavailable { texture, .. })
                if texture == tile_a.texture_instance()
        ));
        platform_atlas
            .retain_texture_instances(&[tile_b.texture_instance()])
            .expect("the replacement texture instance should be retainable");
        Ok(())
    }

    #[test]
    fn live_texture_lease_prevents_slot_reuse() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device,
            queue,
            wgpu::TextureFormat::Bgra8Unorm,
        ));
        let small = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let (key_a, tile_a) = insert_image(&atlas, 20, small)?;
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
        let lease = platform_atlas
            .retain_texture_instances(&[tile_a.texture_instance()])
            .expect("the source texture should be retainable");

        assert_eq!(
            atlas.remove_with_diagnostics(&key_a).outcome,
            AtlasRemoveOutcome::TextureRetained
        );
        let full = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        let (_, tile_b) = insert_image(&atlas, 21, full)?;
        assert_ne!(
            tile_a.texture_id, tile_b.texture_id,
            "a leased slot must not be reused for a replacement texture"
        );
        lease
            .validate()
            .expect("the original instance must remain live while leased");
        Ok(())
    }

    #[test]
    fn renderer_reset_does_not_reuse_texture_instance_identity() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device,
            queue,
            wgpu::TextureFormat::Bgra8Unorm,
        ));
        let size = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let (_, tile_a) = insert_image(&atlas, 30, size)?;
        let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
        let old_lease = platform_atlas
            .clone()
            .retain_texture_instances(&[tile_a.texture_instance()])
            .expect("the pre-reset texture should be retainable");

        atlas.clear();
        let (_, tile_b) = insert_image(&atlas, 31, size)?;

        assert_eq!(tile_a.texture_id, tile_b.texture_id);
        assert_ne!(tile_a.texture_instance(), tile_b.texture_instance());
        assert!(old_lease.validate().is_err());
        assert!(matches!(
            platform_atlas
                .clone()
                .retain_texture_instances(&[tile_a.texture_instance()]),
            Err(AtlasTextureLeaseError::TextureUnavailable { texture, .. })
                if texture == tile_a.texture_instance()
        ));
        let replacement_lease = platform_atlas
            .retain_texture_instances(&[tile_b.texture_instance()])
            .expect("the post-reset replacement should be retainable");
        drop(old_lease);
        replacement_lease
            .validate()
            .expect("dropping the old epoch lease must not affect its replacement");
        Ok(())
    }

    #[test]
    fn presentation_receipt_keeps_texture_view_after_atlas_reset() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = WgpuAtlas::new(device, queue, wgpu::TextureFormat::Bgra8Unorm);
        let size = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let (_, tile) = insert_image(&atlas, 40, size)?;
        let texture = tile.texture_instance();
        let receipt = atlas
            .prepare_presentation([texture])
            .expect("the resident texture should be presentable");

        atlas.clear();

        assert!(receipt.texture_view(texture).is_some());
        Ok(())
    }

    #[test]
    fn presentation_rejects_pre_reset_texture_instance() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = WgpuAtlas::new(device, queue, wgpu::TextureFormat::Bgra8Unorm);
        let size = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let (_, tile) = insert_image(&atlas, 41, size)?;
        let texture = tile.texture_instance();

        atlas.clear();

        assert!(matches!(
            atlas.prepare_presentation([texture]),
            Err(AtlasTextureLeaseError::TextureUnavailable {
                texture: unavailable,
                ..
            }) if unavailable == texture
        ));
        Ok(())
    }

    #[test]
    fn swizzle_upload_data_preserves_bgra_uploads() {
        let input = vec![0x10, 0x20, 0x30, 0x40];
        assert_eq!(
            swizzle_upload_data(&input, wgpu::TextureFormat::Bgra8Unorm),
            input
        );
    }

    #[test]
    fn swizzle_upload_data_converts_bgra_to_rgba() {
        let input = vec![0x10, 0x20, 0x30, 0x40, 0xAA, 0xBB, 0xCC, 0xDD];
        assert_eq!(
            swizzle_upload_data(&input, wgpu::TextureFormat::Rgba8Unorm),
            vec![0x30, 0x20, 0x10, 0x40, 0xCC, 0xBB, 0xAA, 0xDD]
        );
    }
}
