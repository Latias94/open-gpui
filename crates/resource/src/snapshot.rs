use serde::{Deserialize, Serialize};

use crate::{QueryKey, RedactedResourceValue};

/// Current lifecycle state for a query resource.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ResourceStatus {
    /// The resource has no active fetch.
    #[default]
    Idle,
    /// The initial fetch is running.
    Loading,
    /// Data is available.
    Success,
    /// Data is available but stale.
    Stale,
    /// A background fetch is running.
    Refetching,
    /// The latest fetch failed.
    Error,
}

/// Current lifecycle state for a resource mutation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum MutationStatus {
    /// The mutation is idle.
    #[default]
    Idle,
    /// The mutation is running.
    Pending,
    /// The mutation succeeded.
    Success,
    /// The mutation failed.
    Error,
}

/// Redaction-aware resource snapshot for tests, adapters, and devtools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// Stable query key.
    pub key: QueryKey,
    /// Current resource lifecycle state.
    pub status: ResourceStatus,
    /// Redacted resource payload.
    pub data: Option<RedactedResourceValue>,
    /// Current error summary, if any.
    pub error: Option<String>,
    /// Number of active observers.
    pub observer_count: usize,
    /// Number of fetch attempts for the current generation.
    pub fetch_attempts: u32,
}

/// Redaction-aware mutation snapshot for tests, adapters, and devtools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MutationSnapshot {
    /// Stable mutation id.
    pub id: String,
    /// Current mutation lifecycle state.
    pub status: MutationStatus,
    /// Redacted mutation payload.
    pub data: Option<RedactedResourceValue>,
    /// Current error summary, if any.
    pub error: Option<String>,
}
