use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
}

/// Check if this process has accessibility permission.
pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Prompt the user for accessibility permission if not already granted.
/// Returns true if permission is granted (either already or newly).
pub fn request_trust() -> bool {
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();

    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);

    unsafe { AXIsProcessTrustedWithOptions(options.as_CFTypeRef()) }
}
