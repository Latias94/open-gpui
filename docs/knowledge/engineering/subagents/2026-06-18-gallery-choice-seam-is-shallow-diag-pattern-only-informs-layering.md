---
type: "Subagent Finding"
title: "Gallery choice seam is shallow; diag pattern only informs layering"
description: "Subagent Finding for Gallery choice seam is shallow; diag pattern only informs layering."
timestamp: 2026-06-18T17:34:58Z
tags: ["gallery", "choice", "seam", "diag", "architecture"]
source_session: "019ec6c8-5566-7062-8458-21ebe1360573"
---

# Finding

`repo-ref/fret` 里的 diag 结构是“薄入口 + 深实现”的好例子，但它更像层级分工范式，不是这里 choice 家族必须抽 page-local 模块的理由。当前 gallery 的 `Select` / `Combobox` / `Command` 仍然是页面内 sample/state reconstruction 的局部 glue，抽出来主要是整理代码，不会明显提升 headless 复用或跨平台抽象价值。

# Evidence

- `apps/fretboard/src/diag.rs` 只是把 `diag_cmd` 转发到 `fret_diag::diag_cmd`。
- `crates/fret-diag/src/lib.rs` 才承载真正的诊断实现。
- `repo-ref/fret/CONTEXT.md` 明确区分了 `Runtime Substrate`、`Policy Layer`、`Behavior Reference`、`Headless Surface` 和 `Portable Framework Contracts`。
- `examples/ui-foundation-gallery/src/pages/components.rs` 已经拥有 choice 家族的 resolved state 和 builder 组装。
- `examples/ui-foundation-gallery/src/shell.rs` 仍然是在页面层把 choice sample 重建成具体 widget。

# Recommendation

不要为了模仿 diag 的 layering 而把 choice 家族抽成 page-local 模块。若未来真要支持 headless 或跨平台复用，先找稳定的共享 contract；在当前阶段，这组代码更适合保持为 gallery 专用的局部重建逻辑。

# Disposition

当前任务下结论：浅 seam，先不拆。

# Citations

- [repo-ref/fret/CONTEXT.md](../../../repo-ref/fret/CONTEXT.md)
- [apps/fretboard/src/diag.rs](../../../repo-ref/fret/apps/fretboard/src/diag.rs)
- [crates/fret-diag/src/lib.rs](../../../repo-ref/fret/crates/fret-diag/src/lib.rs)
- [examples/ui-foundation-gallery/src/pages/components.rs](../../../../examples/ui-foundation-gallery/src/pages/components.rs)
- [examples/ui-foundation-gallery/src/shell.rs](../../../../examples/ui-foundation-gallery/src/shell.rs)
