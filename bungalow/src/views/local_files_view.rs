//! Port of `LocalFilesView` (bungalow:localfiles): every audio file in a five
//! column table (Name | Version | Artists | Album | Duration).

use std::cell::RefCell;
use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_event_args::downcast_args;
use space::sp_playback_event_args::SPTableRowEventArgs;
use space::sp_section_element::SPSectionElement;
use space::sp_tab_bar_view_element::SPTabBarViewElement;
use space::sp_table_element::SPTableElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

use crate::bungalow_app_handle;
use crate::database::Database;
use crate::local_files_data_source::LocalFilesDataSource;
use crate::play_context::PlayContext;

#[derive(Default)]
pub struct LocalFilesView {
    pub tab_bar_view: SPTabBarViewElement,
    table: RefCell<Option<Rc<SPTableElement>>>,
    data_source: RefCell<Option<Rc<LocalFilesDataSource>>>,
}

impl_element_base!(LocalFilesView, tab_bar_view);

impl LocalFilesView {
    pub fn new(parent: Option<&ElementRef>, db: Option<Rc<Database>>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.tab_bar_view.init_tab_bar_view();
        let tab_stack: ElementRef = rc.tab_bar_view.tab_stack();

        let section = SPSectionElement::new(Some(&tab_stack));
        section.element().this().set_id("overview");
        section.element().set_string_attribute("title", "Local Files");
        section.element().set_flex(1);
        let section_ref: ElementRef = section;

        let data_source = Rc::new(LocalFilesDataSource::new(db.as_deref().map(Database::handle)));
        let table = SPTableElement::new(Some(&section_ref), Some(data_source.clone()));
        table.full_view.set(true);
        table.element().set_flex(1);
        let view = Rc::downgrade(&rc);
        table.element().add_event_listener(
            "rowdblclicked",
            Rc::new(move |_, e| {
                if let (Some(view), Some(args)) = (view.upgrade(), downcast_args::<SPTableRowEventArgs>(e)) {
                    view.on_table_double_click(args.get_row());
                }
                0
            }),
        );
        section_ref.append_child(&(table.clone() as ElementRef));
        tab_stack.append_child(&section_ref);
        rc.tab_bar_view.tab_bar().add_tab("overview", "Local Files", "bungalow:localfiles");
        rc.tab_bar_view.tab_stack().set_tab_id("overview");

        *rc.table.borrow_mut() = Some(table);
        *rc.data_source.borrow_mut() = Some(data_source);
        rc
    }

    fn on_table_double_click(&self, row: i32) {
        let Some(ds) = self.data_source.borrow().clone() else { return };
        let Some(app) = bungalow_app_handle::current() else { return };
        app.play_context(PlayContext { entries: ds.build_queue_entries(), start_index: row.max(0) as usize });
    }
}

impl SPViewClass for LocalFilesView {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri == "bungalow:localfiles"
    }
    fn navigate(&self, _uri: &str) {}
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(if hash.is_empty() { "overview" } else { hash });
    }
}

impl SPElementClass for LocalFilesView {
    fn get_type(&self) -> String {
        "localfilesview".into()
    }
    tab_bar_view_overrides!(tab_bar_view);
}
