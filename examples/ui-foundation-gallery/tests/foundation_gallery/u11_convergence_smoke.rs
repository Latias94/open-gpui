use super::*;

fn navigate_to_page(shell: &Entity<GalleryShell>, cx: &mut VisualTestContext, page: GalleryPage) {
    let selector = format!("gallery:navigation-item:{}", page.id());

    for _ in 0..24 {
        let viewport = bounds(cx, "gallery:navigation-scroll");
        let target = bounds(cx, &selector);
        if viewport.contains(&target.center()) {
            click(cx, &selector);
            settle(cx);
            assert_eq!(shell_snapshot(shell, cx).selected_page, page);
            return;
        }

        let delta_y = if target.center().y < viewport.top() {
            px(120.0)
        } else {
            px(-120.0)
        };
        cx.simulate_event(ScrollWheelEvent {
            position: viewport.center(),
            delta: ScrollDelta::Pixels(point(px(0.0), delta_y)),
            ..Default::default()
        });
        redraw(cx);
    }

    panic!("expected Gallery navigation to reveal `{selector}`");
}

fn a11y_node_with_role_and_label<'a>(
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

fn a11y_node_by_id(update: &accesskit::TreeUpdate, id: accesskit::NodeId) -> &accesskit::Node {
    update
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == id)
        .map(|(_, node)| node)
        .unwrap_or_else(|| panic!("missing accessibility node {id:?}"))
}

#[open_gpui::test]
fn u11_gallery_convergence_smoke_composes_real_authorities_in_one_window(
    cx: &mut open_gpui::TestAppContext,
) {
    const POPOVER_TRIGGER: &str = "popover:overlay-runtime-nested-popover:trigger";
    const POPOVER_CONTENT: &str = "popover:overlay-runtime-nested-popover:content";
    const MENU_TRIGGER: &str = "menu:overlay-runtime-nested-menu:trigger";
    const MENU_CONTENT: &str = "menu:overlay-runtime-nested-menu:content";
    const DIALOG_TRIGGER: &str = "dialog:overlay-runtime-nested-dialog:trigger";
    const DIALOG_SURFACE: &str = "dialog:overlay-runtime-nested-dialog:surface";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Overlay);
    let _ = cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    settle(cx);
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.scroll_page_to(POPOVER_TRIGGER);
        probe.click(POPOVER_TRIGGER);
        probe.settle();
        probe.assert_rendered(POPOVER_CONTENT, "nested Popover content");
        probe.assert_focused(POPOVER_TRIGGER, "nested Popover trigger");

        probe.click(MENU_TRIGGER);
        probe.settle();
        probe.assert_rendered(MENU_CONTENT, "nested Menu content");
        probe.assert_focused(MENU_CONTENT, "nested Menu content");

        probe.click(DIALOG_TRIGGER);
        probe.settle();
        probe.assert_rendered(DIALOG_SURFACE, "nested Dialog surface");
        probe.assert_focused(DIALOG_SURFACE, "nested Dialog surface");

        let popover = probe.overlay_layer("popover:overlay-runtime-nested-popover");
        let menu = probe.overlay_layer("menu:overlay-runtime-nested-menu");
        let dialog = probe.overlay_layer("dialog:overlay-runtime-nested-dialog");
        assert_eq!(popover.kind(), OverlayLayerKind::NonModalDismissible);
        assert_eq!(popover.phase(), OverlayLayerPhase::Open);
        assert_eq!(popover.parent(), None);
        assert_eq!(menu.kind(), OverlayLayerKind::Menu);
        assert_eq!(
            menu.parent().map(|parent| parent.as_str()),
            Some("popover:overlay-runtime-nested-popover")
        );
        assert_eq!(dialog.kind(), OverlayLayerKind::Modal);
        assert_eq!(
            dialog.parent().map(|parent| parent.as_str()),
            Some("menu:overlay-runtime-nested-menu")
        );
    }

    cx.update(|window, app| {
        shell.update(app, |shell, cx| shell.refresh_devtools(window, cx));
    });
    settle(cx);
    let live_capture = cx.update(|_, app| {
        shell
            .read(app)
            .devtools_workbench()
            .inspector_state()
            .current_capture()
    });
    let overlay_snapshot = live_capture
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "overlay.window")
        .expect("live Gallery DevTools must project the window overlay authority");
    let open_overlay_payloads = overlay_snapshot.tree.nodes[0]
        .children
        .iter()
        .filter_map(|node| node.payload.as_ref())
        .filter(|payload| payload["presence"] == "open")
        .collect::<Vec<_>>();
    assert!(
        open_overlay_payloads.len() >= 3,
        "the live DevTools frame must observe the nested Popover, Menu, and Dialog"
    );
    assert!(open_overlay_payloads.iter().any(|payload| {
        payload["kind"] == "modal"
            && payload["parent"].as_str().is_some()
            && payload["focus_active"] == true
    }));

    let runtime_snapshot = live_capture
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "gpui.runtime.gallery")
        .expect("live Gallery DevTools must project GPUI runtime authority");
    let focus_payload = runtime_snapshot.tree.nodes[0]
        .children
        .iter()
        .filter_map(|node| node.payload.as_ref())
        .find(|payload| payload.get("focused_element_rendered").is_some())
        .expect("live GPUI runtime snapshot must include focus authority");
    assert_eq!(focus_payload["focused_window_id"], 1);
    assert_eq!(focus_payload["focused_element_rendered"], true);
    assert!(focus_payload["focus_claim_revision"].as_u64().unwrap() > 0);
    assert!(focus_payload["rendered_frame_revision"].as_u64().unwrap() > 0);
    assert_eq!(focus_payload["focus_scope_count"], serde_json::Value::Null);
    assert_eq!(focus_payload["focus_handle_count"], serde_json::Value::Null);

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.press_escape();
        probe.assert_not_rendered(DIALOG_SURFACE, "dismissed nested Dialog surface");
        probe.assert_rendered(MENU_CONTENT, "Menu retained after Dialog dismissal");
        probe.assert_focused(MENU_CONTENT, "Menu restored after Dialog dismissal");

        probe.press_escape();
        probe.assert_not_rendered(MENU_CONTENT, "dismissed nested Menu content");
        probe.assert_rendered(POPOVER_CONTENT, "Popover retained after Menu dismissal");
        probe.assert_focused(MENU_TRIGGER, "Menu trigger restored after Menu dismissal");

        probe.press_escape();
        probe.drain_next_frame();
        probe.drain_next_frame();
        probe.settle();
        probe.assert_not_rendered(POPOVER_CONTENT, "dismissed nested Popover content");
        probe.assert_focused(
            POPOVER_TRIGGER,
            "Popover trigger restored after Popover dismissal",
        );
    }

    navigate_to_page(&shell, cx, GalleryPage::Tokens);
    assert!(cx.debug_bounds("gallery:theme-scope:dark").is_some());
    assert!(
        cx.debug_bounds("gallery:theme-scope:high-contrast")
            .is_some()
    );
    assert!(
        cx.debug_bounds("button:gallery-theme-scope-app-button:root")
            .is_some(),
        "sibling ThemeScopes must restore the app theme for following content"
    );
    assert!(
        cx.debug_bounds("popover:gallery-theme-scope-dark-popover:content")
            .is_none()
    );
    click(cx, "popover:gallery-theme-scope-dark-popover:trigger");
    settle(cx);
    assert!(
        cx.debug_bounds("popover:gallery-theme-scope-dark-popover:content")
            .is_some(),
        "the dark ThemeScope should open its real deferred Popover"
    );
    assert!(
        cx.debug_bounds("button:gallery-theme-scope-overlay-action:root")
            .is_some(),
        "the deferred scoped surface should render its official child"
    );
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.press_escape();
        probe.drain_next_frame();
        probe.drain_next_frame();
        probe.settle();
        probe.assert_not_rendered(
            "popover:gallery-theme-scope-dark-popover:content",
            "dismissed scoped Popover content",
        );
    }

    navigate_to_page(&shell, cx, GalleryPage::Components);
    jump_components_directory_to(cx, "gallery:component-page-jump:ecosystem-adapters");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-form-adapter-sample:validating",
    );
    assert!(
        cx.debug_bounds("gallery:component-form-adapter-sample:validating")
            .is_some(),
        "the real pending-validation FormStore sample should be rendered"
    );
    assert!(cx.activate_accessibility());
    let form_tree = cx
        .latest_accessibility_tree_update()
        .expect("validating Form sample should publish an accessibility tree");
    let validating_email = form_tree
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == accesskit::Role::TextInput
                && node.is_busy()
                && node.value() == Some("pending@example.com")
        })
        .map(|(_, node)| node)
        .expect("real pending validation should publish a busy Email TextInput");
    assert_eq!(validating_email.value(), Some("pending@example.com"));
    assert_eq!(validating_email.labelled_by().len(), 1);
    let email_label = a11y_node_by_id(&form_tree, validating_email.labelled_by()[0]);
    assert_eq!(email_label.role(), accesskit::Role::Label);
    assert_eq!(email_label.label(), Some("Email"));
    assert!(cx.deactivate_accessibility());

    cx.set_global(pages::components::SidebarSampleRuntimeLog::default());
    jump_components_directory_to(cx, "gallery:component-page-jump:sidebar");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-sidebar-sample:workspace-sidebar",
    );
    let projects = "sidebar:component-sidebar%3Aworkspace-sidebar:item:projects";
    let sidebar_viewport = "scroll-area:sidebar:component-sidebar%3Aworkspace-sidebar:scroll";
    let viewport = bounds(cx, sidebar_viewport);
    scroll_until_visible(
        cx,
        sidebar_viewport,
        projects,
        12,
        point(px(0.0), px(36.0)),
        viewport.center(),
        |container, target| container.contains(&target.center()),
        "expected the Projects Sidebar item to become visible".to_owned(),
    );
    scroll_page_selector_into_view(&shell, cx, projects);
    let projects_point = visible_page_interaction_point(cx, projects);
    click_point(cx, projects_point);
    settle(cx);
    let activations = cx.read_global::<pages::components::SidebarSampleRuntimeLog, _>(|log, _| {
        log.activations().to_vec()
    });
    assert_eq!(
        activations.len(),
        1,
        "one pointer gesture must invoke the semantic activation handler exactly once"
    );
    assert_eq!(activations[0].sample_id(), "workspace-sidebar");
    assert_eq!(activations[0].activation().value(), "projects");
    assert_eq!(
        activations[0].source(),
        open_gpui_ui_components::ActivationSource::Pointer
    );

    const TREE_PAPER: &str = "tree:component-tree:document-outline:item:paper";
    const TREE_INTRO: &str = "tree:component-tree:document-outline:item:intro";
    const TREE_NOTES: &str = "tree:component-tree:document-outline:item:notes";
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    jump_components_directory_to(cx, "gallery:component-page-jump:tree");
    scroll_page_selector_into_view(&shell, cx, "gallery:component-tree-sample:document-outline");
    scroll_page_selector_into_view(&shell, cx, TREE_PAPER);
    click(cx, TREE_PAPER);
    assert!(cx.debug_selector_is_focused(TREE_PAPER));
    cx.simulate_keystrokes("right");
    redraw(cx);
    assert!(
        cx.debug_bounds(TREE_INTRO).is_some(),
        "the focused Paper branch should expand through the real Tree handler"
    );
    cx.simulate_keystrokes("n o");
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(TREE_NOTES),
        "Tree typeahead should refine `n o` to the visible Notes row"
    );
    advance_and_redraw(cx, Duration::from_millis(701));
    cx.simulate_keystrokes("p");
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(TREE_PAPER),
        "advancing the fake clock past the timeout should reset Tree typeahead before `p`"
    );

    navigate_to_page(&shell, cx, GalleryPage::FocusAccessibility);
    scroll_page_selector_into_view(
        &shell,
        cx,
        pages::focus_a11y::TEXTAREA_FIELD_ERROR_TOGGLE_SELECTOR,
    );
    assert!(cx.activate_accessibility());
    let final_tree = cx
        .latest_accessibility_tree_update()
        .expect("Focus/A11y page should publish its final accessibility tree");
    let (_, text_input) = a11y_node_with_role_and_label(
        &final_tree,
        accesskit::Role::TextInput,
        pages::focus_a11y::TEXT_INPUT_LABEL,
    );
    assert_eq!(
        text_input.value(),
        Some(pages::focus_a11y::TEXT_INPUT_INITIAL_VALUE)
    );
    assert!(text_input.supports_action(accesskit::Action::SetValue));
    assert!(text_input.supports_action(accesskit::Action::SetTextSelection));

    let (label_id, _) = a11y_node_with_role_and_label(
        &final_tree,
        accesskit::Role::Label,
        pages::focus_a11y::TEXTAREA_FIELD_LABEL,
    );
    let (help_id, _) = a11y_node_with_role_and_label(
        &final_tree,
        accesskit::Role::Label,
        pages::focus_a11y::TEXTAREA_FIELD_HELP,
    );
    let textarea = final_tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::MultilineTextInput)
        .map(|(_, node)| node)
        .expect("Focus/A11y Field should publish its Textarea control");
    assert_eq!(
        textarea.value(),
        Some(pages::focus_a11y::TEXTAREA_INITIAL_VALUE)
    );
    assert_eq!(textarea.labelled_by(), &[label_id]);
    assert_eq!(textarea.described_by(), &[help_id]);
    assert!(textarea.supports_action(accesskit::Action::SetValue));
}
