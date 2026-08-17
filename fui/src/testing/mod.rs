//! Host-side stand-ins for every C symbol.
//!
//! Without these the test binary does not link, because the real definitions
//! live in `cpp/` and are compiled by the consuming firmware. They also record
//! what crossed the boundary, which is the only way to test the marshalling —
//! that a `None` subtitle really does arrive as null, that a hint that was
//! blanked is not confused with one the host should label itself.
//!
//! This tests the **Rust half**. The C++ half is checked by compiling it; see
//! `cpp/README.md`.

mod calls;
mod stubs;

pub use calls::{Call, calls, reset};
pub use stubs::{HEIGHT, WIDTH};
