use loki_core::{AppInfo, AppTarget, LokiError, LokiResult};
use std::process::Command;
use tracing::{debug, warn};

/// Launch an application by bundle ID or path.
///
/// Uses `open` CLI for reliability — it handles activation, Gatekeeper, and
/// LaunchServices registration. The DesktopDriver trait hides this detail.
pub fn launch_app(target: &AppTarget, args: &[String]) -> LokiResult<()> {
    let mut cmd = Command::new("open");

    match target {
        AppTarget::BundleId(bid) => {
            cmd.args(["-b", bid]);
        }
        AppTarget::Path(path) => {
            cmd.arg(path);
        }
        AppTarget::Pid(_) => {
            return Err(LokiError::LaunchFailed(
                "cannot launch by PID — use bundle ID or path".into(),
            ));
        }
    }

    // Pass through extra arguments after --
    if !args.is_empty() {
        cmd.arg("--args");
        cmd.args(args);
    }

    debug!(?target, "launching app");

    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LokiError::LaunchFailed(stderr.trim().to_string()));
    }

    Ok(())
}

/// Kill an application by bundle ID, name, or PID.
///
/// For PID: sends SIGTERM (or SIGKILL if force).
/// For bundle ID: finds PID via `pgrep -f` or window list, then signals.
pub fn kill_app(target: &AppTarget, force: bool) -> LokiResult<()> {
    let pids = resolve_pids(target)?;

    if pids.is_empty() {
        return Err(LokiError::AppNotFound(format!("{target:?}")));
    }

    let signal = if force { "KILL" } else { "TERM" };

    for pid in &pids {
        debug!(pid, signal, "killing process");
        let output = Command::new("kill")
            .args([&format!("-{signal}"), &pid.to_string()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(pid, %stderr, "kill failed");
        }
    }

    Ok(())
}

/// Check if an app is running.
pub fn app_is_running(target: &AppTarget) -> LokiResult<bool> {
    let pids = resolve_pids(target)?;
    Ok(!pids.is_empty())
}

/// Get info about a running application.
pub fn get_app_info(target: &AppTarget) -> LokiResult<AppInfo> {
    let pids = resolve_pids(target)?;
    let pid = pids
        .first()
        .ok_or_else(|| LokiError::AppNotFound(format!("{target:?}")))?;

    let name = process_name(*pid).unwrap_or_else(|| format!("pid:{pid}"));
    let bundle_id = bundle_id_for_pid(*pid);
    let is_active = is_frontmost(*pid);

    Ok(AppInfo {
        pid: *pid,
        bundle_id,
        name,
        is_active,
    })
}

/// Resolve an AppTarget to one or more PIDs.
fn resolve_pids(target: &AppTarget) -> LokiResult<Vec<u32>> {
    match target {
        AppTarget::Pid(pid) => {
            // Check if process exists
            let output = Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()?;
            if output.status.success() {
                Ok(vec![*pid])
            } else {
                Ok(vec![])
            }
        }
        AppTarget::BundleId(bid) => pids_for_bundle_id(bid),
        AppTarget::Path(path) => {
            // Extract bundle ID from the app path via mdls, or app name from path
            if let Some(bid) = bundle_id_from_path(path) {
                pids_for_bundle_id(&bid)
            } else {
                Ok(vec![])
            }
        }
    }
}

/// Get PIDs for a bundle identifier using lsappinfo.
fn pids_for_bundle_id(bundle_id: &str) -> LokiResult<Vec<u32>> {
    let output = Command::new("lsappinfo")
        .args(["info", "-only", "pid", "-app", bundle_id])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    // Output format: "pid"=12345  or  "pid" = 12345
    let mut pids = Vec::new();
    for line in text.lines() {
        if let Some(val) = line.split('=').nth(1) {
            if let Ok(pid) = val.trim().trim_matches('"').parse::<u32>() {
                if pid > 0 {
                    pids.push(pid);
                }
            }
        }
    }

    Ok(pids)
}

/// Get the bundle ID from an .app path using mdls.
fn bundle_id_from_path(path: &std::path::Path) -> Option<String> {
    let output = Command::new("mdls")
        .args(["-name", "kMDItemCFBundleIdentifier", "-raw"])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() || text == "(null)" {
        None
    } else {
        Some(text)
    }
}

/// Get the process name for a PID.
fn process_name(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // ps returns the full path; extract just the binary name
    let name = name.rsplit('/').next().unwrap_or(&name).to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Get the bundle ID for a running process.
fn bundle_id_for_pid(pid: u32) -> Option<String> {
    let output = Command::new("lsappinfo")
        .args(["info", "-only", "bundleid", "-pid", &pid.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(val) = line.split('=').nth(1) {
            let val = val.trim().trim_matches('"');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Check if a PID is the frontmost application.
fn is_frontmost(pid: u32) -> bool {
    let output = Command::new("lsappinfo")
        .args(["info", "-only", "StatusLabel", "-pid", &pid.to_string()])
        .output()
        .ok();

    match output {
        Some(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            // The frontmost app has StatusLabel with "label"="LSFrontApplication"
            text.contains("LSFrontApplication")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_target_parse_pid() {
        match AppTarget::parse("1234") {
            AppTarget::Pid(pid) => assert_eq!(pid, 1234),
            other => panic!("expected Pid, got {other:?}"),
        }
    }

    #[test]
    fn test_app_target_parse_path() {
        match AppTarget::parse("/Applications/TextEdit.app") {
            AppTarget::Path(p) => assert_eq!(p.to_str().unwrap(), "/Applications/TextEdit.app"),
            other => panic!("expected Path, got {other:?}"),
        }
    }

    #[test]
    fn test_app_target_parse_path_relative() {
        match AppTarget::parse("TextEdit.app") {
            AppTarget::Path(p) => assert_eq!(p.to_str().unwrap(), "TextEdit.app"),
            other => panic!("expected Path, got {other:?}"),
        }
    }

    #[test]
    fn test_app_target_parse_bundle_id() {
        match AppTarget::parse("com.apple.TextEdit") {
            AppTarget::BundleId(bid) => assert_eq!(bid, "com.apple.TextEdit"),
            other => panic!("expected BundleId, got {other:?}"),
        }
    }
}
