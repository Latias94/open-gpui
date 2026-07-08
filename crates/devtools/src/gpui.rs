//! GPUI read-only inspector surface.

use crate::{
    DevtoolsInspectorState, ProbeId, SnapshotDiagnostic, SnapshotEnvelope, SnapshotNode,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
};
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, ParentElement, Pixels, Point, RenderOnce,
    ScrollViewportChangeSource, ScrollViewportProgrammaticSource, ScrollViewportSnapshot,
    SharedString, Size as GpuiSize, Styled, Window, div, px, rgb,
};
use open_gpui_ui_components::prelude::{Sizable, Size};
use open_gpui_ui_components::{FeedbackIntent, ScrollArea, StatusCue};

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

/// Creates a sanitized diagnostic for an unavailable scroll viewport snapshot.
pub fn scroll_viewport_unavailable_diagnostic(probe_id: ProbeId) -> SnapshotDiagnostic {
    SnapshotDiagnostic::new(
        probe_id,
        "runtime.unavailable",
        "scroll viewport snapshot is not committed",
    )
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

impl RenderOnce for DevtoolsInspector {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let debug_id = self.id.to_string();
        let rows = self.state.snapshot_rows();
        let selected = self.state.selected_snapshot().cloned();
        let diagnostics = self.state.diagnostics().to_vec();

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
            .child(
                div()
                    .flex()
                    .gap_3()
                    .min_h(px(0.0))
                    .child(render_snapshot_rows(rows))
                    .child(render_selected_snapshot(selected)),
            )
            .when(!diagnostics.is_empty(), |this| {
                this.child(render_diagnostics(diagnostics))
            })
    }
}

fn render_snapshot_rows(rows: Vec<crate::DevtoolsSnapshotRow>) -> impl IntoElement {
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
                        "{} / roots {} / nodes {} / redacted {}",
                        row.kind_label, row.root_nodes, row.total_nodes, row.redacted_values
                    )))
            })),
    )
    .with_size(Size::Small)
}

fn render_selected_snapshot(snapshot: Option<SnapshotEnvelope>) -> impl IntoElement {
    let content = if let Some(snapshot) = snapshot {
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
            .child("No snapshot selected")
            .into_any_element()
    };

    div()
        .flex_1()
        .min_w(px(0.0))
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xfcfcf8))
        .p_3()
        .child(content)
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
