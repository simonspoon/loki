use assert_cmd::Command;
use predicates::prelude::*;

fn loki() -> Command {
    Command::cargo_bin("loki").unwrap()
}

// ── Help output ──

#[test]
fn test_help_flag() {
    loki()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Desktop app automation for QA testing",
        ))
        .stdout(predicate::str::contains("windows"))
        .stdout(predicate::str::contains("check-permission"))
        .stdout(predicate::str::contains("screenshot"));
}

#[test]
fn test_help_subcommand() {
    loki()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Desktop app automation for QA testing",
        ));
}

#[test]
fn test_windows_help() {
    loki()
        .args(["windows", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List open windows"));
}

// ── Invalid usage ──

#[test]
fn test_no_subcommand() {
    loki()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_invalid_subcommand() {
    loki()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_invalid_format() {
    loki()
        .args(["--format", "xml", "windows"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

// ── check-permission ──
// This command doesn't require accessibility permission to run — it just checks.

#[test]
fn test_check_permission_text() {
    loki()
        .arg("check-permission")
        .assert()
        .success()
        .stdout(predicate::str::contains("Accessibility permission:"));
}

#[test]
fn test_check_permission_json() {
    loki()
        .args(["--format", "json", "check-permission"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"granted\""));
}

// ── windows ──
// list_windows uses Core Graphics, which works without accessibility permission.

#[test]
fn test_windows_text() {
    loki().arg("windows").assert().success();
}

#[test]
fn test_windows_json() {
    loki()
        .args(["--format", "json", "windows"])
        .assert()
        .success();
}

#[test]
fn test_windows_with_filter() {
    // Filter by a non-existent bundle ID — should succeed with empty output
    loki()
        .args(["windows", "--bundle-id", "com.nonexistent.fake.app.12345"])
        .assert()
        .success();
}

// ── completions ──

#[test]
fn test_completions_zsh() {
    loki()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("loki"));
}

#[test]
fn test_completions_bash() {
    loki()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("loki"));
}

// ── Commands that require arguments ──

#[test]
fn test_tree_missing_window_id() {
    loki()
        .arg("tree")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_find_missing_window_id() {
    loki()
        .arg("find")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_click_missing_coords() {
    loki()
        .arg("click")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_launch_missing_target() {
    loki()
        .arg("launch")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_kill_missing_target() {
    loki()
        .arg("kill")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_key_missing_combo() {
    loki()
        .arg("key")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_type_missing_text() {
    loki()
        .arg("type")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ── Version ──

#[test]
fn test_version_flag() {
    loki()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("loki"));
}

// ── app-info flags ──

#[test]
fn test_app_info_no_args() {
    // No target, --pid, or --bundle-id should fail
    loki().arg("app-info").assert().failure();
}

#[test]
fn test_app_info_with_pid_flag() {
    // Invalid PID should fail with app not found
    loki()
        .args(["app-info", "--pid", "99999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("app not found"));
}

// ── PID validation for type/key/click ──

#[test]
fn test_type_invalid_pid_fails() {
    loki()
        .args(["type", "hello", "--pid", "99999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("99999"));
}

#[test]
fn test_key_invalid_pid_fails() {
    loki()
        .args(["key", "cmd+a", "--pid", "99999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("99999"));
}

#[test]
fn test_click_invalid_pid_fails() {
    loki()
        .args(["click", "100", "100", "--pid", "99999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("99999"));
}

// ── Accessibility-dependent tests ──
// These require accessibility permission and are skipped by default.

#[test]
#[ignore]
fn test_tree_with_real_window() {
    // Requires a running app with accessibility permission
    let output = loki()
        .args(["--format", "json", "windows"])
        .output()
        .unwrap();
    let windows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    if let Some(first) = windows.as_array().and_then(|a| a.first()) {
        let wid = first["window_id"].as_u64().unwrap();
        loki()
            .args(["tree", &wid.to_string(), "--depth", "2"])
            .assert()
            .success();
    }
}

#[test]
#[ignore]
fn test_find_with_real_window() {
    let output = loki()
        .args(["--format", "json", "windows"])
        .output()
        .unwrap();
    let windows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    if let Some(first) = windows.as_array().and_then(|a| a.first()) {
        let wid = first["window_id"].as_u64().unwrap();
        loki()
            .args(["find", &wid.to_string(), "--role", "AXButton"])
            .assert()
            .success();
    }
}

#[test]
#[ignore]
fn test_click_element_activates_app() {
    // Regression test: click-element must activate the target app before clicking.
    // Without activation, CGEvent clicks land on whatever window is in the foreground.
    let output = loki()
        .args(["--format", "json", "windows"])
        .output()
        .unwrap();
    let windows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    if let Some(first) = windows.as_array().and_then(|a| a.first()) {
        let wid = first["window_id"].as_u64().unwrap();
        // Find any button and click it — this exercises the activate+click path
        loki()
            .args(["click-element", &wid.to_string(), "--role", "AXButton"])
            .assert()
            .success();
    }
}

#[test]
#[ignore]
fn test_click_with_pid_activates_app() {
    // Regression test: click --pid must activate the target app before clicking.
    let output = loki()
        .args(["--format", "json", "windows"])
        .output()
        .unwrap();
    let windows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    if let Some(first) = windows.as_array().and_then(|a| a.first()) {
        let pid = first["pid"].as_u64().unwrap();
        let frame = &first["frame"];
        let x = frame["x"].as_f64().unwrap() + frame["width"].as_f64().unwrap() / 2.0;
        let y = frame["y"].as_f64().unwrap() + frame["height"].as_f64().unwrap() / 2.0;
        loki()
            .args([
                "click",
                &x.to_string(),
                &y.to_string(),
                "--pid",
                &pid.to_string(),
            ])
            .assert()
            .success();
    }
}
