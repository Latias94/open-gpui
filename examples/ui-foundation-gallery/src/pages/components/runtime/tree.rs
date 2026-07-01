use super::*;

/// One Tree drag move captured from the rendered gallery sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSampleMoveEvent {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Controlled Tree move payload.
    pub tree_move: TreeMove,
}

/// One selection captured from the rendered gallery `Tree` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSampleSelection {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Selected item value.
    pub value: String,
}

/// One expansion toggle captured from the rendered gallery `Tree` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSampleToggleEvent {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Toggled item value.
    pub value: String,
    /// Desired expanded state after the toggle.
    pub expanded: bool,
    /// Currently loaded child descriptor count at toggle time.
    pub loaded_child_count: usize,
    /// Stable child loading state label at toggle time.
    pub children_load_state: String,
    /// Loading or failure message at toggle time, when present.
    pub children_load_message: Option<String>,
}

/// Runtime interaction log used by gallery Tree smoke tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TreeSampleRuntimeLog {
    selections: Vec<TreeSampleSelection>,
    toggles: Vec<TreeSampleToggleEvent>,
    moves: Vec<TreeSampleMoveEvent>,
    tree_item_overrides: BTreeMap<String, Vec<TreeItemDescriptor>>,
}

impl Global for TreeSampleRuntimeLog {}

impl TreeSampleRuntimeLog {
    /// Returns captured selections in event order.
    pub fn selections(&self) -> &[TreeSampleSelection] {
        &self.selections
    }

    /// Returns captured toggles in event order.
    pub fn toggles(&self) -> &[TreeSampleToggleEvent] {
        &self.toggles
    }

    /// Returns captured move payloads in event order.
    pub fn moves(&self) -> &[TreeSampleMoveEvent] {
        &self.moves
    }

    /// Returns the current controlled item descriptors for a sample, if any.
    pub fn tree_item_override(&self, sample_id: &str) -> Option<&[TreeItemDescriptor]> {
        self.tree_item_overrides.get(sample_id).map(Vec::as_slice)
    }

    /// Clears captured interactions.
    pub fn clear(&mut self) {
        self.selections.clear();
        self.toggles.clear();
        self.moves.clear();
        self.tree_item_overrides.clear();
    }
}

/// Records a gallery `Tree` selection in app-global sample state.
pub fn record_tree_selection(sample_id: impl Into<String>, value: impl Into<String>, cx: &mut App) {
    cx.update_default_global::<TreeSampleRuntimeLog, _>(|log, _| {
        log.selections.push(TreeSampleSelection {
            sample_id: sample_id.into(),
            value: value.into(),
        });
    });
}

/// Records a gallery `Tree` expansion toggle in app-global sample state.
pub fn record_tree_toggle(
    sample_id: impl Into<String>,
    value: impl Into<String>,
    expanded: bool,
    loaded_child_count: usize,
    children_load_state: impl Into<String>,
    children_load_message: Option<String>,
    cx: &mut App,
) {
    cx.update_default_global::<TreeSampleRuntimeLog, _>(|log, _| {
        log.toggles.push(TreeSampleToggleEvent {
            sample_id: sample_id.into(),
            value: value.into(),
            expanded,
            loaded_child_count,
            children_load_state: children_load_state.into(),
            children_load_message,
        });
    });
}

/// Returns the current controlled item descriptors for a gallery `Tree` sample.
pub fn current_tree_sample_items(
    sample_id: impl Into<String>,
    fallback: &[TreeItemDescriptor],
    cx: &impl AppContext,
) -> Vec<TreeItemDescriptor> {
    let sample_id = sample_id.into();
    cx.read_global::<TreeSampleRuntimeLog, _>(|log, _| {
        log.tree_item_override(&sample_id)
            .map(|items| items.to_vec())
            .unwrap_or_else(|| fallback.to_vec())
    })
}

/// Records and applies a controlled gallery `Tree` move request.
pub fn record_tree_move(
    sample_id: impl Into<String>,
    fallback: &[TreeItemDescriptor],
    tree_move: &TreeMove,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.to_vec();
    let next = cx.read_global::<TreeSampleRuntimeLog, _>(|log, _| {
        let current = log
            .tree_item_override(&sample_id)
            .map(|items| items.to_vec())
            .unwrap_or_else(|| fallback.clone());
        apply_tree_move(current, tree_move)
    });

    if let Some(next) = next {
        cx.update_default_global::<TreeSampleRuntimeLog, _>(|log, _| {
            log.moves.push(TreeSampleMoveEvent {
                sample_id: sample_id.clone(),
                tree_move: tree_move.clone(),
            });
            log.tree_item_overrides.insert(sample_id, next);
        });
    }
}
