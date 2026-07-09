use crate::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsEventIdentity, DevtoolsInspectorState,
    DevtoolsSessionFrame, DevtoolsTargetId, ProbeId,
};
use open_gpui::prelude::*;
use open_gpui::{
    App, ClipboardItem, Context, ElementId, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window, div, px, rgb,
};
use open_gpui_ui_components::{FeedbackIntent, StatusCue};

use super::render::{
    render_capture_navigation, render_category_summaries, render_diagnostics,
    render_interactive_capture_navigation, render_interactive_selected_detail,
    render_selected_detail, render_session_workbench,
};

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

    pub(super) fn select_target(&mut self, target_id: &DevtoolsTargetId) {
        match self.state.clone().select_target(target_id) {
            Ok(state) => {
                self.state = state;
                self.feedback_label =
                    Some(format!("Selected target {}", target_id.as_str()).into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    pub(super) fn select_domain(&mut self, domain_id: &DevtoolsDomainId) {
        match self.state.clone().select_domain(domain_id) {
            Ok(state) => {
                self.state = state;
                self.feedback_label =
                    Some(format!("Selected domain {}", domain_id.as_str()).into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    pub(super) fn select_event_identity(&mut self, identity: &DevtoolsEventIdentity) {
        match self.state.clone().select_event_identity(identity) {
            Ok(state) => {
                let sequence = identity.sequence;
                self.state = state;
                self.feedback_label = Some(format!("Selected event #{sequence}").into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    pub(super) fn select_probe(&mut self, probe_id: &ProbeId) {
        match self.state.clone().select_probe(probe_id) {
            Ok(state) => {
                self.state = state;
                self.feedback_label =
                    Some(format!("Selected snapshot {}", probe_id.as_str()).into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    pub(super) fn copy_selected_detail(&mut self, cx: &mut Context<Self>) {
        match self.state.copy_selected_detail() {
            Ok(action) => {
                cx.write_to_clipboard(ClipboardItem::new_string(action.pretty_json));
                self.feedback_label = Some(action.feedback_label.into());
            }
            Err(error) => self.feedback_label = Some(error.to_string().into()),
        }
    }

    pub(super) fn export_capture(&mut self, cx: &mut Context<Self>) {
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
