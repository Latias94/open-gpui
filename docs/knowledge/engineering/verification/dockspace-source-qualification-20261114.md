# Dockspace Source Qualification — November 14, 2026

## Fixed Sources

This continuation starts at Open-GPUI
`3a99a5788092b072b207afbb9328ae4c40b622d2`. The production dockspace dependency
remains `c200977d0f34943284a125c492919791650e1bff`, with `serde` enabled and
default features, including motion, disabled. The fixed candidates are:

| Candidate | Exact revision | Purpose |
| --- | --- | --- |
| Original-preview retirement | `5ff5c2c2df0bcbe823771eef985e8d0eed1ec66a` | Settle releases using preview outputs emitted before pointer-up. |
| Complete reviewed range | `31366dbf5deedcdc3edcf8957c494224f8bea730` | Also include endpoint revocation, typed failures and resize corrections. |

The base is an ancestor of both candidates. The sibling is not shallow and the
fixed range has no missing objects. There are **42** commits from the base to
the complete candidate: **23** through `5ff5c2c2`, then **19** more. The merge's
second parent is the existing dependency base. Contributions already present in
that parent are identified below rather than counted as new corrections.

Tracked source snapshots are extracted with `git archive` at the two exact
revisions. No sibling file, branch or untracked work is modified. Cargo uses
the existing Open-GPUI target with one build worker and a temporary Git-source
patch. Full Cargo metadata must identify the archived candidate's manifest
before a candidate execution is credited. A `--config` argument for nextest
must follow `cargo nextest run` so that nextest forwards it to its Cargo
children. Passing it only to the outer Cargo invocation does not establish the
tested dependency source.

The snapshot trees, paths and resolved manifests are recorded in
`/tmp/open-gpui-dockspace-fixed-sources-20261114.json`,
`/tmp/open-gpui-dockspace-5ff-resolved-source-20261114.json` and
`/tmp/open-gpui-dockspace-31366-resolved-source-20261114.json`.

## Open-GPUI Delivery

| Open-GPUI commit | Owning change | Source and execution scope |
| --- | --- | --- |
| `bd9b7aa5` | Native delivery lanes distinguish a frontmost blocking control from an empty background. | Reference `e76172a8`/`5558c6a4`; reproduced original failure and three real GPUI hit-graph controls at the production pin. |
| `2a10311f` | Release to an accepted, unsubmitted destination preserves the source child and View. | Actual GPUI admission boundary, with explicit candidate guard, retirement, new gesture and content-input checks. Not the sibling's pending-preview reproduction. |

The two commits retain the original production dependency and public API.
Candidate source and API experiments remain local qualification artifacts.

## Per-commit Inventory

"Candidate" below means the fixed archived source contains the contribution;
it does not mean that the production dependency pin has moved. Core changes
belong to dockspace. GPUI continues to own its original entities, frame tickets,
input hit graph, output retirement and native resource lifetimes.

| Full dockspace commit | Contribution and disposition for Open-GPUI |
| --- | --- |
| `ae67e035e094c08992b1f1b514db11767622e3a6` | Popup-local motion settlement; the relevant production blobs are already in the base. No new adoption credit. |
| `dcf05398b6a6b7abe3e38634ba2145f55355cc88` | Upstream egui/winit output and window-retirement wakeups; core changes are contract documentation already in the base. GPUI retains its own continuation and retirement owners. |
| `9df72e42ca7902a4d7f3c3754273502215077a2c` | Motion scheduling in the sibling's separate GPUI harness; does not enable motion in this adapter. |
| `1e697b1a180229f91fcf8628c8f7ab91e68ca958` | egui-native retirement after bounded service commits; no GPUI production code to copy. |
| `c4bbf5f77cf711271906be13a3f0deb616d7accd` | Workbench executable logging; embedding applications retain logger ownership. |
| `573c31be21ad28473f0b764e1d2e0c2da3147a77` | Merge includes a real core Dock Back policy/preflight correction relative to the base; present in both candidates. |
| `75abd0f5a6f56bb01ec0e223ef632cf04fa1e7c9` | Core capture-gate recovery, valid motion targeting and exact contained geometry; present in both candidates, no API migration. |
| `52ae3e5ad1e0544613ac48dbd9875056b9931a37` | egui focus and interrupted-pointer recovery; GPUI uses its own native producer. |
| `b50405fcbe43d7e50a72724c7f42e1b0a0baef82` | Native translator recovery and multiview journeys; useful reference coverage, not GPUI input qualification. |
| `7a9cbaaa1ea08bfc5d3e021950861586786b4df7` | Core exact guide partition and clipping boundaries; present in both candidates. |
| `4fda06a5d56d773bf5405a04bed88b91f828397d` | egui overflow and docking recovery regressions; no new shared production behavior. |
| `a1c77d085a410fab23f8e5686fc835d18072d405` | Native constrained-target and unavailable-target regressions; no new shared production behavior. |
| `c2423e56a36a651e7d05e4d1b7ac808f6057a7d8` | Core atomic empty-contained-root retirement after insertion; present in both candidates. Main central roots retain their distinct lifetime rule. |
| `c1bd05980adb1c779a89b5d3c815832983fe0ad8` | Optional core pane compression; default remains Clip. The actual GPUI adapter has no affected egui style literal. |
| `ea1cdf03a002d23ed1c1c99a1f2b561f6a9a48c7` | egui facade action/result correlation; GPUI retains its own frame/report boundary. |
| `73c63f66dc8508dadb9716b0a52e0899d44f7165` | Additive core root-content queries and native host protection; no mandatory GPUI API migration. |
| `9d40c2f7ac21df1a2ac138806d7f79da05249183` | Native renderer recovery and diagnostic visibility; no shared core correction. |
| `82b79d4ebd2e6c05afd087915b901716685e26e3` | Native application protection of main central roots; the shared root rules retain their own owner. |
| `fe4f6e483ff52cddf3b9cff051075adef9e3d9cb` | Native fatal-state presentation; outside the GPUI adapter. |
| `b614d60dac79d66b1dc76b0e04dfcc0d94c1a334` | Native disabled-state diagnostics; outside the GPUI adapter. |
| `ef940f5769355c0beb5d8f618a44d2c1ccba944a` | Additive `retains_empty_central_root` query for existing semantics; no forced migration. |
| `752a50d6fab601a0c46b35aa01a3f5c590b61616` | Public multiview/root-lifecycle regressions; reference coverage only. |
| `5ff5c2c2df0bcbe823771eef985e8d0eed1ec66a` | Core pending release inherits already-emitted outputs matching its exact preview and active stream. All inherited copies Dropped can cancel without moving the View or releasing its child. Present in both candidates. |
| `3622f0369bcae96a8ecff5210c6550f04133b98c` | Stronger public multiview and deferred-close regressions; no new production behavior. |
| `761e2aeb425f8d6fe0781dc3a19de68fc8457117` | Preserve the first typed host-frame reduction failure through poisoning and public errors; complete candidate only. |
| `7a7b9c1ebdfa4cb2717ce59610382d585effc667` | Cancel pending releases when source/target platform authority expires, including after the pointer becomes idle; complete candidate only. Late output settlement cannot revive the mutation. |
| `25662bbd404805d29bc9b18100c74419e4fe98ae` | Native translator duplicate-release suppression; does not replace GPUI's generation correlation. |
| `e76172a8bb13459ba247effdfa521a74d902f672` | Native splitter click-lane blocking. Source review exposes a corresponding GPUI resolver defect, repaired independently in this continuation. |
| `4b6a65f0e7217e1acb8cd5f64792dc0ccb14a605` | Example operation diagnostics; no core replay protocol or GPUI adoption. |
| `6eda3ad50c69b232b767358fc0062d4942be0d5f` | Public drained-layout, motion and output-terminal permutation regressions; no new production behavior. |
| `5558c6a40abc6c52d143bfcfede33cfc1c476a34` | Breaking receiver API names make empty-lane claims explicit. Requires the bounded migration below when selecting the complete candidate. |
| `c5295feb2f940311f61602d929c389025201ec23` | Internal single-owner pending-mutation progress and cancellation; GPUI must keep using public output reports, not copy this state machine. |
| `992bfb2e8b675d31424a8fc6b53b4268b5c24a62` | Native pointer normalization refactor and tests; no GPUI producer replacement. |
| `605ee01214f898426685ed842c133b225a0ac2b6` | Colocate protocol regressions; no production behavior change. |
| `5dc62b8621e20725e538fefbbba97757cbb8e2f2` | Reorganize private core modules by domain. The adapter's public imports retain their entry points. |
| `c0ae12c0593d816944fad3c95dbcafcd500bdd1d` | Join native callback journeys with real egui hits; does not qualify GPUI physical input. |
| `4ed000e82118c723a00560183640c6b96a24d01b` | Native example editor-drain/divider physical smoke; reference coverage only. |
| `96d255371864a5762ae3490fcf9323f579450c17` | Native wheel registration recovery after device removal; GPUI has a separate wheel producer. |
| `c99a0a6f56ffa372efa211df86f69859d9f6addd` | egui measurement refresh after cancellation. GPUI already consults the current paint plan's measurement decision. |
| `d44651a4a9cf6481dad113648968adc5aad49456` | Core active-resize projection refresh and final release coordinates; complete candidate only. Projection freshness also matters with motion disabled. |
| `72b6c26731d69ba0861538bd72482fc40b7a3e9a` | Core consistent resize cancellation and immediate motion recovery; complete candidate only. Motion-specific benefits remain outside the current feature selection. |
| `31366dbf5deedcdc3edcf8957c494224f8bea730` | Native two-endpoint revocation regressions, including late outputs, abandoned candidates and a fresh gesture; no additional production correction. |

## Adapter Receiver Correction

The actual GPUI `host_native::resolve_in_window` uses exact submitted artifacts
and the current immutable hit graph. A frontmost splitter can accept drag while
blocking click. Failure to bind its descriptor to ClickDelivery does not prove
that the semantic click lane is empty. The previous fallback incorrectly
returned `NativeReceiverAnswer::NoReceiver`.

The correction returns `Blocked` for that frontmost descriptor. Actual exposed
background without a semantic receiver retains `NoReceiver`; missing original
presentation correlation retains `Unknown`. This is an adapter correction
compatible with the current pin, with source reference `e76172a8` and the
explicit contract in `5558c6a4`. It does not depend on upgrading dockspace.

Three regressions use a real core painted program and real GPUI hitboxes:

- The fractional point inside a drag-only splitter sees the underlying semantic
  PaneBody in the click lane, but the frontmost framework splitter blocks it.
  The separate drag lane still binds the splitter and starts a gesture.
- A point in exposed rootless background has no semantic receiver in either lane.
- Removing exact presentation correlation keeps a real hit `Unknown`.

Restoring only the original fallback makes the splitter regression fail with
actual `NoReceiver` versus required `Blocked`; the other two controls pass.
The original-source run is `d1871ff7-2fb4-4b47-9003-a14c047896be`, retained in
`/tmp/open-gpui-dockspace-receiver-original-red-20261114.log`.

## Unsubmitted Native Destination Control

The new GPUI release control creates a real managed child using the existing
deterministic platform, paints its original View, starts a joined native gesture
and leaves the destination's original GPUI output accepted but unsubmitted.
It preserves event-time physical geometry in the test input; it does not claim
to observe an operating-system mouse or capture.

The initial test expected a pending release like the sibling's native-harness
scenario. Actual GPUI execution instead returns
`Cancelled(UnknownTargetAuthority)`. That initial observation does not establish
an accepted, unsubmitted destination output; the stronger guard below qualifies
that precondition separately. The diagnostic run is
`75425c17-4185-4c55-a502-0c185b83450f`; this is a fixture-expectation failure,
not reproduction of the `5ff5c2c2` defect.

The retained control requires that conservative cancellation. Retiring the
original unsubmitted outputs cannot revive the transfer or remove the child.
The source View entity, document and binding remain original; a fresh drag and
subsequent content click work, and native shutdown becomes quiescent. The
control protects GPUI's admission boundary without claiming to cover the
sibling's already-emitted-preview pending-release regression.

Adding the explicit accepted/submitted precondition first exposes a fixture
gap: **69/70** pass, while the new test finds no pending destination output.
The adapter had reused an already-submitted packet, so merely setting the test
renderer's outcome to Deferred did not establish an unsubmitted candidate.
The corrected fixture marks that destination region dirty, as the existing
cross-window candidate control does, then requires an accepted generation and
no submitted generation before pointer-up. The complete final current-pin run
passes **70/70** with this stronger precondition. The original failure remains
in `/tmp/open-gpui-dockspace-current-default-20261114.log`; it is not a
production regression or evidence that candidate output retirement failed.

## Public Native Revocation Regression

The identical, unmodified public test
`integration/dockspace-host-harness/tests/native_presentation_revocation.rs`
from `31366dbf` is also executed against both archived dependency candidates.
This standalone temporary consumer tests only dockspace's public protocol,
without GPUI, egui or an operating-system window. It starts a pending native
release, changes the authoritative work-area facts and reports the original
output. The resulting terminal must cancel exactly once without reviving the
mutation.

Against `5ff5c2c2`, the test fails the required
`Cancelled(SceneUnavailable)` assertion: run
`6d580d54-ec49-4213-95e3-abc6bff1cd06`. Against `31366dbf`, the same source
passes **1/1**: run `11f749c2-130e-4755-9675-34ee50a75346`. This confirms
the later revocation contract and supports choosing the complete candidate;
it does not isolate every intervening commit or reproduce the separate
`5ff5c2c2` pre-release-output defect. Both resolved manifests and the shared
test hash are in
`/tmp/open-gpui-dockspace-public-revocation-receipt-20261114.json`; process logs
are `/tmp/open-gpui-dockspace-public-revocation-{release_fix,candidate}-20261114.log`.

## Complete-candidate API Migration

At the starting source there are four production references to removed names:

| Existing API | Complete-candidate API | Actual owner |
| --- | --- | --- |
| `with_no_delivery` | `with_empty_delivery_lane` | Local pointer facts in `adapter/input.rs`. |
| `with_no_drag_delivery` | `with_empty_drag_delivery_lane` | Local pointer facts in `adapter/input.rs`. |
| `NativeReceiverAnswer::NoReceiver` | `NativeReceiverAnswer::EmptyLane` | Two resolver fallbacks in `adapter/host_native.rs` before the receiver correction. |

The receiver correction removes the incorrect fallback rather than relabeling
it as an empty lane. The remaining production migration is three name changes;
the new empty-background regression has one matching variant change. Hover's
`with_clear_hover_lane` retains its independent meaning and must not be changed
to an empty-lane claim. The complete candidate is checked with only those four
temporary name changes; the exact patch is retained at
`/tmp/open-gpui-dockspace-31366-api-migration-20261114.patch`.

The public adapter re-exports `dockspace`, so a future pin update must also call
out the renamed APIs for consumers. Temporary candidate API and lock changes
are removed after qualification; the published current-pin adapter must remain
buildable with its declared dependency.

## Executed Qualification And Publication Boundary

The fixed `5ff5c2c2` snapshot passes the complete current adapter library:
**70/70**, zero skipped, before the strengthened release precondition. Its
resolved-source receipt and execution log are
`/tmp/open-gpui-dockspace-5ff-resolved-source-20261114.json` and
`/tmp/open-gpui-dockspace-5ff-verified-source-20261114.log`.

The complete `31366dbf` snapshot passes default adapter **70/70** and the
all-feature/legacy library and test cohort **1,546/1,546**, zero skipped. The
nextest runs are `c587411c-ead5-4667-bf1e-22c81be28ba3` and
`34f44c69-e842-4781-9701-73aa39c736e5`. Logs are
`/tmp/open-gpui-dockspace-31366-verified-source-20261114.log` and
`/tmp/open-gpui-dockspace-31366-all-features-20261114.log`. These initial
candidate runs precede the stronger destination-output guard above; the final
four-control reruns retain that guard with separately verified candidate
manifests in
`/tmp/open-gpui-dockspace-final-candidate-controls-receipt-20261114.json`.

After removing the temporary dependency/API hunks, the declared production pin
passes final default **70/70** and complete/legacy **1,546/1,546**, zero skipped,
with the stronger release precondition. Runs are
`689f6d0b-7b3f-4c75-ab2c-f7f1def03c6c` and
`2e0b2f1b-eac0-4233-864e-bb47557f6018`; logs are
`/tmp/open-gpui-dockspace-current-default-final-20261114.log` and
`/tmp/open-gpui-dockspace-current-complete-20261114.log`.

The final four-control runs against both candidates pass **4/4**, retaining the
stronger accepted-but-unsubmitted guard: `0cd081f9-a1b7-47f6-90f0-3f408d60e016`
for `5ff5c2c2` and `32ecc88f-dce3-4d6c-a07e-c72930f28011` for `31366dbf`.
These are focused runs with 66 other tests filtered, not additional complete
suite receipts. Package formatting and current-pin all-feature/all-target
Clippy pass with `-D clippy::correctness` and `-D clippy::suspicious`;
advisory warnings remain. Independent diff review finds no blocking defect and
corrects the historical fixture/source attribution in this record.
Rust **1.95.0** current-pin all-feature/all-target compilation also passes in
**1m22s**, including the new regressions. Its log is
`/tmp/open-gpui-dockspace-current-rust195-20261114.log`. This qualifies the
published adapter's declared compiler floor; it is not a candidate compiler-floor
receipt or a new runtime execution.

The rebuilt ordinary editor is also exercised through the existing bounded
signed-App helper at the production pin. Its first launch times out after
30 seconds in `RootSubmitted`, with one exact native binding already Submitted
and a semantic update still pending. Actual OS wait status is **256** (normal
exit **1**, no abort). This is before the receiver correction is exercised;
the receipt does not establish the timeout's cause or connect it causally to
the earlier Mac sheet failure.

One subsequent launch of the same executable completes every original native
phase in **1.453 s**, with actual OS wait status **0**. It creates and admits a
child, submits through the original windows, preserves View identity and
local/shared edits through relocation, drains the original native group and
keeps an independent workspace alive until its own retirement. The executable
SHA-256 is
`b54ed1685176e84497641352deef66668a01855bf1b109d211718499ec5224b2`.
The original failure and success logs are
`/tmp/open-gpui-dockspace-receiver-native-smoke-20261114.log` and
`/tmp/open-gpui-dockspace-receiver-native-smoke-retry-20261114.log`, with
matching `*-receipt-20261114.json` files. Both launches remove their temporary
Apps, and the final process inventory is empty. This is one successful native
trajectory after a failure, not a stable repetition or timeout-repair claim.

The combined local result and final source blob identities are recorded in
`/tmp/open-gpui-dockspace-qualification-final-receipt-20261114.json`.

The earlier temporary standalone consumer cannot run a dependency's private
tests because Cargo requires that package to be a workspace member. A separate
outer-Cargo `--config` attempt still resolves the original Git dependency. Neither
attempt is credited as candidate runtime qualification. Subsequent source
receipts identify the actual archived manifest before execution.

The previous [consumer follow-up](ci-suite-consolidation-20261114.md#consumer-follow-up)
records `5ff5c2c2` as unavailable from the public Git source. The production pin
must stay fetchable. A source publication and an owning native pending-release
trajectory remain prerequisites to claiming the whole candidate integrated and
physically qualified. Windows native capture/hit testing, mixed DPI, scanout and
performance are not established by these deterministic controls.

A subsequent read-only remote check still finds dockspace `main` at
`475e1e36b64f4a6ee33073797f013a10aa7574f9`; querying `31366dbf` returns
HTTP **422**, `No commit found`. The sibling has additional concurrent tracked
and untracked work by this point. Candidate qualification uses the immutable
archives, and no sibling change is discarded, staged, committed or pushed.
