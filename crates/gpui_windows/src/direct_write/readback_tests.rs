use super::{DirectWriteTextSystem, MappedColorReadback, readback_color_texture};
use crate::directx_devices::DirectXDevices;
use anyhow::{Context, Result};
use open_gpui::{
    DevicePixels, Font, FontRun, PlatformTextSystem, RenderGlyphParams, point, px, size,
};
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0},
        Direct3D11::*,
        Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
    },
};
use windows::core::Interface;

#[test]
fn color_readback_releases_staging_texture_for_repeated_reads() -> Result<()> {
    let mut device = None;
    let mut device_context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_DEBUG,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut device_context),
        )
    }
    .context("creating WARP color readback device")?;
    let device = device.context("WARP color readback device missing")?;
    let device_context = device_context.context("WARP color readback context missing")?;
    let info_queue: ID3D11InfoQueue = device.cast().context("querying WARP debug queue")?;
    let bitmap_size = size(DevicePixels(3), DevicePixels(3));
    let rows: [[u8; 12]; 3] = [
        [1, 2, 3, 255, 17, 23, 31, 128, 0, 0, 0, 0],
        [41, 43, 47, 255, 5, 7, 11, 64, 61, 67, 71, 192],
        [73, 79, 83, 255, 13, 19, 29, 32, 89, 97, 101, 224],
    ];
    let expected = rows.concat();
    let mut source_bytes = [0xa5u8; 60];
    for (row, source_row) in rows.iter().zip(source_bytes.chunks_exact_mut(20)) {
        source_row[..row.len()].copy_from_slice(row);
    }
    let initial_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: source_bytes.as_ptr().cast(),
        SysMemPitch: 20,
        SysMemSlicePitch: 0,
    };
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 5,
        Height: 3,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging_texture = None;
    unsafe { device.CreateTexture2D(&desc, Some(&initial_data), Some(&mut staging_texture)) }
        .context("creating initialized color readback staging texture")?;
    let staging_texture = staging_texture.context("color readback staging texture missing")?;

    for iteration in 0..3 {
        unsafe { info_queue.ClearStoredMessages() };
        let actual = readback_color_texture(&device_context, &staging_texture, bitmap_size)
            .with_context(|| format!("reading color bitmap on iteration {iteration}"))?;
        assert_eq!(actual, expected, "color bitmap on iteration {iteration}");

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { device_context.Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
            .with_context(|| {
                format!("remapping staging texture after color readback on iteration {iteration}")
            })?;
        let row_pitch = mapped.RowPitch as usize;
        unsafe { device_context.Unmap(&staging_texture, 0) };
        assert_eq!(
            unsafe { info_queue.GetNumStoredMessagesAllowedByRetrievalFilter() },
            0,
            "native remap must not report an already-mapped resource on iteration {iteration}",
        );
        assert!(
            row_pitch > rows[0].len(),
            "native row pitch must exceed the copied color row width, got {row_pitch}",
        );
    }

    for invalid_size in [
        size(DevicePixels(0), DevicePixels(3)),
        size(DevicePixels(3), DevicePixels(0)),
        size(DevicePixels(-1), DevicePixels(3)),
        size(DevicePixels(3), DevicePixels(-1)),
        size(DevicePixels(6), DevicePixels(3)),
        size(DevicePixels(3), DevicePixels(4)),
        size(DevicePixels(i32::MAX), DevicePixels(i32::MAX)),
    ] {
        assert!(
            readback_color_texture(&device_context, &staging_texture, invalid_size).is_err(),
            "invalid color readback dimensions {invalid_size:?} must be rejected",
        );
    }

    let error = (|| -> Result<()> {
        let _mapped = MappedColorReadback::new(&device_context, &staging_texture)?;
        anyhow::bail!("forced mapped color readback failure")
    })()
    .expect_err("the post-map error control must fail");
    assert_eq!(error.to_string(), "forced mapped color readback failure");
    assert_eq!(
        readback_color_texture(&device_context, &staging_texture, bitmap_size)?,
        expected,
    );
    assert_eq!(
        unsafe { info_queue.GetNumStoredMessagesAllowedByRetrievalFilter() },
        0,
        "a post-map error must release the staging texture",
    );

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let _mapped = MappedColorReadback::new(&device_context, &staging_texture)?;
        panic!("forced mapped color readback unwind")
    }))
    .expect_err("the mapped color readback control must unwind");
    assert_eq!(
        unwind.downcast_ref::<&str>(),
        Some(&"forced mapped color readback unwind"),
    );
    assert_eq!(
        readback_color_texture(&device_context, &staging_texture, bitmap_size)?,
        expected,
    );
    assert_eq!(
        unsafe { info_queue.GetNumStoredMessagesAllowedByRetrievalFilter() },
        0,
        "an unwind must release the staging texture",
    );

    Ok(())
}

#[test]
fn directwrite_color_emoji_rasterization_reads_back_identical_pixels() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(&devices)?;
    let font = Font {
        family: "Segoe UI Emoji".into(),
        ..Default::default()
    };
    let text = "\u{1f600}";
    let font_size = px(32.);
    let font_id = platform.font_id(&font)?;
    let shaped = platform.state.write().layout_line(
        &platform.components,
        text,
        font_size,
        &[FontRun {
            len: text.len(),
            font_id,
        }],
    )?;
    assert_eq!(shaped.runs.len(), 1, "the emoji must shape into one run");
    let run = &shaped.runs[0];
    assert_eq!(run.glyphs.len(), 1, "the emoji must shape into one glyph");
    let glyph = &run.glyphs[0];
    assert!(glyph.is_emoji, "the shaped glyph must use the color font");
    let params = RenderGlyphParams {
        font_id: run.font_id,
        glyph_id: glyph.id,
        font_size,
        subpixel_variant: point(0, 0),
        scale_factor: 1.,
        is_emoji: glyph.is_emoji,
        subpixel_rendering: false,
        dilation: 0,
    };
    let state = platform.state.read();
    let bounds = state.raster_bounds(&platform.components, &params)?;
    assert!(bounds.size.width.0 > 0 && bounds.size.height.0 > 0);
    let first = state
        .rasterize_color(&platform.components, &params, bounds)
        .context("reading back the first DirectWrite color emoji")?;
    assert_eq!(
        first.len(),
        usize::try_from(bounds.size.width.0)? * usize::try_from(bounds.size.height.0)? * 4,
    );
    assert!(
        first.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "the color emoji must contain visible BGRA pixels",
    );

    let padding = DevicePixels(4);
    let padded_bounds = open_gpui::bounds(
        bounds.origin - point(padding, padding),
        bounds.size + size(padding + padding, padding + padding),
    );
    let padded = state
        .rasterize_color(&platform.components, &params, padded_bounds)
        .context("reading back a DirectWrite color emoji with transparent padding")?;
    let padded_width = usize::try_from(padded_bounds.size.width.0)?;
    let padded_height = usize::try_from(padded_bounds.size.height.0)?;
    let padding = usize::try_from(padding.0)?;
    assert_eq!(padded.len(), padded_width * padded_height * 4);
    for (y, row) in padded.chunks_exact(padded_width * 4).enumerate() {
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            if x < padding
                || y < padding
                || x >= padded_width - padding
                || y >= padded_height - padding
            {
                assert_eq!(
                    pixel,
                    [0, 0, 0, 0],
                    "uncovered color glyph pixel at ({x}, {y}) must be transparent black",
                );
            }
        }
    }
    assert!(
        padded.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "transparent padding must preserve visible color glyph content",
    );

    for iteration in 0..3 {
        let repeated_bounds = state.raster_bounds(&platform.components, &params)?;
        let repeated = state
            .rasterize_color(&platform.components, &params, repeated_bounds)
            .with_context(|| {
                format!("reading back DirectWrite color emoji on iteration {iteration}")
            })?;
        assert_eq!(repeated_bounds.size, bounds.size);
        assert_eq!(
            repeated, first,
            "DirectWrite color emoji on iteration {iteration}"
        );
    }

    Ok(())
}
