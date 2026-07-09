//! GPUI read-only inspector surface.

use crate::{
    CaptureProvider, DevtoolsCapture, DevtoolsDiffRow, DevtoolsDomainId, DevtoolsDomainKind,
    DevtoolsDomainSnapshot, DevtoolsEventKind, DevtoolsEventRecord, DevtoolsInspectorDetail,
    DevtoolsInspectorSessionFrameSummary, DevtoolsInspectorState, DevtoolsSessionFrame,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId,
    ProbeSnapshotError, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{sanitize_sensitive_text, snapshot_node_with_payload},
    layout::{
        LayoutBoundsSnapshot, LayoutNodeSnapshot, LayoutPointSnapshot, LayoutSizeSnapshot,
        LayoutSnapshot,
    },
};
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, Div, ElementId, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, ParentElement, Pixels, Point, Render, RenderOnce,
    ScrollViewportChangeSource, ScrollViewportProgrammaticSource, ScrollViewportSnapshot,
    SharedString, Size as GpuiSize, Stateful, Styled, Window, div, px, rgb,
};
use open_gpui_ui_components::prelude::{Sizable, Size};
use open_gpui_ui_components::{FeedbackIntent, ScrollArea, StatusCue};
use serde::{Deserialize, Serialize};

const GPUI_RUNTIME_PROBE_ID: &str = "gpui.runtime";

/// Concrete read-only GPUI inspector for devtools snapshot collections.
#[derive(IntoElement)]
pub struct DevtoolsInspector {
    id: ElementId,
    title: SharedString,
    state: DevtoolsInspectorState,
}

impl DevtoolsInspector {
    /// Creates a read-only inspector.
    pub fn new(id: impl Into<ElementId>, state: DevtoolsInspectorState) -> Self {
        Self {
            id: id.into(),
            title: "DevTools Inspector".into(),
            state,
        }
    }

    /// Applies a visible title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Returns the projected inspector state.
    pub const fn state(&self) -> &DevtoolsInspectorState {
        &self.state
    }
}

/// Stateful GPUI inspector controller with click, keyboard, copy, and export feedback.
#[derive(Debug)]
pub struct DevtoolsInspectorController {
    id: ElementId,
    title: SharedString,
    state: DevtoolsInspectorState,
    focus_handle: FocusHandle,
    feedback_label: Option<SharedString>,
}

impl DevtoolsInspectorController {
    /// Creates a stateful inspector controller.
    pub fn new(
        id: impl Into<ElementId>,
        state: DevtoolsInspectorState,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            title: "DevTools Inspector".into(),
            state,
            focus_handle: cx.focus_handle().tab_stop(true),
            feedback_label: None,
        }
    }

    /// Applies a visible title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Returns the projected inspector state.
    pub const fn state(&self) -> &DevtoolsInspectorState {
        &self.state
    }

    /// Returns the latest interactive feedback label.
    pub fn feedback_label(&self) -> Option<&SharedString> {
        self.feedback_label.as_ref()
    }

    /// Replaces the current capture while preserving inspector filter and selection when possible.
    pub fn update_capture(&mut self, capture: DevtoolsCapture, cx: &mut Context<Self>) {
        self.state = self.state.clone().replace_capture(capture);
        self.feedback_label = Some("DevTools capture refreshed".into());
        cx.notify();
    }

    /// Replaces the current session frame while preserving inspector filter and selection when possible.
    pub fn update_session_frame(&mut self, frame: DevtoolsSessionFrame, cx: &mut Context<Self>) {
        let generation = frame.generation;
        self.state = self.state.clone().replace_session_frame(frame);
        self.feedback_label = Some(format!("DevTools session frame #{generation} loaded").into());
        cx.notify();
    }

    fn select_target(&mut self, target_id: &DevtoolsTargetId) {
        match self.state.clone().select_target(target_id) {
            Ok(state) => {
                self.state = state;
                self.feedback_label =
                    Some(format!("Selected target {}", target_id.as_str()).into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    fn select_domain(&mut self, domain_id: &DevtoolsDomainId) {
        match self.state.clone().select_domain(domain_id) {
            Ok(state) => {
                self.state = state;
                self.feedback_label =
                    Some(format!("Selected domain {}", domain_id.as_str()).into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    fn select_event(&mut self, sequence: u64) {
        match self.state.clone().select_event(sequence) {
            Ok(state) => {
                self.state = state;
                self.feedback_label = Some(format!("Selected event #{sequence}").into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    fn select_probe(&mut self, probe_id: &ProbeId) {
        match self.state.clone().select_probe(probe_id) {
            Ok(state) => {
                self.state = state;
                self.feedback_label =
                    Some(format!("Selected snapshot {}", probe_id.as_str()).into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    fn copy_selected_detail(&mut self, cx: &mut Context<Self>) {
        match self.state.copy_selected_detail() {
            Ok(action) => {
                cx.write_to_clipboard(ClipboardItem::new_string(action.pretty_json));
                self.feedback_label = Some(action.feedback_label.into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    fn export_capture(&mut self, cx: &mut Context<Self>) {
        match self.state.export_capture() {
            Ok(export) => {
                cx.write_to_clipboard(ClipboardItem::new_string(export.pretty_json));
                self.feedback_label = Some(export.feedback_label.into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let result = match event.keystroke.key.as_str() {
            "down" => self.state.clone().select_next_event(),
            "up" => self.state.clone().select_previous_event(),
            "right" => self.state.clone().select_next_domain(),
            "left" => self.state.clone().select_previous_domain(),
            "tab" => self.state.clone().select_next_target(),
            _ => return,
        };

        match result {
            Ok(state) => {
                self.state = state;
                self.feedback_label = Some("Selection moved".into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
        cx.notify();
    }
}

impl Focusable for DevtoolsInspectorController {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

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

impl RenderOnce for DevtoolsInspector {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let debug_id = self.id.to_string();
        let category_summaries = self.state.category_summaries();
        let snapshot_rows = self.state.snapshot_rows();
        let target_rows = self.state.target_rows();
        let domain_rows = self.state.domain_rows();
        let event_rows = self.state.event_rows();
        let selected_snapshot = self.state.selected_snapshot().cloned();
        let selected_detail = self.state.selected_detail();
        let diagnostics = self.state.diagnostics().to_vec();
        let session_frame = self.state.session_frame().cloned();
        let diff_rows = self.state.diff_rows().to_vec();

        div()
            .id(self.id)
            .debug_selector(move || format!("devtools-inspector:{debug_id}:root"))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(self.title),
                    )
                    .child(
                        StatusCue::new("devtools-inspector-status", "read-only").intent(
                            if diagnostics.is_empty() {
                                FeedbackIntent::Success
                            } else {
                                FeedbackIntent::Warning
                            },
                        ),
                    ),
            )
            .child(render_category_summaries(category_summaries))
            .when(session_frame.is_some() || !diff_rows.is_empty(), |this| {
                this.child(render_session_workbench(session_frame, diff_rows))
            })
            .child(
                div()
                    .flex()
                    .gap_3()
                    .min_h(px(0.0))
                    .child(render_capture_navigation(
                        target_rows,
                        domain_rows,
                        event_rows,
                        snapshot_rows,
                    ))
                    .child(render_selected_detail(selected_detail, selected_snapshot)),
            )
            .when(!diagnostics.is_empty(), |this| {
                this.child(render_diagnostics(diagnostics))
            })
    }
}

impl Render for DevtoolsInspectorController {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let debug_id = self.id.to_string();
        let category_summaries = self.state.category_summaries();
        let snapshot_rows = self.state.snapshot_rows();
        let target_rows = self.state.target_rows();
        let domain_rows = self.state.domain_rows();
        let event_rows = self.state.event_rows();
        let selected_snapshot = self.state.selected_snapshot().cloned();
        let selected_detail = self.state.selected_detail();
        let diagnostics = self.state.diagnostics().to_vec();
        let feedback_label = self.feedback_label.clone();
        let session_frame = self.state.session_frame().cloned();
        let diff_rows = self.state.diff_rows().to_vec();

        div()
            .id(self.id.clone())
            .debug_selector(move || format!("devtools-inspector:{debug_id}:root"))
            .key_context("DevtoolsInspector")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key_down(event, cx);
            }))
            .flex()
            .flex_col()
            .gap_3()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(self.title.clone()),
                    )
                    .child(
                        StatusCue::new("devtools-inspector-status", "interactive").intent(
                            if diagnostics.is_empty() {
                                FeedbackIntent::Success
                            } else {
                                FeedbackIntent::Warning
                            },
                        ),
                    ),
            )
            .when_some(feedback_label, |this, feedback| {
                this.child(
                    div()
                        .debug_selector(|| "devtools-inspector:action-feedback".to_owned())
                        .text_xs()
                        .text_color(rgb(0x1f7a66))
                        .child(feedback),
                )
            })
            .child(render_category_summaries(category_summaries))
            .when(session_frame.is_some() || !diff_rows.is_empty(), |this| {
                this.child(render_session_workbench(session_frame, diff_rows))
            })
            .child(
                div()
                    .flex()
                    .gap_3()
                    .min_h(px(0.0))
                    .child(render_interactive_capture_navigation(
                        target_rows,
                        domain_rows,
                        event_rows,
                        snapshot_rows,
                        cx,
                    ))
                    .child(render_interactive_selected_detail(
                        selected_detail,
                        selected_snapshot,
                        cx,
                    )),
            )
            .when(!diagnostics.is_empty(), |this| {
                this.child(render_diagnostics(diagnostics))
            })
    }
}

fn render_category_summaries(
    summaries: Vec<crate::DevtoolsSnapshotCategorySummary>,
) -> impl IntoElement {
    div()
        .debug_selector(|| "devtools-inspector:category-summaries".to_owned())
        .flex()
        .flex_wrap()
        .gap_2()
        .children(summaries.into_iter().map(|summary| {
            let category_label = summary.category_label;
            let snapshot_count = summary.snapshot_count;
            let total_nodes = summary.total_nodes;
            let redacted_values = summary.redacted_values;
            let diagnostics = summary.diagnostics;
            div()
                .id(format!("devtools-inspector-category:{category_label}"))
                .debug_selector({
                    let category_label = category_label.clone();
                    move || format!("devtools-inspector:category:{category_label}")
                })
                .rounded_sm()
                .border_1()
                .border_color(rgb(0xe2e4dc))
                .bg(rgb(0xf7f8f2))
                .px_2()
                .py_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child(category_label),
                )
                .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                    "{} snapshots / {} nodes / {} redacted / {} diagnostics",
                    snapshot_count, total_nodes, redacted_values, diagnostics
                )))
        }))
}

fn render_session_workbench(
    session_frame: Option<DevtoolsInspectorSessionFrameSummary>,
    diff_rows: Vec<DevtoolsDiffRow>,
) -> impl IntoElement {
    let diff_count = diff_rows.len();
    div()
        .debug_selector(|| "devtools-inspector:session-workbench".to_owned())
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xf7f8f2))
        .p_2()
        .flex()
        .flex_col()
        .gap_2()
        .when_some(session_frame, |this, frame| {
            this.child(
                div()
                    .debug_selector(|| "devtools-inspector:session-frame".to_owned())
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(session_pill("session", frame.session_id))
                    .child(session_pill("generation", frame.generation.to_string()))
                    .child(session_pill(
                        "previous",
                        frame
                            .previous_generation
                            .map_or_else(|| "none".to_owned(), |generation| generation.to_string()),
                    ))
                    .child(session_pill("diff rows", frame.diff_row_count.to_string())),
            )
        })
        .when(diff_count > 0, |this| {
            this.child(
                div()
                    .debug_selector(|| "devtools-inspector:diff-list".to_owned())
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(section_label("Diff"))
                    .children(diff_rows.into_iter().take(12).map(render_diff_row))
                    .when(diff_count > 12, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(format!("{} more diff rows", diff_count - 12)),
                        )
                    }),
            )
        })
}

fn session_pill(label: impl Into<String>, value: impl Into<String>) -> impl IntoElement {
    div()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xffffff))
        .px_2()
        .py_1()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child(value.into()),
        )
}

fn render_diff_row(row: DevtoolsDiffRow) -> impl IntoElement {
    let identity = row.identity.clone();
    div()
        .id(format!("devtools-inspector-diff:{identity}"))
        .debug_selector(move || format!("devtools-inspector:diff:{identity}"))
        .rounded_sm()
        .border_1()
        .border_color(if row.status == crate::DevtoolsDiffStatus::Unchanged {
            rgb(0xe2e4dc)
        } else {
            rgb(0x1f7a66)
        })
        .bg(if row.status == crate::DevtoolsDiffStatus::Unchanged {
            rgb(0xfcfcf8)
        } else {
            rgb(0xe8f3ef)
        })
        .px_2()
        .py_1()
        .child(
            div()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child(format!(
                    "{} / {}",
                    row.kind.as_label(),
                    row.status.as_label()
                )),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(format!("{} / {}", row.identity, row.label)),
        )
}

fn render_capture_navigation(
    targets: Vec<crate::DevtoolsTargetRow>,
    domains: Vec<crate::DevtoolsDomainRow>,
    events: Vec<crate::DevtoolsEventRow>,
    snapshots: Vec<crate::DevtoolsSnapshotRow>,
) -> impl IntoElement {
    div()
        .debug_selector(|| "devtools-inspector:capture-navigation".to_owned())
        .w(px(320.0))
        .min_w(px(260.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(render_target_rows(targets))
        .child(render_domain_rows(domains))
        .child(render_event_rows(events))
        .child(render_snapshot_rows(snapshots))
}

fn render_interactive_capture_navigation(
    targets: Vec<crate::DevtoolsTargetRow>,
    domains: Vec<crate::DevtoolsDomainRow>,
    events: Vec<crate::DevtoolsEventRow>,
    snapshots: Vec<crate::DevtoolsSnapshotRow>,
    cx: &mut Context<DevtoolsInspectorController>,
) -> impl IntoElement {
    div()
        .debug_selector(|| "devtools-inspector:capture-navigation".to_owned())
        .w(px(320.0))
        .min_w(px(260.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(render_interactive_target_rows(targets, cx))
        .child(render_interactive_domain_rows(domains, cx))
        .child(render_interactive_event_rows(events, cx))
        .child(render_interactive_snapshot_rows(snapshots, cx))
}

fn render_interactive_target_rows(
    rows: Vec<crate::DevtoolsTargetRow>,
    cx: &mut Context<DevtoolsInspectorController>,
) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:target-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Targets"))
        .children(rows.into_iter().map(|row| {
            let target_id = row.target_id.clone();
            interactive_row_shell(
                format!("devtools-inspector-target:{}", target_id.as_str()),
                {
                    let target_id = target_id.as_str().to_owned();
                    move || format!("devtools-inspector:target:{target_id}")
                },
                row.selected,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_target(&target_id);
                cx.notify();
            }))
            .child(
                div()
                    .text_xs()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(row.label),
            )
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                "{} / {} domains / {} events / {} children",
                row.kind_label, row.domain_count, row.event_count, row.child_target_count
            )))
        }))
        .when(is_empty, |this| this.child(empty_state("No targets")))
}

fn render_interactive_domain_rows(
    rows: Vec<crate::DevtoolsDomainRow>,
    cx: &mut Context<DevtoolsInspectorController>,
) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:domain-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Domains"))
        .children(rows.into_iter().map(|row| {
            let domain_id = row.domain_id.clone();
            interactive_row_shell(
                format!("devtools-inspector-domain:{}", domain_id.as_str()),
                {
                    let domain_id = domain_id.as_str().to_owned();
                    move || format!("devtools-inspector:domain:{domain_id}")
                },
                row.selected,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_domain(&domain_id);
                cx.notify();
            }))
            .child(
                div()
                    .text_xs()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(row.label),
            )
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                "{} / roots {} / events {} / diagnostics {} / redacted {}",
                row.kind_label,
                row.snapshot_root_nodes,
                row.event_count,
                row.diagnostic_count,
                row.redacted_values
            )))
        }))
        .when(is_empty, |this| this.child(empty_state("No domains")))
}

fn render_interactive_event_rows(
    rows: Vec<crate::DevtoolsEventRow>,
    cx: &mut Context<DevtoolsInspectorController>,
) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:event-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Events"))
        .children(rows.into_iter().map(|row| {
            let sequence = row.sequence;
            interactive_row_shell(
                format!("devtools-inspector-event:{sequence}"),
                move || format!("devtools-inspector:event:{sequence}"),
                row.selected,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_event(sequence);
                cx.notify();
            }))
            .child(
                div()
                    .text_xs()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(row.label),
            )
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                "#{} / {} / payload {}",
                row.sequence, row.kind_label, row.has_payload
            )))
        }))
        .when(is_empty, |this| this.child(empty_state("No events")))
}

fn render_interactive_snapshot_rows(
    rows: Vec<crate::DevtoolsSnapshotRow>,
    cx: &mut Context<DevtoolsInspectorController>,
) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:snapshot-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Legacy snapshots"))
        .child(
            ScrollArea::new(
                "devtools-inspector-snapshot-list",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(rows.into_iter().map(|row| {
                        let probe_id = row.probe_id.clone();
                        interactive_row_shell(
                            format!("devtools-inspector-row:{}", probe_id.as_str()),
                            {
                                let probe_id = probe_id.as_str().to_owned();
                                move || format!("devtools-inspector:row:{probe_id}")
                            },
                            row.selected,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_probe(&probe_id);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(open_gpui::FontWeight::BOLD)
                                .child(row.probe_id.as_str().to_owned()),
                        )
                        .child(
                            div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                                "{} / {} / roots {} / nodes {} / redacted {}",
                                row.category_label,
                                row.kind_label,
                                row.root_nodes,
                                row.total_nodes,
                                row.redacted_values
                            )),
                        )
                    })),
            )
            .with_size(Size::Small),
        )
        .when(is_empty, |this| {
            this.child(empty_state("No legacy snapshots"))
        })
}

fn interactive_row_shell(
    id: impl Into<ElementId>,
    debug_selector: impl Fn() -> String + 'static,
    selected: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(debug_selector)
        .cursor_pointer()
        .occlude()
        .rounded_sm()
        .border_1()
        .border_color(if selected {
            rgb(0x1f7a66)
        } else {
            rgb(0xe2e4dc)
        })
        .bg(if selected {
            rgb(0xe8f3ef)
        } else {
            rgb(0xfcfcf8)
        })
        .hover(|style| style.bg(rgb(0xf1f6f4)))
        .px_2()
        .py_1()
        .flex()
        .flex_col()
        .gap_1()
}

fn render_target_rows(rows: Vec<crate::DevtoolsTargetRow>) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:target-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Targets"))
        .children(rows.into_iter().map(|row| {
            div()
                .id(format!(
                    "devtools-inspector-target:{}",
                    row.target_id.as_str()
                ))
                .debug_selector({
                    let target_id = row.target_id.as_str().to_owned();
                    move || format!("devtools-inspector:target:{target_id}")
                })
                .rounded_sm()
                .border_1()
                .border_color(if row.selected {
                    rgb(0x1f7a66)
                } else {
                    rgb(0xe2e4dc)
                })
                .bg(if row.selected {
                    rgb(0xe8f3ef)
                } else {
                    rgb(0xfcfcf8)
                })
                .px_2()
                .py_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child(row.label),
                )
                .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                    "{} / {} domains / {} events / {} children",
                    row.kind_label, row.domain_count, row.event_count, row.child_target_count
                )))
        }))
        .when(is_empty, |this| this.child(empty_state("No targets")))
}

fn render_domain_rows(rows: Vec<crate::DevtoolsDomainRow>) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:domain-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Domains"))
        .children(rows.into_iter().map(|row| {
            div()
                .id(format!(
                    "devtools-inspector-domain:{}",
                    row.domain_id.as_str()
                ))
                .debug_selector({
                    let domain_id = row.domain_id.as_str().to_owned();
                    move || format!("devtools-inspector:domain:{domain_id}")
                })
                .rounded_sm()
                .border_1()
                .border_color(if row.selected {
                    rgb(0x1f7a66)
                } else {
                    rgb(0xe2e4dc)
                })
                .bg(if row.selected {
                    rgb(0xe8f3ef)
                } else {
                    rgb(0xfcfcf8)
                })
                .px_2()
                .py_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child(row.label),
                )
                .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                    "{} / roots {} / events {} / diagnostics {} / redacted {}",
                    row.kind_label,
                    row.snapshot_root_nodes,
                    row.event_count,
                    row.diagnostic_count,
                    row.redacted_values
                )))
        }))
        .when(is_empty, |this| this.child(empty_state("No domains")))
}

fn render_event_rows(rows: Vec<crate::DevtoolsEventRow>) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:event-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Events"))
        .children(rows.into_iter().map(|row| {
            div()
                .id(format!("devtools-inspector-event:{}", row.sequence))
                .debug_selector({
                    let sequence = row.sequence;
                    move || format!("devtools-inspector:event:{sequence}")
                })
                .rounded_sm()
                .border_1()
                .border_color(if row.selected {
                    rgb(0x1f7a66)
                } else {
                    rgb(0xe2e4dc)
                })
                .bg(if row.selected {
                    rgb(0xe8f3ef)
                } else {
                    rgb(0xfcfcf8)
                })
                .px_2()
                .py_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child(row.label),
                )
                .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                    "#{} / {} / payload {}",
                    row.sequence, row.kind_label, row.has_payload
                )))
        }))
        .when(is_empty, |this| this.child(empty_state("No events")))
}

fn render_snapshot_rows(rows: Vec<crate::DevtoolsSnapshotRow>) -> impl IntoElement {
    let is_empty = rows.is_empty();
    div()
        .debug_selector(|| "devtools-inspector:snapshot-list".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .child(section_label("Legacy snapshots"))
        .child(
            ScrollArea::new(
                "devtools-inspector-snapshot-list",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(rows.into_iter().map(|row| {
                        div()
                            .id(format!("devtools-inspector-row:{}", row.probe_id.as_str()))
                            .debug_selector({
                                let probe_id = row.probe_id.as_str().to_owned();
                                move || format!("devtools-inspector:row:{probe_id}")
                            })
                            .rounded_sm()
                            .border_1()
                            .border_color(if row.selected {
                                rgb(0x1f7a66)
                            } else {
                                rgb(0xe2e4dc)
                            })
                            .bg(if row.selected {
                                rgb(0xe8f3ef)
                            } else {
                                rgb(0xfcfcf8)
                            })
                            .px_2()
                            .py_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(row.probe_id.as_str().to_owned()),
                            )
                            .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                                "{} / {} / roots {} / nodes {} / redacted {}",
                                row.category_label,
                                row.kind_label,
                                row.root_nodes,
                                row.total_nodes,
                                row.redacted_values
                            )))
                    })),
            )
            .with_size(Size::Small),
        )
        .when(is_empty, |this| {
            this.child(empty_state("No legacy snapshots"))
        })
}

fn render_selected_detail(
    detail: Option<DevtoolsInspectorDetail>,
    fallback_snapshot: Option<SnapshotEnvelope>,
) -> impl IntoElement {
    let content = if let Some(detail) = detail {
        let payload = detail.json.to_string();
        div()
            .debug_selector(|| "devtools-inspector:selected-detail-content".to_owned())
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(format!("{} / {}", detail.kind_label, detail.label)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(detail.copy_label)
                    .child(detail.export_label)
                    .child(detail.feedback_label),
            )
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(payload))
            .into_any_element()
    } else if let Some(snapshot) = fallback_snapshot {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(format!(
                        "{} / {}",
                        snapshot.probe_id.as_str(),
                        snapshot.kind.as_label()
                    )),
            )
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                "redacted {} values / {} notes",
                snapshot.redaction.redacted_values,
                snapshot.redaction.notes.len()
            )))
            .children(
                snapshot
                    .tree
                    .nodes
                    .into_iter()
                    .map(|node| render_snapshot_node(node, 0)),
            )
            .into_any_element()
    } else {
        div()
            .text_sm()
            .text_color(rgb(0x5a6472))
            .child("No detail selected")
            .into_any_element()
    };

    div()
        .debug_selector(|| "devtools-inspector:selected-detail".to_owned())
        .flex_1()
        .min_w(px(0.0))
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xfcfcf8))
        .p_3()
        .child(content)
}

fn render_interactive_selected_detail(
    detail: Option<DevtoolsInspectorDetail>,
    fallback_snapshot: Option<SnapshotEnvelope>,
    cx: &mut Context<DevtoolsInspectorController>,
) -> impl IntoElement {
    let content = if let Some(detail) = detail {
        let payload = detail.json.to_string();
        div()
            .debug_selector(|| "devtools-inspector:selected-detail-content".to_owned())
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(format!("{} / {}", detail.kind_label, detail.label)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        action_button(
                            "devtools-inspector-copy-detail",
                            "devtools-inspector:copy-detail",
                            detail.copy_label,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.copy_selected_detail(cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        action_button(
                            "devtools-inspector-export-capture",
                            "devtools-inspector:export-capture",
                            "Export capture JSON",
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.export_capture(cx);
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "devtools-inspector:selected-detail-json".to_owned())
                    .text_xs()
                    .text_color(rgb(0x5a6472))
                    .child(payload),
            )
            .into_any_element()
    } else if let Some(snapshot) = fallback_snapshot {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(format!(
                        "{} / {}",
                        snapshot.probe_id.as_str(),
                        snapshot.kind.as_label()
                    )),
            )
            .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                "redacted {} values / {} notes",
                snapshot.redaction.redacted_values,
                snapshot.redaction.notes.len()
            )))
            .children(
                snapshot
                    .tree
                    .nodes
                    .into_iter()
                    .map(|node| render_snapshot_node(node, 0)),
            )
            .into_any_element()
    } else {
        div()
            .text_sm()
            .text_color(rgb(0x5a6472))
            .child("No detail selected")
            .into_any_element()
    };

    div()
        .debug_selector(|| "devtools-inspector:selected-detail".to_owned())
        .flex_1()
        .min_w(px(0.0))
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xfcfcf8))
        .p_3()
        .child(content)
}

fn action_button(
    id: impl Into<ElementId>,
    debug_selector: &'static str,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || debug_selector.to_owned())
        .cursor_pointer()
        .occlude()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .hover(|style| style.bg(rgb(0xf1f6f4)))
        .px_2()
        .py_1()
        .text_xs()
        .child(label.into())
}

fn render_snapshot_node(node: SnapshotNode, depth: usize) -> AnyElement {
    let payload = node
        .payload
        .as_ref()
        .map(|payload| payload.to_string())
        .unwrap_or_else(|| "no payload".to_owned());
    div()
        .ml(px((depth as f32) * 12.0))
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xffffff))
        .px_2()
        .py_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child(format!("{} / {}", node.id, node.label)),
        )
        .child(div().text_xs().text_color(rgb(0x5a6472)).child(payload))
        .children(
            node.children
                .into_iter()
                .map(|child| render_snapshot_node(child, depth + 1)),
        )
        .into_any_element()
}

fn render_diagnostics(diagnostics: Vec<crate::SnapshotDiagnostic>) -> impl IntoElement {
    div()
        .debug_selector(|| "devtools-inspector:diagnostics".to_owned())
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd9c7a8))
        .bg(rgb(0xf4f1ea))
        .p_2()
        .flex()
        .flex_col()
        .gap_1()
        .children(diagnostics.into_iter().map(|diagnostic| {
            div().text_xs().text_color(rgb(0x6a512b)).child(format!(
                "{}: {}",
                diagnostic.probe_id.as_str(),
                diagnostic.message
            ))
        }))
}

fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(open_gpui::FontWeight::BOLD)
        .text_color(rgb(0x2f3947))
        .child(label)
}

fn empty_state(label: &'static str) -> impl IntoElement {
    div().text_xs().text_color(rgb(0x7a8492)).child(label)
}
