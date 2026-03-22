use async_trait::async_trait;
use loki_core::{
    AXElement, AppInfo, AppTarget, DesktopDriver, ElementQuery, LokiError, LokiResult,
    WindowFilter, WindowInfo, WindowRef,
};
use tokio::time::{sleep, Duration, Instant};
use tracing::debug;

use crate::accessibility;
use crate::app;
use crate::permission;
use crate::screenshot;
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

impl MacOSDriver {
    /// Look up a WindowInfo by WindowRef (window_id + pid) from the Core Graphics window list.
    async fn find_window_info(&self, window: &WindowRef) -> LokiResult<WindowInfo> {
        let filter = WindowFilter {
            pid: Some(window.pid),
            ..Default::default()
        };
        let windows = self.list_windows(&filter).await?;

        windows
            .into_iter()
            .find(|w| w.window_id == window.window_id)
            .ok_or_else(|| LokiError::WindowNotFound(format!("window_id={}", window.window_id)))
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

    // ── App lifecycle (Phase 2) ──

    async fn launch_app(&self, target: &str, args: &[String], wait: bool) -> LokiResult<AppInfo> {
        let app_target = AppTarget::parse(target);
        app::launch_app(&app_target, args)?;

        if wait {
            // Poll for the app's process to appear and become queryable
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut delay = Duration::from_millis(50);
            let max_delay = Duration::from_millis(500);

            loop {
                if let Ok(info) = app::get_app_info(&app_target) {
                    return Ok(info);
                }
                if Instant::now() >= deadline {
                    return Err(LokiError::Timeout(10_000));
                }
                sleep(delay).await;
                delay = (delay * 2).min(max_delay);
            }
        } else {
            // Best-effort: try to get info, but don't fail if not ready yet
            tokio::time::sleep(Duration::from_millis(200)).await;
            app::get_app_info(&app_target)
        }
    }

    async fn kill_app(&self, target: &str, force: bool) -> LokiResult<()> {
        let app_target = AppTarget::parse(target);
        app::kill_app(&app_target, force)
    }

    async fn app_info(&self, target: &str) -> LokiResult<AppInfo> {
        let app_target = AppTarget::parse(target);
        app::get_app_info(&app_target)
    }

    // ── Accessibility tree (Phase 2+) ──

    async fn get_tree(
        &self,
        window: &WindowRef,
        max_depth: Option<usize>,
    ) -> LokiResult<AXElement> {
        if !self.has_accessibility_permission() {
            return Err(LokiError::PermissionDenied);
        }

        // Find the window's title from the window list so we can match the AX element
        let win_info = self.find_window_info(window).await?;
        let ax_window = accessibility::find_window_element(window.pid as i32, &win_info.title)?;

        accessibility::walk_tree(ax_window, max_depth, 0, vec![])
    }

    async fn find_elements(
        &self,
        window: &WindowRef,
        query: &ElementQuery,
    ) -> LokiResult<Vec<AXElement>> {
        let tree = self.get_tree(window, query.max_depth).await?;
        Ok(loki_core::query::search_tree(&tree, query))
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

    // ── Screenshot (Phase 2) ──

    async fn screenshot(&self, window_id: Option<u32>, screen: bool) -> LokiResult<Vec<u8>> {
        if screen {
            screenshot::capture_screen()
        } else if let Some(wid) = window_id {
            screenshot::capture_window(wid)
        } else {
            Err(LokiError::ScreenshotFailed(
                "specify --window <ID> or --screen".into(),
            ))
        }
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

    async fn wait_window(&self, filter: &WindowFilter, timeout_ms: u64) -> LokiResult<WindowInfo> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut delay = Duration::from_millis(50);
        let max_delay = Duration::from_millis(500);

        loop {
            if let Some(win) = self.find_window(filter).await? {
                debug!(window_id = win.window_id, "window appeared");
                return Ok(win);
            }
            if Instant::now() >= deadline {
                return Err(LokiError::Timeout(timeout_ms));
            }
            sleep(delay).await;
            delay = (delay * 2).min(max_delay);
        }
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
