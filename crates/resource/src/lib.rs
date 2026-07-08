#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod client;
mod error;
mod fetch;
mod key;
mod mutation;
mod pagination;
mod policy;
mod redaction;
mod snapshot;

pub use client::{InvalidationOutcome, ObserverHandle, ResourceClient};
pub use error::ResourceError;
pub use fetch::FetchTicket;
pub use key::{QueryKey, QueryKeyError, QueryKeySegment};
pub use mutation::MutationTicket;
pub use pagination::{
    PaginatedResourceSnapshot, PaginatedResourceSnapshotView, ResourcePage, ResourcePageSnapshot,
};
pub use policy::RetryPolicy;
pub use redaction::{RedactedResourceValue, ResourceRedactionPolicy};
pub use snapshot::{MutationSnapshot, MutationStatus, ResourceSnapshot, ResourceStatus};
