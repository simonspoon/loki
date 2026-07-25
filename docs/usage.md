# Usage Guide

## Global flags

All commands accept these flags:

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--format` / `-f` | `LOKI_FORMAT` | `text` | Output format: `text` or `json` |
| `--timeout` / `-t` | `LOKI_TIMEOUT` | `5000` | Default timeout in milliseconds |

## Permissions

macOS requires accessibility permission for Loki to inspect and interact with
other apps. Grant it once to your terminal (or the `loki` binary if running
outside a terminal).

```bash
# Check current status
loki check-permission

# Prompt to grant (opens System Settings)
loki request-permission
```

After granting, restart your terminal for the permission to take effect.

## App lifecycle

### Launch

Start an app by name, bundle ID, or file path:

```bash
loki launch Calculator                    # By app name
loki launch com.apple.Calculator          # By bundle ID
loki launch /Applications/Safari.app      # By path
loki launch com.apple.TextEdit --args /tmp/test.txt
```

By default, `launch` waits for the app to finish launching before returning.
Use `--wait false` to return immediately.

### Kill

Terminate an app by bundle ID or name:

```bash
loki kill com.apple.Calculator
loki kill Calculator
loki kill --force com.apple.Calculator   # SIGKILL
```

### App info

Get info about a running app:

```bash
loki app-info Calculator                  # By app name
loki app-info com.apple.Calculator        # By bundle ID
loki app-info --pid 12345                 # By process ID
loki app-info --bundle-id com.apple.Calculator
```

Returns PID, bundle ID, name, and whether the app is active.

## Window discovery

List open windows, optionally filtered:

```bash
loki windows                              # Named windows only
loki windows --all                        # Include untitled windows
loki windows --title "Calculator"         # By title (substring)
loki windows --bundle-id com.apple.Safari # By bundle ID
loki windows --pid 12345                  # By process ID
```

By default, windows with empty titles (system-level helper windows) are hidden.
Use `--all` to include them.

### Window title matching

`--title` matches as a **substring**, the same way `--label` does for elements:
`--title ash-md` finds a window titled `ash-md — README.md`. A pattern carrying
glob metacharacters is used verbatim, so you can anchor it:

```bash
loki windows --title "ash-md"      # substring   → matches "ash-md — README.md"
loki windows --title "ash-md*"     # prefix      → matches "ash-md — README.md"
loki windows --title "*README.md"  # suffix      → does not match "…README.md.bak"
loki windows --title "ash-m[d]"    # whole title → matches only "ash-md"
```

Matching is **case-sensitive** (`ash-md` will not find `ASH-MD`). Every command
that takes a *window* title uses this identical matching — `windows`,
`wait-window`, `wait-title`, and `screenshot --window <title>` — so if one finds
a window, so do the others. Element queries (`find --title`, `click-element
--title`, `wait-for --title`) are a separate, strict field match; use `--label`
there for substring behaviour.

Each window has a numeric `window_id` used by other commands.

## Accessibility tree

### Dump tree

Inspect the UI element hierarchy of a window:

```bash
loki tree <WINDOW_ID>                     # Full tree
loki tree <WINDOW_ID> --depth 3           # Limit depth
loki tree <WINDOW_ID> --flat              # Flat list instead of tree
```

### Find elements

Search for specific elements:

```bash
loki find <WINDOW_ID> --role AXButton
loki find <WINDOW_ID> --title "Save"
loki find <WINDOW_ID> --role AXTextField --id "username"
loki find <WINDOW_ID> --role AXButton --title "OK" --index 0
loki find <WINDOW_ID> --label "Projects"          # Match webview text by any text field
```

Filters:
- `--role` matches the accessibility role (AXButton, AXTextField, etc.)
- `--title` matches the element title/label (strict — title only)
- `--label` matches any text field (title, value, description, identifier).
  Use this to find webview text elements (Tauri/wry, Safari) whose content
  lives in `AXValue` rather than `AXTitle`. Bare patterns are auto-wrapped as
  substring globs (`"Projects"` becomes `*Projects*`); patterns containing
  `*`, `?`, or `[` are used verbatim.
- `--id` matches the accessibility identifier
- `--index` selects the Nth match (0-based)

## Input

### Click at coordinates

```bash
loki click 100 200                        # Left click
loki click 100 200 --double               # Double click
loki click 100 200 --right                # Right click
loki click 100 200 --pid 12345            # Activate app first, then click
loki click 100 200 --window <WINDOW_ID>   # Activate app by window, then click
```

Use `--pid` or `--window` to ensure the target app is frontmost before clicking.
Without these flags, the click goes to whatever window is at those coordinates.

### Click a UI element

Click the center of a matched element:

```bash
loki click-element <WINDOW_ID> --title "Save"
loki click-element <WINDOW_ID> --role AXButton --title "OK"
loki click-element <WINDOW_ID> --id "submit-button"
loki click-element <WINDOW_ID> --label "Submit"   # Match any text field (webview-friendly)
```

The `--label` flag (also supported by `find`, `wait-for`, `wait-gone`) matches
against title, value, description, and identifier. See [Find elements](#find-elements)
for matching rules.

**Which match gets clicked.** Unlike `find`, which lists every match in tree
order, `click-element` picks the one element a click should land on:

- An **actionable** role — `AXButton`, `AXMenuItem`, `AXMenuBarItem`,
  `AXMenuButton`, `AXPopUpButton`, `AXCheckBox`, `AXRadioButton`,
  `AXDisclosureTriangle`, `AXLink`, `AXTextField`, `AXTextArea`, `AXComboBox` —
  beats anything else, whatever the tree order. In a save panel `--label Save`
  matches both the "Save As:" caption and the Save button; the button wins.
- **Several actionable matches** → the command refuses, exits 1, and lists the
  candidates instead of clicking one at random. Narrow with `--role`, `--title`
  or `--id`, or pin `--label` with a glob:

  ```
  $ loki click-element 1573 --label Button
  error: ambiguous match: 2 clickable elements match in window 1573 — narrow
  with --role, --title or --id, or pin --label with a glob:
  AXButton "Cancel" id=CancelButton (76x26 at 790,449) [17.0.9]
  AXButton "Save" id=OKButton (76x26 at 872,449) [17.0.10]
  ```

- **No actionable match** → the first match in tree order is clicked, as before.
  Webview content (Tauri/wry, Safari) is entirely `AXStaticText`, and that is the
  case `--label` exists for.

### Scroll (wheel)

```bash
loki wheel 640 400 0,300                  # Scroll down 300px at those screen coords
loki wheel 640 400 0,-300                 # Negative dY scrolls up
loki wheel 640 400 800,0                  # Positive dX scrolls right
loki wheel 640 400 0,500 --window <WINDOW_ID>       # Activate the app first
loki wheel 640 400 0,500 --steps 8 --delay 25       # Split across 8 wheel events
```

`X` and `Y` are absolute screen coordinates, as with `click` and `drag`;
`--window`/`--pid` only activate the target app, they do not change the
coordinate space.

The delta is a `dX,dY` pair in pixels — the same shape as `khora wheel`, and the
same sign convention as the DOM's `WheelEvent`: **positive `dY` scrolls down,
positive `dX` scrolls right**. (Core Graphics' own wheel axes run the other way;
loki flips them for you.) The pair is required: a bare `300` has no honest
reading, since horizontal and vertical are equally plausible.

Use this rather than `key pagedown` whenever the thing you need to scroll is a
webview container with `overflow-y: auto` and no `tabindex`. Such a pane can
never take focus, so the key is delivered to the document behind it, the pane
does not move, and the screenshot comes back identical — which reads as an app
bug rather than a driving mistake. A wheel event carries its own location, so it
reaches whatever pane sits under `X,Y` regardless of focus.

Raise `--steps` for apps whose momentum or smooth scrolling clamps a single
large jump; the delta is split into whole-pixel increments that still sum to
exactly what you asked for. As with `drag`, a raw wheel event does not activate
the target app, and an inactive app can swallow the scroll without erroring —
so pass `--window` or `--pid`.

### Type text

```bash
loki type "Hello, world"                  # Types into focused app
loki type "Hello" --window <WINDOW_ID>    # Targets specific window's app
loki type "Hello" --pid 12345             # Targets specific process
```

Uses macOS System Events for reliable cross-process typing.

### Key combos

```bash
loki key cmd+s                            # Cmd+S
loki key cmd+shift+a                      # Cmd+Shift+A
loki key ctrl+c                           # Ctrl+C
loki key return                           # Enter
loki key cmd+s --window <WINDOW_ID>       # Target specific window's app
```

Modifier names: `cmd`, `shift`, `ctrl`, `alt`/`option`.

## Menu bar

An app's menu bar hangs off the *application* element, not any window, so
`tree`/`find <WINDOW_ID>` can never see it — and a coordinate `click` on an open
menu is swallowed by its modal event loop. These two commands are the only way
in. Both target the frontmost app unless `--pid`, `--bundle-id`, or `--window`
is given, and both split the path on `>` (override with `--separator`).

Path levels match exact-first, then case-insensitively / by substring, ignoring
a trailing ellipsis — so `"Save As"` finds `"Save As…"`. An unmatched level
errors with the available titles at that level.

### Press an item

```bash
loki menu "File>New" --bundle-id com.apple.TextEdit
loki menu "Format>Font>Bold" --pid <PID>
loki menu "Edit>Select All"                # frontmost app
loki menu "File/New" --separator /
```

### Read item state

`menu-state` observes without invoking anything — it prints the item the path
names plus its immediate children, each with its checkmark, enabled state, and
whether it opens a submenu. Separators are omitted.

```bash
loki menu-state "View>Theme"
```

```
Theme (submenu)
  ✓ Light
    Dark
    System (disabled)
```

JSON adds the raw `mark` character, so a radio-style bullet (`•`) stays
distinguishable from a checkmark (`✓`):

```bash
loki -f json menu-state "View>Theme"
```

```json
{
  "title": "Theme",
  "marked": false,
  "enabled": true,
  "has_submenu": true,
  "children": [
    { "title": "Light", "marked": true, "mark": "✓", "enabled": true, "has_submenu": false },
    { "title": "Dark", "marked": false, "enabled": true, "has_submenu": false }
  ]
}
```

Reading a leaf is safe — `menu-state "File>New"` reports the item and creates
no document. Only a container that owns a submenu is ever opened (to populate a
lazily-built one), and it is closed again before the command returns.

## Screenshots

```bash
loki screenshot --window <WINDOW_ID>      # Capture by window ID
loki screenshot --window "Calculator"     # Capture by window title
loki screenshot --screen                  # Capture full screen
loki screenshot --output result.png       # Custom output path
```

The `--window` flag accepts either a numeric window ID or a window title string.

Default output: `loki-screenshot.png` in the current directory.

## Wait commands

All wait commands poll until the condition is met or the timeout expires.
Timeout defaults to the global `--timeout` value (5000ms) but can be overridden
per-command.

### Wait for element

Wait for a UI element to appear:

```bash
loki wait-for <WINDOW_ID> --role AXButton --title "Done"
loki wait-for <WINDOW_ID> --title "Loading..." --timeout 10000
loki wait-for <WINDOW_ID> --label "Ready" --timeout 10000  # Webview-friendly
```

### Wait for element to disappear

```bash
loki wait-gone <WINDOW_ID> --title "Loading..."
loki wait-gone <WINDOW_ID> --role AXProgressIndicator --timeout 15000
loki wait-gone <WINDOW_ID> --label "Spinner"
```

### Wait for window

Wait for a window to appear:

```bash
loki wait-window --title "Document"
loki wait-window --bundle-id com.apple.TextEdit --timeout 10000
```

Matching is identical to `windows --title` (substring, case-sensitive), so a
timeout here is almost always **launch latency, not a bad pattern**. A freshly
built or newly copied `.app` can take 15s+ to open its first window — macOS
scans a new binary on first launch — so give the first `wait-window` after a
rebuild `--timeout 20000` or more. The timeout error reports the glob it
actually matched, how many windows it saw, and any case-insensitive near-miss:

```
error: timed out after 800ms waiting for title glob "*ASH-MD*"
  seen: 203 windows (62 titled)
  near-miss (title matching is case-sensitive): "ash-md"
  hint: a freshly built or newly copied .app can take 15s+ to open its first window …
```

### Wait for title change

Wait for a window's title to match a pattern:

```bash
loki wait-title <WINDOW_ID> "Saved"
loki wait-title <WINDOW_ID> "*.txt" --timeout 5000
```

## Shell completions

Generate completions for your shell:

```bash
loki completions bash > ~/.bash_completion.d/loki
loki completions zsh > ~/.zfunc/_loki
loki completions fish > ~/.config/fish/completions/loki.fish
```

## JSON output

All commands support `--format json`. Set the env var to make it the default:

```bash
export LOKI_FORMAT=json
loki windows --title "Calculator"
```

Example JSON output from `windows`:

```json
[
  {
    "window_id": 1234,
    "title": "Calculator",
    "pid": 5678,
    "bundle_id": "com.apple.Calculator",
    "bounds": { "x": 100, "y": 200, "width": 300, "height": 400 }
  }
]
```

## Scripting patterns

### Wait-then-act

```bash
loki launch com.apple.TextEdit
loki wait-window --bundle-id com.apple.TextEdit
WINDOW=$(loki windows --bundle-id com.apple.TextEdit -f json | jq -r '.[0].window_id')
loki type "Hello" --window "$WINDOW"
loki key cmd+s --window "$WINDOW"
loki screenshot --window "$WINDOW" --output after-save.png
```

### Verify UI state

```bash
loki wait-for "$WINDOW" --role AXButton --title "Submit"
ELEMENTS=$(loki find "$WINDOW" --role AXStaticText --title "Success" -f json)
if [ "$(echo "$ELEMENTS" | jq length)" -gt 0 ]; then
  echo "PASS: Success message visible"
else
  echo "FAIL: Success message not found"
fi
```

### Verify a radio-style menu group

Assert that exactly one item in a submenu is checked, and that it's the right
one — the menu bar is invisible to `find`, so `menu-state` is the only source:

```bash
MARKED=$(loki -f json menu-state "View>Theme" --bundle-id com.example.app \
  | jq -r '[.children[] | select(.marked) | .title] | join(",")')
if [ "$MARKED" = "Dark" ]; then
  echo "PASS: Dark is the only checked theme"
else
  echo "FAIL: expected exactly 'Dark', got '$MARKED'"
fi
```
