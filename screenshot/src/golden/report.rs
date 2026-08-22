//! What a mismatch shows: the message, the thumbnail with the changed blocks
//! marked, and the side-by-side image beside it.

use std::path::{Path, PathBuf};
use std::{env, fs};

use super::compare::Difference;
use crate::framebuffer::Framebuffer;

/// Columns in the ASCII views a failure prints. Narrow panels get their own
/// width, so a 296-pixel Badger is not stretched to sixty.
const THUMBNAIL_COLUMNS: i32 = 60;

/// Ink between the panels of a diff image, in pixels.
const RULE: i32 = 4;

/// How far outside the changed pixels the diff image draws its marker box. A
/// single flipped pixel is invisible at any zoom; a box around it is not.
const MARKER_INSET: i32 = 4;

pub(super) fn report(
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
pub(super) fn difference_map(expected: &Framebuffer, actual: &Framebuffer, columns: i32) -> String {
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

/// Writes `target/diff/<name>.png` and returns where it went.
pub(super) fn write_diff_image(
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
pub(super) fn diff_image(
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
///
/// Counting three levels is safe here in a way it was not in the ABI checker,
/// which counted its own depth in the repository: this counts cargo's own
/// layout, which cargo guarantees. And a wrong answer puts a failure artefact
/// somewhere unexpected rather than reporting a pass — the fallback is a
/// relative `target/`, not a panic, because a diff nobody can find is a worse
/// outcome than a test that fails without one.
fn diff_dir() -> PathBuf {
    let target = env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("target"));
    target.join("diff")
}
