# Engineering Memory Update Log

## 2026-06-19
* **Update**: Added a browser-level smoke for the controlled hover card toggle surface and gave the toggle a gallery debug selector. The gallery now proves the shell-controlled hover card can be opened from the control surface and dismissed with Escape.
* **Verification**: `cargo fmt --all`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` (50/50), and `cargo nextest run -p open-gpui-ui-components --tests` (147/147) passed after the hover-card control-surface cleanup.
* **Decision**: Keep scanning for the next evidence-backed seam; do not treat the current hover-card chain as a remaining gap unless a new behavior split appears.

* **Update**: Added a browser-level smoke for the tooltip manual delayed sample so gallery automation now proves the forced-open tooltip content renders directly from state, not only from hover/focus interaction.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed with 49/49 tests.
* **Decision**: Keep scanning for the next evidence-backed seam; do not force a shallow refactor unless a stronger contract split appears.

* **Update**: Collapsed the gallery shell's `OverlayControlledOpenState` from seven named booleans into a fixed array keyed by `OverlayControlledSample`, removing the field list and repeated match arms while preserving the same controlled overlay behavior.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed with 48/48 tests after the controlled-open state refactor.
* **Decision**: Keep scanning for the next evidence-backed seam; the shell state is now compact enough that further changes should come from a real contract split, not representation cleanup.

* **Update**: Added a state-driven tooltip smoke for the delayed/manual sample, so the overlay gallery proves the tooltip content renders directly from gallery state without pointer interaction.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the tooltip state smoke addition.
* **Decision**: Keep the overlay coverage focused on real behavior paths; avoid extra shell-side duplication unless a new contract split appears.

* **Update**: Removed the `OverlayBehaviorSample.adapter` duplicate ownership seam from the overlay gallery. `shell.rs` now derives the GPUI adapter from `OverlayResolvedState::resolve(policy)` at render time, so the sample only owns policy plus display metadata.
* **Update**: Added tooltip gallery debug selectors and a smoke test that covers hover, focus, and disabled behavior. The new test makes the tooltip content addressable from the gallery automation path instead of leaving it as an uncloseable popup.
* **Verification**: `cargo fmt --all`, `cargo nextest run -p open-gpui-ui-components --tests` (147/147), and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` (48/48) passed after the overlay/tooltip cleanup.
* **Decision**: Keep scanning for the next evidence-backed seam, especially any remaining sample/state duplication that can be deleted instead of preserved.

* **Update**: Consolidated the GalleryShell overlay controlled-open booleans into a single `OverlayControlledOpenState` with `OverlayControlledSample` selectors, and added hover card debug selectors plus a real hover-card smoke test so the controlled overlay families now share the same automation shape.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed with 47/47 tests.
* **Next Action**: commit the shell/state/refinement pass now that the subagent review gap is closed.

* **Update**: Re-read `repo-ref/fret`'s diag layering pattern and compared it with the gallery overlay `Menu` / `ContextMenu` line (`render_menu_sample_card` / `render_context_menu_sample_card`).
* **Finding**: The diag repo is a thin-entry-point / deep-implementation example, but the overlay menu/context-menu code is still page-local reconstruction glue, not a stable shared contract seam.
* **Decision**: Do not extract a page-local helper/module for overlay menus in this pass. Keep the gallery code as-is and look for a stronger seam if future reuse work creates one.

* **Update**: Re-read `repo-ref/fret`'s diag layering pattern and compared it with the gallery Components choice family (`Select` / `Combobox` / `Command`).
* **Finding**: The diag repo is a thin-entry-point / deep-implementation example, but the gallery choice family is still page-local reconstruction glue, not a stable shared contract seam.
* **Decision**: Do not extract a page-local choice module in this pass. Keep the gallery code as-is and look for a stronger seam if future headless or cross-platform reuse work creates one.

* **Update**: Rechecked the `Tabs` / `ScrollArea` / `Splitter` seam against the gallery smoke tests and the page composition layer.
* **Finding**: These three components are already deep enough on their own; the real scroll, viewport, and vertical-layout rules live in gallery shell composition rather than a shared component helper.
* **Verification**: `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed 45/45.
* **Decision**: Do not extract a shared layout helper from these three components in this pass. Only revisit `render_components_page` / gallery shell composition if a new evidence-backed seam appears.

## 2026-06-18
* **Update**: Restored the Components gallery shell's `Select` / `Combobox` / `Command` active-state propagation so the visible samples consume `state.active_value()` instead of silently flattening the behavior to `selected` alone.
* **Verification**: `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed 45/45 after the active-state propagation fix.
* **Decision**: Keep the architecture pass evidence-backed; the remaining known seams still look narrower than the `active` contract gap that was just fixed.
* **Update**: Added `component_gallery_shell_reads_choice_active_metadata_from_resolved_state()` to lock the Components gallery shell rows to resolved-state `selected` / `active` metadata for `Listbox`, `Select`, `Combobox`, and `Command`.
* **Verification**: `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed 45/45. `git diff --check` only reported the expected CRLF warning on the edited test file.
* **Note**: The requested `repo-ref/fret` reference is not present in this workspace; the only local `repo-ref` checkout is `nako-scraper`, so the fret diag example could not be re-read here.
* **Update**: Re-reviewed the Components page sample/state surface and found no evidence-backed deletion seam comparable to the overlay focus contract. `TabsSample`, `ToolbarSample`, `SidebarSample`, `ListboxSample`, `SelectSample`, `ComboboxSample`, `CommandSample`, `TextInputSample`, and `FieldSample` are already either pure sample material or resolved state.
* **Decision**: Stop the seam hunt on Components for now and only revisit if a new sample/state mismatch is surfaced by tests or subagent review.
* **Update**: Restored overlay menu/context-menu sample-owned `focused_value` metadata so controlled examples can request initial focus from the sample struct, and the gallery shell now treats that request as optional when rebuilding controlled demos.
* **Verification**: `cargo fmt --all --check` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the overlay sample-contract cleanup.
* **Decision**: Keep the remaining overlay contract work scoped to the sample/shell boundary unless a stronger evidence-backed seam appears.
* **Update**: Moved the gallery left navigation off the ad hoc `navigation_scroll` handle and onto `ScrollArea` scroll semantics, so the shell no longer owns a second manual scroll path alongside page scrolling.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the navigation-scroll cleanup.
* **Decision**: Keep the architecture loop narrow unless a new evidence-backed duplication seam appears; otherwise move on to the next product slice.
* **Update**: Moved the gallery page scroll reset off the `GalleryShell` ad hoc `page_scroll` handle and onto `ScrollArea` reset-key semantics, so page switching now uses the same scroll contract as the inner scroll views.
* **Verification**: `cargo fmt --all --check` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the page-scroll cleanup.
* **Decision**: Keep the architecture loop narrow unless a new evidence-backed duplication seam appears; otherwise move on to the next product slice.
* **Update**: Deleted the remaining `Select` helper wrapper that only forwarded `selected` / `active` values into `Listbox`; the render path now applies those values inline.
* **Update**: Added direct tests that lock `Menu` / `ContextMenu` default open focus to the first focusable item, so the shared entry-focus rule is now covered by the component suite.
* **Verification**: `cargo fmt --all --check`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed after the Select / Menu cleanup.
* **Decision**: Keep the architecture loop narrow unless a new evidence-backed duplication seam appears; otherwise move on to the next product slice.
* **Update**: Rechecked `Menu` / `ContextMenu` against `repo-ref/fret`'s `entry_focus` pattern. The current code does not expose modality as a separate public input, so `first_focusable_value()` is the correct stopping point for this pass rather than another extracted helper.
* **Decision**: Stop chasing a deeper `Menu` / `ContextMenu` seam unless a new evidence-backed duplication appears.
* **Update**: Rechecked the current gallery seams against `repo-ref/fret` and confirmed the only clear shared-rule seam left is `Menu` / `ContextMenu` first-focus handling.
* **Decision**: Do not keep deleting overlay sample titles / descriptions / action labels or `TabsSample.title`; those are still constructor inputs or page-card copy, not duplicated resolved state.
* **Decision**: Keep `ScrollAreaState`, `ListboxState`, `SelectState`, `ComboboxState`, and `CommandState` as they are for this pass. They are already deep enough and should not be split further just to create a shallower helper.
* **Update**: Removed the one-off `apply_optional_values` helper from `Combobox` and inlined the selected/active propagation into the render path.
* **Update**: Added `combobox_state_scrollable_content_tracks_filtered_option_count()` so the filtered-option scroll contract is locked by a unit test, alongside the existing listbox threshold test.
* **Decision**: Do not chase `Tabs` / `Toolbar` / `Sidebar` item arrays as deletion seams. They are builder inputs for gallery reconstruction, not duplicated state.
* **Update**: Added `ListboxState::scrollable_content()` and moved the `Select` / `Combobox` scrollability threshold onto the shared listbox state instead of duplicating `> 6` checks in each adapter.
* **Verification**: `cargo fmt --all --check`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the listbox scrollability cleanup.
* **Decision**: Keep the architecture loop narrow. The current subagent review says the remaining `apply_optional_values` builder sugar is not deep enough to extract, so do not keep chasing that seam.
* **Update**: Removed the duplicated local `first_focusable_value` helper from `Menu` / `ContextMenu` by moving the lookup onto `MenuState::first_focusable_value()`. The gallery now consumes the same state-owned entry-focus contract from both code paths.
* **Verification**: `cargo fmt --all --check`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the Menu / ContextMenu entry-focus cleanup.
* **Decision**: Keep the architecture loop narrow. The next pass should only continue if a new evidence-backed duplicate seam appears; otherwise move on to the next product slice.
* **Update**: Used `repo-ref/fret` as the local architecture reference and applied the useful pattern back to the current gallery pass: thin shell reconstruction, resolved state as the behavior contract, and pure helper seams only when they remove duplicated policy.
* **Update**: Added `ListboxState::standalone_options()` and `ListboxState::group_options()` so Listbox / Select / Combobox gallery reconstruction consumes state-owned grouping views instead of filtering `group_index()` in the shell.
* **Update**: Removed the unused Combobox options helper after moving reconstruction to `ListboxState` grouping views. Command continues to use the explicit `CommandState` standalone/grouped views.
* **Subagent Finding**: Captured the architecture review in `docs/knowledge/engineering/subagents/gallery-architecture-review-20260618.md`. The accepted next candidate is shared `Menu` / `ContextMenu` entry-focus logic; `ScrollAreaState` and the current choice/command state seams are deep enough for this pass.
* **Verification**: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed.
* **Decision**: Continue only on evidence-backed deletion seams. The next likely seam is menu/context-menu entry focus, not further splitting scroll or choice collection state.

* **Update**: Read the local reference repo `repo-ref/fret` and confirmed the design pattern we should borrow is layering, not surface shape: thin entry points, real implementation crates underneath, and pure helper modules for viewport / visibility / overflow / scroll math. In particular, `crates/fretboard/src/diag.rs` is only a forwarder and `crates/fret-diag` is the substantive implementation crate.
* **Decision**: For future scroll work, keep `ScrollAreaState` as the gallery seam instead of re-deriving scroll policy in the shell. If we need to deepen scrolling further, prefer a pure helper for viewport containment / overflow membership / scroll-into-view math.
* **Update**: Continued the gallery contract cleanup by making the command palette's synthetic standalone group explicit in resolved state and adding iterator views for standalone items, grouped groups, and group items. The gallery shell now rebuilds command UI from resolved state views instead of splitting on a local magic-string seam.
* **Update**: Re-reviewed the overlay menu/context-menu contract and kept sample-owned `focused_value` metadata in the current shell implementation, because controlled reconstruction needs the original request value instead of reading it back from resolved state.
* **Verification**: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` all passed.
* **Update**: Re-reviewed the gallery Components/Overlay sample-state contract surface and confirmed there is no fresh evidence-backed deletion seam beyond the already-cleaned command standalone group and the current resolved-state ownership split. One earlier `TextInputSample.controller_driven` deletion attempt was rolled back after confirming that field is still the sample-side controller mount switch.
* **Verification**: The current gallery review pass remains green with `cargo fmt --all`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`.
* **Decision**: Keep searching only for real ownership splits that can be deleted or moved into resolved state without losing gallery-specific behavior.
* **Update**: Read `repo-ref/fret` as the current reference baseline for scroll/viewport design. The practical lesson is not the `diag` command surface itself, but the layering behind it: thin entry points, real implementation crates, and headless pure helpers for visibility / overflow / viewport math.
* **Update**: Confirmed that `ScrollAreaState` in `crates/ui_components/src/scroll_area.rs` is already a deep enough seam for the gallery to depend on directly. It owns axis, reset policy, reset key, and scroll-direction decisions, so the shell should not keep re-deriving scroll policy.
* **Verification**: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`, `cargo check -p open-gpui-ui-components --tests`, and `cargo nextest run -p open-gpui-ui-components --tests` all passed after the current state-first cleanup.
* **Decision**: If the scroll story gets deeper, add a pure helper for viewport containment / overflow membership / scroll-into-view math; do not grow more gallery-local visibility branching.
* **Update**: Continued the Components gallery architecture pass by making the command palette's synthetic standalone group explicit in resolved state with `CommandGroupState::standalone()`. The gallery shell now rebuilds command items/groups from that explicit flag instead of the `commands`/`Commands` magic-string seam.
* **Update**: Re-reviewed the low-state primitives (`Separator`, `Kbd`, `Progress`, `Skeleton`, `Avatar`) and confirmed there was no additional deletion value in moving their remaining visible copy into state; they are already at the right level of surface metadata.
* **Verification**: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` passed after the command seam cleanup.
* **Decision**: Keep scanning the Components page for only evidence-backed deletion seams. Prefer changes that remove a real contract split over renaming fields that are already doing useful display work.

* **Update**: Removed the redundant sample-side `open_mode` fields from the Overlay gallery
  samples (`HoverCard`, `Popover`, `Dialog`, `AlertDialog`, `Sheet`, `Menu`, and `ContextMenu`)
  and made `shell.rs` read `state.open_mode()` for controlled/uncontrolled reconstruction. The
  overlay sample structs now carry only resolved state plus display metadata. Menu and ContextMenu
  item lists are now reconstructed from `MenuState` / `ContextMenuState` instead of carrying a
  second sample-side descriptor tree, so the shell no longer keeps duplicate ownership or item
  sources.
* **Verification**: `cargo fmt --all --check` and focused `cargo nextest run -p
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
* **Decision**: Treat overlay sample `open_mode` as a duplicate of resolved state in this gallery
  shape; reopen the seam only if a second independent source of truth appears.
* **Update**: Removed the redundant sample-side `open_mode` fields from the overlay gallery
  samples (`HoverCard`, `Popover`, `Dialog`, `AlertDialog`, `Sheet`, `Menu`, and `ContextMenu`)
  and made `shell.rs` read `state.open_mode()` for controlled/uncontrolled reconstruction. The
  overlay sample structs now carry only resolved state plus display metadata, so the shell no
  longer keeps a second open-ownership source.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests` with 43/43 passing.
* **Decision**: Keep the architecture pass moving only if a new evidence-backed asymmetry appears;
  otherwise move to the next visible sample/state seam.
* **Update**: Tightened the Components gallery mount policy so Select / Combobox / Command do not
  mount open during gallery shell render. The resolved component state still shows in the state
  rows, but the transient surfaces stay closed on mount so page scroll works again.
* **Verification**: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`,
  and full `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` with 43/43 passing.
* **Update**: Removed the redundant sample-side `open_mode` fields from the Select / Combobox /
  Command gallery samples and made `shell.rs` read `state.open_mode()` for mount policy. Also
  deleted the pure `official_component_sample_selectors()` test wrapper so the conformance test
  iterates the canonical selector source directly.
* **Verification**: `cargo fmt --all --check`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery official_component_catalog_entries_have_signals_and_sample_selectors
  components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation --no-capture`, and
  full `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` with 43/43 passing.
* **Update**: Read `repo-ref/fret` as the local reference repository. The important boundary is
  that `crates/fretboard/src/diag.rs` is only a thin public-CLI forwarder, the real diagnostics
  implementation sits in `crates/fret-diag`, and viewport-aware scroll handling lives in
  `crates/fret-ui/src/declarative/host_widget.rs` with no-drift coverage in
  `crates/fret-ui/src/tree/tests/scroll_into_view.rs`.
* **Decision**: Use the reference pattern as the model for gallery helpers: stable `test_id`
  targeting plus explicit viewport containment checks before scrolling.
* **Update**: Continued the gallery contract pass by removing the redundant sample-side open-state
  fields from the Components page and keeping select/combobox/command mount-state behavior as a
  gallery-local adapter policy. The gallery shell no longer stores a duplicate `page_load_open`
  field; the page tests now only assert the resolved state while the shell keeps those popups
  closed on mount.
* **Verification**: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery
  --tests`, and `cargo nextest run -p open-gpui-ui-foundation-gallery` with 43 passing tests.
* **Update**: Continued the gallery contract pass by deleting the duplicated Sidebar sample-side
  field set, adding `size` to the resolved `SidebarState` contract, and making the gallery shell
  render sections/items from `SidebarState.sections()` and `SidebarState.items()` instead of a
  second sample tree. The vertical Splitter sample now starts in a real collapsed state and the
  gallery smoke proves it can restore and keep resizing after a second drag.
* **Verification**: The Sidebar/Splitter cleanup passed `cargo fmt --all --check`, `cargo check -
  p open-gpui-ui-foundation-gallery --tests`, and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata
  components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition
  components_gallery_smoke_scroll_area_samples_scroll_inside_page
  components_gallery_smoke_sidebar_long_navigation_scrolls_inside_sample`.
* **Update**: Inspected `repo-ref/fret` to map the diagnostics and scroll automation design.
  `crates/fretboard/src/diag.rs` is only a thin CLI forwarder; the actual implementation sits in
  `crates/fret-diag`, while viewport-aware scroll handling lives in
  `crates/fret-ui/src/declarative/host_widget.rs` (`scroll_viewport_bounds` and
  `scroll_handle_into_view`) and the no-drift coverage lives in
  `crates/fret-ui/src/tree/tests/scroll_into_view.rs`. The important automation pattern is stable
  `test_id` targeting plus explicit viewport containment checks before issuing scrolls.
* **Decision**: Use the `fret` pattern as the reference model for our gallery helpers and
  automation. Prefer a unified scroll-into-view helper over accumulating more wheel-event loops.
* **Update**: Continued the Components gallery contract pass by making `CommandSample` loading
  metadata the single source of truth for both the sample and its resolved `CommandState`. The
  gallery no longer reconstructs loading state from the `query == "deploy"` sentinel; popup mount
  state is now derived directly from each sample's resolved component state.
* **Update**: Continued the Sidebar gallery contract pass by deleting the duplicated sample-side
  section tree. `SidebarSample` now keeps only display metadata plus resolved `SidebarState`, and
  `shell.rs` rebuilds section/item rendering from `SidebarState.sections()` and
  `SidebarState.items()` instead of reading a second sample structure.
* **Verification**: The Sidebar contract cleanup passed `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, focused Sidebar nextest coverage, and full `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Verification**: The Command loading cleanup passed `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, focused Components gallery nextest coverage for
  metadata/choice/search/scroll reset, full `cargo nextest run -p
  open-gpui-ui-foundation-gallery` with 43 passing tests, and `git diff --check`.
* **Update**: Closed the overlay gallery scroll/navigation regression by stopping the gallery
  shell from auto-expanding uncontrolled overlay previews that block page interaction. The sample
  contracts still keep `default_open` metadata, but the gallery now leaves those previews closed so
  the page can scroll and navigation can switch cleanly.
* **Verification**: `cargo fmt --all` and `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests` with 43 passing tests.
* **Update**: Continued the overlay gallery contract cleanup by moving menu and context-menu
  initial focused-item intent into explicit sample metadata. `MenuSample` and
  `ContextMenuSample` now carry `focused_value`, and the gallery shell feeds that value into the
  rendered menu builders instead of reconstructing it from resolved state.
* **Verification**: The focused-value cleanup passed `cargo fmt --all --check`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts`.
* **Update**: Continued the overlay ownership cleanup by adding explicit `open_mode` metadata to
  the hover-card, popover, dialog, alert-dialog, and sheet samples, and by keeping `Command`'s
  loading metadata sample-owned instead of shell-inferred. The gallery shell now routes these
  cases from sample contracts instead of `sample.id` branches.
* **Verification**: The overlay and command ownership cleanup passed `cargo fmt --all`, focused
  `cargo nextest run -p open-gpui-ui-foundation-gallery` checks for hover-card, popover, dialog,
  alert-dialog, sheet, and command contract tests, full `cargo nextest run -p
  open-gpui-ui-foundation-gallery` with 43 passing tests, and `cargo check -p
  open-gpui-ui-foundation-gallery --tests`.
* **Update**: Continued the overlay gallery ownership cleanup by adding explicit `open_mode`
  metadata to the hover-card, popover, dialog, alert-dialog, and sheet samples. The gallery shell
  now routes controlled versus uncontrolled behavior from sample-owned contract fields instead of
  inferring those cases from `sample.id`.
* **Verification**: The overlay ownership cleanup passed `cargo fmt --all`, focused `cargo nextest
  run -p open-gpui-ui-foundation-gallery` checks for hover-card, popover, dialog, alert-dialog,
  and sheet contracts, full `cargo nextest run -p open-gpui-ui-foundation-gallery` with 43 passing
  tests, and `cargo check -p open-gpui-ui-foundation-gallery --tests`.
* **Update**: Removed the last implicit `sample_id` branching from the overlay gallery shell for
  menu and context-menu open ownership. Overlay samples now carry explicit `open_mode` metadata,
  and the shell routes controlled versus uncontrolled behavior from the sample-owned contract
  instead of inferring it from ids.
* **Verification**: The overlay open-mode cleanup passed `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts`, and full `cargo nextest run
  -p open-gpui-ui-foundation-gallery` with 43 passing tests.
* **Update**: Completed the latest overlay and gallery contract cleanup by moving menu and
  context-menu initial focus intent into explicit sample metadata, deleting a leftover overlay
  helper import, and centralizing stable labels through `as_str()` on the core and component
  vocabularies.
* **Verification**: The cleanup passed `cargo fmt --all`, `cargo check -p open-gpui-ui-components`,
  `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Completed the overlay gallery sample-contract cleanup by moving initial focused-item
  intent into explicit `MenuSample` and `ContextMenuSample` metadata, so the shell no longer
  reconstructs menu focus from closed runtime state. The same pass also removed duplicate overlay
  label helpers by routing stable labels through `as_str()` on the core/component vocabularies.
* **Verification**: The overlay contract cleanup passed `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo check -p open-gpui-ui-components --tests`, and
  `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery` with 211 passing tests.
* **Update**: Continued the overlay gallery architecture pass by moving `MenuSample` and
  `ContextMenuSample` initial focused-item intent into explicit sample metadata. The shell now
  reads `sample.focused_value` instead of reconstructing intent from resolved runtime state.
* **Verification**: The overlay sample intent cleanup passed `cargo fmt --all --check`, `cargo
  check -p open-gpui-ui-foundation-gallery --tests`, and focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
  overlay_page_context_menu_samples_expose_point_anchor_contracts`.
* **Update**: Continued the gallery architecture pass by adding `as_str()` labels to the core
  adaptive and sizing vocabularies (`Density`, `DeviceAdaptiveClass`, `PanelAdaptiveClass`, and
  `DeviceShellMode`) and switching the Sizing & Density gallery page plus the gallery shell to
  derive labels from the vocabulary itself instead of duplicate page-local label tables.
* **Verification**: The vocabulary label cleanup passed `cargo fmt --all`, `cargo nextest run -p
  open-gpui-ui-core --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`, and
  `cargo nextest run -p open-gpui-ui-components --tests`.
* **Update**: Continued the gallery architecture pass by adding `as_str()` labels to the core
  adaptive and sizing vocabularies (`Density`, `DeviceAdaptiveClass`, `PanelAdaptiveClass`, and
  `DeviceShellMode`) and switching the Sizing & Density gallery page to derive labels from the
  vocabulary itself instead of duplicate page-local label tables.
* **Verification**: The vocabulary label cleanup passed `cargo nextest run -p open-gpui-ui-core
  --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`, and `cargo nextest
  run -p open-gpui-ui-components --tests`.
* **Update**: Continued the gallery architecture pass by extracting a thin `gallery_card_shell`
  helper in `examples/ui-foundation-gallery/src/shell.rs`. The Components catalog cards and the
  low-state primitive sample cards now share the same outer shell instead of repeating the
  rounded/bordered/padded wrapper inline.
* **Verification**: The shell-helper extraction stayed green with `cargo fmt --all --check`, `cargo
  check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-components
  --tests`.
* **Update**: Moved the Components gallery catalog state-label fallback and status badge colors
  into `pages/components.rs`, so the shell now renders catalog entries from catalog-owned display
  helpers instead of re-deriving status presentation logic inline.
* **Verification**: The presentation cleanup stayed green with `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-components --tests`.
* **Update**: Deepened the Components gallery catalog so official sample selector metadata now
  lives on `COMPONENT_CATALOG`, and the gallery smoke derives its official selector pairs from that
  single source of truth instead of keeping a second selector table in the test layer.
* **Verification**: The catalog metadata deepening pass passed `cargo fmt --all`, `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery --tests`, and `cargo nextest run -p open-gpui-ui-components --tests`.
* **Update**: Finished the follow-up cleanup for the gallery selector unification pass. The
  remaining tabs ownership issue was fixed, the overlay sample cards now use the sample-owned
  debug selector helpers consistently, and the gallery test lifecycle bug was removed.
* **Verification**: The follow-up cleanup passed `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery
  --tests`, `cargo nextest run -p open-gpui --tests`, `cargo nextest run -p
  open-gpui-ui-components --tests`, `cargo fmt --all --check`, and `git diff --check`.
* **Update**: Unified gallery sample debug selectors so the Components and Overlay pages derive
  stable selector strings from sample-owned helpers instead of repeating family prefixes inline in
  the shell and tests. The Components gallery smoke now derives the official sample selector list
  from the sample builders and checks the visible catalog against the rendered page.
* **Verification**: The selector unification pass passed `cargo check -p
  open-gpui-ui-foundation-gallery --tests`, `cargo check -p open-gpui --tests`, `cargo check -p
  open-gpui-ui-components --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo run -p xtask -- verify`.
* **Update**: Started the second behavior-alignment loop after commit `ea9ffbc`. `ProgressState`
  now exposes `ProgressVisualMode`, indicator start fractions, and indicator width fractions.
  Indeterminate progress renders as a short non-percentage segment instead of a fixed 33% fill.
* **Update**: Added `progress:{id}:indicator` debug selectors and runtime bounds assertions for
  determinate and indeterminate progress, plus gallery state-row text that shows indicator
  start/width during manual dogfood.
* **Update**: Extended the Overlay gallery ContextMenu smoke to cover outside-press dismissal after
  real right-click opening, in addition to the existing Escape path.
* **Update**: Added focused debug selector observability to `open_gpui::VisualTestContext`.
  Tests can now call `debug_selector_is_focused` or `focused_debug_selector` after a draw to assert
  the actual rendered focus owner for any focusable element with a debug selector.
* **Update**: Aligned Popover and Dialog GPUI adapters with their neutral overlay contracts.
  Popover/Dialog triggers now use persistent focus handles, Dialog moves focus to its surface by
  default, dismissals restore focus to the trigger according to `FocusRestoreIntent`, and Popover's
  default initial focus now follows the non-modal overlay default of `InitialFocusIntent::None`.
* **Update**: Strengthened Overlay gallery smoke so controlled Popover and Dialog are opened via
  their real component triggers and assert focus restoration after outside press, modal barrier
  dismissal, and Escape dismissal.
* **Verification**: The second behavior-alignment loop passed focused `cargo nextest run -p
  open-gpui-ui-components progress_state_clamps_values_and_preserves_indeterminate_mode
  low_state_primitives_render_stable_debug_selectors`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery
  overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses`, focused focus-restore
  checks for `open-gpui`, `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`, and
  `cargo run -p xtask -- verify`.
* **Update**: Executed
  `docs/plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md`. The component
  crate default root and prelude now avoid adapter-only GPUI helper exports; those helpers are
  intentionally grouped under `open_gpui_ui_components::gpui_adapter`.
* **Update**: Preserved `open_gpui_ui_components::text_input` as the official module for
  `TextInput`, `TextInputState`, `TextInputColors`, and `TextInputMetrics`, while moving the
  GPUI-backed `TextInputController` and text-input key binding initialization behind the internal
  adapter module and public `gpui_adapter` facade.
* **Update**: Added neutral `Role::Image`, mapped it through the GPUI a11y adapter, and changed
  `AvatarState::role()` from label semantics to image semantics.
* **Update**: Strengthened Components gallery contract automation so official catalog entries must
  align with component/state signals, stable sample selectors, selector family prefixes, and the
  rendered full-page smoke.
* **Verification**: Passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, full `cargo nextest run -p
  open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` with 207
  passing tests, local engineering-wiki structure validation, `git diff --check`, and
  `cargo run -p xtask -- verify`.

## 2026-06-17
* **Update**: Completed U5 of `docs/plans/2026-06-17-004-feat-ui-component-completion-plan.md`.
  Promoted `Separator`, `Kbd`, `Progress`, `Skeleton`, and `Avatar` from deferred catalog entries
  to official Components gallery entries, added visible gallery sample factories/sections for each
  primitive, exposed stable `gallery:component-*-sample:{id}` debug selectors, and extended
  metadata/smoke coverage for catalog status, resolved state rows, and short-viewport scrolling.
* **Verification**: U5 passed `cargo fmt -p open-gpui-ui-foundation-gallery`, `cargo check -p
  open-gpui-ui-foundation-gallery`, focused `cargo nextest run -p
  open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata
  components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation`, and the full
  `cargo nextest run -p open-gpui-ui-foundation-gallery` with 42 passing tests.
* **Update**: Completed U4 of `docs/plans/2026-06-17-004-feat-ui-component-completion-plan.md`.
  Added the `Avatar` primitive to `open-gpui-ui-components` with a resolved-state-first contract
  for display name, fallback initials or explicit fallback text, renderer-neutral source metadata,
  accessible label, size metrics, and theme intents. The GPUI adapter renders a stable circular
  avatar root with explicit debug selectors while keeping image loading and cache state outside
  the primitive contract.
* **Verification**: U4 passed `cargo fmt -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components avatar`, and the full
  `cargo nextest run -p open-gpui-ui-components` with 138 passing tests.
* **Update**: Completed U1 of `docs/plans/2026-06-17-004-feat-ui-component-completion-plan.md`.
  The Components gallery now exposes an official component completion catalog with
  `component-catalog:{name}` debug selectors, and the official-component checklist is recorded in
  `docs/ui/component-contract.md` plus `docs/verification.md`.
* **Verification**: U1 passed `cargo fmt -p open-gpui-ui-foundation-gallery`, `cargo check -p
  open-gpui-ui-foundation-gallery`, and focused gallery nextest coverage for component metadata,
  conformance gates, and short-viewport navigation reset.
* **Update**: Completed U2 of the component completion plan. Added rendered runtime tests for
  standalone controller-backed `TextInput`, filtered keyboard `Combobox` selection, and
  dialog-backed `Command` open/filter/select plus Escape and outside-press dismissal. Updated
  `docs/verification.md` so the automation matrix reflects the new runtime coverage.
* **Verification**: U2 passed `cargo fmt -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-components` with 127 passing
  tests.
* **Update**: Completed U3 of the component completion plan. Added `Separator`, `Kbd`, `Progress`,
  and `Skeleton` to `open-gpui-ui-components` with resolved state, metrics, token intents, explicit
  root/prelude exports, stable rendered debug selectors, and focused tests. Added neutral
  `Role::Separator` to UI core; the current GPUI adapter maps it to the nearest available
  AccessKit role until the bundled AccessKit role enum exposes a separator role.
* **Verification**: U3 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`, `cargo
  check -p open-gpui-ui-core -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-core -p open-gpui-ui-components` with 157 passing tests.
* **Update**: Wrote the next UI component completion plan at
  `docs/plans/2026-06-17-004-feat-ui-component-completion-plan.md`. The plan keeps ADR 0008's
  current-crate product boundary, defines an official-component completion checklist, targets
  rendered runtime gaps for existing complex widgets, and schedules the low-state primitives
  `Separator`, `Kbd`, `Progress`, `Skeleton`, and `Avatar` before heavier widgets.
* **Verification**: Planning-only update passed documentation self-review and `git diff --check`
  for the new plan file. No Rust build or tests were run.
* **Update**: Added rendered Combobox and Command search interaction automation. `TextInput` now
  exposes `text-input:{id}:root`; `Combobox` exposes root/input-row/toggle/content selectors; and
  `Command` exposes root/trigger/content selectors so input-driven components can be tested through
  real pointer focus and `simulate_input`.
* **Update**: The Combobox smoke clicks the controller-backed input, types `re`, verifies the popup
  remains closed until the toggle opens it, checks filtered Listbox options, selects Remix by click,
  and verifies ordered `ComboboxSelection` plus close callbacks.
* **Update**: The Command smoke clicks the controller-backed input, types `file`, verifies inline
  filtering, selects Open File with Down+Enter, verifies the shortcut payload, and confirms non-dialog
  command content stays open after selection.
* **Verification**: Focused Combobox/Command smokes passed with `cargo fmt -p
  open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components
  combobox_runtime_filters_input_and_selects_filtered_option`, `cargo nextest run -p
  open-gpui-ui-components command_runtime_filters_input_and_selects_with_keyboard`, and the full
  `cargo nextest run -p open-gpui-ui-components` with 124 passing tests.
* **Update**: Added Select runtime keyboard automation. The focused smoke opens the real Select
  trigger, verifies disabled popup options do not select or close the popup, selects an enabled
  option by click, reopens the popup, moves through the embedded Listbox with keyboard navigation,
  skips disabled rows, selects with Enter, and verifies `SelectSelection` payloads plus open-change
  callbacks.
* **Update**: Fixed Select popup keyboard navigation by no longer passing the parent-derived
  `active_value` into the embedded Listbox as a controlled active prop. Explicit
  `Select::active(...)` still controls popup Listbox active state, while the uncontrolled popup
  runtime owns active-descendant movement after user navigation.
* **Verification**: Focused Select runtime smoke passed with `cargo fmt -p
  open-gpui-ui-components` and `cargo nextest run -p open-gpui-ui-components
  select_runtime_click_and_keyboard_selection_close_popup_and_emit_payloads`.
* **Update**: Added Listbox runtime keyboard automation. `Listbox` now exposes stable runtime debug
  selectors for the root, empty state, groups, separators, and options, and
  `open-gpui-ui-components` has a real rendered Listbox smoke that rejects disabled option clicks,
  verifies standalone/grouped option payloads, keeps arrow navigation selection-free, skips
  disabled/separator rows, and activates the focused option with Enter.
* **Update**: Fixed Listbox keyboard activation parity so Enter/Space dispatch the option-level
  `on_select` handler before the listbox-level handler, matching the click path.
* **Verification**: Focused Listbox runtime smoke passed with `cargo fmt -p
  open-gpui-ui-components` and `cargo nextest run -p open-gpui-ui-components
  listbox_runtime_click_and_keyboard_selection_skip_disabled_items`.
* **Testing Note**: The first Listbox smoke attempt controlled `active("alpha")`, which correctly
  prevented runtime arrow navigation from changing the active option. The final smoke only seeds
  `selected("alpha")` so the rendered runtime owns active-descendant movement.
* **Update**: Added RadioGroup runtime keyboard automation. `RadioGroup` now exposes stable runtime
  debug selectors for the root and items, and `open-gpui-ui-components` has a real rendered
  RadioGroup smoke that rejects disabled clicks, verifies click payloads, skips disabled items with
  arrow navigation, and confirms Space on an already selected radio does not emit a duplicate
  selection change.
* **Verification**: Focused RadioGroup runtime smoke passed with `cargo nextest run -p
  open-gpui-ui-components
  radio_group_runtime_keyboard_navigation_skips_disabled_items_and_payloads`.
* **Review**: `/root/radio_group_runtime_review` flagged that `End+Space` could not prove a Space
  activation payload because RadioGroup arrow/Home/End navigation selects immediately. The smoke and
  docs now verify Space as a no-duplicate path on the already selected radio instead.
* **Update**: Added Tabs runtime keyboard automation and fixed rendered `Tabs` state hydration.
  `Tabs::render` now seeds its runtime from the builder-selected value on first render and tracks
  per-tab focus handles on the actual trigger elements, so Manual keyboard navigation can move
  focus before Enter activation.
* **Verification**: The focused Tabs runtime smoke first failed on the missing selected seed, then
  passed after the runtime seed/focus-handle fix with `cargo nextest run -p
  open-gpui-ui-components
  tabs_runtime_manual_keyboard_activation_preserves_selected_seed_and_payloads`.
* **Review**: `/root/tabs_runtime_review` found no blocking issues and confirmed the selected seed
  only initializes runtime state, per-tab focus handles are bound to real triggers, and the smoke
  covers seed, disabled click, Manual focus-only arrows, Enter payload, and Home+Enter payload.
  The remaining non-blocking Space activation gap was closed by adding an End+Space path that
  activates the last enabled tab.
* **Update**: Added Toolbar runtime keyboard automation. `Toolbar` now exposes stable runtime debug
  selectors for the root and items, and `open-gpui-ui-components` has a real rendered Toolbar smoke
  that clicks the first action item, moves roving focus with arrow/Home keys, skips disabled and
  separator items, and verifies Enter activation payloads.
* **Review**: `/root/toolbar_keyboard_review` flagged that the initial Toolbar smoke only moved from
  `bold` to `italic` and therefore did not prove disabled/separator skipping. The test now starts
  from `undo`, moves right across disabled `redo` plus the separator, and activates `bold`.
* **Verification**: Focused Toolbar runtime smoke passed with `cargo fmt -p
  open-gpui-ui-components` and `cargo nextest run -p open-gpui-ui-components
  toolbar_runtime_keyboard_navigation_skips_disabled_and_separator_items`.
* **Update**: Added a gallery-level Sidebar internal-scroll smoke. `Sidebar` now exposes stable
  runtime debug selectors for the root and items, the gallery Sidebar/Toolbar cards expose gallery
  sample selectors, and the Components smoke scrolls the long Sidebar viewport while asserting the
  bottom navigation item moves relative to its sample card.
* **Verification**: Focused Sidebar gallery smoke passed with `cargo fmt -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery` and `cargo nextest run -p
  open-gpui-ui-foundation-gallery
  components_gallery_smoke_sidebar_long_navigation_scrolls_inside_sample`.
* **Update**: Promoted the UI foundation/component runtime gates into the default `xtask verify`
  path. The gate now runs `cargo nextest run -p open-gpui-ui-core`, `cargo nextest run -p
  open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery` after the
  workspace checks and before the import-boundary scan.
* **Verification**: The verify-gate promotion passed `cargo fmt -p xtask`, `cargo nextest run -p
  xtask`, `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo run -p xtask -- verify`, engineering wiki validation,
  and `git diff --check` with only CRLF warnings.
* **Update**: Added a compact shell/navigation gallery smoke. The runtime test clicks the compact
  viewport switch, resizes the test window to the compact width, verifies the shell snapshot enters
  mobile/compact mode, scrolls the left navigation rail to deep pages, and confirms switching away
  and back to Components resets page scroll.
* **Verification**: Compact shell/navigation smoke passed as part of `cargo nextest run -p
  open-gpui-ui-foundation-gallery` with 41 passing tests, after `cargo fmt -p
  open-gpui-ui-foundation-gallery` and `cargo check -p open-gpui-ui-foundation-gallery`.
* **Update**: Hardened the Overlay gallery ContextMenu runtime smoke from gallery-control opening
  to the real right-click path. The test now scrolls the controlled ContextMenu hotspot into view,
  sends a right mouse down/up pair through `open_gpui::test`, asserts the surface opens, and closes
  it with Escape.
* **Verification**: The focused right-click ContextMenu smoke passed with `cargo nextest run -p
  open-gpui-ui-foundation-gallery
  overlay_gallery_smoke_opens_context_menu_from_right_click_and_closes_from_escape`.
* **Verification**: Final gate for the ContextMenu right-click smoke hardening passed `cargo fmt
  -p open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, engineering wiki validation, and `git diff --check` with only
  CRLF warnings.
* **Update**: Added a gallery-level Overlay interaction smoke gate. The new runtime tests drive
  controlled Popover outside dismissal, modal Dialog barrier and Escape dismissal, non-modal Sheet
  outside dismissal, Menu Escape/outside dismissal, and ContextMenu right-click hotspot
  open/Escape dismissal through `open_gpui::test`.
* **Decision**: Overlay gallery default-open contract samples now render visually closed at page
  load so floating overlays and modal barriers do not block page scrolling or later samples. The
  state rows still expose the resolved default-open contracts for metadata verification.
* **Verification**: Overlay smoke work passed `cargo fmt -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-foundation-gallery
  overlay_gallery_smoke`, `cargo nextest run -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only CRLF warnings.
* **Update**: Added a gallery-level Components interaction smoke gate. The gallery tests now render
  the full Components page and drive short-viewport page scrolling/navigation reset, Select popup
  outside dismissal, nested ScrollArea wheel scrolling, vertical Tabs rail scrolling, and Splitter
  pointer dragging through `open_gpui::test` runtime events.
* **Verification**: The gallery smoke gate passed `cargo nextest run -p
  open-gpui-ui-foundation-gallery`. `docs/verification.md` now names these automated smoke paths
  before the remaining manual dogfood checklist.
* **Update**: Hardened the Components gallery interaction dogfood surface after manual feedback:
  vertical Tabs triggers now remain non-shrinking in constrained tablists, the gallery vertical Tabs
  sample has enough items to force overflow, and the vertical Splitter sample starts expanded so
  drag resizing can be exercised directly.
* **Verification**: Added runtime event tests for horizontal and two-axis ScrollArea wheel behavior,
  constrained vertical Tabs scrolling, and both horizontal and vertical Splitter dragging. Verified
  with `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo check -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, and `git diff --check` with only CRLF
  warnings.
* **Decision**: Current component-level runtime automation is strong enough for these regressions.
  The next meaningful enhancement is gallery-level visual smoke coverage for overlay dismissal,
  scroll containers, and splitter dragging once UI churn makes screenshot drift worth the cost.
* **Update**: Continued the ADR 0008 productization roadmap through U2-U6 by validating that the
  current crates already contain the runtime foundation, interaction/layout, shell/navigation,
  choice/search, and gallery gate work described by the plan. The follow-up code change tightened
  theme table coverage for dark/high-contrast state-specific intents and added a direct Command
  popup ScrollArea preserve-scroll assertion.
* **Verification**: Productization pass currently passes `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
* **Decision**: Added ADR 0008 and recentered the active UI component roadmap on current-crate
  productization. `open-gpui-ui-core`, `open-gpui-ui-components`, and
  `examples/ui-foundation-gallery` are the product boundary for the next phase; standalone
  `open-gpui-ui-headless` extraction is deferred unless a future plan explicitly reopens it.
* **Update**: Wrote the productization roadmap plan at
  `docs/plans/2026-06-17-003-feat-ui-component-productization-roadmap-plan.md` and added memory
  decision `docs/knowledge/engineering/decisions/open-gpui-ui-productization-roadmap.md`.
* **Next**: Finish U1 documentation alignment, then continue with runtime foundation and
  interaction-family hardening rather than behavior-crate extraction.
* **Update**: Completed the strict UI-core headless boundary plan through the design checkpoint.
  Adaptive policy uses neutral `UiPx`; UI-core `UiPx` no longer implements GPUI style conversions;
  `open-gpui-ui-components::gpui_adapter` exports explicit `gpui_px_from_ui`,
  `gpui_point_from_ui`, and `gpui_size_from_ui`; and `open-gpui-ui-core` dropped its `open_gpui`
  manifest dependency.
* **Decision**: `open-gpui-ui-headless` remains deferred even though the strict UI-core boundary is
  clean. The next work should be a narrow behavior extraction plan that moves one family at a time,
  starting with overlay policy, roving focus, listbox navigation/typeahead, scroll viewport intent,
  or splitter constraints.
* **Update**: Added ADR 0007 as the post-boundary extraction design gate. Its ownership matrix
  keeps `TextInputController`, `ScrollHandle`, `focus_ring_shadow`, `GpuiOverlayState`, GPUI
  geometry conversion helpers, concrete focus handles, GPUI render trees, and AccessKit node wiring
  adapter-owned.
* **Verification**: The strict boundary slice passed `cargo check -p open-gpui-ui-core`, `cargo
  check -p open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, focused
  nextest for UI-core boundary guards, component adapter export guards, and the gallery headless
  checkpoint test. `git diff --check` passed with only CRLF warnings.
* **Update**: Wrote the extraction-prep plan at
  `docs/plans/2026-06-17-001-refactor-ui-headless-extraction-prep-plan.md`. It keeps
  `open-gpui-ui-headless` deferred and breaks the next series into guard inventory, neutral
  geometry, neutral component metrics, focus/a11y facades, overlay-state splitting, adapter-only
  API classification, and a final ADR 0006 checkpoint.
* **Decision**: The next implementation should not create a headless crate yet. It should remove
  or classify the remaining public-boundary blockers first: GPUI geometry aliases, direct GPUI
  focus/a11y re-exports, adapter-facing `GpuiOverlayState`, and GPUI-owned APIs such as
  `TextInputController`, `ScrollHandle`, and `focus_ring_shadow`.
* **Update**: Completed U1 of the extraction-prep plan by adding extraction-blocker inventory
  tests for `open-gpui-ui-components` public `*State`/`*Metrics` contracts and
  `open-gpui-ui-core` public focus/a11y/geometry blockers. The hard runtime/render leak guard still
  fails on new `Window`, `App`, `Context`, rendering, handle, and callback leaks; the new inventory
  pins known blockers for U2-U6 to shrink.
* **Verification**: `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `git diff --check`
  passed for U1.
* **Update**: Completed U2 neutral geometry. Added `UiPx`, `UiPoint`, `UiSize`, `UiRect`, and
  `UiEdges` to `open-gpui-ui-core`; migrated overlay placement inputs, safe bounds, offsets, and
  context-menu state point anchors to those neutral values; and kept GPUI conversion helpers inside
  the component adapter boundary.
* **Verification**: `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery` passed for U2.
* **Update**: Completed U3 neutral component metrics. UI-core `Size` helpers now return `UiPx`,
  all public component `*Metrics` use `UiPx` for layout scalars, gallery sizing samples consume
  `UiPx`, and component/gallery tests now assert neutral metric values directly.
* **Decision**: Kept `UiPx -> open_gpui` style conversion impls in `ui_core::geometry` for this
  adapter-first phase because GPUI `Styled` APIs accept `Into<Length>`/`Into<AbsoluteLength>` at
  render sites. This is a transitional convenience, not the final strict headless crate boundary;
  a later U4/U5 cleanup should move renderer conversion fully into adapter code when focus/a11y and
  overlay state are split.
* **Verification**: U3 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF warnings.
* **Update**: Completed U4 focus and accessibility facades. `open-gpui-ui-core` now owns neutral
  `Role`, `Toggled`, `Orientation`, `AccessibleAction`, and `FocusTargetId`; it no longer
  re-exports GPUI `FocusHandle`, `Focusable`, AccessKit roles, or AccessKit actions. Component and
  gallery render code now crosses into GPUI through `open_gpui_ui_components::a11y` adapter mapping
  functions and explicit `ui_role`, `ui_aria_toggled`, `ui_aria_orientation`, and
  `on_ui_a11y_action` methods.
* **Verification**: U4 currently passes `cargo check -p open-gpui-ui-components` and `cargo check
  -p open-gpui-ui-foundation-gallery`; full fmt/core/component/gallery nextest gate still needs to
  run before committing the slice.
* **Verification**: U4 final gate passed with `cargo fmt -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo check -p
  open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo
  nextest run -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF
  warnings.
* **Update**: Completed U5 neutral overlay state split. Added `OverlayResolvedState` to
  `open-gpui-ui-core`, migrated all overlay-owning component resolved states to expose it, kept
  `GpuiOverlayState` as GPUI adapter scheduling state, and derived deferred priority/snap margin at
  render sites instead of storing them in public `*State` contracts.
* **Review**: U5 read-only review subagent `u5_overlay_state_review` found no blocking issues. It
  confirmed the public-state overlay blocker allowlist is empty and render paths derive GPUI
  scheduling state locally. Its residual clone concern was resolved by making
  `GpuiOverlayState::from_resolved` borrow `OverlayResolvedState`.
* **Verification**: U5 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
* **Update**: Started U6 adapter-only API classification. `FocusRing` now uses neutral `UiPx` for
  public width metadata, while `focus_ring_shadow` remains the GPUI `BoxShadow` adapter helper.
  Added `open_gpui_ui_components::gpui_adapter` as the explicit grouping for concrete GPUI helper
  exports such as `TextInputController`, text-input initialization, `focus_ring_shadow`,
  `GpuiOverlayState`, and overlay scheduling helpers.
* **Verification**: U6 focused checks passed `cargo check -p open-gpui-ui-components`, `cargo test
  -p open-gpui-ui-components adapter_only_public_surfaces_match_allowlist -- --nocapture`, `cargo
  test -p open-gpui-ui-components gpui_adapter_exports_group_runtime_specific_surfaces --
  --nocapture`, and `cargo test -p open-gpui-ui-components
  focus_ring_preserves_token_intent_without_layout_shift -- --nocapture`.
* **Review**: U6 read-only review subagent `u6_adapter_classification_review` did not return
  findings before timeout and was interrupted. Local self-review found and fixed an accidental
  wildcard public re-export in the prelude adapter grouping, then added
  `public_reexports_stay_explicit_without_wildcards`.
* **Verification**: U6 final gate passed `cargo fmt -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo check -p
  open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, `cargo nextest run
  -p open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-foundation-gallery`, and `git
  diff --check` with only existing CRLF warnings.
* **Decision**: Updated ADR 0006 for the U7 extraction-prep checkpoint. Do not create
  `open-gpui-ui-headless` in this branch. Component resolved-state blockers are cleared, adapter
  APIs are classified, and the next extraction design should focus on pure behavior modules after
  deciding how to handle adaptive viewport `Pixels as Px` and `UiPx` GPUI style-conversion impls.
* **Review**: U7 read-only doc review subagent `u7_checkpoint_doc_review` did not return findings
  before timeout and was interrupted. Local self-review found no current-doc references that still
  describe geometry/focus/a11y/overlay split work as unfinished.
* **Verification**: U7 passed `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
* **Update**: Committed U7 as `5318178 docs(ui): update headless extraction checkpoint`.

## 2026-06-16
* **Decision**: Completed U8 of the UI shell, choice, and headless-readiness series by updating ADR
  0006. `open-gpui-ui-headless` remains deferred: the codebase now has real cross-family behavior
  reuse, but extraction still needs neutral geometry/focus/a11y facades, an overlay-state split,
  and a clear stance on GPUI-backed text editing.
* **Update**: Added the component contract guard
  `public_resolved_state_contracts_avoid_gpui_runtime_types`, which scans public resolved-state
  structs for GPUI runtime/rendering/callback leaks (`Window`, `App`, `Context`, `RenderOnce`,
  `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and `Rc<dyn` callback
  storage). Geometry aliases remain a documented extraction blocker, not a failing gate yet.
* **Update**: Updated `docs/ui/component-contract.md` and `docs/verification.md` with the
  headless-readiness checkpoint, the new public-state guard, and the next extraction-prep blockers.
* **Update**: Completed the main U7 Combobox/Command implementation for the UI shell, choice, and
  headless-readiness series. `ComboboxState` now models editable query text, grouped and
  standalone options, selected value/label metadata that survives filtering, active option
  metadata, filtered/total option counts, empty state, nested `ListboxState`, scroll viewport
  metadata, and non-modal dismissible popup policy.
* **Update**: Added `CommandState` with command groups/items, shortcut metadata, disabled state,
  selected/active command values, filtered/total counts, loading metadata, empty state, optional
  dialog wrapper metadata, inline non-modal overlay state, and modal dialog overlay state. Command
  selection payload coverage now verifies shortcuts are preserved through state-level activation.
* **Update**: Added Components gallery Combobox and Command samples plus tests for editable
  filtering, empty/disabled search states, command dialog presentation, loading/empty command
  states, shortcut metadata, explicit exports, and component contract signals.
* **Update**: Updated `docs/ui/component-contract.md` and `docs/verification.md` to move
  Combobox/Command from follow-up scope into the documented component contract and manual dogfood
  path.
* **Review**: U7 read-only review subagent `u7_review_fast` returned after the main commit and
  found one valid gallery coverage gap: the manual dogfood docs promised a combobox sample where
  query filtering hides the selected option while preserving selected metadata. The follow-up fix
  changed the framework combobox sample to select `solid` while querying `re` and added gallery
  assertions for `selected_value() == Some("solid")` plus hidden listbox selection. Its Escape
  concern matched an already-policy-gated render path using `escape_open_change`; a direct
  overlay-policy assertion was added for `EscapeKeyPolicy::Ignore`.
* **Verification**: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest
  run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed during U7.
* **Update**: Completed the main U6 Listbox/Select implementation for the UI shell, choice, and
  headless-readiness series. `ListboxState` now models grouped and standalone options,
  separators, disabled skip behavior, selected/active/tab-stop metadata, APG navigation,
  activation payloads, typeahead target metadata, metrics, color intents, and listbox roles.
* **Update**: Added `SelectState` as a trigger + non-modal dismissible overlay + nested
  `ListboxState` contract, including controlled/uncontrolled open mode, selected trigger label,
  outside-press policy, initial focus and focus restoration intents, scroll viewport metadata, and
  GPUI keyed runtime state for open/selected/active behavior.
* **Update**: Added Components gallery Listbox and Select samples plus tests for grouped choices,
  empty listbox, controlled-open long select, closed selected select, disabled select, choice
  roles, navigation/activation/typeahead, overlay policy, and scrollable popup metadata.
* **Update**: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and engineering
  memory to record the Listbox/Select boundary and manual dogfood checks.
* **Verification**: `cargo fmt --all`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed during U6.
* **Update**: Completed the implementation pass for U5 HoverCard in the UI shell, choice, and
  headless-readiness series. `HoverCardState` records content kind, size, disabled state,
  controlled/uncontrolled open ownership, default-open state, hover/focus/manual open intent,
  placement preference, open/close delay policy, outside-press policy, initial focus intent, focus
  restoration intent, token intents, metrics, and non-modal dismissible overlay state.
* **Update**: The concrete HoverCard adapter owns GPUI focus handles, hover timers, keyed runtime
  open state, deferred anchored rendering, Escape/outside event wiring, and trigger/content
  pointer-focus lifetime coordination. It intentionally does not reuse the descriptive Tooltip
  policy because HoverCard content is interactive and hit-testable.
* **Update**: Added Overlay gallery samples for default-open profile preview, focus-only preview,
  and manual controlled hover card behavior. Gallery/component tests now assert interactive overlay
  contracts, default delay, focus restore defaults, manual controlled state, pass-through versus
  consume outside policy, and explicit exports.
* **Review**: U5 review subagents did not return usable findings before timeout and were
  interrupted. The implemented shape follows the earlier HoverCard reference research; local
  self-review found and fixed a focus-lifetime issue so keyboard focus opening is persisted in
  keyed runtime instead of being only render-derived.
* **Verification**: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U5
  implementation pass.
* **Update**: Completed U4 of the UI shell, choice, and headless-readiness series by adding
  `AlertDialog` and `Sheet` to `open_gpui_ui_components`.
* **Update**: `AlertDialogState` now records required title/description text, cancel and primary
  action metadata, destructive intent, cancel-first focus metadata, outside-press consume policy,
  Escape policy, focus restoration, token intents, and modal layer state. The concrete adapter owns
  GPUI focus handles, callbacks, barrier rendering, and deferred layer wiring.
* **Update**: `SheetState` now records side, modal/non-modal mode, close affordance visibility,
  title/description metadata, Escape/outside policy, focus restoration, token intents, and edge
  placement metrics. Modal sheets default to dismiss-and-consume outside press; non-modal sheets
  default to dismiss-and-pass-through.
* **Update**: Added Overlay gallery samples for destructive/safe AlertDialog cases and left modal,
  right non-modal, and bottom sticky Sheet cases. Gallery tests now cover the new critical-action
  and edge-attached overlay contracts.
* **Fix**: Fixed the modal barrier color intent to use `ColorState::ModalOverlay` instead of
  generic overlay state so the default theme table covers Dialog, AlertDialog, and Sheet barriers.
* **Review**: U4 subagent review found three medium-severity contract issues: render paths were
  reporting uncontrolled overlays as controlled after runtime open changed, controlled gallery
  samples diverged from source metadata, and initial-focus resolution could target hidden or
  disabled affordances. The fixes split effective open from ownership mode, aligned controlled
  sample metadata, skipped unavailable focus targets, and deferred initial focus until the overlay
  layer is scheduled.
* **Verification**: `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery` passed during the U4 implementation pass. After review
  fixes, `cargo fmt --all`, the same two `cargo check` commands, and both focused `cargo nextest`
  runs passed again.
* **Update**: Added U3 Sidebar to `open_gpui_ui_components` with `SidebarState`,
  section/item descriptors, `SidebarSelection`, side/variant/collapse enums, metrics, colors, and a
  concrete GPUI adapter. The resolved state owns selection, focus, tab-stop, collapse, disabled,
  scrollability, and set-position metadata while the adapter owns focus handles, scroll viewport,
  rendering, and click/key dispatch.
* **Review**: Sidebar reference research confirmed the v1 scope should stay bounded: borrow
  `Icon`/`Offcanvas`/`None` collapse semantics and local roving-focus patterns, but avoid
  provider contexts, cookies, global shortcuts, mobile Sheet routing, nested submenu behavior, and
  route integration until shell requirements are clearer.
* **Update**: Added Components gallery Sidebar samples for expanded workspace navigation,
  icon-collapsed rail, and long scrollable reports navigation. Component and gallery tests now
  assert `Role::Navigation`, section roles, selected/focused/tab-stop state, disabled skip
  behavior, icon-collapse label preservation, offcanvas focus removal, and scrollable long-menu
  metadata.
* **Update**: Documented the Sidebar contract and manual dogfood checklist. Verified the U3 slice
  with `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Added U2 Toolbar to `open_gpui_ui_components` with `ToolbarState`,
  `ToolbarItemDescriptor`, `ToolbarItemState`, `ToolbarSelection`, and a concrete GPUI `Toolbar`
  adapter. The resolved state owns item kind, disabled/pressed state, tab-stop selection, metrics,
  colors, and roving-focus targets; the adapter owns focus handles, rendering, and action/toggle
  event dispatch.
* **Review**: Toolbar reference research confirmed the v1 scope should stay primitive and stable:
  reuse the local resolved-state and `roving_focus` patterns, model actions/toggles/separators, and
  defer workspace command registries, automatic overflow measurement, shortcut display, and app
  toolbar customization until shell requirements are clearer.
* **Update**: Added Components gallery Toolbar samples for horizontal editor commands and vertical
  inspector commands, plus gallery/component tests for exports, metadata, disabled/separator skip
  behavior, roving focus, toggle pressed metadata, and keyboard activation payloads.
* **Update**: Documented the Toolbar contract and manual dogfood checklist. Verified the U2 slice
  with `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest
  run -p open-gpui-ui-foundation-gallery`, and `git diff --check`.
* **Update**: Wrote the UI shell, choice, and headless-readiness series plan at
  `docs/plans/2026-06-16-002-feat-ui-shell-choice-headless-series-plan.md`. The sequence starts
  with a gallery conformance gate, then proceeds through Toolbar, Sidebar, AlertDialog/Sheet,
  HoverCard, Listbox/Select, Combobox/Command, and a headless-readiness checkpoint.
* **Update**: Added the Components gallery conformance gate surface for explicit crate/prelude
  exports, gallery metadata, ScrollArea redraw persistence, Splitter runtime constraints, Tabs
  overflow/roving focus, and explicit accessible labels. Tests now assert the stable gate metadata
  and isolate crate-root versus prelude exports so public-surface drift fails before manual dogfood.
* **Review**: U1 subagent review found the first prelude smoke could be satisfied by existing outer
  imports and that the plan wording overstated pointer/focus coverage. The test now resolves
  through `open_gpui_ui_components::prelude`, while the plan keeps pointer drag and focus-visible
  traversal in the manual dogfood path until state-level tests can prove them.
* **Fix**: Fixed `ScrollArea` samples appearing non-scrollable when the component value is rebuilt
  each render. The default `ScrollHandle` now lives in the adapter's keyed runtime instead of the
  `ScrollArea::new` builder value, while externally owned handles remain supported. Added a GPUI
  window regression test that scrolls a reconstructed `ScrollArea` and asserts child bounds move
  after redraw. Verified with `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-components`, and `cargo check -p open-gpui-ui-foundation-gallery`.
* **Fix**: Fixed vertical Splitter dragging for samples that start with a collapsed panel. The state
  layer now treats collapsed panels as restorable once a resize/runtime fraction reaches the
  restore threshold; below that threshold the collapsed fraction remains stable. This preserves the
  collapsed contract while making the gallery's vertical `details-split` sample draggable.
* **Update**: Wired the Splitter pointer-drag runtime. Handles start a GPUI drag, root
  `DragMoveEvent<SplitterDrag>` handlers measure movement against the full splitter bounds, and live
  fractions flow through `SplitterState::with_panel_fractions` plus `SplitterState::resized_by` so
  min/max constraints stay centralized. Drag payloads include the splitter group id to prevent
  cross-talk between multiple Splitters.
* **Update**: Updated the Splitter contract and manual verification docs: pointer dragging is now
  covered, while keyboard resizing, controlled resize callbacks, persisted layouts, RTL behavior,
  and nested splitter arbitration remain follow-up work. Verified with `cargo check -p
  open-gpui-ui-components` and `cargo nextest run -p open-gpui-ui-components`.
* **Update**: Added `Splitter` to `open_gpui_ui_components` as the second U11 layout primitive.
  The resolved state owns panel fraction normalization, min/max constraints, collapsed-panel
  metadata, handle adjacency, and delta clamping through `SplitterState::resized_by`; the GPUI
  adapter renders resolved panels and handle affordances without duplicating sizing rules.
* **Update**: Added Components gallery Splitter samples for horizontal and vertical layouts, updated
  the component contract and verification docs, and verified with focused component/gallery checks
  and nextest runs. Pointer dragging and keyboard resizing remain follow-up runtime work.
* **Update**: Added `ScrollArea` to `open_gpui_ui_components` as the first layout/shell-navigation
  component after the overlay checkpoint. The resolved state keeps viewport id, axis, reset
  policy/key, size, and scrollbar metrics renderer-neutral while the GPUI adapter owns
  `ScrollHandle`, overflow styling, and reset-on-key-change offset mutation.
* **Update**: Added Components gallery ScrollArea samples for vertical, horizontal, and two-axis
  overflow, documented the scroll viewport contract, and verified with focused component/gallery
  checks and nextest runs.
* **Update**: Finished the ADR 0006 stack-ordering follow-up by adding window-free overlay stack ordering
  primitives in `open_gpui_ui_core::overlay`: `resolve_outside_press` and `resolve_focus_restore`.
  Tests now cover topmost dismissible-layer outside-press handling and topmost present
  focus-restore resolution without a GPUI window. Full focus-trap/scope traversal remains deferred
  until nested overlay components need it.
* **Update**: Continued ADR 0006 follow-up by changing `ContextMenuState` to store
  renderer-neutral `OverlayPlacementInput` instead of resolved `GpuiOverlayPlacement`; GPUI
  placement is now resolved only inside the context-menu adapter/render boundary.
* **Update**: Started ADR 0006 follow-up by moving generic roving-focus helpers from `tabs.rs` into
  `open_gpui_ui_components::roving_focus`. `Tabs` keeps compatibility re-exports, and `Menu` plus
  `RadioGroup` now depend on the neutral behavior module instead of borrowing behavior from Tabs.
* **Decision**: Completed the U8 headless-readiness checkpoint and added ADR 0006. The decision is
  to keep `open-gpui-ui-headless` deferred for now: the overlay family proves reusable behavior
  contracts, but several state types still expose GPUI geometry or adapter placement state. Future
  extraction should first neutralize those leaks, move generic roving-focus helpers out of
  component-specific modules, and add window-free focus-scope / dismissible-layer tests.
* **Update**: Completed U5 of the overlay component series by adding the interactive non-modal
  `Popover` component and `PopoverState` contract to `open-gpui-ui-components`. The first slice
  models controlled versus uncontrolled open mode, default-open state, trigger expanded/selected
  intent, outside-press policy, placement metadata, initial focus intent, focus restoration intent,
  non-modal dismissible layer state, token intents, and metrics.
* **Update**: Added Overlay gallery Popover samples for default-open, controlled, consuming
  outside press, and disabled cases. The controlled sample is owned by gallery shell state and
  closes through the shared Escape handler.
* **Update**: Documented Popover in `docs/ui/component-contract.md` and `docs/verification.md`,
  with nested popovers, modal popover barriers, and a reusable focus-scope runtime explicitly
  deferred.
* **Update**: Verified U5 with `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Completed U4 of the overlay component series by adding the descriptive `Tooltip`
  component and `TooltipState` contract to `open-gpui-ui-components`. The first slice models text
  or simple element content, hover/focus/manual open intent, delay policy, placement metadata,
  disabled trigger behavior, tooltip layer state, token intents, and metrics while keeping the
  content non-interactive.
* **Update**: Added Overlay gallery Tooltip samples for hover/focus, focus-only, manual delayed,
  and disabled cases. The gallery wires hover state and focus handles at the adapter layer so
  keyboard traversal can reveal descriptive tooltip content without putting GPUI runtime types into
  `TooltipState`.
* **Update**: Documented Tooltip in `docs/ui/component-contract.md` and `docs/verification.md`,
  including the current `Role::Label` mapping until GPUI exposes a public tooltip role wrapper.
* **Update**: Verified U4 with `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Started U3 by adding `open_gpui_ui_components::overlay`, a narrow GPUI adapter helper
  layer that maps shared overlay policy into deferred priority, snap margin, GPUI anchor/offset,
  Escape open-change, and outside-press open-change decisions without introducing a global overlay
  runtime.
* **Update**: Verified U3 with `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Started U2 of the overlay component series by extending
  `open-gpui-ui-core::overlay` with renderer-neutral behavior contracts for layer kind, presence,
  outside-press policy, Escape policy, dismiss reason, focus restore, initial focus, layer-state
  resolution, Escape stack resolution, and anchor/placement input.
* **Update**: Updated the UI foundation gallery overlay page to show the shared behavior contract
  matrix for tooltip, popover, dialog, and menu policies, and documented the overlay resolved-state
  boundary in `docs/ui/component-contract.md`.
* **Update**: Verified U2 with `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery`.
* **Update**: Started U1 of the overlay component series. Added direct AccessKit repair tests for
  valid and invalid cross-node references in `crates/gpui/src/window/a11y.rs`, converted the
  `svg_renderer` font fixtures to runtime optional loading so the `open-gpui` lib test harness
  compiles without the missing bundled fonts in this checkout, and added a Foundation Gallery test
  that locks explicit accessible labels plus label-to-control association metadata.
* **Update**: Added `--page components` support to the UI foundation gallery binary so the prior
  Components-page `accesskit_consumer` crash path can be smoke-tested directly without manual tab
  navigation first.
* **Update**: Added focused verification guidance for GPUI accessibility repair changes to
  `docs/verification.md`, including the `open-gpui` a11y smoke test and the Components-page
  regression note for the `accesskit_consumer` crash.
* **Update**: Verified U1 with focused `open-gpui`/component/gallery checks, nextest runs, and a
  direct `cargo run -p open-gpui-ui-foundation-gallery -- --page components` smoke that stayed
  alive until the 30s timeout instead of reproducing the Components-page crash.
* **Update**: Wrote the next-series overlay component plan at
  `docs/plans/2026-06-16-001-feat-ui-overlay-component-series-plan.md`, scoped to the post-U6
  sequence: accessibility/gallery runtime gate, shared overlay behavior contracts, GPUI overlay
  adapter helpers, Tooltip, Popover, Dialog, Menu/ContextMenu, and a headless-readiness checkpoint.
* **Decision**: Start the next series with the AccessKit repair smoke instead of jumping directly
  to Tooltip/Popover. Overlay components will increase explicit accessibility relationships, so the
  crash barrier needs a repeatable verification gate first.
* **Decision**: Keep `open-gpui-ui-headless` deferred for now. The overlay series should produce
  renderer-neutral behavior contracts inside the current crates, then reassess extraction after
  multiple overlay components reuse them.
* **Update**: Completed U6 of the official component roadmap by adding `Badge` and `IconButton`
  to `open-gpui-ui-components` with resolved state, GPUI adapters, theme intents, exports, gallery
  samples, component tests, and foundation gallery metadata tests.
* **Update**: Hardened GPUI accessibility tree repair so invalid cross-node AccessKit references
  (`labelled_by`, `controls`, `active_descendant`, and related node-id properties) are stripped
  before the update reaches platform adapters. This addresses the Components-page crash reported
  with `accesskit_consumer` panicking while resolving a missing explicit label reference.
* **Update**: Added `Size::icon_size()` to `open-gpui-ui-core` and moved `IconButton` glyph sizing
  onto that shared foundation helper.
* **Update**: Verified U6 with `cargo fmt --all --check`, focused `cargo check` for `open-gpui`,
  `open-gpui-ui-core`, `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`, plus
  `cargo nextest run -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Verification Note**: The direct `open-gpui` a11y unit test command could not compile the
  package test harness because the local checkout is missing bundled test fonts under
  `assets/fonts/ibm-plex-sans` and `assets/fonts/lilex`; normal `cargo check -p open-gpui` passes.
* **Update**: Applied U5 follow-up cleanup after review: GPUI `div` now exposes `aria_required`
  and `aria_disabled`, RadioGroup uses those flags plus per-item disabled semantics, Tabs/Radio
  share stable-key selection and roving navigation helpers, and Toggle exports its own
  metrics/colors aliases while reusing Button visuals internally.
* **Update**: Accepted the U5 efficiency review findings by avoiding full `BTreeMap` focus-handle
  clones in RadioGroup render and skipping redundant active-state writes on repeated activation.
* **Update**: Verified the U5 cleanup with `cargo fmt --all`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Committed the main U5 slice as
  `5e562f3 feat(ui): add radio group and toggle components`.
* **Update**: Completed U5 of the official component roadmap by adding `RadioGroup` and `Toggle`
  to `open-gpui-ui-components` with pure resolved-state contracts, GPUI adapters, explicit
  exports, gallery dogfood, and targeted tests.
* **Decision**: `RadioGroup` reuses the U4 roving-focus helpers and maps items with
  `Role::RadioButton` plus `aria_selected` because the current GPUI AccessKit wrapper does not
  expose a separate checked property. `Toggle` remains button-like (`Role::Button` +
  `aria_toggled`) and does not reuse Checkbox tri-state semantics.
* **Update**: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and the Components
  gallery samples to cover RadioGroup required/disabled/roving state and Toggle pressed state.
* **Update**: Verified the U5 component and gallery surfaces with `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Completed U4 of the official component roadmap by adding `Tabs` to
  `open-gpui-ui-components` with a pure resolved-state contract, GPUI adapter, roving-focus
  helpers, gallery dogfood, and targeted tests.
* **Update**: Fixed the vertical Tabs dogfood so the left tab rail scrolls inside a constrained
  gallery card, matching the user-reported overflow issue.
* **Update**: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and the foundation
  gallery samples to cover Tabs roving-focus behavior, horizontal automatic activation, and
  vertical manual activation.
* **Update**: Verified the Tabs slice with `cargo fmt --all`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Committed the Tabs slice as `f0dbf96 feat(ui): add Tabs roving focus slice`.

## 2026-06-15
* **Update**: Completed U3 of the official component roadmap by adding `Checkbox` and `Label` to
  `open-gpui-ui-components` with resolved state, GPUI adapters, theme intents, tests, gallery
  samples, and updated verification guidance.
* **Update**: Updated `docs/ui/component-contract.md` so the resolved-state contract now includes
  Checkbox indeterminate state and Label association metadata.
* **Update**: Updated `docs/verification.md` so the Components manual dogfood now includes Checkbox
  and Label association checks in addition to Button, Switch, TextInput, and Field.
* **Update**: Implemented the real single-line editable `TextInputController` slice in
  `open-gpui-ui-components`, including GPUI `EntityInputHandler` / `ElementInputHandler`
  integration, UTF-16 selection and marked-range conversion, grapheme-aware deletion, clipboard
  actions, and gallery dogfood for the default components sample.
* **Subagent Finding**: Recorded editable TextInput controller research at
  `docs/knowledge/engineering/subagents/text-input-controller-research.md`: use GPUI's native
  input handler path for single-line editing and defer multiline/password/editor-grade behavior.
* **Update**: Updated `docs/ui/component-contract.md` so the contract now records that
  `TextInputController` owns the editable single-line path while `Field` stays composition-only,
  and multiline/password/undo/redo/completion remain out of scope.
* **Update**: Completed the runtime theme table slice from the official component roadmap. Added
  `ColorState`, `ThemeMode`, `ThemeColor`, and immutable `ThemeSnapshot` support, taught
  `ThemeResolver::resolve_with` to resolve `(TokenKey, ColorState)` before falling back to intent
  RGB, and exposed light/dark/high-contrast mode metadata in the foundation gallery.
* **Update**: Wrote the official UI component roadmap at
  `docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md`. The next-series order is runtime
  theme table, editable TextInput controller, Checkbox/Label, roving focus/Tabs,
  RadioGroup/Toggle, Badge/IconButton, shared overlay behavior, Tooltip/Popover, Dialog,
  Menu/ContextMenu, ScrollArea/Splitter, Toolbar/Sidebar, gallery conformance, and then headless
  extraction readiness review.
* **Decision**: Keep `open-gpui-ui-headless` deferred. The project should first prove repeated
  renderer-neutral contracts across Button, Switch, TextInput/Field, Checkbox/Radio, Tabs, and at
  least one overlay family; `gpui-component`, `fret-ui-kit`, and `fret-ui-shadcn` remain references
  rather than runtime dependencies.
* **Subagent Finding**: Recorded roadmap reference research at
  `docs/knowledge/engineering/subagents/ui-component-roadmap-reference-research.md`: use
  `gpui-component` for GPUI-native implementation patterns, `fret-ui-kit` for policy-layer
  references, and avoid wholesale copying from either repository.
* **Update**: Added the shared `FocusRing` primitive to `open-gpui-ui-components` and migrated
  Button, Switch, TextInput, and the focus/a11y gallery demo from border-width focus styling to a
  box-shadow focus-visible adapter that does not change layout.
* **Update**: Added `ThemeResolver` to `open-gpui-ui-components` and migrated Button, Switch,
  TextInput, and Field render paths to resolve `ColorIntent` values centrally while keeping token
  intent visible in component state.
* **Update**: Implemented the TextInput/Field component slice from ADR 0005: added
  `TextInputState`, `FieldState`, `FieldMessage`, component exports, gallery samples, tests, and
  `docs/ui/component-contract.md`; focused component and gallery checks pass.
* **Update**: Recorded text input research showing that full editable text input must use GPUI's
  `EntityInputHandler` / `ElementInputHandler` path, so this slice intentionally keeps platform
  text editing as a follow-up rather than faking input with key events.
* **Update**: Drafted ADR 0005 for the official component architecture. It records the adapter-first, headless-ready direction, the GPUI adapter boundary, the future `open-gpui-ui-headless` extraction trigger, and the next follow-up work on `TextInput` / `Field`, theme resolution, and focus rings.
* **Update**: Completed the first `open-gpui-ui-components` slice: the workspace now has Button and Switch components backed by `ui_core` sizing, tokens, role/toggled state, and a Components gallery page; `cargo check` and `cargo nextest` pass for `open-gpui-ui-core`, `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`.
* **Update**: Committed the first UI foundation slice as `f626464 feat(ui): add UI foundation core gallery`, then created the next plan at `docs/plans/2026-06-15-002-feat-ui-components-first-slice-plan.md` for `open-gpui-ui-components` with Button, Switch, gallery dogfood, and verification.
* **Update**: Completed U4 of the UI foundation gallery plan: `docs/verification.md` now documents focused `open-gpui-ui-core` / gallery checks and the manual `cargo run -p open-gpui-ui-foundation-gallery` dogfood path; package checks and nextest runs pass.
* **Update**: Completed U3 of the UI foundation gallery plan: focus/a11y and overlay now have interactive demos backed by `open-gpui-ui-core` focus/a11y/overlay vocabulary, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passes 10/10 tests.
* **Update**: Completed U2 of the UI foundation gallery plan: tokens, sizing/density, and adaptive pages now render real `open-gpui-ui-core` data models, the shell has a compact/desktop switch, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passes 8/8 tests.
* **Update**: Completed U1 of the UI foundation gallery plan by adding `examples/ui-foundation-gallery` as a workspace package with a small library, thin binary, pure foundation dependency surface, empty shell, section registry, and passing `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Wrote the first follow-up plan at `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md` and locked the first consumer choice to a dedicated pure-foundation gallery example.
* **Update**: Recorded the reference repository set for the Open GPUI UI strategy: `fret`, `fret-ui-kit`, `fret-ui-shadcn`, `gpui-component`, plus broader open source comparators such as Flutter, Jetpack Compose, Radix UI, React Aria, React Spectrum, shadcn/ui, and Apple HIG / SwiftUI.
* **Update**: Implemented the first Open GPUI UI foundation slice on `feat/open-gpui-ui-core` with the new `open-gpui-ui-core` crate, sizing/adaptive/token/overlay helpers, a11y/focus re-exports, and passing `cargo nextest run -p open-gpui-ui-core`.
* **Update**: Updated ADR 0004 to prioritize a11y, focus, overlay, tokens, sizing, density, and adaptive layout before broad component rollout; added decision and session memory for the UI foundation-first strategy.
* **Initialization**: Created engineering wiki memory bundle.
