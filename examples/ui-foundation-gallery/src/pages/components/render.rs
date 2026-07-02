//! Components page rendering for the foundation gallery.

use crate::pages;
use crate::shell::*;
use open_gpui::prelude::*;
use open_gpui::{AnyElement, Context, IntoElement, ListSizingBehavior, div, list, px, rgb};
use open_gpui_ui_components::*;
use open_gpui_ui_core::{Orientation, Sizable, Size, ThemeTokens, UiPx};

#[path = "render/families.rs"]
mod families;
#[path = "render/focus.rs"]
mod focus;
#[path = "render/readouts.rs"]
mod readouts;
#[path = "render/sections.rs"]
mod sections;
#[path = "render/support.rs"]
mod support;

use families::*;
use focus::*;
use readouts::*;
use sections::*;
use support::*;
const SWITCH_SECTION_IDS: &[&str] = &["switch", "checkbox", "radio-group", "toggle"];

const COMPONENT_PAGE_RENDER_SECTION_IDS: &[&str] = &[
    "catalog",
    "primitives",
    "feedback",
    "foundation-components",
    "state-contracts",
    "gates",
    "sidebar",
    "tree",
    "toolbar",
    "listbox",
    "select",
    "combobox",
    "command",
    "button",
    "splitter",
    "scroll-area",
    "badge",
    "switch",
    "icon-button",
    "label",
    "text-input",
    "textarea",
    "field",
    "tabs",
    "table",
    "virtualized-list",
    "signals",
];

fn component_page_render_section_id(id: &str) -> &str {
    if SWITCH_SECTION_IDS.contains(&id) {
        "switch"
    } else {
        id
    }
}

pub(crate) fn component_page_section_index(
    mode: pages::components::ComponentFocusMode,
    id: &str,
) -> Option<usize> {
    let id = component_page_render_section_id(id);
    component_page_render_sections(mode)
        .iter()
        .position(|section| section.id == id)
}

pub(crate) fn component_page_section_count(mode: pages::components::ComponentFocusMode) -> usize {
    component_page_render_sections(mode).len()
}

pub(crate) fn component_page_render_sections(
    mode: pages::components::ComponentFocusMode,
) -> Vec<pages::components::ComponentPageJump> {
    match mode {
        pages::components::ComponentFocusMode::All => COMPONENT_PAGE_RENDER_SECTION_IDS
            .iter()
            .map(|id| component_page_jump_for_id(id))
            .collect(),
        pages::components::ComponentFocusMode::Section(focused) => {
            vec![component_page_jump_for_id(
                component_page_render_section_id(focused),
            )]
        }
    }
}

fn component_page_jump_for_id(id: &str) -> pages::components::ComponentPageJump {
    pages::components::COMPONENT_PAGE_JUMPS
        .iter()
        .copied()
        .find(|jump| jump.id == id)
        .expect("render section id should have a matching Components page jump")
}

pub(crate) fn render_components_directory(
    snapshot: GalleryShellSnapshot,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let jumps = pages::components::COMPONENT_PAGE_JUMPS
        .iter()
        .map(|section| component_page_jump(*section, snapshot.tokens, cx))
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
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let sections = component_page_render_sections(snapshot.components_focus);
    let list_state = shell.components_list_state().clone();

    return div()
        .id("gallery-components-page")
        .debug_selector(|| "gallery:components-page".into())
        .size_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .id("gallery-page-scroll-viewport")
                .debug_selector(|| "scroll-area:gallery-page-scroll-viewport".into())
                .size_full()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(
                    list(
                        list_state,
                        cx.processor(move |this, index, _window, cx| {
                            let Some(section) = sections.get(index).copied() else {
                                return div().h_0().into_any_element();
                            };
                            render_components_section(this, section, snapshot, cx)
                        }),
                    )
                    .with_sizing_behavior(ListSizingBehavior::Auto)
                    .size_full(),
                ),
        );
}
