use crate::{
    AtlasTextureKind, AtlasTextureLease, AtlasTextureLeaseEpoch, AtlasTextureLeaseInvalidation,
    AtlasTile, DevicePixels, PlatformAtlas, Size,
};
use std::{any::Any, fmt, rc::Rc, sync::Arc};

/// The transfer function of RGB values stored in, or sampled from, an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageTextureTransfer {
    /// Values use the sRGB transfer function.
    Srgb,
    /// Values are linear-light RGB with sRGB primaries.
    Linear,
}

/// How an image's sampled RGB values relate to its sampled alpha.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageTextureAlpha {
    /// Sampled RGB is independent of alpha.
    Straight,
    /// Sampled RGB has already been multiplied by sampled alpha in the sampled value domain.
    ///
    /// For an sRGB-decoding view, the stored RGB must therefore be the sRGB encoding of
    /// linear-light premultiplied RGB. An unorm view can instead expose RGB premultiplied
    /// in the sRGB-encoded domain. The renderer unpremultiplies before transfer conversion.
    Premultiplied,
    /// Ignore sampled alpha and treat the image as fully opaque.
    Opaque,
}

/// The sampling policy used within an imported image's texel region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageTextureSampling {
    /// Bilinear sampling at the explicitly selected mip level, clamped to the texel region.
    Linear,
    /// Nearest-texel sampling at the explicitly selected mip level.
    Nearest,
}

/// A renderer-local retained image that can participate in normal GPUI paint order.
///
/// Clones share one allocation. The backend allocation and its owner remain alive until the
/// final handle and all frame/recording leases release them. Atlas resets invalidate new paint
/// with this handle; already-recorded GPU work retains its original resources independently.
/// This retains resource identity, not a snapshot of mutable GPU pixel contents.
#[derive(Clone)]
pub struct ImageTexture {
    inner: Rc<ImageTextureInner>,
}

struct ImageTextureInner {
    atlas: Arc<dyn PlatformAtlas>,
    tile: AtlasTile,
    sampled_transfer: ImageTextureTransfer,
    alpha: ImageTextureAlpha,
    sampling: ImageTextureSampling,
    lease: AtlasTextureLease,
    _owner: Arc<dyn Any>,
}

impl ImageTexture {
    /// Creates a retained image from a backend's registered allocation.
    ///
    /// The backend must have registered the exact tile in `atlas` and retain the allocation
    /// through `owner`. The owner may withdraw that registration on its final drop; frame
    /// leases then keep existing paint alive. `sampled_transfer` describes values returned
    /// by texture sampling, including any automatic decoding performed by the texture view.
    /// The backend renderer must implement imported-image region, filtering, transfer and
    /// alpha semantics; registering an ordinary atlas tile does not add that support.
    #[doc(hidden)]
    pub fn from_atlas(
        atlas: Arc<dyn PlatformAtlas>,
        tile: AtlasTile,
        sampled_transfer: ImageTextureTransfer,
        alpha: ImageTextureAlpha,
        sampling: ImageTextureSampling,
        owner: Arc<dyn Any>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            tile.texture_id.kind == AtlasTextureKind::Polychrome
                && tile.bounds.origin.x.0 >= 0
                && tile.bounds.origin.y.0 >= 0
                && tile.bounds.size.width.0 > 0
                && tile.bounds.size.height.0 > 0
                && tile
                    .bounds
                    .origin
                    .x
                    .0
                    .checked_add(tile.bounds.size.width.0)
                    .is_some()
                && tile
                    .bounds
                    .origin
                    .y
                    .0
                    .checked_add(tile.bounds.size.height.0)
                    .is_some()
                && tile.padding == 0
                && tile.texture_generation_padding == 0,
            "image textures require a nonempty unpadded polychrome texel region"
        );
        let lease = atlas
            .clone()
            .retain_texture_instances(&[tile.texture_instance()])?;
        Ok(Self {
            inner: Rc::new(ImageTextureInner {
                atlas,
                tile,
                sampled_transfer,
                alpha,
                sampling,
                lease,
                _owner: owner,
            }),
        })
    }

    /// Returns the dimensions of the selected texel region.
    pub fn size(&self) -> Size<DevicePixels> {
        self.inner.tile.bounds.size
    }

    /// Returns the transfer function after the backend's sampling-time decoding.
    pub fn sampled_transfer(&self) -> ImageTextureTransfer {
        self.inner.sampled_transfer
    }

    /// Returns the sampled alpha convention.
    pub fn alpha(&self) -> ImageTextureAlpha {
        self.inner.alpha
    }

    /// Returns the sampling policy.
    pub fn sampling(&self) -> ImageTextureSampling {
        self.inner.sampling
    }

    /// Returns the atlas lifetime in which this image was registered.
    pub fn epoch(&self) -> AtlasTextureLeaseEpoch {
        self.inner.lease.epoch()
    }

    /// Refuses a handle invalidated by an atlas reset.
    pub fn validate(&self) -> Result<(), AtlasTextureLeaseInvalidation> {
        self.inner.lease.validate()
    }

    pub(crate) fn tile(&self) -> AtlasTile {
        self.inner.tile
    }

    pub(crate) fn belongs_to(&self, atlas: &Arc<dyn PlatformAtlas>) -> bool {
        Arc::ptr_eq(&self.inner.atlas, atlas)
    }

    pub(crate) fn shader_flags(&self) -> u32 {
        16 | u32::from(self.sampled_transfer() == ImageTextureTransfer::Linear)
            | (u32::from(self.alpha() == ImageTextureAlpha::Premultiplied) << 1)
            | (u32::from(self.alpha() == ImageTextureAlpha::Opaque) << 2)
            | (u32::from(self.sampling() == ImageTextureSampling::Nearest) << 3)
    }
}

impl fmt::Debug for ImageTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageTexture")
            .field("tile", &self.inner.tile)
            .field("epoch", &self.epoch())
            .field("sampled_transfer", &self.sampled_transfer())
            .field("alpha", &self.alpha())
            .field("sampling", &self.sampling())
            .finish_non_exhaustive()
    }
}
