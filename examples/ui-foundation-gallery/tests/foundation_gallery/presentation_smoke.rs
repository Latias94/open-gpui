use super::*;

fn presentation_a11y_node_with_role_and_label<'a>(
    update: &'a accesskit::TreeUpdate,
    role: accesskit::Role,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing {role:?} accessibility node labelled `{label}`"))
}

fn assert_accessibility_bounds_match(
    node: &accesskit::Node,
    expected: Bounds<Pixels>,
    scale_factor: f64,
    label: &str,
) {
    let actual = node
        .bounds()
        .unwrap_or_else(|| panic!("{label} accessibility node has no bounds"));
    let epsilon = 0.2;
    assert!(
        (actual.x0 - f64::from(expected.origin.x.as_f32()) * scale_factor).abs() <= epsilon
            && (actual.y0 - f64::from(expected.origin.y.as_f32()) * scale_factor).abs() <= epsilon
            && (actual.x1 - f64::from(expected.right().as_f32()) * scale_factor).abs() <= epsilon
            && (actual.y1 - f64::from(expected.bottom().as_f32()) * scale_factor).abs() <= epsilon,
        "{label} accessibility bounds {actual:?} did not match displayed bounds {expected:?}"
    );
}

fn assert_presentation_tooltip_open(cx: &mut VisualTestContext) {
    let tooltip = bounds(cx, "tooltip:tooltip:content");
    assert!(
        tooltip.size.width > px(0.0) && tooltip.size.height > px(0.0),
        "the Presentation tooltip should expose non-empty displayed bounds"
    );
    let layer = cx.update(|window, app| {
        WindowOverlayRuntime::for_window(window, app)
            .snapshot(window, app)
            .expect("Presentation tooltip should expose an overlay snapshot")
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "tooltip:tooltip")
            .cloned()
            .expect("Presentation tooltip should register its overlay layer")
    });
    assert_eq!(layer.phase(), OverlayLayerPhase::Open);
}

fn assert_presentation_input_accessibility_selection(cx: &mut VisualTestContext) {
    assert!(cx.activate_accessibility());
    let tree = cx
        .latest_accessibility_tree_update()
        .expect("Presentation TextInput should publish accessibility");
    let (_, input) = presentation_a11y_node_with_role_and_label(
        &tree,
        accesskit::Role::TextInput,
        "Presentation input",
    );
    assert!(
        input.text_selection().is_some(),
        "Presentation TextInput should publish its IME selection"
    );
}

fn focus_presentation_button_from_accessibility(cx: &mut VisualTestContext) {
    assert!(cx.activate_accessibility());
    let tree = cx
        .latest_accessibility_tree_update()
        .expect("Presentation Button should publish accessibility");
    let (button_id, _) =
        presentation_a11y_node_with_role_and_label(&tree, accesskit::Role::Button, "Run action");
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: button_id,
        data: None,
    }));
    redraw(cx);
    assert!(cx.debug_selector_is_focused("button:presentation-action:root"));
}

fn press_presentation_enter(cx: &mut VisualTestContext) {
    let enter = open_gpui::Keystroke {
        modifiers: open_gpui::Modifiers::none(),
        key: "enter".to_owned(),
        key_char: None,
    };
    cx.simulate_event(open_gpui::KeyDownEvent {
        keystroke: enter.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(open_gpui::KeyUpEvent { keystroke: enter });
    settle(cx);
}

fn open_presentation_action_tooltip(shell: &Entity<GalleryShell>, cx: &mut VisualTestContext) {
    scroll_page_selector_into_view(shell, cx, "button:presentation-action:root");
    let action = bounds(cx, "button:presentation-action:root");
    cx.simulate_mouse_move(action.center(), None, Default::default());
    redraw(cx);
    advance_and_redraw(cx, Duration::from_millis(500));
    assert_presentation_tooltip_open(cx);
}

#[open_gpui::test]
fn presentation_page_commits_geometry_and_routes_transformed_interactions(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    settle(cx);

    for selector in [
        "gallery:presentation-page",
        "gallery:presentation-stage",
        "gallery:presentation-inner",
        "gallery:presentation-action",
        "gallery:presentation-input",
        "gallery:presentation-scroll",
        "gallery:presentation-drag-source",
        "gallery:presentation-drop-target",
        "gallery:presentation-readout",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected Presentation selector `{selector}` to render"
        );
    }

    let projected_geometry = cx.update(|_, app| {
        shell
            .read(app)
            .presentation_geometry()
            .expect("Presentation stage should publish committed geometry")
    });
    assert_eq!(
        projected_geometry.layout_bounds().size,
        size(px(392.0), px(320.0))
    );
    assert_ne!(
        projected_geometry.layout_bounds(),
        projected_geometry.displayed_bounds(),
        "the projected mode should preserve layout while changing displayed geometry"
    );
    let projected_inner = bounds(cx, "gallery:presentation-inner");

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-action");
    click(cx, "gallery:presentation-action");
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        1,
        "the transformed semantic Button should receive pointer activation"
    );
    focus_presentation_button_from_accessibility(cx);
    press_presentation_enter(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        2,
        "the transformed semantic Button should receive keyboard activation"
    );

    open_presentation_action_tooltip(&shell, cx);

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-input");
    let input = bounds(cx, "gallery:presentation-input");
    cx.simulate_click(
        point(input.right() - px(10.0), input.center().y),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input(" scaled");
    settle(cx);
    assert!(
        cx.update(|_, app| {
            shell
                .read(app)
                .presentation_text_input()
                .read(app)
                .value()
                .ends_with(" scaled")
        }),
        "the transformed TextInput should inverse-project pointer placement and accept input"
    );
    cx.simulate_marked_text(None, "ni", Some(1..2));
    settle(cx);
    assert!(
        cx.update(|_, app| {
            let controller = shell.read(app).presentation_text_input().read(app);
            controller.value().contains("ni") && controller.marked_range_utf16().is_some()
        }),
        "the transformed TextInput should publish marked text and its UTF-16 range"
    );
    assert_presentation_input_accessibility_selection(cx);

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-scroll");
    let scroll_before = cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset());
    let scroll_position = visible_page_interaction_point(cx, "gallery:presentation-scroll");
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-48.0))),
        ..Default::default()
    });
    redraw(cx);
    let scroll_after = cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset());
    assert!(
        scroll_after.y < scroll_before.y,
        "the transformed ScrollArea should consume inverse-projected pixel wheel input"
    );

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-drop-target");
    let drag_source = bounds(cx, "gallery:presentation-drag-source");
    let drop_target = bounds(cx, "gallery:presentation-drop-target");
    drag(cx, drag_source.center(), drop_target.center());
    assert!(
        cx.update(|_, app| {
            shell
                .read(app)
                .presentation_drag_status()
                .starts_with("Dropped Payload at local")
        }),
        "the transformed drop target should receive its target-local drop geometry"
    );

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-mode:final");
    click(cx, "gallery:presentation-mode:final");
    settle(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_projection_progress()),
        1.0
    );
    let final_geometry = cx.update(|_, app| {
        shell
            .read(app)
            .presentation_geometry()
            .expect("Final Presentation stage should publish committed geometry")
    });
    assert_eq!(
        final_geometry.layout_bounds(),
        final_geometry.displayed_bounds()
    );
    let final_inner = bounds(cx, "gallery:presentation-inner");
    assert_ne!(
        projected_inner, final_inner,
        "projected and final modes should expose distinct displayed inner bounds"
    );

    let action_count = cx.update(|_, app| shell.read(app).presentation_action_count());
    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-action");
    click(cx, "gallery:presentation-action");
    focus_presentation_button_from_accessibility(cx);
    press_presentation_enter(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        action_count + 2,
        "identity-mode pointer and keyboard activation should share the semantic action path"
    );
    open_presentation_action_tooltip(&shell, cx);

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-input");
    let final_input = bounds(cx, "gallery:presentation-input");
    cx.simulate_click(final_input.center(), Default::default());
    settle(cx);
    cx.simulate_marked_text(None, "ok", Some(0..2));
    settle(cx);
    assert!(
        cx.update(|_, app| {
            shell
                .read(app)
                .presentation_text_input()
                .read(app)
                .marked_range_utf16()
                .is_some()
        }),
        "identity-mode TextInput should retain IME composition state"
    );
    assert_presentation_input_accessibility_selection(cx);

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-scroll");
    let final_scroll_before =
        cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset());
    let final_scroll_position = visible_page_interaction_point(cx, "gallery:presentation-scroll");
    cx.simulate_event(ScrollWheelEvent {
        position: final_scroll_position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(32.0))),
        ..Default::default()
    });
    redraw(cx);
    let final_scroll_after =
        cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset());
    assert_ne!(
        final_scroll_after, final_scroll_before,
        "identity-mode ScrollArea should consume wheel input"
    );

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-drop-target");
    let final_drag_source = bounds(cx, "gallery:presentation-drag-source");
    let final_drop_target = bounds(cx, "gallery:presentation-drop-target");
    drag(cx, final_drag_source.center(), final_drop_target.center());
    assert!(
        cx.update(|_, app| {
            shell
                .read(app)
                .presentation_drag_status()
                .starts_with("Dropped Payload at local")
        }),
        "identity-mode drag/drop should retain target-local geometry"
    );
}

#[open_gpui::test]
fn presentation_page_accessibility_projects_bounds_and_preserves_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    settle(cx);
    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-action");
    let projected_bounds = bounds(cx, "button:presentation-action:root");
    let scale_factor = cx.update(|window, _| f64::from(window.scale_factor()));

    assert!(cx.activate_accessibility());
    let projected_tree = cx
        .latest_accessibility_tree_update()
        .expect("projected Presentation page should publish accessibility");
    let (projected_id, projected_button) = presentation_a11y_node_with_role_and_label(
        &projected_tree,
        accesskit::Role::Button,
        "Run action",
    );
    assert_accessibility_bounds_match(
        projected_button,
        projected_bounds,
        scale_factor,
        "projected button",
    );
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: projected_id,
        data: None,
    }));
    settle(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        1,
        "AccessKit Click should activate the transformed Button"
    );

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-mode:final");
    click(cx, "gallery:presentation-mode:final");
    settle(cx);
    redraw(cx);
    assert!(cx.activate_accessibility());
    let final_tree = cx
        .latest_accessibility_tree_update()
        .expect("final Presentation page should publish accessibility");
    let (final_id, final_button) = presentation_a11y_node_with_role_and_label(
        &final_tree,
        accesskit::Role::Button,
        "Run action",
    );
    assert_eq!(
        projected_id, final_id,
        "transform changes must not replace the semantic Button node identity"
    );
    assert_accessibility_bounds_match(
        final_button,
        bounds(cx, "button:presentation-action:root"),
        scale_factor,
        "final button",
    );
}

#[open_gpui::test]
fn presentation_page_inspector_picks_projected_and_identity_geometry(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    cx.simulate_resize(size(px(1800.0), px(900.0)));
    settle(cx);
    cx.update(|window, app| window.toggle_inspector(app));
    redraw(cx);
    let projected_button =
        scroll_page_selector_into_view(&shell, cx, "button:presentation-action:root");
    cx.simulate_mouse_move(projected_button.center(), None, Default::default());
    redraw(cx);
    cx.simulate_click(projected_button.center(), Default::default());
    redraw(cx);
    let projected_id = cx
        .update(|window, app| window.inspector_active_element_id_for_test(app))
        .expect("projected Inspector pick should publish an active element")
        .path
        .global_id
        .to_string();
    assert!(
        projected_id.ends_with("presentation-action"),
        "projected Inspector pick should resolve the semantic Button path: {projected_id}"
    );
    assert!(!cx.update(|window, app| window.is_inspector_picking(app)));

    cx.update(|window, app| window.toggle_inspector(app));
    redraw(cx);
    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-mode:final");
    click(cx, "gallery:presentation-mode:final");
    settle(cx);
    scroll_page_selector_into_view(&shell, cx, "button:presentation-action:root");
    cx.update(|window, app| window.toggle_inspector(app));
    redraw(cx);
    let final_button =
        scroll_page_selector_into_view(&shell, cx, "button:presentation-action:root");
    cx.simulate_mouse_move(final_button.center(), None, Default::default());
    redraw(cx);
    cx.simulate_click(final_button.center(), Default::default());
    redraw(cx);
    let final_id = cx
        .update(|window, app| window.inspector_active_element_id_for_test(app))
        .expect("identity Inspector pick should publish an active element")
        .path
        .global_id
        .to_string();
    assert_eq!(projected_id, final_id);
}
