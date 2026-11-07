# ADR 0023: Open GPUI Live Region And Announcement Authority

**Status**: Accepted
**Date**: 2026-07-21

## Context

Open GPUI had renderer-neutral accessibility roles and a final AccessKit tree, but no complete
contract for live updates. Components could either leave status changes silent or invent timers,
hidden labels, and native speech calls. A window-global notification also had no lifecycle boundary
for activation generations, queue pressure, focus independence, or window teardown.

The platform adapter owns delivery to assistive technology, while GPUI owns the committed tree.
Automatic announcement diagnostics and DevTools histories must not retain ordinary announcement
text. Explicit developer inspection of the published accessibility tree is a separate, content-
bearing operation, as is the platform adapter's delivery of that tree.

## Decision

`open_gpui_ui_core::SemanticDescriptor` owns the renderer-neutral live facts:

- `Role::Status` and `Role::Alert` are non-focusable announcement regions;
- `LivePoliteness::{Off, Polite, Assertive}` is a closed priority value;
- `live_atomic` and the existing `busy` fact are explicit descriptor fields;
- `with_live_text` sets both label and value so the pinned AccessKit adapters receive portable
  announcement content.

The UI Components GPUI adapter maps these fields exactly once. Status defaults to polite and
atomic; Alert defaults to assertive and atomic. An explicit `Off` remains a present AccessKit
`Live::Off` value rather than being treated as absence. Components derive the descriptor from
resolved state during render and never submit a transient queue request automatically.

`Window::announce` is the sole public entry point for an explicitly window-global notification. Its
queue is private to the window's `A11y` authority, has a fixed capacity of 32 pending or retained
nodes, preserves request order, assigns a per-window sequence and synthetic identity, and treats
equal text as a new request. `Accepted` means only that a request entered this bounded queue. A node
that reaches a matching committed accessibility generation is retained until a later matching
generation commits its removal. Deactivation, activation replacement, or window close can clear an
accepted request before publication and records a typed `Cleared` lifecycle. Requests submitted
while inactive or closing, and requests rejected by a full queue, receive typed `Dropped` outcomes;
neither cleared nor dropped requests replay.

GPUI promises that publication occurs only through a matching final `TreeUpdate`; queue admission
does not promise that publication wins a later lifecycle boundary or that an assistive technology
will speak the message. Requests never move focus, add actions, or call a platform speech API.
Diagnostics retain only window, request, sequence, politeness, and lifecycle metadata; they contain
no message, length, hash, summary, or derived text. Window teardown clears pending and retained
payloads at the first closing transition.

### Explicit Published-Tree Inspection

The November 7 upstream reconciliation preserves the complete developer inspection mechanism from
Zed `2268045a119030735f762e5afaf59da0bda869f4`, including raw semantic strings and debug-only
node provenance. `Window::debug_a11y_tree_json` reads the last accepted, published tree without
activating accessibility or triggering layout. Its snapshot is owned by the same Window A11y
authority, replaced only with a matching published tree and cleared on activation replacement,
deactivation, interaction quiescence or close. Candidate construction or refusal cannot replace it.
Its frame metadata distinguishes the accepted frame generation from the diagnostic capture count;
it does not assert native submission or assistive-technology consumption.

This explicit inspection API can contain ordinary labels, values, titles and announcement text
already present in that published tree. It does not reveal masked password data absent from the
tree. An application may display or copy the result in response to an explicit developer action.
GPUI does not automatically log, save, transmit or feed this content to DevTools. The existing
typed announcement metadata and automatic diagnostic/export contracts remain unchanged. This
clarification extends the earlier blanket diagnostic wording rather than substituting a redacted
tree for the official inspection capability.

## Consequences

- Feedback, Toast, Command status, Field errors, Tree loading hints, and VirtualizedList status rows
  use declarative final-tree semantics.
- `CommandStatusItem` requires a caller-provided stable id; status identity is not an array index.
- Resource projections require a caller-owned `ResourceAdapterNamespace`; query keys, mutation ids,
  and user content never become status identity.
- Gallery catalog samples whose feedback is illustrative opt into `LivePoliteness::Off` in their
  resolved state; `EmptyState` remains structural and non-live. The Focus & A11y page demonstrates
  real status, busy, alert, repeated-text, inactive-drop, focus, and privacy behavior.
- DevTools receives only allowlisted live priority, atomicity, and busy metadata. It does not read
  `TreeUpdate` or announcement queue entries.
- Empty stable live regions may be committed before their first non-empty update when an application
  wants update-only behavior.

## Verification

Pure tests cover role defaults, explicit Off, atomicity, busy, and exact descriptor mapping. GPUI
tests cover rollback, deferred/cache replay, presentation suppression, queue capacity, ordered
retention/removal, repeated text, activation generations, two-window isolation, focus independence,
and close. Gallery tests drive the actual controls and final tree. A unique runtime canary appears
in the test platform's committed tree and is absent from typed diagnostics, DevTools capture,
history, diff, export, inspector, report, artifact, and fixtures. Separate explicit-inspection tests
prove that the same published canary is available only through the content-bearing Window reader
and deliberate copy action, while the automatic artifacts remain redacted. Publication rejection,
rollback, lifecycle clearing and debug/release provenance are separate inspection acceptance gates.

## Related Documents

- [UI framework authority convergence plan](../plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md)
- [Semantic accessibility and final-tree authority](../knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md)
- [Open GPUI v0.3 UI migration guide](../ui/migration-v0.3.md)
- [Verification guide](../verification.md)
