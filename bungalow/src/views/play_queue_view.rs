//! Port of `PlayQueueView`: the right sidebar panel showing the play queue.
//! It listens to "queue:changed" and "playback:changed" on the application's
//! playback events, so it stays in sync by itself. Double-clicking a row
//! plays that entry.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use space::impl_element_base;
use space::sp_application_context;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass, SP_COLOR_SCHEME_DARK};
use space::sp_event_args::downcast_args;
use space::sp_playback_event_args::SPTableRowEventArgs;
use space::sp_section_element::SPSectionElement;
use space::sp_tab_bar_view_element::SPTabBarViewElement;
use space::sp_table_element::SPTableElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

use crate::bungalow_app_handle;
use crate::play_queue_data_source::PlayQueueDataSource;

#[derive(Default)]
pub struct PlayQueueView {
    pub tab_bar_view: SPTabBarViewElement,
    table: RefCell<Option<Rc<SPTableElement>>>,
    data_source: RefCell<Option<Rc<PlayQueueDataSource>>>,
}

impl_element_base!(PlayQueueView, tab_bar_view);

impl PlayQueueView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.tab_bar_view.init_tab_bar_view();
        let el = rc.element();
        // Override the default flex=1 of the tab bar view so the panel keeps
        // a fixed width as a sidebar column.
        el.set_flex(0);
        el.set_width(240);
        el.set_color_scheme(SP_COLOR_SCHEME_DARK);
        let tab_stack: ElementRef = rc.tab_bar_view.tab_stack();

        let section = SPSectionElement::new(Some(&tab_stack));
        section.element().this().set_id("overview");
        section.element().set_string_attribute("title", "Play queue");
        section.element().set_flex(1);
        let section_ref: ElementRef = section;

        let data_source = Rc::new(PlayQueueDataSource::new());
        let table = SPTableElement::new(Some(&section_ref), Some(data_source.clone()));
        table.full_view.set(true);
        table.gradient_view.set(true);
        table.element().set_flex(1);
        section_ref.append_child(&(table.clone() as ElementRef));

        // Double-click a row → play that entry from the existing queue.
        table.element().add_event_listener(
            "rowdblclicked",
            Rc::new(|_, e| {
                if let (Some(args), Some(app)) =
                    (downcast_args::<SPTableRowEventArgs>(e), bungalow_app_handle::current())
                {
                    app.play_queue_at_index(args.get_row());
                }
                0
            }),
        );
        tab_stack.append_child(&section_ref);
        rc.tab_bar_view.tab_bar().add_tab("overview", "Play queue", "bungalow:internal:playqueue");
        rc.tab_bar_view.tab_stack().set_tab_id("overview");

        *rc.table.borrow_mut() = Some(table);
        *rc.data_source.borrow_mut() = Some(data_source);

        // Subscribe to queue and playback events so the panel stays in sync.
        if let Some(context) = sp_application_context::current() {
            for event in ["queue:changed", "playback:changed"] {
                let view: Weak<Self> = Rc::downgrade(&rc);
                context.get_playback_events().add_listener(event, move |_, _| {
                    if let Some(view) = view.upgrade() {
                        view.reload();
                    }
                    0
                });
            }
        }
        // Populate immediately with the current (possibly empty) queue.
        rc.reload();
        rc
    }

    fn reload(&self) {
        let Some(ds) = self.data_source.borrow().clone() else { return };
        if let Some(app) = bungalow_app_handle::current() {
            ds.set_entries(&app.get_queue_entries(), app.get_queue_index());
        }
        if let Some(table) = self.table.borrow().as_ref() {
            table.element().invalidate();
        }
    }
}

impl SPViewClass for PlayQueueView {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.tab_bar_view.accepts_uri_default(uri)
    }
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(hash);
    }
}

impl SPElementClass for PlayQueueView {
    fn get_type(&self) -> String {
        "playqueueview".into()
    }
    tab_bar_view_overrides!(tab_bar_view);
}
