use super::{WgpuAtlas, WgpuAtlasTexture};
use anyhow::{Context as _, Result, ensure};
use open_gpui::{
    AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind, AtlasTextureLeaseEpoch, AtlasTile,
    Bounds, DevicePixels, ImageTexture, ImageTextureAlpha, ImageTextureSampling,
    ImageTextureTransfer, PlatformAtlas, Size, TileId,
};
use std::{
    any::Any,
    sync::{Arc, Weak},
};

/// Imports a host-owned color texture without reading back or copying its texels.
///
/// The texture must have been created by this atlas's actual device. The host owns ordering
/// producer writes before GPUI sampling and must not call `destroy` while an image, frame or
/// recording still uses the texture. The selected mip and array layer form one 2D view using
/// the texture's original format; format reinterpretation, depth, integer and multisampled
/// views are unsupported. RGBA/BGRA 8-bit unorm/sRGB and linear RGBA16Float are supported.
///
/// Transfer and alpha are explicit because an unorm texture does not describe its RGB content.
/// `transfer` describes stored RGB. An sRGB format must declare `Srgb`; its view automatically
/// decodes to linear samples. `alpha` describes the values after that sampling-time decoding.
/// The renderer unpremultiplies sampled values before converting linear samples to GPUI's
/// existing sRGB-numeric working values. This does not introduce linear-light UI composition,
/// tone mapping or wide-gamut color management.
#[derive(Clone)]
pub struct WgpuImportedImageDescriptor {
    /// The original host texture. Its handle is retained without a texel copy.
    pub texture: wgpu::Texture,
    /// Exactly one existing mip level to expose.
    pub mip_level: u32,
    /// Exactly one existing array layer to expose as a 2D view.
    pub array_layer: u32,
    /// The nonempty, integer texel region within the selected mip.
    ///
    /// Texture coordinates derive from that mip's full dimensions. Linear sampling clamps
    /// to the centers of the region's edge texels instead of sampling neighboring regions.
    pub region: Bounds<DevicePixels>,
    /// The transfer function of stored RGB values, before any sRGB view decoding.
    pub transfer: ImageTextureTransfer,
    /// The alpha convention of sampled values, after any sRGB view decoding.
    pub alpha: ImageTextureAlpha,
    /// The filtering policy at the explicitly selected mip level.
    pub sampling: ImageTextureSampling,
    /// An optional host allocation owner retained through the last recorded GPU use.
    ///
    /// This may keep a pooled render target or other host resource alive. Recording receipts
    /// retain it directly, including after the image handle is dropped and the atlas is reset.
    pub owner: Option<Arc<dyn Any + Send + Sync>>,
}

pub(super) struct ImportedImageResource {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _owner: Option<Arc<dyn Any + Send + Sync>>,
}

struct ImportedImageRegistration {
    atlas: Weak<WgpuAtlas>,
    epoch: AtlasTextureLeaseEpoch,
    texture: AtlasTextureInstanceId,
    _resource: Arc<ImportedImageResource>,
}

impl Drop for ImportedImageRegistration {
    fn drop(&mut self) {
        let Some(atlas) = self.atlas.upgrade() else {
            return;
        };
        let retired = {
            let mut state = atlas.0.lock();
            let backend = &mut state.backend;
            if backend.epoch != self.epoch {
                return;
            }
            let id = self.texture.texture_id;
            let list = &mut backend.storage[id.kind];
            let Some(slot) = list.textures.get_mut(id.index as usize) else {
                return;
            };
            let Some(resident) = slot.as_mut().filter(|resident| resident.id == self.texture)
            else {
                return;
            };
            resident.decrement_ref_count();
            if resident.is_unreferenced() {
                let retired = slot.take();
                list.free_list.push(id.index as usize);
                retired
            } else {
                None
            }
        };
        drop(retired);
    }
}

impl WgpuAtlas {
    /// Registers a selected host texture subresource for ordinary retained image painting.
    ///
    /// Known unsupported descriptor choices are refused before creating a view or mutating
    /// atlas state. The caller must supply a texture from this atlas's actual device, not
    /// merely a device with equivalent adapter properties or enabled features.
    pub fn import_image(
        self: &Arc<Self>,
        descriptor: WgpuImportedImageDescriptor,
    ) -> Result<ImageTexture> {
        let (tile, epoch, sampled_transfer, resource) = {
            let mut state = self.0.lock();
            let backend = &mut state.backend;
            let sampled_transfer = validate_descriptor(
                &descriptor,
                backend.device.features(),
                backend.max_texture_size,
            )?;
            let generation = backend.next_texture_generation;
            let next_generation = generation
                .checked_add(1)
                .context("wgpu atlas texture generation exhausted")?;
            let next_index = backend.storage[AtlasTextureKind::Polychrome]
                .free_list
                .last()
                .copied()
                .unwrap_or_else(|| backend.storage[AtlasTextureKind::Polychrome].textures.len());
            let index = u32::try_from(next_index).context("wgpu atlas texture slots exhausted")?;
            let view = descriptor
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("imported GPUI image"),
                    format: Some(descriptor.texture.format()),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: descriptor.mip_level,
                    mip_level_count: Some(1),
                    base_array_layer: descriptor.array_layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                });
            let texture = AtlasTextureInstanceId {
                texture_id: AtlasTextureId {
                    index,
                    kind: AtlasTextureKind::Polychrome,
                },
                generation,
            };
            let resource = Arc::new(ImportedImageResource {
                _texture: descriptor.texture.clone(),
                _view: view.clone(),
                _owner: descriptor.owner.clone(),
            });
            let resident = WgpuAtlasTexture {
                id: texture,
                allocator: None,
                texture: descriptor.texture.clone(),
                view,
                imported: Some(resource.clone()),
                format: descriptor.texture.format(),
                live_atlas_keys: 1,
                live_visual_leases: 0,
                retired_tiles: Vec::new(),
            };
            let textures = &mut backend.storage[AtlasTextureKind::Polychrome];
            if let Some(index) = textures.free_list.pop() {
                textures.textures[index] = Some(resident);
            } else {
                textures.textures.push(Some(resident));
            }
            backend.next_texture_generation = next_generation;
            (
                AtlasTile {
                    texture_id: texture.texture_id,
                    tile_id: TileId(0),
                    padding: 0,
                    bounds: descriptor.region,
                    texture_generation: texture.generation,
                    texture_generation_padding: 0,
                },
                backend.epoch,
                sampled_transfer,
                resource,
            )
        };
        let owner = Arc::new(ImportedImageRegistration {
            atlas: Arc::downgrade(self),
            epoch,
            texture: tile.texture_instance(),
            _resource: resource,
        });
        let atlas: Arc<dyn PlatformAtlas> = self.clone();
        ImageTexture::from_atlas(
            atlas,
            tile,
            sampled_transfer,
            descriptor.alpha,
            descriptor.sampling,
            owner,
        )
    }
}

fn validate_descriptor(
    descriptor: &WgpuImportedImageDescriptor,
    device_features: wgpu::Features,
    max_texture_size: u32,
) -> Result<ImageTextureTransfer> {
    let texture = &descriptor.texture;
    ensure!(
        texture.dimension() == wgpu::TextureDimension::D2 && texture.sample_count() == 1,
        "imported image must be a single-sample 2D texture"
    );
    ensure!(
        texture
            .usage()
            .contains(wgpu::TextureUsages::TEXTURE_BINDING),
        "imported image requires TEXTURE_BINDING usage"
    );
    let format = texture.format();
    ensure!(
        matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
                | wgpu::TextureFormat::Rgba16Float
        ),
        "unsupported imported image format: {format:?}"
    );
    ensure!(
        format.sample_type(Some(wgpu::TextureAspect::All), Some(device_features))
            == Some(wgpu::TextureSampleType::Float { filterable: true })
            && format
                .guaranteed_format_features(device_features)
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE),
        "imported image format must support the renderer's filterable float binding"
    );
    ensure!(
        descriptor.mip_level < texture.mip_level_count(),
        "imported image mip level is outside the texture"
    );
    ensure!(
        descriptor.array_layer < texture.depth_or_array_layers(),
        "imported image array layer is outside the texture"
    );
    let mip_size = Size {
        width: DevicePixels(i32::try_from(
            texture
                .width()
                .checked_shr(descriptor.mip_level)
                .unwrap_or(0)
                .max(1),
        )?),
        height: DevicePixels(i32::try_from(
            texture
                .height()
                .checked_shr(descriptor.mip_level)
                .unwrap_or(0)
                .max(1),
        )?),
    };
    ensure!(
        mip_size.width.0 as u32 <= max_texture_size && mip_size.height.0 as u32 <= max_texture_size,
        "imported image extent exceeds the atlas device limits"
    );
    let region = descriptor.region;
    ensure!(
        region.origin.x.0 >= 0
            && region.origin.y.0 >= 0
            && region.size.width.0 > 0
            && region.size.height.0 > 0
            && region
                .origin
                .x
                .0
                .checked_add(region.size.width.0)
                .is_some_and(|right| right <= mip_size.width.0)
            && region
                .origin
                .y
                .0
                .checked_add(region.size.height.0)
                .is_some_and(|bottom| bottom <= mip_size.height.0),
        "imported image region must be nonempty and contained in the selected mip"
    );
    ensure!(
        !format.has_srgb_suffix() || descriptor.transfer == ImageTextureTransfer::Srgb,
        "sRGB texture formats require sRGB stored RGB"
    );
    ensure!(
        format != wgpu::TextureFormat::Rgba16Float
            || descriptor.transfer == ImageTextureTransfer::Linear,
        "RGBA16Float imports require linear stored RGB; tone mapping is host-owned"
    );
    Ok(if format.has_srgb_suffix() {
        ImageTextureTransfer::Linear
    } else {
        descriptor.transfer
    })
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::wgpu_atlas::tests::test_device_and_queue;
    use open_gpui::{AtlasTextureLeaseError, ImageId, RenderImageParams, point, size};
    use std::{
        borrow::Cow,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    fn texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("host imported image test"),
            size: wgpu::Extent3d {
                width: 8,
                height: 4,
                depth_or_array_layers: 2,
            },
            mip_level_count: 3,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn descriptor(texture: wgpu::Texture) -> WgpuImportedImageDescriptor {
        WgpuImportedImageDescriptor {
            texture,
            mip_level: 1,
            array_layer: 1,
            region: Bounds::new(
                point(DevicePixels(1), DevicePixels(0)),
                size(DevicePixels(2), DevicePixels(2)),
            ),
            transfer: ImageTextureTransfer::Srgb,
            alpha: ImageTextureAlpha::Straight,
            sampling: ImageTextureSampling::Linear,
            owner: None,
        }
    }

    fn newest_instance(atlas: &WgpuAtlas) -> AtlasTextureInstanceId {
        atlas.0.lock().backend.storage[AtlasTextureKind::Polychrome]
            .textures
            .iter()
            .flatten()
            .max_by_key(|resident| resident.id.generation)
            .expect("test import must be resident")
            .id
    }

    fn assert_empty(atlas: &WgpuAtlas) {
        let state = atlas.0.lock();
        assert_eq!(state.backend.next_texture_generation, 1);
        assert!(state.backend.pending_uploads.is_empty());
        assert!(
            state.backend.storage[AtlasTextureKind::Polychrome]
                .textures
                .is_empty()
        );
        assert!(
            state.backend.storage[AtlasTextureKind::Polychrome]
                .free_list
                .is_empty()
        );
    }

    #[test]
    fn imported_mip_layer_uses_original_texture_without_upload_or_suballocation() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        let source = texture(
            &device,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let mut source_descriptor = descriptor(source.clone());
        source_descriptor.alpha = ImageTextureAlpha::Premultiplied;
        source_descriptor.sampling = ImageTextureSampling::Nearest;
        let image = atlas.import_image(source_descriptor)?;
        let instance = newest_instance(&atlas);
        assert_eq!(image.size(), size(DevicePixels(2), DevicePixels(2)));
        assert_eq!(image.sampled_transfer(), ImageTextureTransfer::Linear);
        assert_eq!(image.alpha(), ImageTextureAlpha::Premultiplied);
        assert_eq!(image.sampling(), ImageTextureSampling::Nearest);
        {
            let state = atlas.0.lock();
            assert!(state.backend.pending_uploads.is_empty());
            let resident = state.backend.storage.get(instance).unwrap();
            assert!(resident.allocator.is_none());
            assert_eq!(resident.texture, source);
            assert_eq!(resident.view.texture(), &source);
        }
        let key = RenderImageParams {
            image_id: ImageId(830),
            frame_index: 0,
        };
        let ordinary = atlas
            .get_or_insert_with(key.into(), &mut || {
                Ok(Some((
                    size(DevicePixels(1), DevicePixels(1)),
                    Cow::Borrowed(&[0, 0, 0, 255][..]),
                )))
            })?
            .unwrap();
        assert_ne!(ordinary.texture_instance(), instance);
        assert_eq!(atlas.0.lock().backend.pending_uploads.len(), 1);
        let receipt = atlas.prepare_presentation([instance])?;
        assert_eq!(receipt.texture_view(instance).unwrap().texture(), &source);
        Ok(())
    }

    #[test]
    fn unsupported_imports_refuse_before_views_or_atlas_changes_and_allow_retry() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let atlas = Arc::new(WgpuAtlas::new(
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        let valid = descriptor(texture(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ));
        let mut invalid = Vec::new();
        let mut bad = valid.clone();
        bad.mip_level = 3;
        invalid.push(bad);
        let mut bad = valid.clone();
        bad.array_layer = 2;
        invalid.push(bad);
        for region in [
            Bounds::new(point(DevicePixels(-1), DevicePixels(0)), valid.region.size),
            Bounds::new(valid.region.origin, size(DevicePixels(0), DevicePixels(1))),
            Bounds::new(valid.region.origin, size(DevicePixels(4), DevicePixels(2))),
            Bounds::new(valid.region.origin, size(DevicePixels(2), DevicePixels(3))),
            Bounds::new(
                point(DevicePixels(i32::MAX), DevicePixels(0)),
                valid.region.size,
            ),
        ] {
            let mut bad = valid.clone();
            bad.region = region;
            invalid.push(bad);
        }
        invalid.push(descriptor(texture(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::COPY_DST,
        )));
        for format in [wgpu::TextureFormat::R8Unorm, wgpu::TextureFormat::Rgba8Uint] {
            invalid.push(descriptor(texture(
                &device,
                format,
                wgpu::TextureUsages::TEXTURE_BINDING,
            )));
        }
        let mut bad = descriptor(texture(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ));
        bad.transfer = ImageTextureTransfer::Linear;
        invalid.push(bad);
        invalid.push(descriptor(texture(
            &device,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::TEXTURE_BINDING,
        )));
        for (dimension, sample_count) in [
            (wgpu::TextureDimension::D1, 1),
            (wgpu::TextureDimension::D2, 4),
            (wgpu::TextureDimension::D3, 1),
        ] {
            let mut bad = valid.clone();
            bad.texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("unsupported imported dimension or samples"),
                size: wgpu::Extent3d {
                    width: 4,
                    height: if dimension == wgpu::TextureDimension::D1 {
                        1
                    } else {
                        4
                    },
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: if sample_count > 1 {
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
                } else {
                    wgpu::TextureUsages::TEXTURE_BINDING
                },
                view_formats: &[],
            });
            invalid.push(bad);
        }
        for bad in invalid {
            assert!(atlas.import_image(bad).is_err());
            assert_empty(&atlas);
        }
        let old_maximum = atlas.0.lock().backend.max_texture_size;
        atlas.0.lock().backend.max_texture_size = 1;
        assert!(atlas.import_image(valid.clone()).is_err());
        assert_empty(&atlas);
        atlas.0.lock().backend.max_texture_size = old_maximum;
        atlas.0.lock().backend.next_texture_generation = u32::MAX;
        assert!(atlas.import_image(valid.clone()).is_err());
        atlas.0.lock().backend.next_texture_generation = 1;
        assert_empty(&atlas);
        atlas.import_image(valid)?;
        assert!(open_gpui::block_on(scope.pop()).is_none());
        Ok(())
    }

    #[test]
    fn supported_color_formats_normalize_only_sampling_time_transfer() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let atlas = Arc::new(WgpuAtlas::new(
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba16Float,
        ] {
            let mut source = descriptor(texture(
                &device,
                format,
                wgpu::TextureUsages::TEXTURE_BINDING,
            ));
            if format == wgpu::TextureFormat::Rgba16Float {
                source.transfer = ImageTextureTransfer::Linear;
            }
            let expected = if format.has_srgb_suffix() {
                ImageTextureTransfer::Linear
            } else {
                source.transfer
            };
            assert_eq!(atlas.import_image(source)?.sampled_transfer(), expected);
        }
        let mut linear_unorm = descriptor(texture(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ));
        linear_unorm.transfer = ImageTextureTransfer::Linear;
        assert_eq!(
            atlas.import_image(linear_unorm)?.sampled_transfer(),
            ImageTextureTransfer::Linear
        );
        assert!(open_gpui::block_on(scope.pop()).is_none());
        Ok(())
    }

    struct HostOwner {
        atlas: Weak<WgpuAtlas>,
        drops: Arc<AtomicUsize>,
        dropped_unlocked: Arc<AtomicBool>,
    }

    impl Drop for HostOwner {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
            let unlocked = self
                .atlas
                .upgrade()
                .is_none_or(|atlas| atlas.0.try_lock().is_some());
            self.dropped_unlocked.store(unlocked, Ordering::SeqCst);
        }
    }

    fn attach_owner(
        atlas: &Arc<WgpuAtlas>,
        descriptor: &mut WgpuImportedImageDescriptor,
    ) -> (Weak<HostOwner>, Arc<AtomicUsize>, Arc<AtomicBool>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let dropped_unlocked = Arc::new(AtomicBool::new(false));
        let owner = Arc::new(HostOwner {
            atlas: Arc::downgrade(atlas),
            drops: drops.clone(),
            dropped_unlocked: dropped_unlocked.clone(),
        });
        let weak = Arc::downgrade(&owner);
        descriptor.owner = Some(owner);
        (weak, drops, dropped_unlocked)
    }

    #[test]
    fn recording_keeps_original_host_owner_after_handle_drop_reset_and_atlas_drop() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        let source = texture(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let mut import = descriptor(source.clone());
        let (owner, drops, dropped_unlocked) = attach_owner(&atlas, &mut import);
        let image = atlas.import_image(import)?;
        let instance = newest_instance(&atlas);
        let recording = atlas.prepare_presentation([instance])?;
        drop(image);
        assert!(owner.upgrade().is_some());
        atlas.clear();
        assert!(owner.upgrade().is_some());
        assert_eq!(recording.texture_view(instance).unwrap().texture(), &source);
        drop(atlas);
        assert!(owner.upgrade().is_some());
        drop(recording);
        assert!(owner.upgrade().is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(dropped_unlocked.load(Ordering::SeqCst));
        Ok(())
    }

    #[test]
    fn image_clones_withdraw_once_and_visual_leases_block_slot_reuse() -> Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = Arc::new(WgpuAtlas::new(
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        let source = descriptor(texture(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ));
        let mut original = source.clone();
        let (owner, drops, dropped_unlocked) = attach_owner(&atlas, &mut original);
        let image = atlas.import_image(original)?;
        let clone = image.clone();
        let first = newest_instance(&atlas);
        let platform: Arc<dyn PlatformAtlas> = atlas.clone();
        let lease = platform.clone().retain_texture_instances(&[first])?;
        drop(image);
        assert_eq!(
            atlas
                .0
                .lock()
                .backend
                .storage
                .get(first)
                .unwrap()
                .live_atlas_keys,
            1
        );
        drop(clone);
        assert_eq!(
            atlas
                .0
                .lock()
                .backend
                .storage
                .get(first)
                .unwrap()
                .live_atlas_keys,
            0
        );
        assert!(owner.upgrade().is_some());
        let second_image = atlas.import_image(source.clone())?;
        let second = newest_instance(&atlas);
        assert_ne!(first.texture_id, second.texture_id);
        drop(lease);
        assert!(owner.upgrade().is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(dropped_unlocked.load(Ordering::SeqCst));
        let reused_image = atlas.import_image(source.clone())?;
        let reused = newest_instance(&atlas);
        assert_eq!(first.texture_id, reused.texture_id);
        assert_ne!(first, reused);
        assert!(matches!(
            platform.retain_texture_instances(&[first]),
            Err(AtlasTextureLeaseError::TextureUnavailable { texture, .. }) if texture == first
        ));
        let mut held_source = source.clone();
        let (held_owner, held_drops, held_unlocked) = attach_owner(&atlas, &mut held_source);
        let held_image = atlas.import_image(held_source)?;
        atlas.clear();
        assert!(second_image.validate().is_err());
        assert!(reused_image.validate().is_err());
        assert!(held_image.validate().is_err());
        assert!(held_owner.upgrade().is_some());
        drop(held_image);
        assert!(held_owner.upgrade().is_none());
        assert_eq!(held_drops.load(Ordering::SeqCst), 1);
        assert!(held_unlocked.load(Ordering::SeqCst));
        let current = atlas.import_image(source)?;
        let current_instance = newest_instance(&atlas);
        drop(second_image);
        drop(reused_image);
        current.validate().unwrap();
        assert_eq!(
            atlas
                .0
                .lock()
                .backend
                .storage
                .get(current_instance)
                .unwrap()
                .live_atlas_keys,
            1
        );
        Ok(())
    }
}
