use open_gpui::{
    Bounds, Entity, MouseButton, Pixels, ScrollDelta, ScrollWheelEvent, VisualTestContext, point,
    px, size,
};
use open_gpui_ui_components::{
    AlertDialogIntent, AlertDialogOpenMode, BadgeVariant, ButtonVariant, ComboboxOpenMode,
    CommandIndexSnapshotMode, CommandOpenMode, CommandSelectionMode, DialogOpenMode,
    FeedbackIntent, HoverCardOpenIntent, HoverCardOpenMode, MenuItemKind, MenuOpenMode,
    OverlayResolvedState, PopoverOpenMode, ScrollAreaAxis, ScrollResetPolicy, SelectOpenMode,
    SheetCloseAffordance, SheetModalMode, SheetOpenMode, SheetSide, ThemeMode, ToggleVariant,
    TooltipOpenIntent, TreeKeyboardAction, VirtualizedListScrollStrategy,
    gpui_adapter::{DEFAULT_OVERLAY_SAFE_MARGIN, default_deferred_priority, gpui_overlay_state},
};
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceShellMode, EscapeKeyPolicy, FocusRestoreIntent,
    InitialFocusIntent, Orientation, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, PanelAdaptiveClass, Role, Size, ThemeTokens,
    Toggled, semantic, ui_px,
};
use open_gpui_ui_foundation_gallery::{
    DEFAULT_GALLERY_WIDTH, GALLERY_SECTIONS, GalleryPage, GalleryShell, GalleryShellSnapshot,
    foundation_snapshot, pages,
};

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
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

    assert_eq!(samples.len(), 4);
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
}

#[test]
fn overlay_page_context_menu_samples_expose_point_anchor_contracts() {
    let samples = pages::overlay::context_menu_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
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
    let fields = pages::components::field_samples(tokens);
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

    assert_eq!(gates.len(), 10);
    assert_eq!(gates[0].id, "public-api-exports");
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/lib.rs")
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
    assert_eq!(gates[6].id, "tree-renderer");
    assert_eq!(gates[7].id, "virtualized-list-renderer");
    assert_eq!(gates[8].id, "state-contract-readouts");
    assert_eq!(gates[9].id, "a11y-labels");

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

    assert_eq!(trees.len(), 1);
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

    assert_eq!(text_inputs.len(), 5);
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

    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].id, "release-queue");
    assert_eq!(tables[0].state.rows().len(), 10_000);
    assert_eq!(
        tables[0].state.sorting()[0].direction().as_str(),
        "descending"
    );
    let release_plan = tables[0].render_plan();
    assert_eq!(release_plan.role(), Role::Table);
    assert_eq!(release_plan.column_header_role(), Role::ColumnHeader);
    assert_eq!(release_plan.cell_role(), Role::Cell);
    assert_eq!(release_plan.aria_row_count(), 10_001);
    assert_eq!(release_plan.aria_column_count(), 4);
    assert!(release_plan.rendered_row_count() <= release_plan.visible_row_count() + 5);

    let filter_plan = tables[1].render_plan();
    assert_eq!(tables[1].id, "filter-board");
    assert_eq!(filter_plan.table().filtered_model().rows().len(), 60);
    assert_eq!(filter_plan.table().final_model().rows().len(), 24);
    assert_eq!(filter_plan.table().final_model().selected_count(), 1);

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

    assert!(select_section.contains("if let Some(active) = state.active_value()"));
    assert!(select_section.contains("select = select.active(active);"));
    assert!(combobox_section.contains("if let Some(active) = state.active_value()"));
    assert!(combobox_section.contains("combobox = combobox.active(active);"));
    assert!(command_section.contains("if let Some(active) = state.active_value()"));
    assert!(command_section.contains("command = command.active(active);"));
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
    let release_queue = &samples[0];
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

    let filter_board = &samples[1];
    let filter_plan = filter_board.render_plan();
    let filter_summary = filter_board.state_summary();

    assert_eq!(filter_board.id, "filter-board");
    assert_eq!(filter_board.state.rows().len(), 180);
    assert_eq!(filter_plan.table().filtered_model().rows().len(), 60);
    assert_eq!(filter_plan.table().final_model().rows().len(), 24);
    assert_eq!(filter_summary.filtered_rows, 60);
    assert_eq!(filter_summary.final_rows, 24);
    assert_eq!(filter_summary.selected_rows, 1);
    assert_eq!(
        filter_plan.table().final_model().rows()[0].id().as_str(),
        "filter-board-row-177"
    );
    assert_eq!(filter_plan.table().final_model().selected_count(), 1);
    assert_eq!(filter_plan.aria_column_count(), 4);
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
    let cx = open_components_gallery(cx);

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
        "status-cue:component-status-cue:sync-warning:root",
        "empty-state:component-empty-state:no-results:root",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected Components page to render real component root `{selector}`"
        );
    }

    let tabs_sample = scroll_page_until_visible(cx, "gallery:component-tabs-sample:workspace-tabs");
    let page_scroll = bounds(cx, "gallery:page-scroll");

    assert!(
        page_scroll.contains(&tabs_sample.center()),
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
        focus_components_catalog_entry(&shell, cx, entry);
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
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "component-catalog:Table");
    click(cx, "component-catalog:Table");
    settle(cx);
    scroll_page_until_visible(cx, "gallery:component-table-sample:release-queue");

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
fn components_gallery_smoke_focused_command_samples_cover_depth_behaviors(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "component-catalog:Command");
    click(cx, "component-catalog:Command");
    settle(cx);

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
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "select:component-select:status-select:trigger");
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
    let cx = open_components_gallery(cx);

    jump_components_directory_to(cx, "gallery:component-page-jump:table");
    scroll_page_until_visible(cx, "gallery:component-table-sample:release-queue");
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
fn components_gallery_smoke_tree_expands_and_selects(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE: &str = "gallery:component-tree-sample:document-outline";
    const PAPER: &str = "tree:component-tree:document-outline:item:paper";
    const INTRO: &str = "tree:component-tree:document-outline:item:intro";

    let cx = open_components_gallery(cx);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());

    scroll_page_until_visible(cx, "component-catalog:Tree");
    click(cx, "component-catalog:Tree");
    settle(cx);
    scroll_page_until_visible(cx, SAMPLE);
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
    scroll_page_until_visible(cx, "gallery:component-tabs-sample:workspace-tabs");
    let directory_after_scroll = bounds(cx, "gallery:components-directory");

    assert_eq!(
        directory_after_scroll, directory_before,
        "expected the Components directory to stay fixed while the page content scrolls"
    );

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
        "expected the Components directory to stay fixed after clicking a jump"
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
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "gallery:component-tabs-sample:workspace-tabs");
    let before = bounds(
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    let tablist = bounds(cx, "tabs:component-tabs:workspace-tabs:tablist");

    cx.simulate_event(ScrollWheelEvent {
        position: tablist.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    redraw(cx);

    let after = bounds(
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
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
    let cx = open_components_gallery(cx);

    scroll_page_until_visible(cx, "gallery:component-splitter-sample:details-split");
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

    let restored_handle = bounds(cx, "splitter:component-splitter:details-split:handle:0").center();
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

    scroll_page_until_visible(cx, "gallery:component-tabs-sample:workspace-tabs");
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
