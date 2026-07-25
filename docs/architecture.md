# Architecture

## Crate structure

Loki is a Rust workspace with three crates:

```
crates/
  loki-core/    Platform-agnostic types, traits, and output formatting
  loki-macos/   macOS implementation using Accessibility API + Core Graphics
  loki-cli/     CLI binary (clap) — wires commands to the driver
```

Dependencies flow one way: `loki-cli -> loki-macos -> loki-core`.

### loki-core

Defines the `DesktopDriver` trait — the platform abstraction. All automation
operations (window listing, accessibility tree, input, screenshots) are methods
on this trait. Platform backends implement it.

Key modules:

| Module | Purpose |
|--------|---------|
| `driver.rs` | `DesktopDriver` trait with async methods |
| `element.rs` | `AXElement`, `WindowInfo`, `WindowRef`, `AppInfo` types |
| `query.rs` | `ElementQuery` and `WindowFilter` for searching |
| `error.rs` | `LokiError` enum with exit codes |
| `output.rs` | `OutputFormat` (text/json) and formatting functions |
| `config.rs` | `LokiConfig` for runtime configuration |

### loki-macos

Implements `DesktopDriver` via `MacOSDriver`. Uses:

- **Accessibility API** (AXUIElement) for tree inspection, element querying, and element-based clicks
- **Core Graphics** (CGEvent) for coordinate-based clicks and screenshots
- **ApplicationServices** for app launch/kill
- **System Events** (via AppleScript/osascript) for keyboard input — this gives reliable cross-process typing without requiring the binary to be trusted for key events

Modules map to capability areas:

| Module | Purpose |
|--------|---------|
| `driver.rs` | `MacOSDriver` struct, implements `DesktopDriver` |
| `window.rs` | Window listing via CGWindowListCopyWindowInfo |
| `accessibility.rs` | AXUIElement tree walking and element queries |
| `app.rs` | NSWorkspace-based app launch, kill, info |
| `input.rs` | CGEvent clicks + osascript keyboard input |
| `screenshot.rs` | CGWindowListCreateImage screenshot capture |
| `permission.rs` | AXIsProcessTrusted checks and prompts |

### loki-cli

Thin CLI layer. Parses commands with clap, creates a `MacOSDriver`, dispatches
to the appropriate trait method, and formats output. No business logic lives here.

## Platform abstraction

The `DesktopDriver` trait is the extension point. To add Linux support, you would:

1. Create `crates/loki-linux/` implementing `DesktopDriver` (likely via AT-SPI2 + XDG)
2. Add a feature flag or compile-time cfg to `loki-cli` to select the backend
3. Core types and output formatting remain shared

This is not yet implemented — current releases are macOS-only.

## Async design

All driver methods are async (via `async-trait`). The macOS implementation uses
`tokio::task::spawn_blocking` for FFI calls that block, keeping the runtime
responsive. The CLI uses `#[tokio::main]` with the multi-thread runtime.

## Error handling

`LokiError` variants map to exit codes (1-6) for scripting. All errors flow
through `Result<T, LokiError>` and are printed to stderr. The CLI returns
`ExitCode` based on the error variant.

## Testing

Tests live at two levels:

**Unit tests** — inline `#[cfg(test)]` modules in the crate source files. These
cover core types and logic without requiring accessibility permission or a
running app:

- `loki-core`: `element.rs`, `config.rs`, `error.rs`, `output.rs`, `query.rs`
- `loki-macos`: `input.rs`, `app.rs`

Run with `cargo test` (the default test run skips ignored tests).

**CLI integration tests** — `crates/loki-cli/tests/cli.rs`, using `assert_cmd`
and `predicates`. These invoke the compiled `loki` binary and check exit codes,
stdout, and stderr. Tests that do not need accessibility permission (help output,
argument validation, `check-permission`, `windows`, `completions`) run normally.
Tests that require accessibility permission or a running app are marked
`#[ignore]` and can be run explicitly with `cargo test -- --ignored`.

## Output formatting

Every command supports `--format text` (default, human-readable) and
`--format json` (structured, for piping). The `LOKI_FORMAT` env var sets the
default. Formatting functions live in `loki-core::output` so they are shared
across any future frontend.

### Pitfall: byte-slicing UTF-8 strings

`output::truncate` must not slice a `&str` at an arbitrary byte index. Rust
panics on `&s[..n]` if `n` falls inside a multi-byte codepoint (em dashes,
emoji, CJK). Walk back to the nearest char boundary with
`s.is_char_boundary(i)` before slicing. Any helper that trims user-supplied
text — window titles, AX values — must do the same. See tests in
`loki-core/src/output.rs` for the regression cases.

### Querying: `--label` vs `--title`

`ElementQuery` has separate `title` and `label` fields with intentionally
different match semantics:

- `title` matches `AXTitle`, `AXDescription`, or `AXIdentifier` — the fields
  that carry a human-readable *label*, and no others (strict: notably **not**
  `AXValue`)
- `label` matches those three plus `AXValue` (broad — needed for webview text
  in Tauri/wry and Safari, where text lives in `AXValue` rather than `AXTitle`)

The CLI auto-wraps bare `--label` patterns as substring globs (`Projects` →
`*Projects*`) but leaves patterns containing `*`, `?`, or `[` untouched. Keep
the `title` branch strict — broadening it to `AXValue` would silently change
long-standing match behavior for existing scripts.

### Querying: case (mesa 540)

`glob_matches` runs `Pattern::matches_with(.., TEXT_MATCH)` with
`case_sensitive: false`, so **every text query folds case** — `--title`,
`--label`, `--value`, `--description`, and window `--title`. Roles
(`role_matches`) and `menu` path levels always did; this made the rest agree.

The decision was between that and documenting case-sensitivity as intentional.
Case-insensitive won because a case-only miss is *silent*: `find` returns "No
elements found" at **exit 0**, which is indistinguishable from the element
genuinely being absent, so it reads as an app bug rather than a typo. It had
already produced one wrong bug report (mesa 530).

Three things to preserve when touching this:

- **`identifier` stays an exact, case-sensitive `!=` compare** and is never
  globbed. It is the escape hatch for telling apart two elements that differ
  only by case; don't route it through `glob_matches`.
- **The invalid-pattern fallback must fold case too**, ASCII-only
  (`to_ascii_lowercase`, matching the glob crate's `chars_eq`) — not
  `to_lowercase`. If the two paths disagree, the same query means different
  things depending on whether its pattern happened to parse as a glob.
- **`TEXT_MATCH`'s other two fields stay `false`**, matching what
  `Pattern::matches` uses. They are path-oriented (`require_literal_separator`,
  `require_literal_leading_dot`) and AX titles are not paths. Note
  `MatchOptions::default()` is *not* `MatchOptions::new()` — the derived default
  has `case_sensitive: false`, the constructor has `true`.

Two accepted costs, both from the `glob` crate: an *alphabetic* character range
matches both cases (`[a-z]` also matches `Q`; `[0-9]` and symbol ranges are
unaffected, guarded in `in_char_specifiers`), and folding is ASCII-only, so
`café` still will not match `CAFÉ`.

### Pitfall: never `AXPress` a menu item to probe it

`menu` and `menu-state` share one walk (`accessibility::resolve_menu_path`).
Some apps build a submenu's children only when it is opened, so the walk presses
a container that reads empty and re-reads it. That press is safe on a *submenu*
and catastrophic on a *leaf* — `AXPress` on `File>New` runs the command, so a
read-only `menu-state` would create a document as a side effect of describing
one.

The guard is `owns_submenu()`: only an item with an `AXMenu` child is ever
pressed to populate. A leaf that reads empty stays empty. `menu_state_path` also
fires `AXCancel` on anything it had to open, so observing a menu never leaves one
hanging open to swallow the next command's input.

Note the asymmetry: `press_menu_path` needs no such cleanup, because pressing
the leaf dismisses the whole chain on its way out.
