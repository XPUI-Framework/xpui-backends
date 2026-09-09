//! A framebuffer to draw into, and several ways to look at it.
//!
//! `std` only, and the reason this crate is not part of a backend. It exists
//! so an app can render a screen with no window and no hardware, then
//! assert on the result — which is what makes screenshot tests possible in
//! ordinary `cargo test`.
//!
//! ```rust
//! use embedded_graphics::pixelcolor::BinaryColor;
//! use embedded_graphics::prelude::*;
//! use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
//! use xpui_screenshot::Framebuffer;
//!
//! let mut frame = Framebuffer::new(64, 32);
//! Rectangle::new(Point::new(4, 4), Size::new(16, 8))
//!     .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
//!     .draw(&mut frame)
//!     .unwrap();
//!
//! assert_eq!(frame.ink_in(4, 4, 16, 8), 16 * 8);
//! assert!(!frame.get(0, 0));
//! ```
//!
//! Comparing a frame against a committed PNG is the `golden` module, behind
//! the feature of the same name.

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
    ///
    /// Private because `width`, `height` and this vector's length are one
    /// invariant, and a caller holding the vector could break it. [`set`] and
    /// [`ink`] are what a caller needs.
    ///
    /// [`set`]: Framebuffer::set
    /// [`ink`]: Framebuffer::ink
    pub(crate) pixels: Vec<bool>,
}

impl Framebuffer {
    pub fn new(width: i32, height: i32) -> Self {
        Framebuffer {
            width,
            height,
            pixels: vec![false; (width * height) as usize],
        }
    }

    /// Lays ink down, or takes it away. Out of bounds is ignored, as
    /// [`get`](Framebuffer::get) reads out of bounds as blank.
    pub fn set(&mut self, x: i32, y: i32, ink: bool) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        self.pixels[(y * self.width + x) as usize] = ink;
    }

    /// Every pixel, row by row, for a caller comparing two whole frames.
    ///
    /// Read-only: the length is part of this frame's invariant.
    pub fn ink(&self) -> &[bool] {
        &self.pixels
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

    /// The pixel size of one [`thumbnail`](Self::thumbnail) character cell, as
    /// `(width, height)`.
    ///
    /// Public because anything that annotates a thumbnail — marking the cells
    /// that changed, say — has to divide coordinates the same way, and two
    /// copies of this arithmetic would drift.
    pub fn block_size(&self, columns: i32) -> (i32, i32) {
        let block = (self.width as f32 / columns as f32).ceil() as i32;
        // Character cells are about twice as tall as they are wide, so square
        // blocks would squash the picture.
        (block, block * 2)
    }

    /// A coarse ASCII view, `columns` characters wide.
    ///
    /// Deliberately lossy, and never an assertion: a block is `width /
    /// columns` pixels wide and twice that tall, so a shift smaller than that
    /// is invisible to it. It is for *reading* — in
    /// a failed screenshot's message, or a `println!` while debugging — where
    /// showing a list that moved or a dialog off-centre is the whole job.
    pub fn thumbnail(&self, columns: i32) -> String {
        const SHADES: [char; 5] = [' ', '.', ':', '#', '@'];

        let (block, block_y) = self.block_size(columns);

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

    /// The panel as a 1-bit greyscale PNG: the format the committed
    /// screenshot goldens are written in.
    ///
    /// Encoded deterministically. The compression level and the row filter are
    /// pinned rather than left to a default that may move, and nothing here
    /// writes a timestamp, so re-blessing a screen that did not change
    /// rewrites the same bytes and leaves no diff to review.
    #[cfg(feature = "golden")]
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::One);
        encoder.set_compression(png::Compression::High);
        // `Up` rather than `Adaptive`: the adaptive filter picks per row by
        // measuring, which is one more thing that can change between releases
        // of the encoder for no change in the image.
        encoder.set_filter(png::Filter::Up);

        let mut writer = encoder
            .write_header()
            .expect("a 1-bit greyscale header is always valid");
        writer
            .write_image_data(&self.packed_rows())
            .expect("the row buffer is sized from the header");
        writer.finish().expect("writing to a Vec cannot fail");

        out
    }

    /// Reads back what [`to_png`](Self::to_png) wrote.
    ///
    /// Accepts any bit depth and greyscale or RGB, because a golden may have
    /// passed through an image editor on its way back in; every non-white
    /// pixel is ink. Fails rather than panics, so a corrupt golden reports
    /// itself as a corrupt golden.
    #[cfg(feature = "golden")]
    pub fn from_png(bytes: &[u8]) -> Result<Framebuffer, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        // Unpacks sub-byte depths and drops 16-bit samples to 8, so what comes
        // back is one byte per sample whatever the file holds.
        decoder.set_transformations(png::Transformations::normalize_to_color8());

        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buffer = vec![0u8; reader.output_buffer_size().ok_or("image is too large")?];
        let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;

        let samples = info.color_type.samples();
        let mut frame = Framebuffer::new(info.width as i32, info.height as i32);
        for y in 0..info.height as usize {
            for x in 0..info.width as usize {
                let sample = buffer[y * info.line_size + x * samples];
                frame.pixels[y * info.width as usize + x] = sample < 0x80;
            }
        }
        Ok(frame)
    }

    /// Rows packed the way a 1-bit greyscale PNG wants them: top down, one bit
    /// per pixel, and 0 is black.
    #[cfg(feature = "golden")]
    fn packed_rows(&self) -> Vec<u8> {
        let row_bytes = (self.width as usize).div_ceil(8);
        let mut out = Vec::with_capacity(row_bytes * self.height as usize);
        for row in 0..self.height {
            // Filled with paper: a width that is not a multiple of eight
            // leaves spare bits in the last byte, and as ink they draw a
            // stripe down the right edge.
            let mut line = vec![0xFFu8; row_bytes];
            for column in 0..self.width {
                if self.get(column, row) {
                    line[(column / 8) as usize] &= !(0x80 >> (column % 8));
                }
            }
            out.extend_from_slice(&line);
        }
        out
    }

    /// Writes a 1-bit BMP next to the test binary, so a person can actually
    /// look at the frame.
    ///
    /// A debugging helper, and only that: **nothing compares a BMP**. The
    /// assertion is `golden::assert_screenshot` against a
    /// committed PNG. Reach for this when you want to eyeball a frame from a
    /// test that has no golden — the icon sheet, or the simulator's screenshot
    /// key — and do not pair it with a screenshot assertion, because two
    /// artifacts where one is authoritative is how the other one gets trusted.
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

/// Where [`Framebuffer::write_bmp`] puts things: `$XPUI_SCREENSHOT_DIR` if
/// set, else `target/screenshots` relative to the working directory.
///
/// Cargo runs an integration test with the working directory at its own crate
/// root, which in a workspace is *not* where the shared `target/` is, so a
/// test that wants them all in one place passes `env!("CARGO_TARGET_TMPDIR")`
/// to [`Framebuffer::write_bmp_in`]. Public so a simulator's screenshot key
/// puts frames in the same place.
pub fn screenshot_dir() -> PathBuf {
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
