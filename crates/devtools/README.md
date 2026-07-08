# open-gpui-devtools

Read-only inspection snapshots and local devtools surfaces for Open GPUI applications.

This crate owns the devtools probe and snapshot vocabulary. Default builds stay renderer-neutral and
do not depend on GPUI. Optional features connect specialized panels and GPUI UI surfaces later:

- `form` for `open-gpui-form` snapshots.
- `resource` for `open-gpui-resource` snapshots.
- `docking` for docking snapshots.
- `motion` for motion snapshots.
- `gpui` for native inspector UI elements.

The first contract is read-only. Devtools can collect, filter, copy, and export snapshots; runtime
mutation and live property editing are intentionally out of scope for the initial surface.
