use open_gpui::{
    Bounds, Entity, MouseButton, Pixels, ScrollDelta, ScrollWheelEvent, VisualTestContext, point,
    px, size,
};
use open_gpui_ui_components::{
    AlertDialogIntent, AlertDialogOpenMode, BadgeVariant, ButtonVariant, ComboboxOpenMode,
    CommandIndexSnapshotMode, CommandOpenMode, CommandSelectionMode, DialogOpenMode,
    FeedbackIntent, HoverCardOpenIntent, HoverCardOpenMode, MenuItemKind, MenuOpenMode,
    OverlayResolvedState, PopoverOpenMode, ScrollAreaAxis, ScrollResetPolicy, SelectOpenMode,
    SheetCloseAffordance, SheetModalMode, SheetOpenMode, SheetSide, TableCellEditor,
    TableCellValue, TableColumnFacets, TableColumnId, TableColumnOrderChange, TableColumnRegion,
    TableExpansionMode, TableExpansionState, TableGlobalFilterChange, TablePredicateFilterChange,
    TablePredicateFilterOperator, TableRangeFilterChange, TableRowChildrenLoadState, TableRowId,
    TableRowRegion, TableStageMode, TableTextFilterOperator, TextInputDisplayMode, ThemeMode,
    ToggleVariant, TooltipOpenIntent, TreeKeyboardAction, VirtualizedListScrollStrategy,
    gpui_adapter::{
        DEFAULT_OVERLAY_SAFE_MARGIN, default_deferred_priority, gpui_overlay_state, init_text_input,
    },
};
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceShellMode, EscapeKeyPolicy, FocusRestoreIntent,
    InitialFocusIntent, Orientation, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, PanelAdaptiveClass, Role, Size,
    TableColumnWidthPolicy, ThemeTokens, Toggled, semantic, ui_point, ui_px,
};
use open_gpui_ui_foundation_gallery::{
    DEFAULT_GALLERY_WIDTH, GALLERY_SECTIONS, GalleryPage, GalleryShell, GalleryShellSnapshot,
    foundation_snapshot, pages,
};
use std::time::Duration;

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
    let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");

    for _ in 0..8 {
        if let Some(target) = cx.debug_bounds(selector) {
            if target_visible_for_interaction(viewport, target) {
                return target;
            }

            let handle = cx.update(|_, app| shell.read(app).page_scroll_handle().clone());
            let delta = target.top() - viewport.top() - px(24.0);
            let offset = handle.offset();
            handle.set_offset(point(offset.x, offset.y - delta));
            redraw(cx);
            continue;
        }

        break;
    }

    let target = bounds(cx, selector);
    let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    assert!(
        target_visible_for_interaction(viewport, target),
        "expected `{selector}` to be visible after aligning the gallery page scroll handle; viewport={viewport:?} target={target:?}"
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
    let catalog_selector = entry.catalog_selector();
    let focus = pages::components::focused_section_for_catalog_entry(entry)
        .unwrap_or_else(|| panic!("expected focusable catalog entry `{}`", entry.name));
    let expected_focus = pages::components::ComponentFocusMode::Section(focus);

    scroll_page_until_visible(cx, catalog_selector.as_str());
    click(cx, catalog_selector.as_str());
    settle(cx);

    assert_eq!(
        shell_snapshot(shell, cx).components_focus,
        expected_focus,
        "expected catalog card `{}` to enter focused mode",
        entry.name
    );

    let focus_selector = entry
        .sample_selector
        .or(entry.state_contract_selector)
        .unwrap_or_else(|| {
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
    let focus = pages::components::focused_section_for_catalog_entry(entry)
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

    let focus_selector = entry
        .sample_selector
        .or(entry.state_contract_selector)
        .unwrap_or_else(|| {
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

#[test]
fn gallery_sections_cover_the_foundation_slices() {
    let ids = GALLERY_SECTIONS
        .iter()
        .map(|section| section.id)
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "tokens",
            "sizing-density",
            "adaptive",
            "focus-a11y",
            "overlay",
            "components"
        ]
    );
}

#[test]
fn default_shell_consumes_ui_core_foundation_vocabulary() {
    let snapshot = foundation_snapshot(DEFAULT_GALLERY_WIDTH, GalleryPage::Tokens);

    assert_eq!(snapshot.selected_page, GalleryPage::Tokens);
    assert_eq!(snapshot.shell_mode, DeviceShellMode::Desktop);
    assert_eq!(snapshot.density, Density::Comfortable);
    assert_eq!(snapshot.control_size, Size::Medium);
    assert_eq!(snapshot.tokens.surface, semantic::SURFACE);
    assert_eq!(snapshot.tokens.focus_ring, semantic::FOCUS_RING);
}

#[test]
fn compact_width_uses_mobile_shell_and_compact_density() {
    let snapshot = foundation_snapshot(px(720.0), GalleryPage::Adaptive);

    assert_eq!(snapshot.selected_page, GalleryPage::Adaptive);
    assert_eq!(snapshot.shell_mode, DeviceShellMode::Mobile);
    assert_eq!(snapshot.density, Density::Compact);
    assert_eq!(snapshot.control_size, Size::Small);
}

#[test]
fn labels_are_stable_for_manual_dogfood_output() {
    assert_eq!(
        GalleryPage::from_id("components"),
        Some(GalleryPage::Components)
    );
    assert_eq!(GalleryPage::from_id("missing"), None);
    assert_eq!(DeviceShellMode::Desktop.as_str(), "desktop");
    assert_eq!(DeviceShellMode::Mobile.as_str(), "mobile");
    assert_eq!(Density::Spacious.as_str(), "spacious");
    assert_eq!(Size::XSmall.as_str(), "xs");
    assert_eq!(DeviceAdaptiveClass::Expanded.as_str(), "expanded device");
    assert_eq!(PanelAdaptiveClass::Wide.as_str(), "wide panel");
}

#[test]
fn package_manifest_stays_foundation_scoped() {
    let manifest = include_str!("../Cargo.toml");

    assert!(manifest.contains("open_gpui.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_core.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_components.workspace = true"));
    assert!(manifest.contains("open_gpui_platform.workspace = true"));
    assert!(!manifest.contains("open_gpui_canvas"));
    assert!(!manifest.contains("open_gpui_docking"));
    assert!(!manifest.contains("open_gpui_ui ="));
}

#[test]
fn productization_checkpoint_keeps_extraction_deferred_and_boundary_refs_available() {
    let workspace_manifest = include_str!("../../../Cargo.toml");
    let adr = include_str!("../../../docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md");
    let design = include_str!("../../../docs/adr/0007-open-gpui-ui-headless-boundary-design.md");
    let productization =
        include_str!("../../../docs/adr/0008-open-gpui-ui-component-productization-roadmap.md");
    let component_contract = include_str!("../../../docs/ui/component-contract.md");

    assert!(!workspace_manifest.contains("open-gpui-ui-headless"));
    assert!(!workspace_manifest.contains("open_gpui_ui_headless"));
    assert!(productization.contains("Treat the current UI crates as the product boundary"));
    assert!(productization.contains(
        "Do not create a standalone `open-gpui-ui-headless` crate in the active roadmap."
    ));
    assert!(productization.contains("This ADR does not invalidate either document."));
    assert!(adr.contains("Do **not** create `open-gpui-ui-headless` yet."));
    assert!(adr.contains("The strict UI-core boundary is clean"));
    assert!(adr.contains("ListboxState"));
    assert!(adr.contains("ComboboxState"));
    assert!(adr.contains("CommandState"));
    assert!(adr.contains("GpuiOverlayState"));
    assert!(adr.contains("TextInputController"));
    assert!(adr.contains("ADR 0007 records that design gate."));
    assert!(!adr.contains("adaptive viewport `Pixels as Px`"));
    assert!(!adr.contains("UiPx` still has GPUI style-conversion impls"));
    assert!(design.contains("This ADR is a design gate only."));
    assert!(design.contains("It does not create `open-gpui-ui-headless`"));
    assert!(design.contains("overlay policy and placement vocabulary"));
    assert!(design.contains("roving-focus navigation helpers"));
    assert!(design.contains("listbox navigation and typeahead target resolution"));
    assert!(design.contains("scroll viewport intent"));
    assert!(design.contains("splitter resize constraints"));
    assert!(design.contains("AccessKit node wiring"));
    assert!(design.contains("TextInputController"));
    assert!(design.contains("focus_ring_shadow"));
    assert!(design.contains("Interaction Ownership Matrix"));
    assert!(
        component_contract
            .contains("ADR 0008 keeps current-crate productization as the active roadmap.")
    );
    assert!(component_contract.contains("ADR 0007 records the"));
    assert!(component_contract.contains("post-boundary extraction design without creating"));
    assert!(component_contract.contains("open_gpui_ui_components::gpui_adapter"));
    assert!(component_contract.contains("TextInputController"));
    assert!(component_contract.contains("ScrollHandle"));
    assert!(component_contract.contains("focus_ring_shadow"));
}

#[test]
fn token_page_samples_follow_theme_token_order() {
    let tokens = ThemeTokens::default();
    let samples = pages::tokens::token_samples(tokens);

    assert_eq!(samples.len(), 12);
    assert_eq!(samples[0].key, semantic::SURFACE);
    assert_eq!(samples[0].preview_rgb, 0xffffff);
    assert_eq!(samples[7].key, semantic::FOCUS_RING);
    assert_eq!(samples[11].key, semantic::MODAL_OVERLAY);
    assert!(pages::tokens::matches_semantic_registry(tokens));
}

#[test]
fn token_page_exposes_runtime_theme_mode_metadata() {
    let samples = pages::tokens::theme_mode_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].mode, ThemeMode::Light);
    assert_eq!(samples[1].mode, ThemeMode::Dark);
    assert_eq!(samples[2].mode, ThemeMode::HighContrast);
    assert!(samples[0].revision < samples[1].revision);
    assert!(samples[1].revision < samples[2].revision);
    assert_ne!(samples[0].surface_rgb, samples[1].surface_rgb);
    assert_ne!(samples[1].focus_ring_rgb, samples[2].focus_ring_rgb);
}

#[test]
fn sizing_page_samples_expose_core_metrics() {
    let sizes = pages::sizing::SIZE_SAMPLES;
    let densities = pages::sizing::DENSITY_SAMPLES;

    assert_eq!(sizes[0].label, "xs");
    assert_eq!(sizes[2].button_h, ui_px(32.0));
    assert_eq!(sizes[3].icon_button_size, ui_px(36.0));
    assert_eq!(densities[0].default_size, Size::Small);
    assert_eq!(densities[2].default_size, Size::Large);
}

#[test]
fn adaptive_page_samples_cover_default_thresholds() {
    let devices = pages::adaptive::device_samples();
    let panels = pages::adaptive::panel_samples();

    assert_eq!(devices[0].shell_mode, DeviceShellMode::Mobile);
    assert_eq!(devices[0].class, DeviceAdaptiveClass::Compact);
    assert_eq!(devices[1].shell_mode, DeviceShellMode::Desktop);
    assert_eq!(devices[1].class, DeviceAdaptiveClass::Regular);
    assert_eq!(devices[2].class, DeviceAdaptiveClass::Expanded);

    assert_eq!(panels[0].class, PanelAdaptiveClass::Compact);
    assert_eq!(panels[1].class, PanelAdaptiveClass::Medium);
    assert_eq!(panels[2].class, PanelAdaptiveClass::Wide);
}

#[test]
fn focus_a11y_page_models_focus_order_and_roles() {
    let controls = pages::focus_a11y::FOCUS_CONTROLS;
    let state = pages::focus_a11y::a11y_demo_state(3, true);

    assert_eq!(controls[0].id, "focus-primary");
    assert_eq!(controls[0].tab_index, 1);
    assert_eq!(controls[1].role, Role::SpinButton);
    assert_eq!(controls[2].role, Role::Switch);
    assert_eq!(state.counter, 3);
    assert_eq!(state.toggled, Toggled::True);
    assert_eq!(state.counter_role, Role::SpinButton);
}

#[test]
fn overlay_page_geometry_prefers_visual_bounds_and_insets_window() {
    let geometry = pages::overlay::demo_geometry();

    assert_eq!(geometry.anchor_rect.size.width, ui_px(1.0));
    assert_eq!(geometry.anchor_rect.size.height, ui_px(1.0));
    assert_eq!(geometry.preferred_rect, geometry.visual_rect);
    assert_eq!(geometry.safe_window_rect.origin.x, ui_px(12.0));
    assert_eq!(geometry.safe_window_rect.origin.y, ui_px(12.0));
    assert_eq!(geometry.safe_window_rect.size.width, ui_px(616.0));
    assert_eq!(geometry.safe_window_rect.size.height, ui_px(336.0));
}

#[test]
fn overlay_page_samples_expose_behavior_contracts() {
    let samples = pages::overlay::behavior_samples();

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "tooltip");
    assert_eq!(samples[0].policy.kind(), OverlayLayerKind::Tooltip);
    let adapter = gpui_overlay_state(&OverlayResolvedState::resolve(samples[0].policy.clone()));
    assert_eq!(
        adapter.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(adapter.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
    assert_eq!(
        samples[0].policy.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert_eq!(
        samples[0].policy.escape_key_policy(),
        EscapeKeyPolicy::Ignore
    );
    assert_eq!(
        samples[0].policy.focus_restore_intent(),
        &FocusRestoreIntent::None
    );
    assert_eq!(
        samples[0].policy.initial_focus_intent(),
        &InitialFocusIntent::None
    );

    assert_eq!(samples[1].id, "popover");
    assert_eq!(
        samples[1].policy.kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        samples[1].policy.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(
        samples[1].policy.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
    assert!(samples[1].policy.layer_state().wants_outside_press());
    let adapter = gpui_overlay_state(&OverlayResolvedState::resolve(samples[1].policy.clone()));
    assert!(adapter.wants_outside_press_handler());

    assert_eq!(samples[2].id, "dialog");
    assert_eq!(samples[2].policy.kind(), OverlayLayerKind::Modal);
    assert_eq!(
        samples[2].policy.outside_press_policy(),
        OutsidePressPolicy::Consume
    );
    assert_eq!(
        samples[2].policy.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert!(samples[2].policy.layer_state().blocks_underlay_input());
    let adapter = gpui_overlay_state(&OverlayResolvedState::resolve(samples[2].policy.clone()));
    assert_eq!(
        adapter.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Modal)
    );

    assert_eq!(samples[3].id, "menu");
    assert_eq!(samples[3].policy.kind(), OverlayLayerKind::Menu);
    assert_eq!(
        samples[3].policy.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(samples[3].policy.kind().as_str(), "menu");
}

#[test]
fn overlay_page_tooltip_samples_expose_focus_hover_and_disabled_contracts() {
    let samples = pages::overlay::tooltip_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "hover-focus");
    assert_eq!(
        samples[0].state.open_intent(),
        TooltipOpenIntent::HoverOrFocus
    );
    assert!(samples[0].state.open_intent().opens_on_hover());
    assert!(samples[0].state.open_intent().opens_on_focus());
    assert!(!samples[0].state.open());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Tooltip
    );

    assert_eq!(samples[1].id, "focus-only");
    assert_eq!(samples[1].state.open_intent(), TooltipOpenIntent::Focus);
    assert!(!samples[1].state.open_intent().opens_on_hover());
    assert!(samples[1].state.open_intent().opens_on_focus());

    assert_eq!(samples[2].id, "delayed-manual");
    assert_eq!(samples[2].state.open_intent(), TooltipOpenIntent::Manual);
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.delay().open_delay(),
        std::time::Duration::from_millis(120)
    );
    assert_eq!(
        samples[2].state.delay().close_delay(),
        std::time::Duration::from_millis(40)
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
    assert!(samples[3].state.descriptive());
    assert!(!samples[3].state.interactive_content());
}

#[test]
fn overlay_page_hover_card_samples_expose_interactive_hover_contracts() {
    let samples = pages::overlay::hover_card_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].id, "profile-preview");
    assert_eq!(
        samples[0].state.open_mode(),
        HoverCardOpenMode::Uncontrolled
    );
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(
        samples[0].state.open_intent(),
        HoverCardOpenIntent::HoverOrFocus
    );
    assert!(samples[0].state.open_intent().opens_on_hover());
    assert!(samples[0].state.open_intent().opens_on_focus());
    assert!(samples[0].state.interactive_content());
    assert!(!samples[0].state.descriptive());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert!(samples[0].state.overlay().wants_outside_press_handler());
    assert_eq!(
        samples[0].state.focus_restore_intent(),
        &FocusRestoreIntent::None
    );

    assert_eq!(samples[1].id, "focus-preview");
    assert_eq!(samples[1].state.open_intent(), HoverCardOpenIntent::Focus);
    assert!(!samples[1].state.open_intent().opens_on_hover());
    assert!(samples[1].state.open_intent().opens_on_focus());
    assert_eq!(
        samples[1].state.placement_side(),
        OverlayPlacementSide::Right
    );

    assert_eq!(samples[2].id, "manual-controlled");
    assert_eq!(samples[2].state.open_mode(), HoverCardOpenMode::Controlled);
    assert_eq!(samples[2].state.open_intent(), HoverCardOpenIntent::Manual);
    assert!(!samples[2].state.open());
    assert_eq!(samples[2].state.delay().open_delay().as_millis(), 80);
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
}

#[test]
fn overlay_page_popover_samples_expose_controlled_and_dismissal_contracts() {
    let samples = pages::overlay::popover_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "default-open");
    assert_eq!(samples[0].state.open_mode(), PopoverOpenMode::Uncontrolled);
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        samples[0].state.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );

    assert_eq!(samples[1].id, "controlled");
    assert_eq!(samples[1].state.open_mode(), PopoverOpenMode::Controlled);
    assert!(!samples[1].state.open());
    assert_eq!(
        samples[1].state.placement_side(),
        open_gpui_ui_core::OverlayPlacementSide::Right
    );

    assert_eq!(samples[2].id, "consume-outside");
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(
        !samples[2]
            .state
            .outside_press_policy()
            .resolve()
            .allows_underlay_dispatch()
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
    assert!(!samples[3].state.activation_enabled());
}

#[test]
fn overlay_page_dialog_samples_expose_modal_and_close_contracts() {
    let samples = pages::overlay::dialog_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "controlled-modal");
    assert_eq!(samples[0].state.title(), "Controlled dialog");
    assert_eq!(
        samples[0].state.description(),
        Some("Escape and the modal barrier can close it.")
    );
    assert_eq!(samples[0].state.open_mode(), DialogOpenMode::Controlled);
    assert!(!samples[0].state.open());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Modal
    );
    assert_eq!(
        samples[0].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(
        !samples[0]
            .state
            .overlay()
            .policy()
            .layer_state()
            .blocks_underlay_input()
    );

    assert_eq!(samples[1].id, "default-open");
    assert_eq!(samples[1].state.open_mode(), DialogOpenMode::Uncontrolled);
    assert!(samples[1].state.default_open());
    assert!(samples[1].state.open());
    assert_eq!(
        samples[1].state.escape_key_policy(),
        EscapeKeyPolicy::Dismiss
    );
    assert!(
        samples[1]
            .state
            .overlay()
            .policy()
            .layer_state()
            .blocks_underlay_input()
    );

    assert_eq!(samples[2].id, "outside-ignore");
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert!(
        !samples[2]
            .state
            .outside_press_policy()
            .resolve()
            .dismisses()
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
    assert!(!samples[3].state.activation_enabled());
}

#[test]
fn overlay_page_alert_dialog_samples_expose_critical_action_contracts() {
    let samples = pages::overlay::alert_dialog_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].id, "destructive-confirm");
    assert_eq!(samples[0].state.title(), "Delete this project?");
    assert_eq!(
        samples[0].state.description(),
        "This permanently removes project data and cannot be undone."
    );
    assert_eq!(
        samples[0].state.open_mode(),
        AlertDialogOpenMode::Controlled
    );
    assert!(!samples[0].state.open());
    assert_eq!(samples[0].state.intent(), AlertDialogIntent::Destructive);
    assert_eq!(samples[0].state.content_role(), Role::AlertDialog);
    assert_eq!(samples[0].state.action().label(), "Delete");
    assert_eq!(samples[0].state.cancel().label(), "Keep project");
    assert!(samples[0].state.cancel().default_focus());
    assert_eq!(
        samples[0].state.outside_press_policy(),
        OutsidePressPolicy::Consume
    );
    assert!(
        !samples[0]
            .state
            .outside_press_policy()
            .resolve()
            .dismisses()
    );
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Modal
    );

    assert_eq!(samples[1].id, "safe-cancel");
    assert_eq!(samples[1].state.title(), "Archive this item?");
    assert_eq!(
        samples[1].state.description(),
        "The item moves out of the active list and can be restored later."
    );
    assert_eq!(
        samples[1].state.open_mode(),
        AlertDialogOpenMode::Uncontrolled
    );
    assert!(samples[1].state.default_open());
    assert!(samples[1].state.open());
    assert_eq!(samples[1].state.intent(), AlertDialogIntent::Default);
    assert_eq!(samples[1].state.action().label(), "Archive");
    assert!(
        samples[1]
            .state
            .overlay()
            .layer_state()
            .blocks_underlay_input()
    );
    assert_eq!(
        samples[1].state.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
}

#[test]
fn overlay_page_sheet_samples_expose_edge_and_policy_contracts() {
    let samples = pages::overlay::sheet_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].id, "left-modal");
    assert_eq!(samples[0].state.title(), "Workspace filters");
    assert_eq!(
        samples[0].state.description(),
        Some("Filter active work without leaving the page.")
    );
    assert_eq!(samples[0].state.open_mode(), SheetOpenMode::Uncontrolled);
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.side(), SheetSide::Left);
    assert_eq!(samples[0].state.modal_mode(), SheetModalMode::Modal);
    assert_eq!(
        samples[0].state.close_affordance(),
        SheetCloseAffordance::Visible
    );
    assert_eq!(
        samples[0].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(
        samples[0]
            .state
            .overlay()
            .layer_state()
            .blocks_underlay_input()
    );

    assert_eq!(samples[1].id, "right-non-modal");
    assert_eq!(samples[1].state.open_mode(), SheetOpenMode::Controlled);
    assert!(!samples[1].state.open());
    assert_eq!(samples[1].state.side(), SheetSide::Right);
    assert_eq!(samples[1].state.modal_mode(), SheetModalMode::NonModal);
    assert_eq!(
        samples[1].state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert!(
        !samples[1]
            .state
            .overlay()
            .layer_state()
            .blocks_underlay_input()
    );
    assert_eq!(
        samples[1].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert!(
        samples[1]
            .state
            .outside_press_policy()
            .resolve()
            .allows_underlay_dispatch()
    );

    assert_eq!(samples[2].id, "bottom-sticky");
    assert_eq!(samples[2].state.side(), SheetSide::Bottom);
    assert_eq!(
        samples[2].state.close_affordance(),
        SheetCloseAffordance::Hidden
    );
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert!(!samples[2].state.overlay().wants_outside_press_handler());
}

#[test]
fn overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts() {
    let samples = pages::overlay::menu_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 7);
    assert_eq!(samples[0].id, "default-open");
    assert_eq!(samples[0].focused_value, Some("save"));
    assert_eq!(samples[0].state.open_mode(), MenuOpenMode::Uncontrolled);
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.focused_value(), Some("save"));
    assert_eq!(samples[0].state.items()[2].kind(), MenuItemKind::Separator);
    assert!(!samples[0].state.items()[3].activation_enabled());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Menu
    );
    assert!(samples[0].state.overlay().wants_outside_press_handler());

    assert_eq!(samples[1].id, "controlled");
    assert_eq!(samples[1].focused_value, Some("copy"));
    assert_eq!(samples[1].state.open_mode(), MenuOpenMode::Controlled);
    assert!(!samples[1].state.open());
    assert_eq!(
        samples[1].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        samples[1].state.escape_key_policy(),
        EscapeKeyPolicy::Dismiss
    );

    assert_eq!(samples[2].id, "outside-ignore");
    assert_eq!(samples[2].focused_value, None);
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert!(
        !samples[2]
            .state
            .outside_press_policy()
            .resolve()
            .dismisses()
    );

    assert_eq!(samples[3].id, "disabled");
    assert_eq!(samples[3].focused_value, None);
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());

    assert_eq!(samples[4].id, "rich-items");
    assert_eq!(samples[4].focused_value, Some("show-hidden"));
    assert!(samples[4].state.open());
    assert_eq!(samples[4].state.items()[0].kind(), MenuItemKind::Checkbox);
    assert_eq!(samples[4].state.items()[0].toggled(), Some(Toggled::True));
    assert_eq!(samples[4].state.items()[1].kind(), MenuItemKind::Radio);
    assert_eq!(samples[4].state.items()[1].toggled(), Some(Toggled::False));
    assert_eq!(samples[4].state.items()[2].toggled(), Some(Toggled::True));
    assert!(samples[4].state.items()[3].has_submenu());
    assert!(samples[4].state.items()[4].has_submenu());
    assert!(!samples[4].state.items()[5].focusable());

    assert_eq!(samples[5].id, "typeahead");
    assert_eq!(
        samples[5]
            .state
            .typeahead_target("br")
            .map(|item| item.value()),
        Some("bravo")
    );

    assert_eq!(samples[6].id, "long-scroll");
    assert!(samples[6].state.scrollable_content());
    assert_eq!(samples[6].state.visible_items().len(), 12);
}

#[test]
fn overlay_page_context_menu_samples_expose_point_anchor_contracts() {
    let samples = pages::overlay::context_menu_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 5);
    assert_eq!(samples[0].id, "point-anchor");
    assert_eq!(samples[0].focused_value, Some("duplicate"));
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.open_mode(), MenuOpenMode::Uncontrolled);
    assert_eq!(samples[0].state.menu().focused_value(), Some("duplicate"));
    assert_eq!(
        samples[0].state.menu().items()[1].kind(),
        MenuItemKind::Separator
    );
    assert!(!samples[0].state.menu().items()[2].activation_enabled());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Menu
    );
    assert_eq!(
        samples[0].state.placement_input().side(),
        OverlayPlacementSide::Bottom
    );
    assert_eq!(
        samples[0].state.placement_input().alignment(),
        OverlayPlacementAlignment::Start
    );
    assert_eq!(samples[0].state.placement_input().safe_bounds(), None);

    assert_eq!(samples[1].id, "controlled");
    assert_eq!(samples[1].focused_value, Some("inspect"));
    assert!(!samples[1].state.open());
    assert_eq!(samples[1].state.open_mode(), MenuOpenMode::Controlled);

    assert_eq!(samples[2].id, "default-open");
    assert_eq!(samples[2].focused_value, None);
    assert!(samples[2].state.default_open());
    assert!(samples[2].state.open());
    assert_eq!(samples[2].state.open_mode(), MenuOpenMode::Uncontrolled);

    assert_eq!(samples[3].id, "rich-items");
    assert_eq!(samples[3].focused_value, Some("snap-grid"));
    assert!(samples[3].state.open());
    assert_eq!(
        samples[3].state.menu().items()[0].kind(),
        MenuItemKind::Checkbox
    );
    assert_eq!(
        samples[3].state.menu().items()[0].toggled(),
        Some(Toggled::True)
    );
    assert!(samples[3].state.menu().items()[3].has_submenu());
    assert_eq!(
        samples[3].state.anchor_point(),
        ui_point(ui_px(620.0), ui_px(340.0))
    );

    assert_eq!(samples[4].id, "edge-long");
    assert!(samples[4].state.menu().scrollable_content());
    assert_eq!(
        samples[4].state.anchor_point(),
        ui_point(ui_px(960.0), ui_px(560.0))
    );
}

#[test]
fn overlay_page_catalog_entries_have_signals_and_sample_selectors() {
    use std::collections::BTreeSet;

    let tokens = ThemeTokens::default();
    let catalog = pages::overlay::OVERLAY_CATALOG;
    let names = catalog.iter().map(|entry| entry.name).collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "Tooltip",
            "HoverCard",
            "Popover",
            "Dialog",
            "AlertDialog",
            "Sheet",
            "Menu",
            "ContextMenu",
        ]
    );
    assert!(
        catalog
            .iter()
            .all(|entry| entry.status == pages::overlay::OverlayCatalogStatus::Official)
    );
    assert!(catalog.iter().all(|entry| !entry.family.trim().is_empty()));
    assert!(catalog.iter().all(|entry| !entry.state.trim().is_empty()));
    assert!(
        catalog
            .iter()
            .all(|entry| !entry.coverage.trim().is_empty())
    );
    assert!(catalog.iter().all(|entry| {
        !entry.behavior_gates.is_empty()
            && entry
                .behavior_gates
                .iter()
                .all(|gate| !gate.trim().is_empty())
    }));

    let catalog_names = catalog
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let selector_names = pages::overlay::overlay_sample_selector_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    assert_eq!(selector_names, catalog_names);

    let selector_values = pages::overlay::overlay_sample_selector_pairs()
        .map(|(_, selector)| selector)
        .collect::<Vec<_>>();
    let unique_selectors = selector_values.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_selectors.len(), selector_values.len());

    let tooltip_samples = pages::overlay::tooltip_samples(tokens);
    let hover_card_samples = pages::overlay::hover_card_samples(tokens);
    let popover_samples = pages::overlay::popover_samples(tokens);
    let dialog_samples = pages::overlay::dialog_samples(tokens);
    let alert_dialog_samples = pages::overlay::alert_dialog_samples(tokens);
    let sheet_samples = pages::overlay::sheet_samples(tokens);
    let menu_samples = pages::overlay::menu_samples(tokens);
    let context_menu_samples = pages::overlay::context_menu_samples(tokens);
    let expected_selectors = [
        ("Tooltip", tooltip_samples[0].debug_selector()),
        ("HoverCard", hover_card_samples[0].debug_selector()),
        ("Popover", popover_samples[0].debug_selector()),
        ("Dialog", dialog_samples[0].debug_selector()),
        ("AlertDialog", alert_dialog_samples[0].debug_selector()),
        ("Sheet", sheet_samples[0].debug_selector()),
        ("Menu", menu_samples[0].debug_selector()),
        ("ContextMenu", context_menu_samples[0].debug_selector()),
    ];

    for (name, selector) in expected_selectors {
        let entry = catalog
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("expected overlay catalog entry `{name}`"));
        assert_eq!(entry.sample_selector, selector.as_str());
    }

    for signal in [
        "open_gpui_ui_foundation_gallery::pages::overlay::OVERLAY_CATALOG",
        "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogEntry",
        "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogStatus",
        "open_gpui_ui_foundation_gallery::pages::overlay::overlay_sample_selector_pairs",
    ] {
        assert!(
            pages::overlay::SIGNALS.contains(&signal),
            "expected overlay catalog signal `{signal}`"
        );
    }

    let mut missing = Vec::new();
    for entry in catalog {
        let component_signal = format!("open_gpui_ui_components::{}", entry.name);
        if !pages::overlay::SIGNALS.contains(&component_signal.as_str()) {
            missing.push(format!(
                "{} component signal `{component_signal}`",
                entry.name
            ));
        }
        let state_signal = format!("open_gpui_ui_components::{}", entry.state);
        if !pages::overlay::SIGNALS.contains(&state_signal.as_str()) {
            missing.push(format!("{} state signal `{state_signal}`", entry.name));
        }
    }

    assert!(
        missing.is_empty(),
        "official overlay catalog entries must have matching signals: {missing:?}"
    );
}

#[test]
fn components_page_samples_expose_component_metadata() {
    let tokens = ThemeTokens::default();
    let catalog = pages::components::COMPONENT_CATALOG;
    let gates = pages::components::CONFORMANCE_GATES;
    let buttons = pages::components::button_samples(tokens);
    let badges = pages::components::badge_samples(tokens);
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let separators = pages::components::separator_samples(tokens);
    let kbds = pages::components::kbd_samples(tokens);
    let progress = pages::components::progress_samples(tokens);
    let skeletons = pages::components::skeleton_samples(tokens);
    let avatars = pages::components::avatar_samples(tokens);
    let status_cues = pages::components::status_cue_samples(tokens);
    let empty_states = pages::components::empty_state_samples(tokens);
    let switches = pages::components::switch_samples(tokens);
    let checkboxes = pages::components::checkbox_samples(tokens);
    let radio_groups = pages::components::radio_group_samples(tokens);
    let toggles = pages::components::toggle_samples(tokens);
    let toolbars = pages::components::toolbar_samples(tokens);
    let sidebars = pages::components::sidebar_samples(tokens);
    let trees = pages::components::tree_samples(tokens);
    let listboxes = pages::components::listbox_samples(tokens);
    let selects = pages::components::select_samples(tokens);
    let comboboxes = pages::components::combobox_samples(tokens);
    let commands = pages::components::command_samples(tokens);
    let labels = pages::components::label_samples(tokens);
    let text_inputs = pages::components::text_input_samples(tokens);
    let textareas = pages::components::textarea_samples(tokens);
    let fields = pages::components::field_samples(tokens);
    let field_textareas = pages::components::field_textarea_samples(tokens);
    let scroll_areas = pages::components::scroll_area_samples(tokens);
    let splitters = pages::components::splitter_samples(tokens);
    let tables = pages::components::table_samples(tokens);
    let virtualized_lists = pages::components::virtualized_list_samples(tokens);

    let official_names: Vec<_> = catalog
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        official_names,
        vec![
            "Button",
            "Badge",
            "IconButton",
            "Switch",
            "Checkbox",
            "RadioGroup",
            "Toggle",
            "Toolbar",
            "Sidebar",
            "Tree",
            "Listbox",
            "Select",
            "Combobox",
            "Command",
            "Label",
            "TextInput",
            "Textarea",
            "Field",
            "Tabs",
            "ScrollArea",
            "Splitter",
            "Table",
            "VirtualizedList",
            "StatusCue",
            "EmptyState",
            "Separator",
            "Kbd",
            "Progress",
            "Skeleton",
            "Avatar",
            "AvatarGroup",
        ]
    );
    assert!(catalog.iter().all(|entry| !entry.name.trim().is_empty()));
    assert!(
        catalog
            .iter()
            .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
            .all(|entry| entry.state.is_some())
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.name == "TextInputController"
                && entry.status == pages::components::ComponentCatalogStatus::AdapterOnly
                && entry.state.is_none())
    );
    assert!(catalog.iter().any(|entry| entry.name == "ToolbarItem"
        && entry.status == pages::components::ComponentCatalogStatus::InternalAnatomy));
    assert!(
        ["Separator", "Kbd", "Progress", "Skeleton", "Avatar"]
            .iter()
            .all(|name| catalog.iter().any(|entry| entry.name == *name
                && entry.status == pages::components::ComponentCatalogStatus::Official
                && entry.coverage == "exports / gallery / state tests"))
    );
    let state_contract_names: Vec<_> = catalog
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::StateContract)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        state_contract_names,
        vec!["TreeState", "VirtualizedListState"]
    );
    assert!(
        catalog
            .iter()
            .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::StateContract)
            .all(|entry| entry.sample_selector.is_none() && entry.state_contract_selector.is_some())
    );

    assert_eq!(gates.len(), 11);
    assert_eq!(gates[0].id, "public-api-exports");
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/lib.rs")
    );
    assert!(
        gates.iter().any(|gate| {
            gate.id == "choice-surfaces"
                && gate.evidence.contains(&"choice.rs")
                && gate.evidence.contains(&"roving_focus.rs")
        }),
        "expected Components conformance gates to expose the shared choice/navigation seam"
    );
    assert_eq!(gates[1].id, "gallery-metadata");
    assert_eq!(gates[2].id, "scroll-redraw");
    assert!(
        gates[2]
            .evidence
            .contains(&"scroll_area_default_handle_survives_reconstructed_component_values")
    );
    assert_eq!(gates[3].id, "splitter-runtime");
    assert_eq!(gates[4].id, "tabs-overflow");
    assert_eq!(gates[5].id, "table-virtualization");
    assert!(gates[5].evidence.contains(&"TableFacetedFilter"));
    assert!(gates[5].evidence.contains(&"TableGlobalFilter"));
    assert!(
        gates[5]
            .evidence
            .contains(&"components_gallery_smoke_global_filter_updates_table_rows")
    );
    assert!(gates[5].evidence.contains(&"TablePredicateFilter"));
    assert!(
        gates[5]
            .evidence
            .contains(&"components_gallery_smoke_predicate_filter_updates_table_rows")
    );
    assert!(
        gates[5]
            .evidence
            .contains(&"components_gallery_smoke_faceted_filter_updates_table_rows")
    );
    assert_eq!(gates[6].id, "tree-renderer");
    assert_eq!(gates[7].id, "virtualized-list-renderer");
    assert_eq!(gates[8].id, "state-contract-readouts");
    assert_eq!(gates[9].id, "choice-surfaces");
    assert_eq!(gates[10].id, "a11y-labels");

    assert_eq!(buttons.len(), 6);
    assert_eq!(buttons[0].id, "default");
    assert_eq!(buttons[0].state.role(), Role::Button);
    assert_eq!(
        buttons[3].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!buttons[5].state.activation_enabled());

    assert_eq!(badges.len(), 4);
    assert_eq!(badges[0].state.variant(), BadgeVariant::Default);
    assert!(badges[0].state.display_only());
    assert_eq!(badges[0].state.role(), None);
    assert_eq!(
        badges[2].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(badges[3].state.variant(), BadgeVariant::Outline);
    assert_eq!(badges[3].state.size(), Size::Small);

    assert_eq!(separators.len(), 3);
    assert_eq!(separators[0].id, "section-rule");
    assert_eq!(separators[0].state.role(), Some(Role::Separator));
    assert_eq!(separators[1].state.orientation(), Orientation::Vertical);
    assert_eq!(separators[1].state.metrics().thickness(), ui_px(2.0));
    assert!(separators[2].state.decorative());
    assert_eq!(separators[2].state.role(), None);

    assert_eq!(kbds.len(), 3);
    assert_eq!(kbds[0].id, "command-palette");
    assert_eq!(kbds[0].state.label(), "Ctrl+K");
    assert!(kbds[0].state.display_only());
    assert_eq!(kbds[2].state.size(), Size::Large);

    assert_eq!(progress.len(), 3);
    assert_eq!(progress[0].state.role(), Role::ProgressIndicator);
    assert_eq!(progress[0].state.value_percent(), Some(64.0));
    assert_eq!(progress[1].state.normalized_value(), Some(1.0));
    assert!(progress[2].state.indeterminate());

    assert_eq!(skeletons.len(), 3);
    assert_eq!(skeletons[0].id, "body-line");
    assert!(skeletons[0].state.display_only());
    assert!(skeletons[1].state.subtle());
    assert_eq!(skeletons[2].state.size(), Size::Large);

    assert_eq!(avatars.len(), 4);
    assert_eq!(avatars[0].state.fallback(), "AL");
    assert_eq!(avatars[1].state.fallback(), "ME");
    assert_eq!(avatars[1].state.accessible_label(), "Current user");
    assert!(avatars[2].state.has_source());
    assert_eq!(
        avatars[2].state.source().map(|source| source.uri()),
        Some("asset://avatars/katherine.png")
    );
    assert_eq!(avatars[3].state.fallback(), "?");

    assert_eq!(status_cues.len(), 3);
    assert_eq!(status_cues[0].id, "sync-warning");
    assert_eq!(status_cues[0].state.intent(), FeedbackIntent::Warning);
    assert_eq!(status_cues[0].state.role(), Role::Label);
    assert!(status_cues[0].state.display_only());
    assert_eq!(status_cues[0].state.size(), Size::Small);
    assert_eq!(status_cues[1].state.intent(), FeedbackIntent::Success);
    assert_eq!(status_cues[2].state.intent(), FeedbackIntent::Info);

    assert_eq!(empty_states.len(), 2);
    assert_eq!(empty_states[0].id, "no-results");
    assert_eq!(empty_states[0].state.intent(), FeedbackIntent::Neutral);
    assert_eq!(empty_states[0].state.role(), Role::Section);
    assert_eq!(
        empty_states[0].state.description(),
        Some("Adjust filters or clear the current query.")
    );
    assert_eq!(empty_states[1].state.intent(), FeedbackIntent::Danger);

    assert_eq!(icon_buttons.len(), 4);
    assert_eq!(icon_buttons[0].state.accessible_label(), "Search");
    assert_eq!(icon_buttons[0].state.role(), Role::Button);
    assert_eq!(icon_buttons[1].state.variant(), ButtonVariant::Outline);
    assert_eq!(
        icon_buttons[1].state.metrics().size(),
        Size::Small.icon_button_size()
    );
    assert_eq!(
        icon_buttons[2].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!icon_buttons[3].state.activation_enabled());

    assert_eq!(switches.len(), 4);
    assert_eq!(switches[0].state.role(), Role::Switch);
    assert_eq!(switches[0].state.toggled(), Toggled::False);
    assert_eq!(switches[1].state.toggled(), Toggled::True);
    assert!(!switches[3].state.activation_enabled());

    assert_eq!(checkboxes.len(), 6);
    assert_eq!(checkboxes[0].state.role(), Role::CheckBox);
    assert_eq!(checkboxes[0].state.toggled(), Toggled::False);
    assert_eq!(checkboxes[1].state.toggled(), Toggled::True);
    assert_eq!(checkboxes[2].state.toggled(), Toggled::Mixed);
    assert!(checkboxes[3].state.required());
    assert!(checkboxes[4].state.invalid());
    assert!(!checkboxes[5].state.activation_enabled());

    assert_eq!(radio_groups.len(), 2);
    assert_eq!(radio_groups[0].state.role(), Role::RadioGroup);
    assert!(radio_groups[0].state.required());
    assert_eq!(radio_groups[0].state.selected_value(), Some("team"));
    assert_eq!(radio_groups[0].state.focused_value(), Some("team"));
    assert!(radio_groups[0].state.items()[2].disabled());
    assert_eq!(radio_groups[0].state.items()[0].role(), Role::RadioButton);
    assert_eq!(radio_groups[1].state.orientation(), Orientation::Horizontal);
    assert_eq!(radio_groups[1].state.selected_value(), Some("europe"));

    assert_eq!(toggles.len(), 4);
    assert_eq!(toggles[0].state.role(), Role::Button);
    assert_eq!(toggles[0].state.toggled(), Toggled::False);
    assert_eq!(toggles[1].state.toggled(), Toggled::True);
    assert_eq!(toggles[2].state.variant(), ToggleVariant::Outline);
    assert!(!toggles[3].state.activation_enabled());

    assert_eq!(toolbars.len(), 2);
    assert_eq!(toolbars[0].id, "editor-toolbar");
    assert_eq!(toolbars[0].state.role(), Role::Toolbar);
    assert_eq!(toolbars[0].state.orientation(), Orientation::Horizontal);
    assert_eq!(toolbars[0].state.focused_value(), Some("bold"));
    assert_eq!(toolbars[0].state.items()[2].kind().as_str(), "separator");
    assert!(!toolbars[0].state.items()[2].focusable());
    assert!(toolbars[0].state.items()[3].pressed());
    assert_eq!(toolbars[1].state.orientation(), Orientation::Vertical);

    assert_eq!(sidebars.len(), 3);
    assert_eq!(sidebars[0].id, "workspace-sidebar");
    assert_eq!(sidebars[0].state.role(), Role::Navigation);
    assert_eq!(sidebars[0].state.side().as_str(), "left");
    assert_eq!(sidebars[0].state.variant().as_str(), "docked");
    assert_eq!(sidebars[0].state.collapse_mode().as_str(), "icon");
    assert_eq!(sidebars[0].state.size(), Size::Medium);
    assert_eq!(sidebars[0].state.label(), "Workspace navigation");
    assert_eq!(sidebars[0].state.selected_value(), Some("projects"));
    assert_eq!(sidebars[0].state.focused_value(), Some("projects"));
    assert_eq!(sidebars[0].state.sections()[0].role(), Role::Section);
    assert_eq!(sidebars[0].state.items()[1].badge_label(), Some("12"));
    assert!(!sidebars[0].state.items()[3].activation_enabled());
    assert!(sidebars[1].state.icon_collapsed());
    assert_eq!(sidebars[1].state.items()[0].label(), "Home");
    assert_eq!(sidebars[2].state.side().as_str(), "right");
    assert!(sidebars[2].state.scrollable());
    assert!(sidebars[2].state.items().len() > 8);

    assert_eq!(trees.len(), 4);
    let tree = &trees[0];
    assert_eq!(tree.id, "document-outline");
    assert_eq!(tree.state.role(), Role::Tree);
    assert_eq!(tree.state.item_role(), Role::TreeItem);
    assert_eq!(tree.state.selected_value(), Some("paper"));
    assert_eq!(tree.state.focused_value(), Some("paper"));
    assert!(tree.state.items().len() > 12);
    assert!(matches!(
        tree.state.keyboard_action_for_key("right"),
        Some(TreeKeyboardAction::Toggle(toggle)) if toggle.value() == "paper" && toggle.expanded()
    ));
    assert_eq!(tree.build_tree().state().role(), Role::Tree);
    let remote_tree = &trees[1];
    assert_eq!(remote_tree.id, "remote-workspace");
    assert_eq!(remote_tree.state.selected_value(), Some("remote-src"));
    assert!(
        remote_tree
            .state
            .item_by_value("remote-src")
            .is_some_and(|item| item.children_unloaded() && item.loaded_child_count() == 0)
    );
    assert!(
        remote_tree
            .state
            .item_by_value("remote-crates")
            .is_some_and(|item| item.children_loading()
                && item.children_load_state().message() == Some("Loading child packages"))
    );
    assert!(
        remote_tree
            .state
            .item_by_value("remote-build")
            .is_some_and(|item| item.children_load_failed()
                && item.children_load_state().message() == Some("Network unavailable"))
    );
    let release_tree = &trees[2];
    assert_eq!(release_tree.id, "release-outline");
    assert!(release_tree.virtualized);
    assert_eq!(release_tree.viewport_item_count, 8);
    assert_eq!(release_tree.overscan_count, 4);
    assert_eq!(release_tree.state.items().len(), 240);
    let release_tree_plan = release_tree.render_plan();
    assert_eq!(release_tree_plan.virtualizer().count(), 240);
    assert_eq!(release_tree_plan.visible_row_count(), 8);
    assert_eq!(release_tree_plan.rendered_row_count(), 12);
    assert_eq!(
        release_tree_plan.rows()[0].render_key(),
        "0:release-node-0000"
    );
    let editable_tree = &trees[3];
    assert_eq!(editable_tree.id, "editable-outline");
    assert!(editable_tree.draggable);
    assert_eq!(editable_tree.state.selected_value(), Some("root"));
    assert_eq!(editable_tree.state.focused_value(), Some("root"));
    assert_eq!(editable_tree.state.items().len(), 4);

    assert_eq!(listboxes.len(), 2);
    assert_eq!(listboxes[0].id, "assignee-listbox");
    assert_eq!(listboxes[0].state.role(), Role::ListBox);
    assert_eq!(listboxes[0].state.selected_value(), Some("owen"));
    assert_eq!(listboxes[0].state.active_value(), Some("maya"));
    assert_eq!(
        listboxes[0].state.options()[0].role(),
        Some(Role::ListBoxOption)
    );
    assert!(
        listboxes[0]
            .state
            .options()
            .iter()
            .any(|option| option.disabled())
    );
    assert!(listboxes[1].state.empty());

    assert_eq!(selects.len(), 3);
    assert_eq!(selects[0].id, "priority-select");
    assert_eq!(selects[0].state.open_mode(), SelectOpenMode::Controlled);
    assert!(selects[0].state.open());
    assert_eq!(selects[0].state.trigger_role(), Role::Button);
    assert_eq!(selects[0].state.content_role(), Role::ListBox);
    assert!(selects[0].state.scrollable_content());
    assert_eq!(selects[1].state.trigger_label(), "Doing");
    assert!(selects[2].state.disabled());
    assert!(!selects[2].state.open());

    assert_eq!(comboboxes.len(), 3);
    assert_eq!(comboboxes[0].id, "framework-combobox");
    assert_eq!(
        comboboxes[0].state.open_mode(),
        ComboboxOpenMode::Controlled
    );
    assert!(comboboxes[0].state.open());
    assert_eq!(comboboxes[0].state.input_role(), Role::EditableComboBox);
    assert_eq!(comboboxes[0].state.content_role(), Role::ListBox);
    assert_eq!(comboboxes[0].state.filtered_option_count(), 3);
    assert_eq!(comboboxes[0].state.selected_value(), Some("solid"));
    assert_eq!(comboboxes[0].state.listbox().selected_value(), None);
    assert_eq!(comboboxes[1].state.filtered_option_count(), 0);
    assert!(comboboxes[1].state.listbox().empty());
    assert!(comboboxes[2].state.disabled());
    assert!(!comboboxes[2].state.open());

    assert_eq!(commands.len(), 4);
    assert_eq!(commands[0].id, "ranked-search");
    assert_eq!(commands[0].state.open_mode(), CommandOpenMode::Controlled);
    assert!(commands[0].state.loading().is_none());
    assert!(commands[0].state.open());
    assert!(commands[0].state.dialog().is_some());
    assert_eq!(commands[0].state.list_role(), Role::ListBox);
    assert_eq!(commands[0].state.selected_value(), Some("open-file"));
    assert_eq!(commands[0].state.filtered_item_count(), 3);
    assert!(
        commands[0]
            .state
            .items()
            .iter()
            .any(|item| item.shortcut().is_some())
    );
    assert_eq!(
        commands[1].state.selection_mode(),
        CommandSelectionMode::Multiple
    );
    assert_eq!(commands[1].state.selected_chips().len(), 2);
    assert_eq!(commands[1].state.filtered_item_count(), 1);
    assert_eq!(commands[2].state.total_item_count(), 10_000);
    assert_eq!(commands[2].state.filtered_item_count(), 10_000);
    assert_eq!(commands[2].viewport_item_count, 7);
    assert!(commands[3].state.loading().is_some());
    assert_eq!(
        commands[3].state.loading().unwrap().message(),
        "Refreshing command index"
    );
    assert_eq!(
        commands[3].state.index_revision(),
        Some("workspace-index-v3")
    );
    assert_eq!(
        commands[3].state.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );

    assert_eq!(labels.len(), 4);
    assert_eq!(labels[0].state.role(), Role::Label);
    assert_eq!(labels[0].state.control_id(), Some("email-input"));
    assert!(labels[1].state.required());
    assert!(labels[2].state.disabled());
    assert!(!labels[3].state.associated());

    assert_eq!(text_inputs.len(), 6);
    assert_eq!(text_inputs[0].state.role(), Role::TextInput);
    assert!(text_inputs[0].state.controller_driven());
    assert!(
        text_inputs[1..]
            .iter()
            .all(|sample| !sample.state.controller_driven())
    );
    assert!(text_inputs[0].state.displaying_placeholder());
    assert!(text_inputs[1].state.has_value());
    assert_eq!(
        text_inputs[2].state.colors().border().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!text_inputs[3].state.editable());
    assert!(!text_inputs[4].state.editable());
    assert_eq!(
        text_inputs[5].state.display_mode(),
        TextInputDisplayMode::Password
    );
    assert_eq!(text_inputs[5].state.display_text().as_ref(), "•••");

    assert_eq!(textareas.len(), 4);
    assert_eq!(textareas[0].state.role(), Role::TextInput);
    assert!(textareas[0].state.displaying_placeholder());
    assert!(!textareas[0].state.controller_driven());
    assert_eq!(textareas[1].state.value(), "Line 1\nLine 2");
    assert_eq!(textareas[1].state.rows(), 4);
    assert_eq!(textareas[2].state.rows(), 3);
    assert!(textareas[2].state.value().contains("Line 8"));
    assert!(textareas[3].state.required());
    assert!(textareas[3].state.invalid());

    assert_eq!(fields.len(), 3);
    assert!(fields[0].state.required());
    assert_eq!(
        fields[0].state.support_text().unwrap(),
        "Use a work address."
    );
    assert!(fields[1].state.support_is_error());
    assert_eq!(
        fields[1].state.support_text().unwrap(),
        "Enter a valid email."
    );
    assert!(fields[2].state.disabled());
    assert!(!fields[2].input_state.editable());

    assert_eq!(field_textareas.len(), 1);
    assert!(field_textareas[0].state.required());
    assert!(field_textareas[0].state.invalid());
    assert_eq!(field_textareas[0].textarea_state.rows(), 4);
    assert_eq!(field_textareas[0].textarea_state.role(), Role::TextInput);

    assert_eq!(scroll_areas.len(), 3);
    assert_eq!(scroll_areas[0].id, "activity-log");
    assert_eq!(scroll_areas[0].state.axis(), ScrollAreaAxis::Vertical);
    assert_eq!(
        scroll_areas[0].state.reset_policy(),
        ScrollResetPolicy::Preserve
    );
    assert_eq!(
        scroll_areas[0].state.metrics().scrollbar_width(),
        ui_px(10.0)
    );
    assert_eq!(scroll_areas[1].state.axis(), ScrollAreaAxis::Horizontal);
    assert_eq!(scroll_areas[1].state.reset_key(), None);
    assert_eq!(scroll_areas[2].state.axis(), ScrollAreaAxis::Both);
    assert_eq!(
        scroll_areas[2].state.reset_policy(),
        ScrollResetPolicy::ResetOnKeyChange
    );
    assert_eq!(scroll_areas[2].state.reset_key(), Some("components"));

    assert_eq!(splitters.len(), 2);
    assert_eq!(splitters[0].id, "workspace-split");
    assert_eq!(splitters[0].state.orientation(), Orientation::Horizontal);
    assert_eq!(splitters[0].state.size(), Size::Medium);
    assert_eq!(splitters[0].state.panels().len(), 3);
    assert_eq!(splitters[0].state.handles().len(), 2);
    assert_eq!(splitters[0].state.panels()[0].id(), "navigator");
    assert_eq!(splitters[0].state.panels()[0].min_fraction(), 0.18);
    assert_eq!(splitters[0].state.panels()[0].max_fraction(), 0.34);
    assert!(!splitters[0].state.handles()[0].disabled());
    assert_eq!(splitters[1].state.orientation(), Orientation::Vertical);
    assert_eq!(splitters[1].state.size(), Size::Small);
    assert!(splitters[1].state.panels()[0].collapsed());
    assert_eq!(splitters[1].state.panels()[0].collapsed_fraction(), 0.08);

    assert_eq!(tables.len(), 15);
    let release_queue = table_sample(tables, "release-queue");
    assert_eq!(release_queue.state.rows().len(), 10_000);
    assert_eq!(
        release_queue.state.sorting()[0].direction().as_str(),
        "descending"
    );
    let release_plan = release_queue.render_plan();
    assert_eq!(release_plan.role(), Role::Table);
    assert_eq!(release_plan.column_header_role(), Role::ColumnHeader);
    assert_eq!(release_plan.cell_role(), Role::Cell);
    assert_eq!(release_plan.aria_row_count(), 10_001);
    assert_eq!(release_plan.aria_column_count(), 4);
    assert!(release_plan.rendered_row_count() <= release_plan.visible_row_count() + 5);

    let filter_board = table_sample(tables, "filter-board");
    let filter_plan = filter_board.render_plan();
    let filter_summary = filter_board.state_summary();
    assert_eq!(filter_plan.table().filtered_model().rows().len(), 60);
    assert_eq!(filter_plan.table().final_model().rows().len(), 24);
    assert_eq!(filter_plan.table().final_model().selected_count(), 1);
    assert_eq!(filter_summary.facet_columns, 4);
    assert_eq!(filter_summary.manual_facet_columns, 0);
    assert_eq!(filter_summary.status_facet_values, 4);
    assert_eq!(filter_summary.status_facet_total_count, 60);
    assert_eq!(filter_summary.score_facet_min, Some(0));
    assert_eq!(filter_summary.score_facet_max, Some(177));

    let status_facet = filter_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("filter-board status facet should resolve");
    assert_eq!(status_facet.mode(), TableStageMode::Client);
    assert_eq!(status_facet.row_count(), 60);
    assert_eq!(
        text_facet_counts(status_facet),
        [
            ("Doing".to_string(), 15),
            ("Done".to_string(), 15),
            ("Review".to_string(), 15),
            ("Todo".to_string(), 15),
        ],
        "client facets should ignore the target column filter and stay deterministic"
    );

    let score_facet = filter_plan
        .column_facet(&TableColumnId::new("score"))
        .expect("filter-board score facet should resolve");
    assert_eq!(score_facet.mode(), TableStageMode::Client);
    assert_eq!(score_facet.row_count(), 60);
    let score_range = score_facet
        .numeric_range()
        .expect("score facet should expose a numeric range");
    assert_eq!(score_range.min(), 0.0);
    assert_eq!(score_range.max(), 177.0);

    let server_paged = table_sample(tables, "server-paged");
    let server_page_plan = server_paged.render_plan();
    let server_page_summary = server_paged.state_summary();
    assert_eq!(server_paged.state.rows().len(), 8);
    assert_eq!(server_page_plan.filtering_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.sorting_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.pagination_mode(), TableStageMode::Manual);
    assert_eq!(server_page_summary.core_rows, 8);
    assert_eq!(server_page_summary.filtered_rows, 8);
    assert_eq!(server_page_summary.final_rows, 8);
    assert!(server_page_summary.manual_filtering);
    assert!(server_page_summary.manual_sorting);
    assert!(server_page_summary.manual_pagination);
    assert_eq!(server_page_summary.pagination_page_index, 2);
    assert_eq!(server_page_summary.pagination_page_size, 8);
    assert_eq!(server_page_summary.pagination_row_count, Some(64));
    assert_eq!(server_page_summary.pagination_page_count, Some(8));
    assert_eq!(server_page_summary.facet_columns, 4);
    assert_eq!(server_page_summary.manual_facet_columns, 2);
    assert_eq!(server_page_summary.status_facet_values, 4);
    assert_eq!(server_page_summary.status_facet_total_count, 64);
    assert_eq!(server_page_summary.score_facet_min, Some(1));
    assert_eq!(server_page_summary.score_facet_max, Some(64));
    assert_eq!(server_page_plan.pagination_row_count(), Some(64));
    assert_eq!(server_page_plan.pagination_page_count(), Some(8));
    assert_eq!(server_page_plan.faceting_mode(), TableStageMode::Client);
    assert_eq!(server_page_plan.column_facets().len(), 4);
    let server_status_facet = server_page_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("server-paged status facet should resolve");
    assert_eq!(server_status_facet.mode(), TableStageMode::Manual);
    assert_eq!(server_status_facet.row_count(), 64);
    assert_eq!(
        text_facet_counts(server_status_facet),
        [
            ("Blocked".to_string(), 16),
            ("Queued".to_string(), 16),
            ("Ready".to_string(), 16),
            ("Review".to_string(), 16),
        ],
        "server facet payloads should stay visible in render-plan metadata"
    );
    let server_score_facet = server_page_plan
        .column_facet(&TableColumnId::new("score"))
        .expect("server-paged score facet should resolve");
    assert_eq!(server_score_facet.mode(), TableStageMode::Manual);
    assert_eq!(server_score_facet.row_count(), 64);
    let server_score_range = server_score_facet
        .numeric_range()
        .expect("server score facet should expose a numeric range");
    assert_eq!(server_score_range.min(), 1.0);
    assert_eq!(server_score_range.max(), 64.0);
    assert_eq!(
        server_page_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "server-paged-row-0016",
            "server-paged-row-0017",
            "server-paged-row-0018",
            "server-paged-row-0019",
            "server-paged-row-0020",
            "server-paged-row-0021",
            "server-paged-row-0022",
            "server-paged-row-0023",
        ]
    );

    let release_resize = table_sample(tables, "release-resize");
    let resize_plan = release_resize.render_plan();
    assert_eq!(release_resize.state.rows().len(), 160);
    assert_eq!(
        resize_plan.columns()[0].width(),
        ui_px(188.0),
        "release-resize should expose committed sizing metadata"
    );
    assert!(
        resize_plan.columns()[0].resizable(),
        "name column should expose a resize handle"
    );
    assert!(
        !resize_plan.columns()[3].resizable(),
        "score column should prove per-column resize disablement"
    );

    let editable_release = table_sample(tables, "editable-release");
    let editable_plan = editable_release.render_plan();
    assert_eq!(editable_release.state.rows().len(), 32);
    assert!(
        editable_plan.columns()[0].text_editable(),
        "editable-release name column should expose text editing metadata"
    );
    assert!(
        editable_plan.columns()[1].text_editable(),
        "editable-release team column should expose text editing metadata"
    );
    assert!(
        !editable_plan.columns()[2].text_editable(),
        "editable-release status column should stay read-only"
    );
    assert!(
        editable_plan.rows()[0].cells()[0].text_editable(),
        "editable-release first visible name cell should render as editable"
    );

    let toggle_release = table_sample(tables, "toggle-release");
    let toggle_plan = toggle_release.render_plan();
    assert_eq!(toggle_release.state.rows().len(), 28);
    assert_eq!(
        toggle_plan.columns()[1].editor(),
        Some(TableCellEditor::Checkbox),
        "toggle-release enabled column should expose checkbox editing metadata"
    );
    assert_eq!(
        toggle_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Checkbox),
        "toggle-release first visible enabled cell should render as a checkbox editor"
    );
    assert_eq!(
        toggle_plan.table().final_model().rows()[0]
            .cell(&TableColumnId::new("enabled"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("true")
    );

    let multiline_release = table_sample(tables, "multiline-release");
    let multiline_plan = multiline_release.render_plan();
    assert_eq!(multiline_release.state.rows().len(), 24);
    assert_eq!(
        multiline_plan.columns()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 }),
        "multiline-release notes column should expose fixed-row multiline editing metadata"
    );
    assert_eq!(
        multiline_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 }),
        "multiline-release first visible notes cell should render as a multiline editor"
    );
    assert!(
        !multiline_plan.columns()[2].text_editable(),
        "multiline-release status column should stay read-only"
    );

    let select_release = table_sample(tables, "select-release");
    let select_plan = select_release.render_plan();
    assert_eq!(select_release.state.rows().len(), 28);
    assert_eq!(
        select_plan.columns()[1].editor(),
        Some(TableCellEditor::Select),
        "select-release status column should expose fixed-option select editing metadata"
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Select),
        "select-release first visible status cell should render as a select editor"
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].text(),
        "Ready",
        "select-release first visible status cell should resolve the display label from select options"
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].select_options().len(),
        2,
        "select-release visible select cell should carry the fixed option list"
    );

    let grouped_release = table_sample(tables, "release-rollup");
    let grouped_plan = grouped_release.render_plan();
    assert_eq!(grouped_release.state.grouping()[0].as_str(), "team");
    assert_eq!(grouped_release.state.aggregations().len(), 2);
    assert_eq!(
        grouped_release.state.column_pinning().left()[0].as_str(),
        "name"
    );
    assert_eq!(
        grouped_release.state.column_pinning().right()[0].as_str(),
        "status"
    );
    assert_eq!(
        grouped_plan.column_region_width(TableColumnRegion::Left),
        ui_px(188.0)
    );
    assert_eq!(
        grouped_plan.column_region_width(TableColumnRegion::Center),
        ui_px(400.0)
    );
    assert_eq!(
        grouped_plan.column_region_width(TableColumnRegion::Right),
        ui_px(164.0)
    );
    assert!(grouped_plan.uses_split_pinned_layout());
    let grouped_layout = grouped_plan
        .pinned_layout()
        .expect("release-rollup should render through the split sticky pinned layout");
    assert_eq!(grouped_layout.left_width(), ui_px(188.0));
    assert_eq!(grouped_layout.center_width(), ui_px(400.0));
    assert_eq!(grouped_layout.right_width(), ui_px(164.0));
    assert_eq!(grouped_layout.total_width(), ui_px(752.0));
    assert!(
        grouped_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .any(|row| row.is_group())
    );
    assert!(
        grouped_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .any(|row| row.is_leaf())
    );
    assert!(
        grouped_plan.rendered_row_count()
            <= grouped_plan.visible_row_count() + grouped_release.overscan
    );

    let release_matrix = table_sample(tables, "release-matrix");
    let matrix_plan = release_matrix.render_plan();
    assert_eq!(release_matrix.state.rows().len(), 480);
    assert_eq!(
        release_matrix.state.column_pinning().left()[0].as_str(),
        "name"
    );
    assert_eq!(
        release_matrix.state.column_pinning().right()[0].as_str(),
        "status"
    );
    assert_eq!(
        release_matrix.state.sorting()[0].column().as_str(),
        "metric_13"
    );
    assert_eq!(
        matrix_plan.column_region_width(TableColumnRegion::Left),
        ui_px(172.0)
    );
    assert_eq!(
        matrix_plan.column_region_width(TableColumnRegion::Center),
        ui_px(1516.0)
    );
    assert_eq!(
        matrix_plan.column_region_width(TableColumnRegion::Right),
        ui_px(148.0)
    );
    assert!(matrix_plan.uses_split_pinned_layout());
    let matrix_layout = matrix_plan
        .pinned_layout()
        .expect("release-matrix should render through the split sticky pinned layout");
    assert_eq!(matrix_layout.left_width(), ui_px(172.0));
    assert_eq!(matrix_layout.center_width(), ui_px(1516.0));
    assert_eq!(matrix_layout.right_width(), ui_px(148.0));
    assert_eq!(matrix_layout.total_width(), ui_px(1836.0));
    assert_eq!(matrix_plan.aria_column_count(), 16);
    assert_eq!(matrix_plan.columns().len(), 16);
    assert!(
        matrix_plan
            .columns()
            .iter()
            .any(|column| column.id().as_str() == "metric_13")
    );
    assert_eq!(matrix_plan.table().final_model().selected_count(), 1);

    let row_pinning = table_sample(tables, "row-pinning");
    let row_pinning_plan = row_pinning.render_plan();
    let row_pinning_summary = row_pinning.state_summary();
    assert_eq!(row_pinning.state.rows().len(), 96);
    assert_eq!(row_pinning_summary.core_rows, 96);
    assert_eq!(row_pinning_summary.final_rows, 14);
    assert_eq!(row_pinning_summary.pinned_top_rows, 1);
    assert_eq!(row_pinning_summary.pinned_center_rows, 11);
    assert_eq!(row_pinning_summary.pinned_bottom_rows, 2);
    assert!(!row_pinning_summary.row_pinning_page_only);
    assert_eq!(row_pinning_summary.pinned_left_columns, 1);
    assert_eq!(row_pinning_summary.pinned_center_columns, 14);
    assert_eq!(row_pinning_summary.pinned_right_columns, 1);
    assert_eq!(row_pinning_summary.pinned_left_width_px, 172);
    assert_eq!(row_pinning_summary.pinned_center_width_px, 1516);
    assert_eq!(row_pinning_summary.pinned_right_width_px, 148);
    assert_eq!(row_pinning_summary.total_column_width_px, 1836);
    assert!(row_pinning_plan.uses_split_pinned_layout());
    assert_eq!(row_pinning_plan.virtualizer().count(), 11);
    assert_eq!(row_pinning_plan.aria_row_count(), 15);
    assert_eq!(
        row_pinning_plan
            .table()
            .top_rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-pinning-row-003"]
    );
    assert_eq!(
        row_pinning_plan
            .table()
            .center_rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "row-pinning-row-024",
            "row-pinning-row-025",
            "row-pinning-row-026",
            "row-pinning-row-027",
            "row-pinning-row-028",
            "row-pinning-row-029",
            "row-pinning-row-031",
            "row-pinning-row-032",
            "row-pinning-row-033",
            "row-pinning-row-034",
            "row-pinning-row-035",
        ]
    );
    assert_eq!(
        row_pinning_plan
            .table()
            .bottom_rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-pinning-row-030", "row-pinning-row-070"]
    );

    let dependency_tree = table_sample(tables, "dependency-tree");
    let tree_plan = dependency_tree.render_plan();
    let tree_summary = dependency_tree.state_summary();
    assert_eq!(dependency_tree.state.rows().len(), 1);
    assert_eq!(tree_summary.core_rows, 7);
    assert_eq!(tree_summary.final_rows, 4);
    assert_eq!(tree_summary.tree_rows, 4);
    assert_eq!(tree_summary.tree_branch_rows, 3);
    assert_eq!(tree_summary.tree_depth, 1);
    assert_eq!(tree_summary.expanded_tree_inputs, 1);
    assert_eq!(tree_summary.pinned_left_columns, 1);
    assert_eq!(tree_summary.pinned_center_columns, 5);
    assert_eq!(tree_summary.pinned_right_columns, 1);
    assert_eq!(tree_summary.total_column_width_px, 956);
    assert_eq!(tree_plan.aria_column_count(), 7);
    assert_eq!(tree_plan.aria_row_count(), 5);
    assert_eq!(
        tree_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "dependency-workspace",
            "dependency-ui",
            "dependency-core",
            "dependency-docs"
        ]
    );
    assert!(
        tree_plan
            .table()
            .final_model()
            .row(&TableRowId::new("dependency-ui-table"))
            .is_some(),
        "collapsed tree descendants should remain addressable by stable row id"
    );
    assert_eq!(
        tree_plan
            .table()
            .final_model()
            .row(&TableRowId::new("dependency-ui"))
            .and_then(|row| row.tree_expanded()),
        Some(false)
    );

    let server_tree = table_sample(tables, "server-tree");
    let server_plan = server_tree.render_plan();
    let server_summary = server_tree.state_summary();
    assert_eq!(
        server_tree.state.expansion_mode(),
        TableExpansionMode::Manual
    );
    assert_eq!(server_tree.state.rows().len(), 3);
    assert_eq!(server_summary.core_rows, 3);
    assert_eq!(server_summary.final_rows, 3);
    assert_eq!(server_summary.tree_rows, 3);
    assert_eq!(server_summary.tree_branch_rows, 3);
    assert_eq!(server_summary.unloaded_tree_branches, 1);
    assert_eq!(server_summary.loading_tree_rows, 1);
    assert_eq!(server_summary.failed_tree_rows, 1);
    assert!(server_summary.manual_expansion);
    assert_eq!(server_summary.expanded_tree_inputs, 0);
    assert_eq!(server_summary.pinned_left_columns, 1);
    assert_eq!(server_summary.pinned_center_columns, 5);
    assert_eq!(server_summary.pinned_right_columns, 1);
    assert_eq!(server_summary.total_column_width_px, 956);
    assert_eq!(server_plan.aria_column_count(), 7);
    assert_eq!(server_plan.aria_row_count(), 4);
    assert_eq!(
        server_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["server-workspace", "server-cache", "server-failed"]
    );

    assert_eq!(virtualized_lists.len(), 1);
    let release_navigation = &virtualized_lists[0];
    assert_eq!(release_navigation.id, "release-navigation");
    assert_eq!(release_navigation.items.len(), 10_000);
    assert_eq!(release_navigation.state.item_count(), 10_000);
    assert_eq!(release_navigation.state.active_index(), Some(0));
    assert_eq!(release_navigation.state.selected_index(), Some(0));
    let release_navigation_plan = release_navigation.render_plan();
    let release_navigation_summary = release_navigation.state_summary();
    assert_eq!(release_navigation_plan.role(), Role::ListBox);
    assert_eq!(release_navigation_plan.row_role(), Role::ListBoxOption);
    assert_eq!(release_navigation_summary.item_count, 10_000);
    assert_eq!(release_navigation_summary.visible_start, 0);
    assert_eq!(release_navigation_summary.active_index, Some(0));
    assert_eq!(release_navigation_summary.selected_index, Some(0));
    assert!(
        release_navigation_plan.rendered_row_count()
            <= release_navigation_plan.visible_row_count() + release_navigation.overscan
    );
}

#[test]
fn components_page_state_contract_samples_expose_tree_and_virtualized_list_contracts() {
    let tree_contracts = pages::components::tree_state_contract_samples();
    let virtualized_list_contracts = pages::components::virtualized_list_state_contract_samples();

    assert_eq!(tree_contracts.len(), 1);
    let tree = &tree_contracts[0].state;
    let tree_values = tree
        .items()
        .iter()
        .map(|item| item.value())
        .collect::<Vec<_>>();
    assert_eq!(
        tree_values,
        ["paper", "intro", "figures", "disabled", "notes"]
    );
    assert_eq!(tree.selected_index(), Some(1));
    assert_eq!(tree.focused_index(), Some(2));
    assert_eq!(tree.items()[3].position_in_set(), None);
    assert_eq!(
        tree.navigation_target("down").map(|item| item.value()),
        Some("notes")
    );
    assert!(matches!(
        tree.keyboard_action_for_key("right"),
        Some(TreeKeyboardAction::Toggle(_))
    ));
    assert!(matches!(
        tree.keyboard_action_for_key("enter"),
        Some(TreeKeyboardAction::Select(_))
    ));

    assert_eq!(virtualized_list_contracts.len(), 1);
    let virtualized = &virtualized_list_contracts[0];
    assert_eq!(
        virtualized.scroll_strategy,
        VirtualizedListScrollStrategy::Center
    );
    assert_eq!(virtualized.state.item_count(), 10_000);
    assert_eq!(virtualized.state.active_index(), Some(42));
    assert_eq!(virtualized.state.selected_index(), Some(40));
    assert_eq!(virtualized.state.viewport_item_count(), 12);
    assert_eq!(virtualized.state.navigation_target("pageup"), Some(30));
    assert_eq!(virtualized.state.navigation_target("pagedown"), Some(54));
    assert_eq!(
        virtualized
            .state
            .activation_for_key("space")
            .map(|activation| activation.index()),
        Some(42)
    );
}

#[test]
fn component_gallery_shell_reads_splitter_behavior_from_resolved_state() {
    let components_source = include_str!("../src/pages/components.rs");
    let render_source = include_str!("../src/pages/components/render.rs");
    let splitter_struct_start = components_source
        .find("pub struct SplitterSample {")
        .expect("expected SplitterSample struct to exist");
    let splitter_struct_end = components_source[splitter_struct_start..]
        .find("impl_component_sample_selectors!(SplitterSample, \"component-splitter-sample\");")
        .map(|offset| splitter_struct_start + offset)
        .expect("expected SplitterSample selector impl to exist");
    let splitter_struct = &components_source[splitter_struct_start..splitter_struct_end];
    let splitter_section = render_source
        .split("splitter_samples.into_iter().map(|sample| {")
        .nth(1)
        .and_then(|section| {
            section
                .split("scroll_area_samples.into_iter().map(|sample| {")
                .next()
        })
        .expect("expected Splitter section in components render source");

    assert!(!splitter_struct.contains("pub orientation: Orientation,"));
    assert!(!splitter_struct.contains("pub size: Size,"));
    assert!(splitter_section.contains(".orientation(state.orientation())"));
    assert!(splitter_section.contains(".with_size(state.size())"));
    assert!(!splitter_section.contains(".orientation(sample.orientation)"));
    assert!(!splitter_section.contains(".with_size(sample.size)"));
}

#[test]
fn component_gallery_shell_reads_choice_active_metadata_from_resolved_state() {
    let shell_source = include_str!("../src/shell.rs");
    let select_section = shell_source
        .split("fn component_select_samples_section(")
        .nth(1)
        .and_then(|section| {
            section
                .split("fn component_combobox_samples_section")
                .next()
        })
        .expect("expected Select sample section in shell source");
    let combobox_section = shell_source
        .split("fn component_combobox_samples_section(")
        .nth(1)
        .and_then(|section| section.split("fn component_command_samples_section").next())
        .expect("expected Combobox sample section in shell source");
    let command_section = shell_source
        .split("fn component_command_samples_section(")
        .nth(1)
        .and_then(|section| section.split("fn resolved_listbox_option").next())
        .expect("expected Command sample section in shell source");
    let listbox_readout = shell_source
        .split("fn component_listbox_state_row(")
        .nth(1)
        .and_then(|section| section.split("fn component_select_state_row").next())
        .expect("expected Listbox state row in shell source");
    let select_readout = shell_source
        .split("fn component_select_state_row(")
        .nth(1)
        .and_then(|section| section.split("fn component_combobox_state_row").next())
        .expect("expected Select state row in shell source");
    let combobox_readout = shell_source
        .split("fn component_combobox_state_row(")
        .nth(1)
        .and_then(|section| section.split("fn component_command_state_row").next())
        .expect("expected Combobox state row in shell source");
    let command_readout = shell_source
        .split("fn component_command_state_row(")
        .nth(1)
        .and_then(|section| {
            section
                .split("pub(crate) fn component_radio_state_row")
                .next()
        })
        .expect("expected Command state row in shell source");

    assert!(select_section.contains("if let Some(active) = state.active_value()"));
    assert!(select_section.contains("select = select.active(active);"));
    assert!(combobox_section.contains("if let Some(active) = state.active_value()"));
    assert!(combobox_section.contains("combobox = combobox.active(active);"));
    assert!(command_section.contains("if let Some(active) = state.active_value()"));
    assert!(command_section.contains("command = command.active(active);"));
    assert!(listbox_readout.contains("typeahead_label"));
    assert!(listbox_readout.contains("first_typeahead_target"));
    assert!(select_readout.contains("listbox selected"));
    assert!(combobox_readout.contains("visible {} of {} / typeahead"));
    assert!(command_readout.contains("selected_values {:?}"));
}

#[test]
fn official_component_catalog_entries_have_signals_and_sample_selectors() {
    use std::collections::BTreeSet;

    let official_names = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let sample_names = pages::components::official_sample_selector_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();

    assert_eq!(sample_names, official_names);

    let selector_values = pages::components::official_sample_selector_pairs()
        .map(|(_, selector)| selector)
        .collect::<Vec<_>>();
    let unique_selectors = selector_values.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_selectors.len(), selector_values.len());

    let stray_selectors = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status != pages::components::ComponentCatalogStatus::Official)
        .filter(|entry| entry.sample_selector.is_some())
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        stray_selectors.is_empty(),
        "non-official catalog entries must not declare sample selectors: {stray_selectors:?}"
    );
    let official_state_contract_selectors = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .filter(|entry| entry.state_contract_selector.is_some())
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        official_state_contract_selectors.is_empty(),
        "official catalog entries must not declare state-contract readout selectors: {official_state_contract_selectors:?}"
    );

    let signals = pages::components::SIGNALS;
    let mut missing = Vec::new();
    for entry in pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
    {
        let component_signal = format!("open_gpui_ui_components::{}", entry.name);
        if !signals.contains(&component_signal.as_str()) {
            missing.push(format!(
                "{} component signal `{component_signal}`",
                entry.name
            ));
        }
        let Some(state) = entry.state else {
            missing.push(format!("{} official entry has no state type", entry.name));
            continue;
        };
        let state_signal = format!("open_gpui_ui_components::{state}");
        if !signals.contains(&state_signal.as_str()) {
            missing.push(format!("{} state signal `{state_signal}`", entry.name));
        }
    }

    assert!(
        missing.is_empty(),
        "official component catalog entries must have matching signals: {missing:?}"
    );
}

#[test]
fn state_contract_catalog_entries_have_signals_and_readout_selectors() {
    use std::collections::BTreeSet;

    let state_contract_names = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::StateContract)
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let readout_names = pages::components::state_contract_readout_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();

    assert_eq!(readout_names, state_contract_names);

    let official_names = pages::components::official_sample_selector_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    assert!(
        state_contract_names.is_disjoint(&official_names),
        "state contracts must not satisfy the official rendered component selector gate"
    );

    let readout_selectors = pages::components::state_contract_readout_pairs()
        .map(|(_, selector)| selector)
        .collect::<Vec<_>>();
    let unique_readout_selectors = readout_selectors.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_readout_selectors.len(), readout_selectors.len());

    let stray_readout_selectors = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status != pages::components::ComponentCatalogStatus::StateContract)
        .filter(|entry| entry.state_contract_selector.is_some())
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        stray_readout_selectors.is_empty(),
        "non-state-contract catalog entries must not declare state-contract selectors: {stray_readout_selectors:?}"
    );

    let required_signals = [
        "open_gpui_ui_components::TreeState",
        "open_gpui_ui_components::TreeItemDescriptor",
        "open_gpui_ui_components::TreeItemState",
        "open_gpui_ui_components::TreeSelection",
        "open_gpui_ui_components::TreeToggle",
        "open_gpui_ui_components::TreeFocusTarget",
        "open_gpui_ui_components::TreeKeyboardAction",
        "open_gpui_ui_components::tree_navigation_target",
        "open_gpui_ui_components::VirtualizedListState",
        "open_gpui_ui_components::VirtualizedListActivation",
        "open_gpui_ui_components::VirtualizedListMetrics",
        "open_gpui_ui_components::VirtualizedListScrollStrategy",
        "open_gpui_ui_components::virtualized_list_navigation_target",
    ];
    for signal in required_signals {
        assert!(
            pages::components::SIGNALS.contains(&signal),
            "expected state-contract signal `{signal}`"
        );
    }
}

#[test]
fn components_page_tabs_samples_expose_roving_focus_contract() {
    let tokens = ThemeTokens::default();
    let tabs = pages::components::tabs_samples(tokens);

    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0].id, "overview-tabs");
    assert_eq!(tabs[0].state.selected_value(), Some("overview"));
    assert_eq!(tabs[0].state.focused_value(), Some("overview"));
    assert!(tabs[0].items.iter().any(|item| item.disabled));

    assert_eq!(tabs[1].id, "workspace-tabs");
    assert!(tabs[1].items.len() >= 12);
    assert_eq!(tabs[1].state.selected_value(), Some("profile"));
    assert_eq!(tabs[1].state.focused_value(), Some("profile"));
    assert!(tabs[1].items[3].disabled);
}

#[test]
fn components_page_table_samples_expose_virtualized_row_model_contract() {
    let samples = pages::components::table_samples(ThemeTokens::default());
    let release_queue = table_sample(samples, "release-queue");
    let release_plan = release_queue.render_plan();
    let release_summary = release_queue.state_summary();

    assert_eq!(release_queue.id, "release-queue");
    assert_eq!(release_queue.state.rows().len(), 10_000);
    assert_eq!(release_plan.table().final_model().rows().len(), 10_000);
    assert_eq!(release_summary.core_rows, 10_000);
    assert_eq!(release_summary.final_rows, 10_000);
    assert_eq!(
        release_summary.rendered_rows,
        release_plan.rendered_row_count()
    );
    assert_eq!(
        release_summary.visible_rows,
        release_plan.visible_row_count()
    );
    assert_eq!(
        release_plan.table().final_model().rows()[0].id().as_str(),
        "release-queue-row-0000"
    );
    assert_eq!(release_plan.virtualizer().count(), 10_000);
    assert!(!release_plan.virtualizer().visible_range().is_empty());
    assert!(release_plan.rendered_row_count() <= release_plan.visible_row_count() + 5);
    assert_eq!(release_plan.row_role(), Role::Row);
    assert_eq!(release_plan.column_header_role(), Role::ColumnHeader);
    assert_eq!(release_plan.cell_role(), Role::Cell);

    let filter_board = table_sample(samples, "filter-board");
    let filter_plan = filter_board.render_plan();
    let filter_summary = filter_board.state_summary();

    assert_eq!(filter_board.id, "filter-board");
    assert_eq!(filter_board.state.rows().len(), 180);
    assert_eq!(filter_plan.table().filtered_model().rows().len(), 60);
    assert_eq!(filter_plan.table().final_model().rows().len(), 24);
    assert_eq!(filter_summary.filtered_rows, 60);
    assert_eq!(filter_summary.final_rows, 24);
    assert_eq!(filter_summary.selected_rows, 1);
    assert_eq!(filter_summary.facet_columns, 4);
    assert_eq!(filter_summary.manual_facet_columns, 0);
    assert_eq!(filter_summary.status_facet_values, 4);
    assert_eq!(filter_summary.status_facet_total_count, 60);
    assert_eq!(filter_summary.score_facet_min, Some(0));
    assert_eq!(filter_summary.score_facet_max, Some(177));
    assert_eq!(
        filter_plan.table().final_model().rows()[0].id().as_str(),
        "filter-board-row-177"
    );
    assert_eq!(filter_plan.table().final_model().selected_count(), 1);
    assert_eq!(filter_plan.aria_column_count(), 4);
    let filter_status_facet = filter_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("filter-board status facet should resolve");
    assert_eq!(filter_status_facet.mode(), TableStageMode::Client);
    assert_eq!(filter_status_facet.row_count(), 60);
    assert_eq!(facet_total_count(filter_status_facet), 60);

    let server_paged = table_sample(samples, "server-paged");
    let server_page_plan = server_paged.render_plan();
    let server_page_summary = server_paged.state_summary();

    assert_eq!(server_paged.id, "server-paged");
    assert_eq!(server_paged.state.rows().len(), 8);
    assert_eq!(server_page_plan.filtering_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.sorting_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.pagination_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.pagination_row_count(), Some(64));
    assert_eq!(server_page_plan.pagination_page_count(), Some(8));
    assert_eq!(server_page_summary.core_rows, 8);
    assert_eq!(server_page_summary.filtered_rows, 8);
    assert_eq!(server_page_summary.final_rows, 8);
    assert_eq!(server_page_summary.selected_rows, 1);
    assert!(server_page_summary.manual_filtering);
    assert!(server_page_summary.manual_sorting);
    assert!(server_page_summary.manual_pagination);
    assert_eq!(server_page_summary.pagination_page_index, 2);
    assert_eq!(server_page_summary.pagination_page_size, 8);
    assert_eq!(server_page_summary.pagination_row_count, Some(64));
    assert_eq!(server_page_summary.pagination_page_count, Some(8));
    assert_eq!(server_page_summary.facet_columns, 4);
    assert_eq!(server_page_summary.manual_facet_columns, 2);
    assert_eq!(server_page_summary.status_facet_values, 4);
    assert_eq!(server_page_summary.status_facet_total_count, 64);
    assert_eq!(server_page_summary.score_facet_min, Some(1));
    assert_eq!(server_page_summary.score_facet_max, Some(64));
    let server_status_facet = server_page_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("server-paged status facet should resolve");
    assert_eq!(server_status_facet.mode(), TableStageMode::Manual);
    assert_eq!(server_status_facet.row_count(), 64);
    assert_eq!(facet_total_count(server_status_facet), 64);
    let server_score_range = server_page_plan
        .column_facet(&TableColumnId::new("score"))
        .and_then(|facet| facet.numeric_range())
        .expect("server-paged score facet should resolve");
    assert_eq!(server_score_range.min(), 1.0);
    assert_eq!(server_score_range.max(), 64.0);
    assert_eq!(
        server_page_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "server-paged-row-0016",
            "server-paged-row-0017",
            "server-paged-row-0018",
            "server-paged-row-0019",
            "server-paged-row-0020",
            "server-paged-row-0021",
            "server-paged-row-0022",
            "server-paged-row-0023",
        ],
        "manual modes should preserve the supplied server page snapshot"
    );
    assert_eq!(server_page_plan.table().final_model().selected_count(), 1);
    assert!(
        server_page_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .any(|row| row.id().as_str() == "server-paged-row-0018" && row.selected())
    );

    let release_resize = table_sample(samples, "release-resize");
    let resize_plan = release_resize.render_plan();
    let resize_summary = release_resize.state_summary();

    assert_eq!(release_resize.id, "release-resize");
    assert_eq!(release_resize.state.rows().len(), 160);
    assert_eq!(resize_plan.table().final_model().rows().len(), 160);
    assert_eq!(resize_summary.core_rows, 160);
    assert_eq!(resize_summary.total_column_width_px, 520);
    assert_eq!(resize_summary.resizable_columns, 3);
    assert_eq!(resize_plan.columns()[0].width(), ui_px(188.0));
    assert_eq!(resize_plan.columns()[1].width(), ui_px(116.0));
    assert_eq!(resize_plan.columns()[2].width(), ui_px(132.0));
    assert_eq!(resize_plan.columns()[3].width(), ui_px(84.0));
    assert!(resize_plan.columns()[0].resizable());
    assert!(resize_plan.columns()[1].resizable());
    assert!(resize_plan.columns()[2].resizable());
    assert!(!resize_plan.columns()[3].resizable());

    let content_fit_release = table_sample(samples, "content-fit-release");
    let content_fit_plan = content_fit_release.render_plan();
    let content_fit_summary = content_fit_release.state_summary();

    assert_eq!(content_fit_release.id, "content-fit-release");
    assert_eq!(content_fit_release.state.rows().len(), 32);
    assert_eq!(content_fit_summary.core_rows, 32);
    assert_eq!(content_fit_summary.selected_rows, 1);
    assert_eq!(
        content_fit_plan.columns()[0].width_policy(),
        TableColumnWidthPolicy::ContentFit
    );
    assert_eq!(content_fit_plan.columns()[3].width(), ui_px(84.0));

    let toggle_release = table_sample(samples, "toggle-release");
    let toggle_plan = toggle_release.render_plan();
    let toggle_summary = toggle_release.state_summary();

    assert_eq!(toggle_release.id, "toggle-release");
    assert_eq!(toggle_release.state.rows().len(), 28);
    assert_eq!(toggle_summary.core_rows, 28);
    assert_eq!(toggle_summary.selected_rows, 1);
    assert_eq!(
        toggle_plan.columns()[1].editor(),
        Some(TableCellEditor::Checkbox)
    );
    assert_eq!(
        toggle_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Checkbox)
    );
    assert_eq!(
        toggle_plan.table().final_model().rows()[0]
            .cell(&TableColumnId::new("enabled"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("true")
    );

    let select_release = table_sample(samples, "select-release");
    let select_plan = select_release.render_plan();
    let select_summary = select_release.state_summary();

    assert_eq!(select_release.id, "select-release");
    assert_eq!(select_release.state.rows().len(), 28);
    assert_eq!(select_summary.core_rows, 28);
    assert_eq!(select_summary.selected_rows, 1);
    assert_eq!(
        select_plan.columns()[1].editor(),
        Some(TableCellEditor::Select)
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Select)
    );
    assert_eq!(select_plan.rows()[0].cells()[1].text(), "Ready");
    assert_eq!(select_plan.rows()[0].cells()[1].select_options().len(), 2);

    let multiline_release = table_sample(samples, "multiline-release");
    let multiline_plan = multiline_release.render_plan();
    let multiline_summary = multiline_release.state_summary();

    assert_eq!(multiline_release.id, "multiline-release");
    assert_eq!(multiline_release.state.rows().len(), 24);
    assert_eq!(multiline_summary.core_rows, 24);
    assert_eq!(multiline_summary.selected_rows, 1);
    assert_eq!(multiline_release.row_height, ui_px(82.0));
    assert_eq!(
        multiline_plan.columns()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert_eq!(
        multiline_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert_eq!(
        multiline_plan.table().final_model().rows()[0]
            .cell(&TableColumnId::new("notes"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("User-visible summary 000\nRollback: pending")
    );

    let grouped_release = table_sample(samples, "release-rollup");
    let grouped_plan = grouped_release.render_plan();
    let grouped_summary = grouped_release.state_summary();

    assert_eq!(grouped_release.id, "release-rollup");
    assert_eq!(grouped_release.state.rows().len(), 320);
    assert_eq!(grouped_release.state.grouping()[0].as_str(), "team");
    assert_eq!(grouped_release.state.aggregations().len(), 2);
    assert!(matches!(
        grouped_release.state.expansion(),
        TableExpansionState::Rows(rows) if rows.len() == 2
    ));
    assert_eq!(
        grouped_release.state.column_pinning().left()[0].as_str(),
        "name"
    );
    assert_eq!(
        grouped_release.state.column_pinning().right()[0].as_str(),
        "status"
    );
    assert_eq!(grouped_summary.core_rows, 320);
    assert_eq!(grouped_summary.grouping_columns, 1);
    assert_eq!(grouped_summary.aggregation_count, 2);
    assert_eq!(grouped_summary.expanded_group_inputs, 2);
    assert!(!grouped_summary.all_rows_expanded);
    assert_eq!(grouped_summary.pinned_left_columns, 1);
    assert_eq!(grouped_summary.pinned_center_columns, 2);
    assert_eq!(grouped_summary.pinned_right_columns, 1);
    assert_eq!(grouped_summary.pinned_left_width_px, 188);
    assert_eq!(grouped_summary.pinned_center_width_px, 400);
    assert_eq!(grouped_summary.pinned_right_width_px, 164);
    assert_eq!(grouped_summary.total_column_width_px, 752);
    assert!(grouped_plan.uses_split_pinned_layout());
    assert!(grouped_summary.group_rows >= 5);
    assert!(grouped_summary.leaf_rows > 0);
    assert!(grouped_summary.expanded_rows < grouped_summary.grouped_rows);

    let ui_group = grouped_plan
        .table()
        .final_model()
        .row(&TableRowId::new("group:team=UI"))
        .expect("expanded UI group should be visible and addressable");
    assert!(ui_group.is_group());
    assert_eq!(
        ui_group
            .cell(&TableColumnId::new("name"))
            .expect("group count aggregate should be present")
            .filter_text(),
        "64"
    );
    assert!(
        !ui_group
            .cell(&TableColumnId::new("score"))
            .expect("group score aggregate should be present")
            .filter_text()
            .is_empty()
    );
    assert!(
        grouped_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .any(|row| row.id().as_str() == "grouped-release-row-000" && row.is_leaf())
    );
    assert!(
        grouped_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .all(|row| row.id().as_str() != "grouped-release-row-001"),
        "Runtime leaf row should stay hidden because that group starts collapsed"
    );
    assert!(
        grouped_plan
            .table()
            .final_model()
            .row(&TableRowId::new("grouped-release-row-001"))
            .is_some(),
        "collapsed descendants should stay addressable by stable row id"
    );
    assert_eq!(
        grouped_plan
            .column_regions()
            .iter()
            .map(|region| (
                region.region(),
                region
                    .columns()
                    .iter()
                    .map(|column| column.id().as_str())
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        [
            (TableColumnRegion::Left, vec!["name"]),
            (TableColumnRegion::Center, vec!["team", "score"]),
            (TableColumnRegion::Right, vec!["status"]),
        ]
    );

    let custom_grouped = table_sample(samples, "grouped-custom-aggregation");
    let custom_plan = custom_grouped.render_plan();
    let custom_summary = custom_grouped.state_summary();

    assert_eq!(custom_grouped.id, "grouped-custom-aggregation");
    assert_eq!(custom_grouped.state.rows().len(), 8);
    assert_eq!(custom_grouped.state.grouping()[0].as_str(), "team");
    assert_eq!(custom_grouped.state.aggregations().len(), 2);
    assert_eq!(custom_grouped.state.aggregation_fn_count(), 1);
    assert!(custom_grouped.state.has_aggregation_fn("score_plus_one"));
    assert_eq!(custom_summary.custom_aggregation_count, 1);
    assert_eq!(custom_plan.aggregation_fn_count(), 1);
    assert_eq!(custom_summary.grouping_columns, 1);
    assert_eq!(custom_summary.aggregation_count, 2);
    assert_eq!(custom_summary.group_rows, 2);
    assert_eq!(custom_summary.leaf_rows, 8);
    assert_eq!(custom_summary.expanded_group_inputs, 2);
    assert_eq!(custom_plan.table().final_model().rows().len(), 10);
    let custom_ui_group = custom_plan
        .table()
        .final_model()
        .row(&TableRowId::new("group:team=UI"))
        .expect("expanded UI custom group should be visible and addressable");
    assert_eq!(
        custom_ui_group
            .cell(&TableColumnId::new("name"))
            .expect("custom group count aggregate should be present")
            .filter_text(),
        "4"
    );
    assert_eq!(
        custom_ui_group
            .cell(&TableColumnId::new("score"))
            .expect("custom score aggregate should be present")
            .filter_text(),
        "11"
    );
    assert_eq!(
        custom_plan
            .table()
            .final_model()
            .row(&TableRowId::new("group:team=Platform"))
            .expect("expanded Platform custom group should be visible and addressable")
            .cell(&TableColumnId::new("score"))
            .expect("platform custom score aggregate should be present")
            .filter_text(),
        "101"
    );

    let release_matrix = table_sample(samples, "release-matrix");
    let matrix_plan = release_matrix.render_plan();
    let matrix_summary = release_matrix.state_summary();

    assert_eq!(release_matrix.id, "release-matrix");
    assert_eq!(release_matrix.state.rows().len(), 480);
    assert_eq!(
        release_matrix.state.sorting()[0].column().as_str(),
        "metric_13"
    );
    assert_eq!(matrix_summary.header_rows, 3);
    assert_eq!(matrix_summary.header_groups, 4);
    assert_eq!(matrix_summary.visible_leaf_columns, 16);
    assert_eq!(matrix_summary.core_rows, 480);
    assert_eq!(matrix_summary.final_rows, 480);
    assert_eq!(matrix_summary.selected_rows, 1);
    assert_eq!(matrix_summary.pinned_left_columns, 1);
    assert_eq!(matrix_summary.pinned_center_columns, 14);
    assert_eq!(matrix_summary.pinned_right_columns, 1);
    assert_eq!(matrix_summary.pinned_left_width_px, 172);
    assert_eq!(matrix_summary.pinned_center_width_px, 1516);
    assert_eq!(matrix_summary.pinned_right_width_px, 148);
    assert_eq!(matrix_summary.total_column_width_px, 1836);
    assert!(matrix_plan.uses_split_pinned_layout());
    assert_eq!(matrix_plan.aria_column_count(), 16);
    assert_eq!(matrix_plan.header_row_count(), 3);
    assert_eq!(matrix_plan.left_header_groups().header_row_count(), 3);
    assert_eq!(matrix_plan.center_header_groups().header_row_count(), 3);
    assert_eq!(matrix_plan.right_header_groups().header_row_count(), 3);
    assert_eq!(
        matrix_plan
            .left_header_groups()
            .group_at_depth(1)
            .expect("left header group row should exist")
            .headers()[0]
            .label(),
        "Identity"
    );
    assert_eq!(
        matrix_plan
            .center_header_groups()
            .group_at_depth(1)
            .expect("center header group row should exist")
            .headers()[0]
            .label(),
        "Metrics"
    );
    assert_eq!(
        matrix_plan
            .right_header_groups()
            .group_at_depth(1)
            .expect("right header group row should exist")
            .headers()[0]
            .label(),
        "Delivery"
    );
    assert_eq!(
        matrix_plan
            .column_regions()
            .iter()
            .find(|region| region.region() == TableColumnRegion::Center)
            .expect("release-matrix should expose a center column region")
            .columns()
            .iter()
            .map(|column| column.id().as_str())
            .collect::<Vec<_>>(),
        [
            "metric_00",
            "metric_01",
            "metric_02",
            "metric_03",
            "metric_04",
            "metric_05",
            "metric_06",
            "metric_07",
            "metric_08",
            "metric_09",
            "metric_10",
            "metric_11",
            "metric_12",
            "metric_13",
        ]
    );
    let row_pinning = table_sample(samples, "row-pinning");
    let row_pinning_plan = row_pinning.render_plan();
    let row_pinning_summary = row_pinning.state_summary();

    assert_eq!(row_pinning.id, "row-pinning");
    assert_eq!(row_pinning.state.rows().len(), 96);
    assert_eq!(row_pinning.state.pagination().page_index(), 2);
    assert_eq!(row_pinning.state.pagination().page_size(), 12);
    assert_eq!(row_pinning_summary.core_rows, 96);
    assert_eq!(row_pinning_summary.final_rows, 14);
    assert_eq!(row_pinning_summary.pinned_top_rows, 1);
    assert_eq!(row_pinning_summary.pinned_center_rows, 11);
    assert_eq!(row_pinning_summary.pinned_bottom_rows, 2);
    assert!(!row_pinning_summary.row_pinning_page_only);
    assert_eq!(
        row_pinning_summary.visible_rows,
        row_pinning_plan.visible_row_count()
    );
    assert_eq!(
        row_pinning_summary.rendered_rows,
        row_pinning_plan.rendered_row_count()
    );
    assert_eq!(row_pinning_plan.virtualizer().count(), 11);
    assert_eq!(row_pinning_plan.aria_row_count(), 15);
    assert!(row_pinning_plan.uses_split_pinned_layout());
    assert_eq!(
        row_pinning_plan
            .top_rows()
            .iter()
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [("row-pinning-row-003", TableRowRegion::Top, 0)]
    );
    assert_eq!(
        row_pinning_plan
            .center_rows()
            .iter()
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-pinning-row-024", TableRowRegion::Center, 0),
            ("row-pinning-row-025", TableRowRegion::Center, 1),
            ("row-pinning-row-026", TableRowRegion::Center, 2),
            ("row-pinning-row-027", TableRowRegion::Center, 3),
            ("row-pinning-row-028", TableRowRegion::Center, 4),
            ("row-pinning-row-029", TableRowRegion::Center, 5),
            ("row-pinning-row-031", TableRowRegion::Center, 6),
            ("row-pinning-row-032", TableRowRegion::Center, 7),
            ("row-pinning-row-033", TableRowRegion::Center, 8),
            ("row-pinning-row-034", TableRowRegion::Center, 9),
            ("row-pinning-row-035", TableRowRegion::Center, 10),
        ]
    );
    assert_eq!(
        row_pinning_plan
            .bottom_rows()
            .iter()
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-pinning-row-030", TableRowRegion::Bottom, 0),
            ("row-pinning-row-070", TableRowRegion::Bottom, 1),
        ]
    );

    let dependency_tree = table_sample(samples, "dependency-tree");
    let tree_plan = dependency_tree.render_plan();
    let tree_summary = dependency_tree.state_summary();

    assert_eq!(dependency_tree.state.rows().len(), 1);
    assert_eq!(tree_summary.core_rows, 7);
    assert_eq!(tree_summary.final_rows, 4);
    assert_eq!(tree_summary.tree_rows, 4);
    assert_eq!(tree_summary.tree_branch_rows, 3);
    assert_eq!(tree_summary.tree_depth, 1);
    assert_eq!(tree_summary.expanded_tree_inputs, 1);
    assert_eq!(tree_summary.pinned_left_columns, 1);
    assert_eq!(tree_summary.pinned_center_columns, 5);
    assert_eq!(tree_summary.pinned_right_columns, 1);
    assert_eq!(tree_summary.pinned_left_width_px, 220);
    assert_eq!(tree_summary.pinned_center_width_px, 604);
    assert_eq!(tree_summary.pinned_right_width_px, 132);
    assert_eq!(tree_summary.total_column_width_px, 956);
    assert!(tree_plan.uses_split_pinned_layout());
    assert_eq!(tree_plan.aria_column_count(), 7);
    assert_eq!(
        tree_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .map(|row| (
                row.id().as_str(),
                row.depth(),
                row.tree_expanded(),
                row.is_tree_branch()
            ))
            .collect::<Vec<_>>(),
        [
            ("dependency-workspace", 0, Some(true), true),
            ("dependency-ui", 1, Some(false), true),
            ("dependency-core", 1, Some(false), true),
            ("dependency-docs", 1, None, false),
        ]
    );
    assert!(
        tree_plan
            .table()
            .final_model()
            .row(&TableRowId::new("dependency-ui-table"))
            .is_some(),
        "collapsed source-tree descendants should stay addressable by stable row id"
    );

    let server_tree = table_sample(samples, "server-tree");
    let server_plan = server_tree.render_plan();
    let server_summary = server_tree.state_summary();

    assert_eq!(server_tree.state.rows().len(), 3);
    assert_eq!(
        server_tree.state.expansion_mode(),
        TableExpansionMode::Manual
    );
    assert_eq!(server_summary.core_rows, 3);
    assert_eq!(server_summary.final_rows, 3);
    assert_eq!(server_summary.tree_rows, 3);
    assert_eq!(server_summary.tree_branch_rows, 3);
    assert_eq!(server_summary.tree_depth, 0);
    assert_eq!(server_summary.unloaded_tree_branches, 1);
    assert_eq!(server_summary.loading_tree_rows, 1);
    assert_eq!(server_summary.failed_tree_rows, 1);
    assert!(server_summary.manual_expansion);
    assert_eq!(server_summary.expanded_tree_inputs, 0);
    assert_eq!(server_summary.pinned_left_columns, 1);
    assert_eq!(server_summary.pinned_center_columns, 5);
    assert_eq!(server_summary.pinned_right_columns, 1);
    assert_eq!(server_summary.total_column_width_px, 956);
    assert!(server_plan.uses_split_pinned_layout());
    assert_eq!(server_plan.aria_column_count(), 7);
    assert_eq!(server_plan.aria_row_count(), 4);
    assert_eq!(
        server_plan
            .table()
            .final_model()
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["server-workspace", "server-cache", "server-failed"]
    );

    let server_workspace = server_plan
        .table()
        .final_model()
        .row(&TableRowId::new("server-workspace"))
        .expect("server workspace row should resolve");
    let server_cache = server_plan
        .table()
        .final_model()
        .row(&TableRowId::new("server-cache"))
        .expect("server cache row should resolve");
    let server_failed = server_plan
        .table()
        .final_model()
        .row(&TableRowId::new("server-failed"))
        .expect("server failed row should resolve");

    assert!(server_workspace.is_tree_branch());
    assert_eq!(server_workspace.loaded_child_count(), 0);
    assert_eq!(
        server_workspace.children_load_state(),
        Some(&TableRowChildrenLoadState::Idle)
    );
    assert_eq!(server_workspace.tree_expanded(), Some(false));
    assert!(server_cache.is_tree_branch());
    assert_eq!(server_cache.loaded_child_count(), 0);
    assert_eq!(
        server_cache
            .children_load_state()
            .and_then(TableRowChildrenLoadState::message),
        Some("Loading cached modules")
    );
    assert!(
        server_cache
            .children_load_state()
            .is_some_and(TableRowChildrenLoadState::is_loading)
    );
    assert_eq!(server_cache.tree_expanded(), Some(false));
    assert!(server_failed.is_tree_branch());
    assert_eq!(server_failed.loaded_child_count(), 0);
    assert_eq!(
        server_failed
            .children_load_state()
            .and_then(TableRowChildrenLoadState::message),
        Some("Gateway timeout")
    );
    assert!(
        server_failed
            .children_load_state()
            .is_some_and(TableRowChildrenLoadState::is_failed)
    );
    assert_eq!(server_failed.tree_expanded(), Some(false));
}

#[test]
fn components_page_sidebar_samples_expose_navigation_contract() {
    let samples = pages::components::sidebar_samples(ThemeTokens::default());
    let workspace = &samples[0].state;
    let icon = &samples[1].state;
    let long = &samples[2].state;

    assert_eq!(workspace.role(), Role::Navigation);
    assert_eq!(workspace.selected_value(), Some("projects"));
    assert_eq!(workspace.focused_value(), Some("projects"));
    assert_eq!(
        workspace.navigation_target("down").map(|item| item.value()),
        Some("inbox")
    );
    assert_eq!(
        workspace
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("projects".to_string())
    );
    assert!(workspace.items().iter().any(|item| item.disabled()));

    assert_eq!(
        icon.metrics().resolved_width(),
        icon.metrics().collapsed_width()
    );
    assert!(icon.icon_collapsed());
    assert!(icon.items().iter().all(|item| !item.label().is_empty()));

    assert_eq!(long.side().as_str(), "right");
    assert_eq!(long.focused_value(), Some("quality"));
    assert_eq!(
        long.navigation_target("down").map(|item| item.value()),
        Some("alerts")
    );
    assert!(long.scrollable());
}

#[test]
fn components_page_toolbar_samples_expose_roving_focus_contract() {
    let tokens = ThemeTokens::default();
    let samples = pages::components::toolbar_samples(tokens);
    let editor = &samples[0].state;
    let inspector = &samples[1].state;

    assert_eq!(editor.role(), Role::Toolbar);
    assert_eq!(editor.focused_value(), Some("bold"));
    assert_eq!(
        editor.navigation_target("right").map(|item| item.value()),
        Some("italic")
    );
    assert_eq!(
        editor
            .activation_for_key("space")
            .map(|selection| selection.value().to_owned()),
        Some("bold".to_string())
    );
    assert_eq!(editor.items()[2].role(), None);
    assert_eq!(editor.items()[3].toggled(), Some(Toggled::True));
    assert_eq!(inspector.orientation(), Orientation::Vertical);
    assert_eq!(
        inspector.navigation_target("down").map(|item| item.value()),
        Some("refresh")
    );
}

#[test]
fn components_page_choice_samples_expose_listbox_and_select_contracts() {
    let tokens = ThemeTokens::default();
    let listboxes = pages::components::listbox_samples(tokens);
    let selects = pages::components::select_samples(tokens);
    let assignee = &listboxes[0].state;
    let empty = &listboxes[1].state;
    let priority = &selects[0].state;
    let status = &selects[1].state;
    let disabled = &selects[2].state;

    assert_eq!(assignee.role(), Role::ListBox);
    assert_eq!(
        assignee
            .navigation_target("down")
            .map(|option| option.value()),
        Some("owen")
    );
    assert_eq!(
        assignee
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("maya".to_string())
    );
    assert_eq!(
        assignee.typeahead_target("no").map(|option| option.value()),
        Some("nora")
    );
    assert!(assignee.options().iter().any(|option| !option.focusable()));

    assert!(empty.empty());
    assert_eq!(empty.active_value(), None);

    assert_eq!(priority.open_mode(), SelectOpenMode::Controlled);
    assert!(priority.open());
    assert_eq!(priority.selected_value(), Some("critical"));
    assert_eq!(priority.active_value(), Some("normal"));
    assert_ne!(priority.selected_value(), priority.active_value());
    assert_eq!(priority.trigger_label(), "Critical");
    assert_eq!(
        priority.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        priority.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        priority.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(
        priority.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
    assert_eq!(priority.listbox().role(), Role::ListBox);
    assert!(priority.scrollable_content());
    assert!(priority.scroll_area().scrolls_y());

    assert_eq!(status.open_mode(), SelectOpenMode::Uncontrolled);
    assert!(!status.open());
    assert_eq!(status.trigger_label(), "Doing");
    assert_eq!(disabled.trigger_label(), "Unavailable");
    assert!(disabled.disabled());
    assert!(!disabled.overlay().should_render_deferred_layer());
}

#[test]
fn components_page_search_samples_expose_combobox_and_command_contracts() {
    let tokens = ThemeTokens::default();
    let comboboxes = pages::components::combobox_samples(tokens);
    let commands = pages::components::command_samples(tokens);

    let framework = &comboboxes[0].state;
    let empty_combo = &comboboxes[1].state;
    let disabled_combo = &comboboxes[2].state;
    let ranked = &commands[0].state;
    let multi = &commands[1].state;
    let virtualized = &commands[2].state;
    let indexed = &commands[3].state;

    assert_eq!(framework.open_mode(), ComboboxOpenMode::Controlled);
    assert!(framework.open());
    assert_eq!(framework.input_role(), Role::EditableComboBox);
    assert_eq!(framework.content_role(), Role::ListBox);
    assert_eq!(framework.total_option_count(), 5);
    assert_eq!(framework.filtered_option_count(), 3);
    assert_eq!(framework.selected_value(), Some("solid"));
    assert_eq!(framework.listbox().selected_value(), None);
    assert_eq!(framework.active_value(), Some("react"));
    assert_ne!(framework.selected_value(), framework.active_value());
    assert_eq!(framework.listbox().typeahead_query(), Some("re"));
    assert_eq!(
        framework.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );

    assert!(empty_combo.empty());
    assert_eq!(empty_combo.selected_value(), None);
    assert!(empty_combo.listbox().empty());
    assert!(disabled_combo.disabled());
    assert!(!disabled_combo.open());
    assert!(!disabled_combo.input().editable());

    assert_eq!(ranked.open_mode(), CommandOpenMode::Controlled);
    assert!(ranked.open());
    assert_eq!(ranked.input_role(), Role::TextInput);
    assert_eq!(ranked.list_role(), Role::ListBox);
    assert_eq!(ranked.selected_value(), Some("open-file"));
    assert_eq!(ranked.active_value(), Some("open-file"));
    assert_eq!(ranked.filtered_item_count(), 3);
    assert_eq!(ranked.groups().len(), 1);
    assert!(ranked.groups()[0].standalone());
    assert!(ranked.items().iter().any(|item| item.shortcut().is_some()));
    let dialog = ranked.dialog().expect("ranked command is dialog-backed");
    assert!(dialog.open());
    assert_eq!(dialog.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(dialog.description(), Some("Run a workspace command"));

    assert_eq!(multi.selection_mode(), CommandSelectionMode::Multiple);
    assert_eq!(multi.selected_values().len(), 2);
    assert_eq!(multi.selected_chips().len(), 2);
    assert_eq!(multi.filtered_item_count(), 1);
    assert_eq!(virtualized.total_item_count(), 10_000);
    assert_eq!(virtualized.filtered_item_count(), 10_000);
    assert_eq!(virtualized.active_value(), Some("command-0000"));
    assert!(indexed.loading().is_some());
    assert_eq!(indexed.loading().unwrap().role(), Role::ProgressIndicator);
    assert_eq!(indexed.index_revision(), Some("workspace-index-v3"));
    assert_eq!(
        indexed.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(
        indexed
            .items()
            .iter()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "recent-open".to_string(),
            "open-file".to_string(),
            "archive".to_string(),
        ]
    );
}

#[test]
fn components_page_samples_keep_explicit_a11y_metadata() {
    use std::collections::BTreeSet;

    let tokens = ThemeTokens::default();
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let labels = pages::components::label_samples(tokens);

    assert!(
        icon_buttons
            .iter()
            .all(|sample| !sample.state.accessible_label().trim().is_empty())
    );
    assert!(
        icon_buttons
            .iter()
            .all(|sample| sample.state.role() == Role::Button)
    );

    let control_ids = labels
        .iter()
        .filter_map(|sample| sample.state.control_id())
        .collect::<Vec<_>>();
    let unique_control_ids = control_ids.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(
        control_ids,
        vec!["email-input", "terms-checkbox", "disabled-control"]
    );
    assert_eq!(unique_control_ids.len(), control_ids.len());
    assert!(
        labels
            .iter()
            .filter(|sample| sample.state.control_id().is_some())
            .all(|sample| sample.state.associated())
    );
    assert!(
        labels
            .iter()
            .filter(|sample| sample.state.control_id().is_none())
            .all(|sample| !sample.state.associated())
    );
}

#[test]
fn components_page_conformance_gates_reference_core_and_gallery_contracts() {
    let gates = pages::components::CONFORMANCE_GATES;
    let signals = pages::components::SIGNALS;

    assert!(gates.iter().all(|gate| !gate.title.trim().is_empty()));
    assert!(gates.iter().all(|gate| !gate.summary.trim().is_empty()));
    assert!(
        gates
            .iter()
            .all(|gate| gate.evidence.iter().all(|item| !item.trim().is_empty()))
    );
    assert!(gates.iter().any(|gate| gate.id == "scroll-redraw"));
    assert!(gates.iter().any(|gate| gate.id == "tabs-overflow"));
    assert!(gates.iter().any(|gate| gate.id == "table-virtualization"));
    assert!(gates.iter().any(|gate| gate.id == "tree-renderer"));
    assert!(
        gates
            .iter()
            .any(|gate| gate.id == "virtualized-list-renderer")
    );
    assert!(
        gates
            .iter()
            .any(|gate| gate.id == "state-contract-readouts")
    );
    assert!(signals.contains(&"open_gpui_ui_components::StatusCue"));
    assert!(signals.contains(&"open_gpui_ui_components::StatusCueState"));
    assert!(signals.contains(&"open_gpui_ui_components::EmptyState"));
    assert!(signals.contains(&"open_gpui_ui_components::EmptyStateState"));
    assert!(signals.contains(&"open_gpui_ui_components::Listbox"));
    assert!(signals.contains(&"open_gpui_ui_components::ListboxState"));
    assert!(signals.contains(&"open_gpui_ui_components::Select"));
    assert!(signals.contains(&"open_gpui_ui_components::SelectState"));
    assert!(signals.contains(&"open_gpui_ui_components::Combobox"));
    assert!(signals.contains(&"open_gpui_ui_components::ComboboxState"));
    assert!(signals.contains(&"open_gpui_ui_components::Command"));
    assert!(signals.contains(&"open_gpui_ui_components::CommandState"));
    assert!(signals.contains(&"open_gpui_ui_components::Table"));
    assert!(signals.contains(&"open_gpui_ui_components::TableState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableAggregation"));
    assert!(signals.contains(&"open_gpui_ui_components::TableFacetedFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TableFacetedFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TableFacetedFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableGlobalFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TableGlobalFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TableGlobalFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterOperator"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterOperatorOptionState"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRangeFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRangeFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRangeFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableColumnPinning"));
    assert!(signals.contains(&"open_gpui_ui_components::TableColumnRegion"));
    assert!(signals.contains(&"open_gpui_ui_components::TableExpansionState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRowPinning"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRowPinningPolicy"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRowRegion"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRowRegions"));
    assert!(signals.contains(&"open_gpui_ui_components::Tree"));
    assert!(signals.contains(&"open_gpui_ui_components::TreeState"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedList"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedListItemDescriptor"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedListRenderPlan"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizerState"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedListState"));
    assert!(signals.contains(&"Role::ListBox"));
    assert!(signals.contains(&"Role::ListBoxOption"));
    assert!(signals.contains(&"Role::EditableComboBox"));
    assert!(signals.contains(&"Role::ProgressIndicator"));
    assert!(signals.contains(&"Role::Image"));
    assert!(signals.contains(&"Role::Label"));
    assert!(signals.contains(&"Role::Table"));
    assert!(signals.contains(&"Role::Row"));
    assert!(signals.contains(&"Role::ColumnHeader"));
    assert!(signals.contains(&"Role::Cell"));
    assert!(signals.contains(&"Role::Tree"));
    assert!(signals.contains(&"Role::TreeItem"));

    let table_gate = gates
        .iter()
        .find(|gate| gate.id == "table-virtualization")
        .unwrap_or_else(|| panic!("expected table conformance gate"));
    assert!(table_gate.evidence.contains(&"TableFacetedFilter"));
    assert!(table_gate.evidence.contains(&"TableGlobalFilter"));
    assert!(table_gate.evidence.contains(&"TablePredicateFilter"));
    assert!(table_gate.evidence.contains(&"TableRangeFilter"));
    assert!(table_gate.evidence.contains(&"TableColumnWidthPolicy"));
    assert!(table_gate.evidence.contains(&"content-fit-release"));
    assert!(table_gate.evidence.contains(&"toggle-release"));
    assert!(table_gate.evidence.contains(&"select-release"));
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_select_table_cell_updates_sample_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_global_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_predicate_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_faceted_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_range_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_content_fit_table_cell_edit_widens_name_column")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_checkbox_table_cell_updates_sample_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_select_table_cell_updates_sample_rows")
    );
    assert!(table_gate.evidence.contains(&"select-release"));
}

#[open_gpui::test]
fn overlay_gallery_smoke_renders_catalog_entries_and_official_samples(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    for entry in pages::overlay::OVERLAY_CATALOG {
        let catalog_card = bounds(cx, entry.catalog_selector());
        assert!(
            catalog_card.size.width > px(0.0) && catalog_card.size.height > px(0.0),
            "expected Overlay page to render official overlay catalog entry `{}`",
            entry.name
        );
    }

    for (name, selector) in pages::overlay::overlay_sample_selector_pairs() {
        let sample = bounds(cx, selector);
        assert!(
            sample.size.width > px(0.0) && sample.size.height > px(0.0),
            "expected Overlay page to render official {name} sample `{selector}`"
        );
    }
}

#[open_gpui::test]
fn overlay_gallery_smoke_dismisses_popover_from_outside_press(cx: &mut open_gpui::TestAppContext) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "popover:overlay-popover-demo:controlled:trigger");
    click(cx, "popover:overlay-popover-demo:controlled:trigger");
    settle(cx);
    assert!(
        cx.debug_bounds("popover:overlay-popover-demo:controlled:content")
            .is_some(),
        "expected controlled Popover content to open from its real trigger"
    );
    assert!(
        cx.debug_selector_is_focused("popover:overlay-popover-demo:controlled:trigger"),
        "expected controlled Popover trigger to remain focused while opened by default policy"
    );

    let popover_content = bounds(cx, "popover:overlay-popover-demo:controlled:content");
    let outside_target = point(
        popover_content.right() + px(24.0),
        popover_content.bottom() + px(24.0),
    );
    click_point(cx, outside_target);
    settle(cx);

    assert!(
        cx.debug_bounds("popover:overlay-popover-demo:controlled:content")
            .is_none(),
        "expected outside press to dismiss the controlled Popover"
    );
    assert!(
        cx.debug_selector_is_focused("popover:overlay-popover-demo:controlled:trigger"),
        "expected outside-dismissed Popover to restore focus to its trigger"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_opens_tooltip_from_hover_focus_and_ignores_disabled(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "gallery:overlay-tooltip-trigger:hover-focus");
    let hover_trigger = bounds(cx, "gallery:overlay-tooltip-trigger:hover-focus").center();
    cx.simulate_mouse_move(hover_trigger, MouseButton::Left, Default::default());
    redraw(cx);
    assert!(
        cx.debug_bounds("tooltip:overlay-tooltip-content:hover-focus:content")
            .is_some(),
        "expected hover tooltip content to open from pointer hover"
    );

    let outside_target = bounds(cx, "gallery:content").center();
    cx.simulate_mouse_move(outside_target, MouseButton::Left, Default::default());
    redraw(cx);
    assert!(
        cx.debug_bounds("tooltip:overlay-tooltip-content:hover-focus:content")
            .is_none(),
        "expected hover tooltip content to dismiss after leaving the trigger"
    );

    scroll_page_until_visible(cx, "gallery:overlay-tooltip-trigger:focus-only");
    click(cx, "gallery:overlay-tooltip-trigger:focus-only");
    redraw(cx);
    assert!(
        cx.debug_bounds("tooltip:overlay-tooltip-content:focus-only:content")
            .is_some(),
        "expected focus-only tooltip content to open from keyboard focus"
    );

    let content_center = bounds(cx, "gallery:content").center();
    click_point(cx, content_center);
    redraw(cx);
    assert!(
        cx.debug_bounds("tooltip:overlay-tooltip-content:focus-only:content")
            .is_none(),
        "expected focus-only tooltip content to dismiss after focus leaves"
    );

    scroll_page_until_visible(cx, "gallery:overlay-tooltip-trigger:disabled");
    let disabled_trigger = bounds(cx, "gallery:overlay-tooltip-trigger:disabled").center();
    cx.simulate_mouse_move(disabled_trigger, MouseButton::Left, Default::default());
    redraw(cx);
    assert!(
        cx.debug_bounds("tooltip:overlay-tooltip-content:disabled:content")
            .is_none(),
        "expected disabled tooltip trigger to stay closed"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_renders_manual_tooltip_from_state(cx: &mut open_gpui::TestAppContext) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "gallery:overlay-tooltip-sample:delayed-manual");
    redraw(cx);
    assert!(
        cx.debug_bounds("tooltip:overlay-tooltip-content:delayed-manual:content")
            .is_some(),
        "expected manual delayed tooltip content to render from gallery state"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(
        cx,
        "hover-card:overlay-hover-card-demo:manual-controlled:trigger",
    );
    click(
        cx,
        "hover-card:overlay-hover-card-demo:manual-controlled:trigger",
    );
    settle(cx);
    assert!(
        cx.debug_bounds("hover-card:overlay-hover-card-demo:manual-controlled:content")
            .is_some(),
        "expected controlled HoverCard content to open from its real trigger"
    );

    let hover_card_content = bounds(
        cx,
        "hover-card:overlay-hover-card-demo:manual-controlled:content",
    );
    let outside_target = point(
        hover_card_content.right() + px(24.0),
        hover_card_content.bottom() + px(24.0),
    );
    click_point(cx, outside_target);
    settle(cx);

    assert!(
        cx.debug_bounds("hover-card:overlay-hover-card-demo:manual-controlled:content")
            .is_none(),
        "expected outside press to dismiss the controlled HoverCard"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_toggles_hover_card_from_control_surface(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "gallery:overlay-hover-card-controlled-toggle");
    click(cx, "gallery:overlay-hover-card-controlled-toggle");
    settle(cx);
    assert!(
        cx.debug_bounds("hover-card:overlay-hover-card-demo:manual-controlled:content")
            .is_some(),
        "expected the controlled HoverCard toggle to open its content"
    );

    press_escape(cx);
    assert!(
        cx.debug_bounds("hover-card:overlay-hover-card-demo:manual-controlled:content")
            .is_none(),
        "expected Escape to close the controlled HoverCard after opening it from the toggle"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "dialog:overlay-dialog-demo:controlled-modal:trigger");
    click(cx, "dialog:overlay-dialog-demo:controlled-modal:trigger");
    settle(cx);
    let dialog_layer = bounds(cx, "dialog:overlay-dialog-demo:controlled-modal:layer");
    assert!(
        cx.debug_bounds("dialog:overlay-dialog-demo:controlled-modal:surface")
            .is_some(),
        "expected controlled Dialog surface to open from its real trigger"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:overlay-dialog-demo:controlled-modal:surface"),
        "expected opened Dialog to move focus to its first focusable surface"
    );

    click_point(cx, outside_top_left(dialog_layer));
    settle(cx);
    assert!(
        cx.debug_bounds("dialog:overlay-dialog-demo:controlled-modal:surface")
            .is_none(),
        "expected modal barrier outside press to dismiss the controlled Dialog"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:overlay-dialog-demo:controlled-modal:trigger"),
        "expected barrier-dismissed Dialog to restore focus to its trigger"
    );

    click(cx, "dialog:overlay-dialog-demo:controlled-modal:trigger");
    settle(cx);
    assert!(
        cx.debug_bounds("dialog:overlay-dialog-demo:controlled-modal:surface")
            .is_some(),
        "expected controlled Dialog surface to reopen after barrier dismissal"
    );
    press_escape(cx);
    assert!(
        cx.debug_bounds("dialog:overlay-dialog-demo:controlled-modal:surface")
            .is_none(),
        "expected Escape to dismiss the controlled Dialog"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:overlay-dialog-demo:controlled-modal:trigger"),
        "expected Escape-dismissed Dialog to restore focus to its trigger"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let trigger = "alert-dialog:overlay-alert-dialog-demo:destructive-confirm:trigger";
    let surface = "alert-dialog:overlay-alert-dialog-demo:destructive-confirm:surface";
    let cancel = "alert-dialog:overlay-alert-dialog-demo:destructive-confirm:cancel";
    let action = "alert-dialog:overlay-alert-dialog-demo:destructive-confirm:action";

    scroll_page_until_visible(cx, trigger);
    click(cx, trigger);
    settle(cx);
    assert!(
        cx.debug_bounds(surface).is_some(),
        "expected controlled AlertDialog surface to open from its real trigger"
    );
    assert!(
        cx.debug_selector_is_focused(cancel),
        "expected opened AlertDialog to move focus to the default cancel action"
    );

    click(cx, action);
    settle(cx);
    assert!(
        cx.debug_bounds(surface).is_none(),
        "expected the primary action to close the controlled AlertDialog"
    );
    assert!(
        cx.debug_selector_is_focused(trigger),
        "expected action-dismissed AlertDialog to restore focus to its trigger"
    );

    click(cx, trigger);
    settle(cx);
    assert!(
        cx.debug_bounds(surface).is_some(),
        "expected controlled AlertDialog surface to reopen after gallery control dismissal"
    );
    press_escape(cx);
    assert!(
        cx.debug_bounds(surface).is_none(),
        "expected Escape to dismiss the controlled AlertDialog"
    );
    assert!(
        cx.debug_selector_is_focused(trigger),
        "expected Escape-dismissed AlertDialog to restore focus to its trigger"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "gallery:overlay-sheet-control:right-non-modal");
    click(cx, "gallery:overlay-sheet-control:right-non-modal");
    assert!(
        cx.debug_bounds("sheet:overlay-sheet-demo:right-non-modal:surface")
            .is_some(),
        "expected controlled non-modal Sheet surface to open from its real trigger"
    );

    let outside_target = bounds(cx, "gallery:content").center();
    click_point(cx, outside_target);

    assert!(
        cx.debug_bounds("sheet:overlay-sheet-demo:right-non-modal:surface")
            .is_none(),
        "expected outside press to dismiss the controlled non-modal Sheet"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_menu_from_escape_and_outside_press(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "gallery:overlay-menu-control:controlled");
    click(cx, "gallery:overlay-menu-control:controlled");
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:controlled:content")
            .is_some(),
        "expected controlled Menu content to open from its real trigger"
    );
    press_escape(cx);
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:controlled:content")
            .is_none(),
        "expected Escape to dismiss the controlled Menu"
    );

    click(cx, "gallery:overlay-menu-control:controlled");
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:controlled:content")
            .is_some(),
        "expected controlled Menu content to reopen after Escape"
    );
    let outside_target = bounds(cx, "gallery:content").center();
    click_point(cx, outside_target);
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:controlled:content")
            .is_none(),
        "expected outside press to dismiss the controlled Menu"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_opens_menu_submenu_from_hover(cx: &mut open_gpui::TestAppContext) {
    let cx = open_overlay_gallery(cx);

    scroll_page_until_visible(cx, "gallery:overlay-menu-sample:rich-items");
    click(cx, "menu:overlay-menu-demo:rich-items:trigger");
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort")
            .is_some(),
        "expected the rich menu submenu trigger to render after opening the menu"
    );
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort/0:name")
            .is_none(),
        "expected rich menu submenu child to start hidden before hover"
    );

    let sort = bounds(cx, "menu:overlay-menu-demo:rich-items:item:3:sort").center();
    cx.simulate_mouse_move(sort, None, Default::default());
    redraw(cx);

    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort/0:name")
            .is_none(),
        "expected hovering the rich menu submenu trigger to keep its child rows hidden before the hover delay"
    );

    advance_and_redraw(cx, Duration::from_millis(200));
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort/0:name")
            .is_some(),
        "expected the rich menu submenu child to render after the hover delay"
    );

    let child = bounds(cx, "menu:overlay-menu-demo:rich-items:item:3:sort/0:name").center();
    cx.simulate_mouse_move(child, None, Default::default());
    redraw(cx);
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort/0:name")
            .is_some(),
        "expected moving into the submenu child to keep the branch open"
    );

    let group = bounds(cx, "menu:overlay-menu-demo:rich-items:item:4:group").center();
    cx.simulate_mouse_move(group, None, Default::default());
    redraw(cx);
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:4:group/0:kind")
            .is_none(),
        "expected hovering another submenu trigger to keep its child rows hidden before the hover delay"
    );
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort/0:name")
            .is_some(),
        "expected switching submenu triggers to keep the previous branch visible until the new hover delay elapses"
    );

    advance_and_redraw(cx, Duration::from_millis(200));
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:4:group/0:kind")
            .is_some(),
        "expected hovering another submenu trigger to open its branch after the hover delay"
    );
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:3:sort/0:name")
            .is_none(),
        "expected switching submenu triggers to close the previous branch after the hover delay"
    );

    let root_item = bounds(cx, "menu:overlay-menu-demo:rich-items:item:0:show-hidden").center();
    cx.simulate_mouse_move(root_item, None, Default::default());
    redraw(cx);
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:4:group/0:kind")
            .is_some(),
        "expected hovering another root item to keep the rich menu submenu branch visible until the close delay elapses"
    );

    advance_and_redraw(cx, Duration::from_millis(200));
    assert!(
        cx.debug_bounds("menu:overlay-menu-demo:rich-items:item:4:group/0:kind")
            .is_none(),
        "expected hovering another root item to close the rich menu submenu branch after the close delay"
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let hotspot = "context-menu:overlay-context-menu-demo:controlled:hotspot";
    let surface = "context-menu:overlay-context-menu-demo:controlled:surface";

    scroll_page_until_visible(cx, hotspot);
    right_click(cx, hotspot);
    assert!(
        cx.debug_bounds(surface).is_some(),
        "expected controlled ContextMenu surface to open from right-clicking its real hotspot"
    );

    press_escape(cx);

    assert!(
        cx.debug_bounds(surface).is_none(),
        "expected Escape to dismiss the controlled ContextMenu"
    );

    right_click(cx, hotspot);
    let surface_bounds = bounds(cx, surface);
    let outside_target = visible_outside_point(bounds(cx, "gallery:content"), surface_bounds);
    click_point(cx, outside_target);

    assert!(
        cx.debug_bounds(surface).is_none(),
        "expected outside press to dismiss the controlled ContextMenu"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    assert!(
        cx.debug_bounds(
            &pages::components::COMPONENT_CATALOG
                .iter()
                .find(|entry| entry.name == "Button")
                .unwrap_or_else(|| panic!("expected catalog entry `Button`"))
                .catalog_selector()
        )
        .is_some(),
        "expected Components page to render official component catalog entries"
    );
    assert!(
        cx.debug_bounds(
            &pages::components::COMPONENT_CATALOG
                .iter()
                .find(|entry| entry.name == "TextInputController")
                .unwrap_or_else(|| panic!("expected catalog entry `TextInputController`"))
                .catalog_selector()
        )
        .is_some(),
        "expected Components page to classify adapter-only public surfaces"
    );
    assert!(
        cx.debug_bounds(
            &pages::components::COMPONENT_CATALOG
                .iter()
                .find(|entry| entry.name == "Avatar")
                .unwrap_or_else(|| panic!("expected catalog entry `Avatar`"))
                .catalog_selector()
        )
        .is_some(),
        "expected Components page to show official primitive entries"
    );
    assert!(
        cx.debug_bounds(
            &pages::components::COMPONENT_CATALOG
                .iter()
                .find(|entry| entry.name == "AvatarGroup")
                .unwrap_or_else(|| panic!("expected catalog entry `AvatarGroup`"))
                .catalog_selector()
        )
        .is_some(),
        "expected Components page to show AvatarGroup as an official primitive entry"
    );
    for (name, selector) in pages::components::official_sample_selector_pairs() {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected Components page to render official {name} sample `{selector}`"
        );
    }
    for (name, selector) in pages::components::state_contract_readout_pairs() {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected Components page to render state-contract {name} readout `{selector}`"
        );
    }
    for selector in [
        "separator:component-separator:section-rule:root",
        "kbd:component-kbd:command-palette:root",
        "progress:component-progress:sync:root",
        "skeleton:component-skeleton:body-line:root",
        "avatar:component-avatar:ada:root",
        "gallery:component-avatar-group-sample:team",
        "status-cue:component-status-cue:sync-warning:root",
        "empty-state:component-empty-state:no-results:root",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected Components page to render real component root `{selector}`"
        );
    }

    jump_components_directory_to(cx, "gallery:component-page-jump:tabs");
    let tabs_sample =
        scroll_page_selector_into_view(&shell, cx, "gallery:component-tabs-sample:workspace-tabs");
    let page_scroll = bounds(cx, "gallery:page-scroll");

    assert!(
        bounds_overlap_y(page_scroll, tabs_sample),
        "expected full Components page to scroll until the vertical Tabs sample is visible"
    );
    let tokens_navigation = bounds(cx, "gallery:navigation-item:tokens").center();
    cx.simulate_click(tokens_navigation, Default::default());
    redraw(cx);
    let components_navigation =
        scroll_navigation_until_visible(cx, "gallery:navigation-item:components").center();
    cx.simulate_click(components_navigation, Default::default());
    redraw(cx);

    let reset_page_scroll = bounds(cx, "gallery:page-scroll");
    if let Some(tabs_after_reset) = cx.debug_bounds("gallery:component-tabs-sample:workspace-tabs")
    {
        assert!(
            !reset_page_scroll.contains(&tabs_after_reset.center()),
            "expected switching away and back to Components to reset page scroll so deep Tabs sample is no longer visible; tabs={tabs_after_reset:?} page={reset_page_scroll:?}"
        );
    }
}

#[open_gpui::test]
fn components_gallery_smoke_focuses_catalog_family_and_restores_all_mode(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    assert!(
        cx.debug_bounds("gallery:component-table-sample:release-queue")
            .is_some(),
        "expected focused Table mode to render the Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:release-rollup")
            .is_some(),
        "expected focused Table mode to render the grouped Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:release-resize")
            .is_some(),
        "expected focused Table mode to render the resizable Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:editable-release")
            .is_some(),
        "expected focused Table mode to render the editable Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:toggle-release")
            .is_some(),
        "expected focused Table mode to render the checkbox Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:select-release")
            .is_some(),
        "expected focused Table mode to render the select Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:content-fit-release")
            .is_some(),
        "expected focused Table mode to render the content-fit Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:release-matrix")
            .is_some(),
        "expected focused Table mode to render the wide matrix Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:grouped-custom-aggregation")
            .is_some(),
        "expected focused Table mode to render the custom aggregation Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-table-sample:row-pinning")
            .is_some(),
        "expected focused Table mode to render the row-pinning Table sample"
    );
    assert!(
        cx.debug_bounds("gallery:component-button-sample:default")
            .is_none(),
        "expected focused Table mode to hide unrelated Button samples"
    );
    assert!(
        cx.debug_bounds("gallery:component-tabs-sample:workspace-tabs")
            .is_none(),
        "expected focused Table mode to hide sibling Field-group samples"
    );
    assert!(
        cx.debug_bounds("gallery:components-directory").is_some(),
        "expected focused mode to preserve the section directory"
    );

    click(cx, "gallery:component-focus:all");
    settle(cx);

    assert_eq!(
        shell_snapshot(&shell, cx).components_focus,
        pages::components::ComponentFocusMode::All
    );
    assert!(
        cx.debug_bounds("gallery:component-button-sample:default")
            .is_some(),
        "expected all-components mode to restore Button samples"
    );
    assert!(
        cx.debug_bounds("gallery:component-tabs-sample:workspace-tabs")
            .is_some(),
        "expected all-components mode to restore nested Tabs samples"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_focuses_every_focusable_catalog_entry(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let mut visited = Vec::new();
    let expected = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| pages::components::focused_section_for_catalog_entry(entry).is_some())
        .count();

    for entry in pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| pages::components::focused_section_for_catalog_entry(entry).is_some())
    {
        focus_components_section(&shell, cx, entry);
        visited.push(entry.name);
    }

    click(cx, "gallery:component-focus:all");
    settle(cx);

    assert_eq!(
        shell_snapshot(&shell, cx).components_focus,
        pages::components::ComponentFocusMode::All,
        "expected `All components` to restore all-mode after matrix traversal"
    );
    for selector in [
        "gallery:component-button-sample:default",
        "gallery:component-tabs-sample:workspace-tabs",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected all-mode restoration after matrix traversal to render `{selector}`"
        );
    }

    assert_eq!(
        visited.len(),
        expected,
        "expected focused catalog matrix to cover every focusable catalog entry"
    );
    assert!(
        visited.contains(&"TreeState") && visited.contains(&"VirtualizedListState"),
        "expected focused catalog matrix to include state-contract entries; visited={visited:?}"
    );
    assert!(
        !visited.contains(&"TextInputController"),
        "expected focused catalog matrix to exclude adapter-only helpers; visited={visited:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_focused_table_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    scroll_page_selector_into_view(&shell, cx, "component-catalog:Table");
    click(cx, "component-catalog:Table");
    settle(cx);
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:table:component-table:release-queue:body-scroll",
    );

    let sample_before = bounds(cx, "gallery:component-table-sample:release-queue");
    let table_viewport = bounds(
        cx,
        "scroll-area:table:component-table:release-queue:body-scroll",
    );

    assert!(
        cx.debug_bounds("table:component-table:release-queue:row:release-queue-row-0000")
            .is_some(),
        "expected the focused Table window to render the first row"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: table_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-queue");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected focused Table viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds("table:component-table:release-queue:row:release-queue-row-0000")
            .is_none(),
        "expected focused virtualized Table row 0000 to leave the rendered window after internal scroll"
    );
    assert!(
        cx.debug_bounds("table:component-table:release-queue:row:release-queue-row-0010")
            .is_some(),
        "expected focused virtualized Table row 0010 to enter the rendered window after internal scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_textarea_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-textarea-sample:overflow";
    const VIEWPORT: &str = "textarea:component-textarea:overflow:root";
    const LINE: &str = "textarea:component-textarea:overflow:line:2";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    scroll_page_selector_into_view(&shell, cx, "component-catalog:Textarea");
    click(cx, "component-catalog:Textarea");
    settle(cx);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);

    let sample_before = bounds(cx, SAMPLE);
    let line_before = bounds(cx, LINE);
    let viewport = bounds(cx, VIEWPORT);

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let line_after = bounds(cx, LINE);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Textarea wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        line_after.top() < line_before.top(),
        "expected Textarea wheel input to move the inner multiline content; before={line_before:?} after={line_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_focused_command_samples_cover_depth_behaviors(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let command_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Command")
        .unwrap_or_else(|| panic!("expected catalog entry `Command`"));
    focus_components_section(&shell, cx, command_entry);

    for selector in [
        "gallery:component-command-sample:ranked-search",
        "gallery:component-command-sample:multi-select",
        "gallery:component-command-sample:virtualized-index",
        "gallery:component-command-sample:indexed-loading",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected focused Command mode to render `{selector}`"
        );
    }

    assert!(
        cx.debug_bounds("command:component-command:multi-select:selected-chip:open-file")
            .is_some(),
        "expected multi-select Command sample to render a hidden selected chip"
    );
    assert!(
        cx.debug_bounds("command:component-command:multi-select:selected-chip:new-file")
            .is_some(),
        "expected multi-select Command sample to render a visible selected chip"
    );
    assert!(
        cx.debug_bounds("command:component-command:indexed-loading:content")
            .is_some(),
        "expected indexed/loading Command sample to render inline content"
    );

    let virtualized_sample = bounds(cx, "gallery:component-command-sample:virtualized-index");
    let command_viewport = bounds(cx, "scroll-area:Virtualized commands:command-list-scroll");

    assert!(
        cx.debug_bounds("command:component-command:virtualized-index:row:command-0000")
            .is_some(),
        "expected initial virtualized Command row to render"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: command_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-520.0))),
        ..Default::default()
    });
    redraw(cx);

    let virtualized_after = bounds(cx, "gallery:component-command-sample:virtualized-index");

    assert_eq!(
        virtualized_after.top(),
        virtualized_sample.top(),
        "expected focused Command viewport wheel input to stay inside the sample"
    );
    assert!(
        cx.debug_bounds("command:component-command:virtualized-index:row:command-0010")
            .is_some(),
        "expected virtualized Command overscan rows to stay bounded and inspectable"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_focused_mode_resets_page_on_family_change(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    scroll_page_until_visible(cx, "component-catalog:Table");
    click(cx, "component-catalog:Table");
    settle(cx);
    scroll_page_until_visible(cx, "gallery:component-table-sample:release-queue");
    let table_sample = bounds(cx, "gallery:component-table-sample:release-queue");
    let page_scroll = bounds(cx, "gallery:page-scroll");
    assert!(
        page_scroll.contains(&table_sample.center()),
        "expected focused Table sample to become visible after page scroll"
    );

    click(cx, "gallery:component-focus:all");
    settle(cx);
    assert_eq!(
        shell_snapshot(&shell, cx).components_focus,
        pages::components::ComponentFocusMode::All
    );
    let reset_page_scroll = bounds(cx, "gallery:page-scroll");
    if let Some(table_after_reset) = cx.debug_bounds("gallery:component-table-sample:release-queue")
    {
        assert!(
            !reset_page_scroll.contains(&table_after_reset.center()),
            "expected returning to all-components mode to reset page scroll; table={table_after_reset:?} page={reset_page_scroll:?}"
        );
    }
}

#[open_gpui::test]
fn gallery_smoke_compact_shell_scrolls_navigation_and_resets_page_on_navigation(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Tokens);

    click(cx, "gallery:viewport-switch:compact");
    cx.simulate_resize(size(px(720.0), px(520.0)));
    redraw(cx);

    let compact = shell_snapshot(&shell, cx);
    assert_eq!(compact.selected_page, GalleryPage::Tokens);
    assert_eq!(compact.shell_mode, DeviceShellMode::Mobile);
    assert_eq!(compact.density, Density::Compact);
    assert_eq!(compact.control_size, Size::Small);

    scroll_navigation_until_visible(cx, "gallery:navigation-item:components");
    click(cx, "gallery:navigation-item:components");
    assert_eq!(
        shell_snapshot(&shell, cx).selected_page,
        GalleryPage::Components
    );

    let directory_viewport = bounds(cx, "scroll-area:gallery-components-directory-scroll");
    scroll_until_visible(
        cx,
        "scroll-area:gallery-components-directory-scroll",
        "gallery:component-page-jump:tree",
        16,
        point(px(0.0), px(-48.0)),
        directory_viewport.center(),
        |container, target| container.contains(&target.center()),
        "expected compact Components directory to reveal the Tree jump".to_string(),
    );
    click(cx, "gallery:component-page-jump:tree");
    settle(cx);
    settle(cx);

    let tree_sample = bounds(cx, "gallery:component-tree-sample:document-outline");
    let page_scroll = bounds(cx, "gallery:page-scroll");
    assert!(
        bounds_overlap_y(page_scroll, tree_sample),
        "expected compact Components page to scroll until the Tree sample is visible"
    );

    scroll_navigation_until_visible(cx, "gallery:navigation-item:overlay");
    click(cx, "gallery:navigation-item:overlay");
    assert_eq!(
        shell_snapshot(&shell, cx).selected_page,
        GalleryPage::Overlay
    );
    assert!(
        cx.debug_bounds("gallery:overlay-page").is_some(),
        "expected compact navigation to switch to the Overlay page"
    );

    scroll_navigation_until_visible(cx, "gallery:navigation-item:components");
    click(cx, "gallery:navigation-item:components");
    assert_eq!(
        shell_snapshot(&shell, cx).selected_page,
        GalleryPage::Components
    );

    let reset_page_scroll = bounds(cx, "gallery:page-scroll");
    if let Some(tree_after_reset) =
        cx.debug_bounds("gallery:component-tree-sample:document-outline")
    {
        assert!(
            !bounds_overlap_y(reset_page_scroll, tree_after_reset),
            "expected compact navigation to reset page scroll after switching away and back; tree={tree_after_reset:?} page={reset_page_scroll:?}"
        );
    }
}

#[open_gpui::test]
fn components_gallery_smoke_closes_select_popup_from_outside_press(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:select");
    scroll_page_selector_into_view(&shell, cx, "select:component-select:status-select:trigger");
    let select_trigger = bounds(cx, "select:component-select:status-select:trigger").center();
    cx.simulate_click(select_trigger, Default::default());
    redraw(cx);

    assert!(
        cx.debug_bounds("select:Status:select-content-scroll:content")
            .is_some(),
        "expected status Select popup content to open from the gallery trigger"
    );

    let outside_target = bounds(cx, "gallery:content").center();
    cx.simulate_click(outside_target, Default::default());
    redraw(cx);

    assert!(
        cx.debug_bounds("select:Status:select-content-scroll:content")
            .is_none(),
        "expected outside press in the gallery to dismiss the Select popup"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_scroll_area_samples_scroll_inside_page(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    let samples = pages::components::scroll_area_samples(ThemeTokens::default());
    let data_grid = samples
        .iter()
        .find(|sample| sample.id == "data-grid")
        .unwrap_or_else(|| panic!("expected scroll area sample `data-grid`"));

    assert_eq!(data_grid.state.axis(), ScrollAreaAxis::Both);
    assert_eq!(
        data_grid.state.reset_policy(),
        ScrollResetPolicy::ResetOnKeyChange
    );
    assert_eq!(data_grid.state.reset_key(), Some("components"));
    assert!(data_grid.state.scrolls_x());
    assert!(data_grid.state.scrolls_y());
    assert_eq!(data_grid.items.len(), 7);

    scroll_page_until_visible(cx, "gallery:component-scroll-area-sample:release-queue");
    let queue_before = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");
    let queue_viewport = bounds(cx, "scroll-area:component-scroll-area:release-queue");

    cx.simulate_event(ScrollWheelEvent {
        position: queue_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-72.0), px(0.0))),
        ..Default::default()
    });
    redraw(cx);

    let queue_after = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");
    assert!(
        queue_after.left() < queue_before.left(),
        "expected the gallery release queue ScrollArea to scroll horizontally inside its viewport; before={queue_before:?} after={queue_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_release_queue_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "gallery:component-scroll-area-sample:release-queue");
    let sample_before = bounds(cx, "gallery:component-scroll-area-sample:release-queue");
    let queue_before = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");
    let queue_viewport = bounds(cx, "scroll-area:component-scroll-area:release-queue");

    cx.simulate_event(ScrollWheelEvent {
        position: queue_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-56.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-scroll-area-sample:release-queue");
    let queue_after = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected the release queue sample card to stay fixed while the inner viewport scrolls; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        queue_after.left() < queue_before.left(),
        "expected the release queue viewport to move horizontally inside the sample; before={queue_before:?} after={queue_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_release_queue_card_wheel_does_not_leak_to_page(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "gallery:component-scroll-area-sample:release-queue");
    let sample_before = bounds(cx, "gallery:component-scroll-area-sample:release-queue");
    let queue_before = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");

    cx.simulate_event(ScrollWheelEvent {
        position: point(
            sample_before.left() + px(24.0),
            sample_before.top() + px(24.0),
        ),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-56.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-scroll-area-sample:release-queue");
    let queue_after = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected the release queue sample card to keep wheel input local to the card chrome; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        queue_after, queue_before,
        "expected wheel input on the release queue card chrome to leave the inner viewport unchanged; before={queue_before:?} after={queue_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_table_scroll_stays_inside_sample(cx: &mut open_gpui::TestAppContext) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:table");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:table:component-table:release-queue:body-scroll",
    );
    let sample_before = bounds(cx, "gallery:component-table-sample:release-queue");
    let table_viewport = bounds(
        cx,
        "scroll-area:table:component-table:release-queue:body-scroll",
    );

    assert!(
        cx.debug_bounds("table:component-table:release-queue:row:release-queue-row-0000")
            .is_some(),
        "expected the initial release queue table window to render the first row"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: table_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-queue");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Table viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds("table:component-table:release-queue:row:release-queue-row-0000")
            .is_none(),
        "expected virtualized Table row 0000 to leave the rendered window after internal scroll"
    );
    assert!(
        cx.debug_bounds("table:component-table:release-queue:row:release-queue-row-0010")
            .is_some(),
        "expected virtualized Table row 0010 to enter the rendered window after internal scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_resizable_table_resize_updates_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());

    jump_components_directory_to(cx, "gallery:component-page-jump:table");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "table:component-table:release-resize:resize:name",
    );
    let sample_before = bounds(cx, "gallery:component-table-sample:release-resize");
    let header_before = bounds(cx, "table:component-table:release-resize:header:name");
    let cell_before = bounds(
        cx,
        "table:component-table:release-resize:cell:release-resize-row-000:name",
    );
    let resize_handle = bounds(cx, "table:component-table:release-resize:resize:name");

    assert_eq!(header_before.size.width, cell_before.size.width);
    assert!(
        cx.debug_bounds("table:component-table:release-resize:resize:score")
            .is_none(),
        "expected the score column to stay non-resizable"
    );

    drag(
        cx,
        resize_handle.center(),
        point(
            resize_handle.center().x + px(60.0),
            resize_handle.center().y,
        ),
    );

    let sample_after = bounds(cx, "gallery:component-table-sample:release-resize");
    let header_after = bounds(cx, "table:component-table:release-resize:header:name");
    let cell_after = bounds(
        cx,
        "table:component-table:release-resize:cell:release-resize-row-000:name",
    );
    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.sizing_changes().to_vec()
    });
    let committed_width =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.committed_sizing("release-resize")
                .and_then(|sizing| sizing.width(&TableColumnId::new("name")))
        });

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Table resize drag to keep the sample card anchored"
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, "release-resize");
    assert_eq!(changes[0].column_id, "name");
    assert!(changes[0].width > ui_px(188.0));
    assert_eq!(committed_width, Some(changes[0].width));
    assert_eq!(header_after.size.width, cell_after.size.width);
    assert!(header_after.size.width > header_before.size.width);
}

#[open_gpui::test]
fn components_gallery_smoke_faceted_filter_updates_table_rows(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TRIGGER: &str = "popover:component-table-faceted-filter:filter-board:status:trigger";
    const CONTENT: &str =
        "table-faceted-filter:component-table-faceted-filter:filter-board:status:content";
    const DONE_OPTION: &str =
        "table-faceted-filter:component-table-faceted-filter:filter-board:status:option:Done";
    const INITIAL_ROW: &str = "table:component-table:filter-board:row:filter-board-row-177";
    const FILTERED_ROW: &str = "table:component-table:filter-board:row:filter-board-row-171";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, TRIGGER);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_some(),
        "expected the initial filtered board row to render before selecting a status facet"
    );

    click(cx, TRIGGER);
    settle(cx);
    if cx.debug_bounds(CONTENT).is_none() {
        click(cx, TRIGGER);
        settle(cx);
    }
    let popup_content = bounds(cx, CONTENT);
    cx.simulate_event(ScrollWheelEvent {
        position: popup_content.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-180.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after_popup_wheel = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after_popup_wheel.top(),
        sample_before.top(),
        "expected faceted-filter popup wheel input to stay inside the table sample"
    );

    click(cx, DONE_OPTION);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.faceted_filter_changes().to_vec()
    });
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, "filter-board");
    assert_eq!(changes[0].column_id, "status");
    assert_eq!(changes[0].selected_values, vec!["Done".to_owned()]);
    assert_eq!(changes[0].toggled_value, Some("Done".to_owned()));
    assert!(changes[0].selected);
    assert_eq!(changes[0].filtered_rows, 15);
    assert_eq!(changes[0].final_rows, 15);
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_none(),
        "expected the Doing row to leave the rendered window after selecting Done"
    );
    assert!(
        cx.debug_bounds(FILTERED_ROW).is_some(),
        "expected the highest-scoring Done row to render after selecting Done"
    );

    if cx.debug_bounds(DONE_OPTION).is_none() {
        click(cx, TRIGGER);
        settle(cx);
    }
    click(cx, DONE_OPTION);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.faceted_filter_changes().to_vec()
    });
    assert_eq!(changes.len(), 2);
    assert!(changes[1].selected_values.is_empty());
    assert_eq!(changes[1].toggled_value, Some("Done".to_owned()));
    assert!(!changes[1].selected);
    assert_eq!(changes[1].filtered_rows, 60);
    assert_eq!(changes[1].final_rows, 24);
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_some(),
        "expected clearing the status facet to restore the original filtered board rows"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_global_filter_updates_table_rows(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE_ID: &str = "filter-board";
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TOOLBAR: &str = "table-toolbar:component-table-toolbar:filter-board:root";
    const INPUT: &str = "text-input:component-table-global-filter:filter-board-input:root";
    const INITIAL_ROW: &str = "table:component-table:filter-board:row:filter-board-row-177";
    const FILTERED_ROW: &str = "table:component-table:filter-board:row:filter-board-row-012";

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let expected_state = TableGlobalFilterChange::new("012").apply_to(sample.state.clone());
    let expected = expected_state.resolve();
    let expected_filtered_rows = expected.filtered_model().rows().len();
    let expected_final_rows = expected.final_model().rows().len();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, INPUT);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(TOOLBAR).is_some(),
        "expected filter-board controls to render inside the table toolbar recipe"
    );
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_some(),
        "expected the initial filtered board row to render before applying a global search"
    );

    click(cx, INPUT);
    settle(cx);
    cx.simulate_input("012");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected the global-search input to stay inside the table sample"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.global_filter_changes().to_vec()
    });
    assert!(!changes.is_empty());
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one global-filter change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.query, "012");
    assert!(!last.cleared);
    assert_eq!(last.filtered_rows, expected_filtered_rows);
    assert_eq!(last.final_rows, expected_final_rows);

    let persisted = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.global_filter_override(SAMPLE_ID)
            .and_then(|state| state.global_filter().map(str::to_owned))
    });
    assert_eq!(persisted.as_deref(), Some("012"));
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_none(),
        "expected the initial board row to leave the rendered window after applying global search"
    );
    assert!(
        cx.debug_bounds(FILTERED_ROW).is_some(),
        "expected the matching board row to render after applying global search"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_predicate_filter_updates_table_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "filter-board";
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TOOLBAR: &str = "table-toolbar:component-table-toolbar:filter-board:root";
    const INPUT: &str = "text-input:component-table-predicate-filter:filter-board:name-value:root";
    const INITIAL_ROW: &str = "table:component-table:filter-board:row:filter-board-row-177";
    const FILTERED_ROW: &str = "table:component-table:filter-board:row:filter-board-row-012";

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let expected_state = TablePredicateFilterChange::new(
        "name",
        TablePredicateFilterOperator::text(TableTextFilterOperator::Contains),
        "012",
    )
    .apply_to(sample.state.clone());
    let expected = expected_state.resolve();
    let expected_filtered_rows = expected.filtered_model().rows().len();
    let expected_final_rows = expected.final_model().rows().len();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, INPUT);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(TOOLBAR).is_some(),
        "expected filter-board controls to render inside the table toolbar recipe"
    );
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_some(),
        "expected the initial filtered board row to render before applying a name predicate"
    );

    click(cx, INPUT);
    settle(cx);
    cx.simulate_input("012");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected the predicate input to stay inside the table sample"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.predicate_filter_changes().to_vec()
    });
    assert!(
        changes.len() >= "012".len(),
        "typing a board-item predicate should record controlled changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one predicate-filter change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.column_id, "name");
    assert_eq!(last.operator.as_deref(), Some("text:contains"));
    assert_eq!(last.value, "012");
    assert!(!last.cleared);
    assert_eq!(last.filtered_rows, expected_filtered_rows);
    assert_eq!(last.final_rows, expected_final_rows);

    let persisted = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.predicate_filter_override(SAMPLE_ID).and_then(|state| {
            state
                .filters()
                .iter()
                .find(|filter| filter.column() == &TableColumnId::new("name"))
                .and_then(|filter| {
                    filter
                        .text_predicate()
                        .map(|(operator, query, _)| (operator, query.to_owned()))
                })
        })
    });
    assert_eq!(
        persisted,
        Some((TableTextFilterOperator::Contains, "012".to_owned()))
    );
    assert!(
        cx.debug_bounds(INITIAL_ROW).is_none(),
        "expected the initial board row to leave the rendered window after applying name predicate"
    );
    assert!(
        cx.debug_bounds(FILTERED_ROW).is_some(),
        "expected the matching board row to render after applying name predicate"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_range_filter_updates_table_rows(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE_ID: &str = "filter-board";
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TRIGGER: &str = "popover:component-table-range-filter:filter-board:score:trigger";
    const CONTENT: &str =
        "table-range-filter:component-table-range-filter:filter-board:score:content";
    const MIN_INPUT: &str = "text-input:component-table-range-filter:filter-board:score-min:root";

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let baseline = sample.state.resolve();
    let baseline_rows = baseline.filtered_model().rows().len();
    let expected_state =
        TableRangeFilterChange::new("score", "170", "").apply_to(sample.state.clone());
    let expected = expected_state.resolve();
    let expected_filtered_rows = expected.filtered_model().rows().len();
    let expected_final_rows = expected.final_model().rows().len();
    let expected_page_row_ids = expected
        .final_model()
        .rows()
        .iter()
        .map(|row| row.id().clone())
        .collect::<Vec<_>>();
    let removed_row_id = baseline
        .final_model()
        .rows()
        .iter()
        .find(|row| !expected_page_row_ids.contains(row.id()))
        .unwrap_or_else(|| panic!("expected score range to remove at least one initial page row"))
        .id()
        .as_str()
        .to_owned();
    let removed_row_selector = format!("table:component-table:filter-board:row:{removed_row_id}");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, TRIGGER);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(&removed_row_selector).is_some(),
        "expected the initial filter-board row to render before applying a score range"
    );

    click(cx, TRIGGER);
    settle(cx);
    let popup_content = bounds(cx, CONTENT);
    cx.simulate_event(ScrollWheelEvent {
        position: popup_content.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-180.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after_popup_wheel = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after_popup_wheel.top(),
        sample_before.top(),
        "expected range-filter popup wheel input to stay inside the table sample"
    );

    click(cx, MIN_INPUT);
    settle(cx);
    cx.simulate_input("170");
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.range_filter_changes().to_vec()
    });
    assert!(
        changes.len() >= 3,
        "typing a three-digit range minimum should record controlled changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one range-filter change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.column_id, "score");
    assert_eq!(last.min_text, "170");
    assert_eq!(last.max_text, "");
    assert_eq!(last.min_value, Some(170.0));
    assert_eq!(last.max_value, None);
    assert!(!last.cleared);
    assert_eq!(last.filtered_rows, expected_filtered_rows);
    assert_eq!(last.final_rows, expected_final_rows);
    assert!(
        last.filtered_rows < baseline_rows,
        "score range should narrow the table row model"
    );

    let persisted_range =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.range_filter_override(SAMPLE_ID).and_then(|state| {
                state
                    .filters()
                    .iter()
                    .find(|filter| filter.column() == &TableColumnId::new("score"))
                    .and_then(|filter| filter.number_range_bounds())
            })
        });
    assert_eq!(persisted_range, Some((Some(170.0), None)));
    assert!(
        cx.debug_bounds(&removed_row_selector).is_none(),
        "expected lower-score filter-board row to leave the rendered window after applying score range"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_editable_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "editable-release";
    const SAMPLE: &str = "gallery:component-table-sample:editable-release";
    const NAME_INPUT: &str = "text-input:table:component-table:editable-release:cell:editable-release-row-000:name:editor:root";
    const STATUS_INPUT: &str = "text-input:table:component-table:editable-release:cell:editable-release-row-000:status:editor:root";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, NAME_INPUT);

    assert!(
        cx.debug_bounds(STATUS_INPUT).is_none(),
        "read-only status column should not mount a text input"
    );
    let sample_before = bounds(cx, SAMPLE);
    let input = bounds(cx, NAME_INPUT);
    cx.simulate_click(
        point(input.right() - px(8.0), input.center().y),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input(" Prime");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "editing a table cell should not move the sample card"
    );
    assert!(
        cx.debug_bounds(NAME_INPUT).is_some(),
        "editable input should remain mounted after app-owned state feedback"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert!(
        changes.len() >= 2,
        "gallery edit should record controlled text changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.row_id, "editable-release-row-000");
    assert_eq!(last.column_id, "name");
    assert_eq!(last.outcome, "updated");
    assert!(last.next_text.contains("Prime"));

    let edited_name = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("name")))
            .map(TableCellValue::filter_text)
    });
    assert_eq!(edited_name.as_deref(), Some("Editable release 000 Prime"));
}

#[open_gpui::test]
fn components_gallery_smoke_checkbox_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "toggle-release";
    const SAMPLE: &str = "gallery:component-table-sample:toggle-release";
    const ENABLED_CHECKBOX: &str = "checkbox:table:component-table:toggle-release:cell:toggle-release-row-000:enabled:editor:root";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, ENABLED_CHECKBOX);

    let sample_before = bounds(cx, SAMPLE);
    let checkbox = bounds(cx, ENABLED_CHECKBOX);
    cx.simulate_click(checkbox.center(), Default::default());
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "toggling a table cell should not move the sample card"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert_eq!(
        changes.len(),
        1,
        "checkbox toggle should record one controlled change"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one checkbox edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.row_id, "toggle-release-row-000");
    assert_eq!(last.column_id, "enabled");
    assert_eq!(last.outcome, "updated");
    assert_eq!(last.previous_text, "true");
    assert_eq!(last.next_text, "false");

    let edited_enabled = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("enabled")))
            .cloned()
    });
    assert_eq!(edited_enabled, Some(TableCellValue::Bool(false)));
}

#[open_gpui::test]
fn components_gallery_smoke_select_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "select-release";
    const SAMPLE: &str = "gallery:component-table-sample:select-release";
    const STATUS_SELECT: &str = "select:table:component-table:select-release:cell:select-release-row-000:status:editor:trigger";
    const STATUS_CONTENT: &str =
        "select:Edit status for row select-release-row-000:select-content-scroll:content";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, STATUS_SELECT);

    let sample_before = bounds(cx, SAMPLE);
    let trigger = bounds(cx, STATUS_SELECT);
    cx.simulate_click(trigger.center(), Default::default());
    settle(cx);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    if cx.debug_bounds(STATUS_CONTENT).is_none() {
        cx.simulate_keystrokes("space");
        settle(cx);
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    assert!(
        cx.debug_bounds(STATUS_CONTENT).is_some(),
        "select content should open from the table trigger"
    );
    let blocked = bounds(
        cx,
        "listbox:table:component-table:select-release:cell:select-release-row-000:status:editor-listbox:option:blocked",
    );
    cx.simulate_click(blocked.center(), Default::default());
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "selecting a table cell should not move the sample card"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert_eq!(
        changes.len(),
        1,
        "select choice should record one controlled change"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one select edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.row_id, "select-release-row-000");
    assert_eq!(last.column_id, "status");
    assert_eq!(last.outcome, "updated");
    assert_eq!(last.previous_text, "ready");
    assert_eq!(last.next_text, "blocked");

    let edited_status = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("status")))
            .map(TableCellValue::filter_text)
    });
    assert_eq!(edited_status.as_deref(), Some("blocked"));
}

#[open_gpui::test]
fn components_gallery_smoke_multiline_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "multiline-release";
    const SAMPLE: &str = "gallery:component-table-sample:multiline-release";
    const NOTES_INPUT: &str = "textarea:table:component-table:multiline-release:cell:multiline-release-row-000:notes:editor:root";
    const NOTES_TEXT_INPUT: &str = "text-input:table:component-table:multiline-release:cell:multiline-release-row-000:notes:editor:root";
    const STATUS_TEXTAREA: &str = "textarea:table:component-table:multiline-release:cell:multiline-release-row-000:status:editor:root";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, NOTES_INPUT);

    assert!(
        cx.debug_bounds(NOTES_TEXT_INPUT).is_none(),
        "multiline notes column should not mount a single-line text input"
    );
    assert!(
        cx.debug_bounds(STATUS_TEXTAREA).is_none(),
        "read-only status column should not mount a textarea"
    );
    let sample_before = bounds(cx, SAMPLE);
    let input = bounds(cx, NOTES_INPUT);
    cx.simulate_click(
        point(input.right() - px(8.0), input.bottom() - px(12.0)),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input("\nQA note");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "editing a multiline table cell should not move the sample card"
    );
    assert!(
        cx.debug_bounds(NOTES_INPUT).is_some(),
        "multiline textarea should remain mounted after app-owned state feedback"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert!(
        changes.len() >= 2,
        "gallery multiline edit should record controlled text changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one multiline edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.row_id, "multiline-release-row-000");
    assert_eq!(last.column_id, "notes");
    assert_eq!(last.outcome, "updated");
    assert!(last.next_text.contains("QA note"));
    assert!(
        last.next_text.contains('\n'),
        "multiline edit payload should preserve newlines"
    );

    let edited_notes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("notes")))
            .map(TableCellValue::filter_text)
    });
    assert!(
        edited_notes
            .as_deref()
            .is_some_and(|notes| notes.contains("QA note") && notes.contains('\n')),
        "app-owned table state should store the newline-preserving textarea edit; notes={edited_notes:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_content_fit_table_cell_edit_widens_name_column(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "content-fit-release";
    const SAMPLE: &str = "gallery:component-table-sample:content-fit-release";
    const NAME_INPUT: &str = "text-input:table:component-table:content-fit-release:cell:editable-release-row-000:name:editor:root";
    const NAME_HEADER: &str = "table:component-table:content-fit-release:header:name";
    const NAME_CELL: &str =
        "table:component-table:content-fit-release:cell:editable-release-row-000:name";
    const SCORE_CELL: &str =
        "table:component-table:content-fit-release:cell:editable-release-row-000:score";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, NAME_INPUT);

    let sample_before = bounds(cx, SAMPLE);
    let header_before = bounds(cx, NAME_HEADER);
    let cell_before = bounds(cx, NAME_CELL);
    let score_before = bounds(cx, SCORE_CELL);
    let input = bounds(cx, NAME_INPUT);

    assert_eq!(header_before.size.width, cell_before.size.width);
    assert_eq!(
        pages::components::table_samples(ThemeTokens::default())
            .iter()
            .find(|sample| sample.id == SAMPLE_ID)
            .expect("content-fit sample should exist")
            .render_plan()
            .columns()[0]
            .width_policy(),
        TableColumnWidthPolicy::ContentFit
    );

    cx.simulate_click(
        point(input.right() - px(8.0), input.center().y),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input(" Prime");
    settle(cx);
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let header_after = bounds(cx, NAME_HEADER);
    let cell_after = bounds(cx, NAME_CELL);
    let score_after = bounds(cx, SCORE_CELL);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "editing a content-fit table cell should not move the sample card"
    );
    assert_eq!(header_after.size.width, cell_after.size.width);
    assert!(header_after.size.width > header_before.size.width);
    assert_eq!(score_after.size.width, score_before.size.width);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.row_id, "editable-release-row-000");
    assert_eq!(last.column_id, "name");
    assert_eq!(last.outcome, "updated");
    assert!(last.next_text.contains("Prime"));
}

#[open_gpui::test]
fn components_gallery_smoke_grouped_table_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-rollup")
        .expect("release-rollup table sample should exist");
    let plan = sample.render_plan();
    let later_row_index = plan.visible_row_count() + sample.overscan + 5;
    let first_row_id = plan.table().final_model().rows()[0]
        .id()
        .as_str()
        .to_owned();
    let later_row_id = plan.table().final_model().rows()[later_row_index]
        .id()
        .as_str()
        .to_owned();
    let first_row_selector = format!("table:component-table:release-rollup:row:{first_row_id}");
    let later_row_selector = format!("table:component-table:release-rollup:row:{later_row_id}");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:table:component-table:release-rollup:body-scroll",
    );
    let sample_before = bounds(cx, "gallery:component-table-sample:release-rollup");
    let header_before = bounds(cx, "table:component-table:release-rollup:header-row");
    let scroll_target = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_id}:name"),
    );

    assert!(
        cx.debug_bounds(&first_row_selector).is_some(),
        "expected grouped Table row `{first_row_id}` to render in the initial window"
    );
    let first_row_before = bounds(cx, &first_row_selector);
    assert!(
        cx.debug_bounds(&later_row_selector).is_none(),
        "expected grouped Table row `{later_row_id}` to start outside the rendered window"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: scroll_target.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-520.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-rollup");
    let header_after = bounds(cx, "table:component-table:release-rollup:header-row");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected grouped Table viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        header_after.top(),
        header_before.top(),
        "expected grouped Table header to stay fixed while the body scrolls; before={header_before:?} after={header_after:?}"
    );
    if let Some(first_row_after) = cx.debug_bounds(&first_row_selector) {
        assert!(
            first_row_after.top() < first_row_before.top(),
            "expected grouped Table row `{first_row_id}` to move up after internal scroll; before={first_row_before:?} after={first_row_after:?}"
        );
    }
    assert!(
        cx.debug_bounds(&later_row_selector).is_some(),
        "expected grouped Table row `{later_row_id}` to enter the rendered window after internal scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-rollup")
        .expect("release-rollup table sample should exist");
    let plan = sample.render_plan();
    assert!(
        plan.uses_split_pinned_layout(),
        "release-rollup should exercise sticky pinned table lanes"
    );
    let first_rendered_row = plan
        .rows()
        .iter()
        .find(|row| row.row().is_leaf())
        .unwrap_or(&plan.rows()[0]);
    let first_row_key = first_rendered_row.render_key().to_owned();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:table:component-table:release-rollup:header-center-scroll",
    );

    let sample_before = bounds(cx, "gallery:component-table-sample:release-rollup");
    let left_before = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_key}:name"),
    );
    let center_header_before = bounds(cx, "table:component-table:release-rollup:header:team");
    let center_cell_before = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_key}:team"),
    );
    let right_before = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_key}:status"),
    );
    assert!(
        cx.debug_bounds(&format!(
            "scroll-area:table:component-table:release-rollup:row-center-scroll:{first_row_key}"
        ))
        .is_some(),
        "expected release-rollup body center lane to expose the shared horizontal viewport"
    );
    let center_viewport = bounds(
        cx,
        "scroll-area:table:component-table:release-rollup:header-center-scroll",
    );

    assert!(
        cx.debug_bounds("scroll-area:table:component-table:release-rollup:header-center-scroll")
            .is_some(),
        "expected release-rollup header center lane to expose the shared horizontal viewport"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-180.0), px(0.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-rollup");
    let left_after = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_key}:name"),
    );
    let center_header_after = bounds(cx, "table:component-table:release-rollup:header:team");
    let center_cell_after = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_key}:team"),
    );
    let right_after = bounds(
        cx,
        &format!("table:component-table:release-rollup:cell:{first_row_key}:status"),
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected sticky pinned Table horizontal wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "expected left pinned lane to keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "expected right pinned lane to keep its screen-space x position"
    );
    assert!(
        center_header_after.left() < center_header_before.left(),
        "expected shared horizontal handle to move center header left; before={center_header_before:?} after={center_header_after:?}"
    );
    assert!(
        center_cell_after.left() < center_cell_before.left(),
        "expected horizontal body center lane to move left; before={center_cell_before:?} after={center_cell_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_grouped_table_column_reorder_updates_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-table-sample:release-rollup";
    const SCORE: &str = "table:component-table:release-rollup:header:score";

    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-rollup")
        .expect("release-rollup table sample should exist");
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    let center_viewport = bounds(
        cx,
        "scroll-area:table:component-table:release-rollup:header-center-scroll",
    );
    cx.simulate_event(ScrollWheelEvent {
        position: center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-180.0), px(0.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_before = bounds(cx, SAMPLE);
    let score_before = bounds(cx, SCORE);
    let team_before = bounds(cx, "table:component-table:release-rollup:header:team");
    let change = TableColumnOrderChange::move_before("score", "team", TableColumnRegion::Center);
    cx.update(|_, app| {
        pages::components::record_table_column_order_change(
            "release-rollup",
            &sample.state,
            &change,
            app,
        );
    });
    cx.run_until_parked();
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let score_after = bounds(cx, SCORE);
    let team_after = bounds(cx, "table:component-table:release-rollup:header:team");
    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.column_order_changes().to_vec()
    });

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected release-rollup reorder update to keep the sample card anchored"
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, "release-rollup");
    assert_eq!(changes[0].column_id, "score");
    assert_eq!(changes[0].target_column_id, "team");
    assert_eq!(changes[0].placement, "before");
    assert_eq!(changes[0].region, "center");
    assert_eq!(
        changes[0].column_order,
        ["name", "score", "team", "status"]
            .iter()
            .map(|column| column.to_string())
            .collect::<Vec<_>>()
    );
    assert!(
        score_after.left() < team_after.left(),
        "expected score to render before team after the reorder; before=({score_before:?}, {team_before:?}) after=({score_after:?}, {team_after:?})"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-matrix")
        .expect("release-matrix table sample should exist");
    let plan = sample.render_plan();
    assert!(
        plan.uses_split_pinned_layout(),
        "release-matrix should exercise sticky pinned table lanes"
    );
    let first_row_key = plan.rows()[0].render_key().to_owned();
    let far_header = "table:component-table:release-matrix:header:metric_13";
    let far_cell = format!("table:component-table:release-matrix:cell:{first_row_key}:metric_13");
    let left_group = "table:component-table:release-matrix:header-group:left:1:identity";
    let metrics_group = "table:component-table:release-matrix:header-group:center:1:metrics";
    let right_group = "table:component-table:release-matrix:header-group:right:1:delivery";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:table:component-table:release-matrix:header-center-scroll",
    );

    let sample_before = bounds(cx, "gallery:component-table-sample:release-matrix");
    let left_before = bounds(
        cx,
        &format!("table:component-table:release-matrix:cell:{first_row_key}:name"),
    );
    let right_before = bounds(
        cx,
        &format!("table:component-table:release-matrix:cell:{first_row_key}:status"),
    );
    let left_group_before = bounds(cx, left_group);
    assert!(
        cx.debug_bounds(metrics_group).is_some(),
        "expected release-matrix metrics group header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(right_group).is_some(),
        "expected release-matrix delivery group header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(left_group).is_some(),
        "expected release-matrix identity group header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:component-table:release-matrix:header:metric_00")
            .is_some(),
        "expected the initial center column window to mount the first metric"
    );
    assert!(
        cx.debug_bounds(far_header).is_none(),
        "expected the far metric header to stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&far_cell).is_none(),
        "expected the far metric cell to stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&format!(
            "scroll-area:table:component-table:release-matrix:row-center-scroll:{first_row_key}"
        ))
        .is_some(),
        "expected release-matrix body center lane to expose the shared horizontal viewport"
    );
    let center_viewport = bounds(
        cx,
        "scroll-area:table:component-table:release-matrix:header-center-scroll",
    );

    for _ in 0..6 {
        if cx.debug_bounds(far_header).is_some() && cx.debug_bounds(&far_cell).is_some() {
            break;
        }

        cx.simulate_event(ScrollWheelEvent {
            position: center_viewport.center(),
            delta: ScrollDelta::Pixels(point(px(-360.0), px(0.0))),
            ..Default::default()
        });
        redraw(cx);
    }

    let sample_after = bounds(cx, "gallery:component-table-sample:release-matrix");
    let left_after = bounds(
        cx,
        &format!("table:component-table:release-matrix:cell:{first_row_key}:name"),
    );
    let left_group_after = bounds(cx, left_group);
    let right_after = bounds(
        cx,
        &format!("table:component-table:release-matrix:cell:{first_row_key}:status"),
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected matrix Table horizontal wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "expected matrix Table left pinned lane to keep its screen-space x position"
    );
    assert_eq!(
        left_group_after.left(),
        left_group_before.left(),
        "expected matrix Table left header group to keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "expected matrix Table right pinned lane to keep its screen-space x position"
    );
    assert!(
        cx.debug_bounds(far_header).is_some(),
        "expected the far metric header to enter the rendered center window after horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(metrics_group).is_some(),
        "expected the metrics group header to stay mounted while the center window scrolls"
    );
    assert!(
        cx.debug_bounds(right_group).is_some(),
        "expected the delivery group header to stay mounted while the center window scrolls"
    );
    assert!(
        cx.debug_bounds(&far_cell).is_some(),
        "expected the far metric cell to enter the rendered center window after horizontal scrolling"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_column_visibility_updates_release_matrix(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "release-matrix";
    const SAMPLE: &str = "gallery:component-table-sample:release-matrix";
    const TOOLBAR: &str = "table-toolbar:component-table-toolbar:release-matrix:root";
    const TRIGGER: &str = "popover:component-table-column-visibility:release-matrix:trigger";
    const CONTENT: &str =
        "table-column-visibility:component-table-column-visibility:release-matrix:content";
    const METRIC_ROW: &str =
        "table-column-visibility:component-table-column-visibility:release-matrix:column:metric_03";
    const SHOW_ALL: &str =
        "table-column-visibility:component-table-column-visibility:release-matrix:show-all";
    const METRIC_HEADER: &str = "table:component-table:release-matrix:header:metric_03";

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let plan = sample.render_plan();
    assert_eq!(plan.aria_column_count(), 16);
    let first_row_key = plan.rows()[0].render_key().to_owned();
    let metric_cell =
        format!("table:component-table:release-matrix:cell:{first_row_key}:metric_03");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, TRIGGER);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(TOOLBAR).is_some(),
        "expected release-matrix controls to render inside the table toolbar recipe"
    );
    assert!(
        cx.debug_bounds(METRIC_HEADER).is_some(),
        "expected metric_03 header to render before hiding the column"
    );
    assert!(
        cx.debug_bounds(&metric_cell).is_some(),
        "expected metric_03 cell to render before hiding the column"
    );

    click(cx, TRIGGER);
    settle(cx);
    assert!(
        cx.debug_bounds(CONTENT).is_some(),
        "expected the column visibility popover content to open"
    );
    click(cx, METRIC_ROW);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_changes().to_vec()
    });
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, SAMPLE_ID);
    assert_eq!(changes[0].action, "toggle_column");
    assert_eq!(changes[0].column_ids, vec!["metric_03".to_owned()]);
    assert_eq!(changes[0].next_visible, Some(false));
    assert_eq!(changes[0].visible_columns, 15);
    assert_eq!(changes[0].hidden_columns, 1);
    let metric_hidden = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_override(SAMPLE_ID)
            .and_then(|visibility| visibility.override_for(&TableColumnId::new("metric_03")))
    });
    assert_eq!(metric_hidden, Some(false));
    assert!(
        cx.debug_bounds(METRIC_HEADER).is_none(),
        "expected metric_03 header to unmount after hiding the column"
    );
    assert!(
        cx.debug_bounds(&metric_cell).is_none(),
        "expected metric_03 cell to unmount after hiding the column"
    );

    let popup_content = bounds(cx, CONTENT);
    cx.simulate_event(ScrollWheelEvent {
        position: popup_content.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-80.0))),
        ..Default::default()
    });
    redraw(cx);
    let sample_after_popup_wheel = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after_popup_wheel.top(),
        sample_before.top(),
        "expected column-visibility popup wheel input to stay inside the table sample"
    );

    if cx.debug_bounds(SHOW_ALL).is_none() {
        click(cx, TRIGGER);
        settle(cx);
    }
    click(cx, SHOW_ALL);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_changes().to_vec()
    });
    assert_eq!(changes.len(), 2);
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected show-all visibility change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.action, "show_all");
    assert!(last.column_ids.contains(&"metric_03".to_owned()));
    assert_eq!(last.next_visible, Some(true));
    assert_eq!(last.visible_columns, 16);
    assert_eq!(last.hidden_columns, 0);
    assert!(
        cx.debug_bounds(METRIC_HEADER).is_some(),
        "expected metric_03 header to return after show-all"
    );
    assert!(
        cx.debug_bounds(&metric_cell).is_some(),
        "expected metric_03 cell to return after show-all"
    );

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected column-visibility interactions to keep the sample card anchored"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "row-pinning")
        .expect("row-pinning table sample should exist");
    let plan = sample.render_plan();
    assert_eq!(plan.top_rows().len(), 1);
    assert_eq!(plan.center_rows().len(), 11);
    assert_eq!(plan.bottom_rows().len(), 2);
    assert!(
        plan.uses_split_pinned_layout(),
        "row-pinning should combine row-pinned bands with pinned column lanes"
    );

    let top_row_key = plan.top_rows()[0].render_key().to_owned();
    let bottom_row_key = plan.bottom_rows()[1].render_key().to_owned();
    let center_cell_selectors = plan
        .center_rows()
        .iter()
        .map(|row| {
            format!(
                "table:component-table:row-pinning:cell:{}:name",
                row.render_key()
            )
        })
        .collect::<Vec<_>>();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, "gallery:component-table-sample:row-pinning");

    assert!(
        cx.debug_bounds("table:component-table:row-pinning:body:top")
            .is_some(),
        "expected row-pinning top band to render"
    );
    assert!(
        cx.debug_bounds("table:component-table:row-pinning:body:center")
            .is_some(),
        "expected row-pinning center band to render"
    );
    assert!(
        cx.debug_bounds("table:component-table:row-pinning:body:bottom")
            .is_some(),
        "expected row-pinning bottom band to render"
    );
    let collect_center_cells = |cx: &mut VisualTestContext| {
        center_cell_selectors
            .iter()
            .enumerate()
            .filter_map(|(index, selector)| {
                cx.debug_bounds(selector)
                    .map(|bounds| (index, selector.clone(), bounds))
            })
            .collect::<Vec<_>>()
    };

    let center_rows_before = collect_center_cells(cx);
    assert!(
        !center_rows_before.is_empty(),
        "expected row-pinning center body to render at least one center row cell"
    );
    let interaction_target = scroll_page_selector_into_view(&shell, cx, &center_rows_before[0].1);

    let sample_before = bounds(cx, "gallery:component-table-sample:row-pinning");
    let top_row_before = bounds(
        cx,
        &format!("table:component-table:row-pinning:row:{top_row_key}"),
    );
    let bottom_row_before = bounds(
        cx,
        &format!("table:component-table:row-pinning:row:{bottom_row_key}"),
    );
    let top_name_before = bounds(
        cx,
        &format!("table:component-table:row-pinning:cell:{top_row_key}:name"),
    );
    let center_rows_before = collect_center_cells(cx);
    cx.simulate_event(ScrollWheelEvent {
        position: interaction_target.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:row-pinning");
    let top_row_after = bounds(
        cx,
        &format!("table:component-table:row-pinning:row:{top_row_key}"),
    );
    let bottom_row_after = bounds(
        cx,
        &format!("table:component-table:row-pinning:row:{bottom_row_key}"),
    );
    let center_rows_after = collect_center_cells(cx);
    assert!(
        !center_rows_after.is_empty(),
        "expected row-pinning center body to keep rendering center row cells after scrolling"
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected row-pinning Table wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "top pinned row band should stay fixed while center rows scroll"
    );
    assert_eq!(
        bottom_row_after.top(),
        bottom_row_before.top(),
        "bottom pinned row band should stay fixed while center rows scroll"
    );
    assert_eq!(
        bounds(
            cx,
            &format!("table:component-table:row-pinning:cell:{top_row_key}:name"),
        )
        .left(),
        top_name_before.left(),
        "left-pinned cells inside pinned rows should stay fixed while center rows scroll"
    );
    let center_window_changed = center_rows_before
        .iter()
        .map(|(index, _, _)| *index)
        .collect::<Vec<_>>()
        != center_rows_after
            .iter()
            .map(|(index, _, _)| *index)
            .collect::<Vec<_>>();
    let center_row_moved = center_rows_before.iter().any(|(_, selector, before)| {
        center_rows_after.iter().any(|(_, after_selector, after)| {
            after_selector == selector && after.top() != before.top()
        })
    });
    assert!(
        center_window_changed || center_row_moved,
        "center rows should move inside the center scroll body; before={center_rows_before:?} after={center_rows_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_table_expands_and_activates(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE: &str = "gallery:component-table-sample:dependency-tree";
    const ROOT_TOGGLE: &str =
        "table:component-table:dependency-tree:tree-toggle:dependency-workspace";
    const UI_TOGGLE: &str = "table:component-table:dependency-tree:tree-toggle:dependency-ui";
    const CHILD_ROW: &str = "table:component-table:dependency-tree:row:dependency-ui-table";
    const CHILD_CELL: &str = "table:component-table:dependency-tree:cell:dependency-ui-table:name";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, UI_TOGGLE);

    assert!(
        cx.debug_bounds(CHILD_ROW).is_none(),
        "expected dependency-ui children to start collapsed"
    );
    let root_toggle = bounds(cx, ROOT_TOGGLE);
    let ui_toggle = bounds(cx, UI_TOGGLE);
    assert!(
        ui_toggle.left() > root_toggle.left(),
        "expected nested tree table toggle to be indented; root={root_toggle:?} ui={ui_toggle:?}"
    );

    click(cx, UI_TOGGLE);
    settle(cx);
    let toggles = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_toggles()
            .iter()
            .map(|toggle| {
                (
                    toggle.sample_id.clone(),
                    toggle.row_id.clone(),
                    toggle.expanded,
                    toggle.depth,
                )
            })
            .collect::<Vec<_>>()
    });
    assert!(
        cx.debug_bounds(CHILD_ROW).is_some(),
        "expected dependency-ui child row to render after expansion; toggles={toggles:?}"
    );
    assert_eq!(
        toggles,
        vec![(
            "dependency-tree".to_owned(),
            "dependency-ui".to_owned(),
            true,
            1
        )]
    );
    let activations_after_toggle =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.row_activations().to_vec()
        });
    assert!(
        activations_after_toggle.is_empty(),
        "expected tree disclosure clicks to avoid row activation"
    );

    click(cx, CHILD_ROW);
    assert!(
        cx.debug_selector_is_focused(CHILD_ROW),
        "expected clicking a tree table row to focus it for keyboard activation; focused={:?} child={:?}",
        cx.focused_debug_selector(),
        bounds(cx, CHILD_CELL)
    );
    let click_activations =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.row_activations().to_vec()
        });
    assert_eq!(click_activations.len(), 1);
    assert_eq!(click_activations[0].sample_id, "dependency-tree");
    assert_eq!(click_activations[0].row_id, "dependency-ui-table");
    assert_eq!(click_activations[0].kind, "click");
    assert_eq!(click_activations[0].depth, 2);
    assert!(!click_activations[0].tree_branch);
    assert_eq!(click_activations[0].tree_expanded, None);

    cx.simulate_keystrokes("enter");
    redraw(cx);
    let activations = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.row_activations().to_vec()
    });
    assert_eq!(activations.len(), 2);
    assert_eq!(activations[1].sample_id, "dependency-tree");
    assert_eq!(activations[1].row_id, "dependency-ui-table");
    assert_eq!(activations[1].kind, "keyboard");
    assert_eq!(activations[1].depth, 2);
}

#[open_gpui::test]
fn components_gallery_smoke_table_server_tree_loads_children_from_expansion_request(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-table-sample:server-tree";
    const WORKSPACE_TOGGLE: &str = "table:component-table:server-tree:tree-toggle:server-workspace";
    const CACHE_TOGGLE: &str = "table:component-table:server-tree:tree-toggle:server-cache";
    const FAILED_TOGGLE: &str = "table:component-table:server-tree:tree-toggle:server-failed";
    const CHILD_ROW: &str = "table:component-table:server-tree:row:server-api";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, WORKSPACE_TOGGLE);

    assert!(
        cx.debug_bounds(CHILD_ROW).is_none(),
        "expected server children to start app-unloaded"
    );
    assert!(
        cx.debug_bounds(CACHE_TOGGLE).is_some(),
        "expected loading server branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(FAILED_TOGGLE).is_some(),
        "expected failed server branch to render a disclosure affordance"
    );

    click(cx, WORKSPACE_TOGGLE);
    settle(cx);
    let toggles = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_toggles().to_vec()
    });
    assert!(
        cx.debug_bounds(CHILD_ROW).is_some(),
        "expected server child row to render after the app supplies loaded children; toggles={toggles:?}"
    );
    assert_eq!(toggles.len(), 1);
    assert_eq!(toggles[0].sample_id, "server-tree");
    assert_eq!(toggles[0].row_id, "server-workspace");
    assert!(toggles[0].expanded);
    assert_eq!(toggles[0].depth, 0);
    assert_eq!(toggles[0].loaded_child_count, 0);
    assert_eq!(toggles[0].children_load_state, "idle");
    assert_eq!(toggles[0].children_load_message, None);
    let activations_after_toggle =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.row_activations().to_vec()
        });
    assert!(
        activations_after_toggle.is_empty(),
        "expected manual expansion disclosure clicks to avoid row activation"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_expands_and_selects(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE: &str = "gallery:component-tree-sample:document-outline";
    const PAPER: &str = "tree:component-tree:document-outline:item:paper";
    const INTRO: &str = "tree:component-tree:document-outline:item:intro";
    const NOTES: &str = "tree:component-tree:document-outline:item:notes";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_section(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, PAPER);
    assert!(
        cx.debug_bounds(INTRO).is_none(),
        "expected collapsed Tree descendants to stay hidden before expansion"
    );

    click(cx, PAPER);
    assert!(
        cx.debug_selector_is_focused(PAPER),
        "expected clicking a Tree row to focus that row for keyboard handling; focused={:?} paper={:?} viewport={:?}",
        cx.focused_debug_selector(),
        bounds(cx, PAPER),
        bounds(
            cx,
            "scroll-area:tree:component-tree:document-outline:scroll"
        )
    );
    cx.update_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    cx.simulate_keystrokes("right");
    redraw(cx);
    assert!(
        cx.debug_bounds(INTRO).is_some(),
        "expected the Paper branch to reveal its child after toggling open"
    );
    let toggles = cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.toggles()
            .iter()
            .map(|toggle| {
                (
                    toggle.sample_id.clone(),
                    toggle.value.clone(),
                    toggle.expanded,
                )
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(
        toggles,
        vec![("document-outline".to_owned(), "paper".to_owned(), true)],
        "expected right arrow to expand the focused root branch"
    );

    cx.simulate_keystrokes("down");
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(INTRO),
        "expected Down to move focus to the newly revealed child row"
    );

    cx.simulate_keystrokes("enter");
    redraw(cx);
    let selections = cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.selections()
            .iter()
            .map(|selection| (selection.sample_id.clone(), selection.value.clone()))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        selections,
        vec![("document-outline".to_owned(), "intro".to_owned())],
        "expected Enter to select the focused child row"
    );

    cx.simulate_keystrokes("n o");
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(NOTES),
        "expected Tree typeahead to focus the visible Notes row; focused={:?}",
        cx.focused_debug_selector()
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_drag_updates_sample(cx: &mut open_gpui::TestAppContext) {
    const CHILD: &str = "tree:component-tree:editable-outline:item:child";
    const PEER: &str = "tree:component-tree:editable-outline:item:peer";
    const SIBLING: &str = "tree:component-tree:editable-outline:item:sibling";
    const DROP: &str = "tree:component-tree:editable-outline:drop:before:sibling";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_catalog_entry(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, CHILD);
    let child_before = bounds(cx, CHILD).center();
    let peer_before = bounds(cx, PEER).center();
    let sibling_before = bounds(cx, SIBLING).center();
    let drop_before = bounds(cx, DROP).center();
    assert!(
        child_before.y < peer_before.y,
        "expected child row to render above peer before drag"
    );
    assert!(peer_before.y < sibling_before.y);

    cx.simulate_click(child_before, Default::default());
    redraw(cx);
    let selections = cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.selections().to_vec()
    });
    assert_eq!(
        selections.len(),
        1,
        "expected the editable Tree row to accept a normal click before dragging"
    );
    assert_eq!(selections[0].sample_id, "editable-outline");
    assert_eq!(selections[0].value, "child");
    cx.update_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    cx.simulate_mouse_down(child_before, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(child_before.x + px(18.0), child_before.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(child_before.x + px(42.0), child_before.y + px(2.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(drop_before, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(drop_before, MouseButton::Left, Default::default());
    cx.run_until_parked();
    redraw(cx);

    let moves =
        cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| log.moves().to_vec());
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].sample_id, "editable-outline");
    assert_eq!(moves[0].tree_move.value(), "child");
    assert_eq!(moves[0].tree_move.source_parent_value(), Some("root"));
    assert_eq!(
        moves[0].tree_move.position(),
        open_gpui_ui_components::TreeDropPosition::Before
    );
    assert_eq!(moves[0].tree_move.target().target_value(), "sibling");
    assert_eq!(moves[0].tree_move.target_parent_value(), None);
    assert_eq!(moves[0].tree_move.sibling_anchor_value(), Some("sibling"));

    redraw(cx);
    let child_after = bounds(cx, CHILD).center();
    let peer_after = bounds(cx, PEER).center();
    let sibling_after = bounds(cx, SIBLING).center();
    assert!(
        child_after.y > peer_after.y,
        "expected child row to move below peer after a before-drop move"
    );
    assert!(peer_after.y < sibling_after.y);
}

#[open_gpui::test]
fn components_gallery_smoke_tree_lazy_branches_emit_load_metadata(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-tree-sample:remote-workspace";
    const UNLOADED_TOGGLE: &str = "tree:component-tree:remote-workspace:toggle:remote-src";
    const LOADING_TOGGLE: &str = "tree:component-tree:remote-workspace:toggle:remote-crates";
    const FAILED_TOGGLE: &str = "tree:component-tree:remote-workspace:toggle:remote-build";
    const UNLOADED_ITEM: &str = "tree:component-tree:remote-workspace:item:remote-src";
    const LOADING_ITEM: &str = "tree:component-tree:remote-workspace:item:remote-crates";
    const FAILED_ITEM: &str = "tree:component-tree:remote-workspace:item:remote-build";
    const LOADING_HINT: &str = "tree:component-tree:remote-workspace:load-state:remote-crates";
    const FAILED_HINT: &str = "tree:component-tree:remote-workspace:load-state:remote-build";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_catalog_entry(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, UNLOADED_ITEM);
    assert!(
        cx.debug_bounds(UNLOADED_TOGGLE).is_some(),
        "expected unloaded remote branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(LOADING_TOGGLE).is_some(),
        "expected loading remote branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(FAILED_TOGGLE).is_some(),
        "expected failed remote branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(LOADING_HINT).is_some(),
        "expected loading branch to expose a visible load-state hint"
    );
    assert!(
        cx.debug_bounds(FAILED_HINT).is_some(),
        "expected failed branch to expose a visible load-state hint"
    );
    cx.update_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    click(cx, UNLOADED_ITEM);
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(UNLOADED_ITEM),
        "expected unloaded branch row to receive focus before Right; focused={:?}",
        cx.focused_debug_selector()
    );
    cx.simulate_keystrokes("right");
    redraw(cx);
    click(cx, LOADING_ITEM);
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(LOADING_ITEM),
        "expected loading branch row to receive focus before Right; focused={:?}",
        cx.focused_debug_selector()
    );
    cx.simulate_keystrokes("right");
    redraw(cx);
    click(cx, FAILED_ITEM);
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(FAILED_ITEM),
        "expected failed branch row to receive focus before Right; focused={:?}",
        cx.focused_debug_selector()
    );
    cx.simulate_keystrokes("right");
    redraw(cx);

    let toggles = cx
        .read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| log.toggles().to_vec());
    assert_eq!(
        toggles.len(),
        2,
        "expected unloaded and failed branches to toggle while loading branch is blocked; toggles={toggles:?}"
    );
    assert_eq!(toggles[0].sample_id, "remote-workspace");
    assert_eq!(toggles[0].value, "remote-src");
    assert!(toggles[0].expanded);
    assert_eq!(toggles[0].loaded_child_count, 0);
    assert_eq!(toggles[0].children_load_state, "unloaded");
    assert_eq!(toggles[0].children_load_message, None);
    assert_eq!(toggles[1].sample_id, "remote-workspace");
    assert_eq!(toggles[1].value, "remote-build");
    assert!(toggles[1].expanded);
    assert_eq!(toggles[1].loaded_child_count, 0);
    assert_eq!(toggles[1].children_load_state, "failed");
    assert_eq!(
        toggles[1].children_load_message.as_deref(),
        Some("Network unavailable")
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_card_wheel_does_not_leak_to_page(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-tree-sample:document-outline";
    const VIEWPORT: &str = "scroll-area:tree:component-tree:document-outline:scroll";
    const ITEM: &str = "tree:component-tree:document-outline:item:appendix-01";

    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, SAMPLE);
    let sample_before = bounds(cx, SAMPLE);
    let item_before = bounds(cx, ITEM);
    let viewport = bounds(cx, VIEWPORT);

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let item_after = bounds(cx, ITEM);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Tree card wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        item_after.top() < item_before.top(),
        "expected Tree card wheel input to move the inner viewport; before={item_before:?} after={item_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_tree_scrolls_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-tree-sample:release-outline";
    const ROOT: &str = "tree:component-tree:release-outline:item:release-node-0000";
    const LAST: &str = "tree:component-tree:release-outline:item:release-node-0239";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_catalog_entry(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    let sample_before = bounds(cx, SAMPLE);
    assert!(cx.debug_bounds(ROOT).is_some());
    assert!(cx.debug_bounds(LAST).is_none());

    click(cx, ROOT);
    redraw(cx);
    cx.simulate_keystrokes("end");
    redraw(cx);
    let sample_after = bounds(cx, SAMPLE);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected virtualized Tree keyboard navigation to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds(LAST).is_none(),
        "expected the far Tree row to remain outside the initial render window after End"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_until_visible(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let sample_before = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000"
        )
        .is_some(),
        "expected the initial VirtualizedList window to render the first row"
    );
    let row_0 = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );
    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0016"
        )
        .is_none(),
        "expected row 0016 to start outside the initial rendered window"
    );
    let row_0_before = row_0;
    cx.simulate_event(ScrollWheelEvent {
        position: point(
            sample_before.left() + px(24.0),
            sample_before.top() + px(24.0),
        ),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-56.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_chrome_after = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let row_0_after = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );
    assert_eq!(
        sample_chrome_after.top(),
        sample_before.top(),
        "expected VirtualizedList card chrome wheel input to stay inside the sample card; before={sample_before:?} after={sample_chrome_after:?}"
    );
    assert_eq!(
        row_0_after.top(),
        row_0_before.top(),
        "expected VirtualizedList card chrome wheel input to leave the rendered window unchanged; before={row_0_before:?} after={row_0_after:?}"
    );

    click(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:root",
    );
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(
            "virtualized-list:component-virtualized-list:release-navigation:root"
        ),
        "expected the VirtualizedList root to own focus after clicking a row"
    );
    cx.simulate_keystrokes("pagedown");
    redraw(cx);
    cx.simulate_keystrokes("pagedown");
    redraw(cx);

    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0016"
        )
        .is_some(),
        "expected repeated PageDown to reveal row 0016 inside the sample"
    );

    let viewport = bounds(
        cx,
        "scroll-area:virtualized-list:component-virtualized-list:release-navigation:viewport",
    );
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected VirtualizedList viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0025"
        )
        .is_some(),
        "expected virtualized list row 0025 to enter the rendered window after keyboard and wheel scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_until_visible(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let sample_before = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let row_before = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );

    cx.simulate_event(ScrollWheelEvent {
        position: point(
            sample_before.left() + px(24.0),
            sample_before.top() + px(24.0),
        ),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let row_after = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected VirtualizedList card chrome wheel input to stay local to the sample; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        row_after, row_before,
        "expected VirtualizedList card chrome wheel input to leave the inner viewport unchanged; before={row_before:?} after={row_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-virtualized-list-sample:release-navigation";
    const ROOT: &str = "virtualized-list:component-virtualized-list:release-navigation:root";
    const ROW_8: &str =
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0008";

    let cx = open_components_gallery(cx);
    cx.set_global(pages::components::VirtualizedListSampleRuntimeLog::default());

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_until_visible(cx, SAMPLE);
    click(cx, ROOT);
    assert!(
        cx.debug_selector_is_focused(ROOT),
        "expected clicking a VirtualizedList row to focus the list root for keyboard handling"
    );
    cx.update_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    let row_8_before = bounds(cx, ROW_8);
    cx.simulate_keystrokes("pagedown");
    redraw(cx);

    let row_8_after = bounds(cx, ROW_8);
    assert!(
        row_8_after.top() < row_8_before.top(),
        "expected PageDown to reveal the next active VirtualizedList row; before={row_8_before:?} after={row_8_after:?}"
    );

    cx.simulate_keystrokes("enter");
    redraw(cx);
    let enter_activations = cx
        .read_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
            log.activations()
                .iter()
                .map(|activation| (activation.sample_id.clone(), activation.index))
                .collect::<Vec<_>>()
        });
    assert_eq!(enter_activations.len(), 1);
    assert_eq!(enter_activations[0].0, "release-navigation");
    let activated_index = enter_activations[0].1;
    assert!(
        activated_index >= 8,
        "expected Enter to activate the row revealed by PageDown; activations={enter_activations:?}"
    );

    cx.simulate_keystrokes("space");
    redraw(cx);
    let activations =
        cx.read_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
            log.activations()
                .iter()
                .map(|activation| (activation.sample_id.clone(), activation.index))
                .collect::<Vec<_>>()
        });
    assert_eq!(
        activations,
        vec![
            ("release-navigation".to_owned(), activated_index),
            ("release-navigation".to_owned(), activated_index),
        ],
        "expected Space to activate the same active row after Enter"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_directory_jump_scrolls_to_tabs_section(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    let directory_before = bounds(cx, "gallery:components-directory");
    let directory_viewport = bounds(cx, "scroll-area:gallery-components-directory-scroll");
    scroll_until_visible(
        cx,
        "scroll-area:gallery-components-directory-scroll",
        "gallery:component-page-jump:tabs",
        32,
        point(px(0.0), px(-48.0)),
        directory_viewport.center(),
        |container, target| container.contains(&target.center()),
        "expected the Components directory jump to become visible after scrolling the directory"
            .to_string(),
    );

    let before = bounds(cx, "gallery:components-section:tabs");
    click(cx, "gallery:component-page-jump:tabs");
    settle(cx);
    settle(cx);

    let after = bounds(cx, "gallery:components-section:tabs");
    let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    let directory_after_click = bounds(cx, "gallery:components-directory");

    assert!(
        (after.top() - viewport.top()).abs() <= px(1.0),
        "expected the Components page directory jump to align the Tabs section with the viewport top; before={before:?} after={after:?} viewport={viewport:?}"
    );
    assert!(
        after.bottom() > viewport.top(),
        "expected the Tabs section to remain visible after clicking the directory jump; viewport={viewport:?} after={after:?}"
    );
    assert_eq!(
        directory_after_click, directory_before,
        "expected the Components directory to stay fixed while clicking a page jump scrolls the content"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_navigation_rail_scrolls_inside_shell(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    let before = bounds(cx, "gallery:navigation-item:components");
    let navigation_viewport = bounds(cx, "gallery:navigation-scroll");

    cx.simulate_event(ScrollWheelEvent {
        position: navigation_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    redraw(cx);

    let after = bounds(cx, "gallery:navigation-item:components");
    assert!(
        after.top() < before.top(),
        "expected gallery navigation rail to scroll independently inside the shell; before={before:?} after={after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_vertical_tabs_scroll_inside_sample(cx: &mut open_gpui::TestAppContext) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:tabs");
    let before = scroll_page_selector_into_view(
        &shell,
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    let sample_before = bounds(cx, "gallery:component-tabs-sample:workspace-tabs");
    let tablist = bounds(cx, "tabs:component-tabs:workspace-tabs:tablist");
    let tablist_viewport = bounds(
        cx,
        "scroll-area:tabs:component-tabs:workspace-tabs:tablist-scroll",
    );
    assert!(
        tablist.contains(&tablist_viewport.center()),
        "expected vertical Tabs ScrollArea viewport to stay inside the tablist shell; tablist={tablist:?} viewport={tablist_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: tablist_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-tabs-sample:workspace-tabs");
    let after = bounds(
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected vertical Tabs rail wheel input to stay inside the sample instead of moving the Components page; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        after.top() < before.top(),
        "expected constrained vertical Tabs sample to scroll its rail inside the card; before={before:?} after={after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_sidebar_long_navigation_scrolls_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "gallery:component-sidebar-sample:long-sidebar");
    let sample_before = bounds(cx, "gallery:component-sidebar-sample:long-sidebar");
    let segments_before = bounds(cx, "sidebar:component-sidebar:long-sidebar:item:segments");
    let sidebar_viewport = bounds(cx, "scroll-area:component-sidebar:long-sidebar-scroll");

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-96.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-sidebar-sample:long-sidebar");
    let segments_after = bounds(cx, "sidebar:component-sidebar:long-sidebar:item:segments");
    let segments_offset_before = segments_before.top() - sample_before.top();
    let segments_offset_after = segments_after.top() - sample_after.top();
    assert!(
        segments_offset_after < segments_offset_before,
        "expected long Sidebar sample to scroll its internal navigation viewport; sample before/after=({sample_before:?}, {sample_after:?}) segments before/after=({segments_before:?}, {segments_after:?})"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:splitter");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "splitter:component-splitter:details-split:handle:0",
    );
    let collapsed_before = bounds(cx, "splitter-panel:summary");
    let top_before = bounds(cx, "splitter-panel:summary");
    let bottom_before = bounds(cx, "splitter-panel:details");
    let handle = bounds(cx, "splitter:component-splitter:details-split:handle:0").center();

    drag(cx, handle, point(handle.x, handle.y + px(68.0)));

    let top_after = bounds(cx, "splitter-panel:summary");
    let bottom_after = bounds(cx, "splitter-panel:details");
    assert!(
        top_before.size.height < bottom_before.size.height,
        "expected the collapsed summary panel to start smaller than the details panel; before=({top_before:?}, {bottom_before:?})"
    );
    assert!(
        top_after.size.height > top_before.size.height
            && bottom_after.size.height < bottom_before.size.height,
        "expected full-page vertical Splitter sample to resize via pointer drag; before=({top_before:?}, {bottom_before:?}) after=({top_after:?}, {bottom_after:?})"
    );

    let restored_handle = scroll_page_selector_into_view(
        &shell,
        cx,
        "splitter:component-splitter:details-split:handle:0",
    )
    .center();
    drag(
        cx,
        restored_handle,
        point(restored_handle.x, restored_handle.y - px(60.0)),
    );

    let top_restored = bounds(cx, "splitter-panel:summary");
    let bottom_restored = bounds(cx, "splitter-panel:details");
    assert!(
        top_restored.size.height < top_after.size.height
            && bottom_restored.size.height > bottom_after.size.height,
        "expected collapsed Splitter panel to restore and keep responding to subsequent drag; collapsed={collapsed_before:?} after-collapse=({top_after:?}, {bottom_after:?}) restored=({top_restored:?}, {bottom_restored:?})"
    );

    jump_components_directory_to(cx, "gallery:component-page-jump:tabs");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    let tablist = bounds(cx, "tabs:component-tabs:workspace-tabs:tablist");
    let tab_before = bounds(
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    cx.simulate_event(ScrollWheelEvent {
        position: tablist.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    redraw(cx);
    let tab_after = bounds(
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    assert!(
        tab_after.top() < tab_before.top(),
        "expected full-page vertical Tabs sample to scroll its tab rail; before={tab_before:?} after={tab_after:?}"
    );
}
