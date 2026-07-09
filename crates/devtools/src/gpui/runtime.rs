use crate::{
    CaptureProvider, DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsEventKind, DevtoolsEventRecord, DevtoolsTargetId, DevtoolsTargetKind,
    DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, ProbeSnapshotError, SnapshotDiagnostic,
    SnapshotEnvelope, SnapshotKind, SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{sanitize_sensitive_text, snapshot_node_with_payload},
    layout::{
        LayoutBoundsSnapshot, LayoutNodeSnapshot, LayoutPointSnapshot, LayoutSizeSnapshot,
        LayoutSnapshot,
    },
};
use open_gpui::{
    Bounds, Pixels, Point, ScrollViewportChangeSource, ScrollViewportProgrammaticSource,
    ScrollViewportSnapshot, Size as GpuiSize,
};
use serde::{Deserialize, Serialize};

const GPUI_RUNTIME_PROBE_ID: &str = "gpui.runtime";

/// Application-supplied metadata snapshot for GPUI runtime inspection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeSnapshot {
    /// Stable application/runtime id; this is sanitized before export.
    pub runtime_id: String,
    /// Monotonic generation assigned by the application or test harness.
    pub generation: u64,
    /// Public window metadata rows.
    pub windows: Vec<GpuiRuntimeWindowSnapshot>,
    /// Optional focus metadata with no element labels.
    pub focus: Option<GpuiRuntimeFocusSnapshot>,
    /// Optional input metadata with counters only.
    pub input: Option<GpuiRuntimeInputSnapshot>,
    /// Optional frame metadata with counters and timing only.
    pub frame: Option<GpuiRuntimeFrameSnapshot>,
    /// Scroll/layout facts converted from committed public scroll snapshots.
    pub scroll_viewports: Vec<GpuiRuntimeScrollSnapshot>,
    /// Explicit diagnostics supplied by the producer.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

/// Metadata for one GPUI window without raw titles or text content.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeWindowSnapshot {
    /// Stable GPUI window id.
    pub window_id: u64,
    /// Public display id, when the producer has one.
    pub display_id: Option<String>,
    /// Whether this window is the currently active application window.
    pub active: bool,
    /// Whether this window owns keyboard focus.
    pub focused: bool,
    /// Window bounds in public pixel coordinates, when available.
    pub bounds: Option<GpuiRuntimeRectSnapshot>,
    /// Content size in public pixel coordinates, when available.
    pub content_size: Option<GpuiRuntimeSizeSnapshot>,
    /// Window scale factor, when available.
    pub scale_factor: Option<f32>,
}

/// Metadata for focus state without element names, labels, or text values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeFocusSnapshot {
    /// Active window id, when known.
    pub active_window_id: Option<u64>,
    /// Focused window id, when known.
    pub focused_window_id: Option<u64>,
    /// Number of focus scopes known to the producer.
    pub focus_scope_count: usize,
    /// Number of focus handles known to the producer.
    pub focus_handle_count: usize,
}

/// Metadata-only input counters; raw key text and clipboard contents are intentionally absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuiRuntimeInputSnapshot {
    /// Count of key-down events observed by the producer.
    pub key_down_count: u64,
    /// Count of pointer events observed by the producer.
    pub pointer_event_count: u64,
    /// Count of scroll-wheel or touch-scroll events observed by the producer.
    pub scroll_event_count: u64,
    /// Count of text-input events observed by the producer; text values are not exported.
    pub text_input_event_count: u64,
    /// Count of IME composition events observed by the producer; composition text is not exported.
    pub ime_event_count: u64,
    /// Count of clipboard interactions observed by the producer; clipboard payloads are not exported.
    pub clipboard_event_count: u64,
    /// Stable kind label for the last input event, when known.
    pub last_event_kind: Option<String>,
}

/// Metadata-only frame counters and timing facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeFrameSnapshot {
    /// Number of frames requested by the producer.
    pub requested_frames: u64,
    /// Number of frames painted by the producer.
    pub painted_frames: u64,
    /// Number of animation frames observed by the producer.
    pub animation_frame_count: u64,
    /// Duration of the latest frame in milliseconds, when known.
    pub last_frame_duration_ms: Option<f32>,
    /// Latest presented generation, when known.
    pub last_presented_generation: Option<u64>,
}

/// Public scroll/layout metadata row for one committed GPUI scroll viewport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeScrollSnapshot {
    /// Scroll viewport generation.
    pub generation: u64,
    /// Stable scroll change source label.
    pub source: String,
    /// Viewport bounds.
    pub bounds: GpuiRuntimeRectSnapshot,
    /// Current scroll offset.
    pub offset: GpuiRuntimePointSnapshot,
    /// Maximum scroll offset.
    pub max_offset: GpuiRuntimePointSnapshot,
    /// Scroll content size.
    pub content_size: GpuiRuntimeSizeSnapshot,
}

/// Point metadata in public pixel coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimePointSnapshot {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

/// Size metadata in public pixel coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeSizeSnapshot {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Rectangle metadata in public pixel coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuiRuntimeRectSnapshot {
    /// Rectangle origin.
    pub origin: GpuiRuntimePointSnapshot,
    /// Rectangle size.
    pub size: GpuiRuntimeSizeSnapshot,
}

impl GpuiRuntimeScrollSnapshot {
    /// Converts a committed public GPUI scroll viewport snapshot into runtime metadata.
    pub fn from_scroll_viewport(snapshot: ScrollViewportSnapshot) -> Self {
        Self {
            generation: snapshot.generation(),
            source: scroll_viewport_source_label(snapshot.source()).to_owned(),
            bounds: runtime_rect_snapshot(snapshot.bounds()),
            offset: runtime_point_snapshot(snapshot.offset()),
            max_offset: runtime_point_snapshot(snapshot.max_offset()),
            content_size: runtime_size_snapshot(snapshot.content_size()),
        }
    }
}

/// Converts GPUI runtime metadata into a target/domain/event capture.
pub fn gpui_runtime_capture(snapshot: &GpuiRuntimeSnapshot) -> DevtoolsCapture {
    let runtime_target_id = gpui_runtime_target_id(&snapshot.runtime_id);
    let domain_id = gpui_runtime_domain_id(&snapshot.runtime_id);
    let envelope = gpui_runtime_snapshot_envelope(snapshot);
    let summary = gpui_runtime_summary_payload(snapshot);

    let mut targets = vec![
        DevtoolsTargetSnapshot::new(
            runtime_target_id.clone(),
            DevtoolsTargetKind::Runtime,
            "GPUI runtime",
        )
        .with_metadata(summary.clone()),
    ];
    targets.extend(snapshot.windows.iter().enumerate().map(|(index, window)| {
        DevtoolsTargetSnapshot::new(
            gpui_window_target_id(index, window),
            DevtoolsTargetKind::Window,
            format!("GPUI window {}", window.window_id),
        )
        .parent_id(runtime_target_id.clone())
        .with_metadata(gpui_window_payload(window))
    }));
    targets.extend(
        snapshot
            .scroll_viewports
            .iter()
            .enumerate()
            .map(|(index, scroll)| {
                DevtoolsTargetSnapshot::new(
                    gpui_scroll_target_id(index, scroll),
                    DevtoolsTargetKind::Viewport,
                    format!("Scroll viewport {index}"),
                )
                .parent_id(runtime_target_id.clone())
                .with_metadata(gpui_scroll_payload(scroll))
            }),
    );

    let mut events = Vec::new();
    if let Some(input) = &snapshot.input {
        events.push(
            DevtoolsEventRecord::new(
                "gpui.input-metadata",
                "Input metadata",
                DevtoolsEventKind::Instant,
            )
            .target_id(runtime_target_id.clone())
            .domain_id(domain_id.clone())
            .with_payload(gpui_input_payload(input)),
        );
    }
    if let Some(frame) = &snapshot.frame {
        events.push(
            DevtoolsEventRecord::new(
                "gpui.frame-metadata",
                "Frame metadata",
                DevtoolsEventKind::Instant,
            )
            .target_id(runtime_target_id.clone())
            .domain_id(domain_id.clone())
            .with_payload(gpui_frame_payload(frame)),
        );
    }

    let mut domain = DevtoolsDomainSnapshot::new(
        domain_id,
        runtime_target_id,
        DevtoolsDomainKind::Custom("gpui-runtime".to_owned()),
        "GPUI runtime",
    )
    .with_summary(summary)
    .with_snapshot(envelope.clone());
    for diagnostic in snapshot.diagnostics.iter().cloned() {
        domain = domain.with_diagnostic(diagnostic);
    }

    DevtoolsCapture::new(
        DevtoolsTargetTree::new(targets),
        [domain],
        events,
        [envelope],
        snapshot.diagnostics.clone(),
    )
}

/// Creates a capture provider for application-supplied GPUI runtime metadata.
pub fn gpui_runtime_capture_provider<F>(
    id: impl Into<String>,
    snapshot: F,
) -> Result<
    CaptureProvider<impl Fn() -> Result<DevtoolsCapture, ProbeSnapshotError>>,
    ProbeSnapshotError,
>
where
    F: Fn() -> GpuiRuntimeSnapshot + Send + Sync + 'static,
{
    CaptureProvider::new(id, move || Ok(gpui_runtime_capture(&snapshot())))
}

/// Converts GPUI runtime metadata into a legacy DevTools probe tree.
pub fn gpui_runtime_probe_snapshot(snapshot: &GpuiRuntimeSnapshot) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(gpui_runtime_tree(snapshot))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts a committed GPUI scroll viewport snapshot into a DevTools tree.
pub fn scroll_viewport_probe_snapshot(snapshot: ScrollViewportSnapshot) -> SnapshotProbeSnapshot {
    let root = snapshot_node_with_payload(
        ["scroll", "viewport"],
        "Scroll viewport",
        serde_json::json!({
            "generation": snapshot.generation(),
            "source": scroll_viewport_source_label(snapshot.source()),
            "bounds": bounds_payload(snapshot.bounds()),
            "offset": point_payload(snapshot.offset()),
            "max_offset": point_payload(snapshot.max_offset()),
            "content_size": size_payload(snapshot.content_size()),
        }),
    );

    SnapshotProbeSnapshot::new(SnapshotTree::new([root]))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts a committed GPUI scroll viewport snapshot into a DevTools layout snapshot.
pub fn scroll_viewport_layout_snapshot(snapshot: ScrollViewportSnapshot) -> LayoutSnapshot {
    let node = LayoutNodeSnapshot::new("scroll.viewport", "Scroll viewport")
        .bounds(layout_bounds_snapshot(snapshot.bounds()))
        .scroll_offset(layout_point_snapshot(snapshot.offset()))
        .max_scroll_offset(layout_point_snapshot(snapshot.max_offset()))
        .content_size(layout_size_snapshot(snapshot.content_size()))
        .with_payload(serde_json::json!({
            "generation": snapshot.generation(),
            "source": scroll_viewport_source_label(snapshot.source()),
        }));

    LayoutSnapshot::new("scroll-viewport", "Scroll viewport layout", [node])
}

/// Converts a committed GPUI scroll viewport snapshot into a DevTools layout probe snapshot.
pub fn scroll_viewport_layout_probe_snapshot(
    snapshot: ScrollViewportSnapshot,
) -> SnapshotProbeSnapshot {
    scroll_viewport_layout_snapshot(snapshot).probe_snapshot()
}

/// Creates a sanitized diagnostic for an unavailable scroll viewport snapshot.
pub fn scroll_viewport_unavailable_diagnostic(probe_id: ProbeId) -> SnapshotDiagnostic {
    SnapshotDiagnostic::new(
        probe_id,
        "runtime.unavailable",
        "scroll viewport snapshot is not committed",
    )
}

fn gpui_runtime_snapshot_envelope(snapshot: &GpuiRuntimeSnapshot) -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        gpui_runtime_probe_id(&snapshot.runtime_id),
        SnapshotKind::Custom("gpui-runtime".to_owned()),
        gpui_runtime_tree(snapshot),
    )
    .with_redaction(SnapshotRedactionSummary::default())
}

fn gpui_runtime_tree(snapshot: &GpuiRuntimeSnapshot) -> SnapshotTree {
    let mut root = snapshot_node_with_payload(
        ["gpui", "runtime"],
        "GPUI runtime",
        gpui_runtime_summary_payload(snapshot),
    );

    for (index, window) in snapshot.windows.iter().enumerate() {
        let index_label = index.to_string();
        root = root.with_child(snapshot_node_with_payload(
            ["gpui", "runtime", "window", index_label.as_str()],
            format!("GPUI window {}", window.window_id),
            gpui_window_payload(window),
        ));
    }

    if let Some(focus) = &snapshot.focus {
        root = root.with_child(snapshot_node_with_payload(
            ["gpui", "runtime", "focus"],
            "Focus metadata",
            gpui_focus_payload(focus),
        ));
    }

    if let Some(input) = &snapshot.input {
        root = root.with_child(snapshot_node_with_payload(
            ["gpui", "runtime", "input"],
            "Input metadata",
            gpui_input_payload(input),
        ));
    }

    if let Some(frame) = &snapshot.frame {
        root = root.with_child(snapshot_node_with_payload(
            ["gpui", "runtime", "frame"],
            "Frame metadata",
            gpui_frame_payload(frame),
        ));
    }

    for (index, scroll) in snapshot.scroll_viewports.iter().enumerate() {
        let index_label = index.to_string();
        root = root.with_child(snapshot_node_with_payload(
            ["gpui", "runtime", "scroll", index_label.as_str()],
            format!("Scroll viewport {index}"),
            gpui_scroll_payload(scroll),
        ));
    }

    SnapshotTree::new([root])
}

fn gpui_runtime_target_id(runtime_id: &str) -> DevtoolsTargetId {
    DevtoolsTargetId::from_parts(["gpui", "runtime", sanitized_runtime_id(runtime_id).as_str()])
}

fn gpui_runtime_domain_id(runtime_id: &str) -> DevtoolsDomainId {
    DevtoolsDomainId::from_parts(["gpui", "runtime", sanitized_runtime_id(runtime_id).as_str()])
}

fn gpui_window_target_id(index: usize, window: &GpuiRuntimeWindowSnapshot) -> DevtoolsTargetId {
    let index_label = index.to_string();
    let window_id = window.window_id.to_string();
    DevtoolsTargetId::from_parts(["gpui", "window", index_label.as_str(), window_id.as_str()])
}

fn gpui_scroll_target_id(index: usize, scroll: &GpuiRuntimeScrollSnapshot) -> DevtoolsTargetId {
    let index_label = index.to_string();
    let generation = scroll.generation.to_string();
    DevtoolsTargetId::from_parts(["gpui", "scroll", index_label.as_str(), generation.as_str()])
}

fn gpui_runtime_probe_id(runtime_id: &str) -> ProbeId {
    ProbeId::new(format!(
        "{GPUI_RUNTIME_PROBE_ID}.{}",
        sanitized_runtime_id(runtime_id)
    ))
    .expect("internal GPUI runtime probe id is non-empty")
}

fn sanitized_runtime_id(runtime_id: &str) -> String {
    let runtime_id = sanitize_sensitive_text(runtime_id);
    if runtime_id.trim().is_empty() {
        "app".to_owned()
    } else {
        runtime_id
    }
}

fn gpui_runtime_summary_payload(snapshot: &GpuiRuntimeSnapshot) -> serde_json::Value {
    serde_json::json!({
        "runtime_id": sanitized_runtime_id(&snapshot.runtime_id),
        "generation": snapshot.generation,
        "window_count": snapshot.windows.len(),
        "has_focus": snapshot.focus.is_some(),
        "has_input": snapshot.input.is_some(),
        "has_frame": snapshot.frame.is_some(),
        "scroll_viewport_count": snapshot.scroll_viewports.len(),
        "diagnostic_count": snapshot.diagnostics.len(),
    })
}

fn gpui_window_payload(window: &GpuiRuntimeWindowSnapshot) -> serde_json::Value {
    serde_json::json!({
        "window_id": window.window_id,
        "display_id": window.display_id.as_deref().map(sanitize_sensitive_text),
        "active": window.active,
        "focused": window.focused,
        "bounds": window.bounds,
        "content_size": window.content_size,
        "scale_factor": window.scale_factor,
    })
}

fn gpui_focus_payload(focus: &GpuiRuntimeFocusSnapshot) -> serde_json::Value {
    serde_json::json!({
        "active_window_id": focus.active_window_id,
        "focused_window_id": focus.focused_window_id,
        "focus_scope_count": focus.focus_scope_count,
        "focus_handle_count": focus.focus_handle_count,
    })
}

fn gpui_input_payload(input: &GpuiRuntimeInputSnapshot) -> serde_json::Value {
    serde_json::json!({
        "key_down_count": input.key_down_count,
        "pointer_event_count": input.pointer_event_count,
        "scroll_event_count": input.scroll_event_count,
        "text_input_event_count": input.text_input_event_count,
        "ime_event_count": input.ime_event_count,
        "clipboard_event_count": input.clipboard_event_count,
        "last_event_kind": input.last_event_kind.as_deref().map(sanitize_sensitive_text),
    })
}

fn gpui_frame_payload(frame: &GpuiRuntimeFrameSnapshot) -> serde_json::Value {
    serde_json::json!({
        "requested_frames": frame.requested_frames,
        "painted_frames": frame.painted_frames,
        "animation_frame_count": frame.animation_frame_count,
        "last_frame_duration_ms": frame.last_frame_duration_ms,
        "last_presented_generation": frame.last_presented_generation,
    })
}

fn gpui_scroll_payload(scroll: &GpuiRuntimeScrollSnapshot) -> serde_json::Value {
    serde_json::json!({
        "generation": scroll.generation,
        "source": sanitize_sensitive_text(&scroll.source),
        "bounds": scroll.bounds,
        "offset": scroll.offset,
        "max_offset": scroll.max_offset,
        "content_size": scroll.content_size,
    })
}

fn runtime_rect_snapshot(bounds: Bounds<Pixels>) -> GpuiRuntimeRectSnapshot {
    GpuiRuntimeRectSnapshot {
        origin: runtime_point_snapshot(bounds.origin),
        size: runtime_size_snapshot(bounds.size),
    }
}

fn runtime_point_snapshot(point: Point<Pixels>) -> GpuiRuntimePointSnapshot {
    GpuiRuntimePointSnapshot {
        x: point.x.as_f32(),
        y: point.y.as_f32(),
    }
}

fn runtime_size_snapshot(size: GpuiSize<Pixels>) -> GpuiRuntimeSizeSnapshot {
    GpuiRuntimeSizeSnapshot {
        width: size.width.as_f32(),
        height: size.height.as_f32(),
    }
}

fn scroll_viewport_source_label(source: ScrollViewportChangeSource) -> &'static str {
    match source {
        ScrollViewportChangeSource::InitialLayout => "initial-layout",
        ScrollViewportChangeSource::Layout => "layout",
        ScrollViewportChangeSource::Resize => "resize",
        ScrollViewportChangeSource::ContentSize => "content-size",
        ScrollViewportChangeSource::Wheel => "wheel",
        ScrollViewportChangeSource::Scrollbar => "scrollbar",
        ScrollViewportChangeSource::Keyboard => "keyboard",
        ScrollViewportChangeSource::Touch => "touch",
        ScrollViewportChangeSource::Programmatic(source) => match source {
            ScrollViewportProgrammaticSource::Offset => "programmatic-offset",
            ScrollViewportProgrammaticSource::Reveal => "programmatic-reveal",
            ScrollViewportProgrammaticSource::ScrollToBottom => "programmatic-scroll-to-bottom",
        },
    }
}

fn bounds_payload(bounds: Bounds<Pixels>) -> serde_json::Value {
    serde_json::json!({
        "origin": point_payload(bounds.origin),
        "size": size_payload(bounds.size),
    })
}

fn point_payload(point: Point<Pixels>) -> serde_json::Value {
    serde_json::json!({
        "x": point.x.as_f32(),
        "y": point.y.as_f32(),
    })
}

fn size_payload(size: GpuiSize<Pixels>) -> serde_json::Value {
    serde_json::json!({
        "width": size.width.as_f32(),
        "height": size.height.as_f32(),
    })
}

fn layout_bounds_snapshot(bounds: Bounds<Pixels>) -> LayoutBoundsSnapshot {
    LayoutBoundsSnapshot::new(
        layout_point_snapshot(bounds.origin),
        layout_size_snapshot(bounds.size),
    )
}

fn layout_point_snapshot(point: Point<Pixels>) -> LayoutPointSnapshot {
    LayoutPointSnapshot::new(point.x.as_f32() as f64, point.y.as_f32() as f64)
}

fn layout_size_snapshot(size: GpuiSize<Pixels>) -> LayoutSizeSnapshot {
    LayoutSizeSnapshot::new(size.width.as_f32() as f64, size.height.as_f32() as f64)
}
