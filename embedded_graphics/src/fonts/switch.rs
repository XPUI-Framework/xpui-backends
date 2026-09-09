//! How a screen asks for the type to change.
//!
//! The framework must not know what a typeface is, so there is no seam in it
//! to carry this: a screen leaves a family here, and each backend picks it up
//! at the top of its next frame. **A standing choice, not a one-shot** — a
//! simulator holds one backend per board, and a choice consumed by the first
//! to see it would be silently undone by switching board. **Between frames,
//! not during one** — a family applied mid-frame leaves a screen measured in
//! one face and painted in another, and [`Backend::begin_frame`] is the one
//! moment nothing has been measured yet.
//!
//! [`Backend::begin_frame`]: crate::Backend::begin_frame

use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use super::Family;

/// The family the application has chosen, or null for "whatever each backend
/// was built with".
///
/// A pointer rather than an index into a list, because the list belongs to the
/// application: the backend is told which family to use and never has to hold
/// a registry of them. `AtomicPtr` and not a `static mut` so this is sound
/// without a safety contract nobody can check — and **load and store only**,
/// never `swap` or `compare_exchange`, because a Cortex-M0+ has no atomic
/// read-modify-write and those do not link there.
static CHOSEN: AtomicPtr<Family> = AtomicPtr::new(ptr::null_mut());

/// Sets the type in `family`, from the next frame, on every backend.
///
/// Takes the family itself rather than a name or an index, so there is nothing
/// to look up and nothing to disagree about: a caller can only ask for a
/// family it already holds.
pub fn request_family(family: &'static Family) {
    // `as *mut` because `AtomicPtr` has no shared-reference flavour. It is
    // never written through, and nothing here is `&mut`.
    CHOSEN.store(family as *const Family as *mut Family, Ordering::Relaxed);
    xpui::host::request_update();
}

/// The family the application has chosen, if it has chosen one.
pub(crate) fn chosen_family() -> Option<&'static Family> {
    let chosen = CHOSEN.load(Ordering::Relaxed);
    if chosen.is_null() {
        return None;
    }
    // Safety: only `request_family` ever writes here, and what it writes is a
    // `&'static Family` — so the pointer is aligned, initialised, and lives as
    // long as the program.
    Some(unsafe { &*(chosen as *const Family) })
}

/// Forgets the choice, so backends keep whatever they were built with.
///
/// For tests, which share this static with every other test in their binary: a
/// family one test chose would still be in force inside the next.
pub fn clear_chosen_family() {
    CHOSEN.store(ptr::null_mut(), Ordering::Relaxed);
}
