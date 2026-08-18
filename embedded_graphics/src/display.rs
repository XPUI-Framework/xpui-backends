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
    /// [`with_display`](Backend::with_display) holds the guard for as long as
    /// its closure runs, so a flush that suspends cannot go inside one: a
    /// borrow held across an `await` is a second task arriving to find the
    /// state already borrowed, which under this workspace's `panic = "abort"`
    /// takes the firmware down. A DMA-backed panel — the whole point of an
    /// async driver — is exactly that case.
    ///
    /// So the display leaves rather than being borrowed. The caller owns it
    /// across the suspension, nothing is held, and the loan puts it back on
    /// drop. **Anything that paints while it is out is discarded**, which is
    /// the price: `Canvas::clear`, every primitive and every glyph become
    /// no-ops for that window. `screen_size` still answers, because layout
    /// measured against zero collapses without saying so.
    ///
    /// `None` when the display is already on loan. Two presents at once is a
    /// caller bug, and handing back a second loan would hand out two `&mut D`.
    ///
    /// # Two ways to lose the panel
    ///
    /// **Do not `mem::forget` a loan.** The display never comes back, and
    /// nothing says so: every paint is discarded from then on, `screen_size`
    /// keeps answering the right number so layout looks healthy, and the only
    /// symptom is a panel that stopped updating. That is the failure this
    /// repository takes most seriously, and it is safe code — so it is a
    /// documented hazard rather than something the type system can refuse.
    ///
    /// **Do not drop a loan inside a backend callback.** `Drop` puts the
    /// display back, which takes the guard, so a loan going out of scope
    /// inside [`input`](Backend::input)'s or `with_display`'s closure
    /// re-enters it and panics. It is the only type here whose destructor
    /// calls back into the backend, and it does so with nothing visible at the
    /// call site.
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
