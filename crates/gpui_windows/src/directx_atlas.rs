use etagere::BucketedAtlasAllocator;
use open_gpui_collections::FxHashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
    },
    Dxgi::Common::*,
};

use open_gpui::{
    AtlasAccess, AtlasBackend, AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasState,
    AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind, AtlasTextureLease,
    AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTextureList, AtlasTile, Bounds,
    DevicePixels, PlatformAtlas, Point, PrimitiveBatch, Scene, Size,
};

pub(crate) struct DirectXAtlas(Mutex<AtlasState<DirectXAtlasTextures>>);

pub(crate) struct DirectXAtlasPreflight {
    lease: AtlasTextureLease,
    views: FxHashMap<AtlasTextureInstanceId, [Option<ID3D11ShaderResourceView>; 1]>,
}

impl DirectXAtlasPreflight {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if let Err(error) = self.lease.validate() {
            anyhow::bail!(
                "DirectX atlas preflight epoch changed from {} to {}",
                error.expected_epoch.get(),
                error.actual_epoch.get()
            );
        }
        Ok(())
    }

    pub(crate) fn texture_view(
        &self,
        texture: AtlasTextureInstanceId,
    ) -> Option<&[Option<ID3D11ShaderResourceView>; 1]> {
        self.views.get(&texture)
    }
}

const MAX_ATLAS_DIMENSION: i32 = 16384;

struct AtlasUpload {
    size: Size<DevicePixels>,
    bytes_per_pixel: u32,
    row_pitch: u32,
    required_bytes: usize,
}

impl AtlasUpload {
    fn for_source(
        size: Size<DevicePixels>,
        bytes_per_pixel: u32,
        byte_len: usize,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            size.width.0 > 0
                && size.height.0 > 0
                && size.width.0 <= MAX_ATLAS_DIMENSION
                && size.height.0 <= MAX_ATLAS_DIMENSION,
            "atlas upload dimensions must be within 1..={MAX_ATLAS_DIMENSION}"
        );
        anyhow::ensure!(
            matches!(bytes_per_pixel, 1 | 4),
            "atlas upload pixel format must use one or four bytes"
        );
        let row_pitch = u32::try_from(size.width.0)?
            .checked_mul(bytes_per_pixel)
            .ok_or_else(|| anyhow::anyhow!("atlas upload row pitch overflow"))?;
        let required_bytes = usize::try_from(row_pitch)?
            .checked_mul(usize::try_from(size.height.0)?)
            .ok_or_else(|| anyhow::anyhow!("atlas upload byte size overflow"))?;
        anyhow::ensure!(
            byte_len >= required_bytes,
            "atlas upload source has {byte_len} bytes but requires {required_bytes}"
        );
        Ok(Self {
            size,
            bytes_per_pixel,
            row_pitch,
            required_bytes,
        })
    }

    fn destination(
        &self,
        bounds: Bounds<DevicePixels>,
        texture_size: Size<DevicePixels>,
    ) -> anyhow::Result<D3D11_BOX> {
        anyhow::ensure!(
            bounds.size == self.size,
            "atlas upload destination size differs from its source"
        );
        let left = u32::try_from(bounds.origin.x.0)?;
        let top = u32::try_from(bounds.origin.y.0)?;
        let right = left
            .checked_add(u32::try_from(self.size.width.0)?)
            .ok_or_else(|| anyhow::anyhow!("atlas upload horizontal bounds overflow"))?;
        let bottom = top
            .checked_add(u32::try_from(self.size.height.0)?)
            .ok_or_else(|| anyhow::anyhow!("atlas upload vertical bounds overflow"))?;
        anyhow::ensure!(
            right <= u32::try_from(texture_size.width.0)?
                && bottom <= u32::try_from(texture_size.height.0)?,
            "atlas upload destination exceeds its texture"
        );
        Ok(D3D11_BOX {
            left,
            top,
            front: 0,
            right,
            bottom,
            back: 1,
        })
    }
}

struct DirectXAtlasTextures {
    epoch: AtlasTextureLeaseEpoch,
    next_texture_generation: u32,
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    monochrome_textures: AtlasTextureList<DirectXAtlasTexture>,
    polychrome_textures: AtlasTextureList<DirectXAtlasTexture>,
    subpixel_textures: AtlasTextureList<DirectXAtlasTexture>,
}

struct DirectXAtlasTexture {
    id: AtlasTextureInstanceId,
    bytes_per_pixel: u32,
    allocator: BucketedAtlasAllocator,
    texture: ID3D11Texture2D,
    view: [Option<ID3D11ShaderResourceView>; 1],
    live_atlas_keys: u32,
    live_visual_leases: u32,
}

impl DirectXAtlas {
    pub(crate) fn new(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Self {
        DirectXAtlas(Mutex::new(AtlasState::new(DirectXAtlasTextures {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_texture_generation: 1,
            device: device.clone(),
            device_context: device_context.clone(),
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
            subpixel_textures: Default::default(),
        })))
    }

    #[cfg(test)]
    pub(crate) fn get_texture_view(
        &self,
        texture: AtlasTextureInstanceId,
    ) -> Option<[Option<ID3D11ShaderResourceView>; 1]> {
        let lock = self.0.lock();
        lock.backend.get_texture_view(texture)
    }

    pub(crate) fn preflight(
        self: &Arc<Self>,
        scene: &Scene,
    ) -> anyhow::Result<DirectXAtlasPreflight> {
        let textures = scene
            .batches()
            .filter_map(|batch| match batch {
                PrimitiveBatch::MonochromeSprites { texture, .. }
                | PrimitiveBatch::SubpixelSprites { texture, .. }
                | PrimitiveBatch::PolychromeSprites { texture, .. } => Some(texture),
                _ => None,
            })
            .collect::<Vec<_>>();
        let atlas: Arc<dyn PlatformAtlas> = self.clone();
        let lease = atlas.retain_texture_instances(&textures)?;
        let views = {
            let state = self.0.lock();
            let backend = &state.backend;
            anyhow::ensure!(
                backend.epoch == lease.epoch(),
                "DirectX atlas epoch changed while preparing texture resources"
            );
            lease
                .texture_instances()
                .iter()
                .copied()
                .map(|texture| {
                    backend
                        .get_texture_view(texture)
                        .map(|view| (texture, view))
                        .ok_or(AtlasTextureLeaseError::TextureUnavailable {
                            texture,
                            epoch: backend.epoch,
                        })
                })
                .collect::<Result<FxHashMap<_, _>, _>>()?
        };
        Ok(DirectXAtlasPreflight { lease, views })
    }

    pub(crate) fn handle_device_lost(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
    ) {
        let mut lock = self.0.lock();
        lock.clear(|backend| {
            backend.epoch = backend.epoch.next();
            backend.device = device.clone();
            backend.device_context = device_context.clone();
            backend.monochrome_textures = AtlasTextureList::default();
            backend.polychrome_textures = AtlasTextureList::default();
            backend.subpixel_textures = AtlasTextureList::default();
        });
    }
}

impl PlatformAtlas for DirectXAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        self.0.lock().get_or_insert_with(key, build)
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<AtlasAccess> {
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
        let lock = &mut state.backend;
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
        let mut state = self.0.lock();
        let lock = &mut state.backend;
        if lock.epoch != epoch {
            return;
        }
        for texture in textures.iter().copied() {
            lock.release_visual_lease(texture);
        }
    }
}

impl AtlasBackend for DirectXAtlasTextures {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> anyhow::Result<AtlasTile> {
        let bytes_per_pixel = match kind {
            AtlasTextureKind::Monochrome => 1,
            AtlasTextureKind::Polychrome | AtlasTextureKind::Subpixel => 4,
        };
        let upload = AtlasUpload::for_source(size, bytes_per_pixel, bytes.len())?;
        let tile = self
            .allocate(size, kind)
            .ok_or_else(|| anyhow::anyhow!("failed to allocate atlas tile"))?;
        let result = self
            .texture_if_resident(tile.texture_instance())
            .ok_or_else(|| anyhow::anyhow!("allocated tile refers to a missing texture"))
            .and_then(|texture| texture.upload(&self.device_context, tile.bounds, bytes, upload));
        if let Err(error) = result {
            self.cancel_unpublished_allocation(tile);
            return Err(error);
        }
        Ok(tile)
    }

    fn remove(&mut self, tile: AtlasTile) {
        self.remove_with_diagnostics(tile);
    }

    fn remove_with_diagnostics(&mut self, tile: AtlasTile) -> AtlasRemoveOutcome {
        let texture = tile.texture_instance();
        let id = texture.texture_id;
        let textures = self.texture_list_mut(id.kind);
        let Some(texture_slot) = textures.textures.get_mut(id.index as usize) else {
            return AtlasRemoveOutcome::RemoveHit;
        };
        if texture_slot
            .as_ref()
            .is_none_or(|resident| resident.id != texture)
        {
            return AtlasRemoveOutcome::RemoveHit;
        }
        let Some(mut resident) = texture_slot.take() else {
            return AtlasRemoveOutcome::RemoveHit;
        };
        resident.decrement_ref_count();
        if resident.is_unreferenced() {
            textures.free_list.push(id.index as usize);
            AtlasRemoveOutcome::TextureFreed
        } else {
            *texture_slot = Some(resident);
            AtlasRemoveOutcome::TextureRetained
        }
    }
}

impl DirectXAtlasTextures {
    fn get_texture_view(
        &self,
        texture: AtlasTextureInstanceId,
    ) -> Option<[Option<ID3D11ShaderResourceView>; 1]> {
        let resident = self.texture_if_resident(texture)?;
        resident.view[0].as_ref()?;
        Some(resident.view.clone())
    }

    fn cancel_unpublished_allocation(&mut self, tile: AtlasTile) {
        let textures = self.texture_list_mut(tile.texture_id.kind);
        let slot = &mut textures.textures[tile.texture_id.index as usize];
        let resident = slot
            .as_mut()
            .expect("an unpublished allocation must retain its texture");
        assert_eq!(resident.id, tile.texture_instance());
        resident.allocator.deallocate(tile.tile_id.into());
        resident.decrement_ref_count();
        if resident.is_unreferenced() {
            slot.take();
            textures.free_list.push(tile.texture_id.index as usize);
        }
    }

    fn texture_list(&self, kind: AtlasTextureKind) -> &AtlasTextureList<DirectXAtlasTexture> {
        match kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
            AtlasTextureKind::Subpixel => &self.subpixel_textures,
        }
    }

    fn texture_list_mut(
        &mut self,
        kind: AtlasTextureKind,
    ) -> &mut AtlasTextureList<DirectXAtlasTexture> {
        match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
        }
    }

    fn texture_if_resident(&self, texture: AtlasTextureInstanceId) -> Option<&DirectXAtlasTexture> {
        let id = texture.texture_id;
        self.texture_list(id.kind)
            .textures
            .get(id.index as usize)
            .and_then(|slot| slot.as_ref())
            .filter(|resident| resident.id == texture)
    }

    fn texture_if_resident_mut(
        &mut self,
        texture: AtlasTextureInstanceId,
    ) -> Option<&mut DirectXAtlasTexture> {
        let id = texture.texture_id;
        self.texture_list_mut(id.kind)
            .textures
            .get_mut(id.index as usize)
            .and_then(|slot| slot.as_mut())
            .filter(|resident| resident.id == texture)
    }

    fn release_visual_lease(&mut self, texture: AtlasTextureInstanceId) {
        let id = texture.texture_id;
        let texture_list = self.texture_list_mut(id.kind);
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
            let textures = match texture_kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
                AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
            };

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return Some(tile);
            }
        }

        let texture = self.push_texture(size, texture_kind)?;
        texture.allocate(size)
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> Option<&mut DirectXAtlasTexture> {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        // Max texture size for DirectX. See:
        // https://learn.microsoft.com/en-us/windows/win32/direct3d11/overviews-direct3d-11-resources-limits
        const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(MAX_ATLAS_DIMENSION),
            height: DevicePixels(MAX_ATLAS_DIMENSION),
        };
        let size = min_size.min(&MAX_ATLAS_SIZE).max(&DEFAULT_ATLAS_SIZE);
        let pixel_format;
        let bind_flag;
        let bytes_per_pixel;
        match kind {
            AtlasTextureKind::Monochrome => {
                pixel_format = DXGI_FORMAT_R8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 1;
            }
            AtlasTextureKind::Polychrome => {
                pixel_format = DXGI_FORMAT_B8G8R8A8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 4;
            }
            AtlasTextureKind::Subpixel => {
                pixel_format = DXGI_FORMAT_R8G8B8A8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 4;
            }
        }
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: size.width.0 as u32,
            Height: size.height.0 as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: pixel_format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind_flag.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe {
            // This only returns None if the device is lost, which we will recreate later.
            // So it's ok to return None here.
            self.device
                .CreateTexture2D(&texture_desc, None, Some(&mut texture))
                .ok()?;
        }
        let texture = texture.unwrap();

        let view = unsafe {
            let mut view = None;
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut view))
                .ok()?;
            [view]
        };
        let texture_list = match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
        };
        let index = texture_list.free_list.pop();
        let generation = self.next_texture_generation;
        self.next_texture_generation = self
            .next_texture_generation
            .checked_add(1)
            .expect("DirectX atlas texture generation exhausted");
        let texture_id = AtlasTextureId {
            index: index.unwrap_or(texture_list.textures.len()) as u32,
            kind,
        };
        let atlas_texture = DirectXAtlasTexture {
            id: AtlasTextureInstanceId {
                texture_id,
                generation,
            },
            bytes_per_pixel,
            allocator: etagere::BucketedAtlasAllocator::new(device_size_to_etagere(size)),
            texture,
            view,
            live_atlas_keys: 0,
            live_visual_leases: 0,
        };
        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list.textures.get_mut(ix).unwrap().as_mut()
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list.textures.last_mut().unwrap().as_mut()
        }
    }
}

impl DirectXAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let allocation = self.allocator.allocate(device_size_to_etagere(size))?;
        let tile = AtlasTile {
            texture_id: self.id.texture_id,
            tile_id: allocation.id.into(),
            bounds: Bounds {
                origin: etagere_point_to_device(allocation.rectangle.min),
                size,
            },
            padding: 0,
            texture_generation: self.id.generation,
            texture_generation_padding: 0,
        };
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn upload(
        &self,
        device_context: &ID3D11DeviceContext,
        bounds: Bounds<DevicePixels>,
        bytes: &[u8],
        upload: AtlasUpload,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            upload.bytes_per_pixel == self.bytes_per_pixel,
            "atlas upload pixel format differs from its texture"
        );
        anyhow::ensure!(
            bytes.len() >= upload.required_bytes,
            "atlas upload source changed after admission"
        );
        let texture_size = self.allocator.size();
        let destination = upload.destination(
            bounds,
            Size {
                width: DevicePixels(texture_size.width),
                height: DevicePixels(texture_size.height),
            },
        )?;
        unsafe {
            device_context.UpdateSubresource(
                &self.texture,
                0,
                Some(&destination),
                bytes.as_ptr() as _,
                upload.row_pitch,
                0,
            );
        }
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

fn device_size_to_etagere(size: Size<DevicePixels>) -> etagere::Size {
    etagere::Size::new(size.width.into(), size.height.into())
}

fn etagere_point_to_device(value: etagere::Point) -> Point<DevicePixels> {
    Point {
        x: DevicePixels::from(value.x),
        y: DevicePixels::from(value.y),
    }
}

#[cfg(test)]
mod admission_tests;
#[cfg(test)]
mod upload_tests;
