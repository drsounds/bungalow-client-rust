//! Port of `SPWindowElement`: the root of an element tree, sized to the
//! graphics context of its [`Space`].

use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_element::{construct, ElementRefExt, SPElement, SPElementClass};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_navigate_event_args::SPNavigateEventArgs;
use crate::sp_skin::SPSkin;
use crate::space::Space;

#[derive(Default)]
pub struct SPWindowElement {
    pub el: SPElement,
}

impl_element_base!(SPWindowElement, el);

impl SPWindowElement {
    pub fn new(space: &Rc<Space>, skin: Option<Rc<dyn SPSkin>>) -> Rc<Self> {
        let rc = construct(Self::default(), None);
        rc.el.set_tag_name("window");
        rc.el.set_space(space);
        rc.el.set_skin(skin);
        rc
    }

    pub fn draw(&self, gc: &dyn SPGraphicsContext, _width: i32, _height: i32) {
        for element in self.el.get_child_nodes() {
            if element.el().is_visible() {
                element.paint(0, 0, gc);
            }
        }
    }

    pub fn invalidate_region(&self, region: SPRectangle) {
        if let Some(gc) = self.create_graphics() {
            gc.invalidate_region(region);
        }
    }

    /// Dispatches "navigate"; the application decides what to show.
    pub fn navigate(&self, uri: &str) {
        let args = SPNavigateEventArgs::new(uri);
        let this = self.el.this();
        self.el.dispatch_event("navigate", &args, Some(&this.as_object()));
    }
}

impl SPElementClass for SPWindowElement {
    fn get_type(&self) -> String {
        "window".into()
    }
    fn pack(&self) {
        let Some(gc) = self.create_graphics() else {
            return;
        };
        let (width, height) = (gc.get_width(), gc.get_height());
        for child in self.el.get_child_nodes() {
            let c = child.el();
            c.set_x(0);
            c.set_y(0);
            c.set_width(width);
            c.set_height(height);
            child.pack();
        }
    }
}
