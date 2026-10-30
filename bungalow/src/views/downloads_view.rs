//! Port of `DownloadsView`: ongoing and completed downloads in two tabs.

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

use crate::download_manager::DownloadManager;
use crate::downloads_data_source::{DownloadsDataSource, Filter};

#[derive(Default)]
pub struct DownloadsView {
    pub tab_bar_view: SPTabBarViewElement,
    dm: RefCell<Option<Rc<DownloadManager>>>,
    ongoing_table: RefCell<Option<Rc<SPTableElement>>>,
    completed_table: RefCell<Option<Rc<SPTableElement>>>,
    ongoing_ds: RefCell<Option<Rc<DownloadsDataSource>>>,
    completed_ds: RefCell<Option<Rc<DownloadsDataSource>>>,
}

impl_element_base!(DownloadsView, tab_bar_view);

impl DownloadsView {
    pub fn new(parent: Option<&ElementRef>, dm: Option<Rc<DownloadManager>>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        *rc.dm.borrow_mut() = dm.clone();
        rc.tab_bar_view.init_tab_bar_view();
        let tab_stack: ElementRef = rc.tab_bar_view.tab_stack();
        let weak = Rc::downgrade(&rc);

        let make_tab = |id: &str, title: &str, filter: Filter| {
            let section = SPSectionElement::new(Some(&tab_stack));
            section.element().this().set_id(id);
            section.element().set_string_attribute("title", title);
            section.element().set_flex(1);
            let section_ref: ElementRef = section.clone();

            let ds = Rc::new(DownloadsDataSource::new(dm.clone(), filter));
            let table = SPTableElement::new(Some(&section_ref), Some(ds.clone()));
            table.full_view.set(true);
            table.element().set_flex(1);
            let view = weak.clone();
            table.element().add_event_listener(
                "rowdblclicked",
                Rc::new(move |_sender, e| {
                    if let (Some(view), Some(args)) = (view.upgrade(), downcast_args::<SPTableRowEventArgs>(e)) {
                        if filter == Filter::Ongoing {
                            view.on_ongoing_row_dbl_clicked(args.get_row());
                        }
                        // Completed rows could open the file — left for a future enhancement.
                    }
                    0
                }),
            );
            section_ref.append_child(&(table.clone() as ElementRef));
            tab_stack.append_child(&section_ref);
            (ds, table)
        };

        let (ongoing_ds, ongoing_table) = make_tab("overview", "Ongoing", Filter::Ongoing);
        let (completed_ds, completed_table) = make_tab("completed", "Completed", Filter::Completed);
        *rc.ongoing_ds.borrow_mut() = Some(ongoing_ds);
        *rc.ongoing_table.borrow_mut() = Some(ongoing_table);
        *rc.completed_ds.borrow_mut() = Some(completed_ds);
        *rc.completed_table.borrow_mut() = Some(completed_table);

        let tab_bar = rc.tab_bar_view.tab_bar();
        tab_bar.add_tab("overview", "Ongoing", "bungalow:downloads");
        tab_bar.add_tab("completed", "Completed", "bungalow:downloads#completed");
        rc.tab_bar_view.tab_stack().set_tab_id("overview");

        if let Some(dm) = &dm {
            let view = weak.clone();
            dm.set_update_callback(Some(Rc::new(move || {
                if let Some(view) = view.upgrade() {
                    view.refresh();
                }
            })));
        }
        rc
    }

    /// Refreshes both tables (called by the DownloadManager's update callback).
    pub fn refresh(&self) {
        for (ds, table) in [(&self.ongoing_ds, &self.ongoing_table), (&self.completed_ds, &self.completed_table)] {
            if let (Some(ds), Some(table)) = (ds.borrow().as_ref(), table.borrow().as_ref()) {
                ds.refresh();
                table.element().invalidate();
            }
        }
    }

    /// Double-clicking an ongoing row cancels the download.
    fn on_ongoing_row_dbl_clicked(&self, row: i32) {
        let Some(ds) = self.ongoing_ds.borrow().clone() else { return };
        let item_id = ds.get_item_id_for_row(row);
        if item_id <= 0 {
            return;
        }
        if let Some(dm) = self.dm.borrow().as_ref() {
            dm.cancel_download(item_id);
        }
    }
}

impl Drop for DownloadsView {
    fn drop(&mut self) {
        // Remove our callback so the DownloadManager doesn't call into a dead view.
        if let Some(dm) = self.dm.borrow().as_ref() {
            dm.set_update_callback(None);
        }
    }
}

impl SPViewClass for DownloadsView {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri == "bungalow:downloads" || uri.starts_with("bungalow:downloads#")
    }
    fn navigate(&self, _uri: &str) {}
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(if hash.is_empty() { "ongoing" } else { hash });
    }
}

impl SPElementClass for DownloadsView {
    fn get_type(&self) -> String {
        "tabbarview".into()
    }
    tab_bar_view_overrides!(tab_bar_view);
}
