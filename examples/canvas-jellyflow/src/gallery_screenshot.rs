use super::*;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GalleryScreenshotFile {
    pub fixture_id: String,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub non_transparent_pixels: usize,
    pub distinct_rgba_samples: usize,
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
        let window = open_gallery_case_window(&mut cx, &case)?;
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
        image.save(&path).map_err(|error| error.to_string())?;
        files.push(GalleryScreenshotFile {
            fixture_id: case.id().to_owned(),
            path,
            width: image.width(),
            height: image.height(),
            non_transparent_pixels: stats.non_transparent_pixels,
            distinct_rgba_samples: stats.distinct_rgba_samples,
        });
    }

    Ok(GalleryScreenshotExportReport::exported(output_dir, files))
}

fn open_gallery_case_window(
    cx: &mut HeadlessAppContext,
    case: &product_gallery::ProductGalleryCase,
) -> Result<AnyWindowHandle, String> {
    let (store, document, projection) =
        project_product_gallery_case(case).map_err(|error| error.to_string())?;
    let editor = editor_for_document(document).map_err(|error| error.to_string())?;
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
                measurement_revision: 1,
                measurement_frame_pending: false,
                auto_fit_viewport: true,
                last_canvas_view_size: None,
                last_canvas_bounds: None,
            })
        })
        .map_err(|error| error.to_string())?;
    Ok(window.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenshotStats {
    non_transparent_pixels: usize,
    distinct_rgba_samples: usize,
}

fn screenshot_stats(image: &image::RgbaImage) -> ScreenshotStats {
    let mut samples = BTreeSet::new();
    let mut non_transparent_pixels = 0;
    for pixel in image.pixels() {
        if pixel[3] > 0 {
            non_transparent_pixels += 1;
        }
        if samples.len() < 64 {
            samples.insert(pixel.0);
        }
    }
    ScreenshotStats {
        non_transparent_pixels,
        distinct_rgba_samples: samples.len(),
    }
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
    }
}
