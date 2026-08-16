// The C ABI between xpui's Rust side and FreeInkUI.
//
// This header is the contract. `src/raw.rs` declares exactly these symbols and
// `xpui_fui.cpp` defines them; all three move together, and nothing checks
// that they agree, so a change here is a change in three places.
//
// Strings are NUL-terminated `const uint8_t*` rather than `const char*`,
// because `char`'s signedness is implementation-defined and Rust's `u8` is
// not. A null pointer is meaningful in several places and is documented per
// function — it is never the same as an empty string.
//
// Coordinates are logical: the origin is the top-left of the screen as the UI
// sees it, and any panel rotation is the DrawTarget's business.

#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// -- lifecycle ---------------------------------------------------------------

// Points the shim at the panel's framebuffer. Call once, before anything else.
//
// The buffer is 1 bit per pixel, MSB first, (width + 7) / 8 bytes per row, and
// a SET bit is WHITE — FreeInkUI's own convention, the inverse of the usual
// one. It must stay valid and writable for as long as anything draws.
void xpui_fui_attach(uint8_t* framebuffer, int32_t width, int32_t height);

// -- canvas ------------------------------------------------------------------

int32_t xpui_fui_screen_width(void);
int32_t xpui_fui_screen_height(void);

// Clears the whole panel to background, ignoring any clip.
void xpui_fui_clear(void);

// `style`: 0 regular, 1 bold, 2 italic, 3 bold-italic.
// `text` is never null. Drawn from its top-left corner, not its baseline.
void xpui_fui_draw_text(int32_t x, int32_t y, const uint8_t* text, int32_t font_id, uint8_t style);

// `black` non-zero fills with ink, zero fills with background.
void xpui_fui_fill_rect(int32_t x, int32_t y, int32_t w, int32_t h, uint8_t black);
void xpui_fui_stroke_rect(int32_t x, int32_t y, int32_t w, int32_t h);
void xpui_fui_draw_line(int32_t x1, int32_t y1, int32_t x2, int32_t y2);

// Clears the rect, then lays a 50% checkerboard over it. Reads as grey on one
// bit. `light` picks which parity, so two adjacent dithers can differ.
void xpui_fui_fill_rect_dither(int32_t x, int32_t y, int32_t w, int32_t h, uint8_t light);

// Adds ink on one checkerboard parity and clears NOTHING, so about half of
// what was already there survives and the region reads as grey with the
// content still legible. This is what dims the screen behind a dialog, and it
// is NOT the same as a dither.
//
// Always the same parity, so a region scrimmed twice is no darker than one
// scrimmed once — an overlay redrawn on a frame that did not clear must not
// creep towards solid black.
void xpui_fui_scrim(int32_t x, int32_t y, int32_t w, int32_t h);

// Confines subsequent drawing to a rect. A zero width or height lifts it.
// FreeInkUI has no clipping of its own, so this is the shim's own.
void xpui_fui_set_clip(int32_t x, int32_t y, int32_t w, int32_t h);

// A 1-bpp bitmap: row-major, MSB first, (w + 7) / 8 bytes per row, and BIT 0
// IS INK — inverted from the framebuffer's convention above. Reads exactly
// (w + 7) / 8 * h bytes.
void xpui_fui_draw_image(const uint8_t* bitmap, int32_t x, int32_t y, int32_t w, int32_t h);

// An icon by role. `role` is an opaque number meaning "the thing you use for
// sun"; the host picks the asset. `size` is the preferred edge length.
void xpui_fui_draw_icon(uint16_t role, uint8_t variant, int32_t size, int32_t x, int32_t y);

// The edge length that would actually be drawn, or 0 if this build ships
// nothing for that role. Returning 0 is how the caller knows to leave no space.
int32_t xpui_fui_icon_size(uint16_t role, uint8_t variant, int32_t size);

// -- fonts -------------------------------------------------------------------

// `role`: 0 interface, 1 small interface, 2 the reading face.
//
// Returns 0 when this build ships no such font — the caller then measures zero
// and draws nothing, rather than substituting some other size. **0 is reserved
// for that answer**, so an implementation whose own font handles start at 0
// (FreeInkUI's slots do) must offset them. Any id outside the range this
// function hands out must behave like 0.
int32_t xpui_fui_font(uint8_t role);

int32_t xpui_fui_text_width(int32_t font_id, const uint8_t* text, uint8_t style);
int32_t xpui_fui_line_height(int32_t font_id);

// -- theme -------------------------------------------------------------------

// A geometry value, tagged by xpui's ThemeMetric. The tags are, in order:
//   0 TopPadding      1 HeaderHeight        2 VerticalSpacing  3 ButtonHintsHeight
//   4 ContentSidePadding                    5 ContentTop       6 ContentBottom
//   7 ListRowHeight   8 ListRowHeightWithSubtitle              9 ProgressBarHeight
//  10 MinTouchSize   11 SliderKnobWidth    12 SliderKnobHeight
//  13 SliderSideInset                      14 ListRowGap      15 SubHeaderHeight
//  16 SpacingSmall
//
// Note 14: the numbering is NOT contiguous with the names' declaration order.
// Read them from this list, not from intuition.
//
// SliderKnobWidth, SliderKnobHeight and SliderSideInset must be the numbers
// the slider is actually PAINTED with — the caller converts a touch into a
// value using them, so a mismatch makes the knob lag the finger.
int32_t xpui_fui_metric(uint8_t metric);

// The title band. Either pointer may be null, meaning "nothing for that part".
void xpui_fui_draw_header(const uint8_t* title, const uint8_t* subtitle);

// A group heading. `label` is never null; `right` may be.
//
// `h` is the band the caller reserved, which is one line of the heading font.
// The label is top-aligned within it; an implementation that wants a rule
// under the heading has the rest of the band to put it in.
void xpui_fui_draw_sub_header(int32_t x, int32_t y, int32_t w, int32_t h, const uint8_t* label, const uint8_t* right);

// The four hints, given by meaning rather than by screen position — the host
// reorders them for the user's button layout.
//
// A NULL pointer means "your own standard label for this slot".
// An EMPTY STRING means the screen asked for that slot to be blank.
// These are different, and confusing them is the most likely bug in this file.
void xpui_fui_draw_button_hints(const uint8_t* back, const uint8_t* confirm, const uint8_t* previous,
                                const uint8_t* next);

void xpui_fui_draw_progress_bar(int32_t x, int32_t y, int32_t w, int32_t h, uint32_t current, uint32_t total);

void xpui_fui_draw_slider(int32_t x, int32_t y, int32_t w, int32_t h, int32_t value, int32_t max);

// The scroll indicator beside a scrolling region. Draw NOTHING when
// content <= visible: a full-height bar says there is more to see when there
// is not.
void xpui_fui_draw_scroll_indicator(int32_t x, int32_t y, int32_t w, int32_t h, int32_t content, int32_t visible,
                                    int32_t offset);

// Asks the caller for one cell.
//
// `field` selects the piece: 0 title, 1 subtitle, 2 value. For a dialog's
// options only field 0 is meaningful.
//
// Returns NULL when that row has no such field, which is how a one-line row is
// told apart from a two-line one. An empty string is NOT the same answer.
//
// The returned pointer belongs to the caller and is valid only until the
// drawing function that supplied this callback returns. Nothing may retain it.
typedef const uint8_t* (*xpui_fui_cell_fn)(void* ctx, int32_t index, int32_t field);

// The themed list. Draws rows from index 0 downward and stops at the first one
// that would not fit; `selected` is the row to highlight, or -1 for none.
//
// **Do not scroll to keep `selected` in view.** The caller owns scrolling: it
// translates this rect's origin and registers its touch rects with row 0 at
// the top of the rect it passed. An implementation that scrolled on its own
// would paint row N where the caller registered row 0 — every tap off by the
// scroll distance, and nothing about the screen looking wrong.
void xpui_fui_draw_list(int32_t x, int32_t y, int32_t w, int32_t h, int32_t rows, int32_t selected,
                        xpui_fui_cell_fn cell, void* ctx);

// A centred dialog. `selected` is the row to highlight, or -1 for none.
void xpui_fui_draw_option_popup(const uint8_t* title, int32_t count, int32_t selected, xpui_fui_cell_fn cell,
                                void* ctx);

// Where row `index` of that same dialog lands, as x, y, w, h.
//
// Must agree with what xpui_fui_draw_option_popup paints, to the pixel: this
// is what a touch is tested against, and a drift of a few pixels means tapping
// one row selects its neighbour. Compute it from the SAME layout the painter
// uses rather than deriving it again, and do not read it back out of a hit
// buffer — this is called before the first paint.
//
// Returns 0 and writes nothing when there is no such row.
uint8_t xpui_fui_option_popup_row_rect(const uint8_t* title, int32_t count, int32_t index, int32_t* out_xywh);

// -- display -----------------------------------------------------------------

// Asks the panel to show what has been drawn. E-ink does not refresh on its
// own. Called once per changed frame, never per draw.
void xpui_fui_request_update(void);

// Where an implementation that owns only a framebuffer hands off to the panel.
//
// `xpui_fui.cpp` draws into memory and has no display driver, so it defines
// this as a weak no-op and calls it from `xpui_fui_request_update`. A firmware
// overrides it with its own definition — push the framebuffer, trigger the
// waveform — and the linker prefers the strong symbol. An implementation that
// already talks to a panel can ignore this and drive it from
// `xpui_fui_request_update` directly.
void xpui_fui_present(void);

#ifdef __cplusplus
}  // extern "C"
#endif
