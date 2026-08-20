use super::{TranslateRequest, Translator};
use anyhow::{bail, Result};

/// macOS Dictionary Services lookup (`DCSCopyTextDefinition`).
pub struct AppleDictionary;

impl Translator for AppleDictionary {
    fn id(&self) -> &'static str {
        "Dictionary"
    }

    fn translate(&self, req: &TranslateRequest) -> Result<String> {
        #[cfg(target_os = "macos")]
        {
            return lookup_macos(req.text.trim());
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = req;
            bail!("Apple Dictionary is only available on macOS");
        }
    }
}

#[cfg(target_os = "macos")]
fn lookup_macos(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        bail!("Nothing to look up");
    }
    match ffi::definition(text) {
        Some(def) if !def.trim().is_empty() => Ok(def.trim().to_string()),
        _ => bail!("No definition in Apple Dictionary"),
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::c_void;
    use std::os::raw::{c_char, c_uchar};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    type CFStringRef = *const c_void;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithBytes(
            alloc: *const c_void,
            bytes: *const u8,
            num_bytes: isize,
            encoding: u32,
            is_external: c_uchar,
        ) -> CFStringRef;
        fn CFStringGetLength(s: CFStringRef) -> isize;
        fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
        fn CFStringGetCString(
            s: CFStringRef,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> c_uchar;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn DCSCopyTextDefinition(
            dictionary: *const c_void,
            text_string: CFStringRef,
            range: CFRange,
        ) -> CFStringRef;
        fn DCSGetTermRangeInString(
            dictionary: *const c_void,
            text_string: CFStringRef,
            offset: isize,
        ) -> CFRange;
    }

    fn valid_range(range: CFRange) -> bool {
        range.location >= 0 && range.length > 0
    }

    unsafe fn cfstring_from_utf8(s: &str) -> CFStringRef {
        unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                s.as_ptr(),
                s.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
                0,
            )
        }
    }

    unsafe fn cfstring_to_string(s: CFStringRef) -> Option<String> {
        if s.is_null() {
            return None;
        }
        unsafe {
            let len = CFStringGetLength(s);
            let max = CFStringGetMaximumSizeForEncoding(len, K_CF_STRING_ENCODING_UTF8) + 1;
            if max <= 1 {
                return None;
            }
            let mut buf = vec![0u8; max as usize];
            let ok = CFStringGetCString(
                s,
                buf.as_mut_ptr().cast::<c_char>(),
                max,
                K_CF_STRING_ENCODING_UTF8,
            );
            if ok == 0 {
                return None;
            }
            let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            String::from_utf8(buf[..end].to_vec()).ok()
        }
    }

    unsafe fn copy_definition(cf_text: CFStringRef, range: CFRange) -> Option<String> {
        if !valid_range(range) {
            return None;
        }
        let def = unsafe { DCSCopyTextDefinition(std::ptr::null(), cf_text, range) };
        let out = unsafe { cfstring_to_string(def) };
        if !def.is_null() {
            unsafe { CFRelease(def) };
        }
        out.filter(|s| !s.trim().is_empty())
    }

    pub fn definition(text: &str) -> Option<String> {
        let cf_text = unsafe { cfstring_from_utf8(text) };
        if cf_text.is_null() {
            return None;
        }
        let full = CFRange {
            location: 0,
            length: unsafe { CFStringGetLength(cf_text) },
        };
        let term = unsafe { DCSGetTermRangeInString(std::ptr::null(), cf_text, 0) };

        let single_token = !text.chars().any(char::is_whitespace);
        let out = if single_token {
            unsafe { copy_definition(cf_text, full) }
                .or_else(|| unsafe { copy_definition(cf_text, term) })
        } else {
            unsafe { copy_definition(cf_text, term) }
                .or_else(|| unsafe { copy_definition(cf_text, full) })
        };
        unsafe { CFRelease(cf_text) };
        out
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn looks_up_hello() {
        let Ok(out) = lookup_macos("hello") else {
            // Dictionary Services are unavailable in some sandbox/CI environments.
            return;
        };
        assert!(
            out.to_ascii_lowercase().contains("hello"),
            "unexpected definition: {out}"
        );
    }
}
