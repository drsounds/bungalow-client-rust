//! Port of `space::SPSpiderElement`: a tab bar view whose content is
//! rendered by an elua template.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_event_args::SPEventArgs;
use space::sp_object::SPObject;
use space::sp_tab_bar_view_element::SPTabBarViewElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

use crate::spider_view::SpiderView;

#[derive(Default)]
pub struct SPSpiderElement {
    pub tab_bar_view: SPTabBarViewElement,
    template: RefCell<String>,
    view: RefCell<Option<SpiderView>>,
}

impl_element_base!(SPSpiderElement, tab_bar_view);

impl SPSpiderElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.tab_bar_view.init_tab_bar_view();
        rc.element().set_tag_name("spider");
        rc
    }

    pub fn set_template(&self, filename: &str) {
        *self.template.borrow_mut() = filename.to_string();
        let mut view = SpiderView::new("");
        view.set_template(Path::new(filename));
        *self.view.borrow_mut() = Some(view);
    }

    pub fn get_template(&self) -> String {
        self.template.borrow().clone()
    }

    /// Renders the template for `method` and loads the markup it printed.
    pub fn process(&self, method: &str, data: &BTreeMap<String, String>) {
        let uri = self.element().get_string_attribute("src", "");
        let markup = match self.view.borrow_mut().as_mut() {
            Some(view) => view.process(method, &uri, data),
            None => return,
        };
        match markup {
            Ok(markup) => match space::sp_xml_parser::parse_sp_xml(&markup) {
                Ok(root) => self.element().load_xul_from_xml_doc(&root),
                Err(e) => log::error!("{e}"),
            },
            Err(e) => log::error!("{e}"),
        }
    }

    pub fn render(&self) {
        self.process("GET", &BTreeMap::new());
    }
}

impl SPViewClass for SPSpiderElement {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.tab_bar_view.accepts_uri_default(uri)
    }
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(hash);
    }
}

impl SPElementClass for SPSpiderElement {
    fn get_type(&self) -> String {
        "spider".into()
    }
    fn pack(&self) {
        SPViewElement::pack_view(self.element());
    }
    fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
        SPViewElement::scroll_view(self.element(), scroll_x, scroll_y);
    }
    fn mouse_down(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        self.element().mouse_down_default(sender, e);
    }
    fn get_view_stack_element(&self) -> Option<ElementRef> {
        Some(self.tab_bar_view.tab_stack())
    }
    fn get_slot_element(&self) -> ElementRef {
        self.tab_bar_view.tab_stack()
    }
    fn clear(&self) {
        self.tab_bar_view.clear_tab_bar_view();
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}
