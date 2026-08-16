//! A framebuffer to draw into, and three ways to look at it.
//!
//! Behind the `framebuffer` feature, and `std` only. It exists so an app built
//! on this backend can render a screen with no window and no hardware, then
//! assert on the result — which is what makes screenshot tests possible in
//! ordinary `cargo test`.
//!
//! ```rust,ignore
//! let backend = Backend::leak(Framebuffer::new(480, 800), Palette::new(On, Off));
//! unsafe { xpui::host::install(backend) };
//! App::new(MyScreen::new()).render();
//! backend.with_display(|fb| {
//!     fb.write_bmp("my_screen");        // look at it
//!     assert!(fb.ink_count() > 0);      // or assert on it
//! });
//! ```

use std::fs;
use std::path::PathBuf;

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// A plain 1-bit framebuffer implementing [`DrawTarget`].
///
/// Not `MockDisplay`: that one is 64x64 and rejects overdraw, and a dither or
/// a scrim deliberately paints over what is already there.
pub struct Framebuffer {
    pub width: i32,
    pub height: i32,
    /// True where ink was laid down.
    pub pixels: Vec<bool>,
}

impl Framebuffer {
    pub fn new(width: i32, height: i32) -> Self {
        Framebuffer {
            width,
            height,
            pixels: vec![false; (width * height) as usize],
        }
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return false;
        }
        self.pixels[(y * self.width + x) as usize]
    }

    pub fn ink_count(&self) -> usize {
        self.pixels.iter().filter(|set| **set).count()
    }

    /// Ink inside a region, for asserting that something landed where it
    /// should without pinning every pixel of it.
    pub fn ink_in(&self, x: i32, y: i32, width: i32, height: i32) -> usize {
        let mut count = 0;
        for row in y..y + height {
            for column in x..x + width {
                if self.get(column, row) {
                    count += 1;
                }
            }
        }
        count
    }

    /// A coarse ASCII view, `columns` characters wide.
    ///
    /// Full resolution is unreadable in a diff — a 480x800 panel is 384,000
    /// characters. Averaging blocks down to about sixty columns keeps a
    /// screenshot small enough to review while still showing layout: a list
    /// that moved, a header that vanished, a dialog off-centre.
    pub fn thumbnail(&self, columns: i32) -> String {
        const SHADES: [char; 5] = [' ', '.', ':', '#', '@'];

        let block = (self.width as f32 / columns as f32).ceil() as i32;
        // Character cells are about twice as tall as they are wide, so square
        // blocks would squash the picture.
        let block_y = block * 2;

        let mut out = String::new();
        let mut y = 0;
        while y < self.height {
            let mut x = 0;
            while x < self.width {
                let total = (block * block_y) as f32;
                let ink = self.ink_in(x, y, block, block_y) as f32;
                let shade = ((ink / total) * (SHADES.len() - 1) as f32).round() as usize;
                out.push(SHADES[shade.min(SHADES.len() - 1)]);
                x += block;
            }
            out.push('\n');
            y += block_y;
        }
        out
    }

    /// Writes a 1-bit BMP next to the test binary, so a person can actually
    /// look at the frame.
    ///
    /// BMP rather than PNG because a 1-bit BMP is sixty lines of header and no
    /// compression, and this crate is not taking an image dependency to
    /// produce a debugging artifact. Opens in anything.
    pub fn write_bmp(&self, name: &str) -> PathBuf {
        self.write_bmp_in(screenshot_dir(), name)
    }

    /// The same, somewhere specific.
    pub fn write_bmp_in(&self, dir: PathBuf, name: &str) -> PathBuf {
        fs::create_dir_all(&dir).ok();
        let path = dir.join(format!("{name}.bmp"));

        // Rows are padded to four bytes, and stored bottom-up.
        let row_bytes = ((self.width as usize).div_ceil(8) + 3) & !3;
        let pixel_bytes = row_bytes * self.height as usize;
        let palette_offset = 14 + 40;
        let data_offset = palette_offset + 8;
        let file_size = data_offset + pixel_bytes;

        let mut out = Vec::with_capacity(file_size);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(file_size as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(data_offset as u32).to_le_bytes());

        out.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&1u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&0u32.to_le_bytes()); // no compression
        out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        out.extend_from_slice(&2835u32.to_le_bytes()); // ~72 dpi
        out.extend_from_slice(&2835u32.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes()); // palette entries
        out.extend_from_slice(&0u32.to_le_bytes());

        // Index 0 is ink, index 1 is paper, as BGRA.
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        out.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0x00]);

        for row in (0..self.height).rev() {
            // Filled with paper, not zero: index 0 is ink, so a width that is
            // not a multiple of eight would leave the last byte's spare bits
            // black and paint a stripe down the right edge.
            let mut line = vec![0xFFu8; row_bytes];
            for column in 0..self.width {
                if self.get(column, row) {
                    // Clearing selects palette index 0, which is ink.
                    line[(column / 8) as usize] &= !(0x80 >> (column % 8));
                }
            }
            out.extend_from_slice(&line);
        }

        fs::write(&path, out).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
        path
    }
}

/// Where [`Framebuffer::write_bmp`] puts things.
///
/// `$XPUI_SCREENSHOT_DIR` if set, else `target/screenshots` relative to the
/// working directory. Cargo runs an integration test with the working
/// directory at its own crate root, which in a workspace is *not* where the
/// shared `target/` is — so a test that wants them all in one place passes
/// `env!("CARGO_TARGET_TMPDIR")` to [`Framebuffer::write_bmp_in`] instead.
/// That variable is set at compile time and does point inside the workspace's
/// target directory.
fn screenshot_dir() -> PathBuf {
    match std::env::var("XPUI_SCREENSHOT_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from("target/screenshots"),
    }
}

impl Dimensions for Framebuffer {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(
            Point::new(0, 0),
            Size::new(self.width as u32, self.height as u32),
        )
    }
}

impl DrawTarget for Framebuffer {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, colour) in pixels {
            if point.x < 0 || point.y < 0 || point.x >= self.width || point.y >= self.height {
                continue;
            }
            self.pixels[(point.y * self.width + point.x) as usize] = colour.is_on();
        }
        Ok(())
    }
}
