//! Committed PNG goldens of what a backend actually painted.
//!
//! The sibling of `xpui::testing::assert_snapshot`, and deliberately not the
//! same thing. That one records the *draw calls* — order, clip lifecycle, the
//! selection a dialog forced on the list behind it — and is text because no
//! image can show any of that. This one records *pixels*, and is an image
//! because no text can show those without throwing most of them away.
//!
//! `no_run` because it installs the process-wide host and writes a golden when
//! one does not exist yet, neither of which belongs in a documentation build:
//!
//! ```rust,no_run
//! # use xpui::{App, NavigationScreen, Screen, Text, View, vstack};
//! # use xpui_eg::{Backend, Framebuffer, Palette, assert_screenshot};
//! # struct MyScreen;
//! # impl MyScreen { fn new() -> Self { MyScreen } }
//! # impl Screen for MyScreen {
//! #     type Message = ();
//! #     fn body(&self) -> impl View<()> { NavigationScreen::new(vstack![0; Text::new("hello")]) }
//! #     fn update(&mut self, _message: ()) {}
//! # }
//! let backend = Backend::leak(Framebuffer::new(480, 800), Palette::INK_IS_ON);
//! unsafe { xpui::host::install(backend) };
//! App::new(MyScreen::new()).render();
//! backend.with_display(|frame| assert_screenshot("my_screen", frame));
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
            panic!(
                "screenshot `{name}` did not exist and has been written to {}.\n\
                 Open it, confirm it is what the screen should look like, then re-run.",
                path.display()
            );
        }
        return;
    }

    let bytes = existing.expect("checked above");
    let expected = Framebuffer::from_png(&bytes)
        .unwrap_or_else(|e| panic!("{} is not a readable png: {e}", path.display()));

    if expected.width != frame.width || expected.height != frame.height {
        panic!(
            "screenshot `{name}` is {}x{} but {} holds {}x{}.\n\
             If this change is intended, re-run with UPDATE_SNAPSHOTS=1.",
            frame.width,
            frame.height,
            path.display(),
            expected.width,
            expected.height
        );
    }

    let Some(difference) = difference(&expected, frame) else {
        return;
    };

    let image = write_diff_image(name, &expected, frame, &difference);
    panic!(
        "{}",
        report(name, &path, &expected, frame, &difference, &image)
    );
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
    use crate::framebuffer::Framebuffer;

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
