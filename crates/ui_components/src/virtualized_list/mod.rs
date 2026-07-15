//! Renderer-neutral state for virtualized list surfaces.

mod data;
mod descriptor;
mod model;
mod motion;
mod render;
mod render_plan;
pub(crate) mod runtime;
mod style;

#[cfg(test)]
use open_gpui_motion::{
    MotionFrameDemand, MotionFrameResetReason, MotionPreference, advanced::MotionPreset,
};
#[cfg(test)]
use open_gpui_ui_core::ui_px;
#[cfg(test)]
use open_gpui_ui_core::virtualizer::VirtualizerGeometryCache;
#[cfg(test)]
use open_gpui_ui_core::{Role, Sizable, Size, UiPx, VirtualizerItemKey, VirtualizerSnapshot};
#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::time::{Duration, Instant};

pub use self::data::{VirtualizedListDataSource, VirtualizedListDataSourceBuilder};
pub use self::descriptor::{
    VirtualizedListItemDescriptor, VirtualizedListRowKind, VirtualizedListStatusKind,
};
pub use self::model::{
    VirtualizedListActivation, VirtualizedListRevealResult, VirtualizedListRevealTarget,
    VirtualizedListScrollStrategy, VirtualizedListSelectionChange, VirtualizedListSelectionMode,
    VirtualizedListState, VirtualizedListStateItem,
};
pub use self::render_plan::{
    VirtualizedListBehaviorSnapshot, VirtualizedListRowBehaviorSnapshot,
    VirtualizedListRowMeasureMode, VirtualizedListRowRenderContext,
    VirtualizedListStickyOverlaySnapshot, VirtualizedListStickySectionSnapshot,
};
pub use self::runtime::VirtualizedList;
pub use self::style::{VirtualizedListColors, VirtualizedListMetrics};

#[cfg(test)]
use self::motion::VirtualizedListActiveIndicatorRuntime;
#[cfg(test)]
use self::render_plan::VirtualizedListRenderPlan;

#[cfg(test)]
mod tests {
    use super::*;

    fn indicator_plan(active_key: &str, scroll_offset: UiPx) -> VirtualizedListRenderPlan {
        let items = (0..12)
            .map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("indicator-row-{index:02}"),
                    format!("Row {index:02}"),
                )
            })
            .collect::<Vec<_>>();
        let metrics = VirtualizedListMetrics::from_size(Size::Small)
            .with_row_height(ui_px(20.0))
            .with_overscan_count(0);
        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            items.iter().map(VirtualizedListStateItem::from),
            Some(active_key),
            [active_key],
            VirtualizedListSelectionMode::Single,
            Some(3),
        )
        .with_metrics(metrics);

        VirtualizedListRenderPlan::resolve(
            "indicator-list",
            "Indicator list",
            state,
            &items,
            VirtualizedListRowMeasureMode::Fixed,
            &BTreeMap::new(),
            None,
            scroll_offset,
            ui_px(60.0),
        )
    }

    #[test]
    fn virtualized_list_state_resolves_active_from_keys_and_preserves_metrics() {
        let items = (0..10)
            .map(|index| VirtualizedListStateItem::new(format!("item-{index}"), index.to_string()))
            .collect::<Vec<_>>();

        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            items,
            Some("item-12"),
            ["item-4"],
            VirtualizedListSelectionMode::Single,
            Some(5),
        );

        assert_eq!(state.size(), Size::Small);
        assert_eq!(state.item_count(), 10);
        assert_eq!(state.active_key(), Some("item-4"));
        assert_eq!(state.active_index(), Some(4));
        assert_eq!(state.selected_index(), Some(4));
        assert_eq!(state.selected_keys(), ["item-4"]);
        assert_eq!(state.viewport_item_count(), 5);
        assert_eq!(state.metrics().row_height(), ui_px(28.0));
        assert!(!state.visible_empty());
    }

    #[test]
    fn virtualized_list_navigation_stays_inside_range() {
        let items = (0..12)
            .map(|index| VirtualizedListStateItem::new(format!("item-{index}"), index.to_string()))
            .collect::<Vec<_>>();
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            items,
            Some("item-6"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(4),
        );

        assert_eq!(state.navigation_target("home"), Some(0));
        assert_eq!(state.navigation_target("end"), Some(11));
        assert_eq!(state.navigation_target("up"), Some(5));
        assert_eq!(state.navigation_target("down"), Some(7));
        assert_eq!(state.navigation_target("pageup"), Some(2));
        assert_eq!(state.navigation_target("pagedown"), Some(10));
    }

    #[test]
    fn virtualized_list_navigation_skips_disabled_rows() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta").disabled(true),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(2),
        );

        assert_eq!(state.navigation_target("down"), Some(2));
        assert_eq!(state.navigation_target("end"), Some(2));
    }

    #[test]
    fn virtualized_list_typeahead_targets_selectable_text_values_from_active() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("recent", "Recent")
                    .row_kind(VirtualizedListRowKind::Section),
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta").disabled(true),
                VirtualizedListStateItem::new("gamma", "Delta Cargo"),
                VirtualizedListStateItem::new("tail", "Tail"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(3),
        );

        assert_eq!(
            state.typeahead_target("de").map(|item| item.key()),
            Some("gamma")
        );
        assert_eq!(
            state.typeahead_target("  TA ").map(|item| item.key()),
            Some("tail")
        );
        assert_eq!(
            state.typeahead_target("AL").map(|item| item.key()),
            Some("alpha"),
            "typeahead should wrap after the active row"
        );
        assert_eq!(state.typeahead_target("be").map(|item| item.key()), None);
        assert_eq!(
            state.typeahead_target("recent").map(|item| item.key()),
            None
        );
        assert_eq!(state.typeahead_target("").map(|item| item.key()), None);
    }

    #[test]
    fn virtualized_list_typeahead_skips_duplicate_keys() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("duplicate", "Duplicate first"),
                VirtualizedListStateItem::new("duplicate", "Duplicate second"),
                VirtualizedListStateItem::new("delta", "Delta"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(3),
        );

        assert_eq!(state.typeahead_target("du").map(|item| item.key()), None);
        assert_eq!(
            state.typeahead_target("de").map(|item| item.key()),
            Some("delta")
        );
    }

    #[test]
    fn virtualized_list_data_source_projects_domain_items_and_status_rows() {
        #[derive(Clone)]
        struct ReleaseRow {
            id: &'static str,
            title: &'static str,
            owner: &'static str,
            blocked: bool,
        }

        let source = VirtualizedListDataSource::builder()
            .prepend_loading("releases-before", "Loading previous releases")
            .section("release-section", "Release queue")
            .mapped_items(
                [
                    ReleaseRow {
                        id: "release-1",
                        title: "Ready release",
                        owner: "Platform",
                        blocked: false,
                    },
                    ReleaseRow {
                        id: "release-2",
                        title: "Blocked release",
                        owner: "Design",
                        blocked: true,
                    },
                ],
                |row| {
                    let descriptor = VirtualizedListItemDescriptor::item(row.id, row.title)
                        .secondary_text(row.owner)
                        .badge("release");
                    if row.blocked {
                        descriptor.disabled_reason("Waiting for review")
                    } else {
                        descriptor
                    }
                },
            )
            .append_loading("releases-after", "Loading archived releases")
            .exhausted("releases-end", "All releases loaded")
            .build();

        assert_eq!(source.len(), 6);
        assert_eq!(source.selectable_count(), 1);
        assert_eq!(source.items()[0].kind(), VirtualizedListRowKind::Loading);
        assert_eq!(source.items()[1].kind(), VirtualizedListRowKind::Section);
        assert_eq!(source.items()[2].key(), "release-1");
        assert!(source.items()[3].disabled_state());
        assert_eq!(
            source.items()[3].disabled_reason_ref(),
            Some("Waiting for review")
        );
        assert_eq!(
            source.items()[5].status_kind(),
            Some(VirtualizedListStatusKind::Exhausted)
        );

        let list = VirtualizedList::from_data_source("release-list", "Releases", source.clone())
            .default_active_key("release-2")
            .default_selected_key("release-1");
        let state = list.state();
        assert_eq!(state.item_count(), 6);
        assert_eq!(state.active_key(), Some("release-1"));
        assert_eq!(state.selected_keys(), ["release-1"]);
    }

    #[test]
    fn virtualized_list_data_source_adds_empty_when_no_selectable_items() {
        let source = VirtualizedListDataSource::builder()
            .section("recent", "Recent")
            .empty_when_no_selectable("empty", "No recent items")
            .build();

        assert_eq!(source.len(), 2);
        assert_eq!(source.selectable_count(), 0);
        assert_eq!(source.items()[1].kind(), VirtualizedListRowKind::Empty);

        let source_with_item = VirtualizedListDataSource::builder()
            .section("recent", "Recent")
            .item(VirtualizedListItemDescriptor::item("alpha", "Alpha"))
            .empty_when_no_selectable("empty", "No recent items")
            .build();

        assert_eq!(source_with_item.len(), 2);
        assert_eq!(source_with_item.selectable_count(), 1);
        assert_eq!(source_with_item.items()[1].key(), "alpha");
    }

    #[test]
    fn virtualized_list_range_selection_replaces_selected_keys_in_current_order() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("recent", "Recent")
                    .row_kind(VirtualizedListRowKind::Section),
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta").disabled(true),
                VirtualizedListStateItem::new("gamma", "Gamma"),
                VirtualizedListStateItem::new("delta", "Delta"),
            ],
            Some("alpha"),
            ["delta"],
            VirtualizedListSelectionMode::Multiple,
            Some(4),
        );

        let change = state
            .range_selection_change(Some("alpha"), "delta")
            .expect("range selection should replace the selected set");
        assert_eq!(change.changed_key(), "delta");
        assert_eq!(change.selected_keys(), ["alpha", "gamma", "delta"]);
    }

    #[test]
    fn virtualized_list_range_selection_falls_back_to_active_anchor() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("beta"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Multiple,
            Some(3),
        );

        let change = state
            .range_selection_change(Some("missing"), "gamma")
            .expect("missing anchor should fall back to active row");
        assert_eq!(change.selected_keys(), ["beta", "gamma"]);

        let single = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(2),
        );
        assert!(
            single
                .range_selection_change(Some("alpha"), "beta")
                .is_none()
        );
    }

    #[test]
    fn virtualized_list_empty_or_disabled_state_has_no_targets() {
        let empty = VirtualizedListState::resolve(
            Size::Medium,
            false,
            Vec::<VirtualizedListStateItem>::new(),
            None,
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            None,
        );
        let disabled = VirtualizedListState::resolve(
            Size::Medium,
            true,
            (0..10).map(|index| {
                VirtualizedListStateItem::new(format!("item-{index}"), index.to_string())
            }),
            Some("item-2"),
            ["item-2"],
            VirtualizedListSelectionMode::Single,
            None,
        );

        assert!(empty.visible_empty());
        assert_eq!(empty.active_index(), None);
        assert_eq!(empty.navigation_target("down"), None);
        assert_eq!(disabled.active_index(), None);
        assert_eq!(disabled.selected_index(), None);
        assert_eq!(disabled.activation_for_key("enter"), None);
    }

    #[test]
    fn virtualized_list_duplicate_keys_are_not_semantic_targets() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("duplicate", "First duplicate"),
                VirtualizedListStateItem::new("duplicate", "Second duplicate"),
                VirtualizedListStateItem::new("tail", "Tail"),
            ],
            Some("duplicate"),
            ["duplicate"],
            VirtualizedListSelectionMode::Single,
            Some(3),
        );

        assert_eq!(state.active_key(), Some("tail"));
        assert_eq!(state.active_index(), Some(2));
        assert!(state.selected_keys().is_empty());
        assert_eq!(
            state.scroll_target_for_key(
                "duplicate",
                VirtualizedListScrollStrategy::Nearest,
                ui_px(84.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::DuplicateKey("duplicate".to_owned())
        );
    }

    #[test]
    fn virtualized_list_state_resolves_selection_by_key_after_reorder() {
        let items = [
            VirtualizedListStateItem::new("alpha", "Alpha"),
            VirtualizedListStateItem::new("beta", "Beta"),
            VirtualizedListStateItem::new("gamma", "Gamma"),
        ];
        let reordered = [items[2].clone(), items[0].clone(), items[1].clone()];

        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            reordered,
            Some("gamma"),
            ["beta"],
            VirtualizedListSelectionMode::Multiple,
            Some(3),
        );

        assert_eq!(state.active_key(), Some("gamma"));
        assert_eq!(state.active_index(), Some(0));
        assert_eq!(state.selected_keys(), ["beta"]);
        assert!(state.selected_key_set().contains("beta"));
        assert_eq!(state.selected_indices(), [2]);
    }

    #[test]
    fn virtualized_list_multi_select_space_toggles_and_enter_activates() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
            ],
            Some("beta"),
            ["alpha"],
            VirtualizedListSelectionMode::Multiple,
            Some(2),
        );

        let change = state
            .selection_change_for_key("space")
            .expect("space should toggle selection in multi-select mode");
        assert_eq!(change.changed_key(), "beta");
        assert_eq!(change.selected_keys(), ["alpha", "beta"]);
        assert_eq!(state.activation_for_key("space"), None);

        let activation = state
            .activation_for_key("enter")
            .expect("enter should activate the active key");
        assert_eq!(activation.key(), "beta");
        assert_eq!(activation.index(), 1);
        assert_eq!(activation.text_value(), "Beta");
    }

    #[test]
    fn virtualized_list_scroll_to_key_reports_reveal_result() {
        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta").disabled(true),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("alpha"),
            ["alpha"],
            VirtualizedListSelectionMode::Single,
            Some(2),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Small).with_row_height(ui_px(28.0)));

        assert_eq!(
            state.scroll_target_for_key(
                "beta",
                VirtualizedListScrollStrategy::Top,
                ui_px(56.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::Disabled("beta".to_owned())
        );
        assert_eq!(
            state.scroll_target_for_key(
                "missing",
                VirtualizedListScrollStrategy::Top,
                ui_px(56.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::NotFound("missing".to_owned())
        );
    }

    #[test]
    fn virtualized_list_status_kinds_are_explicit_and_never_selectable() {
        let rows = [
            VirtualizedListItemDescriptor::loading("initial", "Loading releases"),
            VirtualizedListItemDescriptor::empty("empty", "No releases"),
            VirtualizedListItemDescriptor::append_loading("append", "Loading more"),
            VirtualizedListItemDescriptor::prepend_loading("prepend", "Loading previous"),
            VirtualizedListItemDescriptor::exhausted("done", "End of releases"),
            VirtualizedListItemDescriptor::error("error", "Failed to load"),
            VirtualizedListItemDescriptor::retry("retry", "Failed to refresh", "Retry"),
        ];
        let snapshot = VirtualizedList::new("status-list", "Status list", rows)
            .default_active_key("append")
            .default_selected_keys(["append", "retry"])
            .behavior_snapshot();
        let statuses = snapshot
            .rows()
            .iter()
            .map(|row| {
                (
                    row.key(),
                    row.kind(),
                    row.status_kind(),
                    row.retry_action_label(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(snapshot.state().active_key(), None);
        assert!(snapshot.state().selected_keys().is_empty());
        assert_eq!(snapshot.rows().len(), 7);
        assert_eq!(
            statuses[0].2,
            Some(VirtualizedListStatusKind::InitialLoading)
        );
        assert_eq!(statuses[1].2, Some(VirtualizedListStatusKind::Empty));
        assert_eq!(
            statuses[2].2,
            Some(VirtualizedListStatusKind::AppendLoading)
        );
        assert_eq!(
            statuses[3].2,
            Some(VirtualizedListStatusKind::PrependLoading)
        );
        assert_eq!(statuses[4].2, Some(VirtualizedListStatusKind::Exhausted));
        assert_eq!(statuses[5].2, Some(VirtualizedListStatusKind::Error));
        assert_eq!(statuses[6].2, Some(VirtualizedListStatusKind::Retry));
        assert_eq!(statuses[6].3, Some("Retry"));
        assert!(snapshot.rows().iter().all(|row| {
            row.position_in_set().is_none() && row.size_of_set() == 0 && !row.item().selectable()
        }));
        assert_eq!(snapshot.rows()[0].role(), Role::ProgressIndicator);
        assert_eq!(snapshot.rows()[2].role(), Role::ProgressIndicator);
        assert_eq!(snapshot.rows()[4].role(), Role::Section);
        assert_eq!(snapshot.rows()[6].role(), Role::AlertDialog);
        assert_eq!(
            snapshot.state().scroll_target_for_key(
                "retry",
                VirtualizedListScrollStrategy::Nearest,
                ui_px(84.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::StatusRow("retry".to_owned())
        );
    }

    #[test]
    fn virtualized_list_status_kind_labels_are_stable() {
        assert_eq!(
            VirtualizedListStatusKind::InitialLoading.as_str(),
            "initial-loading"
        );
        assert_eq!(VirtualizedListStatusKind::Empty.as_str(), "empty");
        assert_eq!(
            VirtualizedListStatusKind::AppendLoading.as_str(),
            "append-loading"
        );
        assert_eq!(
            VirtualizedListStatusKind::PrependLoading.as_str(),
            "prepend-loading"
        );
        assert_eq!(VirtualizedListStatusKind::Exhausted.as_str(), "exhausted");
        assert_eq!(VirtualizedListStatusKind::Error.as_str(), "error");
        assert_eq!(VirtualizedListStatusKind::Retry.as_str(), "retry");
    }

    #[test]
    fn virtualized_list_scroll_strategy_labels_are_stable() {
        assert_eq!(VirtualizedListScrollStrategy::Nearest.as_str(), "nearest");
        assert_eq!(VirtualizedListScrollStrategy::Top.as_str(), "top");
        assert_eq!(VirtualizedListScrollStrategy::Center.as_str(), "center");
        assert_eq!(VirtualizedListScrollStrategy::Bottom.as_str(), "bottom");
    }

    #[test]
    fn virtualized_list_behavior_snapshot_preserves_roles_metadata_and_keys() {
        let items = vec![
            VirtualizedListItemDescriptor::new("root", "Root"),
            VirtualizedListItemDescriptor::new("duplicate", "First"),
            VirtualizedListItemDescriptor::new("duplicate", "Second").disabled(true),
            VirtualizedListItemDescriptor::new("tail", "Tail"),
        ];
        let snapshot = VirtualizedList::new("virtualized-list", "Virtualized list", items)
            .with_size(Size::Small)
            .default_active_key("tail")
            .default_selected_key("root")
            .viewport_item_count(2)
            .behavior_snapshot_with_viewport(ui_px(56.0), ui_px(56.0));

        assert_eq!(snapshot.role(), Role::ListBox);
        assert_eq!(snapshot.row_role(), Role::ListBoxOption);
        assert_eq!(snapshot.list_id(), "virtualized-list");
        assert_eq!(snapshot.label(), "Virtualized list");
        assert_eq!(snapshot.visible_row_count(), 2);
        assert_eq!(snapshot.overscan_count(), 4);
        assert_eq!(snapshot.rows().len(), 4);
        assert_eq!(snapshot.rows()[0].item().key(), "root");
        assert_eq!(
            snapshot.rows()[1].render_key(),
            "#duplicate:0:1:9:duplicate"
        );
        assert_eq!(
            snapshot.rows()[2].render_key(),
            "#duplicate:0:2:9:duplicate"
        );
        assert!(snapshot.rows()[0].selected());
        assert!(snapshot.rows()[2].disabled());
        assert!(snapshot.rows()[3].active());
        assert_eq!(snapshot.rows()[2].position_in_set(), None);
        assert_eq!(snapshot.rows()[2].size_of_set(), 2);
        assert_eq!(snapshot.rows()[2].virtual_start(), ui_px(56.0));
        assert_eq!(snapshot.rows()[2].virtual_size(), ui_px(28.0));
        assert!(snapshot.active_row().is_some());
        assert!(snapshot.selected_row().is_some());
        assert_eq!(
            snapshot
                .rows()
                .iter()
                .map(|row| row.render_key())
                .collect::<Vec<_>>(),
            [
                "root",
                "#duplicate:0:1:9:duplicate",
                "#duplicate:0:2:9:duplicate",
                "tail"
            ]
        );
    }

    #[test]
    fn virtualized_list_duplicate_render_keys_cannot_alias_legal_source_keys() {
        let colliding_source_key = "#duplicate:0:0:1:a";
        let snapshot = VirtualizedList::new(
            "collision-list",
            "Collision list",
            [
                VirtualizedListItemDescriptor::new("a", "First duplicate"),
                VirtualizedListItemDescriptor::new("a", "Second duplicate"),
                VirtualizedListItemDescriptor::new(colliding_source_key, "Legal source key"),
            ],
        )
        .viewport_item_count(3)
        .behavior_snapshot_with_viewport(UiPx::ZERO, ui_px(120.0));
        let render_keys = snapshot
            .rows()
            .iter()
            .map(|row| row.render_key())
            .collect::<Vec<_>>();

        assert_eq!(render_keys[0], "#duplicate:1:0:1:a");
        assert_eq!(render_keys[1], "#duplicate:0:1:1:a");
        assert_eq!(render_keys[2], colliding_source_key);
        assert_eq!(
            render_keys.iter().copied().collect::<BTreeSet<_>>().len(),
            3
        );
    }

    #[test]
    fn virtualized_list_typed_item_snapshot_preserves_anatomy() {
        let snapshot = VirtualizedList::new(
            "typed-list",
            "Typed list",
            [
                VirtualizedListItemDescriptor::item("release-42", "Release 42")
                    .secondary_text("Platform / Ready")
                    .with_text_value("release forty two platform ready")
                    .leading_metadata("UI")
                    .trailing_metadata("12 files")
                    .badge("Ready")
                    .status("Verified"),
            ],
        )
        .default_active_key("release-42")
        .default_selected_key("release-42")
        .behavior_snapshot();
        let row = &snapshot.rows()[0];

        assert_eq!(row.kind(), VirtualizedListRowKind::Item);
        assert_eq!(row.label(), "Release 42");
        assert_eq!(row.secondary_text(), Some("Platform / Ready"));
        assert_eq!(row.text_value(), "release forty two platform ready");
        assert_eq!(row.leading_metadata(), Some("UI"));
        assert_eq!(row.trailing_metadata(), Some("12 files"));
        assert_eq!(row.badge(), Some("Ready"));
        assert_eq!(row.status(), Some("Verified"));
        assert_eq!(row.position_in_set(), Some(1));
        assert_eq!(row.size_of_set(), 1);
        assert_eq!(
            snapshot
                .state()
                .activation_for_key("enter")
                .map(|activation| activation.text_value().to_owned()),
            Some("release forty two platform ready".to_owned())
        );
    }

    #[test]
    fn virtualized_list_sections_and_separators_are_not_selectable_options() {
        let snapshot = VirtualizedList::new(
            "sectioned-list",
            "Sectioned list",
            [
                VirtualizedListItemDescriptor::section("recent", "Recent"),
                VirtualizedListItemDescriptor::new("alpha", "Alpha"),
                VirtualizedListItemDescriptor::separator("split"),
                VirtualizedListItemDescriptor::new("beta", "Beta").disabled_reason("Offline"),
            ],
        )
        .default_active_key("recent")
        .default_selected_keys(["recent", "beta"])
        .behavior_snapshot();

        assert_eq!(snapshot.state().active_key(), Some("alpha"));
        assert!(snapshot.state().selected_keys().is_empty());
        assert_eq!(snapshot.rows()[0].kind(), VirtualizedListRowKind::Section);
        assert_eq!(snapshot.rows()[0].role(), Role::Group);
        assert_eq!(snapshot.rows()[0].position_in_set(), None);
        assert_eq!(snapshot.rows()[0].size_of_set(), 1);
        assert_eq!(snapshot.rows()[1].position_in_set(), Some(1));
        assert_eq!(snapshot.rows()[2].kind(), VirtualizedListRowKind::Separator);
        assert_eq!(snapshot.rows()[2].role(), Role::Separator);
        assert_eq!(snapshot.rows()[2].position_in_set(), None);
        assert_eq!(snapshot.rows()[3].disabled_reason(), Some("Offline"));
        assert_eq!(snapshot.rows()[3].position_in_set(), None);
        assert_eq!(snapshot.rows()[3].size_of_set(), 1);
        assert_eq!(
            snapshot.state().scroll_target_for_key(
                "recent",
                VirtualizedListScrollStrategy::Nearest,
                ui_px(84.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::StructuralRow("recent".to_owned())
        );
    }

    #[test]
    fn virtualized_list_snapshot_reports_sticky_section_metadata() {
        let snapshot = VirtualizedList::new(
            "grouped-list",
            "Grouped list",
            [
                VirtualizedListItemDescriptor::section("recent", "Recent"),
                VirtualizedListItemDescriptor::new("alpha", "Alpha"),
                VirtualizedListItemDescriptor::section("archived", "Archived"),
                VirtualizedListItemDescriptor::new("gamma", "Gamma"),
                VirtualizedListItemDescriptor::new("delta", "Delta"),
            ],
        )
        .row_height(ui_px(20.0))
        .overscan(0)
        .behavior_snapshot_with_viewport(ui_px(60.0), ui_px(40.0));

        let sticky = snapshot
            .sticky_section()
            .expect("visible rows should resolve their owning section");
        let overlay = snapshot
            .sticky_overlay()
            .expect("grouped list should resolve a presentation overlay");
        assert_eq!(sticky.key(), "archived");
        assert_eq!(sticky.label(), "Archived");
        assert_eq!(sticky.index(), 2);
        assert_eq!(overlay.section().key(), "archived");
        assert!(!overlay.source_row_visible());
        assert_eq!(overlay.role(), None);
        assert!(!overlay.focusable());
        assert!(!overlay.pointer_interactive());
        assert!(!overlay.allows_interactive_content());
        assert_eq!(snapshot.state().active_key(), Some("alpha"));
        assert_eq!(snapshot.rows()[0].key(), "gamma");
        assert_eq!(snapshot.rows()[0].position_in_set(), Some(2));
    }

    #[test]
    fn virtualized_list_snapshot_omits_sticky_section_without_visible_item() {
        let ungrouped = VirtualizedList::new(
            "ungrouped-list",
            "Ungrouped list",
            [
                VirtualizedListItemDescriptor::new("alpha", "Alpha"),
                VirtualizedListItemDescriptor::new("beta", "Beta"),
            ],
        )
        .behavior_snapshot();
        let status_only = VirtualizedList::new(
            "status-list",
            "Status list",
            [VirtualizedListItemDescriptor::loading("loading", "Loading")],
        )
        .behavior_snapshot();

        assert!(ungrouped.sticky_section().is_none());
        assert!(ungrouped.sticky_overlay().is_none());
        assert!(status_only.sticky_section().is_none());
        assert!(status_only.sticky_overlay().is_none());
    }

    #[test]
    fn virtualized_list_status_rows_suppress_activation_and_expose_roles() {
        let loading = VirtualizedList::new(
            "loading-list",
            "Loading list",
            [VirtualizedListItemDescriptor::loading(
                "loading",
                "Loading releases",
            )],
        )
        .default_active_key("loading")
        .behavior_snapshot();
        let empty = VirtualizedList::new(
            "empty-list",
            "Empty list",
            [VirtualizedListItemDescriptor::empty("empty", "No releases")],
        )
        .behavior_snapshot();
        let error = VirtualizedList::new(
            "error-list",
            "Error list",
            [VirtualizedListItemDescriptor::error(
                "error",
                "Failed to load",
            )],
        )
        .behavior_snapshot();

        assert!(!loading.state().visible_empty());
        assert_eq!(loading.state().active_key(), None);
        assert_eq!(loading.state().activation_for_key("enter"), None);
        assert_eq!(loading.rows()[0].role(), Role::ProgressIndicator);
        assert_eq!(loading.rows()[0].position_in_set(), None);
        assert_eq!(loading.rows()[0].size_of_set(), 0);
        assert_eq!(empty.rows()[0].role(), Role::Section);
        assert_eq!(error.rows()[0].role(), Role::AlertDialog);
    }

    #[test]
    fn virtualized_list_measured_mode_restores_snapshot_by_key() {
        let mut items = vec![
            VirtualizedListItemDescriptor::new("beta", "Beta"),
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
        ];
        items.extend((2..100).map(|index| {
            VirtualizedListItemDescriptor::new(format!("row-{index}"), format!("Row {index}"))
        }));
        let snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("beta"),
                    ui_px(44.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("removed"),
                    ui_px(96.0),
                ),
            ],
        );
        let behavior = VirtualizedList::new("measured-list", "Measured list", items)
            .row_height(ui_px(20.0))
            .overscan(2)
            .row_measure_mode(VirtualizedListRowMeasureMode::Measured)
            .virtualizer_snapshot(snapshot)
            .behavior_snapshot_with_viewport(ui_px(0.0), ui_px(48.0));

        assert_eq!(
            behavior.row_measure_mode(),
            VirtualizedListRowMeasureMode::Measured
        );
        assert_eq!(behavior.state().item_count(), 100);
        assert!(behavior.rendered_row_count() < behavior.state().item_count());
        assert_eq!(behavior.rows()[0].key(), "beta");
        assert_eq!(behavior.rows()[0].virtual_size(), ui_px(44.0));
        assert!(behavior.rows()[0].measured());
        assert_eq!(
            behavior
                .virtualizer_snapshot()
                .measurements()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["beta"]
        );
    }

    #[test]
    fn virtualized_list_measured_mode_prefers_runtime_measurements_over_snapshot() {
        let items = [
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
            VirtualizedListItemDescriptor::new("beta", "Beta"),
            VirtualizedListItemDescriptor::new("gamma", "Gamma"),
        ];
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            items.iter().map(VirtualizedListStateItem::from),
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(3),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Medium).with_row_height(ui_px(20.0)));
        let snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("alpha"),
                    ui_px(20.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("beta"),
                    ui_px(44.0),
                ),
            ],
        );
        let mut measurements = BTreeMap::new();
        measurements.insert("beta".to_owned(), ui_px(72.0));
        let plan = VirtualizedListRenderPlan::resolve(
            "measured-list",
            "Measured list",
            state,
            &items,
            VirtualizedListRowMeasureMode::Measured,
            &measurements,
            Some(&snapshot),
            UiPx::ZERO,
            ui_px(120.0),
        );

        let beta = plan
            .rows()
            .iter()
            .find(|row| row.key() == "beta")
            .expect("beta row should be visible");
        assert_eq!(beta.virtual_start(), ui_px(20.0));
        assert_eq!(beta.virtual_size(), ui_px(72.0));
        assert_eq!(
            plan.virtualizer()
                .snapshot()
                .measurements()
                .iter()
                .find(|item| item.key().as_str() == "beta")
                .map(|item| item.size()),
            Some(ui_px(72.0))
        );
    }

    #[test]
    fn virtualized_list_adapter_invalidates_cached_geometry_on_measurement_revision() {
        struct CountingMeasurements {
            values: BTreeMap<String, UiPx>,
            calls: Cell<usize>,
        }

        impl super::render_plan::VirtualizedListMeasurementLookup for CountingMeasurements {
            fn row_measurement(&self, render_key: &str) -> Option<UiPx> {
                self.calls.set(self.calls.get() + 1);
                self.values.get(render_key).copied()
            }
        }

        let items = [
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
            VirtualizedListItemDescriptor::new("beta", "Beta"),
            VirtualizedListItemDescriptor::new("gamma", "Gamma"),
        ];
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            items.iter().map(VirtualizedListStateItem::from),
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(2),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Medium).with_row_height(ui_px(20.0)));
        let mut measurements = CountingMeasurements {
            values: BTreeMap::from([("beta".to_owned(), ui_px(44.0))]),
            calls: Cell::new(0),
        };
        let mut cache = VirtualizerGeometryCache::default();

        let first = VirtualizedListRenderPlan::resolve_cached(
            "cached-list",
            "Cached list",
            state.clone(),
            &items,
            VirtualizedListRowMeasureMode::Measured,
            &measurements,
            None,
            UiPx::ZERO,
            ui_px(40.0),
            &mut cache,
            1,
        );
        assert_eq!(
            first.virtualizer().item_geometry(1).unwrap().size(),
            ui_px(44.0)
        );
        assert_eq!(measurements.calls.get(), items.len());

        measurements.calls.set(0);
        let scrolled = VirtualizedListRenderPlan::resolve_cached(
            "cached-list",
            "Cached list",
            state.clone(),
            &items,
            VirtualizedListRowMeasureMode::Measured,
            &measurements,
            None,
            ui_px(40.0),
            ui_px(40.0),
            &mut cache,
            1,
        );
        assert_ne!(
            first.virtualizer().visible_range(),
            scrolled.virtualizer().visible_range()
        );
        assert_eq!(measurements.calls.get(), 0);

        measurements.values.insert("beta".to_owned(), ui_px(72.0));
        measurements.calls.set(0);
        let invalidated = VirtualizedListRenderPlan::resolve_cached(
            "cached-list",
            "Cached list",
            state,
            &items,
            VirtualizedListRowMeasureMode::Measured,
            &measurements,
            None,
            UiPx::ZERO,
            ui_px(40.0),
            &mut cache,
            2,
        );
        assert_eq!(
            invalidated.virtualizer().item_geometry(1).unwrap().size(),
            ui_px(72.0)
        );
        assert_eq!(measurements.calls.get(), items.len());
    }

    #[test]
    fn virtualized_list_measured_scroll_target_uses_snapshot_sizes() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(2),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Medium).with_row_height(ui_px(20.0)));
        let exact_snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("alpha"),
                    ui_px(10.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("beta"),
                    ui_px(50.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("gamma"),
                    ui_px(30.0),
                ),
            ],
        );

        assert_eq!(
            state.scroll_target_for_key_with_snapshot(
                "beta",
                VirtualizedListScrollStrategy::Top,
                ui_px(30.0),
                UiPx::ZERO,
                &exact_snapshot,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(10.0),
                false,
            ))
        );
        assert_eq!(
            state.scroll_target_for_key_with_snapshot(
                "beta",
                VirtualizedListScrollStrategy::Center,
                ui_px(30.0),
                UiPx::ZERO,
                &exact_snapshot,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(20.0),
                false,
            ))
        );
        let estimated_snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [open_gpui_ui_core::VirtualizerSnapshotItem::new(
                VirtualizerItemKey::new("alpha"),
                ui_px(10.0),
            )],
        );
        assert_eq!(
            state.scroll_target_for_key_with_snapshot(
                "beta",
                VirtualizedListScrollStrategy::Top,
                ui_px(30.0),
                UiPx::ZERO,
                &estimated_snapshot,
            ),
            VirtualizedListRevealResult::Estimated(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(10.0),
                true,
            ))
        );
    }

    #[test]
    fn virtualized_list_prepend_reveal_preserves_key_identity_with_measured_rows() {
        let prepended_state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("prepend-loading", "Loading previous")
                    .row_kind(VirtualizedListRowKind::Loading)
                    .disabled(true),
                VirtualizedListStateItem::new("new-alpha", "New alpha"),
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("gamma"),
            ["gamma"],
            VirtualizedListSelectionMode::Single,
            Some(3),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Medium).with_row_height(ui_px(20.0)));
        let snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("prepend-loading"),
                    ui_px(16.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("new-alpha"),
                    ui_px(24.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("alpha"),
                    ui_px(10.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("beta"),
                    ui_px(50.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("gamma"),
                    ui_px(30.0),
                ),
            ],
        );

        assert_eq!(prepended_state.active_key(), Some("gamma"));
        assert_eq!(prepended_state.selected_keys(), ["gamma"]);
        assert_eq!(
            prepended_state.scroll_target_for_key_with_snapshot(
                "gamma",
                VirtualizedListScrollStrategy::Top,
                ui_px(30.0),
                UiPx::ZERO,
                &snapshot,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "gamma",
                4,
                ui_px(100.0),
                false,
            ))
        );
    }

    #[test]
    fn virtualized_list_colors_resolve_from_theme_tokens() {
        use open_gpui_ui_core::semantic;

        let colors = VirtualizedListColors::from_tokens(open_gpui_ui_core::ThemeTokens::default());

        assert_eq!(colors.surface().token(), semantic::SURFACE);
        assert_eq!(colors.foreground().token(), semantic::TEXT);
        assert_eq!(
            colors.row_selected_background().token(),
            semantic::SURFACE_MUTED
        );
        assert_eq!(colors.active_indicator_moving().token(), semantic::ACCENT);
        assert_eq!(colors.focus_ring().token(), semantic::FOCUS_RING);
        assert!(!colors.focus_ring_shape().changes_layout());
    }

    #[test]
    fn virtualized_list_row_measure_mode_labels_are_stable() {
        assert_eq!(VirtualizedListRowMeasureMode::Fixed.as_str(), "fixed");
        assert_eq!(VirtualizedListRowMeasureMode::Measured.as_str(), "measured");
        assert!(!VirtualizedListRowMeasureMode::Fixed.measured());
        assert!(VirtualizedListRowMeasureMode::Measured.measured());
    }

    #[test]
    fn virtualized_list_row_context_carries_custom_renderer_invariants() {
        let items = [
            VirtualizedListItemDescriptor::section("recent", "Recent"),
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
            VirtualizedListItemDescriptor::new("beta", "Beta").disabled_reason("Offline"),
            VirtualizedListItemDescriptor::empty("empty", "No results"),
        ];
        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            items.iter().map(VirtualizedListStateItem::from),
            Some("alpha"),
            ["alpha"],
            VirtualizedListSelectionMode::Single,
            Some(4),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Small).with_row_height(ui_px(28.0)));
        let plan = VirtualizedListRenderPlan::resolve(
            "custom-list",
            "Custom list",
            state,
            &items,
            VirtualizedListRowMeasureMode::Fixed,
            &BTreeMap::new(),
            None,
            UiPx::ZERO,
            ui_px(112.0),
        );
        let contexts = plan.row_contexts();

        assert_eq!(contexts.len(), plan.rows().len());
        assert_eq!(contexts[0].key(), "recent");
        assert_eq!(contexts[0].kind(), VirtualizedListRowKind::Section);
        assert_eq!(contexts[0].role(), Role::Group);
        assert_eq!(contexts[0].position_in_set(), None);
        assert_eq!(
            contexts[0].row_measure_mode(),
            VirtualizedListRowMeasureMode::Fixed
        );
        assert_eq!(contexts[1].key(), "alpha");
        assert!(contexts[1].active());
        assert!(contexts[1].selected());
        assert_eq!(contexts[1].position_in_set(), Some(1));
        assert_eq!(contexts[1].size_of_set(), 1);
        assert_eq!(contexts[1].virtual_start(), ui_px(28.0));
        assert_eq!(contexts[1].virtual_size(), ui_px(28.0));
        assert_eq!(contexts[2].disabled_reason(), Some("Offline"));
        assert_eq!(contexts[3].kind(), VirtualizedListRowKind::Empty);
        assert!(!contexts[3].selectable());
    }

    #[test]
    fn virtualized_list_active_indicator_retargets_visible_rows_and_requests_frames() {
        let start = Instant::now();
        let model = MotionPreset::affordance(MotionPreference::Animated).resolve_model();
        let mut indicator = VirtualizedListActiveIndicatorRuntime::default();
        let first = indicator_plan("indicator-row-00", UiPx::ZERO);

        let first_update = indicator.sync(&first, start, model);
        assert_eq!(first_update.frame_demand(), MotionFrameDemand::Idle);
        assert_eq!(
            first_update.reset_reason(),
            Some(MotionFrameResetReason::MotionIdentityChanged)
        );
        let first_snapshot = indicator.snapshot().expect("visible indicator");
        assert_eq!(
            indicator.state.as_ref().map(|state| state.key.as_str()),
            Some("indicator-row-00")
        );
        assert_eq!(first_snapshot.top(), ui_px(0.0));
        assert_eq!(first_snapshot.height(), ui_px(20.0));
        assert_eq!(first_snapshot.frame_demand(), MotionFrameDemand::Idle);

        let second = indicator_plan("indicator-row-02", UiPx::ZERO);
        let demand = indicator.sync(&second, start + Duration::from_millis(16), model);
        assert!(demand.frame_demand().needs_frame());
        assert_eq!(
            demand.reset_reason(),
            Some(MotionFrameResetReason::MotionIdentityChanged)
        );
        let moving = indicator.snapshot().expect("moving indicator");
        assert_eq!(
            indicator.state.as_ref().map(|state| state.key.as_str()),
            Some("indicator-row-02")
        );
        assert!(moving.frame_demand().needs_frame());
        assert!(moving.top().as_f32() < ui_px(40.0).as_f32());

        let final_demand = indicator.sync(&second, start + Duration::from_secs(2), model);
        assert_eq!(final_demand.frame_demand(), MotionFrameDemand::Idle);
        assert_eq!(
            final_demand.reset_reason(),
            Some(MotionFrameResetReason::Finish)
        );
        let final_snapshot = indicator.snapshot().expect("settled indicator");
        assert_eq!(final_snapshot.top(), ui_px(40.0));
        assert_eq!(final_snapshot.height(), ui_px(20.0));
    }

    #[test]
    fn virtualized_list_active_indicator_reduced_motion_publishes_final_bounds() {
        let start = Instant::now();
        let animated = MotionPreset::affordance(MotionPreference::Animated).resolve_model();
        let reduced = MotionPreset::affordance(MotionPreference::Reduced).resolve_model();
        let mut indicator = VirtualizedListActiveIndicatorRuntime::default();
        let first = indicator_plan("indicator-row-00", UiPx::ZERO);
        let second = indicator_plan("indicator-row-02", UiPx::ZERO);

        let first_update = indicator.sync(&first, start, animated);
        assert_eq!(first_update.frame_demand(), MotionFrameDemand::Idle);
        assert_eq!(
            first_update.reset_reason(),
            Some(MotionFrameResetReason::MotionIdentityChanged)
        );
        let demand = indicator.sync(&second, start + Duration::from_millis(16), reduced);

        assert_eq!(demand.frame_demand(), MotionFrameDemand::Idle);
        assert_eq!(
            demand.reset_reason(),
            Some(MotionFrameResetReason::MotionIdentityChanged)
        );
        let snapshot = indicator.snapshot().expect("reduced indicator");
        assert_eq!(
            indicator.state.as_ref().map(|state| state.key.as_str()),
            Some("indicator-row-02")
        );
        assert_eq!(snapshot.top(), ui_px(40.0));
        assert_eq!(snapshot.height(), ui_px(20.0));
        assert_eq!(snapshot.frame_demand(), MotionFrameDemand::Idle);
    }

    #[test]
    fn virtualized_list_active_indicator_hides_when_active_row_is_offscreen() {
        let start = Instant::now();
        let model = MotionPreset::affordance(MotionPreference::Animated).resolve_model();
        let mut indicator = VirtualizedListActiveIndicatorRuntime::default();
        let visible = indicator_plan("indicator-row-00", UiPx::ZERO);
        let offscreen = indicator_plan("indicator-row-00", ui_px(140.0));

        let visible_update = indicator.sync(&visible, start, model);
        assert_eq!(visible_update.frame_demand(), MotionFrameDemand::Idle);
        assert!(indicator.snapshot().is_some());

        let demand = indicator.sync(&offscreen, start + Duration::from_millis(16), model);
        assert_eq!(demand.frame_demand(), MotionFrameDemand::Idle);
        assert_eq!(demand.reset_reason(), Some(MotionFrameResetReason::Cancel));
        assert!(indicator.snapshot().is_none());
        assert!(indicator.state.is_none());
    }

    #[test]
    fn virtualized_list_scroll_target_applies_alignment_strategies() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            (0..100).map(|index| {
                VirtualizedListStateItem::new(format!("row-{index}"), format!("Row {index}"))
            }),
            Some("row-10"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(3),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Medium).with_row_height(ui_px(32.0)));
        let viewport_extent = ui_px(96.0);
        let current = ui_px(320.0);

        assert_eq!(
            state.scroll_target_for_key(
                "row-10",
                VirtualizedListScrollStrategy::Top,
                viewport_extent,
                current,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "row-10",
                10,
                ui_px(320.0),
                false,
            ))
        );
        assert_eq!(
            state.scroll_target_for_key(
                "row-10",
                VirtualizedListScrollStrategy::Center,
                viewport_extent,
                current,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "row-10",
                10,
                ui_px(288.0),
                false,
            ))
        );
        assert_eq!(
            state.scroll_target_for_key(
                "row-10",
                VirtualizedListScrollStrategy::Bottom,
                viewport_extent,
                current,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "row-10",
                10,
                ui_px(256.0),
                false,
            ))
        );
        assert_eq!(
            state.scroll_target_for_key(
                "row-10",
                VirtualizedListScrollStrategy::Nearest,
                viewport_extent,
                current,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "row-10",
                10,
                ui_px(320.0),
                false,
            ))
        );
        assert_eq!(
            state.scroll_target_for_key(
                "row-10",
                VirtualizedListScrollStrategy::Nearest,
                viewport_extent,
                ui_px(0.0),
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "row-10",
                10,
                ui_px(256.0),
                false,
            ))
        );
    }
}
