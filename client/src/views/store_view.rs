//! Port of `StoreView` (bungalow/): handles bungalow:store: URIs. Bare
//! names expand to <name>.store.bungalow.se.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase};

use super::site_view::{hosted_uri_to_url, hosted_url_to_uri, SiteView, SiteViewClass};

const PREFIX: &str = "bungalow:store:";
const STORE_DOMAIN: &str = ".store.bungalow.se";

#[derive(Default)]
pub struct StoreView {
    pub site: SiteView,
}

impl_element_base!(StoreView, site);

impl StoreView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let outer: Rc<dyn SiteViewClass> = rc.clone();
        rc.site.init_site(&outer, PREFIX, true, true, true);
        rc
    }
}

impl SiteViewClass for StoreView {
    fn site(&self) -> &SiteView {
        &self.site
    }
    fn accepts_site_uri(&self, uri: &str) -> bool {
        uri.starts_with(PREFIX)
    }
    /// bungalow:store:mine -> https://mine.store.bungalow.se,
    /// bungalow:store:example.com:page -> https://example.com/page
    fn uri_to_url(&self, uri: &str) -> String {
        hosted_uri_to_url(uri, PREFIX, STORE_DOMAIN)
    }
    fn url_to_uri(&self, url: &str) -> String {
        hosted_url_to_uri(url, PREFIX, STORE_DOMAIN)
    }
}

site_view_subclass!(StoreView, "view");
