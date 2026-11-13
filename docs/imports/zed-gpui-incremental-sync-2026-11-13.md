# Zed GPUI Incremental Sync — November 13, 2026

This continuation fixes the next comparison boundary after the completed
[564-commit reconciliation](zed-gpui-commit-reconciliation-2026-11-12.md).
The earlier ledger remains scoped to its original target; its zero-open result
does not assert adoption of later upstream commits.

**Final code checkpoint:** `ccd5070dfe502ef413b0259c359e66e69f63b1f8`.
The complete required Chromium workflow and the final **14/14 AppKit native
matrix** pass after the recorded source and driver corrections. All fourteen
native applications have actual normal exit status 0 as well as their success receipt.

## Fixed comparison

- Fork starting checkpoint: `d1ffee6315e72dd81f3ef690486860680a696e3a`.
- Excluded upstream left endpoint: `6ceb6fee8738274589b59139df27efc191ec2231`.
- Selected incremental target and separately observed official remote HEAD:
  `27bbbc1cc480a7500cec122f120e796d1fb8500c`.
- Read-only reference checkout: `78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`.
  No reference checkout, reset or fetch was performed.
- Source evidence: the official
  [fixed comparison patch](https://github.com/zed-industries/zed/compare/6ceb6fee8738274589b59139df27efc191ec2231...27bbbc1cc480a7500cec122f120e796d1fb8500c.patch)
  supplies ten full commit headers and every changed file. Individual official
  commit links below also provide permanent patch evidence through their `.patch`
  endpoints. The GitHub API was rate limited; the official comparison patch was
  retrieved successfully instead.
- The complete retrieved comparison patch is 260,622 bytes with SHA-256
  `d5cfd111d374b8b235a62e4c37ae6781691f3cb6a2f1790e95f349779adbcf90`.

Every changed path in all ten commits was inspected, including mixed framework
and product commits and root manifests/lockfiles. Four commits contain framework
contributions: three were missing and one already has an equivalent implementation.
Six commits only change product owners outside the extracted framework closure.
The two ledgers together inventory **574 distinct official commits**; this document
adds ten rows without changing the historical ledger's counts.

All four applicable contributions now have completed source dispositions; there
are **zero open source commits at this fixed endpoint**. Native/browser/device
qualification retains its own executed scope and pending entries below.

## Per-commit ledger

| Full official SHA | Official change | Framework disposition and local owner |
| --- | --- | --- |
| `78a8526196a11eb7c9b63069777fb416876a88f0` | [Update merman to 0.9.3 (#66874)](https://github.com/zed-industries/zed/commit/78a8526196a11eb7c9b63069777fb416876a88f0) | N-SVG: adopt the complete `SvgRenderer` text measurement, bounded owned-font bytes and fallback-selection cache, with upstream regression tests. The `merman` root dependency and Markdown/Mermaid consumers remain outside the framework. |
| `eda58e5040fd907874681efc93fce0bd93f96ce2` | [Keep history of unchanged tasks on tasks file updates (#66646)](https://github.com/zed-industries/zed/commit/eda58e5040fd907874681efc93fce0bd93f96ce2) | Outside framework: project task inventory, tasks UI and workspace task history only. |
| `3c846c3d491095d75a76ca00a99fe348439736f6` | [Fix sidebar and no titlebar aligment (#66877)](https://github.com/zed-industries/zed/commit/3c846c3d491095d75a76ca00a99fe348439736f6) | Outside framework: agent archive/sidebar layouts and sidebar tests only; no GPUI titlebar owner changes. |
| `4dff6f8eef9548d297073845fecb6c41de9f4c36` | [terminal: Route the remaining emulator access through the alacritty module (#66744)](https://github.com/zed-industries/zed/commit/4dff6f8eef9548d297073845fecb6c41de9f4c36) | Outside framework: terminal emulator access and Alacritty adapter only. |
| `7fab4c0527e10a466439b819d4e9e7fe34cd989c` | [language_models: Detect OpenRouter Anthropic aliases (#66857)](https://github.com/zed-industries/zed/commit/7fab4c0527e10a466439b819d4e9e7fe34cd989c) | Outside framework: the OpenRouter language-model provider only. |
| `498af674abe1f054869a4d77b5a96ed8a3f12008` | [fs: Improve responsiveness during heavy filesystem activity (#66835)](https://github.com/zed-industries/zed/commit/498af674abe1f054869a4d77b5a96ed8a3f12008) | Outside framework: Zed's filesystem watcher, which is not a selected framework dependency. |
| `d3fc053d5b62386247b15f069871eabe817032ab` | [gpui: Speed up sorting scene primitives (#66898)](https://github.com/zed-industries/zed/commit/d3fc053d5b62386247b15f069871eabe817032ab) | N-SORT: adopt small-key sorting, stable insertion ties, cycle permutation, the sorted fast return and one shared scratch buffer; add the official finish-only benchmark. Preserve full texture kind/generation, legacy ordering, exact clips and journal indices. |
| `73ad8e3ed01740371df9c1ea579e3f74c4e098d5` | [Fix opening `ssh://` links with a drive letter on Windows remotes (#66814)](https://github.com/zed-industries/zed/commit/73ad8e3ed01740371df9c1ea579e3f74c4e098d5) | N-URL: adopt the public `util::paths::url_path_to_path` helper and all official boundary assertions. Remote-project/workspace/open-listener consumers remain outside the framework. |
| `8e474e1598875be89a2225fb7ba0f6ebcdb7190a` | [gpui: Pass premultiplied alpha to wgpu shaders as a uniform (#66902)](https://github.com/zed-industries/zed/commit/8e474e1598875be89a2225fb7ba0f6ebcdb7190a) | N-ALPHA: the existing alpha uniform is equivalent. Apply the same official compatibility mechanism to the fork's LinearSrgb flag, which still supplied an unused override to some stages. The shared 32-byte uniform now carries two compositing bits; preserve all four alpha/color-space combinations. Safari browser execution remains separately qualified. |
| `27bbbc1cc480a7500cec122f120e796d1fb8500c` | [languages: Highlight C/C++ preprocessor call arguments (#66875)](https://github.com/zed-industries/zed/commit/27bbbc1cc480a7500cec122f120e796d1fb8500c) | Outside framework: language grammar queries, C/C++ language setup and their lockfile changes only. |

## Local implementation and verification

The owning validation record is
[November 13 Incremental Sync and Native Follow-up](../verification.md#november-13-incremental-sync-and-native-follow-up).
The fork's existing accepted-frame, exact-clip, atlas-generation and
renderer-retirement contracts remain in force.

| Owner | Full local implementation SHA | Executed evidence |
| --- | --- | --- |
| N-URL | `778d3c8f78bf16ff0ab8a7ad1f8edf79e4f833ce` | Official boundary assertions pass 1/1; Rust 1.93 library check passes. |
| N-SVG | `d1a258006c944d5f0f3a0f827b9c093e7357cf72` | Complete official SVG regression tests pass in N-CORE 2000/2000; independent source review finds no blocking omission; Rust 1.93 library check passes. |
| N-SORT | `dbe0189ff6a37075c2b77d11d7fd63bf72f7093a` | Stable ties, complete texture keys and retained exact-clip journal controls pass in N-CORE; finish-only Criterion smoke passes 24/24. |
| N-ALPHA / browser sampling | `03f02ef46ac09337d1bd81c21cce333262e69e7f` | Both compositing decisions use the existing uniform; no pipeline override constants. Alpha/color-space pixels and complete WGPU/linear-scene regressions pass 127/127. Chromium WebGL/Auto→WebGPU/explicit WebGPU render and resize panels pass all 22/22 samples at two sizes after explicit mip-zero image sampling. |
| N-NATIVE qualification follow-up | `7c511839c80707d1e6026442fd681a84fa99db42` | Correct fixture ownership/activation prerequisites and controlled failure reporting; a signed-bundle runner requires the success marker plus actual OS exit status. Real marker-then-exit 0/7/SIGKILL controls return 0/1/1. After one active display returns, the final native matrix passes **14/14** in 24.949 s, each with OS status 0; both resize variants produce newer real Submitted generations before exact renderer/native retirement. |
| N-BROWSER driver follow-up | `2bb852d413b5ffcb215f90f3619dc088b3216ada`; `d28f9e4df6fcac7ebd8ddf5891f3cdac5bb8e4f5` | macOS Meta+V and shared native editing commands preserve trusted input; Windows virtual key codes are no longer sent as native macOS key codes. Clipboard, text, font and the complete workflow pass with the original result assertions. |
| N-IME fork integration follow-up | `ccd5070dfe502ef413b0259c359e66e69f63b1f8` | A complete mirror synchronization uses one callback lease and handler identity; focus changes on release reschedule coalesced work. The real mixed-field selection failure is retained; twelve additional A/B/A round trips and unchanged A counts pass in focused and complete browser runs. This is a fork adaptation, not an invented fifth official SHA. |

N-ALPHA's pre-existing field, blend mode and shader formula map to
`c9571c486612f1bd9194faa028709e89e3f58b7e`; surface alpha-mode selection maps to
`67e1360709ca48be74bf986f116b9c004ff5bd59`; current frame upload maps to
`0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; external recording maps to
`0259c88f9deb26e4caa026ebd39be179d19a3ae5`. The separately adapted LinearSrgb
override was introduced at `909f510d18bd0d1e537faf73eb57947cee47f927`.

The compositing adaptation's final WGPU library and ordinary linear-scene consumer
cohort passes **127/127**, zero skipped. Its new real GPU control reads pixels
for encoded/linear RGB crossed with straight/premultiplied alpha. Naga parses
all three production shader variants and verifies every globals member offset;
the uniform remains 32 bytes with compositing flags at byte 8. Native tests do
not assert Safari execution or prove a browser backend was used.

The original browser shader rejection is a fork integration defect exposed by
real execution, rather than a fifth upstream commit omitted from this range.
Explicit mip-zero sampling preserves the ordinary no-instance-read fast return,
imported-image clamping, alpha conversion and exact clips. Both the earlier
rejection and corrected actual browser PNG results remain in the verification record.

The final required Chromium workflow exits **0** in **222.274 s**, with no
unavailable skip. WebGL, Auto-to-WebGPU, explicit WebGPU and the explicit
no-WebGPU Auto fallback retain actual PNG and resize controls. The same run
completes platform identity, trusted clipboard, text input with twelve focus
round trips, font fallback and WebGL touch. A separate explicit WebGPU touch
case passes in **35.176 s**. Browser WebGPU requests a fallback adapter; neither
that receipt nor the synthetic composition controls qualifies physical GPU/input,
OS IME or Safari. The verification record preserves the earlier failing attempts.

The sorting adaptation uses `usize` permutation indices and the complete fork
texture key. This preserves keys that cannot be represented by the upstream
packed `(u64, u32)` scratch entry. Its speedup must be measured locally; upstream
benchmark percentages are not measurements of this implementation.

The SVG adoption retains the fork package alias and bundled-font fixture paths.
It imports no Mermaid product dependency and changes no dependency version or
declared compiler floor. Measurement and rendering share the same `usvg` font
selection, fallback and shaping options. Owned file data stays within 64 MiB per
shared renderer state; fallback selection retains the upstream 16,384-entry cap.

Only this fixed endpoint is selected. A later remote HEAD observation starts a
separate bounded review rather than changing these ten rows during validation.
