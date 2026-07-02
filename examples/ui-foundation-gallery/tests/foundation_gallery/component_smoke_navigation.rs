use super::*;

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

    assert!(
        cx.debug_bounds("gallery:components-section:tabs").is_none(),
        "expected all-mode to leave the deep Tabs section unmounted before the directory jump"
    );
    click(cx, "gallery:component-page-jump:tabs");
    settle(cx);
    settle(cx);

    let after = bounds(cx, "gallery:components-section:tabs");
    let viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    let directory_after_click = bounds(cx, "gallery:components-directory");

    assert!(
        (after.top() - viewport.top()).abs() <= px(1.0),
        "expected the Components page directory jump to align the Tabs section with the viewport top; after={after:?} viewport={viewport:?}"
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
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:tabs:component-tabs:workspace-tabs:tablist-scroll",
    );
    let sample_before = bounds(cx, "gallery:component-tabs-sample:workspace-tabs");
    let tablist = bounds(cx, "tabs:component-tabs:workspace-tabs:tablist");
    let tablist_viewport = bounds(
        cx,
        "scroll-area:tabs:component-tabs:workspace-tabs:tablist-scroll",
    );
    let tablist_position = visible_page_interaction_point(
        cx,
        "scroll-area:tabs:component-tabs:workspace-tabs:tablist-scroll",
    );
    assert!(
        tablist.contains(&tablist_viewport.center()),
        "expected vertical Tabs ScrollArea viewport to stay inside the tablist shell; tablist={tablist:?} viewport={tablist_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: tablist_position,
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
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:sidebar");
    scroll_page_selector_into_view(&shell, cx, "gallery:component-sidebar-sample:long-sidebar");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:component-sidebar:long-sidebar-scroll",
    );
    let sample_before = bounds(cx, "gallery:component-sidebar-sample:long-sidebar");
    let segments_before = bounds(cx, "sidebar:component-sidebar:long-sidebar:item:segments");
    let sidebar_position =
        visible_page_interaction_point(cx, "scroll-area:component-sidebar:long-sidebar-scroll");

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar_position,
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
    scroll_page_selector_into_view(
        &shell,
        cx,
        "scroll-area:tabs:component-tabs:workspace-tabs:tablist-scroll",
    );
    let tablist_position = visible_page_interaction_point(
        cx,
        "scroll-area:tabs:component-tabs:workspace-tabs:tablist-scroll",
    );
    let tab_before = bounds(
        cx,
        "tabs:component-tabs:workspace-tabs:trigger:component-tabs-item:workspace-tabs:billing",
    );
    cx.simulate_event(ScrollWheelEvent {
        position: tablist_position,
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
