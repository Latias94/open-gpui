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
    /// Precomputed state summary used by the gallery page.
    state_summary: VirtualizedListSampleStateSummary,
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
        .overscan(self.overscan)
        .viewport_item_count(self.state.viewport_item_count())
        .disabled(self.state.disabled());

        if let Some(active_key) = self.state.active_key() {
            list = list.default_active_key(active_key);
        }
        if !self.state.selected_key_set().is_empty() {
            list = list.default_selected_keys(self.state.selected_keys());
        }
        list = list.selection_mode(self.state.selection_mode());

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

static VIRTUALIZED_LIST_SAMPLES: LazyLock<[VirtualizedListSample; 1]> =
    LazyLock::new(build_virtualized_list_samples);

/// Returns virtualized-list samples backed by the concrete renderer and virtualizer contract.
pub fn virtualized_list_samples(_tokens: ThemeTokens) -> &'static [VirtualizedListSample] {
    VIRTUALIZED_LIST_SAMPLES.as_slice()
}

fn build_virtualized_list_samples() -> [VirtualizedListSample; 1] {
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

    [VirtualizedListSample {
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
        state_summary: VirtualizedListSampleStateSummary::default(),
    }
    .with_state_summary()]
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

fn release_navigation_item(index: usize) -> VirtualizedListItemDescriptor {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];

    VirtualizedListItemDescriptor::new(
        format!("release-nav-{index:04}"),
        format!(
            "Release #{index:04} / {} / {}",
            teams[index % teams.len()],
            statuses[(index / 11) % statuses.len()]
        ),
    )
}
