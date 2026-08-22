//! That icons draw something recognisable.
//!
//! This backend used to report `icon_size == 0` and draw nothing, so `Icon`
//! and `IconToggle` left holes and the layout did not even reserve space for
//! them. These pin the fix, and the sheet at the end is there to be *looked*
//! at — a glyph can pass every numeric assertion and still be a smudge.

use std::sync::{Mutex, MutexGuard};

use xpui::Point;
use xpui::host::{Canvas, IconRef};
use xpui_chrome::Icon;
use xpui_eg::{Backend, Palette};
use xpui_screenshot::{Framebuffer, assert_screenshot};

/// The installed host is process-wide, so these take turns.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A backend, **installed**.
///
/// `xpui_chrome` paints through `Renderer::`, which reaches the globally
/// installed host — not through a `&self` parameter. So a test that merely
/// constructs a backend and calls `draw_icon` on it watches the ink land
/// somewhere else entirely, and every assertion about "what this backend
/// drew" passes vacuously against an empty framebuffer. That is exactly what
/// the first version of this file did.
fn backend(width: i32, height: i32) -> &'static Backend<Framebuffer> {
    let backend = Backend::leak(Framebuffer::new(width, height), Palette::INK_IS_ON);
    // Safety: serialised by `SERIAL`; nothing has rendered on this backend.
    unsafe { xpui::host::install(backend) };
    backend
}

/// Every icon must draw *something*. One that silently draws nothing is worse
/// than one that draws badly: the layout reserved space for it.
#[test]
fn every_icon_puts_ink_on_the_panel() {
    let _guard = serial();
    for icon in Icon::ALL {
        let backend = backend(64, 64);
        backend.draw_icon(Point::new(16, 16), IconRef::new(icon.kind()));
        let ink = backend.with_display(|frame| frame.ink_count());
        assert!(ink > 0, "{icon:?} drew nothing");
    }
}

/// And must stay inside the box the layout gave it, or it collides with
/// whatever sits beside it.
#[test]
fn every_icon_stays_inside_its_own_box() {
    let _guard = serial();
    for icon in Icon::ALL {
        let backend = backend(64, 64);
        let spec = IconRef::new(icon.kind());
        let size = backend.icon_size(spec);
        backend.draw_icon(Point::new(16, 16), spec);

        let inside = backend.with_display(|frame| frame.ink_in(16, 16, size, size));
        let total = backend.with_display(|frame| frame.ink_count());
        assert_eq!(
            inside, total,
            "{icon:?} painted outside its {size}x{size} box"
        );
    }
}

/// `icon_size` is the layout's only way to reserve space, so it has to be what
/// is actually drawn — not a request, and never a guess.
#[test]
fn the_reported_size_is_the_drawn_size() {
    let _guard = serial();
    let backend = backend(64, 64);
    let spec = IconRef {
        kind: Icon::Gear.kind(),
        variant: 0,
        size: 31,
    };
    let size = backend.icon_size(spec);
    assert_eq!(
        size, 30,
        "rounded down to an even edge, so the centre is a pixel"
    );
    assert!(size <= spec.size, "never larger than was asked for");
}

/// An icon this crate cannot draw must report 0, so the framework leaves no
/// space rather than reserving a hole.
#[test]
fn an_unknown_icon_reports_nothing_and_draws_nothing() {
    let _guard = serial();
    let backend = backend(64, 64);
    let unknown = IconRef::new(9999);

    assert_eq!(backend.icon_size(unknown), 0);
    backend.draw_icon(Point::new(0, 0), unknown);
    assert_eq!(backend.with_display(|f| f.ink_count()), 0);
}

/// Below a certain size the strokes collide and every glyph is the same
/// smudge. Drawing nothing is the honest answer.
#[test]
fn an_icon_too_small_to_read_is_refused() {
    let _guard = serial();
    let backend = backend(64, 64);
    let tiny = IconRef {
        kind: Icon::Sun.kind(),
        variant: 0,
        size: 4,
    };
    assert_eq!(backend.icon_size(tiny), 0);
    backend.draw_icon(Point::new(0, 0), tiny);
    assert_eq!(backend.with_display(|f| f.ink_count()), 0);
}

/// Distinct glyphs must actually differ. Two icons with identical ink are two
/// names for the same picture, which is a bug the eye catches and no assertion
/// about "some ink" ever would.
#[test]
fn no_two_icons_are_the_same_picture() {
    let _guard = serial();
    let pixels: Vec<(Icon, Vec<bool>)> = Icon::ALL
        .into_iter()
        .map(|icon| {
            let backend = backend(48, 48);
            backend.draw_icon(Point::new(8, 8), IconRef::new(icon.kind()));
            (icon, backend.with_display(|frame| frame.ink().to_vec()))
        })
        .collect();

    for (index, (icon, ink)) in pixels.iter().enumerate() {
        for (other, other_ink) in &pixels[index + 1..] {
            assert_ne!(ink, other_ink, "{icon:?} and {other:?} draw the same thing");
        }
    }
}

/// A sheet of every icon at three sizes, held against a golden.
///
/// Numbers cannot tell you whether a gear looks like a gear, so the sheet
/// exists to be looked at — but it is also compared, because a sheet nobody
/// compares is a sheet that can change without anyone noticing. This used to
/// assert only that the file had been written, which is a thing that cannot
/// fail.
#[test]
fn the_icon_sheet() {
    let _guard = serial();
    const SIZES: [i32; 3] = [16, 24, 32];
    const CELL: i32 = 40;

    let width = CELL * Icon::ALL.len() as i32;
    let height = CELL * SIZES.len() as i32;
    let backend = backend(width, height);

    for (row, size) in SIZES.into_iter().enumerate() {
        for (column, icon) in Icon::ALL.into_iter().enumerate() {
            let spec = IconRef {
                kind: icon.kind(),
                variant: 0,
                size,
            };
            let drawn = backend.icon_size(spec);
            backend.draw_icon(
                Point::new(
                    column as i32 * CELL + (CELL - drawn) / 2,
                    row as i32 * CELL + (CELL - drawn) / 2,
                ),
                spec,
            );
        }
    }

    backend.with_display(|frame| assert_screenshot("icons", frame));
}
