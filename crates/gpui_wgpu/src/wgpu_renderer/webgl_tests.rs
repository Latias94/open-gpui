use super::*;

fn validate_wgsl(source: &str, capabilities: naga::valid::Capabilities) {
    let module = naga::front::wgsl::parse_str(source).expect("shader must parse");
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
        .validate(&module)
        .expect("shader must validate");
}

#[test]
fn webgl_shader_is_valid_wgsl_without_storage_buffers() {
    assert!(!WEBGL_SHADERS.contains("var<storage"));
    validate_wgsl(WEBGL_SHADERS, naga::valid::Capabilities::empty());
}

#[test]
fn webgl_uniform_bindings_preserve_downlevel_sixteen_byte_sizes() {
    let module = naga::front::wgsl::parse_str(WEBGL_SHADERS).expect("shader must parse");
    let mut surface_params = None;
    for (_, global) in module.global_variables.iter() {
        if global.space != naga::AddressSpace::Uniform {
            continue;
        }
        let naga::TypeInner::Struct { span, ref members } = module.types[global.ty].inner else {
            panic!("the renderer's uniform bindings must be complete parameter structs");
        };
        assert_eq!(
            span % 16,
            0,
            "WebGL uniform binding {:?} must occupy complete 16-byte blocks",
            global.name
        );
        if module.types[global.ty].name.as_deref() == Some("SurfaceParams") {
            surface_params = Some(span);
            let offsets: Vec<_> = members
                .iter()
                .map(|member| (member.name.as_deref(), member.offset))
                .collect();
            assert_eq!(offsets, vec![(Some("bounds"), 0), (Some("clip"), 16)]);
        }
    }
    assert_eq!(
        surface_params,
        Some(std::mem::size_of::<SurfaceParams>() as u32)
    );
}

#[test]
fn storage_buffer_shader_is_valid_wgsl() {
    validate_wgsl(STORAGE_BUFFER_SHADERS, naga::valid::Capabilities::empty());
}

#[test]
fn subpixel_shader_is_valid_wgsl() {
    validate_wgsl(
        SUBPIXEL_SHADERS,
        naga::valid::Capabilities::DUAL_SOURCE_BLENDING,
    );
}

#[test]
fn webgl_record_sizes_match_shader_word_strides() {
    use open_gpui::{MonochromeSprite, PolychromeSprite, Quad, Shadow, SubpixelSprite, Underline};
    assert_eq!(std::mem::size_of::<Quad>(), 46 * 4);
    assert_eq!(std::mem::size_of::<Shadow>(), 34 * 4);
    assert_eq!(std::mem::size_of::<PathPaint>(), 34 * 4);
    assert_eq!(std::mem::size_of::<PathCurve>(), 10 * 4);
    assert_eq!(std::mem::size_of::<PathTile>(), 14 * 4);
    assert_eq!(std::mem::size_of::<Underline>(), 22 * 4);
    assert_eq!(std::mem::size_of::<MonochromeSprite>(), 30 * 4);
    assert_eq!(std::mem::size_of::<SubpixelSprite>(), 30 * 4);
    assert_eq!(std::mem::size_of::<PolychromeSprite>(), 32 * 4);
    assert_eq!(std::mem::size_of::<GpuClipShape>(), 3 * 16);
    assert_eq!(std::mem::size_of::<PrimitiveTransform>(), 16);
    assert_eq!(std::mem::size_of::<open_gpui::AtlasTile>(), 40);
}

#[test]
fn webgl_quad_layout_matches_fixed_decoder() {
    use open_gpui::{
        BorderStyle, ColorSpace, Corners, Edges, Hsla, Quad, linear_color_stop, linear_gradient,
        point, size,
    };
    let mut quad = Quad::default();
    quad.order = 41;
    quad.border_style = BorderStyle::Dashed;
    quad.bounds = Bounds::new(point(1.0.into(), 2.0.into()), size(3.0.into(), 4.0.into()));
    quad.clip = ClipEnvelope {
        conservative_bounds: Bounds::new(
            point(5.0.into(), 6.0.into()),
            size(7.0.into(), 8.0.into()),
        ),
        first_clip: 0x1234_5678,
        clip_count: 0x0123_4567,
    };
    quad.background = linear_gradient(
        11.0,
        linear_color_stop(
            Hsla {
                h: 12.0,
                s: 13.0,
                l: 14.0,
                a: 15.0,
            },
            16.0,
        ),
        linear_color_stop(
            Hsla {
                h: 17.0,
                s: 18.0,
                l: 19.0,
                a: 20.0,
            },
            21.0,
        ),
    )
    .color_space(ColorSpace::Oklab);
    quad.border_color = Hsla {
        h: 22.0,
        s: 23.0,
        l: 24.0,
        a: 25.0,
    };
    quad.corner_radii = Corners {
        top_left: 26.0.into(),
        top_right: 27.0.into(),
        bottom_right: 28.0.into(),
        bottom_left: 29.0.into(),
    };
    quad.border_widths = Edges {
        top: 30.0.into(),
        right: 31.0.into(),
        bottom: 32.0.into(),
        left: 33.0.into(),
    };
    let bytes = unsafe { WgpuRendererCore::instance_bytes(std::slice::from_ref(&quad)) };
    let words: &[u32] = bytemuck::cast_slice(bytes);
    let mut expected = vec![41, 1];
    expected.extend((1..=8).map(|value| (value as f32).to_bits()));
    expected.extend([0x1234_5678, 0x0123_4567, 1, 1, 0, 0, 0, 0]);
    expected.extend((11..=21).map(|value| (value as f32).to_bits()));
    expected.push(0);
    expected.extend((22..=33).map(|value| (value as f32).to_bits()));
    expected.extend([1.0_f32.to_bits(), 1.0_f32.to_bits(), 0, 0]);
    assert_eq!(words, expected);
}

#[test]
fn packed_allocations_keep_whole_records_and_partial_texels_disjoint() -> anyhow::Result<()> {
    let mut offset = 16;
    for stride in [184, 136, 40, 56, 88, 120, 128] {
        for count in [0, 1, 2, 3, 19] {
            let (start, allocation, first) =
                instance_allocation(offset, stride * count, stride, 16, true)?;
            assert_eq!(start % stride, 0);
            assert_eq!(start % 16, 0);
            assert_eq!(allocation % 16, 0);
            assert_eq!(u64::from(first) * stride, start);
            assert!(start >= offset);
            assert!(allocation >= (stride * count).max(16));
            offset = start + allocation;
        }
    }
    assert!(instance_allocation(u64::MAX, 184, 184, 16, true).is_err());
    assert!(instance_allocation(0, u64::MAX, 184, 16, true).is_err());
    Ok(())
}

#[test]
fn native_storage_allocation_retains_binding_relative_indices() -> anyhow::Result<()> {
    assert_eq!(
        instance_allocation(17, 184, 184, 256, false)?,
        (256, 184, 0)
    );
    Ok(())
}
