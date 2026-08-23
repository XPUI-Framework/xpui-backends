//! An [`xpui`] backend that draws through **FreeInkUI**.
//!
//! FreeInkUI is a header-only C++ component library for e-ink firmware. It
//! already knows what a list row, a dialog and a slider look like, so a screen
//! written against `xpui` and one written in C++ against FreeInkUI come out as
//! the same pixels — which is the whole reason to sit on it rather than beside
//! it.
//!
//! # What ships here
//!
//! Two halves that have to stay in step:
//!
//! | | |
//! |---|---|
//! | `src/` | the Rust `Host` implementation, over a C ABI ([`raw`]) |
//! | `cpp/` | that ABI implemented against FreeInkUI's own `DrawTarget` |
//!
//! The boundary runs both ways. [`raw`] is what Rust calls to draw;
//! [`lifecycle`] is what a host calls to run a screen, and those symbols are
//! defined here in Rust with `cpp/xpui_screen.h` as their declaration.
//!
//! The C++ half is deliberately **not** written against any particular
//! firmware's renderer. It binds to `freeink::ui::DisplayTarget`, which is
//! dependency-free and takes a plain 1-bit framebuffer, so any project that
//! links the FreeInk SDK can add these two files and be done. A firmware with
//! its own themed renderer can substitute its own implementation of the same
//! ABI instead.
//!
//! # Wiring it up
//!
//! ```rust,no_run
//! # use xpui::Button;
//! # use xpui_fui::{Backend, Platform};
//! # struct MyPlatform;
//! # impl Platform for MyPlatform {
//! #     fn millis(&self) -> u32 { 0 }
//! #     fn was_pressed(&self, _button: Button) -> bool { false }
//! #     fn is_pressed(&self, _button: Button) -> bool { false }
//! #     fn was_released(&self, _button: Button) -> bool { false }
//! #     fn has_left_right_keys(&self) -> bool { false }
//! # }
//! # let mut framebuffer = [0u8; 480 * 800 / 8];
//! // Once, with the panel's framebuffer.
//! unsafe { xpui_fui::attach(framebuffer.as_mut_ptr(), 480, 800) };
//!
//! static PLATFORM: MyPlatform = MyPlatform;
//! static BACKEND: Backend<MyPlatform> = Backend::new(&PLATFORM);
//! unsafe { xpui::host::install(&BACKEND) };
//! ```
//!
//! Add `cpp/xpui_fui.cpp` to the firmware's build and put FreeInkUI's include
//! directory on its path. Input and the clock are **not** here: they are
//! platform concerns, and a firmware supplies them by implementing
//! [`Platform`].

#![cfg_attr(target_os = "none", no_std)]

extern crate alloc;

use core::ffi::c_void;

mod backend;
mod canvas;
mod cells;
mod chrome;
pub mod lifecycle;
mod marshal;
mod platform;
pub mod raw;
mod traits;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use backend::{Backend, attach};
pub use platform::{NoInput, Platform};

use cells::cell_trampoline;

// A missing trait is caught here rather than at the install site.
const _: fn() = || {
    fn assert_host<T: xpui::host::Host>() {}
    assert_host::<Backend<NoInput>>();
};

/// Keeps the trampoline's signature honest against the ABI.
const _: raw::CellFn = cell_trampoline;

/// Unused, but it keeps `c_void` imported where the ABI needs it.
#[doc(hidden)]
pub type Context = *mut c_void;

/// The crate's prose, compiled.
///
/// A README that does not build is worse than none: this crate's only usage
/// example passed the wrong form to its own macro for as long as nothing
/// tried it.
#[cfg(doctest)]
mod guides {
    #[doc = include_str!("../README.md")]
    pub mod readme {}
    #[doc = include_str!("../docs/tutorial.md")]
    pub mod tutorial {}
}

/// The source of [`register_screen!`], for an ABI checker to parse.
///
/// The macro generates the factory functions a C++ application exports, so no
/// Rust file spells their signatures out — the only place they exist is the
/// macro's own body. `xpui-cpp` checks its `xpui_app.h` against them, and
/// after the split it cannot reach this file by path: a git dependency lands
/// in a cargo checkout directory, not beside the crate that reads it.
///
/// So the crate that owns the macro hands out its text rather than a sibling
/// guessing at a path. Behind `testing` because a `&'static str` of a source
/// file is bytes a firmware has no use for.
#[cfg(any(test, feature = "testing"))]
pub const LIFECYCLE_SOURCE: &str = include_str!("lifecycle.rs");
