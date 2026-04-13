use glob::Pattern;
use serde::{Deserialize, Serialize};

use crate::element::AXElement;

/// Query to find UI elements in the accessibility tree.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementQuery {
    pub role: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    pub identifier: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub index: Option<usize>,
    pub max_depth: Option<usize>,
}

impl ElementQuery {
    /// Check if an AXElement matches this query.
    ///
    /// All specified criteria must match (AND logic).
    /// Role matching is case-insensitive and allows with or without "AX" prefix.
    pub fn matches(&self, element: &AXElement) -> bool {
        if let Some(ref role_pattern) = self.role {
            if !role_matches(role_pattern, &element.role) {
                return false;
            }
        }
        if let Some(ref pat) = self.title {
            // Match against title, description, or identifier — whichever is
            // the best human-readable label for this element.
            let matches_any_label = element
                .title
                .as_deref()
                .is_some_and(|t| glob_matches(pat, t))
                || element
                    .description
                    .as_deref()
                    .is_some_and(|d| glob_matches(pat, d))
                || element
                    .identifier
                    .as_deref()
                    .is_some_and(|i| glob_matches(pat, i));
            if !matches_any_label {
                return false;
            }
        }
        if let Some(ref pat) = self.label {
            // Match against ANY text field — title, value, description, or
            // identifier. This is broader than --title and catches webview
            // text elements (Tauri/wry, Safari) whose content lives in AXValue.
            let matches_any_text = element
                .title
                .as_deref()
                .is_some_and(|t| glob_matches(pat, t))
                || element
                    .value
                    .as_deref()
                    .is_some_and(|v| glob_matches(pat, v))
                || element
                    .description
                    .as_deref()
                    .is_some_and(|d| glob_matches(pat, d))
                || element
                    .identifier
                    .as_deref()
                    .is_some_and(|i| glob_matches(pat, i));
            if !matches_any_text {
                return false;
            }
        }
        if let Some(ref id) = self.identifier {
            match &element.identifier {
                Some(eid) => {
                    if eid != id {
                        return false;
                    }
                }
                None => return false,
            }
        }
        if let Some(ref pat) = self.value {
            match &element.value {
                Some(v) => {
                    if !glob_matches(pat, v) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        if let Some(ref pat) = self.description {
            match &element.description {
                Some(d) => {
                    if !glob_matches(pat, d) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

/// Check if a role pattern matches an element role.
/// Case-insensitive, allows both "AXButton" and "button" to match "AXButton".
fn role_matches(pattern: &str, element_role: &str) -> bool {
    let p = pattern.to_lowercase();
    let r = element_role.to_lowercase();

    // Strip "ax" prefix from both for comparison
    let p_stripped = p.strip_prefix("ax").unwrap_or(&p);
    let r_stripped = r.strip_prefix("ax").unwrap_or(&r);

    p_stripped == r_stripped
}

/// Search an AXElement tree for elements matching a query.
/// Returns all matches up to the query's index limit.
pub fn search_tree(root: &AXElement, query: &ElementQuery) -> Vec<AXElement> {
    let mut results = Vec::new();
    search_recursive(root, query, 0, &mut results);

    // If query.index is set, return only the nth match
    if let Some(idx) = query.index {
        if idx < results.len() {
            vec![results.remove(idx)]
        } else {
            Vec::new()
        }
    } else {
        results
    }
}

fn search_recursive(
    element: &AXElement,
    query: &ElementQuery,
    depth: usize,
    results: &mut Vec<AXElement>,
) {
    // Respect query max_depth
    if let Some(max_d) = query.max_depth {
        if depth > max_d {
            return;
        }
    }

    if query.matches(element) {
        // Clone without children for flat results
        results.push(AXElement {
            children: Vec::new(),
            ..element.clone()
        });
    }

    for child in &element.children {
        search_recursive(child, query, depth + 1, results);
    }
}

/// Filter for window discovery.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowFilter {
    pub title: Option<String>,
    pub bundle_id: Option<String>,
    pub pid: Option<u32>,
    /// If false (default), exclude windows with empty titles from listing.
    pub include_unnamed: bool,
}

/// Check if a string matches a glob pattern (case-insensitive).
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    // Try as glob pattern first; fall back to substring match if invalid
    match Pattern::new(pattern) {
        Ok(p) => p.matches(value),
        Err(_) => value.contains(pattern),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_matches_exact() {
        assert!(glob_matches("Finder", "Finder"));
        assert!(!glob_matches("Finder", "Safari"));
    }

    #[test]
    fn test_glob_matches_wildcard() {
        assert!(glob_matches("Find*", "Finder"));
        assert!(glob_matches("*der", "Finder"));
        assert!(glob_matches("*ind*", "Finder"));
    }

    #[test]
    fn test_glob_matches_question_mark() {
        assert!(glob_matches("Find?r", "Finder"));
        assert!(!glob_matches("Find?", "Finder"));
    }

    #[test]
    fn test_glob_invalid_falls_back_to_substring() {
        assert!(glob_matches("[invalid", "[invalid pattern"));
    }

    // ── Role matching tests ──

    #[test]
    fn test_role_matches_exact() {
        assert!(role_matches("AXButton", "AXButton"));
        assert!(role_matches("AXWindow", "AXWindow"));
    }

    #[test]
    fn test_role_matches_without_prefix() {
        assert!(role_matches("button", "AXButton"));
        assert!(role_matches("window", "AXWindow"));
        assert!(role_matches("textfield", "AXTextField"));
    }

    #[test]
    fn test_role_matches_case_insensitive() {
        assert!(role_matches("BUTTON", "AXButton"));
        assert!(role_matches("axbutton", "AXButton"));
        assert!(role_matches("Button", "AXButton"));
    }

    #[test]
    fn test_role_matches_mismatch() {
        assert!(!role_matches("button", "AXTextField"));
        assert!(!role_matches("AXWindow", "AXButton"));
    }

    // ── ElementQuery::matches tests ──

    fn make_element(role: &str, title: Option<&str>) -> AXElement {
        AXElement {
            role: role.to_string(),
            subrole: None,
            title: title.map(|s| s.to_string()),
            value: None,
            description: None,
            identifier: None,
            frame: None,
            enabled: true,
            focused: false,
            path: vec![],
            children: vec![],
        }
    }

    #[test]
    fn test_query_matches_empty_matches_all() {
        let q = ElementQuery::default();
        assert!(q.matches(&make_element("AXButton", Some("OK"))));
        assert!(q.matches(&make_element("AXWindow", None)));
    }

    #[test]
    fn test_query_matches_role_only() {
        let q = ElementQuery {
            role: Some("button".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&make_element("AXButton", Some("OK"))));
        assert!(!q.matches(&make_element("AXTextField", Some("name"))));
    }

    #[test]
    fn test_query_matches_title_glob() {
        let q = ElementQuery {
            title: Some("Untitled*".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&make_element("AXWindow", Some("Untitled"))));
        assert!(q.matches(&make_element("AXWindow", Some("Untitled — Edited"))));
        assert!(!q.matches(&make_element("AXWindow", Some("Document 1"))));
        assert!(!q.matches(&make_element("AXWindow", None)));
    }

    #[test]
    fn test_query_matches_and_logic() {
        let q = ElementQuery {
            role: Some("button".to_string()),
            title: Some("OK".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&make_element("AXButton", Some("OK"))));
        assert!(!q.matches(&make_element("AXButton", Some("Cancel"))));
        assert!(!q.matches(&make_element("AXTextField", Some("OK"))));
    }

    #[test]
    fn test_query_matches_identifier() {
        let mut el = make_element("AXButton", Some("OK"));
        el.identifier = Some("btn-ok".to_string());

        let q = ElementQuery {
            identifier: Some("btn-ok".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&el));

        let q2 = ElementQuery {
            identifier: Some("btn-cancel".to_string()),
            ..Default::default()
        };
        assert!(!q2.matches(&el));
    }

    // ── label branch tests ──

    #[test]
    fn test_query_matches_label_on_value_only() {
        // Element with no title, but AXValue="Settings" — typical webview text
        let mut el = make_element("AXStaticText", None);
        el.value = Some("Settings".to_string());

        // label should match via value
        let q_label = ElementQuery {
            label: Some("Settings".to_string()),
            ..Default::default()
        };
        assert!(q_label.matches(&el));

        // title should NOT match — title branch stays strict
        // (title/description/identifier only, not value)
        let q_title = ElementQuery {
            title: Some("Settings".to_string()),
            ..Default::default()
        };
        assert!(!q_title.matches(&el));
    }

    #[test]
    fn test_query_matches_label_on_title() {
        let el = make_element("AXStaticText", Some("Settings"));
        let q = ElementQuery {
            label: Some("Settings".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&el));
    }

    #[test]
    fn test_query_matches_label_on_description() {
        let mut el = make_element("AXStaticText", None);
        el.description = Some("Settings".to_string());
        let q = ElementQuery {
            label: Some("Settings".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&el));
    }

    #[test]
    fn test_query_matches_label_on_identifier() {
        let mut el = make_element("AXStaticText", None);
        el.identifier = Some("Settings".to_string());
        let q = ElementQuery {
            label: Some("Settings".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&el));
    }

    #[test]
    fn test_query_matches_label_glob_wildcard() {
        let mut el = make_element("AXStaticText", None);
        el.value = Some("ordis-dev".to_string());
        let q = ElementQuery {
            label: Some("ordis*".to_string()),
            ..Default::default()
        };
        assert!(q.matches(&el));
    }

    #[test]
    fn test_query_matches_label_none_of_four_fields() {
        // All four text fields are None — label match must fail
        let el = make_element("AXGroup", None);
        let q = ElementQuery {
            label: Some("anything".to_string()),
            ..Default::default()
        };
        assert!(!q.matches(&el));
    }

    // ── search_tree tests ──

    fn make_tree() -> AXElement {
        AXElement {
            role: "AXWindow".to_string(),
            title: Some("Main".to_string()),
            children: vec![
                AXElement {
                    role: "AXButton".to_string(),
                    title: Some("OK".to_string()),
                    path: vec![0],
                    children: vec![],
                    ..make_element("AXButton", Some("OK"))
                },
                AXElement {
                    role: "AXButton".to_string(),
                    title: Some("Cancel".to_string()),
                    path: vec![1],
                    children: vec![],
                    ..make_element("AXButton", Some("Cancel"))
                },
                AXElement {
                    role: "AXTextField".to_string(),
                    title: Some("Name".to_string()),
                    path: vec![2],
                    children: vec![AXElement {
                        role: "AXStaticText".to_string(),
                        title: Some("placeholder".to_string()),
                        path: vec![2, 0],
                        children: vec![],
                        ..make_element("AXStaticText", Some("placeholder"))
                    }],
                    ..make_element("AXTextField", Some("Name"))
                },
            ],
            ..make_element("AXWindow", Some("Main"))
        }
    }

    #[test]
    fn test_search_tree_by_role() {
        let tree = make_tree();
        let q = ElementQuery {
            role: Some("button".to_string()),
            ..Default::default()
        };
        let results = search_tree(&tree, &q);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title.as_deref(), Some("OK"));
        assert_eq!(results[1].title.as_deref(), Some("Cancel"));
    }

    #[test]
    fn test_search_tree_by_index() {
        let tree = make_tree();
        let q = ElementQuery {
            role: Some("button".to_string()),
            index: Some(1),
            ..Default::default()
        };
        let results = search_tree(&tree, &q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title.as_deref(), Some("Cancel"));
    }

    #[test]
    fn test_search_tree_max_depth() {
        let tree = make_tree();
        let q = ElementQuery {
            role: Some("statictext".to_string()),
            max_depth: Some(1),
            ..Default::default()
        };
        // statictext is at depth 2, max_depth 1 should miss it
        let results = search_tree(&tree, &q);
        assert_eq!(results.len(), 0);

        // Without max_depth, should find it
        let q2 = ElementQuery {
            role: Some("statictext".to_string()),
            ..Default::default()
        };
        let results2 = search_tree(&tree, &q2);
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_search_tree_by_label_value() {
        // Webview-style tree: AXStaticText with value but no title, like what
        // Tauri/wry or Safari webviews produce.
        let mut text_el = make_element("AXStaticText", None);
        text_el.value = Some("ordis".to_string());
        text_el.path = vec![0];

        let tree = AXElement {
            children: vec![text_el],
            ..make_element("AXWindow", Some("Webview"))
        };

        // Find via --label: should hit via value field
        let q_label = ElementQuery {
            label: Some("ordis".to_string()),
            ..Default::default()
        };
        let results = search_tree(&tree, &q_label);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value.as_deref(), Some("ordis"));

        // Regression gate: --title must NOT find it (title branch stays strict)
        let q_title = ElementQuery {
            title: Some("ordis".to_string()),
            ..Default::default()
        };
        let results_title = search_tree(&tree, &q_title);
        assert_eq!(results_title.len(), 0);
    }
}
