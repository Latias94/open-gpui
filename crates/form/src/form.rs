use std::collections::BTreeMap;

use serde_json::Value;

use crate::{
    FieldMetaSnapshot, FieldPath, FieldSnapshot, FieldState, FieldValidationOutcome, FormError,
    FormSnapshot, FormStatus, RedactionPolicy, ValidationTicket,
};

/// Renderer-neutral form state owner.
#[derive(Clone, Debug, Default)]
pub struct FormStore {
    fields: BTreeMap<FieldPath, FieldState>,
    status: FormStatus,
    errors: Vec<String>,
    submit_count: u32,
    next_validation_generation: u64,
}

impl FormStore {
    /// Registers a field and its initial value.
    pub fn register_field(
        &mut self,
        path: FieldPath,
        initial_value: Value,
    ) -> Result<(), FormError> {
        if self.fields.contains_key(&path) {
            return Err(FormError::DuplicateField(path));
        }
        self.fields
            .insert(path.clone(), FieldState::new(path, initial_value));
        Ok(())
    }

    /// Returns the current form status.
    pub fn status(&self) -> FormStatus {
        self.status.clone()
    }

    /// Returns a field by path.
    pub fn field(&self, path: &FieldPath) -> Option<&FieldState> {
        self.fields.get(path)
    }

    /// Returns a field's current value.
    pub fn value(&self, path: &FieldPath) -> Result<&Value, FormError> {
        Ok(self.field_or_err(path)?.value())
    }

    /// Updates a field value and dirty state.
    pub fn set_value(&mut self, path: &FieldPath, value: Value) -> Result<(), FormError> {
        self.field_or_err_mut(path)?.set_value(value);
        Ok(())
    }

    /// Marks a field as touched.
    pub fn touch(&mut self, path: &FieldPath) -> Result<(), FormError> {
        self.field_or_err_mut(path)?.touch();
        Ok(())
    }

    /// Marks a field as visited.
    pub fn visit(&mut self, path: &FieldPath) -> Result<(), FormError> {
        self.field_or_err_mut(path)?.visit();
        Ok(())
    }

    /// Runs synchronous validation for one field.
    pub fn validate_field(
        &mut self,
        path: &FieldPath,
        validator: impl FnOnce(&Value) -> Vec<String>,
    ) -> Result<FieldValidationOutcome, FormError> {
        let field = self.field_or_err_mut(path)?;
        let errors = validator(field.value());
        field.set_errors(errors.clone());
        Ok(FieldValidationOutcome { errors })
    }

    /// Starts an async validation generation for one field.
    pub fn begin_async_validation(
        &mut self,
        path: &FieldPath,
    ) -> Result<ValidationTicket, FormError> {
        self.next_validation_generation += 1;
        let generation = self.next_validation_generation;
        self.field_or_err_mut(path)?.begin_validation(generation);
        Ok(ValidationTicket {
            path: path.clone(),
            generation,
        })
    }

    /// Completes an async validation generation.
    pub fn complete_async_validation(
        &mut self,
        ticket: ValidationTicket,
        errors: Vec<String>,
    ) -> bool {
        self.fields
            .get_mut(&ticket.path)
            .is_some_and(|field| field.complete_validation(ticket.generation, errors))
    }

    /// Begins the submit lifecycle.
    pub fn begin_submit(&mut self) -> Result<(), FormError> {
        let has_errors = self
            .fields
            .values()
            .any(|field| !field.meta().errors.is_empty());
        if has_errors {
            return Err(FormError::CannotSubmit {
                reason: "validation errors remain".to_owned(),
            });
        }
        let validating = self.fields.values().any(|field| field.meta().validating);
        if validating {
            return Err(FormError::CannotSubmit {
                reason: "validation is still running".to_owned(),
            });
        }
        self.status = FormStatus::Submitting;
        self.submit_count += 1;
        self.errors.clear();
        Ok(())
    }

    /// Marks the current submit as successful.
    pub fn finish_submit_success(&mut self) {
        self.status = FormStatus::Submitted;
        self.errors.clear();
    }

    /// Marks the current submit as failed.
    pub fn finish_submit_error(&mut self, error: impl Into<String>) {
        self.status = FormStatus::SubmitFailed;
        self.errors = vec![error.into()];
    }

    /// Resets all fields and form-level state.
    pub fn reset(&mut self) {
        for field in self.fields.values_mut() {
            field.reset();
        }
        self.status = FormStatus::Idle;
        self.errors.clear();
    }

    /// Returns a redaction-aware snapshot.
    pub fn snapshot(&self, redaction: RedactionPolicy) -> FormSnapshot {
        FormSnapshot {
            status: self.status.clone(),
            fields: self
                .fields
                .values()
                .map(|field| FieldSnapshot {
                    id: field.id().clone(),
                    path: field.path().clone(),
                    value: redaction.apply(field.value().clone()),
                    meta: FieldMetaSnapshot {
                        dirty: field.meta().dirty,
                        touched: field.meta().touched,
                        visited: field.meta().visited,
                        validating: field.meta().validating,
                        errors: field.meta().errors.clone(),
                    },
                })
                .collect(),
            errors: self.errors.clone(),
            submit_count: self.submit_count,
        }
    }

    fn field_or_err(&self, path: &FieldPath) -> Result<&FieldState, FormError> {
        self.fields
            .get(path)
            .ok_or_else(|| FormError::UnknownField(path.clone()))
    }

    fn field_or_err_mut(&mut self, path: &FieldPath) -> Result<&mut FieldState, FormError> {
        self.fields
            .get_mut(path)
            .ok_or_else(|| FormError::UnknownField(path.clone()))
    }
}
