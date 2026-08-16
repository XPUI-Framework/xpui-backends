//! What this backend actually puts in the framebuffer.
//!
//! Everything else in this repo tests intent: which call was made, with which
//! rectangle. This file is the one place that checks the pixels, because a
//! backend is precisely the thing that turns one into the other.

use xpui::host::{Canvas, FontRole, FontStyle, TextMetrics};
use xpui::{Point, Rect, Size};
use xpui_eg::Framebuffer as TestDisplay;
use xpui_eg::{Backend, Palette};

const WIDTH: i32 = 200;
const HEIGHT: i32 = 120;

/// A backend over a fresh framebuffer, and a way to read it back.
///
/// The backend owns the display, so the assertions go through `with_display`
/// rather than keeping a second handle to it.
fn backend() -> Backend<TestDisplay> {
    Backend::new(TestDisplay::new(WIDTH, HEIGHT), Palette::INK_IS_ON)
}

fn read<R>(backend: &Backend<TestDisplay>, body: impl FnOnce(&TestDisplay) -> R) -> R {
    backend.with_display(|display| body(display))
}

// -- the basics ------------------------------------------------------------

#[test]
fn the_screen_size_is_the_displays_own() {
    let backend = backend();
    assert_eq!(backend.screen_size(), Size::new(WIDTH, HEIGHT));
}

#[test]
fn filling_with_ink_and_with_background_are_opposites() {
    let backend = backend();
    let rect = Rect::new(10, 10, 20, 20);

    backend.fill_rect(rect, true);
    assert_eq!(read(&backend, |d| d.ink_in(10, 10, 20, 20)), 400);

    backend.fill_rect(rect, false);
    assert_eq!(
        read(&backend, |d| d.ink_in(10, 10, 20, 20)),
        0,
        "background clears what ink laid down"
    );
}

#[test]
fn a_stroked_rect_is_an_outline_not_a_fill() {
    let backend = backend();
    backend.stroke_rect(Rect::new(10, 10, 20, 20));

    assert!(read(&backend, |d| d.get(10, 10)), "corner is inked");
    assert!(read(&backend, |d| d.get(29, 29)), "and the far corner");
    assert!(
        read(&backend, |d| !d.get(20, 20)),
        "but the middle is untouched"
    );
}

// -- the two that only mean something on one bit ---------------------------

/// A dither is a *fill*: it clears first, so whatever was behind it goes.
#[test]
fn a_dither_erases_what_was_behind_it() {
    let backend = backend();
    let rect = Rect::new(0, 0, 40, 40);

    backend.fill_rect(rect, true);
    assert_eq!(read(&backend, |d| d.ink_in(0, 0, 40, 40)), 1600);

    backend.fill_rect_dither(rect, false);
    let ink = read(&backend, |d| d.ink_in(0, 0, 40, 40));
    assert_eq!(ink, 800, "exactly half the pixels survive, on one parity");
}

/// A scrim is not a fill: it adds ink on one parity and clears nothing, so
/// roughly half of what was behind stays legible. That is what makes text
/// under a dimmed overlay still readable.
#[test]
fn a_scrim_preserves_what_was_behind_it() {
    let backend = backend();
    let rect = Rect::new(0, 0, 40, 40);

    // A column of ink straddling both parities: half of it sits where the
    // scrim will paint anyway, half where it will not.
    backend.fill_rect(Rect::new(1, 0, 1, 40), true);
    let survivors: Vec<i32> = (0..40)
        .filter(|y| (1 + y) % 2 != 0)
        .filter(|y| read(&backend, |d| d.get(1, *y)))
        .collect();
    assert_eq!(
        survivors.len(),
        20,
        "half the column is off the scrim parity"
    );

    backend.scrim(rect);

    for y in &survivors {
        assert!(
            read(&backend, |d| d.get(1, *y)),
            "ink at (1,{y}) was behind the scrim and must have survived it"
        );
    }
    assert_eq!(
        read(&backend, |d| d.ink_in(0, 0, 40, 40)),
        800 + survivors.len(),
        "the scrim painted one whole parity and left the other alone — \
         so the total is its 800 plus the {} pixels already there",
        survivors.len()
    );
}

/// The two differ in exactly one way, and it is the reason both exist.
#[test]
fn dither_and_scrim_differ_only_in_whether_they_clear() {
    let backend = backend();
    let rect = Rect::new(0, 0, 20, 20);

    backend.fill_rect(rect, true);
    backend.scrim(rect);
    let scrimmed = read(&backend, |d| d.ink_in(0, 0, 20, 20));

    backend.fill_rect(rect, true);
    backend.fill_rect_dither(rect, false);
    let dithered = read(&backend, |d| d.ink_in(0, 0, 20, 20));

    assert_eq!(scrimmed, 400, "a scrim over solid ink leaves it solid");
    assert_eq!(dithered, 200, "a dither over solid ink halves it");
}

// -- clipping --------------------------------------------------------------

#[test]
fn a_clip_confines_drawing_to_its_rect() {
    let backend = backend();
    backend.set_clip(Some(Rect::new(0, 0, 50, 50)));
    backend.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), true);

    assert_eq!(
        read(&backend, |d| d.ink_count()),
        2500,
        "a full-screen fill under a 50x50 clip paints 50x50"
    );
}

#[test]
fn clearing_the_clip_restores_the_whole_panel() {
    let backend = backend();
    backend.set_clip(Some(Rect::new(0, 0, 10, 10)));
    backend.set_clip(None);
    backend.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), true);

    assert_eq!(
        read(&backend, |d| d.ink_count()),
        (WIDTH * HEIGHT) as usize,
        "the clip was lifted"
    );
}

#[test]
fn a_draw_entirely_outside_the_clip_paints_nothing() {
    let backend = backend();
    backend.set_clip(Some(Rect::new(0, 0, 10, 10)));
    backend.fill_rect(Rect::new(100, 100, 20, 20), true);
    assert_eq!(read(&backend, |d| d.ink_count()), 0);
}

/// The scrim goes through the same clip as everything else — an overlay that
/// dimmed past its own band would darken the chrome around it.
#[test]
fn a_clip_confines_the_scrim_too() {
    let backend = backend();
    backend.set_clip(Some(Rect::new(0, 0, 20, 20)));
    backend.scrim(Rect::new(0, 0, WIDTH, HEIGHT));
    assert_eq!(read(&backend, |d| d.ink_count()), 200);
}

// -- text ------------------------------------------------------------------

/// Measurement has to be exact, not close. A backend that guesses lays out
/// correctly and paints off the edge of the panel.
#[test]
fn text_measures_exactly_what_it_draws() {
    let backend = backend();
    let font = backend.font(FontRole::Ui);

    let width = backend.text_width(font, "Hello", FontStyle::Regular);
    backend.draw_text(Point::new(0, 0), "Hello", font, FontStyle::Regular);

    let height = backend.line_height(font);
    let inside = read(&backend, |d| d.ink_in(0, 0, width, height));
    let outside = read(&backend, |d| d.ink_count()) - inside;

    assert!(inside > 0, "something was drawn");
    assert_eq!(outside, 0, "and none of it fell outside the measured box");
}

#[test]
fn text_width_grows_with_the_text() {
    let backend = backend();
    let font = backend.font(FontRole::Ui);
    let one = backend.text_width(font, "A", FontStyle::Regular);
    let three = backend.text_width(font, "AAA", FontStyle::Regular);

    assert!(one > 0);
    assert!(three > one, "three characters are wider than one");
    assert_eq!(backend.text_width(font, "", FontStyle::Regular), 0);
}

/// Measured in characters, not bytes, or every accented word comes out wider
/// than it is painted.
#[test]
fn text_width_counts_characters_not_bytes() {
    let backend = backend();
    let font = backend.font(FontRole::Ui);

    assert_eq!(
        backend.text_width(font, "ééé", FontStyle::Regular),
        backend.text_width(font, "abc", FontStyle::Regular),
        "three characters measure the same however many bytes they take"
    );
}

/// Each role must resolve to something the backend can actually draw with, and
/// never to id 0 — the framework reads that as "this build ships no such font"
/// and silently draws nothing.
#[test]
fn every_font_role_resolves() {
    let backend = backend();
    for role in [FontRole::Ui, FontRole::UiSmall, FontRole::Reader] {
        let font = backend.font(role);
        assert!(font.is_available(), "{role:?} resolved to nothing");
        assert!(backend.line_height(font) > 0, "{role:?} has no height");
    }
}

#[test]
fn the_small_role_is_smaller_than_the_ui_role() {
    let backend = backend();
    let ui = backend.font(FontRole::Ui);
    let small = backend.font(FontRole::UiSmall);
    assert!(backend.line_height(small) < backend.line_height(ui));
}

// -- bitmaps ---------------------------------------------------------------

/// The format the framework documents is inverted from the usual one: bit 0 is
/// ink. Getting this backwards produces a photographic negative, which looks
/// deliberate enough that nobody questions it.
#[test]
fn a_bitmap_treats_a_clear_bit_as_ink() {
    let backend = backend();
    // 8x1: the left half clear (ink), the right half set (paper).
    let data = [0b0000_1111u8];
    backend.draw_image(Point::new(0, 0), &data, Size::new(8, 1));

    for x in 0..4 {
        assert!(read(&backend, |d| d.get(x, 0)), "bit {x} was clear, so ink");
    }
    for x in 4..8 {
        assert!(
            read(&backend, |d| !d.get(x, 0)),
            "bit {x} was set, so background"
        );
    }
}

#[test]
fn a_bitmap_row_starts_on_a_byte_boundary() {
    let backend = backend();
    // 9 pixels wide, so each row is two bytes: one full and one with a single
    // meaningful bit. Row 0 all ink, row 1 all paper.
    let data = [0b0000_0000u8, 0b0111_1111, 0b1111_1111, 0b1111_1111];
    backend.draw_image(Point::new(0, 0), &data, Size::new(9, 2));

    assert_eq!(read(&backend, |d| d.ink_in(0, 0, 9, 1)), 9, "row 0 is ink");
    assert_eq!(
        read(&backend, |d| d.ink_in(0, 1, 9, 1)),
        0,
        "row 1 starts at the next byte boundary, not bit 9"
    );
}

// -- palette ---------------------------------------------------------------

/// The framework is monochrome, but the backend need not be. Swapping the
/// palette inverts the panel with no change to any screen.
#[test]
fn the_palette_decides_which_colour_is_ink() {
    let inverted = Backend::new(TestDisplay::new(WIDTH, HEIGHT), Palette::INK_IS_OFF);

    inverted.fill_rect(Rect::new(0, 0, 10, 10), true);
    let ink = inverted.with_display(|d| d.ink_in(0, 0, 10, 10));

    assert_eq!(
        ink, 0,
        "with the palette inverted, painting ink sets the display's off colour"
    );
}

// -- repaint ---------------------------------------------------------------

#[test]
fn asking_for_a_repaint_marks_the_backend_dirty() {
    use xpui::host::Chrome;

    let backend = backend();
    backend.clear_dirty();
    assert!(!backend.is_dirty());

    backend.request_update();
    assert!(
        backend.is_dirty(),
        "the framework asked, the backend noted it"
    );
}

// -- what a review found could not fail ------------------------------------

/// `text_measures_exactly_what_it_draws` only proves measurement is an upper
/// bound — a `text_width` returning ten times the truth passes it. This pins it
/// from the other side.
///
/// Not to the pixel: a monospaced cell carries side bearing, so the last column
/// of the last glyph is legitimately blank. The tight property is that the
/// width is exactly one character's advance more than the same string without
/// its last character, and that the shorter box really does lose that glyph.
#[test]
fn the_measured_width_is_tight_not_merely_generous() {
    let backend = backend();
    let font = backend.font(FontRole::Ui);
    let height = backend.line_height(font);

    let full = backend.text_width(font, "Hello", FontStyle::Regular);
    let short = backend.text_width(font, "Hell", FontStyle::Regular);
    let advance = backend.text_width(font, "H", FontStyle::Regular);
    assert_eq!(
        full - short,
        advance,
        "one more character is one more advance, not some estimate of one"
    );

    backend.draw_text(Point::new(0, 0), "Hello", font, FontStyle::Regular);
    let all = read(&backend, |d| d.ink_count());
    let inside = read(&backend, |d| d.ink_in(0, 0, full, height));
    let cropped = read(&backend, |d| d.ink_in(0, 0, short, height));

    assert_eq!(inside, all, "the measured box holds every inked pixel");
    assert!(
        cropped < inside,
        "and it is tight: dropping one advance loses the last glyph's ink \
         ({cropped} of {inside})"
    );
}

/// The palette test asserted only that ink was absent, which a `fill_rect`
/// that did nothing at all would satisfy. The background has to be *written*.
#[test]
fn an_inverted_palette_writes_the_background_colour() {
    let inverted = Backend::new(TestDisplay::new(WIDTH, HEIGHT), Palette::INK_IS_OFF);

    // Start with the whole panel inked, so "did nothing" is distinguishable
    // from "wrote the background".
    inverted.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), false);
    let before = inverted.with_display(|d| d.ink_count());
    assert_eq!(
        before,
        (WIDTH * HEIGHT) as usize,
        "with the palette inverted, background is the display's on colour"
    );

    inverted.fill_rect(Rect::new(0, 0, 10, 10), true);
    let after = inverted.with_display(|d| d.ink_count());
    assert_eq!(
        after,
        before - 100,
        "and painting ink cleared exactly the rect it was given"
    );
}

/// A stroke drawn one pixel too wide passed the old test. Pin the extent.
#[test]
fn a_stroked_rect_lands_exactly_on_its_own_edges() {
    let backend = backend();
    let rect = Rect::new(10, 10, 20, 20);
    backend.stroke_rect(rect);

    // 20x20 outline = 2*20 + 2*18 interior-free pixels.
    assert_eq!(read(&backend, |d| d.ink_count()), 76);
    assert!(
        read(&backend, |d| !d.get(9, 10) && !d.get(30, 10)),
        "nothing outside the rect's own columns"
    );
    assert!(
        read(&backend, |d| !d.get(10, 9) && !d.get(10, 30)),
        "nor outside its own rows"
    );
}

/// Clearing must reach the whole panel, and must ignore any clip — the
/// framework calls it before a frame, when a stale clip could still be set.
#[test]
fn clearing_ignores_the_clip_and_covers_the_panel() {
    let backend = backend();
    backend.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), true);
    backend.set_clip(Some(Rect::new(0, 0, 10, 10)));

    backend.clear();

    assert_eq!(
        read(&backend, |d| d.ink_count()),
        0,
        "a clear reaches past whatever clip was left set"
    );
}

#[test]
fn a_line_is_drawn_between_the_points_it_was_given() {
    let backend = backend();
    backend.draw_line(Point::new(0, 5), Point::new(9, 5));

    assert_eq!(read(&backend, |d| d.ink_in(0, 5, 10, 1)), 10);
    assert_eq!(read(&backend, |d| d.ink_count()), 10, "and nothing else");
}

/// An icon this backend has no glyph for must report 0, so the framework
/// reserves no space rather than leaving a hole where one was expected.
///
/// The icons it *can* draw are covered in `icons.rs`, which installs the
/// backend — `xpui_chrome` paints through the global host, so a test that only
/// constructs a backend watches the ink land somewhere else.
#[test]
fn an_unknown_icon_reports_no_size_and_draws_nothing() {
    let backend = backend();
    let unknown = xpui::host::IconRef::new(9999);

    assert_eq!(backend.icon_size(unknown), 0);
    backend.draw_icon(Point::new(0, 0), unknown);
    assert_eq!(read(&backend, |d| d.ink_count()), 0);
}

// -- the tools the screenshots depend on -----------------------------------

/// A `thumbnail` is what a failed screenshot prints and a `write_bmp` is what
/// a test with no golden leaves behind. Neither is an assertion, so nothing
/// else would notice either of them going wrong — a thumbnail that showed the
/// wrong thing would make every screenshot failure unreadable.
#[test]
fn a_thumbnail_reflects_what_is_in_the_framebuffer() {
    let mut display = TestDisplay::new(64, 64);
    let blank = display.thumbnail(8);
    assert!(
        blank.chars().all(|c| c == ' ' || c == '\n'),
        "an empty framebuffer thumbnails as blank:\n{blank}"
    );

    for y in 0..32 {
        for x in 0..64 {
            display.pixels[y * 64 + x] = true;
        }
    }
    let half = display.thumbnail(8);
    let lines: Vec<&str> = half.lines().collect();
    assert!(
        lines[0].chars().all(|c| c == '@'),
        "the inked half is solid: {:?}",
        lines[0]
    );
    assert!(
        lines[lines.len() - 1].chars().all(|c| c == ' '),
        "and the clear half is blank: {:?}",
        lines[lines.len() - 1]
    );
}

/// A width that is not a multiple of eight leaves spare bits in the last byte
/// of every row. They must be paper — as ink they draw a stripe down the right
/// edge of every screenshot.
#[test]
fn a_bmp_has_a_valid_header_and_no_stripe_on_a_ragged_width() {
    let mut display = TestDisplay::new(20, 4);
    display.pixels[0] = true;

    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let path = display.write_bmp_in(dir, "header_check");
    let bytes = std::fs::read(&path).expect("the bmp was written");

    assert_eq!(&bytes[0..2], b"BM", "magic");
    let data_offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    assert_eq!(
        data_offset, 62,
        "14-byte header + 40-byte info + 2x4 palette"
    );
    assert_eq!(
        u32::from_le_bytes(bytes[14..18].try_into().unwrap()),
        40,
        "BITMAPINFOHEADER"
    );
    assert_eq!(
        u16::from_le_bytes(bytes[28..30].try_into().unwrap()),
        1,
        "1bpp"
    );
    assert_eq!(
        bytes.len(),
        data_offset + 4 * 4,
        "rows padded to four bytes"
    );

    // Row 0 is emitted last (BMP is bottom-up). Its first pixel is ink =
    // palette index 0 = a clear bit; every other bit in the row must be set.
    let last_row = &bytes[data_offset + 3 * 4..data_offset + 4 * 4];
    assert_eq!(last_row[0], 0b0111_1111, "one ink pixel, the rest paper");
    assert_eq!(
        last_row[1], 0xFF,
        "the four spare bits past x=19 are paper, not a black stripe"
    );
}
