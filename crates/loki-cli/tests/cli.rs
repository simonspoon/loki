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

#[test]
fn test_find_help_shows_label() {
    loki()
        .args(["find", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--label"));
}

#[test]
fn test_click_element_help_shows_label() {
    loki()
        .args(["click-element", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--label"));
}

#[test]
fn test_drag_help_documents_activation_gotcha() {
    loki()
        .args(["drag", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--steps"))
        .stdout(predicate::str::contains("--delay"))
        // The silent-no-op trap this command exists to remove: a raw mouse
        // event doesn't activate the app, so --pid/--window is load-bearing.
        .stdout(predicate::str::contains("does NOT activate"));
}

#[test]
fn test_drag_requires_four_coordinates() {
    loki()
        .args(["drag", "100", "200", "300"])
        .assert()
        .failure();
}

// A display left of / above the primary has a negative origin, so these are real
// coordinates — clap would otherwise parse "-5" as an unknown flag and refuse.
#[test]
fn test_drag_accepts_negative_coordinates() {
    loki()
        .args(["drag", "-5", "-20", "100", "200", "--help"])
        .assert()
        .success();
}

#[test]
fn test_click_accepts_negative_coordinates() {
    loki()
        .args(["click", "-5", "-20", "--help"])
        .assert()
        .success();
}

#[test]
fn test_drag_rejects_non_numeric_coordinates() {
    loki()
        .args(["drag", "100", "200", "left", "200"])
        .assert()
        .failure();
}

#[test]
fn test_wheel_help_documents_activation_and_sign() {
    loki()
        .args(["wheel", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--steps"))
        // Same silent-no-op trap as drag: a raw wheel event carries no activation.
        .stdout(predicate::str::contains("does NOT activate"))
        // Sign convention is the thing a caller gets wrong first.
        .stdout(predicate::str::contains("scrolls down"));
}

#[test]
fn test_wheel_requires_delta() {
    loki().args(["wheel", "640", "400"]).assert().failure();
}

// `-800,0` is scroll-left. The command's `allow_negative_numbers` does NOT cover
// it — that only admits a leading '-' on a token parsing as a number — so clap
// read it as the flag `-8` and scroll-left was unreachable. Guards the fix
// (`allow_hyphen_values` on `delta`); a unit test of the parser alone never sees
// this, because clap rejects the token before the parser is ever called.
#[test]
fn test_wheel_accepts_negative_dx() {
    loki()
        .args(["wheel", "640", "400", "-800,0", "--help"])
        .assert()
        .success();
}

#[test]
fn test_wheel_accepts_both_deltas_negative() {
    loki()
        .args(["wheel", "640", "400", "-800,-300", "--help"])
        .assert()
        .success();
}

// allow_hyphen_values on the delta must not start swallowing the flags after it.
#[test]
fn test_wheel_flags_after_negative_delta_still_parse() {
    loki()
        .args(["wheel", "640", "400", "-800,0", "--steps", "4", "--help"])
        .assert()
        .success();
}

// `--relative` is only useful if the *CLI* accepts it on all three coordinate
// commands — a unit test of the offset helper passes even when clap rejects the
// flag before that helper is ever called (the trap `-800,0` fell into above).
// These drive the shipped binary: no target means the origin can't be resolved,
// so each exits 1 with the actionable message and posts no event.
#[test]
fn test_click_relative_without_target_fails() {
    loki()
        .args(["click", "10", "10", "--relative"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--relative needs a frame"));
}

#[test]
fn test_drag_relative_without_target_fails() {
    loki()
        .args(["drag", "10", "10", "20", "20", "--relative"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--relative needs a frame"));
}

#[test]
fn test_wheel_relative_without_target_fails() {
    loki()
        .args(["wheel", "10", "10", "0,300", "--relative"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--relative needs a frame"));
}

#[test]
fn test_relative_documented_on_all_three_commands() {
    for cmd in ["click", "drag", "wheel"] {
        loki()
            .args([cmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--relative"));
    }
}

#[test]
fn test_wheel_rejects_bare_number_delta() {
    loki()
        .args(["wheel", "640", "400", "300"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("dX,dY"));
}

#[test]
fn test_wheel_rejects_non_numeric_delta() {
    loki()
        .args(["wheel", "640", "400", "0,down"])
        .assert()
        .failure();
}

#[test]
fn test_wait_for_help_shows_label() {
    loki()
        .args(["wait-for", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--label"));
}

#[test]
fn test_wait_gone_help_shows_label() {
    loki()
        .args(["wait-gone", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--label"));
}

#[test]
fn test_find_label_flag_parses() {
    // Window 0 doesn't exist, so the command must fail — but it must fail
    // because of the missing window, NOT because --label is unknown to clap.
    loki()
        .args(["find", "0", "--label", "Settings"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--label").not());
}

// ── wait-window diagnostics (mesa 530) ──

#[test]
fn test_wait_window_timeout_explains_itself() {
    // A bare "timed out after Nms" made a slow launch look like a matching bug.
    // The timeout must name the glob it matched and stay on exit code 3.
    loki()
        .args([
            "wait-window",
            "--title",
            "loki-no-such-window-530",
            "--timeout",
            "200",
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("timed out after 200ms"))
        .stderr(predicate::str::contains(
            "waiting for title glob \"*loki-no-such-window-530*\"",
        ))
        .stderr(predicate::str::contains("seen:"))
        .stderr(predicate::str::contains("--timeout"));
}

// ── Invalid usage ──

#[test]
fn test_menu_state_help() {
    loki()
        .args(["menu-state", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("without pressing it"))
        .stdout(predicate::str::contains("--bundle-id"))
        .stdout(predicate::str::contains("--separator"));
}

#[test]
fn test_menu_state_listed_in_help() {
    loki()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("menu-state"));
}

#[test]
fn test_menu_state_missing_path() {
    loki().arg("menu-state").assert().failure();
}

#[test]
fn test_menu_state_empty_path_rejected() {
    // A path of only separators has no levels to walk — reject it rather than
    // silently targeting the menu bar root.
    loki()
        .args(["menu-state", ">>", "--pid", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty menu path"));
}

// ── Targeting flags ──

/// Every command that takes a target app must accept all three ways of naming
/// one. Asserted at the CLI, not on the resolver: the bug was clap refusing the
/// token before any code of ours ran, so a unit test on the resolver could never
/// have seen it. A bogus bundle ID is deliberate — reaching "app not found"
/// proves the flag parsed and was resolved, without touching a real app.
#[test]
fn test_bundle_id_accepted_wherever_pid_is() {
    let invocations: &[&[&str]] = &[
        &["key", "cmd+s"],
        &["type", "hello"],
        &["click", "10", "10"],
        &["drag", "10", "10", "20", "20"],
        &["wheel", "10", "10", "0,300"],
        &["menu", "File"],
        &["menu-state", "File"],
    ];
    for args in invocations {
        loki()
            .args(*args)
            .args(["--bundle-id", "com.example.NotARealApp"])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("app not found")
                    .and(predicate::str::contains("unexpected argument").not()),
            );
    }
}

/// The documented precedence is --pid, then --window, then --bundle-id. Passing
/// two must resolve through the *earlier* one and fail there rather than falling
/// through to the later flag — the doc and the resolver disagreed on this order
/// once already, and nothing but a test keeps them honest.
#[test]
fn test_window_takes_precedence_over_bundle_id() {
    loki()
        .args([
            "key",
            "cmd+s",
            "--window",
            "99999999",
            "--bundle-id",
            "com.apple.Finder",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("window not found"));
}

#[test]
fn test_pid_takes_precedence_over_bundle_id() {
    loki()
        .args([
            "key",
            "cmd+s",
            "--pid",
            "999999",
            "--bundle-id",
            "com.example.NotARealApp",
        ])
        .assert()
        .failure()
        // Resolution stopped at the PID; the bogus bundle ID was never consulted.
        .stderr(predicate::str::contains("com.example.NotARealApp").not());
}

#[test]
fn test_bundle_id_documented_on_input_commands() {
    for cmd in ["key", "type", "click", "drag", "wheel"] {
        loki()
            .args([cmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--bundle-id"));
    }
}

/// `--relative` resolves its frame origin from whichever targeting flag was
/// given, so naming the app by bundle ID has to work like naming it by PID.
#[test]
fn test_relative_error_offers_bundle_id() {
    loki()
        .args(["click", "10", "10", "--relative"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bundle-id"));
}

/// A wrong targeting flag must say what the command *does* take — the surface
/// isn't uniform, and the bare clap error leaves you guessing mid-script.
#[test]
fn test_wrong_targeting_flag_names_the_accepted_ones() {
    loki()
        .args(["screenshot", "--pid", "123"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "note: `loki screenshot` targets with --window",
        ));
}

#[test]
fn test_targeting_hint_names_a_positional_target() {
    loki()
        .args(["find", "--bundle-id", "com.example.NotARealApp"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "note: `loki find` targets with <WINDOW_ID>",
        ));
}

/// The hint is scoped to targeting flags — an ordinary typo shouldn't grow one.
#[test]
fn test_no_targeting_hint_for_unrelated_flag() {
    loki()
        .args(["key", "cmd+s", "--nonsense"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("note:").not());
}

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
