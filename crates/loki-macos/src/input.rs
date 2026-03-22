//! Mouse and keyboard input.
//!
//! Mouse clicks use Core Graphics CGEvent API (coordinate-based, no focus needed).
//! Keyboard input uses System Events via osascript (reliable regardless of which
//! process has focus).

use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use loki_core::{LokiError, LokiResult};
use std::thread;
use std::time::Duration;

/// Small delay between down/up events for reliability.
const CLICK_DELAY: Duration = Duration::from_millis(15);

fn event_source() -> LokiResult<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|()| LokiError::InputError("failed to create CGEventSource".into()))
}

// ── Mouse ──

/// Click at absolute screen coordinates.
pub fn click_at(x: f64, y: f64) -> LokiResult<()> {
    let point = CGPoint::new(x, y);
    let source = event_source()?;

    let down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|()| LokiError::InputError("failed to create mouse down event".into()))?;

    let up = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|()| LokiError::InputError("failed to create mouse up event".into()))?;

    down.post(CGEventTapLocation::HID);
    thread::sleep(CLICK_DELAY);
    up.post(CGEventTapLocation::HID);

    Ok(())
}

/// Double-click at absolute screen coordinates.
pub fn double_click_at(x: f64, y: f64) -> LokiResult<()> {
    let point = CGPoint::new(x, y);
    let source = event_source()?;

    // First click
    let down1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|()| LokiError::InputError("failed to create mouse event".into()))?;
    down1.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);

    let up1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|()| LokiError::InputError("failed to create mouse event".into()))?;
    up1.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);

    // Second click (click state = 2)
    let down2 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|()| LokiError::InputError("failed to create mouse event".into()))?;
    down2.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 2);

    let up2 =
        CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
            .map_err(|()| LokiError::InputError("failed to create mouse event".into()))?;
    up2.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 2);

    down1.post(CGEventTapLocation::HID);
    thread::sleep(CLICK_DELAY);
    up1.post(CGEventTapLocation::HID);
    thread::sleep(CLICK_DELAY);
    down2.post(CGEventTapLocation::HID);
    thread::sleep(CLICK_DELAY);
    up2.post(CGEventTapLocation::HID);

    Ok(())
}

/// Right-click at absolute screen coordinates.
pub fn right_click_at(x: f64, y: f64) -> LokiResult<()> {
    let point = CGPoint::new(x, y);
    let source = event_source()?;

    let down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::RightMouseDown,
        point,
        CGMouseButton::Right,
    )
    .map_err(|()| LokiError::InputError("failed to create mouse event".into()))?;

    let up = CGEvent::new_mouse_event(
        source,
        CGEventType::RightMouseUp,
        point,
        CGMouseButton::Right,
    )
    .map_err(|()| LokiError::InputError("failed to create mouse event".into()))?;

    down.post(CGEventTapLocation::HID);
    thread::sleep(CLICK_DELAY);
    up.post(CGEventTapLocation::HID);

    Ok(())
}

// ── Keyboard ──

/// Type a string using System Events keystroke via osascript.
///
/// CGEvent keyboard injection is unreliable when the calling process (terminal)
/// retains focus. System Events `keystroke` is the proven approach on macOS —
/// it routes through the accessibility subsystem and works regardless of which
/// process spawned the command.
///
/// The `pid` parameter is unused here (activation is handled by the driver)
/// but kept for API consistency.
pub fn type_text(text: &str, _pid: Option<i32>) -> LokiResult<()> {
    // Escape backslashes and double quotes for AppleScript string
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");

    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            &format!("tell application \"System Events\" to keystroke \"{escaped}\""),
        ])
        .output()
        .map_err(|e| LokiError::InputError(format!("failed to run osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LokiError::InputError(format!(
            "System Events keystroke failed: {stderr}"
        )));
    }

    Ok(())
}

/// Send a key combination like "cmd+s", "ctrl+shift+a", "enter", "tab".
///
/// Uses System Events for reliability (same reason as type_text).
/// The `pid` parameter is unused (activation handled by driver).
pub fn send_key_combo(combo: &str, _pid: Option<i32>) -> LokiResult<()> {
    let (key_name, modifiers) = parse_combo_for_applescript(combo)?;

    let script = if modifiers.is_empty() {
        // Special keys use "key code" syntax
        if let Some(code) = applescript_key_code(&key_name) {
            format!("tell application \"System Events\" to key code {code}")
        } else if key_name.len() == 1 {
            let escaped = key_name.replace('"', "\\\"");
            format!("tell application \"System Events\" to keystroke \"{escaped}\"")
        } else {
            return Err(LokiError::InputError(format!("unknown key: {key_name}")));
        }
    } else {
        let using_clause = modifiers.join(", ");
        if let Some(code) = applescript_key_code(&key_name) {
            format!(
                "tell application \"System Events\" to key code {code} using {{{using_clause}}}"
            )
        } else if key_name.len() == 1 {
            let escaped = key_name.replace('"', "\\\"");
            format!(
                "tell application \"System Events\" to keystroke \"{escaped}\" using {{{using_clause}}}"
            )
        } else {
            return Err(LokiError::InputError(format!("unknown key: {key_name}")));
        }
    };

    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| LokiError::InputError(format!("failed to run osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LokiError::InputError(format!(
            "System Events key combo failed: {stderr}"
        )));
    }

    Ok(())
}

/// Parse a combo string into (key_name, vec_of_applescript_modifiers).
fn parse_combo_for_applescript(combo: &str) -> LokiResult<(String, Vec<String>)> {
    if combo.is_empty() {
        return Err(LokiError::InputError("empty key combo".into()));
    }
    let parts: Vec<&str> = combo.split('+').collect();

    let mut modifiers = Vec::new();
    let mut key_part = None;

    for part in &parts {
        let lower = part.trim().to_lowercase();
        match lower.as_str() {
            "cmd" | "command" | "super" => modifiers.push("command down".to_string()),
            "shift" => modifiers.push("shift down".to_string()),
            "ctrl" | "control" => modifiers.push("control down".to_string()),
            "alt" | "option" | "opt" => modifiers.push("option down".to_string()),
            _ => {
                if key_part.is_some() {
                    return Err(LokiError::InputError(format!(
                        "multiple non-modifier keys in combo: {combo}"
                    )));
                }
                key_part = Some(lower);
            }
        }
    }

    let key_name = key_part
        .ok_or_else(|| LokiError::InputError(format!("no key specified in combo: {combo}")))?;

    Ok((key_name, modifiers))
}

/// Map key names to AppleScript key codes (for non-character keys).
fn applescript_key_code(name: &str) -> Option<u16> {
    match name {
        "return" | "enter" => Some(36),
        "tab" => Some(48),
        "space" => Some(49),
        "delete" | "backspace" => Some(51),
        "escape" | "esc" => Some(53),
        "up" => Some(126),
        "down" => Some(125),
        "left" => Some(123),
        "right" => Some(124),
        "home" => Some(115),
        "end" => Some(119),
        "pageup" => Some(116),
        "pagedown" => Some(121),
        "forwarddelete" => Some(117),
        "f1" => Some(122),
        "f2" => Some(120),
        "f3" => Some(99),
        "f4" => Some(118),
        "f5" => Some(96),
        "f6" => Some(97),
        "f7" => Some(98),
        "f8" => Some(100),
        "f9" => Some(101),
        "f10" => Some(109),
        "f11" => Some(103),
        "f12" => Some(111),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_combo_single_key() {
        let (key, mods) = parse_combo_for_applescript("a").unwrap();
        assert_eq!(key, "a");
        assert!(mods.is_empty());
    }

    #[test]
    fn test_parse_combo_cmd_key() {
        let (key, mods) = parse_combo_for_applescript("cmd+s").unwrap();
        assert_eq!(key, "s");
        assert_eq!(mods, vec!["command down"]);
    }

    #[test]
    fn test_parse_combo_multiple_modifiers() {
        let (key, mods) = parse_combo_for_applescript("ctrl+shift+a").unwrap();
        assert_eq!(key, "a");
        assert!(mods.contains(&"control down".to_string()));
        assert!(mods.contains(&"shift down".to_string()));
    }

    #[test]
    fn test_parse_combo_special_key() {
        let (key, mods) = parse_combo_for_applescript("cmd+enter").unwrap();
        assert_eq!(key, "enter");
        assert_eq!(mods, vec!["command down"]);
    }

    #[test]
    fn test_parse_combo_modifier_aliases() {
        let (_, mods) = parse_combo_for_applescript("command+a").unwrap();
        assert_eq!(mods, vec!["command down"]);

        let (_, mods) = parse_combo_for_applescript("control+a").unwrap();
        assert_eq!(mods, vec!["control down"]);

        let (_, mods) = parse_combo_for_applescript("option+a").unwrap();
        assert_eq!(mods, vec!["option down"]);

        let (_, mods) = parse_combo_for_applescript("alt+a").unwrap();
        assert_eq!(mods, vec!["option down"]);

        let (_, mods) = parse_combo_for_applescript("opt+a").unwrap();
        assert_eq!(mods, vec!["option down"]);

        let (_, mods) = parse_combo_for_applescript("super+a").unwrap();
        assert_eq!(mods, vec!["command down"]);
    }

    #[test]
    fn test_parse_combo_named_keys_have_codes() {
        assert!(applescript_key_code("tab").is_some());
        assert!(applescript_key_code("space").is_some());
        assert!(applescript_key_code("escape").is_some());
        assert!(applescript_key_code("esc").is_some());
        assert!(applescript_key_code("delete").is_some());
        assert!(applescript_key_code("backspace").is_some());
        assert!(applescript_key_code("up").is_some());
        assert!(applescript_key_code("down").is_some());
        assert!(applescript_key_code("left").is_some());
        assert!(applescript_key_code("right").is_some());
        assert!(applescript_key_code("enter").is_some());
        assert!(applescript_key_code("return").is_some());
        assert!(applescript_key_code("f1").is_some());
        assert!(applescript_key_code("f12").is_some());
    }

    #[test]
    fn test_parse_combo_unknown_key() {
        // Unknown multi-char key with no applescript code — send_key_combo will fail
        let (key, _) = parse_combo_for_applescript("cmd+unicorn").unwrap();
        assert_eq!(key, "unicorn");
        assert!(applescript_key_code(&key).is_none());
    }

    #[test]
    fn test_parse_combo_empty() {
        assert!(parse_combo_for_applescript("").is_err());
    }

    #[test]
    fn test_parse_combo_no_key_only_modifier() {
        assert!(parse_combo_for_applescript("cmd").is_err());
    }

    #[test]
    fn test_parse_combo_multiple_non_modifier_keys() {
        assert!(parse_combo_for_applescript("a+b").is_err());
    }
}
