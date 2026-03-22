use serde::{Deserialize, Serialize};

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
