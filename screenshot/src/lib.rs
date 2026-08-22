//! A host-side framebuffer, and golden images taken from it.
//!
//! Its own crate because it is the opposite of what a backend is. A backend
//! draws on whatever hardware it was handed and builds for bare metal; this
//! reads and writes files, decodes PNG, and only ever runs on a laptop or a CI
//! machine. Keeping the two together made `xpui-embedded-graphics` a `std`
//! crate on the host by accident — `no_std` only when the target said so —
//! and put a PNG codec behind a feature of a crate that ships to bare metal.
//!
//! [`Framebuffer`] is an `embedded-graphics` `DrawTarget` like any other, so a
//! backend needs no knowledge of it at all: point one at this instead of a
//! panel and the same drawing lands somewhere a test can read.
//!
//! # A first run writes the golden and fails
//!
//! Deliberately, so nobody commits a picture they never looked at. Re-bless
//! with `UPDATE_SNAPSHOTS=1` and then actually open them.

pub mod framebuffer;
#[cfg(feature = "golden")]
pub mod golden;

pub use framebuffer::Framebuffer;
#[cfg(feature = "golden")]
pub use golden::{assert_screenshot, check_screenshot};
