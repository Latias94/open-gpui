use open_gpui::{
    App, Bounds, BringIntoViewAlignment, BringIntoViewAxis, BringIntoViewBehavior,
    BringIntoViewCancelReason, BringIntoViewChainGeneration, BringIntoViewCompletion,
    BringIntoViewError, BringIntoViewMargins, BringIntoViewOptions, BringIntoViewOutcome,
    BringIntoViewRequestId, DeferredBringIntoViewGuard, FocusClaimOutcome, FocusHandle,
    IntoElement, Pixels, RevealTargetError, RevealTargetExt as _, RevealTargetHandle,
    ScrollChainFence, ScrollDirectMutationRevision, ScrollHandle, ScrollStrategy, Subscription,
    UniformListScrollHandle, Window, div, px,
};

#[test]
fn bring_into_view_capability_import_paths_compile() {
    fn bind(
        window: &mut Window,
        handle: &RevealTargetHandle,
        bounds: Bounds<Pixels>,
    ) -> Result<(), RevealTargetError> {
        window.bind_reveal_target(handle, bounds)
    }

    fn request(
        window: &mut Window,
        handle: &RevealTargetHandle,
        options: BringIntoViewOptions,
        cx: &mut App,
    ) -> Result<BringIntoViewRequestId, BringIntoViewError> {
        window.bring_into_view(handle, options, cx)
    }

    fn request_with_completion(
        window: &mut Window,
        handle: &RevealTargetHandle,
        options: BringIntoViewOptions,
        cx: &mut App,
    ) -> Result<(BringIntoViewRequestId, Subscription), BringIntoViewError> {
        window.bring_into_view_with_completion(handle, options, cx, |outcome, _, _| {
            let _: BringIntoViewOutcome = outcome;
        })
    }

    fn target(handle: &RevealTargetHandle) -> impl IntoElement {
        div().track_reveal_target(handle)
    }

    fn direct_scroll_revision(handle: &ScrollHandle) -> ScrollDirectMutationRevision {
        let before = handle.direct_scroll_revision();
        assert!(!before.horizontal_changed_since(before));
        assert!(!before.vertical_changed_since(before));
        before
    }

    fn capture_deferred_guard(
        window: &Window,
        handle: &RevealTargetHandle,
        options: BringIntoViewOptions,
    ) -> Result<DeferredBringIntoViewGuard, RevealTargetError> {
        window.capture_deferred_bring_into_view_guard(handle, options)
    }

    fn submit_deferred_guard(
        window: &mut Window,
        guard: DeferredBringIntoViewGuard,
        cx: &mut App,
    ) -> Result<Option<(BringIntoViewRequestId, Subscription)>, BringIntoViewError> {
        window.try_bring_into_view_with_guard_and_completion(guard, cx, |_, _, _| {})
    }

    fn deferred_guard_fence(guard: &DeferredBringIntoViewGuard) -> ScrollChainFence {
        guard.scroll_chain_fence()
    }

    fn capture_committed_fence(
        window: &Window,
        anchor: &RevealTargetHandle,
        options: BringIntoViewOptions,
    ) -> Result<Option<ScrollChainFence>, RevealTargetError> {
        window.capture_committed_scroll_chain_fence(anchor, options)
    }

    fn capture_current_fence(window: &Window, options: BringIntoViewOptions) -> ScrollChainFence {
        window.capture_current_scroll_chain_fence(options)
    }

    fn fence_was_interrupted(window: &Window, fence: &ScrollChainFence) -> bool {
        window.scroll_chain_fence_was_interrupted(fence)
    }

    fn fence_matches_current_ancestry(window: &Window, fence: &ScrollChainFence) -> bool {
        window.scroll_chain_fence_matches_current_ancestry(fence)
    }

    fn guarded_focus(
        window: &mut Window,
        handle: &FocusHandle,
        fence: ScrollChainFence,
        cx: &mut App,
    ) -> Subscription {
        window.focus_with_completion_and_scroll_fence(handle, fence, cx, |outcome, _, _| {
            let _: FocusClaimOutcome = outcome;
        })
    }

    fn post_commit(window: &mut Window) {
        window.record_prepaint_focus_stable_commit(|_, _, _| {});
    }

    let margins = BringIntoViewMargins::try_new(px(1.0), px(2.0), px(3.0), px(4.0))
        .expect("finite non-negative physical margins");
    let options = BringIntoViewOptions::vertical(BringIntoViewAlignment::Center)
        .with_horizontal(BringIntoViewAxis::Preserve)
        .with_vertical(BringIntoViewAxis::Align(BringIntoViewAlignment::MaxEdge))
        .with_margins(margins)
        .with_behavior(BringIntoViewBehavior::Instant);

    assert_eq!(options.horizontal_axis(), BringIntoViewAxis::Preserve);
    assert_eq!(options.margins().left(), px(1.0));
    let _ = BringIntoViewOptions::nearest();
    let _ = BringIntoViewOptions::aligned(BringIntoViewAlignment::MinEdge);
    let _ = BringIntoViewOptions::horizontal(BringIntoViewAlignment::Nearest);
    let _ = BringIntoViewOutcome::Completed(BringIntoViewCompletion::AlreadyVisible);
    let _ = BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::Superseded);

    let _ = Window::new_reveal_target as fn(&mut Window) -> RevealTargetHandle;
    let _ = bind as fn(&mut Window, &RevealTargetHandle, Bounds<Pixels>) -> Result<(), _>;
    let _ = request
        as fn(
            &mut Window,
            &RevealTargetHandle,
            BringIntoViewOptions,
            &mut App,
        ) -> Result<BringIntoViewRequestId, _>;
    let _ = request_with_completion;
    let _ = target;
    let _ = direct_scroll_revision as fn(&ScrollHandle) -> ScrollDirectMutationRevision;
    let _ = capture_deferred_guard
        as fn(
            &Window,
            &RevealTargetHandle,
            BringIntoViewOptions,
        ) -> Result<DeferredBringIntoViewGuard, RevealTargetError>;
    let _ = submit_deferred_guard;
    let _ = deferred_guard_fence;
    let _ = capture_committed_fence;
    let _ = capture_current_fence;
    let _ = fence_was_interrupted;
    let _ = fence_matches_current_ancestry;
    let _ = guarded_focus;
    let _ = post_commit as fn(&mut Window);
    let _ = RevealTargetHandle::window_id;
    let _ = BringIntoViewRequestId::window_id;
    let _ = BringIntoViewRequestId::sequence;
    let _ = BringIntoViewRequestId::chain_generation;
    let _ = BringIntoViewChainGeneration::get;
    let _ = Window::bring_into_view_authority_generation;
}

#[test]
fn low_level_uniform_list_scrolling_stays_explicit_and_encapsulated() {
    fn direct_methods(handle: &UniformListScrollHandle) -> ScrollHandle {
        handle.scroll_to_item(3, ScrollStrategy::Nearest);
        handle.scroll_to_item_strict(4, ScrollStrategy::Center);
        handle.scroll_to_item_with_offset(5, ScrollStrategy::Top, 1);
        handle.scroll_to_item_strict_with_offset(6, ScrollStrategy::Bottom, 2);
        handle.base_handle()
    }

    let _ = UniformListScrollHandle::new as fn() -> UniformListScrollHandle;
    let _ = direct_methods as fn(&UniformListScrollHandle) -> ScrollHandle;

    let uniform_list_source = include_str!("../src/elements/uniform_list.rs");
    let file = syn::parse_file(uniform_list_source).expect("uniform-list source should parse");
    let handle = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "UniformListScrollHandle" => Some(item),
            _ => None,
        })
        .expect("public UniformListScrollHandle should remain present");
    assert!(matches!(handle.vis, syn::Visibility::Public(_)));
    assert!(
        handle
            .fields
            .iter()
            .all(|field| !matches!(field.vis, syn::Visibility::Public(_))),
        "UniformListScrollHandle must not expose its mutable pending-scroll state"
    );

    for internal in ["DeferredScrollToItem", "UniformListScrollState"] {
        let item = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Struct(item) if item.ident == internal => Some(item),
                _ => None,
            })
            .unwrap_or_else(|| panic!("internal `{internal}` declaration should remain present"));
        assert!(matches!(item.vis, syn::Visibility::Inherited));
    }

    let list_source = include_str!("../src/elements/list.rs");
    assert!(
        !list_source.contains("pub fn scroll_to_reveal_item"),
        "ListState must not retain a second index-based reveal authority"
    );
}

#[test]
fn bring_into_view_identity_and_margin_fields_remain_opaque() {
    let source = include_str!("../src/window/bring_into_view.rs");
    let file = syn::parse_file(source).expect("bring-into-view source should parse");

    for name in [
        "RevealTargetHandle",
        "BringIntoViewMargins",
        "BringIntoViewChainGeneration",
        "BringIntoViewRequestId",
        "ScrollDirectMutationRevision",
        "DeferredBringIntoViewGuard",
        "ScrollChainFence",
    ] {
        let item = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Struct(item) if item.ident == name => Some(item),
                _ => None,
            })
            .unwrap_or_else(|| panic!("public `{name}` declaration should remain present"));
        assert!(matches!(item.vis, syn::Visibility::Public(_)));
        assert!(
            item.fields
                .iter()
                .all(|field| !matches!(field.vis, syn::Visibility::Public(_))),
            "`{name}` must not expose writable window, generation, sequence, or geometry fields"
        );
    }

    let snapshot = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "RevealTargetSnapshot" => Some(item),
            _ => None,
        })
        .expect("private reveal snapshot should remain an implementation detail");
    assert!(matches!(snapshot.vis, syn::Visibility::Inherited));
}

#[test]
fn generic_node_and_premature_logical_axis_surfaces_do_not_appear() {
    let source = include_str!("../src/window/bring_into_view.rs");
    for forbidden in [
        "BringIntoViewLogical",
        "BringIntoViewBlock",
        "BringIntoViewInline",
        "BringIntoViewStartEdge",
        "BringIntoViewEndEdge",
        "RevealNodeRef",
        "RevealNodeHandle",
        "raw_matrix",
        "transform_matrix",
    ] {
        assert!(
            !source.contains(forbidden),
            "bring-into-view public surface must not expose `{forbidden}`"
        );
    }
}
