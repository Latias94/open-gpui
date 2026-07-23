//! Bring-into-view authority scenario for the foundation gallery.

use super::*;
use open_gpui::{
    BringIntoViewAlignment, BringIntoViewBehavior, BringIntoViewOptions, Point,
    SubtreeTransformOrigin,
};
use open_gpui_motion::{
    MotionDuration, MotionEasing, MotionIntent, MotionPreference, MotionTransition,
};
use std::time::Duration;

impl GalleryShell {
    fn begin_bring_into_view_demo_operation(&mut self) -> u64 {
        self.bring_into_view_generation = self
            .bring_into_view_generation
            .checked_add(1)
            .expect("Gallery bring-into-view generation exhausted");
        self.bring_into_view_outcome = None;
        self.bring_into_view_generation
    }

    /// Returns the nested scroll offsets used by the bring-into-view scenario.
    pub fn bring_into_view_demo_offsets(&self) -> (Point<Pixels>, Point<Pixels>) {
        (
            self.bring_into_view_outer_scroll.offset(),
            self.bring_into_view_inner_scroll.offset(),
        )
    }

    /// Returns the committed nested scroll extents used by the scenario.
    pub fn bring_into_view_demo_max_offsets(&self) -> (Point<Pixels>, Point<Pixels>) {
        (
            self.bring_into_view_outer_scroll.max_offset(),
            self.bring_into_view_inner_scroll.max_offset(),
        )
    }

    /// Returns the virtual collection offset used by the bring-into-view scenario.
    pub fn bring_into_view_demo_virtual_offset(&self) -> Point<Pixels> {
        self.bring_into_view_virtual_scroll.offset()
    }

    /// Returns the most recent application-request terminal outcome.
    pub const fn bring_into_view_demo_outcome(&self) -> Option<BringIntoViewOutcome> {
        self.bring_into_view_outcome
    }

    fn reset_bring_into_view_demo(&mut self, cx: &mut Context<Self>) {
        self.begin_bring_into_view_demo_operation();
        self.bring_into_view_outer_scroll
            .set_offset(point(px(0.0), px(0.0)));
        self.bring_into_view_inner_scroll
            .set_offset(point(px(0.0), px(0.0)));
        self.bring_into_view_virtual_scroll
            .set_offset(point(px(0.0), px(0.0)));
        self.bring_into_view_virtual_key = None;
        cx.notify();
    }

    fn focus_bring_into_view_demo_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_bring_into_view_demo_operation();
        self.bring_into_view_focus.focus(window, cx);
    }

    fn request_bring_into_view_demo(
        &mut self,
        generation: u64,
        options: BringIntoViewOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.bring_into_view_generation != generation {
            return;
        }
        let shell = cx.entity().downgrade();
        match window.bring_into_view_with_completion(
            &self.bring_into_view_target,
            options,
            cx,
            move |outcome, _, cx| {
                shell
                    .update(cx, |this, cx| {
                        if this.bring_into_view_generation == generation {
                            this.bring_into_view_outcome = Some(outcome);
                            cx.notify();
                        }
                    })
                    .ok();
            },
        ) {
            Ok((_, subscription)) => subscription.detach(),
            Err(_) => cx.notify(),
        }
    }

    fn schedule_bring_into_view_demo(
        &mut self,
        options: BringIntoViewOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = self.begin_bring_into_view_demo_operation();
        let shell = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            shell
                .update(cx, |this, cx| {
                    if this.bring_into_view_generation == generation {
                        this.request_bring_into_view_demo(generation, options, window, cx);
                    }
                })
                .ok();
        });
        cx.notify();
    }

    fn schedule_bring_into_view_demo_virtual_target(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = self.begin_bring_into_view_demo_operation();
        self.bring_into_view_virtual_key = None;
        let shell = cx.entity().downgrade();
        window.on_next_frame(move |_, cx| {
            shell
                .update(cx, |this, cx| {
                    if this.bring_into_view_generation == generation {
                        this.bring_into_view_virtual_key = Some("virtual-target-0080".to_owned());
                        cx.notify();
                    }
                })
                .ok();
        });
        cx.notify();
    }

    fn schedule_animated_bring_into_view_demo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let transition = MotionTransition::duration(
            MotionIntent::Continuity,
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(400)),
            MotionEasing::EaseOut,
        );
        self.schedule_bring_into_view_demo(
            BringIntoViewOptions::nearest()
                .with_behavior(BringIntoViewBehavior::Animated(transition)),
            window,
            cx,
        );
    }

    pub(super) fn render_bring_into_view_demo(
        &self,
        snapshot: GalleryShellSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let inner_transform = SubtreeTransform::try_new(
            size(1.15, 0.9),
            point(px(0.0), px(0.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .expect("Gallery bring-into-view container transform must remain representable");
        let target_transform = SubtreeTransform::try_new(
            size(1.1, 0.85),
            point(px(6.0), px(-4.0)),
            SubtreeTransformOrigin::CENTER,
        )
        .expect("Gallery bring-into-view target transform must remain representable");

        let target = div()
            .id("bring-into-view-demo-target")
            .debug_selector(|| "gallery:bring-into-view:target".into())
            .absolute()
            .left(px(330.0))
            .top(px(270.0))
            .w(px(48.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x1f7a66))
            .bg(rgb(0xe7f3ef))
            .text_xs()
            .text_color(rgb(0x174f45))
            .focusable()
            .tab_index(0)
            .track_focus(&self.bring_into_view_focus)
            .role(open_gpui::accesskit::Role::Group)
            .aria_label("Bring into view target")
            .child("Target")
            .with_subtree_transform(target_transform)
            .track_reveal_target(&self.bring_into_view_target);

        let virtual_items = (0..100).map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("virtual-target-{index:04}"),
                format!("Virtual target {index:04}"),
            )
        });
        let mut virtual_list = VirtualizedList::new(
            "bring-into-view-demo-virtual-list",
            "Virtual targets",
            virtual_items,
        )
        .with_size(Size::Small)
        .row_height(open_gpui_ui_core::ui_px(28.0))
        .viewport_item_count(4)
        .overscan(0)
        .scroll_handle(&self.bring_into_view_virtual_scroll);
        if let Some(key) = self.bring_into_view_virtual_key.as_ref() {
            virtual_list = virtual_list.bring_key_into_view(
                key.clone(),
                BringIntoViewOptions::aligned(BringIntoViewAlignment::MinEdge),
            );
        }
        let virtual_stage = div()
            .debug_selector(|| "gallery:bring-into-view:virtual-stage".into())
            .absolute()
            .left(px(330.0))
            .top(px(430.0))
            .w(px(280.0))
            .h(px(112.0))
            .child(virtual_list);

        let inner_scrollport = div()
            .id("bring-into-view-demo-inner-scrollport")
            .debug_selector(|| "gallery:bring-into-view:inner-scrollport".into())
            .absolute()
            .left(px(350.0))
            .top(px(300.0))
            .w(px(150.0))
            .h(px(120.0))
            .overflow_scroll()
            .track_scroll(&self.bring_into_view_inner_scroll)
            .border_1()
            .border_color(rgb(0x8a5a00))
            .bg(rgb(0xfffbeb))
            .child(
                div()
                    .relative()
                    .w(px(650.0))
                    .h(px(650.0))
                    .child(target)
                    .child(virtual_stage),
            )
            .with_subtree_transform(inner_transform);

        let stage = div()
            .id("bring-into-view-demo-outer-scrollport")
            .debug_selector(|| "gallery:bring-into-view:outer-scrollport".into())
            .relative()
            .w(px(280.0))
            .h(px(190.0))
            .overflow_scroll()
            .track_scroll(&self.bring_into_view_outer_scroll)
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x9ba39a))
            .bg(rgb(0xf7f8f4))
            .child(
                div()
                    .relative()
                    .w(px(800.0))
                    .h(px(800.0))
                    .child(inner_scrollport),
            );

        let (outer_offset, inner_offset) = self.bring_into_view_demo_offsets();
        let outcome = self
            .bring_into_view_outcome
            .map(|outcome| format!("{outcome:?}"))
            .unwrap_or_else(|| "Idle".to_owned());

        div()
            .id("bring-into-view-demo")
            .debug_selector(|| "gallery:bring-into-view:demo".into())
            .flex()
            .flex_col()
            .gap_3()
            .border_t_1()
            .border_color(rgb(0xd6d8ce))
            .pt_4()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child("Bring into view"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .debug_selector(|| "gallery:bring-into-view:reset".into())
                            .child(
                                Button::new("bring-into-view-reset", "Reset")
                                    .variant(ButtonVariant::Secondary)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, _, cx| {
                                        this.reset_bring_into_view_demo(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:bring-into-view:application".into())
                            .child(
                                Button::new("bring-into-view-application", "Application")
                                    .variant(ButtonVariant::Secondary)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, window, cx| {
                                        this.schedule_bring_into_view_demo(
                                            BringIntoViewOptions::aligned(
                                                BringIntoViewAlignment::Center,
                                            ),
                                            window,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:bring-into-view:focus".into())
                            .child(
                                Button::new("bring-into-view-focus", "Focus target")
                                    .variant(ButtonVariant::Secondary)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, window, cx| {
                                        this.focus_bring_into_view_demo_target(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:bring-into-view:animate".into())
                            .child(
                                Button::new("bring-into-view-animate", "Animate")
                                    .variant(ButtonVariant::Secondary)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, window, cx| {
                                        this.schedule_animated_bring_into_view_demo(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "gallery:bring-into-view:virtual".into())
                            .child(
                                Button::new("bring-into-view-virtual", "Materialize row")
                                    .variant(ButtonVariant::Secondary)
                                    .with_size(Size::Small)
                                    .tokens(snapshot.tokens)
                                    .on_activate(cx.processor(|this, _, window, cx| {
                                        this.schedule_bring_into_view_demo_virtual_target(
                                            window, cx,
                                        );
                                    })),
                            ),
                    ),
            )
            .child(stage)
            .child(
                div()
                    .debug_selector(|| "gallery:bring-into-view:readout".into())
                    .text_xs()
                    .text_color(rgb(0x45515f))
                    .child(format!(
                        "outer ({:.1}, {:.1}) | inner ({:.1}, {:.1}) | {outcome}",
                        outer_offset.x.as_f32(),
                        outer_offset.y.as_f32(),
                        inner_offset.x.as_f32(),
                        inner_offset.y.as_f32(),
                    )),
            )
    }
}
