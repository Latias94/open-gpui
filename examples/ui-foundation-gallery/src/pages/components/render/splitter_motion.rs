use super::*;

pub(super) struct SplitterMotionDemo {
    three_panes: bool,
    summary_collapsed: bool,
    navigator: Entity<SplitterMotionPanel>,
    editor: Entity<SplitterMotionPanel>,
    inspector: Entity<SplitterMotionPanel>,
    summary: Entity<SplitterMotionPanel>,
    details: Entity<SplitterMotionPanel>,
}

impl SplitterMotionDemo {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            three_panes: true,
            summary_collapsed: false,
            navigator: cx.new(|_| {
                SplitterMotionPanel::new(
                    "motion-nav",
                    "Navigator",
                    "Project",
                    "Files, symbols, and filters stay rendered while the layout changes.",
                    SplitterMotionPanelTone::Mint,
                )
            }),
            editor: cx.new(|_| {
                SplitterMotionPanel::new(
                    "motion-editor",
                    "Editor",
                    "Primary",
                    "The main surface stretches without replacing its view identity.",
                    SplitterMotionPanelTone::Ink,
                )
            }),
            inspector: cx.new(|_| {
                SplitterMotionPanel::new(
                    "motion-inspector",
                    "Inspector",
                    "Auxiliary",
                    "Remove and restore this pane to inspect retained-view transitions.",
                    SplitterMotionPanelTone::Amber,
                )
            }),
            summary: cx.new(|_| {
                SplitterMotionPanel::new(
                    "motion-summary",
                    "Summary",
                    "Collapsible",
                    "Collapse this header and watch the retained pane animate to its rail.",
                    SplitterMotionPanelTone::Mint,
                )
            }),
            details: cx.new(|_| {
                SplitterMotionPanel::new(
                    "motion-details",
                    "Details",
                    "Fill",
                    "The remaining panel absorbs committed layout changes.",
                    SplitterMotionPanelTone::Ink,
                )
            }),
        }
    }

    fn workspace_splitter(&self) -> Splitter {
        let mut splitter = Splitter::new("component-splitter:motion-workspace")
            .horizontal()
            .with_size(Size::Medium)
            .motion_preference(MotionPreference::Animated)
            .panel(SplitterPanel::view(
                SplitterPanelDescriptor::new("motion-nav", 0.24)
                    .min_fraction(0.16)
                    .max_fraction(0.34),
                self.navigator.clone(),
            ))
            .panel(SplitterPanel::view(
                SplitterPanelDescriptor::new(
                    "motion-editor",
                    if self.three_panes { 0.54 } else { 0.76 },
                )
                .min_fraction(0.42)
                .max_fraction(0.78),
                self.editor.clone(),
            ));

        if self.three_panes {
            splitter = splitter.panel(SplitterPanel::view(
                SplitterPanelDescriptor::new("motion-inspector", 0.22)
                    .min_fraction(0.14)
                    .max_fraction(0.3),
                self.inspector.clone(),
            ));
        }

        splitter
    }

    fn collapse_splitter(&self) -> Splitter {
        Splitter::new("component-splitter:motion-collapse")
            .vertical()
            .with_size(Size::Small)
            .motion_preference(MotionPreference::Animated)
            .panel(SplitterPanel::view(
                SplitterPanelDescriptor::new("motion-summary", 0.34)
                    .min_fraction(0.12)
                    .max_fraction(0.46)
                    .collapsible(true)
                    .collapsed(self.summary_collapsed)
                    .collapsed_fraction(0.08),
                self.summary.clone(),
            ))
            .panel(SplitterPanel::view(
                SplitterPanelDescriptor::new("motion-details", 0.66)
                    .min_fraction(0.42)
                    .max_fraction(0.92),
                self.details.clone(),
            ))
    }
}

impl Render for SplitterMotionDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_splitter = self.workspace_splitter();
        let workspace_state = workspace_splitter.state();
        let collapse_splitter = self.collapse_splitter();
        let collapse_state = collapse_splitter.state();
        let pane_button_label = if self.three_panes {
            "Show 2 panes"
        } else {
            "Show 3 panes"
        };
        let collapse_button_label = if self.summary_collapsed {
            "Expand summary"
        } else {
            "Collapse summary"
        };

        div()
            .id("component-splitter-motion-demo")
            .debug_selector(|| "gallery:component-splitter-motion-demo".into())
            .w(px(680.0))
            .flex()
            .flex_col()
            .gap_2()
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
                            .child("Motion preview"),
                    )
                    .child(label_pill("retained views")),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x5a6472))
                    .child(
                        "Toggles exercise SplitterPanel::view insert/remove and collapse transitions.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        div()
                            .id("component-splitter-motion-toggle-count-target")
                            .debug_selector(|| {
                                "gallery:component-splitter-motion-toggle-count".into()
                            })
                            .child(
                                Button::new(
                                    "component-splitter-motion-toggle-count",
                                    pane_button_label,
                                )
                                .variant(ButtonVariant::Secondary)
                                .selected(self.three_panes)
                                .with_size(Size::Small)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.three_panes = !this.three_panes;
                                    cx.notify();
                                })),
                            ),
                    )
                    .child(
                        div()
                            .id("component-splitter-motion-toggle-collapse-target")
                            .debug_selector(|| {
                                "gallery:component-splitter-motion-toggle-collapse".into()
                            })
                            .child(
                                Button::new(
                                    "component-splitter-motion-toggle-collapse",
                                    collapse_button_label,
                                )
                                .variant(ButtonVariant::Ghost)
                                .selected(self.summary_collapsed)
                                .with_size(Size::Small)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.summary_collapsed = !this.summary_collapsed;
                                    cx.notify();
                                })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x5a6472))
                            .child(if self.three_panes {
                                "3 panes"
                            } else {
                                "2 panes"
                            }),
                    ),
            )
            .child(
                div()
                    .h(px(176.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xe2e4dc))
                    .bg(rgb(0xfcfcf8))
                    .overflow_hidden()
                    .child(workspace_splitter),
            )
            .child(component_splitter_state_row(&workspace_state))
            .child(
                div()
                    .h(px(146.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xe2e4dc))
                    .bg(rgb(0xfcfcf8))
                    .overflow_hidden()
                    .child(collapse_splitter),
            )
            .child(component_splitter_state_row(&collapse_state))
    }
}

struct SplitterMotionPanel {
    id: &'static str,
    title: &'static str,
    kicker: &'static str,
    body: &'static str,
    tone: SplitterMotionPanelTone,
}

impl SplitterMotionPanel {
    fn new(
        id: &'static str,
        title: &'static str,
        kicker: &'static str,
        body: &'static str,
        tone: SplitterMotionPanelTone,
    ) -> Self {
        Self {
            id,
            title,
            kicker,
            body,
            tone,
        }
    }
}

impl Render for SplitterMotionPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (background, accent, foreground) = self.tone.colors();
        let id = self.id;

        div()
            .id(format!("component-splitter-motion-panel:{id}"))
            .debug_selector(move || format!("gallery:component-splitter-motion-panel:{id}"))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .bg(background)
            .px_3()
            .py_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .text_color(foreground)
                                    .truncate()
                                    .child(self.title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x5a6472))
                                    .truncate()
                                    .child(self.kicker),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(accent),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x4d5968))
                    .child(self.body),
            )
    }
}

#[derive(Clone, Copy)]
enum SplitterMotionPanelTone {
    Mint,
    Ink,
    Amber,
}

impl SplitterMotionPanelTone {
    fn colors(self) -> (open_gpui::Rgba, open_gpui::Rgba, open_gpui::Rgba) {
        match self {
            Self::Mint => (rgb(0xf3faf4), rgb(0x1f7a66), rgb(0x1d4038)),
            Self::Ink => (rgb(0xf6f8fb), rgb(0x4268a8), rgb(0x20324f)),
            Self::Amber => (rgb(0xfffaf0), rgb(0xb7791f), rgb(0x533a16)),
        }
    }
}
