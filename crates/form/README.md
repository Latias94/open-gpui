# open-gpui-form

Renderer-neutral form state primitives for Open GPUI applications.

This crate owns form values, field identity, metadata snapshots, validation lifecycle, submission
state, and redaction-aware diagnostic snapshots. It does not depend on GPUI or concrete UI
components. GPUI bindings live above this crate, first in `open-gpui-ui-components`.

The first public contract is intentionally small:

- `FieldPath` and `FieldId` identify fields without renderer state.
- `FieldMetaSnapshot`, `FieldSnapshot`, and `FormSnapshot` describe inspectable form state.
- `RedactionPolicy` controls how values are represented in snapshots.
