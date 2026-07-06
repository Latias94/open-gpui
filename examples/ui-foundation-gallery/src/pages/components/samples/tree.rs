use super::*;

/// One tree state-contract sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeStateContractSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Short explanation of the contract slice.
    pub summary: &'static str,
    /// Resolved renderer-neutral tree state.
    pub state: TreeState,
}

impl TreeStateContractSample {
    /// Returns the stable debug selector used by the state-contract gallery section.
    pub fn debug_selector(&self) -> String {
        format!("gallery:component-tree-state-contract:{}", self.id)
    }
}

/// One rendered tree sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Root item descriptors consumed by the concrete tree renderer.
    pub items: Vec<TreeItemDescriptor>,
    /// Resolved renderer-neutral tree state.
    pub state: TreeState,
    /// Visual size applied to the concrete tree.
    pub size: Size,
    /// Whether the concrete tree uses fixed-row virtualized rendering.
    pub virtualized: bool,
    /// Whether the concrete Tree enables pointer drag move affordances.
    pub draggable: bool,
    /// Fallback virtualized viewport item count before layout measurement.
    pub viewport_item_count: usize,
    /// Virtualized overscan item budget.
    pub overscan_count: usize,
}

impl TreeSample {
    /// Returns the current controlled item descriptors for this sample.
    pub fn current_items(&self, cx: &impl AppContext) -> Vec<TreeItemDescriptor> {
        current_tree_sample_items(self.id, &self.items, cx)
    }

    /// Returns the current controlled tree state for this sample.
    pub fn current_state(&self, cx: &impl AppContext) -> TreeState {
        let items = self.current_items(cx);
        TreeState::resolve(
            self.state.size(),
            self.state.label(),
            self.state.selected_value(),
            self.state.focused_value(),
            items,
        )
    }

    /// Builds the concrete GPUI tree for this sample.
    pub fn build_tree(&self) -> Tree {
        let mut tree = Tree::new(
            format!("component-tree:{}", self.id),
            self.title,
            self.items.clone(),
        )
        .with_size(self.size)
        .virtualized(self.virtualized)
        .draggable(self.draggable)
        .viewport_item_count(self.viewport_item_count)
        .overscan_count(self.overscan_count);

        if let Some(selected) = self.state.selected_value() {
            tree = tree.default_selected(selected);
        }
        if let Some(focused) = self.state.focused_value() {
            tree = tree.default_focused(focused);
        }

        tree
    }

    /// Builds the concrete GPUI tree for this sample using current gallery overrides.
    pub fn build_tree_with_runtime(&self, cx: &impl AppContext) -> Tree {
        let mut tree = Tree::new(
            format!("component-tree:{}", self.id),
            self.title,
            self.current_items(cx),
        )
        .with_size(self.size)
        .virtualized(self.virtualized)
        .draggable(self.draggable)
        .viewport_item_count(self.viewport_item_count)
        .overscan_count(self.overscan_count);

        if let Some(selected) = self.state.selected_value() {
            tree = tree.default_selected(selected);
        }
        if let Some(focused) = self.state.focused_value() {
            tree = tree.default_focused(focused);
        }

        tree
    }

    /// Resolves the sample's virtualized behavior snapshot at the viewport origin.
    pub fn behavior_snapshot(&self) -> TreeBehaviorSnapshot {
        self.build_tree().behavior_snapshot(
            UiPx::ZERO,
            self.state.metrics().row_height() * self.viewport_item_count as f32,
        )
    }
}

static TREE_SAMPLES: LazyLock<[TreeSample; 4]> = LazyLock::new(build_tree_samples);

/// Returns tree samples backed by the concrete renderer and hierarchy contract.
pub fn tree_samples(_tokens: ThemeTokens) -> &'static [TreeSample] {
    TREE_SAMPLES.as_slice()
}

fn build_tree_samples() -> [TreeSample; 4] {
    let size = Size::Small;
    let items = document_outline_tree_sample_items();
    let state = TreeState::resolve(
        size,
        "Document outline",
        Some("paper"),
        Some("paper"),
        items.clone(),
    );
    let editable_items = editable_outline_tree_sample_items();
    let editable_state = TreeState::resolve(
        size,
        "Editable outline",
        Some("root"),
        Some("root"),
        editable_items.clone(),
    );

    let remote_items = remote_workspace_tree_sample_items();
    let remote_state = TreeState::resolve(
        size,
        "Remote workspace",
        Some("remote-src"),
        Some("remote-src"),
        remote_items.clone(),
    );

    let release_items = virtualized_release_tree_sample_items();
    let release_state = TreeState::resolve(
        size,
        "Release outline",
        Some("release-node-0000"),
        Some("release-node-0000"),
        release_items.clone(),
    );

    [
        TreeSample {
            id: "document-outline",
            title: "Document outline",
            summary: "Expandable hierarchy with roving focus, selection, and an owned scroll viewport.",
            badge: "tree",
            items,
            state,
            size,
            virtualized: false,
            draggable: false,
            viewport_item_count: 12,
            overscan_count: 4,
        },
        TreeSample {
            id: "remote-workspace",
            title: "Remote workspace",
            summary: "Loadable branches expose unloaded, loading, loaded, and failed child state.",
            badge: "lazy tree",
            items: remote_items,
            state: remote_state,
            size,
            virtualized: false,
            draggable: false,
            viewport_item_count: 12,
            overscan_count: 4,
        },
        TreeSample {
            id: "release-outline",
            title: "Release outline",
            summary: "Large visible hierarchy rendered through the Tree fixed-row virtual window.",
            badge: "virtual tree",
            items: release_items,
            state: release_state,
            size,
            virtualized: true,
            draggable: false,
            viewport_item_count: 8,
            overscan_count: 4,
        },
        TreeSample {
            id: "editable-outline",
            title: "Editable outline",
            summary: "Controlled drag moves update the visible outline in place.",
            badge: "drag tree",
            items: editable_items,
            state: editable_state,
            size,
            virtualized: false,
            draggable: true,
            viewport_item_count: 12,
            overscan_count: 4,
        },
    ]
}

/// Returns tree state-contract samples for renderer-neutral review.
pub fn tree_state_contract_samples() -> [TreeStateContractSample; 1] {
    [TreeStateContractSample {
        id: "document-outline",
        title: "Document outline",
        summary: "Visible flattening, disabled-row skipping, and APG-style keyboard actions.",
        state: TreeState::resolve(
            Size::Medium,
            "Document outline",
            Some("intro"),
            Some("figures"),
            document_outline_tree_items(),
        ),
    }]
}

fn document_outline_tree_sample_items() -> Vec<TreeItemDescriptor> {
    let appendix_items = (1..=12).map(|index| {
        TreeItemDescriptor::new(
            format!("appendix-{index:02}"),
            format!("Appendix section {index:02}"),
        )
    });

    vec![
        TreeItemDescriptor::new("paper", "Paper")
            .child(TreeItemDescriptor::new("intro", "Introduction"))
            .child(
                TreeItemDescriptor::new("figures", "Figures")
                    .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
            ),
        TreeItemDescriptor::new("appendix", "Appendix")
            .expanded(true)
            .children(appendix_items),
        TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
        TreeItemDescriptor::new("notes", "Notes"),
    ]
}

fn remote_workspace_tree_sample_items() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("remote-root", "Remote project")
            .expanded(true)
            .child(TreeItemDescriptor::new("remote-src", "src").with_children_unloaded())
            .child(
                TreeItemDescriptor::new("remote-crates", "crates")
                    .with_children_loading("Loading child packages"),
            )
            .child(
                TreeItemDescriptor::new("remote-build", "build artifacts")
                    .with_children_load_failed("Network unavailable"),
            )
            .child(
                TreeItemDescriptor::new("remote-docs", "docs")
                    .expanded(true)
                    .child(TreeItemDescriptor::new("remote-readme", "README.md")),
            ),
    ]
}

fn virtualized_release_tree_sample_items() -> Vec<TreeItemDescriptor> {
    (0..240)
        .map(|index| {
            TreeItemDescriptor::new(
                format!("release-node-{index:04}"),
                format!("Release node {index:04}"),
            )
        })
        .collect()
}

fn document_outline_tree_items() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("paper", "Paper")
            .expanded(true)
            .child(TreeItemDescriptor::new("intro", "Introduction"))
            .child(
                TreeItemDescriptor::new("figures", "Figures")
                    .expanded(false)
                    .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
            ),
        TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
        TreeItemDescriptor::new("notes", "Notes"),
    ]
}

fn editable_outline_tree_sample_items() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("root", "Root")
            .expanded(true)
            .child(TreeItemDescriptor::new("child", "Child"))
            .child(TreeItemDescriptor::new("peer", "Peer")),
        TreeItemDescriptor::new("sibling", "Sibling"),
    ]
}
