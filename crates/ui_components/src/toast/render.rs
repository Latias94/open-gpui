use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, SemanticDescriptor, ui_px};

use super::{ToastDismissReason, ToastIntent, ToastStack};
use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationKeyPolicy};
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;

impl RenderOnce for ToastStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let stack_metrics = state.metrics();
        let overflow_colors = ThemeResolver::feedback_colors(state.tokens, ToastIntent::Neutral);
        let stack_id = self.id.clone();
        let debug_id = self.id.to_string();
        let on_action = self.on_action.clone();
        let on_dismiss = self.on_dismiss.clone();
        let action_activation_handles = self.action_activation_handles;
        let dismiss_activation_handles = self.dismiss_activation_handles;
        let label = self.label;
        let stack_semantics = SemanticDescriptor::new(state.role()).with_label(label.as_ref());

        div()
            .id(self.id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("toast-stack:{debug_id}:root")
            })
            .ui_semantics(&stack_semantics)
            .flex()
            .flex_col()
            .items_end()
            .gap(gpui_px_from_ui(stack_metrics.gap()))
            .children(state.visible_toasts().iter().map(|toast| {
                let metrics = toast.metrics();
                let colors = toast.colors();
                let focus_ring = toast.focus_ring();
                let action_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
                let dismiss_focus_shadow = action_focus_shadow.clone();
                let action = toast.action();
                let dismiss = toast.dismiss(ToastDismissReason::Manual);
                let on_action = on_action.clone();
                let on_dismiss = on_dismiss.clone();
                let toast_id = toast.id().to_owned();
                let action_activation_handle =
                    action_activation_handles.get(&toast_id).cloned();
                let dismiss_activation_handle =
                    dismiss_activation_handles.get(&toast_id).cloned();
                let title = toast.title().to_owned();
                let description = toast.description().map(str::to_owned);
                let action_label = toast.action_label().map(str::to_owned);
                let toast_semantics = SemanticDescriptor::new(toast.role()).with_label(&title);
                let action_actions: &[AccessibleAction] = if on_action.is_some() {
                    &[AccessibleAction::Click, AccessibleAction::Focus]
                } else {
                    &[AccessibleAction::Focus]
                };
                let dismiss_actions: &[AccessibleAction] = if on_dismiss.is_some() {
                    &[AccessibleAction::Click, AccessibleAction::Focus]
                } else {
                    &[AccessibleAction::Focus]
                };

                div()
                    .id(format!("toast:{toast_id}"))
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        let toast_id = toast_id.clone();
                        move || format!("toast-stack:{debug_id}:toast:{toast_id}")
                    })
                    .min_w(gpui_px_from_ui(metrics.min_width()))
                    .max_w(gpui_px_from_ui(metrics.max_width()))
                    .p(gpui_px_from_ui(metrics.padding()))
                    .flex()
                    .items_start()
                    .gap(gpui_px_from_ui(metrics.gap()))
                    .rounded(gpui_px_from_ui(metrics.radius()))
                    .border_1()
                    .border_color(theme.resolve(colors.border()))
                    .bg(theme.resolve(colors.background()))
                    .text_color(theme.resolve(colors.foreground()))
                    .ui_semantics(&toast_semantics)
                    .child(
                        div()
                            .mt(gpui_px_from_ui(ui_px(3.0)))
                            .w(gpui_px_from_ui(metrics.marker_size()))
                            .h(gpui_px_from_ui(metrics.marker_size()))
                            .rounded(gpui_px_from_ui(ui_px(999.0)))
                            .bg(theme.resolve(colors.marker())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(gpui_px_from_ui(metrics.title_size()))
                                    .line_height(gpui_px_from_ui(metrics.title_size()))
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(title),
                            )
                            .when_some(description, |this, description| {
                                this.child(
                                    div()
                                        .text_color(theme.resolve(colors.muted_foreground()))
                                        .text_size(gpui_px_from_ui(metrics.description_size()))
                                        .line_height(gpui_px_from_ui(metrics.description_size()))
                                        .child(description),
                                )
                            })
                            .when_some(action_label.zip(action), |this, (label, action)| {
                                let action_debug_id = debug_id.clone();
                                let action_debug_toast_id = toast_id.clone();
                                let action_semantics = SemanticDescriptor::new(toast.action_role())
                                    .with_label(&label)
                                    .with_actions(action_actions);
                                this.child(
                                    div()
                                        .id(format!("toast-action:{toast_id}"))
                                        .debug_selector(move || {
                                            format!(
                                                "toast-stack:{action_debug_id}:toast:{action_debug_toast_id}:action"
                                            )
                                        })
                                        .min_h(gpui_px_from_ui(metrics.action_height()))
                                        .self_start()
                                        .px(gpui_px_from_ui(ui_px(8.0)))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(gpui_px_from_ui(metrics.radius()))
                                        .border_1()
                                        .border_color(theme.resolve(colors.border()))
                                        .focusable()
                                        .tab_stop(true)
                                        .ui_semantics(&action_semantics)
                                        .focus_visible(move |style| {
                                            style.shadow(action_focus_shadow.clone())
                                        })
                                        .cursor_pointer()
                                        .when_some(on_action.clone(), |this, on_action| {
                                            ActivationBinding::new(
                                                window,
                                                cx,
                                                (
                                                    stack_id.clone(),
                                                    format!("toast:{toast_id}:action-activation"),
                                                ),
                                                true,
                                                ActivationKeyPolicy::EnterOrSpace,
                                                move |activation, window, cx| {
                                                    on_action(
                                                        action.clone(),
                                                        activation,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )
                                            .with_programmatic_handle(
                                                action_activation_handle.clone(),
                                            )
                                            .bind(this)
                                        })
                                        .child(label),
                                )
                            }),
                    )
                    .when_some(dismiss, |this, dismiss| {
                        let dismiss_debug_id = debug_id.clone();
                        let dismiss_debug_toast_id = toast_id.clone();
                        let dismiss_semantics = SemanticDescriptor::new(toast.dismiss_role())
                            .with_label("Dismiss notification")
                            .with_actions(dismiss_actions);
                        this.child(
                            div()
                                .id(format!("toast-dismiss:{toast_id}"))
                                .debug_selector(move || {
                                    format!(
                                        "toast-stack:{dismiss_debug_id}:toast:{dismiss_debug_toast_id}:dismiss"
                                    )
                                })
                                .size(gpui_px_from_ui(metrics.dismiss_size()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(gpui_px_from_ui(metrics.radius()))
                                .focusable()
                                .tab_stop(true)
                                .ui_semantics(&dismiss_semantics)
                                .focus_visible(move |style| {
                                    style.shadow(dismiss_focus_shadow.clone())
                                })
                                .cursor_pointer()
                                .when_some(on_dismiss.clone(), |this, on_dismiss| {
                                    ActivationBinding::new(
                                        window,
                                        cx,
                                        (
                                            stack_id.clone(),
                                            format!("toast:{toast_id}:dismiss-activation"),
                                        ),
                                        true,
                                        ActivationKeyPolicy::EnterOrSpace,
                                        move |activation, window, cx| {
                                            on_dismiss(dismiss.clone(), activation, window, cx);
                                        },
                                    )
                                    .with_programmatic_handle(
                                        dismiss_activation_handle.clone(),
                                    )
                                    .bind(this)
                                })
                                .child("x"),
                        )
                    })
                    .into_any_element()
            }))
            .when(state.overflow_count() > 0, |this| {
                this.child(
                    div()
                        .text_size(gpui_px_from_ui(stack_metrics.description_size()))
                        .text_color(theme.resolve(overflow_colors.muted_foreground()))
                        .child(format!("+{} more", state.overflow_count())),
                )
            })
    }
}
