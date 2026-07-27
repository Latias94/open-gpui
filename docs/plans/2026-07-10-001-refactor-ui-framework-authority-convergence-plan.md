---
title: "Open GPUI UI Framework Authority Convergence - Plan"
date: 2026-07-10
type: refactor
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
deepened: 2026-07-27
---

# Open GPUI UI Framework Authority Convergence - Plan

## Goal Capsule

Open GPUI should leave this work as a credible general-purpose desktop UI framework foundation, not only a large component catalog. The framework must have one authoritative path for form lifecycle, focus and overlay arbitration, accessibility semantics and announcements, activation, theme resolution, collection typeahead, component conformance, post-layout subtree geometry, layout-preserving subtree presentation, portal anchors, nested reveal, rounded subtree clipping, Dock presentation and application ownership, lossless platform-event delivery, live platform-window mutation, native multi-viewport drag transport, and DockSurface window-session teardown.

The refactor is intentionally breaking. New authorities replace old forwarding layers, duplicated metadata, source-string scanners, and public callbacks in the same implementation unit. No compatibility aliases or parallel runtimes remain after a migration unit lands.

Success is observable when:

- asynchronous form validation cannot publish stale results and `FormStatus::Validating` is observable from store through UI and DevTools;
- nested overlays in one window share deterministic dismiss, modal, focus-loop, and focus-restore behavior while different windows remain isolated;
- representative components are verified against the final AccessKit tree and AccessKit actions, not only hand-authored claims;
- pointer, keyboard, accessibility, and programmatic activation enter one semantic callback path with role-specific key policy and exactly-once behavior;
- the replacement theme v1 owns stable design scales and resolves app, window, and subtree context without app-global selection leakage;
- collection typeahead has one deterministic, fake-clock-testable session implementation;
- federated typed authorities project their own facts and are cross-checked structurally across component metadata, gallery, DevTools, docs, public surfaces, and executable scenarios;
- one finite, invertible, axis-aligned subtree transform composes paint, clipping, hit testing, local coordinates, accessibility bounds, deferred work, and cache replay without changing layout;
- `Visible`, `Inert`, and `Hidden` have one layout-preserving subtree authority, with exact and testable paint, input, focus, and accessibility participation;
- declarative live regions and transient announcements enter the same committed accessibility tree without stealing focus or retaining message text in diagnostics;
- portal followers consume typed, window-owned committed anchor geometry, and focus, AccessKit, and application reveal requests share one nested bring-into-view authority;
- rectangular and rounded-rect subtree clips constrain paint and hit testing through one checked stack without changing layout;
- Dock hosts resolve one complete visual style at render time, and application code observes one stable surface revision/event stream plus typed panel activation completion;
- platform windows expose capability-specific mutation requests whose queued dispatch and observed native facts remain distinct, allowing Dock viewports to move, resize, change state, and apply supported flags without fabricated success;
- a platform callback that arrives while `App` is already mutably borrowed is queued and replayed under an explicit event-ordering contract rather than logged and dropped, and a frame is never acknowledged as painted until it is accepted for drawing or re-invalidated;
- a detached Dock viewport may appear without stealing focus yet remain normally activatable later, carries an explicit native owner/transient relationship where supported, and belongs to one generation-bound DockSurface window session;
- native pointer capture can route a Dock drag across real windows without the target HWND receiving raw mouse messages, a provisional viewport is visibly presented before release, and release or cancellation settles exactly one graph transition;
- closing a DockSurface anchor cancels active and provisional work and closes only that surface's owned viewports, while another surface in the same application remains intact;
- the existing GPUI substrate, table engine, virtualizer, motion engine, text editing, choice models, and `FormStore` architecture remain deep modules rather than being rewritten for symmetry.

## Product Contract

### Summary

The current codebase is a strong 0.2 desktop UI foundation with substantial behavior and test coverage. The baseline audit passed 861 tests with one skipped test across `open-gpui-ui-core`, `open-gpui-ui-components`, `open-gpui-form`, `open-gpui-motion`, and the foundation gallery. The remaining gaps are not primarily missing widgets. They are split ownership and unverified lifecycle behavior at framework boundaries.

This plan converges those boundaries in dependency order. It adopts the window ownership and focus-scope lessons visible in `repo-ref/gpui-component` and `repo-ref/fret`, while retaining Open GPUI's existing crate direction and GPUI-native component model.

### Problem Frame

The framework currently has several models that look complete in isolation but are not production authorities:

- `ui_core::overlay` can resolve topmost Escape, outside press, and focus restore, but production overlays do not share a per-window stack that consumes those resolvers.
- Modal surfaces use tab grouping but do not provide a nested focus trap or reliable target/fallback resolution.
- Accessibility component state, renderer attributes, static evidence, DevTools output, and the final AccessKit tree can disagree because the final tree is not test-observable on `TestPlatform`.
- `FormStatus::Validating` is effectively unreachable, field projection drops validation activity, and a completion for an old field value can overwrite newer state.
- Theme selection is app-global, revisions are caller-controlled rather than effective-content authority, and the schema only models a small color vocabulary.
- The public SVG-only transformation and renderer matrices cannot transform an arbitrary interactive subtree: visual output, hitboxes, event-local coordinates, clips, accessibility bounds, deferred draws, and cached frame journals have no shared geometry authority.
- `Visibility::Hidden` is applied as a late `div` paint decision while input/focus registration and `Element::a11y_hidden` use separate paths, so a layout-preserving hidden or inert subtree cannot make one coherent cross-channel guarantee.
- Public callbacks expose `ClickEvent` even where consumers only need semantic activation or value change.
- Tree and VirtualizedList duplicate typeahead buffer and timeout behavior.
- Component contract rows, API inventory, public owner tables, gallery catalog, accessibility evidence, and source parsers repeat product facts that can drift while all string checks remain green.
- GPUI platform callbacks all attempt to reenter `App` through the same fallible mutable borrow and currently log and discard the callback on failure. A nested native message can therefore lose frame, placement, activation, mutation-observation, input, or close facts, and the shared log target does not identify which callback was dropped.
- Dock tear-off opens a native window from the source input transaction, and at least one failure path closes that window while still holding the viewport runtime's `RefMut`; synchronous close observers can then reborrow the same runtime.
- Win32 pointer capture keeps move and button-up delivery on the source HWND, while Dock's foreign-hover layer assumes the target window receives raw movement. Existing visual tests inject target events directly and therefore bypass the production transport failure.
- A tear-off viewport is created only after `MouseUp`, so no detached HWND exists to display while the pointer is still down. The source-window drag view is clipped at its own native boundary.
- The creation-time `focus` option is encoded by the Windows backend as permanent `WS_EX_NOACTIVATE`, conflating initial appearance policy with later activation capability.
- `DockSurface` tracks committed Dock facts but not an anchor/window-session lifecycle. Detached viewports are independent top-level windows, and closing the primary can leave them and the process alive.

The failure mode is authority drift: each layer is locally plausible, but no single module owns the end-to-end invariant.

### Requirements

R1. Preserve the dependency direction `open-gpui -> ui_core -> ui_components -> applications/examples`. `ui_core` remains renderer-neutral and cannot depend on GPUI lifecycle types.

R2. Preserve existing deep modules unless a failing characterization proves an ownership defect: GPUI element/entity/context primitives, table engine, virtualizer, motion, text editing, choice state, `FormStore`, theme registry/snapshot, and command/action presentation.

R3. Derive effective form status from validation activity and submission phase. Value changes, reset, and newer generations must invalidate older validation tickets. Stale completion must be a typed no-op result.

R4. Provide a test-only/diagnostic path from `TestAppContext` to activate accessibility, inspect final AccessKit tree updates, and dispatch AccessKit actions against actual nodes.

R5. Provide nested focus scopes with stable target identity, initial-focus policies, forward and reverse loops, stale-target filtering, and deterministic restoration.

R6. Make one per-window overlay runtime the sole authority for registration order, parentage, topmost event arbitration, controlled close requests, closing presence, modal underlay blocking, focus claims, and restoration.

R7. Derive one accessibility semantic projection from each component's existing resolved state. GPUI accessibility output and redacted DevTools inspection must consume that projection; no independently stored descriptor or static evidence may become a second semantic tree.

R8. Replace public physical-click callbacks on official semantic controls with semantic activation or value-change callbacks. Activation source and domain payload must be typed; raw pointer detail remains available only through an explicitly named escape hatch where a real consumer needs it.

R9. The replacement theme v1 must retain a complete color scale. Typography, spacing, radius, elevation, density, and motion-policy are candidate public scales; each token enters schema/snapshot public contract only when at least two distinct production component recipes consume it. Multiple call sites in one component, tests, Gallery, and documentation do not count as independent consumers. Unproven categories remain local and are recorded as intentionally deferred rather than padded to satisfy the plan.

R10. Resolve theme context with precedence `subtree override > window selection/override > app selection > built-in fallback`. Serialized `revision` is source metadata only. Runtime-owned effective revisions are unforgeable and monotonic for effective content or authority-selection changes; metadata-only reloads and exact no-ops do not bump. Invalid loads must be atomic.

R11. Deferred overlays opened from a themed subtree must retain the effective opening theme, including density and motion policy.

R12. Extract one private collection typeahead session used by at least Tree and VirtualizedList, with an injected clock and stable-key behavior. Search inputs such as Combobox and Command must remain separate.

R13. Establish federated typed authorities with narrow ownership: component contract rows own product metadata, Gallery owns selectors/probes, native tests own executable scenario IDs, and xtask cross-checks them structurally. Delete source parsing and parallel hand-authored facts without recreating a central registry.

R14. Preserve Table's engine, `core -> filtered -> grouped -> sorted -> expanded -> paginated -> final` row-model order, stable logical-identity invariant, and independent Virtualizer ownership. U5 may replace `TableRowId`-only identity-sensitive APIs with typed source/group identities and correct column-order ownership, but no engine rewrite or 2D virtualization project is part of this plan.

R15. Breaking migrations must update official components, gallery, DevTools, docs, examples, contracts, and tests in the same unit. Old aliases, forwarding facades, and stale evidence are deleted immediately.

R16. Duplicate Table source rows must never alias focus, edits, callbacks, render/semantic nodes, or virtualizer measurements. Occurrence identities are valid only within their resolved source snapshot; callers that retain identity across source reorder must provide explicit instance IDs. A partial column order reorders columns without hiding otherwise visible columns.

R17. Provide one public, layout-neutral interactive subtree transform in `open-gpui` for finite positive normal axis-aligned `scale(x, y)`, finite translation, and an explicit origin. Scale, inverse, and nested composition must remain representable under a checked numeric contract. The transform must compose through every observable visual, input, accessibility, deferred, portal-anchor, cache, motion, and diagnostic geometry path; a numeric failure suppresses the whole subtree before channel registration rather than using identity, clamping, or a partial projection. Rotation, skew, perspective, and 3D are not part of this contract.

R18. Provide one layout-preserving subtree presentation authority with exact `Visible`, `Inert`, and `Hidden` semantics. `Hidden` preserves layout but removes paint, input, focus, IME, and accessibility participation; `Inert` preserves layout and paint but removes input, focus, IME, and accessibility participation. Ancestor suppression is authoritative, and independent paint/input/focus/accessibility hiding flags cannot remain as competing subtree authorities.

R19. Provide committed live-region semantics and window-scoped transient announcements through the final AccessKit tree. Politeness, atomicity, busy state, stable declarative identity, repeated same-text announcements, activation generations, queue ordering, window isolation, focus independence, and privacy must be explicit; no component timer or direct platform speech API may become a second authority.

R20. Provide a typed, window-owned portal-anchor handle that binds one live target per frame and publishes current-frame or committed `ElementGeometry`, frame generation, presentation membership, and effective clip bounds. Wrong-window use is rejected, absent, hidden, unmounted, or failed targets become explicitly unlinked, inert targets remain linked with their presentation fact, and stale last-known geometry is never silently authoritative.

R21. Provide one window-owned bring-into-view authority for application requests, winning focus claims, and AccessKit `ScrollIntoView`. It must traverse committed nested scroll ancestry from inner to outer, preserve explicit physical horizontal/vertical alignment, use transform-aware local deltas, arbitrate requests whose scroll chains overlap, cancel stale generations and user-overridden motion, and support virtualized targets through a separate materialize-then-reveal protocol. Logical block/inline alignment waits for the locale and direction design epic.

R22. Provide one checked layout-neutral subtree clip authority for rectangles and rounded rectangles. Nested clips constrain paint and initial hit testing through the same exact stack, compose with U12 transforms and U13 presentation, inherit through deferred/cache replay, reset only at named portal boundaries, and fail closed when numeric or backend representation is unsupported. Arbitrary paths are not part of this contract.

R23. Provide one immutable `DockVisualStyle` authority for root surfaces, tabs, splitters, floating containers, drag visuals, drop guides, previews, focus, and rejection states. A named, immutable per-surface or explicit low-level-host render resolver may adapt the current window or subtree theme without making `gpui_docking` depend on UI Components. Drag-session visual snapshots remain separate from `DockDragPayload` identity, `DockDropGuideStyle` is replaced by structurally named metrics, and hard-coded production palettes or independently colored Dock paths cannot remain.

R24. Make `DockSurface` the application-level owner of committed Dock changes. It exposes a monotonic revision and typed layout/viewport change events whose coalescing is defined by an explicit logical transaction identity, stable-item `activate_panel` intent routed through one generation-bound activation host per space with typed focus completion, and explicit snapshot export for caller-owned debounce and storage. It does not perform file I/O, merge independent commands merely because they share an App turn, or treat selection, focus request, mutation dispatch, or dropped observation subscriptions as committed facts.

R25. Provide capability-specific GPUI platform-window mutation for position, size, state, and dynamic flags. Dispatch reports queued, unchanged, unsupported, rejected, or closed without claiming native commitment; a generation-bound observation ticket separately settles as exact, adjusted, superseded, rejected, unsupported, or window closed. Position, size, state, and restore bounds form one explicit placement conflict domain, while independent flags may partially dispatch. Lifetime activation acceptance and click-focus remain independent fields inside one coherent activation-policy domain: they share one generation and terminal observation but are never aliases and cannot partially commit within that domain. All public platform-fact getters read the same committed observation authority. Dock consumes that contract and removes capability claims, direct-backend reads, or sync records that fabricate live support.

R26. Deliver every asynchronous native platform-window callback through one typed, generation-bound event authority that remains usable while `App` is mutably borrowed. Every event receives an application-wide ingress sequence before direct delivery is considered; envelopes also carry the full generation-bearing `WindowId`, event domain, and only the domain-specific pointer, mutation, Dock session, or drag generation they require. A callback-registration epoch is added only if one `WindowId` can demonstrably replace callbacks while remaining live. A callback may drain inline only when no older queued event, active drain, or unresolved barrier can be bypassed. Frame, coherent placement, hover, and continuous pointer movement may coalesce only within the same window/domain/domain generation; queue-eligible button edges, text input, pointer cancellation, close lifecycle, and mutation terminal observations are ordered and non-droppable. Pointer cancellation and close are ordering barriers, stale events cannot target a replacement window, and a deferred frame is re-requested rather than acknowledged as painted. Synchronous native queries never wait on this queue: hit testing reads committed immutable facts, and close permission conservatively prevents immediate native destruction while queueing close intent. A hybrid input callback whose native default disposition depends on the current `DispatchEventResult` is not eligible for a fixed committed fallback or delayed replay. Framework-owned platform commands that can synchronously pump such input execute from one closed typed FIFO only after the outer `AppRefMut` is released and all older native-event barriers are settled, so the callback reads the real handler result with `AppCell` idle; a busy entrance is an invariant violation with callback-specific diagnostics. Any other return-valued callback must define and prove an equally explicit equivalent result contract before it migrates.

R27. Separate creation-time appearance from lifetime activation and native ownership. A window may be shown without initial activation while remaining eligible for later click, programmatic activation, and focus. Permanent non-activation is an explicit capability, not a side effect of `focus_on_appearing = false`. A typed owner/transient relationship is projected to each capable backend without creating a child window or replacing explicit application teardown; unsupported native relationships remain observable.

R28. Give each facade-managed `DockSurface` one explicit, generation-bound window session with a unique anchor, owned committed and provisional viewports, and `Vacant/Closed -> Opening -> Active -> ShuttingDown -> Closed` lifecycle. Only an active anchor accepts viewport work. Opening failure, synchronous close during creation, duplicate open, application shutdown, and stale generation callbacks settle explicitly. Closing the current anchor freezes new work, cancels active drag and pending/provisional viewport work, then force-closes a snapshot of only that surface's owned windows after all runtime borrows are released. `ShuttingDown` reaches `Closed` only after the current-generation runtime registry is empty and every snapshotted native window has produced a terminal close observation or is confirmed absent; reopen is rejected before that point. Shutdown is idempotent, bypasses per-viewport `Prevent` and `MergeBack` policies, never unconditionally quits the application, and cannot be triggered by stale anchor generations or implicit low-level hosts.

R29. Route native cross-window Dock drag from the capture-owning source through one generation-bound active-drag authority using current screen coordinates, an ordered underlay stack of full `WindowId` values, Dock surface/session and host-scene generations, scale factor, and committed host-scene facts. Underlay resolution follows current native z-order and committed application-window geometry while explicitly skipping the provisional/session-suppressed window that may cover the pointer. The route accepts only current hosts from the same surface, excludes provisional and session-suppressed/non-input windows, revalidates at release, and delivers preview, release, and cancellation without depending on raw mouse delivery to the target HWND or transferring native capture. A host from another Dock surface is a typed forbidden target with rejected preview; release over it cancels and restores the source rather than falling through to desktop promotion.

R30. Present a single provisional tear-off viewport before release once a drag crosses the live-undock threshold. It owns a generation-bound presentation lease while the committed `DockGraph` remains unchanged, follows the pointer, and has a visible non-empty first presentation. The native window is created with the ordinary detached viewport's final lifetime activation/input capability but no initial activation; the Dock session suppresses input, focus, activation, routing eligibility, and accessibility ownership until promotion. While that suppression is active, the source retains one stable tab semantic/focus proxy rather than a duplicate panel subtree. Every fallible native effect and promotion preflight completes while the provisional remains suppressed; one final App transaction atomically commits graph ownership, viewport registration, presentation lease, semantic/focus ownership, and gate removal. Input or accessibility work arriving during promotion is bound to the promotion generation and drains only after commit or retires after rollback. Desktop release promotes that same window without requiring a native capability transition; release to another valid host transfers and commits there before activating the target host; return or cancellation restores source presentation and focus. Source deactivation or shutdown never steals focus back. Rejection, Escape, capture loss, source/anchor close, stale generation, creation/presentation failure, or commit failure restores or cancels without graph mutation, duplicate semantic ownership, or leaked windows.

### Acceptance Examples

1. A user opens a Popover, opens a Menu from it, then opens a modal Dialog. Escape is offered only to the Dialog. If its policy ignores Escape, lower layers do not close. Closing the Dialog restores focus inside the Menu; closing the Menu restores its trigger; closing the Popover restores the original application target.
2. A controlled Dialog requests close, but its owner keeps `open = true`. It remains registered, modal, and focused until the controlled value actually closes. A close callback that opens another overlay cannot be followed by an old focus restoration that steals focus.
3. Two windows use different themes. A compact high-contrast subtree in one window opens a deferred Select menu, and the menu retains that subtree's tokens without affecting its sibling or the other window.
4. A field starts asynchronous validation, changes value, and starts another validation. Completion of the first ticket is reported stale and cannot change errors, activity, status, submission eligibility, UI busy state, or DevTools state.
5. Button activation by pointer, Enter, Space, and AccessKit Click reaches the same semantic callback exactly once. Link activates on Enter but not Space. A disabled control advertises no activation and ignores every entry path.
6. A rendered Checkbox changes from unchecked to checked. The same stable AccessKit node is observed on the next frame, with its state updated. DevTools reports the same semantic facts and does not reconstruct them from an evidence string.
7. Tree and VirtualizedList accumulate typeahead within the configured interval, reset after fake-clock advancement, skip structural or disabled targets, preserve stable-key identity across reorder, and never share a buffer across instances or windows.
8. Removing a component's executable scenario binding causes the conformance gate to fail with its component/scenario ID and owner path. Editing a comment or brace count cannot make the gate pass.
9. Two Table rows share one business ID and provide explicit instance IDs. After filtering, sorting, pinning, virtual recycle, and return, focus, edit, keyboard activation, and AccessKit Click still target the chosen instance and reuse its logical node identity. A partial column order keeps every other visible column available and reorderable.
10. A scaled and translated subtree contains text, a clipped scroll region, an editable control, and a button. Every primitive paints at the transformed position, pointer and captured-drag dispatch resolve the intended local coordinates, the IME and inspector bounds match the display, and AccessKit Click activates the same stable node. Its measured size and sibling layout do not change.
11. Three otherwise identical subtrees are `Visible`, `Inert`, and `Hidden`. All three reserve the same layout space; the inert subtree remains painted but cannot receive hover, scrolling, focus, IME, pointer capture, tooltip, or AccessKit actions; the hidden subtree additionally paints nothing. Dynamic transitions remove stale focus, capture, and accessibility membership without affecting sibling layout.
12. A busy status region updates twice without moving focus. A repeated polite announcement with identical text still produces a new committed semantic generation, while an announcement requested before accessibility activation or after window close is dropped and never replayed.
13. A transformed and scrolled trigger binds a portal anchor used by a Popover. The follower tracks the current committed displayed geometry, becomes unlinked when the trigger is hidden or unmounted, and never reuses geometry from a failed or foreign-window frame.
14. A winning focus claim and an AccessKit `ScrollIntoView` action reveal the same target through nested horizontal and vertical scrollports. The request applies inner-to-outer, respects physical `Nearest` alignment and reduced motion, and a newer request sharing any scroll ancestor or direct user scroll cancels stale work without moving an unrelated axis.
15. A rounded clipped subtree contains overlapping text, images, and interactive children under non-uniform scale. Pixels and pointer hits outside a rounded corner are excluded, nested rounded and rectangular clips remain exact, captured input follows capture policy after acquisition, and layout plus conservative accessibility bounds remain stable.
16. Two Dock hosts render under different theme scopes. Each host, its floating panels, and its drop affordances use the local style; a drag preview retains its opening style while the target guide follows the target host, and changing one scope does not mutate the other host or Dock layout.
17. An application activates a panel by stable item ID in a detached viewport. Selection and window activation may be requested first, but one current host-registration generation owns activation and completion reports success only after the exact panel descendant commits focus. One explicit surface transaction coalesces its committed categories into one revision/event, while a second command in the same App turn receives a distinct transaction and revision. The application exports and debounces its own snapshot.
18. A detached Dock viewport is dragged on a backend that supports live placement but not topmost. Its placement request is queued, direct getters still expose the prior committed facts, and an adjusted native callback settles the observation ticket and updates route/snapshot geometry. Topmost reports unsupported without blocking the independent placement dispatch, and the runtime never claims the window moved from dispatch alone.
19. Win32 delivers a frame, placement observation, pointer release, and close callback while another GPUI window update owns the mutable `App`, followed by a synchronous close-permission query. All asynchronous events retain application ingress order, coalescible facts converge only within their domain generation, ordered terminal events arrive exactly once behind their barriers, and the query returns from its immutable snapshot without reentry. After the outer App borrow is released, two queued pump-sensitive platform commands synchronously trigger hybrid inputs whose handlers respectively consume and propagate native default behavior; both return their real immediate handler result, the native-input busy counter remains zero, and nested command enqueue preserves FIFO without recursion. Stale `WindowId`/domain generations do not touch replacement state, and the queued frame is eventually presented without a `RefCell already borrowed` event loss.
20. A detached viewport is shown without taking initial focus. It still activates by click and `activate_panel`, reports its native owner/transient relationship when supported, and never acquires permanent no-activate styling merely because initial focus was disabled.
21. Two independent Dock surfaces each open an anchor and detached viewports. The first admits no viewport until synchronous `App::open_window` returns a committed full `WindowId` and its `Opening` token validates into `Active`. Closing that anchor during an active drag enters shutdown once, cancels its provisional viewport and pending effects, force-closes only its runtime-registered windows despite `Prevent` or `MergeBack`, and leaves the second surface usable. It remains `ShuttingDown` while any old-generation HWND is registered or lacks terminal close observation; reopening is rejected until `Closed`, after which a new generation admits no stale create/close callback.
22. With Win32 capture held by the source HWND, the pointer crosses a second viewport that receives no raw mouse move. The source-owned route resolves an ordered underlay stack through any provisional HWND, converts the current screen point into the target's current coordinate space, publishes the target preview before release, revalidates the same surface and host generation on button-up, and commits the drop exactly once. A host from another Dock surface shows a rejected preview and cancels on release rather than becoming a desktop tear-off. Capture loss, Escape, source close, or anchor shutdown clears every preview and pending task without using the last valid target. Closing only the current target clears that target's preview and pending work while retaining the source-owned drag; the next movement or release resolves again from the current screen point.
23. A user drags a tab onto the desktop. Before button-up, one runtime-registered provisional viewport is visible with a non-empty presented frame while its final native lifetime profile is session-suppressed, one source semantic/focus proxy remains authoritative, and the durable graph/revision remain unchanged. Repeated movement repositions rather than recreates it; entering a host hides the provisional presentation without destroying its reusable HWND and shows only that host's guide. Desktop release completes fallible promotion work while suppressed, then one App transaction commits graph/registry/lease ownership and removes the gate before activating the new viewport and restoring the previously focused live panel descendant. Release over another valid host transfers and commits there before its activation host receives focus; return to the source host or cancellation restores source presentation and focus before destroying the provisional. Recoverable creation, renderer, surface, presentation, optional native-effect, or commit failure restores only while the source generation remains live. Source deactivation or shutdown cancels without stealing focus or attempting restoration into a dead source.

### Scope Boundaries

In scope:

- form lifecycle correctness and projections;
- GPUI accessibility test support;
- focus scopes and the window overlay runtime;
- accessibility migration for every official component that emits semantics, with deep final-tree/action parity for representative families;
- semantic activation breaking APIs;
- replacement theme v1, effective revisions, window/subtree resolution, and deferred inheritance;
- shared collection typeahead;
- typed conformance projections and deletion of duplicate/source-string authorities;
- a finite, positive, invertible axis-aligned transform for arbitrary interactive subtrees, including renderer, input, accessibility, cache, deferred, motion, and Gallery integration;
- layout-preserving visible/inert/hidden subtree semantics across paint, input, focus, IME, and accessibility;
- committed declarative live regions and transient window announcements through final AccessKit updates;
- typed committed portal anchors and one nested bring-into-view authority;
- layout-neutral rectangular and rounded-rect subtree clipping across paint, hit testing, transform, deferred/cache, accessibility, and native renderers;
- a complete render-time Dock visual style, one application-level DockSurface revision/event owner, and stable-item activation completion;
- capability-specific live platform-window position, size, state, and flag mutation consumed by multi-viewport Docking;
- lossless, typed native callback delivery and two-phase platform side effects under App and Dock runtime reentrancy;
- separate window appearance, lifetime activation, and native owner/transient contracts;
- explicit DockSurface anchor/window-session lifecycle with surface-scoped forced teardown;
- source-capture-owned cross-HWND Dock drag routing and release-time target revalidation;
- a pre-release-visible provisional Dock viewport with presentation lease, promotion, rollback, and cancellation semantics;
- owning-platform real-window integration and subprocess evidence for the above behavior;
- common public-surface cleanup, gallery, DevTools, ADRs, migration notes, and verification.

Explicitly out of scope:

- a new standalone headless component crate or a second UI runtime;
- copying the all-purpose `Root` implementation from `gpui-component` or Fret's runtime architecture;
- rewriting GPUI `Element`, `Entity`, `Context`, `Window::use_keyed_state`, the layout engine, or the render pipeline from scratch;
- rewriting Table, Virtualizer, Motion, Choice, text editing, or `FormStore` from scratch;
- moving command execution into the UI activation module or deleting `ActionDescriptor`/`ResolvedActionState` without new evidence;
- replacing the neutral accessibility vocabulary merely to mirror AccessKit types;
- tokenizing every pixel literal; structural component metrics remain local;
- cross-window overlays, general portal routing across native windows, or a generalized dependency-injection container; the scoped Dock active-drag transport in R29 is the sole cross-window routing exception;
- rotation, skew, perspective, 3D transforms, negative or zero scale, and a general affine-transform public API;
- arbitrary path subtree clipping, true group opacity/compositing, and a multi-pointer gesture arena; these remain research-only until their missing renderer or pointer substrate is designed;
- direct ports of ImGui's global `DockContext`, binary split tree, per-frame DockSpace keep-alive, PlatformIO callback table, DockBuilder node pointers, or `.ini` persistence;
- built-in Dock snapshot file I/O, automatic persistence debounce, or a generic mutable service locator for Dock styling;
- atomic multi-property native window transactions where the operating system exposes only independent asynchronous requests;
- locale and logical direction as an isolated flag or partial accessibility-only API; the required cross-domain design epic remains separate from this convergence plan;
- platform-specific screenshot baselines that cannot be generated and verified on the active platform. Gallery runtime and structural smoke coverage remains required.

## Planning Contract

### Key Technical Decisions

KTD1. **Replace, do not layer.** A new runtime or derived projection becomes the sole path for its facts in the owning unit. The old forwarding facade, duplicate state, callback, or evidence path is deleted before the unit completes.

KTD2. **Keep renderer-neutral policy separate from GPUI lifecycle.** Pure stack/status/typeahead resolution belongs in `ui_core` or the owning domain crate. Focus handles, keyed state, AccessKit node updates, event subscriptions, frame scheduling, and deferred rendering remain GPUI adapter responsibilities.

KTD3. **Window ownership is real ownership.** Overlay/focus and active theme selection cannot be modeled as app-global maps keyed by element IDs. Window state uses GPUI window-owned state; subtree context is frame-scoped and stack-balanced.

KTD4. **Callbacks observe the correct ownership state.** Uncontrolled framework state commits before its semantic callback, followed by end-of-turn focus arbitration. Controlled paths emit intent against the owner's currently committed state and do not perform state-dependent cleanup or focus restoration until the owner commits the new state/presence. A newer focus claim wins over an older restore claim.

KTD5. **Controlled components emit intent only.** A close or value-change request does not mutate caller-owned state. Runtime registration and focus restoration follow actual controlled state/presence.

KTD6. **The final accessibility tree is a required test surface.** Model-level tests remain valuable, but claims of role, relation, action, focus, and stale-node cleanup require final `TreeUpdate` assertions.

KTD7. **Semantic activation is not a second command system.** It normalizes UI entry paths and payloads. `ActionDescriptor` continues to own reusable presentation facts; command registry/dispatch continues to own command execution.

KTD8. **Theme context is immutable at render time.** App registry and built-in definitions may be global, but active selection and overrides resolve to immutable snapshots with runtime-owned effective revisions.

KTD9. **Typed context is narrow, not a service locator.** Scoped theme resolution begins with a prototype comparing a theme-specific provider with a GPUI inherited-context primitive. A public generic GPUI primitive is accepted only if the theme-specific path cannot satisfy nested/deferred behavior and a current non-theme domain independently requires the same immutable stack semantics. Deferred theme capture is not counted as a second consumer. The primitive cannot hold arbitrary mutable services.

KTD10. **Conformance is federated and native tests remain native.** Component rows, Gallery probes, exports, and native test scenarios keep narrow ownership; xtask cross-checks their structured IDs without generating all surfaces from one registry. Conformance does not replace the test suite with function pointers or a mega-test runner.

KTD11. **Public API cleanup is evidence-led.** Table's engine and `ActionDescriptor` are preservation gates. Diagnostic table snapshots can move out of common exports after a workspace consumer census, but deeper removal requires a superseding ADR and separate evidence.

KTD12. **Breaking means clean replacement.** No deprecated aliases are retained for old semantic callbacks, color-only theme models/schema, overlay forwarding helpers, or evidence tables. The project is unreleased: the expanded theme contract deletes and replaces the old format in place and remains version `v1`; there is no compatibility loader, dual model, or `v2` naming.

KTD13. **TanStack Table is a semantic reference, not an implementation dependency.** The repository-local `repo-ref/tanstack-table` reference clone (currently `@tanstack/table-core` `9.0.0-beta.31` at `5af79a877fa80f63703c6dc21861acc9d18baecf`) anchors row-model ordering, stable row/column identity, client/manual stage ownership, and pinning-region semantics. Open GPUI retains its Rust engine, GPUI adapter, native keyboard/accessibility policy, and separate virtualizer; this plan does not adopt TanStack v9 atoms/plugins, add a runtime dependency, or pursue full API parity. Row pinning addresses exact authoritative row identities by default; pinning every source instance with one business row ID is an explicitly named bulk target, never implicit string coercion. Occurrence identity is exact only inside its current resolved source snapshot, while retained state across reorder requires a caller-owned explicit instance ID. Caller target order owns order within each pinned region, each bulk target expands in current model order, top targets are resolved first, and a logical row already claimed by top is excluded from bottom. Open GPUI intentionally diverges from TanStack's default index/string identity, stringified group keys, overlap tolerance, and core-row fallback for filtered pinned rows; typed grouping, duplicate diagnostics, top-wins partitioning, filter-aware pinning, and independent Virtualizer ownership remain local contracts.

KTD14. **Complete theme scales are immutable values, not another registry.** Theme v1 reuses renderer-neutral `Density` and `MotionPreference` vocabulary and carries the admitted design scales directly in the immutable snapshot/context. It does not add a parallel scale registry or string lookup layer. Explicit component `Size` outranks theme density; adaptive device density remains a host recommendation rather than an implicit recipe input. Reduced motion is a safety floor: either theme policy or an explicit component request may reduce motion, while no component request may relax a reduced theme. Motion execution remains owned by `open-gpui-motion`.

KTD15. **One transform owns observable subtree geometry.** A narrowly named immutable value validates positive finite normal scale with a representable finite reciprocal, finite translation, and explicit origin before entering a frame-scoped `Window` transform stack. A child's local transform applies before its parent's, so the resolved mapping is `parent_resolved compose child_local`; checked inverse/composition rejects overflow, underflow to zero, non-finite multiply-add results, and values outside the backend-representable contract. A failure preserves layout but suppresses the entire subtree before paint/input/focus/accessibility registration, with a structured diagnostic and no identity/clamp fallback. Layout and measurement stay in untransformed logical coordinates. The resolved transform is projected into scene primitives, rectangular clips, hitboxes and inverse local-coordinate conversion, pointer capture, IME/debug bounds, final accessibility bounds, deferred work, portal anchors, and cached replay. Backend matrices are projections of this authority rather than a second public transform model.

KTD16. **Presentation is one inherited subtree policy.** `Visible`, `Inert`, and `Hidden` form one frame-scoped state that is resolved before paint, hitbox/listener registration, focus/IME registration, and accessibility projection. Hidden dominates inert, and a descendant cannot opt out of an ancestor's suppression. `Display::None` remains the separate layout-removing mechanism, while component disabled state and decorative semantic omission remain domain facts rather than alternate subtree-presentation switches.

KTD17. **Announcements are committed semantics, not speech commands.** Declarative live regions flow through `SemanticDescriptor` and transient window requests synthesize short-lived nodes into the same final AccessKit `TreeUpdate`. GPUI owns sequence, retention, activation-generation, privacy, and window lifecycle; AccessKit adapters own native events and assistive technology owns delivery. Announcement requests never move focus or call a backend speech service directly.

KTD18. **Public geometry handles grant one named capability.** Portal anchors and reveal targets may share private frame-binding machinery, but each public handle is window-owned and exposes only the committed facts its operation needs. Open GPUI does not add a generic public node reference, resolved matrix, mutable last-known rectangle, or cross-window portal capability.

KTD19. **Bring-into-view owns scrolling; focus only requests it.** One generation-based window authority walks committed scroll ancestry inner-to-outer for application, focus, and AccessKit requests. Requests with disjoint chains may proceed independently; a newer request supersedes older work sharing any scroll container before either mutates that container again. Physical horizontal/vertical min-edge, max-edge, center, and nearest alignment ship now; logical block/inline and start/end wait for the direction authority. Virtual collections materialize stable logical identity before binding a physical reveal target. Motion supplies timing samples, theme supplies reduced-motion policy, and neither owns reveal state.

KTD20. **Rounded clipping is one geometry stack, not a renderer effect.** Existing rectangular masks and overflow inputs converge on a checked rect/rounded-rect stack used by scene culling, renderer clips, hit testing, debug geometry, deferred/cache replay, and conservative accessibility projection. Non-uniform scale preserves elliptical radii. Unsupported native-surface or backend combinations fail closed; arbitrary path syntax waits for a separate winding/tessellation/stencil design.

KTD21. **ImGui defines Dock behavior, not Open GPUI architecture.** The repository-local `repo-ref/imgui` docking clone anchors drop rectangles and previews, hovered viewport routing, selected-tab behavior, focus history, splitter hit behavior, and tear-off/reuse/close multi-viewport lifecycle edge cases. Open GPUI retains its Rust retained-mode `DockGraph`, `DockHost`, transaction, session, and generation architecture; global immediate-mode context, raw node pointers, callback tables, per-frame `DockSpace` construction, `DockBuilder`, PlatformIO ownership, and `.ini` persistence are rejected.

KTD22. **Dock style is a named immutable render input.** `gpui_docking` owns one complete `DockVisualStyle` value and built-in fallback. A narrow resolver is installed as an immutable per-surface value, or passed explicitly to a low-level host; there is no mutable app-global registration. It synchronously reads only the active GPUI render context, cannot update entities, notify, dispatch, or reenter rendering, and lets application integration map `ThemeResolver::current` without a reverse dependency on UI Components or a generic context/service API. Target-host affordances resolve target style. Source-owned deferred drag visuals freeze an out-of-band style snapshot keyed by drag session/opening generation, never add visual values to `DockDragPayload` or its equality/route identity, and clear that snapshot on cancel/close before reopen captures anew. Existing drop-guide layout values are renamed `DockDropGuideMetrics` so they cannot appear to compete with the visual authority.

KTD23. **DockSurface publishes committed facts; applications own persistence.** A private owner entity allocates an explicit root transaction identity at every facade, host, or runtime mutation boundary and threads it through typed controller and viewport-runtime commits. Nested work carrying that identity coalesces categories and publishes once when the transaction commits; two root commands in one App turn and work across turns never merge. Asynchronous platform observations enter a new observation transaction and dispatch status never creates a persistence revision. Each Dock space has at most one committed activation-host registration generation; duplicate live registrations reject rather than silently replace. `select_panel` remains selection-only, while stable-item `activate_panel` routes through that owner and settles through GPUI focus completion. Snapshot export is explicit and revision-consistent; storage, serialization timing, debounce, and I/O remain caller responsibilities.

KTD24. **Window mutation is dispatch plus terminal observation, not synchronous truth.** Backend setters first adopt an honest dispatch contract: queued means GPUI handed work to a backend path, not that the OS accepted or applied it, and legacy unit/bool returns cannot be guessed into richer outcomes. A generation-bound ticket later settles from a coherent observed `WindowPlatformFacts` snapshot as exact, adjusted, superseded, rejected, unsupported, or window closed. Position, size, state, and restore bounds share one placement conflict domain and generation because native state transitions move/resize windows; the GPUI planner rejects contradictory placement batches before dispatch and never exposes partially committed placement. Lifetime `accepts_activation` and `focus_on_click` are independent fields in one coherent `ActivationPolicy` domain with one generation and terminal observation; they are never aliases and cannot partially commit. Other independent flag domains may partially dispatch. Public bounds/state/flag getters read this committed cache, seeded at creation and updated only by platform callbacks or an explicit observation refresh that enters the same generation/event path.

KTD25. **Platform callbacks are sequenced facts, not borrow-or-drop notifications.** `AppCell` owns a private native-event ingress beside, not inside, its `RefCell<App>`. Asynchronous callbacks take one application-wide ingress sequence before entering that inbox. Direct delivery is an optimization only when no backlog, active drain, or barrier can be overtaken; otherwise the event queues and wakes the foreground executor. Ordering barriers and generation-scoped per-domain merge rules are part of the runtime contract, not backend folklore. Synchronous native queries use immutable AppCell-adjacent snapshots or documented conservative responses instead of blocking or reentering `App`. Hybrid input whose native default handling depends on the current handler result is a separate contract: fixed snapshots and delayed replay are forbidden, and framework-owned pump-sensitive commands must make an App-busy entrance unreachable so every consumed/propagated result comes from the real idle handler. Frame validation acknowledges accepted or rescheduled work rather than a callback invocation, and diagnostics identify callback kind, full `WindowId`, domain generation, sequence, and disposition.

KTD26. **Native effects cross the borrow boundary required by their reentrancy class.** GPUI and Dock calculate typed effects while entity, controller, or viewport-runtime state is borrowed, release those guards, then apply model-owned open, close, remove, present, and mutation work through the owning `&mut App`. The closed set of platform commands that can synchronously pump native input or delegate callbacks (`activate`, native window menu, interactive move, and interactive resize) instead enters an AppCell-owned FIFO and executes synchronously after the outer `AppRefMut` is released and older native-event barriers settle. The command FIFO carries a full `WindowId`, uses a weak backend dispatcher plus terminal validation, drains non-recursively in order, and is not an arbitrary callback or asynchronous native-effect outbox. `App::open_window` remains synchronous, construction/map remains fallible in the window transaction, close remains ownership-governed, and draw/present remains frame-governed. Events targeting a reserved/current full `WindowId` wait until commit or rollback, then drain or retire. No backend's current asynchronous implementation is treated as a non-reentrancy guarantee.

KTD27. **Initial appearance, lifetime activation, and native ownership are separate contracts.** `focus_on_appearing` is a one-shot show policy. Click/programmatic activation and permanent non-activation are lifetime capabilities. `transient_for`/owner is a typed top-level relationship for z-order, activation, and platform grouping, never `WS_CHILD` and never a substitute for explicit teardown. A provisional viewport is created with the final detached window's lifetime native capabilities but without initial activation; a generation-bound Dock session gate, not an unsupported native flag transition, suppresses input, focus, activation, route eligibility, and accessibility until same-window promotion.

KTD28. **DockSurface owns a window session, not application exit.** `open_primary_window` first reserves an `Opening` anchor token, then calls synchronous `App::open_window` inside the current App transaction. Registry commit returns the full `WindowId`; a short owner borrow validates that ID and token before transitioning to the one current `Active` anchor generation. Native callbacks raised during creation queue until commit or rollback and never activate the session themselves. Opening failure and synchronous close settle that token without admitting viewport work. Active-anchor shutdown first retires the session and freezes new work, then closes an ownership snapshot outside all borrows. Surface-owned forced teardown bypasses individual viewport close policies and is isolated from other surfaces. `ShuttingDown` reaches `Closed` only when the current-generation runtime registry is empty and every snapshotted native window has a terminal close observation or is confirmed absent; reopen is rejected until then. A later generation remains isolated from delayed prior-generation callbacks. Last-window application exit stays GPUI policy rather than a Dock `quit` shortcut.

KTD29. **Live undock and durable graph commit are separate.** The native capture owner transports the current screen point plus an ordered application-window underlay stack; Dock resolves the first eligible same-surface host after skipping provisional/session-suppressed windows, and target HWND input is not required. A foreign-surface host is a typed forbidden target, never desktop fallback. Dock interaction/controller state exclusively owns the panel presentation lease, stable source semantic/focus proxy, and graph saga; the viewport runtime solely stores window handles, DockSurface owns session/shutdown authority over that registry, and GPUI reports only native frame/presentation facts. Release revalidates the target and completes every fallible promotion preflight or native effect while the provisional remains session-suppressed. One final App transaction atomically commits graph ownership, viewport registration, presentation lease, semantic/focus ownership, and gate removal; generation-bound queued input then drains after commit or retires after rollback. Promotion activates the committed destination and restores the valid pre-drag descendant through the destination activation host, while cancellation restores only a live source and source deactivation/shutdown never steals focus. ImGui anchors the visible-before-release behavior and platform-window staging, while Open GPUI retains retained graph transactions and rollback.

### High-Level Technical Design

Authority flow after the refactor:

```text
domain state / component resolved state
             |
             +--> semantic descriptor ------> GPUI element projection
             |              |                         |
             |              +--> DevTools             +--> final AccessKit TreeUpdate
             |                                        +--> semantic AccessKit action
             |
             +--> semantic activation transaction <--- pointer / key / a11y / programmatic
```

Overlay and focus ownership:

```text
Window
  `-- WindowOverlayRuntime
        |-- ordered layer registry (stable instance + parent)
        |-- topmost dismiss arbitration
        |-- modal underlay policy
        |-- FocusScopeRuntime
        |     |-- live target registry
        |     |-- initial-focus claim
        |     |-- Tab / Shift-Tab loop
        |     `-- LIFO restore claim
        `-- presence lifecycle (open -> closing -> unmounted)
```

Theme resolution:

```text
built-in fallback
       -> app selection
             -> window selection/override
                   -> nested subtree override
                         -> immutable effective ThemeSnapshot
                               -> component recipes / deferred overlay capture
```

Form lifecycle:

```text
field value revision + validation generations ---> validation activity
submit begin/finish ------------------------------> submission phase
validation activity + submission phase ----------> derived FormStatus / eligibility
                                                    |
                                                    +-> snapshot -> UI / a11y / DevTools
```

Post-layout subtree geometry:

```text
untransformed layout bounds
        -> validated axis-aligned transform scope
              |-> composed scene primitives / rectangular clips / diagnostics / IME
              |-> transformed hitboxes -> inverse target-local input coordinates
              |-> final AccessKit bounds and actions
              `-> transform-aware deferred, portal-anchor, cache, and motion projection
```

Layout-preserving presentation:

```text
Visible -> layout + paint + input + focus/IME + accessibility
Inert   -> layout + paint
Hidden  -> layout
```

Committed live semantics:

```text
domain/component state -> SemanticDescriptor live facts ----+
                                                          |
window announcement request -> bounded sequenced queue -----+-> final AccessKit TreeUpdate
                                                                 |-> native live-region event
                                                                 `-> later committed removal
```

Geometry capabilities after U12/U13:

```text
committed element binding
        |-> PortalAnchorHandle -> explicit follower unlink policy -> OverlayAnchorInput
        `-> RevealTargetHandle -> committed scroll ancestry -> inner-to-outer reveal

layout bounds -> checked rect/rounded clip stack -> scene + hit test + debug + a11y projection
```

Dock presentation and application ownership:

```text
surface/host resolver + active render context -> immutable DockVisualStyle -> host/floating/guide paint
drag opening session/generation ------------------> out-of-band frozen visual snapshot

logical transaction id -> DockController committed categories --+
                                                              +-> private DockSurface owner -> one revision/event -> caller snapshot/debounce/I/O
logical transaction id -> viewport observed categories --------+

stable panel activation -> unique host registration generation -> selection/window intent -> GPUI focus completion
```

Live platform-window mutation:

```text
Dock viewport sync -> capability-specific request -> Queued(ticket) / Unchanged / Unsupported / Rejected / Closed
                                                           |
native coherent placement/flag callbacks -> committed WindowPlatformFacts -> terminal ticket observation
                                                           |
                                                           `-> public getters + Dock route/snapshot reconciliation
```

Reentrancy-safe native event delivery:

```text
native callback
      -> typed envelope(window instance + generation + domain + sequence)
            |-> App available: ordered delivery
            `-> App busy: mailbox -> foreground wake -> ordered drain
                                      |-> explicit per-domain coalescing
                                      |-> cancel/close ordering barriers
                                      `-> accepted frame or re-invalidation

model/runtime transaction -> typed effects -> release subordinate borrows -> App-owned work
                                      `-> release outer AppRefMut -> typed pump-sensitive commands
```

Dock native window session and live drag:

```text
DockSurface window session
  Vacant/Closed -> Opening(anchor token) -> Active(anchor generation)
      |-> committed viewport ownership
      |-> active drag generation
      |     `-> local -> routed target / provisional visible viewport
      |                         |-> valid-host release: transfer + graph commit
      |                         |-> desktop release: promote + graph commit
      |                         `-> cancel/failure/close: restore + destroy
      `-> anchor close
             -> ShuttingDown (freeze + cancel + snapshot)
             -> close owned windows outside borrows
             -> terminal close observations + empty current-generation registry
             -> Closed (reopen eligible)
```

Live-undock presentation:

```text
Opening       -> source snapshot/proxy remains authoritative; no empty native shell
DesktopVisible -> one non-empty provisional follows the pointer
HostPreview   -> retain but hide provisional; target host owns the guide
Rejected      -> retain but hide provisional; forbidden host owns rejected feedback
Unavailable   -> source presentation remains authoritative
Commit        -> atomic graph/registry/lease/semantic/gate handoff, then activation
Cancel/Failure -> restore only a live source, destroy provisional, retire generation
```

State ownership remains non-overlapping:

| Authority | Sole state owned |
| --- | --- |
| AppCell native-event ingress | callback ingress sequence, queued envelopes, drain/barrier state, immutable synchronous-query snapshots |
| App/window transaction | reserved/current full `WindowId`, commit/rollback boundary, and application of typed native effects after subordinate borrows release |
| GPUI active drag | generic capture-owner pointer transport and terminal input |
| DockSurface owner | session phase, anchor token/generation, and shutdown authority |
| Dock viewport runtime | sole generation-tagged registry of committed and provisional window handles |
| Dock live-undock session | current route, provisional presentation state/handle reference, panel presentation lease, source semantic/focus proxy, and release/compensation saga |

The DockSurface owner snapshots the viewport runtime registry during shutdown but does not mirror it. GPUI does not understand Dock panels or presentation leases, and the Dock live-undock session does not become a second durable graph.

Dependency order:

```text
U1 Form Lifecycle ----------------------------------------------+
                                                                 |
U2 Final AccessKit Harness ------+                               |
                                 +-> U5 A11y Semantic Authority  |
U3 Focus Scope (preparatory) -> U4 Window Overlay Runtime -------+-> U6 Activation
                                                                 |
U7 Scoped Theme Resolution -> U8 Complete Theme V1 --------------+
U9 Collection Typeahead -----------------------------------------+
                                                                 |
U1 + U5 + U6 + U8 + U9 -> U10 Federated Conformance Cleanup -> U11 Prior-Surface Audit
                                                                        |
                                                                        v
U12 Interactive Subtree Transform -> U13 Presentation State -> U14 Live Regions
                                                                        |
                                                                        v
U15 Typed Portal Anchors -> U16 Bring Into View -> U17 Rounded Clip
                                                        |
                                                        v
U18 Dock Visual Style -> U19 DockSurface Owner/Intent -> U20 Window Mutation
                                                             |
                                                             v
U24 Platform Event Safety -> U25 Window Appearance/Owner -> U26 Dock Window Session
                                                             |
                                                             v
U27 Native Drag + Live Tear-Off -> U28 Owning-Platform Evidence -> final gate
```

U2 and U3 have no logical dependency and may be developed independently, although shared-worktree execution may serialize their Cargo gates. U3 and U4 share one authority-completion gate: U3 may commit pure policy and private preparation, but Focus Scope is not declared the production authority until U4 has migrated official overlay consumers and removed their duplicate focus bookkeeping. U12-U20 are serialized after the prior product-surface audit because they change GPUI frame boundaries or converge consumers that exercise those authorities. U14 lands before geometry follow-ons because it closes a renderer-independent accessibility gap. U15 establishes typed committed targets before U16 records scroll ancestry, and U17 follows a renderer/ABI review gate before changing every primitive clip representation. U18 then removes Dock's visual dual authority. U19 installs one application owner over Dock commits but admits only observed viewport facts, never the current sync layer's optimistic `applied` status; U20 reshapes backend dispatch and observation through that stable owner seam without preserving a false revision contract.

The native docking correction remains strictly ordered. U24 fixes the lower-level event-loss and effect-reentrancy substrate before later units depend on callback delivery. U25 separates window appearance, activation, and ownership on that substrate. U26 then gives every provisional or committed viewport a session owner and teardown path. U27 may create and route live provisional windows only after those lifecycle guarantees exist. U28 supplies owning-platform evidence and cannot be replaced by `VisualTestContext` coverage.

### Assumptions

- The active branch starts from a clean `main` at or after commit `67f0048d`; user work appearing later must be preserved and reconciled.
- Open GPUI remains pre-1.0, so concentrated breaking changes are acceptable when documented and migrated atomically.
- `cargo nextest` is the primary test runner; broad Windows builds may need `CARGO_BUILD_JOBS=1` to avoid linker/page-file failures.
- `TestPlatform` can retain final accessibility updates without requiring a real OS accessibility bridge.
- The project and theme schema have not been released. Workspace call sites are migration targets, not compatibility obligations; the old color-only schema can be deleted and replaced by the complete contract under the `v1` name.
- Existing ADRs remain binding unless explicitly superseded or amended by this work.
- Supported renderer backends can compile the same backend-neutral scene contract on their native CI runners; an active-platform render smoke cannot stand in for those checks.
- The workspace's AccessKit version exposes live politeness, atomicity, and busy state, and its supported platform adapters derive native announcements from committed tree changes rather than requiring a direct speech API.
- Existing renderer rounded-rectangle math is reusable only after U17 proves one shared clip ABI and exact hit-test contract; shader support alone is not treated as subtree-clip support.
- Existing platform backends may support only a subset of placement and independent flag mutations. U20 records partial capability honestly across independent conflict domains, while position, size, state, and restore bounds remain one coherent placement domain rather than a partially committed native batch.
- Native APIs may synchronously pump callbacks during create, show, position, activate, or close regardless of whether a current backend usually defers one operation. KTD25-KTD26 therefore treat reentrancy as a platform contract, not a Windows-only anomaly.
- Native owner/transient semantics differ by backend and may be unavailable on some Wayland configurations. Explicit DockSurface teardown remains authoritative even when platform grouping is supported.

### Phased Delivery

Phase 0: Commit this plan, create the breaking-change inventory, lock characterization tests, and inventory workspace consumers. The inventory sizes mechanical migrations and identifies legitimate raw-event consumers; it does not create compatibility code for unreleased APIs or schemas.

Phase 1: Land correctness/proof foundations: U1 Form lifecycle and U2 final AccessKit harness.

Phase 2: Build U3 Focus Scope as a preparatory slice, then use U4's pilot and fleet migration to make Focus Scope and Window Overlay Runtime production authorities under one completion gate.

Phase 3: Land semantic convergence: U5 Accessibility and U6 Activation.

Phase 4: Land design-context depth: U7 scoped theme resolution using the existing immutable snapshot, U8's complete replacement Theme v1 on the proven scope channel, and U9 typeahead.

Phase 5: Delete duplicate authorities and align the U1-U10 product surfaces through U10 and U11.

Phase 6: Establish the two cross-channel GPUI substrate authorities in blast-radius order: U12 owns interactive subtree geometry, then U13 owns layout-preserving presentation.

Phase 7: Close the researched interface gaps in dependency order: U14 committed live regions, U15 typed portal anchors, U16 nested bring-into-view, and U17 rounded-rectangle subtree clipping. U17 begins with a renderer/ABI review gate before public syntax or broad migration.

Phase 8: Converge Dock as a real consumer rather than a special-case subsystem: U18 replaces hard-coded visual state, U19 makes DockSurface the committed application owner, and U20 closes the GPUI live-window mutation gap required by multi-viewport behavior.

Phase 9: Close the native multi-viewport correctness gap discovered by real Windows use: U24 makes callback delivery and native effects reentrancy-safe, U25 separates appearance/activation/owner semantics, U26 adds DockSurface window sessions and deterministic teardown, U27 adds source-capture routing and visible provisional tear-off, and U28 proves the behavior with real native windows and corrects overstated verification claims. These are completion units, not post-plan candidates.

Each unit receives a focused commit after its tests and local review pass. Wide mechanical migrations are serialized even where model work could theoretically run in parallel.

### System-Wide Impact

- `crates/gpui`: test accessibility capture/action support; narrowly scoped inherited render context only if U7's prototype and independent-consumer proof hold; focus/tab-stop support needed by U3; authoritative subtree transform, presentation, announcement, anchor, reveal, and clip state spanning scene, input, focus/IME, accessibility, deferred work, and frame-journal replay; AppCell-adjacent native-event ingress and synchronous-query snapshots, run-loop wake/fair draining, reserved-window commit/rollback delivery, drag transport, first-presentation acknowledgement, and separate appearance/activation/owner window contracts.
- `crates/form`: validation generation/activity and derived status authority.
- `crates/ui_core`: pure focus/overlay policies, semantic descriptors including renderer-neutral live facts, reveal alignment vocabulary, tokens, and public contract boundaries.
- `crates/ui_components`: window runtime adapters, component migrations, recipes, typeahead session, federated contract/probe bindings, public callback breaks, live-region consumers, typed overlay-anchor conversion, and virtual reveal materialization.
- `crates/open-gpui-command`: call-site migration only unless a concrete command bridge defect is exposed; command ownership remains unchanged.
- `crates/devtools`: projection from real semantic/runtime authorities with redaction.
- `crates/gpui_docking`: immutable visual style resolution, private surface owner and revision/events, stable-item activation completion, platform-window mutation consumption, two-phase runtime effects, explicit window sessions, source-capture routing, presentation leases, and provisional viewport promotion/rollback while preserving retained graph/session architecture.
- `crates/gpui_wgpu`, `crates/gpui_windows`, `crates/gpui_macos`, and `crates/gpui_linux`: consume backend-neutral transformed scene and rounded-clip contracts, prove matrix/primitive ABI consistency, and distinguish accepted draw, submitted present, non-empty presentation, renderer/device/surface failure, and native visibility on supported runners.
- `crates/gpui_windows`, `crates/gpui_macos`, and `crates/gpui_linux`: additionally implement and report the supported position, size, state, dynamic window-flag, initial-appearance, lifetime-activation, and owner/transient capabilities without overstating unavailable behavior.
- `crates/gpui_web`: compile against the split appearance/activation/owner and callback/effect contracts, project unsupported native ownership honestly, and preserve its run-loop/presentation behavior without fabricating desktop capabilities.
- `crates/motion`: adapt scale/translation motion to the GPUI transform authority without taking geometry ownership.
- `examples/ui-foundation-gallery`: real lifecycle scenarios and contract-derived catalog.
- `examples/docking-native`, `examples/docking-minimal`, and `examples/docking-multiviewport`: style-scope, application-event/persistence, activation, live-window capability, capture-owned drag, visible provisional viewport, and anchor-teardown demonstrations without adding Docking to the foundation Gallery dependency surface.
- `xtask`, CI, docs, and ADRs: structured conformance, new gates, migrations, architecture decisions, and honest separation between simulated visual coverage and real native-window integration.

### Risks & Mitigations

**Overlay dual authority.** A runtime stack can drift from adapter `open` state or re-enter during callbacks. Mitigation: controlled intent semantics, committed-state callback ordering, end-of-turn focus claims, and deletion of component-owned close tails.

**Half-built focus trap.** A model-only focus scope can claim success without trapping real tab traversal. Mitigation: `TestAppContext` keyboard tests are a merge gate for U3/U4.

**A fourth accessibility fact table.** A new descriptor can coexist with static evidence and render attributes. Mitigation: migrate by family and delete that family's hand-written evidence/projection in the same commit; final-tree tests are mandatory.

**Theme abstraction outruns consumers.** Generic context or tokens can become broader than their use. Mitigation: two-consumer rule, immutable values, no service lookup, semantic metrics only, and explicit stop conditions in U7/U8.

**Activation loses legitimate pointer detail.** Mitigation: inventory consumers before migration and provide an explicitly named raw path only for proven modifiers/position use cases.

**Conformance becomes a mega-test framework.** Mitigation: retain isolated native tests and map typed scenario IDs to them; measure deleted duplicate code against new infrastructure.

**Public churn without depth.** Mitigation: preserve Table/Virtualizer/Action/a11y vocabulary; common-export cleanup needs a consumer census and cannot alter engine behavior.

**Visual-only transform authority.** A renderer matrix can make a subtree look correct while hit testing, clips, pointer capture, IME, AccessKit, deferred work, or cached replay remains untransformed. Mitigation: U12 starts from a backend-neutral validated geometry value, records one resolved frame transform on every affected channel, and requires cross-channel invariant tests before any public wrapper ships.

**Transform scope outruns renderer support.** Adding general affine syntax would expose rotation/skew behavior that rectangular clips, text rasterization, native surfaces, and accessibility bounds cannot yet honor. Even restricted `f32` transforms can overflow or lose an inverse when nested. Mitigation: the public contract accepts only finite positive axis-aligned scale and translation with an explicit origin, uses checked composition/backend conversion, and fail-closes the complete subtree on numeric failure; unsupported forms have no placeholders, clamps, or identity fallbacks.

**Presentation dual authority.** A paint-only hidden flag or independent accessibility/focus suppression can leave invisible interactive descendants or visible inert semantics in the final tree. Mitigation: U13 resolves one inherited presentation state before channel registration, tests dynamic stale-state cleanup, and deletes the old subtree-level gates in the same unit.

**Announcement bypass or privacy leak.** A direct native announce helper can diverge from final-tree membership, replay after accessibility activation, steal focus, or retain sensitive message text in DevTools history. Mitigation: U14 uses one committed-tree authority, bounded per-window sequences, activation-generation checks, deterministic removal, and metadata-only diagnostics with unique canary tests.

**Stale or ambiguous portal geometry.** Raw rectangles and last-known snapshots can cross windows, survive unmount, or carry the wrong transform/clip generation. Mitigation: U15 binds one typed handle per target and frame, separates current candidate from committed reads, returns an explicit unlinked state, rejects wrong-window use, and gives followers an explicit unlink policy.

**Reveal loops and user-scroll fights.** Independent focus/list/accessibility scroll tails can over-scroll nested containers or continue after the user intervenes. Mitigation: U16 owns request generations, committed inner-to-outer ancestry, transform-aware deltas, no-progress termination, focus-claim ordering, and cancellation on newer requests, suppression, unmount, close, or direct user scroll.

**Rounded clip approximated as an AABB.** Reusing rectangular masks or per-primitive corner code can paint or hit outside corners and disagree across backends. Mitigation: U17 keeps an exact nested clip stack, shares normalized checked geometry with hit testing and renderer ABI, defines native-surface rejection, and requires backend conversion plus pixel evidence before the public contract completes.

**Dock styling becomes a theme dependency cycle or global authority.** Moving `ThemeContext` into Docking, installing a mutable app-global resolver, or letting every render path keep local colors would invert dependencies, cross-contaminate surfaces, or preserve drift. Mitigation: U18 owns a complete Dock-native style value and immutable per-surface/explicit-host pure resolver; application integration maps the current theme, drag visuals use out-of-band generation snapshots, and source scans reject production color literals outside the built-in fallback.

**Dock events report intent or coalesce unrelated work.** Selection requests, focus requests, window activation, controller mutations, and viewport observations can settle at different times; end-of-turn batching cannot distinguish one tear-off from two independent commands. Mitigation: U19 threads an explicit root transaction identity through controller/runtime commits, publishes once at transaction completion, never revisions from mutation dispatch, rejects duplicate live activation hosts, generation-guards stale callbacks, and leaves persistence timing to the application.

**Window mutation capability lies or races observation.** Existing unit/bool setters cannot distinguish unchanged, queued, or later native failure, state transitions also move/resize windows, and direct backend getters can bypass an observation cache. Mitigation: U20 reshapes backend dispatch first, defines one placement conflict domain plus independent flag domains, returns generation-bound tickets, settles only from coherent callbacks into one `WindowPlatformFacts` authority used by all public getters, and tests adjustment, partial support, stale generations, close, and external window-manager movement.

**Platform callbacks disappear under App reentrancy.** A nested native message can fail the shared `App` borrow, and coalescing every callback would silently erase input or terminal facts. Mitigation: U24 introduces typed envelopes, per-domain merge rules, FIFO terminal delivery, cancel/close barriers, full generation-bearing `WindowId` values, bounded foreground draining, callback-specific diagnostics, and deterministic already-borrowed tests. A failed frame remains invalidated until accepted.

**A later fast path overtakes queued causality.** A directly deliverable callback can arrive after an older queued event, while related source, target, and anchor events span multiple windows. Mitigation: every callback receives one application-wide ingress sequence first; inline drain requires an empty backlog and exclusive drain ownership, barriers span the relevant session generation, and merge keys include full `WindowId` plus their actual domain generation.

**A synchronous native query cannot wait for the inbox.** Hit testing, close permission, and other snapshot-answerable callbacks may require an answer while `App` is borrowed. Mitigation: U24 classifies queries separately, publishes immutable AppCell-adjacent snapshots, defines conservative responses only where they are semantically equivalent, and queues follow-up facts without blocking or recursive App access.

**Hybrid input fallback changes native behavior.** The current event handler dynamically decides whether native default handling propagates, so a fixed snapshot can suppress a real default and delayed replay can execute it twice. The real result also cannot be computed while the same thread exclusively borrows `App`: queueing misses the WndProc return, blocking deadlocks, and recursive mutation is unsound. Mitigation: U24 inventories hybrid message classes separately, moves every framework-owned pump-sensitive platform command to a closed FIFO that runs after `AppRefMut` release and older barriers, returns the real handler result with `AppCell` idle, and treats any busy entrance as an invariant violation. Consumed and propagated native tests prove both the result and a zero-busy count.

**Inbox delivery does not make native effects borrow-safe.** Opening, closing, activating, or updating sibling windows while holding an entity/controller/viewport-runtime guard can synchronously invoke in-process observers that reborrow that state, while selected platform commands can also pump must-immediate input under the outer App borrow. Mitigation: U24 makes Dock/model domains return typed effects and releases subordinate guards before applying them through `&mut App`; the closed pump-sensitive command set crosses the outer AppCell borrow through its synchronous FIFO. Reserved-window callbacks drain or retire only after App/window commit or rollback. A generic callback outbox and asynchronous open-window outbox remain deliberately forbidden.

**Creation or presentation fails between pending and active state.** Staged synchronous creation can fail, pump a close callback, roll back its reserved window, or lose a renderer surface after an anchor token or presentation lease exists. Mitigation: U24 holds create-time callbacks until App/window commit or rollback, U26 activates the opening token only from the returned committed full `WindowId`, U25 separates accepted draw from non-empty presentation, and U27 compensates renderer/device/surface failures before any durable graph commit.

**Initial no-focus becomes permanent no-activation.** A single boolean can map a harmless initial-show preference to `WS_EX_NOACTIVATE`, leaving detached windows impossible to activate later. Mitigation: U25 separates one-shot appearance, lifetime activation/click, input acceptance, and owner/transient facts; native tests assert both non-stealing first show and later activation.

**Native ownership is mistaken for lifecycle authority.** OS owner/group semantics differ and may not cascade every shutdown case. Mitigation: U25 uses native ownership only for supported z-order/activation UX, while U26's generation-bound DockSurface session always performs explicit, surface-scoped, idempotent teardown.

**Captured drag routes stale or foreign targets.** Screen coordinates, DPI, host scenes, full `WindowId`, or Dock surface/session ownership can change between preview and release, while a visible provisional may cover the real host. Mitigation: U27 resolves an ordered native/application underlay stack and current coordinate conversion from the capture owner, skips provisional/session-suppressed windows, accepts only current same-surface host registrations, classifies foreign-surface hosts as rejected rather than desktop fallback, revalidates release, and treats cancel/close as barriers.

**Provisional viewports leak, flash, or duplicate semantic ownership.** Repeated motion, failed first paint, target switching, delayed promotion, or shutdown can leave windows alive, expose an empty shell, or render one panel as two focus/accessibility owners. Mitigation: U27 allows one provisional viewport, one presentation lease, and one source semantic/focus proxy per drag generation; defines explicit opening/desktop/host/rejected/unavailable/terminal presentation states; keeps durable graph/revision unchanged; completes fallible work under session suppression; and atomically hands off graph, registry, lease, semantic/focus ownership, and the gate before destination activation.

**Native tests reproduce only the model.** `VisualTestContext` does not exercise HWND capture, WndProc reentrancy, paint validation, z-order, or process lifetime. Mitigation: U28 adds deterministic real-window and subprocess gates on the owning platform, exposes event/presentation generations for assertions, bounds every worker by timeout, and relabels existing simulated coverage honestly.

**Windows resource exhaustion.** Mitigation: focused package gates per unit, serialized final DevTools/all-feature builds, and one final workspace gate rather than competing full builds.

### Sources & Research

Repository evidence:

- `crates/ui_core/src/overlay.rs`
- `crates/ui_components/src/overlay/`
- `crates/ui_core/src/focus.rs`
- `crates/ui_components/src/focus.rs`
- `crates/ui_core/src/a11y.rs`
- `crates/ui_components/src/a11y.rs`
- `crates/gpui/src/window/a11y.rs`
- `crates/gpui/src/platform/test/`
- `crates/gpui/src/geometry.rs`
- `crates/gpui/src/element.rs`
- `crates/gpui/src/style.rs`
- `crates/gpui/src/scene.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/app.rs`
- `crates/gpui/src/app/async_context.rs`
- `crates/gpui/src/app/window_registry.rs`
- `crates/gpui/src/window/input_dispatch.rs`
- `crates/gpui/src/window/frame_journal.rs`
- `crates/gpui/src/elements/svg.rs`
- `crates/gpui/src/elements/div.rs`
- `crates/gpui/src/elements/div/accessibility.rs`
- `crates/gpui/src/elements/deferred.rs`
- `crates/gpui/src/elements/list.rs`
- `crates/form/src/form.rs`
- `crates/ui_components/src/form_adapter.rs`
- `crates/ui_components/src/theme/`
- `crates/ui_components/src/component_contract/`
- `crates/ui_components/src/tree/runtime.rs`
- `crates/ui_components/src/virtualized_list/runtime.rs`
- `crates/ui_core/src/table/`
- `crates/ui_components/src/table/`
- `crates/ui_components/src/scroll_surface.rs`
- `crates/ui_components/src/feedback.rs`
- `crates/ui_components/src/toast.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/render_tabs.rs`
- `crates/gpui_docking/src/render_floating.rs`
- `crates/gpui_docking/src/surface.rs`
- `crates/gpui_docking/src/surface/`
- `crates/gpui_docking/src/viewport_platform_sync.rs`
- `crates/gpui_docking/src/viewport_runtime_handle.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/route_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/close_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/scene_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_effects.rs`
- `crates/gpui_docking/src/viewport_window_ownership.rs`
- `crates/gpui/src/platform.rs`
- `crates/gpui_windows/src/events.rs`
- `crates/gpui_windows/src/window.rs`
- `crates/gpui_windows/src/platform.rs`
- `.github/workflows/verify.yml`

Durable decisions and verification:

- `docs/knowledge/engineering/decisions/open-gpui-ui-foundation-first.md`
- `docs/knowledge/engineering/decisions/open-gpui-ui-productization-roadmap.md`
- `docs/knowledge/engineering/decisions/open-gpui-native-ui-framework-distribution-strategy.md`
- `docs/knowledge/engineering/verification/2026-07-02-ui-framework-deep-modules.md`
- `docs/knowledge/engineering/verification/menu-runtime-focus-regression-20260620.md`
- `docs/knowledge/engineering/verification/docking-runtime-capability-alignment-20260701.md`
- `docs/knowledge/engineering/verification/open-gpui-devtools-form-resource-ecosystem-20260708.md`
- `docs/plans/2026-07-04-002-refactor-ui-framework-non-overlay-depth-plan.md`
- `docs/plans/2026-07-05-001-refactor-ui-framework-layer-motion-conformance-plan.md`
- `docs/plans/2026-06-08-001-feat-docking-plan.md`
- `docs/plans/2026-06-08-009-feat-docking-user-api-multiviewport-roadmap-plan.md`
- `docs/plans/2026-06-09-016-feat-imgui-like-multiviewport-docking-plan.md`
- `docs/research/2026-07-18-ui-interface-follow-on-research.md`
- `docs/verification.md`

Reference implementations:

- `repo-ref/gpui-component/crates/ui/src/root.rs`
- `repo-ref/gpui-component/crates/ui/src/`
- `repo-ref/fret/`
- `repo-ref/tanstack-table/packages/table-core/src/core/row-models/coreRowModelsFeature.utils.ts`
- `repo-ref/tanstack-table/packages/table-core/src/core/rows/coreRowsFeature.utils.ts`
- `repo-ref/tanstack-table/packages/table-core/src/features/row-pinning/rowPinningFeature.utils.ts`
- `repo-ref/egui/crates/emath/src/ts_transform.rs`
- `repo-ref/egui/crates/epaint/src/shape_transform.rs`
- `repo-ref/egui/crates/egui/src/hit_test.rs`
- `repo-ref/egui/crates/egui/src/containers/scene.rs`
- `repo-ref/accesskit/common/src/geometry.rs`
- `repo-ref/accesskit/common/src/lib.rs`
- `repo-ref/fret/crates/fret-core/src/semantics.rs`
- `repo-ref/imgui/imgui.cpp`
- `repo-ref/imgui/imgui_internal.h`
- `repo-ref/imgui/backends/imgui_impl_win32.cpp`

The references inform behavior and ownership only. Their package layouts, APIs, and runtimes are not copied wholesale. The repository has no `docs/solutions/` corpus to inherit for App borrow reentrancy, cross-HWND capture routing, or Dock window-session teardown; the code, native reproduction evidence, prior plans, and exact ImGui clone are the grounding sources for U24-U28.

## Implementation Units

| Unit | Outcome | Primary paths | Depends on |
| --- | --- | --- | --- |
| U1 | Form lifecycle authority | `crates/form/`, `crates/ui_components/src/form_adapter.rs` | none |
| U2 | Final AccessKit test harness | `crates/gpui/src/window/a11y.rs`, `crates/gpui/src/platform/test/` | none |
| U3 | Nested focus scopes | `crates/ui_core/src/focus.rs`, `crates/ui_components/src/focus.rs` | none |
| U4 | Window overlay runtime | `crates/ui_components/src/overlay/` | U3 |
| U5 | Semantic accessibility projection | `crates/ui_core/src/a11y.rs`, `crates/ui_components/src/a11y.rs` | U2, U4 |
| U6 | Semantic activation | `crates/ui_components/src/activation.rs`, official controls | U4, U5 |
| U7 | Scoped theme resolution | `crates/ui_components/src/theme/`, `crates/gpui/src/window.rs` | none |
| U8 | Complete Theme v1 | `crates/ui_core/src/theme.rs`, `crates/ui_components/src/theme/` | U7 |
| U9 | Collection typeahead | `crates/ui_components/src/collection_typeahead.rs` | none |
| U10 | Federated conformance cleanup | `crates/ui_components/src/component_contract/`, `xtask/` | U1, U5, U6, U8, U9 |
| U11 | Prior-surface audit | Gallery, DevTools, ADR, migration, and release docs | U10 |
| U12 | Interactive subtree transform | `crates/gpui/`, renderer crates, `crates/motion/` | U11 |
| U13 | Visible/inert/hidden presentation | `crates/gpui/src/window.rs`, `crates/gpui/src/element.rs` | U12 |
| U14 | Committed live regions and announcements | accessibility projections in `ui_core`, `ui_components`, and `gpui` | U13 |
| U15 | Typed committed portal anchors | GPUI geometry/frame journal and UI Components overlays | U14 |
| U16 | Nested bring-into-view authority | GPUI scroll/focus/a11y and virtual collection adapters | U15 |
| U17 | Rounded-rectangle subtree clip | GPUI scene/window plus WGPU, DirectX, and Metal renderers | U16 |
| U18 | Dock visual style authority | `crates/gpui_docking/src/render*.rs`, Dock examples | U17 |
| U19 | DockSurface owner and activation intent | `crates/gpui_docking/src/surface.rs`, controller/runtime events | U18 |
| U20 | Platform-window mutation authority | `crates/gpui/src/platform.rs`, native backends, Dock viewport sync | U19 |
| U24 | Reentrancy-safe platform event delivery | `crates/gpui/src/app/async_context.rs`, `crates/gpui/src/window.rs`, Dock runtime effects | U20 |
| U25 | Window appearance, activation, and ownership | GPUI window options/params and native backends | U24 |
| U26 | DockSurface window sessions and teardown | Dock surface owner, viewport ownership, close/effect orchestration | U19, U24, U25 |
| U27 | Native cross-window drag and live tear-off | GPUI active drag plus Dock route/provisional runtime | U26 |
| U28 | Owning-platform docking evidence | Windows native integration, Dock examples, CI, verification docs | U27 |

### U1. Repair Form Validation And Submission Authority

**Outcome**

`FormStore` derives effective form status and submission eligibility from validation activity plus submission phase. Old validation work cannot mutate a newer value, reset form, or newer generation. UI, accessibility, gallery, and DevTools observe the same lifecycle.

**Primary files**

- `crates/form/src/form.rs`
- `crates/form/src/field.rs`
- `crates/form/src/validation.rs`
- `crates/form/src/snapshot.rs`
- `crates/form/tests/form_lifecycle.rs`
- `crates/ui_components/src/form_adapter.rs`
- `crates/ui_components/src/form_control.rs`
- `crates/ui_components/tests/form.rs`
- `crates/ui_components/tests/form_adapter.rs`
- `crates/devtools/src/form.rs`
- `crates/devtools/tests/form_resource_adapters.rs`
- `examples/ui-foundation-gallery/src/pages/components/runtime/form.rs`
- `docs/ui/migration-v0.3.md`

**Behavioral work**

- Track field value revision as part of validation ticket identity.
- Invalidate pending work on value change, reset, and a newer validation generation.
- Derive `Validating` while any current field validation is pending and submission is not active.
- Keep fields editable while validating, but mark field/form busy and make submit unavailable.
- Reject validation starts during submission, duplicate submit begin, invalid finish, and submit while validating or invalid with typed outcomes.
- Treat edits as a new form revision: they invalidate stale validation and submit tickets, clear terminal submit outcome, and derive the next effective status from remaining current validation activity.
- Enforce DevTools data minimization before capture construction. Adapters may emit structured state, counts, roles, actions, relations, and opaque stable IDs; form values and free-form validation errors become typed redacted/summary markers rather than caller-policy-dependent strings.
- Update the real Gallery form flow, DevTools projection, and form lifecycle migration notes in this unit.

**Normative lifecycle table**

| Event | Allowed source | Result | Error/result retention | UI and projection contract |
| --- | --- | --- | --- | --- |
| edit/value change | any state | bump form/field revision; cancel affected validation; cancel an active submit as stale; derive `Validating` if other current validations remain, else `Idle` | clear terminal submit result; retain unrelated current field errors | field remains editable except while the UI intentionally disables during active submit; DevTools emits no raw value |
| begin validation | `Idle`, `Validating`, `Submitted`, `SubmitFailed` | clear terminal submit outcome and derive `Validating` | retain errors until the current result replaces them | expose busy on affected field/form; submit unavailable |
| complete validation | current ticket only | remain `Validating` while any current ticket exists, otherwise `Idle` | replace only the ticket's field errors | stale/cancelled completion has no UI, a11y, callback, or DevTools side effect |
| begin submit/retry | valid state with no pending validation and no active submit | allocate submit ticket and enter `Submitting` | clear prior submit result; retain field errors | form controls follow submitting policy and submit is unavailable |
| submit success | active ticket only | `Submitted` | retain success summary without sensitive payload | status is observable consistently; a later edit returns to derived non-terminal state |
| submit failure | active ticket only | `SubmitFailed` | retain typed/redacted form-level failure until retry, edit, or reset | retry is available when fields remain valid; no raw server/user text enters DevTools |
| reset | any state | cancel all tickets and enter `Idle` | clear field/form errors, dirty state, and terminal outcome | all projections update in one revision |

**Test scenarios**

- One and multiple concurrent validations enter and leave `Validating` only when the last current ticket completes.
- Old-value, post-reset, and older-generation completions are typed stale/cancelled no-ops.
- Validation errors survive the transition back to idle without corrupting status.
- Submit is blocked while validating/invalid/submitting; counters and callbacks do not advance on rejection.
- Submit success, failure, retry, edit-after-terminal, reset, and stale submit completion follow the normative table.
- UI projection exposes validating/busy separately from disabled/submitting and gallery reaches the state through real async lifecycle.
- DevTools root and field activity match the store snapshot without leaking field values or free-form validation/submit errors through capture, history, diff, export, artifact, report, or Gallery fixtures.

**Deletion/replacement**

- Remove the unreachable stored-status path that treats `FormStatus::Validating` as caller-assigned state.
- Remove projection logic that derives disablement only from `Submitting` and drops validation activity.

**Unit gate**

- Focused nextest passes for form, UI form tests, DevTools form/resource features, and gallery form scenarios.

### U2. Make Final AccessKit Updates Test-Observable

**Outcome**

GPUI tests can activate accessibility, inspect a normalized final `TreeUpdate`, deactivate accessibility, and dispatch actions to real node IDs. This is test/diagnostic infrastructure, not a second accessibility renderer.

**Primary files**

- `crates/gpui/src/platform/test/platform.rs`
- `crates/gpui/src/platform/test/window.rs`
- `crates/gpui/src/app/test_context.rs`
- `crates/gpui/src/window/a11y.rs`
- GPUI accessibility tests near the owning modules

**Behavioral work**

- Retain accessibility callbacks and ordered tree updates in the test window.
- Expose test-context operations for activation, latest normalized tree, update history where needed, deactivation, and action requests.
- Keep inaccessible windows inert.
- Normalize updates sufficiently for deterministic assertions without discarding node identity or relations.
- Remove the obsolete warning that reports no accessible UI when a real tree exists.

**Test scenarios**

- Activation produces root plus rendered nodes; deactivation stops updates.
- Equivalent rerender preserves logical node IDs while state changes.
- Unmount removes stale nodes and no child/control/label relation dangles.
- Focus references a node in the tree.
- An AccessKit action reaches the intended handler and a subsequent frame reflects its result.
- Two test windows retain isolated trees and action routing.

**Deletion/replacement**

- Remove tests that can pass solely by rebuilding expected metadata without inspecting the final tree where final-tree behavior is the claim.

**Unit gate**

- GPUI accessibility-focused nextest passes on `TestPlatform` without a native accessibility bridge.

### U3. Introduce Nested Focus Scope Runtime

**Outcome**

Focus scope policy is renderer-neutral; GPUI owns live handles and traversal. Nested modal scopes loop Tab/Shift-Tab, resolve declared targets to real descendants, ignore stale targets, and restore deterministically.

**Primary files**

- `crates/ui_core/src/focus.rs`
- `crates/ui_components/src/focus.rs`
- new focused runtime module under `crates/ui_components/src/overlay/` or `primitives/`
- `crates/gpui/src/tab_stop.rs` only where the existing traversal API cannot express a scoped loop
- `crates/ui_components/tests/focus_scope.rs`
- `docs/knowledge/engineering/decisions/` for the joint Focus Scope/Window Overlay Runtime ADR

**Behavioral work**

- Model scope identity, nesting, initial intent, live target ordering, restore target, and fallback.
- Resolve explicit target, first focusable, target-or-first, and surface fallback against registered descendants.
- Keep non-modal focus behavior unchanged.
- Ensure only the innermost active modal scope traps traversal.
- Arbitration uses stable logical targets and ignores disabled, hidden, unmounted, or stale registrations.
- Resolve restoration in this order: a newer focus claim; the live saved target; the nearest active ancestor scope's last live target; an explicitly registered window application fallback. If none exists, do not focus arbitrary content or synthesize activation; preserve a still-live current focus or safely leave the window without an element focus.
- Create the joint Focus Scope/Window Overlay Runtime ADR with the preparatory ownership and completion-gate decision; U4 finalizes it against production migration evidence.

**Test scenarios**

- Empty, one-target, and multi-target scopes loop in both directions.
- Missing explicit target follows declared fallback rather than always focusing the surface.
- Nested child close restores within the parent; parent close restores outside.
- Rerender/unmount and a missing trigger do not panic or steal focus.
- Two windows have isolated scope registries.
- Real key events prove that focus cannot escape the active modal underlay.

**Deletion/replacement**

- Remove component-specific focus target bookkeeping once its component migrates.
- Remove target-intent branches that return `None` without consulting the live registry.

**Unit gate**

- ui_core policy tests, GPUI traversal tests, and `TestAppContext` focus-scope integration tests pass as a preparatory gate.
- U3 is not declared a production authority independently. Its completion is shared with U4 and requires official overlay migration plus deletion of component-owned focus bookkeeping.

### U4. Replace Per-Component Overlay Tails With A Window Runtime

**Outcome**

All official overlays register with one window-owned stack. It is the sole authority for topmost Escape/outside press, parent-child inside regions, modal blocking, controlled close intent, closing presence, callback ordering, focus claims, and restoration.

**Primary files**

- `crates/ui_core/src/overlay.rs`
- `crates/ui_components/src/overlay.rs`
- `crates/ui_components/src/overlay/runtime.rs`
- `crates/ui_components/src/overlay/adapter.rs`
- `crates/ui_components/src/overlay/host.rs`
- overlay components including Dialog, AlertDialog, Sheet, Popover, HoverCard, Tooltip, Menu, ContextMenu, Select, Combobox, and Command
- `crates/ui_components/tests/window_overlay_runtime.rs`
- `crates/ui_components/tests/overlay.rs`
- `crates/ui_components/tests/choice.rs`
- `docs/ui/migration-v0.3.md`
- `docs/knowledge/engineering/decisions/` for finalizing the joint Focus Scope/Window Overlay Runtime ADR

**Behavioral work**

- Register stable layer ID, parent ID, modality, dismiss policy, inside regions, presence, focus scope, and controlled-state callbacks.
- Consume the existing pure stack resolvers in production.
- Offer Escape/outside press once to the topmost eligible layer; an explicit Ignore stops cascade.
- Treat child surfaces as inside every ancestor layer.
- Separate close intent, owner commit, and exit-animation presence according to the lifecycle table below.
- Restore after the owner commits semantic close, not after a mere controlled request and not after exit paint finishes. Exit presence may keep its pointer barrier but cannot retain keyboard, focus-scope, or accessibility authority.
- Resolve competing focus claims at end of turn so newly opened overlays win.
- Update each migrated family's Gallery scenario, DevTools/runtime inspection where applicable, and overlay migration notes in the same fleet slice.
- Finalize the joint ADR with the window ownership mechanism, lifecycle matrix, pilot result, and deletion evidence.

**Normative lifecycle table**

| State | Paint | Surface hit/actions | Escape/outside | Accessibility | Modal pointer barrier | Focus trap/restore |
| --- | --- | --- | --- | --- | --- | --- |
| open | yes | enabled | topmost policy | layer present; modal underlay non-navigable | active for modal | top modal traps; no restore |
| close requested, controlled owner still open | yes | enabled | still topmost; duplicate intent suppressed | unchanged from open | unchanged from open | unchanged from open; callback emits intent only |
| closing after owner commits closed | exit paint only | disabled | ineligible | layer removed/inert; underlay restored | retained until presence unmount to prevent click-through | trap removed; end-of-turn restore claim runs once |
| reopened during exit | yes, same logical identity | re-enabled | eligible again | layer restored with stable identity | active according to modality | cancel pending restore; newest initial-focus claim wins |
| unmounted | no | no | absent | nodes removed | absent | no pending claim or registration |

**Overlay family migration matrix**

| Family | Trigger/ownership | Modality | Dismiss policy | Initial focus and Tab | Restore/presence |
| --- | --- | --- | --- | --- | --- |
| Dialog, Sheet | programmatic/trigger; controlled or uncontrolled | modal | Escape and outside follow explicit policy; default consumes and requests close | explicit target, then first focusable, then surface fallback; trap both directions | restore live trigger/ancestor fallback; exit presence uses lifecycle table |
| AlertDialog | programmatic/trigger; controlled or uncontrolled | strict modal | Escape only when explicitly allowed; outside consumes without close by default | least-destructive/explicit target, then first focusable; trap | restore as modal; exit presence uses lifecycle table |
| Popover | click/programmatic; controlled or uncontrolled | non-modal | Escape/outside request close; child overlays count as inside | preserve trigger unless explicit autofocus; no trap | restore only when focus moved into surface; exit presence noninteractive |
| Tooltip, HoverCard | hover/focus/pointer dwell; controlled delay state | passive non-modal | Escape may dismiss active surface; outside press is not an ownership event | never claim or trap focus | no focus restore; delayed open/close and exit identity remain component policy |
| Menu, ContextMenu | trigger/right-click/keyboard; controlled or runtime-owned | active non-modal | Escape/outside close top branch; submenu is inside ancestors | first/selected item; roving focus within branch; Tab closes rather than traps | child restores parent item, root restores trigger/source; exit noninteractive |
| Select | trigger; controlled value/open intent | active non-modal | Escape/outside request close | selected option then first enabled option; no modal trap | restore trigger when listbox owned focus; exit noninteractive |
| Combobox, Command overlay mode | text input/programmatic; controlled query/open intent | non-modal unless wrapped by a modal component | Escape/outside request close according to wrapper | keep editor focus with active-descendant semantics; no independent trap | preserve/restore editor; inline mode does not register an overlay |

Each row receives a characterization test before migration. Any intentional deviation from current behavior is recorded in the unit's migration notes rather than hidden in shared runtime defaults.

**Test scenarios**

- Popover -> Menu -> Dialog nested ordering for Escape and outside press.
- Top Ignore, modal Consume, and explicit pass-through policies behave once and do not leak to underlay.
- Controlled close refusal keeps registration, modality, and focus.
- Uncontrolled close callback sees committed framework state. Controlled callback observes the owner's current committed state and only emits intent; cleanup and restore wait for the owner's later close commit.
- Child, parent, trigger-unmounted, exit/reopen, and window-close restoration paths are deterministic.
- Duplicate layer IDs fail clearly in debug/tests.
- Two windows never share layers, IDs, events, or restore claims.

**Deletion/replacement**

- Delete the shallow `OverlayLayerHost` forwarding facade after callers use the real runtime.
- Delete scattered close helpers and per-component Escape/outside/barrier/focus-restore tails.
- Preserve the placement solver, live measurement, `anchored`, and `deferred` mechanisms.

**Unit gate**

- Pure overlay policy, real GPUI input, choice overlay, and gallery overlay smoke tests pass.
- U4A first migrates Dialog, Popover, and Menu. It must pass controlled/reentrant/focus tests without family-specific runtime branches and delete those three families' old tails before U4B migrates the remaining fleet.
- Every migrated family has exactly one authority throughout the pilot/fleet sequence; no adapter is left half-migrated.
- U3/U4 do not complete if nested modal/menu topmost dismiss, focus trap, LIFO restore, and old-bookkeeping deletion are not proven through `TestAppContext`.

### U5. Converge Accessibility On Semantic Descriptors And Final Trees

**Outcome**

Every official component that emits accessibility semantics derives one ephemeral semantic projection from its existing resolved state. GPUI element attributes, the final AccessKit tree, AccessKit actions, and redacted DevTools summaries consume that projection. The projection cannot become independently stored component state, and manual evidence is no longer a runtime authority.

**Primary files**

- `crates/ui_core/src/a11y.rs`
- `crates/ui_components/src/a11y.rs`
- `crates/ui_core/src/table/`
- `crates/ui_components/src/table/`
- `crates/ui_components/tests/table/`
- official action, form, choice, overlay, navigation, collection, and table component modules
- `crates/gpui/src/window/a11y.rs`
- `crates/devtools/src/ui_components.rs`
- `crates/ui_components/tests/a11y.rs`
- `crates/ui_components/tests/public_surface/adapter.rs`
- `crates/ui_components/src/component_contract/`
- `crates/ui_components/src/public_api/`
- `crates/ui_components/tests/public_surface/`
- `xtask/src/ui_contract.rs` and UI-contract fixtures
- gallery component conformance/catalog modules and tests
- `crates/devtools/tests/framework_adapters.rs`
- `docs/ui/component-contract.md`
- `docs/ui/migration-v0.3.md`
- `docs/knowledge/engineering/decisions/` for the semantic accessibility/final-tree ADR

**Behavioral work**

- Inventory every official component that currently emits accessibility semantics and track its migration/deletion status in this unit.
- Derive required, invalid, busy, values, relations, actions, collection position/count, and modal/hidden facts from existing resolved state rather than duplicating those fields in a stored descriptor.
- Pilot the design on Button and one multi-node family from Tabs/Table. The projection must delete more family-local assembly/evidence than it adds. If it fails that deletion test, resolved state remains the semantic authority and only a shared projection helper is introduced.
- After the pilot gate, migrate every inventoried official producer and centralize GPUI/AccessKit projection; eliminate family-local hand assembly where the projection owns the fact.
- Execute the fleet migration in bounded family checkpoints: action/form controls, text/form fields, choice/navigation, overlay/modal, collections, and structural/display. Each checkpoint runs focused final-tree/action gates and deletes that family's old assembly/evidence before the next checkpoint; the number of static evidence rows is not a producer inventory or completion metric.
- Correct semantic downgrade such as Separator mapping to Group.
- Keep stable node identity across equivalent rerenders and remove nodes/relations on unmount or virtualization recycle.
- For Table, make the typed logical table/row/column/header identity algebra the only identity-sensitive boundary for expansion, default focus, editing, pinning, snapshots, debug selectors, render keys, and semantic nodes. Do not implicitly coerce a business row ID or string into an exact source identity.
- Preserve exact source-instance and typed group identities through `core -> filtered -> grouped -> sorted -> expanded -> paginated -> final`. Duplicate business IDs require an explicit unique/occurrence/instance lookup result, and ambiguous business-ID editing must fail without changing component state or cache identity.
- Keep logical Table focus against the complete final model while the rendered virtual window owns only row-mounted physical focus handles. When the logical row leaves overscan, bind the same focus claim to a stable Table-root focus proxy so real keyboard navigation remains actionable without publishing or impersonating a stale row node. The proxy may carry Table-level AccessKit focus but advertises no missing-row actions. Rebind to the exact row only while that proxy still owns the same claim; if the user moved focus elsewhere, remounting the row must not steal it back. If the exact identity leaves the complete final model, fall back to the first remaining row in final-model order, or clear logical focus when the model is empty. Physical focus and `TreeUpdate.focus` migrate or clear only while the Table/proxy still owns the claim.
- Encode each exact Table node identity once in a collision-free key and derive source-row diagnostic labels from the identity. Avoid per-cell nested identity allocation and per-stage cloning of redundant source labels; only synthetic/group diagnostics retain shared label storage when derivation is unavailable.
- Define partial column order as ordering the listed visible columns first and appending unlisted visible columns in source order. At the mutation boundary, complete a partial order with every source column in source order before applying a moved/target operation, then emit the normalized full order. Column order never owns visibility or pinning.
- While a modal is active, remove its underlay from the navigable accessibility surface, reject underlay focus/value/activation actions, keep accessibility focus within the modal, and restore the prior tree when the modal is semantically closed.
- Project DevTools from the resolved semantic authority, not `COMPONENT_A11Y_EVIDENCE`. Its adapter accepts only allowlisted structural facts and opaque IDs; accessible name, description, value text, labels, user input, and clipboard-derived text become typed redacted/summary markers before capture construction.
- Update each family's Gallery/a11y scenario and migration notes in the same migration slice.
- Create the semantic accessibility/final-tree ADR, including the resolved-state projection rule and final `TreeUpdate` evidence boundary.

**Test scenarios**

- Button, Checkbox, form field, Dialog, Tabs, Slider, Table, and VirtualizedList final nodes match resolved semantics.
- Checked, expanded, invalid, busy, disabled, values, relations, row/column metadata, and available actions update on the same logical node.
- AccessKit Focus/Click/value actions target the correct node and are no-ops when disabled or under a modal.
- Modal open/close TreeUpdates prove the underlay is non-navigable while active and restored afterward, not merely action-blocked.
- Virtualized identities cannot be accidentally reused for a different stable item.
- An explicit duplicate source instance retains the same exact identity, row/cell NodeIds, virtualizer measurement, and action target across filter/sort/paginate stages, reorder, top/bottom pinning, virtual recycle, and return.
- Occurrence identities carry a source-snapshot-local discriminator. Replacing or reordering the source snapshot invalidates retained occurrence-backed focus, pin, edit, and measurement state instead of silently retargeting it; cross-snapshot retention uses an explicit instance identity.
- A business-ID-only edit against duplicate rows returns `AmbiguousRowId` and leaves data, edit state, and cache identity unchanged; exact source identities update only their intended instance.
- Scrolling a logically focused row outside overscan transfers its claim to the stable root proxy; real Up/Down/Home/End and Enter/Space continue to navigate and activate exact logical identities through that proxy. The unmounted row's stale AccessKit node is absent and rejects actions; AccessKit Focus/Click resumes only after reveal/remount publishes the exact row node. Returning the row rebinds only if the proxy still owns that claim. Removing the row from the complete final model selects the first remaining row in final-model order, or clears logical focus when the model is empty, without disturbing focus already moved outside the Table.
- Typed group identities keep Empty, Text, Number, and Bool distinct even when display text matches. Every NaN payload normalizes to one stable Number identity, and `+0.0`/`-0.0` normalize to one Number identity; group counts and codecs prove both rules. Duplicate exact identities cannot collide in node or measurement keys.
- Identity-sensitive public APIs reject raw strings at compile time, while migration examples show explicit unique, occurrence, instance, and bulk business-ID targeting.
- A partial column order preserves every otherwise visible unlisted column in source order; an unlisted column remains reorderable as either the moved or target column under visibility and pinning projections, and the resulting callback carries a normalized full source-column order.
- Unmount and relation repair produce no dangling references.
- DevTools and final tree agree on allowlisted public semantic facts, while unique canaries in accessible free text never reach capture/history/diff/export/artifact/report fixtures.

**Deletion/replacement**

- Delete every inventoried component's duplicated aria assembly and all semantic claims, consumers, and authority uses of `COMPONENT_A11Y_EVIDENCE` as it migrates. U5 owns this deletion; U10 may remove only residual empty types, exports, and conformance scaffolding.
- Delete fallback mappings that silently change role semantics.
- Delete implicit business-ID/string conversions into exact Table row identity and convenience edit paths that hide ambiguity.
- Preserve the neutral vocabulary unless a concrete type has no domain value; do not force `ui_core` to depend on GPUI.

**Unit gate**

- GPUI accessibility tests, UI final-tree tests, public-surface tests, DevTools adapter tests, and gallery Focus/A11y tests pass.
- Table gates include a compile-time signature guard, exact-identity stage/lifecycle tests, virtual-focus restoration, ambiguous-edit non-mutation, duplicate NodeId/measurement checks, and component-contract/migration documentation for typed identity and partial column order.
- `scan-ui-contract`, public-surface tests, and Gallery catalog/conformance tests pass with no semantic `COMPONENT_A11Y_EVIDENCE` claim or consumer; only an empty type/export/scaffold explicitly assigned to U10 may remain.
- The unit cannot claim completion if `TreeUpdate` is not directly observed.
- U5 cannot complete while any inventoried official component retains a parallel semantic assembly/evidence authority. Representative action, form, choice, overlay, navigation, collection, and table families require deep final-tree/action tests; the remaining producers require unified projection coverage and a structured absence check for old authority.

### U6. Break Public Click Callbacks Into Semantic Activation

**Outcome**

Official controls expose semantic activation/value intent rather than physical pointer events. Pointer, keyboard, accessibility, and programmatic entry paths share one disabled gate, one state transaction, one callback, and role-specific key policy.

**Primary files**

- new private activation primitive under `crates/ui_components/src/`
- Button, IconButton, Link, Switch, Toggle, Checkbox, Radio, Tabs, Accordion, Tag, Breadcrumb, Toast, choice rows, and Table row activation
- component contract/public API files
- gallery/examples and downstream workspace call sites
- `crates/ui_components/tests/semantic_activation.rs`
- `docs/ui/migration-v0.3.md`
- `docs/knowledge/engineering/decisions/` for the semantic activation ADR

**Behavioral work**

- Define typed activation source and the minimal domain payload callers need.
- Inventory every public callback signature that exposes `ClickEvent` and classify it as semantic intent/value change or a proven raw pointer escape hatch. The inventory is a migration checklist, not a permanent parallel API authority.
- Apply the normative role matrix below instead of inheriting GPUI's generic Enter/Space click behavior.
- Ensure controlled controls emit intent without changing caller-owned state.
- Route AccessKit action directly to semantic activation for official components rather than relying on coordinate-synthesized click fallback.
- Retain an explicitly named raw-click path only where the consumer census proves modifier or position semantics are required.
- Update Gallery/examples, component contracts, and callback migration notes in the same family slice that deletes the old callback.
- Create the semantic activation ADR and supersede ADR 0005's proposed callback shape where the new matrix intentionally breaks it.

**Normative activation matrix**

| Semantic role/family | Keyboard policy | Timing/repeat/default | Focus/propagation |
| --- | --- | --- | --- |
| Button, IconButton, button-like Tag/Toast action | Enter and Space | activate on unmodified key-up to preserve current GPUI timing; ignore auto-repeat; prevent Space scrolling from key-down through key-up | keep focus unless activation closes its owning surface; stop only the consumed activation path |
| Link and link-like Breadcrumb | Enter only | activate on unmodified key-up; Space is not consumed; ignore repeat | preserve normal focus; raw pointer modifiers/position use the explicit raw path when required |
| Checkbox, Switch, Toggle, Radio | Space only | activate/change on unmodified key-up; prevent Space scrolling; ignore repeat | emit one value intent; read-only/disabled paths neither consume nor change |
| Tabs and Accordion triggers | Enter and Space | activate on unmodified key-up; Arrow/Home/End navigation remains in the roving-focus owner | focus remains on trigger; automatic tab selection, if configured, remains a navigation policy rather than duplicate activation |
| Menu/Listbox/choice rows | Enter; Space only where the owning model defines selection/toggle | key-up, no repeat; editable search input never enters this path | structural/disabled rows are skipped; activation may close through overlay policy |
| Table/tree/collection rows | Enter by default; Space only for an explicit selection/toggle contract | key-up, no repeat; nested editor/action origin suppresses row activation | reveal/focus/selection remain separate model transactions |
| AccessKit/programmatic | semantic action, no synthetic key or coordinates | immediate transaction with typed source; exactly once | same disabled/read-only/nested ownership gates as keyboard/pointer |

All keyboard paths reject modified keystrokes unless a component explicitly documents a modifier contract. Pointer capture and nested-interactive suppression are decided before semantic activation so one physical gesture cannot reach both child and parent callbacks.

**Test scenarios**

- Pointer, allowed key, AccessKit Click, and programmatic activation produce equivalent payloads exactly once.
- Disallowed keys, disabled/read-only/structural targets, and a controlled owner that does not commit state have no hidden state change.
- Uncontrolled state transition precedes callback observation; controlled callbacks observe current owner state, emit one intent, and wait for the owner's later commit before projections change.
- Nested editor/cell actions do not bubble into a parent row activation.
- Button, Link, Checkbox/Toggle, choice row, and Table row provide representative end-to-end coverage.

**Deletion/replacement**

- Delete old public `on_click` callback paths for semantic controls with no compatibility alias.
- Delete ClickEvent-based contract inventory entries and gallery call-site workarounds.
- Preserve `ActionDescriptor` and command execution ownership; activation may consume presentation facts but does not replace them.

**Unit gate**

- Semantic activation, primitives, navigation, choice, table interaction, a11y action, and gallery tests pass.
- Real key-event tests cover every distinct row in the activation matrix, including Space default prevention, key-up timing, repeat rejection, and nested-interactive suppression.
- Every inventoried semantic callback is migrated. A structured public-surface absence gate rejects remaining public `ClickEvent` parameters except explicitly named raw APIs with documented consumers.

### U7. Prove Scoped Theme Resolution Before Generalizing Context

**Outcome**

Using the existing immutable color `ThemeSnapshot`, Open GPUI gains app fallback, window-local selection/override, explicit subtree override, and deferred overlay inheritance. A prototype gate decides whether this remains a theme-specific UI mechanism or earns a narrow generic GPUI inherited-context primitive.

**Primary files**

- `crates/ui_components/src/theme/runtime.rs`
- `crates/ui_components/src/theme/resolver.rs`
- a theme provider/environment element under `crates/ui_components/src/theme/`
- `crates/gpui/src/window.rs` and the GPUI element/deferred frame modules only if the prototype gate proves a substrate gap
- production `ThemeResolver::current` call sites
- native GPUI tooltip attachment points owned by UI Components
- `crates/ui_components/tests/theme_scope.rs`
- gallery shell/token pages
- theme-context migration documentation
- `docs/ui/migration-v0.3.md`
- `docs/knowledge/engineering/decisions/` for the Theme Scope ADR and prototype decision

**Behavioral work**

- Prototype a theme-specific provider against the actual GPUI render timing, nesting, rerender, unwind, and deferred-element lifecycle.
- Prefer the theme-specific path unless it cannot preserve nearest-provider semantics through normal and deferred rendering.
- A public generic GPUI context primitive additionally requires one current independent non-theme consumer with the same immutable stack behavior. U3 focus-scope association is a candidate only if its implementation cannot use existing focus hierarchy/window registration; direct and deferred ThemeContext reads are one consumer, not two.
- Resolve precedence as `subtree override > window selection/override > app selection > built-in fallback`. App selection remains the global fallback for windows without an override.
- Capture the effective opening snapshot for deferred overlays without leaking it to siblings, other windows, or later frames.
- Treat delayed native tooltip builders as a detached render boundary: capture at the trigger's hover/open generation, scope both builder execution and its returned view, and require explicit theme capture for raw GPUI tooltip attachment.
- Invalidate cached child-view journals when a stable subtree scope changes even if the child entity itself did not notify.
- Invalidate only affected windows/scopes when selection changes and drop all window-local state on close.
- Keep any generic primitive private until both proof conditions pass, immutable/clonable only, and unsuitable for arbitrary mutable services.
- Update Gallery scoped-theme behavior and theme-context migration notes in this unit.

**Test scenarios**

- Two windows select independent themes while a third inherits app selection.
- Inheriting windows observe app selection writes in the same transaction; selected and overridden windows read their immutable last-known snapshots without registry re-resolution or panic.
- Nested providers choose the nearest value; siblings and post-scope rendering recover parent context.
- Rerender, early return, and panic/unwind cannot leave scoped state imbalanced.
- Deferred children and overlay surfaces retain the opening subtree's complete color snapshot; a same-mode, same-revision palette canary proves that the full snapshot is frozen rather than reconstructed from metadata.
- Button and IconButton delayed tooltip builders plus their returned views retain the trigger scope; close and reopen recaptures the then-current scope.
- Gallery DevTools reports the window-effective theme in its initial frame when the shell is created under a window selection or override, before any manual refresh.
- Unknown IDs or failed overrides leave effective context unchanged.
- Window close clears local selection and provider state.

**Deletion/replacement**

- Delete `ThemeRuntime: Global` as the sole active-ID authority and replace the app-only resolver seam with explicit app fallback plus optional window/subtree context.
- Retain the app-global definition registry and built-in fallback.
- Do not add a public generic context API if the proof gate yields only ThemeContext as a consumer; ship a theme-specific scope instead.

**Unit gate**

- Theme-scope/deferred tests and Gallery scoped-theme tests pass on the existing snapshot before the complete Theme v1 replacement begins.
- Record the prototype evidence and selected implementation in the Theme Scope ADR. Stop any generic GPUI API if it requires a hidden app-global subtree map, changes arbitrary service lookup, or lacks an independent non-theme consumer.

### U8. Replace Color-Only Theme With Complete V1 Design Scales

**Outcome**

The complete Theme v1 replaces the old color-only payload and schema with an immutable design contract for stable semantic scales. Runtime effective revision changes monotonically when effective content or selection changes; source-file revision remains metadata. This is an intentional clean break under the existing `v1` version name.

**Primary files**

- `crates/ui_core/src/tokens.rs`
- `crates/ui_core/src/sizing.rs`
- `crates/ui_components/src/theme.rs`
- `crates/ui_components/src/theme/snapshot.rs`
- `crates/ui_components/src/theme/registry.rs`
- `crates/ui_components/src/theme/schema.rs`
- `crates/ui_components/src/theme/recipes/`
- `crates/ui_components/tests/theme.rs`
- `docs/schemas/open-gpui-theme-v1.schema.json` replaced in place
- breaking migration documentation for workspace call sites
- existing theme xtask scanners
- `docs/ui/migration-v0.3.md`
- `docs/knowledge/engineering/decisions/` for amending the Theme Scope ADR with complete-v1 revision and clean-break decisions

**Behavioral work**

- Add typed typography, spacing, radius, elevation, density, and motion-policy scales beside color only where each public token has at least two real recipe consumers.
- Carry admitted scales as one immutable `ThemeDesignScales` value in every snapshot/context; do not introduce a second registry beside `ThemeRegistry`.
- Keep structural sizes local to component metrics and motion execution in `open-gpui-motion`.
- Treat serialized `revision` as source metadata. Allocate monotonic runtime effective revisions for changed registration, replacement, app/window/subtree selection, and overrides; callers cannot supply effective revisions, and identical effective or metadata-only reloads do not bump.
- Resolve component size as `explicit Size > theme density default`; adaptive density remains a host recommendation. Merge motion using the strictest preference so reduced motion cannot be relaxed by a component override.
- Parse invalid or unknown content atomically with structured diagnostics and no active-state mutation.
- Delete the old color-only definition/loader/schema shape and replace it directly; old serialized input is unsupported and no compatibility loader remains.
- Migrate U7's app/window/subtree/deferred channel to the complete v1 payload without changing scope precedence.
- Update Gallery token examples, DevTools theme projection, schema docs, and migration notes in this unit.
- Amend the U7 Theme Scope ADR with the complete v1 payload, effective revision authority, and clean-break decision.

**Test scenarios**

- Built-in themes are complete and schema round-trip.
- Old color-only fixtures fail against the replacement schema/loader, while new complete v1 fixtures round-trip; no fallback silently accepts the deleted shape.
- Invalid types, missing required facts, duplicate/unknown tokens, and failed replacement leave registry/selection unchanged.
- Same source revision with changed content bumps effective revision; identical effective content does not.
- Metadata-only reloads preserve effective revision; selecting a different id with identical payload still bumps because authority selection changed. Repeated selection and override no-ops do not bump.
- Compact density and reduced-motion policy reach at least two representative recipes without changing semantic output.
- Explicit component size wins over theme density. Theme reduced motion plus an explicit animated request remains reduced, and either source may request reduction.
- A non-color-only scope change invalidates a cached child; deferred overlays and delayed tooltips freeze density and motion for one opening generation and recapture only after close/reopen.
- Unselected registration, invalid active replacement, and metadata-only active replacement do not refresh unaffected windows.
- Every U7 window/subtree/deferred scope test passes unchanged with the complete v1 payload.

**Deletion/replacement**

- Delete color-only in-memory authority and production-only fallback paths superseded by the complete replacement v1.
- Delete `fallback_mode`, partial color filling, `ThemeRegistrationDiagnostics`, caller-supplied effective revisions, and the old color-only fixtures/schema without aliases or compatibility parsing.
- Remove stable cross-family magic metrics only when recipes consume the replacement token.
- Delete the old color-only schema/model, obsolete fixtures, and any compatibility parsing branch.
- Do not move motion execution out of `open-gpui-motion`.

**Unit gate**

- Theme unit/integration/scope tests and theme drift/schema scanners pass against the sole complete v1 contract.
- No token category is padded solely to satisfy the plan; absent two consumers, keep the metric local and record the category as intentionally not public.

### U9. Extract A Deterministic Collection Typeahead Session

**Outcome**

Tree and VirtualizedList share one private typeahead session for buffer lifetime and key acceptance. Other collection components adopt it only where they have the same runtime behavior; editable search remains separate.

**Primary files**

- new private module under `crates/ui_components/src/`
- `crates/ui_components/src/tree/runtime.rs`
- `crates/ui_components/src/virtualized_list/runtime.rs`
- Menu/Listbox/Select runtime only after behavior equivalence is proven
- `crates/ui_components/tests/typeahead_runtime.rs`
- `crates/ui_components/tests/layout.rs`
- `crates/ui_components/tests/choice.rs`
- `crates/ui_components/tests/overlay.rs`

**Behavioral work**

- Own printable-key filtering, buffer append/reset, timeout, repeated-character cycling signal, and instance lifecycle.
- Inject time for deterministic tests; production adapts GPUI time/events at the component boundary.
- Preserve model-specific matching, visibility, disabled/structural filtering, reveal, focus, and selection semantics in their owning model.
- Preserve stable-key identity across reorder/remove.

**Test scenarios**

- Fake-clock accumulation/reset with no sleeps.
- Repeated-character cycling, wrap, normalization, empty query, and non-printable/modifier/IME filtering.
- Disabled, separator, group, and status rows never become targets.
- Reorder/remove resolves by stable key or clears safely.
- Instances and windows never share buffers.
- Virtualized reveal does not imply selection; editable Combobox/Command query is not intercepted.

**Deletion/replacement**

- Delete Tree and VirtualizedList duplicate buffer, timestamp, timeout constant, and key parser.
- Keep the new session private until a public consumer requirement exists.

**Unit gate**

- Typeahead, layout, choice, overlay, and gallery collection tests pass deterministically.

### U10. Federate Typed Conformance And Public-Surface Authorities

**Outcome**

Narrow typed authorities own facts at their natural lifecycle: `COMPONENT_CONTRACT_ROWS` owns component product metadata, Gallery owns selectors/probes, public API modules own exports, and native tests own executable scenario IDs. Xtask cross-checks these structured sources. Source text is no longer parsed to infer Rust structure or behavior, and ADR 0014's centralized registry is not recreated.

**Primary files**

- `crates/ui_components/src/component_contract/`
- `crates/ui_components/src/public_api/`
- `crates/ui_components/src/lib.rs`
- `crates/ui_components/src/prelude.rs`
- `crates/ui_components/tests/public_surface/`
- `crates/ui_components/tests/support/public_surface/`
- `xtask/src/ui_contract.rs`
- xtask public API scanners and tests
- gallery component catalog/conformance modules
- component contract documentation
- `docs/ui/migration-v0.3.md`
- `docs/knowledge/engineering/decisions/` for the ADR 0014 amendment/reaffirmation

**Behavioral work**

- Keep `COMPONENT_CONTRACT_ROWS` small and limited to product metadata already justified by ADR 0014; remove unrelated method inventories, test execution, and Gallery implementation facts from it.
- Keep Gallery selector/render probes in Gallery and bind them one-to-one to contract IDs through a typed local adapter.
- Let native isolated tests declare scenario IDs through a structured test-side registration/artifact, without function-pointer aggregation.
- Cross-check contract IDs, Gallery probes, public owner/export facts, docs projections, and required scenario IDs in xtask while preserving their narrow owners.
- Produce repo-relative diagnostics for missing/duplicate IDs, owner drift, and projection drift.
- Derive only shared product metadata in Gallery/DevTools from contract rows; their runtime selectors, probes, and inspection data remain locally owned.
- Split common public exports from explicit extended/diagnostic modules.
- Characterize Table consumers; keep `Table`, core state/resolved state, engine, and adapter public. Move diagnostic-only behavior snapshots out of root/common prelude only when the census confirms no intended common API use.
- Calibrate Table characterization against the local TanStack reference boundary and the completed post-U5/U6 contract: preserve `core -> filtered -> grouped -> sorted -> expanded -> paginated -> final` ordering, stable typed row/column IDs across transforms, client/manual ownership for the stages Open GPUI exposes, exact source-identity selection and callbacks, controlled expansion refusal, exact row-identity pinning plus explicitly named business-ID bulk targets, caller-owned pinned-region order, pinning as a partition of logical rows/columns rather than new identities, and the Table/Virtualizer ownership split. Unsupported TanStack features, pre-U5 implicit identity behavior, atoms/plugin registries, and full API parity remain out of scope.
- Keep `TableVirtualizerSnapshot` public as a real restoration input. Move `TableBehaviorSnapshot` and `TableStateCacheKey` out of root/common exports only when the consumer census confirms their diagnostic/owner-module roles; do not delete their underlying contracts merely to shrink an export list.
- Update conformance migration notes and the relevant ADR 0014 amendment/reaffirmation in this unit.

**Test scenarios**

- Add/remove/duplicate a contract row, Gallery binding, export owner, or scenario binding and receive a precise failing diagnostic naming both narrow authorities.
- A final-tree role or activation matrix mismatch fails an executable probe; changing evidence text cannot repair it.
- Comments, aliases, grouped exports, formatting, and braces cannot affect structured checks.
- Gallery and DevTools receive the same contract ID/revision/family metadata without moving their runtime-specific facts into the component contract.
- Table filter/sort/group/expand/paginate/pin/virtualize/edit outputs, logical identities, partial-order behavior, and exposed client/manual stage ownership remain behaviorally identical to the post-U5/U6 checkpoint through export cleanup.
- Duplicate Table source row IDs remain explicitly diagnosed and stably disambiguated by source-instance identity; no duplicate may collide in virtualizer keys or final accessibility nodes.
- Exact selection state and `TableRowSelectionChange::current_selection` distinguish duplicate source instances, descendant propagation follows the exact selected parent, and a refused pointer/keyboard expansion request cannot create hidden adapter state.
- Exact pin targets distinguish duplicate source instances and typed group rows; explicit business-ID bulk targets expand in current model order and caller target order controls each pinned region. Top targets resolve first, and identities claimed by top are excluded from bottom.

**Deletion/replacement**

- Delete `COMPONENT_API_INVENTORY` method-name baselines that mirror Rust source.
- Delete only the empty `COMPONENT_A11Y_EVIDENCE` type/export and conformance-gate scaffolding left after U5; reopening semantic-claim or consumer deletion belongs to U5 rather than delayed U10 cleanup.
- Delete shallow source mapping/owner tables and source-string parsers once structured owner/export facts can be queried directly.
- Delete duplicate default/common re-export lists where one public API owner can generate or structurally validate them.
- Delete unused row-model version-label constants or `implemented_in_v0` flags that merely restate the executable stage pipeline.
- Do not recreate ADR 0014's deleted JSON registry/scaffold product.
- Preserve native nextest isolation, Table engine, neutral a11y vocabulary, and Action presentation authority.

**Unit gate**

- UI public-surface tests, xtask CLI fixture tests, structured scanners, Table characterization, and gallery catalog tests pass.
- New conformance infrastructure must delete more duplicate authority than it adds, must not collapse failures into one mega-test, and must not make component rows the owner of Gallery/test/runtime facts.

### U11. Audit Prior Gallery, DevTools, ADRs, Migration Docs, And Release Gates

**Outcome**

The product surfaces already updated by U1-U10 are audited together, architecture decisions and migration notes are cross-linked, obsolete code is absent, and the existing gates cover the newly added GPUI/accessibility paths. U11 does not close the expanded plan: U12-U20 and U24-U28 own their own Gallery/example, documentation, and release-gate changes rather than hiding them in this prior-surface audit.

**Primary files**

- `examples/ui-foundation-gallery/`
- `crates/devtools/`
- `docs/knowledge/engineering/decisions/`
- `docs/ui/`
- `docs/verification.md`
- `docs/knowledge/engineering/`
- CI and xtask verification configuration
- `.config/nextest.toml` if repository-wide timeout/test-group policy is introduced

**Behavioral work**

- Compose cross-domain Gallery smoke from the real per-unit flows already added for nested overlay/focus, final accessibility state, async form validation, scoped themes, semantic activation, and collection typeahead.
- Audit runtime inspection against an allowlist contract: structured status/count/role/action/relation and opaque IDs only. Free-form form errors, accessible names/descriptions/value text, clipboard, input, and labels must already be typed redacted/summary markers before `DevtoolsCapture` construction.
- Treat Table business/instance IDs, text group values, caller-owned table/column IDs, cell values, encoded identities, diagnostic labels, and debug selectors as sensitive source data. The DevTools adapter assigns non-reversible session-scoped opaque IDs and never persists their raw or merely formatted/hashed representation.
- Cross-link the ADRs created with U3/U4, U5, U6, U7/U8, and U10; reaffirm ADR 0014's federated ownership rather than introducing a central manifest.
- Audit the ADR 0009 reconciliation completed in U10 against the final Gallery/DevTools/release
  surface; keep the recorded TanStack reference boundary, implemented grouped/expanded/pinned
  stages, Table/Virtualizer ownership shape, and existing motion ownership accurate.
- Consolidate and release-audit the callback, theme, overlay, accessibility, and conformance migration guidance already committed with their owning units.
- Extend `xtask verify` so GPUI accessibility/focus tests and required DevTools features cannot be skipped by the main gate.

**Test scenarios**

- Gallery smoke opens nested overlays and verifies topmost dismiss/focus restoration.
- Gallery displays two theme scopes and a real validating form without manually constructing unreachable states.
- DevTools reads theme/form/overlay/focus/a11y/table authorities and preserves redaction across live capture, session frames/history, diff, Inspector detail/copy, session export, headless artifact, report, and Gallery fixture paths.
- Unique canaries injected into form values/errors, accessible name/description/value text, clipboard, user input, `TableRowId`, explicit instance ID, text group value, table/column ID, cell value, identity diagnostic, debug selector, and diagnostic label appear nowhere in those outputs; only typed redacted markers, counts, and adapter-owned session IDs remain.
- Release/doc scanners reject stale callback names, old theme authority, forwarding overlay helpers, manual evidence, and source scanners.

**Deletion/replacement**

- Treat any obsolete example, alias, doc, ADR claim, or feature flag from U1-U10 as an audit failure and reopen its owning migration unit; U11 does not perform delayed domain cleanup.
- Delete temporary characterization helpers that are not durable regression tests.

**Unit gate**

- Focused gallery, DevTools, docs, xtask, and release gates pass before U12 begins.

### U12. Add One Layout-Neutral Interactive Subtree Transform

**Outcome**

`open-gpui` owns one deep, public subtree geometry primitive for finite positive normal axis-aligned scale, finite translation in logical pixels, and an explicit post-layout origin resolved as `anchor * child_size + pixel_offset`. For a child-local point `p`, the contract is `p' = origin + scale * (p - origin) + translation`; the laid-out bounds origin then places that result in the parent coordinate space. Nested child transforms apply first and resolve privately as `parent_resolved compose child_local`. The primitive composes across nested subtrees and every observable geometry channel while measurement, Taffy layout, scroll extent, and sibling flow remain unchanged. Rotation, skew, perspective, 3D, reflection, singular transforms, and numerically unrepresentable inverse/composition results are rejected rather than approximated.

**Primary files**

- `crates/gpui/src/geometry.rs`
- `crates/gpui/src/element.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/window/input_dispatch.rs`
- `crates/gpui/src/window/frame_journal.rs`
- `crates/gpui/src/window/a11y.rs`
- `crates/gpui/src/scene.rs`
- `crates/gpui/src/elements/`, especially `div.rs`, `svg.rs`, `deferred.rs`, `canvas.rs`, and `surface.rs`
- `crates/gpui_wgpu/`, `crates/gpui_windows/`, `crates/gpui_macos/`, and `crates/gpui_linux/`
- `crates/motion/`
- `examples/ui-foundation-gallery/`
- transform API, architecture, migration, and verification documentation

**Behavioral work**

- Add the immutable opaque `SubtreeTransform`, with a checked constructor for two strictly positive finite normal scale components whose reciprocals are finite and backend-representable, finite child-local translation, and a `SubtreeTransformOrigin` made from a finite size-relative anchor plus finite pixel offset. Keep resolved composition and runtime projection private. Public reads use committed `ElementGeometry`, so declaration syntax cannot masquerade as an ancestor-resolved mapping. Provide identity and ergonomic checked scale/translation constructors without exposing a matrix or silently normalizing invalid values. Document the supported scalar/coordinate range from the shared scene/backend representation.
- Add one public element wrapper/extension that delegates layout request, measurement, and child bounds unchanged, then enters a panic-safe frame-scoped `Window` transform during prepaint and paint. Nested scopes compose deterministically; device-pixel snapping occurs after logical transform projection.
- Resolve nested transforms with checked multiply-add operations in the fixed `parent_resolved compose child_local` order. Overflow, scale underflow to zero, non-finite translation/origin, non-representable inverse, or backend conversion failure emits one structured diagnostic and fail-closes the entire affected subtree to layout-only participation before any paint, hitbox, listener, focus/IME, deferred, cache, or accessibility entry is registered. No channel may independently clamp, drop the transform, or substitute identity.
- Carry the resolved transform through every scene primitive: quads, borders, shadows, underlines, monochrome sprites, subpixel sprites, polychrome sprites, paths, text/glyph output, and native/GPU paint surfaces. Culling and existing rectangular content masks operate in the same resolved window space, and no backend may interpret an absent transform differently.
- Record both local geometry and the resolved invertible mapping on hitboxes. Raw platform events and APIs documented as window-space remain window-space; hit testing and every target-local point or vector calculation use the hitbox inverse. Add explicit local-to-window and window-to-local helpers so controls do not duplicate scale/offset arithmetic.
- Expose one immutable `ElementGeometry` snapshot through `Hitbox::geometry()` and committed `MeasuredElementSnapshot` callbacks. It reports layout bounds, displayed bounds, zero-origin local bounds, and checked local/layout/window point and vector conversions. Failed scopes publish no measurement; cache replay publishes the geometry of the current committed ancestor mapping.
- Make hover/cursor resolution, click dispatch, drag/drop, wheel routing, scrollbars, scroll offsets, autoscroll requests, selection handles, and pointer capture transform-aware. A capture retains logical target identity while each committed frame supplies the transform that matches displayed geometry; stale or singular fallback coordinates are forbidden.
- Keep scroll state and layout offsets in logical coordinates. Project descendant geometry through the transform before rectangular clipping/culling, and inverse-project pointer deltas before local scroll/drag policy. Autoscroll converts requested local bounds through each transform/scroll boundary exactly once.
- Project text-input caret and composition bounds into platform window coordinates before IME updates. Inspector overlays, debug bounds, hitbox visualization, screenshots, and diagnostics must describe displayed geometry rather than pre-transform layout geometry.
- Project final AccessKit bounds from the same resolved transform and preserve stable node identity. AccessKit Click and other semantic actions continue to enter the existing activation authority; transforms cannot create a parallel semantic node or action path.
- Capture the current transform for ordinary deferred descendants. Define a named window-space portal boundary that resets content transform only deliberately; portal anchors must use the authoritative local-to-window conversion each frame. A coordinate-space reset does not by itself bypass theme or presentation inheritance.
- Make cached-view/frame-journal replay transform-relative or invalidate it on resolved-transform changes. Replayed scene primitives, hitboxes, pointer-capture bindings, deferred entries, IME/debug geometry, and accessibility nodes must use the current displayed transform and must never reuse stale absolute geometry.
- Make cross-window and retained consumers transactional at the same boundary: prepaint may build a candidate snapshot, but only a valid paint commits it. A stable publication identity expires the previous snapshot when the current frame is invalid or absent because of unmount, prepaint rollback, or an invalid ancestor transform. Docking viewport drop scenes, presentation scenes, and interaction proofs must never publish from prepaint alone or retain last-known state after their producer disappears.
- Preserve visual-surface authority in complex consumers. Dock divider hit resolution groups root and each floating container independently, gives the last rendered floating container a blocking pointer boundary across its complete bounds, and forbids junction synthesis across surfaces. Raw splitter and composite-floating drags use the stable Dock host capture owner; standard GPUI payload drags use a stable source-element owner acquired by GPUI only after crossing the drag threshold, so `on_drag` requires a stable element ID. Terminal `PointerCancel` clears raw state, the owning payload runtime session, previews, anchors, and outside-release polling even when a replacement frame removes the host subtree; GPUI keeps its window-owned active payload visible until every host observer has run so an independent host cannot consume cancellation before the owner. Single-tabs floating drags publish Dock payload state only after floating policy accepts the transient session, and any policy or geometry rejection retracts both the pending GPUI drag and capture.
- Keep `open-gpui-motion` renderer- and GPUI-neutral. It emits a fallible `MotionProjectionTransformSample`; a consumer that depends on both crates converts the checked sample to `SubtreeTransform`. Fake-clock tests cover intermediate and exact final projections, large valid endpoint ratios, and reduced motion resolving directly to identity without changing layout or silently falling back.
- Add a Gallery scenario with nested non-uniform scaling and translation around an explicit origin, containing real text, button/semantic activation, text input/IME, clipped scrolling, drag/pointer capture, tooltip or deferred content, and inspector/accessibility probes. The scenario is an executable interaction surface, not a static transform sample.

**Test scenarios**

- Pure/property tests cover identity, relative anchor-plus-offset origin resolution, the exact private `parent_resolved compose child_local` order, committed geometry point/vector round trips, transformed bounds, child-local translation behavior, large/small positive scale within the supported range, and rejection of NaN, infinity, zero, negative scale, non-representable reciprocal, composition overflow, scale underflow, multiply-add overflow, and inverse/backend-conversion failure.
- Runtime numeric-failure tests prove a locally valid but unrepresentable nested composition preserves layout and suppresses paint, hitboxes/listeners, pointer capture, focus/IME, deferred/cache entries, diagnostics geometry, and final AccessKit nodes as one subtree transaction; identity/clamp/partial-channel fallback is forbidden.
- Transaction tests prove a valid transformed frame runs its commit exactly once, a late-invalid frame runs discard instead, rollback/unmount absence expires the prior publication exactly once, and a subsequent valid frame can republish. Docking tests prove viewport route geometry and floating/drop interaction state are retracted after early failure, late failure, and host-subtree removal.
- Layout characterization proves transformed and identity-wrapped children have identical measured size, flex/grid placement, scroll extent, and sibling positions.
- Backend-neutral scene tests assert every primitive carries the same resolved transform, clipping/culling occurs in transformed space, and scale/translation is applied exactly once. Text and surface primitives are mandatory; a quad-only demonstration is insufficient.
- Pointer tests cover transformed hit/miss edges, nested transforms, overlapping z-order, cursor/hover, click local position, wheel and scrollbar behavior, drag/drop deltas, autoscroll, transform changes during pointer capture, and non-transformed siblings. Dock tests additionally prove floating chrome occludes root dividers, an overlapping floating divider wins without a cross-surface corner, top floating content blocks a lower floating title bar, policy and inverse-geometry rejection leave no GPUI payload, Dock runtime session, or capture, and window deactivation clears captured splitter, composite-floating, and payload-floating sessions plus outside-release polling. Host-subtree removal revokes both single-tabs floating and tab-item payload drags, while a two-host window proves cancellation reaches the owning payload runtime after non-owner observers.
- IME and diagnostics tests assert transformed caret/composition rectangles, inspector/debug bounds, hitbox visualization, committed measurement, and visible-tooltip invalidation when its source moves during a transform-only frame.
- Final `TreeUpdate` tests assert transformed AccessKit bounds, stable node identity across transform-only frames, action dispatch, and stale-node cleanup after cached/deferred changes.
- Deferred tests distinguish inherited transformed content from explicit window-space portals and verify portal anchors. Cache tests change only an ancestor transform without notifying the child and prove scene, hitbox, capture, and accessibility journal replay is current.
- Motion tests use a fake clock for intermediate scale/translation, consumer conversion, exact final state including large valid ratios, and reduced-motion completion. GPUI runtime tests separately prove pointer hit alignment for the converted mapping.
- Each supported renderer backend compiles the shared primitive contract on its native CI runner and has an ABI/conversion test for every transformed primitive batch. Capable runners execute a render-pixel smoke for nested scale/translation and clip; one active-platform screenshot cannot substitute for the matrix.
- The Gallery scenario is exercised through real pointer, keyboard, AccessKit, scroll, text-input, deferred, and inspector paths at identity and non-identity transforms.

**Deletion/replacement**

- Migrate scale/translation consumers away from the SVG-only `Transformation`. Delete it if no production consumer requires SVG raster-space rotation; otherwise rename it and its method to an explicitly leaf-only `SvgPaintTransform`/`with_paint_transform` API whose documentation states that it does not affect layout, hit testing, descendants, or accessibility. No generic `Transformation` alias remains.
- Delete per-element scale/translation math, visual-only subtree flags, duplicate input inversions, and cache-specific transform state replaced by the authority. Internal backend matrices remain projections and are not re-exported as a competing public model.
- Do not add identity or clamp fallbacks, unchecked public constructors/composition, rotation/skew/3D placeholders, or a second transform stack in Motion, Gallery, Canvas, SVG, or a renderer backend.

**Unit gate**

- Focused `open-gpui` geometry, runtime/input, scene, accessibility, deferred/cache, IME/diagnostic, and public-surface tests pass with `test-support` and inspector coverage.
- Motion and Gallery integration tests pass, supported-platform renderer compile/ABI jobs are green, and at least one capable backend render smoke verifies transformed pixels and clipping.
- Review confirms layout is unchanged, numeric failure is transactional and fail-closed, every scene primitive and interactive channel consumes one resolved transform, and no public API claims unsupported affine behavior.

### U13. Converge Layout-Preserving Hidden And Inert Subtree Semantics

**Outcome**

One `open-gpui` subtree presentation authority replaces paint-only visibility, inherited accessibility hiding, and ad hoc input/focus suppression. `Visible` participates in layout, paint, input, focus/IME, and accessibility; `Inert` participates only in layout and paint; `Hidden` participates only in layout. Layout participation includes measurement, flex/grid ordering, scroll extent, and sibling placement. `Display::None` remains the explicit layout-removing choice.

**Primary files**

- `crates/gpui/src/style.rs`
- `crates/gpui/src/element.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/window/input_dispatch.rs`
- `crates/gpui/src/window/frame_journal.rs`
- `crates/gpui/src/window/a11y.rs`
- `crates/gpui/src/elements/div.rs`
- focus, text-input/IME, tooltip, deferred, portal, scroll, drag/drop, and inspector adapters under `crates/gpui/src/elements/` and `crates/gpui/src/window/`
- official component call sites, Gallery scenarios, public API tests, architecture/migration docs, and release scanners

**Behavioral work**

- Replace subtree-level `Visibility` plus independent hiding switches with one public enum such as `SubtreePresentation::{Visible, Inert, Hidden}` and one element/style entry point. Composition chooses the most suppressive ancestor state (`Hidden` over `Inert` over `Visible`); descendants cannot opt back in.
- Preserve request-layout, measurement, ordering, and scroll extent for all three states. `Hidden` skips descendant prepaint/paint work after layout whenever possible; central `Window` gates still guarantee that no raw/custom element can register a hidden channel accidentally. `Inert` paints normally through the U12 transform authority while registration of interactive and semantic channels is suppressed.
- Define input suppression completely: no hitbox eligibility, hover/active/cursor state, pointer or wheel listener dispatch, scroll interaction, drag source/drop target, pointer-capture acquisition, tooltip trigger, or input-created overlay intent may originate in inert/hidden descendants.
- Define focus/IME suppression completely: no focusable registration, Tab target, focus-scope initial/restoration target, focused text input, caret/composition update, or accessibility focus may target an inert/hidden descendant. A dynamic transition invalidates stale focus through the existing focus authority and never invents a second restoration policy.
- Distinguish final window-local focus commitment from effective active-window focus entry. Provide exact-leaf and subtree committed observers plus typed one-shot completion for exact-focus and empty-focus requests. Retained focus transactions settle only from `Committed`, `Rejected`, or `Superseded`, including while the platform window is inactive, and never infer success from request submission, next-frame timing, or later platform activation. Requests made after frame input/accessibility authority is sealed qualify in one later platform generation; focus-only demand cannot recursively redraw inside one effect cycle and is removed when a later ordinary update cancels the request.
- Define pointer-capture transitions: when a captured target becomes inert, hidden, unmounted, or otherwise absent from the committed interactive frame, dispatch one existing cancellation path and release the binding before later events. Old-frame hitboxes cannot keep it interactive.
- Define accessibility suppression at final-tree authority: inert/hidden descendants emit no AccessKit nodes, relations, focus, or actions in the next committed update, and stale actions against removed nodes are ignored. A decorative leaf may still choose not to emit semantics through the semantic projection, but there is no independent ancestor-level `a11y_hidden` presentation stack.
- Make ordinary deferred descendants and cached journal fragments inherit the resolved presentation state. A coordinate-space portal reset does not reset presentation; an independently owned window overlay becomes a new visible root only through an explicit overlay-runtime boundary and its actual mounted presence, not as an accidental consequence of deferral.
- On dynamic transitions, reconcile hover/cursor, tooltip, pressed/drag state, pointer capture, focus/IME, overlay trigger intent, inspector hitboxes, and final accessibility membership against the same committed frame. Returning to `Visible` rebuilds participation from current state without replaying stale input or focus claims.
- Keep component disabled state separate: disabled controls may remain discoverable in accessibility and can expose disabled semantics, while an inert subtree is absent from accessibility and all interaction. Keep `Display::None` separate because it removes layout.
- Add a Gallery matrix for visible/inert/hidden transformed subtrees with identical content and layout metrics. Include live state switching, focusable/editable controls, scroll/drag, tooltip/deferred content, overlay trigger, AccessKit probes, and inspector bounds.

**Test scenarios**

- A channel matrix proves exact layout, paint, hit/input, focus/IME, and final-accessibility participation for visible, inert, and hidden states, including custom elements that call low-level `Window` registration APIs.
- Nested-state tests prove ancestor dominance, no descendant escape, hidden-over-inert composition, and identical layout/scroll extent across state changes and U12 nested transforms.
- Dynamic visible-to-inert/hidden transitions clear hover/cursor, tooltip, pressed/drag state, pointer capture, focus/IME, stale overlay trigger intent, and AccessKit membership/actions in the same committed-frame contract; transitions back do not replay stale events or focus claims.
- Focus tests prove exact-leaf versus subtree observation, committed local focus while a platform window is inactive, active-window focus entry separation, no activation replay, one terminal completion per focus or blur request, sealed-frame deferral, bounded focus-only scheduling under alternating rejected targets, cross-update cancellation, transaction/close/drop cleanup, old-handle release during a newer unbound claim, and rejection of late-invalid presentation targets. Dock tests preserve a committed descendant only after typed completion, reject suppressed or late-invalid commands, and keep `NoPanelFocus` scoped to dock panels rather than clearing external window focus.
- Pointer, wheel, scrollbar, drag/drop, autoscroll, text input, keyboard traversal, semantic activation, and AccessKit Click each fail closed for inert/hidden descendants while visible siblings continue normally.
- Deferred and cache tests toggle only an ancestor presentation state without notifying a cached child and prove no stale paint, hitbox, capture, focus, IME, or accessibility entry survives. Portal tests prove explicit independent-overlay roots and non-resetting ordinary deferral.
- Gallery tests compare layout metrics and exercise real interactions across all three states at identity and transformed geometry.
- Public-surface and source-structure tests reject the removed `a11y_hidden` subtree hook, paint-only `Visibility::Hidden` gate, and any second ancestor-level inert/hidden flag.

**Deletion/replacement**

- Delete the late `div` paint-only `Visibility::Hidden` branch and replace its public subtree API atomically; no deprecated `Visibility` alias remains if the name cannot express inert semantics precisely.
- Delete `Element::a11y_hidden` and its independent inherited stack as subtree-presentation authorities. Preserve intentional leaf semantic omission only through the unified semantic projection.
- Delete ad hoc ancestor flags or component wrappers that separately suppress paint, hit testing, pointer listeners, focusability, IME, or accessibility where `SubtreePresentation` now owns the fact.
- Do not conflate hidden/inert with `Display::None`, disabled component semantics, overlay presence, opacity, clipping, or transform scale.

**Unit gate**

- Focused `open-gpui` layout, scene, input/capture, focus/IME, accessibility, deferred/cache/portal, inspector, and public-surface tests pass.
- Official component, Gallery, DevTools, docs, scanner, and migration tests cover the breaking presentation API and prove no old subtree authority remains.
- Review confirms the exact three-state matrix, dynamic cleanup, transform composition, custom-element fail-closed behavior, and no accessibility or focus escape hatch around an inert/hidden ancestor.

### U14. Add Committed Live Regions And Window Announcements

**Outcome**

Declarative live regions and transient announcements use the final AccessKit tree as their sole delivery authority. Renderer-neutral component state carries role, politeness, atomicity, and busy facts; a bounded per-window queue can add short-lived semantic nodes for imperative application notifications without calling native speech APIs, moving focus, replaying inactive work, or retaining message text in production diagnostics.

**Requirements**

- R4, R5, R7, R15, R18, and R19.

**Primary files**

- `crates/ui_core/src/a11y.rs`
- `crates/ui_components/src/a11y.rs`
- `crates/ui_components/src/feedback.rs`
- `crates/ui_components/src/toast.rs`
- `crates/gpui/src/elements/div/accessibility.rs`
- `crates/gpui/src/window/a11y.rs`
- `crates/gpui/src/platform/test/window.rs`
- accessibility tests under `crates/gpui/src/app/test_context/` and `crates/ui_components/tests/`
- `examples/ui-foundation-gallery/`
- accessibility API, ADR, migration, privacy, and verification documentation

**Behavioral work**

- Add renderer-neutral `Status` and `Alert` roles plus a closed live-politeness value to `SemanticDescriptor`. Preserve the existing busy fact and add atomicity without introducing an AccessKit dependency into `ui_core`.
- Map descriptor facts exactly once through the UI Components GPUI adapter and GPUI accessibility projection. `Status` defaults to polite and atomic, `Alert` defaults to assertive and atomic, and explicit compatible overrides follow the documented semantic contract.
- Add a bounded, window-owned transient announcement queue for explicitly window-global application notifications. U14 uses a fixed, non-configurable limit of 32 pending plus retained transient nodes per window. At capacity, it rejects the newest request with a typed queue-full outcome and metadata-only dropped diagnostic; it never evicts an accepted request or a node still owed its one-generation retention. Each accepted request receives a monotonic per-window sequence and stable synthetic node identity, preserves call order, and treats repeated equal text as a new semantic change. Equal-text coalescing is forbidden unless a later public channel/key contract explicitly requests it.
- Accept transient requests only for the current active accessibility generation while the window remains live. Requests made while inactive, after deactivation, during close, or from a stale activation generation are dropped and never replayed.
- Keep each transient node through at least one complete committed accessibility generation, then remove it in a later committed update without retaining an unbounded message history. Replacement activation, deactivation, and window teardown clear pending and retained nodes.
- Make U12 failed transactions and U13 inert/hidden membership suppress declarative regions with the rest of the semantic subtree. Document that adding a non-empty stable live node, including visible re-entry, can be announced; callers that need update-only behavior first commit an empty stable region.
- Record only window identity, request ID, sequence, politeness, and accepted/dropped lifecycle in diagnostics. Announcement text may appear in the test harness's captured final tree but must not enter DevTools history, export, reports, logs, or persisted diagnostics.
- Require element/component-lifecycle feedback to use declarative live regions, so U13 suppression, unmount, and stale generations are inherited from final-tree membership. The transient queue is never called automatically from component render or mount; application code may use it only when the notification is intentionally window-global and independent of an element source.
- Migrate first-party status/error/loading and toast-like consumers that currently need announcement semantics to declarative descriptors. Components project domain state; none owns an announcement timer, hidden semantic label, native backend call, or queue. An application may separately announce a domain event as a window-global command, but that is not inferred from component presence.
- Add a Gallery scenario for polite status updates, busy batching, assertive alert, same-text repeat, focus stability, inactive-drop behavior, and privacy probes.

**Test scenarios**

- Pure tests cover role defaults, off/polite/assertive, atomicity, busy state, and exact descriptor-to-GPUI mapping without introducing GPUI or AccessKit into `ui_core`.
- Final `TreeUpdate` tests cover stable declarative identity, value changes, busy true/content changes/busy false, unmount, deferred and cache replay, U12 rollback, U13 suppression and visible re-entry, and exact live/atomic/busy fields.
- Queue tests fill all 32 pending/retained slots, prove the newest request receives queue-full without an announcement sequence or text diagnostic, prove accepted and retained nodes are not evicted, and also cover ordered multi-request turns, same-text repetition, one-generation retention, deterministic removal, activation-generation replacement, deactivation, close, and the documented source-free window-global behavior across unrelated U13 subtree changes.
- Component tests prove declarative feedback disappears or becomes ineligible with inert/hidden/unmounted/stale sources and that rendering or remounting a component never submits an imperative queue request.
- Two-window tests prove request sequences, synthetic node IDs, activation state, and cleanup cannot cross windows.
- Focus and action tests prove status, alert, and transient nodes add no tab stops or actions and never change the winning focus claim.
- Privacy tests use a unique message canary that is present in the captured test `TreeUpdate` and absent from production diagnostics, DevTools capture/history/diff/export/artifact/report, and Gallery fixtures.
- Supported AccessKit platform adapters compile at the pinned version; owning native runners smoke-test emitted live-region events where their harness can observe them.

**Deletion/replacement**

- Delete component-owned hidden labels, announcement timers, or direct native announcement adapters replaced by this authority.
- Do not expose `aria-relevant`, delivery guarantees, a process-global queue, a direct platform speech API, or an accessibility-only focus workaround that the current backend contract cannot honor.

**Unit gate**

- Renderer-neutral descriptor, GPUI final-tree, queue lifecycle, window-isolation, focus, privacy, first-party consumer, and Gallery tests pass.
- Review confirms that every announcement is either committed semantic state or a committed transient semantic node, and that no message-text retention or native-speech bypass exists.

### U15. Add Typed Committed Portal Anchors

**Outcome**

GPUI exposes a narrow window-owned portal-anchor capability that binds one live target per frame and yields validated current-frame or committed geometry to followers. The snapshot carries opaque element geometry, generation, presentation membership, and effective clip bounds; absence, hidden state, unmount, failed transactions, and wrong-window use are explicit rather than silently reusing a raw rectangle. Inert remains a linked source fact, and each follower decides whether its own policy requires `Visible`.

**Requirements**

- R15, R17, R18, and R20.

**Primary files**

- `crates/gpui/src/geometry.rs`
- `crates/gpui/src/element.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/window/frame_journal.rs`
- `crates/gpui/src/elements/deferred.rs`
- `crates/ui_core/src/overlay.rs`
- `crates/ui_components/src/overlay/` and official overlay adapters
- anchor/runtime tests under `crates/gpui/src/app/test_context/` and `crates/ui_components/tests/overlay/`
- `examples/ui-foundation-gallery/`
- portal-anchor API, ADR, migration, and verification documentation

**Behavioral work**

- Add a stable handle created and owned by one window, one element binding entry point, and an opaque snapshot. One handle may bind exactly one target per frame and may feed multiple followers; duplicate binding and foreign-window resolution are typed errors.
- Bind during target prepaint. A later follower in the same frame may read the validated candidate, while reads outside the active draw transaction observe only the last committed snapshot. A failed U12/U13 transaction cannot publish a candidate.
- Publish window identity, committed frame generation, `ElementGeometry`, effective presentation state, and effective clip AABB without exposing the resolved transform matrix or mutable rectangle fields.
- Mark the handle unlinked when the target is absent from the completed frame, hidden before it can bind, unmounted, or invalid. An inert target remains linked with `Inert` in the snapshot; the binding authority never guesses whether a particular follower is interactive. Do not retain last-known geometry as an implicit fallback.
- Reproject cache replay under the current ancestor geometry and presentation scopes before binding. Ordinary deferred descendants inherit current geometry and clip; the named window-space portal reset consumes an already projected anchor and does not erase theme or presentation inheritance.
- Keep `OverlayAnchorInput` as renderer-neutral placement input. UI Components requires `Visible` for interactive overlay followers, converts an eligible snapshot to that value, and explicitly selects close, hide, or controlled-owner intent for unlinked or ineligible snapshots. Other follower kinds may define a different presentation policy. The handle itself never mutates overlay state.
- Migrate official trigger-bound Popover, Menu, Select, Tooltip, HoverCard, Dialog/Sheet affordances, and context surfaces where a raw point or bounds snapshot currently acts as a live anchor. Intentional pointer-point anchors remain explicitly named point anchors.
- Add Gallery cases for transformed/scrolled anchors, same-frame following, controlled unlink behavior, and multiple followers.

**Test scenarios**

- Runtime tests cover same-frame target-before-follower ordering, reads before binding, one target with multiple followers using different presentation eligibility, duplicate binding, hidden/absent unlink, inert linked state, unmount/rebind, wrong window, and completed-frame unlink.
- Geometry tests cover nested non-uniform U12 transforms, scrolling, effective clip bounds, U12 numeric failure, U13 transitions, deferred inheritance, cache replay under an ancestor-only change, and explicit portal reset.
- Overlay integration tests prove each unlink policy, controlled close intent, opening-generation stability, focus restoration, and no stale geometry after a failed or missing target frame.
- Public-surface tests prove the handle and snapshot are opaque, window-bound, and capability-specific; no generic node reference, raw matrix, cross-window conversion, or last-known fallback leaks.
- Gallery tests exercise a transformed trigger, multiple followers, scroll movement, hide/unmount, and controlled reopen through real overlay runtime paths.

**Deletion/replacement**

- Delete live-following code that stores raw trigger points/bounds, duplicates transform projection, or retains stale geometry after unlink.
- Do not generalize the handle into a DOM-like node reference, selection API, arbitrary lifecycle observer, or cross-window portal transport.

**Unit gate**

- GPUI binding/journal tests, official overlay suites, Gallery flows, migration docs, and source scans pass.
- Review confirms one target-per-frame ownership, current-versus-committed ordering, explicit unlink policy, and no raw geometry authority beside the handle.

### U16. Unify Bring Into View Across Focus, Accessibility, And Applications

**Outcome**

One window-owned bring-into-view authority resolves a stable target against its committed inner-to-outer scroll ancestry. Application requests, winning focus claims, and AccessKit `ScrollIntoView` use the same generation, physical-axis alignment, transform conversion, overlapping-chain arbitration, cancellation, and completion rules. Virtual collections materialize a logical target before the substrate reveals its physical binding.

**Requirements**

- R4-R6, R12, R15, R17-R18, and R21.

**Primary files**

- `crates/gpui/src/window.rs`
- `crates/gpui/src/window/a11y.rs`
- `crates/gpui/src/elements/div.rs`
- `crates/gpui/src/elements/list.rs`
- `crates/gpui/src/elements/uniform_list.rs`
- `crates/ui_core/src/focus.rs`
- `crates/ui_components/src/focus.rs`
- `crates/ui_components/src/scroll_surface.rs`
- Table, Tree, Command, Select, and VirtualizedList reveal adapters under `crates/ui_components/src/`
- reveal runtime tests under `crates/gpui/src/app/test_context/` and `crates/ui_components/tests/`
- `crates/motion/` consumer integration and `examples/ui-foundation-gallery/`
- reveal API, ADR, migration, and verification documentation

**Behavioral work**

- Add a capability-specific reveal target that records committed `ElementGeometry` and ordered scroll-container ancestry. It may share private binding storage with U15 but cannot be resolved as a portal anchor or generic node reference.
- Add renderer-neutral physical horizontal and vertical alignment values for `Nearest`, `MinEdge`, `Center`, and `MaxEdge`, plus explicit margins and instant/animated behavior. Both axes are always explicit; vertical convenience APIs preserve the horizontal position. Do not publish logical block/inline or start/end names before the locale and direction authority exists.
- Give each request a window-owned sequence and chain generation. Requests with disjoint committed scroll chains may proceed independently. Before mutating any shared scroll container, a newer request cancels older work whose chain overlaps that container; this applies across application, focus, and AccessKit sources. Direct user scroll, target unmount, U13 suppression, window close, or no-progress cycle also cancels affected work deterministically.
- Process committed containers inner-to-outer. Apply one container's local logical delta, commit the resulting geometry, then continue outward until visible or no progress is possible. Every delta uses U12 opaque point/vector conversion, so non-uniform transforms do not over-scroll.
- Submit focus reveal only after end-of-turn focus arbitration selects a winning claim. Losing or stale focus claims do not scroll. Dispatch AccessKit `ScrollIntoView` into the same request path and reject stale or suppressed nodes through existing action authority.
- Preserve an opaque input-era `ScrollChainFence` across virtual materialization and focus handoffs. It validates the full ordered scroll chain, available axes, and direct-scroll revisions without recapturing after input; a fenced focus claim may settle logically while its implicit physical reveal is suppressed.
- Run virtual Tree physical materialization only in a terminal focus-stable prepaint phase after ordinary prepaint commits. Reject focus and blur mutations from that phase so its exact revision check cannot be invalidated after materialization begins.
- Define a two-phase virtual protocol: the collection resolves and materializes a stable logical item, then binds the physical reveal target. GPUI does not infer indices, row IDs, or virtual ranges.
- Use Motion only for deterministic timing samples and respect the effective reduced-motion floor. Instant reveal remains independent of Motion. Animated requests use fake-clock tests and cancellation without altering focus ownership.
- Treat an explicit portal as a new rendered scroll ancestry. Following a source anchor back through an old tree requires an explicit application policy, not implicit ancestor guessing.
- Migrate component-private nearest-row, focus-scroll, and AccessKit reveal tails that duplicate the new authority, preserving list-specific materialization and Table identity semantics.
- Add a Gallery scenario with nested two-axis scrollports, transformed targets, keyboard focus, AccessKit action, direct request, cancellation, and virtual materialization.

**Test scenarios**

- GPUI tests cover nested vertical and mixed-axis scrollports, nearest/min-edge/center/max-edge, margins, oversized targets, already-visible no-op, no-progress termination, wrong-window target, and portal boundary.
- Transform tests cover non-uniform scale and translation at target and container levels, cached/deferred target geometry, logical delta correctness, and U13 suppression.
- Focus tests prove only the winning claim reveals, restore/initial claims preserve ordering, stale claims do not scroll, and focus itself does not gain a second authority.
- Virtual focus tests cover direct input before first materialization, ordinary later-prepaint competing claims, focus-stable callback rejection, rejected static-handoff retry, and a newer claim that prevents that retry from reclaiming focus.
- Accessibility tests dispatch real `ScrollIntoView` against the published node and prove the same request, stale-node rejection, final geometry, and window isolation.
- Arbitration and cancellation tests cover different targets with disjoint chains, different targets sharing an inner or outer container, same-turn application/focus/AccessKit conflicts, newer requests, user wheel/scrollbar input, unmount, suppression, close, reduced-motion completion, and interrupted animation using a fake clock.
- Virtual collection tests cover materialize-then-reveal by stable Tree/List/Table identity across reorder, filtering, recycle, and unavailable targets without teaching GPUI domain IDs.
- Gallery tests exercise application, keyboard-focus, and AccessKit entry paths through the same nested transformed scroll flow.

**Deletion/replacement**

- Delete component-owned focus-scroll tails and fixed-row reveal arithmetic once their materialization adapters target the common authority.
- Preserve low-level direct scrolling as an explicit operation, but do not let it masquerade as nested reveal or continue an older animated request.
- Do not add implicit portal ancestry, index-based virtual targeting, or a focus-owned scrolling runtime.

**Unit gate**

- GPUI, focus, AccessKit, Motion adapter, collection, Table, Gallery, migration, and public-surface tests pass.
- Review confirms one request authority, inner-to-outer committed ordering, overlapping-chain arbitration, transform-correct deltas, deterministic cancellation, physical-axis naming without premature direction semantics, and clean separation between logical materialization and physical reveal.

### U17. Add Exact Rounded-Rectangle Subtree Clipping

**Outcome**

One checked frame-local clip authority handles rectangles and rounded rectangles for an element and all descendants. Nested clips remain an exact stack, constrain both paint and initial hit testing, compose with U12 transforms and U13 presentation, and preserve layout. Every renderer consumes one validated clip ABI; unsupported native-surface or numeric combinations fail closed rather than flattening to an AABB.

**Requirements**

- R1-R2, R15, R17-R18, R20, and R22.

**Primary files**

- `crates/gpui/src/geometry.rs`
- `crates/gpui/src/style.rs`
- `crates/gpui/src/element.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/window/frame_journal.rs`
- `crates/gpui/src/window/a11y.rs`
- `crates/gpui/src/scene.rs`
- `crates/gpui/src/elements/div.rs`, `deferred.rs`, `list.rs`, `uniform_list.rs`, `text.rs`, `img.rs`, and `surface.rs`
- `crates/canvas/src/gpui/painter.rs` and `painter/primitives.rs`
- `crates/gpui_docking/src/viewport_host_geometry.rs` and `viewport_drop_route.rs`
- `crates/gpui_wgpu/`, `crates/gpui_windows/`, and `crates/gpui_macos/` renderer and shader contracts
- clip tests under `crates/gpui/src/app/test_context/`, renderer crates, and `examples/ui-foundation-gallery/`
- clip API, renderer ABI, ADR, migration, accessibility-limit, and verification documentation

**Behavioral work**

- Start with the accepted renderer/ABI checkpoint in ADR 0026. Public declarations are child-local and border-box relative; checked constructors normalize finite non-negative elliptical radii before U12 projection; `SubtreeGeometryValidity` suppresses all affected channels on transform, clip, or device-conversion failure; target capability excludes unsupported native-surface APIs before Scene submission; `ClipStackSnapshot` is the immutable frame/journal/hit snapshot; the Scene owns a dynamically sized deduplicated flattened clip arena; each primitive carries `conservative_bounds + first_clip + clip_count`; and WGPU, DirectX, and Metal consume one shared exact clip-shape ABI. Cache replay and `Scene::finish` import/remap ranges. Do not publish the public wrapper until this checkpoint is reviewable on all supported backends.
- Define public clip bounds in zero-origin child-local logical coordinates relative to the child's post-layout border box. Provide an own-border-box shorthand; explicit bounds remain in that same local space. Normalize non-negative finite elliptical radii against the declared local clip box before U12 projection, preserve the resulting ellipses under non-uniform scale, and reject unrepresentable projection instead of clamping into a different shape after composition.
- Replace the rectangle-only `ContentMask` stack with one resolved clip stack. Existing overflow/content-mask call sites become inputs to the same authority and remain fast rectangular paths where possible.
- Carry the exact nested stack through every scene primitive and renderer batch. Culling may use conservative AABBs, but final fragment coverage and hit containment cannot collapse rounded intersections to a single rectangle.
- Use the same committed stack for initial hover/click/wheel/drag/drop hit testing and debug visualization. Pointer capture acquired from a valid hit continues according to existing capture rules after the pointer leaves the clip; the clip does not invent a second capture policy.
- Make ordinary deferred and cached descendants inherit and replay the clip under current transforms. The named window-space portal resets clip ancestry deliberately. Numeric or backend conversion failure suppresses the complete affected subtree transaction across paint, hit/input, focus/IME, debug, deferred/cache, and accessibility.
- Update U15 portal-anchor snapshots from the new resolved stack. Their effective clip remains a conservative committed window-space AABB, with rounded/nested containment retained privately for paint and hit testing; transform, cache replay, and portal reset must not publish a stale rectangle.
- Project accessibility conservatively: fully clipped nodes are absent, partially clipped nodes retain an AABB intersection, and the clip owner exposes `clips_children` where supported. Document that AccessKit cannot express rounded hit regions.
- Gate accessibility publication through the shared CPU exact-visibility query. It must return a conservative AABB and a point proven inside the candidate and every clip; uncertain or boundary-only intersections fail closed, and built-in fallback `Click` uses that witness rather than the AABB center.
- Resolve Canvas clips during prepaint into an opaque window/frame/validity-bound token and permit paint to re-enter only that token. For style overflow, model one-axis clipping as an inherited-stack rectangular strip and derive two-axis padding-box ellipses from asymmetric borders before shared normalization. Dock route snapshots retain validity and presentation eligibility as well as exact geometry.
- Define native paint-surface behavior explicitly. A backend unable to apply the resolved clip rejects or isolates the combination; it never paints outside the shape while reporting success.
- Add a Gallery matrix for rectangular, symmetric/asymmetric rounded, nested, transformed, scrolling, deferred, image/text/surface, and interactive clips.

**Test scenarios**

- Pure tests cover normalized and asymmetric elliptical radii, zero radii equivalence, invalid values, nested stack order, U12 composition, and points immediately inside/outside every corner.
- Runtime tests cover layout invariance, exact hover/click/wheel/drag/drop containment, capture after acquisition, scrollports, U13 suppression, deferred inheritance, cache replay, portal reset, debug geometry, and fail-closed late conversion.
- Anchor regression tests cover conservative effective clip AABBs for nested rect/rounded stacks under non-uniform transform, cache replay, scrolling, hidden/unlinked state, and named portal reset.
- Scene tests prove every primitive carries the same stack and conservative culling never becomes final clip coverage. Text, glyph sprites, paths, images, shadows, and surfaces are mandatory.
- Accessibility tests cover fully and partially clipped nodes, conservative bounds, `clips_children`, stable identity, actions inside the visible region, and stale-node removal.
- Each renderer has compile-time ABI/conversion tests for rect and rounded stacks. Capable native runners execute nested asymmetric/non-uniform pixel smokes, including overlap and corner-edge samples.
- Gallery tests exercise real pointer, scrolling, drag/drop, deferred/cache, inspector, and accessibility paths across all clip variants.

**Deletion/replacement**

- Delete rectangle-only clip stacks, duplicate per-element descendant clip flags, and renderer-specific rounded approximations replaced by the shared authority.
- Keep primitive-local corner radii for drawing a shape, but do not treat them as descendant clipping unless they enter the subtree clip stack.
- Do not expose arbitrary path clips, fill rules, stencil/tessellation choices, group opacity, blend modes, or silent AABB/native-surface fallbacks.

**Unit gate**

- The renderer/ABI checkpoint is accepted before public syntax lands. GPUI cross-channel, renderer conversion/ABI, Gallery, migration, and public-surface tests pass locally before commit; native pixel and owning-platform backend jobs pass before U17 is declared complete.
- Review confirms exact nested containment, one paint/hit authority, layout invariance, transform/presentation/deferred/cache/portal behavior, conservative accessibility limits, and no path-shaped placeholder.

### U18. Add One Dock Visual Style Authority

**Outcome**

Every Dock visual path consumes one immutable `DockVisualStyle` resolved in the active host context. Applications may map the current window or subtree theme through a named resolver without introducing a `gpui_docking -> ui_components` dependency. The built-in fallback is the only production location for literal default colors.

**Requirements**

- R1-R2, R9-R11, R15, R18, and R23.

**Primary files**

- `crates/gpui_docking/src/host.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/render_tabs.rs`
- `crates/gpui_docking/src/render_floating.rs`
- `crates/gpui_docking/src/drop_preview.rs`
- `crates/gpui_docking/src/geometry.rs`
- new style/resolver modules and tests under `crates/gpui_docking/src/`
- `crates/gpui_docking/src/public_surface_tests.rs`
- `examples/docking-native/` and `examples/docking-minimal/`
- `xtask/src/ui_contract.rs`, Dock API docs, ADR, migration, and release documentation

**Behavioral work**

- Inventory every Dock paint fact: host background, empty/missing states, tab strip and every reachable selected/hovered tab state, splitter states, floating frame/title/actions, focus ring, drag payload, inner/outer guides, accepted/rejected previews, and transparent routing surfaces. Do not publish speculative disabled-tab states until the Dock model owns corresponding disabled behavior.
- Define one complete immutable style value with explicit state palettes and visual elevation/shadow inputs. Keep layout geometry, hit slop, splitter thickness, and drop-guide sizing in structural options. Rename public `DockDropGuideStyle` to `DockDropGuideMetrics` in the breaking migration so two public values cannot claim visual-style authority.
- Add one named render-time resolver owned by Docking. The immutable resolver value is installed per `DockSurface` or passed explicitly to a low-level `DockHost`; no mutable app-global registry or fallback lookup exists. Its synchronous callback accepts only the active GPUI render context, returns a complete style, and must not update entities, notify, dispatch, mutate registration, or reenter rendering.
- Resolve each host from its current window/subtree context on every relevant render generation. U7 ancestor-only theme changes must invalidate cached Dock rendering without mutating DockGraph or surface revision.
- Render target-window drop guides and previews with the target host style. Freeze a source-owned deferred drag visual from the source style in runtime metadata keyed by drag session and opening generation, matching U7 overlay capture semantics. The visual snapshot is not stored in `DockDragPayload`, cannot affect payload `Eq`, identity, route validation, or persistence, and is cleared on close/cancel before reopen captures a new style.
- Keep a deterministic built-in fallback for applications that install no resolver. Application/example integration may map `ThemeResolver::current` into `DockVisualStyle`, but that mapping remains outside `gpui_docking`.
- Add a source gate that rejects Dock production color/elevation literals outside the built-in style definition and explicitly allowlisted transparent hit-routing constants.

**Test scenarios**

- Pure tests cover completeness, equality, built-in fallback, every interaction-state lookup, and absence of implicit partial/default merging.
- Runtime tests render two surfaces with different immutable resolvers, two hosts in different windows, and two hosts under different U7 subtree scopes. Changing one scope proves independent visual updates with unchanged layout, selection, focus history, and surface revision; resolver callbacks are also guarded against update/notify/reentrant use.
- Cache/deferred tests cover ancestor-only theme changes, floating panels, source-frozen drag visuals, target-resolved guides, cancellation, and reopen generation capture. Payload equality and route validation remain identical across different visual snapshots, and retired session metadata cannot leak into a later drag.
- U13 tests prove Inert remains styled and painted while Hidden emits no Dock paint; restoring Visible resolves the current style rather than replaying stale paint.
- Public-surface and source-scan tests prove the resolver is named, immutable, per-surface/explicit-host, and narrow; `DockDropGuideMetrics` is structural; no UI Components dependency enters `gpui_docking`; and production render paths contain no competing palettes.
- `examples/docking-native` exposes light, dark, and high-contrast host contexts with real drag/drop and floating surfaces; structural smoke tests assert resolver/style changes rather than pixel-perfect platform screenshots.

**Deletion/replacement**

- Delete hard-coded production colors, local palette constructors, per-render-path style defaults replaced by `DockVisualStyle`, and the misleading `DockDropGuideStyle` name.
- Do not move `ThemeContext` or `ThemeResolver` into Docking, add an optional reverse dependency on UI Components, or expose a generic render-context registry.
- Do not copy ImGui's default colors pixel-for-pixel; ImGui remains the interaction-state reference, while applications own brand/theme mapping.

**Unit gate**

- Dock render/runtime tests, examples, public-surface checks, dependency scans, style-literal scan, docs, and release verification pass.
- Review confirms one complete visual authority, resolver scope and purity, window/subtree isolation, source-versus-target drag styling with payload-identity separation, layout neutrality, and no theme dependency inversion.

### U19. Make DockSurface The Application Owner Of Change And Activation

**Outcome**

`DockSurface` becomes a cloneable handle to one private owner entity that aggregates committed controller and viewport-runtime changes. Applications receive a monotonic revision and typed change events, activate panels by stable item ID with typed focus completion, and explicitly export snapshots for their own debounce and persistence policy.

**Requirements**

- R1-R2, R5-R6, R13, R15, R18, and R24.

**Primary files**

- `crates/gpui_docking/src/surface.rs`
- `crates/gpui_docking/src/surface/panel.rs`
- `crates/gpui_docking/src/surface/state.rs`
- `crates/gpui_docking/src/surface/viewport.rs`
- `crates/gpui_docking/src/controller.rs`
- `crates/gpui_docking/src/host.rs`
- `crates/gpui_docking/src/presentation_commands.rs`
- `crates/gpui_docking/src/viewport_runtime.rs` and `viewport_runtime_handle.rs`
- `crates/gpui_docking/src/surface_tests.rs`, `host_render_tests.rs`, and `public_surface_tests.rs`
- `examples/docking-minimal/`, `examples/docking-native/`, and `examples/docking-multiviewport/`
- Dock facade, persistence, ADR, migration, and release documentation

**Behavioral work**

- Replace the plain controller/runtime tuple facade with a private owner entity referenced by every `DockSurface` clone. Low-level `DockController` construction remains available for advanced internal composition, but ordinary facade hosts and viewport sessions share one owner.
- Add a private `DockSurfaceTransactionId` and root transaction boundary. Every facade/host/runtime mutation allocates a root identity before work begins; nested controller and viewport operations carry that identity plus typed change categories, and the owner publishes once only when the outer transaction commits. Advanced low-level controller mutations allocate their own commit identity when they enter a surface owner. Two root commands in the same App turn, two commands across turns, and an asynchronous platform observation are distinct transactions; no end-of-turn heuristic may merge them.
- Add typed controller-commit and viewport-runtime-commit notifications so the owner never infers state changes from generic `notify`, render count, or snapshot comparison. Host drag/drop, splitter, selection, open/close, floating, tear-off, dock-back, observed external native placement, and facade commands enter these commit channels. Viewport request dispatch, including any pre-U20 `applied` sync record, is intent only and cannot create a surface revision.
- Publish one monotonic surface revision and a typed event carrying the revision and bounded change categories such as layout, selection, panel lifecycle, viewport topology, and observed viewport placement. The private transaction identity governs internal coalescing but is not exposed as a mutable or public transaction API. Coalesce categories carrying the same transaction into one revision; failed, rejected, superseded, unchanged, focus-only, style-only, and dispatch-only work emits no persistence revision.
- Keep event payloads metadata-only. Applications call `export_snapshot` after their own debounce; the owner does not allocate a full snapshot per event, start timers, choose a storage format beyond the existing versioned snapshot, or perform file I/O.
- Make snapshot export read layout and viewport placement from one committed owner turn and expose the associated revision, preventing callers from pairing a new layout with stale viewport facts silently.
- Give each space exactly one committed activation-host registration generation within a surface. The first live mounted host owns activation until it unregisters; a duplicate same-window or cross-window host registration is rejected with a typed diagnostic and cannot silently replace ownership. A later host may acquire a new generation only after release, and stale registration callbacks cannot act on it.
- Add stable-item `activate_panel` to the facade. It locates the owning space and its unique current activation-host/window generation, selects the item when necessary, requests platform activation under existing policy, and sends a generation-bound panel focus command. Its terminal typed outcome follows the exact descendant GPUI focus completion: committed, rejected, superseded, unavailable, duplicate-host conflict, or window closed.
- Preserve `select_panel` as an explicitly selection-only operation. Dropping an activation subscription cancels observation but not the issued activation intent, matching GPUI focus completion semantics.
- Demote node-ID `DockHost::focus_pane` from the common public facade because the workspace has no external consumer; keep any required node-level primitive crate-private for spatial presentation commands.

**Test scenarios**

- Owner tests prove every committed layout and observed viewport category increments once, categories carrying one explicit transaction identity coalesce, two root transactions in one App turn do not coalesce, cross-turn transactions remain distinct, revisions are monotonic across clones, and failed/unchanged/style/focus/dispatch-only operations do not emit.
- Persistence tests subscribe, debounce with a fake clock in application test code, export the event revision, round-trip the snapshot, and prove no automatic timer, path, or file I/O exists in Docking.
- Activation tests cover selected and hidden tabs, nested descendant caret preservation, detached viewport activation, inactive windows, U13 Inert/Hidden rejection, late-invalid targets, item removal, newer activation, dropped subscription, window close, exact once terminal delivery, duplicate hosts in one and multiple windows, unregister/replacement generations, and stale owner callbacks.
- Adversarial ordering tests cover selection commit followed by rejected focus, viewport replacement during activation, stale equal-item generation callbacks, and a callback that immediately activates another panel.
- Public-surface tests require stable item IDs and typed outcomes, reject node-ID facade focus, and preserve advanced low-level model access without exposing the private owner.
- Example smokes demonstrate revision/event logging, caller-owned debounced snapshot export, restore, and panel activation without direct controller/runtime assembly.

**Deletion/replacement**

- Delete facade paths that assemble independent controller/runtime handles, generic-notify or end-of-turn persistence inference, request-dispatch placement revisions, silent duplicate-host registration, and public node-ID focus entry points superseded by stable-item activation.
- Do not add a global Dock manager, automatic file writer, built-in debounce duration, filesystem path setting, or snapshot event payload.
- Do not merge selection and focus facts: selection may commit while focus completion rejects, and the event/activation outcomes must report that split honestly.

**Unit gate**

- Surface, controller, host, viewport, persistence, activation, example, public API, migration, and release tests pass.
- Review confirms one owner and revision stream, explicit transaction identity and boundaries, commit-only event publication from observed facts, unique activation-host generations, exact focus completion, clone/window lifecycle safety, and caller-owned persistence policy.

### U20. Add Capability-Specific Platform Window Mutation

**Outcome**

GPUI exposes one backend-neutral contract for mutating an already-open window's placement and supported independent flags. Capabilities remain property-specific, but position, size, state, and restore bounds share one coherent placement conflict domain. Dispatch reports only what GPUI can know synchronously and returns a generation-bound observation ticket when work is queued; observed `WindowPlatformFacts` remain the sole committed authority consumed by public getters and Dock. Dock viewport synchronization no longer advertises `live_window_move` when only resize exists.

**Requirements**

- R1-R2, R13, R15, and R25.

**Primary files**

- `crates/gpui/src/platform.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/platform/test/`
- native `PlatformWindow` implementations and tests in `crates/gpui_windows/`, `crates/gpui_macos/`, and `crates/gpui_linux/`
- `crates/gpui_docking/src/viewport_platform_sync.rs`
- `crates/gpui_docking/src/viewport_runtime_status.rs`
- `crates/gpui_docking/src/surface/viewport_readiness.rs`
- `crates/gpui_docking/src/host_viewport_platform_capability_tests.rs`
- `examples/docking-multiviewport/` and `crates/gpui/examples/window_positioning.rs`
- platform capability, Dock facade, ADR, migration, and release documentation

**Behavioral work**

- Begin with a backend census and design checkpoint covering shared-desktop coordinates, DPI conversion, work-area clamping, position, size, windowed/maximized/fullscreen/minimized state and restore bounds, pointer acceptance, focus-on-appear/click, alpha, topmost, and taskbar presence. Record each property as unsupported, creation-only, or live rather than grouping them under one boolean. Inventory every current unit/bool setter and logged asynchronous failure before assigning an outcome.
- Reshape native backend setter contracts before adding the public wrapper. A dispatch outcome is `Queued(ticket)`, `Unchanged`, `Unsupported`, `Rejected`, or `WindowClosed`; queued means only that GPUI handed the request to the backend dispatch path. It never means the OS accepted or applied it, and legacy unit/bool returns that conflate unchanged/applied/unsupported must be replaced rather than inferred.
- Define the ticket's terminal observation independently: exact observed facts, adjusted observed facts, superseded by a newer generation in the same conflict domain, rejected/native failure when observable, unsupported, or window closed. Dropping an observation subscription cancels delivery only, not queued intent. Every terminal path is exactly once and bounded even if callbacks reorder or never arrive before close.
- Treat position, size, state, and restore bounds as one placement request, generation, and conflict domain. The planner canonicalizes a coherent placement before dispatch, defines windowed restore-bounds semantics, and rejects contradictory or unrepresentable state-plus-geometry input without sending a partial native request. Independent flag domains may coexist and partially dispatch; partial success never exists inside placement itself.
- Add one committed `WindowPlatformFacts` cache inside `Window`, seeded from creation facts. Coherent moved/bounds/state/flag callbacks or an explicit backend observation refresh update that cache and settle matching tickets; queued intent cannot update it. Existing public `window_bounds`, `inner_window_bounds`, `bounds`, fullscreen/minimized, pointer-input, and related getters read this authority rather than querying a second backend fact path.
- Give six live request domains monotonic generations: one coherent placement domain, pointer input, one coherent `ActivationPolicy` domain, alpha, topmost, and taskbar visibility. `ActivationPolicy` carries independent `accepts_activation` and `focus_on_click` fields under one generation and terminal observation; neither field derives the other and the domain cannot partially commit. A newer request supersedes older pending intent in that domain; unrelated flag requests may coexist. `focus_on_appearing` remains a creation-time fact and never becomes a live mutation domain. Every backend terminal carries its domain and generation, and GPUI rejects stale generations before committing their facts. State-generated move/resize callbacks belong to the placement generation and commit one coherent snapshot rather than accidentally settling independent size/position tickets. External user or window-manager adjustments publish actual facts without a corrective loop unless a newer explicit request still owns the domain. Window close invalidates queued backend generations before settling retained tickets.
- Require a readable observation path for any property advertised as live. If a backend can issue a setter but cannot determine the resulting fact, it remains creation-only or unsupported in the public capability contract.
- Migrate existing resize, pointer-input, fullscreen, and creation-option paths into the common dispatch/ticket vocabulary where they represent the same operation. Preserve ergonomic single-property helpers as thin typed wrappers over placement or a flag domain rather than parallel authorities.
- Replace Dock's `live_window_move` and flag capability mirrors with projections of the GPUI contract. Dock viewport sync requests only changed domains, records queued dispatch separately from terminal observation, uses committed facts for snapshots/routes/readiness, and reports independent unsupported flags without discarding a supported placement request. U19 may revision only the resulting observed placement transaction, never this dispatch record.
- Query capabilities for every window's actual `WindowKind` and target display, then capture the resolved matrix in one immutable mutation profile when the window opens. An unavailable saved display id resolves to the current default before both projection and opening, while a backend with no default fails structurally rather than indexing an invalid screen. Keep the profile readable by handle while a window update temporarily owns mutable state, and remove it on close. Dock runtime status projects each registered viewport from that profile rather than applying the backend's normal-window or primary-display matrix to heterogeneous windows. Display-dependent support such as X11 alpha creation must reflect the target screen's actual native resources.
- Implement every property each backend can prove; unsupported properties remain explicit. Native owning-platform tests and CI, not active-platform assumptions, determine the capability matrix.

**Test scenarios**

- Pure tests cover capability projection, unsupported/creation-only/live distinctions, all six live request domains plus creation-time appearance, every independent `accepts_activation`/`focus_on_click` combination inside one coherent generation, dispatch and terminal outcomes, unchanged detection, coherent state/geometry planning, restore bounds, contradictory batch rejection, DPI/shared-coordinate conversion, independent-domain partial dispatch, rejection of partial activation-policy commit, generation supersession, and stale-terminal rejection before fact commit.
- `TestPlatform` tests deterministically separate queued dispatch from observation, report backend rejection, adjust requested placement, inject external movement, emit state and geometry callbacks in either order, omit callbacks until close, normalize unavailable display ids to the default before creation, and prove committed facts plus one-shot observers cannot be forged by intent. Request, immediate public getter, callback, terminal delivery, supersession, dropped subscription, and close ordering are all explicit.
- Dock tests cover move-only, resize-only, move-plus-resize, windowed/maximized/fullscreen/minimized transitions with restore bounds, mixed supported/unsupported flags, stale/adjusted observations, external user movement, route/preview geometry, placement export, no retry loop, and absence of a revision from dispatch alone.
- Native Windows, macOS, and Linux tests compile every trait implementation and assert exact capability matrices plus kind-specific and display-dependent projections such as Wayland LayerShell and X11 screens with or without a transparent visual. On every backend that advertises a live domain, owning-platform integration tests exercise supported dispatch, native failure, getter-cache seeding, callback conversion, placement, and each live flag against actual coherent observed facts. Creation-only backends prove their creation projection without fabricating live dispatch.
- Multi-viewport example smokes display the capability matrix and last request/observation separately, then exercise tear-off placement, live move/resize, docking back, and graceful unsupported flags.
- Public-surface and scanner tests prove the ambiguous `live_window_move` claim and Dock-local semantic duplicates are absent.

**Deletion/replacement**

- Delete `live_window_move`, Dock-local capability facts that can drift from GPUI, unsupported records that label queued intent as an applied platform fact, direct backend getter paths that bypass `WindowPlatformFacts`, and legacy unit/bool setter results whose meanings are ambiguous.
- Do not expose native window handles, backend callback tables, an ImGui PlatformIO clone, or one optimistic `set_bounds` API that hides partial support.
- Do not promise synchronous OS commitment, partial success inside the placement conflict domain, atomicity across placement and independent flags, or dynamic support for creation-only flags.

**Unit gate**

- GPUI/TestPlatform, Dock sync/readiness/runtime, examples, public API, migration, and release tests pass locally. Every native backend compiles on its owning runner, and its advertised live properties pass observed-fact integration tests before U20 is complete.
- Review confirms capability honesty, dispatch/terminal-observation separation, placement conflict and restore-bounds semantics, coherent committed getter authority, independent-domain partial support, coordinate correctness, and no Dock-local platform authority.

### U24. Make Platform Event Delivery Reentrancy-Safe

**Outcome**

Every asynchronous native callback reaches GPUI through an AppCell-owned typed ingress even when another window update already owns the mutable `App`; synchronous native queries have an immutable, non-reentrant answer path. Hybrid input always returns its immediate handler-derived native disposition: framework-owned commands capable of pumping such input execute only after the outer App borrow is released, and a busy entrance is an invariant violation rather than a guessed fallback. Callback delivery has explicit global sequencing, merge, and barrier rules, frames remain invalid until accepted, Dock/model effects cross subordinate borrows, and the closed pump-sensitive command set crosses the outer AppCell borrow without becoming a generic outbox.

**Requirements**

- R1-R2, R15, and R25-R26.

**Primary files**

- `crates/gpui/src/app.rs`
- `crates/gpui/src/app/cell.rs`
- `crates/gpui/src/app/async_context.rs`
- `crates/gpui/src/app/window_registry.rs`
- `crates/gpui/src/window.rs`
- private native-event ingress and synchronous-query snapshot modules adjacent to `AppCell`
- private closed typed platform-command FIFO adjacent to `AppCell`
- `crates/gpui/src/platform/test/`
- `crates/gpui_windows/src/events.rs`
- `crates/gpui_windows/src/platform.rs`
- corresponding callback adapters in `crates/gpui_macos/` and `crates/gpui_linux/`
- `crates/gpui_web/` compile and run-loop adapters
- `crates/gpui_docking/src/viewport_runtime_handle.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/close_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/scene_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_effects.rs`
- focused GPUI callback and Dock runtime reentrancy tests

**Behavioral work**

- Inventory every platform callback currently routed through `AsyncApp::update_window` and classify it first as an asynchronous fact/event, synchronous query, or hybrid event plus immediate platform disposition. For asynchronous events, assign a typed domain, merge policy, ordering/barrier requirement, terminal behavior, and stale-window disposition. Hit testing reads committed immutable facts, and close permission prevents immediate native destruction while queueing close intent for later approval. For every hybrid input message class, record whether native default handling depends on the current `DispatchEventResult`; fixed committed policy and delayed replay are forbidden for those classes. Identify every framework-owned platform operation that can synchronously pump them, route that closed command set through the post-`AppRefMut` FIFO, and prove both consumed and propagated handler results with zero busy entrances. Any busy entrance fails the invariant and U24 gate. Any other return-valued callback must name and prove its equivalent result contract before implementation.
- Put the private ingress beside `RefCell<App>` in `AppCell`, where a native callback can write while `App` is borrowed. The envelope carries the full generation-bearing `WindowId`, application-wide ingress sequence, callback kind, typed payload, and only its actual pointer/mutation/session/drag domain generation. Do not add another generic window generation unless callback replacement within one still-live `WindowId` proves a separate epoch necessary.
- Assign ingress sequence before testing the App borrow. A callback may drain inline only after acquiring exclusive drain ownership when no backlog, active drain, or unresolved barrier exists. Otherwise it queues and schedules a foreground wake. Reentrant events created during drain return to the sequenced queue rather than recursively borrowing `App`.
- Define event-domain behavior before implementation. Frame requests, hover, continuous pointer movement within one unchanged capture/button generation, and coherent move/resize/state facts may replace older pending facts in their domain. Queue-eligible button edges and text input, pointer cancellation, asynchronous close-request/closed lifecycle facts, and mutation terminal observations are FIFO and non-droppable. Must-immediate key, wheel, non-client, and other handler-disposition-dependent classes use the synchronous idle-only path. Pointer cancellation and close create barriers after which older input cannot run; close settles retained mutation observations and retires the generation before a reused ID can receive events.
- Bound each drain turn so an input or callback storm cannot starve the foreground executor. Preserve application ingress order and defined cross-window session barriers across partial drains, schedule another wake while work remains, and expose structured pending/delivered/coalesced/stale/closed dispositions to test diagnostics without retaining user input text.
- Change frame acknowledgement so an App-borrow conflict does not count as a successful paint. A native invalidation remains pending until GPUI accepts a draw/present request or explicitly re-invalidates and schedules it. Windows paint validation cannot permanently consume a failed callback; repeated frame callbacks for one generation may coalesce while guaranteeing a later presentation opportunity.
- Route placement/state, activation facts, hover, queue-eligible input, close, frame, and U20 mutation-observation callbacks through the typed ingress. Route must-immediate hybrid input through one dedicated synchronous AppCell entry that first respects older ingress barriers and rejects a busy App invariant instead of defaulting. No backend may keep a privileged direct path whose loss semantics differ.
- Make entity/controller/viewport-runtime operations return typed native effects. Release those subordinate guards, then apply model-owned create/open, remove/close, present, sibling-window update, and mutation work through the current `&mut App`. Capture `activate`, native window menu, interactive move, and interactive resize as the closed `PlatformWindowCommand` set with a weak backend dispatcher; enqueue them with full `WindowId`, release the outer `AppRefMut`, settle older event barriers, then drain commands synchronously and non-recursively. Commands enqueued by a command append to FIFO. Before native creation, reserve the full `WindowId`; synchronous callback envelopes wait until the window transaction commits or rolls back, then deliver to the committed window or retire against the rolled-back ID. Keep `App::open_window`, fallible map, ownership close, and frame draw/present in their existing synchronous authorities; no arbitrary callback outbox or asynchronous open-window outbox is added.
- Fix the known tear-off failure path that closes a newly opened viewport while holding the runtime `RefMut`. Apply the same two/three-phase pattern to scene reconciliation, activation, source invalidation, close observers, and shutdown preparation: collect identity/generation, release the guard, obtain external facts or apply effects, then short-borrow to finalize only if the generation still matches.
- Establish the smallest reusable Windows real-HWND test support needed for reentrant create/show/paint/activate/close, reserved-window commit/rollback, and ingress observations. U25-U27 extend this same harness; U28 completes the scenario matrix and CI/subprocess hardening.

**Test scenarios**

- Deterministic App-borrow tests inject every asynchronous callback domain while an update is active and prove direct and queued delivery produce the same committed result, callback kind is observable, no event is reduced to a generic `RefCell already borrowed` log, and nested drain-time callbacks do not recurse. Synchronous-query tests prove committed hit testing and prevent-and-queue close intent. TestPlatform's command executor synchronously triggers at least one consumed and one propagated hybrid input after `AppRefMut` release, proves the exact handler results, asserts the App can be mutably borrowed at callback entrance, keeps a zero busy-invariant count, and proves nested command enqueue is FIFO rather than recursive. The real-HWND matrix repeats the owning-platform result contract. Any fixed fallback, delayed replay, or busy entrance fails.
- Ordering tests cover an older queued event followed by a borrowable callback, cross-window source/target/anchor causality, coalesced frame/hover/move/placement facts, non-coalesced down/up/key/text/terminal observations, cancellation and close barriers, generation-bearing `WindowId` reuse, close settling pending mutation tickets, partial drains, wake coalescing, and exactly-once terminal delivery.
- Frame tests force a callback borrow conflict, observe that paint remains pending, then prove one later accepted non-empty presentation. Multiple invalidations coalesce without either losing the last request or drawing after close.
- TestPlatform and the minimal real-HWND harness cover callbacks arriving synchronously during create, show, mutation dispatch, activation, paint, and close rather than assuming every backend defers. Reserved-window events deliver after commit and retire after rollback; synchronous close after commit follows normal ordered teardown.
- Dock tests reproduce the registered tear-off commit-error close-observer path, source close, activation callback, sibling reconciliation, and surface sink reentry. They prove native effects execute with no runtime borrow held and stale finalize work cannot mutate a replacement generation.
- Cross-backend and web compile/adapter tests prove every existing asynchronous callback enters the typed authority, synchronous queries use the declared snapshot/fallback contract, and no owning backend retains log-and-drop behavior.

**Deletion/replacement**

- Delete callback-local `.log_err()` handling that discards a platform event, generic callback diagnostics without event identity, and backend-specific retry tails superseded by the mailbox.
- Delete Dock paths that hold a runtime `RefMut` across controller/entity/window updates or open/show/close/remove/activate/native-mutation effects.
- Do not turn the mailbox into a public arbitrary-event queue, serialize user input to disk, retry an event without generation checks, or coalesce ordered terminal/input facts merely for throughput.

**Unit gate**

- GPUI ingress, synchronous-query, reserved-window commit/rollback, TestPlatform synchronous-callback, minimal real-HWND, Dock reentrancy, native-adapter, docs, and public-surface tests pass.
- Review confirms AppCell ownership, global ingress order, explicit query/merge/barrier semantics, exact handler-derived results and zero busy entrances for every must-immediate hybrid class, bounded fair draining, closed command FIFO order/non-recursion/terminal validation, full-`WindowId` isolation, commit/rollback settlement, accepted-or-reinvalidated frame behavior, no callback loss, no native side effect under an entity/controller/runtime borrow, and no pump-sensitive command under the outer App borrow.

### U25. Separate Window Appearance, Activation, And Ownership

**Outcome**

GPUI models creation-time appearance, lifetime activation/input policy, and native owner/transient relationships as independent facts. A detached viewport can appear without stealing focus, present its first frame, and later activate normally; a provisional viewport has the same final native lifetime capability but remains session-suppressed until same-window promotion.

**Requirements**

- R1-R2, R15, R25, and R27.

**Primary files**

- `crates/gpui/src/platform.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/window_platform_mutation.rs`
- `crates/gpui/src/platform/test/window.rs`
- window creation and mutation adapters in `crates/gpui_windows/`, `crates/gpui_macos/`, and `crates/gpui_linux/`
- `crates/gpui_web/` window contract projection
- `crates/gpui_docking/src/viewport_runtime.rs`
- `crates/gpui_docking/src/viewport_activation.rs`
- `crates/gpui_docking/src/viewport_platform_sync.rs`
- window option, platform-fact, activation, ownership, and native integration tests
- window lifecycle ADR and breaking migration documentation

**Behavioral work**

- Replace the overloaded creation `focus` meaning with final typed vocabulary: creation-only `focus_on_appearing`; lifetime `accepts_activation`, `focus_on_click`, and `accepts_pointer_input`; plus `transient_for`. Permanent non-activation is explicit and limited to window kinds or options that request it. `accepts_activation` and `focus_on_click` remain independent fields in U20's one coherent `ActivationPolicy` mutation domain, sharing generation and terminal observation without aliasing or partial commit.
- Keep `focus_on_appearing` creation-only. Delete `WindowMutationDomain::FocusOnAppearing`, `WindowMutationRequest::FocusOnAppearing`, `request_focus_on_appearing`, and equivalent backend live setters. Initial no-activate show cannot install permanent no-activate native style or disable later click focus. Existing public facts and options migrate atomically; no compatibility boolean remains to recreate the ambiguity.
- Add a typed top-level owner/transient relationship from window options through resolved creation params and owning backends. It references one live owner generation, rejects self/closed/foreign-application owners, and reports unsupported behavior rather than guessing. Windows uses top-level owner semantics rather than `WS_CHILD`; macOS and Linux project their supported child/group/transient relationships; Wayland limitations remain explicit.
- Treat native owner semantics as z-order, activation, minimization/grouping assistance only. Closing the owner does not satisfy R28 by itself, and a backend without owner support does not weaken DockSurface's explicit teardown.
- Define staged appearance facts for windows that must not expose an empty shell: native window created without activation, root/callbacks installed, first frame accepted and presented, then shown under the requested appearance policy. Backends that must show earlier still expose presentation generation separately, and no caller may equate `IsWindowVisible` with non-empty presentation.
- Define the ordinary detached Dock native profile as no initial activation, later programmatic/click activation, pointer input accepted, and current-surface ownership. Create the provisional with that same final lifetime native profile; while the source owns capture, a Dock session gate suppresses its GPUI input, focus, activation, route eligibility, and accessibility participation. U27's final atomic promotion transaction removes that gate after fallible preflight and does not depend on a live native activation/input transition. Optional taskbar/topmost changes remain honest U20 capabilities and cannot block correctness.
- Keep platform facts coherent with U20: initial appearance is immutable creation history, lifetime flags use their own live or creation-only capabilities, owner relationship is observed or capability-qualified, and public getters never infer facts from requested options alone.

**Test scenarios**

- Contract tests prove every combination of initial appearance, later activation, click focus, pointer input, and permanent non-activation without deriving one fact from another, including coherent generation and no-partial-commit behavior for the two-field activation policy.
- Windows native tests show an ordinary detached viewport without foreground theft, assert absence of permanent no-activate style, then activate it by click and programmatic request. A deliberately permanent non-activating window remains non-activating.
- Owner tests cover valid owner, closed/stale/self owner rejection, owner generation replacement, same-process top-level relationship, z-order/activation behavior where supported, and honest unsupported capability on other backends.
- Presentation tests distinguish native creation, visibility, first accepted frame, submitted present, and first non-empty presentation; a deferred frame or renderer/device/surface failure cannot be treated as presented and terminally settles pending opening work.
- Provisional tests cover final native lifetime capability plus session-level suppression, exclusion from hovered-window and Dock route targeting, same-window promotion without native profile mutation, optional unsupported taskbar/topmost changes, and no intermediate focus or accessibility ownership.
- TestPlatform, web, and every owning backend compile against the new option/param/fact contract; web and unsupported backends project ownership/activation limitations honestly, and migration scans prove the old overloaded `focus` path is absent.

**Deletion/replacement**

- Delete the overloaded creation-focus boolean and Windows mapping from no-initial-focus to permanent `WS_EX_NOACTIVATE`.
- Delete implicit "current active window" owner selection and Dialog-only owner special cases that bypass the typed relationship.
- Do not promise that native ownership cascades lifecycle, expose raw HWND/NSWindow/X11 handles, use `focus_on_click` as an alias for lifetime activation capability, or make correctness depend on a creation-only flag changing live.

**Unit gate**

- GPUI option/fact, TestPlatform, Dock profile, native appearance/activation/owner, migration, and documentation tests pass.
- Review confirms non-stealing initial show plus later activation, explicit permanent non-activation, owner generation safety, first-presentation observability, same-window session promotion, and honest optional per-backend flag capability.

### U26. Add DockSurface Window Sessions And Deterministic Teardown

**Outcome**

Each facade-managed `DockSurface` owns one explicit anchor session and shutdown authority, while the viewport runtime remains the sole registry of committed and provisional handles for that session. Anchor close freezes and drains only that surface through an idempotent, borrow-safe shutdown sequence; another DockSurface remains independent.

**Requirements**

- R1-R2, R15, R24, R26-R28.

**Primary files**

- `crates/gpui_docking/src/surface.rs`
- `crates/gpui_docking/src/surface/owner.rs`
- `crates/gpui_docking/src/surface/viewport.rs`
- `crates/gpui_docking/src/viewport_window_ownership.rs`
- `crates/gpui_docking/src/viewport_runtime.rs`
- `crates/gpui_docking/src/viewport_runtime_handle.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/close_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_effects.rs`
- `crates/gpui_docking/src/surface_tests.rs`
- `crates/gpui_docking/src/surface_owner_tests.rs`
- `crates/gpui_docking/src/host_viewport_close_tests.rs`
- `crates/gpui_docking/src/host_viewport_lifecycle_tests.rs`
- `examples/docking-native/` and `examples/docking-multiviewport/`
- Dock facade, lifecycle ADR, migration, and verification documentation

**Behavioral work**

- Extend the private surface owner only with session phase, unique session generation, optional opening/active anchor token or handle, and shutdown cause. The generation-tagged viewport runtime registry remains the single storage authority for committed and provisional window handles; the owner snapshots it for teardown rather than mirroring it.
- Make `open_primary_window` reserve an `Opening` anchor token before applying the U24 typed native-create effect through the current App transaction. Synchronous `App::open_window` reserves and commits the full `WindowId` before returning; afterward a short owner borrow validates that ID and opening token, then transitions to `Active`. Native callbacks raised during creation remain in AppCell ingress until commit or rollback and can settle failure/close afterward, but never activate the session. Creation failure, rollback, synchronous close, App shutdown, or cancellation settles without viewport admission. A second opening or active anchor returns a typed conflict. Low-level `host_view` and independently constructed runtimes do not implicitly become facade anchors or acquire facade teardown policy.
- Permit an explicit later `open_primary_window` only after the prior session reaches `Closed`, creating a new generation. `ShuttingDown` reaches `Closed` only when the current-generation runtime registry is empty and every snapshotted native window has produced terminal close observation or is confirmed absent; reopen while any old HWND remains unresolved returns a typed not-closed result. Delayed close, observer, activation, drag, platform observation, or native-event work from an older generation cannot join or shut down the replacement.
- Only `Active` accepts committed/provisional viewport work. On current-anchor close, transition to `ShuttingDown` before invoking any external callback. Freeze new viewport open, activation, drag, route, mutation, and registration work; cancel active drag, polling, prepared/provisional tear-off, pending promotion, and focus restoration; atomically retire surface mappings; then snapshot the viewport runtime's current-generation handles and typed close effects.
- Release every owner/controller/runtime/entity borrow before applying close effects. The forced shutdown path bypasses per-viewport `Prevent` and `MergeBack`, suppresses normal merge-back graph mutation and focus restore, tolerates windows already destroyed by the OS, and lets each close observer converge idempotently.
- Close only windows owned by the current surface generation. Multiple surfaces, low-level independent runtimes, ordinary application windows, and a later surface generation are untouched. Dock never calls `cx.quit`; GPUI's normal last-window rule decides application exit.
- Preserve U19 revision semantics. Entering shutdown, cancelling preview-only work, and dispatching close intent do not fabricate committed layout revisions. Any durable graph cleanup caused by the authoritative session transition publishes once under a typed shutdown transaction, and repeated/late observers publish nothing.
- Integrate U25 owner/transient relationship for committed and provisional viewports where supported while retaining the viewport runtime registry as the sole cross-platform handle authority.

**Test scenarios**

- Session tests cover `Opening` reservation, synchronous `App::open_window` registry commit followed by token/`WindowId` activation, native create success, creation rollback, callbacks queued during creation, synchronous close during creation, duplicate opening/active-primary conflict, App shutdown while opening, explicit reopen with a new generation, stale old-anchor close, stale viewport callback, and registration attempts before `Active` or during `ShuttingDown`/`Closed`.
- Shutdown tests create multiple detached and provisional windows, begin drag/activation/mutation work, close the anchor, and prove freeze-before-close ordering, borrow-free native effects, exactly-once cancellation, complete ownership retirement, no focus restore, and idempotent repeated/late close.
- Policy tests prove ordinary child-window close still honors `Prevent`, `MergeBack`, and close-request semantics, while anchor shutdown force-closes those same windows without merging them back or being blocked.
- Isolation tests run two Dock surfaces plus an independent low-level runtime; closing one anchor leaves the other surface's anchor, viewports, drag route, revisions, and activation usable.
- Failure-order tests cover the OS destroying a child first, anchor close during provisional creation or promotion, tear-off commit failure, close observer reentry, callback-opened work, delayed terminal observation for one old HWND, rejected reopen while the registry or close observations remain incomplete, and successful reopen only after deterministic convergence.
- Example tests close the real primary through the facade and assert owned viewport convergence without relying on example-level `cx.quit`.

**Deletion/replacement**

- Delete window ownership represented only by unscoped `WindowId` sets, any surface-owner mirror of the viewport runtime handle registry, activation-only close cleanup, and tests that require detached facade viewports to survive the current primary by default.
- Delete example-specific primary-close `cx.quit` behavior used to mask absent surface teardown.
- Do not make all low-level Dock runtimes application-global, infer an anchor from the first rendered host, or delegate forced shutdown to per-viewport close policy.

**Unit gate**

- Surface owner/session, viewport close/lifecycle, reentrancy, example, public API, migration, and documentation tests pass.
- Review confirms non-overlapping state ownership, synchronous registry-commit activation of opening tokens, explicit reopen only after native-window terminal convergence, freeze/snapshot/release/apply shutdown ordering, forced-policy behavior, multi-surface isolation, commit-only revisions, and absence of Dock-owned application exit.

### U27. Add Native Cross-Window Drag Transport And Live Tear-Off

**Outcome**

A source HWND that owns native pointer capture transports one Dock drag generation across application windows using current screen-space facts. A real provisional viewport with one presentation lease appears and presents before release, while durable Dock topology changes only after a revalidated release outcome.

**Requirements**

- R1-R2, R15, R26-R30.

**Primary files**

- GPUI active-drag and pointer-session code in `crates/gpui/src/app.rs`, `crates/gpui/src/window.rs`, and input dispatch modules
- `crates/gpui/src/app/test_context/pointer_session_tests.rs`
- `crates/gpui_windows/src/events.rs`
- `crates/gpui_docking/src/interaction.rs`
- `crates/gpui_docking/src/drop_runtime.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/host_interaction_outcome.rs`
- `crates/gpui_docking/src/host_render_actions.rs`
- `crates/gpui_docking/src/host_outside_release.rs`
- `crates/gpui_docking/src/host_viewport_drop.rs`
- `crates/gpui_docking/src/viewport_target_context.rs`
- `crates/gpui_docking/src/viewport_tear_off.rs`
- `crates/gpui_docking/src/viewport_tear_off_move.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/route_ops.rs`
- Dock host interaction, route, tear-off, render, and lifecycle tests
- `examples/docking-native/` and `examples/docking-multiviewport/`

**Behavioral work**

- Extend GPUI's active-drag boundary with a typed captured-pointer transport. The source capture owner samples the current screen point and an ordered application-window underlay stack from current native z-order plus committed window geometry, preserves source button/cancel ordering, and delivers a generation-bound drag fact to the registered consumer. The stack explicitly skips the visible provisional/session-suppressed window that may cover the pointer. The transport does not inject arbitrary raw GPUI input into another window or transfer native capture.
- Resolve the target fresh from each full `WindowId` candidate, scale factor, current screen-to-window conversion, current committed host-scene generation, Dock surface/session generation, and input eligibility. Skip provisional and session-suppressed/non-input windows, ordinary non-Dock windows, closed/replaced registrations, and stale local coordinates while continuing through the underlay stack. Resolve a registered host from another Dock surface as `ForbiddenTarget`, not absence: that host owns a rejected preview, and release cancels/restores rather than falling through to desktop tear-off.
- Let the target host consume routed preview facts through Dock's existing interaction authority without assuming its HWND received `MouseMove`. Source and target redraw scheduling use U24; only one route owns the current preview, and stale target callbacks cannot restore an old guide.
- Treat `MouseUp`, Escape, `PointerCanceled`, native capture change/cancel mode, source deactivation, source close, anchor shutdown, and drag-generation replacement as terminal inputs to one idempotent cancellation/release state machine. Terminal handling clears preview, active drag, capture, polling task, presentation lease, and provisional viewport exactly once.
- Add one private `DockLiveUndockSession` as the sole owner of current route, provisional handle reference, panel presentation lease, and release/compensation saga. GPUI active drag owns only generic capture transport; the DockSurface owner owns session phase/anchor/shutdown; the viewport runtime registry remains the sole handle store. Existing interaction, outside-release, and tear-off states are folded into this session rather than retained as parallel state machines.
- Advance that retained live-undock session from prepared payload through provisional opening, first presentation, visible movement, promotion/transfer, committed, cancelled, and superseded states. One drag generation may own at most one provisional viewport; repeated movement repositions it through U20 rather than opening another.
- Let Dock interaction/controller state exclusively acquire and transfer the presentation lease. The committed graph remains unchanged, the source retains one stable tab semantic/focus proxy rather than a duplicate panel subtree, and the provisional renders the real session-bound content while its Dock session gate suppresses focus, input, activation, route eligibility, and accessibility. The proxy preserves one semantic owner and the pre-drag focused descendant identity without claiming that the hidden source still presents the full panel. GPUI reports frame/presentation facts but does not understand the panel lease. If creation, renderer/device/surface setup, or first presentation fails, the lease stays/restores at the source and desktop release returns a typed unavailable result.
- Make provisional visibility require both native show and a non-empty presentation generation. Its explicit presentation states are: `Opening`, where the source snapshot/proxy remains authoritative and no empty native shell is shown; `DesktopVisible`, where one non-empty provisional follows the pointer; `HostPreview`, where the same HWND is retained but hidden and only the valid target host draws its guide; `Rejected`, where the provisional is hidden and the forbidden host draws rejected feedback; `Unavailable`, where the live source remains authoritative; and terminal `Commit`, `Cancel`, or `Failure`, which atomically hand off or restore ownership and destroy/retain no extra window. The provisional has the final detached window's lifetime native capabilities already present, belongs to the current U26 session, remains session-suppressed and ineligible as a drop target, and is reused if the pointer leaves a host again.
- Define the release decision table:
  - a current valid target in another viewport completes all fallible target/native preflight while the provisional stays suppressed, then one App transaction transfers presentation, commits the cross-window Dock transaction and semantic/focus ownership, removes the gate, destroys the provisional, and activates the target host;
  - a current valid source-host target performs the normal local drop or no-op, restores source presentation plus the valid pre-drag descendant focus, then destroys the provisional;
  - desktop release completes all fallible promotion/native work while the already-presented window stays suppressed, then one App transaction atomically commits graph/registration/presentation/semantic ownership and removes the gate before activating the new viewport;
  - a target that closed, changed generation, moved, changed DPI, or became invalid is resolved again from the current screen point and becomes a current target, desktop tear-off, or cancellation; the last preview is never release authority;
  - a foreign-surface host is a forbidden target that retains rejected feedback and cancels/restores on release rather than silently using a prior accepted route or desktop promotion;
  - source deactivation, source/anchor shutdown, or a dead source cancels without focus restoration or any attempt to reattach presentation to the dead generation.
- Execute release as one generation-bound compensating saga. Prepare the graph transaction and complete every fallible native effect or promotion preflight while the provisional remains session-suppressed. Bind input, focus, and accessibility work arriving during that interval to the promotion generation. One final App transaction atomically commits graph ownership, viewport registration, presentation lease, semantic/focus ownership, and gate removal; queued work then drains against the committed destination or retires after rollback. If preflight or commit fails, compensation restores the exact live source lease and destroys the provisional without exposing an interactive half-promoted window. U19 publishes one revision only after final commit, while opening, movement, preview, and compensation remain non-durable.
- Define focus and accessibility handoff by release outcome. During drag, the source semantic/focus proxy remains the only AccessKit owner. Desktop promotion and cross-window transfer restore the previously focused live panel descendant through the committed destination's U19 activation host after durable commit; local return and cancellation restore it at the live source. Source deactivation, source close, or anchor shutdown never requests activation or steals focus. Every terminal path removes the proxy and leaves exactly one final AccessKit subtree.

**Test scenarios**

- Captured-routing tests inject movement and release only into the source window while target raw input remains absent. The ordered underlay resolver skips a visible provisional and finds the current same-surface target, which receives preview and one release result with correct DPI/coordinate conversion.
- Route eligibility tests cover target resize/move/DPI change, host-scene generation replacement, target close just before release, foreign-surface forbidden preview/cancellation, provisional and non-input windows, non-Dock underlays, stale local points, and last-preview rejection. Host-to-desktop-to-host movement proves the same provisional never obscures the restored host route.
- Closing only the current target immediately clears that target's preview and pending work while retaining the source-owned drag; the next movement or release resolves again from the current screen point.
- Terminal tests cover Escape, pointer cancel, native capture loss, source deactivation, source or anchor close, replacement drag generation, repeated terminal callbacks, and poll-task races; each leaves no capture, preview, lease, pending task, or provisional window.
- Provisional tests cover every `Opening`/`DesktopVisible`/`HostPreview`/`Rejected`/`Unavailable`/terminal transition, one-window reuse across repeated motion and target transitions, no empty shell, final lifetime native capability plus active session suppression, native visibility plus non-empty first presentation before `MouseUp`, live position updates, one source semantic/focus proxy, no duplicate focus/accessibility/input ownership, and no early graph revision.
- Release-table tests cover other host, source host, same-window desktop promotion, foreign-surface forbidden target, invalid target, stale target re-resolution, optional native flag failure, provisional creation/renderer/surface/first-presentation failure, promotion preflight failure, graph commit failure, queued input/focus/accessibility during delayed promotion, compensation failure handling, and rollback to the exact source generation. Each success and cancellation path asserts final activation completion, focused descendant, one AccessKit owner, and proxy removal; source deactivation/shutdown asserts no focus steal.
- Multi-surface and shutdown tests prove route/session isolation and atomic cleanup when the anchor closes during any provisional state.
- Existing direct-target VisualTest flows are rewritten or relabeled so they remain model-level tests and no longer claim to reproduce Win32 capture transport.

**Deletion/replacement**

- Delete the assumption that a target HWND receives raw mouse movement under source capture, the outside-release polling path as route authority, and release-only viewport creation.
- Delete duplicate source-window and provisional semantic presentations, parallel outside-release/tear-off/interaction route state superseded by `DockLiveUndockSession`, last-valid-preview release fallback, hovered-top-window-only routing that cannot see through a provisional, and any provisional window not owned by the current surface/drag generation.
- Do not expose a general cross-window event injection API, copy ImGui's global moving-window context, commit the durable graph merely to obtain a drag visual, or transfer native capture between HWNDs.

**Unit gate**

- GPUI pointer-session, Dock interaction/route/tear-off/lifecycle, multi-surface, example, migration, and documentation tests pass.
- Review confirms non-overlapping state ownership, capture-owner transport, z-ordered underlay resolution through the provisional, foreign-surface rejection, complete terminal cleanup, one presentation lease and semantic/focus proxy, explicit presentation states, visible non-empty pre-release viewport, gate-preserving preflight plus atomic final handoff, exhaustive release/focus/accessibility outcomes, graph atomicity, and no duplicate semantic owner.

### U28. Prove Owning-Platform Multi-Viewport Behavior

**Outcome**

The release gate distinguishes model simulation from real native-window behavior. Deterministic Windows integration and subprocess scenarios exercise actual HWND creation, capture, nested message dispatch, first presentation, activation, ownership, cross-window drag, provisional teardown, and process/window convergence.

**Requirements**

- R13, R15, and R26-R30.

**Primary files**

- native integration support and tests in `crates/gpui_windows/src/platform.rs` and `crates/gpui_windows/src/events.rs`, or a dedicated Windows integration target in that crate
- GPUI test diagnostics for native-event and presentation generations
- `crates/gpui_wgpu/` presentation outcome hooks and cross-backend compile fixtures
- `crates/gpui_web/` contract compile coverage
- `crates/gpui_docking/` native-integration fixtures
- `examples/docking-native/`
- `examples/docking-multiviewport/`
- `.github/workflows/verify.yml`
- `docs/verification.md`
- Dock verification, ADR, migration, and release documentation

**Behavioral work**

- Extend U24's `gpui_windows`-owned real-HWND support into a deterministic full harness around the Windows application, message pump, WndProc, renderer/present path, and worker subprocess. `gpui_docking` supplies backend-neutral fixtures but does not own raw-HWND infrastructure. Test-only observation and input-control seams cannot replace HWNDs with `TestWindow`, call Dock handlers directly, or inject target-window events that production capture would prevent.
- Expose typed, metadata-only test observations for callback envelope/disposition, window and session generation, capture owner, first accepted frame, first non-empty presentation, native visibility, activation, owner relationship, and terminal shutdown. Assertions use those facts rather than parsing one generic log or treating visibility as paint.
- Exercise accepted draw, submitted present, non-empty presentation, renderer/device/surface failure, and native visibility as distinct facts. Opening/provisional work must terminally compensate when presentation cannot complete.
- Exercise create/show/position/activate/close callbacks while another GPUI update owns `App`, including nested Windows message dispatch. Prove queued replay, ordering barriers, no dropped mutation terminal, no runtime double borrow, and eventual first presentation.
- Drive two real HWNDs with capture retained by the source. Before release, assert the target Dock preview commits a presentation generation without target raw mouse input. Outside every host, assert the provisional HWND is visible, contains a non-empty presented frame, follows the pointer, has the final native lifetime profile, and remains Dock-session-suppressed for input/focus/activation/routing/accessibility.
- Add at least one Windows scenario that uses system-level pointer injection at screen coordinates and never selects the receiving HWND. At down, cross-window move, and up, assert `GetCapture`, the actual WndProc recipient, the source-only raw move/up stream, and final capture release; retain the negative assertion that the target HWND receives no raw move/up. Directly sending those messages to the source remains useful deterministic coverage but cannot satisfy this routing claim.
- Cover the full release and cancellation matrix through actual native messages: valid cross-window drop, desktop promotion, source return, Escape, capture loss, target-close re-resolution, source or anchor terminal close, provisional creation or presentation failure, commit failure, repeated close, and stale generation. Bound every worker with a timeout and guaranteed process-level cleanup so failures cannot leave CI windows/processes alive.
- Verify ordinary detached creation does not steal foreground focus but later click and programmatic activation work. Assert native owner/top-level relationship where Windows supports it, while separately proving explicit surface teardown.
- Start two Dock surfaces in one process, close one real anchor during active/provisional work, and assert all and only its HWNDs disappear. Close the remaining anchor and assert the application reaches its normal last-window/process termination condition without example-level `cx.quit`.
- Keep TestPlatform/Visual tests as fast graph, route, and presentation-model evidence. Rename CI jobs and documentation that call them native rendered or end-to-end tests unless they execute the real owning-platform harness.
- Add compile/capability gates for the U25 appearance/owner and U24 ingress/commit-boundary contracts on macOS, Linux, and web. Platform-specific real drag scenarios are required when a desktop backend advertises equivalent native capture and window capabilities; unsupported relationships are documented rather than imitated.

**Test scenarios**

- Real callback-reentrancy tests cover every U24 event class, merge/barrier order, close during queued work, a reused window generation, and first paint after a deferred frame.
- Real two-HWND tests cover OS-routed source-only captured move/up through system pointer injection, actual recipient and `GetCapture` observations, underlay resolution through a visible provisional, target preview before release, provisional presentation and every display state before release, coordinate/DPI changes, activation/focus/AccessKit handoff after no-focus appearance, and owner/z-order facts.
- Real failure tests cover capture cancel/deactivation, source/target/anchor destruction, failed provisional first presentation, failed promotion/commit, runtime close-observer reentry, and repeated/late close with no graph corruption or leaked HWND.
- Real surface tests cover `Prevent`/`MergeBack` under ordinary close, forced bypass under anchor shutdown, two-surface isolation, delayed child terminal-close observation, rejected reopen while any old HWND remains live or registered, explicit reopen generation after convergence, absence of overlapping old/new generation HWNDs, and final process convergence.
- CI-negative fixtures prove a VisualTest-only target cannot satisfy the native gate and that missing presentation/event observations fail with the exact window/session/event domain.

**Deletion/replacement**

- Delete or rename claims that direct `VisualTestContext` event injection is native Dock end-to-end coverage.
- Delete example-only quit behavior, manual-only acceptance credit, and log-string-only checks superseded by typed native observations.
- Do not require pixel-perfect screenshots for correctness, depend on a human moving the pointer, or let timeout/retry hide deterministic event loss.

**Unit gate**

- Windows real-HWND integration, Dock subprocess, fast simulated suites, cross-platform compile/capability, CI, verification docs, ADR, migration, and release gates pass.
- Review confirms the tests cross the actual WndProc/capture/present/lifetime boundaries, every reported regression has a native negative-then-positive scenario, simulated evidence is labeled honestly, and no test worker can leak a process or HWND after failure.

### Post-U28 Candidate Roadmap

The following labels are planning handles for separate design/implementation epics. They do not
add requirements, acceptance credit, or Definition-of-Done work to this convergence plan, and U28
must not publish placeholder APIs for them.

- **Candidate U21: Locale and logical direction authority.** Design separate immutable
  `LocaleContext` and inherited `LayoutDirection` facts with app/OS fallback, window override, and
  subtree override. Add logical start/end edges without reinterpreting physical left/right APIs,
  then carry the authority through layout, overlays, keyboard navigation, icon mirroring, shaping
  and bidi, selection/IME, AccessKit, portals, deferred work, and cache replay. The design gate
  must inventory existing direction-shaped APIs first; an `rtl: bool` shortcut is rejected.
- **Candidate U22: Input-independent semantic move sessions.** Extract only the stable identity,
  source snapshot/generation, accepted target, preview/commit agreement, cancellation, and
  accessibility/programmatic move outcomes shared by Tree, Table, and Dock. Pointer, keyboard,
  AccessKit, and programmatic entry paths adapt into the same domain session, while each component
  keeps its own hierarchy, row model, graph transaction, rendering, and focus policy. TanStack
  Table and Dear ImGui remain semantic references for Table and Dock respectively, not runtime
  dependencies or a reason to merge their models.
- **Candidate U23: Unified pointer-contact substrate.** Add stable per-contact identity and device
  kind, down/move/up/cancel, pressure/buttons/primary facts, coalescing policy, per-pointer capture,
  deterministic lost-capture delivery, backend parity, and a deterministic multi-contact test
  injector. Existing mouse listeners and drag/drop receive an explicit compatibility projection.
  A gesture arena remains a later decision after this substrate proves nested scroll/drag,
  transformed coordinates, suppression, and window-lifecycle behavior.

## Verification Contract

Verification is layered. A lower layer cannot substitute for a higher authority claim.

1. **Pure/domain tests:** form generations/status, overlay stack policy, focus ordering, token/schema, typeahead session, table characterization, transform validation/composition/inversion, presentation lattice, live semantics, reveal alignment, rounded-clip containment, Dock style completeness, surface revision categories, coherent two-field activation-policy mutation, window-mutation capabilities/outcomes, mailbox ordering/merge/barriers, Dock window-session generations and terminal convergence, release decisions, foreign-surface rejection, and provisional presentation-state transitions.
2. **GPUI runtime tests:** real input dispatch, exact must-immediate hybrid native dispositions with zero busy entrances, focus traversal, per-window isolation, controlled lifecycle, final accessibility updates/actions/announcements, deferred theme capture, transformed scene/input/IME/cache behavior, dynamic hidden/inert cleanup, anchor binding, nested reveal, exact clipped hit testing, Dock activation completion, window request-versus-observation ordering, already-borrowed asynchronous callback replay, source-only captured drag transport, ordered underlay resolution, presentation leases, semantic/focus proxy handoff, and borrow-free native effects.
3. **Projection tests:** UI state, every transformed and clipped scene primitive, final AccessKit tree/bounds/actions/live facts, allowlisted DevTools summaries, inspector geometry, Dock style/event/readiness projections, native callback/presentation generations, federated contract/Gallery/scenario/public-API bindings, and redaction agree.
4. **Gallery and example flows:** representative user journeys run through actual component adapters; U12-U17 exercise transformed and visible/inert/hidden subtrees, live regions, typed anchors, nested reveal, and rounded clips through pointer, keyboard, scrolling, text input, deferred content, AccessKit, and inspector paths. U18-U20 and U24-U28 use the Dock examples for scoped styling, events/snapshot export, stable-item activation, multi-viewport mutation, capture-owned drag, pre-release provisional presentation, and anchor teardown because the foundation Gallery intentionally excludes a Docking dependency.
5. **Owning-platform integration:** real native windows, message dispatch, system-level pointer injection, capture ownership and actual WndProc recipients, presentation, activation/focus/accessibility handoff, ownership, terminal close observation, and process/window teardown prove claims that TestPlatform and visual injection cannot exercise. Fast model tests remain required but provide no native credit.
6. **Workspace/release gates:** formatting, checks, nextest, docs, xtask scanners, dependency/import boundaries, supported-platform renderer compile/ABI/render smoke, native window-mutation and multi-viewport integration tests, and release verification.

Focused commands are run per unit using the packages and test targets named above. After U28 and its review checkpoint, the deterministic local final gate is:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo nextest run --workspace --no-fail-fast --locked

cargo nextest run -p open-gpui --features test-support,inspector --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked

cargo test -p open-gpui -p open-gpui-docking -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-form -p open-gpui-devtools --doc --locked
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
cargo run -p xtask -- scan-ui-contract
cargo run -p xtask -- scan-public-api --check
cargo run -p xtask -- scan-import-boundary
cargo run -p xtask -- scan-doc-links
cargo run -p xtask -- verify-release-docs
cargo run -p xtask -- dependency-health
cargo run -p xtask -- verify
git diff --check
```

The local command block is necessary but not sufficient for U12, U17, U20, or U28. CI must also compile each supported native renderer and platform window backend on its owning platform, run transform and clip primitive conversion/ABI tests, run the designated render-pixel smokes on capable runners, prove every advertised live window mutation from observed native facts, and execute the real-window multi-viewport scenarios where supported. Those jobs are part of the final gate even though no single developer platform can execute the whole matrix.

Test execution rules:

- Use fake clocks for typeahead, transform motion/intermediate/final state, debounce, and validation timing; correctness tests cannot use sleeps.
- Do not run competing full-workspace Cargo gates from multiple agents against the shared target directory.
- Use package-focused nextest during implementation and one final workspace run.
- On Windows, serialize resource-heavy GPUI inspector, all-feature DevTools, and link steps with `CARGO_BUILD_JOBS=1`.
- Correctness tests use no retries. Any introduced nextest timeout/group configuration must preserve fast unit-test parallelism and isolate only native/GPU/singleton tests.
- Platform-specific visual baselines are not a completion claim for this plan; structural/gallery/runtime assertions must pass on the active platform.
- Transform correctness requires backend-neutral primitive assertions plus native-backend conversion/ABI coverage. Unsupported local input must fail at construction, and an unrepresentable nested composition must fail-close the subtree transactionally; identity/clamp fallback, dropped primitive transforms, partial-channel output, and active-platform-only evidence fail the gate.
- Presentation correctness is asserted at the committed frame boundary. Tests must cover stale hitboxes, pointer capture, focus/IME, deferred/cache replay, portals, and final AccessKit membership rather than checking only style values or paint output.
- Announcement correctness is asserted through final `TreeUpdate` generations. Direct speech mocks, focus changes, component timers, or model-only descriptor assertions cannot replace committed semantic and privacy evidence.
- Anchor and reveal correctness uses window-owned generation-bound handles. Raw rectangle equality or a single-scroll-container test cannot prove unlink, cancellation, nested ancestry, transform conversion, or virtual materialization behavior.
- Rounded-clip correctness requires exact hit containment plus renderer conversion/ABI and capable-runner pixel evidence. A conservative AABB is allowed only for culling and accessibility bounds, never final paint or pointer coverage.
- Dock style correctness requires state-complete structural assertions, immutable per-surface/explicit-host resolver isolation and purity, source-opening versus target-host drag behavior, payload-identity separation, the `DockDropGuideMetrics` migration, and a source scan; pixel-perfect ImGui colors are not evidence and are not a goal.
- DockSurface correctness is asserted from explicit transaction identities, typed committed controller/observed-runtime events, unique activation-host generations, and exact focus completion. Generic notifications, App-turn coalescing, selection alone, mutation dispatch, a dropped completion subscription, snapshot diffing, or timer passage cannot claim an activation or persistence revision.
- Platform-window mutation correctness separates queued dispatch from terminal observation, treats state/geometry/restore bounds as one placement domain, verifies partial capabilities only across independent domains on each owning backend, and uses one committed fact cache for public getters, bounds, routes, and snapshots. A queued request cannot satisfy an observed-state assertion.
- Platform-event correctness is asserted from AppCell-owned typed ingress, application-wide sequence, explicit domain merge and FIFO/cross-window barrier rules, full `WindowId` plus actual domain generations, immutable synchronous-query behavior, exact handler-derived disposition plus zero App-busy entrances for every must-immediate hybrid input class, and accepted-or-reinvalidated frame state. A fixed default policy, delayed replay, generic log absence, executor flush, or final snapshot alone cannot prove an ordered input/terminal event was delivered correctly.
- Native-effect correctness requires entity/controller/runtime guards to be released before App-owned native work and requires the closed pump-sensitive platform-command set to execute only after the outer `AppRefMut` is released, in FIFO order, with weak-dispatcher terminal validation and no recursion. Reserved-window callbacks must wait for commit or retire on rollback. An asynchronous current backend implementation is not evidence that either borrow boundary is safe.
- Dock window-session correctness covers synchronous registry-commit activation of `Opening`, active anchor generations, one runtime handle registry, freeze-before-close, forced policy bypass, multi-surface isolation, stale callback rejection, `Closed` only after registry and native terminal convergence, and reopen only afterward. Native owner relationships or `cx.quit` cannot substitute for surface-scoped teardown.
- Native drag correctness requires OS-routed source-only captured input, actual capture/recipient observation, z-ordered underlay resolution through the provisional, current screen-to-target conversion, typed foreign-surface rejection, release-time revalidation, one live-undock session/presentation lease and source semantic/focus proxy, explicit presentation states, a visible non-empty provisional frame before `MouseUp`, fallible preflight under suppression, atomic graph/registry/lease/semantic/gate handoff, destination activation/focus completion, compensation, and complete terminal cleanup. Direct target event injection, direct source message injection alone, `IsWindowVisible` alone, or a release-created window cannot satisfy the claim.
- U28 native scenarios run without correctness retries. Timeouts are failure bounds and cleanup guards, never a retry policy; every failed worker must still terminate and destroy its HWNDs.
- Redaction tests use unique canary strings, including Table identity/debug-label/selector sources, and assert their absence from live capture, history, diff, Inspector detail/copy, session export, artifact, report, and Gallery fixtures. Post-hoc generic string sanitization does not satisfy this contract.

## Definition of Done

- U1-U20 and U24-U28 are implemented in dependency order. Candidate U21-U23 remain separate follow-on epics. Every unit's focused local tests pass before its commit; platform-owned CI evidence that requires a committed revision passes before that unit and the plan are declared complete.
- `FormStatus::Validating` is reachable from real store activity, stale validation cannot mutate newer state, and UI/DevTools projections agree.
- Every official overlay family uses the per-window runtime; old component-specific Escape/outside/focus tails and shallow host forwarding are gone. U3/U4 share this completion gate.
- Nested modal focus trap, controlled close, exit/reopen, callback reentrancy, trigger loss, LIFO restore, and multi-window isolation are proven with real GPUI tests.
- Every official component that emits accessibility semantics derives the unified projection from resolved state and has no parallel evidence/assembly authority. Representative action, form, choice, overlay, navigation, collection, and table families are asserted in final AccessKit trees with real action dispatch and stable node identity; all remaining producers have projection/absence coverage.
- Official semantic controls no longer expose legacy ClickEvent callbacks; semantic entry paths are role-correct, disabled-safe, and exactly once.
- Theme scope is proven on the existing snapshot before replacement. The sole complete Theme v1 contract, required color scale, every consumer-proven candidate scale, clean rejection of the deleted color-only shape, effective revision, window/subtree scope, deferred inheritance, and recipe consumption pass focused tests and scanners; unproven categories are explicitly deferred rather than stubbed.
- Tree and VirtualizedList no longer own duplicate typeahead buffer/timing implementations.
- Federated typed component rows, Gallery probes, native scenario IDs, and public owners replace manual API inventory, duplicate catalogs/maps, source parsing, and the residual empty a11y-evidence exports/scaffolding left by U5 wherever covered by U10, without recreating a central registry.
- Table keeps its engine/virtualizer ownership split while exact typed identities survive every row-model stage, pinning region, edit/focus path, and virtual recycle. U5's intentional typed-identity API and partial-column-order changes are documented and characterized. U10 preserves that completed post-U5 Table contract; only evidence-backed common/diagnostic export narrowing may add further public-surface breakage.
- The sole public interactive subtree transform accepts only finite positive normal axis-aligned scale with a representable inverse, finite translation, and explicit child-local origin; checked child-before-parent composition fails closed transactionally on numeric/backend conversion error, layout is invariant, every scene primitive and observable geometry channel agrees, nested/deferred/portal/cache/motion paths are covered, and supported renderer jobs pass. Generic or visual-only transform aliases are absent.
- The sole layout-preserving presentation authority implements the exact visible/inert/hidden matrix. Dynamic suppression removes stale paint/input/capture/focus/IME/accessibility state, descendant escape is impossible, transformed/deferred/cached paths agree, and old paint-only or a11y-only subtree authorities are absent.
- Declarative live regions and transient announcements use final committed AccessKit updates, remain focus-independent and window-isolated, handle repeated text and activation generations deterministically, and retain no message text in production diagnostics or DevTools artifacts. No component or native speech path competes with this authority.
- Typed portal anchors bind one target per window/frame, distinguish current candidates from committed snapshots, become explicitly unlinked on absence, Hidden state, unmount, or invalidity, preserve Inert as a linked snapshot fact for follower-specific eligibility, feed official overlays without raw live geometry, and expose no generic DOM-like node reference.
- Application, winning-focus, and AccessKit reveal requests share one committed inner-to-outer bring-into-view authority with explicit axes, transform-correct deltas, deterministic cancellation, reduced-motion behavior, and virtual materialize-then-reveal adapters.
- Rectangular and rounded-rect subtree clipping uses one exact paint/hit stack across transforms, presentation, deferred/cache replay, portals, debug, accessibility limits, and supported renderer ABIs. Arbitrary path and silent AABB/native-surface fallbacks are absent.
- Every Dock render path consumes one complete immutable style resolved by an immutable per-surface or explicit-host pure resolver. Hard-coded production palettes, stale cross-window/subtree style, UI Components reverse dependencies, generic/global style lookup, visual data in payload identity, and the misleading `DockDropGuideStyle` name are absent.
- Every facade-created Dock host and viewport belongs to one private DockSurface owner with explicit transaction boundaries, monotonic commit-only revisions, typed change events, and one generation-bound activation host per space. Independent commands in one App turn never coalesce; stable-item activation settles from exact focus completion; selection-only remains explicit; snapshot export is revision-consistent; and persistence debounce/I/O remains application-owned.
- GPUI exposes honest property capabilities, placement-conflict semantics, typed dispatch outcomes, and generation-bound terminal observations, while one committed `WindowPlatformFacts` authority backs public bounds/state/flag getters. Lifetime activation acceptance and click-focus are independent fields in one coherent no-partial-commit policy domain. Dock no longer carries an ambiguous `live_window_move`, optimistic applied records, or duplicate mutation authority, and owning-platform tests prove every advertised live domain.
- Native callbacks never disappear because `App` is already borrowed. Every asynchronous callback receives an application-wide sequence and enters the AppCell-owned typed ingress without overtaking backlog; ordered queue-eligible input and terminal facts remain non-droppable behind cancel/close barriers, coalescing is domain-generation-specific, frames are accepted or re-invalidated, and callback-specific diagnostics replace the generic `RefCell already borrowed` loss path. Synchronous native queries use the declared immutable snapshot/conservative contract without recursive App access. Every must-immediate hybrid input class returns its real handler-derived disposition from an App-idle entrance; a fixed fallback, delayed replay, or nonzero busy count fails the plan.
- GPUI and Dock apply App-owned typed window effects only after all entity/controller/runtime borrows are released, and execute the closed pump-sensitive command set only after the outer App borrow is released and older barriers settle. Reserved/current `WindowId` callbacks settle through the ingress after commit or rollback, and the registered tear-off commit-error close-observer path, sibling reconciliation, activation, mutation observation, and shutdown cannot reborrow the same runtime.
- Creation-time focus, lifetime activation/click/input, permanent non-activation, accepted/submitted/non-empty presentation, and owner/transient relationships are independent facts. An ordinary detached viewport does not steal initial focus but later activates; a provisional viewport starts with the final native lifetime profile but remains Dock-session-suppressed until same-window promotion; unsupported ownership and optional flags remain explicit.
- Every facade-managed DockSurface has an explicit `Opening` token, one current active anchor generation, and an idempotent window session. Synchronous window creation must return a committed full `WindowId` before a short token validation activates the session; creation callbacks cannot activate it. The surface owner stores phase/anchor/shutdown, the viewport runtime solely stores window handles, and opening failure or synchronous close settles before viewport admission. Anchor shutdown freezes new work, cancels provisional/pending work, force-closes only the current surface outside all borrows, bypasses `Prevent`/`MergeBack`, leaves other surfaces intact, reaches `Closed` only after registry/native terminal convergence, rejects premature reopen, and never relies on `cx.quit` or stale callbacks.
- Native captured Dock drag routes from the source using current screen-space and an ordered native/application underlay stack without target raw mouse delivery. Release revalidates the target, foreign-surface hosts are typed rejected targets rather than desktop fallback, all cancel/close paths clear exactly one drag generation, and stale/provisional targets cannot receive a drop.
- One Dock live-undock session exclusively owns route, provisional reference, presentation lease, source semantic/focus proxy, and compensation saga. Its explicit display states never expose an empty or duplicate shell, and its viewport becomes visibly non-empty before release while the graph/revision remain unchanged. Every fallible promotion step completes under session suppression; one final App transaction atomically commits graph, registry, lease, semantic/focus ownership, and gate removal before destination activation. Creation, renderer/surface, presentation, native preflight, graph commit, cancellation, or shutdown failure restores only a live source, never steals focus on deactivation/shutdown, and leaks no window or duplicate semantic owner.
- Windows real-HWND and subprocess gates cross WndProc, system-injected pointer routing, actual capture/recipient observation, paint/present, activation/focus/AccessKit handoff, owner, native terminal close, and process-lifetime boundaries. TestPlatform coverage is labeled as simulation and cannot claim native end-to-end credit.
- Action presentation and command execution remain separate, with no speculative replacement runtime.
- ADRs and breaking migration documentation match the shipped architecture; stale helpers, aliases, evidence, and docs are deleted.
- DevTools allowlist and canary tests prove that sensitive free text cannot enter or persist through capture, inspection, export, artifact, report, or Gallery paths.
- The complete Verification Contract passes, `git diff --check` is clean, review findings are resolved, and no user-authored unrelated change is reverted.

## Appendix

### Requirement Trace

| Requirement | Owning unit(s) | Completion evidence |
| --- | --- | --- |
| R1-R2 | all units; preservation gates in U10-U20 and U24-U28 | import/dependency scans and preserved-module focused tests |
| R3 | U1 | lifecycle table tests from store through UI/DevTools |
| R4 | U2 | final `TreeUpdate` capture and real action dispatch |
| R5 | U3, completed with U4 | real nested Tab/Shift-Tab and restore tests |
| R6 | U4 | pilot/fleet migration, runtime tests, old-tail absence |
| R7 | U5 | all-producer migration inventory plus representative final-tree tests |
| R8 | U6 | callback inventory, activation matrix tests, public `ClickEvent` absence gate |
| R9-R11 | U7-U8 | scope tests on old/new payload, schema/recipe scanners, deferred capture |
| R12 | U9 | fake-clock cross-collection tests and duplicate implementation deletion |
| R13 | U10 | federated binding fixtures and source-scanner deletion |
| R14 | U5, U10 | typed-identity/stage tests and post-U5 Table characterization through export cleanup |
| R15 | each breaking unit; U11 audits prior surfaces; U12-U20 and U24-U28 own their migrations | same-unit migration docs/Gallery/examples/DevTools updates and final residual scan |
| R16 | U5; preservation gates in U10/U11 | occurrence invalidation and explicit-instance focus/edit/callback/NodeId/measurement tests, normalized partial-order characterization, and Table redaction canaries |
| R17 | U12 | checked construction/inverse/composition and numeric fail-closed tests, layout invariance, all-primitive scene projection, inverse input/capture, IME/debug/a11y/deferred/cache/motion coverage, Gallery flow, and supported-renderer matrix |
| R18 | U13 | exact channel matrix, nested/dynamic suppression, stale-state cleanup, transformed/deferred/cache/portal coverage, final-tree absence, and old-authority deletion scans |
| R19 | U14 | descriptor mapping, final-tree live facts, queue generations/order/removal, window isolation, focus stability, privacy canaries, and native adapter gates |
| R20 | U15, U17 | current/committed binding order, typed unlink/errors, transformed/clip/presentation snapshots, rounded-stack AABB regression, official overlay migration, and stale-geometry absence |
| R21 | U16 | nested-axis alignment, focus/AccessKit/application parity, transform-correct deltas, generation cancellation, virtual materialization, and Gallery flow |
| R22 | U17 | exact rounded containment, all-primitive clip projection, cross-channel inheritance/reset/failure, accessibility limits, renderer ABI, and native pixel smokes |
| R23 | U18 | complete style/state lookup, immutable resolver isolation/purity, window/subtree and out-of-band drag-generation resolution, payload identity stability, guide-metrics rename, dependency and literal-color scans, Dock example smokes |
| R24 | U19, U26-U27 | explicit transaction-boundary/coalescing matrix, unique activation-host and window-session generations, commit-only revision/event matrix, stable-item activation completion, surface lifecycle, revision-consistent snapshot export, caller-owned fake-clock debounce, and provisional non-revision tests |
| R25 | U20, U24-U25 | capability plus dispatch/terminal-outcome matrix, coherent placement/restore-bounds conflicts, coherent two-field activation-policy domain, committed getter authority, Dock independent-domain partial sync, callback-safe observations, separate appearance/activation facts, native owning-platform tests, and old-capability/direct-backend absence scan |
| R26 | U24, U28 | AppCell-owned ingress, application sequence and merge/FIFO/barrier matrix, synchronous-query snapshots, exact App-idle hybrid input disposition with zero busy entrances, closed post-borrow command FIFO, already-borrowed asynchronous replay, full-`WindowId` commit/rollback isolation, borrow-free subordinate effects, accepted-or-reinvalidated frame tests, and real nested native-message evidence |
| R27 | U25, U28 | independent appearance/activation/input/owner contract, deletion of live focus-on-appear mutation, accepted/submitted/non-empty presentation generations, no-focus-then-activate native flow, owner capability tests, and same-window session promotion |
| R28 | U26, U28 | synchronous registry-commit activation of opening token, sole runtime handle registry, forced shutdown policy, borrow-free close snapshot, terminal native-window convergence before `Closed`/reopen, stale generations, multi-surface isolation, real HWND and process convergence |
| R29 | U27-U28 | OS-routed source-only captured move/up, actual capture/recipient facts, z-ordered underlay resolution through provisional windows, screen/DPI conversion, current same-surface host eligibility, foreign-surface rejection, release revalidation, cancel/close barriers, and real two-HWND preview/drop evidence |
| R30 | U27-U28 | one live-undock session, presentation lease, and semantic/focus proxy, explicit presentation states, non-empty pre-release viewport, fallible preflight under suppression, atomic graph/registry/lease/semantic/gate handoff, destination focus completion, exhaustive release/cancel/failure table, and leak-free native teardown |

### Priority Rationale

- **P0 correctness:** U1 and U2. They fix data corruption risk and make a critical user-facing authority observable.
- **P1 interaction/runtime:** U3-U6, U12-U17, U19-U20, and U24-U28. They resolve modal, focus, accessibility, activation, geometry, presentation, announcement, anchoring, reveal, clipping, Dock application ownership, platform-event loss, native window semantics, session teardown, and real multi-viewport drag with the largest user impact. The later units were discovered or promoted by substrate, real-consumer, and owning-platform audits; their execution position does not reduce their severity.
- **P2 framework depth:** U7-U9 and U18. Scoped resolution is proven before the complete Theme v1 replaces its payload; typeahead improves interaction consistency independently; Dock visual styling then consumes the established theme boundary without inverting dependencies.
- **P3 convergence/release:** U10-U11. They delete drift-prone scaffolding only after executable authorities can replace it.

### Deferred Follow-on Research

These candidates have no requirement IDs, acceptance credit, or Definition-of-Done credit in this
plan. Candidate U21-U23 are post-plan scheduling handles only; U12-U20 and U24-U28 must not add public
placeholders for them.

- **Group opacity and compositing:** determine isolation boundaries, offscreen target ownership, nested blend semantics, native-surface behavior, cache invalidation, GPU memory cost, and backend parity before exposing subtree opacity as more than a leaf paint property.
- **Locale and logical direction (candidate U21):** inventory locale fallback, inherited direction, physical versus logical edges, overlay placement, keyboard navigation, icon mirroring, shaping/bidi, selection/IME, AccessKit projection, and portal/cache capture before a dedicated design epic chooses public types. No `rtl: bool` substitute enters this plan.
- **Semantic move sessions (candidate U22):** inventory Tree, Table, and Dock reorder/move identities, source generations, accepted-target resolution, preview/commit consistency, cancellation, keyboard and AccessKit actions, focus, undo, and persistence boundaries before selecting shared renderer-neutral vocabulary. Do not share component models or input runtimes merely because they all support movement.
- **Arbitrary path subtree clipping:** after U17, determine fill rule/winding, path ownership, tessellation versus stencil, antialiasing, transformed containment, nesting limits, cache keys, native surfaces, and memory/performance policy before exposing any path variant.
- **Multi-pointer substrate (candidate U23) and gesture arena:** define pointer identity, cancellation, capture transfer, nested scroll/drag interaction, touch/pen parity, and deterministic test input first. Recognizer ownership and arbitration remain a later gesture-arena design after the substrate ships.

### Explicit Preservation Gates

- Table engine, `RowWindow`, stable identities, `core -> filtered -> grouped -> sorted -> expanded -> paginated -> final` pipeline, exact-identity pinning and explicit business-ID bulk semantics, pinning partition semantics, client/manual ownership, and separate virtualization stay; the legacy `TableRowId`-only pin-state representation is not a preservation gate.
- `ActionDescriptor`/`ResolvedActionState` stay unless implementation uncovers a separate proven deletion case; they are not the semantic activation runtime.
- The renderer-neutral accessibility vocabulary stays unless an individual type demonstrably adds no domain value; only duplicate mappings/evidence are deleted.
- `open-gpui-motion` retains execution ownership; theme supplies policy/defaults only.
- Taffy measurement, layout order, scroll extent, and sibling flow stay authoritative; U12, U13, U15, U16, and U17 consume committed layout without replacing it. Existing renderer matrices and clip payloads remain internal projections rather than public subtree semantics.
- Dock retains its canonical retained `DockGraph`, n-ary same-axis split normalization, transaction/session generations, and explicit `DockLayout`/`DockSurfaceSnapshot` persistence values. U18-U20 and U24-U28 do not port ImGui's immediate-mode context or binary node identity; U27 adds a transient presentation lease without making provisional state durable topology.
- `Display::None`, component disabled state, overlay presence, and decorative semantic omission remain distinct facts; U13 deletes only competing ancestor-level presentation authorities.
- Cargo/typed contracts remain the distribution seam; no registry/scaffold system returns.

### Required Review Checkpoints

- After U2: verify the AccessKit harness reflects platform output rather than a test-only reconstruction.
- After U4: adversarial review for dual overlay authority and callback/focus reentrancy.
- After U5/U6: final-tree/action parity and API migration review across representative families.
- During U7 prototype: require an independent non-theme consumer before exposing generic inherited context; audit for hidden global state.
- After U10: compare deleted duplicated authority and new federated conformance code; reject a net-shallower design or any central-registry revival.
- After U12: require joint renderer/input/accessibility review of primitive coverage, transform composition/inversion and numeric fail-closed behavior, clipping/scroll/capture/IME, deferred/portal/cache behavior, layout invariance, Motion ownership, and the supported-platform matrix. Reject visual-only success, identity/clamp recovery, partial-channel failure, or a prematurely general affine API.
- After U13: require joint input/focus/overlay/accessibility review of the exact state matrix, ancestor dominance, dynamic stale-state cleanup, custom-element fail-closed behavior, and deletion of paint/a11y dual authorities.
- After U14: require accessibility/privacy review of final-tree live defaults, transient lifetime/order, inactive and multi-window behavior, focus independence, native adapter evidence, and message-canary non-retention.
- After U15/U16: require joint geometry/overlay/focus/accessibility/virtualization review of handle ownership, current-versus-committed ordering, unlink and cancellation semantics, nested transform-aware reveal, and deletion of raw live-geometry/reveal tails.
- Before U17 public API: require renderer/input/accessibility review of child-local border-box coordinates, own-bounds shorthand, normalization timing, exact nested representation, primitive ABI, native surfaces, fail-closed behavior, and supported-backend test feasibility.
- After U17: require cross-backend paint/hit parity, deferred/cache/portal and presentation review, accessibility-limit documentation, and source scans proving no rectangle-only competing stack or arbitrary path placeholder remains.
- After U18: require Dock/theme/dependency review of style completeness, resolver scope/purity, cache/deferred and cross-window/subtree resolution, out-of-band drag source/target generation semantics, payload identity stability, guide-metrics naming, and hard-coded palette deletion.
- After U19: require Dock/application/focus review of explicit transaction boundaries, same-turn independent commands, commit-only observed event ordering, revision coalescing, unique activation-host generations, activation terminal outcomes, reentrancy, close/replacement, and caller-owned persistence.
- Before U20 public API: require Windows/macOS/Linux capability and legacy-return census plus review of coordinate spaces, creation-only versus live flags, placement/restore-bounds conflict semantics, coherent two-field activation-policy semantics, dispatch/terminal-observation vocabulary, committed getter authority, independent-domain partial batches, and test feasibility.
- After U20: require owning-platform evidence for every advertised live domain plus Dock route/placement/readiness review proving coherent observed facts remain authoritative and dispatch never creates a persistence revision.
- Before U24 implementation: require a callback-by-callback asynchronous-event, synchronous-query, and hybrid-result taxonomy; a consumed/propagated App-idle native-disposition prototype plus zero-busy instrumentation for every must-immediate hybrid class; application ingress sequence plus merge/FIFO/cross-window barrier rules; full-`WindowId`/domain-generation identity; and an audit of every native effect currently executed under outer App or subordinate entity/controller/runtime borrows.
- After U24: require GPUI/platform/Dock reentrancy review of AppCell ingress ownership, immutable query snapshots, exact handler-derived hybrid dispositions without fixed fallback or replay, zero busy entrances, closed command FIFO ordering/non-recursion/terminal validation, bounded fair drain, terminal ordering, reserved-window commit/rollback delivery, both effect borrow boundaries, frame invalidation/validation, nested callbacks, and the tear-off close-observer failure path.
- Before U25 public API: require Windows/macOS/Linux/web review of creation-only `focus_on_appearing`, lifetime activation/click/input, deletion of live focus-on-appear mutation, permanent non-activation, owner/transient semantics, accepted/submitted/non-empty presentation, and same-window session promotion.
- After U26: require Dock/application/platform review of non-overlapping owner/runtime/live-undock state, synchronous registry-commit activation of opening tokens, explicit reopen only after registry/native terminal convergence, freeze/snapshot/release/apply shutdown, forced close-policy bypass, stale callbacks, commit-only revisions, and two-surface isolation without `cx.quit`.
- Before U27 live content: require Dock/GPUI/accessibility review of the single live-undock session, controller-owned presentation lease, source semantic/focus proxy, explicit provisional presentation states, session suppression, z-ordered underlay and foreign-surface qualification, exhaustive release/focus/accessibility decisions, gate-preserving preflight, atomic final handoff, and renderer/surface failure rollback.
- After U27: require native-input/Dock review of source-capture transport, target raw-event independence, underlay resolution through the provisional, release revalidation, all terminal cancellation paths, visible non-empty pre-release presentation, same-window promotion, one-window reuse, destination activation/focus/AccessKit ownership, and graph/registry/lease/gate atomicity.
- After U28: require evidence review proving the designated tests use U24's real-HWND support, system-level pointer injection without a selected receiver, actual WndProc/capture/present/lifetime boundaries, delayed-close/reopen exclusion, every reported regression's failure path, deterministic worker cleanup, and honest VisualTest labeling.
- Before completion: simplify-code pass, structured code review, supported-platform renderer/window-mutation/multi-viewport evidence, full Verification Contract after U28, and release-doc audit.
