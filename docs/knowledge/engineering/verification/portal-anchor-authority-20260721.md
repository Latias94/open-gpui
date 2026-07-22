---
type: Verification Evidence
title: Typed committed portal anchor authority
status: complete
timestamp: 2026-07-21
git_branch: refactor/ui-framework-authority-convergence
verified_by:
  - cargo nextest run --locked -p open-gpui portal_anchor_tests --no-fail-fast
  - cargo nextest run --locked -p open-gpui --test portal_anchor_surface --no-fail-fast
  - cargo nextest run --locked -p open-gpui presentation_tests --no-fail-fast
  - cargo nextest run --locked -p open-gpui --test presentation_surface --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-components --test overlay --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-components --test window_overlay_runtime --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-components --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery --no-fail-fast
  - cargo run --locked -p xtask -- scan-theme-drift
  - cargo run --locked -p xtask -- scan-theme-schema
  - cargo run --locked -p xtask -- scan-ui-contract
  - cargo run --locked -p xtask -- scan-doc-links
  - cargo fmt --all -- --check
  - git diff --check
---

# Verification

- Added a window-bound `PortalAnchorHandle` whose committed snapshot carries final-frame bounds,
  presentation, and generation. A follower reads only committed geometry and never retains a
  last-known rectangle after the target unlinks.
- The target registers its final transformed and scrolled bounds during prepaint. Deferred
  followers resolve and prepare their window-space child in one deferred round from the same
  committed authority, while independent cached follower views are invalidated when their target
  dependency may have changed.
- GPUI treats `Inert` anchors as linked geometry. The UI Components overlay runtime requires a
  `Visible` anchor, dismisses uncontrolled followers on unlink, and sends exactly one
  `DismissReason::AnchorUnlinked` intent to a controlled owner.
- Popover, Select, Combobox, HoverCard, Menu, ContextMenu, and Tooltip use the typed anchor path.
  Window-point ContextMenu placement and full-window Dialog or Sheet placement intentionally use
  explicit window-portal modes instead.

# Tests Added Or Tightened

- Core tests cover same-frame binding, transforms, scroll, presentation, unlink, rebind,
  wrong-window use, immutable registration mode, cached official followers, and custom deferred
  resolvers. A chain that completes on the tenth deferred round proves the depth boundary is
  inclusive without weakening runaway-cycle detection. Target-root transform and presentation
  wrappers are also verified in both builder orders. A tracker outside a cached `AnyView` retains
  the rendered root's transform and Inert presentation across consecutive frames, and stable
  cached followers prove their cross-view dependency is renewed every frame.
- UI integration tests cover one target with multiple followers, stable generations while moving,
  controlled and uncontrolled unlink behavior, repeated-frame deduplication, explicit reopen,
  initial closed Tooltip binding, hidden or disabled deferred-surface suppression, a four-level
  submenu chain with one deferred round per follower, and root-only `AnchorUnlinked` reason
  supersession without disturbing an existing descendant close intent. A transformed runtime
  surface proves outside-press arbitration consumes visible displayed geometry rather than raw or
  clipped-away layout bounds.
- The Gallery Overlay page exposes a real scrollable two-follower flow with move, hide, unmount,
  mount, and reopen controls. Its headless test compares committed target and follower movement and
  verifies unlink callback counts.

# Public Contract

- Create a handle with `Window::new_portal_anchor`, bind it with
  `PortalAnchorExt::track_portal_anchor`, and place a detached follower with
  `portal_anchor_follower`.
- Keep a handle stable for the lifetime of one logical target. Replacing an external handle for an
  existing overlay registration is an error rather than an implicit retarget.
- A controlled standalone Tooltip must commit the intent delivered through `on_open_change`.
  Merely receiving `AnchorUnlinked` does not mutate caller-owned state.
- Use window-point or full-window overlay registration when placement is intentionally independent
  of an element subtree.

# References

- Architecture decision: `docs/adr/0024-open-gpui-typed-committed-portal-anchor-authority.md`
- Component contract: `docs/ui/component-contract.md`
- Migration guide: `docs/ui/migration-v0.3.md`
- Core implementation: `crates/gpui/src/window/portal_anchor.rs`,
  `crates/gpui/src/elements/portal_anchor.rs`
- Overlay integration: `crates/ui_components/src/overlay/window_runtime/surface.rs`,
  `crates/ui_components/src/tooltip.rs`
