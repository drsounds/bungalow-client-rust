//! Port of `SPTableElement`: a track list with sticky headers, zebra rows,
//! selection, double-click, right-click and keyboard navigation.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::impl_element_base;
use crate::libspace::{SPEvent, SPRectangle};
use crate::sp_application_context;
use crate::sp_color::{SPColor, SPGradient};
use crate::sp_element::{construct, ElementRef, ElementRefExt, SPElement, SPElementClass, SP_COLOR_SCHEME_DARK};
use crate::sp_event_args::{downcast_args, SPEventArgs, SPKeyEventArgs};
use crate::sp_font_style::SPFontStyle;
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_object::SPObject;
use crate::sp_playback_event_args::SPTableRowEventArgs;
use crate::sp_table_data_source::SPTableDataSource;

const ROW_H: i32 = 18;
const DBL_CLICK: Duration = Duration::from_millis(300);

pub struct SPTableElement {
    pub el: SPElement,
    data_source: RefCell<Option<Rc<dyn SPTableDataSource>>>,
    /// When true, zebra stripes continue past the last data row to fill
    /// the parent view's remaining height (iTunes-style empty-area fill).
    pub full_view: Cell<bool>,
    pub gradient_view: Cell<bool>,
    selected_rows: RefCell<BTreeSet<i32>>,
    /// last directly-clicked or arrow-moved row
    anchor_row: Cell<i32>,
    /// screen y of first data row (set during paint)
    first_row_y: Cell<i32>,
    last_click_time: Cell<Option<Instant>>,
    last_click_row: Cell<i32>,
}

impl Default for SPTableElement {
    fn default() -> Self {
        Self {
            el: SPElement::default(),
            data_source: RefCell::new(None),
            full_view: Cell::new(false),
            gradient_view: Cell::new(false),
            selected_rows: RefCell::default(),
            anchor_row: Cell::new(-1),
            first_row_y: Cell::new(0),
            last_click_time: Cell::new(None),
            last_click_row: Cell::new(-1),
        }
    }
}

impl_element_base!(SPTableElement, el);

impl SPTableElement {
    pub fn new(parent: Option<&ElementRef>, data_source: Option<Rc<dyn SPTableDataSource>>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("table");
        *rc.data_source.borrow_mut() = data_source;
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

    pub fn data_source(&self) -> Option<Rc<dyn SPTableDataSource>> {
        self.data_source.borrow().clone()
    }

    pub fn set_data_source(&self, data_source: Option<Rc<dyn SPTableDataSource>>) {
        *self.data_source.borrow_mut() = data_source;
        self.selected_rows.borrow_mut().clear();
        self.anchor_row.set(-1);
    }

    pub fn is_row_selected(&self, row: i32) -> bool {
        self.selected_rows.borrow().contains(&row)
    }

    pub fn get_selected_rows(&self) -> Vec<i32> {
        self.selected_rows.borrow().iter().copied().collect()
    }

    fn select_only(&self, row: i32) {
        let mut selected = self.selected_rows.borrow_mut();
        selected.clear();
        selected.insert(row);
    }

    fn dispatch_row_event(&self, event: &str, row: i32) {
        let this = self.el.this();
        self.el.dispatch_event(event, &SPTableRowEventArgs::new(row), Some(&this.as_object()));
    }
}

impl SPElementClass for SPTableElement {
    fn get_type(&self) -> String {
        "table".into()
    }

    fn paint(&self, x: i32, y: i32, g: &dyn SPGraphicsContext) {
        let x = x + self.el.get_x();
        let y = y + self.el.get_y();
        self.el.set_absolute_bounds(SPRectangle::new(x, y, self.el.get_width(), self.el.get_height()));
        let y = y + 1;
        let (Some(data_source), Some(skin)) = (self.data_source(), self.el.get_skin()) else {
            return;
        };
        let parent = self.el.get_parent_element();

        let mut width = self.el.get_width();
        if width < 1 {
            width = parent.as_ref().map(|p| p.el().get_width()).unwrap_or(0);
        }
        let mut column_y = y;
        let padding = 15;
        let column_count = data_source.get_column_count();
        let column_width = if column_count > 0 { width / column_count } else { width };
        let fg_color = self.el.get_foreground_color();

        // Draw column headers first (sticky at column_y)
        if column_y < 60 {
            if let Some(parent) = &parent {
                column_y = parent.el().get_absolute_bounds().y + 1;
            }
        }
        // remember for hit-testing in mouse_down
        self.first_row_y.set(column_y + ROW_H);
        skin.draw_table_header(g, self, x, column_y, width, ROW_H);
        let mut left = 0;
        for i in 0..column_count {
            let text = data_source.get_text_for_column(i);
            g.draw_string(
                &text,
                &SPFontStyle::new("Tahoma", 13, 1, true, false),
                fg_color,
                x + padding + left,
                column_y + 1,
                column_width,
                ROW_H,
            );
            g.draw_string(
                &text,
                &SPFontStyle::new("Tahoma", 13, 1, false, false),
                fg_color.lighten(-10.0),
                x + padding + left,
                column_y,
                column_width,
                ROW_H,
            );
            left += column_width;
        }

        // Draw row backgrounds (zebra stripe) then row text
        let row_count = data_source.get_row_count();
        let first_row_y = column_y + ROW_H;
        for i in 0..row_count {
            skin.draw_table_row(g, self, i, x, first_row_y + i * ROW_H, width, ROW_H, i);
        }

        // Full-view mode: extend zebra stripes into the empty area below the last row
        if self.full_view.get() {
            let parent_bottom =
                parent.as_ref().map(|p| p.el().get_absolute_bounds().y + p.el().get_height()).unwrap_or(0);
            let mut empty_y = first_row_y + row_count * ROW_H;
            let mut ghost_row = row_count;
            if self.gradient_view.get() {
                // Let the skin fill the empty area; otherwise draw a gradient instead of zebra stripes
                let empty = SPRectangle::new(x, empty_y, width, parent_bottom - empty_y);
                if !skin.draw_element(g, &self.el.this(), "", &empty) {
                    let bg = self.el.get_background_color();
                    let gradient = SPGradient::new(bg.lighten(0.5), bg.lighten(0.0), 0);
                    g.fill_rounded_linear_gradient_rectangle(
                        x,
                        empty_y,
                        width,
                        parent_bottom - empty_y,
                        0,
                        0,
                        &gradient,
                    );
                }
            } else {
                while empty_y < parent_bottom {
                    skin.draw_table_row(g, self, ghost_row, x, empty_y, width, ROW_H, ghost_row);
                    empty_y += ROW_H;
                    ghost_row += 1;
                }
            }
        }

        let mut row_y = first_row_y;
        let sel_fg_color = SPColor::new(95, 132, 180, 255);
        let unplayable_dark = SPColor::new(163, 100, 105, 255);
        let unplayable_light = SPColor::new(105, 0, 46, 255);
        let is_dark = self.el.get_color_scheme() == SP_COLOR_SCHEME_DARK;
        let app = sp_application_context::current();
        let font = SPFontStyle::new("Tahoma", 13, 1, false, false);
        for i in 0..row_count {
            let mut row_fg = fg_color;
            if !data_source.is_row_playable(i) {
                row_fg = if is_dark { unplayable_dark } else { unplayable_light };
            }
            if let Some(app) = &app {
                if app.has_current_playable() && data_source.get_row_uri(i) == app.current_playable_uri() {
                    row_fg = SPColor::new(50, 255, 50, 255);
                }
            }
            if self.is_row_selected(i) {
                row_fg = sel_fg_color;
            }
            let progress = data_source.get_progress_for_row(i);
            for j in 0..column_count {
                // Last column: draw a progress bar when progress >= 0
                if progress >= 0.0 && j == column_count - 1 {
                    let bar_x = x + j * column_width + padding;
                    let bar_w = column_width - padding * 2;
                    let bar_y = row_y + 4;
                    let bar_h = ROW_H - 8;
                    if !self.gradient_view.get() {
                        g.fill_rectangle(bar_x, bar_y, bar_w, bar_h, SPColor::new(80, 80, 80, 120));
                    }
                    let filled = ((bar_w as f32 * progress) as i32).max(0);
                    if filled > 0 {
                        g.fill_rectangle(bar_x, bar_y, filled, bar_h, SPColor::new(60, 120, 220, 220));
                    }
                } else {
                    let text = data_source.get_text_for_cell(j, i);
                    let left = x + j * column_width + padding;
                    g.draw_string(&text, &font, row_fg, left, row_y + 1, column_width, ROW_H);
                }
            }
            row_y += ROW_H;
        }
    }

    /// Mouse click → row selection + double-click / right-click detection.
    fn mouse_down(&self, _sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let Some(me) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        let Some(data_source) = self.data_source() else {
            return;
        };
        let rel_y = me.get_y() - self.first_row_y.get();
        if rel_y < 0 {
            return;
        }
        let row = rel_y / ROW_H;
        if row >= data_source.get_row_count() {
            return;
        }

        // Right-click → select row + fire "rowrightclicked"
        if me.get_button() == 1 {
            self.select_only(row);
            self.anchor_row.set(row);
            self.el.invalidate();
            self.dispatch_row_event("rowrightclicked", row);
            return;
        }

        // Double-click detection
        let now = Instant::now();
        let is_double = row == self.last_click_row.get()
            && self.last_click_time.get().is_some_and(|t| now.duration_since(t) < DBL_CLICK);
        if is_double {
            self.dispatch_row_event("rowdblclicked", row);
            self.last_click_row.set(-1);
            self.last_click_time.set(None);
        } else {
            self.last_click_row.set(row);
            self.last_click_time.set(Some(now));
        }

        // Selection update
        if me.get_cmd_down() {
            let mut selected = self.selected_rows.borrow_mut();
            if !selected.remove(&row) {
                selected.insert(row);
            }
        } else {
            self.select_only(row);
        }
        self.anchor_row.set(row);
        self.el.invalidate();
    }

    /// Keyboard navigation.
    fn process_event(&self, event: &SPEvent) {
        if event.type_ == "keydown" {
            let (Some(ke), Some(data_source)) =
                (downcast_args::<SPKeyEventArgs>(event.args.as_ref()), self.data_source())
            else {
                return;
            };
            let row_count = data_source.get_row_count();
            if row_count == 0 {
                return;
            }
            let anchor = self.anchor_row.get();
            let next = match ke.get_key_code() {
                SPKeyEventArgs::KEY_UP => (anchor - 1).max(0),
                SPKeyEventArgs::KEY_DOWN => (anchor + 1).min(row_count - 1),
                _ => return,
            };
            self.select_only(next);
            self.anchor_row.set(next);
            self.el.invalidate();
            return;
        }
        self.el.process_event_default(event);
    }
}
