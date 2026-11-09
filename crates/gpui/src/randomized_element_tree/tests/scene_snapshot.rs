use super::*;
use crate::{
    AtlasTile, Background, BorderStyle, Bounds, ClipEnvelope, Corners, Edges, GpuClipShape,
    PrimitiveTransform, ScaledPixels, Scene,
};

#[derive(Debug, PartialEq)]
pub(super) struct SceneSnapshot {
    pub(super) quads: Vec<QuadSnapshot>,
    /// Every other primitive kind, formatted without the per-window atlas tile and
    /// path IDs, which differ between two windows drawing the same scene.
    pub(super) other_primitives: Vec<String>,
    pub(super) element_ids: BTreeSet<u64>,
}

impl SceneSnapshot {
    pub(super) fn from_window(window: &Window) -> Self {
        let scene = &window.rendered_frame.scene;
        assert!(
            scene.surfaces.is_empty(),
            "randomized tree fixtures must not introduce unobserved native surface pixels"
        );
        let mut texture_ids = std::collections::HashMap::new();
        let mut tile_ids = std::collections::HashMap::new();
        for tile in scene
            .monochrome_sprites
            .iter()
            .map(|sprite| sprite.tile)
            .chain(scene.subpixel_sprites.iter().map(|sprite| sprite.tile))
            .chain(scene.polychrome_sprites.iter().map(|sprite| sprite.tile))
        {
            let texture_id = u32::try_from(texture_ids.len()).expect("texture count fits u32");
            texture_ids
                .entry(tile.texture_instance())
                .or_insert(texture_id);
            let tile_id = u32::try_from(tile_ids.len()).expect("tile count fits u32");
            tile_ids
                .entry((tile.texture_instance(), tile.tile_id.0))
                .or_insert(tile_id);
        }
        let normalize_tile = |tile: &mut AtlasTile| {
            let texture = tile.texture_instance();
            let tile_id = tile_ids[&(texture, tile.tile_id.0)];
            tile.texture_id.index = texture_ids[&texture];
            tile.texture_generation = 0;
            tile.tile_id = crate::TileId(tile_id);
        };
        let other_primitives = scene
            .shadows
            .iter()
            .map(|shadow| {
                let mut shadow = *shadow;
                let clip = ClipSnapshot::new(scene, shadow.clip);
                shadow.clip.first_clip = 0;
                format!("shadow {shadow:?} {clip:?}")
            })
            .chain(scene.underlines.iter().map(|underline| {
                let mut underline = *underline;
                let clip = ClipSnapshot::new(scene, underline.clip);
                underline.clip.first_clip = 0;
                format!("underline {underline:?} {clip:?}")
            }))
            .chain(scene.paths.iter().map(|path| {
                format!(
                    "path {} {:?} {:?} {:?} {:?} {:?}",
                    path.order,
                    path.bounds,
                    ClipSnapshot::new(scene, path.clip),
                    path.color,
                    path.vertices,
                    path.transform,
                )
            }))
            .chain(scene.monochrome_sprites.iter().map(|sprite| {
                let mut sprite = *sprite;
                let clip = ClipSnapshot::new(scene, sprite.clip);
                sprite.clip.first_clip = 0;
                normalize_tile(&mut sprite.tile);
                format!("monochrome {sprite:?} {clip:?}")
            }))
            .chain(scene.subpixel_sprites.iter().map(|sprite| {
                let mut sprite = *sprite;
                let clip = ClipSnapshot::new(scene, sprite.clip);
                sprite.clip.first_clip = 0;
                normalize_tile(&mut sprite.tile);
                format!("subpixel {sprite:?} {clip:?}")
            }))
            .chain(scene.polychrome_sprites.iter().map(|sprite| {
                let mut sprite = *sprite;
                let clip = ClipSnapshot::new(scene, sprite.clip);
                sprite.clip.first_clip = 0;
                normalize_tile(&mut sprite.tile);
                format!("polychrome {sprite:?} {clip:?}")
            }))
            .collect();
        Self {
            quads: scene
                .quads
                .iter()
                .map(|quad| QuadSnapshot::new(scene, quad))
                .collect(),
            other_primitives,
            element_ids: window
                .rendered_frame
                .element_states
                .keys()
                .filter_map(|(global_id, _)| {
                    let ElementId::NamedInteger(name, element_id) = global_id.last()? else {
                        return None;
                    };
                    (name.as_ref() == "randomized-element").then_some(*element_id)
                })
                .collect(),
        }
    }
}

#[derive(Debug, PartialEq)]
struct ClipSnapshot {
    conservative_bounds: Bounds<ScaledPixels>,
    ancestry: Vec<GpuClipShape>,
}

impl ClipSnapshot {
    fn new(scene: &Scene, clip: ClipEnvelope) -> Self {
        let first = usize::try_from(clip.first_clip).expect("clip index fits usize");
        let count = usize::try_from(clip.clip_count).expect("clip count fits usize");
        let end = first
            .checked_add(count)
            .expect("clip range must not overflow");
        Self {
            conservative_bounds: clip.conservative_bounds,
            ancestry: scene
                .clip_shapes()
                .get(first..end)
                .expect("accepted primitive must reference its complete clip ancestry")
                .to_vec(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct QuadSnapshot {
    order: u32,
    border_style: BorderStyle,
    bounds: Bounds<ScaledPixels>,
    clip: ClipSnapshot,
    background: Background,
    border_color: Hsla,
    corner_radii: Corners<ScaledPixels>,
    border_widths: Edges<ScaledPixels>,
    transform: PrimitiveTransform,
}

impl QuadSnapshot {
    fn new(scene: &Scene, quad: &crate::Quad) -> Self {
        Self {
            order: quad.order,
            border_style: quad.border_style,
            bounds: quad.bounds,
            clip: ClipSnapshot::new(scene, quad.clip),
            background: quad.background,
            border_color: quad.border_color,
            corner_radii: quad.corner_radii,
            border_widths: quad.border_widths,
            transform: quad.transform,
        }
    }
}
