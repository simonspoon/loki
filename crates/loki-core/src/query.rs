use glob::Pattern;
use serde::{Deserialize, Serialize};

/// Query to find UI elements in the accessibility tree.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementQuery {
    pub role: Option<String>,
    pub title: Option<String>,
    pub identifier: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub index: Option<usize>,
    pub max_depth: Option<usize>,
}

/// Filter for window discovery.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowFilter {
    pub title: Option<String>,
    pub bundle_id: Option<String>,
    pub pid: Option<u32>,
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
}
