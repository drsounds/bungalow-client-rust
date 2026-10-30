//! Port of `SPVBoxElement`: stacks visible children vertically, sharing the
//! remaining height between children with a flex.

use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};

#[derive(Default)]
pub struct SPVBoxElement {
    pub el: SPElement,
}

impl_element_base!(SPVBoxElement, el);

impl SPVBoxElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("vbox");
        rc
    }

    /// `SPVBoxElement::pack()`, callable by subclasses.
    pub fn pack_vbox(el: &SPElement) {
        el.pack_default();
        let padding = el.get_padding();
        let mut children = Vec::new();
        let mut total_flex = 0;
        let mut fixed_height = padding.top + padding.bottom;
        for child in el.get_child_nodes() {
            let c = child.element();
            if !c.is_visible() {
                continue;
            }
            let margins = c.get_margins();
            fixed_height += margins.top + margins.bottom;
            if c.get_flex() > 0 {
                total_flex += c.get_flex();
            } else {
                fixed_height += c.get_height();
            }
            children.push(child);
        }

        let gap = el.get_gap();
        if children.len() > 1 {
            fixed_height += gap * (children.len() as i32 - 1);
        }
        let flex_height = (el.get_height() - fixed_height).max(0);
        let mut cumulative_flex = 0;
        let mut assigned_flex_height = 0;
        let mut y = padding.top;
        let content_width = (el.get_width() - padding.left - padding.right).max(0);

        for (i, child) in children.iter().enumerate() {
            let c = child.element();
            let margins = c.get_margins();
            c.set_x(padding.left + margins.left);
            c.set_y(y + margins.top);
            c.set_width((content_width - margins.left - margins.right).max(0));

            if c.get_flex() > 0 {
                cumulative_flex += c.get_flex();
                let next_assigned_height = if total_flex > 0 {
                    ((flex_height as i64 * cumulative_flex as i64) / total_flex as i64) as i32
                } else {
                    assigned_flex_height
                };
                c.set_height(next_assigned_height - assigned_flex_height);
                assigned_flex_height = next_assigned_height;
            }

            y += margins.top + c.get_height() + margins.bottom;
            if i + 1 < children.len() {
                y += gap;
            }
        }

        for child in children {
            child.pack();
        }
    }
}

impl SPElementClass for SPVBoxElement {
    fn get_type(&self) -> String {
        "vbox".into()
    }
    fn pack(&self) {
        Self::pack_vbox(&self.el);
    }
}
