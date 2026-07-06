use super::*;

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
    assert!(
        cx.debug_bounds("gallery:component-button-sample:default")
            .is_none(),
        "expected Components all-mode to lazy-render deep samples instead of mounting every section immediately"
    );

    for (jump, expected_selector) in [
        (
            "gallery:component-page-jump:primitives",
            "separator:component-separator:section-rule:root",
        ),
        (
            "gallery:component-page-jump:feedback",
            "status-cue:component-status-cue:sync-warning:root",
        ),
        (
            "gallery:component-page-jump:state-contracts",
            "gallery:component-tree-state-contract:document-outline",
        ),
        (
            "gallery:component-page-jump:button",
            "gallery:component-button-sample:default",
        ),
    ] {
        jump_components_directory_to(cx, jump);
        assert!(
            cx.debug_bounds(expected_selector).is_some(),
            "expected Components page jump `{jump}` to render lazy target `{expected_selector}`"
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
    let table_story = component_story_contract("Table");
    let button_story = component_story_contract("Button");
    let tabs_story = component_story_contract("Tabs");
    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let mut probe = StoryRuntimeProbe::new(cx);
    probe.assert_story_can(&table_story, StoryProbeOperation::Scroll);
    probe.assert_story_can(&table_story, StoryProbeOperation::Edit);
    for sample in table_samples {
        probe.assert_rendered(
            &sample.debug_selector(),
            &format!("focused Table sample `{}`", sample.id),
        );
    }
    probe.assert_not_rendered(
        button_story
            .selectors()
            .primary_selector()
            .expect("Button story should declare a sample selector"),
        "unrelated Button sample",
    );
    probe.assert_not_rendered(
        tabs_story
            .selectors()
            .primary_selector()
            .expect("Tabs story should declare a sample selector"),
        "sibling Tabs sample",
    );
    probe.assert_rendered("gallery:components-directory", "focused mode directory");

    probe.click("gallery:component-focus:all");
    probe.settle();

    assert_eq!(
        shell_snapshot(&shell, probe.cx).components_focus,
        pages::components::ComponentFocusMode::All
    );
    probe.assert_not_rendered(
        button_story
            .selectors()
            .primary_selector()
            .expect("Button story should declare a sample selector"),
        "lazy Button sample after all-mode restoration",
    );
    probe.assert_not_rendered(
        tabs_story
            .selectors()
            .primary_selector()
            .expect("Tabs story should declare a sample selector"),
        "lazy Tabs sample after all-mode restoration",
    );
    probe.assert_rendered(
        button_story
            .selectors()
            .catalog_selector()
            .expect("Button story should declare a catalog selector"),
        "Button catalog card after all-mode restoration",
    );
}

#[open_gpui::test]
fn components_gallery_smoke_focuses_every_focusable_catalog_entry(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let mut visited = Vec::new();
    let focusable_stories = pages::components::component_story_contracts_for_focus(
        pages::components::ComponentFocusMode::All,
    )
    .into_iter()
    .filter(|story| story.section_id().is_some())
    .collect::<Vec<_>>();
    let expected = focusable_stories.len();

    for story in focusable_stories {
        let entry = pages::components::COMPONENT_CATALOG
            .iter()
            .find(|entry| entry.name == story.owner_name())
            .unwrap_or_else(|| panic!("expected catalog entry `{}`", story.owner_name()));
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
        component_story_contract("Button")
            .selectors()
            .primary_selector()
            .expect("Button story should declare a sample selector"),
        component_story_contract("Tabs")
            .selectors()
            .primary_selector()
            .expect("Tabs story should declare a sample selector"),
    ] {
        assert!(
            cx.debug_bounds(selector).is_none(),
            "expected all-mode restoration after matrix traversal to keep deep lazy sample `{selector}` unmounted"
        );
    }
    let button_story = component_story_contract("Button");
    StoryRuntimeProbe::new(cx).assert_rendered(
        button_story
            .selectors()
            .catalog_selector()
            .expect("Button story should declare a catalog selector"),
        "Button catalog card after matrix all-mode restoration",
    );

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

    jump_components_directory_to(cx, "gallery:component-page-jump:textarea");
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, VIEWPORT);

    let sample_before = bounds(cx, SAMPLE);
    let line_before = bounds(cx, LINE);
    let viewport_position = visible_page_interaction_point(cx, VIEWPORT);

    cx.simulate_event(ScrollWheelEvent {
        position: viewport_position,
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
        "gallery:component-command-sample:diagnostics-empty",
        "gallery:component-command-sample:keymap-resolution",
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
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-command-sample:diagnostics-empty",
    );
    assert!(
        cx.debug_bounds("command:component-command:diagnostics-empty:status")
            .is_some(),
        "expected diagnostics Command sample to render component-owned status list"
    );
    assert!(
        cx.debug_bounds("command:component-command:diagnostics-empty:status:0")
            .is_some(),
        "expected diagnostics Command sample to render a provider error status item"
    );
    assert!(
        cx.debug_bounds("listbox:component-command:diagnostics-empty-listbox:empty")
            .is_some(),
        "expected diagnostics Command sample to render the empty state inside the command list"
    );
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-command-sample:keymap-resolution",
    );
    assert!(
        cx.debug_bounds("gallery:component-command-sample:keymap-resolution:keymap-resolution")
            .is_some(),
        "expected keymap Command sample to render keymap resolution readouts"
    );
    assert!(
        cx.debug_bounds("gallery:component-command-sample:keymap-resolution:keymap-resolution:0")
            .is_some(),
        "expected keymap Command sample to render the pending chord readout"
    );
    assert!(
        cx.debug_bounds("gallery:component-command-sample:keymap-resolution:keymap-resolution:4")
            .is_some(),
        "expected keymap Command sample to render the missing command readout"
    );
    assert!(
        cx.debug_bounds("gallery:component-command-sample:keymap-resolution:shortcut-inspector")
            .is_some(),
        "expected keymap Command sample to render shortcut inspector state"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:shortcut-inspector:matched:0"
        )
        .is_some(),
        "expected keymap Command sample to render the matched shortcut inspector command"
    );
    assert!(
        cx.debug_bounds("gallery:component-command-sample:keymap-resolution:keybinding-editor")
            .is_some(),
        "expected keymap Command sample to render keybinding editor state"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor:row:0"
        )
        .is_some(),
        "expected keymap Command sample to render keybinding editor rows"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor:conflict:0"
        )
        .is_some(),
        "expected keymap Command sample to render keybinding editor conflicts"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor:diagnostic:0"
        )
        .is_some(),
        "expected keymap Command sample to render keybinding editor diagnostics"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor-preview"
        )
        .is_some(),
        "expected keymap Command sample to render keybinding edit preview"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor-preview:capture"
        )
        .is_some(),
        "expected keymap Command sample to render captured keybinding input"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor-preview:patch"
        )
        .is_some(),
        "expected keymap Command sample to render keybinding patch preview"
    );
    assert!(
        cx.debug_bounds(
            "gallery:component-command-sample:keymap-resolution:keybinding-editor-preview:row:0"
        )
        .is_some(),
        "expected keymap Command sample to render keybinding preview rows"
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
fn components_gallery_smoke_focused_choice_search_state_readouts_render(
    cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    for (name, sample_selector, state_readout_selector) in [
        (
            "Listbox",
            "gallery:component-listbox-sample:assignee-listbox",
            "gallery:component-listbox-sample:assignee-listbox:state",
        ),
        (
            "Select",
            "gallery:component-select-sample:priority-select",
            "gallery:component-select-sample:priority-select:state",
        ),
        (
            "Combobox",
            "gallery:component-combobox-sample:framework-combobox",
            "gallery:component-combobox-sample:framework-combobox:state",
        ),
        (
            "Command",
            "gallery:component-command-sample:ranked-search",
            "gallery:component-command-sample:ranked-search:state",
        ),
    ] {
        let entry = pages::components::COMPONENT_CATALOG
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("expected catalog entry `{name}`"));
        let story = component_story_contract(name);

        for operation in [
            StoryProbeOperation::Open,
            StoryProbeOperation::Select,
            StoryProbeOperation::Focus,
            StoryProbeOperation::ReadPublicPayload,
        ] {
            assert!(
                story.has_operation(operation),
                "expected focused choice/search story `{name}` to declare `{}`",
                operation.as_str()
            );
        }

        assert_eq!(
            story.selectors().state_readout_selector(),
            Some(state_readout_selector)
        );

        focus_components_section(&shell, cx, entry);
        scroll_page_selector_into_view(&shell, cx, state_readout_selector);

        assert!(
            cx.debug_bounds(sample_selector).is_some(),
            "expected focused choice/search story `{name}` to render sample `{sample_selector}`"
        );
        assert!(
            cx.debug_bounds(state_readout_selector).is_some(),
            "expected focused choice/search story `{name}` to render state readout `{state_readout_selector}`"
        );
    }
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
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

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

    jump_components_directory_to(cx, "gallery:component-page-jump:scroll-area");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-scroll-area-sample:release-queue",
    );
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:component-scroll-area:release-queue",
    );
    let queue_before = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");
    let queue_position =
        visible_page_interaction_point(cx, "scroll-area:component-scroll-area:release-queue");

    cx.simulate_event(ScrollWheelEvent {
        position: queue_position,
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
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:scroll-area");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-scroll-area-sample:release-queue",
    );
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:component-scroll-area:release-queue",
    );
    let sample_before = bounds(cx, "gallery:component-scroll-area-sample:release-queue");
    let queue_before = bounds(cx, "gallery:component-scroll-area-item:release-queue:2");
    let queue_position =
        visible_page_interaction_point(cx, "scroll-area:component-scroll-area:release-queue");

    cx.simulate_event(ScrollWheelEvent {
        position: queue_position,
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
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:scroll-area");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "gallery:component-scroll-area-sample:release-queue",
    );
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
