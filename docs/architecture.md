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

This is not yet implemented — v0.1.0 is macOS-only.

## Async design

All driver methods are async (via `async-trait`). The macOS implementation uses
`tokio::task::spawn_blocking` for FFI calls that block, keeping the runtime
responsive. The CLI uses `#[tokio::main]` with the multi-thread runtime.

## Error handling

`LokiError` variants map to exit codes (1-6) for scripting. All errors flow
through `Result<T, LokiError>` and are printed to stderr. The CLI returns
`ExitCode` based on the error variant.

## Output formatting

Every command supports `--format text` (default, human-readable) and
`--format json` (structured, for piping). The `LOKI_FORMAT` env var sets the
default. Formatting functions live in `loki-core::output` so they are shared
across any future frontend.
