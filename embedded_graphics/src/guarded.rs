//! The one place this backend's shared state is reached through.
//!
//! `xpui::host::Host` requires `Sync`, and it requires it for a real reason:
//! `crates/xpui/src/screen/mod.rs` documents that `loop_` and `render` may run
//! on different tasks. This backend's state sits behind interior mutability,
//! which is not `Sync` on its own — so something has to make the claim.
//!
//! Two ways to make it, chosen by the `critical-section` feature:
//!
//! | feature | what this is | what `Backend` is |
//! |---|---|---|
//! | on | `critical_section::Mutex<RefCell<T>>` | `Sync` **by the compiler**, with no `unsafe impl` anywhere |
//! | off | a bare `RefCell<T>` | `Sync` only by an `unsafe impl` that is **unsound off one thread** |
//!
//! That is the whole of the difference, and it is why the feature exists
//! rather than the guard being unconditional: a desktop simulator and a
//! bare-metal loop that ticks and paints in one place are single-threaded, and
//! neither wants interrupts masked on every text measurement.
//!
//! One seam rather than two spellings at twenty-odd call sites, because a
//! guard that is only remembered at some of them guards nothing. The inner
//! field is private to this module, so that is enforceable rather than
//! aspirational: there is no way to reach the state except through here.
//!
//! # How long the guard is held
//!
//! **For a whole draw call, not for a memory write.** That matters on device,
//! and "briefly" would be the wrong word: `Canvas::clear` on a display that
//! writes straight through to the panel — the Tufty's parallel bus, say — puts
//! the entire transfer inside the critical section, with interrupts masked for
//! all of it. `draw_text` rasterises glyphs in there too, and a full-screen
//! `scrim` walks every pixel.
//!
//! On a display that owns a RAM framebuffer, like the Badger's, the same calls
//! are memory writes and much shorter — but still whole calls.
//!
//! That granularity is the price of a seam this simple. Narrowing it means
//! taking the display out rather than borrowing it, which is what
//! spec 08 — *an asynchronous present* — is about.

use core::cell::RefCell;

/// State that one accessor reaches at a time.
#[cfg(feature = "critical-section")]
pub(crate) struct Guarded<T>(critical_section::Mutex<RefCell<T>>);

/// State that one accessor reaches at a time — **if the caller keeps to one
/// thread**. See the module docs.
#[cfg(not(feature = "critical-section"))]
pub(crate) struct Guarded<T>(RefCell<T>);

impl<T> Guarded<T> {
    #[cfg(feature = "critical-section")]
    pub(crate) const fn new(value: T) -> Self {
        Guarded(critical_section::Mutex::new(RefCell::new(value)))
    }

    #[cfg(not(feature = "critical-section"))]
    pub(crate) const fn new(value: T) -> Self {
        Guarded(RefCell::new(value))
    }

    /// Runs `body` with the state borrowed exclusively.
    ///
    /// **`body` must not reach *this* state again.** A second borrow of the
    /// same `Guarded` panics; a borrow of a *different* one is fine, and the
    /// critical section itself is re-entrant on every implementation this
    /// repository targets. The distinction matters because `Backend::input`
    /// and `with_display` hand `body` to a caller.
    #[cfg(feature = "critical-section")]
    pub(crate) fn with<R>(&self, body: impl FnOnce(&mut T) -> R) -> R {
        critical_section::with(|cs| body(&mut self.0.borrow_ref_mut(cs)))
    }

    /// Runs `body` with the state borrowed exclusively. See the guarded form
    /// above; this is the same call with nothing keeping a second thread out.
    #[cfg(not(feature = "critical-section"))]
    pub(crate) fn with<R>(&self, body: impl FnOnce(&mut T) -> R) -> R {
        body(&mut self.0.borrow_mut())
    }

    /// Runs `body` with the state borrowed *shared*.
    ///
    /// The read-only paths — every `InputSource` query, and `screen_size` —
    /// took a shared borrow before this seam existed, and they keep one. Not
    /// a micro-optimisation: `with` would make two overlapping reads panic
    /// where they have always been legal, which is a narrower contract than
    /// ten public trait methods had.
    #[cfg(feature = "critical-section")]
    pub(crate) fn with_ref<R>(&self, body: impl FnOnce(&T) -> R) -> R {
        critical_section::with(|cs| body(&self.0.borrow_ref(cs)))
    }

    /// Runs `body` with the state borrowed shared. See above.
    #[cfg(not(feature = "critical-section"))]
    pub(crate) fn with_ref<R>(&self, body: impl FnOnce(&T) -> R) -> R {
        body(&self.0.borrow())
    }
}
