# open-gpui-resource

Renderer-neutral async resource state primitives for Open GPUI applications.

This crate owns query keys, observer snapshots, mutation snapshots, cache lifecycle vocabulary, and
redaction-aware diagnostics. It is protocol-agnostic: applications provide fetchers and mutations;
HTTP integration belongs in optional adapters or examples.

The first public contract is intentionally small:

- `QueryKey` identifies cache entries without transport policy.
- `ResourceStatus` and `MutationStatus` describe async lifecycle state.
- `ResourceSnapshot` and `MutationSnapshot` are safe to show in tests and devtools when paired with
  `RedactionPolicy`.
