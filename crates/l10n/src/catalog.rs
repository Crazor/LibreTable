//! The embedded catalog: every file in `/locales`.
//!
//! Adding a `.ftl` file means adding it here for every locale; the test
//! `tests/catalog.rs` fails while a file in `/locales` is missing from
//! [`SOURCES`] or a locale directory is missing from [`LOCALES`].

use crate::{Catalog, Source};

/// The source locale: English. Every other locale falls back to it.
pub const SOURCE_LOCALE: &str = "en";

/// Supported locales, the source locale first.
pub const LOCALES: &[&str] = &[SOURCE_LOCALE, "de"];

/// `locales/<locale>/<file>.ftl`, embedded.
macro_rules! source {
    ($locale:literal, $file:literal) => {
        Source {
            locale: $locale,
            file: $file,
            text: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../locales/",
                $locale,
                "/",
                $file,
                ".ftl"
            )),
        }
    };
}

/// Every message file of every locale.
pub const SOURCES: &[Source] = &[
    source!("en", "cli"),
    source!("en", "rock-paper-scissors"),
    source!("de", "cli"),
    source!("de", "rock-paper-scissors"),
];

/// The embedded catalog.
pub const CATALOG: Catalog = Catalog {
    locales: LOCALES,
    sources: SOURCES,
};
