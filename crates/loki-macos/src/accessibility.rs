//! Safe(r) wrappers around macOS Accessibility (AXUIElement) APIs.
//!
//! Provides tree walking, attribute extraction, and window element lookup.

use core_foundation::base::{CFGetTypeID, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringGetTypeID, CFStringRef};
use loki_core::{AXElement, ElementFrame, LokiError, LokiResult};
use std::ffi::c_void;
use tracing::trace;

// ── FFI declarations ──

type AXUIElementRef = CFTypeRef;
type AXError = i32;

const AX_ERROR_SUCCESS: AXError = 0;
const AX_ERROR_API_DISABLED: AXError = -25211;
const AX_ERROR_NO_VALUE: AXError = -25212;
const AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25205;

// AXValueType constants
const K_AX_VALUE_CG_POINT_TYPE: u32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: u32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, value_ptr: *mut c_void) -> bool;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
    fn CFRelease(cf: CFTypeRef);
}

// ── CGPoint / CGSize for AXValue extraction ──

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

// ── Public API ──

/// Create an AX element reference for a running application by PID.
pub fn create_application_element(pid: i32) -> AXUIElementRef {
    unsafe { AXUIElementCreateApplication(pid) }
}

/// Get all AX window elements of an application.
pub fn get_application_windows(app: AXUIElementRef) -> LokiResult<Vec<AXUIElementRef>> {
    get_array_attribute(app, "AXWindows")
}

/// Find a specific window AX element by matching window title.
pub fn find_window_element(pid: i32, window_title: &str) -> LokiResult<AXUIElementRef> {
    let app = create_application_element(pid);
    if app.is_null() {
        return Err(LokiError::Platform(format!(
            "failed to create AX element for PID {pid}"
        )));
    }

    let windows = get_application_windows(app)?;

    for win in &windows {
        if let Some(title) = get_string_attribute(*win, "AXTitle") {
            if title == window_title {
                // Retain the element we're returning; release the app ref
                let found = *win;
                unsafe { CFRelease(app) };
                return Ok(found);
            }
        }
    }

    // Fallback: if no exact title match, try to return the first window
    // This handles cases where the window title from CG doesn't exactly match AX
    if let Some(first) = windows.into_iter().next() {
        unsafe { CFRelease(app) };
        return Ok(first);
    }

    unsafe { CFRelease(app) };
    Err(LokiError::WindowNotFound(format!(
        "no AX window with title '{window_title}' for PID {pid}"
    )))
}

/// Recursively walk the accessibility tree from a root element, building an AXElement tree.
///
/// - `max_depth`: None = unlimited, Some(n) = stop at depth n
/// - `current_depth`: current recursion depth (start at 0)
/// - `path`: index path from root for ElementRef construction
pub fn walk_tree(
    element: AXUIElementRef,
    max_depth: Option<usize>,
    current_depth: usize,
    path: Vec<usize>,
) -> LokiResult<AXElement> {
    let role = get_string_attribute(element, "AXRole").unwrap_or_else(|| "AXUnknown".to_string());
    let subrole = get_string_attribute(element, "AXSubrole");
    let title = get_string_attribute(element, "AXTitle");
    let value = get_string_attribute(element, "AXValue");
    let description = get_string_attribute(element, "AXDescription");
    let identifier = get_string_attribute(element, "AXIdentifier");
    let frame = get_frame(element);
    let enabled = get_bool_attribute(element, "AXEnabled");
    let focused = get_bool_attribute(element, "AXFocused");

    trace!(
        role = %role,
        title = ?title,
        depth = current_depth,
        "walking element"
    );

    let children = if max_depth.is_some_and(|d| current_depth >= d) {
        Vec::new()
    } else {
        let child_refs = get_children(element);
        let mut child_elements = Vec::with_capacity(child_refs.len());
        for (i, child_ref) in child_refs.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            match walk_tree(*child_ref, max_depth, current_depth + 1, child_path) {
                Ok(child) => child_elements.push(child),
                Err(e) => {
                    trace!(error = %e, "skipping child element");
                }
            }
        }
        child_elements
    };

    Ok(AXElement {
        role,
        subrole,
        title,
        value,
        description,
        identifier,
        frame,
        enabled,
        focused,
        path,
        children,
    })
}

// ── Menu bar navigation ──

const AX_PRESS_ACTION: &str = "AXPress";

/// Navigate an application's menu bar and press the item named by `path`.
///
/// The menu bar hangs off the *application* AX element (`AXMenuBar`), not the
/// window tree — so coordinate clicks and window-scoped `find` can't reach it.
/// Each path segment names one level: `["File", "Open File…"]`. Titles are
/// matched leniently (case-insensitive, trailing ellipsis ignored, substring),
/// so `"Open File"` matches a `"Open File…"` item.
///
/// AX exposes the whole menu tree even while closed, so we walk it directly and
/// fire `AXPress` on the leaf — this bypasses the modal NSMenu event loop that
/// swallows synthetic `CGEvent` clicks. If a level's children aren't populated
/// yet (some apps build submenus lazily), we press the parent to open it and
/// re-read once.
pub fn press_menu_path(pid: i32, path: &[String]) -> LokiResult<AXElement> {
    if path.is_empty() {
        return Err(LokiError::InputError("empty menu path".into()));
    }

    let app = create_application_element(pid);
    if app.is_null() {
        return Err(LokiError::Platform(format!(
            "failed to create AX element for PID {pid}"
        )));
    }

    let menu_bar = get_element_attribute(app, "AXMenuBar").ok_or_else(|| {
        LokiError::ElementNotFound(format!("app (PID {pid}) has no menu bar"))
    })?;

    // Walk down the path, one segment per level. `parent` is the container we
    // enumerate items from: the menu bar itself, then each matched item.
    let mut parent = menu_bar;
    let last = path.len() - 1;
    for (idx, segment) in path.iter().enumerate() {
        let mut items = menu_items(parent);
        if items.is_empty() {
            // Lazily-built submenu: open the parent, wait briefly, re-read.
            let _ = perform_press(parent);
            std::thread::sleep(std::time::Duration::from_millis(80));
            items = menu_items(parent);
        }

        // Prefer an exact (normalized) title match; only fall back to fuzzy
        // substring/glob if nothing matches exactly. Without this, "Edit"
        // would substring-match the "TextEdit" application menu.
        let title_of = |it: &AXUIElementRef| get_string_attribute(*it, "AXTitle");
        let matched = items
            .iter()
            .find(|it| title_of(it).is_some_and(|t| menu_title_eq(segment, &t)))
            .or_else(|| {
                items
                    .iter()
                    .find(|it| title_of(it).is_some_and(|t| menu_title_fuzzy(segment, &t)))
            })
            .copied()
            .ok_or_else(|| {
                let available: Vec<String> = items
                    .iter()
                    .filter_map(|it| get_string_attribute(*it, "AXTitle"))
                    .filter(|t| !t.is_empty())
                    .collect();
                LokiError::ElementNotFound(format!(
                    "menu item '{segment}' not found; available: [{}]",
                    available.join(", ")
                ))
            })?;

        if idx == last {
            perform_press(matched)?;
            return walk_tree(matched, Some(0), 0, vec![]);
        }
        parent = matched;
    }

    unreachable!("loop returns on the last segment")
}

/// Enumerate the selectable menu items directly under `container`.
///
/// A menu-bar item wraps its contents in an `AXMenu` child whose children are
/// the real `AXMenuItem`s; the menu bar itself lists `AXMenuBarItem`s directly.
/// Unwrap one `AXMenu` layer so callers see a flat list of pressable items.
fn menu_items(container: AXUIElementRef) -> Vec<AXUIElementRef> {
    let mut out = Vec::new();
    for child in get_children(container) {
        match get_string_attribute(child, "AXRole").as_deref() {
            Some("AXMenu") => out.extend(get_children(child)),
            _ => out.push(child),
        }
    }
    out
}

/// Normalize a menu title for comparison: trim, drop a trailing ellipsis
/// (`…` or `...`), and case-fold.
fn normalize_menu_title(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_suffix('…')
        .or_else(|| s.strip_suffix("..."))
        .unwrap_or(s);
    s.trim().to_lowercase()
}

/// Exact menu-title match after normalization — case-insensitive and ignoring a
/// trailing ellipsis, so `"Open"` matches `"Open…"`. This is the preferred pass;
/// it stops a segment from substring-matching a longer sibling (`"Edit"` must
/// not match the `"TextEdit"` app menu).
fn menu_title_eq(pattern: &str, title: &str) -> bool {
    normalize_menu_title(pattern) == normalize_menu_title(title)
}

/// Fuzzy fallback: normalized substring, or glob. Used only when no item matches
/// exactly, e.g. `"Open"` → `"Open Recent"` or `"Save*"` → `"Save As…"`.
fn menu_title_fuzzy(pattern: &str, title: &str) -> bool {
    let np = normalize_menu_title(pattern);
    let nt = normalize_menu_title(title);
    nt.contains(&np) || loki_core::query::glob_matches(pattern, title)
}

/// Lenient menu-title match: exact (normalized) OR fuzzy substring/glob.
#[cfg(test)]
fn menu_title_matches(pattern: &str, title: &str) -> bool {
    menu_title_eq(pattern, title) || menu_title_fuzzy(pattern, title)
}

/// Fire the `AXPress` action on an AX element.
fn perform_press(element: AXUIElementRef) -> LokiResult<()> {
    unsafe {
        let action = CFString::new(AX_PRESS_ACTION);
        let err = AXUIElementPerformAction(element, action.as_concrete_TypeRef());
        match err {
            AX_ERROR_SUCCESS => Ok(()),
            AX_ERROR_API_DISABLED => Err(LokiError::PermissionDenied),
            _ => Err(LokiError::Platform(format!(
                "AXPress failed (AXError {err})"
            ))),
        }
    }
}

/// Get a single element-valued attribute (e.g. `AXMenuBar`) as a raw ref.
fn get_element_attribute(element: AXUIElementRef, attribute: &str) -> Option<AXUIElementRef> {
    unsafe {
        let cf_attr = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value);
        if err != AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        Some(value as AXUIElementRef)
    }
}

// ── Attribute helpers ──

/// Get a single string attribute from an AX element.
fn get_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    unsafe {
        let cf_attr = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value);

        if err != AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        let type_id = CFGetTypeID(value);
        let result = if type_id == CFStringGetTypeID() {
            let cf_str = CFString::wrap_under_get_rule(value as CFStringRef);
            Some(cf_str.to_string())
        } else {
            None
        };

        CFRelease(value);
        result
    }
}

/// Get a boolean attribute from an AX element (defaults to false).
fn get_bool_attribute(element: AXUIElementRef, attribute: &str) -> bool {
    unsafe {
        let cf_attr = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value);

        if err != AX_ERROR_SUCCESS || value.is_null() {
            return false;
        }

        // CFBoolean is toll-free bridged; true == kCFBooleanTrue
        let result = value == core_foundation::boolean::kCFBooleanTrue as CFTypeRef;
        CFRelease(value);
        result
    }
}

/// Get the frame (position + size) of an AX element.
fn get_frame(element: AXUIElementRef) -> Option<ElementFrame> {
    let position = get_position(element)?;
    let size = get_size(element)?;
    Some(ElementFrame {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

fn get_position(element: AXUIElementRef) -> Option<CGPoint> {
    unsafe {
        let cf_attr = CFString::new("AXPosition");
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value);

        if err != AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        let mut point = CGPoint::default();
        let ok = AXValueGetValue(
            value,
            K_AX_VALUE_CG_POINT_TYPE,
            &mut point as *mut CGPoint as *mut c_void,
        );
        CFRelease(value);

        if ok {
            Some(point)
        } else {
            None
        }
    }
}

fn get_size(element: AXUIElementRef) -> Option<CGSize> {
    unsafe {
        let cf_attr = CFString::new("AXSize");
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value);

        if err != AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        let mut size = CGSize::default();
        let ok = AXValueGetValue(
            value,
            K_AX_VALUE_CG_SIZE_TYPE,
            &mut size as *mut CGSize as *mut c_void,
        );
        CFRelease(value);

        if ok {
            Some(size)
        } else {
            None
        }
    }
}

/// Get children of an AX element as raw refs.
fn get_children(element: AXUIElementRef) -> Vec<AXUIElementRef> {
    get_array_attribute(element, "AXChildren").unwrap_or_default()
}

/// Get an array-valued attribute, returning the raw CFTypeRef items.
fn get_array_attribute(
    element: AXUIElementRef,
    attribute: &str,
) -> LokiResult<Vec<AXUIElementRef>> {
    unsafe {
        let cf_attr = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value);

        match err {
            AX_ERROR_SUCCESS if !value.is_null() => {
                // value is a CFArray
                let array = value as core_foundation::array::CFArrayRef;
                let count = core_foundation::array::CFArrayGetCount(array);
                let mut items = Vec::with_capacity(count as usize);
                for i in 0..count {
                    let item = core_foundation::array::CFArrayGetValueAtIndex(array, i);
                    items.push(item as AXUIElementRef);
                }
                // Don't release the array — items are borrowed from it and we need them alive.
                // The caller's walk_tree scope keeps them valid.
                Ok(items)
            }
            AX_ERROR_API_DISABLED => Err(LokiError::PermissionDenied),
            AX_ERROR_NO_VALUE | AX_ERROR_ATTRIBUTE_UNSUPPORTED => Ok(Vec::new()),
            _ => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{menu_title_eq, menu_title_fuzzy, menu_title_matches};

    #[test]
    fn test_menu_title_eq_prefers_exact_not_substring() {
        // Regression: "Edit" must NOT exact-match the "TextEdit" app menu.
        // press_menu_path uses eq first, so this keeps top-level nav correct.
        assert!(!menu_title_eq("Edit", "TextEdit"));
        assert!(menu_title_eq("Edit", "Edit"));
        // ...but fuzzy would (wrongly) accept it — which is why eq goes first.
        assert!(menu_title_fuzzy("Edit", "TextEdit"));
    }

    #[test]
    fn test_menu_title_eq_ellipsis() {
        assert!(menu_title_eq("Open", "Open…"));
        assert!(menu_title_eq("Save As…", "Save As"));
    }

    #[test]
    fn test_menu_title_exact() {
        assert!(menu_title_matches("File", "File"));
        assert!(!menu_title_matches("File", "Edit"));
    }

    #[test]
    fn test_menu_title_ignores_trailing_ellipsis() {
        // User omits the ellipsis the app renders.
        assert!(menu_title_matches("Open File", "Open File…"));
        assert!(menu_title_matches("Open File", "Open File..."));
        // And the reverse: user types it, app omits it.
        assert!(menu_title_matches("Save As…", "Save As"));
    }

    #[test]
    fn test_menu_title_case_insensitive() {
        assert!(menu_title_matches("open file", "Open File…"));
        assert!(menu_title_matches("FILE", "File"));
    }

    #[test]
    fn test_menu_title_substring() {
        assert!(menu_title_matches("Open", "Open Recent"));
    }

    #[test]
    fn test_menu_title_glob() {
        assert!(menu_title_matches("Save*", "Save As…"));
    }

    #[test]
    fn test_menu_title_no_false_match() {
        assert!(!menu_title_matches("Close", "Open File…"));
    }
}
