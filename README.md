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

No Homebrew? Install the release binary directly:

```
curl -fsSL https://raw.githubusercontent.com/simonspoon/loki/main/install.sh | sh
```

Installs to `/usr/local/bin` (override with `LOKI_INSTALL_DIR`, pin with `LOKI_VERSION`).
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
loki find <WINDOW_ID> --label "Projects"          # Match any text field (great for webviews)
loki click-element <WINDOW_ID> --title "7"
loki click-element <WINDOW_ID> --title "Add"
loki click-element <WINDOW_ID> --title "3"
loki click-element <WINDOW_ID> --title "Equals"

# Drag a divider, resizer, or slider (real OS mouse events — synthetic ones
# never get pointer capture, so weaker input is ignored silently)
loki drag 404 300 244 300 --window <WINDOW_ID>
loki drag 404 300 244 300 --window <WINDOW_ID> --steps 20 --delay 25

# Scroll a pane the keyboard can't reach — a webview `overflow-y: auto`
# container has no tabindex, so it never takes focus and `key pagedown`
# scrolls the document behind it instead, leaving the screenshot unchanged.
# The wheel event carries its own location, so it hits the pane under X,Y.
loki wheel 640 400 0,300 --window <WINDOW_ID>      # positive dY scrolls down
loki wheel 640 400 0,-300 --window <WINDOW_ID>     # negative dY scrolls up
loki wheel 640 400 800,0 --window <WINDOW_ID>      # positive dX scrolls right
loki wheel 640 400 0,500 --window <WINDOW_ID> --steps 8   # for momentum scrollers

# Type text and send key combos
loki type "Hello" --window <WINDOW_ID>
loki key cmd+a --window <WINDOW_ID>

# Drive the app menu bar (opens and presses the item — reaches menus that
# live off the app, not any window, and that swallow coordinate clicks)
loki menu "File>New" --bundle-id com.apple.TextEdit
loki menu "Format>Font>Bold" --pid <PID>
loki menu "Edit>Select All"                       # no target = frontmost app

# Read menu state without pressing anything (checkmark / enabled / submenu).
# The menu bar hangs off the app, so `find <WID>` can never see it.
loki menu-state "View>Theme" --bundle-id com.example.app
loki -f json menu-state "View>Theme" | jq '[.children[] | select(.marked) | .title]'

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
| `windows` | List open windows (filter by title/bundle-id/pid; `--title` is a case-insensitive substring) |
| `tree` | Dump accessibility tree for a window |
| `find` | Find elements by role, title, label, identifier (text matching is case-insensitive; `--id` is exact) |
| `click` | Click at screen coordinates (use --pid to target an app) |
| `click-element` | Click a UI element by query |
| `drag` | Drag between two screen points with real OS mouse events (dividers, resizers, sliders) |
| `wheel` | Scroll at screen coordinates with a real wheel event — reaches panes that can't take focus |
| `type` | Type text (use --window to target an app) |
| `key` | Send key combo, e.g. `cmd+s`, `ctrl+shift+a` |
| `menu` | Open and press an app menu-bar item by path, e.g. `"File>Open File…"` |
| `menu-state` | Read a menu item + its children (checkmark, enabled, submenu) without pressing |
| `screenshot` | Capture window (by ID or title) or screen as PNG |
| `wait-for` | Wait for an element to appear |
| `wait-gone` | Wait for an element to disappear |
| `wait-window` | Wait for a window to appear (same matching as `windows`; a fresh `.app` needs `--timeout 20000`+) |
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
