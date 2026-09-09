//! The one place this backend's shared state is reached through.
//!
//! `xpui::host::Host` requires `Sync`: `loop_` and `render` may run on
//! different tasks. This state sits behind interior mutability, so something
//! has to make the claim, and the `critical-section` feature chooses how: on,
//! this is a `critical_section::Mutex<RefCell<T>>` and `Backend` is `Sync` by
//! the compiler; off, a bare `RefCell<T>` and `Sync` only by an `unsafe impl`
//! that is unsound off one thread. A single-threaded loop does not want
//! interrupts masked on every text measurement, which is why it is a choice.
//!
//! **The guard is held for a whole draw call**, not a memory write: on a
//! display that writes straight through to the panel, `Canvas::clear` puts
//! the entire transfer inside the critical section. Narrowing it means
//! taking the display out rather than borrowing it — `Backend::loan_display`.

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

    /// Runs `body` with the state borrowed *shared*: the read-only paths —
    /// every `InputSource` query, and `screen_size` — may overlap without a
    /// panic.
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
