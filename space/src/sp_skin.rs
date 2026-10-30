//! Port of `SPSkin`, the base class of every skin.
//!
//! The C++ base class mixes data (HSL tint, image/colour maps) with virtual
//! drawing hooks. The data lives in [`SPSkinBase`]; the hooks are the
//! [`SPSkin`] trait, whose implementors hand out their base via [`SPSkin::base`].

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::libspace::SPRectangle;
use crate::sp_color::SPColor;
use crate::sp_element::{ElementRef, SP_COLOR_SCHEME_DARK};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_image::{Pixel, SPImage, Slice};
use crate::sp_tab_bar_element::{SPTab, SPTabBarElement, SPTabToolButton};
use crate::sp_table_element::SPTableElement;
use crate::sp_tree_item::SPTreeItem;
use crate::sp_tree_view_element::SPTreeViewElement;

pub const SP_MODE_DEFAULT: i32 = 0;
pub const SP_MODE_DARK: i32 = 1;
pub const SP_MODE_LIGHT: i32 = 2;

/// Button texture directive — skins use this to decide how to paint button fills.
/// Glossy adds a separate specular sheen layer on top, independent of this setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SPButtonStyle {
    /// Plain solid fill, no gradient
    Flat = 0,
    /// Classic Bootstrap 1.x vertical light→dark gradient
    #[default]
    Gradient = 1,
    /// Chromatic vertical gradient (accent → secondary hue)
    ModernGradient = 2,
    /// macOS X Aqua gel — vivid base + white highlight band
    Aqua = 3,
}

impl SPButtonStyle {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Flat,
            2 => Self::ModernGradient,
            3 => Self::Aqua,
            _ => Self::Gradient,
        }
    }
}

/// The data part of `SPSkin`.
#[derive(Default)]
pub struct SPSkinBase {
    hue: Cell<f32>,
    saturation: Cell<f32>,
    lightness: Cell<f32>,
    pub images: RefCell<BTreeMap<String, Rc<SPImage>>>,
    pub colors: RefCell<BTreeMap<String, Pixel>>,
    pub slices: RefCell<BTreeMap<String, Slice>>,
    pub image: RefCell<Option<Rc<SPImage>>>,
}

impl SPSkinBase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_hue(&self, h: f32) {
        self.hue.set(h);
    }
    pub fn get_hue(&self) -> f32 {
        self.hue.get()
    }
    pub fn set_saturation(&self, s: f32) {
        self.saturation.set(s);
    }
    pub fn get_saturation(&self) -> f32 {
        self.saturation.get()
    }
    pub fn set_lightness(&self, l: f32) {
        self.lightness.set(l);
    }
    pub fn get_lightness(&self) -> f32 {
        self.lightness.get()
    }

    /// Very pale tinted background (light mode).
    pub fn bg_light(&self) -> SPColor {
        // Vista-style: very pale, slightly blue-tinted gray
        let l = 0.935 - self.get_lightness() * 0.025;
        SPColor::from_hsl(self.get_hue(), self.get_saturation() * 0.11, l, 255)
    }

    /// Dark navy background (dark mode).
    pub fn bg_dark(&self) -> SPColor {
        // Same hue, dark navy — lightness is inverted relative to light
        let l = 0.10 + self.get_lightness() * 0.06;
        SPColor::from_hsl(self.get_hue(), self.get_saturation() * 0.22, l, 255)
    }

    /// Resolved background for the given scheme.
    pub fn bg(&self, scheme: i16) -> SPColor {
        if scheme == SP_COLOR_SCHEME_DARK {
            self.bg_dark()
        } else {
            self.bg_light()
        }
    }

    /// Vivid accent — used for selections, active tabs, primary buttons.
    pub fn accent(&self) -> SPColor {
        let l = 0.52 + self.get_lightness() * 0.08;
        SPColor::from_hsl(self.get_hue(), self.get_saturation() * 0.85, l, 255)
    }

    /// Tropical tab-gradient start.
    pub fn tab_start(&self) -> SPColor {
        // Lighter tropical start — aqua/cyan range
        SPColor::from_hsl(self.get_hue(), self.get_saturation() * 0.90, 0.74 + self.get_lightness() * 0.06, 255)
    }

    /// Tropical tab-gradient end (shifted +30° hue).
    pub fn tab_end(&self) -> SPColor {
        // Slightly deeper end with tropical hue shift, still light
        SPColor::from_hsl(self.get_hue() + 30.0, self.get_saturation() * 0.80, 0.64 + self.get_lightness() * 0.06, 255)
    }

    /// Text colour appropriate for the colour scheme.
    pub fn text_color(&self, scheme: i16) -> SPColor {
        if scheme == SP_COLOR_SCHEME_DARK {
            return SPColor::from_hsl(self.get_hue(), 0.10, 0.90, 255);
        }
        // Dark blue in light mode — tied to the current hue for coherence
        SPColor::from_hsl(self.get_hue(), 0.55, 0.17, 255)
    }

    pub fn has_color(&self, selector: &str) -> bool {
        self.colors.borrow().contains_key(selector)
    }

    pub fn has_image(&self, selector: &str) -> bool {
        self.images.borrow().contains_key(selector)
    }

    pub fn get_color(&self, selector: &str, _def: &str) -> SPColor {
        match self.colors.borrow().get(selector) {
            Some(pixel) => pixel.to_color(),
            None => SPColor::new(255, 255, 255, 0),
        }
    }

    pub fn get_image(&self, selector: &str, _def: &str) -> Option<Rc<SPImage>> {
        self.images.borrow().get(selector).cloned()
    }
}

/// The virtual part of `SPSkin`. Every hook defaults to "not handled".
#[allow(unused_variables, clippy::too_many_arguments)]
pub trait SPSkin {
    fn base(&self) -> &SPSkinBase;

    fn draw_element(&self, gc: &dyn SPGraphicsContext, element: &ElementRef, state: &str, bounds: &SPRectangle) -> bool {
        false
    }
    /// The part of an element's bounds left for its content once the
    /// skin's frame is drawn, e.g. where a native edit control goes.
    fn get_content_bounds(&self, element: &ElementRef, bounds: &SPRectangle) -> SPRectangle {
        *bounds
    }
    /// Label colour the skin wants for an element in a state, if any.
    fn get_text_color(&self, element: &ElementRef, state: &str) -> Option<SPColor> {
        None
    }
    fn draw_table_row(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, i: i32, x: i32, y: i32, width: i32, height: i32, row: i32) {}
    fn draw_table_header(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, x: i32, y: i32, width: i32, height: i32) {}
    fn draw_tab_bar_tab(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, tab: &SPTab, index: i32, active: bool, x: i32, y: i32, width: i32, height: i32) {}
    fn draw_tab_bar_tool_button(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, button: &SPTabToolButton, x: i32, y: i32, width: i32, height: i32) {}
    fn draw_tab_bar(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, x: i32, y: i32, width: i32, height: i32) {}
    fn draw_tree_item(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, item: &SPTreeItem, item_height: i32, x: i32, y: i32, width: i32, height: i32) {}
    fn draw_section_header(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, title: &str, x: i32, y: i32, width: i32, height: i32) {}

    fn set_glossy(&self, value: bool) {}
    fn is_glossy(&self) -> bool {
        true
    }

    fn set_button_style(&self, style: SPButtonStyle) {}
    fn get_button_style(&self) -> SPButtonStyle {
        SPButtonStyle::Gradient
    }
}

/// A skin with no drawing hooks: elements fall back to their own painting.
#[derive(Default)]
pub struct SPPlainSkin {
    base: SPSkinBase,
}

impl SPSkin for SPPlainSkin {
    fn base(&self) -> &SPSkinBase {
        &self.base
    }
}
