//! Port of `SPTabBarElement`: a row of tabs plus right-aligned tool buttons.

use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::impl_event_args;
use crate::sp_element::{construct, ElementRef, ElementRefExt, SPElement, SPElementClass};
use crate::sp_event_args::{downcast_args, SPEventArgs};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_image::SPImage;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_object::SPObject;

#[derive(Clone, Default)]
pub struct SPTabToolButton {
    pub id: String,
    pub label: String,
    pub icon: Option<Rc<SPImage>>,
}

pub struct TabBarToolButtonEventArgs {
    pub button_id: String,
}
impl_event_args!(TabBarToolButtonEventArgs);

#[derive(Debug, Clone, PartialEq)]
pub struct SPTab {
    pub id: String,
    pub name: String,
    pub uri: String,
}

impl SPTab {
    pub fn new(name: &str, id: &str, uri: &str) -> Self {
        Self { id: id.to_string(), name: name.to_string(), uri: uri.to_string() }
    }
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
    pub fn get_uri(&self) -> String {
        self.uri.clone()
    }
}

pub struct TabBarEventArgs {
    pub tab: Rc<SPTab>,
}
impl_event_args!(TabBarEventArgs);

#[derive(Default)]
pub struct SPTabBarElement {
    pub el: SPElement,
    pub tab_background_image: RefCell<Option<Rc<SPImage>>>,
    pub active_tab_background_image: RefCell<Option<Rc<SPImage>>>,
    tabs: RefCell<Vec<Rc<SPTab>>>,
    active_tab: RefCell<Option<Rc<SPTab>>>,
    tool_buttons: RefCell<Vec<SPTabToolButton>>,
}

impl_element_base!(SPTabBarElement, el);

impl SPTabBarElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("tabbar");
        let weak = Rc::downgrade(&rc);
        rc.el.add_event_listener(
            "mousedown",
            Rc::new(move |sender, e| {
                if let Some(this) = weak.upgrade() {
                    this.mouse_down(sender, e);
                }
                0
            }),
        );
        rc
    }

    pub fn get_tabs(&self) -> Vec<Rc<SPTab>> {
        self.tabs.borrow().clone()
    }

    pub fn get_active_tab(&self) -> Option<Rc<SPTab>> {
        self.active_tab.borrow().clone()
    }

    pub fn is_active(&self, tab: &SPTab) -> bool {
        self.active_tab.borrow().as_deref().is_some_and(|t| std::ptr::eq(t, tab))
    }

    pub fn add_tab(&self, id: &str, name: &str, uri: &str) {
        let tab = Rc::new(SPTab::new(name, id, uri));
        self.tabs.borrow_mut().push(tab.clone());
        if self.tabs.borrow().len() == 1 {
            *self.active_tab.borrow_mut() = Some(tab);
        }
    }

    pub fn remove_tab(&self, tab: &Rc<SPTab>) {
        self.tabs.borrow_mut().retain(|t| !Rc::ptr_eq(t, tab));
    }

    pub fn remove(&self, uri: &str) {
        let found = self.tabs.borrow().iter().find(|t| t.uri == uri).cloned();
        if let Some(tab) = found {
            self.remove_tab(&tab);
        }
    }

    pub fn add_tool_button(&self, id: &str, label: &str, icon: Option<Rc<SPImage>>) {
        self.tool_buttons.borrow_mut().push(SPTabToolButton { id: id.to_string(), label: label.to_string(), icon });
    }

    pub fn remove_tool_button(&self, id: &str) {
        self.tool_buttons.borrow_mut().retain(|b| b.id != id);
    }

    pub fn get_tool_buttons(&self) -> Vec<SPTabToolButton> {
        self.tool_buttons.borrow().clone()
    }

    fn select(&self, tab: Rc<SPTab>) {
        *self.active_tab.borrow_mut() = Some(tab.clone());
        let this = self.el.this();
        this.notify("tabselected", Some(&this.as_object()), &TabBarEventArgs { tab });
    }
}

impl SPElementClass for SPTabBarElement {
    fn get_type(&self) -> String {
        "tabbar".into()
    }

    fn clear(&self) {
        self.tabs.borrow_mut().clear();
    }

    fn mouse_down(&self, _sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let Some(me) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        let Some(g) = self.create_graphics() else {
            return;
        };
        let x = me.get_x();
        let font = self.el.get_font_style();
        let mut tab_hit = false;
        let mut left = self.el.get_absolute_bounds().x;
        for tab in self.get_tabs() {
            let str_size = g.measure_string(&tab.name, &font);
            let width = 20 + str_size.width + 20;
            if x > left && x < left + width {
                self.select(tab);
                tab_hit = true;
                break;
            }
            left += width;
        }
        let buttons = self.get_tool_buttons();
        if !tab_hit && !buttons.is_empty() {
            let mut tool_right = self.el.get_absolute_bounds().x + self.el.get_width();
            for button in buttons.iter().rev() {
                let size = g.measure_string(&button.label, &font);
                let btn_w = if button.label.is_empty() { self.el.get_height() } else { 8 + size.width + 8 };
                let btn_left = tool_right - btn_w;
                if x >= btn_left && x < tool_right {
                    let this = self.el.this();
                    this.notify(
                        "toolbuttonclicked",
                        Some(&this.as_object()),
                        &TabBarToolButtonEventArgs { button_id: button.id.clone() },
                    );
                    break;
                }
                tool_right = btn_left;
            }
        }
        self.el.invalidate();
    }

    fn paint_content(&self, x: i32, y: i32, width: i32, height: i32, g: &dyn SPGraphicsContext) {
        let Some(skin) = self.el.get_skin() else {
            return;
        };
        let font = self.el.get_font_style();
        skin.draw_tab_bar(g, self, x, y, width, height);
        let mut left = 0;
        for (index, tab) in self.get_tabs().iter().enumerate() {
            let mut fg_color = self.el.get_foreground_color();
            let mut shadow_color = fg_color.lighten(-10.0);
            let active = self.is_active(tab);
            if !active {
                fg_color = fg_color.lighten(-2.0);
                shadow_color = fg_color.lighten(10.0);
            }
            let str_size = g.measure_string(&tab.name, &font);
            let tab_width = 20 + str_size.width + 20;
            skin.draw_tab_bar_tab(g, self, tab, index as i32, active, x + left, y, tab_width, height);
            let text_x = left + x + (tab_width / 2) - (str_size.width / 2);
            let text_y = y + (height / 2) - (str_size.height / 2);
            g.draw_string(&tab.name, &font, shadow_color, text_x, text_y - 1, str_size.width, str_size.height);
            g.draw_string(&tab.name, &font, fg_color, text_x, text_y - 2, str_size.width, str_size.height);
            left += tab_width;
        }
        let mut tool_right = x + width;
        for button in self.get_tool_buttons().iter().rev() {
            let size = g.measure_string(&button.label, &font);
            let btn_w = if button.label.is_empty() { height } else { 8 + size.width + 8 };
            let btn_x = tool_right - btn_w;
            skin.draw_tab_bar_tool_button(g, self, button, btn_x, y, btn_w, height);
            if !button.label.is_empty() {
                let fg = self.el.get_foreground_color();
                let shadow = fg.lighten(-10.0);
                let txt_x = btn_x + (btn_w / 2) - (size.width / 2);
                let txt_y = y + (height / 2) - (size.height / 2);
                g.draw_string(&button.label, &font, shadow, txt_x, txt_y - 1, size.width, size.height);
                g.draw_string(&button.label, &font, fg, txt_x, txt_y - 2, size.width, size.height);
            }
            tool_right = btn_x;
        }
    }
}
