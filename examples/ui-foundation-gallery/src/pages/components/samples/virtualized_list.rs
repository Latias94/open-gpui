use super::*;

/// One virtualized-list state-contract sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListStateContractSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Short explanation of the contract slice.
    pub summary: &'static str,
    /// Resolved renderer-neutral virtualized-list state.
    pub state: VirtualizedListState,
    /// Semantic scroll alignment the rendered adapter can apply when revealing the active row.
    pub scroll_strategy: VirtualizedListScrollStrategy,
}

impl VirtualizedListStateContractSample {
    /// Returns the stable debug selector used by the state-contract gallery section.
    pub fn debug_selector(&self) -> String {
        format!(
            "gallery:component-virtualized-list-state-contract:{}",
            self.id
        )
    }
}

/// One virtualized list sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Shared item descriptors consumed by the concrete list renderer.
    pub items: Arc<[VirtualizedListItemDescriptor]>,
    /// Resolved renderer-neutral list state.
    pub state: VirtualizedListState,
    /// Visual size applied to the concrete list.
    pub size: Size,
    /// Fixed list viewport used by the sample summary.
    pub viewport_extent: UiPx,
    /// Fixed row height used by the virtualizer.
    pub row_height: UiPx,
    /// Overscan row budget.
    pub overscan: usize,
    /// Row height ownership mode.
    pub row_measure_mode: VirtualizedListRowMeasureMode,
    /// Optional measured virtualizer restore payload.
    pub snapshot: Option<VirtualizerSnapshot>,
    /// Concrete content renderer variant used by this sample.
    pub renderer: VirtualizedListSampleRenderer,
    /// Precomputed state summary used by the gallery page.
    state_summary: VirtualizedListSampleStateSummary,
}

/// Concrete row-content renderer variant used by gallery samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualizedListSampleRenderer {
    /// Uses the default component content renderer.
    Default,
    /// Uses a custom compact metadata renderer.
    CompactMetadata,
}

/// Precomputed state summary for a virtualized list sample.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VirtualizedListSampleStateSummary {
    /// Total source item count.
    pub item_count: usize,
    /// Rendered row count after overscan.
    pub rendered_rows: usize,
    /// Visible row count before overscan.
    pub visible_rows: usize,
    /// Visible row range start.
    pub visible_start: usize,
    /// Visible row range end.
    pub visible_end: usize,
    /// Overscan row range start.
    pub overscan_start: usize,
    /// Overscan row range end.
    pub overscan_end: usize,
    /// Active row index.
    pub active_index: Option<usize>,
    /// Selected row index.
    pub selected_index: Option<usize>,
    /// Active row key.
    pub active_key: Option<String>,
    /// Selected row keys.
    pub selected_keys: Vec<String>,
}

impl VirtualizedListSampleStateSummary {
    fn from_snapshot(snapshot: &VirtualizedListBehaviorSnapshot) -> Self {
        let visible = snapshot.visible_range();
        let overscan = snapshot.overscan_range();

        Self {
            item_count: snapshot.state().item_count(),
            rendered_rows: snapshot.rendered_row_count(),
            visible_rows: snapshot.visible_row_count(),
            visible_start: visible.start(),
            visible_end: visible.end(),
            overscan_start: overscan.start(),
            overscan_end: overscan.end(),
            active_index: snapshot.state().active_index(),
            selected_index: snapshot.state().selected_index(),
            active_key: snapshot.state().active_key().map(str::to_owned),
            selected_keys: snapshot
                .state()
                .selected_keys()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

impl VirtualizedListSample {
    /// Builds the concrete GPUI virtualized list for this sample.
    pub fn build_list(&self) -> VirtualizedList {
        let mut list = VirtualizedList::from_shared_items(
            format!("component-virtualized-list:{}", self.id),
            self.title,
            self.items.clone(),
        )
        .with_size(self.size)
        .row_height(self.row_height)
        .row_measure_mode(self.row_measure_mode)
        .overscan(self.overscan)
        .viewport_item_count(self.state.viewport_item_count())
        .disabled(self.state.disabled());

        if let Some(snapshot) = self.snapshot.clone() {
            list = list.virtualizer_snapshot(snapshot);
        }
        if let Some(active_key) = self.state.active_key() {
            list = list.default_active_key(active_key);
        }
        if !self.state.selected_key_set().is_empty() {
            list = list.default_selected_keys(self.state.selected_keys());
        }
        list = list.selection_mode(self.state.selection_mode());

        if self.renderer == VirtualizedListSampleRenderer::CompactMetadata {
            list = list.render_row(|context, _, _| render_compact_virtualized_list_row(context));
        }

        list
    }

    /// Resolves the sample's behavior snapshot at the viewport origin.
    pub fn behavior_snapshot(&self) -> VirtualizedListBehaviorSnapshot {
        self.build_list()
            .behavior_snapshot_with_viewport(UiPx::ZERO, self.viewport_extent)
    }

    /// Returns the precomputed state summary.
    pub fn state_summary(&self) -> VirtualizedListSampleStateSummary {
        self.state_summary.clone()
    }
}

/// Returns virtualized-list state-contract samples for renderer follow-up review.
pub fn virtualized_list_state_contract_samples() -> [VirtualizedListStateContractSample; 1] {
    [VirtualizedListStateContractSample {
        id: "release-navigation",
        title: "Release navigation",
        summary: "Long-list active descendant navigation without duplicating virtualizer range math.",
        state: VirtualizedListState::resolve(
            Size::Small,
            false,
            (0..10_000).map(|index| release_navigation_item(index).state_item()),
            Some("release-nav-0042"),
            ["release-nav-0040"],
            VirtualizedListSelectionMode::Single,
            Some(12),
        ),
        scroll_strategy: VirtualizedListScrollStrategy::Center,
    }]
}

static VIRTUALIZED_LIST_SAMPLES: LazyLock<Vec<VirtualizedListSample>> =
    LazyLock::new(build_virtualized_list_samples);

/// Returns virtualized-list samples backed by the concrete renderer and virtualizer contract.
pub fn virtualized_list_samples(_tokens: ThemeTokens) -> &'static [VirtualizedListSample] {
    VIRTUALIZED_LIST_SAMPLES.as_slice()
}

fn build_virtualized_list_samples() -> Vec<VirtualizedListSample> {
    vec![
        release_navigation_sample(),
        primary_options_sample(),
        section_status_sample(),
        custom_renderer_sample(),
        measured_notes_sample(),
    ]
    .into_iter()
    .map(VirtualizedListSample::with_state_summary)
    .collect()
}

impl VirtualizedListSample {
    fn with_state_summary(self) -> Self {
        let snapshot = self.behavior_snapshot();
        Self {
            state_summary: VirtualizedListSampleStateSummary::from_snapshot(&snapshot),
            ..self
        }
    }
}

fn release_navigation_sample() -> VirtualizedListSample {
    let size = Size::Small;
    let row_height = ui_px(28.0);
    let overscan = 4;
    let item_count = 10_000;
    let items: Arc<[VirtualizedListItemDescriptor]> = Arc::from(
        (0..item_count)
            .map(release_navigation_item)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let state = VirtualizedListState::resolve(
        size,
        false,
        items.iter().map(|item| item.state_item()),
        Some("release-nav-0000"),
        ["release-nav-0000"],
        VirtualizedListSelectionMode::Single,
        Some(8),
    )
    .with_metrics(
        VirtualizedListMetrics::from_size(size)
            .with_row_height(row_height)
            .with_overscan_count(overscan),
    );

    VirtualizedListSample {
        id: "release-navigation",
        title: "Release navigation",
        summary: "Ten thousand stable options with a local virtualized viewport and keyboard reveal.",
        badge: "10k items",
        items,
        state,
        size,
        viewport_extent: ui_px(224.0),
        row_height,
        overscan,
        row_measure_mode: VirtualizedListRowMeasureMode::Fixed,
        snapshot: None,
        renderer: VirtualizedListSampleRenderer::Default,
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
}

fn primary_options_sample() -> VirtualizedListSample {
    let size = Size::Small;
    let row_height = ui_px(30.0);
    let overscan = 3;
    let items = shared_virtualized_items(
        (0..96)
            .map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("quick-option-{index:02}"),
                    format!("Option {index:02}"),
                )
            })
            .collect(),
    );
    let state = VirtualizedListState::resolve(
        size,
        false,
        items.iter().map(|item| item.state_item()),
        Some("quick-option-00"),
        ["quick-option-00"],
        VirtualizedListSelectionMode::Single,
        Some(6),
    )
    .with_metrics(
        VirtualizedListMetrics::from_size(size)
            .with_row_height(row_height)
            .with_overscan_count(overscan),
    );

    VirtualizedListSample {
        id: "primary-options",
        title: "Primary options",
        summary: "A compact fixed-height list that only supplies primary labels.",
        badge: "plain",
        items,
        state,
        size,
        viewport_extent: ui_px(180.0),
        row_height,
        overscan,
        row_measure_mode: VirtualizedListRowMeasureMode::Fixed,
        snapshot: None,
        renderer: VirtualizedListSampleRenderer::Default,
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
}

fn section_status_sample() -> VirtualizedListSample {
    let size = Size::Small;
    let row_height = ui_px(32.0);
    let overscan = 2;
    let items = shared_virtualized_items(vec![
        VirtualizedListItemDescriptor::section("deploy-section", "Deployment queue"),
        VirtualizedListItemDescriptor::new("deploy-ready", "Ready to ship")
            .secondary_text("Production release has a green verification run")
            .badge("ready")
            .status("queued"),
        VirtualizedListItemDescriptor::new("deploy-review", "Needs design review")
            .secondary_text("Motion polish must be signed off before release")
            .badge("review")
            .status("blocked")
            .disabled_reason("Waiting for owner"),
        VirtualizedListItemDescriptor::separator("deploy-divider"),
        VirtualizedListItemDescriptor::loading("deploy-loading", "Loading archived deploys"),
        VirtualizedListItemDescriptor::empty("deploy-empty", "No archived deploys"),
        VirtualizedListItemDescriptor::error("deploy-error", "Archive provider unavailable"),
    ]);
    let state = VirtualizedListState::resolve(
        size,
        false,
        items.iter().map(|item| item.state_item()),
        Some("deploy-ready"),
        ["deploy-ready"],
        VirtualizedListSelectionMode::Single,
        Some(7),
    )
    .with_metrics(
        VirtualizedListMetrics::from_size(size)
            .with_row_height(row_height)
            .with_overscan_count(overscan),
    );

    VirtualizedListSample {
        id: "section-status",
        title: "Section and status rows",
        summary: "Non-selectable section, separator, loading, empty, and error rows in one list.",
        badge: "mixed",
        items,
        state,
        size,
        viewport_extent: ui_px(224.0),
        row_height,
        overscan,
        row_measure_mode: VirtualizedListRowMeasureMode::Fixed,
        snapshot: None,
        renderer: VirtualizedListSampleRenderer::Default,
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
}

fn custom_renderer_sample() -> VirtualizedListSample {
    let size = Size::Small;
    let row_height = ui_px(34.0);
    let overscan = 3;
    let items = shared_virtualized_items(
        (0..64)
            .map(|index| {
                let status = if index % 7 == 0 { "late" } else { "open" };
                VirtualizedListItemDescriptor::new(
                    format!("custom-row-{index:02}"),
                    format!("Task {index:02}"),
                )
                .secondary_text(format!("Owner lane {}", index % 5))
                .leading_metadata(format!("P{}", (index % 3) + 1))
                .trailing_metadata(format!("{}m", 10 + index))
                .badge(status)
                .status(if status == "late" {
                    "needs triage"
                } else {
                    "healthy"
                })
            })
            .collect(),
    );
    let state = VirtualizedListState::resolve(
        size,
        false,
        items.iter().map(|item| item.state_item()),
        Some("custom-row-03"),
        ["custom-row-03"],
        VirtualizedListSelectionMode::Single,
        Some(6),
    )
    .with_metrics(
        VirtualizedListMetrics::from_size(size)
            .with_row_height(row_height)
            .with_overscan_count(overscan),
    );

    VirtualizedListSample {
        id: "custom-renderer",
        title: "Custom renderer",
        summary: "A renderer hook replaces row content while the list owns layout and ARIA.",
        badge: "hook",
        items,
        state,
        size,
        viewport_extent: ui_px(204.0),
        row_height,
        overscan,
        row_measure_mode: VirtualizedListRowMeasureMode::Fixed,
        snapshot: None,
        renderer: VirtualizedListSampleRenderer::CompactMetadata,
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
}

fn measured_notes_sample() -> VirtualizedListSample {
    let size = Size::Small;
    let row_height = ui_px(34.0);
    let overscan = 3;
    let items = shared_virtualized_items(vec![
        VirtualizedListItemDescriptor::section("notes-section", "Release notes"),
        VirtualizedListItemDescriptor::new("note-001", "Motion contract")
            .secondary_text("Controller demand, terminal lifecycle, and frame clock behavior.")
            .badge("motion")
            .status("ready"),
        VirtualizedListItemDescriptor::new("note-002", "Virtual list measurement")
            .secondary_text("Rows can report measured content height and restore snapshots.")
            .badge("virtual")
            .status("ready"),
        VirtualizedListItemDescriptor::new("note-003", "Custom renderer safety")
            .secondary_text("Renderer owns content only; outer rows retain role, focus, hit testing, and measured geometry.")
            .badge("render")
            .status("review"),
        VirtualizedListItemDescriptor::new("note-004", "Gallery coverage")
            .secondary_text("Samples cover plain, rich, mixed semantic rows, custom rendering, and measured restore.")
            .badge("docs")
            .status("queued"),
        VirtualizedListItemDescriptor::new("note-005", "Release checklist")
            .secondary_text("Public surface tests and gallery smoke tests gate the v0.2.0 API.")
            .badge("ship")
            .status("ready"),
    ]);
    let snapshot = VirtualizerSnapshot::new(
        ui_px(18.0),
        [
            VirtualizerSnapshotItem::new(VirtualizerItemKey::from("note-001"), ui_px(58.0)),
            VirtualizerSnapshotItem::new(VirtualizerItemKey::from("note-002"), ui_px(62.0)),
            VirtualizerSnapshotItem::new(VirtualizerItemKey::from("note-003"), ui_px(72.0)),
            VirtualizerSnapshotItem::new(VirtualizerItemKey::from("note-004"), ui_px(64.0)),
        ],
    );
    let state = VirtualizedListState::resolve(
        size,
        false,
        items.iter().map(|item| item.state_item()),
        Some("note-002"),
        ["note-002"],
        VirtualizedListSelectionMode::Single,
        Some(5),
    )
    .with_metrics(
        VirtualizedListMetrics::from_size(size)
            .with_row_height(row_height)
            .with_overscan_count(overscan),
    );

    VirtualizedListSample {
        id: "measured-notes",
        title: "Measured notes",
        summary: "Variable-height content restored from virtualizer measurements.",
        badge: "measured",
        items,
        state,
        size,
        viewport_extent: ui_px(210.0),
        row_height,
        overscan,
        row_measure_mode: VirtualizedListRowMeasureMode::Measured,
        snapshot: Some(snapshot),
        renderer: VirtualizedListSampleRenderer::Default,
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
}

fn shared_virtualized_items(
    items: Vec<VirtualizedListItemDescriptor>,
) -> Arc<[VirtualizedListItemDescriptor]> {
    Arc::from(items.into_boxed_slice())
}

fn render_compact_virtualized_list_row(
    context: VirtualizedListRowRenderContext,
) -> impl open_gpui::IntoElement {
    if context.kind().as_str() == "separator" {
        return div()
            .mx(open_gpui::px(8.0))
            .h(open_gpui::px(1.0))
            .w_full()
            .bg(rgb(0xd9ded6));
    }

    let label = context.label().to_owned();
    let secondary = context.secondary_text().map(str::to_owned).or_else(|| {
        context
            .disabled_reason()
            .map(|reason| format!("Disabled: {reason}"))
    });
    let meta = context
        .trailing_metadata()
        .or_else(|| context.status())
        .unwrap_or(context.kind().as_str())
        .to_owned();
    let badge = context.badge().map(str::to_owned);
    let state_label = if context.active() {
        "active"
    } else if context.selected() {
        "selected"
    } else if context.disabled() {
        "disabled"
    } else {
        context.kind().as_str()
    };
    let marker_color = if context.active() {
        rgb(0x176f5d)
    } else if context.disabled() {
        rgb(0xb7bdc5)
    } else {
        rgb(0x5b6ee1)
    };

    div()
        .w_full()
        .min_w(open_gpui::px(0.0))
        .px(open_gpui::px(10.0))
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_none()
                .w(open_gpui::px(7.0))
                .h(open_gpui::px(7.0))
                .rounded_full()
                .bg(marker_color),
        )
        .child(
            div()
                .flex_1()
                .min_w(open_gpui::px(0.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::MEDIUM)
                        .child(label),
                )
                .when_some(secondary, |this, secondary| {
                    this.child(div().text_xs().text_color(rgb(0x667085)).child(secondary))
                }),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(meta),
        )
        .when_some(badge, |this, badge| {
            this.child(
                div()
                    .rounded(open_gpui::px(4.0))
                    .bg(rgb(0xe9eefc))
                    .px_1()
                    .text_xs()
                    .text_color(rgb(0x344054))
                    .child(badge),
            )
        })
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x667085))
                .child(state_label.to_owned()),
        )
}

fn release_navigation_item(index: usize) -> VirtualizedListItemDescriptor {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let team = teams[index % teams.len()];
    let status = statuses[(index / 11) % statuses.len()];

    VirtualizedListItemDescriptor::new(
        format!("release-nav-{index:04}"),
        format!("Release #{index:04}"),
    )
    .secondary_text(format!("{team} lane / {status}"))
    .with_text_value(format!("release {index:04} {team} {status}"))
    .leading_metadata(team)
    .trailing_metadata(format!("batch {}", index / 100))
    .badge(status)
    .status(if status == "Blocked" {
        "Needs owner"
    } else {
        "On track"
    })
}
