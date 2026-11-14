//! Benchmark-only primitive construction through the production Scene journal.

use crate::{
    AtlasTile, Background, Bounds, Corners, Hsla, PaintedPath, Pixels, PrimitiveTransform,
    geometry::ClipStackSnapshot,
    scene::{MonochromeSprite, PolychromeSprite, Primitive, Quad, Scene, Shadow, Underline},
};

/// Builds benchmark scenes with the same exact clip, ordering and journal owners as a window.
pub struct SceneBuilder<'a> {
    scene: &'a mut Scene,
    clips: ClipStackSnapshot,
}

impl<'a> SceneBuilder<'a> {
    /// Uses logical pixels at scale one and one exact viewport clip.
    pub fn new(scene: &'a mut Scene, viewport: Bounds<Pixels>) -> Self {
        Self {
            scene,
            clips: ClipStackSnapshot::root(viewport),
        }
    }

    fn insert(&mut self, primitive: impl Into<Primitive>) {
        self.scene
            .insert_primitive_scoped(primitive, &self.clips, 1., None)
            .expect("benchmark primitive must have valid geometry and exact clipping");
    }

    /// Clears the Scene while retaining the fixture's exact viewport clip snapshot.
    pub fn clear(&mut self) {
        self.scene.clear();
    }

    /// Borrows the constructed Scene without acquiring rendering or submission authority.
    pub fn scene(&self) -> &Scene {
        self.scene
    }

    /// Inserts a filled rectangle without a border.
    pub fn quad(&mut self, bounds: Bounds<Pixels>, background: Background) {
        self.insert(Quad {
            bounds: bounds.scale(1.),
            background,
            transform: PrimitiveTransform::IDENTITY,
            ..Default::default()
        });
    }

    /// Inserts the ordinary drop shadow used by the primitive workload.
    pub fn shadow(
        &mut self,
        bounds: Bounds<Pixels>,
        color: Hsla,
        blur_radius: Pixels,
        corner_radii: Corners<Pixels>,
    ) {
        let bounds = bounds.scale(1.);
        let corner_radii = corner_radii.scale(1.);
        self.insert(Shadow {
            order: 0,
            blur_radius: blur_radius.scale(1.),
            bounds,
            corner_radii,
            clip: Default::default(),
            color,
            element_bounds: bounds,
            element_corner_radii: corner_radii,
            inset: 0,
            pad: 0,
            transform: PrimitiveTransform::IDENTITY,
        });
    }

    /// Inserts a straight underline of the supplied thickness.
    pub fn underline(&mut self, bounds: Bounds<Pixels>, color: Hsla, thickness: Pixels) {
        self.insert(Underline {
            order: 0,
            pad: 0,
            bounds: bounds.scale(1.),
            clip: Default::default(),
            color,
            thickness: thickness.scale(1.),
            wavy: false.into(),
            transform: PrimitiveTransform::IDENTITY,
        });
    }

    /// Inserts an already binned path, preserving its local paint bounds and raster restriction.
    pub fn path(&mut self, path: PaintedPath) {
        self.insert(path);
    }

    /// Inserts one monochrome sprite with its renderer-local texture generation intact.
    pub fn monochrome_sprite(&mut self, bounds: Bounds<Pixels>, tile: AtlasTile, color: Hsla) {
        self.insert(MonochromeSprite {
            order: 0,
            animation: Default::default(),
            transformation: Default::default(),
            tile,
            bounds: bounds.scale(1.),
            clip: Default::default(),
            color,
            transform: PrimitiveTransform::IDENTITY,
        });
    }

    /// Inserts an ordinary opaque-color image without rounded corners or imported-image flags.
    pub fn polychrome_sprite(&mut self, bounds: Bounds<Pixels>, tile: AtlasTile) {
        self.insert(PolychromeSprite {
            order: 0,
            pad: 0,
            tile,
            grayscale: false.into(),
            opacity: 1.,
            bounds: bounds.scale(1.),
            clip: Default::default(),
            corner_radii: Default::default(),
            transform: PrimitiveTransform::IDENTITY,
        });
    }

    /// Sorts and batches the completed production Scene.
    pub fn finish(&mut self) {
        self.scene.finish();
    }
}
