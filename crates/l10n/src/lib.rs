//! Localization for the clients: loads the Fluent message catalogs in
//! `/locales` and turns message IDs into text (see
//! `docs/design/localization.md`).
//!
//! The deterministic crates (`game-core`, `games/*`, `bots`, `conformance`)
//! never depend on this crate; they return message IDs only. Clients (CLI,
//! server, bindings) format those IDs with a [`Localizer`]:
//!
//! ```
//! use libretable_l10n::{Arg, Localizer};
//!
//! let de = Localizer::new("de-AT");
//! assert_eq!(de.locale(), "de");
//! assert_eq!(de.format("rps-rock", &[]), "Stein");
//! // An ID without any message comes back unchanged.
//! assert_eq!(de.format("no-such-message", &[]), "no-such-message");
//! # let _ = Arg::from(3);
//! ```
//!
//! The catalog is embedded at compile time, so a `Localizer` needs no files
//! at run time. [`Catalog::check`] is the CI check that keeps the catalog
//! complete.

#![forbid(unsafe_code)]

mod catalog;
mod check;

use std::fmt;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

pub use catalog::{CATALOG, LOCALES, SOURCE_LOCALE, SOURCES};
pub use check::Problem;

/// One `.ftl` file: `locales/<locale>/<file>.ftl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    /// Locale directory, e.g. `"de"`.
    pub locale: &'static str,
    /// File name without `.ftl`: a crate name without the `libretable-`
    /// prefix (`"cli"`) or a game ID (`"rock-paper-scissors"`).
    pub file: &'static str,
    /// The file's Fluent source.
    pub text: &'static str,
}

/// A set of locales and their message files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Catalog {
    /// Supported locales; the first is the source locale and the fallback
    /// for every other.
    pub locales: &'static [&'static str],
    /// Every message file of every locale.
    pub sources: &'static [Source],
}

impl Catalog {
    /// The source locale, `locales[0]`.
    ///
    /// # Panics
    ///
    /// If `locales` is empty.
    #[must_use]
    pub fn source_locale(&self) -> &'static str {
        self.locales[0]
    }

    /// The best supported locale for a BCP 47 tag such as `"de-CH"`: an exact
    /// match, else the first supported locale with the same language, else
    /// the source locale.
    #[must_use]
    pub fn negotiate(&self, requested: &str) -> &'static str {
        let Ok(requested) = requested.parse::<LanguageIdentifier>() else {
            return self.source_locale();
        };
        let parsed = |locale: &str| locale.parse::<LanguageIdentifier>().ok();
        self.locales
            .iter()
            .find(|locale| parsed(locale).as_ref() == Some(&requested))
            .or_else(|| {
                self.locales.iter().find(|locale| {
                    parsed(locale).is_some_and(|id| id.language == requested.language)
                })
            })
            .copied()
            .unwrap_or_else(|| self.source_locale())
    }

    /// A localizer for the best match of `requested` (see
    /// [`negotiate`](Self::negotiate)), falling back to the source locale.
    #[must_use]
    pub fn localizer(&self, requested: &str) -> Localizer {
        let locale = self.negotiate(requested);
        let mut chain = vec![locale];
        if locale != self.source_locale() {
            chain.push(self.source_locale());
        }
        Localizer {
            locale,
            bundles: chain
                .into_iter()
                .map(|locale| self.bundle(locale))
                .collect(),
        }
    }

    /// All files of `locale` in one bundle. Never fails: syntax errors and
    /// duplicate IDs are reported by [`check`](Self::check), not here.
    fn bundle(&self, locale: &'static str) -> FluentBundle<FluentResource> {
        let id = locale.parse().unwrap_or_default();
        let mut bundle = FluentBundle::new_concurrent(vec![id]);
        // Unicode bidi isolation marks around placeables only matter for
        // right-to-left locales and show up as noise in terminals.
        bundle.set_use_isolating(false);
        for source in self.sources.iter().filter(|s| s.locale == locale) {
            let resource = FluentResource::try_new(source.text.to_owned())
                .unwrap_or_else(|(partial, _errors)| partial);
            bundle.add_resource_overriding(resource);
        }
        bundle
    }
}

/// A value for a message's `{ $variable }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg<'a> {
    /// Text, inserted as is (a player name, an already formatted message).
    Text(&'a str),
    /// A number; Fluent picks plural forms with it.
    Number(i64),
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(text: &'a str) -> Self {
        Self::Text(text)
    }
}

impl<'a> From<&'a String> for Arg<'a> {
    fn from(text: &'a String) -> Self {
        Self::Text(text)
    }
}

macro_rules! number_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg<'_> {
            fn from(n: $t) -> Self {
                Self::Number(i64::from(n))
            }
        }
    )*};
}
number_arg!(i8, i16, i32, i64, u8, u16, u32);

/// Formats message IDs in one locale, falling back to the source locale.
///
/// `Send + Sync`, so one localizer per locale can be shared.
pub struct Localizer {
    locale: &'static str,
    /// The chosen locale first, then the source locale if different.
    bundles: Vec<FluentBundle<FluentResource>>,
}

impl Localizer {
    /// A localizer over the embedded catalog for the best match of a BCP 47
    /// tag such as `"de-DE"`. Unsupported or malformed tags get the source
    /// locale, `en`.
    #[must_use]
    pub fn new(requested: &str) -> Self {
        CATALOG.localizer(requested)
    }

    /// The locale chosen for this localizer, e.g. `"de"`.
    #[must_use]
    pub fn locale(&self) -> &'static str {
        self.locale
    }

    /// Whether `id` has a message with a value in this locale or the source
    /// locale.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.bundles
            .iter()
            .any(|bundle| bundle.get_message(id).is_some_and(|m| m.value().is_some()))
    }

    /// The IDs among `ids` that [`has`](Self::has) no message for. Clients
    /// test their own ID lists with it.
    #[must_use]
    pub fn missing<'a>(&self, ids: &[&'a str]) -> Vec<&'a str> {
        ids.iter().copied().filter(|id| !self.has(id)).collect()
    }

    /// The text of message `id`: from this locale, else from the source
    /// locale, else `None`. A missing or wrongly typed argument is shown as
    /// `{$name}` in the text rather than failing.
    #[must_use]
    pub fn try_format(&self, id: &str, args: &[(&str, Arg<'_>)]) -> Option<String> {
        let args = (!args.is_empty()).then(|| {
            let mut fluent = FluentArgs::with_capacity(args.len());
            for (name, value) in args {
                let value = match *value {
                    Arg::Text(text) => FluentValue::from(text),
                    Arg::Number(n) => FluentValue::from(n),
                };
                fluent.set(*name, value);
            }
            fluent
        });
        self.bundles.iter().find_map(|bundle| {
            let pattern = bundle.get_message(id)?.value()?;
            let mut errors = Vec::new();
            Some(
                bundle
                    .format_pattern(pattern, args.as_ref(), &mut errors)
                    .into_owned(),
            )
        })
    }

    /// Like [`try_format`](Self::try_format), but an ID without any message
    /// comes back as the ID itself, so a gap in the catalog shows up as
    /// visible text instead of an error. The CI check keeps such gaps out of
    /// the shipped catalog.
    #[must_use]
    pub fn format(&self, id: &str, args: &[(&str, Arg<'_>)]) -> String {
        self.try_format(id, args).unwrap_or_else(|| id.to_owned())
    }
}

impl fmt::Debug for Localizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Localizer")
            .field("locale", &self.locale)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST: Catalog = Catalog {
        locales: &["en", "de"],
        sources: &[
            Source {
                locale: "en",
                file: "test",
                text: "hello = Hello, { $name }!\n\
                       cards = { $count ->\n    [one] one card\n   *[other] { $count } cards\n}\n\
                       only-en = Only English\n",
            },
            Source {
                locale: "de",
                file: "test",
                text: "hello = Hallo, { $name }!\n\
                       cards = { $count ->\n    [one] eine Karte\n   *[other] { $count } Karten\n}\n",
            },
        ],
    };

    #[test]
    fn negotiates_exact_language_then_source() {
        assert_eq!(TEST.negotiate("de"), "de");
        assert_eq!(TEST.negotiate("de-CH"), "de");
        assert_eq!(TEST.negotiate("de_DE"), "de");
        assert_eq!(TEST.negotiate("en-GB"), "en");
        assert_eq!(TEST.negotiate("fr"), "en");
        assert_eq!(TEST.negotiate("not a tag!"), "en");
    }

    #[test]
    fn formats_arguments_and_plurals() {
        let de = TEST.localizer("de");
        assert_eq!(
            de.format("hello", &[("name", "Anna".into())]),
            "Hallo, Anna!"
        );
        assert_eq!(de.format("cards", &[("count", 1.into())]), "eine Karte");
        assert_eq!(de.format("cards", &[("count", 3.into())]), "3 Karten");
        let en = TEST.localizer("en");
        assert_eq!(en.format("cards", &[("count", 1.into())]), "one card");
    }

    #[test]
    fn falls_back_to_the_source_locale_then_the_id() {
        let de = TEST.localizer("de");
        assert_eq!(de.format("only-en", &[]), "Only English");
        assert!(de.has("only-en"));
        assert_eq!(de.try_format("unknown", &[]), None);
        assert_eq!(de.format("unknown", &[]), "unknown");
        assert_eq!(de.missing(&["hello", "unknown"]), ["unknown"]);
    }

    #[test]
    fn a_missing_argument_does_not_fail() {
        let en = TEST.localizer("en");
        assert_eq!(en.format("hello", &[]), "Hello, {$name}!");
    }

    #[test]
    fn the_embedded_catalog_formats_game_ids() {
        let en = Localizer::new("en-US");
        assert_eq!(
            en.format("rock-paper-scissors", &[]),
            "Rock, Paper, Scissors"
        );
        assert_eq!(Localizer::new("de").format("rps-rock", &[]), "Stein");
    }

    #[test]
    fn a_localizer_can_be_shared_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Localizer>();
    }
}
