use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{CommandFactory, Parser, Subcommand};
use loki_core::{DesktopDriver, ElementQuery, OutputFormat, WindowFilter, WindowRef};
use loki_macos::MacOSDriver;
use std::path::PathBuf;
use std::process::ExitCode;

/// Wrap a bare `--label` pattern with substring globs so that `"Projects"` matches
/// any text field containing "Projects". Window `--title` filters get the same
/// treatment inside `WindowFilter::matches_title`.
fn auto_wrap_label(s: &str) -> String {
    loki_core::query::auto_wrap_pattern(s)
}

#[derive(Parser)]
#[command(
    name = "loki",
    about = "Desktop app automation for QA testing",
    version
)]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "text",
        global = true,
        env = "LOKI_FORMAT"
    )]
    format: OutputFormat,

    #[arg(
        short,
        long,
        default_value = "5000",
        global = true,
        env = "LOKI_TIMEOUT"
    )]
    timeout: u64,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List open windows
    Windows {
        #[arg(long)]
        bundle_id: Option<String>,
        #[arg(long)]
        pid: Option<u32>,
        /// Match the window title. Case-insensitive; supports glob metacharacters (*, ?, [..]) — without them, matches as substring.
        #[arg(long)]
        title: Option<String>,
        /// Include windows with empty titles
        #[arg(long)]
        all: bool,
        /// Exit 1 when no window matches, instead of exiting 0 with an empty
        /// list. Without it a mistyped --title is byte-identical to the window
        /// genuinely not being open, and the usual
        /// `WID=$(loki -f json windows --title X | jq -r '.[0].window_id')`
        /// resolves the string "null", so the *next* command fails on an
        /// unrelated parse error. Off by default: a script that legitimately
        /// polls for absence (`windows … | jq length` == 0) must keep exiting 0.
        #[arg(long)]
        require_match: bool,
    },

    /// Check if accessibility permission is granted
    CheckPermission,

    /// Request accessibility permission (opens system prompt)
    RequestPermission,

    /// Launch an application
    Launch {
        target: String,
        #[arg(long)]
        args: Vec<String>,
        #[arg(long, default_value = "true")]
        wait: bool,
    },

    /// Kill an application
    Kill {
        target: String,
        #[arg(long)]
        force: bool,
    },

    /// Get info about a running application
    AppInfo {
        /// Target app (bundle ID, path, PID, or name)
        target: Option<String>,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID
        #[arg(long)]
        bundle_id: Option<String>,
    },

    /// Capture a screenshot
    Screenshot {
        /// Window ID (numeric) or window title (string)
        #[arg(long)]
        window: Option<String>,
        #[arg(long)]
        screen: bool,
        #[arg(long, short)]
        output: Option<String>,
    },

    /// Dump the accessibility tree for a window
    Tree {
        window_id: u32,
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long)]
        flat: bool,
    },

    /// Find elements in a window's accessibility tree
    Find {
        window_id: u32,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Match element where any text field (title, value, description, identifier) contains the pattern. Case-insensitive; supports glob metacharacters (*, ?, [..]) — without them, matches as substring.
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        index: Option<usize>,
        /// Exit 1 when the query matches nothing, instead of exiting 0 with an
        /// empty result. Without it a mistyped query is byte-identical to the
        /// element genuinely being absent, so a typo reads as an app bug.
        /// Off by default: a script that legitimately asserts absence
        /// (`find … | jq length` == 0) must keep exiting 0.
        #[arg(long)]
        require_match: bool,
    },

    /// Click at screen coordinates
    // Screens left of / above the primary display have negative origins, so a
    // coordinate can legitimately start with '-'; without this clap reads it as a flag.
    #[command(allow_negative_numbers = true)]
    Click {
        x: f64,
        y: f64,
        #[arg(long)]
        double: bool,
        #[arg(long)]
        right: bool,
        /// Target process ID (activates app before clicking)
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID, e.g. com.apple.TextEdit (activates app before clicking)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (activates app before clicking)
        #[arg(long)]
        window: Option<u32>,
        /// Read X and Y as offsets from the --window/--pid/--bundle-id target's
        /// frame origin instead of absolute screen coordinates
        #[arg(long)]
        relative: bool,
    },

    /// Click a UI element by query
    ClickElement {
        window_id: u32,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Match element where any text field (title, value, description, identifier) contains the pattern. Case-insensitive; supports glob metacharacters (*, ?, [..]) — without them, matches as substring.
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },

    /// Drag from one screen point to another, e.g. `drag 404 300 244 300`
    ///
    /// Posts a real press → move → release mouse gesture at the OS event tap —
    /// the only input a divider, resizer, or slider accepts, since weaker
    /// synthetic events never get `setPointerCapture` and are ignored silently.
    /// Pass --pid, --bundle-id or --window: a raw mouse event does NOT activate
    /// the target app, and an inactive app swallows the whole drag without erroring.
    /// Note a resizer's grab strip often sits a few pixels beside the visible
    /// boundary line; aim at the hit target, not at what you can see.
    #[command(allow_negative_numbers = true)]
    Drag {
        /// Start X in absolute screen coordinates
        x1: f64,
        /// Start Y in absolute screen coordinates
        y1: f64,
        /// End X in absolute screen coordinates
        x2: f64,
        /// End Y in absolute screen coordinates
        y2: f64,
        /// Number of intermediate move events along the path. Very low counts are
        /// unreliable — a single-jump drag is intermittently ignored; raise, don't lower
        #[arg(long, default_value_t = loki_macos::input::DEFAULT_DRAG_STEPS)]
        steps: usize,
        /// Pause between mouse events in milliseconds (lets the app re-render mid-drag)
        #[arg(long, default_value_t = loki_macos::input::DEFAULT_DRAG_DELAY_MS)]
        delay: u64,
        /// Target process ID (activates app before dragging)
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID, e.g. com.apple.TextEdit (activates app before dragging)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (activates app before dragging)
        #[arg(long)]
        window: Option<u32>,
        /// Read both endpoints as offsets from the --window/--pid/--bundle-id
        /// target's frame origin instead of absolute screen coordinates
        #[arg(long)]
        relative: bool,
    },

    /// Scroll at screen coordinates with a real wheel event, e.g. `wheel 640 400 0,300`
    ///
    /// Posts a real OS-level scroll wheel event carrying its own location, so it
    /// hits whatever pane sits under (X, Y). `key pagedown` is not a substitute
    /// for a webview `overflow-y: auto` pane with no `tabindex`: the pane can
    /// never take focus, so the key scrolls the document behind it and the
    /// screenshot comes back identical, reading as an app bug.
    /// Pass --pid, --bundle-id or --window: a raw wheel event does NOT activate
    /// the target app, and an inactive app swallows the scroll without erroring.
    #[command(allow_negative_numbers = true)]
    Wheel {
        /// X in absolute screen coordinates
        x: f64,
        /// Y in absolute screen coordinates
        y: f64,
        /// Scroll delta as dX,dY in pixels, e.g. `0,300` scrolls down 300px.
        /// Positive dY scrolls down and positive dX scrolls right, matching
        /// khora's `wheel` and the DOM's WheelEvent — not Core Graphics' axes,
        /// which run the other way
        ///
        /// `allow_hyphen_values`, not the command's `allow_negative_numbers`:
        /// the latter admits a leading '-' only on a token that parses as a
        /// number, and `-800,0` does not — clap read it as the flag `-8` and
        /// scroll-left was unreachable from the CLI.
        #[arg(allow_hyphen_values = true)]
        delta: String,
        /// Number of wheel events the delta is split across. Raise for apps with
        /// momentum/smooth scrolling that clamp a single large jump
        #[arg(long, default_value_t = loki_macos::input::DEFAULT_WHEEL_STEPS)]
        steps: usize,
        /// Pause between wheel events in milliseconds (lets the app re-render)
        #[arg(long, default_value_t = loki_macos::input::DEFAULT_WHEEL_DELAY_MS)]
        delay: u64,
        /// Target process ID (activates app before scrolling)
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID, e.g. com.apple.TextEdit (activates app before scrolling)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (activates app before scrolling)
        #[arg(long)]
        window: Option<u32>,
        /// Read X and Y as offsets from the --window/--pid/--bundle-id target's
        /// frame origin instead of absolute screen coordinates
        #[arg(long)]
        relative: bool,
    },

    /// Type a string of text (use --pid, --bundle-id or --window to target an app)
    Type {
        text: String,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID, e.g. com.apple.TextEdit (resolves PID automatically)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (resolves PID automatically)
        #[arg(long)]
        window: Option<u32>,
    },

    /// Press a key combination, e.g. "cmd+shift+s" (use --pid, --bundle-id or --window)
    Key {
        combo: String,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID, e.g. com.apple.TextEdit (resolves PID automatically)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (resolves PID automatically)
        #[arg(long)]
        window: Option<u32>,
    },

    /// Open and press an app menu-bar item by path, e.g. "File>Open File…"
    ///
    /// The menu bar hangs off the application (not any window), so coordinate
    /// clicks and window-scoped `find` can't reach it. This walks the app's
    /// AXMenuBar and fires AXPress on the target item. Targets the frontmost app
    /// unless --pid, --bundle-id, or --window is given.
    Menu {
        /// Menu path, levels separated by the separator (default '>'), e.g. "File>Open File…"
        path: String,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID (e.g. com.apple.TextEdit)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (resolves the owning PID)
        #[arg(long)]
        window: Option<u32>,
        /// Path level separator
        #[arg(long, default_value = ">")]
        separator: String,
    },

    /// Read an app menu-bar item's state without pressing it, e.g. "View>Theme"
    ///
    /// Prints the item the path names plus its immediate children, each with
    /// its checkmark, enabled state, and whether it opens a submenu — enough to
    /// assert "exactly one item is checked, and it's the right one". The menu
    /// bar is invisible to `find <WID>`, so this is the only way to read it.
    /// Targets the frontmost app unless --pid, --bundle-id, or --window is given.
    MenuState {
        /// Menu path, levels separated by the separator (default '>'), e.g. "View>Theme"
        path: String,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
        /// Target bundle ID (e.g. com.apple.TextEdit)
        #[arg(long)]
        bundle_id: Option<String>,
        /// Target window ID (resolves the owning PID)
        #[arg(long)]
        window: Option<u32>,
        /// Path level separator
        #[arg(long, default_value = ">")]
        separator: String,
    },

    /// Wait for an element to appear
    WaitFor {
        window_id: u32,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Match element where any text field (title, value, description, identifier) contains the pattern. Case-insensitive; supports glob metacharacters (*, ?, [..]) — without them, matches as substring.
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Wait for an element to disappear
    WaitGone {
        window_id: u32,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Match element where any text field (title, value, description, identifier) contains the pattern. Case-insensitive; supports glob metacharacters (*, ?, [..]) — without them, matches as substring.
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Wait for a window to appear
    ///
    /// Matches exactly like `windows --title`. A timeout here is usually launch
    /// latency, not a bad pattern — a freshly built `.app` can take 15s+ to open
    /// its first window because macOS scans a new binary on first launch.
    WaitWindow {
        /// Match the window title. Case-insensitive; supports glob metacharacters (*, ?, [..]) — without them, matches as substring.
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        bundle_id: Option<String>,
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Wait for a window title to match a pattern
    WaitTitle {
        window_id: u32,
        pattern: String,
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Generate shell completions
    Completions { shell: clap_complete::Shell },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let argv: Vec<String> = std::env::args().skip(1).collect();
            let hint = targeting_hint(&e, &argv);
            let _ = e.print();
            if let Some(hint) = hint {
                eprintln!("{hint}");
            }
            return ExitCode::from(e.exit_code() as u8);
        }
    };
    let driver = MacOSDriver::new();

    match run(&cli, &driver).await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

/// clap arg ids that name *which app/window* a command acts on.
const TARGETING_ARGS: &[&str] = &["pid", "bundle_id", "window", "window_id"];

/// Name the targeting flags the command actually accepts, when the user guessed
/// a wrong one.
///
/// clap's "unexpected argument '--bundle-id' found" says what you passed and
/// never what the command takes, and targeting is the one place loki's surface
/// isn't uniform: `screenshot` takes only `--window`, while `find`/`tree`/
/// `wait-for` take a bare `<WINDOW_ID>` positional with no flag at all. The
/// exit-2 lands mid-script, where a `$WID` capture on the next line then
/// resolves to null and every command after it dies on an unrelated parse
/// error — so one wrong flag reads as four broken commands. Spend the line.
///
/// The accepted set is read back out of clap's own command tree rather than a
/// hand-kept list, so a flag added to a command can't leave this stale.
fn targeting_hint(err: &clap::Error, argv: &[String]) -> Option<String> {
    if err.kind() != ErrorKind::UnknownArgument {
        return None;
    }
    let Some(ContextValue::String(invalid)) = err.get(ContextKind::InvalidArg) else {
        return None;
    };
    let id = invalid
        .trim_start_matches('-')
        .split(['=', ' '])
        .next()?
        .replace('-', "_");
    if !TARGETING_ARGS.contains(&id.as_str()) {
        return None;
    }

    let cli = Cli::command();
    // The subcommand always precedes its own arguments, so the first argv token
    // clap recognises as one is the command being invoked.
    let sub = argv.iter().find_map(|a| cli.find_subcommand(a))?;
    let accepted: Vec<String> = sub
        .get_arguments()
        .filter(|a| TARGETING_ARGS.contains(&a.get_id().as_str()))
        .map(|a| {
            let id = a.get_id().as_str();
            if a.is_positional() {
                format!("<{}>", id.to_uppercase())
            } else {
                format!("--{}", id.replace('_', "-"))
            }
        })
        .collect();

    let name = sub.get_name();
    Some(if accepted.is_empty() {
        format!("note: `loki {name}` takes no targeting flags")
    } else {
        format!("note: `loki {name}` targets with {}", accepted.join(", "))
    })
}

async fn run(cli: &Cli, driver: &MacOSDriver) -> Result<String, loki_core::LokiError> {
    match &cli.command {
        Command::Windows {
            bundle_id,
            pid,
            title,
            all,
            require_match,
        } => {
            let filter = WindowFilter {
                title: title.clone(),
                bundle_id: bundle_id.clone(),
                pid: *pid,
                include_unnamed: *all,
            };
            let windows = driver.list_windows(&filter).await?;

            if windows.is_empty() {
                let detail = explain_empty_windows(driver, &filter).await;
                if *require_match {
                    return Err(loki_core::LokiError::WindowNotFound(detail));
                }
                // Default path keeps exit 0 and the machine-readable shape:
                // `-f json` must stay `[]` for `jq length` absence polls.
                // Text output carries the same diagnostic the error would, so
                // the caller who never passes --require-match — the one this
                // ticket exists for — still sees *why* it was empty.
                if matches!(cli.format, OutputFormat::Text) {
                    return Ok(format!("No windows found: {detail}"));
                }
            }
            Ok(loki_core::output::format_windows(&windows, cli.format))
        }

        Command::CheckPermission => {
            let granted = driver.has_accessibility_permission();
            match cli.format {
                OutputFormat::Text => {
                    if granted {
                        Ok("Accessibility permission: granted".to_string())
                    } else {
                        Ok("Accessibility permission: denied\nGrant access in System Settings > Privacy & Security > Accessibility".to_string())
                    }
                }
                OutputFormat::Json => Ok(serde_json::to_string_pretty(
                    &serde_json::json!({ "granted": granted }),
                )
                .unwrap()),
            }
        }

        Command::RequestPermission => {
            let granted = driver.request_accessibility_permission();
            match cli.format {
                OutputFormat::Text => {
                    if granted {
                        Ok("Accessibility permission: granted".to_string())
                    } else {
                        Ok(
                            "Accessibility permission prompt opened. Grant access and re-run."
                                .to_string(),
                        )
                    }
                }
                OutputFormat::Json => Ok(serde_json::to_string_pretty(
                    &serde_json::json!({ "granted": granted }),
                )
                .unwrap()),
            }
        }

        Command::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "loki", &mut std::io::stdout());
            Ok(String::new())
        }

        Command::Launch { target, args, wait } => {
            let info = driver.launch_app(target, args, *wait).await?;
            Ok(loki_core::output::format_app_info(&info, cli.format))
        }

        Command::Kill { target, force } => {
            driver.kill_app(target, *force).await?;
            match cli.format {
                OutputFormat::Text => Ok(format!("Killed: {target}")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(
                    &serde_json::json!({ "killed": target }),
                )
                .unwrap()),
            }
        }

        Command::AppInfo {
            target,
            pid,
            bundle_id,
        } => {
            let resolved = if let Some(p) = pid {
                p.to_string()
            } else if let Some(ref bid) = bundle_id {
                bid.clone()
            } else if let Some(ref t) = target {
                t.clone()
            } else {
                return Err(loki_core::LokiError::InputError(
                    "specify a target, --pid, or --bundle-id".into(),
                ));
            };
            let info = driver.app_info(&resolved).await?;
            Ok(loki_core::output::format_app_info(&info, cli.format))
        }

        Command::Screenshot {
            window,
            screen,
            output,
        } => {
            let window_id = match window {
                Some(ref w) => {
                    if let Ok(id) = w.parse::<u32>() {
                        Some(id)
                    } else {
                        // Treat as window title — look it up
                        let filter = WindowFilter {
                            title: Some(w.clone()),
                            include_unnamed: true,
                            ..Default::default()
                        };
                        let win = driver.find_window(&filter).await?.ok_or_else(|| {
                            loki_core::LokiError::WindowNotFound(format!(
                                "no window matching title '{w}'"
                            ))
                        })?;
                        Some(win.window_id)
                    }
                }
                None => None,
            };
            let png_bytes = driver.screenshot(window_id, *screen).await?;
            let path = PathBuf::from(output.as_deref().unwrap_or("loki-screenshot.png"));
            std::fs::write(&path, &png_bytes)?;

            match cli.format {
                OutputFormat::Text => Ok(format!(
                    "Screenshot saved: {} ({} bytes)",
                    path.display(),
                    png_bytes.len()
                )),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "path": path.display().to_string(),
                    "format": "png",
                    "size": png_bytes.len(),
                }))
                .unwrap()),
            }
        }

        Command::Tree {
            window_id,
            depth,
            flat,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let tree = driver.get_tree(&window, *depth).await?;

            if *flat {
                let elements = loki_core::output::flatten_tree(&tree);
                Ok(loki_core::output::format_elements(&elements, cli.format))
            } else {
                Ok(loki_core::output::format_tree(&tree, cli.format))
            }
        }

        Command::Find {
            window_id,
            role,
            title,
            label,
            id,
            index,
            require_match,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
                label: label.as_deref().map(auto_wrap_label),
                identifier: id.clone(),
                index: *index,
                ..Default::default()
            };
            let elements = driver.find_elements(&window, &query).await?;

            if elements.is_empty() {
                let detail = explain_empty_find(driver, &window, &query).await;
                if *require_match {
                    return Err(loki_core::LokiError::ElementNotFound(detail));
                }
                // Default path keeps exit 0 and the machine-readable shape:
                // `-f json` must stay `[]` for `jq length` absence asserts.
                // Text output gets the same diagnostic the error carries, so a
                // caller who never passes --require-match still sees *why*.
                if matches!(cli.format, OutputFormat::Text) {
                    return Ok(format!("No elements found: {detail}"));
                }
            }
            Ok(loki_core::output::format_elements(&elements, cli.format))
        }

        Command::Click {
            x,
            y,
            double,
            right,
            pid,
            bundle_id,
            window,
            relative,
        } => {
            let target_pid =
                resolve_target_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            let origin = resolve_relative_origin(driver, *relative, target_pid, *window).await?;
            let (sx, sy) = apply_origin(origin, *x, *y);
            driver.click(sx, sy, *double, *right, target_pid).await?;
            match cli.format {
                OutputFormat::Text => Ok(format!(
                    "Clicked at ({sx}, {sy}){}{}",
                    if *double {
                        " (double)"
                    } else if *right {
                        " (right)"
                    } else {
                        ""
                    },
                    origin_suffix(origin, &[(*x, *y)])
                )),
                OutputFormat::Json => Ok(json_with_relative(
                    serde_json::json!({
                        "action": "click",
                        "x": sx,
                        "y": sy,
                        "double": double,
                        "right": right,
                    }),
                    origin,
                    serde_json::json!({ "x": x, "y": y }),
                )),
            }
        }

        Command::ClickElement {
            window_id,
            role,
            title,
            label,
            id,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
                label: label.as_deref().map(auto_wrap_label),
                identifier: id.clone(),
                ..Default::default()
            };
            let element = driver.click_element(&window, &query).await?;
            Ok(loki_core::output::format_elements(&[element], cli.format))
        }

        Command::Drag {
            x1,
            y1,
            x2,
            y2,
            steps,
            delay,
            pid,
            bundle_id,
            window,
            relative,
        } => {
            let target_pid =
                resolve_target_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            let origin = resolve_relative_origin(driver, *relative, target_pid, *window).await?;
            let (sx1, sy1) = apply_origin(origin, *x1, *y1);
            let (sx2, sy2) = apply_origin(origin, *x2, *y2);
            driver
                .drag((sx1, sy1), (sx2, sy2), *steps, *delay, target_pid)
                .await?;
            match cli.format {
                OutputFormat::Text => Ok(format!(
                    "Dragged from ({sx1}, {sy1}) to ({sx2}, {sy2}){}",
                    origin_suffix(origin, &[(*x1, *y1), (*x2, *y2)])
                )),
                OutputFormat::Json => Ok(json_with_relative(
                    serde_json::json!({
                        "action": "drag",
                        "from": { "x": sx1, "y": sy1 },
                        "to": { "x": sx2, "y": sy2 },
                        "steps": steps,
                        "delay": delay,
                    }),
                    origin,
                    serde_json::json!({
                        "from": { "x": x1, "y": y1 },
                        "to": { "x": x2, "y": y2 },
                    }),
                )),
            }
        }

        Command::Wheel {
            x,
            y,
            delta,
            steps,
            delay,
            pid,
            bundle_id,
            window,
            relative,
        } => {
            let (dx, dy) = parse_wheel_delta(delta)?;
            let target_pid =
                resolve_target_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            let origin = resolve_relative_origin(driver, *relative, target_pid, *window).await?;
            let (sx, sy) = apply_origin(origin, *x, *y);
            driver
                .wheel((sx, sy), (dx, dy), *steps, *delay, target_pid)
                .await?;
            match cli.format {
                OutputFormat::Text => Ok(format!(
                    "Scrolled ({dx}, {dy}) at ({sx}, {sy}){}",
                    origin_suffix(origin, &[(*x, *y)])
                )),
                OutputFormat::Json => Ok(json_with_relative(
                    serde_json::json!({
                        "action": "wheel",
                        "at": { "x": sx, "y": sy },
                        "delta": { "dx": dx, "dy": dy },
                        "steps": steps,
                        "delay": delay,
                    }),
                    origin,
                    serde_json::json!({ "at": { "x": x, "y": y } }),
                )),
            }
        }

        Command::Type {
            text,
            pid,
            bundle_id,
            window,
        } => {
            let target_pid =
                resolve_target_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            driver.type_text(text, target_pid).await?;
            match cli.format {
                OutputFormat::Text => Ok(format!("Typed: {text}")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "action": "type",
                    "text": text,
                }))
                .unwrap()),
            }
        }

        Command::Key {
            combo,
            pid,
            bundle_id,
            window,
        } => {
            let target_pid =
                resolve_target_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            driver.key_press(combo, target_pid).await?;
            match cli.format {
                OutputFormat::Text => Ok(format!("Key: {combo}")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "action": "key",
                    "combo": combo,
                }))
                .unwrap()),
            }
        }

        Command::Menu {
            path,
            pid,
            bundle_id,
            window,
            separator,
        } => {
            let target_pid = resolve_menu_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            let segments = split_menu_path(path, separator)?;
            let element = driver.press_menu(target_pid, &segments).await?;
            Ok(loki_core::output::format_elements(&[element], cli.format))
        }

        Command::MenuState {
            path,
            pid,
            bundle_id,
            window,
            separator,
        } => {
            let target_pid = resolve_menu_pid(driver, *pid, *window, bundle_id.as_deref()).await?;
            let segments = split_menu_path(path, separator)?;
            let state = driver.menu_state(target_pid, &segments).await?;
            Ok(loki_core::output::format_menu_state(&state, cli.format))
        }

        Command::WaitFor {
            window_id,
            role,
            title,
            label,
            id,
            timeout,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
                label: label.as_deref().map(auto_wrap_label),
                identifier: id.clone(),
                ..Default::default()
            };
            let t = timeout.unwrap_or(cli.timeout);
            let element = driver.wait_for(&window, &query, t).await?;
            Ok(loki_core::output::format_elements(&[element], cli.format))
        }

        Command::WaitGone {
            window_id,
            role,
            title,
            label,
            id,
            timeout,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
                label: label.as_deref().map(auto_wrap_label),
                identifier: id.clone(),
                ..Default::default()
            };
            let t = timeout.unwrap_or(cli.timeout);
            driver.wait_gone(&window, &query, t).await?;
            match cli.format {
                OutputFormat::Text => Ok("Element is gone.".to_string()),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(
                    &serde_json::json!({ "status": "gone" }),
                )
                .unwrap()),
            }
        }

        Command::WaitWindow {
            title,
            bundle_id,
            timeout,
        } => {
            let filter = WindowFilter {
                title: title.clone(),
                bundle_id: bundle_id.clone(),
                pid: None,
                include_unnamed: true,
            };
            let t = timeout.unwrap_or(cli.timeout);
            let info = match driver.wait_window(&filter, t).await {
                Ok(info) => info,
                // A bare "timed out after Nms" can't tell a slow launch from a
                // pattern that never matched — say which, with evidence.
                Err(loki_core::LokiError::Timeout(ms)) => {
                    return Err(explain_wait_window_timeout(driver, &filter, ms).await)
                }
                Err(e) => return Err(e),
            };
            Ok(loki_core::output::format_windows(&[info], cli.format))
        }

        Command::WaitTitle {
            window_id,
            pattern,
            timeout,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let t = timeout.unwrap_or(cli.timeout);
            let info = driver.wait_title(&window, pattern, t).await?;
            Ok(loki_core::output::format_windows(&[info], cli.format))
        }
    }
}

/// Turn an empty `find` result into something diagnosable. The whole point of
/// mesa 550 is that "no match" and "wrong query" printed the same four words,
/// so this reports what was actually searched and which *relaxation* of the
/// query would have hit — which is what separates a typo from an absent
/// element. Same shape as `explain_wait_window_timeout`.
///
/// The named causes it has to distinguish: wrong `--role`, wrong window,
/// element not rendered yet, and an out-of-range `--index` (which silently
/// empties a result set that did match).
async fn explain_empty_find(
    driver: &MacOSDriver,
    window: &loki_core::WindowRef,
    query: &ElementQuery,
) -> String {
    // The tree as it stands, unfiltered — an empty default query matches every
    // node, so this is the denominator every line below is measured against.
    let all = driver
        .find_elements(window, &ElementQuery::default())
        .await
        .unwrap_or_default();

    let mut wanted = Vec::new();
    if let Some(ref r) = query.role {
        wanted.push(format!("role {r:?}"));
    }
    if let Some(ref t) = query.title {
        wanted.push(format!("title {t:?}"));
    }
    if let Some(ref l) = query.label {
        wanted.push(format!("label {l:?}"));
    }
    if let Some(ref i) = query.identifier {
        wanted.push(format!("id {i:?}"));
    }
    if let Some(idx) = query.index {
        wanted.push(format!("index {idx}"));
    }
    let wanted = if wanted.is_empty() {
        "any element".to_string()
    } else {
        wanted.join(" + ")
    };

    let mut lines = vec![
        format!("no element matched {wanted}"),
        format!(
            "  searched: {} elements in window {} (pid {})",
            all.len(),
            window.window_id,
            window.pid
        ),
    ];

    if all.is_empty() {
        // Either the wrong window id, or a webview: both look like this.
        lines.push(
            "  the window's tree is empty — wrong window id, or a WKWebView/Electron UI \
             that the AX API does not expose (screenshot + coordinate clicks instead)"
                .to_string(),
        );
        return lines.join("\n");
    }

    // --index is applied *after* filtering, so an out-of-range index empties a
    // result set that matched perfectly well. Report the count it should use.
    let without_index = ElementQuery {
        index: None,
        ..query.clone()
    };
    if query.index.is_some() {
        let n = all.iter().filter(|e| without_index.matches(e)).count();
        if n > 0 {
            lines.push(format!(
                "  {n} element(s) matched the rest of the query — --index is 0-based, so the \
                 highest valid one here is {}",
                n - 1
            ));
            return lines.join("\n");
        }
    }

    let has_text = query.title.is_some() || query.label.is_some() || query.identifier.is_some();

    // Relax the role: if the text alone hits, the role was the wrong guess —
    // the ticket's first named cause, and invisible in the old output.
    if query.role.is_some() && has_text {
        let relaxed = ElementQuery {
            role: None,
            index: None,
            ..query.clone()
        };
        let hits: Vec<&loki_core::AXElement> = all.iter().filter(|e| relaxed.matches(e)).collect();
        if !hits.is_empty() {
            let mut roles: Vec<String> = hits.iter().map(|e| e.role.clone()).collect();
            roles.sort();
            roles.dedup();
            lines.push(format!(
                "  {} element(s) matched the text but not the role — they are: {}",
                hits.len(),
                roles.join(", ")
            ));
            return lines.join("\n");
        }
    }

    // Role given, nothing matched it at all: name the roles that do exist.
    if query.role.is_some() {
        let matched_role = ElementQuery {
            role: query.role.clone(),
            ..Default::default()
        };
        if !all.iter().any(|e| matched_role.matches(e)) {
            let mut roles: Vec<String> = all.iter().map(|e| e.role.clone()).collect();
            roles.sort();
            roles.dedup();
            let shown = roles.len().min(12);
            lines.push(format!(
                "  no element has that role; roles present: {}{}",
                roles[..shown].join(", "),
                if roles.len() > shown { ", …" } else { "" }
            ));
            return lines.join("\n");
        }
    }

    // Text given and nothing matched: is the needle in the tree at all, under
    // a field this query does not look at? `--title` is a strict field match
    // where `--label` is the broad one, so this is where that mix-up shows up.
    if has_text {
        let needle = query
            .title
            .as_deref()
            .or(query.label.as_deref())
            .or(query.identifier.as_deref())
            .unwrap_or_default()
            .trim_matches('*')
            .to_lowercase();
        if !needle.is_empty() {
            let near: Vec<String> = all
                .iter()
                .filter(|e| {
                    [&e.title, &e.value, &e.description, &e.identifier]
                        .iter()
                        .any(|f| {
                            f.as_deref()
                                .is_some_and(|s| s.to_lowercase().contains(&needle))
                        })
                })
                .take(3)
                .map(|e| {
                    let label = e
                        .title
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .or(e.value.as_deref())
                        .or(e.description.as_deref())
                        .unwrap_or("");
                    format!("{} {label:?}", e.role)
                })
                .collect();
            if near.is_empty() {
                lines.push(format!(
                    "  no element's text contains {needle:?} — matching is case-insensitive, \
                     so this is not a case problem (--id stays exact); the element may not be \
                     rendered yet: `wait-for` instead of `find`"
                ));
            } else {
                lines.push(format!(
                    "  near-miss (some text field contains {needle:?}): {}",
                    near.join(", ")
                ));
                lines.push(
                    "  hint: --title matches title/description/identifier; --label also \
                     matches AXValue (webview text)"
                        .to_string(),
                );
            }
        }
    }

    lines.join("\n")
}

/// Turn an empty `windows` listing into something diagnosable.
///
/// `windows` is the first line of nearly every loki script — `WID=$(loki -f
/// json windows --title X | jq -r '.[0].window_id')` — so a miss here does not
/// surface here. `$WID` becomes the string `"null"` and the *next* command dies
/// on an unrelated parse error, which reads as a broken loki rather than a
/// mistyped title (mesa 565). Same shape as `explain_empty_find` (mesa 550) and
/// `explain_wait_window_timeout`: report what was actually searched, and which
/// *relaxation* of the query would have hit.
///
/// The causes it has to separate: a title the glob anchored past, a wrong
/// `--bundle-id`/`--pid`, a window that has not opened yet — and the one no
/// caller can see, an untitled window dropped before any flag was applied
/// because `--all` was not passed.
async fn explain_empty_windows(driver: &MacOSDriver, filter: &WindowFilter) -> String {
    // The list as it stands, unfiltered and including untitled windows — the
    // denominator every line below is measured against.
    let all = driver
        .list_windows(&WindowFilter {
            include_unnamed: true,
            ..Default::default()
        })
        .await
        .unwrap_or_default();
    let titled = all.iter().filter(|w| !w.title.is_empty()).count();

    let mut wanted = Vec::new();
    if let Some(pattern) = filter.effective_title_pattern() {
        wanted.push(format!("title glob {pattern:?}"));
    }
    if let Some(ref bundle_id) = filter.bundle_id {
        wanted.push(format!("bundle-id {bundle_id:?}"));
    }
    if let Some(pid) = filter.pid {
        wanted.push(format!("pid {pid}"));
    }
    let wanted = if wanted.is_empty() {
        "any window".to_string()
    } else {
        wanted.join(" + ")
    };

    let mut lines = vec![
        format!("no window matched {wanted}"),
        format!(
            "  searched: {} windows ({titled} titled, {} untitled{})",
            all.len(),
            all.len() - titled,
            if filter.include_unnamed {
                ""
            } else {
                ", excluded without --all"
            }
        ),
    ];

    if all.is_empty() {
        // Nothing in the CG list at all — not a query problem.
        lines.push(
            "  the window list itself is empty — no app has an on-screen window, or screen \
             access is not granted (`loki check-permission`)"
                .to_string(),
        );
        return lines.join("\n");
    }

    // Relaxation 1: `--all`. Checked first because it is the only constraint
    // the caller never typed — the default listing drops untitled windows
    // before any flag is applied, so a filter that matches perfectly still
    // comes back empty and nothing in the query explains it.
    if !filter.include_unnamed {
        let relaxed = WindowFilter {
            include_unnamed: true,
            ..filter.clone()
        };
        let n = all.iter().filter(|w| relaxed.matches(w)).count();
        if n > 0 {
            lines.push(format!(
                "  {n} window(s) match the rest of the query but have an empty title — `--all` \
                 would have included them"
            ));
            return lines.join("\n");
        }
    }

    // Relaxation 2: drop one typed flag at a time and report which single one
    // is doing the excluding. This is what separates "wrong bundle-id" from
    // "right app, wrong title" when both were passed.
    let mut relaxations: Vec<(&str, WindowFilter)> = Vec::new();
    if filter.title.is_some() {
        relaxations.push((
            "--title",
            WindowFilter {
                title: None,
                ..filter.clone()
            },
        ));
    }
    if filter.bundle_id.is_some() {
        relaxations.push((
            "--bundle-id",
            WindowFilter {
                bundle_id: None,
                ..filter.clone()
            },
        ));
    }
    if filter.pid.is_some() {
        relaxations.push((
            "--pid",
            WindowFilter {
                pid: None,
                ..filter.clone()
            },
        ));
    }
    // Only meaningful when more than one flag was given: with a single flag,
    // "dropping it would have matched" just restates that other windows exist.
    if relaxations.len() > 1 {
        let hits: Vec<(&str, usize)> = relaxations
            .iter()
            .map(|(name, relaxed)| (*name, all.iter().filter(|w| relaxed.matches(w)).count()))
            .filter(|(_, n)| *n > 0)
            .collect();
        match hits.len() {
            0 => {}
            // Exactly one flag is doing the excluding — the actionable case.
            1 => lines.push(format!(
                "  dropping {} would have matched {} window(s) — that flag is the one excluding \
                 everything",
                hits[0].0, hits[0].1
            )),
            // Several. Saying "that flag is the one" of each would contradict
            // itself: each *alone* is satisfiable, and it is the combination
            // that is not — usually flags pointing at two different apps.
            _ => lines.push(format!(
                "  no window satisfies all of them together, though each is satisfiable alone: {} \
                 — the flags are describing different windows",
                hits.iter()
                    .map(|(name, n)| format!("dropping {name} matches {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    // A wrong --bundle-id and an app that simply has no window yet look
    // identical from the listing, and only one of them is a typo.
    if let Some(ref bundle_id) = filter.bundle_id {
        let n = all
            .iter()
            .filter(|w| {
                w.bundle_id
                    .as_deref()
                    .is_some_and(|b| b.eq_ignore_ascii_case(bundle_id))
            })
            .count();
        if n == 0 {
            let needle = bundle_id.to_lowercase();
            // Dedup before truncating: one app owns many windows, so without
            // this the "near-miss" list is the same bundle-id three times.
            let mut near: Vec<String> = all
                .iter()
                .filter_map(|w| w.bundle_id.as_deref())
                .filter(|b| {
                    let b = b.to_lowercase();
                    b.contains(&needle) || needle.contains(&b)
                })
                .map(|b| format!("{b:?}"))
                .collect();
            near.sort();
            near.dedup();
            near.truncate(3);
            if near.is_empty() {
                lines.push(format!(
                    "  no window belongs to {bundle_id:?}; if the app is running it has not \
                     opened a window yet — `wait-window --bundle-id` (exit 3) rather than \
                     polling `windows`"
                ));
            } else {
                lines.push(format!(
                    "  no window belongs to {bundle_id:?} — near-miss bundle-ids present: {}",
                    near.join(", ")
                ));
            }
        }
    }

    if let Some(pid) = filter.pid {
        if !all.iter().any(|w| w.pid == pid) {
            lines.push(format!(
                "  no window belongs to pid {pid} — the process may have exited, or the pid is \
                 stale (re-resolve it with `loki app-info`)"
            ));
        }
    }

    // Case stopped being a cause of a miss in mesa 540, so what is left for a
    // title is an anchoring miss: the title carries the typed text but the
    // glob's leading or trailing anchor excluded it. Report the *trimmed*
    // needle actually tested — the title does not contain the `*`.
    if let Some(raw) = filter.title.as_deref() {
        let needle = raw.trim_matches('*');
        let lowered = needle.to_lowercase();
        // Only a window the *glob itself* rejected is an anchoring near-miss.
        // Without the `!matches_title` guard this happily reports a title the
        // glob matched perfectly, when it was --bundle-id or --pid that
        // excluded it — a diagnostic pointing at the wrong flag is worse than
        // none, which is the whole complaint this ticket is about.
        let mut near: Vec<String> = all
            .iter()
            .filter(|w| {
                !w.title.is_empty()
                    && !filter.matches_title(&w.title)
                    && w.title.to_lowercase().contains(&lowered)
            })
            .map(|w| format!("{:?}", w.title))
            .collect();
        near.sort();
        near.dedup();
        near.truncate(3);
        // Distinct from "the glob anchored past it": if the glob does match
        // some title, the title is not the reason the result was empty, and
        // the lines above have already named the flag that was.
        let title_alone_hits = all
            .iter()
            .any(|w| !w.title.is_empty() && filter.matches_title(&w.title));
        if near.is_empty() && !title_alone_hits {
            let sample: Vec<String> = all
                .iter()
                .filter(|w| !w.title.is_empty())
                .map(|w| format!("{:?}", w.title))
                .take(3)
                .collect();
            lines.push(format!(
                "  no window title contains {needle:?} — matching is case-insensitive, so this \
                 is not a case problem; the window may not be open yet (`wait-window --title`, \
                 exit 3). Titles present: {}{}",
                sample.join(", "),
                if titled > sample.len() { ", …" } else { "" }
            ));
        } else if !near.is_empty() {
            lines.push(format!(
                "  near-miss (contains {needle:?} but the glob above did not match): {}",
                near.join(", ")
            ));
        }
    }

    lines.join("\n")
}

/// Turn a `wait-window` timeout into something diagnosable: the glob actually
/// matched against, the window list as it stood when time ran out, any loose
/// near-miss, and the launch-latency trap that is the usual cause (a freshly
/// built `.app` can take well over 10s to show its window).
async fn explain_wait_window_timeout(
    driver: &MacOSDriver,
    filter: &WindowFilter,
    timeout_ms: u64,
) -> loki_core::LokiError {
    let all = driver
        .list_windows(&WindowFilter {
            include_unnamed: true,
            ..Default::default()
        })
        .await
        .unwrap_or_default();
    let titled: Vec<&loki_core::WindowInfo> = all.iter().filter(|w| !w.title.is_empty()).collect();

    let mut wanted = Vec::new();
    if let Some(pattern) = filter.effective_title_pattern() {
        wanted.push(format!("title glob {pattern:?}"));
    }
    if let Some(ref bundle_id) = filter.bundle_id {
        wanted.push(format!("bundle-id {bundle_id:?}"));
    }
    let wanted = if wanted.is_empty() {
        "any window".to_string()
    } else {
        wanted.join(" + ")
    };

    let mut lines = vec![
        format!("waiting for {wanted}"),
        format!("  seen: {} windows ({} titled)", all.len(), titled.len()),
    ];

    if let Some(ref bundle_id) = filter.bundle_id {
        let n = all
            .iter()
            .filter(|w| {
                w.bundle_id
                    .as_deref()
                    .is_some_and(|b| b.eq_ignore_ascii_case(bundle_id))
            })
            .count();
        lines.push(format!("  {n} of them belong to {bundle_id:?}"));
    }

    // Case is no longer a cause of a miss (mesa 540), so what is left is an
    // anchoring miss: the title carries the typed text, but the glob's leading
    // or trailing anchor excluded it — e.g. `"ash-md*"` against
    // `"the ash-md window"`. Report the *trimmed* needle actually tested, not
    // the raw pattern: the title does not contain the `*`.
    //
    // A pattern whose metacharacters are `?` or `[..]` will usually produce no
    // near-miss at all, since those characters stay in the needle and so are
    // themselves required to appear in the title.
    if let Some(raw) = filter.title.as_deref() {
        let needle = raw.trim_matches('*');
        let lowered = needle.to_lowercase();
        let near: Vec<String> = titled
            .iter()
            .filter(|w| w.title.to_lowercase().contains(&lowered))
            .map(|w| format!("{:?}", w.title))
            .take(3)
            .collect();
        if !near.is_empty() {
            lines.push(format!(
                "  near-miss (contains {needle:?} but the glob above did not match): {}",
                near.join(", ")
            ));
        }
    }

    lines.push(
        "  hint: a freshly built or newly copied .app can take 15s+ to open its first window \
         (macOS scans a new binary on first launch) — retry with a longer --timeout"
            .to_string(),
    );

    loki_core::LokiError::TimeoutDetail {
        timeout_ms,
        detail: lines.join("\n"),
    }
}

/// Resolve the app whose menu bar to walk: the shared targeting flags, then the
/// frontmost app — `menu`/`menu-state` always need *some* app, unlike the input
/// commands where "no target" legitimately means "whatever has focus".
async fn resolve_menu_pid(
    driver: &MacOSDriver,
    pid: Option<u32>,
    window: Option<u32>,
    bundle_id: Option<&str>,
) -> Result<i32, loki_core::LokiError> {
    if let Some(p) = resolve_target_pid(driver, pid, window, bundle_id).await? {
        return Ok(p);
    }
    Ok(loki_macos::app::frontmost_pid().ok_or_else(|| {
        loki_core::LokiError::AppNotFound(
            "no frontmost app — specify --pid, --bundle-id, or --window".into(),
        )
    })? as i32)
}

/// Split a menu path like `"View>Theme"` into its levels.
fn split_menu_path(path: &str, separator: &str) -> Result<Vec<String>, loki_core::LokiError> {
    let segments: Vec<String> = path
        .split(separator)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return Err(loki_core::LokiError::InputError(format!(
            "empty menu path '{path}'"
        )));
    }
    Ok(segments)
}

/// Parse a `dX,dY` scroll delta in pixels.
///
/// The pair is required rather than inferred from a bare number: `wheel 640 400
/// 300` has no honest reading — horizontal and vertical are equally plausible —
/// and guessing vertical would scroll the wrong axis silently. Same shape as
/// `khora wheel <AT> <DELTA>`.
fn parse_wheel_delta(delta: &str) -> Result<(i32, i32), loki_core::LokiError> {
    let bad = || {
        loki_core::LokiError::InputError(format!(
            "invalid scroll delta '{delta}' — expected dX,dY in pixels, e.g. '0,300' to scroll down"
        ))
    };
    let (dx, dy) = delta.split_once(',').ok_or_else(bad)?;
    Ok((
        dx.trim().parse::<i32>().map_err(|_| bad())?,
        dy.trim().parse::<i32>().map_err(|_| bad())?,
    ))
}

/// Resolve the frame origin `--relative` coordinates are measured from.
///
/// `None` when the flag is off — callers then use the coordinates as given.
/// The target must be unambiguous: a wrong origin produces a click that lands
/// somewhere plausible and still exits 0, which is the failure shape loki keeps
/// paying for. `--window` names exactly one window and always wins; an app
/// (named by `--pid` or `--bundle-id`, already resolved to `pid` here) is only
/// usable when it owns exactly one on-screen window.
async fn resolve_relative_origin(
    driver: &MacOSDriver,
    relative: bool,
    pid: Option<i32>,
    window_id: Option<u32>,
) -> Result<Option<(f64, f64)>, loki_core::LokiError> {
    if !relative {
        return Ok(None);
    }

    if let Some(wid) = window_id {
        let filter = WindowFilter {
            include_unnamed: true,
            ..Default::default()
        };
        let info = driver
            .list_windows(&filter)
            .await?
            .into_iter()
            .find(|w| w.window_id == wid)
            .ok_or_else(|| loki_core::LokiError::WindowNotFound(format!("window_id={wid}")))?;
        return Ok(Some((info.frame.x, info.frame.y)));
    }

    let Some(p) = pid else {
        return Err(loki_core::LokiError::InputError(
            "--relative needs a frame to resolve against — pass --window <ID> (or --pid <PID> / --bundle-id <ID> for a single-window app)".into(),
        ));
    };

    let filter = WindowFilter {
        pid: Some(p as u32),
        include_unnamed: true,
        ..Default::default()
    };
    let mut windows: Vec<_> = driver
        .list_windows(&filter)
        .await?
        .into_iter()
        .filter(|w| w.is_on_screen)
        .collect();

    match windows.len() {
        1 => {
            let w = windows.remove(0);
            Ok(Some((w.frame.x, w.frame.y)))
        }
        0 => Err(loki_core::LokiError::InputError(format!(
            "--relative: PID {p} has no on-screen window to resolve coordinates against"
        ))),
        n => {
            let candidates = windows
                .iter()
                .map(|w| {
                    let title = if w.title.is_empty() {
                        "<untitled>"
                    } else {
                        &w.title
                    };
                    format!(
                        "  --window {} — \"{}\" at {},{}",
                        w.window_id, title, w.frame.x, w.frame.y
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Err(loki_core::LokiError::InputError(format!(
                "--relative: PID {p} has {n} on-screen windows — name one with --window:\n{candidates}"
            )))
        }
    }
}

/// Offset a point by a `--relative` origin, or pass it through unchanged.
fn apply_origin(origin: Option<(f64, f64)>, x: f64, y: f64) -> (f64, f64) {
    match origin {
        Some((ox, oy)) => (ox + x, oy + y),
        None => (x, y),
    }
}

/// Attach the `relative` block (origin + the coordinates as typed) to a JSON
/// payload whose top-level coordinates are the resolved screen point.
///
/// The key is *absent* without `--relative` rather than null: absolute mode is
/// the default and its output shape must stay exactly what existing consumers
/// already parse.
fn json_with_relative(
    mut payload: serde_json::Value,
    origin: Option<(f64, f64)>,
    input: serde_json::Value,
) -> String {
    if let (Some((ox, oy)), Some(obj)) = (origin, payload.as_object_mut()) {
        let mut relative = serde_json::json!({ "origin": { "x": ox, "y": oy } });
        if let (Some(rel), Some(input)) = (relative.as_object_mut(), input.as_object()) {
            rel.extend(input.clone());
        }
        obj.insert("relative".into(), relative);
    }
    serde_json::to_string_pretty(&payload).unwrap()
}

/// Echo the window-relative input alongside the screen coordinates actually
/// used, so a mis-resolved origin is visible in stdout rather than only in a
/// screenshot.
fn origin_suffix(origin: Option<(f64, f64)>, points: &[(f64, f64)]) -> String {
    let Some((ox, oy)) = origin else {
        return String::new();
    };
    let pts = points
        .iter()
        .map(|(x, y)| format!("({x}, {y})"))
        .collect::<Vec<_>>()
        .join(" → ");
    format!(" [relative {pts} from window origin ({ox}, {oy})]")
}

/// Resolve a target PID from the targeting flags every input command shares:
/// --pid, then --window's owner, then --bundle-id.
/// Returns Some(pid) if any is specified, None otherwise (uses focused app).
async fn resolve_target_pid(
    driver: &MacOSDriver,
    pid: Option<u32>,
    window_id: Option<u32>,
    bundle_id: Option<&str>,
) -> Result<Option<i32>, loki_core::LokiError> {
    if let Some(p) = pid {
        return Ok(Some(p as i32));
    }
    if let Some(wid) = window_id {
        let wref = find_window_ref(driver, wid).await?;
        return Ok(Some(wref.pid as i32));
    }
    if let Some(bid) = bundle_id {
        return Ok(Some(driver.app_info(bid).await?.pid as i32));
    }
    Ok(None)
}

/// Look up a WindowRef by window ID from the system window list.
async fn find_window_ref(
    driver: &MacOSDriver,
    window_id: u32,
) -> Result<WindowRef, loki_core::LokiError> {
    let filter = WindowFilter {
        include_unnamed: true,
        ..Default::default()
    };
    let windows = driver.list_windows(&filter).await?;

    let info = windows
        .into_iter()
        .find(|w| w.window_id == window_id)
        .ok_or_else(|| loki_core::LokiError::WindowNotFound(format!("window_id={window_id}")))?;

    Ok(WindowRef {
        window_id: info.window_id,
        pid: info.pid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wheel_delta_pair() {
        assert_eq!(parse_wheel_delta("0,300").unwrap(), (0, 300));
    }

    #[test]
    fn test_parse_wheel_delta_negative_and_spaced() {
        assert_eq!(parse_wheel_delta("-40, -300").unwrap(), (-40, -300));
    }

    #[test]
    fn test_parse_wheel_delta_rejects_bare_number() {
        // Ambiguous between axes — must not silently be read as vertical.
        let err = parse_wheel_delta("300").unwrap_err().to_string();
        assert!(err.contains("dX,dY"), "unhelpful error: {err}");
    }

    #[test]
    fn test_apply_origin_offsets_both_axes() {
        assert_eq!(
            apply_origin(Some((1200.0, 80.0)), 100.0, 50.0),
            (1300.0, 130.0)
        );
    }

    #[test]
    fn test_apply_origin_passthrough_when_absolute() {
        assert_eq!(apply_origin(None, 100.0, 50.0), (100.0, 50.0));
    }

    #[test]
    fn test_apply_origin_handles_negative_origin() {
        // A display left of the primary has a negative origin.
        assert_eq!(
            apply_origin(Some((-1440.0, 0.0)), 10.0, 20.0),
            (-1430.0, 20.0)
        );
    }

    #[test]
    fn test_json_omits_relative_key_when_absolute() {
        // Absolute mode's output shape must not change at all.
        let out = json_with_relative(
            serde_json::json!({ "action": "click", "x": 1.0 }),
            None,
            serde_json::json!({ "x": 1.0 }),
        );
        assert!(!out.contains("relative"), "unexpected key: {out}");
    }

    #[test]
    fn test_json_carries_origin_and_input_when_relative() {
        let out = json_with_relative(
            serde_json::json!({ "action": "click", "x": 620.0, "y": 183.0 }),
            Some((520.0, 163.0)),
            serde_json::json!({ "x": 100.0, "y": 20.0 }),
        );
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["x"], 620.0, "top level stays the screen point: {out}");
        assert_eq!(v["relative"]["origin"]["x"], 520.0);
        assert_eq!(v["relative"]["x"], 100.0);
    }

    #[test]
    fn test_origin_suffix_empty_when_absolute() {
        assert_eq!(origin_suffix(None, &[(1.0, 2.0)]), "");
    }

    #[test]
    fn test_origin_suffix_echoes_input_and_origin() {
        let s = origin_suffix(Some((100.0, 20.0)), &[(5.0, 6.0), (7.0, 8.0)]);
        assert!(s.contains("(5, 6) → (7, 8)"), "missing input points: {s}");
        assert!(s.contains("(100, 20)"), "missing origin: {s}");
    }

    #[test]
    fn test_parse_wheel_delta_rejects_non_numeric() {
        assert!(parse_wheel_delta("0,down").is_err());
        assert!(parse_wheel_delta("0,300,0").is_err());
        assert!(parse_wheel_delta("").is_err());
    }

    #[test]
    fn test_split_menu_path_levels() {
        assert_eq!(
            split_menu_path("View>Theme", ">").unwrap(),
            ["View", "Theme"]
        );
    }

    #[test]
    fn test_split_menu_path_trims_spaces_around_separator() {
        assert_eq!(
            split_menu_path("View > Theme", ">").unwrap(),
            ["View", "Theme"]
        );
    }

    #[test]
    fn test_split_menu_path_custom_separator() {
        assert_eq!(
            split_menu_path("View/Theme", "/").unwrap(),
            ["View", "Theme"]
        );
    }

    #[test]
    fn test_split_menu_path_rejects_empty() {
        assert!(split_menu_path("", ">").is_err());
        assert!(split_menu_path(">>", ">").is_err());
    }

    #[test]
    fn test_auto_wrap_bare_literal() {
        assert_eq!(auto_wrap_label("Projects"), "*Projects*");
    }

    #[test]
    fn test_auto_wrap_empty_string() {
        assert_eq!(auto_wrap_label(""), "");
    }

    #[test]
    fn test_auto_wrap_star_suffix_passthrough() {
        assert_eq!(auto_wrap_label("Projects*"), "Projects*");
    }

    #[test]
    fn test_auto_wrap_leading_star_passthrough() {
        assert_eq!(auto_wrap_label("*Projects*"), "*Projects*");
    }

    #[test]
    fn test_auto_wrap_question_passthrough() {
        assert_eq!(auto_wrap_label("Proj?cts"), "Proj?cts");
    }

    #[test]
    fn test_auto_wrap_bracket_passthrough() {
        assert_eq!(auto_wrap_label("[test]"), "[test]");
    }

    #[test]
    fn test_auto_wrap_closing_bracket_wraps() {
        assert_eq!(auto_wrap_label("]"), "*]*");
    }

    #[test]
    fn test_auto_wrap_unicode_emoji() {
        assert_eq!(auto_wrap_label("📋"), "*📋*");
    }

    #[test]
    fn test_auto_wrap_whitespace() {
        assert_eq!(auto_wrap_label("  "), "*  *");
    }
}
