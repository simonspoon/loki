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
loki windows --title "Calculator"         # By title glob
loki windows --bundle-id com.apple.Safari # By bundle ID
loki windows --pid 12345                  # By process ID
```

By default, windows with empty titles (system-level helper windows) are hidden.
Use `--all` to include them.

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
```

Filters:
- `--role` matches the accessibility role (AXButton, AXTextField, etc.)
- `--title` matches the element title/label
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
```

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
```

### Wait for element to disappear

```bash
loki wait-gone <WINDOW_ID> --title "Loading..."
loki wait-gone <WINDOW_ID> --role AXProgressIndicator --timeout 15000
```

### Wait for window

Wait for a window to appear:

```bash
loki wait-window --title "Document"
loki wait-window --bundle-id com.apple.TextEdit --timeout 10000
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
