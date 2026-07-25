# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Every command taking a *window* title — `windows`, `wait-window`, `wait-title`, `screenshot --window <title>` — now auto-wraps bare patterns as substring matches, the same treatment `--label` got in 0.2.2: `--title ash-md` matches a window titled `ash-md — README.md`. Explicit glob metacharacters (`*`, `?`, `[..]`) still pass through verbatim, so `--title "ash-m[d]"` pins the whole title. Element `--title` (`find`, `click-element`, `wait-for`, `wait-gone`) is unchanged and still strict.

### Fixed

- `click-element` no longer clicks the first match in tree order. A query that matches both a caption and the control it labels now lands on the control: driving a save panel, `--label Save` matched the "Save As:" `AXStaticText` (`id=nameFieldLabel`) *before* the Save `AXButton`, clicked the caption, and exited 0 — a silent no-op indistinguishable from a successful save. Actionable roles (`AXButton`, `AXMenuItem`, `AXMenuBarItem`, `AXMenuButton`, `AXPopUpButton`, `AXCheckBox`, `AXRadioButton`, `AXDisclosureTriangle`, `AXLink`, `AXTextField`, `AXTextArea`, `AXComboBox`) now outrank everything else. When *several* actionable elements match there is no safe guess, so the command refuses with exit 1 and lists the candidates in `find` format rather than clicking one of them. When nothing actionable matches, first-match order still stands — a webview's text content (Tauri/wry, Safari) is all `AXStaticText`, which is the case `--label` exists for.

- `wait-window` timeouts now say what they were waiting for instead of a bare `timed out after 10000ms`: the glob actually matched, how many windows were visible, how many belong to a `--bundle-id`, any case-insensitive near-miss, and the usual cause — a freshly built `.app` can take 15s+ to open its first window because macOS scans a new binary on first launch. Exit code is still 3. The bare message had made slow launches look like a matching bug between `wait-window` and `windows`, which share one code path and have never differed.

### Added

- `wheel` command: scroll at screen coordinates with a real OS-level wheel event, e.g. `loki wheel 640 400 0,300`. `key pagedown` cannot stand in for a webview pane with `overflow-y: auto` and no `tabindex`: the pane can never take focus, so the key scrolls the document behind it, the pane doesn't move, and the identical screenshot reads as an app bug. A wheel event carries its own location, so it reaches whatever pane sits under `X,Y`. Coordinates are absolute screen coordinates like `click`/`drag` — `--window`/`--pid` activate the target app and do not change the coordinate space. The delta is a `dX,dY` pixel pair matching `khora wheel`, with the DOM's `WheelEvent` signs (positive `dY` scrolls down, positive `dX` right); the pair is required, since a bare number is ambiguous between axes. `--steps` splits the delta across several events for momentum/smooth scrollers, in whole-pixel increments that still sum to exactly the requested delta.

- `menu` command: navigate and press an application menu-bar item by path, e.g. `loki menu "File>Open File…"`. The menu bar hangs off the *application* AX element (`AXMenuBar`), not the window tree, so window-scoped `find`/`click-element` can't reach it, and coordinate `click` on an open NSMenu is swallowed by its modal event loop. `menu` walks the app's `AXMenuBar` and fires `AXPress` on the target item — no visual opening required. Path levels split on `>` (override with `--separator`); each level matches exact-first, then case-insensitive substring/glob, ignoring a trailing ellipsis (so `"Save As"` matches `"Save As…"`). Nested submenus add levels (`"Format>Font>Bold"`). Targets the frontmost app unless `--pid`, `--bundle-id`, or `--window` is given; a miss lists the available titles at that level.

## [0.2.2] - 2026-04-13

### Changed

- CLI: `--label` now auto-wraps bare patterns (no glob metacharacters) as substring matches. Previously `--label "Projects"` required an exact literal match; now it matches any text field containing "Projects". Explicit glob metacharacters (`*`, `?`, `[..]`) continue to work unchanged. Users who relied on bare-literal exact-match semantics can switch to `--title` or `--id` for strict field matching.

## [0.2.1] - 2026-04-13

### Added

- `--label` query flag on `find`, `click-element`, `wait-for`, and `wait-gone` commands. Matches elements where any text field (title, value, description, or identifier) glob-matches the pattern. Distinct from `--title`, which remains strict. This enables finding webview (Tauri/wry, Safari) text elements whose content lives in `AXValue` rather than `AXTitle`.

## [0.2.0] - earlier

- Prior releases not tracked in this file.
