use glob::{MatchOptions, Pattern};
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
    /// Text matching (`title`, `label`, `value`, `description`) is
    /// case-insensitive too; `identifier` is an exact, case-sensitive compare.
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

/// AX roles that actually *do* something when clicked.
///
/// A query hitting one of these outranks a caption that merely mentions the
/// same word: in a save panel `--label Save` matches both the "Save As:"
/// AXStaticText and the Save AXButton, and clicking the caption is a silent
/// no-op that looks exactly like success.
const ACTIONABLE_ROLES: &[&str] = &[
    "AXButton",
    "AXMenuItem",
    "AXMenuBarItem",
    "AXMenuButton",
    "AXPopUpButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXDisclosureTriangle",
    "AXLink",
    "AXTextField",
    "AXTextArea",
    "AXComboBox",
];

/// Whether clicking an element of this role is a meaningful action.
pub fn is_actionable(role: &str) -> bool {
    ACTIONABLE_ROLES.iter().any(|r| role_matches(r, role))
}

/// Which element a click should land on, given every match for a query.
#[derive(Debug)]
pub enum ClickTarget<'a> {
    /// Nothing matched.
    None,
    /// One unambiguous target.
    One(&'a AXElement),
    /// Several *actionable* elements matched and there is no safe guess between
    /// them. Carries the candidates so the caller can name them instead of
    /// clicking a coin flip.
    Ambiguous(Vec<&'a AXElement>),
}

/// Narrow a match list down to the one element a click should land on.
///
/// Actionable roles win over everything else, so a `--label Save` that hits the
/// "Save As:" caption *and* the Save button lands on the button. Among several
/// actionable matches there is no safe guess — the caller is told to
/// disambiguate. When nothing actionable matched, first-match order stands:
/// a webview's text content is all AXStaticText (see the Tauri/wry case
/// `--label` was built for) and clicking the first hit is the established
/// behaviour there.
pub fn pick_click_target(matches: &[AXElement]) -> ClickTarget<'_> {
    let actionable: Vec<&AXElement> = matches
        .iter()
        .filter(|e| is_actionable(&e.role))
        .collect();

    match actionable.len() {
        0 => match matches.first() {
            Some(el) => ClickTarget::One(el),
            None => ClickTarget::None,
        },
        1 => ClickTarget::One(actionable[0]),
        _ => ClickTarget::Ambiguous(actionable),
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

impl WindowFilter {
    /// Check a window title against this filter's title pattern.
    ///
    /// Window titles match as a **case-insensitive substring**: a bare
    /// `"ash-md"` matches `"ash-md — README.md"` and `"ASH-MD"`, the same way
    /// `--label` behaves for elements. A pattern carrying glob metacharacters
    /// is used verbatim, so `"ash-md*"` anchors the start and `"ash-m[d]"`
    /// pins the whole title.
    pub fn matches_title(&self, title: &str) -> bool {
        match self.title {
            Some(ref pattern) => glob_matches(&auto_wrap_pattern(pattern), title),
            None => true,
        }
    }

    /// The glob actually used to match titles — for diagnostics, so an error
    /// can show what was matched rather than what was typed.
    pub fn effective_title_pattern(&self) -> Option<String> {
        self.title.as_deref().map(auto_wrap_pattern)
    }
}

/// Wrap a bare pattern with substring globs so `"Projects"` matches any value
/// containing "Projects". A pattern that already carries glob metacharacters
/// (`*`, `?`, `[`) passes through unchanged. Empty stays empty — wrapping it to
/// `**` would match everything.
pub fn auto_wrap_pattern(pattern: &str) -> String {
    if pattern.is_empty() {
        return String::new();
    }
    if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
        return pattern.to_string();
    }
    format!("*{pattern}*")
}

/// Match options for every text query: identical to `Pattern::matches`'s
/// defaults except for case. The other two fields are path-oriented and stay
/// off — AX titles are not paths.
const TEXT_MATCH: MatchOptions = MatchOptions {
    case_sensitive: false,
    require_literal_separator: false,
    require_literal_leading_dot: false,
};

/// Check if a string matches a glob pattern. **Case-insensitive** (mesa 540).
///
/// Titles, labels, values and descriptions are human-facing strings where case
/// is presentation, not identity, and a case-only miss returns an empty result
/// indistinguishable from "the element isn't there". Roles and menu paths have
/// always folded case; this makes the rest of the tool agree with them.
/// `--identifier` stays an exact compare — that is the strict escape hatch.
///
/// Two consequences of `case_sensitive: false`, both from the `glob` crate:
/// folding is **ASCII-only**, so `é` still won't match `É`; and an *alphabetic*
/// character range relaxes, so `[a-z]` also matches `Q`. Numeric and symbol
/// ranges (`[0-9]`, `[!.]`) are unaffected.
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    // Try as glob pattern first; fall back to substring match if invalid.
    // The fallback folds case ASCII-only too, so both paths agree.
    match Pattern::new(pattern) {
        Ok(p) => p.matches_with(value, TEXT_MATCH),
        Err(_) => value
            .to_ascii_lowercase()
            .contains(&pattern.to_ascii_lowercase()),
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

    // ── Case-insensitive text matching (mesa 540) ──

    #[test]
    fn test_glob_matches_is_case_insensitive() {
        // The reported trap: a name typed from memory in the wrong case read as
        // "element not present" rather than "you typed it differently".
        assert!(glob_matches("save", "Save"));
        assert!(glob_matches("SAVE", "save"));
        assert!(glob_matches("*ash-md*", "ASH-MD — README.md"));
        assert!(glob_matches("Find?R", "finder"));
        // Still a real mismatch, not a blanket yes.
        assert!(!glob_matches("Finder", "Safari"));
    }

    #[test]
    fn test_glob_invalid_fallback_also_folds_case() {
        // The invalid-pattern path must agree with the glob path, or the same
        // query means two different things depending on its metacharacters.
        assert!(glob_matches("[INVALID", "an [invalid pattern"));
    }

    #[test]
    fn test_glob_case_folding_is_ascii_only() {
        // Documented limitation of the glob crate's `chars_eq`: non-ASCII case
        // relationships are not folded. Callers matching accented titles must
        // still type the case they see.
        assert!(!glob_matches("café", "CAFÉ"));
        // The ASCII half of the same string folds, so a wildcard gets there.
        assert!(glob_matches("caf*", "CAFÉ"));
    }

    #[test]
    fn test_glob_alphabetic_char_ranges_relax() {
        // The known cost of `case_sensitive: false` — an alphabetic range
        // matches both cases. Non-alphabetic ranges are unaffected.
        assert!(glob_matches("[a-z]", "Q"));
        assert!(glob_matches("[A-Z]", "q"));
        assert!(!glob_matches("[0-9]", "q"));
        assert!(glob_matches("[0-9]", "7"));
    }

    #[test]
    fn test_identifier_stays_case_sensitive_exact() {
        // The strict escape hatch: `--id` never globbed and never folds, so a
        // caller who needs exact identity still has one.
        let mut el = make_element("AXButton", Some("Save"));
        el.identifier = Some("btn-ok".to_string());

        let exact = ElementQuery {
            identifier: Some("btn-ok".to_string()),
            ..Default::default()
        };
        assert!(exact.matches(&el));

        let wrong_case = ElementQuery {
            identifier: Some("BTN-OK".to_string()),
            ..Default::default()
        };
        assert!(!wrong_case.matches(&el));
    }

    #[test]
    fn test_query_title_and_label_fold_case() {
        let el = make_element("AXButton", Some("Save"));
        let by_title = ElementQuery {
            title: Some("save".to_string()),
            ..Default::default()
        };
        assert!(by_title.matches(&el));

        let mut webview_text = make_element("AXStaticText", None);
        webview_text.value = Some("Settings".to_string());
        let by_label = ElementQuery {
            label: Some("settings".to_string()),
            ..Default::default()
        };
        assert!(by_label.matches(&webview_text));
    }

    #[test]
    fn test_query_value_and_description_fold_case() {
        let mut el = make_element("AXStaticText", None);
        el.value = Some("Ready".to_string());
        el.description = Some("Status Line".to_string());

        let by_value = ElementQuery {
            value: Some("ready".to_string()),
            ..Default::default()
        };
        assert!(by_value.matches(&el));

        let by_desc = ElementQuery {
            description: Some("STATUS*".to_string()),
            ..Default::default()
        };
        assert!(by_desc.matches(&el));
    }

    // ── Window title matching (mesa 530) ──

    fn title_filter(pattern: &str) -> WindowFilter {
        WindowFilter {
            title: Some(pattern.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_window_title_matches_substring() {
        // The reported case: a bare app name against a decorated window title.
        assert!(title_filter("ash-md").matches_title("ash-md — README.md"));
        assert!(title_filter("ash-md").matches_title("ash-md"));
        assert!(!title_filter("ash-md").matches_title("Safari"));
    }

    #[test]
    fn test_window_title_glob_passthrough_anchors() {
        assert!(title_filter("ash-md*").matches_title("ash-md — README.md"));
        assert!(!title_filter("*README.md").matches_title("ash-md — README.md.bak"));
        // Metacharacters escape the auto-wrap, so a whole-title pin stays possible.
        assert!(title_filter("ash-m[d]").matches_title("ash-md"));
        assert!(!title_filter("ash-m[d]").matches_title("ash-md — README.md"));
    }

    #[test]
    fn test_window_title_absent_pattern_matches_all() {
        assert!(WindowFilter::default().matches_title("anything"));
        assert!(WindowFilter::default().matches_title(""));
    }

    #[test]
    fn test_window_title_is_case_insensitive() {
        // Was the reverse until mesa 540; this exact pair (`--title ASH-MD`
        // against a window titled `ash-md`) is what the wait-window timeout
        // diagnostic used to have to report as a near-miss.
        assert!(title_filter("ash-md").matches_title("ASH-MD"));
        assert!(title_filter("ASH-MD").matches_title("ash-md — README.md"));
        // Glob metacharacters still anchor; only case stopped mattering.
        assert!(!title_filter("ASH-MD*").matches_title("the ash-md window"));
    }

    #[test]
    fn test_effective_title_pattern_reports_the_wrap() {
        assert_eq!(
            title_filter("ash-md").effective_title_pattern().unwrap(),
            "*ash-md*"
        );
        assert_eq!(
            title_filter("ash-md*").effective_title_pattern().unwrap(),
            "ash-md*"
        );
        assert_eq!(WindowFilter::default().effective_title_pattern(), None);
    }

    #[test]
    fn test_auto_wrap_pattern_empty_stays_empty() {
        assert_eq!(auto_wrap_pattern(""), "");
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

    // ── Click-target selection (mesa 537) ──

    #[test]
    fn test_is_actionable_covers_click_targets_not_captions() {
        assert!(is_actionable("AXButton"));
        assert!(is_actionable("AXMenuItem"));
        assert!(is_actionable("AXTextField"));
        assert!(is_actionable("AXCheckBox"));
        // Prefix-tolerant and case-insensitive, like every other role compare.
        assert!(is_actionable("button"));
        // The roles that made the bug: labels, containers, decoration.
        assert!(!is_actionable("AXStaticText"));
        assert!(!is_actionable("AXGroup"));
        assert!(!is_actionable("AXImage"));
    }

    fn pick_one(matches: &[AXElement]) -> &AXElement {
        match pick_click_target(matches) {
            ClickTarget::One(el) => el,
            other => panic!("expected one target, got {other:?}"),
        }
    }

    #[test]
    fn test_pick_click_target_prefers_button_over_caption() {
        // The reported case: `--label Save` in a save panel matches the
        // "Save As:" caption first (it sits earlier in the tree) and the Save
        // button second. Tree order must not decide this.
        let mut caption = make_element("AXStaticText", Some("Save As:"));
        caption.identifier = Some("nameFieldLabel".to_string());
        let button = make_element("AXButton", Some("Save"));

        let matches = [caption, button];
        let picked = pick_one(&matches);
        assert_eq!(picked.role, "AXButton");
        assert_eq!(picked.title.as_deref(), Some("Save"));
    }

    #[test]
    fn test_pick_click_target_ambiguous_actionable_refuses() {
        // Two real buttons: no safe guess, so the caller gets the candidates
        // rather than a silent coin flip.
        let matches = vec![
            make_element("AXButton", Some("Save")),
            make_element("AXStaticText", Some("Save As:")),
            make_element("AXButton", Some("Save All")),
        ];
        match pick_click_target(&matches) {
            ClickTarget::Ambiguous(candidates) => {
                // Only the actionable ones are offered as candidates.
                assert_eq!(candidates.len(), 2);
                assert!(candidates.iter().all(|c| c.role == "AXButton"));
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
    }

    #[test]
    fn test_pick_click_target_all_static_keeps_first_match() {
        // Webview content (Tauri/wry, Safari) is nested AXStaticText with no
        // actionable role anywhere — refusing here would break the flow
        // `--label` exists for. First match still wins.
        let mut outer = make_element("AXGroup", None);
        outer.value = Some("Settings".to_string());
        let mut inner = make_element("AXStaticText", None);
        inner.value = Some("Settings".to_string());

        let matches = [outer, inner];
        let picked = pick_one(&matches);
        assert_eq!(picked.role, "AXGroup");
    }

    #[test]
    fn test_pick_click_target_single_match_unchanged() {
        let matches = [make_element("AXStaticText", Some("only"))];
        let picked = pick_one(&matches);
        assert_eq!(picked.title.as_deref(), Some("only"));
    }

    #[test]
    fn test_pick_click_target_empty_is_none() {
        assert!(matches!(pick_click_target(&[]), ClickTarget::None));
    }

    #[test]
    fn test_pick_click_target_index_query_stays_deterministic() {
        // `--index` narrows inside search_tree to exactly one element, so the
        // picker must pass it through even when it is a caption — the caller
        // already chose from `find`'s tree-ordered list.
        let tree = make_tree();
        let q = ElementQuery {
            role: Some("statictext".to_string()),
            index: Some(0),
            ..Default::default()
        };
        let results = search_tree(&tree, &q);
        assert_eq!(results.len(), 1);
        assert_eq!(pick_one(&results).role, "AXStaticText");
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
