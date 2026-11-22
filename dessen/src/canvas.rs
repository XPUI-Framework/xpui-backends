//! Painting and measuring: [`Canvas`] and [`TextMetrics`] over dessen's
//! drawing calls, which the firmware forwards to FreeInkUI's `DrawTarget`.

use dessen::{
    DSN_ALIGN_LEFT, DSN_BITMAP_CENTER, DSN_BITMAP_MASK, DSN_BLACK, DSN_DARK_GRAY, DSN_FONT_BODY,
    DSN_FONT_SMALL, DSN_FONT_TITLE, DSN_LIGHT_GRAY, DSN_SCRIM, DSN_TEXT_BOLD, DSN_WHITE,
};
use xpui::host::{Canvas, FontId, FontRole, FontStyle, IconRef, TextMetrics};
use xpui::{Point, Rect, Size};

use crate::Backend;
use crate::calls::{
    dsn_bitmap, dsn_clear, dsn_fill, dsn_line, dsn_line_height, dsn_measure_text, dsn_screen,
    dsn_set_clip, dsn_stroke, dsn_text_box,
};

/// The dessen font a [`FontId`] stands for.
///
/// Ids are dessen's font numbers plus one, so that none is `0`, which means
/// "none". The firmware's faces are fixed for a build and an app cannot load
/// its own, so a constant id names the same bytes for the app's whole life.
fn dessen_font(font: FontId) -> i32 {
    (font.0 - 1).clamp(DSN_FONT_SMALL, DSN_FONT_TITLE)
}

/// The dessen text flags for a style. Bold is the one dessen draws; an italic
/// asks the firmware for its upright face rather than nothing.
fn text_flags(style: FontStyle) -> i32 {
    if matches!(style, FontStyle::Bold | FontStyle::BoldItalic) {
        DSN_TEXT_BOLD
    } else {
        0
    }
}

fn fill(rect: Rect, color: i32) {
    dsn_fill(
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
        color,
    );
}

impl Canvas for Backend {
    fn screen_size(&self) -> Size {
        // The safe area, not the panel: dessen's coordinates start inside it.
        let packed = dsn_screen();
        Size::new((packed >> 16) as i32, (packed & 0xffff) as i32)
    }

    fn clear(&self) {
        dsn_clear();
    }

    fn draw_text(&self, origin: Point, text: &str, font: FontId, style: FontStyle) {
        // Top-left, as dessen's text box is: no baseline to convert. Width
        // and height 0 ask for the rest of the line and one line's height.
        let flags = text_flags(style);
        let font = dessen_font(font);
        dsn_text_box(
            origin.x,
            origin.y,
            0,
            0,
            text.as_bytes(),
            font,
            DSN_ALIGN_LEFT,
            1,
            flags,
        );
    }

    fn fill_rect(&self, rect: Rect, black: bool) {
        fill(rect, if black { DSN_BLACK } else { DSN_WHITE });
    }

    fn stroke_rect(&self, rect: Rect) {
        let (origin, size) = (rect.origin, rect.size);
        dsn_stroke(
            origin.x,
            origin.y,
            size.width,
            size.height,
            DSN_BLACK,
            1,
            0,
            0,
        );
    }

    fn draw_line(&self, from: Point, to: Point) {
        dsn_line(from.x, from.y, to.x, to.y, 1, DSN_BLACK);
    }

    fn fill_rect_dither(&self, rect: Rect, light: bool) {
        // FreeInkUI's gray paints are its dither patterns on a 1-bit panel.
        fill(rect, if light { DSN_LIGHT_GRAY } else { DSN_DARK_GRAY });
    }

    fn scrim(&self, rect: Rect) {
        // Light-gray ink over what is there. Whether the background shows
        // through is the firmware's renderer's; see docs/coverage.md.
        fill(rect, DSN_SCRIM);
    }

    fn set_clip(&self, rect: Option<Rect>) {
        // The firmware clips to the target and to the safe area; a negative
        // width lifts the clip. A refused clip leaves drawing unclipped, which
        // is all a target without clipping could do anyway.
        let _ = match rect {
            Some(r) => dsn_set_clip(
                r.origin.x,
                r.origin.y,
                r.size.width.max(0),
                r.size.height.max(0),
            ),
            None => dsn_set_clip(0, 0, -1, -1),
        };
    }

    fn draw_image(&self, origin: Point, data: &[u8], size: Size) {
        if size.width <= 0 || size.height <= 0 {
            return;
        }
        let bytes = (size.width as usize).div_ceil(8) * size.height as usize;
        let Some(data) = data.get(..bytes) else {
            return; // shorter than the size it claims: refused whole
        };
        // `xpui`'s bit 0 is ink, which is dessen's mask format: drawn straight
        // from the app's bytes, with no copy. Only ink is painted, as with
        // every dessen bitmap.
        let (w, h) = (size.width, size.height);
        dsn_bitmap(
            origin.x,
            origin.y,
            w,
            h,
            data,
            w,
            h,
            DSN_BITMAP_MASK,
            DSN_BITMAP_CENTER,
            DSN_BLACK,
            0,
        );
    }

    fn draw_icon(&self, origin: Point, icon: IconRef) {
        // Drawn from lines and rectangles: an app carries no icon assets.
        xpui_chrome::draw_icon(origin, icon);
    }

    fn icon_size(&self, icon: IconRef) -> i32 {
        xpui_chrome::icon_size(icon)
    }
}

impl TextMetrics for Backend {
    fn font(&self, role: FontRole) -> FontId {
        let font = match role {
            FontRole::UiSmall => DSN_FONT_SMALL,
            FontRole::Ui | FontRole::Reader => DSN_FONT_BODY,
        };
        FontId(font + 1)
    }

    fn text_width(&self, font: FontId, text: &str, style: FontStyle) -> i32 {
        // Measured by the firmware's own renderer, the one that paints it.
        let packed = dsn_measure_text(text.as_bytes(), dessen_font(font), text_flags(style));
        (packed >> 16) as i32
    }

    fn line_height(&self, font: FontId) -> i32 {
        dsn_line_height(dessen_font(font))
    }
}
