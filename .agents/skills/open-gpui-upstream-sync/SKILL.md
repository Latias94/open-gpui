---
name: open-gpui-upstream-sync
description: Analyze or implement Zed GPUI updates in the extracted Open-GPUI workspace, establish fixed comparison revisions, and reconcile upstream mechanisms with fork-owned contracts.
---

# Open-GPUI Upstream Sync

Use the existing [candidate queue](../../../docs/imports/zed-gpui-upstream-sync-candidates.md)
for fixed revisions and dispositions, and the linked owning plan for execution and verification.
Read [ADR 0001](../../../docs/adr/0001-open-gpui-fork-strategy.md) and the
[import record](../../../docs/imports/zed-gpui-import.md) when the dependency boundary changes.
Keep changing hashes, task status and results in those documents rather than this skill.
For a read-only assessment, return the evidenced differences, proposed dispositions, prerequisites
and verification requirements. Implement, edit records or run tests only within the caller's scope.

## Establish The Comparison

Inspect the worktree and separate committed framework state from pending changes. Record:

- The fork revision being compared and the original import's source revision, if evidenced.
- The historical upstream comparison revision and what its previous port actually covered.
- The fixed upstream target for this cycle and the separately observed remote HEAD.

An extracted repository has an independent history. A comparison pin, import date or previous
candidate queue does not establish a fully synchronized upstream base. Report an unknown source
revision as unknown. Use exact trees for comparison; use ancestry only within verified history.
When a historical target covered only selected ports, use the content-verified import anchor to
reconstruct earlier differences as well. A later-only upstream range would miss inherited gaps.
The final fork-to-target reconciliation must include those earlier omissions.

Check object availability and shallow boundaries before interpreting missing commits:

```text
git rev-parse --is-shallow-repository
git cat-file -t REVISION
git show --no-patch --format=fuller REVISION
git rev-list --parents -n 1 REVISION
git ls-remote zed-upstream HEAD
```

Fetch only missing comparison objects or a bounded history needed for provenance. A partial clone
may fetch blobs while running a diff. Avoid interpreting shallow `git log`, `blame` or ancestry
failures as proof that a fix never existed. Keep the cycle's target fixed when remote HEAD moves.

## Read Three Differences

Replace the revision placeholders below with the recorded full hashes. Begin with the imported
crate closure from the import record; include relevant root manifest and toolchain changes.

```text
git diff --name-status --find-renames OLD_UPSTREAM TARGET -- crates/gpui crates/gpui_windows
git diff OLD_UPSTREAM TARGET -- crates/gpui/src/view.rs
git diff OLD_UPSTREAM FORK_REVISION -- crates/gpui/src/view.rs
git diff FORK_REVISION TARGET -- crates/gpui/src/view.rs
git log --no-merges --oneline OLD_UPSTREAM..TARGET -- crates/gpui/src/view.rs
git show UPSTREAM_FIX -- crates/gpui/src/view.rs
```

The first comparison finds upstream additions; the second explains fork changes; the third shows
the remaining reconciliation. The example paths are a drill-down, not the complete sync scope.
Expand to platform implementations, traits, consumers, manifests and tests touched by each change.

Map relocated code to its current owner before comparing it. For example, the fork splits Window
state into `window/frame_journal.rs` and other owner files. Account for `open-gpui-*` package names
and `open_gpui_*` Rust paths separately from behavior. Use temporary text normalization only as a
reading aid, retaining the original diff. Large line counts can reflect moved code and fork tests;
they measure neither missing features nor maintenance cost.

Classify each relevant mechanism as missing, equivalent, intentionally divergent, superseded or
outside the framework closure. Record source evidence and an owning verification path. Keep the
mechanism's state separate from the decision to adopt it and its executed qualification. Inspect
the fix's introduction, follow-ups and reverts where history is available. A similar symbol or an
upstream commit title alone does not prove equivalence. Group related changes into complete units
with their prerequisites; preserve upstream regression scenarios when adapting them.

## Integrate Through Existing Owners

Use the fixed target's complete official mechanisms as the default baseline within the framework
closure. Preserve their supported behavior and regression scenarios while adding features and
refactoring into reusable modules. First adjust or replace fork-owned changes to fit that baseline.
A retained difference needs a concrete consumer or Accepted contract, source evidence and an
owning verification path; an unimplemented official mechanism remains queued rather than becoming
an implicit rejection. Keep ordinary public consumers usable without engine-private dependencies.

Respect package/dependency cleanup and in-progress component/docking work. Remove obsolete fork
mechanisms as their successors are qualified. Framework maintenance does not introduce a consumer
engine's document, history or ECS owner.

For the affected unit, trace the full path through notification, layout/prepaint, deferred work,
paint/cache replay, candidate acceptance and native presentation. In this fork, subscriptions,
focus and receipts can commit only with their admitted candidate. Account for nested window draws
and unwind, exact geometry/clip/presentation validity, portals, atlas lifetime and native command
ordering when the change touches them. These contracts require adaptation beyond copying an
upstream function.

Compare dependency and API prerequisites explicitly. Upstream's workspace toolchain is not proof
of a change's minimum compiler requirement. Qualify the independently locked consumer and the
affected backends; keep a compiler-floor change explicit. Use the existing import checks instead
of adding a name resolver, synchronization engine or second dependency-policy scanner.

## Verify And Record

Use the [verification guide](../../../docs/verification.md) for existing commands and platform
claims. Trust unchanged official mechanisms and their upstream regression coverage. Compile the
affected packages and public consumers; run existing regressions for changed behavior and add
tests for fork adaptations, integration seams or concrete defects. Expand to broader cohorts or
platform/compiler matrices only when the affected contract, a failure or a release claim needs
them. Serialize Cargo in the checkout and reuse the caller's configured target.

Where practical, show the regression failing before the owning fix. Preserve failures and actual
rerun results; distinguish source inspection, compilation, headless execution and native submitted
frames. Keep unexecuted OS, input, performance and engine-integration scope visible.

Update the existing candidate queue and plan with upstream provenance, fork disposition, local
contract adaptations, actual results and remaining gaps. A few correctness ports do not complete
the whole refresh. The cycle closes only when the recorded framework differences have dispositions
and its declared verification passes. Record the resulting fork revision without claiming complete
source parity or unexecuted platform qualification. Commit or publish only within the caller's
authorization; the skill supplies no additional external authority.
