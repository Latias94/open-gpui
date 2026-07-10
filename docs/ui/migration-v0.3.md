# Open GPUI v0.3 UI Migration

Open GPUI v0.3 is an intentionally breaking, pre-release architecture release. Replaced APIs are
deleted rather than retained as aliases. This guide records each migration in the implementation
unit that introduces it.

## Form Lifecycle Authority

`FormStore` now derives `FormStatus` and submission eligibility from active validation plus its
submission phase. Callers no longer coordinate status changes indirectly.

`FormStore` is no longer `Clone`: cloning a live authority made its outstanding tickets ambiguous.
Share `FormSnapshot` or application-owned field values when another consumer needs a copy. Tickets
are scoped to the exact store that created them and are rejected by other stores.

### Async Validation

`ValidationTicket` is opaque and includes the field value revision. Completion returns a typed
result instead of `bool`:

```rust
let ticket = form.begin_async_validation(&email)?;
let completion = form.complete_async_validation(ticket, errors);

match completion {
    ValidationCompletion::Applied => {}
    ValidationCompletion::Stale | ValidationCompletion::Cancelled => return Ok(()),
}
```

An edit, reset, synchronous validation, or newer generation invalidates old work. Starting
validation during submission now returns `FormError::CannotValidateWhileSubmitting`.

Successfully registering a new field also advances the form revision and invalidates an active or
terminal submission because the submitted data shape changed. A rejected duplicate registration
does not mutate lifecycle state.

### Submission

`begin_submit()` now returns `Result<SubmitTicket, FormError>`. Both finish methods require that
ticket and return `SubmitCompletion`:

```rust
let ticket = form.begin_submit()?;

match request_result {
    Ok(()) => {
        let _completion = form.finish_submit_success(ticket);
    }
    Err(error) => {
        let _completion = form.finish_submit_error(ticket, error.to_string());
    }
}
```

Handle `FormError::CannotSubmit { reason }` with `SubmitBlockReason::{Invalid, Validating,
AlreadySubmitting}`. The old free-form rejection reason and ticketless finish methods no longer
exist. Rejected starts do not increment `submit_count`.

### UI Projection

Use `FormProjection::resolve(&snapshot, disabled)` for form-level busy state and submit eligibility.
`FormFieldProjection` now exposes `validating()` and `busy()`, and propagates busy state to Field,
TextInput, Textarea, NumberInput, and Checkbox state. Busy validation does not imply disabled or
read-only input. When rebuilding concrete components from those states, pass `busy(state.busy())`;
all five concrete builders preserve that fact in their resolved `state()`.

### DevTools Boundary

The first-party form adapter no longer forwards caller-selected values, field paths, field ids, or
free-form errors. It emits typed redaction markers, counts, lifecycle facts, and deterministic
opaque field identities before a `DevtoolsCapture` is constructed. Do not parse DevTools payloads
for application form data; application and rendering code should consume `FormSnapshot` directly.
