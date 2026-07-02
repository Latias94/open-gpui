use super::*;

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
    let story = overlay_story_contract("Popover");
    let trigger = story
        .selectors()
        .trigger_selector()
        .expect("Popover story should declare a trigger selector");
    let content = story
        .selectors()
        .surface_selector()
        .expect("Popover story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.assert_story_can(&story, StoryProbeOperation::Focus);
    probe.scroll_page_to(trigger);
    probe.click(trigger);
    probe.settle();
    probe.assert_rendered(content, "opened Popover content");
    probe.assert_focused(trigger, "opened Popover trigger");

    let popover_content = probe.render_bounds(content);
    let outside_target = point(
        popover_content.right() + px(24.0),
        popover_content.bottom() + px(24.0),
    );
    probe.click_point(outside_target);
    probe.settle();

    probe.assert_not_rendered(content, "outside-dismissed Popover content");
    probe.assert_focused(trigger, "outside-dismissed Popover trigger");
}

#[open_gpui::test]
fn overlay_gallery_smoke_opens_tooltip_from_hover_focus_and_ignores_disabled(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("Tooltip");
    let hover_trigger = story
        .selectors()
        .trigger_selector()
        .expect("Tooltip story should declare a trigger selector");
    let hover_content = story
        .selectors()
        .surface_selector()
        .expect("Tooltip story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.assert_story_can(&story, StoryProbeOperation::Focus);
    probe.scroll_page_to(hover_trigger);
    probe.move_mouse_to(hover_trigger);
    probe.assert_rendered(hover_content, "hover-opened Tooltip content");

    let outside_target = probe.outside_gallery_point();
    probe.move_mouse_to_point(outside_target);
    probe.assert_not_rendered(hover_content, "hover-dismissed Tooltip content");

    let focus_trigger = "gallery:overlay-tooltip-trigger:focus-only";
    let focus_content = "tooltip:overlay-tooltip-content:focus-only:content";
    probe.scroll_page_to(focus_trigger);
    probe.click(focus_trigger);
    probe.redraw();
    probe.assert_rendered(focus_content, "focus-opened Tooltip content");

    let content_center = probe.outside_gallery_point();
    probe.click_point(content_center);
    probe.redraw();
    probe.assert_not_rendered(focus_content, "focus-dismissed Tooltip content");

    let disabled_trigger = "gallery:overlay-tooltip-trigger:disabled";
    probe.scroll_page_to(disabled_trigger);
    probe.move_mouse_to(disabled_trigger);
    probe.assert_not_rendered(
        "tooltip:overlay-tooltip-content:disabled:content",
        "disabled Tooltip content",
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_renders_manual_tooltip_from_state(cx: &mut open_gpui::TestAppContext) {
    let cx = open_overlay_gallery(cx);

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.scroll_page_to("gallery:overlay-tooltip-sample:delayed-manual");
    probe.redraw();
    probe.assert_rendered(
        "tooltip:overlay-tooltip-content:delayed-manual:content",
        "manual Tooltip content",
    );
}

#[open_gpui::test]
fn overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("HoverCard");
    let trigger = story
        .selectors()
        .trigger_selector()
        .expect("HoverCard story should declare a trigger selector");
    let content = story
        .selectors()
        .surface_selector()
        .expect("HoverCard story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.scroll_page_to(trigger);
    probe.click(trigger);
    probe.settle();
    probe.assert_rendered(content, "opened HoverCard content");

    let hover_card_content = probe.render_bounds(content);
    let outside_target = point(
        hover_card_content.right() + px(24.0),
        hover_card_content.bottom() + px(24.0),
    );
    probe.click_point(outside_target);
    probe.settle();

    probe.assert_not_rendered(content, "outside-dismissed HoverCard content");
}

#[open_gpui::test]
fn overlay_gallery_smoke_toggles_hover_card_from_control_surface(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("HoverCard");
    let control = story
        .selectors()
        .control_selector()
        .expect("HoverCard story should declare a control selector");
    let content = story
        .selectors()
        .surface_selector()
        .expect("HoverCard story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.scroll_page_to(control);
    probe.click(control);
    probe.settle();
    probe.assert_rendered(content, "controlled HoverCard content");

    probe.press_escape();
    probe.assert_not_rendered(content, "escape-dismissed HoverCard content");
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("Dialog");
    let trigger = story
        .selectors()
        .trigger_selector()
        .expect("Dialog story should declare a trigger selector");
    let surface = story
        .selectors()
        .surface_selector()
        .expect("Dialog story should declare a surface selector");
    let layer = "dialog:overlay-dialog-demo:controlled-modal:layer";

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.assert_story_can(&story, StoryProbeOperation::Focus);
    probe.scroll_page_to(trigger);
    probe.click(trigger);
    probe.settle();
    let dialog_layer = probe.render_bounds(layer);
    probe.assert_rendered(surface, "opened Dialog surface");
    probe.assert_focused(surface, "opened Dialog surface");

    probe.click_point(outside_top_left(dialog_layer));
    probe.settle();
    probe.assert_not_rendered(surface, "barrier-dismissed Dialog surface");
    probe.assert_focused(trigger, "barrier-dismissed Dialog trigger");

    probe.click(trigger);
    probe.settle();
    probe.assert_rendered(surface, "reopened Dialog surface");
    probe.press_escape();
    probe.assert_not_rendered(surface, "escape-dismissed Dialog surface");
    probe.assert_focused(trigger, "escape-dismissed Dialog trigger");
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("AlertDialog");
    let trigger = story
        .selectors()
        .trigger_selector()
        .expect("AlertDialog story should declare a trigger selector");
    let surface = story
        .selectors()
        .surface_selector()
        .expect("AlertDialog story should declare a surface selector");
    let cancel = "alert-dialog:overlay-alert-dialog-demo:destructive-confirm:cancel";
    let action = "alert-dialog:overlay-alert-dialog-demo:destructive-confirm:action";

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.assert_story_can(&story, StoryProbeOperation::Activate);
    probe.assert_story_can(&story, StoryProbeOperation::Focus);
    probe.scroll_page_to(trigger);
    probe.click(trigger);
    probe.settle();
    probe.assert_rendered(surface, "opened AlertDialog surface");
    probe.assert_focused(cancel, "opened AlertDialog cancel action");

    probe.click(action);
    probe.settle();
    probe.assert_not_rendered(surface, "action-dismissed AlertDialog surface");
    probe.assert_focused(trigger, "action-dismissed AlertDialog trigger");

    probe.click(trigger);
    probe.settle();
    probe.assert_rendered(surface, "reopened AlertDialog surface");
    probe.press_escape();
    probe.assert_not_rendered(surface, "escape-dismissed AlertDialog surface");
    probe.assert_focused(trigger, "escape-dismissed AlertDialog trigger");
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("Sheet");
    let trigger = story
        .selectors()
        .trigger_selector()
        .expect("Sheet story should declare a trigger selector");
    let surface = story
        .selectors()
        .surface_selector()
        .expect("Sheet story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.scroll_page_to(trigger);
    probe.click(trigger);
    probe.assert_rendered(surface, "opened Sheet surface");

    let outside_target = probe.outside_gallery_point();
    probe.click_point(outside_target);

    probe.assert_not_rendered(surface, "outside-dismissed Sheet surface");
}

#[open_gpui::test]
fn overlay_gallery_smoke_closes_menu_from_escape_and_outside_press(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_overlay_gallery(cx);
    let story = overlay_story_contract("Menu");
    let trigger = story
        .selectors()
        .trigger_selector()
        .expect("Menu story should declare a trigger selector");
    let content = story
        .selectors()
        .surface_selector()
        .expect("Menu story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.scroll_page_to(trigger);
    probe.click(trigger);
    probe.assert_rendered(content, "opened Menu content");
    probe.press_escape();
    probe.assert_not_rendered(content, "escape-dismissed Menu content");

    probe.click(trigger);
    probe.assert_rendered(content, "reopened Menu content");
    let outside_target = probe.outside_gallery_point();
    probe.click_point(outside_target);
    probe.assert_not_rendered(content, "outside-dismissed Menu content");
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
    let story = overlay_story_contract("ContextMenu");
    let hotspot = story
        .selectors()
        .trigger_selector()
        .expect("ContextMenu story should declare a hotspot selector");
    let surface = story
        .selectors()
        .surface_selector()
        .expect("ContextMenu story should declare a surface selector");

    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&story, StoryProbeOperation::Open);
    probe.assert_story_can(&story, StoryProbeOperation::Dismiss);
    probe.scroll_page_to(hotspot);
    probe.right_click(hotspot);
    probe.assert_rendered(surface, "right-click opened ContextMenu surface");

    probe.press_escape();

    probe.assert_not_rendered(surface, "escape-dismissed ContextMenu surface");

    probe.right_click(hotspot);
    let surface_bounds = probe.render_bounds(surface);
    let outside_target =
        visible_outside_point(probe.render_bounds("gallery:content"), surface_bounds);
    probe.click_point(outside_target);

    probe.assert_not_rendered(surface, "outside-dismissed ContextMenu surface");
}
