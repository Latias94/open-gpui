---
type: "Current State"
title: "Current Engineering State"
description: "Short durable summary of the active engineering state."
tags: ["engineering-memory"]
timestamp: 2026-06-16T22:53:21Z
status: "active"
---

# Current State

## 2026-06-18

- Done: Restored overlay menu/context-menu sample-owned `focused_value` metadata so controlled examples keep their requested initial focus in the sample struct, and the gallery shell now consumes that request value when present while falling back to resolved state when it is absent.
- Last verified: `cargo fmt --all --check` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the overlay sample-contract cleanup.
- Next action: wait for the remaining gallery drift review; if no stronger seam appears, commit the overlay contract alignment.

- Done: Moved the gallery left navigation off the ad hoc `navigation_scroll` handle and onto `ScrollArea` scroll semantics, so the shell no longer owns a second manual scroll path alongside page scrolling.
- Last verified: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the navigation-scroll cleanup.
- Next action: keep the architecture loop narrow unless a new evidence-backed duplication seam appears; otherwise move to the next product slice.

- Done: Moved the gallery page scroll reset off the `GalleryShell` ad hoc `page_scroll` handle and onto `ScrollArea` reset-key semantics, so page switching now reuses the same scroll contract the component stack already uses for inner scroll views.
- Last verified: `cargo fmt --all --check` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the page-scroll cleanup.
- Next action: keep the architecture loop narrow unless a new evidence-backed duplication seam appears; otherwise move to the next product slice.

- Done: Deleted the remaining `Select` helper wrapper that only forwarded `selected` / `active` values into `Listbox`; the render path now applies those values inline.
- Done: Added direct tests that lock `Menu` / `ContextMenu` default open focus to the first focusable item, so the shared entry-focus rule is now covered by the component test suite.
- Last verified: `cargo fmt --all --check`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed after the Select / Menu cleanup.
- Next action: keep the architecture loop narrow unless a new evidence-backed duplication seam appears; otherwise move to the next product slice.

- Done: Rechecked the live gallery seams against `repo-ref/fret`'s `entry_focus` pattern. `Menu` / `ContextMenu` still do not have a deeper shared-rule seam worth extracting in this pass because the current code does not model modality as a separate public input; the current `first_focusable_value()` helper is the right stopping point for now.
- Done: Confirmed again that `TabsSample.title` is page-card copy, not duplicated resolved state, and that the remaining overlay titles / descriptions / action labels are still constructor inputs or display content.
- Next action: stop the seam hunt unless a new evidence-backed duplication appears; otherwise move to the next product slice and keep the current architecture pass narrow.

- Done: Rechecked the live gallery seams against `repo-ref/fret` and the current code. `Menu` / `ContextMenu` first-focus handling is still the only clear shared-rule seam; `ScrollAreaState`, `ListboxState`, `SelectState`, `ComboboxState`, and `CommandState` are already deep enough for this pass.
- Done: Confirmed that `TabsSample.title` is page-card copy, not duplicated resolved state. The remaining overlay titles, descriptions, and action labels are still constructor inputs or display content, so they should not be deleted just to move string literals around.
- Next action: continue the architecture loop only if a new evidence-backed duplication seam appears; otherwise move to the next product slice and stop chasing shallow helpers.

- Done: Removed the one-off `apply_optional_values` helper from `Combobox` and inlined the selected/active propagation, so the popup listbox setup now stays local to the render path.
- Done: Added `combobox_state_scrollable_content_tracks_filtered_option_count()` to lock the filtered-option scroll contract alongside the existing `ListboxState::scrollable_content()` threshold test.
- Next action: keep scanning for true ownership splits only; the `Tabs` / `Toolbar` / `Sidebar` item arrays are builder inputs, not duplicate state, so do not chase them as deletion seams.

- Done: Added `ListboxState::scrollable_content()` so `SelectState` and `ComboboxState` read the same listbox-owned overflow threshold instead of duplicating `> 6` checks.
- Last verified: `cargo fmt --all --check`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed after the listbox scrollability cleanup.
- Next action: keep the architecture loop narrow; the current subagent review says the remaining `apply_optional_values` builder sugar is not deep enough to extract, so move on unless a new evidence-backed seam appears.

- Done: Used the `MenuState::first_focusable_value()` seam to remove the duplicated local helper from `Menu` / `ContextMenu`. The gallery now reads the same behavior contract from state on both paths, so entry-focus selection is owned by the menu state instead of two call-site copies.
- Last verified: `cargo fmt --all --check`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed after the Menu / ContextMenu entry-focus cleanup.
- Next action: continue the architecture loop only if a new evidence-backed duplicate seam appears; otherwise move to the next product slice.

- Done: Used `repo-ref/fret` as the local architecture reference for the current pass. The durable takeaway is to keep public entry points thin and move behavior math into owned state/helper seams only when that removes duplicate policy.
- Done: Added `ListboxState::standalone_options()` and `ListboxState::group_options()` so the gallery shell no longer filters `ListboxOptionState::group_index()` directly for Listbox / Select / Combobox reconstruction.
- Done: Removed the now-unused Combobox option helper in the gallery shell and kept command reconstruction on the explicit `CommandState` standalone/grouped views.
- Done: Captured the subagent architecture review in [Gallery architecture review 2026-06-18](subagents/gallery-architecture-review-20260618.md). The accepted next candidate is shared `Menu` / `ContextMenu` entry-focus handling; `ScrollAreaState`, `ListboxState`, `SelectState`, `ComboboxState`, and `CommandState` are deep enough for the current pass.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed.
- Next action: continue the architecture loop on `Menu` / `ContextMenu` entry-focus only if it removes duplicate branching; otherwise pause for the next product slice.

- Done: Read `repo-ref/fret` as the local reference repo for this pass and confirmed the useful pattern is a thin entry point at the edge, a real implementation crate underneath, and pure helper layers for visibility / overflow / viewport math. `crates/fretboard/src/diag.rs` is just a forwarder; the substantive logic lives in `crates/fret-diag`, and the scroll/viewport pattern in `fret-ui` is the model to borrow when we deepen scrolling later.
- Done: Confirmed the gallery does not need to re-derive scroll-area policy. `ScrollAreaState` already owns the relevant reset/axis/viewport decisions, so future scroll work should stay on that seam instead of growing more gallery-local boolean logic.
- Done: Continued the gallery contract cleanup by making the command palette's synthetic standalone group explicit in resolved state and adding iterator views for standalone items, grouped groups, and group items. The shell now reconstructs the command UI from those state views instead of splitting on a local magic-string seam.
- Done: Re-reviewed the overlay menu/context-menu contract and kept sample-owned `focused_value` metadata in the current shell implementation, because controlled reconstruction needs the original request value instead of reading it back from resolved state.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed.

- Done: Re-reviewed the gallery Components/Overlay sample-state contract surface and confirmed there is no fresh evidence-backed deletion seam beyond the already-cleaned command standalone group and the current resolved-state ownership split. One earlier `TextInputSample.controller_driven` deletion attempt was rolled back after confirming that field is still the sample-side controller mount switch.
- Done: Kept the Components gallery automation green after the review pass. The current verification set still passes: `cargo fmt --all`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`.
- Next action: keep searching only for real ownership splits that can be deleted or moved into resolved state without losing gallery-specific behavior.
- Done: Read `repo-ref/fret` as the local reference repo for the current architecture pass. The useful pattern is a thin forwarder at the edge, a real implementation crate underneath, and headless pure helpers for viewport/visibility/overflow decisions.
- Done: Confirmed that `ScrollAreaState` is already a reasonably deep seam. It owns `viewport_id`, axis, size, reset policy, reset key, and the `should_reset_for_key_change` / `scrolls_x` / `scrolls_y` decisions, so the gallery should keep using it instead of re-deriving scroll-area policy in the shell.
- Done: Verified the current gallery contract cleanup is green after the latest state-first refactor. `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`, `cargo check -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-components --tests` all passed.
- Next action: if we deepen the scroll story further, do it as a pure helper for viewport containment / overflow membership / scroll-into-view math, not as another gallery-local boolean maze.

- Done: Continued the Components gallery architecture pass by making the command palette's synthetic standalone group explicit in resolved state (`CommandGroupState::standalone()`), then switching the gallery shell away from the `commands`/`Commands` magic-string seam.
- Done: Re-reviewed the low-state primitives (`Separator`, `Kbd`, `Progress`, `Skeleton`, `Avatar`) and found no further deletion value in moving their remaining display copy into state; the visible copy there is already the right level of surface metadata.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed after the command seam cleanup.
- Next action: keep scanning the Components page for the next real deletion seam, but prefer only changes that remove an actual contract split rather than renaming display fields.


- Done: Removed the redundant sample-side `open_mode` fields from the Overlay gallery samples
  (`HoverCard`, `Popover`, `Dialog`, `AlertDialog`, `Sheet`, `Menu`, and `ContextMenu`) and made
  `shell.rs` read `state.open_mode()` for controlled/uncontrolled reconstruction. The overlay
  sample structs now carry only resolved state plus display metadata. Menu and ContextMenu item
  lists are now reconstructed from `MenuState` / `ContextMenuState` instead of carrying a second
  sample-side descriptor tree, so the shell no longer keeps duplicate ownership or item sources.
- Last verified: `cargo fmt --all --check` and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_hover_card_samples_expose_interactive_hover_contracts
  overlay_page_popover_samples_expose_controlled_and_dismissal_contracts
  overlay_page_dialog_samples_expose_modal_and_close_contracts
  overlay_page_alert_dialog_samples_expose_critical_action_contracts
  overlay_page_sheet_samples_expose_edge_and_policy_contracts
  overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts
  components_page_samples_expose_component_metadata
  components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation
  components_gallery_smoke_closes_select_popup_from_outside_press
  components_gallery_smoke_closes_combobox_popup_from_outside_press
  components_gallery_smoke_closes_command_popup_from_outside_press
  official_component_catalog_entries_have_signals_and_sample_selectors`.
- Next action: keep looking for the next real sample/state duplication seam only if evidence
  shows it still buys locality or test leverage.

- Done: Removed the last pure Components-gallery helper wrappers from the test file and folded the
  gallery mount flag into a single module constant in `shell.rs`. The remaining scroll helper now
  carries the full visibility contract inline at the one call site that needs vertical containment,
  and the shell no longer repeats `false` as three separate locals.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`,
  and full `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` with 43/43 passing.
- Next action: keep the architecture pass moving only if a new evidence-backed asymmetry appears;
  otherwise move to the next visible sample/state seam.

- Done: Removed the redundant sample-side `open_mode` fields from the overlay gallery samples
  (`HoverCard`, `Popover`, `Dialog`, `AlertDialog`, `Sheet`, `Menu`, and `ContextMenu`) and made
  `shell.rs` read `state.open_mode()` for controlled/uncontrolled reconstruction. The overlay
  sample structs now carry only resolved state plus display metadata, so the shell no longer keeps
  a second open-ownership source.
- Last verified: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery
  --tests` with 43/43 passing.
- Next action: keep the architecture pass moving only if a new evidence-backed asymmetry appears;
  otherwise move to the next visible sample/state seam.

- Done: Tightened the Components gallery mount policy so the Select / Combobox / Command samples
  no longer mount in an open state during the gallery shell render. The page now keeps those
  transient surfaces closed on mount, while the resolved component state still remains visible in
  the state rows. This restored page scrolling and fixed the gallery smoke regressions that were
  failing on short-vs-tall viewport navigation.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`,
  and full `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` with 43/43 passing.
- Next action: decide whether to keep refining the gallery adapter policy labels or move on to the
  next visible contract seam.

- Done: Removed the redundant sample-side `open_mode` fields from the Select / Combobox / Command
  gallery samples and made `shell.rs` read `state.open_mode()` for mount policy. The gallery shell
  still keeps those surfaces closed on mount, but the source of truth now lives with the resolved
  state instead of the sample shell. The component conformance test also dropped the pure
  `official_component_sample_selectors()` wrapper and now iterates the canonical selector source
  directly.
- Last verified for the follow-up cleanup: `cargo fmt --all --check`, focused
  `cargo nextest run -p open-gpui-ui-foundation-gallery official_component_catalog_entries_have_signals_and_sample_selectors components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation --no-capture`, and full `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` with 43/43 passing.
- Next action: continue the architecture pass on overlay/menu and the remaining sample/state seams
  only if new evidence appears.

- Done: Read `repo-ref/fret` as the local reference repository. The main takeaways are that
  `crates/fretboard/src/diag.rs` is only a thin public-CLI forwarder, the real diagnostics
  implementation lives in `crates/fret-diag`, and viewport-aware scroll handling lives in
  `crates/fret-ui/src/declarative/host_widget.rs` with coverage in
  `crates/fret-ui/src/tree/tests/scroll_into_view.rs`. The reuse pattern is stable `test_id`
  targeting plus explicit viewport containment checks before scrolling.
- Next action: apply that reference pattern back to the gallery helpers and continue the contract
  cleanup only where there is still evidence of drift.

- Done: Continued the gallery contract pass by removing the redundant sample-side open-state
  fields from the Components page and keeping select/combobox/command mount-state behavior as a
  gallery-local adapter policy. The gallery shell no longer stores a duplicate `page_load_open`
  field; the page tests now only assert the resolved state while the shell keeps those popups
  closed on mount.
- Last verified for the open-state cleanup: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery` with 43 passing tests.
- Next action: continue the architecture pass only if a new evidence-backed asymmetry appears;
  otherwise keep tightening the current contract surface.

- Done: Continued the gallery contract pass by deleting the duplicated Sidebar sample-side field
  set, adding `size` to the resolved `SidebarState` contract, and making the gallery shell render
  sections/items from `SidebarState.sections()` and `SidebarState.items()` instead of a second
  sample tree. The vertical Splitter sample now starts in a real collapsed state and the gallery
  smoke proves it can restore and keep resizing after a second drag.
- Last verified for the Sidebar/Splitter cleanup: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata
  components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition
  components_gallery_smoke_scroll_area_samples_scroll_inside_page
  components_gallery_smoke_sidebar_long_navigation_scrolls_inside_sample`.
- Next action: continue the architecture pass on the remaining sample/state seams only if a new
  evidence-backed asymmetry appears; otherwise keep tightening the current contract surface.

- Done: Inspected `repo-ref/fret` as the reference repo for diagnostics and scroll automation.
  Key finding: `crates/fretboard/src/diag.rs` is only a thin CLI forwarder; the actual diagnostics
  implementation lives in `crates/fret-diag`, while viewport-aware scroll handling lives in
  `crates/fret-ui/src/declarative/host_widget.rs` (`scroll_viewport_bounds` and
  `scroll_handle_into_view`) and the no-drift coverage lives in
  `crates/fret-ui/src/tree/tests/scroll_into_view.rs`. The automation model is stable
  `test_id`-based selection plus explicit viewport containment checks before scrolling.
- Next action: use that pattern as the reference for our gallery test helpers instead of growing
  more wheel-event loops and ad hoc visibility checks.

- Done: Continued the Components gallery contract pass by making `CommandSample` loading metadata
  the single source of truth for both the sample and its resolved `CommandState`. The gallery no
  longer reconstructs the loading state from the `query == "deploy"` sentinel; popup mount/open
  state is now derived directly from each sample's resolved component state.
- Last verified for the Command loading cleanup: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, focused Components gallery nextest coverage for
  metadata/choice/search/scroll reset, `cargo nextest run -p open-gpui-ui-foundation-gallery`, and
  `git diff --check`.
- Done: Continued the Sidebar gallery contract pass by deleting the duplicated sample-side
  section tree. `SidebarSample` now keeps only display metadata plus resolved `SidebarState`, and
  `shell.rs` rebuilds section/item rendering from `SidebarState.sections()` and
  `SidebarState.items()` instead of reading a second sample structure.
- Last verified for the Sidebar contract cleanup: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, focused Sidebar nextest coverage, and full `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
- Next action: continue the architecture pass on Sidebar/ScrollArea and only change page-level
  state when it removes a real contract split.

- Done: Closed the overlay gallery scroll/navigation regression by stopping the gallery shell from
  auto-expanding uncontrolled overlay previews that interfere with page interaction. The sample
  contracts still keep `default_open` metadata, but the gallery now leaves those previews closed
  so the page can scroll and navigation can switch cleanly.
- Last verified: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery
  --tests` with 43 passing tests.
- Next action: only revisit if a new overlay sample regresses hit-testing or page scroll.

- Done: Continued the overlay gallery contract cleanup by moving menu and context-menu initial
  focused-item intent into explicit sample metadata. `MenuSample` and `ContextMenuSample` now
  carry `focused_value`, and the gallery shell feeds that value into the rendered menu builders
  instead of reconstructing it from resolved state.
- Last verified for the focused-value cleanup: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts`.
- Next action: stop the overlay seam hunt unless a new evidence-backed asymmetry appears; the
  remaining controlled branches are explicit sample ownership, not shell inference.

- Done: Continued the overlay ownership cleanup by adding explicit `open_mode` metadata to the
  hover-card, popover, dialog, alert-dialog, and sheet samples, and by keeping `Command`'s loading
  metadata sample-owned instead of shell-inferred. The gallery shell now routes these cases from
  sample contracts instead of `sample.id` branches.
- Last verified for the overlay and command ownership cleanup: `cargo fmt --all`, focused
  `cargo nextest run -p open-gpui-ui-foundation-gallery` checks for hover-card, popover, dialog,
  alert-dialog, sheet, and command contract tests, full `cargo nextest run -p
  open-gpui-ui-foundation-gallery` with 43 passing tests, and `cargo check -p
  open-gpui-ui-foundation-gallery --tests`.
- Next action: stop the seam hunt unless a new evidence-backed asymmetry appears; the remaining
  Sidebar and choice/search cases are state-bearing sample pages, not a missing abstraction.
- Done: Continued the overlay gallery ownership cleanup by adding explicit `open_mode` metadata to
  the hover-card, popover, dialog, alert-dialog, and sheet samples. The gallery shell now routes
  controlled versus uncontrolled behavior from sample-owned contract fields instead of inferring
  those cases from `sample.id`.
- Last verified for the overlay ownership cleanup: `cargo fmt --all`, focused `cargo nextest run
  -p open-gpui-ui-foundation-gallery` checks for hover-card, popover, dialog, alert-dialog, and
  sheet contracts, full `cargo nextest run -p open-gpui-ui-foundation-gallery` with 43 passing
  tests, and `cargo check -p open-gpui-ui-foundation-gallery --tests`.
- Next action: stop the seam hunt unless a new evidence-backed asymmetry appears; the remaining
  Sidebar and choice/search cases are currently state-bearing shell samples, not a missing
  abstraction.
- Done: Continued the component productization pass by removing the last implicit `sample_id`
  branching from the overlay gallery shell for menu and context-menu open ownership. The overlay
  sample data now carries explicit `open_mode` metadata, and the shell routes controlled versus
  uncontrolled behavior from that sample-owned contract instead of inferring it from ids.
- Last verified for the overlay open-mode cleanup: `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts`, and full `cargo nextest run
  -p open-gpui-ui-foundation-gallery` with 43 passing tests.
- Next action: stop the current seam hunt unless a new evidence-backed asymmetry appears; the
  Sidebar and choice/search pages now read as sample/value shells rather than a missing deep
  abstraction.
- Done: Completed the latest overlay and gallery contract cleanup by moving menu and context-menu
  initial focus intent into explicit sample metadata, deleting a leftover overlay helper import,
  and centralizing stable labels through `as_str()` on the core and component vocabularies.
- Last verified for the cleanup: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`,
  `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
- Next action: continue the architecture pass only if a new seam clearly earns it; the most likely
  follow-up candidate is the Sidebar plus choice/search family.
- Done: Completed the overlay gallery sample-contract cleanup by moving initial focused-item intent
  into explicit `MenuSample` and `ContextMenuSample` metadata, so the shell no longer reconstructs
  menu focus from closed runtime state. The same pass also removed duplicate overlay label helpers
  by routing stable labels through `as_str()` on the core/component vocabularies.
- Last verified for the overlay contract cleanup: `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo check -p open-gpui-ui-components --tests`, and
  `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery` with 211 passing tests.
- Done: Continued the overlay gallery architecture pass by moving `MenuSample` and
  `ContextMenuSample` initial focused-item intent into explicit sample metadata, so the shell now
  reads the sample-owned `focused_value` field instead of reconstructing intent from resolved
  runtime state.
- Last verified for the overlay sample intent cleanup: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts`.
- Done: Continued the gallery architecture pass by adding `as_str()` labels to the core adaptive
  and sizing vocabularies (`Density`, `DeviceAdaptiveClass`, `PanelAdaptiveClass`, and
  `DeviceShellMode`) and switching the Sizing & Density gallery page plus the gallery shell to
  derive labels from the vocabulary itself instead of duplicate page-local label tables.
- Last verified for the vocabulary label cleanup: `cargo fmt --all`, `cargo nextest run -p
  open-gpui-ui-core --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`, and
  `cargo nextest run -p open-gpui-ui-components --tests`.
- Done: Continued the gallery architecture pass by extracting a thin `gallery_card_shell` helper in
  `examples/ui-foundation-gallery/src/shell.rs`. The Components catalog and low-state primitive
  sample cards now reuse the shared shell instead of repeating the same rounded/bordered/padded
  wrapper inline.
- Done: The `gallery_card_shell` helper deliberately stays thin: it only owns `id`, optional
  `debug_selector`, and the common white card chrome. The component-specific sample content and
  state rows still live at the call site.
- Done: Added `overlay_sample_card_shell` on top of `gallery_card_shell` so the overlay sample cards
  share the same outer shell and text styling while their trigger and dismissal logic stay local.
- Last verified for the shell helper extraction: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-components
  --tests`.
- Done: Moved the Components gallery catalog state-label fallback and status badge color mapping
  into `pages/components.rs`, so `shell.rs` now renders catalog entries from catalog-owned display
  helpers instead of re-deriving status presentation logic inline.
- Last verified for the catalog presentation cleanup: `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-components
  --tests`.
- Done: Deepened `ComponentCatalogEntry` with stable `sample_selector` metadata for official
  gallery entries, so the Components gallery smoke no longer rebuilds official sample selectors
  from sample constructors.
- Done: The Components gallery conformance test now derives the official selector set directly
  from `COMPONENT_CATALOG` and asserts that non-official entries do not carry sample selectors.
- Last verified for the catalog-metadata pass: `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo run
  -p xtask -- verify`.
- Done: Unified gallery sample debug selectors so the Components and Overlay pages derive stable
  selector strings from sample-owned helpers instead of repeating family prefixes inline in the
  shell and tests. The Components gallery smoke now derives the official sample selector list from
  the sample builders and checks the visible catalog against the rendered page.
- Last verified for the selector unification pass: `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo check -p open-gpui --tests`, `cargo check -p
  open-gpui-ui-components --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo run -p xtask -- verify`.
- Done: Finished the follow-up ownership cleanup and kept the selector contract unified through the
  overlay cards and gallery smoke tests. The pass now also has `cargo nextest run -p open-gpui
  --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`, `cargo fmt --all --check`,
  and `git diff --check` green.
- Done: Deepened the Components gallery catalog so official sample selector metadata now lives on
  `COMPONENT_CATALOG` and the gallery smoke derives its official selector pairs from that single
  source of truth. Non-official catalog entries stay explicit and do not declare sample selectors.
- Last verified for the catalog metadata deepening pass: `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-components --tests`.

- Done: Began the second behavior-alignment loop after commit `ea9ffbc`. `ProgressState` now
  exposes `ProgressVisualMode`, determinate indicator fractions, and indeterminate indicator
  fractions; the GPUI renderer uses a short non-percentage segment for indeterminate progress
  instead of a fixed 33% left-anchored fill.
- Done: Component runtime tests now expose `progress:{id}:indicator` selectors and compare rendered
  indicator bounds for determinate versus indeterminate progress. The gallery progress state row
  reports indicator start/width for manual dogfood.
- Done: Overlay gallery ContextMenu smoke now covers both real right-click opening plus Escape
  dismissal and a second right-click open plus outside-press dismissal.
- Done: `open_gpui::VisualTestContext` now exposes `debug_selector_is_focused` and
  `focused_debug_selector`, backed by test-only selector-to-focus-handle data recorded during GPUI
  painting. This gives runtime smoke tests a stable way to assert the actual focused element.
- Done: Popover and Dialog GPUI adapters now bind persistent trigger/content or trigger/surface
  focus handles, apply the neutral overlay focus-restore intent on dismissal, and keep Popover's
  default initial-focus intent aligned with the non-modal overlay default (`None`). Dialog opening
  now moves focus to its surface by default, while Popover keeps focus on its trigger unless a
  caller explicitly requests content focus.
- Done: Overlay gallery smoke now opens the controlled Popover and Dialog through their real
  component triggers, asserts Dialog initial focus, and verifies Popover/Dialog focus restoration
  after outside press, modal barrier dismissal, and Escape dismissal.
- Last verified for the second behavior-alignment loop: focused `cargo nextest run -p
  open-gpui-ui-components progress_state_clamps_values_and_preserves_indeterminate_mode
  low_state_primitives_render_stable_debug_selectors`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery
  overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses`, focused focus-restore
  commands for `open-gpui`, `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`, and
  `cargo run -p xtask -- verify`.
- Done: Started and executed
  `docs/plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md` to align the
  current component product surface with the stricter adapter-first contract.
- Done: Adapter-only GPUI helpers now stay off the default crate root and prelude. Concrete runtime
  helpers such as `TextInputController`, text-input initialization, overlay scheduling helpers,
  focus-ring shadow painting, a11y conversion helpers, and neutral-to-GPUI geometry conversion are
  grouped under `open_gpui_ui_components::gpui_adapter`.
- Done: `open_gpui_ui_components::text_input` remains the official component module for
  `TextInput`, `TextInputState`, `TextInputColors`, and `TextInputMetrics`; the GPUI-backed
  `TextInputController` implementation now lives behind the internal adapter module and is exposed
  publicly only through `gpui_adapter`.
- Done: Avatar now resolves to neutral `Role::Image`, and the GPUI adapter maps that role to the
  concrete AccessKit image role.
- Done: The Components gallery gained stricter catalog conformance coverage: every official catalog
  entry must have component/state signals when declared, one stable rendered sample selector, and a
  selector family prefix that matches the component name.
- Last verified: `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, full `cargo nextest run -p
  open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` with 207
  passing tests, local engineering-wiki structure validation, `git diff --check`, and
  `cargo run -p xtask -- verify`.

## 2026-06-17

- Done: Completed U1 and U2 of the UI component completion plan
  `docs/plans/2026-06-17-004-feat-ui-component-completion-plan.md`.
- Done: U1 added an official component completion catalog to the Components gallery, made catalog
  entries test-visible through `component-catalog:{name}` debug selectors, and documented the
  official-component checklist in `docs/ui/component-contract.md` and `docs/verification.md`.
- Done: U2 closed rendered runtime coverage gaps in `open-gpui-ui-components`: standalone
  `TextInput` now has a controller-backed rendered input smoke; `Combobox` now has filtered
  keyboard open/select coverage; dialog-backed `Command` now opens, filters, selects, closes on
  Escape, and closes on outside press without leaving modal content mounted.
- Done: U3 added low-state primitive components `Separator`, `Kbd`, `Progress`, and `Skeleton`.
  Each has resolved state, metrics, token intents, explicit root/prelude exports, stable rendered
  debug selectors, and focused component tests. `Role::Separator` is now part of UI core; the
  current GPUI adapter maps it to the nearest available AccessKit role because the bundled
  AccessKit enum does not expose a separator role.
- Done: U4 added the `Avatar` primitive to `open-gpui-ui-components` with resolved display name,
  fallback initials or explicit fallback text, renderer-neutral `AvatarSource` metadata,
  accessible label, size metrics, theme intents, explicit root/prelude exports, stable rendered
  debug selector, component tests, and contract/verification documentation. Image loading, cache
  state, retry policy, fallback delay timers, and AvatarGroup layout remain outside the first
  primitive contract.
- Done: U5 promoted `Separator`, `Kbd`, `Progress`, `Skeleton`, and `Avatar` to official
  Components gallery catalog entries, added visible sample factories and rendered gallery sections
  for each primitive, exposed stable `gallery:component-*-sample:{id}` debug selectors, and extended
  metadata/smoke tests to prove resolved-state rows and short-viewport scrolling still work.
- Last verified for U5: `cargo fmt -p open-gpui-ui-foundation-gallery`, `cargo check -p
  open-gpui-ui-foundation-gallery`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata
  components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation`, and full `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed with 42 tests.
- Next action: Start U6 from the component completion plan: run the broader release gate, refresh
  verification/memory as needed, perform a final review pass, and commit the series closeout.
- Done: Wrote the next UI component completion plan at
  `docs/plans/2026-06-17-004-feat-ui-component-completion-plan.md`.
- Decision: The next component series should define the official-component completion checklist and
  catalog first, then close rendered runtime gaps for existing complex widgets, then add the
  low-state primitives `Separator`, `Kbd`, `Progress`, `Skeleton`, and `Avatar`, then wire the
  gallery and verification gate. Standalone `open-gpui-ui-headless` remains deferred under ADR
  0008.
- Last verified for the plan write: documentation-only self-review plus `git diff --check` for the
  new plan file. No Rust build or tests were run for this planning-only update.
- Next action: Start implementation from U1 of the component completion plan, then proceed through
  U2 runtime gap closure before adding the new primitive batch.
- Done: Added a gallery-level Overlay interaction smoke gate. The gallery now renders the Overlay
  page in `open_gpui::test` and drives controlled Popover outside dismissal, modal Dialog barrier
  plus Escape dismissal, non-modal Sheet outside dismissal, Menu Escape/outside dismissal, and
  ContextMenu right-click hotspot open/Escape dismissal through runtime events.
- Decision: Overlay gallery default-open contract samples now stay visually closed on initial page
  render so modal barriers and floating layers do not block page scrolling or later samples. The
  resolved-state metadata rows still report the original default-open policy.
- Last verified for the Overlay smoke gate: `cargo fmt -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-foundation-gallery
  overlay_gallery_smoke`, `cargo nextest run -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only CRLF warnings.
- Done: Hardened the ContextMenu smoke from gallery-control opening to the real right-click hotspot
  path. The gallery test now scrolls the controlled ContextMenu hotspot into view, sends a right
  mouse down/up pair, asserts the surface appears, and closes it with Escape.
- Last verified for the ContextMenu right-click smoke hardening: `cargo fmt -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, engineering wiki validation, and `git diff --check` with only
  CRLF warnings.
- Done: Added a gallery-level Components interaction smoke gate. `open-gpui-ui-foundation-gallery`
  now renders the full Components page in `open_gpui::test` and drives short-viewport page
  scrolling/navigation reset, Select popup outside dismissal, nested ScrollArea wheel scrolling,
  vertical Tabs rail scrolling, Splitter pointer dragging, and long Sidebar internal navigation
  scrolling through runtime events.
- Done: Added stable runtime debug selectors for `RadioGroup` root/items plus a rendered RadioGroup
  keyboard smoke in `open-gpui-ui-components`. The smoke rejects disabled radio clicks, verifies
  click and arrow-selection payloads, skips disabled items with arrow navigation, and confirms Space
  on an already selected radio does not emit a duplicate selection change.
- Last verified for the RadioGroup runtime smoke: `cargo nextest run -p open-gpui-ui-components
  radio_group_runtime_keyboard_navigation_skips_disabled_items_and_payloads`.
- Done: Added stable runtime debug selectors for `Listbox` root, empty state, groups, separators,
  and options plus a rendered Listbox smoke in `open-gpui-ui-components`. The smoke rejects
  disabled option clicks, verifies standalone and grouped option payloads, keeps arrow navigation
  selection-free, skips disabled/separator rows, and locks keyboard Enter/Space activation so it
  dispatches both option-level and listbox-level callbacks.
- Last verified for the Listbox runtime smoke: `cargo fmt -p open-gpui-ui-components` and `cargo
  nextest run -p open-gpui-ui-components
  listbox_runtime_click_and_keyboard_selection_skip_disabled_items`.
- Done: Added a rendered Select smoke in `open-gpui-ui-components` and fixed Select popup keyboard
  navigation by no longer passing the parent-derived active value as a controlled active prop to the
  embedded Listbox while preserving explicit `Select::active(...)` control. The smoke opens the
  real trigger, rejects disabled popup option clicks, verifies ordered click and keyboard selection
  payloads, confirms popup Listbox arrows skip disabled rows, and checks selection closes the popup
  with open-change callbacks.
- Last verified for the Select runtime smoke: `cargo fmt -p open-gpui-ui-components` and `cargo
  nextest run -p open-gpui-ui-components
  select_runtime_click_and_keyboard_selection_close_popup_and_emit_payloads`.
- Done: Added stable runtime debug selectors for controller-backed `TextInput`, `Combobox`, and
  `Command` surfaces plus rendered Combobox and Command search interaction smokes. The Combobox
  smoke clicks the real text input, types a query, opens the filtered popup, verifies filtered
  Listbox options, selects a filtered option by click, and checks ordered select/open callbacks.
  The Command smoke clicks the real text input, types a query, verifies inline filtering, selects
  the active command with Down+Enter, checks shortcut payloads, and keeps non-dialog content open.
- Last verified for the Combobox/Command runtime smokes: `cargo fmt -p open-gpui-ui-components`,
  focused `cargo nextest run -p open-gpui-ui-components
  combobox_runtime_filters_input_and_selects_filtered_option`, focused `cargo nextest run -p
  open-gpui-ui-components command_runtime_filters_input_and_selects_with_keyboard`, and full
  `cargo nextest run -p open-gpui-ui-components` with 124 passing tests.
- Done: Added Tabs runtime keyboard automation in `open-gpui-ui-components` and fixed the rendered
  `Tabs` runtime to honor the builder-selected seed on first render plus bind per-tab focus handles
  to actual trigger elements. The smoke rejects disabled tab clicks, keeps Manual arrow navigation
  focus-only, and verifies Enter, Home+Enter, and End+Space activation payloads plus selected panel
  swaps.
- Last verified for the Tabs runtime smoke: red first on missing selected seed, then green with
  `cargo nextest run -p open-gpui-ui-components
  tabs_runtime_manual_keyboard_activation_preserves_selected_seed_and_payloads`.
- Done: Added stable runtime debug selectors for `Sidebar` root/items plus gallery Sidebar/Toolbar
  sample cards so shell-navigation interaction smoke tests can target real rendered nodes.
- Last verified for the Sidebar internal-scroll smoke: `cargo fmt -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery` and `cargo nextest run -p open-gpui-ui-foundation-gallery
  components_gallery_smoke_sidebar_long_navigation_scrolls_inside_sample`.
- Done: Added stable runtime debug selectors for `Toolbar` root/items plus a rendered Toolbar
  keyboard smoke in `open-gpui-ui-components`. The smoke clicks the first action item, moves roving
  focus with arrow/Home keys, skips disabled and separator items, and asserts Enter activation
  payloads.
- Last verified for the Toolbar keyboard smoke: `cargo fmt -p open-gpui-ui-components` and `cargo
  nextest run -p open-gpui-ui-components
  toolbar_runtime_keyboard_navigation_skips_disabled_and_separator_items`.
- Done: Added a compact shell/navigation runtime smoke. The gallery test now clicks the compact
  viewport switch, resizes to the compact viewport, asserts the mobile shell plus compact density
  snapshot, scrolls the left navigation rail to deep pages, and confirms switching away and back to
  Components resets the page scroll position.
- Done: Promoted the UI foundation/component runtime gates into the default `xtask verify` path.
  The local and Windows CI verify gate now runs `cargo nextest run -p open-gpui-ui-core`, `cargo
  nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery` after workspace checks and before the import-boundary scan.
- Last verified for the verify-gate promotion: `cargo fmt -p xtask`, `cargo nextest run -p xtask`,
  `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo run -p xtask -- verify`, engineering wiki validation,
  and `git diff --check` with only CRLF warnings.
- Last verified for the gallery smoke gate: `cargo fmt -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
- Last verified for the compact shell/navigation smoke: `cargo fmt -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-foundation-gallery`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` with 41 passing tests.
- Next automation direction: broaden the gallery smoke gate only when new UI families land or a
  manual regression exposes another runtime path. Screenshot coverage is not the default next step.
- Done: Hardened the Components gallery interaction dogfood surface after manual feedback. Vertical
  Tabs triggers now opt out of flex shrink so constrained vertical tablists can overflow and scroll;
  the gallery vertical Tabs sample now has enough items to dogfood that behavior, and the vertical
  Splitter sample starts expanded so drag resizing is visible.
- Done: Added runtime UI event regressions for horizontal and two-axis `ScrollArea` wheel behavior,
  constrained vertical Tabs scrolling, and both horizontal and vertical Splitter dragging. The
  components now expose test-only `debug_selector` anchors for these interaction tests; non-test
  builds keep `debug_selector` as a no-op.
- Last verified for the interaction hardening pass: `cargo fmt -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only CRLF warnings.
- Next automation direction: the component-level runtime tests are now strong enough for ScrollArea,
  Tabs, and Splitter regressions. The remaining useful enhancement is a small gallery-level visual
  smoke harness for overlay dismissal, scrolling containers, and splitter dragging when UI churn
  increases.
- Done: Wrote the current UI component productization roadmap at
  `docs/plans/2026-06-17-003-feat-ui-component-productization-roadmap-plan.md` and added ADR 0008
  at `docs/adr/0008-open-gpui-ui-component-productization-roadmap.md`.
- Decision: ADR 0008 makes `open-gpui-ui-core`, `open-gpui-ui-components`, and
  `examples/ui-foundation-gallery` the active product boundary for the next UI component phase.
  ADR 0006 and ADR 0007 remain extraction-boundary references, but standalone
  `open-gpui-ui-headless` creation is no longer the active roadmap.
- Done: Continued the productization roadmap through U2-U6 by verifying that the runtime
  foundations, interaction/layout primitives, shell/navigation family, choice/search family, and
  gallery release gate are already represented in the current component stack. This pass tightened
  dark/high-contrast theme table coverage, added a direct Command popup ScrollArea preserve-scroll
  assertion, and updated the gallery checkpoint test to assert ADR 0008 productization semantics.
- Last verified for the productization pass: `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
- Next action: Run final formatting/diff checks, commit the productization verification pass, then
  decide the next roadmap from actual product gaps rather than from standalone headless extraction.
- Done: Completed the strict UI-core headless boundary plan through the design checkpoint.
  `open-gpui-ui-core` no longer depends on `open_gpui`, its strict boundary guard has an empty
  blocker set, adaptive policy uses neutral `UiPx`, and GPUI geometry/style conversion now lives in
  `open_gpui_ui_components::gpui_adapter`.
- Done: Added ADR 0007 at
  `docs/adr/0007-open-gpui-ui-headless-boundary-design.md`. It is a design gate, not crate
  creation. It names overlay policy, roving focus, listbox navigation/typeahead, scroll viewport
  intent, and splitter constraints as first extraction candidates while keeping
  `TextInputController`, `ScrollHandle`, `focus_ring_shadow`, `GpuiOverlayState`, AccessKit node
  wiring, concrete focus handles, GPUI render trees, and adapter geometry conversions in the GPUI
  adapter layer.
- Last verified for the strict boundary slice: `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, focused nextest for
  `ui_core_extraction_blockers_match_allowlist`, `ui_core_strict_boundary_blockers_match_allowlist`,
  component adapter export/boundary guards, and the gallery headless checkpoint test. `git diff
  --check` passed with only existing CRLF warnings.
- Prior next action superseded by ADR 0008: the ADR 0007 behavior-crate extraction design is
  deferred reference material, not the next implementation step.
- Done: Wrote the next extraction-prep plan at
  `docs/plans/2026-06-17-001-refactor-ui-headless-extraction-prep-plan.md`. The plan keeps
  `open-gpui-ui-headless` deferred and targets the blockers recorded by ADR 0006: public GPUI
  geometry aliases, direct GPUI focus/a11y re-exports, adapter-facing `GpuiOverlayState`, and
  ambiguous adapter-only APIs such as `TextInputController`, `ScrollHandle`, and `focus_ring_shadow`.
- Decision: The next implementation series should first strengthen the extraction guard inventory,
  then migrate neutral geometry/metrics, add focus and accessibility facades, split neutral overlay
  state from GPUI adapter scheduling, classify adapter-only APIs, and finally update ADR 0006 with
  the crate-extraction readiness decision.
- Done: Implemented U1 guard inventory. `open-gpui-ui-components` now has a companion extraction
  blocker allowlist for public `*State` and `*Metrics` contracts, while `open-gpui-ui-core` has a
  new `tests/headless_contracts.rs` guard for direct GPUI focus/a11y and geometry blockers.
- Last verified: `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `git diff --check`
  passed for U1.
- Done: Implemented U2 neutral geometry. `open-gpui-ui-core` now owns `UiPx`, `UiPoint`,
  `UiSize`, `UiRect`, and `UiEdges`; overlay `Rect`, `OverlaySize`, `OverlayEdges`, point anchors,
  offsets, and safe bounds now use those neutral values. `open-gpui-ui-components` converts at the
  GPUI adapter edge, and `ContextMenuState` exposes a neutral point anchor instead of
  `Point<Pixels>`.
- Last verified: U2 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
- Done: Implemented U3 neutral component metrics. `Size` helpers and public component `*Metrics`
  now return `UiPx` instead of GPUI `Pixels`, gallery sizing samples use `UiPx`, and the component
  extraction-blocker allowlist shrank to the remaining `GpuiOverlayState` public-state blockers.
  `UiPx` has GPUI style-conversion impls in UI core as a transitional convenience for the current
  adapter-first crates; a later strict headless boundary should move that conversion out with the
  overlay/focus split.
- Last verified: U3 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF warnings.
- Done: Implemented U4 focus and accessibility facades. `open-gpui-ui-core` now defines neutral
  `Role`, `Toggled`, `Orientation`, `AccessibleAction`, and `FocusTargetId` instead of
  re-exporting GPUI focus/a11y types. `open-gpui-ui-components::a11y` owns the GPUI/AccessKit
  adapter mapping, and render code applies neutral roles/states/actions through explicit
  `ui_*` adapter methods.
- Last verified: U4 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF warnings.
- Done: Implemented U5 neutral overlay state split. `open-gpui-ui-core` now owns
  `OverlayResolvedState`; Tooltip, Popover, Dialog, Menu, ContextMenu, AlertDialog, Sheet,
  HoverCard, Select, Combobox, and Command expose that neutral state publicly. GPUI deferred
  priority and snap margin remain in `GpuiOverlayState` and are derived at render/adapter sites.
- Last verified: U5 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
- Done: Started U6 adapter-only API classification. `FocusRing` now exposes neutral `UiPx` width,
  while `focus_ring_shadow` remains the GPUI `BoxShadow` adapter helper. `TextInputController`,
  text-input initialization, `focus_ring_shadow`, `GpuiOverlayState`, and GPUI overlay scheduling
  helpers are grouped under `open_gpui_ui_components::gpui_adapter`; externally supplied
  `ScrollHandle` remains documented as a `ScrollArea` adapter escape hatch.
- Review: U6 read-only review subagent `u6_adapter_classification_review` did not return findings
  before timeout and was interrupted; local self-review removed an accidental wildcard public
  re-export and added a guard for explicit re-exports.
- Last verified: U6 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF warnings.
- Done: Updated ADR 0006 for the U7 extraction-prep checkpoint. The decision remains not to create
  `open-gpui-ui-headless` in this branch. Component resolved-state blockers are cleared, but a
  strict headless crate still needs decisions for adaptive viewport `Pixels as Px` and `UiPx` GPUI
  style-conversion impls in UI core.
- Review: U7 read-only doc review subagent `u7_checkpoint_doc_review` did not return findings
  before timeout and was interrupted. Local self-review found no current-doc references that still
  describe geometry/focus/a11y/overlay split work as unfinished.
- Last verified: U7 passed `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
- Done: Committed U7 as `5318178 docs(ui): update headless extraction checkpoint`.
- Next action: Plan a narrow behavior-crate extraction design that starts with overlay, roving
  focus, listbox navigation, scroll viewport intent, and splitter constraints after the two
  remaining core-boundary questions are resolved.

## 2026-06-16

- Done: Completed U8 of the UI shell, choice, and headless-readiness series by updating ADR 0006
  after Toolbar/Sidebar, ScrollArea/Splitter, Listbox/Select, Combobox, and Command landed. The
  decision remains to defer `open-gpui-ui-headless`: reusable behavior is now proven across
  overlay policy, roving focus, listbox navigation, scroll viewport intent, and splitter
  constraints, but extraction still needs neutral geometry/focus/a11y facades, an overlay-state
  split, and a decision on GPUI-backed text editing.
- Done: Added a component contract guard,
  `public_resolved_state_contracts_avoid_gpui_runtime_types`, that scans public resolved-state
  structs for GPUI runtime/rendering/callback leaks (`Window`, `App`, `Context`, `RenderOnce`,
  `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and `Rc<dyn` callback
  storage). `Pixels`/geometry aliases remain a documented blocker rather than a failing gate.
- Last verified: U8 reused the U7 final checks plus the new contract guard:
  `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check`.
- Next action: Commit U8, then plan extraction-prep work rather than creating
  `open-gpui-ui-headless` immediately. The obvious next plan should split neutral geometry/a11y/
  focus vocabulary, rename/split `GpuiOverlayState`, and classify `TextInputController` as
  adapter-only or factor out a smaller neutral editing model.
- Done: Continued the UI shell, choice, and headless-readiness series with U7 Combobox and
  Command. Added `Combobox`, `ComboboxState`, grouped/standalone option descriptors, editable
  query metadata, selected value/label metadata that survives filtering, active option metadata,
  empty state, non-modal popup overlay policy, scroll viewport metadata, and explicit crate-root/
  prelude exports to `open_gpui_ui_components`.
- Done: Added `Command`, `CommandState`, command groups/items, shortcut metadata, loading metadata,
  empty state, optional dialog wrapper state, modal dialog overlay policy, and command selection
  payloads. Command v1 remains a local search/list surface and intentionally defers async loading,
  fuzzy ranking, multi-select chips, virtualized result sets, and global app command registration.
- Done: Added Components gallery Combobox and Command samples for filtered grouped options, empty
  search, disabled search, dialog-backed workspace commands, inline empty loading commands, and
  disabled commands. Gallery tests now assert combobox roles/filtering/empty state and command
  dialog/loading/shortcut contracts.
- Review: U7 review subagent `u7_review_fast` returned after the main U7 commit. It flagged a
  gallery coverage gap for combobox selection persistence, which was fixed in the gallery sample
  and tests. Its Escape-policy concern matched an already-policy-gated render path using
  `escape_open_change`, so a direct overlay-policy assertion was added instead of changing the
  close handler.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed during U7.
- Next action: Commit U7, then start U8 headless-readiness checkpoint. The checkpoint should audit
  public resolved-state types for GPUI runtime leaks and decide whether ADR 0006 needs an
  extraction-plan follow-up.
- Done: Continued the UI shell, choice, and headless-readiness series with U6 Listbox and Select.
  Added `Listbox`, `ListboxState`, grouped/standalone descriptors, option/separator anatomy,
  selected and active descendant metadata, disabled/separator skipping, typeahead target metadata,
  and explicit crate-root/prelude exports to `open_gpui_ui_components`.
- Done: Added `Select`, `SelectState`, controlled/uncontrolled open mode, placeholder and selected
  trigger label metadata, nested `ListboxState`, non-modal dismissible overlay policy, scroll
  viewport metadata, and keyed runtime state for open/selected/active behavior. Select v1 composes
  trigger + overlay + Listbox; searchable Combobox/Command behavior remains deferred.
- Done: Added Components gallery Listbox and Select samples for grouped choices, empty state,
  controlled-open long select, closed selected select, and disabled empty select. Gallery tests now
  assert choice roles, listbox navigation/activation/typeahead, Select overlay/focus/outside-press
  policy, and scrollable popup metadata.
- Last verified: `cargo fmt --all`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed during the U6 Listbox/Select slice.
- Next action: Finish U6 review/doc polish and commit, then start U7 Combobox and Command. Listbox
  real text event accumulation for typeahead, multi-select, option virtualization, richer
  active-descendant AccessKit references, and Select item-aligned positioning remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U5 HoverCard. Added
  `HoverCard`, `HoverCardState`, hover/focus/manual open intent, controlled/uncontrolled open
  mode, delay policy, interactive non-modal overlay metadata, token intents, metrics, and explicit
  crate-root/prelude exports to `open_gpui_ui_components`.
- Done: HoverCard uses a non-modal dismissible overlay contract instead of reusing descriptive
  Tooltip semantics. It defaults to no initial focus and no focus restoration, dismiss-and-pass-
  through outside behavior, open/close delay handling, and keyed runtime state for hover/focus
  lifetime so keyboard focus can move between trigger and content without immediately closing.
- Done: Added Overlay gallery HoverCard samples for default-open profile preview, focus-only
  preview, and manual controlled card behavior. Gallery tests cover interactive overlay metadata,
  manual/controlled policy, default delay, focus restore defaults, and placement intent.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U5
  HoverCard slice.
- Next action: Commit U5 HoverCard, then start U6 Listbox and Select foundation.
  HoverCard safe pointer corridors, arrows, text-selection leases, richer focus-scope traversal,
  and role refinement beyond `Role::Window` remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U4 AlertDialog and
  Sheet. Added `AlertDialog`, `AlertDialogState`, action metadata, destructive intent, and `Sheet`,
  `SheetState`, side/modal/close-affordance enums to `open_gpui_ui_components`, keeping
  critical-action and edge-attached overlay semantics in resolved state while GPUI adapters own
  focus handles, callbacks, deferred rendering, barriers, and placement.
- Done: AlertDialog defaults to `Role::AlertDialog`, cancel-first initial focus metadata, trigger
  focus restore, modal underlay blocking, and outside-press consume-without-dismiss. Sheet models
  left/right/top/bottom attachment, modal versus non-modal overlay kind, close affordance
  visibility, and explicit outside-press policy instead of inheriting Dialog defaults.
- Done: Added Overlay gallery AlertDialog samples for destructive confirmation and safe cancel,
  plus Sheet samples for left modal, right non-modal, and bottom sticky behavior. Gallery tests now
  assert critical action contracts, sheet edge/policy contracts, and visible state metadata.
- Done: U4 subagent review found render-time state ownership drift, gallery sample contract drift,
  and hidden/disabled initial-focus target risks. The fixes preserve uncontrolled mode when runtime
  open changes, align controlled gallery samples with metadata, skip unavailable initial-focus
  targets, and defer overlay initial focus until after the layer is scheduled.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery` passed after the U4 review fixes.
- Next action: Commit U4, then start U5 HoverCard. AlertDialog/Sheet full focus trap traversal,
  labelled-by/described-by relationships, animation lifecycle, sheet resizing, and part-based
  composition APIs remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U3 Sidebar. Added
  `Sidebar`, `SidebarState`, `SidebarSection`, `SidebarItem`, descriptors, selection payloads,
  side/variant/collapse enums, metrics, colors, vertical roving-focus navigation, and explicit
  crate-root/prelude exports to `open_gpui_ui_components`.
- Done: Sidebar resolved state now models expanded, icon-collapsed, and offcanvas-collapsed
  behavior without GPUI runtime types. Icon collapse hides visible text but keeps item labels and
  focusability; offcanvas collapse removes items from roving focus; disabled items are skipped and
  cannot produce activation payloads.
- Done: Added Components gallery Sidebar samples for expanded workspace navigation, icon rail, and
  a long scrollable reports navigation, plus gallery tests for navigation metadata, roles,
  collapse behavior, disabled skip behavior, and scrollability.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U3 Sidebar slice.
- Next action: Commit U3 after review, then start U4 AlertDialog and Sheet. Sidebar provider
  contexts, mobile sheet routing, nested submenus, route integration, persisted layout preferences,
  animation lifecycle, shortcuts, and command registry integration remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U2 Toolbar. Added
  `Toolbar`, `ToolbarState`, `ToolbarItem`, `ToolbarItemDescriptor`, `ToolbarItemState`, and
  `ToolbarSelection` to `open_gpui_ui_components`, keeping command grouping, item kind, disabled
  state, pressed state, tab-stop state, metrics, colors, and roving-focus decisions in resolved
  state while the GPUI adapter owns focus handles, events, and rendering.
- Done: Added horizontal and vertical Toolbar samples to the Components gallery, including action,
  toggle, separator, disabled, focused, and pressed item states. Gallery tests now assert the
  Toolbar metadata and roving-focus contract, and component tests assert disabled/separator skip
  behavior plus keyboard activation payloads.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest
  run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U2 Toolbar
  slice.
- Next action: Commit U2, then start U3 Sidebar on the same resolved-state plus GPUI-adapter
  boundary. Toolbar overflow behavior, shortcut display, app command registry integration,
  customization, and icon asset resolution remain deferred.
- Done: Started the UI shell, choice, and headless-readiness series with
  `docs/plans/2026-06-16-002-feat-ui-shell-choice-headless-series-plan.md`. U1 adds a visible
  Components conformance gate surface for explicit crate/prelude exports, gallery metadata,
  ScrollArea redraw persistence, Splitter runtime constraints, Tabs overflow/roving focus, and
  explicit accessible labels.
- Done: The Components gallery now renders the conformance gates, and gallery tests assert stable
  gate ids/evidence alongside the existing component metadata samples. `open_gpui_ui_components`
  also has an isolated crate-root/prelude export smoke test so deleting a public re-export fails
  the intended surface.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest
  run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U1 conformance
  gate slice.
- Next action: Start U2 Toolbar on the existing resolved-state plus GPUI-adapter boundary.
- Done: Completed the U11 Splitter half by adding `Splitter` to `open_gpui_ui_components`.
  `SplitterState` now records group id, orientation, panel fractions, min/max constraints,
  collapsible/collapsed metadata, handle adjacency, disabled state, and metrics. It also owns
  `resized_by` so min/max delta clamping is testable without a GPUI window.
- Done: Added Components gallery Splitter samples for horizontal workspace panes and a vertical
  collapsed/details stack, plus gallery metadata tests for panel/handle state. The concrete adapter
  now renders resolved fractions and handle affordances and wires local pointer dragging through a
  keyed runtime. Drag move events are handled on the root splitter so pixel deltas are measured
  against the full splitter bounds, then fed through `SplitterState::resized_by`; drag payloads carry
  the group id to avoid multi-splitter cross-talk.
- Done: Added `SplitterState::with_panel_fractions` so live runtime fractions reuse the same
  normalization and min/max constraint path as descriptor-based state. Keyboard resizing,
  controlled resize callbacks, persisted layouts, RTL behavior, and nested splitter arbitration
  remain deferred.
- Done: Fixed the vertical collapsed Splitter drag path: dragging a collapsed collapsible panel
  below its restore threshold keeps the collapsed fraction stable; dragging far enough clears
  `collapsed` and resumes normal min/max resizing. This fixes the gallery's vertical
  `details-split` sample, whose top panel starts collapsed.
- Done: Fixed `ScrollArea` appearing non-scrollable in the Components gallery. The default
  `ScrollHandle` now lives in `ScrollAreaRuntime` keyed by the viewport element id instead of being
  allocated inside each `ScrollArea::new` builder value, so wheel scrolling survives the redraw that
  the scroll event itself triggers. Externally supplied handles remain supported for callers that
  need to inspect or manipulate offset directly.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery` passed after the first Splitter slice.
  `cargo check -p open-gpui-ui-components` and `cargo nextest run -p open-gpui-ui-components`
  passed again after the pointer-drag runtime. `cargo check -p open-gpui-ui-foundation-gallery`
  also passed after the collapsed-panel restore fix. `cargo fmt -p open-gpui-ui-components`,
  `cargo check -p open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo check -p open-gpui-ui-foundation-gallery` passed after the ScrollArea runtime-handle fix.
- Next action: Run a manual Components-gallery dogfood pass, then move to U12 Toolbar/Sidebar or
  the next gallery conformance item.
- Done: Started the layout/shell-navigation component series by adding `ScrollArea` to
  `open_gpui_ui_components`. `ScrollAreaState` records stable viewport id, axis, reset policy/key,
  size, and scrollbar metrics without storing GPUI handles; the concrete adapter owns
  `ScrollHandle`, GPUI overflow styles, scrollbar width, and reset-on-key-change offset mutation.
- Done: Added Components gallery ScrollArea samples for vertical, horizontal, and two-axis overflow,
  plus gallery metadata coverage for axis/reset/metrics. Also repaired the gallery ContextMenu test
  to assert renderer-neutral `OverlayPlacementInput` fields after the prior placement extraction.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery` passed after the ScrollArea slice.
- Next action: Continue the layout/shell-navigation series with Splitter/ResizablePanel primitives
  or Toolbar, using the same resolved-state plus GPUI-adapter boundary. For ScrollArea, custom
  scrollbar anatomy, hover/auto visibility, nested scroll routing, and wheel arbitration remain
  deferred until the base viewport is dogfooded.
- Done: Finished the ADR 0006 stack-ordering follow-up by adding window-free overlay stack ordering
  primitives in `open_gpui_ui_core::overlay`: `resolve_outside_press` and
  `resolve_focus_restore`, plus tests for topmost dismissible-layer handling and focus restoration.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-core`,
  `cargo nextest run -p open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, and
  `cargo check -p open-gpui-ui-foundation-gallery` passed after the overlay stack resolver work.
- Next action: Start the next official component roadmap item after ADR 0006; likely candidates are
  ScrollArea/Toolbar/Sidebar, or a focused geometry-alias cleanup if the headless boundary should
  be tightened further first. Full focus-trap/scope traversal remains deferred until nested overlay
  components need it.
- Done: Continued ADR 0006 follow-up by moving `ContextMenuState` to renderer-neutral
  `OverlayPlacementInput` instead of storing resolved `GpuiOverlayPlacement`. The GPUI placement
  is now derived only at the adapter/render boundary.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed for the context-menu placement extraction slice.
- Next action: Keep removing remaining GPUI geometry leaks from resolved state, then add
  window-free focus-scope and dismissible-layer ordering tests before reconsidering a headless
  crate.
- Done: Started ADR 0006 follow-up by moving shared roving-focus helpers out of `tabs.rs` into
  `open_gpui_ui_components::roving_focus`. `Tabs` preserves compatibility re-exports for the old
  helper paths, while `Menu` and `RadioGroup` now depend on the neutral behavior module.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed for this roving-focus extraction slice.
- Next action: Separate renderer-neutral menu/context-menu placement input from
  `GpuiOverlayPlacement`, then add window-free focus-scope and dismissible-layer ordering tests
  before reconsidering a headless crate.
- Done: Completed U8 of the overlay component series by adding ADR 0006
  (`docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md`). The checkpoint keeps
  `open-gpui-ui-headless` deferred: overlay components now prove repeated behavior contracts, but
  several state types still expose GPUI geometry or adapter placement state, so extraction would
  freeze the wrong boundary.
- Done: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and engineering memory so
  the overlay family documents which behavior is renderer-neutral, which remains GPUI adapter
  responsibility, and what gate must be met before a future headless crate.
- Last verified: Final overlay-series quality pass ran `cargo fmt --all`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
- Next action: Start the next official component roadmap slice from ADR 0006 follow-ups: neutralize
  shared roving-focus helpers and remove GPUI placement leaks before reconsidering a headless crate.
- Done: Completed U7 of the overlay component series by adding `Menu` and `ContextMenu` to
  `open-gpui-ui-components` with shared menu item descriptors, action/separator items, disabled
  item state, roving-focus navigation, keyboard activation payloads, Escape/outside policies,
  trigger-anchored menu placement, point-anchored context-menu placement, exports, and tests.
- Done: Added Overlay gallery Menu samples for default-open, controlled, outside-ignored, and
  disabled cases, plus ContextMenu samples for point-anchor, controlled, and default-open cases.
  Gallery tests now cover menu roving-focus contracts and context-menu point-anchor placement.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U7 Menu/ContextMenu work.
- Next action: Start U8 overlay examples checkpoint and headless-readiness review, then run the
  final quality pass for the full overlay component series.
- Done: Completed U6 of the overlay component series by adding modal `Dialog` to
  `open-gpui-ui-components` with `DialogState`, controlled/uncontrolled open mode, default-open
  state, title/description metadata, Escape policy, outside-press policy, initial focus and
  focus-restore intent, modal layer state, token/metric resolution, exports, and targeted tests.
- Done: Added Overlay gallery Dialog samples for controlled modal, default-open modal,
  outside-ignored modal, and disabled trigger. The controlled sample is owned by gallery shell state;
  Escape and the modal barrier can close it, while open modal layers block underlay input.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U6 Dialog work.
- Next action: Start U7 Menu/ContextMenu on top of the shared overlay policy and Dialog/Popover
  precedent, covering item roles, selection/disabled state, keyboard/Escape behavior, and context
  trigger positioning.
- Done: Completed U5 of the overlay component series by adding interactive non-modal `Popover` to
  `open-gpui-ui-components` with `PopoverState`, controlled/uncontrolled open mode, default-open
  state, trigger expanded/selected intent, outside-press policy, placement metadata, initial focus
  and focus-restore intent, token/metric resolution, exports, and targeted tests.
- Done: Added Overlay gallery Popover samples for default-open, controlled, consuming outside
  press, and disabled cases. The controlled sample is owned by gallery shell state and closes on
  Escape via the shared shell handler.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U5 Popover work.
- Next action: Start U6 Dialog on top of the shared overlay adapter and Popover precedent, covering
  modal layer state, title/description metadata, Escape/outside policies, and focus restoration.
- Done: Completed U4 of the overlay component series by adding descriptive `Tooltip` to
  `open-gpui-ui-components` with `TooltipState`, hover/focus/manual open intent, delay policy,
  placement metadata, token/metric resolution, explicit exports, component tests, and Overlay
  gallery samples that reveal tooltip content from hover or keyboard focus while keeping disabled
  triggers closed.
- Done: Updated `docs/ui/component-contract.md` and `docs/verification.md` so Tooltip is documented
  as a descriptive non-interactive overlay contract, with timing execution and trigger/focus wiring
  remaining in the GPUI adapter/gallery layer.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U4 Tooltip work.
- Next action: Start U5 Popover on top of the shared overlay adapter, covering controlled/default
  open state, Escape/outside dismissal, placement, and focus restoration.
- Done: Started U2 by extending `open-gpui-ui-core::overlay` from geometry helpers to
  renderer-neutral overlay behavior contracts: layer identity/kind, presence, outside-press policy,
  Escape policy, dismiss reason, focus restore intent, initial focus intent, layer-state resolution,
  Escape stack resolution, and anchor/placement input. These contracts intentionally avoid GPUI
  runtime types.
- Done: Updated the foundation gallery overlay page to expose a behavior contract matrix for
  tooltip, popover, dialog, and menu policies, and updated `docs/ui/component-contract.md` with the
  overlay resolved-state boundary.
- Last verified: `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery` passed after the U2 overlay behavior contract work.
- Done: Started U3 by adding `open_gpui_ui_components::overlay`, a narrow GPUI adapter mapping
  layer that resolves deferred priority, snap margin, GPUI anchor/offset, Escape open-change, and
  outside-press open-change from the U2 renderer-neutral policy without owning a global overlay
  runtime or storing GPUI callbacks.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U3 adapter helper work.
- Done: Started the overlay component series with U1 accessibility/gallery runtime gate work:
  added direct coverage for valid and invalid AccessKit cross-node references in
  `crates/gpui/src/window/a11y.rs`, removed the compile-time bundled-font dependency from the
  `svg_renderer` test harness so the `open-gpui` library tests compile in this checkout, and added
  a Gallery metadata test plus `--page components` startup path that lock explicit accessible
  labels, label-to-control association metadata, and the direct Components runtime smoke.
- Last verified: `cargo check -p open-gpui`, `cargo check -p open-gpui-ui-components`, `cargo
  check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui --lib
  window::a11y::tests::repair_tree_update`, `cargo nextest run -p open-gpui --lib
  svg_renderer::tests::`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run
  -p open-gpui-ui-foundation-gallery` passed for U1. `cargo run -p
  open-gpui-ui-foundation-gallery -- --page components` stayed alive until the 30s smoke timeout
  and did not reproduce the `accesskit_consumer` panic.
- Done: Committed the U6 Badge/IconButton slice as
  `9206210 feat(ui): add badge and icon button components`.
- Done: Wrote the next-series overlay component plan at
  `docs/plans/2026-06-16-001-feat-ui-overlay-component-series-plan.md`.
- Decision: The next execution series starts with an accessibility/gallery runtime gate, then
  renderer-neutral overlay behavior contracts, GPUI overlay adapter helpers, `Tooltip`, `Popover`,
  `Dialog`, `Menu`/`ContextMenu`, and finally a headless-readiness checkpoint. `ScrollArea`,
  `Splitter`, `Toolbar`, and `Sidebar` move to the following layout/shell-navigation series.
- Next action: Start U1 of the overlay component series: prove the AccessKit repair and gallery
  runtime smoke path before adding new overlay-heavy components.
- Done: Completed U6 of the official component roadmap by adding `Badge` and `IconButton` to
  `open-gpui-ui-components` with pure resolved-state contracts, GPUI adapters, explicit exports,
  theme intents, gallery dogfood, and targeted tests.
- Done: Added `Size::icon_size()` to the UI foundation sizing vocabulary so icon-bearing controls
  do not hide glyph metrics in individual component adapters.
- Done: Hardened GPUI accessibility tree repair to strip invalid cross-node references such as
  `labelled_by`, `controls`, and active-descendant pointers before handing updates to AccessKit
  platform adapters. This addresses the Components-page crash where `accesskit_consumer` panicked
  while resolving an explicit label reference to a missing node.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui`, `cargo check -p
  open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passed
  during U6 implementation.
- Verification note: `cargo nextest run -p open-gpui --lib
  window::a11y::tests::repair_tree_update_strips_invalid_node_references` could not compile the
  `open-gpui` test harness because local font assets under `assets/fonts/ibm-plex-sans` and
  `assets/fonts/lilex` are missing. The regular `cargo check -p open-gpui` path passes.
- Done: The next-series plan resolves the prior fork by doing the accessibility runtime smoke first
  and then continuing into shared overlay behavior.
- Done: Completed U5 of the official component roadmap by adding `RadioGroup` and `Toggle` to
  `open-gpui-ui-components` with pure resolved-state contracts, GPUI adapters, exports, gallery
  dogfood, and targeted tests.
- Done: Applied a follow-up U5 cleanup that shared Tabs/Radio selection helpers, removed the
  Toggle gallery sample drift, and gave `Toggle` its own exported metrics/colors aliases while
  keeping the Button implementation as the underlying visual model.
- Done: Committed the main U5 slice as `5e562f3 feat(ui): add radio group and toggle components`.
- Done: `RadioGroup` now reuses the U4 roving-focus helpers, exposes group required/disabled
  metadata plus per-item selected/focused/tab-stop state, and maps radio items with
  `Role::RadioButton` + `aria_selected` on the current GPUI/AccessKit surface.
- Done: `Toggle` now models button-like pressed state through `Role::Button` + `aria_toggled`
  while staying separate from Checkbox tri-state semantics.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed during U5 implementation and again after
  the follow-up cleanup.
- Next action: Start the next roadmap item (`Badge` / `IconButton`).
- Done: Completed U4 of the official component roadmap by adding `Tabs` to
  `open-gpui-ui-components` with a pure resolved-state contract, GPUI adapter, roving-focus
  helpers, gallery dogfood, and targeted tests.
- Done: Fixed the vertical Tabs dogfood so the left tab rail scrolls inside a constrained gallery
  card, matching the user-reported overflow issue.
- Done: Updated the Components gallery and verification docs to cover horizontal automatic
  activation and vertical manual activation, plus keyboard roving-focus verification.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the Tabs scroll fix.
- Done: Committed the Tabs slice as `f0dbf96 feat(ui): add Tabs roving focus slice`.
- Next action: Start the next roadmap item (`RadioGroup` / `Toggle`).

## 2026-06-15

- Goal: Grow the official Open GPUI component system under the adapter-first, headless-ready
  architecture from ADR 0005.
- Branch: `feat/open-gpui-ui-core`
- Last verified: `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo check -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passed after
  the Checkbox/Label slice.
- Done: Added the `open-gpui-ui-core` crate with sizing, density, adaptive, token, overlay, a11y, and focus foundation vocabulary; ADR 0004 and memory bundle now point at the foundation-first direction and explicitly record the reference repositories (`fret`, `fret-ui-kit`, `fret-ui-shadcn`, `gpui-component`, plus broader open source UI references).
- Done: Wrote the first follow-up plan for a dedicated pure-foundation gallery example at `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md`.
- Done: Completed U1 of the gallery plan by adding `examples/ui-foundation-gallery` as a workspace package with a small library, thin binary entrypoint, pure foundation dependency surface, empty shell, section registry, and targeted tests.
- Done: Completed U2 by replacing the U1 placeholder for tokens, sizing/density, and adaptive pages with real `open-gpui-ui-core` data models, rendered sample tables, and a compact/desktop viewport switch.
- Done: Completed U3 by replacing focus/a11y and overlay placeholders with interactive demos: focus-visible controls, accessibility roles/actions/state, overlay geometry samples, and an anchored deferred popover.
- Done: Completed U4 by adding the UI foundation gallery to `docs/verification.md` with focused package commands and manual compact/desktop, focus/a11y, and overlay dogfood checks.
- Done: Committed the foundation slice as `f626464 feat(ui): add UI foundation core gallery`.
- Done: Wrote the next plan for `open-gpui-ui-components` at `docs/plans/2026-06-15-002-feat-ui-components-first-slice-plan.md`, scoped to Button, Switch, gallery dogfood, and verification.
- Done: Completed the first components slice by scaffolding `crates/ui_components` as `open-gpui-ui-components`, implementing Button and Switch, wiring the Components gallery page, and updating the engineering memory bundle.
- Done: Drafted ADR 0005 for the official component architecture, choosing an adapter-first, headless-ready model and a future extraction path for `open-gpui-ui-headless`.
- Done: Wrote the TextInput/Field implementation plan at
  `docs/plans/2026-06-15-003-feat-ui-text-field-slice-plan.md` and the component contract guide at
  `docs/ui/component-contract.md`.
- Done: Implemented `TextInput` and `Field` in `open-gpui-ui-components` with resolved state,
  metrics, token intents, role/message metadata, tests, explicit exports, and gallery dogfood.
- Done: Recorded subagent research showing full editable text input must use GPUI's
  `EntityInputHandler` / `ElementInputHandler` path, so this slice intentionally remains a
  display/semantic contract slice.
- Done: Committed the TextInput/Field slice as `33842c4 feat(ui): add text field component slice`.
- Done: Added `ThemeResolver` to `open-gpui-ui-components`, moved Button/Switch/TextInput/Field
  render-time color conversion through it, and kept `ColorIntent` as the resolved state contract for
  token-aware tests and future headless extraction.
- Done: Added `FocusRing` to `open-gpui-ui-components`, migrated Button/Switch/TextInput and the
  focus/a11y gallery demo to paint focus-visible state with GPUI box-shadow instead of changing
  border width, and covered the token intent plus no-layout-shift contract in tests.
- Done: Implemented the real single-line editable `TextInputController` slice in
  `open-gpui-ui-components`, including GPUI `EntityInputHandler` / `ElementInputHandler`
  integration, UTF-16 selection and marked-range conversion, grapheme-aware deletion, clipboard
  actions, and gallery dogfood for the default components sample.
- Done: Completed U3 of the official component roadmap by adding `Checkbox` and `Label` to
  `open-gpui-ui-components` with resolved state, GPUI adapters, theme intents, tests, gallery
  samples, and updated verification guidance.
- Done: Updated `docs/verification.md` so the Components manual dogfood now includes Checkbox and
  Label association checks in addition to Button, Switch, TextInput, and Field.
- Done: Updated the component contract to record that `TextInputController` now owns the editable
  single-line path while `Field` remains composition-only, and that multiline/password/undo/redo/
  completion stay out of scope.
- Done: Wrote the next-series roadmap at
  `docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md`. The planned order is runtime
  theme table, real editable TextInput controller, Checkbox/Label, roving focus/Tabs,
  RadioGroup/Toggle, Badge/IconButton, shared overlay behavior, Tooltip/Popover, Dialog,
  Menu/ContextMenu, ScrollArea/Splitter, Toolbar/Sidebar, gallery conformance, then a headless
  extraction readiness review.
- Done: Recorded the planning decision that `open-gpui-ui-headless` remains deferred until repeated
  contracts exist across Button/Switch/TextInput/Field, Checkbox/Radio, Tabs, and at least one
  overlay family. Reference repositories remain inputs, not runtime dependencies.
- Done: Recorded reference repository findings at
  `docs/knowledge/engineering/subagents/ui-component-roadmap-reference-research.md`: use
  `gpui-component` for GPUI-native implementation patterns, `fret-ui-kit` for policy-layer
  references, and do not copy Fret runtime or `gpui-component` editor-grade input code wholesale.
- Done: Completed the runtime theme table slice by adding `ColorState`, `ThemeMode`,
  `ThemeColor`, and immutable `ThemeSnapshot` support to `open-gpui-ui-components`.
  `ThemeResolver::resolve_with` now resolves `(TokenKey, ColorState)` from light, dark, or
  high-contrast snapshots before falling back to intent RGB; the gallery token page exposes
  mode/revision metadata.
- Done: Recorded runtime theme reference guidance at
  `docs/knowledge/engineering/subagents/runtime-theme-reference-research.md`: keep U1 to
  immutable snapshots plus fallback semantics; defer app-level registries, user theme files, JSON
  schema, and hot reload.
- Done: Recorded editable TextInput controller reference guidance at
  `docs/knowledge/engineering/subagents/text-input-controller-research.md`: keep U2 to a
  single-line controller plus GPUI input handler adapter; defer multiline/password/editor features.
- Done: Updated `docs/ui/component-contract.md` to include Checkbox indeterminate state and Label
  association metadata in the resolved-state contract.
- Blocked: None.
- Next action: Commit the Checkbox/Label slice, then start U4 on roving focus and Tabs.

# Citations

[1] [ADR 0004](../../adr/0004-open-gpui-component-library-strategy.md)
[2] [Decision](decisions/open-gpui-ui-foundation-first.md)
[3] [Session handoff](sessions/open-gpui-component-library-handoff.md)
[4] [Verification](../../adr/0004-open-gpui-component-library-strategy.md#success-metrics)
[5] [Plan](../../plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md)
[6] [Manual verification guide](../../verification.md)
[7] [Components first slice plan](../../plans/2026-06-15-002-feat-ui-components-first-slice-plan.md)
[8] [Official component architecture](../../adr/0005-open-gpui-official-component-architecture.md)
[9] [TextInput/Field plan](../../plans/2026-06-15-003-feat-ui-text-field-slice-plan.md)
[10] [Component contract guide](../../ui/component-contract.md)
[11] [Text input subagent finding](subagents/text-input-patterns.md)
[12] [Official UI component roadmap](../../plans/2026-06-15-004-feat-ui-component-roadmap-plan.md)
[13] [Roadmap reference research](subagents/ui-component-roadmap-reference-research.md)
[14] [Runtime theme reference research](subagents/runtime-theme-reference-research.md)
[15] [Text input controller research](subagents/text-input-controller-research.md)
