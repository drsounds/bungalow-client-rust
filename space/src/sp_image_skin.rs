//! Port of `SPImageSkin`: a skin whose colours and slices are read from
//! fixed positions of one sprite sheet.

use std::rc::Rc;

use crate::sp_image::{SPImage, Slice};
use crate::sp_skin::{SPSkin, SPSkinBase};

pub struct SPImageSkin {
    base: SPSkinBase,
}

impl SPImageSkin {
    pub fn new(bitmap: &str) -> Result<Self, String> {
        let image = Rc::new(SPImage::from_file(bitmap)?);
        let base = SPSkinBase::new();
        {
            let mut colors = base.colors.borrow_mut();
            for (selector, x) in [
                ("body.background.color", 0),
                ("body.foreground.color", 1),
                ("::selected.background.color", 7),
                ("::selected.foreground.color", 8),
                ("table::nth-child(2n).background.color", 22),
                ("treeitem::selected.background.color", 6),
                ("treeitem::selected.foreground.color", 7),
                ("treeview.foreground.color", 1),
                ("treeitem::focus.foreground.color", 2),
                ("treeview.background.color", 3),
            ] {
                if (x as u32) < image.width && image.height > 0 {
                    colors.insert(selector.to_string(), image.get_pixel(x, 0));
                }
            }
            let mut slices = base.slices.borrow_mut();
            for (selector, x1, y1, x2, y2) in [
                ("#header.background.image", 214, 0, 27, 56),
                ("tab.background.image::active", 0, 11, 32, 23),
                ("tabbar.background.image", 97, 1, 1, 22),
                ("button.background.image", 128, 1, 71, 19),
                ("table::columnheader.background.image", 200, 63, 18, 18),
            ] {
                slices.insert(selector.to_string(), Slice { image: image.clone(), x1, y1, x2, y2 });
            }
        }
        *base.image.borrow_mut() = Some(image);
        Ok(Self { base })
    }
}

impl SPSkin for SPImageSkin {
    fn base(&self) -> &SPSkinBase {
        &self.base
    }
}
