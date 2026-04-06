<p align="center">
  <img src="icon.png" width="128" height="128" alt="loki">
</p>

# loki

Desktop app QA automation for agents. Launch apps, inspect accessibility trees, click elements, type text, take screenshots — all from the command line.

macOS-first. Built for CI/CD pipelines and agent workflows where you need to verify a desktop app actually works, not just that it compiles.

## Install

```
brew install simonspoon/tap/loki
```

Or download from [Releases](https://github.com/simonspoon/loki/releases).

## Quick start

```bash
# Grant accessibility permission (one-time)
loki check-permission
loki request-permission

# Launch an app and inspect it
loki launch com.apple.Calculator
loki windows --title "Calculator"
loki tree <WINDOW_ID> --depth 3

# Find and click elements
loki find <WINDOW_ID> --role AXButton --title "7"
loki click-element <WINDOW_ID> --title "7"
loki click-element <WINDOW_ID> --title "Add"
loki click-element <WINDOW_ID> --title "3"
loki click-element <WINDOW_ID> --title "Equals"

# Type text and send key combos
loki type "Hello" --window <WINDOW_ID>
loki key cmd+a --window <WINDOW_ID>

# Screenshot and verify
loki screenshot --window <WINDOW_ID> --output result.png
loki wait-for <WINDOW_ID> --role AXButton --title "Equals" --timeout 3000

# Clean up
loki kill com.apple.Calculator
```

## Commands

| Command | Description |
|---------|-------------|
| `launch` | Launch an app by name, bundle ID, or path |
| `kill` | Terminate an app |
| `app-info` | Get info about a running app (by name, bundle ID, or --pid) |
| `windows` | List open windows (filter by title/bundle-id/pid) |
| `tree` | Dump accessibility tree for a window |
| `find` | Find elements by role, title, identifier |
| `click` | Click at screen coordinates (use --pid to target an app) |
| `click-element` | Click a UI element by query |
| `type` | Type text (use --window to target an app) |
| `key` | Send key combo, e.g. `cmd+s`, `ctrl+shift+a` |
| `screenshot` | Capture window (by ID or title) or screen as PNG |
| `wait-for` | Wait for an element to appear |
| `wait-gone` | Wait for an element to disappear |
| `wait-window` | Wait for a window to appear |
| `wait-title` | Wait for window title to match pattern |
| `check-permission` | Check accessibility permission |
| `request-permission` | Prompt for accessibility permission |
| `completions` | Generate shell completions |

## Output

All commands support `--format json` for structured output. Use `LOKI_FORMAT=json` to default to JSON.

Default timeout is 5000ms, override with `--timeout` or `LOKI_TIMEOUT`.

## Requirements

- macOS (uses Accessibility API and Core Graphics)
- Accessibility permission must be granted to the terminal or binary

## License

MIT
