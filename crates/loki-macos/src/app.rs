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
///
/// `lsappinfo -app` is case-sensitive, but `open -b` and users often use
/// a different case than what macOS registers. We try exact match first,
/// then fall back to a case-insensitive scan of all running apps.
fn pids_for_bundle_id(bundle_id: &str) -> LokiResult<Vec<u32>> {
    // Try exact match first (fast path)
    if let Ok(pids) = pids_for_bundle_id_exact(bundle_id) {
        if !pids.is_empty() {
            return Ok(pids);
        }
    }

    // Fall back to case-insensitive scan of running apps
    pids_for_bundle_id_scan(bundle_id)
}

fn pids_for_bundle_id_exact(bundle_id: &str) -> LokiResult<Vec<u32>> {
    let output = Command::new("lsappinfo")
        .args(["info", "-only", "pid", "-app", bundle_id])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    Ok(parse_pid_output(&String::from_utf8_lossy(&output.stdout)))
}

fn pids_for_bundle_id_scan(bundle_id: &str) -> LokiResult<Vec<u32>> {
    let output = Command::new("lsappinfo").arg("list").output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let bid_lower = bundle_id.to_ascii_lowercase();
    let mut pids = Vec::new();

    // lsappinfo list output has blocks per app. Each block has lines like:
    //   bundleID="com.apple.calculator"
    //   pid = 42446 type="Foreground" ...
    // Blocks are separated by blank lines or new app headers.
    let mut current_pid: Option<u32> = None;
    let mut current_bid_matches = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(val) = extract_quoted_value(line, "bundleID") {
            current_bid_matches = val.to_ascii_lowercase() == bid_lower;
        }

        // Match pid lines: "pid = 42446 ..." or "pid"=42446
        if trimmed.starts_with("pid") || trimmed.starts_with("\"pid\"") {
            if let Some(pid) = extract_pid_value(line) {
                current_pid = Some(pid);
            }
        }

        // New app block starts with number+) like "103) "Calculator" ASN:..."
        // or blank line signals end of block
        let is_block_boundary = trimmed.is_empty()
            || (trimmed.len() > 2
                && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
                && trimmed.contains(')'));

        if is_block_boundary && current_bid_matches {
            if let Some(pid) = current_pid {
                pids.push(pid);
            }
            current_pid = None;
            current_bid_matches = false;
        } else if is_block_boundary {
            current_pid = None;
            current_bid_matches = false;
        }
    }
    // Handle last block
    if current_bid_matches {
        if let Some(pid) = current_pid {
            pids.push(pid);
        }
    }

    Ok(pids)
}

fn extract_quoted_value(line: &str, key: &str) -> Option<String> {
    // Match patterns like:
    //   bundleID="com.apple.calculator"      (no quotes on key)
    //   "bundleID"="com.apple.calculator"    (quotes on key)
    //   "pid"=42446                          (quotes on key)
    let trimmed = line.trim();
    // Check both quoted and unquoted key forms
    let has_key = trimmed.starts_with(&format!("{key}="))
        || trimmed.starts_with(&format!("{key} ="))
        || trimmed.starts_with(&format!("\"{key}\"="))
        || trimmed.starts_with(&format!("\"{key}\" ="));
    if !has_key {
        return None;
    }
    let after_eq = trimmed.split('=').nth(1)?;
    let val = after_eq.trim().trim_matches('"');
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

fn extract_pid_value(line: &str) -> Option<u32> {
    // Match lines like:  pid = 42446 type="Foreground"...
    // or:  "pid"=42446
    if !line.contains("pid") {
        return None;
    }
    let after_eq = line.split('=').nth(1)?;
    // Take only the first whitespace-delimited token after '='
    let token = after_eq.split_whitespace().next()?;
    token
        .trim_matches('"')
        .parse::<u32>()
        .ok()
        .filter(|&p| p > 0)
}

fn parse_pid_output(text: &str) -> Vec<u32> {
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
    pids
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

/// Bring an application to the foreground by PID.
pub fn activate_app(pid: u32) -> LokiResult<()> {
    let output = Command::new("osascript")
        .args([
            "-e",
            &format!(
                "tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"
            ),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(pid, %stderr, "failed to activate app");
    }

    // Give macOS a moment to complete the activation
    std::thread::sleep(std::time::Duration::from_millis(100));
    Ok(())
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
