use super::{GpuData, INSTANCE_TEXTURE_TEXEL_SIZE, MAX_INSTANCE_BUFFER_SIZE};

impl GpuData {
    pub(super) fn create(
        device: &wgpu::Device,
        label: &str,
        requested_capacity: u64,
        texture_transport: bool,
    ) -> anyhow::Result<(Self, u64)> {
        if !texture_transport {
            return Ok((
                Self::Storage(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: requested_capacity,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })),
                requested_capacity,
            ));
        }
        let maximum = u64::from(device.limits().max_texture_dimension_2d);
        anyhow::ensure!(maximum > 0, "adapter has no instance texture capacity");
        let texels = requested_capacity
            .div_ceil(INSTANCE_TEXTURE_TEXEL_SIZE)
            .max(1);
        anyhow::ensure!(
            texels <= maximum * maximum,
            "packed data requires {texels} texels, exceeding adapter texture limits"
        );
        let width = texels.min(maximum) as u32;
        let height = texels.div_ceil(u64::from(width)) as u32;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let capacity = (u64::from(width) * u64::from(height) * INSTANCE_TEXTURE_TEXEL_SIZE)
            .min(Self::maximum_capacity(device, true));
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok((
            Self::Texture {
                texture,
                view,
                width,
                height,
            },
            capacity,
        ))
    }

    pub(super) fn maximum_capacity(device: &wgpu::Device, texture_transport: bool) -> u64 {
        if texture_transport {
            let dimension = u64::from(device.limits().max_texture_dimension_2d);
            dimension
                .saturating_mul(dimension)
                .saturating_mul(INSTANCE_TEXTURE_TEXEL_SIZE)
                .min(MAX_INSTANCE_BUFFER_SIZE)
        } else {
            device
                .limits()
                .max_buffer_size
                .min(u64::from(device.limits().max_storage_buffer_binding_size))
                .min(MAX_INSTANCE_BUFFER_SIZE)
        }
    }

    pub(super) fn binding(&self, offset: u64, size: u64) -> wgpu::BindingResource<'_> {
        match self {
            Self::Storage(buffer) => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset,
                size: std::num::NonZeroU64::new(size),
            }),
            Self::Texture { view, .. } => wgpu::BindingResource::TextureView(view),
        }
    }

    pub(super) fn write(
        &self,
        queue: &wgpu::Queue,
        offset: u64,
        data: &[u8],
    ) -> anyhow::Result<()> {
        match self {
            Self::Storage(buffer) => {
                anyhow::ensure!(
                    offset
                        .checked_add(data.len() as u64)
                        .is_some_and(|end| end <= buffer.size()),
                    "GPU storage write exceeds allocation"
                );
                if !data.is_empty() {
                    queue.write_buffer(buffer, offset, data);
                }
            }
            Self::Texture {
                texture,
                width,
                height,
                ..
            } => {
                anyhow::ensure!(
                    offset % INSTANCE_TEXTURE_TEXEL_SIZE == 0,
                    "packed GPU data must begin at a texel boundary"
                );
                let allocation = (data.len() as u64).next_multiple_of(INSTANCE_TEXTURE_TEXEL_SIZE);
                let capacity = u64::from(*width) * u64::from(*height) * INSTANCE_TEXTURE_TEXEL_SIZE;
                anyhow::ensure!(
                    offset
                        .checked_add(allocation)
                        .is_some_and(|end| end <= capacity),
                    "packed GPU data write exceeds allocation"
                );
                let mut byte_offset = 0;
                let mut texel_offset = offset / INSTANCE_TEXTURE_TEXEL_SIZE;
                while byte_offset < data.len() {
                    let column = (texel_offset % u64::from(*width)) as u32;
                    let row = (texel_offset / u64::from(*width)) as u32;
                    let remaining = data.len() - byte_offset;
                    let whole_texels = (remaining as u64 / INSTANCE_TEXTURE_TEXEL_SIZE)
                        .min(u64::from(*width - column));
                    let mut tail = [0; INSTANCE_TEXTURE_TEXEL_SIZE as usize];
                    let (bytes, texels) = if whole_texels == 0 {
                        tail[..remaining].copy_from_slice(&data[byte_offset..]);
                        (tail.as_slice(), 1)
                    } else {
                        let length = (whole_texels * INSTANCE_TEXTURE_TEXEL_SIZE) as usize;
                        (
                            &data[byte_offset..byte_offset + length],
                            whole_texels as u32,
                        )
                    };
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: column,
                                y: row,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytes,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texels * INSTANCE_TEXTURE_TEXEL_SIZE as u32),
                            rows_per_image: Some(1),
                        },
                        wgpu::Extent3d {
                            width: texels,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                    );
                    byte_offset += remaining.min(bytes.len());
                    texel_offset += u64::from(texels);
                }
            }
        }
        Ok(())
    }
}

pub(super) fn instance_allocation(
    offset: u64,
    byte_size: u64,
    stride: u64,
    alignment: u64,
    texture_transport: bool,
) -> anyhow::Result<(u64, u64, u32)> {
    let size = byte_size.max(INSTANCE_TEXTURE_TEXEL_SIZE);
    let stride = stride.max(1);
    let alignment = if texture_transport {
        let mut divisor = alignment.max(1);
        let mut remainder = stride;
        while remainder != 0 {
            let next = divisor % remainder;
            divisor = remainder;
            remainder = next;
        }
        (alignment.max(1) / divisor)
            .checked_mul(stride)
            .ok_or_else(|| anyhow::anyhow!("packed instance alignment overflow"))?
    } else {
        alignment.max(1)
    };
    let offset = offset
        .checked_next_multiple_of(alignment)
        .ok_or_else(|| anyhow::anyhow!("instance offset overflow"))?;
    let allocation = if texture_transport {
        size.checked_next_multiple_of(INSTANCE_TEXTURE_TEXEL_SIZE)
            .ok_or_else(|| anyhow::anyhow!("packed instance size overflow"))?
    } else {
        size
    };
    let first_instance = if texture_transport {
        u32::try_from(offset / stride)?
    } else {
        0
    };
    Ok((offset, allocation, first_instance))
}
