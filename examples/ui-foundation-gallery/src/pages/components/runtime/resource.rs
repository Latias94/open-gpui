/// Deterministic resource runtime event shown by the gallery integration sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceSampleRuntimeEvent {
    /// Initial query fetch.
    Fetch,
    /// Retry after a failed fetch.
    Retry,
    /// Invalidate an observed query and refetch in the background.
    Invalidate,
    /// Run a mutation that may invalidate queries.
    Mutate,
}

/// Read-only runtime log for resource adapter samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSampleRuntimeLog {
    /// Stable sample id.
    pub sample_id: &'static str,
    /// Deterministic events represented by the sample set.
    pub events: Vec<ResourceSampleRuntimeEvent>,
}

/// Returns the deterministic resource sample runtime log.
pub fn resource_sample_runtime_log() -> ResourceSampleRuntimeLog {
    ResourceSampleRuntimeLog {
        sample_id: "resource-adapters",
        events: vec![
            ResourceSampleRuntimeEvent::Fetch,
            ResourceSampleRuntimeEvent::Retry,
            ResourceSampleRuntimeEvent::Invalidate,
            ResourceSampleRuntimeEvent::Mutate,
        ],
    }
}
