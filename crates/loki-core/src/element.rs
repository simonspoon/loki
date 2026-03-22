use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How to identify an application for launch/kill/info operations.
#[derive(Debug, Clone)]
pub enum AppTarget {
    BundleId(String),
    Path(PathBuf),
    Pid(u32),
}

impl AppTarget {
    /// Auto-detect the target type from a CLI string.
    /// - If the string parses as a number, treat as PID.
    /// - If it contains '/' or ends with ".app", treat as path.
    /// - Otherwise, treat as bundle ID.
    pub fn parse(s: &str) -> Self {
        if let Ok(pid) = s.parse::<u32>() {
            AppTarget::Pid(pid)
        } else if s.contains('/') || s.ends_with(".app") {
            AppTarget::Path(PathBuf::from(s))
        } else {
            AppTarget::BundleId(s.to_string())
        }
    }
}

/// Bounding rectangle for a window or UI element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Information about an on-screen window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub window_id: u32,
    pub pid: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub frame: ElementFrame,
    pub is_on_screen: bool,
}

/// Information about a running application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub name: String,
    pub is_active: bool,
}

/// Lightweight reference to a window by ID and owning PID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRef {
    pub window_id: u32,
    pub pid: u32,
}

/// Full accessibility tree node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXElement {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<ElementFrame>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub focused: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub path: Vec<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<AXElement>,
}

/// Lightweight reference to an element within a window's accessibility tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRef {
    pub window: WindowRef,
    pub path: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AppTarget::parse tests ──

    #[test]
    fn test_parse_numeric_as_pid() {
        match AppTarget::parse("1234") {
            AppTarget::Pid(pid) => assert_eq!(pid, 1234),
            other => panic!("expected Pid, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_zero_as_pid() {
        match AppTarget::parse("0") {
            AppTarget::Pid(pid) => assert_eq!(pid, 0),
            other => panic!("expected Pid(0), got {other:?}"),
        }
    }

    #[test]
    fn test_parse_absolute_path() {
        match AppTarget::parse("/Applications/TextEdit.app") {
            AppTarget::Path(p) => assert_eq!(p.to_str().unwrap(), "/Applications/TextEdit.app"),
            other => panic!("expected Path, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_relative_dot_app() {
        match AppTarget::parse("TextEdit.app") {
            AppTarget::Path(p) => assert_eq!(p.to_str().unwrap(), "TextEdit.app"),
            other => panic!("expected Path, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_path_with_slash() {
        match AppTarget::parse("some/path/to/app") {
            AppTarget::Path(p) => assert_eq!(p.to_str().unwrap(), "some/path/to/app"),
            other => panic!("expected Path, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_bundle_id() {
        match AppTarget::parse("com.apple.TextEdit") {
            AppTarget::BundleId(bid) => assert_eq!(bid, "com.apple.TextEdit"),
            other => panic!("expected BundleId, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_plain_name_as_bundle_id() {
        // A plain name without dots, slashes, or .app is treated as bundle ID
        match AppTarget::parse("Calculator") {
            AppTarget::BundleId(bid) => assert_eq!(bid, "Calculator"),
            other => panic!("expected BundleId, got {other:?}"),
        }
    }

    // ── ElementFrame serialization ──

    #[test]
    fn test_element_frame_roundtrip() {
        let frame = ElementFrame {
            x: 10.5,
            y: 20.0,
            width: 800.0,
            height: 600.0,
        };
        let json = serde_json::to_string(&frame).unwrap();
        let parsed: ElementFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.x, 10.5);
        assert_eq!(parsed.width, 800.0);
    }

    // ── WindowInfo serialization ──

    #[test]
    fn test_window_info_json_omits_none_bundle_id() {
        let info = WindowInfo {
            window_id: 1,
            pid: 100,
            title: "Test".to_string(),
            bundle_id: None,
            frame: ElementFrame {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            is_on_screen: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("bundle_id"));
    }

    #[test]
    fn test_window_info_json_includes_bundle_id() {
        let info = WindowInfo {
            window_id: 1,
            pid: 100,
            title: "Test".to_string(),
            bundle_id: Some("com.test.app".to_string()),
            frame: ElementFrame {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            is_on_screen: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("com.test.app"));
    }

    // ── AXElement serialization ──

    #[test]
    fn test_ax_element_empty_children_omitted() {
        let el = AXElement {
            role: "AXButton".to_string(),
            subrole: None,
            title: Some("OK".to_string()),
            value: None,
            description: None,
            identifier: None,
            frame: None,
            enabled: true,
            focused: false,
            path: vec![],
            children: vec![],
        };
        let json = serde_json::to_string(&el).unwrap();
        assert!(!json.contains("children"));
        assert!(!json.contains("path"));
    }
}
