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
