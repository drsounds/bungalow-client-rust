//! Ports of the views that need the client: the web-backed site views
//! (wxwidgets/app and windows/ `SiteView`, `AppView`, `SPAppView`,
//! bungalow/ `CuratorView`, `StoreView`) and the Now Playing view with its
//! canvas elements.

/// The `SPElementClass` overrides every `SPViewElement` subclass inherits.
macro_rules! view_element_overrides {
    () => {
        fn pack(&self) {
            space::sp_view_element::SPViewElement::pack_view(self.element());
        }
        fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
            space::sp_view_element::SPViewElement::scroll_view(self.element(), scroll_x, scroll_y);
        }
        fn as_view(&self) -> Option<&dyn space::sp_view_element::SPViewClass> {
            Some(self)
        }
    };
}

/// The `SPElementClass` and `SPViewClass` glue of a `SiteView` subclass whose
/// embedded `SiteView` is the field `site`.
macro_rules! site_view_subclass {
    ($type:ty, $tag:expr) => {
        impl space::sp_element::SPElementClass for $type {
            fn get_type(&self) -> String {
                $tag.into()
            }
            view_element_overrides!();
            fn set_visible(&self, value: bool) {
                self.site.set_visible_site(value);
            }
            fn paint(&self, x: i32, y: i32, gc: &dyn space::sp_graphics_context::SPGraphicsContext) {
                self.site.paint_site(x, y, gc);
            }
        }

        impl space::sp_view_element::SPViewClass for $type {
            fn view(&self) -> &space::sp_view_element::SPViewElement {
                &self.site.view
            }
            fn accepts_uri(&self, uri: &str) -> bool {
                $crate::views::site_view::SiteViewClass::accepts_site_uri(self, uri)
            }
            fn navigate(&self, uri: &str) {
                self.site.navigate_site(uri);
            }
            fn navigate_back(&self, uri: &str) {
                self.site.navigate_back_site(uri);
            }
            fn navigate_forward(&self, uri: &str) {
                self.site.navigate_back_site(uri);
            }
        }
    };
}

pub mod app_view;
pub mod curator_view;
pub mod player_view;
pub mod site_view;
pub mod sp_app_view;
pub mod sp_music_visualizer_canvas_element;
pub mod sp_music_visualizer_element;
pub mod sp_player_canvas_element;
pub mod store_view;
#[cfg(feature = "swimming")]
pub mod sp_swimming_app_element;
