//! Ports of the views in `bungalow/` (namespace `space::views`).

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

/// The `SPElementClass` overrides every `SPTabBarViewElement` subclass
/// inherits; `$tab_bar_view` is the path to the embedded tab bar view.
macro_rules! tab_bar_view_overrides {
    ($($tab_bar_view:ident).+) => {
        view_element_overrides!();
        fn mouse_down(&self, sender: Option<&space::sp_object::SPObject>, e: &dyn space::sp_event_args::SPEventArgs) {
            self.element().mouse_down_default(sender, e);
        }
        fn get_view_stack_element(&self) -> Option<space::sp_element::ElementRef> {
            Some(self.$($tab_bar_view).+.tab_stack())
        }
        fn get_slot_element(&self) -> space::sp_element::ElementRef {
            self.$($tab_bar_view).+.tab_stack()
        }
        fn clear(&self) {
            self.$($tab_bar_view).+.clear_tab_bar_view();
        }
    };
}


pub mod artist_over_view;
pub mod artist_view;
pub mod config_view;
pub mod doctrine_view;
pub mod downloads_view;
pub mod space_view;
pub mod start_feature;
pub mod start_view;
pub mod top_list_view;
pub mod user_view;
pub mod whats_new_view;
