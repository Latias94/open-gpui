use crate::{
    DevtoolsDiffRow, DevtoolsInspectorDetail, DevtoolsInspectorSessionFrameSummary,
    SnapshotEnvelope, SnapshotNode,
};
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, Context, Div, ElementId, IntoElement, ParentElement, SharedString, Stateful,
    Styled, div, px, rgb,
};
use open_gpui_ui_components::ScrollArea;
use open_gpui_ui_components::prelude::{Sizable, Size};

use super::inspector::DevtoolsInspectorController;

pub(super) fn render_category_summaries(
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

pub(super) fn render_session_workbench(
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

pub(super) fn render_capture_navigation(
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

pub(super) fn render_interactive_capture_navigation(
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
            let event_identity = row.event_identity.clone();
            let event_identity_key = event_identity.as_key();
            interactive_row_shell(
                format!("devtools-inspector-event:{event_identity_key}"),
                move || format!("devtools-inspector:event:{event_identity_key}"),
                row.selected,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_event_identity(&event_identity);
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
            let event_identity_key = row.event_identity.as_key();
            div()
                .id(format!("devtools-inspector-event:{event_identity_key}"))
                .debug_selector(move || format!("devtools-inspector:event:{event_identity_key}"))
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

pub(super) fn render_selected_detail(
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

pub(super) fn render_interactive_selected_detail(
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

pub(super) fn render_diagnostics(diagnostics: Vec<crate::SnapshotDiagnostic>) -> impl IntoElement {
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
