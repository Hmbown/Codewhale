//! Shared web-tool helpers: the SSRF guard and the search scrapers.
//!
//! `fetch_url`, `web_search`, and `web.run` are thin surfaces over this
//! module so security and parsing behavior cannot drift between tools.

pub(crate) mod adapter;
pub(crate) mod backend;
pub(crate) mod cache;
pub(crate) mod citations;
pub(crate) mod contract;
pub(crate) mod extract;
pub(crate) mod fetch;
pub(crate) mod guard;
pub(crate) mod overflow;
pub mod scrape;
