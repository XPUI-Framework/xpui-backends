//! Which pixels two frames disagree about.

use crate::framebuffer::Framebuffer;

/// Every pixel that differs, summarised.
pub(super) struct Difference {
    pub(super) count: usize,
    pub(super) first: (i32, i32),
    /// Inclusive bounds of the changed pixels.
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
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
pub(super) fn difference(expected: &Framebuffer, actual: &Framebuffer) -> Option<Difference> {
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
