# Raw

The C ABI the Rust half draws through, in one module. Every function here is
implemented by `cpp/xpui_fui.cpp` against FreeInkUI and declared for C in
`cpp/xpui_fui.h`; the `testing` feature swaps in host doubles, so the Rust
half's tests link with no firmware. A screen never calls these:
[`Backend`](backend.md#xpui_fuibackend) does. They are listed for a firmware
that implements the ABI itself, and for a reader following a draw call across
the boundary.

The conventions every function shares:

- **Strings cross as `*const u8`**, NUL-terminated, not `*const c_char`,
  because `char`'s signedness is implementation-defined.
- **`safe`** marks a function that passes no pointer and cannot reach undefined
  behaviour once [`attach`](backend.md#xpui_fuiattach)'s contract holds.
  Everything that takes a pointer stays `unsafe`; its rule is in the table.
- **Three polarities.** In the framebuffer a set bit is white; in
  `xpui_fui_draw_image`'s bitmap bit 0 is ink; `black` non-zero is ink.
- **`draw_text` takes a top-left origin**, not a baseline.

A symbol added to the header, `src/raw.rs`, the C++ or the doubles without the
others is a link error or a corrupt call frame.

## Topics

| | |
|---|---|
| [Lifecycle and display](#lifecycle-and-display) | attaching a framebuffer, and pushing a frame to the panel |
| [Canvas](#canvas) | text, fills, strokes, lines, the dither, the scrim, clipping, bitmaps and icons |
| [Fonts](#fonts) | resolving a role and measuring text |
| [Theme](#theme) | theme values and the themed components |
| [`CellFn`](#xpui_fuirawcellfn) | the callback the list and the dialog read their strings through |

**Example — through the doubles**

```rust
use xpui_fui::raw;

let width = raw::xpui_fui_screen_width();
raw::xpui_fui_fill_rect(0, 0, width, 1, 1); // a rule across the top, in ink
// Safety: a NUL-terminated title that lives for the call, and a null subtitle.
unsafe { raw::xpui_fui_draw_header(c"Settings".as_ptr().cast(), core::ptr::null()) };
raw::xpui_fui_request_update();
```

## Lifecycle and display

| Function | What it does | Rule |
|---|---|---|
| `xpui_fui::raw::xpui_fui_attach` | Points the shim at the panel it should draw into. | `unsafe`: `framebuffer` points at `(width + 7) / 8 * height` writable bytes that stay valid while anything draws |
| `xpui_fui::raw::xpui_fui_request_update` | Asks the panel to show what has been drawn. | `safe` |

The framebuffer is 1 bit per pixel, MSB first, `(width + 7) / 8` bytes per
row, and a **set bit is white**: FreeInkUI's convention, the inverse of the
usual one. How `xpui_fui_request_update` reaches the glass, a hook or a weak
symbol, is in [firmware.md](../firmware.md#wiring-it-up).

## Canvas

| Function | What it does | Rule |
|---|---|---|
| `xpui_fui::raw::xpui_fui_screen_width` | The attached panel's width in pixels. | `safe` |
| `xpui_fui::raw::xpui_fui_screen_height` | The attached panel's height in pixels. | `safe` |
| `xpui_fui::raw::xpui_fui_clear` | Clears the whole panel to background. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_text` | Draws `text` with its **top-left** corner at `x`, `y` — not a baseline. | `unsafe`: `text` is NUL-terminated, never null |
| `xpui_fui::raw::xpui_fui_fill_rect` | Fills a rect; `black` non-zero is ink, zero is background. | `safe` |
| `xpui_fui::raw::xpui_fui_stroke_rect` | Outlines a rect in ink, one pixel wide, inside its bounds. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_line` | A one-pixel line in ink, both ends included. | `safe` |
| `xpui_fui::raw::xpui_fui_fill_rect_dither` | Fills with a 50% dither, clearing first, which reads as grey on one bit. | `safe` |
| `xpui_fui::raw::xpui_fui_scrim` | Adds ink on one checkerboard parity without clearing, so about half of whatever was behind survives. | `safe` |
| `xpui_fui::raw::xpui_fui_set_clip` | Confines drawing to a rect; a zero width or height lifts the clip. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_image` | Draws a 1-bpp bitmap. | `unsafe`: `bitmap` points at `(w + 7) / 8 * h` readable bytes |
| `xpui_fui::raw::xpui_fui_draw_icon` | Draws the icon for `role` with its top-left corner at `x`, `y`, at the size `xpui_fui_icon_size` answers. | `safe` |
| `xpui_fui::raw::xpui_fui_icon_size` | Edge length the host would actually draw, or 0 when it ships nothing. | `safe` |

A bitmap is row-major, MSB first, `(w + 7) / 8` bytes per row, and **bit 0 is
ink**, inverted from the framebuffer's convention. FreeInkUI has no clipping of
its own, so the shim implements `xpui_fui_set_clip` over the framebuffer, which
clips glyph pixels exactly. This build ships no icons: `xpui_fui_icon_size`
answers `0`, which reserves no space.

## Fonts

| Function | What it does | Rule |
|---|---|---|
| `xpui_fui::raw::xpui_fui_font` | Resolves a role to a font id, or 0 when this build ships none. | `safe` |
| `xpui_fui::raw::xpui_fui_text_width` | The width `text` paints at in `font_id` and `style`, in pixels. | `unsafe`: `text` is NUL-terminated; the shipped shim null-checks, a firmware's own may not |
| `xpui_fui::raw::xpui_fui_line_height` | The height one line of `font_id` occupies. | `safe` |

## Theme

| Function | What it does | Rule |
|---|---|---|
| `xpui_fui::raw::xpui_fui_metric` | One theme value, by `ThemeMetric`'s discriminant, in pixels. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_header` | The header band, where a null `title` or `subtitle` means nothing for that part. | `unsafe`: each pointer null or NUL-terminated |
| `xpui_fui::raw::xpui_fui_draw_sub_header` | A section heading in the rect, with an optional right-aligned value. | `unsafe`: `label` NUL-terminated, `right` null or NUL-terminated |
| `xpui_fui::raw::xpui_fui_draw_button_hints` | The four hint slots in meaning order. | `unsafe`: each label null or NUL-terminated |
| `xpui_fui::raw::xpui_fui_draw_progress_bar` | The themed progress bar, `current` of `total` along. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_slider` | The themed slider; `state` is `ControlState`'s discriminant. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_scroll_indicator` | The scroll indicator: how much of `content` the `visible` window shows, and how far down it sits. | `safe` |
| `xpui_fui::raw::xpui_fui_draw_list` | The themed list. | `unsafe`: `ctx` is what `cell` expects, alive for the call |
| `xpui_fui::raw::xpui_fui_draw_option_popup` | The themed dialog; `cell` is called back per option with `field` 0. | `unsafe`: `title` NUL-terminated, `ctx` what `cell` expects |
| `xpui_fui::raw::xpui_fui_option_popup_row_rect` | Where row `index` of the themed dialog lands, written into `out_xywh`. | `unsafe`: `title` NUL-terminated, `out_xywh` four writable `i32`s |

**Button hints.** A null label means the host's own word for that slot, chosen
by the `*_word` tag beside it; an empty one means blank. The slots come in the
order the ABI names them, so a firmware that lets the user remap its buttons
reorders the arguments before calling.

**The list.** `cell` is called back per row and field: `field` selects title
(0), subtitle (1) or value (2), and returning null omits that field. The
strings it returns belong to the caller and are valid only for the call, since
FreeInkUI's props borrow them rather than copying.

**The dialog's rows.** `xpui_fui_option_popup_row_rect` is recomputed from the
layout the painter uses, not read back from a hit buffer, so it answers before
the first paint and cannot go stale. It returns `0` when there is no such row.

## `xpui_fui::raw::CellFn`

Handed to the row and option callbacks so the C++ side can ask Rust for a cell without owning any of the strings.

```text
pub type CellFn = extern "C" fn(ctx: *mut c_void, index: i32, field: i32) -> *const u8
```

| Parameter | Meaning |
|---|---|
| `ctx` | Whatever the caller of `xpui_fui_draw_list` or `xpui_fui_draw_option_popup` passed alongside it. |
| `index` | The row, or the option, from 0. |
| `field` | For the list: title (0), subtitle (1) or value (2). For the dialog: always 0. |

It answers a NUL-terminated string that lives until the draw call returns, or
null to omit the field.
