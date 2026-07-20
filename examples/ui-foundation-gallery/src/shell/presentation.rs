//! Interactive subtree presentation helpers for the foundation gallery shell.

use super::*;
use open_gpui::{
    DropEvent, ElementGeometry, MeasuredElementSnapshot, SubtreeTransform, SubtreeTransformExt,
    SubtreeTransformOrigin, measured_element, point,
};
use open_gpui_motion::{MotionProjection, motion_point, motion_px, motion_rect, motion_size};
use open_gpui_ui_components::{TextInput, gpui_adapter::subtree_transform_from_motion_projection};

const PRESENTATION_STAGE_WIDTH: Pixels = px(392.0);
const PRESENTATION_STAGE_HEIGHT: Pixels = px(320.0);

#[derive(Clone, Copy, Debug)]
struct PresentationDragPayload {
    label: &'static str,
}

struct PresentationDragPreview {
    label: &'static str,
}

impl Render for PresentationDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(92.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x155e75))
            .bg(rgb(0xe6f6fa))
            .text_xs()
            .text_color(rgb(0x164e63))
            .shadow_md()
            .child(self.label)
    }
}

impl GalleryShell {
    pub fn set_presentation_progress(&mut self, progress: f32, cx: &mut Context<Self>) {
        if self.presentation_projection_progress != progress {
            self.presentation_projection_progress = progress;
            cx.notify();
        }
    }

    pub fn set_presentation_state(
        &mut self,
        presentation: SubtreePresentation,
        cx: &mut Context<Self>,
    ) {
        if self.presentation_state != presentation {
            self.presentation_state = presentation;
            cx.notify();
        }
    }

    fn increment_presentation_action(&mut self, cx: &mut Context<Self>) {
        self.presentation_action_count += 1;
        cx.notify();
    }

    fn set_presentation_drag_status(&mut self, status: impl Into<String>, cx: &mut Context<Self>) {
        let status = status.into();
        if self.presentation_drag_status != status {
            self.presentation_drag_status = status;
            cx.notify();
        }
    }

    fn set_presentation_geometry(&mut self, geometry: ElementGeometry, cx: &mut Context<Self>) {
        if self.presentation_geometry != Some(geometry) {
            self.presentation_geometry = Some(geometry);
            cx.notify();
        }
    }

    pub(super) fn render_presentation_page(
        &self,
        snapshot: GalleryShellSnapshot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let progress = self.presentation_projection_progress;
        let presentation = self.presentation_state;
        let projection = MotionProjection::between(
            motion_rect(
                motion_point(motion_px(18.0), motion_px(12.0)),
                motion_size(motion_px(354.0), motion_px(282.0)),
            ),
            motion_rect(
                motion_point(motion_px(0.0), motion_px(0.0)),
                motion_size(
                    motion_px(PRESENTATION_STAGE_WIDTH.as_f32()),
                    motion_px(PRESENTATION_STAGE_HEIGHT.as_f32()),
                ),
            ),
        );
        let projection_sample = projection
            .try_transform_sample(progress)
            .expect("Gallery presentation projection uses finite positive geometry");
        let projection_transform = subtree_transform_from_motion_projection(projection_sample)
            .expect("Checked Motion projection must convert to a GPUI subtree transform");
        let nested_progress = progress.clamp(0.0, 1.0);
        let nested_transform = SubtreeTransform::try_new(
            size(
                1.0 + 0.05 * (1.0 - nested_progress),
                1.0 - 0.06 * (1.0 - nested_progress),
            ),
            point(px(0.0), px(0.0)),
            SubtreeTransformOrigin::CENTER,
        )
        .expect("Gallery nested transform uses finite positive scale");

        let shell = cx.entity().downgrade();
        let action = Button::new("presentation-action", "Run action")
            .variant(ButtonVariant::Secondary)
            .with_size(snapshot.control_size)
            .tokens(snapshot.tokens)
            .tooltip_text("Committed transformed tooltip")
            .on_activate(cx.processor(|this, _, _, cx| {
                this.increment_presentation_action(cx);
            }));
        let popover = Popover::new(
            "presentation-popover",
            "Open details",
            "Presentation overlay content",
        );

        let drag_source = div()
            .id("presentation-drag-source")
            .debug_selector(|| "gallery:presentation-drag-source".into())
            .h(px(46.0))
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x155e75))
            .bg(rgb(0xe6f6fa))
            .text_xs()
            .text_color(rgb(0x164e63))
            .cursor_move()
            .child("Source")
            .on_drag(PresentationDragPayload { label: "Payload" }, {
                let shell = shell.clone();
                move |payload, geometry, _, cx| {
                    let local = geometry
                        .target_local_position()
                        .map(format_point)
                        .unwrap_or_else(|_| "unavailable".to_owned());
                    let preview = format_point(geometry.window_preview_offset());
                    shell
                        .update(cx, |this, cx| {
                            this.set_presentation_drag_status(
                                format!(
                                    "Started {} at local {local}; preview {preview}",
                                    payload.label
                                ),
                                cx,
                            );
                        })
                        .ok();
                    cx.new(|_| PresentationDragPreview {
                        label: payload.label,
                    })
                }
            });

        let drop_target = div()
            .id("presentation-drop-target")
            .debug_selector(|| "gallery:presentation-drop-target".into())
            .h(px(46.0))
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x8a5a00))
            .bg(rgb(0xfff5d6))
            .text_xs()
            .text_color(rgb(0x684300))
            .can_drop(|value, _, _| value.is::<PresentationDragPayload>())
            .on_drag_move::<PresentationDragPayload>({
                let shell = shell.clone();
                move |event, _, cx| {
                    let local = event
                        .target_local_position()
                        .map(format_point)
                        .unwrap_or_else(|_| "unavailable".to_owned());
                    shell
                        .update(cx, |this, cx| {
                            this.set_presentation_drag_status(
                                format!("Moving {} at local {local}", event.drag().label),
                                cx,
                            );
                        })
                        .ok();
                }
            })
            .on_drop({
                let shell = shell.clone();
                move |event: &DropEvent<PresentationDragPayload>, _, cx| {
                    let local = event
                        .pointer()
                        .target_local_position()
                        .map(format_point)
                        .unwrap_or_else(|_| "unavailable".to_owned());
                    shell
                        .update(cx, |this, cx| {
                            this.set_presentation_drag_status(
                                format!("Dropped {} at local {local}", event.value().label),
                                cx,
                            );
                        })
                        .ok();
                }
            })
            .child("Drop target");

        let scroll_content = div()
            .flex()
            .flex_col()
            .gap_1()
            .children((0..8).map(|index| {
                div()
                    .h(px(24.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_2()
                    .bg(if index % 2 == 0 {
                        rgb(0xf4f5f0)
                    } else {
                        rgb(0xffffff)
                    })
                    .text_xs()
                    .text_color(rgb(0x45515f))
                    .child(format!("Timeline {:02}", index + 1))
            }));

        let inner = div()
            .id("presentation-inner")
            .debug_selector(|| "gallery:presentation-inner".into())
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .role(open_gpui::accesskit::Role::Group)
            .aria_label("Transformed interactive presentation")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Committed subtree"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .debug_selector(|| "gallery:presentation-popover".into())
                                    .child(popover),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "gallery:presentation-action".into())
                                    .child(action),
                            ),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "gallery:presentation-input".into())
                    .child(
                        TextInput::new("presentation-text-input", "Presentation input")
                            .controller(self.presentation_text_input.clone())
                            .placeholder("Transform-aware input")
                            .with_size(snapshot.control_size)
                            .tokens(snapshot.tokens),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "gallery:presentation-scroll".into())
                    .h(px(82.0))
                    .min_h(px(82.0))
                    .overflow_hidden()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xd6d8ce))
                    .child(
                        ScrollArea::new("presentation-scroll", scroll_content)
                            .vertical()
                            .scroll_handle(&self.presentation_scroll_handle)
                            .with_size(snapshot.control_size),
                    ),
            )
            .child(div().flex().gap_2().child(drag_source).child(drop_target))
            .with_subtree_transform(nested_transform);

        let stage = div()
            .id("presentation-stage")
            .debug_selector(|| "gallery:presentation-stage".into())
            .w(PRESENTATION_STAGE_WIDTH)
            .h(PRESENTATION_STAGE_HEIGHT)
            .flex_none()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xb8beb3))
            .bg(rgb(0xffffff))
            .p_3()
            .child(inner);
        let measured_stage = measured_element("gallery-presentation-stage-measurement", stage, {
            let shell = shell.clone();
            move |measurement: MeasuredElementSnapshot, cx| {
                shell
                    .update(cx, |this, cx| {
                        this.set_presentation_geometry(measurement.geometry(), cx);
                    })
                    .ok();
            }
        })
        .with_subtree_transform(projection_transform)
        .with_subtree_presentation(presentation);
        let presentation_slot = div()
            .id("presentation-slot")
            .debug_selector(|| "gallery:presentation-slot".into())
            .w(PRESENTATION_STAGE_WIDTH)
            .flex_none()
            .child(measured_stage);
        let presentation_flow_sentinel = div()
            .debug_selector(|| "gallery:presentation-flow-sentinel".into())
            .w(PRESENTATION_STAGE_WIDTH)
            .h(px(1.0))
            .flex_none();
        let matrix_lane = |key: &'static str, label: &'static str, state: SubtreePresentation| {
            let slot_selector = format!("gallery:presentation-matrix:{key}:slot");
            let sentinel_selector = format!("gallery:presentation-matrix:{key}:sentinel");
            div()
                .min_w(px(112.0))
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_xs().text_color(rgb(0x45515f)).child(label))
                .child(
                    div()
                        .debug_selector(move || slot_selector.clone())
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .h(px(64.0))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xb8beb3))
                                .bg(rgb(0xf4f5f0))
                                .child(
                                    div()
                                        .id(format!("presentation-matrix-{key}-control"))
                                        .focusable()
                                        .tab_index(0)
                                        .role(open_gpui::accesskit::Role::Button)
                                        .aria_label("Presentation matrix control")
                                        .text_xs()
                                        .child("Same layout"),
                                )
                                .with_subtree_transform(projection_transform)
                                .with_subtree_presentation(state),
                        )
                        .child(
                            div()
                                .debug_selector(move || sentinel_selector.clone())
                                .h(px(1.0))
                                .flex_none(),
                        ),
                )
        };
        let presentation_matrix = div()
            .debug_selector(|| "gallery:presentation-matrix".into())
            .flex()
            .flex_wrap()
            .gap_3()
            .children([
                matrix_lane("visible", "Visible", SubtreePresentation::Visible),
                matrix_lane("inert", "Inert", SubtreePresentation::Inert),
                matrix_lane("hidden", "Hidden", SubtreePresentation::Hidden),
            ]);

        let geometry_summary = self.presentation_geometry.map_or_else(
            || "Awaiting committed geometry".to_owned(),
            |geometry| {
                format!(
                    "layout {} | displayed {}",
                    format_bounds(geometry.layout_bounds()),
                    format_bounds(geometry.displayed_bounds())
                )
            },
        );
        let projected = progress < 1.0;

        div()
            .id("gallery-presentation-page")
            .debug_selector(|| "gallery:presentation-page".into())
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .debug_selector(|| "gallery:presentation-mode:projected".into())
                            .child(
                                Button::new("presentation-mode-projected", "Projected")
                                    .variant(ButtonVariant::Secondary)
                                    .selected(projected)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, _, cx| {
                                        this.set_presentation_progress(0.0, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:presentation-state:visible".into())
                            .child(
                                Button::new("presentation-state-visible", "Visible")
                                    .variant(ButtonVariant::Secondary)
                                    .selected(presentation == SubtreePresentation::Visible)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, _, cx| {
                                        this.set_presentation_state(
                                            SubtreePresentation::Visible,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:presentation-state:inert".into())
                            .child(
                                Button::new("presentation-state-inert", "Inert")
                                    .variant(ButtonVariant::Secondary)
                                    .selected(presentation == SubtreePresentation::Inert)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, _, cx| {
                                        this.set_presentation_state(SubtreePresentation::Inert, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:presentation-state:hidden".into())
                            .child(
                                Button::new("presentation-state-hidden", "Hidden")
                                    .variant(ButtonVariant::Secondary)
                                    .selected(presentation == SubtreePresentation::Hidden)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, _, cx| {
                                        this.set_presentation_state(
                                            SubtreePresentation::Hidden,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:presentation-mode:final".into())
                            .child(
                                Button::new("presentation-mode-final", "Final")
                                    .variant(ButtonVariant::Secondary)
                                    .selected(!projected)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, _, cx| {
                                        this.set_presentation_progress(1.0, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x5a6472))
                            .child(format!("projection {:.2}", progress)),
                    ),
            )
            .child(presentation_slot)
            .child(presentation_flow_sentinel)
            .child(presentation_matrix)
            .child(
                div()
                    .id("presentation-readout")
                    .debug_selector(|| "gallery:presentation-readout".into())
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_l_2()
                    .border_color(rgb(0x1f7a66))
                    .pl_3()
                    .text_xs()
                    .text_color(rgb(0x45515f))
                    .child(geometry_summary)
                    .child(format!(
                        "presentation {presentation:?} | actions {} | drag {}",
                        self.presentation_action_count, self.presentation_drag_status,
                    )),
            )
            .child(self.render_signal_list(snapshot.selected_page))
    }
}

fn format_point(point: open_gpui::Point<Pixels>) -> String {
    format!("({:.1}, {:.1})", point.x.as_f32(), point.y.as_f32())
}

fn format_bounds(bounds: Bounds<Pixels>) -> String {
    format!(
        "({:.1}, {:.1}) {:.1}x{:.1}",
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32()
    )
}
