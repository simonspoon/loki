use clap::{Parser, Subcommand};
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
        /// Target window ID (activates app before clicking)
        #[arg(long)]
        window: Option<u32>,
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
    /// Pass --pid or --window: a raw mouse event does NOT activate the target
    /// app, and an inactive app swallows the whole drag without erroring.
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
        /// Target window ID (activates app before dragging)
        #[arg(long)]
        window: Option<u32>,
    },

    /// Scroll at screen coordinates with a real wheel event, e.g. `wheel 640 400 0,300`
    ///
    /// Posts a real OS-level scroll wheel event carrying its own location, so it
    /// hits whatever pane sits under (X, Y). `key pagedown` is not a substitute
    /// for a webview `overflow-y: auto` pane with no `tabindex`: the pane can
    /// never take focus, so the key scrolls the document behind it and the
    /// screenshot comes back identical, reading as an app bug.
    /// Pass --pid or --window: a raw wheel event does NOT activate the target
    /// app, and an inactive app swallows the scroll without erroring.
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
        /// Target window ID (activates app before scrolling)
        #[arg(long)]
        window: Option<u32>,
    },

    /// Type a string of text (use --pid or --window to target a specific app)
    Type {
        text: String,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
        /// Target window ID (resolves PID automatically)
        #[arg(long)]
        window: Option<u32>,
    },

    /// Press a key combination, e.g. "cmd+shift+s" (use --pid or --window to target)
    Key {
        combo: String,
        /// Target process ID
        #[arg(long)]
        pid: Option<u32>,
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

    let cli = Cli::parse();
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

async fn run(cli: &Cli, driver: &MacOSDriver) -> Result<String, loki_core::LokiError> {
    match &cli.command {
        Command::Windows {
            bundle_id,
            pid,
            title,
            all,
        } => {
            let filter = WindowFilter {
                title: title.clone(),
                bundle_id: bundle_id.clone(),
                pid: *pid,
                include_unnamed: *all,
            };
            let windows = driver.list_windows(&filter).await?;
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
            Ok(loki_core::output::format_elements(&elements, cli.format))
        }

        Command::Click {
            x,
            y,
            double,
            right,
            pid,
            window,
        } => {
            let target_pid = resolve_target_pid(driver, *pid, *window).await?;
            driver.click(*x, *y, *double, *right, target_pid).await?;
            match cli.format {
                OutputFormat::Text => Ok(format!(
                    "Clicked at ({x}, {y}){}",
                    if *double {
                        " (double)"
                    } else if *right {
                        " (right)"
                    } else {
                        ""
                    }
                )),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "action": "click",
                    "x": x,
                    "y": y,
                    "double": double,
                    "right": right,
                }))
                .unwrap()),
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
            window,
        } => {
            let target_pid = resolve_target_pid(driver, *pid, *window).await?;
            driver
                .drag((*x1, *y1), (*x2, *y2), *steps, *delay, target_pid)
                .await?;
            match cli.format {
                OutputFormat::Text => Ok(format!("Dragged from ({x1}, {y1}) to ({x2}, {y2})")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "action": "drag",
                    "from": { "x": x1, "y": y1 },
                    "to": { "x": x2, "y": y2 },
                    "steps": steps,
                    "delay": delay,
                }))
                .unwrap()),
            }
        }

        Command::Wheel {
            x,
            y,
            delta,
            steps,
            delay,
            pid,
            window,
        } => {
            let (dx, dy) = parse_wheel_delta(delta)?;
            let target_pid = resolve_target_pid(driver, *pid, *window).await?;
            driver
                .wheel((*x, *y), (dx, dy), *steps, *delay, target_pid)
                .await?;
            match cli.format {
                OutputFormat::Text => Ok(format!("Scrolled ({dx}, {dy}) at ({x}, {y})")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "action": "wheel",
                    "at": { "x": x, "y": y },
                    "delta": { "dx": dx, "dy": dy },
                    "steps": steps,
                    "delay": delay,
                }))
                .unwrap()),
            }
        }

        Command::Type { text, pid, window } => {
            let target_pid = resolve_target_pid(driver, *pid, *window).await?;
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

        Command::Key { combo, pid, window } => {
            let target_pid = resolve_target_pid(driver, *pid, *window).await?;
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

/// Resolve the app whose menu bar to walk: --pid, then --window's owner, then
/// --bundle-id, then the frontmost app.
async fn resolve_menu_pid(
    driver: &MacOSDriver,
    pid: Option<u32>,
    window: Option<u32>,
    bundle_id: Option<&str>,
) -> Result<i32, loki_core::LokiError> {
    if let Some(p) = pid {
        return Ok(p as i32);
    }
    if let Some(wid) = window {
        return Ok(find_window_ref(driver, wid).await?.pid as i32);
    }
    if let Some(bid) = bundle_id {
        return Ok(driver.app_info(bid).await?.pid as i32);
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

/// Resolve a target PID from --pid or --window flags.
/// Returns Some(pid) if either is specified, None otherwise (uses focused app).
async fn resolve_target_pid(
    driver: &MacOSDriver,
    pid: Option<u32>,
    window_id: Option<u32>,
) -> Result<Option<i32>, loki_core::LokiError> {
    if let Some(p) = pid {
        return Ok(Some(p as i32));
    }
    if let Some(wid) = window_id {
        let wref = find_window_ref(driver, wid).await?;
        return Ok(Some(wref.pid as i32));
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
