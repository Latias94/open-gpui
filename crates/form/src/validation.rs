use std::{collections::BTreeMap, time::Duration};

use crate::{FieldPath, FormError, FormStore};

/// Result of a synchronous field validation pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FieldValidationOutcome {
    /// Validation errors produced by this pass.
    pub errors: Vec<String>,
}

impl FieldValidationOutcome {
    /// Returns true when no errors were produced.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Ticket identifying one async validation generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationTicket {
    /// Field path being validated.
    pub path: FieldPath,
    /// Monotonic generation assigned by the form store.
    pub generation: u64,
}

/// Deterministic debounce queue for async validation requests.
#[derive(Clone, Debug)]
pub struct DebouncedValidationQueue {
    debounce: Duration,
    pending: BTreeMap<FieldPath, ValidationTicket>,
}

impl DebouncedValidationQueue {
    /// Creates a debounce queue.
    pub fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            pending: BTreeMap::new(),
        }
    }

    /// Returns the debounce duration.
    pub fn debounce(&self) -> Duration {
        self.debounce
    }

    /// Requests validation for a field, replacing older pending requests for that field.
    pub fn request(
        &mut self,
        store: &mut FormStore,
        path: &FieldPath,
    ) -> Result<ValidationTicket, FormError> {
        let ticket = store.begin_async_validation(path)?;
        self.pending.insert(path.clone(), ticket.clone());
        Ok(ticket)
    }

    /// Returns the number of pending field requests.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Pops the next pending validation ticket in deterministic field order.
    pub fn take_pending(&mut self) -> Option<ValidationTicket> {
        let path = self.pending.keys().next().cloned()?;
        self.pending.remove(&path)
    }
}
