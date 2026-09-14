//! A host-side framebuffer, and golden images taken from it.
//!
//! Its own crate because it is the opposite of what a backend is. A backend
//! draws on whatever hardware it was handed and builds for bare metal; this
//! reads and writes files, decodes PNG, and only ever runs on a laptop or a CI
//! machine.
//!
//! [`Framebuffer`] is an `embedded-graphics` `DrawTarget` like any other, so a
//! backend needs no knowledge of it at all: point one at this instead of a
//! panel and the same drawing lands somewhere a test can read.
//!
//! # A first run writes the golden and fails
//!
//! Deliberately, so nobody commits a picture they never looked at. Re-bless
//! with `UPDATE_SNAPSHOTS=1` and then actually open them.

#![deny(missing_docs)]

/// This crate's prose, compiled. Gated on `golden` as well as `doctest`: the
/// snippet calls `assert_screenshot`, which the feature gates, and the crate
/// must keep compiling its own doctests with the feature off — the shape
/// `xpui-simulator` consumes.
#[cfg(all(doctest, feature = "golden"))]
mod guides {
    #[doc = include_str!("../README.md")]
    pub mod readme {}
    #[doc = include_str!("../docs/reference.md")]
    pub mod reference {}
}

pub mod framebuffer;
#[cfg(feature = "golden")]
pub mod golden;

pub use framebuffer::Framebuffer;
#[cfg(feature = "golden")]
pub use golden::{assert_screenshot, check_screenshot};
