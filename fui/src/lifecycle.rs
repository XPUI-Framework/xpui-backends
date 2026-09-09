//! The lifecycle a C++ host drives a screen through.
//!
//! A host that owns its own screen stack holds one screen at a time as an
//! opaque handle and calls the six entry points below, declared for C++ in
//! `cpp/xpui_screen.h`. They live here rather than in `xpui` because they
//! are the C boundary.
//!
//! **The handle is boxed twice.** [`Driver`] is a trait object, so
//! `Box<dyn Driver>` is a fat pointer and cannot cross as one `void*`: the
//! inner box erases the screen type, the outer gives a thin pointer. Every
//! function here reverses exactly that, and getting it wrong is not a compile
//! error — it is a wild pointer.

use alloc::boxed::Box;
use core::ffi::c_void;

use xpui::screen::{Driver, Runtime, Screen};

/// Wraps a screen for a C++ host to own, returning the opaque handle.
///
/// The screen is live from here on and belongs to the caller, which must
/// eventually pass the handle to [`xpui_screen_destroy`] or [`reclaim`].
pub fn into_handle<S: Screen + 'static>(screen: S) -> *mut c_void {
    handle_for(Box::new(Runtime::new(screen)))
}

/// The same, for a screen whose type has already been erased.
///
/// This is what lets a Rust screen push another one onto a stack that lives in
/// C++: [`Navigator::present`](xpui::host::Navigator::present) is handed a
/// `Box<dyn Driver>`, and a host with no way to turn that into a handle can
/// only hand it back.
pub fn handle_for(screen: Box<dyn Driver>) -> *mut c_void {
    Box::into_raw(Box::new(screen)) as *mut c_void
}

/// Takes a screen back out of a handle the host declined.
///
/// The mirror of [`handle_for`], and the reason a `present` that crosses the
/// FFI can still honour its contract: a host that refuses the push has not
/// taken ownership, so the screen is reclaimed here and handed back to whoever
/// tried to present it rather than leaked.
///
/// # Safety
/// `handle` must be null, or a handle from [`into_handle`] or [`handle_for`]
/// that has not been destroyed or reclaimed already. Reclaiming one the host
/// did take frees a screen it is still driving.
pub unsafe fn reclaim(handle: *mut c_void) -> Option<Box<dyn Driver>> {
    if handle.is_null() {
        return None;
    }
    // Safety: the caller guarantees the handle came from `handle_for`, which
    // is the only thing that makes this cast and this box type correct.
    Some(*unsafe { Box::from_raw(handle as *mut Box<dyn Driver>) })
}

/// Runs `body` against the screen a handle names.
///
/// # Safety
/// `handle` must be null or a live handle from [`into_handle`] or
/// [`handle_for`], not yet destroyed or reclaimed.
unsafe fn with(handle: *mut c_void, body: impl FnOnce(&mut dyn Driver)) {
    if handle.is_null() {
        return;
    }
    // Safety: the caller's — a live handle from `handle_for`, which is what
    // makes this cast right. Running `body` is ordinary Rust.
    let driver = unsafe { (*(handle as *mut Box<dyn Driver>)).as_mut() };
    body(driver);
}

/// The screen is being shown.
///
/// Nothing is installed here: a host with one thread installs once, before
/// the first screen exists, and a second install would only be a second
/// chance to race.
///
/// # Safety
/// `handle` must be null or a live handle from [`into_handle`] or
/// [`handle_for`], not yet destroyed or reclaimed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xpui_screen_on_enter(handle: *mut c_void) {
    // Safety: the caller's, as documented above.
    unsafe { with(handle, |driver| driver.on_enter()) }
}

/// One frame of input. Call once per host frame, before rendering.
///
/// # Safety
/// `handle` must be null or a live handle from [`into_handle`] or
/// [`handle_for`], not yet destroyed or reclaimed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xpui_screen_loop(handle: *mut c_void) {
    // Safety: the caller's, as documented above.
    unsafe { with(handle, |driver| driver.loop_()) }
}

/// The screen is being taken off the stack.
///
/// # Safety
/// `handle` must be null or a live handle from [`into_handle`] or
/// [`handle_for`], not yet destroyed or reclaimed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xpui_screen_on_exit(handle: *mut c_void) {
    // Safety: the caller's, as documented above.
    unsafe { with(handle, |driver| driver.on_exit()) }
}

/// Paints the screen into whatever `xpui_fui_attach` was given. No renderer
/// argument: what paints is the installed host, and a parameter nothing
/// reads can go out of step with the header unnoticed.
///
/// # Safety
/// `handle` must be null or a live handle from [`into_handle`] or
/// [`handle_for`], not yet destroyed or reclaimed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xpui_screen_render(handle: *mut c_void) {
    // Safety: the caller's, as documented above.
    unsafe { with(handle, |driver| driver.render()) }
}

/// Offers the system home gesture. Returns non-zero when the screen took it.
///
/// # Safety
/// `handle` must be null or a live handle from [`into_handle`] or
/// [`handle_for`], not yet destroyed or reclaimed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xpui_screen_home_gesture(handle: *mut c_void) -> u8 {
    let mut claimed = false;
    // Safety: the caller's, as documented above.
    unsafe { with(handle, |driver| claimed = driver.handle_home_gesture()) }
    u8::from(claimed)
}

/// Drops the screen. The handle is dangling afterwards.
///
/// # Safety
/// `handle` must be null, or a handle from [`into_handle`] or [`handle_for`]
/// that has not been destroyed or reclaimed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xpui_screen_destroy(handle: *mut c_void) {
    // Safety: the caller's, as documented above.
    drop(unsafe { reclaim(handle) })
}

/// Exports a C factory for a screen, so a C++ host can create one by name.
///
/// The screen type never crosses the boundary — only the handle does — so the
/// host needs no header describing it, and adding a screen is one line here
/// plus one declaration on the C++ side.
///
/// ```rust
/// use xpui::screen::Screen;
/// use xpui::{NavigationScreen, Text, View};
/// use xpui_fui::register_screen;
///
/// struct MainMenu;
///
/// impl MainMenu {
///     // The macro calls this, so a registered screen needs one.
///     fn new() -> Self {
///         MainMenu
///     }
/// }
///
/// impl Screen for MainMenu {
///     type Message = ();
///     fn body(&self) -> impl View<()> {
///         NavigationScreen::new(Text::new("Main menu")).title("Menu")
///     }
///     fn update(&mut self, _message: ()) {}
/// }
///
/// register_screen!(MainMenu, xpui_app_create_menu);
///
/// // What C++ calls, and what it gets back.
/// let handle = xpui_app_create_menu();
/// assert!(!handle.is_null());
/// // Safety: a live handle from the factory, destroyed exactly once.
/// unsafe { xpui_fui::lifecycle::xpui_screen_destroy(handle) };
/// ```
#[macro_export]
macro_rules! register_screen {
    ($screen:ty, $factory:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $factory() -> *mut core::ffi::c_void {
            $crate::lifecycle::into_handle(<$screen>::new())
        }
    };
}
