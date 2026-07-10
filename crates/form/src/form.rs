use std::{collections::BTreeMap, sync::Arc};

use serde_json::Value;

use crate::{
    FieldMetaSnapshot, FieldPath, FieldSnapshot, FieldState, FieldValidationOutcome, FormError,
    FormSnapshot, FormStatus, RedactionPolicy, SubmitBlockReason, ValidationCompletion,
    ValidationTicket,
};

#[derive(Debug, Default)]
pub(crate) struct FormAuthority;

/// Opaque ticket identifying one active form submission.
#[must_use = "submission tickets must be completed or explicitly cancelled by a form mutation"]
#[derive(Clone, Debug)]
pub struct SubmitTicket {
    authority: Arc<FormAuthority>,
    form_revision: u64,
    generation: u64,
}

impl SubmitTicket {
    /// Returns the form revision captured when submission began.
    pub fn form_revision(&self) -> u64 {
        self.form_revision
    }

    /// Returns the monotonic submission generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn belongs_to(&self, authority: &Arc<FormAuthority>) -> bool {
        Arc::ptr_eq(&self.authority, authority)
    }
}

impl PartialEq for SubmitTicket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.authority, &other.authority)
            && self.form_revision == other.form_revision
            && self.generation == other.generation
    }
}

impl Eq for SubmitTicket {}

/// Result of attempting to complete submission work.
#[must_use = "submission completion reports whether the result was applied"]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitCompletion {
    /// The ticket was current and its result was applied.
    Applied,
    /// A newer active submission superseded the ticket.
    Stale,
    /// The submission was cancelled or had already completed.
    Cancelled,
}

#[derive(Clone, Debug, Default)]
enum SubmissionPhase {
    #[default]
    Idle,
    Submitting(SubmitTicket),
    Submitted,
    SubmitFailed,
}

/// Renderer-neutral form state owner.
#[derive(Debug, Default)]
pub struct FormStore {
    authority: Arc<FormAuthority>,
    fields: BTreeMap<FieldPath, FieldState>,
    submission: SubmissionPhase,
    errors: Vec<String>,
    submit_count: u32,
    form_revision: u64,
    next_validation_generation: u64,
    next_submit_generation: u64,
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
        self.advance_form_revision();
        Ok(())
    }

    /// Returns the current form status.
    pub fn status(&self) -> FormStatus {
        match self.submission {
            SubmissionPhase::Submitting(_) => FormStatus::Submitting,
            SubmissionPhase::Submitted => FormStatus::Submitted,
            SubmissionPhase::SubmitFailed => FormStatus::SubmitFailed,
            SubmissionPhase::Idle if self.is_validating() => FormStatus::Validating,
            SubmissionPhase::Idle => FormStatus::Idle,
        }
    }

    /// Returns whether the form can begin submission in its current state.
    pub fn can_submit(&self) -> bool {
        !matches!(self.submission, SubmissionPhase::Submitting(_))
            && !self.is_validating()
            && !self.is_invalid()
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
        let changed = self.field_or_err_mut(path)?.set_value(value);
        if changed {
            self.advance_form_revision();
        }
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
        if matches!(self.submission, SubmissionPhase::Submitting(_)) {
            return Err(FormError::CannotValidateWhileSubmitting);
        }
        self.field_or_err(path)?;
        self.clear_terminal_submission();
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
        if matches!(self.submission, SubmissionPhase::Submitting(_)) {
            return Err(FormError::CannotValidateWhileSubmitting);
        }
        self.field_or_err(path)?;
        let generation = next_generation(&mut self.next_validation_generation)?;
        self.clear_terminal_submission();
        let authority = Arc::clone(&self.authority);
        Ok(self
            .field_or_err_mut(path)?
            .begin_validation(authority, generation))
    }

    /// Completes an async validation generation.
    pub fn complete_async_validation(
        &mut self,
        ticket: ValidationTicket,
        errors: Vec<String>,
    ) -> ValidationCompletion {
        if !ticket.belongs_to(&self.authority) {
            return ValidationCompletion::Cancelled;
        }
        let Some(field) = self.fields.get_mut(ticket.path()) else {
            return ValidationCompletion::Cancelled;
        };
        field.complete_validation(&ticket, errors)
    }

    /// Begins the submit lifecycle.
    pub fn begin_submit(&mut self) -> Result<SubmitTicket, FormError> {
        if matches!(self.submission, SubmissionPhase::Submitting(_)) {
            return Err(FormError::CannotSubmit {
                reason: SubmitBlockReason::AlreadySubmitting,
            });
        }
        if self.is_validating() {
            return Err(FormError::CannotSubmit {
                reason: SubmitBlockReason::Validating,
            });
        }
        if self.is_invalid() {
            return Err(FormError::CannotSubmit {
                reason: SubmitBlockReason::Invalid,
            });
        }

        let generation = next_generation(&mut self.next_submit_generation)?;
        let ticket = SubmitTicket {
            authority: Arc::clone(&self.authority),
            form_revision: self.form_revision,
            generation,
        };
        self.submission = SubmissionPhase::Submitting(ticket.clone());
        self.submit_count = self.submit_count.saturating_add(1);
        self.errors.clear();
        Ok(ticket)
    }

    /// Marks the current submit as successful.
    pub fn finish_submit_success(&mut self, ticket: SubmitTicket) -> SubmitCompletion {
        let completion = self.submit_completion(&ticket);
        if completion != SubmitCompletion::Applied {
            return completion;
        }

        self.submission = SubmissionPhase::Submitted;
        self.errors.clear();
        SubmitCompletion::Applied
    }

    /// Marks the current submit as failed.
    pub fn finish_submit_error(
        &mut self,
        ticket: SubmitTicket,
        error: impl Into<String>,
    ) -> SubmitCompletion {
        let completion = self.submit_completion(&ticket);
        if completion != SubmitCompletion::Applied {
            return completion;
        }

        self.submission = SubmissionPhase::SubmitFailed;
        self.errors = vec![error.into()];
        SubmitCompletion::Applied
    }

    /// Resets all fields and form-level state.
    pub fn reset(&mut self) {
        for field in self.fields.values_mut() {
            field.reset();
        }
        self.advance_form_revision();
    }

    /// Returns a redaction-aware snapshot.
    pub fn snapshot(&self, redaction: RedactionPolicy) -> FormSnapshot {
        FormSnapshot {
            status: self.status(),
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

    fn is_validating(&self) -> bool {
        self.fields.values().any(|field| field.meta().validating)
    }

    fn is_invalid(&self) -> bool {
        self.fields
            .values()
            .any(|field| !field.meta().errors.is_empty())
    }

    fn clear_terminal_submission(&mut self) {
        if matches!(
            self.submission,
            SubmissionPhase::Submitted | SubmissionPhase::SubmitFailed
        ) {
            self.submission = SubmissionPhase::Idle;
            self.errors.clear();
        }
    }

    fn advance_form_revision(&mut self) {
        self.form_revision = self.form_revision.saturating_add(1);
        self.submission = SubmissionPhase::Idle;
        self.errors.clear();
    }

    fn submit_completion(&self, ticket: &SubmitTicket) -> SubmitCompletion {
        if !ticket.belongs_to(&self.authority) {
            return SubmitCompletion::Cancelled;
        }

        match &self.submission {
            SubmissionPhase::Submitting(active) if active == ticket => SubmitCompletion::Applied,
            SubmissionPhase::Submitting(_) => SubmitCompletion::Stale,
            SubmissionPhase::Idle | SubmissionPhase::Submitted | SubmissionPhase::SubmitFailed => {
                SubmitCompletion::Cancelled
            }
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

fn next_generation(current: &mut u64) -> Result<u64, FormError> {
    let next = current
        .checked_add(1)
        .ok_or(FormError::LifecycleGenerationExhausted)?;
    *current = next;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_validation_generation_preserves_terminal_state_atomically() {
        let mut store = FormStore::default();
        let email = FieldPath::new("account.email").unwrap();
        store
            .register_field(email.clone(), serde_json::json!("team@example.com"))
            .unwrap();
        let submit = store.begin_submit().unwrap();
        assert_eq!(
            store.finish_submit_error(submit, "retry later"),
            SubmitCompletion::Applied
        );
        store.next_validation_generation = u64::MAX;
        let before = store.snapshot(RedactionPolicy::Expose);

        assert_eq!(
            store.begin_async_validation(&email),
            Err(FormError::LifecycleGenerationExhausted)
        );
        assert_eq!(store.snapshot(RedactionPolicy::Expose), before);
    }
}
