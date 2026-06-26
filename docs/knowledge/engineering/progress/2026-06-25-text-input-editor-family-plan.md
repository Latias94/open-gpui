---
type: Work Progress
title: Text input editor family progress
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: feat/table-nested-headers
related_plan: docs/plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md
---

# Summary

- The next component-depth boundary is the text input editor family.
- The plan keeps the current-crate product boundary and does not create a standalone headless text
  editing crate.
- The implementation order is value/display projection helpers, single-line password display,
  separate controlled `Textarea`, and optional Table multiline editor composition after the
  primitive is stable.
- U1 is complete: `TextInput` now has internal value/display projection helpers and offset mapping
  tests for ASCII, emoji, CJK, and ZWJ values.
- U2 is complete in the working tree: `TextInputDisplayMode` supports `Plain` and `Password`,
  password mode masks one glyph per stored grapheme for static and controller-backed rendering,
  `TextInputController` maps display offsets back to stored offsets for hit testing and IME
  geometry, and the Components gallery exposes a password sample.
- U3 is complete in the working tree: `Textarea` is now an official separate multiline form
  editor with `Textarea::value(...).on_change(...)`, newline-preserving callback payloads,
  renderer-neutral `TextareaState`, local textarea viewport scrolling, root/prelude exports,
  component API inventory coverage, Components gallery samples, Field+Textarea composition, and a
  focused gallery smoke for inner-scroll containment.
- U4 is complete in the working tree: `TableCellEditor::MultilineText { rows }` extends the
  renderer-neutral Table editor contract, the GPUI adapter composes fixed-height `Textarea` editors
  for editable leaf cells, `TableCellEditChange` remains the app-owned payload, and the Components
  gallery exposes `multiline-release` with a focused newline-preserving edit smoke.
- Key references are the existing `TextInputController` research, `gpui-component` masked offset
  handling, Fret textarea/editor boundaries, and TanStack app-owned editable table examples.
- The plan intentionally defers password reveal toggles, undo/redo, completion, validation
  engines, rich text, code-editor behavior, dynamic textarea auto-grow, and server mutation
  workflows.

# Verification

- `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`
- `cargo nextest run -p open-gpui-ui-components text_input`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata`
- `cargo nextest run -p open-gpui-ui-components crate_root_and_prelude_exports_remain_explicit component_api_inventory_covers_official_gallery_catalog public_resolved_state_contracts_avoid_gpui_runtime_types gpui_adapter_exports_group_runtime_specific_surfaces`
- `cargo nextest run -p open-gpui-ui-components public_contract_extraction_blockers_match_allowlist adapter_only_public_surfaces_match_allowlist adapter_only_helpers_do_not_leak_from_default_exports`
- `cargo nextest run -p open-gpui-ui-components textarea component_api_inventory crate_root_and_prelude_exports_remain_explicit controlled_textarea_on_change_preserves_newline_input default_textarea_state_uses_text_input_role_and_rows filled_textarea_preserves_newlines_in_state disabled_read_only_and_invalid_textareas_expose_control_state`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata official_component_catalog_entries_have_signals_and_sample_selectors components_gallery_smoke_textarea_scroll_stays_inside_sample`
- `cargo nextest run -p open-gpui-ui-core multiline_text_editor_rows_are_normalized`
- `cargo nextest run -p open-gpui-ui-components table_render_plan_exposes_text_cell_editability_for_leaf_cells_only table_runtime_text_cell_edit_emits_change_without_row_interaction table_runtime_multiline_cell_edit_emits_newline_change_without_row_interaction`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_table_samples_expose_virtualized_row_model_contract components_gallery_smoke_multiline_table_cell_updates_sample_rows`

# Next Action

- Close U5 with final docs/verification validation, then choose the next component-depth boundary.
  Dynamic textarea auto-grow, Table editor row measurement, validation, dirty tracking, and
  commit/cancel workflows remain deferred.

# Citations

[1] [Plan](../../../plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md)
[2] [Text input controller research](../subagents/text-input-controller-research.md)
[3] [Text input patterns research](../subagents/text-input-patterns.md)
[4] [Component contract](../../../ui/component-contract.md)
