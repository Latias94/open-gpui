# open-gpui-form

Renderer-neutral form state primitives for Open GPUI applications.

This crate owns form values, field identity, metadata snapshots, validation lifecycle, submission
state, and redaction-aware diagnostic snapshots. It does not depend on GPUI or concrete UI
components. GPUI bindings live above this crate, first in `open-gpui-ui-components`.

## Public Contract

- `FieldPath` and `FieldId` identify fields without renderer state.
- `FormStore` owns field registration, values, dirty/touched/visited state, validation generations,
  submit lifecycle, reset, and redacted snapshots.
- `FieldLens<T>` lets typed application state project individual fields without moving ownership
  into UI components.
- `FieldMetaSnapshot`, `FieldSnapshot`, and `FormSnapshot` describe inspectable form state.
- `RedactionPolicy` controls how values are represented in snapshots.
- `DebouncedValidationQueue` and `ValidationTicket` give async validation a generation boundary so
  stale results can be ignored.

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
- Treat snapshots as diagnostic payloads. Use `RedactionPolicy::RedactAll` or
  `RedactionPolicy::Summarize` before exposing form data to devtools, tests, or logs.

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
