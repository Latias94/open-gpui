use super::*;

const INSTANCE_BUFFER_ALIGNMENT: usize = 256;
const MAX_INSTANCE_BUFFER_SIZE: usize = 256 * 1024 * 1024;

pub(super) struct InstanceBinding {
    pub buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    pub offset: usize,
}

pub(super) struct InstanceBindings {
    pub clip_shapes: InstanceBinding,
    pub quads: InstanceBinding,
    pub shadows: InstanceBinding,
    pub underlines: InstanceBinding,
    pub monochrome_sprites: InstanceBinding,
    pub polychrome_sprites: InstanceBinding,
    pub surfaces: InstanceBinding,
}

pub(super) fn write_instances(
    scene: &Scene,
    writer: &mut InstanceBufferWriter,
) -> Result<InstanceBindings> {
    Ok(InstanceBindings {
        clip_shapes: writer.write(scene.clip_shapes())?,
        quads: writer.write(&scene.quads)?,
        shadows: writer.write(&scene.shadows)?,
        underlines: writer.write(&scene.underlines)?,
        monochrome_sprites: writer.write(&scene.monochrome_sprites)?,
        polychrome_sprites: writer.write(&scene.polychrome_sprites)?,
        surfaces: writer.write_surface_bounds(&scene.surfaces)?,
    })
}

pub(super) struct InstanceBufferWriter {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    pool: Arc<Mutex<InstanceBufferPool>>,
    unified_memory: bool,
    filled: Vec<(InstanceBuffer, usize)>,
    current: InstanceBuffer,
    offset: usize,
}

impl InstanceBufferWriter {
    pub(super) fn new(
        device: &ProtocolObject<dyn MTLDevice>,
        pool: &Arc<Mutex<InstanceBufferPool>>,
        unified_memory: bool,
    ) -> Result<Self> {
        let current = pool.lock().acquire(device, unified_memory)?;
        Ok(Self {
            device: device.retain(),
            pool: pool.clone(),
            unified_memory,
            filled: Vec::new(),
            current,
            offset: 0,
        })
    }

    fn reserve(&mut self, size: usize) -> Result<(InstanceBinding, *mut u8)> {
        let mut offset = self.offset.next_multiple_of(INSTANCE_BUFFER_ALIGNMENT);
        if offset
            .checked_add(size)
            .is_none_or(|end| end > self.current.size)
        {
            self.grow(size)?;
            offset = 0;
        }
        self.offset = offset + size;
        let binding = InstanceBinding {
            buffer: self.current.metal_buffer.clone(),
            offset,
        };
        // SAFETY: The reserved byte range lies within this frame's exclusively owned buffer.
        let destination = unsafe {
            self.current
                .metal_buffer
                .contents()
                .as_ptr()
                .cast::<u8>()
                .add(offset)
        };
        Ok((binding, destination))
    }

    pub(super) fn write<T>(&mut self, values: &[T]) -> Result<InstanceBinding> {
        let size = mem::size_of_val(values);
        let (binding, destination) = self.reserve(size)?;
        // SAFETY: Both disjoint ranges cover size bytes; the byte copy needs no T alignment.
        unsafe {
            ptr::copy_nonoverlapping(values.as_ptr().cast::<u8>(), destination, size);
        }
        Ok(binding)
    }

    fn write_surface_bounds(&mut self, surfaces: &[PaintSurface]) -> Result<InstanceBinding> {
        let stride = mem::size_of::<SurfaceBounds>();
        let size = stride
            .checked_mul(surfaces.len())
            .context("Metal instance byte count overflowed")?;
        let (binding, destination) = self.reserve(size)?;
        for (index, surface) in surfaces.iter().enumerate() {
            let bounds = SurfaceBounds {
                bounds: surface.bounds,
                clip: surface.clip,
                transform: surface.renderer_transform(),
            };
            // SAFETY: The reservation holds one record per surface, and this write needs no alignment.
            unsafe {
                destination
                    .add(index * stride)
                    .cast::<SurfaceBounds>()
                    .write_unaligned(bounds);
            }
        }
        Ok(binding)
    }

    fn grow(&mut self, required: usize) -> Result<()> {
        anyhow::ensure!(
            required <= MAX_INSTANCE_BUFFER_SIZE,
            "instance buffer needs {required} bytes, above the maximum of {MAX_INSTANCE_BUFFER_SIZE}"
        );
        let mut pool = self.pool.lock();
        let buffer_size = (pool.buffer_size * 2)
            .max(required.next_power_of_two())
            .min(MAX_INSTANCE_BUFFER_SIZE);
        anyhow::ensure!(
            buffer_size > self.current.size,
            "frame instance data exceeds the {MAX_INSTANCE_BUFFER_SIZE}-byte maximum"
        );
        if buffer_size != pool.buffer_size {
            log::info!("increased instance buffer size to {buffer_size}");
            pool.reset(buffer_size);
        }
        let buffer = pool.acquire(&self.device, self.unified_memory)?;
        drop(pool);
        let filled = mem::replace(&mut self.current, buffer);
        self.filled.push((filled, self.offset));
        self.offset = 0;
        Ok(())
    }

    pub(super) fn finish(self) -> InstanceBuffer {
        let Self {
            unified_memory,
            filled,
            current,
            offset,
            ..
        } = self;
        #[cfg(target_os = "macos")]
        if !unified_memory {
            for (buffer, written) in &filled {
                if *written > 0 {
                    buffer.metal_buffer.didModifyRange(NSRange {
                        location: 0,
                        length: *written,
                    });
                }
            }
            if offset > 0 {
                current.metal_buffer.didModifyRange(NSRange {
                    location: 0,
                    length: offset,
                });
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (unified_memory, offset);
        drop(filled);
        current
    }
}
