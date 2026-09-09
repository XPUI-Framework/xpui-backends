//! Confining a draw to the current clip, and the coordinate conversion that
//! takes an `xpui` rectangle into `embedded-graphics`.

use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use xpui::Rect;

use crate::backend::{Backend, Frame};

impl<D: DrawTarget> Backend<D> {
    /// Whether any of `rect` is inside the clip. Used only to skip work; the
    /// clipping itself is done by the target, not by arithmetic here.
    pub(crate) fn intersects_clip(&self, frame: &Frame<D>, rect: Rect) -> bool {
        let Some(clip) = frame.clip else {
            return true;
        };
        rect.x() < clip.x() + clip.width()
            && clip.x() < rect.x() + rect.width()
            && rect.y() < clip.y() + clip.height()
            && clip.y() < rect.y() + rect.height()
    }
}

/// Runs `$body` against a draw target that honours the current clip.
///
/// `DrawTargetExt::clipped` drops out-of-area pixels inside the target, which
/// is the only way to clip *text*: glyphs are rasterised by the font, and
/// intersecting rectangles beforehand cannot cut one in half.
///
/// A macro rather than a function because the two arms have different target
/// types and `DrawTarget` is not object-safe.
///
/// **Draws nothing while the display is on loan.** `Backend::loan_display`
/// takes it out so a caller can flush it across an `await`, and this is the one
/// place that has to know: a paint arriving in that window is discarded, which
/// is the cost the loan pays for holding no borrow.
macro_rules! with_clip {
    ($frame:expr, |$target:ident| $body:expr) => {{
        // Read before the display is borrowed, or the two borrows overlap.
        let clip = $frame.clip;
        if let Some(display) = $frame.display.as_mut() {
            match clip {
                Some(clip) => {
                    let area = $crate::clip::to_eg_rect(clip);
                    let mut clipped = display.clipped(&area);
                    let $target = &mut clipped;
                    let _ = $body;
                }
                None => {
                    let $target = display;
                    let _ = $body;
                }
            }
        }
    }};
}

pub(crate) use with_clip;

pub(crate) type EgPoint = embedded_graphics::geometry::Point;

pub(crate) fn to_eg_rect(rect: Rect) -> Rectangle {
    Rectangle::new(
        EgPoint::new(rect.x(), rect.y()),
        embedded_graphics::geometry::Size::new(
            rect.width().max(0) as u32,
            rect.height().max(0) as u32,
        ),
    )
}
