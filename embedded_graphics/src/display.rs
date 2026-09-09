//! Reaching the panel: borrowing the display, lending it out, taking it back.
//!
//! Its own file because it is its own job, and one with a rule the rest of the
//! backend does not have: **the display can be absent.** While it is on loan
//! for a present there is nothing to draw on, so every paint is discarded and
//! `screen_size` is answered from a figure captured at construction.
//!
//! Everything that knows the display can be missing lives here, apart from the
//! single `if let` in `clip.rs`'s `with_clip!`.

use embedded_graphics::prelude::*;

use crate::backend::Backend;

impl<D: DrawTarget> Backend<D> {
    /// Borrows the display, for pushing the framebuffer to a panel.
    ///
    /// Same rule as [`input`](Backend::input): the closure holds this
    /// backend's state, so it must not draw through the backend while inside.
    /// Flush the panel, read the pixels, return.
    ///
    /// # Panics
    /// If the display is on [loan](Backend::loan_display). Presenting twice at
    /// once is a caller bug, and the two calls would otherwise each believe
    /// they had the panel. Named rather than silent, because a present that
    /// quietly did nothing is a stale frame with no other symptom.
    pub fn with_display<R>(&self, body: impl FnOnce(&mut D) -> R) -> R {
        self.frame.with(|frame| {
            let display = frame
                .display
                .as_mut()
                .expect("with_display while the display is on loan for a present");
            body(display)
        })
    }

    /// Takes the display **out** of the backend, until the loan is dropped.
    ///
    /// A flush that suspends cannot go inside
    /// [`with_display`](Backend::with_display), which holds the guard for as
    /// long as its closure runs: a borrow held across an
    /// `await` is a second task arriving to find the state already borrowed,
    /// which under `panic = "abort"` takes the firmware down. So the display
    /// leaves, the caller owns it across the suspension, and the loan puts it
    /// back on drop. **Anything that paints while it is out is discarded**;
    /// `screen_size` still answers. `None` when the display is already out.
    ///
    /// **Do not `mem::forget` a loan**: the display never comes back, every
    /// paint is discarded from then on, and the only symptom is a panel that
    /// stopped updating. **Do not drop a loan inside a backend callback**:
    /// `Drop` puts the display back, which takes the guard, and re-enters it.
    pub fn loan_display(&self) -> Option<DisplayLoan<'_, D>> {
        let display = self.frame.with(|frame| frame.display.take())?;
        Some(DisplayLoan {
            backend: self,
            display: Some(display),
        })
    }

    /// Puts a loaned display back. The inverse of [`loan_display`].
    ///
    /// [`loan_display`]: Backend::loan_display
    fn return_display(&self, display: D) {
        self.frame.with(|frame| frame.display = Some(display));
    }
}

/// A display that has been taken out of its backend, and goes back on drop.
///
/// Held across an `await` on purpose — that is what it is for — so it borrows
/// the backend only shared, and a shared borrow of a `Sync` backend is `Send`.
pub struct DisplayLoan<'a, D: DrawTarget> {
    backend: &'a Backend<D>,
    /// Always `Some` until `Drop` takes it. An `Option` because a destructor
    /// cannot move out of a field any other way.
    display: Option<D>,
}

impl<D: DrawTarget> core::ops::Deref for DisplayLoan<'_, D> {
    type Target = D;

    fn deref(&self) -> &D {
        self.display
            .as_ref()
            .expect("a loan holds its display until drop")
    }
}

impl<D: DrawTarget> core::ops::DerefMut for DisplayLoan<'_, D> {
    fn deref_mut(&mut self) -> &mut D {
        self.display
            .as_mut()
            .expect("a loan holds its display until drop")
    }
}

impl<D: DrawTarget> Drop for DisplayLoan<'_, D> {
    fn drop(&mut self) {
        if let Some(display) = self.display.take() {
            self.backend.return_display(display);
        }
    }
}
