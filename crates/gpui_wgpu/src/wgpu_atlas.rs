use anyhow::{Context as _, Result};
use etagere::{BucketedAtlasAllocator, size2};
use open_gpui::{
    AtlasAccess, AtlasBackend, AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasState,
    AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind, AtlasTextureLease,
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

pub struct WgpuAtlas(Mutex<AtlasState<WgpuAtlasTextures>>);

struct PendingUpload {
    texture: AtlasTextureInstanceId,
    bounds: Bounds<DevicePixels>,
    bytes_per_row: u32,
    data: Vec<u8>,
}

struct WgpuAtlasTextures {
    epoch: AtlasTextureLeaseEpoch,
    next_texture_generation: u32,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    max_texture_size: u32,
    color_texture_format: wgpu::TextureFormat,
    storage: WgpuAtlasStorage,
    pending_uploads: Vec<PendingUpload>,
}

pub(crate) struct WgpuAtlasPresentationReceipt {
    texture_views: FxHashMap<AtlasTextureInstanceId, wgpu::TextureView>,
    _lease: AtlasTextureLease,
}

impl WgpuAtlasPresentationReceipt {
    pub(crate) fn texture_view(
        &self,
        texture: AtlasTextureInstanceId,
    ) -> Option<&wgpu::TextureView> {
        self.texture_views.get(&texture)
    }

    pub(crate) fn texture_views(
        &self,
    ) -> impl Iterator<Item = (AtlasTextureInstanceId, &wgpu::TextureView)> {
        self.texture_views
            .iter()
            .map(|(texture, view)| (*texture, view))
    }
}

impl WgpuAtlas {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        color_texture_format: wgpu::TextureFormat,
    ) -> Self {
        let max_texture_size = device.limits().max_texture_dimension_2d;
        WgpuAtlas(Mutex::new(AtlasState::new(WgpuAtlasTextures {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_texture_generation: 1,
            device,
            queue,
            max_texture_size,
            color_texture_format,
            storage: WgpuAtlasStorage::default(),
            pending_uploads: Vec::new(),
        })))
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
        lock.backend.flush_uploads();
    }

    pub(crate) fn prepare_presentation(
        self: &Arc<Self>,
        textures: impl IntoIterator<Item = AtlasTextureInstanceId>,
    ) -> std::result::Result<WgpuAtlasPresentationReceipt, AtlasTextureLeaseError> {
        let textures = textures.into_iter().collect::<Vec<_>>();
        let atlas: Arc<dyn PlatformAtlas> = self.clone();
        let lease = atlas.retain_texture_instances(&textures)?;
        let texture_views = {
            let lock = self.0.lock();
            lease
                .texture_instances()
                .iter()
                .copied()
                .map(|texture| {
                    lock.backend
                        .storage
                        .get(texture)
                        .map(|resident| (texture, resident.view.clone()))
                        .ok_or(AtlasTextureLeaseError::TextureUnavailable {
                            texture,
                            epoch: lock.backend.epoch,
                        })
                })
                .collect::<std::result::Result<_, _>>()?
        };
        Ok(WgpuAtlasPresentationReceipt {
            texture_views,
            _lease: lease,
        })
    }

    /// Clears all cached textures and tiles, forcing them to be recreated.
    /// Use this for incremental recovery when the device is still valid.
    pub fn clear(&self) {
        self.0.lock().clear(|backend| {
            backend.epoch = backend.epoch.next();
            backend.storage = WgpuAtlasStorage::default();
            backend.pending_uploads.clear();
        });
    }

    /// Handles device lost by clearing all textures and cached tiles.
    /// The atlas will lazily recreate textures as needed on subsequent frames.
    pub fn handle_device_lost(&self, context: &WgpuContext) {
        self.0.lock().clear(|backend| {
            backend.epoch = backend.epoch.next();
            backend.device = context.device.clone();
            backend.queue = context.queue.clone();
            backend.max_texture_size = context.device.limits().max_texture_dimension_2d;
            backend.color_texture_format = context.color_texture_format();
            backend.storage = WgpuAtlasStorage::default();
            backend.pending_uploads.clear();
        });
    }
}

impl PlatformAtlas for WgpuAtlas {
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

    #[cfg(any(test, feature = "test-support"))]
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
        let backend = &mut lock.backend;
        let epoch = backend.epoch;
        for texture in textures.iter().copied() {
            let Some(resident) = backend.storage.get(texture) else {
                return Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch });
            };
            if resident.live_visual_leases == u32::MAX {
                return Err(AtlasTextureLeaseError::LeaseCountOverflow { texture, epoch });
            }
        }
        for texture in textures.iter().copied() {
            backend
                .storage
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
        if lock.backend.epoch != epoch {
            return;
        }
        for texture in textures.iter().copied() {
            lock.backend.release_visual_lease(texture);
        }
    }
}

impl AtlasBackend for WgpuAtlasTextures {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> Result<AtlasTile> {
        anyhow::ensure!(
            size.width.0 > 0
                && size.height.0 > 0
                && size.width.0 as u32 <= self.max_texture_size
                && size.height.0 as u32 <= self.max_texture_size,
            "atlas upload dimensions must be within 1..={}",
            self.max_texture_size
        );
        let bytes_per_pixel = match kind {
            AtlasTextureKind::Monochrome => 1,
            AtlasTextureKind::Subpixel | AtlasTextureKind::Polychrome => 4,
        };
        let bytes_per_row = u32::try_from(size.width.0)?
            .checked_mul(bytes_per_pixel)
            .context("atlas upload row pitch overflow")?;
        let required_bytes = usize::try_from(bytes_per_row)?
            .checked_mul(usize::try_from(size.height.0)?)
            .context("atlas upload byte size overflow")?;
        anyhow::ensure!(
            bytes.len() >= required_bytes,
            "atlas upload source has {} bytes but requires {required_bytes}",
            bytes.len()
        );
        let tile = self.allocate(size, kind).context("failed to allocate")?;
        self.upload_texture(
            tile.texture_instance(),
            tile.bounds,
            bytes_per_row,
            &bytes[..required_bytes],
        );
        Ok(tile)
    }

    fn remove(&mut self, tile: AtlasTile) {
        self.remove_with_diagnostics(tile);
    }

    fn remove_with_diagnostics(&mut self, tile: AtlasTile) -> AtlasRemoveOutcome {
        let texture = tile.texture_instance();
        let id = texture.texture_id;
        let Some(slot) = self.storage[id.kind].textures.get_mut(id.index as usize) else {
            return AtlasRemoveOutcome::RemoveHit;
        };
        if slot.as_ref().is_none_or(|resident| resident.id != texture) {
            return AtlasRemoveOutcome::RemoveHit;
        }
        let mut resident = slot.take().expect("validated texture is resident");
        resident.decrement_ref_count();
        if resident.live_visual_leases == 0 {
            resident.allocator.deallocate(tile.tile_id.into());
        } else {
            resident.retired_tiles.push(tile);
        }
        if resident.is_unreferenced() {
            self.pending_uploads
                .retain(|upload| upload.texture != texture);
            self.storage[id.kind].free_list.push(id.index as usize);
            AtlasRemoveOutcome::TextureFreed
        } else {
            *slot = Some(resident);
            AtlasRemoveOutcome::TextureRetained
        }
    }
}

impl WgpuAtlasTextures {
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
        if resident.live_visual_leases == 0 {
            for tile in resident.retired_tiles.drain(..) {
                resident.allocator.deallocate(tile.tile_id.into());
            }
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
        let max_texture_size = i32::try_from(self.max_texture_size).unwrap_or(i32::MAX);
        let max_atlas_size = Size {
            width: DevicePixels(max_texture_size),
            height: DevicePixels(max_texture_size),
        };

        let size = min_size.max(&DEFAULT_ATLAS_SIZE).min(&max_atlas_size);
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
            retired_tiles: Vec::new(),
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
        bytes_per_row: u32,
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
            bytes_per_row,
            data,
        });
    }

    fn flush_uploads(&mut self) {
        for upload in self.pending_uploads.drain(..) {
            let Some(texture) = self.storage.get(upload.texture) else {
                continue;
            };
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
                    bytes_per_row: Some(upload.bytes_per_row),
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
    retired_tiles: Vec<AtlasTile>,
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
    use open_gpui::{FontId, GlyphId, ImageId, RenderGlyphParams, RenderImageParams, px};
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
                    default_queue: wgpu::QueueDescriptor::default(),
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
            .get_or_insert_with(key.clone(), &mut build)?
            .expect("test image should allocate an atlas tile");
        Ok((key, tile))
    }

    fn glyph_key(kind: AtlasTextureKind, identifier: u32) -> AtlasKey {
        AtlasKey::Glyph(RenderGlyphParams {
            font_id: FontId(0),
            glyph_id: GlyphId(identifier),
            font_size: px(14.0),
            subpixel_variant: Point { x: 0, y: 0 },
            scale_factor: 1.0,
            is_emoji: kind == AtlasTextureKind::Polychrome,
            subpixel_rendering: kind == AtlasTextureKind::Subpixel,
            dilation: 0,
        })
    }

    fn insert_source(
        atlas: &WgpuAtlas,
        key: AtlasKey,
        dimensions: Size<DevicePixels>,
        bytes: &[u8],
        diagnostics: bool,
    ) -> Result<Option<AtlasTile>> {
        let mut build = || Ok(Some((dimensions, Cow::Borrowed(bytes))));
        if diagnostics {
            atlas
                .get_or_insert_with_diagnostics(key, &mut build)
                .map(|access| access.tile)
        } else {
            atlas.get_or_insert_with(key, &mut build)
        }
    }

    fn assert_rejected_source_left_empty(atlas: &WgpuAtlas, key: &AtlasKey) {
        let state = atlas.0.lock();
        assert!(!state.contains(key));
        assert_eq!(state.backend.epoch, AtlasTextureLeaseEpoch::INITIAL);
        assert_eq!(state.backend.next_texture_generation, 1);
        assert!(state.backend.pending_uploads.is_empty());
        for kind in [
            AtlasTextureKind::Monochrome,
            AtlasTextureKind::Subpixel,
            AtlasTextureKind::Polychrome,
        ] {
            assert!(state.backend.storage[kind].textures.is_empty());
            assert!(state.backend.storage[kind].free_list.is_empty());
        }
    }

    #[test]
    fn malformed_sources_fail_before_allocation_and_allow_same_key_retry() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for kind in [
            AtlasTextureKind::Monochrome,
            AtlasTextureKind::Subpixel,
            AtlasTextureKind::Polychrome,
        ] {
            let channels = if kind == AtlasTextureKind::Monochrome {
                1
            } else {
                4
            };
            let dimensions = Size {
                width: DevicePixels(3),
                height: DevicePixels(2),
            };
            let source = vec![0x71; 6 * channels];
            for diagnostics in [false, true] {
                for short_len in [0, 3 * channels - 1, source.len() - 1] {
                    let atlas = WgpuAtlas::new(
                        device.clone(),
                        queue.clone(),
                        wgpu::TextureFormat::Rgba8Unorm,
                    );
                    let key = glyph_key(kind, 100);
                    assert!(
                        insert_source(
                            &atlas,
                            key.clone(),
                            dimensions,
                            &source[..short_len],
                            diagnostics,
                        )
                        .is_err()
                    );
                    assert_rejected_source_left_empty(&atlas, &key);
                    atlas.before_frame();
                    assert_rejected_source_left_empty(&atlas, &key);
                    let mut padded = source.clone();
                    padded.extend_from_slice(&[0x13, 0x29, 0x37]);
                    let tile =
                        insert_source(&atlas, key.clone(), dimensions, &padded, diagnostics)?
                            .expect("a complete same-key retry must insert");
                    assert_eq!(tile.texture_instance().generation, 1);
                    assert!(atlas.contains(&key));
                    {
                        let state = atlas.0.lock();
                        assert_eq!(state.backend.next_texture_generation, 2);
                        let pending = &state.backend.pending_uploads;
                        assert_eq!(pending.len(), 1);
                        assert_eq!(pending[0].texture, tile.texture_instance());
                        assert_eq!(pending[0].bytes_per_row, (3 * channels) as u32);
                        assert_eq!(pending[0].data.len(), source.len());
                    }
                    atlas.before_frame();
                    queue.submit(std::iter::empty());
                }
            }
        }
        assert!(block_on(scope.pop()).is_none());
        Ok(())
    }

    #[test]
    fn malformed_dimensions_and_row_pitch_do_not_mutate_atlas() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let limit = i32::try_from(device.limits().max_texture_dimension_2d)?;
        for diagnostics in [false, true] {
            for (width, height) in [
                (-1, 1),
                (0, 1),
                (1, -1),
                (1, 0),
                (limit + 1, 1),
                (1, limit + 1),
            ] {
                let atlas = WgpuAtlas::new(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Bgra8Unorm,
                );
                let key = glyph_key(AtlasTextureKind::Polychrome, 101);
                let error = insert_source(
                    &atlas,
                    key.clone(),
                    Size {
                        width: DevicePixels(width),
                        height: DevicePixels(height),
                    },
                    &[],
                    diagnostics,
                )
                .unwrap_err();
                assert!(error.to_string().contains("dimensions"));
                assert_rejected_source_left_empty(&atlas, &key);
                insert_source(
                    &atlas,
                    key,
                    Size {
                        width: DevicePixels(1),
                        height: DevicePixels(1),
                    },
                    &[0, 0, 255, 255],
                    diagnostics,
                )?
                .expect("valid dimensions must allow retry");
            }
            let atlas = WgpuAtlas::new(
                device.clone(),
                queue.clone(),
                wgpu::TextureFormat::Bgra8Unorm,
            );
            atlas.0.lock().backend.max_texture_size = u32::MAX;
            let key = glyph_key(AtlasTextureKind::Polychrome, 102);
            let error = insert_source(
                &atlas,
                key.clone(),
                Size {
                    width: DevicePixels(i32::MAX),
                    height: DevicePixels(1),
                },
                &[],
                diagnostics,
            )
            .unwrap_err();
            assert!(error.to_string().contains("row pitch overflow"));
            assert_rejected_source_left_empty(&atlas, &key);
        }
        Ok(())
    }

    #[test]
    fn source_rejection_preserves_resident_regions_pending_uploads_and_leases() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        for diagnostics in [false, true] {
            let atlas = Arc::new(WgpuAtlas::new(
                device.clone(),
                queue.clone(),
                wgpu::TextureFormat::Bgra8Unorm,
            ));
            let (keeper_key, keeper) = insert_image(
                &atlas,
                103,
                Size {
                    width: DevicePixels(1),
                    height: DevicePixels(1),
                },
            )?;
            let platform: Arc<dyn PlatformAtlas> = atlas.clone();
            let lease = platform.retain_texture_instances(&[keeper.texture_instance()])?;
            let snapshot = || {
                let state = atlas.0.lock();
                let backend = &state.backend;
                let resident = backend.storage.get(keeper.texture_instance()).unwrap();
                (
                    backend.epoch,
                    backend.next_texture_generation,
                    resident.live_atlas_keys,
                    resident.live_visual_leases,
                    resident.retired_tiles.len(),
                    backend
                        .pending_uploads
                        .iter()
                        .map(|upload| {
                            (
                                upload.texture,
                                upload.bounds,
                                upload.bytes_per_row,
                                upload.data.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            };
            let before = snapshot();
            let rejected_key = glyph_key(AtlasTextureKind::Polychrome, 104);
            assert!(
                insert_source(
                    &atlas,
                    rejected_key.clone(),
                    Size {
                        width: DevicePixels(2),
                        height: DevicePixels(2)
                    },
                    &[0, 0, 255],
                    diagnostics,
                )
                .is_err()
            );
            assert_eq!(snapshot(), before);
            assert!(atlas.contains(&keeper_key));
            assert!(!atlas.contains(&rejected_key));
            lease
                .validate()
                .expect("rejected upload must preserve the existing lease");
            let replacement = insert_source(
                &atlas,
                rejected_key,
                Size {
                    width: DevicePixels(2),
                    height: DevicePixels(2),
                },
                &[255; 16],
                diagnostics,
            )?
            .expect("the refused key must remain retryable");
            assert_eq!(replacement.texture_instance(), keeper.texture_instance());
            assert!(!replacement.bounds.intersects(&keeper.bounds));
        }
        Ok(())
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
            .get_or_insert_with(key.clone(), &mut build)?
            .expect("tile should be created");
        atlas.remove(&key);
        atlas.before_frame();
        Ok(())
    }

    #[test]
    fn remove_deallocates_unleased_tile_space_for_reuse() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = WgpuAtlas::new(device, queue, wgpu::TextureFormat::Bgra8Unorm);
        let (_, keeper) = insert_image(
            &atlas,
            2,
            Size {
                width: DevicePixels(64),
                height: DevicePixels(64),
            },
        )?;
        let dimensions = Size {
            width: DevicePixels(700),
            height: DevicePixels(700),
        };
        let (key, first) = insert_image(&atlas, 3, dimensions)?;
        assert_eq!(keeper.texture_instance(), first.texture_instance());
        atlas.remove(&key);
        let (_, replacement) = insert_image(&atlas, 4, dimensions)?;
        assert_eq!(replacement.texture_instance(), keeper.texture_instance());
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
    fn presentation_leases_are_atomic_deduplicated_and_release_without_submission()
    -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device,
            queue,
            wgpu::TextureFormat::Bgra8Unorm,
        ));
        let size = Size {
            width: DevicePixels(700),
            height: DevicePixels(700),
        };
        let (key, tile) = insert_image(&atlas, 42, size)?;
        let (_, keeper) = insert_image(
            &atlas,
            43,
            Size {
                width: DevicePixels(1),
                height: DevicePixels(1),
            },
        )?;
        let texture = tile.texture_instance();
        assert_eq!(keeper.texture_instance(), texture);
        let stale = AtlasTextureInstanceId {
            texture_id: texture.texture_id,
            generation: texture.generation + 1,
        };
        assert!(matches!(
            atlas.prepare_presentation([texture, texture, stale]),
            Err(AtlasTextureLeaseError::TextureUnavailable { texture, .. }) if texture == stale
        ));
        let lease_count = || {
            atlas
                .0
                .lock()
                .backend
                .storage
                .get(texture)
                .unwrap()
                .live_visual_leases
        };
        assert_eq!(lease_count(), 0);
        let receipt = atlas.prepare_presentation([texture, texture])?;
        assert_eq!(lease_count(), 1);
        assert_eq!(receipt.texture_views().count(), 1);
        assert_eq!(
            atlas.remove_with_diagnostics(&key).outcome,
            AtlasRemoveOutcome::TextureRetained,
        );
        {
            let state = atlas.0.lock();
            let resident = state.backend.storage.get(texture).unwrap();
            assert_eq!(resident.retired_tiles, [tile]);
        }
        drop(receipt);
        assert_eq!(lease_count(), 0);
        assert!(
            atlas
                .0
                .lock()
                .backend
                .storage
                .get(texture)
                .unwrap()
                .retired_tiles
                .is_empty()
        );
        let (_, reused) = insert_image(&atlas, 44, size)?;
        assert_eq!(reused.texture_instance(), texture);
        assert_eq!(reused.bounds, tile.bounds);
        Ok(())
    }

    #[test]
    fn presentation_receipt_keeps_texture_view_after_atlas_reset() -> anyhow::Result<()> {
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
        let atlas = Arc::new(WgpuAtlas::new(
            device,
            queue,
            wgpu::TextureFormat::Bgra8Unorm,
        ));
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
