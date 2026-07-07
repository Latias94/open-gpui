use super::*;

pub(super) fn render_component_scroll_area_section(
    focus_mode: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
) -> AnyElement {
    component_page_section("scroll-area")
        .when(!show_component_section(focus_mode, "scroll-area"), |this| {
            this.hidden()
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("ScrollArea"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::scroll_area_samples(tokens)
                            .into_iter()
                            .map(|sample| render_scroll_area_sample(sample)),
                    ),
                ),
        )
        .into_any_element()
}

fn render_scroll_area_sample(sample: pages::components::ScrollAreaSample) -> impl IntoElement {
    let sample_id = sample.id;
    let debug_selector = sample.debug_selector();
    let title = sample.title;
    let summary = sample.summary;
    let items = sample.items;
    let state = sample.state.clone();
    let horizontal = state.axis() == ScrollAreaAxis::Horizontal;
    let two_axis = state.axis() == ScrollAreaAxis::Both;
    let content = div()
        .when(horizontal, |this| this.flex().gap_2().min_w(px(860.0)))
        .when(two_axis, |this| {
            this.flex().flex_col().gap_1().min_w(px(860.0))
        })
        .when(!horizontal && !two_axis, |this| {
            this.flex().flex_col().gap_1()
        })
        .children(items.into_iter().enumerate().map(move |(index, item)| {
            let vertical_only = !horizontal && !two_axis;
            div()
                .id(format!("component-scroll-area-item:{sample_id}:{index}"))
                .debug_selector(move || {
                    format!("gallery:component-scroll-area-item:{sample_id}:{index}")
                })
                .when(horizontal, |this| this.w(px(132.0)).min_h(px(88.0)))
                .when(two_axis, |this| this.w(px(1240.0)).min_h(px(88.0)))
                .when(vertical_only, |this| this.min_h(px(28.0)))
                .rounded_sm()
                .border_1()
                .border_color(rgb(0xd6d8ce))
                .bg(rgb(0xf8f9f3))
                .px_3()
                .py_2()
                .text_xs()
                .text_color(rgb(0x3f4a57))
                .child(item)
        }));
    let scroll_area = ScrollArea::new(format!("component-scroll-area:{sample_id}"), content)
        .axis(state.axis())
        .with_size(state.size());
    let scroll_area = if let Some(reset_key) = state.reset_key() {
        scroll_area.reset_on_key(reset_key)
    } else {
        scroll_area
    };

    div()
        .id(format!("component-scroll-area-sample:{sample_id}"))
        .debug_selector(move || debug_selector)
        .w(px(360.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .on_scroll_wheel(|_, _, _| open_gpui::ScrollWheelIntent::handled().stop_propagation())
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
                        .child(title),
                )
                .child(label_pill(state.axis().as_str())),
        )
        .child(div().text_xs().text_color(rgb(0x5a6472)).child(summary))
        .child(
            div()
                .w(px(360.0))
                .h(px(154.0))
                .min_h(px(0.0))
                .overflow_hidden()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0xe2e4dc))
                .bg(rgb(0xfcfcf8))
                .child(scroll_area),
        )
        .child(component_scroll_area_state_row(&state))
}
