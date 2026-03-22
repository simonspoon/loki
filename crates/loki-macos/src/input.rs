//! Mouse and keyboard input via Core Graphics CGEvent API.

use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField, KeyCode,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use loki_core::{LokiError, LokiResult};
use std::thread;
use std::time::Duration;

/// Small delay between down/up events for reliability.
const CLICK_DELAY: Duration = Duration::from_millis(15);
/// Small delay between typed characters.
const TYPE_DELAY: Duration = Duration::from_millis(8);

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

/// Type a string character by character using CGEvent Unicode string injection.
pub fn type_text(text: &str) -> LokiResult<()> {
    let source = event_source()?;

    for ch in text.chars() {
        let s = ch.to_string();

        // Use keycode 0 as a dummy — the Unicode string override takes precedence.
        let down = CGEvent::new_keyboard_event(source.clone(), 0, true)
            .map_err(|()| LokiError::InputError("failed to create key event".into()))?;
        down.set_string(&s);

        let up = CGEvent::new_keyboard_event(source.clone(), 0, false)
            .map_err(|()| LokiError::InputError("failed to create key event".into()))?;

        down.post(CGEventTapLocation::HID);
        thread::sleep(TYPE_DELAY);
        up.post(CGEventTapLocation::HID);
        thread::sleep(TYPE_DELAY);
    }

    Ok(())
}

/// Send a key combination like "cmd+s", "ctrl+shift+a", "enter", "tab".
pub fn send_key_combo(combo: &str) -> LokiResult<()> {
    let (keycode, flags) = parse_combo(combo)?;
    let source = event_source()?;

    let down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
        .map_err(|()| LokiError::InputError("failed to create key event".into()))?;
    let up = CGEvent::new_keyboard_event(source, keycode, false)
        .map_err(|()| LokiError::InputError("failed to create key event".into()))?;

    if !flags.is_empty() {
        down.set_flags(flags);
        up.set_flags(flags);
    }

    down.post(CGEventTapLocation::HID);
    thread::sleep(CLICK_DELAY);
    up.post(CGEventTapLocation::HID);

    Ok(())
}

/// Parse a combo string like "cmd+shift+a" into (keycode, modifier_flags).
fn parse_combo(combo: &str) -> LokiResult<(u16, CGEventFlags)> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err(LokiError::InputError("empty key combo".into()));
    }

    let mut flags = CGEventFlags::CGEventFlagNull;
    let mut key_part = None;

    for part in &parts {
        let lower = part.trim().to_lowercase();
        match lower.as_str() {
            "cmd" | "command" | "super" => flags |= CGEventFlags::CGEventFlagCommand,
            "shift" => flags |= CGEventFlags::CGEventFlagShift,
            "ctrl" | "control" => flags |= CGEventFlags::CGEventFlagControl,
            "alt" | "option" | "opt" => flags |= CGEventFlags::CGEventFlagAlternate,
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

    let keycode = name_to_keycode(&key_name)
        .ok_or_else(|| LokiError::InputError(format!("unknown key: {key_name}")))?;

    Ok((keycode, flags))
}

/// Map a key name to a macOS virtual keycode.
fn name_to_keycode(name: &str) -> Option<u16> {
    // Single character — letter or digit
    if name.len() == 1 {
        let ch = name.chars().next().unwrap();
        return match ch {
            'a' => Some(KeyCode::ANSI_A),
            'b' => Some(KeyCode::ANSI_B),
            'c' => Some(KeyCode::ANSI_C),
            'd' => Some(KeyCode::ANSI_D),
            'e' => Some(KeyCode::ANSI_E),
            'f' => Some(KeyCode::ANSI_F),
            'g' => Some(KeyCode::ANSI_G),
            'h' => Some(KeyCode::ANSI_H),
            'i' => Some(KeyCode::ANSI_I),
            'j' => Some(KeyCode::ANSI_J),
            'k' => Some(KeyCode::ANSI_K),
            'l' => Some(KeyCode::ANSI_L),
            'm' => Some(KeyCode::ANSI_M),
            'n' => Some(KeyCode::ANSI_N),
            'o' => Some(KeyCode::ANSI_O),
            'p' => Some(KeyCode::ANSI_P),
            'q' => Some(KeyCode::ANSI_Q),
            'r' => Some(KeyCode::ANSI_R),
            's' => Some(KeyCode::ANSI_S),
            't' => Some(KeyCode::ANSI_T),
            'u' => Some(KeyCode::ANSI_U),
            'v' => Some(KeyCode::ANSI_V),
            'w' => Some(KeyCode::ANSI_W),
            'x' => Some(KeyCode::ANSI_X),
            'y' => Some(KeyCode::ANSI_Y),
            'z' => Some(KeyCode::ANSI_Z),
            '0' => Some(KeyCode::ANSI_0),
            '1' => Some(KeyCode::ANSI_1),
            '2' => Some(KeyCode::ANSI_2),
            '3' => Some(KeyCode::ANSI_3),
            '4' => Some(KeyCode::ANSI_4),
            '5' => Some(KeyCode::ANSI_5),
            '6' => Some(KeyCode::ANSI_6),
            '7' => Some(KeyCode::ANSI_7),
            '8' => Some(KeyCode::ANSI_8),
            '9' => Some(KeyCode::ANSI_9),
            '-' => Some(KeyCode::ANSI_MINUS),
            '=' => Some(KeyCode::ANSI_EQUAL),
            '[' => Some(KeyCode::ANSI_LEFT_BRACKET),
            ']' => Some(KeyCode::ANSI_RIGHT_BRACKET),
            ';' => Some(KeyCode::ANSI_SEMICOLON),
            '\'' => Some(KeyCode::ANSI_QUOTE),
            '\\' => Some(KeyCode::ANSI_BACKSLASH),
            ',' => Some(KeyCode::ANSI_COMMA),
            '.' => Some(KeyCode::ANSI_PERIOD),
            '/' => Some(KeyCode::ANSI_SLASH),
            '`' => Some(KeyCode::ANSI_GRAVE),
            _ => None,
        };
    }

    // Named keys
    match name {
        "return" | "enter" => Some(KeyCode::RETURN),
        "tab" => Some(KeyCode::TAB),
        "space" => Some(KeyCode::SPACE),
        "delete" | "backspace" => Some(KeyCode::DELETE),
        "escape" | "esc" => Some(KeyCode::ESCAPE),
        "up" => Some(KeyCode::UP_ARROW),
        "down" => Some(KeyCode::DOWN_ARROW),
        "left" => Some(KeyCode::LEFT_ARROW),
        "right" => Some(KeyCode::RIGHT_ARROW),
        "home" => Some(KeyCode::HOME),
        "end" => Some(KeyCode::END),
        "pageup" => Some(KeyCode::PAGE_UP),
        "pagedown" => Some(KeyCode::PAGE_DOWN),
        "forwarddelete" => Some(KeyCode::FORWARD_DELETE),
        "f1" => Some(KeyCode::F1),
        "f2" => Some(KeyCode::F2),
        "f3" => Some(KeyCode::F3),
        "f4" => Some(KeyCode::F4),
        "f5" => Some(KeyCode::F5),
        "f6" => Some(KeyCode::F6),
        "f7" => Some(KeyCode::F7),
        "f8" => Some(KeyCode::F8),
        "f9" => Some(KeyCode::F9),
        "f10" => Some(KeyCode::F10),
        "f11" => Some(KeyCode::F11),
        "f12" => Some(KeyCode::F12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_combo_single_key() {
        let (keycode, flags) = parse_combo("a").unwrap();
        assert_eq!(keycode, KeyCode::ANSI_A);
        assert!(flags.is_empty() || flags == CGEventFlags::CGEventFlagNull);
    }

    #[test]
    fn test_parse_combo_cmd_key() {
        let (keycode, flags) = parse_combo("cmd+s").unwrap();
        assert_eq!(keycode, KeyCode::ANSI_S);
        assert!(flags.contains(CGEventFlags::CGEventFlagCommand));
    }

    #[test]
    fn test_parse_combo_multiple_modifiers() {
        let (keycode, flags) = parse_combo("ctrl+shift+a").unwrap();
        assert_eq!(keycode, KeyCode::ANSI_A);
        assert!(flags.contains(CGEventFlags::CGEventFlagControl));
        assert!(flags.contains(CGEventFlags::CGEventFlagShift));
    }

    #[test]
    fn test_parse_combo_special_key() {
        let (keycode, flags) = parse_combo("cmd+enter").unwrap();
        assert_eq!(keycode, KeyCode::RETURN);
        assert!(flags.contains(CGEventFlags::CGEventFlagCommand));
    }

    #[test]
    fn test_parse_combo_modifier_aliases() {
        let (_, flags) = parse_combo("command+a").unwrap();
        assert!(flags.contains(CGEventFlags::CGEventFlagCommand));

        let (_, flags) = parse_combo("control+a").unwrap();
        assert!(flags.contains(CGEventFlags::CGEventFlagControl));

        let (_, flags) = parse_combo("option+a").unwrap();
        assert!(flags.contains(CGEventFlags::CGEventFlagAlternate));

        let (_, flags) = parse_combo("alt+a").unwrap();
        assert!(flags.contains(CGEventFlags::CGEventFlagAlternate));

        let (_, flags) = parse_combo("opt+a").unwrap();
        assert!(flags.contains(CGEventFlags::CGEventFlagAlternate));

        let (_, flags) = parse_combo("super+a").unwrap();
        assert!(flags.contains(CGEventFlags::CGEventFlagCommand));
    }

    #[test]
    fn test_parse_combo_named_keys() {
        assert_eq!(parse_combo("tab").unwrap().0, KeyCode::TAB);
        assert_eq!(parse_combo("space").unwrap().0, KeyCode::SPACE);
        assert_eq!(parse_combo("escape").unwrap().0, KeyCode::ESCAPE);
        assert_eq!(parse_combo("esc").unwrap().0, KeyCode::ESCAPE);
        assert_eq!(parse_combo("delete").unwrap().0, KeyCode::DELETE);
        assert_eq!(parse_combo("backspace").unwrap().0, KeyCode::DELETE);
        assert_eq!(parse_combo("up").unwrap().0, KeyCode::UP_ARROW);
        assert_eq!(parse_combo("down").unwrap().0, KeyCode::DOWN_ARROW);
        assert_eq!(parse_combo("left").unwrap().0, KeyCode::LEFT_ARROW);
        assert_eq!(parse_combo("right").unwrap().0, KeyCode::RIGHT_ARROW);
        assert_eq!(parse_combo("f1").unwrap().0, KeyCode::F1);
        assert_eq!(parse_combo("f12").unwrap().0, KeyCode::F12);
    }

    #[test]
    fn test_parse_combo_digit_keys() {
        assert_eq!(parse_combo("1").unwrap().0, KeyCode::ANSI_1);
        assert_eq!(parse_combo("0").unwrap().0, KeyCode::ANSI_0);
    }

    #[test]
    fn test_parse_combo_unknown_key() {
        assert!(parse_combo("cmd+unicorn").is_err());
    }

    #[test]
    fn test_parse_combo_empty() {
        assert!(parse_combo("").is_err());
    }

    #[test]
    fn test_parse_combo_no_key_only_modifier() {
        assert!(parse_combo("cmd").is_err());
    }

    #[test]
    fn test_parse_combo_multiple_non_modifier_keys() {
        assert!(parse_combo("a+b").is_err());
    }

    #[test]
    fn test_name_to_keycode_letters() {
        // Verify a few key mappings are correct (not sequential like ASCII)
        assert_eq!(name_to_keycode("a"), Some(0x00));
        assert_eq!(name_to_keycode("s"), Some(0x01));
        assert_eq!(name_to_keycode("z"), Some(0x06));
        assert_eq!(name_to_keycode("q"), Some(0x0C));
    }
}
