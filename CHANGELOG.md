# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-06-07

### Added

- Root-level fork attribution and licensing notes, plus per-crate `NOTICE` files that preserve
  upstream copyright notices.

- A publish-check workflow that validates leaf crate packaging first and package contents for the
  rest of the workspace.

### Changed

- Public package names and Rust import paths are standardized around the `open-gpui` /
  `open_gpui::...` branding.
- Workspace metadata is aligned to the fork author and unified version line for the first release.
- Fork dependencies now resolve from crates.io via `open-gpui-scap` and `open-gpui-font-kit`, and publishable Open GPUI crates no longer inherit the workspace root's `publish = false` guard.
