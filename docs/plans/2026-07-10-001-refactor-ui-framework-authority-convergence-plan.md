---
title: "Open GPUI UI Framework Authority Convergence - Plan"
date: 2026-07-10
type: refactor
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
deepened: 2026-07-30
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
- a detached Dock viewport may appear without stealing focus yet remain normally activatable later, defaults to a peer top-level window while preserving explicit supported owner/transient relationships, and belongs to one generation-bound DockSurface window session;
- native pointer capture can route a Dock drag across real windows without the target HWND receiving raw mouse messages, a provisional viewport is visibly presented before normal release, an early release settles through one bounded pending state, a release to a new destination commits at most one durable graph transition, and release or cancellation settles exactly once without cancellation mutating the durable graph;
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

R25. Provide capability-specific GPUI platform-window mutation for position, size, state, dynamic flags, and programmatic activation. Dispatch reports queued, unchanged, unsupported, rejected, or closed without claiming native commitment; a generation-bound observation ticket separately settles as exact, adjusted, superseded, rejected, unsupported, or window closed. Position, size, state, restore bounds, and the native client-geometry readback they produce form one explicit placement conflict domain, while independent flags may partially dispatch. Lifetime activation acceptance and click-focus remain independent fields inside one coherent activation-policy domain: they share one generation and terminal observation but are never aliases and cannot partially commit within that domain. Programmatic activation is a separate observed transaction: an accepted native command is only dispatch evidence, and exact target focus/foreground observation, supersession, explicit cancellation, rejection, or window terminal must settle its ticket. A temporary loss-before-gain gap does not settle failure. Target-display selection consumes one complete KTD38 publication generation; logical client size and cursor anchoring use that target scale while the callback-scoped physical screen point remains unchanged. All public platform-fact getters read the same committed observation authority. Dock consumes that contract and removes capability claims, direct-backend reads, or sync records that fabricate live support.

R26. Deliver every asynchronous native platform-window callback through one typed, generation-bound event authority that remains usable while `App` is mutably borrowed. Every event receives an application-wide ingress sequence before direct delivery is considered; envelopes also carry the full generation-bearing `WindowId`, event domain, and only the domain-specific pointer, mutation, Dock session, or drag generation they require. A callback-registration epoch is added only if one `WindowId` can demonstrably replace callbacks while remaining live. A callback may drain inline only when no older queued event, active drain, or unresolved barrier can be bypassed. Frame and coherent placement facts may coalesce only within the same window/domain/domain generation. Activation, synthetic modifier, and hover edges are sequenced and non-coalescing because they carry cancellation and enter/leave side effects; queue-eligible button edges, text input, pointer cancellation, close lifecycle, and mutation terminal observations are likewise ordered and non-droppable. Pointer cancellation and close are ordering barriers, stale events cannot target a replacement window, and a deferred frame is re-requested rather than acknowledged as painted. Synchronous native queries never wait on this queue: hit testing reads committed immutable facts, and close permission conservatively prevents immediate native destruction while queueing close intent. A hybrid input callback whose native default disposition depends on the current `DispatchEventResult` is not eligible for a fixed committed fallback or delayed replay. Framework-owned platform commands that can synchronously pump such input execute from one closed typed FIFO only after the outer `AppRefMut` is released and all older native-event barriers are settled, so the callback reads the real handler result with `AppCell` idle; a busy entrance is an invariant violation with callback-specific diagnostics. Any other return-valued callback must define and prove an equally explicit equivalent result contract before it migrates.

R27. Separate creation-time appearance from lifetime activation and native ownership. A window may be shown without initial activation while remaining eligible for later click, programmatic activation, and focus. Permanent non-activation is an explicit capability, not a side effect of `focus_on_appearing = false`. A typed owner/transient relationship is projected to each capable backend without creating a child window or replacing explicit application teardown; session lineage never implicitly becomes native ownership. Ordinary detached Dock viewports and same-HWND provisional promotion use one role-aware final top-level policy, defaulting to peer top-level windows; an application may explicitly request a supported owner relationship. Unsupported native relationships remain observable.

R28. Give each facade-managed `DockSurface` one explicit, generation-bound window session with a unique anchor, owned committed and provisional viewports, and `Vacant/Closed -> Opening -> Active -> ShuttingDown -> Closed` lifecycle. Embedded, primary-anchor, and managed-viewport hosts have distinct private roles; render cannot infer or activate an anchor. Primary creation is authorized by its exact `Opening` token; after commit, only work carrying the exact current `Active` lease is admitted, including managed viewport opens/restores, registration, scene publication, activation, drag, route, mutation, and observation. Every managed or provisional child establishes a validated anchor-to-child native-retirement dependency before runtime admission; rejected identity or cycle registration compensates that child while the surface remains Active. Pre-commit creation/map/draw failure, synchronous close during creation, duplicate open, application shutdown, post-commit presentation failure, and stale generation callbacks settle explicitly without rolling an Active session back into Opening. Closing the current anchor freezes new work, exact-generation cancels the matching GPUI active drag/native pointer capture, Dock route/feedback, and pending/provisional viewport work, and commits a non-interactive `ShuttingDown` anchor state that suppresses pointer, keyboard, IME, focus, routing, and AccessKit actions while retaining one stable frame. It then force-closes a deduplicated snapshot of only that surface's owned windows after all runtime borrows are released. Dependent native terminals release the queued anchor retirement; logical registry removal never proves native destruction. A failed or borrow-conflicted close dispatch remains retryable until native terminal, and a cleanup panic cannot prevent the remaining forced-close effects from running or strand the session after `ShuttingDown`; the first panic may propagate only after required terminal cleanup has been applied. `ShuttingDown` reaches `Closed` only when the exact anchor and every snapshotted close ticket have reached native terminal and the current-generation runtime registry is empty; reopen is rejected before that point. App shutdown may clear the logical registry first, but GPUI retains detached owners and routes their exact native-terminal callbacks until the same convergence condition holds. Shutdown and repeated close requests are idempotent, bypass per-viewport `Prevent` and `MergeBack` policies, never unconditionally quit the application, and cannot be triggered by stale anchor generations or implicit low-level hosts.

R29. Route native cross-window Dock drag from the capture-owning source through one generation-bound active-drag authority. GPUI reserves one non-forgeable start generation before invoking the drag listener and exposes only that exact token through the drag-start context. The listener may return one unactivated Dock consumer registration bound to the reservation; a callback-free GPUI start commit installs the active-drag authority, outbox consumer, and Dock route activation together, while listener failure or rejected start synchronously revokes the reservation, prepared registration, active drag, and pointer capture. No event-receiving route may contain an optional, absent, or deferred generation binding. Each native source move and `MouseUp` produces one immutable captured-drag fact containing the source full `WindowId`, drag generation, ingress sequence, button or terminal kind, and one native pointer observation in a single physical desktop coordinate frame. `WM_MOUSELEAVE`/`MouseExitEvent` remains local hover invalidation because it has no callback-scoped routing point and cannot synthesize captured movement. Non-pointer terminal barriers such as Escape, close, deactivation, and capture loss carry their exact identity, generation, sequence, and reason without sampling the current cursor or reconstructing a physical observation. The source backend derives a move or `MouseUp` global `Point<DevicePixels>` directly from that callback's signed client-device coordinates and client-to-screen conversion before any logical-coordinate projection; it also records the exact source geometry used by that callback, so a later DPI or geometry query can never rescale the input. The point-scoped hit observation owns its sampled point and classified front-to-back entries through the first non-pass-through terminal; each registered entry embeds one coherent immutable client geometry instead of exposing independently sampled bounds and scale. It distinguishes generation-bearing registered application windows from opaque barriers without leaking raw handles; only destroyed, minimized, or demonstrably invisible entries may be skipped. Dock resolves that fact using the exact surface/session and host-scene frame generations. Scene publication and cleanup carry the complete frame identity, so delayed G1 cleanup cannot remove a same-registration G2 scene, feedback projection, or route supported by a current scene. Underlay resolution passes through only exact generation-bearing entries: the current provisional/session-gated window and a current no-input pointer observation. Dock revalidates every pass-through generation when a queued captured event is consumed, so any intervening input-capability change, including a false-to-true-to-false ABA, yields `Unavailable`. A current same-surface host is eligible; a host from another Dock surface is a typed forbidden target with rejected preview; every ordinary, foreign-process, unregistered, stale-scene, or otherwise unknown visible top-level window is an opaque barrier that yields desktop tear-off rather than allowing a hidden Dock host below it to receive preview or drop. Windows may report an available observation only when repeated Z-order classification is stable and an independent point hit agrees with the first effective entry after the exact pass-through prefix; incomplete enumeration before that terminal, a cycle, a destroyed handle, sampled-point mismatch, geometry drift, pass-through-generation drift, or verifier disagreement returns typed `Unavailable`. Windows must preserve exact registered sibling IDs before child-only normalization. Dock converts the locked physical screen point directly into the chosen target's local logical `Pixels` using only the geometry embedded in that same observation; it never subtracts logical global origins measured under different monitor scale factors or re-queries current DPI, bounds, or client origin during resolution. `MouseUp` locks its point, hit observation, candidate, and generations exactly once; later first-presentation work may revalidate liveness at that locked point but may not read a later mouse position. Source callbacks record facts, release their `Window`/Dock borrows, and then deliver them in R26 ingress order; they never inject raw input into a target HWND or transfer native capture. A normal `MouseUp` is terminal release authority, while its following capture-change notification is cleanup only.

R30. Start opening and reusing one provisional tear-off viewport before release once a drag crosses the live-undock threshold. The threshold fires once per drag generation on the first available physical desktop/opaque-barrier route that crosses a hysteresis measured from the immutable drag-start source-scene snapshot; valid hosts, foreign-surface rejection, and `Unavailable` do not trigger creation. Reserve and register one exact U26 pending provisional-opening token as session-owned work before `open_window`. Anchor or App shutdown cancels that slot and waits for its terminal result; a late-returning HWND first receives its exact close/quiescence ticket and is compensated without gaining presentation, route, or promotion admission. When synchronous opening returns a committed full `WindowId` for a still-current slot, register an explicit U26-owned `ProvisionalViewport` role and close ticket before admitting presentation, routing, or promotion facts. It is not a committed Dock viewport, host-scene, activation target, or durable `DockGraph` entry until promotion. It owns one generation-bound payload-subtree presentation lease and immutable source presentation snapshot covering item, tabs, and floating payloads while the committed graph and revision remain unchanged. Lease activation immediately revokes the old source payload subtree's input, focus, and accessibility eligibility, then requires `SourceProxyCommitted(source WindowId, source frame generation, lease generation)` before the hidden provisional mounts the real payload. The source retains one stable semantic/focus proxy rather than a duplicate panel subtree, and the provisional becomes the only live content renderer only after that committed source-frame barrier.

The provisional is created with the ordinary detached viewport's final role-aware native profile, normally a peer top-level window with no initial activation, so same-HWND promotion never requires a native owner or capability transition. A generation-bound GPUI window-session interaction gate is synchronously readable by every owning backend without borrowing `App`: it suppresses provisional native activation, pointer/key/text/IME dispatch, focus, route eligibility, and AccessKit ownership while continuing paint, resize, close, and lifecycle facts. The owning platform also makes only the exact current provisional natively hit-transparent and reports its z-order/reveal observation; rejecting GPUI input alone does not satisfy underlay routing or visible presentation. Suppressed user input and accessibility actions are rejected rather than replayed after promotion; only framework-owned promotion/focus completion work may carry the promotion generation. A private provisional-only deferred-initial-presentation mode retains the intended hidden placement after initial root commit. After the exact lease, role, gate, source-proxy barrier, and payload mount are ready, the session explicitly invalidates every affected source, provisional, previous-target, and next-target surface, requests the bootstrap frame, and wakes the frame loop. The exact current non-empty present enqueues one generation-bound, non-activating reveal through the post-App platform-command FIFO; that command atomically consumes the retained placement, keeps the foreground window unchanged, and publishes visible/z-order observations. Stale, rejected, or terminal reveal outcomes compensate the exact session. No public visibility mutation is introduced, and first presentation never depends on another pointer move, release, activation, or incidental expose event.

The live-undock state is orthogonal: transport terminal state, pending open/provisional readiness, current route feedback, presentation-lease location, observed placement, and release latch evolve independently. Hidden create advances through non-empty accepted/submitted/present observation and no-activate show into a continuously visible provisional. Entering a valid host or forbidden surface does not hide that HWND: the target and provisional render generation-matched accepted or rejected route feedback, with alpha only an optional visual refinement. Repeated movement repositions the same window. A `MouseUp` that races readiness records exactly one locked `ReleasePending` fact from R29, including desired physical bounds and placement generation. A host outcome may retire an unseen provisional. A desktop release waits a bounded, injectable deadline for the same HWND to become non-empty visible and for that release placement to settle `Exact` or `Adjusted`; superseded, rejected, closed, or timed-out placement restores the still-live source. Direct provisional terminal close or renderer failure is `Unavailable`: a current host result may still commit, but desktop release restores the source.

Every independently fallible promotion preflight completes while the provisional remains gated. A `PreparedPromotion` validates the complete next Dock workspace, destination identity, role, registry, payload lease, retained presentation, semantic/focus seed, placement ticket, and gate transition before it enters one private promotion executor. The executor owns one exact identity and one monotonic authority transition from `Abortable` to `ForwardOnly`; the first irreversible receipt crosses that boundary. Every later provider, workspace, viewport, Host, retained-visual, surface, publication, and native-window-effect stage uses a keyed commit-or-replay receipt. No post-boundary path rolls back or interprets a fixed retry count as a semantic failure. A superseded or missing Graph receipt, destination replacement, placement-generation change, or endpoint loss enters committed-destination recovery with the same journal. Surface events publish through the owner in revision order, and subscriber reentry cannot invalidate an already committed revision or make a later revision overtake it. A generation-bound `DestinationSemanticsAccepted` receipt records the exact focus-stable semantic candidate but does not remove the gate. Only a matching `DestinationSemanticsSubmitted` acknowledgement, produced when that same window and frame generation receives renderer `Submitted` evidence under the exact workspace Graph, placement, lease, and semantic generations, removes the gate and schedules activation/focus. `Deferred` keeps the journal gated, `RepaintRequired` invalidates the accepted candidate and requires a newer one, and `Rejected` or renderer/surface terminal follows the pre- or post-boundary recovery rule. Stale acknowledgements cannot open a replacement generation. Desktop promotion retains the same HWND; release to another valid host transfers and settles there before activating the target host; return or cancellation restores source presentation and focus only while the executor remains `Abortable`. Source deactivation or shutdown never steals focus back. Repeated release/cancel callbacks, rejection, Escape, capture loss, source/payload/anchor close, stale generation, creation/presentation failure, or pre-boundary preparation failure settle one terminal result without duplicate semantic ownership or leaked windows. Post-boundary loss continues forward settlement or transfers the exact journal to committed-destination or surface-shutdown recovery; it never publishes a generic terminal failure while mandatory authority remains. A generation-bound `PresentationShutdownTicket` stops new draw/present work, drains or retires current submissions, releases backend surface-bound resources, and records a typed quiesced acknowledgement before native window destruction may publish terminal state. App shutdown's synchronous pre-clear barrier must claim and quiesce every current ticket before registry clear detaches the platform owner into GPUI's native-retirement coordinator; the later native terminal remains mandatory and an asynchronous future cannot supply the pre-clear ordering.

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
20. A detached viewport is shown without taking initial focus. It still activates by click and `activate_panel`, is a peer top-level by default rather than inheriting DockSurface session lineage as a native owner, reports an explicit requested owner/transient relationship when supported, and never acquires permanent no-activate styling merely because initial focus was disabled.
21. Two independent Dock surfaces each open an anchor and detached viewports. An embedded host never becomes an anchor, and the first managed surface admits no viewport or scene/activation fact until synchronous `App::open_window` returns a committed full `WindowId` and its `Opening` token validates into an exact `Active` lease. Closing that anchor during an active drag exact-generation ends the matching GPUI active drag and native capture, retires Dock route/feedback, cancels opening/provisional work, force-closes only its runtime-owned dependents despite `Prevent` or `MergeBack`, and leaves the second surface usable. A borrow-conflicted dependent close remains retryable; a route or activation cleanup panic cannot stop the remaining cancellation and close effects from being applied or durably scheduled before the first panic resumes, and the retryable tickets later converge every dependent and the anchor to terminal. The surface remains `ShuttingDown` until the exact anchor and every close ticket are native-terminal and its runtime is empty; reopening is rejected until `Closed`, after which a new generation admits no stale create, close, or drag cancellation. Direct App shutdown with all window kinds alive quiesces presentation before registry clear and converges from retained native-retirement terminal callbacks without relying on logical close observers.
22. With Win32 capture held by the source HWND, the pointer crosses a second viewport that receives no raw mouse move. The source callback records the signed client device point, its client-to-screen physical point, source geometry, and a point-bound hit observation before logical conversion; changing the source DPI before deferred consumption cannot move that fact. `WM_MOUSELEAVE` changes only local hover and publishes no captured movement. The source-owned route passes through only its exact provisional role, converts the locked physical point through the coherent geometry embedded for the target, publishes the target preview before release, and commits the drop exactly once. A Windows observation is available only when stable classification agrees with an independent point hit; disagreement is unavailable rather than permission to reach a hidden host. `MouseUp` locks its ingress sequence, point, hit observation, candidate, and generations; a delayed first-presentation task may revalidate only liveness at that locked point and never substitute a later cursor position. A host from another Dock surface shows paired source/target rejected feedback and cancels on release. An ordinary or foreign opaque top-level window over a Dock host is a desktop barrier, so the hidden host receives neither preview nor drop. Capture loss, Escape, source close, or anchor shutdown clears every preview and pending task without using the last valid target. Closing only the current target clears the exact source/target projection pair while retaining the source-owned drag; the next movement or release resolves again. Delayed G1 scene cleanup cannot remove a same-registration G2 scene, projection, or route.
23. A user drags an item, tab set, or floating payload onto the desktop. After crossing the physical source-scene threshold, the session owns one pending provisional open before native creation. The source first commits a generation-matched semantic/focus proxy frame; only then does the U26-owned provisional mount the real payload and explicitly request and wake its bootstrap frame, becoming natively hit-transparent and visible at the intended z-order with a non-empty renderer-submitted frame before button-up even if no further pointer, activation, or expose event occurs. Its final peer-top-level native profile remains exact-session-gated and the durable graph/revision remain unchanged. Repeated movement repositions the same HWND through typed live-route client-geometry transactions rather than recreating it; entering a host or rejected foreign surface keeps that provisional visible and projects current accepted/rejected feedback into both surfaces. Its gate rejects provisional native activation, pointer/key/text/IME, focus, route, and AccessKit actions rather than replaying them after promotion. If button-up races source-proxy commit, create, show, first-present, or release placement, one locked `ReleasePending` callback frame preserves suppression: a host result retires the unseen provisional, while desktop waits for the same HWND's exact generation-bound presentation and final-placement acknowledgement or restores the source on failure. Normal desktop release prepares and validates the complete next state while suppressed, then the KTD32 executor commits the first irreversible receipt and settles every later provider, Graph, viewport, Host, retained, publication, and native-effect stage from exact receipts; the gate remains closed after semantic-frame acceptance until that exact frame is renderer-submitted, after which KTD34 activation and focus restore the previously focused live payload descendant. A destination terminal after the boundary is a separate committed viewport-loss transition, not a second drag release or rollback. Release over another valid host transfers and commits there before its activation host receives focus; return or cancellation restores source presentation and focus before destroying the provisional. Anchor/App shutdown during native opening terminally compensates any late-returning HWND before admission. Source deactivation or shutdown cancels without stealing focus or attempting restoration into a dead source, and every provisional close quiesces renderer/surface work before native terminal state.

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

KTD21. **ImGui defines Dock behavior, not Open GPUI architecture.** The repository-local `repo-ref/imgui` clone is pinned to `docking@4be08b1ecf7709f15e4274fb2ddac37e121d7d9a`, and the companion `repo-ref/dear-imgui-rs` snapshot is pinned to `abac30563586d5a4b0f7fbb911e86c576c1db9a1`. They anchor drop rectangles and previews, selected-tab/focus/splitter behavior, threshold-triggered viewport creation before release, source-owned native capture, continuously presented moving payload viewports, route feedback drawn into both payload and target surfaces, input-transparency semantics, logical close followed by explicit native teardown, and renderer-before-platform destruction. `WindowSelectViewport`, `DockNodeUpdate`, and `SetWindowViewport` prove that a detached/docked transition can retain one native-window identity; they do not prove Open GPUI's retained payload lease, prepared promotion, or atomic graph transaction. Dear ImGui's Winit history, especially `ef28268c`, `fd6ba908`, and `194cfee2`, is comparative evidence that monitor publication must be a complete detached transaction, public viewport position means native client geometry, client and outer rectangles cannot be interchanged, decoration/frame-extents and DPI changes may require later native-event reconciliation, and an event-driven host must explicitly wake geometry updates. These lessons strengthen R25, KTD36, and KTD38; they do not justify Winit callbacks, `ImGuiViewport*` identity, a frame-driven global mouse poll, capture transfer to the main viewport, `HWND_TOP` without point-scoped barriers, optimistic focus, or a fixed timeout as semantic authority. ImGui's 2025 `ConfigViewportsNoDefaultParent = true` change anchors the peer-top-level default: native owner relationships are role-specific and never inferred from logical session ownership or used for lifetime cleanup. ImGui calls `Platform_ShowWindow` before a separate renderer render/swap, so it supports staged create/update/show/render/destroy ownership but does not prove Open GPUI's stronger non-empty-present-before-show contract; U29 must establish that contract from GPUI's own presentation authority. ImGui's inability to reliably see opaque non-ImGui occluders is a known limitation, not behavior to copy: only Open GPUI's exact provisional/session-gated window is transparent to Dock target resolution. Open GPUI may impose the stronger dependent-windows-before-anchor shutdown order; ImGui's current `DestroyPlatformWindows` traversal is not evidence of reverse-order destruction. Open GPUI retains its Rust retained-mode `DockGraph`, `DockHost`, typed outcomes, transaction, session, and generation architecture; global immediate-mode context, raw node pointers, callback tables, frame-age garbage collection, focus-stamp z-order guesses, `p_open` close authority, topology mutation at drag start, per-frame `DockSpace` construction, `DockBuilder`, PlatformIO ownership, and `.ini` persistence are rejected.

KTD22. **Dock style is a named immutable render input.** `gpui_docking` owns one complete `DockVisualStyle` value and built-in fallback. A narrow resolver is installed as an immutable per-surface value, or passed explicitly to a low-level host; there is no mutable app-global registration. It synchronously reads only the active GPUI render context, cannot update entities, notify, dispatch, or reenter rendering, and lets application integration map `ThemeResolver::current` without a reverse dependency on UI Components or a generic context/service API. Target-host affordances resolve target style. Source-owned deferred drag visuals freeze an out-of-band style snapshot keyed by drag session/opening generation, never add visual values to `DockDragPayload` or its equality/route identity, and clear that snapshot on cancel/close before reopen captures anew. Existing drop-guide layout values are renamed `DockDropGuideMetrics` so they cannot appear to compete with the visual authority.

KTD23. **DockSurface publishes committed facts; applications own persistence.** A private owner entity allocates an explicit root transaction identity at every facade, host, or runtime mutation boundary and threads it through typed controller and viewport-runtime commits. Nested work carrying that identity coalesces categories and publishes once when the transaction commits; two root commands in one App turn and work across turns never merge. Asynchronous platform observations enter a new observation transaction and dispatch status never creates a persistence revision. Each Dock space has at most one committed activation-host registration generation; duplicate live registrations reject rather than silently replace. `select_panel` remains selection-only, while stable-item `activate_panel` routes through that owner and settles through GPUI focus completion. Snapshot export is explicit and revision-consistent; storage, serialization timing, debounce, and I/O remain caller responsibilities.

KTD24. **Window mutation is dispatch plus terminal observation, not synchronous truth.** Backend setters first adopt an honest dispatch contract: queued means GPUI handed work to a backend path, not that the OS accepted or applied it, and legacy unit/bool returns cannot be guessed into richer outcomes. A generation-bound ticket later settles from a coherent observed `WindowPlatformFacts` snapshot as exact, adjusted, superseded, rejected, unsupported, or window closed. Position, size, state, and restore bounds share one placement conflict domain and generation because native state transitions move/resize windows; the GPUI planner rejects contradictory placement batches before dispatch and never exposes partially committed placement. Lifetime `accepts_activation` and `focus_on_click` are independent fields in one coherent `ActivationPolicy` domain with one generation and terminal observation; they are never aliases and cannot partially commit. Other independent flag domains may partially dispatch. Public bounds/state/flag getters read this committed cache, seeded at creation and updated only by platform callbacks or an explicit observation refresh that enters the same generation/event path.

KTD25. **Platform callbacks are sequenced facts, not borrow-or-drop notifications.** `AppCell` owns a private native-event ingress beside, not inside, its `RefCell<App>`. Asynchronous callbacks take one application-wide ingress sequence before entering that inbox. Direct delivery is an optimization only when no backlog, active drain, or barrier can be overtaken; otherwise the event queues and wakes the foreground executor. Ordering barriers and generation-scoped per-domain merge rules are part of the runtime contract, not backend folklore. Synchronous native queries use immutable AppCell-adjacent snapshots or documented conservative responses instead of blocking or reentering `App`. Hybrid input whose native default handling depends on the current handler result is a separate contract: fixed snapshots and delayed replay are forbidden, and framework-owned pump-sensitive commands must make an App-busy entrance unreachable so every consumed/propagated result comes from the real idle handler. Frame validation acknowledges accepted or rescheduled work rather than a callback invocation, and diagnostics identify callback kind, full `WindowId`, domain generation, sequence, and disposition.

KTD26. **Native effects cross the borrow boundary required by their reentrancy class.** GPUI and Dock calculate typed effects while entity, controller, or viewport-runtime state is borrowed, release those guards, then apply model-owned open, close, remove, present, and mutation work through the owning `&mut App`. The closed set of platform commands that can synchronously pump native input or delegate callbacks (`activate`, native window menu, interactive move, and interactive resize) instead enters an AppCell-owned FIFO and executes synchronously after the outer `AppRefMut` is released and older native-event barriers settle. The command FIFO carries a full `WindowId`, uses a weak backend dispatcher plus terminal validation, drains non-recursively in order, and is not an arbitrary callback or asynchronous native-effect outbox. `App::open_window` remains synchronous, construction/map remains fallible in the window transaction, close remains ownership-governed, and draw/present remains frame-governed. Events targeting a reserved/current full `WindowId` wait until commit or rollback, then drain or retire. No backend's current asynchronous implementation is treated as a non-reentrancy guarantee.

KTD27. **Initial appearance, lifetime activation, and native ownership are separate contracts.** `focus_on_appearing` is a one-shot show policy. Click/programmatic activation and permanent non-activation are lifetime capabilities. `transient_for`/owner is a typed top-level relationship for z-order, activation, and platform grouping, never `WS_CHILD` and never a substitute for explicit teardown. DockSurface session lineage never chooses a native owner. Ordinary committed detached viewports and provisional windows use the same role-aware final relationship, defaulting to peer top-level; explicit application ownership remains supported where the backend can report it. A provisional is created with its final detached lifetime capabilities but without initial activation; a generation-bound Dock session gate, not a native flag transition, suppresses input, focus, activation, route eligibility, and accessibility until same-window promotion.

KTD28. **DockSurface owns a window session, not application exit.** A private session authority, distinct from the viewport facade, owns `Vacant -> Opening -> Active -> ShuttingDown -> Closed`, a monotonic generation, exact anchor identity, shutdown cause, and terminal ticket snapshot. `open_primary_window` first reserves an `Opening` anchor token, then calls synchronous `App::open_window` inside the current App transaction. Registry commit returns the full `WindowId`; a short owner borrow validates that ID and token before transitioning to the one current `Active` generation. Native callbacks raised during creation queue until commit or rollback and never activate the session themselves; a primary host carries `Opening(token)` until commit and may draw without registering, then atomically becomes `Active(lease)`. Pre-commit create/map/initial-draw or before-visibility presentation failure settles `Opening -> Closed`; once the full ID commits, later presentation failure settles `Active -> ShuttingDown(PresentationFailed) -> Closed` through forced teardown rather than reviving or rolling back the consumed token. Every post-commit facade-owned operation carries the opaque exact-generation lease so stale work cannot pass a new session through optional-value equality. Every managed or provisional child establishes the anchor-to-child native-retirement dependency before runtime admission; a rejected identity or cycle compensates that child while the surface remains Active. The primary close guard validates the lease, commits a stable non-interactive `ShuttingDown` frame, freezes admission, and snapshots exact drag, route, runtime, activation, provisional, window, and ticket effects while owner/runtime state is borrowed. After all subordinate borrows release, DockSurface orchestrates GPUI exact-generation capture cancellation, Dock cleanup, presentation quiescence, dependent close, anchor close, and convergence. GPUI alone ends the active drag, pointer capture, and outbox generation; DockSurface never mutates generic drag internals or waits for HWND destruction to imply cancellation. Close tickets distinguish pending, dispatching, dispatched, and terminal states so a failed or borrow-conflicted dispatch can return to pending and retry through the U24 post-borrow boundary. Cleanup stages retain the first panic, apply every required compensation and close effect, and only then resume unwind, so a panic cannot leave a permanently vetoed session with live dependents. Repeated requests are idempotent, and direct native destruction enters the same path from its exact native terminal. Surface-owned forced teardown bypasses individual viewport close policies and is isolated from other surfaces. `Closed` means the exact anchor and every snapshot ticket have reached native terminal and the current-generation runtime registry is empty. App shutdown may clear the logical registry first, but GPUI retains each detached owner until its native-retirement barrier publishes terminal; reopen is rejected until convergence. A later generation remains isolated from delayed prior-generation callbacks. Native owner relationships remain presentation hints, and last-window application exit stays GPUI policy rather than a Dock `quit` shortcut.

KTD29. **Captured transport is source-owned, point-scoped, and post-borrow.** GPUI reserves a non-forgeable start generation before the drag listener; Dock may return one exact-generation prepared registration, and a callback-free start commit installs active authority, outbox consumer, and route activation together. Reservation failure or listener/commit panic revokes the reservation, prepared route, active drag, and pointer capture, and a receiving route never stores an optional generation. GPUI alone owns the active generation, source capture, outbox ordering, and terminal cancellation. The platform alone owns callback-scoped physical points, source geometry, and point-scoped hit observations. Dock route alone interprets accepted, forbidden, unavailable, or desktop results and owns the current source/target feedback pair; viewport runtimes only validate exact scenes and execute projections. Dock consumes one sealed composite native capability rather than a Boolean bag: `Exact` proves source capture, callback-scoped point stacks, generation-bound no-input pass-through, provisional reveal/placement, target-DPI conversion, and activation-terminal observation as one supported profile; `InWindowOnly` and `Unsupported` carry typed reasons. The native capture owner publishes the immutable physical observation after source `Window` and Dock borrows release. Only native move/up callbacks with their own physical frame can advance the route; hover leave cannot reuse or resample a point. The hit observation owns its sampled point and the ordered entries needed through the first non-pass-through terminal; every registered entry owns one coherent immutable client geometry. On Windows, repeated top-level classification must be stable and independently agree with `WindowFromPoint` at the sampled point after excluding only the exact current provisional pass-through; `EnumWindows` alone cannot prove completeness because it is limited to desktop-app top-level windows on Windows 8 and later. Any omission before the terminal, cycle, destroyed handle, geometry drift, or verifier disagreement yields typed `Unavailable`. The capability remains false and every query returns `Unavailable` until sampled-point ownership, coherent geometry, checked coverage, complete-through-terminal classification, independent verification, and activation-terminal observation are all implemented. Scene removal uses the complete published frame identity rather than host or registration alone; stale cleanup cannot remove a replacement scene, feedback pair, or source route supported by a current scene. Target terminal clears the exact source/target feedback pair while leaving the source route active for the next fact. `MouseUp` latches its callback frame, route fact, point, hit stack, desired bounds, and generation exactly once and is release authority; an immediate later native move cannot redirect it. Terminal claim removes that exact route before effects and gives cleanup to a panic-safe guard, while later capture-change is cleanup and later readiness work may revalidate only liveness at the latched point. Direct source-HWND terminal settles capture and route exactly once without transferring capture or affecting a replacement generation. DockSurface owns only the exact-session shutdown decision and dependent-before-anchor tickets. U29's live-undock session owns only its temporary payload lease, readiness, release, and promotion saga. ImGui informs source capture and platform-window staging; Open GPUI rejects a single global docking context in favor of these typed authorities, full generations, and retained-graph transactions.

KTD30. **Live provisional presentation is a gated payload lease, not an early graph commit.** A `DockLiveUndockSession` owns the payload-subtree presentation lease, immutable source snapshot, provisional role/handle reference, route feedback, readiness, release latch, destination-semantics barrier, and compensating saga; the viewport runtime remains the sole handle registry and DockSurface remains the sole shutdown authority. The lease covers item, tabs, and floating payloads, immediately revokes stale source-subtree interaction/semantic eligibility, waits for a generation-matched `SourceProxyCommitted` frame, then exposes the real content only in the provisional while leaving one source semantic/focus proxy. A frozen noninteractive source visual remains until the matching provisional is non-empty and visible, preventing a visual gap without creating a second input or semantic owner. A frozen-provisional-only design is rejected: it cannot remain authoritative for asynchronously changing retained content, representative surface-backed payloads, or the same-HWND release path without introducing a second post-release mount and another visible/semantic handoff. The U29 phase-zero native proof must validate that premise with one asynchronously changing payload and one representative renderer/surface-bound payload before the general lease is implemented; failure reopens KTD30 instead of adding a screenshot exception. The provisional is registered only as a U26-owned provisional role until final promotion. Once the exact lease, role, gate, source-proxy barrier, and payload mount are ready, the session invalidates all affected surfaces, explicitly requests the provisional bootstrap frame, and wakes the run loop. A private deferred-initial-presentation mode retains hidden placement until a current non-empty submitted present queues one exact-generation, non-activating reveal through the post-App command FIFO. The first armed bootstrap snapshot remains stable so continuous motion cannot starve reveal. After reveal, every newer route generation repositions the same visible HWND through one `LiveRoute` placement ticket carrying its point, physical client bounds, point-scoped z-order proof, and terminal observation; old generations coalesce or settle stale without freezing the window. `MouseUp` atomically replaces that authority with one `FinalRelease` placement ticket, and only that purpose may become promotion evidence. The visible provisional stays across desktop, accepted-host, rejected-host, and recoverable unavailable feedback; the route owns one source-side/target projection pair, where the source-side projection renders in the visible provisional when present and otherwise in the source, instead of treating the provisional as a third independently cleanable marker or hiding and reopening the HWND. A backend-readable interaction gate rejects all provisional user input and accessibility actions without altering final native lifetime capabilities. `ReleasePending` retains the locked callback frame while readiness settles; unavailable release restores the source and retires the provisional without promotion. Independently fallible preflight remains gated. `PreparedPromotion` transfers one exact request into the KTD32 executor, which owns every irreversible receipt and the sole forward-settlement journal. An exact focus-stable destination tree produces `DestinationSemanticsAccepted`; only matching renderer-submitted evidence produces `DestinationSemanticsSubmitted`, removes the gate, and schedules activation/focus after the exact workspace Graph receipt, final placement, lease, and semantic proofs agree. Graph supersession enters committed-destination recovery. Same-HWND desktop promotion, host transfer, cancellation, failure, source/anchor shutdown, and stale callbacks each converge once without durable pre-boundary mutation, duplicate semantics, or focus theft. Provisional teardown requires a generation-bound renderer/surface quiescence acknowledgement before native terminal state.

KTD31. **Real native prerequisites gate live-undock implementation rather than merely validating it afterwards.** Before U29 generalizes the payload lease, the owning Windows backend must prove on a real HWND that a hidden window can accept and submit a non-empty frame, reveal without activation, remain natively hit-transparent, follow at least three distinct route positions while the source retains capture, lock one `MouseUp` callback frame against an immediate later pointer move, convert the same HWND from provisional to committed role, hand off the representative live payloads required by KTD30, observe activation terminally, and quiesce renderer/surface work before native destruction. This phase-zero proof runs on the same named interactive runner contract used by U28 and may not receive native credit from direct-message or `TestPlatform` simulation. The repository names the runner label, owning maintainer group, capability sentinel, serial global-cursor policy, and failure escalation before U29 begins; an unavailable runner blocks the native gate rather than silently skipping it. U28 then expands the proven substrate into the complete scenario matrix instead of discovering load-bearing backend failures after U29 is complete.

KTD32. **Promotion crosses one irreversible boundary and then settles forward through exact receipts.** One private promotion executor owns the journal for GPUI rehost, workspace Graph, viewport runtime, Host semantics, retained visuals, the DockSurface publication obligation/receipt, native window effects, lower receipt retirement, payload finalization, and shutdown transfer. Its request key binds live-undock identity, promotion token, destination identity, payload identity, and the intended surface revision. Authority moves monotonically from `Abortable` to `ForwardOnly` immediately before the first lower-layer write or when GPUI already proves `DestinationCommitted`. Every post-boundary stage is keyed `commit_or_replay`; equality of current state is not causal proof, and a fixed retry count or timer cannot decide success. Timers only wake a parked exact journal. Graph writes carry an exact commit identity and monotonic workspace revision. Only `Exact` may continue; `Superseded` or `Missing` proves that a newer or absent Workspace authority has replaced this projection and immediately enters committed-destination recovery. Graph equality cannot turn supersession into success, so ABA remains fail-closed and no second causal-lineage registry is introduced. Endpoint loss or placement-generation drift follows the same recovery rule. The executor remains alive until mandatory publication, window effects, provider/Host/retained cleanup, finalizer settlement, and lower tombstone retirement are proven or transferred linearly to DockSurface shutdown. GPUI owns the opaque rehost session and compensation ordering; Dock never drives raw provider phases. The private DockSurface owner queue remains the sole authority that commits and externally publishes surface revisions/events; the executor submits one causal publication obligation and waits for its exact acceptance receipt. Subscriber panic cannot reorder or roll back committed authority.

KTD33. **Graph mutation owns pre-mutation staging roots and one final sweep.** A checked mutation captures the identities of pre-existing unattached staging roots before the first topology write; it keeps the old dependency closure only as a deletion guard, never as a set of permanent roots. Local normalization may rewrite affected live spaces but cannot reclaim SlotMap entries. After the whole mutation succeeds, one mark-and-sweep starts from current live roots plus every captured staging root that still exists, then follows each root's current transitive dependencies; detached old dependencies and speculative nodes are reclaimed. `remove_subtree`, `simplify_space`, or another local helper cannot physically delete a node still reachable from a surviving staging authority. The post-commit invariant is `stored nodes == live reachable nodes union current dependencies of surviving pre-mutation staging roots`; repeated float, redock, merge, and empty-space moves must not increase the unreachable remainder. Builder/import canonicalization may perform an explicit full sweep at its own transaction boundary, but ordinary public staging operations remain valid between `insert_node` and `set_root`. Tests cover a shared staging/live subtree whose old dependency is disconnected during mutation and prove that only the current staging closure survives.

KTD34. **Native activation is an observed transaction, not a command side effect.** One window-owned activation ticket binds the full target `WindowId`, request generation, activation-policy generation, and optional caller completion. Platform dispatch may report queued, rejected, unsupported, or closed, but success requires an exact native focus/foreground observation for that target generation. The ordinary source-loss-before-destination-gain gap remains pending; a newer owned winner supersedes the request, and explicit cancellation, policy change, target replacement, or native terminal settles it without a fixed deadline verdict. Dock activation and source-focus restoration consume this ticket plus GPUI focus completion; neither may publish success from `SetForegroundWindow`, `SetFocus`, or equivalent dispatch alone.

KTD35. **Destination semantic authority is accepted and then renderer-submitted.** A focus-stable candidate frame can establish `DestinationSemanticsAccepted`, but that receipt only proves the semantic tree was accepted into the candidate/committed frame journal. The interaction gate, focus restoration, activation, and AccessKit ownership remain suppressed until the exact same window, semantic ticket, frame generation, final-placement generation, workspace Graph receipt, and payload lease receive renderer `Submitted` evidence as `DestinationSemanticsSubmitted`. `Deferred` parks the same journal, `RepaintRequired` invalidates the accepted generation, and `Rejected` or per-window renderer terminal enters the correct pre-boundary compensation or post-boundary committed recovery. An accepted-frame receipt, non-empty scene, or visibility observation cannot substitute for submission.

KTD36. **Window placement commits client geometry through native observation, not setter success.** Public position and size describe the native client rectangle in one declared desktop coordinate space. Outer-frame conversion is backend-private and uses the target monitor DPI plus the actual decoration policy; undecorated or custom-frame windows cannot inherit fictitious caption offsets from style bits. The callback-scoped screen point remains immutable in physical desktop coordinates, while logical client size and logical cursor offset are converted with the target-display scale captured by the same placement generation; source-window DPI cannot determine destination client size or anchor position. Hidden bootstrap, visible `LiveRoute`, and locked `FinalRelease` retain one target-bound placement intent containing the physical point, client bounds, display identity, display-publication generation, scale, and full/work bounds. A native setter, synchronous API return, requested monitor, or matching client rectangle alone is only dispatch evidence: exact settlement requires one coherent native readback whose bounds, display identity, topology generation, and scale still match that intent. Windows may perform one deterministic target-DPI correction under the same generation; X11 or another backend with asynchronous frame extents waits for generation-bound native move/resize/scale observations and readback. Decoration changes, initial show, DPI transitions, and window-manager adjustment preserve the requested client geometry or settle `Adjusted`/`Rejected` explicitly. Event-driven hosts explicitly wake the pending geometry reconciliation. Timers are watchdogs only.

KTD37. **Each renderer-owned native window has one private surface lifecycle.** A renderer-neutral window consumes `Submitted`, `Deferred`, `RepaintRequired`, and typed per-window `Terminal` facts, while backend-specific swapchain/surface states remain private. WGPU or another shared-device backend owns per-window `Active`, `SuspendedZeroExtent`, `RecreatePending`, `Draining`, and `Terminal` transitions. Ordinary resize or DPI change cannot perform an unbounded whole-device wait; zero extent suspends acquisition and cannot create a presentation receipt; a usable suboptimal frame presents before reconfiguration; surface loss wakes an exact-generation recreation or publishes a terminal fact for only that window. Shutdown continues to use the stronger generation-bound presentation ticket and exact last-use drain before native retirement. Fixed sleeps, indefinite warning loops, and backend enums exposed to Dock are rejected.

KTD38. **Display topology is one complete immutable publication.** A platform publishes displays, one uniquely proven primary identity, logical and physical desktop bounds, work areas, scale, stable provenance, and one monotonic generation as a detached transaction. Initial unavailability fails explicitly; a later partial enumeration, ambiguous primary, or per-display construction failure retains the previous complete generation and reports degradation rather than publishing a mixed or partial topology. A `DisplayId` is meaningful only with that publication generation, and target-display observations carried by pointer and placement transactions are immutable values rather than live native handles. Work-area-only, scale-only, provenance, primary, and topology changes each produce a new generation. Signed negative desktop coordinates remain exact.

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
  Embedded host -> render only; no implicit anchor/session teardown
  Vacant/Closed -> Opening(anchor token; draw-only primary role)
      -> Active(exact anchor generation + private admission lease)
      |-> committed/provisional viewport ownership with the same lineage
      |-> active drag generation
      |     `-> capture-owning source -> AppCell outbox with original ingress sequence -> Dock route
      |                                      |-> valid-host release: transfer + graph commit
      |                                      |-> desktop release: promote + graph commit
      |                                      |-> early MouseUp: locked ReleasePending -> promote/compensate
      |                                      `-> cancel/failure/close: restore + destroy
      `-> anchor close
             -> ShuttingDown (freeze + exact drag/route/window/ticket snapshot)
             -> release borrows -> exact GPUI capture cancel -> Dock/presentation cleanup
             -> retryable dependent close dispatch, then live anchor
             -> exact anchor native terminal + snapshot tickets native terminal
             -> empty current-generation runtime registry
             -> Closed (reopen eligible)
```

Live-undock presentation and promotion:

```text
transport:      moving ------------------------------------------------------> terminal
pending open:   none -> session-owned opening -> admitted provisional / compensated terminal
source proxy:   stale subtree revoked -> proxy frame committed / failed
readiness:      hidden Opening -> payload mounted -> non-empty presented -> visible + z-order observed / Unavailable
source visual:  frozen noninteractive payload ---------> retired after visible provisional
route feedback: desktop { open space | opaque barrier } <-> accepted host <-> rejected foreign surface <-> unavailable
lease location: source proxy <-> gated provisional -> committed destination
placement:      moving placement generations -> locked release placement -> Exact / Adjusted / terminal failure
release latch:  none -> locked ReleasePending -> consumed terminal result
destination:    not prepared -> Abortable journal -> ForwardOnly exact receipts -> semantics committed -> gate open / committed recovery
shutdown:       none -> presentation ticket -> renderer/surface quiesced -> HWND terminal

visible provisional: follows pointer and remains shown across desktop, host, and rejected feedback
promotion: preflight -> first irreversible receipt -> keyed forward settlement -> ordered revision publication -> semantics commit -> activation
cancel/failure: restore only a live source, quiesce presentation, destroy provisional, retire generation
```

State ownership remains non-overlapping:

| Authority | Sole state owned |
| --- | --- |
| AppCell native-event ingress | callback ingress sequence, queued envelopes, captured-drag outbox, drain/barrier state, immutable synchronous-query snapshots |
| App/window transaction | reserved/current full `WindowId`, commit/rollback boundary, and application of typed native effects after subordinate borrows release |
| GPUI active drag | generic capture-owner pointer transport, active generation, outbox ordering, and terminal cancellation |
| Platform hit observation | callback-scoped physical point/source geometry and complete-through-terminal point stack |
| GPUI window session gate | backend-readable provisional activation/input/focus suppression and terminal generation |
| Dock captured route | accepted/forbidden/unavailable/desktop interpretation and exact source/target feedback pair |
| DockSurface owner | session phase, exact anchor token/generation, opaque active lease, retryable terminal tickets, and dependent-before-anchor shutdown authority |
| Dock viewport runtime | sole session-lineage-tagged registry of committed, pending-opening, and provisional window handles plus exact scene/projection execution |
| Dock live-undock session | locked release/placement facts, source-proxy and provisional readiness, provisional role/handle reference, payload presentation lease, source semantic/focus proxy, promotion request identity, presentation-shutdown ticket, and pre-boundary compensation saga |
| Dock promotion executor | sole `Abortable -> ForwardOnly` promotion journal, exact commit-or-replay receipts, one causal publication obligation/acceptance receipt, mandatory native effects, committed-destination recovery, finalizer settlement, and shutdown transfer; the DockSurface owner remains the sole revision/event publisher |

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
U27 Captured Native Drag Routing -> U29 Live Provisional Promotion -> U28 Owning-Platform Evidence -> U30 Accepted-Frame Activation -> U31 Renderer Surface Lifecycle -> U32 Display/Geometry Authority -> final gate
```

U2 and U3 have no logical dependency and may be developed independently, although shared-worktree execution may serialize their Cargo gates. U3 and U4 share one authority-completion gate: U3 may commit pure policy and private preparation, but Focus Scope is not declared the production authority until U4 has migrated official overlay consumers and removed their duplicate focus bookkeeping. U12-U20 are serialized after the prior product-surface audit because they change GPUI frame boundaries or converge consumers that exercise those authorities. U14 lands before geometry follow-ons because it closes a renderer-independent accessibility gap. U15 establishes typed committed targets before U16 records scroll ancestry, and U17 follows a renderer/ABI review gate before changing every primitive clip representation. U18 then removes Dock's visual dual authority. U19 installs one application owner over Dock commits but admits only observed viewport facts, never the current sync layer's optimistic `applied` status; U20 reshapes backend dispatch and observation through that stable owner seam without preserving a false revision contract.

The native docking correction remains strictly ordered. U24 fixes the lower-level event-loss and effect-reentrancy substrate before later units depend on callback delivery. U25 separates window appearance, activation, and ownership on that substrate. U26 gives committed viewports a session owner and exposes the exact lease, handle-registry, close-ticket, and teardown substrate that U29 extends to provisional viewports. U27 establishes source-captured transport and fail-closed point routing without creating live content. U29 builds the gated provisional and same-HWND promotion on that route and session substrate. U28 supplies owning-platform evidence and cannot be replaced by `VisualTestContext` coverage. U30 moves programmatic activation publication onto the accepted-frame authority. U31 then isolates renderer-owned window surfaces and supplies exact submitted-frame evidence, while U32 converges the complete display topology and event-driven client-geometry authority required to make the multi-display claim portable rather than Windows-incidental.

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

Phase 9: Close the native multi-viewport correctness gap discovered by real Windows use: U24 makes callback delivery and native effects reentrancy-safe, U25 separates appearance/activation/owner semantics, U26 adds DockSurface window sessions and deterministic teardown, U27 adds source-capture routing and classified point stacks, U29 adds visible gated provisional presentation and same-HWND promotion, U28 proves the behavior with real native windows and corrects overstated verification claims, U30 publishes programmatic activation only from accepted frames, U31 converges per-window renderer surfaces, and U32 converges display topology plus event-driven client geometry. These are completion units, not post-plan candidates.

Each unit receives a focused commit after its tests and local review pass. Wide mechanical migrations are serialized even where model work could theoretically run in parallel.

### System-Wide Impact

- `crates/gpui`: test accessibility capture/action support; narrowly scoped inherited render context only if U7's prototype and independent-consumer proof hold; focus/tab-stop support needed by U3; authoritative subtree transform, presentation, announcement, anchor, reveal, and clip state spanning scene, input, focus/IME, accessibility, deferred work, and frame-journal replay; AppCell-adjacent native-event ingress and synchronous-query snapshots, run-loop wake/fair draining, reserved-window commit/rollback delivery, drag transport, first-presentation acknowledgement, and separate appearance/activation/owner window contracts.
- `crates/form`: validation generation/activity and derived status authority.
- `crates/ui_core`: pure focus/overlay policies, semantic descriptors including renderer-neutral live facts, reveal alignment vocabulary, tokens, and public contract boundaries.
- `crates/ui_components`: window runtime adapters, component migrations, recipes, typeahead session, federated contract/probe bindings, public callback breaks, live-region consumers, typed overlay-anchor conversion, and virtual reveal materialization.
- `crates/open-gpui-command`: call-site migration only unless a concrete command bridge defect is exposed; command ownership remains unchanged.
- `crates/devtools`: projection from real semantic/runtime authorities with redaction.
- `crates/gpui_docking`: immutable visual style resolution, private surface owner and revision/events, stable-item activation completion, platform-window mutation consumption, two-phase runtime effects, explicit window sessions, source-capture routing, presentation leases, and provisional viewport promotion/rollback while preserving retained graph/session architecture.
- `crates/gpui_wgpu`, `crates/gpui_windows`, `crates/gpui_macos`, and `crates/gpui_linux`: consume backend-neutral transformed scene and rounded-clip contracts, prove matrix/primitive ABI consistency, distinguish accepted draw, submitted present, non-empty presentation, renderer/device/surface failure, and native visibility on supported runners, and publish complete generation-bound display/client-geometry facts without partial enumeration or timer-driven reconciliation.
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

**Creation or presentation fails between pending and active state.** Staged synchronous creation can fail, pump a close callback, roll back its reserved window, or lose a renderer surface after an anchor token or presentation lease exists. Mitigation: U24 holds create-time callbacks until App/window commit or rollback, U26 activates the opening token only from the returned committed full `WindowId`, U25 separates accepted draw from non-empty presentation, and U29 holds a provisional role and source lease through renderer/device/surface compensation before any durable graph commit.

**Initial no-focus becomes permanent no-activation.** A single boolean can map a harmless initial-show preference to `WS_EX_NOACTIVATE`, leaving detached windows impossible to activate later. Mitigation: U25 separates one-shot appearance, lifetime activation/click, input acceptance, and owner/transient facts; native tests assert both non-stealing first show and later activation.

**Native ownership is mistaken for lifecycle authority.** OS owner/group semantics differ and may not cascade every shutdown case. Mitigation: U25 uses native ownership only for supported z-order/activation UX, while U26's generation-bound DockSurface session always performs explicit, surface-scoped, idempotent teardown.

**Captured drag routes stale, foreign, or occluded targets.** Screen coordinates, DPI, host scenes, full `WindowId`, or Dock surface/session ownership can change between preview and release, while a visible provisional or an unrelated top-level may cover the real host. Mitigation: U27 resolves a classified point hit stack and global coordinate conversion from the capture owner, passes through only the exact current provisional role, preserves ordinary/foreign opaque barriers, accepts only exact-session current same-surface hosts, classifies foreign-surface hosts as rejected rather than desktop fallback, locks `MouseUp` facts before readiness work, and treats cancel/close as barriers.

**The native point is reconstructed with a different DPI frame.** Win32 mouse messages carry signed client device coordinates, while a later `GetDpiForWindow` or logical-to-device round trip may observe the post-`WM_DPICHANGED` scale and move the route point. Mitigation: U27 captures one callback-scoped native physical frame before logical conversion, binds source geometry to it, embeds target geometry in the point observation, and rejects cross-frame reuse. Mixed-DPI tests deliberately change the current scale after input and require the original physical point to remain exact.

**A stable but incomplete window enumeration looks authoritative.** `EnumWindows` can repeat the same result while omitting non-desktop-app top-level windows, and its contract does not make a repeated list sufficient proof of the frontmost point hit. Mitigation: U27 binds the sampled point into the observation, stabilizes classification and geometry, independently verifies the first effective entry with `WindowFromPoint`, bounds and cycle-checks any Z-order walk, and returns `Unavailable` on every disagreement instead of exposing a hidden Dock host.

**A terminal route survives a panicking commit.** Marking a route terminal in place before invoking Dock/runtime work can strand the generation, retain previews, and reject every later drag when that work panics or reenters. Mitigation: U27 atomically claims and removes the exact route before effects, gives cleanup to a scope guard, and proves panic recovery plus next-generation registration without weakening exactly-once commit.

**A native callback arrives before the Dock drag generation binds.** Installing an event-receiving route with `generation: None` and filling it from deferred work creates a reentrancy window where `MouseMove` or `MouseUp` is silently rejected or attached to the wrong session. Mitigation: U27 installs GPUI active-drag and Dock consumer generation in one synchronous transition and injects callback reentrancy before any deferred effect can run.

**The provisional waits forever for incidental input to draw.** A hidden window whose payload mount only marks local state may not receive another mouse, activation, or expose event, reproducing the viewport that appears only after release. Mitigation: U29 invalidates every affected surface, requests the provisional bootstrap frame, wakes the run loop, and proves first non-empty presentation progresses with no further input.

**Provisional viewports leak, flash, or duplicate semantic ownership.** Repeated motion, failed first paint, target switching, an early `MouseUp`, delayed promotion, or shutdown can leave windows alive, expose an empty shell, settle twice, or render one payload as two focus/accessibility owners. Mitigation: U29 allows one U26-owned provisional role, one payload-subtree presentation lease, and one source semantic/focus proxy per drag/session generation; revokes the stale source subtree and waits for a committed proxy frame before mounting provisional content; separates readiness, route, live/final placement, lease, and release-latch axes; keeps the non-empty provisional continuously visible through exact client-geometry observations; bounds early desktop release with an injectable clock; rejects gated user input rather than replaying it; validates the complete next state before KTD32's first irreversible receipt; settles every later stage through exact forward receipts while retaining the gate; removes that gate only after the exact destination semantic frame is renderer-submitted; treats a destination terminal after the boundary but before that acknowledgement as one committed viewport-loss rather than source rollback; and requires renderer/surface quiescence before HWND terminal.

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

Owning-platform contracts:

- [Microsoft `WM_MOUSEMOVE`](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousemove) and [`WM_LBUTTONUP`](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup): capture keeps move/up delivery on the source HWND and supplies signed client-area coordinates.
- [Microsoft `ClientToScreen`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-clienttoscreen): client-to-screen conversion remains in device coordinates.
- [Microsoft `WM_DPICHANGED`](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged): window DPI may change as the window crosses monitors, so a later DPI query cannot reconstruct an earlier input frame.
- [Microsoft `EnumWindows`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows), [`GetWindow`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindow), and [`WindowFromPoint`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-windowfrompoint): enumeration is fallible and Windows 8+ limits `EnumWindows` to desktop-app top-level windows, while point-hit verification has its own hidden/disabled-window semantics.
- [Microsoft `WM_CAPTURECHANGED`](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-capturechanged): normal `ReleaseCapture` also produces capture-change, so that notification cannot replace an already locked release.

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
- `repo-ref/fret/crates/fret-launch/src/runner/desktop/runner/window_under_cursor.rs`
- `repo-ref/fret/crates/fret-launch/src/runner/desktop/runner/window_insert.rs`
- `repo-ref/fret/crates/fret-launch/src/runner/desktop/runner/win32.rs`
- `repo-ref/imgui/` docking branch at `4be08b1ecf77`
- `repo-ref/imgui/imgui.cpp`
- `repo-ref/imgui/imgui_internal.h`
- `repo-ref/imgui/backends/imgui_impl_win32.cpp`
- ImGui commit `a1c0836becbae04a0727d981b464f3a56867e2c1`, which changed the default to no viewport parent after z-order inference failures

The references inform behavior and ownership only. Their package layouts, APIs, and runtimes are not copied wholesale. The repository has no `docs/solutions/` corpus to inherit for App borrow reentrancy, cross-HWND capture routing, or Dock window-session teardown; the code, native reproduction evidence, prior plans, and exact ImGui clone are the grounding sources for U24-U29. ImGui contributes source capture, staged platform-window ownership, same-window reuse, and the evidence that default native parentage harms z-order inference, but its show-before-render/swap sequence does not establish Open GPUI's non-empty-present-before-show contract. Fret contributes the useful pattern of explicitly requesting redraw and waking the event loop after native insertion; it does not make its polling, cursor resampling, or last-hover fallback authoritative. Open GPUI deliberately retains its fail-closed opaque-barrier hit stack, typed generations, retained graph transactions, and explicit session teardown rather than copying ImGui's global moving-window state or foreign-window fallback.

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
| U27 | Captured native cross-window drag routing | GPUI active drag, platform point hit stack, Windows classification, Dock route | U26 |
| U29 | Live provisional presentation and same-HWND promotion | Dock payload lease, provisional role/gate, promotion saga | U27 |
| U28 | Owning-platform docking evidence | Windows native integration, Dock examples, CI, verification docs | U27, U29 |
| U30 | Accepted-frame programmatic activation | GPUI frame journal, activation handles, overlay focus restoration | U6, U13, U24, U28 |

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

The product surfaces already updated by U1-U10 are audited together, architecture decisions and migration notes are cross-linked, obsolete code is absent, and the existing gates cover the newly added GPUI/accessibility paths. U11 does not close the expanded plan: U12-U20 and U24-U32 own their own Gallery/example, documentation, and release-gate changes rather than hiding them in this prior-surface audit.

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
- Preserve visual-surface authority in complex consumers. Dock divider hit resolution groups root and each floating container independently, gives the last rendered floating container a blocking pointer boundary across its complete bounds, and forbids junction synthesis across surfaces. Raw splitter and composite-floating drags use the stable Dock host capture owner; standard GPUI payload drags use a stable source-element owner acquired by GPUI only after crossing the drag threshold, so `on_drag` requires a stable element ID. Terminal `PointerCancel` clears raw state, the owning payload runtime session, previews, anchors, and the exact captured-native route/transport generation even when a replacement frame removes the host subtree; GPUI keeps its window-owned active payload visible until every host observer has run so an independent host cannot consume cancellation before the owner. Single-tabs floating drags publish Dock payload state only after floating policy accepts the transient session, and any policy or geometry rejection retracts both the pending GPUI drag and capture.
- Keep `open-gpui-motion` renderer- and GPUI-neutral. It emits a fallible `MotionProjectionTransformSample`; a consumer that depends on both crates converts the checked sample to `SubtreeTransform`. Fake-clock tests cover intermediate and exact final projections, large valid endpoint ratios, and reduced motion resolving directly to identity without changing layout or silently falling back.
- Add a Gallery scenario with nested non-uniform scaling and translation around an explicit origin, containing real text, button/semantic activation, text input/IME, clipped scrolling, drag/pointer capture, tooltip or deferred content, and inspector/accessibility probes. The scenario is an executable interaction surface, not a static transform sample.

**Test scenarios**

- Pure/property tests cover identity, relative anchor-plus-offset origin resolution, the exact private `parent_resolved compose child_local` order, committed geometry point/vector round trips, transformed bounds, child-local translation behavior, large/small positive scale within the supported range, and rejection of NaN, infinity, zero, negative scale, non-representable reciprocal, composition overflow, scale underflow, multiply-add overflow, and inverse/backend-conversion failure.
- Runtime numeric-failure tests prove a locally valid but unrepresentable nested composition preserves layout and suppresses paint, hitboxes/listeners, pointer capture, focus/IME, deferred/cache entries, diagnostics geometry, and final AccessKit nodes as one subtree transaction; identity/clamp/partial-channel fallback is forbidden.
- Transaction tests prove a valid transformed frame runs its commit exactly once, a late-invalid frame runs discard instead, rollback/unmount absence expires the prior publication exactly once, and a subsequent valid frame can republish. Docking tests prove viewport route geometry and floating/drop interaction state are retracted after early failure, late failure, and host-subtree removal.
- Layout characterization proves transformed and identity-wrapped children have identical measured size, flex/grid placement, scroll extent, and sibling positions.
- Backend-neutral scene tests assert every primitive carries the same resolved transform, clipping/culling occurs in transformed space, and scale/translation is applied exactly once. Text and surface primitives are mandatory; a quad-only demonstration is insufficient.
- Pointer tests cover transformed hit/miss edges, nested transforms, overlapping z-order, cursor/hover, click local position, wheel and scrollbar behavior, drag/drop deltas, autoscroll, transform changes during pointer capture, and non-transformed siblings. Dock tests additionally prove floating chrome occludes root dividers, an overlapping floating divider wins without a cross-surface corner, top floating content blocks a lower floating title bar, policy and inverse-geometry rejection leave no GPUI payload, Dock runtime session, or capture, and window deactivation clears captured splitter, composite-floating, payload-floating, and captured-native route/transport generations. Host-subtree removal revokes both single-tabs floating and tab-item payload drags, while a two-host window proves cancellation reaches the owning payload runtime after non-owner observers.
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
- Define event-domain behavior before implementation. Frame requests and coherent move/resize/state facts may replace older pending facts in their domain. Activation, synthetic modifier, and hover edges are FIFO and non-coalescing because they carry cancellation and enter/leave semantics. Queue-eligible button edges and text input, pointer cancellation, asynchronous close-request/closed lifecycle facts, and mutation terminal observations are likewise FIFO and non-droppable. Must-immediate key, wheel, non-client, and other handler-disposition-dependent classes use the synchronous idle-only path. Pointer cancellation and close create barriers after which older input cannot run; close settles retained mutation observations and retires the generation before a reused ID can receive events.
- Bound each drain turn so an input or callback storm cannot starve the foreground executor. Preserve application ingress order and defined cross-window session barriers across partial drains, schedule another wake while work remains, and expose structured pending/delivered/coalesced/stale/closed dispositions to test diagnostics without retaining user input text.
- Change frame acknowledgement so an App-borrow conflict does not count as a successful paint. A native invalidation remains pending until GPUI accepts a draw/present request or explicitly re-invalidates and schedules it. Windows paint validation cannot permanently consume a failed callback; repeated frame callbacks for one generation may coalesce while guaranteeing a later presentation opportunity.
- Route placement/state, activation facts, hover, queue-eligible input, close, frame, and U20 mutation-observation callbacks through the typed ingress. Route must-immediate hybrid input through one dedicated synchronous AppCell entry that first respects older ingress barriers and rejects a busy App invariant instead of defaulting. No backend may keep a privileged direct path whose loss semantics differ.
- Make entity/controller/viewport-runtime operations return typed native effects. Release those subordinate guards, then apply model-owned create/open, remove/close, present, sibling-window update, and mutation work through the current `&mut App`. Capture initial presentation completion, `activate`, native window menu, interactive move, and interactive resize as the closed `PlatformWindowCommand` set with a weak backend dispatcher; enqueue them with full `WindowId`, release the outer `AppRefMut`, settle older event barriers, then drain commands synchronously and non-recursively. Commands enqueued by a command append to FIFO. Every dispatcher attempt terminates as `Accepted` or `Rejected`; only accepted initial presentation publishes completion, while its sole retry is bounded to two diagnosed attempts total and every other rejection is terminal. Before native creation, reserve the full `WindowId`; construct an inert native target, install every callback including must-immediate input, perform a synchronous fallible map that cannot pump hybrid input, sample coherent post-map facts, build and draw the root, then commit the registry and native snapshot exactly. Builder or initial-render closure retires the reservation without publishing close/last-window semantics. Only the committed transaction enqueues `CompleteInitialPresentation`; synchronous callback envelopes wait until commit or rollback and then deliver to the committed window or retire against the rolled-back ID. Initial show/focus work that can pump input runs only from that post-borrow command. `App::open_window` and fallible map remain synchronous; no arbitrary callback outbox or asynchronous open-window outbox is added.
- Fix the known tear-off failure path that closes a newly opened viewport while holding the runtime `RefMut`. Apply the same two/three-phase pattern to scene reconciliation, activation, source invalidation, close observers, and shutdown preparation: collect identity/generation, release the guard, obtain external facts or apply effects, then short-borrow to finalize only if the generation still matches.
- Establish the smallest reusable Windows real-HWND test support needed for reentrant create/show/paint/activate/close, reserved-window commit/rollback, and ingress observations. U25-U29 extend this same harness; U28 completes the scenario matrix and CI/subprocess hardening.

**Test scenarios**

- Deterministic App-borrow tests inject every asynchronous callback domain while an update is active and prove direct and queued delivery produce the same committed result, callback kind is observable, no event is reduced to a generic `RefCell already borrowed` log, and nested drain-time callbacks do not recurse. Synchronous-query tests prove committed hit testing and prevent-and-queue close intent. TestPlatform's command executor synchronously triggers at least one consumed and one propagated hybrid input after `AppRefMut` release, proves the exact handler results, asserts the App can be mutably borrowed at callback entrance, keeps a zero busy-invariant count, and proves nested command enqueue is FIFO rather than recursive. The real-HWND matrix repeats the owning-platform result contract. Any fixed fallback, delayed replay, or busy entrance fails.
- Ordering tests cover an older queued event followed by a borrowable callback, cross-window source/target/anchor causality, coalesced frame/move/placement facts, non-coalesced activation/modifier/hover edges and down/up/key/text/terminal observations, cancellation and close barriers, generation-bearing `WindowId` reuse, close settling pending mutation tickets, partial drains, wake coalescing, and exactly-once terminal delivery.
- Frame tests force a callback borrow conflict, observe that paint remains pending, then prove one later accepted non-empty presentation. Multiple invalidations coalesce without either losing the last request or drawing after close.
- TestPlatform and the minimal real-HWND harness cover callbacks arriving synchronously during create, show, mutation dispatch, activation, paint, and close rather than assuming every backend defers. TestPlatform and the real HWND remain hidden until accepted initial presentation; an injected first rejection retains intent, retries once, and emits exactly one completion only after acceptance. Reserved-window events deliver after commit and retire after rollback; synchronous close after commit follows normal ordered teardown.
- Construction tests prove post-map facts seed the root builder and first draw, initial presentation observes a committed full `WindowId` with `AppCell` idle, nested completion commands remain FIFO, map cannot dispatch hybrid input, and builder/initial-draw removal rolls back exactly without a close observer or last-window quit.
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

GPUI models creation-time appearance, lifetime activation/input policy, and native owner/transient relationships as independent facts. A detached viewport can appear without stealing focus, present its first frame, and later activate normally. The same final native profile can be installed when a provisional viewport is created; U29 owns its temporary generation gate and same-window promotion.

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
- On Windows, showing an owned viewport without initial activation must preserve the native owner relationship without activating, raising, or reordering the owner. The creation path uses an owner-preserving no-activate show sequence and never treats temporary native-parent manipulation as a committed ownership fact.
- Treat native owner semantics as z-order, activation, minimization/grouping assistance only. Closing the owner does not satisfy R28 by itself, and a backend without owner support does not weaken DockSurface's explicit teardown.
- Define staged appearance facts for windows that must not expose an empty shell: native window created without activation, root/callbacks installed, first frame accepted and presented, then shown under the requested appearance policy. The creation capability distinguishes presentation before visibility, after visibility, and protocols such as Wayland where the first buffer commit establishes visibility. No caller may equate native visibility with non-empty presentation.
- Define the ordinary detached Dock native profile as no initial activation, later programmatic/click activation, pointer input accepted, and peer top-level default ownership. DockSurface session lineage does not imply `transient_for`; explicit application ownership remains opt-in. Preserve the creation-time contract U29 consumes: a provisional can start with that same final lifetime native profile, so later promotion requires no live native activation, input, or owner transition. U29, not U25, owns the temporary session interaction gate and its atomic removal. Optional taskbar/topmost changes remain honest U20 capabilities and cannot block correctness.
- Implement KTD34 as one GPUI-owned programmatic activation ticket. `activate_window` dispatch returns or exposes the exact ticket instead of treating backend command acceptance as focus success. Native focus/foreground observations settle the matching target generation; a newer winner, explicit cancellation, activation-policy change, target replacement, rejection, unsupported backend, or native terminal settles every other terminal path. A temporary `NoWindow` observation after source loss remains pending until the matching gain or another terminal fact. Dock's activation host and source-focus restoration wait for both this native ticket and their existing GPUI focus completion.
- Keep platform facts coherent with U20: initial appearance is immutable creation history, lifetime flags use their own live or creation-only capabilities, owner relationship is observed or capability-qualified, and public getters never infer facts from requested options alone.

**Test scenarios**

- Contract tests prove every combination of initial appearance, later activation, click focus, pointer input, and permanent non-activation without deriving one fact from another, including coherent generation and no-partial-commit behavior for the two-field activation policy.
- Windows native tests show an ordinary detached viewport without foreground theft, assert absence of permanent no-activate style, then activate it by click and programmatic request. A deliberately permanent non-activating window remains non-activating.
- Activation tests cover dispatch accepted but OS focus denied, source loss before target gain, temporary `NoWindow`, another owned window winning, target replacement, policy revocation, close, explicit cancellation, and delayed stale gain. Only an exact target-generation positive observation succeeds; timer passage and API return cannot. Source-focus restoration remains incomplete until both native activation and GPUI descendant-focus completion settle.
- Windows native tests snapshot foreground ownership and front-to-back z-order before and after showing an owned no-initial-activation viewport, proving the owner is neither raised nor reordered and the final top-level owner relationship remains intact.
- Owner tests cover valid owner, closed/stale/self owner rejection, owner generation replacement, same-process top-level relationship, z-order/activation behavior where supported, and honest unsupported capability on other backends.
- Presentation tests distinguish native creation, visibility, first accepted frame, submitted present, and first non-empty presentation; a deferred frame or renderer/device/surface failure cannot be treated as presented and terminally settles pending opening work.
- Profile-compatibility tests prove a future provisional can be created with the final detached lifetime capabilities and later promoted without mutating its native activation, input, or owner profile. U29 owns session-gate, route-exclusion, focus/accessibility suppression, and same-window promotion behavior; optional unsupported taskbar/topmost changes remain covered here.
- TestPlatform, web, and every owning backend compile against the new option/param/fact contract; web and unsupported backends project ownership/activation limitations honestly, and migration scans prove the old overloaded `focus` path is absent.

**Deletion/replacement**

- Delete the overloaded creation-focus boolean and Windows mapping from no-initial-focus to permanent `WS_EX_NOACTIVATE`.
- Delete implicit "current active window" owner selection and Dialog-only owner special cases that bypass the typed relationship.
- Do not promise that native ownership cascades lifecycle, expose raw HWND/NSWindow/X11 handles, use `focus_on_click` as an alias for lifetime activation capability, or make correctness depend on a creation-only flag changing live.

**Unit gate**

- GPUI option/fact, TestPlatform, Dock profile, native appearance/activation/owner, migration, and documentation tests pass.
- Review confirms non-stealing initial show plus later activation, KTD34 generation-bound activation dispatch/observation/cancellation, loss-before-gain handling, explicit permanent non-activation, owner generation safety, first-presentation observability, a native profile compatible with later same-window promotion, and honest optional per-backend flag capability.

### U26. Add DockSurface Window Sessions And Deterministic Teardown

**Outcome**

Each facade-managed `DockSurface` owns one explicit anchor session and shutdown authority, while the viewport runtime remains the sole registry of committed, opening, and tear-off handles for that session. U29 must attach provisional handles to this same lease, registry, and close-ticket authority rather than create parallel ownership. An ordinary anchor close request is held until that surface drains its dependents; direct destruction and App shutdown still converge idempotently, and another DockSurface remains independent.

**Requirements**

- R1-R2, R15, R24, R26-R28.

**Primary files**

- `crates/gpui/src/app.rs`
- `crates/gpui/src/app/window_registry.rs`
- `crates/gpui_docking/src/surface.rs`
- `crates/gpui_docking/src/surface/owner.rs`
- `crates/gpui_docking/src/surface/viewport.rs`
- `crates/gpui_docking/src/surface/window_session.rs`
- `crates/gpui_docking/src/surface/activation.rs`
- `crates/gpui_docking/src/surface/viewport_readiness.rs`
- `crates/gpui_docking/src/lib.rs`
- `crates/gpui_docking/src/prelude.rs`
- `crates/gpui_docking/src/public_surface_tests.rs`
- `crates/gpui_docking/src/host.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/viewport_registry.rs`
- `crates/gpui_docking/src/viewport_registration.rs`
- `crates/gpui_docking/src/viewport_window_ownership.rs`
- `crates/gpui_docking/src/viewport_runtime.rs`
- `crates/gpui_docking/src/viewport_runtime_handle.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/close_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/route_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/scene_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_effects.rs`
- `crates/gpui_docking/src/viewport_runtime_status.rs`
- `crates/gpui_docking/src/surface_tests.rs`
- `crates/gpui_docking/src/surface_owner_tests.rs`
- `crates/gpui_docking/src/surface/window_session_tests.rs`
- `crates/gpui_docking/src/host_viewport_close_tests.rs`
- `crates/gpui_docking/src/host_viewport_lifecycle_tests.rs`
- `crates/devtools/src/docking.rs` and Dock DevTools tests/fixtures
- `examples/docking-minimal/`, `examples/docking-native/`, and `examples/docking-multiviewport/`
- Dock facade, lifecycle ADR, migration, and verification documentation

**Behavioral work**

- Add a private `window_session` authority that owns only `Vacant/Opening/Active/ShuttingDown/Closed`, a monotonic session generation, exact opening/anchor identity, shutdown cause, and terminal ticket snapshot. The viewport runtime remains the sole handle authority for committed, opening, and tear-off windows; the owner receives a teardown snapshot but never mirrors those handles. U29 extends this authority with its explicit provisional role and ticket.
- Rename the existing stateless `DockSurfaceViewportSession` facade to `DockSurfaceViewports`, including `DockSurface::viewports()` and public exports, without a compatibility alias. Publish a read-only `DockSurfaceWindowSessionStatus` and typed phase/reason projection for DevTools. Return typed primary-open conflicts/rollback/not-closed outcomes instead of erasing them into a generic `GpuiResult`, and add a session-inactive reason to viewport unavailable/readiness results. Keep the opaque admission lease crate-private.
- Give every surface-created host one private role: `Embedded`, `PrimaryAnchor { authority: Opening(token) | Active(lease) }`, or `ManagedViewport { lease }`. Embedded `host_view` renders without creating an anchor or facade-owned teardown; applications that want managed native viewports use `open_primary_window`, while advanced custom ownership uses the low-level runtime. The `Opening` primary's initial draw may render but cannot register a route, activation host, runtime mapping, or active session. After `App::open_window` returns a committed full `WindowId`, validate the token and identity, atomically replace the host authority with `Active(lease)`, and refresh so later scene publication registers the primary under that lease.
- Make `open_primary_window` reserve an `Opening` token before applying the U24 typed create effect. Native callbacks raised during construction remain in AppCell ingress until commit/rollback and never activate the session. Pre-commit create/map/initial-draw failure, a before-visibility presentation failure, synchronous close, App shutdown, or cancellation settles `Opening -> Closed` and removes every tentative mapping. Once registry commit consumes the token and activates the exact lease, a later presentation terminal becomes `ShuttingDown(PresentationFailed)` and uses the same forced teardown/convergence path; it never tries to settle or recreate the Opening token. A duplicate opening, active anchor, or not-yet-closed predecessor returns its specific typed outcome.
- Require one opaque exact `DockSurfaceWindowSessionLease` for every facade-owned viewport open/restore, ownership record, registration, host-scene publication, activation host/request, drag/route reservation, mutation dispatch, and observation. Runtime records store this lineage in addition to their existing registration/window generations. An old lease always fails closed against a replacement generation, including the case where both optional anchor fields would otherwise compare as `None`; independent low-level runtimes carry an explicit unmanaged lineage rather than forging a surface lease.
- Install the surface close-request guard on the primary during construction. An ordinary request for the exact current anchor transitions to `ShuttingDown`, prepares shutdown, and returns `false` so native destruction cannot overtake dependents. One runtime operation validates and freezes the active lease, cancels drag/preview/tear-off/focus/activation/mutation work, retires the generation's mappings, and builds a deduplicated exact-window snapshot across committed adapters, opening ownership, and tear-off reservations. U29 extends that same snapshot with provisional handles and tickets. The operation returns pure typed effects after terminally settling pending activation/completion work; when dependent tickets converge, one effect explicitly removes the still-live exact anchor. Repeated requests remain vetoed/idempotent, while programmatic/native destruction that bypasses the guard enters the same cleanup through its exact native terminal.
- Split close dispatch from native convergence. `claim_close_dispatch` is exactly-once intent only, while the matching native `Closed` event settles terminal state. A failed exact-handle update remains pending because neither dispatch failure nor logical registry removal proves native destruction. Establish each anchor-to-child native-retirement dependency before child runtime admission so native ordering cannot fail late during teardown. Apply effects only after every owner/controller/runtime/entity borrow is released. Forced teardown bypasses `Prevent` and `MergeBack`, suppresses ordinary graph merge/focus restoration, closes dependents before releasing queued anchor retirement, then admits `Closed` only when the anchor and all snapshot tickets are native-terminal and the current-generation runtime is empty.
- Make facade close observers order-independent. Exact anchor close freezes the session first; once `ShuttingDown`, the runtime observer may perform only forced terminal cleanup and cannot run ordinary merge-back or focus recovery. Consolidate facade orchestration behind the window-session authority where practical while preserving the existing unmanaged-runtime observer path.
- Integrate App shutdown explicitly. The synchronous pre-clear portion of `on_app_quit` freezes the session, snapshots ownership, releases borrows, and applies forced close without the ordinary deferred close applicator. GPUI's native-retirement coordinator detaches each still-live platform owner from the logical registry and retains it through the exact native terminal, including terminals published after registry clear. Test and document this lifecycle rather than treating registry removal or a missing logical close observer as terminal proof.
- Close only windows carrying the current surface lease. Multiple surfaces, independent low-level runtimes, ordinary application windows, and later generations remain untouched. Preserve U19 commit-only revisions: shutdown intent and preview cancellation are non-durable, one authoritative durable cleanup publishes at most once, and late observers publish nothing. Integrate U25 transient ownership only as a supported native grouping hint. Dock never calls `cx.quit`; GPUI's normal last-window policy decides exit.
- Convert `docking-native` from its direct-runtime primary and example-level quit workaround to the facade-first `open_primary_window`/`viewports()` path. Keep any unmanaged example path explicitly labeled advanced. Use `docking-multiviewport` for two-surface isolation and expose the same session phase, generation, shutdown cause, anchor, owned-window counts, and terminal convergence in DevTools.

**Test scenarios**

- Session tests cover `Opening` reservation, committed `WindowId` activation, native create success, pre-commit create/map/initial-draw and before-visibility presentation rollback without a stale Dock mapping, post-commit presentation failure through `Active -> ShuttingDown -> Closed`, synchronous close during creation, queued callbacks, duplicate/open-active/not-closed typed outcomes, explicit reopen with a new generation, stale old-anchor/viewport callbacks, and exact-lease admission before `Active`, during shutdown, and after G2 becomes active. A stale G1 operation with absent optional anchor fields must not pass G2 admission.
- Host-role and public-surface tests prove embedded render never creates an anchor, an `Opening` primary cannot register route/activation/runtime facts, a managed viewport publishes only under its current active lease, `DockSurfaceViewports` replaces the misleading old facade name without an alias, and DevTools/readiness expose typed inactive and terminal status.
- Shutdown tests create committed and opening windows, begin drag/activation/mutation work, request anchor close, and prove the first/repeated close request is vetoed after freeze, deduplicated all-registry snapshotting, borrow-free effects, exactly-once cancellation, renderer/presentation retirement before native terminal, dependent-terminal-before-explicit-anchor-removal ordering, complete ownership retirement, no focus restore, and idempotent repeated/late close. A direct anchor removal that bypasses the guard exercises the same exact native-terminal cleanup path. U29 adds the equivalent provisional-window cases through the same snapshot and ticket authority.
- Policy tests prove ordinary child-window close still honors `Prevent`, `MergeBack`, and close-request semantics, while anchor shutdown force-closes those same windows without merging them back or being blocked.
- Isolation tests run two Dock surfaces plus an independent low-level runtime; closing one anchor leaves the other surface's anchor, viewports, drag route, revisions, and activation usable.
- Failure-order tests cover the OS destroying a child first, tear-off commit failure, both close-observer orders, callback-opened work, delayed terminal observation for one old HWND, failed exact-handle updates remaining pending until native terminal, rejected reopen while the anchor/registry/tickets remain incomplete, and successful reopen only after deterministic convergence. U29 owns anchor close during provisional creation or promotion.
- App-shutdown tests cover both `Opening` and a fully `Active` session with anchor and committed viewports. They prove pre-clear freeze/close, retained-owner settlement from exact native terminals after logical registry clear, no permanent `ShuttingDown`, and no cross-surface or unmanaged-runtime corruption. U29 adds the provisional-alive shutdown case.
- Migrate facade tests that currently open viewports from `Vacant`: managed-lifecycle tests first open a primary, inactive-readiness tests assert typed rejection, and pure close-policy tests use an explicit unmanaged runtime fixture. Example tests close the real primary through the facade and assert owned viewport/process convergence without `cx.quit`.

**Deletion/replacement**

- Delete `DockSurfaceViewportSession`, window ownership represented only by unscoped `WindowId` sets, any surface-owner mirror of the viewport runtime handle registry, first-render anchor inference, activation-only close cleanup, swallowed forced-close update failures, and tests that require detached facade viewports to survive the current primary by default.
- Delete example-specific primary-close `cx.quit` behavior used to mask absent surface teardown.
- Do not make all low-level Dock runtimes application-global, infer an anchor from the first rendered host, expose the private admission lease, or delegate forced shutdown to per-viewport close policy.

**Unit gate**

- Surface owner/session, viewport close/lifecycle, reentrancy, example, public API, migration, and documentation tests pass.
- Review confirms non-overlapping state ownership, host-role separation, exact-generation admission, synchronous registry-commit activation without initial-render registration, explicit reopen only after anchor/runtime/ticket convergence, freeze/snapshot/release/apply shutdown ordering, App-shutdown settlement, forced-policy behavior, multi-surface isolation, commit-only revisions, and absence of Dock-owned application exit.

### U27. Add Captured Native Cross-Window Drag Routing

**Outcome**

A source HWND that owns native pointer capture transports one Dock drag generation across application windows using an immutable, current screen-space fact. Every routed preview and release is derived from a classified point hit stack, locked terminal facts, and exact generations without target raw mouse delivery, polling, or App/Dock borrow reentry.

**Requirements**

- R1-R2, R15, R25-R29.

**Primary files**

- GPUI active-drag and pointer-session code in `crates/gpui/src/app.rs`, `crates/gpui/src/window.rs`, `crates/gpui/src/window/pointer_session.rs`, and input dispatch modules
- `crates/gpui/src/app/native_captured_drag.rs`
- `crates/gpui/src/app/cell.rs`
- `crates/gpui/src/app/native_event_ingress.rs`
- `crates/gpui/src/platform.rs`
- `crates/gpui/src/app/native_platform_commands.rs`
- `crates/gpui/src/app/test_context/pointer_session_tests.rs`
- `crates/gpui_windows/src/events.rs`
- `crates/gpui_windows/src/platform.rs`
- `crates/gpui_windows/src/window.rs`
- `crates/gpui_windows/src/native_test_harness.rs`
- `crates/gpui_docking/src/interaction.rs`
- `crates/gpui_docking/src/native_captured_drag.rs`
- `crates/gpui_docking/src/drop_runtime.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/host_interaction_outcome.rs`
- `crates/gpui_docking/src/host_render_actions.rs`
- `crates/gpui_docking/src/host_outside_release.rs`
- `crates/gpui_docking/src/host_viewport_drop.rs`
- `crates/gpui_docking/src/viewport_target_context.rs`
- `crates/gpui_docking/src/surface.rs`
- `crates/gpui_docking/src/surface/window_session.rs`
- `crates/gpui_docking/src/surface/window_session_tests.rs`
- `crates/gpui_docking/src/surface/viewport_readiness.rs`
- `crates/gpui_docking/src/viewport_runtime_status.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/route_ops.rs`
- Dock host interaction, route, render, and lifecycle tests
- `examples/docking-native/` and `examples/docking-multiviewport/`
- `docs/adr/0002-docking-gpui-integration.md`
- `docs/architecture/docking-architecture-audit-20260609.md`
- `docs/adr/0021-open-gpui-interactive-subtree-transform-authority.md`
- `docs/ui/migration-v0.3.md`
- `docs/verification.md`
- `crates/gpui_docking/README.md`

**Behavioral work**

- Extend GPUI's active-drag boundary with one typed captured-pointer transport. The source capture owner turns each native move/up callback into one physical frame containing the signed native client point, exact client-to-screen `Point<DevicePixels>`, coherent source geometry, button or terminal kind, and point-scoped classified native hit observation under one drag generation and ingress sequence. `MouseExitEvent` remains local hover invalidation and cannot publish a captured movement fact. Logical GPUI coordinates remain an input-dispatch projection and are never multiplied by a later DPI sample to reconstruct routing facts. The transport does not inject arbitrary raw input into another window or transfer native capture.
- Add one private AppCell-owned `NativeCapturedDragOutbox` at the input transaction boundary. U24's native ingress passes the original, non-forgeable application sequence into the `WindowUpdateTransaction`; the source callback may only append an immutable drag fact to that outbox. Before the drag listener, GPUI reserves a non-forgeable `NativeCapturedDragStartReservation` and exposes its read-only exact generation through `DragStartGeometry`. Dock returns one `PreparedNativeCapturedDragConsumer` whose route already stores that non-optional generation but remains unable to receive facts. With no intervening user callback or platform-command pump, `start_active_drag` commits the active authority, outbox consumer, and prepared route activation as one callback-free transition. Listener panic, mismatched payload/source, rejected start, or commit failure synchronously revokes the reservation, prepared registration, active drag, and exact pointer capture; deferred work may schedule effects but may not bind a generation. After the window transaction and outer `AppRefMut` are released, AppCell drains the outbox to that exact generation-bound consumer while preserving the original sequence. Move facts may coalesce only under the declared drag generation, while release, cancel, capture-change, close, and consumer replacement remain non-coalescing barriers. A missing, stale, or unregistered consumer retires the fact explicitly instead of redispatching it under a new sequence.
- Add a point-scoped platform hit-observation capability beside the legacy application-window list. `Available` owns the sampled physical point; each `RegisteredApplication(full WindowId)` entry owns one coherent `PlatformWindowPhysicalGeometry`, while `OpaqueBarrier` owns checked point coverage. A caller cannot attach a stack to another point or reconstruct geometry from independently sampled bounds and scale. Missing, malformed, cross-point, or incomplete observations are a typed route-capability gap, never permission to look through an unknown top-level. The backend advertises the capability only after sampled-point ownership, coherent geometry, checked coverage, complete-through-terminal classification, and independent verification all exist; every intermediate implementation keeps the capability false and returns `Unavailable`. The narrow consumer is Dock routing; it is not a general cross-window event or global-position API.
- Implement the Windows observation as a bounded, cycle-safe front-to-back classification at the sampled physical screen point. Stabilize the relevant HWND ordering, registration generation, visibility/cloak state, point coverage, client origin, client extent, and DPI, then independently verify the first effective entry with `WindowFromPoint`; when U29's exact provisional is input-transparent, verification applies to the first entry after that one allowed pass-through. `EnumWindows` is only one fallible input because Windows 8+ omits non-desktop-app top-level windows; repeated identical enumeration cannot by itself prove completeness. Any verifier mismatch or observation drift returns `Unavailable`. Preserve ordinary, unregistered, and foreign-process top-level windows as opaque barriers. Match a registered top-level HWND to its exact full `WindowId` before child-only root normalization; never normalize through `GW_OWNER`, which would collapse registered siblings into the anchor. Skip only destroyed, minimized, or demonstrably invisible entries. Keep the observation in Win32 physical desktop coordinates without upgrading the backend's broader WindowLocal logical-position contract or dividing each window's global origin into a different logical coordinate space.
- Resolve every Dock route from the fact's full `WindowId`, exact U26 `Active` lease, locked physical point/coverage, immutable target geometry embedded in that observation, current committed host-scene generation, and input eligibility. Convert from the physical screen point to local logical `Pixels` only after selecting the target and only through that embedded `PlatformWindowPhysicalGeometry`; resolution must not re-query current DPI, client bounds, or client origin. A same-surface current host is eligible; a foreign-surface registered host is `ForbiddenTarget`; a registered window with no current eligible scene and every opaque entry terminates lookup as a desktop barrier. Until U29 adds the provisional role, nothing is transparent. A later U29 provisional may be passed through only when its exact role, drag, and session generations match.
- Route preview facts to the target host through Dock's existing interaction authority without assuming that HWND received `MouseMove`. Source and target redraw scheduling use U24; one current route owns an exact source/target feedback pair, while each runtime only validates its complete `DockViewportHostSceneFrame` and executes the corresponding projection. Target close clears both exact projections but leaves the source route active for the next fact. Prepaint candidate state retains the last committed frame token; commit replaces it and discard removes only that exact frame, so stale registration- or frame-generation cleanup cannot remove a replacement scene or feedback. A foreign-surface target renders generation-matched rejected feedback in the foreign host and provisional/source surfaces but remains ineligible for commit. Existing direct-target listeners remain local behavior only and lose cross-window authority.
- Make `MouseUp`, Escape, `PointerCanceled`, capture change/cancel mode, source deactivation, source close, anchor shutdown, and drag-generation replacement terminal inputs to one source-owned route state machine. `MouseUp` locks the sampled point, hit observation, candidate, host-scene/session/drag generations, and ingress sequence before generic drag/capture cleanup. Terminal claim atomically removes the exact route before any user/runtime commit effect runs; a scope guard retires previews, subscriptions, and capture state even if resolution or commit panics, so a failed effect cannot leave an unreplaceable terminal route. Windows callback panic recovery synthesizes cancellation only when a real pointer session remains active and no terminal was already reserved; hover panic, completed `MouseUp`, and reentrant capture loss cannot create ghost or duplicate cancellation. A later `WM_CAPTURECHANGED` from normal release may clean up but cannot cancel or replace that locked release. U29 may revalidate liveness at the locked point only; it may not read a later pointer position.
- Bind every route candidate, preview, release fact, and post-borrow effect to the exact U26 `Active` lease. Session shutdown snapshots the exact source `WindowId`, drag generation, route, projections, windows, and close tickets, then releases all owner/runtime borrows. GPUI cancels only that matching active drag/native capture/outbox generation before Dock cleanup and U26 dependent-window teardown. DockSurface retains the first cleanup panic while still applying all remaining cancellation and close effects. A failed dependent close dispatch returns its exact ticket to pending for post-borrow retry; delayed G1 work cannot cancel G2 or leave G1 permanently `ShuttingDown`. Embedded and unmanaged hosts participate only through their declared low-level ownership contract.
- Replace the 16 ms outside-release poll with source transport terminal facts and remove its interaction state, task control, rendered-release requests, tests, and documentation. Delete last-hovered-viewport release authority and every raw target-window cross-window release path; retained target listeners serve only local synthetic drag behavior and cannot authorize a native route. Preserve release-time desktop tear-off only as a temporary U27 destination after the locked route has resolved; U29 replaces that fallback with pre-release provisional presentation. No direct native open/show/move/close, target update, or refresh occurs while a source Window, Dock entity, controller, or runtime `RefCell` borrow is held; those operations use U24's post-borrow effect/command boundary and revalidate their generation on return.
- Project the new hit-observation capability through Dock readiness and runtime status without claiming broader live window-position support. Captured requests carry only the locked transport facts and declared capability; they never re-sample app hover, focus, window stacks, current cursor position, or last-hover state.

**Test scenarios**

- Captured-routing tests inject movement and release only into the source window while target raw input remains absent. A current same-surface target receives routed preview and exactly one release after physical-screen-to-target-local conversion; a direct target-window injection is retained only as a model-level local-listener test.
- Atomic-start tests synchronously reenter native movement/release both from inside the drag listener and immediately after its start commit, before any deferred effect can run. Listener-internal reentry cannot enter the reserved generation because neither authority is active; post-commit reentry observes both the active GPUI authority and exact Dock consumer. No optional generation, transient missing consumer, later bind, dropped release, or cross-generation adoption is possible. Listener rejection or panic proves the prepared route, active drag, and pointer capture are synchronously revoked.
- Point-observation tests cover overlapping windows from the same and different Dock surfaces, ordinary application and foreign-process barriers, visible registered windows without eligible host scenes, registered siblings retaining distinct full `WindowId` values despite native ownership, child-only normalization, destroyed/minimized/invisible entries, exact generation replacement, negative physical screen coordinates, and mixed-DPI monitor boundaries. They reject sampled-point reuse, independently mixed client bounds/scale, overflowed coverage, a stable enumeration that disagrees with `WindowFromPoint`, registration or geometry changes between classification passes, bounded-walk cycles, and any incomplete scan. The mixed-DPI case proves target-local conversion uses the geometry embedded in the chosen entry and never subtracts per-window logical global origins.
- Native physical-frame tests derive the global point from signed callback client device coordinates, then change the platform window's current DPI and geometry before route consumption. The captured global point and source geometry remain unchanged, the hit observation is queried at exactly that point, and no logical-to-device reconstruction occurs. Missing native physical frames fail closed rather than sampling `GetCursorPos` after `MouseUp`.
- Event-classification and panic tests prove `WM_MOUSELEAVE` publishes no captured movement or route change, an uncaptured hover callback panic creates no pointer cancel, a completed `MouseUp` panic creates no duplicate cancel, and reentrant capture loss followed by an outer panic preserves one terminal reservation.
- Route eligibility tests cover resize/move/DPI change, host-scene/session replacement, target close just before release, foreign-surface rejected feedback/cancellation, opaque barriers, stale local points, and last-preview rejection. An opaque window over a Dock host proves the hidden host receives neither preview nor drop; a foreign-surface host proves rejected feedback appears but cannot commit or degrade to desktop promotion. Target close removes the exact source/target marker pair without retiring the route, while delayed G1 registration or same-registration frame cleanup preserves the current G2 scene, projection, and source route.
- Locked-release tests move the pointer after `MouseUp` but before delayed consumer work or readiness completion and prove the original point, observation, candidate, and generations remain authority. A source `MouseUp` listener that replaces host scene G1 with G2 after the release locker runs proves the post-borrow consumer cannot re-hit-test into G2: it may commit only the frozen candidate under a semantically identical current frame or fail closed. Renderer-only hitbox-token replacement is accepted only when registration, binding, runtime context, bounds, interactive geometry, and complete routing facts remain identical; registration, topology, geometry, or fact changes fail closed. A normal release followed by `WM_CAPTURECHANGED` proves capture cleanup cannot convert release into cancellation; Escape/cancel remain ordered barriers. A forced resolver or commit panic proves the exact route is already detached, cleanup still retires preview/capture state, and a later drag generation can register normally without replaying the failed terminal.
- Borrow-boundary tests synchronously pump move/up/close/capture-change callbacks while source input, target updates, and post-borrow effects interleave. They prove the AppCell outbox preserves the original ingress sequence across release of `WindowUpdateTransaction` and outer `AppRefMut`, no nested `RefCell` borrow or newly sequenced redispatch occurs, no terminal is dropped, and no native open/show/move/close runs under a live Dock/runtime borrow. A first dependent-close dispatch that encounters a busy exact window returns to pending, retries after release, and reaches native terminal before anchor close. Missing/stale consumer, consumer replacement, App shutdown, and queued close barriers each retire or deliver the exact fact once.
- Terminal tests cover Escape, pointer cancel, native capture loss, source deactivation, source or anchor close, replacement drag generation, repeated terminal callbacks, and source-close-before-capture-release. Each leaves no capture, route preview, pending route fact, or stale desktop fallback.
- Multi-surface and shutdown tests prove route/session isolation, paired target-close cleanup/re-resolution, exact GPUI drag/capture/outbox cancellation before U26 dependent-window cleanup, retryable close dispatch, and panic-safe convergence. A capture release is prepared before its first dispatch can be delayed, all retries reuse that platform pointer-session snapshot, saturated rejection converges through the exact native-window terminal, and a failed native destroy retains its owner until retirement succeeds. A stale G1 shutdown cannot cancel G2, and a cleanup panic cannot stop G1 dependents and anchor from reaching terminal or affect a second surface.
- Authority-deletion tests and scans prove `host_outside_release`, outside-release poll state/tasks, rendered-outside-release requests, raw cross-window target listeners, last-hovered-viewport release selection, and their stale documentation are absent from production routing. Existing direct-target VisualTest flows are rewritten or relabeled so they remain model-level tests and no longer claim to reproduce Win32 capture transport.

**Deletion/replacement**

- Delete the assumption that a target HWND receives raw mouse movement under source capture, the complete outside-release polling/rendered-release state machine, app-only stack filtering that makes opaque ordinary/foreign windows transparent, and last-valid/last-hovered-preview release fallback.
- Delete target-local listeners as cross-window routing authority and any owner-root normalization that aliases a registered sibling viewport to the anchor.
- Delete stale ADR, architecture-audit, migration, verification, and README claims that identify polling, target raw input, or last-hover state as current native route authority.
- Do not expose a general cross-window event injection API, copy ImGui's global moving-window context, transfer native capture between HWNDs, or introduce live provisional content/graph mutations before U29.

**Unit gate**

- GPUI pointer-session/platform, Windows classification, Dock interaction/route/lifecycle, multi-surface, example, migration, and documentation tests pass.
- Review confirms one synchronously generation-bound source transport, AppCell outbox preservation of the original ingress sequence across both borrow boundaries, callback-scoped move/up facts without hover resampling, point-bound hit observations with coherent embedded geometry and checked coverage, capability false until independent Windows frontmost verification is complete, full-`WindowId` classification, fail-closed unavailable/opaque handling, exact-frame scene and paired-feedback cleanup, visible foreign-surface rejection, `MouseUp` latching, exact GPUI cancellation, retryable/panic-safe surface teardown, complete poll/rendered-release/last-hover authority deletion, and no native effect under a live source/Dock/runtime borrow.

### U29. Add Live Provisional Presentation And Same-HWND Promotion

**Outcome**

After a Dock payload crosses the live-undock threshold, one U26-owned provisional viewport renders the live payload before normal release without committing durable topology. It stays visible and gated while routing changes, then is promoted or transferred through one exact forward-only journal, or compensated before that journal crosses its irreversible boundary.

**Requirements**

- R1-R2, R15, R24, R27-R30.

**Primary files**

- `crates/gpui/src/window.rs`
- `crates/gpui/src/platform.rs`
- `crates/gpui/src/app.rs`
- `crates/gpui/src/app/window_registry.rs`
- `crates/gpui/src/platform/test/window.rs`
- `crates/gpui/src/app/test_context/pointer_session_tests.rs`
- `crates/gpui_windows/src/events.rs`
- `crates/gpui_windows/src/directx_renderer.rs`
- `crates/gpui_windows/src/window.rs`
- `crates/gpui_windows/src/platform.rs`
- `crates/gpui_wgpu/src/wgpu_renderer.rs`
- `crates/gpui_docking/src/drag.rs`
- `crates/gpui_docking/src/interaction.rs`
- `crates/gpui_docking/src/drop_runtime.rs`
- `crates/gpui_docking/src/graph.rs`
- `crates/gpui_docking/src/graph_canonical.rs`
- `crates/gpui_docking/src/graph_op_validation.rs`
- `crates/gpui_docking/src/host.rs`
- `crates/gpui_docking/src/host_render_session.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/render_tabs.rs`
- `crates/gpui_docking/src/surface.rs`
- `crates/gpui_docking/src/viewport_window_ownership.rs`
- `crates/gpui_docking/src/viewport_runtime.rs`
- `crates/gpui_docking/src/viewport_runtime_handle.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/close_ops.rs`
- `crates/gpui_docking/src/viewport_runtime_handle/route_ops.rs`
- `crates/gpui_docking/src/viewport_tear_off.rs`
- `crates/gpui_docking/src/viewport_tear_off_move.rs`
- Dock render, route, tear-off, surface lifecycle, accessibility, and native fixture tests
- `examples/docking-native/` and `examples/docking-multiviewport/`
- `docs/adr/0002-docking-gpui-integration.md`, `docs/architecture/docking-architecture-audit-20260609.md`, `docs/ui/migration-v0.3.md`, `docs/verification.md`, `crates/gpui_docking/README.md`, and affected release documentation

**Phase-zero native proof gate**

- Before implementing the general lease/session machinery, extend the U24 real-HWND harness with a narrow vertical slice that proves hidden non-empty presentation, generation-bound no-activate reveal, native hit transparency, same-HWND provisional-to-committed role conversion, one asynchronously changing retained payload, one renderer/surface-bound payload, and renderer quiescence before native terminal. The proof must exercise the production window and renderer paths; direct messages and `TestPlatform` fixtures remain supporting simulation only.
- Provision the runner contract first: the required label is `open-gpui-windows-native-interactive-ephemeral`, ownership is assigned to the Windows backend CODEOWNERS, the sentinel proves interactive desktop, compatible integrity/UIPI, system pointer injection, WndProc receipt, capture transitions, renderer/GPU availability, and virtual-screen bounds, and the job serializes global cursor ownership. Missing runner capacity is an infrastructure failure that blocks U29 rather than a skip. Record the owner, escalation path, and sentinel command in `docs/verification.md` and the workflow before this gate can pass.
- Use the vertical slice to falsify KTD30's lease premise. A frozen provisional is acceptable only if it can track asynchronous retained content and the representative surface-bound payload, preserve continuous non-empty presentation, and perform same-HWND promotion without another payload/semantic handoff. Otherwise retain the live payload lease and record the failed alternative as proof, not as a permanent second implementation.

**Behavioral work**

- Add an explicit private `ProvisionalViewport` ownership and host role. Before opening the HWND, register a pending provisional-opening slot under the exact U26 `Active` lease with one opening generation and cancel-on-return state. Anchor/App shutdown freezes and cancels that slot and cannot reach `Closed` until it settles. If `open_window` returns after cancellation, bind the full `WindowId` to its native terminal and presentation-shutdown tickets, then compensate it without admitting presentation, route, or promotion facts. A still-current return converts the slot into the provisional role before admission. Include every pending or committed ticket in the session's shutdown snapshot and forced close/terminal convergence. The role is intentionally absent from committed Dock viewport registration, host-scene routing, activation, persistence, and durable graph topology until the final promotion transaction converts the same full `WindowId` into a committed viewport role.
- Add one `DockLiveUndockSession` as the sole owner of the payload presentation lease, immutable source presentation snapshot, source-proxy commit barrier, route feedback, readiness facts, locked release fact, provisional role/handle reference, prepared promotion, presentation-shutdown ticket, and compensation saga. `DockInteractionRuntime` may resolve host-local scenes but cannot independently release, lease, open, or close the provisional. Existing outside-release, tear-off, and payload-drag state is folded into this authority rather than kept as parallel release machines.
- Converge GPUI presentation rehosting behind one opaque deep session. GPUI owns phase transitions, exact receipt pairing, ordinary source settlement, source-loss abandonment, and terminal record retirement; Dock supplies resolved roots and endpoint-specific rendering or durable-commit intent, but does not assemble `cancel -> finish` or invalidation-disposition protocols itself. The KTD32 promotion executor may consume one opaque, single-use provider token through `prepare -> can_commit -> commit`; that token cannot expose or duplicate GPUI's phase machine. A prepared session has one mandatory compensation path until authority crosses into the executor's `ForwardOnly` journal or transfers to payload recovery, and every post-prepare error consumes or deliberately transfers that obligation. If a Host endpoint disappears after provider finish but before Host accepted-frame acknowledgement, release only the exact still-stable batch members and preserve any replacement generation. Raw provider phases and single-use receipts remain module-private wherever Dock does not need them to render the current projection. No second Dock-side identity comparator, terminal table, or compatibility path may define GPUI presentation authority.
- Replace the broad promotion fields in `DockLiveUndockRuntime` with one private deep promotion executor governed by KTD32. The live-undock reducer submits an exact request and receives domain outcomes; it does not match provider, Graph, viewport, Host, retained-visual, surface-publication, native-effect, or lower-retirement phases. Same-window and Host routes use one stage model with route-specific receipts. Reentry observes a parked or in-flight exact stage and requests a wake without executing it twice. Shutdown claims `Abortable` work for compensation or attaches its dependency to the existing `ForwardOnly` journal; it never starts a parallel cleanup algorithm.
- Make every checked DockGraph mutation follow KTD33. Capture pre-existing staging root identities before the first write, use the old dependency closure only as a deletion guard, perform all affected-space rewrites without physical subtree deletion, and run one final mark-and-sweep from live roots plus surviving staging roots and their current dependencies. Detached old dependencies and speculative nodes are reclaimed; public `insert_node` followed later by `set_root` remains valid. Builder/import canonicalization may request an explicit global sweep only at its own complete-graph boundary.
- Define a payload-subtree presentation lease for every `DockDragPayload` form: item, tabs, and floating subtree. Lease activation immediately marks the old source payload subtree ineligible for input, focus, and AccessKit actions. The source retains its exact last committed payload visual as a frozen, non-interactive, non-semantic presentation until the matching provisional reports a current non-empty visible presentation; the generation-bound handoff then retires that frozen visual. The source commits an immutable semantic/focus proxy frame and reports `SourceProxyCommitted` under the exact source window, frame, payload, and lease generations. Only after that barrier may the hidden provisional mount the real payload and become its sole live renderer. Every committed frame during the handoff has at least one visible payload representation, but there is always only one live input and semantic owner. Source close, failed proxy draw, generation mismatch, removal, graph mutation, or incompatible application action either rejects against the live lease or cancels it before mutating; final preparation revalidates the exact source payload snapshot and generation.
- Define `SourceSemanticFocusProxy` as an exact AccessKit contract, not merely a marker. It owns one stable proxy node identity derived from the source slot and payload instance but distinct from every payload descendant; exposes a `Group` role, the source container's accessible name, no editable value, the frozen source presentation bounds, and no user actions while gated; and becomes the tree focus only when the drag-start snapshot contained the focused payload descendant. The `SourceProxyCommitted` update atomically removes the payload subtree and installs this proxy, while the gated provisional exposes no duplicate subtree. Cancellation atomically restores the original subtree/focus before removing the proxy; successful destination semantics atomically installs the payload subtree at the destination before removing the proxy and then restores the exact surviving focused descendant; source terminal removes the proxy without focus restoration. Tests assert node identity, role/name, bounds, action set, tree focus, owner window, and replacement/removal timing at every handoff stage.
- Add a generation-bound GPUI window-session interaction gate whose immutable snapshot can be read synchronously by owning platform callbacks without borrowing `App`. Create its closed handle before `open_window`, carry it through the reserved-window/creation record and root closure, and bind it to the committed full `WindowId` only after opening succeeds; construction callbacks and initial root work therefore cannot observe an ungated provisional. The gate permits paint, resize, close, presentation, and lifecycle facts, but rejects provisional native activation, pointer/key/text/IME dispatch, focus, Dock route eligibility, and AccessKit subtree ownership. On Windows the exact current provisional also participates in native non-client hit testing so it is truly hit-transparent to the underlay; dropping GPUI input alone is insufficient. The platform publishes typed visibility, z-order, foreground, and hit-transparency observations under the same role/generation. Root rendering uses the matching inert presentation scope. The gate is not a `PointerInput` or activation-policy mutation: the final detached native profile remains installed from creation. Suppressed user input and accessibility actions are discarded, never replayed after promotion; only internal focus/promotion completion work may be queued with the promotion generation.

| Live-undock interaction state | Generation-bound visible treatment | Source-owned cursor | Terminal clearing rule |
| --- | --- | --- | --- |
| `Desktop` dragging | accepted tear-off outline on the current source-side presentation | move | replace on the next exact route fact |
| `Forbidden` | rejected target and source-side outline with non-color-only forbidden glyph/pattern | not allowed | clear the exact source/target pair on route change or terminal |
| `Unavailable` | neutral unavailable dashed treatment on the gated source-side presentation; no target marker | not allowed | clear on a later available fact or terminal |
| `ReleasePending` | retained locked placement plus non-color-only pending indicator | progress | clear only when the release latch settles |
| `AwaitingDestinationSemantics` | committed placement remains visibly gated with a non-color-only pending indicator | progress | clear only on exact semantics commit or committed viewport loss |
| `Committed` | ordinary destination presentation with no drag treatment | default | current generation has no drag feedback |

The cursor remains owned by the capture source even when the provisional displays the source-side treatment. Every treatment carries the drag, route, lease, and relevant presentation generation; recovery, compensation, commit, and stale terminal callbacks may clear only the exact matching treatment.
- Decouple logical DockSurface lineage from native owner selection. Enforce U25's existing peer-top-level contract by deleting any residual legacy automatic `transient_for` assignment for facade-managed viewports. Ordinary committed and provisional detached viewports are peer top-level by default; an explicit application-requested owner survives only when it matches the final role and is supported. The same HWND is therefore valid before and after desktop promotion. U26 session teardown remains the only lifecycle cascade authority.
- Create the provisional hidden with the final no-initial-activation profile, pre-created interaction gate, and private provisional-only deferred-initial-presentation mode. Its root remains an empty gated shell until the matching source proxy frame commits; only then bind/mount the source snapshot under the payload lease and admit its accepted/submitted/non-empty-present observations. Initial root commit retains the requested hidden client placement rather than consuming it as an ordinary show. Before reveal, apply one KTD36 generation-bound physical client-geometry transaction derived from the callback-scoped physical route point and the target-display snapshot captured for that same route generation. Keep the physical point unchanged, but convert the logical preferred client size and logical cursor offset with the target scale; using `source_geometry().scale_factor()` for destination size or anchoring is forbidden. `LiveRoute` and `FinalRelease` carry the target display identity and scale observation that produced their physical bounds. Wait for exact/adjusted native readback plus the resulting coherent scale/geometry observation, then require a newer non-empty renderer-submitted frame under that placement generation. The reveal command is reveal-only: it does not move or resize the HWND again. The payload transition explicitly invalidates the source proxy, provisional, previous route target, and new route target as applicable, requests the provisional bootstrap frame, and wakes the GPUI run loop. A current non-empty submitted present enqueues one exact-generation `RevealDeferredInitialPresentation` command through U24's post-App FIFO; the owning backend shows without activation and keeps the foreground unchanged. Z-order uses one achievable point-scoped contiguous-band rule: insert immediately below the first unrelated opaque barrier and above only participating same-session peers in that band. If same-session peers straddle the barrier, publish typed `Adjusted` or `Unavailable` rather than crossing the opaque window or claiming a globally impossible order. Publish typed visible/z-order observations for the chosen result. A stale, rejected, already-consumed, or terminal command returns a typed outcome that settles compensation and hides or retires any native side effect that occurred before failure. First-presentation progress cannot depend on later pointer movement, release, activation, native expose, or another route transition. The armed bootstrap snapshot remains stable so continuous movement cannot starve first reveal. Once shown, the provisional remains visible through desktop, host-preview, and rejected feedback; do not add a general live visibility mutation or emulate it with minimize, close, or pointer-input capability changes. Each newer U27 route generation drives one `LiveRoute` U20 placement request carrying the exact route point, physical client bounds, target-display observation, and point-scoped z-order proof. Its terminal observation either advances the latest live position, coalesces behind a successor, or fails closed; it cannot create final-placement evidence. `MouseUp` atomically locks and supersedes this with one `FinalRelease` request, and late live-route outcomes cannot redirect promotion. Route feedback is rendered from one generation into the target host and the current source-side presentation location; alpha is optional polish rather than correctness.
- Model live undock as orthogonal axes rather than a linear enum: transport terminal state; pending open state; source-proxy readiness (`AwaitingSourceProxy`, committed, failed); provisional readiness (`Opening`, payload mounted, non-empty visible with z-order observation, `Unavailable`, terminal); current route (`Desktop { OpenSpace | OpaqueBarrier }`, valid host, forbidden host, `Unavailable`); lease location (source proxy, gated provisional, committed destination); observed placement generation; release latch (`None`, exact locked `ReleasePending`, consumed); destination semantics (not prepared, awaiting committed tree, committed, committed viewport loss); and presentation shutdown (none, quiescing ticket, acknowledged). `AwaitingDestinationSemantics` additionally owns a generation-bound, fake-clock-testable liveness watchdog. Elapsed time may only wake the parked journal and request another exact frame; it never establishes success, admits interaction, or fabricates terminal failure. Destination loss and semantic failure require explicit window, renderer, surface, Graph receipt, placement generation, or semantics-ticket evidence. The live-undock threshold is a one-way per-generation transition measured in physical coordinates from the immutable drag-start source scene; valid host, foreign rejection, or `Unavailable` alone cannot open a provisional. Snapshot the owning platform's drag hysteresis at drag start in physical device pixels (`SM_CXDRAG`/`SM_CYDRAG` on Windows), compare each axis inclusively against the immutable source-scene departure rectangle, and use the same threshold for item, tabs, and floating payloads unless a future typed payload policy supersedes it. Opaque barriers use desktop preview and release semantics; only a foreign Dock surface is rejected. A route observation of `Unavailable` clears target and route feedback, keeps any existing visible provisional gated, and may recover on a later valid movement fact. A first-present fact can therefore arrive while the route is host, foreign, or unavailable, and target movement cannot destroy readiness or overwrite the locked release fact.
- When U27 reports any release before provisional readiness settles, retain exactly one `ReleasePending` with its locked point, observation, candidate, generations, desired physical bounds, and placement generation. A valid host or source result retires the unseen provisional and commits or restores immediately. A desktop result waits a bounded, injectable deadline (default 500 ms) for that same HWND's non-empty visible presentation and the locked placement to settle `Exact` or `Adjusted`; superseded, rejected, closed, or timed-out placement compensates to the exact still-live source. Presentation and timeout use the same ingress/terminal-claim ordering, so only one result can win. An `Unavailable` route release, whether early or ready, consumes the latch immediately, restores only the exact still-live source, and quiesces/retires the provisional without graph commit or desktop promotion. Motion facts may coalesce only within the active drag generation; release, cancel, close, presentation terminal, and promotion results are ordered non-coalescing barriers.
- Define one compensating promotion/transfer saga around KTD32. Complete every fallible native effect, renderer/surface preflight, route check, source payload validation, intended next-graph construction, and role/registry/lease/semantic/gate validation while the provisional is gated, producing an immutable `PreparedPromotion`. Submission creates one exact executor journal. Before its first irreversible receipt the journal is `Abortable` and any failure compensates to the exact live source. At or after that receipt it is `ForwardOnly`: every later stage must commit-or-replay from exact receipts, park on explicit acknowledgement, or enter committed-destination recovery; it cannot roll back, discard a partial handoff, or treat a retry count as a semantic verdict. The exact destination reports `DestinationSemanticsAccepted` under the promotion, destination-window, final-placement, exact workspace-Graph, presentation-tree, and lease generations. Renderer `Submitted` for that exact semantic frame produces `DestinationSemanticsSubmitted`; only that acknowledgement removes the gate and schedules KTD34 activation plus GPUI focus completion. `Deferred` remains pending, `RepaintRequired` requires a newer accepted semantic frame, and `Rejected`/terminal follows the current boundary's compensation or committed recovery. A stale acknowledgement cannot enable a replacement. Desktop promotion preserves the same HWND; a valid other-host release transfers to that host and destroys the provisional only after the transfer is settled; local return/cancellation restore only a live source before the boundary. U19 emits no durable revision before the owner has accepted the revision-ordered publication.
- Define the committed viewport-loss recovery flow explicitly. One post-boundary loss transition moves the payload identity into a surface-owned `LostViewportRecovery` record, publishes one `ViewportLostAfterPromotion` revision/event, and renders a stable recovery entry in the live anchor's recovery region with one semantic `Restore` action; it never reconstructs a second drag release. Focus falls to that entry when the lost destination owned tree focus, its AccessKit `Group` owns the payload name but no payload descendants, and the dead destination owns no tree or gate. Activating `Restore` performs a new ordinary surface transaction that re-homes the payload into the anchor's current recovery tab group, restores the exact surviving descendant focus when possible, removes the recovery entry, and publishes `ViewportRecovered`. If the anchor is already shutting down, the record follows surface teardown and no focus is stolen. U29 deterministic final-tree/action tests assert focus fallback, unique AccessKit ownership, restore result, and stale-action rejection. U28 real-HWND tests assert the visible recovery entry, native activation of its Restore control, the resulting revisions/topology, and native window convergence; they do not claim OS assistive-technology E2E.
- Source, payload, or anchor shutdown, drag replacement, Escape, and capture loss settle the matching live session once before U26 dependent-window teardown. Before release is latched, closing only the current non-source target clears the exact route-owned source-side/target feedback pair and waits for the next U27 movement or release fact instead of terminating the source-owned drag; when a provisional is visible it is the source-side display location, not a third marker authority. After `MouseUp` is latched, target validation failure during `PreparedPromotion` compensates to the exact still-live source without awaiting an impossible later U27 fact or sampling a new point. Direct provisional native close or renderer/device/surface failure marks readiness `Unavailable`: a current valid-host release may still commit, while a desktop result restores only the still-live source. Stale callbacks retire without changing current state. A dead source never receives restored focus or presentation. Every provisional close first claims an exact-generation `PresentationShutdownTicket`, prevents new draw/present submission, drains or invalidates outstanding work, releases DirectX swapchain/DirectComposition and WGPU surface-bound resources through a backend quiesce hook, and publishes a typed acknowledgement. Only that acknowledgement permits native destruction and terminal publication. App shutdown must claim and synchronously quiesce current tickets before registry clear; registry clear then detaches the owner into GPUI's native-retirement coordinator, and only its exact later native terminal settles identity. U28 verifies the production ordering rather than supplying it.

**Test scenarios**

- Lease tests cover item, tabs, and floating payloads. They hold the provisional payload unmounted until `SourceProxyCommitted`, immediately reject stale source hit/focus/AccessKit actions, and prove source-proxy draw failure or generation mismatch cancels without showing a provisional. The source's frozen noninteractive visual remains until the matching provisional is non-empty and visible, then retires in the same generation; every committed handoff frame has at least one visible representation while retaining one live renderer and one semantic owner. After the barrier and before promotion, each has one source semantic/focus proxy, no duplicate AccessKit subtree, no graph revision, and a conflict table for close/remove/reorder/application activation against the lease.
- Gate tests prove a provisional can paint, resize, report first presentation, close, and receive terminal lifecycle facts while `WM_MOUSEACTIVATE`, hit test/input, key/text/IME, focus, Dock targeting, and AccessKit actions are rejected. After KTD32 crosses its first irreversible receipt, the gate remains closed through every forward-settlement stage and after `DestinationSemanticsAccepted`. Inject exact semantic-frame outcomes `Deferred`, `RepaintRequired`, `Rejected`, and `Submitted`; only a matching `DestinationSemanticsSubmitted` may enable committed destination input, KTD34 activation, focus, and AccessKit ownership. Stale, other-window, older-placement, or replacement generations cannot open the gate, and no suppressed user action is replayed.
- Threshold and pending-open tests cover below/at/above physical hysteresis, host/foreign/unavailable movement without creation, one-way generation activation, anchor/App shutdown nested during create/map/root/initial draw, and a late-returning HWND that gains terminal tickets but no admission.
- Presentation tests cover pre-created closed gate, hidden empty root, retained hidden client placement, KTD36 target-monitor client/outer conversion and native readback, source-proxy commit, source snapshot binding, payload mount, explicit invalidation and run-loop wake, first accepted frame, submitted present, non-empty present, generation-bound non-activating reveal-only command, typed z-order/foreground/native hit-transparency observations, continuous visible movement, host/rejected feedback projection, same-HWND reuse, renderer/device/surface failure, and no empty shell or payload-visibility gap. Decorated, custom-titlebar/undecorated, negative-origin, and mixed-DPI cases prove no fictitious caption offset, no double scaling, and no second reveal-time geometry mutation. Independent placement-oracle tests cover both 100%-to-150% and 150%-to-100% moves: expected physical client size and cursor anchoring are derived from declared logical geometry plus a separately supplied target-display scale, never by calling the production tear-off placement helper or recording its first result as expected. A no-further-input case stops pointer, timer polling, activation, refresh, and expose events immediately after payload mount and still reaches non-empty visible presentation through native ingress and the post-App reveal command. An unrelated opaque top-level remains above the provisional, while the exact provisional alone exposes its underlay. An interleaved front-to-back order `session A > opaque X > session B` proves the provisional stays in one point-scoped contiguous band and reports `Adjusted` or `Unavailable` rather than crossing X or claiming to be above both peers. Stale/rejected/terminal reveal commands compensate once. Entering a host never hides or destroys the provisional, and repeated host-desktop-host movement never opens a second window.
- Live-route tests arm and reveal once, then move the same visible HWND through A, B, and C while the source retains capture. Each latest `LiveRoute` generation carries point plus physical client bounds, re-establishes the point-scoped z-order proof, and settles from typed placement observation; no graph revision, payload transfer, activation, hide, destroy, or second window occurs. Pending A superseded by B and late A success/failure cannot change B. `MouseUp` at A immediately followed by native movement to B proves `FinalRelease(A)` supersedes live movement and every later callback, placement, destination, and promotion fact remains bound to A.
- Interaction-feedback tests traverse `Desktop`, `Forbidden`, `Unavailable`, `ReleasePending`, `AwaitingDestinationSemantics`, compensation, committed viewport loss, and `Committed`. They assert the exact source-owned cursor, non-color-only source-side/target treatment, generation fencing, and clearing rule from the table above; a stale generation cannot clear or repaint the current state. Watchdog tests advance retry, destination-loss, and terminal-failure acknowledgements and prove a visible `AwaitingDestinationSemantics` session cannot remain inert without a generation-bound wake or an explicit recovery/terminal fact.
- Role/ownership tests cover U26 provisional admission and close tickets, separation from committed registry/scenes/activation/persistence, same-HWND role conversion, peer top-level default, explicit supported owner, owner mismatch rejection, and dependent-before-anchor terminal convergence without relying on native owner cascade.
- Orthogonal-state tests combine opening with accepted/foreign/barrier/unavailable routes, first-present during host feedback, route changes during readiness, delayed feedback, and target close. Before a release latch, target close clears only its preview and may recover from the next U27 fact; after a locked release, target close fails preparation and restores the exact source without waiting or resampling.
- Early-release tests lock the full `MouseUp` callback frame, desired physical bounds, `FinalRelease` purpose, and placement generation, pump capture-change/close/live-placement/final-placement/first-present callbacks during creation and show, advance only a fake/injectable clock, and prove exactly one result: host/source retirement, same-HWND desktop promotion after `Exact`/`Adjusted` final placement, or restoration to the exact live source. A settled `LiveRoute` ticket can never construct a final receipt or begin destination semantics. Superseded/rejected final placement and ready/not-yet-ready `Unavailable` release consume the latch, restore the source, and retire the provisional without commit. Later pointer motion, live-route terminal, or DPI change cannot alter the locked release point, bounds, or target.
- Promotion-journal tests cover valid other host, valid source host, desktop, opaque barrier desktop, forbidden foreign surface, target generation replacement, provisional creation/renderer/surface/first-presentation failure, pre-boundary preparation failure, and every KTD32 forward stage. Inject one panic or reentrant endpoint change before each lower call and after each lower commit but before its upper checkpoint. Pre-boundary failures restore the exact source. Post-boundary failures preserve the journal and resume or enter committed-destination recovery without duplicate commit, revision, close, focus, or accessibility ownership. Explicit tests cover Graph supersession and ABA entering recovery, placement-generation drift, owner publication panic and revision ordering, Host window-effect completion, provider/Host endpoint loss, two-stage lower tombstone retirement, payload-finalizer panic, persistent parked failure during shutdown, and transfer to the exact surface-shutdown dependency.
- Graph transaction tests cover pre-existing unattached staging trees that share live descendants, floating merge/redock/empty-space moves, staged dependencies that survive unrelated mutations, and 100+ repeated float/redock cycles. After every successful transaction, assert the KTD33 stored/live/staging-closure invariant and prove that no orphan wrapper accumulates.
- Presentation-shutdown tests prove an exact ticket blocks new draw/present work, late generations cannot quiesce a replacement, DirectX and WGPU surface-bound resources acknowledge release before native terminal, repeated close is idempotent, and no late present targets a destroyed window. They also prove a semantics-accepted but not renderer-submitted destination remains gated and follows the correct compensation or committed-loss path when surface terminal arrives.
- Rehost-session tests cover every post-prepare endpoint failure, Host release before and after source release, install rejection, destination-first invalidation followed by source close, source close during restoration, cross-frame restore mismatch, repeated compensation, and retry of the same durable recovery action. Each path proves one terminal provider outcome, no `RehostInFlight` residue, no authority bound to a dead Host endpoint, and exact native-source-terminal waiting where required. Compile-time surface checks reject Dock-side construction of provider receipts or direct orchestration of raw cancellation/abandonment phases.
- Multi-surface/App-shutdown tests prove only the owning surface's pending or committed provisional is cancelled/closed, direct provisional terminal events cannot leak a handle or late present, the synchronous pre-clear barrier quiesces renderer/surface work, and registry clear converges even without per-window close observers.

**Deletion/replacement**

- Delete release-only viewport creation as the normal desktop-drag experience, any implicit panel-only lease that ignores tabs/floating payloads, and the assumption that graph mutation is required to render live provisional content.
- Delete provisional hide/show state transitions, automatic facade viewport native owner assignment, source/provisional duplicate live semantic trees, and queued replay of suppressed provisional user input or accessibility actions.
- Delete parallel outside-release/tear-off/interaction state ownership superseded by `DockLiveUndockSession`; do not expose a general visibility API solely for this drag path, make a session gate a public permanent activation mutation, or persist provisional topology.
- Delete Dock-side rehost compatibility entry points, copied exact-identity comparisons, cancel-without-finish helpers, and public raw provider transitions superseded by the opaque GPUI session.

**Unit gate**

- Dock live-session, payload lease, GPUI gate, Windows behavior, route/tear-off/surface lifecycle, accessibility, multi-surface, example, migration, and documentation tests pass.
- Review confirms pending-open shutdown fencing, provisional-role isolation, exact U26 lease/ticket ownership, physical threshold semantics, source-proxy committed barrier before payload mount, frozen-source-to-visible-provisional continuity, KTD36 hidden client-geometry settlement and reveal-only show, same-HWND typed live-route movement, atomic `FinalRelease` locking, point-scoped z-order and native hit transparency, one payload renderer and source proxy, backend-readable gate/no replay, recoverable unavailable movement and terminal unavailable release, peer-top-level default and explicit owner policy, same-HWND promotion, complete preflight before KTD32's first irreversible receipt, exact forward settlement after that boundary, KTD33 pre-mutation staging retention and single final sweep, KTD35 renderer-submitted destination semantics before gate removal, KTD34 observed activation, committed viewport-loss handling before submitted-semantics acknowledgement, opaque GPUI rehost-session ownership with mandatory compensation and no Dock-side raw terminal protocol, presentation-shutdown acknowledgement before HWND terminal/App registry clear, exact Graph/revision receipts with fail-closed supersession, ordered publication, and exhaustive cleanup.

### U28. Prove Owning-Platform Multi-Viewport Behavior

**Outcome**

The release gate distinguishes model simulation from real native-window behavior. Deterministic Windows integration and subprocess scenarios exercise actual HWND creation, source capture, nested message dispatch, classified point stacks, first non-empty presentation, session gating, peer/explicit-owner roles, same-HWND promotion, renderer-before-HWND teardown, and process/window convergence.

**Requirements**

- R13, R15, and R26-R30.

**Native multi-viewport support tiers**

| Backend | Release tier after this plan | Guaranteed evidence |
| --- | --- | --- |
| Windows | Tier 1, release-gating | Complete R26-R30 real-HWND, renderer, system-input, and subprocess matrix on `open-gpui-windows-native-interactive-ephemeral`, plus deterministic final AccessKit-tree/action gates; OS assistive-technology E2E is not claimed |
| macOS | Capability-declared | Compile and typed capability contract only until an owning-platform real-window suite proves each advertised equivalent; no Windows evidence is inherited |
| Linux | Capability-declared per backend/session | Compile and typed capability contract only until an owning-platform real-window suite proves each advertised equivalent; X11/Wayland claims remain separate |
| web | Explicitly unsupported native multi-viewport | Typed unsupported capability and no emulated native-window claim |

The Goal Capsule's general-purpose framework claim is limited to this matrix. A backend moves to Tier 1 only with equivalent owning-platform evidence; unsupported owner, capture, presentation, or same-window promotion relationships remain explicit rather than imitated.

**Primary files**

- native integration support and tests in `crates/gpui_windows/src/platform.rs`, `crates/gpui_windows/src/events.rs`, and `crates/gpui_windows/src/native_test_harness.rs`, or a dedicated Windows integration target in that crate
- `crates/gpui_windows/src/window.rs` and renderer/surface terminal support in `crates/gpui_windows/` and `crates/gpui_wgpu/`
- GPUI test diagnostics for native-event and presentation generations
- `crates/gpui_wgpu/` presentation outcome hooks and cross-backend compile fixtures
- `crates/gpui_web/` contract compile coverage
- `crates/gpui_docking/` native-integration fixtures
- `examples/docking-native/`
- `examples/docking-native/tests/native_windows_interactive.native-scenarios.toml`
- `examples/docking-multiviewport/`
- `xtask/src/native_windows_interactive.rs`
- `.github/workflows/native-windows-interactive.yml`
- `docs/verification.md`
- Dock verification, ADR, migration, and release documentation

**Behavioral work**

- Extend U24's `gpui_windows`-owned real-HWND support into a deterministic typed harness around raw HWND creation, Z-order barriers, external-window underlays, system input, process HWND census, the Windows application/message pump, WndProc, renderer/present path, and worker subprocess. `gpui_docking` supplies backend-neutral fixtures and Dock assertions but does not own duplicate raw-HWND or desktop-enumeration infrastructure. Test-only observation and input-control seams cannot replace HWNDs with `TestWindow`, call Dock handlers directly, or inject target-window events that production capture would prevent.
- Make `examples/docking-native/tests/native_windows_interactive.native-scenarios.toml` the native-test-owned scenario metadata and selector authority. Its scope is only U27-U32 native scenario identity, ownership, observation domains, behavior, and test coordinates; it does not replace component contracts, Gallery probes, public owner tables, or runtime authorities. A typed `xtask` runner parses it and constructs exact package/test filters, ignored-test policy, serial execution, runner preflight, and failure reporting. The workflow invokes only that stable runner entry. Rust scenario code retains behavior dispatch but receives the manifest row rather than duplicating IDs, requirement owners, observation domains, and test coordinates. Do not expand `xtask` into a YAML, shell, or Rust-source parser.
- Expose typed, metadata-only test observations for callback envelope/disposition and WndProc ordinal, full window and session generation, immutable native-window snapshot, source capture owner, screen point and classified hit stack, locked release facts, live-undock phase, source-proxy frame commit, first accepted frame, exact renderer-submitted generation, first non-empty presentation, native client geometry and scale, native visibility and point-scoped Z-order, interaction-gate state, native activation ticket/observation, peer/explicit-owner relationship, `PresentationShutdownTicket` claim/quiescence acknowledgement, and terminal shutdown. Assertions use those facts rather than parsing one generic log or treating visibility, setter return, or accepted frame as presentation.
- Exercise accepted draw, submitted present, non-empty presentation, renderer/device/surface failure, and native visibility as distinct facts. Pre-commit anchor presentation failure settles `Opening` without registration; post-commit anchor presentation failure enters `ShuttingDown(PresentationFailed)` and converges through forced teardown. Provisional presentation failure compensates its live-undock generation without mutating the durable graph. A destination semantic frame accepted but not submitted stays gated; exact `Submitted` is required before activation, focus, input, and AccessKit ownership.
- Exercise create/show/position/activate/close callbacks while another GPUI update owns `App`, including nested Windows message dispatch. Prove queued replay, ordering barriers, no dropped mutation terminal, no runtime double borrow, and eventual first presentation.
- Use a two-HWND source/target subcase for U27 transport, then a multi-HWND U29 harness containing source, target, and concurrently visible provisional windows; opaque-occlusion cases add another top-level barrier HWND. Drive the source-only captured move/up stream through WndProc while create/show/present callbacks reenter GPUI and prove no `RefCell already borrowed` panic or dropped terminal occurs. Before a normal release, assert the target Dock preview commits a presentation generation without target raw mouse input. Outside every host, stop all further pointer, activation, and expose input after payload mount and assert the explicitly bootstrapped provisional HWND is already visible with a non-empty submitted frame before `MouseUp`. While the button remains down, move through distinct A, B, and C points and wait at each point for the same HWND's exact client bounds, route generation, non-empty submitted frame, current point-scoped Z-order proof, and unchanged source capture; graph revision and payload ownership remain unchanged, and the target sees no raw pointer input. Entering a valid or forbidden host must retain that same visible HWND and project route feedback, not hide it. Separately inject `MouseUp` at A and immediately move the system cursor to B before App drain; the locked callback frame, final placement, destination, and promotion remain A, and the later `WM_CAPTURECHANGED` is cleanup only.
- Add at least one Windows scenario that uses system-level pointer injection at screen coordinates and never selects the receiving HWND. At down, cross-window move, and up, assert `GetCapture`, the actual WndProc recipient, the source-only raw move/up stream, callback-scoped client-to-screen physical frames, and final capture release; retain the negative assertion that the target HWND receives no raw move/up. Move the source across a DPI boundary between callback and deferred consumption and prove a newly observed DPI cannot rescale the locked point. Directly sending those messages to the source remains useful deterministic coverage but cannot satisfy this routing claim.
- Add owning-platform NoInput scenarios with a real GPUI no-input HWND above a GPUI target, above an external HWND, and as two consecutive pass-through windows. The sampled stack retains each exact pointer-input generation and continues to the real lower terminal. Advance one covering window through false-to-true-to-false after the source callback but before Dock consumes the queued release; the old generation produces `Unavailable` with no preview or drop.
- Add KTD36 native client-geometry scenarios. A real mixed-DPI live-undock runner has two monitors with distinct effective DPI values; a negative-origin scenario additionally requires `SM_XVIRTUALSCREEN < 0` or `SM_YVIRTUALSCREEN < 0`. Drive the production captured source move, hidden initial placement, reveal-only show, continuous visible movement, locked release, and same-HWND promotion onto the non-source display in both lower-to-higher and higher-to-lower scale directions. The callback frame keeps its physical point immutable while the requested logical client size and cursor offset use the captured target-display scale. The backend accepts any synchronous `WM_DPICHANGED`, performs at most one deterministic target-side correction under the same placement generation, and publishes exact client bounds, display identity, and scale for the same HWND. The expected size/anchor oracle is independent of production placement code. Decorated and custom-titlebar/undecorated windows prove client origin is preserved without a fictitious caption offset; a native policy/decoration change and an X11-capable fixture prove asynchronous frame extents require later event/readback reconciliation rather than setter success. A separate idle subcase dispatches one hidden, live, or final placement and then stops timers, pointer input, activation, refresh, and expose work; only the exact native move/resize/DPI/visibility callbacks may wake and settle that generation. Missing distinct-DPI or negative-origin capacity is an explicit runner capability failure for its scenario, not a skip, tolerance-based pass, or inherited Windows proof for another backend.
- Run system-injected scenarios only on the named `open-gpui-windows-native-interactive-ephemeral` runner owned by the Windows backend CODEOWNERS, with an interactive input session/desktop, compatible integrity level and UIPI boundary, serial isolation from other cursor tests, sufficient virtual-screen bounds, the declared mixed-DPI topology when that scenario runs, and renderer/GPU availability. The self-hosted workflow accepts only merge-queue commits, trusted main-branch pushes, or a maintainer `workflow_dispatch` with an exact SHA; it never executes pull-request code directly. Before Cargo runs, the job must verify infrastructure-provided one-job registration/clean-image attestation, a dedicated low-privilege account, no repository/organization secrets, and the documented network deny/allowlist; missing proof fails closed. Each worker records and restores the original cursor position in guaranteed cleanup. A preflight injects down/move/up into a sentinel HWND and proves real WndProc receipt plus capture acquisition/release before the Dock scenario begins. CI records runner availability and the documented escalation owner; it never skips the gate, falls back to an incapable hosted runner, or substitutes direct `SendMessage`/`PostMessage` delivery.
- Register the native scenarios through R13's federated typed binding with stable IDs and explicit requirement owners: `native.u27.source-capture`, `native.u27.opaque-occlusion`, `native.u27.surface-shutdown`, `native.u29.provisional-same-hwnd-promotion`, `native.u29.live-route-and-release-lock`, `native.u29.committed-loss-recovery`, `native.u28.activation-terminal`, `native.u28.client-geometry-reconciliation`, `native.u28.event-driven-geometry-wake`, `native.u28.process-convergence`, `native.u28.no-input-pass-through`, and `native.u28.mixed-dpi-client-bounds`. This native-scenario manifest is the only metadata authority within that test-owned scope; the workflow and Rust worker receive its selected row instead of exporting duplicate ID/owner/test/domain tables. Missing, duplicate, simulated-only, or unowned bindings fail the conformance scanner before scenario execution.
- Cover the full release and cancellation matrix through actual native messages: valid cross-window drop, desktop same-HWND promotion, source return, Escape, capture loss, target-close re-resolution, source/payload/anchor terminal close, source-proxy commit failure, provisional creation or presentation failure, promotion preparation/next-state validation failure, repeated close, stale generation, and suppressed provisional input actions. Deterministic final AccessKit-tree/action tests prove the corresponding accessibility suppression and post-promotion ownership without claiming OS assistive-technology E2E. Every failure before KTD32's first irreversible receipt compensates to the exact live source; every failure after it preserves the same journal and settles forward or enters committed-destination recovery. A destination-window, renderer, or surface terminal after the boundary but before `DestinationSemanticsSubmitted` must publish one committed viewport-loss transition, preserve recoverable topology, retire the gate/ticket, and never roll back or emit a second drag release. Bind every worker to a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` before releasing its start barrier, terminate the entire job on timeout, and use inherited anonymous pipes with a per-run nonce for report/release handshakes. Acceptance requires an empty post-scenario job/process census, so failures cannot leave CI windows or processes alive and local temp files cannot forge success.
- Verify ordinary detached creation does not steal foreground focus but later click and programmatic activation work through KTD34. Exercise source deactivation, a temporary `NoWindow` observation, destination activation, and delayed stale gain in that order; the pending ticket survives the loss-before-gain gap and only exact destination observation completes it. Inject OS activation rejection, another owned winner, target replacement, and target close; every public completion settles, source-focus restoration never claims success from dispatch alone, and a post-boundary promotion is not cancelled merely because the old source deactivated. Assert the default peer top-level relationship, explicit supported owner behavior, and no automatic DockSurface-native owner coupling, while separately proving explicit surface teardown.
- Prove U29's production `PresentationShutdownTicket` prevents new submissions, drains or invalidates outstanding work, releases DirectX and WGPU surface-bound resources, and records a typed quiesced acknowledgement before HWND destruction and terminal observation. A late generation cannot acknowledge a replacement ticket and no late presentation may target a destroyed HWND. This is an ordering assertion, not a log-string claim.
- Start two Dock surfaces in one process, close one real anchor during active/provisional work, and first assert exact-generation route cancellation releases native capture (`GetCapture() == NULL`) and removes GPUI drag/outbox plus Dock feedback before any dependent close dispatch. Assert busy or borrow-conflicted close dispatch remains retryable, cleanup continues after the first injected callback panic, all and only that surface's dependent HWNDs observe `WM_NCDESTROY` before the anchor, and the session reaches `Closed` only after their tickets settle; the other surface remains live throughout. Close the remaining anchor and require the application to return through its normal last-window policy without example-level `cx.quit`. The worker then publishes a fail-closed pre-exit census and remains alive. The parent independently verifies the still-live PID's top-level and message-only HWNDs plus the worker's GPUI registry, active drag, surface session/runtime, terminal tickets, capture, and native-generation census, then sends the release acknowledgement that permits process exit. Any enumeration or identity-read failure fails the gate. In a separate worker, invoke App shutdown while an active anchor, committed viewports, a pending provisional open, and a visible provisional are live; it must cancel or bind-and-compensate the late open, claim and synchronously quiesce every presentation ticket before registry clear, and converge even without per-window close observers.
- Keep TestPlatform/Visual tests as fast graph, route, and presentation-model evidence. Rename CI jobs and documentation that call them native rendered or end-to-end tests unless they execute the real owning-platform harness.
- Add compile/capability gates for the U25 appearance/owner and U24 ingress/commit-boundary contracts on macOS, Linux, and web. Platform-specific real drag scenarios are required when a desktop backend advertises equivalent native capture and window capabilities; unsupported relationships are documented rather than imitated.

**Test scenarios**

- Real callback-reentrancy tests cover every U24 event class, merge/barrier order, close during queued work, a reused window generation, and first paint after a deferred frame.
- Real native tests use a two-HWND source/target transport subcase and a multi-HWND source/target/provisional topology, adding opaque barriers, external underlays, consecutive no-input windows, and distinct-DPI/negative-origin monitor topologies when required. They cover OS-routed source-only captured move/up through system pointer injection, actual recipient and `GetCapture` observations, native callback physical frames, point-bound classified observations, delayed pass-through-generation revalidation, independent frontmost verification, exact no-input/provisional underlay traversal, opaque ordinary/foreign-window barriers over a Dock host, distinct exact IDs for peer siblings and explicit owned siblings, target preview before release, same-HWND A-to-B-to-C continuous provisional movement before release, immediate post-`MouseUp` cursor divergence, input-frame/current-DPI divergence, KTD36 client-geometry reconciliation, exact target-DPI final placement, gated native-input rejection, KTD34 activation loss-before-gain and terminal outcomes, and owner/z-order facts. Deterministic final AccessKit-tree/action tests separately prove accessibility suppression, stale-action rejection, and post-promotion ownership. A deterministic platform seam forces enumeration/frontmost disagreement and must produce `Unavailable` without preview or drop.
- The provisional presentation, interaction gate, and same-HWND promotion claims form one continuous native scenario because the existing harness observes them along one indivisible opening generation. It must prove a non-empty visible frame before release, reject pre-release pointer/activation input without replay, and then complete KTD32 promotion on the exact same HWND with committed input and native role. Split rows are allowed only when each has a genuinely different termination or fault sequence; wrappers over the same execution are forbidden.
- Runner-contract tests fail before scenario execution when the sentinel cannot prove true system injection, WndProc receipt, or capture transitions. They serialize global cursor ownership, restore the cursor and every HWND on success, panic, or timeout, and distinguish an incapable runner configuration from a product failure without converting either into a skip.
- Real failure tests cover capture cancel/deactivation, hover-only `WM_MOUSELEAVE`, listener/reentrant-callback panic with exactly one terminal reservation, `MouseUp`/capture-change/close nested during source-proxy commit or provisional opening, source/target/payload/anchor destruction, pre-commit and post-commit anchor presentation failure with their distinct terminal paths, failed provisional first presentation, failed promotion preparation/next-state validation, post-boundary/pre-semantics destination loss, presentation-ticket acknowledgement before HWND terminal, runtime close-observer reentry, and repeated/late close with no double borrow, graph corruption, duplicate release, or leaked HWND.
- A source-HWND-close-during-capture test observes exactly one pointer cancellation and `GetCapture() == NULL` before dependent HWND close dispatch, with no stale GPUI active drag/outbox, Dock route, paired source/target feedback, scene frame, or provisional presentation surviving the close. A delayed G1 cleanup cannot cancel or remove a replacement G2 authority.
- Real surface tests cover `Prevent`/`MergeBack` under ordinary close, forced bypass under anchor shutdown, two-surface isolation, committed non-interactive `ShuttingDown` state with pointer/key/IME/focus/AccessKit suppression and idempotent repeated close, retry after one busy/borrow-conflicted close dispatch, cleanup completion followed by rethrow of the first injected panic, dependent `WM_NCDESTROY` before anchor destruction, delayed child terminal-close observation, active App-shutdown ticket quiescence before registry clear without close callbacks, rejected reopen while the old anchor/runtime/tickets remain live or registered, explicit reopen generation after convergence, absence of overlapping old/new generation HWNDs, and final process convergence.
- CI-negative fixtures prove a VisualTest-only target cannot satisfy the native gate and that missing presentation/event observations fail with the exact window/session/event domain.

**Deletion/replacement**

- Delete or rename claims that direct `VisualTestContext` event injection is native Dock end-to-end coverage.
- Delete example-only quit behavior, manual-only acceptance credit, and log-string-only checks superseded by typed native observations.
- Do not require pixel-perfect screenshots for correctness, depend on a human moving the pointer, or let timeout/retry hide deterministic event loss.

**Unit gate**

- Windows real-HWND integration, Dock subprocess, fast simulated suites, cross-platform compile/capability, CI, verification docs, ADR, migration, and release gates pass.
- Review confirms the tests cross the actual WndProc/capture/point-stack/present/lifetime boundaries on a preflight-proven capable injection runner, every reported regression has a native negative-then-positive scenario, source-only captured callbacks never double-borrow `App`, no-input generations remain current through delayed consumption, KTD36 decorated/undecorated, negative-origin, and mixed-DPI client placement is exact, the same provisional HWND visibly follows A-to-B-to-C before normal release, the locked `MouseUp` callback frame survives immediate later movement, opaque occlusion cannot target hidden hosts, unavailable release cannot promote, the source-proxy barrier prevents overlapping payload ownership or a visible handoff gap, gated provisional input is never replayed, exact renderer-submitted destination semantics precede gate removal and KTD34 activation, peer/explicit-owner roles are distinguished, KTD32 forward settlement survives each injected post-boundary failure, exact capture/route cleanup precedes dependent close, retryable/panic-safe shutdown destroys dependents before the anchor, the production presentation-shutdown ticket precedes HWND terminal and registry clear, App shutdown and anchor-last convergence are proven by a live pre-exit census, simulated evidence is labeled honestly, and no test worker can leak a process or HWND after failure.

### U30. Publish Programmatic Activation From Accepted Frames

**Outcome**

An `ActivationHandle` always targets the control visible in the latest accepted frame. A rejected,
rolled-back, hidden, absent, or superseded candidate cannot replace or clear the committed binding.

**Requirements**

- R1-R2, R8, R15, R26.

**Primary files**

- `crates/gpui/src/window.rs`
- `crates/gpui/src/window/frame_journal.rs`
- `crates/ui_components/src/activation.rs`
- `crates/ui_components/src/overlay/focus_scope.rs`
- `crates/ui_components/src/overlay/adapter.rs`
- semantic control tests and migration/verification documentation

**Behavioral work**

- Give each logical `ActivationHandle` binding one stable, window-scoped prepaint publication identity. Candidate render records a commit/discard transaction instead of mutating the handle immediately. Only an accepted valid frame publishes the exact window and dispatcher; candidate rollback preserves the prior committed dispatcher, while absence from a later accepted frame or committed non-interactive presentation clears only that exact publication.
- Reuse GPUI's frame journal and `record_prepaint_window_transaction` semantics so cached subtrees, transactional rendering, transform failure, layout-preserving Hidden/Inert presentation, and atlas rejection share one publication authority. The transaction callbacks receive a GPUI-created, window-bound `AcceptedFrameFence`, not a bare revision, so lifecycle consumers can prove that their transition is already represented by the rendered frame. Do not add a component-owned frame counter, timer, or render-order heuristic.
- Distinguish ordinary focus transitions, which must wait for a later accepted frame, from portal-anchor publication transitions backed by an `AcceptedFrameFence`, which may settle against that accepted frame. Owner release must preserve an inactive scope's already-established restore obligation, while stale subtree replacement must cancel the old obligation. Keep focus mutation deferred until the focus-stable phase has completed.
- Reusing one handle for two simultaneously committed controls remains last-accepted-publication-wins only if that ambiguity is intentionally preserved and documented; otherwise reject duplicate same-frame publication with a typed diagnostic and require one handle per independently addressable control. Cross-window requests continue to return `WrongWindow`, and disabled committed controls continue to return `Blocked`.

**Test scenarios**

- Commit control A, render candidate B with the same handle, reject B through atlas or transaction failure, and prove the still-visible A remains dispatchable.
- Commit A, accept B, then prove B replaces A exactly once. Remove, hide, or make B inert in an accepted frame and prove the handle becomes unavailable without a stale discard clearing a later replacement.
- Cover cached journal replay, subtree transaction rollback, transform failure, window replacement, duplicate/superseded publication, disabled state, and cross-window requests.
- Prove a portal-anchor unmount restores focus against the same accepted frame without waiting for generation N+1, while an ordinary event-driven deactivate still waits for the next accepted frame. Prove an atlas-rejected candidate emits no accepted-frame fence and that stale same-ID subtree replacement cannot revive the prior restore claim.

**Deletion/replacement**

- Delete render-time `RefCell` publication and window-only unbind logic that can clear a replacement dispatcher.
- Do not expose frame transactions through the public component API or make applications manually acknowledge render acceptance.

**Unit gate**

- Activation/component, overlay focus, frame-journal, transform/presentation, and compile-contract tests pass.
- Review confirms the handle is a projection of accepted-frame authority, rejected candidates preserve the visible committed target, absence clears exact identity, accepted-frame-backed focus settlement does not invent an extra frame barrier, ordinary transitions still wait for a future accepted frame, and no parallel component-side frame lifecycle remains.

### U31. Converge Per-Window Renderer Surface Lifecycle

**Outcome**

Every renderer-owned native window has one private, generation-bound surface lifecycle that isolates resize, zero extent, surface loss, device recovery, and shutdown from other windows sharing the renderer device. Window and Dock consume renderer-neutral presentation facts; backend swapchain state never becomes public framework or Dock vocabulary.

**Requirements**

- R1-R2, R15, R27, and R30.

**Primary files**

- `crates/gpui/src/platform.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui_wgpu/src/wgpu_renderer.rs`
- DirectX, Metal, and headless renderer lifecycle implementations and tests
- renderer/window presentation contract tests
- native multi-window renderer smoke targets and CI workflows
- renderer lifecycle ADR, verification, migration, and release documentation

**Behavioral work**

- Implement KTD37 as one private per-window surface runtime. The renderer-neutral boundary reports only `Submitted`, `Deferred`, `RepaintRequired`, and typed per-window `Terminal` with exact window/device/surface generations. Dock and provisional sessions never match WGPU, DXGI, Metal, swapchain, texture, or surface-error enums.
- Replace ordinary resize's unbounded shared-device wait with per-window attachment replacement and generation-bound last-use retirement. Shared device loss remains a device-generation coordinator, but one window's resize, occlusion, zero extent, or surface recreation cannot synchronously freeze every other window.
- Treat zero physical extent as `SuspendedZeroExtent`: release or park surface-bound attachments as required, perform no acquire/submit/present, and publish no presentation receipt. A non-zero coherent native-geometry observation resumes configuration under a newer generation.
- Present a usable suboptimal frame, record its exact submission, and schedule reconfiguration at the next safe point. Surface loss enters `RecreatePending` and wakes on exact native-handle/geometry/device facts; an unrecoverable or authority-lost surface publishes a typed terminal for only that window. Fixed sleep and endless warn/retry loops cannot decide semantics.
- Preserve the existing stronger shutdown contract: a generation-bound `PresentationShutdownTicket` stops new submissions, drains or retires exact last-use work with bounded typed acknowledgement, releases per-window resources, and only then permits native retirement. A normal surface-terminal fact cannot bypass that order.
- Feed KTD35 directly from renderer submission. The exact destination semantic frame advances to `DestinationSemanticsSubmitted` only from the matching per-window `Submitted` outcome; `Deferred`, `RepaintRequired`, zero-extent suspension, surface loss, or terminal cannot open interaction.

**Test scenarios**

- Renderer-neutral contract tests inject `Submitted`, `Deferred`, `RepaintRequired`, and `Terminal` for the exact semantic frame, an older frame, another window, a replacement surface generation, and a replacement device generation. Only the exact submitted frame opens the KTD35 gate.
- WGPU tests cover zero extent suspend/resume, rapid resize and mixed-DPI changes without an unbounded `device.poll(Wait)`, usable suboptimal render-then-reconfigure, surface loss followed by exact recreation, recreation authority loss followed by per-window terminal, and another window continuing to submit throughout.
- Shutdown tests prove exact last-use drain, idempotent repeated close, surface terminal during drain, replacement-generation isolation, and renderer resources quiesced before native terminal on every supported backend.
- A real WGPU owning-platform gate creates two native windows, submits non-empty frames to both, resizes and suspends/resumes one, destroys the secondary after renderer quiescence, and then closes the main window with an empty surface/native census. Linux/X11 may use Xvfb plus Mesa/Lavapipe only when the workflow proves those facilities and fails closed when absent. Metal and other released owning backends require the same semantic sequence before claiming equivalent support; Wayland/web limitations remain explicit.

**Deletion/replacement**

- Delete the WGPU zero-size `1x1` substitution, ordinary resize whole-device unbounded wait, suboptimal-frame discard, indefinite surface-recreate warning loop, and fixed semantic recovery sleep.
- Delete any Dock-side renderer error matching or platform-wide recovery path used only because one window lacks a local surface authority.
- Do not introduce Winit, Dear ImGui renderer callback tables, a global secondary-window render pass, show-before-render, or a second presentation-generation authority.

**Unit gate**

- Renderer-neutral window tests, WGPU surface tests, supported-backend shutdown tests, real owning-platform multi-window smokes, CI, verification, ADR, migration, and release documentation pass.
- Review confirms per-window fault isolation, zero-extent honesty, no unbounded UI-thread device wait, exact submission evidence for KTD35, typed terminal propagation, exact shutdown drain, no backend enum leakage into Dock, and no parallel presentation authority.

### U32. Converge Display Topology And Event-Driven Client Geometry

**Outcome**

GPUI owns one complete, immutable, generation-bound display publication and one event-driven geometry reconciliation path. Pointer frames and placement tickets carry detached target-display facts from that authority; partial display enumeration, source-DPI destination sizing, live native monitor handles, and timer-driven progress cannot become placement truth.

**Requirements**

- R1-R2, R15, R25-R26, and R29-R30.

**Primary files**

- `crates/gpui/src/platform.rs`
- `crates/gpui/src/app.rs`
- `crates/gpui/src/app/native_event_ingress.rs`
- `crates/gpui/src/window.rs`
- `crates/gpui/src/platform/test/`
- `crates/gpui_windows/src/display.rs`
- `crates/gpui_windows/src/window.rs`
- corresponding display and geometry adapters in `crates/gpui_macos/` and `crates/gpui_linux/`
- `crates/gpui_docking/src/viewport_tear_off_placement.rs`
- `crates/gpui_docking/src/native_captured_drag.rs`
- `crates/gpui_docking/src/surface/live_undock_runtime.rs`
- owning-platform display, placement, and event-loop tests
- display/placement ADR, verification, migration, and release documentation

**Behavioral work**

- Implement KTD38 as one private display-snapshot authority. One successful publication contains the complete display set, exactly one proven primary identity when the platform exposes one, stable provenance, logical and signed physical desktop bounds, work areas, scale factors, and one monotonic generation. Public `displays`, `primary_display`, and lookup APIs project that immutable value rather than performing independent native queries.
- Build a candidate snapshot completely before publication. Initial enumeration or construction failure is explicit. After one complete publication exists, any partial enumeration, ambiguous primary, duplicate identity, unavailable scale/work area, or per-display construction failure retains the previous complete generation and reports a typed degraded refresh; it cannot `filter_map` a mixed topology into committed state.
- Treat native monitor/display handles as adapter-local inputs only. Persisted or cross-callback facts use stable detached identity plus publication generation. Reused native handles, primary changes, work-area-only changes, scale changes, provenance changes, and topology changes each produce a new generation; signed negative origins are preserved.
- Extend callback-scoped physical pointer facts with the exact target-display observation at the sampled point when the platform advertises exact multi-display placement. The screen point remains physical and immutable. Dock converts logical preferred client size and logical cursor offset with that target scale, and records the display identity/publication generation that produced the `LiveRoute` or `FinalRelease` bounds. Missing or stale target-display facts fail closed rather than falling back to source DPI.
- Bind explicit placement tickets to the target display publication generation used for planning. A newer display publication before native commitment settles the old request as stale/adjusted/rejected according to coherent native readback; it cannot silently reinterpret the old logical size under a new scale. The backend still owns client-to-outer conversion and exact observed client geometry under KTD36.
- Make native creation and the first explicit client-geometry request one retained placement transaction. A request accepted before initial map/reveal retargets that retained authority synchronously; an already queued initial-presentation command cannot overtake it and publish the older client origin or size. Decorated X11 creation remains pending until the matching post-map frame-extents/configure observation either proves the requested client geometry or performs one generation-bound correction, while a later user move or resize supersedes that correction.
- Make secondary-window move, resize, scale, visibility, decoration/frame-extents, and display-topology callbacks enter U24's native ingress and schedule one foreground wake whenever matching geometry work remains. Completion must be possible with the event loop otherwise idle. Timers may diagnose or wake a parked retry, but repeated polling, continuous redraw, activation, or pointer motion cannot be required for semantic progress.
- On Windows, retain exact client readback and deterministic same-generation correction. One private non-client policy distinguishes a native caption, a custom titlebar, and an undecorated frame; it is the single source for creation styles, `WM_NCCALCSIZE`, client-to-outer projection, and hit testing. Initial presentation first settles hidden client geometry, then performs a move-free/size-free reveal, and commits only after a visible native readback still matches the same client-placement intent. On X11 or another asynchronous frame-extents backend, keep the transaction pending across map/decoration events, reconcile only the matching generation after native readback, and let a later unrelated user move win. A backend without that proof declares global client bounds, target-display DPI, and exact placement unsupported instead of inheriting Windows credit.
- On macOS, keep Cocoa point-space client geometry authoritative: creation, ordinary bounds, restore placement, `content_size`, display identity, and backing scale must come from one coherent `contentRect`/`contentView` observation. `NSWindow.frame` is an outer-frame projection only and must never be persisted as `WindowBounds` or reused as a client origin. A titlebar, transparent titlebar, fullscreen, or Retina-screen transition starts a generation-bound observation. The selected `NSScreen`, detached CG display row, `NSWindow` backing scale, content-view backing conversion, client rect, and window state must agree in one complete sample; otherwise the backend retains the whole previous fact or reports a typed degradation, never a requested-value seed or old-geometry/new-state splice.

**Test scenarios**

- Pure snapshot tests cover complete publication, initial failure, partial refresh retention, duplicate or ambiguous primary rejection, native-handle reuse, scale/work-area/provenance-only changes, negative origins, monotonic generations, and immutable lookup projections.
- Pointer/placement tests capture one physical point on a target display whose scale differs from the source, mutate the current source and display facts later, and prove the locked physical point, target identity, logical size conversion, cursor anchor, and placement generation remain coherent. Both lower-to-higher and higher-to-lower scale directions use independent expected values.
- Supersession tests change the target display generation before dispatch, during native placement, and after exact observation. Old generations never commit replacement facts or reinterpret logical size, while the exact native result settles once as stale, adjusted, rejected, or committed.
- Event-driven tests dispatch hidden initial, visible live-route, final-release, ordinary move/resize, DPI, decoration changes, and X11 frame-extents work, then stop timers, pointer input, activation, refresh, expose, and continuous rendering. The matching native callback alone wakes and settles the transaction; stale callbacks cannot wake or commit a replacement generation. Initial-map tests issue a newer client-geometry request before the first presentation command drains and prove that only the newer request can become visible.
- Owning-platform tests cover Windows native-caption, undecorated, and custom-titlebar exact client geometry, the exact style bits for each policy, a move-free/size-free first reveal, visible post-show readback, live decoration changes, negative-origin and two-way mixed-DPI moves, complete display refresh failure, and target display removal during placement. macOS covers decorated and transparent-titlebar content-rect round trips, three consecutive save/reopen cycles without drift, vertical display work-area coordinates, whole-fact retention across unstable fullscreen/titlebar transitions, and Retina/non-Retina migration from one coherent `contentRect`/screen/window/view-backing observation. X11 receives an asynchronous initial-map/frame-extents fixture that proves the requested client origin and size, one matching late-extents correction, and a later native user move winning before advertising equivalent exactness; until then it reports window-local bounds and target-display DPI/physical placement as unsupported. Unsupported backends compile and report their narrower capability honestly.

**Deletion/replacement**

- Delete partial `filter_map` display publication, independently queried primary identity, source-DPI destination sizing, production mixed-DPI tests whose expected value is copied from the implementation under test, timer polling used as geometry-progress proof, unexplained native-position nudges used to compensate for unmapped or late frame extents, and any macOS path that persists an outer `NSWindow.frame` as client `WindowBounds`.
- Delete cross-callback native monitor handles and display-index authority. Do not expose a mutable display registry, Winit monitor handle, ImGui monitor index, or second Dock-local display snapshot.
- Do not weaken Windows exact readback to pixel tolerance, assume undecorated means client equals outer, or generalize X11's late-frame-extents compensation to platforms that provide synchronous exact geometry.

**Unit gate**

- GPUI display/placement tests, Windows owning-platform geometry tests, supported Linux/macOS capability tests, Dock mixed-DPI and route-placement tests, native idle-wake scenarios, CI, verification, ADR, migration, and release documentation pass.
- Review confirms one complete display publication, target-display rather than source-DPI sizing, immutable callback points, generation-bound placement, event-driven progress without polling, exact negative-coordinate preservation, honest cross-platform capability, and no parallel display or geometry authority.

### Post-U32 Candidate Roadmap

The following labels are planning handles for separate design/implementation epics. They do not
add requirements, acceptance credit, or Definition-of-Done work to this convergence plan, and U32
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

1. **Pure/domain tests:** form generations/status, overlay stack policy, focus ordering, token/schema, typeahead session, table characterization, transform validation/composition/inversion, presentation lattice, live semantics, reveal alignment, rounded-clip containment, Dock style completeness, surface revision categories, coherent two-field activation-policy mutation, window-mutation capabilities/outcomes, mailbox ordering/merge/barriers, Dock window-session generations and terminal convergence, point-hit-stack routes, locked release decisions, foreign-surface rejection, and orthogonal provisional readiness/route/lease/release transitions.
2. **GPUI runtime tests:** real input dispatch, exact must-immediate hybrid native dispositions with zero busy entrances, focus traversal, per-window isolation, controlled lifecycle, final accessibility updates/actions/announcements, deferred theme capture, transformed scene/input/IME/cache behavior, dynamic hidden/inert cleanup, anchor binding, nested reveal, exact clipped hit testing, Dock activation completion, window request-versus-observation ordering, already-borrowed asynchronous callback replay, source-only post-borrow captured drag transport, ordered opaque-barrier resolution, provisional role/gate rejection, item/tabs/floating payload leases, semantic/focus proxy handoff, and borrow-free native effects.
3. **Projection tests:** UI state, every transformed and clipped scene primitive, final AccessKit tree/bounds/actions/live facts, allowlisted DevTools summaries, inspector geometry, Dock style/event/readiness projections, native callback/presentation generations, federated contract/Gallery/scenario/public-API bindings, and redaction agree.
4. **Gallery and example flows:** representative user journeys run through actual component adapters; U12-U17 and U30 exercise transformed and visible/inert/hidden subtrees, live regions, typed anchors, nested reveal, rounded clips, and accepted-frame activation through pointer, keyboard, scrolling, text input, deferred content, AccessKit, inspector, and programmatic-request paths. U18-U20 and U24-U29 use the Dock examples for scoped styling, events/snapshot export, stable-item activation, multi-viewport mutation, capture-owned drag, pre-release provisional presentation, and anchor teardown because the foundation Gallery intentionally excludes a Docking dependency. U31 uses renderer-owned native examples, while U32 uses platform window/display examples and native fixtures rather than adding backend state to Gallery.
5. **Owning-platform integration:** real native windows, message dispatch, system-level pointer injection, capture ownership and actual WndProc recipients, point hit stacks, locked release, exact client-geometry readback, non-empty renderer-submitted presentation, interaction-gate rejection, observed activation/focus/accessibility handoff, peer/explicit ownership, per-window surface isolation, renderer-before-HWND terminal observation, and process/window teardown prove claims that TestPlatform and visual injection cannot exercise. Fast model tests remain required but provide no native credit.
6. **Workspace/release gates:** formatting, checks, nextest, docs, xtask scanners, dependency/import boundaries, supported-platform renderer compile/ABI/render smoke, native window-mutation and multi-viewport integration tests, and release verification.

Focused commands are run per unit using the packages and test targets named above. After U32 and its review checkpoint, the deterministic local final gate is:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo nextest run --workspace --no-fail-fast --locked

cargo nextest run -p open-gpui --features test-support,inspector --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked
cargo nextest run -p open-gpui-wgpu --all-features --no-fail-fast --locked

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

The local command block is necessary but not sufficient for U12, U17, U20, U28, U31, or U32. CI must also compile each supported native renderer and platform window backend on its owning platform, run transform and clip primitive conversion/ABI tests, run the designated render-pixel smokes on capable runners, prove every advertised live window mutation from observed native facts, execute the real-window multi-viewport and event-driven display/client-geometry scenarios where supported, and run the real two-window renderer surface lifecycle gate for each backend claiming release support. Those jobs are part of the final gate even though no single developer platform can execute the whole matrix.

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
- Dock window-session correctness covers host-role separation, synchronous registry-commit activation of `Opening`, exact active admission leases, one session-lineage runtime handle registry, freeze-before-close, forced policy bypass, multi-surface isolation, stale callback rejection, active App-shutdown registry clear, and `Closed` only after exact anchor/runtime/ticket terminal convergence. Native owner relationships, first-render inference, close dispatch alone, or `cx.quit` cannot substitute for surface-scoped teardown.
- Native drag correctness requires one reserved exact generation whose callback-free start commit atomically activates GPUI authority and the prepared Dock consumer, OS-routed source-only captured input, actual capture/recipient observation, one callback-scoped physical pointer frame, a sampled-point-bound classified observation with coherent embedded geometries, independent Windows frontmost agreement, full registered sibling IDs, target-local conversion across mixed DPI, opaque ordinary/foreign barriers, visible typed foreign-surface rejection, post-borrow route consumption, complete deletion of poll/rendered-release/last-hover authority, and a locked `MouseUp` fact that later cursor motion, DPI queries, or capture-change cannot replace. Live-undock correctness additionally requires one U26-owned provisional role, one item/tabs/floating payload lease, a generation-matched committed source semantic/focus proxy before payload mount, continuous frozen-source-to-visible-provisional handoff, orthogonal readiness/route/live-placement/final-placement/lease/release/destination-semantics/shutdown state, a backend-readable interaction gate with no user-action replay, a KTD36 exact client-geometry transaction followed by a private reveal-only command independent of later input, a continuously visible non-empty renderer-submitted peer-top-level provisional that follows the same HWND before normal `MouseUp`, terminal unavailable release without promotion, bounded early-release settlement, complete `PreparedPromotion` validation under the gate, KTD32's first irreversible receipt followed by exact forward settlement, KTD35 destination-semantics submission only after exact Graph, final placement, semantic frame, renderer submission, and lease proofs, immediate committed-destination recovery on Graph supersession, a generation-bound presentation-shutdown acknowledgement before HWND terminal, compensation, and complete cleanup. Direct target event injection, direct source message injection alone, `IsWindowVisible` alone, accepted-frame evidence alone, or a release-created window cannot satisfy the claim.
- U28 scenario workers do not rerun a failed scenario to manufacture success. Timeouts are failure bounds and cleanup guards, never a harness retry policy; this does not remove U24's explicitly tested two-attempt platform-command contract for initial presentation. Every failed worker must still terminate and destroy its HWNDs.
- Redaction tests use unique canary strings, including Table identity/debug-label/selector sources, and assert their absence from live capture, history, diff, Inspector detail/copy, session export, artifact, report, and Gallery fixtures. Post-hoc generic string sanitization does not satisfy this contract.

## Definition of Done

- U1-U20 and U24-U32 are implemented in dependency order. Candidate U21-U23 remain separate follow-on epics. Every unit's focused local tests pass before its commit; platform-owned CI evidence that requires a committed revision passes before that unit and the plan are declared complete.
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
- Creation-time focus, lifetime activation/click/input, permanent non-activation, accepted/submitted/non-empty presentation, and owner/transient relationships are independent facts. An ordinary detached viewport does not steal initial focus but later activates through KTD34's generation-bound native observation ticket; command acceptance, temporary `NoWindow`, or timer passage is not focus success. DockSurface lineage never implies native ownership; ordinary and provisional detached viewports are peer top-level by default, while an explicit supported owner remains available. A provisional starts with the final native lifetime profile but remains generation-gated until same-window promotion; unsupported ownership and optional flags remain explicit.
- Every facade-managed DockSurface has a private session authority distinct from the renamed viewport facade, explicit host roles, an `Opening` token, one exact active anchor generation/lease, public read-only status, and typed admission/open outcomes. Synchronous window creation must return a committed full `WindowId` before token validation activates the session; construction callbacks and first render cannot activate or register it. The surface owner stores phase/anchor/shutdown/tickets, the session-lineage runtime solely stores window handles, and opening failure or synchronous close settles every tentative mapping before viewport admission. Anchor shutdown freezes new work, cancels provisional/pending work, cancels the exact GPUI active drag/native capture/outbox generation and Dock route/feedback before dependent close, then force-closes dependent windows before a live anchor outside all borrows. A failed or borrow-conflicted close dispatch returns to pending and retries until native terminal; cleanup retains the first panic, applies or durably schedules every required cancellation and close effect, and only then resumes unwind. Shutdown bypasses `Prevent`/`MergeBack`, leaves other surfaces intact, and reaches `Closed` only after exact anchor/runtime/ticket convergence. Active App shutdown quiesces current presentation tickets before registry clear and converges without ordinary close observers; premature reopen, `cx.quit`, stale callbacks, and native owner lifetime assumptions are absent.
- Native captured Dock drag routes from one callback-scoped physical input frame and point-bound hit observation without target raw mouse delivery. Delivery occurs after source/Dock borrows release. The sampled point, source geometry, and each target's coherent embedded geometry cannot be mixed across frames; a later DPI query cannot reconstruct or move the input. Windows availability requires stable classification plus independent frontmost agreement and fails closed on omission, drift, cycle, malformed coverage, or point mismatch. Target-local logical coordinates are derived only after target selection, including across mixed-DPI monitors. `MouseUp` locks its point, observation, candidate, and generations; later cursor motion and normal capture-change cleanup cannot replace it. Routing passes through only the exact current provisional role, stops at ordinary/foreign/unknown opaque barriers, renders foreign-surface hosts as rejected targets rather than desktop fallback, clears one drag generation on every cancel/close path, and never drops onto stale, provisional, or occluded hosts. Polling, rendered outside-release, target-raw, and last-hover release authorities are absent from production code and current documentation.
- One Dock live-undock session exclusively owns the provisional role/reference, item/tabs/floating payload lease, immutable source snapshot, source semantic/focus proxy, readiness/route/live-placement/final-placement/lease/release/destination-semantics/shutdown axes, prepared promotion, presentation-shutdown ticket, and compensation saga. Threshold crossing revokes stale source interaction/semantic eligibility, commits the source proxy frame, then mounts the payload in one peer-top-level reusable viewport before release while a frozen noninteractive source visual prevents a presentation gap. KTD36 commits hidden client geometry, a private generation-bound reveal-only command makes only a current non-empty renderer-submitted provisional visible without activation or later input, and typed live-route placement keeps that same HWND following current movement across desktop/host/rejected/unavailable feedback while the graph/revision stay unchanged. The backend-readable generation gate rejects activation, input, IME, focus, routing, and AccessKit actions without replay. A locked early `MouseUp` atomically supersedes live placement with one `FinalRelease` settlement and cannot promote an empty shell or settle twice; unavailable release restores the source and retires the provisional. Every fallible promotion step and complete next-state validation finishes under the gate; KTD32's executor records the first irreversible receipt, settles later stages by exact replayable journal entries, and publishes one revision only after owner acceptance. `DestinationSemanticsAccepted` remains gated; only exact KTD35 renderer submission produces `DestinationSemanticsSubmitted`, removes the gate, and schedules KTD34 activation plus focus. Creation, renderer/surface, presentation, native preflight, promotion preparation, next-state validation, cancellation, or shutdown failure before that receipt restores only a live source; failures after it preserve forward authority or enter committed-destination recovery. A destination window, renderer, or surface terminal after the boundary but before submitted semantics acknowledgement enters one committed viewport-loss transition, preserves recoverable topology, retires the gate/ticket, and cannot roll back or publish a second drag release. Every terminal path avoids focus theft, acknowledges renderer/surface quiescence before HWND terminal, and leaks no window or duplicate semantic owner.
- Every renderer-owned native window follows KTD37. Zero extent suspends rather than submitting a synthetic `1x1` frame, ordinary resize cannot wait without bound on a shared device, usable suboptimal frames present before reconfiguration, surface loss is isolated and typed per window, and renderer shutdown drains exact last-use work before native terminal. Dock never receives backend-specific surface enums, and accepted-frame evidence cannot substitute for renderer submission.
- Windows real-HWND and subprocess gates run on a sentinel-preflighted capable interactive runner and cross WndProc, true system-injected pointer routing, actual capture/recipient and point-stack observation, locked release, continuous non-empty presentation, gate rejection, activation/focus handoff, peer/explicit-owner roles, renderer-before-HWND terminal close, and process-lifetime boundaries. Deterministic final AccessKit-tree/action gates prove accessibility ownership without claiming OS assistive-technology E2E. Direct-message or TestPlatform coverage is labeled as simulation and cannot claim native end-to-end credit.
- Action presentation and command execution remain separate, with no speculative replacement runtime.
- ADRs and breaking migration documentation match the shipped architecture; stale helpers, aliases, evidence, and docs are deleted.
- DevTools allowlist and canary tests prove that sensitive free text cannot enter or persist through capture, inspection, export, artifact, report, or Gallery paths.
- The complete Verification Contract passes, `git diff --check` is clean, review findings are resolved, and no user-authored unrelated change is reverted.

## Appendix

### Requirement Trace

| Requirement | Owning unit(s) | Completion evidence |
| --- | --- | --- |
| R1-R2 | all units; preservation gates in U10-U20 and U24-U32 | import/dependency scans and preserved-module focused tests |
| R3 | U1 | lifecycle table tests from store through UI/DevTools |
| R4 | U2 | final `TreeUpdate` capture and real action dispatch |
| R5 | U3, completed with U4 | real nested Tab/Shift-Tab and restore tests |
| R6 | U4 | pilot/fleet migration, runtime tests, old-tail absence |
| R7 | U5 | all-producer migration inventory plus representative final-tree tests |
| R8 | U6, U30 | callback inventory, activation matrix tests, public `ClickEvent` absence gate, and accepted-frame publication/rollback evidence |
| R9-R11 | U7-U8 | scope tests on old/new payload, schema/recipe scanners, deferred capture |
| R12 | U9 | fake-clock cross-collection tests and duplicate implementation deletion |
| R13 | U10, U28 | federated binding fixtures and source-scanner deletion plus the native-test-owned scenario manifest, typed runner binding, stable IDs, unique ownership, and duplicate/simulated-only rejection |
| R14 | U5, U10 | typed-identity/stage tests and post-U5 Table characterization through export cleanup |
| R15 | each breaking unit; U11 audits prior surfaces; U12-U20 and U24-U32 own their migrations | same-unit migration docs/Gallery/examples/DevTools updates and final residual scan |
| R16 | U5; preservation gates in U10/U11 | occurrence invalidation and explicit-instance focus/edit/callback/NodeId/measurement tests, normalized partial-order characterization, and Table redaction canaries |
| R17 | U12 | checked construction/inverse/composition and numeric fail-closed tests, layout invariance, all-primitive scene projection, inverse input/capture, IME/debug/a11y/deferred/cache/motion coverage, Gallery flow, and supported-renderer matrix |
| R18 | U13 | exact channel matrix, nested/dynamic suppression, stale-state cleanup, transformed/deferred/cache/portal coverage, final-tree absence, and old-authority deletion scans |
| R19 | U14 | descriptor mapping, final-tree live facts, queue generations/order/removal, window isolation, focus stability, privacy canaries, and native adapter gates |
| R20 | U15, U17 | current/committed binding order, typed unlink/errors, transformed/clip/presentation snapshots, rounded-stack AABB regression, official overlay migration, and stale-geometry absence |
| R21 | U16 | nested-axis alignment, focus/AccessKit/application parity, transform-correct deltas, generation cancellation, virtual materialization, and Gallery flow |
| R22 | U17 | exact rounded containment, all-primitive clip projection, cross-channel inheritance/reset/failure, accessibility limits, renderer ABI, and native pixel smokes |
| R23 | U18 | complete style/state lookup, immutable resolver isolation/purity, window/subtree and out-of-band drag-generation resolution, payload identity stability, guide-metrics rename, dependency and literal-color scans, Dock example smokes |
| R24 | U19, U26-U27, U29 | explicit transaction-boundary/coalescing matrix, unique activation-host and window-session generations, commit-only revision/event matrix, stable-item activation completion, surface lifecycle, revision-consistent snapshot export, caller-owned fake-clock debounce, and provisional non-revision tests |
| R25 | U20, U24-U25, U29, U28, U32 | capability plus dispatch/terminal-outcome matrix, KTD36 coherent client-geometry/placement/restore-bounds conflicts, KTD38 complete display publication and target-scale binding, coherent two-field activation-policy domain, KTD34 native activation ticket and loss-before-gain settlement, committed getter authority, Dock independent-domain partial sync, callback-safe observations, separate appearance/activation facts, native owning-platform tests, and old-capability/direct-backend absence scan |
| R26 | U24, U27-U28, U30, U32 | AppCell-owned ingress, application sequence and merge/FIFO/barrier matrix, synchronous-query snapshots, exact App-idle hybrid input disposition with zero busy entrances, closed post-borrow command FIFO, already-borrowed asynchronous replay, full-`WindowId` commit/rollback isolation, borrow-free subordinate effects, accepted-or-reinvalidated frame tests, captured transport post-borrow delivery, event-driven geometry wakes without polling, real nested native-message evidence, and accepted-frame activation publication/rollback evidence |
| R27 | U25, U29, U28, U31 | independent appearance/activation/input/owner contract, peer-top-level default plus explicit owner, deletion of live focus-on-appear mutation, KTD34 observed activation, accepted/submitted/non-empty presentation generations, KTD37 per-window surface lifecycle, no-focus-then-activate native flow, owner capability tests, and same-window session promotion |
| R28 | U26, U27, U29, U28 | host-role separation, synchronous registry-commit activation, exact active lease, pending/committed provisional and close-ticket ownership, sole session-lineage runtime registry, typed public status/outcomes, exact GPUI capture/drag/outbox plus Dock route/feedback cancellation before close, retryable busy dispatch, cleanup-before-first-panic propagation, dependent-`WM_NCDESTROY`-before-anchor and App-shutdown ticket quiescence/terminal convergence before `Closed`/registry clear/reopen, stale generations, multi-surface isolation, real HWND/process evidence |
| R29 | U27, U29, U28, U32 | reserved exact-generation atomic start for GPUI authority and prepared Dock consumer, OS-routed source-only captured move/up, actual capture/recipient facts, callback-scoped native physical frames with KTD38 target-display observations, hover-only `WM_MOUSELEAVE`, sampled-point-bound classified observations with coherent embedded geometry and independent frontmost verification, resolution through only the exact provisional role, target-local conversion across mixed DPI, opaque ordinary/foreign barriers, exact session/complete-scene-frame eligibility, exact paired-feedback cleanup, visible foreign-surface rejection, locked `MouseUp` facts, poll/rendered-release/last-hover authority deletion, panic-safe single-terminal cancel/close barriers, stale-G1/replacement-G2 isolation, and real two-HWND preview/drop evidence |
| R30 | U29, U28, U31-U32 | one live-undock session, pending-open cancellation/bind-and-compensate, payload-subtree presentation lease, generation-matched committed source proxy before payload mount, frozen-source visual continuity, orthogonal readiness/route/live-placement/final-placement/lease/release/destination-semantics/shutdown state, KTD36 hidden client-geometry transaction plus reveal-only show, KTD38 target-display generation binding, same-HWND continuous route movement and locked callback-frame release, native hit transparency and point-scoped Z-order evidence, terminal unavailable release, provisional role, fail-closed gate/no user replay, complete `PreparedPromotion` validation, KTD32 `Abortable -> ForwardOnly` exact-receipt settlement, KTD33 transaction-level staging retention and one final Graph sweep, KTD35 renderer-submitted semantics only after exact Graph/final-placement/semantic-frame/lease proofs, KTD37 per-window renderer surface lifecycle, committed-destination recovery on Graph supersession or post-boundary/pre-semantics terminal failure, exhaustive release/cancel/failure table, generation-bound `PresentationShutdownTicket` acknowledgement before HWND terminal and App registry clear, and leak-free native teardown |

### Priority Rationale

- **P0 correctness:** U1 and U2. They fix data corruption risk and make a critical user-facing authority observable.
- **P1 interaction/runtime:** U3-U6, U12-U17, U19-U20, and U24-U32. They resolve modal, focus, accessibility, observed native activation, client geometry, target-display scaling, complete display topology, renderer submission, per-window surface isolation, presentation, announcement, anchoring, reveal, clipping, Dock application ownership, platform-event loss, native window semantics, session teardown, accepted-frame programmatic control activation, and real multi-viewport drag with the largest user impact. The later units were discovered or promoted by substrate, real-consumer, and owning-platform audits; their execution position does not reduce their severity.
- **P2 framework depth:** U7-U9 and U18. Scoped resolution is proven before the complete Theme v1 replaces its payload; typeahead improves interaction consistency independently; Dock visual styling then consumes the established theme boundary without inverting dependencies.
- **P3 convergence/release:** U10-U11. They delete drift-prone scaffolding only after executable authorities can replace it.

### Deferred Follow-on Research

These candidates have no requirement IDs, acceptance credit, or Definition-of-Done credit in this
plan. Candidate U21-U23 are post-plan scheduling handles only; U12-U20 and U24-U32 must not add public
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
- Dock retains its canonical retained `DockGraph`, n-ary same-axis split normalization, transaction/session generations, and explicit `DockLayout`/`DockSurfaceSnapshot` persistence values. U18-U20 and U24-U29 do not port ImGui's immediate-mode context or binary node identity; U29 adds a transient payload presentation lease without making provisional state durable topology.
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
- Before U25 public API: require Windows/macOS/Linux/web review of creation-only `focus_on_appearing`, lifetime activation/click/input, deletion of live focus-on-appear mutation, permanent non-activation, owner/transient semantics, accepted/submitted/non-empty presentation, KTD34 activation dispatch/positive-observation/cancellation and loss-before-gain semantics, and same-window session promotion.
- After U26: require Dock/application/platform review of host-role separation, Opening-token versus exact-Active-lease admission, pre-commit rollback versus post-commit presentation-failure shutdown, first-render non-registration, non-overlapping owner/runtime state, close-request veto plus dependent-before-anchor removal, exact anchor/runtime/ticket convergence, App-shutdown clear/flush settlement, forced close-policy bypass, stale callbacks, commit-only revisions, public status/rename migration, and two-surface isolation without `cx.quit`.
- Before U27 public transport: require GPUI/platform/Dock review of drag generation, source-only post-borrow delivery, point-scoped typed hit stacks in physical device-pixel desktop coordinates, exact sibling `WindowId` preservation, child-only normalization, target-scale local conversion across mixed DPI, fail-closed opaque barriers, foreign-surface rejection, locked `MouseUp` facts, terminal ordering, and absence of target raw-event or outside-poll authority.
- After U27: require native-input/Dock review of source-capture transport, target raw-event independence, point-stack capability/fallback honesty, opaque occlusion, current host-scene/session eligibility, `MouseUp` latching despite later cursor motion or capture-change, all terminal cancellation paths, and proof that no native or target effect runs under a source/Dock/runtime borrow.
- Before U29 live content: require the phase-zero real-HWND proof and named `open-gpui-windows-native-interactive-ephemeral` runner contract from KTD31, including KTD36 hidden client-geometry readback, non-empty renderer submission, non-activating reveal-only show, native hit transparency, same-HWND A-to-B-to-C movement, immediate post-`MouseUp` divergence resistance, representative live payload handoff, same-HWND role conversion, KTD34 activation terminal observation, and renderer quiescence. Then require Dock/GPUI/accessibility/platform review of the single live-undock session, exact U26 provisional role/ticket, item/tabs/floating payload lease, exact `SourceSemanticFocusProxy`, generation-matched source-proxy commit barrier, frozen-source visual continuity, orthogonal readiness/route/live-placement/final-placement/lease/release/destination-semantics/shutdown axes, backend-readable interaction gate with no user-input replay, point-scoped contiguous-band Z-order, generation-bound interaction feedback, peer-top-level default, private generation-bound reveal, continuous visible provisional feedback, unavailable recovery/release, locked `ReleasePending`, committed viewport-loss recovery, exhaustive release/focus/accessibility decisions, complete pre-boundary validation, KTD32 forward-only settlement after the first irreversible receipt, KTD33 Graph staging retention, KTD35 accepted-then-submitted semantic authority, and production renderer/surface quiescence before native teardown.
- After U29: require native-input/Dock review of exact provisional-only transparency, one content renderer and one semantic owner after the source-proxy barrier, no empty shell or visual handoff gap, KTD36 exact client geometry before reveal, visible non-empty renderer submission before normal release, same-HWND continuous live-route placement with no host-transition hide/show, atomic `FinalRelease` supersession, same-HWND promotion, one-window reuse, payload conflict policy, KTD35 destination semantics only after exact Graph/final-placement/semantic-frame/submission/lease proofs, KTD34 observed activation, fail-closed recovery on Graph supersession, KTD32 receipt-driven forward settlement and committed-destination recovery, KTD33 single-sweep Graph convergence, U26 shutdown integration, and `PresentationShutdownTicket` acknowledgement before HWND terminal.
- After U28: require evidence review proving the designated tests use U24's real-HWND support, system-level pointer injection without a selected receiver, actual WndProc/capture/point-stack/client-geometry/present/lifetime boundaries, same-HWND A-to-B-to-C movement, locked release despite immediate later movement, negative-origin and mixed-DPI capability honesty, activation loss-before-gain and terminal outcomes, gated input rejection, peer/explicit-owner roles, renderer-before-HWND teardown, delayed-close/reopen exclusion, every reported regression's failure path, deterministic worker cleanup, and honest VisualTest labeling.
- After U30: require activation/GPUI review proving candidate bindings publish only from accepted frames, rejected or rolled-back candidates preserve the visible committed target, exact absence clears without affecting a replacement, and cached journal replay cannot create a second frame authority.
- After U31: require renderer/platform/window review proving KTD37 per-window isolation, zero-extent suspension, no unbounded ordinary resize wait, suboptimal render-then-reconfigure, typed per-window surface terminal, exact KTD35 submission evidence, renderer-before-native shutdown, and real two-window owning-platform gates without backend vocabulary leaking into Dock.
- After U32: require platform/window/Dock review proving KTD38 complete display publication, target-display rather than source-DPI sizing, generation-bound client geometry, timer-free native wake, signed negative-coordinate preservation, and honest X11/other-backend frame-extents capability.
- Before completion: simplify-code pass, structured code review, supported-platform renderer/window-mutation/multi-viewport evidence, full Verification Contract after U32, and release-doc audit.
