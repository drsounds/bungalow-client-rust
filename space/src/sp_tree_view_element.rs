//! Port of `SPTreeViewElement`: the sidebar tree with sections, expandable
//! items, icons, selection and context menus.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::impl_element_base;
use crate::impl_event_args;
use crate::libspace::SPRectangle;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, ElementRefExt, SPElement, SPElementClass};
use crate::sp_event_args::{downcast_args, SPEventArgs};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_image::SPImage;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_tree_item::SPTreeItem;

#[derive(Default)]
pub struct TreeViewEventArgs {
    pub item: Option<Rc<SPTreeItem>>,
    pub x: i32,
    pub y: i32,
}
impl_event_args!(TreeViewEventArgs);

impl TreeViewEventArgs {
    pub fn get_item(&self) -> Option<Rc<SPTreeItem>> {
        self.item.clone()
    }
    pub fn get_x(&self) -> i32 {
        self.x
    }
    pub fn get_y(&self) -> i32 {
        self.y
    }
}

pub struct SPTreeViewElement {
    pub el: SPElement,
    items: RefCell<Vec<Rc<SPTreeItem>>>,
    item_height: Cell<i32>,
    icon_cache: RefCell<BTreeMap<String, Option<Rc<SPImage>>>>,
}

impl_element_base!(SPTreeViewElement, el);

impl SPTreeViewElement {
    pub const SECTION_HEIGHT: i32 = 22;
    pub const INDENT: i32 = 20;
    pub const ARROW_WIDTH: i32 = 16;
    pub const CHILD_INDENT: i32 = 32;
    /// icon rendered at 16×16
    pub const ICON_SIZE: i32 = 16;
    /// 16px icon + 4px gap
    pub const ICON_SLOT: i32 = 20;

    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(
            Self {
                el: SPElement::default(),
                items: RefCell::default(),
                item_height: Cell::new(24),
                icon_cache: RefCell::default(),
            },
            parent,
        );
        rc.el.set_tag_name("treeview");
        let weak = Rc::downgrade(&rc);
        rc.el.add_event_listener(
            "mousedown",
            Rc::new(move |_, e| {
                if let Some(this) = weak.upgrade() {
                    this.treeview_mousedown(e);
                }
                0
            }),
        );
        let weak = Rc::downgrade(&rc);
        rc.el.add_event_listener(
            "mouseup",
            Rc::new(move |_, e| {
                if let Some(this) = weak.upgrade() {
                    this.treeview_mouseup(e);
                }
                0
            }),
        );
        rc
    }

    pub fn get_items(&self) -> Vec<Rc<SPTreeItem>> {
        self.items.borrow().clone()
    }

    pub fn set_item_height(&self, height: i32) {
        self.item_height.set(height);
    }

    pub fn get_item_height(&self) -> i32 {
        self.item_height.get()
    }

    pub fn add_item(&self, item: Rc<SPTreeItem>) {
        self.items.borrow_mut().push(item);
    }

    pub fn add_section(&self, title: &str) -> Rc<SPTreeItem> {
        // no URI — section headers are not navigable
        let item = SPTreeItem::section();
        item.set_text(title);
        item.set_is_section(true);
        item.set_collapsed(false);
        self.items.borrow_mut().push(item.clone());
        item
    }

    pub fn clear_items(&self) {
        self.items.borrow_mut().clear();
        self.icon_cache.borrow_mut().clear();
    }

    pub fn for_each_item(&self, f: impl Fn(&Rc<SPTreeItem>)) {
        for item in self.get_items() {
            f(&item);
            for child in item.get_children() {
                f(&child);
            }
        }
    }

    fn flatten_visible(&self) -> Vec<(Rc<SPTreeItem>, i32)> {
        let mut result = Vec::new();
        for item in self.get_items() {
            let expand = !item.is_section() && item.has_children() && !item.is_collapsed();
            let children = item.get_children();
            result.push((item, 0));
            if expand {
                result.extend(children.into_iter().map(|c| (c, 1)));
            }
        }
        result
    }

    fn row_height(&self, item: &SPTreeItem) -> i32 {
        if item.is_section() {
            Self::SECTION_HEIGHT
        } else {
            self.item_height.get()
        }
    }

    fn notify_item(&self, event: &str, args: TreeViewEventArgs) {
        let this = self.el.this();
        this.notify(event, Some(&this.as_object()), &args);
    }

    fn invalidate_tree(&self) {
        let bounds = self.el.get_absolute_bounds();
        let region = SPRectangle::new(bounds.x, bounds.y, self.el.get_width(), self.el.get_height());
        if let Some(window) = self.el.get_window_element() {
            if let Some(gc) = window.create_graphics() {
                gc.invalidate_region(region);
            }
        }
    }

    fn treeview_mousedown(&self, e: &dyn SPEventArgs) {
        let Some(args) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        // Right-click mousedown: do nothing here, handle on mouseup
        if args.get_button() == 1 {
            return;
        }
        let rel_y = args.get_y() - self.el.get_absolute_bounds().y;
        let mut top = 0;
        for (item, _depth) in self.flatten_visible() {
            let row_h = self.row_height(&item);
            if rel_y > top && rel_y < top + row_h {
                if !item.is_section() {
                    item.set_highlighted(true);
                    self.notify_item("itemhighlighted", TreeViewEventArgs { item: Some(item.clone()), ..Default::default() });
                }
            } else {
                item.set_highlighted(false);
            }
            top += row_h;
        }
        self.invalidate_tree();
    }

    fn treeview_mouseup(&self, e: &dyn SPEventArgs) {
        let Some(args) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        let rel_x = args.get_x() - self.el.get_absolute_bounds().x;
        let rel_y = args.get_y() - self.el.get_absolute_bounds().y;

        // Clear all highlights
        self.for_each_item(|item| item.set_highlighted(false));

        let mut top = 0;
        let mut clicked: Option<(Rc<SPTreeItem>, i32)> = None;
        for (item, depth) in self.flatten_visible() {
            let row_h = self.row_height(&item);
            if rel_y > top && rel_y < top + row_h {
                clicked = Some((item, depth));
            }
            top += row_h;
        }

        if let Some((item, depth)) = clicked.filter(|(item, _)| !item.is_section()) {
            if args.get_button() == 1 {
                // Fire context menu event (only for bookmark items)
                if item.is_bookmark() {
                    self.notify_item(
                        "itemcontextmenu",
                        TreeViewEventArgs { item: Some(item), x: args.get_x(), y: args.get_y() },
                    );
                }
            } else {
                // Left click: check if user clicked the arrow area for expandable items
                let text_indent = if depth == 1 { Self::CHILD_INDENT } else { Self::INDENT };
                if item.has_children() && rel_x < text_indent {
                    // Toggle collapse — no navigation event
                    item.set_collapsed(!item.is_collapsed());
                } else {
                    // Navigate: deselect all, select this item, fire event
                    self.for_each_item(|item| item.set_selected(false));
                    item.set_selected(true);
                    self.notify_item("itemselected", TreeViewEventArgs { item: Some(item), ..Default::default() });
                }
            }
        }
        self.invalidate_tree();
    }
}

impl SPElementClass for SPTreeViewElement {
    fn get_type(&self) -> String {
        "treeview".into()
    }

    fn paint_content(&self, x: i32, y: i32, width: i32, height: i32, g: &dyn SPGraphicsContext) {
        // Background
        let mut bg_color = self.el.get_background_color();
        if let Some(tint) = self.el.get_tint_color() {
            bg_color = bg_color.tint(&tint);
        }
        let skin = self.el.get_skin();
        let this = self.el.this();
        if !skin
            .as_ref()
            .is_some_and(|skin| skin.draw_element(g, &this, "", &SPRectangle::new(x, y, width, height)))
        {
            g.fill_rectangle(x, y, width, height, bg_color);
        }

        let color = self.el.get_foreground_color();
        let fs = self.el.get_font_style();
        let mut top = 0;
        for (item, depth) in self.flatten_visible() {
            let row_h = self.row_height(&item);
            if item.is_section() {
                if let Some(skin) = &skin {
                    skin.draw_section_header(g, self, &item.get_text(), x, y + top, width, row_h);
                }
            } else {
                // Selection / highlight background
                if let Some(skin) = &skin {
                    skin.draw_tree_item(g, self, &item, row_h, x, y + top, width, row_h);
                }
                // Disclosure arrow for expandable items
                if item.has_children() {
                    let arrow = if item.is_collapsed() { "\u{25b6}" } else { "\u{25bc}" };
                    g.draw_string(arrow, &fs, SPColor::new(128, 128, 128, 180), x + 4, y + top, Self::ARROW_WIDTH, row_h);
                }
                // Icon — always reserve ICON_SLOT px regardless of whether icon loads
                let icon_x = x + if depth == 1 { Self::CHILD_INDENT } else { Self::INDENT };
                let icon_path = item.get_icon_path();
                if !icon_path.is_empty() {
                    // None if load failed — don't retry
                    let icon = self
                        .icon_cache
                        .borrow_mut()
                        .entry(icon_path.clone())
                        .or_insert_with(|| g.load_image(&icon_path))
                        .clone();
                    if let Some(icon) = icon {
                        let icon_y = y + top + (row_h - Self::ICON_SIZE) / 2;
                        g.draw_image(&icon, icon_x, icon_y, Self::ICON_SIZE, Self::ICON_SIZE);
                    }
                }
                // Item text — shifted right to make room for the icon slot
                let text_x = icon_x + Self::ICON_SLOT;
                let text = item.get_text();
                let bounds = g.measure_string(&text, &fs);
                let ttop = top + (row_h / 2) - (bounds.height / 2);
                let item_color = if item.is_selected() { SPColor::new(95, 132, 180, 255) } else { color };
                g.draw_string(&text, &fs, item_color, text_x, y + ttop, width, row_h);
            }
            top += row_h;
        }
    }
}
