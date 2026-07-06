---
type: Repo Convention
title: Engineering wiki memory concurrency convention
tags:
  - engineering-memory
  - concurrency
  - codex
timestamp: 2026-07-04T00:00:00Z
---

# Convention

Treat `docs/knowledge/engineering/current-state.md`, `log.md`, and root `index.md` as rollup views.
They may lag behind the checked-out branch and should not be edited for normal task progress.

For ordinary work:

- write durable implementation progress to a new file under `progress/`;
- write test/build evidence to a new file under `verification/`;
- write handoffs to a new file under `sessions/`;
- write subagent findings to a new file under `subagents/`;
- write quick chronological events to a new file under `logs/`, preferably through
  `wiki_memory.py log`;
- publish active parallel work in `registry/` with one registration file per producer, branch,
  worktree, or delegated agent lane.

Use unique filenames for new shards, usually date plus a specific slug. Do not update another
producer's shard; create a successor shard and cite the older one when needed.

# Rollup Policy

Only refresh shared rollups during an explicit integration pass, typically after pulling or rebasing
`main`, or when a human asks for a consolidated memory refresh. If a rollup conflicts, keep the
sharded concept files as the source of truth and regenerate or manually reconcile the rollup later.

When validating the bundle, warnings about large or stale rollups are migration signals, not task
blockers, as long as new facts are captured in shards and `registry/` exists for active parallel
work.

# Validation

Use:

```powershell
python "$HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py" validate --root docs\knowledge\engineering
```

Expected warnings in this repository may include the historical size of `current-state.md` and
`log.md`. New work should reduce future conflict risk by avoiding those files.
