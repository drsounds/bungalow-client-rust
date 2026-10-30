//! Port of `LibraryView` (bungalow:library): the saved tracks in a three
//! column table (Name | Artists | Release). Double-clicking a row plays the
//! library from that row.

use std::cell::RefCell;
use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, ElementRefExt, SPElementBase, SPElementClass};
use space::sp_event_args::downcast_args;
use space::sp_playback_event_args::SPTableRowEventArgs;
use space::sp_section_element::SPSectionElement;
use space::sp_tab_bar_element::TabBarToolButtonEventArgs;
use space::sp_tab_bar_view_element::SPTabBarViewElement;
use space::sp_table_element::SPTableElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

use crate::bungalow_app_handle;
use crate::database::Database;
use crate::library_data_source::LibraryDataSource;
use crate::platform_ui;
use crate::play_context::PlayContext;

const ID_PLAY: i32 = 10100;
const ID_EDIT: i32 = 10101;

#[derive(Default)]
pub struct LibraryView {
    pub tab_bar_view: SPTabBarViewElement,
    table: RefCell<Option<Rc<SPTableElement>>>,
    data_source: RefCell<Option<Rc<LibraryDataSource>>>,
    db: RefCell<Option<Rc<Database>>>,
}

impl_element_base!(LibraryView, tab_bar_view);

impl LibraryView {
    pub fn new(parent: Option<&ElementRef>, db: Option<Rc<Database>>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.tab_bar_view.init_tab_bar_view();
        let weak = Rc::downgrade(&rc);
        let tab_stack: ElementRef = rc.tab_bar_view.tab_stack();

        let section = SPSectionElement::new(Some(&tab_stack));
        section.element().this().set_id("overview");
        section.element().set_string_attribute("title", "Library");
        section.element().set_flex(1);
        let section_ref: ElementRef = section;

        let data_source = Rc::new(LibraryDataSource::new(db.as_deref().map(Database::handle)));
        let table = SPTableElement::new(Some(&section_ref), Some(data_source.clone()));
        table.full_view.set(true);
        section_ref.append_child(&(table.clone() as ElementRef));
        table.element().set_flex(1);

        let view = weak.clone();
        table.element().add_event_listener(
            "rowdblclicked",
            Rc::new(move |_, e| {
                if let (Some(view), Some(args)) = (view.upgrade(), downcast_args::<SPTableRowEventArgs>(e)) {
                    view.play_row_in_context(args.get_row());
                }
                0
            }),
        );
        let view = weak.clone();
        table.element().add_event_listener(
            "rowrightclicked",
            Rc::new(move |_, e| {
                if let (Some(view), Some(args)) = (view.upgrade(), downcast_args::<SPTableRowEventArgs>(e)) {
                    view.on_row_right_clicked(args.get_row());
                }
                0
            }),
        );
        tab_stack.append_child(&section_ref);

        let tab_bar = rc.tab_bar_view.tab_bar();
        tab_bar.add_tab("overview", "Library", "bungalow:library");
        rc.tab_bar_view.tab_stack().set_tab_id("overview");

        // + tool button on the right of the tab bar
        tab_bar.add_tool_button("add", "+", None);
        let view = weak;
        tab_bar.element().add_event_listener(
            "toolbuttonclicked",
            Rc::new(move |_, e| {
                if let (Some(view), Some(args)) = (view.upgrade(), downcast_args::<TabBarToolButtonEventArgs>(e)) {
                    if args.button_id == "add" {
                        view.edit_track(0);
                    }
                }
                0
            }),
        );

        *rc.table.borrow_mut() = Some(table);
        *rc.data_source.borrow_mut() = Some(data_source);
        *rc.db.borrow_mut() = db;
        rc
    }

    fn reload(&self) {
        let db = self.db.borrow().clone();
        if let (Some(ds), Some(db)) = (self.data_source.borrow().as_ref(), db) {
            ds.reload(Some(db.handle()));
            if let Some(table) = self.table.borrow().as_ref() {
                table.element().invalidate();
            }
        }
    }

    fn play_row_in_context(&self, row: i32) {
        let Some(app) = bungalow_app_handle::current() else { return };
        let db = self.db.borrow().clone();
        let (Some(ds), Some(db)) = (self.data_source.borrow().clone(), db) else { return };
        app.play_context(PlayContext {
            entries: ds.build_queue_entries(Some(db.handle())),
            start_index: row.max(0) as usize,
        });
    }

    /// Shows the add (`track_id` == 0) or edit dialog and reloads on save.
    fn edit_track(&self, track_id: i32) {
        let db = self.db.borrow().clone();
        let Some(db) = db else { return };
        if platform_ui::show_add_track_dialog(Some(db.handle()), track_id) {
            self.reload();
        }
    }

    fn on_row_right_clicked(&self, row: i32) {
        let Some(ds) = self.data_source.borrow().clone() else { return };
        use space::sp_table_data_source::SPTableDataSource;
        let track_id = ds.get_track_id_for_row(row);
        if track_id <= 0 {
            return;
        }
        let view = Rc::downgrade(&self.element().this());
        let items = vec![(ID_PLAY, "Play".into()), (ID_EDIT, "Edit...".into())];
        platform_ui::show_popup_menu(
            &items,
            Box::new(move |selection| {
                let Some(view) = view.upgrade() else { return };
                let Some(view) = view.downcast::<LibraryView>() else { return };
                match selection {
                    ID_PLAY => view.play_row_in_context(row),
                    ID_EDIT => view.edit_track(track_id),
                    _ => {}
                }
            }),
        );
    }
}

impl SPViewClass for LibraryView {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri == "bungalow:library"
    }
    fn navigate(&self, _uri: &str) {
        self.reload();
    }
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(if hash.is_empty() { "overview" } else { hash });
    }
}

impl SPElementClass for LibraryView {
    fn get_type(&self) -> String {
        "libraryview".into()
    }
    tab_bar_view_overrides!(tab_bar_view);
}
