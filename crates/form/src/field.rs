use std::sync::Arc;

use serde_json::Value;

use crate::{
    FieldId, FieldMetaSnapshot, FieldPath, ValidationCompletion, ValidationTicket,
    form::FormAuthority,
};

/// Internal renderer-neutral field state owned by a [`crate::FormStore`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldState {
    id: FieldId,
    path: FieldPath,
    initial_value: Value,
    value: Value,
    meta: FieldMetaSnapshot,
    value_revision: u64,
    active_validation_generation: Option<u64>,
}

impl FieldState {
    /// Creates field state with an initial value.
    pub fn new(path: FieldPath, initial_value: Value) -> Self {
        let id = FieldId::from(&path);
        Self {
            id,
            path,
            initial_value: initial_value.clone(),
            value: initial_value,
            meta: FieldMetaSnapshot::default(),
            value_revision: 0,
            active_validation_generation: None,
        }
    }

    /// Returns the stable field id.
    pub fn id(&self) -> &FieldId {
        &self.id
    }

    /// Returns the stable field path.
    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    /// Returns the current value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Returns the meta snapshot.
    pub fn meta(&self) -> &FieldMetaSnapshot {
        &self.meta
    }

    pub(crate) fn set_value(&mut self, value: Value) -> bool {
        if self.value == value {
            return false;
        }

        self.value = value;
        self.value_revision = self.value_revision.saturating_add(1);
        self.meta.dirty = self.value != self.initial_value;
        self.meta.errors.clear();
        self.cancel_validation();
        true
    }

    pub(crate) fn touch(&mut self) {
        self.meta.touched = true;
    }

    pub(crate) fn visit(&mut self) {
        self.meta.visited = true;
    }

    pub(crate) fn set_errors(&mut self, errors: Vec<String>) {
        self.cancel_validation();
        self.meta.errors = errors;
    }

    pub(crate) fn begin_validation(
        &mut self,
        authority: Arc<FormAuthority>,
        generation: u64,
    ) -> ValidationTicket {
        self.active_validation_generation = Some(generation);
        self.meta.validating = true;
        ValidationTicket::new(
            authority,
            self.path.clone(),
            self.value_revision,
            generation,
        )
    }

    pub(crate) fn complete_validation(
        &mut self,
        ticket: &ValidationTicket,
        errors: Vec<String>,
    ) -> ValidationCompletion {
        match self.active_validation_generation {
            Some(generation) if generation == ticket.generation() => {}
            Some(_) => return ValidationCompletion::Stale,
            None => return ValidationCompletion::Cancelled,
        }
        if ticket.value_revision() != self.value_revision {
            return ValidationCompletion::Cancelled;
        }

        self.active_validation_generation = None;
        self.meta.validating = false;
        self.meta.errors = errors;
        ValidationCompletion::Applied
    }

    pub(crate) fn cancel_validation(&mut self) {
        self.active_validation_generation = None;
        self.meta.validating = false;
    }

    pub(crate) fn reset(&mut self) {
        self.value = self.initial_value.clone();
        self.value_revision = self.value_revision.saturating_add(1);
        self.meta = FieldMetaSnapshot::default();
        self.active_validation_generation = None;
    }
}
