# Image Regression Fixtures

These small fixtures preserve the official GPUI regressions at fixed Zed revision
`635026fabc38440fce5e1767d125026a17b12012`. They belong to the imported GPUI test
closure; the workspace's GPUI upstream license and attribution apply.

| Fixture | Source | Purpose |
| --- | --- | --- |
| `exif-orientation-rotate-180.jpg` | Exact `crates/gpui/examples/image/exif-orientation-rotate-180.jpg`, introduced by `9552acc2bc242d45342fa9b5a987d43868aee1ec` | A 16×32 JPEG with EXIF orientation 3; opposite corners expose dropped orientation. |
| `red-blue-100ms.webp` | Exact 200 bytes from `ANIMATED_WEBP` in `crates/gpui/src/platform.rs`, introduced by the fixed target commit | A 1×1, two-frame lossy WebP, red then blue, with 100 ms per frame. |

The fork adds public-loader, alpha, orientation-matrix and presentation consumers around
these fixtures. Those additional controls are fork qualification, rather than upstream tests.
