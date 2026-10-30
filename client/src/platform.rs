//! Port of the Win32 `PlatformUi` dialogs (windows/PlatformUi.cpp): native
//! file/folder pickers and message boxes through `rfd`, and popup menus
//! drawn with egui at the pointer.
//!
//! `TrackPopupMenu` runs a nested modal loop; egui cannot, so a requested
//! menu is parked in [`PopupMenus`] and drawn by the frame loop, and the
//! choice is delivered to the caller's callback.

use std::cell::RefCell;
use std::rc::Rc;

use bungalow::platform_ui::{MenuCallback, MenuItems, PlatformHooks};

struct OpenMenu {
    items: MenuItems,
    on_select: Option<MenuCallback>,
    /// Where the menu opens, in points; filled in on the first frame.
    position: Option<egui::Pos2>,
}

/// Popup menus waiting to be shown or picked from.
#[derive(Default)]
pub struct PopupMenus {
    open: RefCell<Option<OpenMenu>>,
}

impl PopupMenus {
    pub fn show(&self, items: &MenuItems, on_select: MenuCallback) {
        // Opening a menu dismisses the previous one.
        self.close(-1);
        *self.open.borrow_mut() = Some(OpenMenu { items: items.clone(), on_select: Some(on_select), position: None });
    }

    pub fn is_open(&self) -> bool {
        self.open.borrow().is_some()
    }

    fn close(&self, selection: i32) {
        let callback = self.open.borrow_mut().take().and_then(|mut m| m.on_select.take());
        if let Some(callback) = callback {
            callback(selection);
        }
    }

    /// Draws the open menu, if any; a click elsewhere or Escape dismisses it.
    pub fn ui(&self, ctx: &egui::Context) {
        let (items, position) = {
            let mut open = self.open.borrow_mut();
            let Some(menu) = open.as_mut() else { return };
            let position = *menu
                .position
                .get_or_insert_with(|| ctx.input(|i| i.pointer.latest_pos()).unwrap_or(egui::Pos2::new(40.0, 40.0)));
            (menu.items.clone(), position)
        };
        let mut chosen = None;
        let area = egui::Area::new(egui::Id::new("sp-popup-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(140.0);
                    for (id, label) in &items {
                        if ui.add(egui::Button::new(label.as_str()).frame(false)).clicked() {
                            chosen = Some(*id);
                        }
                    }
                });
            });
        if let Some(id) = chosen {
            self.close(id);
            return;
        }
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p));
        if clicked_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close(-1);
        }
    }
}

pub struct ClientPlatformHooks {
    pub menus: Rc<PopupMenus>,
}

impl PlatformHooks for ClientPlatformHooks {
    fn show_popup_menu(&self, items: &MenuItems, on_select: MenuCallback) {
        self.menus.show(items, on_select);
    }

    fn show_folder_dialog(&self, title: &str, initial_path: &str) -> String {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if !initial_path.is_empty() && std::path::Path::new(initial_path).is_dir() {
            dialog = dialog.set_directory(initial_path);
        }
        dialog.pick_folder().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    }

    fn show_open_audio_file_dialog(&self, title: &str) -> String {
        rfd::FileDialog::new()
            .set_title(title)
            .add_filter("Audio files", &["mp3", "wav", "flac", "ogg", "m4a", "aac", "wma", "aiff", "opus"])
            .add_filter("All files", &["*"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn show_message(&self, title: &str, message: &str) {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(rfd::MessageLevel::Info)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }

    fn ask_yes_no(&self, title: &str, message: &str) -> bool {
        let result = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        result == rfd::MessageDialogResult::Yes
    }
}

/// `ShellExecuteW(..., L"open", url, ...)`: opens `url` in the default browser.
pub fn open_in_system_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = result {
        log::warn!("Cannot open {url}: {e}");
    }
}
