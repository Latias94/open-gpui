@group(1) @binding(0) var t_instances: texture_2d<u32>;
@group(0) @binding(2) var t_clip_shapes: texture_2d<u32>;

struct InstanceCursor {
    word_index: u32,
    texel_index: u32,
    texel: vec4<u32>,
    width: u32,
}

fn fetch_instance_texel(index: u32, width: u32) -> vec4<u32> {
    return textureLoad(t_instances, vec2<i32>(i32(index % width), i32(index / width)), 0);
}

fn instance_cursor(word_index: u32) -> InstanceCursor {
    let width = textureDimensions(t_instances).x;
    let texel_index = word_index / 4u;
    return InstanceCursor(word_index, texel_index, fetch_instance_texel(texel_index, width), width);
}

fn read_word(cursor: ptr<function, InstanceCursor>) -> u32 {
    let word_index = (*cursor).word_index;
    let texel_index = word_index / 4u;
    if texel_index != (*cursor).texel_index {
        (*cursor).texel = fetch_instance_texel(texel_index, (*cursor).width);
        (*cursor).texel_index = texel_index;
    }
    (*cursor).word_index = word_index + 1u;
    return (*cursor).texel[word_index % 4u];
}

fn read_f32(cursor: ptr<function, InstanceCursor>) -> f32 {
    return bitcast<f32>(read_word(cursor));
}

fn read_i32(cursor: ptr<function, InstanceCursor>) -> i32 {
    return bitcast<i32>(read_word(cursor));
}

fn read_vec2_f32(cursor: ptr<function, InstanceCursor>) -> vec2<f32> {
    return vec2<f32>(read_f32(cursor), read_f32(cursor));
}

fn read_vec2_i32(cursor: ptr<function, InstanceCursor>) -> vec2<i32> {
    return vec2<i32>(read_i32(cursor), read_i32(cursor));
}

fn read_hsla(cursor: ptr<function, InstanceCursor>) -> Hsla {
    return Hsla(read_f32(cursor), read_f32(cursor), read_f32(cursor), read_f32(cursor));
}

fn read_bounds(cursor: ptr<function, InstanceCursor>) -> Bounds {
    return Bounds(read_vec2_f32(cursor), read_vec2_f32(cursor));
}

fn read_corners(cursor: ptr<function, InstanceCursor>) -> Corners {
    return Corners(read_f32(cursor), read_f32(cursor), read_f32(cursor), read_f32(cursor));
}

fn read_color_stop(cursor: ptr<function, InstanceCursor>) -> LinearColorStop {
    return LinearColorStop(read_hsla(cursor), read_f32(cursor));
}

fn read_background(cursor: ptr<function, InstanceCursor>) -> Background {
    return Background(
        read_word(cursor), read_word(cursor), read_hsla(cursor), read_f32(cursor),
        array<LinearColorStop, 2>(read_color_stop(cursor), read_color_stop(cursor)),
        read_word(cursor),
    );
}

fn read_clip(cursor: ptr<function, InstanceCursor>) -> ClipEnvelope {
    return ClipEnvelope(read_bounds(cursor), read_word(cursor), read_word(cursor));
}

fn read_transform(cursor: ptr<function, InstanceCursor>) -> PrimitiveTransform {
    return PrimitiveTransform(read_f32(cursor), read_f32(cursor), read_f32(cursor), read_f32(cursor));
}

fn read_transformation(cursor: ptr<function, InstanceCursor>) -> TransformationMatrix {
    return TransformationMatrix(
        mat2x2<f32>(read_vec2_f32(cursor), read_vec2_f32(cursor)),
        read_vec2_f32(cursor),
    );
}

fn read_atlas_tile(cursor: ptr<function, InstanceCursor>) -> AtlasTile {
    return AtlasTile(
        AtlasTextureId(read_word(cursor), read_word(cursor)),
        read_word(cursor), read_word(cursor),
        AtlasBounds(read_vec2_i32(cursor), read_vec2_i32(cursor)),
        read_word(cursor), read_word(cursor),
    );
}

fn clip_arena_length() -> u32 {
    let dimensions = textureDimensions(t_clip_shapes);
    return dimensions.x * dimensions.y / 3u;
}

fn load_clip_shape(index: u32) -> GpuClipShape {
    let width = textureDimensions(t_clip_shapes).x;
    let first = index * 3u;
    let bounds = bitcast<vec4<f32>>(textureLoad(
        t_clip_shapes, vec2<i32>(i32(first % width), i32(first / width)), 0));
    let radii_x = bitcast<vec4<f32>>(textureLoad(
        t_clip_shapes, vec2<i32>(i32((first + 1u) % width), i32((first + 1u) / width)), 0));
    let radii_y = bitcast<vec4<f32>>(textureLoad(
        t_clip_shapes, vec2<i32>(i32((first + 2u) % width), i32((first + 2u) / width)), 0));
    return GpuClipShape(
        Bounds(bounds.xy, bounds.zw),
        Corners(radii_x.x, radii_x.y, radii_x.z, radii_x.w),
        Corners(radii_y.x, radii_y.y, radii_y.z, radii_y.w),
    );
}

fn load_quad(instance_id: u32) -> Quad {
    let first = instance_id * 46u / 4u;
    let width = textureDimensions(t_instances).x;
    var texel0 = fetch_instance_texel(first, width);
    var texel1 = fetch_instance_texel(first + 1u, width);
    var texel2 = fetch_instance_texel(first + 2u, width);
    var texel3 = fetch_instance_texel(first + 3u, width);
    var texel4 = fetch_instance_texel(first + 4u, width);
    var texel5 = fetch_instance_texel(first + 5u, width);
    var texel6 = fetch_instance_texel(first + 6u, width);
    var texel7 = fetch_instance_texel(first + 7u, width);
    var texel8 = fetch_instance_texel(first + 8u, width);
    var texel9 = fetch_instance_texel(first + 9u, width);
    var texel10 = fetch_instance_texel(first + 10u, width);
    var texel11 = fetch_instance_texel(first + 11u, width);
    if (instance_id & 1u) != 0u {
        texel0 = vec4<u32>(texel0.zw, texel1.xy);
        texel1 = vec4<u32>(texel1.zw, texel2.xy);
        texel2 = vec4<u32>(texel2.zw, texel3.xy);
        texel3 = vec4<u32>(texel3.zw, texel4.xy);
        texel4 = vec4<u32>(texel4.zw, texel5.xy);
        texel5 = vec4<u32>(texel5.zw, texel6.xy);
        texel6 = vec4<u32>(texel6.zw, texel7.xy);
        texel7 = vec4<u32>(texel7.zw, texel8.xy);
        texel8 = vec4<u32>(texel8.zw, texel9.xy);
        texel9 = vec4<u32>(texel9.zw, texel10.xy);
        texel10 = vec4<u32>(texel10.zw, texel11.xy);
        texel11 = vec4<u32>(texel11.zw, 0u, 0u);
    }
    let values0 = bitcast<vec4<f32>>(texel0);
    let values1 = bitcast<vec4<f32>>(texel1);
    let values2 = bitcast<vec4<f32>>(texel2);
    let values3 = bitcast<vec4<f32>>(texel3);
    let values4 = bitcast<vec4<f32>>(texel4);
    let values5 = bitcast<vec4<f32>>(texel5);
    let values6 = bitcast<vec4<f32>>(texel6);
    let values7 = bitcast<vec4<f32>>(texel7);
    let values8 = bitcast<vec4<f32>>(texel8);
    let values9 = bitcast<vec4<f32>>(texel9);
    let values10 = bitcast<vec4<f32>>(texel10);
    let values11 = bitcast<vec4<f32>>(texel11);
    return Quad(
        texel0.x, texel0.y, Bounds(values0.zw, values1.xy),
        ClipEnvelope(Bounds(values1.zw, values2.xy), texel2.z, texel2.w),
        Background(
            texel3.x, texel3.y, Hsla(values3.z, values3.w, values4.x, values4.y), values4.z,
            array<LinearColorStop, 2>(
                LinearColorStop(Hsla(values4.w, values5.x, values5.y, values5.z), values5.w),
                LinearColorStop(Hsla(values6.x, values6.y, values6.z, values6.w), values7.x)),
            texel7.y),
        Hsla(values7.z, values7.w, values8.x, values8.y),
        Corners(values8.z, values8.w, values9.x, values9.y),
        Edges(values9.z, values9.w, values10.x, values10.y),
        PrimitiveTransform(values10.z, values10.w, values11.x, values11.y),
    );
}

fn load_shadow(instance_id: u32) -> Shadow {
    var cursor = instance_cursor(instance_id * 34u);
    return Shadow(
        read_word(&cursor), read_f32(&cursor), read_bounds(&cursor),
        read_corners(&cursor), read_clip(&cursor), read_hsla(&cursor),
        read_bounds(&cursor), read_corners(&cursor), read_word(&cursor),
        read_word(&cursor), read_transform(&cursor),
    );
}

fn load_path_tile(instance_id: u32) -> PathTile {
    var cursor = instance_cursor(instance_id * 14u);
    return PathTile(
        read_word(&cursor), read_word(&cursor), read_word(&cursor), read_i32(&cursor),
        read_vec2_f32(&cursor), read_vec2_f32(&cursor), read_vec2_f32(&cursor),
        read_vec2_f32(&cursor), read_vec2_f32(&cursor),
    );
}

fn load_path_curve(index: u32) -> PathCurve {
    var cursor = instance_cursor((globals.path_curve_base + index) * 10u);
    return PathCurve(
        read_vec2_f32(&cursor), read_vec2_f32(&cursor), read_vec2_f32(&cursor),
        read_vec2_f32(&cursor), read_f32(&cursor), read_word(&cursor),
    );
}

fn load_path_paint(index: u32) -> PathPaint {
    var cursor = instance_cursor((globals.path_paint_base + index) * 34u);
    return PathPaint(
        read_bounds(&cursor), read_clip(&cursor), read_background(&cursor),
        read_word(&cursor), read_word(&cursor), read_transform(&cursor),
    );
}

fn load_underline(instance_id: u32) -> Underline {
    var cursor = instance_cursor(instance_id * 22u);
    return Underline(
        read_word(&cursor), read_word(&cursor), read_bounds(&cursor), read_clip(&cursor),
        read_hsla(&cursor), read_f32(&cursor), read_word(&cursor), read_transform(&cursor),
    );
}

fn load_mono_sprite(instance_id: u32) -> MonochromeSprite {
    var cursor = instance_cursor(instance_id * 36u);
    return MonochromeSprite(
        read_word(&cursor), read_word(&cursor), read_atlas_tile(&cursor), read_bounds(&cursor),
        read_clip(&cursor), read_hsla(&cursor), read_transform(&cursor), read_transformation(&cursor),
    );
}

fn load_subpixel_sprite(instance_id: u32) -> SubpixelSprite {
    var cursor = instance_cursor(instance_id * 30u);
    return SubpixelSprite(
        read_word(&cursor), read_word(&cursor), read_atlas_tile(&cursor), read_bounds(&cursor),
        read_clip(&cursor), read_hsla(&cursor), read_transform(&cursor),
    );
}

fn load_poly_sprite(instance_id: u32) -> PolychromeSprite {
    var cursor = instance_cursor(instance_id * 32u);
    return PolychromeSprite(
        read_word(&cursor), read_word(&cursor), read_atlas_tile(&cursor), read_word(&cursor),
        read_f32(&cursor), read_bounds(&cursor), read_clip(&cursor), read_corners(&cursor),
        read_transform(&cursor),
    );
}
