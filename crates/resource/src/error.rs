use crate::QueryKey;

/// Error returned by renderer-neutral resource operations.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ResourceError {
    /// No query exists for this key.
    #[error("unknown query key: {0:?}")]
    UnknownQuery(QueryKey),
    /// No observer exists for this handle.
    #[error("unknown observer")]
    UnknownObserver,
    /// Mutation ids cannot be empty.
    #[error("mutation id cannot be empty")]
    EmptyMutationId,
    /// No mutation exists for this id.
    #[error("unknown mutation: {0}")]
    UnknownMutation(String),
}
