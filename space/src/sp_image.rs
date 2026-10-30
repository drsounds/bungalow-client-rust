//! Port of `SPImage`: an RGBA bitmap decoded from PNG.

use std::fs::File;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::sp_color::SPColor;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    /// `space_pixel_to_hex`.
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// `space_pixel_to_colour`.
    pub fn to_color(&self) -> SPColor {
        SPColor::new(self.r as i16, self.g as i16, self.b as i16, self.a as i16)
    }
}

static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct SPImage {
    /// Unique per image; backends key their texture caches on it (the C++
    /// code stored an OS handle on the image instead).
    pub id: u64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub left_border: i32,
    pub top_border: i32,
    pub bottom_border: i32,
    pub right_border: i32,
    pub pixels: Vec<Pixel>,
}

impl Default for SPImage {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl SPImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            x: 0,
            y: 0,
            width,
            height,
            left_border: 0,
            top_border: 0,
            bottom_border: 0,
            right_border: 0,
            pixels: vec![Pixel::default(); (width * height) as usize],
        }
    }

    pub fn num_pixels(&self) -> usize {
        self.pixels.len()
    }

    /// Loads a PNG file, normalising every PNG flavour to 8-bit RGBA.
    pub fn from_file(file_name: &str) -> Result<Self, String> {
        let file = File::open(file_name)
            .map_err(|e| format!("[read_png_file] File {file_name} could not be opened for reading: {e}"))?;
        Self::from_png_reader(file).map_err(|e| format!("{file_name}: {e}"))
    }

    pub fn from_png_bytes(bytes: &[u8]) -> Result<Self, String> {
        Self::from_png_reader(std::io::Cursor::new(bytes))
    }

    fn from_png_reader<R: std::io::Read>(reader: R) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(reader);
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
        let (width, height) = (info.width, info.height);
        let data = &buffer[..info.buffer_size()];
        let mut image = Self::new(width, height);
        let channels = info.color_type.samples();
        for (i, pixel) in image.pixels.iter_mut().enumerate() {
            let p = &data[i * channels..];
            *pixel = match channels {
                1 => Pixel { r: p[0], g: p[0], b: p[0], a: 255 },
                2 => Pixel { r: p[0], g: p[0], b: p[0], a: p[1] },
                3 => Pixel { r: p[0], g: p[1], b: p[2], a: 255 },
                _ => Pixel { r: p[0], g: p[1], b: p[2], a: p[3] },
            };
        }
        Ok(image)
    }

    /// Copies the width x height region at (x, y) into a new image.
    #[allow(clippy::too_many_arguments)]
    pub fn slice_image(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        left: i32,
        top: i32,
        bottom: i32,
        right: i32,
    ) -> SPImage {
        let x = x.min(self.width);
        let y = y.min(self.height);
        let width = width.min(self.width - x);
        let height = height.min(self.height - y);
        let mut image = SPImage::new(width, height);
        for row in 0..height {
            let src = ((y + row) * self.width + x) as usize;
            let dst = (row * width) as usize;
            image.pixels[dst..dst + width as usize].copy_from_slice(&self.pixels[src..src + width as usize]);
        }
        image.left_border = left;
        image.top_border = top;
        image.right_border = right;
        image.bottom_border = bottom;
        image
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Pixel {
        self.pixels[(x + self.width * y) as usize]
    }

    pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Pixel {
        let width = self.width;
        &mut self.pixels[(x + width * y) as usize]
    }
}

/// A region of a skin image (`slice` in `SPSkin.h`).
#[derive(Debug, Clone)]
pub struct Slice {
    pub image: Rc<SPImage>,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slicing_copies_the_region() {
        let mut image = SPImage::new(4, 4);
        *image.get_pixel_mut(2, 1) = Pixel { r: 255, g: 0, b: 0, a: 255 };
        let slice = image.slice_image(1, 1, 2, 2, 0, 0, 0, 0);
        assert_eq!(slice.width, 2);
        assert_eq!(slice.get_pixel(1, 0).r, 255);
        assert_eq!(slice.get_pixel(1, 0).to_hex(), "#FF0000");
    }
}
