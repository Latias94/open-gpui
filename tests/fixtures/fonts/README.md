# Font Fixtures

The unmodified Lilex and IBM Plex fonts and their licenses come from Zed revision
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012/assets/fonts).
The tests embed these checked-in files and do not require `repo-ref`, network access, or OS font
installation. All fonts use the SIL Open Font License 1.1. Original notices are preserved in the
font metadata and accompanying license files.
The license blobs retain their upstream content, including trailing spaces in the Lilex and
IBM Plex notices. The local Git whitespace attribute preserves those notices verbatim.

| Fixture | Font family | Original Zed path | Git blob |
| --- | --- | --- | --- |
| `Lilex-Regular.ttf` | `Lilex` | `assets/fonts/lilex/Lilex-Regular.ttf` | `cb98a69b0ffb1976805bf5af3047307fe2422b1d` |
| `Lilex-Bold.ttf` | `Lilex` | `assets/fonts/lilex/Lilex-Bold.ttf` | `45930ee30b81a78e964c3ff494e05ea45147ed9b` |
| `Lilex-OFL.txt` | Lilex license | `assets/fonts/lilex/OFL.txt` | `156240bc907aa14571afb18b4b1bde62d3ea33e0` |
| `IBMPlexSans-Regular.ttf` | `IBM Plex Sans` | `assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf` | `bd6817d5202895da5ac4fad88de3da71e652881a` |
| `IBMPlexSans-OFL.txt` | IBM Plex license | `assets/fonts/ibm-plex-sans/license.txt` | `f72f76504cd73cf2c00b140b2743c138320597b6` |

The unmodified Noto Emoji font and its accompanying license come from egui revision
[`fc9d66dda742d722eedaa8684d23f86369cf8e5f`](https://github.com/emilk/egui/tree/fc9d66dda742d722eedaa8684d23f86369cf8e5f/crates/epaint_default_fonts/fonts).
The font's internal metadata preserves `Copyright 2013 Google Inc. All Rights Reserved.` and
declares the SIL Open Font License 1.1. Its family name is `Noto Emoji`; `NotoEmoji` is its
PostScript name. This fixture supports tests for retaining fallback faces without a Latin `m`.

| Fixture | Original egui path | Git blob |
| --- | --- | --- |
| `NotoEmoji-Regular.ttf` | `crates/epaint_default_fonts/fonts/NotoEmoji-Regular.ttf` | `19b7badf4afe64dae28cd64fc506b95291bb8b03` |
| `NotoEmoji-OFL.txt` | `crates/epaint_default_fonts/fonts/OFL.txt` | `d952d62c065f3f35fb83a173496e90b21525aef3` |

SHA-256:

- `Lilex-Regular.ttf`: `8a467ad5a1941dc7c9cebaee34e3ed9364a73b48393034e363e4b528bd711d04`
- `Lilex-Bold.ttf`: `3369214e2ecddb45ae5024b11d45dd637e99e9d2127ab729817d41d72b8a9c68`
- `IBMPlexSans-Regular.ttf`: `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5`
- `NotoEmoji-Regular.ttf`: `415dc6290378574135b64c808dc640c1df7531973290c4970c51fdeb849cb0c5`
- `NotoEmoji-OFL.txt`: `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2`

The Windows fallback-to-direct installation test requires that `Lilex` is absent from the OS font
collection when the platform starts. It fails with an explicit prerequisite message if the family
is already installed. The test installs font bytes only in its DirectWrite text system and uses
the existing public application and window APIs; it does not modify the OS font collection.
The additional-weight trajectory installs only Regular before requesting Bold, then installs
Bold through the same public TextSystem. The native tests distinguish their real `m` outlines:
Regular has design bounds `(54, 0, 492, 528)` and Bold `(30, 0, 540, 528)` at 1,000 units/em;
both have a 600-unit advance. Equal advance alone cannot prove that the new face was selected.
DirectWrite initially synthesizes the requested Bold from Regular; the backend regression checks
that installation replaces the simulation with the actual Bold face while preserving old glyphs.
