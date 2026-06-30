---
type: Verification Evidence
title: Docking nested inner-edge ImGui alignment verification
status: complete
timestamp: 2026-06-28
git_branch: refactor/docking-viewport-authority-break
git_commit: d8656b670c104ff7d028fee9a2ab467b70e58314
verified_by:
  - cargo test -p open-gpui-docking --lib inner_edge_dock_does_not_cross_opposing_axis_ancestor -- --nocapture
  - cargo test -p open-gpui-docking --lib leaf_edge_plan_wraps_leaf_below_opposing_axis_parent -- --nocapture
  - cargo test -p open-gpui-docking --lib runtime_opened_cross_window_inner_edge_drag_then_re_docks_nested_mixed_axes -- --nocapture
  - cargo test -p open-gpui-docking --lib graph_split_tests -- --nocapture
  - cargo test -p open-gpui-docking --lib runtime_opened_cross_window_inner_edge_drag -- --nocapture
  - cargo test -p open-gpui-docking --lib drop_target::tests::leaf_edge -- --nocapture
  - cargo fmt --all -- --check
  - git diff --check
  - cargo check -p open-gpui-docking
  - cargo nextest run -p open-gpui-docking --no-fail-fast
---

# Verification

- Fixed nested mixed-axis inner-edge docking so dropping on a leaf edge stays scoped to that leaf.
  `DockGraph::edge_dock_plan` now stops ancestor reuse at the first real split boundary if that
  split has the opposing axis, then falls back to wrapping the hit target.
- This matches the ImGui docking path: `DockNodeTreeFindVisibleNodeByPos` selects the visible node
  under the pointer, inner preview stores `data->SplitNode = host_node`, delivery queues that split
  node, and `DockNodeTreeSplit(ctx, node, ...)` splits the queued target node.
- The user-reported repro was a tab dragged from a child window into the main window's lower-right
  area, aiming at that area's left edge. The previous plan crossed the lower-right area's vertical
  parent and inserted beside the whole right region; the fixed plan wraps the lower-right leaf with
  a local horizontal split.

# Tests Added Or Tightened

- `graph_split_tests::inner_edge_dock_does_not_cross_opposing_axis_ancestor` covers horizontal root
  plus right-side vertical subtree for left/right leaf-edge drops.
- `graph_split_tests::inner_edge_dock_does_not_cross_opposing_axis_ancestor_mirrored` covers the
  mirrored vertical root plus bottom horizontal subtree for top/bottom leaf-edge drops.
- `drop_target::tests::leaf_edge_plan_wraps_leaf_below_opposing_axis_parent` verifies the layout
  resolver carries a `WrapTarget` edge plan for a leaf under an opposing-axis parent.
- `host_viewport_runtime_handle_tests::runtime_opened_cross_window_inner_edge_drag_then_re_docks_nested_mixed_axes`
  now asserts the second-stage drop wraps the target leaf inside the nested split instead of adding
  a new root sibling.

# References

- Implementation commit: `d8656b670c104ff7d028fee9a2ab467b70e58314`
- Local fix: `crates/gpui_docking/src/graph_edge_dock.rs`
- Main regression tests: `crates/gpui_docking/src/graph_split_tests.rs`,
  `crates/gpui_docking/src/drop_target.rs`,
  `crates/gpui_docking/src/host_viewport_runtime_handle_tests.rs`
- ImGui reference points: `repo-ref/imgui/imgui.cpp:20508`,
  `repo-ref/imgui/imgui.cpp:20008`, `repo-ref/imgui/imgui.cpp:21470`,
  `repo-ref/imgui/imgui.cpp:18410`
