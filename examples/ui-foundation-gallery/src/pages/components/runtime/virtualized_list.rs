use super::*;

/// One activation captured from the rendered gallery `VirtualizedList` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleActivation {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Activated item index.
    pub index: usize,
}

/// Runtime activation log used by gallery smoke tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VirtualizedListSampleRuntimeLog {
    activations: Vec<VirtualizedListSampleActivation>,
}

impl Global for VirtualizedListSampleRuntimeLog {}

impl VirtualizedListSampleRuntimeLog {
    /// Returns captured activations in event order.
    pub fn activations(&self) -> &[VirtualizedListSampleActivation] {
        &self.activations
    }

    /// Clears captured activations.
    pub fn clear(&mut self) {
        self.activations.clear();
    }
}

/// Records a gallery `VirtualizedList` activation in app-global sample state.
pub fn record_virtualized_list_activation(
    sample_id: impl Into<String>,
    index: usize,
    cx: &mut App,
) {
    cx.update_default_global::<VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.activations.push(VirtualizedListSampleActivation {
            sample_id: sample_id.into(),
            index,
        });
    });
}
