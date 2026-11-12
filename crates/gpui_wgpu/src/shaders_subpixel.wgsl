// --- subpixel sprites --- //

struct SubpixelSprite {
    order: u32,
    pad: u32,
    tile: AtlasTile,
    bounds: Bounds,
    clip: ClipEnvelope,
    color: Hsla,
    transform: PrimitiveTransform,
}
@group(1) @binding(0) var<storage, read> b_subpixel_sprites: array<SubpixelSprite>;

struct SubpixelSpriteOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tile_position: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(4) @interpolate(flat) clip_bounds: vec4<f32>,
    @location(5) @interpolate(flat) clip_range: vec2<u32>,
}

struct SubpixelSpriteFragmentOutput {
    @location(0) @blend_src(0) foreground: vec4<f32>,
    @location(0) @blend_src(1) alpha: vec4<f32>,
}

@vertex
fn vs_subpixel_sprite(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> SubpixelSpriteOutput {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
    let sprite = b_subpixel_sprites[instance_id];
    let clipped = clip_to_mask(unit_vertex, sprite.bounds, sprite.clip, sprite.transform);

    var out = SubpixelSpriteOutput();
    out.position = to_device_position_impl(clipped.position);
    out.tile_position = to_tile_position(clipped.unit_vertex, sprite.tile);
    out.color = hsla_to_rgba(sprite.color);
    out.clip_bounds = vec4<f32>(sprite.clip.conservative_bounds.origin, sprite.clip.conservative_bounds.size);
    out.clip_range = vec2<u32>(sprite.clip.first_clip, sprite.clip.clip_count);
    return out;
}

@fragment
fn fs_subpixel_sprite(input: SubpixelSpriteOutput) -> SubpixelSpriteFragmentOutput {
    var sample = textureSample(t_sprite, s_sprite, input.tile_position).rgb;
    if (gamma_params.is_bgr != 0u) {
        sample = sample.bgr;
    }
    let alpha_corrected = apply_contrast_and_gamma_correction3(sample, input.color.rgb, gamma_params.subpixel_enhanced_contrast, gamma_params.gamma_ratios);

    let clip = ClipEnvelope(Bounds(input.clip_bounds.xy, input.clip_bounds.zw), input.clip_range.x, input.clip_range.y);
    if (!clip_envelope_contains(input.position.xy, clip)) {
        return SubpixelSpriteFragmentOutput(vec4<f32>(0.0), vec4<f32>(0.0));
    }

    var out = SubpixelSpriteFragmentOutput();
    out.foreground = vec4<f32>(srgb_to_scene(input.color).rgb, 1.0);
    out.alpha = vec4<f32>(input.color.a * alpha_corrected, 1.0);
    return out;
}
