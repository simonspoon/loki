use clap::{Parser, Subcommand};
use loki_core::{DesktopDriver, ElementQuery, OutputFormat, WindowFilter, WindowRef};
use loki_macos::MacOSDriver;
use std::path::PathBuf;
use std::process::ExitCode;

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
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        index: Option<usize>,
    },

    /// Click at screen coordinates
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
        #[arg(long)]
        id: Option<String>,
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

    /// Wait for an element to appear
    WaitFor {
        window_id: u32,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        title: Option<String>,
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
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Wait for a window to appear
    WaitWindow {
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
            id,
            index,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
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
            id,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
                identifier: id.clone(),
                ..Default::default()
            };
            let element = driver.click_element(&window, &query).await?;
            Ok(loki_core::output::format_elements(&[element], cli.format))
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

        Command::WaitFor {
            window_id,
            role,
            title,
            id,
            timeout,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
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
            id,
            timeout,
        } => {
            let window = find_window_ref(driver, *window_id).await?;
            let query = ElementQuery {
                role: role.clone(),
                title: title.clone(),
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
            let info = driver.wait_window(&filter, t).await?;
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
