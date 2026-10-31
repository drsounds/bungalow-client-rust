//! Port of `NativeApplication` (windows/): owns the Space, its window
//! element and graphics context, and the BungalowApp, and turns window
//! input into Space events. The window frame itself is [`crate::MainFrame`].

use std::rc::Rc;
use std::time::Duration;

use bungalow::ui_dispatcher;

use space::libspace::{SPEvent, SPRectangle};
use space::sp_classic_skin::SPClassicSkin;
use space::sp_element::{ElementRef, SP_COLOR_SCHEME_DARK};
use space::sp_event_args::SPKeyEventArgs;
use space::sp_graphics_context::SPGraphicsContext;
use space::sp_mouse_event_args::SPMouseEventArgs;
use space::sp_object::SPObject;
use space::sp_window_element::SPWindowElement;
use space::space::Space;

use crate::bungalow_app::BungalowApp;
use crate::egui_graphics_context::EguiGraphicsContext;
use crate::views::sp_player_canvas_element::SPPlayerCanvasElement;

/// Windows virtual-key codes for the keys the Space elements look at.
pub mod vk {
    pub const BACK: i32 = 0x08;
    pub const TAB: i32 = 0x09;
    pub const RETURN: i32 = 0x0D;
    pub const ESCAPE: i32 = 0x1B;
    pub const SPACE: i32 = 0x20;
    pub const PRIOR: i32 = 0x21;
    pub const NEXT: i32 = 0x22;
    pub const END: i32 = 0x23;
    pub const HOME: i32 = 0x24;
    pub const LEFT: i32 = 0x25;
    pub const RIGHT: i32 = 0x27;
    pub const DELETE: i32 = 0x2E;
}

#[derive(Default)]
pub struct NativeApplication {
    space: Option<Rc<Space>>,
    window_element: Option<Rc<SPWindowElement>>,
    main_element: Option<ElementRef>,
    graphics: Option<Rc<EguiGraphicsContext>>,
    app: Option<Rc<BungalowApp>>,
    left_button_down: bool,
    right_button_down: bool,
}

impl NativeApplication {
    /// The caption (menus, title, window buttons) overlays the top of the
    /// app header, which reserves this much top padding for it.
    pub const CAPTION_HEIGHT: i32 = 25;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(&mut self, ctx: &egui::Context, width: i32, height: i32) -> Result<(), String> {
        let waker_ctx = ctx.clone();
        ui_dispatcher::init_ui_dispatcher(Some(Box::new(move || waker_ctx.request_repaint())));

        let space = Space::new();
        let graphics = Rc::new(EguiGraphicsContext::new(ctx.clone()));
        graphics.set_size(width, height);
        space.set_graphics_context(Some(graphics.clone() as Rc<dyn SPGraphicsContext>));

        let window_element = SPWindowElement::new(&space, Some(Rc::new(SPClassicSkin::new())));
        window_element.el.set_color_scheme(SP_COLOR_SCHEME_DARK);
        space.add_window(window_element.clone());

        self.space = Some(space.clone());
        self.graphics = Some(graphics);
        self.window_element = Some(window_element.clone());

        #[cfg(feature = "wef")]
        crate::wefhost::register_backend(ctx);

        let app = BungalowApp::new();
        self.app = Some(app.clone());
        let main = app.initialize(&space, &window_element)?;
        self.main_element = Some(main as ElementRef);
        self.resize(width, height);
        Ok(())
    }

    pub fn app(&self) -> Option<&Rc<BungalowApp>> {
        self.app.as_ref()
    }

    /// Perceived brightness (0-255) of the header behind the caption, so the
    /// caption text can pick a readable color. The C++ caption is drawn by
    /// the OS and has no equivalent.
    pub fn header_luminance(&self) -> Option<i32> {
        let main = self.main_element.as_ref()?;
        let el = main.as_ref().element();
        let color = el.get_background_color();
        if color.get_a() == 0 {
            return Some(if el.get_color_scheme() == SP_COLOR_SCHEME_DARK { 30 } else { 230 });
        }
        let (r, g, b) = (color.get_r() as i32, color.get_g() as i32, color.get_b() as i32);
        Some((r * 299 + g * 587 + b * 114) / 1000)
    }

    pub fn resize(&self, width: i32, height: i32) {
        let (Some(window), Some(main), Some(graphics)) = (&self.window_element, &self.main_element, &self.graphics)
        else {
            return;
        };
        graphics.set_size(width, height);
        window.el.set_absolute_bounds(SPRectangle::new(0, 0, width, height));
        let m = main.as_ref().element();
        m.set_x(0);
        m.set_width(width.max(0));
        m.set_y(0);
        m.set_height(height.max(0));
        main.pack();
    }

    pub fn paint(&self, ui: &mut egui::Ui) {
        let (Some(window), Some(graphics)) = (&self.window_element, &self.graphics) else { return };
        graphics.begin_paint(ui.painter().clone());
        window.draw(graphics.as_ref(), graphics.get_width(), graphics.get_height());
        graphics.end_paint(ui);
    }

    pub fn mouse_event(&mut self, name: &str, x: i32, y: i32, button: i32, command_down: bool) {
        let (Some(space), Some(window)) = (&self.space, &self.window_element) else { return };
        let sender = SPObject::Element(window.clone() as ElementRef);
        let args = SPMouseEventArgs::with_cmd(button, x, y, command_down);
        space.notify(name, Some(&sender), &args);
        let released = (button == 0 && self.left_button_down) || (button == 1 && self.right_button_down);
        if name == "mouseup" && released {
            let click = SPMouseEventArgs::with_cmd(button, x, y, command_down);
            space.notify("click", Some(&sender), &click);
            if button == 0 {
                self.left_button_down = false;
            } else {
                self.right_button_down = false;
            }
        } else if name == "mousedown" && button == 0 {
            self.left_button_down = true;
        } else if name == "mousedown" && button == 1 {
            self.right_button_down = true;
        }
    }

    /// Scrolls the views under the pointer (positive `dy` scrolls down).
    pub fn scroll_event(&self, dx: i32, dy: i32, x: i32, y: i32) {
        let Some(window) = &self.window_element else { return };
        window.el.scroll_default(dx, dy, x, y);
        if let Some(graphics) = &self.graphics {
            graphics.invalidate_region(SPRectangle::new(0, 0, graphics.get_width(), graphics.get_height()));
        }
    }

    pub fn key_event(&self, key_code: i32, command_down: bool) {
        let (Some(space), Some(window)) = (&self.space, &self.window_element) else { return };
        let args = SPKeyEventArgs::new(key_code, command_down);
        let sender = SPObject::Element(window.clone() as ElementRef);
        space.notify("keydown", Some(&sender), &args);
        space.process_event(&SPEvent::new("keydown", Rc::new(args), None));
    }

    /// The frame loop's timers: work posted to the UI thread (download
    /// progress, favicons, resolved entities), the playback position timer,
    /// the player canvas audio timer and the frames of Swimming apps being
    /// shown. Returns how long until a timer is due again.
    pub fn timer(&self) -> Option<Duration> {
        ui_dispatcher::run_pending();
        let position = self.app.as_ref().and_then(|a| a.tick());
        let canvas = SPPlayerCanvasElement::tick_all();
        #[cfg(feature = "swimming")]
        let swimming = crate::views::sp_swimming_app_element::SPSwimmingAppElement::tick_all();
        #[cfg(not(feature = "swimming"))]
        let swimming = None;
        [position, canvas, swimming].into_iter().flatten().min()
    }

    /// Develop menu: act on the visible web page or app.
    pub fn show_web_inspector(&self) {
        if let Some(app) = &self.app {
            app.show_web_inspector();
        }
    }

    pub fn reload_web_page(&self) {
        if let Some(app) = &self.app {
            app.reload_web_page();
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(app) = self.app.take() {
            app.shutdown();
        }
    }
}
