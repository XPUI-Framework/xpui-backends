//! Painting: every primitive `xpui` asks a backend for, on a `DrawTarget`.

use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle};
use u8g2_fonts::types::{FontColor, VerticalPosition};

use xpui::host::{Canvas, FontId, FontStyle, IconRef};
use xpui::{Point, Rect, Size};

use crate::backend::Backend;
use crate::clip::{EgPoint, to_eg_rect, with_clip};
use crate::fonts;

impl<D: DrawTarget> Backend<D> {
    fn colour(&self, ink: bool) -> D::Color {
        if ink {
            self.palette.ink
        } else {
            self.palette.background
        }
    }

    fn fill(&self, rect: Rect, colour: D::Color) {
        let mut frame = self.frame.borrow_mut();
        if !self.intersects_clip(&frame, rect) {
            return;
        }
        let area = to_eg_rect(rect);
        // Errors are swallowed on purpose: a `DrawTarget` failing mid-frame is
        // a display fault, and there is nothing a UI framework can do about it
        // that is better than drawing the rest of the screen.
        with_clip!(frame, |target| target.fill_solid(&area, colour));
    }

    /// Paints `colour` on the pixels of `rect` whose coordinates sum to
    /// `parity`.
    ///
    /// One `draw_iter` rather than a pixel loop, so a target that batches gets
    /// to. Used for both dithering and the scrim, which differ only in whether
    /// the other parity is cleared first.
    fn stipple(&self, rect: Rect, colour: D::Color, parity: u32) {
        let mut frame = self.frame.borrow_mut();
        if !self.intersects_clip(&frame, rect) {
            return;
        }
        let pixels = (rect.y()..rect.y() + rect.height()).flat_map(move |y| {
            (rect.x()..rect.x() + rect.width())
                .filter(move |x| (*x as u32).wrapping_add(y as u32) % 2 == parity)
                .map(move |x| Pixel(EgPoint::new(x, y), colour))
        });
        with_clip!(frame, |target| target.draw_iter(pixels));
    }
}

impl<D: DrawTarget> Canvas for Backend<D> {
    fn screen_size(&self) -> Size {
        let bounds = self.frame.borrow().display.bounding_box();
        Size::new(bounds.size.width as i32, bounds.size.height as i32)
    }

    fn clear(&self) {
        let background = self.palette.background;
        let mut frame = self.frame.borrow_mut();
        // Straight through, ignoring the clip: clearing is a whole-screen act,
        // and the framework only calls it before anything else is drawn.
        let _ = frame.display.clear(background);
    }

    /// Text from its top-left, which is the convention the whole framework
    /// lays out with — a UI test resolves tap targets from the same origin.
    ///
    /// u8g2 draws from a baseline, so the origin is converted through
    /// [`fonts::baseline_offset`], which is chosen so every glyph in the face
    /// lands inside `[y, y + line_height)`.
    ///
    /// Painted piece by piece through [`fonts::pieces`] — the same walk
    /// [`fonts::text_width`] measures with, so what lands on the panel occupies
    /// exactly what was reserved for it. A string the face can draw whole is
    /// one piece and one call, which is every ordinary label.
    fn draw_text(&self, origin: Point, text: &str, font: FontId, style: FontStyle) {
        let resolved = self.fonts().face(font, style);
        let colour = FontColor::Transparent(self.palette.ink);
        let ink = self.palette.ink;
        let mut pen = origin.x;
        let mut frame = self.frame.borrow_mut();

        fonts::pieces(resolved, text, |piece| {
            match piece {
                fonts::Piece::Run { face, text } => {
                    let baseline = EgPoint::new(pen, origin.y + fonts::baseline_offset(face));
                    with_clip!(frame, |target| face.render(
                        text,
                        baseline,
                        VerticalPosition::Baseline,
                        colour,
                        target
                    ));
                }
                // Nothing in the chain has the character. A hollow box says so,
                // sitting on the baseline where the glyph would have been — a
                // label with a visible box in it reads as a missing glyph,
                // where one that silently drops a character reads as the wrong
                // words.
                fonts::Piece::Marker { width, height } => {
                    let baseline = origin.y + fonts::baseline_offset(resolved.renderer());
                    let area = Rect::new(pen, baseline - height, width, height);
                    let outline = to_eg_rect(area).into_styled(PrimitiveStyle::with_stroke(ink, 1));
                    with_clip!(frame, |target| outline.draw(target));
                }
            }
            pen += fonts::advance(piece);
        });
    }

    fn fill_rect(&self, rect: Rect, black: bool) {
        self.fill(rect, self.colour(black));
    }

    fn stroke_rect(&self, rect: Rect) {
        let colour = self.palette.ink;
        let mut frame = self.frame.borrow_mut();
        if !self.intersects_clip(&frame, rect) {
            return;
        }
        let outline = to_eg_rect(rect).into_styled(PrimitiveStyle::with_stroke(colour, 1));
        with_clip!(frame, |target| outline.draw(target));
    }

    fn draw_line(&self, from: Point, to: Point) {
        let colour = self.palette.ink;
        let mut frame = self.frame.borrow_mut();
        let line = Line::new(EgPoint::new(from.x, from.y), EgPoint::new(to.x, to.y))
            .into_styled(PrimitiveStyle::with_stroke(colour, 1));
        with_clip!(frame, |target| line.draw(target));
    }

    fn fill_rect_dither(&self, rect: Rect, light: bool) {
        // Cleared first, then patterned: this is a *fill*, so whatever was
        // behind it goes. `scrim` is the one that preserves.
        self.fill(rect, self.palette.background);
        self.stipple(rect, self.palette.ink, u32::from(light));
    }

    fn scrim(&self, rect: Rect) {
        // Ink on one parity only, and nothing cleared, so about half of what
        // was behind survives and the region reads as grey.
        self.stipple(rect, self.palette.ink, 0);
    }

    fn set_clip(&self, rect: Option<Rect>) {
        self.frame.borrow_mut().clip = rect;
    }

    fn draw_image(&self, origin: Point, data: &[u8], size: Size) {
        if size.width <= 0 || size.height <= 0 {
            return;
        }
        let stride = (size.width as usize).div_ceil(8);
        // Refused whole rather than drawn as far as the buffer goes. Skipping
        // the missing pixels one at a time paints a fragment of the image and
        // reports nothing, which reads as a corrupt asset rather than as a
        // caller passing the wrong size.
        if data.len() < stride * size.height as usize {
            return;
        }
        let ink = self.palette.ink;

        let mut frame = self.frame.borrow_mut();
        let pixels = (0..size.height).flat_map(move |row| {
            (0..size.width).filter_map(move |column| {
                let byte = *data.get(row as usize * stride + column as usize / 8)?;
                // Bit 0 is ink, inverted from the usual convention — the
                // format the framework documents, and the one e-ink panels
                // use, where a set bit is white.
                if byte & (0x80 >> (column % 8)) != 0 {
                    return None;
                }
                Some(Pixel(EgPoint::new(origin.x + column, origin.y + row), ink))
            })
        });
        with_clip!(frame, |target| target.draw_iter(pixels));
    }

    fn draw_icon(&self, origin: Point, icon: IconRef) {
        // Drawn from lines and rectangles rather than blitted from an asset
        // set: this backend has no assets, and a stored bitmap would cost
        // flash on a board that has two megabytes of it.
        xpui_chrome::draw_icon(origin, icon);
    }

    fn icon_size(&self, icon: IconRef) -> i32 {
        // 0 for an icon this crate cannot draw, which is how the framework
        // knows to reserve no space rather than leaving a hole.
        xpui_chrome::icon_size(icon)
    }
}
