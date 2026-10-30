//! Port of `MainElement` (wxwidgets/app/, shared with the WTL build): the
//! root element of the window. It loads views/app.xml, fills the navigation
//! tree with sections and bookmarks, and owns the view stack.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use bungalow::bungalow_app_handle;
use bungalow::platform_ui;
use bungalow::views::config_view::ConfigView;
use bungalow::views::downloads_view::DownloadsView;
use bungalow::views::library_view::LibraryView;
use bungalow::views::local_files_view::LocalFilesView;
use bungalow::views::play_queue_view::PlayQueueView;
use bungalow::views::space_view::SpaceView;
use bungalow::views::start_view::StartView;

use space::impl_element_base;
use space::libspace::SPMargin;
use space::sp_app_footer_element::SPAppFooterElement;
use space::sp_app_header_element::SPAppHeaderElement;
use space::sp_bookmark_event_args::SPBookmarkEventArgs;
use space::sp_color::SPColor;
use space::sp_element::{construct, ElementRef, ElementRefExt, SPElementBase, SPElementClass};
use space::sp_event_args::{downcast_args, SPEventArgs};
use space::sp_info_element::SPInfoElement;
use space::sp_input_element::SPInputElement;
use space::sp_input_event_args::SPInputEventArgs;
use space::sp_message_type::SPMessageType;
use space::sp_navigate_event_args::SPNavigateEventArgs;
use space::sp_now_playing_bar_element::SPNowPlayingBarElement;
use space::sp_splitter_element::SPSplitterElement;
use space::sp_tab_bar_element::SPTabBarElement;
use space::sp_tree_item::SPTreeItem;
use space::sp_tree_view_element::{SPTreeViewElement, TreeViewEventArgs};
use space::sp_uri::SPUri;
use space::sp_vbox_element::SPVBoxElement;
use space::sp_view_stack_element::SPViewStackElement;

use crate::favicon_cache::FaviconCache;
use crate::views::app_view::AppView;
use crate::views::curator_view::CuratorView;
use crate::views::player_view::PlayerView;
use crate::views::site_view::SiteView;
use crate::views::store_view::StoreView;

/// The elements `layout()` looks up in app.xml or creates.
#[derive(Default)]
struct Parts {
    search_element: Option<ElementRef>,
    view_stack: Option<ElementRef>,
    tree_view: Option<ElementRef>,
    info_element: Option<Rc<SPInfoElement>>,
    footer: Option<ElementRef>,
    header: Option<ElementRef>,
    body: Option<ElementRef>,
    left_sidebar: Option<ElementRef>,
    tab_bar: Option<ElementRef>,
    back_button: Option<ElementRef>,
    forward_button: Option<ElementRef>,
    now_playing_bar: Option<Rc<SPNowPlayingBarElement>>,
}

#[derive(Default)]
pub struct MainElement {
    pub vbox: SPVBoxElement,
    parts: RefCell<Parts>,
    favicon_cache: RefCell<Option<FaviconCache>>,
}

impl_element_base!(MainElement, vbox);

// ── Helpers ─────────────────────────────────────────────────────────────────

/// space:, bungalow: and musik: are all aliases for the bungalow: scheme.
fn normalize_scheme(uri: &str) -> String {
    for alias in ["space:", "bungalow:", "musik:"] {
        if let Some(rest) = uri.strip_prefix(alias) {
            return format!("bungalow:{rest}");
        }
    }
    uri.to_string()
}

fn has_bungalow_scheme(uri: &str) -> bool {
    uri.starts_with("bungalow:") || uri.starts_with("space:") || uri.starts_with("musik:")
}

/// bungalow:site:example.com:page -> bungalow:site:example.com;
/// empty if the URI is already at the domain level.
fn domain_uri(uri: &str) -> String {
    let Some(first) = uri.find(':') else { return String::new() };
    let Some(second) = uri[first + 1..].find(':').map(|i| i + first + 1) else { return String::new() };
    let Some(third) = uri[second + 1..].find(':').map(|i| i + second + 1) else { return String::new() };
    uri[..third].to_string()
}

fn tree_item(uri: &str, text: &str, icon: &str) -> Rc<SPTreeItem> {
    let item = SPTreeItem::new(SPUri::new(uri));
    item.set_text(text);
    item.set_icon_path(icon);
    item
}

impl MainElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("vbox");
        *rc.favicon_cache.borrow_mut() = Some(FaviconCache::new());
        rc.element().load_xul("views/app.xml");
        rc.layout();
        if let Some(window) = rc.element().get_window_element() {
            let weak = Rc::downgrade(&rc);
            window.element().add_event_listener(
                "navigate",
                Rc::new(move |_, args| {
                    if let (Some(this), Some(args)) = (weak.upgrade(), downcast_args::<SPNavigateEventArgs>(args)) {
                        this.navigate(&args.get_url());
                    }
                    0
                }),
            );
            let weak = Rc::downgrade(&rc);
            window.element().add_event_listener(
                "bookmarktoggled",
                Rc::new(move |_, args| {
                    if let Some(this) = weak.upgrade() {
                        this.on_bookmark_toggled(args);
                    }
                    0
                }),
            );
        }
        rc
    }

    fn tree_view(&self) -> Option<ElementRef> {
        self.parts.borrow().tree_view.clone()
    }

    fn with_tree_view(&self, f: impl FnOnce(&SPTreeViewElement)) {
        if let Some(tree) = self.tree_view() {
            if let Some(tree) = tree.downcast::<SPTreeViewElement>() {
                f(tree);
            }
        }
    }

    fn view_stack(&self) -> Option<ElementRef> {
        self.parts.borrow().view_stack.clone()
    }

    fn with_view_stack<R>(&self, f: impl FnOnce(&SPViewStackElement) -> R) -> Option<R> {
        let stack = self.view_stack()?;
        stack.downcast::<SPViewStackElement>().map(f)
    }

    fn set_search_value(&self, value: &str) {
        let search = self.parts.borrow().search_element.clone();
        if let Some(input) = search.as_ref().and_then(|s| s.downcast::<SPInputElement>()) {
            input.set_value(value);
        }
    }

    pub fn hide_message(&self) {
        if let Some(info) = self.parts.borrow().info_element.clone() {
            info.hide();
            info.element().invalidate();
        }
    }

    pub fn show_message(&self, message_type: SPMessageType, message: &str) {
        if let Some(info) = self.parts.borrow().info_element.clone() {
            info.show_message(message_type, message);
        }
    }

    /// The effective hostname for favicon fetching of a site/app/curator/store
    /// URI, with the hosted-domain suffix for bare names; "" otherwise.
    pub fn favicon_domain(uri: &str) -> String {
        const SCHEMES: [(&str, Option<&str>); 4] = [
            ("bungalow:site:", None),
            ("bungalow:app:", Some(".apps.bungalow.se")),
            ("bungalow:curator:", Some(".curator.bungalow.se")),
            ("bungalow:store:", Some(".store.bungalow.se")),
        ];
        for (prefix, hosted) in SCHEMES {
            let Some(remainder) = uri.strip_prefix(prefix) else { continue };
            let mut domain = remainder.split(':').next().unwrap_or_default().to_string();
            if let Some(hosted) = hosted {
                if !domain.contains('.') && !domain.is_empty() {
                    domain += hosted;
                }
            }
            return domain;
        }
        String::new()
    }

    /// Marks the tree items whose URI is `uri` as selected.
    fn select_tree_items(&self, uri: &str) {
        self.with_tree_view(|tree| {
            tree.for_each_item(|item| {
                if let Some(item_uri) = item.get_uri().map(SPUri::get_uri).filter(|u| !u.is_empty()) {
                    item.set_selected(item_uri == uri);
                }
            });
            tree.element().invalidate();
        });
    }

    // ── Navigation ────────────────────────────────────────────────────────

    pub fn navigate(&self, uri: &str) {
        let normalized = normalize_scheme(uri);
        self.with_view_stack(|stack| stack.navigate(&normalized));
        self.set_search_value(uri);
        self.select_tree_items(&normalized);
    }

    fn on_tree_item_selected(&self, e: &dyn SPEventArgs) {
        let Some(args) = downcast_args::<TreeViewEventArgs>(e) else { return };
        let Some(item) = args.get_item() else { return };
        let Some(uri) = item.get_uri().map(SPUri::get_uri).filter(|u| !u.is_empty()) else { return };
        self.navigate(&uri);
    }

    fn on_nav_item_context_menu(self: &Rc<Self>, e: &dyn SPEventArgs) {
        let Some(args) = downcast_args::<TreeViewEventArgs>(e) else { return };
        let Some(item) = args.get_item() else { return };
        if !item.is_bookmark() {
            return;
        }
        // Capture the URI before the menu closes (the item may be rebuilt).
        let uri = item.get_uri().map(SPUri::get_uri).unwrap_or_default();
        const ID_REMOVE_BOOKMARK: i32 = 10200;
        let weak = Rc::downgrade(self);
        platform_ui::show_popup_menu(
            &vec![(ID_REMOVE_BOOKMARK, "Remove bookmark".into())],
            Box::new(move |selection| {
                let Some(this) = weak.upgrade() else { return };
                if selection == ID_REMOVE_BOOKMARK && !uri.is_empty() {
                    if let Some(db) = bungalow_app_handle::current().and_then(|a| a.get_database()) {
                        db.remove_bookmark(&uri);
                    }
                    this.rebuild_nav_items();
                    this.with_tree_view(|tree| tree.element().invalidate());
                }
            }),
        );
    }

    /// Fired from SiteView / AppView.
    fn on_bookmark_toggled(self: &Rc<Self>, e: &dyn SPEventArgs) {
        let Some(args) = downcast_args::<SPBookmarkEventArgs>(e) else { return };
        let Some(db) = bungalow_app_handle::current().and_then(|a| a.get_database()) else { return };
        let (uri, title) = (args.get_uri(), args.get_title());

        if db.is_bookmarked(uri) {
            db.remove_bookmark(uri);
        } else {
            let parent = domain_uri(uri);
            if parent.is_empty() {
                db.add_bookmark(uri, title, 0);
            } else {
                if !db.is_bookmarked(&parent) {
                    db.add_bookmark(&parent, &parent, 0);
                }
                let parent_id = db.get_bookmark_id(&parent);
                db.add_bookmark(uri, title, parent_id);
            }

            // Kick off a favicon fetch for newly added site/app bookmarks.
            let domain = Self::favicon_domain(uri);
            if !domain.is_empty() {
                let weak: Weak<Self> = Rc::downgrade(self);
                if let Some(cache) = self.favicon_cache.borrow().as_ref() {
                    cache.fetch_if_needed(
                        &domain,
                        &format!("https://{domain}/favicon.ico"),
                        Box::new(move || {
                            if let Some(this) = weak.upgrade() {
                                this.rebuild_nav_items();
                                this.with_tree_view(|tree| tree.element().invalidate());
                            }
                        }),
                    );
                }
            }
        }

        self.rebuild_nav_items();
        self.with_tree_view(|tree| tree.element().invalidate());
    }

    // ── Nav rebuild (on startup and after bookmark changes) ───────────────

    fn rebuild_nav_items(&self) {
        let cache = self.favicon_cache.borrow();
        self.with_tree_view(|tree| {
            tree.clear_items();

            tree.add_section("MAIN");
            tree.add_item(tree_item("bungalow:start", "Start", "icons/NowPlaying.svg"));
            tree.add_item(tree_item("bungalow:player", "Now Playing", "icons/NowPlaying.svg"));

            tree.add_section("LIBRARY");
            let settings = tree_item("bungalow:config", "Settings", "icons/Document.svg");
            settings.set_collapsed(false); // expanded by default
            tree.add_item(settings);

            let library = tree_item("bungalow:library", "Library", "icons/Library.svg");
            library.add_child(tree_item("bungalow:library", "Tracks", "icons/MusicNote.svg"));
            tree.add_item(library.clone());
            library.add_child(tree_item("bungalow:localfiles", "Local Files", "icons/MusicFile.svg"));

            tree.add_item(tree_item("bungalow:downloads", "Downloads", "icons/Downloads.svg"));

            // The APPS, CURATORS, STORE and WEB SITES sections (with their
            // bookmarks) are switched off in the C++ client (`if (false)`).
            const SHOW_BOOKMARK_SECTIONS: bool = false;
            if SHOW_BOOKMARK_SECTIONS {
                let db = bungalow_app_handle::current().and_then(|a| a.get_database());
                // A cached favicon if there is one, otherwise the default icon.
                let resolve_icon = |uri: &str, fallback: &str| -> String {
                    let domain = Self::favicon_domain(uri);
                    match cache.as_ref() {
                        Some(cache) if !domain.is_empty() => {
                            let cached = cache.get_cached_path(&domain);
                            if cached.is_empty() {
                                fallback.to_string()
                            } else {
                                cached
                            }
                        }
                        _ => fallback.to_string(),
                    }
                };
                let add_bookmark_section = |prefix: &str, default_icon: &str| {
                    let Some(db) = &db else { return };
                    for parent in db.get_top_level_bookmarks(prefix) {
                        let parent_item =
                            tree_item(&parent.uri, &parent.title, &resolve_icon(&parent.uri, default_icon));
                        parent_item.set_is_bookmark(true);
                        parent_item.set_collapsed(false);
                        for child in db.get_child_bookmarks(parent.id) {
                            let child_item =
                                tree_item(&child.uri, &child.title, &resolve_icon(&child.uri, default_icon));
                            child_item.set_is_bookmark(true);
                            parent_item.add_child(child_item);
                        }
                        tree.add_item(parent_item);
                    }
                };

                tree.add_section("APPS");
                tree.add_item(tree_item(
                    "bungalow:app:finder",
                    "App Finder",
                    &resolve_icon("bungalow:app:finder", "icons/App.svg"),
                ));
                add_bookmark_section("bungalow:app:", "icons/App.svg");

                tree.add_section("CURATORS");
                add_bookmark_section("bungalow:curator:", "icons/WebPage.svg");

                tree.add_section("STORE");
                add_bookmark_section("bungalow:store:", "icons/WebPage.svg");

                tree.add_section("WEB SITES");
                add_bookmark_section("bungalow:site:", "icons/WebPage.svg");
            }
        });
    }

    // ── layout() ──────────────────────────────────────────────────────────

    fn layout(self: &Rc<Self>) {
        let el = self.element();
        el.set_clip_view(false);
        let this = el.this();
        let by_id = |id: &str| el.get_element_by_id(id);

        let header = by_id("header");
        if let Some(header) = &header {
            header.element().set_height(60);
            header.element().set_padding(SPMargin::new(5, 25, 5, 5));
            header.element().set_gap(5);
        }

        let weak = Rc::downgrade(self);
        let back_button = by_id("backButton");
        if let Some(button) = &back_button {
            let b = button.element();
            b.set_inner_text("<");
            b.set_width(28);
            b.set_height(28);
            b.add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(this) = weak.upgrade() {
                        this.on_back_clicked();
                    }
                    0
                }),
            );
        }

        let weak = Rc::downgrade(self);
        let forward_button = by_id("forwardButton");
        if let Some(button) = &forward_button {
            let b = button.element();
            b.set_inner_text(">");
            b.set_width(28);
            b.set_height(28);
            b.add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(this) = weak.upgrade() {
                        this.on_forward_clicked();
                    }
                    0
                }),
            );
        }

        let weak = Rc::downgrade(self);
        let search_element = by_id("searchInput");
        if let Some(search) = &search_element {
            let s = search.element();
            s.set_width(120);
            s.set_height(28);
            s.set_color_attribute("bgcolor", SPColor::from_html("#777777"));
            s.add_event_listener(
                "enter",
                Rc::new(move |_, e| {
                    if let (Some(this), Some(args)) = (weak.upgrade(), downcast_args::<SPInputEventArgs>(e)) {
                        this.on_search_entered(&args.get_text());
                    }
                    0
                }),
            );
        }

        // Created but, as in the C++ client, never appended to the tree.
        let info_element = SPInfoElement::new(Some(&this));
        info_element.element().set_height(28);
        info_element.element().this().set_id("Info");
        info_element.hide();

        let body = by_id("body");
        if let Some(body) = &body {
            body.element().set_flex(1);
        }
        let left_sidebar = by_id("leftSidebar");
        let tab_bar = left_sidebar
            .as_ref()
            .and_then(|s| s.element().get_element_by_id("leftTabBar"))
            .filter(|t| t.downcast::<SPTabBarElement>().is_some());

        let tree_view = by_id("treeview");
        if let Some(tree) = &tree_view {
            let weak = Rc::downgrade(self);
            tree.element().add_event_listener(
                "itemselected",
                Rc::new(move |_, e| {
                    if let Some(this) = weak.upgrade() {
                        this.on_tree_item_selected(e);
                    }
                    0
                }),
            );
            let weak = Rc::downgrade(self);
            tree.element().add_event_listener(
                "itemcontextmenu",
                Rc::new(move |_, e| {
                    if let Some(this) = weak.upgrade() {
                        this.on_nav_item_context_menu(e);
                    }
                    0
                }),
            );
            tree.element().set_flex(1);
        }

        {
            let mut parts = self.parts.borrow_mut();
            parts.header = header;
            parts.back_button = back_button;
            parts.forward_button = forward_button;
            parts.search_element = search_element;
            parts.info_element = Some(info_element);
            parts.body = body.clone();
            parts.left_sidebar = left_sidebar.clone();
            parts.tab_bar = tab_bar;
            parts.tree_view = tree_view;
        }
        self.rebuild_nav_items();

        // Now-playing widget pinned to the bottom of the sidebar.
        if let Some(sidebar) = &left_sidebar {
            let bar = SPNowPlayingBarElement::new(Some(sidebar));
            sidebar.append_child(&(bar.clone() as ElementRef));
            self.parts.borrow_mut().now_playing_bar = Some(bar);
        }

        // The view stack and its views.
        let view_stack = by_id("viewstack");
        if let Some(stack) = &view_stack {
            stack.element().set_flex(1);
            self.parts.borrow_mut().view_stack = Some(stack.clone());
            let app = bungalow_app_handle::current();
            let db = app.as_ref().and_then(|a| a.get_database());
            let dm = app.as_ref().and_then(|a| a.get_download_manager());
            let views: Vec<ElementRef> = vec![
                SpaceView::new(Some(stack)),
                PlayerView::new(Some(stack)),
                StartView::new(Some(stack)),
                ConfigView::new(Some(stack)),
                LibraryView::new(Some(stack), db.clone()),
                LocalFilesView::new(Some(stack), db),
                DownloadsView::new(Some(stack), dm),
                SiteView::new(Some(stack)),
                AppView::new(Some(stack)),
                CuratorView::new(Some(stack)),
                StoreView::new(Some(stack)),
            ];
            for view in views {
                stack.append_child(&view);
            }
        }

        if let Some(body) = &body {
            body.append_child(&(SPSplitterElement::new(Some(body)) as ElementRef));
            body.append_child(&(PlayQueueView::new(Some(body)) as ElementRef));
            body.pack();
        }

        let footer = by_id("footer");
        if let Some(footer) = &footer {
            footer.element().set_height(32);
            footer.set_id("footer");
            debug_assert!(footer.downcast::<SPAppFooterElement>().is_some());
        }
        self.parts.borrow_mut().footer = footer;

        self.navigate("bungalow:start");
    }

    fn on_search_entered(&self, text: &str) {
        let mut query = text.to_string();
        if !has_bungalow_scheme(&query) {
            query = format!("bungalow:search:{query}");
        }
        self.navigate(&query);
    }

    fn after_history_move(&self) {
        let uri = self.with_view_stack(|s| s.get_current_uri()).unwrap_or_default();
        self.set_search_value(&uri);
        self.select_tree_items(&uri);
    }

    fn on_back_clicked(&self) {
        if self.with_view_stack(|s| s.can_go_back()) != Some(true) {
            return;
        }
        self.with_view_stack(|s| s.go_back());
        self.after_history_move();
    }

    fn on_forward_clicked(&self) {
        if self.with_view_stack(|s| s.can_go_forward()) != Some(true) {
            return;
        }
        self.with_view_stack(|s| s.go_forward());
        self.after_history_move();
    }

    pub fn get_view_stack(&self) -> Option<ElementRef> {
        self.view_stack()
    }

    pub fn set_full_width_address_bar(&self, full_width: bool) {
        let Some(search) = self.parts.borrow().search_element.clone() else { return };
        let s = search.element();
        if full_width {
            s.set_flex(1);
            s.set_width(0);
        } else {
            s.set_flex(0);
            s.set_width(120);
        }
    }
}

impl SPElementClass for MainElement {
    fn get_type(&self) -> String {
        "main".into()
    }

    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }

    fn create_element(&self, tag_name: &str, parent: &ElementRef) -> ElementRef {
        match tag_name {
            "appheader" => SPAppHeaderElement::new(Some(parent)),
            "viewstack" => SPViewStackElement::new(Some(parent)),
            "treeview" => SPTreeViewElement::new(Some(parent)),
            "appfooter" => SPAppFooterElement::new(Some(parent)),
            "nowplayingbar" => SPNowPlayingBarElement::new(Some(parent)),
            "tabbar" => SPTabBarElement::new(Some(parent)),
            _ => space::sp_element_factory::create_element(tag_name, parent),
        }
    }
}
