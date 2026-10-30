//! Port of `CuratorView` (bungalow/): handles bungalow:curator: URIs. Bare
//! names expand to <name>.curator.bungalow.se.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase};

use super::site_view::{hosted_uri_to_url, hosted_url_to_uri, SiteView, SiteViewClass};

const PREFIX: &str = "bungalow:curator:";
const CURATOR_DOMAIN: &str = ".curator.bungalow.se";

#[derive(Default)]
pub struct CuratorView {
    pub site: SiteView,
}

impl_element_base!(CuratorView, site);

impl CuratorView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let outer: Rc<dyn SiteViewClass> = rc.clone();
        rc.site.init_site(&outer, PREFIX, true, true, true);
        rc
    }
}

impl SiteViewClass for CuratorView {
    fn site(&self) -> &SiteView {
        &self.site
    }
    fn accepts_site_uri(&self, uri: &str) -> bool {
        uri.starts_with(PREFIX)
    }
    /// bungalow:curator:mine -> https://mine.curator.bungalow.se,
    /// bungalow:curator:example.com:page -> https://example.com/page
    fn uri_to_url(&self, uri: &str) -> String {
        hosted_uri_to_url(uri, PREFIX, CURATOR_DOMAIN)
    }
    fn url_to_uri(&self, url: &str) -> String {
        hosted_url_to_uri(url, PREFIX, CURATOR_DOMAIN)
    }
}

site_view_subclass!(CuratorView, "view");
