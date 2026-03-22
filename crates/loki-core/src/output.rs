use crate::element::{AppInfo, WindowInfo};

/// Output format for CLI results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Format a list of windows for display.
pub fn format_windows(windows: &[WindowInfo], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_windows_text(windows),
        OutputFormat::Json => serde_json::to_string_pretty(windows).unwrap_or_default(),
    }
}

fn format_windows_text(windows: &[WindowInfo]) -> String {
    if windows.is_empty() {
        return "No windows found.".to_string();
    }

    let mut lines = Vec::with_capacity(windows.len() + 1);
    lines.push(format!(
        "{:<8} {:<8} {:<40} {:>6}x{:<6}",
        "WID", "PID", "TITLE", "W", "H"
    ));

    for w in windows {
        lines.push(format!(
            "{:<8} {:<8} {:<40} {:>6.0}x{:<6.0}",
            w.window_id,
            w.pid,
            truncate(&w.title, 40),
            w.frame.width,
            w.frame.height,
        ));
    }

    lines.join("\n")
}

/// Format app info for display.
pub fn format_app_info(info: &AppInfo, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let mut lines = Vec::new();
            lines.push(format!("PID:       {}", info.pid));
            lines.push(format!("Name:      {}", info.name));
            if let Some(ref bid) = info.bundle_id {
                lines.push(format!("Bundle ID: {bid}"));
            }
            lines.push(format!(
                "Active:    {}",
                if info.is_active { "yes" } else { "no" }
            ));
            lines.join("\n")
        }
        OutputFormat::Json => serde_json::to_string_pretty(info).unwrap_or_default(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::ElementFrame;

    fn sample_window() -> WindowInfo {
        WindowInfo {
            window_id: 42,
            pid: 100,
            title: "Test Window".to_string(),
            bundle_id: Some("com.test.app".to_string()),
            frame: ElementFrame {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            is_on_screen: true,
        }
    }

    #[test]
    fn test_format_windows_text_empty() {
        assert_eq!(format_windows(&[], OutputFormat::Text), "No windows found.");
    }

    #[test]
    fn test_format_windows_text_has_header() {
        let output = format_windows(&[sample_window()], OutputFormat::Text);
        assert!(output.starts_with("WID"));
        assert!(output.contains("Test Window"));
    }

    #[test]
    fn test_format_windows_json() {
        let output = format_windows(&[sample_window()], OutputFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed[0]["window_id"], 42);
    }
}
