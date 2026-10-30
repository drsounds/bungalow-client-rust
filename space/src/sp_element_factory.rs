//! Port of `SPElementFactory`: maps markup tag names to element classes.

use crate::sp_button_element::SPButtonElement;
use crate::sp_check_box_element::SPCheckBoxElement;
use crate::sp_element::{ElementRef, ElementRefExt, SPElement};
use crate::sp_hbox_element::SPHBoxElement;
use crate::sp_image_element::SPImageElement;
use crate::sp_input_element::SPInputElement;
use crate::sp_page_element::SPPageElement;
use crate::sp_section_element::SPSectionElement;
use crate::sp_splitter_element::SPSplitterElement;
use crate::sp_tab_bar_view_element::SPTabBarViewElement;
use crate::sp_text_element::SPTextElement;
use crate::sp_vbox_element::SPVBoxElement;

/// Creates the element class for `tag_name` as a child of `parent`.
pub fn create_element(tag_name: &str, parent: &ElementRef) -> ElementRef {
    let parent = Some(parent);
    let elm: ElementRef = match tag_name {
        "text" => SPTextElement::new(parent),
        "img" => SPImageElement::new(parent),
        "button" => SPButtonElement::new(parent),
        "input" => SPInputElement::new(parent),
        "checkbox" => SPCheckBoxElement::new(parent),
        "view" => SPTabBarViewElement::new(parent),
        "vbox" => SPVBoxElement::new(parent),
        "splitter" => SPSplitterElement::new(parent),
        "hbox" => SPHBoxElement::new(parent),
        "section" => SPSectionElement::new(parent),
        "page" => SPPageElement::new(parent),
        _ => SPElement::new(parent),
    };
    elm.el().set_tag_name(tag_name);
    elm
}
