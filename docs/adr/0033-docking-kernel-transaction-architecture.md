# ADR 0033: Docking Kernel Transaction Architecture

**Status**: Accepted
**Date**: 2026-08-21

## Context

The original docking model served two incompatible roles. A live `DockGraph` was the semantic
layout authority, while the same graph also accepted unattached nodes during construction. Checked
operations mutated that shared structure, ran a global retention sweep and validation pass, and
restored a complete graph clone on failure. Workspace, drop, merge, recovery, and live-undock paths
then added expected/projected graph snapshots around the same mechanism.

That design preserved atomic behavior but made transaction cost proportional to unrelated graph
size. It also coupled live node allocation, staging lifetime, durable layout, transient tab history,
floating geometry, and asynchronous native effects. Structural node ids were treated as both
replaceable topology keys and long-lived command identities, while floating roots had both a node
wrapper and a side-list owner.

Dear ImGui's docking branch provides a mature behavior reference. Its useful properties are binary
local split/merge rewrites, explicit central-node inheritance, queued docking requests, one proposal
for preview and submit, parent identity during sibling promotion, and independent viewport
reconciliation. Its global mutable context, raw pointer requests, assertion-based failure handling,
and platform viewport heuristics do not fit GPUI's retained ownership model.

## Decision

Open GPUI will replace the staged docking kernel instead of wrapping it.

The canonical live model uses:

- stable `DockSpaceId`, group, item, and floating-root identities;
- replaceable structural node identities;
- binary branch nodes with exactly two children and an inclusive finite ratio in `[0, 1]`;
- group leaves that own tab order and durable selection;
- DockSpace-owned central policy that may remain alive while empty;
- first-class floating roots with one owner, bounds, and z-order;
- canonical node-parent, node-owner, group-leaf, item-group, and floating-item indexes;
- an authority epoch plus topology revision for every proof against a published graph.

`DockGraphAssembly` is the only place where unattached construction values may exist. Publication
consumes an assembly, validates the entire forest, builds reverse indexes, and returns an always
canonical graph. Live graph validation rejects cycles, shared children, unreachable nodes, duplicate
durable identities, duplicate items, invalid runtime MRU, invalid ratios or size references, invalid
central ownership, and ambiguous floating z-order.

Payload state has three distinct ownership forms:

1. `DockPayloadRef` is a cloneable, graph-version-bound read proof.
2. `PreparedDetach` is a non-cloneable proof consumed by the commit engine.
3. `DetachedDockTree` is a sole owner created only during assembly/import or inside the infallible
   commit interval after authoritative extraction.

Preparing a payload never copies or removes a live subtree. Central membership remains owned by the
source DockSpace and never travels inside detached content.

Persistent layout version 3 stores binary topology, stable group and floating ids, tab order,
selection, central policy, floating bounds and z-order, and optional `SizeRef`. It does not persist
runtime structural keys, MRU, GPUI focus, viewport handles, or platform facts. The decoder reads
`layout_version` before selecting a version-specific DTO. Layout version 2 is accepted only through
one deterministic migrator:

- n-ary splits fold into a right-deep binary chain;
- each ratio is the current weight divided by the remaining suffix weight;
- an all-zero original split is invalid, while an unobservable all-zero suffix uses `0.5`;
- group ids derive from encoded DockSpace, main/floating scope, and the original child-index path;
- floating ids and z-order derive from the DockSpace-local floating ordinal;
- version 2 does not invent `SizeRef` values.

Binary geometry follows the pinned ImGui behavior contract: split creation truncates the first size
and gives the second child the remainder; regular layout rounds the first size after applying the
minimum; a central descendant receives the remainder while its sibling's explicit size is
preserved; sibling promotion keeps the parent structural identity and `SizeRef`, retires both child
roots, and emits an explicit survivor-to-parent remap. Inner splits transfer central identity to the
content inheritor; outer splits keep central identity inside the old tree.

The replacement domain remains crate-private until production readers and writers move together.
The later transaction engine must perform all fallible semantic work before its first authoritative
write, reserve structural capacity, commit a local patch without business-level failure, advance
typed revisions, and install an idempotent receipt before publishing effects.

## Preserved Boundaries

- `DockSurface` remains the application facade.
- The graph remains pure semantic data and does not own GPUI windows, focus, renderer submission,
  native capture, HWND lifetime, or viewport session lineage.
- Presentation scenes remain derived per-host geometry authorities.
- ADR 0032's crate-private scenario vocabulary remains the single functional and benchmark adapter
  surface.
- Preview and submit will consume the same immutable proposal; native routing remains explicit and
  point-scoped rather than copied from ImGui's global viewport heuristics.

## Superseded Decisions

This ADR supersedes:

- public or runtime staged `insert_node -> set_root` construction;
- `may_have_staging_nodes`, mutation-retention closure, and global canonical sweeps;
- whole-graph rollback clones and expected/projected graph replacement transactions;
- n-ary same-axis flattening as the live canonical topology;
- floating wrapper nodes plus a separately mutable floating side list;
- raw structural node ids as durable command or persistence identities.

Compatibility adapters may exist only inside characterization tests during the cutover and must be
removed when the production path migrates.

## Consequences

- Interactive mutation cost can scale with the affected ancestor path instead of total graph size.
- Stable content identity survives structural rewrites without pretending structural nodes are
  durable.
- Invalid construction cannot leak into persistence or live interaction.
- Failed preparation leaves graph contents and future allocator behavior unchanged because no
  authoritative write has occurred.
- Workspace and native sagas can carry bounded patches, read sets, reservations, and receipts rather
  than full graph snapshots.
- The migration is intentionally breaking. Existing low-level staged builders, graph replacement
  APIs, and graph-shaped queued actions will be deleted as their production callers move.

## Rejected Alternatives

### Keep the staged graph and optimize its clone

**Rejected** because structural copy-on-write still performs graph-size-dependent slot copying on
the first write and preserves the dual live/staging authority.

### Use a general undo log

**Rejected** because SlotMap insertion and removal change allocator generations and free-list state.
A generic undo log cannot prove exact failure atomicity or future id allocation equivalence.

### Copy ImGui's docking context directly

**Rejected** because its global mutable context, pointer lifetime assumptions, settings mirroring,
and viewport heuristics conflict with retained GPUI and native authority boundaries.

### Keep n-ary splits as the transaction topology

**Rejected** because local split, collapse, and identity remap become less explicit, and flattening
introduces broad vector rewrites that are unnecessary for ImGui-parity interaction behavior.

## Success Criteria

- A published replacement graph cannot contain unattached or multiply owned nodes.
- Payload proof, prepared detachment, and detached ownership are distinct types with no copied live
  subtree before commit.
- Equivalent version-2 layouts migrate to the same durable identities and binary behavior regardless
  of legacy node ids or serialized node order.
- Layout version-3 export/import preserves durable content and geometry facts while dropping runtime
  MRU and platform state.
- Size allocation, central inheritance, and sibling promotion match the pinned ImGui oracle.
- Production cutover removes staging, whole-graph rollback, and graph-replacement transaction paths
  rather than retaining fallbacks.
