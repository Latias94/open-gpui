# open-gpui-resource

Renderer-neutral async resource state primitives for Open GPUI applications.

This crate owns query keys, observer snapshots, mutation snapshots, cache lifecycle vocabulary, and
redaction-aware diagnostics. It is protocol-agnostic: applications provide fetchers and mutations;
HTTP integration belongs in optional adapters or examples.

## Public Contract

- `QueryKey` identifies cache entries without transport policy.
- `ResourceClient` owns query entries, observer counts, fetch generations, invalidation, mutation
  generations, and snapshot production.
- `ResourceStatus` and `MutationStatus` describe query and mutation lifecycle state.
- `RetryPolicy` records retry/backoff policy without owning timers or an executor.
- `PaginatedResourceSnapshot` and `ResourcePageSnapshot` describe ordered pages and cursor metadata.
- `ResourceSnapshot` and `MutationSnapshot` are safe to show in tests and devtools when paired with
  `ResourceRedactionPolicy`.

## Basic Use

```rust
use open_gpui_resource::{QueryKey, ResourceClient, ResourceRedactionPolicy, ResourceStatus};

let mut client = ResourceClient::default();
let key = QueryKey::new(["projects"])?;

let _observer = client.subscribe(key.clone())?;
let fetch = client.begin_fetch(&key)?;
assert!(client.complete_fetch_success(fetch, serde_json::json!([
    { "name": "Open GPUI" }
])));

let snapshot = client.snapshot(&key, ResourceRedactionPolicy::RedactAll)?;
assert_eq!(snapshot.status, ResourceStatus::Success);
assert_eq!(snapshot.observer_count, 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Integration Rules

- Keep fetching protocol-agnostic. This crate does not know about HTTP, GraphQL, files, sockets, or
  executors; apps start/complete fetch and mutation generations around their own async work.
- Keep cache data redacted by default when crossing diagnostic boundaries. Use
  `ResourceRedactionPolicy::RedactAll` or `ResourceRedactionPolicy::Summarize` before sending
  snapshots to devtools.
- Project query snapshots into UI components through
  `open_gpui_ui_components::ResourceCollectionProjection` and mutation snapshots through
  `open_gpui_ui_components::ResourceMutationProjection`.
- Use `open_gpui_devtools::resource` when the `open-gpui-devtools/resource` feature is enabled;
  the adapter derives DevTools redaction summaries from `RedactedResourceValue::Redacted` query,
  mutation, and paginated page values.

## Verification

For focused resource changes, run:

```sh
cargo fmt -p open-gpui-resource
cargo check -p open-gpui-resource --tests --locked
cargo nextest run -p open-gpui-resource --no-fail-fast --locked
```

When changing UI adapters that consume resource snapshots, also run:

```sh
cargo nextest run -p open-gpui-ui-components resource_adapter --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery resource --no-fail-fast --locked
```
