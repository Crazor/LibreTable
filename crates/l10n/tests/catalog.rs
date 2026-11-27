//! The localization check run in CI: `/locales` matches the embedded catalog,
//! every key of `en` is translated, and every message ID declared by code
//! has an `en` message (docs/design/localization.md).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use libretable_conformance::rock_paper_scissors;
use libretable_game_core::Game;
use libretable_l10n::{CATALOG, LOCALES, SOURCE_LOCALE, SOURCES};

/// Message IDs declared by code, by origin. Every game adds its
/// `MESSAGE_IDS` here under its game ID; the conformance suite checks that
/// the game emits no ID outside that list. Client crates that depend on this
/// crate check their own lists with `Localizer::missing` in their tests.
const CODE_IDS: &[(&str, &[&str])] = &[(
    rock_paper_scissors::RockPaperScissors::ID.as_str(),
    rock_paper_scissors::MESSAGE_IDS,
)];

fn locales_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales"))
}

fn names(dir: &Path, want_dirs: bool) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().unwrap().is_dir() == want_dirs)
        .map(|entry| entry.file_name().into_string().unwrap())
        .collect()
}

#[test]
fn every_locale_directory_is_supported() {
    let dirs = names(locales_dir(), true);
    let supported: BTreeSet<String> = LOCALES.iter().map(|l| (*l).to_owned()).collect();
    assert_eq!(dirs, supported, "directories in /locales vs. LOCALES");
    assert_eq!(LOCALES[0], SOURCE_LOCALE);
}

#[test]
fn every_file_in_locales_is_embedded() {
    for locale in LOCALES {
        let on_disk = names(&locales_dir().join(locale), false);
        let embedded: BTreeSet<String> = SOURCES
            .iter()
            .filter(|s| s.locale == *locale)
            .map(|s| format!("{}.ftl", s.file))
            .collect();
        assert_eq!(
            on_disk, embedded,
            "files in /locales/{locale} vs. SOURCES in crates/l10n/src/catalog.rs"
        );
    }
}

#[test]
fn the_catalog_is_complete() {
    let problems = CATALOG.check(CODE_IDS);
    let report: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        problems.is_empty(),
        "localization problems:\n{}",
        report.join("\n")
    );
}
