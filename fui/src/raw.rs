//! The C ABI, in one place.
//!
//! Every symbol here is implemented by `cpp/xpui_fui.cpp` against FreeInkUI
//! and doubled in `testing/stubs.rs`. A mismatch is a link error at best and
//! a corrupt call frame at worst; `tests/abi.rs` compares the three.
//!
//! Strings cross as `*const u8` rather than `*const c_char`, because `char`'s
//! signedness is implementation-defined and the C++ side would otherwise need
//! a cast per argument. They are NUL-terminated either way.
//!
//! `safe fn` marks a call that passes no pointer and cannot reach undefined
//! behaviour, provided `attach`'s contract holds. Everything taking a raw pointer stays `unsafe`, so the keyword
//! keeps meaning "there is a rule here you must keep" rather than "this
//! crosses into C++", which is true of every line.

use core::ffi::c_void;

/// Handed to the row and option callbacks so the C++ side can ask Rust for a
/// cell without owning any of the strings.
pub type CellFn = extern "C" fn(ctx: *mut c_void, index: i32, field: i32) -> *const u8;

unsafe extern "C" {
    // -- lifecycle ----------------------------------------------------------
    /// Points the shim at the panel it should draw into.
    ///
    /// `framebuffer` is 1 bit per pixel, MSB first, `(width + 7) / 8` bytes
    /// per row, and a **set bit is white** — FreeInkUI's own convention, and
    /// the inverse of the usual one.
    ///
    /// # Safety
    /// `framebuffer` must point at `(width + 7) / 8 * height` writable bytes
    /// that stay valid for as long as anything draws.
    pub fn xpui_fui_attach(framebuffer: *mut u8, width: i32, height: i32);

    // -- canvas -------------------------------------------------------------
    /// The attached panel's width in pixels.
    pub safe fn xpui_fui_screen_width() -> i32;
    /// The attached panel's height in pixels.
    pub safe fn xpui_fui_screen_height() -> i32;
    /// Clears the whole panel to background.
    pub safe fn xpui_fui_clear();
    /// Draws `text` with its **top-left** corner at `x`, `y` — not a baseline.
    ///
    /// # Safety
    /// `text` must be NUL-terminated and valid for the call. The header says
    /// never null, and a firmware's own shim may dereference it.
    pub fn xpui_fui_draw_text(x: i32, y: i32, text: *const u8, font_id: i32, style: u8);
    /// Fills a rect; `black` non-zero is ink, zero is background.
    pub safe fn xpui_fui_fill_rect(x: i32, y: i32, w: i32, h: i32, black: u8);
    /// Outlines a rect in ink, one pixel wide, inside its bounds.
    pub safe fn xpui_fui_stroke_rect(x: i32, y: i32, w: i32, h: i32);
    /// A one-pixel line in ink, both ends included.
    pub safe fn xpui_fui_draw_line(x1: i32, y1: i32, x2: i32, y2: i32);
    /// Fills with a 50% dither, clearing first. Reads as grey on one bit.
    pub safe fn xpui_fui_fill_rect_dither(x: i32, y: i32, w: i32, h: i32, light: u8);
    /// Adds ink on one checkerboard parity without clearing, so about half of
    /// whatever was behind survives.
    pub safe fn xpui_fui_scrim(x: i32, y: i32, w: i32, h: i32);
    /// Confines drawing to a rect. A zero width or height lifts the clip.
    ///
    /// FreeInkUI has no clipping of its own, so the shim implements this
    /// itself over the framebuffer.
    pub safe fn xpui_fui_set_clip(x: i32, y: i32, w: i32, h: i32);
    /// Draws a 1-bpp bitmap. Row-major, MSB first, `(w + 7) / 8` bytes per
    /// row, and **bit 0 is ink** — inverted from the framebuffer's own
    /// convention, and from the usual one.
    ///
    /// # Safety
    /// `bitmap` must point at `(w + 7) / 8 * h` readable bytes for the call.
    pub fn xpui_fui_draw_image(bitmap: *const u8, x: i32, y: i32, w: i32, h: i32);
    /// Draws the icon for `role` with its top-left corner at `x`, `y`, at the
    /// size [`xpui_fui_icon_size`] answers.
    pub safe fn xpui_fui_draw_icon(role: u16, variant: u8, size: i32, x: i32, y: i32);
    /// Edge length the host would actually draw, or 0 when it ships nothing.
    pub safe fn xpui_fui_icon_size(role: u16, variant: u8, size: i32) -> i32;

    // -- fonts --------------------------------------------------------------
    /// Resolves a role to a font id, or 0 when this build ships none.
    pub safe fn xpui_fui_font(role: u8) -> i32;
    /// The width `text` paints at in `font_id` and `style`, in pixels.
    ///
    /// # Safety
    /// `text` must be NUL-terminated and valid for the call. The shipped shim
    /// null-checks, but the header promises nothing, so a firmware's own may
    /// dereference it.
    pub fn xpui_fui_text_width(font_id: i32, text: *const u8, style: u8) -> i32;
    /// The height one line of `font_id` occupies.
    pub safe fn xpui_fui_line_height(font_id: i32) -> i32;

    // -- theme --------------------------------------------------------------
    /// One theme value, by `ThemeMetric`'s discriminant, in pixels.
    pub safe fn xpui_fui_metric(metric: u8) -> i32;
    /// The header band. A null `title` or `subtitle` means nothing for that
    /// part.
    ///
    /// # Safety
    /// Each pointer must be null or NUL-terminated and valid for the call.
    pub fn xpui_fui_draw_header(title: *const u8, subtitle: *const u8);
    /// A section heading in the rect, with an optional right-aligned value.
    ///
    /// # Safety
    /// `label` must be NUL-terminated and `right` null or NUL-terminated, both
    /// valid for the call.
    pub fn xpui_fui_draw_sub_header(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        label: *const u8,
        right: *const u8,
    );
    /// The four hint slots in meaning order. A null label means the host's
    /// own word for that slot, chosen by the `*_word` tag beside it; an empty
    /// one means blank.
    ///
    /// # Safety
    /// Each pointer must be null or NUL-terminated and valid for the call.
    pub fn xpui_fui_draw_button_hints(
        back: *const u8,
        back_word: i32,
        confirm: *const u8,
        confirm_word: i32,
        previous: *const u8,
        previous_word: i32,
        next: *const u8,
        next_word: i32,
    );
    /// The themed progress bar, `current` of `total` along.
    pub safe fn xpui_fui_draw_progress_bar(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        current: u32,
        total: u32,
    );
    /// The themed slider; `state` is `ControlState`'s discriminant.
    pub safe fn xpui_fui_draw_slider(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        value: i32,
        max: i32,
        state: i32,
    );
    /// The scroll indicator: how much of `content` the `visible` window
    /// shows, and how far down it sits.
    pub safe fn xpui_fui_draw_scroll_indicator(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        content: i32,
        visible: i32,
        offset: i32,
    );
    /// The themed list. `cell` is called back per row and field; `field`
    /// selects title (0), subtitle (1) or value (2), and returning null omits
    /// that field.
    ///
    /// The strings the callback returns belong to the caller and are only
    /// valid for the duration of this call — FreeInkUI's props borrow them
    /// rather than copying, so nothing may retain them past the return.
    ///
    /// # Safety
    /// `ctx` must be what `cell` expects, alive for the call.
    pub fn xpui_fui_draw_list(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        rows: i32,
        selected: i32,
        cell: CellFn,
        ctx: *mut c_void,
    );
    /// The themed dialog; `cell` is called back per option with `field` 0.
    ///
    /// # Safety
    /// `title` must be NUL-terminated and `ctx` what `cell` expects, both
    /// alive for the call.
    pub fn xpui_fui_draw_option_popup(
        title: *const u8,
        count: i32,
        selected: i32,
        cell: CellFn,
        ctx: *mut c_void,
    );
    /// Where row `index` of that dialog lands, written into `out_xywh`.
    ///
    /// Recomputed from the same layout the painter uses rather than read back
    /// out of a hit buffer, so it answers before the first paint and cannot go
    /// stale. Returns 0 when there is no such row.
    ///
    /// # Safety
    /// `title` must be NUL-terminated and `out_xywh` must point at four
    /// writable `i32`s, both valid for the call.
    pub fn xpui_fui_option_popup_row_rect(
        title: *const u8,
        count: i32,
        index: i32,
        out_xywh: *mut i32,
    ) -> u8;

    // -- display ------------------------------------------------------------
    /// Asks the panel to show what has been drawn.
    pub safe fn xpui_fui_request_update();
}
