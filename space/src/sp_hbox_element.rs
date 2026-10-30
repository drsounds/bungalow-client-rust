//! Port of `SPHBoxElement`: lays visible children out horizontally, sharing
//! the remaining width between children with a flex.

use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};

#[derive(Default)]
pub struct SPHBoxElement {
    pub el: SPElement,
}

impl_element_base!(SPHBoxElement, el);

impl SPHBoxElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("hbox");
        rc
    }

    /// `SPHBoxElement::pack()`, callable by subclasses.
    pub fn pack_hbox(el: &SPElement) {
        el.pack_default();
        let padding = el.get_padding();
        let mut children = Vec::new();
        let mut total_flex = 0;
        let mut fixed_width = padding.left + padding.right;
        for child in el.get_child_nodes() {
            let c = child.element();
            if !c.is_visible() {
                continue;
            }
            let margins = c.get_margins();
            fixed_width += margins.left + margins.right;
            if c.get_flex() > 0 {
                total_flex += c.get_flex();
            } else {
                fixed_width += c.get_width();
            }
            children.push(child);
        }

        let gap = el.get_gap();
        if children.len() > 1 {
            fixed_width += gap * (children.len() as i32 - 1);
        }
        let flex_width = (el.get_width() - fixed_width).max(0);
        let mut cumulative_flex = 0;
        let mut assigned_flex_width = 0;
        let mut x = padding.left;
        let content_height = (el.get_height() - padding.top - padding.bottom).max(0);

        for (i, child) in children.iter().enumerate() {
            let c = child.element();
            let margins = c.get_margins();
            c.set_x(x + margins.left);
            c.set_y(padding.top + margins.top);
            c.set_height((content_height - margins.top - margins.bottom).max(0));

            if c.get_flex() > 0 {
                cumulative_flex += c.get_flex();
                let next_assigned_width = if total_flex > 0 {
                    ((flex_width as i64 * cumulative_flex as i64) / total_flex as i64) as i32
                } else {
                    assigned_flex_width
                };
                c.set_width(next_assigned_width - assigned_flex_width);
                assigned_flex_width = next_assigned_width;
            }

            x += margins.left + c.get_width() + margins.right;
            if i + 1 < children.len() {
                x += gap;
            }
        }

        for child in children {
            child.pack();
        }
    }
}

impl SPElementClass for SPHBoxElement {
    fn get_type(&self) -> String {
        "hbox".into()
    }
    fn pack(&self) {
        Self::pack_hbox(&self.el);
    }
}
