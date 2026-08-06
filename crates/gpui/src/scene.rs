// todo("windows"): remove
#![cfg_attr(windows, allow(dead_code))]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use crate::PlatformPixelBuffer;
use crate::{
    AtlasTextureInstanceId, AtlasTile, Background, Bounds, Corners, Edges, Hsla, Pixels, Point,
    ScaledPixels, Size, SubtreeTransformError,
    bounds_tree::BoundsTree,
    geometry::{
        ClipStackSnapshot, ResolvedClip, ResolvedSubtreeTransform, SubtreeGeometryError,
        SubtreeGeometryValidity,
    },
    point,
};
use std::{fmt::Debug, iter::Peekable, ops::Range, slice};

#[allow(non_camel_case_types, unused)]
#[expect(missing_docs)]
pub type PathVertex_ScaledPixels = PathVertex<ScaledPixels>;

#[expect(missing_docs)]
pub type DrawOrder = u32;

/// One exact window-space rounded-rectangle clip consumed by renderer shaders.
#[derive(Default, Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct GpuClipShape {
    /// Window-space bounds in device-scaled pixels.
    pub bounds: Bounds<ScaledPixels>,
    /// Horizontal radii in top-left, top-right, bottom-right, bottom-left order.
    pub radii_x: Corners<ScaledPixels>,
    /// Vertical radii in top-left, top-right, bottom-right, bottom-left order.
    pub radii_y: Corners<ScaledPixels>,
}

impl GpuClipShape {
    fn try_from_resolved(
        clip: &ResolvedClip,
        scale_factor: f32,
    ) -> Result<Self, SubtreeGeometryError> {
        let shape = Self {
            bounds: clip.bounds().scale(scale_factor),
            radii_x: clip.radii_x().scale(scale_factor),
            radii_y: clip.radii_y().scale(scale_factor),
        };
        if shape.is_finite() {
            Ok(shape)
        } else {
            Err(SubtreeGeometryError::DeviceConversion)
        }
    }

    fn is_finite(&self) -> bool {
        let values = [
            self.bounds.origin.x,
            self.bounds.origin.y,
            self.bounds.size.width,
            self.bounds.size.height,
            self.radii_x.top_left,
            self.radii_x.top_right,
            self.radii_x.bottom_right,
            self.radii_x.bottom_left,
            self.radii_y.top_left,
            self.radii_y.top_right,
            self.radii_y.bottom_right,
            self.radii_y.bottom_left,
        ];
        values.into_iter().all(|value| value.0.is_finite())
    }
}

/// A primitive's conservative clip bounds and range in [`Scene::clip_shapes`].
#[derive(Default, Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct ClipEnvelope {
    /// Conservative intersection AABB used only for culling and early rejection.
    pub conservative_bounds: Bounds<ScaledPixels>,
    /// First [`GpuClipShape`] element in the Scene clip arena.
    pub first_clip: u32,
    /// Number of consecutive [`GpuClipShape`] elements in the Scene clip arena.
    pub clip_count: u32,
}

#[derive(Default)]
#[expect(missing_docs)]
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<DrawOrder>,
    pub shadows: Vec<Shadow>,
    pub quads: Vec<Quad>,
    pub paths: Vec<Path<ScaledPixels>>,
    pub underlines: Vec<Underline>,
    pub monochrome_sprites: Vec<MonochromeSprite>,
    pub subpixel_sprites: Vec<SubpixelSprite>,
    pub polychrome_sprites: Vec<PolychromeSprite>,
    pub surfaces: Vec<PaintSurface>,
    clip_shapes: Vec<GpuClipShape>,
    clip_stack_ranges: Vec<Range<u32>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetainedVisualSceneError {
    InvalidRange,
    InvalidGeometry,
    Geometry(SubtreeGeometryError),
}

#[derive(Clone, Copy)]
struct SceneJournalCheckpoint {
    paint_operations_len: usize,
    clip_shapes_len: usize,
    clip_stack_ranges_len: usize,
}

#[expect(missing_docs)]
impl Scene {
    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
        self.clip_shapes.clear();
        self.clip_stack_ranges.clear();
    }

    /// Returns the exact frame-local clip-shape arena referenced by primitive envelopes.
    pub fn clip_shapes(&self) -> &[GpuClipShape] {
        &self.clip_shapes
    }

    pub fn len(&self) -> usize {
        self.paint_operations
            .iter()
            .filter(|operation| {
                operation
                    .validity
                    .as_ref()
                    .is_none_or(SubtreeGeometryValidity::is_valid)
            })
            .count()
    }

    pub(crate) fn has_primitives(&self) -> bool {
        !self.shadows.is_empty()
            || !self.quads.is_empty()
            || !self.paths.is_empty()
            || !self.underlines.is_empty()
            || !self.monochrome_sprites.is_empty()
            || !self.subpixel_sprites.is_empty()
            || !self.polychrome_sprites.is_empty()
            || !self.surfaces.is_empty()
    }

    pub(crate) fn journal_len(&self) -> usize {
        self.paint_operations.len()
    }

    pub(crate) fn retained_visual_fragment(
        &self,
        range: Range<usize>,
    ) -> Result<(Self, Box<[Option<SubtreeGeometryValidity>]>), RetainedVisualSceneError> {
        let Some(operations) = self.paint_operations.get(range.clone()) else {
            return Err(RetainedVisualSceneError::InvalidRange);
        };
        if operations.iter().any(|operation| {
            operation
                .validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
        }) {
            return Err(RetainedVisualSceneError::InvalidGeometry);
        }

        let mut fragment = Scene::default();
        fragment
            .replay(range, self, None)
            .map_err(RetainedVisualSceneError::Geometry)?;
        let validities = operations
            .iter()
            .map(|operation| operation.validity.clone())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok((fragment, validities))
    }

    pub(crate) fn atlas_texture_instances(&self) -> Vec<AtlasTextureInstanceId> {
        let mut textures = Vec::new();
        for operation in &self.paint_operations {
            let texture = match &operation.kind {
                PaintOperationKind::Primitive(Primitive::MonochromeSprite(sprite)) => {
                    Some(sprite.tile.texture_instance())
                }
                PaintOperationKind::Primitive(Primitive::SubpixelSprite(sprite)) => {
                    Some(sprite.tile.texture_instance())
                }
                PaintOperationKind::Primitive(Primitive::PolychromeSprite(sprite)) => {
                    Some(sprite.tile.texture_instance())
                }
                _ => None,
            };
            if let Some(texture) = texture
                && !textures.contains(&texture)
            {
                textures.push(texture);
            }
        }
        textures
    }

    pub fn push_layer(&mut self, bounds: Bounds<ScaledPixels>) {
        self.push_layer_scoped(bounds, None);
    }

    pub(crate) fn push_layer_scoped(
        &mut self,
        bounds: Bounds<ScaledPixels>,
        validity: Option<SubtreeGeometryValidity>,
    ) {
        self.push_layer_for_render(bounds);
        self.paint_operations.push(PaintOperation {
            kind: PaintOperationKind::StartLayer(bounds),
            validity,
        });
    }

    fn push_layer_for_render(&mut self, bounds: Bounds<ScaledPixels>) {
        let order = self.primitive_bounds.insert(bounds);
        self.layer_stack.push(order);
    }

    pub fn pop_layer(&mut self) {
        self.pop_layer_scoped(None);
    }

    pub(crate) fn pop_layer_scoped(&mut self, validity: Option<SubtreeGeometryValidity>) {
        self.pop_layer_for_render();
        self.paint_operations.push(PaintOperation {
            kind: PaintOperationKind::EndLayer,
            validity,
        });
    }

    fn pop_layer_for_render(&mut self) {
        self.layer_stack.pop();
    }

    pub(crate) fn insert_primitive_scoped(
        &mut self,
        primitive: impl Into<Primitive>,
        clip_stack: &ClipStackSnapshot,
        scale_factor: f32,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let mut primitive = primitive.into();
        let clip = self.intern_clip_stack(clip_stack, scale_factor)?;
        primitive.set_clip_envelope(clip);
        self.record_primitive(primitive, validity)
    }

    fn insert_primitive_with_gpu_clips(
        &mut self,
        mut primitive: Primitive,
        clip_shapes: &[GpuClipShape],
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let clip = self.intern_gpu_clip_stack(clip_shapes)?;
        primitive.set_clip_envelope(clip);
        self.record_primitive(primitive, validity)
    }

    fn record_primitive(
        &mut self,
        mut primitive: Primitive,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        if !self.push_primitive_for_render(&mut primitive)? {
            return Ok(());
        }
        self.paint_operations.push(PaintOperation {
            kind: PaintOperationKind::Primitive(primitive),
            validity,
        });
        Ok(())
    }

    fn push_primitive_for_render(
        &mut self,
        primitive: &mut Primitive,
    ) -> Result<bool, SubtreeGeometryError> {
        let clipped_bounds = primitive
            .try_visual_bounds()?
            .intersect(&primitive.clip_envelope().conservative_bounds);

        if clipped_bounds.is_empty() {
            return Ok(false);
        }

        let order = self
            .layer_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.primitive_bounds.insert(clipped_bounds));
        match primitive {
            Primitive::Shadow(shadow) => {
                shadow.order = order;
                self.shadows.push(*shadow);
            }
            Primitive::Quad(quad) => {
                quad.order = order;
                self.quads.push(*quad);
            }
            Primitive::Path(path) => {
                path.order = order;
                path.id = PathId(self.paths.len());
                self.paths.push(path.clone());
            }
            Primitive::Underline(underline) => {
                underline.order = order;
                self.underlines.push(*underline);
            }
            Primitive::MonochromeSprite(sprite) => {
                sprite.order = order;
                self.monochrome_sprites.push(*sprite);
            }
            Primitive::SubpixelSprite(sprite) => {
                sprite.order = order;
                self.subpixel_sprites.push(*sprite);
            }
            Primitive::PolychromeSprite(sprite) => {
                sprite.order = order;
                self.polychrome_sprites.push(*sprite);
            }
            Primitive::Surface(surface) => {
                surface.order = order;
                self.surfaces.push(surface.clone());
            }
        }
        Ok(true)
    }

    fn intern_clip_stack(
        &mut self,
        snapshot: &ClipStackSnapshot,
        scale_factor: f32,
    ) -> Result<ClipEnvelope, SubtreeGeometryError> {
        let shapes = snapshot
            .clips()
            .iter()
            .map(|clip| GpuClipShape::try_from_resolved(clip, scale_factor))
            .collect::<Result<Vec<_>, _>>()?;
        self.intern_gpu_clip_stack(&shapes)
    }

    fn intern_gpu_clip_stack(
        &mut self,
        shapes: &[GpuClipShape],
    ) -> Result<ClipEnvelope, SubtreeGeometryError> {
        let Some(first_shape) = shapes.first() else {
            return Err(SubtreeGeometryError::DeviceConversion);
        };
        let conservative_bounds = shapes
            .iter()
            .skip(1)
            .fold(first_shape.bounds, |bounds, shape| {
                bounds.intersect(&shape.bounds)
            });

        for range in &self.clip_stack_ranges {
            let start = range.start as usize;
            let end = range.end as usize;
            if self.clip_shapes[start..end] == *shapes {
                return Ok(ClipEnvelope {
                    conservative_bounds,
                    first_clip: range.start,
                    clip_count: range.end - range.start,
                });
            }
        }

        let first_clip = u32::try_from(self.clip_shapes.len())
            .map_err(|_| SubtreeGeometryError::DeviceConversion)?;
        let clip_count =
            u32::try_from(shapes.len()).map_err(|_| SubtreeGeometryError::DeviceConversion)?;
        let end = first_clip
            .checked_add(clip_count)
            .ok_or(SubtreeGeometryError::DeviceConversion)?;
        self.clip_shapes.extend_from_slice(shapes);
        self.clip_stack_ranges.push(first_clip..end);
        Ok(ClipEnvelope {
            conservative_bounds,
            first_clip,
            clip_count,
        })
    }

    fn clip_shapes_for(
        &self,
        envelope: ClipEnvelope,
    ) -> Result<&[GpuClipShape], SubtreeGeometryError> {
        let start = envelope.first_clip as usize;
        let end = start
            .checked_add(envelope.clip_count as usize)
            .ok_or(SubtreeGeometryError::DeviceConversion)?;
        self.clip_shapes
            .get(start..end)
            .filter(|shapes| !shapes.is_empty())
            .ok_or(SubtreeGeometryError::DeviceConversion)
    }

    pub(crate) fn replay(
        &mut self,
        range: Range<usize>,
        prev_scene: &Scene,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let checkpoint = self.journal_checkpoint();
        let result = (|| {
            let operations = prev_scene
                .paint_operations
                .get(range)
                .ok_or(SubtreeGeometryError::DeviceConversion)?;
            for operation in operations {
                let replayed_validity = SubtreeGeometryValidity::replayed_under(
                    operation.validity.as_ref(),
                    validity.clone(),
                );
                match &operation.kind {
                    PaintOperationKind::Primitive(primitive) => self
                        .insert_primitive_with_gpu_clips(
                            primitive.clone(),
                            prev_scene.clip_shapes_for(primitive.clip_envelope())?,
                            replayed_validity,
                        )?,
                    PaintOperationKind::StartLayer(bounds) => {
                        self.push_layer_scoped(*bounds, replayed_validity)
                    }
                    PaintOperationKind::EndLayer => self.pop_layer_scoped(replayed_validity),
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.rollback_to(checkpoint);
        }
        result
    }

    pub fn finish(&mut self) {
        if self.paint_operations.iter().any(|operation| {
            operation
                .validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
        }) {
            self.rebuild_render_state_from_journal();
        }
        self.shadows.sort_by_key(|shadow| shadow.order);
        self.quads.sort_by_key(|quad| quad.order);
        self.paths.sort_by_key(|path| path.order);
        self.underlines.sort_by_key(|underline| underline.order);
        self.monochrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.subpixel_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.polychrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.surfaces.sort_by_key(|surface| surface.order);
    }

    fn journal_checkpoint(&self) -> SceneJournalCheckpoint {
        SceneJournalCheckpoint {
            paint_operations_len: self.paint_operations.len(),
            clip_shapes_len: self.clip_shapes.len(),
            clip_stack_ranges_len: self.clip_stack_ranges.len(),
        }
    }

    fn rollback_to(&mut self, checkpoint: SceneJournalCheckpoint) {
        self.paint_operations
            .truncate(checkpoint.paint_operations_len);
        self.clip_shapes.truncate(checkpoint.clip_shapes_len);
        self.clip_stack_ranges
            .truncate(checkpoint.clip_stack_ranges_len);
        self.rebuild_render_state_from_journal();
    }

    fn rebuild_render_state_from_journal(&mut self) {
        let operations = std::mem::take(&mut self.paint_operations);
        self.clear_render_state();
        for operation in &operations {
            if operation
                .validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
            {
                continue;
            }
            match &operation.kind {
                PaintOperationKind::Primitive(primitive) => {
                    let mut primitive = primitive.clone();
                    self.push_primitive_for_render(&mut primitive)
                        .expect("accepted scene primitive must remain representable");
                }
                PaintOperationKind::StartLayer(bounds) => self.push_layer_for_render(*bounds),
                PaintOperationKind::EndLayer => self.pop_layer_for_render(),
            }
        }
        self.paint_operations = operations;
    }

    fn clear_render_state(&mut self) {
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
    }

    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    pub fn batches(&self) -> impl Iterator<Item = PrimitiveBatch> + '_ {
        BatchIterator {
            shadows_start: 0,
            shadows_iter: self.shadows.iter().peekable(),
            quads_start: 0,
            quads_iter: self.quads.iter().peekable(),
            paths_start: 0,
            paths_iter: self.paths.iter().peekable(),
            underlines_start: 0,
            underlines_iter: self.underlines.iter().peekable(),
            monochrome_sprites_start: 0,
            monochrome_sprites_iter: self.monochrome_sprites.iter().peekable(),
            subpixel_sprites_start: 0,
            subpixel_sprites_iter: self.subpixel_sprites.iter().peekable(),
            polychrome_sprites_start: 0,
            polychrome_sprites_iter: self.polychrome_sprites.iter().peekable(),
            surfaces_start: 0,
            surfaces_iter: self.surfaces.iter().peekable(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Default)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) enum PrimitiveKind {
    Shadow,
    #[default]
    Quad,
    Path,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
    Surface,
}

pub(crate) struct PaintOperation {
    kind: PaintOperationKind,
    validity: Option<SubtreeGeometryValidity>,
}

pub(crate) enum PaintOperationKind {
    Primitive(Primitive),
    StartLayer(Bounds<ScaledPixels>),
    EndLayer,
}

#[derive(Clone)]
#[expect(missing_docs)]
pub enum Primitive {
    Shadow(Shadow),
    Quad(Quad),
    Path(Path<ScaledPixels>),
    Underline(Underline),
    MonochromeSprite(MonochromeSprite),
    SubpixelSprite(SubpixelSprite),
    PolychromeSprite(PolychromeSprite),
    Surface(PaintSurface),
}

#[expect(missing_docs)]
impl Primitive {
    pub fn bounds(&self) -> &Bounds<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.bounds,
            Primitive::Quad(quad) => &quad.bounds,
            Primitive::Path(path) => &path.bounds,
            Primitive::Underline(underline) => &underline.bounds,
            Primitive::MonochromeSprite(sprite) => &sprite.bounds,
            Primitive::SubpixelSprite(sprite) => &sprite.bounds,
            Primitive::PolychromeSprite(sprite) => &sprite.bounds,
            Primitive::Surface(surface) => &surface.bounds,
        }
    }

    pub(crate) fn atlas_texture_instance(&self) -> Option<AtlasTextureInstanceId> {
        match self {
            Primitive::MonochromeSprite(sprite) => Some(sprite.tile.texture_instance()),
            Primitive::SubpixelSprite(sprite) => Some(sprite.tile.texture_instance()),
            Primitive::PolychromeSprite(sprite) => Some(sprite.tile.texture_instance()),
            Primitive::Shadow(_)
            | Primitive::Quad(_)
            | Primitive::Path(_)
            | Primitive::Underline(_)
            | Primitive::Surface(_) => None,
        }
    }

    /// Returns the primitive's range into the owning Scene clip arena.
    pub fn clip_envelope(&self) -> ClipEnvelope {
        match self {
            Primitive::Shadow(shadow) => shadow.clip,
            Primitive::Quad(quad) => quad.clip,
            Primitive::Path(path) => path.clip,
            Primitive::Underline(underline) => underline.clip,
            Primitive::MonochromeSprite(sprite) => sprite.clip,
            Primitive::SubpixelSprite(sprite) => sprite.clip,
            Primitive::PolychromeSprite(sprite) => sprite.clip,
            Primitive::Surface(surface) => surface.clip,
        }
    }

    fn set_clip_envelope(&mut self, clip: ClipEnvelope) {
        match self {
            Primitive::Shadow(shadow) => shadow.clip = clip,
            Primitive::Quad(quad) => quad.clip = clip,
            Primitive::Path(path) => path.clip = clip,
            Primitive::Underline(underline) => underline.clip = clip,
            Primitive::MonochromeSprite(sprite) => sprite.clip = clip,
            Primitive::SubpixelSprite(sprite) => sprite.clip = clip,
            Primitive::PolychromeSprite(sprite) => sprite.clip = clip,
            Primitive::Surface(surface) => surface.clip = clip,
        }
    }

    pub fn transform(&self) -> PrimitiveTransform {
        match self {
            Primitive::Shadow(shadow) => shadow.transform,
            Primitive::Quad(quad) => quad.transform,
            Primitive::Path(path) => path.transform,
            Primitive::Underline(underline) => underline.transform,
            Primitive::MonochromeSprite(sprite) => sprite.transform,
            Primitive::SubpixelSprite(sprite) => sprite.transform,
            Primitive::PolychromeSprite(sprite) => sprite.transform,
            Primitive::Surface(surface) => surface.transform,
        }
    }

    fn local_raster_bounds(&self) -> Bounds<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => shadow.local_raster_bounds(),
            _ => *self.bounds(),
        }
    }

    pub fn try_visual_bounds(&self) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        self.transform()
            .try_project_bounds(self.local_raster_bounds())
    }
}

#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
struct BatchIterator<'a> {
    shadows_start: usize,
    shadows_iter: Peekable<slice::Iter<'a, Shadow>>,
    quads_start: usize,
    quads_iter: Peekable<slice::Iter<'a, Quad>>,
    paths_start: usize,
    paths_iter: Peekable<slice::Iter<'a, Path<ScaledPixels>>>,
    underlines_start: usize,
    underlines_iter: Peekable<slice::Iter<'a, Underline>>,
    monochrome_sprites_start: usize,
    monochrome_sprites_iter: Peekable<slice::Iter<'a, MonochromeSprite>>,
    subpixel_sprites_start: usize,
    subpixel_sprites_iter: Peekable<slice::Iter<'a, SubpixelSprite>>,
    polychrome_sprites_start: usize,
    polychrome_sprites_iter: Peekable<slice::Iter<'a, PolychromeSprite>>,
    surfaces_start: usize,
    surfaces_iter: Peekable<slice::Iter<'a, PaintSurface>>,
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = PrimitiveBatch;

    fn next(&mut self) -> Option<Self::Item> {
        let mut orders_and_kinds = [
            (
                self.shadows_iter.peek().map(|s| s.order),
                PrimitiveKind::Shadow,
            ),
            (self.quads_iter.peek().map(|q| q.order), PrimitiveKind::Quad),
            (self.paths_iter.peek().map(|q| q.order), PrimitiveKind::Path),
            (
                self.underlines_iter.peek().map(|u| u.order),
                PrimitiveKind::Underline,
            ),
            (
                self.monochrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::MonochromeSprite,
            ),
            (
                self.subpixel_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::SubpixelSprite,
            ),
            (
                self.polychrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::PolychromeSprite,
            ),
            (
                self.surfaces_iter.peek().map(|s| s.order),
                PrimitiveKind::Surface,
            ),
        ];
        orders_and_kinds.sort_by_key(|(order, kind)| (order.unwrap_or(u32::MAX), *kind));

        let first = orders_and_kinds[0];
        let second = orders_and_kinds[1];
        let (batch_kind, max_order_and_kind) = if first.0.is_some() {
            (first.1, (second.0.unwrap_or(u32::MAX), second.1))
        } else {
            return None;
        };

        match batch_kind {
            PrimitiveKind::Shadow => {
                let shadows_start = self.shadows_start;
                let mut shadows_end = shadows_start + 1;
                self.shadows_iter.next();
                while self
                    .shadows_iter
                    .next_if(|shadow| (shadow.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    shadows_end += 1;
                }
                self.shadows_start = shadows_end;
                Some(PrimitiveBatch::Shadows(shadows_start..shadows_end))
            }
            PrimitiveKind::Quad => {
                let quads_start = self.quads_start;
                let mut quads_end = quads_start + 1;
                self.quads_iter.next();
                while self
                    .quads_iter
                    .next_if(|quad| (quad.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    quads_end += 1;
                }
                self.quads_start = quads_end;
                Some(PrimitiveBatch::Quads(quads_start..quads_end))
            }
            PrimitiveKind::Path => {
                let paths_start = self.paths_start;
                let mut paths_end = paths_start + 1;
                self.paths_iter.next();
                while self
                    .paths_iter
                    .next_if(|path| (path.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    paths_end += 1;
                }
                self.paths_start = paths_end;
                Some(PrimitiveBatch::Paths(paths_start..paths_end))
            }
            PrimitiveKind::Underline => {
                let underlines_start = self.underlines_start;
                let mut underlines_end = underlines_start + 1;
                self.underlines_iter.next();
                while self
                    .underlines_iter
                    .next_if(|underline| (underline.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    underlines_end += 1;
                }
                self.underlines_start = underlines_end;
                Some(PrimitiveBatch::Underlines(underlines_start..underlines_end))
            }
            PrimitiveKind::MonochromeSprite => {
                let texture = self
                    .monochrome_sprites_iter
                    .peek()
                    .unwrap()
                    .tile
                    .texture_instance();
                let sprites_start = self.monochrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.monochrome_sprites_iter.next();
                while self
                    .monochrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_instance() == texture
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.monochrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::MonochromeSprites {
                    texture,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::SubpixelSprite => {
                let texture = self
                    .subpixel_sprites_iter
                    .peek()
                    .unwrap()
                    .tile
                    .texture_instance();
                let sprites_start = self.subpixel_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.subpixel_sprites_iter.next();
                while self
                    .subpixel_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_instance() == texture
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.subpixel_sprites_start = sprites_end;
                Some(PrimitiveBatch::SubpixelSprites {
                    texture,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::PolychromeSprite => {
                let texture = self
                    .polychrome_sprites_iter
                    .peek()
                    .unwrap()
                    .tile
                    .texture_instance();
                let sprites_start = self.polychrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.polychrome_sprites_iter.next();
                while self
                    .polychrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_instance() == texture
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.polychrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::PolychromeSprites {
                    texture,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::Surface => {
                let surfaces_start = self.surfaces_start;
                let mut surfaces_end = surfaces_start + 1;
                self.surfaces_iter.next();
                while self
                    .surfaces_iter
                    .next_if(|surface| (surface.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    surfaces_end += 1;
                }
                self.surfaces_start = surfaces_end;
                Some(PrimitiveBatch::Surfaces(surfaces_start..surfaces_end))
            }
        }
    }
}

#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[allow(missing_docs)]
pub enum PrimitiveBatch {
    Shadows(Range<usize>),
    Quads(Range<usize>),
    Paths(Range<usize>),
    Underlines(Range<usize>),
    MonochromeSprites {
        texture: AtlasTextureInstanceId,
        range: Range<usize>,
    },
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    SubpixelSprites {
        texture: AtlasTextureInstanceId,
        range: Range<usize>,
    },
    PolychromeSprites {
        texture: AtlasTextureInstanceId,
        range: Range<usize>,
    },
    Surfaces(Range<usize>),
}

/// Renderer-facing raster projection derived from GPUI's checked subtree transform authority.
///
/// This type is an internal cross-crate ABI. Application code constructs [`crate::SubtreeTransform`]
/// instead. [`crate::Window`] may retarget this axis-aligned projection after projecting and
/// snapping a primitive's raster envelope, while shaders retain the primitive's local coordinates
/// for gradients, signed-distance fields, and texture sampling.
#[doc(hidden)]
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct PrimitiveTransform {
    scale_x: f32,
    scale_y: f32,
    translation_x: f32,
    translation_y: f32,
}

impl Default for PrimitiveTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl PrimitiveTransform {
    pub const IDENTITY: Self = Self {
        scale_x: 1.0,
        scale_y: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
    };

    pub(crate) fn try_new(
        scale: Size<f32>,
        translation: Point<ScaledPixels>,
    ) -> Result<Self, SubtreeTransformError> {
        if !scale.width.is_normal()
            || scale.width <= 0.0
            || !scale.height.is_normal()
            || scale.height <= 0.0
            || !scale.width.recip().is_finite()
            || !scale.height.recip().is_finite()
        {
            return Err(SubtreeTransformError::InvalidScale);
        }
        if !translation.x.0.is_finite() || !translation.y.0.is_finite() {
            return Err(SubtreeTransformError::NonFiniteTranslation);
        }
        Ok(Self {
            scale_x: scale.width,
            scale_y: scale.height,
            translation_x: translation.x.0,
            translation_y: translation.y.0,
        })
    }

    pub(crate) fn try_from_resolved(
        resolved: ResolvedSubtreeTransform,
        scale_factor: f32,
    ) -> Result<Self, SubtreeTransformError> {
        if !scale_factor.is_normal() || scale_factor <= 0.0 {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        let offset = resolved.offset();
        let translation_x = offset.x.0 * scale_factor;
        let translation_y = offset.y.0 * scale_factor;
        if !translation_x.is_finite() || !translation_y.is_finite() {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        Self::try_new(
            resolved.scale(),
            point(ScaledPixels(translation_x), ScaledPixels(translation_y)),
        )
    }

    pub fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    pub(crate) fn scale(self) -> Size<f32> {
        Size::new(self.scale_x, self.scale_y)
    }

    fn try_from_components(components: [f32; 4]) -> Result<Self, SubtreeTransformError> {
        Self::try_new(
            Size::new(components[0], components[1]),
            point(ScaledPixels(components[2]), ScaledPixels(components[3])),
        )
    }

    #[doc(hidden)]
    pub const fn components(self) -> [f32; 4] {
        [
            self.scale_x,
            self.scale_y,
            self.translation_x,
            self.translation_y,
        ]
    }

    fn validate(self) -> Result<(), SubtreeTransformError> {
        Self::try_from_components(self.components()).map(|_| ())
    }

    pub fn try_project_bounds(
        self,
        bounds: Bounds<ScaledPixels>,
    ) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        self.validate()?;
        let origin_x = self.scale_x.mul_add(bounds.origin.x.0, self.translation_x);
        let origin_y = self.scale_y.mul_add(bounds.origin.y.0, self.translation_y);
        let width = self.scale_x * bounds.size.width.0;
        let height = self.scale_y * bounds.size.height.0;
        if !origin_x.is_finite() || !origin_y.is_finite() {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        for (source, projected) in [(bounds.size.width.0, width), (bounds.size.height.0, height)] {
            if !projected.is_finite() || (source != 0.0 && projected == 0.0) {
                return Err(SubtreeTransformError::UnrepresentableResult);
            }
        }
        Ok(Bounds::new(
            point(ScaledPixels(origin_x), ScaledPixels(origin_y)),
            Size::new(ScaledPixels(width), ScaledPixels(height)),
        ))
    }

    /// Returns an axis-aligned projection that maps `source` onto `target` exactly at both edges.
    ///
    /// This is used after the checked subtree projection has produced device-space bounds and the
    /// window has applied its single device-pixel snapping policy. Interior points remain a linear
    /// interpolation of the primitive's original local coordinate space.
    pub(crate) fn try_retarget_bounds(
        self,
        source: Bounds<ScaledPixels>,
        target: Bounds<ScaledPixels>,
    ) -> Result<Self, SubtreeTransformError> {
        self.validate()?;

        fn retarget_axis(
            source_origin: f32,
            source_size: f32,
            target_origin: f32,
            target_size: f32,
            fallback_scale: f32,
        ) -> Result<(f32, f32), SubtreeTransformError> {
            if !source_origin.is_finite()
                || !source_size.is_finite()
                || !target_origin.is_finite()
                || !target_size.is_finite()
                || source_size < 0.0
                || target_size < 0.0
            {
                return Err(SubtreeTransformError::UnrepresentableResult);
            }

            let scale = if source_size == 0.0 {
                if target_size != 0.0 {
                    return Err(SubtreeTransformError::UnrepresentableResult);
                }
                fallback_scale
            } else {
                if target_size == 0.0 {
                    return Err(SubtreeTransformError::UnrepresentableResult);
                }
                target_size / source_size
            };
            let translation = scale.mul_add(-source_origin, target_origin);
            if !scale.is_normal()
                || scale <= 0.0
                || !scale.recip().is_finite()
                || !translation.is_finite()
            {
                return Err(SubtreeTransformError::UnrepresentableResult);
            }
            Ok((scale, translation))
        }

        let (scale_x, translation_x) = retarget_axis(
            source.origin.x.0,
            source.size.width.0,
            target.origin.x.0,
            target.size.width.0,
            self.scale_x,
        )?;
        let (scale_y, translation_y) = retarget_axis(
            source.origin.y.0,
            source.size.height.0,
            target.origin.y.0,
            target.size.height.0,
            self.scale_y,
        )?;

        Self::try_new(
            Size::new(scale_x, scale_y),
            point(ScaledPixels(translation_x), ScaledPixels(translation_y)),
        )
        .map_err(|_| SubtreeTransformError::UnrepresentableResult)
    }
}

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Quad {
    pub order: DrawOrder,
    pub border_style: BorderStyle,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners<ScaledPixels>,
    pub border_widths: Edges<ScaledPixels>,
    pub(crate) transform: PrimitiveTransform,
}

impl From<Quad> for Primitive {
    fn from(quad: Quad) -> Self {
        Primitive::Quad(quad)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Underline {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Hsla,
    pub thickness: ScaledPixels,
    pub wavy: u32,
    pub(crate) transform: PrimitiveTransform,
}

impl From<Underline> for Primitive {
    fn from(underline: Underline) -> Self {
        Primitive::Underline(underline)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Shadow {
    pub order: DrawOrder,
    pub blur_radius: ScaledPixels,
    pub bounds: Bounds<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Hsla,
    pub element_bounds: Bounds<ScaledPixels>,
    pub element_corner_radii: Corners<ScaledPixels>,
    /// 0 = drop shadow (rendered outside the element), 1 = inset shadow (rendered inside).
    pub inset: u32,
    pub pad: u32, // align to 8 bytes
    pub(crate) transform: PrimitiveTransform,
}

impl Shadow {
    pub(crate) fn local_raster_bounds(&self) -> Bounds<ScaledPixels> {
        if self.inset != 0 {
            self.element_bounds
        } else {
            self.bounds.dilate(ScaledPixels(3.0 * self.blur_radius.0))
        }
    }
}

impl From<Shadow> for Primitive {
    fn from(shadow: Shadow) -> Self {
        Primitive::Shadow(shadow)
    }
}

/// The style of a border.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum BorderStyle {
    /// A solid border.
    #[default]
    Solid = 0,
    /// A dashed border.
    Dashed = 1,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct MonochromeSprite {
    pub order: DrawOrder,
    pub pad: u32,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Hsla,
    pub tile: AtlasTile,
    pub(crate) transform: PrimitiveTransform,
}

impl From<MonochromeSprite> for Primitive {
    fn from(sprite: MonochromeSprite) -> Self {
        Primitive::MonochromeSprite(sprite)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct SubpixelSprite {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Hsla,
    pub tile: AtlasTile,
    pub(crate) transform: PrimitiveTransform,
}

impl From<SubpixelSprite> for Primitive {
    fn from(sprite: SubpixelSprite) -> Self {
        Primitive::SubpixelSprite(sprite)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PolychromeSprite {
    pub order: DrawOrder,
    pub pad: u32,
    pub grayscale: bool,
    pub opacity: f32,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub corner_radii: Corners<ScaledPixels>,
    pub tile: AtlasTile,
    pub(crate) transform: PrimitiveTransform,
}

impl From<PolychromeSprite> for Primitive {
    fn from(sprite: PolychromeSprite) -> Self {
        Primitive::PolychromeSprite(sprite)
    }
}

impl PolychromeSprite {
    #[doc(hidden)]
    pub const fn renderer_transform(&self) -> PrimitiveTransform {
        self.transform
    }
}

#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct PaintSurface {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub(crate) transform: PrimitiveTransform,
    #[cfg(target_os = "macos")]
    pub image_buffer: PlatformPixelBuffer,
}

impl From<PaintSurface> for Primitive {
    fn from(surface: PaintSurface) -> Self {
        Primitive::Surface(surface)
    }
}

impl PaintSurface {
    #[doc(hidden)]
    pub const fn renderer_transform(&self) -> PrimitiveTransform {
        self.transform
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[expect(missing_docs)]
pub struct PathId(pub usize);

/// A line made up of a series of vertices and control points.
#[derive(Clone, Debug)]
#[expect(missing_docs)]
pub struct Path<P: Clone + Debug + Default + PartialEq> {
    pub id: PathId,
    pub order: DrawOrder,
    pub bounds: Bounds<P>,
    pub clip: ClipEnvelope,
    pub vertices: Vec<PathVertex<P>>,
    pub color: Background,
    pub(crate) transform: PrimitiveTransform,
    start: Point<P>,
    current: Point<P>,
    contour_count: usize,
}

impl<P: Clone + Debug + Default + PartialEq> Path<P> {
    #[doc(hidden)]
    pub const fn renderer_transform(&self) -> PrimitiveTransform {
        self.transform
    }
}

impl Path<Pixels> {
    /// Create a new path with the given starting point.
    pub fn new(start: Point<Pixels>) -> Self {
        Self {
            id: PathId(0),
            order: DrawOrder::default(),
            vertices: Vec::new(),
            start,
            current: start,
            bounds: Bounds {
                origin: start,
                size: Default::default(),
            },
            clip: Default::default(),
            color: Default::default(),
            transform: PrimitiveTransform::IDENTITY,
            contour_count: 0,
        }
    }

    /// Scale this path by the given factor.
    pub fn scale(&self, factor: f32) -> Path<ScaledPixels> {
        Path {
            id: self.id,
            order: self.order,
            bounds: self.bounds.scale(factor),
            clip: self.clip,
            vertices: self
                .vertices
                .iter()
                .map(|vertex| vertex.scale(factor))
                .collect(),
            start: self.start.map(|start| start.scale(factor)),
            current: self.current.scale(factor),
            contour_count: self.contour_count,
            color: self.color,
            transform: self.transform,
        }
    }

    /// Move the start, current point to the given point.
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        self.start = to;
        self.current = to;
    }

    /// Draw a straight line from the current point to the given point.
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }
        self.current = to;
    }

    /// Draw a curve from the current point to the given point, using the given control point.
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }

        self.push_triangle(
            (self.current, ctrl, to),
            (point(0., 0.), point(0.5, 0.), point(1., 1.)),
        );
        self.current = to;
    }

    /// Push a triangle to the Path.
    pub fn push_triangle(
        &mut self,
        xy: (Point<Pixels>, Point<Pixels>, Point<Pixels>),
        st: (Point<f32>, Point<f32>, Point<f32>),
    ) {
        self.bounds = self
            .bounds
            .union(&Bounds {
                origin: xy.0,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.1,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.2,
                size: Default::default(),
            });

        self.vertices.push(PathVertex {
            xy_position: xy.0,
            st_position: st.0,
        });
        self.vertices.push(PathVertex {
            xy_position: xy.1,
            st_position: st.1,
        });
        self.vertices.push(PathVertex {
            xy_position: xy.2,
            st_position: st.2,
        });
    }
}

impl From<Path<ScaledPixels>> for Primitive {
    fn from(path: Path<ScaledPixels>) -> Self {
        Primitive::Path(path)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathVertex<P: Clone + Debug + Default + PartialEq> {
    pub xy_position: Point<P>,
    pub st_position: Point<f32>,
}

#[expect(missing_docs)]
impl PathVertex<Pixels> {
    pub fn scale(&self, factor: f32) -> PathVertex<ScaledPixels> {
        PathVertex {
            xy_position: self.xy_position.scale(factor),
            st_position: self.st_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SubtreeClip, SubtreeClipError, px, size};

    fn scaled_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
        Bounds::new(
            point(ScaledPixels(x), ScaledPixels(y)),
            Size::new(ScaledPixels(width), ScaledPixels(height)),
        )
    }

    fn logical_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), Size::new(px(width), px(height)))
    }

    fn rounded_stack(viewport: Bounds<Pixels>, clip_bounds: Bounds<Pixels>) -> ClipStackSnapshot {
        let radii = Corners {
            top_left: size(px(10.0), px(10.0)),
            top_right: size(px(10.0), px(10.0)),
            bottom_right: size(px(10.0), px(10.0)),
            bottom_left: size(px(10.0), px(10.0)),
        };
        let clip = SubtreeClip::try_rounded_rect(clip_bounds, radii)
            .unwrap()
            .resolve_with_accessibility_axes(
                Bounds::new(Point::default(), viewport.size),
                ResolvedSubtreeTransform::IDENTITY,
                viewport,
                point(true, true),
            )
            .unwrap();
        ClipStackSnapshot::root(viewport).push(clip)
    }

    #[test]
    fn layer_operations_are_not_non_empty_paint_content() {
        let mut scene = Scene::default();
        scene.push_layer(scaled_bounds(0.0, 0.0, 100.0, 100.0));
        scene.pop_layer();

        assert_eq!(scene.len(), 2);
        assert!(!scene.has_primitives());
    }

    #[test]
    fn transformed_visual_bounds_drive_scene_culling_and_order() {
        let clip = ClipStackSnapshot::root(logical_bounds(100.0, 50.0, 40.0, 40.0));
        let translated = PrimitiveTransform::try_new(
            Size::new(2.0, 3.0),
            point(ScaledPixels(100.0), ScaledPixels(50.0)),
        )
        .unwrap();
        let mut scene = Scene::default();
        let first = Quad {
            bounds: scaled_bounds(0.0, 0.0, 10.0, 10.0),
            transform: translated,
            ..Default::default()
        };
        scene
            .insert_primitive_scoped(first, &clip, 1.0, None)
            .unwrap();

        assert_eq!(scene.quads.len(), 1);
        assert_eq!(
            Primitive::Quad(first).try_visual_bounds().unwrap(),
            scaled_bounds(100.0, 50.0, 20.0, 30.0)
        );

        let second = Quad {
            bounds: scaled_bounds(105.0, 55.0, 10.0, 10.0),
            transform: PrimitiveTransform::IDENTITY,
            ..Default::default()
        };
        scene
            .insert_primitive_scoped(second, &clip, 1.0, None)
            .unwrap();
        assert!(scene.quads[1].order > scene.quads[0].order);
        assert_eq!(
            scene.clip_shapes().len(),
            1,
            "identical stacks must deduplicate"
        );
        assert_eq!(scene.quads[0].clip, scene.quads[1].clip);

        let outside = Quad {
            bounds: scaled_bounds(0.0, 0.0, 10.0, 10.0),
            transform: PrimitiveTransform::IDENTITY,
            ..Default::default()
        };
        scene
            .insert_primitive_scoped(outside, &clip, 1.0, None)
            .unwrap();
        assert_eq!(scene.quads.len(), 2);
    }

    #[test]
    fn projected_bounds_allow_translation_to_zero() {
        let transform = PrimitiveTransform::try_new(
            Size::new(2.0, 3.0),
            point(ScaledPixels(-20.0), ScaledPixels(15.0)),
        )
        .unwrap();

        assert_eq!(
            transform
                .try_project_bounds(scaled_bounds(10.0, -5.0, 4.0, 6.0))
                .unwrap(),
            scaled_bounds(0.0, 0.0, 8.0, 18.0)
        );
    }

    #[test]
    fn raster_transform_retargets_projected_edges_after_snapping() {
        let source = scaled_bounds(0.4, 0.4, 1.0, 1.0);
        let base = PrimitiveTransform::try_new(
            Size::new(2.0, 2.0),
            point(ScaledPixels(0.4), ScaledPixels(0.4)),
        )
        .unwrap();
        let snapped_projected = scaled_bounds(1.0, 1.0, 2.0, 2.0);
        let raster = base.try_retarget_bounds(source, snapped_projected).unwrap();

        assert_eq!(
            raster.try_project_bounds(source).unwrap(),
            snapped_projected
        );
        assert_ne!(
            base.try_project_bounds(scaled_bounds(0.0, 0.0, 1.0, 1.0))
                .unwrap(),
            snapped_projected,
            "pre-projection snapping must remain observably different"
        );
    }

    #[test]
    fn raster_transform_preserves_a_degenerate_axis_and_rejects_collapse() {
        let base = PrimitiveTransform::try_new(
            Size::new(2.0, 3.0),
            point(ScaledPixels(0.25), ScaledPixels(-0.5)),
        )
        .unwrap();
        let source = scaled_bounds(4.0, 2.0, 0.0, 3.0);
        let target = scaled_bounds(8.0, 6.0, 0.0, 9.0);

        let raster = base.try_retarget_bounds(source, target).unwrap();
        assert_eq!(raster.try_project_bounds(source).unwrap(), target);
        assert_eq!(
            base.try_retarget_bounds(
                scaled_bounds(0.0, 0.0, 0.25, 1.0),
                scaled_bounds(0.0, 0.0, 0.0, 1.0),
            ),
            Err(SubtreeTransformError::UnrepresentableResult)
        );
    }

    #[test]
    fn shadow_visual_bounds_include_the_shader_raster_envelope() {
        let shadow = Shadow {
            order: 0,
            blur_radius: ScaledPixels(2.0),
            bounds: scaled_bounds(10.0, 20.0, 30.0, 40.0),
            corner_radii: Corners::default(),
            clip: ClipEnvelope::default(),
            color: Hsla::default(),
            element_bounds: scaled_bounds(10.0, 20.0, 30.0, 40.0),
            element_corner_radii: Corners::default(),
            inset: 0,
            pad: 0,
            transform: PrimitiveTransform::IDENTITY,
        };

        assert_eq!(
            Primitive::Shadow(shadow).try_visual_bounds().unwrap(),
            scaled_bounds(4.0, 14.0, 42.0, 52.0)
        );
    }

    #[test]
    fn scene_replay_imports_and_remaps_exact_clip_ranges() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let source_clip = rounded_stack(viewport, logical_bounds(20.0, 20.0, 80.0, 80.0));
        let mut source = Scene::default();
        source
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(30.0, 30.0, 20.0, 20.0),
                    ..Default::default()
                },
                &source_clip,
                1.0,
                None,
            )
            .unwrap();

        let seed_clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 10.0, 10.0));
        let mut replayed = Scene::default();
        replayed
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(1.0, 1.0, 5.0, 5.0),
                    ..Default::default()
                },
                &seed_clip,
                1.0,
                None,
            )
            .unwrap();
        replayed
            .replay(0..source.journal_len(), &source, None)
            .unwrap();

        let source_envelope = source.quads[0].clip;
        let replayed_envelope = replayed.quads[1].clip;
        assert_ne!(source_envelope.first_clip, replayed_envelope.first_clip);
        assert_eq!(source_envelope.clip_count, replayed_envelope.clip_count);
        let source_range = source_envelope.first_clip as usize
            ..(source_envelope.first_clip + source_envelope.clip_count) as usize;
        let replayed_range = replayed_envelope.first_clip as usize
            ..(replayed_envelope.first_clip + replayed_envelope.clip_count) as usize;
        assert_eq!(
            &source.clip_shapes()[source_range],
            &replayed.clip_shapes()[replayed_range]
        );
    }

    #[test]
    fn scene_replay_rolls_back_every_channel_after_a_late_error() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let clip = rounded_stack(viewport, logical_bounds(20.0, 20.0, 80.0, 80.0));
        let mut source = Scene::default();
        for x in [30.0, 60.0] {
            source
                .insert_primitive_scoped(
                    Quad {
                        bounds: scaled_bounds(x, 30.0, 20.0, 20.0),
                        ..Default::default()
                    },
                    &clip,
                    1.0,
                    None,
                )
                .unwrap();
        }
        let PaintOperationKind::Primitive(second) = &mut source.paint_operations[1].kind else {
            panic!("test source should contain a primitive");
        };
        let mut invalid_envelope = second.clip_envelope();
        invalid_envelope.first_clip = u32::MAX;
        second.set_clip_envelope(invalid_envelope);

        let mut target = Scene::default();
        target
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(30.0, 30.0, 10.0, 10.0),
                    ..Default::default()
                },
                &clip,
                1.0,
                None,
            )
            .unwrap();
        let journal_len = target.journal_len();
        let clip_shapes = target.clip_shapes().to_vec();
        let quad_count = target.quads.len();
        let quad_order = target.quads[0].order;
        let quad_bounds = target.quads[0].bounds;
        let quad_clip = target.quads[0].clip;

        assert_eq!(
            target.replay(0..source.journal_len(), &source, None),
            Err(SubtreeGeometryError::DeviceConversion)
        );
        assert_eq!(target.journal_len(), journal_len);
        assert_eq!(target.clip_shapes(), clip_shapes);
        assert_eq!(target.quads.len(), quad_count);
        assert_eq!(target.quads[0].order, quad_order);
        assert_eq!(target.quads[0].bounds, quad_bounds);
        assert_eq!(target.quads[0].clip, quad_clip);
    }

    #[test]
    fn scene_finish_discards_invalid_render_output_without_compacting_the_journal() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let invalid_clip = rounded_stack(viewport, logical_bounds(10.0, 10.0, 60.0, 60.0));
        let valid_clip = rounded_stack(viewport, logical_bounds(80.0, 80.0, 60.0, 60.0));
        let invalidity = SubtreeGeometryValidity::new(None);
        let mut scene = Scene::default();
        scene
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(20.0, 20.0, 20.0, 20.0),
                    ..Default::default()
                },
                &invalid_clip,
                1.0,
                Some(invalidity.clone()),
            )
            .unwrap();
        scene
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(90.0, 90.0, 20.0, 20.0),
                    ..Default::default()
                },
                &valid_clip,
                1.0,
                None,
            )
            .unwrap();
        let journal_len = scene.journal_len();
        let clip_shapes = scene.clip_shapes().to_vec();
        let valid_envelope = scene.quads[1].clip;

        invalidity.invalidate(SubtreeClipError::UnrepresentableResult);
        scene.finish();

        assert_eq!(scene.quads.len(), 1);
        assert_eq!(scene.quads[0].bounds, scaled_bounds(90.0, 90.0, 20.0, 20.0));
        assert_eq!(scene.quads[0].clip, valid_envelope);
        assert_eq!(scene.journal_len(), journal_len);
        assert_eq!(scene.clip_shapes(), clip_shapes);

        let mut replayed = Scene::default();
        replayed.replay(1..2, &scene, None).unwrap();
        replayed.finish();
        assert_eq!(replayed.quads.len(), 1);
        assert_eq!(
            replayed.quads[0].bounds,
            scaled_bounds(90.0, 90.0, 20.0, 20.0)
        );
    }
}
