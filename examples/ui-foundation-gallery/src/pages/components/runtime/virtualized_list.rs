use super::*;

/// One activation captured from the rendered gallery `VirtualizedList` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleActivation {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Activated item index.
    pub index: usize,
    /// Activated item key.
    pub key: String,
    /// Activated item text value.
    pub text_value: String,
}

/// One nested row action captured from the rendered gallery `VirtualizedList` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleNestedAction {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Stable item key that owned the nested action.
    pub key: String,
}

/// Runtime activation log used by gallery smoke tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleRuntimeLog {
    activations: Vec<VirtualizedListSampleActivation>,
    nested_actions: Vec<VirtualizedListSampleNestedAction>,
}

impl Global for VirtualizedListSampleRuntimeLog {}

impl VirtualizedListSampleRuntimeLog {
    /// Returns captured activations in event order.
    pub fn activations(&self) -> &[VirtualizedListSampleActivation] {
        &self.activations
    }

    /// Returns captured nested actions in event order.
    pub fn nested_actions(&self) -> &[VirtualizedListSampleNestedAction] {
        &self.nested_actions
    }

    /// Clears captured activations.
    pub fn clear(&mut self) {
        self.activations.clear();
        self.nested_actions.clear();
    }
}

/// Records a gallery `VirtualizedList` activation in app-global sample state.
pub fn record_virtualized_list_activation(
    sample_id: impl Into<String>,
    index: usize,
    key: impl Into<String>,
    text_value: impl Into<String>,
    cx: &mut App,
) {
    cx.update_default_global::<VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.activations.push(VirtualizedListSampleActivation {
            sample_id: sample_id.into(),
            index,
            key: key.into(),
            text_value: text_value.into(),
        });
    });
}

/// Records a gallery `VirtualizedList` nested row action in app-global sample state.
pub fn record_virtualized_list_nested_action(
    sample_id: impl Into<String>,
    key: impl Into<String>,
    cx: &mut App,
) {
    cx.update_default_global::<VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.nested_actions.push(VirtualizedListSampleNestedAction {
            sample_id: sample_id.into(),
            key: key.into(),
        });
    });
}
