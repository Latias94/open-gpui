use etagere::BucketedAtlasAllocator;
use open_gpui_collections::FxHashMap;
use parking_lot::Mutex;
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
    },
    Dxgi::Common::*,
};

use open_gpui::{
    AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome, AtlasKey, AtlasRemoveDiagnostic,
    AtlasRemoveOutcome, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind,
    AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTextureList, AtlasTile, Bounds,
    DevicePixels, PlatformAtlas, Point, Size,
};

pub(crate) struct DirectXAtlas(Mutex<DirectXAtlasState>);

struct DirectXAtlasState {
    epoch: AtlasTextureLeaseEpoch,
    next_texture_generation: u32,
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    monochrome_textures: AtlasTextureList<DirectXAtlasTexture>,
    polychrome_textures: AtlasTextureList<DirectXAtlasTexture>,
    subpixel_textures: AtlasTextureList<DirectXAtlasTexture>,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
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
        DirectXAtlas(Mutex::new(DirectXAtlasState {
            epoch: AtlasTextureLeaseEpoch::INITIAL,
            next_texture_generation: 1,
            device: device.clone(),
            device_context: device_context.clone(),
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
            subpixel_textures: Default::default(),
            tiles_by_key: Default::default(),
        }))
    }

    pub(crate) fn get_texture_view(
        &self,
        texture: AtlasTextureInstanceId,
    ) -> [Option<ID3D11ShaderResourceView>; 1] {
        let lock = self.0.lock();
        let tex = lock.texture(texture);
        tex.view.clone()
    }

    pub(crate) fn handle_device_lost(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
    ) {
        let mut lock = self.0.lock();
        lock.epoch = lock.epoch.next();
        lock.device = device.clone();
        lock.device_context = device_context.clone();
        lock.monochrome_textures = AtlasTextureList::default();
        lock.polychrome_textures = AtlasTextureList::default();
        lock.subpixel_textures = AtlasTextureList::default();
        lock.tiles_by_key.clear();
    }
}

impl PlatformAtlas for DirectXAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(*tile))
        } else {
            let Some((size, bytes)) = build()? else {
                return Ok(None);
            };
            let tile = lock
                .allocate(size, key.texture_kind())
                .ok_or_else(|| anyhow::anyhow!("failed to allocate"))?;
            let texture = lock.texture(tile.texture_instance());
            texture.upload(&lock.device_context, tile.bounds, &bytes);
            lock.tiles_by_key.insert(key.clone(), tile);
            Ok(Some(tile))
        }
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<AtlasAccess> {
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
                .ok_or_else(|| anyhow::anyhow!("failed to allocate"))?;
            let texture = lock.texture(tile.texture_instance());
            texture.upload(&lock.device_context, tile.bounds, &bytes);
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
            AtlasTextureKind::Subpixel => &mut lock.subpixel_textures,
        };

        let Some(texture_slot) = textures.textures.get_mut(id.index as usize) else {
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
                textures
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

        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &mut lock.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut lock.polychrome_textures,
            AtlasTextureKind::Subpixel => &mut lock.subpixel_textures,
        };

        let Some(texture_slot) = textures.textures.get_mut(id.index as usize) else {
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
                textures
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

impl DirectXAtlasState {
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
            width: DevicePixels(16384),
            height: DevicePixels(16384),
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
        let view = unsafe {
            let mut view = None;
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut view))
                .ok()?;
            [view]
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

    fn texture(&self, texture: AtlasTextureInstanceId) -> &DirectXAtlasTexture {
        self.texture_if_resident(texture)
            .expect("texture must be resident after atlas allocation")
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
    ) {
        unsafe {
            device_context.UpdateSubresource(
                &self.texture,
                0,
                Some(&D3D11_BOX {
                    left: bounds.left().0 as u32,
                    top: bounds.top().0 as u32,
                    front: 0,
                    right: bounds.right().0 as u32,
                    bottom: bounds.bottom().0 as u32,
                    back: 1,
                }),
                bytes.as_ptr() as _,
                bounds.size.width.to_bytes(self.bytes_per_pixel as u8),
                0,
            );
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

fn device_size_to_etagere(size: Size<DevicePixels>) -> etagere::Size {
    etagere::Size::new(size.width.into(), size.height.into())
}

fn etagere_point_to_device(value: etagere::Point) -> Point<DevicePixels> {
    Point {
        x: DevicePixels::from(value.x),
        y: DevicePixels::from(value.y),
    }
}
