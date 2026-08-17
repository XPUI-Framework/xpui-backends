//! Turning Rust strings into the pointers the C ABI expects.

/// Copies `text` into a NUL-terminated buffer for the C++ side to read.
///
/// The C++ side reads it during the call and keeps nothing, so the buffer is
/// freed as soon as the call returns. A string with an interior NUL cannot be
/// represented as one, and comes back empty rather than silently truncated.
pub(crate) fn as_c(text: &str) -> alloc::ffi::CString {
    alloc::ffi::CString::new(text).unwrap_or_default()
}

/// The pointer for an optional label: null means "you decide".
pub(crate) fn optional(text: Option<&str>) -> Option<alloc::ffi::CString> {
    text.map(as_c)
}

pub(crate) fn ptr_of(text: &Option<alloc::ffi::CString>) -> *const u8 {
    text.as_ref()
        .map_or(core::ptr::null(), |text| text.as_ptr().cast())
}
