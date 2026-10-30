//! Port of the `spider/` directory of bungalow-client.
//!
//! The C++ sources are an early sketch (they are not part of any CMake
//! target); this port keeps the same three classes and makes them work.

pub mod sp_spider_element;
pub mod spider;
pub mod spider_view;

pub use sp_spider_element::SPSpiderElement;
pub use spider::Spider;
pub use spider_view::SpiderView;
