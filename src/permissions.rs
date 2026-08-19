#[derive(Debug, Clone)]
pub struct PermissionStatus {
    pub accessibility_ok: bool,
    pub message: String,
}

pub fn current_status() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        let ok = macos_accessibility_trusted();
        PermissionStatus {
            accessibility_ok: ok,
            message: if ok {
                "Accessibility is granted.".to_string()
            } else {
                "macOS Accessibility is required to read selected text. Open System Settings → Privacy & Security → Accessibility and enable swtrans.".to_string()
            },
        }
    }

    #[cfg(target_os = "linux")]
    {
        PermissionStatus {
            accessibility_ok: true,
            message: "Linux: AT-SPI is used when available. On Wayland, bind a compositor shortcut to `swtrans translate-selection` because in-app global hotkeys are not available.".to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    {
        PermissionStatus {
            accessibility_ok: true,
            message: "Windows: UI Automation is used when the focused control exposes a text pattern.".to_string(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        PermissionStatus {
            accessibility_ok: true,
            message: String::new(),
        }
    }
}

pub fn accessibility_settings_url() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_accessibility_trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
pub fn become_accessory() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn become_accessory() {}
