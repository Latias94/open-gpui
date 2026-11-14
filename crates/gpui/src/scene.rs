// todo("windows"): remove
#![cfg_attr(windows, allow(dead_code))]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::PlatformPixelBuffer;
use crate::{
    AtlasTextureInstanceId, AtlasTextureLease, AtlasTextureLeaseError, AtlasTile, Background,
    Bounds, Corners, Edges, Hsla, Path, PathTiles, Pixels, PlatformAtlas, Point, Radians,
    ScaledPixels, Size, SubtreeTransformError, TimedTransformation,
    bounds_tree::BoundsTree,
    geometry::{
        ClipStackSnapshot, ResolvedClip, ResolvedSubtreeTransform, SubtreeGeometryError,
        SubtreeGeometryValidity,
    },
    point,
};
use smallvec::SmallVec;
use std::{
    collections::VecDeque, fmt::Debug, iter::Peekable, ops::Range, rc::Rc, slice, sync::Arc,
    time::Duration,
};

#[expect(missing_docs)]
pub type DrawOrder = u32;

/// A boolean with a fully initialized four-byte GPU storage representation.
///
/// Values are zero or one by construction. Shaders consume the complete value as a `u32`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct PaddedBool32(u32);

impl From<bool> for PaddedBool32 {
    fn from(value: bool) -> Self {
        Self(u32::from(value))
    }
}

impl PaddedBool32 {
    /// Returns the wrapped boolean.
    pub fn as_bool(self) -> bool {
        self.0 != 0
    }
}

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
    /// Device-space raster bounds from the exact clip stack and any primitive raster restriction.
    pub conservative_bounds: Bounds<ScaledPixels>,
    /// First [`GpuClipShape`] element in the Scene clip arena.
    pub first_clip: u32,
    /// Number of consecutive [`GpuClipShape`] elements in the Scene clip arena.
    pub clip_count: u32,
}

#[derive(Clone)]
struct Layer {
    primitives: SmallVec<[Range<usize>; 1]>,
    bounds: Bounds<ScaledPixels>,
    max_kind: PrimitiveKind,
    max_shared_kind: PrimitiveKind,
}

/// Owned drawing data, including the frame-local clip arena and paint journal.
///
/// Cloning preserves this data without acquiring atlas texture leases or transferring
/// accepted-frame, renderer-submission or native-window authority.
/// Geometry-validity associations remain tied to their original scope.
#[derive(Clone, Default)]
#[expect(missing_docs)]
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<usize>,
    layers: VecDeque<Layer>,
    legacy_sprite_order: Vec<LegacySpriteOrder>,
    pub shadows: Vec<Shadow>,
    pub quads: Vec<Quad>,
    pub paths: Vec<PaintedPath>,
    pub underlines: Vec<Underline>,
    pub monochrome_sprites: Vec<MonochromeSprite>,
    pub subpixel_sprites: Vec<SubpixelSprite>,
    pub polychrome_sprites: Vec<PolychromeSprite>,
    pub surfaces: Vec<PaintSurface>,
    clip_shapes: Vec<GpuClipShape>,
    clip_stack_ranges: Vec<Range<u32>>,
    animations: Vec<SpriteAnimation>,
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
    animations_len: usize,
}

#[derive(Clone, Copy, Default)]
struct LegacySpriteOrder {
    monochrome: bool,
    subpixel: bool,
    polychrome: bool,
}

#[expect(missing_docs)]
impl Scene {
    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.clear_render_state();
        self.clip_shapes.clear();
        self.clip_stack_ranges.clear();
        self.animations.clear();
    }

    /// How long the window may wait between presents needed only by sprite animations.
    pub(crate) fn animation_frame_interval(&self) -> Option<Duration> {
        self.animations
            .iter()
            .filter(|animation| {
                animation.sprite_index < self.monochrome_sprites.len()
                    && self.paint_operations[animation.journal_index]
                        .validity
                        .as_ref()
                        .is_none_or(SubtreeGeometryValidity::is_valid)
            })
            .map(|animation| {
                animation
                    .transformation
                    .min_frame_interval()
                    .unwrap_or_default()
            })
            .min()
    }

    /// Updates renderer sprites from their original color and the shared animation clock.
    pub(crate) fn advance_animations(&mut self, elapsed: Duration) {
        if self.animations.iter().any(|animation| {
            animation.sprite_index < self.monochrome_sprites.len()
                && self.paint_operations[animation.journal_index]
                    .validity
                    .as_ref()
                    .is_some_and(|validity| !validity.is_valid())
        }) {
            self.finish();
        }
        for animation in &self.animations {
            let Some(sprite) = self.monochrome_sprites.get_mut(animation.sprite_index) else {
                continue;
            };
            let pose = animation.transformation.at(elapsed);
            let transformation = pose.into_matrix(animation.center, animation.scale_factor);
            let color = animation.base_color.opacity(pose.alpha_factor());
            if !color.a.is_finite()
                || transformation
                    .try_transform_bounds(sprite.bounds)
                    .and_then(|bounds| sprite.transform.try_project_bounds(bounds))
                    .and_then(|bounds| {
                        if bounds.right().0.is_finite() && bounds.bottom().0.is_finite() {
                            Ok(bounds)
                        } else {
                            Err(SubtreeTransformError::UnrepresentableResult)
                        }
                    })
                    .is_err()
            {
                continue;
            }
            sprite.transformation = transformation;
            sprite.color = color;
        }
    }

    /// Returns the exact frame-local clip-shape arena referenced by primitive envelopes.
    pub fn clip_shapes(&self) -> &[GpuClipShape] {
        &self.clip_shapes
    }

    pub(crate) fn valid_geometry_scopes(
        &self,
    ) -> impl Iterator<Item = SubtreeGeometryValidity> + '_ {
        self.paint_operations
            .iter()
            .filter_map(|operation| operation.validity.clone())
            .filter(SubtreeGeometryValidity::is_valid)
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

    /// Retains this Scene's exact renderer-local atlas textures before delayed recording.
    ///
    /// Keep the lease with the Scene and validate it before recording. An atlas reset invalidates
    /// the lease; it cannot make a Scene portable to another renderer. This protects atlas
    /// residency, not accepted-frame or submission authority. After encoding, retain the
    /// backend's recording resources through the host's corresponding GPU completion.
    pub fn retain_atlas_textures(
        &self,
        atlas: Arc<dyn PlatformAtlas>,
    ) -> Result<AtlasTextureLease, AtlasTextureLeaseError> {
        let textures = self
            .batches()
            .filter_map(|batch| match batch {
                PrimitiveBatch::MonochromeSprites { texture, .. }
                | PrimitiveBatch::SubpixelSprites { texture, .. }
                | PrimitiveBatch::PolychromeSprites { texture, .. } => Some(texture),
                _ => None,
            })
            .collect::<Vec<_>>();
        atlas.retain_texture_instances(&textures)
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
        self.push_layer_for_render();
        self.paint_operations.push(PaintOperation {
            kind: PaintOperationKind::StartLayer(bounds),
            validity,
        });
    }

    fn push_layer_for_render(&mut self) {
        self.layer_stack.push(self.layers.len());
        self.layers.push_back(Layer {
            primitives: SmallVec::new(),
            bounds: Bounds::default(),
            max_kind: PrimitiveKind::Shadow,
            max_shared_kind: PrimitiveKind::Surface,
        });
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
        if self.layer_stack.is_empty() {
            while let Some(layer) = self.layers.pop_front() {
                self.insert_layer(layer);
            }
        }
    }

    fn insert_layer(&mut self, layer: Layer) {
        if layer.primitives.is_empty() {
            return;
        }
        let order =
            self.primitive_bounds
                .insert(layer.bounds, layer.max_kind, layer.max_shared_kind);
        for index in layer.primitives.into_iter().flatten() {
            if let PaintOperationKind::Primitive(primitive) = &self.paint_operations[index].kind {
                let mut primitive = primitive.clone();
                self.push_primitive(&mut primitive, order, true);
            }
        }
    }

    pub(crate) fn insert_primitive_scoped(
        &mut self,
        primitive: impl Into<Primitive>,
        clip_stack: &ClipStackSnapshot,
        scale_factor: f32,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let mut primitive = primitive.into();
        let mut clip = self.intern_clip_stack(clip_stack, scale_factor)?;
        if let Primitive::Path(path) = &primitive {
            clip.conservative_bounds = clip
                .conservative_bounds
                .intersect(&path.paint.clip.conservative_bounds);
        }
        if let Primitive::MonochromeSprite(sprite) = &mut primitive {
            sprite.animation = SpriteAnimationId::NONE;
        }
        primitive.set_clip_envelope(clip);
        self.record_primitive(primitive, validity)
    }

    /// Inserts an animated SVG with the same clip and geometry ownership as other primitives.
    /// The journal keeps its starting matrix and its color before the animation's alpha.
    pub(crate) fn insert_animated_sprite_scoped(
        &mut self,
        mut sprite: MonochromeSprite,
        animation: SpriteAnimation,
        clip_stack: &ClipStackSnapshot,
        scale_factor: f32,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        sprite.clip = self.intern_clip_stack(clip_stack, scale_factor)?;
        self.record_animated_sprite(sprite, animation, validity)
    }

    fn record_animated_sprite(
        &mut self,
        mut sprite: MonochromeSprite,
        mut animation: SpriteAnimation,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let animation_id = u32::try_from(self.animations.len())
            .ok()
            .filter(|id| *id != SpriteAnimationId::NONE.0)
            .ok_or(SubtreeGeometryError::DeviceConversion)?;
        sprite.animation = SpriteAnimationId(animation_id);
        animation.base_color = sprite.color;
        animation.sprite_index = usize::MAX;
        animation.journal_index = self.paint_operations.len();
        let journal_index = animation.journal_index;
        self.animations.push(animation);
        let result = self.record_primitive(sprite.into(), validity);
        if self.paint_operations.len() == journal_index {
            self.animations.pop();
        }
        result
    }

    pub(crate) fn insert_quad_partition_scoped(
        &mut self,
        quad: Quad,
        inner: Bounds<ScaledPixels>,
        plain_border: bool,
        clip_stack: &ClipStackSnapshot,
        scale_factor: f32,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let clip = self.intern_clip_stack(clip_stack, scale_factor)?;
        let inherited = clip.conservative_bounds;
        let outer = quad.transform.try_project_bounds(quad.bounds)?;
        let outer = if plain_border {
            outer
        } else {
            let padded = outer.dilate(ScaledPixels(0.5));
            Bounds::from_corners(
                padded.origin.map(|coordinate| coordinate.floor()),
                padded.bottom_right().map(|coordinate| coordinate.ceil()),
            )
        };
        let strips = [
            Bounds::from_corners(outer.origin, point(outer.right(), inner.top())),
            Bounds::from_corners(point(outer.left(), inner.bottom()), outer.bottom_right()),
            Bounds::from_corners(point(outer.left(), inner.top()), inner.bottom_left()),
            Bounds::from_corners(inner.top_right(), point(outer.right(), inner.bottom())),
        ];
        let raster_bounds = strips.map(|strip| inherited.intersect(&strip));
        let interior = inherited.intersect(&inner);
        let safe_end = |raster: ScaledPixels, cut: ScaledPixels, inherited: ScaledPixels| {
            if inherited <= cut {
                raster == inherited
            } else {
                raster <= inherited && (raster.0 - cut.0).abs() <= 0.125
            }
        };
        if strips
            .iter()
            .zip(&raster_bounds)
            .chain([(&inner, &interior)])
            .any(|(strip, raster)| {
                !raster.is_empty()
                    && (!safe_end(raster.right(), strip.right(), inherited.right())
                        || !safe_end(raster.bottom(), strip.bottom(), inherited.bottom()))
            })
        {
            return self.record_primitive(Quad { clip, ..quad }.into(), validity);
        }

        if !quad.background.is_transparent() && !interior.is_empty() {
            self.record_primitive(
                Quad {
                    clip: ClipEnvelope {
                        conservative_bounds: interior,
                        ..clip
                    },
                    border_widths: Edges::default(),
                    corner_radii: Corners::default(),
                    ..quad
                }
                .into(),
                validity.clone(),
            )?;
        }
        let strip_quad = if plain_border {
            Quad {
                background: quad.border_color.into(),
                border_widths: Edges::default(),
                corner_radii: Corners::default(),
                ..quad
            }
        } else {
            quad
        };
        for conservative_bounds in raster_bounds {
            if !conservative_bounds.is_empty() {
                let clip = ClipEnvelope {
                    conservative_bounds,
                    ..clip
                };
                self.record_primitive(Quad { clip, ..strip_quad }.into(), validity.clone())?;
            }
        }
        Ok(())
    }

    fn insert_primitive_with_gpu_clips(
        &mut self,
        mut primitive: Primitive,
        clip_shapes: &[GpuClipShape],
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        let mut clip = self.intern_gpu_clip_stack(clip_shapes)?;
        let recorded_bounds = primitive.clip_envelope().conservative_bounds;
        if clip.conservative_bounds != recorded_bounds {
            clip.conservative_bounds = clip.conservative_bounds.intersect(&recorded_bounds);
        }
        primitive.set_clip_envelope(clip);
        if let Primitive::MonochromeSprite(sprite) = &mut primitive {
            sprite.animation = SpriteAnimationId::NONE;
        }
        self.record_primitive(primitive, validity)
    }

    fn record_primitive(
        &mut self,
        mut primitive: Primitive,
        validity: Option<SubtreeGeometryValidity>,
    ) -> Result<(), SubtreeGeometryError> {
        if !self.push_primitive_for_render(&mut primitive, self.paint_operations.len())? {
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
        journal_index: usize,
    ) -> Result<bool, SubtreeGeometryError> {
        let clipped_bounds = primitive
            .try_visual_bounds()?
            .intersect(&primitive.clip_envelope().conservative_bounds);

        if clipped_bounds.is_empty() {
            return Ok(false);
        }

        let kind = primitive.kind();
        if let Some(&layer_index) = self.layer_stack.last()
            && let Some(layer) = self.layers.get_mut(layer_index)
        {
            layer.bounds = layer.bounds.join(&clipped_bounds);
            layer.max_kind = layer.max_kind.max(kind);
            layer.max_shared_kind = layer.max_shared_kind.min(kind.max_shared_kind());
            match layer.primitives.last_mut() {
                Some(range) if range.end == journal_index => range.end += 1,
                _ => layer.primitives.push(journal_index..journal_index + 1),
            }
        } else {
            let order = self
                .primitive_bounds
                .insert(clipped_bounds, kind, kind.max_shared_kind());
            self.push_primitive(primitive, order, false);
        }
        Ok(true)
    }

    fn push_primitive(&mut self, primitive: &mut Primitive, order: DrawOrder, in_layer: bool) {
        let clipped_bounds = primitive
            .try_visual_bounds()
            .expect("accepted scene primitive must remain representable")
            .intersect(&primitive.clip_envelope().conservative_bounds);
        let keep_sprite_order = |transform: PrimitiveTransform, bounds| {
            in_layer
                || !transform.has_precise_raster_coordinates(bounds)
                || [
                    clipped_bounds.left(),
                    clipped_bounds.top(),
                    clipped_bounds.right(),
                    clipped_bounds.bottom(),
                ]
                .into_iter()
                .any(|edge| (edge.0.fract().abs() - 0.5).abs() <= 0.125)
        };
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
                self.paths.push(path.clone());
            }
            Primitive::Underline(underline) => {
                underline.order = order;
                self.underlines.push(*underline);
            }
            Primitive::MonochromeSprite(sprite) => {
                if sprite.animation != SpriteAnimationId::NONE
                    || sprite.transformation != TransformationMatrix::unit()
                    || keep_sprite_order(sprite.transform, sprite.bounds)
                {
                    self.legacy_sprite_order_mut(order).monochrome = true;
                }
                sprite.order = order;
                if let Some(animation) = self.animations.get_mut(sprite.animation.0 as usize) {
                    animation.sprite_index = self.monochrome_sprites.len();
                }
                self.monochrome_sprites.push(*sprite);
            }
            Primitive::SubpixelSprite(sprite) => {
                if keep_sprite_order(sprite.transform, sprite.bounds) {
                    self.legacy_sprite_order_mut(order).subpixel = true;
                }
                sprite.order = order;
                self.subpixel_sprites.push(*sprite);
            }
            Primitive::PolychromeSprite(sprite) => {
                if keep_sprite_order(sprite.transform, sprite.bounds) {
                    self.legacy_sprite_order_mut(order).polychrome = true;
                }
                sprite.order = order;
                self.polychrome_sprites.push(*sprite);
            }
            Primitive::Surface(surface) => {
                surface.order = order;
                self.surfaces.push(surface.clone());
            }
        }
    }

    fn legacy_sprite_order_mut(&mut self, order: DrawOrder) -> &mut LegacySpriteOrder {
        let index = order as usize;
        if index >= self.legacy_sprite_order.len() {
            self.legacy_sprite_order
                .resize(index + 1, LegacySpriteOrder::default());
        }
        &mut self.legacy_sprite_order[index]
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
                    PaintOperationKind::Primitive(primitive) => {
                        let clips = prev_scene.clip_shapes_for(primitive.clip_envelope())?;
                        if let Primitive::MonochromeSprite(sprite) = primitive
                            && let Some(animation) =
                                prev_scene.animations.get(sprite.animation.0 as usize)
                        {
                            let mut sprite = *sprite;
                            let mut clip = self.intern_gpu_clip_stack(clips)?;
                            clip.conservative_bounds = clip
                                .conservative_bounds
                                .intersect(&sprite.clip.conservative_bounds);
                            sprite.clip = clip;
                            self.record_animated_sprite(sprite, *animation, replayed_validity)?;
                        } else {
                            self.insert_primitive_with_gpu_clips(
                                primitive.clone(),
                                clips,
                                replayed_validity,
                            )?;
                        }
                    }
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
        let keys = &mut Vec::new();
        sort_in_place(&mut self.shadows, keys, |shadow| (shadow.order, 0, 0, 0));
        sort_in_place(&mut self.quads, keys, |quad| (quad.order, 0, 0, 0));
        self.paths.sort_by_key(|path| path.order);
        sort_in_place(&mut self.underlines, keys, |underline| {
            (underline.order, 0, 0, 0)
        });
        sort_sprites(
            &mut self.monochrome_sprites,
            keys,
            &self.legacy_sprite_order,
            |kinds| kinds.monochrome,
            |sprite| (sprite.order, &sprite.tile),
        );
        sort_sprites(
            &mut self.subpixel_sprites,
            keys,
            &self.legacy_sprite_order,
            |kinds| kinds.subpixel,
            |sprite| (sprite.order, &sprite.tile),
        );
        sort_sprites(
            &mut self.polychrome_sprites,
            keys,
            &self.legacy_sprite_order,
            |kinds| kinds.polychrome,
            |sprite| (sprite.order, &sprite.tile),
        );
        self.surfaces.sort_by_key(|surface| surface.order);
        self.bind_animations();
    }

    fn bind_animations(&mut self) {
        for animation in &mut self.animations {
            animation.sprite_index = usize::MAX;
        }
        for (index, sprite) in self.monochrome_sprites.iter().enumerate() {
            if let Some(animation) = self.animations.get_mut(sprite.animation.0 as usize) {
                animation.sprite_index = index;
            }
        }
    }

    fn journal_checkpoint(&self) -> SceneJournalCheckpoint {
        SceneJournalCheckpoint {
            paint_operations_len: self.paint_operations.len(),
            clip_shapes_len: self.clip_shapes.len(),
            clip_stack_ranges_len: self.clip_stack_ranges.len(),
            animations_len: self.animations.len(),
        }
    }

    fn rollback_to(&mut self, checkpoint: SceneJournalCheckpoint) {
        self.paint_operations
            .truncate(checkpoint.paint_operations_len);
        self.clip_shapes.truncate(checkpoint.clip_shapes_len);
        self.clip_stack_ranges
            .truncate(checkpoint.clip_stack_ranges_len);
        self.animations.truncate(checkpoint.animations_len);
        self.rebuild_render_state_from_journal();
    }

    fn rebuild_render_state_from_journal(&mut self) {
        // A deferred scene may already have been delivered at a sampled pose. Rebuilding to
        // filter an invalid sibling must preserve that sample while the journal keeps its base.
        let mut poses = vec![None; self.animations.len()];
        for sprite in &self.monochrome_sprites {
            if let Some(pose) = poses.get_mut(sprite.animation.0 as usize) {
                *pose = Some((sprite.transformation, sprite.color));
            }
        }
        self.clear_render_state();
        for index in 0..self.paint_operations.len() {
            let operation = self.paint_operations[index].clone();
            match operation.kind {
                PaintOperationKind::Primitive(mut primitive) => {
                    if operation
                        .validity
                        .as_ref()
                        .is_some_and(|validity| !validity.is_valid())
                    {
                        continue;
                    }
                    self.push_primitive_for_render(&mut primitive, index)
                        .expect("accepted scene primitive must remain representable");
                }
                PaintOperationKind::StartLayer(_) => self.push_layer_for_render(),
                PaintOperationKind::EndLayer => self.pop_layer_for_render(),
            }
        }
        for sprite in &mut self.monochrome_sprites {
            if let Some(Some((transformation, color))) = poses.get(sprite.animation.0 as usize) {
                sprite.transformation = *transformation;
                sprite.color = *color;
            }
        }
    }

    fn clear_render_state(&mut self) {
        for animation in &mut self.animations {
            animation.sprite_index = usize::MAX;
        }
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.layers.clear();
        self.legacy_sprite_order.clear();
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

/// Sorts small `(key, index)` pairs; insertion indices preserve stable ties.
/// Follows permutation cycles to move each primitive into place once.
fn sort_in_place<T: Copy, K: Ord>(
    primitives: &mut [T],
    keys: &mut Vec<(K, usize)>,
    key: impl Fn(&T) -> K,
) {
    if primitives.is_sorted_by_key(&key) {
        return;
    }
    keys.clear();
    keys.extend(
        primitives
            .iter()
            .enumerate()
            .map(|(index, primitive)| (key(primitive), index)),
    );
    keys.sort_unstable();

    // Each target stores its source index; self-indices mark visited slots.
    for start in 0..primitives.len() {
        if keys[start].1 == start {
            continue;
        }
        let displaced = primitives[start];
        let mut target = start;
        loop {
            let source = keys[target].1;
            keys[target].1 = target;
            if source == start {
                primitives[target] = displaced;
                break;
            }
            primitives[target] = primitives[source];
            target = source;
        }
    }
}

/// The depth step between consecutive quads in a frame with `quad_count` quads.
pub fn quad_depth_scale(quad_count: usize) -> f32 {
    let denominator = (quad_count.min((1 << 24) - 1) + 1).next_power_of_two();
    1.0 / denominator as f32
}

/// Maps a quad's index in [`Scene::quads`] to depth, with greater values closer.
pub fn quad_depth(quad_id: u32, scale: f32) -> f32 {
    ((u64::from(quad_id) + 1) as f32 * scale).min(1.0)
}

fn sort_sprites<S: Copy>(
    sprites: &mut [S],
    keys: &mut Vec<((DrawOrder, u32, u32, u32), usize)>,
    legacy_sprite_order: &[LegacySpriteOrder],
    needs_legacy_order: impl Fn(&LegacySpriteOrder) -> bool,
    order_and_tile: impl Fn(&S) -> (DrawOrder, &AtlasTile),
) {
    sort_in_place(sprites, keys, |sprite| {
        let (order, tile) = order_and_tile(sprite);
        if legacy_sprite_order
            .get(order as usize)
            .is_some_and(&needs_legacy_order)
        {
            (order, tile.tile_id.0, 0, 0)
        } else {
            let AtlasTextureInstanceId {
                texture_id,
                generation,
            } = tile.texture_instance();
            (order, texture_id.index, texture_id.kind as u32, generation)
        }
    });
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

impl PrimitiveKind {
    fn max_shared_kind(self) -> Self {
        match self {
            Self::MonochromeSprite => Self::Underline,
            Self::SubpixelSprite => Self::MonochromeSprite,
            Self::PolychromeSprite => Self::SubpixelSprite,
            kind => kind,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PaintOperation {
    kind: PaintOperationKind,
    validity: Option<SubtreeGeometryValidity>,
}

#[derive(Clone)]
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
    Path(PaintedPath),
    Underline(Underline),
    MonochromeSprite(MonochromeSprite),
    SubpixelSprite(SubpixelSprite),
    PolychromeSprite(PolychromeSprite),
    Surface(PaintSurface),
}

#[expect(missing_docs)]
impl Primitive {
    pub(crate) fn kind(&self) -> PrimitiveKind {
        match self {
            Self::Shadow(_) => PrimitiveKind::Shadow,
            Self::Quad(_) => PrimitiveKind::Quad,
            Self::Path(_) => PrimitiveKind::Path,
            Self::Underline(_) => PrimitiveKind::Underline,
            Self::MonochromeSprite(_) => PrimitiveKind::MonochromeSprite,
            Self::SubpixelSprite(_) => PrimitiveKind::SubpixelSprite,
            Self::PolychromeSprite(_) => PrimitiveKind::PolychromeSprite,
            Self::Surface(_) => PrimitiveKind::Surface,
        }
    }

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
            Primitive::Path(path) => path.paint.clip,
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
            Primitive::Path(path) => path.paint.clip = clip,
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
            Primitive::Path(path) => path.renderer_transform(),
            Primitive::Underline(underline) => underline.transform,
            Primitive::MonochromeSprite(sprite) => sprite.transform,
            Primitive::SubpixelSprite(sprite) => sprite.transform,
            Primitive::PolychromeSprite(sprite) => sprite.transform,
            Primitive::Surface(surface) => surface.transform,
        }
    }

    fn local_raster_bounds(&self) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        match self {
            Primitive::Shadow(shadow) => Ok(shadow.local_raster_bounds()),
            Primitive::MonochromeSprite(sprite) => {
                if ![
                    sprite.color.h,
                    sprite.color.s,
                    sprite.color.l,
                    sprite.color.a,
                ]
                .into_iter()
                .all(f32::is_finite)
                {
                    return Err(SubtreeTransformError::UnrepresentableResult);
                }
                let bounds = sprite.transformation.try_transform_bounds(sprite.bounds)?;
                if sprite.animation != SpriteAnimationId::NONE && bounds.is_empty() {
                    let projected = sprite.transform.try_project_bounds(bounds)?;
                    if !projected.right().0.is_finite() || !projected.bottom().0.is_finite() {
                        return Err(SubtreeTransformError::UnrepresentableResult);
                    }
                    Ok(sprite.bounds)
                } else {
                    Ok(bounds)
                }
            }
            _ => Ok(*self.bounds()),
        }
    }

    pub fn try_visual_bounds(&self) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        if let Self::Path(path) = self {
            PaintedPath::try_project_bounds(path.paint.bounds, path.paint.transform)?;
        }
        let bounds = self
            .transform()
            .try_project_bounds(self.local_raster_bounds()?)?;
        if matches!(self, Self::MonochromeSprite(_))
            && (!bounds.right().0.is_finite() || !bounds.bottom().0.is_finite())
        {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        Ok(bounds)
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
    paths_iter: Peekable<slice::Iter<'a, PaintedPath>>,
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

impl PrimitiveBatch {
    /// Returns a diagnostic label with the primitive count and the atlas instance for sprites.
    pub fn label(&self) -> String {
        fn sprite_label(kind: &str, count: usize, texture: AtlasTextureInstanceId) -> String {
            format!(
                "{kind} sprites ({count}) on atlas {} ({:?}, generation {})",
                texture.texture_id.index, texture.texture_id.kind, texture.generation,
            )
        }

        match self {
            Self::Shadows(range) => format!("shadows ({})", range.len()),
            Self::Quads(range) => format!("quads ({})", range.len()),
            Self::Paths(range) => format!("paths ({})", range.len()),
            Self::Underlines(range) => format!("underlines ({})", range.len()),
            Self::MonochromeSprites { texture, range } => {
                sprite_label("monochrome", range.len(), *texture)
            }
            Self::SubpixelSprites { texture, range } => {
                sprite_label("subpixel", range.len(), *texture)
            }
            Self::PolychromeSprites { texture, range } => {
                sprite_label("polychrome", range.len(), *texture)
            }
            Self::Surfaces(range) => format!("surfaces ({})", range.len()),
        }
    }
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
    pub(crate) const RASTER_COORDINATE_LIMIT: f32 = 32_768.0;

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

    /// Bounds the f32 error of projection, including large opposing translations, and local
    /// interpolation. Keep this domain in sync with quad vertex clipping in the WGPU shader.
    pub(crate) fn has_precise_raster_coordinates(self, bounds: Bounds<ScaledPixels>) -> bool {
        [
            (
                bounds.origin.x.0,
                bounds.size.width.0,
                self.scale_x,
                self.translation_x,
            ),
            (
                bounds.origin.y.0,
                bounds.size.height.0,
                self.scale_y,
                self.translation_y,
            ),
        ]
        .into_iter()
        .all(|(origin, extent, scale, translation)| {
            let local_magnitude = origin.abs() + extent;
            local_magnitude <= Self::RASTER_COORDINATE_LIMIT
                && local_magnitude * scale + translation.abs() <= Self::RASTER_COORDINATE_LIMIT
        })
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
    pub wavy: PaddedBool32,
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

/// An affine transformation of an SVG's raster, independent of layout and hit testing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct TransformationMatrix {
    /// Row-major rotation and scale matrix.
    pub rotation_scale: [[f32; 2]; 2],
    /// Translation in device-scaled pixels.
    pub translation: [f32; 2],
}

impl Eq for TransformationMatrix {}

impl TransformationMatrix {
    /// The identity matrix.
    pub fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }

    /// Translates the origin before applying this matrix.
    pub fn translate(self, offset: Point<ScaledPixels>) -> Self {
        self.compose(Self {
            translation: [offset.x.0, offset.y.0],
            ..Self::unit()
        })
    }

    /// Rotates clockwise around the origin before applying this matrix.
    pub fn rotate(self, angle: Radians) -> Self {
        let (sin, cos) = angle.0.sin_cos();
        self.compose(Self {
            rotation_scale: [[cos, -sin], [sin, cos]],
            translation: [0.0, 0.0],
        })
    }

    /// Scales around the origin before applying this matrix.
    pub fn scale(self, scale: Size<f32>) -> Self {
        self.compose(Self {
            rotation_scale: [[scale.width, 0.0], [0.0, scale.height]],
            translation: [0.0, 0.0],
        })
    }

    /// Composes the matrices, applying `other` first and then `self`.
    #[inline]
    pub fn compose(self, other: Self) -> Self {
        if other == Self::unit() {
            return self;
        }
        Self {
            rotation_scale: [
                [
                    self.rotation_scale[0][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][0],
                    self.rotation_scale[0][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][1],
                ],
                [
                    self.rotation_scale[1][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][0],
                    self.rotation_scale[1][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][1],
                ],
            ],
            translation: [
                self.translation[0]
                    + self.rotation_scale[0][0] * other.translation[0]
                    + self.rotation_scale[0][1] * other.translation[1],
                self.translation[1]
                    + self.rotation_scale[1][0] * other.translation[0]
                    + self.rotation_scale[1][1] * other.translation[1],
            ],
        }
    }

    pub(crate) fn try_transform_bounds(
        self,
        bounds: Bounds<ScaledPixels>,
    ) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        let values = [
            bounds.origin.x.0,
            bounds.origin.y.0,
            bounds.size.width.0,
            bounds.size.height.0,
            bounds.right().0,
            bounds.bottom().0,
        ];
        if !self
            .rotation_scale
            .as_flattened()
            .iter()
            .chain(&self.translation)
            .chain(&values)
            .all(|value| value.is_finite())
            || bounds.size.width.0 < 0.0
            || bounds.size.height.0 < 0.0
        {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        if self == Self::unit() {
            return Ok(bounds);
        }
        let [[xx, xy], [yx, yy]] = self.rotation_scale;
        let [translate_x, translate_y] = self.translation;
        let corners = [
            bounds.origin,
            bounds.top_right(),
            bounds.bottom_left(),
            bounds.bottom_right(),
        ]
        .map(|corner| {
            point(
                ScaledPixels(xx * corner.x.0 + xy * corner.y.0 + translate_x),
                ScaledPixels(yx * corner.x.0 + yy * corner.y.0 + translate_y),
            )
        });
        if corners
            .iter()
            .any(|corner| !corner.x.0.is_finite() || !corner.y.0.is_finite())
        {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        let [first, rest @ ..] = corners;
        let top_left = rest.iter().fold(first, |min, corner| min.min(corner));
        let bottom_right = rest.iter().fold(first, |max, corner| max.max(corner));
        let transformed = Bounds::from_corners(top_left, bottom_right);
        if !transformed.size.width.0.is_finite() || !transformed.size.height.0.is_finite() {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        Ok(transformed)
    }

    /// Applies the matrix to a point expressed in the matrix's coordinate space.
    pub fn apply(&self, position: Point<Pixels>) -> Point<Pixels> {
        let [[xx, xy], [yx, yy]] = self.rotation_scale;
        point(
            Pixels(xx * position.x.0 + xy * position.y.0 + self.translation[0]),
            Pixels(yx * position.x.0 + yy * position.y.0 + self.translation[1]),
        )
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct MonochromeSprite {
    pub order: DrawOrder,
    pub animation: SpriteAnimationId,
    pub tile: AtlasTile,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Hsla,
    pub(crate) transform: PrimitiveTransform,
    pub transformation: TransformationMatrix,
}

/// A scene-local animation identifier stored in a four-byte sprite slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SpriteAnimationId(u32);

impl SpriteAnimationId {
    /// The sprite has no animation.
    pub const NONE: Self = Self(u32::MAX);
}

impl Default for SpriteAnimationId {
    fn default() -> Self {
        Self::NONE
    }
}

/// A timed SVG transformation applied to the renderer projection before presenting.
#[derive(Copy, Clone, Debug)]
pub(crate) struct SpriteAnimation {
    transformation: TimedTransformation,
    center: Point<Pixels>,
    scale_factor: f32,
    base_color: Hsla,
    sprite_index: usize,
    journal_index: usize,
}

impl SpriteAnimation {
    pub(crate) fn new(
        transformation: TimedTransformation,
        center: Point<Pixels>,
        scale_factor: f32,
    ) -> Self {
        Self {
            transformation,
            center,
            scale_factor,
            base_color: Hsla::default(),
            sprite_index: usize::MAX,
            journal_index: usize::MAX,
        }
    }
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
    pub tile: AtlasTile,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Hsla,
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
    /// Renderer flags: linear sampled RGB (1), premultiplied sampled RGB (2), opaque (4),
    /// nearest sampling (8), and imported-image region clamping (16). Ordinary images use zero.
    pub pad: u32,
    pub tile: AtlasTile,
    pub grayscale: PaddedBool32,
    pub opacity: f32,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub corner_radii: Corners<ScaledPixels>,
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
    #[cfg(any(target_os = "macos", target_os = "ios"))]
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

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathCurve {
    pub p0: Point<f32>,
    pub p1: Point<f32>,
    pub a: Point<f32>,
    pub b: Point<f32>,
    pub sign: f32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathPaint {
    /// Device-scaled local bounds used for evaluating the background.
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub color: Background,
    pub even_odd: PaddedBool32,
    pub pad: u32,
    /// Projects background coordinates into the device space of the baked curves.
    pub transform: PrimitiveTransform,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathTile {
    pub paint: u32,
    pub curve_start: u32,
    pub curve_count: u32,
    pub backdrop: i32,
    pub top_left: Point<f32>,
    pub bottom_right: Point<f32>,
    pub reference: Point<f32>,
    pub curves_top_left: Point<f32>,
    pub curves_bottom_right: Point<f32>,
}

#[derive(Clone, Debug)]
#[expect(missing_docs)]
pub struct PaintedPath {
    order: DrawOrder,
    bounds: Bounds<ScaledPixels>,
    tiles: Rc<PathTiles>,
    paint: PathPaint,
}

#[expect(missing_docs)]
impl PaintedPath {
    pub(crate) fn try_project_bounds(
        bounds: Bounds<ScaledPixels>,
        transform: PrimitiveTransform,
    ) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        let is_valid = |bounds: Bounds<ScaledPixels>| {
            bounds.size.width.0 >= 0.0
                && bounds.size.height.0 >= 0.0
                && [bounds.left(), bounds.top(), bounds.right(), bounds.bottom()]
                    .into_iter()
                    .all(|edge| edge.0.is_finite())
        };
        if !is_valid(bounds) {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        let projected = transform.try_project_bounds(bounds)?;
        if !is_valid(projected) {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        Ok(projected)
    }

    /// Bins `path` into device tiles, to be filled with `color` within `mask_bounds`.
    pub fn new(
        path: Path,
        scale_factor: f32,
        mask_bounds: Bounds<Pixels>,
        color: Background,
    ) -> Result<Self, SubtreeTransformError> {
        Self::new_transformed(
            path,
            scale_factor,
            PrimitiveTransform::IDENTITY,
            mask_bounds,
            color,
        )
    }

    pub(crate) fn new_transformed(
        path: Path,
        scale_factor: f32,
        transform: PrimitiveTransform,
        mask_bounds: Bounds<Pixels>,
        color: Background,
    ) -> Result<Self, SubtreeTransformError> {
        let bounds = path.try_bounds(scale_factor)?;
        Self::try_project_bounds(bounds, transform)?;
        let conservative_bounds = mask_bounds.scale(scale_factor);
        Self::try_project_bounds(conservative_bounds, PrimitiveTransform::IDENTITY)?;
        let mut tiles = path.try_bin_transformed(scale_factor, transform)?;
        tiles.retain_visible(conservative_bounds);
        let paint = PathPaint {
            bounds,
            clip: ClipEnvelope {
                conservative_bounds,
                ..Default::default()
            },
            color,
            even_odd: path.even_odd().into(),
            pad: 0,
            transform,
        };
        Ok(Self {
            order: 0,
            bounds: tiles.painted_bounds(),
            tiles: Rc::new(tiles),
            paint,
        })
    }

    #[doc(hidden)]
    pub const fn renderer_transform(&self) -> PrimitiveTransform {
        PrimitiveTransform::IDENTITY
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.tiles().len()
    }

    pub fn append_to(
        &self,
        paints: &mut Vec<PathPaint>,
        curves: &mut Vec<PathCurve>,
        tiles: &mut Vec<PathTile>,
    ) {
        let paint_index = paints.len() as u32;
        let curve_start = curves.len() as u32;
        paints.push(self.paint);
        curves.extend_from_slice(self.tiles.curves());
        tiles.extend(self.tiles.tiles().iter().map(|tile| PathTile {
            paint: paint_index,
            curve_start: curve_start + tile.curve_start,
            ..*tile
        }));
    }
}

impl From<PaintedPath> for Primitive {
    fn from(path: PaintedPath) -> Self {
        Primitive::Path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AtlasTextureId, AtlasTextureKind, SubtreeClip, SubtreeClipError, TileId, Transformation,
        hsla, px, size,
    };
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn sort_in_place_matches_stable_sort_and_reuses_scratch() {
        let mut keys = Vec::new();
        for orders in [
            &[][..],
            &[0][..],
            &[0, 0, 1, 1, u32::MAX][..],
            &[2, 0, 1][..],
            &[3, 1, 2, 0, 3, 1][..],
            &[u32::MAX, 0, u32::MAX, 0][..],
            &[0, 0, 1][..],
            &[][..],
        ] {
            let mut primitives = orders.iter().copied().enumerate().collect::<Vec<_>>();
            let mut expected = primitives.clone();
            expected.sort_by_key(|(_, order)| *order);
            let capacity = keys.capacity();

            sort_in_place(&mut primitives, &mut keys, |(_, order)| *order);

            assert_eq!(primitives, expected);
            if orders.is_sorted() {
                assert_eq!(keys.capacity(), capacity);
            }
        }
    }

    #[test]
    fn gpu_boolean_fields_occupy_initialized_four_byte_slots() {
        assert_eq!(std::mem::size_of::<PaddedBool32>(), 4);
        assert_eq!(std::mem::align_of::<PaddedBool32>(), 4);
        for (value, expected) in [
            (PaddedBool32::default(), 0u32),
            (false.into(), 0),
            (true.into(), 1),
        ] {
            assert_eq!(value.0, expected);
            assert_eq!(value.0.to_ne_bytes(), expected.to_ne_bytes());
        }
        let sprite = PolychromeSprite {
            order: 0,
            pad: 0,
            tile: AtlasTile {
                texture_id: AtlasTextureId {
                    index: 0,
                    kind: AtlasTextureKind::Polychrome,
                },
                tile_id: TileId(0),
                padding: 0,
                bounds: Bounds::new(Point::default(), Size::default()),
                texture_generation: 1,
                texture_generation_padding: 0,
            },
            grayscale: false.into(),
            opacity: 1.0,
            bounds: scaled_bounds(0.0, 0.0, 16.0, 16.0),
            clip: ClipEnvelope::default(),
            corner_radii: Corners::default(),
            transform: PrimitiveTransform::IDENTITY,
        };
        let underline = Underline {
            order: 0,
            pad: 0,
            bounds: scaled_bounds(0.0, 0.0, 16.0, 4.0),
            clip: ClipEnvelope::default(),
            color: Hsla::default(),
            thickness: ScaledPixels(1.0),
            wavy: false.into(),
            transform: PrimitiveTransform::IDENTITY,
        };
        assert_eq!(std::mem::size_of_val(&sprite.grayscale), 4);
        assert_eq!(std::mem::size_of_val(&underline.wavy), 4);
        assert_eq!(std::mem::size_of::<Underline>(), 88);
        assert_eq!(std::mem::offset_of!(Underline, wavy), 68);
        assert_eq!(std::mem::offset_of!(Underline, transform), 72);
    }

    #[test]
    fn sprite_gpu_layouts_colocate_order_and_tile() {
        assert_eq!(
            [
                size_of::<MonochromeSprite>(),
                align_of::<MonochromeSprite>(),
                offset_of!(MonochromeSprite, order),
                offset_of!(MonochromeSprite, animation),
                offset_of!(MonochromeSprite, tile),
                offset_of!(MonochromeSprite, bounds),
                offset_of!(MonochromeSprite, clip),
                offset_of!(MonochromeSprite, color),
                offset_of!(MonochromeSprite, transform),
                offset_of!(MonochromeSprite, transformation),
            ],
            [144, 4, 0, 4, 8, 48, 64, 88, 104, 120],
        );
        assert_eq!(size_of::<TransformationMatrix>(), 24);
        assert_eq!(align_of::<TransformationMatrix>(), 4);
        assert_eq!(offset_of!(TransformationMatrix, rotation_scale), 0);
        assert_eq!(offset_of!(TransformationMatrix, translation), 16);
        assert_eq!(size_of::<SpriteAnimationId>(), 4);
        assert_eq!(SpriteAnimationId::default(), SpriteAnimationId::NONE);
        assert_eq!(
            [
                size_of::<SubpixelSprite>(),
                align_of::<SubpixelSprite>(),
                offset_of!(SubpixelSprite, order),
                offset_of!(SubpixelSprite, pad),
                offset_of!(SubpixelSprite, tile),
                offset_of!(SubpixelSprite, bounds),
                offset_of!(SubpixelSprite, clip),
                offset_of!(SubpixelSprite, color),
                offset_of!(SubpixelSprite, transform),
            ],
            [120, 4, 0, 4, 8, 48, 64, 88, 104],
        );
        assert_eq!(
            [
                size_of::<PolychromeSprite>(),
                align_of::<PolychromeSprite>(),
                offset_of!(PolychromeSprite, order),
                offset_of!(PolychromeSprite, pad),
                offset_of!(PolychromeSprite, tile),
                offset_of!(PolychromeSprite, grayscale),
                offset_of!(PolychromeSprite, opacity),
                offset_of!(PolychromeSprite, bounds),
                offset_of!(PolychromeSprite, clip),
                offset_of!(PolychromeSprite, corner_radii),
                offset_of!(PolychromeSprite, transform),
            ],
            [128, 4, 0, 4, 8, 48, 52, 56, 72, 96, 112],
        );
    }

    fn scaled_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
        Bounds::new(
            point(ScaledPixels(x), ScaledPixels(y)),
            Size::new(ScaledPixels(width), ScaledPixels(height)),
        )
    }

    fn logical_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), Size::new(px(width), px(height)))
    }

    fn test_quad(x: f32) -> Quad {
        Quad {
            bounds: scaled_bounds(x, 0.0, 10.0, 10.0),
            ..Default::default()
        }
    }

    fn test_underline(x: f32) -> Underline {
        Underline {
            order: 0,
            pad: 0,
            bounds: scaled_bounds(x, 0.0, 10.0, 10.0),
            clip: ClipEnvelope::default(),
            color: Hsla::default(),
            thickness: ScaledPixels(1.0),
            wavy: false.into(),
            transform: PrimitiveTransform::IDENTITY,
        }
    }

    fn insert_test_primitive(scene: &mut Scene, primitive: impl Into<Primitive>) {
        scene
            .insert_primitive_scoped(
                primitive,
                &ClipStackSnapshot::root(logical_bounds(-100.0, -100.0, 1000.0, 1000.0)),
                1.0,
                None,
            )
            .unwrap();
    }

    fn batch_labels(scene: &mut Scene) -> Vec<String> {
        scene.finish();
        scene.batches().map(|batch| batch.label()).collect()
    }

    #[test]
    fn layers_are_drawn_in_paint_order() {
        let mut scene = Scene::default();
        insert_test_primitive(&mut scene, test_quad(0.0));
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_underline(0.0));
        insert_test_primitive(&mut scene, test_underline(5.0));
        scene.pop_layer();
        insert_test_primitive(&mut scene, test_quad(5.0));
        assert_eq!(
            batch_labels(&mut scene),
            ["quads (1)", "underlines (2)", "quads (1)"]
        );

        let mut scene = Scene::default();
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_underline(0.0));
        scene.pop_layer();
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_quad(5.0));
        scene.pop_layer();
        assert_eq!(batch_labels(&mut scene), ["underlines (1)", "quads (1)"]);
    }

    #[test]
    fn nested_layers_are_drawn_above_their_parents() {
        let mut scene = Scene::default();
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_underline(0.0));
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_quad(5.0));
        scene.pop_layer();
        insert_test_primitive(&mut scene, test_underline(5.0));
        scene.pop_layer();
        assert_eq!(batch_labels(&mut scene), ["underlines (2)", "quads (1)"]);

        let mut scene = Scene::default();
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_quad(0.0));
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_quad(5.0));
        scene.pop_layer();
        insert_test_primitive(&mut scene, test_quad(8.0));
        scene.pop_layer();
        assert_eq!(batch_labels(&mut scene), ["quads (3)"]);
        assert_eq!(
            scene
                .quads
                .iter()
                .map(|quad| quad.bounds)
                .collect::<Vec<_>>(),
            [
                test_quad(0.0).bounds,
                test_quad(8.0).bounds,
                test_quad(5.0).bounds
            ]
        );

        let mut scene = Scene::default();
        scene.push_layer(Bounds::default());
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_underline(0.0));
        scene.pop_layer();
        scene.pop_layer();
        scene.push_layer(Bounds::default());
        insert_test_primitive(&mut scene, test_quad(5.0));
        scene.pop_layer();
        assert_eq!(batch_labels(&mut scene), ["underlines (1)", "quads (1)"]);
    }

    #[test]
    fn layers_use_actual_clipped_union_instead_of_the_bounds_hint() {
        let mut scene = Scene::default();
        insert_test_primitive(&mut scene, test_quad(0.0));
        scene.push_layer(scaled_bounds(-100.0, -100.0, 1000.0, 1000.0));
        scene
            .insert_primitive_scoped(
                Underline {
                    bounds: scaled_bounds(0.0, 0.0, 500.0, 10.0),
                    ..test_underline(0.0)
                },
                &ClipStackSnapshot::root(logical_bounds(200.0, 0.0, 10.0, 10.0)),
                1.0,
                None,
            )
            .unwrap();
        scene.pop_layer();
        insert_test_primitive(&mut scene, test_quad(0.0));
        assert_eq!(batch_labels(&mut scene), ["quads (2)", "underlines (1)"]);
    }

    #[test]
    fn invalid_layer_operations_keep_journal_indices_and_structural_balance() {
        let invalidity = SubtreeGeometryValidity::new(None);
        let clip = ClipStackSnapshot::root(logical_bounds(-100.0, -100.0, 1000.0, 1000.0));
        let mut scene = Scene::default();
        scene
            .insert_primitive_scoped(test_quad(0.0), &clip, 1.0, Some(invalidity.clone()))
            .unwrap();
        scene.push_layer(Bounds::default());
        scene
            .insert_primitive_scoped(test_quad(0.0), &clip, 1.0, Some(invalidity.clone()))
            .unwrap();
        insert_test_primitive(&mut scene, test_underline(0.0));
        scene.push_layer_scoped(Bounds::default(), Some(invalidity.clone()));
        scene
            .insert_primitive_scoped(test_quad(8.0), &clip, 1.0, Some(invalidity.clone()))
            .unwrap();
        insert_test_primitive(&mut scene, test_quad(5.0));
        scene.pop_layer_scoped(Some(invalidity.clone()));
        insert_test_primitive(&mut scene, test_underline(5.0));
        scene.pop_layer();
        let journal_len = scene.journal_len();
        let mut copied = scene.clone();
        invalidity.invalidate(SubtreeTransformError::UnrepresentableResult);

        for scene in [&mut scene, &mut copied] {
            assert_eq!(batch_labels(scene), ["underlines (2)", "quads (1)"]);
            assert_eq!(scene.quads[0].bounds, test_quad(5.0).bounds);
            assert_eq!(scene.journal_len(), journal_len);
            assert!(scene.layer_stack.is_empty());
            assert!(scene.layers.is_empty());
            let mut replayed = Scene::default();
            replayed.replay(0..journal_len, scene, None).unwrap();
            assert_eq!(batch_labels(&mut replayed), ["underlines (2)", "quads (1)"]);
            assert_eq!(replayed.journal_len(), journal_len);
        }
    }

    #[test]
    fn cloned_open_layers_own_their_journal_and_pending_primitive_ranges() {
        let mut source = Scene::default();
        source.push_layer(Bounds::default());
        insert_test_primitive(&mut source, test_underline(0.0));
        source.push_layer(Bounds::default());
        insert_test_primitive(&mut source, test_quad(5.0));
        let mut copied = source.clone();
        source.clear();
        copied.pop_layer();
        insert_test_primitive(&mut copied, test_underline(5.0));
        copied.pop_layer();
        assert_eq!(batch_labels(&mut copied), ["underlines (2)", "quads (1)"]);
        assert_eq!(copied.journal_len(), 7);
        assert!(!source.has_primitives());
    }

    #[test]
    fn retained_fragment_replay_failure_restores_open_layers_and_original_indices() {
        let mut source = Scene::default();
        let source_clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 100.0, 100.0));
        source.push_layer(Bounds::default());
        for x in [5.0, 8.0] {
            source
                .insert_primitive_scoped(test_quad(x), &source_clip, 1.0, None)
                .unwrap();
        }
        source.pop_layer();
        let (mut fragment, _) = source
            .retained_visual_fragment(0..source.journal_len())
            .unwrap();
        let PaintOperationKind::Primitive(primitive) = &mut fragment.paint_operations[2].kind
        else {
            panic!("fragment must retain the second primitive");
        };
        let mut invalid_clip = primitive.clip_envelope();
        invalid_clip.first_clip = u32::MAX;
        primitive.set_clip_envelope(invalid_clip);

        let invalidity = SubtreeGeometryValidity::new(None);
        let clip = ClipStackSnapshot::root(logical_bounds(-100.0, -100.0, 1000.0, 1000.0));
        let mut target = Scene::default();
        target
            .insert_primitive_scoped(test_quad(0.0), &clip, 1.0, Some(invalidity.clone()))
            .unwrap();
        target.push_layer(Bounds::default());
        target
            .insert_primitive_scoped(test_quad(8.0), &clip, 1.0, Some(invalidity.clone()))
            .unwrap();
        insert_test_primitive(&mut target, test_underline(0.0));
        invalidity.invalidate(SubtreeTransformError::UnrepresentableResult);
        let journal_len = target.journal_len();
        let clips = target.clip_shapes().to_vec();
        assert_eq!(
            target.replay(0..fragment.journal_len(), &fragment, None),
            Err(SubtreeGeometryError::DeviceConversion)
        );
        assert_eq!(target.journal_len(), journal_len);
        assert_eq!(target.clip_shapes(), clips);
        assert_eq!(target.layer_stack.len(), 1);
        assert_eq!(target.layers.len(), 1);
        insert_test_primitive(&mut target, test_underline(5.0));
        target.pop_layer();
        assert_eq!(batch_labels(&mut target), ["underlines (2)"]);
        assert!(target.quads.is_empty());
    }

    #[test]
    fn path_gpu_layouts_preserve_official_curves_tiles_and_fork_paint_contract() {
        assert_eq!(size_of::<PathCurve>(), 40);
        assert_eq!(align_of::<PathCurve>(), 4);
        assert_eq!(offset_of!(PathCurve, sign), 32);
        assert_eq!(offset_of!(PathCurve, pad), 36);
        assert_eq!(size_of::<PathTile>(), 56);
        assert_eq!(offset_of!(PathTile, top_left), 16);
        assert_eq!(offset_of!(PathTile, curves_bottom_right), 48);
        assert_eq!(size_of::<PathPaint>(), 136);
        assert_eq!(align_of::<PathPaint>(), 4);
        assert_eq!(offset_of!(PathPaint, bounds), 0);
        assert_eq!(offset_of!(PathPaint, clip), 16);
        assert_eq!(offset_of!(PathPaint, color), 40);
        assert_eq!(offset_of!(PathPaint, even_odd), 112);
        assert_eq!(offset_of!(PathPaint, pad), 116);
        assert_eq!(offset_of!(PathPaint, transform), 120);
    }

    fn rectangular_path(bounds: Bounds<Pixels>) -> Path {
        let mut builder = crate::PathBuilder::fill();
        builder.add_polygon(
            &[
                bounds.origin,
                bounds.top_right(),
                bounds.bottom_right(),
                bounds.bottom_left(),
            ],
            true,
        );
        builder.build().unwrap()
    }

    #[test]
    fn painted_paths_keep_local_fill_bounds_and_final_device_coverage() {
        let local = logical_bounds(2.0, 4.0, 8.0, 10.0);
        let transform = PrimitiveTransform::try_new(
            size(2.0, 0.5),
            point(ScaledPixels(100.0), ScaledPixels(50.0)),
        )
        .unwrap();
        let viewport = logical_bounds(0.0, 0.0, 200.0, 100.0);
        let path = PaintedPath::new_transformed(
            rectangular_path(local),
            1.5,
            transform,
            viewport,
            crate::red().into(),
        )
        .unwrap();
        let projected = transform.try_project_bounds(local.scale(1.5)).unwrap();
        assert_eq!(path.paint.bounds, local.scale(1.5));
        assert_eq!(path.paint.transform, transform);
        assert_eq!(path.renderer_transform(), PrimitiveTransform::IDENTITY);
        assert_eq!(
            path.bounds,
            projected.dilate(ScaledPixels(0.5 + 1.0 / 256.0))
        );
        assert_eq!(
            Primitive::from(path.clone()).try_visual_bounds().unwrap(),
            path.bounds
        );
        assert!(path.tile_count() > 0);

        let mut paints = Vec::new();
        let mut curves = Vec::new();
        let mut tiles = Vec::new();
        path.append_to(&mut paints, &mut curves, &mut tiles);
        let curve_count = curves.len();
        let tile_count = tiles.len();
        path.append_to(&mut paints, &mut curves, &mut tiles);
        assert_eq!(paints.len(), 2);
        assert_eq!(curves[..curve_count], curves[curve_count..]);
        for (first, second) in tiles[..tile_count].iter().zip(&tiles[tile_count..]) {
            assert_eq!(first.paint, 0);
            assert_eq!(second.paint, 1);
            assert_eq!(second.curve_start, first.curve_start + curve_count as u32);
            assert_eq!(first.top_left, second.top_left);
            assert_eq!(first.bottom_right, second.bottom_right);
        }
        assert_eq!(paints[1].bounds, local.scale(1.5));
        assert_eq!(paints[1].transform, transform);
    }

    #[test]
    fn painted_path_retention_replays_exact_clip_and_rebased_validity() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let clip = rounded_stack(viewport, logical_bounds(20.0, 20.0, 80.0, 80.0));
        let validity = SubtreeGeometryValidity::new(None);
        let path = PaintedPath::new(
            rectangular_path(logical_bounds(30.0, 30.0, 20.0, 20.0)),
            1.0,
            viewport,
            crate::red().into(),
        )
        .unwrap();
        let mut source = Scene::default();
        source.push_layer(Bounds::default());
        source
            .insert_primitive_scoped(path, &clip, 1.0, Some(validity.clone()))
            .unwrap();
        source.pop_layer();
        let (mut fragment, validities) = source
            .retained_visual_fragment(0..source.journal_len())
            .unwrap();
        assert_eq!(validities.len(), 3);
        let fragment_validity = fragment.paint_operations[1]
            .validity
            .as_ref()
            .expect("the retained path must have its own replay validity")
            .clone();
        let mut recorded = source.clone();
        source.clear();
        let replay_validity = SubtreeGeometryValidity::new(None);
        let mut replayed = Scene::default();
        insert_test_primitive(&mut replayed, test_quad(0.0));
        replayed
            .replay(
                0..fragment.journal_len(),
                &fragment,
                Some(replay_validity.clone()),
            )
            .unwrap();
        replayed.finish();
        let source_clip = fragment.paths[0].paint.clip;
        let replayed_clip = replayed.paths[0].paint.clip;
        assert_ne!(source_clip.first_clip, replayed_clip.first_clip);
        assert_eq!(
            fragment.clip_shapes_for(source_clip).unwrap(),
            replayed.clip_shapes_for(replayed_clip).unwrap()
        );
        assert_eq!(
            fragment.paths[0].tiles.curves(),
            replayed.paths[0].tiles.curves()
        );
        assert_eq!(
            fragment.paths[0].tiles.tiles(),
            replayed.paths[0].tiles.tiles()
        );
        validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        recorded.finish();
        assert!(recorded.paths.is_empty());
        assert!(!validities[1].as_ref().unwrap().is_valid());
        assert!(fragment_validity.is_valid());
        assert!(replay_validity.is_valid());
        fragment.finish();
        replayed.finish();
        assert_eq!(fragment.paths.len(), 1);
        assert_eq!(replayed.paths.len(), 1);

        fragment_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        fragment.finish();
        assert!(fragment.paths.is_empty());
        replayed.finish();
        assert_eq!(replayed.paths.len(), 1);

        replay_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        replayed.finish();
        assert!(replayed.paths.is_empty());
        assert_eq!(replayed.quads.len(), 1);
        assert!(matches!(
            fragment.retained_visual_fragment(0..3),
            Err(RetainedVisualSceneError::InvalidGeometry)
        ));
    }

    #[test]
    fn path_visual_bounds_reject_invalid_fill_projection_before_empty_tile_culling() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let mut path = PaintedPath::new(
            rectangular_path(logical_bounds(1.0, 1.0, 10.0, 10.0)),
            1.0,
            viewport,
            crate::red().into(),
        )
        .unwrap();
        path.paint.bounds.origin.x = ScaledPixels(f32::MAX);
        path.paint.transform =
            PrimitiveTransform::try_new(size(2.0, 1.0), Point::default()).unwrap();
        path.bounds = Bounds::default();
        let mut scene = Scene::default();
        assert_eq!(
            scene.insert_primitive_scoped(path, &ClipStackSnapshot::root(viewport), 1.0, None),
            Err(SubtreeGeometryError::Transform(
                SubtreeTransformError::UnrepresentableResult
            ))
        );
        assert_eq!(scene.journal_len(), 0);
        assert!(scene.paths.is_empty());
    }

    #[test]
    fn path_mask_restriction_survives_exact_clip_interning_and_replay() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let mask = logical_bounds(35.0, 35.0, 5.0, 5.0);
        let path = PaintedPath::new(
            rectangular_path(logical_bounds(30.0, 30.0, 20.0, 20.0)),
            1.0,
            mask,
            crate::red().into(),
        )
        .unwrap();
        let clip = rounded_stack(viewport, logical_bounds(20.0, 20.0, 80.0, 80.0));
        let mut source = Scene::default();
        source
            .insert_primitive_scoped(path, &clip, 1.0, None)
            .unwrap();
        assert_eq!(
            source.paths[0].paint.clip.conservative_bounds,
            mask.scale(1.0)
        );
        assert_eq!(source.paths[0].paint.clip.clip_count, 2);
        let mut replayed = Scene::default();
        replayed
            .replay(0..source.journal_len(), &source, None)
            .unwrap();
        assert_eq!(
            replayed.paths[0].paint.clip.conservative_bounds,
            mask.scale(1.0)
        );
        assert_eq!(source.clip_shapes(), replayed.clip_shapes());
    }

    #[test]
    fn quad_depth_uses_representable_steps_and_clamps_extreme_indices() {
        for count in [0, 1, 2, 3, 1000, (1 << 24) - 1, usize::MAX] {
            let scale = quad_depth_scale(count);
            assert!(scale.is_normal() && scale > 0.0 && scale <= 1.0);
            assert!(scale.recip().log2().fract() == 0.0);
        }
        let scale = quad_depth_scale(1000);
        let mut previous = 0.0;
        for index in 0..1000 {
            let depth = quad_depth(index, scale);
            assert!(depth > previous && depth < 1.0);
            previous = depth;
        }
        assert_eq!(quad_depth(u32::MAX, scale), 1.0);
    }

    fn monochrome_sprite(tile_id: u32, index: u32, generation: u32, x: f32) -> MonochromeSprite {
        MonochromeSprite {
            order: 0,
            animation: SpriteAnimationId::NONE,
            tile: AtlasTile {
                texture_id: AtlasTextureId {
                    index,
                    kind: AtlasTextureKind::Monochrome,
                },
                tile_id: TileId(tile_id),
                padding: 0,
                bounds: Bounds::default(),
                texture_generation: generation,
                texture_generation_padding: 0,
            },
            bounds: scaled_bounds(x, 0.0, 4.0, 4.0),
            clip: ClipEnvelope::default(),
            color: Hsla::default(),
            transform: PrimitiveTransform::IDENTITY,
            transformation: TransformationMatrix::unit(),
        }
    }

    #[test]
    fn sprite_sort_preserves_full_texture_keys_and_legacy_ties() {
        use AtlasTextureKind::{Monochrome, Polychrome, Subpixel};

        let mut sprites = [
            (u32::MAX, 0, u32::MAX, Monochrome, u32::MAX),
            (u32::MAX, 1, 1, Subpixel, 1),
            (u32::MAX, 2, 1, Monochrome, u32::MAX),
            (u32::MAX, 3, 1, Monochrome, 1),
            (u32::MAX, 4, 1, Polychrome, 1),
            (u32::MAX, 5, 1, Monochrome, 1),
            (0, u32::MAX, 0, Monochrome, 1),
            (0, 2, u32::MAX, Subpixel, u32::MAX),
            (0, 2, 0, Polychrome, 1),
        ]
        .into_iter()
        .enumerate()
        .map(
            |(index, (order, tile_id, texture_index, kind, generation))| {
                let mut sprite =
                    monochrome_sprite(tile_id, texture_index, generation, index as f32);
                sprite.order = order;
                sprite.tile.texture_id.kind = kind;
                sprite
            },
        )
        .collect::<Vec<_>>();

        sort_sprites(
            &mut sprites,
            &mut Vec::new(),
            &[LegacySpriteOrder {
                monochrome: true,
                ..Default::default()
            }],
            |kinds| kinds.monochrome,
            |sprite| (sprite.order, &sprite.tile),
        );

        assert!(
            sprites
                .iter()
                .map(|sprite| sprite.bounds.origin.x.0)
                .eq([7.0, 8.0, 6.0, 3.0, 5.0, 2.0, 4.0, 1.0, 0.0])
        );
    }

    #[test]
    fn parked_scene_retains_original_atlas_textures_after_source_key_removal() -> anyhow::Result<()>
    {
        use crate::{AtlasKey, HeadlessAtlas, ImageId, RenderImageParams};
        use std::borrow::Cow;

        let atlas = Arc::new(HeadlessAtlas::default());
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(91),
            frame_index: 0,
        });
        let tile = atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((
                    Size::new(crate::DevicePixels(1), crate::DevicePixels(1)),
                    Cow::Borrowed(&[255_u8; 4][..]),
                )))
            })?
            .unwrap();
        let mut scene = Scene::default();
        scene
            .insert_primitive_scoped(
                PolychromeSprite {
                    order: 0,
                    pad: 0,
                    tile,
                    grayscale: false.into(),
                    opacity: 1.0,
                    bounds: scaled_bounds(0.0, 0.0, 4.0, 4.0),
                    clip: ClipEnvelope::default(),
                    corner_radii: Corners::default(),
                    transform: PrimitiveTransform::IDENTITY,
                },
                &ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 8.0, 8.0)),
                1.0,
                None,
            )
            .unwrap();
        scene.finish();
        let lease = scene.retain_atlas_textures(atlas.clone())?;
        atlas.remove(&key);
        let parked = scene.clone();
        drop(scene);
        assert!(lease.validate().is_ok());
        assert!(parked.retain_atlas_textures(atlas.clone()).is_ok());
        drop(lease);
        assert!(parked.retain_atlas_textures(atlas).is_err());
        Ok(())
    }

    #[test]
    fn parked_scene_lease_uses_the_actual_renderer_sprite_arrays() -> anyhow::Result<()> {
        use crate::{AtlasKey, HeadlessAtlas, ImageId, RenderImageParams};
        use std::borrow::Cow;

        let atlas = Arc::new(HeadlessAtlas::default());
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(92),
            frame_index: 0,
        });
        let tile = atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((
                    Size::new(crate::DevicePixels(1), crate::DevicePixels(1)),
                    Cow::Borrowed(&[255_u8; 4][..]),
                )))
            })?
            .unwrap();
        let mut scene = Scene::default();
        scene.polychrome_sprites.push(PolychromeSprite {
            order: 0,
            pad: 0,
            tile,
            grayscale: false.into(),
            opacity: 1.0,
            bounds: scaled_bounds(0.0, 0.0, 4.0, 4.0),
            clip: ClipEnvelope::default(),
            corner_radii: Corners::default(),
            transform: PrimitiveTransform::IDENTITY,
        });
        scene.finish();
        let lease = scene.retain_atlas_textures(atlas.clone())?;
        assert_eq!(lease.texture_instances(), &[tile.texture_instance()]);
        atlas.remove(&key);
        assert!(scene.retain_atlas_textures(atlas.clone()).is_ok());
        drop(lease);
        assert!(scene.retain_atlas_textures(atlas).is_err());
        Ok(())
    }

    #[test]
    fn disjoint_sprites_group_by_texture_slot_and_generation() {
        let clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 64.0, 8.0));
        let mut scene = Scene::default();
        for (tile_id, index, generation, x) in [
            (5, 2, 7, 0.0),
            (4, 1, 8, 8.0),
            (3, 1, 7, 16.0),
            (2, 2, 7, 24.0),
            (1, 1, 8, 32.0),
            (0, 1, 7, 40.0),
        ] {
            scene
                .insert_primitive_scoped(
                    monochrome_sprite(tile_id, index, generation, x),
                    &clip,
                    1.0,
                    None,
                )
                .unwrap();
        }
        scene.finish();

        let batches = scene
            .batches()
            .map(|batch| match batch {
                PrimitiveBatch::MonochromeSprites { texture, range } => {
                    (texture.texture_id.index, texture.generation, range)
                }
                _ => panic!("unexpected batch: {batch:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(batches, [(1, 7, 0..2), (1, 8, 2..4), (2, 7, 4..6)]);
        assert!(
            scene
                .monochrome_sprites
                .iter()
                .all(|sprite| sprite.order == 1)
        );
        assert!(
            scene
                .monochrome_sprites
                .iter()
                .map(|sprite| sprite.tile.tile_id.0)
                .eq([3, 0, 4, 1, 5, 2])
        );
    }

    #[test]
    fn overlapping_sprites_keep_draw_order_across_textures() {
        let clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 64.0, 8.0));
        let mut scene = Scene::default();
        for (tile_id, index, x) in [(2, 2, 0.0), (0, 1, 2.0), (1, 2, 1.0)] {
            scene
                .insert_primitive_scoped(monochrome_sprite(tile_id, index, 7, x), &clip, 1.0, None)
                .unwrap();
        }
        scene.finish();

        assert_eq!(scene.batches().count(), 3);
        assert!(
            scene
                .monochrome_sprites
                .iter()
                .map(|sprite| (sprite.order, sprite.tile.tile_id.0))
                .eq([(1, 2), (2, 0), (3, 1)])
        );
    }

    #[test]
    fn same_order_sprites_keep_legacy_order_when_raster_edges_can_share_a_pixel() {
        let viewport = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 64.0, 8.0));
        let mut large_origin = monochrome_sprite(2, 1, 7, 16_777_216.0);
        large_origin.bounds.size.width = ScaledPixels(7.0);
        large_origin.transform = PrimitiveTransform::try_new(
            Size::new(1.0, 1.0),
            point(ScaledPixels(-16_777_206.0), ScaledPixels(0.0)),
        )
        .unwrap();
        assert_eq!(
            Primitive::from(large_origin)
                .try_visual_bounds()
                .unwrap()
                .right(),
            ScaledPixels(17.0)
        );
        assert_eq!(
            large_origin.bounds.right().0 + large_origin.transform.components()[2],
            18.0
        );

        let left_clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 4.5, 8.0));
        let right_clip = ClipStackSnapshot::root(logical_bounds(4.5, 0.0, 59.5, 8.0));
        for ((first, first_clip), (second, second_clip)) in [
            (
                (large_origin, &viewport),
                (monochrome_sprite(1, 2, 7, 17.0), &viewport),
            ),
            (
                (monochrome_sprite(2, 1, 7, 2.0), &left_clip),
                (monochrome_sprite(1, 2, 7, 4.0), &right_clip),
            ),
        ] {
            let mut scene = Scene::default();
            scene
                .insert_primitive_scoped(first, first_clip, 1.0, None)
                .unwrap();
            scene
                .insert_primitive_scoped(second, second_clip, 1.0, None)
                .unwrap();
            scene.finish();
            assert!(
                scene
                    .monochrome_sprites
                    .iter()
                    .map(|sprite| (sprite.order, sprite.tile.tile_id.0))
                    .eq([(1, 1), (1, 2)])
            );
        }
    }

    #[test]
    fn fixed_layer_order_keeps_legacy_sprite_order_without_disabling_other_groups() {
        let clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 64.0, 8.0));
        let mut scene = Scene::default();
        scene.push_layer(scaled_bounds(0.0, 0.0, 64.0, 8.0));
        for (tile_id, index) in [(2, 2), (0, 2), (1, 1)] {
            let mut sprite = monochrome_sprite(tile_id, index, 7, 0.0);
            sprite.bounds.size.width = ScaledPixels(64.0);
            scene
                .insert_primitive_scoped(sprite, &clip, 1.0, None)
                .unwrap();
        }
        scene.pop_layer();
        for (tile_id, index, x) in [(5, 2, 0.0), (4, 1, 8.0), (3, 2, 16.0), (6, 1, 24.0)] {
            scene
                .insert_primitive_scoped(monochrome_sprite(tile_id, index, 7, x), &clip, 1.0, None)
                .unwrap();
        }
        scene.finish();

        assert_eq!(scene.batches().count(), 5);
        assert!(
            scene
                .monochrome_sprites
                .iter()
                .map(|sprite| (sprite.order, sprite.tile.tile_id.0))
                .eq([(1, 0), (1, 1), (1, 2), (2, 4), (2, 6), (2, 5), (2, 3)])
        );
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
    fn svg_raster_matrix_precedes_the_checked_subtree_transform() {
        let mut sprite = monochrome_sprite(0, 0, 1, 2.0);
        sprite.bounds = scaled_bounds(2.0, 3.0, 4.0, 6.0);
        sprite.transformation = TransformationMatrix {
            rotation_scale: [[0.0, -2.0], [3.0, 0.0]],
            translation: [20.0, 5.0],
        };
        sprite.transform = PrimitiveTransform::try_new(
            size(2.0, 3.0),
            point(ScaledPixels(7.0), ScaledPixels(11.0)),
        )
        .unwrap();
        assert_eq!(
            Primitive::from(sprite).local_raster_bounds().unwrap(),
            scaled_bounds(2.0, 11.0, 12.0, 12.0)
        );
        assert_eq!(
            Primitive::from(sprite).try_visual_bounds().unwrap(),
            scaled_bounds(11.0, 44.0, 24.0, 36.0)
        );

        let clip = rounded_stack(
            logical_bounds(0.0, 0.0, 200.0, 200.0),
            logical_bounds(10.0, 40.0, 30.0, 50.0),
        );
        let mut scene = Scene::default();
        scene
            .insert_primitive_scoped(sprite, &clip, 1.0, None)
            .unwrap();
        scene.finish();
        assert_eq!(scene.monochrome_sprites.len(), 1);
        assert_eq!(scene.monochrome_sprites[0].bounds, sprite.bounds);
        assert_eq!(scene.monochrome_sprites[0].transform, sprite.transform);
        assert_eq!(
            scene.monochrome_sprites[0].transformation,
            sprite.transformation
        );
        assert_eq!(scene.monochrome_sprites[0].clip.clip_count, 2);
        assert_eq!(scene.animation_frame_interval(), None);
    }

    #[test]
    fn animated_sprite_bindings_follow_sorting_and_geometry_invalidation() {
        let clip = rounded_stack(
            logical_bounds(0.0, 0.0, 200.0, 200.0),
            logical_bounds(0.0, 0.0, 120.0, 100.0),
        );
        let invalidity = SubtreeGeometryValidity::new(None);
        let mut scene = Scene::default();
        for (index, x) in [20.0, 20.0, 60.0, 60.0].into_iter().enumerate() {
            let from =
                Transformation::alpha(0.5).with_translation(point(px(index as f32), px(0.0)));
            let timed = TimedTransformation::new(Duration::from_secs(1))
                .from(from)
                .to(from
                    .with_translation(point(px(index as f32 + 4.0), px(0.0)))
                    .with_alpha(1.0))
                .with_max_fps([5.0, 10.0, 20.0, 40.0][index]);
            let mut sprite = monochrome_sprite(3 - index as u32, 1, 1, x);
            sprite.color = hsla(0.25, 1.0, 0.5, 0.8);
            sprite.transformation = from.into_matrix(Point::default(), 1.0);
            scene
                .insert_animated_sprite_scoped(
                    sprite,
                    SpriteAnimation::new(timed, Point::default(), 1.0),
                    &clip,
                    1.0,
                    (index == 3).then(|| invalidity.clone()),
                )
                .unwrap();
        }
        let original = scene.monochrome_sprites.clone();
        let shapes = scene.clip_shapes().to_vec();
        assert!(original.iter().map(|sprite| sprite.order).eq([1, 2, 1, 2]));
        scene.finish();
        assert!(
            scene
                .monochrome_sprites
                .iter()
                .map(|sprite| sprite.tile.tile_id.0)
                .eq([1, 3, 0, 2])
        );
        assert_eq!(
            scene.animation_frame_interval(),
            Some(Duration::from_millis(25))
        );

        for _ in 0..2 {
            scene.advance_animations(Duration::from_millis(250));
            for (index, sprite) in scene.monochrome_sprites.iter().enumerate() {
                let animation_index = sprite.animation.0 as usize;
                assert_eq!(scene.animations[animation_index].sprite_index, index);
                assert_eq!(
                    sprite.transformation.translation,
                    [animation_index as f32 + 1.0, 0.0]
                );
                assert!((sprite.color.a - 0.5).abs() < 1e-6);
            }
        }
        invalidity.invalidate(SubtreeClipError::UnrepresentableResult);
        assert_eq!(
            scene.animation_frame_interval(),
            Some(Duration::from_millis(50))
        );
        scene.advance_animations(Duration::ZERO);
        assert_eq!(scene.monochrome_sprites.len(), 3);
        assert_eq!(scene.journal_len(), 4);
        assert_eq!(scene.clip_shapes(), shapes);
        assert_eq!(scene.animations[3].sprite_index, usize::MAX);
        for (index, sprite) in scene.monochrome_sprites.iter().enumerate() {
            let animation_index = sprite.animation.0 as usize;
            assert_eq!(scene.animations[animation_index].sprite_index, index);
            assert_eq!(
                sprite.transformation,
                original[animation_index].transformation
            );
            assert!((sprite.color.a - 0.4).abs() < 1e-6);
        }
        for (operation, sprite) in scene.paint_operations.iter().zip(&original) {
            let PaintOperationKind::Primitive(Primitive::MonochromeSprite(recorded)) =
                &operation.kind
            else {
                panic!("the journal should contain its original SVG sprite");
            };
            assert_eq!(recorded.color, sprite.color);
            assert_eq!(recorded.transformation, sprite.transformation);
        }
    }

    #[test]
    fn animated_journal_replay_rebinds_ids_without_accumulating_pose_or_alpha() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let source_clip = rounded_stack(viewport, logical_bounds(20.0, 20.0, 80.0, 80.0));
        let source_validity = SubtreeGeometryValidity::new(None);
        let from = Transformation::alpha(0.4).with_translation(point(px(2.0), px(0.0)));
        let timed = TimedTransformation::new(Duration::from_secs(1))
            .from(from)
            .to(from
                .with_translation(point(px(6.0), px(0.0)))
                .with_alpha(0.2))
            .with_max_fps(20.0);
        let mut sprite = monochrome_sprite(0, 1, 2, 30.0);
        sprite.bounds.origin.y = ScaledPixels(30.0);
        sprite.color = hsla(0.4, 0.6, 0.5, 0.6);
        sprite.transformation = from.into_matrix(Point::default(), 1.0);
        let mut source = Scene::default();
        source
            .insert_animated_sprite_scoped(
                sprite,
                SpriteAnimation::new(timed, Point::default(), 1.0),
                &source_clip,
                1.0,
                Some(source_validity.clone()),
            )
            .unwrap();
        source.finish();
        source.advance_animations(Duration::from_millis(500));
        assert!((source.monochrome_sprites[0].color.a - 0.18).abs() < 1e-6);
        let (mut fragment, _) = source.retained_visual_fragment(0..1).unwrap();

        let replay_validity = SubtreeGeometryValidity::new(None);
        let mut target = Scene::default();
        target
            .insert_animated_sprite_scoped(
                monochrome_sprite(1, 2, 1, 120.0),
                SpriteAnimation::new(timed.with_max_fps(10.0), Point::default(), 1.0),
                &ClipStackSnapshot::root(viewport),
                1.0,
                None,
            )
            .unwrap();
        target
            .replay(0..1, &source, Some(replay_validity.clone()))
            .unwrap();
        target.finish();
        let PaintOperationKind::Primitive(Primitive::MonochromeSprite(recorded)) =
            &target.paint_operations[1].kind
        else {
            panic!("replay should record an animated SVG");
        };
        assert_eq!(recorded.animation, SpriteAnimationId(1));
        assert_eq!(recorded.color, sprite.color);
        assert_eq!(recorded.transformation, sprite.transformation);
        assert_ne!(
            recorded.clip.first_clip,
            source.monochrome_sprites[0].clip.first_clip
        );
        assert_eq!(
            target.clip_shapes_for(recorded.clip).unwrap(),
            source
                .clip_shapes_for(source.monochrome_sprites[0].clip)
                .unwrap()
        );

        source_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        source.finish();
        assert_eq!(source.animation_frame_interval(), None);
        source.clear();
        fragment.finish();
        fragment.advance_animations(Duration::ZERO);
        assert_eq!(
            fragment.monochrome_sprites[0].transformation,
            sprite.transformation
        );
        assert!((fragment.monochrome_sprites[0].color.a - 0.24).abs() < 1e-6);

        for _ in 0..2 {
            target.advance_animations(Duration::ZERO);
            let replayed = target
                .monochrome_sprites
                .iter()
                .find(|sprite| sprite.animation == SpriteAnimationId(1))
                .unwrap();
            assert_eq!(replayed.transformation, sprite.transformation);
            assert!((replayed.color.a - 0.24).abs() < 1e-6);
        }
        let mut copied = target.clone();
        target.clear();
        copied.advance_animations(Duration::from_millis(500));
        assert_eq!(
            copied.animation_frame_interval(),
            Some(Duration::from_millis(50))
        );
        replay_validity.invalidate(SubtreeClipError::UnrepresentableResult);
        copied.finish();
        assert_eq!(copied.monochrome_sprites.len(), 1);
        assert_eq!(
            copied.animation_frame_interval(),
            Some(Duration::from_millis(100))
        );
        copied.clear();
        assert!(copied.animations.is_empty());
        assert_eq!(copied.journal_len(), 0);
        assert_eq!(copied.animation_frame_interval(), None);
    }

    #[test]
    fn animated_replay_failure_restores_open_layers_and_animation_demand() {
        let clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 200.0, 200.0));
        let timed =
            TimedTransformation::rotation(Duration::from_secs(1), Transformation::default())
                .with_max_fps(20.0);
        let mut source = Scene::default();
        for index in 0..2 {
            source
                .insert_animated_sprite_scoped(
                    monochrome_sprite(index, 0, 1, index as f32 * 20.0),
                    SpriteAnimation::new(timed, Point::default(), 1.0),
                    &clip,
                    1.0,
                    None,
                )
                .unwrap();
        }
        let PaintOperationKind::Primitive(second) = &mut source.paint_operations[1].kind else {
            panic!("the source should contain two animated sprites");
        };
        let mut invalid_clip = second.clip_envelope();
        invalid_clip.first_clip = u32::MAX;
        second.set_clip_envelope(invalid_clip);

        let mut target = Scene::default();
        target.push_layer(scaled_bounds(0.0, 0.0, 200.0, 200.0));
        target
            .insert_animated_sprite_scoped(
                monochrome_sprite(2, 1, 3, 40.0),
                SpriteAnimation::new(timed.with_max_fps(10.0), Point::default(), 1.0),
                &clip,
                1.0,
                None,
            )
            .unwrap();
        let shapes = target.clip_shapes().to_vec();
        assert_eq!(target.animation_frame_interval(), None);
        assert_eq!(
            target.replay(0..2, &source, None),
            Err(SubtreeGeometryError::DeviceConversion)
        );
        assert_eq!(target.journal_len(), 2);
        assert_eq!(target.animations.len(), 1);
        assert_eq!(target.clip_shapes(), shapes);
        assert_eq!(target.layer_stack.len(), 1);
        assert!(target.monochrome_sprites.is_empty());
        target.pop_layer();
        target.finish();
        assert_eq!(target.monochrome_sprites.len(), 1);
        assert_eq!(target.monochrome_sprites[0].animation, SpriteAnimationId(0));
        assert_eq!(target.animations[0].sprite_index, 0);
        assert_eq!(
            target.animation_frame_interval(),
            Some(Duration::from_millis(100))
        );
        target.advance_animations(Duration::from_millis(250));
        assert!(target.monochrome_sprites[0].transformation.rotation_scale[0][0].abs() < 1e-5);
    }

    #[test]
    fn rejected_and_invalid_animated_sprites_leave_no_animation_demand() {
        let clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 10.0, 10.0));
        let timed =
            TimedTransformation::rotation(Duration::from_secs(1), Transformation::default());
        let animation = SpriteAnimation::new(timed, Point::default(), 1.0);
        let mut scene = Scene::default();
        scene
            .insert_animated_sprite_scoped(
                monochrome_sprite(0, 0, 1, 20.0),
                animation,
                &clip,
                1.0,
                None,
            )
            .unwrap();
        assert!(scene.animations.is_empty());
        assert_eq!(scene.journal_len(), 0);
        assert_eq!(scene.animation_frame_interval(), None);

        for coefficient in [f32::NAN, f32::INFINITY, f32::MAX] {
            let mut sprite = monochrome_sprite(0, 0, 1, 0.0);
            sprite.transformation.rotation_scale[0][0] = coefficient;
            assert_eq!(
                scene.insert_animated_sprite_scoped(sprite, animation, &clip, 1.0, None),
                Err(SubtreeGeometryError::Transform(
                    SubtreeTransformError::UnrepresentableResult
                ))
            );
            assert!(scene.animations.is_empty());
            assert_eq!(scene.journal_len(), 0);
            assert_eq!(scene.animation_frame_interval(), None);
            sprite.animation = SpriteAnimationId::NONE;
            assert!(
                scene
                    .insert_primitive_scoped(sprite, &clip, 1.0, None)
                    .is_err()
            );
        }
        assert_eq!(
            scene.insert_animated_sprite_scoped(
                monochrome_sprite(0, 0, 1, 0.0),
                animation,
                &clip,
                f32::INFINITY,
                None,
            ),
            Err(SubtreeGeometryError::DeviceConversion)
        );
        assert!(scene.animations.is_empty());

        let invalidity = SubtreeGeometryValidity::new(None);
        invalidity.invalidate(SubtreeClipError::UnrepresentableResult);
        scene
            .insert_animated_sprite_scoped(
                monochrome_sprite(0, 0, 1, 0.0),
                animation,
                &clip,
                1.0,
                Some(invalidity),
            )
            .unwrap();
        assert_eq!(scene.animation_frame_interval(), None);
        scene.finish();
        assert!(scene.monochrome_sprites.is_empty());
        assert_eq!(scene.journal_len(), 1);
        assert_eq!(scene.animation_frame_interval(), None);
    }

    #[test]
    fn a_zero_scale_start_keeps_animation_demand_without_changing_the_journal_pose() {
        let clip = ClipStackSnapshot::root(logical_bounds(10.0, 20.0, 8.0, 12.0));
        let from = Transformation::scale(size(0.0, 0.0));
        let timed = TimedTransformation::new(Duration::from_secs(1))
            .from(from)
            .to(Transformation::default());
        let center = point(px(2.0), px(2.0));
        let mut sprite = monochrome_sprite(0, 0, 1, 0.0);
        sprite.color = hsla(0.0, 1.0, 0.5, 0.8);
        sprite.transformation = from.into_matrix(center, 1.0);
        sprite.transform = PrimitiveTransform::try_new(
            size(2.0, 3.0),
            point(ScaledPixels(10.0), ScaledPixels(20.0)),
        )
        .unwrap();
        let mut scene = Scene::default();
        scene
            .insert_animated_sprite_scoped(
                sprite,
                SpriteAnimation::new(timed, center, 1.0),
                &clip,
                1.0,
                None,
            )
            .unwrap();
        scene.finish();
        assert_eq!(scene.monochrome_sprites.len(), 1);
        assert_eq!(scene.animation_frame_interval(), Some(Duration::ZERO));
        assert_eq!(
            scene.monochrome_sprites[0].transformation,
            sprite.transformation
        );
        scene.advance_animations(Duration::from_millis(500));
        assert_eq!(
            scene.monochrome_sprites[0].transformation.rotation_scale,
            [[0.5, 0.0], [0.0, 0.5]]
        );
        assert_eq!(scene.monochrome_sprites[0].transform, sprite.transform);
        assert_eq!(scene.monochrome_sprites[0].bounds, sprite.bounds);

        let mut static_scene = Scene::default();
        static_scene
            .insert_primitive_scoped(sprite, &clip, 1.0, None)
            .unwrap();
        assert!(static_scene.monochrome_sprites.is_empty());
        assert_eq!(static_scene.animation_frame_interval(), None);
    }

    #[test]
    fn animation_pose_validation_includes_the_fixed_parent_projection() {
        let clip = ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 100.0, 100.0));
        let timed = TimedTransformation::new(Duration::from_secs(1))
            .to(Transformation::translate(point(px(f32::MAX), px(0.0))).with_alpha(0.2));
        let mut sprite = monochrome_sprite(0, 0, 1, 0.0);
        sprite.color = hsla(0.0, 1.0, 0.5, 0.8);
        sprite.transform = PrimitiveTransform::try_new(size(4.0, 1.0), Point::default()).unwrap();
        let mut scene = Scene::default();
        scene
            .insert_animated_sprite_scoped(
                sprite,
                SpriteAnimation::new(timed, Point::default(), 1.0),
                &clip,
                1.0,
                None,
            )
            .unwrap();
        scene.finish();
        scene.advance_animations(Duration::from_millis(500));
        assert_eq!(
            scene.monochrome_sprites[0].transformation,
            sprite.transformation
        );
        assert_eq!(scene.monochrome_sprites[0].color, sprite.color);
        scene.advance_animations(Duration::ZERO);
        assert_eq!(
            scene.monochrome_sprites[0].transformation,
            TransformationMatrix::unit()
        );
        assert_eq!(scene.monochrome_sprites[0].color, sprite.color);
    }

    #[test]
    fn retained_animated_sprites_keep_their_original_atlas_lease() -> anyhow::Result<()> {
        use crate::{AtlasKey, HeadlessAtlas, RenderSvgParams};
        use std::borrow::Cow;

        let atlas = Arc::new(HeadlessAtlas::default());
        let key = AtlasKey::Svg(RenderSvgParams {
            path: "animated.svg".into(),
            size: size(crate::DevicePixels(1), crate::DevicePixels(1)),
        });
        let tile = atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((
                    size(crate::DevicePixels(1), crate::DevicePixels(1)),
                    Cow::Borrowed(&[255_u8][..]),
                )))
            })?
            .unwrap();
        let timed =
            TimedTransformation::rotation(Duration::from_secs(1), Transformation::default());
        let mut sprite = monochrome_sprite(0, 0, 1, 0.0);
        sprite.tile = tile;
        let mut source = Scene::default();
        source
            .insert_animated_sprite_scoped(
                sprite,
                SpriteAnimation::new(timed, point(px(2.0), px(2.0)), 1.0),
                &ClipStackSnapshot::root(logical_bounds(0.0, 0.0, 20.0, 20.0)),
                1.0,
                None,
            )
            .unwrap();
        source.finish();
        let lease = source.retain_atlas_textures(atlas.clone())?;
        let (mut retained, _) = source
            .retained_visual_fragment(0..source.journal_len())
            .unwrap();
        source.clear();
        atlas.remove(&key);
        retained.finish();
        retained.advance_animations(Duration::from_millis(250));
        assert_eq!(
            retained.atlas_texture_instances(),
            [tile.texture_instance()]
        );
        assert_eq!(lease.texture_instances(), &[tile.texture_instance()]);
        assert!(lease.validate().is_ok());
        assert!(retained.retain_atlas_textures(atlas.clone()).is_ok());
        drop(lease);
        assert!(retained.retain_atlas_textures(atlas).is_err());
        Ok(())
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
        assert_eq!(scene.quads[1].order, scene.quads[0].order);
        assert_eq!(scene.quads[0].bounds, first.bounds);
        assert_eq!(scene.quads[1].bounds, second.bounds);
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
    fn scene_clone_owns_complete_clip_and_journal_data_after_source_clear() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let clip = rounded_stack(viewport, logical_bounds(20.0, 20.0, 80.0, 80.0));
        let layer = scaled_bounds(0.0, 0.0, 200.0, 200.0);
        let mut source = Scene::default();
        source.push_layer(layer);
        source
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(30.0, 30.0, 20.0, 20.0),
                    ..Default::default()
                },
                &clip,
                1.0,
                None,
            )
            .unwrap();
        source.pop_layer();
        source.finish();
        let expected_clip = source.clip_shapes().to_vec();
        let original_order = source.quads[0].order;
        let envelope = source.quads[0].clip;
        let mut copied = source.clone();
        assert_ne!(copied.clip_shapes().as_ptr(), source.clip_shapes().as_ptr());
        source.clear();
        assert_eq!(copied.clip_shapes(), expected_clip);
        assert_eq!(copied.quads[0].clip, envelope);
        assert_eq!(copied.journal_len(), 3);
        assert_eq!(copied.len(), 3);
        assert_eq!(copied.batches().count(), 1);

        let mut replayed = Scene::default();
        replayed
            .replay(0..copied.journal_len(), &copied, None)
            .unwrap();
        replayed.finish();
        assert_eq!(replayed.clip_shapes(), copied.clip_shapes());
        assert_eq!(replayed.quads[0].clip, envelope);
        assert_eq!(replayed.quads[0].order, original_order);

        copied.push_layer(layer);
        copied
            .insert_primitive_scoped(
                Quad {
                    bounds: scaled_bounds(40.0, 40.0, 20.0, 20.0),
                    ..Default::default()
                },
                &clip,
                1.0,
                None,
            )
            .unwrap();
        copied.pop_layer();
        copied.finish();
        assert_eq!(copied.quads.len(), 2);
        assert_eq!(copied.quads[1].order, original_order);
        assert_eq!(
            copied.quads[1].bounds,
            scaled_bounds(40.0, 40.0, 20.0, 20.0)
        );
        assert_eq!(copied.journal_len(), 6);
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
    fn scene_finish_preserves_sprite_clips_and_retained_journal_indices() {
        let viewport = logical_bounds(0.0, 0.0, 200.0, 200.0);
        let invalidity = SubtreeGeometryValidity::new(None);
        let mut scene = Scene::default();
        for (index, x) in [20.0, 20.0, 60.0, 60.0].into_iter().enumerate() {
            let clip = rounded_stack(
                viewport,
                logical_bounds(0.0, 0.0, 100.0 + index as f32, 100.0),
            );
            scene
                .insert_primitive_scoped(
                    monochrome_sprite(index as u32, 1, 1, x),
                    &clip,
                    1.0,
                    (index == 3).then(|| invalidity.clone()),
                )
                .unwrap();
        }
        let original = scene.monochrome_sprites.clone();
        let clip_shapes = scene.clip_shapes().to_vec();
        let clip_stack_ranges = scene.clip_stack_ranges.clone();
        assert!(original.iter().map(|sprite| sprite.order).eq([1, 2, 1, 2]));

        for valid_count in [4, 3] {
            if valid_count == 3 {
                invalidity.invalidate(SubtreeClipError::UnrepresentableResult);
            }
            let mut expected = original[..valid_count].to_vec();
            expected.sort_by_key(|sprite| sprite.order);
            scene.finish();

            assert_eq!(scene.monochrome_sprites.len(), expected.len());
            for (actual, expected) in scene.monochrome_sprites.iter().zip(&expected) {
                assert_eq!(
                    (
                        actual.order,
                        actual.tile.tile_id,
                        actual.bounds,
                        actual.clip
                    ),
                    (
                        expected.order,
                        expected.tile.tile_id,
                        expected.bounds,
                        expected.clip
                    )
                );
            }
            assert_eq!(scene.journal_len(), original.len());
            assert_eq!(scene.clip_shapes(), clip_shapes);
            assert_eq!(scene.clip_stack_ranges, clip_stack_ranges);

            for (index, recorded) in original.iter().take(valid_count).enumerate() {
                let (mut fragment, _) = scene.retained_visual_fragment(index..index + 1).unwrap();
                fragment.finish();
                assert_eq!(fragment.monochrome_sprites.len(), 1);
                let retained = &fragment.monochrome_sprites[0];
                assert_eq!(retained.bounds, recorded.bounds);
                assert_eq!(retained.tile.tile_id, recorded.tile.tile_id);
                assert_eq!(
                    fragment.clip_shapes_for(retained.clip).unwrap(),
                    scene.clip_shapes_for(recorded.clip).unwrap()
                );
            }
        }
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
        let mut copied = scene.clone();

        invalidity.invalidate(SubtreeClipError::UnrepresentableResult);
        scene.finish();
        copied.finish();

        assert_eq!(scene.quads.len(), 1);
        assert_eq!(scene.quads[0].bounds, scaled_bounds(90.0, 90.0, 20.0, 20.0));
        assert_eq!(scene.quads[0].clip, valid_envelope);
        assert_eq!(scene.journal_len(), journal_len);
        assert_eq!(scene.clip_shapes(), clip_shapes);
        assert_eq!(copied.quads.len(), 1);
        assert_eq!(copied.quads[0].bounds, scene.quads[0].bounds);
        assert_eq!(copied.quads[0].clip, scene.quads[0].clip);
        assert_eq!(copied.journal_len(), journal_len);
        assert_eq!(copied.clip_shapes(), clip_shapes);

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
