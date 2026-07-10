use std::{collections::BTreeMap, sync::Arc, time::Duration};

use crate::{FieldPath, FormError, FormStore, form::FormAuthority};

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

/// Result of attempting to complete asynchronous validation work.
#[must_use = "validation completion reports whether the result was applied"]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationCompletion {
    /// The ticket was current and its result was applied.
    Applied,
    /// A newer validation generation now owns the field.
    Stale,
    /// The ticket was cancelled by an edit, reset, synchronous validation, or completion.
    Cancelled,
}

/// Opaque ticket identifying one async validation generation for one field value.
#[must_use = "validation tickets must be completed or explicitly cancelled by a form mutation"]
#[derive(Clone, Debug)]
pub struct ValidationTicket {
    authority: Arc<FormAuthority>,
    path: FieldPath,
    value_revision: u64,
    generation: u64,
}

impl ValidationTicket {
    pub(crate) fn new(
        authority: Arc<FormAuthority>,
        path: FieldPath,
        value_revision: u64,
        generation: u64,
    ) -> Self {
        Self {
            authority,
            path,
            value_revision,
            generation,
        }
    }

    /// Returns the field path being validated.
    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    /// Returns the field value revision captured when validation began.
    pub fn value_revision(&self) -> u64 {
        self.value_revision
    }

    /// Returns the monotonic validation generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn belongs_to(&self, authority: &Arc<FormAuthority>) -> bool {
        Arc::ptr_eq(&self.authority, authority)
    }
}

impl PartialEq for ValidationTicket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.authority, &other.authority)
            && self.path == other.path
            && self.value_revision == other.value_revision
            && self.generation == other.generation
    }
}

impl Eq for ValidationTicket {}

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
