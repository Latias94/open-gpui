@group(0) @binding(2) var<storage, read> b_clip_shapes: array<GpuClipShape>;
@group(1) @binding(0) var<storage, read> b_quads: array<Quad>;
@group(1) @binding(0) var<storage, read> b_shadows: array<Shadow>;
@group(1) @binding(0) var<storage, read> b_path_vertices: array<PathRasterizationVertex>;
@group(1) @binding(0) var<storage, read> b_path_sprites: array<PathSprite>;
@group(1) @binding(0) var<storage, read> b_underlines: array<Underline>;
@group(1) @binding(0) var<storage, read> b_mono_sprites: array<MonochromeSprite>;
@group(1) @binding(0) var<storage, read> b_poly_sprites: array<PolychromeSprite>;

fn clip_arena_length() -> u32 { return arrayLength(&b_clip_shapes); }
fn load_clip_shape(index: u32) -> GpuClipShape { return b_clip_shapes[index]; }
fn load_quad(index: u32) -> Quad { return b_quads[index]; }
fn load_shadow(index: u32) -> Shadow { return b_shadows[index]; }
fn load_path_vertex(index: u32) -> PathRasterizationVertex { return b_path_vertices[index]; }
fn load_path_sprite(index: u32) -> PathSprite { return b_path_sprites[index]; }
fn load_underline(index: u32) -> Underline { return b_underlines[index]; }
fn load_mono_sprite(index: u32) -> MonochromeSprite { return b_mono_sprites[index]; }
fn load_poly_sprite(index: u32) -> PolychromeSprite { return b_poly_sprites[index]; }
