//! Committed PNG goldens of what a backend actually painted.
//!
//! The sibling of `xpui::testing::assert_snapshot`, and deliberately not the
//! same thing. That one records the *draw calls* — order, clip lifecycle, the
//! selection a dialog forced on the list behind it — and is text because no
//! image can show any of that. This one records *pixels*, and is an image
//! because no text can show those without throwing most of them away.
//!
//! `no_run` because it writes a golden when one does not exist yet, which
//! does not belong in a documentation build:
//!
//! ```rust,no_run
//! # use embedded_graphics::pixelcolor::BinaryColor;
//! # use embedded_graphics::prelude::*;
//! # use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
//! use xpui_screenshot::{Framebuffer, assert_screenshot};
//!
//! # let mut frame = Framebuffer::new(64, 32);
//! # Rectangle::new(Point::new(4, 4), Size::new(16, 8))
//! #     .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
//! #     .draw(&mut frame)
//! #     .unwrap();
//! assert_screenshot("a_filled_rectangle", &frame);   // against a committed PNG
//! assert!(frame.ink_in(4, 4, 16, 8) > 0);            // and what a picture cannot say
//! ```
//!
//! When the change is intended:
//!
//! ```bash
//! UPDATE_SNAPSHOTS=1 cargo test
//! ```
//!
//! which rewrites every golden the run touched. Open them before committing —
//! a blessed screenshot is an assertion you have made, and blessing a
//! regression is the one failure mode this technique has.

mod compare;
mod report;

use std::path::PathBuf;
use std::{env, fs};

use compare::difference;
use report::{report, write_diff_image};

use crate::framebuffer::Framebuffer;

/// Where goldens live, relative to the crate being tested.
const DIR: &str = "tests/screenshots";

/// Asserts that `frame` matches the golden committed as `tests/screenshots/<name>.png`.
///
/// Writes the golden instead when `UPDATE_SNAPSHOTS` is set, and always writes
/// it when it does not exist yet — a new test should not need two runs.
///
/// On a mismatch it prints an ASCII view of what changed and writes a
/// side-by-side image to `target/diff/<name>.png`.
pub fn assert_screenshot(name: &str, frame: &Framebuffer) {
    if let Err(report) = check_screenshot(name, frame) {
        panic!("{report}");
    }
}

/// The same comparison, as a `Result`.
///
/// For a caller capturing many frames in one test: it can gather what every
/// one of them said and report them together, where [`assert_screenshot`]
/// would stop at the first. `Err` holds the whole report, ready to print —
/// the same text the assertion would have panicked with.
///
/// A caller that wants one frame checked should use [`assert_screenshot`],
/// which puts the failure where it happened.
///
/// A `String` rather than an error type, against this crate's habit: the
/// `Err` is a finished report — a pixel count, a bounding box, two ASCII views
/// and a path — and its only use is to be printed. There is nothing in it a
/// caller could match on.
///
/// It still panics rather than returning `Err` when the harness itself is
/// broken: a golden that reads but does not decode, or one that cannot be
/// written — no directory to put it in, or a file that will not take it.
/// Neither is a screen having changed, and gathering them into a report of
/// what moved would file them under the wrong heading. A golden that cannot be
/// *opened* is not one of these — that is indistinguishable from one that is
/// not there yet, and takes the same path as a new capture.
pub fn check_screenshot(name: &str, frame: &Framebuffer) -> Result<(), String> {
    let path = path_for(name);
    let existing = fs::read(&path).ok();

    if updating() || existing.is_none() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("cannot create {}: {e}", parent.display()));
        }
        fs::write(&path, frame.to_png())
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));

        // A golden that did not exist is now whatever the code happens to do,
        // which proves nothing. Say so rather than passing quietly.
        if existing.is_none() && !updating() {
            return Err(format!(
                "screenshot `{name}` did not exist and has been written to {}.\n\
                 Open it, confirm it is what the screen should look like, then re-run.",
                path.display()
            ));
        }
        return Ok(());
    }

    let bytes = existing.expect("checked above");
    let expected = Framebuffer::from_png(&bytes)
        .unwrap_or_else(|e| panic!("{} is not a readable png: {e}", path.display()));

    if expected.width != frame.width || expected.height != frame.height {
        return Err(format!(
            "screenshot `{name}` is {}x{} but {} holds {}x{}.\n\
             If this change is intended, re-run with UPDATE_SNAPSHOTS=1.",
            frame.width,
            frame.height,
            path.display(),
            expected.width,
            expected.height
        ));
    }

    let Some(difference) = difference(&expected, frame) else {
        return Ok(());
    };

    let image = write_diff_image(name, &expected, frame, &difference);
    Err(report(name, &path, &expected, frame, &difference, &image))
}

fn path_for(name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR is the crate under test, so a backend crate keeps its
    // own goldens rather than writing into another crate's.
    let root = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set under cargo test");
    PathBuf::from(root).join(DIR).join(format!("{name}.png"))
}

fn updating() -> bool {
    env::var("UPDATE_SNAPSHOTS").is_ok_and(|value| value != "0")
}

#[cfg(test)]
mod tests {
    use super::compare::{Difference, difference};
    use super::report::{diff_image, difference_map};
    use std::sync::Mutex;

    use super::{check_screenshot, path_for, updating};
    use crate::framebuffer::Framebuffer;

    /// A golden of its own, so the comparison is exercised against a file on
    /// disk rather than only in memory. The one golden this crate owns; every
    /// other belongs to whichever crate's test draws it.
    const PROBE: &str = "check_screenshot_probe";

    /// The two tests that resolve a golden path, serialised.
    ///
    /// `CARGO_MANIFEST_DIR` is process-wide, and one of them moves it to prove
    /// the resolver reads it rather than a constant compiled in here.
    static RESOLVING: Mutex<()> = Mutex::new(());

    /// What `PROBE` holds: a shape with ink in both halves, so a flipped pixel
    /// can be put somewhere the encoder is not already writing.
    fn probe_frame() -> Framebuffer {
        let mut frame = Framebuffer::new(32, 24);
        for x in 4..28 {
            frame.pixels[6 * 32 + x] = true;
            frame.pixels[17 * 32 + x] = true;
        }
        frame
    }

    /// **The line every pixel assertion in the organisation stands on.**
    ///
    /// Each one is this function returning `Err`: eighty-two of them, and only
    /// seven are in this repository. Seventy-three are `xpui-gallery`'s —
    /// seventy board captures and three families — and two are the tutorial
    /// screen's, beside them. The eighty-third is this test's own probe, here.
    ///
    /// Replace its last two lines with `Ok(())` and the whole suite still
    /// passes while nothing is compared at all, which is the one failure this
    /// technique cannot survive. So it is checked here, on a golden kept for
    /// the purpose.
    #[test]
    fn a_frame_that_differs_from_its_golden_comes_back_as_an_error() {
        let _guard = RESOLVING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // `UPDATE_SNAPSHOTS` means "rewrite, do not compare", so under it there
        // is no comparison to make and the second half would overwrite the
        // probe with the wrong picture. The gate runs `cargo test` without it.
        if updating() {
            return;
        }

        let frame = probe_frame();
        assert!(
            check_screenshot(PROBE, &frame).is_ok(),
            "the committed {PROBE}.png is not what `probe_frame` paints, so \
             neither half of this test means anything"
        );

        let mut moved = probe_frame();
        moved.pixels[17 * 32 + 15] = false;

        let report = check_screenshot(PROBE, &moved)
            .expect_err("one pixel was flipped and the comparison said nothing");
        assert!(
            report.contains(PROBE),
            "the report does not name the golden it is about: {report}"
        );
        assert!(
            report.contains("1 of 768 pixels differ"),
            "the report does not say what moved: {report}"
        );
    }

    /// And the golden it reads is the one its name points at, rather than
    /// whatever the working directory happened to be.
    #[test]
    fn a_golden_lives_under_the_crate_being_tested() {
        let _guard = RESOLVING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let real = std::env::var("CARGO_MANIFEST_DIR").expect("set under cargo test");

        let path = path_for(PROBE);
        assert!(
            path.ends_with("tests/screenshots/check_screenshot_probe.png"),
            "{}",
            path.display()
        );
        assert!(
            path.starts_with(&real),
            "a golden was resolved outside the crate under test: {}",
            path.display()
        );

        // The property this test exists for is that the root is read at *run*
        // time, from whichever crate is under test. Asserting against
        // `env!(..)` cannot show that: inside one crate the compile-time and
        // run-time values are the same string, so a resolver that hardcoded
        // its own directory would pass. Move the variable and the answer has
        // to move with it, or every consumer's goldens land in this crate.
        //
        // Safety: `set_var` is not thread-safe, and `RESOLVING` is what makes
        // this block single-threaded — the other test that resolves a path
        // takes the same lock. Breaking that invariant is a data race, which
        // is undefined behaviour rather than a wrong path.
        unsafe { std::env::set_var("CARGO_MANIFEST_DIR", "/proof/of/the/root") };
        let moved = path_for(PROBE);
        // Safety: as above, and restoring it before the guard drops is what
        // keeps every other test seeing the real directory.
        unsafe { std::env::set_var("CARGO_MANIFEST_DIR", &real) };

        assert!(
            moved.starts_with("/proof/of/the/root"),
            "the golden root is compiled in rather than read, so every crate's \
             goldens would be written into this one: {}",
            moved.display()
        );
    }

    fn pair() -> (Framebuffer, Framebuffer) {
        let mut expected = Framebuffer::new(64, 64);
        for x in 0..64 {
            expected.pixels[x] = true;
        }
        let actual = Framebuffer::new(64, 64);
        (expected, actual)
    }

    #[test]
    fn identical_frames_have_no_difference() {
        let (expected, _) = pair();
        assert!(difference(&expected, &expected).is_none());
    }

    /// The whole point of the exercise: one pixel, and the comparison notices.
    #[test]
    fn one_flipped_pixel_is_a_difference() {
        let expected = Framebuffer::new(64, 64);
        let mut actual = Framebuffer::new(64, 64);
        actual.pixels[40 * 64 + 33] = true;

        let found = difference(&expected, &actual).expect("one pixel differs");
        assert_eq!(found.count, 1);
        assert_eq!(found.first, (33, 40));
        assert_eq!((found.left, found.right), (33, 33));
        assert_eq!((found.top, found.bottom), (40, 40));
    }

    /// And the message shows where, which a plain thumbnail cannot: one pixel
    /// in an 8x16 block averages to blank.
    #[test]
    fn the_map_marks_the_block_a_single_pixel_landed_in() {
        let expected = Framebuffer::new(64, 64);
        let mut actual = Framebuffer::new(64, 64);
        actual.pixels[40 * 64 + 33] = true;

        assert!(
            !actual.thumbnail(8).contains('X'),
            "a thumbnail alone shows nothing, which is why the map exists"
        );

        let map = difference_map(&expected, &actual, 8);
        let lines: Vec<&str> = map.lines().collect();
        // Blocks are 8 wide and 16 tall at eight columns.
        assert_eq!(lines[40 / 16].chars().nth(33 / 8), Some('X'), "{map}");
        assert_eq!(map.matches('X').count(), 1, "{map}");
    }

    #[test]
    fn the_diff_image_is_three_panels_wide() {
        let (expected, actual) = pair();
        let found = difference(&expected, &actual).expect("they differ");
        let image = diff_image(&expected, &actual, &found);

        assert_eq!(image.width, 64 * 3 + 4 * 2);
        assert_eq!(image.height, 64);
        // The rules between the panels.
        assert!(image.get(64, 0) && image.get(64 + 3, 0));
        assert!(image.get(64 * 2 + 4, 0));
    }

    /// The marker box is what makes a one-pixel difference findable by eye.
    #[test]
    fn the_diff_image_boxes_the_changed_pixels() {
        let expected = Framebuffer::new(64, 64);
        let mut actual = Framebuffer::new(64, 64);
        actual.pixels[40 * 64 + 33] = true;

        let found = difference(&expected, &actual).expect("they differ");
        let image = diff_image(&expected, &actual, &found);

        let origin = (64 + 4) * 2;
        assert!(image.get(origin + 33, 40), "the changed pixel itself");
        assert!(image.get(origin + 33, 40 - 4), "the top of the box");
        assert!(image.get(origin + 33 - 4, 40), "and its left edge");
    }

    /// A frame survives the round trip through the format the goldens are in.
    #[test]
    fn a_frame_round_trips_through_png() {
        let mut frame = Framebuffer::new(37, 21);
        for i in (0..frame.pixels.len()).step_by(3) {
            frame.pixels[i] = true;
        }

        let decoded = Framebuffer::from_png(&frame.to_png()).expect("it decodes");
        assert_eq!((decoded.width, decoded.height), (37, 21));
        assert!(difference(&frame, &decoded).is_none());
    }

    /// And encoding is deterministic, or a re-bless would churn every golden.
    #[test]
    fn the_same_frame_encodes_to_the_same_bytes() {
        let (expected, _) = pair();
        assert_eq!(expected.to_png(), expected.to_png());
    }

    #[test]
    fn a_difference_is_found_at_the_last_pixel_too() {
        let expected = Framebuffer::new(8, 8);
        let mut actual = Framebuffer::new(8, 8);
        actual.pixels[63] = true;

        let found: Difference = difference(&expected, &actual).expect("the corner differs");
        assert_eq!(found.first, (7, 7));
    }
}
