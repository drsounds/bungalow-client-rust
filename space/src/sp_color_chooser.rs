//! Port of `SPColorChooser`: a grid of colour swatches.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use crate::sp_event_args::{downcast_args, SPEventArgs};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_object::SPObject;

const SWATCH_SIZE: i32 = 28;
const SWATCH_GAP: i32 = 4;
const COLS: i32 = 8;

pub const DEFAULT_COLORS: [SPColor; 12] = [
    SPColor::new(220, 60, 60, 255),   // Red
    SPColor::new(230, 120, 40, 255),  // Orange
    SPColor::new(210, 190, 40, 255),  // Yellow
    SPColor::new(60, 180, 60, 255),   // Green
    SPColor::new(40, 180, 200, 255),  // Cyan
    SPColor::new(80, 120, 240, 255),  // Blue
    SPColor::new(140, 70, 220, 255),  // Purple
    SPColor::new(220, 60, 140, 255),  // Pink
    SPColor::new(160, 110, 60, 255),  // Brown
    SPColor::new(100, 160, 140, 255), // Teal
    SPColor::new(200, 200, 200, 255), // Silver
    SPColor::new(60, 60, 60, 255),    // Dark
];

pub struct SPColorChooser {
    pub el: SPElement,
    colors: Vec<SPColor>,
    selected_color: Cell<SPColor>,
    pub on_change: RefCell<Option<Box<dyn Fn(&SPColor)>>>,
}

impl_element_base!(SPColorChooser, el);

impl SPColorChooser {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(
            Self {
                el: SPElement::default(),
                colors: DEFAULT_COLORS.to_vec(),
                // Default: blue
                selected_color: Cell::new(DEFAULT_COLORS[5]),
                on_change: RefCell::new(None),
            },
            parent,
        );
        let rows = (rc.colors.len() as i32 + COLS - 1) / COLS;
        rc.el.set_width(COLS * (SWATCH_SIZE + SWATCH_GAP) - SWATCH_GAP);
        rc.el.set_height(rows * (SWATCH_SIZE + SWATCH_GAP) - SWATCH_GAP);
        rc
    }

    pub fn get_selected_color(&self) -> SPColor {
        self.selected_color.get()
    }

    pub fn set_selected_color(&self, color: SPColor) {
        self.selected_color.set(color);
        self.el.invalidate();
    }

    fn swatch_index_at(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let (col, row) = (x / (SWATCH_SIZE + SWATCH_GAP), y / (SWATCH_SIZE + SWATCH_GAP));
        let (local_x, local_y) = (x % (SWATCH_SIZE + SWATCH_GAP), y % (SWATCH_SIZE + SWATCH_GAP));
        if local_x >= SWATCH_SIZE || local_y >= SWATCH_SIZE || col >= COLS {
            return None;
        }
        let index = (row * COLS + col) as usize;
        (index < self.colors.len()).then_some(index)
    }
}

impl SPElementClass for SPColorChooser {
    fn get_type(&self) -> String {
        "colorchooser".into()
    }

    fn paint(&self, x: i32, y: i32, c: &dyn SPGraphicsContext) {
        let bounds = SPRectangle::new(self.el.get_x() + x, self.el.get_y() + y, self.el.get_width(), self.el.get_height());
        self.el.set_absolute_bounds(bounds);
        let selected = self.selected_color.get().to_html_color();
        for (i, color) in self.colors.iter().enumerate() {
            let (col, row) = (i as i32 % COLS, i as i32 / COLS);
            let sx = bounds.x + col * (SWATCH_SIZE + SWATCH_GAP);
            let sy = bounds.y + row * (SWATCH_SIZE + SWATCH_GAP);
            c.fill_rounded_rectangle(sx, sy, SWATCH_SIZE, SWATCH_SIZE, 4, *color);
            if color.to_html_color() == selected {
                c.draw_rounded_rectangle(sx - 2, sy - 2, SWATCH_SIZE + 4, SWATCH_SIZE + 4, 6, SPColor::new(255, 255, 255, 255));
                c.draw_rounded_rectangle(sx - 1, sy - 1, SWATCH_SIZE + 2, SWATCH_SIZE + 2, 5, SPColor::new(30, 30, 30, 220));
            }
        }
    }

    fn notify(&self, event: &str, sender: Option<&SPObject>, data: &dyn SPEventArgs) -> i32 {
        if event == "click" {
            if let Some(mouse) = downcast_args::<SPMouseEventArgs>(data) {
                let bounds = self.el.get_absolute_bounds();
                if let Some(index) = self.swatch_index_at(mouse.get_x() - bounds.x, mouse.get_y() - bounds.y) {
                    self.set_selected_color(self.colors[index]);
                    if let Some(on_change) = self.on_change.borrow().as_ref() {
                        on_change(&self.colors[index]);
                    }
                }
            }
        }
        self.el.notify_default(event, sender, data)
    }
}
