#include "alpha_correction.hlsl"

cbuffer GlobalParams: register(b0) {
    float4 gamma_ratios;
    float2 global_viewport_size;
    float grayscale_enhanced_contrast;
    float subpixel_enhanced_contrast;
    uint is_bgr;
    uint global_clip_shape_count;
    float depth_scale;
    uint global_pad;
};

cbuffer BatchParams: register(b1) {
    uint batch_start_index;
    uint3 batch_pad;
};

Texture2D<float4> t_sprite: register(t0);
SamplerState s_sprite: register(s0);

struct SubpixelSpriteFragmentOutput {
    float4 foreground : SV_Target0;
    float4 alpha : SV_Target1;
};

struct Bounds {
    float2 origin;
    float2 size;
};

struct Corners {
    float top_left;
    float top_right;
    float bottom_right;
    float bottom_left;
};

struct GpuClipShape {
    Bounds bounds;
    Corners radii_x;
    Corners radii_y;
};

struct ClipEnvelope {
    Bounds conservative_bounds;
    uint first_clip;
    uint clip_count;
};

StructuredBuffer<GpuClipShape> clip_shapes: register(t2);

struct Edges {
    float top;
    float right;
    float bottom;
    float left;
};

struct Hsla {
    float h;
    float s;
    float l;
    float a;
};

struct LinearColorStop {
    Hsla color;
    float percentage;
};

struct Background {
    // 0u is Solid
    // 1u is LinearGradient
    // 2u is PatternSlash
    uint tag;
    // 0u is sRGB color
    // 1u is Oklab color
    uint color_space;
    Hsla solid;
    float gradient_angle_or_pattern_height;
    LinearColorStop colors[2];
    uint pad;
};

struct PreparedBackground {
    nointerpolation float4 color0: COLOR1;
    nointerpolation float4 color1: COLOR2;
    nointerpolation uint tag: TEXCOORD2;
};

struct AtlasTextureId {
    uint index;
    uint kind;
};

struct AtlasBounds {
    int2 origin;
    int2 size;
};

struct AtlasTile {
    AtlasTextureId texture_id;
    uint tile_id;
    uint padding;
    AtlasBounds bounds;
    uint texture_generation;
    uint texture_generation_padding;
};

struct PrimitiveTransform {
    float scale_x;
    float scale_y;
    float translation_x;
    float translation_y;
};

static const float M_PI_F = 3.141592653f;
static const float3 GRAYSCALE_FACTORS = float3(0.2126f, 0.7152f, 0.0722f);
static const uint SIMPLE_PRIMITIVE_FLAG = 1u << 31;
static const float4 CULLED_POSITION = float4(0.0, 0.0, 0.0, 1.0);

float quad_depth(uint quad_id) {
    return saturate(float(quad_id + 1u) * depth_scale);
}

float4 to_device_position_impl(float2 position) {
    float2 device_position = position / global_viewport_size * float2(2.0, -2.0) + float2(-1.0, 1.0);
    return float4(device_position, 0., 1.);
}

float2 apply_primitive_transform(float2 position, PrimitiveTransform transform) {
    return position * float2(transform.scale_x, transform.scale_y)
        + float2(transform.translation_x, transform.translation_y);
}

float2 inverse_primitive_transform(float2 position, PrimitiveTransform transform) {
    return (position - float2(transform.translation_x, transform.translation_y))
        / float2(transform.scale_x, transform.scale_y);
}

struct ClippedVertex {
    float2 position;
    float2 local_position;
    float2 unit_vertex;
};

bool has_precise_raster_coordinates(Bounds bounds, PrimitiveTransform transform) {
    float2 local_magnitude = abs(bounds.origin) + bounds.size;
    float2 window_magnitude = local_magnitude * float2(transform.scale_x, transform.scale_y)
        + abs(float2(transform.translation_x, transform.translation_y));
    return all(local_magnitude <= 32768.0) && all(window_magnitude <= 32768.0);
}

Bounds pixel_raster_mask(Bounds bounds) {
    Bounds mask;
    mask.origin = floor(bounds.origin);
    mask.size = max(ceil(bounds.origin + bounds.size) - mask.origin, 0.0);
    return mask;
}

ClippedVertex clip_to_mask(float2 unit_vertex, Bounds bounds, Bounds mask,
                           PrimitiveTransform transform) {
    ClippedVertex result;
    result.local_position = unit_vertex * bounds.size + bounds.origin;
    result.position = apply_primitive_transform(result.local_position, transform);
    result.unit_vertex = unit_vertex;
    if (!has_precise_raster_coordinates(bounds, transform)) {
        return result;
    }
    float2 clipped = clamp(result.position, mask.origin, mask.origin + max(mask.size, 0.0));
    float2 delta = (clipped - result.position) / float2(transform.scale_x, transform.scale_y);
    result.local_position.x += clipped.x == result.position.x ? 0.0 : delta.x;
    result.local_position.y += clipped.y == result.position.y ? 0.0 : delta.y;
    if (clipped.x != result.position.x && bounds.size.x > 0.0) {
        result.unit_vertex.x = (result.local_position.x - bounds.origin.x) / bounds.size.x;
    }
    if (clipped.y != result.position.y && bounds.size.y > 0.0) {
        result.unit_vertex.y = (result.local_position.y - bounds.origin.y) / bounds.size.y;
    }
    result.position = clipped;
    return result;
}

bool rounded_clip_corner_contains(
    float2 position,
    float2 center,
    float2 radii,
    bool in_corner
) {
    if (!in_corner || radii.x == 0.0 || radii.y == 0.0) {
        return true;
    }
    float2 delta = (position - center) / radii;
    return dot(delta, delta) <= 1.0;
}

bool rounded_clip_shape_contains(float2 position, GpuClipShape shape) {
    float left = shape.bounds.origin.x;
    float top = shape.bounds.origin.y;
    float right = left + shape.bounds.size.x;
    float bottom = top + shape.bounds.size.y;
    if (shape.bounds.size.x <= 0.0 || shape.bounds.size.y <= 0.0 ||
        position.x < left || position.x > right ||
        position.y < top || position.y > bottom) {
        return false;
    }

    float2 top_left_radii = float2(shape.radii_x.top_left, shape.radii_y.top_left);
    float2 top_right_radii = float2(shape.radii_x.top_right, shape.radii_y.top_right);
    float2 bottom_right_radii = float2(shape.radii_x.bottom_right, shape.radii_y.bottom_right);
    float2 bottom_left_radii = float2(shape.radii_x.bottom_left, shape.radii_y.bottom_left);

    return rounded_clip_corner_contains(
        position,
        float2(left, top) + top_left_radii,
        top_left_radii,
        position.x < left + top_left_radii.x && position.y < top + top_left_radii.y
    ) && rounded_clip_corner_contains(
        position,
        float2(right - top_right_radii.x, top + top_right_radii.y),
        top_right_radii,
        position.x > right - top_right_radii.x && position.y < top + top_right_radii.y
    ) && rounded_clip_corner_contains(
        position,
        float2(right, bottom) - bottom_right_radii,
        bottom_right_radii,
        position.x > right - bottom_right_radii.x && position.y > bottom - bottom_right_radii.y
    ) && rounded_clip_corner_contains(
        position,
        float2(left + bottom_left_radii.x, bottom - bottom_left_radii.y),
        bottom_left_radii,
        position.x < left + bottom_left_radii.x && position.y > bottom - bottom_left_radii.y
    );
}

bool exact_clip_stack_contains(float2 position, uint2 range) {
    if (range.y == 0u || range.x >= global_clip_shape_count) {
        return false;
    }
    if (range.y > global_clip_shape_count - range.x) {
        return false;
    }

    [loop]
    for (uint index = 0u; index < range.y; index++) {
        if (!rounded_clip_shape_contains(position, clip_shapes[range.x + index])) {
            return false;
        }
    }
    return true;
}

bool clip_stack_contains(float2 position, ClipEnvelope envelope) {
    float2 envelope_min = envelope.conservative_bounds.origin;
    float2 envelope_max = envelope_min + envelope.conservative_bounds.size;
    if (any(envelope.conservative_bounds.size <= 0.0) ||
        any(position < envelope_min) || any(position >= envelope_max)) {
        return false;
    }

    return exact_clip_stack_contains(position, uint2(envelope.first_clip, envelope.clip_count));
}

bool clip_stack_contains(float2 position, float4 bounds, uint2 range) {
    ClipEnvelope envelope;
    envelope.conservative_bounds.origin = bounds.xy;
    envelope.conservative_bounds.size = bounds.zw;
    envelope.first_clip = range.x;
    envelope.clip_count = range.y;
    return clip_stack_contains(position, envelope);
}

// Convert linear RGB to sRGB
float3 linear_to_srgb(float3 color) {
    return pow(color, float3(1.0 / 2.2, 1.0 / 2.2, 1.0 / 2.2));
}

// Convert sRGB to linear RGB
float3 srgb_to_linear(float3 color) {
    return pow(color, float3(2.2, 2.2, 2.2));
}

/// Hsla to linear RGBA conversion.
float4 hsla_to_rgba(Hsla hsla) {
    float h = hsla.h * 6.0; // Now, it's an angle but scaled in [0, 6) range
    float s = hsla.s;
    float l = hsla.l;
    float a = hsla.a;

    float c = (1.0 - abs(2.0 * l - 1.0)) * s;
    float x = c * (1.0 - abs(fmod(h, 2.0) - 1.0));
    float m = l - c / 2.0;

    float r = 0.0;
    float g = 0.0;
    float b = 0.0;

    if (h >= 0.0 && h < 1.0) {
        r = c;
        g = x;
        b = 0.0;
    } else if (h >= 1.0 && h < 2.0) {
        r = x;
        g = c;
        b = 0.0;
    } else if (h >= 2.0 && h < 3.0) {
        r = 0.0;
        g = c;
        b = x;
    } else if (h >= 3.0 && h < 4.0) {
        r = 0.0;
        g = x;
        b = c;
    } else if (h >= 4.0 && h < 5.0) {
        r = x;
        g = 0.0;
        b = c;
    } else {
        r = c;
        g = 0.0;
        b = x;
    }

    float4 rgba;
    rgba.x = (r + m);
    rgba.y = (g + m);
    rgba.z = (b + m);
    rgba.w = a;
    return rgba;
}

// Converts a sRGB color to the Oklab color space.
// Reference: https://bottosson.github.io/posts/oklab/#converting-from-linear-srgb-to-oklab
float4 srgb_to_oklab(float4 color) {
    // Convert non-linear sRGB to linear sRGB
    color = float4(srgb_to_linear(color.rgb), color.a);

    float l = 0.4122214708 * color.r + 0.5363325363 * color.g + 0.0514459929 * color.b;
    float m = 0.2119034982 * color.r + 0.6806995451 * color.g + 0.1073969566 * color.b;
    float s = 0.0883024619 * color.r + 0.2817188376 * color.g + 0.6299787005 * color.b;

    float l_ = pow(l, 1.0/3.0);
    float m_ = pow(m, 1.0/3.0);
    float s_ = pow(s, 1.0/3.0);

    return float4(
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
        color.a
    );
}

// Converts an Oklab color to the sRGB color space.
float4 oklab_to_srgb(float4 color) {
    float l_ = color.r + 0.3963377774 * color.g + 0.2158037573 * color.b;
    float m_ = color.r - 0.1055613458 * color.g - 0.0638541728 * color.b;
    float s_ = color.r - 0.0894841775 * color.g - 1.2914855480 * color.b;

    float l = l_ * l_ * l_;
    float m = m_ * m_ * m_;
    float s = s_ * s_ * s_;

    float3 linear_rgb = float3(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    );

    // Convert linear sRGB to non-linear sRGB
    return float4(linear_to_srgb(linear_rgb), color.a);
}

float4 over(float4 below, float4 above) {
    float4 result;
    float alpha = above.a + below.a * (1.0 - above.a);
    result.rgb = (above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha;
    result.a = alpha;
    return result;
}

float2 to_tile_position(float2 unit_vertex, AtlasTile tile) {
    float2 atlas_size;
    t_sprite.GetDimensions(atlas_size.x, atlas_size.y);
    return (float2(tile.bounds.origin) + unit_vertex * float2(tile.bounds.size)) / atlas_size;
}

// Selects corner radius based on quadrant.
float pick_corner_radius(float2 center_to_point, Corners corner_radii) {
    if (center_to_point.x < 0.) {
        if (center_to_point.y < 0.) {
            return corner_radii.top_left;
        } else {
            return corner_radii.bottom_left;
        }
    } else {
        if (center_to_point.y < 0.) {
            return corner_radii.top_right;
        } else {
            return corner_radii.bottom_right;
        }
    }
}

// Implementation of quad signed distance field
float quad_sdf_impl(float2 corner_center_to_point, float corner_radius) {
    if (corner_radius == 0.0) {
        // Fast path for unrounded corners
        return max(corner_center_to_point.x, corner_center_to_point.y);
    } else {
        // Signed distance of the point from a quad that is inset by corner_radius
        // It is negative inside this quad, and positive outside
        float signed_distance_to_inset_quad =
            // 0 inside the inset quad, and positive outside
            length(max(float2(0.0, 0.0), corner_center_to_point)) +
            // 0 outside the inset quad, and negative inside
            min(0.0, max(corner_center_to_point.x, corner_center_to_point.y));

        return signed_distance_to_inset_quad - corner_radius;
    }
}

float quad_sdf(float2 pt, Bounds bounds, Corners corner_radii) {
    float2 half_size = bounds.size / 2.;
    float2 center = bounds.origin + half_size;
    float2 center_to_point = pt - center;
    float corner_radius = pick_corner_radius(center_to_point, corner_radii);
    float2 corner_to_point = abs(center_to_point) - half_size;
    float2 corner_center_to_point = corner_to_point + corner_radius;
    return quad_sdf_impl(corner_center_to_point, corner_radius);
}

PreparedBackground prepare_background(Background background) {
    PreparedBackground output;
    output.tag = background.tag;
    output.color1 = float4(0.0, 0.0, 0.0, 0.0);
    if (background.tag == 1u) {
        output.color0 = hsla_to_rgba(background.colors[0].color);
        output.color1 = hsla_to_rgba(background.colors[1].color);
        if (background.color_space == 1u) {
            output.color0 = srgb_to_oklab(output.color0);
            output.color1 = srgb_to_oklab(output.color1);
        }
    } else {
        output.color0 = hsla_to_rgba(background.solid);
    }

    return output;
}

float dither_offset(float2 position) {
    uint2 cell = uint2(position) & 3u;
    uint z = cell.x ^ cell.y;
    uint threshold = ((z & 1u) << 3)
        | ((cell.y & 1u) << 2)
        | (z & 2u)
        | ((cell.y & 2u) >> 1);
    return (float(threshold) + 0.5) * (1.0 / 8.0) - 1.0;
}

float4 background_color(PreparedBackground prepared, Background background, Bounds bounds,
                        float2 position) {
    float4 color = prepared.color0;
    if (prepared.tag == 0u) {
        return color;
    }

    switch (prepared.tag) {
        case 1: {
            float radians = (fmod(background.gradient_angle_or_pattern_height, 360.0) - 90.0)
                * (M_PI_F / 180.0);
            float2 direction = float2(cos(radians), sin(radians));
            if (bounds.size.x > bounds.size.y) {
                direction.y *= bounds.size.y / bounds.size.x;
            } else {
                direction.x *= bounds.size.x / bounds.size.y;
            }
            float extent = abs(direction.x) > abs(direction.y) ? bounds.size.x : bounds.size.y;
            float direction_length = length(direction);
            float span = background.colors[1].percentage - background.colors[0].percentage;
            float offset = 0.5 - background.colors[0].percentage;
            float3 gradient = float3(0.0, 0.0, 0.0);
            if (direction_length != 0.0 && extent != 0.0) {
                if (span == 0.0) {
                    const float STEP = 1e30;
                    gradient = float3(direction / (direction_length * extent) * STEP, offset * STEP);
                } else {
                    gradient = float3(direction / (direction_length * extent * span), offset / span);
                }
            }
            float2 center = bounds.origin + bounds.size * 0.5;
            float t = saturate(dot(position - center, gradient.xy) + gradient.z);
            color = lerp(prepared.color0, prepared.color1, t);
            if (background.color_space == 1u) {
                color = oklab_to_srgb(color);
            }
            float dither = dither_offset(position);
            color.rgb += dither * 1.0 / 255.0;
            color.a += dither * 1.5 / 255.0;
            break;
        }
        case 2: {
            float pattern_height_encoded = background.gradient_angle_or_pattern_height;
            float pattern_width = (pattern_height_encoded / 65535.0) / 255.0;
            float pattern_interval = fmod(pattern_height_encoded, 65535.0) / 255.0;
            float pattern_height = pattern_width + pattern_interval;
            float stripe_angle = M_PI_F / 4.0;
            float2 stripe_direction = float2(cos(stripe_angle), sin(stripe_angle));
            float pattern_period = pattern_height * sin(stripe_angle);
            float threshold = pattern_period * (pattern_width / pattern_height) / 2.0;
            float pattern = fmod(dot(position - bounds.origin, stripe_direction), pattern_period);
            float distance = min(pattern, pattern_period - pattern) - threshold;
            color.a *= saturate(0.5 - distance);
            break;
        }
        case 3: {
            float cell_size = background.gradient_angle_or_pattern_height;
            float2 relative_position = position - bounds.origin;
            float x_index = floor(relative_position.x / cell_size);
            float y_index = floor(relative_position.y / cell_size);
            color.a *= saturate((x_index + y_index) % 2.0);
            break;
        }
    }

    return color;
}

// Returns the dash velocity of a corner given the dash velocity of the two
// sides, by returning the slower velocity (larger dashes).
//
// Since 0 is used for dash velocity when the border width is 0 (instead of
// +inf), this returns the other dash velocity in that case.
//
// An alternative to this might be to appropriately interpolate the dash
// velocity around the corner, but that seems overcomplicated.
float corner_dash_velocity(float dv1, float dv2) {
    if (dv1 == 0.0) {
        return dv2;
    } else if (dv2 == 0.0) {
        return dv1;
    } else {
        return min(dv1, dv2);
    }
}

// Returns alpha used to render antialiased dashes.
// `t` is within the dash when `fmod(t, period) < length`.
float dash_alpha(
    float t, float period, float length, float dash_velocity,
    float antialias_threshold
) {
    float half_period = period / 2.0;
    float half_length = length / 2.0;
    // Value in [-half_period, half_period]
    // The dash is in [-half_length, half_length]
    float centered = fmod(t + half_period - half_length, period) - half_period;
    // Signed distance for the dash, negative values are inside the dash
    float signed_distance = abs(centered) - half_length;
    // Antialiased alpha based on the signed distance
    return saturate(antialias_threshold - signed_distance / dash_velocity);
}

// This approximates distance to the nearest point to a quarter ellipse in a way
// that is sufficient for anti-aliasing when the ellipse is not very eccentric.
// The components of `point` are expected to be positive.
//
// Negative on the outside and positive on the inside.
float quarter_ellipse_sdf(float2 pt, float2 radii) {
    // Scale the space to treat the ellipse like a unit circle
    float2 circle_vec = pt / radii;
    float unit_circle_sdf = length(circle_vec) - 1.0;
    // Approximate up-scaling of the length by using the average of the radii.
    //
    // TODO: A better solution would be to use the gradient of the implicit
    // function for an ellipse to approximate a scaling factor.
    return unit_circle_sdf * (radii.x + radii.y) * -0.5;
}

/*
**
**              Quads
**
*/

struct Quad {
    uint order;
    uint border_style;
    Bounds bounds;
    ClipEnvelope clip;
    Background background;
    Hsla border_color;
    Corners corner_radii;
    Edges border_widths;
    PrimitiveTransform transform;
};

struct QuadFragmentInput {
    nointerpolation uint quad_id: TEXCOORD0;
    float4 position: SV_Position;
    nointerpolation float4 border_color: COLOR0;
    PreparedBackground background;
    float2 local_position: TEXCOORD1;
    nointerpolation uint2 clip_range: TEXCOORD9;
    nointerpolation float4 clip_bounds: TEXCOORD10;
};

bool has_rounded_corners(Corners radii) {
    return radii.top_left != 0.0 || radii.top_right != 0.0
        || radii.bottom_right != 0.0 || radii.bottom_left != 0.0;
}

bool has_border(Edges widths) {
    return widths.top != 0.0 || widths.right != 0.0
        || widths.bottom != 0.0 || widths.left != 0.0;
}

bool clip_stack_is_rectangular(ClipEnvelope envelope) {
    if (envelope.clip_count == 0u || envelope.first_clip >= global_clip_shape_count
        || envelope.clip_count > global_clip_shape_count - envelope.first_clip) {
        return false;
    }
    [loop]
    for (uint index = 0u; index < envelope.clip_count; index++) {
        GpuClipShape shape = clip_shapes[envelope.first_clip + index];
        if (has_rounded_corners(shape.radii_x) || has_rounded_corners(shape.radii_y)) {
            return false;
        }
    }
    return true;
}

bool is_opaque_rectangle(Quad quad) {
    return quad.background.tag == 0u && quad.background.solid.a >= 1.0
        && !has_rounded_corners(quad.corner_radii) && !has_border(quad.border_widths)
        && has_precise_raster_coordinates(quad.bounds, quad.transform)
        && clip_stack_is_rectangular(quad.clip);
}

StructuredBuffer<Quad> quads: register(t1);

QuadFragmentInput quad_vertex(uint vertex_id: SV_VertexID, uint instance_id: SV_InstanceID) {
    uint quad_id = batch_start_index + instance_id;
    Quad quad = quads[quad_id];
    if (is_opaque_rectangle(quad)) {
        QuadFragmentInput culled = (QuadFragmentInput)0;
        culled.position = CULLED_POSITION;
        return culled;
    }
    float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
    ClippedVertex vertex = clip_to_mask(unit_vertex, quad.bounds,
                                       pixel_raster_mask(quad.clip.conservative_bounds), quad.transform);

    QuadFragmentInput output;
    output.position = to_device_position_impl(vertex.position);
    output.position.z = quad_depth(quad_id);
    output.border_color = hsla_to_rgba(quad.border_color);
    bool simple = !has_rounded_corners(quad.corner_radii)
        && !has_border(quad.border_widths) && quad.background.tag == 0u;
    output.quad_id = quad_id | (simple ? SIMPLE_PRIMITIVE_FLAG : 0u);
    output.background = prepare_background(quad.background);
    output.local_position = vertex.local_position;
    output.clip_range = uint2(quad.clip.first_clip, quad.clip.clip_count);
    output.clip_bounds = float4(quad.clip.conservative_bounds.origin,
                                quad.clip.conservative_bounds.size);
    return output;
}

float4 quad_fragment(QuadFragmentInput input): SV_Target {
    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }
    if ((input.quad_id & SIMPLE_PRIMITIVE_FLAG) != 0u) {
        return input.background.color0;
    }
    Quad quad = quads[input.quad_id];
    float4 fill = background_color(input.background, quad.background, quad.bounds,
                                   input.local_position);
    bool unrounded = !has_rounded_corners(quad.corner_radii);

    float2 size = quad.bounds.size;
    float2 half_size = size / 2.;
    float2 the_point = input.local_position - quad.bounds.origin;
    if (quad.border_style == 1u && has_precise_raster_coordinates(quad.bounds, quad.transform)) {
        float2 window_origin = apply_primitive_transform(quad.bounds.origin, quad.transform);
        the_point = (input.position.xy - window_origin)
            / float2(quad.transform.scale_x, quad.transform.scale_y);
    }
    float2 center_to_point = the_point - half_size;

    // Signed distance field threshold for inclusion of pixels. 0.5 is the
    // minimum distance between the center of the pixel and the edge.
    const float antialias_threshold = 0.5;

    // Radius of the nearest corner
    float corner_radius = pick_corner_radius(center_to_point, quad.corner_radii);

    float2 border = float2(
        center_to_point.x < 0.0 ? quad.border_widths.left : quad.border_widths.right,
        center_to_point.y < 0.0 ? quad.border_widths.top : quad.border_widths.bottom
    );

    // 0-width borders are reduced so that `inner_sdf >= antialias_threshold`.
    // The purpose of this is to not draw antialiasing pixels in this case.
    float2 reduced_border = float2(
        border.x == 0.0 ? -antialias_threshold : border.x,
        border.y == 0.0 ? -antialias_threshold : border.y
    );

    // Vector from the corner of the quad bounds to the point, after mirroring
    // the point into the bottom right quadrant. Both components are <= 0.
    float2 corner_to_point = abs(center_to_point) - half_size;

    // Vector from the point to the center of the rounded corner's circle, also
    // mirrored into bottom right quadrant.
    float2 corner_center_to_point = corner_to_point + corner_radius;

    // Whether the nearest point on the border is rounded
    bool is_near_rounded_corner =
        corner_center_to_point.x >= 0.0 &&
        corner_center_to_point.y >= 0.0;

    // Vector from straight border inner corner to point.
    //
    // 0-width borders are turned into width -1 so that inner_sdf is > 1.0 near
    // the border. Without this, antialiasing pixels would be drawn.
    float2 straight_border_inner_corner_to_point = corner_to_point + reduced_border;

    // Whether the point is beyond the inner edge of the straight border
    bool is_beyond_inner_straight_border =
        straight_border_inner_corner_to_point.x > 0.0 ||
        straight_border_inner_corner_to_point.y > 0.0;

    // Whether the point is far enough inside the quad, such that the pixels are
    // not affected by the straight border.
    bool is_within_inner_straight_border =
        straight_border_inner_corner_to_point.x < -antialias_threshold &&
        straight_border_inner_corner_to_point.y < -antialias_threshold;

    // Fast path for points that must be part of the background
    if (is_within_inner_straight_border && !is_near_rounded_corner) {
        return fill;
    }

    // Signed distance of the point to the outside edge of the quad's border
    float outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);

    // Approximate signed distance of the point to the inside edge of the quad's
    // border. It is negative outside this edge (within the border), and
    // positive inside.
    //
    // This is not always an accurate signed distance:
    // * The rounded portions with varying border width use an approximation of
    //   nearest-point-on-ellipse.
    // * When it is quickly known to be outside the edge, -1.0 is used.
    float inner_sdf = 0.0;
    if (corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0) {
        // Fast paths for straight borders
        inner_sdf = -max(straight_border_inner_corner_to_point.x,
                        straight_border_inner_corner_to_point.y);
    } else if (is_beyond_inner_straight_border) {
        // Fast path for points that must be outside the inner edge
        inner_sdf = -1.0;
    } else if (reduced_border.x == reduced_border.y) {
        // Fast path for circular inner edge.
        inner_sdf = -(outer_sdf + reduced_border.x);
    } else {
        float2 ellipse_radii = max(float2(0.0, 0.0), float2(corner_radius, corner_radius) - reduced_border);
        inner_sdf = quarter_ellipse_sdf(corner_center_to_point, ellipse_radii);
    }

    // Negative when inside the border
    float border_sdf = max(inner_sdf, outer_sdf);

    float4 color = fill;
    if (border_sdf < antialias_threshold) {
        float4 border_color = input.border_color;
        // Dashed border logic when border_style == 1
        if (quad.border_style == 1) {
            // Position along the perimeter in "dash space", where each dash
            // period has length 1
            float t = 0.0;

            // Total number of dash periods, so that the dash spacing can be
            // adjusted to evenly divide it
            float max_t = 0.0;

            // Border width is proportional to dash size. This is the behavior
            // used by browsers, but also avoids dashes from different segments
            // overlapping when dash size is smaller than the border width.
            //
            // Dash pattern: (2 * border width) dash, (1 * border width) gap
            const float dash_length_per_width = 2.0;
            const float dash_gap_per_width = 1.0;
            const float dash_period_per_width = dash_length_per_width + dash_gap_per_width;

            // Since the dash size is determined by border width, the density of
            // dashes varies. Multiplying a pixel distance by this returns a
            // position in dash space - it has units (dash period / pixels). So
            // a dash velocity of (1 / 10) is 1 dash every 10 pixels.
            float dash_velocity = 0.0;

            // Dividing this by the border width gives the dash velocity
            const float dv_numerator = 1.0 / dash_period_per_width;

            if (unrounded) {
                // When corners aren't rounded, the dashes are separately laid
                // out on each straight line, rather than around the whole
                // perimeter. This way each line starts and ends with a dash.
                bool is_horizontal = corner_center_to_point.x < corner_center_to_point.y;
                // Choosing the right border width for dashed borders.
                // TODO: A better solution exists taking a look at the whole file.
                // this does not fix single dashed borders at the corners
                float2 dashed_border = float2(
                    max(quad.border_widths.bottom, quad.border_widths.top),
                    max(quad.border_widths.right, quad.border_widths.left)
                );
                float border_width = is_horizontal ? dashed_border.x : dashed_border.y;
                dash_velocity = dv_numerator / border_width;
                t = is_horizontal ? the_point.x : the_point.y;
                t *= dash_velocity;
                max_t = is_horizontal ? size.x : size.y;
                max_t *= dash_velocity;
            } else {
                // When corners are rounded, the dashes are laid out clockwise
                // around the whole perimeter.

                float r_tr = quad.corner_radii.top_right;
                float r_br = quad.corner_radii.bottom_right;
                float r_bl = quad.corner_radii.bottom_left;
                float r_tl = quad.corner_radii.top_left;

                float w_t = quad.border_widths.top;
                float w_r = quad.border_widths.right;
                float w_b = quad.border_widths.bottom;
                float w_l = quad.border_widths.left;

                // Straight side dash velocities
                float dv_t = w_t <= 0.0 ? 0.0 : dv_numerator / w_t;
                float dv_r = w_r <= 0.0 ? 0.0 : dv_numerator / w_r;
                float dv_b = w_b <= 0.0 ? 0.0 : dv_numerator / w_b;
                float dv_l = w_l <= 0.0 ? 0.0 : dv_numerator / w_l;

                // Straight side lengths in dash space
                float s_t = (size.x - r_tl - r_tr) * dv_t;
                float s_r = (size.y - r_tr - r_br) * dv_r;
                float s_b = (size.x - r_br - r_bl) * dv_b;
                float s_l = (size.y - r_bl - r_tl) * dv_l;

                float corner_dash_velocity_tr = corner_dash_velocity(dv_t, dv_r);
                float corner_dash_velocity_br = corner_dash_velocity(dv_b, dv_r);
                float corner_dash_velocity_bl = corner_dash_velocity(dv_b, dv_l);
                float corner_dash_velocity_tl = corner_dash_velocity(dv_t, dv_l);

                // Corner lengths in dash space
                float c_tr = r_tr * (M_PI_F / 2.0) * corner_dash_velocity_tr;
                float c_br = r_br * (M_PI_F / 2.0) * corner_dash_velocity_br;
                float c_bl = r_bl * (M_PI_F / 2.0) * corner_dash_velocity_bl;
                float c_tl = r_tl * (M_PI_F / 2.0) * corner_dash_velocity_tl;

                // Cumulative dash space upto each segment
                float upto_tr = s_t;
                float upto_r = upto_tr + c_tr;
                float upto_br = upto_r + s_r;
                float upto_b = upto_br + c_br;
                float upto_bl = upto_b + s_b;
                float upto_l = upto_bl + c_bl;
                float upto_tl = upto_l + s_l;
                max_t = upto_tl + c_tl;

                if (is_near_rounded_corner) {
                    float radians = atan2(corner_center_to_point.y, corner_center_to_point.x);
                    float corner_t = radians * corner_radius;

                    if (center_to_point.x >= 0.0) {
                        if (center_to_point.y < 0.0) {
                            dash_velocity = corner_dash_velocity_tr;
                            // Subtracted because radians is pi/2 to 0 when
                            // going clockwise around the top right corner,
                            // since the y axis has been flipped
                            t = upto_r - corner_t * dash_velocity;
                        } else {
                            dash_velocity = corner_dash_velocity_br;
                            // Added because radians is 0 to pi/2 when going
                            // clockwise around the bottom-right corner
                            t = upto_br + corner_t * dash_velocity;
                        }
                    } else {
                        if (center_to_point.y >= 0.0) {
                            dash_velocity = corner_dash_velocity_bl;
                            // Subtracted because radians is pi/1 to 0 when
                            // going clockwise around the bottom-left corner,
                            // since the x axis has been flipped
                            t = upto_l - corner_t * dash_velocity;
                        } else {
                            dash_velocity = corner_dash_velocity_tl;
                            // Added because radians is 0 to pi/2 when going
                            // clockwise around the top-left corner, since both
                            // axis were flipped
                            t = upto_tl + corner_t * dash_velocity;
                        }
                    }
                } else {
                    // Straight borders
                    bool is_horizontal = corner_center_to_point.x < corner_center_to_point.y;
                    if (is_horizontal) {
                        if (center_to_point.y < 0.0) {
                            dash_velocity = dv_t;
                            t = (the_point.x - r_tl) * dash_velocity;
                        } else {
                            dash_velocity = dv_b;
                            t = upto_bl - (the_point.x - r_bl) * dash_velocity;
                        }
                    } else {
                        if (center_to_point.x < 0.0) {
                            dash_velocity = dv_l;
                            t = upto_tl - (the_point.y - r_tl) * dash_velocity;
                        } else {
                            dash_velocity = dv_r;
                            t = upto_r + (the_point.y - r_tr) * dash_velocity;
                        }
                    }
                }
            }
            float dash_length = dash_length_per_width / dash_period_per_width;
            float desired_dash_gap = dash_gap_per_width / dash_period_per_width;

            // Straight borders should start and end with a dash, so max_t is
            // reduced to cause this.
            max_t -= unrounded ? dash_length : 0.0;
            if (max_t >= 1.0) {
                // Adjust dash gap to evenly divide max_t
                float dash_count = floor(max_t);
                float dash_period = max_t / dash_count;
                border_color.a *= dash_alpha(t, dash_period, dash_length, dash_velocity, antialias_threshold);
            } else if (unrounded) {
                // When there isn't enough space for the full gap between the
                // two start / end dashes of a straight border, reduce gap to
                // make them fit.
                float dash_gap = max_t - dash_length;
                if (dash_gap > 0.0) {
                    float dash_period = dash_length + dash_gap;
                    border_color.a *= dash_alpha(t, dash_period, dash_length, dash_velocity, antialias_threshold);
                }
            }
        }

        // Blend the border on top of the background and then linearly interpolate
        // between the two as we slide inside the background.
        float4 blended_border = over(fill, border_color);
        color = lerp(fill, blended_border,
                    saturate(antialias_threshold - inner_sdf));
    }

    return color * float4(1.0, 1.0, 1.0, saturate(antialias_threshold - outer_sdf));
}

/*
**
**              Opaque quads
**
*/

struct OpaqueQuadFragmentInput {
    float4 position: SV_Position;
    nointerpolation float4 color: COLOR0;
    nointerpolation float4 clip_bounds: TEXCOORD0;
};

OpaqueQuadFragmentInput opaque_quad_vertex(uint vertex_id: SV_VertexID,
                                            uint instance_id: SV_InstanceID) {
    uint quad_id = batch_start_index - instance_id;
    Quad quad = quads[quad_id];
    OpaqueQuadFragmentInput output;
    if (!is_opaque_rectangle(quad)) {
        output = (OpaqueQuadFragmentInput)0;
        output.position = CULLED_POSITION;
        return output;
    }
    float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
    ClippedVertex vertex = clip_to_mask(unit_vertex, quad.bounds,
                                       pixel_raster_mask(quad.clip.conservative_bounds), quad.transform);
    output.position = to_device_position_impl(vertex.position);
    output.position.z = quad_depth(quad_id);
    output.color = hsla_to_rgba(quad.background.solid);
    output.clip_bounds = float4(quad.clip.conservative_bounds.origin,
                                quad.clip.conservative_bounds.size);
    return output;
}

float4 opaque_quad_fragment(OpaqueQuadFragmentInput input): SV_Target {
    if (any(input.clip_bounds.zw <= 0.0) || any(input.position.xy < input.clip_bounds.xy)
        || any(input.position.xy >= input.clip_bounds.xy + input.clip_bounds.zw)) {
        discard;
    }
    return input.color;
}

/*
**
**              Shadows
**
*/

struct Shadow {
    uint order;
    float blur_radius;
    Bounds bounds;
    Corners corner_radii;
    ClipEnvelope clip;
    Hsla color;
    Bounds element_bounds;
    Corners element_corner_radii;
    uint inset;
    uint pad; // align to 8 bytes
    PrimitiveTransform transform;
};

struct PreparedBlurredRoundedRect {
    nointerpolation float4 shape: TEXCOORD1;
    nointerpolation float4 r1: TEXCOORD2;
    nointerpolation float4 exponent: TEXCOORD3;
    nointerpolation float4 scale: TEXCOORD4;
};

struct ShadowFragmentInput {
    nointerpolation uint shadow_id: TEXCOORD0;
    float4 position: SV_Position;
    nointerpolation float4 color: COLOR;
    PreparedBlurredRoundedRect blur;
    float2 local_position: TEXCOORD5;
    nointerpolation uint2 clip_range: TEXCOORD9;
    nointerpolation float4 clip_bounds: TEXCOORD10;
};

StructuredBuffer<Shadow> shadows: register(t1);

float erf7(float value) {
    value = clamp(value, -10., 10.) * (2. / sqrt(M_PI_F));
    float squared = value * value;
    value = value + (0.24295 + (0.03395 + 0.0104 * squared) * squared) * (value * squared);
    return value / sqrt(1. + value * value);
}

// Raph Levien's blurred rounded rectangle approximation, whose blur s is sigma * sqrt(2):
// https://raphlinus.github.io/graphics/2020/04/21/blurred-rounded-rects.html
PreparedBlurredRoundedRect prepare_blurred_rounded_rect(float2 size, Corners corner_radii,
                                                       float sigma) {
    float blur_scale = max(sigma * sqrt(2.), 1e-6);
    float inverse_scale = 1. / blur_scale;
    float min_edge = min(size.x, size.y);
    float max_radius = 0.5 * min_edge;
    float4 radii = float4(corner_radii.top_left, corner_radii.top_right,
                          corner_radii.bottom_right, corner_radii.bottom_left);
    float4 r0 = min(sqrt(radii * radii + 1.15 * 1.15 * blur_scale * blur_scale), max_radius);
    float4 r1 = min(sqrt(radii * radii + 4. * blur_scale * blur_scale), max_radius);
    float2 half_extent = 0.5 * inverse_scale * size;
    float delta = 1.25 * blur_scale
        * (exp(-half_extent.x * half_extent.x) - exp(-half_extent.y * half_extent.y));
    float2 adjusted_size = float2(size.x + min(delta, 0.), size.y - max(delta, 0.));
    float4 peak = inverse_scale * 0.5 * (max(adjusted_size.x, adjusted_size.y) - 0.5 * radii);

    PreparedBlurredRoundedRect output;
    output.shape = float4(0.5 * adjusted_size, inverse_scale, min_edge);
    output.r1 = r1;
    output.exponent = 2. * r1 / r0;
    output.scale = 0.5 * float4(erf7(peak.x), erf7(peak.y), erf7(peak.z), erf7(peak.w));
    return output;
}

float pick_corner(float2 center_to_point, float4 values) {
    Corners corners = { values.x, values.y, values.z, values.w };
    return pick_corner_radius(center_to_point, corners);
}

float blurred_rounded_rect(float2 center_to_point, PreparedBlurredRoundedRect prepared) {
    float radius = pick_corner(center_to_point, prepared.r1);
    float exponent = pick_corner(center_to_point, prepared.exponent);
    float scale = pick_corner(center_to_point, prepared.scale);
    float inverse_scale = prepared.shape.z;
    float min_edge = prepared.shape.w;
    float2 corner_delta = abs(center_to_point) - (prepared.shape.xy - radius);
    float2 positive_delta = max(corner_delta, 0.);
    float positive_distance;
    [branch] if (positive_delta.x > 0. && positive_delta.y > 0.) {
        positive_distance = pow(pow(positive_delta.x, exponent)
            + pow(positive_delta.y, exponent), 1. / exponent);
    } else {
        positive_distance = positive_delta.x + positive_delta.y;
    }
    float negative_distance = min(max(corner_delta.x, corner_delta.y), 0.);
    float distance = positive_distance + negative_distance - radius;
    return scale * (erf7(inverse_scale * (min_edge + distance)) - erf7(inverse_scale * distance));
}

ShadowFragmentInput shadow_vertex(uint vertex_id: SV_VertexID, uint instance_id: SV_InstanceID) {
    uint shadow_id = batch_start_index + instance_id;
    float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
    Shadow shadow = shadows[shadow_id];

    Bounds bounds;
    if (shadow.inset != 0u) {
        bounds = shadow.element_bounds;
    } else {
        float margin = 3.0 * shadow.blur_radius;
        bounds = shadow.bounds;
        bounds.origin -= margin;
        bounds.size += 2.0 * margin;
    }

    ClippedVertex vertex = clip_to_mask(unit_vertex, bounds,
                                       pixel_raster_mask(shadow.clip.conservative_bounds), shadow.transform);
    float4 color = hsla_to_rgba(shadow.color);

    ShadowFragmentInput output;
    output.position = to_device_position_impl(vertex.position);
    output.color = color;
    output.shadow_id = shadow_id;
    output.local_position = vertex.local_position;
    output.clip_range = uint2(shadow.clip.first_clip, shadow.clip.clip_count);
    output.clip_bounds = float4(shadow.clip.conservative_bounds.origin,
                                shadow.clip.conservative_bounds.size);
    output.blur = prepare_blurred_rounded_rect(shadow.bounds.size, shadow.corner_radii,
                                              shadow.blur_radius);

    return output;
}

float4 shadow_fragment(ShadowFragmentInput input): SV_TARGET {
    Shadow shadow = shadows[input.shadow_id];
    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }

    float2 half_size = shadow.bounds.size / 2.;
    float2 center = shadow.bounds.origin + half_size;
    float2 point0 = input.local_position - center;
    float alpha;
    if (shadow.blur_radius == 0.) {
        float distance = quad_sdf(input.local_position, shadow.bounds, shadow.corner_radii);
        alpha = saturate(0.5 - distance);
    } else {
        alpha = blurred_rounded_rect(point0, input.blur);
    }

    if (shadow.inset != 0u) {
        // The inset shadow is the complement of the (blurred) hole rect, clipped to the element.
        // `saturate(0.5 - d)` gives a 1-pixel antialiased edge: d <= -0.5 -> 1, d >= 0.5 -> 0.
        alpha = 1.0 - alpha;
        float element_distance = quad_sdf(input.local_position, shadow.element_bounds,
                                          shadow.element_corner_radii);
        alpha *= saturate(0.5 - element_distance);
    }

    return input.color * float4(1., 1., 1., alpha);
}

/*
**
**              Path Tiles
**
*/

struct PathCurve {
    float2 p0;
    float2 p1;
    float2 a;
    float2 b;
    float sign;
    uint pad;
};

struct PathTile {
    uint paint;
    uint curve_start;
    uint curve_count;
    int backdrop;
    float2 top_left;
    float2 bottom_right;
    float2 reference;
    float2 curves_top_left;
    float2 curves_bottom_right;
};

struct PathPaint {
    Bounds bounds;
    ClipEnvelope clip;
    Background color;
    uint even_odd;
    uint pad;
    PrimitiveTransform transform;
};

StructuredBuffer<PathTile> path_tiles: register(t1);
StructuredBuffer<PathCurve> path_curves: register(t3);
StructuredBuffer<PathPaint> path_paints: register(t4);

struct PathTileFragmentInput {
    float4 position: SV_Position;
    nointerpolation uint tile_id: TEXCOORD0;
    nointerpolation float4 curve_box: TEXCOORD1;
    PreparedBackground background;
    nointerpolation uint2 clip_range: TEXCOORD9;
    nointerpolation float4 clip_bounds: TEXCOORD10;
};

PathTileFragmentInput path_tile_vertex(uint vertex_id: SV_VertexID, uint tile_id: SV_InstanceID) {
    uint tile_index = batch_start_index + tile_id;
    PathTile tile = path_tiles[tile_index];
    PathPaint paint = path_paints[tile.paint];
    float2 mask_top_left = floor(paint.clip.conservative_bounds.origin);
    float2 mask_bottom_right = ceil(paint.clip.conservative_bounds.origin
                                    + paint.clip.conservative_bounds.size);
    float2 top_left = clamp(tile.top_left, mask_top_left, mask_bottom_right);
    float2 bottom_right = clamp(tile.bottom_right, top_left, mask_bottom_right);
    float2 position = float2(
        (vertex_id & 1u) != 0u ? bottom_right.x : top_left.x,
        (vertex_id & 2u) != 0u ? bottom_right.y : top_left.y);

    PathTileFragmentInput output;
    output.position = to_device_position_impl(position);
    output.tile_id = tile_index;
    output.curve_box = float4(tile.curves_top_left, tile.curves_bottom_right);
    output.background = prepare_background(paint.color);
    output.clip_range = uint2(paint.clip.first_clip, paint.clip.clip_count);
    output.clip_bounds = float4(paint.clip.conservative_bounds.origin,
                                paint.clip.conservative_bounds.size);
    return output;
}

float monotone_quadratic_root(float coefficient_a, float coefficient_b, float coefficient_c,
                              float direction) {
    float discriminant = max(coefficient_b * coefficient_b
        - 4.0 * coefficient_a * coefficient_c, 0.0);
    float denominator = coefficient_b + direction * sqrt(discriminant);
    if (denominator == 0.0) {
        return 0.0;
    }
    return saturate(-2.0 * coefficient_c / denominator);
}

// Coverage estimate adapted from https://github.com/EricLengyel/Slug/blob/be3c13eb7d63f9e8aa5c583e42d92c374cb91d98/SlugPixelShader.hlsl
// Copyright (c) 2017 Eric Lengyel.
// Licensed under the MIT license.

void add_curve_winding(PathCurve curve, float2 center, inout float2 winding, inout float2 weight) {
    float min_x = min(curve.p0.x, curve.p1.x);
    float max_x = max(curve.p0.x, curve.p1.x);

    [branch]
    if (curve.p0.y <= center.y && center.y < curve.p1.y) {
        [branch]
        if (max_x <= center.x - 0.5) {
            winding.x += curve.sign;
        } else {
            float root = monotone_quadratic_root(curve.a.y, curve.b.y, curve.p0.y - center.y, 1.0);
            float crossing = (curve.a.x * root + curve.b.x) * root + curve.p0.x;
            float distance = center.x - crossing;
            winding.x += curve.sign * saturate(distance + 0.5);
            weight.x = max(weight.x, saturate(1.0 - 2.0 * abs(distance)));
        }
    }

    [branch]
    if (min_x <= center.x && center.x < max_x) {
        float direction = curve.p1.x > curve.p0.x ? 1.0 : -1.0;
        [branch]
        if (curve.p1.y <= center.y - 0.5) {
            winding.y -= curve.sign * direction;
        } else if (curve.p0.y < center.y + 0.5) {
            float root = monotone_quadratic_root(curve.a.x, curve.b.x, curve.p0.x - center.x, direction);
            float crossing = (curve.a.y * root + curve.b.y) * root + curve.p0.y;
            float distance = center.y - crossing;
            winding.y -= curve.sign * direction * saturate(distance + 0.5);
            weight.y = max(weight.y, saturate(1.0 - 2.0 * abs(distance)));
        }
    }
}

void correct_for_endpoint(float2 endpoint, float step, float2 reference, float2 center,
                          inout float2 winding) {
    if (endpoint.x <= reference.x) {
        winding.x -= step * (float(endpoint.y <= center.y) - float(endpoint.y <= reference.y));
    }
    if (endpoint.y <= reference.y && endpoint.x <= center.x) {
        winding.y += step;
    }
}

float4 path_tile_fragment(PathTileFragmentInput input): SV_Target {
    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }
    float2 center = floor(input.position.xy) + 0.5;
    if (input.background.tag == 0u
        && (any(center < input.curve_box.xy) || any(center > input.curve_box.zw))) {
        return input.background.color0;
    }
    PathTile tile = path_tiles[input.tile_id];
    PathPaint paint = path_paints[tile.paint];
    float2 winding = float(tile.backdrop);
    float2 weight = float2(0.0, 0.0);
    [loop]
    for (uint index = 0u; index < tile.curve_count; index++) {
        PathCurve curve = path_curves[tile.curve_start + index];
        if (min(curve.p0.x, curve.p1.x) >= center.x + 0.5) {
            break;
        }
        if (curve.p0.y < center.y + 0.5) {
            add_curve_winding(curve, center, winding, weight);
            correct_for_endpoint(curve.p0, curve.sign, tile.reference, center, winding);
            correct_for_endpoint(curve.p1, -curve.sign, tile.reference, center, winding);
        }
    }

    float coverage = max(
        abs(dot(winding, weight)) / max(weight.x + weight.y, 1.0 / 65536.0),
        min(abs(winding.x), abs(winding.y)));
    coverage = paint.even_odd != 0u
        ? 1.0 - abs(1.0 - frac(coverage * 0.5) * 2.0)
        : saturate(coverage);
    if (coverage == 0.0) {
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    float2 local_position = inverse_primitive_transform(input.position.xy, paint.transform);
    float4 color = background_color(input.background, paint.color, paint.bounds, local_position);
    return float4(color.rgb, color.a * coverage);
}

/*
**
**              Underlines
**
*/

struct Underline {
    uint order;
    uint pad;
    Bounds bounds;
    ClipEnvelope clip;
    Hsla color;
    float thickness;
    uint wavy;
    PrimitiveTransform transform;
};

struct UnderlineFragmentInput {
  nointerpolation uint underline_id: TEXCOORD0;
  float4 position: SV_Position;
  nointerpolation float4 color: COLOR;
  float2 local_position: TEXCOORD1;
  nointerpolation uint2 clip_range: TEXCOORD9;
  nointerpolation float4 clip_bounds: TEXCOORD10;
};

StructuredBuffer<Underline> underlines: register(t1);

UnderlineFragmentInput underline_vertex(uint vertex_id: SV_VertexID, uint instance_id: SV_InstanceID) {
    uint underline_id = batch_start_index + instance_id;
    float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
    Underline underline = underlines[underline_id];
    ClippedVertex vertex = clip_to_mask(unit_vertex, underline.bounds,
                                       pixel_raster_mask(underline.clip.conservative_bounds), underline.transform);
    float4 color = hsla_to_rgba(underline.color);

    UnderlineFragmentInput output;
    output.position = to_device_position_impl(vertex.position);
    output.color = color;
    output.underline_id = underline_id | (underline.wavy == 0u ? SIMPLE_PRIMITIVE_FLAG : 0u);
    output.local_position = vertex.local_position;
    output.clip_range = uint2(underline.clip.first_clip, underline.clip.clip_count);
    output.clip_bounds = float4(underline.clip.conservative_bounds.origin,
                                underline.clip.conservative_bounds.size);
    return output;
}

float4 underline_fragment(UnderlineFragmentInput input): SV_Target {
    const float WAVE_FREQUENCY = 2.0;
    const float WAVE_HEIGHT_RATIO = 0.8;

    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }
    if ((input.underline_id & SIMPLE_PRIMITIVE_FLAG) != 0u) {
        return input.color;
    }
    Underline underline = underlines[input.underline_id];
    float half_thickness = underline.thickness * 0.5;
    float2 origin = underline.bounds.origin;
    float2 st = ((input.local_position - origin) / underline.bounds.size.y) - float2(0., 0.5);
    float frequency = (M_PI_F * WAVE_FREQUENCY * underline.thickness) / underline.bounds.size.y;
    float amplitude = (underline.thickness * WAVE_HEIGHT_RATIO) / underline.bounds.size.y;
    float sine = sin(st.x * frequency) * amplitude;
    float dSine = cos(st.x * frequency) * amplitude * frequency;
    float distance = (st.y - sine) / sqrt(1. + dSine * dSine);
    float distance_in_pixels = distance * underline.bounds.size.y;
    float distance_from_top_border = distance_in_pixels - half_thickness;
    float distance_from_bottom_border = distance_in_pixels + half_thickness;
    float alpha = saturate(0.5 - max(-distance_from_bottom_border, distance_from_top_border));
    return input.color * float4(1., 1., 1., alpha);
}

/*
**
**              Monochrome sprites
**
*/

struct MonochromeSprite {
    uint order;
    uint pad;
    AtlasTile tile;
    Bounds bounds;
    ClipEnvelope clip;
    Hsla color;
    PrimitiveTransform transform;
};

struct MonochromeSpriteFragmentInput {
    float4 position: SV_Position;
    float2 tile_position: POSITION;
    nointerpolation float4 color: COLOR;
    nointerpolation uint2 clip_range: TEXCOORD9;
    nointerpolation float4 clip_bounds: TEXCOORD10;
};

StructuredBuffer<MonochromeSprite> mono_sprites: register(t1);

MonochromeSpriteFragmentInput monochrome_sprite_vertex(uint vertex_id: SV_VertexID, uint instance_id: SV_InstanceID) {
    uint sprite_id = batch_start_index + instance_id;
    float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
    MonochromeSprite sprite = mono_sprites[sprite_id];
    ClippedVertex vertex = clip_to_mask(unit_vertex, sprite.bounds,
                                       pixel_raster_mask(sprite.clip.conservative_bounds), sprite.transform);
    float2 tile_position = to_tile_position(vertex.unit_vertex, sprite.tile);
    float4 color = hsla_to_rgba(sprite.color);

    MonochromeSpriteFragmentInput output;
    output.position = to_device_position_impl(vertex.position);
    output.tile_position = tile_position;
    output.color = color;
    output.clip_range = uint2(sprite.clip.first_clip, sprite.clip.clip_count);
    output.clip_bounds = float4(sprite.clip.conservative_bounds.origin,
                                sprite.clip.conservative_bounds.size);
    return output;
}

float4 monochrome_sprite_fragment(MonochromeSpriteFragmentInput input): SV_Target {
    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }
    float sample = t_sprite.Sample(s_sprite, input.tile_position).r;
    float alpha_corrected = apply_contrast_and_gamma_correction(sample, input.color.rgb, grayscale_enhanced_contrast, gamma_ratios);
    return float4(input.color.rgb, input.color.a * alpha_corrected);
}

MonochromeSpriteFragmentInput subpixel_sprite_vertex(uint vertex_id: SV_VertexID, uint instance_id: SV_InstanceID) {
    return monochrome_sprite_vertex(vertex_id, instance_id);
}

SubpixelSpriteFragmentOutput subpixel_sprite_fragment(MonochromeSpriteFragmentInput input) {
    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }
    float3 sample = t_sprite.Sample(s_sprite, input.tile_position).rgb;
    if (is_bgr) {
        sample = sample.bgr;
    }
    float3 alpha_corrected = apply_contrast_and_gamma_correction3(sample, input.color.rgb, subpixel_enhanced_contrast, gamma_ratios);

    SubpixelSpriteFragmentOutput output;
    output.foreground = float4(input.color.rgb, 1.0f);
    output.alpha = float4(input.color.a * alpha_corrected, 1.0f);
    return output;
}

/*
**
**              Polychrome sprites
**
*/

struct PolychromeSprite {
    uint order;
    uint pad;
    AtlasTile tile;
    uint grayscale;
    float opacity;
    Bounds bounds;
    ClipEnvelope clip;
    Corners corner_radii;
    PrimitiveTransform transform;
};

struct PolychromeSpriteFragmentInput {
    nointerpolation uint sprite_id: TEXCOORD0;
    float4 position: SV_Position;
    float2 tile_position: POSITION;
    float2 local_position: TEXCOORD1;
    nointerpolation float opacity: TEXCOORD3;
    nointerpolation uint2 clip_range: TEXCOORD9;
    nointerpolation float4 clip_bounds: TEXCOORD10;
};

StructuredBuffer<PolychromeSprite> poly_sprites: register(t1);

PolychromeSpriteFragmentInput polychrome_sprite_vertex(uint vertex_id: SV_VertexID, uint instance_id: SV_InstanceID) {
    uint sprite_id = batch_start_index + instance_id;
    float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
    PolychromeSprite sprite = poly_sprites[sprite_id];
    ClippedVertex vertex = clip_to_mask(unit_vertex, sprite.bounds,
                                       pixel_raster_mask(sprite.clip.conservative_bounds), sprite.transform);
    float2 tile_position = to_tile_position(vertex.unit_vertex, sprite.tile);

    PolychromeSpriteFragmentInput output;
    output.position = to_device_position_impl(vertex.position);
    output.tile_position = tile_position;
    bool simple = !has_rounded_corners(sprite.corner_radii) && sprite.grayscale == 0u;
    output.sprite_id = sprite_id | (simple ? SIMPLE_PRIMITIVE_FLAG : 0u);
    output.local_position = vertex.local_position;
    output.opacity = sprite.opacity;
    output.clip_range = uint2(sprite.clip.first_clip, sprite.clip.clip_count);
    output.clip_bounds = float4(sprite.clip.conservative_bounds.origin,
                                sprite.clip.conservative_bounds.size);
    return output;
}

float4 polychrome_sprite_fragment(PolychromeSpriteFragmentInput input): SV_Target {
    if (!clip_stack_contains(input.position.xy, input.clip_bounds, input.clip_range)) {
        discard;
    }
    float4 sample = t_sprite.Sample(s_sprite, input.tile_position);
    if ((input.sprite_id & SIMPLE_PRIMITIVE_FLAG) != 0u) {
        return float4(sample.rgb, sample.a * input.opacity);
    }
    PolychromeSprite sprite = poly_sprites[input.sprite_id];
    float distance = quad_sdf(input.local_position, sprite.bounds, sprite.corner_radii);

    float4 color = sample;
    if (sprite.grayscale != 0u) {
        float3 grayscale = dot(color.rgb, GRAYSCALE_FACTORS);
        color = float4(grayscale, sample.a);
    }
    color.a *= sprite.opacity * saturate(0.5 - distance);
    return color;
}
