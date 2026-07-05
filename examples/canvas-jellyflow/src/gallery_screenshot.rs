use super::*;
use jellyflow_open_gpui::testing::{
    OpenGpuiScreenshotFixtureEvidence, OpenGpuiScreenshotRegionEvidence,
    OpenGpuiScreenshotRegionKind, OpenGpuiScreenshotRegionRect, OpenGpuiScreenshotRegionReport,
    assert_screenshot_region_report_gates,
};
use open_gpui::{AnyWindowHandle, HeadlessAppContext};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GalleryScreenshotExportReport {
    pub output_dir: PathBuf,
    pub files: Vec<GalleryScreenshotFile>,
    pub skipped_reason: Option<String>,
}

impl GalleryScreenshotExportReport {
    fn skipped(output_dir: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            output_dir,
            files: Vec::new(),
            skipped_reason: Some(reason.into()),
        }
    }

    fn exported(output_dir: PathBuf, files: Vec<GalleryScreenshotFile>) -> Self {
        Self {
            output_dir,
            files,
            skipped_reason: None,
        }
    }

    fn region_report(&self) -> OpenGpuiScreenshotRegionReport {
        if let Some(reason) = &self.skipped_reason {
            return OpenGpuiScreenshotRegionReport::skipped(reason.clone());
        }

        let mut report = OpenGpuiScreenshotRegionReport::default();
        for file in &self.files {
            report.push_fixture(file.fixture_evidence.clone());
        }
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GalleryScreenshotFile {
    pub fixture_id: String,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub non_transparent_pixels: usize,
    pub distinct_rgba_samples: usize,
    pub fixture_evidence: OpenGpuiScreenshotFixtureEvidence,
}

pub(super) fn default_gallery_screenshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .join("target")
        .join("open-gpui-jellyflow-gallery")
}

pub(super) fn export_gallery_screenshot_smoke(
    output_dir: impl AsRef<Path>,
    require_renderer: bool,
) -> Result<GalleryScreenshotExportReport, String> {
    let output_dir = output_dir.as_ref().to_path_buf();
    if open_gpui_platform::current_headless_renderer().is_none() {
        if require_renderer {
            return Err("Open GPUI headless renderer is unavailable".to_owned());
        }
        return Ok(GalleryScreenshotExportReport::skipped(
            output_dir,
            "Open GPUI headless renderer is unavailable",
        ));
    }

    std::fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let platform = open_gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        open_gpui_platform::current_headless_renderer,
    );
    cx.update(init_canvas_jellyflow_app);
    let mut files = Vec::new();

    for case in product_gallery::product_gallery_cases() {
        let (window, expected_regions) = open_gallery_case_window(&mut cx, &case)?;
        cx.run_until_parked();
        let image = match cx.capture_screenshot(window) {
            Ok(image) => image,
            Err(error) if !require_renderer => {
                return Ok(GalleryScreenshotExportReport::skipped(
                    output_dir,
                    format!("Open GPUI screenshot capture failed: {error}"),
                ));
            }
            Err(error) => return Err(error.to_string()),
        };
        let stats = screenshot_stats(&image);
        if stats.non_transparent_pixels == 0 || stats.distinct_rgba_samples < 2 {
            if require_renderer {
                return Err(format!(
                    "Open GPUI screenshot for `{}` is blank or single-color: {stats:?}",
                    case.id()
                ));
            }
            return Ok(GalleryScreenshotExportReport::skipped(
                output_dir,
                format!(
                    "Open GPUI screenshot for `{}` is blank or single-color: {stats:?}",
                    case.id()
                ),
            ));
        }

        let path = output_dir.join(format!("{}.png", screenshot_file_stem(case.id())));
        let mut fixture_evidence = OpenGpuiScreenshotFixtureEvidence::captured(
            case.id(),
            image.width(),
            image.height(),
            stats.non_transparent_pixels,
            stats.distinct_rgba_samples,
        );
        for region in screenshot_region_evidence(&image, expected_regions) {
            fixture_evidence.push_region(region);
        }
        image.save(&path).map_err(|error| error.to_string())?;
        files.push(GalleryScreenshotFile {
            fixture_id: case.id().to_owned(),
            path,
            width: image.width(),
            height: image.height(),
            non_transparent_pixels: stats.non_transparent_pixels,
            distinct_rgba_samples: stats.distinct_rgba_samples,
            fixture_evidence,
        });
    }

    Ok(GalleryScreenshotExportReport::exported(output_dir, files))
}

fn open_gallery_case_window(
    cx: &mut HeadlessAppContext,
    case: &product_gallery::ProductGalleryCase,
) -> Result<(AnyWindowHandle, Vec<ExpectedScreenshotRegion>), String> {
    let (store, document, projection) =
        project_product_gallery_case(case).map_err(|error| error.to_string())?;
    let editor = editor_for_document(document).map_err(|error| error.to_string())?;
    let expected_regions = expected_screenshot_regions(editor.document(), &editor.viewport());
    let mut gallery = product_gallery::ProductGalleryState::default();
    gallery.set_active(case.id().to_owned());
    let node_kit_registry = NodeKitRegistry::builtin();
    let semantic_registry = node_kit_registry.node_registry();
    let window = cx
        .open_window(size(px(CANVAS_WIDTH), px(CANVAS_HEIGHT)), move |_, cx| {
            cx.new(|cx| JellyflowCanvasView {
                editor,
                store,
                focus_handle: cx.focus_handle(),
                projection,
                gallery,
                adapter: OpenGpuiAdapter::default(),
                semantic_registry,
                node_kit_registry,
                measured_regions: OpenGpuiBoundsCollector::new(),
                measurement_coverage: BTreeMap::new(),
                measurement_revision: 1,
                measurement_refresh_requested: false,
                measurement_frame_pending: false,
                measurement_frame_generation: 0,
                auto_fit_viewport: true,
                deferred_editor_refresh: false,
                last_canvas_view_size: None,
                last_canvas_bounds: None,
                last_canvas_scene: None,
            })
        })
        .map_err(|error| error.to_string())?;
    Ok((window.into(), expected_regions))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenshotStats {
    non_transparent_pixels: usize,
    distinct_rgba_samples: usize,
}

fn screenshot_stats(image: &image::RgbaImage) -> ScreenshotStats {
    screenshot_region_stats(
        image,
        OpenGpuiScreenshotRegionRect {
            x: 0,
            y: 0,
            width: image.width(),
            height: image.height(),
        },
    )
}

fn screenshot_region_stats(
    image: &image::RgbaImage,
    rect: OpenGpuiScreenshotRegionRect,
) -> ScreenshotStats {
    let mut samples = BTreeSet::new();
    let mut non_transparent_pixels = 0;
    let max_x = rect.x.saturating_add(rect.width).min(image.width());
    let max_y = rect.y.saturating_add(rect.height).min(image.height());
    for pixel_y in rect.y.min(image.height())..max_y {
        for pixel_x in rect.x.min(image.width())..max_x {
            let pixel = image.get_pixel(pixel_x, pixel_y);
            if pixel[3] > 0 {
                non_transparent_pixels += 1;
            }
            if samples.len() < 64 {
                samples.insert(pixel.0);
            }
        }
    }
    ScreenshotStats {
        non_transparent_pixels,
        distinct_rgba_samples: samples.len(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ExpectedScreenshotRegion {
    kind: OpenGpuiScreenshotRegionKind,
    bounds: Bounds<Pixels>,
}

fn expected_screenshot_regions(
    document: &CanvasDocument,
    viewport: &CanvasViewport,
) -> Vec<ExpectedScreenshotRegion> {
    let mut regions = Vec::new();
    if let Some(node_bounds) = largest_node_view_bounds(document, viewport) {
        regions.push(ExpectedScreenshotRegion {
            kind: OpenGpuiScreenshotRegionKind::NodeBody,
            bounds: screenshot_bounds_from_canvas_view(node_bounds),
        });
        regions.push(ExpectedScreenshotRegion {
            kind: OpenGpuiScreenshotRegionKind::NodeInternalUi,
            bounds: screenshot_bounds_from_canvas_view(inset_view_bounds(
                node_bounds,
                px(24.0),
                px(34.0),
            )),
        });
    }
    if let Some(wire_bounds) = first_wire_view_bounds(document, viewport) {
        regions.push(ExpectedScreenshotRegion {
            kind: OpenGpuiScreenshotRegionKind::WirePath,
            bounds: screenshot_bounds_from_canvas_view(wire_bounds),
        });
    }
    if let Some(port_bounds) = first_port_view_bounds(document, viewport) {
        regions.push(ExpectedScreenshotRegion {
            kind: OpenGpuiScreenshotRegionKind::PortArea,
            bounds: screenshot_bounds_from_canvas_view(port_bounds),
        });
    }
    regions
}

fn largest_node_view_bounds(
    document: &CanvasDocument,
    viewport: &CanvasViewport,
) -> Option<Bounds<Pixels>> {
    document
        .nodes()
        .filter(|node| !node.hidden)
        .map(|node| viewport.document_bounds_to_view(node.bounds()))
        .max_by(|left, right| {
            let left_area = left.size.width.as_f32() * left.size.height.as_f32();
            let right_area = right.size.width.as_f32() * right.size.height.as_f32();
            left_area
                .partial_cmp(&right_area)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn first_wire_view_bounds(
    document: &CanvasDocument,
    viewport: &CanvasViewport,
) -> Option<Bounds<Pixels>> {
    document.edges().find_map(|edge| {
        let source = endpoint_view_position(document, viewport, &edge.source)?;
        let target = endpoint_view_position(document, viewport, &edge.target)?;
        let left = source.x.min(target.x);
        let top = source.y.min(target.y);
        let right = source.x.max(target.x);
        let bottom = source.y.max(target.y);
        Some(expand_view_bounds(
            Bounds::new(
                point(left, top),
                size((right - left).max(px(1.0)), (bottom - top).max(px(1.0))),
            ),
            px(28.0),
        ))
    })
}

fn first_port_view_bounds(
    document: &CanvasDocument,
    viewport: &CanvasViewport,
) -> Option<Bounds<Pixels>> {
    document.nodes().find_map(|node| {
        node.handles
            .iter()
            .find(|handle| !handle.hidden)
            .map(|handle| {
                expand_view_bounds(
                    viewport.document_bounds_to_view(handle.bounds_in_document(node)),
                    px(18.0),
                )
            })
    })
}

fn endpoint_view_position(
    document: &CanvasDocument,
    viewport: &CanvasViewport,
    endpoint: &open_gpui_canvas::CanvasEndpoint,
) -> Option<open_gpui::Point<Pixels>> {
    let node = document.node(&endpoint.node_id)?;
    let local = node
        .handle(endpoint.handle_id.as_ref())
        .map(|handle| handle.position)
        .unwrap_or_else(|| point(node.size.width * 0.5, node.size.height * 0.5));
    Some(viewport.document_to_view(node.position + local))
}

fn screenshot_bounds_from_canvas_view(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(bounds.origin.x, bounds.origin.y + px(TOOLBAR_HEIGHT)),
        bounds.size,
    )
}

fn inset_view_bounds(bounds: Bounds<Pixels>, x: Pixels, y: Pixels) -> Bounds<Pixels> {
    let width = (bounds.size.width - x * 2.0).max(px(8.0));
    let height = (bounds.size.height - y * 2.0).max(px(8.0));
    Bounds::new(bounds.origin + point(x, y), size(width, height))
}

fn expand_view_bounds(bounds: Bounds<Pixels>, amount: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        bounds.origin - point(amount, amount),
        size(
            bounds.size.width + amount * 2.0,
            bounds.size.height + amount * 2.0,
        ),
    )
}

fn screenshot_region_evidence(
    image: &image::RgbaImage,
    regions: Vec<ExpectedScreenshotRegion>,
) -> Vec<OpenGpuiScreenshotRegionEvidence> {
    regions
        .into_iter()
        .filter_map(|region| {
            let rect = screenshot_rect_from_bounds(region.bounds, image.width(), image.height())?;
            let stats = screenshot_region_stats(image, rect);
            Some(OpenGpuiScreenshotRegionEvidence::new(
                region.kind,
                rect,
                stats.non_transparent_pixels,
                stats.distinct_rgba_samples,
            ))
        })
        .collect()
}

fn screenshot_rect_from_bounds(
    bounds: Bounds<Pixels>,
    image_width: u32,
    image_height: u32,
) -> Option<OpenGpuiScreenshotRegionRect> {
    let scale_x = image_width as f32 / CANVAS_WIDTH.max(1.0);
    let scale_y = image_height as f32 / CANVAS_HEIGHT.max(1.0);
    let left = scaled_floor(bounds.origin.x, scale_x, image_width);
    let top = scaled_floor(bounds.origin.y, scale_y, image_height);
    let right = scaled_ceil(bounds.origin.x + bounds.size.width, scale_x, image_width);
    let bottom = scaled_ceil(bounds.origin.y + bounds.size.height, scale_y, image_height);
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    (width > 0 && height > 0).then_some(OpenGpuiScreenshotRegionRect {
        x: left,
        y: top,
        width,
        height,
    })
}

fn scaled_floor(value: Pixels, scale: f32, max: u32) -> u32 {
    (value.as_f32() * scale).floor().clamp(0.0, max as f32) as u32
}

fn scaled_ceil(value: Pixels, scale: f32, max: u32) -> u32 {
    (value.as_f32() * scale).ceil().clamp(0.0, max as f32) as u32
}

fn screenshot_file_stem(fixture_id: &str) -> String {
    fixture_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[test]
fn screenshot_region_evidence_rejects_single_color_roi() {
    let image = image::RgbaImage::from_pixel(96, 72, image::Rgba([16, 16, 16, 255]));
    let stats = screenshot_stats(&image);
    let mut fixture = OpenGpuiScreenshotFixtureEvidence::captured(
        product_gallery::product_gallery_cases()
            .first()
            .expect("fixture")
            .id(),
        image.width(),
        image.height(),
        stats.non_transparent_pixels,
        stats.distinct_rgba_samples,
    );
    for region in screenshot_region_evidence(&image, single_color_test_regions()) {
        fixture.push_region(region);
    }
    let mut report = OpenGpuiScreenshotRegionReport::default();
    report.push_fixture(fixture);

    let result = std::panic::catch_unwind(|| assert_screenshot_region_report_gates(&report));
    assert!(
        result.is_err(),
        "single-color screenshots must not satisfy ROI evidence gates"
    );
}

fn single_color_test_regions() -> Vec<ExpectedScreenshotRegion> {
    [
        OpenGpuiScreenshotRegionKind::NodeBody,
        OpenGpuiScreenshotRegionKind::NodeInternalUi,
        OpenGpuiScreenshotRegionKind::WirePath,
        OpenGpuiScreenshotRegionKind::PortArea,
    ]
    .into_iter()
    .map(|kind| ExpectedScreenshotRegion {
        kind,
        bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(96.0), px(72.0))),
    })
    .collect()
}

#[test]
fn product_gallery_screenshot_exporter_writes_nonblank_pngs_or_skips() {
    let report = export_gallery_screenshot_smoke(default_gallery_screenshot_dir(), false)
        .expect("screenshot exporter should report artifacts or a skip reason");
    if let Some(reason) = &report.skipped_reason {
        assert!(
            report.files.is_empty(),
            "skipped screenshot export must not report files: {report:?}"
        );
        assert!(
            !reason.is_empty(),
            "skipped screenshot export must include a reason"
        );
        return;
    }

    let expected = product_gallery::product_gallery_cases().len();
    assert_eq!(
        report.files.len(),
        expected,
        "screenshot exporter should write one PNG per product fixture: {report:?}"
    );
    for file in &report.files {
        assert!(file.path.exists(), "screenshot file missing: {file:?}");
        assert!(
            file.width > 0 && file.height > 0,
            "invalid dimensions: {file:?}"
        );
        assert!(
            file.non_transparent_pixels > 0 && file.distinct_rgba_samples >= 2,
            "screenshot must be nonblank: {file:?}"
        );
        assert!(
            file.fixture_evidence
                .regions
                .iter()
                .all(OpenGpuiScreenshotRegionEvidence::is_present),
            "screenshot regions must be present: {file:?}"
        );
    }
    assert_screenshot_region_report_gates(&report.region_report());
}
