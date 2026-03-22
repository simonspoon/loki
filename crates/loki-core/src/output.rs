use crate::element::{AXElement, AppInfo, WindowInfo};

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

/// Format an accessibility tree for display.
pub fn format_tree(element: &AXElement, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_tree_text(element, 0),
        OutputFormat::Json => serde_json::to_string_pretty(element).unwrap_or_default(),
    }
}

fn format_tree_text(element: &AXElement, indent: usize) -> String {
    let mut lines = Vec::new();
    let prefix = "  ".repeat(indent);

    let mut line = format!("{prefix}{}", element.role);

    // Show the best available label: title, then description, then identifier
    let label = element
        .title
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(element.description.as_deref().filter(|s| !s.is_empty()))
        .or(element.identifier.as_deref().filter(|s| !s.is_empty()));

    if let Some(label) = label {
        line.push_str(&format!(" \"{label}\""));
    }

    if let Some(ref frame) = element.frame {
        line.push_str(&format!(
            " ({:.0}x{:.0} at {:.0},{:.0})",
            frame.width, frame.height, frame.x, frame.y
        ));
    }

    lines.push(line);

    for child in &element.children {
        lines.push(format_tree_text(child, indent + 1));
    }

    lines.join("\n")
}

/// Flatten an AXElement tree to a list of all elements (depth-first).
pub fn flatten_tree(root: &AXElement) -> Vec<AXElement> {
    let mut result = Vec::new();
    flatten_recursive(root, &mut result);
    result
}

fn flatten_recursive(element: &AXElement, result: &mut Vec<AXElement>) {
    result.push(AXElement {
        children: Vec::new(),
        ..element.clone()
    });
    for child in &element.children {
        flatten_recursive(child, result);
    }
}

/// Format a list of elements for display.
pub fn format_elements(elements: &[AXElement], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_elements_text(elements),
        OutputFormat::Json => serde_json::to_string_pretty(elements).unwrap_or_default(),
    }
}

fn format_elements_text(elements: &[AXElement]) -> String {
    if elements.is_empty() {
        return "No elements found.".to_string();
    }

    let mut lines = Vec::with_capacity(elements.len());
    for el in elements {
        let mut parts = vec![el.role.clone()];

        // Show the best available label
        let label = el
            .title
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(el.description.as_deref().filter(|s| !s.is_empty()));

        if let Some(label) = label {
            parts.push(format!("\"{label}\""));
        }

        if let Some(ref id) = el.identifier {
            if !id.is_empty() {
                parts.push(format!("id={id}"));
            }
        }

        if let Some(ref frame) = el.frame {
            parts.push(format!(
                "({:.0}x{:.0} at {:.0},{:.0})",
                frame.width, frame.height, frame.x, frame.y
            ));
        }

        if !el.path.is_empty() {
            let path_str: Vec<String> = el.path.iter().map(|p| p.to_string()).collect();
            parts.push(format!("[{}]", path_str.join(".")));
        }

        lines.push(parts.join(" "));
    }

    lines.join("\n")
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

    // ── Tree formatting tests ──

    fn sample_tree() -> AXElement {
        AXElement {
            role: "AXWindow".to_string(),
            subrole: None,
            title: Some("Untitled".to_string()),
            value: None,
            description: None,
            identifier: None,
            frame: Some(ElementFrame {
                x: 0.0,
                y: 25.0,
                width: 1920.0,
                height: 1080.0,
            }),
            enabled: true,
            focused: false,
            path: vec![],
            children: vec![AXElement {
                role: "AXScrollArea".to_string(),
                subrole: None,
                title: None,
                value: None,
                description: None,
                identifier: None,
                frame: Some(ElementFrame {
                    x: 0.0,
                    y: 50.0,
                    width: 1920.0,
                    height: 1055.0,
                }),
                enabled: true,
                focused: false,
                path: vec![0],
                children: vec![AXElement {
                    role: "AXButton".to_string(),
                    subrole: None,
                    title: Some("OK".to_string()),
                    value: None,
                    description: None,
                    identifier: None,
                    frame: None,
                    enabled: true,
                    focused: false,
                    path: vec![0, 0],
                    children: vec![],
                }],
            }],
        }
    }

    #[test]
    fn test_format_tree_text_indentation() {
        let output = format_tree(&sample_tree(), OutputFormat::Text);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("AXWindow"));
        assert!(lines[0].contains("\"Untitled\""));
        assert!(lines[0].contains("1920x1080"));
        assert!(lines[1].starts_with("  AXScrollArea"));
        assert!(lines[2].starts_with("    AXButton"));
        assert!(lines[2].contains("\"OK\""));
    }

    #[test]
    fn test_format_tree_json() {
        let output = format_tree(&sample_tree(), OutputFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["role"], "AXWindow");
        assert!(parsed["children"].is_array());
    }

    #[test]
    fn test_format_elements_text_empty() {
        assert_eq!(
            format_elements(&[], OutputFormat::Text),
            "No elements found."
        );
    }

    #[test]
    fn test_format_elements_text_with_items() {
        let elements = vec![AXElement {
            role: "AXButton".to_string(),
            subrole: None,
            title: Some("OK".to_string()),
            value: None,
            description: None,
            identifier: Some("btn-ok".to_string()),
            frame: Some(ElementFrame {
                x: 10.0,
                y: 20.0,
                width: 80.0,
                height: 30.0,
            }),
            enabled: true,
            focused: false,
            path: vec![0, 1],
            children: vec![],
        }];
        let output = format_elements(&elements, OutputFormat::Text);
        assert!(output.contains("AXButton"));
        assert!(output.contains("\"OK\""));
        assert!(output.contains("id=btn-ok"));
        assert!(output.contains("80x30 at 10,20"));
        assert!(output.contains("[0.1]"));
    }

    #[test]
    fn test_flatten_tree() {
        let tree = sample_tree();
        let flat = flatten_tree(&tree);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].role, "AXWindow");
        assert_eq!(flat[1].role, "AXScrollArea");
        assert_eq!(flat[2].role, "AXButton");
        // Flattened elements should have no children
        assert!(flat[0].children.is_empty());
    }
}
