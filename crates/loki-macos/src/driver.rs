use async_trait::async_trait;
use loki_core::{
    AXElement, AppInfo, DesktopDriver, ElementQuery, LokiError, LokiResult, WindowFilter,
    WindowInfo, WindowRef,
};

use crate::permission;
use crate::window;

/// macOS implementation of the DesktopDriver trait.
pub struct MacOSDriver;

impl MacOSDriver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DesktopDriver for MacOSDriver {
    // ── Window discovery (Phase 1) ──

    async fn list_windows(&self, filter: &WindowFilter) -> LokiResult<Vec<WindowInfo>> {
        let all = window::list_all_windows();

        let filtered: Vec<WindowInfo> = all
            .into_iter()
            .filter(|w| {
                if let Some(ref pat) = filter.title {
                    if !loki_core::query::glob_matches(pat, &w.title) {
                        return false;
                    }
                }
                if let Some(ref bid) = filter.bundle_id {
                    match &w.bundle_id {
                        Some(wb) if wb == bid => {}
                        _ => return false,
                    }
                }
                if let Some(pid) = filter.pid {
                    if w.pid != pid {
                        return false;
                    }
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    async fn find_window(&self, filter: &WindowFilter) -> LokiResult<Option<WindowInfo>> {
        let windows = self.list_windows(filter).await?;
        Ok(windows.into_iter().next())
    }

    // ── App lifecycle (Phase 2+) ──

    async fn launch_app(
        &self,
        _target: &str,
        _args: &[String],
        _wait: bool,
    ) -> LokiResult<AppInfo> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn kill_app(&self, _target: &str, _force: bool) -> LokiResult<()> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn app_info(&self, _target: &str) -> LokiResult<AppInfo> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    // ── Accessibility tree (Phase 2+) ──

    async fn get_tree(
        &self,
        _window: &WindowRef,
        _max_depth: Option<usize>,
    ) -> LokiResult<AXElement> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn find_elements(
        &self,
        _window: &WindowRef,
        _query: &ElementQuery,
    ) -> LokiResult<Vec<AXElement>> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    // ── Input (Phase 2+) ──

    async fn click(&self, _x: f64, _y: f64, _double: bool, _right: bool) -> LokiResult<()> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn click_element(
        &self,
        _window: &WindowRef,
        _query: &ElementQuery,
    ) -> LokiResult<AXElement> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn type_text(&self, _text: &str) -> LokiResult<()> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn key_press(&self, _combo: &str) -> LokiResult<()> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    // ── Screenshot (Phase 2+) ──

    async fn screenshot(&self, _window_id: Option<u32>, _screen: bool) -> LokiResult<Vec<u8>> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    // ── Wait (Phase 2+) ──

    async fn wait_for(
        &self,
        _window: &WindowRef,
        _query: &ElementQuery,
        _timeout_ms: u64,
    ) -> LokiResult<AXElement> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn wait_gone(
        &self,
        _window: &WindowRef,
        _query: &ElementQuery,
        _timeout_ms: u64,
    ) -> LokiResult<()> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn wait_window(
        &self,
        _filter: &WindowFilter,
        _timeout_ms: u64,
    ) -> LokiResult<WindowInfo> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    async fn wait_title(
        &self,
        _window: &WindowRef,
        _pattern: &str,
        _timeout_ms: u64,
    ) -> LokiResult<WindowInfo> {
        Err(LokiError::Platform("not yet implemented".into()))
    }

    // ── Permissions (Phase 1) ──

    fn has_accessibility_permission(&self) -> bool {
        permission::is_trusted()
    }

    fn request_accessibility_permission(&self) -> bool {
        permission::request_trust()
    }
}
