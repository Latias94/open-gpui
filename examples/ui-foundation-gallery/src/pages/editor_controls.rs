//! A small application composing editable properties, a controlled tree, and catalog search.

use std::{collections::BTreeSet, time::Duration};

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Subscription, Window, div, px, rgb,
};
use open_gpui_ui_components::{
    Button, ButtonVariant, ColorIntent, Combobox, ComboboxOptionDescriptor, ComboboxQueryRequest,
    ComboboxQuerySession, ComboboxResults, ComboboxSelection, NumberInput, NumberInputCancel,
    NumberInputCancelReason, NumberInputChange, NumberInputCommit, NumberInputPreview, Tree,
    TreeItemDescriptor, TreeRowAction, TreeSelection, TreeToggle,
    gpui_adapter::NumberInputController,
    theme::{ThemeContext, ThemeResolver},
};
use open_gpui_ui_core::{Sizable, semantic};

/// Page title.
pub const TITLE: &str = "Editor controls";
/// Page summary.
pub const SUMMARY: &str = "Select an object, preview its properties, and apply or discard changes.";
/// Foundation signals exercised by the application.
pub const SIGNALS: &[&str] = &[
    "Entity-owned property editing",
    "Numeric preview, commit, and cancellation",
    "Controlled Tree selection and row actions",
    "Application-owned asynchronous catalog search",
];

const MAX_ACCEPTED_WIDTH: f32 = 180.0;

#[derive(Clone, Debug, PartialEq)]
struct Properties {
    width: f32,
    material: ComboboxSelection,
}

struct Object {
    id: &'static str,
    name: &'static str,
    saved: Properties,
}

const MATERIALS: [(&str, &str, u32); 4] = [
    ("clay", "Clay", 0xc78968),
    ("slate", "Slate", 0x62778d),
    ("moss", "Moss", 0x71906a),
    ("cobalt", "Cobalt", 0x506eaf),
];

fn catalog_results(query: &str, offline: bool) -> ComboboxResults {
    if offline {
        return ComboboxResults::failed("Catalog is offline. Reconnect and retry.");
    }
    let query = query.trim().to_lowercase();
    ComboboxResults::new().options(
        MATERIALS
            .iter()
            .filter(|(_, label, _)| label.to_lowercase().contains(&query))
            .map(|(value, label, _)| ComboboxOptionDescriptor::new(*value, *label)),
    )
}

/// An ordinary application entity; the controls retain their original interaction owners.
pub struct EditorControlsPage {
    objects: [Object; 2],
    selected: usize,
    expanded: BTreeSet<String>,
    staged: Properties,
    width_input: Entity<NumberInputController>,
    edit_revision: u64,
    apply_requested: bool,
    catalog: ComboboxQuerySession,
    offline: bool,
    ignored_replies: usize,
    located: Option<String>,
    feedback: String,
    catalog_feedback: String,
    _width_observation: Subscription,
}

impl EditorControlsPage {
    /// Creates two independent objects and one persistent property editor in this window.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let objects = [
            Object {
                id: "panel",
                name: "Panel",
                saved: Properties {
                    width: 112.0,
                    material: ComboboxSelection::new("clay", "Clay"),
                },
            },
            Object {
                id: "marker",
                name: "Marker",
                saved: Properties {
                    width: 72.0,
                    material: ComboboxSelection::new("cobalt", "Cobalt"),
                },
            },
        ];
        let staged = objects[0].saved.clone();
        let width_input = cx.new(|cx| NumberInputController::new(staged.width, window, cx));
        let observation = cx.observe(&width_input, |_, _, cx| cx.notify());
        let mut page = Self {
            objects,
            selected: 0,
            expanded: BTreeSet::from(["objects".to_owned()]),
            staged,
            width_input,
            edit_revision: 0,
            apply_requested: false,
            catalog: ComboboxQuerySession::new(),
            offline: false,
            ignored_replies: 0,
            located: None,
            feedback: "Choose an object and edit its width or material.".to_owned(),
            catalog_feedback: String::new(),
            _width_observation: observation,
        };
        page.search(String::new(), cx);
        page
    }

    fn has_changes(&self, cx: &App) -> bool {
        self.staged != self.objects[self.selected].saved
            || self.width_input.read(cx).draft().is_dirty()
    }

    fn select_object(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selected) = self.objects.iter().position(|object| object.id == value) else {
            return;
        };
        if self.selected == selected {
            return;
        }
        if self.has_changes(cx) || self.apply_requested {
            self.feedback =
                "Apply or cancel your changes before choosing another object.".to_owned();
            cx.notify();
            return;
        }
        self.selected = selected;
        self.staged = self.objects[selected].saved.clone();
        self.edit_revision += 1;
        let width = self.staged.width;
        self.width_input
            .update(cx, |input, cx| input.set_value(width, window, cx));
        self.feedback = format!("Editing {}.", self.objects[selected].name);
        self.catalog.cancel();
        self.search(String::new(), cx);
    }

    fn search(&mut self, query: String, cx: &mut Context<Self>) {
        let request = self.catalog.begin(query);
        let offline = self.offline;
        let delay = if request.query().chars().count() == 1 {
            Duration::from_millis(650)
        } else {
            Duration::from_millis(120)
        };
        self.catalog_feedback = "Searching materials…".to_owned();
        cx.spawn(async move |page, cx| {
            cx.background_executor().timer(delay).await;
            let results = catalog_results(request.query(), offline);
            let _ = page.update(cx, |page, cx| page.receive_results(&request, results, cx));
        })
        .detach();
        cx.notify();
    }

    fn receive_results(
        &mut self,
        request: &ComboboxQueryRequest,
        results: ComboboxResults,
        cx: &mut Context<Self>,
    ) {
        if self.catalog.complete(request, results) {
            self.catalog_feedback = match self.catalog.results().status() {
                open_gpui_ui_components::ComboboxResultsStatus::Error(message) => message.clone(),
                _ => format!(
                    "{} materials available.",
                    self.catalog.results().options_ref().len()
                ),
            };
        } else {
            self.ignored_replies += 1;
        }
        cx.notify();
    }

    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_requested = true;
        if self.width_input.read(cx).draft().is_dirty() {
            self.width_input
                .update(cx, |input, cx| input.commit(window, cx));
        } else {
            self.save_changes(cx);
        }
        cx.notify();
    }

    fn save_changes(&mut self, cx: &mut Context<Self>) {
        let object = &mut self.objects[self.selected];
        object.saved = self.staged.clone();
        self.feedback = format!("Applied changes to {}.", object.name);
        self.apply_requested = false;
        self.edit_revision += 1;
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_requested = false;
        self.edit_revision += 1;
        self.staged = self.objects[self.selected].saved.clone();
        let width = self.staged.width;
        self.width_input
            .update(cx, |input, cx| input.set_value(width, window, cx));
        self.catalog.cancel();
        self.feedback = format!(
            "Restored saved properties for {}.",
            self.objects[self.selected].name
        );
        self.search(String::new(), cx);
    }

    fn render_tree(&self, cx: &mut Context<Self>) -> Tree {
        let owner = cx.entity().downgrade();
        let selected = self.objects[self.selected].id;
        let dirty = self.has_changes(cx);
        Tree::new(
            "editor-controls-objects",
            "Objects",
            [TreeItemDescriptor::new("objects", "Objects").children(
                self.objects
                    .iter()
                    .map(|object| TreeItemDescriptor::new(object.id, object.name)),
            )],
        )
        .small()
        .selected(Some(selected.to_owned()))
        .expanded_values(self.expanded.clone())
        .default_focused(selected)
        .on_select(cx.processor(|page, selection: TreeSelection, window, cx| {
            page.select_object(selection.value(), window, cx);
        }))
        .on_toggle(cx.processor(|page, toggle: TreeToggle, _, cx| {
            if toggle.expanded() {
                page.expanded.insert(toggle.value().to_owned());
            } else {
                page.expanded.remove(toggle.value());
            }
            cx.notify();
        }))
        .row_content(move |item, _, _| {
            let value = item.value().to_owned();
            let owner = owner.clone();
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_1().child(item.label().to_owned()))
                .when(dirty && item.value() == selected, |this| this.child("•"))
                .when(!item.has_children(), |this| {
                    this.child(TreeRowAction::new(
                        format!("locate:{value}"),
                        Button::new(format!("editor-locate:{value}"), "Locate")
                            .variant(ButtonVariant::Ghost)
                            .xsmall()
                            .on_activate(move |_, _, cx| {
                                let _ = owner.update(cx, |page, cx| {
                                    page.located = Some(value.clone());
                                    page.feedback = format!("Located {value} in the preview.");
                                    cx.notify();
                                });
                            }),
                    ))
                })
        })
        .row_appearance(|item, appearance, _, _| {
            if item.selected() {
                appearance.with_foreground(ColorIntent::new(semantic::ACCENT, 0x1f7a66))
            } else {
                appearance
            }
        })
    }

    fn render_width(&self, cx: &mut Context<Self>) -> NumberInput {
        let revision = self.edit_revision;
        let preview = cx.listener(move |page, preview: &NumberInputPreview, _, cx| {
            if page.edit_revision == revision {
                page.feedback = format!(
                    "Width preview: {}. Apply to save your changes.",
                    preview.draft().text()
                );
                cx.notify();
            }
        });
        let change = cx.listener(move |page, change: &NumberInputChange, _, cx| {
            if page.edit_revision == revision {
                if change.value() <= MAX_ACCEPTED_WIDTH {
                    page.staged.width = change.value();
                }
                cx.notify();
            }
        });
        let commit = cx.listener(move |page, commit: &NumberInputCommit, _, cx| {
            if page.edit_revision != revision {
                return;
            }
            if !commit.accepted() {
                page.apply_requested = false;
                page.feedback =
                    "Width was kept at its previous value. Choose 180 or less.".to_owned();
            } else if page.apply_requested {
                page.save_changes(cx);
                return;
            } else {
                page.feedback = "Width is staged. Apply to save or Cancel to restore.".to_owned();
            }
            cx.notify();
        });
        let cancel = cx.listener(move |page, cancel: &NumberInputCancel, _, cx| {
            if page.edit_revision == revision {
                page.apply_requested = false;
                page.feedback = match cancel.reason() {
                    NumberInputCancelReason::InvalidDraft => {
                        "Enter a complete number before applying changes."
                    }
                    _ => "Width preview canceled. Saved properties are unchanged.",
                }
                .to_owned();
                cx.notify();
            }
        });
        NumberInput::new("editor-width", "Width")
            .controller(self.width_input.clone())
            .value(self.staged.width)
            .min(0.0)
            .max(240.0)
            .step(1.0)
            .small()
            .on_preview(move |event, window, cx| preview(&event, window, cx))
            .on_change(move |event, window, cx| change(&event, window, cx))
            .on_commit(move |event, window, cx| commit(&event, window, cx))
            .on_cancel(move |event, window, cx| cancel(&event, window, cx))
    }

    fn render_material(&self, cx: &mut Context<Self>) -> Combobox {
        let revision = self.edit_revision;
        Combobox::new("editor-material", "Material")
            .small()
            .placeholder("Search materials")
            .query(self.catalog.query().to_owned())
            .selected_item(Some(self.staged.material.clone()))
            .results(self.catalog.results().clone())
            .on_query_change(cx.processor(move |page, query: String, _, cx| {
                if page.edit_revision == revision {
                    page.search(query, cx);
                }
            }))
            .on_select(
                cx.processor(move |page, selection: ComboboxSelection, _, cx| {
                    if page.edit_revision == revision {
                        page.staged.material = selection;
                        page.feedback =
                            "Material is staged. Apply to save or Cancel to restore.".to_owned();
                        cx.notify();
                    }
                }),
            )
            .option_content(|option, _, _| {
                let color = MATERIALS
                    .iter()
                    .find(|(value, _, _)| *value == option.value())
                    .map(|(_, _, color)| *color)
                    .unwrap_or(0x808080);
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(24.0)).rounded_md().bg(rgb(color)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(option.label().to_owned())
                            .child(
                                div()
                                    .text_xs()
                                    .child(format!("Material · {}", option.value())),
                            ),
                    )
            })
            .results_content(|state, listbox, _, _| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{} matches", state.filtered_option_count())),
                    )
                    .child(listbox)
            })
    }

    fn render_preview(&self, theme: &ThemeContext, cx: &App) -> AnyElement {
        let draft = self.width_input.read(cx).draft();
        let preview_width = if draft.is_dirty() {
            draft.parsed().value().unwrap_or(self.staged.width)
        } else {
            self.staged.width
        };
        card("preview", theme)
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Preview"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .children(self.objects.iter().enumerate().map(|(index, object)| {
                        let selected = self.selected == index;
                        let properties = if selected {
                            &self.staged
                        } else {
                            &object.saved
                        };
                        let width = if selected {
                            preview_width
                        } else {
                            properties.width
                        };
                        let color = MATERIALS
                            .iter()
                            .find(|(value, _, _)| *value == properties.material.value())
                            .map(|(_, _, color)| *color)
                            .unwrap_or(0x62778d);
                        let id = object.id;
                        div()
                            .id(format!("editor-preview:{id}"))
                            .debug_selector(move || format!("gallery:editor-controls:preview:{id}"))
                            .w(px(260.0))
                            .h(px(104.0))
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .border_1()
                            .border_color(theme.resolve(ColorIntent::new(
                                if selected || self.located.as_deref() == Some(id) {
                                    semantic::ACCENT
                                } else {
                                    semantic::BORDER
                                },
                                0xd6d8ce,
                            )))
                            .rounded_sm()
                            .child(
                                div()
                                    .text_xs()
                                    .child(format!("{} · {:.0} px", object.name, width)),
                            )
                            .child(
                                div()
                                    .w(px(width.clamp(0.0, 240.0)))
                                    .h(px(32.0))
                                    .rounded_sm()
                                    .bg(rgb(color)),
                            )
                    })),
            )
            .into_any_element()
    }
}

fn card(id: &'static str, theme: &ThemeContext) -> open_gpui::Stateful<open_gpui::Div> {
    div()
        .id(format!("editor-card:{id}"))
        .debug_selector(move || format!("gallery:editor-controls:{id}"))
        .min_w(px(0.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(theme.resolve(ColorIntent::new(semantic::BORDER, 0xd6d8ce)))
        .bg(theme.resolve(ColorIntent::new(semantic::SURFACE, 0xffffff)))
        .text_color(theme.resolve(ColorIntent::new(semantic::TEXT, 0x2f3845)))
}

impl Render for EditorControlsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let object = &self.objects[self.selected];
        let changed = self.has_changes(cx);
        let composing = self
            .width_input
            .read(cx)
            .text_input()
            .read(cx)
            .marked_range_utf16()
            .is_some();
        let draft_text = self.width_input.read(cx).draft().text().to_owned();
        div()
            .id("gallery-editor-controls-page")
            .debug_selector(|| "gallery:editor-controls:page".into())
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_sm().child("Edit a width or choose a material. Apply saves the object; Cancel restores its saved properties."))
            .child(
                div().flex().flex_wrap().items_start().gap_4()
                    .child(
                        card("objects", &theme)
                            .w(px(240.0))
                            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Objects"))
                            .child(div().h(px(176.0)).child(self.render_tree(cx)))
                            .child(div().text_xs().child("Apply or cancel edits before switching objects.")),
                    )
                    .child(
                        card("inspector", &theme)
                            .flex_1()
                            .min_w(px(280.0))
                            .gap_3()
                            .child(div().text_lg().font_weight(FontWeight::BOLD).child(object.name))
                            .child(div().text_xs().child(if changed { "Unsaved changes" } else { "All changes saved" }))
                            .child(div().text_sm().child("Width"))
                            .child(self.render_width(cx))
                            .child(div().text_xs().child("Type a value or drag the scrub handle. This document accepts widths up to 180 px."))
                            .child(div().text_sm().child(format!("Material · {}", self.staged.material.label())))
                            .child(self.render_material(cx))
                            .child(div().text_xs().child("Material selection stays available while another search loads."))
                            .child(
                                div().flex().gap_2()
                                    .child(Button::new("editor-apply", "Apply")
                                        .small()
                                        .disabled(!changed || composing || self.apply_requested)
                                        .on_activate(cx.processor(|page, _, window, cx| page.apply(window, cx))))
                                    .child(Button::new("editor-cancel", "Cancel")
                                        .small()
                                        .variant(ButtonVariant::Secondary)
                                        .disabled(!changed && !self.apply_requested)
                                        .on_activate(cx.processor(|page, _, window, cx| page.cancel(window, cx)))),
                            )
                            .child(div().text_xs().child(format!(
                                "Saved: {:.0} px · {}  |  Staged: {:.0} px · {}  |  Draft: {}",
                                object.saved.width, object.saved.material.label(), self.staged.width,
                                self.staged.material.label(), draft_text,
                            ))),
                    ),
            )
            .child(self.render_preview(&theme, cx))
            .child(
                card("activity", &theme)
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Activity"))
                    .child(div().text_sm().child(self.feedback.clone()))
                    .child(div().text_xs().child(format!("{} Older replies ignored: {}.", self.catalog_feedback, self.ignored_replies)))
                    .child(
                        div().flex().flex_wrap().gap_2()
                            .child(Button::new("editor-catalog-connectivity", if self.offline { "Reconnect catalog" } else { "Take catalog offline" })
                                .small()
                                .variant(ButtonVariant::Outline)
                                .on_activate(cx.processor(|page, _, _, cx| {
                                    page.offline = !page.offline;
                                    page.search(page.catalog.query().to_owned(), cx);
                                })))
                            .child(Button::new("editor-catalog-retry", "Retry search")
                                .small()
                                .variant(ButtonVariant::Ghost)
                                .on_activate(cx.processor(|page, _, _, cx| page.search(page.catalog.query().to_owned(), cx)))),
                    )
                    .child(div().text_xs().child("Try typing ‘c’, then ‘co’ quickly: the newer result arrives first. Locate highlights an object without selecting it.")),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{Focusable, TestAppContext, VisualTestContext};
    use open_gpui_ui_components::{ComboboxResultsStatus, gpui_adapter::init_text_input};

    fn open_page(cx: &mut TestAppContext) -> (Entity<EditorControlsPage>, &mut VisualTestContext) {
        cx.update(init_text_input);
        let (page, cx) = cx.add_window_view(EditorControlsPage::new);
        draw(cx);
        (page, cx)
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn click(cx: &mut VisualTestContext, selector: &str) {
        let bounds = cx
            .debug_bounds(selector)
            .expect("editor control should be visible");
        cx.simulate_click(bounds.center(), Default::default());
        draw(cx);
    }

    fn edit_width(page: &Entity<EditorControlsPage>, text: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            let input = page.read(cx).width_input.clone();
            window.focus(&input.focus_handle(cx), cx);
        });
        cx.simulate_keystrokes("ctrl-a");
        cx.simulate_input(text);
        draw(cx);
    }

    #[open_gpui::test]
    fn numeric_preview_and_selection_rejection_keep_the_inspector_on_its_object(
        cx: &mut TestAppContext,
    ) {
        let (page, cx) = open_page(cx);
        edit_width(&page, "150", cx);
        page.read_with(cx, |page, cx| {
            assert_eq!(page.objects[0].saved.width, 112.0);
            assert_eq!(page.staged.width, 112.0);
            assert_eq!(
                page.width_input.read(cx).draft().parsed().value(),
                Some(150.0)
            );
        });
        click(cx, "tree:editor-controls-objects:item:marker");
        page.read_with(cx, |page, _| {
            assert_eq!(page.selected, 0);
            assert_eq!(page.objects[0].saved.width, 112.0);
        });
        click(cx, "button:editor-apply:root");
        page.read_with(cx, |page, _| {
            assert_eq!(page.selected, 0);
            assert_eq!(page.objects[0].saved.width, 150.0);
            assert_eq!(page.objects[1].saved.width, 72.0);
        });
        click(cx, "button:editor-locate:marker:root");
        page.read_with(cx, |page, _| {
            assert_eq!(page.selected, 0);
            assert_eq!(page.located.as_deref(), Some("marker"));
        });
        click(cx, "tree:editor-controls-objects:item:marker");
        page.read_with(cx, |page, _| {
            assert_eq!(page.selected, 1);
            assert_eq!(page.staged.width, 72.0);
        });
    }

    #[open_gpui::test]
    fn cancel_and_owner_refusal_restore_preview_without_saving(cx: &mut TestAppContext) {
        let (page, cx) = open_page(cx);
        edit_width(&page, "140", cx);
        click(cx, "button:editor-cancel:root");
        page.read_with(cx, |page, cx| {
            assert_eq!(page.staged.width, 112.0);
            assert_eq!(page.objects[0].saved.width, 112.0);
            assert_eq!(page.width_input.read(cx).draft().text(), "112");
            assert!(!page.has_changes(cx));
        });
        edit_width(&page, "220", cx);
        cx.simulate_keystrokes("enter");
        draw(cx);
        page.read_with(cx, |page, cx| {
            assert_eq!(page.staged.width, 112.0);
            assert_eq!(page.objects[0].saved.width, 112.0);
            assert_eq!(page.width_input.read(cx).value(), 112.0);
            assert!(page.feedback.contains("180 or less"));
        });
        edit_width(&page, "-", cx);
        click(cx, "button:editor-apply:root");
        page.read_with(cx, |page, _| {
            assert_eq!(page.objects[0].saved.width, 112.0);
            assert!(!page.apply_requested);
        });
    }

    #[open_gpui::test]
    fn catalog_accepts_the_latest_reply_and_keeps_selection_through_failure_and_retry(
        cx: &mut TestAppContext,
    ) {
        let (page, cx) = open_page(cx);
        page.update(cx, |page, cx| {
            page.search("c".to_owned(), cx);
            page.search("co".to_owned(), cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(120));
        draw(cx);
        let ignored = page.read_with(cx, |page, _| {
            assert_eq!(page.catalog.query(), "co");
            assert_eq!(page.catalog.results().options_ref().len(), 1);
            assert_eq!(page.catalog.results().options_ref()[0].value(), "cobalt");
            assert_eq!(page.staged.material.value(), "clay");
            page.ignored_replies
        });
        cx.executor().advance_clock(Duration::from_millis(650));
        draw(cx);
        page.read_with(cx, |page, _| {
            assert!(page.ignored_replies > ignored);
            assert_eq!(page.catalog.results().options_ref()[0].value(), "cobalt");
        });
        page.update(cx, |page, cx| {
            page.offline = true;
            page.search("co".to_owned(), cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(120));
        draw(cx);
        page.read_with(cx, |page, _| {
            assert!(matches!(
                page.catalog.results().status(),
                ComboboxResultsStatus::Error(_)
            ));
            assert_eq!(page.staged.material.value(), "clay");
        });
        page.update(cx, |page, cx| {
            page.offline = false;
            page.search("co".to_owned(), cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(120));
        draw(cx);
        page.read_with(cx, |page, _| {
            assert_eq!(
                page.catalog.results().status(),
                &ComboboxResultsStatus::Ready
            );
            assert_eq!(page.staged.material.value(), "clay");
        });
    }

    #[open_gpui::test]
    fn material_query_selection_and_apply_survive_object_navigation(cx: &mut TestAppContext) {
        let (page, cx) = open_page(cx);
        click(cx, "text-input:editor-material-input:root");
        cx.simulate_keystrokes("ctrl-a");
        cx.simulate_input("mo");
        draw(cx);
        cx.executor().advance_clock(Duration::from_millis(120));
        draw(cx);
        page.read_with(cx, |page, _| {
            assert_eq!(page.catalog.query(), "mo");
            assert_eq!(page.catalog.results().options_ref()[0].value(), "moss");
        });
        cx.simulate_keystrokes("down enter");
        draw(cx);
        page.read_with(cx, |page, _| {
            assert_eq!(page.staged.material.value(), "moss");
            assert_eq!(page.objects[0].saved.material.value(), "clay");
        });
        assert!(
            cx.debug_bounds("combobox:editor-material:content")
                .is_none()
        );
        click(cx, "button:editor-apply:root");
        click(cx, "tree:editor-controls-objects:item:marker");
        click(cx, "tree:editor-controls-objects:item:panel");
        page.read_with(cx, |page, cx| {
            assert_eq!(page.selected, 0);
            assert_eq!(page.objects[0].saved.material.value(), "moss");
            assert_eq!(page.staged.material.value(), "moss");
            assert_eq!(page.objects[1].saved.material.value(), "cobalt");
            assert!(!page.has_changes(cx));
        });
    }
}
