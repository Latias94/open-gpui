/// Ticket identifying one mutation generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationTicket {
    /// Stable mutation id.
    pub id: String,
    /// Monotonic generation assigned by the resource client.
    pub generation: u64,
}
