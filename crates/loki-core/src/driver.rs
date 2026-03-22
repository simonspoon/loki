use async_trait::async_trait;

use crate::element::{AXElement, AppInfo, WindowInfo, WindowRef};
use crate::error::LokiResult;
use crate::query::{ElementQuery, WindowFilter};

/// Platform-agnostic interface for desktop automation.
///
/// Phase 1 implementations: `list_windows`, `find_window`,
/// `has_accessibility_permission`, `request_accessibility_permission`.
#[async_trait]
pub trait DesktopDriver: Send + Sync {
    // ── Window discovery ──

    /// List all windows, optionally filtered.
    async fn list_windows(&self, filter: &WindowFilter) -> LokiResult<Vec<WindowInfo>>;

    /// Find the first window matching the filter.
    async fn find_window(&self, filter: &WindowFilter) -> LokiResult<Option<WindowInfo>>;

    // ── App lifecycle ──

    /// Launch an application by bundle ID or path.
    async fn launch_app(&self, target: &str, args: &[String], wait: bool) -> LokiResult<AppInfo>;

    /// Kill an application by bundle ID, name, or PID.
    async fn kill_app(&self, target: &str, force: bool) -> LokiResult<()>;

    /// Get info about a running application.
    async fn app_info(&self, target: &str) -> LokiResult<AppInfo>;

    // ── Accessibility tree ──

    /// Get the full accessibility tree for a window.
    async fn get_tree(&self, window: &WindowRef, max_depth: Option<usize>)
        -> LokiResult<AXElement>;

    /// Find elements matching a query in a window's accessibility tree.
    async fn find_elements(
        &self,
        window: &WindowRef,
        query: &ElementQuery,
    ) -> LokiResult<Vec<AXElement>>;

    // ── Input ──

    /// Click at absolute screen coordinates.
    async fn click(&self, x: f64, y: f64, double: bool, right: bool) -> LokiResult<()>;

    /// Click the center of a UI element.
    async fn click_element(
        &self,
        window: &WindowRef,
        query: &ElementQuery,
    ) -> LokiResult<AXElement>;

    /// Type a string of text. If `pid` is Some, target that process directly.
    async fn type_text(&self, text: &str, pid: Option<i32>) -> LokiResult<()>;

    /// Press a key combination (e.g. "cmd+shift+s"). If `pid` is Some, target that process.
    async fn key_press(&self, combo: &str, pid: Option<i32>) -> LokiResult<()>;

    // ── Screenshot ──

    /// Capture a screenshot, returning the PNG bytes.
    async fn screenshot(&self, window_id: Option<u32>, screen: bool) -> LokiResult<Vec<u8>>;

    // ── Wait ──

    /// Wait for an element matching a query to appear.
    async fn wait_for(
        &self,
        window: &WindowRef,
        query: &ElementQuery,
        timeout_ms: u64,
    ) -> LokiResult<AXElement>;

    /// Wait for an element matching a query to disappear.
    async fn wait_gone(
        &self,
        window: &WindowRef,
        query: &ElementQuery,
        timeout_ms: u64,
    ) -> LokiResult<()>;

    /// Wait for a window matching the filter to appear.
    async fn wait_window(&self, filter: &WindowFilter, timeout_ms: u64) -> LokiResult<WindowInfo>;

    /// Wait for a window's title to match a pattern.
    async fn wait_title(
        &self,
        window: &WindowRef,
        pattern: &str,
        timeout_ms: u64,
    ) -> LokiResult<WindowInfo>;

    // ── Permissions ──

    /// Check whether this process has accessibility permission.
    fn has_accessibility_permission(&self) -> bool;

    /// Prompt the user to grant accessibility permission.
    fn request_accessibility_permission(&self) -> bool;
}
