//! Components page rendering for the foundation gallery.

use crate::pages;
use crate::shell::*;
use open_gpui::prelude::*;
use open_gpui::{AnyElement, Context, IntoElement, ScrollAnchor, ScrollHandle, div, px, rgb};
use open_gpui_ui_components::*;
use open_gpui_ui_core::{Orientation, Sizable, Size, ThemeTokens};

const SWITCH_SECTION_IDS: &[&str] = &["switch", "checkbox", "radio-group", "toggle"];
const FIELD_SECTION_IDS: &[&str] = &[
    "textarea",
    "field",
    "tabs",
    "table",
    "virtualized-list",
    "signals",
];

pub(crate) struct ComponentPageAnchors {
    catalog: ScrollAnchor,
    primitives: ScrollAnchor,
    feedback: ScrollAnchor,
    state_contracts: ScrollAnchor,
    gates: ScrollAnchor,
    sidebar: ScrollAnchor,
    tree: ScrollAnchor,
    toolbar: ScrollAnchor,
    listbox: ScrollAnchor,
    select: ScrollAnchor,
    combobox: ScrollAnchor,
    command: ScrollAnchor,
    button: ScrollAnchor,
    splitter: ScrollAnchor,
    scroll_area: ScrollAnchor,
    badge: ScrollAnchor,
    switch: ScrollAnchor,
    checkbox: ScrollAnchor,
    radio_group: ScrollAnchor,
    toggle: ScrollAnchor,
    icon_button: ScrollAnchor,
    label: ScrollAnchor,
    text_input: ScrollAnchor,
    textarea: ScrollAnchor,
    field: ScrollAnchor,
    tabs: ScrollAnchor,
    table: ScrollAnchor,
    virtualized_list: ScrollAnchor,
    signals: ScrollAnchor,
}

impl ComponentPageAnchors {
    pub(crate) fn new(handle: &ScrollHandle) -> Self {
        let anchor = || ScrollAnchor::for_handle(handle.clone());

        Self {
            catalog: anchor(),
            primitives: anchor(),
            feedback: anchor(),
            state_contracts: anchor(),
            gates: anchor(),
            sidebar: anchor(),
            tree: anchor(),
            toolbar: anchor(),
            listbox: anchor(),
            select: anchor(),
            combobox: anchor(),
            command: anchor(),
            button: anchor(),
            splitter: anchor(),
            scroll_area: anchor(),
            badge: anchor(),
            switch: anchor(),
            checkbox: anchor(),
            radio_group: anchor(),
            toggle: anchor(),
            icon_button: anchor(),
            label: anchor(),
            text_input: anchor(),
            textarea: anchor(),
            field: anchor(),
            tabs: anchor(),
            table: anchor(),
            virtualized_list: anchor(),
            signals: anchor(),
        }
    }

    fn for_id(&self, id: &str) -> ScrollAnchor {
        match id {
            "catalog" => self.catalog.clone(),
            "primitives" => self.primitives.clone(),
            "feedback" => self.feedback.clone(),
            "state-contracts" => self.state_contracts.clone(),
            "gates" => self.gates.clone(),
            "sidebar" => self.sidebar.clone(),
            "tree" => self.tree.clone(),
            "toolbar" => self.toolbar.clone(),
            "listbox" => self.listbox.clone(),
            "select" => self.select.clone(),
            "combobox" => self.combobox.clone(),
            "command" => self.command.clone(),
            "button" => self.button.clone(),
            "splitter" => self.splitter.clone(),
            "scroll-area" => self.scroll_area.clone(),
            "badge" => self.badge.clone(),
            "switch" => self.switch.clone(),
            "checkbox" => self.checkbox.clone(),
            "radio-group" => self.radio_group.clone(),
            "toggle" => self.toggle.clone(),
            "icon-button" => self.icon_button.clone(),
            "label" => self.label.clone(),
            "text-input" => self.text_input.clone(),
            "textarea" => self.textarea.clone(),
            "field" => self.field.clone(),
            "tabs" => self.tabs.clone(),
            "table" => self.table.clone(),
            "virtualized-list" => self.virtualized_list.clone(),
            "signals" => self.signals.clone(),
            other => panic!("unknown Components page section id `{other}`"),
        }
    }
}

pub(crate) fn render_components_directory(
    anchors: &ComponentPageAnchors,
    snapshot: GalleryShellSnapshot,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let jumps = pages::components::COMPONENT_PAGE_JUMPS
        .iter()
        .map(|jump| {
            component_page_jump(
                jump.id,
                jump.label,
                anchors.for_id(jump.id),
                snapshot.tokens,
            )
        })
        .collect::<Vec<_>>();

    div()
        .id("gallery-components-directory")
        .debug_selector(|| "gallery:components-directory".into())
        .flex_none()
        .h(px(128.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Section directory"),
        )
        .child(component_focus_button(
            "all",
            "All components",
            snapshot.components_focus == pages::components::ComponentFocusMode::All,
            pages::components::ComponentFocusMode::All,
            snapshot.tokens,
            cx,
        ))
        .child(
            ScrollArea::new(
                "gallery-components-directory-scroll",
                div().flex().flex_wrap().gap_2().children(jumps),
            )
            .preserve_scroll(),
        )
}

pub(crate) fn render_components_page(
    shell: &GalleryShell,
    snapshot: GalleryShellSnapshot,
    anchors: &ComponentPageAnchors,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let component_catalog = pages::components::COMPONENT_CATALOG;
    let component_catalog_cards = component_catalog
        .iter()
        .map(|entry| {
            let catalog_selector = entry.catalog_selector();
            let focus = pages::components::focused_section_for_catalog_entry(entry);
            let focused = snapshot.components_focus.focused_section() == focus;
            let card = gallery_card_shell(catalog_selector.clone(), Some(catalog_selector))
                .min_w(px(180.0))
                .flex()
                .flex_col()
                .gap_1()
                .border_color(if focused {
                    rgb(0x1f7a66)
                } else {
                    rgb(0xd6d8ce)
                })
                .bg(if focused {
                    rgb(0xe8f3ef)
                } else {
                    rgb(0xffffff)
                })
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
                                .child(entry.name),
                        )
                        .child(component_catalog_status_pill(entry.status)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x5a6472))
                        .child(entry.family),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x5a6472))
                        .child(entry.display_state_label()),
                )
                .child(
                    div()
                        .text_xs()
                        .line_height(px(18.0))
                        .text_color(rgb(0x5a6472))
                        .child(entry.coverage),
                );

            match focus {
                Some(section_id) => card
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xf1f5ee)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_components_focus(
                            pages::components::ComponentFocusMode::Section(section_id),
                            cx,
                        );
                    })),
                None => card,
            }
        })
        .collect::<Vec<_>>();
    let conformance_gates = pages::components::CONFORMANCE_GATES;
    let conformance_gate_cards = conformance_gates.iter().map(|gate| {
        let gate_selector = format!("component-gate:{}", gate.id);
        gallery_card_shell(gate_selector.clone(), Some(gate_selector))
            .min_w(px(220.0))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(gate.title),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x5a6472))
                    .child(gate.summary),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x5a6472))
                    .child(gate.evidence.join(" / ")),
            )
    });
    let tabs_samples = pages::components::tabs_samples(snapshot.tokens);
    let radio_samples = pages::components::radio_group_samples(snapshot.tokens);
    let toggle_samples = pages::components::toggle_samples(snapshot.tokens);
    let toolbar_samples = pages::components::toolbar_samples(snapshot.tokens);
    let sidebar_samples = pages::components::sidebar_samples(snapshot.tokens);
    let listbox_samples = pages::components::listbox_samples(snapshot.tokens);
    let select_samples = pages::components::select_samples(snapshot.tokens);
    let combobox_samples = pages::components::combobox_samples(snapshot.tokens);
    let command_samples = pages::components::command_samples(snapshot.tokens);
    let badge_samples = pages::components::badge_samples(snapshot.tokens);
    let icon_button_samples = pages::components::icon_button_samples(snapshot.tokens);
    let separator_samples = pages::components::separator_samples(snapshot.tokens);
    let kbd_samples = pages::components::kbd_samples(snapshot.tokens);
    let progress_samples = pages::components::progress_samples(snapshot.tokens);
    let skeleton_samples = pages::components::skeleton_samples(snapshot.tokens);
    let avatar_samples = pages::components::avatar_samples(snapshot.tokens);
    let avatar_group_samples = pages::components::avatar_group_samples(snapshot.tokens);
    let status_cue_samples = pages::components::status_cue_samples(snapshot.tokens);
    let empty_state_samples = pages::components::empty_state_samples(snapshot.tokens);
    let tree_samples = pages::components::tree_samples(snapshot.tokens);
    let tree_state_contract_samples = pages::components::tree_state_contract_samples();
    let virtualized_list_state_contract_samples =
        pages::components::virtualized_list_state_contract_samples();
    let scroll_area_samples = pages::components::scroll_area_samples(snapshot.tokens);
    let splitter_samples = pages::components::splitter_samples(snapshot.tokens);
    let table_samples = pages::components::table_samples(snapshot.tokens);
    let virtualized_list_samples = pages::components::virtualized_list_samples(snapshot.tokens);
    let focus_mode = snapshot.components_focus;

    div()
        .id("gallery-components-page")
        .debug_selector(|| "gallery:components-page".into())
        .flex()
        .flex_col()
        .gap_5()
        .child(render_component_focus_mode(
            snapshot.components_focus,
            snapshot.tokens,
            cx,
        ))
        .child(
            component_page_section("catalog", anchors.catalog.clone()).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Component catalog"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .flex_wrap()
                            .children(component_catalog_cards),
                    ),
            ),
        )
        .child(
            component_page_section("primitives", anchors.primitives.clone())
                .when(!show_component_section(focus_mode, "primitives"), |this| {
                    this.hidden()
                })
                .child(component_primitive_samples_section(
                    separator_samples,
                    kbd_samples,
                    progress_samples,
                    skeleton_samples,
                    avatar_samples,
                    avatar_group_samples,
                    snapshot.tokens,
                )),
        )
        .child(
            component_page_section("feedback", anchors.feedback.clone())
                .when(!show_component_section(focus_mode, "feedback"), |this| {
                    this.hidden()
                })
                .child(component_feedback_samples_section(
                    status_cue_samples,
                    empty_state_samples,
                    snapshot.tokens,
                )),
        )
        .child(
            component_page_section("state-contracts", anchors.state_contracts.clone())
                .when(
                    !show_component_section(focus_mode, "state-contracts"),
                    |this| this.hidden(),
                )
                .child(component_state_contract_samples_section(
                    tree_state_contract_samples,
                    virtualized_list_state_contract_samples,
                )),
        )
        .child(
            component_page_section("gates", anchors.gates.clone())
                .when(!show_component_section(focus_mode, "gates"), |this| {
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
                                .child("Conformance gates"),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .flex_wrap()
                                .children(conformance_gate_cards),
                        ),
                ),
        )
        .child(
            component_page_section("sidebar", anchors.sidebar.clone())
                .when(!show_component_section(focus_mode, "sidebar"), |this| {
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
                                .child("Sidebar"),
                        )
                        .child(div().flex().gap_3().flex_wrap().children(
                            sidebar_samples.into_iter().map(|sample| {
                                let state = sample.state.clone();
                                let title = state.label().to_owned();
                                let mut sidebar = Sidebar::new(
                                    format!("component-sidebar:{}", sample.id),
                                    state.label(),
                                )
                                .side(state.side())
                                .variant(state.variant())
                                .collapse_mode(state.collapse_mode())
                                .collapsed(state.collapsed())
                                .with_size(state.size())
                                .tokens(snapshot.tokens);
                                if let Some(selected) = state.selected_value() {
                                    sidebar = sidebar.selected(selected);
                                }
                                if let Some(focused) = state.focused_value() {
                                    sidebar = sidebar.default_focused(focused);
                                }
                                for section in state.sections() {
                                    let mut sidebar_section =
                                        SidebarSection::new(section.value(), section.label());
                                    for item in state
                                        .items()
                                        .iter()
                                        .filter(|item| item.section_index() == section.index())
                                    {
                                        let mut sidebar_item =
                                            SidebarItem::new(item.value(), item.label());
                                        if let Some(icon) = item.icon_label() {
                                            sidebar_item = sidebar_item.icon(icon);
                                        }
                                        if let Some(badge) = item.badge_label() {
                                            sidebar_item = sidebar_item.badge(badge);
                                        }
                                        if let Some(action_label) = item.action_label_text() {
                                            sidebar_item = sidebar_item.action_label(action_label);
                                        }
                                        sidebar_section = sidebar_section
                                            .item(sidebar_item.disabled(item.disabled()));
                                    }
                                    sidebar = sidebar.section(sidebar_section);
                                }

                                div()
                                    .id(format!("component-sidebar-sample:{}", sample.id))
                                    .debug_selector({
                                        let debug_selector = sample.debug_selector();
                                        move || debug_selector
                                    })
                                    .w(px(360.0))
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
                                                    .child(title),
                                            )
                                            .child(label_pill(state.collapse_mode().as_str())),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x5a6472))
                                            .child(sample.summary),
                                    )
                                    .child(
                                        div()
                                            .h(px(214.0))
                                            .min_h(px(0.0))
                                            .flex()
                                            .overflow_hidden()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xe2e4dc))
                                            .bg(rgb(0xfcfcf8))
                                            .when(state.side() == SidebarSide::Right, |this| {
                                                this.justify_end()
                                            })
                                            .child(sidebar),
                                    )
                                    .child(component_sidebar_state_row(&state))
                            }),
                        )),
                ),
        )
        .child(
            component_page_section("tree", anchors.tree.clone())
                .when(!show_component_section(focus_mode, "tree"), |this| {
                    this.hidden()
                })
                .child(component_tree_samples_section(tree_samples, cx)),
        )
        .child(
            component_page_section("toolbar", anchors.toolbar.clone())
                .when(!show_component_section(focus_mode, "toolbar"), |this| {
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
                                .child("Toolbar"),
                        )
                        .child(div().flex().gap_3().flex_wrap().children(
                            toolbar_samples.into_iter().map(|sample| {
                                let state = sample.state.clone();
                                let toolbar = sample.build_toolbar(snapshot.tokens);

                                div()
                                    .id(format!("component-toolbar-sample:{}", sample.id))
                                    .debug_selector({
                                        let debug_selector = sample.debug_selector();
                                        move || debug_selector
                                    })
                                    .w(px(420.0))
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
                                                    .child(state.label().to_owned()),
                                            )
                                            .child(label_pill(match state.orientation() {
                                                Orientation::Horizontal => "horizontal",
                                                Orientation::Vertical => "vertical",
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x5a6472))
                                            .child(sample.summary),
                                    )
                                    .child(toolbar)
                                    .child(component_toolbar_state_row(&state))
                            }),
                        )),
                ),
        )
        .child(
            component_page_section("listbox", anchors.listbox.clone())
                .when(!show_component_section(focus_mode, "listbox"), |this| {
                    this.hidden()
                })
                .child(component_listbox_samples_section(
                    listbox_samples,
                    snapshot.tokens,
                )),
        )
        .child(
            component_page_section("select", anchors.select.clone())
                .when(!show_component_section(focus_mode, "select"), |this| {
                    this.hidden()
                })
                .child(component_select_samples_section(
                    select_samples,
                    snapshot.tokens,
                )),
        )
        .child(
            component_page_section("combobox", anchors.combobox.clone())
                .when(!show_component_section(focus_mode, "combobox"), |this| {
                    this.hidden()
                })
                .child(component_combobox_samples_section(
                    combobox_samples,
                    snapshot.tokens,
                )),
        )
        .child(
            component_page_section("command", anchors.command.clone())
                .when(!show_component_section(focus_mode, "command"), |this| {
                    this.hidden()
                })
                .child(component_command_samples_section(
                    command_samples,
                    snapshot.tokens,
                )),
        )
        .child(
            component_page_section("button", anchors.button.clone())
                .when(!show_component_section(focus_mode, "button"), |this| {
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
                                .child("Button"),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::button_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state;
                                        div()
                                            .id(format!("component-button-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(180.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(
                                                Button::new(
                                                    format!("component-button:{}", sample.id),
                                                    sample.label,
                                                )
                                                .variant(state.variant())
                                                .with_size(state.size())
                                                .disabled(state.disabled())
                                                .selected(state.selected())
                                                .tokens(snapshot.tokens),
                                            )
                                            .child(component_button_state_row(state))
                                    }),
                            ),
                        ),
                ),
        )
        .child(
            component_page_section("splitter", anchors.splitter.clone())
                .when(!show_component_section(focus_mode, "splitter"), |this| {
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
                                .child("Splitter"),
                        )
                        .child(div().flex().gap_3().flex_wrap().children(
                            splitter_samples.into_iter().map(|sample| {
                                let debug_selector = sample.debug_selector();
                                let state = sample.state.clone();
                                let splitter = sample.panels.into_iter().fold(
                                    Splitter::new(format!("component-splitter:{}", sample.id))
                                        .orientation(state.orientation())
                                        .with_size(state.size()),
                                    |splitter, panel| {
                                        splitter.panel(SplitterPanel::new(
                                            panel.descriptor,
                                            div()
                                                .size_full()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .bg(rgb(0xf8f9f3))
                                                .px_3()
                                                .py_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(open_gpui::FontWeight::BOLD)
                                                        .text_color(rgb(0x3f4a57))
                                                        .child(panel.title),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x5a6472))
                                                        .child(panel.body),
                                                ),
                                        ))
                                    },
                                );

                                div()
                                    .id(format!("component-splitter-sample:{}", sample.id))
                                    .debug_selector(move || debug_selector)
                                    .w(px(520.0))
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
                                                    .child(sample.title),
                                            )
                                            .child(label_pill(match state.orientation() {
                                                Orientation::Horizontal => "horizontal",
                                                Orientation::Vertical => "vertical",
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x5a6472))
                                            .child(sample.summary),
                                    )
                                    .child(
                                        div()
                                            .h(px(164.0))
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xe2e4dc))
                                            .bg(rgb(0xfcfcf8))
                                            .child(splitter),
                                    )
                                    .child(component_splitter_state_row(&state))
                            }),
                        )),
                ),
        )
        .child(
            component_page_section("scroll-area", anchors.scroll_area.clone())
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
                        .child(div().flex().gap_3().flex_wrap().children(
                            scroll_area_samples.into_iter().map(|sample| {
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
                                    .children(items.into_iter().enumerate().map(
                                        move |(index, item)| {
                                            let vertical_only = !horizontal && !two_axis;
                                            div()
                                    .id(format!(
                                        "component-scroll-area-item:{}:{}",
                                        sample_id, index
                                    ))
                                    .debug_selector(move || {
                                        format!(
                                            "gallery:component-scroll-area-item:{sample_id}:{index}"
                                        )
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
                                        },
                                    ));
                                let scroll_area = ScrollArea::new(
                                    format!("component-scroll-area:{}", sample_id),
                                    content,
                                )
                                .axis(state.axis())
                                .with_size(state.size());
                                let scroll_area = if let Some(reset_key) = state.reset_key() {
                                    scroll_area.reset_on_key(reset_key)
                                } else {
                                    scroll_area
                                };

                                div()
                                    .id(format!("component-scroll-area-sample:{}", sample_id))
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
                                    // Keep wheel gestures on scroll demos from leaking to the page shell.
                                    .on_scroll_wheel(|_, window, cx| {
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    })
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
                            }),
                        )),
                ),
        )
        .child(
            component_page_section("badge", anchors.badge.clone())
                .when(!show_component_section(focus_mode, "badge"), |this| {
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
                                .child("Badge"),
                        )
                        .child(div().flex().gap_3().flex_wrap().children(
                            badge_samples.into_iter().map(|sample| {
                                let sample_id = sample.id;
                                let debug_selector = sample.debug_selector();
                                let state = sample.state;
                                div()
                                    .id(format!("component-badge-sample:{sample_id}"))
                                    .debug_selector(move || debug_selector)
                                    .min_w(px(160.0))
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .gap_2()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0xd6d8ce))
                                    .bg(rgb(0xffffff))
                                    .p_3()
                                    .child(
                                        Badge::new(
                                            format!("component-badge:{}", sample.id),
                                            sample.label,
                                        )
                                        .variant(state.variant())
                                        .with_size(state.size())
                                        .tokens(snapshot.tokens),
                                    )
                                    .child(component_badge_state_row(state))
                            }),
                        )),
                ),
        )
        .child(
            component_page_section("switch", anchors.switch.clone())
                .when(
                    !show_component_section_group(focus_mode, SWITCH_SECTION_IDS),
                    |this| this.hidden(),
                )
                .child(
                    div()
                        .when(!show_component_section(focus_mode, "switch"), |this| {
                            this.hidden()
                        })
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(open_gpui::FontWeight::BOLD)
                                .child("Switch"),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::switch_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state;
                                        div()
                                            .id(format!("component-switch-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(200.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(
                                                Switch::new(format!(
                                                    "component-switch:{}",
                                                    sample.id
                                                ))
                                                .label(sample.label)
                                                .checked(state.checked())
                                                .disabled(state.disabled())
                                                .with_size(state.size())
                                                .tokens(snapshot.tokens),
                                            )
                                            .child(component_switch_state_row(state))
                                    }),
                            ),
                        ),
                )
                .child(
                    component_page_section("checkbox", anchors.checkbox.clone())
                        .when(!show_component_section(focus_mode, "checkbox"), |this| {
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
                                        .child("Checkbox"),
                                )
                                .child(
                                    div().flex().gap_3().flex_wrap().children(
                                        pages::components::checkbox_samples(snapshot.tokens)
                                            .into_iter()
                                            .map(|sample| {
                                                let sample_id = sample.id;
                                                let debug_selector = sample.debug_selector();
                                                let state = sample.state;
                                                div()
                                                    .id(format!(
                                                        "component-checkbox-sample:{sample_id}"
                                                    ))
                                                    .debug_selector(move || debug_selector)
                                                    .min_w(px(220.0))
                                                    .flex()
                                                    .flex_col()
                                                    .gap_2()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(rgb(0xd6d8ce))
                                                    .bg(rgb(0xffffff))
                                                    .p_3()
                                                    .child(component_checkbox(
                                                        format!("component-checkbox:{}", sample.id),
                                                        sample.label,
                                                        state,
                                                        snapshot.tokens,
                                                    ))
                                                    .child(component_checkbox_state_row(state))
                                            }),
                                    ),
                                ),
                        ),
                )
                .child(
                    component_page_section("radio-group", anchors.radio_group.clone())
                        .when(!show_component_section(focus_mode, "radio-group"), |this| {
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
                                        .child("RadioGroup"),
                                )
                                .child(div().flex().gap_3().flex_wrap().children(
                                    radio_samples.into_iter().map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state.clone();
                                        let mut radio = RadioGroup::new(format!(
                                            "component-radio:{}",
                                            sample.id
                                        ))
                                        .label(sample.title)
                                        .orientation(state.orientation())
                                        .default_selected(state.selected_value().unwrap_or("none"))
                                        .required(state.required())
                                        .disabled(state.disabled())
                                        .with_size(state.size())
                                        .tokens(snapshot.tokens);
                                        for item in state.items().iter() {
                                            radio = radio.item(
                                                RadioItem::new(item.value(), item.label())
                                                    .disabled(item.disabled()),
                                            );
                                        }

                                        div()
                                            .id(format!("component-radio-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(240.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(radio)
                                            .child(component_radio_state_row(&state))
                                    }),
                                )),
                        ),
                )
                .child(
                    component_page_section("toggle", anchors.toggle.clone())
                        .when(!show_component_section(focus_mode, "toggle"), |this| {
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
                                        .child("Toggle"),
                                )
                                .child(div().flex().gap_3().flex_wrap().children(
                                    toggle_samples.into_iter().map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state;
                                        div()
                                            .id(format!("component-toggle-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(180.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(
                                                Toggle::new(
                                                    format!("component-toggle:{}", sample.id),
                                                    sample.label,
                                                )
                                                .variant(state.variant())
                                                .pressed(state.pressed())
                                                .disabled(state.disabled())
                                                .with_size(state.size())
                                                .tokens(snapshot.tokens),
                                            )
                                            .child(component_toggle_state_row(&state))
                                    }),
                                )),
                        ),
                ),
        )
        .child(
            component_page_section("icon-button", anchors.icon_button.clone())
                .when(!show_component_section(focus_mode, "icon-button"), |this| {
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
                                .child("IconButton"),
                        )
                        .child(div().flex().gap_3().flex_wrap().children(
                            icon_button_samples.into_iter().map(|sample| {
                                let sample_id = sample.id;
                                let debug_selector = sample.debug_selector();
                                let state = sample.state;
                                let accessible_label = state.accessible_label().to_owned();
                                div()
                                    .id(format!("component-icon-button-sample:{sample_id}"))
                                    .debug_selector(move || debug_selector)
                                    .min_w(px(170.0))
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .gap_2()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0xd6d8ce))
                                    .bg(rgb(0xffffff))
                                    .p_3()
                                    .child(
                                        IconButton::new(
                                            format!("component-icon-button:{}", sample.id),
                                            sample.icon,
                                            accessible_label.clone(),
                                        )
                                        .variant(state.variant())
                                        .disabled(state.disabled())
                                        .with_size(state.size())
                                        .tokens(snapshot.tokens),
                                    )
                                    .child(component_icon_button_state_row(
                                        &accessible_label,
                                        state,
                                    ))
                            }),
                        )),
                ),
        )
        .child(
            component_page_section("label", anchors.label.clone())
                .when(!show_component_section(focus_mode, "label"), |this| {
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
                                .child("Label"),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::label_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state.clone();
                                        div()
                                            .id(format!("component-label-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(220.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(component_label(
                                                format!("component-label:{}", sample.id),
                                                &state,
                                                snapshot.tokens,
                                            ))
                                            .child(component_label_state_row(&state))
                                    }),
                            ),
                        ),
                ),
        )
        .child(
            component_page_section("text-input", anchors.text_input.clone())
                .when(!show_component_section(focus_mode, "text-input"), |this| {
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
                                .child("TextInput"),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::text_input_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state.clone();
                                        let controller = state
                                            .controller_driven()
                                            .then(|| shell.editable_text_input().clone());
                                        div()
                                            .id(format!("component-text-input-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(240.0))
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
                                                    .text_xs()
                                                    .font_weight(open_gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x3f4a57))
                                                    .child(sample.label),
                                            )
                                            .child(component_text_input(
                                                format!("component-text-input:{}", sample.id),
                                                sample.label,
                                                &state,
                                                snapshot.tokens,
                                                controller,
                                            ))
                                            .child(component_text_input_state_row(&state))
                                    }),
                            ),
                        ),
                ),
        )
        .child(
            component_page_section("textarea", anchors.textarea.clone())
                .when(!show_component_section(focus_mode, "textarea"), |this| {
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
                                .child("Textarea"),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::textarea_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state.clone();
                                        div()
                                            .id(format!("component-textarea-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(280.0))
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
                                                    .text_xs()
                                                    .font_weight(open_gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x3f4a57))
                                                    .child(sample.label),
                                            )
                                            .child(component_textarea(
                                                format!("component-textarea:{}", sample.id),
                                                sample.label,
                                                &state,
                                                snapshot.tokens,
                                            ))
                                            .child(component_textarea_state_row(&state))
                                    }),
                            ),
                        ),
                ),
        )
        .child(
            component_page_section("field", anchors.field.clone())
                .when(
                    !show_component_section_group(focus_mode, FIELD_SECTION_IDS),
                    |this| this.hidden(),
                )
                .child(
                    div()
                        .when(!show_component_section(focus_mode, "field"), |this| {
                            this.hidden()
                        })
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(open_gpui::FontWeight::BOLD)
                                .child("Field"),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::field_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let field_state = sample.state.clone();
                                        let input_state = sample.input_state.clone();
                                        div()
                                            .id(format!("component-field-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(280.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(component_field(
                                                format!("component-field:{}", sample.id),
                                                &field_state,
                                                component_text_input(
                                                    format!("component-field-input:{}", sample.id),
                                                    field_state.label(),
                                                    &input_state,
                                                    snapshot.tokens,
                                                    None,
                                                ),
                                                snapshot.tokens,
                                            ))
                                            .child(component_field_state_row(
                                                &field_state,
                                                &input_state,
                                            ))
                                    }),
                            ),
                        )
                        .child(
                            div().flex().gap_3().flex_wrap().children(
                                pages::components::field_textarea_samples(snapshot.tokens)
                                    .into_iter()
                                    .map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let field_state = sample.state.clone();
                                        let textarea_state = sample.textarea_state.clone();
                                        div()
                                            .id(format!(
                                                "component-field-textarea-sample:{sample_id}"
                                            ))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(320.0))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .p_3()
                                            .child(component_field(
                                                format!(
                                                    "component-field-textarea:{}",
                                                    sample.id
                                                ),
                                                &field_state,
                                                component_textarea(
                                                    format!(
                                                        "component-field-textarea-control:{}",
                                                        sample.id
                                                    ),
                                                    field_state.label(),
                                                    &textarea_state,
                                                    snapshot.tokens,
                                                ),
                                                snapshot.tokens,
                                            ))
                                            .child(component_field_textarea_state_row(
                                                &field_state,
                                                &textarea_state,
                                            ))
                                    }),
                            ),
                        )
                )
                .child(
                    component_page_section("tabs", anchors.tabs.clone())
                        .when(!show_component_section(focus_mode, "tabs"), |this| {
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
                                        .child("Tabs"),
                                )
                                .child(div().flex().gap_3().flex_wrap().children(
                                    tabs_samples.into_iter().map(|sample| {
                                        let debug_selector = sample.debug_selector();
                                        let state = sample.state.clone();
                                        let tabs = sample.build_tabs(snapshot.tokens);

                                        div()
                                            .id(format!("component-tabs-sample:{}", sample.id))
                                            .debug_selector(move || debug_selector)
                                            .min_w(px(360.0))
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
                                                            .font_weight(
                                                                open_gpui::FontWeight::BOLD,
                                                            )
                                                            .child(sample.title),
                                                    )
                                                    .child(label_pill(
                                                        state.activation_mode().as_str(),
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x5a6472))
                                                    .child(sample.summary),
                                            )
                                            .child(
                                                div()
                                                    .when(
                                                        state.orientation()
                                                            == Orientation::Vertical,
                                                        |this| this.h(px(240.0)).min_h(px(0.0)),
                                                    )
                                                    .overflow_hidden()
                                                    .child(tabs),
                                            )
                                            .child(component_tabs_state_row(&state))
                                    }),
                                )),
                        ),
                )
                .child(
                    component_page_section("table", anchors.table.clone())
                        .when(!show_component_section(focus_mode, "table"), |this| {
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
                                        .child("Table"),
                                )
                                .child(div().flex().gap_3().flex_wrap().children(
                                    table_samples.into_iter().map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let title = sample.title;
                                        let summary = sample.summary;
                                        let badge = sample.badge;
                                        let base_sizing = sample.state.column_sizing().clone();
                                        let current_sizing =
                                            pages::components::current_table_sample_sizing(
                                                sample_id,
                                                &base_sizing,
                                                cx,
                                            );
                                        let base_expansion = sample.state.expansion().clone();
                                        let current_expansion =
                                            pages::components::current_table_sample_expansion(
                                                sample_id,
                                                &base_expansion,
                                                cx,
                                            );
                                        let table_state =
                                            pages::components::table_sample_state_with_runtime(
                                                sample,
                                                current_sizing,
                                                current_expansion,
                                                cx,
                                        );
                                        let state_summary =
                                            sample.state_summary_for_state(&table_state);
                                        let mut table = sample.build_table_with_state(table_state);
                                        let table_plan = table.state();
                                        let global_filter_control: Option<AnyElement> =
                                            if sample_id == "filter-board" {
                                                let query = table
                                                    .table_state()
                                                    .global_filter()
                                                    .unwrap_or("")
                                                    .to_owned();
                                                let sample_id_for_global = sample_id.to_owned();
                                                let base_state = sample.state.clone();

                                                Some(
                                                    div()
                                                        .w_full()
                                                        .child(
                                                            TableGlobalFilter::new(
                                                                format!(
                                                                    "component-table-global-filter:{}",
                                                                    sample_id
                                                                ),
                                                                "Search",
                                                            )
                                                            .query(query)
                                                            .placeholder("Search board rows")
                                                            .clear_label("Clear search")
                                                            .small()
                                                            .tokens(snapshot.tokens)
                                                            .on_change(move |change, _, cx| {
                                                                pages::components::record_table_global_filter_change(
                                                                    sample_id_for_global.clone(),
                                                                    &base_state,
                                                                    &change,
                                                                    cx,
                                                                );
                                                            }),
                                                        )
                                                        .into_any_element(),
                                                )
                                            } else {
                                                None
                                            };
                                        let predicate_filter_control: Option<AnyElement> =
                                            if sample_id == "filter-board" {
                                                let name_column = TableColumnId::new("name");
                                                let (selected_operator, selected_value) = table
                                                    .table_state()
                                                    .filters()
                                                    .iter()
                                                    .find(|filter| filter.column() == &name_column)
                                                    .and_then(|filter| {
                                                        filter.text_predicate().map(
                                                            |(operator, query, _)| {
                                                                (
                                                                    TablePredicateFilterOperator::text(
                                                                        operator,
                                                                    ),
                                                                    query.to_owned(),
                                                                )
                                                            },
                                                        )
                                                    })
                                                    .unwrap_or((
                                                        TablePredicateFilterOperator::text(
                                                            TableTextFilterOperator::Contains,
                                                        ),
                                                        String::new(),
                                                    ));
                                                let sample_id_for_predicate = sample_id.to_owned();
                                                let base_state = sample.state.clone();

                                                Some(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(
                                                                    open_gpui::FontWeight::BOLD,
                                                                )
                                                                .text_color(rgb(0x3f4a57))
                                                                .child("Name"),
                                                        )
                                                        .child(
                                                            TablePredicateFilter::new(
                                                                format!(
                                                                    "component-table-predicate-filter:{}:name",
                                                                    sample_id
                                                                ),
                                                                "Name",
                                                                "name",
                                                            )
                                                            .default_operator(
                                                                TablePredicateFilterOperator::text(
                                                                    TableTextFilterOperator::Contains,
                                                                ),
                                                            )
                                                            .operator(selected_operator)
                                                            .value(selected_value)
                                                            .operators([
                                                                TablePredicateFilterOperator::text(
                                                                    TableTextFilterOperator::Contains,
                                                                ),
                                                                TablePredicateFilterOperator::text(
                                                                    TableTextFilterOperator::StartsWith,
                                                                ),
                                                                TablePredicateFilterOperator::text(
                                                                    TableTextFilterOperator::EndsWith,
                                                                ),
                                                            ])
                                                            .placeholder("Filter names")
                                                            .clear_label("Clear name")
                                                            .small()
                                                            .tokens(snapshot.tokens)
                                                            .on_change(move |change, _, cx| {
                                                                pages::components::record_table_predicate_filter_change(
                                                                    sample_id_for_predicate.clone(),
                                                                    &base_state,
                                                                    &change,
                                                                    cx,
                                                                );
                                                            }),
                                                        )
                                                        .into_any_element(),
                                                )
                                            } else {
                                                None
                                            };
                                        let faceted_filter_control: Option<AnyElement> =
                                            if sample_id == "filter-board" {
                                                let status_column = TableColumnId::new("status");
                                                let selected_values = table
                                                    .table_state()
                                                    .filters()
                                                    .iter()
                                                    .find(|filter| filter.column() == &status_column)
                                                    .and_then(|filter| filter.selected_values())
                                                    .map(|values| {
                                                        values.iter().cloned().collect::<Vec<_>>()
                                                    })
                                                    .unwrap_or_default();
                                                table_plan
                                                    .column_facet(&status_column)
                                                    .cloned()
                                                    .map(|facets| {
                                                        let sample_id_for_filter =
                                                            sample_id.to_owned();
                                                        let base_state = sample.state.clone();
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_1()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(
                                                                        open_gpui::FontWeight::BOLD,
                                                                    )
                                                                    .text_color(rgb(0x3f4a57))
                                                                    .child("Status"),
                                                            )
                                                            .child(
                                                                TableFacetedFilter::new(
                                                                    format!(
                                                                        "component-table-faceted-filter:{}:status",
                                                                        sample_id
                                                                    ),
                                                                    "Status",
                                                                    "status",
                                                                )
                                                                .facets(facets)
                                                                .selected_values(selected_values)
                                                                .default_query("")
                                                                .placeholder("Filter statuses")
                                                                .empty_label("No matching statuses")
                                                                .clear_label("Clear status")
                                                                .small()
                                                                .tokens(snapshot.tokens)
                                                                .on_change(move |change, _, cx| {
                                                                    pages::components::record_table_faceted_filter_change(
                                                                        sample_id_for_filter.clone(),
                                                                        &base_state,
                                                                        &change,
                                                                        cx,
                                                                    );
                                                                }),
                                                            )
                                                            .into_any_element()
                                                    })
                                            } else {
                                                None
                                            };
                                        let range_filter_control: Option<AnyElement> =
                                            if sample_id == "filter-board" {
                                                let score_column = TableColumnId::new("score");
                                                let (selected_min, selected_max) = table
                                                    .table_state()
                                                    .filters()
                                                    .iter()
                                                    .find(|filter| filter.column() == &score_column)
                                                    .and_then(|filter| {
                                                        filter.number_range_bounds()
                                                    })
                                                    .unwrap_or((None, None));
                                                table_plan
                                                    .column_facet(&score_column)
                                                    .cloned()
                                                    .map(|facets| {
                                                        let sample_id_for_range =
                                                            sample_id.to_owned();
                                                        let base_state = sample.state.clone();
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_1()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(
                                                                        open_gpui::FontWeight::BOLD,
                                                                    )
                                                                    .text_color(rgb(0x3f4a57))
                                                                    .child("Score"),
                                                            )
                                                            .child(
                                                                TableRangeFilter::new(
                                                                    format!(
                                                                        "component-table-range-filter:{}:score",
                                                                        sample_id
                                                                    ),
                                                                    "Score",
                                                                    "score",
                                                                )
                                                                .facets(facets)
                                                                .range(selected_min, selected_max)
                                                                .clear_label("Clear score")
                                                                .small()
                                                                .tokens(snapshot.tokens)
                                                                .on_change(move |change, _, cx| {
                                                                    pages::components::record_table_range_filter_change(
                                                                        sample_id_for_range.clone(),
                                                                        &base_state,
                                                                        &change,
                                                                        cx,
                                                                    );
                                                                }),
                                                            )
                                                            .into_any_element()
                                                    })
                                            } else {
                                                None
                                            };
                                        let column_visibility_control: Option<AnyElement> =
                                            if sample_id == "release-matrix" {
                                                let sample_id_for_visibility =
                                                    sample_id.to_owned();
                                                let base_state = table.table_state().clone();

                                                Some(
                                                    TableColumnVisibility::new(
                                                        format!(
                                                            "component-table-column-visibility:{}",
                                                            sample_id
                                                        ),
                                                        "Columns",
                                                    )
                                                    .columns(
                                                        table
                                                            .table_state()
                                                            .columns()
                                                            .iter()
                                                            .cloned(),
                                                    )
                                                    .visibility(
                                                        table
                                                            .table_state()
                                                            .column_visibility()
                                                            .clone(),
                                                    )
                                                    .default_visibility(
                                                        sample.state.column_visibility().clone(),
                                                    )
                                                    .show_all_label("Show all metrics")
                                                    .reset_label("Reset columns")
                                                    .viewport_item_count(7)
                                                    .small()
                                                    .tokens(snapshot.tokens)
                                                    .on_change(move |change, _, cx| {
                                                        pages::components::record_table_column_visibility_change(
                                                            sample_id_for_visibility.clone(),
                                                            &base_state,
                                                            &change,
                                                            cx,
                                                        );
                                                    })
                                                    .into_any_element(),
                                                )
                                            } else {
                                                None
                                            };
                                        let table_toolbar: Option<AnyElement> =
                                            if sample_id == "filter-board"
                                                || sample_id == "release-matrix"
                                            {
                                                let mut has_toolbar_controls = false;
                                                let toolbar_summary = if sample_id == "filter-board"
                                                {
                                                    format!(
                                                        "{} filtered / {} final rows",
                                                        state_summary.filtered_rows,
                                                        state_summary.final_rows
                                                    )
                                                } else {
                                                    format!(
                                                        "{} visible / {} total columns",
                                                        state_summary.aria_columns,
                                                        table.table_state().columns().len()
                                                    )
                                                };
                                                let mut toolbar = TableToolbar::new(
                                                    format!("component-table-toolbar:{sample_id}"),
                                                    if sample_id == "filter-board" {
                                                        "Table filters"
                                                    } else {
                                                        "Table columns"
                                                    },
                                                )
                                                .small()
                                                .tokens(snapshot.tokens)
                                                .summary(toolbar_summary);

                                                if let Some(control) = global_filter_control {
                                                    has_toolbar_controls = true;
                                                    toolbar = toolbar.control(control);
                                                }
                                                if let Some(control) = predicate_filter_control {
                                                    has_toolbar_controls = true;
                                                    toolbar = toolbar.secondary_control(control);
                                                }
                                                if let Some(control) = faceted_filter_control {
                                                    has_toolbar_controls = true;
                                                    toolbar = toolbar.secondary_control(control);
                                                }
                                                if let Some(control) = range_filter_control {
                                                    has_toolbar_controls = true;
                                                    toolbar = toolbar.secondary_control(control);
                                                }
                                                if let Some(control) = column_visibility_control {
                                                    has_toolbar_controls = true;
                                                    toolbar = toolbar.control(control);
                                                }

                                                has_toolbar_controls
                                                    .then(|| toolbar.into_any_element())
                                            } else {
                                                None
                                            };
                                        if sample_id == "release-resize" {
                                            let sample_id_for_resize = sample_id.to_owned();
                                            table = table.on_column_sizing_change(
                                                move |change, _, cx| {
                                                    pages::components::record_table_sizing_change(
                                                        sample_id_for_resize.clone(),
                                                        &change,
                                                        cx,
                                                    );
                                                },
                                            );
                                        }
                                        if sample_id == "release-rollup" {
                                            let sample_id_for_order = sample_id.to_owned();
                                            let base_state_for_order = sample.state.clone();
                                            table = table.on_column_order_change(
                                                move |change, _, cx| {
                                                    pages::components::record_table_column_order_change(
                                                        sample_id_for_order.clone(),
                                                        &base_state_for_order,
                                                        &change,
                                                        cx,
                                                    );
                                                },
                                            );
                                        }
                                        let sample_id_for_activation = sample_id.to_owned();
                                        table = table.on_row_activate(move |activation, _, cx| {
                                            pages::components::record_table_row_activation(
                                                sample_id_for_activation.clone(),
                                                &activation,
                                                cx,
                                            );
                                        });
                                        let sample_id_for_expansion = sample_id.to_owned();
                                        table =
                                            table.on_row_expansion_request(move |toggle, _, cx| {
                                                pages::components::record_table_expansion_request(
                                                    sample_id_for_expansion.clone(),
                                                    &base_expansion,
                                                    &toggle,
                                                    cx,
                                                );
                                            });
                                        let sample_id_for_edit = sample_id.to_owned();
                                        let base_state_for_edit = sample.state.clone();
                                        table =
                                            table.on_cell_edit_change(move |change, _, cx| {
                                                pages::components::record_table_cell_edit_change(
                                                    sample_id_for_edit.clone(),
                                                    &base_state_for_edit,
                                                    &change,
                                                    cx,
                                                );
                                            });

                                        div()
                                            .id(format!("component-table-sample:{sample_id}"))
                                            .debug_selector(move || debug_selector)
                                            .w(px(560.0))
                                            .flex_none()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .on_scroll_wheel(|_, window, cx| {
                                                window.prevent_default();
                                                cx.stop_propagation();
                                            })
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
                                                            .font_weight(
                                                                open_gpui::FontWeight::BOLD,
                                                            )
                                                            .child(title),
                                                    )
                                                    .child(label_pill(badge)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x5a6472))
                                                    .child(summary),
                                            )
                                            .when_some(table_toolbar, |this, toolbar| {
                                                this.child(toolbar)
                                            })
                                            .child(
                                                div()
                                                    .h(px(228.0))
                                                    .min_h(px(0.0))
                                                    .overflow_hidden()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(rgb(0xe2e4dc))
                                                    .bg(rgb(0xfcfcf8))
                                                    .child(table),
                                            )
                                            .child(component_table_state_row(&state_summary))
                                    }),
                                )),
                        ),
                )
                .child(
                    component_page_section("virtualized-list", anchors.virtualized_list.clone())
                        .when(
                            !show_component_section(focus_mode, "virtualized-list"),
                            |this| this.hidden(),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(open_gpui::FontWeight::BOLD)
                                        .child("VirtualizedList"),
                                )
                                .child(div().flex().gap_3().flex_wrap().children(
                                    virtualized_list_samples.into_iter().map(|sample| {
                                        let sample_id = sample.id;
                                        let debug_selector = sample.debug_selector();
                                        let title = sample.title;
                                        let summary = sample.summary;
                                        let badge = sample.badge;
                                        let state = sample.state.clone();
                                        let state_summary = sample.state_summary();
                                        let sample_id_for_activation = sample_id.to_owned();
                                        let list = sample.build_list().on_activate(
                                        move |activation, _, cx| {
                                            pages::components::record_virtualized_list_activation(
                                                sample_id_for_activation.clone(),
                                                activation.index(),
                                                cx,
                                            );
                                        },
                                    );

                                        div()
                                            .id(format!(
                                                "component-virtualized-list-sample:{sample_id}"
                                            ))
                                            .debug_selector(move || debug_selector)
                                            .w(px(420.0))
                                            .flex_none()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0xd6d8ce))
                                            .bg(rgb(0xffffff))
                                            .on_scroll_wheel(|_, window, cx| {
                                                window.prevent_default();
                                                cx.stop_propagation();
                                            })
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
                                                            .font_weight(
                                                                open_gpui::FontWeight::BOLD,
                                                            )
                                                            .child(title),
                                                    )
                                                    .child(label_pill(badge)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x5a6472))
                                                    .child(summary),
                                            )
                                            .child(
                                                div()
                                                    .h(px(224.0))
                                                    .min_h(px(0.0))
                                                    .overflow_hidden()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(rgb(0xe2e4dc))
                                                    .bg(rgb(0xfcfcf8))
                                                    .child(list),
                                            )
                                            .child(component_virtualized_list_state_row(
                                                &state_summary,
                                                &state,
                                            ))
                                    }),
                                )),
                        ),
                )
                .child(
                    component_page_section("signals", anchors.signals.clone())
                        .when(!show_component_section(focus_mode, "signals"), |this| {
                            this.hidden()
                        })
                        .child(shell.render_signal_list(snapshot.selected_page)),
                ),
        )
}

fn render_component_focus_mode(
    mode: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let label = match mode {
        pages::components::ComponentFocusMode::All => "All components".to_owned(),
        pages::components::ComponentFocusMode::Section(section) => {
            let label = pages::components::COMPONENT_PAGE_JUMPS
                .iter()
                .find(|jump| jump.id == section)
                .map(|jump| jump.label)
                .unwrap_or(section);
            format!("Focused: {label}")
        }
    };

    div()
        .id("gallery-components-focus-mode")
        .debug_selector(|| "gallery:component-focus:mode".into())
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xfcfcf8))
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child(label),
                )
                .child(
                    div().text_xs().text_color(rgb(0x5a6472)).child(
                        "Use catalog cards or section chips to inspect one component family.",
                    ),
                ),
        )
        .child(component_focus_button(
            "all-inline",
            "All components",
            mode == pages::components::ComponentFocusMode::All,
            pages::components::ComponentFocusMode::All,
            tokens,
            cx,
        ))
}

fn component_focus_button(
    id: &'static str,
    label: &'static str,
    selected: bool,
    focus: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> open_gpui::Stateful<open_gpui::Div> {
    div()
        .id(format!("gallery-components-focus:{id}"))
        .debug_selector(move || format!("gallery:component-focus:{id}"))
        .flex_none()
        .child(
            Button::new(format!("gallery-components-focus-button:{id}"), label)
                .variant(if selected {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                })
                .selected(selected)
                .with_size(Size::Small)
                .tokens(tokens)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_components_focus(focus, cx);
                })),
        )
}

fn show_component_section(mode: pages::components::ComponentFocusMode, id: &'static str) -> bool {
    mode.shows_section(id)
}

fn show_component_section_group(
    mode: pages::components::ComponentFocusMode,
    ids: &[&'static str],
) -> bool {
    mode == pages::components::ComponentFocusMode::All
        || ids.iter().any(|id| mode.shows_section(id))
}

fn component_tree_samples_section(
    tree_samples: &'static [pages::components::TreeSample],
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let sample_cards = tree_samples
        .iter()
        .map(|sample| {
            let sample_id = sample.id;
            let debug_selector = sample.debug_selector();
            let title = sample.title;
            let summary = sample.summary;
            let badge = sample.badge;
            let sample_id_for_selection = sample_id.to_owned();
            let sample_id_for_toggle = sample_id.to_owned();
            let mut tree = sample
                .build_tree_with_runtime(cx)
                .on_select(move |selection, _, cx| {
                    pages::components::record_tree_selection(
                        sample_id_for_selection.clone(),
                        selection.value().to_owned(),
                        cx,
                    );
                })
                .on_toggle(move |toggle, _, cx| {
                    pages::components::record_tree_toggle(
                        sample_id_for_toggle.clone(),
                        toggle.value().to_owned(),
                        toggle.expanded(),
                        toggle.loaded_child_count(),
                        toggle.children_load_state().as_str().to_owned(),
                        toggle.children_load_state().message().map(str::to_owned),
                        cx,
                    );
                });

            if sample.draggable {
                let sample_id_for_move = sample_id.to_owned();
                let base_items = sample.items.clone();
                tree = tree.on_move(move |tree_move, _, cx| {
                    pages::components::record_tree_move(
                        sample_id_for_move.clone(),
                        &base_items,
                        &tree_move,
                        cx,
                    );
                });
            }

            div()
                .id(format!("component-tree-sample:{sample_id}"))
                .debug_selector(move || debug_selector)
                .w(px(420.0))
                .flex_none()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0xd6d8ce))
                .bg(rgb(0xffffff))
                .on_scroll_wheel(|_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
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
                        .child(label_pill(badge)),
                )
                .child(div().text_xs().text_color(rgb(0x5a6472)).child(summary))
                .child(
                    div()
                        .h(px(240.0))
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .child(tree),
                )
                .child(component_tree_state_contract_row(&sample.current_state(cx)))
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Tree"),
        )
        .child(div().flex().gap_3().flex_wrap().children(sample_cards))
}

fn component_feedback_samples_section(
    status_cue_samples: [pages::components::StatusCueSample; 3],
    empty_state_samples: [pages::components::EmptyStateSample; 2],
    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Feedback"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(status_cue_samples.into_iter().map(|sample| {
                    let sample_id = sample.id;
                    let debug_selector = sample.debug_selector();
                    let title = sample.title;
                    let state = sample.state.clone();
                    let label = state.label().to_owned();

                    gallery_card_shell(
                        format!("component-status-cue-sample:{sample_id}"),
                        Some(debug_selector),
                    )
                    .min_w(px(260.0))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(title),
                            )
                            .child(label_pill(state.intent().as_str())),
                    )
                    .child(
                        StatusCue::new(format!("component-status-cue:{sample_id}"), label)
                            .intent(state.intent())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_status_cue_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(empty_state_samples.into_iter().map(|sample| {
                    let sample_id = sample.id;
                    let debug_selector = sample.debug_selector();
                    let title = sample.title;
                    let state = sample.state.clone();
                    let state_title = state.title().to_owned();
                    let description = state.description().map(str::to_owned);
                    let empty_state =
                        EmptyState::new(format!("component-empty-state:{sample_id}"), state_title)
                            .intent(state.intent())
                            .with_size(state.size())
                            .tokens(tokens);
                    let empty_state = match description {
                        Some(description) => empty_state.description(description),
                        None => empty_state,
                    };

                    gallery_card_shell(
                        format!("component-empty-state-sample:{sample_id}"),
                        Some(debug_selector),
                    )
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .items_stretch()
                    .gap_2()
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
                            .child(label_pill(state.intent().as_str())),
                    )
                    .child(empty_state)
                    .child(component_empty_state_state_row(&state))
                })),
        )
}

fn component_state_contract_samples_section(
    tree_samples: [pages::components::TreeStateContractSample; 1],
    virtualized_list_samples: [pages::components::VirtualizedListStateContractSample; 1],
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("State contracts"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(tree_samples.into_iter().map(|sample| {
                    let debug_selector = sample.debug_selector();
                    let state = sample.state.clone();

                    gallery_card_shell(
                        format!("component-tree-state-contract:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(520.0))
                    .flex()
                    .flex_col()
                    .gap_2()
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
                                    .child(sample.title),
                            )
                            .child(label_pill("TreeState")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(component_tree_state_contract_row(&state))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(state.items().iter().map(component_tree_item_readout)),
                    )
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(virtualized_list_samples.into_iter().map(|sample| {
                    let debug_selector = sample.debug_selector();
                    let state = sample.state.clone();
                    let scroll_strategy = sample.scroll_strategy;

                    gallery_card_shell(
                        format!("component-virtualized-list-state-contract:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(520.0))
                    .flex()
                    .flex_col()
                    .gap_2()
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
                                    .child(sample.title),
                            )
                            .child(label_pill("VirtualizedListState")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(component_virtualized_list_state_contract_row(
                        &state,
                        scroll_strategy,
                    ))
                })),
        )
}

pub(crate) fn component_status_cue_state_row(state: &StatusCueState) -> impl IntoElement {
    let metrics = state.metrics();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {:?} / {}",
            state.intent().as_str(),
            state.role(),
            state.size().as_str()
        ))
        .child(format!(
            "marker {} / gap {} / text {}",
            format_px(metrics.marker_size()),
            format_px(metrics.gap()),
            format_px(metrics.text_size())
        ))
        .child(format!("display-only {}", state.display_only()))
}

pub(crate) fn component_empty_state_state_row(state: &EmptyStateState) -> impl IntoElement {
    let metrics = state.metrics();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {:?} / {}",
            state.intent().as_str(),
            state.role(),
            state.size().as_str()
        ))
        .child(format!(
            "description {} / max {}",
            if state.description().is_some() {
                "present"
            } else {
                "none"
            },
            format_px(metrics.max_width())
        ))
        .child(format!(
            "padding {} / gap {}",
            format_px(metrics.padding()),
            format_px(metrics.gap())
        ))
}

pub(crate) fn component_tree_state_contract_row(state: &TreeState) -> impl IntoElement {
    let selected = state
        .selected_index()
        .and_then(|index| state.items().get(index))
        .map(TreeItemState::value)
        .unwrap_or("none");
    let focused = state
        .focused_index()
        .and_then(|index| state.items().get(index))
        .map(TreeItemState::value)
        .unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} visible / {} disabled / {}",
            state.items().len(),
            disabled_count,
            state.size().as_str()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "left {} / right {}",
            tree_keyboard_action_label(state.keyboard_action_for_key("left")),
            tree_keyboard_action_label(state.keyboard_action_for_key("right"))
        ))
        .child(format!(
            "enter {} / space {}",
            tree_keyboard_action_label(state.keyboard_action_for_key("enter")),
            tree_keyboard_action_label(state.keyboard_action_for_key("space"))
        ))
}

fn component_tree_item_readout(item: &TreeItemState) -> impl IntoElement {
    let position = item
        .position_in_set()
        .map(|position| format!("{position}/{}", item.size_of_set()))
        .unwrap_or_else(|| "disabled".to_owned());
    let parent = item.parent_value().unwrap_or("root");

    div()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(if item.focused() {
            rgb(0xe8f3ef)
        } else {
            rgb(0xfcfcf8)
        })
        .px_2()
        .py_1()
        .text_xs()
        .text_color(if item.disabled() {
            rgb(0x7a8492)
        } else {
            rgb(0x3f4a57)
        })
        .child(format!(
            "{}:{} / d{} / parent {} / pos {} / expanded {} / selected {}",
            item.index(),
            item.value(),
            item.depth(),
            parent,
            position,
            item.expanded(),
            item.selected()
        ))
}

pub(crate) fn component_virtualized_list_state_contract_row(
    state: &VirtualizedListState,
    scroll_strategy: VirtualizedListScrollStrategy,
) -> impl IntoElement {
    let activation = state
        .activation_for_key("enter")
        .map(|activation| activation.index().to_string())
        .unwrap_or_else(|| "none".to_owned());

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} items / active {} / selected {}",
            state.item_count(),
            optional_index_label(state.active_index()),
            optional_index_label(state.selected_index())
        ))
        .child(format!(
            "viewport {} / row {} / overscan {}",
            state.viewport_item_count(),
            format_px(state.metrics().row_height()),
            state.metrics().overscan_count()
        ))
        .child(format!(
            "home {} / end {} / pageup {} / pagedown {}",
            optional_index_label(state.navigation_target("home")),
            optional_index_label(state.navigation_target("end")),
            optional_index_label(state.navigation_target("pageup")),
            optional_index_label(state.navigation_target("pagedown"))
        ))
        .child(format!(
            "activation {} / scroll {}",
            activation,
            scroll_strategy.as_str()
        ))
}

fn optional_index_label(index: Option<usize>) -> String {
    index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "none".to_owned())
}

fn tree_keyboard_action_label(action: Option<TreeKeyboardAction>) -> String {
    match action {
        Some(TreeKeyboardAction::Focus(target)) => {
            format!("focus {}@{}", target.value(), target.index())
        }
        Some(TreeKeyboardAction::Toggle(toggle)) => {
            format!("toggle {} -> {}", toggle.value(), toggle.expanded())
        }
        Some(TreeKeyboardAction::Select(selection)) => {
            format!("select {}@{}", selection.value(), selection.index())
        }
        None => "none".to_owned(),
    }
}

pub(crate) fn component_tabs_state_row(state: &TabsState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.activation_mode().as_str(),
            state.size().as_str()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

pub(crate) fn component_table_state_row(
    summary: &super::TableSampleStateSummary,
) -> impl IntoElement {
    let mut row = div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} core / {} final / {} rendered",
            summary.core_rows, summary.final_rows, summary.rendered_rows
        ))
        .child(format!(
            "visible {}..{} / overscan {}..{}",
            summary.visible_start,
            summary.visible_end,
            summary.overscan_start,
            summary.overscan_end
        ))
        .child(format!(
            "{} columns / {} aria rows / {} selected",
            summary.aria_columns, summary.aria_rows, summary.selected_rows
        ));

    if summary.grouping_columns > 0 || summary.aggregation_count > 0 || summary.group_rows > 0 {
        row = row.child(format!(
            "grouped {} / expanded {} / groups {} / leaves {} / grouping {} / aggregates {} / custom {} / expanded inputs {}{}",
            summary.grouped_rows,
            summary.expanded_rows,
            summary.group_rows,
            summary.leaf_rows,
            summary.grouping_columns,
            summary.aggregation_count,
            summary.custom_aggregation_count,
            summary.expanded_group_inputs,
            if summary.all_rows_expanded { " all" } else { "" }
        ));
    }

    if summary.header_rows > 1 || summary.header_groups > 0 {
        row = row.child(format!(
            "headers {} / groups {} / leaves {}",
            summary.header_rows, summary.header_groups, summary.visible_leaf_columns
        ));
    }

    if summary.tree_rows > 0 {
        row = row.child(format!(
            "tree {} / branches {} / depth {} / expanded inputs {}{}{}",
            summary.tree_rows,
            summary.tree_branch_rows,
            summary.tree_depth,
            summary.expanded_tree_inputs,
            if summary.manual_expansion {
                " / manual"
            } else {
                ""
            },
            if summary.all_rows_expanded {
                " all"
            } else {
                ""
            }
        ));

        if summary.unloaded_tree_branches > 0
            || summary.loading_tree_rows > 0
            || summary.failed_tree_rows > 0
        {
            row = row.child(format!(
                "async branches unloaded {} / loading {} / failed {}",
                summary.unloaded_tree_branches, summary.loading_tree_rows, summary.failed_tree_rows
            ));
        }
    }

    if summary.manual_filtering || summary.manual_sorting || summary.manual_pagination {
        row = row.child(format!(
            "manual filter {} / sort {} / page {} / page {} size {} / total {} / pages {}",
            summary.manual_filtering,
            summary.manual_sorting,
            summary.manual_pagination,
            summary.pagination_page_index,
            summary.pagination_page_size,
            summary
                .pagination_row_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
            summary
                .pagination_page_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_owned())
        ));
    }

    if summary.facet_columns > 0 {
        row = row.child(format!(
            "facets {} columns / {} manual / status {} values total {} / score {}..{}",
            summary.facet_columns,
            summary.manual_facet_columns,
            summary.status_facet_values,
            summary.status_facet_total_count,
            summary
                .score_facet_min
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            summary
                .score_facet_max
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned())
        ));
    }

    if summary.pinned_top_rows > 0 || summary.pinned_bottom_rows > 0 {
        row = row.child(format!(
            "row pinning {}-{}-{} / {}",
            summary.pinned_top_rows,
            summary.pinned_center_rows,
            summary.pinned_bottom_rows,
            if summary.row_pinning_page_only {
                "page-only"
            } else {
                "keep-pinned"
            }
        ));
    }

    if summary.pinned_left_columns > 0 || summary.pinned_right_columns > 0 {
        row = row.child(format!(
            "pinned {}-{}-{} / widths {}-{}-{}px / {} resizable columns",
            summary.pinned_left_columns,
            summary.pinned_center_columns,
            summary.pinned_right_columns,
            summary.pinned_left_width_px,
            summary.pinned_center_width_px,
            summary.pinned_right_width_px,
            summary.resizable_columns
        ));
    } else {
        row = row.child(format!(
            "width {}px / {} resizable columns",
            summary.total_column_width_px, summary.resizable_columns
        ));
    }

    row
}

pub(crate) fn component_virtualized_list_state_row(
    summary: &super::VirtualizedListSampleStateSummary,
    state: &VirtualizedListState,
) -> impl IntoElement {
    let activation = state
        .activation_for_key("enter")
        .map(|activation| activation.index().to_string())
        .unwrap_or_else(|| "none".to_owned());

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} items / active {} / selected {}",
            summary.item_count,
            optional_index_label(summary.active_index),
            optional_index_label(summary.selected_index)
        ))
        .child(format!(
            "viewport {} / row {} / overscan {}",
            state.viewport_item_count(),
            format_px(state.metrics().row_height()),
            state.metrics().overscan_count()
        ))
        .child(format!(
            "visible {}..{} / overscan {}..{}",
            summary.visible_start,
            summary.visible_end,
            summary.overscan_start,
            summary.overscan_end
        ))
        .child(format!(
            "{} visible / {} rendered / activation {}",
            summary.visible_rows, summary.rendered_rows, activation
        ))
}

pub(crate) fn component_scroll_area_state_row(state: &ScrollAreaState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.axis().as_str(),
            state.reset_policy().as_str(),
            state.size().as_str()
        ))
        .child(format!(
            "viewport {} / scrollbar {}",
            state.viewport_id(),
            format_px(state.metrics().scrollbar_width())
        ))
        .child(format!(
            "x {} / y {}",
            if state.scrolls_x() { "scroll" } else { "clip" },
            if state.scrolls_y() { "scroll" } else { "clip" }
        ))
}

pub(crate) fn component_splitter_state_row(state: &SplitterState) -> impl IntoElement {
    let fractions = state
        .panels()
        .iter()
        .map(|panel| {
            if panel.collapsed() {
                format!("{}:{:.0}% collapsed", panel.id(), panel.fraction() * 100.0)
            } else {
                format!("{}:{:.0}%", panel.id(), panel.fraction() * 100.0)
            }
        })
        .collect::<Vec<_>>()
        .join(" / ");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} panels / {} handles",
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.panels().len(),
            state.handles().len()
        ))
        .child(fractions)
        .child(format!(
            "handle {} hit {}",
            format_px(state.metrics().handle_thickness()),
            format_px(state.metrics().handle_hit_size())
        ))
}

pub(crate) fn component_sidebar_state_row(state: &SidebarState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {} / {}",
            state.role(),
            state.side().as_str(),
            state.variant().as_str(),
            state.collapse_mode().as_str()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} sections / {} items / {} disabled / width {}",
            state.sections().len(),
            state.items().len(),
            disabled_count,
            format_px(state.metrics().resolved_width())
        ))
}

pub(crate) fn component_toolbar_state_row(state: &ToolbarState) -> impl IntoElement {
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();
    let kinds = state
        .items()
        .iter()
        .map(|item| item.kind().as_str())
        .collect::<Vec<_>>()
        .join("/");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {}",
            state.role(),
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.size().as_str()
        ))
        .child(format!("focus {}", focused))
        .child(format!(
            "{} items / {} disabled / {}",
            state.items().len(),
            disabled_count,
            kinds
        ))
}

pub(crate) fn gallery_card_shell(
    id: impl Into<open_gpui::ElementId>,
    debug_selector: Option<String>,
) -> open_gpui::Stateful<open_gpui::Div> {
    let card = div().id(id);
    let card = match debug_selector {
        Some(debug_selector) => card.debug_selector(move || debug_selector),
        None => card,
    };

    card.rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .p_3()
}
