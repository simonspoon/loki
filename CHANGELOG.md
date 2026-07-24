# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-07-23

### Added

- `menu` command: navigate and press an application menu-bar item by path, e.g. `loki menu "File>Open File…"`. The menu bar hangs off the *application* AX element (`AXMenuBar`), not the window tree, so window-scoped `find`/`click-element` can't reach it, and coordinate `click` on an open NSMenu is swallowed by its modal event loop. `menu` walks the app's `AXMenuBar` and fires `AXPress` on the target item — no visual opening required. Path levels split on `>` (override with `--separator`); each level matches exact-first, then case-insensitive substring/glob, ignoring a trailing ellipsis (so `"Save As"` matches `"Save As…"`). Nested submenus add levels (`"Format>Font>Bold"`). Targets the frontmost app unless `--pid`, `--bundle-id`, or `--window` is given; a miss lists the available titles at that level.

## [0.2.2] - 2026-04-13

### Changed

- CLI: `--label` now auto-wraps bare patterns (no glob metacharacters) as substring matches. Previously `--label "Projects"` required an exact literal match; now it matches any text field containing "Projects". Explicit glob metacharacters (`*`, `?`, `[..]`) continue to work unchanged. Users who relied on bare-literal exact-match semantics can switch to `--title` or `--id` for strict field matching.

## [0.2.1] - 2026-04-13

### Added

- `--label` query flag on `find`, `click-element`, `wait-for`, and `wait-gone` commands. Matches elements where any text field (title, value, description, or identifier) glob-matches the pattern. Distinct from `--title`, which remains strict. This enables finding webview (Tauri/wry, Safari) text elements whose content lives in `AXValue` rather than `AXTitle`.

## [0.2.0] - earlier

- Prior releases not tracked in this file.
