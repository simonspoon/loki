# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-04-13

### Added

- `--label` query flag on `find`, `click-element`, `wait-for`, and `wait-gone` commands. Matches elements where any text field (title, value, description, or identifier) glob-matches the pattern. Distinct from `--title`, which remains strict. This enables finding webview (Tauri/wry, Safari) text elements whose content lives in `AXValue` rather than `AXTitle`.

## [0.2.0] - earlier

- Prior releases not tracked in this file.
