//! GPUI read-only inspector surface.

use crate::{
    DevtoolsDomainId, DevtoolsInspectorDetail, DevtoolsInspectorState, DevtoolsTargetId, ProbeId,
    SnapshotDiagnostic, SnapshotEnvelope, SnapshotNode, SnapshotProbeSnapshot,
    SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
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
