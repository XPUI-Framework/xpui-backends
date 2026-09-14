//! The backend, and the one call that points the C++ side at a framebuffer.

use crate::platform::Platform;
use crate::raw;

/// The backend, stateless because everything it needs lives on the C++ side,
/// bound once by [`attach`].
pub struct Backend<P: Platform + 'static> {
    pub(crate) platform: &'static P,
}

impl<P: Platform + 'static> Backend<P> {
    /// A backend forwarding input and the clock to `platform`.
    pub const fn new(platform: &'static P) -> Self {
        Backend { platform }
    }
}

/// Points the C++ side at the panel's framebuffer.
///
/// # Safety
/// `framebuffer` must be writable for `(width + 7) / 8 * height` bytes and
/// must outlive every subsequent draw. Call before installing the backend.
pub unsafe fn attach(framebuffer: *mut u8, width: i32, height: i32) {
    // Safety: the caller's, as documented above.
    unsafe { raw::xpui_fui_attach(framebuffer, width, height) }
}
