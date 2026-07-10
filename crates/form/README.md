# open-gpui-form

Renderer-neutral form state primitives for Open GPUI applications.

This crate owns form values, field identity, metadata snapshots, validation lifecycle, submission
state, and redaction-aware diagnostic snapshots. It does not depend on GPUI or concrete UI
components. GPUI bindings live above this crate, first in `open-gpui-ui-components`.

## Public Contract

- `FieldPath` and `FieldId` identify fields without renderer state.
- `FormStore` owns field registration, values, dirty/touched/visited state, validation generations,
  submission phase, reset, and redaction-aware snapshots. Effective `FormStatus` and submission
  eligibility are derived from that state rather than assigned independently.
- `FieldLens<T>` lets typed application state project individual fields without moving ownership
  into UI components.
- `FieldMetaSnapshot`, `FieldSnapshot`, and `FormSnapshot` describe inspectable form state.
- `RedactionPolicy` controls how values are represented in snapshots.
- `ValidationTicket` and `SubmitTicket` bind asynchronous work to the owning `FormStore`, revision,
  and generation that created it. `ValidationCompletion` and `SubmitCompletion` distinguish
  applied, stale, and cancelled results without asking callers to infer lifecycle state.
- `DebouncedValidationQueue` keeps at most one queued validation ticket per field.

## Lifecycle Rules

- Starting async validation makes the effective status `Validating` until the final current field
  ticket completes. Validation does not make fields read-only.
- A value change cancels validation for that field and any active submission. Writing the same
  value is a no-op and preserves current tickets.
- Successfully registering a new field advances the form revision because it changes the submitted
  data shape. It cancels an active submission or clears a terminal submission result; rejected
  duplicate registration is atomic and changes nothing.
- Reset and synchronous validation cancel affected async work. A stale or cancelled completion
  never changes values, errors, status, or counters.
- Submission is rejected with `SubmitBlockReason` while the form is invalid, validating, or already
  submitting. Successful starts return an opaque `SubmitTicket`; only that active ticket may
  complete the submission.
- Validation cannot start while submission owns the lifecycle. An edit after `Submitted` or
  `SubmitFailed` clears the terminal result and returns to the status derived from current fields.
- `FormStore` is a single lifecycle authority and is intentionally not `Clone`. Share immutable
  snapshots or application-owned values instead of duplicating a live store and its tickets.

## Basic Use

```rust
use open_gpui_form::{FieldPath, FormStore, RedactionPolicy};

let mut form = FormStore::default();
let email = FieldPath::new("account.email")?;

form.register_field(email.clone(), serde_json::json!(""))?;
form.set_value(&email, serde_json::json!("team@example.com"))?;
form.touch(&email)?;
form.validate_field(&email, |value| {
    value
        .as_str()
        .filter(|value| value.contains('@'))
        .map(|_| Vec::new())
        .unwrap_or_else(|| vec!["Email is invalid".to_owned()])
})?;

let snapshot = form.snapshot(RedactionPolicy::RedactAll);
assert_eq!(snapshot.fields.len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Integration Rules

- Keep application data app-owned. `FormStore` stores field values and metadata, while rendering
  code projects `FieldSnapshot` into concrete controls through `open_gpui_ui_components::FormFieldProjection`.
- Keep validation adapter-friendly. Core validation accepts closures and tickets; schema libraries
  should integrate outside this crate.
- Treat snapshots as application/UI payloads that may contain field and error text. Use
  `RedactionPolicy::RedactAll` or `RedactionPolicy::Summarize` before exposing them to tests or logs.
- Use `open_gpui_devtools::form` when the `open-gpui-devtools/form` feature is enabled; the
  adapter applies an allowlist before capture construction. It always replaces values and
  free-form errors, uses opaque field identities, and records a redaction summary regardless of the
  snapshot's caller-selected value policy.

## Verification

For focused form changes, run:

```sh
cargo fmt -p open-gpui-form
cargo check -p open-gpui-form --tests --locked
cargo nextest run -p open-gpui-form --no-fail-fast --locked
```

When changing UI adapters that consume form snapshots, also run:

```sh
cargo nextest run -p open-gpui-ui-components form_adapter --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery form --no-fail-fast --locked
```
