//! Port of `SPGraphicsContext`, the drawing backend interface.
//!
//! Methods take `&self`: backends keep their state (clip stack, pending
//! input overlays) behind interior mutability, so a context can be shared
//! as `Rc<dyn SPGraphicsContext>` between the [`crate::Space`] and its windows.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::libspace::SPRectangle;
use crate::sp_color::{SPColor, SPGradient};
use crate::sp_element::ElementRef;
use crate::sp_font_style::SPFontStyle;
use crate::sp_image::{SPImage, Slice};

pub type RequestCallback = Box<dyn FnOnce(bool, String)>;

#[allow(unused_variables)]
pub trait SPGraphicsContext {
    fn request(&self, method: &str, uri: &str, data: &str, callback: RequestCallback) {
        callback(false, String::new());
    }
    fn set_control_attribute(&self, element: &ElementRef, name: &str, value: &str) {}
    #[allow(clippy::too_many_arguments)]
    fn draw_image_part(
        &self,
        image: &SPImage,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    ) {
    }
    fn draw_skin_image(&self, resource_id: &str, x: i32, y: i32, w: i32, h: i32) {}
    fn restore_origo(&self) {}
    fn set_origo(&self, x: i32, y: i32) {}
    fn set_clip(&self, rect: &SPRectangle) {}
    fn declip_rect(&self) {}
    fn draw_line(&self, x: i32, y: i32, x2: i32, y2: i32, color: SPColor) {}
    fn draw_rectangle(&self, x: i32, y: i32, w: i32, h: i32, color: SPColor) {}
    fn fill_rectangle(&self, x: i32, y: i32, w: i32, h: i32, color: SPColor) {}
    fn fill_rounded_rectangle(&self, x: i32, y: i32, w: i32, h: i32, r: i32, color: SPColor) {}
    fn draw_rounded_rectangle(&self, x: i32, y: i32, w: i32, h: i32, r: i32, color: SPColor) {}
    #[allow(clippy::too_many_arguments)]
    fn fill_rounded_linear_gradient_rectangle(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        r: i32,
        dir: i32,
        gradient: &SPGradient,
    ) {
    }
    fn draw_image(&self, image: &SPImage, x: i32, y: i32, w: i32, h: i32) {}
    /// Frees whatever the backend cached for `image` (e.g. its texture), for
    /// images that are replaced every frame.
    fn release_image(&self, image: &SPImage) {}
    fn draw_control(&self, name: &str, element: &ElementRef, x: i32, y: i32, w: i32, h: i32) {}
    #[allow(clippy::too_many_arguments)]
    fn draw_string(&self, text: &str, font: &SPFontStyle, color: SPColor, x: i32, y: i32, w: i32, h: i32) {}
    #[allow(clippy::too_many_arguments)]
    fn draw_html_text(&self, html: &str, font: &SPFontStyle, color: SPColor, x: i32, y: i32, w: i32, h: i32) {}
    fn invalidate_region(&self, region: SPRectangle) {}
    fn draw_linear_gradient(&self, rect: &SPRectangle, gradient: &SPGradient) {}
    fn load_image(&self, path: &str) -> Option<Rc<SPImage>> {
        SPImage::from_file(path).ok().map(Rc::new)
    }
    fn freeze(&self) {}
    fn thaw(&self) {}
    fn measure_string(&self, text: &str, font: &SPFontStyle) -> SPRectangle {
        SPRectangle::default()
    }
    fn get_width(&self) -> i32 {
        0
    }
    fn get_height(&self) -> i32 {
        0
    }
    /// Called when the skin, colour scheme or tint changes so backends can drop pre-rendered assets.
    fn theme_changed(&self) {}
    fn draw_input_element(&self, element: &ElementRef, x: i32, y: i32, w: i32, h: i32) {}

    #[allow(clippy::too_many_arguments)]
    fn draw_image_slices(
        &self,
        slices: &BTreeMap<String, Slice>,
        selector: &str,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
    ) {
        if let Some(slice) = slices.get(selector) {
            self.draw_image_part(&slice.image, left, top, width, height, slice.x1, slice.y1, slice.x2, slice.y2);
        }
    }
}

/// A context that draws nothing, used by tests and headless runs. It
/// measures text with a fixed advance so layout code still works.
#[derive(Default)]
pub struct SPNullGraphicsContext {
    pub width: i32,
    pub height: i32,
}

impl SPGraphicsContext for SPNullGraphicsContext {
    fn measure_string(&self, text: &str, font: &SPFontStyle) -> SPRectangle {
        let advance = (font.height.max(1) * 6) / 10;
        SPRectangle::new(0, 0, text.chars().count() as i32 * advance, font.height + 4)
    }
    fn get_width(&self) -> i32 {
        self.width
    }
    fn get_height(&self) -> i32 {
        self.height
    }
}
