//! Painting: every primitive `xpui` asks a backend for, forwarded to the shim.

use xpui::host::{Canvas, FontId, FontStyle, IconRef};
use xpui::{Point, Rect, Size};

use crate::backend::Backend;
use crate::marshal::as_c;
use crate::platform::Platform;
use crate::raw;

impl<P: Platform> Canvas for Backend<P> {
    fn screen_size(&self) -> Size {
        Size::new(raw::xpui_fui_screen_width(), raw::xpui_fui_screen_height())
    }

    fn clear(&self) {
        raw::xpui_fui_clear();
    }

    fn draw_text(&self, origin: Point, text: &str, font: FontId, style: FontStyle) {
        // A font this build compiled out measures zero and must draw nothing,
        // rather than painting at some arbitrary substitute size.
        if !font.is_available() {
            return;
        }
        let text = as_c(text);
        unsafe {
            raw::xpui_fui_draw_text(
                origin.x,
                origin.y,
                text.as_ptr().cast(),
                font.0,
                style as u8,
            )
        }
    }

    fn fill_rect(&self, rect: Rect, black: bool) {
        raw::xpui_fui_fill_rect(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            u8::from(black),
        );
    }

    fn stroke_rect(&self, rect: Rect) {
        raw::xpui_fui_stroke_rect(rect.x(), rect.y(), rect.width(), rect.height());
    }

    fn draw_line(&self, from: Point, to: Point) {
        raw::xpui_fui_draw_line(from.x, from.y, to.x, to.y);
    }

    fn fill_rect_dither(&self, rect: Rect, light: bool) {
        raw::xpui_fui_fill_rect_dither(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            u8::from(light),
        );
    }

    fn scrim(&self, rect: Rect) {
        raw::xpui_fui_scrim(rect.x(), rect.y(), rect.width(), rect.height());
    }

    fn set_clip(&self, rect: Option<Rect>) {
        // A zero-sized rect is how the ABI spells "no clip", so `None` becomes
        // one rather than needing a second symbol.
        let rect = rect.unwrap_or(Rect::new(0, 0, 0, 0));
        raw::xpui_fui_set_clip(rect.x(), rect.y(), rect.width(), rect.height());
    }

    fn draw_image(&self, origin: Point, data: &[u8], size: Size) {
        if data.is_empty() || size.width <= 0 || size.height <= 0 {
            return;
        }
        // The C++ side reads `(w + 7) / 8 * h` bytes, so a short slice would
        // walk off the end of ours.
        let needed = (size.width as usize).div_ceil(8) * size.height as usize;
        if data.len() < needed {
            return;
        }
        unsafe {
            raw::xpui_fui_draw_image(data.as_ptr(), origin.x, origin.y, size.width, size.height)
        }
    }

    fn draw_icon(&self, origin: Point, icon: IconRef) {
        raw::xpui_fui_draw_icon(icon.kind, icon.variant, icon.size, origin.x, origin.y);
    }

    fn icon_size(&self, icon: IconRef) -> i32 {
        raw::xpui_fui_icon_size(icon.kind, icon.variant, icon.size)
    }
}
