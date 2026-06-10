---
title: Refactor Canvas Tool Gesture Session Semantics
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Gesture Session Semantics

## Summary

Tool switching, custom tools, persistence, and relation no-op detection should all use one coherent
gesture/session mutation model. Active transient document changes must either commit through the
journal or cancel back to the baseline; they must never remain in memory without history or log
facts.

---

## Problem Frame

`CanvasEditor::set_tool` currently resets the tool state without resolving an active gesture. If a
drag has already applied transient document updates, switching tools can leave those changes in the
document while skipping undo/persistence. `begin_gesture` can also overwrite an existing baseline.

Custom tools have the opposite problem: they can only return `ApplyTransaction`, so pointer-move
updates become many undo/log entries instead of one coalesced gesture commit. The internal gesture
effects are intentionally crate-private, but the public custom-tool vocabulary still needs a stable
transaction-session interface.

Two adjacent journal issues should be closed in the same pass: relation order-only changes should
not produce semantic diffs, and log entries with partial committed facts must not be reported as
complete committed mutations.

---

## Requirements

- R1. Switching tools with an active gesture must cancel the gesture and restore the baseline before
  changing tools.
- R2. Starting a gesture while one is active must preserve the original baseline instead of
  overwriting it.
- R3. Public custom tools must be able to begin, update, commit, and cancel a transient transaction
  session without seeing `ToolState` or `CanvasToolEffect`.
- R4. Persistent public tool intents must log only the committed session mutation, not every update.
- R5. Relation equality must be semantic, not Vec-order based, so remove+add of the same relation is
  a no-op.
- R6. Persistence log kind must distinguish complete committed mutation facts from partial legacy
  committed facts.

---

## Key Technical Decisions

- KTD1. Change `CanvasEditor::set_tool` to return `Result<(), DocumentError>` because canceling an
  active gesture can fail during rollback validation.
- KTD2. Name the public custom-tool session intents around transient transactions, not internal
  tool state: `BeginTransientTransaction`, `UpdateTransientTransaction`,
  `CommitTransientTransaction`, and `CancelTransientTransaction`.
- KTD3. Keep `ToolState` and `CanvasToolEffect` crate-private; public intents map into the same
  editor/persistence gesture journal path.
- KTD4. Implement semantic equality on `CanvasRecordRelations`; do not rely on serialized order as
  document meaning.
- KTD5. Extend `CanvasLogEntryKind` with a partial committed state rather than pretending older
  record-only committed logs have complete relation facts.

---

## Implementation Units

### U1. Resolve active gestures on tool switching

- **Files:** `crates/canvas/src/tool.rs`, examples using `CanvasEditor::set_tool`.
- **Test scenarios:** Switching tools during an uncommitted node move restores the original node,
  keeps undo depth unchanged, updates runtime hit-test state, and then changes the active tool;
  repeated begin gesture calls preserve the first baseline.

### U2. Expose public transient transaction intents

- **Files:** `crates/canvas/src/tool.rs`, `crates/canvas/src/persistence/store.rs`,
  `crates/canvas/README.md`, examples.
- **Test scenarios:** Applying public begin/update/commit intents produces one undo entry; applying
  the persistent public intents produces one log entry; cancel restores baseline without history or
  log.

### U3. Make relation no-op detection semantic

- **Files:** `crates/canvas/src/relations.rs`, `crates/canvas/src/document.rs` tests,
  `crates/canvas/src/persistence/tests.rs`.
- **Test scenarios:** Removing and re-adding the same group relation in one transaction returns an
  empty diff, does not push history, and does not append a persistence log.

### U4. Distinguish partial committed log facts

- **Files:** `crates/canvas/src/persistence/store.rs`, `crates/canvas/src/persistence/tests.rs`,
  `crates/canvas/README.md`.
- **Test scenarios:** Logs with both committed batches are `CommittedMutation`; legacy replay logs
  are `LegacyReplayTransaction`; old record-only committed logs are `PartialCommittedMutation` and
  do not pretend relation facts are complete.

---

## Scope Boundaries

- This plan does not make `ToolState` public.
- This plan does not add CRDT/redb/rkyv adapters.
- This plan does not optimize away full document clone in mutation preparation; that needs a
  separate benchmark-backed design.
