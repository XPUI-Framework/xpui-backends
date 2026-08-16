//! Committed PNG goldens of what a backend actually painted.
//!
//! The sibling of `xpui::testing::assert_snapshot`, and deliberately not the
//! same thing. That one records the *draw calls* — order, clip lifecycle, the
//! selection a dialog forced on the list behind it — and is text because no
//! image can show any of that. This one records *pixels*, and is an image
//! because no text can show those without throwing most of them away.
//!
//! ```rust,ignore
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

use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::framebuffer::Framebuffer;

/// Where goldens live, relative to the crate being tested.
const DIR: &str = "tests/screenshots";

/// Columns in the ASCII views a failure prints. Narrow panels get their own
/// width, so a 296-pixel Badger is not stretched to sixty.
const THUMBNAIL_COLUMNS: i32 = 60;

/// Ink between the panels of a diff image, in pixels.
const RULE: i32 = 4;

/// How far outside the changed pixels the diff image draws its marker box. A
/// single flipped pixel is invisible at any zoom; a box around it is not.
const MARKER_INSET: i32 = 4;

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

// -- what changed ----------------------------------------------------------

/// Every pixel that differs, summarised.
struct Difference {
    count: usize,
    first: (i32, i32),
    /// Inclusive bounds of the changed pixels.
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `None` when the two frames are pixel-for-pixel identical.
///
/// Compares decoded pixels rather than file bytes on purpose. The `png` crate
/// documents that its DEFLATE output may change in a semver-compatible
/// release, so a byte comparison would fail twenty tests on a dependency bump
/// with nothing visibly different in any of them — a failure nobody can read,
/// which is exactly what this module exists to avoid. Every pixel still has to
/// match exactly: these are 1-bit panels, there is no anti-aliasing, and no
/// tolerance is warranted.
fn difference(expected: &Framebuffer, actual: &Framebuffer) -> Option<Difference> {
    let mut found: Option<Difference> = None;

    for y in 0..expected.height {
        for x in 0..expected.width {
            if expected.get(x, y) == actual.get(x, y) {
                continue;
            }
            match &mut found {
                None => {
                    found = Some(Difference {
                        count: 1,
                        first: (x, y),
                        left: x,
                        top: y,
                        right: x,
                        bottom: y,
                    });
                }
                Some(difference) => {
                    difference.count += 1;
                    difference.left = difference.left.min(x);
                    difference.right = difference.right.max(x);
                    difference.bottom = y;
                }
            }
        }
    }

    found
}

// -- the error message -----------------------------------------------------

fn report(
    name: &str,
    golden: &Path,
    expected: &Framebuffer,
    actual: &Framebuffer,
    difference: &Difference,
    image: &Path,
) -> String {
    let columns = THUMBNAIL_COLUMNS.min(actual.width);
    let total = (actual.width * actual.height) as usize;
    let (x, y) = difference.first;

    format!(
        "screenshot `{name}` does not match {golden}\n\n\
         {count} of {total} pixels differ, first at ({x}, {y}), \
         all of them within ({left}, {top} {width}x{height})\n\n\
         side by side — expected | actual | differences:\n  {image}\n\n\
         what was painted, with every changed block marked X:\n{map}\n\
         what the golden holds:\n{was}\n\
         If this change is intended, re-run with UPDATE_SNAPSHOTS=1 and look at the goldens.",
        golden = golden.display(),
        count = difference.count,
        left = difference.left,
        top = difference.top,
        width = difference.right - difference.left + 1,
        height = difference.bottom - difference.top + 1,
        image = image.display(),
        map = difference_map(expected, actual, columns),
        was = expected.thumbnail(columns),
    )
}

/// The painted frame as ASCII, with every character cell holding a changed
/// pixel replaced by `X`.
///
/// A plain thumbnail cannot show a small change — one pixel in an 8x16 block
/// averages to nothing — so the marking, not the shading, is what carries the
/// information here.
fn difference_map(expected: &Framebuffer, actual: &Framebuffer, columns: i32) -> String {
    let (block, block_y) = actual.block_size(columns);
    let mut lines: Vec<Vec<char>> = actual
        .thumbnail(columns)
        .lines()
        .map(|line| line.chars().collect())
        .collect();

    for y in 0..actual.height {
        for x in 0..actual.width {
            if expected.get(x, y) == actual.get(x, y) {
                continue;
            }
            if let Some(cell) = lines
                .get_mut((y / block_y) as usize)
                .and_then(|line| line.get_mut((x / block) as usize))
            {
                *cell = 'X';
            }
        }
    }

    lines
        .into_iter()
        .map(|line| line.into_iter().collect::<String>() + "\n")
        .collect()
}

// -- the diff image --------------------------------------------------------

/// Writes `target/diff/<name>.png` and returns where it went.
fn write_diff_image(
    name: &str,
    expected: &Framebuffer,
    actual: &Framebuffer,
    difference: &Difference,
) -> PathBuf {
    let dir = diff_dir();
    fs::create_dir_all(&dir).ok();
    let path = dir.join(format!("{name}.png"));
    let image = diff_image(expected, actual, difference);
    fs::write(&path, image.to_png())
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    path
}

/// Three panels side by side, separated by an inked rule: what was blessed,
/// what was painted, and where the two disagree.
///
/// The third panel carries a box around the changed pixels, because a
/// difference of one pixel in a panel this size cannot be found by looking.
fn diff_image(
    expected: &Framebuffer,
    actual: &Framebuffer,
    difference: &Difference,
) -> Framebuffer {
    let width = expected.width;
    let mut image = Framebuffer::new(width * 3 + RULE * 2, expected.height);

    for y in 0..expected.height {
        for x in 0..width {
            set(&mut image, x, y, expected.get(x, y));
            set(&mut image, width + RULE + x, y, actual.get(x, y));
            set(
                &mut image,
                (width + RULE) * 2 + x,
                y,
                expected.get(x, y) != actual.get(x, y),
            );
        }
        for rule in 0..RULE {
            set(&mut image, width + rule, y, true);
            set(&mut image, width * 2 + RULE + rule, y, true);
        }
    }

    let origin = (width + RULE) * 2;
    let left = difference.left - MARKER_INSET;
    let right = difference.right + MARKER_INSET;
    let top = difference.top - MARKER_INSET;
    let bottom = difference.bottom + MARKER_INSET;
    for x in left..=right {
        set(&mut image, origin + x, top, true);
        set(&mut image, origin + x, bottom, true);
    }
    for y in top..=bottom {
        set(&mut image, origin + left, y, true);
        set(&mut image, origin + right, y, true);
    }

    image
}

fn set(frame: &mut Framebuffer, x: i32, y: i32, ink: bool) {
    if x < 0 || y < 0 || x >= frame.width || y >= frame.height {
        return;
    }
    frame.pixels[(y * frame.width + x) as usize] = ink;
}

/// The workspace's `target/diff`.
///
/// Derived from the test binary rather than the working directory: cargo runs
/// an integration test with the working directory at its own crate root, which
/// in a workspace is not where the shared `target/` is. The binary itself is
/// always at `<target>/<profile>/deps/<name>-<hash>`.
fn diff_dir() -> PathBuf {
    let target = env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("target"));
    target.join("diff")
}

#[cfg(test)]
mod tests {
    use super::{Difference, diff_image, difference, difference_map};
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
