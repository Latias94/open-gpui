use open_gpui::{
    Bounds, Entity, MouseButton, Pixels, ScrollDelta, ScrollWheelEvent, VisualTestContext,
    accesskit, point, px, size,
};
use open_gpui_command::{
    CommandKeyBindingPatchOperation, CommandKeyBindingPatchOutcome, CommandProviderState,
};
use open_gpui_ui_components::component_contract::{
    SurfaceDocsStatus, SurfaceGalleryStatus, component_contract_entry, gallery_surface_rows,
    official_component_rows,
};
use open_gpui_ui_components::{
    AlertDialogIntent, AlertDialogOpenMode, BadgeVariant, ButtonVariant, ComboboxOpenMode,
    CommandIndexSnapshotMode, CommandKeyBindingEditorFilterMode, CommandOpenMode,
    CommandSelectionMode, CommandStatusIntent, DialogOpenMode, FeedbackIntent, HoverCardOpenIntent,
    HoverCardOpenMode, MenuItemKind, MenuOpenMode, OverlayResolvedState, PopoverOpenMode,
    ScrollAreaAxis, ScrollResetPolicy, SelectOpenMode, SheetCloseAffordance, SheetModalMode,
    SheetOpenMode, SheetSide, TableColumnOrderChange, TableGlobalFilterChange,
    TablePredicateFilterChange, TablePredicateFilterOperator, TableRangeFilterChange,
    TextInputDisplayMode, ThemeMode, ToggleVariant, TooltipOpenIntent, TreeKeyboardAction,
    VirtualizedListRowMeasureMode, VirtualizedListScrollStrategy,
    gpui_adapter::{
        DEFAULT_OVERLAY_SAFE_MARGIN, OverlayLayerPhase, OverlayLayerSnapshot, WindowOverlayRuntime,
        WindowOverlaySnapshot, default_deferred_priority, gpui_overlay_state, init_text_input,
    },
};
use open_gpui_ui_core::{
    AccessibleAction, Density, DeviceAdaptiveClass, DeviceShellMode, EscapeKeyPolicy,
    FocusRestoreIntent, InitialFocusIntent, Orientation, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, PanelAdaptiveClass, Role, Size,
    TableCellEditor, TableCellValue, TableColumnFacets, TableColumnId, TableColumnRegion,
    TableColumnWidthPolicy, TableExpansionMode, TableExpansionState, TableRowChildrenLoadState,
    TableRowId, TableRowRegion, TableStageMode, TableTextFilterOperator, ThemeTokens, Toggled,
    semantic, ui_point, ui_px,
};
use open_gpui_ui_foundation_gallery::{
    DEFAULT_GALLERY_WIDTH, GALLERY_SECTIONS, GalleryPage, GalleryShell, GalleryShellSnapshot,
    StoryContract, StoryProbeOperation, foundation_snapshot, pages,
};
use std::time::Duration;

open_gpui::actions!(foundation_gallery_shortcut_test, [DisplayShortcutCommand]);

fn display_shortcut(keystrokes: &str) -> String {
    open_gpui::KeyBinding::new(keystrokes, DisplayShortcutCommand, None)
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
}

fn advance_and_redraw(cx: &mut VisualTestContext, duration: Duration) {
    cx.executor().advance_clock(duration);
    redraw(cx);
}

fn set_short_gallery_viewport(cx: &mut VisualTestContext) {
    cx.simulate_resize(size(px(1040.0), px(520.0)));
    redraw(cx);
}

fn open_gallery_page(
    cx: &mut open_gpui::TestAppContext,
    page: GalleryPage,
) -> &mut VisualTestContext {
    let (_, cx) = open_gallery_page_with_shell(cx, page);
    cx
}

fn open_gallery_page_with_shell(
    cx: &mut open_gpui::TestAppContext,
    page: GalleryPage,
) -> (Entity<GalleryShell>, &mut VisualTestContext) {
    cx.update(init_text_input);
    let (shell, cx) = cx.add_window_view(|_, cx| GalleryShell::with_selected_page(page, cx));
    set_short_gallery_viewport(cx);
    redraw(cx);
    (shell, cx)
}

fn open_components_gallery(cx: &mut open_gpui::TestAppContext) -> &mut VisualTestContext {
    open_gallery_page(cx, GalleryPage::Components)
}

fn open_overlay_gallery(cx: &mut open_gpui::TestAppContext) -> &mut VisualTestContext {
    open_gallery_page(cx, GalleryPage::Overlay)
}

fn shell_snapshot(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
) -> GalleryShellSnapshot {
    cx.update(|_, app| shell.read(app).snapshot())
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("expected debug selector `{selector}` to be rendered"))
}

fn table_sample<'a>(
    samples: &'a [pages::components::TableSample],
    id: &str,
) -> &'a pages::components::TableSample {
    samples
        .iter()
        .find(|sample| sample.id == id)
        .unwrap_or_else(|| panic!("expected table sample `{id}`"))
}

fn text_facet_counts(facet: &TableColumnFacets) -> Vec<(String, usize)> {
    facet
        .unique_values()
        .iter()
        .map(|entry| (entry.value().filter_text(), entry.count()))
        .collect()
}

fn facet_total_count(facet: &TableColumnFacets) -> usize {
    facet
        .unique_values()
        .iter()
        .map(|entry| entry.count())
        .sum()
}

fn component_story_contract(name: &str) -> StoryContract {
    pages::components::component_story_contract_for(name)
        .unwrap_or_else(|| panic!("expected component story contract `{name}`"))
}

fn overlay_story_contract(name: &str) -> StoryContract {
    pages::overlay::overlay_story_contracts()
        .into_iter()
        .find(|story| story.owner_name() == name)
        .unwrap_or_else(|| panic!("expected overlay story contract `{name}`"))
}

struct StoryRuntimeProbe<'a> {
    cx: &'a mut VisualTestContext,
}

impl<'a> StoryRuntimeProbe<'a> {
    fn new(cx: &'a mut VisualTestContext) -> Self {
        Self { cx }
    }

    fn overlay_snapshot(&mut self) -> WindowOverlaySnapshot {
        self.cx.update(|window, cx| {
            WindowOverlayRuntime::for_window(window, cx)
                .snapshot(window, cx)
                .expect("gallery overlay runtime snapshot should resolve")
        })
    }

    fn overlay_layer(&mut self, id: &str) -> OverlayLayerSnapshot {
        self.overlay_snapshot()
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == id)
            .cloned()
            .unwrap_or_else(|| panic!("expected gallery overlay runtime layer `{id}`"))
    }

    fn render_bounds(&mut self, selector: &str) -> Bounds<Pixels> {
        bounds(self.cx, selector)
    }

    fn assert_rendered(&mut self, selector: &str, label: &str) -> Bounds<Pixels> {
        let rendered = self.render_bounds(selector);
        assert!(
            rendered.size.width > px(0.0) && rendered.size.height > px(0.0),
            "expected story probe target `{label}` to render selector `{selector}`"
        );
        rendered
    }

    fn assert_not_rendered(&mut self, selector: &str, label: &str) {
        assert!(
            self.cx.debug_bounds(selector).is_none(),
            "expected story probe target `{label}` to hide selector `{selector}`"
        );
    }

    fn assert_focused(&mut self, selector: &str, label: &str) {
        assert!(
            self.cx.debug_selector_is_focused(selector),
            "expected story probe target `{label}` to focus selector `{selector}`; focused={:?}",
            self.cx.focused_debug_selector()
        );
    }

    fn scroll_page_to(&mut self, selector: &str) -> Bounds<Pixels> {
        scroll_page_until_visible(self.cx, selector)
    }

    fn click(&mut self, selector: &str) {
        click(self.cx, selector);
    }

    fn settle(&mut self) {
        settle(self.cx);
    }

    fn drain_next_frame(&mut self) {
        self.cx.update(|window, cx| {
            window.drain_next_frame_callbacks_for_test(cx);
            window.draw(cx).clear();
        });
        self.cx.run_until_parked();
    }

    fn redraw(&mut self) {
        redraw(self.cx);
    }

    fn click_point(&mut self, position: open_gpui::Point<Pixels>) {
        click_point(self.cx, position);
    }

    fn right_click(&mut self, selector: &str) {
        right_click(self.cx, selector);
    }

    fn move_mouse_to(&mut self, selector: &str) {
        let target = self.render_bounds(selector).center();
        self.cx
            .simulate_mouse_move(target, MouseButton::Left, Default::default());
        self.redraw();
    }

    fn move_mouse_to_point(&mut self, position: open_gpui::Point<Pixels>) {
        self.cx
            .simulate_mouse_move(position, MouseButton::Left, Default::default());
        self.redraw();
    }

    fn press_escape(&mut self) {
        press_escape(self.cx);
    }

    fn assert_story_can(&self, story: &StoryContract, operation: StoryProbeOperation) {
        assert!(
            story.has_operation(operation),
            "story `{}` should declare `{}` probe support",
            story.owner_name(),
            operation.as_str()
        );
    }

    fn outside_gallery_point(&mut self) -> open_gpui::Point<Pixels> {
        self.render_bounds("gallery:content").center()
    }
}

fn scroll_until_visible(
    cx: &mut VisualTestContext,
    viewport_selector: &str,
    selector: &str,
    attempts: usize,
    delta: open_gpui::Point<Pixels>,
    scroll_position: open_gpui::Point<Pixels>,
    is_visible: impl Fn(Bounds<Pixels>, Bounds<Pixels>) -> bool,
    failure_message: String,
) -> Bounds<Pixels> {
    let scroll_bounds = bounds(cx, viewport_selector);

    for _ in 0..attempts {
        if let Some(target) = cx.debug_bounds(selector) {
            if is_visible(scroll_bounds, target) {
                return target;
            }
        }

        cx.simulate_event(ScrollWheelEvent {
            position: scroll_position,
            delta: ScrollDelta::Pixels(delta),
            ..Default::default()
        });
        redraw(cx);
    }

    panic!("{failure_message}");
}

fn scroll_page_until_visible(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    let scroll_bounds = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    scroll_until_visible(
        cx,
        "scroll-area:gallery-page-scroll-viewport",
        selector,
        240,
        point(px(0.0), px(-160.0)),
        point(
            scroll_bounds.right() - px(6.0),
            scroll_bounds.top() + px(18.0),
        ),
        |container, target| container.contains(&target.center()),
        format!("expected `{selector}` to become visible after scrolling the gallery page"),
    )
}

fn scroll_page_selector_into_view(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    selector: &str,
) -> Bounds<Pixels> {
    for _ in 0..96 {
        let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
        if let Some(target) = cx.debug_bounds(selector) {
            if target_visible_for_interaction(viewport, target) {
                return target;
            }

            if shell_snapshot(shell, cx).selected_page == GalleryPage::Components {
                let delta = if target.top() < viewport.top() {
                    target.top() - viewport.top() - px(12.0)
                } else {
                    target.bottom() - viewport.bottom() + px(12.0)
                };
                cx.update(|_, app| {
                    shell.read(app).components_list_state().scroll_by(delta);
                });
            } else {
                let handle = cx.update(|_, app| shell.read(app).page_scroll_handle().clone());
                let delta = target.top() - viewport.top() - px(24.0);
                let offset = handle.offset();
                handle.set_offset(point(offset.x, offset.y - delta));
            }
            redraw(cx);
            continue;
        }

        if shell_snapshot(shell, cx).selected_page == GalleryPage::Components {
            cx.update(|_, app| {
                shell.read(app).components_list_state().scroll_by(px(160.0));
            });
        } else {
            let handle = cx.update(|_, app| shell.read(app).page_scroll_handle().clone());
            let offset = handle.offset();
            handle.set_offset(point(offset.x, offset.y - px(160.0)));
            cx.simulate_event(ScrollWheelEvent {
                position: point(viewport.right() - px(6.0), viewport.center().y),
                delta: ScrollDelta::Pixels(point(px(0.0), px(-160.0))),
                ..Default::default()
            });
        }
        redraw(cx);
    }

    let target = bounds(cx, selector);
    let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    assert!(
        target_visible_for_interaction(viewport, target),
        "expected `{selector}` to be visible after scrolling the gallery page viewport; viewport={viewport:?} target={target:?}"
    );
    target
}

fn target_visible_for_interaction(container: Bounds<Pixels>, target: Bounds<Pixels>) -> bool {
    if target.size.height <= container.size.height && target.size.width <= container.size.width {
        container.contains(&target.center())
    } else {
        bounds_overlap_y(container, target)
    }
}

fn bounds_overlap_y(container: Bounds<Pixels>, target: Bounds<Pixels>) -> bool {
    target.bottom() >= container.top() && target.top() <= container.bottom()
}

fn visible_page_interaction_point(
    cx: &mut VisualTestContext,
    selector: &str,
) -> open_gpui::Point<Pixels> {
    let target = bounds(cx, selector);
    let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    let left = target.left().max(viewport.left());
    let right = target.right().min(viewport.right());
    let top = target.top().max(viewport.top());
    let bottom = target.bottom().min(viewport.bottom());

    assert!(
        left < right && top < bottom,
        "expected `{selector}` to overlap the visible gallery page viewport; target={target:?} viewport={viewport:?}"
    );

    point(left + (right - left) * 0.5, top + (bottom - top) * 0.5)
}

fn scroll_navigation_until_visible(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    let scroll_bounds = bounds(cx, "gallery:navigation-scroll");
    scroll_until_visible(
        cx,
        "gallery:navigation-scroll",
        selector,
        12,
        point(px(0.0), px(-120.0)),
        point(
            scroll_bounds.right() - px(4.0),
            scroll_bounds.bottom() - px(8.0),
        ),
        |container, target| container.contains(&target.center()),
        format!("expected `{selector}` to become visible after scrolling gallery navigation"),
    )
}

fn jump_components_directory_to(cx: &mut VisualTestContext, jump_selector: &str) {
    let directory_center = bounds(cx, "scroll-area:gallery-components-directory-scroll").center();
    scroll_until_visible(
        cx,
        "scroll-area:gallery-components-directory-scroll",
        jump_selector,
        32,
        point(px(0.0), px(-48.0)),
        directory_center,
        |container, target| container.contains(&target.center()),
        format!("expected the Components directory jump `{jump_selector}` to become visible"),
    );
    click(cx, jump_selector);
    settle(cx);
    settle(cx);
}

fn focus_components_catalog_entry(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    entry: &pages::components::ComponentCatalogEntry,
) -> pages::components::ComponentFocusMode {
    let story = component_story_contract(entry.name);
    let catalog_selector = story.selectors().catalog_selector().unwrap_or_else(|| {
        panic!(
            "expected story `{}` to declare a catalog selector",
            entry.name
        )
    });
    let focus = story
        .section_id()
        .unwrap_or_else(|| panic!("expected focusable catalog entry `{}`", entry.name));
    let expected_focus = pages::components::ComponentFocusMode::Section(focus);

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.scroll_page_to(catalog_selector);
        probe.click(catalog_selector);
        probe.settle();
    }

    assert_eq!(
        shell_snapshot(shell, cx).components_focus,
        expected_focus,
        "expected catalog card `{}` to enter focused mode",
        entry.name
    );

    let focus_selector = story.selectors().primary_selector().unwrap_or_else(|| {
        panic!(
            "expected focused selector for catalog entry `{}`",
            entry.name
        )
    });
    let section_selector = format!("gallery:components-section:{focus}");

    assert!(
        cx.debug_bounds(section_selector.as_str()).is_some(),
        "expected focused catalog entry `{}` to render section `{section_selector}`",
        entry.name
    );
    assert!(
        cx.debug_bounds(focus_selector).is_some(),
        "expected focused catalog entry `{}` to render selector `{focus_selector}`",
        entry.name
    );
    assert!(
        cx.debug_bounds("gallery:components-directory").is_some(),
        "expected focused catalog entry `{}` to keep the section directory available",
        entry.name
    );

    expected_focus
}

fn focus_components_section(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    entry: &pages::components::ComponentCatalogEntry,
) -> pages::components::ComponentFocusMode {
    let story = component_story_contract(entry.name);
    let focus = story
        .section_id()
        .unwrap_or_else(|| panic!("expected focusable catalog entry `{}`", entry.name));
    let expected_focus = pages::components::ComponentFocusMode::Section(focus);

    cx.update(|_, app| {
        shell.update(app, |shell, cx| {
            shell.set_components_focus(expected_focus, cx);
        });
    });
    settle(cx);

    assert_eq!(
        shell_snapshot(shell, cx).components_focus,
        expected_focus,
        "expected catalog entry `{}` to enter focused mode",
        entry.name
    );

    let focus_selector = story.selectors().primary_selector().unwrap_or_else(|| {
        panic!(
            "expected focused selector for catalog entry `{}`",
            entry.name
        )
    });
    let section_selector = format!("gallery:components-section:{focus}");

    assert!(
        cx.debug_bounds(section_selector.as_str()).is_some(),
        "expected focused catalog entry `{}` to render section `{section_selector}`",
        entry.name
    );
    assert!(
        cx.debug_bounds(focus_selector).is_some(),
        "expected focused catalog entry `{}` to render selector `{focus_selector}`",
        entry.name
    );
    assert!(
        cx.debug_bounds("gallery:components-directory").is_some(),
        "expected focused catalog entry `{}` to keep the section directory available",
        entry.name
    );

    expected_focus
}

fn drag(
    cx: &mut VisualTestContext,
    start: open_gpui::Point<Pixels>,
    end: open_gpui::Point<Pixels>,
) {
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(
            start.x + (end.x - start.x) * 0.1,
            start.y + (end.y - start.y) * 0.1,
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(
            start.x + (end.x - start.x) * 0.35,
            start.y + (end.y - start.y) * 0.35,
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.run_until_parked();
    redraw(cx);
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let target = bounds(cx, selector).center();
    cx.simulate_click(target, Default::default());
    redraw(cx);
}

fn right_click(cx: &mut VisualTestContext, selector: &str) {
    let target = bounds(cx, selector).center();
    cx.simulate_mouse_down(target, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Right, Default::default());
    cx.run_until_parked();
    redraw(cx);
}

fn click_point(cx: &mut VisualTestContext, point: open_gpui::Point<Pixels>) {
    cx.simulate_click(point, Default::default());
    redraw(cx);
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    redraw(cx);
}

fn visible_outside_point(
    container: Bounds<Pixels>,
    excluded: Bounds<Pixels>,
) -> open_gpui::Point<Pixels> {
    let inset = px(24.0);
    let candidates = [
        point(container.left() + inset, container.top() + inset),
        point(container.right() - inset, container.top() + inset),
        point(container.left() + inset, container.bottom() - inset),
        point(container.right() - inset, container.bottom() - inset),
        container.center(),
    ];

    candidates
        .into_iter()
        .find(|candidate| container.contains(candidate) && !excluded.contains(candidate))
        .unwrap_or_else(|| {
            panic!("expected visible outside press point in `{container:?}` outside `{excluded:?}`")
        })
}

fn press_escape(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("escape");
    settle(cx);
}

fn outside_top_left(layer: Bounds<Pixels>) -> open_gpui::Point<Pixels> {
    point(layer.left() + px(12.0), layer.top() + px(12.0))
}

#[path = "foundation_gallery/component_catalog_contracts.rs"]
mod component_catalog_contracts;
#[path = "foundation_gallery/component_sample_contracts.rs"]
mod component_sample_contracts;
#[path = "foundation_gallery/component_smoke_navigation.rs"]
mod component_smoke_navigation;
#[path = "foundation_gallery/component_smoke_shell.rs"]
mod component_smoke_shell;
#[path = "foundation_gallery/component_smoke_table_interactions.rs"]
mod component_smoke_table_interactions;
#[path = "foundation_gallery/component_smoke_table_models.rs"]
mod component_smoke_table_models;
#[path = "foundation_gallery/component_smoke_tree_virtualized.rs"]
mod component_smoke_tree_virtualized;
#[path = "foundation_gallery/devtools_contracts.rs"]
mod devtools_contracts;
#[path = "foundation_gallery/focus_a11y_smoke.rs"]
mod focus_a11y_smoke;
#[path = "foundation_gallery/foundation_contracts.rs"]
mod foundation_contracts;
#[path = "foundation_gallery/overlay_contracts.rs"]
mod overlay_contracts;
#[path = "foundation_gallery/overlay_smoke.rs"]
mod overlay_smoke;
