use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionAll,
};
use loki_core::{ElementFrame, WindowInfo};
use tracing::trace;

/// Fetch all windows from Core Graphics.
pub fn list_all_windows() -> Vec<WindowInfo> {
    let options = kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements;

    let Some(window_list) = copy_window_info(options, kCGNullWindowID) else {
        return Vec::new();
    };

    let mut windows = Vec::new();

    for i in 0..window_list.len() {
        let dict_ref = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(
                window_list.as_concrete_TypeRef(),
                i as _,
            )
        } as CFDictionaryRef;

        if let Some(info) = parse_window_dict(dict_ref) {
            windows.push(info);
        }
    }

    trace!(count = windows.len(), "listed windows");
    windows
}

fn parse_window_dict(dict: CFDictionaryRef) -> Option<WindowInfo> {
    let window_id = get_number(dict, "kCGWindowNumber")? as u32;
    let pid = get_number(dict, "kCGWindowOwnerPID")? as u32;
    let title = get_string(dict, "kCGWindowName").unwrap_or_default();
    let is_on_screen = get_bool(dict, "kCGWindowIsOnscreen").unwrap_or(false);

    let frame = get_bounds(dict).unwrap_or(ElementFrame {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    });

    // Skip windows with no size (menubar items, etc.)
    if frame.width <= 0.0 && frame.height <= 0.0 {
        return None;
    }

    let bundle_id = bundle_id_for_pid(pid);

    Some(WindowInfo {
        window_id,
        pid,
        title,
        bundle_id,
        frame,
        is_on_screen,
    })
}

fn get_number(dict: CFDictionaryRef, key: &str) -> Option<i64> {
    unsafe {
        let cf_key = CFString::new(key);
        let mut value: *const std::ffi::c_void = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            cf_key.as_CFTypeRef() as _,
            &mut value,
        ) != 0
        {
            let cf_num = CFNumber::wrap_under_get_rule(value as _);
            cf_num.to_i64()
        } else {
            None
        }
    }
}

fn get_string(dict: CFDictionaryRef, key: &str) -> Option<String> {
    unsafe {
        let cf_key = CFString::new(key);
        let mut value: *const std::ffi::c_void = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            cf_key.as_CFTypeRef() as _,
            &mut value,
        ) != 0
            && !value.is_null()
        {
            let type_id = core_foundation::base::CFGetTypeID(value as _);
            if type_id == core_foundation::string::CFStringGetTypeID() {
                let cf_str = CFString::wrap_under_get_rule(value as CFStringRef);
                Some(cf_str.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}

fn get_bool(dict: CFDictionaryRef, key: &str) -> Option<bool> {
    unsafe {
        let cf_key = CFString::new(key);
        let mut value: *const std::ffi::c_void = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            cf_key.as_CFTypeRef() as _,
            &mut value,
        ) != 0
            && !value.is_null()
        {
            let type_id = core_foundation::base::CFGetTypeID(value as _);
            if type_id == CFBoolean::type_id() {
                Some(value == core_foundation::boolean::kCFBooleanTrue as *const _)
            } else {
                let cf_num = CFNumber::wrap_under_get_rule(value as _);
                cf_num.to_i32().map(|n| n != 0)
            }
        } else {
            None
        }
    }
}

fn get_bounds(dict: CFDictionaryRef) -> Option<ElementFrame> {
    unsafe {
        let cf_key = CFString::new("kCGWindowBounds");
        let mut value: *const std::ffi::c_void = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            cf_key.as_CFTypeRef() as _,
            &mut value,
        ) != 0
            && !value.is_null()
        {
            let bounds_dict = value as CFDictionaryRef;
            let x = get_number(bounds_dict, "X").unwrap_or(0) as f64;
            let y = get_number(bounds_dict, "Y").unwrap_or(0) as f64;
            let width = get_number(bounds_dict, "Width").unwrap_or(0) as f64;
            let height = get_number(bounds_dict, "Height").unwrap_or(0) as f64;
            Some(ElementFrame {
                x,
                y,
                width,
                height,
            })
        } else {
            None
        }
    }
}

/// Look up the bundle identifier for a process via lsappinfo.
fn bundle_id_for_pid(pid: u32) -> Option<String> {
    let output = std::process::Command::new("lsappinfo")
        .args(["info", "-only", "bundleid", "-pid", &pid.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    // Output varies by macOS version:
    //   "bundleid"="com.apple.Finder"
    //   "CFBundleIdentifier"="com.apple.Finder"
    // Look for any line with an = and extract the value after it.
    for line in text.lines() {
        if let Some(val) = line.split('=').nth(1) {
            let val = val.trim().trim_matches('"');
            if !val.is_empty() && val != "[ NULL ]" && val != "(null)" {
                return Some(val.to_string());
            }
        }
    }
    None
}
