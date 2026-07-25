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

fn set_presentation_state(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    presentation: SubtreePresentation,
) {
    cx.update(|_, app| {
        shell.update(app, |shell, cx| {
            shell.set_presentation_state(presentation, cx)
        })
    });
    settle(cx);
}

fn set_presentation_progress(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    progress: f32,
) {
    cx.update(|_, app| {
        shell.update(app, |shell, cx| {
            shell.set_presentation_progress(progress, cx)
        })
    });
    settle(cx);
}

fn presentation_popover_is_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, app| {
        WindowOverlayRuntime::for_window(window, app)
            .snapshot(window, app)
            .is_ok_and(|snapshot| {
                snapshot.layers().iter().any(|layer| {
                    layer.id().as_str() == "popover:presentation-popover"
                        && layer.phase() == OverlayLayerPhase::Open
                })
            })
    })
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

fn settle_bring_into_view_demo(cx: &mut VisualTestContext) {
    for _ in 0..6 {
        cx.update(|window, cx| {
            window.drain_next_frame_callbacks_for_test(cx);
        });
        settle(cx);
    }
}

fn reset_bring_into_view_demo(shell: &Entity<GalleryShell>, cx: &mut VisualTestContext) {
    scroll_page_selector_into_view(shell, cx, "gallery:bring-into-view:reset");
    click(cx, "gallery:bring-into-view:reset");
    settle(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_offsets()),
        (point(px(0.0), px(0.0)), point(px(0.0), px(0.0)))
    );
}

fn focus_bring_into_view_demo_target_with_keyboard(cx: &mut VisualTestContext) {
    assert!(cx.activate_accessibility());
    let tree = cx
        .latest_accessibility_tree_update()
        .expect("focus-target command should publish accessibility");
    let (button_id, _) =
        presentation_a11y_node_with_role_and_label(&tree, accesskit::Role::Button, "Focus target");
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: button_id,
        data: None,
    }));
    settle(cx);
    assert!(cx.debug_selector_is_focused("button:bring-into-view-focus:root"));

    press_presentation_enter(cx);
    settle_bring_into_view_demo(cx);
    assert!(cx.debug_selector_is_focused("gallery:bring-into-view:target"));
}

fn assert_bring_into_view_demo_revealed(shell: &Entity<GalleryShell>, cx: &mut VisualTestContext) {
    let (outer, inner) = cx.update(|_, app| shell.read(app).bring_into_view_demo_offsets());
    let max_offsets = cx.update(|_, app| shell.read(app).bring_into_view_demo_max_offsets());
    let outcome = cx.update(|_, app| shell.read(app).bring_into_view_demo_outcome());
    assert!(
        outer.x < px(0.0) && outer.y < px(0.0),
        "expected both outer physical axes to scroll; outer={outer:?} inner={inner:?} max={max_offsets:?} outcome={outcome:?}"
    );
    assert!(
        inner.x < px(0.0) && inner.y < px(0.0),
        "expected both inner physical axes to scroll; offset={inner:?}"
    );

    let target = bounds(cx, "gallery:bring-into-view:target");
    let outer = bounds(cx, "gallery:bring-into-view:outer-scrollport");
    let inner = bounds(cx, "gallery:bring-into-view:inner-scrollport");
    assert!(outer.contains(&target.center()));
    assert!(inner.contains(&target.center()));
}

#[allow(clippy::too_many_arguments)]
fn assert_presentation_channels_suppressed(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    action: Bounds<Pixels>,
    popover: Bounds<Pixels>,
    scroll_position: open_gpui::Point<Pixels>,
    drag_source: Bounds<Pixels>,
    drop_target: Bounds<Pixels>,
    stale_button: accesskit::NodeId,
    expected_action_count: usize,
    expected_scroll: open_gpui::Point<Pixels>,
    expected_drag_status: &str,
) {
    cx.simulate_click(action.center(), Default::default());
    drag(cx, drag_source.center(), drop_target.center());
    cx.simulate_click(popover.center(), Default::default());
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: stale_button,
        data: None,
    }));
    // Keep wheel input last because a parent scroll surface may also consume it and move the page.
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-48.0))),
        ..Default::default()
    });
    settle(cx);

    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        expected_action_count
    );
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset()),
        expected_scroll
    );
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_drag_status().to_owned()),
        expected_drag_status
    );
    assert!(!cx.update(|_, app| app.has_active_drag()));
    assert!(!presentation_popover_is_open(cx));
}

fn restore_page_scroll(
    shell: &Entity<GalleryShell>,
    cx: &mut VisualTestContext,
    offset: open_gpui::Point<Pixels>,
) {
    let handle = cx.update(|_, app| shell.read(app).page_scroll_handle().clone());
    handle.set_offset(offset);
    redraw(cx);
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
        "gallery:presentation-popover",
        "gallery:presentation-action",
        "gallery:presentation-input",
        "gallery:presentation-scroll",
        "gallery:presentation-drag-source",
        "gallery:presentation-drop-target",
        "gallery:presentation-flow-sentinel",
        "gallery:presentation-matrix",
        "gallery:presentation-readout",
        "gallery:bring-into-view:demo",
        "gallery:bring-into-view:readout",
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
    let drag_status = cx.update(|_, app| shell.read(app).presentation_drag_status().to_owned());
    assert!(
        drag_status.starts_with("Dropped Payload at local"),
        "the transformed drop target should receive its target-local drop geometry; status={drag_status:?}"
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
fn presentation_bring_into_view_unifies_all_entry_paths_and_virtual_materialization(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    settle(cx);
    scroll_page_selector_into_view(&shell, cx, "gallery:bring-into-view:demo");

    reset_bring_into_view_demo(&shell, cx);
    click(cx, "gallery:bring-into-view:application");
    settle_bring_into_view_demo(cx);
    assert_bring_into_view_demo_revealed(&shell, cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_outcome()),
        Some(open_gpui::BringIntoViewOutcome::Completed(
            open_gpui::BringIntoViewCompletion::Revealed
        ))
    );

    reset_bring_into_view_demo(&shell, cx);
    focus_bring_into_view_demo_target_with_keyboard(cx);
    assert!(cx.debug_selector_is_focused("gallery:bring-into-view:target"));
    assert_bring_into_view_demo_revealed(&shell, cx);

    reset_bring_into_view_demo(&shell, cx);
    assert!(cx.activate_accessibility());
    let tree = cx
        .latest_accessibility_tree_update()
        .expect("bring-into-view target should publish accessibility");
    let (target_id, target) = presentation_a11y_node_with_role_and_label(
        &tree,
        accesskit::Role::Group,
        "Bring into view target",
    );
    assert!(!target.supports_action(accesskit::Action::Click));
    assert!(target.supports_action(accesskit::Action::ScrollIntoView));
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::ScrollIntoView,
        target_tree: accesskit::TreeId::ROOT,
        target_node: target_id,
        data: None,
    }));
    settle_bring_into_view_demo(cx);
    assert_bring_into_view_demo_revealed(&shell, cx);

    reset_bring_into_view_demo(&shell, cx);
    assert!(
        cx.debug_bounds(
            "virtualized-list:bring-into-view-demo-virtual-list:row:virtual-target-0080"
        )
        .is_none()
    );
    click(cx, "gallery:bring-into-view:virtual");
    settle_bring_into_view_demo(cx);
    let virtual_target = bounds(
        cx,
        "virtualized-list:bring-into-view-demo-virtual-list:row:virtual-target-0080",
    );
    let virtual_viewport = bounds(
        cx,
        "scroll-area:virtualized-list:bring-into-view-demo-virtual-list:viewport",
    );
    let inner_viewport = bounds(cx, "gallery:bring-into-view:inner-scrollport");
    let outer_viewport = bounds(cx, "gallery:bring-into-view:outer-scrollport");
    assert!(
        virtual_viewport.contains(&virtual_target.center())
            && inner_viewport.contains(&virtual_target.center())
            && outer_viewport.contains(&virtual_target.center()),
        "the materialized row must finish inside the virtual, inner, and outer viewports"
    );
    let (outer_offset, inner_offset) =
        cx.update(|_, app| shell.read(app).bring_into_view_demo_offsets());
    let virtual_offset = cx.update(|_, app| shell.read(app).bring_into_view_demo_virtual_offset());
    assert_ne!(virtual_offset.y, px(0.0));
    assert_ne!(inner_offset, point(px(0.0), px(0.0)));
    assert_ne!(outer_offset, point(px(0.0), px(0.0)));

    reset_bring_into_view_demo(&shell, cx);
    click(cx, "gallery:bring-into-view:animate");
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
    });
    redraw(cx);
    advance_and_redraw(cx, Duration::from_millis(100));
    let scroll_position =
        visible_page_interaction_point(cx, "gallery:bring-into-view:outer-scrollport");
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-16.0))),
        ..Default::default()
    });
    settle_bring_into_view_demo(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_outcome()),
        Some(open_gpui::BringIntoViewOutcome::Cancelled(
            open_gpui::BringIntoViewCancelReason::ScrollOverridden
        ))
    );
}

#[open_gpui::test]
fn presentation_bring_into_view_reset_invalidates_stale_callbacks(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    settle(cx);
    scroll_page_selector_into_view(&shell, cx, "gallery:bring-into-view:demo");

    reset_bring_into_view_demo(&shell, cx);
    click(cx, "gallery:bring-into-view:application");
    click(cx, "gallery:bring-into-view:reset");
    settle_bring_into_view_demo(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_offsets()),
        (point(px(0.0), px(0.0)), point(px(0.0), px(0.0)))
    );
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_outcome()),
        None,
        "a reset must invalidate a request that was only scheduled"
    );

    click(cx, "gallery:bring-into-view:animate");
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
    });
    redraw(cx);
    advance_and_redraw(cx, Duration::from_millis(100));
    click(cx, "gallery:bring-into-view:reset");
    settle_bring_into_view_demo(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_offsets()),
        (point(px(0.0), px(0.0)), point(px(0.0), px(0.0)))
    );
    assert_eq!(
        cx.update(|_, app| shell.read(app).bring_into_view_demo_outcome()),
        None,
        "a reset must ignore the terminal callback from an invalidated request"
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

#[open_gpui::test]
fn presentation_page_three_state_matrix_preserves_layout_and_requires_fresh_intent(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    cx.simulate_resize(size(px(1800.0), px(1000.0)));
    settle(cx);
    assert!(cx.activate_accessibility());
    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-slot");

    let page_scroll = cx.update(|_, app| shell.read(app).page_scroll_handle().offset());
    let slot = bounds(cx, "gallery:presentation-slot");
    let flow_sentinel = bounds(cx, "gallery:presentation-flow-sentinel");
    let matrix_visible_slot = bounds(cx, "gallery:presentation-matrix:visible:slot");
    let matrix_inert_slot = bounds(cx, "gallery:presentation-matrix:inert:slot");
    let matrix_hidden_slot = bounds(cx, "gallery:presentation-matrix:hidden:slot");
    assert_eq!(matrix_visible_slot.size, matrix_inert_slot.size);
    assert_eq!(matrix_visible_slot.size, matrix_hidden_slot.size);
    let matrix_visible_sentinel = bounds(cx, "gallery:presentation-matrix:visible:sentinel");
    let matrix_inert_sentinel = bounds(cx, "gallery:presentation-matrix:inert:sentinel");
    let matrix_hidden_sentinel = bounds(cx, "gallery:presentation-matrix:hidden:sentinel");
    assert_eq!(
        matrix_visible_sentinel.origin.y,
        matrix_inert_sentinel.origin.y
    );
    assert_eq!(
        matrix_visible_sentinel.origin.y, matrix_hidden_sentinel.origin.y,
        "the simultaneous Hidden lane must preserve the same child-driven flow height"
    );

    let projected_action = bounds(cx, "button:presentation-action:root");
    let projected_popover = bounds(cx, "popover:presentation-popover:trigger");
    let projected_input = bounds(cx, "gallery:presentation-input");
    let projected_scroll_position =
        visible_page_interaction_point(cx, "gallery:presentation-scroll");
    let projected_drag_source = bounds(cx, "gallery:presentation-drag-source");
    let projected_drop_target = bounds(cx, "gallery:presentation-drop-target");
    let projected_tree = cx
        .latest_accessibility_tree_update()
        .expect("visible Presentation page should publish accessibility");
    let (projected_button_id, _) = presentation_a11y_node_with_role_and_label(
        &projected_tree,
        accesskit::Role::Button,
        "Run action",
    );
    let action_count = cx.update(|_, app| shell.read(app).presentation_action_count());
    let projected_scroll =
        cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset());
    let projected_drag_status =
        cx.update(|_, app| shell.read(app).presentation_drag_status().to_owned());

    cx.simulate_click(projected_input.center(), Default::default());
    settle(cx);
    cx.simulate_marked_text(None, "gate", Some(0..4));
    settle(cx);
    assert!(cx.update(|_, app| {
        shell
            .read(app)
            .presentation_text_input()
            .read(app)
            .marked_range_utf16()
            .is_some()
    }));

    set_presentation_state(&shell, cx, SubtreePresentation::Inert);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_state()),
        SubtreePresentation::Inert
    );
    assert_eq!(bounds(cx, "gallery:presentation-slot"), slot);
    assert_eq!(
        bounds(cx, "gallery:presentation-flow-sentinel"),
        flow_sentinel
    );
    assert!(cx.update(|window, app| window.focused(app).is_none()));
    assert!(cx.update(|_, app| {
        shell
            .read(app)
            .presentation_text_input()
            .read(app)
            .marked_range_utf16()
            .is_none()
    }));
    assert_presentation_channels_suppressed(
        &shell,
        cx,
        projected_action,
        projected_popover,
        projected_scroll_position,
        projected_drag_source,
        projected_drop_target,
        projected_button_id,
        action_count,
        projected_scroll,
        &projected_drag_status,
    );
    restore_page_scroll(&shell, cx, page_scroll);
    let inert_tree = cx
        .latest_accessibility_tree_update()
        .expect("inert Presentation page should publish its remaining tree");
    assert!(
        inert_tree
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Run action"))
    );
    cx.simulate_mouse_move(projected_action.center(), None, Default::default());
    advance_and_redraw(cx, Duration::from_millis(500));
    let inert_tooltip_open = cx.update(|window, app| {
        WindowOverlayRuntime::for_window(window, app)
            .snapshot(window, app)
            .unwrap()
            .layers()
            .iter()
            .any(|layer| {
                layer.id().as_str() == "tooltip:tooltip" && layer.phase() == OverlayLayerPhase::Open
            })
    });
    assert!(!inert_tooltip_open);

    cx.update(|window, app| window.toggle_inspector(app));
    redraw(cx);
    cx.simulate_mouse_move(projected_action.center(), None, Default::default());
    cx.simulate_click(projected_action.center(), Default::default());
    redraw(cx);
    let inert_pick = cx
        .update(|window, app| window.inspector_active_element_id_for_test(app))
        .map(|id| id.path.global_id.to_string());
    assert!(
        inert_pick
            .as_deref()
            .is_none_or(|id| !id.ends_with("presentation-action")),
        "Inspector must not pick an inert descendant: {inert_pick:?}"
    );
    cx.update(|window, app| window.toggle_inspector(app));
    redraw(cx);

    set_presentation_state(&shell, cx, SubtreePresentation::Hidden);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_state()),
        SubtreePresentation::Hidden
    );
    assert_eq!(bounds(cx, "gallery:presentation-slot"), slot);
    assert_eq!(
        bounds(cx, "gallery:presentation-flow-sentinel"),
        flow_sentinel
    );
    assert!(cx.debug_bounds("gallery:presentation-stage").is_none());
    assert_presentation_channels_suppressed(
        &shell,
        cx,
        projected_action,
        projected_popover,
        projected_scroll_position,
        projected_drag_source,
        projected_drop_target,
        projected_button_id,
        action_count,
        projected_scroll,
        &projected_drag_status,
    );
    restore_page_scroll(&shell, cx, page_scroll);

    set_presentation_state(&shell, cx, SubtreePresentation::Visible);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_state()),
        SubtreePresentation::Visible
    );
    assert_eq!(bounds(cx, "gallery:presentation-slot"), slot);
    assert_eq!(
        bounds(cx, "gallery:presentation-flow-sentinel"),
        flow_sentinel
    );
    assert!(!cx.debug_selector_is_focused("text-input:presentation-text-input:root"));
    click(cx, "button:presentation-action:root");
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        action_count + 1,
        "restoration should accept only a fresh activation"
    );
    click(cx, "popover:presentation-popover:trigger");
    settle(cx);
    assert!(presentation_popover_is_open(cx));
    click(cx, "popover:presentation-popover:trigger");
    settle(cx);
    assert!(!presentation_popover_is_open(cx));

    set_presentation_progress(&shell, cx, 1.0);
    assert_eq!(bounds(cx, "gallery:presentation-slot"), slot);
    assert_eq!(
        bounds(cx, "gallery:presentation-flow-sentinel"),
        flow_sentinel
    );
    let identity_action = bounds(cx, "button:presentation-action:root");
    let identity_popover = bounds(cx, "popover:presentation-popover:trigger");
    let identity_scroll_position =
        visible_page_interaction_point(cx, "gallery:presentation-scroll");
    let identity_drag_source = bounds(cx, "gallery:presentation-drag-source");
    let identity_drop_target = bounds(cx, "gallery:presentation-drop-target");
    assert!(cx.activate_accessibility());
    let identity_tree = cx
        .latest_accessibility_tree_update()
        .expect("identity Presentation page should publish accessibility");
    let (identity_button_id, _) = presentation_a11y_node_with_role_and_label(
        &identity_tree,
        accesskit::Role::Button,
        "Run action",
    );
    let identity_count = cx.update(|_, app| shell.read(app).presentation_action_count());
    let identity_scroll = cx.update(|_, app| shell.read(app).presentation_scroll_handle().offset());
    let identity_drag_status =
        cx.update(|_, app| shell.read(app).presentation_drag_status().to_owned());

    set_presentation_state(&shell, cx, SubtreePresentation::Inert);
    assert_eq!(bounds(cx, "gallery:presentation-slot"), slot);
    assert_eq!(
        bounds(cx, "gallery:presentation-flow-sentinel"),
        flow_sentinel
    );
    assert_presentation_channels_suppressed(
        &shell,
        cx,
        identity_action,
        identity_popover,
        identity_scroll_position,
        identity_drag_source,
        identity_drop_target,
        identity_button_id,
        identity_count,
        identity_scroll,
        &identity_drag_status,
    );
    restore_page_scroll(&shell, cx, page_scroll);

    set_presentation_state(&shell, cx, SubtreePresentation::Hidden);
    assert_eq!(bounds(cx, "gallery:presentation-slot"), slot);
    assert_eq!(
        bounds(cx, "gallery:presentation-flow-sentinel"),
        flow_sentinel
    );
    assert_presentation_channels_suppressed(
        &shell,
        cx,
        identity_action,
        identity_popover,
        identity_scroll_position,
        identity_drag_source,
        identity_drop_target,
        identity_button_id,
        identity_count,
        identity_scroll,
        &identity_drag_status,
    );
    restore_page_scroll(&shell, cx, page_scroll);

    set_presentation_state(&shell, cx, SubtreePresentation::Visible);
    click(cx, "button:presentation-action:root");
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_action_count()),
        identity_count + 1,
        "identity restoration must also require a fresh activation"
    );
}

#[open_gpui::test]
fn presentation_clip_matrix_composes_exact_clips_with_runtime_descendants(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Presentation);
    set_presentation_progress(&shell, cx, 1.0);

    for selector in [
        "gallery:presentation-clips",
        "gallery:presentation-clips:rectangle",
        "gallery:presentation-clips:symmetric",
        "gallery:presentation-clips:asymmetric",
        "gallery:presentation-clips:nested",
        "gallery:presentation-clips:transformed",
        "gallery:presentation-clips:scrolling",
        "gallery:presentation-clips:deferred",
        "gallery:presentation-clips:canvas",
        "gallery:presentation-clips:interactive",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "expected subtree clip matrix selector `{selector}` to render"
        );
    }

    scroll_page_selector_into_view(&shell, cx, "gallery:presentation-clips:interactive-target");
    let target = bounds(cx, "gallery:presentation-clips:interactive-target");
    let count = cx.update(|_, app| shell.read(app).presentation_clip_action_count());
    cx.simulate_click(
        point(target.origin.x + px(1.0), target.origin.y + px(1.0)),
        Default::default(),
    );
    settle(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_clip_action_count()),
        count,
        "the target AABB corner must not activate through the rounded subtree clip"
    );

    cx.simulate_click(target.center(), Default::default());
    settle(cx);
    assert_eq!(
        cx.update(|_, app| shell.read(app).presentation_clip_action_count()),
        count + 1,
        "a point inside the rounded clip should retain ordinary pointer activation"
    );

    assert!(cx.activate_accessibility());
    let tree = cx
        .latest_accessibility_tree_update()
        .expect("the visible clipped target should publish accessibility");
    let (_, node) = presentation_a11y_node_with_role_and_label(
        &tree,
        accesskit::Role::Button,
        "Exact clipped activation",
    );
    assert!(node.supports_action(accesskit::Action::Click));
}
