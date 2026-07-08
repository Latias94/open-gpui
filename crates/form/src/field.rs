use serde_json::Value;

use crate::{FieldId, FieldMetaSnapshot, FieldPath};

/// Internal renderer-neutral field state owned by a [`crate::FormStore`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldState {
    id: FieldId,
    path: FieldPath,
    initial_value: Value,
    value: Value,
    meta: FieldMetaSnapshot,
    validation_generation: u64,
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
            validation_generation: 0,
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

    pub(crate) fn set_value(&mut self, value: Value) {
        self.value = value;
        self.meta.dirty = self.value != self.initial_value;
    }

    pub(crate) fn touch(&mut self) {
        self.meta.touched = true;
    }

    pub(crate) fn visit(&mut self) {
        self.meta.visited = true;
    }

    pub(crate) fn set_errors(&mut self, errors: Vec<String>) {
        self.meta.errors = errors;
    }

    pub(crate) fn begin_validation(&mut self, generation: u64) {
        self.validation_generation = generation;
        self.meta.validating = true;
    }

    pub(crate) fn complete_validation(&mut self, generation: u64, errors: Vec<String>) -> bool {
        if self.validation_generation != generation {
            return false;
        }
        self.meta.validating = false;
        self.meta.errors = errors;
        true
    }

    pub(crate) fn reset(&mut self) {
        self.value = self.initial_value.clone();
        self.meta = FieldMetaSnapshot::default();
        self.validation_generation = 0;
    }
}
