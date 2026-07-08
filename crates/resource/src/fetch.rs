use crate::QueryKey;

/// Ticket identifying one query fetch generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchTicket {
    /// Query key being fetched.
    pub key: QueryKey,
    /// Monotonic generation assigned by the resource client.
    pub generation: u64,
}
